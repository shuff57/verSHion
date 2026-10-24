//! "My shared items": what the pool holds of mine, what state each item is in, and what I may do
//! about it. No egui: tests drive every path with a scripted session.
//!
//! # Screens
//!
//! ```text
//! Gate --(signed in and approved)--> List --open--> Detail (+ a panel: confirm delete | appeal | counter-notice | sent)
//!                                      \--------------> Account (download my data, deleting the account, legal pages)
//! ```
//!
//! Rules this module keeps:
//!
//! - A closed model does no I/O: `frame` returns at once. Nothing goes to the network until the
//!   window is open and the account is signed in and approved.
//! - What a person may do with an item is decided in ONE place, [`actions_for`], from the state the
//!   pool reports. The buttons are drawn from it and every action method checks it again.
//! - Server-supplied text is kept raw here and drawn (and worded in `mine_text`) through
//!   [`api::plain`]; it is never markup and never a link.
//! - A counter-notice carries a name, address and phone number. It is typed into a form kept only in
//!   memory, sent once, and cleared the moment it is sent (or the window closes). It is never logged
//!   (its `Debug` is blank) and never written to disk.
//! - An account export holds personal data and answer keys. It is kept in memory only until it is
//!   saved to the file the person picks, and its contents are never logged.
//! - A cancelled or superseded request is forgotten: its receiver is dropped, a late answer is
//!   ignored.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::mpsc::{Receiver, Sender, TryRecvError, channel};
use std::sync::Arc;
use std::time::Duration;

use serde_json::Value;
use web_time::Instant;

use super::api::{self, ApiError};
use super::browse::{Problem, now_ms};
use super::mine_text as text;
use super::model::{Notice, Runner};
use super::pool::{Ack, CounterNotice, CounterNoticeAck, ItemState, MineItem, MineList, Pool, PoolError, RefusalKind, RetryPolicy, Storage, Visibility, is_item_id};
use super::publish::{AccountSnapshot, Gate, LogIo, PublishedRecord, SharedSession};

const NOTE_MAX: usize = 500;
/// The most parts of an export that are fetched (the Worker makes one per 4 MB of stored text).
const MAX_PARTS: usize = 500;
/// The most items the list keeps in memory.
const MAX_ITEMS: usize = 5000;

// ---- what can be done ---------------------------------------------------------------------------------

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Action {
    MakePublic,
    MakePrivate,
    Delete,
    Undo,
    Republish,
    Appeal,
    CounterNotice,
}

impl Action {
    pub fn label(self) -> &'static str {
        match self {
            Action::MakePublic => "Make public",
            Action::MakePrivate => "Make private",
            Action::Delete => "Delete...",
            Action::Undo => "Undo delete",
            Action::Republish => "Share again...",
            Action::Appeal => "Appeal...",
            Action::CounterNotice => "Send a counter-notice...",
        }
    }
}

/// The actions the pool lets an author take on an item in its state. This is the whole matrix:
///
/// | state                       | actions                                              |
/// |-----------------------------|------------------------------------------------------|
/// | live, public                | make private, delete, share again                    |
/// | live, private               | make public, delete, share again                     |
/// | deleted, inside the window  | undo delete                                          |
/// | deleted, window over        | none                                                 |
/// | taken down                  | counter-notice, only while the pool offers it         |
/// | copyright notice pending    | counter-notice, until the 30 days end and unsent     |
/// | removed (privacy)           | appeal, while offered and inside the 14 days          |
/// | removed (admin)             | appeal, only when the pool offers it                  |
/// | needs picture again         | share again, delete                                  |
/// | answer key may be visible   | share again, delete                                  |
/// | held for review             | appeal when offered; share again when a hold names questions |
/// | anything else               | none                                                 |
pub fn actions_for(it: &MineItem, now: i64) -> Vec<Action> {
    let appeal_open = it.appeal.as_deref() == Some("available") && it.appeal_until.is_none_or(|u| u <= 0 || u > now);
    match it.state {
        ItemState::Live => vec![if it.is_unlisted() { Action::MakePublic } else { Action::MakePrivate }, Action::Delete, Action::Republish],
        ItemState::Deleted => match it.undo_until {
            Some(u) if u > now => vec![Action::Undo],
            _ => vec![],
        },
        ItemState::TakenDown => {
            if it.counter_notice.as_deref() == Some("available") {
                vec![Action::CounterNotice]
            } else {
                vec![]
            }
        }
        ItemState::TakedownPending => {
            let open = !it.counter_notice_sent && it.counter_notice_by.is_some_and(|b| b > now) && it.action_id.is_some();
            if open { vec![Action::CounterNotice] } else { vec![] }
        }
        ItemState::RemovedPrivacy | ItemState::Removed => {
            if appeal_open {
                vec![Action::Appeal]
            } else {
                vec![]
            }
        }
        ItemState::NeedsReupload | ItemState::KeyLeakReview => vec![Action::Republish, Action::Delete],
        ItemState::HeldForReview => {
            let mut v = Vec::new();
            if appeal_open {
                v.push(Action::Appeal);
            }
            if it.hold_class.is_some() {
                v.push(Action::Republish);
            }
            v
        }
        ItemState::Other => vec![],
    }
}

// ---- the counter-notice form ------------------------------------------------------------------------------

/// What is wrong with a field.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Issue {
    Empty,
    TooLong(usize),
    /// The signature is not the typed full name.
    NotYourName,
    NotTicked,
}

