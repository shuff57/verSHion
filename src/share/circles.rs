//! The Circles tab: circles I am in, one circle's page, a circle's bank, joining with a code,
//! copying a bank. No egui: tests drive every path with a scripted session.
//!
//! # Screens
//!
//! ```text
//! Gate --(signed in and approved)--> List --open--> Circle --open--> Bank
//!                                      |--> Create        |--> a panel: code | confirm leave/close/remove/transfer | rename
//!                                      \--> Join (code -> preview -> join)
//! ```
//!
//! Rules this module keeps (the same as the other pool models):
//!
//! - A closed model does no I/O. Nothing goes to the network until the page is open and the account
//!   is signed in and approved.
//! - What a person may do is decided in ONE place ([`can`], [`bank_can`]) from their role and the
//!   circle's state; the buttons are drawn from it and every action method checks it again.
//! - Server text is kept raw and drawn through [`api::plain`]. It is never markup and never a link.
//! - A circle code is a [`Secret`]: shown once, wiped when the panel closes, never logged.
//! - A cancelled or superseded request is forgotten: its receiver is dropped, a late answer is
//!   ignored. A result for a circle or bank that is no longer on screen is ignored.
//! - A copy of a bank is made on this device and recorded like an import (`BrowseModel::record_circle_copy`),
//!   so it shows under "Things I imported". Nothing a person does to the copy goes back to the circle.

use std::sync::Arc;
use std::sync::mpsc::{Receiver, Sender, TryRecvError, channel};
use std::time::Duration;

use web_time::Instant;

use super::api::{self, ApiError};
use super::browse::{
    Applied, CircleOrigin, Dest, ImageSink, Imported, Problem, SharedImages, StoreImages, apply_import, now_ms, theirs_of,
};
use super::circles_api::*;
use super::circles_text as text;
use super::model::{Notice, Runner};
use super::pool::{ImportInfo, ItemHead, ItemView, License, Pool, PoolError, RefusalKind, RetryPolicy, Served, StudentView, is_sha256};
use super::publish::{AccountSnapshot, Gate, SharedSession};
use super::secret::Secret;
use crate::model::{ImageRef, Question, QuestionBank};

// ---- what a person may do ----------------------------------------------------------------------------

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Action {
    Rename,
    MakeCode,
    WithdrawCode,
    ChangeRole,
    RemoveMember,
    Transfer,
    Close,
    Reopen,
    Leave,
    AddBank,
}

