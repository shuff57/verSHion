//! The publish flow's logic ("Share to pool..."): which step shows, the options form, the
//! mandatory "this is what students will see" preview, the refusal loop, progress, cancel and the
//! result. No egui, so tests drive every path with a scripted session.
//!
//! # Steps
//!
//! ```text
//! Gate --(signed in and approved)--> Options --check--> Checking --ok--> Review --publish--> Publishing --ok--> Done
//!                                       ^                  |                |                    |
//!                                       |                  +--refused-------+--refused-----------+--> Fix (decide / acknowledge / change)
//!                                       +------------------------------------------------------------recheck--> Checking
//!                          Failed (offline, a limit, the server is busy, ...): retry, or back to Options
//! ```
//!
//! Rules this module keeps:
//!
//! - Nothing is sent unless the window is open and the person pressed a button; a closed model does
//!   no I/O at all (`frame` returns at once).
//! - Publishing is impossible without the own-work box ticked and a preview of exactly these
//!   options that was shown and looked at. Changing any option (or the target) makes the preview
//!   stale.
//! - An acknowledgement can only be given for something the server asked about (a question it
//!   flagged, a position it listed); the app never acknowledges on its own.
//! - A cancelled or lost publish is never reported as "not published": the same publish id is kept,
//!   so sending again is the same publish (the server answers from its record).
//! - Server-supplied text is drawn after [`api::plain`]; this model keeps it raw, the drawing code
//!   (and `publish_text`) cleans it. Nothing here logs a token, a password or the author-only
//!   served orders.

use std::collections::{BTreeMap, BTreeSet};
use std::sync::mpsc::{Receiver, TryRecvError, channel};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use serde_json::Value;
use web_time::Instant;

use super::api::{self, ApiError, Status};
use super::model::{AccountModel, Notice, Page, Runner, ServerMode};
use super::pool::{
    BlobOrder, ImageUse, IndependentOf, ItemMeta, License, LabelChoice, LicenseRefusal, MediaSource, PayloadRefusal, Pool, PoolError,
    PoolSession, Pos, PreviewResult, PublishOptions, PublishProgress, PublishRequest, PublishResult, PublishTarget, RefusalKind, RetryPolicy,
    Served, StudentView, StoreMedia, Visibility, images_in_payload, is_item_id,
};
use crate::model::{Exam, QuestionBank};

pub type SharedSession = Arc<dyn PoolSession + Send + Sync>;
pub type SharedMedia = Arc<dyn MediaSource + Send + Sync>;

const TITLE_MAX: usize = 200;
const META_MAX: usize = 100;
/// Schema 1 for a new item (what the app writes today); a republish needs 2.
const SCHEMA_NEW: u32 = 1;
const SCHEMA_REPUBLISH: u32 = 2;

// ---- what is being shared -----------------------------------------------------------------------

/// One question of the source, for lists ("Question 3: Which planet ...").
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct QuestionInfo {
    /// 1-based.
    pub number: usize,
    pub stem: String,
    pub kind: String,
    /// `media/<sha>.<ext>` when the question has a picture.
    pub image: Option<String>,
}

/// A bank or an exam, frozen when "Share to pool..." was chosen. The payload is made here from the
/// app's own model; the keyless / origin / rekeyed fields the Worker also knows are not in the
/// model yet (the deferred patch), and the Worker treats them as absent.
#[derive(Clone, Debug)]
pub struct ShareSource {
    /// `bank:<id>` or `exam:<id>`: what a previous publish is remembered under.
    pub key: String,
    /// "Bank: Unit 3" for the window's heading.
    pub label: String,
    /// The suggested listing title.
    pub title: String,
    pub questions: Vec<QuestionInfo>,
    /// Questions of an exam that could not be found in any bank (not shared).
    pub missing: usize,
    bank_json: String,
    /// The local id of each question, in order (for `PublishedRecord::qids`).
    question_ids: Vec<String>,
}

impl ShareSource {
    pub fn from_bank(bank: &QuestionBank) -> Result<ShareSource, String> {
        if bank.questions.is_empty() {
            return Err("Add a question first: there is nothing to share yet.".into());
        }
        ShareSource::build(format!("bank:{}", bank.id), format!("Bank: {}", bank.name), bank, 0)
    }

    /// An exam is shared as a bank of its questions, in exam order, named like the exam. The
    /// exam's layout (sections, spacing, formatting) is not shared; the questions are.
    pub fn from_exam(exam: &Exam, banks: &[QuestionBank]) -> Result<ShareSource, String> {
        let mut questions = Vec::new();
        let mut missing = 0;
        for it in &exam.items {
            match banks.iter().find(|b| b.id == it.bank_id).and_then(|b| b.questions.iter().find(|q| q.id == it.question_id)) {
                Some(q) => questions.push(q.clone()),
                None => missing += 1,
            }
        }
        if questions.is_empty() {
            return Err("This exam has no questions to share yet.".into());
        }
        let bank = QuestionBank { id: exam.id.clone(), name: exam.name.clone(), questions, created: exam.modified, modified: exam.modified, details: Default::default() };
        ShareSource::build(format!("exam:{}", exam.id), format!("Exam: {}", exam.name), &bank, missing)
    }

    fn build(key: String, label: String, bank: &QuestionBank, missing: usize) -> Result<ShareSource, String> {
        let v = serde_json::to_value(bank).map_err(|e| e.to_string())?;
        let questions = v
            .get("questions")
            .and_then(Value::as_array)
            .map(|qs| {
                qs.iter()
                    .enumerate()
                    .map(|(i, q)| QuestionInfo {
                        number: i + 1,
                        stem: q.get("stem").and_then(Value::as_str).unwrap_or("").lines().next().unwrap_or("").chars().take(90).collect(),
                        kind: q.get("kind").and_then(Value::as_object).and_then(|o| o.keys().next().cloned()).unwrap_or_default(),
                        image: q.pointer("/image/file").and_then(Value::as_str).map(str::to_string),
                    })
                    .collect()
            })
            .unwrap_or_default();
        let question_ids = ids_of(&v);
        Ok(ShareSource { key, label, title: bank.name.chars().take(TITLE_MAX).collect(), questions, missing, bank_json: v.to_string(), question_ids })
    }

    /// A source from ready-made bank JSON (tests, other callers).
    pub fn from_bank_json(key: &str, label: &str, title: &str, bank_json: &str) -> ShareSource {
        let v: Value = serde_json::from_str(bank_json).unwrap_or(Value::Null);
        let questions = v
            .get("questions")
            .and_then(Value::as_array)
            .map(|qs| {
                qs.iter()
                    .enumerate()
                    .map(|(i, q)| QuestionInfo {
                        number: i + 1,
                        stem: q.get("stem").and_then(Value::as_str).unwrap_or("").lines().next().unwrap_or("").chars().take(90).collect(),
                        kind: q.get("kind").and_then(Value::as_object).and_then(|o| o.keys().next().cloned()).unwrap_or_default(),
                        image: q.pointer("/image/file").and_then(Value::as_str).map(str::to_string),
                    })
                    .collect()
            })
            .unwrap_or_default();
        ShareSource { key: key.into(), label: label.into(), title: title.into(), questions, missing: 0, bank_json: bank_json.into(), question_ids: ids_of(&v) }
    }

