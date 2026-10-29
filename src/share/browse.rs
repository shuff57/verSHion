//! The pool browser's logic: searching and paging the pool, one item's preview (questions, licence
//! and credits), reporting, importing into a local bank, and the update checks for what was
//! imported. No egui: tests drive every path with a scripted session.
//!
//! # Screens
//!
//! ```text
//! Gate --(signed in and approved)--> Browse (Search | Imported) --open--> Item --import--> Done
//!                                        ^                                  |  \--report--> Sent
//!                                        +------------ back ----------------+
//! ```
//!
//! Rules this module keeps:
//!
//! - A closed model does no I/O at all: `frame` and `sync` return at once. Nothing is read from
//!   disk until a screen needs it, and nothing goes to the network until the window is open and
//!   the account is signed in and approved.
//! - Nothing is ever overwritten. An import adds questions to a bank (the chosen one, which the
//!   person sees named before pressing the button) or makes a new bank. An update from the pool is
//!   always a NEW bank: the old one, with whatever the teacher changed in it, is not touched.
//! - Server-supplied text is kept raw here and drawn (and worded in `browse_text`) through
//!   [`api::plain`]; it is never markup and never a link.
//! - The licence and the author of a copy are stored on the bank (`BankDetails`) and in
//!   `share-imported.json`, which is also where the version of each import is remembered (no
//!   model field is needed for either).
//! - A cancelled or superseded request is forgotten: its receiver is dropped, a late answer is
//!   ignored.

use std::sync::mpsc::{Receiver, TryRecvError, channel};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use serde_json::Value;
use web_time::Instant;

use super::api::{self, ApiError};
use super::browse_text::{self as text, ReportChoice};
use super::merge::{self, BaseQuestion, Choice, MergeOutcome, MergePlan, Theirs};
use super::model::{Notice, Runner};
use super::pool::{
    Ack, BrowsePage, BrowseQuery, ImportInfo, ItemDoc, ItemHead, ListItem, Pool, PoolError, RefusalKind, RetryPolicy, Scope, Served, StudentView, VersionEntry,
    VersionState, is_item_id, is_sha256,
};
use super::publish::{AccountSnapshot, AccountTurn, AccountWatch, Gate, SharedSession};
use crate::model::{BankDetails, ImageRef, Question, QuestionBank};

pub const IMPORTED_DOC: &str = "share-imported.json";
pub const PAGE_SIZE: u32 = 20;
/// How long a check of an import stays fresh before opening the browser checks again.
pub const CHECK_EVERY_MS: i64 = 6 * 3_600_000;
const NOTE_MAX: usize = 500;

pub fn now_ms() -> i64 {
    web_time::SystemTime::now().duration_since(web_time::UNIX_EPOCH).map(|d| d.as_millis() as i64).unwrap_or(0)
}

// ---- what was imported ---------------------------------------------------------------------------------

/// A newer version the last check found.
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct NewerInfo {
    pub version: u64,
    #[serde(default)]
    pub license: Option<String>,
    #[serde(default)]
    pub license_version: Option<u64>,
}

/// The record's bank is linked to a circle on this computer (as the circles model last read its links).
pub fn linked_here(r: &ImportedRecord) -> bool {
    !super::circle_links::circles_of_bank(&r.bank_id).is_empty()
}

/// One import into one bank. `fingerprints` let a later update say what changed without keeping
/// the old text; `imported_at` lets it say which questions the teacher edited since.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ImportedRecord {
    pub item_id: String,
    pub bank_id: String,
    #[serde(default)]
    pub bank_name: String,
    pub version: u64,
    pub license: String,
    #[serde(default)]
    pub title: String,
    #[serde(default)]
    pub author: String,
    pub imported_at: i64,
    #[serde(default)]
    pub question_ids: Vec<String>,
    #[serde(default)]
    pub fingerprints: Vec<String>,
    #[serde(default)]
    pub newer: Option<NewerInfo>,
    /// What the last check said when it was not "current" or "newer": `under_review`, `removed`, `unavailable`.
    #[serde(default)]
    pub status: Option<String>,
    #[serde(default)]
    pub checked_at: i64,
    #[serde(default)]
    pub note: Option<String>,
    /// What each imported question was when imported or last merged: the BASE of a merge. Empty for
    /// an import made before merging existed (or one too big to keep); such a record can only be
    /// updated as a new bank.
    #[serde(default)]
    pub base: Vec<BaseQuestion>,
    /// Set when this is a copy of a bank in one of the person's circles (then `item_id` is the
    /// circle bank's id, and the public pool is never asked about it).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub circle: Option<CircleOrigin>,
}

/// Which circle a copied bank came from.
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct CircleOrigin {
    pub circle_id: String,
    #[serde(default)]
    pub circle_name: String,
}

impl ImportedRecord {
    pub fn has_update(&self) -> bool {
        self.newer.as_ref().is_some_and(|n| n.version > self.version)
    }
}

#[derive(Clone, Debug, Default, serde::Serialize, serde::Deserialize)]
struct ImportedFile {
    #[serde(default)]
    imports: Vec<ImportedRecord>,
}

/// Where the records are kept: the app's store, or a shared value (tests).
#[derive(Clone, Debug)]
pub enum ImportLog {
    Store,
    Memory(Arc<Mutex<Vec<ImportedRecord>>>),
}

impl ImportLog {
    pub fn memory() -> ImportLog {
        ImportLog::Memory(Arc::default())
    }

    pub fn load(&self) -> Vec<ImportedRecord> {
        match self {
            ImportLog::Store => crate::store::load_json::<ImportedFile>(IMPORTED_DOC)
                .map(|f| f.imports.into_iter().filter(|r| is_item_id(&r.item_id)).collect())
                .unwrap_or_default(),
            ImportLog::Memory(m) => m.lock().unwrap_or_else(|p| p.into_inner()).clone(),
        }
    }

    pub fn save(&self, items: &[ImportedRecord]) -> Result<(), String> {
        match self {
            ImportLog::Store => crate::store::save_json(IMPORTED_DOC, &ImportedFile { imports: items.to_vec() }),
            ImportLog::Memory(m) => {
                *m.lock().unwrap_or_else(|p| p.into_inner()) = items.to_vec();
                Ok(())
            }
        }
    }
}

// ---- pictures ---------------------------------------------------------------------------------------------

/// Where an imported picture is kept on this device.
pub trait ImageSink {
    fn put(&self, bytes: &[u8]) -> Result<ImageRef, String>;
}

/// The app's media store (`store::put_image`).
pub struct StoreImages;

impl ImageSink for StoreImages {
    fn put(&self, bytes: &[u8]) -> Result<ImageRef, String> {
        crate::store::put_image(bytes)
    }
}

pub type SharedImages = Arc<dyn ImageSink + Send + Sync>;

// ---- restore points ---------------------------------------------------------------------------------------

/// Where the copy of the banks made just before a merge goes. Returns where it is, in words.
pub trait RestoreSink {
    fn save(&self, banks: &[QuestionBank]) -> Result<String, String>;
    /// (round 17, critic 96 I1) The same before something other than a merge (`what`: "difficulty", the circle's difficulty
    /// put into a bank), named for it where the sink names its copies.
    fn save_named(&self, banks: &[QuestionBank], what: &str) -> Result<String, String> {
        let _ = what;
        self.save(banks)
    }
}

/// `backups/banks-before-merge-<date>.json` in the app's data folder (next to the automatic
/// backups): a complete copy of `banks.json` as it was before the merge.
pub struct StoreRestore;

impl RestoreSink for StoreRestore {
    fn save(&self, banks: &[QuestionBank]) -> Result<String, String> {
        self.save_named(banks, "merge")
    }

    /// `backups/banks-before-<what>-<date>.json`.
    fn save_named(&self, banks: &[QuestionBank], what: &str) -> Result<String, String> {
        crate::store::with_backend(|b| save_copy_to(b, banks, what, now_ms().div_euclid(1000)))
    }
}

/// (round 18, survivor k95) A safety copy of all the banks in `b`: `backups/banks-before-<what>-<date>.json` (`secs`: now), never
/// over another one. Where it is, in words. (round 6, critic) The same banks as the newest copy of that kind are not written
/// again: a second restore (or a retried one) over banks nothing else touched since leaves that one as its restore point.
pub fn save_copy_to(b: &dyn crate::store::Backend, banks: &[QuestionBank], what: &str, secs: i64) -> Result<String, String> {
    let bytes = serde_json::to_vec(banks).map_err(|e| e.to_string())?;
    let prefix = format!("banks-before-{what}-");
    let mut have: Vec<String> = b.list("backups").into_iter().filter(|n| n.starts_with(&prefix) && n.ends_with(".json")).collect();
    have.sort();
    if let Some(last) = have.last()
        && b.read(&format!("backups/{last}")).as_deref() == Some(bytes.as_slice())
    {
        return Ok(format!("{} (backups/{last})", b.label()));
    }
    let stamp = crate::store::backup_stamp(secs);
    let mut rel = format!("backups/banks-before-{what}-{stamp}.json");
    let mut n = 2;
    while b.exists(&rel) {
        rel = format!("backups/banks-before-{what}-{stamp}-{n}.json");
        n += 1;
    }
    b.write(&rel, &bytes)?;
    Ok(format!("{} ({rel})", b.label()))
}

