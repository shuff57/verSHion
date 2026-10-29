//! Phase D2 client, round 1 (plan 4.5, 4.6, 5.3-5.5, 10): the change-list read, one question's timeline, compare,
//! the version-body cache and restoring one question. A taped session, no egui.

use std::collections::{HashMap, VecDeque};
use std::sync::{Arc, Mutex};

use serde_json::{Value, json};

use super::api::ApiError;
use super::browse::{ImageSink, RestoreSink};
use super::circle_links::*;
use super::circle_versions_api::*;
use super::circles_api::BankHead;
use super::circles_tests::{BID, CID, DAY, NOW};
use super::merge::base_of;
use super::pool::*;
use super::question_history::*;
use super::transport::{LocalFuture, Request, Response, block_on};
use crate::model::{ImageRef, QuestionBank};

const LOCAL: &str = "local-bank-1";
const QA: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa1";
const QB: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa2";
const QC: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa3";
const QD: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa4";
const SCOPE: &str = "?|false|false";

// ---- a taped session: replies by "Method /path", every request kept with its headers ----------------------------

/// A request as kept: its "Method /path" and headers.
type Seen = (String, Vec<(String, String)>);

#[derive(Default)]
struct Tape {
    routes: Mutex<HashMap<String, VecDeque<Response>>>,
    seen: Mutex<Vec<Seen>>,
}

impl Tape {
    fn new() -> Arc<Tape> {
        Arc::new(Tape::default())
    }
    fn reply(&self, m: &str, path: &str, r: Response) {
        self.routes.lock().unwrap().entry(format!("{m} {path}")).or_default().push_back(r);
    }
    fn json(&self, m: &str, path: &str, status: u16, body: Value, etag: Option<&str>) {
        let mut r = Response::new(status, &body.to_string());
        r.etag = etag.map(str::to_string);
        self.reply(m, path, r);
    }
    fn raw(&self, path: &str, bytes: Vec<u8>) {
        let mut r = Response::new(200, "");
        r.bytes = bytes;
        self.reply("Get", path, r);
    }
    fn calls(&self, needle: &str) -> usize {
        self.seen.lock().unwrap().iter().filter(|(p, _)| p.contains(needle)).count()
    }
    fn header(&self, i: usize, name: &str) -> Option<String> {
        self.seen.lock().unwrap().get(i).and_then(|(_, h)| h.iter().find(|(k, _)| k.eq_ignore_ascii_case(name)).map(|(_, v)| v.clone()))
    }
}

impl PoolSession for Tape {
    fn base(&self) -> String {
        "https://pool.test".into()
    }
    fn send<'a>(&'a self, build: &'a dyn Fn() -> Request) -> LocalFuture<'a, Result<Response, ApiError>> {
        Box::pin(async move {
            let r = build();
            let path = r.url.trim_start_matches("https://pool.test").to_string();
            let key = format!("{:?} {}", r.method, path);
            self.seen.lock().unwrap().push((key.clone(), r.headers.iter().map(|(k, v)| (k.clone(), v.expose().to_string())).collect()));
            match self.routes.lock().unwrap().get_mut(&key).and_then(VecDeque::pop_front) {
                Some(r) => Ok(r),
                None => panic!("no taped reply for {key}"),
            }
        })
    }
}

fn changes_path() -> String {
    format!("/circles/{CID}/banks/{BID}/changes")
}

fn version_path(n: u64) -> String {
    format!("/circles/{CID}/banks/{BID}/versions/{n}")
}

// ---- fixtures ------------------------------------------------------------------------------------------------------

fn mc(id: &str, stem: &str, points: f64) -> Value {
    json!({"id": id, "stem": stem, "kind": {"Mc": {"answers": [{"text": "A", "correct": true}, {"text": "B", "correct": false}]}}, "points": points, "difficulty": null, "created": 5, "modified": 6})
}

fn payload(qs: &[Value]) -> Vec<u8> {
    json!({"schema_version": 2, "bank": {"id": BID, "name": "Cells", "questions": qs, "created": 1, "modified": 2}}).to_string().into_bytes()
}

/// A kept version as the pool serves it.
fn served(version: u64, qs: &[Value], keys_hidden: bool) -> Vec<u8> {
    let head = json!({"circle": {"id": CID, "name": "Biology team"}, "bank": {"id": BID, "title": "Cells", "version": version, "version_id": format!("{version:032x}"), "question_count": qs.len(), "committed_at": NOW - DAY, "keys_hidden": keys_hidden}, "origins": null});
    let h = head.to_string();
    format!("{},\"payload\":{}}}", &h[..h.len() - 1], String::from_utf8(payload(qs)).unwrap()).into_bytes()
}

fn body(version: u64, qs: &[Value], keys_hidden: bool) -> VersionBody {
    VersionBody::from_payload(version, &payload(qs), keys_hidden, NOW)
}

fn v9() -> Vec<Value> {
    vec![mc(QA, "Question A as of version 9", 1.0), mc(QB, "Question B as of version 8", 1.0), mc(QC, "Question C", 1.0)]
}

fn v6() -> Vec<Value> {
    vec![mc(QA, "Question A as of version 6", 2.0), mc(QB, "Question B as of version 4", 1.0), mc(QC, "Question C, an older wording", 1.0)]
}

/// A copy in step with version 9 (the bank, its link). Local ids are new; `local_of` finds them.
fn linked() -> (QuestionBank, CircleLink) {
    let theirs = circle_theirs(&payload(&v9()), NOW);
    let bank = QuestionBank { id: LOCAL.into(), name: "Cells".into(), questions: theirs.iter().map(|t| t.q.clone()).collect(), created: NOW / 1000, modified: NOW / 1000, details: Default::default() };
    let head = BankHead { id: BID.into(), title: "Cells".into(), license: "CC-BY-4.0".into(), version: 9, version_id: format!("{:032x}", 9), committed_at: NOW - DAY, ..BankHead::default() };
    let base = theirs.iter().map(|t| base_of(&t.q, &t.qid, &t.fp, t.sha.clone())).collect();
    let mut link = CircleLink::from_head(CID, "Biology team", &head, LOCAL, base, NOW);
    link.mark_bank(&bank);
    (bank, link)
}

fn local_of(link: &CircleLink, qid: &str) -> String {
    link.qids.iter().find(|q| q.qid == qid).map(|q| q.local.clone()).expect("linked")
}

fn line(v: Value) -> VersionLine {
    serde_json::from_value(v).unwrap()
}

/// Versions 9 to 1: 7 is held back for review, 1 and 2 are no longer kept.
fn lines() -> Vec<VersionLine> {
    let kept = |v: u64, by: &str, msg: &str| line(json!({"version": v, "version_id": format!("{v:032x}"), "by": by, "at": NOW - (10 - v as i64) * DAY, "message": msg, "kept": true}));
    vec![
        kept(9, "B. Teacher", "Fixed a distractor"),
        kept(8, "A. Teacher", "Reworded B"),
        line(json!({"version": 7, "by": "", "at": NOW - 3 * DAY, "message": null, "kept": false, "withheld": true, "review_copy": "full"})),
        kept(6, "You", "Unit 2 rewrite"),
        kept(5, "A. Teacher", ""),
        kept(4, "A. Teacher", "Added B"),
        kept(3, "A. Teacher", "First version kept"),
        line(json!({"version": 2, "by": "A. Teacher", "at": NOW - 8 * DAY, "message": "early", "kept": false})),
        line(json!({"version": 1, "by": "A. Teacher", "at": NOW - 9 * DAY, "message": "start", "kept": false})),
    ]
}

fn entry(version: u64, compared_to: u64, added: &[&str], changed: &[&str], removed: &[&str]) -> ChangeEntry {
    let s = |l: &[&str]| l.iter().map(|x| x.to_string()).collect();
    ChangeEntry { version, compared_to, gap: false, added: s(added), changed: s(changed), removed: s(removed) }
}

/// 9 vs 8 changed A; 8 vs 6 (7 held back) changed B; 6 vs 5 a gap; 5 vs 4 changed A; 4 vs 3 added B.
fn changes() -> ChangesPage {
    ChangesPage {
        bank_id: BID.into(),
        head_version: 9,
        per_question: true,
        versions: vec![entry(9, 8, &[], &[QA], &[]), entry(8, 6, &[], &[QB], &[]), ChangeEntry { version: 6, compared_to: 5, gap: true, ..ChangeEntry::default() }, entry(5, 4, &[], &[QA], &[]), entry(4, 3, &[QB], &[], &[])],
        ..ChangesPage::default()
    }
}

fn tl(link: &CircleLink, bank: &QuestionBank, page: &ChangesPage, qid: &str, show_all: bool) -> Timeline {
    let lines = lines();
    let local = local_of(link, qid);
    timeline(&TimelineIn { lines: &lines, changes: page, link, bank, local_id: &local, show_all, qid: None })
}

fn tags(t: &Timeline) -> Vec<(u64, Option<Tag>)> {
    t.rows.iter().filter_map(|r| if let Row::Version { version, tag, .. } = r { Some((*version, *tag)) } else { None }).collect()
}

// ---- the change-list read ------------------------------------------------------------------------------------------

#[test]
fn d2_changes_reads_lists_gaps_and_nulls() {
    let s = Tape::new();
    s.json(
        "Get",
        &changes_path(),
        200,
        json!({"bank_id": BID, "head_version": 9, "per_question": true, "versions": [
            {"version": 9, "compared_to": 8, "added": [QD], "changed": [QA], "removed": null},
            {"version": 8, "compared_to": 6, "added": [], "changed": [QB], "removed": []},
            {"version": 6, "compared_to": 5, "gap": true},
            {"version": null, "compared_to": 4, "added": [QA]},
            {"version": 5, "compared_to": 4, "added": null, "changed": null, "removed": null},
            {"version": 4, "compared_to": 4, "added": [QA]}
        ]}),
        Some("\"tag-1\""),
    );
    let p = block_on(Pool::new(s.as_ref()).circle_changes(CID, BID)).unwrap();
    assert_eq!(s.calls("/changes"), 1);
    assert!(p.per_question && p.head_version == 9 && p.bank_id == BID && !p.from_cache);
    assert_eq!(p.etag.as_deref(), Some("\"tag-1\""));
    // a null version and an entry compared with itself are left out; one with no list at all is a gap
    assert_eq!(p.versions.iter().map(|e| (e.version, e.compared_to, e.gap)).collect::<Vec<_>>(), vec![(9, 8, false), (8, 6, false), (6, 5, true), (5, 4, true)]);
    assert_eq!((p.versions[0].added.clone(), p.versions[0].changed.clone(), p.versions[0].removed.clone()), (vec![QD.to_string()], vec![QA.to_string()], vec![]));
    assert_eq!(p.versions[0].touches(QA), Some(QuestionChange::Changed));
    assert_eq!(p.versions[2].touches(QA), None);
    // a bad id never reaches the pool
    assert!(block_on(Pool::new(s.as_ref()).circle_changes("nope", BID)).is_err());
    assert_eq!(s.seen.lock().unwrap().len(), 1);
}

#[test]
fn d2_changes_for_a_viewer_of_a_hiding_bank_carry_no_lists() {
    let s = Tape::new();
    // even if lists came with it, a keys_hidden answer is read as none
    s.json("Get", &changes_path(), 200, json!({"bank_id": BID, "head_version": 3, "per_question": false, "reason": "keys_hidden", "versions": [{"version": 3, "compared_to": 2, "changed": [QA]}]}), None);
    s.json("Get", &changes_path(), 200, json!({"bank_id": BID, "head_version": 3, "per_question": null, "reason": "keys_hidden"}), None);
    for _ in 0..2 {
        let p = block_on(Pool::new(s.as_ref()).circle_changes(CID, BID)).unwrap();
        assert!(!p.per_question && p.versions.is_empty() && p.reason == "keys_hidden", "{p:?}");
    }
}

#[test]
fn d2_changes_send_if_none_match_and_a_304_is_the_page_read_before() {
    let s = Tape::new();
    s.json("Get", &changes_path(), 200, json!({"bank_id": BID, "head_version": 2, "per_question": true, "versions": [{"version": 2, "compared_to": 1, "added": [], "changed": [QA], "removed": []}]}), Some("\"t2\""));
    s.json("Get", &changes_path(), 304, json!({}), Some("\"t2\""));
    let pool = Pool::new(s.as_ref());
    let first = block_on(pool.circle_changes(CID, BID)).unwrap();
    assert_eq!(s.header(0, "If-None-Match"), None);
    let again = block_on(pool.circle_changes_if(CID, BID, Some(&first))).unwrap();
    assert_eq!(s.header(1, "If-None-Match").as_deref(), Some("\"t2\""));
    assert!(again.from_cache);
    assert_eq!(ChangesPage { from_cache: false, ..again }, first);
}

// ---- the timeline --------------------------------------------------------------------------------------------------

#[test]
fn d2_timeline_lists_only_this_questions_versions_and_one_line_per_gap() {
    let (bank, link) = linked();
    let t = tl(&link, &bank, &changes(), QA, false);
    assert!(t.per_question);
    assert_eq!(t.qid.as_deref(), Some(QA));
    assert_eq!(t.your_copy, YourCopy::SameAs(9));
    assert_eq!(
        t.rows,
        vec![
            Row::Version { version: 9, line: Some(lines()[0].clone()), tag: Some(Tag::Changed), compared_to: Some(8), compare: true, restore: true },
            // 8 did not change A; 7 is held back and 6 has no list: one line for the run
            Row::Gap(GapRun { newer: 8, older: 5, held: vec![7], not_kept: vec![], unreadable: true }),
            // (round 3, critic P8) the plan's "Version 7 · held back for review · no details" line stays in the default view
            Row::HeldBack { version: 7, line: Some(lines()[2].clone()) },
            Row::Version { version: 5, line: Some(lines()[4].clone()), tag: Some(Tag::Changed), compared_to: Some(4), compare: true, restore: true },
            Row::Version { version: 3, line: Some(lines()[6].clone()), tag: Some(Tag::AsItWas), compared_to: None, compare: true, restore: true },
            Row::Older { from: 1, to: 2, count: 2 },
        ]
    );
    // B: changed in 8, added in 4, and nothing older is about it
    let t = tl(&link, &bank, &changes(), QB, false);
    assert_eq!(tags(&t), vec![(8, Some(Tag::Changed)), (4, Some(Tag::Added))]);
    assert!(t.rows.contains(&Row::Gap(GapRun { newer: 8, older: 5, held: vec![7], not_kept: vec![], unreadable: true })), "{:?}", t.rows);
    // a gap below the last news of A: whether the oldest version had it is not known, so it is not said
    let mut p = changes();
    p.versions[3] = ChangeEntry { version: 5, compared_to: 4, gap: true, ..ChangeEntry::default() };
    let t = tl(&link, &bank, &p, QA, false);
    assert!(t.rows.contains(&Row::Gap(GapRun { newer: 8, older: 4, held: vec![7], not_kept: vec![], unreadable: true })), "{:?}", t.rows);
    assert_eq!(tags(&t), vec![(9, Some(Tag::Changed))]);
    assert_eq!(gap_text(&GapRun { newer: 8, older: 6, held: vec![7], not_kept: vec![], unreadable: false }), "Changes between version 6 and version 8: version 7 is held back.");
}

#[test]
fn d2_a_withheld_version_is_never_a_row_with_details_nor_compared() {
    let (bank, link) = linked();
    // a page that (wrongly) lists the held-back version 7 changing A: the lines say it is held back, so it is not shown
    let mut p = changes();
    p.versions.insert(1, entry(7, 6, &[], &[QA], &[]));
    p.versions[2] = entry(8, 7, &[], &[QB], &[]);
    for show_all in [false, true] {
        let t = tl(&link, &bank, &p, QA, show_all);
        assert!(!tags(&t).iter().any(|(v, _)| *v == 7), "{:?}", t.rows);
        for r in &t.rows {
            if let Row::Version { compared_to, .. } = r {
                assert_ne!(*compared_to, Some(7), "never a comparison base");
            }
        }
    }
    let t = tl(&link, &bank, &changes(), QA, true);
    assert!(t.rows.iter().any(|r| matches!(r, Row::HeldBack { version: 7, .. })));
    let mut cache = HistoryCache::default();
    assert_eq!(reads_needed(&lines(), &mut cache, CID, BID, SCOPE, &[Side::Version(7), Side::Version(9)]), Err(NotReadable::Withheld(7)));
}

#[test]
fn d2_pruned_versions_are_who_and_when_only_behind_show_every_version() {
    let (bank, link) = linked();
    let t = tl(&link, &bank, &changes(), QA, false);
    assert!(!t.rows.iter().any(|r| matches!(r, Row::LinesOnly { .. })));
    let t = tl(&link, &bank, &changes(), QA, true);
    let lines_only: Vec<u64> = t.rows.iter().filter_map(|r| if let Row::LinesOnly { line } = r { Some(line.version) } else { None }).collect();
    assert_eq!(lines_only, vec![2, 1]);
    assert!(!t.rows.iter().any(|r| matches!(r, Row::Older { .. })));
    // every kept version is listed, the ones that did not change A without a tag, offered for compare only
    assert_eq!(tags(&t), vec![(9, Some(Tag::Changed)), (8, None), (6, None), (5, Some(Tag::Changed)), (4, None), (3, Some(Tag::AsItWas))]);
    assert!(t.rows.iter().any(|r| matches!(r, Row::Version { version: 8, compare: true, restore: false, .. })));
    let mut cache = HistoryCache::default();
    assert_eq!(reads_needed(&lines(), &mut cache, CID, BID, SCOPE, &[Side::Version(2)]), Err(NotReadable::NotKept(2)));
}