    /// `{"schema_version": n, "bank": ...}`.
    pub fn payload(&self, schema: u32) -> Vec<u8> {
        format!("{{\"schema_version\":{schema},\"bank\":{}}}", self.bank_json).into_bytes()
    }

    /// Like [`ShareSource::payload`], and for a republish over `link` each question that was shared
    /// before carries `published {item_id, qid, publish_id, version}` so the Worker keeps its id.
    /// A question with no link (new, or from another bank) is sent without one and gets a new id.
    pub fn payload_linked(&self, schema: u32, link: Option<&PublishedRecord>) -> Vec<u8> {
        let Some(rec) = link.filter(|r| !r.qids.is_empty() && schema >= SCHEMA_REPUBLISH) else { return self.payload(schema) };
        let Ok(mut v) = serde_json::from_str::<Value>(&self.bank_json) else { return self.payload(schema) };
        if let Some(qs) = v.get_mut("questions").and_then(Value::as_array_mut) {
            for q in qs {
                let local = q.get("id").and_then(Value::as_str).unwrap_or("");
                if let Some(l) = rec.qids.iter().find(|l| l.local == local && is_item_id(&l.qid))
                    && let Some(o) = q.as_object_mut()
                {
                    o.insert(
                        "published".into(),
                        serde_json::json!({ "item_id": rec.item_id, "qid": l.qid, "publish_id": rec.publish_id, "version": rec.version }),
                    );
                }
            }
        }
        format!("{{\"schema_version\":{schema},\"bank\":{v}}}").into_bytes()
    }

    /// The links to remember after a publish: the server's `qids`, in order, against the local ids.
    /// Nothing when the counts differ (then the next republish simply gets new ids).
    pub fn links_for(&self, qids: &[String]) -> Vec<QidLink> {
        if qids.len() != self.question_ids.len() {
            return Vec::new();
        }
        self.question_ids.iter().zip(qids).map(|(local, qid)| QidLink { local: local.clone(), qid: qid.clone() }).collect()
    }

    /// The question text of a 0-based index, for lists.
    pub fn question(&self, index: usize) -> Option<&QuestionInfo> {
        self.questions.get(index)
    }

}

// ---- what a previous publish left behind ---------------------------------------------------------

/// Where the last publish of a source went, so the next one can be a republish.
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct PublishedRecord {
    pub key: String,
    pub item_id: String,
    /// The publish this version is: the next republish's `base_publish_id`.
    pub publish_id: String,
    pub version: u64,
    pub visibility: String,
    pub license: String,
    pub title: String,
    /// The pool's id for each question as of this version, by the local question id. A republish
    /// sends it back (`published.qid`) so an unchanged question keeps its identity: importers'
    /// update checks and merges match by it, and the Worker keeps the served form of a hidden-key
    /// item byte for byte when nothing a student sees changed. Ids only: no text, no keys.
    #[serde(default)]
    pub qids: Vec<QidLink>,
}

/// A local question id and the pool's id for it.
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct QidLink {
    pub local: String,
    pub qid: String,
}

#[derive(Clone, Debug, Default, serde::Serialize, serde::Deserialize)]
struct LogFile {
    #[serde(default)]
    items: Vec<PublishedRecord>,
}

pub const LOG_DOC: &str = "share-published.json";

/// Where the records are kept: the app's store, or a shared value (tests).
#[derive(Clone, Debug)]
pub enum LogIo {
    Store,
    Memory(Arc<Mutex<Vec<PublishedRecord>>>),
}

impl LogIo {
    pub fn memory() -> LogIo {
        LogIo::Memory(Arc::default())
    }

    fn load(&self) -> Vec<PublishedRecord> {
        match self {
            LogIo::Store => crate::store::load_json::<LogFile>(LOG_DOC).map(|f| f.items).unwrap_or_default(),
            LogIo::Memory(m) => m.lock().unwrap_or_else(|p| p.into_inner()).clone(),
        }
    }

    fn save(&self, items: &[PublishedRecord]) -> Result<(), String> {
        match self {
            LogIo::Store => crate::store::save_json(LOG_DOC, &LogFile { items: items.to_vec() }),
            LogIo::Memory(m) => {
                *m.lock().unwrap_or_else(|p| p.into_inner()) = items.to_vec();
                Ok(())
            }
        }
    }

    /// Every record (the list of what this device shared).
    pub fn load_all(&self) -> Vec<PublishedRecord> {
        self.load().into_iter().filter(|r| is_item_id(&r.item_id)).collect()
    }

    pub fn find(&self, key: &str) -> Option<PublishedRecord> {
        self.load().into_iter().find(|r| r.key == key && is_item_id(&r.item_id))
    }

    pub fn put(&self, rec: PublishedRecord) -> Result<(), String> {
        let mut items = self.load();
        items.retain(|r| r.key != rec.key);
        items.push(rec);
        self.save(&items)
    }

    pub fn forget(&self, key: &str) -> Result<(), String> {
        let mut items = self.load();
        items.retain(|r| r.key != key);
        self.save(&items)
    }
}

// ---- the account gate -------------------------------------------------------------------------------

/// Whether the account can publish right now.
#[derive(Clone, Debug, PartialEq)]
pub enum Gate {
    /// The account is being looked up.
    Checking,
    /// A release build made without a pool server.
    NotSetUp,
    /// A development build with no server address yet.
    NoServer,
    SignedOut,
    /// The sign-in could not be checked (offline?).
    Unreachable,
    /// Signed in, but not (or no longer) approved to share.
    NotApproved(Status),
    Ready,
}

/// What the publish flow needs of the account, every frame.
#[derive(Clone)]
pub struct AccountSnapshot {
    pub gate: Gate,
    pub session: Option<SharedSession>,
    /// The Account window is open (shown or quiet).
    pub account_open: bool,
}

impl AccountSnapshot {
    pub fn ready(session: SharedSession) -> AccountSnapshot {
        AccountSnapshot { gate: Gate::Ready, session: Some(session), account_open: false }
    }
    pub fn of_gate(gate: Gate) -> AccountSnapshot {
        AccountSnapshot { gate, session: None, account_open: false }
    }
}

