//! Working copies: a local bank linked to a circle bank. The link is kept in the side file
//! `share-circles.json` (never in `banks.json`), so a bank file shared or exported carries nothing of
//! it. No egui and no network here: the record, the payload to send, and the count of what changed.
//!
//! A link holds what a three-way merge needs: the version the copy was last in step with
//! (`base_version`), what each linked question was then (`base`, bounded like an import's), and which
//! local question is which circle question (`qids`: ids only, so a send keeps every question's
//! identity even when the snapshot was too big to keep).

use std::sync::{Arc, Mutex};

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use super::browse::theirs_of;
use super::circles_api::{BankHead, SentBank};
use super::merge::{self, BaseQuestion, Theirs};
use super::pool::{ItemHead, ItemMeta, ItemView, License, is_item_id};
use super::publish::QidLink;
use crate::model::{Question, QuestionBank};

pub const LINKS_DOC: &str = "share-circles.json";

/// The send payload's schema: 2 is the one that carries `published.qid`.
pub const SEND_SCHEMA: u32 = 2;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct CircleLink {
    pub circle_id: String,
    #[serde(default)]
    pub circle_name: String,
    pub circle_bank_id: String,
    pub local_bank_id: String,
    #[serde(default)]
    pub title: String,
    #[serde(default)]
    pub subject: String,
    #[serde(default)]
    pub course: String,
    #[serde(default)]
    pub grade: String,
    /// The bank's licence (wire form): a send goes with it; only the bank's author changes it.
    #[serde(default)]
    pub license: String,
    /// The version this copy was last in step with, and that version's id.
    pub base_version: u64,
    #[serde(default)]
    pub base_version_id: String,
    #[serde(default)]
    pub qids: Vec<QidLink>,
    /// Empty when too big to keep: then a merge reads the base from the circle, or is two-way.
    #[serde(default)]
    pub base: Vec<BaseQuestion>,
    /// The newest version the last quiet check saw, and who sent it and when.
    #[serde(default)]
    pub head_seen: u64,
    #[serde(default)]
    pub head_seen_at: Option<i64>,
    #[serde(default)]
    pub head_seen_by: Option<String>,
    pub linked_at: i64,
    /// The local bank's `modified` when this link was last written with the bank (a copy, a merge, a
    /// send). A bank OLDER than this was put back from a saved copy: the link is ahead of it.
    #[serde(default)]
    pub bank_modified: i64,
    /// A fingerprint of the bank's questions as the app last wrote them with this link. A bank whose
    /// content is the same is never "put back"; one that is older AND different is.
    #[serde(default)]
    pub bank_print: String,
}

impl CircleLink {
    /// Linked from a finished copy of a circle bank (`b` is the head it was made from).
    pub fn from_head(circle_id: &str, circle_name: &str, b: &BankHead, local_bank_id: &str, base: Vec<BaseQuestion>, now: i64) -> CircleLink {
        let qids = qids_of(&base);
        CircleLink {
            circle_id: circle_id.to_string(),
            circle_name: circle_name.to_string(),
            circle_bank_id: b.id.clone(),
            local_bank_id: local_bank_id.to_string(),
            title: b.title.clone(),
            subject: b.subject.clone(),
            course: b.course.clone(),
            grade: b.grade.clone(),
            license: if b.license.is_empty() { b.version_license.clone() } else { b.license.clone() },
            base_version: b.version,
            base_version_id: b.version_id.clone(),
            qids,
            base: merge::bounded(base),
            head_seen: b.version,
            head_seen_at: Some(b.committed_at).filter(|a| *a > 0),
            head_seen_by: b.committed_by.clone(),
            linked_at: now,
            bank_modified: 0,
            bank_print: String::new(),
        }
    }

    pub fn meta(&self) -> ItemMeta {
        ItemMeta { title: self.title.chars().take(200).collect(), subject: self.subject.clone(), course: self.course.clone(), grade: self.grade.clone(), ..ItemMeta::default() }
    }

    pub fn licence(&self) -> Option<License> {
        License::parse(&self.license)
    }

