//! Working copies against a scripted session (no network, no egui window): every route and the errors
//! it can meet, the send / conflict / merge / send state machine, the commit id kept across a retry,
//! one change at a time, a sign-in that ends in the middle, texts the pool sends as `null`, and
//! headless smoke frames of every screen wide and narrow.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use serde_json::{Value, json};

use super::api::ApiError;
use super::browse::RestoreSink;
use super::circle_edit::Sent as SentInfo;
use super::circle_edit::*;
use super::circles_tests::Sent;
use super::circles::{CirclesModel, Screen};
use super::circle_links::*;
use super::circles_api::*;
use super::circles_tests::*;
use super::merge::{Choice, RowKind, base_of};
use super::model::Runner;
use super::pool::*;
use super::publish::{AccountSnapshot, SharedMedia};
use super::transport::Method;
use crate::model::QuestionBank;

const LOCAL: &str = "local-bank-1";
const QA: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa1";
const QB: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa2";
const QC: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa3";

fn hex(v: u64) -> String {
    format!("{v:032x}")
}

fn bank_path() -> String {
    format!("/circles/{CID}/banks/{BID}")
}

#[derive(Default)]
struct MemRestore(Mutex<usize>);

impl RestoreSink for MemRestore {
    fn save(&self, banks: &[QuestionBank]) -> Result<String, String> {
        *self.0.lock().unwrap() += 1;
        Ok(format!("memory ({} banks)", banks.len()))
    }
}

struct BadRestore;

impl RestoreSink for BadRestore {
    fn save(&self, _: &[QuestionBank]) -> Result<String, String> {
        Err("the disk is full".into())
    }
}

fn qjson(id: &str, stem: &str) -> Value {
    json!({"id": id, "stem": stem, "kind": {"Mc": {"answers": [{"text": "A", "correct": true}, {"text": "B", "correct": false}]}}, "points": 1.0, "difficulty": null, "created": 5, "modified": 6})
}

/// The payload of a circle bank: the served question `id` is the circle's id for it.
fn payload_of_items(items: &[(&str, &str)]) -> Vec<u8> {
    let qs: Vec<Value> = items.iter().map(|(qid, stem)| qjson(qid, stem)).collect();
    json!({"schema_version": 2, "bank": {"id": BID, "name": "Cells", "questions": qs, "created": 1, "modified": 2}}).to_string().into_bytes()
}

/// A bank as the pool serves it (head or a kept version).
fn served(version: u64, items: &[(&str, &str)]) -> Vec<u8> {
    let head = json!({"circle": {"id": CID, "name": "Biology team"}, "bank": {"id": BID, "title": "Cells", "subject": "Biology", "course": "", "grade": "", "license": "CC-BY-4.0", "version_license": "CC-BY-4.0", "author_name": "Mr Keesler", "is_author": false, "version": version, "version_id": hex(version), "question_count": items.len(), "has_images": false, "committed_at": NOW - DAY}, "origins": null});
    let h = head.to_string();
    let p = String::from_utf8(payload_of_items(items)).unwrap();
    format!("{},\"payload\":{}}}", &h[..h.len() - 1], p).into_bytes()
}

fn editor() -> Perm {
    Perm { role: Role::Editor, is_author: false, closed: false }
}

fn owner() -> Perm {
    Perm { role: Role::Owner, is_author: false, closed: false }
}

fn viewer() -> Perm {
    Perm { role: Role::Viewer, is_author: false, closed: false }
}

/// A working copy made from version 1 with these questions: the bank and its link.
fn linked(items: &[(&str, &str)]) -> (QuestionBank, CircleLink) {
    let theirs = circle_theirs(&payload_of_items(items), NOW);
    let bank = QuestionBank { id: LOCAL.into(), name: "Cells".into(), questions: theirs.iter().map(|t| t.q.clone()).collect(), created: NOW / 1000, modified: NOW / 1000, details: Default::default() };
    let head = BankHead { id: BID.into(), title: "Cells".into(), subject: "Biology".into(), license: "CC-BY-4.0".into(), version_license: "CC-BY-4.0".into(), version: 1, version_id: hex(1), committed_at: NOW - DAY, ..BankHead::default() };
    let base = theirs.iter().map(|t| base_of(&t.q, &t.qid, &t.fp, t.sha.clone())).collect();
    (bank, CircleLink::from_head(CID, "Biology team", &head, LOCAL, base, NOW))
}

fn heads_json(license: &str, version: u64) -> Value {
    json!({"banks": [{"bank_id": BID, "status": "current", "version": version, "version_id": hex(version), "license": license, "at": NOW - 1000, "by": "Ms Frizzle"}]})
}

struct Rig {
    m: CircleEditModel,
    s: Arc<Script>,
    log: LinkLog,
    restores: Arc<MemRestore>,
    banks: Vec<QuestionBank>,
}

impl Rig {
    fn new(items: &[(&str, &str)]) -> Rig {
        let s = Script::new();
        let log = LinkLog::memory();
        let restores = Arc::new(MemRestore::default());
        let media: SharedMedia = Arc::new(HashMap::<String, Vec<u8>>::new());
        s.keep("Post", "/circles/heads", 200, heads_json("CC-BY-4.0", 1));
        let mut m = CircleEditModel::new(Runner::Inline, Arc::new(Mem::default()), media, restores.clone(), log.clone()).with_retry(RetryPolicy::none());
        m.set_clock(NOW);
        let (bank, link) = linked(items);
        m.add_link(link).unwrap();
        let mut r = Rig { m, s, log, restores, banks: vec![bank] };
        r.frame();
        r
    }

    fn frame(&mut self) {
        self.m.frame(now(), &ready(&self.s));
    }

    /// Opens the send page, lets the licence check come back, and ticks the own-work box.
    fn open_send(&mut self, perm: Perm) {
        self.m.open_send(LOCAL, perm);
        self.frame();
        self.m.set_attest(true);
    }

    /// One turn of the app: the model's frame, then sync with the banks.
    fn turn(&mut self) -> bool {
        self.frame();
        let c = self.m.sync(&mut self.banks);
        self.frame();
        c
    }

    fn turns(&mut self, n: usize) {
        for _ in 0..n {
            self.turn();
        }
    }

    fn link(&mut self) -> CircleLink {
        self.m.link_for_local(LOCAL).unwrap()
    }

    fn puts(&self) -> Vec<Sent> {
        self.s.seen().into_iter().filter(|x| x.method == Method::Put).collect()
    }

    fn edit_stem(&mut self, qid_index: usize, stem: &str) {
        self.banks[0].questions[qid_index].stem = stem.into();
        self.banks[0].modified += 1;
    }

    fn send_stage(&self) -> &SendStage {
        match &self.m.view {
            EditView::Send(v) => &v.stage,
            other => panic!("not on the send page: {other:?}"),
        }
    }

    fn merge_phase(&self) -> &MergePhase {
        match &self.m.view {
            EditView::Merge(v) => &v.phase,
            other => panic!("not on the merge page: {other:?}"),
        }
    }

    fn history(&self) -> &HistoryView {
        match &self.m.view {
            EditView::History(h) => h,
            other => panic!("not on the history page: {other:?}"),
        }
    }
}

/// The meta line and payload of a send body.
fn body_parts(x: &Sent) -> (Value, Value) {
    let text = String::from_utf8(x.body.clone()).unwrap();
    let (meta, payload) = text.split_once('\n').unwrap();
    (serde_json::from_str(meta).unwrap(), serde_json::from_str(payload).unwrap())
}

fn sent_ok(version: u64, qids: &[&str]) -> Value {
    json!({"circle_id": CID, "bank_id": BID, "version": version, "version_id": hex(version), "qids": qids, "added": 0, "changed": 1, "removed": 0})
}

fn page(head: u64, lines: &[u64], next: Option<u64>) -> Value {
    let ls: Vec<Value> = lines
        .iter()
        .map(|v| json!({"version": v, "version_id": hex(*v), "by": "Mr Keesler", "at": NOW - DAY * (*v as i64), "message": format!("note {v}"), "added": 0, "changed": 1, "removed": 0, "question_count": 2, "license": "CC-BY-4.0", "kept": true, "withheld": false}))
        .collect();
    json!({"bank_id": BID, "head_version": head, "oldest_kept": 1, "pinned_versions": [], "versions": ls, "next_before": next})
}

// ---- gating ---------------------------------------------------------------------------------------------------------

#[test]
fn a_closed_model_does_no_io() {
    let mut r = Rig::new(&[(QA, "One"), (QB, "Two")]);
    for _ in 0..3 {
        r.turn();
    }
    assert!(!r.m.active() && !r.m.busy());
    assert!(r.s.seen().is_empty());
}

// ---- links ----------------------------------------------------------------------------------------------------------

#[test]
fn a_link_is_kept_in_its_own_file_and_survives_a_restart() {
    let r = Rig::new(&[(QA, "One"), (QB, "Two")]);
    let again = r.log.load();
    assert_eq!(again.len(), 1);
    assert_eq!(again[0].circle_bank_id, BID);
    assert_eq!(again[0].qids.len(), 2);
    assert_eq!(again[0].qids[0].qid, QA);
    assert_eq!(again[0].base.len(), 2);
    assert_eq!(LINKS_DOC, "share-circles.json");
}

#[test]
fn changes_are_counted_against_what_the_copy_was_in_step_with() {
    let mut r = Rig::new(&[(QA, "One"), (QB, "Two")]);
    r.turn();
    assert_eq!(r.m.status(LOCAL).unwrap().changes.total(), 0);
    r.edit_stem(0, "One, reworded");
    r.banks[0].questions.remove(1);
    r.banks[0].questions.push(serde_json::from_value(qjson("new-local", "Brand new")).unwrap());
    r.turn();
    let c = r.m.status(LOCAL).unwrap().changes;
    assert_eq!((c.changed, c.added, c.removed, c.known), (1, 1, 1, true));
}

#[test]
fn a_missing_local_bank_is_noticed_and_can_be_forgotten() {
    let mut r = Rig::new(&[(QA, "One")]);
    r.banks.clear();
    r.turn();
    assert!(r.m.status(LOCAL).unwrap().missing);
    r.m.forget(LOCAL);
    assert!(r.log.load().is_empty());
}

#[test]
fn the_quiet_check_marks_a_copy_that_is_behind() {
    let mut r = Rig::new(&[(QA, "One")]);
    assert_eq!(r.link().behind(), 0);
    r.m.apply_heads(&[HeadEntry { bank_id: BID.into(), state: HeadState::Current { version: 4, license: None, at: Some(NOW), by: Some("Ms Frizzle".into()), hide_key: None } }]);
    let l = r.link();
    assert_eq!(l.behind(), 3);
    assert_eq!(behind_line(l.behind()), "3 new versions in the circle");
    assert_eq!(behind_line(1), "1 new version in the circle");
    assert_eq!(r.log.load()[0].head_seen, 4);
}

// ---- send -----------------------------------------------------------------------------------------------------------

#[test]
fn sending_puts_the_changes_over_the_base_version_and_moves_the_link_on() {
    let mut r = Rig::new(&[(QA, "One"), (QB, "Two")]);
    r.edit_stem(0, "One, reworded");
    r.banks[0].questions.push(serde_json::from_value(qjson("new-local", "Brand new")).unwrap());
    r.open_send(editor());
    r.turn();
    let EditView::Send(v) = &r.m.view else { panic!() };
    assert_eq!(v.changes.map(|c| (c.changed, c.added, c.removed)), Some((1, 1, 0)));
    r.m.set_message("Reworded the first question");
    r.s.on("Put", &bank_path(), 201, sent_ok(2, &[QA, QB, QC]));
    r.m.send();
    r.turn();
    let puts = r.puts();
    assert_eq!(puts.len(), 1);
    let (meta, payload) = body_parts(&puts[0]);
    assert_eq!(meta["base_version"], 1);
    assert_eq!(meta["license"], "CC-BY-4.0");
    assert_eq!(meta["message"], "Reworded the first question");
    assert_eq!(meta["attest"]["own_work"], true);
    assert!(meta.get("merge_token").is_none() && meta.get("confirm_large_change").is_none());
    assert_eq!(meta["meta"]["title"], "Cells");
    let qs = payload["bank"]["questions"].as_array().unwrap();
    assert_eq!(qs.len(), 3);
    assert_eq!(qs[0]["published"]["qid"], QA, "a known question keeps its identity");
    assert_eq!(qs[1]["published"]["qid"], QB);
    assert!(qs[2].get("published").is_none(), "a new question is sent without one");
    assert_eq!(payload["schema_version"], 2);
    assert!(matches!(r.send_stage(), SendStage::Done(s) if s.version == 2 && s.changed == 1));
    let l = r.link();
    assert_eq!(l.base_version, 2);
    assert_eq!(l.base_version_id, hex(2));
    assert_eq!(l.qids.iter().map(|q| q.qid.as_str()).collect::<Vec<_>>(), vec![QA, QB, QC]);
    r.turn();
    assert_eq!(r.m.status(LOCAL).map(|s| s.changes.total()), Some(0), "nothing is left to send");
    assert_eq!(r.m.take_advanced(), vec![Advanced { circle_bank_id: BID.into(), local_bank_id: LOCAL.into(), version: 2 }]);
}

#[test]
fn a_viewer_can_not_send_and_nothing_goes_out() {
    let mut r = Rig::new(&[(QA, "One")]);
    r.open_send(viewer());
    assert!(r.m.send_blocker().is_some());
    r.m.send();
    r.turns(2);
    assert!(r.puts().is_empty());
    let mut c = owner();
    c.closed = true;
    r.m.open_send(LOCAL, c);
    assert!(r.m.send_blocker().is_some_and(|b| b.contains("closed")));
}

#[test]
fn a_lost_answer_is_retried_with_the_same_commit_id_and_a_changed_note_starts_over() {
    let mut r = Rig::new(&[(QA, "One")]);
    r.edit_stem(0, "One, reworded");
    r.open_send(editor());
    r.s.on("Post", "/circles/heads", 200, heads_json("CC-BY-4.0", 1));
    r.s.fail(ApiError::Offline("down".into()));
    r.m.send();
    r.turn();
    assert!(matches!(r.send_stage(), SendStage::Failed(p) if p.retry));
    r.s.on("Put", &bank_path(), 201, json!({"circle_id": CID, "bank_id": BID, "version": 2, "version_id": hex(2), "qids": [QA], "added": 0, "changed": 1, "removed": 0, "replayed": true}));
    r.m.retry_act();
    r.turn();
    let puts = r.puts();
    assert_eq!(puts.len(), 2);
    assert_eq!(body_parts(&puts[0]).0["commit_id"], body_parts(&puts[1]).0["commit_id"]);
    assert!(matches!(r.send_stage(), SendStage::Done(s) if s.replayed));
    let lines = sent_lines(&SentInfo { version: 2, added: 0, changed: 1, removed: 0, replayed: true, held: false, withheld: false, merged_from: None, left_out_unasked: Vec::new(), not_asked: Vec::new(), hiding_stopped: None, ask_next: false }, true);
    assert!(lines.iter().any(|l| l.contains("nothing was sent twice")));

    // Another try with a different note is another send.
    let mut r = Rig::new(&[(QA, "One")]);
    r.edit_stem(0, "One, reworded");
    r.open_send(editor());
    r.s.on("Post", "/circles/heads", 200, heads_json("CC-BY-4.0", 1));
    r.s.fail(ApiError::Offline("down".into()));
    r.m.send();
    r.turn();
    r.m.back_to_note();
    r.m.set_message("A note this time");
    r.s.on("Post", "/circles/heads", 200, heads_json("CC-BY-4.0", 1));
    r.s.fail(ApiError::Offline("down".into()));
    r.m.send();
    r.turn();
    let puts = r.puts();
    assert_ne!(body_parts(&puts[0]).0["commit_id"], body_parts(&puts[1]).0["commit_id"]);
}

#[test]
fn pressing_send_twice_sends_once() {
    let mut r = Rig::new(&[(QA, "One")]);
    r.edit_stem(0, "One, reworded");
    r.open_send(editor());
    r.s.on("Put", &bank_path(), 201, sent_ok(2, &[QA]));
    r.m.send();
    r.m.send();
    r.m.confirm_large_send();
    r.turn();
    assert_eq!(r.puts().len(), 1);
    r.m.send();
    r.turn();
    assert_eq!(r.puts().len(), 1, "a finished send is not sent again by a stray press");
}

#[test]
fn someone_else_sending_first_is_a_conflict_and_leads_to_merge() {
    let mut r = Rig::new(&[(QA, "One")]);
    r.edit_stem(0, "One, reworded");
    r.open_send(editor());
    r.s.on("Put", &bank_path(), 409, json!({"error": "conflict", "head_version": 3, "head_at": NOW - 1000, "head_by": "Ms Frizzle"}));
    r.m.send();
    r.turn();
    let SendStage::Conflict(c) = r.send_stage() else { panic!("{:?}", r.send_stage()) };
    assert_eq!(c.head_version, Some(3));
    assert!(conflict_line(c, NOW).contains("Ms Frizzle sent version 3"));
    assert_eq!(r.link().head_seen, 3, "the badge follows");
    assert_eq!(r.puts().len(), 1, "nothing else was sent");
    assert_eq!(r.banks[0].questions[0].stem, "One, reworded", "the bank is untouched");
}

#[test]
fn a_large_change_asks_first_and_then_goes_with_the_confirmation() {
    let mut r = Rig::new(&[(QA, "One"), (QB, "Two"), (QC, "Three")]);
    r.banks[0].questions.truncate(1);
    r.banks[0].modified += 1;
    r.open_send(editor());
    r.s.on("Put", &bank_path(), 428, json!({"error": "confirm_large_required", "removed": 2, "total": 3, "message": "raw words"}));
    r.m.send();
    r.turn();
    assert!(matches!(r.send_stage(), SendStage::Large { removed: 2, total: 3 }));
    assert_eq!(r.puts().len(), 1);
    r.m.cancel_large_send();
    assert!(matches!(r.send_stage(), SendStage::Editing));
    r.s.on("Put", &bank_path(), 428, json!({"error": "confirm_large_required", "removed": 2, "total": 3}));
    r.m.send();
    r.turn();
    assert!(matches!(r.send_stage(), SendStage::Large { .. }), "asked again");
    r.s.on("Put", &bank_path(), 201, sent_ok(2, &[QA]));
    r.m.confirm_large_send();
    r.turn();
    let puts = r.puts();
    let last = body_parts(puts.last().unwrap()).0;
    assert_eq!(last["confirm_large_change"], true);
    assert!(matches!(r.send_stage(), SendStage::Done(_)));
}

