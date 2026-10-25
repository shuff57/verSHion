//! Working copies of circle banks: send my changes, get the latest and merge, the history of a bank
//! (look at a version, restore it, trim the old ones). No egui: tests drive every path with a
//! scripted session. The circle page draws this through `circle_edit_ui`.
//!
//! # Rules this module keeps
//!
//! - A closed model does no I/O. Nothing goes to the network until a person opens one of these
//!   screens and the account is signed in and approved.
//! - A send is a compare-and-set on the version the copy was in step with. When someone sent first
//!   the answer is "Someone else sent changes first": nothing is overwritten, the person is taken
//!   to "Get latest and merge", and only then sends again.
//! - A merge is started by the person, never by the app. It is the same three-way merge as for the
//!   public pool (`merge.rs`): the version the copy was last in step with, the bank now, the circle's
//!   newest version; a restore point is saved before anything is written; every question that both
//!   sides changed is the person's decision. Where the earlier version is no longer kept and the
//!   copy has no snapshot of it, the merge is two-way: every difference is a decision, and the
//!   screen says so.
//! - One change at a time (a send, a restore, a trim): a second press while one runs does nothing.
//!   A send keeps its `commit_id` while nothing it carries changes, so a lost answer is retried
//!   without sending twice.
//! - The bank on this device changes only in [`CircleEditModel::sync`] (which is given the banks),
//!   only after the restore point is written, and only when everything worked.
//! - Server text is kept raw and drawn through `api::plain`. Nothing here keeps a token or password:
//!   the merge token lives in memory for the length of one merge.

use std::collections::HashMap;
use std::sync::Arc;
use std::sync::mpsc::{Receiver, Sender, TryRecvError, channel};
use std::time::Duration;

use web_time::Instant;

use super::api::{self, ApiError};
use super::browse::{Imported, Problem, SharedImages, SharedRestore, StoreImages, StoreRestore, now_ms};
use super::circle_links::*;
use super::circle_versions_api::*;
use super::circles_api::*;
use super::circle_hide::{Decisions, Pictures, decision_refusal, left_out_unasked};
use super::circles_text as text;
use super::merge::{self, BaseQuestion, Choice, MergeOutcome, MergePlan};
use super::model::Runner;
use super::pool::{ATTEST_STATEMENT, License, LocalProblem, PayloadRefusal, Pool, PoolError, PublishProgress, RefusalKind, RetryPolicy, StoreMedia, sha256_hex};
use super::publish::{AccountSnapshot, Gate, SharedMedia, SharedSession};
use crate::model::QuestionBank;

/// What the person may do with one bank, from their role and the circle's state.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Perm {
    pub role: Role,
    pub is_author: bool,
    pub closed: bool,
}

impl Perm {
    /// Send a version (the owner and editors).
    pub fn can_send(&self) -> bool {
        !self.closed && self.role.can_add_banks()
    }
    /// Restore an old version and trim: the owner, or the bank's author while an editor.
    pub fn can_rewind(&self) -> bool {
        !self.closed && (self.role == Role::Owner || (self.is_author && self.role == Role::Editor))
    }
}

// ---- what is on screen -------------------------------------------------------------------------------------

#[derive(Debug, Default)]
pub enum EditView {
    #[default]
    Closed,
    Send(Box<SendView>),
    Merge(Box<MergeView>),
    History(Box<HistoryView>),
    Version(Box<VersionView>),
}

/// What a finished send says, in numbers.
#[derive(Clone, Debug, PartialEq)]
pub struct Sent {
    pub version: u64,
    pub added: u32,
    pub changed: u32,
    pub removed: u32,
    pub replayed: bool,
    /// The bank is hidden from the rest of the circle while it is reviewed.
    pub held: bool,
    /// This version is hidden: it held text taken for student information.
    pub withheld: bool,
    pub merged_from: Option<u64>,
    /// The bank hides its answer keys: pictures the viewers' copy left out that the person did not choose to
    /// drop (question positions).
    pub left_out_unasked: Vec<usize>,
    /// The bank hides its answer keys but this computer did not know it, so the picture step was not shown: the
    /// pictures the viewers' copy left out (critic 62 I5).
    pub not_asked: Vec<usize>,
    /// The picture step was shown, but the bank no longer hides its answer keys: viewers get the version in full. The
    /// pictures the person chose to drop (critic 62 I1).
    pub hiding_stopped: Option<Vec<usize>>,
    /// The next send to the bank asks about pictures (it is known to hide its keys now).
    pub ask_next: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ConflictInfo {
    pub head_version: Option<u64>,
    pub head_at: Option<i64>,
    pub head_by: Option<String>,
}

#[derive(Debug)]
pub enum SendStage {
    Editing,
    Working(Option<PublishProgress>),
    Done(Box<Sent>),
    Failed(Problem),
    /// 428: the version drops more than half of the bank's questions. Nothing was stored.
    Large { removed: u64, total: u64 },
    /// 409: someone sent first.
    Conflict(ConflictInfo),
    /// 422: the pool checked the questions and listed what to change. Nothing was stored.
    Payload(Box<PayloadRefusal>),
    /// A refusal with its own words and a way on (the licence, the merge token).
    Refused { message: String, merge_again: bool },
    /// Questions copied without their answer keys (phase C, `keyless_question`): nothing was sent.
    /// `positions` are 0-based; `kinds` is each one's kind (`Short` needs a sample answer).
    Keyless { positions: Vec<usize>, kinds: Vec<String> },
    /// The bank hides its answer keys from viewers and the pool needs decisions for the viewers' copy.
    Decide(Box<Decisions>),
    /// The bank hides its answer keys from viewers and the version has pictures: which ones viewers keep.
    /// Nothing was sent yet.
    Pictures(Box<Pictures>),
}

#[derive(Debug)]
pub struct SendView {
    pub local_id: String,
    pub cid: String,
    pub bid: String,
    pub title: String,
    pub perm: Perm,
    pub message: String,
    pub changes: Option<ChangeCount>,
    pub stage: SendStage,
    /// The person accepted a large change.
    pub confirm_large: bool,
    /// Sending over a version the person could not read (a hidden newest version).
    pub base_override: Option<u64>,
    /// The send carries the token of a merge just made.
    pub merged: bool,
    /// The own-work statement is ticked. Never remembered: every send starts unticked.
    pub attest: bool,
    /// The bank's licence as the pool has it now (a send goes with that, never with an old one).
    pub licence: LicenceState,
    /// The hidden newest version this send goes over, and the readable versions it replaces.
    pub replaces: Vec<VersionLine>,
    /// For a bank that hides its answer keys: the decisions for its viewers' copy, made on this page.
    pub decisions: StripDecisions,
    /// The sha256 of the payload `decisions` were made for: they go only with that content (critic 57 I1).
    pub decisions_for: Option<String>,
    /// The sha256 of the payload last sent from this page.
    pub sent_payload: Option<String>,
    /// The picture step's choices (for the payload whose sha256 they carry): kept and dropped. The kept ones go in
    /// `decisions.image_ok`; these say what the person chose, for the answer.
    pub picked: Option<Box<Pictures>>,
}

#[derive(Debug)]
pub enum LicenceState {
    Checking,
    Known(License),
    Failed(Problem),
}

/// The statement the person ticks on the send page. For the bank's author it is the same as the first
/// share. Anyone else did not choose the licence, so it says whose it is.
pub const SEND_ATTEST: &str = ATTEST_STATEMENT;

pub fn attest_statement(is_author: bool, licence: Option<License>) -> String {
    if is_author {
        return SEND_ATTEST.to_string();
    }
    let l = licence.map(|l| super::publish_text::license_name(l.as_str())).map(|n| n.split(" (").next().unwrap_or("").to_string()).filter(|n| !n.is_empty());
    format!(
        "I confirm that I wrote these changes, or have the right to share them, and that they are not copied from a published source (textbooks, publisher test banks, exam boards, paid worksheets) without permission. I understand the bank's licence is {}, as set by its author, and that my changes go with it.",
        l.as_deref().unwrap_or("the one its author set")
    )
}

/// The licence line of the send page.
pub fn licence_line(is_author: bool, l: License) -> String {
    let full = super::publish_text::license_name(l.as_str());
    if is_author {
        format!("Licence: {full}. A send keeps the bank's own licence; you can change it from the bank's page in the circle.")
    } else {
        let short = full.split(" (").next().unwrap_or("").to_string();
        format!("Licence: {short}, set by the bank's author. Your changes go with it; only the author can change it.")
    }
}

#[derive(Debug)]
pub struct Fetched {
    pub start: MergeStart,
    pub head: BankDoc,
    pub base: Option<BankDoc>,
}

#[derive(Debug)]
pub struct Review {
    pub plan: MergePlan,
    /// The earlier version is not known: every difference is a decision.
    pub fallback: bool,
    pub start: MergeStart,
    pub problem: Option<String>,
    base: Vec<BaseQuestion>,
    /// The bank as it was when this review was built: Apply refuses if it changed since.
    ours_print: String,
    fetched: Box<Fetched>,
}

#[derive(Debug)]
pub struct MergeDone {
    pub outcome: MergeOutcome,
    pub restore: String,
    pub version: u64,
    /// What the bank still has that the circle does not (to send).
    pub changes: ChangeCount,
}

/// What the person is being asked to confirm on a stale copy.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StaleAsk {
    /// Treat the bank as it is as in step with the circle.
    Rebase,
    /// Let go of the link.
    Forget,
}

#[derive(Debug)]
pub enum MergePhase {
    Fetching,
    /// Fetched; the plan is built by the next `sync` (it needs the banks).
    Ready(Box<Fetched>),
    Review(Box<Review>),
    /// The newest version's pictures are being fetched; then the bank changes.
    Applying(Box<Review>),
    Applied(Box<MergeDone>),
    /// The copy already has the circle's newest version.
    UpToDate,
    /// The same, but this device's bank is older than the link (a saved copy of the banks was put
    /// back): nothing to get, and the link can be set in step with the bank as it is, or let go.
    StaleUpToDate { ask: Option<StaleAsk> },
    /// The newest version is hidden while it is reviewed: it can not be merged.
    HeadWithheld { head_version: Option<u64>, replaces: Vec<VersionLine> },
    Blocked(String),
    Failed(Problem),
}

#[derive(Debug)]
pub struct MergeView {
    pub local_id: String,
    pub cid: String,
    pub bid: String,
    pub title: String,
    pub perm: Perm,
    pub phase: MergePhase,
}

#[derive(Debug)]
pub enum RestoreStage {
    Confirm,
    Working,
    /// 428: the restored version drops more than half of the questions.
    Large { removed: u64, total: u64 },
    Done { version: u64, held: bool, withheld: bool },
    Failed(Problem),
}

#[derive(Debug)]
pub struct RestoreAsk {
    pub to: u64,
    pub stage: RestoreStage,
}

#[derive(Debug)]
pub enum TrimStage {
    Editing,
    Confirm,
    Working,
    Done(Trimmed),
    Failed(Problem),
}

#[derive(Debug)]
pub struct TrimAsk {
    pub keep: u32,
    pub stage: TrimStage,
}

#[derive(Debug)]
pub struct HistoryView {
    pub cid: String,
    pub bid: String,
    /// The local working copy, when there is one.
    pub local_id: Option<String>,
    pub title: String,
    pub perm: Perm,
    pub loaded: bool,
    pub loading_more: bool,
    pub head_version: u64,
    pub oldest_kept: Option<u64>,
    pub pinned: Vec<u64>,
    pub lines: Vec<VersionLine>,
    pub next_before: Option<u64>,
    pub problem: Option<Problem>,
    pub restore: Option<RestoreAsk>,
    pub trim: Option<TrimAsk>,
    /// The versions the pool would not show without their answer keys (`keys_hidden_unavailable`: a viewer,
    /// and a version with no viewers' copy). Each is not offered again: the history line does not say which
    /// versions have a viewers' copy (critic 58 I3), and that is per version, not ordered (critic 59 I1).
    pub keys_closed: std::collections::BTreeSet<u64>,
}

impl HistoryView {
    /// "Look at it" is offered for this line.
    pub fn can_look(&self, l: &VersionLine) -> bool {
        l.kept && !l.withheld && !self.closed_to_me(l)
    }