    /// The bank went back in time (a saved copy of the banks was put back) while the link stayed
    /// at the newer version: the link no longer says what the bank was last in step with.
    pub fn stale_for(&self, bank: &QuestionBank) -> bool {
        // Bank times are seconds. A value this large is milliseconds written by an earlier build.
        let at = if self.bank_modified > 100_000_000_000 { self.bank_modified / 1000 } else { self.bank_modified };
        // The same for the bank: an earlier build wrote milliseconds into it.
        let bank_at = if bank.modified > 100_000_000_000 { bank.modified / 1000 } else { bank.modified };
        if at <= 0 || bank_at >= at {
            return false;
        }
        // Older: stale only when the content is not what this app wrote (a file put back).
        self.bank_print.is_empty() || bank_print(bank) != self.bank_print
    }

    /// Says the bank as it is now is what this link is in step with (after a copy, merge or send).
    pub fn mark_bank(&mut self, bank: &QuestionBank) {
        self.bank_modified = bank.modified;
        self.bank_print = bank_print(bank);
    }

    /// How many versions the circle has that this copy has not seen.
    pub fn behind(&self) -> u64 {
        self.head_seen.saturating_sub(self.base_version)
    }

    /// The circle's question id for a local question.
    pub fn qid_of(&self, local: &str) -> Option<&str> {
        self.qids.iter().find(|q| q.local == local).map(|q| q.qid.as_str())
    }
}

/// `(local id, circle id)` for each base question that has one.
pub fn qids_of(base: &[BaseQuestion]) -> Vec<QidLink> {
    base.iter().filter(|b| !b.qid.is_empty()).map(|b| QidLink { local: b.local_id.clone(), qid: b.qid.clone() }).collect()
}

// ---- what changed since the copy was in step -----------------------------------------------------------

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ChangeCount {
    pub added: usize,
    pub changed: usize,
    pub removed: usize,
    /// False when the snapshot was not kept: the changes can not be counted, only sent.
    pub known: bool,
}

impl ChangeCount {
    pub fn total(&self) -> usize {
        self.added + self.changed + self.removed
    }
    pub fn any(&self) -> bool {
        self.total() > 0 || !self.known
    }
}

/// Compares the local bank with what it was when last in step.
pub fn changes_since(link: &CircleLink, bank: &QuestionBank) -> ChangeCount {
    let mut c = ChangeCount { known: !link.base.is_empty() || link.qids.is_empty(), ..ChangeCount::default() };
    for q in &bank.questions {
        match link.qid_of(&q.id) {
            None => c.added += 1,
            Some(_) => match link.base.iter().find(|b| b.local_id == q.id) {
                Some(b) => {
                    if merge::ours_changed(b, q) {
                        c.changed += 1;
                    }
                }
                None => c.known = false,
            },
        }
    }
    c.removed = link.qids.iter().filter(|l| !bank.questions.iter().any(|q| q.id == l.local)).count();
    c
}

// ---- the payload ----------------------------------------------------------------------------------------

/// The bank as sent: its JSON, and for each question the circle already knows, `published.qid` so it
/// keeps its identity (an unchanged question is not counted as changed, nor is its credit lost).
pub fn payload_of(link: &CircleLink, bank: &QuestionBank) -> Result<Vec<u8>, String> {
    if bank.questions.is_empty() {
        return Err("The bank has no questions. A circle bank needs at least one.".into());
    }
    let mut v = serde_json::to_value(bank).map_err(|e| e.to_string())?;
    let pid = if super::pool::is_publish_id(&link.base_version_id) { link.base_version_id.clone() } else { link.circle_bank_id.clone() };
    if let Some(qs) = v.get_mut("questions").and_then(Value::as_array_mut) {
        for q in qs {
            let local = q.get("id").and_then(Value::as_str).unwrap_or("").to_string();
            if let Some(qid) = link.qid_of(&local).filter(|id| is_item_id(id))
                && let Some(o) = q.as_object_mut()
            {
                o.insert("published".into(), json!({ "item_id": link.circle_bank_id, "qid": qid, "publish_id": pid, "version": link.base_version.min(u64::from(u32::MAX)) }));
            }
        }
    }
    Ok(format!("{{\"schema_version\":{SEND_SCHEMA},\"bank\":{v}}}").into_bytes())
}

