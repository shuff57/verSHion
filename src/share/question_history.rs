//! Phase D2 (plan 4.5, 4.6, 5.3-5.5): one question's history in its circle, comparing two of its versions, and
//! restoring one question. No egui. The timeline is built from the bank's history lines (`circle_versions`) and the
//! change lists (`circle_changes`); compare and restore read version bodies on demand, held in [`HistoryCache`] (4
//! bodies, dropped for another account: the critic 60/61 rule). History lines stay raw (drawn through `api::plain`);
//! compare fields are worded like `merge::FieldDiff` (already plain).

use std::sync::Arc;

use serde_json::{Value, json};

#[cfg(test)]
use super::browse::ImageSink;
use super::browse::RestoreSink;
use super::canon;
#[cfg(test)]
use super::circle_links::payload_question_count;
use super::circle_links::{CircleLink, circle_theirs_as, sha_of_file};
use super::circle_versions_api::{ChangeEntry, ChangesPage, QuestionChange, VersionLine};
use super::merge::{self, Theirs};
#[cfg(test)]
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
    /// (round 3, critic P9) Nothing counted changed since version N, but the copy has no record of all of it (a base an
    /// earlier build wrote kept no difficulty): not said to be "the same", as the strip does not say "up to date".
    NotCounted(u64),
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
    /// (round 3, critic P3) The circle's question the page opened on, kept when a restore unpaired the local question from
    /// it (the circle had removed it). `None`: the link's pair.
    pub qid: Option<&'a str>,
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
            // (round 3, critic P9) as the strip: a base with no record of the difficulty is never "the same"
            Some(b) if b.snap.difficulty.is_none() => YourCopy::NotCounted(link.base_version),
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
        YourCopy::NotCounted(n) => format!("Your copy: no change counted since version {n}, but this computer has no record of every detail of it."),
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

/// The lines, newest first, with (round 5, critic S3) an index by version.
struct Lines(Vec<VersionLine>, std::collections::HashMap<u64, usize>);