/// The form. Its `Debug` shows nothing: it holds a name, address and phone number.
#[derive(Clone, Default, PartialEq, Eq)]
pub struct CounterForm {
    pub signature: String,
    pub material_desc: String,
    pub name: String,
    pub address: String,
    pub phone: String,
    pub perjury_stmt: bool,
    pub consent_jurisdiction: bool,
    pub accept_service: bool,
}

impl std::fmt::Debug for CounterForm {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("CounterForm(..)")
    }
}

fn same_name(a: &str, b: &str) -> bool {
    let norm = |s: &str| s.split_whitespace().collect::<Vec<_>>().join(" ").to_lowercase();
    !norm(a).is_empty() && norm(a) == norm(b)
}

impl CounterForm {
    /// Every field's problem, in the order the form shows them. Empty: the form can be sent.
    pub fn issues(&self) -> Vec<(&'static str, Issue)> {
        let mut v = Vec::new();
        let mut text = |wire: &'static str, s: &str, max: usize| {
            let n = s.trim().chars().count();
            if n == 0 {
                v.push((wire, Issue::Empty));
            } else if n > max {
                v.push((wire, Issue::TooLong(max)));
            }
        };
        text("name", &self.name, 200);
        text("address", &self.address, 500);
        text("phone", &self.phone, 50);
        text("material_desc", &self.material_desc, 2000);
        text("signature", &self.signature, 200);
        if !self.signature.trim().is_empty() && !same_name(&self.signature, &self.name) && !v.iter().any(|(f, _)| *f == "signature") {
            v.push(("signature", Issue::NotYourName));
        }
        for (wire, on) in [("perjury_stmt", self.perjury_stmt), ("consent_jurisdiction", self.consent_jurisdiction), ("accept_service", self.accept_service)] {
            if !on {
                v.push((wire, Issue::NotTicked));
            }
        }
        v
    }

    pub fn ready(&self) -> bool {
        self.issues().is_empty()
    }

    pub fn clear(&mut self) {
        *self = CounterForm::default();
    }

    fn to_wire(&self) -> CounterNotice {
        CounterNotice {
            signature: self.signature.clone(),
            material_desc: self.material_desc.clone(),
            name: self.name.clone(),
            address: self.address.clone(),
            phone: self.phone.clone(),
            perjury_stmt: self.perjury_stmt,
            consent_jurisdiction: self.consent_jurisdiction,
            accept_service: self.accept_service,
        }
    }
}

/// A sentence for one field's problem.
pub fn issue_text(field: &str, issue: Issue) -> String {
    let name = text::field_name(field);
    match issue {
        Issue::Empty => format!("Fill in the {name}."),
        Issue::TooLong(max) => format!("The {name} is too long (at most {max} characters)."),
        Issue::NotYourName => "Type your full name again as the signature, exactly as above.".into(),
        Issue::NotTicked => format!("Tick {name} to send the counter-notice."),
    }
}

// ---- screens and panels ---------------------------------------------------------------------------------------

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Screen {
    /// Waiting for a signed-in, approved account.
    Gate,
    List,
    Detail,
    /// Download my data, deleting the account, the legal pages.
    Account,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Panel {
    None,
    ConfirmDelete,
    Appeal,
    Counter,
    /// A counter-notice went out; nothing of it is kept.
    CounterSent(CounterNoticeAck),
}

/// What "Share again..." needs from the app.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Republish {
    /// Start the publish flow for this source key (`bank:<id>`); it finds `share-published.json`'s
    /// record and republishes over this item.
    Source(String),
    /// Shared from an exam: the exam's File menu does it.
    Exam,
    /// No record on this device: shared elsewhere, or before records existed.
    Unknown,
}

// ---- the list ---------------------------------------------------------------------------------------------------

#[derive(Clone, Debug, Default)]
pub struct ListState {
    pub items: Vec<MineItem>,
    pub storage: Storage,
    pub next: Option<String>,
    pub loaded: bool,
    pub problem: Option<Problem>,
}

// ---- the export ---------------------------------------------------------------------------------------------------

/// The pieces of an export, in memory. `Debug` shows sizes only.
#[derive(Clone, Default)]
pub struct Bundle {
    files: Vec<(String, Vec<u8>)>,
}

impl std::fmt::Debug for Bundle {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Bundle({} files, {} bytes)", self.files.len(), self.size())
    }
}

impl Bundle {
    pub fn size(&self) -> usize {
        self.files.iter().map(|(_, b)| b.len()).sum()
    }
    pub fn parts(&self) -> usize {
        self.files.len().saturating_sub(1)
    }
}

pub const EXPORT_NAME: &str = "vershion-pool-export.json";

#[derive(Clone, Debug, PartialEq)]
pub enum ExportState {
    Idle,
    /// `total` is 0 until the first answer says whether there are parts.
    Running { done: usize, total: usize },
    /// Fetched, not saved yet.
    Ready { size: usize, parts: usize },
    Saved { place: String, parts: usize },
    Failed(Problem),
}

/// Where a saved export goes: the platform's save dialog, or a test's memory.
pub trait ExportSink: Send + Sync {
    /// The file the person picked, or `None` when they cancelled.
    fn choose(&self, default_name: &str) -> Option<PathBuf>;
    fn write(&self, path: &Path, bytes: &[u8]) -> std::io::Result<()>;
}

pub struct PlatformSink;

impl ExportSink for PlatformSink {
    fn choose(&self, default_name: &str) -> Option<PathBuf> {
        crate::platform::save_path(default_name, "JSON", &["json"])
    }
    fn write(&self, path: &Path, bytes: &[u8]) -> std::io::Result<()> {
        crate::platform::write(path, bytes)
    }
}