#[test]
fn only_the_author_changes_the_licence() {
    let mut r = Rig::new(&[(QA, "One")]);
    r.edit_stem(0, "One, reworded");
    r.open_send(editor());
    r.s.on("Put", &bank_path(), 422, json!({"error": "license_author_only", "license": "CC-BY-SA-4.0"}));
    r.m.send();
    r.turn();
    let SendStage::Refused { message, merge_again } = r.send_stage() else { panic!() };
    assert!(message.contains("author") && !*merge_again);
    assert_eq!(r.link().license, "CC-BY-SA-4.0", "the bank's own licence is used from now on");
    r.s.keep("Post", "/circles/heads", 200, heads_json("CC-BY-SA-4.0", 1));
    r.s.on("Put", &bank_path(), 201, sent_ok(2, &[QA]));
    r.m.send();
    r.turn();
    let puts = r.puts();
    assert_eq!(body_parts(&puts[1]).0["license"], "CC-BY-SA-4.0");
}

#[test]
fn held_and_withheld_results_are_said_in_plain_words() {
    let mut r = Rig::new(&[(QA, "One")]);
    r.edit_stem(0, "One, reworded");
    r.open_send(editor());
    r.s.on("Put", &bank_path(), 201, json!({"circle_id": CID, "bank_id": BID, "version": 2, "version_id": hex(2), "qids": [QA], "added": 0, "changed": 1, "removed": 0, "held": true, "withheld": true}));
    r.m.send();
    r.turn();
    let SendStage::Done(s) = r.send_stage() else { panic!() };
    assert!(s.held && s.withheld);
    let all = sent_lines(s, true).join(" ");
    // (phase C, round 44: the pool does not say why a version is held back; the words stay generic)
    assert!(all.contains("held back") && all.contains("administrator") && !all.contains("student"), "{all}");
    assert!(!all.contains("withheld:") && !all.contains("held:"));
}

#[test]
fn a_sign_in_that_ends_in_the_middle_resets_to_the_note_and_keeps_the_commit_id() {
    let mut r = Rig::new(&[(QA, "One")]);
    r.edit_stem(0, "One, reworded");
    r.open_send(editor());
    r.m.set_message("Keep this note");
    r.s.on("Post", "/circles/heads", 200, heads_json("CC-BY-4.0", 1));
    r.s.fail(ApiError::SignedOut);
    r.m.send();
    r.turn();
    assert!(r.m.take_signed_out());
    let EditView::Send(v) = &r.m.view else { panic!() };
    assert!(matches!(v.stage, SendStage::Editing));
    assert_eq!(v.message, "Keep this note", "nothing typed is lost");
    r.s.on("Put", &bank_path(), 201, sent_ok(2, &[QA]));
    r.m.send();
    r.turn();
    let puts = r.puts();
    assert_eq!(body_parts(&puts[0]).0["commit_id"], body_parts(&puts[1]).0["commit_id"]);
}

#[test]
fn a_bank_with_no_questions_is_not_sent() {
    let mut r = Rig::new(&[(QA, "One")]);
    r.banks[0].questions.clear();
    r.open_send(editor());
    r.m.send();
    r.turn();
    assert!(matches!(r.send_stage(), SendStage::Failed(p) if p.message.contains("at least one")));
    assert!(r.puts().is_empty());
}

#[test]
fn other_refusals_of_a_send_are_in_plain_words() {
    let cases: Vec<(u16, Value, &str)> = vec![
        (409, json!({"error": "publish_in_progress"}), "another"),
        (409, json!({"error": "bank_removed"}), "removed"),
        (403, json!({"error": "circle_role_required", "needed": "editor"}), "look at"),
        (409, json!({"error": "circle_closed"}), "closed"),
        (413, json!({"error": "quota_exceeded", "scope": "circle", "used": 90_000_000u64, "limit": 100_000_000u64}), "trim"),
        (429, json!({"error": "daily_limit", "metric": "circle_commits"}), "sent as many"),
        (400, json!({"error": "invalid_request", "field": "message"}), "500 characters"),
        (409, json!({"error": "commit_id_conflict"}), "start it again"),
    ];
    for (status, body, word) in cases {
        let mut r = Rig::new(&[(QA, "One")]);
        r.edit_stem(0, "One, reworded");
        r.open_send(editor());
        r.s.on("Put", &bank_path(), status, body.clone());
        r.m.send();
        r.turn();
        let msg = match r.send_stage() {
            SendStage::Failed(p) => p.message.clone(),
            other => panic!("{body}: {other:?}"),
        };
        assert!(msg.to_lowercase().contains(word), "{body}: {msg}");
        let code = body["error"].as_str().unwrap();
        assert!(!msg.contains(code), "the raw code leaked: {msg}");
        assert!(!msg.contains('_'), "{msg}");
    }
}

#[test]
fn a_merge_token_the_pool_refuses_says_to_merge_again() {
    let mut r = Rig::new(&[(QA, "One")]);
    r.edit_stem(0, "One, reworded");
    r.open_send(editor());
    r.s.on("Put", &bank_path(), 422, json!({"error": "merge_token_invalid", "message": "raw"}));
    r.m.send();
    r.turn();
    assert!(matches!(r.send_stage(), SendStage::Refused { merge_again: true, message } if message.contains("out of date")));
}

// ---- get latest and merge ---------------------------------------------------------------------------------------

fn merge_start_json(base: u64, head: u64, base_kept: bool) -> Value {
    json!({"bank_id": BID, "base_version": base, "base_kept": base_kept, "oldest_kept": 1, "head_version": head, "head_version_id": hex(head), "head_at": NOW - 1000, "head_by": "Ms Frizzle",
        "between": [{"version": head, "version_id": hex(head), "by": "Ms Frizzle", "at": NOW - 1000, "message": "Fixed the second question", "added": 1, "changed": 1, "removed": 0, "question_count": 3, "license": "CC-BY-4.0", "kept": true, "withheld": false}],
        "between_total": head - base, "merge_token": "tok.en", "merge_token_expires_at": NOW + 1_800_000})
}

/// Version 2 of the circle: the second question changed, a third added.
fn v2() -> Vec<(&'static str, &'static str)> {
    vec![(QA, "One"), (QB, "Two, fixed"), (QC, "Three")]
}

fn script_merge(r: &Rig, head_items: &[(&str, &str)], head: u64) {
    r.s.on("Post", &format!("{}/merge", bank_path()), 200, merge_start_json(1, head, true));
    r.s.keep_raw("Get", &bank_path(), served(head, head_items));
}

#[test]
fn get_latest_and_merge_then_send_goes_with_the_token() {
    let mut r = Rig::new(&[(QA, "One"), (QB, "Two")]);
    r.edit_stem(0, "One, reworded by me");
    script_merge(&r, &v2(), 2);
    r.m.open_merge(LOCAL, editor());
    r.turns(2);
    assert_eq!(r.s.last().method, Method::Get, "the newest version was read after merge-initiate");
    let mi = r.s.seen().into_iter().find(|x| x.path.ends_with("/merge")).unwrap();
    assert_eq!(mi.json(), json!({"base_version": 1}));
    let MergePhase::Review(rev) = r.merge_phase() else { panic!("{:?}", r.merge_phase()) };
    assert!(!rev.fallback);
    assert_eq!(rev.plan.count(RowKind::OursChanged), 1);
    assert_eq!(rev.plan.count(RowKind::TheirsChanged), 1);
    assert_eq!(rev.plan.count(RowKind::NewInTheirs), 1);
    assert_eq!(rev.start.merge_token, "tok.en");
    assert!(r.m.can_apply_merge());
    assert_eq!(r.banks[0].questions[0].stem, "One, reworded by me", "nothing changes until the person applies");
    assert_eq!(*r.restores.0.lock().unwrap(), 0);

    r.m.apply_merge();
    r.turns(2);
    assert!(matches!(r.merge_phase(), MergePhase::Applied(_)), "{:?}", r.merge_phase());
    assert_eq!(*r.restores.0.lock().unwrap(), 1, "the restore point was saved first");
    let b = &r.banks[0];
    assert_eq!(b.questions.len(), 3);
    assert_eq!(b.questions[0].stem, "One, reworded by me", "mine is kept");
    assert_eq!(b.questions[1].stem, "Two, fixed", "theirs is taken");
    let l = r.link();
    assert_eq!(l.base_version, 2);
    assert_eq!(l.base_version_id, hex(2));
    assert_eq!(l.qids.len(), 3);
    let MergePhase::Applied(done) = r.merge_phase() else { panic!() };
    assert_eq!(done.changes.total(), 1, "my reworded question is still to send");
    assert!(done.restore.contains("memory"));

    r.open_send(editor());
    r.turn();
    r.s.on("Put", &bank_path(), 201, sent_ok(3, &[QA, QB, QC]));
    r.m.send();
    r.turn();
    let puts = r.puts();
    let (meta, _) = body_parts(&puts[0]);
    assert_eq!(meta["base_version"], 2);
    assert_eq!(meta["merge_token"], "tok.en");
    assert_eq!(meta["merged_from"], 1);
    assert!(matches!(r.send_stage(), SendStage::Done(_)));
}

#[test]
fn a_copy_with_no_changes_of_its_own_just_catches_up() {
    let mut r = Rig::new(&[(QA, "One"), (QB, "Two")]);
    script_merge(&r, &v2(), 2);
    r.m.open_merge(LOCAL, editor());
    r.turns(2);
    r.m.apply_merge();
    r.turns(2);
    let MergePhase::Applied(done) = r.merge_phase() else { panic!("{:?}", r.merge_phase()) };
    assert_eq!(done.changes.total(), 0, "nothing to send");
    assert_eq!(r.banks[0].questions.len(), 3);
    assert_eq!(r.link().base_version, 2);
}

#[test]
fn a_question_both_sides_changed_needs_a_decision_before_apply() {
    let mut r = Rig::new(&[(QA, "One"), (QB, "Two")]);
    r.edit_stem(1, "Two, my way");
    script_merge(&r, &v2(), 2);
    r.m.open_merge(LOCAL, editor());
    r.turns(2);
    assert!(!r.m.can_apply_merge());
    r.m.apply_merge();
    r.turn();
    assert!(matches!(r.merge_phase(), MergePhase::Review(_)), "apply does nothing while a conflict is open");
    let conflict = {
        let MergePhase::Review(rev) = r.merge_phase() else { panic!() };
        rev.plan.rows.iter().position(|x| x.kind == RowKind::Conflict).unwrap()
    };
    r.m.set_merge_choice(conflict, Choice::Ours);
    assert!(r.m.can_apply_merge());
    r.m.apply_merge();
    r.turns(2);
    assert!(matches!(r.merge_phase(), MergePhase::Applied(_)));
    assert_eq!(r.banks[0].questions[1].stem, "Two, my way");
}

#[test]
fn when_the_earlier_version_is_not_kept_the_merge_is_two_way_and_says_so() {
    let mut r = Rig::new(&[(QA, "One"), (QB, "Two")]);
    let mut l = r.link();
    l.base.clear();
    r.m.add_link(l).unwrap();
    r.edit_stem(0, "One, reworded by me");
    r.s.on("Post", &format!("{}/merge", bank_path()), 200, merge_start_json(1, 2, false));
    r.s.keep_raw("Get", &bank_path(), served(2, &v2()));
    r.m.open_merge(LOCAL, editor());
    r.turns(2);
    assert!(r.s.paths().iter().all(|p| !p.contains("/versions/")), "a base that is not kept is not asked for");
    let MergePhase::Review(rev) = r.merge_phase() else { panic!("{:?}", r.merge_phase()) };
    assert!(rev.fallback);
    // Both sides differ on the first and second question: nothing is decided for the person.
    assert!(rev.plan.count(RowKind::Conflict) >= 1);
    assert!(!r.m.can_apply_merge());
}

#[test]
fn when_the_copy_kept_no_snapshot_but_the_circle_still_has_the_base_it_is_read() {
    let mut r = Rig::new(&[(QA, "One"), (QB, "Two")]);
    let mut l = r.link();
    l.base.clear();
    r.m.add_link(l).unwrap();
    r.edit_stem(0, "One, reworded by me");
    r.s.on("Post", &format!("{}/merge", bank_path()), 200, merge_start_json(1, 2, true));
    r.s.keep_raw("Get", &bank_path(), served(2, &v2()));
    r.s.keep_raw("Get", &format!("{}/versions/1", bank_path()), served(1, &[(QA, "One"), (QB, "Two")]));
    r.m.open_merge(LOCAL, editor());
    r.turns(2);
    assert!(r.s.paths().iter().any(|p| p.ends_with("/versions/1")));
    let MergePhase::Review(rev) = r.merge_phase() else { panic!("{:?}", r.merge_phase()) };
    assert!(!rev.fallback, "a three-way merge, as the base is known");
    assert_eq!(rev.plan.count(RowKind::OursChanged), 1);
    assert_eq!(rev.plan.count(RowKind::TheirsChanged), 1);
}

#[test]
fn a_copy_that_is_up_to_date_says_so() {
    let mut r = Rig::new(&[(QA, "One")]);
    r.s.on("Post", &format!("{}/merge", bank_path()), 400, json!({"error": "invalid_request", "field": "base_version", "head_version": 1}));
    r.m.open_merge(LOCAL, editor());
    r.turns(2);
    assert!(matches!(r.merge_phase(), MergePhase::UpToDate));
}

#[test]
fn a_hidden_newest_version_can_not_be_merged_but_can_be_sent_over() {
    let mut r = Rig::new(&[(QA, "One")]);
    r.edit_stem(0, "One, reworded");
    r.m.apply_heads(&[HeadEntry { bank_id: BID.into(), state: HeadState::Current { version: 3, license: None, at: None, by: None, hide_key: None } }]);
    r.s.on("Post", &format!("{}/merge", bank_path()), 200, merge_start_json(1, 3, true));
    r.s.keep("Get", &bank_path(), 404, json!({"error": "version_withheld", "message": "raw words"}));
    r.m.open_merge(LOCAL, editor());
    r.turns(2);
    assert!(matches!(r.merge_phase(), MergePhase::HeadWithheld { head_version: Some(3), .. }));
    r.m.send_on_top(3, editor());
    r.frame();
    r.m.set_attest(true);
    r.turn();
    r.s.on("Put", &bank_path(), 201, sent_ok(4, &[QA]));
    r.m.send();
    r.turn();
    let puts = r.puts();
    assert_eq!(body_parts(&puts[0]).0["base_version"], 3);
}

#[test]
fn a_newest_version_that_moved_while_reviewing_applies_nothing() {
    let mut r = Rig::new(&[(QA, "One"), (QB, "Two")]);
    script_merge(&r, &v2(), 2);
    r.m.open_merge(LOCAL, editor());
    r.turns(2);
    // By the time the pictures are fetched, there is a version 3.
    r.s.keep_raw("Get", &bank_path(), served(3, &v2()));
    r.m.apply_merge();
    r.turns(2);
    let MergePhase::Review(rev) = r.merge_phase() else { panic!("{:?}", r.merge_phase()) };
    assert!(rev.problem.as_deref().is_some_and(|p| p.contains("different newest version")));
    assert_eq!(r.banks[0].questions.len(), 2, "the bank is untouched");
    assert_eq!(*r.restores.0.lock().unwrap(), 0);
}

#[test]
fn if_the_restore_point_can_not_be_written_nothing_changes() {
    let s = Script::new();
    let media: SharedMedia = Arc::new(HashMap::<String, Vec<u8>>::new());
    let mut m = CircleEditModel::new(Runner::Inline, Arc::new(Mem::default()), media, Arc::new(BadRestore), LinkLog::memory()).with_retry(RetryPolicy::none());
    let (bank, link) = linked(&[(QA, "One"), (QB, "Two")]);
    m.add_link(link).unwrap();
    m.frame(now(), &ready(&s));
    let mut banks = vec![bank];
    s.on("Post", &format!("{}/merge", bank_path()), 200, merge_start_json(1, 2, true));
    s.keep_raw("Get", &bank_path(), served(2, &v2()));
    m.open_merge(LOCAL, editor());
    for _ in 0..2 {
        m.frame(now(), &ready(&s));
        m.sync(&mut banks);
    }
    m.apply_merge();
    for _ in 0..2 {
        m.frame(now(), &ready(&s));
        m.sync(&mut banks);
    }
    let EditView::Merge(v) = &m.view else { panic!() };
    let MergePhase::Review(rev) = &v.phase else { panic!("{:?}", v.phase) };
    assert!(rev.problem.as_deref().is_some_and(|p| p.contains("could not be saved first")));
    assert_eq!(banks[0].questions.len(), 2);
    assert_eq!(m.link_for_local(LOCAL).unwrap().base_version, 1);
}

#[test]
fn merge_errors_are_in_plain_words_and_a_dead_session_goes_to_sign_in() {
    let mut r = Rig::new(&[(QA, "One")]);
    r.s.fail(ApiError::Offline("down".into()));
    r.m.open_merge(LOCAL, editor());
    r.turn();
    assert!(matches!(r.merge_phase(), MergePhase::Failed(p) if p.retry));
    r.s.fail(ApiError::SignedOut);
    r.m.retry_merge();
    r.turn();
    assert!(r.m.take_signed_out());
    assert!(matches!(r.merge_phase(), MergePhase::Failed(p) if p.message.contains("Sign in again")));
    r.s.on("Post", &format!("{}/merge", bank_path()), 400, json!({"error": "invalid_request", "field": "merge_token"}));
    r.m.retry_merge();
    r.turn();
    assert!(matches!(r.merge_phase(), MergePhase::Failed(p) if p.message.contains("out of date")));
}

// ---- history ----------------------------------------------------------------------------------------------------------

