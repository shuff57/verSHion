//! Phase D2 (plan 4.5, 4.6, 5.3-5.5): one question's history in its circle, comparing two of its versions, and
//! restoring one question. No egui. The timeline is built from the bank's history lines (`circle_versions`) and the
//! change lists (`circle_changes`); compare and restore read version bodies on demand, held in [`HistoryCache`] (4
//! bodies, dropped for another account: the critic 60/61 rule). History lines stay raw (drawn through `api::plain`);
//! compare fields are worded like `merge::FieldDiff` (already plain).

use std::sync::Arc;

use serde_json::{Value, json};

use super::browse::{ImageSink, RestoreSink};
use super::canon;
use super::circle_links::{CircleLink, circle_theirs_as, payload_question_count, sha_of_file};
use super::circle_versions_api::{ChangeEntry, ChangesPage, QuestionChange, VersionLine};
use super::merge::{self, Theirs};
use super::pool::{LocalProblem, Pool, PoolError};
use super::publish_text::shown;
use crate::model::{Answer, ImageRef, Kind, PartKind, Question, QuestionBank, is_keyless};

// ---- the timeline (plan 4.5) ------------------------------------------------------------------------------------------

/// What happened to the question in a version. `AsItWas`: the oldest version the lists reach, which has it (plan 13.5:
/// "as it was then").
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Tag {
    Added,
    Changed,
    Removed,
    AsItWas,
}

impl From<QuestionChange> for Tag {
    fn from(c: QuestionChange) -> Tag {
        match c {
            QuestionChange::Added => Tag::Added,
            QuestionChange::Changed => Tag::Changed,
            QuestionChange::Removed => Tag::Removed,
        }
    }
}

/// One run of versions whose changes to the question can not be told apart: between `older` and `newer` lie versions
/// held back for review (`held`) or no longer kept (`not_kept`), or a list could not be worked out (`unreadable`).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct GapRun {
    pub newer: u64,
    pub older: u64,
    pub held: Vec<u64>,
    pub not_kept: Vec<u64>,
    pub unreadable: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Row {
    /// A kept, readable version. `tag: None`: the question did not change in it, or (a viewer of a bank hiding its
    /// keys) that is not told. `compared_to`: the version its list (or the default compare) is against.
    Version { version: u64, line: Option<VersionLine>, tag: Option<Tag>, compared_to: Option<u64>, compare: bool, restore: bool },
    /// Held back for review: no details, never compared (plan 5.4).
    HeldBack { version: u64, line: Option<VersionLine> },
    Gap(GapRun),
    /// No longer kept in full: who, when and the message only ("Show every version").
    LinesOnly { line: VersionLine },
    /// The default view's one line for those: "Versions 1-2 are no longer kept in full: who and when only."
    Older { from: u64, to: u64, count: usize },
}

/// The "Your copy" line, from the link's base (no network).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum YourCopy {
    /// Changed since the version the copy is based on, not sent yet.
    Unsent,
    /// The same as that version.
    SameAs(u64),
    /// New in this copy (the circle does not know it).
    NewHere,
    /// Deleted from this copy, not sent yet.
    DeletedHere,
    /// The bank was put back from a safety copy: what it matches is not known.
    Stale,
    /// The link kept no snapshot of it: can not be told.
    Unknown,
}

pub struct TimelineIn<'a> {
    /// The history lines read so far (any order).
    pub lines: &'a [VersionLine],
    pub changes: &'a ChangesPage,
    pub link: &'a CircleLink,
    pub bank: &'a QuestionBank,
    /// The question in the bank (its circle id comes from the link's id pairs).
    pub local_id: &'a str,
    /// "Show every version".
    pub show_all: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Timeline {
    /// The circle's id for the question; `None`: the circle does not know it, and there is no history.
    pub qid: Option<String>,
    /// False for a viewer of a bank that hides its keys: bank lines only (Q4).
    pub per_question: bool,
    pub your_copy: YourCopy,
    pub rows: Vec<Row>,
}

/// Restore is offered on edit links only (not a follow link, not one made from the viewers' form).
pub fn restore_offered(link: &CircleLink) -> bool {
    link.can_send()
}

