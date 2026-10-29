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
    timeline(&TimelineIn { lines: &lines, changes: page, link, bank, local_id: &local, show_all })
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
    assert_eq!(reads_needed(&lines(), &mut cache, CID, BID, &[Side::Version(7), Side::Version(9)]), Err(NotReadable::Withheld(7)));
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
    assert_eq!(reads_needed(&lines(), &mut cache, CID, BID, &[Side::Version(2)]), Err(NotReadable::NotKept(2)));
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
    assert_eq!(c.same_fields(), vec!["Choices", "Answer key", "Difficulty", "Topics", "Explanation", "Picture"]);
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
    let mine = side_of_copy(&bank, &local_of(&link, QA));
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
    let need = reads_needed(&ls, &mut cache, CID, BID, &[Side::Version(6), Side::Version(9)]).unwrap();
    assert_eq!(need, vec![6, 9]);
    for v in need {
        let b = block_on(read_body(&pool, &mut cache, CID, BID, v, NOW)).unwrap();
        assert_eq!(b.version, v);
        assert!(b.question(QA).is_some());
    }
    assert_eq!(s.calls("/versions/"), 2);
    // Restore from the version just compared: no read
    assert_eq!(reads_needed(&ls, &mut cache, CID, BID, &[Side::Version(6)]).unwrap(), Vec::<u64>::new());
    let again = block_on(read_body(&pool, &mut cache, CID, BID, 6, NOW)).unwrap();
    assert_eq!(again.version, 6);
    assert_eq!(s.calls("/versions/"), 2);
    // your copy is never a read; restoring from another version is one
    assert_eq!(reads_needed(&ls, &mut cache, CID, BID, &[Side::YourCopy, Side::Version(3)]).unwrap(), vec![3]);
    block_on(read_body(&pool, &mut cache, CID, BID, 3, NOW)).unwrap();
    assert_eq!(s.calls("/versions/"), 3);
}

#[test]
fn d2_the_cache_keeps_four_bodies_and_drops_them_for_another_account() {
    let mut cache = HistoryCache::default();
    cache.note_account(Some("acct-1"));
    for v in 1..=5 {
        cache.put_body(CID, BID, body(v, &v9(), false));
    }
    assert_eq!(cache.bodies(), BODY_CACHE_MAX);
    assert_eq!(BODY_CACHE_MAX, 4);
    assert!(cache.body(CID, BID, 1).is_none(), "the oldest went first");
    assert!(cache.body(CID, BID, 2).is_some());
    // a body looked at is kept over the others
    cache.put_body(CID, BID, body(6, &v9(), false));
    assert!(cache.body(CID, BID, 2).is_some() && cache.body(CID, BID, 3).is_none());
    // the same account again keeps them; another account (or none) drops them
    cache.note_account(Some("acct-1"));
    assert_eq!(cache.bodies(), 4);
    cache.note_account(Some("acct-2"));
    assert_eq!(cache.bodies(), 0);
    cache.put_body(CID, BID, body(9, &v9(), false));
    cache.note_account(None);
    assert_eq!(cache.bodies(), 0);
    cache.put_body(CID, BID, body(9, &v9(), false));
    cache.forget();
    assert_eq!(cache.bodies(), 0);
    // another bank's version is another body
    cache.put_body(CID, BID, body(9, &v9(), false));
    assert!(cache.body(CID, "0000000000000000000000000000000f", 9).is_none());
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
    RestoreAsk { link, local_id: local, qid, body, picture: None, removed_in_circle: false }
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
