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

use web_time::Instant;

use super::api::{self, ApiError};
use super::browse::{
    CircleOrigin, Dest, ImageSink, Imported, Problem, SharedImages, StoreImages, apply_import, now_ms,
};
use super::circle_edit::{CircleEditModel, Perm};
use super::circle_hide::{Decisions, Pictures, decision_refusal, left_out_unasked};
use super::circle_links::{CircleLink, LinkLog};
use super::circles_api::*;
use super::circles_text as text;
use super::model::{Notice, Runner};
use super::pool::{ImportInfo, ItemHead, ItemView, License, Pool, PoolError, RefusalKind, RetryPolicy, Served, StoreMedia, StudentView, is_sha256};
use super::publish::{AccountSnapshot, Gate, SharedSession};
use super::browse::StoreRestore;
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
    /// Hide the answer keys from the circle's viewers, or show them again (phase C, D12): the bank's author
    /// while an editor, or the circle's owner.
    HideKeys,
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
        BankAction::HideKeys => !removed && !closed && (circle_role == Role::Owner || (b.is_author && circle_role == Role::Editor)),
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

/// "Invite by email" (phase C, D5). The address lives only in this form: it is sent once, wiped as soon as
/// the pool took it and when the panel closes, never written anywhere, never in `Debug`.
pub struct EmailForm {
    pub address: String,
    pub role: Role,
    pub working: bool,
    pub problem: Option<Problem>,
    /// The mail domain (never the address) a per-domain hourly limit in `problem` is for: its wait holds
    /// back only addresses at that domain (critic 57 I4).
    pub problem_domain: Option<String>,
    /// What the last invitation did: the pool's masked label, never the address.
    pub sent: Option<EmailInviteSent>,
}

impl Default for EmailForm {
    fn default() -> Self {
        EmailForm { address: String::new(), role: Role::Editor, working: false, problem: None, problem_domain: None, sent: None }
    }
}

/// The mail domain of an address, in lower case ("pvhs.org").
fn mail_domain(address: &str) -> Option<String> {
    address.trim().rsplit_once('@').map(|(_, d)| d.trim_end_matches('.').to_ascii_lowercase()).filter(|d| !d.is_empty())
}

impl std::fmt::Debug for EmailForm {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "EmailForm(role: {:?}, working: {}, address: <{} characters>)", self.role, self.working, self.address.chars().count())
    }
}

impl Drop for EmailForm {
    fn drop(&mut self) {
        zeroize::Zeroize::zeroize(&mut self.address);
    }
}

#[derive(Debug, Default)]
pub enum Panel {
    #[default]
    None,
    NewCode(CodeForm),
    /// Invite a teacher by email.
    Email(EmailForm),
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

pub struct JoinState {
    /// What was pasted (a code, a link from an invitation mail, a message): never in `Debug`.
    pub text: String,
    pub stage: JoinStage,
}

impl std::fmt::Debug for JoinState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "JoinState(<{} characters>, {:?})", self.text.chars().count(), self.stage)
    }
}