pub fn your_copy(link: &CircleLink, bank: &QuestionBank, local_id: &str) -> YourCopy {
    if link.stale_for(bank) {
        return YourCopy::Stale;
    }
    match (bank.questions.iter().find(|q| q.id == local_id), link.qid_of(local_id)) {
        (None, Some(_)) => YourCopy::DeletedHere,
        (None, None) => YourCopy::Unknown,
        (Some(_), None) => YourCopy::NewHere,
        (Some(q), Some(_)) => match link.base.iter().find(|b| b.local_id == local_id) {
            Some(b) if merge::ours_changed(b, q) => YourCopy::Unsent,
            Some(_) => YourCopy::SameAs(link.base_version),
            None => YourCopy::Unknown,
        },
    }
}

pub fn your_copy_text(y: YourCopy) -> String {
    match y {
        YourCopy::Unsent => "Your copy: changed, not sent yet.".into(),
        YourCopy::SameAs(n) => format!("Your copy: the same as version {n}."),
        YourCopy::NewHere => "Your copy: new here, not sent yet.".into(),
        YourCopy::DeletedHere => "Your copy: deleted here, not sent yet.".into(),
        YourCopy::Stale => "Your copy: put back from a safety copy, so which version it matches is not known.".into(),
        YourCopy::Unknown => "Your copy: can't be compared with the circle's on this computer.".into(),
    }
}

fn versions_words(v: &[u64]) -> String {
    let s: Vec<String> = v.iter().map(u64::to_string).collect();
    match s.as_slice() {
        [] => String::new(),
        [one] => format!("version {one}"),
        [rest @ .., last] => format!("versions {} and {last}", rest.join(", ")),
    }
}

/// "Changes between version 6 and version 8: version 7 is held back."
pub fn gap_text(g: &GapRun) -> String {
    let mut parts = Vec::new();
    if !g.held.is_empty() {
        parts.push(format!("{} {} held back", versions_words(&g.held), if g.held.len() == 1 { "is" } else { "are" }));
    }
    if !g.not_kept.is_empty() {
        parts.push(format!("{} {} no longer kept", versions_words(&g.not_kept), if g.not_kept.len() == 1 { "is" } else { "are" }));
    }
    if g.unreadable {
        parts.push("some changes can't be shown".into());
    }
    if parts.is_empty() {
        return format!("Changes between version {} and version {} can't be shown.", g.older, g.newer);
    }
    format!("Changes between version {} and version {}: {}.", g.older, g.newer, parts.join("; "))
}

fn readable_line(l: &VersionLine) -> bool {
    l.kept && !l.withheld
}

struct Lines(Vec<VersionLine>);

impl Lines {
    fn of(lines: &[VersionLine]) -> Lines {
        let mut l = lines.to_vec();
        l.sort_by_key(|a| std::cmp::Reverse(a.version));
        l.dedup_by_key(|x| x.version);
        Lines(l)
    }
    fn get(&self, v: u64) -> Option<&VersionLine> {
        self.0.iter().find(|l| l.version == v)
    }
    /// Kept and readable, as far as the lines say (a version with no line read yet is taken as the lists say).
    fn readable(&self, v: u64) -> bool {
        self.get(v).is_none_or(readable_line)
    }
    fn lower_readable(&self, v: u64) -> Option<u64> {
        self.0.iter().find(|l| l.version < v && readable_line(l)).map(|l| l.version)
    }
}

/// The page's entries, newest first, with the lines' word over theirs: an entry for a version the lines say is held
/// back or not kept is dropped, and one compared with such a version is a gap against the next readable one (a
/// withheld version is never one side of a comparison, plan 5.4).
fn sanitized(entries: &[ChangeEntry], lines: &Lines) -> Vec<ChangeEntry> {
    let mut out: Vec<ChangeEntry> = Vec::new();
    let mut sorted = entries.to_vec();
    sorted.sort_by_key(|a| std::cmp::Reverse(a.version));
    for e in sorted {
        if !lines.readable(e.version) || out.iter().any(|o| o.version == e.version) {
            continue;
        }
        if lines.readable(e.compared_to) {
            out.push(e);
        } else if let Some(to) = lines.lower_readable(e.compared_to) {
            out.push(ChangeEntry { version: e.version, compared_to: to, gap: true, ..ChangeEntry::default() });
        }
    }
    out
}

fn push_gap(rows: &mut Vec<Row>, g: GapRun) {
    if let Some(Row::Gap(last)) = rows.last_mut()
        && last.older == g.newer
    {
        last.older = g.older;
        last.held.extend(g.held);
        last.not_kept.extend(g.not_kept);
        last.unreadable |= g.unreadable;
        return;
    }
    rows.push(Row::Gap(g));
}

fn older_row(older: Option<(u64, u64, usize)>) -> Option<Row> {
    older.map(|(from, to, count)| Row::Older { from, to, count })
}