    /// A version the pool would not show this person without its answer keys (learnt from its own refusal).
    pub fn closed_to_me(&self, l: &VersionLine) -> bool {
        self.keys_closed.contains(&l.version)
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct QuestionLine {
    pub number: usize,
    pub stem: String,
    pub kind: String,
    pub has_image: bool,
}

#[derive(Debug)]
pub struct VersionBody {
    pub title: String,
    pub license: String,
    pub questions: Vec<QuestionLine>,
    /// Phase C: the bank hides its answer keys from viewers, and this read is (or is not) the viewers' form.
    pub hide_key: bool,
    pub keys_hidden: bool,
}

#[derive(Debug)]
pub enum VersionLoad {
    Loading,
    Ready(Box<VersionBody>),
    Failed(Problem),
}

#[derive(Debug)]
pub struct VersionView {
    pub cid: String,
    pub bid: String,
    pub n: u64,
    pub line: Option<VersionLine>,
    pub load: VersionLoad,
}

/// A link moved on (a send or a merge), for the list of imports to follow.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Advanced {
    pub circle_bank_id: String,
    pub local_bank_id: String,
    pub version: u64,
}

/// Where one linked copy stands.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct LinkStatus {
    key: (i64, usize),
    pub changes: ChangeCount,
    /// The bank is not on this device any more.
    pub missing: bool,
    /// The bank is older than the link (a saved copy of the banks was put back).
    pub stale: bool,
}

// ---- jobs ---------------------------------------------------------------------------------------------------------