#[test]
fn the_history_pages_and_flags_withheld_and_unkept_versions() {
    let mut r = Rig::new(&[(QA, "One")]);
    let mut first = page(5, &[5, 4, 3], Some(3));
    first["versions"][1]["withheld"] = json!(true);
    first["versions"][1]["kept"] = json!(false);
    first["versions"][1]["message"] = json!("");
    first["versions"][2]["merged_from"] = json!(1);
    first["versions"][2]["rollback_of"] = json!(2);
    first["pinned_versions"] = json!([3]);
    r.s.on("Get", &format!("{}/versions", bank_path()), 200, first);
    r.s.on("Get", &format!("{}/versions", bank_path()), 200, page(5, &[2, 1], None));
    r.m.open_history(CID, BID, "Cells", editor());
    r.turn();
    {
        let h = r.history();
        assert!(h.loaded && h.lines.len() == 3 && h.next_before == Some(3) && h.head_version == 5);
        assert_eq!(version_flags(&h.lines[1], &h.pinned), vec!["Hidden while it is reviewed"]);
        assert_eq!(version_flags(&h.lines[2], &h.pinned), vec!["Kept for a review"]);
        assert!(version_flags(&h.lines[0], &h.pinned).is_empty());
        assert_eq!(h.lines[2].merged_from, Some(1));
        assert_eq!(h.lines[2].rollback_of, Some(2));
    }
    r.m.more_history();
    r.turn();
    let h = r.history();
    assert_eq!(h.lines.iter().map(|l| l.version).collect::<Vec<_>>(), vec![5, 4, 3, 2, 1]);
    assert!(h.next_before.is_none());
    let seen = r.s.seen();
    assert!(seen[1].path.ends_with("versions?before=3"));
    r.m.more_history();
    r.turn();
    assert_eq!(r.s.seen().len(), 2, "no more pages: nothing asked");
}

#[test]
fn a_kept_version_can_be_looked_at_and_a_withheld_one_can_not() {
    let mut r = Rig::new(&[(QA, "One")]);
    let mut p = page(3, &[3, 2, 1], None);
    p["versions"][1]["withheld"] = json!(true);
    p["versions"][1]["kept"] = json!(false);
    r.s.on("Get", &format!("{}/versions", bank_path()), 200, p);
    r.m.open_history(CID, BID, "Cells", viewer());
    r.turn();
    r.m.view_version(2);
    assert!(matches!(r.m.view, EditView::History(_)), "a hidden version is not opened");
    assert_eq!(r.s.seen().len(), 1);
    r.s.keep_raw("Get", &format!("{}/versions/1", bank_path()), served(1, &[(QA, "First question"), (QB, "Second question")]));
    r.m.view_version(1);
    r.turn();
    let EditView::Version(v) = &r.m.view else { panic!() };
    let VersionLoad::Ready(b) = &v.load else { panic!("{:?}", v.load) };
    assert_eq!(b.questions.len(), 2);
    assert_eq!(b.questions[0].stem, "First question");
    assert_eq!(b.questions[0].kind, "Mc");
    r.m.back();
    assert!(matches!(r.m.view, EditView::History(_)));
    r.m.back();
    assert!(matches!(r.m.view, EditView::Closed));
}

#[test]
fn a_version_that_is_no_longer_kept_says_so() {
    let mut r = Rig::new(&[(QA, "One")]);
    r.s.on("Get", &format!("{}/versions", bank_path()), 200, page(30, &[30, 5], None));
    r.m.open_history(CID, BID, "Cells", editor());
    r.turn();
    r.s.on("Get", &format!("{}/versions/5", bank_path()), 404, json!({"error": "version_not_retained", "oldest_kept": 11, "message": "raw"}));
    r.m.view_version(5);
    r.turn();
    let EditView::Version(v) = &r.m.view else { panic!() };
    assert!(matches!(&v.load, VersionLoad::Failed(p) if p.message.contains("no longer kept") && !p.message.contains("version_not_retained")));
}

#[test]
fn restoring_a_version_asks_then_posts_a_rollback_and_reloads() {
    let mut r = Rig::new(&[(QA, "One")]);
    r.s.on("Get", &format!("{}/versions", bank_path()), 200, page(3, &[3, 2, 1], None));
    r.m.open_history(CID, BID, "Cells", owner());
    r.turn();
    r.m.ask_restore(3);
    assert!(r.history().restore.is_none(), "the newest version is not restorable");
    r.m.ask_restore(1);
    assert!(matches!(r.history().restore.as_ref().map(|x| &x.stage), Some(RestoreStage::Confirm)));
    assert_eq!(r.s.seen().len(), 1, "asking sends nothing");
    r.s.on("Post", &format!("{}/rollback", bank_path()), 200, json!({"circle_id": CID, "bank_id": BID, "version": 4, "version_id": hex(4), "qids": [QA], "added": 0, "changed": 1, "removed": 0, "rollback_of": 1}));
    r.s.on("Get", &format!("{}/versions", bank_path()), 200, page(4, &[4, 3, 2, 1], None));
    r.m.confirm_restore(false);
    r.m.confirm_restore(false);
    r.turn();
    let post = r.s.seen().into_iter().find(|x| x.path.ends_with("/rollback")).unwrap();
    let j = post.json();
    assert_eq!((j["base_version"].as_u64(), j["to_version"].as_u64()), (Some(3), Some(1)));
    assert!(is_publish_id(j["commit_id"].as_str().unwrap()));
    assert_eq!(r.s.seen().iter().filter(|x| x.path.ends_with("/rollback")).count(), 1, "double press sends once");
    assert!(matches!(r.history().restore.as_ref().map(|x| &x.stage), Some(RestoreStage::Done { version: 4, .. })));
    assert_eq!(r.history().head_version, 4, "the history was reloaded");
    r.m.close_restore_result();
    assert!(r.history().restore.is_none());
}

#[test]
fn only_the_owner_or_the_author_can_restore_or_trim() {
    for p in [editor(), viewer(), Perm { role: Role::Viewer, is_author: true, closed: false }] {
        let mut r = Rig::new(&[(QA, "One")]);
        r.s.on("Get", &format!("{}/versions", bank_path()), 200, page(3, &[3, 2, 1], None));
        r.m.open_history(CID, BID, "Cells", p);
        r.turn();
        r.m.ask_restore(1);
        r.m.open_trim();
        assert!(r.history().restore.is_none() && r.history().trim.is_none());
        r.m.confirm_restore(false);
        r.m.confirm_trim();
        assert_eq!(r.s.seen().len(), 1);
    }
    let mut r = Rig::new(&[(QA, "One")]);
    r.s.on("Get", &format!("{}/versions", bank_path()), 200, page(3, &[3, 2, 1], None));
    r.m.open_history(CID, BID, "Cells", Perm { role: Role::Editor, is_author: true, closed: false });
    r.turn();
    r.m.ask_restore(1);
    assert!(r.history().restore.is_some(), "the bank's author, while an editor, can");
}

#[test]
fn a_restore_that_drops_most_questions_asks_again_and_a_daily_cap_is_plain() {
    let mut r = Rig::new(&[(QA, "One")]);
    r.s.on("Get", &format!("{}/versions", bank_path()), 200, page(3, &[3, 2, 1], None));
    r.m.open_history(CID, BID, "Cells", owner());
    r.turn();
    r.m.ask_restore(1);
    r.s.on("Post", &format!("{}/rollback", bank_path()), 428, json!({"error": "confirm_large_required", "removed": 8, "total": 10}));
    r.m.confirm_restore(false);
    r.turn();
    assert!(matches!(r.history().restore.as_ref().map(|x| &x.stage), Some(RestoreStage::Large { removed: 8, total: 10 })));
    r.s.on("Post", &format!("{}/rollback", bank_path()), 429, json!({"error": "daily_limit", "metric": "circle_rollbacks"}));
    r.m.confirm_restore(true);
    r.turn();
    let j = r.s.seen().into_iter().rfind(|x| x.path.ends_with("/rollback")).unwrap().json();
    assert_eq!(j["confirm_large_change"], true);
    let Some(RestoreStage::Failed(p)) = r.history().restore.as_ref().map(|x| &x.stage) else { panic!() };
    assert!(p.message.contains("restored as many older versions today") && !p.message.contains("circle_rollbacks"));
}

#[test]
fn a_restore_over_a_newer_version_reloads_the_history() {
    let mut r = Rig::new(&[(QA, "One")]);
    r.s.on("Get", &format!("{}/versions", bank_path()), 200, page(3, &[3, 2, 1], None));
    r.m.open_history(CID, BID, "Cells", owner());
    r.turn();
    r.m.ask_restore(1);
    r.s.on("Post", &format!("{}/rollback", bank_path()), 409, json!({"error": "conflict", "head_version": 4}));
    r.s.on("Get", &format!("{}/versions", bank_path()), 200, page(4, &[4, 3, 2, 1], None));
    r.m.confirm_restore(false);
    r.turn();
    let Some(RestoreStage::Failed(p)) = r.history().restore.as_ref().map(|x| &x.stage) else { panic!() };
    assert!(p.message.contains("newer version arrived"));
    assert_eq!(r.history().head_version, 4);
}

#[test]
fn trimming_asks_first_keeps_one_to_nineteen_and_reloads() {
    let mut r = Rig::new(&[(QA, "One")]);
    r.s.on("Get", &format!("{}/versions", bank_path()), 200, page(25, &[25, 24], Some(24)));
    r.m.open_history(CID, BID, "Cells", owner());
    r.turn();
    r.m.open_trim();
    r.m.set_trim_keep(0);
    assert_eq!(r.history().trim.as_ref().unwrap().keep, 1);
    r.m.set_trim_keep(99);
    assert_eq!(r.history().trim.as_ref().unwrap().keep, 19);
    r.m.set_trim_keep(8);
    r.m.confirm_trim();
    assert_eq!(r.s.seen().len(), 1, "no trim without the second question");
    r.m.ask_trim();
    r.s.on("Post", &format!("{}/trim", bank_path()), 200, json!({"ok": true, "pruned": 12, "oldest_kept": 18, "pinned_versions": [3]}));
    r.s.on("Get", &format!("{}/versions", bank_path()), 200, page(25, &[25, 24], None));
    r.m.confirm_trim();
    r.m.confirm_trim();
    r.turn();
    let post = r.s.seen().into_iter().find(|x| x.path.ends_with("/trim")).unwrap();
    assert_eq!(post.json(), json!({"keep": 8}));
    assert_eq!(r.s.seen().iter().filter(|x| x.path.ends_with("/trim")).count(), 1);
    let Some(TrimStage::Done(t)) = r.history().trim.as_ref().map(|x| &x.stage) else { panic!() };
    assert_eq!((t.pruned, t.oldest_kept, t.pinned_versions.clone()), (12, Some(18), vec![3]));
    r.m.close_trim_result();
    assert!(r.history().trim.is_none());
}

#[test]
fn a_trim_the_pool_refuses_is_in_plain_words() {
    let mut r = Rig::new(&[(QA, "One")]);
    r.s.on("Get", &format!("{}/versions", bank_path()), 200, page(25, &[25], None));
    r.m.open_history(CID, BID, "Cells", owner());
    r.turn();
    r.m.open_trim();
    r.m.ask_trim();
    r.s.on("Post", &format!("{}/trim", bank_path()), 409, json!({"error": "under_moderation", "message": "This bank is under review; its versions stay until an administrator decides."}));
    r.m.confirm_trim();
    r.turn();
    let Some(TrimStage::Failed(p)) = r.history().trim.as_ref().map(|x| &x.stage) else { panic!() };
    assert!(p.message.contains("under review") && !p.message.contains("under_moderation"));
}

// ---- null tolerance and wording ------------------------------------------------------------------------------

#[test]
fn texts_the_pool_sends_as_null_are_read_as_empty() {
    let mut r = Rig::new(&[(QA, "One")]);
    let p = json!({"bank_id": BID, "head_version": 2, "oldest_kept": null, "pinned_versions": null,
        "versions": [{"version": 2, "version_id": null, "by": null, "at": NOW, "message": null, "added": null, "changed": 1, "removed": 0, "question_count": 2, "license": null, "kept": true, "withheld": false, "rollback_of": null, "merged_from": null},
                     {"version": 1, "version_id": hex(1), "by": "Someone", "at": NOW, "message": "", "added": 0, "changed": 0, "removed": 0, "question_count": 2, "license": "CC0-1.0", "kept": true, "withheld": false}], "next_before": null});
    r.s.on("Get", &format!("{}/versions", bank_path()), 200, p);
    r.m.open_history(CID, BID, "Cells", editor());
    r.turn();
    let h = r.history();
    assert!(h.loaded && h.problem.is_none());
    assert_eq!(h.lines[0].by, "");
    assert_eq!(h.lines[0].message, "");
    assert_eq!(h.lines[0].added, 0);
}

#[test]
fn a_history_the_app_cannot_read_is_refused_not_shown_as_empty() {
    let mut r = Rig::new(&[(QA, "One")]);
    r.s.on("Get", &format!("{}/versions", bank_path()), 200, json!({"bank_id": BID, "head_version": 2, "versions": "oops"}));
    r.m.open_history(CID, BID, "Cells", editor());
    r.turn();
    let h = r.history();
    assert!(!h.loaded && h.problem.is_some());
}

#[test]
fn a_merge_start_without_a_token_is_refused() {
    let mut r = Rig::new(&[(QA, "One")]);
    let mut v = merge_start_json(1, 2, true);
    v["merge_token"] = Value::Null;
    r.s.on("Post", &format!("{}/merge", bank_path()), 200, v);
    r.m.open_merge(LOCAL, editor());
    r.turn();
    assert!(matches!(r.merge_phase(), MergePhase::Failed(_)));
}

#[test]
fn every_new_pool_code_is_in_plain_words_without_the_code() {
    let cases: Vec<(&str, u16, Value)> = vec![
        ("confirm_large_required", 428, json!({"removed": 3, "total": 4})),
        ("license_author_only", 422, json!({"license": "CC0-1.0"})),
        ("merge_token_invalid", 422, json!({})),
        ("version_withheld", 404, json!({"message": "raw"})),
        ("version_not_retained", 404, json!({"oldest_kept": 3})),
        ("conflict", 409, json!({"head_version": 3, "head_at": 1, "head_by": "X"})),
        ("under_moderation", 409, json!({})),
        ("publish_in_progress", 409, json!({})),
        ("commit_id_conflict", 409, json!({})),
        ("bank_removed", 409, json!({})),
        ("daily_limit", 429, json!({"metric": "circle_rollbacks"})),
        ("quota_exceeded", 413, json!({"scope": "circle", "used": 5, "limit": 6})),
        ("invalid_request", 400, json!({"field": "keep"})),
    ];
    for (code, status, extra) in cases {
        let s = Script::new();
        let mut body = json!({"error": code});
        merge_json(&mut body, extra);
        s.on("Get", &format!("{}/versions/3", bank_path()), status, body);
        let pool = Pool::new(&*s).with_retry(RetryPolicy::none());
        let e = super::transport::block_on(pool.circle_version(CID, BID, 3)).unwrap_err();
        let msg = e.message();
        assert!(!msg.is_empty(), "{code}");
        assert!(!msg.contains(code), "{code} leaked: {msg}");
        assert!(!msg.contains('_'), "{code}: {msg}");
    }
}

fn merge_json(v: &mut Value, extra: Value) {
    if let (Some(o), Some(e)) = (v.as_object_mut(), extra.as_object()) {
        for (k, x) in e {
            o.insert(k.clone(), x.clone());
        }
    }
}

#[test]
fn the_byte_cap_points_at_trimming() {
    let s = Script::new();
    s.on("Put", &bank_path(), 413, json!({"error": "quota_exceeded", "scope": "circle", "used": 99_000_000u64, "limit": 100_000_000u64}));
    let pool = Pool::new(&*s).with_retry(RetryPolicy::none());
    let (bank, link) = linked(&[(QA, "One")]);
    let mut req = CircleSend::new(payload_of(&link, &bank).unwrap(), License::Cc0, link.meta());
    req.base_version = Some(1);
    let media: HashMap<String, Vec<u8>> = HashMap::new();
    let e = super::transport::block_on(pool.send_circle_version(CID, BID, &req, &media, &mut |_| {})).unwrap_err();
    assert!(e.message().contains("trim"), "{}", e.message());
}

// ---- the routes by hand ---------------------------------------------------------------------------------------

#[test]
fn a_send_over_a_bank_needs_a_base_version_and_a_known_bank_id() {
    let s = Script::new();
    let pool = Pool::new(&*s).with_retry(RetryPolicy::none());
    let (bank, link) = linked(&[(QA, "One")]);
    let req = CircleSend::new(payload_of(&link, &bank).unwrap(), License::Cc0, link.meta());
    let media: HashMap<String, Vec<u8>> = HashMap::new();
    assert!(super::transport::block_on(pool.send_circle_version(CID, BID, &req, &media, &mut |_| {})).is_err());
    assert!(super::transport::block_on(pool.circle_versions("nope", BID, None)).is_err());
    assert!(super::transport::block_on(pool.trim_circle_bank(CID, BID, 20)).is_err());
    assert!(super::transport::block_on(pool.trim_circle_bank(CID, BID, 0)).is_err());
    assert!(super::transport::block_on(pool.rollback_circle_bank(CID, BID, "short", 3, 1, "", false)).is_err());
    assert!(s.seen().is_empty(), "nothing is sent for a request that can not be right");
}

// ---- working copies from the circle page ---------------------------------------------------------------------

fn open_bank_page(role: &str) -> (CirclesModel, Arc<Script>) {
    let (mut m, s) = open_circle(role, json!({}));
    s.keep_raw("Get", &bank_path(), served(2, &[(QA, "One"), (QB, "Two")]));
    m.open_bank(BID);
    for _ in 0..3 {
        m.frame(now(), &ready(&s));
    }
    (m, s)
}

fn link_log_model(m: &mut CirclesModel) -> LinkLog {
    let log = LinkLog::memory();
    let media: SharedMedia = Arc::new(HashMap::<String, Vec<u8>>::new());
    m.edit = CircleEditModel::new(Runner::Inline, Arc::new(Mem::default()), media, Arc::new(MemRestore::default()), log.clone()).with_retry(RetryPolicy::none());
    log
}