pub type SharedSink = Arc<dyn ExportSink>;

/// The name of part `n` beside the main file: `vershion-pool-export.json` gives
/// `vershion-pool-export-part-2.json`.
fn part_path(main: &Path, n: usize) -> PathBuf {
    let stem = main.file_stem().and_then(|s| s.to_str()).unwrap_or("vershion-pool-export");
    main.with_file_name(format!("{stem}-part-{n}.json"))
}

// ---- jobs -----------------------------------------------------------------------------------------------------

#[derive(Clone)]
enum Work {
    List(Option<String>),
    Visibility(String, Visibility),
    Delete(String),
    Undo(String),
    Appeal(String, String),
    Counter(String, Box<CounterNotice>),
    Export,
}

impl std::fmt::Debug for Work {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Work({:?})", self.kind())
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum JobKind {
    List,
    /// Any change to one item: only one runs at a time.
    Act,
    Export,
}

impl Work {
    fn kind(&self) -> JobKind {
        match self {
            Work::List(_) => JobKind::List,
            Work::Export => JobKind::Export,
            _ => JobKind::Act,
        }
    }
}

enum JobMsg {
    List(bool, Result<MineList, PoolError>),
    Visibility(String, Visibility, Result<Visibility, PoolError>),
    Delete(String, Result<Option<i64>, PoolError>),
    Undo(String, Result<(), PoolError>),
    Appeal(String, Result<Ack, PoolError>),
    Counter(String, Result<CounterNoticeAck, PoolError>),
    Progress(usize, usize),
    Export(Result<Bundle, PoolError>),
}

impl JobMsg {
    fn kind(&self) -> JobKind {
        match self {
            JobMsg::List(..) => JobKind::List,
            JobMsg::Progress(..) | JobMsg::Export(_) => JobKind::Export,
            _ => JobKind::Act,
        }
    }
}

struct Job {
    kind: JobKind,
    rx: Receiver<JobMsg>,
}

async fn run_work_async(session: SharedSession, retry: RetryPolicy, work: Work, tx: Sender<JobMsg>, repaint: Option<Arc<dyn Fn() + Send + Sync>>) {
    let pool = Pool::new(&*session).with_retry(retry);
    let poke = || {
        if let Some(r) = &repaint {
            r();
        }
    };
    let msg = match work {
        Work::List(after) => JobMsg::List(after.is_none(), pool.mine_page(after.as_deref()).await),
        Work::Visibility(id, v) => {
            let r = pool.set_visibility(&id, v).await;
            JobMsg::Visibility(id, v, r)
        }
        Work::Delete(id) => {
            let r = pool.delete_item(&id).await;
            JobMsg::Delete(id, r)
        }
        Work::Undo(id) => {
            let r = pool.undelete_item(&id).await;
            JobMsg::Undo(id, r)
        }
        Work::Appeal(id, note) => {
            let r = pool.appeal(&id, &note).await;
            JobMsg::Appeal(id, r)
        }
        Work::Counter(id, n) => {
            let r = pool.counter_notice(&id, &n).await;
            JobMsg::Counter(id, r)
        }
        Work::Export => JobMsg::Export(fetch_export(&pool, &tx, &poke).await),
    };
    let _ = tx.send(msg);
    poke();
}

#[cfg(not(target_arch = "wasm32"))]
fn run_work(session: SharedSession, retry: RetryPolicy, work: Work, tx: Sender<JobMsg>, repaint: Option<Arc<dyn Fn() + Send + Sync>>) {
    super::transport::block_on(run_work_async(session, retry, work, tx, repaint));
}

/// The first answer, then (when the pool made a manifest) every part, telling the screen how far
/// it is. Stops as soon as the screen stopped listening.
async fn fetch_export(pool: &Pool<'_>, tx: &Sender<JobMsg>, poke: &dyn Fn()) -> Result<Bundle, PoolError> {
    let first = pool.export_account().await?;
    let mut bundle = Bundle { files: vec![(EXPORT_NAME.to_string(), first)] };
    let too_large;
    let count;
    {
        // Only the two flags are read; nothing of the contents is kept or shown.
        let v: Value = serde_json::from_slice(&bundle.files[0].1).map_err(|e| PoolError::Api(ApiError::BadReply(e.to_string())))?;
        too_large = v.get("export_too_large").and_then(Value::as_bool).unwrap_or(false);
        count = v.pointer("/manifest/parts").and_then(Value::as_array).map_or(0, Vec::len);
    }
    if !too_large {
        return Ok(bundle);
    }
    if count == 0 || count > MAX_PARTS {
        return Err(PoolError::Api(ApiError::BadReply("the export manifest is not usable".into())));
    }
    for n in 1..=count {
        if tx.send(JobMsg::Progress(n - 1, count)).is_err() {
            return Err(PoolError::Api(ApiError::BadReply("cancelled".into())));
        }
        poke();
        let bytes = pool.export_part(n as u32).await?;
        bundle.files.push((format!("part-{n}"), bytes));
    }
    Ok(bundle)
}

// ---- the model -----------------------------------------------------------------------------------------------------

pub struct MineModel {
    pub open: bool,
    pub screen: Screen,
    pub gate: Gate,
    pub notice: Option<Notice>,
    pub list: ListState,
    pub selected: Option<String>,
    pub panel: Panel,
    /// The last action's failure, shown on the detail screen.
    pub problem: Option<Problem>,
    pub appeal_note: String,
    pub counter: CounterForm,
    pub export: ExportState,
    /// Where the legal pages are published (`ShareSettings::legal_base`, NOT the API address); empty: no links.
    pub base_url: String,
    /// What each appeal I sent this session was answered with (one appeal at a time).
    pub appealed: HashMap<String, String>,
    open_account: bool,
    asked_account: bool,
    want_delete_page: bool,
    resume: Screen,
    session: Option<SharedSession>,
    runner: Runner,
    retry: RetryPolicy,
    jobs: Vec<Job>,
    published: LogIo,
    records: Option<Vec<PublishedRecord>>,
    republish: Option<String>,
    export_data: Option<Bundle>,
    sink: SharedSink,
    clock: Option<i64>,
    now: Option<Instant>,
    repaint: Option<Arc<dyn Fn() + Send + Sync>>,
}

impl Default for MineModel {
    fn default() -> Self {
        MineModel::new(Runner::Background, LogIo::Store, Arc::new(PlatformSink))
    }
}

impl MineModel {
    /// Does no I/O: nothing is read or sent until the window is opened.
    pub fn new(runner: Runner, published: LogIo, sink: SharedSink) -> MineModel {
        MineModel {
            open: false,
            screen: Screen::Gate,
            gate: Gate::Checking,
            notice: None,
            list: ListState::default(),
            selected: None,
            panel: Panel::None,
            problem: None,
            appeal_note: String::new(),
            counter: CounterForm::default(),
            export: ExportState::Idle,
            base_url: String::new(),
            appealed: HashMap::new(),
            open_account: false,
            asked_account: false,
            want_delete_page: false,
            resume: Screen::List,
            session: None,
            runner,
            retry: RetryPolicy::default(),
            jobs: Vec::new(),
            published,
            records: None,
            republish: None,
            export_data: None,
            sink,
            clock: None,
            now: None,
            repaint: None,
        }
    }