fn note_older(older: &mut Option<(u64, u64, usize)>, v: u64) {
    *older = Some(match *older {
        None => (v, v, 1),
        Some((from, to, n)) => (from.min(v), to.max(v), n + 1),
    });
}

/// A viewer of a bank that hides its keys: the bank's lines, each kept one comparable with the kept one below it.
fn bank_lines(lines: &Lines, show_all: bool) -> Vec<Row> {
    let mut rows = Vec::new();
    let mut older = None;
    for l in &lines.0 {
        if l.withheld {
            rows.push(Row::HeldBack { version: l.version, line: Some(l.clone()) });
        } else if l.kept {
            rows.push(Row::Version { version: l.version, line: Some(l.clone()), tag: None, compared_to: lines.lower_readable(l.version), compare: true, restore: false });
        } else if show_all {
            rows.push(Row::LinesOnly { line: l.clone() });
        } else {
            note_older(&mut older, l.version);
        }
    }
    rows.extend(older_row(older));
    rows
}

pub fn timeline(i: &TimelineIn) -> Timeline {
    let qid = i.link.qid_of(i.local_id).map(str::to_string);
    let your_copy = your_copy(i.link, i.bank, i.local_id);
    let per_question = i.changes.per_question;
    let lines = Lines::of(i.lines);
    let rows = match &qid {
        None => Vec::new(),
        Some(_) if !per_question => bank_lines(&lines, i.show_all),
        Some(q) => question_rows(i, &lines, q),
    };
    Timeline { qid, per_question, your_copy, rows }
}

fn question_rows(i: &TimelineIn, lines: &Lines, qid: &str) -> Vec<Row> {
    let restore_ok = restore_offered(i.link);
    let entries = sanitized(&i.changes.versions, lines);
    let top = entries.first().map_or(0, |e| e.version);
    let oldest = entries.last().map(|e| e.compared_to).or_else(|| lines.0.iter().find(|l| readable_line(l)).map(|l| l.version));
    let added_at = entries.iter().find(|e| e.touches(qid) == Some(QuestionChange::Added)).map(|e| e.version);
    // The oldest version the lists reach has the question when it is known to be there at some version `p` above it,
    // with no add since and no gap between: a changed or removed entry says it was in the version compared with, and
    // the link says it was in its base.
    let mut evidence: Vec<u64> = entries.iter().filter(|e| matches!(e.touches(qid), Some(QuestionChange::Changed | QuestionChange::Removed))).map(|e| e.compared_to).collect();
    if i.link.qid_of(i.local_id) == Some(qid) {
        evidence.push(i.link.base_version);
    }
    let as_it_was = match (oldest, added_at) {
        (Some(o), None) => evidence.iter().filter(|p| **p >= o).min().is_some_and(|p| !entries.iter().any(|e| e.gap && e.version > o && e.version <= *p)),
        _ => false,
    };
    let covered = |v: u64| entries.iter().any(|e| e.compared_to < v && v < e.version);
    let mut all: Vec<u64> = lines.0.iter().map(|l| l.version).chain(entries.iter().flat_map(|e| [e.version, e.compared_to])).filter(|v| *v > 0).collect();
    all.sort_unstable_by(|a, b| b.cmp(a));
    all.dedup();
    let mut rows = Vec::new();
    let mut older = None;
    // past the version that added the question: nothing older is about it (the default view stops listing)
    let mut done = false;
    for v in all {
        let line = lines.get(v).cloned();
        if let Some(e) = entries.iter().find(|e| e.version == v) {
            let tag = if e.gap { None } else { e.touches(qid).map(Tag::from) };
            if tag.is_some() || (i.show_all && !(done && e.gap)) {
                rows.push(Row::Version { version: v, line, tag, compared_to: Some(e.compared_to), compare: true, restore: restore_ok && matches!(tag, Some(Tag::Added | Tag::Changed)) });
            }
            if !done && e.gap {
                push_gap(&mut rows, GapRun { newer: v, older: e.compared_to, unreadable: true, ..GapRun::default() });
            }
            if !done && e.compared_to + 1 < v {
                let between = |f: fn(&VersionLine) -> bool| lines.0.iter().filter(|l| l.version > e.compared_to && l.version < v && f(l)).map(|l| l.version).rev().collect::<Vec<_>>();
                push_gap(&mut rows, GapRun { newer: v, older: e.compared_to, held: between(|l| l.withheld), not_kept: between(|l| !l.kept && !l.withheld), unreadable: false });
            }
            done |= tag == Some(Tag::Added);
            continue;
        }
        if Some(v) == oldest && lines.readable(v) {
            let tag = (as_it_was && !done).then_some(Tag::AsItWas);
            if tag.is_some() || i.show_all {
                rows.push(Row::Version { version: v, line, tag, compared_to: None, compare: true, restore: restore_ok && tag.is_some() });
            }
            continue;
        }
        let Some(l) = line else { continue };
        if v > top && readable_line(&l) {
            // sent after the lists were read: not known yet whether it changed the question
            rows.push(Row::Version { version: v, compared_to: lines.lower_readable(v), line: Some(l), tag: None, compare: true, restore: false });
        } else if l.withheld {
            if i.show_all || (!done && !covered(v)) {
                rows.push(Row::HeldBack { version: v, line: Some(l) });
            }
        } else if i.show_all {
            if l.kept {
                rows.push(Row::Version { version: v, compared_to: lines.lower_readable(v), line: Some(l), tag: None, compare: true, restore: false });
            } else {
                rows.push(Row::LinesOnly { line: l });
            }
        } else if !covered(v) {
            note_older(&mut older, v);
        }
    }
    rows.extend(older_row(older));
    rows
}