enum Work {
    Send { cid: String, bid: String, req: Box<CircleSend> },
    MergeFetch { cid: String, bid: String, base: u64, need_base: bool },
    MergeApply { cid: String, bid: String, version: u64 },
    Versions { cid: String, bid: String, before: Option<u64> },
    Version { cid: String, bid: String, n: u64 },
    Rollback { cid: String, bid: String, commit_id: String, base: u64, to: u64, confirm: bool },
    Trim { cid: String, bid: String, keep: u32 },
    Licence { bid: String },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum JobKind {
    /// A send, a restore or a trim: one at a time.
    Act,
    Merge,
    Versions,
    Version,
    Licence,
}

impl Work {
    fn kind(&self) -> JobKind {
        match self {
            Work::Send { .. } | Work::Rollback { .. } | Work::Trim { .. } => JobKind::Act,
            Work::MergeFetch { .. } | Work::MergeApply { .. } => JobKind::Merge,
            Work::Versions { .. } => JobKind::Versions,
            Work::Version { .. } => JobKind::Version,
            Work::Licence { .. } => JobKind::Licence,
        }
    }
}

enum Fetch {
    Ready(Box<Fetched>),
    /// The newest version is hidden while it is reviewed; what merge-initiate said is kept.
    Withheld(Box<MergeStart>),
    /// The circle's newest version changed while it was being read.
    Moved,
}

enum Msg {
    Progress(PublishProgress),
    /// The answer, and the licence the send really went with (the pool's, asked just before).
    Sent(String, String, Result<SentBank, PoolError>, Option<License>),
    Fetched(String, String, Result<Fetch, PoolError>),
    Applied(String, String, Result<Option<Box<Imported>>, PoolError>),
    Versions(String, String, bool, Result<VersionsPage, PoolError>),
    Version(String, String, u64, Box<Result<BankDoc, PoolError>>),
    Rolled(String, String, u64, Result<SentBank, PoolError>),
    Trimmed(String, String, Result<Trimmed, PoolError>),
    Licence(String, Result<Vec<HeadEntry>, PoolError>),
}

impl Msg {
    fn kind(&self) -> JobKind {
        match self {
            Msg::Progress(_) | Msg::Sent(..) | Msg::Rolled(..) | Msg::Trimmed(..) => JobKind::Act,
            Msg::Fetched(..) | Msg::Applied(..) => JobKind::Merge,
            Msg::Versions(..) => JobKind::Versions,
            Msg::Version(..) => JobKind::Version,
            Msg::Licence(..) => JobKind::Licence,
        }
    }
}

struct Job {
    kind: JobKind,
    rx: Receiver<Msg>,
}

type Repaint = Option<Arc<dyn Fn() + Send + Sync>>;

async fn run_async(session: SharedSession, images: SharedImages, media: SharedMedia, retry: RetryPolicy, work: Work, tx: Sender<Msg>, repaint: Repaint) {
    let pool = Pool::new(&*session).with_retry(retry);
    let msg = match work {
        Work::Send { cid, bid, req } => {
            let (ptx, prepaint) = (tx.clone(), repaint.clone());
            let mut progress = move |p: PublishProgress| {
                let _ = ptx.send(Msg::Progress(p));
                if let Some(r) = &prepaint {
                    r();
                }
            };
            let (r, eff) = send_checked(&pool, &cid, &bid, *req, &*media, &mut progress).await;
            Msg::Sent(cid, bid, r, eff)
        }
        Work::MergeFetch { cid, bid, base, need_base } => {
            let r = fetch_for_merge(&pool, &cid, &bid, base, need_base).await;
            Msg::Fetched(cid, bid, r)
        }
        Work::MergeApply { cid, bid, version } => {
            let r = fetch_for_apply(&pool, &images, &cid, &bid, version).await;
            Msg::Applied(cid, bid, r)
        }
        Work::Versions { cid, bid, before } => {
            let r = pool.circle_versions(&cid, &bid, before).await;
            Msg::Versions(cid, bid, before.is_none(), r)
        }
        Work::Version { cid, bid, n } => {
            let r = pool.circle_version(&cid, &bid, n).await;
            Msg::Version(cid, bid, n, Box::new(r))
        }
        Work::Rollback { cid, bid, commit_id, base, to, confirm } => {
            let r = pool.rollback_circle_bank(&cid, &bid, &commit_id, base, to, "", confirm).await;
            Msg::Rolled(cid, bid, to, r)
        }
        Work::Trim { cid, bid, keep } => {
            let r = pool.trim_circle_bank(&cid, &bid, keep).await;
            Msg::Trimmed(cid, bid, r)
        }
        Work::Licence { bid } => {
            let r = pool.circle_heads(std::slice::from_ref(&bid)).await;
            Msg::Licence(bid, r)
        }
    };
    let _ = tx.send(msg);
    if let Some(r) = repaint {
        r();
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn run(session: SharedSession, images: SharedImages, media: SharedMedia, retry: RetryPolicy, work: Work, tx: Sender<Msg>, repaint: Repaint) {
    super::transport::block_on(run_async(session, images, media, retry, work, tx, repaint));
}

/// The send, with the licence asked of the pool just before: the page may have been open a while, and a
/// send must never change the bank's licence back. If the pool can not be asked, nothing is sent.
async fn send_checked(pool: &Pool<'_>, cid: &str, bid: &str, mut req: CircleSend, media: &dyn super::pool::MediaSource, progress: &mut dyn FnMut(PublishProgress)) -> (Result<SentBank, PoolError>, Option<License>) {
    let now = match pool.circle_heads(&[bid.to_string()]).await {
        Ok(es) => es.into_iter().find(|e| e.bank_id == bid).and_then(|e| match e.state {
            HeadState::Current { license: Some(l), .. } => License::parse(&l),
            _ => None,
        }),
        Err(e) => return (Err(e), None),
    };
    if let Some(l) = now {
        req.license = l;
    }
    (pool.send_circle_version(cid, bid, &req, media, progress).await, now)
}

/// merge-initiate, the newest version, and (when the copy kept no snapshot) the version it was made from.
async fn fetch_for_merge(pool: &Pool<'_>, cid: &str, bid: &str, base: u64, need_base: bool) -> Result<Fetch, PoolError> {
    let start = pool.merge_start(cid, bid, base).await?;
    let head = match pool.circle_bank(cid, bid).await {
        Ok(h) => h,
        Err(PoolError::Refused(r)) if matches!(r.kind, RefusalKind::VersionWithheld) => return Ok(Fetch::Withheld(Box::new(start))),
        Err(e) => return Err(e),
    };
    if head.bank.version != start.head_version {
        return Ok(Fetch::Moved);
    }
    let base_doc = if need_base && start.base_kept {
        match pool.circle_version(cid, bid, base).await {
            Ok(d) => Some(d),
            // Gone or hidden since: the merge is two-way.
            Err(PoolError::Refused(r)) if matches!(r.kind, RefusalKind::VersionNotRetained | RefusalKind::VersionWithheld) => None,
            Err(e) => return Err(e),
        }
    } else {
        None
    };
    Ok(Fetch::Ready(Box::new(Fetched { start, head, base: base_doc })))
}

/// The newest version again, with its pictures stored here. `None` when it is not the version that was reviewed.
async fn fetch_for_apply(pool: &Pool<'_>, images: &SharedImages, cid: &str, bid: &str, version: u64) -> Result<Option<Box<Imported>>, PoolError> {
    let doc = pool.circle_bank(cid, bid).await?;
    if doc.bank.version != version {
        return Ok(None);
    }
    Ok(Some(Box::new(super::circles::copy_bank(pool, &**images, &doc).await?)))
}

// ---- the model ---------------------------------------------------------------------------------------------------

struct Inflight {
    link: CircleLink,
    bank: QuestionBank,
    /// The questions of the payload sent that have a picture, those whose picture the send kept for viewers
    /// (`image_ok`), and those the person chose to drop on the picture step.
    pictures: Vec<usize>,
    image_ok: Vec<usize>,
    dropped: Vec<usize>,
    /// The picture step was shown for this content (the person was told the bank hides its keys).
    asked: bool,
}

struct Carry {
    local_id: String,
    token: String,
    from: u64,
    head: u64,
}

pub struct CircleEditModel {
    pub view: EditView,
    pub notice: Option<String>,
    /// The pool answered `invite_required`.
    pub invite_required: bool,
    links: Vec<CircleLink>,
    loaded: bool,
    log: LinkLog,
    statuses: HashMap<String, LinkStatus>,
    gate: Gate,
    session: Option<SharedSession>,
    images: SharedImages,
    media: SharedMedia,
    restore: SharedRestore,
    runner: Runner,
    retry: RetryPolicy,
    jobs: Vec<Job>,
    repaint: Repaint,
    now: Option<Instant>,
    clock: Option<i64>,
    /// The send the person asked for, started by `sync` (it needs the banks).
    cmd_send: bool,
    /// A merge applied by `sync`: the newest version, with pictures.
    applying: Option<Box<Imported>>,
    inflight: Option<Inflight>,
    /// The commit id of the last send and what it carried.
    pending_send: Option<(String, String)>,
    pending_restore: Option<(String, String)>,
    carry: Option<Carry>,
    history_stash: Option<Box<HistoryView>>,
    advanced: Vec<Advanced>,
    signed_out: bool,
    last_act: Option<ActAgain>,
    /// Said on the next review (the bank changed under the last one).
    merge_note: Option<String>,
    /// The links file is there but could not be read: it is not written over until it can be.
    unreadable: Option<String>,
    /// The card whose "Forget this link" was pressed once.
    forget_ask: Option<String>,
    /// "Treat this copy as in step": done by `sync`, which has the banks.
    cmd_rebase: bool,
    /// Circle banks known to hide their answer keys from viewers (`true`) or not, as last read (the circle page,
    /// a bank's read, a change of the setting, a send's answer). A send to one asks about pictures first.
    hiding: HashMap<String, bool>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ActAgain {
    Send,
    Restore(u64),
    Trim(u32),
}

impl Default for CircleEditModel {
    fn default() -> Self {
        CircleEditModel::new(Runner::Background, Arc::new(StoreImages), Arc::new(StoreMedia), Arc::new(StoreRestore), LinkLog::Store)
    }
}

impl CircleEditModel {
    /// Does no I/O: nothing is read or sent until a screen is opened.
    pub fn new(runner: Runner, images: SharedImages, media: SharedMedia, restore: SharedRestore, log: LinkLog) -> CircleEditModel {
        CircleEditModel {
            view: EditView::Closed,
            notice: None,
            invite_required: false,
            links: Vec::new(),
            loaded: false,
            log,
            statuses: HashMap::new(),
            gate: Gate::Checking,
            session: None,
            images,
            media,
            restore,
            runner,
            retry: RetryPolicy::default(),
            jobs: Vec::new(),
            repaint: None,
            now: None,
            clock: None,
            cmd_send: false,
            applying: None,
            inflight: None,
            pending_send: None,
            pending_restore: None,
            carry: None,
            history_stash: None,
            advanced: Vec::new(),
            signed_out: false,
            last_act: None,
            merge_note: None,
            unreadable: None,
            forget_ask: None,
            cmd_rebase: false,
            hiding: HashMap::new(),
        }
    }

    pub fn with_retry(mut self, retry: RetryPolicy) -> CircleEditModel {
        self.retry = retry;
        self
    }

    pub fn set_retry(&mut self, retry: RetryPolicy) {
        self.retry = retry;
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

    /// One of these screens is on (the circle page draws it instead of itself).
    pub fn active(&self) -> bool {
        !matches!(self.view, EditView::Closed)
    }

    pub fn busy(&self) -> bool {
        !self.jobs.is_empty() || self.cmd_send || self.applying.is_some()
    }

    fn running(&self, k: JobKind) -> bool {
        self.jobs.iter().any(|j| j.kind == k)
    }

    /// A send, a restore or a trim is on its way.
    pub fn acting(&self) -> bool {
        self.running(JobKind::Act) || self.cmd_send
    }

    pub fn take_signed_out(&mut self) -> bool {
        std::mem::take(&mut self.signed_out)
    }

    pub fn take_advanced(&mut self) -> Vec<Advanced> {
        std::mem::take(&mut self.advanced)
    }

    /// The screens close and their jobs go, except a send already on its way (critic 61 I4): its answer only moves
    /// this computer's link on (see `on_sent`: "a send that went through still moves the link on"), so the link is
    /// never left behind a version the person sent, and their next send is not refused as a conflict with it.
    pub fn close(&mut self) {
        let sending = self.inflight.is_some();
        self.jobs.retain(|j| sending && j.kind == JobKind::Act);
        if self.jobs.is_empty() {
            self.inflight = None;
        }
        self.view = EditView::Closed;
        self.history_stash = None;
        self.cmd_send = false;
        self.applying = None;
        self.carry = None;
    }

    /// The account signed out or another signed in (`another`, critic 60 I1): the screens close (a history goes with
    /// the versions refused to that account), and nothing that account was told or asked for stays. The working-copy
    /// links on this computer are kept, and so is a send already on its way (see `close`). Critic 61 I4: after a
    /// sign-out the commit ids of the last send and restore are kept too, so the same account, signed in again,
    /// retries a send or a restore whose answer it never read as the same commit (the pool replays it, nothing is
    /// doubled); another account starts its own.
    pub fn forget_account(&mut self, another: bool) {
        self.close();
        self.notice = None;
        self.invite_required = false;
        if another {
            self.pending_send = None;
            self.pending_restore = None;
        }
        self.signed_out = false;
        self.last_act = None;
        self.merge_note = None;
        self.forget_ask = None;
        // (read for the account: read again with the circle page)
        self.hiding.clear();
    }

    // ---- the links -----------------------------------------------------------------------------------------

    fn ensure(&mut self) {
        if self.loaded {
            return;
        }
        match self.log.load_checked() {
            Ok(file) => {
                // Anything added while the file could not be read is kept, and wins over the file.
                let pending = std::mem::replace(&mut self.links, file);
                for p in pending {
                    // The same local bank, or the same circle bank, is one link: the newer wins.
                    self.links.retain(|l| l.local_bank_id != p.local_bank_id && !(l.circle_id == p.circle_id && l.circle_bank_id == p.circle_bank_id));
                    self.links.push(p);
                }
                self.loaded = true;
                self.unreadable = None;
            }
            Err(m) => self.unreadable = Some(m),
        }
    }

    pub fn links(&mut self) -> &[CircleLink] {
        self.ensure();
        &self.links
    }

    pub fn link_for_local(&mut self, local_id: &str) -> Option<CircleLink> {
        self.ensure();
        self.links.iter().find(|l| l.local_bank_id == local_id).cloned()
    }

    pub fn link_for_bank(&mut self, cid: &str, bid: &str) -> Option<CircleLink> {
        self.ensure();
        self.links.iter().find(|l| l.circle_id == cid && l.circle_bank_id == bid).cloned()
    }

    pub fn links_in(&mut self, cid: &str) -> Vec<CircleLink> {
        self.ensure();
        self.links.iter().filter(|l| l.circle_id == cid).cloned().collect()
    }

    fn copy_is_stale(&self, local_id: &str) -> bool {
        self.statuses.get(local_id).is_some_and(|s| s.stale)
    }

    pub fn status(&self, local_id: &str) -> Option<&LinkStatus> {
        self.statuses.get(local_id)
    }

    /// Remembers a new working copy. The copy itself is already in the banks.
    pub fn add_link(&mut self, link: CircleLink) -> Result<(), String> {
        self.ensure();
        self.links.retain(|l| l.local_bank_id != link.local_bank_id && !(l.circle_id == link.circle_id && l.circle_bank_id == link.circle_bank_id));
        self.links.push(link);
        self.save()
    }

    /// Stops following the circle bank. The local bank stays as it is.
    pub fn forget(&mut self, local_id: &str) {
        self.ensure();
        self.links.retain(|l| l.local_bank_id != local_id);
        self.statuses.remove(local_id);
        if let Err(m) = self.save() {
            self.notice = Some(format!("The list of working copies could not be saved ({m})."));
        }
    }

    fn save(&mut self) -> Result<(), String> {
        // Never write over a file that could not be read: it may hold every link.
        self.ensure();
        if let Some(m) = &self.unreadable {
            return Err(format!("the list of working copies could not be read ({m}), so it was left as it is"));
        }
        self.log.save(&self.links)
    }

    fn replace_link(&mut self, link: CircleLink) {
        self.ensure();
        match self.links.iter_mut().find(|l| l.local_bank_id == link.local_bank_id) {
            Some(l) => *l = link,
            None => return,
        }
        if let Err(m) = self.save() {
            self.notice = Some(format!("The working copy was updated, but the list could not be saved ({m}). The next check will catch up."));
        }
    }

    /// The quiet check of the circles' newest versions: a link that is behind says so.
    pub fn apply_heads(&mut self, entries: &[HeadEntry]) {
        self.ensure();
        let mut changed = false;
        for l in &mut self.links {
            let Some(e) = entries.iter().find(|e| e.bank_id == l.circle_bank_id) else { continue };
            if let HeadState::Current { version, at, by, .. } = &e.state
                && *version >= l.head_seen
                && (*version != l.head_seen || l.head_seen_at != *at)
            {
                l.head_seen = *version;
                l.head_seen_at = *at;
                l.head_seen_by = by.clone();
                changed = true;
            }
        }
        if changed && let Err(m) = self.save() {
            self.notice = Some(format!("The result of the check could not be saved ({m})."));
        }
    }

    // ---- frame ------------------------------------------------------------------------------------------------

    pub fn frame(&mut self, now: Instant, snap: &AccountSnapshot) {
        self.now = Some(now);
        self.session = snap.session.clone();
        self.gate = snap.gate.clone();
        self.drain();
    }

    // ---- errors ------------------------------------------------------------------------------------------------

    /// An error as a `Problem`; `None` when the sign-in ended (the person is sent to sign in again).
    fn problem_of(&mut self, e: &PoolError) -> Option<Problem> {
        let now = self.now.unwrap_or_else(Instant::now);
        let mut p = Problem { message: e.message(), retry: false, retry_at: None, account: false };
        match e {
            PoolError::Api(ApiError::SignedOut) => {
                self.signed_out = true;
                self.notice = Some("Your session ended. Sign in again; where you were is kept, and nothing was lost.".into());
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
                    RefusalKind::DailyLimit { metric } => {
                        p.retry = true;
                        p.retry_at = wait;
                        match metric.as_deref() {
                            Some("circle_rollbacks") => p.message = "You have restored as many older versions today as the pool allows. Try again tomorrow.".into(),
                            Some("circle_commits") => p.message = "You have sent as many versions to circles today as the pool allows. Your changes are still on this device. Try again tomorrow.".into(),
                            _ => {}
                        }
                    }
                    RefusalKind::RateLimited | RefusalKind::TooManyOpenReads | RefusalKind::TooManyUploads | RefusalKind::Busy | RefusalKind::PublishInProgress => {
                        p.retry = true;
                        p.retry_at = wait;
                    }
                    RefusalKind::NotFound => p.message = "That circle or bank is not available to you any more. It may have been closed, removed or hidden, or you may have left the circle.".into(),
                    RefusalKind::ModerationState { message } => {
                        p.message = message.as_deref().map(api::plain).filter(|m| !m.is_empty()).unwrap_or_else(|| "The pool does not allow that right now.".into());
                    }
                    RefusalKind::InvalidRequest { field } => {
                        p.message = match field.as_deref() {
                            Some("base_version") => "The circle has no newer version than the one you have.".into(),
                            Some("message") => "The note can be at most 500 characters.".into(),
                            Some("keep") => "Choose how many versions to keep, from 1 to 19.".into(),
                            Some("to_version") => "Choose an older version than the newest one.".into(),
                            Some("merge_token" | "merged_from") => "This merge is out of date. Get the latest changes and merge again, then send.".into(),
                            Some("commit_id") => "That send could not be identified. Start it again.".into(),
                            Some("license") => "Choose a licence for this bank.".into(),
                            Some(f) => format!("The pool did not accept the {}. Nothing was changed.", f.replace('_', " ")),
                            None => "The pool did not accept that. Nothing was changed.".into(),
                        }
                    }
                    RefusalKind::Other if r.detail.code == "pool_migrating" => {
                        p.message = "The pool is being upgraded. Try again in a few minutes.".into();
                        p.retry = true;
                        p.retry_at = wait;
                    }
                    _ => {}
                }
            }
        }
        Some(p)
    }

    // ---- jobs ----------------------------------------------------------------------------------------------------

    fn launch(&mut self, work: Work) -> bool {
        let Some(session) = self.session.clone() else {
            return false;
        };
        let kind = work.kind();
        if kind == JobKind::Act && self.running(JobKind::Act) {
            return false;
        }
        self.jobs.retain(|j| j.kind != kind);
        let (tx, rx) = channel::<Msg>();
        let (retry, repaint, images, media) = (self.retry, self.repaint.clone(), self.images.clone(), self.media.clone());
        match self.runner {
            #[cfg(not(target_arch = "wasm32"))]
            Runner::Inline => run(session, images, media, retry, work, tx, repaint),
            #[cfg(target_arch = "wasm32")]
            Runner::Inline => wasm_bindgen_futures::spawn_local(async move { run_async(session, images, media, retry, work, tx, repaint).await }),
            #[cfg(not(target_arch = "wasm32"))]
            Runner::Background => {
                std::thread::spawn(move || run(session, images, media, retry, work, tx, repaint));
            }
            #[cfg(target_arch = "wasm32")]
            Runner::Background => wasm_bindgen_futures::spawn_local(async move { run_async(session, images, media, retry, work, tx, repaint).await }),
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
            if let Msg::Progress(p) = m {
                if let EditView::Send(v) = &mut self.view
                    && matches!(v.stage, SendStage::Working(_))
                {
                    v.stage = SendStage::Working(Some(p));
                }
                continue;
            }
            let kind = m.kind();
            self.jobs.retain(|j| j.kind != kind);
            lost.retain(|k| *k != kind);
            match m {
                Msg::Progress(_) => {}
                Msg::Sent(cid, bid, r, eff) => self.on_sent(&cid, &bid, r, eff),
                Msg::Fetched(cid, bid, r) => self.on_fetched(&cid, &bid, r),
                Msg::Applied(cid, bid, r) => self.on_applied(&cid, &bid, r),
                Msg::Versions(cid, bid, first, r) => self.on_versions(&cid, &bid, first, r),
                Msg::Version(cid, bid, n, r) => self.on_version(&cid, &bid, n, *r),
                Msg::Rolled(cid, bid, to, r) => self.on_rolled(&cid, &bid, to, r),
                Msg::Trimmed(cid, bid, r) => self.on_trimmed(&cid, &bid, r),
                Msg::Licence(bid, r) => self.on_licence(&bid, r),
            }
        }
        // A job that ended without an answer (its thread died).
        for k in lost {
            self.jobs.retain(|j| j.kind != k);
            let p = Problem { message: "The pool did not answer. Nothing was changed. Try again.".into(), retry: true, retry_at: None, account: false };
            self.fail_kind(k, p);
        }
    }

    fn fail_kind(&mut self, k: JobKind, p: Problem) {
        match (&mut self.view, k) {
            (EditView::Send(v), JobKind::Act) => v.stage = SendStage::Failed(p),
            (EditView::Merge(v), JobKind::Merge) => v.phase = MergePhase::Failed(p),
            (EditView::History(h), JobKind::Versions) => {
                h.loading_more = false;
                h.problem = Some(p);
            }
            (EditView::History(h), JobKind::Act) => {
                if let Some(r) = &mut h.restore {
                    r.stage = RestoreStage::Failed(p.clone());
                }
                if let Some(t) = &mut h.trim {
                    t.stage = TrimStage::Failed(p);
                }
            }
            (EditView::Version(v), JobKind::Version) => v.load = VersionLoad::Failed(p),
            (EditView::Send(v), JobKind::Licence) => v.licence = LicenceState::Failed(p),
            _ => {}
        }
    }

    fn here(&self, cid: &str, bid: &str) -> bool {
        match &self.view {
            EditView::Send(v) => v.cid == cid && v.bid == bid,
            EditView::Merge(v) => v.cid == cid && v.bid == bid,
            EditView::History(v) => v.cid == cid && v.bid == bid,
            EditView::Version(v) => v.cid == cid && v.bid == bid,
            EditView::Closed => false,
        }
    }

    // ---- send my changes ------------------------------------------------------------------------------------

    /// "Send my changes": the page where the person adds a note and sends.
    pub fn open_send(&mut self, local_id: &str, perm: Perm) {
        self.ensure();
        let Some(l) = self.links.iter().find(|l| l.local_bank_id == local_id).cloned() else { return };
        self.jobs.clear();
        self.view = EditView::Send(Box::new(SendView {
            local_id: l.local_bank_id.clone(),
            cid: l.circle_id.clone(),
            bid: l.circle_bank_id.clone(),
            title: api::plain(&l.title),
            perm,
            message: String::new(),
            changes: None,
            stage: SendStage::Editing,
            confirm_large: false,
            base_override: None,
            merged: self.carry.as_ref().is_some_and(|c| c.local_id == local_id && c.head == l.base_version),
            attest: false,
            licence: LicenceState::Checking,
            replaces: Vec::new(),
            decisions: StripDecisions::default(),
            decisions_for: None,
            sent_payload: None,
            picked: None,
        }));
        // The counts are filled in by `sync`. The bank's licence is asked of the pool now: a send
        // goes with the licence the pool has, never with the one this device saw at copy time.
        self.statuses.remove(local_id);
        self.check_licence();
    }

    fn check_licence(&mut self) {
        let EditView::Send(v) = &mut self.view else { return };
        v.licence = LicenceState::Checking;
        let bid = v.bid.clone();
        if !self.launch(Work::Licence { bid })
            && let EditView::Send(v) = &mut self.view
        {
            v.licence = LicenceState::Failed(Problem { message: "Not signed in to the pool. Open the account window and sign in.".into(), retry: true, retry_at: None, account: true });
        }
    }

    /// "Try again" when the licence could not be checked.
    pub fn retry_licence(&mut self) {
        if matches!(&self.view, EditView::Send(v) if matches!(v.licence, LicenceState::Failed(_))) {
            self.check_licence();
        }
    }

    fn on_licence(&mut self, bid: &str, r: Result<Vec<HeadEntry>, PoolError>) {
        if !matches!(&self.view, EditView::Send(v) if v.bid == bid) {
            return;
        }
        let state = match r {
            Ok(entries) => {
                let cid = if let EditView::Send(v) = &self.view { v.cid.clone() } else { String::new() };
                let theirs = entries.iter().find(|e| e.bank_id == bid).and_then(|e| match &e.state {
                    HeadState::Current { license: Some(l), .. } => License::parse(l),
                    _ => None,
                });
                // (critic 62 I5) Whether the bank hides its keys, as the pool says now: the picture step is shown,
                // or not, from this rather than from an older read. Not said: what was known stays.
                if let Some(h) = entries.iter().find(|e| e.bank_id == bid).and_then(|e| match &e.state {
                    HeadState::Current { hide_key, .. } => *hide_key,
                    _ => None,
                }) {
                    self.note_hiding(bid, h);
                }
                let link = self.links.iter().find(|l| l.circle_id == cid && l.circle_bank_id == bid).cloned();
                match (theirs, link) {
                    (Some(l), Some(mut link)) => {
                        if link.license != l.as_str() {
                            link.license = l.as_str().to_string();
                            self.replace_link(link);
                        }
                        LicenceState::Known(l)
                    }
                    (None, Some(link)) => match link.licence() {
                        Some(l) => LicenceState::Known(l),
                        None => LicenceState::Failed(Problem { message: "This bank's licence is not known on this device and the pool did not say. Try again.".into(), retry: true, retry_at: None, account: false }),
                    },
                    _ => LicenceState::Failed(Problem { message: "This working copy is not in the list any more.".into(), retry: false, retry_at: None, account: false }),
                }
            }
            Err(e) => match self.problem_of(&e) {
                Some(p) => LicenceState::Failed(p),
                None => LicenceState::Failed(Problem { message: "Your session ended. Sign in again, then press Try again.".into(), retry: true, retry_at: None, account: false }),
            },
        };
        if let EditView::Send(v) = &mut self.view {
            v.licence = state;
        }
    }

    /// The own-work box. Never remembered between sends.
    pub fn set_attest(&mut self, on: bool) {
        if let EditView::Send(v) = &mut self.view
            && !matches!(v.stage, SendStage::Working(_))
        {
            v.attest = on;
        }
    }

    pub fn set_message(&mut self, m: &str) {
        if let EditView::Send(v) = &mut self.view
            && !matches!(v.stage, SendStage::Working(_))
        {
            v.message = m.chars().take(MESSAGE_MAX).collect();
        }
    }

    /// Why the Send button can not be pressed, or `None`.
    pub fn send_blocker(&self) -> Option<&'static str> {
        let EditView::Send(v) = &self.view else { return Some("Nothing to send.") };
        if !v.perm.can_send() {
            return Some(if v.perm.closed { "This circle is closed." } else { "You can only look at this circle's banks. The owner can let you add banks by changing your role to editor." });
        }
        if self.acting() {
            return Some("Wait for the current step to finish.");
        }
        if !matches!(v.stage, SendStage::Editing | SendStage::Failed(_) | SendStage::Large { .. } | SendStage::Refused { .. }) {
            return Some("Nothing to send.");
        }
        match &v.licence {
            LicenceState::Checking => return Some("Checking the bank's licence..."),
            LicenceState::Failed(_) => return Some("The bank's licence could not be checked, so nothing is sent. Try again."),
            LicenceState::Known(_) => {}
        }
        if self.copy_is_stale(&v.local_id) {
            return Some("This copy no longer matches where it was last in step with the circle (a saved copy of your banks may have been put back). Get the latest changes and merge first.");
        }
        if !v.attest {
            return Some("Tick the box that says this is your own work.");
        }
        None
    }

    /// The Send button.
    pub fn send(&mut self) {
        if self.send_blocker().is_some() {
            return;
        }
        if let EditView::Send(v) = &mut self.view {
            v.stage = SendStage::Working(None);
        }
        self.last_act = Some(ActAgain::Send);
        self.cmd_send = true;
    }

    /// "Send anyway": the version drops more than half of the questions and the person said so.
    pub fn confirm_large_send(&mut self) {
        let acting = self.acting();
        if let EditView::Send(v) = &mut self.view
            && matches!(v.stage, SendStage::Large { .. })
            && !acting
        {
            v.confirm_large = true;
            v.stage = SendStage::Working(None);
            self.last_act = Some(ActAgain::Send);
            self.cmd_send = true;
        }
    }

    pub fn cancel_large_send(&mut self) {
        if let EditView::Send(v) = &mut self.view
            && matches!(v.stage, SendStage::Large { .. })
        {
            v.stage = SendStage::Editing;
        }
    }

    fn send_decisions_mut(&mut self) -> Option<&mut Decisions> {
        match &mut self.view {
            EditView::Send(v) => match &mut v.stage {
                SendStage::Decide(d) => Some(d),
                _ => None,
            },
            _ => None,
        }
    }

    pub fn send_decide_label(&mut self, at: super::pool::Pos, c: super::pool::LabelChoice) {
        if let Some(d) = self.send_decisions_mut() {
            d.decide_label(at, c);
        }
    }

    pub fn send_set_order(&mut self, at: super::pool::Pos, yes: bool) {
        if let Some(d) = self.send_decisions_mut() {
            d.set_order(at, yes);
        }
    }

    pub fn send_all_orders(&mut self) {
        if let Some(d) = self.send_decisions_mut() {
            d.ack_all_orders();
        }
    }

    pub fn send_set_formatting(&mut self, question: usize, yes: bool) {
        if let Some(d) = self.send_decisions_mut() {
            d.set_formatting(question, yes);
        }
    }

    /// What this computer last heard about whether a circle bank hides its answer keys from viewers.
    pub fn note_hiding(&mut self, bid: &str, on: bool) {
        self.hiding.insert(bid.to_string(), on);
    }

    /// The bank is known to hide its answer keys (a send to it asks about pictures first). Not known: no step,
    /// and the answer to the send says what viewers miss.
    pub fn hides_keys(&self, bid: &str) -> bool {
        self.hiding.get(bid).copied().unwrap_or(false)
    }

    /// "Keep it for viewers" / "Drop it" on the picture step of a send.
    pub fn send_keep_picture(&mut self, question: usize, yes: bool) {
        if let EditView::Send(v) = &mut self.view
            && let SendStage::Pictures(p) = &mut v.stage
        {
            p.set_keep(question, yes);
        }
    }

    /// "Send" on the picture step: the send goes with the questions whose picture is kept (`image_ok`), for the
    /// content the step was made for (changed content is asked again).
    pub fn send_with_pictures(&mut self) {
        let acting = self.acting();
        let EditView::Send(v) = &mut self.view else { return };
        let SendStage::Pictures(p) = &v.stage else { return };
        if acting || !v.attest {
            return;
        }
        let p = p.clone();
        if v.decisions_for.as_deref() != Some(p.payload_sha.as_str()) {
            v.decisions = StripDecisions::default();
        }
        v.decisions.image_ok = p.image_ok();
        v.decisions_for = Some(p.payload_sha.clone());
        v.picked = Some(p);
        v.stage = SendStage::Working(None);
        self.last_act = Some(ActAgain::Send);
        self.cmd_send = true;
    }

    /// "Send with these decisions": everything the pool listed is decided; the send goes again with them.
    pub fn send_with_decisions(&mut self) {
        let acting = self.acting();
        let EditView::Send(v) = &mut self.view else { return };
        let SendStage::Decide(d) = &v.stage else { return };
        if !d.ready() || acting || !v.attest {
            return;
        }
        v.decisions = d.decisions();
        // (the pool listed these findings for the content just sent)
        v.decisions_for = v.sent_payload.clone();
        v.stage = SendStage::Working(None);
        self.last_act = Some(ActAgain::Send);
        self.cmd_send = true;
    }

    /// After "Failed" or a refusal: back to the note, nothing lost but the viewers' copy decisions: the bank
    /// may be changed from there, and they were made for its content as it was (critic 57 I1).
    pub fn back_to_note(&mut self) {
        if let EditView::Send(v) = &mut self.view
            && !matches!(v.stage, SendStage::Working(_))
        {
            v.stage = SendStage::Editing;
            v.decisions = StripDecisions::default();
            v.decisions_for = None;
            v.picked = None;
        }
    }

    /// Sends the person's version over a newest version that is hidden (it can not be merged).
    pub fn send_on_top(&mut self, head_version: u64, perm: Perm) {
        let EditView::Merge(m) = &self.view else { return };
        let local = m.local_id.clone();
        let replaces = match &m.phase {
            MergePhase::HeadWithheld { replaces, .. } => replaces.clone(),
            _ => Vec::new(),
        };
        self.open_send(&local, perm);
        if let EditView::Send(v) = &mut self.view {
            v.base_override = Some(head_version);
            v.replaces = replaces;
        }
    }

    fn do_send(&mut self, banks: &[QuestionBank]) {
        let hiding = match &self.view {
            EditView::Send(v) => self.hides_keys(&v.bid),
            _ => return,
        };
        let EditView::Send(v) = &mut self.view else { return };
        let Some(link) = self.links.iter().find(|l| l.local_bank_id == v.local_id).cloned() else {
            v.stage = SendStage::Failed(Problem { message: "This working copy is not in the list any more.".into(), retry: false, retry_at: None, account: false });
            return;
        };
        let fail = |v: &mut SendView, m: &str| v.stage = SendStage::Failed(Problem { message: m.into(), retry: false, retry_at: None, account: false });
        if !v.perm.can_send() {
            fail(v, "Your role in this circle does not allow sending a version.");
            return;
        }
        let Some(bank) = banks.iter().find(|b| b.id == link.local_bank_id) else {
            fail(v, "The bank is not on this device any more, so there is nothing to send.");
            return;
        };
        let LicenceState::Known(license) = v.licence else {
            fail(v, "The bank's licence has not been checked, so nothing was sent.");
            return;
        };
        if !v.attest {
            fail(v, "The own-work box is not ticked, so nothing was sent.");
            return;
        }
        if link.stale_for(bank) {
            fail(v, "This copy no longer matches where it was last in step with the circle. Get the latest changes and merge first. Nothing was sent.");
            return;
        }
        // Questions copied without their keys can not go to a circle (phase C): said here, nothing is sent.
        let keyless: Vec<(usize, String)> = bank.questions.iter().enumerate().filter(|(_, q)| crate::model::is_keyless(q)).map(|(i, q)| (i, kind_tag(&q.kind).to_string())).collect();
        if !keyless.is_empty() {
            v.stage = SendStage::Keyless { positions: keyless.iter().map(|k| k.0).collect(), kinds: keyless.into_iter().map(|k| k.1).collect() };
            return;
        }
        let payload = match payload_of(&link, bank) {
            Ok(p) => p,
            Err(m) => {
                fail(v, &m);
                return;
            }
        };
        let base = v.base_override.unwrap_or(link.base_version);
        // (critic 57 I1) The decisions are by position: they go only with the content they were made for.
        let payload_sha = sha256_hex(&payload);
        if v.decisions_for.as_deref() != Some(payload_sha.as_str()) {
            v.decisions = StripDecisions::default();
            v.decisions_for = None;
        }
        // A bank that hides its keys: which pictures viewers keep is asked first (the pool never asks: it leaves
        // out every picture not in `image_ok`). Asked again for changed content.
        if v.picked.as_ref().is_some_and(|p| p.payload_sha != payload_sha) {
            v.picked = None;
            v.decisions.image_ok.clear();
        }
        let pictures = Pictures::of_payload(&payload);
        if hiding
            && v.picked.is_none()
            && let Some(p) = pictures.clone()
        {
            v.stage = SendStage::Pictures(Box::new(p));
            return;
        }
        let shown: Vec<usize> = pictures.map(|p| p.list.iter().map(|x| x.question).collect()).unwrap_or_default();
        let asked = v.picked.is_some();
        let dropped = v.picked.as_ref().map(|p| p.dropped()).unwrap_or_default();
        let image_ok = v.decisions.image_ok.clone();
        v.sent_payload = Some(payload_sha);
        let mut req = CircleSend::new(payload, license, link.meta());
        req.message = v.message.trim().to_string();
        req.base_version = Some(base);
        req.confirm_large = v.confirm_large;
        req.attestation = true;
        req.decisions = v.decisions.clone();
        if let Some(c) = self.carry.as_ref().filter(|c| c.local_id == v.local_id && c.head == base) {
            req.merge = Some((c.token.clone(), c.from));
        }
        // The same commit id while nothing the server reads changed: a lost answer is retried, not doubled.
        let print = format!("{}|{}|{base}|{}|{}|{}|{:?}|{:?}|{}", link.circle_id, link.circle_bank_id, license.as_str(), req.message, req.confirm_large, req.merge, req.decisions, sha256_hex(&req.payload));
        match &self.pending_send {
            Some((p, id)) if *p == print => req.commit_id = id.clone(),
            _ => self.pending_send = Some((print, req.commit_id.clone())),
        }
        let (cid, bid) = (link.circle_id.clone(), link.circle_bank_id.clone());
        let bank = bank.clone();
        if self.launch(Work::Send { cid, bid, req: Box::new(req) }) {
            self.inflight = Some(Inflight { link, bank, pictures: shown, image_ok, dropped, asked });
        } else if let EditView::Send(v) = &mut self.view {
            v.stage = SendStage::Failed(Problem { message: "Not signed in to the pool. Open the account window and sign in.".into(), retry: true, retry_at: None, account: true });
        }
    }

    fn on_sent(&mut self, cid: &str, bid: &str, r: Result<SentBank, PoolError>, eff: Option<License>) {
        let mut inflight = self.inflight.take();
        if let (Some(l), Some(i)) = (eff, inflight.as_mut())
            && i.link.license != l.as_str()
        {
            // The licence changed while the page was open: the send went with the pool's, and the link follows.
            i.link.license = l.as_str().to_string();
            let link = i.link.clone();
            self.replace_link(link);
            if let EditView::Send(v) = &mut self.view {
                v.licence = LicenceState::Known(l);
            }
        }
        // (the answer says whether the bank hides its keys: the next send asks about pictures, or not)
        if let Ok(sent) = &r
            && sent.version > 0
        {
            self.note_hiding(bid, sent.keys_hidden);
        }
        if !self.here(cid, bid) || !matches!(self.view, EditView::Send(_)) {
            // The person left: a send that went through still moves the link on.
            if let (Ok(sent), Some(i)) = (&r, &inflight) {
                self.finish_send(i, sent);
            }
            return;
        }
        match r {
            Ok(sent) => {
                let merged_from = sent.merged_from;
                if let Some(i) = &inflight {
                    self.finish_send(i, &sent);
                }
                // The viewers' copy left out every picture not in `image_ok` (share-core's strip): said when the person
                // did not choose that: kept and then dropped by a restart, or never asked (critic 62 I5: this computer did
                // not know the bank hides its keys). And a step shown for a bank that no longer hides them: viewers get
                // everything, the pictures the person chose to drop too (critic 62 I1).
                let (mut left_out, mut not_asked, mut hiding_stopped) = (Vec::new(), Vec::new(), None);
                if let Some(i) = &inflight {
                    let removed: Vec<usize> = i.pictures.iter().copied().filter(|q| !i.image_ok.contains(q)).collect();
                    match (sent.keys_hidden, i.asked) {
                        (true, true) => left_out = left_out_unasked(&i.dropped, &removed),
                        (true, false) => not_asked = removed,
                        (false, true) if sent.version > 0 => hiding_stopped = Some(i.dropped.clone()),
                        _ => {}
                    }
                }
                let ask_next = self.hides_keys(bid);
                if let EditView::Send(v) = &mut self.view {
                    v.stage = SendStage::Done(Box::new(Sent { version: sent.version, added: sent.added, changed: sent.changed, removed: sent.removed, replayed: sent.replayed, held: sent.held, withheld: sent.withheld, merged_from, left_out_unasked: left_out, not_asked, hiding_stopped, ask_next }));
                }
            }
            Err(e) => {
                let stage = match &e {
                    PoolError::Refused(r) => match &r.kind {
                        RefusalKind::CircleConflict { head_version, head_at, head_by } => {
                            if let (Some(h), Some(i)) = (head_version, &inflight) {
                                let mut l = i.link.clone();
                                if *h > l.head_seen {
                                    l.head_seen = *h;
                                    l.head_seen_at = *head_at;
                                    l.head_seen_by = head_by.clone();
                                    self.replace_link(l);
                                }
                            }
                            Some(SendStage::Conflict(ConflictInfo { head_version: *head_version, head_at: *head_at, head_by: head_by.clone() }))
                        }
                        RefusalKind::ConfirmLarge { removed, total } => Some(SendStage::Large { removed: *removed, total: *total }),
                        // A bank that hides its answer keys: the pool asks how the viewers' copy is made.
                        k if decision_refusal(k).is_some() => {
                            self.pending_send = None;
                            let carried = if let EditView::Send(v) = &self.view { v.decisions.clone() } else { StripDecisions::default() };
                            decision_refusal(k).map(|p| SendStage::Decide(Box::new(Decisions::new(p.clone(), carried))))
                        }
                        RefusalKind::InvalidPayload(p) | RefusalKind::LabelsNeedDecision(p) | RefusalKind::OrderAckRequired(p) => {
                            self.pending_send = None;
                            Some(SendStage::Payload(p.clone()))
                        }
                        RefusalKind::KeylessQuestion { positions, kinds } => {
                            self.pending_send = None;
                            Some(SendStage::Keyless { positions: positions.clone(), kinds: kinds.clone() })
                        }
                        RefusalKind::HideKeyChanged { hide_key, .. } => {
                            // Nothing was stored; the same send goes again (with decisions if the pool asks for them,
                            // and the picture step first when the keys are hidden now).
                            if let Some(h) = hide_key {
                                self.note_hiding(bid, *h);
                            }
                            Some(SendStage::Refused { message: e.message(), merge_again: false })
                        }
                        RefusalKind::AttestationRequired => {
                            Some(SendStage::Failed(Problem { message: e.message(), retry: false, retry_at: None, account: false }))
                        }
                        RefusalKind::LicenseAuthorOnly { license } => {
                            if let (Some(l), Some(i)) = (license.as_deref().filter(|l| super::pool::License::parse(l).is_some()), &inflight) {
                                let mut link = i.link.clone();
                                link.license = l.to_string();
                                self.replace_link(link);
                                if let (EditView::Send(v), Some(known)) = (&mut self.view, super::pool::License::parse(l)) {
                                    v.licence = LicenceState::Known(known);
                                }
                            }
                            Some(SendStage::Refused { message: "Only the bank's author chooses its licence. Your changes will go with the bank's own licence. Send again.".into(), merge_again: false })
                        }
                        RefusalKind::MergeTokenInvalid => {
                            self.carry = None;
                            Some(SendStage::Refused { message: "This merge is out of date. Get the latest changes and merge again, then send.".into(), merge_again: true })
                        }
                        RefusalKind::CommitIdConflict => {
                            self.pending_send = None;
                            Some(SendStage::Failed(Problem { message: "This send was already used for different content. Press Send again to start it again.".into(), retry: true, retry_at: None, account: false }))
                        }
                        RefusalKind::BankRemoved => Some(SendStage::Failed(Problem { message: "This bank was removed from the circle, so nothing can be sent to it. The owner or the bank's author may be able to restore it.".into(), retry: false, retry_at: None, account: false })),
                        _ => None,
                    },
                    _ => None,
                };
                match stage {
                    Some(s) => {
                        let unticked = matches!(&e, PoolError::Refused(r) if matches!(r.kind, RefusalKind::AttestationRequired));
                        if let EditView::Send(v) = &mut self.view {
                            v.stage = s;
                            if unticked {
                                v.attest = false;
                            }
                        }
                    }
                    None => match self.problem_of(&e) {
                        Some(p) => {
                            if let EditView::Send(v) = &mut self.view {
                                v.stage = SendStage::Failed(p);
                            }
                        }
                        None => {
                            // Signed out: back to the note, the commit id is kept for the retry.
                            if let EditView::Send(v) = &mut self.view {
                                v.stage = SendStage::Editing;
                            }
                        }
                    },
                }
            }
        }
    }

    fn finish_send(&mut self, i: &Inflight, sent: &SentBank) {
        self.pending_send = None;
        self.carry = None;
        if sent.version == 0 {
            self.notice = Some("The version was sent, but the pool's answer could not be read. Press Get latest and merge to catch up.".into());
            return;
        }
        let l = after_send(&i.link, &i.bank, sent);
        self.advanced.push(Advanced { circle_bank_id: l.circle_bank_id.clone(), local_bank_id: l.local_bank_id.clone(), version: l.base_version });
        self.statuses.remove(&l.local_bank_id);
        self.replace_link(l);
    }

    // ---- get latest and merge --------------------------------------------------------------------------------

    pub fn open_merge(&mut self, local_id: &str, perm: Perm) {
        self.ensure();
        let Some(l) = self.links.iter().find(|l| l.local_bank_id == local_id).cloned() else { return };
        self.jobs.clear();
        self.view = EditView::Merge(Box::new(MergeView { local_id: l.local_bank_id.clone(), cid: l.circle_id.clone(), bid: l.circle_bank_id.clone(), title: api::plain(&l.title), perm, phase: MergePhase::Fetching }));
        let w = Work::MergeFetch { cid: l.circle_id, bid: l.circle_bank_id, base: l.base_version, need_base: l.base.is_empty() };
        if !self.launch(w)
            && let EditView::Merge(m) = &mut self.view
        {
            m.phase = MergePhase::Failed(Problem { message: "Not signed in to the pool. Open the account window and sign in.".into(), retry: true, retry_at: None, account: true });
        }
    }

    /// "Try again" on a failed fetch.
    pub fn retry_merge(&mut self) {
        let EditView::Merge(m) = &self.view else { return };
        if matches!(m.phase, MergePhase::Failed(_) | MergePhase::Blocked(_)) {
            let (local, perm) = (m.local_id.clone(), m.perm);
            self.open_merge(&local, perm);
        }
    }

    fn on_fetched(&mut self, cid: &str, bid: &str, r: Result<Fetch, PoolError>) {
        if !self.here(cid, bid) || !matches!(&self.view, EditView::Merge(m) if matches!(m.phase, MergePhase::Fetching)) {
            return;
        }
        let phase = match r {
            Ok(Fetch::Ready(f)) => MergePhase::Ready(f),
            Ok(Fetch::Withheld(start)) => {
                if start.head_version > 0 {
                    self.note_head(cid, bid, start.head_version);
                }
                // Versions other people sent that can be read and that a send over the hidden one replaces.
                let replaces: Vec<VersionLine> = start.between.iter().filter(|l| !l.withheld && l.version < start.head_version).cloned().collect();
                MergePhase::HeadWithheld { head_version: Some(start.head_version).filter(|h| *h > 0), replaces }
            }
            Ok(Fetch::Moved) => MergePhase::Failed(Problem { message: "Someone sent changes while this was loading. Press Try again to start the merge over.".into(), retry: true, retry_at: None, account: false }),
            Err(e) => {
                let kind = if let PoolError::Refused(r) = &e { Some(r.kind.clone()) } else { None };
                match kind {
                    Some(RefusalKind::VersionWithheld) => MergePhase::HeadWithheld { head_version: self.link_head(cid, bid), replaces: Vec::new() },
                    Some(RefusalKind::InvalidRequest { field: Some(f) }) if f == "base_version" => {
                        // The copy is already at the newest version.
                        let local = if let EditView::Merge(m) = &self.view { m.local_id.clone() } else { String::new() };
                        if self.copy_is_stale(&local) { MergePhase::StaleUpToDate { ask: None } } else { MergePhase::UpToDate }
                    }
                    _ => match self.problem_of(&e) {
                        Some(p) => MergePhase::Failed(p),
                        None => {
                            if let EditView::Merge(m) = &mut self.view {
                                m.phase = MergePhase::Failed(Problem { message: "Your session ended. Sign in again, then press Try again.".into(), retry: true, retry_at: None, account: false });
                            }
                            return;
                        }
                    },
                }
            }
        };
        if let EditView::Merge(m) = &mut self.view {
            m.phase = phase;
        }
    }

    fn link_head(&self, cid: &str, bid: &str) -> Option<u64> {
        self.links.iter().find(|l| l.circle_id == cid && l.circle_bank_id == bid).map(|l| l.head_seen).filter(|h| *h > 0)
    }

    /// Builds the review from what was fetched. Needs the banks.
    fn build_review(&mut self, banks: &[QuestionBank]) {
        let EditView::Merge(m) = &mut self.view else { return };
        let MergePhase::Ready(f) = std::mem::replace(&mut m.phase, MergePhase::Fetching) else { return };
        let Some(link) = self.links.iter().find(|l| l.local_bank_id == m.local_id).cloned() else {
            m.phase = MergePhase::Blocked("This working copy is not in the list any more.".into());
            return;
        };
        let Some(bank) = banks.iter().find(|b| b.id == link.local_bank_id) else {
            m.phase = MergePhase::Blocked("The bank is not on this device any more, so there is nothing to merge into.".into());
            return;
        };
        let at = self.clock.unwrap_or_else(now_ms);
        let theirs = circle_theirs(&f.head.payload, at);
        if theirs.is_empty() {
            m.phase = MergePhase::Blocked("The circle's newest version could not be read. Nothing was changed.".into());
            return;
        }
        if payload_question_count(&f.head.payload) > theirs.len() {
            m.phase = MergePhase::Blocked(LocalProblem::NewerQuestions.message());
            return;
        }
        // A bank put back from a saved copy is older than the link: what it was last in step with is not known.
        let stale = link.stale_for(bank);
        let (base, fallback) = if !stale && !link.base.is_empty() {
            (link.base.clone(), false)
        } else if let (false, Some(bd)) = (stale, &f.base) {
            let bt = circle_theirs(&bd.payload, at);
            // A question deleted here stays in the base (so it is "deleted by you"), not "new in the circle".
            let rebuilt: Vec<BaseQuestion> = link
                .qids
                .iter()
                .filter_map(|l| {
                    let t = bt.iter().find(|t| t.qid == l.qid)?;
                    let mut b = merge::base_of(&t.q, &t.qid, &t.fp, t.sha.clone());
                    b.local_id = l.local.clone();
                    b.local_image = bank.questions.iter().find(|q| q.id == l.local).and_then(|q| q.image.as_ref()).map(|i| i.file.clone()).filter(|file| sha_of_file(file) == t.sha);
                    Some(b)
                })
                .collect();
            if rebuilt.is_empty() { (merge::unknown_base(bank, &pairs(&link), &theirs), true) } else { (rebuilt, false) }
        } else {
            (merge::unknown_base(bank, &pairs(&link), &theirs), true)
        };
        let plan = merge::build_plan(&link.circle_bank_id, &base, bank, &theirs, link.base_version, f.start.head_version, None, String::new());
        let start = f.start.clone();
        m.phase = MergePhase::Review(Box::new(Review { plan, fallback, start, problem: self.merge_note.take(), base, ours_print: print_of(bank), fetched: f }));
    }

    pub fn set_merge_choice(&mut self, row: usize, c: Choice) {
        if let EditView::Merge(m) = &mut self.view
            && let MergePhase::Review(r) = &mut m.phase
            && let Some(row) = r.plan.rows.get_mut(row)
            && row.options.contains(&c)
        {
            row.choice = c;
            r.problem = None;
        }
    }

    pub fn can_apply_merge(&self) -> bool {
        matches!(&self.view, EditView::Merge(m) if matches!(&m.phase, MergePhase::Review(r) if r.plan.ready()))
    }

    /// "Apply merge": fetches the newest version with its pictures; the bank changes in `sync`.
    pub fn apply_merge(&mut self) {
        if !self.can_apply_merge() {
            return;
        }
        let EditView::Merge(m) = &mut self.view else { return };
        let (cid, bid) = (m.cid.clone(), m.bid.clone());
        let MergePhase::Review(r) = std::mem::replace(&mut m.phase, MergePhase::Fetching) else { return };
        let version = r.plan.new_version;
        m.phase = MergePhase::Applying(r);
        if !self.launch(Work::MergeApply { cid, bid, version })
            && let EditView::Merge(m) = &mut self.view
            && let MergePhase::Applying(r) = std::mem::replace(&mut m.phase, MergePhase::Fetching)
        {
            let mut r = r;
            r.problem = Some("Not signed in to the pool. Sign in, then press Apply merge again.".into());
            m.phase = MergePhase::Review(r);
        }
    }

    /// On a stale copy with nothing newer in the circle: the bank as it is now is what the link is in
    /// step with. The bank is not changed; the next send sends it as it is.
    /// First click: asks. Nothing changes until the second click ([`rebaseline`](Self::rebaseline)).
    pub fn ask_stale(&mut self, what: StaleAsk) {
        if let EditView::Merge(m) = &mut self.view
            && let MergePhase::StaleUpToDate { ask } = &mut m.phase
        {
            *ask = Some(what);
        }
    }

    pub fn cancel_stale_ask(&mut self) {
        if let EditView::Merge(m) = &mut self.view
            && let MergePhase::StaleUpToDate { ask } = &mut m.phase
        {
            *ask = None;
        }
    }

    /// Second click of "Treat this copy as in step": only after it was asked.
    pub fn rebaseline(&mut self) {
        if matches!(&self.view, EditView::Merge(m) if matches!(m.phase, MergePhase::StaleUpToDate { ask: Some(StaleAsk::Rebase) })) {
            self.cmd_rebase = true;
        }
    }

    /// Second click of "Forget this link" on the stale screen: only after it was asked.
    pub fn forget_stale(&mut self) {
        let local = match &self.view {
            EditView::Merge(m) if matches!(m.phase, MergePhase::StaleUpToDate { ask: Some(StaleAsk::Forget) }) => m.local_id.clone(),
            _ => return,
        };
        self.forget(&local);
        self.cancel_merge();
    }

    /// First click of "Forget this link" on a card.
    pub fn ask_forget(&mut self, local_id: &str) {
        self.forget_ask = Some(local_id.to_string());
    }

    pub fn cancel_forget(&mut self) {
        self.forget_ask = None;
    }

    pub fn forget_asked(&self, local_id: &str) -> bool {
        self.forget_ask.as_deref() == Some(local_id)
    }

    /// Second click: only for the link that was asked about.
    pub fn confirm_forget(&mut self, local_id: &str) {
        if self.forget_asked(local_id) {
            self.forget_ask = None;
            self.forget(local_id);
        }
    }

    fn do_rebaseline(&mut self, banks: &[QuestionBank]) {
        let EditView::Merge(m) = &mut self.view else { return };
        let local = m.local_id.clone();
        let (Some(bank), Some(mut link)) = (banks.iter().find(|b| b.id == local), self.links.iter().find(|l| l.local_bank_id == local).cloned()) else { return };
        link.mark_bank(bank);
        self.statuses.remove(&local);
        self.replace_link(link);
        if let EditView::Merge(m) = &mut self.view {
            m.phase = MergePhase::UpToDate;
        }
        self.notice = Some("This copy is now treated as in step with the circle's newest version. Your bank was not changed; the next send sends it as it is.".into());
    }

    /// Leaves a review or a wait: nothing was changed.
    pub fn cancel_merge(&mut self) {
        if matches!(self.view, EditView::Merge(_)) {
            self.jobs.retain(|j| j.kind != JobKind::Merge);
            self.applying = None;
            self.view = EditView::Closed;
        }
    }

    fn back_to_review(&mut self, why: String) {
        if let EditView::Merge(m) = &mut self.view
            && let MergePhase::Applying(r) = std::mem::replace(&mut m.phase, MergePhase::Fetching)
        {
            let mut r = r;
            r.problem = Some(why);
            m.phase = MergePhase::Review(r);
        }
    }

    fn on_applied(&mut self, cid: &str, bid: &str, r: Result<Option<Box<Imported>>, PoolError>) {
        if !self.here(cid, bid) || !matches!(&self.view, EditView::Merge(m) if matches!(m.phase, MergePhase::Applying(_))) {
            return;
        }
        match r {
            Ok(Some(imp)) => self.applying = Some(imp),
            Ok(None) => self.back_to_review("The circle has a different newest version now than the one you reviewed. Nothing was applied. Cancel and start the merge again.".into()),
            Err(e) => match self.problem_of(&e) {
                Some(p) => self.back_to_review(p.message),
                None => self.back_to_review("Your session ended. Sign in again, then press Apply merge. Nothing was changed.".into()),
            },
        }
    }

    /// The newest version has arrived: restore point first, then the bank, then the link.
    fn finish_merge(&mut self, banks: &mut [QuestionBank], imp: Box<Imported>) -> bool {
        let EditView::Merge(m) = &mut self.view else { return false };
        let local_id = m.local_id.clone();
        let MergePhase::Applying(rev) = std::mem::replace(&mut m.phase, MergePhase::Fetching) else { return false };
        let back = |m: &mut MergeView, mut rev: Box<Review>, why: String| {
            rev.problem = Some(why);
            m.phase = MergePhase::Review(rev);
        };
        let Some(mut link) = self.links.iter().find(|l| l.local_bank_id == local_id).cloned() else {
            back(m, rev, "This working copy is not in the list any more. Nothing was applied.".into());
            return false;
        };
        if imp.head.item.version != rev.plan.new_version {
            back(m, rev, "The circle has a different newest version now than the one you reviewed. Nothing was applied. Cancel and start the merge again.".into());
            return false;
        }
        let Some(pos) = banks.iter().position(|b| b.id == local_id) else {
            back(m, rev, "The bank is not on this device any more. Nothing was applied.".into());
            return false;
        };
        if imp.pictures_missing > 0 {
            let n = imp.pictures_missing;
            back(m, rev, format!("{n} picture{} of the circle's newest version could not be fetched. Nothing was applied, so no picture is lost. Press Apply merge to try again.", if n == 1 { "" } else { "s" }));
            return false;
        }
        if print_of(&banks[pos]) != rev.ours_print {
            // Edited here after the review was built: the review is made again from the bank as it is now.
            let r = *rev;
            self.merge_note = Some("Your bank changed while you were reviewing, so nothing was applied. This is the review again, with your latest changes. Check it, then press Apply merge.".into());
            m.phase = MergePhase::Ready(r.fetched);
            return false;
        }
        let author = api::plain(&imp.head.item.author_name);
        let mut bank = banks[pos].clone();
        let (outcome, new_base) = match merge::apply_merge(&mut bank, &rev.plan, &imp.theirs, &rev.base, &author, imp.at_ms) {
            Ok(a) => a,
            Err(msg) => {
                back(m, rev, msg);
                return false;
            }
        };
        // The restore point holds the banks exactly as they were; if it can not be written, the merge does not happen.
        let restore = match self.restore.save(banks) {
            Ok(w) => w,
            Err(msg) => {
                back(m, rev, format!("A copy of your banks could not be saved first ({msg}), so nothing was changed."));
                return false;
            }
        };
        for q in &mut bank.questions {
            for t in &mut q.topics {
                if let Some(rest) = t.strip_prefix("From the pool: ") {
                    *t = format!("From the circle: {rest}");
                }
            }
        }
        let prev = banks[pos].modified;
        banks[pos] = bank;
        // Seconds, like every editor writes; never the millisecond clock.
        banks[pos].modified = (self.now_ms() / 1000).max(prev.saturating_add(1));
        link.mark_bank(&banks[pos]);
        link.qids = qids_of(&new_base);
        link.base = merge::bounded(new_base);
        link.base_version = imp.head.item.version;
        link.base_version_id = imp.head.item.publish_id.clone();
        link.head_seen = link.head_seen.max(link.base_version);
        let carry = Carry { local_id: local_id.clone(), token: rev.start.merge_token.clone(), from: rev.plan.old_version, head: link.base_version };
        let changes = changes_since(&link, &banks[pos]);
        self.advanced.push(Advanced { circle_bank_id: link.circle_bank_id.clone(), local_bank_id: local_id.clone(), version: link.base_version });
        let version = link.base_version;
        self.statuses.remove(&local_id);
        self.replace_link(link);
        // The next send, if there is something to send, goes with the token.
        self.carry = (changes.total() > 0).then_some(carry);
        if let EditView::Merge(m) = &mut self.view {
            m.phase = MergePhase::Applied(Box::new(MergeDone { outcome, restore, version, changes }));
        }
        true
    }

    // ---- the history -------------------------------------------------------------------------------------------

    pub fn open_history(&mut self, cid: &str, bid: &str, title: &str, perm: Perm) {
        if !super::pool::is_item_id(cid) || !super::pool::is_item_id(bid) {
            return;
        }
        self.ensure();
        self.jobs.clear();
        self.history_stash = None;
        let local_id = self.links.iter().find(|l| l.circle_id == cid && l.circle_bank_id == bid).map(|l| l.local_bank_id.clone());
        self.view = EditView::History(Box::new(HistoryView {
            cid: cid.to_string(),
            bid: bid.to_string(),
            local_id,
            title: api::plain(title),
            perm,
            loaded: false,
            loading_more: false,
            head_version: 0,
            oldest_kept: None,
            pinned: Vec::new(),
            lines: Vec::new(),
            next_before: None,
            problem: None,
            restore: None,
            trim: None,
            keys_closed: Default::default(),
        }));
        self.load_history(None);
    }

    fn load_history(&mut self, before: Option<u64>) {
        let EditView::History(h) = &mut self.view else { return };
        h.problem = None;
        if before.is_some() {
            h.loading_more = true;
        }
        let w = Work::Versions { cid: h.cid.clone(), bid: h.bid.clone(), before };
        if !self.launch(w)
            && let EditView::History(h) = &mut self.view
        {
            h.loading_more = false;
            h.problem = Some(Problem { message: "Not signed in to the pool. Open the account window and sign in.".into(), retry: true, retry_at: None, account: true });
        }
    }

    /// "Older versions".
    pub fn more_history(&mut self) {
        let EditView::History(h) = &self.view else { return };
        if let Some(b) = h.next_before.filter(|_| !h.loading_more && h.loaded) {
            self.load_history(Some(b));
        }
    }

    /// "Try again" / "Reload" on the history.
    pub fn reload_history(&mut self) {
        if let EditView::History(h) = &mut self.view {
            h.loaded = false;
            h.lines.clear();
            h.next_before = None;
            self.load_history(None);
        }
    }

    fn on_versions(&mut self, cid: &str, bid: &str, first: bool, r: Result<VersionsPage, PoolError>) {
        if !self.here(cid, bid) || !matches!(self.view, EditView::History(_)) {
            return;
        }
        match r {
            Ok(page) => {
                let head = page.head_version;
                if let EditView::History(h) = &mut self.view {
                    h.loaded = true;
                    h.loading_more = false;
                    h.head_version = page.head_version;
                    h.oldest_kept = page.oldest_kept;
                    h.pinned = page.pinned_versions;
                    h.next_before = page.next_before;
                    if first {
                        h.lines = page.versions;
                    } else {
                        for l in page.versions {
                            if !h.lines.iter().any(|x| x.version == l.version) {
                                h.lines.push(l);
                            }
                        }
                    }
                    h.problem = None;
                }
                self.note_head(cid, bid, head);
            }
            Err(e) => {
                let p = self.problem_of(&e);
                if let EditView::History(h) = &mut self.view {
                    h.loading_more = false;
                    h.problem = p.or(Some(Problem { message: "Your session ended. Sign in again, then press Try again.".into(), retry: true, retry_at: None, account: false }));
                }
            }
        }
    }

    /// What a read says about the newest version, for the badge.
    fn note_head(&mut self, cid: &str, bid: &str, head: u64) {
        if head == 0 {
            return;
        }
        if let Some(l) = self.links.iter().find(|l| l.circle_id == cid && l.circle_bank_id == bid && l.head_seen < head).cloned() {
            let mut l = l;
            l.head_seen = head;
            self.replace_link(l);
        }
    }

    // ---- looking at a version ---------------------------------------------------------------------------------

    pub fn view_version(&mut self, n: u64) {
        let EditView::History(h) = &self.view else { return };
        let line = h.lines.iter().find(|l| l.version == n).cloned();
        if !line.as_ref().is_some_and(|l| h.can_look(l)) {
            return;
        }
        let (cid, bid) = (h.cid.clone(), h.bid.clone());
        let EditView::History(h) = std::mem::take(&mut self.view) else { return };
        self.history_stash = Some(h);
        self.view = EditView::Version(Box::new(VersionView { cid: cid.clone(), bid: bid.clone(), n, line, load: VersionLoad::Loading }));
        if !self.launch(Work::Version { cid, bid, n })
            && let EditView::Version(v) = &mut self.view
        {
            v.load = VersionLoad::Failed(Problem { message: "Not signed in to the pool. Open the account window and sign in.".into(), retry: true, retry_at: None, account: true });
        }
    }

    fn on_version(&mut self, cid: &str, bid: &str, n: u64, r: Result<BankDoc, PoolError>) {
        if !matches!(&self.view, EditView::Version(v) if v.cid == cid && v.bid == bid && v.n == n) {
            return;
        }
        // (critic 58 I3) No viewers' copy of this version: it is not offered again (critic 59 I1: only it).
        if matches!(r.as_ref().err().and_then(PoolError::kind), Some(RefusalKind::KeysHiddenUnavailable))
            && let Some(h) = self.history_stash.as_mut()
        {
            h.keys_closed.insert(n);
        }
        let load = match r {
            Ok(doc) => VersionLoad::Ready(Box::new(VersionBody { title: doc.bank.title.clone(), license: if doc.bank.version_license.is_empty() { doc.bank.license.clone() } else { doc.bank.version_license.clone() }, questions: question_lines(&doc.payload), hide_key: doc.bank.hide_key, keys_hidden: doc.bank.keys_hidden })),
            Err(e) => match self.problem_of(&e) {
                Some(p) => VersionLoad::Failed(p),
                None => VersionLoad::Failed(Problem { message: "Your session ended. Sign in again, then press Try again.".into(), retry: true, retry_at: None, account: false }),
            },
        };
        if let EditView::Version(v) = &mut self.view {
            v.load = load;
        }
    }

    pub fn retry_version(&mut self) {
        let EditView::Version(v) = &mut self.view else { return };
        if matches!(v.load, VersionLoad::Failed(_)) {
            v.load = VersionLoad::Loading;
            let w = Work::Version { cid: v.cid.clone(), bid: v.bid.clone(), n: v.n };
            self.launch(w);
        }
    }

    /// Back from a version to the history (or out, when there is none).
    pub fn back(&mut self) {
        match std::mem::take(&mut self.view) {
            EditView::Version(_) => {
                self.jobs.retain(|j| j.kind != JobKind::Version);
                self.view = self.history_stash.take().map_or(EditView::Closed, EditView::History);
            }
            _ => {
                self.jobs.clear();
                self.history_stash = None;
                self.view = EditView::Closed;
            }
        }
    }

    // ---- restore -----------------------------------------------------------------------------------------------

    /// "Restore this version...": asks first.
    pub fn ask_restore(&mut self, to: u64) {
        let EditView::History(h) = &mut self.view else { return };
        let ok = h.perm.can_rewind() && h.lines.iter().any(|l| l.version == to && l.kept && !l.withheld) && to < h.head_version;
        if ok && h.restore.is_none() && !matches!(h.trim.as_ref().map(|t| &t.stage), Some(TrimStage::Working)) {
            h.restore = Some(RestoreAsk { to, stage: RestoreStage::Confirm });
        }
    }

    pub fn cancel_restore(&mut self) {
        if let EditView::History(h) = &mut self.view
            && !matches!(h.restore.as_ref().map(|r| &r.stage), Some(RestoreStage::Working))
        {
            h.restore = None;
        }
    }

    pub fn confirm_restore(&mut self, large_ok: bool) {
        let acting = self.running(JobKind::Act);
        let EditView::History(h) = &mut self.view else { return };
        let Some(r) = &mut h.restore else { return };
        if !h.perm.can_rewind() || matches!(r.stage, RestoreStage::Working | RestoreStage::Done { .. }) || acting {
            return;
        }
        let confirm = large_ok && matches!(r.stage, RestoreStage::Large { .. });
        let (cid, bid, base, to) = (h.cid.clone(), h.bid.clone(), h.head_version, r.to);
        let print = format!("{cid}|{bid}|{base}|{to}|{confirm}");
        let id = match &self.pending_restore {
            Some((p, id)) if *p == print => id.clone(),
            _ => {
                let id = super::pool::new_publish_id();
                self.pending_restore = Some((print, id.clone()));
                id
            }
        };
        r.stage = RestoreStage::Working;
        self.last_act = Some(ActAgain::Restore(to));
        if !self.launch(Work::Rollback { cid, bid, commit_id: id, base, to, confirm })
            && let EditView::History(h) = &mut self.view
            && let Some(r) = &mut h.restore
        {
            r.stage = RestoreStage::Failed(Problem { message: "Not signed in to the pool. Open the account window and sign in.".into(), retry: true, retry_at: None, account: true });
        }
    }

    fn on_rolled(&mut self, cid: &str, bid: &str, to: u64, r: Result<SentBank, PoolError>) {
        if !matches!(&self.view, EditView::History(h) if h.cid == cid && h.bid == bid && h.restore.as_ref().is_some_and(|x| x.to == to)) {
            return;
        }
        let stage = match r {
            Ok(sent) => {
                self.pending_restore = None;
                Some(RestoreStage::Done { version: sent.version, held: sent.held, withheld: sent.withheld })
            }
            Err(e) => match &e {
                PoolError::Refused(rf) if matches!(rf.kind, RefusalKind::ConfirmLarge { .. }) => {
                    let RefusalKind::ConfirmLarge { removed, total } = rf.kind else { unreachable!() };
                    Some(RestoreStage::Large { removed, total })
                }
                PoolError::Refused(rf) if matches!(rf.kind, RefusalKind::CircleConflict { .. }) => {
                    self.pending_restore = None;
                    Some(RestoreStage::Failed(Problem { message: "A newer version arrived while you were looking. The history has been reloaded; choose the version again.".into(), retry: false, retry_at: None, account: false }))
                }
                PoolError::Refused(rf) if matches!(rf.kind, RefusalKind::CommitIdConflict) => {
                    self.pending_restore = None;
                    Some(RestoreStage::Failed(Problem { message: "This restore was already used for something else. Press Restore again to start it again.".into(), retry: true, retry_at: None, account: false }))
                }
                PoolError::Refused(rf) if matches!(rf.kind, RefusalKind::HideKeyChanged { .. }) => {
                    // (round 48: nothing was stored; a restore says "restore again", not "send again")
                    self.pending_restore = None;
                    Some(RestoreStage::Failed(Problem { message: "The bank's answer-key setting changed while this was on its way, so nothing was stored. Press Restore again.".into(), retry: true, retry_at: None, account: false }))
                }
                _ => self.problem_of(&e).map(RestoreStage::Failed),
            },
        };
        let reload = matches!(stage, Some(RestoreStage::Done { .. })) || matches!(&stage, Some(RestoreStage::Failed(p)) if p.message.starts_with("A newer version"));
        if let EditView::History(h) = &mut self.view
            && let Some(x) = &mut h.restore
        {
            x.stage = stage.unwrap_or(RestoreStage::Confirm);
        }
        if reload {
            self.reload_history_keeping_ask();
        }
    }

    fn reload_history_keeping_ask(&mut self) {
        if let EditView::History(h) = &mut self.view {
            h.loaded = false;
            h.lines.clear();
            h.next_before = None;
        }
        self.load_history(None);
    }

    pub fn close_restore_result(&mut self) {
        if let EditView::History(h) = &mut self.view
            && matches!(h.restore.as_ref().map(|r| &r.stage), Some(RestoreStage::Done { .. } | RestoreStage::Failed(_)))
        {
            h.restore = None;
        }
    }

    // ---- trim ---------------------------------------------------------------------------------------------------

    pub fn open_trim(&mut self) {
        if let EditView::History(h) = &mut self.view
            && h.perm.can_rewind()
            && h.trim.is_none()
            && h.restore.is_none()
        {
            h.trim = Some(TrimAsk { keep: 10, stage: TrimStage::Editing });
        }
    }

    pub fn set_trim_keep(&mut self, keep: u32) {
        if let EditView::History(h) = &mut self.view
            && let Some(t) = &mut h.trim
            && matches!(t.stage, TrimStage::Editing)
        {
            t.keep = keep.clamp(1, KEPT_VERSIONS - 1);
        }
    }

    /// "Trim...": asks once more.
    pub fn ask_trim(&mut self) {
        if let EditView::History(h) = &mut self.view
            && let Some(t) = &mut h.trim
            && matches!(t.stage, TrimStage::Editing)
        {
            t.stage = TrimStage::Confirm;
        }
    }

    pub fn cancel_trim(&mut self) {
        if let EditView::History(h) = &mut self.view
            && !matches!(h.trim.as_ref().map(|t| &t.stage), Some(TrimStage::Working))
        {
            h.trim = None;
        }
    }

    pub fn confirm_trim(&mut self) {
        let acting = self.running(JobKind::Act);
        let EditView::History(h) = &mut self.view else { return };
        let Some(t) = &mut h.trim else { return };
        if !h.perm.can_rewind() || !matches!(t.stage, TrimStage::Confirm | TrimStage::Failed(_)) || acting {
            return;
        }
        let keep = t.keep;
        let (cid, bid) = (h.cid.clone(), h.bid.clone());
        t.stage = TrimStage::Working;
        self.last_act = Some(ActAgain::Trim(keep));
        if !self.launch(Work::Trim { cid, bid, keep })
            && let EditView::History(h) = &mut self.view
            && let Some(t) = &mut h.trim
        {
            t.stage = TrimStage::Failed(Problem { message: "Not signed in to the pool. Open the account window and sign in.".into(), retry: true, retry_at: None, account: true });
        }
    }

    fn on_trimmed(&mut self, cid: &str, bid: &str, r: Result<Trimmed, PoolError>) {
        if !matches!(&self.view, EditView::History(h) if h.cid == cid && h.bid == bid && h.trim.is_some()) {
            return;
        }
        let stage = match r {
            Ok(t) => TrimStage::Done(t),
            Err(e) => match self.problem_of(&e) {
                Some(p) => TrimStage::Failed(p),
                None => TrimStage::Confirm,
            },
        };
        let done = matches!(stage, TrimStage::Done(_));
        if let EditView::History(h) = &mut self.view
            && let Some(t) = &mut h.trim
        {
            t.stage = stage;
        }
        if done {
            self.reload_history_keeping_ask();
        }
    }

    pub fn close_trim_result(&mut self) {
        if let EditView::History(h) = &mut self.view
            && matches!(h.trim.as_ref().map(|t| &t.stage), Some(TrimStage::Done(_) | TrimStage::Failed(_)))
        {
            h.trim = None;
        }
    }

    /// The last change again after a failure ("Try again").
    pub fn retry_act(&mut self) {
        match self.last_act {
            Some(ActAgain::Send) => {
                if matches!(&self.view, EditView::Send(v) if matches!(v.stage, SendStage::Failed(_))) {
                    // (the pictures kept go again: the pool never asks about them, so they would be lost)
                    let keep = match &mut self.view {
                        EditView::Send(v) => v.picked.take().map(|p| (p, std::mem::take(&mut v.decisions.image_ok), v.decisions_for.clone())),
                        _ => None,
                    };
                    self.back_to_note();
                    if let (EditView::Send(v), Some((p, image_ok, Some(sha)))) = (&mut self.view, keep) {
                        v.decisions.image_ok = image_ok;
                        v.decisions_for = Some(sha);
                        v.picked = Some(p);
                    }
                    self.send();
                }
            }
            Some(ActAgain::Restore(_)) => {
                if matches!(&self.view, EditView::History(h) if h.restore.as_ref().is_some_and(|r| matches!(r.stage, RestoreStage::Failed(_)))) {
                    if let EditView::History(h) = &mut self.view
                        && let Some(r) = &mut h.restore
                    {
                        r.stage = RestoreStage::Confirm;
                    }
                    self.confirm_restore(false);
                }
            }
            Some(ActAgain::Trim(_)) => self.confirm_trim(),
            None => {}
        }
    }

    // ---- sync -----------------------------------------------------------------------------------------------------

    /// Does what needs the banks: counts what changed, starts a send, builds a review, applies a
    /// merge. Returns true when a bank changed (the caller saves the banks).
    pub fn sync(&mut self, banks: &mut [QuestionBank]) -> bool {
        self.ensure();
        self.refresh_statuses(banks);
        if let EditView::Send(v) = &mut self.view
            && let Some(s) = self.statuses.get(&v.local_id)
        {
            v.changes = Some(s.changes);
        }
        if self.cmd_send {
            self.cmd_send = false;
            self.do_send(banks);
        }
        if std::mem::take(&mut self.cmd_rebase) {
            self.do_rebaseline(banks);
        }
        if matches!(&self.view, EditView::Merge(m) if matches!(m.phase, MergePhase::Ready(_))) {
            self.build_review(banks);
        }
        if let Some(imp) = self.applying.take() {
            return self.finish_merge(banks, imp);
        }
        false
    }

    fn refresh_statuses(&mut self, banks: &[QuestionBank]) {
        for l in &self.links {
            let bank = banks.iter().find(|b| b.id == l.local_bank_id);
            let key = bank.map_or((0, 0), |b| (b.modified, b.questions.len()));
            let stale = bank.is_some_and(|b| l.stale_for(b));
            let fresh = self.statuses.get(&l.local_bank_id).is_some_and(|s| s.key == key && s.missing == bank.is_none() && s.stale == stale);
            if fresh {
                continue;
            }
            let changes = bank.map(|b| changes_since(l, b)).unwrap_or_default();
            self.statuses.insert(l.local_bank_id.clone(), LinkStatus { key, changes, missing: bank.is_none(), stale });
        }
    }
}

/// What the bank's questions are right now, to notice an edit made while a review was open.
fn print_of(bank: &QuestionBank) -> String {
    bank_print(bank)
}

fn pairs(l: &CircleLink) -> Vec<(String, String)> {
    l.qids.iter().map(|q| (q.local.clone(), q.qid.clone())).collect()
}

/// The questions of a version, one line each, for looking at it.
fn question_lines(payload: &[u8]) -> Vec<QuestionLine> {
    let v: serde_json::Value = serde_json::from_slice(payload).unwrap_or(serde_json::Value::Null);
    let Some(qs) = v.pointer("/bank/questions").and_then(|q| q.as_array()) else { return Vec::new() };
    qs.iter()
        .enumerate()
        .map(|(i, q)| QuestionLine {
            number: i + 1,
            stem: api::plain(q.get("stem").and_then(|s| s.as_str()).unwrap_or("")).lines().next().unwrap_or("").chars().take(160).collect(),
            kind: q.get("kind").and_then(|k| k.as_object()).and_then(|o| o.keys().next().cloned()).unwrap_or_default(),
            has_image: q.pointer("/image/file").is_some(),
        })
        .collect()
}

// ---- words ---------------------------------------------------------------------------------------------------------

/// What to do about a hidden version or a held bank: the bank's author and the owner can fix it;
/// anyone else has to ask them (the bank is not even listed for an editor while it is held).
fn hidden_lines(held: bool, withheld: bool, can_fix: bool) -> Vec<String> {
    // (round 44: the pool does not say why a version is held back, student information or a copyright match, and the
    // app does not guess)
    let mut out = Vec::new();
    if withheld {
        out.push(if can_fix {
            "This version is held back while an administrator reviews it: something in it matched what the pool checks for, so nobody can open it until an administrator releases it. Change the questions that may be the cause and send a clean version.".to_string()
        } else {
            "This version is held back while an administrator reviews it: something in it matched what the pool checks for, so nobody can open it until an administrator releases it. Only the bank's author or the circle's owner can send a clean version. Ask one of them.".to_string()
        });
    }
    if held {
        out.push(if can_fix {
            "The bank is hidden from the rest of the circle for now while it is reviewed. Your questions are safe, and a clean version sent later lifts this.".to_string()
        } else {
            "The bank is hidden from the rest of the circle for now while it is reviewed, and you will not see it either. Your questions are safe on this device. The bank's author or the circle's owner can lift this by sending a clean version.".to_string()
        });
    }
    out
}

/// The lines under "Sent": what happened, in plain words. `can_fix`: the owner, or the bank's author.
pub fn sent_lines(s: &Sent, can_fix: bool) -> Vec<String> {
    let mut out = vec![format!("Version {} is now the circle's newest. {} added, {} changed, {} removed.", s.version, s.added, s.changed, s.removed)];
    if s.replayed {
        out.push("This send had already gone through; nothing was sent twice.".into());
    }
    if let Some(m) = s.merged_from {
        out.push(format!("It records that you merged in the changes made since version {m}."));
    }
    out.extend(hidden_lines(s.held, s.withheld, can_fix));
    out.extend(super::circles_text::pictures_left_out_line(&s.left_out_unasked));
    out.extend(super::circles_text::pictures_not_asked_line(&s.not_asked, s.ask_next));
    if let Some(d) = &s.hiding_stopped {
        out.push(super::circles_text::hiding_stopped_line(d));
    }
    out
}

/// The lines under a finished restore: what happened, including when the restored content was held.
pub fn restore_lines(version: u64, to: u64, held: bool, withheld: bool, can_fix: bool) -> Vec<String> {
    if withheld || held {
        let mut out = vec![format!("Version {version} was made from version {to}, but it is not shown to the circle yet.")];
        // A restore sends nothing from this computer: "take it out and send again" would point at the
        // wrong questions, so the advice names what a restore can do instead.
        if withheld && can_fix {
            out.push(format!("It is held back while an administrator reviews it: something in version {to} matched what the pool checks for, so nobody can open the new version until an administrator releases it. To give the circle a clean newest version, restore a different version, or send a clean one from a working copy."));
            out.extend(hidden_lines(held, false, can_fix));
            return out;
        }
        out.extend(hidden_lines(held, withheld, can_fix));
        return out;
    }
    vec![format!("Done. Version {version} is now the circle's newest, with the content of version {to}."), "Everyone with a working copy sees it as a new version, and merges it in when they choose.".into()]
}

/// What a trim did, in words: how many versions went, and the ones it could not delete (kept for an
/// administrator's review: only an administrator frees that space).
pub fn trim_lines(t: &Trimmed) -> Vec<String> {
    let mut out = vec![match t.pruned {
        0 => "Nothing was deleted: no older version could be trimmed.".to_string(),
        1 => "Done. The content of 1 older version was deleted.".to_string(),
        n => format!("Done. The content of {n} older versions was deleted."),
    }];
    let kept = t.evidence_kept.max(t.pinned_versions.len() as u64);
    if kept > 0 {
        let which = if t.pinned_versions.is_empty() { String::new() } else { format!(" ({})", t.pinned_versions.iter().map(|v| format!("version {v}")).collect::<Vec<_>>().join(", ")) };
        out.push(format!(
            "{}{which} {} kept while an administrator reviews {}. A trim can not delete {}: only an administrator can free that space.",
            if kept == 1 { "1 version".to_string() } else { format!("{kept} versions") },
            if kept == 1 { "is" } else { "are" },
            if kept == 1 { "it" } else { "them" },
            if kept == 1 { "it" } else { "them" },
        ));
    }
    out
}

/// The tag of a question's kind, as the pool names it (`Mc`, `Tf`, `Short`, ...).
fn kind_tag(k: &crate::model::Kind) -> &'static str {
    use crate::model::Kind;
    match k {
        Kind::Mc { .. } => "Mc",
        Kind::Tf { .. } => "Tf",
        Kind::Matching { .. } => "Matching",
        Kind::Blank { .. } => "Blank",
        Kind::Short { .. } => "Short",
        Kind::Multipart { .. } => "Multipart",
    }
}

/// "N new versions" for a working copy that is behind.
pub fn behind_line(n: u64) -> String {
    if n == 1 { "1 new version in the circle".into() } else { format!("{n} new versions in the circle") }
}

/// What a version line says about itself.
pub fn version_flags(l: &VersionLine, pinned: &[u64]) -> Vec<&'static str> {
    let mut v = Vec::new();
    // (critic 56a V7: a withheld line is said as withheld, never as "trimmed", whatever `kept` says)
    if l.fingerprints_only() {
        v.push("Hidden while it is reviewed");
        v.push("Kept for review as fingerprints only");
    } else if l.withheld {
        v.push("Hidden while it is reviewed");
    } else if !l.kept {
        v.push("No longer kept");
    } else if pinned.contains(&l.version) {
        v.push("Kept for a review");
    }
    v
}

/// "Someone else sent changes first" with who and when, when the pool said.
pub fn conflict_line(c: &ConflictInfo, now: i64) -> String {
    let who = c.head_by.as_deref().map(api::plain).filter(|s| !s.is_empty());
    match (who, c.head_at) {
        (Some(w), Some(at)) => {
            let when = text::ago(at, now);
            match (c.head_version, when.is_empty()) {
                (Some(v), false) => format!("{w} sent version {v}, {when}."),
                (Some(v), true) => format!("{w} sent version {v}."),
                (None, false) => format!("{w} sent a newer version {when}."),
                (None, true) => format!("{w} sent a newer version."),
            }
        }
        (Some(w), None) => format!("{w} sent a newer version."),
        _ => "A newer version was sent.".into(),
    }
}

impl From<LocalProblem> for Problem {
    fn from(l: LocalProblem) -> Problem {
        Problem { message: PoolError::Local(l).message(), retry: false, retry_at: None, account: false }
    }
}
