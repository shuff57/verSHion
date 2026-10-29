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

use std::collections::{BTreeSet, HashMap};
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
use super::act_results::{ActResults, ResultBank, ResultKind};
use super::circles_text as text;
use super::merge::{self, BaseQuestion, Choice, MergeOutcome, MergePlan, RowKind};
use super::model::Runner;
use super::partial_send::{BodyChanges, ComposedPartial, ComposedQuestion, SendRow, changes_against_body, compose_partial_parts, send_rows};
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
    /// (phase D2, plan 4.5) One question's history in its circle, opened from its card's menu on the bank's page.
    Question(Box<super::question_history_ui::QuestionPage>),
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

/// (phase D3, plan 4.7 and 6) Where the send summary stands. The base version's body is read once when the page opens,
/// because the link's snapshot (`merge::Snap`) has no `figure` and not always a `difficulty`, so it can neither list
/// every change nor rebuild an unticked question.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PartialSend {
    /// The base version's body is on its way.
    Reading,
    /// The changes against `body`, the body of version `base`, read when the page opened. `print` is the bank's
    /// fingerprint then: a bank that changed since is listed again by `do_send` before anything is sent. `ticked` holds
    /// the local ids to send: every change at first, except the ones in `link.held_back` (the person left them unticked
    /// at the last send). `rows` is the same list as the page draws it (kind, the bank's own question number, the stem).
    Ready { base: u64, body: Vec<u8>, print: String, changes: BodyChanges, ticked: BTreeSet<String>, rows: Vec<SendRow> },
    /// The base can not be had because the circle no longer keeps it (or holds it back), so choosing which changes to
    /// send is not possible: the whole bank is sent, and the words say why in plain language (plan 4.7).
    Unavailable(String),
    /// The base read failed for a reason that may pass (no connection, the server's own trouble, the daily limit, a
    /// session that ended): the page stays, nothing is sent, and Try again runs the one read again. Never the
    /// whole-bank path: that is only for a version the circle no longer keeps.
    ReadFailed(String),
}

/// (phase D3, plan 4.7) Why the send summary can not tick changes: the base version is no longer kept (or is held back).
/// The whole bank may be sent, said plainly on the page.
pub const PARTIAL_UNAVAILABLE: &str = "Choosing which changes to send needs the version your copy is based on, and the circle no longer keeps it. Send everything, or get the latest first.";

/// (phase D3) The base read failed for a reason that may pass: nothing is sent until it is read (Try again).
pub const PARTIAL_READ_FAILED: &str = "The version your copy is based on could not be read just now, so nothing is sent. Try again.";

/// (phase D3) What the Send button says while the base version is being read.
pub const PARTIAL_READING: &str = "Checking the version your copy is based on...";

/// (phase D3, plan 4.7) Nothing is ticked: the send refuses and the page stays as it is.
pub const TICK_ONE: &str = "Tick at least one change";

/// (phase D3) The bank changed while the person was choosing: the list is worked out again and nothing is sent yet.
pub const SEND_LIST_AGAIN: &str = "Your bank changed while you were choosing what to send, so nothing was sent. This is the list again.";

/// (phase D3) A send over a held version goes as the whole bank: changes can not be chosen against a base that is
/// not the version it goes over.
pub const SEND_ON_TOP_WHOLE: &str = "This send goes over a version that is held back while it is reviewed, so your whole bank is sent. Changes can not be chosen for it.";

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
    /// (phase D3) The tick list of a partial send, and the base version's body it was worked out against.
    pub partial: PartialSend,
    /// (phase D3) The base version's body once read (`version`, bytes): the bank is needed to work out the changes, and
    /// `sync` is the only place that has it. Taken by `resolve_partial`.
    pub base_body: Option<(u64, Vec<u8>)>,
    /// (phase D3) A line for the page: the bank changed while the person was choosing, or a held version sends the
    /// whole bank. The drawing comes with the tick boxes.
    pub notice: Option<String>,
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

/// Why a copy that follows the circle can not send (phase D1).
pub const FOLLOW_NO_SEND: &str = "This copy follows the circle: it gets newer versions, but changes can't be sent from it.";

/// Why no bank is linked while the links file could not be read (critic 65 B1).
pub fn unreadable_words(m: &str) -> String {
    format!("the list of shared banks on this computer couldn't be read ({m}), so no bank was linked")
}

/// Why a second bank can not take a circle bank's link (critic 63 B3).
pub fn already_linked(name: &str) -> String {
    let name = api::plain(name);
    if name.is_empty() {
        "another bank in My Banks is already connected to this circle bank, so this one is not; that bank and its changes stay as they are".into()
    } else {
        format!("your bank \"{name}\" is already connected to this circle bank, so this one is not; \"{name}\" and its changes stay as they are")
    }
}

/// Get latest on a copy that has its answer keys, when the circle serves the person the viewers' form (critic 63 B1).
pub const KEYS_KEPT: &str = "The circle now shows you this bank without its answer keys: they are hidden from viewers. Your copy has its answer keys, and getting this version would take them out, so nothing was changed. Your copy stays as it is. An editor or the circle's owner reads the bank with its keys.";

/// (critic 67 M2) The same for a copy made from the viewers' form, reviewed with the keys and then read without them.
pub const KEYS_GONE_AT_APPLY: &str = "The circle now shows you this bank without its answer keys again, so the version you reviewed with them can't be applied. Nothing was changed. Cancel, then press Get latest again.";

/// (critic 71 minor 4) A send, restore or trim another account asked for before this one signed in is still on its
/// way: the strip of its bank says neither what it is nor "up to date" meanwhile.
/// (critic 71 minor 2, critic 74 I1) A send that went through whose answer had no version the app could read, said on
/// the bank's strip.
pub const SENT_UNREAD: &str = "The version was sent, but the server's answer could not be read. Press Get latest to catch up.";
/// The same, in the "Sent" box of the Send page.
pub const SENT_UNREAD_PAGE: &str = "Your changes were sent, but the circle's answer could not be read, so the new version's number is not known here. Press Get latest to catch up.";

/// (critic 73 minor 3) A signed-out answer about a session that ended before the account signed in again.
pub const EARLIER_SESSION: &str = "Your session ended before this was answered, so it may not have gone through. Press Try again: if it did, nothing is done twice.";
/// The same for a send, on the bank's strip.
pub const LEFT_EARLIER_SESSION: &str = "Your session ended before the send was answered. Press Send my changes to send them again; if it went through, nothing is sent twice.";

/// (critic 76 I3) A send refused because the person can only view the circle's banks now.
pub const SEND_ROLE_LEFT: &str = "You can only look at this circle's banks now. The owner can let you send changes by making you an editor.";

pub const OTHER_ACT_LINE: &str = "Waiting for an earlier request on this computer to finish...";
pub const OTHER_ACT_WAIT: &str = "Wait for an earlier request on this computer to finish, then try again.";

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
        format!("Licence: {full}. A send keeps the bank's own licence; you can change it from Bank settings on the circle's page.")
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
    /// (critic 83 I2) The version a send with no readable answer made, when colleagues sent more after it: what the
    /// questions it carried are compared from (`None`: not needed, or not readable).
    pub mine: Option<BankDoc>,
}

#[derive(Debug)]
pub struct Review {
    pub plan: MergePlan,
    /// The earlier version is not known: every difference is a decision.
    pub fallback: bool,
    pub start: MergeStart,
    pub problem: Option<String>,
    /// (critic 80 I1) The version that is the person's own send whose answer did not come back (its questions are
    /// matched to the bank's), said plainly on the review.
    pub own_version: Option<u64>,
    /// (critic 80 I1) The questions whose earlier version here is what that send carried (by their id in the bank): the
    /// review compares them with the circle's newest only, never under the old version's number.
    pub resent: Vec<String>,
    /// (critic 86 I2) The version those questions were matched to: the person's own (an older one, when colleagues sent
    /// more since and it could be read), or the newest.
    pub resent_at: u64,
    /// (critic 85 m1) The version a send with no readable answer may have made, when colleagues sent more since and it can
    /// not be read: what that send carried could only be matched to the newest, so the review says so.
    pub own_unread: Option<u64>,
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
    /// (round 17, critic 96 I2) Opened by "Use my bank as it is...": the page is titled for it, and "No, go back" leaves it.
    pub as_is: bool,
}

#[derive(Debug)]
pub enum RestoreStage {
    Confirm,
    Working,
    /// 428: the restored version drops more than half of the questions.
    Large { removed: u64, total: u64 },
    Done { version: u64, held: bool, withheld: bool },
    Failed(Problem),
    /// (critic 77 I2) No clear answer came (none, an unreadable one, the server failed on its side): it may have gone
    /// through. Why, in words. Try again goes as the same restore, which the circle never makes twice.
    Unclear(String),
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
    /// (critic 77 I2) No clear answer came: it may have gone through. Why, in words.
    Unclear(String),
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
    /// The bank's name in My Banks (empty when it is missing).
    pub bank_name: String,
    /// (critic 85 I1) The link's base kept no difficulty for some question when this was worked out.
    unknown: bool,
    /// (round 15) What `changes_since` counted for the bank as it is (`changes` says it as the link stands now).
    raw: ChangeCount,
    /// (round 15, critics 91 I2 and I4) A link an earlier build wrote whose difficulties the circle has not told yet: where its
    /// read stands. Until it has succeeded nothing is sent and nothing is said to be up to date. `None`: nothing awaited.
    pub learning: Option<Learning>,
    /// (round 17, critic 96 minor 4) `Learning::Held`: why, in plain words, and how many tries failed in a row.
    pub held: Option<(String, u32)>,
    /// (round 17, critic 96 minor 3) `Learning::Limited`: how long until the circle's daily limit resets, as it said (seconds).
    pub limit_left: Option<u64>,
    /// (round 18, critic 98 minor 3) `Learning::NotKept`: why the version can't be read.
    pub unread: Option<Unread>,
    /// (round 19, critic 100 I2) The banks file could not be written for a change she asked for here: what the strip says, and the
    /// tries that failed. Her own edits are not saved either until a save works.
    pub unsaved: Option<(String, u32)>,
    /// (round 19, critic 100 minor 5) The link has difficulty news with something left for her to do (see `news_settled`).
    pub news: bool,
}

/// (round 18, critic 98 minor 3) Why the circle's version a copy last matched can't be read (`Learning::NotKept`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Unread {
    /// The circle no longer keeps it.
    NotKept,
    /// It is held back while an administrator reviews it: waiting is enough.
    Withheld,
    /// Its reply could not be read (or was not that version).
    Other,
}

/// (round 15) Where the read of a link's base version stands, while the copy waits for it (see `LinkStatus::learning`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Learning {
    /// It is on its way, or about to be asked.
    Checking,
    /// The circle could not be reached: asked again after `BASE_RETRY`, or at once on Try again.
    CantReach,
    /// The circle would not show that version (no longer kept, held back, a reply that can't be read): the difficulties are
    /// not known. Asked again later (see `BASE_UNREADABLE_RETRY`), or at once on Check now.
    NotKept,
    /// (round 16, critic 93 minor 4) The circle refused the read for today (its daily limit): asked again when it said.
    Limited,
    /// (round 16, critic 93 I1) The read succeeded, but the circle's ratings could not be put into the bank (its restore point
    /// or the banks file could not be saved): tried again after `BASE_RETRY`, or at once on Try again.
    Held,
}

/// (round 15) A link an earlier build wrote whose difficulties the circle has not told yet, from a copy that sends: its base
/// version is to be read, and until that has succeeded nothing is sent from it and its count is never complete.
pub fn awaits_read(l: &CircleLink) -> bool {
    needs_difficulty_read(l) && !l.gone() && !l.seen.removed && l.can_send() && l.role() != Some(Role::Viewer)
}

/// (round 15, critic 91 I1) A question whose difficulty a link an earlier build wrote just took from the circle: its bank, its
/// id, and its difficulty before and after.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Inherited {
    pub local: String,
    pub qid: String,
    pub before: Option<crate::model::Difficulty>,
    pub after: Option<crate::model::Difficulty>,
}

/// (round 15, critic 91 I1) The question editor open on such a question is told, as the topic dialog tells it of a topic
/// renamed: a draft still at the old difficulty takes the circle's, so that Done does not write the old one back. One whose
/// difficulty the person changed in the editor keeps hers.
pub fn refresh_draft(draft: Option<&mut crate::qedit::Draft>, moved: &[Inherited]) {
    let Some(d) = draft else { return };
    for i in moved {
        if d.bank_id == i.local && d.qid.as_deref() == Some(i.qid.as_str()) && d.difficulty == i.before {
            d.difficulty = i.after;
        }
    }
}

/// Where the open screen was asked for (phase D1): a bank's page in My Banks draws it in place of the questions,
/// with this way back. `None`: the Shared page draws it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EditHost {
    pub local_id: String,
    pub back: String,
}

// ---- jobs ---------------------------------------------------------------------------------------------------------

enum Work {
    /// `fetch`: (phase D3, plan section 6 "Pictures") the pictures of questions sent as the base has them, by
    /// `media/<sha256>.<ext>`: the job fetches each one this computer lacks from the circle before the send, so a
    /// base raw question is never sent with a missing picture. Empty for a whole-bank send.
    Send { cid: String, bid: String, req: Box<CircleSend>, fetch: Vec<String> },
    /// `own`: the version a send with no readable answer was based on, and the person's display name (see `OwnRead`).
    MergeFetch { cid: String, bid: String, base: u64, need_base: bool, own: Option<OwnRead> },
    MergeApply { cid: String, bid: String, version: u64 },
    /// The viewers' form is shuffled afresh on every read (critic 63 B2): the body that was reviewed is the one
    /// applied; only its pictures are fetched.
    MergeApplyBody { doc: Box<BankDoc> },
    /// Get the latest for a copy that can not send (a viewer's follow link, phase D1): no merge-initiate (owner and
    /// editors only), the newest version read directly.
    FollowFetch { cid: String, bid: String, base: u64, need_base: bool },
    Versions { cid: String, bid: String, before: Option<u64> },
    Version { cid: String, bid: String, n: u64 },
    Rollback { cid: String, bid: String, commit_id: String, base: u64, to: u64, confirm: bool },
    Trim { cid: String, bid: String, keep: u32 },
    Licence { bid: String },
    /// (critic 87 B1) The circle's version at a link's base version, read quietly to learn the difficulties a base an
    /// earlier build wrote kept none of.
    BaseRead { cid: String, bid: String, n: u64 },
    /// (phase D3) The base version's body for the open send page's summary, read once per page.
    SendBase { cid: String, bid: String, n: u64 },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum JobKind {
    /// A send, a restore or a trim: one at a time.
    Act,
    Merge,
    Versions,
    Version,
    Licence,
    /// (critic 87 B1) One at a time, beside whatever else runs.
    Base,
    /// (phase D3) The open send page's one read of its base version's body.
    SendBase,
}

impl Work {
    fn kind(&self) -> JobKind {
        match self {
            Work::Send { .. } | Work::Rollback { .. } | Work::Trim { .. } => JobKind::Act,
            Work::MergeFetch { .. } | Work::FollowFetch { .. } | Work::MergeApply { .. } | Work::MergeApplyBody { .. } => JobKind::Merge,
            Work::Versions { .. } => JobKind::Versions,
            Work::Version { .. } => JobKind::Version,
            Work::Licence { .. } => JobKind::Licence,
            Work::BaseRead { .. } => JobKind::Base,
            Work::SendBase { .. } => JobKind::SendBase,
        }
    }
}

enum Fetch {
    Ready(Box<Fetched>),
    /// The newest version is hidden while it is reviewed; what merge-initiate said is kept.
    Withheld(Box<MergeStart>),
    /// The circle's newest version changed while it was being read.
    Moved,
    /// The copy already has the newest version (a follow link's read says so itself).
    UpToDate,
}

enum Msg {
    Progress(PublishProgress),
    /// The answer, and the licence the send really went with (the pool's, asked just before).
    Sent(String, String, Result<SentBank, PoolError>, Option<License>),
    Fetched(String, String, Result<Fetch, PoolError>),
    Applied(String, String, Result<ApplyRead, PoolError>),
    Versions(String, String, bool, Result<VersionsPage, PoolError>),
    Version(String, String, u64, Box<Result<BankDoc, PoolError>>),
    Rolled(String, String, u64, Result<SentBank, PoolError>),
    Trimmed(String, String, Result<Trimmed, PoolError>),
    Licence(String, Result<Vec<HeadEntry>, PoolError>),
    BaseRead(String, String, u64, Box<Result<BankDoc, PoolError>>),
    /// (phase D3) The base version's body for the send summary.
    SendBase(String, String, u64, Box<Result<BankDoc, PoolError>>),
}

impl Msg {
    fn kind(&self) -> JobKind {
        match self {
            Msg::Progress(_) | Msg::Sent(..) | Msg::Rolled(..) | Msg::Trimmed(..) => JobKind::Act,
            Msg::Fetched(..) | Msg::Applied(..) => JobKind::Merge,
            Msg::Versions(..) => JobKind::Versions,
            Msg::Version(..) => JobKind::Version,
            Msg::Licence(..) => JobKind::Licence,
            Msg::BaseRead(..) => JobKind::Base,
            Msg::SendBase(..) => JobKind::SendBase,
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
        Work::Send { cid, bid, req, fetch } => {
            let (ptx, prepaint) = (tx.clone(), repaint.clone());
            let mut progress = move |p: PublishProgress| {
                let _ = ptx.send(Msg::Progress(p));
                if let Some(r) = &prepaint {
                    r();
                }
            };
            let (r, eff) = send_checked(&pool, &cid, &bid, *req, &*media, &*images, &fetch, &mut progress).await;
            Msg::Sent(cid, bid, r, eff)
        }
        Work::MergeFetch { cid, bid, base, need_base, own } => {
            let r = fetch_for_merge(&pool, &cid, &bid, base, need_base, own).await;
            Msg::Fetched(cid, bid, r)
        }
        Work::FollowFetch { cid, bid, base, need_base } => {
            let r = fetch_for_follow(&pool, &cid, &bid, base, need_base).await;
            Msg::Fetched(cid, bid, r)
        }
        Work::MergeApply { cid, bid, version } => {
            let r = fetch_for_apply(&pool, &images, &cid, &bid, version).await;
            Msg::Applied(cid, bid, r)
        }
        Work::MergeApplyBody { doc } => {
            let (cid, bid) = (doc.circle_id.clone(), doc.bank.id.clone());
            let r = super::circles::copy_bank(&pool, &*images, &doc).await.map(|imp| ApplyRead::Doc(Box::new(imp)));
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
        Work::BaseRead { cid, bid, n } => {
            let r = pool.circle_version(&cid, &bid, n).await;
            Msg::BaseRead(cid, bid, n, Box::new(r))
        }
        Work::SendBase { cid, bid, n } => {
            let r = pool.circle_version(&cid, &bid, n).await;
            Msg::SendBase(cid, bid, n, Box::new(r))
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
///
/// (phase D3, plan section 6 "Pictures") `fetch` names the pictures of questions sent as the base has them; each one
/// this computer lacks is fetched from the circle (hash checked by `circle_blob`) and stored through the same image
/// sink the merge uses. The send then reads those bytes from `FetchedMedia`, so the base raw question keeps its own
/// file name even when the store gives it another (a picture kept as PNG or JPEG under its own hash). A fetch that
/// fails sends nothing and says the plain line; only a sign-out keeps the sign-in path.
async fn send_checked(pool: &Pool<'_>, cid: &str, bid: &str, mut req: CircleSend, media: &dyn super::pool::MediaSource, images: &dyn super::browse::ImageSink, fetch: &[String], progress: &mut dyn FnMut(PublishProgress)) -> (Result<SentBank, PoolError>, Option<License>) {
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
    let mut extra: HashMap<String, Vec<u8>> = HashMap::new();
    for file in fetch {
        if media.read(file).is_some() {
            continue;
        }
        let Some(sha) = super::pool::media_sha(file) else {
            return (Err(LocalProblem::BaseMediaUnavailable.into()), now);
        };
        match pool.circle_blob(cid, bid, &sha).await {
            Ok(blob) => {
                // Best effort: kept on this computer for the next send or open; the send itself reads the bytes below
                // (the store may name the file differently, e.g. a picture kept as WebP).
                let _ = images.put(&blob.bytes);
                extra.insert(file.clone(), blob.bytes);
            }
            // The sign-in ended: that is the page's own step, not a picture problem.
            Err(e @ PoolError::Api(ApiError::SignedOut)) => return (Err(e), now),
            Err(_) => return (Err(LocalProblem::BaseMediaUnavailable.into()), now),
        }
    }
    let media = FetchedMedia { base: media, fetched: std::cell::RefCell::new(extra) };
    (pool.send_circle_version(cid, bid, &req, &media, progress).await, now)
}

/// The pictures of a send: those this computer has (`base`), and the ones just fetched from the circle, by the file
/// name the composed question names (which the image store may keep under another name). A fetched entry is handed
/// over (moved out) on its one read, so the send does not hold a second copy of every fetched picture.
struct FetchedMedia<'a> {
    base: &'a dyn super::pool::MediaSource,
    fetched: std::cell::RefCell<HashMap<String, Vec<u8>>>,
}

impl super::pool::MediaSource for FetchedMedia<'_> {
    fn read(&self, file: &str) -> Option<Vec<u8>> {
        self.fetched.borrow_mut().remove(file).or_else(|| self.base.read(file))
    }
}

/// (critic 83 I2) A send from the bank with no readable answer: the version it was based on, and the person's display
/// name (whose version the one right after it is).
#[derive(Clone, Debug)]
struct OwnRead {
    base: u64,
    me: Option<String>,
}

/// merge-initiate, the newest version, (when the copy kept no snapshot) the version it was made from, and (critic 83 I2)
/// the person's own version right after the one a send with no readable answer was based on, when newer ones came after
/// it (the newest is read anyway).
async fn fetch_for_merge(pool: &Pool<'_>, cid: &str, bid: &str, base: u64, need_base: bool, own: Option<OwnRead>) -> Result<Fetch, PoolError> {
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
    // (only one the circle lists as kept and readable, never a colleague's)
    let next = own.as_ref().and_then(|o| o.base.checked_add(1).map(|n| (n, o)));
    let mine = match next {
        Some((n, o)) if start.head_version > n && start.between.iter().any(|l| l.version == n && l.kept && !l.withheld) && !matches!(sender_of(o.me.as_deref(), next_sender(o.base, &start)), Who::Other(_)) => {
            match pool.circle_version(cid, bid, n).await {
                Ok(d) => Some(d),
                // (critic 87 m2) the sign-in ended or the circle could not be reached: said as that, with Try again, never
                // as a version that can't be read
                Err(e) if not_reached(&e) => return Err(e),
                // (not readable, whatever else the reason: matched with the newest only, as before; critic 85 m2: it never
                // stops Get latest)
                Err(_) => None,
            }
        }
        _ => None,
    };
    Ok(Fetch::Ready(Box::new(Fetched { start, head, base: base_doc, mine })))
}

/// Get the latest for a copy that can not send (phase D1): the newest version read directly (merge-initiate is for
/// the owner and editors), and the version the copy was made from when it kept no snapshot. The same review
/// follows; no merge token (nothing is sent from such a copy).
async fn fetch_for_follow(pool: &Pool<'_>, cid: &str, bid: &str, base: u64, need_base: bool) -> Result<Fetch, PoolError> {
    let head = match pool.circle_bank(cid, bid).await {
        Ok(h) => h,
        Err(PoolError::Refused(r)) if matches!(r.kind, RefusalKind::VersionWithheld) => return Ok(Fetch::Withheld(Box::new(MergeStart { base_version: base, ..MergeStart::default() }))),
        Err(e) => return Err(e),
    };
    if head.bank.version <= base {
        return Ok(Fetch::UpToDate);
    }
    let base_doc = if need_base {
        match pool.circle_version(cid, bid, base).await {
            Ok(d) => Some(d),
            Err(PoolError::Refused(r)) if matches!(r.kind, RefusalKind::VersionNotRetained | RefusalKind::VersionWithheld | RefusalKind::KeysHiddenUnavailable) => None,
            Err(e) => return Err(e),
        }
    } else {
        None
    };
    let start = MergeStart {
        base_version: base,
        base_kept: base_doc.is_some(),
        head_version: head.bank.version,
        head_version_id: head.bank.version_id.clone(),
        head_at: head.bank.committed_at,
        head_by: head.bank.committed_by.clone().unwrap_or_default(),
        ..MergeStart::default()
    };
    Ok(Fetch::Ready(Box::new(Fetched { start, head, base: base_doc, mine: None })))
}

/// What the apply's read of the newest version found.
enum ApplyRead {
    /// The version reviewed, with its pictures stored here.
    Doc(Box<Imported>),
    /// Another version is the newest now.
    Moved,
    /// The review was read in full, and this read is the viewers' form (the person was made a viewer of a bank that
    /// hides its keys meanwhile): nothing is applied (critic 63 B1).
    KeysHidden,
}

/// The newest version again (a review read in full), with its pictures stored here.
async fn fetch_for_apply(pool: &Pool<'_>, images: &SharedImages, cid: &str, bid: &str, version: u64) -> Result<ApplyRead, PoolError> {
    let doc = pool.circle_bank(cid, bid).await?;
    if doc.bank.keys_hidden {
        return Ok(ApplyRead::KeysHidden);
    }
    if doc.bank.version != version {
        return Ok(ApplyRead::Moved);
    }
    Ok(ApplyRead::Doc(Box::new(super::circles::copy_bank(pool, &**images, &doc).await?)))
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
    /// The sender's rights (who can fix a held version), for the answer said on the strip when the page was closed.
    perm: Perm,
    /// (critic 78 I1, critic 79 minor 4) What shaped the request besides the bank: a send with no clear answer is tried
    /// again with it (the same request).
    retry: RetrySend,
    /// (critic 79 I2) The version the send was based on.
    base: u64,
    /// (phase D3) A partial send's composed questions and the unticked change ids that still differ: what the link
    /// keeps after it (`after_partial_send`). `None`: the whole bank was sent.
    partial: Option<ComposedPartial>,
    held_back: Vec<String>,
}

/// (critic 78 I1, critic 79 minor 4) What the Send page sent besides the bank itself: its note, the viewers' copy
/// decisions and pictures (for the content they were made for), the hidden version it went over, a large change
/// accepted, and the merge it went with. After a send with no clear answer, the bank's next Send page starts from it, so
/// the same changes go as the same request (the circle answers the first one again, never twice).
#[derive(Clone)]
struct RetrySend {
    local_id: String,
    message: String,
    decisions: StripDecisions,
    decisions_for: Option<String>,
    picked: Option<Box<Pictures>>,
    base_override: Option<u64>,
    replaces: Vec<VersionLine>,
    confirm_large: bool,
    carry: Option<Carry>,
    /// (phase D3) The local ids the person unticked for this request: reopening after an answer that could not be read
    /// starts the same selection, and a conflict then Get latest keeps these marks by local id.
    unticked: BTreeSet<String>,
}

/// (critic 79 I2, critic 80 I2) What the newest version says of a send with no clear answer.
#[derive(Clone, Debug, PartialEq, Eq)]
enum Found {
    /// The circle still has the version the send was based on: it has not arrived (critic 81 minor 5: it may yet).
    Still,
    /// A colleague (named) sent the version right after it: it did not go through.
    OtherFirst(String),
    /// The person sent the version right after it: it went through.
    Mine,
    /// A newer version: it may be the send.
    Maybe,
}

/// (critic 80 I2) Who sent a version, as far as the names say.
#[derive(Clone, Debug, PartialEq, Eq)]
enum Who {
    Me,
    Other(String),
    Unknown,
}

#[derive(Clone)]
struct Carry {
    local_id: String,
    token: String,
    from: u64,
    head: u64,
}

pub struct CircleEditModel {
    pub view: EditView,
    /// (critic 71, round 5) How a restore, trim, send, Get latest, unlink or check went, where the person did not see
    /// it happen: per bank, until dismissed or replaced by the next outcome of its kind (see `act_results`).
    results: ActResults,
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
    /// "Try again" was pressed on a links file that could not be read: the next use reads it again (critic 65).
    read_again: bool,
    /// "Open account..." was pressed on one of these screens: the page that hosts them opens the Account window.
    account_ask: bool,
    /// The card whose "Forget this link" was pressed once.
    forget_ask: Option<String>,
    /// (round 20, critic 100 minor 4) The bank whose "Put all of mine back" was pressed once: the news asks before it changes
    /// several questions at once.
    put_all_ask: Option<String>,
    /// "Treat this copy as in step": done by `sync`, which has the banks.
    cmd_rebase: bool,
    /// Circle banks known to hide their answer keys from viewers (`true`) or not, as last read (the circle page,
    /// a bank's read, a change of the setting, a send's answer). A send to one asks about pictures first.
    hiding: HashMap<String, bool>,
    /// Where the open screen was asked for (phase D1): a bank's page, or the Shared page (`None`).
    pub host: Option<EditHost>,
    /// The frame the bank's page last drew the open screen: a History or a version left behind when the person went
    /// elsewhere is closed when they come back (critic 64 polish 13).
    pub host_frame: Option<u64>,
    /// (phase D2, plan 5.3) The question history's reads kept for this session: change lists per bank (a 304 costs
    /// nothing), history lines per head, and the last 4 version bodies. Dropped on sign-out and account switch.
    pub question_cache: super::question_history::HistoryCache,
    /// (round 5, critic I3) The bank one of whose questions is open in the app's editor (set by the bank's page each frame).
    question_draft: Option<String>,
    /// "Treat as in step..." from a bank's page: when Get latest finds nothing newer, the question is asked at once.
    rebase_next: bool,
    /// (critic 67 I3) Which of a send, restore or trim is on its way, and the bank's name.
    act_for: Option<ActFor>,
    /// A screen was asked for while that was on its way: the strip says to wait.
    act_waited: bool,
    /// (critic 71) Which account's turn it is: another account signing in starts a new one. A send, restore or trim
    /// started in an earlier turn is not told to this account, and what it answers changes nothing for it.
    turn: u64,
    /// (critic 71 I1) The app's frame (egui pass) the model was last given, and the last one that drew the History or
    /// one of its versions: a History not drawn since the last frame is not shown, wherever it was left open.
    /// `None` until the app says (the model alone, in tests: the open History counts as shown).
    pass: Option<u64>,
    drawn: Option<u64>,
    /// The account was signed in at the last frame: "your session ended" goes once it is again.
    was_ready: bool,
    /// (critic 73 I1) Banks whose strip checks the circle at once (see `take_check_asks`).
    check_asks: Vec<String>,
    /// (critic 82 I1) Banks the circle answered for since their strip last asked (see `take_answered`).
    answered: Vec<String>,
    /// (critic 75 I1) What the strip says of the answer on the open Send page (a failure, a hold, a conflict...), if
    /// the page is closed before the person acts on it there: it may have landed while the page was left undrawn.
    send_note: Option<(ResultBank, ResultKind, String)>,
    /// (critic 73 minor 1) Circle banks whose restore or trim had no answer in time (it may still land): their newest
    /// version is "when last checked", never "the latest", until the next check or History read.
    unsure: std::collections::HashSet<(String, String)>,
    /// (critic 73 minor 4) The account signed in at the last frame that had one (the pool's address and its email).
    account: Option<String>,
    /// (critic 73 minor 3) Sign-outs and account changes seen: a signed-out answer to a job started before the last one
    /// is about a session that already ended, not the one signed in now.
    outs: u64,
    /// (critic 78 I1) Circle banks whose last send had no clear answer and whose note still says the circle "is checked
    /// again": the check's answer replaces it once. (critic 79 I2) With the version the send was based on: the check
    /// decides by it, never by whether the check itself moved the newest version (another read may have seen it first).
    /// Whether the send may have gone through (a conflict may then be with it) is kept with the link (`sent_record`).
    unclear_noted: HashMap<(String, String), u64>,
    /// (critic 78 I1, critic 79 minor 4) A send with no clear answer, as its page sent it: the next Send page of the bank
    /// starts from it (see `RetrySend`).
    retry_send: Option<RetrySend>,
    /// (critic 77 I2) Circle banks whose restore had no clear answer: the version restored, (critic 79 I2) the version the
    /// restore was based on, and what the check asked for at once found (said in place of "may have gone through" once
    /// it answered); (critic 84 I1) whether that was the circle still at the version it was based on (the next check says
    /// again what it finds).
    unclear_restores: HashMap<(String, String), (u64, u64, Option<String>, bool)>,
    /// (critic 80 I2) The display name of the account signed in, when known: a version the circle says this name sent is
    /// the person's own.
    me_name: Option<String>,
    /// (critic 87 B1) The circle's versions read to learn the difficulties a base an earlier build wrote kept none of, by
    /// (circle, bank, version): each is read once (see `read_bases`).
    base_reads: HashMap<(String, String, u64), BaseRead>,
    /// (critic 87 B1) The base version being read now (meaningful only while a `Base` job runs), and (critic 91 minor) when it
    /// was asked.
    base_asked: Option<(String, String, u64)>,
    base_since: Option<Instant>,
    /// (critic 89 m2) The base version the open Get latest reads itself to learn its difficulties (the quiet read is then not
    /// asked beside it).
    merge_base: Option<(String, String, u64)>,
    /// (round 15, critic 91 I1) Questions whose difficulty was just taken from the circle, for the open question editor (see
    /// [`refresh_draft`]); taken by the page after each pass.
    inherited: Vec<Inherited>,
    /// (round 16, critic 93 I1 and minor 1) Where the banks are saved when the circle's ratings are put into one: before the
    /// links file says it was done (`None`: the banks file of the store the links file is in).
    bank_saver: Option<BankSaver>,
    /// (round 16, critic 93 I1) Banks whose difficulties from the circle could not be written (the restore point or the banks
    /// file could not be saved): tried again after `BASE_RETRY` (or at once on Try again). (round 17) With why, how many tries
    /// failed, and the restore point already saved for the bank as it was (not saved again while it stays the same).
    inherit_held: HashMap<String, Held>,
    /// (round 17, critic 95 minor 3; round 18, critic 97 m4; round 19, critic 99 m5) The print of all the banks a safety copy was
    /// last made of before the circle's difficulty went in: not made again while they stay the same, whatever the number of banks
    /// held. (Base questions a bank lacked at the read are kept on its link: `CircleLink::missing_at_read`.)
    safety_copy_for: Option<String>,
    /// (round 18, critic 97 m6) The links file could not be written at this time: it is written again a little later.
    links_unsaved: Option<Instant>,
    /// (round 19, critic 100 I2) The banks file could not be written when she asked for a change ("Put mine back"): so her own
    /// edits are not saved either. By bank: what the strip says, and the tries that failed. Said until a save works.
    banks_unsaved: HashMap<String, (String, u32)>,
    /// (round 19, critic 100 I2) "Try again" on such a bank: all the banks are written at the next pass.
    banks_again: bool,
    /// (phase D3) Unticked local ids of a send page that is about to be rebuilt (a 409 then Get latest, an answer that
    /// could not be read): ids still changes start unticked, new changes ticked. By local bank; consumed by
    /// `resolve_partial`, cleared when a send is done or the page abandoned.
    send_unticked: Option<(String, BTreeSet<String>)>,
}

/// (round 16, critic 93 I1; round 17) The circle's difficulties could not be written into a bank: when that was last tried
/// (`None`: Try again was pressed, tried at the next pass), why in plain words, and the tries that failed in a row. (round 19,
/// critic 99 m5: the safety copy already made is the model's, see `safety_copy_for`.)
#[derive(Clone, Debug)]
struct Held {
    at: Option<Instant>,
    why: String,
    tries: u32,
}

/// (round 16, critic 93 minor 1) Saves all the banks; tests give one that records or fails.
pub type BankSaver = Arc<dyn Fn(&[QuestionBank]) -> Result<(), String> + Send + Sync>;

/// (critic 87 B1) How the read of a link's base version went, when it taught nothing (one that did is recorded on the link
/// itself: see `CircleLink::difficulty_read`).
#[derive(Clone, Debug)]
enum BaseRead {
    /// It can't be read (no longer kept, hidden, not the version asked for, a reply that can't be read): (critic 89 m4) asked
    /// again after `BASE_UNREADABLE_RETRY`, or as soon as the circle's state of the bank (as the link last saw it) changes.
    Unreadable(Instant, String, Unread),
    /// The circle could not be asked (signed out, offline, a problem of its own): asked again after `BASE_RETRY`. (round 16,
    /// critic 94 m2) How many times in a row: the first is asked again after `BASE_FIRST_RETRY`, and is not said yet.
    NotReached(Instant, u32),
    /// (round 16, critic 93 minor 4) The circle's daily limit: asked again at the time given.
    Limited(Instant),
}

/// (critic 89 m4) The circle's state of a bank as the link last saw it: its newest version, whether it is held back or
/// removed, the person's role.
fn circle_state(l: &CircleLink) -> String {
    format!("{}|{:?}|{:?}|{:?}", l.head_seen, l.head_seen_at, l.seen, l.role_seen)
}

/// (critic 87 B1) How long a read of a base version that could not reach the circle waits before it is tried again.
pub const BASE_RETRY: Duration = Duration::from_secs(60);

/// (round 16, critic 94 m2) How long a read that could not reach the circle the first time waits before it is asked again
/// (meanwhile the copy is still said to be checking: a page just opened does not say at once that it can't check).
pub const BASE_FIRST_RETRY: Duration = Duration::from_secs(5);

/// (critic 91 minor) How long Get latest waits for the quiet read of its base version before it says that the circle can't be
/// reached just now (it goes on waiting: the read is still on its way).
pub const BASE_WAIT: Duration = Duration::from_secs(15);

/// (critic 89 m4) How long a base version the circle would not show (no longer kept, held back, a reply that could not be
/// read) waits before it is asked again; sooner when the circle's state of the bank changes.
pub const BASE_UNREADABLE_RETRY: Duration = Duration::from_secs(30 * 60);

/// (critic 87 m2) The sign-in ended, or the circle could not be reached or had a problem of its own: that says nothing of
/// the version asked for, so it is never taken for a version that can't be read.
fn not_reached(e: &PoolError) -> bool {
    match e {
        PoolError::Api(a) => !matches!(a, ApiError::BadReply(_) | ApiError::TooLarge(_)),
        PoolError::Refused(r) => matches!(r.kind, RefusalKind::Busy | RefusalKind::RateLimited | RefusalKind::TooManyOpenReads | RefusalKind::PoolNotConfigured) || matches!(r.detail.status, 401 | 408 | 429 | 500..=599),
        PoolError::Local(_) => false,
    }
}

/// (critic 67 I3, critic 69 I1) The send, restore or trim on its way: what it is, the bank's name, its bank on this
/// computer, and the version restored (a restore) or the versions kept (a trim).
#[derive(Clone, Debug)]
struct ActFor {
    what: &'static str,
    name: String,
    local: Option<String>,
    cid: String,
    bid: String,
    arg: u64,
    /// (critic 79 I2) The version a send or restore was based on (a trim: 0).
    base: u64,
    /// The account's turn it was started in (see `CircleEditModel::turn`).
    turn: u64,
    /// (critic 73 minor 4) The account it was asked for, when known: the same account back after another one is
    /// told how it went.
    account: Option<String>,
    /// (critic 73 minor 3) The sign-outs seen before it started (see `CircleEditModel::outs`).
    outs: u64,
    /// When it started (a restore or trim is waited for at most `ACT_PATIENCE`).
    since: Instant,
}

/// (critic 69 minor 7, critic 71) How long a restore or trim with no answer holds Send, Get latest and History on
/// every linked bank: after this the screens open again and the bank's strip says it may still go through. (The
/// request itself stops at the transport's limit and its retries, which can be over three minutes.)
pub const ACT_PATIENCE: Duration = Duration::from_secs(60);

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
            results: ActResults::default(),
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
            read_again: false,
            account_ask: false,
            forget_ask: None,
            put_all_ask: None,
            cmd_rebase: false,
            hiding: HashMap::new(),
            host: None,
            host_frame: None,
            rebase_next: false,
            act_for: None,
            act_waited: false,
            turn: 0,
            pass: None,
            drawn: None,
            was_ready: false,
            check_asks: Vec::new(),
            answered: Vec::new(),
            send_note: None,
            unsure: Default::default(),
            account: None,
            outs: 0,
            unclear_noted: HashMap::new(),
            retry_send: None,
            unclear_restores: HashMap::new(),
            me_name: None,
            base_reads: HashMap::new(),
            base_asked: None,
            base_since: None,
            merge_base: None,
            inherited: Vec::new(),
            bank_saver: None,
            inherit_held: HashMap::new(),
            safety_copy_for: None,
            links_unsaved: None,
            banks_unsaved: HashMap::new(),
            banks_again: false,
            send_unticked: None,
            question_cache: Default::default(),
            question_draft: None,
        }
    }