#[test]
fn an_editor_makes_a_working_copy_that_is_linked_to_the_circle_bank() {
    let (mut m, s) = open_bank_page("editor");
    let log = link_log_model(&mut m);
    let mut browse = super::browse::BrowseModel::new(Runner::Inline, Arc::new(Mem::default()), super::browse::ImportLog::memory());
    assert!(m.can_make_working_copy());
    m.begin_copy();
    m.make_working_copy();
    m.frame(now(), &ready(&s));
    let mut banks = Vec::new();
    assert!(m.sync(&mut banks, &mut browse));
    assert_eq!(banks.len(), 1);
    let links = log.load();
    assert_eq!(links.len(), 1);
    let l = &links[0];
    assert_eq!((l.circle_id.as_str(), l.circle_bank_id.as_str(), l.local_bank_id.as_str()), (CID, BID, banks[0].id.as_str()));
    assert_eq!(l.base_version, 2);
    assert_eq!(l.qids.iter().map(|q| q.qid.as_str()).collect::<Vec<_>>(), vec![QA, QB]);
    assert_eq!(l.base.len(), 2);
    assert_eq!(l.license, "CC-BY-4.0");
    assert_eq!(banks[0].questions.len(), 2);
    assert!(browse.imported().iter().any(|r| r.circle.is_some()), "it is also in the list of copies");
    // A second copy button press after the copy is done makes nothing more.
    assert!(!m.sync(&mut banks, &mut browse));
}

#[test]
fn a_viewer_keeps_a_plain_read_only_copy() {
    let (mut m, s) = open_bank_page("viewer");
    let log = link_log_model(&mut m);
    let mut browse = super::browse::BrowseModel::new(Runner::Inline, Arc::new(Mem::default()), super::browse::ImportLog::memory());
    assert!(!m.can_make_working_copy());
    m.begin_copy();
    m.make_working_copy();
    assert!(!m.busy(), "no working copy for a viewer");
    m.get_copy();
    m.frame(now(), &ready(&s));
    let mut banks = Vec::new();
    assert!(m.sync(&mut banks, &mut browse));
    assert!(log.load().is_empty(), "a plain copy is not linked");
}

#[test]
fn a_plain_copy_for_an_editor_is_not_linked_either() {
    let (mut m, s) = open_bank_page("editor");
    let log = link_log_model(&mut m);
    let mut browse = super::browse::BrowseModel::new(Runner::Inline, Arc::new(Mem::default()), super::browse::ImportLog::memory());
    m.begin_copy();
    m.get_copy();
    m.frame(now(), &ready(&s));
    let mut banks = Vec::new();
    m.sync(&mut banks, &mut browse);
    assert!(log.load().is_empty());
}

#[test]
fn the_list_of_copies_follows_a_send_or_a_merge() {
    let (mut m, s) = open_bank_page("editor");
    let _ = link_log_model(&mut m);
    let mut browse = super::browse::BrowseModel::new(Runner::Inline, Arc::new(Mem::default()), super::browse::ImportLog::memory());
    m.begin_copy();
    m.make_working_copy();
    m.frame(now(), &ready(&s));
    let mut banks = Vec::new();
    m.sync(&mut banks, &mut browse);
    let local = banks[0].id.clone();
    banks[0].questions[0].stem = "One, reworded".into();
    banks[0].modified += 1;
    let perm = m.perm_for(CID, BID);
    s.keep("Post", "/circles/heads", 200, heads_json("CC-BY-4.0", 1));
    m.edit.open_send(&local, perm);
    m.frame(now(), &ready(&s));
    m.edit.set_attest(true);
    m.sync(&mut banks, &mut browse);
    s.on("Put", &bank_path(), 201, sent_ok(3, &[QA, QB]));
    m.edit.send();
    m.sync(&mut banks, &mut browse);
    m.frame(now(), &ready(&s));
    m.sync(&mut banks, &mut browse);
    let rec = browse.imported().iter().find(|r| r.circle.is_some()).cloned().unwrap();
    assert_eq!(rec.version, 3);
}

#[test]
fn a_session_that_ends_inside_a_working_copy_screen_sends_the_page_to_sign_in() {
    let (mut m, s) = open_bank_page("editor");
    let _ = link_log_model(&mut m);
    m.frame(now(), &ready(&s));
    let perm = m.perm_for(CID, BID);
    s.fail(ApiError::SignedOut);
    m.edit.open_history(CID, BID, "Cells", perm);
    m.frame(now(), &ready(&s));
    assert_eq!(m.screen, Screen::Gate);
    assert_eq!(m.gate, super::publish::Gate::SignedOut);
}

fn _unused(_: &AccountSnapshot) {}


// ---- smoke frames --------------------------------------------------------------------------------------------

/// Draws the rig's edit screen through the circles tab, wide and narrow, and returns the visible text.
fn draw_all(r: &mut Rig, ctx: &egui::Context) -> String {
    let mut all = String::new();
    for width in [1000.0, 420.0] {
        let mut cm = model();
        std::mem::swap(&mut cm.edit, &mut r.m);
        frame(ctx, width, &mut |ui| {
            super::circles_ui::body_of(ui, &mut cm);
        });
        let out = ctx.run_ui(
            egui::RawInput { screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(width, 900.0))), time: Some(0.0), ..Default::default() },
            |ui| {
                egui::CentralPanel::default().show(ui, |ui| {
                    super::circles_ui::body_of(ui, &mut cm);
                });
            },
        );
        for s in &out.shapes {
            if let egui::epaint::Shape::Text(t) = &s.shape {
                all.push_str(t.galley.text());
                all.push('\n');
            }
        }
        out.drop_without_applying_deltas();
        std::mem::swap(&mut cm.edit, &mut r.m);
    }
    all
}

#[test]
fn every_working_copy_screen_and_state_draws_wide_and_narrow() {
    let ctx = egui::Context::default();
    crate::theme::setup(&ctx);

    // Send: editing, conflict, large, done.
    let mut r = Rig::new(&[(QA, "One"), (QB, "Two")]);
    r.edit_stem(0, "One, reworded");
    r.open_send(editor());
    r.turn();
    let t = draw_all(&mut r, &ctx);
    assert!(t.contains("Send my changes") && t.contains("A note for the history"), "{t}");
    r.s.on("Put", &bank_path(), 409, json!({"error": "conflict", "head_version": 3, "head_at": NOW - 1000, "head_by": "Ms Frizzle"}));
    r.m.send();
    r.turn();
    let t = draw_all(&mut r, &ctx);
    assert!(t.contains("Someone else sent changes first") && t.contains("Get latest and merge"), "{t}");
    r.m.back_to_note();
    r.s.on("Put", &bank_path(), 428, json!({"error": "confirm_large_required", "removed": 1, "total": 2}));
    r.m.send();
    r.turn();
    let t = draw_all(&mut r, &ctx);
    assert!(t.contains("removes 1 of the bank's 2 questions") && t.contains("Send anyway"), "{t}");
    r.m.cancel_large_send();
    r.s.on("Put", &bank_path(), 201, sent_ok(4, &[QA, QB]));
    r.m.send();
    r.turn();
    let t = draw_all(&mut r, &ctx);
    assert!(t.contains("Sent"), "{t}");

    // Merge: review with a conflict, applied.
    let mut r = Rig::new(&[(QA, "One"), (QB, "Two")]);
    r.edit_stem(1, "Two, my way");
    script_merge(&r, &v2(), 2);
    r.m.open_merge(LOCAL, editor());
    let t = draw_all(&mut r, &ctx);
    assert!(t.contains("Looking at what changed"), "{t}");
    r.turns(2);
    let t = draw_all(&mut r, &ctx);
    assert!(t.contains("Apply merge") && t.contains("Fixed the second question") && t.contains("Keep mine"), "{t}");
    let i = {
        let MergePhase::Review(rev) = r.merge_phase() else { panic!() };
        rev.plan.rows.iter().position(|x| x.kind == RowKind::Conflict).unwrap()
    };
    r.m.set_merge_choice(i, Choice::Ours);
    r.m.apply_merge();
    r.turns(2);
    let t = draw_all(&mut r, &ctx);
    assert!(t.contains("now in step with version 2") && t.contains("Your way back"), "{t}");

    // Two-way fallback says so.
    let mut r = Rig::new(&[(QA, "One"), (QB, "Two")]);
    let mut l = r.link();
    l.base.clear();
    r.m.add_link(l).unwrap();
    r.edit_stem(0, "One, reworded by me");
    r.s.on("Post", &format!("{}/merge", bank_path()), 200, merge_start_json(1, 2, false));
    r.s.keep_raw("Get", &bank_path(), served(2, &v2()));
    r.m.open_merge(LOCAL, editor());
    r.turns(2);
    let t = draw_all(&mut r, &ctx);
    assert!(t.contains("no longer kept") && !t.contains("When you copied it"), "{t}");

    // Up to date, hidden head, failure.
    let mut r = Rig::new(&[(QA, "One")]);
    r.s.on("Post", &format!("{}/merge", bank_path()), 400, json!({"error": "invalid_request", "field": "base_version"}));
    r.m.open_merge(LOCAL, editor());
    r.turns(2);
    let t = draw_all(&mut r, &ctx);
    assert!(!t.contains("invalid_request"), "{t}");
    let mut r = Rig::new(&[(QA, "One")]);
    r.m.apply_heads(&[HeadEntry { bank_id: BID.into(), state: HeadState::Current { version: 3, license: None, at: None, by: None, hide_key: None } }]);
    r.s.on("Post", &format!("{}/merge", bank_path()), 200, merge_start_json(1, 3, true));
    r.s.keep("Get", &bank_path(), 404, json!({"error": "version_withheld"}));
    r.m.open_merge(LOCAL, editor());
    r.turns(2);
    let t = draw_all(&mut r, &ctx);
    assert!(t.contains("Send my version on top"), "{t}");

    // History with flags, a version, restore and trim panels.
    let mut r = Rig::new(&[(QA, "One")]);
    let mut p = page(25, &[25, 24, 23], Some(23));
    p["versions"][1]["withheld"] = json!(true);
    p["versions"][1]["kept"] = json!(false);
    p["versions"][2]["merged_from"] = json!(20);
    p["versions"][2]["rollback_of"] = json!(2);
    r.s.on("Get", &format!("{}/versions", bank_path()), 200, p);
    r.m.open_history(CID, BID, "Cells", owner());
    let t = draw_all(&mut r, &ctx);
    assert!(t.contains("Loading the history"), "{t}");
    r.turn();
    let t = draw_all(&mut r, &ctx);
    assert!(t.contains("Version 25") && t.contains("Hidden while it is reviewed") && t.contains("Trim old versions") && t.contains("Older versions"), "{t}");
    r.m.ask_restore(23);
    let t = draw_all(&mut r, &ctx);
    assert!(t.contains("Restore it"), "{t}");
    r.m.cancel_restore();
    r.m.open_trim();
    let t = draw_all(&mut r, &ctx);
    assert!(t.contains("Keep the newest"), "{t}");
    r.m.ask_trim();
    let t = draw_all(&mut r, &ctx);
    assert!(t.contains("It can not be undone"), "{t}");
    r.m.cancel_trim();
    r.s.keep_raw("Get", &format!("{}/versions/23", bank_path()), served(23, &[(QA, "First question")]));
    r.m.view_version(23);
    r.turn();
    let t = draw_all(&mut r, &ctx);
    assert!(t.contains("First question"), "{t}");

    // A viewer sees the history without restore or trim.
    let mut r = Rig::new(&[(QA, "One")]);
    r.s.on("Get", &format!("{}/versions", bank_path()), 200, page(3, &[3, 2, 1], None));
    r.m.open_history(CID, BID, "Cells", viewer());
    r.turn();
    let t = draw_all(&mut r, &ctx);
    assert!(!t.contains("Restore this version") && !t.contains("Trim old versions"), "{t}");
}

#[test]
fn the_circle_page_shows_working_copies_and_the_bank_page_offers_a_working_copy() {
    let ctx = egui::Context::default();
    crate::theme::setup(&ctx);
    for width in [1000.0, 420.0] {
        let (mut m, _s) = open_circle("editor", json!({}));
        let (_, mut l) = linked(&[(QA, "One")]);
        l.head_seen = 3;
        m.edit.add_link(l).unwrap();
        let mut seen = String::new();
        let out = ctx.run_ui(
            egui::RawInput { screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(width, 1400.0))), time: Some(0.0), ..Default::default() },
            |ui| {
                egui::CentralPanel::default().show(ui, |ui| {
                    super::circles_ui::body_of(ui, &mut m);
                });
            },
        );
        for s in &out.shapes {
            if let egui::epaint::Shape::Text(t) = &s.shape {
                seen.push_str(t.galley.text());
                seen.push('\n');
            }
        }
        out.drop_without_applying_deltas();
        assert!(seen.contains("My working copies") && seen.contains("2 new versions in the circle"), "{seen}");
        assert!(seen.contains("You have a working copy"), "{seen}");
    }
}

// ---- critic 49 / 50 fixes ----------------------------------------------------------------------------------

/// A bank as the pool serves it, from ready-made question values.
fn served_raw(version: u64, qs: Vec<Value>) -> Vec<u8> {
    let head = json!({"circle": {"id": CID, "name": "Biology team"}, "bank": {"id": BID, "title": "Cells", "subject": "Biology", "course": "", "grade": "", "license": "CC-BY-4.0", "version_license": "CC-BY-4.0", "author_name": "Mr Keesler", "is_author": false, "version": version, "version_id": hex(version), "question_count": qs.len(), "has_images": false, "committed_at": NOW - DAY}, "origins": null});
    let h = head.to_string();
    let p = json!({"schema_version": 2, "bank": {"id": BID, "name": "Cells", "questions": qs, "created": 1, "modified": 2}}).to_string();
    format!("{},\"payload\":{}}}", &h[..h.len() - 1], p).into_bytes()
}

fn tick_free_send(r: &mut Rig) {
    r.m.open_send(LOCAL, editor());
    r.frame();
}

#[test]
fn b1_a_send_needs_the_own_work_box_ticked_every_time() {
    let mut r = Rig::new(&[(QA, "One")]);
    r.edit_stem(0, "One, reworded");
    tick_free_send(&mut r);
    assert!(r.m.send_blocker().is_some_and(|b| b.contains("own work")), "{:?}", r.m.send_blocker());
    r.m.send();
    r.turns(2);
    assert!(r.puts().is_empty(), "nothing is sent without the tick");
    assert!(matches!(r.send_stage(), SendStage::Editing));
    r.m.set_attest(true);
    assert!(r.m.send_blocker().is_none());
    r.s.on("Put", &bank_path(), 201, sent_ok(2, &[QA]));
    r.m.send();
    r.turn();
    let (meta, _) = body_parts(&r.puts()[0]);
    assert_eq!(meta["attest"]["own_work"], true);
    // The tick is not remembered: the next page starts unticked.
    r.m.back();
    r.m.close();
    tick_free_send(&mut r);
    let EditView::Send(v) = &r.m.view else { panic!() };
    assert!(!v.attest);
    assert!(r.m.send_blocker().is_some());
    // The statement on screen is the same words as the first share.
    assert_eq!(SEND_ATTEST, ATTEST_STATEMENT);
}

#[test]
fn b1_a_refused_attestation_unticks_the_box_with_plain_words() {
    let mut r = Rig::new(&[(QA, "One")]);
    r.open_send(editor());
    r.s.on("Put", &bank_path(), 422, json!({"error": "attestation_required"}));
    r.m.send();
    r.turn();
    let EditView::Send(v) = &r.m.view else { panic!() };
    assert!(!v.attest);
    assert!(matches!(&v.stage, SendStage::Failed(p) if !p.message.contains("attestation_required") && !p.message.is_empty()));
}

#[test]
fn b2_the_send_goes_with_the_licence_the_pool_has_now() {
    let mut r = Rig::new(&[(QA, "One")]);
    // The author tightened the licence after this copy was made.
    r.s.keep("Post", "/circles/heads", 200, heads_json("circle-only", 1));
    r.edit_stem(0, "One, reworded");
    r.open_send(owner());
    let EditView::Send(v) = &r.m.view else { panic!() };
    assert!(matches!(v.licence, LicenceState::Known(License::CircleOnly)));
    r.s.on("Put", &bank_path(), 201, sent_ok(2, &[QA]));
    r.m.send();
    r.turn();
    let (meta, _) = body_parts(&r.puts()[0]);
    assert_eq!(meta["license"], "circle-only", "never the stale CC-BY");
    assert_eq!(r.link().license, "circle-only");
}

#[test]
fn b2_a_licence_that_can_not_be_checked_blocks_the_send_and_can_be_tried_again() {
    let mut r = Rig::new(&[(QA, "One")]);
    r.s.keep("Post", "/circles/heads", 503, json!({"error": "busy"}));
    r.m.open_send(LOCAL, editor());
    r.frame();
    r.m.set_attest(true);
    assert!(r.m.send_blocker().is_some_and(|b| b.contains("licence")));
    r.m.send();
    r.turns(2);
    assert!(r.puts().is_empty());
    r.s.keep("Post", "/circles/heads", 200, heads_json("CC-BY-4.0", 1));
    r.m.retry_licence();
    r.frame();
    assert!(r.m.send_blocker().is_none());
}

#[test]
fn b3_apply_refuses_when_the_bank_was_edited_during_the_review_and_rebuilds_it() {
    let mut r = Rig::new(&[(QA, "One"), (QB, "Two")]);
    script_merge(&r, &v2(), 2);
    r.m.open_merge(LOCAL, editor());
    r.turns(2);
    assert!(matches!(r.merge_phase(), MergePhase::Review(_)));
    // The person goes to the editor and rewrites question two while the review is open.
    r.edit_stem(1, "Two, MY LATER EDIT");
    r.m.apply_merge();
    r.turns(4);
    assert_eq!(r.banks[0].questions[1].stem, "Two, MY LATER EDIT", "the edit is not overwritten");
    assert_eq!(*r.restores.0.lock().unwrap(), 0, "nothing was applied");
    let MergePhase::Review(rev) = r.merge_phase() else { panic!("{:?}", r.merge_phase()) };
    assert!(rev.problem.as_deref().is_some_and(|p| p.contains("changed while you were reviewing")));
    // The review is made again from the bank as it is: now question two is a conflict.
    assert_eq!(rev.plan.count(RowKind::Conflict), 1);
}