/// Whether the circle's newest version still has the question, by the newest entry about it: `Some(true)` removed,
/// `Some(false)` not removed in what the lists reach, `None` not known (a gap above, or no lists).
pub fn removed_at_head(page: &ChangesPage, qid: &str) -> Option<bool> {
    if !page.per_question {
        return None;
    }
    for e in &page.versions {
        if e.gap {
            return None;
        }
        match e.touches(qid) {
            Some(QuestionChange::Removed) => return Some(true),
            Some(_) => return Some(false),
            None => {}
        }
    }
    Some(false)
}

// ---- version bodies, the cache and the reads (plan 5.3, 5.5) -------------------------------------------------------

/// A kept version's questions in this app's shape (pictures not fetched), by the circle's question id.
#[derive(Clone, Debug)]
pub struct VersionBody {
    pub version: u64,
    /// Read in the viewers' form (no keys): compared as such, never restored from.
    pub keys_hidden: bool,
    pub questions: Vec<Theirs>,
}

impl VersionBody {
    pub fn from_payload(version: u64, payload: &[u8], keys_hidden: bool, at_ms: i64) -> VersionBody {
        VersionBody { version, keys_hidden, questions: circle_theirs_as(payload, at_ms, keys_hidden) }
    }

    pub fn question(&self, qid: &str) -> Option<&Theirs> {
        self.questions.iter().find(|t| !t.qid.is_empty() && t.qid == qid)
    }
}

/// The most bodies held (plan 5.3).
pub const BODY_CACHE_MAX: usize = 4;

type BodyKey = (String, String, u64);

/// Version bodies read for compare and restore, in memory only, for one account: another account (or a sign-out,
/// [`HistoryCache::forget`]) drops them. The least recently used goes first.
#[derive(Default)]
pub struct HistoryCache {
    account: Option<String>,
    bodies: Vec<(BodyKey, Arc<VersionBody>)>,
}

impl HistoryCache {
    /// The account signed in now (`bank_links::account_tag`): bodies another account read are dropped.
    pub fn note_account(&mut self, tag: Option<&str>) {
        if self.account.as_deref() != tag {
            self.bodies.clear();
            self.account = tag.map(str::to_string);
        }
    }

    /// Signed out: nothing is kept.
    pub fn forget(&mut self) {
        self.bodies.clear();
        self.account = None;
    }

    pub fn body(&mut self, cid: &str, bid: &str, version: u64) -> Option<Arc<VersionBody>> {
        let at = self.bodies.iter().position(|((c, b, v), _)| c == cid && b == bid && *v == version)?;
        let e = self.bodies.remove(at);
        let body = e.1.clone();
        self.bodies.push(e);
        Some(body)
    }

    pub fn put_body(&mut self, cid: &str, bid: &str, body: VersionBody) -> Arc<VersionBody> {
        let version = body.version;
        self.bodies.retain(|((c, b, v), _)| !(c == cid && b == bid && *v == version));
        let body = Arc::new(body);
        self.bodies.push(((cid.to_string(), bid.to_string(), version), body.clone()));
        while self.bodies.len() > BODY_CACHE_MAX {
            self.bodies.remove(0);
        }
        body
    }