/// Reads the Account window's state. Does no I/O.
pub fn snapshot_of(m: &AccountModel) -> AccountSnapshot {
    let session: Option<SharedSession> = m.client().map(|c| c.clone() as SharedSession);
    let gate = if *m.server_mode() == ServerMode::NotSetUp {
        Gate::NotSetUp
    } else if let Some(me) = &m.me {
        match &me.status {
            Status::Approved if session.is_some() => Gate::Ready,
            Status::Approved => Gate::Checking,
            other => Gate::NotApproved(other.clone()),
        }
    } else if !m.open {
        Gate::Checking
    } else {
        match m.page {
            Page::NotSetUp => Gate::NotSetUp,
            Page::NotConfigured | Page::Server => Gate::NoServer,
            Page::Opening => Gate::Checking,
            Page::Status if m.busy() => Gate::Checking,
            Page::Status => Gate::Unreachable,
            _ if m.busy() => Gate::Checking,
            _ => Gate::SignedOut,
        }
    };
    AccountSnapshot { gate, session, account_open: m.open }
}

// ---- the form -----------------------------------------------------------------------------------------

/// Every field of the options page and the decisions of the refusal loop.
#[derive(Clone, Debug)]
pub struct Form {
    /// Nothing is preselected: the person chooses.
    pub license: Option<License>,
    pub visibility: Visibility,
    pub hide_key: bool,
    pub title: String,
    pub subject: String,
    pub course: String,
    pub grade: String,
    /// Comma or line separated.
    pub tags: String,
    pub standards: String,
    /// The own-work statement, ticked.
    pub attest: bool,
    /// "I looked at what students will see", on the review step.
    pub reviewed: bool,
    /// "Update the item I shared before" (when there is one) rather than a new item.
    pub update_existing: bool,
    pub claim_independent: bool,
    /// Item ids or links of items only looked at, whitespace or comma separated.
    pub independent_items: String,
    pub independent_statement: String,
    labels: BTreeMap<Pos, LabelChoice>,
    order_acks: BTreeSet<Pos>,
    ack_formatting: BTreeSet<usize>,
    keep_images: BTreeSet<usize>,
}

impl Form {
    fn new(title: &str, record: bool) -> Form {
        Form {
            license: None,
            visibility: Visibility::Public,
            hide_key: true,
            title: title.to_string(),
            subject: String::new(),
            course: String::new(),
            grade: String::new(),
            tags: String::new(),
            standards: String::new(),
            attest: false,
            reviewed: false,
            update_existing: record,
            claim_independent: false,
            independent_items: String::new(),
            independent_statement: String::new(),
            labels: BTreeMap::new(),
            order_acks: BTreeSet::new(),
            ack_formatting: BTreeSet::new(),
            keep_images: BTreeSet::new(),
        }
    }

    pub fn label(&self, at: Pos) -> Option<LabelChoice> {
        self.labels.get(&at).copied()
    }
    pub fn order_acked(&self, at: Pos) -> bool {
        self.order_acks.contains(&at)
    }
    pub fn formatting_acked(&self, question: usize) -> bool {
        self.ack_formatting.contains(&question)
    }
    pub fn image_kept(&self, question: usize) -> bool {
        self.keep_images.contains(&question)
    }
}

fn split_list(s: &str) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for part in s.split([',', '\n', ';']) {
        let t = api::plain(part);
        if !t.is_empty() && !out.contains(&t) {
            out.push(t);
        }
    }
    out
}

/// Item ids in text the person pasted: bare ids or links containing them.
pub fn item_ids_in(s: &str) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for tok in s.split(|c: char| !c.is_ascii_alphanumeric()) {
        let t = tok.to_ascii_lowercase();
        if is_item_id(&t) && !out.contains(&t) {
            out.push(t);
        }
    }
    out
}

// ---- steps and what they hold ---------------------------------------------------------------------------

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Step {
    /// Waiting for a signed-in, approved account.
    Gate,
    Options,
    /// The preview is being made.
    Checking,
    /// "This is what students will see".
    Review,
    /// The server refused something the person can decide or change.
    Fix,
    Publishing,
    Done,
    Failed,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RetryWhat {
    Preview,
    Publish,
}

/// Where this publish goes.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Target {
    New,
    Republish { item_id: String, base_publish_id: String, version: u64 },
}

/// A picture of the source and whether students get it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ImageNote {
    /// 0-based question index.
    pub question: usize,
    pub file: String,
    pub kept: bool,
}

/// The finished preview: the request it is for, and what students will see.
#[derive(Clone, Debug)]
pub struct Preview {
    pub result: PreviewResult,
    pub view: StudentView,
    pub options: PublishOptions,
    pub target: Target,
    pub images: Vec<ImageNote>,
    /// Why this can not be published, if the preview shows something wrong (key material in a
    /// hidden-key form).
    pub blocked: Option<String>,
}

/// What the server refused, as the loop shows it.
#[derive(Clone, Debug)]
pub enum Fix {
    Payload(Box<PayloadRefusal>),
    License(Box<LicenseRefusal>),
    /// Someone republished first. `current_publish_id` is the version now on the server.
    Conflict { current_version: Option<u64>, current_publish_id: Option<String> },
    /// The item was deleted for good (or can not be republished over): publish as a new item.
    Gone { message: String },
}

/// What is left to do in a payload refusal.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Todo {
    /// Findings that must be changed in the question (they can not be acknowledged).
    pub hard: usize,
    pub formatting: usize,
    pub labels: usize,
    pub orders: usize,
}

impl Todo {
    pub fn done(&self) -> bool {
        *self == Todo::default()
    }
}

#[derive(Clone, Debug)]
pub struct Failure {
    pub message: String,
    pub retry: Option<RetryWhat>,
    /// The server asked for a wait; retry is disabled until then.
    pub retry_at: Option<Instant>,
    /// The Account window can help (sign in, approval).
    pub account: bool,
    /// The publish may have reached the server; sending again is the same publish.
    pub maybe_published: bool,
}

#[derive(Clone, Debug)]
pub struct Done {
    pub result: PublishResult,
    pub republish: bool,
    pub title: String,
}