fn picture_questions() -> Vec<Value> {
    let mut two = qjson(QB, "Two, with a picture");
    two["image"] = json!({"file": format!("media/{}.png", "ab".repeat(32)), "width": 2, "height": 2});
    vec![qjson(QA, "One"), two]
}

#[test]
fn b4_a_picture_that_fails_to_download_stops_the_merge() {
    let mut r = Rig::new(&[(QA, "One"), (QB, "Two")]);
    r.s.on("Post", &format!("{}/merge", bank_path()), 200, merge_start_json(1, 2, true));
    r.s.keep_raw("Get", &bank_path(), served_raw(2, picture_questions()));
    r.s.keep("Get", &format!("{}/blobs/{}", bank_path(), "ab".repeat(32)), 503, json!({"error": "busy"}));
    r.m.open_merge(LOCAL, editor());
    r.turns(2);
    assert!(matches!(r.merge_phase(), MergePhase::Review(_)));
    r.m.apply_merge();
    r.turns(3);
    let MergePhase::Review(rev) = r.merge_phase() else { panic!("{:?}", r.merge_phase()) };
    assert!(rev.problem.as_deref().is_some_and(|p| p.contains("picture") && p.contains("Nothing was applied")), "{:?}", rev.problem);
    assert_eq!(*r.restores.0.lock().unwrap(), 0);
    assert_eq!(r.banks[0].questions[1].stem, "Two");
    assert_eq!(r.link().base_version, 1, "the link did not move");
}

#[test]
fn b4_a_working_copy_that_lost_pictures_is_not_linked() {
    let (mut m, s) = open_circle("editor", json!({}));
    let _ = link_log_model(&mut m);
    let mut browse = super::browse::BrowseModel::new(Runner::Inline, Arc::new(Mem::default()), super::browse::ImportLog::memory());
    let with_picture = String::from_utf8(bank_body(&["First", "Second"])).unwrap().replacen("\"keyless\":false", &format!("\"image\":{{\"file\":\"media/{}.png\",\"height\":2,\"width\":2}},\"keyless\":false", "ab".repeat(32)), 1);
    s.keep_raw("Get", &bank_path(), with_picture.into_bytes());
    s.keep("Get", &format!("{}/blobs/{}", bank_path(), "ab".repeat(32)), 503, json!({"error": "busy"}));
    m.open_bank(BID);
    m.frame(now(), &ready(&s));
    m.frame(now(), &ready(&s));
    m.begin_copy();
    m.make_working_copy();
    m.frame(now(), &ready(&s));
    let mut banks = Vec::new();
    m.sync(&mut banks, &mut browse);
    assert_eq!(banks.len(), 1, "the questions were added as a plain copy");
    assert!(m.edit.link_for_local(&banks[0].id).is_none(), "but it is not linked: a send would remove the pictures");
    assert!(matches!(&m.notice, Some(super::model::Notice::Error(t)) if t.contains("not linked")));
}

#[test]
fn b5_a_bank_put_back_from_a_saved_copy_is_stale_and_merges_two_way() {
    let mut r = Rig::new(&[(QA, "One"), (QB, "Two")]);
    let before = r.banks[0].clone();
    script_merge(&r, &v2(), 2);
    r.m.open_merge(LOCAL, editor());
    r.turns(2);
    r.m.apply_merge();
    r.turns(2);
    assert!(matches!(r.merge_phase(), MergePhase::Applied(_)));
    assert_eq!(r.link().base_version, 2);
    // The "way back": the saved copy of the banks is put back, the link file is not.
    r.m.close();
    r.banks[0] = before;
    r.turn();
    assert!(r.m.status(LOCAL).unwrap().stale);
    r.open_send(editor());
    r.turn();
    assert!(r.m.send_blocker().is_some_and(|b| b.contains("no longer matches")), "{:?}", r.m.send_blocker());
    r.m.send();
    r.turns(2);
    assert!(r.puts().is_empty(), "a stale copy never sends: it would revert other people's work");
    // Merging again compares both ways: nothing is decided for the person.
    r.m.close();
    script_merge(&r, &v2(), 2);
    r.s.on("Post", &format!("{}/merge", bank_path()), 200, merge_start_json(1, 2, true));
    r.m.open_merge(LOCAL, editor());
    r.turns(2);
    let MergePhase::Review(rev) = r.merge_phase() else { panic!("{:?}", r.merge_phase()) };
    assert!(rev.fallback);
}

#[test]
fn b6_a_circle_question_this_build_cannot_read_stops_the_merge() {
    let mut r = Rig::new(&[(QA, "One"), (QB, "Two")]);
    let mut odd = qjson(QC, "From the future");
    odd["kind"] = json!({"HotSpot": {"regions": []}});
    r.s.on("Post", &format!("{}/merge", bank_path()), 200, merge_start_json(1, 2, true));
    r.s.keep_raw("Get", &bank_path(), served_raw(2, vec![qjson(QA, "One"), qjson(QB, "Two"), odd]));
    r.m.open_merge(LOCAL, editor());
    r.turns(2);
    let MergePhase::Blocked(why) = r.merge_phase() else { panic!("{:?}", r.merge_phase()) };
    assert!(why.contains("newer verSHion") && why.contains("Update"), "{why}");
    assert_eq!(r.banks[0].questions.len(), 2);
}

#[test]
fn i1_a_purge_held_back_by_a_review_is_in_plain_words() {
    let s = Script::new();
    s.on("Post", &format!("{}/purge", bank_path()), 409, json!({"error": "pending_review", "message": "raw"}));
    let pool = Pool::new(&*s).with_retry(RetryPolicy::none());
    let e = super::transport::block_on(pool.purge_circle_bank(CID, BID)).unwrap_err();
    let msg = e.message();
    assert!(!msg.contains("pending_review") && !msg.contains("refused the request"), "{msg}");
    assert!(msg.contains("reviewing"), "{msg}");
}

#[test]
fn i2_findings_of_a_refused_payload_are_listed_on_the_send_page() {
    let mut r = Rig::new(&[(QA, "One")]);
    r.open_send(editor());
    r.s.on("Put", &bank_path(), 422, json!({"error": "invalid_payload", "errors": [{"code": "too_long", "path": "bank.questions[0].stem", "limit": 8192}]}));
    r.m.send();
    r.turn();
    let SendStage::Payload(p) = r.send_stage() else { panic!("{:?}", r.send_stage()) };
    assert_eq!(p.issues.len(), 1);
    let ctx = egui::Context::default();
    crate::theme::setup(&ctx);
    let t = draw_all(&mut r, &ctx);
    assert!(t.contains("needs you to change the items below") && t.contains("Question 1"), "{t}");
    assert!(!t.contains("listed below"), "no pointer to a list that is not there");
    // Back to the note, nothing lost; the same commit id is not kept for different content.
    r.m.back_to_note();
    assert!(matches!(r.send_stage(), SendStage::Editing));
}

#[test]
fn a_restore_that_comes_back_hidden_does_not_say_done() {
    let mut r = Rig::new(&[(QA, "One")]);
    r.s.on("Get", &format!("{}/versions", bank_path()), 200, page(3, &[3, 2, 1], None));
    r.m.open_history(CID, BID, "Cells", owner());
    r.turn();
    r.m.ask_restore(1);
    r.s.on("Post", &format!("{}/rollback", bank_path()), 200, json!({"circle_id": CID, "bank_id": BID, "version": 4, "version_id": hex(4), "qids": [QA], "added": 0, "changed": 1, "removed": 0, "rollback_of": 1, "held": true, "withheld": true}));
    r.s.on("Get", &format!("{}/versions", bank_path()), 200, page(4, &[4, 3, 2, 1], None));
    r.m.confirm_restore(false);
    r.turn();
    let Some(RestoreStage::Done { version, held, withheld }) = r.history().restore.as_ref().map(|x| &x.stage) else { panic!() };
    let lines = restore_lines(*version, 1, *held, *withheld, true).join(" ");
    assert!(!lines.starts_with("Done") && lines.contains("not shown to the circle") && lines.contains("hidden"), "{lines}");
}

#[test]
fn hidden_advice_is_for_whoever_can_fix_it() {
    let s = SentInfo { version: 2, added: 0, changed: 1, removed: 0, replayed: false, held: true, withheld: true, merged_from: None, left_out_unasked: Vec::new(), not_asked: Vec::new(), hiding_stopped: None, ask_next: false };
    let author = sent_lines(&s, true).join(" ");
    let editor = sent_lines(&s, false).join(" ");
    assert!(author.contains("Change the questions") && !author.contains("student"), "{author}");
    assert!(!editor.contains("Change the questions") && editor.contains("author or the circle's owner") && !editor.contains("student"), "{editor}");
}

#[test]
fn sending_on_top_says_it_replaces_and_shows_the_real_head() {
    let mut r = Rig::new(&[(QA, "One")]);
    r.edit_stem(0, "One, reworded");
    // The copy never heard of version 3: the head number comes from merge-initiate, not a guess.
    let mut start = merge_start_json(1, 3, true);
    start["between"] = json!([
        {"version": 2, "version_id": hex(2), "by": "Ms Frizzle", "at": NOW - 5000, "message": "Fixed it", "added": 0, "changed": 1, "removed": 0, "question_count": 1, "license": "CC-BY-4.0", "kept": true, "withheld": false},
        {"version": 3, "version_id": hex(3), "by": "Ms Frizzle", "at": NOW - 1000, "message": "", "added": 0, "changed": 1, "removed": 0, "question_count": 1, "license": "CC-BY-4.0", "kept": false, "withheld": true}
    ]);
    r.s.on("Post", &format!("{}/merge", bank_path()), 200, start);
    r.s.keep("Get", &bank_path(), 404, json!({"error": "version_withheld"}));
    r.m.open_merge(LOCAL, owner());
    r.turns(2);
    let MergePhase::HeadWithheld { head_version, replaces } = r.merge_phase() else { panic!("{:?}", r.merge_phase()) };
    assert_eq!(*head_version, Some(3));
    assert_eq!(replaces.iter().map(|l| l.version).collect::<Vec<_>>(), vec![2], "the readable version that would be replaced");
    let ctx = egui::Context::default();
    crate::theme::setup(&ctx);
    let t = draw_all(&mut r, &ctx);
    assert!(t.contains("REPLACES") && t.contains("(3)") && t.contains("Version 2 by Ms Frizzle"), "{t}");
    r.m.send_on_top(3, owner());
    let EditView::Send(v) = &r.m.view else { panic!() };
    assert_eq!((v.base_override, v.replaces.len()), (Some(3), 1));
}