    /// (round 16) Where the banks are saved when the circle's ratings are put into one (see `bank_saver`).
    pub fn with_bank_saver(mut self, f: BankSaver) -> CircleEditModel {
        self.bank_saver = Some(f);
        self
    }

    /// (round 15, critic 91 I1) The questions whose difficulty was taken from the circle since the last call (see
    /// [`refresh_draft`]).
    pub fn take_inherited(&mut self) -> Vec<Inherited> {
        std::mem::take(&mut self.inherited)
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

    /// (critic 89 m2) The quiet read of this base version is on its way.
    fn quiet_read_of(&self, key: &(String, String, u64)) -> bool {
        self.running(JobKind::Base) && self.base_asked.as_ref() == Some(key)
    }

    /// (critic 89 m2) The open Get latest waits for the quiet read of its bank's base version on its way (its difficulties are
    /// to be learnt, and the review is made after).
    fn merge_waits_for_base(&self) -> bool {
        let EditView::Merge(m) = &self.view else { return false };
        self.links.iter().find(|l| l.local_bank_id == m.local_id).is_some_and(|l| needs_difficulty_read(l) && self.quiet_read_of(&(l.circle_id.clone(), l.circle_bank_id.clone(), l.base_version)))
    }

    /// (round 17, critic 95 minor 1) The open Get latest has what it needs, but its bank still waits for the circle's
    /// difficulties to be put into it (they could not be saved yet: held). (A bank no longer here is not waited for: the
    /// review says so.)
    fn merge_waits_for_difficulty(&self) -> bool {
        let EditView::Merge(m) = &self.view else { return false };
        // (round 19, critic 99 m7) a held pass for a question put back after the read too
        matches!(m.phase, MergePhase::Ready(_)) && self.inherit_held.contains_key(&m.local_id) && self.links.iter().any(|l| l.local_bank_id == m.local_id)
    }

    /// (critic 87 B1, critic 89 m2) What the open Get latest read of its bank's base version (when it read it to learn the
    /// difficulties) is learnt before the review is made; one it could not read is not asked again quietly until later.
    fn learn_from_fetch(&mut self) {
        let EditView::Merge(m) = &self.view else { return };
        let MergePhase::Ready(f) = &m.phase else { return };
        let Some(link) = self.links.iter().find(|l| l.local_bank_id == m.local_id) else { return };
        let key = (link.circle_id.clone(), link.circle_bank_id.clone(), link.base_version);
        let asked = self.merge_base.take().is_some_and(|k| k == key);
        if !needs_difficulty_read(link) {
            return;
        }
        let at = self.clock.unwrap_or_else(now_ms);
        let read = f.base.as_ref().filter(|bd| bd.bank.version == key.2).map(|bd| circle_theirs_as(&bd.payload, at, bd.bank.keys_hidden));
        match read {
            Some(theirs) => self.learn_base(&key.0, &key.1, key.2, &theirs),
            None if asked => {
                let state = self.unreadable_now(&key.0, &key.1, Unread::NotKept);
                self.base_reads.insert(key, state);
            }
            None => {}
        }
    }

    /// A send, a restore or a trim is on its way.
    pub fn acting(&self) -> bool {
        self.running(JobKind::Act) || self.cmd_send
    }

    /// (critic 67 I3) A send, restore or trim is on its way: no other screen opens in its place (one model serves every
    /// bank's page, and the new screen would drop the answer that moves that bank's link on). Returns true, and the
    /// strip says to wait, when the screen asked for is refused.
    fn wait_for_act(&mut self) -> bool {
        self.act_waited = self.running(JobKind::Act);
        self.act_waited
    }

    /// What the strip says after a screen was refused by [`wait_for_act`](Self::wait_for_act), while that is so.
    pub fn wait_note(&self) -> Option<String> {
        if !self.act_waited || !self.running(JobKind::Act) {
            return None;
        }
        let a = self.act_for.as_ref()?;
        // (critic 71 minor 4) one an earlier account asked for is not named to this one
        if !self.mine(a) {
            return Some(OTHER_ACT_WAIT.into());
        }
        let of = if a.what == "send" { "from" } else { "of" };
        Some(format!("Wait for the {} {of} \"{}\" to finish, then try again.", a.what, a.name))
    }

    /// (critic 69 I1) A restore or trim of this bank is on its way while its History is not shown: what the strip says
    /// in place of where the copy stands (never "up to date" while the circle's newest version may be changing).
    pub fn act_line(&self, local_id: &str) -> Option<String> {
        let a = self.act_for.as_ref().filter(|a| self.running(JobKind::Act) && a.local.as_deref() == Some(local_id))?;
        self.act_words(a)
    }

    /// (critic 71 I1) The same for a circle bank's Bank settings page, linked here or not.
    pub fn act_line_of(&self, cid: &str, bid: &str) -> Option<String> {
        let a = self.act_for.as_ref().filter(|a| self.running(JobKind::Act) && a.cid == cid && a.bid == bid)?;
        self.act_words(a)
    }

    fn act_words(&self, a: &ActFor) -> Option<String> {
        // (critic 71 minor 4) an earlier account's send, restore or trim: not named, and not "up to date" meanwhile
        if !self.mine(a) {
            return Some(OTHER_ACT_LINE.into());
        }
        match a.what {
            "restore" => Some(format!("Restoring version {} in the circle...", a.arg)),
            "trim" => Some("Trimming the bank's old versions in the circle...".into()),
            _ => None,
        }
    }

    /// (critic 69 minor 2) A send from this bank is on its way.
    pub fn sending_from(&self, local_id: &str) -> bool {
        self.running(JobKind::Act) && self.act_for.as_ref().is_some_and(|a| a.what == "send" && a.local.as_deref() == Some(local_id))
    }

    /// (critic 74 I1) The last send from this bank went through, but its answer could not be read: the copy has not
    /// caught up with the version it made. (critic 75 minor 5) Kept in the links file; said only to the account that
    /// sent it.
    pub fn sent_unread(&self, local_id: &str) -> bool {
        self.sent_unread_link(local_id).is_some()
    }

    /// (critic 76 I1) The same for `bank`, and whether it changed after that send (`Some(true)`: changes made since
    /// are not sent).
    pub fn sent_unread_of(&self, bank: &QuestionBank) -> Option<bool> {
        self.sent_unread_link(&bank.id).map(|u| bank.modified != u.modified)
    }

    fn sent_unread_link(&self, local_id: &str) -> Option<&SentUnread> {
        let u = self.links.iter().find(|l| l.local_bank_id == local_id)?.sent_unread.as_ref()?;
        let me = super::bank_links::account_tag(self.account.as_deref()).unwrap_or_default();
        (u.by.is_empty() || u.by == me).then_some(u)
    }

    /// (critic 73 minor 1) A restore or trim of this bank had no answer in time and may still land.
    pub fn unsure(&self, local_id: &str) -> bool {
        self.links.iter().find(|l| l.local_bank_id == local_id).is_some_and(|l| self.unsure.contains(&(l.circle_id.clone(), l.circle_bank_id.clone())))
    }

    /// (critic 73 minors 1, 6) A check of this bank was answered (`moved`: it found a newer version).
    pub fn checked(&mut self, local_id: &str, moved: bool) {
        let Some(l) = self.links.iter().find(|l| l.local_bank_id == local_id).cloned() else { return };
        let key = (l.circle_id.clone(), l.circle_bank_id.clone());
        let head = l.head_seen;
        self.unsure.remove(&key);
        if moved {
            self.head_moved(local_id);
        }
        // (critic 79 I2) A send with no clear answer is decided by the version it was based on, never by whether this
        // check moved the newest version: a History, the Shared page or a merge may have seen it first. (critic 80 I2) A
        // colleague's version right after it means it did not go through (it could only be refused now); the person's own
        // means it did. (critic 81 minor 5) The circle still at that version proves nothing: a slow commit may land after
        // the check, so nothing is settled then.
        let by = l.head_seen_by.clone();
        if let Some(base) = self.pending_send_base(&l) {
            match self.found(head, base, by.as_deref()) {
                Found::OtherFirst(_) => self.update_link(local_id, |l| l.sent_record.as_mut().is_some_and(|r| !std::mem::replace(&mut r.not_through, true))),
                // the person's own version right after it: the send went, as if its answer had come but could not be read
                // (its changes are not unsent, and the strip offers Get latest)
                Found::Mine => self.update_link(local_id, went_through),
                Found::Still | Found::Maybe => {}
            }
        }
        // (critic 78 I1) Its note said the circle "is checked again"; now it says what the check found, on the strip and
        // in what the open Send page leaves there. (critic 84 I1) The circle still at that version settles nothing (the
        // send may land later): the next check says again what it finds, so the note never stays "may not have arrived
        // yet" beside a newer version.
        if let Some(&base) = self.unclear_noted.get(&key) {
            let found = self.found(head, base, by.as_deref());
            if found != Found::Still {
                self.unclear_noted.remove(&key);
            }
            let text = match found {
                Found::Still => unclear_still(head),
                Found::OtherFirst(who) => unclear_other_first(&who, head),
                Found::Mine => unclear_mine(head),
                Found::Maybe => unclear_moved(head),
            };
            if let Some(b) = self.result_bank_of_local(local_id) {
                self.results.replace_text(&b, ResultKind::Send, &text);
            }
            if let Some((b, ResultKind::Send, t)) = &mut self.send_note
                && b.cid == key.0
                && b.bid == key.1
            {
                *t = text;
            }
        }
        // (critic 77 I2) The same for a restore with no clear answer (on the strip, and when its History is left),
        // (critic 79 I2) decided by the version the restore was based on; (critic 84 I1) said again by each check while
        // the circle is still at that version.
        if let Some((to, base, found, still)) = self.unclear_restores.get_mut(&key)
            && (found.is_none() || *still)
        {
            let to = *to;
            *still = head <= *base;
            let text = if !*still {
                format!("The check found a newer version in the circle, version {head}. It may be the restore of version {to}: open History to see.")
            } else {
                // (critic 81 minor 5) not proof that it failed: it may land after the check
                format!("The restore of version {to} may not have arrived yet: the circle still has version {head}. Open History to try again.")
            };
            *found = Some(text.clone());
            if let Some(b) = self.result_bank_of_local(local_id) {
                self.results.replace_text(&b, ResultKind::Restore, &text);
            }
        }
    }

    /// (critic 79 I2, critic 80 I2) What the newest version `head`, sent by `by`, says of a send based on `base`.
    fn found(&self, head: u64, base: u64, by: Option<&str>) -> Found {
        if head <= base {
            return Found::Still;
        }
        // (critic 81 minor 6) never past the largest version number
        match (base.checked_add(1) == Some(head), self.sender(by)) {
            (true, Who::Other(who)) => Found::OtherFirst(who),
            (true, Who::Me) => Found::Mine,
            _ => Found::Maybe,
        }
    }

    /// (critic 80 I2) Whether `by`, a name the circle gave for who sent a version, is the person signed in. (critic 81
    /// minor 1) The circle names who sent a version by display name only (its heads, its versions and a conflict carry no
    /// account id), so a colleague with exactly the same display name is taken for the person: a limit of what the
    /// circle says. Names are compared exactly, white space aside.
    fn sender(&self, by: Option<&str>) -> Who {
        sender_of(self.me_name.as_deref(), by)
    }

    /// (critic 79 I2) The send with no clear answer kept with this link, for the account signed in, while it may have gone
    /// through: the version it was based on. (One whose answer came but could not be read went: `sent_unread` says so
    /// first wherever this is asked.)
    fn pending_send_base(&self, l: &CircleLink) -> Option<u64> {
        self.record_mine(l).filter(|r| !r.not_through).map(|r| r.base)
    }

    /// (critic 80 I1) What a send from this link with no readable outcome carried, when the account signed in sent it
    /// (one whose account is not known is said to no one but the account of this turn, like `sent_unread`).
    fn record_mine<'a>(&self, l: &'a CircleLink) -> Option<&'a SentRecord> {
        let r = l.sent_record.as_ref()?;
        let me = super::bank_links::account_tag(self.account.as_deref()).unwrap_or_default();
        (r.by.is_empty() || r.by == me).then_some(r)
    }

    /// (critic 78 I2) What the bank's last send is known to have done, when it may make the circle ahead of the copy:
    /// it went through but its answer could not be read (`Some(true)`), or no clear answer came (`Some(false)`).
    pub fn earlier_send(&self, local_id: &str) -> Option<bool> {
        if self.sent_unread(local_id) {
            return Some(true);
        }
        let l = self.links.iter().find(|l| l.local_bank_id == local_id)?;
        self.pending_send_base(l).map(|_| false)
    }

    /// (critic 78 I2, critic 80 I2) A conflict on a send from this bank is with the person's own earlier send: it went
    /// through (`Some(true)`: its answer could not be read, or the version right after the one it was based on is the
    /// person's), it may be (`Some(false)`), or the newer version is a colleague's (`None`).
    pub fn conflict_own(&self, local_id: &str, c: &ConflictInfo) -> Option<bool> {
        if self.earlier_send(local_id)? {
            return Some(true);
        }
        let base = self.links.iter().find(|l| l.local_bank_id == local_id).and_then(|l| self.pending_send_base(l))?;
        match self.sender(c.head_by.as_deref()) {
            Who::Other(_) => None,
            Who::Me => Some(base.checked_add(1).is_some_and(|next| c.head_version == Some(next))),
            Who::Unknown => Some(false),
        }
    }

    /// The bank's send with no clear answer is no longer in question (caught up, unlinked).
    fn forget_maybe(&mut self, local_id: &str) {
        if let Some(key) = self.links.iter().find(|l| l.local_bank_id == local_id).map(|l| (l.circle_id.clone(), l.circle_bank_id.clone())) {
            self.unclear_noted.remove(&key);
        }
        self.update_link(local_id, |l| l.sent_record.take().is_some());
        if self.retry_send.as_ref().is_some_and(|r| r.local_id == local_id) {
            self.retry_send = None;
        }
    }

    /// (critic 73 minor 6) The circle's newest version moved on for this bank: what said it matches the newest version,
    /// or that a restore may still go through, is no longer so.
    fn head_moved(&mut self, local_id: &str) {
        if let Some(b) = self.result_bank_of_local(local_id) {
            self.results.drop_of(&b, ResultKind::GetLatest);
            self.results.drop_of(&b, ResultKind::NoAnswer);
        }
    }

    /// The send, restore or trim on its way was asked for in this account's turn (critic 71: not by an account that
    /// signed in on this computer before this one).
    fn act_mine(&self) -> bool {
        self.act_for.as_ref().is_some_and(|a| self.mine(a))
    }

    /// (critic 73 minor 4) It was asked for by the account signed in now: the same account (also back after another
    /// one signed in meanwhile), or, when the accounts are not known, in this account's turn.
    fn mine(&self, a: &ActFor) -> bool {
        match (&a.account, &self.account) {
            (Some(x), Some(y)) => x == y,
            _ => a.turn == self.turn,
        }
    }

    /// (critic 73 minor 3) A signed-out answer to the send, restore or trim on its way, while the account that asked for
    /// it is signed in again after a sign-out: the session that ended was the earlier one. Nothing asks to sign in.
    fn ended_earlier(&self, e: &PoolError) -> bool {
        matches!(e, PoolError::Api(ApiError::SignedOut)) && self.gate == Gate::Ready && self.act_for.as_ref().is_some_and(|a| a.outs != self.outs && self.mine(a))
    }