#[test]
fn d2_a_viewer_of_a_hiding_bank_gets_bank_lines_only() {
    let (bank, link) = linked();
    // no per-question lists, even if a page carried some
    let mut p = changes();
    p.per_question = false;
    p.reason = "keys_hidden".into();
    for show_all in [false, true] {
        let t = tl(&link, &bank, &p, QA, show_all);
        assert!(!t.per_question);
        assert!(tags(&t).iter().all(|(_, tag)| tag.is_none()), "{:?}", t.rows);
        assert!(!t.rows.iter().any(|r| matches!(r, Row::Gap(_))));
        assert!(t.rows.iter().all(|r| !matches!(r, Row::Version { restore: true, .. })));
        assert_eq!(tags(&t).iter().map(|(v, _)| *v).collect::<Vec<_>>(), vec![9, 8, 6, 5, 4, 3]);
        assert!(t.rows.iter().any(|r| matches!(r, Row::HeldBack { version: 7, .. })));
    }
    // each kept line compares with the kept line below it
    let t = tl(&link, &bank, &p, QA, false);
    assert!(t.rows.contains(&Row::Version { version: 8, line: Some(lines()[1].clone()), tag: None, compared_to: Some(6), compare: true, restore: false }));
}

#[test]
fn d2_your_copy_line_follows_the_link_base() {
    let (mut bank, mut link) = linked();
    let a = local_of(&link, QA);
    assert_eq!(your_copy(&link, &bank, &a), YourCopy::SameAs(9));
    bank.questions.iter_mut().find(|q| q.id == a).unwrap().stem = "Edited here".into();
    bank.modified += 10;
    assert_eq!(your_copy(&link, &bank, &a), YourCopy::Unsent);
    let b = local_of(&link, QB);
    bank.questions.retain(|q| q.id != b);
    assert_eq!(your_copy(&link, &bank, &b), YourCopy::DeletedHere);
    let mut extra = bank.questions[0].clone();
    extra.id = "new-here".into();
    bank.questions.push(extra);
    assert_eq!(your_copy(&link, &bank, "new-here"), YourCopy::NewHere);
    // a bank put back from a safety copy: what it was in step with is not known
    link.bank_modified = bank.modified + 100;
    assert_eq!(your_copy(&link, &bank, &a), YourCopy::Stale);
    assert_eq!(your_copy_text(YourCopy::SameAs(9)), "Your copy: the same as version 9.");
    assert_eq!(your_copy_text(YourCopy::Unsent), "Your copy: changed, not sent yet.");
}

#[test]
fn d2_removed_at_head_reads_the_newest_entry_about_the_question() {
    let mut p = changes();
    assert_eq!(removed_at_head(&p, QA), Some(false));
    p.versions.insert(0, entry(10, 9, &[], &[], &[QC]));
    p.head_version = 10;
    assert_eq!(removed_at_head(&p, QC), Some(true));
    // a gap above the newest news: not known
    p.versions.insert(0, ChangeEntry { version: 11, compared_to: 10, gap: true, ..ChangeEntry::default() });
    assert_eq!(removed_at_head(&p, QC), None);
    p.per_question = false;
    assert_eq!(removed_at_head(&p, QA), None);
}

// ---- compare -------------------------------------------------------------------------------------------------------

#[test]
fn d2_compare_flags_the_changed_fields_and_folds_the_rest() {
    let (a, b) = (body(6, &v6(), false), body(9, &v9(), false));
    let c = compare(side_of_version(&a, QA).as_ref(), side_of_version(&b, QA).as_ref(), false);
    assert_eq!(c.changed_fields(), vec!["Question text", "Points"]);
    assert_eq!(c.same_fields(), vec!["Choices", "Answer key", "Difficulty", "Topics", "Explanation", "Picture", "Figure"]);
    let stem = c.fields.iter().find(|f| f.field == "Question text").unwrap();
    assert_eq!((stem.left.as_str(), stem.right.as_str()), ("Question A as of version 6", "Question A as of version 9"));
    // a changed key is a changed key
    let mut other = mc(QA, "Question A as of version 9", 1.0);
    other["kind"] = json!({"Mc": {"answers": [{"text": "A", "correct": false}, {"text": "B", "correct": true}]}});
    let k = body(10, &[other], false);
    let c = compare(side_of_version(&b, QA).as_ref(), side_of_version(&k, QA).as_ref(), false);
    assert_eq!(c.changed_fields(), vec!["Answer key"]);
    // your copy against a version, and a version without the question
    let (bank, link) = linked();
    let mine = side_of_copy(&link, &bank, &local_of(&link, QA));
    let c = compare(side_of_version(&b, QA).as_ref(), mine.as_ref(), false);
    assert!(c.changed_fields().is_empty() && !c.left_absent && !c.right_absent);
    let c = compare(side_of_version(&b, QD).as_ref(), mine.as_ref(), false);
    assert!(c.left_absent && !c.right_absent);
}

fn matching(id: &str, lefts: &[&str], rights: &[&str], bank: &[&str]) -> Value {
    let pairs: Vec<Value> = lefts.iter().zip(rights).map(|(l, r)| json!({"left": l, "right": r})).collect();
    json!({"id": id, "stem": "Match the organelle to its job", "kind": {"Matching": {"pairs": pairs, "distractors": bank}}, "points": 1.0, "difficulty": null, "created": 5, "modified": 6, "keyless": true})
}

#[test]
fn d2_compare_of_viewers_forms_ignores_the_matching_shuffle() {
    // two stripped reads of the same question: the left items and the word bank come in a fresh order each time
    let one = body(3, &[matching(QA, &["Nucleus", "Ribosome", "Mitochondrion"], &["", "", ""], &["energy", "proteins", "DNA", "sugar"])], true);
    let two = body(4, &[matching(QA, &["Ribosome", "Mitochondrion", "Nucleus"], &["", "", ""], &["DNA", "sugar", "energy", "proteins"])], true);
    let c = compare(side_of_version(&one, QA).as_ref(), side_of_version(&two, QA).as_ref(), true);
    assert!(c.changed_fields().is_empty(), "{:?}", c.fields);
    // the shown lists are sorted too, so the two columns read the same
    let ch = c.fields.iter().find(|f| f.field == "Choices").unwrap();
    assert_eq!(ch.left, ch.right);
    // the key is not compared in the viewers' form
    let key = c.fields.iter().find(|f| f.field == "Answer key").unwrap();
    assert!(!key.changed);
    // a real change to the word bank is a change
    let three = body(5, &[matching(QA, &["Ribosome", "Mitochondrion", "Nucleus"], &["", "", ""], &["DNA", "glucose", "energy", "proteins"])], true);
    let c = compare(side_of_version(&one, QA).as_ref(), side_of_version(&three, QA).as_ref(), true);
    assert_eq!(c.changed_fields(), vec!["Choices"]);
    // and in the full form the order is the author's, so a move is a change
    let full = |lefts: &[&str]| body(6, &[matching(QA, lefts, &["a", "b", "c"], &[])], false);
    let c = compare(side_of_version(&full(&["x", "y", "z"]), QA).as_ref(), side_of_version(&full(&["y", "x", "z"]), QA).as_ref(), false);
    assert!(c.changed_fields().contains(&"Choices"));
}

// ---- the body cache and the read counts ----------------------------------------------------------------------------

#[test]
fn d2_compare_reads_two_versions_restore_one_and_none_after_a_compare() {
    let s = Tape::new();
    s.raw(&version_path(6), served(6, &v6(), false));
    s.raw(&version_path(9), served(9, &v9(), false));
    s.raw(&version_path(3), served(3, &v6(), false));
    let pool = Pool::new(s.as_ref());
    let mut cache = HistoryCache::default();
    cache.note_account(Some("acct-1"));
    let ls = lines();
    let need = reads_needed(&ls, &mut cache, CID, BID, SCOPE, &[Side::Version(6), Side::Version(9)]).unwrap();
    assert_eq!(need, vec![6, 9]);
    for v in need {
        let b = block_on(read_body(&pool, &mut cache, CID, BID, SCOPE, v, NOW)).unwrap();
        assert_eq!(b.version, v);
        assert!(b.question(QA).is_some());
    }
    assert_eq!(s.calls("/versions/"), 2);
    // Restore from the version just compared: no read
    assert_eq!(reads_needed(&ls, &mut cache, CID, BID, SCOPE, &[Side::Version(6)]).unwrap(), Vec::<u64>::new());
    let again = block_on(read_body(&pool, &mut cache, CID, BID, SCOPE, 6, NOW)).unwrap();
    assert_eq!(again.version, 6);
    assert_eq!(s.calls("/versions/"), 2);
    // your copy is never a read; restoring from another version is one
    assert_eq!(reads_needed(&ls, &mut cache, CID, BID, SCOPE, &[Side::YourCopy, Side::Version(3)]).unwrap(), vec![3]);
    block_on(read_body(&pool, &mut cache, CID, BID, SCOPE, 3, NOW)).unwrap();
    assert_eq!(s.calls("/versions/"), 3);
}

#[test]
fn d2_the_cache_keeps_four_bodies_and_drops_them_for_another_account() {
    let mut cache = HistoryCache::default();
    cache.note_account(Some("acct-1"));
    for v in 1..=5 {
        cache.put_body(CID, BID, SCOPE, body(v, &v9(), false));
    }
    assert_eq!(cache.bodies(), BODY_CACHE_MAX);
    assert_eq!(BODY_CACHE_MAX, 4);
    assert!(cache.body(CID, BID, SCOPE, 1).is_none(), "the oldest went first");
    assert!(cache.body(CID, BID, SCOPE, 2).is_some());
    // a body looked at is kept over the others
    cache.put_body(CID, BID, SCOPE, body(6, &v9(), false));
    assert!(cache.body(CID, BID, SCOPE, 2).is_some() && cache.body(CID, BID, SCOPE, 3).is_none());
    // the same account again keeps them; another account (or none) drops them
    cache.note_account(Some("acct-1"));
    assert_eq!(cache.bodies(), 4);
    cache.note_account(Some("acct-2"));
    assert_eq!(cache.bodies(), 0);
    cache.put_body(CID, BID, SCOPE, body(9, &v9(), false));
    cache.note_account(None);
    assert_eq!(cache.bodies(), 0);
    cache.put_body(CID, BID, SCOPE, body(9, &v9(), false));
    cache.forget();
    assert_eq!(cache.bodies(), 0);
    // another bank's version is another body
    cache.put_body(CID, BID, SCOPE, body(9, &v9(), false));
    assert!(cache.body(CID, "0000000000000000000000000000000f", SCOPE, 9).is_none());
}

// ---- restore one question ------------------------------------------------------------------------------------------

struct LogRestore {
    log: Arc<Mutex<Vec<String>>>,
    copy: Mutex<Option<Vec<QuestionBank>>>,
}

impl RestoreSink for LogRestore {
    fn save(&self, banks: &[QuestionBank]) -> Result<String, String> {
        self.log.lock().unwrap().push("restore point".into());
        *self.copy.lock().unwrap() = Some(banks.to_vec());
        Ok("memory".into())
    }
}

struct BadRestore;

impl RestoreSink for BadRestore {
    fn save(&self, _: &[QuestionBank]) -> Result<String, String> {
        Err("the disk is full".into())
    }
}

fn json_of(b: &[QuestionBank]) -> String {
    serde_json::to_string(b).unwrap()
}

fn ask<'a>(link: &'a CircleLink, local: Option<&'a str>, qid: &'a str, body: &'a VersionBody) -> RestoreAsk<'a> {
    RestoreAsk { link, local_id: local, qid, body, picture: None, store_picture: None, removed_in_circle: false }
}

#[test]
fn d2_restore_writes_a_restore_point_first_and_changes_only_that_question() {
    let (bank, link) = linked();
    let mut banks = vec![QuestionBank { id: "other".into(), questions: Vec::new(), ..bank.clone() }, bank.clone()];
    let a = local_of(&link, QA);
    let log = Arc::new(Mutex::new(Vec::new()));
    let sink = LogRestore { log: log.clone(), copy: Mutex::new(None) };
    let l2 = log.clone();
    let save = move |_: &[QuestionBank]| -> Result<(), String> {
        l2.lock().unwrap().push("banks".into());
        Ok(())
    };
    let six = body(6, &v6(), false);
    let before = json_of(&banks);
    let done = restore_question(&mut banks, &ask(&link, Some(&a), QA, &six), &sink, &save, NOW + 1000).unwrap();
    assert_eq!(*log.lock().unwrap(), vec!["restore point".to_string(), "banks".to_string()]);
    assert_eq!(json_of(sink.copy.lock().unwrap().as_ref().unwrap()), before, "the restore point is the banks as they were");
    assert_eq!((done.local_id.as_str(), done.version, done.added, done.link.is_none(), done.restore_point.as_str()), (a.as_str(), 6, false, true, "memory"));
    let after = &banks[1];
    // the same place and id, version 6's content
    assert_eq!(after.questions.iter().map(|q| q.id.clone()).collect::<Vec<_>>(), bank.questions.iter().map(|q| q.id.clone()).collect::<Vec<_>>());
    let qa = &after.questions[0];
    assert_eq!((qa.stem.as_str(), qa.points), ("Question A as of version 6", Some(2.0)));
    assert_eq!(qa.created, bank.questions[0].created);
    for i in 1..3 {
        assert_eq!(serde_json::to_value(&after.questions[i]).unwrap(), serde_json::to_value(&bank.questions[i]).unwrap());
    }
    assert_eq!(json_of(&banks[..1]), json_of(&[QuestionBank { id: "other".into(), questions: Vec::new(), ..bank.clone() }]));
    // one unsent change, and the copy is not taken for one put back
    let c = changes_since(&link, after);
    assert_eq!((c.added, c.changed, c.removed, c.known), (0, 1, 0, true));
    assert!(!link.stale_for(after));
    assert_eq!(your_copy(&link, after, &a), YourCopy::Unsent);
    // the same again is nothing to do: no second restore point
    let r = restore_question(&mut banks, &ask(&link, Some(&a), QA, &six), &sink, &save, NOW + 2000);
    assert_eq!(r, Err(RestoreRefused::Same));
    assert_eq!(log.lock().unwrap().len(), 2);
}

#[test]
fn d2_a_restore_point_that_can_not_be_saved_changes_nothing() {
    let (bank, link) = linked();
    let a = local_of(&link, QA);
    let six = body(6, &v6(), false);
    let mut banks = vec![bank];
    let before = json_of(&banks);
    let saved = Arc::new(Mutex::new(0));
    let s2 = saved.clone();
    let save = move |_: &[QuestionBank]| -> Result<(), String> {
        *s2.lock().unwrap() += 1;
        Ok(())
    };
    let r = restore_question(&mut banks, &ask(&link, Some(&a), QA, &six), &BadRestore, &save, NOW + 1000);
    assert!(matches!(r, Err(RestoreRefused::RestorePoint(ref m)) if m.contains("disk is full")), "{r:?}");
    assert_eq!(json_of(&banks), before);
    assert_eq!(*saved.lock().unwrap(), 0, "the banks were not written");
    // banks that can not be written: put back as they were
    let sink = LogRestore { log: Arc::default(), copy: Mutex::new(None) };
    let r = restore_question(&mut banks, &ask(&link, Some(&a), QA, &six), &sink, &|_: &[QuestionBank]| Err("read-only".to_string()), NOW + 1000);
    assert_eq!(r, Err(RestoreRefused::NotSaved("read-only".into())));
    assert_eq!(json_of(&banks), before);
}

#[test]
fn d2_restoring_a_question_the_circle_removed_comes_back_as_added() {
    let (bank, link) = linked();
    let c = local_of(&link, QC);
    let six = body(6, &v6(), false);
    let sink = LogRestore { log: Arc::default(), copy: Mutex::new(None) };
    let ok = |_: &[QuestionBank]| -> Result<(), String> { Ok(()) };
    // still in this copy (not merged yet), removed in the circle's newest version
    let mut banks = vec![bank.clone()];
    let done = restore_question(&mut banks, &RestoreAsk { removed_in_circle: true, ..ask(&link, Some(&c), QC, &six) }, &sink, &ok, NOW + 1000).unwrap();
    assert!(done.added);
    assert_eq!(done.local_id, c);
    assert_eq!(banks[0].questions[2].id, c, "its place and id are kept");
    assert_eq!(banks[0].questions[2].stem, "Question C, an older wording");
    let l = done.link.expect("the link no longer pairs it with the circle's question");
    assert_eq!(l.qid_of(&c), None);
    assert!(!l.base.iter().any(|b| b.local_id == c));
    let n = changes_since(&l, &banks[0]);
    assert_eq!((n.added, n.changed, n.removed), (1, 0, 0));
    // gone from this copy too (a merge took the removal): it comes back at the end, new
    let mut gone = bank.clone();
    gone.questions.retain(|q| q.id != c);
    let mut l2 = link.clone();
    l2.qids.retain(|q| q.qid != QC);
    l2.base.retain(|b| b.qid != QC);
    l2.mark_bank(&gone);
    let mut banks = vec![gone];
    let done = restore_question(&mut banks, &RestoreAsk { removed_in_circle: true, ..ask(&l2, None, QC, &six) }, &sink, &ok, NOW + 1000).unwrap();
    assert!(done.added && done.link.is_none());
    assert_eq!(banks[0].questions.len(), 3);
    assert_eq!(banks[0].questions[2].id, done.local_id);
    let n = changes_since(&l2, &banks[0]);
    assert_eq!((n.added, n.changed, n.removed), (1, 0, 0));
}