    pub fn with_retry(mut self, retry: RetryPolicy) -> MineModel {
        self.retry = retry;
        self
    }

    pub fn set_repaint(&mut self, f: Arc<dyn Fn() + Send + Sync>) {
        self.repaint = Some(f);
    }

    /// Fixes "now" (milliseconds) for tests.
    pub fn set_clock(&mut self, ms: i64) {
        self.clock = Some(ms);
    }

    pub fn now_ms(&self) -> i64 {
        self.clock.unwrap_or_else(now_ms)
    }

    pub fn busy(&self) -> bool {
        !self.jobs.is_empty()
    }

    #[cfg(test)]
    pub(super) fn jobs_idle(&self) -> bool {
        self.jobs.is_empty()
    }

    #[cfg(test)]
    pub(super) fn session_for_test(&mut self, s: &Arc<dyn super::pool::PoolSession + Send + Sync>) {
        self.session = Some(s.clone());
    }

    /// A change to an item is on its way: the window should not be closed in the middle of it.
    pub fn busy_action(&self) -> bool {
        self.running(JobKind::Act)
    }

    fn running(&self, k: JobKind) -> bool {
        self.jobs.iter().any(|j| j.kind == k)
    }

    // ---- opening --------------------------------------------------------------------------------------------

    fn open_window(&mut self, screen: Screen) {
        self.cancel_all();
        self.open = true;
        self.notice = None;
        self.problem = None;
        self.panel = Panel::None;
        self.selected = None;
        self.list = ListState::default();
        self.records = None;
        self.counter.clear();
        self.appeal_note.clear();
        self.export = ExportState::Idle;
        self.export_data = None;
        self.open_account = false;
        self.asked_account = false;
        self.want_delete_page = false;
        self.resume = screen;
        self.screen = Screen::Gate;
    }

    /// "My shared items".
    pub fn open_list(&mut self) {
        self.open_window(Screen::List);
    }

    /// Straight to the page with the data download and the legal pages.
    pub fn open_account_page(&mut self) {
        self.open_window(Screen::Account);
    }

    /// Opens the list at one item (the Done screen of the publish flow).
    pub fn open_item(&mut self, id: &str) {
        self.open_window(Screen::List);
        if is_item_id(id) {
            self.selected = Some(id.to_string());
        }
    }

    pub fn close(&mut self) {
        self.open = false;
        self.cancel_all();
        self.open_account = false;
        // Nothing typed into the counter-notice stays behind, and an unsaved export is dropped.
        self.counter.clear();
        self.appeal_note.clear();
        self.export_data = None;
        self.export = ExportState::Idle;
    }

    fn cancel_all(&mut self) {
        // Dropping the receivers makes late answers go nowhere.
        self.jobs.clear();
    }

    pub fn wants_account_check(&self) -> bool {
        self.open && self.screen == Screen::Gate && self.gate == Gate::Checking && !self.asked_account
    }

    pub fn account_check_asked(&mut self) {
        self.asked_account = true;
    }

    pub fn take_open_account(&mut self) -> bool {
        std::mem::take(&mut self.open_account)
    }

    pub fn request_open_account(&mut self) {
        self.open_account = true;
    }

    /// "Delete my account": the app shows the Account window's delete page.
    pub fn take_delete_page(&mut self) -> bool {
        std::mem::take(&mut self.want_delete_page)
    }

    pub fn request_delete_page(&mut self) {
        self.want_delete_page = true;
    }

    // ---- frame -----------------------------------------------------------------------------------------------