    pub fn bodies(&self) -> usize {
        self.bodies.len()
    }
}

/// One side of a compare.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Side {
    Version(u64),
    YourCopy,
}

/// A version that can not be one side of a compare or a restore.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NotReadable {
    Withheld(u64),
    NotKept(u64),
}

/// The version reads a compare or a restore of `sides` needs: none for "Your copy" or a body already held, so a
/// compare is 2 reads and a restore 1 (0 right after a compare). Withheld and pruned versions are refused.
pub fn reads_needed(lines: &[VersionLine], cache: &mut HistoryCache, cid: &str, bid: &str, sides: &[Side]) -> Result<Vec<u64>, NotReadable> {
    let mut out = Vec::new();
    for s in sides {
        let Side::Version(v) = *s else { continue };
        if let Some(l) = lines.iter().find(|l| l.version == v) {
            if l.withheld {
                return Err(NotReadable::Withheld(v));
            }
            if !l.kept {
                return Err(NotReadable::NotKept(v));
            }
        }
        if !out.contains(&v) && cache.body(cid, bid, v).is_none() {
            out.push(v);
        }
    }
    Ok(out)
}

/// A version's body: from the cache, or one `GET .../versions/:n` (kept in the cache).
pub async fn read_body(pool: &Pool<'_>, cache: &mut HistoryCache, cid: &str, bid: &str, version: u64, at_ms: i64) -> Result<Arc<VersionBody>, PoolError> {
    if let Some(b) = cache.body(cid, bid, version) {
        return Ok(b);
    }
    let doc = pool.circle_version(cid, bid, version).await?;
    let body = VersionBody::from_payload(version, &doc.payload, doc.bank.keys_hidden, at_ms);
    // a question this build can not read would be missing from the compare, and from a restore
    if payload_question_count(&doc.payload) > body.questions.len() {
        return Err(PoolError::Local(LocalProblem::NewerQuestions));
    }
    Ok(cache.put_body(cid, bid, body))
}

/// A picture of the circle bank by its hash (checked against the bytes by `circle_blob`), stored on this computer.
/// `Ok(None)`: it could not be stored here.
pub async fn fetch_picture(pool: &Pool<'_>, cid: &str, bid: &str, sha: &str, images: &dyn ImageSink) -> Result<Option<ImageRef>, PoolError> {
    let blob = pool.circle_blob(cid, bid, sha).await?;
    Ok(images.put(&blob.bytes).ok())
}

// ---- compare (plan 4.6) ----------------------------------------------------------------------------------------------

/// One side's question, with its picture's name on the circle (a hash).
#[derive(Clone, Debug)]
pub struct Shown {
    pub q: Question,
    pub sha: Option<String>,
}

/// One field of a compare, worded like `merge::FieldDiff` (`left` is the older or chosen side).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FieldRow {
    pub field: &'static str,
    pub left: String,
    pub right: String,
    pub changed: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Comparison {
    pub fields: Vec<FieldRow>,
    /// That side does not have the question (removed, or not added yet).
    pub left_absent: bool,
    pub right_absent: bool,
}

impl Comparison {
    pub fn changed_fields(&self) -> Vec<&'static str> {
        self.fields.iter().filter(|f| f.changed).map(|f| f.field).collect()
    }

    /// The folded ones ("Points, topics: the same").
    pub fn same_fields(&self) -> Vec<&'static str> {
        self.fields.iter().filter(|f| !f.changed).map(|f| f.field).collect()
    }
}

pub fn side_of_version(body: &VersionBody, qid: &str) -> Option<Shown> {
    body.question(qid).map(|t| Shown { q: t.q.clone(), sha: t.sha.clone() })
}

pub fn side_of_copy(bank: &QuestionBank, local_id: &str) -> Option<Shown> {
    bank.questions.iter().find(|q| q.id == local_id).map(|q| Shown { q: q.clone(), sha: q.image.as_ref().and_then(|i| sha_of_file(&i.file)) })
}

fn canon_text(s: &str) -> String {
    canon::block(s).into_owned()
}

/// The texts of a list of choices, no key. `order_free` (the viewers' form): unlocked choices are shuffled on every
/// read, so they are kept apart from the locked ones and sorted.
fn answers_shape(a: &[Answer], order_free: bool) -> Value {
    if !order_free {
        return a.iter().map(|x| json!([canon_text(&x.text), x.locked])).collect::<Vec<_>>().into();
    }
    let locked: Vec<Value> = a.iter().enumerate().filter(|(_, x)| x.locked).map(|(i, x)| json!([i, canon_text(&x.text)])).collect();
    let mut free: Vec<String> = a.iter().filter(|x| !x.locked).map(|x| canon_text(&x.text)).collect();
    free.sort();
    json!({ "locked": locked, "free": free })
}