#[test]
fn d2_follow_and_viewers_form_links_refuse_restore_and_still_compare() {
    let (bank, mut link) = linked();
    let a = local_of(&link, QA);
    let six = body(6, &v6(), false);
    let log = Arc::new(Mutex::new(Vec::new()));
    let sink = LogRestore { log: log.clone(), copy: Mutex::new(None) };
    let ok = |_: &[QuestionBank]| -> Result<(), String> { Ok(()) };
    let mut banks = vec![bank.clone()];
    let before = json_of(&banks);
    link.mode = LinkMode::Follow;
    assert!(!restore_offered(&link));
    assert_eq!(restore_question(&mut banks, &ask(&link, Some(&a), QA, &six), &sink, &ok, NOW), Err(RestoreRefused::FollowLink));
    let t = tl(&link, &bank, &changes(), QA, false);
    assert!(t.rows.iter().all(|r| !matches!(r, Row::Version { restore: true, .. })));
    assert!(t.rows.iter().any(|r| matches!(r, Row::Version { compare: true, .. })));
    link.mode = LinkMode::Edit;
    link.viewers_form = true;
    assert!(!restore_offered(&link));
    assert_eq!(restore_question(&mut banks, &ask(&link, Some(&a), QA, &six), &sink, &ok, NOW), Err(RestoreRefused::ViewersForm));
    // an edit link never restores from a viewers'-form read
    link.viewers_form = false;
    assert!(restore_offered(&link));
    let stripped = body(6, &v6(), true);
    assert_eq!(restore_question(&mut banks, &ask(&link, Some(&a), QA, &stripped), &sink, &ok, NOW), Err(RestoreRefused::KeysHidden));
    assert_eq!(json_of(&banks), before);
    assert!(log.lock().unwrap().is_empty(), "no restore point for a refusal");
}

#[test]
fn d2_restore_never_makes_the_copy_look_put_back() {
    // the bank's clock is ahead of this computer's now (edited on a faster clock): the restore still moves it forward
    let (mut bank, mut link) = linked();
    bank.modified = NOW / 1000 + 5_000;
    link.mark_bank(&bank);
    let a = local_of(&link, QA);
    let six = body(6, &v6(), false);
    let sink = LogRestore { log: Arc::default(), copy: Mutex::new(None) };
    let mut banks = vec![bank.clone()];
    restore_question(&mut banks, &ask(&link, Some(&a), QA, &six), &sink, &|_: &[QuestionBank]| Ok(()), NOW).unwrap();
    assert!(banks[0].modified > bank.modified);
    assert!(!link.stale_for(&banks[0]));
    // and a copy that already looks put back is not restored into
    let mut stale = link.clone();
    stale.bank_modified = banks[0].modified + 100;
    stale.bank_print = "something else".into();
    let mut b2 = banks.clone();
    let nine = body(9, &v9(), false);
    assert_eq!(restore_question(&mut b2, &ask(&stale, Some(&a), QA, &nine), &sink, &|_: &[QuestionBank]| Ok(()), NOW), Err(RestoreRefused::Stale));
}

#[derive(Default)]
struct Pics(Mutex<usize>);

impl ImageSink for Pics {
    fn put(&self, bytes: &[u8]) -> Result<ImageRef, String> {
        *self.0.lock().unwrap() += 1;
        Ok(ImageRef { file: format!("media/{}.webp", sha256_hex(bytes)), width: 2, height: 2 })
    }
}

#[test]
fn d2_a_restored_picture_is_fetched_by_its_hash_and_never_half_written() {
    let bytes = b"a picture's bytes".to_vec();
    let sha = sha256_hex(&bytes);
    let mut with_pic = mc(QA, "Question A with a picture", 1.0);
    with_pic["image"] = json!({"file": format!("media/{sha}.webp"), "width": 2, "height": 2});
    let pic_body = body(5, &[with_pic], false);
    let (bank, link) = linked();
    let a = local_of(&link, QA);
    let sink = LogRestore { log: Arc::default(), copy: Mutex::new(None) };
    let ok = |_: &[QuestionBank]| -> Result<(), String> { Ok(()) };
    let mut banks = vec![bank.clone()];
    // without the picture: nothing
    assert_eq!(restore_question(&mut banks, &ask(&link, Some(&a), QA, &pic_body), &sink, &ok, NOW + 1000), Err(RestoreRefused::PictureMissing));
    assert_eq!(json_of(&banks), json_of(std::slice::from_ref(&bank)));
    // fetched by its hash; bytes that do not match the name are refused
    let s = Tape::new();
    let blob = format!("/circles/{CID}/banks/{BID}/blobs/{sha}");
    s.raw(&blob, b"other bytes".to_vec());
    s.raw(&blob, bytes.clone());
    let pics = Pics::default();
    let pool = Pool::new(s.as_ref());
    assert!(block_on(fetch_picture(&pool, CID, BID, &sha, &pics)).is_err());
    assert_eq!(*pics.0.lock().unwrap(), 0);
    let got = block_on(fetch_picture(&pool, CID, BID, &sha, &pics)).unwrap().expect("stored");
    assert!(block_on(fetch_picture(&pool, CID, BID, "not-a-hash", &pics)).is_err());
    let done = restore_question(&mut banks, &RestoreAsk { picture: Some(got.clone()), ..ask(&link, Some(&a), QA, &pic_body) }, &sink, &ok, NOW + 1000).unwrap();
    assert_eq!(banks[0].questions[0].image.as_ref().map(|i| i.file.clone()), Some(got.file.clone()));
    assert_eq!(done.local_id, a);
    // a copy that already has that picture needs no fetch
    let mut b2 = vec![bank.clone()];
    b2[0].questions[0].image = Some(ImageRef { file: format!("media/{sha}.webp"), width: 2, height: 2 });
    b2[0].questions[0].stem = "Edited".into();
    assert!(restore_question(&mut b2, &ask(&link, Some(&a), QA, &pic_body), &sink, &ok, NOW + 1000).is_ok());
    assert_eq!(b2[0].questions[0].image.as_ref().map(|i| i.file.clone()), Some(format!("media/{sha}.webp")));
}


// ---- round 3 (critics on round 2): each test written first ------------------------------------------------------------

fn changes_of(v: Value) -> ChangesPage {
    super::circle_versions_api::changes_page(&v, None)
}

#[test]
fn d2f_a_hostile_change_list_is_capped() {
    // (critic I2) 100 entries, one above the head, one with 20,000 ids: never more than the pool's 40, never above the head
    let mut versions: Vec<Value> = (2..=100u64).rev().map(|v| json!({"version": v, "compared_to": v - 1, "added": [], "changed": [QA], "removed": []})).collect();
    versions.insert(0, json!({"version": 500, "compared_to": 100, "changed": [QB]}));
    let many: Vec<String> = (0..20_000).map(|i| format!("{i:032x}")).collect();
    versions[5]["changed"] = json!(many);
    let p = changes_of(json!({"bank_id": BID, "head_version": 100, "per_question": true, "versions": versions}));
    assert!(p.versions.len() <= CHANGES_MAX && CHANGES_MAX == 40, "{}", p.versions.len());
    assert!(p.versions.iter().all(|e| e.version <= 100));
    assert!(p.versions.iter().all(|e| e.added.len() + e.changed.len() + e.removed.len() <= CHANGE_IDS_MAX));
    assert_eq!(p.versions[0].version, 100, "the newest within the head first");
}

fn blank(id: &str, accept: &str) -> Value {
    json!({"id": id, "stem": "Fill it", "kind": {"Blank": {"accept": [[accept]]}}, "points": 1.0, "difficulty": null, "created": 5, "modified": 6})
}

#[test]
fn d2f_blank_keys_are_drawn_as_plain_text() {
    // (critic M3) a key with a right-to-left override is drawn without it
    let (a, b) = (body(3, &[blank(QA, "x\u{202E}cba")], false), body(4, &[blank(QA, "y")], false));
    let c = compare(side_of_version(&a, QA).as_ref(), side_of_version(&b, QA).as_ref(), false);
    let k = c.fields.iter().find(|f| f.field == "Answer key").unwrap();
    assert!(k.changed && !k.left.contains('\u{202E}'), "{k:?}");
    // and in the merge review (merge::key_text)
    let theirs_body = body(4, &[blank(QA, "z\u{202E}cba")], false);
    let mine = body(3, &[blank(QA, "y")], false);
    let t = theirs_body.question(QA).unwrap().clone();
    let m = mine.question(QA).unwrap();
    let base = vec![base_of(&m.q, QA, &m.fp, None)];
    let bank = QuestionBank { id: LOCAL.into(), name: "b".into(), questions: vec![m.q.clone()], created: 1, modified: 1, details: Default::default() };
    let plan = super::merge::build_plan(BID, &base, &bank, &[t], 3, 4, None, String::new());
    let d = plan.rows[0].diffs.iter().find(|d| d.field == "Answer key").expect("the key changed");
    assert!(!d.theirs.contains('\u{202E}'), "{d:?}");
}

#[test]
fn d2f_restore_refuses_a_question_the_link_pairs_with_another() {
    // (critic M5) the local question asked is paired with B, not A
    let (bank, link) = linked();
    let b = local_of(&link, QB);
    let six = body(6, &v6(), false);
    let mut banks = vec![bank];
    let before = json_of(&banks);
    let sink = LogRestore { log: Arc::default(), copy: Mutex::new(None) };
    assert_eq!(restore_question(&mut banks, &ask(&link, Some(&b), QA, &six), &sink, &|_: &[QuestionBank]| Ok(()), NOW + 1000), Err(RestoreRefused::Moved));
    assert_eq!(json_of(&banks), before);
    assert!(sink.log.lock().unwrap().is_empty());
}

fn multipart(id: &str, first_correct: bool, locked: bool) -> Value {
    json!({"id": id, "stem": "Two parts", "kind": {"Multipart": {"parts": [
        {"stem": "Part one", "kind": {"Mc": {"answers": [{"text": "yes", "correct": first_correct, "locked": locked}, {"text": "no", "correct": !first_correct}]}}},
        {"stem": "Part two", "kind": {"Tf": {"correct": true}}}
    ]}}, "points": 1.0, "difficulty": null, "created": 5, "modified": 6})
}

#[test]
fn d2f_multipart_pictures_and_figures_compare_by_what_they_hold() {
    // (critic P4) a part's key differs: the key changed, the choices did not, and each part's choices are shown
    let (a, b) = (body(3, &[multipart(QA, true, false)], false), body(4, &[multipart(QA, false, false)], false));
    let c = compare(side_of_version(&a, QA).as_ref(), side_of_version(&b, QA).as_ref(), false);
    assert_eq!(c.changed_fields(), vec!["Answer key"]);
    let ch = c.fields.iter().find(|f| f.field == "Choices").unwrap();
    assert!(ch.left.contains("yes") && ch.left.contains("no") && ch.left.contains("Part one"), "{ch:?}");
    let key = c.fields.iter().find(|f| f.field == "Answer key").unwrap();
    assert_ne!(key.left, key.right, "{key:?}");
    // a choice locked in its place: a change, and said
    let l = body(5, &[multipart(QA, true, true)], false);
    let c = compare(side_of_version(&a, QA).as_ref(), side_of_version(&l, QA).as_ref(), false);
    let ch = c.fields.iter().find(|f| f.field == "Choices").unwrap();
    assert!(ch.changed && ch.right.contains("(fixed place)"), "{ch:?}");
    // pictures: a different one, or the same one
    let pic = |sha: &str| {
        let mut q = mc(QA, "With a picture", 1.0);
        q["image"] = json!({"file": format!("media/{}.webp", sha.repeat(64)), "width": 2, "height": 2});
        body(6, &[q], false)
    };
    let c = compare(side_of_version(&pic("a"), QA).as_ref(), side_of_version(&pic("b"), QA).as_ref(), false);
    let p = c.fields.iter().find(|f| f.field == "Picture").unwrap();
    assert!(p.changed && p.right == "a different picture", "{p:?}");
    let c = compare(side_of_version(&pic("a"), QA).as_ref(), side_of_version(&pic("a"), QA).as_ref(), false);
    let p = c.fields.iter().find(|f| f.field == "Picture").unwrap();
    assert!(!p.changed && p.left == "the same picture", "{p:?}");
    // a figure: its own field
    let fig = |v: i64| {
        let mut q = mc(QA, "With a figure", 1.0);
        q["figure"] = json!({"id": "triangle", "params": {"a": v}});
        body(7, &[q], false)
    };
    let c = compare(side_of_version(&fig(3), QA).as_ref(), side_of_version(&fig(4), QA).as_ref(), false);
    assert_eq!(c.changed_fields(), vec!["Figure"]);
}

#[test]
fn d2f_a_viewers_form_compare_never_shows_a_full_sides_explanation_or_key() {
    // (critic B1) one side read in full earlier, one stripped now
    let mut full = mc(QA, "Same stem", 1.0);
    full["explanation"] = json!("SECRET EXPLANATION");
    let (a, b) = (body(3, &[full], false), body(4, &[mc(QA, "Same stem", 1.0)], true));
    let c = compare(side_of_version(&a, QA).as_ref(), side_of_version(&b, QA).as_ref(), true);
    for f in &c.fields {
        assert!(!f.left.contains("SECRET") && !f.right.contains("SECRET") && !f.left.contains("choice 1"), "{f:?}");
    }
}

#[test]
fn d2f_your_copy_is_not_the_same_when_the_base_has_no_record_of_a_difficulty() {
    // (critic P9) as the strip: a base an earlier build wrote kept no difficulty, so nothing is said to be "the same"
    let (bank, mut link) = linked();
    let a = local_of(&link, QA);
    for b in &mut link.base {
        b.snap.difficulty = None;
    }
    assert_eq!(your_copy(&link, &bank, &a), YourCopy::NotCounted(9));
}

#[test]
fn d2f_undo_restore_puts_back_exactly_what_was_replaced() {
    // (critic P2)
    let (bank, link) = linked();
    let a = local_of(&link, QA);
    let six = body(6, &v6(), false);
    let mut banks = vec![bank.clone()];
    banks[0].questions[0].stem = "My unsent edit".into();
    banks[0].modified += 5;
    let before = serde_json::to_value(&banks[0].questions).unwrap();
    let log = Arc::new(Mutex::new(Vec::new()));
    let sink = LogRestore { log: log.clone(), copy: Mutex::new(None) };
    let ok = |_: &[QuestionBank]| -> Result<(), String> { Ok(()) };
    let done = restore_question(&mut banks, &ask(&link, Some(&a), QA, &six), &sink, &ok, NOW + 1000).unwrap();
    assert_eq!(done.replaced.as_ref().map(|(i, q)| (*i, q.stem.clone())), Some((0, "My unsent edit".to_string())));
    let t = banks[0].modified;
    let back = undo_restore(&mut banks, &link, &done, &sink, &ok, NOW + 2000).unwrap();
    assert!(back.is_none());
    assert_eq!(serde_json::to_value(&banks[0].questions).unwrap(), before, "exactly the replaced question");
    assert!(banks[0].modified > t && !link.stale_for(&banks[0]));
    assert_eq!(log.lock().unwrap().len(), 2, "a restore point each time");
    // a question the circle removed: the link comes back as it was
    let c = local_of(&link, QC);
    let mut banks = vec![bank.clone()];
    let done = restore_question(&mut banks, &RestoreAsk { removed_in_circle: true, ..ask(&link, Some(&c), QC, &six) }, &sink, &ok, NOW + 1000).unwrap();
    let unpaired = done.link.clone().unwrap();
    let back = undo_restore(&mut banks, &unpaired, &done, &sink, &ok, NOW + 2000).unwrap();
    assert_eq!(back.as_ref(), Some(&link));
    assert_eq!(serde_json::to_value(&banks[0].questions).unwrap(), serde_json::to_value(&bank.questions).unwrap());
    // a follow link never undoes
    let mut follow = link.clone();
    follow.mode = LinkMode::Follow;
    assert_eq!(undo_restore(&mut banks, &follow, &done, &sink, &ok, NOW + 3000), Err(RestoreRefused::FollowLink));
    // a restore point that can not be saved: nothing changes (a fresh restore: the one above is undone already)
    let done = restore_question(&mut banks, &RestoreAsk { removed_in_circle: true, ..ask(&link, Some(&c), QC, &six) }, &sink, &ok, NOW + 4000).unwrap();
    let unpaired = done.link.clone().unwrap();
    let before = json_of(&banks);
    assert!(matches!(undo_restore(&mut banks, &unpaired, &done, &BadRestore, &ok, NOW + 5000), Err(RestoreRefused::RestorePoint(_))));
    assert_eq!(json_of(&banks), before);
}