/// What `role` may do in a circle that is (not) closed.
pub fn can(role: Role, closed: bool, a: Action) -> bool {
    match a {
        Action::Reopen => role == Role::Owner && closed,
        Action::Leave => true,
        _ if closed => false,
        Action::AddBank => role.can_add_banks(),
        Action::Rename | Action::MakeCode | Action::WithdrawCode | Action::ChangeRole | Action::RemoveMember | Action::Transfer | Action::Close => role == Role::Owner,
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BankAction {
    Look,
    Copy,
    Report,
    Remove,
    Restore,
    SetLicense,
    /// The owner deletes a removed bank for good, now.
    Purge,
}

/// What a member may do with one bank. `circle_role` is the person's role in the circle.
pub fn bank_can(circle_role: Role, closed: bool, b: &BankEntry, a: BankAction) -> bool {
    let removed = b.removed.is_some();
    match a {
        BankAction::Look | BankAction::Copy => !removed && !closed,
        BankAction::Report => !removed && !closed && !b.is_author,
        BankAction::Remove => !removed && !closed && (circle_role == Role::Owner || b.is_author),
        BankAction::Restore => {
            // Only the author's own removal and the owner's can be undone (not an erasure, an admin or a closed circle);
            // the author can not undo the owner's removal.
            let reason = b.removed.as_ref().and_then(|r| r.reason.as_deref()).unwrap_or_default();
            removed && !closed && (circle_role == Role::Owner && matches!(reason, "author_removed" | "owner_removed") || b.is_author && reason == "author_removed")
        }
        BankAction::Purge => {
            let reason = b.removed.as_ref().and_then(|r| r.reason.as_deref()).unwrap_or_default();
            removed && !closed && circle_role == Role::Owner && !matches!(reason, "admin" | "privacy" | "takedown")
        }
        BankAction::SetLicense => !removed && !closed && b.is_author,
    }
}

// ---- screens and state -----------------------------------------------------------------------------------

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Screen {
    /// Waiting for a signed-in, approved account.
    Gate,
    List,
    Circle,
    Bank,
    Join,
    Create,
}

#[derive(Debug, Default)]
pub struct ListState {
    pub loaded: bool,
    pub items: Vec<CircleSummary>,
    pub problem: Option<Problem>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct CodeForm {
    pub role: Role,
    pub people: u32,
    pub days: u32,
    pub note: String,
}

impl Default for CodeForm {
    fn default() -> Self {
        CodeForm { role: Role::Editor, people: 1, days: 7, note: String::new() }
    }
}

#[derive(Debug, Default)]
pub enum Panel {
    #[default]
    None,
    NewCode(CodeForm),
    /// A code was just made: shown once.
    Code(Box<InviteCreated>),
    ConfirmLeave,
    ConfirmClose,
    ConfirmRemove(String),
    ConfirmTransfer(String),
    Rename { name: String },
}

#[derive(Debug, Default)]
pub struct CircleView {
    pub id: String,
    pub detail: Option<CircleDetail>,
    pub problem: Option<Problem>,
    /// Owner only; `None` until loaded.
    pub invites: Option<Vec<Invite>>,
    pub invites_next: Option<String>,
    pub invites_problem: Option<Problem>,
    pub activity: Vec<ActivityEntry>,
    pub activity_next: Option<String>,
    pub activity_loaded: bool,
    pub activity_problem: Option<Problem>,
    /// The last change's failure.
    pub act_problem: Option<Problem>,
    /// A removed bank the owner is about to delete for good.
    pub confirm_purge: Option<String>,
}

impl CircleView {
    pub fn role(&self) -> Role {
        self.detail.as_ref().map(|d| d.circle.role()).unwrap_or(Role::Viewer)
    }
    pub fn closed(&self) -> bool {
        self.detail.as_ref().is_some_and(|d| d.circle.closed())
    }
}

#[derive(Debug)]
pub enum JoinStage {
    Typing,
    Looking,
    /// What the code is for; nothing is used yet. Holds the cleaned code.
    Preview(Box<JoinPreview>, Secret),
    Joining,
    Failed(Problem),
    Done { name: String, role: Role, already: bool, circle_id: String },
}

#[derive(Debug)]
pub struct JoinState {
    pub text: String,
    pub stage: JoinStage,
}

impl Default for JoinState {
    fn default() -> Self {
        JoinState { text: String::new(), stage: JoinStage::Typing }
    }
}

#[derive(Debug, Default)]
pub struct CreateForm {
    pub name: String,
    pub description: String,
    pub working: bool,
    pub problem: Option<Problem>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct CopyDone {
    pub bank_name: String,
    pub added: usize,
    pub pictures: usize,
    pub pictures_missing: usize,
    pub version: u64,
}

#[derive(Debug, Default)]
pub enum CopyPanel {
    #[default]
    Closed,
    Choosing { name: String },
    Working,
    Done(CopyDone),
    Failed(Problem),
}

#[derive(Debug)]
pub struct BankLoaded {
    pub doc: BankDoc,
    pub view: Result<StudentView, String>,
}

#[derive(Debug)]
pub enum BankLoad {
    Loading,
    Ready(Box<BankLoaded>),
    Failed(Problem),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ReportReasonChoice {
    StudentData,
    Copyright,
    Inappropriate,
    Other,
}

impl ReportReasonChoice {
    pub const ALL: [ReportReasonChoice; 4] = [ReportReasonChoice::StudentData, ReportReasonChoice::Copyright, ReportReasonChoice::Inappropriate, ReportReasonChoice::Other];
    pub fn wire(self) -> &'static str {
        match self {
            ReportReasonChoice::StudentData => "student_data",
            ReportReasonChoice::Copyright => "copyright",
            ReportReasonChoice::Inappropriate => "inappropriate",
            ReportReasonChoice::Other => "other",
        }
    }
    pub fn label(self) -> &'static str {
        match self {
            ReportReasonChoice::StudentData => "It shows information about students",
            ReportReasonChoice::Copyright => "It is copied from a book or test bank",
            ReportReasonChoice::Inappropriate => "It is inappropriate",
            ReportReasonChoice::Other => "Something else",
        }
    }
}

#[derive(Debug)]
pub enum ReportState {
    Editing,
    Sending,
    Sent,
    Failed(Problem),
}

#[derive(Debug)]
pub struct ReportForm {
    pub reason: ReportReasonChoice,
    pub note: String,
    pub state: ReportState,
}

#[derive(Debug)]
pub struct BankView {
    pub cid: String,
    pub bid: String,
    pub load: BankLoad,
    pub copy: CopyPanel,
    pub report: Option<ReportForm>,
    pub act_problem: Option<Problem>,
    pub confirm_remove: bool,
    /// The licence the author is about to set.
    pub license_choice: Option<License>,
}

// ---- jobs -------------------------------------------------------------------------------------------------------

#[derive(Clone, Debug)]
enum Act {
    Create { name: String, description: String },
    Rename { cid: String, name: String },
    Close(String),
    Reopen(String),
    MakeCode { cid: String, role: Role, people: u32, days: u32, note: String },
    Withdraw { cid: String, invite: String },
    Leave(String),
    Remove { cid: String, user: String },
    SetRole { cid: String, user: String, role: Role },
    Offer { cid: String, user: String },
    AcceptOffer(String),
    CancelOffer(String),
    SetLicense { cid: String, bid: String, license: License },
    RemoveBank { cid: String, bid: String },
    RestoreBank { cid: String, bid: String },
    PurgeBank { cid: String, bid: String },
    Report { cid: String, bid: String, reason: ReportReasonChoice, note: String },
}

enum ActDone {
    Created(Box<CircleInfo>),
    Done,
    Closed(Option<i64>),
    Code(Box<InviteCreated>),
    RoleSet(bool),
    BankRemoved(Option<i64>),
}

#[derive(Clone)]
enum Work {
    List,
    Detail(String),
    Invites(String, Option<String>),
    Activity(String, Option<String>),
    Bank(String, String),
    Copy(Box<BankDoc>, String),
    Preview(String),
    Join(String),
    Heads(Vec<String>),
    Act(Act),
}

impl std::fmt::Debug for Work {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Work({:?})", self.kind())
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum JobKind {
    List,
    Detail,
    Invites,
    Activity,
    Bank,
    Copy,
    /// Preview and Join of a code: one at a time.
    Code,
    Heads,
    /// Any change: one at a time.
    Act,
}

impl Work {
    fn kind(&self) -> JobKind {
        match self {
            Work::List => JobKind::List,
            Work::Detail(_) => JobKind::Detail,
            Work::Invites(..) => JobKind::Invites,
            Work::Activity(..) => JobKind::Activity,
            Work::Bank(..) => JobKind::Bank,
            Work::Copy(..) => JobKind::Copy,
            Work::Preview(_) | Work::Join(_) => JobKind::Code,
            Work::Heads(_) => JobKind::Heads,
            Work::Act(_) => JobKind::Act,
        }
    }
}

enum JobMsg {
    List(Result<Vec<CircleSummary>, PoolError>),
    Detail(String, Result<CircleDetail, PoolError>),
    Invites(String, bool, Result<InvitePage, PoolError>),
    Activity(String, bool, Result<ActivityPage, PoolError>),
    Bank(String, String, Result<BankDoc, PoolError>),
    Copy(Box<(String, String)>, Result<Box<Imported>, PoolError>),
    Preview(Result<JoinPreview, PoolError>, String),
    Join(Result<Joined, PoolError>, String),
    Heads(Result<Vec<HeadEntry>, PoolError>),
    Act(Act, Result<ActDone, PoolError>),
}

impl JobMsg {
    fn kind(&self) -> JobKind {
        match self {
            JobMsg::List(_) => JobKind::List,
            JobMsg::Detail(..) => JobKind::Detail,
            JobMsg::Invites(..) => JobKind::Invites,
            JobMsg::Activity(..) => JobKind::Activity,
            JobMsg::Bank(..) => JobKind::Bank,
            JobMsg::Copy(..) => JobKind::Copy,
            JobMsg::Preview(..) | JobMsg::Join(..) => JobKind::Code,
            JobMsg::Heads(_) => JobKind::Heads,
            JobMsg::Act(..) => JobKind::Act,
        }
    }
}

struct Job {
    kind: JobKind,
    rx: Receiver<JobMsg>,
}

async fn do_act(pool: &Pool<'_>, a: &Act) -> Result<ActDone, PoolError> {
    Ok(match a {
        Act::Create { name, description } => ActDone::Created(Box::new(pool.create_circle(name, description).await?)),
        Act::Rename { cid, name } => {
            pool.update_circle(cid, Some(name), None).await?;
            ActDone::Done
        }
        Act::Close(cid) => ActDone::Closed(pool.close_circle(cid).await?),
        Act::Reopen(cid) => {
            pool.reopen_circle(cid).await?;
            ActDone::Done
        }
        Act::MakeCode { cid, role, people, days, note } => ActDone::Code(Box::new(pool.create_invite(cid, *role, *people, *days, note).await?)),
        Act::Withdraw { cid, invite } => {
            pool.revoke_invite(cid, invite).await?;
            ActDone::Done
        }
        Act::Leave(cid) => {
            pool.leave_circle(cid).await?;
            ActDone::Done
        }
        Act::Remove { cid, user } => {
            pool.remove_member(cid, user).await?;
            ActDone::Done
        }
        Act::SetRole { cid, user, role } => ActDone::RoleSet(pool.set_member_role(cid, user, *role).await?),
        Act::Offer { cid, user } => {
            pool.offer_ownership(cid, user).await?;
            ActDone::Done
        }
        Act::AcceptOffer(cid) => {
            pool.accept_ownership(cid).await?;
            ActDone::Done
        }
        Act::CancelOffer(cid) => {
            pool.cancel_ownership_offer(cid).await?;
            ActDone::Done
        }
        Act::SetLicense { cid, bid, license } => {
            pool.set_bank_license(cid, bid, *license).await?;
            ActDone::Done
        }
        Act::RemoveBank { cid, bid } => ActDone::BankRemoved(pool.remove_circle_bank(cid, bid).await?),
        Act::RestoreBank { cid, bid } => {
            pool.restore_circle_bank(cid, bid).await?;
            ActDone::Done
        }
        Act::PurgeBank { cid, bid } => {
            pool.purge_circle_bank(cid, bid).await?;
            ActDone::Done
        }
        Act::Report { cid, bid, reason, note } => {
            pool.report_circle_bank(cid, bid, reason.wire(), note).await?;
            ActDone::Done
        }
    })
}

async fn run_work_async(session: SharedSession, images: SharedImages, retry: RetryPolicy, work: Work, tx: Sender<JobMsg>, repaint: Option<Arc<dyn Fn() + Send + Sync>>) {
    let pool = Pool::new(&*session).with_retry(retry);
    let msg = match work {
        Work::List => JobMsg::List(pool.circles().await),
        Work::Detail(cid) => {
            let r = pool.circle(&cid).await;
            JobMsg::Detail(cid, r)
        }
        Work::Invites(cid, after) => {
            let r = pool.invites(&cid, after.as_deref()).await;
            JobMsg::Invites(cid, after.is_none(), r)
        }
        Work::Activity(cid, before) => {
            let r = pool.circle_activity(&cid, before.as_deref()).await;
            JobMsg::Activity(cid, before.is_none(), r)
        }
        Work::Bank(cid, bid) => {
            let r = pool.circle_bank(&cid, &bid).await;
            JobMsg::Bank(cid, bid, r)
        }
        Work::Copy(doc, _name) => {
            let key = Box::new((doc.circle_id.clone(), doc.bank.id.clone()));
            let r = copy_bank(&pool, &*images, &doc).await.map(Box::new);
            JobMsg::Copy(key, r)
        }
        Work::Preview(code) => {
            let r = pool.join_preview(&code).await;
            JobMsg::Preview(r, code)
        }
        Work::Join(code) => {
            let r = pool.join_circle(&code).await;
            JobMsg::Join(r, code)
        }
        Work::Heads(ids) => JobMsg::Heads(pool.circle_heads(&ids).await),
        Work::Act(a) => {
            let r = do_act(&pool, &a).await;
            JobMsg::Act(a, r)
        }
    };
    let _ = tx.send(msg);
    if let Some(r) = repaint {
        r();
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn run_work(session: SharedSession, images: SharedImages, retry: RetryPolicy, work: Work, tx: Sender<JobMsg>, repaint: Option<Arc<dyn Fn() + Send + Sync>>) {
    super::transport::block_on(run_work_async(session, images, retry, work, tx, repaint));
}

/// The bank as the pool sent it, in the shape of this app: pictures fetched from the circle and
/// stored here, new ids for every question.
async fn copy_bank(pool: &Pool<'_>, images: &(dyn ImageSink + Send + Sync), doc: &BankDoc) -> Result<Imported, PoolError> {
    let b = &doc.bank;
    let license = if b.version_license.is_empty() { b.license.clone() } else { b.version_license.clone() };
    let item = ItemView {
        id: b.id.clone(),
        kind: "bank".into(),
        title: b.title.clone(),
        subject: b.subject.clone(),
        course: b.course.clone(),
        grade: b.grade.clone(),
        question_count: b.question_count,
        version: b.version,
        publish_id: b.version_id.clone(),
        license: license.clone(),
        hide_key: false,
        has_images: b.has_images,
        author_name: b.author_name.clone(),
        committed_at: b.committed_at,
        form: "full".into(),
        is_author: b.is_author,
        ..ItemView::default()
    };
    let head = ItemHead { item, questions: Vec::new(), license_check: None };
    let at_ms = now_ms();
    let mut theirs = theirs_of(&doc.payload, &head, at_ms);
    if theirs.is_empty() {
        return Err(PoolError::Local(super::pool::LocalProblem::BadItemShape));
    }
    let mut stored: std::collections::HashMap<String, Option<ImageRef>> = Default::default();
    let (mut pictures, mut missing) = (0usize, 0usize);
    for t in &mut theirs {
        let Some(sha) = t.sha.clone() else { continue };
        pictures += 1;
        let image = match stored.get(&sha) {
            Some(done) => done.clone(),
            None => {
                let got = if is_sha256(&sha) {
                    match pool.circle_blob(&doc.circle_id, &b.id, &sha).await {
                        Ok(blob) => images.put(&blob.bytes).ok(),
                        Err(PoolError::Api(ApiError::SignedOut)) => return Err(PoolError::Api(ApiError::SignedOut)),
                        Err(_) => None,
                    }
                } else {
                    None
                };
                stored.insert(sha, got.clone());
                got
            }
        };
        if image.is_none() {
            missing += 1;
        }
        t.q.image = image;
    }
    let questions: Vec<Question> = theirs.iter().map(|t| t.q.clone()).collect();
    let fingerprints = theirs.iter().map(|t| t.fp.clone()).collect();
    let info = ImportInfo { license, ..ImportInfo::default() };
    Ok(Imported { head, info, questions, fingerprints, pictures, pictures_missing: missing, keyless: false, at_ms, theirs })
}

// ---- the model ----------------------------------------------------------------------------------------------------------

/// A finished copy waiting to be put into the banks by [`CirclesModel::sync`].
struct ReadyCopy {
    imp: Box<Imported>,
    name: String,
    circle: CircleOrigin,
}

pub struct CirclesModel {
    pub open: bool,
    pub screen: Screen,
    pub gate: Gate,
    pub notice: Option<Notice>,
    pub list: ListState,
    pub circle: Option<CircleView>,
    pub panel: Panel,
    pub join: JoinState,
    pub create: CreateForm,
    pub bank: Option<BankView>,
    /// The pool answered `invite_required` (the Shared page then asks for the pool's invite code).
    pub invite_required: bool,
    open_account: bool,
    resume: Screen,
    session: Option<SharedSession>,
    images: SharedImages,
    runner: Runner,
    retry: RetryPolicy,
    jobs: Vec<Job>,
    ready: Option<ReadyCopy>,
    last_act: Option<Act>,
    heads_wanted: Option<Vec<String>>,
    heads_done: bool,
    heads: Option<Vec<HeadEntry>>,
    copy_name: Option<String>,
    joining_name: Option<String>,
    clock: Option<i64>,
    now: Option<Instant>,
    repaint: Option<Arc<dyn Fn() + Send + Sync>>,
}

impl Default for CirclesModel {
    fn default() -> Self {
        CirclesModel::new(Runner::Background, Arc::new(StoreImages))
    }
}

impl CirclesModel {
    /// Does no I/O: nothing is read or sent until the page is opened.
    pub fn new(runner: Runner, images: SharedImages) -> CirclesModel {
        CirclesModel {
            open: false,
            screen: Screen::Gate,
            gate: Gate::Checking,
            notice: None,
            list: ListState::default(),
            circle: None,
            panel: Panel::None,
            join: JoinState::default(),
            create: CreateForm::default(),
            bank: None,
            invite_required: false,
            open_account: false,
            resume: Screen::List,
            session: None,
            images,
            runner,
            retry: RetryPolicy::default(),
            jobs: Vec::new(),
            ready: None,
            last_act: None,
            heads_wanted: None,
            heads_done: false,
            heads: None,
            copy_name: None,
            joining_name: None,
            clock: None,
            now: None,
            repaint: None,
        }
    }

    pub fn with_retry(mut self, retry: RetryPolicy) -> CirclesModel {
        self.retry = retry;
        self
    }

    pub fn set_repaint(&mut self, f: Arc<dyn Fn() + Send + Sync>) {
        self.repaint = Some(f);
    }

    pub fn set_clock(&mut self, ms: i64) {
        self.clock = Some(ms);
    }

    pub fn now_ms(&self) -> i64 {
        self.clock.unwrap_or_else(now_ms)
    }

    pub fn busy(&self) -> bool {
        !self.jobs.is_empty()
    }

    /// A change is on its way (the page should not be left in the middle of it).
    pub fn busy_action(&self) -> bool {
        self.running(JobKind::Act) || self.running(JobKind::Copy)
    }

    fn running(&self, k: JobKind) -> bool {
        self.jobs.iter().any(|j| j.kind == k)
    }

    // ---- opening --------------------------------------------------------------------------------------------

    fn reset(&mut self, screen: Screen) {
        self.cancel_all();
        self.open = true;
        self.notice = None;
        self.list = ListState::default();
        self.circle = None;
        self.panel = Panel::None;
        self.join = JoinState::default();
        self.create = CreateForm::default();
        self.bank = None;
        self.open_account = false;
        self.heads_done = false;
        self.resume = screen;
        self.screen = Screen::Gate;
    }

    /// Opens the Circles tab at the list.
    pub fn open_list(&mut self) {
        self.reset(Screen::List);
    }

    /// Opens straight into the "enter a code" page (a code was pasted or a link asked for it).
    pub fn open_join(&mut self, text: &str) {
        self.reset(Screen::Join);
        self.join.text = text.chars().take(500).collect();
    }

    /// Opens straight into "Create a circle".
    pub fn open_create(&mut self) {
        self.reset(Screen::Create);
    }

    /// Opens one circle.
    pub fn open_circle_at(&mut self, cid: &str) {
        self.reset(Screen::List);
        if super::pool::is_item_id(cid) {
            self.circle = Some(CircleView { id: cid.to_string(), ..CircleView::default() });
            self.resume = Screen::Circle;
        }
    }

    /// Opens one circle's bank.
    pub fn open_bank_at(&mut self, cid: &str, bid: &str) {
        self.reset(Screen::List);
        if super::pool::is_item_id(cid) && super::pool::is_item_id(bid) {
            self.circle = Some(CircleView { id: cid.to_string(), ..CircleView::default() });
            self.bank = Some(new_bank_view(cid, bid));
            self.resume = Screen::Bank;
        }
    }

    pub fn close(&mut self) {
        self.open = false;
        self.cancel_all();
        self.wipe_secrets();
        self.ready = None;
    }

    fn cancel_all(&mut self) {
        self.jobs.clear();
    }

    /// Nothing typed or shown once stays behind.
    fn wipe_secrets(&mut self) {
        self.join = JoinState::default();
        if matches!(self.panel, Panel::Code(_)) {
            self.panel = Panel::None;
        }
    }

    pub fn take_open_account(&mut self) -> bool {
        std::mem::take(&mut self.open_account)
    }

    pub fn request_open_account(&mut self) {
        self.open_account = true;
    }

    // ---- frame -----------------------------------------------------------------------------------------------

    /// Applies finished jobs and the account's state, and loads what the screen shows. Does nothing
    /// when closed.
    pub fn frame(&mut self, now: Instant, snap: &AccountSnapshot) {
        if !self.open {
            return;
        }
        self.now = Some(now);
        self.apply_snapshot(snap);
        self.drain();
        self.auto();
    }

    fn apply_snapshot(&mut self, snap: &AccountSnapshot) {
        self.session = snap.session.clone();
        self.gate = snap.gate.clone();
        if self.screen == Screen::Gate {
            if self.gate == Gate::Ready {
                self.screen = self.resume;
            }
        } else if matches!(self.gate, Gate::SignedOut | Gate::NotApproved(_) | Gate::NotSetUp | Gate::NoServer) {
            self.resume = self.screen;
            self.screen = Screen::Gate;
        }
    }

    fn auto(&mut self) {
        if self.screen == Screen::Gate || self.session.is_none() {
            return;
        }
        if !self.list.loaded && self.list.problem.is_none() && !self.running(JobKind::List) {
            self.launch(Work::List);
        }
        if let Some(ids) = self.heads_wanted.take()
            && !ids.is_empty()
            && !self.heads_done
        {
            self.heads_done = true;
            self.launch(Work::Heads(ids));
        }
        if matches!(self.screen, Screen::Circle | Screen::Bank) {
            self.auto_circle();
        }
        if self.screen == Screen::List {
            // Opened at a circle or bank by id: go there as soon as the session works.
            if (self.resume == Screen::Circle && self.circle.is_some()) || (self.resume == Screen::Bank && self.bank.is_some()) {
                self.screen = self.resume;
            } else if matches!(self.resume, Screen::Circle | Screen::Bank) {
                self.resume = Screen::List;
            }
        }
        if self.screen == Screen::Bank {
            self.auto_bank();
        }
    }

    fn auto_circle(&mut self) {
        let Some(c) = &self.circle else { return };
        let cid = c.id.clone();
        if c.detail.is_none() && c.problem.is_none() && !self.running(JobKind::Detail) {
            self.launch(Work::Detail(cid.clone()));
        }
        let Some(c) = &self.circle else { return };
        if self.screen != Screen::Circle {
            return;
        }
        if let Some(d) = &c.detail {
            let owner = d.circle.role() == Role::Owner;
            if owner && c.invites.is_none() && c.invites_problem.is_none() && !self.running(JobKind::Invites) {
                self.launch(Work::Invites(cid.clone(), None));
            }
            let Some(c) = &self.circle else { return };
            if !c.activity_loaded && c.activity_problem.is_none() && !self.running(JobKind::Activity) {
                self.launch(Work::Activity(cid, None));
            }
        }
    }

    fn auto_bank(&mut self) {
        let Some(b) = &self.bank else { return };
        if matches!(b.load, BankLoad::Loading) && !self.running(JobKind::Bank) {
            let (c, k) = (b.cid.clone(), b.bid.clone());
            self.launch(Work::Bank(c, k));
        }
    }

    // ---- errors ----------------------------------------------------------------------------------------------

    /// Turns an error into a `Problem`, or sends the person to sign in (and returns `None`) when the
    /// sign-in ended. Words that are specific to circles are chosen here.
    fn problem_of(&mut self, e: &PoolError) -> Option<Problem> {
        let now = self.now.unwrap_or_else(Instant::now);
        let mut p = Problem { message: e.message(), retry: false, retry_at: None, account: false };
        match e {
            PoolError::Api(ApiError::SignedOut) => {
                self.gate = Gate::SignedOut;
                if self.screen != Screen::Gate {
                    self.resume = if self.screen == Screen::Bank { Screen::Circle } else { self.screen };
                }
                self.screen = Screen::Gate;
                self.notice = Some(Notice::Info("Your session ended. Sign in again; where you were is kept.".into()));
                return None;
            }
            PoolError::Api(_) => p.retry = true,
            PoolError::Local(_) => {}
            PoolError::Refused(r) => {
                let wait = r.detail.retry_after.filter(|s| *s > 0).map(|s| now + Duration::from_secs(s));
                match &r.kind {
                    RefusalKind::NotApproved { .. } => p.account = true,
                    RefusalKind::InviteRequired => {
                        p.account = true;
                        self.invite_required = true;
                    }
                    RefusalKind::DailyLimit { .. } => {
                        p.retry = true;
                        p.retry_at = wait;
                        if let RefusalKind::DailyLimit { metric: Some(m) } = &r.kind
                            && m == "circle_join_tries"
                        {
                            p.message = "You have tried as many circle codes today as the pool allows. Try again tomorrow, or ask the owner to check the code.".into();
                        }
                    }
                    RefusalKind::RateLimited | RefusalKind::TooManyOpenReads | RefusalKind::TooManyUploads | RefusalKind::Busy => {
                        p.retry = true;
                        p.retry_at = wait;
                    }
                    RefusalKind::NotFound => p.message = "That circle or bank is not available to you any more. It may have been closed, or you may have left it or been removed.".into(),
                    RefusalKind::CircleClosed { undo_until, reason } => {
                        let owner = !matches!(reason.as_deref(), Some("admin" | "last_member_gone"));
                        p.message = match undo_until {
                            Some(u) if owner && *u > self.now_ms() => format!("This circle is closed. The owner can still reopen it {}.", text::until(*u, self.now_ms())),
                            _ => e.message(),
                        }
                    }
                    RefusalKind::ModerationState { message } => {
                        p.message = message.as_deref().map(api::plain).filter(|m| !m.is_empty()).unwrap_or_else(|| "The pool does not allow that right now.".into());
                    }
                    RefusalKind::InvalidRequest { field } => {
                        p.message = match field.as_deref() {
                            Some("name") => "Give the circle a name of 1 to 80 characters.".into(),
                            Some("description") => "The description can be at most 1,000 characters.".into(),
                            Some("code") => "That does not look like a circle code. It starts with C- and has 24 letters and digits.".into(),
                            Some("role") => "Choose whether the person can add banks or can only look and copy.".into(),
                            Some("max_uses") => "Choose how many people the code is for, from 1 to 50.".into(),
                            Some("expires_in_days") => "Choose how long the code works, from 1 to 30 days.".into(),
                            Some("note") => "The note can be at most 200 characters.".into(),
                            Some("user_id") => "The pool did not recognise that member. Reload the circle.".into(),
                            Some(f) => format!("The pool did not accept the {}. Nothing was changed.", f.replace('_', " ")),
                            None => "The pool did not accept that. Nothing was changed.".into(),
                        }
                    }
                    RefusalKind::Other => match (r.detail.status, r.detail.code.as_str()) {
                        (409, "conflict") => p.message = "Something changed at the same moment. Reload the page and try again.".into(),
                        (_, "pool_migrating") => {
                            p.message = "The pool is being upgraded. Try again in a few minutes.".into();
                            p.retry = true;
                            p.retry_at = wait;
                        }
                        _ => {}
                    },
                    _ => {}
                }
            }
        }
        Some(p)
    }

    // ---- jobs ---------------------------------------------------------------------------------------------------

    fn launch(&mut self, work: Work) -> bool {
        let Some(session) = self.session.clone() else {
            self.gate = Gate::Checking;
            return false;
        };
        let kind = work.kind();
        // One change at a time: a second click while one runs would drop the first one's answer.
        if kind == JobKind::Act && self.running(JobKind::Act) {
            return false;
        }
        if let Work::Act(a) = &work {
            self.last_act = Some(a.clone());
        }
        self.jobs.retain(|j| j.kind != kind);
        let (tx, rx) = channel::<JobMsg>();
        let (retry, repaint, images) = (self.retry, self.repaint.clone(), self.images.clone());
        match self.runner {
            #[cfg(not(target_arch = "wasm32"))]
            Runner::Inline => run_work(session, images, retry, work, tx, repaint),
            #[cfg(target_arch = "wasm32")]
            Runner::Inline => wasm_bindgen_futures::spawn_local(async move { run_work_async(session, images, retry, work, tx, repaint).await }),
            #[cfg(not(target_arch = "wasm32"))]
            Runner::Background => {
                std::thread::spawn(move || run_work(session, images, retry, work, tx, repaint));
            }
            #[cfg(target_arch = "wasm32")]
            Runner::Background => wasm_bindgen_futures::spawn_local(async move { run_work_async(session, images, retry, work, tx, repaint).await }),
        }
        self.jobs.push(Job { kind, rx });
        true
    }

    fn drain(&mut self) {
        let mut msgs = Vec::new();
        let mut lost = Vec::new();
        for job in &self.jobs {
            loop {
                match job.rx.try_recv() {
                    Ok(m) => msgs.push(m),
                    Err(TryRecvError::Empty) => break,
                    Err(TryRecvError::Disconnected) => {
                        lost.push(job.kind);
                        break;
                    }
                }
            }
        }
        for m in msgs {
            let kind = m.kind();
            self.jobs.retain(|j| j.kind != kind);
            lost.retain(|k| *k != kind);
            match m {
                JobMsg::List(r) => self.on_list(r),
                JobMsg::Detail(cid, r) => self.on_detail(cid, r),
                JobMsg::Invites(cid, first, r) => self.on_invites(cid, first, r),
                JobMsg::Activity(cid, first, r) => self.on_activity(cid, first, r),
                JobMsg::Bank(cid, bid, r) => self.on_bank(cid, bid, r),
                JobMsg::Copy(key, r) => self.on_copy(*key, r),
                JobMsg::Preview(r, code) => self.on_preview(r, code),
                JobMsg::Join(r, _code) => self.on_join(r),
                JobMsg::Heads(r) => self.on_heads(r),
                JobMsg::Act(a, r) => self.on_act(a, r),
            }
        }
        for kind in lost {
            self.jobs.retain(|j| j.kind != kind);
            let p = Problem { message: "The request stopped unexpectedly.".into(), retry: true, retry_at: None, account: false };
            match kind {
                JobKind::List => self.list.problem = Some(p),
                JobKind::Detail => {
                    if let Some(c) = &mut self.circle {
                        c.problem = Some(p);
                    }
                }
                JobKind::Bank => {
                    if let Some(b) = &mut self.bank {
                        b.load = BankLoad::Failed(p);
                    }
                }
                JobKind::Code => self.join.stage = JoinStage::Failed(p),
                JobKind::Copy => {
                    if let Some(b) = &mut self.bank {
                        b.copy = CopyPanel::Failed(p);
                    }
                }
                _ => {
                    if let Some(c) = &mut self.circle {
                        c.act_problem = Some(p);
                    }
                }
            }
        }
    }

    // ---- results ------------------------------------------------------------------------------------------------------

    fn on_list(&mut self, r: Result<Vec<CircleSummary>, PoolError>) {
        match r {
            Ok(items) => {
                self.list.items = items;
                self.list.loaded = true;
                self.list.problem = None;
            }
            Err(e) => {
                if let Some(p) = self.problem_of(&e) {
                    self.list.problem = Some(p);
                }
            }
        }
    }

    fn is_current(&self, cid: &str) -> bool {
        self.circle.as_ref().is_some_and(|c| c.id == cid)
    }

    fn on_detail(&mut self, cid: String, r: Result<CircleDetail, PoolError>) {
        if !self.is_current(&cid) {
            return;
        }
        match r {
            Ok(d) => {
                if let Some(c) = &mut self.circle {
                    c.detail = Some(d);
                    c.problem = None;
                }
            }
            Err(e) => {
                let gone = matches!(e.kind(), Some(RefusalKind::NotFound));
                if let Some(p) = self.problem_of(&e) {
                    if gone {
                        self.list.items.retain(|i| i.id != cid);
                        self.circle = None;
                        self.bank = None;
                        self.panel = Panel::None;
                        self.screen = Screen::List;
                        self.resume = Screen::List;
                        self.notice = Some(Notice::Info(p.message));
                    } else if let Some(c) = &mut self.circle {
                        c.problem = Some(p);
                    }
                }
            }
        }
    }

    fn on_invites(&mut self, cid: String, first: bool, r: Result<InvitePage, PoolError>) {
        if !self.is_current(&cid) {
            return;
        }
        match r {
            Ok(page) => {
                if let Some(c) = &mut self.circle {
                    if first {
                        c.invites = Some(page.invites);
                    } else {
                        c.invites.get_or_insert_with(Vec::new).extend(page.invites);
                    }
                    c.invites_next = page.next_cursor;
                    c.invites_problem = None;
                }
            }
            Err(e) => {
                if let Some(p) = self.problem_of(&e)
                    && let Some(c) = &mut self.circle
                {
                    c.invites_problem = Some(p);
                }
            }
        }
    }

    fn on_activity(&mut self, cid: String, first: bool, r: Result<ActivityPage, PoolError>) {
        if !self.is_current(&cid) {
            return;
        }
        match r {
            Ok(page) => {
                if let Some(c) = &mut self.circle {
                    if first {
                        c.activity = page.entries;
                    } else {
                        c.activity.extend(page.entries);
                    }
                    c.activity_next = page.next_before;
                    c.activity_loaded = true;
                    c.activity_problem = None;
                }
            }
            Err(e) => {
                if let Some(p) = self.problem_of(&e)
                    && let Some(c) = &mut self.circle
                {
                    c.activity_problem = Some(p);
                }
            }
        }
    }

    fn is_current_bank(&self, cid: &str, bid: &str) -> bool {
        self.bank.as_ref().is_some_and(|b| b.cid == cid && b.bid == bid)
    }

    fn on_bank(&mut self, cid: String, bid: String, r: Result<BankDoc, PoolError>) {
        if !self.is_current_bank(&cid, &bid) {
            return;
        }
        match r {
            Ok(doc) => {
                let view = StudentView::from_payload(&doc.payload, Served::Full).map_err(|e| e.message());
                if let Some(b) = &mut self.bank {
                    b.load = BankLoad::Ready(Box::new(BankLoaded { doc, view }));
                }
            }
            Err(e) => {
                let removed = matches!(e.kind(), Some(RefusalKind::BankRemoved));
                if let Some(mut p) = self.problem_of(&e) {
                    if removed {
                        p.message = "This bank was removed from the circle. If you or the circle's owner removed it, it can be brought back for 14 days from the circle's page.".into();
                    }
                    if let Some(b) = &mut self.bank {
                        b.load = BankLoad::Failed(p);
                    }
                }
            }
        }
    }

    fn on_copy(&mut self, key: (String, String), r: Result<Box<Imported>, PoolError>) {
        if !self.is_current_bank(&key.0, &key.1) {
            return;
        }
        match r {
            Ok(imp) => {
                let circle_name = self.circle.as_ref().and_then(|c| c.detail.as_ref()).map(|d| api::plain(&d.circle.name)).or_else(|| self.list.items.iter().find(|i| i.id == key.0).map(|i| api::plain(&i.name))).unwrap_or_default();
                let wanted = self.copy_name.take().unwrap_or_default();
                self.ready = Some(ReadyCopy { imp, name: wanted, circle: CircleOrigin { circle_id: key.0, circle_name } });
            }
            Err(e) => {
                let name = self.copy_name.take().unwrap_or_default();
                match self.problem_of(&e) {
                    Some(p) => {
                        if let Some(b) = &mut self.bank {
                            b.copy = CopyPanel::Failed(p);
                        }
                    }
                    None => {
                        // The sign-in ended: no copy was made; the person can ask again after signing in.
                        if let Some(b) = &mut self.bank {
                            b.copy = CopyPanel::Choosing { name };
                        }
                        self.notice = Some(Notice::Info("Your session ended, so no copy was made. Sign in again, then press Get a copy again.".into()));
                    }
                }
            }
        }
    }

    fn on_preview(&mut self, r: Result<JoinPreview, PoolError>, code: String) {
        if !matches!(self.join.stage, JoinStage::Looking) {
            return;
        }
        match r {
            Ok(p) => self.join.stage = JoinStage::Preview(Box::new(p), Secret::new(code)),
            Err(e) => {
                if let Some(p) = self.problem_of(&e) {
                    self.join.stage = JoinStage::Failed(p);
                } else {
                    self.join.stage = JoinStage::Typing;
                }
            }
        }
    }

    fn on_join(&mut self, r: Result<Joined, PoolError>) {
        if !matches!(self.join.stage, JoinStage::Joining) {
            return;
        }
        match r {
            Ok(j) => {
                let name = self.joining_name.take().unwrap_or_default();
                self.join = JoinState { text: String::new(), stage: JoinStage::Done { name, role: Role::parse(&j.role), already: j.already, circle_id: j.circle_id } };
                self.list = ListState::default();
            }
            Err(e) => {
                if let Some(p) = self.problem_of(&e) {
                    self.join.stage = JoinStage::Failed(p);
                } else {
                    self.join.stage = JoinStage::Typing;
                }
            }
        }
    }

    fn on_heads(&mut self, r: Result<Vec<HeadEntry>, PoolError>) {
        if let Ok(h) = r {
            self.heads = Some(h);
        } else if let Err(e) = r {
            // A failed quiet check is not shown; the sign-in ending is handled.
            let _ = self.problem_of(&e);
        }
    }

    fn on_act(&mut self, a: Act, r: Result<ActDone, PoolError>) {
        let result = match r {
            Ok(d) => d,
            Err(e) => {
                let gone = matches!(e.kind(), Some(RefusalKind::NotFound));
                let status = match &e {
                    PoolError::Refused(r) => Some(r.detail.status),
                    _ => None,
                };
                let Some(mut p) = self.problem_of(&e) else {
                    // The sign-in ended: nothing happened, and the buttons that were waiting are free again.
                    match &a {
                        Act::Create { .. } => self.create.working = false,
                        Act::Report { .. } => {
                            if let Some(f) = self.bank.as_mut().and_then(|b| b.report.as_mut()) {
                                f.state = ReportState::Editing;
                            }
                        }
                        _ => {}
                    }
                    if let Some(n) = &mut self.notice {
                        *n = Notice::Info("Your session ended, so that was not done. Sign in again, then do it again.".into());
                    }
                    return;
                };
                match &a {
                    Act::Create { .. } => {
                        if matches!(e.kind(), Some(RefusalKind::CirclesLimit { .. })) {
                            p.retry = false;
                        }
                        self.create.working = false;
                        self.create.problem = Some(p);
                    }
                    Act::SetLicense { .. } | Act::RemoveBank { .. } | Act::Report { .. } if self.bank.is_some() => {
                        if let Some(b) = &mut self.bank {
                            if let (Act::Report { .. }, Some(f)) = (&a, b.report.as_mut()) {
                                f.state = ReportState::Failed(p);
                            } else {
                                b.act_problem = Some(p);
                            }
                        }
                    }
                    _ => {
                        // A refusal of a change on the circle page: show it there, and read the page again when
                        // what it showed may be out of date (a role changed, a member left, an offer ended).
                        if gone {
                            // A 404 here is usually the member or code that is gone, not the circle: the page is read
                            // again, and a circle that is really gone shows from that read.
                            p.message = "That is not there any more (it may have just changed). The page was refreshed.".into();
                        }
                        let stale = matches!(status, Some(403 | 404 | 409));
                        if let Some(c) = &mut self.circle {
                            c.act_problem = Some(p);
                            if stale {
                                c.detail = None;
                                c.invites = None;
                                c.activity_loaded = false;
                            }
                        }
                    }
                }
                return;
            }
        };
        let a_copy = a.clone();
        match (a, result) {
            (Act::Create { .. }, ActDone::Created(info)) => {
                self.create = CreateForm::default();
                self.list = ListState::default();
                self.circle = Some(CircleView { id: info.id.clone(), ..CircleView::default() });
                self.panel = Panel::None;
                self.screen = Screen::Circle;
                self.notice = Some(Notice::Info("Your circle is ready. Make a code to invite another teacher.".into()));
            }
            (Act::Rename { name, .. }, _) => {
                self.panel = Panel::None;
                if let Some(c) = &mut self.circle
                    && let Some(d) = &mut c.detail
                {
                    d.circle.name = name;
                }
                self.list = ListState::default();
            }
            (Act::Close(_), ActDone::Closed(undo)) => {
                self.panel = Panel::None;
                self.list = ListState::default();
                let u = undo.unwrap_or(self.now_ms() + 14 * 86_400_000);
                if let Some(c) = &mut self.circle {
                    c.detail = None;
                    c.activity_loaded = false;
                    c.invites = None;
                }
                self.notice = Some(Notice::Info(format!("The circle is closed. You can reopen it {}.", text::until(u, self.now_ms()))));
            }
            (Act::Reopen(_), _) => {
                self.list = ListState::default();
                if let Some(c) = &mut self.circle {
                    c.detail = None;
                    c.activity_loaded = false;
                    c.invites = None;
                }
                self.notice = Some(Notice::Info("The circle is open again.".into()));
            }
            (Act::MakeCode { .. }, ActDone::Code(code)) => {
                self.panel = Panel::Code(code);
                if let Some(c) = &mut self.circle {
                    c.invites = None;
                    c.act_problem = None;
                }
            }
            (Act::Withdraw { .. }, _) => {
                if let Some(c) = &mut self.circle {
                    c.invites = None;
                    c.act_problem = None;
                }
            }
            (Act::Leave(cid), _) => {
                self.list.items.retain(|i| i.id != cid);
                self.list.loaded = false;
                self.circle = None;
                self.bank = None;
                self.panel = Panel::None;
                self.screen = Screen::List;
                self.notice = Some(Notice::Info("You left the circle. Copies you made stay on your computer.".into()));
            }
            (Act::Remove { .. } | Act::SetRole { .. } | Act::Offer { .. } | Act::AcceptOffer(_) | Act::CancelOffer(_), done) => {
                if let (Act::Offer { .. }, _) = (&a_copy, &done) {
                    self.notice = Some(Notice::Info("The offer is sent. They have 7 days to accept; until then you stay the owner.".into()));
                } else if matches!(a_copy, Act::AcceptOffer(_)) {
                    self.notice = Some(Notice::Info("You are now the owner of this circle.".into()));
                }
                self.panel = Panel::None;
                let changed = !matches!(done, ActDone::RoleSet(false));
                let _ = changed;
                if let Some(c) = &mut self.circle {
                    c.detail = None;
                    c.activity_loaded = false;
                    c.invites = None;
                    c.act_problem = None;
                }
                self.list = ListState::default();
            }
            (Act::SetLicense { .. }, _) => {
                if let Some(b) = &mut self.bank {
                    b.act_problem = None;
                    b.license_choice = None;
                    // The circle page and this bank are read again.
                    b.load = BankLoad::Loading;
                }
                if let Some(c) = &mut self.circle {
                    c.detail = None;
                }
            }
            (Act::RemoveBank { .. }, ActDone::BankRemoved(undo)) => {
                self.bank = None;
                self.screen = Screen::Circle;
                if let Some(c) = &mut self.circle {
                    c.detail = None;
                    c.activity_loaded = false;
                }
                let u = undo.unwrap_or(self.now_ms() + 14 * 86_400_000);
                self.notice = Some(Notice::Info(format!("The bank was removed from the circle. It can be brought back {}.", text::until(u, self.now_ms()))));
            }
            (Act::PurgeBank { .. }, _) => {
                if let Some(c) = &mut self.circle {
                    c.detail = None;
                    c.activity_loaded = false;
                    c.act_problem = None;
                }
                self.notice = Some(Notice::Info("The bank was deleted for good.".into()));
            }
            (Act::RestoreBank { .. }, _) => {
                if let Some(c) = &mut self.circle {
                    c.detail = None;
                    c.activity_loaded = false;
                }
                self.notice = Some(Notice::Info("The bank is back in the circle.".into()));
            }
            (Act::Report { .. }, _) => {
                if let Some(f) = self.bank.as_mut().and_then(|b| b.report.as_mut()) {
                    f.state = ReportState::Sent;
                    f.note.clear();
                }
            }
            _ => {}
        }
    }
}

fn new_bank_view(cid: &str, bid: &str) -> BankView {
    BankView { cid: cid.to_string(), bid: bid.to_string(), load: BankLoad::Loading, copy: CopyPanel::Closed, report: None, act_problem: None, confirm_remove: false, license_choice: None }
}

// ---- what the screens ask for ----------------------------------------------------------------------------

impl CirclesModel {
    fn my_role(&self) -> Role {
        self.circle.as_ref().map(|c| c.role()).unwrap_or(Role::Viewer)
    }

    fn is_closed(&self) -> bool {
        self.circle.as_ref().is_some_and(|c| c.closed())
    }

    fn permitted(&self, a: Action) -> bool {
        self.circle.as_ref().is_some_and(|c| c.detail.is_some()) && can(self.my_role(), self.is_closed(), a)
    }

    /// May the person do this in the circle that is open? The buttons ask this.
    pub fn allowed(&self, a: Action) -> bool {
        // An administrator's close (or an empty circle's) can not be undone by the owner.
        if a == Action::Reopen && self.circle.as_ref().and_then(|c| c.detail.as_ref()).is_some_and(|d| !d.circle.closed_by_owner()) {
            return false;
        }
        self.permitted(a)
    }

    /// The person's role in the circle that is open (viewer when unknown).
    pub fn my_role_public(&self) -> Role {
        self.my_role()
    }

    /// The clock the page waits by.
    pub fn now_instant(&self) -> Instant {
        self.now.unwrap_or_else(Instant::now)
    }

    pub fn cid(&self) -> Option<String> {
        self.circle.as_ref().map(|c| c.id.clone())
    }

    fn member(&self, user: &str) -> Option<&Member> {
        self.circle.as_ref().and_then(|c| c.detail.as_ref()).and_then(|d| d.members.iter().find(|m| m.user_id == user))
    }

    // ---- navigation ----------------------------------------------------------------------------------------

    pub fn go_list(&mut self) {
        self.screen = Screen::List;
        self.resume = Screen::List;
        self.circle = None;
        self.bank = None;
        self.panel = Panel::None;
        self.join = JoinState::default();
        self.create = CreateForm::default();
        self.notice = None;
        // Counts and names may have changed: read the list again, quietly.
        if self.session.is_some() {
            self.launch(Work::List);
        } else {
            self.list.loaded = false;
        }
    }

    pub fn open_circle(&mut self, cid: &str) {
        if !super::pool::is_item_id(cid) {
            return;
        }
        self.circle = Some(CircleView { id: cid.to_string(), ..CircleView::default() });
        self.bank = None;
        self.panel = Panel::None;
        self.notice = None;
        self.resume = Screen::Circle;
        self.screen = Screen::Circle;
    }

    pub fn open_bank(&mut self, bid: &str) {
        let Some(cid) = self.cid() else { return };
        if !super::pool::is_item_id(bid) {
            return;
        }
        self.bank = Some(new_bank_view(&cid, bid));
        self.panel = Panel::None;
        self.notice = None;
        self.resume = Screen::Bank;
        self.screen = Screen::Bank;
    }

    /// One step back: bank to circle, circle (or join or create) to the list.
    pub fn back(&mut self) {
        if self.busy_action() {
            return;
        }
        match self.screen {
            Screen::Bank => {
                self.bank = None;
                self.screen = Screen::Circle;
                self.resume = Screen::Circle;
                // The circle page is read again so a change shows.
                if let Some(c) = &mut self.circle {
                    c.detail = None;
                }
            }
            Screen::Circle | Screen::Join | Screen::Create => self.go_list(),
            _ => {}
        }
    }

    /// Reads the open circle again.
    pub fn reload_circle(&mut self) {
        if let Some(c) = &mut self.circle {
            let id = c.id.clone();
            *c = CircleView { id, ..CircleView::default() };
        }
    }

    pub fn retry_list(&mut self) {
        self.list.problem = None;
    }

    pub fn retry_circle(&mut self) {
        if let Some(c) = &mut self.circle {
            c.problem = None;
            c.invites_problem = None;
            c.activity_problem = None;
        }
    }

    pub fn retry_bank(&mut self) {
        if let Some(b) = &mut self.bank {
            b.load = BankLoad::Loading;
        }
    }

    // ---- create ---------------------------------------------------------------------------------------------------

    pub fn start_create(&mut self) {
        self.create = CreateForm::default();
        self.notice = None;
        self.screen = Screen::Create;
    }

    pub fn create_blocker(&self) -> Option<&'static str> {
        let n = self.create.name.trim();
        if n.is_empty() {
            Some("Give the circle a name.")
        } else if n.chars().count() > NAME_MAX {
            Some("The name is too long (80 characters at most).")
        } else if self.create.description.chars().count() > DESCRIPTION_MAX {
            Some("The description is too long (1,000 characters at most).")
        } else {
            None
        }
    }

    pub fn create_circle(&mut self) {
        if self.create.working || self.create_blocker().is_some() {
            return;
        }
        self.create.problem = None;
        let a = Act::Create { name: self.create.name.trim().to_string(), description: self.create.description.trim().to_string() };
        if self.launch(Work::Act(a)) {
            self.create.working = true;
        }
    }

    // ---- joining -----------------------------------------------------------------------------------------------------

    pub fn start_join(&mut self) {
        self.join = JoinState::default();
        self.notice = None;
        self.screen = Screen::Join;
    }

    /// The text in the box holds a code (a pasted link or message counts).
    pub fn join_code(&self) -> Option<String> {
        code_in(&self.join.text)
    }

    /// "Look up": asks what the code is for. Uses nothing.
    pub fn look_up_code(&mut self) {
        let Some(code) = self.join_code() else { return };
        if matches!(self.join.stage, JoinStage::Looking | JoinStage::Joining) {
            return;
        }
        if self.launch(Work::Preview(code)) {
            self.join.stage = JoinStage::Looking;
        }
    }

    /// "Join": uses the code.
    pub fn confirm_join(&mut self) {
        let JoinStage::Preview(p, code) = &self.join.stage else { return };
        let (name, code) = (api::plain(&p.circle.name), code.expose().to_string());
        self.joining_name = Some(name);
        if self.launch(Work::Join(code)) {
            self.join.stage = JoinStage::Joining;
        }
    }

    pub fn change_code(&mut self) {
        self.join.stage = JoinStage::Typing;
    }

    /// After joining: open the circle.
    pub fn open_joined(&mut self) {
        let JoinStage::Done { circle_id, .. } = &self.join.stage else { return };
        let id = circle_id.clone();
        self.join = JoinState::default();
        self.open_circle(&id);
    }

    // ---- the circle page -----------------------------------------------------------------------------------

    pub fn cancel_panel(&mut self) {
        if self.running(JobKind::Act) {
            return;
        }
        self.panel = Panel::None;
        if let Some(c) = &mut self.circle {
            c.act_problem = None;
        }
    }

    pub fn begin_rename(&mut self) {
        if !self.permitted(Action::Rename) {
            return;
        }
        let name = self.circle.as_ref().and_then(|c| c.detail.as_ref()).map(|d| api::plain(&d.circle.name)).unwrap_or_default();
        self.panel = Panel::Rename { name };
    }

    pub fn rename(&mut self) {
        let Panel::Rename { name } = &self.panel else { return };
        let name = name.trim().to_string();
        let Some(cid) = self.cid() else { return };
        if !self.permitted(Action::Rename) || name.is_empty() || name.chars().count() > NAME_MAX {
            return;
        }
        self.launch(Work::Act(Act::Rename { cid, name }));
    }

    pub fn begin_code(&mut self) {
        if self.permitted(Action::MakeCode) {
            self.panel = Panel::NewCode(CodeForm::default());
        }
    }

    pub fn make_code(&mut self) {
        let Panel::NewCode(f) = &self.panel else { return };
        let Some(cid) = self.cid() else { return };
        if !self.permitted(Action::MakeCode) || f.role == Role::Owner || !(1..=50).contains(&f.people) || !(1..=30).contains(&f.days) {
            return;
        }
        let a = Act::MakeCode { cid, role: f.role, people: f.people, days: f.days, note: f.note.trim().to_string() };
        self.launch(Work::Act(a));
    }

    /// The code was copied or the person is done with it: it is wiped.
    pub fn done_with_code(&mut self) {
        if matches!(self.panel, Panel::Code(_)) {
            self.panel = Panel::None;
        }
    }

    pub fn withdraw_code(&mut self, invite_id: &str) {
        let Some(cid) = self.cid() else { return };
        if !self.permitted(Action::WithdrawCode) {
            return;
        }
        let ok = self.circle.as_ref().and_then(|c| c.invites.as_ref()).is_some_and(|l| l.iter().any(|i| i.invite_id == invite_id && i.active()));
        if ok {
            self.launch(Work::Act(Act::Withdraw { cid, invite: invite_id.to_string() }));
        }
    }

    pub fn more_invites(&mut self) {
        let Some(c) = &self.circle else { return };
        if let Some(next) = c.invites_next.clone() {
            let cid = c.id.clone();
            self.launch(Work::Invites(cid, Some(next)));
        }
    }

    pub fn more_activity(&mut self) {
        let Some(c) = &self.circle else { return };
        if let Some(next) = c.activity_next.clone() {
            let cid = c.id.clone();
            self.launch(Work::Activity(cid, Some(next)));
        }
    }

    pub fn ask_leave(&mut self) {
        if self.permitted(Action::Leave) {
            self.panel = Panel::ConfirmLeave;
        }
    }

    pub fn leave(&mut self) {
        let Some(cid) = self.cid() else { return };
        if matches!(self.panel, Panel::ConfirmLeave) && self.permitted(Action::Leave) {
            self.launch(Work::Act(Act::Leave(cid)));
        }
    }

    pub fn ask_close(&mut self) {
        if self.permitted(Action::Close) {
            self.panel = Panel::ConfirmClose;
        }
    }

    pub fn close_the_circle(&mut self) {
        let Some(cid) = self.cid() else { return };
        if matches!(self.panel, Panel::ConfirmClose) && self.permitted(Action::Close) {
            self.launch(Work::Act(Act::Close(cid)));
        }
    }

    pub fn reopen(&mut self) {
        let Some(cid) = self.cid() else { return };
        if self.allowed(Action::Reopen) {
            self.launch(Work::Act(Act::Reopen(cid)));
        }
    }

    pub fn ask_remove(&mut self, user: &str) {
        if self.permitted(Action::RemoveMember) && self.member(user).is_some_and(|m| !m.is_me && m.role() != Role::Owner) {
            self.panel = Panel::ConfirmRemove(user.to_string());
        }
    }

    pub fn remove_member(&mut self) {
        let Panel::ConfirmRemove(user) = &self.panel else { return };
        let user = user.clone();
        let Some(cid) = self.cid() else { return };
        if self.permitted(Action::RemoveMember) && self.member(&user).is_some_and(|m| !m.is_me && m.role() != Role::Owner) {
            self.launch(Work::Act(Act::Remove { cid, user }));
        }
    }

    pub fn set_role(&mut self, user: &str, role: Role) {
        let Some(cid) = self.cid() else { return };
        if role == Role::Owner || !self.permitted(Action::ChangeRole) {
            return;
        }
        if self.member(user).is_some_and(|m| !m.is_me && m.role() != Role::Owner && m.role() != role) {
            self.launch(Work::Act(Act::SetRole { cid, user: user.to_string(), role }));
        }
    }

    pub fn ask_transfer(&mut self, user: &str) {
        if self.permitted(Action::Transfer) && self.member(user).is_some_and(|m| !m.is_me && m.role() != Role::Owner) {
            self.panel = Panel::ConfirmTransfer(user.to_string());
        }
    }

    pub fn offer_ownership(&mut self) {
        let Panel::ConfirmTransfer(user) = &self.panel else { return };
        let user = user.clone();
        let Some(cid) = self.cid() else { return };
        if self.permitted(Action::Transfer) && self.member(&user).is_some_and(|m| !m.is_me && m.role() != Role::Owner) {
            self.launch(Work::Act(Act::Offer { cid, user }));
        }
    }

    /// The ownership offer on the open circle, if the pool showed one (to the owner and the member offered).
    pub fn pending_offer(&self) -> Option<&PendingTransfer> {
        self.circle.as_ref()?.detail.as_ref()?.pending_transfer.as_ref()
    }

    /// The offer is for me: I can accept or decline it.
    pub fn offered_to_me(&self) -> bool {
        let Some(p) = self.pending_offer() else { return false };
        self.member(&p.to_user_id).is_some_and(|m| m.is_me) && self.my_role() != Role::Owner && !self.is_closed()
    }

    /// The owner can take their offer back.
    pub fn can_withdraw_offer(&self) -> bool {
        self.pending_offer().is_some() && self.my_role() == Role::Owner && !self.is_closed()
    }

    pub fn accept_offer(&mut self) {
        let Some(cid) = self.cid() else { return };
        if self.offered_to_me() {
            self.launch(Work::Act(Act::AcceptOffer(cid)));
        }
    }

    /// Declines (the member) or withdraws (the owner) the pending offer.
    pub fn cancel_offer(&mut self) {
        let Some(cid) = self.cid() else { return };
        if self.offered_to_me() || self.can_withdraw_offer() {
            self.launch(Work::Act(Act::CancelOffer(cid)));
        }
    }

    /// A member's display name for a confirmation, cleaned.
    pub fn member_name(&self, user: &str) -> String {
        self.member(user).map(|m| deleted_teacher(&m.display_name)).unwrap_or_else(|| "this member".into())
    }

    // ---- one bank -----------------------------------------------------------------------------------------------

    fn bank_entry(&self) -> Option<&BankEntry> {
        let b = self.bank.as_ref()?;
        self.circle.as_ref()?.detail.as_ref()?.banks.iter().find(|e| e.id == b.bid)
    }

    /// May the person do this with the bank that is open? When the circle page has not been read the
    /// answer comes from the bank itself.
    pub fn bank_allowed(&self, a: BankAction) -> bool {
        if self.is_closed() {
            return matches!(a, BankAction::Look);
        }
        match (self.bank_entry(), self.bank.as_ref()) {
            (Some(e), _) => bank_can(self.my_role(), false, e, a),
            (None, Some(b)) => {
                let (is_author, role) = match &b.load {
                    BankLoad::Ready(l) => (l.doc.bank.is_author, self.my_role()),
                    _ => (false, self.my_role()),
                };
                let stub = BankEntry { is_author, ..BankEntry::default() };
                matches!(b.load, BankLoad::Ready(_)) && bank_can(role, false, &stub, a)
            }
            _ => false,
        }
    }

    pub fn begin_copy(&mut self) {
        if !self.bank_allowed(BankAction::Copy) {
            return;
        }
        if let Some(b) = &mut self.bank
            && let BankLoad::Ready(l) = &b.load
        {
            let name = api::plain(&l.doc.bank.title);
            b.copy = CopyPanel::Choosing { name };
        }
    }

    pub fn cancel_copy(&mut self) {
        if let Some(b) = &mut self.bank
            && !matches!(b.copy, CopyPanel::Working)
        {
            b.copy = CopyPanel::Closed;
        }
    }

    /// "Get a copy": fetches the pictures and makes the bank on this device.
    pub fn get_copy(&mut self) {
        if !self.bank_allowed(BankAction::Copy) {
            return;
        }
        let Some(b) = &mut self.bank else { return };
        let (CopyPanel::Choosing { name }, BankLoad::Ready(l)) = (&b.copy, &b.load) else { return };
        let name = name.trim().to_string();
        let doc = Box::new(l.doc.clone());
        self.copy_name = Some(name.clone());
        if self.launch(Work::Copy(doc, name)) && let Some(b) = &mut self.bank {
            b.copy = CopyPanel::Working;
        }
    }

    pub fn begin_report(&mut self) {
        if self.bank_allowed(BankAction::Report)
            && let Some(b) = &mut self.bank
        {
            b.report = Some(ReportForm { reason: ReportReasonChoice::Other, note: String::new(), state: ReportState::Editing });
        }
    }

    pub fn cancel_report(&mut self) {
        if let Some(b) = &mut self.bank {
            b.report = None;
        }
    }

    pub fn send_report(&mut self) {
        if !self.bank_allowed(BankAction::Report) {
            return;
        }
        let Some(b) = &mut self.bank else { return };
        let Some(f) = &mut b.report else { return };
        if matches!(f.state, ReportState::Sending | ReportState::Sent) {
            return;
        }
        let a = Act::Report { cid: b.cid.clone(), bid: b.bid.clone(), reason: f.reason, note: f.note.trim().to_string() };
        if self.running(JobKind::Act) {
            return;
        }
        if let Some(f) = self.bank.as_mut().and_then(|b| b.report.as_mut()) {
            f.state = ReportState::Sending;
        }
        self.launch(Work::Act(a));
    }

    /// The bank author sets the re-share setting.
    pub fn set_bank_license(&mut self, l: License) {
        if !self.bank_allowed(BankAction::SetLicense) {
            return;
        }
        let Some(b) = &mut self.bank else { return };
        let a = Act::SetLicense { cid: b.cid.clone(), bid: b.bid.clone(), license: l };
        b.license_choice = Some(l);
        self.launch(Work::Act(a));
    }

    pub fn ask_remove_bank(&mut self) {
        if self.bank_allowed(BankAction::Remove)
            && let Some(b) = &mut self.bank
        {
            b.confirm_remove = true;
        }
    }

    pub fn cancel_remove_bank(&mut self) {
        if let Some(b) = &mut self.bank {
            b.confirm_remove = false;
        }
    }

    pub fn remove_bank(&mut self) {
        if !self.bank_allowed(BankAction::Remove) {
            return;
        }
        let Some(b) = &mut self.bank else { return };
        if !b.confirm_remove {
            return;
        }
        let a = Act::RemoveBank { cid: b.cid.clone(), bid: b.bid.clone() };
        self.launch(Work::Act(a));
    }

    /// Brings back a removed bank from the circle's page.
    pub fn restore_bank(&mut self, bid: &str) {
        let Some(cid) = self.cid() else { return };
        let now = self.now_ms();
        let ok = self.circle.as_ref().and_then(|c| c.detail.as_ref()).is_some_and(|d| d.banks.iter().any(|e| e.id == bid && bank_can(d.circle.role(), d.circle.closed(), e, BankAction::Restore) && e.removed.as_ref().and_then(|r| r.undo_until).is_some_and(|u| u > now)));
        if ok {
            self.launch(Work::Act(Act::RestoreBank { cid, bid: bid.to_string() }));
        }
    }

    /// The owner deletes a removed bank for good, now (the page asks first).
    pub fn purge_bank(&mut self, bid: &str) {
        let Some(cid) = self.cid() else { return };
        let ok = self.circle.as_ref().and_then(|c| c.detail.as_ref()).is_some_and(|d| d.banks.iter().any(|e| e.id == bid && bank_can(d.circle.role(), d.circle.closed(), e, BankAction::Purge)));
        if ok {
            if let Some(c) = &mut self.circle {
                c.confirm_purge = None;
            }
            self.launch(Work::Act(Act::PurgeBank { cid, bid: bid.to_string() }));
        }
    }

    pub fn ask_purge(&mut self, bid: &str) {
        if let Some(c) = &mut self.circle {
            c.confirm_purge = Some(bid.to_string());
        }
    }

    pub fn cancel_purge(&mut self) {
        if let Some(c) = &mut self.circle {
            c.confirm_purge = None;
        }
    }

    /// "Try again" on a failed change: the same change once more.
    pub fn retry_act(&mut self) {
        let Some(a) = self.last_act.clone() else { return };
        if let Some(c) = &mut self.circle {
            c.act_problem = None;
        }
        self.launch(Work::Act(a));
    }

    // ---- copies and update badges -------------------------------------------------------------------------

    /// Puts a finished copy into the banks and records it like an import. Returns true when `banks`
    /// changed (the caller then saves them). Does nothing when nothing is waiting.
    pub fn sync(&mut self, banks: &mut Vec<QuestionBank>, browse: &mut super::browse::BrowseModel) -> bool {
        let Some(r) = self.ready.take() else { return false };
        let name = if r.name.trim().is_empty() { api::plain(&r.imp.head.item.title) } else { r.name.clone() };
        let applied: Result<Applied, String> = apply_import(banks, &Dest::New, &name, &r.imp);
        match applied {
            Ok(a) => {
                // The credit line says where it came from: the circle, not the public pool.
                if let Some(b) = banks.iter_mut().find(|b| b.id == a.bank_id) {
                    let it = &r.imp.head.item;
                    b.details.description = format!("From the circle \"{}\": \"{}\" by {}, version {}.", r.circle.circle_name, api::plain(&it.title), super::browse_text::author(&it.author_name), it.version);
                }
                let saved = browse.record_circle_copy(&r.imp, &a, r.circle);
                if let Some(b) = &mut self.bank {
                    b.copy = CopyPanel::Done(CopyDone { bank_name: a.bank_name.clone(), added: a.added, pictures: r.imp.pictures, pictures_missing: r.imp.pictures_missing, version: r.imp.head.item.version });
                }
                if let Err(m) = saved {
                    self.notice = Some(Notice::Error(format!("The questions were added, but the list of copies could not be saved ({m}). Updates will not be noticed for this one.")));
                }
                true
            }
            Err(m) => {
                if let Some(b) = &mut self.bank {
                    b.copy = CopyPanel::Failed(Problem { message: m, retry: false, retry_at: None, account: false });
                }
                false
            }
        }
    }

    /// The shared page tells the model which circle banks were copied (once per opening); the
    /// model asks the pool where they stand.
    pub fn want_heads(&mut self, ids: Vec<String>) {
        if !self.heads_done && self.heads_wanted.is_none() {
            self.heads_wanted = Some(ids);
        }
    }

    /// The answer of the quiet check, once.
    pub fn take_heads(&mut self) -> Option<Vec<HeadEntry>> {
        self.heads.take()
    }
}

/// "Deleted teacher" stays "deleted teacher"; a name is cleaned.
pub fn deleted_teacher(name: &str) -> String {
    let n = api::plain(name);
    if n.is_empty() { "A teacher".into() } else { n }
}