/// A Matching question's left items and word bank: in the viewers' form both come in a fresh order on every read
/// (and the word bank holds the right items), so both are sorted.
fn matching_parts(pairs: &[crate::model::Pair], distractors: &[String], order_free: bool) -> (Vec<String>, Vec<String>) {
    let mut left: Vec<String> = pairs.iter().map(|p| canon_text(&p.left)).collect();
    if !order_free {
        return (left, distractors.iter().map(|d| canon_text(d)).collect());
    }
    left.sort();
    let mut bank: Vec<String> = pairs.iter().map(|p| canon_text(&p.right)).chain(distractors.iter().map(|d| canon_text(d))).filter(|s| !s.trim().is_empty()).collect();
    bank.sort();
    bank.dedup();
    (left, bank)
}

fn shape_of(k: &Kind, order_free: bool) -> Value {
    match k {
        Kind::Mc { answers } => json!({ "Mc": answers_shape(answers, order_free) }),
        Kind::Tf { .. } => json!("Tf"),
        Kind::Matching { pairs, distractors } => json!({ "Matching": matching_parts(pairs, distractors, order_free) }),
        Kind::Blank { accept } => json!({ "Blank": accept.len() }),
        Kind::Short { .. } => json!("Short"),
        Kind::Multipart { parts } => json!({ "Multipart": parts.iter().map(|p| json!([canon_text(&p.stem), match &p.kind {
            PartKind::Mc { answers } => answers_shape(answers, order_free),
            PartKind::Tf { .. } => json!("Tf"),
            PartKind::Short { .. } => json!("Short"),
        }])).collect::<Vec<_>>() }),
    }
}

fn key_of(k: &Kind) -> Value {
    match k {
        Kind::Mc { answers } => answers.iter().map(|a| a.correct).collect::<Vec<_>>().into(),
        Kind::Tf { correct } => (*correct).into(),
        Kind::Matching { pairs, .. } => pairs.iter().map(|p| canon_text(&p.right)).collect::<Vec<_>>().into(),
        Kind::Blank { accept } => accept.iter().map(|a| a.iter().map(|x| canon_text(x)).collect::<Vec<_>>()).collect::<Vec<_>>().into(),
        Kind::Short { sample } => canon_text(sample).into(),
        Kind::Multipart { parts } => parts
            .iter()
            .map(|p| match &p.kind {
                PartKind::Mc { answers } => answers.iter().map(|a| a.correct).collect::<Vec<_>>().into(),
                PartKind::Tf { correct } => (*correct).into(),
                PartKind::Short { sample } => Value::from(canon_text(sample)),
            })
            .collect::<Vec<Value>>()
            .into(),
    }
}

fn choices_text(k: &Kind, order_free: bool) -> String {
    match k {
        Kind::Matching { pairs, distractors } if order_free => {
            let (left, bank) = matching_parts(pairs, distractors, true);
            let s = |l: &[String]| l.iter().map(|x| shown(x)).collect::<Vec<_>>().join(" | ");
            format!("{} (word bank: {})", s(&left), s(&bank))
        }
        Kind::Mc { answers } if order_free => {
            let mut free: Vec<&str> = answers.iter().filter(|a| !a.locked).map(|a| a.text.as_str()).collect();
            free.sort_unstable();
            let mut free = free.into_iter();
            answers.iter().map(|a| shown(if a.locked { &a.text } else { free.next().unwrap_or_default() })).collect::<Vec<_>>().join(" | ")
        }
        _ => merge::kind_text(k),
    }
}

fn key_words(q: &Question) -> String {
    if is_keyless(q) {
        return "no key".into();
    }
    match &q.kind {
        Kind::Mc { answers } => {
            let c: Vec<String> = answers.iter().enumerate().filter(|(_, a)| a.correct).map(|(i, _)| (i + 1).to_string()).collect();
            if c.is_empty() { "none marked".into() } else { format!("choice {}", c.join(", ")) }
        }
        Kind::Tf { correct } => (if *correct { "True" } else { "False" }).into(),
        Kind::Matching { pairs, .. } => pairs.iter().map(|p| shown(&p.right)).collect::<Vec<_>>().join(" | "),
        Kind::Blank { accept } => accept.iter().map(|a| a.join(" / ")).collect::<Vec<_>>().join(" | "),
        Kind::Short { sample } => shown(sample),
        Kind::Multipart { .. } => "per part".into(),
    }
}