#[test]
fn d2f_the_cache_keeps_each_scope_apart_and_drops_a_bank_or_a_version() {
    // (critic B1) a read made under one role and hiding state is never used under another
    let mut cache = HistoryCache::default();
    cache.put_body(CID, BID, "viewer|false|false", body(3, &v6(), false));
    assert!(cache.body(CID, BID, "viewer|true|false", 3).is_none());
    assert!(cache.body(CID, BID, "viewer|false|false", 3).is_some());
    cache.put_page(CID, BID, "viewer|false|false", changes());
    assert!(cache.page(CID, BID, "viewer|true|false").is_none());
    cache.put_lines(CID, BID, "viewer|false|false", 9, lines(), true);
    assert!(cache.lines(CID, BID, "editor|false|false", 9).is_none());
    assert_eq!(cache.lines(CID, BID, "viewer|false|false", 9).map(|(l, more)| (l.len(), more)), Some((lines().len(), true)));
    // a version the pool no longer serves: gone under every scope
    cache.put_body(CID, BID, "editor|false|false", body(3, &v6(), false));
    cache.drop_body(CID, BID, 3);
    assert_eq!(cache.bodies(), 0);
    // a bank: its bodies, lists and lines, not another bank's
    cache.put_body(CID, BID, SCOPE, body(4, &v6(), false));
    cache.put_body(CID, "0000000000000000000000000000000f", SCOPE, body(4, &v6(), false));
    cache.drop_bank(CID, BID);
    assert_eq!(cache.bodies(), 1);
    assert!(cache.page(CID, BID, "viewer|false|false").is_none() && cache.lines(CID, BID, "viewer|false|false", 9).is_none());
}


// ---- round 4 (second critic pair): each test written first --------------------------------------------------------------

#[test]
fn d2g_undo_patches_the_link_as_it_is_now_and_refuses_when_its_version_moved() {
    // (critic S-I1) the restore unpaired C; meanwhile the link moved on in other ways: undo puts back only C's pair
    let (bank, link) = linked();
    let c = local_of(&link, QC);
    let six = body(6, &v6(), false);
    let sink = LogRestore { log: Arc::default(), copy: Mutex::new(None) };
    let ok = |_: &[QuestionBank]| -> Result<(), String> { Ok(()) };
    let mut banks = vec![bank.clone()];
    let done = restore_question(&mut banks, &RestoreAsk { removed_in_circle: true, ..ask(&link, Some(&c), QC, &six) }, &sink, &ok, NOW + 1000).unwrap();
    let mut now = done.link.clone().unwrap();
    now.head_seen = 12;
    now.head_seen_by = Some("C. Teacher".into());
    now.seen.hide_key = true;
    let back = undo_restore(&mut banks, &now, &done, &sink, &ok, NOW + 2000).unwrap().expect("the pair comes back");
    assert_eq!((back.head_seen, back.head_seen_by.as_deref(), back.seen.hide_key), (12, Some("C. Teacher"), true), "nothing else is put back");
    assert_eq!(back.qid_of(&c), Some(QC));
    assert_eq!(back.qids, link.qids, "in its place");
    assert_eq!(serde_json::to_value(&back.base).unwrap(), serde_json::to_value(&link.base).unwrap());
    // the copy moved to another version since the restore (a merge, a send): refused, nothing changed
    let mut banks = vec![bank.clone()];
    let done = restore_question(&mut banks, &RestoreAsk { removed_in_circle: true, ..ask(&link, Some(&c), QC, &six) }, &sink, &ok, NOW + 1000).unwrap();
    let mut moved = done.link.clone().unwrap();
    moved.base_version = 10;
    let before = json_of(&banks);
    assert_eq!(undo_restore(&mut banks, &moved, &done, &sink, &ok, NOW + 2000), Err(RestoreRefused::LinkMoved));
    assert_eq!(json_of(&banks), before);
}

#[test]
fn d2g_undo_never_overwrites_a_later_edit_nor_brings_back_a_deleted_question() {
    // (critic S-I2)
    let (bank, link) = linked();
    let a = local_of(&link, QA);
    let six = body(6, &v6(), false);
    let sink = LogRestore { log: Arc::default(), copy: Mutex::new(None) };
    let ok = |_: &[QuestionBank]| -> Result<(), String> { Ok(()) };
    // edited after the restore
    let mut banks = vec![bank.clone()];
    let done = restore_question(&mut banks, &ask(&link, Some(&a), QA, &six), &sink, &ok, NOW + 1000).unwrap();
    banks[0].questions[0].stem = "Edited after the restore".into();
    assert_eq!(undo_restore(&mut banks, &link, &done, &sink, &ok, NOW + 2000), Err(RestoreRefused::EditedSince));
    assert_eq!(banks[0].questions[0].stem, "Edited after the restore");
    // deleted after the restore: not put back
    let mut banks = vec![bank.clone()];
    let done = restore_question(&mut banks, &ask(&link, Some(&a), QA, &six), &sink, &ok, NOW + 1000).unwrap();
    banks[0].questions.retain(|q| q.id != a);
    assert_eq!(undo_restore(&mut banks, &link, &done, &sink, &ok, NOW + 2000), Err(RestoreRefused::EditedSince));
    assert_eq!(banks[0].questions.len(), 2);
    // a question the restore brought back new, edited since: not deleted
    let mut gone = bank.clone();
    gone.questions.retain(|q| q.id != local_of(&link, QC));
    let mut l2 = link.clone();
    l2.qids.retain(|q| q.qid != QC);
    l2.base.retain(|b| b.qid != QC);
    l2.mark_bank(&gone);
    let mut banks = vec![gone];
    let done = restore_question(&mut banks, &RestoreAsk { removed_in_circle: true, ..ask(&l2, None, QC, &six) }, &sink, &ok, NOW + 1000).unwrap();
    banks[0].questions[2].points = Some(9.0);
    assert_eq!(undo_restore(&mut banks, &l2, &done, &sink, &ok, NOW + 2000), Err(RestoreRefused::EditedSince));
    assert_eq!(banks[0].questions.len(), 3);
}

#[test]
fn d2g_a_restore_the_count_could_not_see_is_refused() {
    // (critic P-2) version 6 differs from the copy only in its figure's settings, with the same picture
    let (mut bank, link) = linked();
    let a = local_of(&link, QA);
    bank.questions[0].figure = Some(crate::model::FigureSpec { id: "triangle".into(), params: [("a".to_string(), json!(3))].into_iter().collect() });
    let mut q = mc(QA, "Question A as of version 9", 1.0);
    q["figure"] = json!({"id": "triangle", "params": {"a": 4}});
    let b = body(6, &[q, mc(QB, "Question B as of version 8", 1.0), mc(QC, "Question C", 1.0)], false);
    let mut banks = vec![bank];
    let before = json_of(&banks);
    let sink = LogRestore { log: Arc::default(), copy: Mutex::new(None) };
    assert_eq!(restore_question(&mut banks, &ask(&link, Some(&a), QA, &b), &sink, &|_: &[QuestionBank]| Ok(()), NOW + 1000), Err(RestoreRefused::NotCountable));
    assert_eq!(json_of(&banks), before);
    assert!(sink.log.lock().unwrap().is_empty());
}

#[test]
fn d2g_compare_reuses_the_merges_figure_rule_and_names_what_it_did_not_compare() {
    // (critic P-7) a figure value the merge drops (null, a list) is not a change
    let fig = |extra: Value| {
        let mut q = mc(QA, "With a figure", 1.0);
        q["figure"] = json!({"id": "triangle", "params": {"a": 3, "note": extra}});
        body(7, &[q], false)
    };
    let c = compare(side_of_version(&fig(Value::Null), QA).as_ref(), side_of_version(&fig(json!([1, 2])), QA).as_ref(), false);
    assert!(!c.changed_fields().contains(&"Figure"), "{:?}", c.fields);
    let f = c.fields.iter().find(|f| f.field == "Figure").unwrap();
    assert!(!f.left.contains('"'), "values drawn without JSON quotes: {f:?}");
    // (critic P-8) in the viewers' form the key and the explanation are not compared, and not called "the same"
    let (a, b) = (body(3, &[mc(QA, "Same", 1.0)], true), body(4, &[mc(QA, "Same", 1.0)], true));
    let c = compare(side_of_version(&a, QA).as_ref(), side_of_version(&b, QA).as_ref(), true);
    assert!(!c.same_fields().contains(&"Answer key") && !c.same_fields().contains(&"Explanation"), "{:?}", c.same_fields());
    assert_eq!(c.hidden_fields(), vec!["Answer key", "Explanation"]);
    // (critic P-9) an empty value is "none", never blank
    let c = compare(side_of_version(&a, QA).as_ref(), side_of_version(&b, QA).as_ref(), false);
    assert!(c.fields.iter().all(|f| !f.left.trim().is_empty() && !f.right.trim().is_empty()), "{:?}", c.fields);
}

#[test]
fn d2g_overlapping_gaps_are_one_line_and_the_empty_state_does_not_contradict_them() {
    // (critic P-10) two gap entries whose spans overlap (an odd answer): one line
    let (bank, link) = linked();
    let mut p = changes();
    p.versions = vec![ChangeEntry { version: 9, compared_to: 5, gap: true, ..ChangeEntry::default() }, ChangeEntry { version: 6, compared_to: 4, gap: true, ..ChangeEntry::default() }];
    let t = tl(&link, &bank, &p, QA, false);
    let gaps: Vec<&Row> = t.rows.iter().filter(|r| matches!(r, Row::Gap(_))).collect();
    assert_eq!(gaps.len(), 1, "{:?}", t.rows);
    assert!(matches!(gaps[0], Row::Gap(g) if g.newer == 9 && g.older == 4), "{:?}", gaps);
    assert_eq!(empty_text(&t), Some("No kept version is known to have changed this question."));
    let mut quiet = changes();
    quiet.versions = vec![entry(9, 8, &[], &[QB], &[])];
    let t = tl(&link, &bank, &quiet, QC, false);
    let _ = empty_text(&t);
}

// ---- round 5 (third critic pair): the critics' probes, ported to assertions -----------------------------------------

#[test]
fn probe_not_countable_bypass_via_unsent_edit() {
    // (critic B1-A) an unsent stem edit, and version 6 = the base's content with other figure settings (same picture): the
    // strip would count nothing after it, so it is refused
    let (mut bank, link) = linked();
    let a = local_of(&link, QA);
    bank.questions[0].stem = "My unsent edit".into();
    bank.modified += 10;
    let mut q = mc(QA, "Question A as of version 9", 1.0);
    q["figure"] = json!({"id": "triangle", "params": {"a": 4}});
    let b = body(6, &[q, mc(QB, "Question B as of version 8", 1.0), mc(QC, "Question C", 1.0)], false);
    let mut banks = vec![bank];
    let before = json_of(&banks);
    let sink = LogRestore { log: Arc::default(), copy: Mutex::new(None) };
    let r = restore_question(&mut banks, &ask(&link, Some(&a), QA, &b), &sink, &|_: &[QuestionBank]| Ok(()), NOW + 100_000);
    assert_eq!(r.map(|d| d.version), Err(RestoreRefused::NotCountable));
    assert_eq!(json_of(&banks), before);
}

#[test]
fn probe_not_countable_bypass_via_crlf() {
    // (critic B1-B) no local edit; the copy's stem has CRLF, version 6 the same text with LF and other figure settings
    let (mut bank, mut link) = linked();
    let a = local_of(&link, QA);
    bank.questions[0].stem = "Line one\r\nline two".into();
    link.base[0].snap.stem = "Line one\r\nline two".into();
    link.mark_bank(&bank);
    let mut q = mc(QA, "Line one\nline two", 1.0);
    q["figure"] = json!({"id": "triangle", "params": {"a": 4}});
    let b = body(6, &[q, mc(QB, "Question B as of version 8", 1.0), mc(QC, "Question C", 1.0)], false);
    let mut banks = vec![bank];
    let sink = LogRestore { log: Arc::default(), copy: Mutex::new(None) };
    let r = restore_question(&mut banks, &ask(&link, Some(&a), QA, &b), &sink, &|_: &[QuestionBank]| Ok(()), NOW + 100_000);
    assert_eq!(r.map(|d| d.version), Err(RestoreRefused::NotCountable));
    assert_eq!(changes_since(&link, &banks[0]).changed, 0);
}

#[test]
fn d2h_restoring_the_base_over_an_unsent_edit_is_allowed_and_counts_nothing() {
    // (the probe that passed: kept as a guard) restoring the version the copy is based on puts it back in step
    let (mut bank, link) = linked();
    let a = local_of(&link, QA);
    bank.questions[0].stem = "My unsent edit".into();
    bank.modified += 10;
    let mut banks = vec![bank];
    let sink = LogRestore { log: Arc::default(), copy: Mutex::new(None) };
    let nine = body(9, &v9(), false);
    assert!(restore_question(&mut banks, &ask(&link, Some(&a), QA, &nine), &sink, &|_: &[QuestionBank]| Ok(()), NOW + 100_000).is_ok());
    let c = changes_since(&link, &banks[0]);
    assert_eq!((c.changed, c.known), (0, true));
    assert_eq!(your_copy(&link, &banks[0], &a), YourCopy::SameAs(9));
}

#[test]
fn d2h_a_history_page_is_read_no_further_than_the_pool_pages_it() {
    // (critic S3) a reply with 20,000 lines: never more than the pool's page of 50
    let s = Tape::new();
    let lines: Vec<Value> = (1..=20_000u64).rev().map(|v| json!({"version": v, "kept": true, "by": "x", "at": NOW, "message": "m"})).collect();
    s.json("Get", &format!("/circles/{CID}/banks/{BID}/versions"), 200, json!({"bank_id": BID, "head_version": 20_000, "versions": lines}), None);
    let p = block_on(Pool::new(s.as_ref()).circle_versions(CID, BID, None)).unwrap();
    assert_eq!(p.versions.len(), HISTORY_PAGE);
    assert_eq!(p.versions[0].version, 20_000);
}

// ---- throwaway critic probes (round 6) ---------------------------------------------------------------------------------

#[test]
fn d2i_restoring_the_base_version_over_a_stem_and_figure_edit_is_allowed() {
    // (round 6, critic N1) an unsent stem edit AND an unsent figure-settings edit; the base version itself (9, no figure)
    // is restored. The result IS version 9 exactly, so it can bring nothing the link's record lacks: never NotCountable.
    let (mut bank, link) = linked();
    let a = local_of(&link, QA);
    bank.questions[0].stem = "My unsent edit".into();
    bank.questions[0].figure = Some(crate::model::FigureSpec { id: "triangle".into(), params: [("a".to_string(), json!(3))].into_iter().collect() });
    bank.modified += 10;
    let mut banks = vec![bank];
    let sink = LogRestore { log: Arc::default(), copy: Mutex::new(None) };
    let nine = body(9, &v9(), false);
    let r = restore_question(&mut banks, &ask(&link, Some(&a), QA, &nine), &sink, &|_: &[QuestionBank]| Ok(()), NOW + 100_000);
    assert!(r.is_ok(), "restoring the base version exactly is refused: {:?}", r.map(|d| d.version));
    assert_eq!(changes_since(&link, &banks[0]).changed, 0);
}

#[test]
fn d2i_a_figure_the_copy_already_has_but_the_base_lacks_is_refused() {
    // (round 6, critic N1 reported this as a known gap; round 8, critic T1 closes it) base v9 has no figure; the copy
    // already has figure a=4 (uncounted by the strip) and a stem edit; version 6 is v9's content with the same figure a=4.
    // The base record now holds its figure (none), so restoring a figure the base never had would leave the strip saying
    // the copy is in step over a change it can never send: refused.
    let (mut bank, link) = linked();
    let a = local_of(&link, QA);
    bank.questions[0].stem = "edit".into();
    bank.questions[0].figure = Some(crate::model::FigureSpec { id: "triangle".into(), params: [("a".to_string(), json!(4))].into_iter().collect() });
    bank.modified += 10;
    let mut q = mc(QA, "Question A as of version 9", 1.0);
    q["figure"] = json!({"id": "triangle", "params": {"a": 4}});
    let b = body(6, &[q, mc(QB, "Question B as of version 8", 1.0), mc(QC, "Question C", 1.0)], false);
    let mut banks = vec![bank];
    let before = json_of(&banks);
    let sink = LogRestore { log: Arc::default(), copy: Mutex::new(None) };
    let r = restore_question(&mut banks, &ask(&link, Some(&a), QA, &b), &sink, &|_: &[QuestionBank]| Ok(()), NOW + 100_000);
    assert_eq!(r.map(|d| d.version), Err(RestoreRefused::NotCountable));
    assert_eq!(json_of(&banks), before);
}