    /// Applies finished jobs and the account's state, and loads the first page of the list. Does
    /// nothing when closed.
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
        let mut gate = snap.gate.clone();
        if gate == Gate::Checking && self.asked_account && !snap.account_open {
            gate = Gate::SignedOut;
        }
        self.gate = gate.clone();
        if self.screen == Screen::Gate {
            if gate == Gate::Ready {
                self.screen = self.resume;
            }
        } else if matches!(gate, Gate::SignedOut | Gate::NotApproved(_) | Gate::NotSetUp | Gate::NoServer) {
            self.resume = self.screen;
            self.screen = Screen::Gate;
        }
    }

    fn auto(&mut self) {
        if self.screen == Screen::Gate || self.session.is_none() {
            return;
        }
        if !self.list.loaded && self.list.problem.is_none() && !self.running(JobKind::List) {
            self.launch(Work::List(None));
        }
        // Opened at one item: show it once the list has it.
        if self.list.loaded
            && let Some(id) = self.selected.clone()
            && self.screen == Screen::List
        {
            if self.list.items.iter().any(|i| i.id == id) {
                self.screen = Screen::Detail;
            } else {
                self.selected = None;
            }
        }
    }

    // ---- errors ----------------------------------------------------------------------------------------------

    /// Turns an error into a `Problem`, or sends the person to the Account window (and returns
    /// `None`) when the sign-in ended.
    fn problem_of(&mut self, e: &PoolError) -> Option<Problem> {
        let now = self.now.unwrap_or_else(Instant::now);
        let mut p = Problem { message: e.message(), retry: false, retry_at: None, account: false };
        match e {
            PoolError::Api(ApiError::SignedOut) => {
                self.gate = Gate::SignedOut;
                if self.screen != Screen::Gate {
                    self.resume = if self.screen == Screen::Detail { Screen::List } else { self.screen };
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
                    RefusalKind::NotApproved { .. } | RefusalKind::InviteRequired => p.account = true,
                    RefusalKind::DailyLimit { metric } => {
                        p.retry = true;
                        p.retry_at = wait;
                        if metric.as_deref() == Some("exports") {
                            p.message = "You have downloaded your data three times today, which is the most the pool allows. Try again tomorrow.".into();
                        } else if metric.as_deref() == Some("appeals") {
                            p.message = "You have sent as many appeals as the pool allows for today. Try again tomorrow.".into();
                        }
                    }
                    RefusalKind::RateLimited | RefusalKind::TooManyOpenReads | RefusalKind::TooManyUploads | RefusalKind::Busy => {
                        p.retry = true;
                        p.retry_at = wait;
                    }
                    RefusalKind::NotFound => p.message = "That item is not available to you any more. It may have been removed for good.".into(),
                    RefusalKind::ModerationState { message } => {
                        p.message = message.as_deref().map(api::plain).filter(|m| !m.is_empty()).unwrap_or_else(|| "The item is not in a state where that can be done.".into());
                    }
                    RefusalKind::CounterNoticeIncomplete { missing } => {
                        let names: Vec<&str> = missing.iter().map(|m| text::field_name(m)).collect();
                        p.message = if names.is_empty() {
                            "The counter-notice is incomplete. Nothing was sent.".into()
                        } else {
                            format!("The counter-notice is missing: {}. Nothing was sent.", names.join(", "))
                        };
                    }
                    RefusalKind::InvalidRequest { field } => {
                        p.message = match field.as_deref() {
                            Some("note") => "Write a note of 1 to 500 characters.".into(),
                            Some(f) => format!("The pool did not accept the {}. Nothing was changed.", text::field_name(f)),
                            None => "The pool did not accept that. Nothing was changed.".into(),
                        }
                    }
                    RefusalKind::Other => match (r.detail.status, r.detail.code.as_str()) {
                        (403, "reauth_required") => {
                            p.message = "Sign in again to download your data: an export needs a sign-in from the last 24 hours.".into();
                            p.account = true;
                        }
                        (410, _) => p.message = "That item has been removed for good.".into(),
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

    pub fn retry_wait(&self, p: &Problem) -> Option<Duration> {
        let (at, now) = (p.retry_at?, self.now?);
        (at > now).then(|| at - now)
    }

    /// An item that changed or went away under the person: look at the list again.
    fn stale(e: &PoolError) -> bool {
        match e {
            PoolError::Refused(r) => matches!(r.kind, RefusalKind::NotFound | RefusalKind::ModerationState { .. } | RefusalKind::ItemRemoved { .. }) || r.detail.status == 410,
            _ => false,
        }
    }

    // ---- launching -------------------------------------------------------------------------------------------

    fn launch(&mut self, work: Work) -> bool {
        let Some(session) = self.session.clone() else {
            self.gate = Gate::Checking;
            return false;
        };
        let kind = work.kind();
        self.jobs.retain(|j| j.kind != kind);
        let (tx, rx) = channel::<JobMsg>();
        let (retry, repaint) = (self.retry, self.repaint.clone());
        match self.runner {
            #[cfg(not(target_arch = "wasm32"))]
            Runner::Inline => run_work(session, retry, work, tx, repaint),
            #[cfg(target_arch = "wasm32")]
            Runner::Inline => wasm_bindgen_futures::spawn_local(async move { run_work_async(session, retry, work, tx, repaint).await }),
            #[cfg(not(target_arch = "wasm32"))]
            Runner::Background => {
                std::thread::spawn(move || run_work(session, retry, work, tx, repaint));
            }
            #[cfg(target_arch = "wasm32")]
            Runner::Background => wasm_bindgen_futures::spawn_local(async move { run_work_async(session, retry, work, tx, repaint).await }),
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
            if !matches!(m, JobMsg::Progress(..)) {
                self.jobs.retain(|j| j.kind != kind);
                lost.retain(|k| *k != kind);
            }
            match m {
                JobMsg::List(first, r) => self.on_list(first, r),
                JobMsg::Visibility(id, v, r) => self.on_visibility(id, v, r),
                JobMsg::Delete(id, r) => self.on_delete(id, r),
                JobMsg::Undo(id, r) => self.on_undo(id, r),
                JobMsg::Appeal(id, r) => self.on_appeal(id, r),
                JobMsg::Counter(id, r) => self.on_counter(id, r),
                JobMsg::Progress(done, total) => self.export = ExportState::Running { done, total },
                JobMsg::Export(r) => self.on_export(r),
            }
        }
        for kind in lost {
            self.jobs.retain(|j| j.kind != kind);
            let p = Problem { message: "The request stopped unexpectedly.".into(), retry: true, retry_at: None, account: false };
            match kind {
                JobKind::List => self.list.problem = Some(p),
                JobKind::Act => self.problem = Some(p),
                JobKind::Export => self.export = ExportState::Failed(p),
            }
        }
    }

    // ---- the list ---------------------------------------------------------------------------------------------

    /// Fetches the list again from the first page (items stay shown until the answer arrives).
    pub fn reload(&mut self) {
        self.list.problem = None;
        self.launch(Work::List(None));
    }

    pub fn can_load_more(&self) -> bool {
        self.list.next.is_some() && !self.running(JobKind::List) && self.list.items.len() < MAX_ITEMS
    }

    pub fn load_more(&mut self) {
        if let Some(c) = self.list.next.clone().filter(|_| self.can_load_more()) {
            self.launch(Work::List(Some(c)));
        }
    }

    fn on_list(&mut self, first: bool, r: Result<MineList, PoolError>) {
        match r {
            Ok(page) => {
                self.list.problem = None;
                self.list.loaded = true;
                self.list.storage = page.storage;
                self.list.next = page.next_cursor.filter(|c| !c.is_empty());
                let mut items: Vec<MineItem> = page.items.into_iter().filter(|i| is_item_id(&i.id)).collect();
                if first {
                    self.list.items = items;
                } else {
                    // A page is newest first; what is already shown is not shown twice.
                    items.retain(|i| !self.list.items.iter().any(|o| o.id == i.id));
                    self.list.items.extend(items);
                }
                self.list.items.truncate(MAX_ITEMS);
                if let Some(id) = &self.selected
                    && !self.list.items.iter().any(|i| &i.id == id)
                    && self.screen == Screen::Detail
                    && !self.list.items.is_empty()
                    && self.list.next.is_none()
                {
                    self.back();
                }
            }
            Err(e) => {
                if let Some(p) = self.problem_of(&e) {
                    self.list.problem = Some(p);
                }
            }
        }
    }

    pub fn item(&self, id: &str) -> Option<&MineItem> {
        self.list.items.iter().find(|i| i.id == id)
    }

    pub fn selected_item(&self) -> Option<&MineItem> {
        self.selected.as_deref().and_then(|id| self.item(id))
    }

    fn item_mut(&mut self, id: &str) -> Option<&mut MineItem> {
        self.list.items.iter_mut().find(|i| i.id == id)
    }

    pub fn select(&mut self, id: &str) {
        if self.item(id).is_some() {
            self.selected = Some(id.to_string());
            self.screen = Screen::Detail;
            self.reset_detail();
        }
    }

    pub fn back(&mut self) {
        self.selected = None;
        self.screen = Screen::List;
        self.reset_detail();
    }

    fn reset_detail(&mut self) {
        self.panel = Panel::None;
        self.problem = None;
        self.notice = None;
        self.counter.clear();
        self.appeal_note.clear();
    }

    pub fn open_account_screen(&mut self) {
        self.screen = Screen::Account;
        self.reset_detail();
    }

    pub fn close_account_screen(&mut self) {
        self.screen = Screen::List;
    }

    // ---- what is allowed -------------------------------------------------------------------------------------

    /// The actions open to the selected item right now. An appeal already sent this session is not
    /// offered again.
    pub fn actions(&self) -> Vec<Action> {
        let Some(it) = self.selected_item() else { return vec![] };
        let mut v = actions_for(it, self.now_ms());
        if self.appealed.contains_key(&it.id) {
            v.retain(|a| *a != Action::Appeal);
        }
        v
    }

    fn allowed(&self, a: Action) -> bool {
        !self.running(JobKind::Act) && self.actions().contains(&a)
    }

    /// The server's answer to an appeal sent this session, if any.
    pub fn appeal_answer(&self, id: &str) -> Option<&str> {
        self.appealed.get(id).map(String::as_str)
    }

    fn refuse(&mut self, what: &str) {
        self.notice = Some(Notice::Error(format!("{what} can not be done to this item right now.")));
    }

    // ---- visibility, delete, undo ------------------------------------------------------------------------

    pub fn set_visibility(&mut self, to: Visibility) {
        let a = if to == Visibility::Public { Action::MakePublic } else { Action::MakePrivate };
        if !self.allowed(a) {
            return self.refuse("Changing who can see it");
        }
        let Some(id) = self.selected.clone() else { return };
        self.problem = None;
        self.notice = None;
        self.launch(Work::Visibility(id, to));
    }

    fn on_visibility(&mut self, id: String, want: Visibility, r: Result<Visibility, PoolError>) {
        match r {
            Ok(v) => {
                if let Some(it) = self.item_mut(&id) {
                    it.visibility = v.as_str().to_string();
                }
                let _ = want;
                self.notice = Some(Notice::Info(
                    if v == Visibility::Public { "Now public: other approved teachers can find it." } else { "Now private: only you can see it." }.into(),
                ));
            }
            Err(e) => self.act_failed(&e),
        }
    }

    pub fn ask_delete(&mut self) {
        if self.allowed(Action::Delete) {
            self.panel = Panel::ConfirmDelete;
            self.problem = None;
        } else {
            self.refuse("Deleting");
        }
    }

    pub fn cancel_panel(&mut self) {
        if !self.running(JobKind::Act) {
            self.panel = Panel::None;
            self.problem = None;
            self.counter.clear();
            self.appeal_note.clear();
        }
    }

    /// The confirmed delete.
    pub fn confirm_delete(&mut self) {
        if self.panel != Panel::ConfirmDelete || !self.allowed(Action::Delete) {
            return self.refuse("Deleting");
        }
        let Some(id) = self.selected.clone() else { return };
        self.problem = None;
        self.launch(Work::Delete(id));
    }

    fn on_delete(&mut self, id: String, r: Result<Option<i64>, PoolError>) {
        match r {
            Ok(until) => {
                let fallback = self.now_ms() + 14 * 86_400_000;
                if let Some(it) = self.item_mut(&id) {
                    it.state = ItemState::Deleted;
                    it.undo_until = Some(until.unwrap_or(fallback));
                }
                self.panel = Panel::None;
                self.notice = Some(Notice::Info(format!(
                    "Deleted. It is hidden from everyone. You can undo this until {}.",
                    text::date(until.unwrap_or(fallback))
                )));
            }
            Err(e) => self.act_failed(&e),
        }
    }

    pub fn undo_delete(&mut self) {
        if !self.allowed(Action::Undo) {
            return self.refuse("Undoing the delete");
        }
        let Some(id) = self.selected.clone() else { return };
        self.problem = None;
        self.launch(Work::Undo(id));
    }

    fn on_undo(&mut self, id: String, r: Result<(), PoolError>) {
        match r {
            Ok(()) => {
                // The pool decides what it is now (it may be held); ask.
                if let Some(it) = self.item_mut(&id) {
                    it.state = ItemState::Live;
                    it.undo_until = None;
                }
                self.notice = Some(Notice::Info("Restored. The list is being refreshed.".into()));
                self.reload();
            }
            Err(e) => self.act_failed(&e),
        }
    }

    // ---- appeals --------------------------------------------------------------------------------------------

    pub fn start_appeal(&mut self) {
        if self.allowed(Action::Appeal) {
            self.panel = Panel::Appeal;
            self.problem = None;
        } else {
            self.refuse("Appealing");
        }
    }

    pub fn appeal_ready(&self) -> bool {
        let n = self.appeal_note.trim().chars().count();
        (1..=NOTE_MAX).contains(&n)
    }

    pub fn send_appeal(&mut self) {
        if self.panel != Panel::Appeal || !self.allowed(Action::Appeal) {
            return self.refuse("Appealing");
        }
        if !self.appeal_ready() {
            self.problem = Some(Problem { message: "Write a note of 1 to 500 characters.".into(), retry: false, retry_at: None, account: false });
            return;
        }
        let Some(id) = self.selected.clone() else { return };
        self.problem = None;
        let note = self.appeal_note.trim().to_string();
        self.launch(Work::Appeal(id, note));
    }

    fn on_appeal(&mut self, id: String, r: Result<Ack, PoolError>) {
        match r {
            Ok(ack) => {
                let msg = ack.message.as_deref().map(api::plain).filter(|m| !m.is_empty()).unwrap_or_else(|| "The admin will review your item.".into());
                self.appealed.insert(id, msg.clone());
                self.panel = Panel::None;
                self.appeal_note.clear();
                self.notice = Some(Notice::Info(msg));
            }
            Err(e) => {
                // "Decided" and "nothing to appeal" end the offer for this item.
                if let PoolError::Refused(r) = &e
                    && matches!(r.detail.code.as_str(), "decided" | "nothing_to_appeal")
                    && let Some(id) = self.selected.clone()
                {
                    let m = r.message();
                    self.appealed.insert(id, api::plain(&m));
                    self.panel = Panel::None;
                }
                self.act_failed(&e);
            }
        }
    }

    // ---- counter-notice ---------------------------------------------------------------------------------------

    pub fn start_counter(&mut self) {
        if self.allowed(Action::CounterNotice) {
            self.panel = Panel::Counter;
            self.problem = None;
            // No autofill: every field starts empty.
            self.counter.clear();
        } else {
            self.refuse("A counter-notice");
        }
    }

    /// The first thing wrong with the form, if anything (the screen lists them all via `issues`).
    pub fn counter_issues(&self) -> Vec<(&'static str, Issue)> {
        self.counter.issues()
    }

    /// Sends it, only when every field is filled and every statement is ticked.
    pub fn send_counter(&mut self) {
        if self.panel != Panel::Counter || !self.allowed(Action::CounterNotice) {
            return self.refuse("A counter-notice");
        }
        if !self.counter.ready() {
            self.problem = Some(Problem { message: "Fill in every field and tick every statement first. Nothing was sent.".into(), retry: false, retry_at: None, account: false });
            return;
        }
        let Some(it) = self.selected_item() else { return };
        // A pending takedown is answered on the action the Worker listed; a live one by its id.
        let id = it.id.clone();
        self.problem = None;
        let wire = self.counter.to_wire();
        self.launch(Work::Counter(id, Box::new(wire)));
    }

    fn on_counter(&mut self, id: String, r: Result<CounterNoticeAck, PoolError>) {
        match r {
            Ok(ack) => {
                // Sent: nothing of it stays.
                self.counter.clear();
                if let Some(it) = self.item_mut(&id) {
                    if it.state == ItemState::TakedownPending {
                        it.counter_notice_sent = true;
                    } else {
                        it.counter_notice = Some("received".into());
                    }
                }
                self.panel = Panel::CounterSent(ack);
            }
            Err(e) => self.act_failed(&e),
        }
    }

    fn act_failed(&mut self, e: &PoolError) {
        if let Some(p) = self.problem_of(e) {
            self.problem = Some(p);
        }
        if Self::stale(e) {
            self.reload();
        }
    }

    // ---- share again ---------------------------------------------------------------------------------------------

    fn ensure_records(&mut self) -> &[PublishedRecord] {
        if self.records.is_none() {
            self.records = Some(self.published.load_all());
        }
        self.records.as_deref().unwrap_or(&[])
    }

    /// Where "Share again" for an item leads. Reads `share-published.json`.
    pub fn republish_plan(&mut self, id: &str) -> Republish {
        match self.ensure_records().iter().find(|r| r.item_id == id) {
            Some(r) if r.key.starts_with("bank:") => Republish::Source(r.key.clone()),
            Some(r) if r.key.starts_with("exam:") => Republish::Exam,
            _ => Republish::Unknown,
        }
    }

    /// The local source an item was shared from, as the publish flow keeps it, if this device
    /// shared it (the "Shared" mark on a bank page).
    pub fn shared_record(&mut self, key: &str) -> Option<PublishedRecord> {
        self.ensure_records().iter().find(|r| r.key == key).cloned()
    }

    /// "Share again..." on the selected item. The key to hand to the publish flow is taken by the
    /// app with [`MineModel::take_republish`].
    pub fn share_again(&mut self) {
        if !self.allowed(Action::Republish) {
            return self.refuse("Sharing again");
        }
        let Some(id) = self.selected.clone() else { return };
        match self.republish_plan(&id) {
            Republish::Source(k) => {
                self.republish = Some(k);
                self.notice = Some(Notice::Info("The Share to pool window opens with this bank. Sharing it again replaces this item with a new version.".into()));
            }
            Republish::Exam => {
                self.notice = Some(Notice::Info("This was shared from an exam. Open that exam and choose Share to pool... in its File menu; it will update this item.".into()));
            }
            Republish::Unknown => {
                self.notice = Some(Notice::Info(
                    "This device has no record of sharing this item. Open the bank it came from, on the computer you shared it from, and choose Share to pool....".into(),
                ));
            }
        }
    }

    pub fn take_republish(&mut self) -> Option<String> {
        self.republish.take()
    }

    // ---- the export ------------------------------------------------------------------------------------------

    pub fn start_export(&mut self) {
        if self.running(JobKind::Export) {
            return;
        }
        self.export_data = None;
        self.export = ExportState::Running { done: 0, total: 0 };
        if !self.launch(Work::Export) {
            self.export = ExportState::Idle;
        }
    }

    /// Stops a running download; nothing is kept.
    pub fn cancel_export(&mut self) {
        self.jobs.retain(|j| j.kind != JobKind::Export);
        self.export_data = None;
        self.export = ExportState::Idle;
    }

    fn on_export(&mut self, r: Result<Bundle, PoolError>) {
        match r {
            Ok(b) => {
                self.export = ExportState::Ready { size: b.size(), parts: b.parts() };
                self.export_data = Some(b);
            }
            Err(e) => match self.problem_of(&e) {
                Some(p) => self.export = ExportState::Failed(p),
                None => self.export = ExportState::Idle,
            },
        }
    }

    /// Asks where to save, writes the file (and any part files beside it), then forgets the data.
    pub fn save_export(&mut self) {
        let Some(bundle) = self.export_data.take() else { return };
        let Some(path) = self.sink.choose(EXPORT_NAME) else {
            // Cancelled in the dialog: still there to save.
            self.export_data = Some(bundle);
            return;
        };
        let mut place = path.display().to_string();
        let mut failed = None;
        for (i, (_, bytes)) in bundle.files.iter().enumerate() {
            let target = if i == 0 { path.clone() } else { part_path(&path, i) };
            if let Err(e) = self.sink.write(&target, bytes) {
                failed = Some(format!("Could not write {}: {}", target.display(), e.kind()));
                break;
            }
        }
        match failed {
            None => {
                if cfg!(target_arch = "wasm32") {
                    place = "your browser's downloads".to_string();
                }
                self.export = ExportState::Saved { place, parts: bundle.parts() };
            }
            Some(m) => {
                self.export_data = Some(bundle);
                self.export = ExportState::Failed(Problem { message: m, retry: false, retry_at: None, account: false });
            }
        }
    }

    /// The export is fetched and waiting for a place to go.
    pub fn export_ready(&self) -> bool {
        self.export_data.is_some()
    }

    // ---- legal ----------------------------------------------------------------------------------------------

    pub fn set_base_url(&mut self, base: &str) {
        if self.base_url != base {
            self.base_url = base.to_string();
        }
    }

    /// The four draft legal pages with their addresses; `None` links when no server is configured.
    pub fn legal_links(&self) -> Vec<(&'static str, Option<String>)> {
        text::LEGAL_PAGES.iter().map(|(name, file)| (*name, text::legal_url(&self.base_url, file))).collect()
    }
}