fn explanation_of(q: &Question) -> Option<String> {
    if is_keyless(q) { None } else { q.explanation.clone().filter(|e| !canon::block(e).trim().is_empty()) }
}

fn level(d: Option<crate::model::Difficulty>) -> String {
    use crate::model::Difficulty;
    match d {
        Some(Difficulty::Low) => "Easy",
        Some(Difficulty::Medium) => "Medium",
        Some(Difficulty::High) => "Hard",
        None => "none",
    }
    .into()
}

/// The two sides field by field. `viewers_form`: both are viewers' forms of a bank that hides its keys (stripped
/// reads): the key is not compared, and a fresh shuffle of choices or a Matching word bank is not a change.
pub fn compare(left: Option<&Shown>, right: Option<&Shown>, viewers_form: bool) -> Comparison {
    const ABSENT: &str = "(not in this version)";
    type Field = (&'static str, fn(&Shown, bool) -> String, fn(&Shown, &Shown, bool) -> bool);
    let fields: [Field; 8] = [
        ("Question text", |s, _| shown(&s.q.stem), |a, b, _| canon_text(&a.q.stem) != canon_text(&b.q.stem)),
        ("Choices", |s, f| choices_text(&s.q.kind, f), |a, b, f| std::mem::discriminant(&a.q.kind) != std::mem::discriminant(&b.q.kind) || shape_of(&a.q.kind, f) != shape_of(&b.q.kind, f)),
        (
            "Answer key",
            |s, f| if f { "hidden from viewers".into() } else { key_words(&s.q) },
            |a, b, f| !f && (is_keyless(&a.q) != is_keyless(&b.q) || (!is_keyless(&a.q) && key_of(&a.q.kind) != key_of(&b.q.kind))),
        ),
        ("Points", |s, _| s.q.points.map_or("none".into(), |p| p.to_string()), |a, b, _| a.q.points != b.q.points),
        ("Difficulty", |s, _| level(s.q.difficulty), |a, b, _| a.q.difficulty != b.q.difficulty),
        ("Topics", |s, _| s.q.topics.iter().map(|t| shown(t)).collect::<Vec<_>>().join(", "), |a, b, _| canon::topics(&a.q.topics) != canon::topics(&b.q.topics)),
        (
            "Explanation",
            |s, _| explanation_of(&s.q).map_or("none".into(), |e| shown(&e)),
            |a, b, _| explanation_of(&a.q).map(|e| canon_text(&e)) != explanation_of(&b.q).map(|e| canon_text(&e)),
        ),
        ("Picture", |s, _| if s.sha.is_some() { "a picture".into() } else { "no picture".into() }, |a, b, _| a.sha != b.sha),
    ];
    let rows = fields
        .iter()
        .map(|(name, text, differs)| match (left, right) {
            (Some(a), Some(b)) => FieldRow { field: name, left: text(a, viewers_form), right: text(b, viewers_form), changed: differs(a, b, viewers_form) },
            (Some(a), None) => FieldRow { field: name, left: text(a, viewers_form), right: ABSENT.into(), changed: true },
            (None, Some(b)) => FieldRow { field: name, left: ABSENT.into(), right: text(b, viewers_form), changed: true },
            (None, None) => FieldRow { field: name, left: ABSENT.into(), right: ABSENT.into(), changed: false },
        })
        .collect();
    Comparison { fields: rows, left_absent: left.is_none(), right_absent: right.is_none() }
}

// ---- restore one question (plan 4.5) ---------------------------------------------------------------------------------

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RestoreRefused {
    /// A follow link (a viewer's): Get latest only.
    FollowLink,
    /// Made from the viewers' form: never sendable, so never restored into.
    ViewersForm,
    /// The version was read in the viewers' form: it has no keys to restore.
    KeysHidden,
    /// The linked bank is not on this computer.
    NoBank,
    /// The bank was put back from a safety copy: Get latest or "Use my bank as it is" first.
    Stale,
    /// That version does not have the question.
    NotInVersion,
    /// The copy already is that version's question.
    Same,
    /// The version's picture was not fetched: nothing is half-written.
    PictureMissing,
    /// The restore point could not be saved: nothing changed.
    RestorePoint(String),
    /// The banks could not be saved: put back as they were.
    NotSaved(String),
}

pub struct RestoreAsk<'a> {
    pub link: &'a CircleLink,
    /// The question in the bank; `None` (or one no longer there): it comes back.
    pub local_id: Option<&'a str>,
    pub qid: &'a str,
    pub body: &'a VersionBody,
    /// The version's picture of the question, fetched by [`fetch_picture`] (not needed when the copy has it already).
    pub picture: Option<ImageRef>,
    /// The circle's newest version no longer has the question ([`removed_at_head`]): it comes back as a new question
    /// when sent (the pool keeps a question id only if the newest version holds it).
    pub removed_in_circle: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Restored {
    pub local_id: String,
    pub version: u64,
    /// Counts as added (new to the circle) rather than changed.
    pub added: bool,
    /// Where the restore point is, in words.
    pub restore_point: String,
    /// The link to keep instead (`CircleEditModel::update_link`): the question no longer paired with the circle's
    /// removed one. `None`: the link is unchanged.
    pub link: Option<CircleLink>,
}

/// "Replace your copy of question N with version V's": a local edit. The restore point is written first (nothing
/// changes when it can not be); then only that question changes, keeping its id and place, and the banks are written
/// (`save`; put back as they were if that fails). The bank's time moves forward as any edit's does, so it is one unsent
/// change and never taken for a bank put back from a safety copy.
pub fn restore_question(banks: &mut [QuestionBank], ask: &RestoreAsk, sink: &dyn RestoreSink, save: &dyn Fn(&[QuestionBank]) -> Result<(), String>, now_ms: i64) -> Result<Restored, RestoreRefused> {
    let link = ask.link;
    if !link.can_send() {
        return Err(if link.viewers_form { RestoreRefused::ViewersForm } else { RestoreRefused::FollowLink });
    }
    if ask.body.keys_hidden {
        return Err(RestoreRefused::KeysHidden);
    }
    let pos = banks.iter().position(|b| b.id == link.local_bank_id).ok_or(RestoreRefused::NoBank)?;
    if link.stale_for(&banks[pos]) {
        return Err(RestoreRefused::Stale);
    }
    let t = ask.body.question(ask.qid).ok_or(RestoreRefused::NotInVersion)?;
    let bank = &banks[pos];
    let in_bank = |id: &str| bank.questions.iter().position(|q| q.id == id);
    let paired = link.qids.iter().find(|l| l.qid == ask.qid).map(|l| l.local.clone());
    let at = ask.local_id.and_then(in_bank).or_else(|| paired.as_deref().and_then(in_bank));
    let existing = at.map(|i| &bank.questions[i]);
    let local_id = existing.map(|q| q.id.clone()).or(paired.clone()).unwrap_or_else(crate::model::new_id);
    let secs = now_ms.div_euclid(1000);
    let image = match &t.sha {
        None => None,
        Some(sha) => match existing.and_then(|q| q.image.clone()).filter(|i| sha_of_file(&i.file).as_deref() == Some(sha.as_str())) {
            Some(i) => Some(i),
            None => Some(ask.picture.clone().ok_or(RestoreRefused::PictureMissing)?),
        },
    };
    let mut r = t.q.clone();
    r.id = local_id.clone();
    r.created = existing.map_or(secs, |q| q.created);
    r.modified = secs;
    r.image = image;
    if let Some(q) = existing
        && merge::same_question(q, &r)
        && q.image.as_ref().map(|i| &i.file) == r.image.as_ref().map(|i| &i.file)
    {
        return Err(RestoreRefused::Same);
    }
    let mut next = bank.clone();
    match at {
        Some(i) => next.questions[i] = r,
        None => next.questions.push(r),
    }
    // seconds, like every editor writes, and never back in time (a bank older than its link reads as put back)
    next.modified = secs.max(bank.modified.saturating_add(1));
    let restore_point = sink.save_named(banks, "restore").map_err(RestoreRefused::RestorePoint)?;
    let before = std::mem::replace(&mut banks[pos], next);
    if let Err(m) = save(banks) {
        banks[pos] = before;
        return Err(RestoreRefused::NotSaved(m));
    }
    let unpair = ask.removed_in_circle && link.qid_of(&local_id) == Some(ask.qid);
    let new_link = unpair.then(|| {
        let mut l = link.clone();
        l.qids.retain(|q| q.local != local_id);
        l.base.retain(|b| b.local_id != local_id);
        l
    });
    let added = unpair || link.qid_of(&local_id).is_none();
    Ok(Restored { local_id, version: ask.body.version, added, restore_point, link: new_link })
}