    // ---- results (critic 71, round 5) ---------------------------------------------------------------------------

    /// The bank a result about this circle bank is said for: its linked bank here, if it has one.
    fn result_bank(&self, cid: &str, bid: &str, name: &str) -> ResultBank {
        ResultBank { local: self.local_of(cid, bid), cid: cid.to_string(), bid: bid.to_string(), name: api::plain(name) }
    }

    /// The bank of a linked bank on this computer.
    fn result_bank_of_local(&self, local: &str) -> Option<ResultBank> {
        let l = self.links.iter().find(|l| l.local_bank_id == local)?;
        Some(ResultBank { local: Some(local.to_string()), cid: l.circle_id.clone(), bid: l.circle_bank_id.clone(), name: self.act_name(&l.circle_id, &l.circle_bank_id) })
    }

    /// The bank of the send, restore or trim on its way (critic 71 I2): its own, as it was when it started, unless it
    /// is no longer linked here (unlinked meanwhile): then the circle bank alone, said with its name.
    fn act_bank(&self, cid: &str, bid: &str) -> ResultBank {
        let a = self.act_for.as_ref().filter(|a| a.cid == cid && a.bid == bid);
        let still = |l: &String| self.links.iter().any(|x| &x.local_bank_id == l && x.circle_id == cid && x.circle_bank_id == bid);
        let local = a.and_then(|a| a.local.clone()).filter(still).or_else(|| self.local_of(cid, bid));
        let name = a.map(|a| a.name.clone()).filter(|n| !n.is_empty()).unwrap_or_else(|| self.act_name(cid, bid));
        ResultBank { local, cid: cid.to_string(), bid: bid.to_string(), name }
    }

    fn say(&mut self, bank: ResultBank, kind: ResultKind, text: impl Into<String>) {
        self.results.put(bank, kind, text);
    }

    /// The results of this linked bank, for its strip: (id, words).
    pub fn results_of(&self, local: &str) -> Vec<(u64, String)> {
        self.results.of_local(local).into_iter().map(|r| (r.id, r.text.clone())).collect()
    }

    /// The results of this circle bank, linked here or not, for its Bank settings page.
    pub fn results_of_circle_bank(&self, cid: &str, bid: &str) -> Vec<(u64, String)> {
        self.results.of_circle_bank(cid, bid).into_iter().map(|r| (r.id, r.text.clone())).collect()
    }

    /// The results of circle banks with no copy on this computer, named, for the top of the Banks list.
    pub fn results_bankless(&self) -> Vec<(u64, String)> {
        self.results.bankless().into_iter().map(|r| (r.id, if r.bank.name.is_empty() { r.text.clone() } else { format!("\"{}\": {}", r.bank.name, r.text) })).collect()
    }

    /// The results drawn above the open screen: those of the screen's own bank only (critic 69 I2). (critic 88 I3) Above a
    /// review that can't read the version a send with no readable answer may have made, not what the check said of that send
    /// (it went through, by the name on it): the review says what is known; the strip keeps the note.
    pub fn page_results(&self) -> Vec<(u64, String)> {
        let own_unread = matches!(&self.view, EditView::Merge(m) if matches!(&m.phase, MergePhase::Review(r) | MergePhase::Applying(r) if r.own_unread.is_some()));
        if let Some(l) = self.view_local() {
            return self.results.of_local(&l).into_iter().filter(|r| !(own_unread && r.kind == ResultKind::Send)).map(|r| (r.id, r.text.clone())).collect();
        }
        match self.view_circle() {
            Some((cid, bid)) => self.results_of_circle_bank(&cid, &bid),
            None => Vec::new(),
        }
    }

    /// "Dismiss" on a result.
    pub fn dismiss_result(&mut self, id: u64) {
        self.results.dismiss(id);
    }

    /// (critic 71 I1) The app's frame (egui pass): the History counts as shown only while it is drawn.
    pub fn set_pass(&mut self, nr: u64) {
        self.pass = Some(nr);
    }

    /// The History, or one of its versions, was drawn in this frame.
    pub fn mark_drawn(&mut self, nr: u64) {
        self.drawn = Some(nr);
    }

    /// The open History is in front of the person: drawn in this frame or the last one (always, until the app says
    /// which frame it is).
    fn history_shown(&self) -> bool {
        match (self.pass, self.drawn) {
            (None, _) => true,
            (Some(p), Some(d)) => d.saturating_add(1) >= p,
            (Some(_), None) => false,
        }
    }

    /// The bank the open screen is about, if it has one on this computer.
    fn view_local(&self) -> Option<String> {
        match &self.view {
            EditView::Send(v) => Some(v.local_id.clone()),
            EditView::Merge(v) => Some(v.local_id.clone()),
            EditView::History(h) => h.local_id.clone(),
            EditView::Version(_) => self.history_stash.as_ref().and_then(|h| h.local_id.clone()),
            EditView::Question(p) => Some(p.local_bank_id.clone()),
            EditView::Closed => None,
        }
        .or_else(|| self.host.as_ref().map(|h| h.local_id.clone()))
    }

    /// The circle bank the open screen is about.
    fn view_circle(&self) -> Option<(String, String)> {
        match &self.view {
            EditView::Send(v) => Some((v.cid.clone(), v.bid.clone())),
            EditView::Merge(v) => Some((v.cid.clone(), v.bid.clone())),
            EditView::History(h) => Some((h.cid.clone(), h.bid.clone())),
            EditView::Version(v) => Some((v.cid.clone(), v.bid.clone())),
            EditView::Question(p) => Some((p.cid.clone(), p.bid.clone())),
            EditView::Closed => None,
        }
    }

    /// The local bank linked to this circle bank.
    fn local_of(&self, cid: &str, bid: &str) -> Option<String> {
        self.links.iter().find(|l| l.circle_id == cid && l.circle_bank_id == bid).map(|l| l.local_bank_id.clone())
    }

    /// (critic 69 I1) The History is left (Back, the Circles tab, another bank's screen): a restore or trim result the
    /// person has not closed is said on the bank's strip. One still on its way is kept (see `close`), and its answer
    /// is said there when it comes.
    fn leave_view(&mut self) {
        // (critic 74 I1, critic 75 I1) The answer on the Send page that the person did not act on there (a failure, a
        // hold, a conflict, the daily limit, the "Sent" box of a send whose answer could not be read): said on the
        // bank's strip. It may have landed while the page was left undrawn (the person on another bank's page), and
        // whatever closes the page now (another bank's screen, "Circle page", the Shared page, a sign-out) would drop
        // it. A send that simply went through says nothing more.
        let note = self.send_note.take();
        if let EditView::Send(v) = &self.view {
            if !matches!(v.stage, SendStage::Editing | SendStage::Working(_))
                && let Some((bank, kind, text)) = note
            {
                self.say(bank, kind, text);
            }
            return;
        }
        self.park_result();
    }

    /// (critic 71 I1) The History was not drawn since the last frame (left open on the Shared page while the person is
    /// on a bank's page, or on another tab): a result on it is said on its bank's strip and Bank settings instead.
    fn park_unseen(&mut self) {
        if !self.history_shown() {
            self.park_result();
        }
    }

    /// A restore or trim result on the History (or the one kept while a version is looked at) the person has not
    /// closed: said for its bank, and taken off the History (it is said once).
    fn park_result(&mut self) {
        let h = match &self.view {
            EditView::History(h) => Some(&**h),
            EditView::Version(_) => self.history_stash.as_deref(),
            _ => None,
        };
        let Some(h) = h else { return };
        let (cid, bid, title) = (h.cid.clone(), h.bid.clone(), h.title.clone());
        // (critic 77 I2) a restore with no clear answer whose check answered meanwhile: what the check found
        let found = h.restore.as_ref().filter(|r| matches!(r.stage, RestoreStage::Unclear(_))).and_then(|_| self.unclear_restores.get(&(cid.clone(), bid.clone()))).and_then(|(_, _, f, _)| f.clone());
        let restore = found.or_else(|| h.restore.as_ref().and_then(|r| restore_note(r.to, Some(&r.stage))));
        let trim = h.trim.as_ref().and_then(|t| trim_note(Some(&t.stage)));
        if restore.is_none() && trim.is_none() {
            return;
        }
        let bank = self.result_bank(&cid, &bid, &title);
        let h = match &mut self.view {
            EditView::History(h) => Some(&mut **h),
            EditView::Version(_) => self.history_stash.as_deref_mut(),
            _ => None,
        };
        if let Some(h) = h {
            if restore.is_some() {
                h.restore = None;
            }
            if trim.is_some() {
                h.trim = None;
            }
        }
        if let Some(t) = restore {
            self.say(bank.clone(), ResultKind::Restore, t);
        }
        if let Some(t) = trim {
            self.say(bank, ResultKind::Trim, t);
        }
    }

    /// The name a send, restore or trim of this circle bank goes by: the bank's name in My Banks, else the circle's.
    fn act_name(&self, cid: &str, bid: &str) -> String {
        let local = self.links.iter().find(|l| l.circle_id == cid && l.circle_bank_id == bid);
        let mine = local.and_then(|l| self.statuses.get(&l.local_bank_id)).map(|s| s.bank_name.clone()).filter(|n| !n.is_empty());
        let shown = match &self.view {
            EditView::History(h) if h.cid == cid && h.bid == bid => Some(h.title.clone()),
            _ => None,
        };
        api::plain(&mine.or(shown).or_else(|| local.map(|l| l.title.clone())).unwrap_or_default())
    }

    pub fn take_signed_out(&mut self) -> bool {
        std::mem::take(&mut self.signed_out)
    }

    /// "Open account..." on one of these screens (critic 66 I7): the page hosting them opens the Account window.
    pub fn ask_account(&mut self) {
        self.account_ask = true;
    }

    pub fn take_account_ask(&mut self) -> bool {
        std::mem::take(&mut self.account_ask)
    }

    pub fn take_advanced(&mut self) -> Vec<Advanced> {
        std::mem::take(&mut self.advanced)
    }

    /// The screens close and their jobs go, except a send, restore or trim already on its way. A send's answer only
    /// moves this computer's link on (critic 61 I4, see `on_sent`: "a send that went through still moves the link
    /// on"), so the link is never left behind a version the person sent, and their next send is not refused as a
    /// conflict with it. A restore's or trim's answer is said on the bank's strip (critic 69 I1): the person was told
    /// to wait for it.
    pub fn close(&mut self) {
        self.leave_view();
        // (critic 89 m2) and a quiet read of a base version on its way: dropped here, it would be asked again beside the one
        // still running at the server
        self.jobs.retain(|j| matches!(j.kind, JobKind::Act | JobKind::Base));
        // (round 15, survivors x_close_inflight and c93 m24) what a send carried is read only by its own answer (`on_sent` takes
        // it) or its lost job (`fail_kind`), and every send sets it first: one left here is never read, so nothing is dropped
        self.view = EditView::Closed;
        self.history_stash = None;
        self.cmd_send = false;
        self.applying = None;
        self.carry = None;
        self.host = None;
        self.rebase_next = false;
        // (phase D3) the send page is abandoned: a carried selection does not outlive it (a retry record keeps its own)
        self.send_unticked = None;
    }

    /// The account signed out or another signed in (`another`, critic 60 I1): the screens close (a history goes with
    /// the versions refused to that account), and nothing that account was told or asked for stays. The working-copy
    /// links on this computer are kept, and so is a send already on its way (see `close`). Critic 61 I4: after a
    /// sign-out the commit ids of the last send and restore are kept too, so the same account, signed in again,
    /// retries a send or a restore whose answer it never read as the same commit (the pool replays it, nothing is
    /// doubled); another account starts its own.
    pub fn forget_account(&mut self, another: bool) {
        self.close();
        // (phase D2) the question history's reads belong to the account that made them
        self.question_cache.forget();
        self.outs += 1;
        self.invite_required = false;
        if another {
            self.pending_send = None;
            self.pending_restore = None;
            // (critic 78 I1) the earlier account's send with no clear answer is not this account's (what it carried
            // stays with the link, for that account: see `record_mine`)
            self.unclear_noted.clear();
            self.me_name = None;
            self.retry_send = None;
            self.unclear_restores.clear();
            // (critic 87 B1) a version this account can't read may be readable to the next one; (critic 89 m2) a quiet read
            // asked with the earlier account's sign-in is not this one's
            self.base_reads.clear();
            self.jobs.retain(|j| j.kind != JobKind::Base);
            // (critic 69 I1, critic 71) this account's turn: a send, restore or trim the first account asked for is
            // not told to this one, and nothing the first one was told stays. (A sign-out keeps the results: the same
            // account signing in again sees them.)
            self.turn += 1;
            self.results.clear();
            // (critic 75 minor 5) A send that could not be read is kept in the link with the account that sent it (told
            // again when that account is back); one whose account is not known is not told to this one. The account
            // signed in now is learnt at the next frame.
            self.account = None;
            // (critic 83 minor) The same for what a send with no readable outcome carried: one whose account is not known is
            // that turn's only, never the next account's own.
            let unknown: Vec<String> = self.links.iter().filter(|l| l.sent_unread.as_ref().is_some_and(|u| u.by.is_empty()) || l.sent_record.as_ref().is_some_and(|r| r.by.is_empty())).map(|l| l.local_bank_id.clone()).collect();
            for local in unknown {
                self.update_link(&local, |l| {
                    let unread = l.sent_unread.take_if(|u| u.by.is_empty()).is_some();
                    let record = l.sent_record.take_if(|r| r.by.is_empty()).is_some();
                    unread || record
                });
            }
        }
        self.signed_out = false;
        self.last_act = None;
        self.merge_note = None;
        self.forget_ask = None;
        self.put_all_ask = None;
        // (read for the account: read again with the circle page)
        self.hiding.clear();
    }

    // ---- the links -----------------------------------------------------------------------------------------

    fn ensure(&mut self) {
        // (critic 65 minor) A file that could not be read is read again when the person asks ("Try again", or the
        // Shared page opening), not on every badge and every frame.
        if !self.loaded && (self.unreadable.is_none() || std::mem::take(&mut self.read_again)) {
            match self.log.load_checked() {
                Ok(file) => {
                    // (critic 65 B1, critic 67 k01) Nothing is ever linked before the file is read: every way in
                    // (`add_link`, `add_link_replacing`, `take_shared_link`) reads it first and refuses while it can
                    // not be read, so the list here is the file's.
                    self.links = file;
                    self.loaded = true;
                    self.unreadable = None;
                    note_linked(&self.links);
                }
                Err(m) => self.unreadable = Some(m),
            }
        }
        // (phase D1) Links made by the publish window: a bank just shared into a circle.
        for l in take_queued() {
            self.take_shared_link(l);
        }
    }

    /// A bank shared into a circle becomes the linked bank (Q6), unless it is already linked to another circle bank:
    /// that link stays (one link per bank), and the publish window says so.
    fn take_shared_link(&mut self, link: CircleLink) {
        let bid = link.circle_bank_id.clone();
        // (critic 65 B1) While the file can not be read, no bank can be told apart: nothing is linked.
        if let Some(m) = &self.unreadable {
            note_outcome(&bid, LinkOutcome::NotSaved(unreadable_words(m)));
            return;
        }
        if self.links.iter().any(|l| l.local_bank_id == link.local_bank_id) {
            note_outcome(&bid, LinkOutcome::AlreadyLinked);
            return;
        }
        match self.put_link(link) {
            Ok(()) => note_outcome(&bid, LinkOutcome::Linked),
            Err(m) => note_outcome(&bid, LinkOutcome::NotSaved(m)),
        }
    }

    /// "Try again" on a links file that could not be read: the next use reads it again.
    pub fn read_links_again(&mut self) {
        self.read_again = true;
    }

    /// The links file is there but could not be read (nothing is written over it): the words why.
    pub fn unreadable(&mut self) -> Option<String> {
        self.ensure();
        self.unreadable.clone()
    }

    /// Where the link of this bank stands, counted from this computer only (the Banks list and a bank's page, for
    /// the banks on screen). Cached by the bank's `modified` and question count, as the circle page's counts are.
    pub fn status_of(&mut self, bank: &QuestionBank) -> Option<LinkStatus> {
        self.ensure();
        let links = std::mem::take(&mut self.links);
        if let Some(l) = links.iter().find(|l| l.local_bank_id == bank.id) {
            self.refresh_one(l, Some(bank));
        }
        let found = links.iter().any(|l| l.local_bank_id == bank.id);
        self.links = links;
        if found { self.statuses.get(&bank.id).cloned() } else { None }
    }

    /// (critic 92 I2) The banks in My Banks linked to circle `cid`, that the person can send from, with changes not sent yet as
    /// their counts last said: the bank's name, and how many (`None`: they can't be counted). For the leave panel.
    pub fn unsent_in(&mut self, cid: &str) -> Vec<(String, Option<usize>, bool)> {
        self.unsent_of(cid, None)
    }

    /// (round 16, critic 94 m6) [`unsent_in`](Self::unsent_in) for one circle bank (removing it).
    pub fn unsent_of_bank(&mut self, cid: &str, bid: &str) -> Vec<(String, Option<usize>, bool)> {
        self.unsent_of(cid, Some(bid))
    }

    fn unsent_of(&mut self, cid: &str, bid: Option<&str>) -> Vec<(String, Option<usize>, bool)> {
        self.ensure();
        let sendable = |l: &CircleLink| l.circle_id == cid && bid.is_none_or(|b| l.circle_bank_id == b) && l.can_send() && super::bank_links::role_of(l).can_add_banks();
        self.links
            .iter()
            .filter(|l| sendable(l))
            .filter_map(|l| {
                let s = self.statuses.get(&l.local_bank_id).filter(|s| !s.missing)?;
                let n = s.changes.total();
                // (round 16, critic 93 minor 3, q93_4) a copy waiting for the circle to be checked can't be sent now: named
                // whatever is counted, with its count only where its strip says one ("not kept")
                if let Some(w) = s.learning {
                    let n = (w == Learning::NotKept && s.changes.partial() && n > 0).then_some(n);
                    return Some((super::api::plain(&s.bank_name), n, true));
                }
                match (s.changes.known || s.changes.partial(), n) {
                    (true, 0) => None,
                    (true, n) => Some((super::api::plain(&s.bank_name), Some(n), false)),
                    (false, _) => Some((super::api::plain(&s.bank_name), None, false)),
                }
            })
            .collect()
    }

    /// (critic 88 I2) The version the copy last matched, as its link says.
    pub fn base_version_of(&self, local_id: &str) -> Option<u64> {
        self.links.iter().find(|l| l.local_bank_id == local_id).map(|l| l.base_version)
    }

    /// The link of a circle bank, without copying its snapshot (`f` reads it).
    pub fn with_bank_link<R>(&mut self, cid: &str, bid: &str, f: impl FnOnce(&CircleLink) -> R) -> Option<R> {
        self.ensure();
        self.links.iter().find(|l| l.circle_id == cid && l.circle_bank_id == bid).map(f)
    }

    /// The link of a local bank, without copying its snapshot (`f` reads it).
    pub fn with_link<R>(&mut self, local_id: &str, f: impl FnOnce(&CircleLink) -> R) -> Option<R> {
        self.ensure();
        self.links.iter().find(|l| l.local_bank_id == local_id).map(f)
    }