/// The picture's name on the circle (a hash) for a local file `media/<sha>.<ext>`.
pub fn sha_of_file(file: &str) -> Option<String> {
    file.strip_prefix("media/").and_then(|r| r.split_once('.')).map(|(s, _)| s.to_string())
}

fn base_for(q: &Question, qid: &str) -> BaseQuestion {
    merge::base_of(q, qid, "", q.image.as_ref().and_then(|i| sha_of_file(&i.file)))
}

/// The link after `sent` went through: the bank as sent is now what the circle has.
pub fn after_send(link: &CircleLink, bank: &QuestionBank, sent: &SentBank) -> CircleLink {
    let mut l = link.clone();
    let base: Vec<BaseQuestion> = if sent.qids.len() == bank.questions.len() { bank.questions.iter().zip(&sent.qids).map(|(q, id)| base_for(q, id)).collect() } else { Vec::new() };
    l.qids = if base.is_empty() { Vec::new() } else { qids_of(&base) };
    l.base = merge::bounded(base);
    l.base_version = sent.version;
    l.base_version_id = sent.version_id.clone();
    l.head_seen = l.head_seen.max(sent.version);
    l.mark_bank(bank);
    l
}

// ---- where the links are kept ------------------------------------------------------------------------------

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
struct LinkFile {
    #[serde(default)]
    links: Vec<CircleLink>,
}

/// Where the links are kept: the app's store, a folder (tests: real files, a crash can be mimicked),
/// or a shared value (tests).
#[derive(Clone, Debug)]
pub enum LinkLog {
    Store,
    #[cfg(not(target_arch = "wasm32"))]
    Dir(std::path::PathBuf),
    Memory(Arc<Mutex<Vec<CircleLink>>>),
}

/// The links that parse; an entry that does not is skipped and the file's text is kept as `.bad`.
fn parse_links(bytes: &[u8]) -> (Vec<CircleLink>, bool) {
    let Ok(v) = serde_json::from_slice::<Value>(bytes) else { return (Vec::new(), true) };
    let Some(list) = v.get("links").and_then(Value::as_array) else { return (Vec::new(), v.get("links").is_some()) };
    let mut out = Vec::new();
    let mut bad = false;
    for e in list {
        match serde_json::from_value::<CircleLink>(e.clone()) {
            Ok(l) if is_item_id(&l.circle_id) && is_item_id(&l.circle_bank_id) => out.push(l),
            _ => bad = true,
        }
    }
    (out, bad)
}

impl LinkLog {
    pub fn memory() -> LinkLog {
        LinkLog::Memory(Arc::default())
    }

    fn read(&self) -> Option<Vec<u8>> {
        match self {
            LinkLog::Store => crate::store::with_backend(|b| b.read_doc(LINKS_DOC)),
            #[cfg(not(target_arch = "wasm32"))]
            LinkLog::Dir(d) => crate::store::Backend::read_doc(&crate::store::FsBackend::new(d.clone()), LINKS_DOC),
            LinkLog::Memory(_) => None,
        }
    }

    fn write(&self, name: &str, bytes: &[u8]) -> Result<(), String> {
        match self {
            LinkLog::Store => crate::store::with_backend(|b| b.write_doc(name, bytes)),
            #[cfg(not(target_arch = "wasm32"))]
            LinkLog::Dir(d) => crate::store::Backend::write_doc(&crate::store::FsBackend::new(d.clone()), name, bytes),
            LinkLog::Memory(_) => Ok(()),
        }
    }

    fn exists(&self) -> bool {
        match self {
            LinkLog::Store => crate::store::with_backend(|b| b.exists(LINKS_DOC)),
            #[cfg(not(target_arch = "wasm32"))]
            LinkLog::Dir(d) => crate::store::Backend::exists(&crate::store::FsBackend::new(d.clone()), LINKS_DOC),
            LinkLog::Memory(_) => false,
        }
    }

    fn read_named(&self, name: &str) -> Option<Vec<u8>> {
        match self {
            LinkLog::Store => crate::store::with_backend(|b| b.read_doc(name)),
            #[cfg(not(target_arch = "wasm32"))]
            LinkLog::Dir(d) => crate::store::Backend::read_doc(&crate::store::FsBackend::new(d.clone()), name),
            LinkLog::Memory(_) => None,
        }
    }