pub type SharedRestore = Arc<dyn RestoreSink + Send + Sync>;

// ---- small types ------------------------------------------------------------------------------------------

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Screen {
    /// Waiting for a signed-in, approved account.
    Gate,
    Browse,
    Item,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Tab {
    Search,
    /// What was imported, and whether the pool has newer versions.
    Imported,
}

/// A local bank as the chooser shows it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BankInfo {
    pub id: String,
    pub name: String,
    pub count: usize,
}

/// Where an import goes.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Dest {
    Existing(String),
    New,
}

/// A failure, in words, with what can be done about it.
#[derive(Clone, Debug, PartialEq)]
pub struct Problem {
    pub message: String,
    pub retry: bool,
    pub retry_at: Option<Instant>,
    /// "Open account..." helps.
    pub account: bool,
}

/// The search form. `grade` is matched on the page already loaded (the pool has no grade filter).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct SearchForm {
    pub q: String,
    pub subject: String,
    pub kind: String,
    pub tag: String,
    pub standard: String,
    pub license: String,
    pub grade: String,
}

impl SearchForm {
    pub fn is_empty(&self) -> bool {
        *self == SearchForm::default()
    }

    fn query(&self, cursor: Option<String>) -> BrowseQuery {
        let some = |s: &str| Some(s.trim().to_string()).filter(|s| !s.is_empty());
        BrowseQuery {
            scope: Scope::Public,
            q: some(&self.q),
            subject: some(&self.subject),
            kind: some(&self.kind),
            tag: some(&self.tag),
            standard: some(&self.standard),
            license: some(&self.license),
            limit: Some(PAGE_SIZE),
            cursor,
        }
    }
}

/// The results shown.
#[derive(Clone, Debug, Default)]
pub struct Results {
    pub items: Vec<ListItem>,
    pub next: Option<String>,
    /// 0-based page number; the cursors that reached each page (the first is none).
    pub page: usize,
    cursors: Vec<Option<String>>,
    pub loaded: bool,
    pub problem: Option<Problem>,
    /// The query the page was fetched with: next and previous use it, not the edited form.
    pub applied: BrowseQuery,
}

/// What a read of one item gave.
#[derive(Clone, Debug)]
pub struct Loaded {
    pub doc: ItemDoc,
    pub view: Result<StudentView, String>,
    pub fingerprints: Vec<String>,
}

#[derive(Clone, Debug)]
pub enum Load {
    Loading,
    Ready(Box<Loaded>),
    Failed(Problem),
}

/// What an update would change, worked out from the fingerprints of the last import.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Changes {
    pub old_version: u64,
    pub new_version: u64,
    pub added: usize,
    pub removed: usize,
    pub unchanged: usize,
    /// Questions of the old import the teacher has edited since (they keep their edits).
    pub edited_locally: usize,
    /// Questions of the old import that are no longer in the bank.
    pub deleted_locally: usize,
    pub license: Option<(String, String)>,
    pub bank_missing: bool,
    /// Every question is the same as when imported and the licence is the same: the author
    /// republished without a change an importer would see. Nothing is offered.
    pub nothing_new: bool,
}