    /// Changes one link with `f` and saves the list when `f` says something changed (a check's answer).
    pub fn update_link(&mut self, local_id: &str, f: impl FnOnce(&mut CircleLink) -> bool) {
        self.ensure();
        let Some(mut l) = self.links.iter().find(|l| l.local_bank_id == local_id).cloned() else { return };
        if f(&mut l) {
            self.replace_link(l);
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

    /// (round 16, critic 94 m1) The copy waits for the circle's version it last matched, which the circle would not show:
    /// "Use my bank as it is" is the way to send from it before someone sends a newer version.
    pub fn cant_be_read(&self, local_id: &str) -> bool {
        self.statuses.get(local_id).is_some_and(|s| s.learning == Some(Learning::NotKept))
    }

    pub fn status(&self, local_id: &str) -> Option<&LinkStatus> {
        self.statuses.get(local_id)
    }

    /// Remembers a new working copy. The copy itself is already in the banks. Critic 63 B3: one link per circle
    /// bank, and a link is never taken from another bank of this computer that still has it (its unsent work could
    /// no longer reach the circle): that is refused, and the words say which bank has it.
    pub fn add_link(&mut self, link: CircleLink) -> Result<(), String> {
        self.ensure();
        if let Some(m) = &self.unreadable {
            return Err(unreadable_words(m));
        }
        if let Some(name) = self.linked_elsewhere(&link.circle_id, &link.circle_bank_id, &link.local_bank_id) {
            return Err(already_linked(&name));
        }
        self.put_link(link)
    }

    /// "Add a full copy": the new bank takes the link of the circle bank on purpose (the earlier copy stays in My
    /// Banks, no longer connected; the page says so).
    pub fn add_link_replacing(&mut self, link: CircleLink) -> Result<(), String> {
        self.ensure();
        if let Some(m) = &self.unreadable {
            return Err(unreadable_words(m));
        }
        self.put_link(link)
    }

    /// Puts a link in the list and saves it. (critic 65 B1) A link that could not be saved is not kept as if it
    /// were: the list stays as it was, so nothing the page said "could not be saved" is used.
    fn put_link(&mut self, link: CircleLink) -> Result<(), String> {
        let before = self.links.clone();
        self.links.retain(|l| l.local_bank_id != link.local_bank_id && !(l.circle_id == link.circle_id && l.circle_bank_id == link.circle_bank_id));
        self.links.push(link);
        if let Err(m) = self.save() {
            self.links = before;
            note_linked(&self.links);
            return Err(m);
        }
        Ok(())
    }

    /// The name of another bank of this computer that is linked to this circle bank and still here (`None` when
    /// none is, or its bank is known to be gone). A bank not counted yet is taken as here.
    pub fn linked_elsewhere(&mut self, cid: &str, bid: &str, local_id: &str) -> Option<String> {
        self.ensure();
        let l = self.links.iter().find(|l| l.circle_id == cid && l.circle_bank_id == bid && l.local_bank_id != local_id)?;
        match self.statuses.get(&l.local_bank_id) {
            Some(s) if s.missing => None,
            Some(s) => Some(s.bank_name.clone()),
            None => Some(String::new()),
        }
    }

    /// Stops following the circle bank. The local bank stays as it is.
    pub fn forget(&mut self, local_id: &str) {
        self.ensure();
        self.forget_maybe(local_id);
        let before = self.links.clone();
        let bank = self.result_bank_of_local(local_id);
        self.links.retain(|l| l.local_bank_id != local_id);
        self.statuses.remove(local_id);
        match self.save() {
            // (the results of a link that is gone go with it)
            Ok(()) => self.results.forget_local(local_id),
            Err(m) => {
                // (critic 71 I2) The file still has the link: it is kept here too (as a link that could not be saved
                // is never kept, critic 65 B1), and the bank's strip says the unlink did not happen.
                self.links = before;
                note_linked(&self.links);
                if let Some(b) = bank {
                    self.say(b, ResultKind::Unlink, format!("The bank could not be unlinked: the list of shared banks could not be saved ({m}). It is still linked."));
                }
            }
        }
    }

    fn save(&mut self) -> Result<(), String> {
        // Never write over a file that could not be read: it may hold every link.
        self.ensure();
        if let Some(m) = &self.unreadable {
            return Err(format!("the list of shared banks could not be read ({m}), so it was left as it is"));
        }
        note_linked(&self.links);
        let r = self.log.save(&self.links);
        if r.is_ok() {
            // (critic 71) every link in memory is in the file now: "could not be saved" is no longer so
            self.results.drop_kind(ResultKind::Check);
            self.links_unsaved = None;
        } else {
            // (round 18, critic 97 m6) written again a little later (see `links_again`)
            self.links_unsaved = Some(self.now.unwrap_or_else(Instant::now));
        }
        r
    }

    /// (round 18, critic 97 m6) A links file that could not be written is written again `BASE_FIRST_RETRY` after the last try, so
    /// that what the links say (a pass done, its news) reaches the file once it can.
    fn links_again(&mut self) {
        let now = self.now.unwrap_or_else(Instant::now);
        if self.links_unsaved.is_some_and(|at| now.saturating_duration_since(at) >= BASE_FIRST_RETRY) {
            let _ = self.save();
        }
    }

    /// (round 18, critic 97 m6) Changes one link with `f` and saves the list now, saying whether it could be saved (the change
    /// stays in memory either way, and is written again later).
    fn save_link_now(&mut self, local_id: &str, f: impl FnOnce(&mut CircleLink)) -> Result<(), String> {
        let Some(l) = self.links.iter_mut().find(|l| l.local_bank_id == local_id) else { return Ok(()) };
        f(l);
        self.save()
    }

    fn replace_link(&mut self, link: CircleLink) {
        self.ensure();
        let local = link.local_bank_id.clone();
        match self.links.iter_mut().find(|l| l.local_bank_id == link.local_bank_id) {
            Some(l) => *l = link,
            None => return,
        }
        if let Err(m) = self.save()
            && let Some(b) = self.result_bank_of_local(&local)
        {
            self.say(b, ResultKind::Check, format!("The bank's link was updated, but the list could not be saved ({m}). The next check will catch up."));
        }
    }

    /// The quiet check of the circles' newest versions: a link that is behind says so.
    pub fn apply_heads(&mut self, entries: &[HeadEntry]) {
        self.ensure();
        let mut changed = Vec::new();
        for l in &mut self.links {
            let Some(e) = entries.iter().find(|e| e.bank_id == l.circle_bank_id) else { continue };
            if let HeadState::Current { version, at, by, .. } = &e.state
                && *version >= l.head_seen
                && (*version != l.head_seen || l.head_seen_at != *at)
            {
                l.head_seen = *version;
                l.head_seen_at = *at;
                l.head_seen_by = by.clone();
                changed.push(l.local_bank_id.clone());
            }
        }
        for local in &changed {
            self.head_moved(local);
        }
        if !changed.is_empty()
            && let Err(m) = self.save()
        {
            // (critic 71 I2) said on the strip of each bank it changed
            for local in changed {
                if let Some(b) = self.result_bank_of_local(&local) {
                    self.say(b, ResultKind::Check, format!("The result of the check could not be saved ({m})."));
                }
            }
        }
    }

    // ---- frame ------------------------------------------------------------------------------------------------

    pub fn frame(&mut self, now: Instant, snap: &AccountSnapshot) {
        self.now = Some(now);
        self.session = snap.session.clone();
        self.gate = snap.gate.clone();
        // ("your session ended" is no longer so once the account is signed in again). Round 54: a confirmed pending
        // account is signed in for circle work (merge base reads, restores, trims).
        let ready = self.gate == Gate::Ready || self.gate.confirmed_pending();
        self.learn_account(snap);
        if ready && !self.was_ready {
            self.results.drop_kind(ResultKind::Session);
        }
        self.was_ready = ready;
        self.drain();
        self.stop_waiting(now);
        self.park_unseen();
        self.read_bases();
    }

    /// (critic 87 B1) A base an earlier build wrote kept no difficulty: the circle's own version at the link's base version is
    /// read quietly, one at a time while signed in, and each difficulty is learnt from it (never from the bank). One that
    /// can't be read is asked again only later (critic 89 m4): the count then stays not complete, and Get latest leaves a
    /// differing difficulty to the person. (critic 89 m3) Never for a copy that can't send (it shows no count, and a viewer's
    /// read of a bank that hides its keys is one of the few they have); (critic 89 m2) never beside Get latest's own read of
    /// that bank.
    fn read_bases(&mut self) {
        if !self.loaded || !(self.gate == Gate::Ready || self.gate.confirmed_pending()) || self.running(JobKind::Base) {
            return;
        }
        let now = self.now.unwrap_or_else(Instant::now);
        let merging = if let EditView::Merge(m) = &self.view { Some(m.local_id.as_str()) } else { None };
        // (critic 91 minor) by this account's role: one another account on this computer saw is not asked by
        let me = super::bank_links::account_tag(self.account.as_deref());
        let next = self
            .links
            .iter()
            .filter(|l| awaits_read(l) && (l.role_seen_by.is_none() || l.role_seen_by == me) && merging != Some(l.local_bank_id.as_str()))
            .find(|l| self.base_read_due(l, now))
            .map(|l| (l.circle_id.clone(), l.circle_bank_id.clone(), l.base_version));
        if let Some((cid, bid, n)) = next {
            // (a read dropped on its way by another screen is asked again at the next frame; see `drain` for one whose thread
            // died)
            self.base_asked = Some((cid.clone(), bid.clone(), n));
            self.base_since = Some(now);
            self.launch(Work::BaseRead { cid, bid, n });
        }
    }

    /// (round 15) "Try again" or "Check now" on a copy waiting for the read of its base version: it is asked at the next frame.
    pub fn read_base_again(&mut self, local: &str) {
        // (round 16) and difficulties that could not be written into the bank are tried again at once (round 17, critic 96
        // minor 4: the hold stays, so that a try that fails again is said to have been tried again)
        if let Some(h) = self.inherit_held.get_mut(local) {
            h.at = None;
        }
        // (round 19, critic 100 I2) and the banks are written again
        if self.banks_unsaved.contains_key(local) {
            self.banks_again = true;
        }
        if let Some(l) = self.links.iter().find(|l| l.local_bank_id == local) {
            let key = (l.circle_id.clone(), l.circle_bank_id.clone(), l.base_version);
            self.base_reads.remove(&key);
        }
    }

    /// (critic 91 minor) What the open Get latest says while it waits for the quiet read of its base version: that it is
    /// checking, and after `BASE_WAIT` that the circle can't be reached just now. `None` when it is not waiting for it.
    pub fn merge_wait_note(&self) -> Option<String> {
        // (round 17, critic 95 minor 1) the bank waits for the circle's difficulties: said with why and what happens next
        if self.merge_waits_for_difficulty() {
            let local = if let EditView::Merge(m) = &self.view { m.local_id.as_str() } else { "" };
            return self.inherit_held.get(local).map(|h| text::merge_waits_held(&h.why));
        }
        if !matches!(&self.view, EditView::Merge(m) if matches!(m.phase, MergePhase::Ready(_))) || !self.merge_waits_for_base() {
            return None;
        }
        let now = self.now.unwrap_or_else(Instant::now);
        let long = self.base_since.is_some_and(|t| now.saturating_duration_since(t) >= BASE_WAIT);
        // (round 16, critic 94 m2) "couldn't check" only for a read that failed before; one that is only slow is still checking
        let local = if let EditView::Merge(m) = &self.view { m.local_id.as_str() } else { "" };
        let failed = self.links.iter().find(|l| l.local_bank_id == local).is_some_and(|l| matches!(self.base_reads.get(&(l.circle_id.clone(), l.circle_bank_id.clone(), l.base_version)), Some(BaseRead::NotReached(..))));
        Some(match (long, failed) {
            (false, _) => text::CHECKING,
            (true, false) => text::STILL_CHECKING,
            (true, true) => text::CHECKING_LONG,
        }
        .to_string())
    }

    /// (critic 89 m4) This link's base version may be asked for now: never asked, or what stopped the last read may have passed
    /// (a minute for a circle that could not be reached; a while, or a change in the circle's state of the bank, for one that
    /// would not show it).
    fn base_read_due(&self, l: &CircleLink, now: Instant) -> bool {
        match self.base_reads.get(&(l.circle_id.clone(), l.circle_bank_id.clone(), l.base_version)) {
            None => true,
            Some(BaseRead::NotReached(at, tries)) => now.saturating_duration_since(*at) >= if *tries <= 1 { BASE_FIRST_RETRY } else { BASE_RETRY },
            Some(BaseRead::Limited(until)) => now >= *until,
            Some(BaseRead::Unreadable(at, seen, _)) => now.saturating_duration_since(*at) >= BASE_UNREADABLE_RETRY || *seen != circle_state(l),
        }
    }

    fn on_base_read(&mut self, cid: &str, bid: &str, n: u64, r: Result<BankDoc, PoolError>) {
        // (`base_asked` says which read is on its way only while a Base job runs: every reader asks `running(JobKind::Base)`
        // first and every launch sets it again, so clearing it here or not is the same (survivors b_asked_kept, c93 m23))
        let key = (cid.to_string(), bid.to_string(), n);
        let state = match r {
            // (only the version asked for: survivor c2)
            Ok(doc) if doc.bank.version == n => {
                let theirs = circle_theirs_as(&doc.payload, self.now_ms(), doc.bank.keys_hidden);
                // (what it taught is kept on the link: it is not read for again)
                self.learn_base(cid, bid, n, &theirs);
                return;
            }
            // (round 16, critic 93 minor 4) the circle's daily limit is not "can't reach": waited out as it says
            Err(PoolError::Refused(r)) if matches!(r.kind, RefusalKind::DailyLimit { .. }) => {
                // (round 17, critic 95 minor 5) a huge Retry-After is capped, never an overflow
                let secs = r.detail.retry_after.filter(|s| *s > 0).unwrap_or(BASE_UNREADABLE_RETRY.as_secs()).min(api::MAX_WAIT_SECS);
                BaseRead::Limited(api::later(self.now.unwrap_or_else(Instant::now), secs))
            }
            Err(e) if not_reached(&e) => self.not_reached_now(&key),
            // (round 18, critic 98 minor 3) said by why: no longer kept, held back for review, or a reply that can't be read
            Err(PoolError::Refused(r)) if matches!(r.kind, RefusalKind::VersionNotRetained) => self.unreadable_now(cid, bid, Unread::NotKept),
            Err(PoolError::Refused(r)) if matches!(r.kind, RefusalKind::VersionWithheld) => self.unreadable_now(cid, bid, Unread::Withheld),
            _ => self.unreadable_now(cid, bid, Unread::Other),
        };
        self.base_reads.insert(key, state);
    }

    /// (round 16, critic 94 m2) The read of this base version could not reach the circle (once more in a row).
    fn not_reached_now(&self, key: &(String, String, u64)) -> BaseRead {
        let tries = match self.base_reads.get(key) {
            Some(BaseRead::NotReached(_, n)) => n.saturating_add(1),
            _ => 1,
        };
        BaseRead::NotReached(self.now.unwrap_or_else(Instant::now), tries)
    }

    /// (critic 89 m4) A base version of this circle bank can't be read now, as the circle's state of it is.
    fn unreadable_now(&self, cid: &str, bid: &str, why: Unread) -> BaseRead {
        let seen = self.links.iter().find(|l| l.circle_id == cid && l.circle_bank_id == bid).map(circle_state).unwrap_or_default();
        BaseRead::Unreadable(self.now.unwrap_or_else(Instant::now), seen, why)
    }

    /// (critic 87 B1) The link of this circle bank, still at version `n`, learns its base's difficulties from that version.
    /// (critic 89 m1) Its count is worked out again (what it learnt may change it, also where it still knows not all).
    fn learn_base(&mut self, cid: &str, bid: &str, n: u64, theirs: &[merge::Theirs]) {
        // (round 16, critic 93 I2) only while it is awaited: "Use my bank as it is" settled it, whatever arrives after
        let Some(local) = self.links.iter().find(|l| l.circle_id == cid && l.circle_bank_id == bid && l.base_version == n && needs_difficulty_read(l)).map(|l| l.local_bank_id.clone()) else { return };
        self.update_link(&local, |l| learn_difficulty(l, theirs));
        self.recount(&local);
    }

    /// (critic 89 m1) The bank's count is worked out again at the next pass (what the link knows changed, not the bank).
    /// What the status says meanwhile (a bank put back, say) stays until then.
    fn recount(&mut self, local: &str) {
        if let Some(s) = self.statuses.get_mut(local) {
            s.key = (i64::MIN, usize::MAX);
        }
    }

    /// (the owner's rule; round 15) A copy whose difficulties were just learnt takes the circle's difficulty for each question
    /// the version has (see `inherit_difficulty`). The bank's time moves on (a preview made from it is made again), and a send
    /// whose answer could not be read is not said to have changes after it for this. (critic 91 I1) What moved is kept for the
    /// open question editor (see [`refresh_draft`]). Returns true when a bank changed (the caller saves the banks).
    ///
    /// (round 16, critic 93 I1, critic 94 O1) Recoverable: a restore point is saved before any rating is replaced, as a merge's,
    /// and the strip says once what was taken from which version; a rating she already pressed Send for is kept (see
    /// `inherit_difficulty`). (critic 93 minor 1) The banks file is written first, and only then does the links file say it
    /// was done: if either can't be saved nothing changes, Send waits, and it is tried again (see `inherit_held`). (critic 93
    /// minor 2) A base question the bank did not have then (deleted before the read) takes the circle's rating when it is
    /// put back.
    fn inherit_pending(&mut self, banks: &mut [QuestionBank]) -> bool {
        let now = self.now.unwrap_or_else(Instant::now);
        // a base version moved on (a send, a merge): what was missing then is no longer the circle's
        let mut dropped = false;
        for l in self.links.iter_mut().filter(|l| l.missing_at_read.as_ref().is_some_and(|(n, _)| *n != l.base_version)) {
            l.missing_at_read = None;
            dropped = true;
        }
        if dropped {
            let _ = self.save();
        }
        let mut work: Vec<(String, Option<Vec<String>>)> = self.links.iter().filter(|l| l.inherit).map(|l| (l.local_bank_id.clone(), None)).collect();
        for l in self.links.iter().filter(|l| !l.inherit) {
            let Some((_, ids)) = &l.missing_at_read else { continue };
            let Some(bank) = banks.iter().find(|b| b.id == l.local_bank_id) else { continue };
            let back: Vec<String> = ids.iter().filter(|id| bank.questions.iter().any(|q| &q.id == *id)).cloned().collect();
            if !back.is_empty() {
                work.push((l.local_bank_id.clone(), Some(back)));
            }
        }
        // (round 18, critic 97 m1) a hold with nothing left to try (its question deleted again, the copy moved on) goes
        self.inherit_held.retain(|local, _| work.iter().any(|(l, _)| l == local));
        let mut changed = false;
        for (local, only) in work {
            if self.inherit_held.get(&local).is_some_and(|h| h.at.is_some_and(|at| now.saturating_duration_since(at) < BASE_RETRY)) {
                continue;
            }
            let Some(pos) = banks.iter().position(|b| b.id == local) else { continue };
            // (a copy: the link is changed in the list as the pass goes)
            let Some(link) = self.links.iter().find(|l| l.local_bank_id == local).cloned() else { continue };
            // (round 17, critic 95 minor 2) signed out (no account known yet after a restart), her send record is hers to keep;
            // (round 18, critic 97 m5) one whose account is not the one this link was last seen by may be another account's: the
            // pass waits for a sign-in to tell
            let record = match &self.account {
                None => match link.sent_record.as_ref() {
                    Some(r) if !r.by.is_empty() && link.role_seen_by.as_deref() != Some(r.by.as_str()) => continue,
                    r => r.cloned(),
                },
                Some(_) => self.record_mine(&link).cloned(),
            };
            let mut bank = banks[pos].clone();
            let moved = inherit_difficulty(&link, &mut bank, record.as_ref(), only.as_deref());
            let version = link.base_version;
            // (round 17, critic 95 I1) the bank as this first pass finds it, kept on the link if the pass can't be saved; (round 18,
            // critic 97 m1) for a question put back after the read too, as the first pass for it finds it
            let at_read: Option<Vec<AtRead>> = match &only {
                None => link.at_read.is_none().then(|| link.base.iter().filter_map(|b| banks[pos].questions.iter().find(|q| q.id == b.local_id)).map(|q| AtRead { id: q.id.clone(), difficulty: q.difficulty }).collect()),
                Some(back) => {
                    let mut have = link.at_read.clone().unwrap_or_default();
                    let known = have.len();
                    for q in banks[pos].questions.iter().filter(|q| back.contains(&q.id)) {
                        if !have.iter().any(|a| a.id == q.id) {
                            have.push(AtRead { id: q.id.clone(), difficulty: q.difficulty });
                        }
                    }
                    (have.len() > known).then_some(have)
                }
            };
            if only.is_none() {
                // (the bank as the read found it: a base question it lacks may be put back) (round 19, critic 99 m4) kept on the link,
                // in the file
                let missing: Vec<String> = link.base.iter().filter(|b| b.snap.difficulty.is_some() && !banks[pos].questions.iter().any(|q| q.id == b.local_id)).map(|b| b.local_id.clone()).collect();
                let now_missing = (!missing.is_empty() && !link.stale_for(&banks[pos])).then_some((version, missing));
                if let Some(l) = self.links.iter_mut().find(|l| l.local_bank_id == local)
                    && l.missing_at_read != now_missing
                {
                    l.missing_at_read = now_missing;
                    let _ = self.save();
                }
            }
            // (round 18, critic 97 m1) a question put back is no longer waited for only once its pass worked: a held one is tried
            // again. Returns whether the link changed.
            let done_back = |l: &mut CircleLink| -> bool {
                let Some(back) = &only else { return false };
                let Some((_, ids)) = l.missing_at_read.as_mut() else { return false };
                let n = ids.len();
                ids.retain(|id| !back.contains(id));
                let changed = ids.len() != n;
                if ids.is_empty() {
                    l.missing_at_read = None;
                }
                changed
            };
            // (round 18, critic 97 m6) a pass that already ran (its banks file written, its links file not: written again after a
            // restart) is told by the bank as the read found it: what it gave the circle's difficulty is in the news again. (round 19,
            // critic 99 m6) Only where that write may have happened (the links file said the banks file was next, and no try of it
            // failed since): a question she set to the circle's difficulty herself during a hold is her choice, never the app's
            // (critic 101 m1) `pass_times` is written before the safety copy and the banks file, so it alone does not say the banks
            // write happened: the bank's time does (`after`, the time the pass wrote). Without that, a crash or a failed safety copy
            // with an unwritable links file could leave `pass_times` on file with the banks never written, and the pass would be
            // recorded as done. (A later edit of that same bank after such a failure can still look like the write.)
            let ran = link.pass_times.is_some_and(|(_, after)| banks[pos].modified >= after) && !self.inherit_held.contains_key(&local);
            let already = if ran { inherited_before(&link, &banks[pos]) } else { Vec::new() };
            // (round 19, critic 99 m2) such a pass is done: what differs from the circle's now was put back by her since (Put mine
            // back, or her own edit), and is never taken again; it is listed as hers, back. (critic 101 m1) `ran` alone says it:
            // "Put all of mine back" can leave `already` empty, and the pass must not run again over her put-back.
            if only.is_none() && ran {
                self.update_link(&local, |l| {
                    // (round 19, critic 99 m3) an unclear send's time moves with the pass, as it did in the session that ran it
                    if let Some((before, after)) = l.pass_times {
                        if let Some(u) = l.sent_unread.as_mut().filter(|u| u.modified == before) {
                            u.modified = after;
                        }
                        if let Some(r) = l.sent_record.as_mut().filter(|r| r.modified == before) {
                            r.modified = after;
                        }
                    }
                    l.inherit = false;
                    l.at_read = None;
                    l.pass_times = None;
                    add_news(l, version, &already);
                    add_news_back(l, version, &moved);
                    true
                });
                continue;
            }
            if moved.is_empty() {
                self.inherit_held.remove(&local);
                self.update_link(&local, |l| {
                    let was = done_back(l) | (l.inherit || l.at_read.is_some() || l.pass_times.is_some() || !already.is_empty());
                    l.inherit = false;
                    l.at_read = None;
                    l.pass_times = None;
                    add_news(l, version, &already);
                    was
                });
                continue;
            }
            let before = bank.modified;
            // Seconds, like every editor writes.
            bank.modified = (self.now_ms() / 1000).max(before.saturating_add(1));
            let after = bank.modified;
            // (round 18, critic 97 m6) the bank as the read found it goes into the links file before the banks file changes: if the
            // links file can't be written after the pass, a restart runs it again over what she has not changed since. (round 19,
            // critic 99 m1) At every try, with the times of this write: a retry never writes the banks file while the links file is
            // not in step with it, and holds instead
            if let Err(m) = self.save_link_now(&local, |l| {
                if let Some(a) = at_read {
                    l.at_read = Some(a);
                }
                l.pass_times = Some((before, after));
            }) {
                self.hold_inherit(&local, now, text::links_not_saved(&m));
                continue;
            }
            // the restore point first, as a merge's: if it can't be saved, nothing changes. (round 17, critic 95 minor 3) One
            // already saved for the banks as they are now is not saved again while the banks file can't be written; (round 18,
            // critic 97 m4) all the banks: work on another bank during the hold makes a new one
            // (round 19, critic 99 m5) one for the whole model, whatever the number of banks held
            let print = print_all(banks);
            if self.safety_copy_for.as_deref() != Some(print.as_str()) {
                if let Err(m) = self.restore.save_named(banks, "difficulty") {
                    self.hold_inherit(&local, now, text::safety_copy_not_saved(&m));
                    continue;
                }
                self.safety_copy_for = Some(print);
            }
            let mut next = banks.to_vec();
            next[pos] = bank;
            // then the banks file, and only then the links file
            if let Err(m) = self.save_banks(&next) {
                self.hold_inherit(&local, now, text::banks_not_saved(&m));
                continue;
            }
            banks[pos] = next.swap_remove(pos);
            self.banks_saved();
            self.inherit_held.remove(&local);
            let moved_news = moved.clone();
            self.update_link(&local, |l| {
                done_back(l);
                if let Some(u) = l.sent_unread.as_mut().filter(|u| u.modified == before) {
                    u.modified = after;
                }
                if let Some(r) = l.sent_record.as_mut().filter(|r| r.modified == before) {
                    r.modified = after;
                }
                l.inherit = false;
                l.at_read = None;
                l.pass_times = None;
                // (round 17, critic 96 I1) kept until she dismisses it: what replaced what
                add_news(l, version, &already);
                add_news(l, version, &moved_news);
                true
            });
            // (the line that said it could not be saved is out of date)
            if let Some(b) = self.result_bank_of_local(&local) {
                self.results.drop_of(&b, ResultKind::Ratings);
            }
            changed = true;
            self.inherited.extend(moved.into_iter().map(|(qid, before, after)| Inherited { local: local.clone(), qid, before, after }));
        }
        changed
    }

    /// (round 16, critic 93 I1) The circle's difficulties could not be written into this bank: nothing changed; said on its
    /// strip and badge (round 17: in plain words, with the tries), and tried again after `BASE_RETRY` (Send waits meanwhile: see
    /// `Learning::Held`). (round 19, critic 99 m2) The links file no longer says the banks file has the pass (written quietly; one
    /// that can't be is written again later).
    fn hold_inherit(&mut self, local: &str, now: Instant, why: String) {
        let h = self.inherit_held.entry(local.to_string()).or_insert(Held { at: None, why: String::new(), tries: 0 });
        h.at = Some(now);
        h.why = why;
        h.tries = h.tries.saturating_add(1);
        if let Some(l) = self.links.iter_mut().find(|l| l.local_bank_id == local)
            && l.pass_times.take().is_some()
        {
            let _ = self.save();
        }
        self.statuses.remove(local);
    }

    /// (round 17, critic 96 I1) "Put mine back" on one question of the difficulty news: her difficulty before the circle's
    /// replaced it comes back as an ordinary change she has not sent (the bank's time moves on; the open question editor is
    /// told). Returns true when the bank changed (the caller saves the banks).
    pub fn put_mine_back(&mut self, banks: &mut [QuestionBank], local: &str, qid: &str) -> bool {
        self.put_back_these(banks, local, &[qid.to_string()])
    }

    /// (round 18, critic 98 minor 6) "Put all of mine back": every question of the news that still offers it, in one save.
    pub fn put_all_mine_back(&mut self, banks: &mut [QuestionBank], local: &str) -> bool {
        let Some(link) = self.links.iter().find(|l| l.local_bank_id == local) else { return false };
        let Some(bank) = banks.iter().find(|b| b.id == local) else { return false };
        let ids: Vec<String> = link.difficulty_news.as_ref().map(|n| n.questions.iter().filter(|i| news_state(link, i, bank) == NewsState::Replaced).map(|i| i.id.clone()).collect()).unwrap_or_default();
        self.put_back_these(banks, local, &ids)
    }

    /// Her difficulty before the circle's, back on each of `ids` that still offers it (see `news_state`). (round 18, critic 97 m3) The
    /// banks file is written first; only then does the links file say it was put back. If the banks can't be saved nothing changes,
    /// the button stays, and the strip says why.
    fn put_back_these(&mut self, banks: &mut [QuestionBank], local: &str, ids: &[String]) -> bool {
        let Some(link) = self.links.iter().find(|l| l.local_bank_id == local) else { return false };
        let Some(pos) = banks.iter().position(|b| b.id == local) else { return false };
        // (round 18, critic 98 I1) only while the question still has the circle's difficulty from the read and the copy is still at
        // that version: never over her newer choice or a colleague's newer difficulty
        let items: Vec<NewsItem> = link.difficulty_news.as_ref().map(|n| n.questions.iter().filter(|i| ids.contains(&i.id) && news_state(link, i, &banks[pos]) == NewsState::Replaced).cloned().collect()).unwrap_or_default();
        if items.is_empty() {
            return false;
        }
        let mut next = banks.to_vec();
        let bank = &mut next[pos];
        let mut moved = Vec::new();
        for item in &items {
            if let Some(q) = bank.questions.iter_mut().find(|q| q.id == item.id) {
                moved.push(Inherited { local: local.to_string(), qid: item.id.clone(), before: q.difficulty, after: item.before });
                q.difficulty = item.before;
            }
        }
        let prev = bank.modified;
        // Seconds, like every editor writes.
        bank.modified = (self.now_ms() / 1000).max(prev.saturating_add(1));
        let result = self.result_bank_of_local(local);
        if let Err(m) = self.save_banks(&next) {
            // (round 19, critic 100 I2) her own edits aren't saved either: said on the strip in the warning colour until a save
            // works (never a line she can dismiss)
            self.not_saved(local, text::put_back_not_saved(&m));
            return false;
        }
        banks[pos] = next.swap_remove(pos);
        self.banks_saved();
        if let Some(b) = result {
            self.results.drop_of(&b, ResultKind::Ratings);
        }
        self.update_link(local, |l| {
            if let Some(n) = l.difficulty_news.as_mut() {
                for q in n.questions.iter_mut().filter(|q| items.iter().any(|i| i.id == q.id)) {
                    q.put_back = true;
                }
            }
            true
        });
        self.inherited.extend(moved);
        self.statuses.remove(local);
        true
    }

    /// (round 18, critic 98 I1) Each link's difficulty news, against its bank as it is: a question changed since is noted for good
    /// (saved with the links).
    fn settle_all_news(&mut self, banks: &[QuestionBank]) {
        // (in place: a link is not copied every frame for this)
        let mut changed = false;
        for l in self.links.iter_mut().filter(|l| l.difficulty_news.is_some()) {
            if let Some(bank) = banks.iter().find(|b| b.id == l.local_bank_id) {
                changed |= settle_news(l, bank);
            }
        }
        // (one that can't be saved now is written again later: see `links_again`)
        if changed {
            let _ = self.save();
        }
    }

    /// (round 17, critic 96 I1) "Dismiss" on the difficulty news: it goes from the badge and the strip.
    pub fn dismiss_news(&mut self, local: &str) {
        self.update_link(local, |l| l.difficulty_news.take().is_some());
    }

    /// (round 19, critic 100 I2) The banks file could not be written for a change she asked for on `local`'s page: said there (with
    /// the tries) until a save works.
    fn not_saved(&mut self, local: &str, line: String) {
        let e = self.banks_unsaved.entry(local.to_string()).or_insert((String::new(), 0));
        e.0 = line;
        e.1 = e.1.saturating_add(1);
        self.statuses.remove(local);
    }

    /// (round 19, critic 100 I2) The banks file was written: her edits are saved again.
    fn banks_saved(&mut self) {
        for local in std::mem::take(&mut self.banks_unsaved).into_keys() {
            self.statuses.remove(&local);
        }
    }

    /// (round 19, critic 100 I2) "Try again" after the banks file could not be written: all the banks, written now.
    fn banks_again(&mut self, banks: &[QuestionBank]) {
        if !std::mem::take(&mut self.banks_again) || self.banks_unsaved.is_empty() {
            return;
        }
        match self.save_banks(banks) {
            Ok(()) => self.banks_saved(),
            Err(_) => {
                for (local, (_, tries)) in self.banks_unsaved.iter_mut() {
                    *tries = tries.saturating_add(1);
                    self.statuses.remove(local);
                }
            }
        }
    }

    /// (round 16, critic 93 minor 1) All the banks, saved now (see `bank_saver`).
    fn save_banks(&self, banks: &[QuestionBank]) -> Result<(), String> {
        match &self.bank_saver {
            Some(f) => f(banks),
            None => self.log.save_banks(banks),
        }
    }

    /// (critic 73 minor 4, critic 77 minor 3) The account signed in, when the snapshot says (the Banks list learns it too,
    /// for its badges).
    pub fn learn_account(&mut self, snap: &AccountSnapshot) {
        if snap.gate == Gate::Ready {
            self.account = snap.account.clone();
            self.me_name = snap.name.clone();
        }
    }

    /// (critic 69 minor 7, critic 71) A restore or trim with no answer after `ACT_PATIENCE` is no longer waited for:
    /// the other screens open again, and the bank's strip says it may still go through (its commit id is kept: the
    /// same restore asked again is not made twice). A send is always waited for: its answer moves the link on.
    fn stop_waiting(&mut self, now: Instant) {
        let Some(a) = self.act_for.clone().filter(|a| a.what != "send" && self.running(JobKind::Act)) else { return };
        if now.saturating_duration_since(a.since) < ACT_PATIENCE {
            return;
        }
        self.jobs.retain(|j| j.kind != JobKind::Act);
        self.act_waited = false;
        if let Some(h) = self.history_of(&a.cid, &a.bid) {
            if h.restore.as_ref().is_some_and(|r| matches!(r.stage, RestoreStage::Working)) {
                h.restore = None;
            }
            if h.trim.as_ref().is_some_and(|t| matches!(t.stage, TrimStage::Working)) {
                h.trim = None;
            }
        }
        // (critic 73 minor 1) the circle's newest version may still change: never "the latest" or "up to date" until
        // the next check or History read
        self.unsure.insert((a.cid.clone(), a.bid.clone()));
        if !self.mine(&a) {
            return;
        }
        let said = if a.what == "restore" {
            format!("No answer came for the restore of version {} within a minute. It may still go through: open History to see whether it did.", a.arg)
        } else {
            "No answer came for the trim within a minute. It may still go through: open History to see whether it did.".to_string()
        };
        let bank = self.act_bank(&a.cid, &a.bid);
        self.say(bank, ResultKind::NoAnswer, said);
    }

    // ---- errors ------------------------------------------------------------------------------------------------

    /// An error as a `Problem`; `None` when the sign-in ended (the person is sent to sign in again).
    fn problem_of(&mut self, e: &PoolError) -> Option<Problem> {
        let now = self.now.unwrap_or_else(Instant::now);
        let mut p = Problem { message: e.message(), retry: false, retry_at: None, account: false };
        match e {
            PoolError::Api(ApiError::SignedOut) => {
                // (the screen that asked says it: the strip offers "Sign in...", the Shared page says so)
                self.signed_out = true;
                return None;
            }
            PoolError::Api(_) => p.retry = true,
            // (phase D3, plan section 6 "Pictures") A base picture that could not be fetched is worth trying again: the
            // Failed page then draws "Try again", which re-runs the send (and so the fetch). The line says so.
            PoolError::Local(LocalProblem::BaseMediaUnavailable) => p.retry = true,
            PoolError::Local(_) => {}
            PoolError::Refused(r) => {
                let wait = r.detail.retry_after.filter(|s| *s > 0).map(|s| api::later(now, s));
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
                            Some("circle_rollbacks") => p.message = "You have restored as many older versions today as verSHion allows. Try again tomorrow.".into(),
                            Some("circle_commits") => p.message = "You have sent as many versions to circles today as verSHion allows. Your changes are still on this device. Try again tomorrow.".into(),
                            // (round 17, critic 96 minor 3) reading the circle's banks: the circle's limit, when it resets, and
                            // no Try again (it would meet the same limit)
                            Some("item_reads") => {
                                p.message = text::read_limit_page(r.detail.retry_after);
                                p.retry = false;
                                p.retry_at = None;
                            }
                            _ => {}
                        }
                    }
                    RefusalKind::RateLimited | RefusalKind::TooManyOpenReads | RefusalKind::TooManyUploads | RefusalKind::Busy | RefusalKind::PublishInProgress => {
                        p.retry = true;
                        p.retry_at = wait;
                    }
                    RefusalKind::NotFound => p.message = "That circle or bank is not available to you any more. It may have been closed, removed or hidden, or you may have left the circle.".into(),
                    RefusalKind::ModerationState { message } => {
                        p.message = message.as_deref().map(api::plain).filter(|m| !m.is_empty()).unwrap_or_else(|| "verSHion does not allow that in this circle right now.".into());
                    }
                    RefusalKind::InvalidRequest { field } => {
                        p.message = match field.as_deref() {
                            Some("base_version") => "The circle has no newer version than the one you have.".into(),
                            Some("message") => "The note can be at most 500 characters.".into(),
                            Some("keep") => "Choose how many versions to keep, from 1 to 19.".into(),
                            Some("to_version") => "Choose an older version than the newest one.".into(),
                            Some("merge_token" | "merged_from") => "What you got is out of date. Get latest again, then send.".into(),
                            Some("commit_id") => "That send could not be identified. Start it again.".into(),
                            Some("license") => "Choose a licence for this bank.".into(),
                            Some(f) => format!("verSHion did not accept the {}. Nothing was changed.", f.replace('_', " ")),
                            None => "verSHion did not accept that. Nothing was changed.".into(),
                        }
                    }
                    RefusalKind::Other if r.detail.code == "pool_migrating" => {
                        p.message = "verSHion's server is being upgraded. Try again in a few minutes.".into();
                        p.retry = true;
                        p.retry_at = wait;
                    }
                    _ => {}
                }
            }
        }
        // (critic 68 minor 1) a refusal not mapped above says the circle, not the public pool
        p.message = super::circles_text::circle_words(&p.message);
        Some(p)
    }

    // ---- jobs ----------------------------------------------------------------------------------------------------

    fn launch(&mut self, work: Work) -> bool {
        // (critic 71 minor 3) the job keeps to the account signed in now: a retry after another account signed in is
        // not sent with that account's sign-in
        let Some(session) = self.session.clone().map(super::pool::pinned) else {
            return false;
        };
        let kind = work.kind();
        if kind == JobKind::Act && self.running(JobKind::Act) {
            return false;
        }
        let act = match &work {
            Work::Send { cid, bid, req, .. } => Some(("send", cid, bid, 0, req.base_version.unwrap_or(0))),
            Work::Rollback { cid, bid, to, base, .. } => Some(("restore", cid, bid, *to, *base)),
            Work::Trim { cid, bid, keep } => Some(("trim", cid, bid, u64::from(*keep), 0)),
            _ => None,
        };
        if let Some((what, cid, bid, arg, base)) = act {
            let since = self.now.unwrap_or_else(Instant::now);
            let a = ActFor { what, name: self.act_name(cid, bid), local: self.local_of(cid, bid), cid: cid.clone(), bid: bid.clone(), arg, base, turn: self.turn, account: self.account.clone(), outs: self.outs, since };
            // (critic 71 I1) a new restore, trim or send of the bank: what the last one said is out of date
            let kind = match what {
                "restore" => ResultKind::Restore,
                "trim" => ResultKind::Trim,
                _ => ResultKind::Send,
            };
            let bank = self.result_bank(cid, bid, &a.name);
            self.results.settle(&bank, kind);
            self.act_for = Some(a);
            self.act_waited = false;
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
            self.note_answer(&m);
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
                Msg::BaseRead(cid, bid, n, r) => self.on_base_read(&cid, &bid, n, *r),
                Msg::SendBase(cid, bid, n, r) => self.on_send_base(&cid, &bid, n, *r),
            }
        }
        // A job that ended without an answer (its thread died).
        for k in lost {
            self.jobs.retain(|j| j.kind != k);
            // (critic 87 B1) the quiet read of a base version: asked again later, nothing said
            if k == JobKind::Base {
                if let Some(key) = self.base_asked.take() {
                    let state = self.not_reached_now(&key);
                    self.base_reads.insert(key, state);
                }
                continue;
            }
            let p = Problem { message: "verSHion's server did not answer. Nothing was changed. Try again.".into(), retry: true, retry_at: None, account: false };
            self.fail_kind(k, p);
        }
    }

    fn fail_kind(&mut self, k: JobKind, p: Problem) {
        // (phase D3) The send page's base read was lost on its way: the page stays and Try again reads it again; never
        // the whole bank for a read that may yet work.
        if k == JobKind::SendBase {
            if let EditView::Send(v) = &mut self.view
                && matches!(v.partial, PartialSend::Reading)
            {
                v.partial = PartialSend::ReadFailed(PARTIAL_READ_FAILED.into());
            }
            return;
        }
        // (critic 73 I1) a send whose page was closed under it: said for its bank
        if k == JobKind::Act
            && let Some(a) = self.act_for.clone().filter(|a| a.what == "send")
            && !matches!(&self.view, EditView::Send(v) if v.cid == a.cid && v.bid == a.bid)
        {
            self.inflight = None;
            if self.act_mine() {
                let bank = self.act_bank(&a.cid, &a.bid);
                self.say(bank, ResultKind::Send, not_sent(&p.message));
            }
            return;
        }
        // (critic 69 I1) a restore or trim: on the History showing it (also one kept while a version is looked at),
        // else, the History left, on the bank's strip
        if k == JobKind::Act
            && let Some(a) = self.act_for.clone().filter(|a| a.what != "send")
        {
            // (critic 71 minor 2) an earlier account's: nothing is said to this one
            if !self.mine(&a) {
                return;
            }
            let shown = self.history_shown();
            match self.history_of(&a.cid, &a.bid).filter(|h| shown && (h.restore.is_some() || h.trim.is_some())) {
                Some(h) => {
                    if let Some(r) = &mut h.restore {
                        r.stage = RestoreStage::Failed(p.clone());
                    }
                    if let Some(t) = &mut h.trim {
                        t.stage = TrimStage::Failed(p);
                    }
                }
                None => {
                    if let Some(h) = self.history_of(&a.cid, &a.bid) {
                        h.restore = None;
                        h.trim = None;
                    }
                    let (kind, said) = if a.what == "restore" { (ResultKind::Restore, restore_note(a.arg, Some(&RestoreStage::Failed(p)))) } else { (ResultKind::Trim, trim_note(Some(&TrimStage::Failed(p)))) };
                    if let Some(t) = said {
                        let bank = self.act_bank(&a.cid, &a.bid);
                        self.say(bank, kind, t);
                    }
                }
            }
            return;
        }
        // (critic 75 I1) on its own Send page: what the strip says if the page is closed without the person acting on it
        if k == JobKind::Act
            && let EditView::Send(v) = &self.view
            && self.act_mine()
        {
            let bank = self.act_bank(&v.cid.clone(), &v.bid.clone());
            self.send_note = Some((bank, ResultKind::Send, not_sent(&p.message)));
        }
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
            // (phase D2) the question history runs its own reads: no answer of this model's is for it
            EditView::Question(_) => false,
            EditView::Closed => false,
        }
    }

    // ---- send my changes ------------------------------------------------------------------------------------

    /// "Send my changes": the page where the person adds a note and sends. Returns true when it opened.
    pub fn open_send(&mut self, local_id: &str, perm: Perm) -> bool {
        self.ensure();
        let Some(l) = self.links.iter().find(|l| l.local_bank_id == local_id).cloned() else { return false };
        // (phase D1) A copy that follows the circle (a viewer's, or the viewers' form) never sends; (round 15, critic 91 I2) nor
        // one whose difficulties the circle has not told yet.
        if !l.can_send() || awaits_read(&l) || l.inherit || self.wait_for_act() {
            return false;
        }
        self.leave_view();
        // (critic 89 m2) a quiet read of a base version on its way is kept
        self.jobs.retain(|j| j.kind == JobKind::Base);
        // (critic 78 I1, critic 79 minor 4) after a send with no clear answer, what its page sent: the same changes then
        // go as the same request (a merge it went with is kept for it, even when the page was closed meanwhile)
        let retry = self.retry_send.clone().filter(|r| r.local_id == l.local_bank_id);
        // (phase D3, plan section 6) A selection carried from a conflict's Get latest stays for this bank only; an
        // answer that could not be read is fresher still, and reopening starts from it.
        let carried = self.send_unticked.take().filter(|(bank, _)| bank == local_id);
        self.send_unticked = match &retry {
            Some(r) if !r.unticked.is_empty() => Some((local_id.to_string(), r.unticked.clone())),
            Some(_) => None,
            None => carried,
        };
        if self.carry.is_none()
            && let Some(c) = retry.as_ref().and_then(|r| r.carry.clone())
        {
            self.carry = Some(c);
        }
        let message = retry.as_ref().map(|r| r.message.clone()).unwrap_or_default();
        self.view = EditView::Send(Box::new(SendView {
            local_id: l.local_bank_id.clone(),
            cid: l.circle_id.clone(),
            bid: l.circle_bank_id.clone(),
            title: api::plain(&l.title),
            perm,
            message,
            changes: None,
            stage: SendStage::Editing,
            partial: PartialSend::Reading,
            base_body: None,
            notice: None,
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
        if let (Some(r), EditView::Send(v)) = (retry, &mut self.view) {
            // (`do_send` drops the decisions and pictures again for content they were not made for)
            v.decisions = r.decisions;
            v.decisions_for = r.decisions_for;
            v.picked = r.picked;
            v.base_override = r.base_override;
            v.replaces = r.replaces;
            v.confirm_large = r.confirm_large;
        }
        // The counts are filled in by `sync`. The bank's licence is asked of the pool now: a send
        // goes with the licence the pool has, never with the one this device saw at copy time.
        self.statuses.remove(local_id);
        self.check_licence();
        // (phase D3, plan 5.5) ONE read of the base version's body for this page's summary. A read that can not be
        // made keeps the page with Try again; only a version the circle no longer keeps sends the whole bank.
        self.start_partial_send(&l);
        // (phase D3) A send over a held version goes as the whole bank: composing against one base and sending over
        // another is never done.
        if let EditView::Send(v) = &mut self.view
            && v.base_override.is_some()
        {
            v.partial = PartialSend::Unavailable(SEND_ON_TOP_WHOLE.into());
            v.base_body = None;
        }
        let over_held = matches!(&self.view, EditView::Send(v) if matches!(&v.partial, PartialSend::Unavailable(why) if why.as_str() == SEND_ON_TOP_WHOLE));
        if over_held {
            self.jobs.retain(|j| j.kind != JobKind::SendBase);
        }
        true
    }

    /// (phase D3) Starts the one read of a send page's base version body; the answer is worked out by `resolve_partial`
    /// in `sync`, which has the bank. A read that can not even be asked (not signed in) keeps the page with Try again.
    fn start_partial_send(&mut self, l: &CircleLink) {
        let made = l.base_version != 0 && self.launch(Work::SendBase { cid: l.circle_id.clone(), bid: l.circle_bank_id.clone(), n: l.base_version });
        if !made
            && let EditView::Send(v) = &mut self.view
        {
            v.partial = PartialSend::ReadFailed(PARTIAL_READ_FAILED.into());
        }
    }

    /// (phase D3) "Try again" on a base version that could not be read: the one read runs again.
    pub fn retry_partial(&mut self) {
        let Some(local) = (match &self.view {
            EditView::Send(v) if matches!(v.partial, PartialSend::ReadFailed(_)) => Some(v.local_id.clone()),
            _ => None,
        }) else {
            return;
        };
        let Some(l) = self.links.iter().find(|l| l.local_bank_id == local).cloned() else { return };
        if let EditView::Send(v) = &mut self.view {
            v.partial = PartialSend::Reading;
            v.base_body = None;
            v.notice = None;
        }
        self.start_partial_send(&l);
    }

    /// (phase D3) The base version's body is back: kept for `sync`, which has the bank. Only a version the circle no
    /// longer keeps (or holds back) is the whole-bank path, said plainly (plan 4.7); any other failure keeps the page
    /// with Try again.
    fn on_send_base(&mut self, cid: &str, bid: &str, n: u64, r: Result<BankDoc, PoolError>) {
        let matches_page = matches!(&self.view, EditView::Send(v) if v.cid == cid && v.bid == bid && matches!(v.partial, PartialSend::Reading));
        if !matches_page {
            return;
        }
        let gone = matches!(&r, Err(PoolError::Refused(rf)) if matches!(rf.kind, RefusalKind::VersionNotRetained | RefusalKind::VersionWithheld));
        let state = match r {
            // (only the version asked for: a reply for another version is not this page's base)
            Ok(doc) if doc.bank.version == n => {
                if let EditView::Send(v) = &mut self.view {
                    v.base_body = Some((n, doc.payload));
                }
                return;
            }
            Ok(_) => PartialSend::ReadFailed(PARTIAL_READ_FAILED.into()),
            Err(_) if gone => PartialSend::Unavailable(PARTIAL_UNAVAILABLE.into()),
            Err(e) => {
                // (no borrow of the view while `problem_of` runs: a signed-out answer is noted there)
                let why = self.problem_of(&e).map(|p| p.message).filter(|m| !m.is_empty()).unwrap_or_else(|| PARTIAL_READ_FAILED.into());
                PartialSend::ReadFailed(why)
            }
        };
        if let EditView::Send(v) = &mut self.view
            && matches!(v.partial, PartialSend::Reading)
        {
            v.partial = state;
        }
    }

    fn check_licence(&mut self) {
        let EditView::Send(v) = &mut self.view else { return };
        v.licence = LicenceState::Checking;
        let bid = v.bid.clone();
        if !self.launch(Work::Licence { bid })
            && let EditView::Send(v) = &mut self.view
        {
            v.licence = LicenceState::Failed(Problem { message: "Not signed in. Open the Account window and sign in.".into(), retry: true, retry_at: None, account: true });
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
                        None => LicenceState::Failed(Problem { message: "This bank's licence is not known on this device and verSHion's server did not say. Try again.".into(), retry: true, retry_at: None, account: false }),
                    },
                    _ => LicenceState::Failed(Problem { message: "This bank is no longer linked to the circle.".into(), retry: false, retry_at: None, account: false }),
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

    /// (phase D3) Where the open send page's summary stands (`None`: no send page).
    pub fn partial_send(&self) -> Option<&PartialSend> {
        match &self.view {
            EditView::Send(v) => Some(&v.partial),
            _ => None,
        }
    }

    /// (phase D3) The circle's name for the open send page's heading (`None`: no send page, or the link knows none).
    pub fn send_circle_name(&self) -> Option<&str> {
        let EditView::Send(v) = &self.view else { return None };
        self.links.iter().find(|l| l.local_bank_id == v.local_id).map(|l| l.circle_name.as_str()).filter(|n| !n.is_empty())
    }

    /// (phase D3) The changes the open send page lists (only once its base version's body is read).
    pub fn send_changes(&self) -> Option<&BodyChanges> {
        match self.partial_send()? {
            PartialSend::Ready { changes, .. } => Some(changes),
            _ => None,
        }
    }

    /// (phase D3) The local ids ticked on the open send page, and the base body they were worked out against.
    pub fn send_ticked(&self) -> Option<&BTreeSet<String>> {
        match self.partial_send()? {
            PartialSend::Ready { ticked, .. } => Some(ticked),
            _ => None,
        }
    }

    pub fn send_base_body(&self) -> Option<&[u8]> {
        match self.partial_send()? {
            PartialSend::Ready { body, .. } => Some(body),
            _ => None,
        }
    }

    /// (phase D3, plan 4.7) Tick or untick one change of the open send page. Only a change can be ticked. The page
    /// disables Send while nothing is ticked, so the send's own "Tick at least one change" refusal below is dead
    /// defence for a caller other than the page (it keeps the ticks and the page as they are).
    pub fn toggle_change(&mut self, local: &str, on: bool) {
        if let EditView::Send(v) = &mut self.view
            && !matches!(v.stage, SendStage::Working(_))
            && let PartialSend::Ready { changes, ticked, .. } = &mut v.partial
            && changes.contains(local)
        {
            if on {
                ticked.insert(local.to_string());
            } else {
                ticked.remove(local);
            }
        }
    }

    /// (phase D3) "Tick all" or "Untick all": every change on the open send page.
    pub fn tick_all(&mut self, on: bool) {
        if let EditView::Send(v) = &mut self.view
            && !matches!(v.stage, SendStage::Working(_))
            && let PartialSend::Ready { changes, ticked, .. } = &mut v.partial
        {
            if on {
                *ticked = changes.ids();
            } else {
                ticked.clear();
            }
        }
    }

    /// Why the Send button can not be pressed, or `None`.
    pub fn send_blocker(&self) -> Option<&'static str> {
        let EditView::Send(v) = &self.view else { return Some("Nothing to send.") };
        if !v.perm.can_send() {
            return Some(if v.perm.closed { "This circle is closed." } else { "You can only look at this circle's banks. The owner can let you add banks by changing your role to editor." });
        }
        if self.links.iter().find(|l| l.local_bank_id == v.local_id).is_some_and(|l| !l.can_send()) {
            return Some(FOLLOW_NO_SEND);
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
        // (phase D3) the base version's body is on its way, or the read failed for a reason that may pass: the summary
        // decides what is sent, so the page waits (Try again for the failed read)
        if matches!(v.partial, PartialSend::Reading) {
            return Some(PARTIAL_READING);
        }
        if matches!(v.partial, PartialSend::ReadFailed(_)) {
            return Some(PARTIAL_READ_FAILED);
        }
        if self.copy_is_stale(&v.local_id) {
            return Some("This bank no longer matches what it last had from the circle (a safety copy of your banks may have been put back). Get the latest first.");
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
        // (phase D3, plan 4.7) The page disables Send while nothing is ticked and says "Tick at least one change".
        // This refusal is dead defence for a caller other than the page: the ticks are kept and nothing is sent.
        if let EditView::Send(v) = &self.view
            && let PartialSend::Ready { ticked, .. } = &v.partial
            && ticked.is_empty()
        {
            if let EditView::Send(v) = &mut self.view {
                v.stage = SendStage::Refused { message: TICK_ONE.into(), merge_again: false };
            }
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
            // (phase D3) the selection stays in `partial`; a carry meant for a page rebuilt after Get latest is not
            // needed once the person is back on this page's own list.
            self.send_unticked = None;
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
        // (phase D3) This send goes over a held version: the whole bank, never a selection composed against another
        // base. The reason stays on the page for the tick boxes to show.
        self.jobs.retain(|j| j.kind != JobKind::SendBase);
        if let EditView::Send(v) = &mut self.view {
            v.base_override = Some(head_version);
            v.replaces = replaces;
            v.partial = PartialSend::Unavailable(SEND_ON_TOP_WHOLE.into());
            v.base_body = None;
        }
    }

    fn do_send(&mut self, banks: &[QuestionBank]) {
        // (critic 75 I1) the person sends again: what the page said of the last answer is out of date
        self.send_note = None;
        let hiding = match &self.view {
            EditView::Send(v) => self.hides_keys(&v.bid),
            _ => return,
        };
        let EditView::Send(v) = &mut self.view else { return };
        let Some(link) = self.links.iter().find(|l| l.local_bank_id == v.local_id).cloned() else {
            v.stage = SendStage::Failed(Problem { message: "This bank is no longer linked to the circle.".into(), retry: false, retry_at: None, account: false });
            return;
        };
        let fail = |v: &mut SendView, m: &str| v.stage = SendStage::Failed(Problem { message: m.into(), retry: false, retry_at: None, account: false });
        if !v.perm.can_send() {
            fail(v, "Your role in this circle does not allow sending a version.");
            return;
        }
        if !link.can_send() {
            fail(v, FOLLOW_NO_SEND);
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
            fail(v, "This bank no longer matches what it last had from the circle. Get the latest first. Nothing was sent.");
            return;
        }
        // (phase D3) A partial send composes its payload from the bank and the base version's body; a page whose base
        // can not be had sends the whole bank exactly as before. Nothing here falls back silently.
        match &v.partial {
            PartialSend::Reading => {
                fail(v, "Checking the version your copy is based on. Try again in a moment. Nothing was sent.");
                return;
            }
            // A read that may yet work is never the whole-bank path: the page stays with Try again.
            PartialSend::ReadFailed(why) => {
                let why = why.clone();
                fail(v, &why);
                return;
            }
            _ => {}
        }
        // (phase D3) The summary is a snapshot taken when the body arrived. A bank that changed since is worked out
        // again against the SAME base body: nothing is sent on that press, the ticks stay by local id, and a change the
        // person has not seen yet starts ticked (plan 4.7).
        if let PartialSend::Ready { base, body, print, changes, ticked, .. } = &v.partial
            && bank_print(bank) != *print
        {
            let (base, body, old_changes, old_ticked) = (*base, body.clone(), changes.clone(), ticked.clone());
            match changes_against_body(&link, bank, &body) {
                Ok(fresh) => {
                    // (an id no longer a change is simply not in the new list; a new change starts ticked)
                    let ticked = fresh.ids().into_iter().filter(|id| if old_changes.contains(id) { old_ticked.contains(id) } else { true }).collect();
                    let rows = send_rows(&link, bank, &fresh);
                    v.partial = PartialSend::Ready { base, body: body.clone(), print: bank_print(bank), changes: fresh, ticked, rows };
                    v.stage = SendStage::Editing;
                    v.notice = Some(SEND_LIST_AGAIN.into());
                    // (the large-change confirmation was for the old payload)
                    v.confirm_large = false;
                    self.last_act = None;
                    return;
                }
                Err(m) => {
                    fail(v, &m);
                    return;
                }
            }
        }
        // (phase D3) A send over a held version goes as the whole bank: never compose against one base and send over
        // another. `send_on_top` says so on the page.
        if v.base_override.is_some()
            && matches!(v.partial, PartialSend::Ready { .. })
        {
            v.partial = PartialSend::Unavailable(SEND_ON_TOP_WHOLE.into());
        }
        // (the list is the bank's now: an earlier "the list again" line is out of date)
        v.notice = None;
        let plan = match &v.partial {
            PartialSend::Ready { base, body, changes, ticked, .. } if v.base_override.is_none() => Some((*base, body.clone(), changes.changed.clone(), ticked.clone())),
            _ => None,
        };
        // (phase D3) The compare-and-set goes over the version the body was read at, not whatever the link says now.
        let plan_base = plan.as_ref().map(|(base, ..)| *base);
        let mut composed: Option<ComposedPartial> = None;
        let mut held_back: Vec<String> = Vec::new();
        // (phase D3, plan section 6 "Pictures") The base's picture files for questions sent as the base has it; the
        // send job fetches any this computer lacks. Only those questions are carried: a picture on the person's own
        // question keeps today's MissingMedia refusal.
        let mut fetch: Vec<String> = Vec::new();
        let payload = match plan {
            Some((_base, body, changed, ticked)) => {
                if ticked.is_empty() {
                    v.stage = SendStage::Refused { message: TICK_ONE.into(), merge_again: false };
                    return;
                }
                match compose_partial_parts(&link, bank, &body, &changed, &ticked) {
                    Ok(c) => {
                        // (plan section 6) The unticked changes still differing: what the link keeps in `held_back`.
                        if let PartialSend::Ready { changes, .. } = &v.partial {
                            held_back = changes.ids().into_iter().filter(|id| !ticked.contains(id)).collect();
                        }
                        for cq in &c.questions {
                            if let Some(file) = cq.base_image.as_ref()
                                && !fetch.contains(file)
                            {
                                fetch.push(file.clone());
                            }
                        }
                        let payload = c.payload.clone();
                        composed = Some(c);
                        payload
                    }
                    Err(m) => {
                        // Never silently the whole bank: the reason stays on the page and nothing is sent.
                        fail(v, &m);
                        return;
                    }
                }
            }
            None => match payload_of(&link, bank) {
                Ok(p) => p,
                Err(m) => {
                    fail(v, &m);
                    return;
                }
            },
        };
        // (plan section 6) The checks run on the composed version: a change the person unticked goes as the base had
        // it, so a key the bank lacks there is no key the circle is given. The refusals name the bank's question
        // numbers, not the payload's (a restored removal would shift them). The whole-bank path is untouched.
        let keyless: Vec<(usize, String)> = match &composed {
            Some(c) => c
                .questions
                .iter()
                .enumerate()
                .filter_map(|(i, cq)| {
                    let q = cq.question.as_ref().filter(|q| crate::model::is_keyless(q))?;
                    let at = bank.questions.iter().position(|bq| bq.id == cq.local).unwrap_or(i);
                    Some((at, kind_tag(&q.kind).to_string()))
                })
                .collect(),
            None => bank.questions.iter().enumerate().filter(|(_, q)| crate::model::is_keyless(q)).map(|(i, q)| (i, kind_tag(&q.kind).to_string())).collect(),
        };
        if !keyless.is_empty() {
            v.stage = SendStage::Keyless { positions: keyless.iter().map(|k| k.0).collect(), kinds: keyless.into_iter().map(|k| k.1).collect() };
            return;
        }
        let base = v.base_override.or(plan_base).unwrap_or(link.base_version);
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
        // (phase D3) The picture rows are named by the bank's question numbers on screen; `image_ok` stays in payload
        // positions (what the pool reads), so a restored removal shifts only what the person sees.
        let pictures = Pictures::of_payload(&payload).map(|mut p| {
            if let Some(c) = &composed {
                p.set_bank_map(bank_positions(&c.questions, bank));
            }
            p
        });
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
        let perm = v.perm;
        let carry = self.carry.clone().filter(|c| c.local_id == v.local_id && c.head == base);
        // (phase D3) What this selection leaves unticked: kept with the retry record and across a conflict's Get latest.
        let unticked: BTreeSet<String> = match (&v.partial, plan_base) {
            (PartialSend::Ready { changes, ticked, .. }, Some(_)) => changes.ids().difference(ticked).cloned().collect(),
            _ => BTreeSet::new(),
        };
        let retry = RetrySend {
            local_id: v.local_id.clone(),
            message: v.message.trim().to_string(),
            decisions: v.decisions.clone(),
            decisions_for: v.decisions_for.clone(),
            picked: v.picked.clone(),
            base_override: v.base_override,
            replaces: v.replaces.clone(),
            confirm_large: v.confirm_large,
            carry,
            unticked,
        };
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
        if self.launch(Work::Send { cid, bid, req: Box::new(req), fetch }) {
            self.inflight = Some(Inflight { link, bank, pictures: shown, image_ok, dropped, asked, perm, retry, base, partial: composed, held_back });
        } else if let EditView::Send(v) = &mut self.view {
            v.stage = SendStage::Failed(Problem { message: "Not signed in. Open the Account window and sign in.".into(), retry: true, retry_at: None, account: true });
        }
    }

    fn on_sent(&mut self, cid: &str, bid: &str, r: Result<SentBank, PoolError>, eff: Option<License>) {
        let mut inflight = self.inflight.take();
        if let (Some(l), Some(i)) = (eff, inflight.as_mut())
            && i.link.license != l.as_str()
        {
            // The licence changed while the page was open: the send went with the pool's, and the link follows.
            // (critic 75 minor 4) the link as it is now: what a check learnt meanwhile stays
            i.link.license = l.as_str().to_string();
            let lic = i.link.license.clone();
            self.update_link(&i.link.local_bank_id, |x| {
                x.license = lic;
                true
            });
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
        // (critic 73 I1) a conflict names the circle's newer version: the link learns it, whether or not the page is
        // still open, so the strip never names the old version as the latest. (critic 75 minor 4) Written into the link
        // as it is now, not as it was when the send started: what a check learnt meanwhile stays.
        if let (Err(PoolError::Refused(rf)), Some(i)) = (&r, &inflight)
            && let RefusalKind::CircleConflict { head_version: Some(h), head_at, head_by } = &rf.kind
        {
            let local = i.link.local_bank_id.clone();
            let mut moved = false;
            self.update_link(&local, |l| {
                moved = *h > l.head_seen;
                if moved {
                    l.head_seen = *h;
                    l.head_seen_at = *head_at;
                    l.head_seen_by = head_by.clone();
                }
                moved
            });
            if moved {
                self.head_moved(&local);
            }
            // (critic 80 I2) a colleague's version right after the one an earlier send with no clear answer was based on:
            // that send did not go through (the app says so from now on, and never "it may be your earlier send")
            if let Some(l) = self.links.iter().find(|l| l.local_bank_id == local).cloned()
                && let Some(base) = self.pending_send_base(&l)
            {
                match self.found(*h, base, head_by.as_deref()) {
                    Found::OtherFirst(_) => self.update_link(&local, |l| l.sent_record.as_mut().is_some_and(|r| !std::mem::replace(&mut r.not_through, true))),
                    // (critic 86 minor) the person's own version right after it: that send went, as when a check finds it,
                    // so what it carried is never counted again as unsent
                    Found::Mine => self.update_link(&local, went_through),
                    Found::Still | Found::Maybe => {}
                }
            }
        }
        // (critic 75 minor 3) An answer that does not say whether the send went through (none came, it could not be
        // read, the server failed on its side): never "not sent"; and (critic 76 I3) a send refused because the person
        // can only view now. Where the circle bank stands is checked at once; until then its newest version is "when
        // last checked".
        let unclear = r.as_ref().err().and_then(unclear_why);
        // (critic 84 I1) The circle answered this send: what the strip says of it is that answer now, never what a later
        // check would say of an earlier send's note.
        if matches!(&r, Ok(_) | Err(PoolError::Refused(_))) {
            self.unclear_noted.remove(&(cid.to_string(), bid.to_string()));
        }
        let role_left = matches!(&r, Err(PoolError::Refused(rf)) if matches!(rf.kind, RefusalKind::CircleRoleRequired { .. }));
        if unclear.is_some() || role_left {
            if let Some(l) = self.local_of(cid, bid) {
                self.check_asks.push(l);
            }
            if unclear.is_some() {
                self.unsure.insert((cid.to_string(), bid.to_string()));
                // (critic 78 I1, I2) it may have gone through: the check's answer replaces the note, a conflict may be
                // with this send, and the next Send page tries the same request again; (critic 79 I2, critic 80 I1)
                // what it carried and the version it was based on are kept with the link (also across a restart)
                if let Some(i) = &inflight {
                    self.unclear_noted.insert((cid.to_string(), bid.to_string()), i.base);
                    self.retry_send = Some(i.retry.clone());
                    self.keep_record(i);
                }
            }
        }
        // (critic 73 I1) The person left (the page closed under the send: the Shared page opened, another bank's
        // "Circle page", a sign-out): a send that went through still moves the link on, and how it went is said for
        // its bank. (critic 75 I1) Still open, the answer goes on the page, and what the strip would say of it is kept
        // for when the page is closed without the person acting on it.
        let here = self.here(cid, bid) && matches!(self.view, EditView::Send(_));
        if here {
            self.send_note = None;
        }
        let can_fix = inflight.as_ref().is_some_and(|i| i.perm.can_rewind());
        match r {
            Ok(sent) => {
                let merged_from = sent.merged_from;
                if let Some(i) = &inflight {
                    self.finish_send(i, &sent);
                    // (phase D3) An answer with no version (the send went through, but what it made is not known)
                    // keeps the selection too: the next page starts from the same request, ticks included.
                    if sent.version == 0 {
                        self.retry_send = Some(i.retry.clone());
                    }
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
                let done = Sent { version: sent.version, added: sent.added, changed: sent.changed, removed: sent.removed, replayed: sent.replayed, held: sent.held, withheld: sent.withheld, merged_from, left_out_unasked: left_out, not_asked, hiding_stopped, ask_next };
                // What the "Sent" box says beyond the counts (held for review, pictures left out), or that the answer
                // could not be read (critic 74 I1); nothing for a send that simply went through. (critic 71 minor 2)
                // Never said to an account that signed in after it was sent.
                let note = if sent.version == 0 {
                    Some(SENT_UNREAD.to_string())
                } else {
                    let rest: Vec<String> = sent_lines(&Sent { replayed: false, merged_from: None, ..done.clone() }, can_fix).into_iter().skip(1).collect();
                    (!rest.is_empty()).then(|| format!("Version {} was sent. {}", sent.version, rest.join(" ")))
                };
                let note = note.filter(|_| self.act_mine());
                if here {
                    if let EditView::Send(v) = &mut self.view {
                        v.stage = SendStage::Done(Box::new(done));
                    }
                    let bank = self.act_bank(cid, bid);
                    self.send_note = note.map(|t| (bank, ResultKind::Send, t));
                } else if let Some(t) = note {
                    let bank = self.act_bank(cid, bid);
                    self.say(bank, ResultKind::Send, t);
                }
            }
            Err(e) => {
                let stage = match (&e, &unclear) {
                    (_, Some(why)) => Some(SendStage::Failed(Problem { message: unclear_page(why), retry: true, retry_at: None, account: false })),
                    (PoolError::Refused(r), None) => match &r.kind {
                        RefusalKind::CircleConflict { head_version, head_at, head_by } => Some(SendStage::Conflict(ConflictInfo { head_version: *head_version, head_at: *head_at, head_by: head_by.clone() })),
                        RefusalKind::ConfirmLarge { removed, total } => Some(SendStage::Large { removed: *removed, total: *total }),
                        // A bank that hides its answer keys: the pool asks how the viewers' copy is made.
                        k if decision_refusal(k).is_some() => {
                            self.pending_send = None;
                            let carried = if let EditView::Send(v) = &self.view { v.decisions.clone() } else { StripDecisions::default() };
                            // (phase D3) The page names the bank's questions; the refusal and the decisions stay in
                            // payload positions (what the pool reads).
                            let display = inflight.as_ref().and_then(|i| i.partial.as_ref().map(|c| bank_positions(&c.questions, &i.bank)));
                            decision_refusal(k).map(|p| {
                                let mut d = Decisions::new(p.clone(), carried);
                                if let Some(map) = display.clone() {
                                    d.set_bank_map(map);
                                }
                                SendStage::Decide(Box::new(d))
                            })
                        }
                        RefusalKind::InvalidPayload(p) | RefusalKind::LabelsNeedDecision(p) | RefusalKind::OrderAckRequired(p) => {
                            self.pending_send = None;
                            // (phase D3) a payload refusal's numbers are shown as the bank's; nothing is decided from it
                            let p = match inflight.as_ref().and_then(|i| i.partial.as_ref().map(|c| bank_positions(&c.questions, &i.bank))) {
                                Some(map) => Box::new(refusal_for_display((**p).clone(), &map)),
                                None => p.clone(),
                            };
                            Some(SendStage::Payload(p))
                        }
                        RefusalKind::KeylessQuestion { positions, kinds } => {
                            self.pending_send = None;
                            // (phase D3) A partial send's payload positions are not the bank's question numbers.
                            let positions = match inflight.as_ref().and_then(|i| i.partial.as_ref().map(|c| (c, i))) {
                                Some((c, i)) => bank_numbers(positions, &bank_positions(&c.questions, &i.bank)),
                                None => positions.clone(),
                            };
                            Some(SendStage::Keyless { positions, kinds: kinds.clone() })
                        }
                        RefusalKind::HideKeyChanged { hide_key, .. } => {
                            // Nothing was stored; the same send goes again (with decisions if the pool asks for them,
                            // and the picture step first when the keys are hidden now).
                            if let Some(h) = hide_key {
                                self.note_hiding(bid, *h);
                            }
                            Some(SendStage::Refused { message: super::circles_text::circle_words(&e.message()), merge_again: false })
                        }
                        RefusalKind::AttestationRequired => {
                            Some(SendStage::Failed(Problem { message: super::circles_text::circle_words(&e.message()), retry: false, retry_at: None, account: false }))
                        }
                        RefusalKind::LicenseAuthorOnly { license } => {
                            if let (Some(l), Some(i)) = (license.as_deref().filter(|l| super::pool::License::parse(l).is_some()), &inflight) {
                                // (critic 75 minor 4) into the link as it is now
                                let lic = l.to_string();
                                self.update_link(&i.link.local_bank_id, |x| {
                                    x.license = lic;
                                    true
                                });
                                if let (EditView::Send(v), Some(known)) = (&mut self.view, super::pool::License::parse(l)) {
                                    v.licence = LicenceState::Known(known);
                                }
                            }
                            Some(SendStage::Refused { message: "Only the bank's author chooses its licence. Your changes will go with the bank's own licence. Send again.".into(), merge_again: false })
                        }
                        RefusalKind::MergeTokenInvalid => {
                            self.carry = None;
                            Some(SendStage::Refused { message: "What you got is out of date. Get latest again, then send.".into(), merge_again: true })
                        }
                        RefusalKind::CommitIdConflict => {
                            self.pending_send = None;
                            Some(SendStage::Failed(Problem { message: "This send was already used for different content. Press Send again to start it again.".into(), retry: true, retry_at: None, account: false }))
                        }
                        RefusalKind::BankRemoved => Some(SendStage::Failed(Problem { message: "This bank was removed from the circle, so nothing can be sent to it. The owner or the bank's author may be able to restore it.".into(), retry: false, retry_at: None, account: false })),
                        // (critic 76 I3) the person can only view now: the send, not adding banks
                        RefusalKind::CircleRoleRequired { needed } if needed.as_deref().is_none_or(|n| n == "editor") => Some(SendStage::Failed(Problem { message: SEND_ROLE_LEFT.into(), retry: false, retry_at: None, account: false })),
                        _ => None,
                    },
                    _ => None,
                };
                // (phase D3, plan section 6) A conflict sends nothing; the unticked marks survive Get latest and the
                // merge, by local id, and the list rebuilt from the new base starts them unticked.
                if matches!(&stage, Some(SendStage::Conflict(_)))
                    && let Some(i) = &inflight
                {
                    self.send_unticked = (!i.retry.unticked.is_empty()).then(|| (i.link.local_bank_id.clone(), i.retry.unticked.clone()));
                }
                if !here {
                    self.say_send_left(cid, bid, &e, stage);
                    return;
                }
                // (critic 73 minor 3) the session that ended was an earlier one: the account is signed in again
                let (stage, earlier) = match stage {
                    Some(s) => (Some(s), false),
                    None if self.ended_earlier(&e) => (Some(SendStage::Failed(Problem { message: EARLIER_SESSION.into(), retry: true, retry_at: None, account: false })), true),
                    None => (self.problem_of(&e).map(SendStage::Failed), false),
                };
                let Some(stage) = stage else {
                    // Signed out: back to the note, the commit id is kept for the retry; said above the page.
                    if let EditView::Send(v) = &mut self.view {
                        v.stage = SendStage::Editing;
                        let (cid, bid, title) = (v.cid.clone(), v.bid.clone(), v.title.clone());
                        let bank = self.result_bank(&cid, &bid, &title);
                        self.say(bank, ResultKind::Session, "Your session ended. Sign in again; where you were is kept, and nothing was lost.");
                    }
                    return;
                };
                let note = if !self.act_mine() {
                    None
                } else if earlier {
                    Some(LEFT_EARLIER_SESSION.to_string())
                } else {
                    let bank = self.act_bank(cid, bid);
                    self.stage_words(&bank, &stage, unclear.as_deref())
                };
                let unticked = matches!(&e, PoolError::Refused(r) if matches!(r.kind, RefusalKind::AttestationRequired));
                if let EditView::Send(v) = &mut self.view {
                    v.stage = stage;
                    if unticked {
                        v.attest = false;
                    }
                }
                let bank = self.act_bank(cid, bid);
                self.send_note = note.map(|t| (bank, ResultKind::Send, t));
            }
        }
    }

    /// (critic 73 I1, critic 75 I1) What the bank's strip says of a send's answer the person did not see on its page:
    /// the failure's words, the newer version a conflict named (the strip then offers Get latest; a conflict that named
    /// none has the bank checked at once), or that Send has something to ask. `None` for a stage that is no answer.
    fn stage_words(&mut self, bank: &ResultBank, stage: &SendStage, unclear: Option<&str>) -> Option<String> {
        if let Some(why) = unclear {
            return Some(unclear_strip(why));
        }
        // (critic 78 I2) the circle is ahead because of the person's own earlier send, or may be; (critic 80 I2) never when
        // the newer version is a colleague's
        let earlier = match (stage, bank.local.as_deref()) {
            (SendStage::Conflict(c), Some(l)) => self.conflict_own(l, c),
            _ => None,
        };
        Some(match stage {
            SendStage::Conflict(c) if earlier.is_some() => {
                if c.head_version.is_none()
                    && let Some(l) = &bank.local
                {
                    self.check_asks.push(l.clone());
                }
                own_conflict_strip(earlier == Some(true), c.head_version)
            }
            SendStage::Conflict(c) => match c.head_version {
                Some(h) => format!("Your changes were not sent: someone sent version {h} first. Get the latest, then send them."),
                None => {
                    if let Some(l) = &bank.local {
                        self.check_asks.push(l.clone());
                    }
                    "Your changes were not sent: someone sent a newer version first. Get the latest, then send them.".to_string()
                }
            },
            SendStage::Large { removed, total } => format!("Your changes were not sent: they remove {removed} of the bank's {total} questions. Press Send my changes to send them anyway."),
            SendStage::Decide(_) | SendStage::Payload(_) | SendStage::Keyless { .. } | SendStage::Pictures(_) => "Your changes were not sent: the circle needs a few choices or changes first. Press Send my changes to see them.".to_string(),
            SendStage::Refused { message, .. } => not_sent(message),
            SendStage::Failed(p) => not_sent(&p.message),
            SendStage::Editing | SendStage::Working(_) | SendStage::Done(_) => return None,
        })
    }

    /// (critic 73 I1) How a send that did not go through went, said for its bank when its page was closed under it.
    /// Nothing is said to an account that signed in after the send was asked for.
    fn say_send_left(&mut self, cid: &str, bid: &str, e: &PoolError, stage: Option<SendStage>) {
        if !self.act_mine() {
            return;
        }
        let bank = self.act_bank(cid, bid);
        let unclear = unclear_why(e);
        let text = match stage {
            Some(s) => match self.stage_words(&bank, &s, unclear.as_deref()) {
                Some(t) => t,
                None => return,
            },
            // (critic 73 minor 3) the session that ended was an earlier one: the account is signed in again
            None if self.ended_earlier(e) => LEFT_EARLIER_SESSION.to_string(),
            None => match self.problem_of(e) {
                Some(p) => not_sent(&p.message),
                None => {
                    // (the strip offers "Sign in...": `problem_of` noted the session ended)
                    self.say(bank, ResultKind::Session, "Your session ended before the send was answered. Sign in, then send your changes again; they are still on this computer.");
                    return;
                }
            },
        };
        self.say(bank, ResultKind::Send, text);
    }

    /// (critic 82 I1) The circle answered for a bank (a send, Get latest, a merge, its History, a restore or trim, its
    /// licence): it can be reached, whatever an earlier check met.
    fn note_answer(&mut self, m: &Msg) {
        fn answer<T>(r: &Result<T, PoolError>) -> bool {
            matches!(r, Ok(_) | Err(PoolError::Refused(_)))
        }
        let bank = match m {
            Msg::Sent(c, b, r, _) | Msg::Rolled(c, b, _, r) => answer(r).then_some((c, b)),
            Msg::Fetched(c, b, r) => answer(r).then_some((c, b)),
            Msg::Applied(c, b, r) => answer(r).then_some((c, b)),
            Msg::Versions(c, b, _, r) => answer(r).then_some((c, b)),
            Msg::Version(c, b, _, r) | Msg::BaseRead(c, b, _, r) | Msg::SendBase(c, b, _, r) => answer(r).then_some((c, b)),
            Msg::Trimmed(c, b, r) => answer(r).then_some((c, b)),
            Msg::Licence(b, r) => {
                if answer(r) {
                    let local = self.links.iter().find(|l| l.circle_bank_id == *b).map(|l| l.local_bank_id.clone());
                    if let Some(l) = local.filter(|l| !self.answered.contains(l)) {
                        self.answered.push(l);
                    }
                }
                None
            }
            Msg::Progress(_) => None,
        };
        if let Some(l) = bank.and_then(|(c, b)| self.local_of(c, b)).filter(|l| !self.answered.contains(l)) {
            self.answered.push(l);
        }
    }

    /// (critic 82 I1) Banks the circle answered for since this was last asked: a strip that could not check the circle
    /// does not go on saying so (its page checks again at once).
    pub fn take_answered(&mut self) -> Vec<String> {
        std::mem::take(&mut self.answered)
    }

    /// (critic 73 I1) Banks whose strip checks the circle at once (the next frame of its page): a conflict that did not
    /// name the newer version, (critic 75 minor 3) a send with no clear answer, (critic 76 I3) a send refused because
    /// the person can only view now, (critic 76 I2) a restore with no readable answer.
    pub fn take_check_asks(&mut self) -> Vec<String> {
        std::mem::take(&mut self.check_asks)
    }

    /// (critic 79 I2, critic 80 I1) A send with no readable outcome: what it carried and the version it was based on are
    /// kept with the link, for the account that sent it (the same rule as `sent_unread`).
    fn keep_record(&mut self, i: &Inflight) {
        let sender = self.act_for.as_ref().and_then(|a| a.account.clone());
        if sender.is_none() && !self.act_mine() {
            return;
        }
        let by = super::bank_links::account_tag(sender.as_deref().or(self.account.as_deref())).unwrap_or_default();
        let record = sent_record(&i.link, &i.bank, i.base, by);
        self.update_link(&i.link.local_bank_id, |l| {
            l.sent_record = Some(record);
            true
        });
    }

    fn finish_send(&mut self, i: &Inflight, sent: &SentBank) {
        // (phase D3) An answer with no version is not a clear answer: the same selection tried again is the same
        // request (the pool replays it, never a second version), so the commit id stays. A readable answer ends it.
        if sent.version != 0 {
            self.pending_send = None;
        }
        self.carry = None;
        // (phase D3) the send is done: no selection waits for another page
        self.send_unticked = None;
        // (critic 78 I1) the send's answer came: it went (read, or `sent_unread` below)
        let key = (i.link.circle_id.clone(), i.link.circle_bank_id.clone());
        self.unclear_noted.remove(&key);
        if self.retry_send.as_ref().is_some_and(|r| r.local_id == i.link.local_bank_id) {
            self.retry_send = None;
        }
        if sent.version == 0 {
            // (critic 74 I1) the changes went: never counted as unsent, and the strip offers Get latest (for the account
            // that sent them); (critic 75 minor 5) kept in the links file, with (critic 76 I1) the bank as it was sent,
            // so that changes made after it are not taken as sent. (critic 77 minor 4) Kept for the account that sent it
            // whoever is signed in now: it is said to that account only. One whose sender is not known is kept only
            // while that account's turn lasts.
            let sender = self.act_for.as_ref().and_then(|a| a.account.clone());
            if sender.is_some() || self.act_mine() {
                let by = super::bank_links::account_tag(sender.as_deref().or(self.account.as_deref())).unwrap_or_default();
                let modified = i.bank.modified;
                self.update_link(&i.link.local_bank_id, |l| {
                    l.sent_unread = Some(SentUnread { by, modified });
                    true
                });
                // (critic 80 I1) and what it carried, so Get latest matches the circle's questions to it
                self.keep_record(i);
            }
            return;
        }
        // (critic 75 minor 4) the link as it is now (a check may have learnt something meanwhile) moves on
        let now = self.links.iter().find(|l| l.local_bank_id == i.link.local_bank_id).cloned().unwrap_or_else(|| i.link.clone());
        // (phase D3, plan section 6) A partial send writes the composed version as the base: `bank` is the bank as it
        // was when the send started, never the composed payload, so an edit made while it was in flight is newer and
        // shows as unsent. A whole-bank send is unchanged. The links file is written, never the banks file.
        let mut l = match &i.partial {
            Some(c) => after_partial_send(&now, &i.bank, &c.questions, sent, i.held_back.clone()),
            None => after_send(&now, &i.bank, sent),
        };
        // (critic 74 I1) a send whose answer was read: the copy is at the version it made; (critic 80 I1) an earlier send
        // with no readable outcome is settled too (this one went on top of it, or was it)
        l.sent_unread = None;
        l.sent_record = None;
        self.advanced.push(Advanced { circle_bank_id: l.circle_bank_id.clone(), local_bank_id: l.local_bank_id.clone(), version: l.base_version });
        self.statuses.remove(&l.local_bank_id);
        self.replace_link(l);
    }

    // ---- get latest and merge --------------------------------------------------------------------------------

    /// "Get latest". Returns true when the page opened.
    pub fn open_merge(&mut self, local_id: &str, perm: Perm) -> bool {
        self.ensure();
        let Some(l) = self.links.iter().find(|l| l.local_bank_id == local_id).cloned() else { return false };
        if self.wait_for_act() {
            return false;
        }
        // (critic 75 I1) Get latest pressed on the Send page's own answer (a conflict, a merge out of date): acted on
        if matches!(&self.view, EditView::Send(v) if v.local_id == local_id) {
            self.send_note = None;
        }
        self.leave_view();
        // (critic 89 m2) a quiet read of a base version on its way is kept: when it is this bank's, the review waits for it
        // and the base version is not read a second time beside it
        self.jobs.retain(|j| j.kind == JobKind::Base);
        self.view = EditView::Merge(Box::new(MergeView { local_id: l.local_bank_id.clone(), cid: l.circle_id.clone(), bid: l.circle_bank_id.clone(), title: api::plain(&l.title), perm, phase: MergePhase::Fetching, as_is: false }));
        self.rebase_next = false;
        // (critic 87 B1) also read for a base whose difficulties are to be learnt: the review learns them from it
        let key = (l.circle_id.clone(), l.circle_bank_id.clone(), l.base_version);
        // (critic 89 m4) not one the circle would not show a moment ago (a reply that can't be read, a version no longer kept)
        let refused = matches!(self.base_reads.get(&key), Some(BaseRead::Unreadable(..) | BaseRead::Limited(..))) && !self.base_read_due(&l, self.now.unwrap_or_else(Instant::now));
        let learn = needs_difficulty_read(&l) && !self.quiet_read_of(&key) && !refused;
        self.merge_base = learn.then_some(key);
        let need_base = l.base.is_empty() || learn;
        // (phase D1) A copy that can not send gets the latest without merge-initiate, which is for the owner and editors.
        let w = if perm.can_send() && l.can_send() {
            // (critic 83 I2) a send with no readable answer that may have gone through: its own version may be needed
            let own = self.record_mine(&l).filter(|r| !r.not_through && !r.questions.is_empty()).map(|r| OwnRead { base: r.base, me: self.me_name.clone() });
            Work::MergeFetch { cid: l.circle_id, bid: l.circle_bank_id, base: l.base_version, need_base, own }
        } else {
            Work::FollowFetch { cid: l.circle_id, bid: l.circle_bank_id, base: l.base_version, need_base }
        };
        if !self.launch(w)
            && let EditView::Merge(m) = &mut self.view
        {
            m.phase = MergePhase::Failed(Problem { message: "Not signed in. Open the Account window and sign in.".into(), retry: true, retry_at: None, account: true });
        }
        true
    }

    /// "Treat as in step..." on a bank's page (phase D1): Get latest, and when the circle has nothing newer the
    /// question "treat this copy as in step?" is asked at once. When it has something newer, the merge review
    /// comes instead (it puts the copy in step itself). Returns true when the page opened.
    pub fn open_merge_to_rebase(&mut self, local_id: &str, perm: Perm) -> bool {
        // (round 17, critic 96 I2 and minor 1) A copy whose version can't be read any more (not one put back from a saved
        // copy): the question is asked at once, nothing needs reading for it (also while the circle's newest version is held
        // back for review). Whatever is newer in the circle is met by the next send, as for any copy.
        if perm.can_send() && self.cant_be_read(local_id) && !self.copy_is_stale(local_id) {
            let Some(l) = self.links.iter().find(|l| l.local_bank_id == local_id).cloned() else { return false };
            if self.wait_for_act() {
                return false;
            }
            self.leave_view();
            self.jobs.retain(|j| j.kind == JobKind::Base);
            self.rebase_next = false;
            self.view = EditView::Merge(Box::new(MergeView { local_id: l.local_bank_id, cid: l.circle_id, bid: l.circle_bank_id, title: api::plain(&l.title), perm, phase: MergePhase::StaleUpToDate { ask: Some(StaleAsk::Rebase) }, as_is: true }));
            return true;
        }
        let opened = self.open_merge(local_id, perm);
        self.rebase_next = opened && perm.can_send();
        if let EditView::Merge(m) = &mut self.view {
            m.as_is = opened;
        }
        opened
    }

    /// (round 17, critic 96 I2) The open page is "Use my bank as it is": asked (its question on screen), or opened for it and
    /// still reading. Its title says so.
    pub fn asking_as_is(&self) -> bool {
        match &self.view {
            EditView::Merge(m) => matches!(m.phase, MergePhase::StaleUpToDate { ask: Some(StaleAsk::Rebase) }) || (m.as_is && matches!(m.phase, MergePhase::Fetching | MergePhase::Ready(_) | MergePhase::UpToDate)),
            _ => false,
        }
    }

    /// (round 17, critic 96 I2) The open page was opened by "Use my bank as it is..." (its words after the confirmation).
    pub fn opened_as_is(&self) -> bool {
        matches!(&self.view, EditView::Merge(m) if m.as_is)
    }

    /// (round 17, critic 96 I2) The copy's difficulties were never read from the circle (an earlier build linked it): "Use my
    /// bank as it is" also overwrites the difficulties colleagues gave there.
    pub fn never_read(&self, local_id: &str) -> bool {
        self.links.iter().find(|l| l.local_bank_id == local_id).is_some_and(|l| l.difficulty_read.is_none())
    }

    /// (round 17, critic 96 I2) "Use my bank as it is" was confirmed for this copy: its Send page says what the send does.
    pub fn sends_as_is(&self, local_id: &str) -> bool {
        self.links.iter().find(|l| l.local_bank_id == local_id).is_some_and(|l| l.as_is)
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
            Ok(Fetch::Moved) => MergePhase::Failed(Problem { message: "Someone sent changes while this was loading. Press Try again to start over.".into(), retry: true, retry_at: None, account: false }),
            Ok(Fetch::UpToDate) => {
                let local = if let EditView::Merge(m) = &self.view { m.local_id.clone() } else { String::new() };
                self.caught_up(&local);
                if self.copy_is_stale(&local) || self.cant_be_read(&local) { MergePhase::StaleUpToDate { ask: std::mem::take(&mut self.rebase_next).then_some(StaleAsk::Rebase) } } else { MergePhase::UpToDate }
            }
            Err(e) => {
                let kind = if let PoolError::Refused(r) = &e { Some(r.kind.clone()) } else { None };
                match kind {
                    Some(RefusalKind::VersionWithheld) => MergePhase::HeadWithheld { head_version: self.link_head(cid, bid), replaces: Vec::new() },
                    Some(RefusalKind::InvalidRequest { field: Some(f) }) if f == "base_version" => {
                        // The copy is already at the newest version.
                        let local = if let EditView::Merge(m) = &self.view { m.local_id.clone() } else { String::new() };
                        self.caught_up(&local);
                        if self.copy_is_stale(&local) || self.cant_be_read(&local) { MergePhase::StaleUpToDate { ask: std::mem::take(&mut self.rebase_next).then_some(StaleAsk::Rebase) } } else { MergePhase::UpToDate }
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
        // (asked once, for this fetch only)
        self.rebase_next = false;
        if let EditView::Merge(m) = &mut self.view {
            // (round 17) opened for "Use my bank as it is", but there is nothing to decide: an ordinary Get latest page
            if !matches!(phase, MergePhase::StaleUpToDate { .. }) {
                m.as_is = false;
            }
            m.phase = phase;
        }
    }

    /// (critic 72 I2) Get latest found the copy at the circle's newest version, or put it there: a result that said to
    /// catch up is out of date.
    fn caught_up(&mut self, local: &str) {
        // (critic 75 minor 5) in the links file too
        self.update_link(local, |l| l.sent_unread.take().is_some());
        // (critic 78 I1) the copy is at the circle's newest version: whatever the earlier send did is in it (and what it
        // carried is no longer needed to tell)
        self.forget_maybe(local);
        if let Some(b) = self.result_bank_of_local(local) {
            self.results.settle(&b, ResultKind::GetLatest);
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
            m.phase = MergePhase::Blocked("This bank is no longer linked to the circle.".into());
            return;
        };
        let Some(bank) = banks.iter().find(|b| b.id == link.local_bank_id) else {
            m.phase = MergePhase::Blocked("The bank is not on this device any more, so there is nothing to bring changes into.".into());
            return;
        };
        // (critic 63 B1) The viewers' form never goes into a copy that has its answer keys: nothing is reviewed.
        if f.head.bank.keys_hidden && !link.viewers_form {
            m.phase = MergePhase::Blocked(KEYS_KEPT.into());
            return;
        }
        let at = self.clock.unwrap_or_else(now_ms);
        // (phase D1: a viewer's copy of a bank that hides its keys follows the viewers' form, marked keyless)
        let theirs = circle_theirs_as(&f.head.payload, at, f.head.bank.keys_hidden);
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
        // (critic 87 B1) A base whose difficulties were to be learnt from the circle's version at the base version learnt them
        // from the one read for this review before it is made (see `learn_from_fetch`).
        let (base, fallback) = if !stale && !link.base.is_empty() {
            (link.base.clone(), false)
        } else if let (false, Some(bd)) = (stale, &f.base) {
            let bt = circle_theirs_as(&bd.payload, at, bd.bank.keys_hidden);
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
        // (critic 63 B2) The viewers' form is shuffled afresh on every read: a question whose choices only moved is
        // not a change, and keeps the order this copy has.
        let mut theirs = theirs;
        if f.head.bank.keys_hidden {
            settle_shuffle(&mut theirs, &base);
        }
        // (critic 80 I1) A send from this bank whose answer did not say what it made: where the circle has a question as
        // that send carried it, that is what the copy is in step with (a correction made since is "changed only by you",
        // never a conflict with the person's own send).
        // (the same account rule as `record_mine`, read here beside the open review) (critic 81 I2) Only while that send may
        // have gone through: not once a colleague's version is known to have come right after the one it was based on,
        // and only when the version right after it, as the circle lists it now, is not a colleague's. (The circle gives
        // names only, see `sender`: while the person's own name is not known, what the send carried decides alone.)
        let me = super::bank_links::account_tag(self.account.as_deref()).unwrap_or_default();
        let record = link.sent_record.clone().filter(|r| {
            (r.by.is_empty() || r.by == me) && !r.not_through && r.base.checked_add(1).is_some() && !matches!(sender_of(self.me_name.as_deref(), next_sender(r.base, &f.start)), Who::Other(_))
        });
        let own_at = record.as_ref().and_then(|r| r.base.checked_add(1)).unwrap_or(0);
        // (critic 83 I2) What that send carried is compared with the version it made, the person's own right after the one
        // it was based on: the newest, or (colleagues sent more since) that version as read for this review. Not readable:
        // the newest only, as before.
        let own_doc = f.mine.as_ref().filter(|d| f.start.head_version != own_at && d.bank.version == own_at && !d.bank.keys_hidden);
        let mine = own_doc.map(|d| circle_theirs(&d.payload, at));
        let sent: Vec<(&str, &crate::model::Question)> = record.iter().flat_map(|r| r.questions.iter()).map(|q| (q.qid.as_str(), &q.q)).collect();
        let linked: Vec<&str> = link.qids.iter().map(|q| q.qid.as_str()).collect();
        let mut base = base;
        let found = merge::rebase_own_send(&mut base, &linked, &sent, mine.as_deref().unwrap_or(&theirs), &same_picture, !fallback);
        let mut plan = merge::build_plan(&link.circle_bank_id, &base, bank, &theirs, link.base_version, f.start.head_version, None, String::new());
        for row in plan.rows.iter_mut().filter(|r| found.resent.contains(&r.base_id)) {
            match row.kind {
                RowKind::OursChanged => row.note = Some(text::sent_then_changed(own_at)),
                // (critic 78 I1, critic 80 I1) the question that send added, as the circle has it: linked, not added twice
                RowKind::Unchanged if found.added.contains(&row.base_id) => {
                    row.note = Some(text::SAME_ADDED.into());
                    plan.matched += 1;
                }
                _ => {}
            }
        }
        // (critic 78 I1) A question new in the circle that this bank already has, unlinked and the same (the person's own,
        // sent without a readable answer): linked to it, never added a second time.
        merge::match_new_to_unlinked(&mut plan, bank, &theirs, &same_picture, text::SAME_ADDED);
        // (critic 85 m1) That send's own version was needed (colleagues sent more since) and can not be read: what it
        // changed and the newest does not have is not said to be the person's alone, and while a question it added is not
        // matched, a question new in the circle (which may be that one, changed since) is not added by default.
        // (also in a two-way merge: a question that send added is no more matched there)
        let own_unread = (own_at > 0 && own_at < f.start.head_version && mine.is_none()).then_some(own_at);
        if let Some(n) = own_unread {
            let linked_now: Vec<&str> = plan.rows.iter().filter_map(|r| r.local_id.as_deref()).collect();
            let unmatched_added = sent.iter().any(|(qid, q)| qid.is_empty() && !linked_now.contains(&q.id.as_str()) && bank.questions.iter().any(|b| b.id == q.id));
            let changed: Vec<&str> = sent.iter().filter(|(qid, q)| !qid.is_empty() && base.iter().any(|b| b.local_id == q.id && merge::ours_changed(b, q))).map(|(_, q)| q.id.as_str()).collect();
            for row in &mut plan.rows {
                match row.kind {
                    RowKind::OursChanged if changed.contains(&row.base_id.as_str()) => {
                        row.note = Some(text::sent_change_unread(n));
                        // (critic 88 I3) someone may have changed it since: the circle's version can be taken instead
                        row.options = vec![Choice::Keep, Choice::Take];
                    }
                    RowKind::NewInTheirs if unmatched_added => {
                        row.choice = Choice::Undecided;
                        row.note = Some(text::maybe_your_added(n));
                    }
                    _ => {}
                }
            }
        }
        let start = f.start.clone();
        // (critic 85 m4) The version right after the one the send was based on is the person's by name and holds every
        // question the send carried as it carried them: it is that send, also where what the send changed can not be told
        // (a difficulty, on a base an earlier build wrote).
        // (Only when what it carried was matched to that version itself: the newest, read in its place, says nothing of it.)
        let by_me = record.as_ref().is_some_and(|r| matches!(sender_of(self.me_name.as_deref(), next_sender(r.base, &f.start)), Who::Me));
        let own = found.own || (by_me && (mine.is_some() || own_at == f.start.head_version) && found.carried > 0 && found.matched == found.carried);
        let resent = found.resent;
        let own_version = own.then_some(own_at);
        // (the review says the version may be the send's only where it does not already say it is)
        let own_unread = own_unread.filter(|_| !own);
        let resent_at = if mine.is_some() { own_at } else { start.head_version };
        m.phase = MergePhase::Review(Box::new(Review { plan, fallback, start, problem: self.merge_note.take(), own_version, resent, resent_at, own_unread, base, ours_print: print_of(bank), fetched: f }));
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
        // (critic 63 B2) A read in the viewers' form is not read again: the next read would be shuffled afresh.
        let work = if r.fetched.head.bank.keys_hidden { Work::MergeApplyBody { doc: Box::new(r.fetched.head.clone()) } } else { Work::MergeApply { cid, bid, version } };
        m.phase = MergePhase::Applying(r);
        if !self.launch(work)
            && let EditView::Merge(m) = &mut self.view
            && let MergePhase::Applying(r) = std::mem::replace(&mut m.phase, MergePhase::Fetching)
        {
            let mut r = r;
            r.problem = Some("Not signed in. Sign in, then press Apply changes again.".into());
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
        // (round 17) "No, go back" on the page opened for "Use my bank as it is": back to the bank
        if matches!(&self.view, EditView::Merge(m) if m.as_is && matches!(m.phase, MergePhase::StaleUpToDate { ask: Some(StaleAsk::Rebase) })) {
            self.cancel_merge();
            return;
        }
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

    /// (round 20, critic 100 minor 4) First press of "Put all of mine back": the news asks, and changes nothing yet.
    pub fn ask_put_all(&mut self, local_id: &str) {
        self.put_all_ask = Some(local_id.to_string());
    }

    pub fn cancel_put_all(&mut self) {
        self.put_all_ask = None;
    }

    pub fn put_all_asked(&self, local_id: &str) -> bool {
        self.put_all_ask.as_deref() == Some(local_id)
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
        // (round 16, critic 93 I2) she declared the bank as it is to be what the next send sends: a read still awaited (on its
        // way, or asked again later) is settled here, so nothing it brings changes the bank, whichever arrives first. What the
        // circle's version rated stays unknown (critic 87 B1: never the bank's rating taken for the circle's), so a change to
        // a difficulty is said not to be counted, and Get latest asks about a differing one.
        if needs_difficulty_read(&link) || link.inherit {
            // (round 17, critic 95 I2) a base an earlier build wrote (the round 12 build filled it from the bank) is not the
            // circle's: its difficulties are not known, as for a phase C link, so the strip, Get latest and Send agree
            if link.difficulty_read.is_none() {
                for b in &mut link.base {
                    b.snap.difficulty = None;
                }
            }
            link.difficulty_read = Some(link.base_version);
            link.inherit = false;
        }
        link.at_read = None;
        link.pass_times = None;
        // (round 17, critic 96 I2) the next send makes the circle's newest version match the bank: its Send page says so
        link.as_is = true;
        self.inherit_held.remove(&local);
        link.missing_at_read = None;
        self.statuses.remove(&local);
        self.replace_link(link);
        if let EditView::Merge(m) = &mut self.view {
            m.phase = MergePhase::UpToDate;
            // (round 17) the page says what was done
            m.as_is = true;
        }
        if let Some(b) = self.result_bank_of_local(&local) {
            // (round 17, critic 96 I2) in the words of its page: what the next send does
            self.say(b, ResultKind::GetLatest, text::AS_IS_RESULT);
        }
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

    fn on_applied(&mut self, cid: &str, bid: &str, r: Result<ApplyRead, PoolError>) {
        if !self.here(cid, bid) || !matches!(&self.view, EditView::Merge(m) if matches!(m.phase, MergePhase::Applying(_))) {
            return;
        }
        match r {
            Ok(ApplyRead::Doc(imp)) => self.applying = Some(imp),
            Ok(ApplyRead::Moved) => self.back_to_review("The circle has a different newest version now than the one you reviewed. Nothing was applied. Cancel and press Get latest again.".into()),
            Ok(ApplyRead::KeysHidden) => {
                // (critic 67 M2) a copy made from the viewers' form has no keys to keep: it is told what happened
                let keyless = self.links.iter().any(|l| l.circle_id == cid && l.circle_bank_id == bid && l.viewers_form);
                self.back_to_review(if keyless { KEYS_GONE_AT_APPLY } else { KEYS_KEPT }.into())
            }
            Err(e) => match self.problem_of(&e) {
                Some(p) => self.back_to_review(p.message),
                None => self.back_to_review("Your session ended. Sign in again, then press Apply changes. Nothing was changed.".into()),
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
            back(m, rev, "This bank is no longer linked to the circle. Nothing was applied.".into());
            return false;
        };
        if imp.head.item.version != rev.plan.new_version {
            back(m, rev, "The circle has a different newest version now than the one you reviewed. Nothing was applied. Cancel and press Get latest again.".into());
            return false;
        }
        let Some(pos) = banks.iter().position(|b| b.id == local_id) else {
            back(m, rev, "The bank is not on this device any more. Nothing was applied.".into());
            return false;
        };
        if imp.pictures_missing > 0 {
            let n = imp.pictures_missing;
            back(m, rev, format!("{n} picture{} of the circle's newest version could not be fetched. Nothing was applied, so no picture is lost. Press Apply changes to try again.", if n == 1 { "" } else { "s" }));
            return false;
        }
        if print_of(&banks[pos]) != rev.ours_print {
            // Edited here after the review was built: the review is made again from the bank as it is now.
            let r = *rev;
            self.merge_note = Some("Your bank changed while you were reviewing, so nothing was applied. This is the review again, with your latest changes. Check it, then press Apply changes.".into());
            m.phase = MergePhase::Ready(r.fetched);
            return false;
        }
        let author = api::plain(&imp.head.item.author_name);
        let mut bank = banks[pos].clone();
        // (the same order as the review: see build_review)
        let mut theirs = imp.theirs.clone();
        if rev.fetched.head.bank.keys_hidden {
            settle_shuffle(&mut theirs, &rev.base);
        }
        let (outcome, new_base) = match merge::apply_merge(&mut bank, &rev.plan, &theirs, &rev.base, &author, imp.at_ms) {
            Ok(a) => a,
            Err(msg) => {
                // (critic 72 I1) merge.rs speaks of "the pool": here it is the circle
                back(m, rev, super::circles_text::merge_words(&msg));
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
        let before = banks[pos].clone();
        let prev = banks[pos].modified;
        banks[pos] = bank;
        // Seconds, like every editor writes; never the millisecond clock.
        banks[pos].modified = (self.now_ms() / 1000).max(prev.saturating_add(1));
        // (critic 101 I1) the merged bank reaches the disk before the link says the merge was made: a links file ahead
        // of the banks file is the "put back from a safety copy" state, and her choices in the merge are lost. The save
        // is the model's, checked, not the caller's (which discards its error). On failure nothing changes.
        let saved = match &self.bank_saver {
            Some(f) => f(banks),
            None => self.log.save_banks(banks),
        };
        if let Err(msg) = saved {
            banks[pos] = before;
            let why = format!("Your banks couldn't be saved ({}), so nothing was changed. Press Apply changes to try again; if it keeps failing, free some disk space.", super::circles_text::save_problem(&msg));
            if let EditView::Merge(mv) = &mut self.view {
                back(mv, rev, why);
            }
            return false;
        }
        link.mark_bank(&banks[pos]);
        link.qids = qids_of(&new_base);
        link.base = merge::bounded(new_base);
        link.base_version = imp.head.item.version;
        link.base_version_id = imp.head.item.publish_id.clone();
        link.head_seen = link.head_seen.max(link.base_version);
        // (critic 89 I1) the base is the circle's version as merged: its difficulties are known as written, and (round 16) none
        // is left to take (one that could not be written before is the merge's to decide)
        link.difficulty_read = Some(0);
        link.inherit = false;
        link.at_read = None;
        link.pass_times = None;
        link.missing_at_read = None;
        link.as_is = false;
        // (round 7, critic M3) the base is the circle's version as merged: no restored question waits for a difficulty pass
        // any more (the base record and the bank's files are new)
        link.restored.clear();
        // (critic 63 I3) Read in full: the answer keys came with it, so the copy no longer follows the viewers' form.
        if !rev.fetched.head.bank.keys_hidden {
            link.viewers_form = false;
        }
        let carry = Carry { local_id: local_id.clone(), token: rev.start.merge_token.clone(), from: rev.plan.old_version, head: link.base_version };
        let changes = changes_since(&link, &banks[pos]);
        self.advanced.push(Advanced { circle_bank_id: link.circle_bank_id.clone(), local_bank_id: local_id.clone(), version: link.base_version });
        let version = link.base_version;
        self.statuses.remove(&local_id);
        self.replace_link(link);
        // the merge wrote every bank a moment ago: a "your own edits aren't saved" warning is out of date
        self.banks_saved();
        self.caught_up(&local_id);
        // The next send, if there is something to send, goes with the token (a follow link's get has none).
        self.carry = (changes.total() > 0 && !carry.token.is_empty()).then_some(carry);
        if let EditView::Merge(m) = &mut self.view {
            m.phase = MergePhase::Applied(Box::new(MergeDone { outcome, restore, version, changes }));
        }
        true
    }

    // ---- one question's history (phase D2) --------------------------------------------------------------------------

    /// (round 5, critic I3) The bank whose question is open in the app's editor (from the bank's page), or none.
    pub fn set_question_draft(&mut self, bank_id: Option<&str>) {
        self.question_draft = bank_id.map(str::to_string);
    }

    /// "History in the circle" on a question's card: the page opens in place of the bank's questions (the strip draws it,
    /// as it draws the bank's History). Returns true when it opened. Reads nothing for a bank with no link.
    pub fn open_question_history(&mut self, bank: &QuestionBank, question_id: &str) -> bool {
        self.ensure();
        let Some(link) = self.links.iter().find(|l| l.local_bank_id == bank.id).cloned() else { return false };
        if self.wait_for_act() {
            return false;
        }
        self.leave_view();
        // (critic 89 m2) a quiet read of a base version on its way is kept
        self.jobs.retain(|j| j.kind == JobKind::Base);
        self.history_stash = None;
        // (round 4, critic S-m6) the cache belongs to the account signed in now, before anything is read from it
        let tag = super::bank_links::account_tag(self.session.as_ref().and_then(|s| s.account()).as_deref());
        self.question_cache.note_account(tag.as_deref());
        let env = super::question_history_ui::Env { session: self.session.clone(), runner: self.runner, retry: self.retry, repaint: self.repaint.clone(), images: self.images.clone(), restore: self.restore.clone() };
        let page = super::question_history_ui::QuestionPage::open(&link, bank, question_id, env, &mut self.question_cache);
        self.view = EditView::Question(Box::new(page));
        self.host = Some(EditHost { local_id: bank.id.clone(), back: format!("< Back to {}", api::plain(&bank.name)) });
        // (the strip notes the frame it draws the page in: one an earlier screen left must not close this one)
        self.host_frame = None;
        true
    }

    // ---- the history -------------------------------------------------------------------------------------------

    /// The bank's history of versions. Returns true when the page opened.
    pub fn open_history(&mut self, cid: &str, bid: &str, title: &str, perm: Perm) -> bool {
        if !super::pool::is_item_id(cid) || !super::pool::is_item_id(bid) {
            return false;
        }
        self.ensure();
        if self.wait_for_act() {
            return false;
        }
        self.leave_view();
        // (critic 89 m2) a quiet read of a base version on its way is kept
        self.jobs.retain(|j| j.kind == JobKind::Base);
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
        true
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
            h.problem = Some(Problem { message: "Not signed in. Open the Account window and sign in.".into(), retry: true, retry_at: None, account: true });
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
                if first {
                    self.unsure.remove(&(cid.to_string(), bid.to_string()));
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
            let local = l.local_bank_id.clone();
            self.replace_link(l);
            self.head_moved(&local);
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
            v.load = VersionLoad::Failed(Problem { message: "Not signed in. Open the Account window and sign in.".into(), retry: true, retry_at: None, account: true });
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
        if matches!(self.view, EditView::Version(_)) {
            self.jobs.retain(|j| j.kind != JobKind::Version);
            self.view = self.history_stash.take().map_or(EditView::Closed, EditView::History);
            // (a restore or trim answered while a version was looked at: the history is read again)
            if matches!(&self.view, EditView::History(h) if !h.loaded && h.problem.is_none()) {
                self.load_history(None);
            }
            return;
        }
        // (critic 69 I1) a restore or trim on its way is kept, and a result not closed is said on the bank's strip
        self.leave_view();
        self.jobs.retain(|j| matches!(j.kind, JobKind::Act | JobKind::Base));
        self.history_stash = None;
        self.view = EditView::Closed;
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
            r.stage = RestoreStage::Failed(Problem { message: "Not signed in. Open the Account window and sign in.".into(), retry: true, retry_at: None, account: true });
        }
    }

    /// The History showing this circle bank: the open one, or the one kept while one of its versions is looked at.
    fn history_of(&mut self, cid: &str, bid: &str) -> Option<&mut HistoryView> {
        match &mut self.view {
            EditView::History(h) if h.cid == cid && h.bid == bid => Some(h),
            EditView::Version(_) => self.history_stash.as_deref_mut().filter(|h| h.cid == cid && h.bid == bid),
            _ => None,
        }
    }

    /// A History whose list changed while it was not the open screen is read again when it is shown (see `back`).
    fn history_changed(&mut self, cid: &str, bid: &str) {
        if matches!(&self.view, EditView::History(h) if h.cid == cid && h.bid == bid) {
            self.reload_history_keeping_ask();
        } else if let Some(h) = self.history_of(cid, bid) {
            h.loaded = false;
            h.lines.clear();
            h.next_before = None;
        }
    }

    fn on_rolled(&mut self, cid: &str, bid: &str, to: u64, r: Result<SentBank, PoolError>) {
        // (critic 69 I1, critic 71 I3) the circle's newest version, as the answer says it: the bank's strip never says
        // "up to date" behind it, even when the History's own reload is never read (it was left)
        let conflict = match &r {
            Err(PoolError::Refused(rf)) => match &rf.kind {
                RefusalKind::CircleConflict { head_version, .. } => Some(*head_version),
                _ => None,
            },
            _ => None,
        };
        match (&r, conflict) {
            (Ok(sent), _) => self.note_head(cid, bid, sent.version),
            (_, Some(Some(h))) => self.note_head(cid, bid, h),
            _ => {}
        }
        // (critic 76 I2) Restored, but the answer named no version the app can read: the circle's newest version is not
        // known here. It is "when last checked" (never "up to date") until the bank is checked, which it is at once.
        // (critic 77 I2) The same for an answer that does not say whether it went (none came, it could not be read, the
        // server failed on its side): never "did not go through".
        let unclear = r.as_ref().err().and_then(unclear_why);
        // (critic 77 I2) this answer replaces what an earlier unclear one left to be checked
        self.unclear_restores.remove(&(cid.to_string(), bid.to_string()));
        if unclear.is_some() {
            // (critic 79 I2) with the version it was based on: the check decides by it
            let base = self.act_for.as_ref().filter(|a| a.what == "restore" && a.cid == cid && a.bid == bid).map(|a| a.base).or_else(|| self.link_head(cid, bid)).unwrap_or(0);
            self.unclear_restores.insert((cid.to_string(), bid.to_string()), (to, base, None, false));
        }
        if matches!(&r, Ok(sent) if sent.version == 0) || unclear.is_some() {
            self.unsure.insert((cid.to_string(), bid.to_string()));
            if let Some(l) = self.local_of(cid, bid) {
                self.check_asks.push(l);
            }
        }
        // (critic 71 minor 2) asked for before another account signed in: nothing changes for the account here now (no
        // sign-out, no result)
        if !self.act_mine() {
            return;
        }
        let earlier = matches!(&r, Err(e) if self.ended_earlier(e));
        let stage = match r {
            Ok(sent) => {
                self.pending_restore = None;
                Some(RestoreStage::Done { version: sent.version, held: sent.held, withheld: sent.withheld })
            }
            // (critic 77 I2) the commit id is kept: Try again goes as the same restore
            Err(_) if let Some(why) = unclear => Some(RestoreStage::Unclear(why)),
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
                // (critic 73 minor 3) the session that ended was an earlier one: the account is signed in again
                _ if earlier => Some(RestoreStage::Failed(Problem { message: EARLIER_SESSION.into(), retry: true, retry_at: None, account: false })),
                _ => self.problem_of(&e).map(RestoreStage::Failed),
            },
        };
        // (critic 73 minors 1, 3) the session ended before the answer: the restore may have gone through, so the newest
        // version is "when last checked" until the circle is read again
        if earlier || stage.is_none() {
            self.unsure.insert((cid.to_string(), bid.to_string()));
        }
        let reload = matches!(stage, Some(RestoreStage::Done { .. })) || conflict.is_some();
        let asked = self.history_of(cid, bid).is_some_and(|h| h.restore.as_ref().is_some_and(|x| x.to == to));
        if !(asked && self.history_shown()) {
            // (critic 69 I1, critic 71 I1) The History was left (Back, the Circles tab, another bank's page), or is
            // open where the person is not looking (Bank settings on the Shared page): how it went is said on the
            // bank's strip and Bank settings, and the History no longer holds the restore.
            if asked && let Some(h) = self.history_of(cid, bid) {
                h.restore = None;
            }
            if reload {
                self.history_changed(cid, bid);
            }
            let said = match conflict {
                // (critic 71 I3) no history was reloaded here: the person chooses again
                Some(h) => restore_conflict_note(to, h),
                None if earlier => format!("Your session ended before the restore of version {to} was answered. Open History to see whether it went through."),
                None => restore_note(to, stage.as_ref()).unwrap_or_else(|| format!("Your session ended before the restore of version {to} was answered. Sign in, then open History to see whether it went through.")),
            };
            let bank = self.act_bank(cid, bid);
            self.say(bank, ResultKind::Restore, said);
            return;
        }
        if let Some(h) = self.history_of(cid, bid)
            && let Some(x) = &mut h.restore
        {
            x.stage = stage.unwrap_or(RestoreStage::Confirm);
        }
        if reload {
            self.history_changed(cid, bid);
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
            && matches!(h.restore.as_ref().map(|r| &r.stage), Some(RestoreStage::Done { .. } | RestoreStage::Failed(_) | RestoreStage::Unclear(_)))
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
        if !h.perm.can_rewind() || !matches!(t.stage, TrimStage::Confirm | TrimStage::Failed(_) | TrimStage::Unclear(_)) || acting {
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
            t.stage = TrimStage::Failed(Problem { message: "Not signed in. Open the Account window and sign in.".into(), retry: true, retry_at: None, account: true });
        }
    }

    fn on_trimmed(&mut self, cid: &str, bid: &str, r: Result<Trimmed, PoolError>) {
        // (critic 71 minor 2) asked for before another account signed in: nothing changes for the account here now
        if !self.act_mine() {
            return;
        }
        let earlier = matches!(&r, Err(e) if self.ended_earlier(e));
        let stage = match r {
            Ok(t) => Some(TrimStage::Done(t)),
            // (critic 77 I2) no clear answer: it may have gone through
            Err(e) if let Some(why) = unclear_why(&e) => Some(TrimStage::Unclear(why)),
            // (critic 73 minor 3) the session that ended was an earlier one: the account is signed in again
            Err(_) if earlier => Some(TrimStage::Failed(Problem { message: EARLIER_SESSION.into(), retry: true, retry_at: None, account: false })),
            Err(e) => self.problem_of(&e).map(TrimStage::Failed),
        };
        let done = matches!(stage, Some(TrimStage::Done(_)));
        let asked = self.history_of(cid, bid).is_some_and(|h| h.trim.is_some());
        if !(asked && self.history_shown()) {
            // (critic 69 I1, critic 71 I1) the History was left, or is not where the person is looking: how the trim
            // went is said on the bank's strip and Bank settings
            if asked && let Some(h) = self.history_of(cid, bid) {
                h.trim = None;
            }
            if done {
                self.history_changed(cid, bid);
            }
            let said = if earlier { Some("Your session ended before the trim was answered. Open History to see whether it went through.".to_string()) } else { trim_note(stage.as_ref()) };
            let said = said.unwrap_or_else(|| "Your session ended before the trim was answered. Sign in, then open History to see whether it went through.".into());
            let bank = self.act_bank(cid, bid);
            self.say(bank, ResultKind::Trim, said);
            return;
        }
        if let Some(h) = self.history_of(cid, bid)
            && let Some(t) = &mut h.trim
        {
            t.stage = stage.unwrap_or(TrimStage::Confirm);
        }
        if done {
            self.history_changed(cid, bid);
        }
    }

    pub fn close_trim_result(&mut self) {
        if let EditView::History(h) = &mut self.view
            && matches!(h.trim.as_ref().map(|t| &t.stage), Some(TrimStage::Done(_) | TrimStage::Failed(_) | TrimStage::Unclear(_)))
        {
            h.trim = None;
        }
    }

    /// The last change again after a failure ("Try again").
    pub fn retry_act(&mut self) {
        match self.last_act {
            Some(ActAgain::Send) => {
                if matches!(&self.view, EditView::Send(v) if matches!(v.stage, SendStage::Failed(_))) {
                    // (the pictures kept go again: the pool never asks about them, so they would be lost; critic 79 minor
                    // 4: and every viewers' copy decision, so that Try again is the same request. `do_send` drops them
                    // for content they were not made for.)
                    let keep = match &mut self.view {
                        EditView::Send(v) => Some((v.picked.take(), std::mem::take(&mut v.decisions), v.decisions_for.clone())),
                        _ => None,
                    };
                    self.back_to_note();
                    if let (EditView::Send(v), Some((p, decisions, Some(sha)))) = (&mut self.view, keep) {
                        v.decisions = decisions;
                        v.decisions_for = Some(sha);
                        v.picked = p;
                    }
                    self.send();
                }
            }
            Some(ActAgain::Restore(_)) => {
                if matches!(&self.view, EditView::History(h) if h.restore.as_ref().is_some_and(|r| matches!(r.stage, RestoreStage::Failed(_) | RestoreStage::Unclear(_)))) {
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
    ///
    /// (round 6, critic N3) This says the bank's page is **not** in front of the person: a restore asked for on the
    /// question's history is not made here. The bank's strip, which is about to draw that page, calls [`sync_in_front`].
    /// The Shared page and tests that drive the model on its own come through here.
    pub fn sync(&mut self, banks: &mut [QuestionBank]) -> bool {
        self.sync_in_front(banks, false)
    }

    /// [`sync`], saying whether the bank's page is drawn in this frame: a restore (or its undo) is made only then, so
    /// its result and Undo are seen.
    pub fn sync_in_front(&mut self, banks: &mut [QuestionBank], page_in_front: bool) -> bool {
        self.ensure();
        // (round 18, critic 97 m6) a links file that could not be written is written again
        self.links_again();
        // (round 19, critic 100 I2) and the banks file, when she pressed Try again after it could not be written
        self.banks_again(banks);
        // (critic 89 m2) a review waits for the quiet read of its base version on its way
        let review = matches!(&self.view, EditView::Merge(m) if matches!(m.phase, MergePhase::Ready(_))) && !self.merge_waits_for_base();
        if review {
            self.learn_from_fetch();
        }
        // (the owner's decision, critic 90 I1) before anything is counted or reviewed
        let inherited = self.inherit_pending(banks);
        // (round 18, critic 98 I1) what changed since in the difficulty news is noted for good
        self.settle_all_news(banks);
        // (round 17, critic 95 minor 1) nor is a review built while the circle's difficulties wait to be put into the bank (it
        // would call the circle's difficulty hers): it is built once they are in
        let review = review && !self.merge_waits_for_difficulty();
        // (phase D2) the question history page: its reads, the bank it draws, and a restore of one question, which changes the
        // bank only here and writes it first, as a merge does; then its link, when the question no longer pairs with the circle's
        let now = self.now_ms();
        let (saver, log, session) = (self.bank_saver.clone(), self.log.clone(), self.session.clone());
        let save = move |b: &[QuestionBank]| match &saver {
            Some(f) => f(b),
            None => log.save_banks(b),
        };
        // (round 5, critic I3; round 6, critic N3) a restore is made only while its page is in front of the person: the
        // frame syncing must be the one that drew it last (the strip calls `sync_in_front` for its own bank, and the page is
        // drawn after this sync) and no question of the bank may be open in the editor
        let shown = page_in_front && self.host_frame.is_some_and(|f| self.pass.is_none_or(|p| f + 1 >= p));
        let draft = match &self.view {
            EditView::Question(p) => self.question_draft.as_deref() == Some(p.local_bank_id.as_str()),
            _ => false,
        };
        let out = match &mut self.view {
            EditView::Question(p) => Some((super::question_history_ui::sync_page(p, &mut self.question_cache, banks, &self.links, session, &save, now, draft, shown), p.cid.clone(), p.bid.clone())),
            _ => None,
        };
        // (round 5) the way back names the bank as it is called now
        if let (EditView::Question(p), Some(h)) = (&self.view, self.host.as_mut())
            && let Some(b) = banks.iter().find(|b| b.id == p.local_bank_id)
        {
            h.back = format!("< Back to {}", api::plain(&b.name));
        }
        // (round 3, critic M1) the link or the bank went, or the circle no longer lists the person: the page closes, and what
        // it read for the bank goes with it
        if let Some((o, cid, bid)) = &out
            && o.close
        {
            self.question_cache.drop_bank(cid, bid);
            self.close();
        }
        let restored = out.and_then(|(o, _, _)| o.changed);
        let inherited = inherited || restored.is_some();
        // (round 3, critic M6) the bank is written first (by the restore), then the link: see `restore_question`
        if let Some((local, Some(link))) = restored {
            self.update_link(&local, |l| {
                *l = link;
                true
            });
        }
        self.refresh_statuses(banks);
        if let EditView::Send(v) = &mut self.view
            && let Some(s) = self.statuses.get(&v.local_id)
        {
            v.changes = Some(s.changes);
        }
        // (phase D3) the body read when the page opened, worked out against the bank as it is now
        self.resolve_partial(banks);
        if self.cmd_send {
            self.cmd_send = false;
            self.do_send(banks);
        }
        if std::mem::take(&mut self.cmd_rebase) {
            self.do_rebaseline(banks);
        }
        if review {
            self.build_review(banks);
        }
        if let Some(imp) = self.applying.take() {
            return self.finish_merge(banks, imp) || inherited;
        }
        inherited
    }

    /// (phase D3) The send page's base version body, against the bank as it is now: every change ticked except the
    /// ones the link left in `held_back`; a base that can not be read, or a bank it can not list, leaves the whole
    /// bank (plan 4.7). Does nothing until both the body and the bank are here.
    fn resolve_partial(&mut self, banks: &[QuestionBank]) {
        let (local, arrived) = match &mut self.view {
            EditView::Send(v) => {
                if !matches!(v.partial, PartialSend::Reading) {
                    return;
                }
                (v.local_id.clone(), v.base_body.take())
            }
            _ => return,
        };
        let Some((n, body)) = arrived else { return };
        let (Some(link), Some(bank)) = (self.links.iter().find(|l| l.local_bank_id == local).cloned(), banks.iter().find(|b| b.id == local)) else {
            // (the bank may be on its way, or the link written yet: the body waits for the next pass)
            if let EditView::Send(v) = &mut self.view {
                v.base_body = Some((n, body));
            }
            return;
        };
        let state = if link.base_version != n {
            // (the copy caught up with another version meanwhile: this body is not its base)
            PartialSend::Unavailable(PARTIAL_UNAVAILABLE.into())
        } else {
            match changes_against_body(&link, bank, &body) {
                Ok(changes) => {
                    // All changes start ticked, but the ones the person left unticked at the last send, and the ones a
                    // conflict's Get latest carries by local id. An id no longer a change is simply not here; a change
                    // the person has not seen yet starts ticked (plan 4.7).
                    let mut ticked = changes.ids();
                    ticked.retain(|id| !link.held_back.contains(id));
                    let carried = self.send_unticked.take().filter(|(l, _)| *l == local);
                    if let Some((_, unticked)) = carried {
                        ticked.retain(|id| !unticked.contains(id));
                    }
                    let rows = send_rows(&link, bank, &changes);
                    PartialSend::Ready { base: n, body, print: bank_print(bank), changes, ticked, rows }
                }
                Err(m) => PartialSend::Unavailable(m),
            }
        };
        if let EditView::Send(v) = &mut self.view
            && matches!(v.partial, PartialSend::Reading)
        {
            v.partial = state;
            v.notice = None;
        }
    }

    fn refresh_statuses(&mut self, banks: &[QuestionBank]) {
        // (the links are lent out, not copied: a link's snapshot can be large)
        let links = std::mem::take(&mut self.links);
        for l in &links {
            let bank = banks.iter().find(|b| b.id == l.local_bank_id);
            self.refresh_one(l, bank);
        }
        self.links = links;
    }

    /// Works out where the link stands, unless that is known for the bank as it is (and for what the link knows of the
    /// difficulties: critic 87 B1 learns them from the circle).
    fn refresh_one(&mut self, l: &CircleLink, bank: Option<&QuestionBank>) {
        let key = bank.map_or((0, 0), |b| (b.modified, b.questions.len()));
        let stale = bank.is_some_and(|b| l.stale_for(b));
        let name = bank.map(|b| b.name.clone()).unwrap_or_default();
        let unknown = difficulty_unknown(l);
        let fresh = self.statuses.get(&l.local_bank_id).is_some_and(|s| s.key == key && s.missing == bank.is_none() && s.stale == stale && s.bank_name == name && s.unknown == unknown);
        if !fresh {
            let raw = bank.map(|b| changes_since(l, b)).unwrap_or_default();
            self.statuses.insert(l.local_bank_id.clone(), LinkStatus { key, raw, changes: raw, missing: bank.is_none(), stale, bank_name: name, unknown, learning: None, held: None, limit_left: None, unread: None, unsaved: None, news: false });
        }
        let learning = self.learning_of(l);
        let unread = learning.is_some();
        if let Some(s) = self.statuses.get_mut(&l.local_bank_id) {
            s.changes = s.raw;
            // (round 15, critic 91 I4) until the read has succeeded, a change to a difficulty may be missing from the count
            if unread && s.changes.known {
                s.changes.known = false;
                s.changes.difficulty = true;
            }
            s.learning = learning;
            s.held = (learning == Some(Learning::Held)).then(|| self.inherit_held.get(&l.local_bank_id).map(|h| (h.why.clone(), h.tries))).flatten();
            s.unsaved = self.banks_unsaved.get(&l.local_bank_id).cloned();
            // (round 19, critic 100 minor 5) the news, while something of it is left for her to do
            s.news = l.difficulty_news.is_some() && bank.is_some_and(|b| !super::circle_links::news_settled(l, b));
            let now = self.now.unwrap_or_else(Instant::now);
            let read = self.base_reads.get(&(l.circle_id.clone(), l.circle_bank_id.clone(), l.base_version));
            s.limit_left = match read {
                Some(BaseRead::Limited(until)) if learning == Some(Learning::Limited) => Some(until.saturating_duration_since(now).as_secs().max(1)),
                _ => None,
            };
            s.unread = match read {
                Some(BaseRead::Unreadable(_, _, why)) if learning == Some(Learning::NotKept) => Some(*why),
                _ => None,
            };
        }
    }

    /// (round 15) Where the read of this link's base version stands, while the copy waits for it.
    fn learning_of(&self, l: &CircleLink) -> Option<Learning> {
        if !awaits_read(l) {
            // (round 18, critic 97 m1) a held pass for a question put back after the read waits too (Send waits for it)
            if self.inherit_held.contains_key(&l.local_bank_id) {
                return Some(Learning::Held);
            }
            // (round 16, critic 93 I1) read, but the circle's ratings are not in the bank yet: Send waits for them
            return l.inherit.then_some(Learning::Checking);
        }
        let key = (l.circle_id.clone(), l.circle_bank_id.clone(), l.base_version);
        if self.quiet_read_of(&key) {
            return Some(Learning::Checking);
        }
        Some(match self.base_reads.get(&key) {
            None => Learning::Checking,
            // (round 16, critic 94 m2) one failure is not said yet: it is asked again within seconds
            Some(BaseRead::NotReached(_, tries)) => if *tries <= 1 { Learning::Checking } else { Learning::CantReach },
            Some(BaseRead::Unreadable(..)) => Learning::NotKept,
            Some(BaseRead::Limited(..)) => Learning::Limited,
        })
    }
}

/// (critic 80 I2, critic 86 minor) The send kept in `sent_record` went through (the person's own version came right after
/// the one it was based on): as if its answer had come but could not be read.
fn went_through(l: &mut CircleLink) -> bool {
    let Some(r) = &l.sent_record else { return false };
    l.sent_unread = Some(SentUnread { by: r.by.clone(), modified: r.modified });
    true
}

/// What the bank's questions are right now, to notice an edit made while a review was open.
fn print_of(bank: &QuestionBank) -> String {
    bank_print(bank)
}

/// (round 18, critic 97 m4) What all the banks are right now: a safety copy of them is made again once any of them changed.
fn print_all(banks: &[QuestionBank]) -> String {
    sha256_hex(&serde_json::to_vec(banks).unwrap_or_default())
}

/// (round 17, critic 96 I1) The difficulty news on `l` after a pass of the read of `version`: each question it moved, before ->
/// after (one already listed takes the new "after", and is no longer put back or changed since).
fn add_news(l: &mut CircleLink, version: u64, moved: &[(String, Option<crate::model::Difficulty>, Option<crate::model::Difficulty>)]) {
    if moved.is_empty() {
        return;
    }
    let news = l.difficulty_news.get_or_insert_with(Default::default);
    news.version = version;
    for (id, was, now) in moved {
        match news.questions.iter_mut().find(|n| &n.id == id) {
            Some(n) => {
                n.after = *now;
                n.put_back = false;
                n.changed = false;
            }
            None => news.questions.push(NewsItem { id: id.clone(), before: *was, after: *now, put_back: false, changed: false }),
        }
    }
}

/// (round 19, critic 99 m2) Questions a pass gave the circle's difficulty, that she put back since (each her difficulty, then the
/// circle's): listed as put back.
fn add_news_back(l: &mut CircleLink, version: u64, back: &[(String, Option<crate::model::Difficulty>, Option<crate::model::Difficulty>)]) {
    if back.is_empty() {
        return;
    }
    let news = l.difficulty_news.get_or_insert_with(Default::default);
    news.version = version;
    for (id, mine, circle) in back {
        match news.questions.iter_mut().find(|n| &n.id == id) {
            Some(n) => n.put_back = true,
            None => news.questions.push(NewsItem { id: id.clone(), before: *mine, after: *circle, put_back: true, changed: false }),
        }
    }
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
            "This version is held back while an administrator reviews it: something in it matched what verSHion checks for, so nobody can open it until an administrator releases it. Change the questions that may be the cause and send a clean version.".to_string()
        } else {
            "This version is held back while an administrator reviews it: something in it matched what verSHion checks for, so nobody can open it until an administrator releases it. Only the bank's author or the circle's owner can send a clean version. Ask one of them.".to_string()
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

/// (critic 75 minor 3) An answer to a send that does not say whether it went through: none came (offline, a timeout
/// after the request left), it could not be read, or the server failed on its side. The words why; `None` for an
/// answer that says the send was refused (nothing was stored).
fn unclear_why(e: &PoolError) -> Option<String> {
    match e {
        PoolError::Api(ApiError::Offline(why)) => Some(why.trim().to_string()),
        PoolError::Api(ApiError::BadReply(_) | ApiError::TooLarge(_)) => Some("The circle's answer could not be read.".into()),
        PoolError::Refused(r) if matches!(r.detail.status, 500 | 502 | 504) && matches!(r.kind, RefusalKind::Other) => Some("verSHion's server had a problem on its side.".into()),
        _ => None,
    }
}

/// The same on the Send page: Try again sends the same changes again, which the circle never takes twice.
fn unclear_page(why: &str) -> String {
    format!("Your send may have gone through, but no clear answer came: {why} Press Try again: if it went through, nothing is sent twice. Your changes are still on this computer.")
}

/// (critic 78 I1) The bank's question and the circle's have the same picture (or neither has one).
/// (critic 80 I2) Whether `by` names the person whose display name is `me` (see `CircleEditModel::sender`).
fn sender_of(me: Option<&str>, by: Option<&str>) -> Who {
    let name = |s: &str| api::plain(s).trim().to_string();
    let (Some(by), Some(me)) = (by.map(name).filter(|s| !s.is_empty()), me.map(name).filter(|s| !s.is_empty())) else { return Who::Unknown };
    if by == me { Who::Me } else { Who::Other(by) }
}

/// (critic 81 I2) Who sent the version right after `base`, as the merge-initiate answer lists the versions after the copy's
/// (the first 50, oldest first: that one is always among them).
fn next_sender(base: u64, start: &MergeStart) -> Option<&str> {
    let next = base.checked_add(1)?;
    start.between.iter().find(|l| l.version == next).map(|l| l.by.as_str())
}

pub fn same_picture(q: &crate::model::Question, t: &merge::Theirs) -> bool {
    match (&q.image, &t.sha) {
        (None, None) => true,
        (Some(i), Some(sha)) => sha_of_file(&i.file).as_deref() == Some(sha.as_str()),
        _ => false,
    }
}

/// (critic 77 I2) A restore with no clear answer, on the History: Try again goes as the same restore.
pub fn unclear_restore_page(why: &str) -> String {
    format!("Your restore may have gone through, but no clear answer came: {why} Press Try again: if it went through, nothing is restored twice.")
}

/// (critic 77 I2) The same for a trim; (critic 79 minor 6) exactly what trying again does: it keeps the same newest
/// versions, so after a trim that went through it deletes only what fell out of them since.
pub fn unclear_trim_page(why: &str) -> String {
    format!("Your trim may have gone through, but no clear answer came: {why} Press Try again to make sure. If it went through, trying again deletes nothing more, unless newer versions were sent since.")
}

/// (critic 78 I1) What the check after a send with no clear answer found: a newer version (it may be the send).
fn unclear_moved(head: u64) -> String {
    format!("The check found a newer version in the circle, version {head}. It may be the one you sent: press Get latest to see. Your changes are still on this computer.")
}

/// (critic 78 I1) The same, when the circle still has the version the send was based on.
fn unclear_still(head: u64) -> String {
    // (critic 81 minor 5) not "did not go through": a slow commit may land after the check
    format!("Your send may not have arrived yet: the circle still has version {head}. Your changes are still on this computer; press Send my changes to send them.")
}

/// (critic 80 I2) The same, when a colleague sent the version right after the one the send was based on.
fn unclear_other_first(who: &str, head: u64) -> String {
    format!("{who} sent version {head} first, so your send did not go through. Your changes are still on this computer: get the latest, then send them.")
}

/// (critic 80 I2) The same, when the version right after it is the person's own.
fn unclear_mine(head: u64) -> String {
    format!("The check found version {head} in the circle, sent by you: your send went through. Press Get latest to catch up.")
}

/// (critic 78 I2) A conflict with the person's own earlier send (`went`: it went through but its answer could not be
/// read), or one that may be (no clear answer came), said on the strip: never "someone sent".
fn own_conflict_strip(went: bool, head: Option<u64>) -> String {
    match (went, head) {
        (true, Some(h)) => format!("Your changes were not sent: your earlier send went through, so the circle is ahead of your copy (version {h}). Get the latest, then send what is left."),
        (true, None) => "Your changes were not sent: your earlier send went through, so the circle is ahead of your copy. Get the latest, then send what is left.".into(),
        (false, Some(h)) => format!("Your changes were not sent: the circle has version {h}, which may be your earlier send. Get the latest, then send what is left."),
        (false, None) => "Your changes were not sent: the circle has a newer version, which may be your earlier send. Get the latest, then send what is left.".into(),
    }
}

/// (critic 78 I2) The conflict box on the Send page for the same: the box's words and what to do.
pub fn own_conflict_page(went: bool) -> (&'static str, &'static str) {
    if went {
        ("Your earlier send went through, so the circle is ahead of your copy: nothing was sent and nothing was overwritten.", "Get the latest to catch up, then send what is left.")
    } else {
        ("The circle has a newer version than your copy, so nothing was sent and nothing was overwritten. It may be your earlier send.", "Get the latest to catch up, then send what is left. You decide what happens to every question that was changed on both sides.")
    }
}

/// (critic 78 I2) The Send page of a bank whose earlier send went through but whose answer could not be read.
pub const SEND_PAGE_EARLIER: &str = "Your earlier send went through, so these counts still include its changes until you get the latest. Get the latest first, then send what is left.";

/// The same on the bank's strip, which checks the circle at once.
fn unclear_strip(why: &str) -> String {
    format!("Your send may have gone through, but no clear answer came: {why} Your changes are still on this computer; the circle is checked again to see where it stands.")
}

/// (critic 73 I1) A send that did not go through, said on the strip: the reason, and that the changes are kept.
fn not_sent(m: &str) -> String {
    let m = m.trim_end();
    if m.contains("still on this") { format!("Your changes were not sent: {m}") } else { format!("Your changes were not sent: {m} They are still on this computer.") }
}

/// The lines under "Sent": what happened, in plain words. `can_fix`: the owner, or the bank's author.
pub fn sent_lines(s: &Sent, can_fix: bool) -> Vec<String> {
    // (critic 74 I1) an answer with no version the app could read: never "Version 0 ... 0 added"
    let first = if s.version == 0 { SENT_UNREAD_PAGE.to_string() } else { format!("Version {} is now the circle's newest. {} added, {} changed, {} removed.", s.version, s.added, s.changed, s.removed) };
    let mut out = vec![first];
    if s.replayed {
        out.push("This send had already gone through; nothing was sent twice.".into());
    }
    if let Some(m) = s.merged_from {
        out.push(format!("It records that you took in the changes made since version {m}."));
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
        // (critic 76 I2) never "version 0": an answer that named no version the app can read
        let made = if version == 0 { "A new version".to_string() } else { format!("Version {version}") };
        let mut out = vec![format!("{made} was made from version {to}, but it is not shown to the circle yet.")];
        // A restore sends nothing from this computer: "take it out and send again" would point at the
        // wrong questions, so the advice names what a restore can do instead.
        if withheld && can_fix {
            out.push(format!("It is held back while an administrator reviews it: something in version {to} matched what verSHion checks for, so nobody can open the new version until an administrator releases it. To give the circle a clean newest version, restore a different version, or send a clean one from the bank in My Banks."));
            out.extend(hidden_lines(held, false, can_fix));
            return out;
        }
        out.extend(hidden_lines(held, withheld, can_fix));
        return out;
    }
    let first = if version == 0 {
        // (critic 76 I2) restored, but the answer named no version the app can read
        format!("Done. Version {to} was restored as the circle's newest version, but the circle's answer could not be read, so the new version's number is not known here.")
    } else {
        format!("Done. Version {version} is now the circle's newest, with the content of version {to}.")
    };
    vec![first, "Everyone who has the bank in My Banks sees it as a new version, and gets it when they choose.".into()]
}

/// (critic 69 I1) How a restore went, for the bank's strip when its History is no longer shown (`None`: nothing to
/// say yet).
fn restore_note(to: u64, stage: Option<&RestoreStage>) -> Option<String> {
    match stage? {
        RestoreStage::Done { version, held, withheld } if *held || *withheld => restore_lines(*version, to, *held, *withheld, true).into_iter().next(),
        // (critic 72 I2) no "it is now the circle's newest": the strip's own line says which version is the newest
        // (critic 76 I2) no "version 0": the strip checks the circle at once
        RestoreStage::Done { version: 0, .. } => Some(format!("Version {to} was restored, but the circle's answer could not be read. Open History to see the new version.")),
        RestoreStage::Done { version, .. } => Some(format!("Version {version} was restored from version {to}.")),
        RestoreStage::Large { removed, total } => Some(format!("Version {to} was not restored: it would remove {removed} of the bank's {total} questions. Open History to restore it anyway.")),
        RestoreStage::Failed(p) => Some(format!("The restore of version {to} did not go through: {} Open History to try again.", p.message.trim_end())),
        RestoreStage::Unclear(why) => Some(format!("Your restore of version {to} may have gone through, but no clear answer came: {why} Open History to see whether it did.")),
        RestoreStage::Confirm | RestoreStage::Working => None,
    }
}

/// (critic 71 I3) A restore refused because someone sent a newer version first, said where no History was reloaded.
fn restore_conflict_note(to: u64, head: Option<u64>) -> String {
    match head {
        Some(h) => format!("The restore of version {to} did not go through: someone sent version {h} first. Open History to choose again."),
        None => format!("The restore of version {to} did not go through: someone sent a newer version first. Open History to choose again."),
    }
}

/// (critic 69 I1) How a trim went, for the bank's strip when its History is no longer shown; (critic 71 minor 7) with
/// the versions it could not delete, kept for an administrator's review.
fn trim_note(stage: Option<&TrimStage>) -> Option<String> {
    match stage? {
        TrimStage::Done(t) => {
            let first = match t.pruned {
                0 => "The trim deleted nothing: no older version could be trimmed.".to_string(),
                1 => "The trim deleted the content of 1 older version.".to_string(),
                n => format!("The trim deleted the content of {n} older versions."),
            };
            Some(match trim_lines(t).get(1) {
                Some(kept) => format!("{first} {kept}"),
                None => first,
            })
        }
        TrimStage::Failed(p) => Some(format!("The trim of old versions did not go through: {} Open History to try again.", p.message.trim_end())),
        TrimStage::Unclear(why) => Some(format!("Your trim of old versions may have gone through, but no clear answer came: {why} Open History to see whether it did.")),
        TrimStage::Editing | TrimStage::Confirm | TrimStage::Working => None,
    }
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
/// (phase D3) For each question of a composed payload, its number in the bank (`None`: a restored removal, which the
/// bank no longer has). The circle's refusals name payload positions; the page names the bank's own questions, which a
/// restored removal would otherwise shift.
fn bank_positions(composed: &[ComposedQuestion], bank: &QuestionBank) -> Vec<Option<usize>> {
    composed.iter().map(|c| bank.questions.iter().position(|q| q.id == c.local)).collect()
}

/// (phase D3) Payload positions as the bank's question numbers, where the composed question is in the bank.
fn bank_numbers(positions: &[usize], map: &[Option<usize>]) -> Vec<usize> {
    positions.iter().map(|p| map.get(*p).copied().flatten().unwrap_or(*p)).collect()
}

/// (phase D3) A payload refusal's question numbers as the bank's, for the page only: `decisions_from` and the request
/// keep the payload positions the pool names. Where the payload position has no bank question, its payload number
/// stays (never a wrong bank number).
fn refusal_for_display(mut r: PayloadRefusal, map: &[Option<usize>]) -> PayloadRefusal {
    fn shown(map: &[Option<usize>], payload: usize) -> usize {
        map.get(payload).copied().flatten().unwrap_or(payload)
    }
    fn path_question(path: &str) -> Option<usize> {
        if path.starts_with("question") && !path.starts_with("questions") {
            return Some(0);
        }
        let rest = path.split("questions[").nth(1).or_else(|| path.split("question=").nth(1))?;
        rest.chars().take_while(char::is_ascii_digit).collect::<String>().parse().ok()
    }
    fn path_for(path: &str, map: &[Option<usize>]) -> String {
        let Some(q) = path_question(path) else { return path.to_string() };
        let b = shown(map, q);
        if b == q {
            return path.to_string();
        }
        path.replace(&format!("questions[{q}]"), &format!("questions[{b}]")).replace(&format!("question={q}"), &format!("question={b}"))
    }
    for i in &mut r.issues {
        i.path = path_for(&i.path, map);
    }
    for f in &mut r.formatting_findings {
        f.question = shown(map, f.question);
        f.path = path_for(&f.path, map);
    }
    for f in &mut r.label_findings {
        f.question = shown(map, f.question);
    }
    for p in &mut r.needs_order_ack {
        p.question = shown(map, p.question);
    }
    for o in &mut r.order_reasons {
        o.question = shown(map, o.question);
    }
    for o in &mut r.order_preview {
        o.question = shown(map, o.question);
    }
    r
}

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
        Problem { message: super::circles_text::circle_words(&PoolError::Local(l).message()), retry: false, retry_at: None, account: false }
    }
}