#[test]
fn d2i_a_topics_reorder_is_a_counted_change_never_refused() {
    // (round 6, critic N1) the pool keeps its topics in order (its hash reads them in order), so version 6's reordered
    // topics are a change the strip counts: the restore is allowed and the copy counts as changed, never "in step".
    let (mut bank, link) = linked();
    let a = local_of(&link, QA);
    bank.questions[0].topics = vec!["x".into(), "y".into()];
    let mut link = link;
    link.base[0].snap.topics = vec!["x".into(), "y".into()];
    link.mark_bank(&bank);
    let mut q = mc(QA, "Question A as of version 9", 1.0);
    q["topics"] = json!(["y", "x"]);
    let b = body(6, &[q, mc(QB, "Question B as of version 8", 1.0), mc(QC, "Question C", 1.0)], false);
    let mut banks = vec![bank];
    let sink = LogRestore { log: Arc::default(), copy: Mutex::new(None) };
    let r = restore_question(&mut banks, &ask(&link, Some(&a), QA, &b), &sink, &|_: &[QuestionBank]| Ok(()), NOW + 100_000);
    assert_eq!(r.map(|d| d.version), Ok(6));
    assert_eq!(banks[0].questions[0].topics, vec!["y".to_string(), "x".to_string()]);
    assert_eq!(changes_since(&link, &banks[0]).changed, 1);
}

#[test]
fn d2i_a_figure_only_change_cannot_slip_in_through_a_deleted_question() {
    // (round 6, critic N1) QA deleted here (unsent). Version 6 = version 9's QA with other figure settings, same (no)
    // picture. The strip would count nothing after such a restore (the removal is undone, the figure is invisible), so it
    // is refused exactly as for a copy present: never skipped because the question is gone.
    let (mut bank, link) = linked();
    let a = local_of(&link, QA);
    bank.questions.retain(|q| q.id != a);
    bank.modified += 10;
    let mut q = mc(QA, "Question A as of version 9", 1.0);
    q["figure"] = json!({"id": "triangle", "params": {"a": 4}});
    let b = body(6, &[q, mc(QB, "Question B as of version 8", 1.0), mc(QC, "Question C", 1.0)], false);
    let mut banks = vec![bank];
    let before = json_of(&banks);
    let sink = LogRestore { log: Arc::default(), copy: Mutex::new(None) };
    let r = restore_question(&mut banks, &ask(&link, Some(&a), QA, &b), &sink, &|_: &[QuestionBank]| Ok(()), NOW + 100_000);
    assert_eq!(r.map(|d| d.version), Err(RestoreRefused::NotCountable), "a figure-only difference slipped in through a deleted question");
    assert_eq!(json_of(&banks), before);
    assert!(sink.log.lock().unwrap().is_empty());
}

#[test]
fn d2i_restoring_the_base_version_is_allowed_without_a_base_record() {
    // (round 6, critic N1) a base too big to keep (no record): the strip says the count is not fully known, never "in step",
    // so restoring the base version is allowed even over a figure-settings edit the record could not hold
    let (mut bank, mut link) = linked();
    let a = local_of(&link, QA);
    bank.questions[0].stem = "My unsent edit".into();
    bank.questions[0].figure = Some(crate::model::FigureSpec { id: "triangle".into(), params: [("a".to_string(), json!(3))].into_iter().collect() });
    bank.modified += 10;
    link.base.clear();
    link.mark_bank(&bank);
    let mut banks = vec![bank];
    let sink = LogRestore { log: Arc::default(), copy: Mutex::new(None) };
    let nine = body(9, &v9(), false);
    let r = restore_question(&mut banks, &ask(&link, Some(&a), QA, &nine), &sink, &|_: &[QuestionBank]| Ok(()), NOW + 100_000);
    assert_eq!(r.map(|d| d.version), Ok(9));
}

#[test]
fn d2i_a_deleted_question_comes_back_from_a_counted_version() {
    // (round 6, critic N1) the same deleted question, restored from a version that really changes its words: the picture
    // and words count, so it comes back (never refused for a figure it does not carry).
    let (mut bank, link) = linked();
    let a = local_of(&link, QA);
    bank.questions.retain(|q| q.id != a);
    bank.modified += 10;
    let mut banks = vec![bank];
    let sink = LogRestore { log: Arc::default(), copy: Mutex::new(None) };
    let six = body(6, &v6(), false);
    let r = restore_question(&mut banks, &ask(&link, Some(&a), QA, &six), &sink, &|_: &[QuestionBank]| Ok(()), NOW + 100_000);
    assert_eq!(r.map(|d| d.version), Ok(6));
    assert_eq!(banks[0].questions.len(), 3);
    assert_eq!(banks[0].questions.iter().find(|q| q.id == a).map(|q| q.stem.as_str()), Some("Question A as of version 6"));
}

// ---- round 6, critic N2: a re-encoded picture is the same picture ------------------------------------------------------

#[test]
fn d2i_a_circle_picture_the_store_keeps_as_webp_is_the_same_picture() {
    // a PNG in the circle (sent by an older build, whose files are the hash of a PNG): this build keeps it as WebP, under a
    // new file name. Compare, the page's header ("the same as version 9", never NotCounted) and a restore of that very
    // version read it as the same picture by the link's record of the circle's name for it, never by the local file's.
    let mut png = Vec::new();
    image::DynamicImage::ImageRgb8(image::RgbImage::from_pixel(3, 2, image::Rgb([200, 10, 10])))
        .write_to(&mut std::io::Cursor::new(&mut png), image::ImageFormat::Png)
        .unwrap();
    let circle_sha = sha256_hex(&png);
    let prepared = crate::store::prepare_image(&png).unwrap();
    let local_file = format!("media/{}.webp", sha256_hex(&prepared.bytes));
    assert_ne!(Some(circle_sha.clone()), sha_of_file(&local_file), "the store renames it");
    // version 9 as the pool serves it; the teammate's copy as a copy lands it (picture stored, base from theirs)
    let mut q = mc(QA, "With a picture", 1.0);
    q["image"] = json!({"file": format!("media/{circle_sha}.png"), "width": 3, "height": 2});
    let nine = body(9, &[q.clone()], false);
    let mut theirs = circle_theirs(&payload(&[q]), NOW);
    for t in &mut theirs {
        t.q.image = Some(ImageRef { file: local_file.clone(), width: 3, height: 2 });
    }
    let bank = QuestionBank { id: LOCAL.into(), name: "Cells".into(), questions: theirs.iter().map(|t| t.q.clone()).collect(), created: NOW / 1000, modified: NOW / 1000, details: Default::default() };
    let head = BankHead { id: BID.into(), title: "Cells".into(), license: "CC-BY-4.0".into(), version: 9, version_id: format!("{:032x}", 9), committed_at: NOW - DAY, ..BankHead::default() };
    let base = theirs.iter().map(|t| base_of(&t.q, &t.qid, &t.fp, t.sha.clone())).collect();
    let mut link = CircleLink::from_head(CID, "Biology team", &head, LOCAL, base, NOW);
    link.mark_bank(&bank);
    let a = local_of(&link, QA);
    // the strip: in step, nothing to send
    let c = changes_since(&link, &bank);
    assert_eq!((c.changed, c.added, c.removed, c.known), (0, 0, 0, true));
    assert_eq!(your_copy(&link, &bank, &a), YourCopy::SameAs(9));
    // the picture row of "Your copy" against the very version it is in step with: the same picture
    let cmp = compare(side_of_copy(&link, &bank, &a).as_ref(), side_of_version(&nine, QA).as_ref(), false);
    assert!(cmp.changed_fields().is_empty(), "the copy and the version it is in step with differ only in how this computer named the same picture: {:?}", cmp.changed_fields());
    let pic = cmp.fields.iter().find(|f| f.field == "Picture").unwrap();
    assert_eq!((pic.left.as_str(), pic.right.as_str()), ("the same picture", "the same picture"));
    // a restore of that same version is "already the same", never a picture that has to be fetched again
    let mut banks = vec![bank.clone()];
    let before = json_of(&banks);
    let r = restore_question(&mut banks, &ask(&link, Some(&a), QA, &nine), &BadRestore, &|_| Ok(()), NOW + 1000);
    assert_eq!(r.map(|d| d.version), Err(RestoreRefused::Same));
    assert_eq!(json_of(&banks), before);
}

// ---- round 6, the "should fix" items -----------------------------------------------------------------------------------

#[test]
fn d2l_a_history_lines_author_and_message_are_cut_at_parse() {
    // (round 6, critic) an untrusted reply: 100,000 characters of author and of message are cut when the line is read,
    // never kept whole for the session
    let s = Tape::new();
    let big = "x".repeat(100_000);
    s.json("Get", &format!("/circles/{CID}/banks/{BID}/versions"), 200, json!({"bank_id": BID, "head_version": 1, "versions": [{"version": 1, "kept": true, "by": big, "at": NOW, "message": big}]}), None);
    let p = block_on(Pool::new(s.as_ref()).circle_versions(CID, BID, None)).unwrap();
    assert_eq!(p.versions.len(), 1);
    assert_eq!(p.versions[0].by.chars().count(), super::circle_versions_api::BY_MAX);
    assert_eq!(p.versions[0].message.chars().count(), super::circles_api::MESSAGE_MAX);
}