#[derive(Clone, Debug)]
pub struct UpdateCtx {
    pub record: ImportedRecord,
    pub changes: Option<Changes>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ImportChoice {
    pub dest: Dest,
    pub new_name: String,
}

/// The result of a finished import, for the screen.
#[derive(Clone, Debug, PartialEq)]
pub struct ImportDone {
    pub title: String,
    pub author: String,
    pub license: String,
    pub version: u64,
    pub bank_id: String,
    pub bank_name: String,
    pub new_bank: bool,
    pub added: usize,
    pub pictures: usize,
    pub pictures_missing: usize,
    pub keyless: bool,
    pub previously: Vec<u64>,
    pub may_republish_as: Vec<String>,
    pub recorded: bool,
    pub held_pictures_quota: bool,
}

#[derive(Clone, Debug)]
pub enum ImportPanel {
    Closed,
    Choosing(ImportChoice),
    Working,
    Done(Box<ImportDone>),
    Failed(Problem),
}

#[derive(Clone, Debug)]
pub enum ReportState {
    Editing,
    Sending,
    Sent(String),
    Failed(Problem),
}

#[derive(Clone, Debug)]
pub struct ReportForm {
    pub choice: Option<ReportChoice>,
    pub note: String,
    pub state: ReportState,
}

pub struct ItemState {
    pub id: String,
    /// The listing row the item was opened from, shown while the item loads.
    pub summary: Option<ListItem>,
    pub load: Load,
    pub import: ImportPanel,
    pub report: Option<ReportForm>,
    pub update: Option<UpdateCtx>,
    pub merge: Option<MergeState>,
}

/// The converted questions of a finished import job, waiting to be put into a bank by `sync`.
#[derive(Clone, Debug)]
pub struct Imported {
    pub head: ItemHead,
    pub info: ImportInfo,
    pub questions: Vec<Question>,
    pub fingerprints: Vec<String>,
    pub pictures: usize,
    pub pictures_missing: usize,
    pub keyless: bool,
    pub at_ms: i64,
    /// The pool's questions as imported (pictures stored), in the pool's order: what a merge uses.
    pub theirs: Vec<Theirs>,
}

// ---- merge state ------------------------------------------------------------------------------------------

#[derive(Clone, Debug)]
pub struct MergeDone {
    pub outcome: MergeOutcome,
    pub restore: String,
    pub new_version: u64,
    pub bank_name: String,
    pub license_changed: bool,
}

#[derive(Clone, Debug)]
pub enum MergePhase {
    /// "Merge into this bank..." was pressed; the review is built on the next `sync`.
    Wanted,
    /// The per-question review. Nothing has been written.
    Review,
    /// The pool's version is being fetched; the bank is written when it arrives.
    Applying,
    Done(Box<MergeDone>),
    /// A merge is not possible (the words say why).
    Blocked(String),
}

#[derive(Clone, Debug)]
pub struct MergeState {
    pub phase: MergePhase,
    pub plan: Option<MergePlan>,
    /// Why the last apply did not happen (nothing was written).
    pub problem: Option<Problem>,
}

// ---- jobs -----------------------------------------------------------------------------------------------------

#[derive(Clone, Debug)]
enum Work {
    Search(Box<BrowseQuery>),
    Item(String),
    Import(String),
    Report { id: String, choice: ReportChoice, note: String },
    Versions(Vec<String>),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum JobKind {
    Search,
    Item,
    Import,
    Report,
    Versions,
}

impl Work {
    fn kind(&self) -> JobKind {
        match self {
            Work::Search(_) => JobKind::Search,
            Work::Item(_) => JobKind::Item,
            Work::Import(_) => JobKind::Import,
            Work::Report { .. } => JobKind::Report,
            Work::Versions(_) => JobKind::Versions,
        }
    }
}

enum JobMsg {
    Search(Result<BrowsePage, PoolError>),
    Item(Result<Loaded, PoolError>),
    Import(Result<Imported, PoolError>),
    Report(Result<Ack, PoolError>),
    Versions(Result<Vec<VersionEntry>, PoolError>),
}

struct Job {
    kind: JobKind,
    rx: Receiver<JobMsg>,
}

async fn run_work_async(
    session: SharedSession,
    images: SharedImages,
    retry: RetryPolicy,
    work: Work,
    tx: std::sync::mpsc::Sender<JobMsg>,
    repaint: Option<Arc<dyn Fn() + Send + Sync>>,
) {
    let pool = Pool::new(&*session).with_retry(retry);
    let msg = match work {
        Work::Search(q) => JobMsg::Search(pool.browse(&q).await),
        Work::Item(id) => JobMsg::Item(load_item(&pool, &id).await),
        Work::Import(id) => JobMsg::Import(import_item(&pool, &*images, &id).await),
        Work::Report { id, choice, note } => JobMsg::Report(send_report(&pool, &id, choice, &note).await),
        Work::Versions(ids) => JobMsg::Versions(pool.versions(&ids).await),
    };
    let _ = tx.send(msg);
    if let Some(r) = repaint {
        r();
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn run_work(
    session: SharedSession,
    images: SharedImages,
    retry: RetryPolicy,
    work: Work,
    tx: std::sync::mpsc::Sender<JobMsg>,
    repaint: Option<Arc<dyn Fn() + Send + Sync>>,
) {
    super::transport::block_on(run_work_async(session, images, retry, work, tx, repaint));
}

async fn send_report(pool: &Pool<'_>, id: &str, choice: ReportChoice, note: &str) -> Result<Ack, PoolError> {
    let note = note.trim();
    pool.report(id, choice.reason(), Some(note).filter(|n| !n.is_empty())).await
}

/// Reads an item and works out what the screen needs of it.
async fn load_item(pool: &Pool<'_>, id: &str) -> Result<Loaded, PoolError> {
    let doc = pool.read_item(id, None).await?;
    let form = served_form(&doc.head);
    let view = StudentView::from_payload(&doc.payload, form).map_err(|e| e.message());
    let fingerprints = fingerprints_of(&doc.payload);
    Ok(Loaded { doc, view, fingerprints })
}

fn served_form(head: &ItemHead) -> Served {
    if head.item.form == "full" { Served::Full } else { Served::Stripped }
}

/// FNV-1a over the question's text and kind (ids, dates and pictures left out): the same question
/// in two versions has the same fingerprint, an edited one does not. Not a secret, only a
/// comparison.
fn fingerprint(q: &Value) -> String {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    let mut feed = |bytes: &[u8]| {
        for b in bytes {
            h ^= u64::from(*b);
            h = h.wrapping_mul(0x0000_0100_0000_01b3);
        }
        h ^= 0xff;
        h = h.wrapping_mul(0x0000_0100_0000_01b3);
    };
    feed(q.get("stem").and_then(Value::as_str).unwrap_or("").as_bytes());
    feed(q.get("kind").map(|k| k.to_string()).unwrap_or_default().as_bytes());
    format!("{h:016x}")
}

fn questions_of(payload: &[u8]) -> Option<(Option<String>, Vec<Value>)> {
    let v: Value = serde_json::from_slice(payload).ok()?;
    if let Some(b) = v.get("bank") {
        let name = b.get("name").and_then(Value::as_str).map(str::to_string);
        Some((name, b.get("questions")?.as_array()?.clone()))
    } else {
        Some((None, vec![v.get("question")?.clone()]))
    }
}

pub fn fingerprints_of(payload: &[u8]) -> Vec<String> {
    questions_of(payload).map(|(_, qs)| qs.iter().map(fingerprint).collect()).unwrap_or_default()
}

/// The pool's questions in this app's shape, with the pool's id and fingerprint for each.
pub(super) fn theirs_of(payload: &[u8], head: &ItemHead, at_ms: i64) -> Vec<Theirs> {
    let keyless_form = served_form(head) == Served::Stripped;
    let Some((_, raw)) = questions_of(payload) else { return Vec::new() };
    raw.into_iter()
        .enumerate()
        .filter_map(|(index, q)| {
            let fp = fingerprint(&q);
            let qid = head.questions.iter().find(|p| p.pos == index).map(|p| p.qid.clone()).unwrap_or_default();
            let (q, sha) = merge::convert_question(q, keyless_form, at_ms)?;
            Some(Theirs { index, qid, fp, sha, q })
        })
        .collect()
}

/// The import job: the server's import answer, then the held pictures, then the questions in the
/// shape of this app (new ids, pictures stored here, keyless questions marked).
async fn import_item(pool: &Pool<'_>, images: &(dyn ImageSink + Send + Sync), id: &str) -> Result<Imported, PoolError> {
    let doc = pool.import_item(id).await?;
    let info = doc.import.clone().unwrap_or_default();
    if questions_of(&doc.payload).is_none() {
        return Err(PoolError::Local(super::pool::LocalProblem::BadItemShape));
    }
    let at_ms = now_ms();
    let mut theirs = theirs_of(&doc.payload, &doc.head, at_ms);
    let mut stored: std::collections::HashMap<String, Option<ImageRef>> = Default::default();
    let (mut pictures, mut missing) = (0usize, 0usize);
    for t in &mut theirs {
        let Some(sha) = t.sha.clone() else { continue };
        pictures += 1;
        let image = match stored.get(&sha) {
            Some(done) => done.clone(),
            None => {
                let got = if is_sha256(&sha) {
                    match pool.get_blob(id, &sha).await {
                        Ok(b) => images.put(&b.bytes).ok(),
                        // The sign-in ended: the whole import is retried after signing in again.
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
    let keyless = questions.iter().any(crate::model::is_keyless);
    Ok(Imported { head: doc.head, info, questions, fingerprints, pictures, pictures_missing: missing, keyless, at_ms, theirs })
}

// ---- putting an import into the bank list ---------------------------------------------------------------------------

/// The one-line credit stored on the bank.
pub fn attribution_line(title: &str, author: &str, license: &str, item_id: &str, version: u64) -> String {
    format!(
        "From the shared pool: \"{}\" by {}, licence {}, item {} version {}.",
        api::plain(title),
        text::author(author),
        text::license_name(license),
        api::plain(item_id),
        version
    )
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Applied {
    pub bank_id: String,
    pub bank_name: String,
    pub added: usize,
    pub new_bank: bool,
}

/// Adds the imported questions to the banks: appended to an existing bank, or as a new bank. The
/// attribution goes on the bank: a new bank gets the author and licence fields and a description;
/// an existing bank keeps its own details and only gains a line in its description, so mixed
/// sources stay visible. Nothing already there is changed or removed.
pub fn apply_import(banks: &mut Vec<QuestionBank>, dest: &Dest, new_name: &str, imp: &Imported) -> Result<Applied, String> {
    let item = &imp.head.item;
    let license = if imp.info.license.is_empty() { item.license.clone() } else { imp.info.license.clone() };
    let line = attribution_line(&item.title, &item.author_name, &license, &item.id, item.version);
    match dest {
        Dest::New => {
            let name = new_name.trim();
            let name = if name.is_empty() { api::plain(&item.title) } else { api::plain(name) };
            let name = if name.is_empty() { "Imported from the pool".to_string() } else { name };
            let bank = QuestionBank {
                id: crate::model::new_id(),
                name: name.clone(),
                questions: imp.questions.clone(),
                // The app keeps bank times in seconds (the editors write seconds); `at_ms` is milliseconds.
                created: imp.at_ms / 1000,
                modified: imp.at_ms / 1000,
                details: BankDetails {
                    description: line,
                    author: text::author(&item.author_name),
                    license_name: text::license_name(&license),
                    license_url: text::license_url(&license).to_string(),
                },
            };
            let id = bank.id.clone();
            banks.push(bank);
            Ok(Applied { bank_id: id, bank_name: name, added: imp.questions.len(), new_bank: true })
        }
        Dest::Existing(id) => {
            let bank = banks.iter_mut().find(|b| b.id == *id).ok_or_else(|| "The bank you chose is not there any more. Choose another, or make a new one.".to_string())?;
            bank.questions.extend(imp.questions.iter().cloned());
            bank.modified = (imp.at_ms / 1000).max(bank.modified);
            if bank.details.is_empty() && bank.questions.len() == imp.questions.len() {
                bank.details = BankDetails {
                    description: line,
                    author: text::author(&item.author_name),
                    license_name: text::license_name(&license),
                    license_url: text::license_url(&license).to_string(),
                };
            } else if bank.details.description.trim().is_empty() {
                bank.details.description = line;
            } else {
                bank.details.description = format!("{}\n{}", bank.details.description.trim_end(), line);
            }
            Ok(Applied { bank_id: id.clone(), bank_name: bank.name.clone(), added: imp.questions.len(), new_bank: false })
        }
    }
}

// ---- the model ----------------------------------------------------------------------------------------------------

pub struct BrowseModel {
    pub open: bool,
    pub screen: Screen,
    pub tab: Tab,
    pub gate: Gate,
    pub notice: Option<Notice>,
    pub form: SearchForm,
    pub results: Results,
    pub item: Option<ItemState>,
    /// A problem with the last update check (quiet: shown on the Imported tab only).
    pub check_problem: Option<Problem>,
    /// The pool answered `invite_required` (the Shared page then asks for the code).
    pub invite_required: bool,
    /// The local banks, for the destination chooser (refreshed by `sync`).
    pub banks: Vec<BankInfo>,
    /// The bank the person came from: the import's default destination.
    pub default_bank: Option<String>,
    /// "Open in My Banks" on the Imported tab (critic 68 minor 6): the Shared page opens this bank's page.
    pub open_bank: Option<String>,
    open_account: bool,
    asked_account: bool,
    resume: Screen,
    session: Option<SharedSession>,
    images: SharedImages,
    runner: Runner,
    retry: RetryPolicy,
    jobs: Vec<Job>,
    log: ImportLog,
    imported: Vec<ImportedRecord>,
    loaded_log: bool,
    ready: Option<(Dest, String, Box<Imported>)>,
    ready_merge: Option<Box<Imported>>,
    restore: SharedRestore,
    pending_dest: Option<(Dest, String)>,
    pending_update: Option<ImportedRecord>,
    checked_this_open: bool,
    now: Option<Instant>,
    repaint: Option<Arc<dyn Fn() + Send + Sync>>,
    /// Who the results and the item open belong to (critic 61 I3).
    watch: AccountWatch,
}

impl Default for BrowseModel {
    fn default() -> Self {
        BrowseModel::new(Runner::Background, Arc::new(StoreImages), ImportLog::Store)
    }
}

impl BrowseModel {
    /// Does no I/O: nothing is read or sent until the window is opened.
    pub fn new(runner: Runner, images: SharedImages, log: ImportLog) -> BrowseModel {
        BrowseModel {
            open: false,
            screen: Screen::Gate,
            tab: Tab::Search,
            gate: Gate::Checking,
            notice: None,
            form: SearchForm::default(),
            results: Results::default(),
            item: None,
            check_problem: None,
            invite_required: false,
            banks: Vec::new(),
            default_bank: None,
            open_bank: None,
            open_account: false,
            asked_account: false,
            resume: Screen::Browse,
            session: None,
            images,
            runner,
            retry: RetryPolicy::default(),
            jobs: Vec::new(),
            log,
            imported: Vec::new(),
            loaded_log: false,
            ready: None,
            ready_merge: None,
            restore: Arc::new(StoreRestore),
            pending_dest: None,
            pending_update: None,
            checked_this_open: false,
            now: None,
            repaint: None,
            watch: AccountWatch::default(),
        }
    }

    pub fn with_retry(mut self, retry: RetryPolicy) -> BrowseModel {
        self.retry = retry;
        self
    }

    pub fn with_restore(mut self, r: SharedRestore) -> BrowseModel {
        self.restore = r;
        self
    }

    pub fn set_repaint(&mut self, f: Arc<dyn Fn() + Send + Sync>) {
        self.repaint = Some(f);
    }

    pub fn busy(&self) -> bool {
        !self.jobs.is_empty()
    }

    /// An import is running: the window should not be closed or left until it finishes.
    pub fn busy_import(&self) -> bool {
        self.running(JobKind::Import)
    }

    /// How many imports the last check found a newer version for. (critic 66 I2) Not a bank linked to a circle on
    /// this computer: its strip on the bank's page offers Get latest.
    pub fn update_count(&self) -> usize {
        self.imported.iter().filter(|r| r.has_update() && !linked_here(r)).count()
    }

    fn running(&self, k: JobKind) -> bool {
        self.jobs.iter().any(|j| j.kind == k)
    }

    // ---- opening -------------------------------------------------------------------------------------------

    fn open_window(&mut self, tab: Tab, default_bank: Option<String>) {
        self.cancel_all();
        self.pending_update = None;
        self.open = true;
        self.tab = tab;
        self.default_bank = default_bank;
        self.notice = None;
        self.item = None;
        self.check_problem = None;
        self.open_account = false;
        self.asked_account = false;
        self.checked_this_open = false;
        self.resume = Screen::Browse;
        self.screen = Screen::Gate;
        self.ensure_log();
    }

    /// "Browse shared questions" (from the Account window or Settings).
    pub fn open_browse(&mut self) {
        self.open_window(Tab::Search, None);
    }

    /// "From pool..." on a bank page: importing into that bank is the default.
    pub fn open_for_bank(&mut self, bank_id: &str) {
        self.open_window(Tab::Search, Some(bank_id.to_string()));
    }

    pub fn open_imported(&mut self) {
        self.open_window(Tab::Imported, None);
    }

    /// "Update from pool" on a bank page: opens the newest version of what that bank was imported
    /// from, with what changed. Falls back to the Imported tab when the bank has several sources.
    pub fn open_update(&mut self, bank_id: &str) {
        self.open_window(Tab::Imported, Some(bank_id.to_string()));
        let ups: Vec<ImportedRecord> = self.imported.iter().filter(|r| r.bank_id == bank_id && r.has_update() && r.circle.is_none()).cloned().collect();
        if let [one] = ups.as_slice() {
            self.pending_update = Some(one.clone());
        }
    }

    pub fn close(&mut self) {
        self.open = false;
        self.cancel_all();
        self.open_account = false;
        self.pending_update = None;
    }

    fn cancel_all(&mut self) {
        // Dropping the receivers makes late answers go nowhere.
        self.jobs.clear();
        self.ready = None;
        self.ready_merge = None;
        self.pending_dest = None;
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

    // ---- the imports log ---------------------------------------------------------------------------------------

    fn ensure_log(&mut self) {
        if !self.loaded_log {
            self.imported = self.log.load();
            self.loaded_log = true;
        }
    }

    /// The records, oldest first. Reads the small file on first use.
    pub fn imported(&mut self) -> &[ImportedRecord] {
        self.ensure_log();
        &self.imported
    }

    /// The quiet "update available" note for a bank page ("Update from pool..."), from the last check (no network).
    /// (critic 66 I2) Never for a copy of a circle bank, which is not a public pool item, nor for a bank linked to a
    /// circle on this computer: its strip offers Get latest.
    pub fn update_for_bank(&mut self, bank_id: &str) -> Option<ImportedRecord> {
        self.ensure_log();
        self.imported.iter().find(|r| r.bank_id == bank_id && r.has_update() && r.circle.is_none() && !linked_here(r)).cloned()
    }

    /// The record of an earlier import of this item, newest first.
    pub fn imports_of(&mut self, item_id: &str) -> Vec<ImportedRecord> {
        self.ensure_log();
        let mut v: Vec<ImportedRecord> = self.imported.iter().filter(|r| r.item_id == item_id).cloned().collect();
        v.sort_by_key(|r| std::cmp::Reverse(r.imported_at));
        v
    }

    fn save_log(&mut self) -> Result<(), String> {
        self.log.save(&self.imported)
    }

    // ---- frame ----------------------------------------------------------------------------------------------------

    /// Applies finished jobs and the account's state, starts the first search and the quiet update
    /// check. Does nothing when closed.
    pub fn frame(&mut self, now: Instant, snap: &AccountSnapshot) {
        if !self.open {
            return;
        }
        self.now = Some(now);
        self.note_account(snap);
        self.apply_snapshot(snap);
        self.drain(now);
        self.auto(now);
    }

    /// The account's state, every frame, the window open or not (critic 61 I3): what was read for one account is
    /// never shown to another. A sign-out keeps the place and what was loaded (the same account comes back to it).
    /// Another account signing in gets none of it: the results (they can hold the first account's own items), the
    /// item open (with its report and a merge under review), the search typed, and the jobs reading for the first
    /// account (an import on its way is dropped, as closing the window drops it: nothing is added to the banks). It
    /// starts again at the browser.
    pub fn note_account(&mut self, snap: &AccountSnapshot) {
        if self.watch.note(snap) != AccountTurn::Another {
            return;
        }
        self.cancel_all();
        self.pending_update = None;
        self.results = Results::default();
        self.form = SearchForm::default();
        self.item = None;
        self.check_problem = None;
        self.notice = None;
        self.invite_required = false;
        self.checked_this_open = false;
        self.resume = Screen::Browse;
        self.screen = Screen::Gate;
    }

    fn apply_snapshot(&mut self, snap: &AccountSnapshot) {
        self.session = snap.session.clone();
        let mut gate = snap.gate.clone();
        if gate == Gate::Checking && self.asked_account && !snap.account_open {
            gate = Gate::SignedOut;
        }
        self.gate = gate.clone();
        match self.screen {
            Screen::Gate => {
                if gate == Gate::Ready {
                    self.screen = self.resume;
                }
            }
            _ => {
                let definite = matches!(gate, Gate::SignedOut | Gate::NotApproved(_) | Gate::NotSetUp | Gate::NoServer);
                if definite && !self.running(JobKind::Import) {
                    self.resume = self.screen;
                    self.screen = Screen::Gate;
                }
            }
        }
    }

    fn auto(&mut self, _now: Instant) {
        if self.screen == Screen::Gate || self.session.is_none() {
            return;
        }
        if let Some(rec) = self.pending_update.take() {
            self.open_item_for_update(rec);
            return;
        }
        if self.screen == Screen::Browse && self.tab == Tab::Search && !self.results.loaded && self.results.problem.is_none() && !self.running(JobKind::Search) {
            self.search();
        }
        if !self.checked_this_open && !self.running(JobKind::Versions) {
            self.checked_this_open = true;
            let stale = self.imported.iter().any(|r| now_ms() - r.checked_at > CHECK_EVERY_MS);
            if stale {
                self.check_updates();
            }
        }
    }

    // ---- errors ----------------------------------------------------------------------------------------------------

    /// Turns an error into a `Problem`, or sends the person to the Account window (and returns
    /// `None`) when the sign-in ended.
    fn problem_of(&mut self, e: &PoolError) -> Option<Problem> {
        let now = self.now.unwrap_or_else(Instant::now);
        let mut p = Problem { message: e.message(), retry: false, retry_at: None, account: false };
        match e {
            PoolError::Api(ApiError::SignedOut) => {
                self.gate = Gate::SignedOut;
                self.resume = if self.screen == Screen::Gate { self.resume } else { self.screen };
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
                    RefusalKind::DailyLimit { .. } | RefusalKind::RateLimited | RefusalKind::TooManyOpenReads | RefusalKind::TooManyUploads | RefusalKind::Busy => {
                        p.retry = true;
                        p.retry_at = wait;
                    }
                    RefusalKind::NotFound => p.message = "That item is not available. It may have been removed or made private.".into(),
                    RefusalKind::Other => match r.detail.code.as_str() {
                        "pool_migrating" => {
                            p.message = "The pool is being upgraded. Try again in a few minutes.".into();
                            p.retry = true;
                            p.retry_at = wait;
                        }
                        "imports_quota" => p.message = "You have copied as many different items as the pool allows an account. Nothing was imported.".into(),
                        "reads_quota" => p.message = "You have looked at as many different items as the pool allows an account.".into(),
                        "evidence_quota_exceeded" => p.message = "The pool can not take another import for this account right now. Nothing was imported.".into(),
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

    // ---- launching ----------------------------------------------------------------------------------------------------

    fn launch(&mut self, work: Work) -> bool {
        let Some(session) = self.session.clone() else {
            self.gate = Gate::Checking;
            return false;
        };
        let kind = work.kind();
        self.jobs.retain(|j| j.kind != kind);
        let (tx, rx) = channel::<JobMsg>();
        let (images, retry, repaint) = (self.images.clone(), self.retry, self.repaint.clone());
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

    fn drain(&mut self, _now: Instant) {
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
            let kind = match &m {
                JobMsg::Search(_) => JobKind::Search,
                JobMsg::Item(_) => JobKind::Item,
                JobMsg::Import(_) => JobKind::Import,
                JobMsg::Report(_) => JobKind::Report,
                JobMsg::Versions(_) => JobKind::Versions,
            };
            self.jobs.retain(|j| j.kind != kind);
            lost.retain(|k| *k != kind);
            match m {
                JobMsg::Search(r) => self.on_search(r),
                JobMsg::Item(r) => self.on_item(r),
                JobMsg::Import(r) => self.on_import(r),
                JobMsg::Report(r) => self.on_report(r),
                JobMsg::Versions(r) => self.on_versions(r),
            }
        }
        for kind in lost {
            self.jobs.retain(|j| j.kind != kind);
            let p = Problem { message: "The request stopped unexpectedly.".into(), retry: true, retry_at: None, account: false };
            self.fail_kind(kind, p);
        }
    }

    fn fail_kind(&mut self, kind: JobKind, p: Problem) {
        match kind {
            JobKind::Search => self.results.problem = Some(p),
            JobKind::Item => {
                if let Some(it) = &mut self.item {
                    it.load = Load::Failed(p);
                }
            }
            JobKind::Import => {
                if let Some(it) = &mut self.item {
                    it.import = ImportPanel::Failed(p);
                }
            }
            JobKind::Report => {
                if let Some(r) = self.item.as_mut().and_then(|i| i.report.as_mut()) {
                    r.state = ReportState::Failed(p);
                }
            }
            JobKind::Versions => self.check_problem = Some(p),
        }
    }

    // ---- searching ----------------------------------------------------------------------------------------------

    fn run_search(&mut self, query: BrowseQuery, page: usize, cursors: Vec<Option<String>>) {
        self.results.problem = None;
        self.results.applied = query.clone();
        self.results.page = page;
        self.results.cursors = cursors;
        self.launch(Work::Search(Box::new(query)));
    }

    /// Searches with the form as it is, from the first page.
    pub fn search(&mut self) {
        let q = self.form.query(None);
        self.results.loaded = false;
        self.run_search(q, 0, vec![None]);
    }

    pub fn clear_search(&mut self) {
        self.form = SearchForm::default();
        self.search();
    }

    pub fn can_next(&self) -> bool {
        self.results.next.is_some() && !self.running(JobKind::Search)
    }

    pub fn can_prev(&self) -> bool {
        self.results.page > 0 && !self.running(JobKind::Search)
    }

    pub fn next_page(&mut self) {
        let Some(next) = self.results.next.clone().filter(|_| self.can_next()) else { return };
        let mut cursors = self.results.cursors.clone();
        cursors.push(Some(next.clone()));
        let mut q = self.results.applied.clone();
        q.cursor = Some(next);
        let page = self.results.page + 1;
        self.run_search(q, page, cursors);
    }

    pub fn prev_page(&mut self) {
        if !self.can_prev() {
            return;
        }
        let page = self.results.page - 1;
        let mut cursors = self.results.cursors.clone();
        cursors.truncate(page + 1);
        let mut q = self.results.applied.clone();
        q.cursor = cursors.get(page).cloned().flatten();
        self.run_search(q, page, cursors);
    }

    /// Tries the failed search again (the same page).
    pub fn retry_search(&mut self) {
        let (q, page, cursors) = (self.results.applied.clone(), self.results.page, self.results.cursors.clone());
        let cursors = if cursors.is_empty() { vec![None] } else { cursors };
        self.run_search(q, page, cursors);
    }

    fn on_search(&mut self, r: Result<BrowsePage, PoolError>) {
        match r {
            Ok(p) => {
                self.results.items = p.items;
                self.results.next = p.next_cursor;
                self.results.loaded = true;
                self.results.problem = None;
            }
            Err(e) => {
                if let Some(p) = self.problem_of(&e) {
                    self.results.problem = Some(p);
                }
            }
        }
    }

    /// The loaded page, narrowed by the grade box (the pool has no grade filter).
    pub fn visible(&self) -> Vec<&ListItem> {
        let g = self.form.grade.trim().to_lowercase();
        self.results.items.iter().filter(|i| g.is_empty() || i.grade.to_lowercase().contains(&g) || i.course.to_lowercase().contains(&g)).collect()
    }

    // ---- one item ------------------------------------------------------------------------------------------------

    pub fn open_item(&mut self, id: &str, summary: Option<ListItem>) {
        self.item = Some(ItemState { id: id.to_string(), summary, load: Load::Loading, import: ImportPanel::Closed, report: None, update: None, merge: None });
        self.screen = Screen::Item;
        self.launch(Work::Item(id.to_string()));
    }

    fn open_item_for_update(&mut self, rec: ImportedRecord) {
        // (critic 66 I2) a circle bank's id is not a public pool item: it is never opened as one
        if rec.circle.is_some() {
            return;
        }
        self.open_item(&rec.item_id.clone(), None);
        if let Some(it) = &mut self.item {
            it.update = Some(UpdateCtx { record: rec, changes: None });
        }
    }

    /// "Update..." on an imported record that has a newer version.
    pub fn start_update(&mut self, rec: &ImportedRecord) {
        self.open_item_for_update(rec.clone());
    }

    pub fn reload_item(&mut self) {
        let Some(id) = self.item.as_ref().map(|i| i.id.clone()) else { return };
        if let Some(it) = &mut self.item {
            it.load = Load::Loading;
        }
        self.launch(Work::Item(id));
    }

    pub fn back_to_list(&mut self) {
        self.jobs.retain(|j| !matches!(j.kind, JobKind::Item | JobKind::Report));
        if self.running(JobKind::Import) {
            return;
        }
        self.item = None;
        self.screen = Screen::Browse;
    }

    fn on_item(&mut self, r: Result<Loaded, PoolError>) {
        match r {
            Ok(l) => {
                if let Some(it) = &mut self.item {
                    it.load = Load::Ready(Box::new(l));
                }
            }
            Err(e) => {
                if let Some(p) = self.problem_of(&e) {
                    self.fail_kind(JobKind::Item, p);
                }
            }
        }
    }

    pub fn loaded(&self) -> Option<&Loaded> {
        match self.item.as_ref().map(|i| &i.load) {
            Some(Load::Ready(l)) => Some(l),
            _ => None,
        }
    }

    // ---- importing -----------------------------------------------------------------------------------------------

    /// Why this item can not be imported right now, in words.
    pub fn import_blocker(&self) -> Option<String> {
        let Some(l) = self.loaded() else { return Some("The item is still loading.".into()) };
        if l.doc.head.item.is_author {
            return Some("This is your own item. It is already in your banks.".into());
        }
        match &l.view {
            Err(m) => Some(format!("The item could not be read: {m}")),
            Ok(v) if !v.is_clean() && v.form == Served::Stripped => {
                Some("This item says its answer key is hidden, but the key seems to be in it. It is not imported; please report it.".into())
            }
            Ok(v) if v.questions.is_empty() => Some("This item has no questions.".into()),
            Ok(_) => None,
        }
    }

    /// "Import..." opens the choice of where the copy goes. An update only ever makes a new bank.
    pub fn begin_import(&mut self) {
        if self.import_blocker().is_some() {
            return;
        }
        let Some(l) = self.loaded() else { return };
        let title = api::plain(&l.doc.head.item.title);
        let updating = self.item.as_ref().is_some_and(|i| i.update.is_some());
        let default = self.default_bank.clone().filter(|id| self.banks.iter().any(|b| b.id == *id));
        let (dest, new_name) = if updating {
            (Dest::New, format!("{title} (version {})", l.doc.head.item.version))
        } else {
            (default.map_or(Dest::New, Dest::Existing), title)
        };
        if let Some(it) = &mut self.item {
            it.import = ImportPanel::Choosing(ImportChoice { dest, new_name });
        }
    }

    pub fn set_dest(&mut self, dest: Dest) {
        let updating = self.item.as_ref().is_some_and(|i| i.update.is_some());
        if updating {
            return;
        }
        if let Some(ImportPanel::Choosing(c)) = self.item.as_mut().map(|i| &mut i.import) {
            c.dest = dest;
        }
    }

    pub fn set_new_name(&mut self, name: &str) {
        if let Some(ImportPanel::Choosing(c)) = self.item.as_mut().map(|i| &mut i.import) {
            c.new_name = name.chars().take(120).collect();
        }
    }

    pub fn cancel_import(&mut self) {
        if let Some(it) = &mut self.item
            && matches!(it.import, ImportPanel::Choosing(_) | ImportPanel::Failed(_))
        {
            it.import = ImportPanel::Closed;
        }
    }

    /// The person pressed "Import" with the choice shown.
    pub fn confirm_import(&mut self) {
        let Some(it) = &mut self.item else { return };
        let ImportPanel::Choosing(choice) = &it.import else { return };
        let (dest, name, id) = (choice.dest.clone(), choice.new_name.clone(), it.id.clone());
        if let Dest::Existing(b) = &dest
            && !self.banks.iter().any(|x| x.id == *b)
        {
            it.import = ImportPanel::Failed(Problem {
                message: "The bank you chose is not there any more. Choose another, or make a new one.".into(),
                retry: false,
                retry_at: None,
                account: false,
            });
            return;
        }
        it.import = ImportPanel::Working;
        self.pending_dest = Some((dest, name));
        if !self.launch(Work::Import(id)) {
            self.pending_dest = None;
            if let Some(it) = &mut self.item {
                it.import = ImportPanel::Closed;
            }
        }
    }

    /// Tries a failed import again (it goes to the same place).
    pub fn retry_import(&mut self) {
        let Some(it) = &mut self.item else { return };
        if !matches!(it.import, ImportPanel::Failed(_)) {
            return;
        }
        let id = it.id.clone();
        it.import = ImportPanel::Working;
        if self.pending_dest.is_none() {
            self.pending_dest = Some((Dest::New, String::new()));
        }
        self.launch(Work::Import(id));
    }

    fn merging(&self) -> bool {
        self.item.as_ref().and_then(|i| i.merge.as_ref()).is_some_and(|m| matches!(m.phase, MergePhase::Applying))
    }

    fn on_import(&mut self, r: Result<Imported, PoolError>) {
        if self.merging() {
            match r {
                Ok(imp) => self.ready_merge = Some(Box::new(imp)),
                Err(e) => {
                    // Nothing was written: back to the review, with the reason.
                    if let Some(ms) = self.item.as_mut().and_then(|i| i.merge.as_mut()) {
                        ms.phase = MergePhase::Review;
                    }
                    if let Some(p) = self.problem_of(&e)
                        && let Some(ms) = self.item.as_mut().and_then(|i| i.merge.as_mut())
                    {
                        ms.problem = Some(p);
                    }
                }
            }
            return;
        }
        match r {
            Ok(imp) => {
                let (dest, name) = self.pending_dest.take().unwrap_or((Dest::New, String::new()));
                self.ready = Some((dest, name, Box::new(imp)));
            }
            Err(e) => {
                if let Some(p) = self.problem_of(&e)
                    && let Some(it) = &mut self.item
                {
                    it.import = ImportPanel::Failed(p);
                }
            }
        }
    }

    // ---- syncing with the banks -----------------------------------------------------------------------------------

    /// Called every frame with the app's banks: refreshes the destination list, works out what an
    /// update would change, and puts a finished import into the banks. Returns true when `banks`
    /// changed (the caller then saves them). Does nothing when closed.
    pub fn sync(&mut self, banks: &mut Vec<QuestionBank>) -> bool {
        if !self.open {
            return false;
        }
        if self.banks.len() != banks.len() || self.banks.iter().zip(banks.iter()).any(|(a, b)| a.id != b.id || a.name != b.name || a.count != b.questions.len()) {
            self.banks = banks.iter().map(|b| BankInfo { id: b.id.clone(), name: b.name.clone(), count: b.questions.len() }).collect();
        }
        self.refresh_changes(banks);
        self.refresh_merge(banks);
        if let Some(imp) = self.ready_merge.take() {
            return self.finish_merge(banks, &imp);
        }
        let Some((dest, name, imp)) = self.ready.take() else { return false };
        match apply_import(banks, &dest, &name, &imp) {
            Ok(applied) => {
                self.finish_import(&imp, &applied);
                true
            }
            Err(m) => {
                if let Some(it) = &mut self.item {
                    it.import = ImportPanel::Failed(Problem { message: m, retry: false, retry_at: None, account: false });
                }
                false
            }
        }
    }

    fn finish_import(&mut self, imp: &Imported, applied: &Applied) {
        self.ensure_log();
        let item = &imp.head.item;
        let license = if imp.info.license.is_empty() { item.license.clone() } else { imp.info.license.clone() };
        let rec = ImportedRecord {
            item_id: item.id.clone(),
            bank_id: applied.bank_id.clone(),
            bank_name: applied.bank_name.clone(),
            version: item.version,
            license: license.clone(),
            title: api::plain(&item.title),
            author: text::author(&item.author_name),
            imported_at: imp.at_ms,
            question_ids: imp.questions.iter().map(|q| q.id.clone()).collect(),
            fingerprints: imp.fingerprints.clone(),
            newer: None,
            status: None,
            checked_at: imp.at_ms,
            note: None,
            base: merge::bounded(imp.theirs.iter().map(|t| merge::base_of(&t.q, &t.qid, &t.fp, t.sha.clone())).collect()),
            circle: None,
        };
        // A bank that gets a second import of the same item keeps both lists of questions.
        let mut rec = rec;
        if let Some(old) = self.imported.iter().find(|r| r.item_id == rec.item_id && r.bank_id == rec.bank_id) {
            let mut ids = old.question_ids.clone();
            ids.extend(rec.question_ids.clone());
            rec.question_ids = ids;
            let mut fp = old.fingerprints.clone();
            fp.extend(rec.fingerprints.clone());
            rec.fingerprints = fp;
            let mut base = old.base.clone();
            base.extend(rec.base.clone());
            rec.base = merge::bounded(base);
        }
        self.imported.retain(|r| !(r.item_id == rec.item_id && r.bank_id == rec.bank_id));
        // An update made into a new bank settles the older record's badge.
        if let Some(upd) = self.item.as_ref().and_then(|i| i.update.as_ref()) {
            let (old_item, old_bank) = (upd.record.item_id.clone(), upd.record.bank_id.clone());
            let note = format!("Version {} was imported as the bank \"{}\".", item.version, applied.bank_name);
            for r in self.imported.iter_mut().filter(|r| r.item_id == old_item && r.bank_id == old_bank) {
                r.newer = None;
                r.note = Some(note.clone());
            }
        }
        let previously = imp.info.previously_imported_versions.clone();
        let done = ImportDone {
            title: api::plain(&item.title),
            author: text::author(&item.author_name),
            license,
            version: item.version,
            bank_id: applied.bank_id.clone(),
            bank_name: applied.bank_name.clone(),
            new_bank: applied.new_bank,
            added: applied.added,
            pictures: imp.pictures,
            pictures_missing: imp.pictures_missing,
            keyless: imp.keyless,
            previously,
            may_republish_as: imp.info.may_republish_as.clone(),
            recorded: imp.info.recorded,
            held_pictures_quota: imp.info.images.reason.as_deref() == Some("quota"),
        };
        self.imported.push(rec);
        let saved = self.save_log();
        if let Some(it) = &mut self.item {
            it.import = ImportPanel::Done(Box::new(done));
        }
        if let Err(m) = saved {
            self.notice = Some(Notice::Error(format!("The questions were added, but the list of imports could not be saved ({m}). Updates will not be noticed for this one.")));
        }
    }

    /// What an update of the open item would change, for the bank it came from.
    fn refresh_changes(&mut self, banks: &[QuestionBank]) {
        let Some(it) = &self.item else { return };
        let (Some(upd), Load::Ready(l)) = (&it.update, &it.load) else { return };
        if upd.changes.is_some() {
            return;
        }
        let rec = upd.record.clone();
        let bank = banks.iter().find(|b| b.id == rec.bank_id);
        let mut old = rec.fingerprints.clone();
        let mut added = 0usize;
        let mut unchanged = 0usize;
        for f in &l.fingerprints {
            if let Some(i) = old.iter().position(|o| o == f) {
                old.swap_remove(i);
                unchanged += 1;
            } else {
                added += 1;
            }
        }
        let (mut edited, mut deleted) = (0, 0);
        if let Some(b) = bank {
            for id in &rec.question_ids {
                match b.questions.iter().find(|q| q.id == *id) {
                    Some(q) if q.modified > rec.imported_at => edited += 1,
                    Some(_) => {}
                    None => deleted += 1,
                }
            }
        }
        let new_license = &l.doc.head.item.license;
        let changes = Changes {
            old_version: rec.version,
            new_version: l.doc.head.item.version,
            added,
            removed: old.len(),
            unchanged,
            edited_locally: edited,
            deleted_locally: deleted,
            license: (rec.license != *new_license && !new_license.is_empty()).then(|| (rec.license.clone(), new_license.clone())),
            bank_missing: bank.is_none(),
            nothing_new: false,
        };
        let mut changes = changes;
        changes.nothing_new = !rec.fingerprints.is_empty()
            && changes.added == 0
            && changes.removed == 0
            && changes.license.is_none()
            && changes.new_version > rec.version;
        if changes.nothing_new {
            // The pool's version number moved, the content did not: remember the new number so
            // the update is not offered again (the questions and the merge base are unchanged).
            let (item, bank, v) = (rec.item_id.clone(), rec.bank_id.clone(), changes.new_version);
            for r in self.imported.iter_mut().filter(|r| r.item_id == item && r.bank_id == bank) {
                r.version = v;
                r.newer = None;
            }
            let _ = self.save_log();
        }
        if let Some(u) = self.item.as_mut().and_then(|i| i.update.as_mut()) {
            u.changes = Some(changes);
        }
    }

    // ---- merging a newer version into the bank --------------------------------------------------------------

    /// Why "Merge into this bank..." is not possible for the open update, in words. `None`: it is.
    pub fn merge_blocker(&self) -> Option<String> {
        let it = self.item.as_ref()?;
        let u = it.update.as_ref()?;
        if let Some(why) = self.import_blocker() {
            return Some(why);
        }
        if u.record.base.is_empty() {
            return Some("This bank was imported before verSHion kept a copy of what it imported (or the copy was too big to keep), so it can not tell your changes from the pool's. Add the new version as a new bank instead.".into());
        }
        if u.changes.as_ref().is_some_and(|c| c.bank_missing) {
            return Some(format!("The bank \"{}\" is not there any more, so there is nothing to merge into. Add the new version as a new bank instead.", text::shown(&u.record.bank_name)));
        }
        let new = self.loaded().map_or(0, |l| l.doc.head.item.version);
        if new <= u.record.version {
            return Some("The bank already has this version.".into());
        }
        None
    }

    /// "Merge into this bank..." Builds the review on the next `sync`; nothing is written.
    pub fn begin_merge(&mut self) {
        if self.merge_blocker().is_some() || self.running(JobKind::Import) {
            return;
        }
        if let Some(it) = &mut self.item {
            it.merge = Some(MergeState { phase: MergePhase::Wanted, plan: None, problem: None });
        }
    }

    /// Leaves the review (or the result) without writing anything.
    pub fn cancel_merge(&mut self) {
        if self.merging() {
            self.jobs.retain(|j| j.kind != JobKind::Import);
            self.ready_merge = None;
        }
        if let Some(it) = &mut self.item {
            it.merge = None;
        }
    }

    pub fn merge_plan(&self) -> Option<&MergePlan> {
        self.item.as_ref()?.merge.as_ref()?.plan.as_ref()
    }

    /// Decides a row. Only choices the row offers are accepted.
    pub fn set_merge_choice(&mut self, row: usize, c: Choice) {
        let Some(ms) = self.item.as_mut().and_then(|i| i.merge.as_mut()) else { return };
        if !matches!(ms.phase, MergePhase::Review) {
            return;
        }
        if let Some(r) = ms.plan.as_mut().and_then(|p| p.rows.get_mut(row))
            && r.options.contains(&c)
        {
            r.choice = c;
        }
    }

    /// Whether "Apply merge" is allowed: reviewing, every conflict decided, nothing running.
    pub fn can_apply_merge(&self) -> bool {
        let Some(ms) = self.item.as_ref().and_then(|i| i.merge.as_ref()) else { return false };
        matches!(ms.phase, MergePhase::Review) && ms.plan.as_ref().is_some_and(MergePlan::ready) && !self.running(JobKind::Import)
    }

    /// "Apply merge": fetches the pool's version (with its pictures); the bank is written, after a
    /// restore point, when it arrives. A failure leaves everything as it was.
    pub fn apply_merge(&mut self) {
        if !self.can_apply_merge() {
            return;
        }
        let Some(id) = self.item.as_ref().map(|i| i.id.clone()) else { return };
        if let Some(ms) = self.item.as_mut().and_then(|i| i.merge.as_mut()) {
            ms.phase = MergePhase::Applying;
            ms.problem = None;
        }
        if !self.launch(Work::Import(id))
            && let Some(ms) = self.item.as_mut().and_then(|i| i.merge.as_mut())
        {
            ms.phase = MergePhase::Review;
        }
    }

    fn refresh_merge(&mut self, banks: &[QuestionBank]) {
        let wanted = self.item.as_ref().and_then(|i| i.merge.as_ref()).is_some_and(|m| matches!(m.phase, MergePhase::Wanted));
        if !wanted {
            return;
        }
        let built = (|| {
            let it = self.item.as_ref()?;
            let (u, l) = (it.update.as_ref()?, self.loaded()?);
            let bank = banks.iter().find(|b| b.id == u.record.bank_id);
            let Some(bank) = bank else { return Some(Err("The bank is not there any more.".to_string())) };
            let head = &l.doc.head;
            let theirs = theirs_of(&l.doc.payload, head, now_ms());
            let item = &head.item;
            let license = (u.record.license != item.license && !item.license.is_empty()).then(|| (u.record.license.clone(), item.license.clone()));
            let credit = attribution_line(&item.title, &item.author_name, &item.license, &item.id, item.version);
            Some(Ok(merge::build_plan(&item.id, &u.record.base, bank, &theirs, u.record.version, item.version, license, credit)))
        })();
        if let Some(ms) = self.item.as_mut().and_then(|i| i.merge.as_mut()) {
            match built {
                Some(Ok(p)) => {
                    ms.plan = Some(p);
                    ms.phase = MergePhase::Review;
                }
                Some(Err(m)) => ms.phase = MergePhase::Blocked(m),
                None => ms.phase = MergePhase::Blocked("The item is not loaded.".into()),
            }
        }
    }

    fn back_to_review(&mut self, message: String) {
        if let Some(ms) = self.item.as_mut().and_then(|i| i.merge.as_mut()) {
            ms.phase = MergePhase::Review;
            ms.problem = Some(Problem { message, retry: false, retry_at: None, account: false });
        }
    }

    /// The pool's version has arrived: restore point first, then the bank, then the record. Any
    /// step that fails stops here with nothing changed.
    fn finish_merge(&mut self, banks: &mut [QuestionBank], imp: &Imported) -> bool {
        let Some(plan) = self.merge_plan().cloned() else { return false };
        let Some(rec) = self.item.as_ref().and_then(|i| i.update.as_ref()).map(|u| u.record.clone()) else { return false };
        if imp.head.item.version != plan.new_version {
            self.back_to_review("The pool has a different version now than the one you reviewed. Nothing was applied. Close this and start the merge again.".into());
            return false;
        }
        let Some(pos) = banks.iter().position(|b| b.id == plan.bank_id) else {
            self.back_to_review("The bank is not there any more. Nothing was applied.".into());
            return false;
        };
        let author = text::author(&imp.head.item.author_name);
        let mut bank = banks[pos].clone();
        let applied = match merge::apply_merge(&mut bank, &plan, &imp.theirs, &rec.base, &author, imp.at_ms) {
            Ok(a) => a,
            Err(m) => {
                self.back_to_review(m);
                return false;
            }
        };
        // The restore point holds the banks exactly as they were; if it can not be written, the
        // merge does not happen.
        let restore = match self.restore.save(banks) {
            Ok(w) => w,
            Err(m) => {
                self.back_to_review(format!("A copy of your banks could not be saved first ({m}), so nothing was changed."));
                return false;
            }
        };
        let (outcome, new_base) = applied;
        let item = &imp.head.item;
        let license = if imp.info.license.is_empty() { item.license.clone() } else { imp.info.license.clone() };
        let license_changed = license != rec.license;
        merge::update_credit(&mut bank, &item.id, &plan.new_credit);
        if license_changed && bank.details.license_name == text::license_name(&rec.license) {
            bank.details.license_name = text::license_name(&license);
            bank.details.license_url = text::license_url(&license).to_string();
        }
        let bank_name = bank.name.clone();
        banks[pos] = bank;
        self.ensure_log();
        if let Some(r) = self.imported.iter_mut().find(|r| r.item_id == rec.item_id && r.bank_id == rec.bank_id) {
            r.version = item.version;
            r.license = license;
            r.title = api::plain(&item.title);
            r.author = author;
            r.imported_at = imp.at_ms;
            r.question_ids = new_base.iter().map(|b| b.local_id.clone()).collect();
            r.fingerprints = imp.theirs.iter().map(|t| t.fp.clone()).collect();
            r.base = merge::bounded(new_base);
            r.newer = r.newer.take().filter(|n| n.version > item.version);
            r.status = None;
            r.checked_at = imp.at_ms;
            r.note = Some(format!("Version {} was merged into this bank on {}.", item.version, text::date(imp.at_ms)));
        }
        let saved = self.save_log();
        if let Some(ms) = self.item.as_mut().and_then(|i| i.merge.as_mut()) {
            ms.phase = MergePhase::Done(Box::new(MergeDone { outcome, restore, new_version: item.version, bank_name, license_changed }));
        }
        if let Err(m) = saved {
            self.notice = Some(Notice::Error(format!("The bank was merged, but the list of imports could not be saved ({m}). The update badge may stay until the next check.")));
        }
        true
    }

    // ---- reporting ----------------------------------------------------------------------------------------------

    /// "Report..." (not offered for the person's own item).
    pub fn open_report(&mut self) {
        let own = self.loaded().is_some_and(|l| l.doc.head.item.is_author);
        if own {
            return;
        }
        if let Some(it) = &mut self.item
            && it.report.is_none()
        {
            it.report = Some(ReportForm { choice: None, note: String::new(), state: ReportState::Editing });
        }
    }

    pub fn close_report(&mut self) {
        if self.running(JobKind::Report) {
            self.jobs.retain(|j| j.kind != JobKind::Report);
        }
        if let Some(it) = &mut self.item {
            it.report = None;
        }
    }

    pub fn set_report_choice(&mut self, c: ReportChoice) {
        if let Some(r) = self.item.as_mut().and_then(|i| i.report.as_mut())
            && matches!(r.state, ReportState::Editing | ReportState::Failed(_))
        {
            r.choice = Some(c);
        }
    }

    pub fn set_report_note(&mut self, note: &str) {
        if let Some(r) = self.item.as_mut().and_then(|i| i.report.as_mut()) {
            r.note = note.chars().take(NOTE_MAX).collect();
        }
    }

    /// "The answer key is visible" is offered only for an item served without its key (published
    /// with the key hidden): the Worker refuses the report for any other.
    pub fn key_leak_offered(&self) -> bool {
        self.loaded().is_some_and(|l| served_form(&l.doc.head) == Served::Stripped)
    }

    /// What is missing before the report can be sent.
    pub fn report_blocker(&self) -> Option<&'static str> {
        let r = self.item.as_ref()?.report.as_ref()?;
        match (r.choice, r.note.trim().is_empty()) {
            (None, _) => Some("Choose what is wrong."),
            (Some(ReportChoice::Other), true) => Some("Say what is wrong in the note."),
            (Some(ReportChoice::KeyLeak), _) if !self.key_leak_offered() => Some("An answer key leak can only be reported for an item whose key is hidden."),
            _ => None,
        }
    }

    pub fn send_report(&mut self) {
        if self.report_blocker().is_some() {
            return;
        }
        let Some(it) = &mut self.item else { return };
        let Some(r) = &mut it.report else { return };
        if matches!(r.state, ReportState::Sending | ReportState::Sent(_)) {
            return;
        }
        let (Some(choice), note, id) = (r.choice, r.note.clone(), it.id.clone()) else { return };
        r.state = ReportState::Sending;
        self.launch(Work::Report { id, choice, note });
    }

    fn on_report(&mut self, r: Result<Ack, PoolError>) {
        match r {
            Ok(ack) => {
                if let Some(f) = self.item.as_mut().and_then(|i| i.report.as_mut()) {
                    f.state = ReportState::Sent(ack.message.as_deref().map(api::plain).filter(|m| !m.is_empty()).unwrap_or_else(|| text::THANKS.to_string()));
                }
            }
            Err(e) => {
                let own = matches!(e.kind(), Some(RefusalKind::OwnItem));
                if let Some(mut p) = self.problem_of(&e) {
                    if own {
                        p.message = text::REPORT_OWN.into();
                        p.retry = false;
                    }
                    // The same report may not be sent twice by a retry: the person presses Send again.
                    p.retry = false;
                    if let Some(f) = self.item.as_mut().and_then(|i| i.report.as_mut()) {
                        f.state = ReportState::Failed(p);
                    }
                }
            }
        }
    }

    // ---- update checks ------------------------------------------------------------------------------------------

    /// Asks the pool where each imported item stands (one request for up to 200 items).
    pub fn check_updates(&mut self) {
        self.ensure_log();
        let mut ids: Vec<String> = self.imported.iter().filter(|r| r.circle.is_none()).map(|r| r.item_id.clone()).collect();
        ids.sort();
        ids.dedup();
        if ids.is_empty() {
            return;
        }
        self.check_problem = None;
        self.launch(Work::Versions(ids));
    }

    pub fn checking(&self) -> bool {
        self.running(JobKind::Versions)
    }

    /// The circle banks that were copied (each once), for the circles page to ask about.
    pub fn circle_bank_ids(&mut self) -> Vec<String> {
        self.ensure_log();
        let mut ids: Vec<String> = self.imported.iter().filter(|r| r.circle.is_some()).map(|r| r.item_id.clone()).collect();
        ids.sort();
        ids.dedup();
        ids.truncate(super::circles_api::MAX_HEADS);
        ids
    }

    /// Notes what the circles page learned about the banks copied from circles: a newer version, or
    /// that the bank can not be shown any more (the copy is unaffected).
    pub fn apply_circle_heads(&mut self, entries: &[super::circles_api::HeadEntry]) {
        use super::circles_api::HeadState;
        self.ensure_log();
        let now = now_ms();
        let mut changed = false;
        for r in self.imported.iter_mut().filter(|r| r.circle.is_some()) {
            let Some(e) = entries.iter().find(|e| e.bank_id == r.item_id) else { continue };
            r.checked_at = now;
            r.status = None;
            match &e.state {
                HeadState::Current { version, license, .. } => {
                    r.newer = (*version > r.version).then(|| NewerInfo { version: *version, license: license.clone(), license_version: None });
                }
                HeadState::Removed { .. } => r.status = Some("removed".into()),
                HeadState::UnderReview => r.status = Some("under_review".into()),
                HeadState::Unavailable => r.status = Some("unavailable".into()),
            }
            changed = true;
        }
        if changed {
            let _ = self.save_log();
        }
    }

    /// A working copy moved on (a send or a merge): the list of imports follows.
    pub fn set_circle_version(&mut self, item_id: &str, bank_id: &str, version: u64) {
        self.ensure_log();
        if let Some(r) = self.imported.iter_mut().find(|r| r.item_id == item_id && r.bank_id == bank_id && r.circle.is_some()) {
            r.version = version;
            r.newer = r.newer.take().filter(|n| n.version > version);
            let _ = self.save_log();
        }
    }

    /// Records a copy of a circle bank like an import, so it shows under "Things I imported".
    pub fn record_circle_copy(&mut self, imp: &Imported, applied: &Applied, circle: CircleOrigin) -> Result<(), String> {
        self.ensure_log();
        let item = &imp.head.item;
        let license = if imp.info.license.is_empty() { item.license.clone() } else { imp.info.license.clone() };
        let rec = ImportedRecord {
            item_id: item.id.clone(),
            bank_id: applied.bank_id.clone(),
            bank_name: applied.bank_name.clone(),
            version: item.version,
            license,
            title: api::plain(&item.title),
            author: text::author(&item.author_name),
            imported_at: imp.at_ms,
            question_ids: imp.questions.iter().map(|q| q.id.clone()).collect(),
            fingerprints: imp.fingerprints.clone(),
            newer: None,
            status: None,
            checked_at: imp.at_ms,
            note: None,
            base: merge::bounded(imp.theirs.iter().map(|t| merge::base_of(&t.q, &t.qid, &t.fp, t.sha.clone())).collect()),
            circle: Some(circle),
        };
        self.imported.retain(|r| !(r.item_id == rec.item_id && r.bank_id == rec.bank_id));
        self.imported.push(rec);
        self.save_log()
    }

    fn on_versions(&mut self, r: Result<Vec<VersionEntry>, PoolError>) {
        let entries = match r {
            Ok(e) => e,
            Err(e) => {
                if let Some(p) = self.problem_of(&e) {
                    self.check_problem = Some(p);
                }
                return;
            }
        };
        let now = now_ms();
        for r in &mut self.imported {
            let Some(e) = entries.iter().find(|e| e.id == r.item_id) else { continue };
            r.checked_at = now;
            r.status = None;
            match &e.state {
                VersionState::Current { .. } => r.newer = None,
                VersionState::Newer { version, license, license_version } => {
                    r.newer = Some(NewerInfo { version: *version, license: license.clone(), license_version: *license_version });
                }
                VersionState::UnderReview { .. } => r.status = Some("under_review".into()),
                VersionState::Removed { .. } => r.status = Some("removed".into()),
                VersionState::Unavailable => r.status = Some("unavailable".into()),
            }
        }
        if let Err(m) = self.save_log() {
            self.check_problem = Some(Problem { message: format!("The result of the check could not be saved ({m})."), retry: false, retry_at: None, account: false });
        }
    }
}