impl Lines {
    fn of(lines: &[VersionLine]) -> Lines {
        let mut l = lines.to_vec();
        l.sort_by_key(|a| std::cmp::Reverse(a.version));
        l.dedup_by_key(|x| x.version);
        let at = l.iter().enumerate().map(|(i, x)| (x.version, i)).collect();
        Lines(l, at)
    }
    fn get(&self, v: u64) -> Option<&VersionLine> {
        self.1.get(&v).map(|i| &self.0[*i])
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
    // (round 3) the held-back lines of a run stay under its one gap line
    let at = rows.iter().rposition(|r| !matches!(r, Row::HeldBack { .. }));
    // (round 4, critic P-10) one line also for spans that overlap, not only those that meet
    if let Some(Row::Gap(last)) = at.and_then(|i| rows.get_mut(i))
        && last.older <= g.newer
    {
        last.older = last.older.min(g.older);
        last.held.extend(g.held);
        last.held.sort_unstable();
        last.held.dedup();
        last.not_kept.extend(g.not_kept);
        last.not_kept.sort_unstable();
        last.not_kept.dedup();
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
    let qid = i.qid.or_else(|| i.link.qid_of(i.local_id)).map(str::to_string);
    let your_copy = your_copy(i.link, i.bank, i.local_id);
    // (round 3, critic I3) a viewer of a bank that hides its keys, as the link last saw it, gets bank lines whatever an answer says
    let hiding_viewer = i.link.seen.hide_key && i.link.role() == Some(super::circles_api::Role::Viewer);
    let per_question = i.changes.per_question && !hiding_viewer && !i.link.viewers_form;
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
    if i.link.qids.iter().any(|l| l.qid == qid) {
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
            // (round 3, critic P8) the plan's "Version 7 · held back for review · no details" line, also inside a gap
            if i.show_all || !done {
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

/// (round 4, critic P-10) The empty state's words, when the list has no version row: never contradicting a gap line.
pub fn empty_text(t: &Timeline) -> Option<&'static str> {
    if !(t.rows.is_empty() || (t.per_question && !t.rows.iter().any(|r| matches!(r, Row::Version { .. })))) {
        return None;
    }
    Some(if t.rows.iter().any(|r| matches!(r, Row::Gap(_))) { "No kept version is known to have changed this question." } else { "No version that is still kept changed this question." })
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

type BodyKey = (String, String, String, u64);
/// A bank's lists and lines: its circle, the bank, the scope they were read under.
type PageKey = (String, String, String);

/// The most banks whose change lists and history lines are kept for the session.
pub const PAGE_CACHE_MAX: usize = 8;

/// (round 3, critic B1) What a read was made under: the role the link last saw, whether the bank hides its keys, and
/// whether the copy is the viewers' form. A read made under another is never used.
pub fn scope_of(link: &CircleLink) -> String {
    format!("{}|{}|{}", link.role_seen.as_deref().unwrap_or("?"), link.seen.hide_key, link.viewers_form)
}

/// Version bodies, change lists and history lines read for one question's history, in memory only, for one account:
/// another account (or a sign-out, [`HistoryCache::forget`]) drops them. The least recently used goes first.

#[derive(Default)]
pub struct HistoryCache {
    account: Option<String>,
    bodies: Vec<(BodyKey, Arc<VersionBody>)>,
    /// (round 2) Each bank's change lists as last read, sent back as `If-None-Match` (a 304 costs nothing).
    pages: Vec<(PageKey, ChangesPage)>,
    /// (round 2) Each bank's history lines, for the newest version they were read at.
    lines: Vec<(PageKey, u64, Vec<VersionLine>)>,
}

impl std::fmt::Debug for HistoryCache {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("HistoryCache").field("bodies", &self.bodies.len()).field("pages", &self.pages.len()).field("lines", &self.lines.len()).finish()
    }
}

impl HistoryCache {
    /// The account signed in now (`bank_links::account_tag`): bodies another account read are dropped.
    pub fn note_account(&mut self, tag: Option<&str>) {
        if self.account.as_deref() != tag {
            self.bodies.clear();
            self.pages.clear();
            self.lines.clear();
            self.account = tag.map(str::to_string);
        }
    }

    /// Signed out: nothing is kept.
    pub fn forget(&mut self) {
        self.bodies.clear();
        self.pages.clear();
        self.lines.clear();
        self.account = None;
    }

    /// The bank's change lists as last read (to send back with `If-None-Match`).
    pub fn page(&self, cid: &str, bid: &str, scope: &str) -> Option<&ChangesPage> {
        self.pages.iter().find(|((c, b, s), _)| c == cid && b == bid && s == scope).map(|(_, p)| p)
    }

    pub fn put_page(&mut self, cid: &str, bid: &str, scope: &str, page: ChangesPage) {
        self.pages.retain(|((c, b, _), _)| !(c == cid && b == bid));
        self.pages.push(((cid.to_string(), bid.to_string(), scope.to_string()), page));
        while self.pages.len() > PAGE_CACHE_MAX {
            self.pages.remove(0);
        }
    }

    /// The bank's history lines, if they were read at the newest version `head`.
    pub fn lines(&self, cid: &str, bid: &str, scope: &str, head: u64) -> Option<&[VersionLine]> {
        self.lines.iter().find(|((c, b, s), h, _)| c == cid && b == bid && s == scope && *h == head && head > 0).map(|(_, _, l)| l.as_slice())
    }

    pub fn put_lines(&mut self, cid: &str, bid: &str, scope: &str, head: u64, lines: Vec<VersionLine>) {
        self.lines.retain(|((c, b, _), _, _)| !(c == cid && b == bid));
        self.lines.push(((cid.to_string(), bid.to_string(), scope.to_string()), head, lines));
        while self.lines.len() > PAGE_CACHE_MAX {
            self.lines.remove(0);
        }
    }

    /// Something of the account is held (tests, and the critic 60/61 rule's check).
    pub fn is_empty(&self) -> bool {
        self.bodies.is_empty() && self.pages.is_empty() && self.lines.is_empty()
    }

    pub fn body(&mut self, cid: &str, bid: &str, scope: &str, version: u64) -> Option<Arc<VersionBody>> {
        let at = self.bodies.iter().position(|((c, b, s, v), _)| c == cid && b == bid && s == scope && *v == version)?;
        let e = self.bodies.remove(at);
        let body = e.1.clone();
        self.bodies.push(e);
        Some(body)
    }

    pub fn put_body(&mut self, cid: &str, bid: &str, scope: &str, body: VersionBody) -> Arc<VersionBody> {
        let version = body.version;
        self.bodies.retain(|((c, b, _, v), _)| !(c == cid && b == bid && *v == version));
        let body = Arc::new(body);
        self.bodies.push(((cid.to_string(), bid.to_string(), scope.to_string(), version), body.clone()));
        while self.bodies.len() > BODY_CACHE_MAX {
            self.bodies.remove(0);
        }
        body
    }

    pub fn bodies(&self) -> usize {
        self.bodies.len()
    }

    /// (round 3) Everything held for one bank: its bodies, change lists and lines.
    pub fn drop_bank(&mut self, cid: &str, bid: &str) {
        self.bodies.retain(|((c, b, _, _), _)| !(c == cid && b == bid));
        self.pages.retain(|((c, b, _), _)| !(c == cid && b == bid));
        self.lines.retain(|((c, b, _), _, _)| !(c == cid && b == bid));
    }

    /// (round 4) Something is held for the bank (a body, lists or lines).
    pub fn has_bank(&self, cid: &str, bid: &str) -> bool {
        self.bodies.iter().any(|((c, b, _, _), _)| c == cid && b == bid) || self.pages.iter().any(|((c, b, _), _)| c == cid && b == bid) || self.lines.iter().any(|((c, b, _), _, _)| c == cid && b == bid)
    }

    /// (round 3) One version's body, whatever it was read under (the pool no longer serves it).
    pub fn drop_body(&mut self, cid: &str, bid: &str, version: u64) {
        self.bodies.retain(|((c, b, _, v), _)| !(c == cid && b == bid && *v == version));
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
pub fn reads_needed(lines: &[VersionLine], cache: &mut HistoryCache, cid: &str, bid: &str, scope: &str, sides: &[Side]) -> Result<Vec<u64>, NotReadable> {
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
        if !out.contains(&v) && cache.body(cid, bid, scope, v).is_none() {
            out.push(v);
        }
    }
    Ok(out)
}

/// A version's body: from the cache, or one `GET .../versions/:n` (kept in the cache). (round 5, critic S-m4) Tests
/// only: the page reads through its own jobs, which check the answer.
#[cfg(test)]
pub async fn read_body(pool: &Pool<'_>, cache: &mut HistoryCache, cid: &str, bid: &str, scope: &str, version: u64, at_ms: i64) -> Result<Arc<VersionBody>, PoolError> {
    if let Some(b) = cache.body(cid, bid, scope, version) {
        return Ok(b);
    }
    let doc = pool.circle_version(cid, bid, version).await?;
    let body = VersionBody::from_payload(version, &doc.payload, doc.bank.keys_hidden, at_ms);
    // a question this build can not read would be missing from the compare, and from a restore
    if payload_question_count(&doc.payload) > body.questions.len() {
        return Err(PoolError::Local(LocalProblem::NewerQuestions));
    }
    Ok(cache.put_body(cid, bid, scope, body))
}

/// A picture of the circle bank by its hash (checked against the bytes by `circle_blob`), stored on this computer.
/// `Ok(None)`: it could not be stored here. Tests only, as `read_body`.
#[cfg(test)]
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
    /// (round 4, critic P-8) Not compared: the viewers' form does not show it.
    pub hidden: bool,
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
        self.fields.iter().filter(|f| !f.changed && !f.hidden).map(|f| f.field).collect()
    }

    /// (round 4, critic P-8) The ones not compared (the viewers' form).
    pub fn hidden_fields(&self) -> Vec<&'static str> {
        self.fields.iter().filter(|f| f.hidden).map(|f| f.field).collect()
    }
}

pub fn side_of_version(body: &VersionBody, qid: &str) -> Option<Shown> {
    body.question(qid).map(|t| Shown { q: t.q.clone(), sha: t.sha.clone() })
}

/// (round 6, critic N2) The circle's name for the copy's picture: the picture the link recorded for it (`snap.image`) while
/// the copy still has the file it landed with (`local_image`). The store may keep a circle's PNG (or JPEG) as WebP under a
/// new name, so the file's own name is only used where the link has no record of that file (a picture changed here, or none).
pub fn picture_sha_on_circle(link: &CircleLink, q: &Question) -> Option<String> {
    let file = q.image.as_ref()?.file.as_str();
    match link.base.iter().find(|b| b.local_id == q.id) {
        Some(b) if b.local_image.as_deref() == Some(file) => b.snap.image.clone(),
        _ => sha_of_file(file),
    }
}

pub fn side_of_copy(link: &CircleLink, bank: &QuestionBank, local_id: &str) -> Option<Shown> {
    bank.questions.iter().find(|q| q.id == local_id).map(|q| Shown { q: q.clone(), sha: picture_sha_on_circle(link, q) })
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


fn choices_text(k: &Kind, order_free: bool) -> String {
    match k {
        Kind::Matching { pairs, distractors } if order_free => {
            let (left, bank) = matching_parts(pairs, distractors, true);
            let s = |l: &[String]| l.iter().map(|x| shown(x)).collect::<Vec<_>>().join(" | ");
            format!("{} (word bank: {})", s(&left), s(&bank))
        }
        Kind::Mc { answers } => answers_text(answers, order_free),
        // (round 3, critic P4) each part's own choices, not only its stem
        Kind::Multipart { parts } => parts
            .iter()
            .enumerate()
            .map(|(i, p)| {
                let what = match &p.kind {
                    PartKind::Mc { answers } => answers_text(answers, order_free),
                    PartKind::Tf { .. } => "True / False".into(),
                    PartKind::Short { .. } => "Written answer".into(),
                };
                format!("Part {}: {} - {what}", i + 1, shown(&p.stem))
            })
            .collect::<Vec<_>>()
            .join(" / "),
        _ => merge::kind_text(k),
    }
}

/// A list of choices in words, each one locked in its place marked so (round 3, critic P4). In the viewers' form the
/// unlocked ones are sorted, as they are shuffled on each read.
fn answers_text(answers: &[Answer], order_free: bool) -> String {
    let mut free: Vec<&str> = answers.iter().filter(|a| !a.locked).map(|a| a.text.as_str()).collect();
    if order_free {
        free.sort_unstable();
    }
    let mut free = free.into_iter();
    answers.iter().map(|a| if a.locked { format!("{} (fixed place)", shown(&a.text)) } else { shown(free.next().unwrap_or_default()) }).collect::<Vec<_>>().join(" | ")
}

fn mc_key(answers: &[Answer]) -> String {
    let c: Vec<String> = answers.iter().enumerate().filter(|(_, a)| a.correct).map(|(i, _)| (i + 1).to_string()).collect();
    if c.is_empty() { "none marked".into() } else { format!("choice {}", c.join(", ")) }
}

fn key_words(q: &Question) -> String {
    if is_keyless(q) {
        return "no key".into();
    }
    match &q.kind {
        Kind::Mc { answers } => mc_key(answers),
        Kind::Tf { correct } => (if *correct { "True" } else { "False" }).into(),
        Kind::Matching { pairs, .. } => pairs.iter().map(|p| shown(&p.right)).collect::<Vec<_>>().join(" | "),
        // (round 3, critic M3) the accepted answers are server text: drawn plain
        Kind::Blank { accept } => accept.iter().map(|a| a.iter().map(|x| shown(x)).collect::<Vec<_>>().join(" / ")).collect::<Vec<_>>().join(" | "),
        Kind::Short { sample } => shown(sample),
        // (round 3, critic P4) each part's key
        Kind::Multipart { parts } => parts
            .iter()
            .enumerate()
            .map(|(i, p)| {
                let k = match &p.kind {
                    PartKind::Mc { answers } => mc_key(answers),
                    PartKind::Tf { correct } => (if *correct { "True" } else { "False" }).into(),
                    PartKind::Short { sample } => shown(sample),
                };
                format!("Part {}: {k}", i + 1)
            })
            .collect::<Vec<_>>()
            .join("; "),
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
/// reads): the key and the explanation are not compared (round 4, critic P-8: listed as such, never as "the same"), and a
/// fresh shuffle of choices or a Matching word bank is not a change. (round 4, critic P-7) The full form's choices, key and
/// figure are compared by the merge's own rules (`merge::shape`, `merge::key_value`, `merge::Sig`).
pub fn compare(left: Option<&Shown>, right: Option<&Shown>, viewers_form: bool) -> Comparison {
    const ABSENT: &str = "(not in this version)";
    const HIDDEN: &str = "hidden from viewers, not compared";
    type Field = (&'static str, fn(&Shown, bool) -> String, fn(&Shown, &Shown, bool) -> bool, bool);
    let fields: [Field; 9] = [
        ("Question text", |s, _| shown(&s.q.stem), |a, b, _| canon_text(&a.q.stem) != canon_text(&b.q.stem), false),
        (
            "Choices",
            |s, f| choices_text(&s.q.kind, f),
            |a, b, f| std::mem::discriminant(&a.q.kind) != std::mem::discriminant(&b.q.kind) || if f { shape_of(&a.q.kind, f) != shape_of(&b.q.kind, f) } else { merge::shape(&a.q.kind) != merge::shape(&b.q.kind) },
            false,
        ),
        (
            "Answer key",
            |s, _| key_words(&s.q),
            |a, b, _| is_keyless(&a.q) != is_keyless(&b.q) || (!is_keyless(&a.q) && merge::key_value(&a.q.kind) != merge::key_value(&b.q.kind)),
            true,
        ),
        ("Points", |s, _| s.q.points.map_or("none".into(), |p| p.to_string()), |a, b, _| a.q.points != b.q.points, false),
        ("Difficulty", |s, _| level(s.q.difficulty), |a, b, _| a.q.difficulty != b.q.difficulty, false),
        ("Topics", |s, _| s.q.topics.iter().map(|t| shown(t)).collect::<Vec<_>>().join(", "), |a, b, _| canon::topics(&a.q.topics) != canon::topics(&b.q.topics), false),
        (
            // (round 3, critic B1) never a full side's explanation beside the viewers' form
            "Explanation",
            |s, _| explanation_of(&s.q).map_or("none".into(), |e| shown(&e)),
            |a, b, _| explanation_of(&a.q).map(|e| canon_text(&e)) != explanation_of(&b.q).map(|e| canon_text(&e)),
            true,
        ),
        ("Picture", |s, _| if s.sha.is_some() { "a picture".into() } else { "no picture".into() }, |a, b, _| a.sha != b.sha, false),
        // (round 3, critic P4) the figure the picture was drawn from, and its values
        ("Figure", |s, _| figure_text(&s.q), |a, b, _| merge::Sig::of(&a.q).figure != merge::Sig::of(&b.q).figure, false),
    ];
    // (round 4, critic P-9) an empty value is "none"
    let none = |t: String| if t.trim().is_empty() { "none".to_string() } else { t };
    let rows = fields
        .iter()
        .map(|(name, text, differs, keyed)| {
            let hide = viewers_form && *keyed;
            let txt = |s: &Shown| if hide { HIDDEN.to_string() } else { none(text(s, viewers_form)) };
            match (left, right) {
                (Some(a), Some(b)) => FieldRow { field: name, left: txt(a), right: txt(b), changed: !hide && differs(a, b, viewers_form), hidden: hide },
                (Some(a), None) => FieldRow { field: name, left: txt(a), right: ABSENT.into(), changed: !hide, hidden: hide },
                (None, Some(b)) => FieldRow { field: name, left: ABSENT.into(), right: txt(b), changed: !hide, hidden: hide },
                (None, None) => FieldRow { field: name, left: ABSENT.into(), right: ABSENT.into(), changed: false, hidden: hide },
            }
        })
        .collect::<Vec<FieldRow>>();
    let mut rows = rows;
    // (round 3, critic P4) two pictures: the same one, or a different one
    if let (Some(a), Some(b)) = (left, right)
        && let (Some(x), Some(y)) = (&a.sha, &b.sha)
        && let Some(f) = rows.iter_mut().find(|f| f.field == "Picture")
    {
        if x == y {
            f.left = "the same picture".into();
            f.right = "the same picture".into();
        } else {
            f.right = "a different picture".into();
        }
    }
    Comparison { fields: rows, left_absent: left.is_none(), right_absent: right.is_none() }
}

/// The figure and the values the merge compares (round 4, critic P-7: the same rule, `merge::Sig`), without JSON quotes.
fn figure_text(q: &Question) -> String {
    use merge::FigureValue;
    let Some((id, ps)) = merge::Sig::of(q).figure else { return "no figure".into() };
    if ps.is_empty() {
        return shown(&id);
    }
    let v = |x: &FigureValue| match x {
        FigureValue::Number(n) => n.to_string(),
        FigureValue::Text(t) => shown(t),
        FigureValue::Flag(b) => (if *b { "yes" } else { "no" }).into(),
    };
    format!("{} ({})", shown(&id), ps.iter().map(|(k, x)| format!("{} = {}", shown(k), v(x))).collect::<Vec<_>>().join(", "))
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
    /// (round 3, critic M5) The link no longer pairs that local question with that circle question.
    Moved,
    /// (round 4, critic P-2) The version differs from the copy only in what the copy's record of the circle can't count
    /// (a figure's settings with the same picture): the strip would still say "in step".
    NotCountable,
    /// (round 4, critic S-I2) Undo: the question was edited (or deleted) after the restore.
    EditedSince,
    /// (round 4, critic S-I1) Undo: the copy moved to another version of the circle since the restore.
    LinkMoved,
}

/// What a restore took out of the link (see [`Restored::unpaired`]).
#[derive(Clone, Debug, PartialEq)]
pub struct Unpaired {
    pub qid_at: usize,
    pub pair: super::publish::QidLink,
    pub base_at: usize,
    pub base: Option<merge::BaseQuestion>,
    pub base_version: u64,
    pub base_version_id: String,
}

pub struct RestoreAsk<'a> {
    pub link: &'a CircleLink,
    /// The question in the bank; `None` (or one no longer there): it comes back.
    pub local_id: Option<&'a str>,
    pub qid: &'a str,
    pub body: &'a VersionBody,
    /// The version's picture of the question, fetched by [`fetch_picture`] (not needed when the copy has it already).
    pub picture: Option<ImageRef>,
    /// (round 4, critic S-m3) Or: stores the version's picture (its bytes held in memory), called only after the restore
    /// point is saved.
    pub store_picture: Option<&'a dyn Fn() -> Result<ImageRef, String>>,
    /// The circle's newest version no longer has the question ([`removed_at_head`]): it comes back as a new question
    /// when sent (the pool keeps a question id only if the newest version holds it).
    pub removed_in_circle: bool,
}

#[derive(Clone, Debug)]
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
    /// (round 4, critic S-I2) The question as the restore wrote it: undo only puts back over exactly this.
    pub wrote: Option<Question>,
    /// (round 4, critic S-I1) The pair (and its base entry) a restore took out of the link, with their places, and the
    /// version the link was at: undo puts back only these, into the link as it is then.
    pub unpaired: Option<Unpaired>,
    /// (round 3, critic P2) What "Undo restore" puts back: the question replaced and its place (`None`: it was not in the
    /// copy, so undo removes it again).
    pub replaced: Option<(usize, Question)>,
}

impl PartialEq for Restored {
    fn eq(&self, o: &Self) -> bool {
        let q = |r: &Option<(usize, Question)>| r.as_ref().map(|(i, q)| (*i, serde_json::to_value(q).ok()));
        let w = |r: &Option<Question>| r.as_ref().map(|q| serde_json::to_value(q).ok());
        self.local_id == o.local_id && self.version == o.version && self.added == o.added && self.restore_point == o.restore_point && self.link == o.link && q(&self.replaced) == q(&o.replaced) && w(&self.wrote) == w(&o.wrote) && self.unpaired == o.unpaired
    }
}

fn same_json(a: &Question, b: &Question) -> bool {
    serde_json::to_value(a).ok() == serde_json::to_value(b).ok()
}

/// (round 3, critic P2) "Undo restore": puts back exactly the question a restore replaced (or removes the one it brought
/// back), through the same safe local edit: a restore point first, then the bank, its time moved forward. Edit links only.
/// (round 4, critic S-I2) Only over exactly what the restore wrote: a question edited or deleted since is left as it is.
/// (round 4, critic S-I1) Returns the link as it is now with only the restore's own change taken back (the pair and base
/// entry it took out), never an older copy of the whole link; refused when the copy moved to another version since.
pub fn undo_restore(banks: &mut [QuestionBank], link: &CircleLink, done: &Restored, sink: &dyn RestoreSink, save: &dyn Fn(&[QuestionBank]) -> Result<(), String>, now_ms: i64) -> Result<Option<CircleLink>, RestoreRefused> {
    if !link.can_send() {
        return Err(if link.viewers_form { RestoreRefused::ViewersForm } else { RestoreRefused::FollowLink });
    }
    if let Some(u) = &done.unpaired
        && (link.base_version != u.base_version || link.base_version_id != u.base_version_id)
    {
        return Err(RestoreRefused::LinkMoved);
    }
    let pos = banks.iter().position(|b| b.id == link.local_bank_id).ok_or(RestoreRefused::NoBank)?;
    if link.stale_for(&banks[pos]) {
        return Err(RestoreRefused::Stale);
    }
    let bank = &banks[pos];
    let mut next = bank.clone();
    let now_at = next.questions.iter().position(|q| q.id == done.local_id);
    let untouched = |i: usize| done.wrote.as_ref().is_some_and(|w| same_json(&next.questions[i], w));
    match (&done.replaced, now_at) {
        (Some((_, q)), Some(i)) if untouched(i) => next.questions[i] = q.clone(),
        (None, Some(i)) if untouched(i) => {
            next.questions.remove(i);
        }
        // edited since, or deleted since (never brought back)
        _ => return Err(RestoreRefused::EditedSince),
    }
    next.modified = now_ms.div_euclid(1000).max(bank.modified.saturating_add(1));
    sink.save_named(banks, "restore").map_err(RestoreRefused::RestorePoint)?;
    let before = std::mem::replace(&mut banks[pos], next);
    if let Err(m) = save(banks) {
        banks[pos] = before;
        return Err(RestoreRefused::NotSaved(m));
    }
    Ok(done.unpaired.as_ref().map(|u| {
        let mut l = link.clone();
        if !l.qids.iter().any(|q| q.local == u.pair.local || q.qid == u.pair.qid) {
            l.qids.insert(u.qid_at.min(l.qids.len()), u.pair.clone());
        }
        if let Some(b) = &u.base
            && !l.base.iter().any(|x| x.local_id == b.local_id)
        {
            l.base.insert(u.base_at.min(l.base.len()), b.clone());
        }
        l
    }))
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
    // (round 3, critic M5) the question asked is the one the link pairs with that circle question (or, unpaired, with none)
    if let Some(local) = ask.local_id
        && (paired.as_deref().is_some_and(|p| p != local) || (paired.is_none() && link.qid_of(local).is_some()))
    {
        return Err(RestoreRefused::Moved);
    }
    let at = ask.local_id.and_then(in_bank).or_else(|| paired.as_deref().and_then(in_bank));
    let existing = at.map(|i| &bank.questions[i]);
    let local_id = existing.map(|q| q.id.clone()).or(paired.clone()).unwrap_or_else(crate::model::new_id);
    let secs = now_ms.div_euclid(1000);
    // its picture: the copy's own when it is the same one; given; or (critic S-m3) stored once the restore point is saved
    let mut deferred = false;
    let image = match &t.sha {
        None => None,
        // (round 6, critic N2) the same picture by the circle's name for it, never the local file's (a re-encoded file)
        Some(sha) => match existing.filter(|q| picture_sha_on_circle(link, q).as_deref() == Some(sha.as_str())).and_then(|q| q.image.clone()) {
            Some(i) => Some(i),
            None => match (&ask.picture, ask.store_picture) {
                (Some(p), _) => Some(p.clone()),
                (None, Some(_)) => {
                    deferred = true;
                    None
                }
                (None, None) => return Err(RestoreRefused::PictureMissing),
            },
        },
    };
    let mut r = t.q.clone();
    r.id = local_id.clone();
    r.created = existing.map_or(secs, |q| q.created);
    r.modified = secs;
    r.image = image;
    if !deferred
        && let Some(q) = existing
        && merge::same_question(q, &r)
        && q.image.as_ref().map(|i| &i.file) == r.image.as_ref().map(|i| &i.file)
    {
        return Err(RestoreRefused::Same);
    }
    // (round 4, critic P-2; round 5, critic B1) a restore the strip would count as nothing, judged as the strip judges:
    // against the link's record of the base, its texts compared as the server stores them. If the copy would then be
    // "in step" but differ from what it is now in what that record can not hold (a figure's settings), the strip would
    // say "up to date" over a change never offered: it is not made here.
    // (round 6, critic N1) The version the copy is based on *is* that record, so restoring it can not bring anything the
    // record lacks: no refusal, whatever the record holds. And a question the copy no longer has is judged the same way,
    // never skipped: with no copy's figure to compare, any figure the version carries is a change the record can not count.
    // (round 7, critic R1) The picture is judged by its identity (the version's circle name against the record's), so the
    // guard runs also while the picture is still to be stored: a fetch is no reason to skip the count.
    let base = link.base.iter().find(|b| b.local_id == local_id);
    match base {
        Some(b) => {
            let picture_same = t.sha == b.snap.image;
            // the record's own file stands for the picture: the strip sees no picture change then
            let mut judged = r.clone();
            if picture_same {
                judged.image = b.local_image.clone().map(|file| ImageRef { file, width: 0, height: 0 });
            }
            let figure_moves = existing.map_or(r.figure.is_some(), |q| merge::Sig::of(q).figure != merge::Sig::of(&r).figure);
            if ask.body.version != link.base_version && picture_same && figure_moves && !merge::ours_changed(b, &judged) {
                // (round 7, critic R2) Decided by state, never by a version number: a restore that puts back a question the
                // person changed and that already carries a figure of its own is her own unsent edit coming undone, and the
                // version's figure is what she is putting back. The refusal is for a figure that would appear or move with
                // nothing countable to undo (a copy already in step) or for a copy that is gone, where the record can not
                // count a figure the version carries and the strip would say "up to date" over a change never offered.
                let undoable = existing.is_some_and(|q| q.figure.is_some() && merge::ours_changed(b, q));
                if !undoable {
                    return Err(RestoreRefused::NotCountable);
                }
            }
        }
        // no record of the base: as before, the two compared directly
        None if existing.is_some() => {
            let q = existing.expect("checked");
            let sha = |x: &Question| x.image.as_ref().map(|i| picture_sha_on_circle(link, x).unwrap_or_else(|| i.file.clone()));
            let snap = |x: &Question| serde_json::to_value(merge::snap_of(x, sha(x))).ok();
            if snap(q) == snap(&r) {
                return Err(RestoreRefused::NotCountable);
            }
        }
        _ => {}
    }
    let replaced = at.map(|i| (i, bank.questions[i].clone()));
    let restore_point = sink.save_named(banks, "restore").map_err(RestoreRefused::RestorePoint)?;
    if deferred && let Some(store) = ask.store_picture {
        r.image = Some(store().map_err(|_| RestoreRefused::PictureMissing)?);
    }
    let bank = &banks[pos];
    let mut next = bank.clone();
    match at {
        Some(i) => next.questions[i] = r.clone(),
        None => next.questions.push(r.clone()),
    }
    // seconds, like every editor writes, and never back in time (a bank older than its link reads as put back)
    next.modified = secs.max(bank.modified.saturating_add(1));
    let before = std::mem::replace(&mut banks[pos], next);
    if let Err(m) = save(banks) {
        banks[pos] = before;
        return Err(RestoreRefused::NotSaved(m));
    }
    let unpair = ask.removed_in_circle && link.qid_of(&local_id) == Some(ask.qid);
    let unpaired = unpair.then(|| Unpaired {
        qid_at: link.qids.iter().position(|q| q.local == local_id).unwrap_or(link.qids.len()),
        pair: link.qids.iter().find(|q| q.local == local_id).cloned().unwrap_or(super::publish::QidLink { local: local_id.clone(), qid: ask.qid.to_string() }),
        base_at: link.base.iter().position(|b| b.local_id == local_id).unwrap_or(link.base.len()),
        base: link.base.iter().find(|b| b.local_id == local_id).cloned(),
        base_version: link.base_version,
        base_version_id: link.base_version_id.clone(),
    });
    let new_link = unpair.then(|| {
        let mut l = link.clone();
        l.qids.retain(|q| q.local != local_id);
        l.base.retain(|b| b.local_id != local_id);
        l
    });
    let added = unpair || link.qid_of(&local_id).is_none();
    Ok(Restored { local_id, version: ask.body.version, added, restore_point, link: new_link, wrote: Some(r), unpaired, replaced })
}