#[test]
fn d2l_the_same_restore_point_is_not_saved_twice() {
    // (round 6, critic) a second restore point over banks nothing else touched (a retried restore) is the one already kept;
    // banks that moved get a new point, and the first is never written over
    let dir = std::env::temp_dir().join(format!("vershion-d2-r6-restore-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let (mut bank, _) = linked();
    let fs = crate::store::FsBackend::new(dir.clone());
    let first = super::browse::save_copy_to(&fs, std::slice::from_ref(&bank), "restore", NOW / 1000).unwrap();
    let again = super::browse::save_copy_to(&fs, std::slice::from_ref(&bank), "restore", NOW / 1000 + 1).unwrap();
    assert_eq!(first, again, "the same banks are one restore point");
    assert_eq!(std::fs::read_dir(dir.join("backups")).unwrap().count(), 1);
    bank.questions[0].stem = "Changed".into();
    let moved = super::browse::save_copy_to(&fs, std::slice::from_ref(&bank), "restore", NOW / 1000 + 2).unwrap();
    assert_ne!(moved, first);
    assert_eq!(std::fs::read_dir(dir.join("backups")).unwrap().count(), 2);
    let _ = std::fs::remove_dir_all(&dir);
}

// ---- round 5 critics' probes (r7) ---------------------------------------------------------------------------------------

/// A bank in step with `qs` as version 9, its link, with the circle's pictures named in the payload as given.
fn linked_with(qs: &[Value]) -> (QuestionBank, CircleLink) {
    let theirs = circle_theirs(&payload(qs), NOW);
    let bank = QuestionBank { id: LOCAL.into(), name: "Cells".into(), questions: theirs.iter().map(|t| t.q.clone()).collect(), created: NOW / 1000, modified: NOW / 1000, details: Default::default() };
    let head = BankHead { id: BID.into(), title: "Cells".into(), license: "CC-BY-4.0".into(), version: 9, version_id: format!("{:032x}", 9), committed_at: NOW - DAY, ..BankHead::default() };
    let base = theirs.iter().map(|t| base_of(&t.q, &t.qid, &t.fp, t.sha.clone())).collect();
    let mut link = CircleLink::from_head(CID, "Biology team", &head, LOCAL, base, NOW);
    link.mark_bank(&bank);
    (bank, link)
}

#[test]
fn d2m_a_picture_fetch_does_not_skip_the_not_countable_guard() {
    // The NotCountable guard sits under `if !deferred`, so a restore that must FETCH a picture never runs it.
    // A locally DELETED question; version 6 has version 9's words and the base's own picture (so the picture does
    // not count as a change) but other figure settings (which the strip cannot count, and Snap does not hold at
    // all). The restore lands and the strip says "in step" (0 changes): exactly the hole r6 claimed to close for a
    // deleted question, defeated by adding a picture.
    let (mut bank, mut link) = linked();
    let a = local_of(&link, QA);
    let sha = format!("{:064x}", 0x5a);
    let file = format!("media/{sha}.webp");
    // the base recorded the picture the copy landed with, and the circle's name for it
    link.base[0].snap.image = Some(sha.clone());
    link.base[0].local_image = Some(file.clone());
    // the person deleted the question (unsent)
    bank.questions.retain(|q| q.id != a);
    bank.modified += 10;
    // version 6: version 9's words, the same picture, other figure settings
    let mut q = mc(QA, "Question A as of version 9", 1.0);
    q["image"] = json!({"file": format!("media/{sha}.png"), "width": 3, "height": 2});
    q["figure"] = json!({"id": "triangle", "params": {"a": 4}});
    let b = body(6, &[q, mc(QB, "Question B as of version 8", 1.0), mc(QC, "Question C", 1.0)], false);
    assert_eq!(b.question(QA).and_then(|t| t.sha.clone()).as_deref(), Some(sha.as_str()), "the version carries the picture");
    let mut banks = vec![bank];
    let before = json_of(&banks);
    let sink = LogRestore { log: Arc::default(), copy: Mutex::new(None) };
    let picture = ImageRef { file: file.clone(), width: 3, height: 2 };
    let store = move || Ok(picture.clone());
    let with = RestoreAsk { store_picture: Some(&store), ..ask(&link, Some(&a), QA, &b) };
    let r = restore_question(&mut banks, &with, &sink, &|_: &[QuestionBank]| Ok(()), NOW + 100_000);
    let after = banks[0].questions.iter().find(|q| q.id == a).cloned();
    assert_eq!(
        r.map(|d| d.version),
        Err(RestoreRefused::NotCountable),
        "a picture fetch skipped the NotCountable guard: restored figure = {:?}, strip count = {:?}, banks before = {before}",
        after.as_ref().and_then(|q| q.figure.clone()),
        changes_since(&link, &banks[0])
    );
}

#[test]
fn d2m_a_restore_that_undoes_an_unsent_edit_is_not_refused_as_figure_only() {
    // N1 was fixed by skipping the count for the base version's own number. A version that is not the base can hold the
    // base version's question exactly (the question did not change between them): restoring it is the same restore.
    // Here the copy has an unsent stem edit AND a different figure; version 6's QA is v9's QA exactly (text and figure),
    // so the restore brings the copy to v9 in step (a counted change undone). It must not be refused as "figure settings
    // only".
    let mut q9 = mc(QA, "Question A as of version 9", 1.0);
    q9["figure"] = json!({"id": "triangle", "params": {"a": 4}});
    let v9q = vec![q9, mc(QB, "Question B as of version 8", 1.0), mc(QC, "Question C", 1.0)];
    let (mut bank, link) = linked_with(&v9q);
    let a = local_of(&link, QA);
    bank.questions.iter_mut().find(|q| q.id == a).unwrap().stem = "My unsent edit".into();
    bank.questions.iter_mut().find(|q| q.id == a).unwrap().figure = Some(crate::model::FigureSpec { id: "triangle".into(), params: [("a".to_string(), json!(3))].into_iter().collect() });
    bank.modified += 10;
    // the strip counts the unsent stem edit
    assert_eq!(changes_since(&link, &bank).changed, 1);
    let mut banks = vec![bank];
    let sink = LogRestore { log: Arc::default(), copy: Mutex::new(None) };
    let six = body(6, &v9q, false);
    let r = restore_question(&mut banks, &ask(&link, Some(&a), QA, &six), &sink, &|_| Ok(()), NOW + 100_000);
    assert_eq!(r.map(|d| d.version), Ok(6), "restoring a version whose question is the base version's exactly was refused");
    assert_eq!(changes_since(&link, &banks[0]).changed, 0, "the copy is now the base version, in step");
}

#[test]
fn d2m_a_picture_a_restore_stored_under_its_own_name_compares_as_the_same_picture() {
    // N2 compares a picture by the circle's name for it through the link's record (`local_image`). A restore that
    // fetches and stores a PNG/JPEG version's picture writes a new file (the store keeps it as WebP), and nothing puts
    // that file in the link's record. Comparing "Your copy" with the very version restored then falls back to the local
    // file's own name and calls the same picture "a different picture".
    let mut png = Vec::new();
    image::DynamicImage::ImageRgb8(image::RgbImage::from_pixel(3, 2, image::Rgb([200, 10, 10])))
        .write_to(&mut std::io::Cursor::new(&mut png), image::ImageFormat::Png)
        .unwrap();
    let circle_sha = sha256_hex(&png);
    let prepared = crate::store::prepare_image(&png).unwrap();
    let stored_file = format!("media/{}.webp", sha256_hex(&prepared.bytes));
    assert_ne!(stored_file, format!("media/{circle_sha}.png"), "the store renames it");
    // version 6: a question the circle has with a picture (a PNG in the circle's store)
    let mut q = mc(QA, "Question A as of version 6", 2.0);
    q["image"] = json!({"file": format!("media/{circle_sha}.png"), "width": 3, "height": 2});
    let six = body(6, &[q, mc(QB, "Question B", 1.0), mc(QC, "Question C", 1.0)], false);
    // the copy in step with v9, which has no picture on this question: base.local_image is none
    let (bank, link) = linked();
    let a = local_of(&link, QA);
    let mut banks = vec![bank];
    let sink = LogRestore { log: Arc::default(), copy: Mutex::new(None) };
    let (p2f, p2w, p2h) = (stored_file.clone(), prepared.width, prepared.height);
    let store = move || -> Result<ImageRef, String> { Ok(ImageRef { file: p2f.clone(), width: p2w, height: p2h }) };
    let with = RestoreAsk { link: &link, local_id: Some(&a), qid: QA, body: &six, picture: None, store_picture: Some(&store), removed_in_circle: false };
    let done = restore_question(&mut banks, &with, &sink, &|_| Ok(()), NOW + 1000).unwrap();
    assert_eq!(done.version, 6);
    assert_eq!(banks[0].questions.iter().find(|q| q.id == a).and_then(|q| q.image.clone()).map(|i| i.file), Some(stored_file));
    // the copy is now version 6's question, picture included. (The critic's probe compared with the pre-restore link; the
    // picture record the restore writes is in the link it hands back, so that is the one the page keeps and this reads.)
    let kept = done.link.clone().unwrap_or_else(|| link.clone());
    let cmp = compare(side_of_copy(&kept, &banks[0], &a).as_ref(), side_of_version(&six, QA).as_ref(), false);
    assert!(cmp.changed_fields().is_empty(), "the copy IS version 6's question, but Compare says {:?}", cmp.changed_fields());
    let pic = cmp.fields.iter().find(|f| f.field == "Picture").unwrap();
    assert_eq!((pic.left.as_str(), pic.right.as_str()), ("the same picture", "the same picture"));
}

#[test]
fn d2m_the_restore_point_dedupe_compares_against_the_newest_copy() {
    // `save_copy_to` sorts the names and takes the LAST, but within one stamp `…-2.json` sorts BEFORE `….json`
    // ('.' 0x2E > '-' 0x2D). So "the newest copy of that kind" is really the FIRST copy made in the newest second:
    // an identical restore point made later in the same second is written again (dedupe misses it).
    let dir = std::env::temp_dir().join(format!("vershion-d2-r6-dedupe-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let (bank, _) = linked();
    let fs = crate::store::FsBackend::new(dir.clone());
    let secs = NOW / 1000;
    let mut moved = bank.clone();
    moved.questions[0].stem = "Changed".into();
    let b0 = super::browse::save_copy_to(&fs, std::slice::from_ref(&bank), "restore", secs).unwrap();
    let b1 = super::browse::save_copy_to(&fs, std::slice::from_ref(&moved), "restore", secs).unwrap();
    assert_ne!(b0, b1);
    // the same banks as the *newest* copy (`b1`), made again in the same second: should be `b1`, not a third file
    let again = super::browse::save_copy_to(&fs, std::slice::from_ref(&moved), "restore", secs).unwrap();
    let files = std::fs::read_dir(dir.join("backups")).unwrap().count();
    let _ = std::fs::remove_dir_all(&dir);
    assert_eq!(again, b1, "the newest restore point was not the one compared; {} files kept", files);
}

#[test]
fn d2m_a_history_lines_version_id_is_kept_whole() {
    // r6 caps `by` and `message` at parse, but the same untrusted line still carries `version_id` (and `license`,
    // `review_copy`) unbounded, and these are kept for the session (50 lines per bank, 8 banks). A 100,000-char
    // `version_id` is kept whole, so the cap is incomplete: the "never kept whole" claim covers only two fields.
    let s = Tape::new();
    let big = "v".repeat(100_000);
    s.json("Get", &format!("/circles/{CID}/banks/{BID}/versions"), 200, json!({"bank_id": BID, "head_version": 1, "versions": [{"version": 1, "kept": true, "version_id": big, "by": "x", "at": NOW, "message": "m"}]}), None);
    let p = block_on(Pool::new(s.as_ref()).circle_versions(CID, BID, None)).unwrap();
    assert!(p.versions[0].version_id.chars().count() <= 200, "version_id kept whole: {} chars", p.versions[0].version_id.chars().count());
}

// ---- round 6 critics' probes (r8) ---------------------------------------------------------------------------------------

#[test]
fn d2n_a_text_edit_never_lets_an_uncounted_figure_move_pass() {
    // R2's fix made the NotCountable guard "undoable = the copy has a figure AND any unsent change". But any unsent
    // change is enough: an unsent STEM edit alone lets a restore carry a figure the base record can not count, while
    // the counted change it undoes (the stem) is also undone -- so the strip lands on 0 ("up to date") with the figure
    // silently moved. The invariant R1/N1/R2 all chase: never let a restore leave the copy "in step" over a figure move.
    let mut q9 = mc(QA, "Question A as of version 9", 1.0);
    q9["figure"] = json!({"id": "triangle", "params": {"a": 3}});
    let v9q = vec![q9, mc(QB, "Question B as of version 8", 1.0), mc(QC, "Question C", 1.0)];
    let (mut bank, link) = linked_with(&v9q);
    let a = local_of(&link, QA);
    {
        let q = bank.questions.iter_mut().find(|q| q.id == a).unwrap();
        // the person edited only the STEM; the figure is still the base's own a:3
        q.stem = "My unsent stem edit".into();
        assert_eq!(q.figure.as_ref().map(|f| f.params.get("a").cloned()), Some(Some(json!(3))), "the copy's figure is the base's");
    }
    bank.modified += 10;
    assert_eq!(changes_since(&link, &bank).changed, 1, "the strip counts the stem edit");
    // version 6's QA carries the BASE version's words exactly, but a DIFFERENT figure (a:9)
    let mut q6 = mc(QA, "Question A as of version 9", 1.0);
    q6["figure"] = json!({"id": "triangle", "params": {"a": 9}});
    let six = body(6, &[q6, mc(QB, "Question B as of version 8", 1.0), mc(QC, "Question C", 1.0)], false);
    let mut banks = vec![bank];
    let sink = LogRestore { log: Arc::default(), copy: Mutex::new(None) };
    let r = restore_question(&mut banks, &ask(&link, Some(&a), QA, &six), &sink, &|_| Ok(()), NOW + 100_000);
    let after = banks[0].questions.iter().find(|q| q.id == a).cloned();
    let count = changes_since(&link, &banks[0]);
    assert_eq!(
        r.map(|d| d.version),
        Err(RestoreRefused::NotCountable),
        "a text edit let an uncounted figure move through: restored figure = {:?}, strip says changed = {} (up to date)",
        after.as_ref().and_then(|q| q.figure.clone()),
        count.changed
    );
}

#[test]
fn d2n_a_changes_reply_strings_are_cut_at_parse() {
    // M2/r6 caps the history *lines*; the `/changes` reply is held for the session too (8 banks) and its texts are not
    // capped: `reason`, `bank_id`, and every qid in the added/changed/removed lists. Bounded only by the 1 MB response
    // cap, so up to 8 MB of untrusted text can be kept, each string whole. Same gap the r6/r7 cap claims to close.
    let s = Tape::new();
    let big = "r".repeat(100_000);
    let qid = "q".repeat(100_000);
    s.json("Get", &format!("/circles/{CID}/banks/{BID}/changes"), 200, json!({"bank_id": BID, "head_version": 2, "reason": big, "versions": [{"version": 2, "compared_to": 1, "added": [qid]}]}), None);
    let p = block_on(Pool::new(s.as_ref()).circle_changes(CID, BID)).unwrap();
    assert!(p.reason.chars().count() <= 200, "changes reason kept whole: {} chars", p.reason.chars().count());
    assert!(p.versions[0].added[0].chars().count() <= 200, "changes qid kept whole: {} chars", p.versions[0].added[0].chars().count());
}

#[test]
fn d2n_a_figure_not_in_the_base_is_refused() {
    let mut q9 = mc(QA, "Question A as of version 9", 1.0);
    q9["figure"] = json!({"id": "triangle", "params": {"a": 4}});
    let v9q = vec![q9, mc(QB, "Question B as of version 8", 1.0), mc(QC, "Question C", 1.0)];
    let (mut bank, link) = linked_with(&v9q);
    let a = local_of(&link, QA);
    // the person edited the stem (counted) and set the figure to a = 3 (uncounted)
    {
        let q = bank.questions.iter_mut().find(|q| q.id == a).unwrap();
        q.stem = "My unsent edit".into();
        q.figure = Some(crate::model::FigureSpec { id: "triangle".into(), params: [("a".to_string(), json!(3))].into_iter().collect() });
    }
    bank.modified += 10;
    assert_eq!(changes_since(&link, &bank).changed, 1, "the strip counts the stem edit");
    // version 6: the SAME countable fields as v9, but its figure is a = 5 (v9's, the base's, is a = 4)
    let mut q6 = mc(QA, "Question A as of version 9", 1.0);
    q6["figure"] = json!({"id": "triangle", "params": {"a": 5}});
    let six = body(6, &[q6, mc(QB, "Question B as of version 8", 1.0), mc(QC, "Question C", 1.0)], false);
    let mut banks = vec![bank];
    let sink = LogRestore { log: Arc::default(), copy: Mutex::new(None) };
    let r = restore_question(&mut banks, &ask(&link, Some(&a), QA, &six), &sink, &|_| Ok(()), NOW + 100_000);
    let rv = r.map(|d| d.version);
    let after_fig = banks[0].questions.iter().find(|q| q.id == a).and_then(|q| q.figure.clone());
    let strip = changes_since(&link, &banks[0]);
    assert!(
        rv == Err(RestoreRefused::NotCountable),
        "R2 allows a figure that is not the base's: restore gave {rv:?}; the copy's figure is now {after_fig:?} (version 9's is a = 4); strip changed = {}",
        strip.changed
    );
    // the refusal changed nothing: her own unsent stem edit is still the one change the strip counts
    assert_eq!(strip.changed, 1, "nothing was restored, so the stem edit is still to send");
    assert_eq!(after_fig.as_ref().map(|f| f.params.get("a").cloned()), Some(Some(json!(3))), "her figure is untouched");
}

#[test]
fn d2n_a_restore_landing_in_step_is_allowed_with_no_copy_figure() {
    let mut q9 = mc(QA, "Question A as of version 9", 1.0);
    q9["figure"] = json!({"id": "triangle", "params": {"a": 4}});
    let v9q = vec![q9, mc(QB, "Question B as of version 8", 1.0), mc(QC, "Question C", 1.0)];
    let (mut bank, link) = linked_with(&v9q);
    let a = local_of(&link, QA);
    // the person edited the stem (counted) and removed the figure (uncounted)
    {
        let q = bank.questions.iter_mut().find(|q| q.id == a).unwrap();
        q.stem = "My unsent edit".into();
        q.figure = None;
    }
    bank.modified += 10;
    assert_eq!(changes_since(&link, &bank).changed, 1);
    // version 6's QA is v9's QA exactly (stem and figure a = 4): restoring lands the copy on the base
    let mut q6 = mc(QA, "Question A as of version 9", 1.0);
    q6["figure"] = json!({"id": "triangle", "params": {"a": 4}});
    let six = body(6, &[q6, mc(QB, "Question B as of version 8", 1.0), mc(QC, "Question C", 1.0)], false);
    let mut banks = vec![bank];
    let sink = LogRestore { log: Arc::default(), copy: Mutex::new(None) };
    let r = restore_question(&mut banks, &ask(&link, Some(&a), QA, &six), &sink, &|_| Ok(()), NOW + 100_000);
    let rv = r.map(|d| d.version);
    assert!(rv == Ok(6), "a restore that lands the copy in step was refused because the copy had no figure: {rv:?}");
}

#[test]
fn d2n_an_unpaired_restore_compares_as_the_same_picture() {
    let mut png = Vec::new();
    image::DynamicImage::ImageRgb8(image::RgbImage::from_pixel(3, 2, image::Rgb([200, 10, 10])))
        .write_to(&mut std::io::Cursor::new(&mut png), image::ImageFormat::Png)
        .unwrap();
    let circle_sha = sha256_hex(&png);
    let prepared = crate::store::prepare_image(&png).unwrap();
    let stored_file = format!("media/{}.webp", sha256_hex(&prepared.bytes));
    assert_ne!(stored_file, format!("media/{circle_sha}.png"), "the store renames it");
    // the copy is in step with v9, whose QA carries the circle's picture (kept here as a WebP under its own name)
    let mut q9 = mc(QA, "Question A as of version 9", 1.0);
    q9["image"] = json!({"file": format!("media/{circle_sha}.png"), "width": 3, "height": 2});
    let v9q = vec![q9, mc(QB, "Question B as of version 8", 1.0), mc(QC, "Question C", 1.0)];
    let (mut bank, mut link) = linked_with(&v9q);
    let a = local_of(&link, QA);
    link.base.iter_mut().find(|b| b.local_id == a).unwrap().local_image = Some(stored_file.clone());
    bank.questions.iter_mut().find(|q| q.id == a).unwrap().image = Some(ImageRef { file: stored_file.clone(), width: prepared.width, height: prepared.height });
    link.mark_bank(&bank);
    // version 6 has QA with the SAME circle picture
    let mut q6 = mc(QA, "Question A as of version 6", 2.0);
    q6["image"] = json!({"file": format!("media/{circle_sha}.png"), "width": 3, "height": 2});
    let six = body(6, &[q6, mc(QB, "Question B", 1.0), mc(QC, "Question C", 1.0)], false);
    let mut banks = vec![bank];
    let sink = LogRestore { log: Arc::default(), copy: Mutex::new(None) };
    // the circle removed QA: restoring brings it back new (unpaired)
    let with = RestoreAsk { link: &link, local_id: Some(&a), qid: QA, body: &six, picture: None, store_picture: None, removed_in_circle: true };
    let done = restore_question(&mut banks, &with, &sink, &|_| Ok(()), NOW + 1000).unwrap();
    assert!(done.added, "the question comes back new");
    assert!(!done.link.as_ref().unwrap().base.iter().any(|b| b.local_id == a), "the base entry is removed on unpair");
    // the copy IS version 6's question, picture included: Compare must say the same picture
    let kept = done.link.clone().unwrap();
    let cmp = compare(side_of_copy(&kept, &banks[0], &a).as_ref(), side_of_version(&six, QA).as_ref(), false);
    let pic = cmp.fields.iter().find(|f| f.field == "Picture").unwrap();
    assert!(
        (pic.left.as_str(), pic.right.as_str(), pic.changed) == ("the same picture", "the same picture", false),
        "the copy IS version 6's question, but Compare says {} / {} (changed = {})",
        pic.left,
        pic.right,
        pic.changed
    );
}

#[test]
fn d2n_a_changes_reply_and_merge_start_are_capped_at_parse() {
    // (round 8, critic S) the /changes bank id and the merge answer's line count are bounded like the history lines
    let s = Tape::new();
    let big = "b".repeat(100_000);
    s.json("Get", &format!("/circles/{CID}/banks/{BID}/changes"), 200, json!({"bank_id": big, "head_version": 1, "versions": []}), None);
    let p = block_on(Pool::new(s.as_ref()).circle_changes(CID, BID)).unwrap();
    assert!(p.bank_id.chars().count() <= super::circle_versions_api::ID_MAX, "bank_id kept whole: {} chars", p.bank_id.chars().count());
    // a merge answer naming 500 versions: never more than one history page is kept
    let lines: Vec<Value> = (1..=500u64).map(|v| json!({"version": v, "kept": true, "by": "x", "at": NOW, "message": "m"})).collect();
    s.json("Post", &format!("/circles/{CID}/banks/{BID}/merge"), 200, json!({"bank_id": BID, "base_version": 1, "base_kept": true, "head_version": 500, "head_version_id": format!("{:032x}", 500), "head_at": NOW, "head_by": big, "between": lines, "between_total": 499, "merge_token": "tok"}), None);
    let m = block_on(Pool::new(s.as_ref()).merge_start(CID, BID, 1)).unwrap();
    assert_eq!(m.between.len(), super::circle_versions_api::HISTORY_PAGE);
    assert!(m.head_by.chars().count() <= super::circle_versions_api::BY_MAX);
}

// ---- round 8 product critic's probes -------------------------------------------------------------------------------------

#[test]
fn d2o_picture_records_kept_below_the_cap_still_compare() {
    // 33 unpaired picture restores in one session: PICTURE_FILES_MAX keeps 32, evicting the oldest. A question whose
    // picture is still the first one's then compares as "a different picture", because its base entry went with the unpair.
    let n = 33usize;
    let qid = |i: usize| format!("c{:031}", i);
    let sha9 = |i: usize| format!("{:064x}", 0x9000 + i);
    let sha6 = |i: usize| format!("{:064x}", 0x6000 + i);
    let stored = |i: usize| format!("media/{:064x}.webp", 0x1000 + i);
    let v9q: Vec<Value> = (0..n)
        .map(|i| {
            let mut q = mc(&qid(i), &format!("Question {i} as of version 9"), 1.0);
            q["image"] = json!({"file": format!("media/{}.png", sha9(i)), "width": 3, "height": 2});
            q
        })
        .collect();
    let v6q: Vec<Value> = (0..n)
        .map(|i| {
            let mut q = mc(&qid(i), &format!("Question {i} as of version 9"), 1.0);
            q["image"] = json!({"file": format!("media/{}.png", sha6(i)), "width": 3, "height": 2});
            q
        })
        .collect();
    let (bank, mut link) = linked_with(&v9q);
    let locals: Vec<String> = (0..n).map(|i| local_of(&link, &qid(i))).collect();
    let mut banks = vec![bank];
    let six = body(6, &v6q, false);
    let sink = LogRestore { log: Arc::default(), copy: Mutex::new(None) };
    for i in 0..n {
        let pic = ImageRef { file: stored(i), width: 3, height: 2 };
        let with = RestoreAsk { link: &link, local_id: Some(&locals[i]), qid: &qid(i), body: &six, picture: Some(pic), store_picture: None, removed_in_circle: true };
        let done = restore_question(&mut banks, &with, &sink, &|_| Ok(()), NOW + 1000 + i as i64).unwrap();
        link = done.link.clone().unwrap_or(link);
    }
    // (round 9, critic M4) 33 restores are all kept: the cap is far above a session's unpaired restores, and what it drops
    // are the oldest names, never the newest
    assert_eq!(link.picture_files.len(), n, "the map keeps every restore below its cap");
    let cmp = compare(side_of_copy(&link, &banks[0], &locals[0]).as_ref(), side_of_version(&six, &qid(0)).as_ref(), false);
    let pic = cmp.fields.iter().find(|f| f.field == "Picture").unwrap();
    assert!(
        (pic.left.as_str(), pic.right.as_str(), pic.changed) == ("the same picture", "the same picture", false),
        "after 33 restores the first picture's record is gone: Compare says {} / {} (changed = {})",
        pic.left,
        pic.right,
        pic.changed
    );
}

#[test]
fn d2o_an_old_links_unknown_figure_may_not_vanish() {
    // A link an earlier build wrote has no figure record. Restoring a version whose question has NO figure, over a base
    // version whose question HAD one, moves the copy off the base's figure with nothing countable left: the guard does not
    // fire (None == None), and the strip then says 0 changed.
    let mut q9 = mc(QA, "Question A as of version 9", 1.0);
    q9["figure"] = json!({"id": "triangle", "params": {"a": 4}});
    let v9q = vec![q9, mc(QB, "Question B as of version 8", 1.0), mc(QC, "Question C", 1.0)];
    let (mut bank, mut link) = linked_with(&v9q);
    // as an earlier build wrote it: the base version's figure is not recorded at all (round 10: `None` alone no longer
    // means that, so the record itself must say it does not know)
    link.base[0].figure = None;
    link.base[0].figure_recorded = false;
    let a = local_of(&link, QA);
    bank.questions.iter_mut().find(|q| q.id == a).unwrap().stem = "My unsent edit".into();
    bank.modified += 10;
    assert_eq!(changes_since(&link, &bank).changed, 1, "the strip counts the stem edit");
    let six = body(6, &[mc(QA, "Question A as of version 9", 1.0), mc(QB, "Question B as of version 8", 1.0), mc(QC, "Question C", 1.0)], false);
    let mut banks = vec![bank];
    let sink = LogRestore { log: Arc::default(), copy: Mutex::new(None) };
    let r = restore_question(&mut banks, &ask(&link, Some(&a), QA, &six), &sink, &|_| Ok(()), NOW + 100_000);
    let fig = banks[0].questions.iter().find(|q| q.id == a).and_then(|q| q.figure.clone());
    let count = changes_since(&link, &banks[0]).changed;
    let restored = r.map(|d| d.version);
    assert!(
        !(restored == Ok(6) && fig.is_none() && count == 0),
        "restore allowed and the strip says up to date over the base version's figure: restore {restored:?}, copy figure {fig:?}, strip changed {count}"
    );
}

#[test]
fn d2o_an_unpaired_restore_that_stores_the_picture_compares_as_the_same() {
    // The builder's unpaired test reuses the copy's picture; the store path (PNG -> WebP under a new name) is the one the
    // brief named. Confirm the link-level record covers it too.
    let mut png = Vec::new();
    image::DynamicImage::ImageRgb8(image::RgbImage::from_pixel(3, 2, image::Rgb([20, 200, 10])))
        .write_to(&mut std::io::Cursor::new(&mut png), image::ImageFormat::Png)
        .unwrap();
    let circle_sha = sha256_hex(&png);
    let prepared = crate::store::prepare_image(&png).unwrap();
    let stored_file = format!("media/{}.webp", sha256_hex(&prepared.bytes));
    // the copy in step with v9: QA has no picture
    let (bank, link) = linked();
    let a = local_of(&link, QA);
    // version 6: QA with the circle's picture (a PNG in the circle's store)
    let mut q6 = mc(QA, "Question A as of version 9", 1.0);
    q6["image"] = json!({"file": format!("media/{circle_sha}.png"), "width": 3, "height": 2});
    let six = body(6, &[q6, mc(QB, "Question B as of version 8", 1.0), mc(QC, "Question C", 1.0)], false);
    let mut banks = vec![bank];
    let sink = LogRestore { log: Arc::default(), copy: Mutex::new(None) };
    let (f, w, h) = (stored_file.clone(), prepared.width, prepared.height);
    let store = move || -> Result<ImageRef, String> { Ok(ImageRef { file: f.clone(), width: w, height: h }) };
    // the circle removed QA: the restore brings it back new (unpaired) and must store the picture
    let with = RestoreAsk { link: &link, local_id: Some(&a), qid: QA, body: &six, picture: None, store_picture: Some(&store), removed_in_circle: true };
    let done = restore_question(&mut banks, &with, &sink, &|_| Ok(()), NOW + 1000).unwrap();
    assert!(done.added && !done.link.as_ref().unwrap().base.iter().any(|b| b.local_id == a));
    let kept = done.link.clone().unwrap();
    let cmp = compare(side_of_copy(&kept, &banks[0], &a).as_ref(), side_of_version(&six, QA).as_ref(), false);
    let pic = cmp.fields.iter().find(|f| f.field == "Picture").unwrap();
    assert!(
        (pic.left.as_str(), pic.right.as_str(), pic.changed) == ("the same picture", "the same picture", false),
        "the stored picture is version 6's, but Compare says {} / {} (changed = {})",
        pic.left,
        pic.right,
        pic.changed
    );
}

#[test]
fn d2o_a_figure_move_with_a_counted_change_keeps_the_strip_truthful() {
    // The guard allows a figure move when the restored question itself differs from the base in something countable. The
    // strip must then count it: never "up to date" over the figure.
    let mut q9 = mc(QA, "Question A as of version 9", 1.0);
    q9["figure"] = json!({"id": "triangle", "params": {"a": 4}});
    let v9q = vec![q9, mc(QB, "Question B as of version 8", 1.0), mc(QC, "Question C", 1.0)];
    let (mut bank, link) = linked_with(&v9q);
    let a = local_of(&link, QA);
    bank.questions.iter_mut().find(|q| q.id == a).unwrap().stem = "My unsent edit".into();
    bank.questions.iter_mut().find(|q| q.id == a).unwrap().figure = Some(crate::model::FigureSpec { id: "triangle".into(), params: [("a".to_string(), json!(3))].into_iter().collect() });
    bank.modified += 10;
    // version 6's QA carries a different stem (a countable change from the base) and a different figure
    let mut q6 = mc(QA, "Question A, the older wording", 1.0);
    q6["figure"] = json!({"id": "triangle", "params": {"a": 5}});
    let six = body(6, &[q6, mc(QB, "Question B as of version 8", 1.0), mc(QC, "Question C", 1.0)], false);
    let mut banks = vec![bank];
    let sink = LogRestore { log: Arc::default(), copy: Mutex::new(None) };
    let r = restore_question(&mut banks, &ask(&link, Some(&a), QA, &six), &sink, &|_| Ok(()), NOW + 100_000);
    let count = changes_since(&link, &banks[0]).changed;
    assert_eq!(r.map(|d| d.version), Ok(6), "a countable change plus a figure move must be restorable");
    assert!(count >= 1, "the strip must count the restored question, got changed = {count}");
}

#[test]
fn d2o_a_partial_send_keeps_a_held_back_restored_pictures_name() {
    // The store renames a circle PNG to WebP; the restore records W -> S on the link. A partial send that leaves that
    // restore unticked clears the record (after_partial_send), and the held question's base is rebuilt from the base
    // body (which has no picture), so nothing names W on the circle any more and Compare says "a different picture"
    // against the very version restored.
    let mut png = Vec::new();
    image::DynamicImage::ImageRgb8(image::RgbImage::from_pixel(3, 2, image::Rgb([10, 20, 200])))
        .write_to(&mut std::io::Cursor::new(&mut png), image::ImageFormat::Png)
        .unwrap();
    let circle_sha = sha256_hex(&png);
    let prepared = crate::store::prepare_image(&png).unwrap();
    let stored_file = format!("media/{}.webp", sha256_hex(&prepared.bytes));
    let (bank, link) = linked();
    let a = local_of(&link, QA);
    // version 6: QA with the circle's picture (a PNG in the circle's store)
    let mut q6 = mc(QA, "Question A as of version 9", 1.0);
    q6["image"] = json!({"file": format!("media/{circle_sha}.png"), "width": 3, "height": 2});
    let six = body(6, &[q6, mc(QB, "Question B as of version 8", 1.0), mc(QC, "Question C", 1.0)], false);
    let mut banks = vec![bank];
    let sink = LogRestore { log: Arc::default(), copy: Mutex::new(None) };
    let (f, w, h) = (stored_file.clone(), prepared.width, prepared.height);
    let store = move || -> Result<ImageRef, String> { Ok(ImageRef { file: f.clone(), width: w, height: h }) };
    let with = RestoreAsk { link: &link, local_id: Some(&a), qid: QA, body: &six, picture: None, store_picture: Some(&store), removed_in_circle: false };
    let done = restore_question(&mut banks, &with, &sink, &|_| Ok(()), NOW + 1000).unwrap();
    let kept = done.link.clone().unwrap();
    assert_eq!(kept.picture_files, vec![(stored_file.clone(), circle_sha.clone())], "the restore records the name");
    // the send page: that change is held back (unticked); the composed question is the base body's (v9's, no picture)
    let q9 = v9();
    let composed: Vec<super::partial_send::ComposedQuestion> = vec![
        super::partial_send::ComposedQuestion { local: a.clone(), qid: Some(QA.into()), question: Some(serde_json::from_value(q9[0].clone()).unwrap()), base_image: None },
        super::partial_send::ComposedQuestion { local: local_of(&kept, QB), qid: Some(QB.into()), question: Some(serde_json::from_value(q9[1].clone()).unwrap()), base_image: None },
        super::partial_send::ComposedQuestion { local: local_of(&kept, QC), qid: Some(QC.into()), question: Some(serde_json::from_value(q9[2].clone()).unwrap()), base_image: None },
    ];
    let sent = super::circles_api::SentBank { circle_id: CID.into(), bank_id: BID.into(), version: 10, version_id: format!("{:032x}", 10), qids: vec![QA.into(), QB.into(), QC.into()], ..Default::default() };
    let after = super::circle_links::after_partial_send(&kept, &banks[0], &composed, &sent, vec![a.clone()]);
    // (round 9, critic T1) the held-back question keeps its picture record; the two sent questions have rebuilt bases that
    // name their pictures, so nothing of theirs is left to keep
    assert_eq!(after.picture_files, vec![(stored_file.clone(), circle_sha.clone())], "the partial send kept the held-back record");
    let cmp = compare(side_of_copy(&after, &banks[0], &a).as_ref(), side_of_version(&six, QA).as_ref(), false);
    let pic = cmp.fields.iter().find(|f| f.field == "Picture").unwrap();
    assert!(
        (pic.left.as_str(), pic.right.as_str(), pic.changed) == ("the same picture", "the same picture", false),
        "the partial send lost the name of the picture version 6's restore stored: Compare says {} / {} (changed = {})",
        pic.left,
        pic.right,
        pic.changed
    );
}

#[test]
fn d2p_a_merge_keeps_the_picture_record_of_an_unpaired_restore() {
    // (round 9's test pinned the opposite: finish_merge cleared the whole map.) A merge rebuilds the base records of the
    // questions the circle has, but a question it no longer has (an unpaired restore) keeps no record: the link-level name
    // for the file the restore stored is the only thing that says the copy's picture is the version's, and clearing it
    // makes Compare report "a different picture" against the very version restored.
    let src = include_str!("circle_edit.rs");
    let fm = src.find("fn finish_merge").expect("finish_merge");
    let next = src[fm..].find("\n    }\n").map(|i| fm + i).unwrap_or(src.len());
    let body = &src[fm..next];
    assert!(!body.contains("picture_files.clear()"), "the merge wipes every picture name, including one it never rebuilt");
    assert!(body.contains("picture_files_kept"), "the merge must keep the names of the files its rebuilt records do not name");

    // and the rule itself: the circle no longer has A (the base entry and the pair are gone); its stored file keeps its name
    let (mut bank, mut link) = linked();
    let a = local_of(&link, QA);
    let file = format!("media/{}.webp", format!("{:064x}", 0x33u8));
    let sha = format!("{:064x}", 0x44u8);
    bank.questions.iter_mut().find(|q| q.id == a).unwrap().image = Some(ImageRef { file: file.clone(), width: 1, height: 1 });
    link.base.retain(|b| b.local_id != a);
    link.qids.retain(|q| q.local != a);
    link.picture_files.push((file.clone(), sha.clone()));
    assert_eq!(picture_files_kept(&link.picture_files, &link.base, &bank), vec![(file.clone(), sha.clone())], "the unpaired restore's name is kept");
    // once no question has the file any more it is dropped, so the map does not grow for ever
    bank.questions.retain(|q| q.id != a);
    assert!(picture_files_kept(&link.picture_files, &link.base, &bank).is_empty(), "a name for a file the bank no longer has is kept");
}

#[test]
fn d2p_two_questions_sharing_one_stored_picture_keep_the_held_back_ones_name() {
    // (round 10, critic minor) The partial send's retain dropped a name by FILE. A stored file is content-addressed, so two
    // questions restored from the same picture share it: sending one rebuilt that question's record naming the file, which
    // dropped the name the held-back question still needs (its record is the base body's, which has no picture).
    let (mut bank, mut link) = linked();
    let (a, b) = (local_of(&link, QA), local_of(&link, QB));
    let file = format!("media/{}.webp", format!("{:064x}", 0x55u8));
    let sha = format!("{:064x}", 0x66u8);
    for id in [&a, &b] {
        bank.questions.iter_mut().find(|q| q.id == *id).unwrap().image = Some(ImageRef { file: file.clone(), width: 1, height: 1 });
    }
    link.picture_files.push((file.clone(), sha.clone()));
    // A is sent with the picture (its rebuilt record names the file); B is held back (its record is the base body's)
    let mut qa = v9()[0].clone();
    qa["image"] = json!({"file": file, "width": 1, "height": 1});
    let composed: Vec<super::partial_send::ComposedQuestion> = vec![
        super::partial_send::ComposedQuestion { local: a.clone(), qid: Some(QA.into()), question: Some(serde_json::from_value(qa).unwrap()), base_image: None },
        super::partial_send::ComposedQuestion { local: b.clone(), qid: Some(QB.into()), question: Some(serde_json::from_value(v9()[1].clone()).unwrap()), base_image: None },
        super::partial_send::ComposedQuestion { local: local_of(&link, QC), qid: Some(QC.into()), question: Some(serde_json::from_value(v9()[2].clone()).unwrap()), base_image: None },
    ];
    let sent = super::circles_api::SentBank { circle_id: CID.into(), bank_id: BID.into(), version: 10, version_id: format!("{:032x}", 10), qids: vec![QA.into(), QB.into(), QC.into()], ..Default::default() };
    let after = super::circle_links::after_partial_send(&link, &bank, &composed, &sent, vec![b.clone()]);
    assert!(
        after.picture_files.iter().any(|(f, s)| *f == file && *s == sha),
        "the send dropped the name of a file the held-back question still uses: {:?}",
        after.picture_files
    );
    assert_eq!(picture_sha_on_circle(&after, bank.questions.iter().find(|q| q.id == b).unwrap()).as_deref(), Some(sha.as_str()), "the held-back question lost the circle's name for its picture");
}

#[test]
fn d2p_a_known_no_figure_record_allows_a_restore_that_lands_on_the_base() {
    // (round 10, critic I2) The base version has NO figure, and this build recorded that (`figure: None, figure_recorded:
    // true`). The person added a figure locally. Version 6's question is the base's exactly: no figure, same (no) picture,
    // same words. Restoring lands the copy on the base -- yet round 9 refused it as "not countable", because `None` also
    // meant "not recorded".
    let (mut bank, mut link) = linked();
    let a = local_of(&link, QA);
    let base = link.base.iter().find(|b| b.local_id == a).unwrap();
    assert!(base.figure.is_none() && base.figure_recorded, "the base is a record that has no figure");
    bank.questions.iter_mut().find(|q| q.id == a).unwrap().figure = Some(crate::model::FigureSpec {
        id: "triangle".into(),
        params: [("a".to_string(), json!(3))].into_iter().collect(),
    });
    bank.modified += 10;
    let six = body(6, &v9(), false);
    let mut banks = vec![bank];
    let sink = LogRestore { log: Arc::default(), copy: Mutex::new(None) };
    // the other direction: the same restore over a link that does NOT record the base's figure is still unsafe, and refused
    let mut unsure = link.clone();
    unsure.base[0].figure_recorded = false;
    let mut other = banks.clone();
    let none = restore_question(&mut other, &ask(&unsure, Some(&a), QA, &six), &sink, &|_| Ok(()), NOW + 100_000);
    assert_eq!(none.map(|d| d.version), Err(RestoreRefused::FigureNotRecorded), "a link with no figure record let a figure vanish");
    let r = restore_question(&mut banks, &ask(&link, Some(&a), QA, &six), &sink, &|_| Ok(()), NOW + 100_000);
    assert_eq!(r.map(|d| d.version), Ok(6), "a restore that puts back exactly the base (no figure, no picture, same words) was refused");
    assert!(banks[0].questions.iter().find(|q| q.id == a).unwrap().figure.is_none(), "the copy is the base version's: no figure");
}

#[test]
fn d2o_a_stale_picture_record_must_not_override_the_base_record() {
    // picture_files is consulted FIRST. After a merge that rebuilt an accurate base record (local_image F, snap.image
    // S_new) the stale entry F -> S_old still wins, so Compare reads the copy's picture as S_old.
    let (mut bank, mut link) = linked();
    let a = local_of(&link, QA);
    let file = format!("media/{}.webp", format!("{:064x}", 0x11u8));
    let s_new = format!("{:064x}", 0x11u8);
    let s_old = format!("{:064x}", 0x22u8);
    {
        let b = link.base.iter_mut().find(|b| b.local_id == a).unwrap();
        b.local_image = Some(file.clone());
        b.snap.image = Some(s_new.clone());
    }
    bank.questions.iter_mut().find(|q| q.id == a).unwrap().image = Some(ImageRef { file: file.clone(), width: 1, height: 1 });
    link.picture_files.push((file.clone(), s_old.clone()));
    assert_eq!(
        picture_sha_on_circle(&link, &bank.questions[0]).as_deref(),
        Some(s_new.as_str()),
        "the stale link-level map overrides the accurate per-question base record"
    );
}

#[test]
fn d2o_a_tampered_picture_record_must_not_make_a_different_picture_the_same() {
    let (mut bank, mut link) = linked();
    let a = local_of(&link, QA);
    let actual = format!("{:064x}", 0x11u8);
    let lie = format!("{:064x}", 0x22u8);
    let file = format!("media/{actual}.webp");
    {
        let b = link.base.iter_mut().find(|b| b.local_id == a).unwrap();
        b.local_image = Some(file.clone());
        b.snap.image = Some(actual.clone());
    }
    bank.questions.iter_mut().find(|q| q.id == a).unwrap().image = Some(ImageRef { file: file.clone(), width: 1, height: 1 });
    link.picture_files.push((file.clone(), lie.clone()));
    let mut q = mc(QA, "Question A as of version 9", 1.0);
    q["image"] = json!({"file": format!("media/{lie}.png"), "width": 1, "height": 1});
    let six = body(6, &[q, mc(QB, "Question B as of version 8", 1.0), mc(QC, "Question C", 1.0)], false);
    let cmp = compare(side_of_copy(&link, &bank, &a).as_ref(), side_of_version(&six, QA).as_ref(), false);
    let pic = cmp.fields.iter().find(|f| f.field == "Picture").unwrap();
    assert!(pic.changed, "a different local picture is read as the circle's own: {} / {}", pic.left, pic.right);
}

#[test]
fn d2o_a_link_with_one_unreadable_figure_still_parses() {
    // picture_now and snap.difficulty read leniently ("the link is kept, never dropped for it"); figure does not.
    let (_, link) = linked();
    let mut v = serde_json::to_value(&link).unwrap();
    v["base"][0]["figure"] = json!("triangle");
    assert!(serde_json::from_value::<CircleLink>(v).is_ok(), "one unreadable figure drops the whole link");
}

// ---- round 11 (security critic 9): the merge's figure record, picture names when the base is dropped, and lenient link fields ----

fn with_figure(qid: &str, a: i64) -> crate::model::Question {
    let mut q = mc(qid, "One", 1.0);
    q["figure"] = json!({ "id": "triangle", "params": { "a": a } });
    serde_json::from_value(q).expect("a figure question")
}

#[test]
fn d2p_r10_a_merge_records_the_new_bases_figure() {
    // `theirs_delta` does not compare figures (plan 13.9), so a circle change of only the figure arrives as
    // "only you changed this". The merge keeps our question but advances the base version: the record it writes
    // must still name the NEW base version's figure. Carrying the old base's figure makes the record lie, and the
    // restore guard then lets that figure back in (see the end-to-end probe in the security report).
    use super::merge::{self, Theirs};
    let qid = QA.to_string();
    let v9 = with_figure(&qid, 3);
    let base = base_of(&v9, &qid, "fp", None);
    let mut ours = v9.clone();
    ours.stem = "One (mine)".into();
    let mut bank = QuestionBank { id: LOCAL.into(), name: "B".into(), questions: vec![ours], created: 0, modified: 5, details: Default::default() };
    let v10 = with_figure(&qid, 4);
    let theirs = vec![Theirs { index: 0, qid: qid.clone(), fp: "fp".into(), sha: None, q: v10.clone() }];
    let plan = merge::build_plan("item", std::slice::from_ref(&base), &bank, &theirs, 9, 10, None, String::new());
    assert_eq!(plan.rows.len(), 1);
    assert_eq!(plan.rows[0].kind, merge::RowKind::OursChanged, "the pool's figure-only change is invisible to the merge");
    let (_, new_base) = merge::apply_merge(&mut bank, &plan, &theirs, &[base], "a", 10_000).expect("apply");
    assert_eq!(new_base.len(), 1);
    assert_eq!(new_base[0].figure, v10.figure, "the record kept the old base's figure, not the new base version's");
    assert!(new_base[0].figure_recorded, "the record says its figure is the base version's");
}

#[test]
fn d2p_r10_a_merge_marks_an_old_links_figure_record_as_recorded() {
    // A link written before round 10 has `figure_recorded: false`. Get latest with every row Unchanged writes the pool's
    // snapshot and figure into the record; leaving the flag false made the restore guard refuse a restore that lands
    // exactly on the base, with a false "the figure is not the base's" explanation.
    use super::merge::{self, Theirs};
    let qid = QA.to_string();
    let v9 = with_figure(&qid, 3);
    let mut base = base_of(&v9, &qid, "fp", None);
    base.figure_recorded = false; // an old link: no record of the base's figure
    let mut bank = QuestionBank { id: LOCAL.into(), name: "B".into(), questions: vec![v9.clone()], created: 0, modified: 5, details: Default::default() };
    let v10 = with_figure(&qid, 4);
    let theirs = vec![Theirs { index: 0, qid: qid.clone(), fp: "fp".into(), sha: None, q: v10.clone() }];
    let plan = merge::build_plan("item", std::slice::from_ref(&base), &bank, &theirs, 9, 10, None, String::new());
    assert_eq!(plan.rows.len(), 1);
    assert_eq!(plan.rows[0].kind, merge::RowKind::Unchanged);
    let (_, new_base) = merge::apply_merge(&mut bank, &plan, &theirs, &[base], "a", 10_000).expect("apply");
    assert_eq!(new_base[0].figure, v10.figure);
    assert!(new_base[0].figure_recorded, "a merge that wrote the pool's figure left the old link's flag false");
}

#[test]
fn d2p_r10_a_whole_bank_send_without_a_kept_base_drops_stale_picture_names() {
    // A whole-bank send carried every question as itself. When the answer's qids cannot be matched to the bank the
    // base is dropped; `picture_files_kept` then used to keep every name for a file the bank still has, and the stale
    // name (the circle's older name for that picture) overrode the name the send itself gave the file. Compare then
    // said the picture the send had just made was "a different picture".
    let (mut bank, mut link) = linked();
    let a = local_of(&link, QA);
    let own = format!("{:064x}", 0xaau8);
    let old = format!("{:064x}", 0xbbu8);
    let file = format!("media/{own}.webp");
    bank.questions.iter_mut().find(|q| q.id == a).unwrap().image = Some(ImageRef { file: file.clone(), width: 1, height: 1 });
    link.picture_files.push((file.clone(), old.clone()));
    let sent = super::circles_api::SentBank { circle_id: CID.into(), bank_id: BID.into(), version: 10, version_id: format!("{:032x}", 10), qids: Vec::new(), ..Default::default() };
    let after = super::circle_links::after_send(&link, &bank, &sent);
    assert!(after.base.is_empty(), "the answer's qids cannot be matched: the base is dropped");
    assert!(after.picture_files.is_empty(), "a stale name survived a whole-bank send: {:?}", after.picture_files);
    let q = bank.questions.iter().find(|q| q.id == a).unwrap();
    assert_eq!(picture_sha_on_circle(&after, q).as_deref(), Some(own.as_str()), "Compare would call the just-sent picture a different picture");
}

#[test]
fn d2p_r10_a_partial_send_without_a_kept_base_keeps_only_the_held_back_name() {
    // The partial send's base is bounded away for a bank over `BASE_MAX_QUESTIONS`. The keep must then still be
    // judged per question from the COMPOSED questions, not guessed from the empty stored base: only a question
    // held back may keep a link-level picture name, never one the send carried as this bank's own.
    use super::partial_send::ComposedQuestion;
    let n = super::merge::BASE_MAX_QUESTIONS + 1;
    let held_local = "local-held".to_string();
    let held_file = format!("media/{}.webp", format!("{:064x}", 0xc1u8));
    let held_sha = format!("{:064x}", 0xd1u8);
    // the file of the first question the send CARRIES as this bank's own (i = 1 below)
    let sent_file = format!("media/{}.webp", format!("{:064x}", 0x1001u64));
    let mut questions = Vec::new();
    let mut composed = Vec::new();
    let mut qids = Vec::new();
    for i in 0..n {
        let qid = format!("{:032x}", i + 1);
        let local = if i == 0 { held_local.clone() } else { format!("local-{i}") };
        let file = if i == 0 { held_file.clone() } else { format!("media/{}.webp", format!("{:064x}", 0x1000u64 + i as u64)) };
        let mut q = with_figure(&qid, 1);
        q.id = local.clone();
        q.image = Some(ImageRef { file: file.clone(), width: 1, height: 1 });
        // the held-back question is sent as the base body has it (the base body has no picture of its own here)
        let in_payload = if i == 0 { let mut b = q.clone(); b.image = None; b } else { q.clone() };
        composed.push(ComposedQuestion {
            local: local.clone(),
            qid: Some(qid.clone()),
            question: Some(in_payload),
            base_image: (i == 0).then(|| "media/base.webp".to_string()),
        });
        qids.push(qid);
        questions.push(q);
    }
    let bank = QuestionBank { id: LOCAL.into(), name: "B".into(), questions, created: 0, modified: 5, details: Default::default() };
    let head = BankHead { id: BID.into(), title: "B".into(), license: "CC-BY-4.0".into(), version: 9, version_id: format!("{:032x}", 9), ..BankHead::default() };
    let base: Vec<_> = bank.questions.iter().zip(&qids).map(|(q, id)| base_of(q, id, "fp", None)).collect();
    let mut link = CircleLink::from_head(CID, "C", &head, LOCAL, base, NOW);
    link.picture_files.push((held_file.clone(), held_sha.clone()));
    link.picture_files.push((sent_file.clone(), format!("{:064x}", 0xffu8)));
    let sent = super::circles_api::SentBank { circle_id: CID.into(), bank_id: BID.into(), version: 10, version_id: format!("{:032x}", 10), qids, ..Default::default() };
    let after = super::circle_links::after_partial_send(&link, &bank, &composed, &sent, vec![held_local.clone()]);
    assert!(after.base.is_empty(), "the composed base is dropped for size");
    assert_eq!(after.picture_files, vec![(held_file, held_sha)], "only the held-back question's name may survive: {:?}", after.picture_files);
}

#[test]
fn d2p_r10_a_held_back_reencoded_picture_keeps_this_stores_file() {
    // (round 12, product critic 10 P1) A partial send rebuilds a held-back question's base record from the base body's
    // RAW question, whose image.file is the circle's own name (here a PNG). This store keeps the same picture as WebP
    // under its own name, which the old base record named. The rebuilt record must keep that local file: otherwise
    // ours_delta sees a picture change that never happened, the strip counts an unsent change, and Compare says
    // "a different picture" against the very base version whose picture did not change.
    use super::partial_send::ComposedQuestion;
    let (mut bank, mut link) = linked();
    let a = local_of(&link, QA);
    let local_file = format!("media/{}.webp", format!("{:064x}", 0xb2u8));
    let circle_file = format!("media/{}.png", format!("{:064x}", 0xa1u8));
    let circle_sha = format!("{:064x}", 0xa1u8);
    {
        let q = bank.questions.iter_mut().find(|q| q.id == a).unwrap();
        q.image = Some(ImageRef { file: local_file.clone(), width: 1, height: 1 });
        // a held-back figure-only edit: the snapshot cannot count it (plan 13.9), so it must not show as a change
        q.figure = Some(crate::model::FigureSpec { id: "triangle".into(), params: [("a".to_string(), json!(3))].into_iter().collect() });
    }
    {
        let b = link.base.iter_mut().find(|b| b.local_id == a).unwrap();
        b.local_image = Some(local_file.clone());
        b.snap.image = Some(circle_sha.clone());
        b.picture_now = None;
    }
    let mut qa = v9()[0].clone();
    qa["image"] = json!({ "file": circle_file, "width": 1, "height": 1 });
    let composed: Vec<ComposedQuestion> = vec![
        ComposedQuestion { local: a.clone(), qid: Some(QA.into()), question: Some(serde_json::from_value(qa).unwrap()), base_image: Some(circle_file.clone()) },
        ComposedQuestion { local: local_of(&link, QB), qid: Some(QB.into()), question: Some(serde_json::from_value(v9()[1].clone()).unwrap()), base_image: None },
        ComposedQuestion { local: local_of(&link, QC), qid: Some(QC.into()), question: Some(serde_json::from_value(v9()[2].clone()).unwrap()), base_image: None },
    ];
    let sent = super::circles_api::SentBank { circle_id: CID.into(), bank_id: BID.into(), version: 10, version_id: format!("{:032x}", 10), qids: vec![QA.into(), QB.into(), QC.into()], ..Default::default() };
    let after = super::circle_links::after_partial_send(&link, &bank, &composed, &sent, vec![a.clone()]);
    let b = after.base.iter().find(|b| b.local_id == a).expect("the held-back question keeps a record");
    assert_eq!(b.local_image.as_deref(), Some(local_file.as_str()), "the rebuilt record names the circle's PNG, not this store's WebP");
    assert_eq!(b.snap.image.as_deref(), Some(circle_sha.as_str()), "the base version's circle name is unchanged");
    assert_eq!(changes_since(&after, &bank).changed, 0, "the copy is reported as an unsent picture change that never happened");
    let q = bank.questions.iter().find(|q| q.id == a).unwrap();
    assert_eq!(picture_sha_on_circle(&after, q).as_deref(), Some(circle_sha.as_str()), "Compare would call the unchanged picture a different one");
}

#[test]
fn d2p_r10_unreadable_viewers_form_and_role_still_keep_the_link() {
    // One unreadable field must not drop the whole link (the round-9 M3 rule). `viewers_form` is a safety flag: an
    // unreadable value is the never-sendable true, never serde's false default. `role_seen`/`role_seen_at` read none.
    let (_, link) = linked();
    let mut v = serde_json::to_value(&link).unwrap();
    v["viewers_form"] = json!("yes");
    v["role_seen"] = json!(5);
    v["role_seen_at"] = json!("yesterday");
    let back: CircleLink = serde_json::from_value(v).expect("one unreadable field must not drop the link");
    assert!(back.viewers_form, "an unreadable viewers_form must read as never-sendable");
    assert_eq!(back.role_seen, None);
    assert_eq!(back.role_seen_at, None);
    // (round 12, product critic 10 P3) an unreadable `seen` keeps the link but must read the SAFE hiding state; a
    // genuinely missing field still means "nothing known".
    let mut v = serde_json::to_value(&link).unwrap();
    v["seen"] = json!("junk");
    let back: CircleLink = serde_json::from_value(v).expect("an unreadable seen must not drop the link");
    assert!(back.seen.hide_key, "an unreadable seen must read as hiding the keys");
    let mut v = serde_json::to_value(&link).unwrap();
    v.as_object_mut().unwrap().remove("seen");
    let back: CircleLink = serde_json::from_value(v).expect("a missing seen is the default");
    assert!(!back.seen.hide_key, "a missing seen means nothing known");
}