    /// Keeps the file as it was: the first copy is `.bad`, later different ones `.bad.2` to `.bad.9`.
    /// A copy that is already kept is not written again.
    fn keep_bad(&self, bytes: &[u8]) {
        for n in 1..=9 {
            let name = if n == 1 { format!("{LINKS_DOC}.bad") } else { format!("{LINKS_DOC}.bad.{n}") };
            match self.read_named(&name) {
                Some(old) if old == bytes => return,
                Some(_) => continue,
                None => {
                    let _ = self.write(&name, bytes);
                    return;
                }
            }
        }
    }

    /// The links. One entry that can not be read does not drop the others: it is skipped, and the
    /// file as it was is kept beside it as `share-circles.json.bad` before anything is written over it.
    pub fn load(&self) -> Vec<CircleLink> {
        self.load_checked().unwrap_or_default()
    }

    /// Like [`load`](Self::load), but a file that is there and could not be read is an error, not "no
    /// links": the caller must not write over it.
    pub fn load_checked(&self) -> Result<Vec<CircleLink>, String> {
        if let LinkLog::Memory(m) = self {
            return Ok(m.lock().unwrap_or_else(|p| p.into_inner()).clone());
        }
        let Some(bytes) = self.read() else {
            return if self.exists() { Err("the file could not be read just now".into()) } else { Ok(Vec::new()) };
        };
        let (links, bad) = parse_links(&bytes);
        if bad {
            self.keep_bad(&bytes);
        }
        Ok(links)
    }

    pub fn save(&self, links: &[CircleLink]) -> Result<(), String> {
        match self {
            LinkLog::Memory(m) => {
                *m.lock().unwrap_or_else(|p| p.into_inner()) = links.to_vec();
                Ok(())
            }
            _ => {
                let bytes = serde_json::to_vec(&LinkFile { links: links.to_vec() }).map_err(|e| e.to_string())?;
                // The backend writes beside the name and renames into place: a crash leaves the old file or the new, never half.
                self.write(LINKS_DOC, &bytes)
            }
        }
    }
}

/// What the bank's questions are right now: noticed edits, and a bank put back from a saved copy.
pub fn bank_print(bank: &QuestionBank) -> String {
    super::pool::sha256_hex(&serde_json::to_vec(&bank.questions).unwrap_or_default())
}

/// How many questions the payload holds, readable or not.
pub fn payload_question_count(payload: &[u8]) -> usize {
    serde_json::from_slice::<Value>(payload).ok().and_then(|v| v.pointer("/bank/questions").and_then(Value::as_array).map(Vec::len)).unwrap_or(0)
}

/// A circle bank's questions in this app's shape (pictures left for the caller). The circle's id for
/// a question is the `id` it was served with; it stays the same across versions.
pub fn circle_theirs(payload: &[u8], at_ms: i64) -> Vec<Theirs> {
    circle_theirs_as(payload, at_ms, false)
}

/// The same for a read in the viewers' form (`keys_hidden`, phase C): every question is marked as having
/// no key, whether or not the question says so itself.
pub fn circle_theirs_as(payload: &[u8], at_ms: i64, keys_hidden: bool) -> Vec<Theirs> {
    let ids: Vec<String> = serde_json::from_slice::<Value>(payload)
        .ok()
        .and_then(|v| v.pointer("/bank/questions").and_then(Value::as_array).map(|qs| qs.iter().map(|q| q.get("id").and_then(Value::as_str).unwrap_or("").to_string()).collect()))
        .unwrap_or_default();
    // A circle bank is served whole, unless it is the viewers' form of a bank that hides its keys.
    let head = ItemHead { item: ItemView { form: if keys_hidden { "stripped" } else { "full" }.into(), ..ItemView::default() }, ..ItemHead::default() };
    let mut theirs = theirs_of(payload, &head, at_ms);
    for t in &mut theirs {
        if let Some(id) = ids.get(t.index).filter(|i| is_item_id(i)) {
            t.qid = id.clone();
        }
    }
    theirs
}