impl Drop for JoinState {
    fn drop(&mut self) {
        zeroize::Zeroize::zeroize(&mut self.text);
    }
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

/// "Hide answer keys from viewers" on a bank's page (the author while an editor, or the owner).
#[derive(Debug, Default)]
pub enum HideStage {
    #[default]
    Idle,
    /// The person pressed the switch: what it means, then Yes.
    Confirm { on: bool },
    /// The change is on its way. `back` is the decisions page it came from (kept when the sign-in ends).
    Working { on: bool, back: Option<Box<Decisions>> },
    /// The pool listed what it needs decided for the viewers' copy (or what has to be changed first).
    Decide(Box<Decisions>),
    /// Before hiding is turned on, when the newest version has pictures: which ones viewers keep. Nothing was
    /// sent yet.
    Pictures(Box<Pictures>),
    Done(HideKeyResult),
    /// A refusal or an error: said, and "Try again" asks once more (`on` is what was asked for). `back` is
    /// the decisions page the change came from: kept unless the bank changed, and sent again by "Try again".
    Failed { on: bool, problem: Problem, back: Option<Box<Decisions>> },
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
    pub hide: HideStage,
    /// The last read of the bank failed because its newest version is held back for review (`version_withheld`).
    pub withheld_head: bool,
    /// That refusal said turning hiding off would release it (`hide_off_releases`, critic 59 B1).
    pub hide_off_releases: bool,
    /// The picture step's choices for the change on its way (what was kept and dropped, for which version):
    /// sent again by "Try again", and compared with what the pool left out.
    pub picked: Option<Box<Pictures>>,
    /// The picture step (and what follows it) is "Change picture choices" on a bank that already hides its keys
    /// (critic 62 I2), not turning hiding on.
    pub pictures_change: bool,
    /// Owner decision (a): a read-only Preview of the bank's questions, not its settings.
    pub preview: bool,
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
    /// The address is a [`Secret`]: never in `Debug`, wiped when the act is dropped, never kept for "Try again".
    EmailInvite { cid: String, email: Secret, role: Role },
    /// `base`: the bank version the change (and its decisions) is for. The head is read right before the
    /// PATCH, which names no version, and nothing is sent when the head is another one (critic 57 B1).
    HideKey { cid: String, bid: String, on: bool, decisions: StripDecisions, base: Option<u64> },
}

enum ActDone {
    Created(Box<CircleInfo>),
    Done,
    Closed(Option<i64>),
    Code(Box<InviteCreated>),
    RoleSet(bool),
    BankRemoved(Option<i64>),
    Emailed(Box<EmailInviteSent>),
    Hidden(Box<HideKeyResult>),
    /// The head was not the version the change was for: nothing was sent.
    HideHeadMoved,
    Purged(Purged),
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
    /// "Add to My Banks" / "Make my own copy" from the circle page (phase D1): the bank read, then copied.
    Add(String, String),
    /// The banks of a circle just joined, for the offer to add them (phase D1).
    Offer(String),
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
    Offer,
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
            // (one copy at a time, as on the bank's page)
            Work::Add(..) => JobKind::Copy,
            Work::Offer(_) => JobKind::Offer,
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
    Added(Box<(String, String)>, Result<Box<(BankHead, Imported)>, PoolError>),
    Offer(String, Result<CircleDetail, PoolError>),
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
            JobMsg::Added(..) => JobKind::Copy,
            JobMsg::Offer(..) => JobKind::Offer,
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
        Act::PurgeBank { cid, bid } => ActDone::Purged(pool.purge_circle_bank(cid, bid).await?),
        Act::EmailInvite { cid, email, role } => ActDone::Emailed(Box::new(pool.email_invite(cid, email, *role).await?)),
        Act::HideKey { cid, bid, on, decisions, base } => {
            // (critic 57 B1) The decisions are by position: the head is read right before, and a moved one sends
            // nothing. (critic 62 B1) The PATCH is pinned to that version too (`base_version`): one that arrives
            // after this read is the pool's `409 conflict`, and nothing changes.
            if let Some(base) = base {
                let heads = pool.circle_heads(std::slice::from_ref(bid)).await?;
                let same = heads.iter().any(|h| h.bank_id == *bid && matches!(h.state, HeadState::Current { version, .. } if version == *base));
                if !same {
                    return Ok(ActDone::HideHeadMoved);
                }
            }
            ActDone::Hidden(Box::new(pool.set_hide_key(cid, bid, *on, decisions, *base).await?))
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
        Work::Add(cid, bid) => {
            let r = match pool.circle_bank(&cid, &bid).await {
                Ok(doc) => copy_bank(&pool, &*images, &doc).await.map(|imp| Box::new((doc.bank, imp))),
                Err(e) => Err(e),
            };
            JobMsg::Added(Box::new((cid, bid)), r)
        }
        Work::Offer(cid) => {
            let r = pool.circle(&cid).await;
            JobMsg::Offer(cid, r)
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
pub(super) async fn copy_bank(pool: &Pool<'_>, images: &(dyn ImageSink + Send + Sync), doc: &BankDoc) -> Result<Imported, PoolError> {
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
    // (critic 57) the viewers' form marks every question as keyless, flag or not
    let mut theirs = super::circle_links::circle_theirs_as(&doc.payload, at_ms, b.keys_hidden);
    if theirs.is_empty() {
        return Err(PoolError::Local(super::pool::LocalProblem::BadItemShape));
    }
    // A question this build can not read would be dropped from the copy, and then from the circle on the next send.
    if super::circle_links::payload_question_count(&doc.payload) > theirs.len() {
        return Err(PoolError::Local(super::pool::LocalProblem::NewerQuestions));
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
    // A viewer's copy of a bank that hides its keys (phase C): its questions arrive without keys and are marked so.
    let keyless = b.keys_hidden || questions.iter().any(crate::model::is_keyless);
    Ok(Imported { head, info, questions, fingerprints, pictures, pictures_missing: missing, keyless, at_ms, theirs })
}

// ---- the model ----------------------------------------------------------------------------------------------------------

/// A finished copy waiting to be put into the banks by [`CirclesModel::sync`].
struct ReadyCopy {
    imp: Box<Imported>,
    name: String,
    circle: CircleOrigin,
    /// A working copy: the head it was made from (the link is made from it).
    working: Option<BankHead>,
    /// Made by "Add to My Banks" or "Make my own copy" (phase D1).
    add: Option<AddFor>,
}

/// The join offer while this computer's list of shared banks can not be read (critic 65 B1).
pub const LINKS_UNREADABLE_JOIN: &str = "The list of shared banks on this computer couldn't be read just now, so the circle's banks can't be added yet. Nothing was changed.";

/// "Add a full copy" whose read came without the answer keys (critic 65 I3).
pub const FULL_COPY_KEYLESS: &str = "The circle sent this bank without its answer keys (you can only view it now), so no full copy was made. Nothing was changed.";

/// Who asked for an added bank, and how it is linked (phase D1).
#[derive(Clone, Debug, PartialEq)]
struct AddFor {
    cid: String,
    bid: String,
    own: bool,
    role: Role,
    /// "Add a full copy": the new bank takes the circle bank's link from the copy without keys (or the viewer's).
    full: bool,
}

// ---- adding banks to My Banks (phase D1) ------------------------------------------------------------------------------

/// Where one "Add to My Banks" or "Make my own copy" stands.
#[derive(Clone, Debug, PartialEq)]
pub enum AddState {
    /// Waiting for the copy before it to finish.
    Waiting,
    Working,
    /// In My Banks under this name (and, for an added bank, linked); `why` it is not linked when it should be.
    Done { bank_id: String, bank_name: String, linked: bool, why: Option<String> },
    Failed(Problem),
}

#[derive(Clone, Debug, PartialEq)]
pub struct AddJob {
    pub cid: String,
    pub bid: String,
    /// "Make my own copy": not linked, with a note of where it came from.
    pub own: bool,
    pub title: String,
    pub role: Role,
    pub state: AddState,
    /// "Add a full copy" (see [`CirclesModel::add_full_copy`]).
    pub full: bool,
    /// The person dismissed its line on the Banks list (critic 66 I4).
    pub dismissed: bool,
    /// (critic 69 minor 1) The account that asked for it (see `bank_links::account_tag`): it is made for that account
    /// only, and "Try again" by another account does nothing.
    pub account: Option<String>,
}

/// After joining: "Add its banks to My Banks?" with a tick per bank.
#[derive(Clone, Debug, PartialEq)]
pub struct JoinOffer {
    pub cid: String,
    pub name: String,
    pub role: Role,
    /// `None` while the circle's banks are read.
    pub banks: Option<Vec<OfferBank>>,
    pub problem: Option<Problem>,
    /// "Add the ticked banks" was pressed.
    pub started: bool,
    /// The banks are read from the frame after the join's answer, with the list of circles.
    pub wait: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub struct OfferBank {
    pub id: String,
    pub title: String,
    pub ticked: bool,
    /// This computer already has it in My Banks, under this name: it is not ticked and not added again (critic 63 B3).
    pub in_banks: Option<String>,
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
    /// Working copies: send my changes, get the latest and merge, the history.
    pub edit: CircleEditModel,
    /// The pool answered `invite_required` (the Shared page then asks for the pool's invite code).
    pub invite_required: bool,
    /// Removed banks whose "Delete for good now" was only partly done (round 48): versions an
    /// administrator is still reviewing are kept, the rest is gone and nothing can be brought back. The
    /// pool's circle page does not say so, so it is remembered here (bank id -> versions kept).
    pub purge_claimed: std::collections::HashMap<String, u64>,
    /// Phase D1: the checks of linked banks for the badge and the strip in My Banks.
    pub links: super::bank_links::BankLinks,
    /// Phase D1: "Add to My Banks" and "Make my own copy", one copy at a time.
    pub adds: Vec<AddJob>,
    /// Phase D1: the offer to add a just-joined circle's banks.
    pub join_offer: Option<JoinOffer>,
    /// A bank's row to bring into view on the circle page (a link to a circle and one of its banks).
    pub scroll_to: Option<String>,
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
    copy_seed: Option<BankHead>,
    joining_name: Option<String>,
    clock: Option<i64>,
    now: Option<Instant>,
    repaint: Option<Arc<dyn Fn() + Send + Sync>>,
    /// The account what is loaded here was read for (critic 60 I1), and whether it has signed out since.
    account: Option<String>,
    account_out: bool,
    /// (critic 67 I1) An add was answered "signed out" while the account still looks signed in: no add starts until
    /// a sign-out is seen, the Circles tab opens again, or the person presses "Sign in..." on the Banks list.
    adds_out: bool,
    /// "Sign in..." was pressed on the Banks list: the page there opens the Account window.
    adds_sign_in: bool,
    /// "Open it" was pressed on the Banks list: the page there opens this bank.
    open_bank_asked: Option<String>,
    /// (critic 69 minor 3) Where the number of adds not finished is written, for the next start.
    adds_log: AddsLog,
    /// What that file says now (`None`: not read yet this start).
    unfinished_noted: Option<usize>,
    /// The adds the last run did not finish (it was closed first), said on the Banks list until dismissed.
    unfinished_at_start: usize,
}

/// (critic 69 minor 3) Adds live in memory only: a closed app drops those not finished. Resuming them would need the
/// account they were asked for, which is not known at start; so only how many were not finished is written down, and
/// the next start says so.
pub const ADDS_DOC: &str = "share-circle-adds.json";

/// Where [`ADDS_DOC`] is kept: the app's store, or memory (tests never touch the app's real data folder).
#[derive(Clone, Debug)]
pub enum AddsLog {
    Store,
    Memory(Arc<std::sync::Mutex<Option<usize>>>),
}

impl AddsLog {
    pub fn memory() -> AddsLog {
        AddsLog::Memory(Arc::default())
    }

    fn read(&self) -> usize {
        match self {
            AddsLog::Store => crate::store::with_backend(|b| b.read_doc(ADDS_DOC)).and_then(|v| serde_json::from_slice::<serde_json::Value>(&v).ok()).and_then(|v| v.get("unfinished").and_then(serde_json::Value::as_u64)).unwrap_or(0) as usize,
            AddsLog::Memory(m) => m.lock().unwrap_or_else(|p| p.into_inner()).unwrap_or(0),
        }
    }

    fn write(&self, n: usize) {
        match self {
            AddsLog::Store => {
                let _ = crate::store::with_backend(|b| b.write_doc(ADDS_DOC, serde_json::json!({"unfinished": n}).to_string().as_bytes()));
            }
            AddsLog::Memory(m) => *m.lock().unwrap_or_else(|p| p.into_inner()) = Some(n),
        }
    }
}

/// (critic 67 I2) An add that was waiting or under way when another account signed in: it is not made for that one.
pub const ADD_OTHER_ACCOUNT: &str = "Another account signed in before it was finished. Sign in with the account that asked for it, then add it again from the circle's page.";

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
            edit: CircleEditModel::new(runner, images.clone(), Arc::new(StoreMedia), Arc::new(StoreRestore), LinkLog::Store),
            invite_required: false,
            purge_claimed: Default::default(),
            links: super::bank_links::BankLinks::new(runner),
            adds: Vec::new(),
            join_offer: None,
            scroll_to: None,
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
            copy_seed: None,
            joining_name: None,
            clock: None,
            now: None,
            repaint: None,
            account: None,
            account_out: false,
            adds_out: false,
            adds_sign_in: false,
            open_bank_asked: None,
            adds_log: if cfg!(test) { AddsLog::memory() } else { AddsLog::Store },
            unfinished_noted: None,
            unfinished_at_start: 0,
        }
    }

    pub fn with_retry(mut self, retry: RetryPolicy) -> CirclesModel {
        self.retry = retry;
        self.edit.set_retry(retry);
        self.links.set_retry(retry);
        self
    }

    pub fn set_repaint(&mut self, f: Arc<dyn Fn() + Send + Sync>) {
        self.edit.set_repaint(f.clone());
        self.links.set_repaint(f.clone());
        self.repaint = Some(f);
    }

    /// A repaint was set (the Shared page or a linked bank's page sets it once).
    pub fn has_repaint(&self) -> bool {
        self.repaint.is_some()
    }

    pub fn set_clock(&mut self, ms: i64) {
        self.clock = Some(ms);
    }

    pub fn now_ms(&self) -> i64 {
        self.clock.unwrap_or_else(now_ms)
    }

    pub fn busy(&self) -> bool {
        !self.jobs.is_empty() || self.edit.busy()
    }

    /// A change is on its way (the page should not be left in the middle of it).
    pub fn busy_action(&self) -> bool {
        self.running(JobKind::Act) || self.running(JobKind::Copy)
    }

    fn running(&self, k: JobKind) -> bool {
        self.jobs.iter().any(|j| j.kind == k)
    }

    // ---- opening --------------------------------------------------------------------------------------------

    /// An add whose job was just dropped starts again; its copy is made once (a copy already handed to the banks
    /// lands on this add, see `finish_landing`).
    fn requeue_adds(&mut self) {
        for a in &mut self.adds {
            if a.state == AddState::Working {
                a.state = AddState::Waiting;
            }
        }
    }

    fn reset(&mut self, screen: Screen) {
        self.cancel_all();
        // (critic 65 minor) a list of shared banks that could not be read is read again when the page opens
        self.edit.read_links_again();
        self.requeue_adds();
        // (critic 67 I1) opening the tab tries an add stopped by a signed-out answer once more
        self.adds_out = false;
        self.edit.close();
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

    /// Opens one circle at one of its banks (phase D1): the circle page, with the bank's row brought into view.
    /// `<bank id>:settings` opens the bank's settings page instead (owner and author; QA rigs).
    pub fn open_bank_at(&mut self, cid: &str, bid: &str) {
        if let Some(b) = bid.strip_suffix(":settings") {
            return self.open_bank_settings_at(cid, b);
        }
        self.open_circle_at(cid);
        if self.circle.is_some() && super::pool::is_item_id(bid) {
            self.scroll_to = Some(bid.to_string());
        }
    }

    /// Opens one circle's bank settings page.
    pub fn open_bank_settings_at(&mut self, cid: &str, bid: &str) {
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
        // (critic 67 M1) an add whose read was dropped here is read again, not left "under way" for ever
        self.requeue_adds();
        self.edit.close();
        self.wipe_secrets();
        self.ready = None;
    }

    fn cancel_all(&mut self) {
        self.jobs.clear();
    }

    /// Nothing typed or shown once stays behind (a code, a pasted link, an address).
    fn wipe_secrets(&mut self) {
        self.join = JoinState::default();
        if matches!(self.panel, Panel::Code(_) | Panel::Email(_)) {
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
        self.edit.frame(now, snap);
        if self.edit.take_account_ask() {
            self.open_account = true;
        }
        if self.edit.take_signed_out() {
            self.gate = Gate::SignedOut;
            if self.screen != Screen::Gate {
                self.resume = if self.screen == Screen::Bank { Screen::Circle } else { self.screen };
            }
            self.screen = Screen::Gate;
            self.notice = Some(Notice::Info("Your session ended. Sign in again; where you were is kept, and nothing was lost.".into()));
        }
        if self.edit.invite_required {
            self.invite_required = true;
        }
        self.auto();
    }

    /// The account's state, every frame of the Shared page, the tabs shown or not (the tabs are not drawn while
    /// signed out, so `frame` never sees a sign-out). Critic 60 I1: what was read for one account is never shown to
    /// another. Signed out: everything read goes (a bank's page and its hiding state, the circle's page with its
    /// members and invitations, the history and the versions refused); where the person was is kept, and read again
    /// when the same account signs in. Another account signing in starts at the list.
    pub fn note_account(&mut self, snap: &AccountSnapshot) {
        // (critic 69 minor 5) the account is looked up again or signed out: a sign-in follows, and the adds stopped by a
        // signed-out answer are tried once more then
        if !snap.gate.confirmed_pending() && snap.gate != Gate::Ready {
            self.adds_out = false;
        }
        match (&snap.gate, &snap.account) {
            // (round 54: a confirmed pending account uses circles, so its reads belong to it too)
            (g, Some(a)) if g == &Gate::Ready || g.confirmed_pending() => {
                if self.account.as_ref().is_some_and(|p| p != a) {
                    self.forget_account();
                }
                self.account = Some(a.clone());
                self.account_out = false;
            }
            (Gate::SignedOut, _) if self.account.is_some() && !self.account_out => {
                self.forget_loaded();
                self.account_out = true;
            }
            _ => {}
        }
    }

    /// What was read for the account, and the account's own jobs and answers, go; `keep` says where to start again;
    /// `another`: another account signed in (not only a sign-out).
    fn forget_reads(&mut self, keep: Screen, another: bool) {
        let (open, cid, bid) = (self.open, self.circle.as_ref().map(|c| c.id.clone()), self.bank.as_ref().map(|b| b.bid.clone()));
        // (a copy already put into the banks is linked first, for the account that asked for it)
        if let Some(l) = take_landed() {
            self.finish_landing(l);
        }
        // (critic 69 minor 4) signed out: a copy read for an add stays in memory, not put into the banks, until the
        // same account is back (it is not read twice); another account: it goes
        let kept = if another { None } else { self.ready.take().or_else(|| LANDING.with(|s| s.borrow_mut().take()).map(|l| l.r)).filter(|r| r.add.is_some()) };
        self.reset(Screen::List);
        self.edit.forget_account(another);
        match (keep, cid, bid) {
            (Screen::Bank, Some(c), Some(b)) => {
                self.circle = Some(CircleView { id: c.clone(), ..CircleView::default() });
                self.bank = Some(new_bank_view(&c, &b));
                self.resume = Screen::Bank;
            }
            (Screen::Circle | Screen::Bank, Some(c), _) => {
                self.circle = Some(CircleView { id: c, ..CircleView::default() });
                self.resume = Screen::Circle;
            }
            (Screen::Join | Screen::Create, ..) => self.resume = keep,
            _ => {}
        }
        self.open = open;
        self.ready = kept;
        self.last_act = None;
        self.heads_wanted = None;
        self.heads = None;
        self.copy_name = None;
        self.copy_seed = None;
        self.joining_name = None;
        self.purge_claimed.clear();
        self.invite_required = false;
        // (phase D1) what the checks read; a copy read for the account and not yet in the banks is read again
        self.links.forget();
        drop_landing();
        if another {
            // (critic 67 I2) none of the adds is made for the account that signed in now: those not finished say
            // so on the Banks list; what the earlier ones said goes with what was read for that account. (critic 71
            // minor 5) Those stopped by an account switch stay, not dismissed, so the account that asked for them is
            // told once it is back.
            self.adds.retain(|a| in_flight(&a.state) || (stopped_by_switch(&a.state) && !a.dismissed));
            for a in &mut self.adds {
                if in_flight(&a.state) {
                    a.state = AddState::Failed(Problem { message: ADD_OTHER_ACCOUNT.into(), retry: false, retry_at: None, account: false });
                }
            }
        }
        // (critic 67 I2) signed out: the adds wait (`reset` above put the one under way back), and are made once the
        // same account signs in again
        self.join_offer = None;
        self.scroll_to = None;
    }

    /// Signed out: nothing read stays, where the person was is kept (see note_account).
    fn forget_loaded(&mut self) {
        let at = if self.screen == Screen::Gate { self.resume } else { self.screen };
        self.forget_reads(at, false);
        self.gate = Gate::SignedOut;
    }

    /// Another account signed in: it starts at the list (see note_account).
    fn forget_account(&mut self) {
        self.forget_reads(Screen::List, true);
    }

    fn apply_snapshot(&mut self, snap: &AccountSnapshot) {
        self.note_account(snap);
        self.session = snap.session.clone();
        self.gate = snap.gate.clone();
        // (round 54: a confirmed pending account is signed in and may use circles: it is not sent to the gate)
        let usable = self.gate == Gate::Ready || self.gate.confirmed_pending();
        if self.screen == Screen::Gate {
            if usable {
                self.screen = self.resume;
            }
        } else if !usable && matches!(self.gate, Gate::SignedOut | Gate::NotApproved(_) | Gate::NotSetUp | Gate::NoServer) {
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
        self.auto_adds();
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

    /// The next "Add to My Banks" or "Make my own copy", once the one before is in the banks; and the banks of a
    /// circle just joined, for the offer.
    fn auto_adds(&mut self) {
        let wait = self.join_offer.as_mut().is_some_and(|o| std::mem::take(&mut o.wait));
        if !wait
            && let Some(o) = &self.join_offer
            && o.banks.is_none()
            && o.problem.is_none()
            && !self.running(JobKind::Offer)
        {
            let cid = o.cid.clone();
            self.launch(Work::Offer(cid));
        }
        self.next_add();
    }

    /// Starts the next add the person asked for, once the one before is in the banks (one copy at a time).
    fn next_add(&mut self) {
        if self.adds_out || self.running(JobKind::Copy) || self.ready.is_some() || landing_pending() {
            return;
        }
        // (critic 69 minor 1) only an add the account signed in now asked for
        let Some(i) = self.adds.iter().position(|a| a.state == AddState::Waiting && self.ours(a)) else { return };
        let (cid, bid) = (self.adds[i].cid.clone(), self.adds[i].bid.clone());
        if self.launch(Work::Add(cid, bid)) {
            self.adds[i].state = AddState::Working;
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
                let wait = r.detail.retry_after.filter(|s| *s > 0).map(|s| super::api::later(now, s));
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
                            p.message = "You have tried as many circle codes today as verSHion allows. Try again tomorrow, or ask the owner to check the code.".into();
                        }
                    }
                    RefusalKind::RateLimited | RefusalKind::TooManyOpenReads | RefusalKind::TooManyUploads | RefusalKind::Busy | RefusalKind::HourlyLimit { .. } => {
                        p.retry = true;
                        p.retry_at = wait;
                    }
                    // (a `409 conflict` without a head: something changed at the same moment; "republish" is the
                    // public pool's word, not a circle's)
                    RefusalKind::Conflict { .. } => p.message = "Something changed at the same moment. Reload the page and try again.".into(),
                    RefusalKind::NotFound => p.message = "That circle or bank is not available to you any more. It may have been closed, or you may have left it or been removed.".into(),
                    RefusalKind::CircleClosed { undo_until, reason } => {
                        let owner = !matches!(reason.as_deref(), Some("admin" | "last_member_gone"));
                        p.message = match undo_until {
                            Some(u) if owner && *u > self.now_ms() => format!("This circle is closed. The owner can still reopen it {}.", text::until(*u, self.now_ms())),
                            _ => e.message(),
                        }
                    }
                    RefusalKind::ModerationState { message } => {
                        p.message = message.as_deref().map(api::plain).filter(|m| !m.is_empty()).unwrap_or_else(|| "verSHion does not allow that in this circle right now.".into());
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
                            Some("user_id") => "verSHion did not recognise that member. Reload the circle.".into(),
                            Some("email") => "That is not an email address verSHion can send to. Check it for a typo.".into(),
                            Some(f) => format!("verSHion did not accept the {}. Nothing was changed.", f.replace('_', " ")),
                            None => "verSHion did not accept that. Nothing was changed.".into(),
                        }
                    }
                    RefusalKind::Other => match (r.detail.status, r.detail.code.as_str()) {
                        (409, "conflict") => p.message = "Something changed at the same moment. Reload the page and try again.".into(),
                        (_, "pool_migrating") => {
                            p.message = "verSHion's server is being upgraded. Try again in a few minutes.".into();
                            p.retry = true;
                            p.retry_at = wait;
                        }
                        _ => {}
                    },
                    _ => {}
                }
            }
        }
        // (critic 70 I1) the public pool's words, as every circle screen says them
        p.message = text::circle_words(&p.message);
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
            // An emailed invitation is never sent again by "Try again": its address is not kept.
            self.last_act = if matches!(a, Act::EmailInvite { .. }) { None } else { Some(a.clone()) };
        }
        self.jobs.retain(|j| j.kind != kind);
        let (tx, rx) = channel::<JobMsg>();
        let (retry, repaint, images) = (self.retry, self.repaint.clone(), self.images.clone());
        // (critic 71 minor 3) kept to the account signed in now: a retry after another account signed in is not sent as it
        let session = super::pool::pinned(session);
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
                JobMsg::Added(key, r) => self.on_added(*key, r),
                JobMsg::Offer(cid, r) => self.on_offer(cid, r),
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
                    for a in &mut self.adds {
                        if a.state == AddState::Working {
                            a.state = AddState::Failed(p.clone());
                        }
                    }
                    if let Some(b) = &mut self.bank {
                        b.copy = CopyPanel::Failed(p);
                    }
                }
                JobKind::Offer => {
                    if let Some(o) = &mut self.join_offer {
                        o.problem = Some(p);
                    }
                }
                _ => {
                    // A change that lost its answer frees what was waiting for it.
                    if let Panel::Email(f) = &mut self.panel
                        && f.working
                    {
                        f.working = false;
                        f.problem = Some(p.clone());
                        f.problem_domain = None;
                    }
                    if let Some(b) = &mut self.bank
                        && matches!(b.hide, HideStage::Working { .. })
                        && let HideStage::Working { on, back } = std::mem::take(&mut b.hide)
                    {
                        b.hide = HideStage::Failed { on, problem: p.clone(), back };
                    }
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
                // (the working copies' sends ask about pictures first for a bank that hides its keys)
                // (critic 65 I2) only a link whose role was seen by the account signed in now: whether another account
                // is the bank's author is not this account's to say
                let tag = super::bank_links::account_tag(self.account.as_deref());
                for b in &d.banks {
                    self.edit.note_hiding(&b.id, b.hide_key);
                    // (critic 63 I5) a link of this computer learns whether the person is the bank's author
                    if let Some(local) = self.edit.with_bank_link(&cid, &b.id, |l| (l.role_seen_by == tag).then(|| l.local_bank_id.clone())).flatten() {
                        self.edit.update_link(&local, |l| super::bank_links::learn_author(l, &d));
                    }
                }
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
                // A cursor the pool does not take (round 45: `400 invalid_request {field: "before"}`), or one this
                // app could not send: the list starts again from the newest line.
                let bad_cursor = !first
                    && (matches!(e.kind(), Some(RefusalKind::InvalidRequest { field: Some(f) }) if f == "before") || matches!(e, PoolError::Local(super::pool::LocalProblem::BadField("cursor"))));
                if bad_cursor {
                    if let Some(c) = &mut self.circle {
                        c.activity.clear();
                        c.activity_next = None;
                        c.activity_loaded = false;
                        c.activity_problem = None;
                    }
                    return;
                }
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
                // A viewer of a bank that hides its keys is served the viewers' form (phase C).
                let form = if doc.bank.keys_hidden { Served::Stripped } else { Served::Full };
                let view = StudentView::from_payload(&doc.payload, form).map_err(|e| text::circle_words(&e.message()));
                self.edit.note_hiding(&bid, doc.bank.hide_key || doc.bank.keys_hidden);
                if let Some(b) = &mut self.bank {
                    b.load = BankLoad::Ready(Box::new(BankLoaded { doc, view }));
                    b.withheld_head = false;
                    b.hide_off_releases = false;
                }
            }
            Err(e) => {
                let removed = matches!(e.kind(), Some(RefusalKind::BankRemoved));
                let withheld = matches!(e.kind(), Some(RefusalKind::VersionWithheld));
                // (critic 59 B1) Only the pool knows whether turning hiding off releases the version.
                let releases = withheld && matches!(&e, PoolError::Refused(r) if r.detail.json.get("hide_off_releases").and_then(serde_json::Value::as_bool) == Some(true));
                if let Some(mut p) = self.problem_of(&e) {
                    if removed {
                        p.message = "This bank was removed from the circle. If you or the circle's owner removed it, it can be brought back for 14 days from the circle's page.".into();
                    }
                    if let Some(b) = &mut self.bank {
                        b.load = BankLoad::Failed(p);
                        b.withheld_head = withheld;
                        b.hide_off_releases = releases;
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
                let working = self.copy_seed.take();
                self.ready = Some(ReadyCopy { imp, name: wanted, circle: CircleOrigin { circle_id: key.0, circle_name }, working, add: None });
            }
            Err(e) => {
                self.copy_seed = None;
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

    /// The circle's name as this page knows it (its page, the list, the join offer).
    pub fn circle_name_of(&self, cid: &str) -> String {
        self.circle
            .as_ref()
            .filter(|c| c.id == cid)
            .and_then(|c| c.detail.as_ref())
            .map(|d| api::plain(&d.circle.name))
            .or_else(|| self.list.items.iter().find(|i| i.id == cid).map(|i| api::plain(&i.name)))
            .or_else(|| self.join_offer.as_ref().filter(|o| o.cid == cid).map(|o| o.name.clone()))
            .unwrap_or_default()
    }

    fn on_added(&mut self, key: (String, String), r: Result<Box<(BankHead, Imported)>, PoolError>) {
        let Some(i) = self.adds.iter().position(|a| a.cid == key.0 && a.bid == key.1 && a.state == AddState::Working) else { return };
        match r {
            Ok(b) => {
                let (head, imp) = *b;
                let a = self.adds[i].clone();
                let circle_name = self.circle_name_of(&a.cid);
                let working = (!a.own).then(|| head.clone());
                self.ready = Some(ReadyCopy { imp: Box::new(imp), name: String::new(), circle: CircleOrigin { circle_id: a.cid.clone(), circle_name }, working, add: Some(AddFor { cid: a.cid, bid: a.bid, own: a.own, role: a.role, full: a.full }) });
            }
            Err(e) => match self.problem_of(&e) {
                Some(p) => self.adds[i].state = AddState::Failed(p),
                // (the sign-in ended: it is made once signed in again; critic 67 I1: not asked again on every frame
                // while the account still looks signed in)
                None => {
                    self.adds[i].state = AddState::Waiting;
                    self.adds_out = true;
                }
            },
        }
    }

    fn on_offer(&mut self, cid: String, r: Result<CircleDetail, PoolError>) {
        if !self.join_offer.as_ref().is_some_and(|o| o.cid == cid) {
            return;
        }
        match r {
            Ok(d) => {
                // (critic 65 B1) While this computer's list of shared banks can not be read, no bank can be told
                // apart from one it already has: none is offered.
                if self.edit.unreadable().is_some() {
                    if let Some(o) = &mut self.join_offer {
                        o.problem = Some(Problem { message: LINKS_UNREADABLE_JOIN.into(), retry: false, retry_at: None, account: false });
                    }
                    return;
                }
                let readable: Vec<&BankEntry> = d.banks.iter().filter(|b| b.removed.is_none() && !b.hidden).collect();
                // (critic 63 B3) A bank this computer already has is said, never ticked, never added again.
                let mut banks: Vec<OfferBank> = readable.iter().map(|b| OfferBank { id: b.id.clone(), title: super::publish_text::shown(&b.title), ticked: false, in_banks: self.edit.linked_elsewhere(&cid, &b.id, "") }).collect();
                let tick = super::bank_links::join_ticks(banks.iter().filter(|b| b.in_banks.is_none()).count());
                for b in &mut banks {
                    b.ticked = tick && b.in_banks.is_none();
                }
                if let Some(o) = &mut self.join_offer {
                    o.banks = Some(banks);
                    o.problem = None;
                }
            }
            Err(e) => {
                let p = self.problem_of(&e);
                if let Some(o) = &mut self.join_offer {
                    o.problem = p.or(Some(Problem { message: "Your session ended. Sign in again, then add the circle's banks from its page.".into(), retry: false, retry_at: None, account: false }));
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
                let role = Role::parse(&j.role);
                // (critic 65 I1) a copy the circle had said the person was no longer in is checked again at once
                let locals: Vec<String> = self.edit.links_in(&j.circle_id).into_iter().map(|l| l.local_bank_id).collect();
                self.links.rearm(&locals);
                // (phase D1) "Add its banks to My Banks?": the circle's banks are read for the offer.
                self.join_offer = (!j.already && super::pool::is_item_id(&j.circle_id)).then(|| JoinOffer { cid: j.circle_id.clone(), name: name.clone(), role, banks: None, problem: None, started: false, wait: true });
                self.join = JoinState { text: String::new(), stage: JoinStage::Done { name, role, already: j.already, circle_id: j.circle_id } };
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
            self.edit.apply_heads(&h);
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
                        Act::EmailInvite { .. } => {
                            // (the address stays in the form, so it can be sent once signed in again)
                            if let Panel::Email(f) = &mut self.panel {
                                f.working = false;
                            }
                        }
                        Act::HideKey { .. } => {
                            if let Some(b) = &mut self.bank {
                                let next = match std::mem::take(&mut b.hide) {
                                    HideStage::Working { back: Some(d), .. } => HideStage::Decide(d),
                                    // (from the picture step: back to it, the choices as they were)
                                    HideStage::Working { on: true, back: None } => b.picked.clone().map_or(HideStage::Idle, HideStage::Pictures),
                                    _ => HideStage::Idle,
                                };
                                if matches!(next, HideStage::Idle) {
                                    b.picked = None;
                                }
                                b.hide = next;
                            }
                        }
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
                    Act::EmailInvite { email, .. } => {
                        // Said in the form, which stays open with the address (to fix a typo); the page is not read
                        // again, so the form is not lost.
                        // (critic 57 I4) A limit the pool keeps per mail domain waits for that domain only.
                        let domain = match e.kind() {
                            Some(RefusalKind::HourlyLimit { metric: Some(m), .. }) if matches!(m.as_str(), "circle_email_inviter_domain_hourly" | "circle_email_domain_hourly") => mail_domain(email.expose()),
                            _ => None,
                        };
                        match e.kind() {
                            Some(RefusalKind::Conflict { .. } | RefusalKind::CircleConflict { .. }) => {
                                p.message = "That invitation changed at the same moment (it was used, withdrawn or it expired). Nothing was sent: invite the address again.".into();
                            }
                            Some(RefusalKind::Busy) => {
                                p.message = "Too many invitations are being sent right now, by everyone. Nothing was sent. Try again later.".into();
                            }
                            Some(RefusalKind::InviteNotSent { .. } | RefusalKind::CircleFull { .. } | RefusalKind::CirclesLimit { .. }) => p.retry = false,
                            Some(RefusalKind::NotFound) => p.message = "This circle is not available to you any more. Nothing was sent.".into(),
                            Some(RefusalKind::InvalidRequest { field }) => {
                                p.message = match field.as_deref() {
                                    Some("email") => "That is not an email address verSHion can send to. Check it for a typo. Nothing was sent.".into(),
                                    Some("role") => "Choose whether the person can add banks or can only look and copy.".into(),
                                    _ => "verSHion did not accept that invitation. Nothing was sent.".into(),
                                }
                            }
                            _ => {}
                        }
                        let stale = matches!(e.kind(), Some(RefusalKind::Conflict { .. } | RefusalKind::CircleConflict { .. } | RefusalKind::CircleRoleRequired { .. }));
                        if let Panel::Email(f) = &mut self.panel {
                            f.working = false;
                            f.problem = Some(p);
                            f.problem_domain = domain;
                        }
                        if let Some(c) = &mut self.circle
                            && stale
                        {
                            c.invites = None;
                        }
                    }
                    Act::HideKey { decisions, on, base, .. } => {
                        let asks = if let PoolError::Refused(r) = &e { decision_refusal(&r.kind).cloned() } else { None };
                        if let Some(pr) = asks {
                            // The pool lists what it needs decided for the viewers' copy: the guided page.
                            if let Some(b) = &mut self.bank {
                                let mut d = Decisions::new(pr, decisions.clone());
                                d.head = *base;
                                b.hide = HideStage::Decide(Box::new(d));
                            }
                            return;
                        }
                        let mut reread = false;
                        // (critic 57 I2) The decisions made stay for "Try again", unless the bank changed under them.
                        let mut keep = true;
                        match e.kind() {
                            // (critic 62 B1: the PATCH is pinned to the version its choices were made on; a newer one
                            // is this conflict, and every choice made goes, the pictures' too)
                            Some(RefusalKind::CircleConflict { .. } | RefusalKind::Conflict { .. }) => {
                                let (pictures, change) = self.bank.as_ref().map_or((false, false), |b| (b.picked.is_some(), b.pictures_change));
                                p.message = text::hide_moved_message(has_decisions(decisions), pictures, change);
                                reread = true;
                                keep = false;
                            }
                            Some(RefusalKind::InvalidRequest { field: Some(f) }) if matches!(f.as_str(), "labels" | "order_acks" | "ack_formatting" | "image_ok") => {
                                p.message = "The bank changed meanwhile (its newest version is not the one the decisions were for), so nothing was changed. The bank was read again: start again.".into();
                                reread = true;
                                keep = false;
                            }
                            Some(RefusalKind::InvalidRequest { .. }) => p.message = "verSHion did not accept that request. Nothing was changed.".into(),
                            Some(RefusalKind::BankRemoved | RefusalKind::NotFound | RefusalKind::CircleRoleRequired { .. } | RefusalKind::CircleClosed { .. }) => reread = true,
                            _ => {}
                        }
                        if let PoolError::Api(ApiError::Offline(why)) = &e {
                            // A lost answer can not say "nothing changed": the change may have been made.
                            let kept = if decisions.is_empty() { "" } else { " Your decisions are kept." };
                            p.message = format!("{why} The answer did not arrive, so it is not known whether the setting changed. Try again to find out.{kept}");
                        }
                        if let Some(b) = &mut self.bank {
                            let back = match std::mem::take(&mut b.hide) {
                                HideStage::Working { back, .. } if keep => back,
                                _ => None,
                            };
                            // (the picture step's choices go with the decisions: kept for "Try again", or dropped)
                            if !keep {
                                b.picked = None;
                            }
                            b.hide = HideStage::Failed { on: *on, problem: p, back };
                            if reread {
                                b.load = BankLoad::Loading;
                            }
                        }
                        if reread && let Some(c) = &mut self.circle {
                            c.detail = None;
                        }
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
                        // The page shows several banks: say which one an administrator is still reviewing.
                        if let (Act::PurgeBank { bid, .. }, PoolError::Refused(r)) = (&a, &e)
                            && r.detail.code == "pending_review"
                        {
                            let title = self.circle.as_ref().and_then(|c| c.detail.as_ref()).and_then(|d| d.banks.iter().find(|b| b.id == *bid)).map(|b| super::publish_text::shown(&b.title)).filter(|t| !t.is_empty());
                            if let Some(t) = title {
                                p.message = format!("An administrator is still reviewing a version of \"{t}\". It can be deleted for good once that review is done. You can leave it removed until then.");
                            }
                        }
                        // After a purge was claimed (round 48) the bank can not be brought back.
                        if let (Act::RestoreBank { .. }, Some(RefusalKind::BankRemoved)) = (&a, e.kind()) {
                            p.message = "This bank can no longer be brought back: deleting it for good has already started.".into();
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
            (Act::PurgeBank { bid, .. }, done) => {
                if let Some(c) = &mut self.circle {
                    c.detail = None;
                    c.activity_loaded = false;
                    c.act_problem = None;
                }
                match done {
                    ActDone::Purged(p) if p.partial => {
                        self.notice = Some(Notice::Info(text::purge_partial_line(p.evidence_kept)));
                        self.purge_claimed.insert(bid, p.evidence_kept);
                    }
                    _ => self.notice = Some(Notice::Info("The bank was deleted for good.".into())),
                }
            }
            (Act::EmailInvite { .. }, ActDone::Emailed(sent)) => {
                if let Panel::Email(f) = &mut self.panel {
                    // The pool has it now: the address is not kept a moment longer.
                    zeroize::Zeroize::zeroize(&mut f.address);
                    f.working = false;
                    f.problem = None;
                    f.problem_domain = None;
                    f.sent = Some(*sent);
                }
                if let Some(c) = &mut self.circle {
                    c.invites = None;
                    c.act_problem = None;
                    c.activity_loaded = false;
                }
            }
            (Act::HideKey { on, decisions, .. }, ActDone::HideHeadMoved) => {
                // (critic 57 B1) Nothing was sent. What was decided was for the older version, so it goes.
                let (pictures, change) = self.bank.as_ref().map_or((false, false), |b| (b.picked.is_some(), b.pictures_change));
                let message = text::hide_moved_message(has_decisions(&decisions), pictures, change);
                if let Some(b) = &mut self.bank {
                    b.hide = HideStage::Failed { on, problem: Problem { message, retry: false, retry_at: None, account: false }, back: None };
                    b.load = BankLoad::Loading;
                    b.picked = None;
                }
                if let Some(c) = &mut self.circle {
                    c.detail = None;
                }
            }
            (Act::HideKey { bid, .. }, ActDone::Hidden(r)) => {
                let mut r = *r;
                self.edit.note_hiding(&bid, r.hide_key);
                if let Some(b) = &mut self.bank {
                    // What the pool left out that the person did not choose to drop (kept, or never listed).
                    let dropped = b.picked.take().filter(|_| b.bid == bid).map(|p| p.dropped()).unwrap_or_default();
                    r.left_out_unasked = left_out_unasked(&dropped, &r.removed_pictures);
                    b.hide = HideStage::Done(r);
                    // The bank and the circle page are read again (the setting, a hold).
                    b.load = BankLoad::Loading;
                }
                if let Some(c) = &mut self.circle {
                    c.detail = None;
                    c.activity_loaded = false;
                }
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

/// Decisions other than the pictures kept (labels, orders, formatting checks).
fn has_decisions(d: &StripDecisions) -> bool {
    !(d.labels.is_empty() && d.order_acks.is_empty() && d.ack_formatting.is_empty())
}

fn new_bank_view(cid: &str, bid: &str) -> BankView {
    BankView { cid: cid.to_string(), bid: bid.to_string(), load: BankLoad::Loading, copy: CopyPanel::Closed, report: None, act_problem: None, confirm_remove: false, license_choice: None, hide: HideStage::Idle, withheld_head: false, hide_off_releases: false, picked: None, pictures_change: false, preview: false }
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

    /// Owner decision (a): "Preview" in a bank's row on the circle page. The bank is read once (the reads every
    /// member's open of a bank already counts; the viewers' form for a viewer of a bank that hides its keys) and its
    /// questions shown without their answers. Nothing on that page edits or adds anything.
    pub fn open_preview(&mut self, bid: &str) {
        let Some(cid) = self.cid() else { return };
        let role = self.my_role();
        let closed = self.is_closed();
        let Some(e) = self.entry_of(bid) else { return };
        // (a removed bank can not be read: `bank_can` says so)
        if e.hidden || !bank_can(role, closed, e, BankAction::Copy) {
            return;
        }
        let mut v = new_bank_view(&cid, bid);
        v.preview = true;
        self.bank = Some(v);
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
                // (a preview changed nothing: the circle's page is not read again)
                let preview = self.bank.as_ref().is_some_and(|b| b.preview);
                self.bank = None;
                self.screen = Screen::Circle;
                self.resume = Screen::Circle;
                // The circle page is read again so a change shows.
                if let Some(c) = &mut self.circle
                    && !preview
                {
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
        // (round 54) a confirmed pending account joins by code; creating a circle waits for approval (the server refuses it).
        if self.gate.confirmed_pending() {
            return;
        }
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

    // ---- invite by email (phase C, D5) ------------------------------------------------------------------------

    /// "Invite by email...": the owner's form (the address, the role).
    pub fn begin_email(&mut self) {
        if self.permitted(Action::MakeCode) {
            self.panel = Panel::Email(EmailForm::default());
        }
    }

    /// Why "Send the invitation" can not be pressed, or `None`.
    pub fn email_blocker(&self) -> Option<&'static str> {
        let Panel::Email(f) = &self.panel else { return Some("Open the invitation form first.") };
        if f.working {
            return Some("Sending the invitation...");
        }
        if self.running(JobKind::Act) {
            return Some("Wait for the current step to finish.");
        }
        if let Some(p) = email_problem(&f.address) {
            return Some(p);
        }
        // An hourly limit's wait (critic 57 I4): one kept per mail domain holds back only that domain.
        if f.problem.as_ref().and_then(|p| p.retry_at).is_some_and(|at| at > self.now_instant()) {
            match &f.problem_domain {
                Some(d) if mail_domain(&f.address).as_deref() == Some(d.as_str()) => {
                    return Some("Addresses at this mail domain have to wait for the time shown above. You can invite someone with an address at another mail domain now.");
                }
                Some(_) => {}
                None => return Some("Wait for the time shown above, then send it."),
            }
        }
        None
    }

    /// Sends the invitation: the address goes to the pool once. It is wiped from the form as soon as the pool
    /// took it; on a refusal it stays (to fix a typo) until the form is closed.
    pub fn send_email_invite(&mut self) {
        let Some(cid) = self.cid() else { return };
        if !self.permitted(Action::MakeCode) || self.email_blocker().is_some() {
            return;
        }
        let Panel::Email(f) = &self.panel else { return };
        if f.role == Role::Owner {
            return;
        }
        let a = Act::EmailInvite { cid, email: Secret::new(f.address.trim()), role: f.role };
        if self.launch(Work::Act(a))
            && let Panel::Email(f) = &mut self.panel
        {
            f.working = true;
            f.problem = None;
            f.problem_domain = None;
            f.sent = None;
        }
    }

    /// "Invite someone else": an empty form with the same role (the last result goes).
    pub fn another_email(&mut self) {
        if let Panel::Email(f) = &mut self.panel
            && !f.working
        {
            zeroize::Zeroize::zeroize(&mut f.address);
            f.problem = None;
            f.problem_domain = None;
            f.sent = None;
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

    // ---- add to my banks / make my own copy (phase D1) ------------------------------------------------------------

    /// Where "Add to My Banks" or "Make my own copy" of this circle bank stands, the newest first.
    pub fn add_state(&self, cid: &str, bid: &str) -> Option<&AddJob> {
        self.adds.iter().rev().find(|a| a.cid == cid && a.bid == bid)
    }

    fn entry_of(&self, bid: &str) -> Option<&BankEntry> {
        self.circle.as_ref()?.detail.as_ref()?.banks.iter().find(|e| e.id == bid)
    }

    /// "Add to My Banks" (`own` false: linked, Q2: a follow link for a viewer) or "Make my own copy" (`own`: not
    /// linked, with a note of where it came from) for a bank of the open circle.
    pub fn add_bank(&mut self, bid: &str, own: bool) {
        let Some(cid) = self.cid() else { return };
        let role = self.my_role();
        let Some(e) = self.entry_of(bid) else { return };
        if e.hidden || !bank_can(role, self.is_closed(), e, BankAction::Copy) {
            return;
        }
        let title = super::publish_text::shown(&e.title);
        self.queue_add(&cid, bid, own, &title, role, false);
    }

    /// "Add a full copy" (phase D1): the bank, read in full, as a new bank in My Banks that takes the circle bank's
    /// link from this computer's copy. Only where that copy can never send while the person now could: one made
    /// from the viewers' form, or a viewer's copy after a promotion to editor (critic 63 I3, critic 64 I2). The
    /// earlier copy stays in My Banks, not connected.
    pub fn add_full_copy(&mut self, bid: &str) {
        let Some(cid) = self.cid() else { return };
        let role = self.my_role();
        let Some(e) = self.entry_of(bid) else { return };
        if e.hidden || !role.can_add_banks() || !bank_can(role, self.is_closed(), e, BankAction::Copy) {
            return;
        }
        let title = super::publish_text::shown(&e.title);
        if self.edit.with_bank_link(&cid, bid, |l| super::bank_links::full_copy_offered(l, role)) == Some(true) {
            self.queue_add(&cid, bid, false, &title, role, true);
        }
    }

    fn queue_add(&mut self, cid: &str, bid: &str, own: bool, title: &str, role: Role, full: bool) {
        if self.adds.iter().any(|a| a.cid == cid && a.bid == bid && matches!(a.state, AddState::Waiting | AddState::Working)) {
            return;
        }
        // (critic 63 B3) Never a second linked bank for a circle bank this computer still has; (critic 65 B1) and
        // none while this computer's list of shared banks can not be read, as it may have it.
        if !own && (self.edit.unreadable().is_some() || (!full && self.edit.linked_elsewhere(cid, bid, "").is_some())) {
            return;
        }
        self.adds.retain(|a| !(a.cid == cid && a.bid == bid && a.own == own));
        let account = self.account_tag();
        self.adds.push(AddJob { cid: cid.to_string(), bid: bid.to_string(), own, title: title.to_string(), role, state: AddState::Waiting, full, dismissed: false, account });
        self.note_unfinished();
    }

    /// The account signed in as this model last saw it, as a short fingerprint (see `bank_links::account_tag`).
    pub fn account_tag(&self) -> Option<String> {
        super::bank_links::account_tag(self.account.as_deref())
    }

    /// (critic 69 minor 1) An add asked for by the account signed in now (or by one this model never knew).
    fn ours(&self, a: &AddJob) -> bool {
        a.account.is_none() || a.account == self.account_tag()
    }

    /// A tick of the join offer.
    pub fn tick_offer(&mut self, bid: &str, on: bool) {
        if let Some(o) = &mut self.join_offer
            && !o.started
            && let Some(b) = o.banks.as_mut().and_then(|v| v.iter_mut().find(|b| b.id == bid))
        {
            b.ticked = on && b.in_banks.is_none();
        }
    }

    /// "Add the ticked banks": each is added to My Banks (linked; a follow link for a viewer), one after another.
    pub fn add_ticked(&mut self) {
        let Some(o) = &self.join_offer else { return };
        if o.started {
            return;
        }
        let (cid, role) = (o.cid.clone(), o.role);
        let picks: Vec<(String, String)> = o.banks.iter().flatten().filter(|b| b.ticked && b.in_banks.is_none()).map(|b| (b.id.clone(), b.title.clone())).collect();
        if picks.is_empty() {
            return;
        }
        for (bid, title) in picks {
            self.queue_add(&cid, &bid, false, &title, role, false);
        }
        if let Some(o) = &mut self.join_offer {
            o.started = true;
        }
    }

    /// "Not now" (or the offer is done with).
    pub fn dismiss_offer(&mut self) {
        self.join_offer = None;
    }

    /// "Try again" on a failed add: only by the account that asked for it (critic 69 minor 1), with its role now.
    pub fn retry_add(&mut self, cid: &str, bid: &str) {
        let role = (self.cid().as_deref() == Some(cid)).then(|| self.my_role());
        let Some(i) = self.adds.iter().rposition(|a| a.cid == cid && a.bid == bid) else { return };
        if !matches!(self.adds[i].state, AddState::Failed(_)) || !self.ours(&self.adds[i]) {
            return;
        }
        let a = &mut self.adds[i];
        a.state = AddState::Waiting;
        if let Some(r) = role {
            a.role = r;
        }
        self.note_unfinished();
    }

    /// "Report..." in a bank's row on the circle page: the form opens under the row.
    pub fn begin_report_on(&mut self, bid: &str) {
        self.hold_row(bid);
        self.begin_report();
    }

    /// "Remove..." in a bank's row on the circle page: asks under the row.
    pub fn ask_remove_on(&mut self, bid: &str) {
        self.hold_row(bid);
        self.ask_remove_bank();
    }

    /// The bank whose report form or removal question is open under its row, while the circle page shows.
    pub fn row_bank(&self) -> Option<&BankView> {
        self.bank.as_ref().filter(|_| self.screen == Screen::Circle)
    }

    fn hold_row(&mut self, bid: &str) {
        let Some(cid) = self.cid() else { return };
        if !super::pool::is_item_id(bid) {
            return;
        }
        // (the bank is not read: the circle page's row says everything the form needs)
        if !self.bank.as_ref().is_some_and(|b| b.cid == cid && b.bid == bid) {
            self.bank = Some(new_bank_view(&cid, bid));
        }
    }

    /// The row to bring into view, once.
    pub fn take_scroll_to(&mut self, bid: &str) -> bool {
        if self.scroll_to.as_deref() == Some(bid) {
            self.scroll_to = None;
            return true;
        }
        false
    }

    /// "Get a copy": fetches the pictures and makes the bank on this device.
    pub fn get_copy(&mut self) {
        self.start_copy(false);
    }

    /// "Make a working copy": the same copy, and the bank stays linked to the circle bank, so
    /// changes can be sent back and the circle's newer versions merged in.
    pub fn make_working_copy(&mut self) {
        if self.can_make_working_copy() {
            self.start_copy(true);
        }
    }

    /// Whether "Make a working copy" is offered for the bank on screen. Never for the viewers' form of a
    /// bank that hides its keys: a copy without keys can not be sent back (`keyless_question`).
    pub fn can_make_working_copy(&self) -> bool {
        self.bank_allowed(BankAction::Copy) && self.role().can_add_banks() && !self.keys_hidden_here()
    }

    /// The bank on screen was served without its answer keys (the person reads it as a viewer).
    pub fn keys_hidden_here(&self) -> bool {
        matches!(self.bank.as_ref().map(|b| &b.load), Some(BankLoad::Ready(l)) if l.doc.bank.keys_hidden)
    }

    /// The bank on screen hides its answer keys from viewers (every member is told).
    pub fn hides_keys_here(&self) -> bool {
        match self.bank.as_ref().map(|b| &b.load) {
            Some(BankLoad::Ready(l)) => l.doc.bank.hide_key || l.doc.bank.keys_hidden,
            _ => self.bank_entry().is_some_and(|e| e.hide_key),
        }
    }

    fn role(&self) -> Role {
        self.circle.as_ref().map(|c| c.role()).unwrap_or(Role::Viewer)
    }

    fn start_copy(&mut self, working: bool) {
        if !self.bank_allowed(BankAction::Copy) || (working && self.keys_hidden_here()) {
            return;
        }
        let Some(b) = &mut self.bank else { return };
        let (CopyPanel::Choosing { name }, BankLoad::Ready(l)) = (&b.copy, &b.load) else { return };
        let name = name.trim().to_string();
        let doc = Box::new(l.doc.clone());
        let seed = working.then(|| l.doc.bank.clone());
        self.copy_name = Some(name.clone());
        self.copy_seed = seed;
        if self.launch(Work::Copy(doc, name)) {
            if let Some(b) = &mut self.bank {
                b.copy = CopyPanel::Working;
            }
        } else {
            self.copy_seed = None;
        }
    }

    /// What the person may do with a bank of this circle, for the working-copy screens.
    pub fn perm_for(&self, cid: &str, bid: &str) -> Perm {
        let d = self.circle.as_ref().filter(|c| c.id == cid).and_then(|c| c.detail.as_ref());
        match d {
            Some(d) => Perm { role: d.circle.role(), is_author: d.banks.iter().find(|b| b.id == bid).is_some_and(|b| b.is_author), closed: d.circle.closed() },
            None => Perm { role: Role::Viewer, is_author: false, closed: false },
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

    // ---- hide answer keys from viewers (phase C, D12) ---------------------------------------------------------

    /// The switch is offered: the bank's author while an editor, or the circle's owner, on a bank read in full.
    pub fn hide_allowed(&self) -> bool {
        self.bank_allowed(BankAction::HideKeys) && matches!(self.bank.as_ref().map(|b| &b.load), Some(BankLoad::Ready(_))) && !self.keys_hidden_here()
    }

    /// The setting as last read, or as the change just made left it (until the bank is read again).
    pub fn hide_key_on(&self) -> bool {
        match self.bank.as_ref().map(|b| &b.hide) {
            Some(HideStage::Done(r)) => r.hide_key,
            _ => self.hides_keys_here(),
        }
    }

    /// The newest version is held back for review, so the bank can not be read, and the person may change the
    /// setting: the switch stays, both ways, so hiding can always be turned on again (critic 59 B1).
    pub fn hide_switch_on_held_head(&self) -> bool {
        let Some(b) = &self.bank else { return false };
        b.withheld_head && matches!(b.load, BankLoad::Failed(_)) && self.bank_allowed(BankAction::HideKeys)
    }

    /// The bank hides its keys, its newest version is held back for review, and the pool said turning hiding
    /// off releases it (`hide_off_releases`: only an answer-key change held it back): "Turn hiding off" is
    /// offered there (critic 58 B2, critic 59 B1). For the people who may change the setting only.
    pub fn hide_off_offered(&self) -> bool {
        let Some(b) = &self.bank else { return false };
        b.hide_off_releases && self.hide_key_on() && self.hide_switch_on_held_head()
    }

    /// Whether this change of the setting may be asked for now (on a held-back head: only to the other setting).
    fn hide_change_allowed(&self, on: bool) -> bool {
        self.hide_allowed() || (self.hide_switch_on_held_head() && on != self.hide_key_on())
    }

    /// The switch was pressed: asks first, saying what it means.
    pub fn ask_hide(&mut self, on: bool) {
        if !self.hide_change_allowed(on) {
            return;
        }
        if let Some(b) = &mut self.bank
            && matches!(b.hide, HideStage::Idle | HideStage::Done(_) | HideStage::Failed { .. })
        {
            b.hide = HideStage::Confirm { on };
            b.picked = None;
            b.pictures_change = false;
        }
    }

    /// Leaves the confirmation, the decisions page or a result: nothing is sent.
    pub fn cancel_hide(&mut self) {
        if let Some(b) = &mut self.bank
            && !matches!(b.hide, HideStage::Working { .. })
        {
            b.hide = HideStage::Idle;
            b.picked = None;
            b.pictures_change = false;
        }
    }

    /// "Try again" after a failure: the decisions made go again when they were kept (critic 57 I2), else it
    /// asks once more (each try counts toward the day's tries).
    pub fn retry_hide(&mut self) {
        let Some(b) = &mut self.bank else { return };
        if !matches!(b.hide, HideStage::Failed { .. }) {
            return;
        }
        let HideStage::Failed { on, back, .. } = std::mem::take(&mut b.hide) else { return };
        match back {
            Some(d) if on => {
                b.hide = HideStage::Decide(d);
                self.send_hide_decisions();
            }
            // (the change went from the picture step: it goes again with the pictures kept)
            None if on && b.picked.is_some() => {
                b.hide = b.picked.clone().map_or(HideStage::Idle, HideStage::Pictures);
                self.send_hide_pictures();
            }
            _ => self.ask_hide(on),
        }
    }

    /// "Yes": the change goes to the pool (with no decisions yet: the pool lists what it needs). Turning hiding on
    /// over a newest version with pictures asks first which ones viewers keep (the pool never asks: it leaves out
    /// every picture not kept).
    pub fn confirm_hide(&mut self) {
        let on = match self.bank.as_ref().map(|b| &b.hide) {
            Some(HideStage::Confirm { on }) => *on,
            _ => return,
        };
        let base = if on { self.read_version() } else { None };
        if on
            && self.hide_change_allowed(true)
            && let Some(mut p) = self.head_pictures()
        {
            p.head = base;
            if let Some(b) = &mut self.bank {
                b.hide = HideStage::Pictures(Box::new(p));
            }
            return;
        }
        self.start_hide(on, StripDecisions::default(), base, None);
    }

    /// Turning hiding on asks about pictures after the confirmation (the newest version read has some): its button
    /// only leads to the picture step, and costs no try.
    pub fn hide_pictures_follow(&self) -> bool {
        matches!(self.bank.as_ref().map(|b| &b.hide), Some(HideStage::Confirm { on: true })) && self.hide_change_allowed(true) && self.head_pictures().is_some()
    }

    /// "Change picture choices" is offered (critic 62 I2): the keys are hidden, the person may change the setting,
    /// the newest version read in full has pictures, and nothing else is going on.
    pub fn can_change_pictures(&self) -> bool {
        self.hide_allowed()
            && self.hide_key_on()
            && matches!(self.bank.as_ref().map(|b| &b.hide), Some(HideStage::Idle | HideStage::Done(_) | HideStage::Failed { .. }))
            && self.head_pictures().is_some()
    }

    /// "Change picture choices": the picture step again for the newest version read, pinned to it. What was chosen
    /// before is not known here, so every picture starts as dropped; saving sends hiding on again with the new choices.
    pub fn change_pictures(&mut self) {
        if !self.can_change_pictures() {
            return;
        }
        let base = self.read_version();
        let Some(mut p) = self.head_pictures() else { return };
        p.head = base;
        if let Some(b) = &mut self.bank {
            b.hide = HideStage::Pictures(Box::new(p));
            b.picked = None;
            b.pictures_change = true;
        }
    }

    /// The pictures of the version this page read in full (none: no picture step).
    fn head_pictures(&self) -> Option<Pictures> {
        match self.bank.as_ref().map(|b| &b.load) {
            Some(BankLoad::Ready(l)) if !l.doc.bank.keys_hidden => Pictures::of_payload(&l.doc.payload),
            _ => None,
        }
    }

    /// The version of the bank as this page read it.
    fn read_version(&self) -> Option<u64> {
        match self.bank.as_ref().map(|b| &b.load) {
            Some(BankLoad::Ready(l)) => Some(l.doc.bank.version),
            _ => None,
        }
    }

    fn start_hide(&mut self, on: bool, decisions: StripDecisions, base: Option<u64>, back: Option<Box<Decisions>>) {
        if !self.hide_change_allowed(on) {
            return;
        }
        let Some(b) = &self.bank else { return };
        let a = Act::HideKey { cid: b.cid.clone(), bid: b.bid.clone(), on, decisions, base };
        if self.launch(Work::Act(a))
            && let Some(b) = &mut self.bank
        {
            b.hide = HideStage::Working { on, back };
        }
    }

    fn decisions_mut(&mut self) -> Option<&mut Decisions> {
        match self.bank.as_mut().map(|b| &mut b.hide) {
            Some(HideStage::Decide(d)) => Some(d),
            _ => None,
        }
    }

    pub fn hide_decide_label(&mut self, at: super::pool::Pos, c: super::pool::LabelChoice) {
        if let Some(d) = self.decisions_mut() {
            d.decide_label(at, c);
        }
    }

    pub fn hide_set_order(&mut self, at: super::pool::Pos, yes: bool) {
        if let Some(d) = self.decisions_mut() {
            d.set_order(at, yes);
        }
    }

    pub fn hide_all_orders(&mut self) {
        if let Some(d) = self.decisions_mut() {
            d.ack_all_orders();
        }
    }

    pub fn hide_set_formatting(&mut self, question: usize, yes: bool) {
        if let Some(d) = self.decisions_mut() {
            d.set_formatting(question, yes);
        }
    }

    /// "Keep it for viewers" / "Drop it" on the picture step.
    pub fn hide_keep_picture(&mut self, question: usize, yes: bool) {
        if let Some(HideStage::Pictures(p)) = self.bank.as_mut().map(|b| &mut b.hide) {
            p.set_keep(question, yes);
        }
    }

    /// "Hide the keys" on the picture step: the change goes with the questions whose picture is kept (`image_ok`),
    /// for the version the list was made from (the head is read right before, as for the decisions).
    pub fn send_hide_pictures(&mut self) {
        if self.running(JobKind::Act) {
            return;
        }
        let Some(p) = (match self.bank.as_ref().map(|b| &b.hide) {
            Some(HideStage::Pictures(p)) => Some(p.clone()),
            _ => None,
        }) else {
            return;
        };
        let decisions = StripDecisions { image_ok: p.image_ok(), ..StripDecisions::default() };
        let base = p.head.or_else(|| self.read_version());
        if let Some(b) = &mut self.bank {
            b.picked = Some(p);
        }
        self.start_hide(true, decisions, base, None);
    }

    /// "Hide the keys with these decisions": only when everything listed is decided.
    pub fn send_hide_decisions(&mut self) {
        let Some(b) = &mut self.bank else { return };
        let HideStage::Decide(d) = std::mem::take(&mut b.hide) else { return };
        if !d.ready() || self.running(JobKind::Act) {
            if let Some(b) = &mut self.bank {
                b.hide = HideStage::Decide(d);
            }
            return;
        }
        let decisions = d.decisions();
        // (critic 57 B1) Checked against the version the page was made for, right before sending.
        let base = d.head.or_else(|| self.read_version());
        if let Some(b) = &mut self.bank {
            b.hide = HideStage::Decide(d.clone());
        }
        self.start_hide(true, decisions, base, Some(d));
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
        if self.purge_claimed.contains_key(bid) {
            return;
        }
        let now = self.now_ms();
        let ok = self.circle.as_ref().and_then(|c| c.detail.as_ref()).is_some_and(|d| d.banks.iter().any(|e| e.id == bid && bank_can(d.circle.role(), d.circle.closed(), e, BankAction::Restore) && e.removed.as_ref().and_then(|r| r.undo_until).is_some_and(|u| u > now)));
        if ok {
            self.launch(Work::Act(Act::RestoreBank { cid, bid: bid.to_string() }));
        }
    }

    /// The owner deletes a removed bank for good, now (the page asks first).
    pub fn purge_bank(&mut self, bid: &str) {
        let Some(cid) = self.cid() else { return };
        if self.purge_claimed.contains_key(bid) {
            return;
        }
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
        // (owner decision b) a copy put into the banks in the background: its link and its row
        if let Some(l) = take_landed() {
            self.finish_landing(l);
        }
        let mut changed = self.sync_copy(banks, browse);
        if self.open {
            changed |= self.edit.sync(banks);
            for a in self.edit.take_advanced() {
                browse.set_circle_version(&a.circle_bank_id, &a.local_bank_id, a.version);
            }
        }
        // (critic 71 minor 1) an add that failed on the Shared page is no longer counted as unfinished
        self.note_unfinished();
        changed
    }

    fn sync_copy(&mut self, banks: &mut Vec<QuestionBank>, browse: &mut super::browse::BrowseModel) -> bool {
        let Some(l) = self.prepare_landing() else { return false };
        let landed = land(l, banks, browse);
        let changed = landed.applied.is_ok();
        self.finish_landing(landed);
        changed
    }

    /// The model's half of putting a finished copy into the banks: its name, and the link it may take from an
    /// earlier copy ("Add a full copy").
    fn prepare_landing(&mut self) -> Option<Landing> {
        let r = self.ready.take()?;
        // (critic 65 I3) "Add a full copy" read without the answer keys (the person can only view it now): no
        // bank is made and no link moves; the row says why.
        if let (Some(x), Some(h)) = (&r.add, &r.working)
            && x.full
            && h.keys_hidden
        {
            if let Some(job) = self.adds.iter_mut().rev().find(|j| j.cid == x.cid && j.bid == x.bid && j.full && in_flight(&j.state)) {
                job.state = AddState::Failed(Problem { message: FULL_COPY_KEYLESS.into(), retry: false, retry_at: None, account: false });
            }
            return None;
        }
        // (critic 64 I5) Never two banks of the same name from one circle bank without a word: an own copy and a full
        // copy say what they are; the earlier copy a full copy takes the link from is named in the notice.
        let title = api::plain(&r.imp.head.item.title);
        let earlier = r.add.as_ref().filter(|x| x.full).and_then(|x| self.edit.with_bank_link(&x.cid, &x.bid, |l| (l.local_bank_id.clone(), l.viewers_form)));
        let name = match &r.add {
            _ if !r.name.trim().is_empty() => r.name.clone(),
            Some(x) if x.own => format!("{title} (my copy)"),
            Some(x) if x.full && earlier.as_ref().is_some_and(|e| e.1) => format!("{title} (with answer keys)"),
            Some(x) if x.full => format!("{title} (full copy)"),
            _ => title.clone(),
        };
        let tag = super::bank_links::account_tag(self.account.as_deref());
        Some(Landing { r, name, earlier, now_ms: self.now_ms(), tag })
    }

    /// The model's half after the copy is in the banks: the link (never taken from a bank that still has it, critic
    /// 63 B3), what the circle page and the join offer say.
    fn finish_landing(&mut self, landed: Landed) {
        let Landed { add, applied, link, mut link_note, saved, earlier_name, copy_done, mut why } = landed;
        match applied {
            Ok((bank_id, bank_name)) => {
                let mut linked = false;
                if let Some(link) = link {
                    let full = add.as_ref().is_some_and(|x| x.full);
                    match if full { self.edit.add_link_replacing(link) } else { self.edit.add_link(link) } {
                        Ok(()) => {
                            linked = true;
                            if let Some((old_name, keyless)) = earlier_name {
                                let what = if keyless { "Your earlier copy without answer keys" } else { "Your earlier copy" };
                                let named = if old_name.is_empty() { String::new() } else { format!(", \"{old_name}\",") };
                                self.notice = Some(Notice::Info(format!("{what}{named} stays in My Banks but is no longer connected to the circle. You can delete it when you're ready.")));
                            }
                        }
                        Err(m) => {
                            why = Some(m.clone());
                            link_note = Some(m);
                        }
                    }
                }
                if let Some(x) = &add {
                    if let Some(job) = self.adds.iter_mut().rev().find(|j| j.cid == x.cid && j.bid == x.bid && j.own == x.own && in_flight(&j.state)) {
                        job.state = AddState::Done { bank_id, bank_name, linked, why: if linked || x.own { None } else { why } };
                    }
                } else if let (Some(b), Some(done)) = (&mut self.bank, copy_done) {
                    b.copy = CopyPanel::Done(done);
                }
                if let Err(m) = saved {
                    self.notice = Some(Notice::Error(format!("The questions were added, but the list of copies could not be saved ({m}). Updates will not be noticed for this one.")));
                }
                if let Some(m) = link_note {
                    self.notice = Some(Notice::Error(if m.starts_with("The bank was added") {
                        m
                    } else if m.contains("already connected") {
                        // (critic 63 B3) another bank of this computer keeps the link and its changes
                        format!("The bank was added, but {m}.")
                    } else {
                        format!("The bank was added, but the link to the circle bank could not be saved ({m}). Changes can't be sent from it.")
                    }));
                }
            }
            Err(m) => {
                let p = Problem { message: m, retry: false, retry_at: None, account: false };
                if let Some(x) = &add {
                    if let Some(job) = self.adds.iter_mut().rev().find(|j| j.cid == x.cid && j.bid == x.bid && j.own == x.own && in_flight(&j.state)) {
                        job.state = AddState::Failed(p);
                    }
                } else if let Some(b) = &mut self.bank {
                    b.copy = CopyPanel::Failed(p);
                }
            }
        }
        // (critic 71 minor 1) the adds not finished, as they are now, wherever this ran (also on the Shared page)
        self.note_unfinished();
    }

    /// Owner decision (b): "Add to My Banks" and "Add the ticked banks" finish after the Shared page is left. Called
    /// by the app's frame (`shared_ui::take_goto`) when the Shared page was not drawn. Does nothing at all unless an
    /// add the person asked for is waiting, under way, or landing: then it applies answers that came, starts the
    /// next add, and hands a finished copy to the banks' side ([`land_waiting`], from the browse window's frame,
    /// which has the banks). It reads nothing else (no list, no circle page). Returns true while a copy is under way
    /// or landing (adds waiting for a sign-in do not keep the app awake).
    ///
    /// (critic 75 minor 6) `frame`: the app's frame (egui pass) this runs in. The Shared page was not drawn in it, so a
    /// History left open there is not in front of the person (a bank's page that drew it in this frame says so): an
    /// answer to its restore or trim is said for its bank, on the Banks list for a bank with no copy here.
    pub fn tick_background_in(&mut self, now: Instant, snap: &AccountSnapshot, frame: u64) -> bool {
        self.edit.set_pass(frame);
        self.tick_background(now, snap)
    }

    /// [`tick_background_in`](Self::tick_background_in), the frame left as the last one given.
    pub fn tick_background(&mut self, now: Instant, snap: &AccountSnapshot) -> bool {
        self.finish_landed();
        self.note_unfinished();
        // (critic 77 minor 3) the Banks list's badges keep the account rule from its first frame: what a send whose answer
        // could not be read left in the links file is said (as sent) to the account that sent it only
        // (round 54: a confirmed pending account's circle work — copies, merge base reads — runs too)
        let circle_ready = snap.gate == Gate::Ready || snap.gate.confirmed_pending();
        if circle_ready {
            self.note_account(snap);
            self.edit.learn_account(snap);
        }
        // (critic 73 minor 5) a send, restore or trim on its way is read on any page (the Banks list says how it went for
        // a circle bank with no copy here), under the same account rule as the Shared page
        if self.edit.acting() {
            self.note_account(snap);
            self.edit.frame(now, snap);
        }
        let pending = self.adds.iter().any(|a| in_flight(&a.state)) || self.ready.is_some() || landing_pending();
        if !pending {
            return false;
        }
        self.now = Some(now);
        self.note_account(snap);
        self.session = if circle_ready { snap.session.clone() } else { None };
        self.drain();
        // (critic 69 minor 4) a copy read for the account is put in only while it is signed in (or confirmed pending)
        let can_land = circle_ready && !self.account_out;
        if can_land
            && self.ready.is_some()
            && !landing_pending()
            && let Some(l) = self.prepare_landing()
        {
            LANDING.with(|s| *s.borrow_mut() = Some(l));
        }
        self.next_add();
        self.note_unfinished();
        // (critic 67 I1, I2) adds that wait for a sign-in keep no frames coming: only a copy under way or landing does
        self.running(JobKind::Copy) || (can_land && self.ready.is_some()) || landing_pending()
    }

    /// (critic 69 minor 3) A copy the browse window put into the banks gets its link now, wherever the Shared page is
    /// (also on a sign-in step): the banks are saved with it, so it is never left without its link for long.
    pub fn finish_landed(&mut self) {
        if let Some(l) = take_landed() {
            self.finish_landing(l);
        }
    }

    /// (critic 69 minor 3) Writes how many adds are not finished, when that changed (read first, once a start).
    fn note_unfinished(&mut self) {
        self.read_unfinished();
        let n = self.adds.iter().filter(|a| in_flight(&a.state)).count();
        if self.unfinished_noted != Some(n) {
            self.adds_log.write(n);
            self.unfinished_noted = Some(n);
        }
    }

    fn read_unfinished(&mut self) {
        if self.unfinished_noted.is_none() {
            let n = self.adds_log.read();
            self.unfinished_at_start = n;
            self.unfinished_noted = Some(n);
        }
    }

    /// The adds the last run did not finish (verSHion was closed first), for the Banks list.
    pub fn unfinished_at_start(&mut self) -> usize {
        // (read, then the file says this run's number: the start after this one does not say it again)
        self.note_unfinished();
        self.unfinished_at_start
    }

    pub fn dismiss_unfinished(&mut self) {
        self.unfinished_at_start = 0;
    }

    pub fn set_adds_log(&mut self, log: AddsLog) {
        self.adds_log = log;
        self.unfinished_noted = None;
        self.unfinished_at_start = 0;
    }

    /// (critic 67 I1, I2) The adds wait for a sign-in: the session ended during one, or a sign-out was seen.
    pub fn adds_need_sign_in(&self) -> bool {
        self.adds_out || self.account_out
    }

    /// "Sign in..." on the Banks list: the Account window opens, and the adds are tried again.
    pub fn sign_in_for_adds(&mut self) {
        self.adds_out = false;
        self.adds_sign_in = true;
    }

    pub fn take_adds_sign_in(&mut self) -> bool {
        std::mem::take(&mut self.adds_sign_in)
    }

    /// "Open it" on the Banks list (critic 68 minor 3): the page there opens this bank.
    pub fn ask_open_bank(&mut self, local_id: &str) {
        self.open_bank_asked = Some(local_id.to_string());
    }

    pub fn take_open_bank(&mut self) -> Option<String> {
        self.open_bank_asked.take()
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

// ---- putting a finished copy into the banks (owner decision b) ----------------------------------------------------------

/// A finished copy and what the model decided for it, waiting for the banks.
struct Landing {
    r: ReadyCopy,
    name: String,
    /// "Add a full copy": the bank whose link it takes, and whether that one was made from the viewers' form.
    earlier: Option<(String, bool)>,
    now_ms: i64,
    /// The account the link's role is for (see `bank_links::account_tag`).
    tag: Option<String>,
}

/// What putting it into the banks did, for the model.
struct Landed {
    add: Option<AddFor>,
    /// The new bank's id and name.
    applied: Result<(String, String), String>,
    link: Option<CircleLink>,
    link_note: Option<String>,
    saved: Result<(), String>,
    /// The earlier copy's name (for the notice of a full copy), and whether it had no answer keys.
    earlier_name: Option<(String, bool)>,
    copy_done: Option<CopyDone>,
    /// Why an added bank that should be linked is not, in a few words (critic 66 I4, I5).
    why: Option<String>,
}

thread_local! {
    /// A copy handed from the model to the banks' side while the Shared page is not drawn (UI thread only).
    static LANDING: std::cell::RefCell<Option<Landing>> = const { std::cell::RefCell::new(None) };
    static LANDED: std::cell::RefCell<Option<Landed>> = const { std::cell::RefCell::new(None) };
}

/// An add asked for and not finished: waiting, or under way. A copy handed to the banks lands on it in either state
/// (an add put back to waiting when its job was dropped keeps the copy already read, and is made once).
fn in_flight(s: &AddState) -> bool {
    matches!(s, AddState::Waiting | AddState::Working)
}

/// (critic 71 minor 5) An add not made because another account signed in before it was.
pub(super) fn stopped_by_switch(s: &AddState) -> bool {
    matches!(s, AddState::Failed(p) if p.message == ADD_OTHER_ACCOUNT)
}

fn landing_pending() -> bool {
    LANDING.with(|s| s.borrow().is_some()) || LANDED.with(|s| s.borrow().is_some())
}

fn take_landed() -> Option<Landed> {
    LANDED.with(|s| s.borrow_mut().take())
}

/// Drops a copy handed over and not yet put into the banks (another account signed in).
fn drop_landing() {
    LANDING.with(|s| *s.borrow_mut() = None);
}

/// The banks' half of a background add: puts a copy the model handed over into the banks (owner decision b). Called
/// every frame by the browse window, which has the banks, whatever the screen; returns true when the banks changed
/// (the app saves them). Does nothing when nothing was handed over.
pub fn land_waiting(banks: &mut Vec<QuestionBank>, browse: &mut super::browse::BrowseModel) -> bool {
    let Some(l) = LANDING.with(|s| s.borrow_mut().take()) else { return false };
    let landed = land(l, banks, browse);
    let changed = landed.applied.is_ok();
    LANDED.with(|s| *s.borrow_mut() = Some(landed));
    changed
}

/// Puts a finished copy into the banks and records it like an import; makes the link it should have (the model
/// saves it). Nothing here reads or writes the links.
fn land(l: Landing, banks: &mut Vec<QuestionBank>, browse: &mut super::browse::BrowseModel) -> Landed {
    let Landing { r, name, earlier, now_ms, tag } = l;
    let mut out = Landed { add: r.add.clone(), applied: Err(String::new()), link: None, link_note: None, saved: Ok(()), earlier_name: None, copy_done: None, why: None };
    // (critic 66 I5) a bank added again (after one that was not connected) gets a name of its own
    let name = if r.add.is_some() { free_name(banks, &name) } else { name };
    let a = match apply_import(banks, &Dest::New, &name, &r.imp) {
        Ok(a) => a,
        Err(m) => {
            out.applied = Err(m);
            return out;
        }
    };
    let own = r.add.as_ref().is_some_and(|x| x.own);
    // The credit line says where it came from: the circle, not the public pool.
    if let Some(b) = banks.iter_mut().find(|b| b.id == a.bank_id) {
        let it = &r.imp.head.item;
        b.details.description = if own {
            // (phase D1, Q7) "Make my own copy": where it came from and when; it is not linked.
            super::bank_links::own_copy_line(&r.circle.circle_name, &api::plain(&it.title), &super::browse_text::author(&it.author_name), it.version, now_ms)
        } else if r.add.is_some() {
            // (critic 64 polish 14) a linked bank's version is on its page, and changes: not written here
            format!("From the circle \"{}\": \"{}\" by {}.", r.circle.circle_name, api::plain(&it.title), super::browse_text::author(&it.author_name))
        } else {
            format!("From the circle \"{}\": \"{}\" by {}, version {}.", r.circle.circle_name, api::plain(&it.title), super::browse_text::author(&it.author_name), it.version)
        };
    }
    // (Q2) An added bank follows the circle for a viewer, and for a read without the answer keys.
    let (mode, viewers_form) = match (&r.add, &r.working) {
        (Some(x), Some(h)) => super::bank_links::add_mode(x.role, h.keys_hidden),
        _ => (super::circle_links::LinkMode::Edit, false),
    };
    let sendable = mode == super::circle_links::LinkMode::Edit && !viewers_form;
    if r.working.is_some() && sendable && r.imp.pictures_missing > 0 {
        // Sending from a copy that lost pictures would remove them from the circle for everyone.
        // (critic 64 I6) said plainly, with the way on
        out.link_note = Some(format!("The bank was added, but {} of {} pictures couldn't be fetched, so it is not linked to the circle (a send from it would remove those pictures for everyone). Delete it and press Add to My Banks again when the pictures can be fetched.", r.imp.pictures_missing, r.imp.pictures));
        out.why = Some(format!("{} of {} pictures couldn't be fetched", r.imp.pictures_missing, r.imp.pictures));
    } else if let Some(head) = &r.working {
        let base = r.imp.theirs.iter().map(|t| super::merge::base_of(&t.q, &t.qid, &t.fp, t.sha.clone())).collect();
        let mut link = CircleLink::from_head(&r.circle.circle_id, &r.circle.circle_name, head, &a.bank_id, base, r.imp.at_ms);
        if let Some(x) = &r.add {
            link.mode = mode;
            link.viewers_form = viewers_form;
            link.role_seen = Some(x.role.as_str().to_string());
            link.role_seen_at = Some(now_ms);
            link.role_seen_by = tag;
        }
        if let Some(b) = banks.iter().find(|b| b.id == a.bank_id) {
            link.mark_bank(b);
        }
        out.link = Some(link);
    }
    // (Q7) An own copy gets no update notices; (critic 66 I2) nor does a linked bank: its strip says where it stands,
    // and the Shared page's "Things I imported" would say nothing goes back to the circle. A copy that is not linked
    // (a plain copy, or one missing pictures) is recorded as before.
    out.saved = if own || out.link.is_some() { Ok(()) } else { browse.record_circle_copy(&r.imp, &a, r.circle.clone()) };
    out.earlier_name = earlier.filter(|e| e.0 != a.bank_id).map(|(old, keyless)| (banks.iter().find(|b| b.id == old).map(|b| api::plain(&b.name)).unwrap_or_default(), keyless));
    if r.add.is_none() {
        out.copy_done = Some(CopyDone { bank_name: a.bank_name.clone(), added: a.added, pictures: r.imp.pictures, pictures_missing: r.imp.pictures_missing, version: r.imp.head.item.version });
    }
    out.applied = Ok((a.bank_id, a.bank_name));
    out
}

/// `name`, or "name (2)", "name (3)" ... when My Banks already has a bank of that name (critic 66 I5).
fn free_name(banks: &[QuestionBank], name: &str) -> String {
    let taken = |n: &str| banks.iter().any(|b| b.name.trim() == n.trim());
    if name.trim().is_empty() || !taken(name) {
        return name.to_string();
    }
    (2..1000).map(|k| format!("{name} ({k})")).find(|n| !taken(n)).unwrap_or_else(|| name.to_string())
}

/// "Deleted teacher" stays "deleted teacher"; a name is cleaned.
pub fn deleted_teacher(name: &str) -> String {
    let n = api::plain(name);
    if n.is_empty() { "A teacher".into() } else { n }
}