impl Done {
    pub fn public(&self) -> bool {
        self.result.visibility == "public"
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum JobKind {
    Preview,
    Publish,
}

enum JobMsg {
    Progress(PublishProgress),
    Preview(Result<PreviewResult, PoolError>),
    Published(Result<PublishResult, PoolError>),
}

struct Job {
    kind: JobKind,
    rx: Receiver<JobMsg>,
}

/// What the last request was built from; reused while nothing changed.
#[derive(Clone, Debug)]
struct Prepared {
    options: PublishOptions,
    target: Target,
    publish_id: String,
}

pub struct PublishModel {
    pub open: bool,
    pub step: Step,
    pub source: Option<ShareSource>,
    pub form: Form,
    pub notice: Option<Notice>,
    pub preview: Option<Preview>,
    pub fix: Option<Fix>,
    pub failure: Option<Failure>,
    pub done: Option<Done>,
    pub progress: Option<PublishProgress>,
    pub gate: Gate,
    /// The Account window should be shown (the person pressed "Open account").
    open_account: bool,
    /// The Account window was asked to look the sign-in up in the background.
    asked_account: bool,
    record: Option<PublishedRecord>,
    target: Target,
    prepared: Option<Prepared>,
    /// A check already got the pictures to the server (privately, for a day): later requests only
    /// send them again if the server says one is missing.
    images_sent: bool,
    resume: Step,
    session: Option<SharedSession>,
    media: SharedMedia,
    runner: Runner,
    retry: RetryPolicy,
    job: Option<Job>,
    log: LogIo,
    now: Option<Instant>,
    repaint: Option<Arc<dyn Fn() + Send + Sync>>,
}

impl Default for PublishModel {
    fn default() -> Self {
        PublishModel::new(Runner::Background, Arc::new(StoreMedia), LogIo::Store)
    }
}

impl PublishModel {
    /// Does no I/O: nothing is read or sent until a source is started and the window is open.
    pub fn new(runner: Runner, media: SharedMedia, log: LogIo) -> PublishModel {
        PublishModel {
            open: false,
            step: Step::Gate,
            source: None,
            form: Form::new("", false),
            notice: None,
            preview: None,
            fix: None,
            failure: None,
            done: None,
            progress: None,
            gate: Gate::Checking,
            open_account: false,
            asked_account: false,
            record: None,
            target: Target::New,
            prepared: None,
            images_sent: false,
            resume: Step::Options,
            session: None,
            media,
            runner,
            retry: RetryPolicy::default(),
            job: None,
            log,
            now: None,
            repaint: None,
        }
    }

    pub fn with_retry(mut self, retry: RetryPolicy) -> PublishModel {
        self.retry = retry;
        self
    }

    pub fn with_runner(mut self, runner: Runner) -> PublishModel {
        self.runner = runner;
        self
    }

    pub fn set_repaint(&mut self, f: Arc<dyn Fn() + Send + Sync>) {
        self.repaint = Some(f);
    }

    pub fn busy(&self) -> bool {
        self.job.is_some()
    }

    /// Opens the window for a bank or exam. Reads the record of an earlier publish of it (one small
    /// file) so the choice "update it or share a new copy" can be offered.
    pub fn start(&mut self, source: ShareSource) {
        self.cancel_job();
        let record = self.log.find(&source.key);
        self.form = Form::new(&source.title, record.is_some());
        if let Some(r) = &record {
            self.form.license = License::parse(&r.license);
            self.form.visibility = if r.visibility == "public" { Visibility::Public } else { Visibility::Unlisted };
            if !r.title.is_empty() {
                self.form.title = r.title.clone();
            }
        }
        self.record = record;
        self.source = Some(source);
        self.target = self.target_for_form();
        self.notice = None;
        self.preview = None;
        self.fix = None;
        self.failure = None;
        self.done = None;
        self.progress = None;
        self.prepared = None;
        self.images_sent = false;
        self.open_account = false;
        self.asked_account = false;
        self.resume = Step::Options;
        self.step = Step::Gate;
        self.open = true;
    }

    pub fn close(&mut self) {
        self.open = false;
        self.cancel_job();
        self.open_account = false;
    }

    /// The record of an earlier publish of this source, if any.
    pub fn record(&self) -> Option<&PublishedRecord> {
        self.record.as_ref()
    }

    pub fn target(&self) -> &Target {
        &self.target
    }

    fn target_for_form(&self) -> Target {
        match (&self.record, self.form.update_existing) {
            (Some(r), true) => Target::Republish { item_id: r.item_id.clone(), base_publish_id: r.publish_id.clone(), version: r.version },
            _ => Target::New,
        }
    }

    /// The Account window should be looked up quietly in the background (once).
    pub fn wants_account_check(&self) -> bool {
        self.open && self.step == Step::Gate && self.gate == Gate::Checking && !self.asked_account
    }

    /// Called by the drawing code once it has asked the Account window for the check.
    pub fn account_check_asked(&mut self) {
        self.asked_account = true;
    }

    /// The person pressed "Open account": true once, then false.
    pub fn take_open_account(&mut self) -> bool {
        std::mem::take(&mut self.open_account)
    }

    pub fn request_open_account(&mut self) {
        self.open_account = true;
    }

    // ---- frame ------------------------------------------------------------------------------------------

    /// Applies finished jobs and the account's state. Does nothing when closed.
    pub fn frame(&mut self, now: Instant, snap: &AccountSnapshot) {
        if !self.open {
            return;
        }
        self.now = Some(now);
        self.apply_snapshot(snap);
        self.drain(now);
    }

    fn apply_snapshot(&mut self, snap: &AccountSnapshot) {
        self.session = snap.session.clone();
        let mut gate = snap.gate.clone();
        // The quiet look-up finished without a sign-in: that is "signed out", not "still checking".
        if gate == Gate::Checking && self.asked_account && !snap.account_open {
            gate = Gate::SignedOut;
        }
        self.gate = gate.clone();
        match self.step {
            Step::Gate => {
                if gate == Gate::Ready {
                    self.step = self.resume;
                }
            }
            Step::Done => {}
            _ => {
                let definite = matches!(gate, Gate::SignedOut | Gate::NotApproved(_) | Gate::NotSetUp | Gate::NoServer);
                if definite && self.job.is_none() {
                    self.resume = self.step;
                    self.step = Step::Gate;
                }
            }
        }
    }

    // ---- building the request --------------------------------------------------------------------------

    /// The options of the form as the server takes them, or what the person has to fix first.
    pub fn options(&self) -> Result<PublishOptions, String> {
        let f = &self.form;
        let Some(license) = f.license else {
            return Err("Choose a licence. Nothing is chosen for you.".into());
        };
        let title = f.title.trim();
        if title.is_empty() {
            return Err("Give it a title.".into());
        }
        if title.chars().count() > TITLE_MAX {
            return Err(format!("The title is too long (at most {TITLE_MAX} characters)."));
        }
        for (name, v) in [("subject", &f.subject), ("course", &f.course), ("grade", &f.grade)] {
            if v.trim().chars().count() > META_MAX {
                return Err(format!("The {name} is too long (at most {META_MAX} characters)."));
            }
        }
        let new_item = matches!(self.target_for_form(), Target::New);
        if new_item && f.visibility == Visibility::Public && license == License::CircleOnly {
            return Err("An all-rights-reserved item can not be public. Pick a Creative Commons licence, or make it unlisted.".into());
        }
        let mut o = PublishOptions::new(license, f.visibility, f.hide_key);
        o.meta = ItemMeta {
            title: title.to_string(),
            subject: f.subject.trim().to_string(),
            course: f.course.trim().to_string(),
            grade: f.grade.trim().to_string(),
            tags: split_list(&f.tags),
            standards: split_list(&f.standards),
        };
        for (at, c) in &f.labels {
            o.decide_label(*at, *c);
        }
        o.order_acks = f.order_acks.iter().copied().collect();
        o.ack_formatting = f.ack_formatting.iter().copied().collect();
        o.image_ok = if f.hide_key { f.keep_images.iter().copied().collect() } else { Vec::new() };
        if f.claim_independent {
            let items = item_ids_in(&f.independent_items);
            if items.is_empty() || items.len() > 50 {
                return Err("Name the items you only looked at (1 to 50 item ids or links).".into());
            }
            if f.independent_statement.trim().chars().count() < 10 {
                return Err("Say in a sentence why these questions are your own work (at least 10 characters).".into());
            }
            o.independent_of = Some(IndependentOf { items, statement: f.independent_statement.trim().to_string() });
        }
        Ok(o)
    }

    fn make_request(&mut self, options: PublishOptions) -> Option<PublishRequest> {
        let source = self.source.as_ref()?;
        let target = self.target_for_form();
        let schema = if matches!(target, Target::New) { SCHEMA_NEW } else { SCHEMA_REPUBLISH };
        let link = match &target {
            Target::Republish { item_id, .. } => self.record.as_ref().filter(|r| r.item_id == *item_id),
            Target::New => None,
        };
        let payload = source.payload_linked(schema, link);
        let mut req = match &target {
            Target::New => PublishRequest::new_item(payload, options.clone()),
            Target::Republish { item_id, base_publish_id, .. } => PublishRequest::republish(item_id, base_publish_id, payload, options.clone()),
        };
        // The same publish id while nothing changed, so a retry is the same publish; a new one as soon
        // as anything the server reads changed.
        match &self.prepared {
            Some(p) if p.options == options && p.target == target => req.publish_id = p.publish_id.clone(),
            _ => self.prepared = Some(Prepared { options, target: target.clone(), publish_id: req.publish_id.clone() }),
        }
        self.target = target;
        Some(req)
    }

    /// The preview shown matches the options and target now in the form.
    pub fn preview_is_current(&self) -> bool {
        match (&self.preview, self.options()) {
            (Some(p), Ok(o)) => p.options == o && p.target == self.target_for_form(),
            _ => false,
        }
    }

    /// Why the final button is disabled, or `None` when it can be pressed.
    pub fn publish_blocker(&self) -> Option<&'static str> {
        if self.busy() {
            return Some("Wait for the current step to finish.");
        }
        if self.gate != Gate::Ready || self.session.is_none() {
            return Some("Sign in with an approved account first.");
        }
        if !self.form.attest {
            return Some("Tick the box that says this is your own work.");
        }
        let Some(p) = &self.preview else { return Some("Check what students will see first.") };
        if !self.preview_is_current() {
            return Some("You changed something. Check what students will see again.");
        }
        if p.blocked.is_some() {
            return Some("The preview shows something wrong. Do not publish.");
        }
        if !self.form.reviewed {
            return Some("Tick the box to say you looked at what students will see.");
        }
        None
    }

    // ---- actions ------------------------------------------------------------------------------------------

    fn local_ready(&mut self) -> Option<(PublishOptions, SharedSession)> {
        if self.busy() {
            return None;
        }
        if self.gate != Gate::Ready {
            self.notice = Some(Notice::Error("Sign in with an approved account first.".into()));
            return None;
        }
        let Some(session) = self.session.clone() else {
            self.notice = Some(Notice::Error("Sign in with an approved account first.".into()));
            return None;
        };
        if !self.form.attest {
            self.notice = Some(Notice::Error("Tick the box that says this is your own work (or that you have the right to share it). Nothing is shared without it.".into()));
            return None;
        }
        match self.options() {
            Ok(o) => Some((o, session)),
            Err(e) => {
                self.notice = Some(Notice::Error(e));
                None
            }
        }
    }

    /// "Check what students will see": the dry run. Sends the pictures privately first when the
    /// source has any (the server needs them to check), but shares nothing.
    pub fn check(&mut self) {
        let Some((options, session)) = self.local_ready() else { return };
        let Some(req) = self.make_request(options) else { return };
        self.notice = None;
        self.failure = None;
        self.fix = None;
        self.form.reviewed = false;
        self.start_job(JobKind::Preview, req, session);
        self.step = Step::Checking;
    }

    /// The final button. Only works with the attestation ticked and a current, clean, looked-at preview.
    pub fn publish(&mut self) {
        if let Some(why) = self.publish_blocker() {
            self.notice = Some(Notice::Error(why.into()));
            return;
        }
        let Some((options, session)) = self.local_ready() else { return };
        let Some(req) = self.make_request(options) else { return };
        self.notice = None;
        self.failure = None;
        self.start_job(JobKind::Publish, req, session);
        self.step = Step::Publishing;
    }

    /// Stops waiting. A preview is simply dropped. A publish may already have reached the server:
    /// the same publish id is kept, so sending it again can not make a second item.
    pub fn cancel(&mut self) {
        let Some(job) = self.job.take() else { return };
        self.progress = None;
        match job.kind {
            JobKind::Preview => {
                self.step = if self.preview.is_some() && self.fix.is_none() { Step::Review } else { Step::Options };
                self.notice = Some(Notice::Info("Stopped. Nothing was shared.".into()));
            }
            JobKind::Publish => {
                self.failure = Some(Failure {
                    message: "You stopped waiting. The publish may already have reached the server, so it may be shared. \
                              Sending it again is safe: the server recognises it and does not make a second copy."
                        .into(),
                    retry: Some(RetryWhat::Publish),
                    retry_at: None,
                    account: false,
                    maybe_published: true,
                });
                self.step = Step::Failed;
            }
        }
    }

    fn cancel_job(&mut self) {
        self.job = None;
        self.progress = None;
    }

    /// Back to the options page (from a preview, a refusal or a failure). Keeps everything typed.
    pub fn back_to_options(&mut self) {
        if self.busy() {
            return;
        }
        self.failure = None;
        self.fix = None;
        self.notice = None;
        self.step = Step::Options;
    }

    pub fn retry(&mut self) {
        let Some(f) = self.failure.clone() else { return };
        if self.retry_wait().is_some() {
            return;
        }
        match f.retry {
            Some(RetryWhat::Preview) => self.check(),
            Some(RetryWhat::Publish) => self.resend_publish(),
            None => {}
        }
    }

    /// The same publish again (after a stop or a lost connection), with the same id.
    fn resend_publish(&mut self) {
        if self.busy() {
            return;
        }
        let Some((options, session)) = self.local_ready() else { return };
        let Some(req) = self.make_request(options) else { return };
        self.failure = None;
        self.notice = None;
        self.start_job(JobKind::Publish, req, session);
        self.step = Step::Publishing;
    }

    /// How long until retry is allowed (the server's `Retry-After`).
    pub fn retry_wait(&self) -> Option<Duration> {
        let at = self.failure.as_ref()?.retry_at?;
        let now = self.now?;
        let left = at.saturating_duration_since(now);
        (!left.is_zero()).then_some(left)
    }

    pub fn set_update_existing(&mut self, yes: bool) {
        if self.record.is_some() && self.form.update_existing != yes {
            self.form.update_existing = yes;
            self.target = self.target_for_form();
            self.form.reviewed = false;
        }
    }

    pub fn set_visibility(&mut self, v: Visibility) {
        self.form.visibility = v;
        if v == Visibility::Public && self.form.license == Some(License::CircleOnly) {
            self.form.license = None;
        }
    }

    // ---- the refusal loop -----------------------------------------------------------------------------------

    pub fn payload_fix(&self) -> Option<&PayloadRefusal> {
        match &self.fix {
            Some(Fix::Payload(p)) => Some(p),
            _ => None,
        }
    }

    /// Findings that can only be fixed in the question text.
    pub fn hard_issues(&self) -> Vec<&super::pool::Issue> {
        let Some(p) = self.payload_fix() else { return Vec::new() };
        let ackable = ackable_questions(p);
        p.issues
            .iter()
            .filter(|i| !matches!(i.code.as_str(), "order_ack_required" | "labels_need_decision"))
            .filter(|i| !(i.can_acknowledge() && i.question_index().is_some_and(|q| ackable.contains(&q))))
            .collect()
    }

    /// Questions whose low-risk findings may be acknowledged: every finding on them is low risk.
    pub fn ackable_questions(&self) -> Vec<usize> {
        self.payload_fix().map(ackable_questions).unwrap_or_default()
    }

    /// Label findings with no decision yet (server's earlier decision counts).
    pub fn undecided_labels(&self) -> Vec<Pos> {
        let Some(p) = self.payload_fix() else { return Vec::new() };
        p.label_findings
            .iter()
            .map(|f| (Pos { question: f.question, part: f.part }, f.decision))
            .filter(|(at, server)| self.form.label(*at).is_none() && server.is_none())
            .map(|(at, _)| at)
            .collect()
    }

    pub fn todo(&self) -> Todo {
        let Some(p) = self.payload_fix() else { return Todo::default() };
        Todo {
            hard: self.hard_issues().len(),
            formatting: ackable_questions(p).into_iter().filter(|q| !self.form.formatting_acked(*q)).count(),
            labels: self.undecided_labels().len(),
            orders: p.needs_order_ack.iter().filter(|a| !self.form.order_acked(**a)).count(),
        }
    }

    /// Records Strip or Keep for a finding the server listed. Anything else is ignored.
    pub fn decide_label(&mut self, at: Pos, choice: LabelChoice) {
        let Some(p) = self.payload_fix() else { return };
        let Some(f) = p.label_findings.iter().find(|f| (f.question, f.part) == (at.question, at.part)) else { return };
        if choice == LabelChoice::Strip && !f.strip_allowed {
            return;
        }
        self.form.labels.insert(at, choice);
        self.form.reviewed = false;
    }

    /// Acknowledges that students will see this choice order (only for a position the server asked about).
    pub fn set_order_ack(&mut self, at: Pos, yes: bool) {
        let Some(p) = self.payload_fix() else { return };
        if !p.needs_order_ack.contains(&at) {
            return;
        }
        if yes {
            self.form.order_acks.insert(at);
        } else {
            self.form.order_acks.remove(&at);
        }
        self.form.reviewed = false;
    }

    pub fn ack_all_orders(&mut self) {
        let Some(p) = self.payload_fix() else { return };
        let all: Vec<Pos> = p.needs_order_ack.clone();
        self.form.order_acks.extend(all);
        self.form.reviewed = false;
    }

    /// "I checked: this does not give the answer away", for one question the server flagged as
    /// low risk. A question with a finding that can not be acknowledged is refused.
    pub fn set_formatting_ack(&mut self, question: usize, yes: bool) {
        if !self.ackable_questions().contains(&question) {
            return;
        }
        if yes {
            self.form.ack_formatting.insert(question);
        } else {
            self.form.ack_formatting.remove(&question);
        }
        self.form.reviewed = false;
    }

    /// Whether students get a picture of a hidden-key item (it is removed unless the author says it
    /// does not show the answer). Changes the options, so the preview is stale until checked again.
    pub fn set_keep_image(&mut self, question: usize, yes: bool) {
        let has = self.source.as_ref().is_some_and(|s| s.questions.get(question).is_some_and(|q| q.image.is_some()));
        if !has {
            return;
        }
        if yes {
            self.form.keep_images.insert(question);
        } else {
            self.form.keep_images.remove(&question);
        }
        self.form.reviewed = false;
    }

    /// Picks one of the licences the server said every question allows, and checks again.
    pub fn choose_allowed_license(&mut self, l: License) {
        let Some(Fix::License(r)) = &self.fix else { return };
        if !r.allowed.iter().any(|a| a == l.as_str()) || (self.form.visibility == Visibility::Public && l == License::CircleOnly) {
            return;
        }
        self.form.license = Some(l);
        self.fix = None;
        self.check();
    }

    /// Sends the independence statement with the next check (the licence refusal's other way out).
    pub fn claim_independence(&mut self) {
        self.form.claim_independent = true;
        if let Err(e) = self.options() {
            self.form.claim_independent = false;
            self.notice = Some(Notice::Error(e));
            return;
        }
        self.fix = None;
        self.check();
    }

    /// After the person decided what the refusal asked: ask again.
    pub fn recheck(&mut self) {
        if !self.todo().done() {
            self.notice = Some(Notice::Error("Some things still need your decision.".into()));
            return;
        }
        self.fix = None;
        self.check();
    }

    /// Conflict: publish over the version now on the server (the person chose to).
    pub fn replace_newer_version(&mut self) {
        let Some(Fix::Conflict { current_publish_id: Some(cur), .. }) = self.fix.clone() else { return };
        let (Some(rec), true) = (self.record.as_mut(), super::pool::is_publish_id(&cur)) else { return };
        rec.publish_id = cur;
        self.fix = None;
        self.preview = None;
        self.check();
    }

    /// Gone / kind mismatch: forget the old item and share as a new one.
    pub fn publish_as_new(&mut self) {
        if let Some(r) = &self.record {
            let _ = self.log.forget(&r.key);
        }
        self.record = None;
        self.form.update_existing = false;
        self.target = Target::New;
        self.fix = None;
        self.failure = None;
        self.preview = None;
        self.prepared = None;
        self.step = Step::Options;
    }

    // ---- jobs ---------------------------------------------------------------------------------------------

    fn start_job(&mut self, kind: JobKind, req: PublishRequest, session: SharedSession) {
        let (tx, rx) = channel::<JobMsg>();
        let order = if matches!(req.target, PublishTarget::New) && !self.images_sent { BlobOrder::First } else { BlobOrder::OnDemand };
        let media = self.media.clone();
        let retry = self.retry;
        let repaint = self.repaint.clone();
        self.progress = None;
        match self.runner {
            #[cfg(not(target_arch = "wasm32"))]
            Runner::Inline => run_job(session, media, retry, kind, req, order, tx, repaint),
            #[cfg(target_arch = "wasm32")]
            Runner::Inline => wasm_bindgen_futures::spawn_local(async move {
                run_job_async(session, media, retry, kind, req, order, tx, repaint).await
            }),
            #[cfg(not(target_arch = "wasm32"))]
            Runner::Background => {
                std::thread::spawn(move || run_job(session, media, retry, kind, req, order, tx, repaint));
            }
            #[cfg(target_arch = "wasm32")]
            Runner::Background => wasm_bindgen_futures::spawn_local(async move {
                run_job_async(session, media, retry, kind, req, order, tx, repaint).await
            }),
        }
        self.job = Some(Job { kind, rx });
    }

    fn drain(&mut self, now: Instant) {
        let mut msgs = Vec::new();
        let mut lost = false;
        if let Some(job) = &self.job {
            loop {
                match job.rx.try_recv() {
                    Ok(m) => msgs.push(m),
                    Err(TryRecvError::Empty) => break,
                    Err(TryRecvError::Disconnected) => {
                        lost = true;
                        break;
                    }
                }
            }
        }
        for m in msgs {
            match m {
                JobMsg::Progress(p) => self.progress = Some(p),
                JobMsg::Preview(r) => {
                    self.job = None;
                    self.progress = None;
                    self.on_preview(r, now);
                    lost = false;
                }
                JobMsg::Published(r) => {
                    self.job = None;
                    self.progress = None;
                    self.on_published(r, now);
                    lost = false;
                }
            }
        }
        if lost && let Some(job) = self.job.take() {
            self.progress = None;
            self.failure = Some(Failure {
                message: "The request stopped unexpectedly.".into(),
                retry: Some(if job.kind == JobKind::Publish { RetryWhat::Publish } else { RetryWhat::Preview }),
                retry_at: None,
                account: false,
                maybe_published: job.kind == JobKind::Publish,
            });
            self.step = Step::Failed;
        }
    }

    fn on_preview(&mut self, r: Result<PreviewResult, PoolError>, now: Instant) {
        match r {
            Ok(res) => {
                let Some(prep) = self.prepared.clone() else { return };
                self.images_sent = true;
                self.show_preview(res, prep);
            }
            Err(e) => self.on_error(e, RetryWhat::Preview, now),
        }
    }

    fn show_preview(&mut self, res: PreviewResult, prep: Prepared) {
        let Some(source) = &self.source else { return };
        let mut blocked = None;
        let view = if prep.options.hide_key {
            let bytes = res.student_view.to_string().into_bytes();
            match StudentView::from_payload(&bytes, Served::Stripped) {
                Ok(v) => {
                    if !v.is_clean() {
                        blocked = Some("The preview still contains answer key material. Nothing was published. Please report this.".to_string());
                    }
                    if v.questions.len() != source.questions.len() {
                        blocked = Some("The preview does not have the same number of questions as your bank. Nothing was published.".to_string());
                    }
                    Some(v)
                }
                Err(_) => None,
            }
        } else {
            StudentView::from_payload(&source.payload(SCHEMA_NEW), Served::Full).ok()
        };
        let Some(view) = view else {
            self.failure = Some(Failure {
                message: "The server's preview could not be read. Nothing was shared.".into(),
                retry: Some(RetryWhat::Preview),
                retry_at: None,
                account: false,
                maybe_published: false,
            });
            self.step = Step::Failed;
            return;
        };
        let images = images_in_payload(&source.payload(SCHEMA_NEW))
            .unwrap_or_default()
            .into_iter()
            .map(|ImageUse { position, file, .. }| ImageNote { question: position, file, kept: !prep.options.hide_key || !res.images_removed.contains(&position) })
            .collect();
        self.preview = Some(Preview { result: res, view, options: prep.options, target: prep.target, images, blocked });
        self.form.reviewed = false;
        self.fix = None;
        self.step = Step::Review;
    }

    fn on_published(&mut self, r: Result<PublishResult, PoolError>, now: Instant) {
        match r {
            Ok(res) => {
                let republish = !matches!(self.target, Target::New);
                let title = self.form.title.trim().to_string();
                if let Some(source) = &self.source {
                    let rec = PublishedRecord {
                        key: source.key.clone(),
                        item_id: res.item_id.clone(),
                        publish_id: res.publish_id.clone(),
                        version: res.version,
                        visibility: res.visibility.clone(),
                        license: res.license.clone(),
                        title: title.clone(),
                        qids: source.links_for(&res.qids),
                    };
                    if let Err(e) = self.log.put(rec.clone()) {
                        self.notice = Some(Notice::Error(format!(
                            "It was shared, but this device could not remember that (so a later update would be a new item): {e}"
                        )));
                    }
                    self.record = Some(rec);
                }
                self.done = Some(Done { result: res, republish, title });
                self.preview = None;
                self.fix = None;
                self.failure = None;
                self.prepared = None;
                self.step = Step::Done;
            }
            Err(e) => self.on_error(e, RetryWhat::Publish, now),
        }
    }

    fn fail(&mut self, message: String, retry: Option<RetryWhat>, retry_at: Option<Instant>, maybe_published: bool) {
        self.failure = Some(Failure { message, retry, retry_at, account: false, maybe_published });
        self.step = Step::Failed;
    }

    fn on_error(&mut self, e: PoolError, what: RetryWhat, now: Instant) {
        let publishing = what == RetryWhat::Publish;
        match &e {
            PoolError::Api(ApiError::SignedOut) => {
                self.gate = Gate::SignedOut;
                self.resume = if publishing { Step::Review } else { Step::Options };
                self.step = Step::Gate;
                self.notice = Some(Notice::Info("Your session ended. Sign in again; what you entered is kept.".into()));
            }
            PoolError::Api(api_err) => {
                let mut m = api_err.message(api::Flow::Status);
                if publishing {
                    m.push_str(" It may or may not have reached the server. Trying again is safe: it is the same publish, so it can not be shared twice.");
                }
                self.fail(m, Some(what), None, publishing);
            }
            PoolError::Local(l) => {
                self.fail(l.message(), None, None, false);
                if let Some(f) = &mut self.failure {
                    f.retry = None;
                }
            }
            PoolError::Refused(r) => {
                let wait = r.detail.retry_after.filter(|s| *s > 0).map(|s| now + Duration::from_secs(s));
                match &r.kind {
                    RefusalKind::InvalidPayload(p) | RefusalKind::LabelsNeedDecision(p) | RefusalKind::OrderAckRequired(p) => {
                        self.on_payload_refusal(p);
                    }
                    RefusalKind::LicenseIncompatible(l) => {
                        self.fix = Some(Fix::License(l.clone()));
                        self.step = Step::Fix;
                    }
                    RefusalKind::LicenseNotPublic => {
                        self.form.license = None;
                        self.notice = Some(Notice::Error(e.message()));
                        self.step = Step::Options;
                    }
                    RefusalKind::AttestationRequired => {
                        self.form.attest = false;
                        self.notice = Some(Notice::Error(e.message()));
                        self.step = Step::Options;
                    }
                    RefusalKind::Conflict { current_version, current_publish_id } => {
                        self.fix = Some(Fix::Conflict { current_version: *current_version, current_publish_id: current_publish_id.clone() });
                        self.step = Step::Fix;
                    }
                    RefusalKind::PublishIdConflict => {
                        // The id was used for something else: start the publish over with a new one.
                        self.prepared = None;
                        self.fail(e.message(), Some(what), None, false);
                    }
                    RefusalKind::ItemRemoved { purged: true, .. } | RefusalKind::KindMismatch => {
                        self.fix = Some(Fix::Gone { message: e.message() });
                        self.step = Step::Fix;
                    }
                    RefusalKind::NotApproved { .. } | RefusalKind::InviteRequired => {
                        self.fail(e.message(), None, None, false);
                        if let Some(f) = &mut self.failure {
                            f.account = true;
                        }
                    }
                    RefusalKind::PoolNotConfigured => self.fail(e.message(), None, None, false),
                    RefusalKind::DailyLimit { .. }
                    | RefusalKind::RateLimited
                    | RefusalKind::TooManyOpenReads
                    | RefusalKind::TooManyUploads
                    | RefusalKind::Busy
                    | RefusalKind::PublishInProgress => self.fail(e.message(), Some(what), wait, false),
                    RefusalKind::ItemRemoved { .. }
                    | RefusalKind::ModerationState { .. }
                    | RefusalKind::SchemaUpgradeRequired { .. }
                    | RefusalKind::QuotaExceeded(_)
                    | RefusalKind::RemovedStorageExceeded { .. }
                    | RefusalKind::SharedQuestionsLimit(_)
                    | RefusalKind::BodyTooLarge { .. }
                    | RefusalKind::TooManyTags { .. }
                    | RefusalKind::ImageRefused => self.fail(e.message(), None, None, false),
                    RefusalKind::BlobMissing { .. } | RefusalKind::HashMismatch => self.fail(e.message(), Some(what), None, false),
                    _ => {
                        let code = r.detail.code.as_str();
                        let m = match code {
                            "pool_migrating" => "The pool is being upgraded. Try again in a few minutes.".to_string(),
                            _ => e.message(),
                        };
                        let retry = matches!(code, "pool_migrating").then_some(what);
                        self.fail(m, retry, wait, false);
                    }
                }
            }
        }
    }

    fn on_payload_refusal(&mut self, p: &PayloadRefusal) {
        // A kept label the server refuses goes back to undecided.
        if p.issues.iter().any(|i| i.code == "labels_keep_refused") {
            for f in &p.label_findings {
                let at = Pos { question: f.question, part: f.part };
                if self.form.labels.get(&at) == Some(&LabelChoice::Keep) {
                    self.form.labels.remove(&at);
                }
            }
        }
        self.fix = Some(Fix::Payload(Box::new(p.clone())));
        self.step = Step::Fix;
    }

    /// The position text for a question index of the source ("Question 3: Which planet ...").
    pub fn question_title(&self, index: usize) -> String {
        let label = super::publish_text::question_label(index, false);
        match self.source.as_ref().and_then(|s| s.question(index)) {
            Some(q) if !q.stem.is_empty() => format!("{label}: {}", api::plain(&q.stem)),
            _ => label,
        }
    }
}

/// Questions where every flagged finding can be acknowledged.
fn ackable_questions(p: &PayloadRefusal) -> Vec<usize> {
    let mut by_q: BTreeMap<usize, bool> = BTreeMap::new();
    for i in &p.issues {
        if matches!(i.code.as_str(), "order_ack_required" | "labels_need_decision") {
            continue;
        }
        if let Some(q) = i.question_index() {
            let e = by_q.entry(q).or_insert(true);
            *e &= i.can_acknowledge();
        }
    }
    by_q.into_iter().filter(|(_, ok)| *ok).map(|(q, _)| q).collect()
}

// ---- running a job ---------------------------------------------------------------------------------------

#[allow(clippy::too_many_arguments)]
async fn run_job_async(
    session: SharedSession,
    media: SharedMedia,
    retry: RetryPolicy,
    kind: JobKind,
    req: PublishRequest,
    order: BlobOrder,
    tx: std::sync::mpsc::Sender<JobMsg>,
    repaint: Option<Arc<dyn Fn() + Send + Sync>>,
) {
    let pool = Pool::new(&*session).with_retry(retry);
    let (ptx, prepaint) = (tx.clone(), repaint.clone());
    let mut progress = move |p: PublishProgress| {
        let _ = ptx.send(JobMsg::Progress(p));
        if let Some(r) = &prepaint {
            r();
        }
    };
    let msg = match kind {
        JobKind::Preview => JobMsg::Preview(pool.preview(&req, &*media, order, &mut progress).await),
        JobKind::Publish => JobMsg::Published(pool.publish(&req, &*media, order, &mut progress).await),
    };
    let _ = tx.send(msg);
    if let Some(r) = repaint {
        r();
    }
}

#[cfg(not(target_arch = "wasm32"))]
#[allow(clippy::too_many_arguments)]
fn run_job(
    session: SharedSession,
    media: SharedMedia,
    retry: RetryPolicy,
    kind: JobKind,
    req: PublishRequest,
    order: BlobOrder,
    tx: std::sync::mpsc::Sender<JobMsg>,
    repaint: Option<Arc<dyn Fn() + Send + Sync>>,
) {
    super::transport::block_on(run_job_async(session, media, retry, kind, req, order, tx, repaint));
}

fn ids_of(bank: &Value) -> Vec<String> {
    bank.get("questions")
        .and_then(Value::as_array)
        .map(|qs| qs.iter().map(|q| q.get("id").and_then(Value::as_str).unwrap_or("").to_string()).collect())
        .unwrap_or_default()
}