#[test]
fn one_unreadable_link_does_not_drop_the_others_and_the_file_is_kept() {
    let dir = std::env::temp_dir().join(format!("vershion-links-{}-a", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let log = LinkLog::Dir(dir.clone());
    let (_, good) = linked(&[(QA, "One")]);
    let mut v = serde_json::to_value(&good).unwrap();
    let broken = json!({"circle_id": CID, "circle_bank_id": BID, "local_bank_id": "x"});
    let list = json!({"links": [broken, v.take()]});
    std::fs::write(dir.join(LINKS_DOC), list.to_string()).unwrap();
    let got = log.load();
    assert_eq!(got.len(), 1, "the good one survives");
    assert_eq!(got[0].local_bank_id, LOCAL);
    let kept = std::fs::read_to_string(dir.join(format!("{LINKS_DOC}.bad"))).unwrap();
    assert!(kept.contains("\"local_bank_id\":\"x\""), "the file as it was is kept beside it");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_crash_while_writing_leaves_the_old_links_and_garbage_is_not_trusted() {
    let dir = std::env::temp_dir().join(format!("vershion-links-{}-b", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let log = LinkLog::Dir(dir.clone());
    let (_, good) = linked(&[(QA, "One")]);
    log.save(std::slice::from_ref(&good)).unwrap();
    // A crash half-way through the next save: the temporary file is half written; the real file was not touched.
    std::fs::write(dir.join(format!("{LINKS_DOC}.tmp")), "{\"links\":[{\"circle_id\":").unwrap();
    assert_eq!(log.load(), vec![good.clone()]);
    // A save writes beside the name and renames into place: no temporary file stays behind.
    let mut changed = good.clone();
    changed.head_seen = 9;
    log.save(&[changed.clone()]).unwrap();
    assert_eq!(log.load(), vec![changed]);
    assert!(!dir.join(format!("{LINKS_DOC}.tmp")).exists());
    // A file that is not JSON at all gives no links and is kept as it was.
    std::fs::write(dir.join(LINKS_DOC), "not json").unwrap();
    assert!(log.load().is_empty());
    assert_eq!(std::fs::read_to_string(dir.join(format!("{LINKS_DOC}.bad"))).unwrap(), "not json");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_question_deleted_here_stays_deleted_in_both_fallback_merges() {
    // No stored snapshot, but the circle still has the base: the question is "deleted by you", not "new in the circle".
    let mut r = Rig::new(&[(QA, "One"), (QB, "Two")]);
    let mut l = r.link();
    l.base.clear();
    r.m.add_link(l).unwrap();
    r.banks[0].questions.remove(1);
    r.banks[0].modified += 1;
    r.s.on("Post", &format!("{}/merge", bank_path()), 200, merge_start_json(1, 2, true));
    r.s.keep_raw("Get", &bank_path(), served(2, &[(QA, "One"), (QB, "Two")]));
    r.s.keep_raw("Get", &format!("{}/versions/1", bank_path()), served(1, &[(QA, "One"), (QB, "Two")]));
    r.m.open_merge(LOCAL, editor());
    r.turns(3);
    let MergePhase::Review(rev) = r.merge_phase() else { panic!("{:?}", r.merge_phase()) };
    assert!(!rev.fallback);
    assert_eq!(rev.plan.count(RowKind::RemovedLocally), 1);
    assert_eq!(rev.plan.count(RowKind::NewInTheirs), 0);

    // The two-way path (the base is gone): the same.
    let mut r = Rig::new(&[(QA, "One"), (QB, "Two")]);
    let mut l = r.link();
    l.base.clear();
    r.m.add_link(l).unwrap();
    r.banks[0].questions.remove(1);
    r.banks[0].modified += 1;
    r.s.on("Post", &format!("{}/merge", bank_path()), 200, merge_start_json(1, 2, false));
    r.s.keep_raw("Get", &bank_path(), served(2, &[(QA, "One"), (QB, "Two")]));
    r.m.open_merge(LOCAL, editor());
    r.turns(2);
    let MergePhase::Review(rev) = r.merge_phase() else { panic!("{:?}", r.merge_phase()) };
    assert!(rev.fallback);
    assert_eq!(rev.plan.count(RowKind::RemovedLocally), 1);
    assert_eq!(rev.plan.count(RowKind::NewInTheirs), 0, "it does not come back by default");
}

// ---- critic 51: units, fingerprints, escapes -------------------------------------------------------------------

/// A working copy made through the real copy flow, then an edit written the way the editor writes it.
fn real_working_copy() -> (CirclesModel, Arc<Script>, Vec<QuestionBank>, super::browse::BrowseModel) {
    let (mut m, s) = open_bank_page("editor");
    let _ = link_log_model(&mut m);
    let mut browse = super::browse::BrowseModel::new(Runner::Inline, Arc::new(Mem::default()), super::browse::ImportLog::memory());
    m.begin_copy();
    m.make_working_copy();
    m.frame(now(), &ready(&s));
    let mut banks = Vec::new();
    m.sync(&mut banks, &mut browse);
    assert_eq!(banks.len(), 1);
    (m, s, banks, browse)
}

#[test]
fn n1_a_real_edit_after_a_real_working_copy_is_not_stale_and_can_be_sent() {
    let (mut m, s, mut banks, mut browse) = real_working_copy();
    let local = banks[0].id.clone();
    // The editor writes seconds.
    banks[0].questions[0].stem = "First, reworded".into();
    banks[0].modified = crate::screens::now_secs();
    s.keep("Post", "/circles/heads", 200, heads_json("CC-BY-4.0", 1));
    let perm = m.perm_for(CID, BID);
    m.edit.open_send(&local, perm);
    m.frame(now(), &ready(&s));
    m.edit.set_attest(true);
    m.sync(&mut banks, &mut browse);
    assert!(!m.edit.status(&local).unwrap().stale, "an honest edit is not a restored file");
    assert_eq!(m.edit.send_blocker(), None);
}

#[test]
fn n1_every_place_that_writes_bank_modified_writes_seconds() {
    // Seconds are about 1.7e9; milliseconds 1.7e12. Anything above 1e11 is a unit mistake.
    const SECONDS_MAX: i64 = 100_000_000_000;
    // A copy made through the real flow.
    let (m, _s, banks, _b) = real_working_copy();
    assert!(banks[0].modified < SECONDS_MAX && banks[0].created < SECONDS_MAX, "copy: {}", banks[0].modified);
    let mut m = m;
    let l = m.edit.link_for_local(&banks[0].id).unwrap();
    assert!(l.bank_modified < SECONDS_MAX, "link: {}", l.bank_modified);

    // A merge applied through the circle model.
    let mut r = Rig::new(&[(QA, "One"), (QB, "Two")]);
    r.banks[0].modified = 1_700_000_000;
    script_merge(&r, &v2(), 2);
    r.m.open_merge(LOCAL, editor());
    r.turns(2);
    r.m.apply_merge();
    r.turns(2);
    assert!(matches!(r.merge_phase(), MergePhase::Applied(_)));
    assert!(r.banks[0].modified < SECONDS_MAX, "merge: {}", r.banks[0].modified);
    assert!(r.link().bank_modified < SECONDS_MAX);

    // A send moves the link on with the bank as sent.
    r.banks[0].modified = 1_700_000_100;
    r.open_send(editor());
    r.s.on("Put", &bank_path(), 201, sent_ok(3, &[QA, QB, QC]));
    r.m.send();
    r.turn();
    assert_eq!(r.link().bank_modified, 1_700_000_100);
}

#[test]
fn n1_a_stale_copy_that_is_level_with_the_circle_can_be_set_in_step_or_forgotten() {
    let mut r = Rig::new(&[(QA, "One"), (QB, "Two")]);
    r.banks[0].modified = 1_700_000_000;
    let mut l = r.link();
    l.bank_modified = 1_700_000_500;
    l.bank_print = "a print of some other content".into();
    r.m.add_link(l).unwrap();
    r.turn();
    assert!(r.m.status(LOCAL).unwrap().stale);
    // "Get latest and merge" finds nothing newer.
    r.s.on("Post", &format!("{}/merge", bank_path()), 400, json!({"error": "invalid_request", "field": "base_version"}));
    r.m.open_merge(LOCAL, editor());
    r.turns(2);
    assert!(matches!(r.merge_phase(), MergePhase::StaleUpToDate { ask: None }), "{:?}", r.merge_phase());
    let ctx = egui::Context::default();
    crate::theme::setup(&ctx);
    let t = draw_all(&mut r, &ctx);
    assert!(t.contains("Treat this copy as in step") && t.contains("Forget this link"), "{t}");
    // One click only asks.
    r.m.rebaseline();
    r.turns(2);
    assert!(matches!(r.merge_phase(), MergePhase::StaleUpToDate { ask: None }), "no second click, no change");
    r.m.ask_stale(StaleAsk::Rebase);
    r.m.cancel_stale_ask();
    r.m.rebaseline();
    r.turns(2);
    assert!(matches!(r.merge_phase(), MergePhase::StaleUpToDate { .. }));
    r.m.ask_stale(StaleAsk::Forget);
    r.m.rebaseline();
    r.turns(2);
    assert!(matches!(r.merge_phase(), MergePhase::StaleUpToDate { ask: Some(StaleAsk::Forget) }), "the wrong question confirms nothing");
    let t = draw_all(&mut r, &ctx);
    assert!(t.contains("Yes, forget the link"), "{t}");
    r.m.cancel_stale_ask();
    r.m.ask_stale(StaleAsk::Rebase);
    let t = draw_all(&mut r, &ctx);
    assert!(t.contains("Are you sure") && t.contains("Yes, treat it as in step"), "{t}");
    r.m.rebaseline();
    r.turns(2);
    assert!(matches!(r.merge_phase(), MergePhase::UpToDate));
    assert!(!r.m.status(LOCAL).unwrap().stale, "cleared");
    assert_eq!(r.banks[0].questions.len(), 2, "the bank was not touched");
    // Sending works again.
    r.m.close();
    r.open_send(editor());
    r.turn();
    assert_eq!(r.m.send_blocker(), None);

    // The other way out: let go of the link.
    let mut l = r.link();
    l.bank_modified = 1_700_000_500;
    l.bank_print = "other".into();
    r.m.add_link(l).unwrap();
    r.turn();
    assert!(r.m.status(LOCAL).unwrap().stale);
    // On the stale screen: a first click asks, only the second forgets.
    r.s.on("Post", &format!("{}/merge", bank_path()), 400, json!({"error": "invalid_request", "field": "base_version"}));
    r.m.open_merge(LOCAL, editor());
    r.turns(2);
    r.m.forget_stale();
    assert!(r.m.link_for_local(LOCAL).is_some(), "not without being asked");
    r.m.ask_stale(StaleAsk::Forget);
    r.m.forget_stale();
    assert!(r.m.link_for_local(LOCAL).is_none());
    assert!(!r.m.active());
}

#[test]
fn a_card_forgets_a_link_only_after_the_second_click() {
    let mut r = Rig::new(&[(QA, "One")]);
    r.m.confirm_forget(LOCAL);
    assert!(r.m.link_for_local(LOCAL).is_some(), "no question asked, nothing forgotten");
    r.m.ask_forget(LOCAL);
    r.m.confirm_forget("another-bank");
    assert!(r.m.link_for_local(LOCAL).is_some(), "an answer for another bank forgets nothing");
    r.m.cancel_forget();
    r.m.confirm_forget(LOCAL);
    assert!(r.m.link_for_local(LOCAL).is_some());
    r.m.ask_forget(LOCAL);
    assert!(r.m.forget_asked(LOCAL));
    r.m.confirm_forget(LOCAL);
    assert!(r.m.link_for_local(LOCAL).is_none());
}

#[test]
fn n1_content_the_app_wrote_is_never_stale_whatever_the_clock_says() {
    let (_, mut l) = linked(&[(QA, "One")]);
    let (bank, _) = linked(&[(QA, "One")]);
    l.mark_bank(&bank);
    let mut older = bank.clone();
    older.modified = 1_000;
    l.bank_modified = 2_000;
    assert!(!l.stale_for(&older), "same content: not a restored file");
    older.questions[0].stem = "different".into();
    assert!(l.stale_for(&older), "older and different: a saved copy was put back");
    // A link written by an earlier build in milliseconds is read as seconds.
    l.bank_modified = 1_792_864_237_304;
    let mut b = bank.clone();
    b.modified = 1_792_864_300;
    b.questions[0].stem = "edited".into();
    assert!(!l.stale_for(&b));
}

#[test]
fn v1_a_licence_tightened_while_the_page_was_open_is_what_the_send_carries() {
    let mut r = Rig::new(&[(QA, "One")]);
    r.edit_stem(0, "One, reworded");
    r.open_send(owner());
    // Another window of the same author tightens the licence after the page checked it.
    r.s.keep("Post", "/circles/heads", 200, heads_json("circle-only", 1));
    r.s.on("Put", &bank_path(), 201, sent_ok(2, &[QA]));
    r.m.send();
    r.turn();
    let (meta, _) = body_parts(&r.puts()[0]);
    assert_eq!(meta["license"], "circle-only", "never the CC-BY the page saw");
    assert_eq!(r.link().license, "circle-only");
}

#[test]
fn v1_if_the_licence_can_not_be_asked_just_before_nothing_is_sent() {
    let mut r = Rig::new(&[(QA, "One")]);
    r.open_send(owner());
    r.s.keep("Post", "/circles/heads", 503, json!({"error": "busy"}));
    r.m.send();
    r.turn();
    assert!(r.puts().is_empty());
    assert!(matches!(r.send_stage(), SendStage::Failed(_)));
}

#[test]
fn v5_a_links_file_that_can_not_be_read_is_never_written_over() {
    let dir = std::env::temp_dir().join(format!("vershion-links-{}-c", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    // A file that is there but cannot be read (a directory stands in for a transient read error).
    std::fs::create_dir_all(dir.join(LINKS_DOC)).unwrap();
    let mut m = CircleEditModel::new(Runner::Inline, Arc::new(Mem::default()), Arc::new(HashMap::<String, Vec<u8>>::new()), Arc::new(MemRestore::default()), LinkLog::Dir(dir.clone()));
    let (_, l1) = linked(&[(QA, "One")]);
    assert!(m.add_link(l1.clone()).is_err(), "the save is refused");
    assert!(dir.join(LINKS_DOC).is_dir(), "nothing was written over it");
    // It becomes readable again, with a link in it: the link added meanwhile is kept, the file's is not lost.
    std::fs::remove_dir_all(dir.join(LINKS_DOC)).unwrap();
    let mut other = l1.clone();
    other.local_bank_id = "local-bank-2".into();
    other.circle_bank_id = "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb1".into();
    LinkLog::Dir(dir.clone()).save(std::slice::from_ref(&other)).unwrap();
    assert_eq!(m.links().len(), 2);
    assert!(m.add_link(l1).is_ok());
    assert_eq!(LinkLog::Dir(dir.clone()).load().len(), 2);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn v5_the_first_bad_copy_is_kept_and_later_ones_are_numbered() {
    let dir = std::env::temp_dir().join(format!("vershion-links-{}-d", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let log = LinkLog::Dir(dir.clone());
    std::fs::write(dir.join(LINKS_DOC), "{\"links\": [{\"x\": 1}]}").unwrap();
    log.load();
    std::fs::write(dir.join(LINKS_DOC), "{\"links\": [{\"x\": 2}]}").unwrap();
    log.load();
    log.load();
    assert!(std::fs::read_to_string(dir.join(format!("{LINKS_DOC}.bad"))).unwrap().contains("\"x\": 1"), "the first is not overwritten");
    assert!(std::fs::read_to_string(dir.join(format!("{LINKS_DOC}.bad.2"))).unwrap().contains("\"x\": 2"));
    assert!(!dir.join(format!("{LINKS_DOC}.bad.3")).exists(), "the same file is not kept twice");
    let _ = std::fs::remove_dir_all(&dir);
}


#[test]
fn r4_an_i64_max_bank_time_does_not_overflow_a_merge() {
    let mut r = Rig::new(&[(QA, "One"), (QB, "Two")]);
    r.banks[0].modified = i64::MAX;
    script_merge(&r, &v2(), 2);
    r.m.open_merge(LOCAL, editor());
    r.turns(2);
    r.m.apply_merge();
    r.turns(2);
    assert!(matches!(r.merge_phase(), MergePhase::Applied(_)));
    assert_eq!(r.banks[0].modified, i64::MAX);
}

#[test]
fn r4_an_editor_who_is_not_the_author_reads_the_right_words() {
    let author = attest_statement(true, Some(License::CcBy));
    assert!(author.contains("the licence I chose"));
    let editor = attest_statement(false, Some(License::CcBy));
    assert!(!editor.contains("the licence I chose"), "{editor}");
    assert!(editor.contains("I wrote these changes") && editor.contains("set by its author") && editor.contains("CC BY 4.0") && !editor.contains("credit me"), "{editor}");
    let line = licence_line(false, License::CcBy);
    assert!(line.contains("set by the bank's author") && !line.contains("credit me"), "{line}");
    assert!(licence_line(true, License::CcBy).contains("credit me"));
    // On the page, for an editor who is not the author, and the semantic (own work) is unchanged.
    let mut r = Rig::new(&[(QA, "One")]);
    r.edit_stem(0, "One, reworded");
    r.open_send(editor_perm());
    let ctx = egui::Context::default();
    crate::theme::setup(&ctx);
    let t = draw_all(&mut r, &ctx);
    assert!(t.contains("set by its author") && !t.contains("the licence I chose"), "{t}");
    r.s.on("Put", &bank_path(), 201, sent_ok(2, &[QA]));
    r.m.send();
    r.turn();
    assert_eq!(body_parts(&r.puts()[0]).0["attest"]["own_work"], true);
}

fn editor_perm() -> Perm {
    editor()
}

#[test]
fn r4_the_pending_review_message_names_the_bank() {
    let (mut m, s) = open_circle("owner", json!({"banks": [removed_bank("owner_removed", false, json!(NOW + DAY))]}));
    s.act("Post", &format!("/banks/{BID}/purge"), 409, json!({"error": "pending_review", "message": "raw"}));
    m.purge_bank(BID);
    m.frame(now(), &ready(&s));
    let p = m.circle.as_ref().unwrap().act_problem.as_ref().unwrap().message.clone();
    assert!(p.contains("\"Cells\"") && p.contains("reviewing") && !p.contains("pending_review"), "{p}");
}

#[test]
fn r4_a_removed_bank_offers_no_send() {
    let (mut m, _s) = open_circle("editor", json!({"banks": [removed_bank("owner_removed", false, json!(NOW + DAY))]}));
    let (_, l) = linked(&[(QA, "One")]);
    m.edit.add_link(l).unwrap();
    let ctx = egui::Context::default();
    crate::theme::setup(&ctx);
    let mut seen = String::new();
    let out = ctx.run_ui(
        egui::RawInput { screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(1000.0, 1600.0))), time: Some(0.0), ..Default::default() },
        |ui| {
            egui::CentralPanel::default().show(ui, |ui| {
                super::circles_ui::body_of(ui, &mut m);
            });
        },
    );
    for sh in &out.shapes {
        if let egui::epaint::Shape::Text(t) = &sh.shape {
            seen.push_str(t.galley.text());
            seen.push('\n');
        }
    }
    out.drop_without_applying_deltas();
    assert!(seen.contains("was removed from the circle, so nothing can be sent"), "{seen}");
    // The state the buttons read is the same one the page tests.
    assert!(m.circle.as_ref().and_then(|c| c.detail.as_ref()).unwrap().banks.iter().any(|b| b.id == BID && b.removed.is_some()));
}

#[test]
fn r4_a_bank_time_in_milliseconds_from_an_older_build_is_read_as_seconds() {
    let (bank, mut l) = linked(&[(QA, "One")]);
    let mut b = bank.clone();
    b.questions[0].stem = "Content of an older state".into();
    l.bank_modified = 1_792_864_237; // seconds
    l.bank_print = "something else".into();
    b.modified = 1_792_864_100_000; // milliseconds, older than the link
    assert!(l.stale_for(&b), "an older bank is found even when it was written in ms");
    b.modified = 1_792_864_300_000; // milliseconds, newer
    assert!(!l.stale_for(&b));
}

#[test]
fn r4_the_same_circle_bank_is_one_link_when_the_file_is_read_again() {
    let dir = std::env::temp_dir().join(format!("vershion-links-{}-e", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::create_dir_all(dir.join(LINKS_DOC)).unwrap();
    let mut m = CircleEditModel::new(Runner::Inline, Arc::new(Mem::default()), Arc::new(HashMap::<String, Vec<u8>>::new()), Arc::new(MemRestore::default()), LinkLog::Dir(dir.clone()));
    let (_, a) = linked(&[(QA, "One")]);
    assert!(m.add_link(a.clone()).is_err());
    std::fs::remove_dir_all(dir.join(LINKS_DOC)).unwrap();
    let mut b = a.clone();
    b.local_bank_id = "other-local".into(); // the same circle bank under another local id
    LinkLog::Dir(dir.clone()).save(&[b]).unwrap();
    assert_eq!(m.links().len(), 1, "one link per circle bank");
    assert_eq!(m.links()[0].local_bank_id, LOCAL, "the one added meanwhile wins");
    let _ = std::fs::remove_dir_all(&dir);
}

// ---- phase C: sends to a bank that hides its keys, keyless copies, evidence kept --------------------------------

#[test]
fn c_a_copy_without_keys_is_never_sent_and_says_what_to_do() {
    let mut r = Rig::new(&[(QA, "One"), (QB, "Two")]);
    r.banks[0].questions[1].explanation = Some(crate::model::KEYLESS_SENTINEL.into());
    r.banks[0].modified += 1;
    r.open_send(editor());
    r.m.send();
    r.turn();
    assert!(r.puts().is_empty(), "nothing goes to the circle");
    let SendStage::Keyless { positions, kinds } = r.send_stage() else { panic!("{:?}", r.send_stage()) };
    assert_eq!((positions.clone(), kinds.clone()), (vec![1], vec!["Mc".to_string()]));
    let t = keyless_text(positions, kinds);
    assert!(t.contains("question 2") && t.contains("mark the correct answer") && t.contains("editor"), "{t}");
    r.m.back_to_note();
    assert!(matches!(r.send_stage(), SendStage::Editing));
}

#[test]
fn c_the_pools_keyless_refusal_is_said_with_its_questions() {
    let mut r = Rig::new(&[(QA, "One"), (QB, "Two")]);
    r.edit_stem(0, "One, reworded");
    r.open_send(editor());
    r.s.on("Put", &bank_path(), 422, json!({"error": "keyless_question", "positions": [0, 1], "kinds": ["Short", "Tf"], "message": "RAWSERVER"}));
    r.m.send();
    r.turn();
    let SendStage::Keyless { positions, kinds } = r.send_stage() else { panic!("{:?}", r.send_stage()) };
    let t = keyless_text(positions, kinds);
    assert!(t.contains("questions 1 and 2") && t.contains("sample answer") && !t.contains("RAWSERVER"), "{t}");
}

#[test]
fn c_a_setting_changed_meanwhile_is_sent_again_with_nothing_stored() {
    let mut r = Rig::new(&[(QA, "One")]);
    r.edit_stem(0, "One, reworded");
    r.open_send(editor());
    r.s.on("Put", &bank_path(), 409, json!({"error": "hide_key_changed", "hide_key": true, "head_version": 1, "message": "RAWSERVER"}));
    r.m.send();
    r.turn();
    let SendStage::Refused { message, merge_again } = r.send_stage() else { panic!("{:?}", r.send_stage()) };
    assert!(!merge_again, "not a merge: the head did not move");
    assert!(message.contains("nothing was stored") && message.contains("Send it again") && !message.contains("RAWSERVER"), "{message}");
    assert_eq!(r.link().base_version, 1, "the link did not move");
    r.m.back_to_note();
    r.s.on("Put", &bank_path(), 201, sent_ok(2, &[QA]));
    r.m.send();
    r.turn();
    assert_eq!(r.puts().len(), 2);
    assert!(matches!(r.send_stage(), SendStage::Done(_)));
}

#[test]
fn c_a_send_to_a_hiding_bank_asks_for_decisions_and_carries_them() {
    let mut r = Rig::new(&[(QA, "One"), (QB, "Two")]);
    r.edit_stem(0, "One, reworded");
    r.open_send(editor());
    r.s.on("Put", &bank_path(), 422, json!({"error": "labels_need_decision", "errors": [{"code": "labels_need_decision", "path": "bank.questions[1]"}], "label_findings": [{"question": 1, "strip_allowed": true, "choices": [{"before": "A) x", "after": "x"}]}]}));
    r.m.send();
    r.turn();
    let first_id = body_parts(&r.puts()[0]).0["commit_id"].clone();
    assert!(body_parts(&r.puts()[0]).0.get("labels").is_none());
    let SendStage::Decide(d) = r.send_stage() else { panic!("{:?}", r.send_stage()) };
    assert!(!d.ready());
    r.m.send_with_decisions();
    r.turn();
    assert_eq!(r.puts().len(), 1, "nothing goes while a decision is open");
    r.m.send_decide_label(Pos::question(1), LabelChoice::Strip);
    r.s.on("Put", &bank_path(), 201, json!({"circle_id": CID, "bank_id": BID, "version": 2, "version_id": hex(2), "qids": [QA, QB], "added": 0, "changed": 1, "removed": 0, "keys_hidden": true}));
    r.m.send_with_decisions();
    r.m.send_with_decisions();
    r.turn();
    let puts = r.puts();
    assert_eq!(puts.len(), 2, "one more send, not two");
    let (meta, _) = body_parts(&puts[1]);
    assert_eq!(meta["labels"], json!([{"question": 1, "choice": "strip"}]));
    assert_ne!(meta["commit_id"], first_id, "different content, a new send id");
    assert!(matches!(r.send_stage(), SendStage::Done(_)));
}

#[test]
fn c_a_viewer_is_not_offered_again_a_version_without_a_viewers_copy() {
    // (critic 58 I3: in a bank that hides its keys, a version from before hiding may have no viewers' copy and a
    // "Look at it" on it was a dead end; the history line does not say which, so each refusal is kept. Critic 59 I1:
    // per version only: which versions have a viewers' copy is not ordered, an older one may well have one)
    let mut r = Rig::new(&[(QA, "One")]);
    r.s.on("Get", &format!("{}/versions", bank_path()), 200, page(3, &[3, 2, 1], None));
    r.m.open_history(CID, BID, "Cells", viewer());
    r.turn();
    let gets = |r: &Rig, n: u64| r.s.seen().iter().filter(|x| x.method == Method::Get && x.path == format!("{}/versions/{n}", bank_path())).count();
    r.s.on("Get", &format!("{}/versions/2", bank_path()), 409, json!({"error": "keys_hidden_unavailable", "message": "RAWSERVER"}));
    r.m.view_version(2);
    r.turn();
    assert_eq!(gets(&r, 2), 1);
    r.m.back();
    let h = r.history();
    assert!(!h.can_look(&h.lines[1]), "version 2 has no viewers' copy");
    assert!(h.can_look(&h.lines[0]) && h.can_look(&h.lines[2]), "the newest and the older one still open");
    let ctx = egui::Context::default();
    crate::theme::setup(&ctx);
    let t = draw_all(&mut r, &ctx);
    let closed = "This version has no copy without answer keys, so you can not open it.";
    assert!(t.contains(closed), "{t}");
    for card in t.split("\nVersion ").skip(1) {
        assert_eq!(card.contains(closed), card.starts_with("2\n"), "on version 2 only: {card}");
    }
    assert!(!t.contains("viewers can not open it") && !t.contains("RAWSERVER"), "{t}");
    r.m.view_version(2);
    r.turn();
    assert!(matches!(r.m.view, EditView::History(_)));
    assert_eq!(gets(&r, 2), 1, "no second dead end");
    // the older version is still offered; refused too, it is closed on its own, and the newest stays open
    r.s.on("Get", &format!("{}/versions/1", bank_path()), 409, json!({"error": "keys_hidden_unavailable"}));
    r.m.view_version(1);
    assert!(matches!(r.m.view, EditView::Version(_)), "version 1 is still offered");
    r.turn();
    r.m.back();
    let h = r.history();
    assert!(!h.can_look(&h.lines[1]) && !h.can_look(&h.lines[2]) && h.can_look(&h.lines[0]));
    assert_eq!(gets(&r, 1), 1);
    r.s.on("Get", &format!("{}/versions/3", bank_path()), 200, json!({"error": "x"}));
    r.m.view_version(3);
    assert!(matches!(r.m.view, EditView::Version(_)), "the newest is still offered");
}

#[test]
fn c_send_decisions_go_only_with_the_content_they_were_made_for() {
    // (critic 57 I1: a decision about question 2 went out after question 2 was rewritten into another question)
    let setup = || {
        let mut r = Rig::new(&[(QA, "One"), (QB, "Two")]);
        r.edit_stem(0, "One, reworded");
        r.open_send(editor());
        r.s.on("Put", &bank_path(), 422, json!({"error": "labels_need_decision", "errors": [{"code": "labels_need_decision", "path": "labels[question=1]"}], "label_findings": [{"question": 1, "strip_allowed": true}]}));
        r.m.send();
        r.turn();
        r.m.send_decide_label(Pos::question(1), LabelChoice::Keep);
        r.s.on("Put", &bank_path(), 409, json!({"error": "hide_key_changed", "hide_key": true, "head_version": 1, "message": "x"}));
        r.m.send_with_decisions();
        r.turn();
        assert_eq!(body_parts(r.puts().last().unwrap()).0["labels"], json!([{"question": 1, "choice": "keep"}]));
        assert!(matches!(r.send_stage(), SendStage::Refused { .. }));
        r
    };
    let ok = || json!({"circle_id": CID, "bank_id": BID, "version": 2, "version_id": hex(2), "qids": [QA, QB], "added": 0, "changed": 2, "removed": 0, "keys_hidden": true});
    // the bank is changed, then sent again straight from the refusal: the decisions do not go
    let mut r = setup();
    r.edit_stem(1, "A different question now");
    r.s.on("Put", &bank_path(), 201, ok());
    r.m.send();
    r.turn();
    let (meta, _) = body_parts(r.puts().last().unwrap());
    assert!(meta.get("labels").is_none(), "a decision made for the old question 2 went with the new one: {meta}");
    // the same content sent again keeps them (nothing was decided for anything else)
    let mut r = setup();
    r.s.on("Put", &bank_path(), 201, ok());
    r.m.send();
    r.turn();
    assert_eq!(body_parts(r.puts().last().unwrap()).0["labels"], json!([{"question": 1, "choice": "keep"}]));
    // back to the note (where the bank may be changed) drops them, whether or not it was changed
    let mut r = setup();
    r.m.back_to_note();
    r.edit_stem(1, "A different question now");
    r.s.on("Put", &bank_path(), 201, ok());
    r.m.send();
    r.turn();
    assert!(body_parts(r.puts().last().unwrap()).0.get("labels").is_none());
    let mut r = setup();
    r.m.back_to_note();
    r.s.on("Put", &bank_path(), 201, ok());
    r.m.send();
    r.turn();
    assert!(body_parts(r.puts().last().unwrap()).0.get("labels").is_none());
}

#[test]
fn c_a_restore_meeting_a_changed_setting_is_plain() {
    let mut r = Rig::new(&[(QA, "One")]);
    r.s.on("Get", &format!("{}/versions", bank_path()), 200, page(3, &[3, 2, 1], None));
    r.m.open_history(CID, BID, "Cells", owner());
    r.turn();
    r.m.ask_restore(1);
    r.s.on("Post", &format!("{}/rollback", bank_path()), 409, json!({"error": "hide_key_changed", "hide_key": true, "head_version": 3, "message": "RAWSERVER send"}));
    r.m.confirm_restore(false);
    r.turn();
    let Some(RestoreStage::Failed(p)) = r.history().restore.as_ref().map(|x| &x.stage) else { panic!() };
    assert!(p.message.contains("Restore again") && !p.message.contains("RAWSERVER"), "{}", p.message);
}

#[test]
fn c_a_trim_says_what_it_could_not_delete() {
    let mut r = Rig::new(&[(QA, "One")]);
    r.s.on("Get", &format!("{}/versions", bank_path()), 200, page(9, &[9, 8], None));
    r.m.open_history(CID, BID, "Cells", owner());
    r.turn();
    r.m.open_trim();
    r.m.set_trim_keep(1);
    r.m.ask_trim();
    r.s.on("Post", &format!("{}/trim", bank_path()), 200, json!({"ok": true, "pruned": 3, "evidence_kept": 5, "oldest_kept": 9, "pinned_versions": [1, 2, 3, 4, 5]}));
    r.s.on("Get", &format!("{}/versions", bank_path()), 200, page(9, &[9], None));
    r.m.confirm_trim();
    r.turn();
    let Some(TrimStage::Done(t)) = r.history().trim.as_ref().map(|x| &x.stage) else { panic!() };
    assert_eq!(t.evidence_kept, 5);
    let lines = trim_lines(t).join(" ");
    assert!(lines.contains("3 older versions") && lines.contains("5 versions") && lines.contains("only an administrator"), "{lines}");
    // an answer without the count reads the pinned list; a null oldest is no oldest
    let s = Script::new();
    s.on("Post", &format!("{}/trim", bank_path()), 200, json!({"ok": true, "pruned": 0, "oldest_kept": null, "pinned_versions": [2]}));
    let t = super::transport::block_on(async { Pool::new(s.as_ref()).trim_circle_bank(CID, BID, 1).await }).unwrap();
    assert_eq!((t.evidence_kept, t.oldest_kept), (1, None));
    assert!(trim_lines(&t)[0].starts_with("Nothing was deleted"));
}

#[test]
fn c_a_review_copy_kept_as_fingerprints_is_said_so_and_never_as_trimmed() {
    let line = |v: Value| serde_json::from_value::<super::circle_versions_api::VersionLine>(v).unwrap();
    let l = line(json!({"version": 4, "by": "Mr Keesler", "at": NOW, "kept": false, "withheld": true, "review_copy": "hashes_only"}));
    let f = version_flags(&l, &[]);
    assert!(f.contains(&"Kept for review as fingerprints only") && !f.contains(&"No longer kept"), "{f:?}");
    let l = line(json!({"version": 4, "kept": false, "withheld": true, "review_copy": "full"}));
    assert_eq!(version_flags(&l, &[]), vec!["Hidden while it is reviewed"]);
    // nulls are read as nothing, line by line
    let l = line(json!({"version": 4, "kept": null, "withheld": null, "review_copy": null, "by": null, "message": null}));
    assert!(!l.kept && !l.withheld && !l.fingerprints_only());
    let s = Script::new();
    s.on("Get", &format!("{}/versions", bank_path()), 200, json!({"bank_id": BID, "head_version": 4, "versions": [{"version": 4, "kept": null, "withheld": true, "review_copy": "hashes_only"}], "next_before": null}));
    let p = super::transport::block_on(async { Pool::new(s.as_ref()).circle_versions(CID, BID, None).await }).expect("a null flag does not refuse the page");
    assert!(p.versions[0].fingerprints_only());
}

#[test]
fn c_the_new_send_states_draw_wide_and_narrow() {
    let ctx = egui::Context::default();
    crate::theme::setup(&ctx);
    let mut r = Rig::new(&[(QA, "One"), (QB, "Two")]);
    r.edit_stem(0, "One, reworded");
    r.open_send(editor());
    r.s.on("Put", &bank_path(), 422, json!({"error": "keyless_question", "positions": [0, 1], "kinds": ["Short", "Mc"]}));
    r.m.send();
    r.turn();
    let t = draw_all(&mut r, &ctx);
    assert!(t.contains("no answer keys") && t.contains("Back to the note"), "{t}");
    r.m.back_to_note();
    r.s.on("Put", &bank_path(), 422, json!({"error": "order_ack_required", "errors": [{"code": "order_ack_required", "path": "bank.questions[0]"}], "needs_order_ack": [{"question": 0}, {"question": 1}], "order_reasons": [{"question": 0, "reason": "few_unlocked"}]}));
    r.m.send();
    r.turn();
    let t = draw_all(&mut r, &ctx);
    assert!(t.contains("hides its answer keys") && t.contains("Send with these decisions") && t.contains("Confirm all"), "{t}");
    // the history with a fingerprints-only line and a trim that kept evidence
    let mut r = Rig::new(&[(QA, "One")]);
    r.s.on("Get", &format!("{}/versions", bank_path()), 200, json!({"bank_id": BID, "head_version": 3, "oldest_kept": 2, "pinned_versions": [2], "versions": [
        {"version": 3, "by": "Mr Keesler", "at": NOW, "kept": true, "withheld": false},
        {"version": 2, "by": "Mr Keesler", "at": NOW - DAY, "kept": false, "withheld": true, "review_copy": "hashes_only"},
        {"version": 1, "by": "Ms Frizzle", "at": NOW - 2 * DAY, "kept": false, "withheld": false}
    ], "next_before": null}));
    r.m.open_history(CID, BID, "Cells", owner());
    r.turn();
    let t = draw_all(&mut r, &ctx);
    assert!(t.contains("fingerprints only"), "{t}");
    r.m.open_trim();
    r.m.ask_trim();
    r.s.on("Post", &format!("{}/trim", bank_path()), 200, json!({"ok": true, "pruned": 0, "evidence_kept": 1, "pinned_versions": [2]}));
    r.s.keep("Get", &format!("{}/versions", bank_path()), 200, page(3, &[3], None));
    r.m.confirm_trim();
    r.turn();
    let t = draw_all(&mut r, &ctx);
    assert!(t.contains("only an administrator can free that space"), "{t}");
}

#[test]
fn c_a_viewers_look_at_a_version_says_the_keys_are_hidden() {
    let mut r = Rig::new(&[(QA, "One")]);
    r.s.on("Get", &format!("{}/versions", bank_path()), 200, page(2, &[2, 1], None));
    r.m.open_history(CID, BID, "Cells", viewer());
    r.turn();
    let raw = String::from_utf8(served(1, &[(QA, "One")])).unwrap().replacen("\"held\"", "\"x\"", 1).replacen("\"version\":1,", "\"version\":1,\"hide_key\":true,\"keys_hidden\":true,", 1);
    r.s.keep_raw("Get", &format!("{}/versions/1", bank_path()), raw.into_bytes());
    r.m.view_version(1);
    r.turn();
    let EditView::Version(v) = &r.m.view else { panic!() };
    let VersionLoad::Ready(b) = &v.load else { panic!("{:?}", v.load) };
    assert!(b.keys_hidden && b.hide_key);
    // a viewer refused a version without recorded decisions reads it in plain words
    let mut r = Rig::new(&[(QA, "One")]);
    r.s.on("Get", &format!("{}/versions", bank_path()), 200, page(2, &[2, 1], None));
    r.m.open_history(CID, BID, "Cells", viewer());
    r.turn();
    r.s.on("Get", &format!("{}/versions/1", bank_path()), 409, json!({"error": "keys_hidden_unavailable", "message": "RAWSERVER"}));
    r.m.view_version(1);
    r.turn();
    let EditView::Version(v) = &r.m.view else { panic!() };
    let VersionLoad::Failed(p) = &v.load else { panic!("{:?}", v.load) };
    assert!(p.message.contains("can not be shown without them") && !p.message.contains("RAWSERVER"), "{}", p.message);
}

// ---- critic 61 I4: a sign-out (or another account) while a send or a restore is on its way ----

#[test]
fn c61_a_send_that_went_through_before_a_sign_out_still_moves_the_link_on() {
    for another in [false, true] {
        let mut r = Rig::new(&[(QA, "One"), (QB, "Two")]);
        r.edit_stem(0, "One, reworded");
        r.open_send(editor());
        r.turn();
        r.s.on("Put", &bank_path(), 201, sent_ok(2, &[QA, QB]));
        r.m.send();
        r.frame();
        let _ = r.m.sync(&mut r.banks); // the PUT goes out here (the pool commits version 2); its answer is not read yet
        assert_eq!(r.puts().len(), 1, "the send reached the pool");
        // The person signs out (or another account signs in) before the answer is read.
        r.m.forget_account(another);
        assert!(matches!(r.m.view, EditView::Closed), "the screens close");
        r.turns(2);
        assert_eq!(r.link().base_version, 2, "the send that went through moved this computer's link on (another: {another})");
        assert_eq!(r.m.status(LOCAL).map(|s| s.changes.total()), Some(0), "nothing shows as unsent");
        assert!(r.m.notice.is_none());
        // Nothing is sent again.
        r.open_send(editor());
        r.turns(2);
        assert_eq!(r.puts().len(), 1);
    }
}

#[test]
fn c61_the_same_account_back_retries_a_lost_send_with_its_commit_id_and_another_account_does_not() {
    for another in [false, true] {
        let mut r = Rig::new(&[(QA, "One")]);
        r.edit_stem(0, "One, reworded");
        r.open_send(editor());
        r.turn();
        r.s.on("Post", "/circles/heads", 200, heads_json("CC-BY-4.0", 1));
        r.s.fail(ApiError::SignedOut);
        r.m.send();
        r.frame();
        let _ = r.m.sync(&mut r.banks); // the PUT went out; its answer (the sign-in ended) is not read yet
        r.m.forget_account(another);
        r.turns(2);
        assert_eq!(r.link().base_version, 1);
        r.open_send(editor());
        r.turn();
        r.s.on("Put", &bank_path(), 201, sent_ok(2, &[QA]));
        r.m.send();
        r.turns(2);
        let puts = r.puts();
        assert_eq!(puts.len(), 2);
        let same = body_parts(&puts[0]).0["commit_id"] == body_parts(&puts[1]).0["commit_id"];
        assert_eq!(same, !another, "the same account retries the same commit (the pool replays it); another account sends its own");
        assert_eq!(r.link().base_version, 2);
    }
}

#[test]
fn c61_the_same_account_back_retries_a_lost_restore_with_its_commit_id_and_another_account_does_not() {
    for another in [false, true] {
        let mut r = Rig::new(&[(QA, "One")]);
        r.s.on("Get", &format!("{}/versions", bank_path()), 200, page(3, &[3, 2, 1], None));
        r.m.open_history(CID, BID, "Cells", owner());
        r.turn();
        r.m.ask_restore(1);
        r.s.fail(ApiError::SignedOut);
        r.m.confirm_restore(false); // the POST went out; its answer is not read yet
        r.m.forget_account(another);
        r.turns(2);
        r.s.on("Get", &format!("{}/versions", bank_path()), 200, page(3, &[3, 2, 1], None));
        r.m.open_history(CID, BID, "Cells", owner());
        r.turn();
        r.m.ask_restore(1);
        r.s.on("Post", &format!("{}/rollback", bank_path()), 200, json!({"circle_id": CID, "bank_id": BID, "version": 4, "version_id": hex(4), "qids": [QA], "added": 0, "changed": 1, "removed": 0, "rollback_of": 1}));
        r.s.on("Get", &format!("{}/versions", bank_path()), 200, page(4, &[4, 3, 2, 1], None));
        r.m.confirm_restore(false);
        r.turn();
        let posts: Vec<_> = r.s.seen().into_iter().filter(|x| x.path.ends_with("/rollback")).collect();
        assert_eq!(posts.len(), 2);
        let same = posts[0].json()["commit_id"] == posts[1].json()["commit_id"];
        assert_eq!(same, !another, "another: {another}");
    }
}

// ---- "keep this picture" on a send to a bank that hides its keys --------------------------------------------------

const PIC: &[u8] = b"a tiny picture";

/// A working copy whose question 2 (position 1) has a picture, its bytes on this computer and the pool taking them.
fn pictured_rig(hiding: Option<bool>) -> Rig {
    let s = Script::new();
    let log = LinkLog::memory();
    let restores = Arc::new(MemRestore::default());
    let sha = sha256_hex(PIC);
    let file = format!("media/{sha}.png");
    let media: SharedMedia = Arc::new(HashMap::from([(file.clone(), PIC.to_vec())]));
    s.keep("Post", "/circles/heads", 200, heads_json("CC-BY-4.0", 1));
    s.keep("Put", &format!("/blobs/{sha}"), 200, json!({"sha256": sha, "size": PIC.len(), "width": 4, "height": 4}));
    let mut m = CircleEditModel::new(Runner::Inline, Arc::new(Mem::default()), media, restores.clone(), log.clone()).with_retry(RetryPolicy::none());
    m.set_clock(NOW);
    let (mut bank, link) = linked(&[(QA, "One"), (QB, "Two")]);
    bank.questions[1].image = Some(crate::model::ImageRef { file, width: 4, height: 4 });
    bank.questions[1].stem = "Two, with a picture".into();
    bank.modified += 1;
    m.add_link(link).unwrap();
    if let Some(h) = hiding {
        m.note_hiding(BID, h);
    }
    let mut r = Rig { m, s, log, restores, banks: vec![bank] };
    r.frame();
    r
}

/// The sends of the bank (not the pictures' uploads, which are PUTs too).
fn sends(r: &Rig) -> Vec<Sent> {
    r.puts().into_iter().filter(|x| x.path == bank_path()).collect()
}

fn sent_hidden(version: u64) -> Value {
    json!({"circle_id": CID, "bank_id": BID, "version": version, "version_id": hex(version), "qids": [QA, QB], "added": 0, "changed": 1, "removed": 0, "keys_hidden": true})
}

fn picture_stage(r: &Rig) -> &super::circle_hide::Pictures {
    match r.send_stage() {
        SendStage::Pictures(p) => p,
        other => panic!("not the picture step: {other:?}"),
    }
}

#[test]
fn pictures_a_send_to_a_hiding_bank_asks_which_pictures_viewers_get() {
    let mut r = pictured_rig(Some(true));
    r.open_send(editor());
    r.m.send();
    r.turn();
    assert!(sends(&r).is_empty(), "nothing is sent before the pictures are decided");
    let p = picture_stage(&r);
    assert_eq!(p.list.iter().map(|x| x.question).collect::<Vec<_>>(), vec![1]);
    assert!(p.image_ok().is_empty());
    r.m.send_keep_picture(1, true);
    r.m.send_keep_picture(0, true); // no picture there
    r.s.on("Put", &bank_path(), 201, sent_hidden(2));
    r.m.send_with_pictures();
    r.m.send_with_pictures();
    r.turn();
    let puts = sends(&r);
    assert_eq!(puts.len(), 1, "one send");
    assert_eq!(body_parts(&puts[0]).0["image_ok"], json!([1]));
    let SendStage::Done(d) = r.send_stage() else { panic!("{:?}", r.send_stage()) };
    assert!(d.left_out_unasked.is_empty());
    // none kept: no image_ok, and the dropped picture is not said as a surprise
    let mut r = pictured_rig(Some(true));
    r.open_send(editor());
    r.m.send();
    r.turn();
    r.s.on("Put", &bank_path(), 201, sent_hidden(2));
    r.m.send_with_pictures();
    r.turn();
    assert!(body_parts(&sends(&r)[0]).0.get("image_ok").is_none());
    let SendStage::Done(d) = r.send_stage() else { panic!("{:?}", r.send_stage()) };
    assert!(d.left_out_unasked.is_empty());
}

#[test]
fn pictures_no_step_for_a_bank_that_shows_its_keys_or_a_bank_without_pictures() {
    let mut r = pictured_rig(Some(false));
    r.open_send(editor());
    r.s.on("Put", &bank_path(), 201, sent_ok(2, &[QA, QB]));
    r.m.send();
    r.turn();
    assert_eq!(sends(&r).len(), 1, "sent straight away");
    assert!(body_parts(&sends(&r)[0]).0.get("image_ok").is_none());
    assert!(matches!(r.send_stage(), SendStage::Done(_)));
    let mut r = Rig::new(&[(QA, "One"), (QB, "Two")]);
    r.m.note_hiding(BID, true);
    r.edit_stem(0, "One, reworded");
    r.open_send(editor());
    r.s.on("Put", &bank_path(), 201, sent_hidden(2));
    r.m.send();
    r.turn();
    assert_eq!(sends(&r).len(), 1, "no pictures: no step");
}

#[test]
fn pictures_kept_go_with_the_pools_decisions_too() {
    let mut r = pictured_rig(Some(true));
    r.open_send(editor());
    r.m.send();
    r.turn();
    r.m.send_keep_picture(1, true);
    r.s.on("Put", &bank_path(), 422, json!({"error": "labels_need_decision", "errors": [], "label_findings": [{"question": 0, "strip_allowed": true}]}));
    r.m.send_with_pictures();
    r.turn();
    assert_eq!(body_parts(&sends(&r)[0]).0["image_ok"], json!([1]));
    let SendStage::Decide(_) = r.send_stage() else { panic!("{:?}", r.send_stage()) };
    r.m.send_decide_label(Pos::question(0), LabelChoice::Strip);
    r.s.on("Put", &bank_path(), 201, sent_hidden(2));
    r.m.send_with_decisions();
    r.turn();
    let (meta, _) = body_parts(&sends(&r)[1]);
    assert_eq!(meta["labels"], json!([{"question": 0, "choice": "strip"}]));
    assert_eq!(meta["image_ok"], json!([1]), "the pictures kept go again");
    let SendStage::Done(d) = r.send_stage() else { panic!("{:?}", r.send_stage()) };
    assert!(d.left_out_unasked.is_empty());
}

#[test]
fn pictures_changed_content_is_asked_again() {
    let mut r = pictured_rig(Some(true));
    r.open_send(editor());
    r.m.send();
    r.turn();
    r.m.send_keep_picture(1, true);
    // (a sign-in that ends keeps the choice: see below) then the bank is changed
    r.s.fail(ApiError::SignedOut);
    r.m.send_with_pictures();
    r.turn();
    assert!(matches!(r.send_stage(), SendStage::Editing));
    r.edit_stem(1, "Two, rewritten");
    r.m.send();
    r.turn();
    assert_eq!(sends(&r).len(), 0, "the signed-out try never reached the pool");
    let p = picture_stage(&r);
    assert!(p.image_ok().is_empty(), "asked again, from nothing, for the changed content");
    assert_eq!(p.list[0].stem, "Two, rewritten");
    // back to the note drops them, as it drops the other decisions
    let mut r = pictured_rig(Some(true));
    r.open_send(editor());
    r.m.send();
    r.turn();
    r.m.send_keep_picture(1, true);
    r.m.back_to_note();
    r.m.send();
    r.turn();
    assert!(picture_stage(&r).image_ok().is_empty());
}

#[test]
fn pictures_a_failure_keeps_the_choices_and_try_again_sends_them() {
    let mut r = pictured_rig(Some(true));
    r.open_send(editor());
    r.m.send();
    r.turn();
    r.m.send_keep_picture(1, true);
    r.s.on("Put", &bank_path(), 503, json!({"error": "busy"}));
    r.m.send_with_pictures();
    r.turn();
    let SendStage::Failed(p) = r.send_stage() else { panic!("{:?}", r.send_stage()) };
    assert!(p.retry);
    r.s.on("Put", &bank_path(), 201, sent_hidden(2));
    r.m.retry_act();
    r.turn();
    let puts = sends(&r);
    assert_eq!(puts.len(), 2, "Try again sends, without asking again");
    assert_eq!(body_parts(&puts[1]).0["image_ok"], json!([1]));
    assert!(matches!(r.send_stage(), SendStage::Done(_)));
}

#[test]
fn pictures_a_sign_in_that_ends_keeps_the_choices() {
    let mut r = pictured_rig(Some(true));
    r.open_send(editor());
    r.m.send();
    r.turn();
    r.m.send_keep_picture(1, true);
    r.s.fail(ApiError::SignedOut);
    r.m.send_with_pictures();
    r.turn();
    assert!(matches!(r.send_stage(), SendStage::Editing), "{:?}", r.send_stage());
    r.s.on("Put", &bank_path(), 201, sent_hidden(2));
    r.m.send();
    r.turn();
    let puts = sends(&r);
    assert_eq!(body_parts(puts.last().unwrap()).0["image_ok"], json!([1]), "not asked again: the choice is kept");
}

#[test]
fn pictures_a_restart_drops_the_kept_pictures_and_the_result_says_so() {
    let mut r = pictured_rig(Some(true));
    r.open_send(editor());
    r.m.send();
    r.turn();
    r.m.send_keep_picture(1, true);
    r.s.on("Put", &bank_path(), 422, json!({"error": "labels_need_decision", "errors": [], "label_findings": [{"question": 0, "strip_allowed": true}]}));
    r.m.send_with_pictures();
    r.turn();
    r.m.send_decide_label(Pos::question(0), LabelChoice::Keep);
    r.s.on("Put", &bank_path(), 422, json!({"error": "labels_need_decision", "errors": [{"code": "labels_unused", "path": "labels[question=0]"}], "label_findings": [{"question": 1, "strip_allowed": true}]}));
    r.m.send_with_decisions();
    r.turn();
    let SendStage::Decide(d) = r.send_stage() else { panic!("{:?}", r.send_stage()) };
    assert!(d.restarted && d.pictures_dropped && d.decisions().image_ok.is_empty());
    r.m.send_decide_label(Pos::question(1), LabelChoice::Strip);
    r.s.on("Put", &bank_path(), 201, sent_hidden(2));
    r.m.send_with_decisions();
    r.turn();
    let (meta, _) = body_parts(sends(&r).last().unwrap());
    assert!(meta.get("image_ok").is_none(), "{meta}");
    let SendStage::Done(d) = r.send_stage() else { panic!("{:?}", r.send_stage()) };
    assert_eq!(d.left_out_unasked, vec![1]);
    let t = sent_lines(d, false).join(" ");
    assert!(t.contains("question 2") && t.contains("did not choose to drop"), "{t}");
}

#[test]
fn pictures_a_bank_not_known_to_hide_keys_says_after_the_send_what_viewers_miss_and_asks_next_time() {
    let mut r = pictured_rig(None);
    r.open_send(editor());
    r.s.on("Put", &bank_path(), 201, sent_hidden(2));
    r.m.send();
    r.turn();
    assert_eq!(sends(&r).len(), 1, "not known to hide its keys: no step");
    let SendStage::Done(d) = r.send_stage() else { panic!("{:?}", r.send_stage()) };
    // (critic 62 I5: the person was never asked, so the line says that, and how to choose)
    assert_eq!((d.left_out_unasked.clone(), d.not_asked.clone()), (vec![], vec![1]), "the pool hid the keys and left the picture out");
    let t = sent_lines(d, false).join(" ");
    assert!(t.contains("question 2") && t.contains("You were not asked") && t.contains("did not know the bank hides its answer keys") && t.contains("Send again to choose which pictures viewers get") && !t.contains("did not choose to drop") && !t.contains('_'), "{t}");
    assert!(r.m.hides_keys(BID), "the answer says the bank hides its keys");
    // the next send asks
    r.m.close();
    r.edit_stem(0, "One, again");
    r.open_send(editor());
    r.m.send();
    r.turn();
    let _ = picture_stage(&r);
    // a hide_key_changed refusal says so too
    let mut r = pictured_rig(None);
    r.open_send(editor());
    r.s.on("Put", &bank_path(), 409, json!({"error": "hide_key_changed", "hide_key": true, "head_version": 1}));
    r.m.send();
    r.turn();
    assert!(r.m.hides_keys(BID));
    r.m.back_to_note();
    r.m.send();
    r.turn();
    let _ = picture_stage(&r);
    // a send answered without keys_hidden: the bank shows its keys now, and the person who chose to drop the picture
    // is told plainly that viewers get it, keys and all (critic 62 I1: this was silent)
    let mut r = pictured_rig(Some(true));
    r.open_send(editor());
    r.m.send();
    r.turn();
    r.s.on("Put", &bank_path(), 201, sent_ok(2, &[QA, QB]));
    r.m.send_with_pictures();
    r.turn();
    let SendStage::Done(d) = r.send_stage() else { panic!("{:?}", r.send_stage()) };
    assert!(d.left_out_unasked.is_empty() && d.not_asked.is_empty());
    assert_eq!(d.hiding_stopped, Some(vec![1]));
    let t = sent_lines(d, false).join(" ");
    assert!(t.contains("no longer hides its answer keys") && t.contains("answer keys and every picture") && t.contains("question 2") && t.contains("chose to drop") && !t.contains('_'), "{t}");
    assert!(!r.m.hides_keys(BID));
    // the step was not shown (the bank was not known to hide its keys) and it still does not: nothing to say
    let mut r = pictured_rig(Some(false));
    r.open_send(editor());
    r.s.on("Put", &bank_path(), 201, sent_ok(2, &[QA, QB]));
    r.m.send();
    r.turn();
    let SendStage::Done(d) = r.send_stage() else { panic!("{:?}", r.send_stage()) };
    assert_eq!((d.hiding_stopped.clone(), d.not_asked.clone(), d.left_out_unasked.clone()), (None, vec![], vec![]));
    assert_eq!(sent_lines(d, false).len(), 1, "{:?}", sent_lines(d, false));
}

/// The pool's `POST /circles/heads` answer for the bank, saying whether it hides its keys (critic 62 I5).
fn heads_hiding(hide: bool) -> Value {
    let mut h = heads_json("CC-BY-4.0", 1);
    h["banks"][0]["hide_key"] = json!(hide);
    h
}

#[test]
fn c62_i5_the_send_pages_own_check_of_the_bank_says_whether_it_hides_its_keys_before_anything_is_sent() {
    // not known here, but the pool's check when the page opens says it hides them: the step comes first
    let mut r = pictured_rig(None);
    r.s.keep("Post", "/circles/heads", 200, heads_hiding(true));
    r.open_send(editor());
    assert!(r.m.hides_keys(BID));
    r.m.send();
    r.turn();
    assert!(sends(&r).is_empty(), "nothing is sent before the pictures are decided");
    let _ = picture_stage(&r);
    // believed to hide them (an old read), but the pool says it no longer does: no step, nothing promised
    let mut r = pictured_rig(Some(true));
    r.s.keep("Post", "/circles/heads", 200, heads_hiding(false));
    r.open_send(editor());
    assert!(!r.m.hides_keys(BID));
    r.s.on("Put", &bank_path(), 201, sent_ok(2, &[QA, QB]));
    r.m.send();
    r.turn();
    assert_eq!(sends(&r).len(), 1);
    assert!(matches!(r.send_stage(), SendStage::Done(d) if d.hiding_stopped.is_none()));
    // a check that does not say keeps what was known
    let mut r = pictured_rig(Some(true));
    r.open_send(editor());
    assert!(r.m.hides_keys(BID));
}

/// Clicks the `nth` text `label` on the send page as the Circles tab draws it, 420 wide.
fn click_send_page(r: &mut Rig, ctx: &egui::Context, label: &str, nth: usize) -> bool {
    let mut cm = model();
    std::mem::swap(&mut cm.edit, &mut r.m);
    let hit = super::circles_phasec_tests::click_text(ctx, 420.0, label, nth, &mut |ui| {
        super::circles_ui::body_of(ui, &mut cm);
    });
    std::mem::swap(&mut cm.edit, &mut r.m);
    hit
}

#[test]
fn c62_i4_on_the_send_page_keep_then_drop_sends_no_picture() {
    let mut r = pictured_rig(Some(true));
    r.open_send(editor());
    r.m.send();
    r.turn();
    let ctx = egui::Context::default();
    crate::theme::setup(&ctx);
    assert!(click_send_page(&mut r, &ctx, super::circles_text::PICTURE_KEEP, 0));
    assert_eq!(picture_stage(&r).image_ok(), vec![1]);
    assert!(click_send_page(&mut r, &ctx, super::circles_text::PICTURE_DROP, 0));
    assert!(picture_stage(&r).image_ok().is_empty(), "Drop it un-keeps it");
    r.s.on("Put", &bank_path(), 201, sent_hidden(2));
    assert!(click_send_page(&mut r, &ctx, "Send", 0));
    r.turn();
    let puts = sends(&r);
    assert_eq!(puts.len(), 1);
    assert!(body_parts(&puts[0]).0.get("image_ok").is_none(), "{}", body_parts(&puts[0]).0);
}

#[test]
fn pictures_another_account_forgets_what_was_known_and_chosen() {
    let mut r = pictured_rig(Some(true));
    r.open_send(editor());
    r.m.send();
    r.turn();
    r.m.send_keep_picture(1, true);
    r.m.forget_account(true);
    assert!(matches!(r.m.view, EditView::Closed));
    assert!(!r.m.hides_keys(BID));
}

#[test]
fn pictures_the_send_step_draws_wide_and_narrow() {
    let mut r = pictured_rig(Some(true));
    r.open_send(editor());
    r.m.send();
    r.turn();
    r.m.send_keep_picture(1, true);
    let ctx = egui::Context::default();
    for dark in [false, true] {
        crate::theme::set_dark(dark, Some(&ctx));
        crate::theme::setup(&ctx);
        let t = draw_all(&mut r, &ctx);
        for want in ["Keep it for viewers", "Drop it", "Question 2", "Two, with a picture", "does not show an answer or a hint", "Send"] {
            assert!(t.contains(want), "{want}: {t}");
        }
    }
    crate::theme::set_dark(false, Some(&ctx));
}
