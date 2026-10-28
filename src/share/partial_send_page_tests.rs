//! Circles phase D3, piece 4 (docs/circles-phaseD-plan.md 4.7, 5.5 and 6): the Send page's summary of changes
//! against the base version's body. Each test pins one rule: the body is read once when the page opens, the
//! defaults are every change ticked except `held_back`, an unticked change goes as the base's raw question, an
//! unticked removal is put back and keeps its pair, a base that can not be had sends the whole bank said plainly,
//! nothing ticked refuses the send, and an edit made while the send was in flight shows as unsent afterwards.
//!
//! The drawing is the next piece: these tests drive the model only.

use std::collections::BTreeSet;
use std::sync::Arc;

use serde_json::{Value, json};

use super::bank_links_fix11_tests::version_path;
use super::bank_links_fix4_tests::{background, owner};
use super::bank_links_fix_tests::{QC, linked_from, qfull, served_qs};
use super::bank_links_tests::{LOCAL, QA, QB, heads, hex};
use super::circle_edit::*;
use super::circle_links::{CircleLink, changes_since, payload_of};
use super::circles::CirclesModel;
use super::circles_tests::{Script, model, now, ready};
use super::merge::{Choice, RowKind};
use super::partial_send::BodyChanges;
use super::transport::Method;
use crate::model::QuestionBank;

/// The question only this bank has (added: no circle id).
const QD: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa4";
/// The circle id the answer gives the added question.
const QNEW: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa9";

fn base_qs() -> Vec<Value> {
    vec![qfull(QA, "A"), qfull(QB, "B"), qfull(QC, "C")]
}

fn added_question() -> crate::model::Question {
    let mut q = super::circle_links::circle_theirs(&super::bank_links_fix_tests::body_of(vec![qfull(QD, "D")]), super::circles_tests::NOW).remove(0).q;
    q.id = crate::model::new_id();
    q
}

/// The local id this copy has for a circle question.
fn local(link: &CircleLink, qid: &str) -> String {
    link.qids.iter().find(|q| q.qid == qid).expect("a linked question").local.clone()
}

/// A working copy in step with version 3 (the base body is `base_qs`), `edit` applied, and the Send page opened
/// with its base body read once. The links file lives in memory.
fn open_page(edit: &dyn Fn(&mut QuestionBank), held_back: &[&str]) -> (CirclesModel, Arc<Script>, Vec<QuestionBank>) {
    let base = base_qs();
    let (mut bank, mut link) = linked_from(base.clone(), false);
    link.base_version = 3;
    link.head_seen = 3;
    link.role_seen = Some("owner".into());
    link.author_known = true;
    link.is_author = true;
    link.held_back = held_back.iter().map(|qid| local(&link, qid)).collect();
    edit(&mut bank);
    bank.modified += 5;
    let s = Script::new();
    s.keep("Post", "/circles/heads", 200, heads(3));
    s.keep_raw("Get", &version_path(3), served_qs(3, base, false));
    let mut banks = vec![bank];
    let mut m = model();
    m.edit.add_link(link).unwrap();
    m.edit.frame(now(), &ready(&s));
    m.edit.sync(&mut banks);
    assert!(m.edit.open_send(LOCAL, owner()));
    m.edit.frame(now(), &ready(&s));
    m.edit.sync(&mut banks);
    m.edit.set_attest(true);
    (m, s, banks)
}

/// The same, with the circle answering the base read `status`/`body` instead.
fn open_page_base_error(status: u16, body: Value, edit: &dyn Fn(&mut QuestionBank)) -> (CirclesModel, Arc<Script>, Vec<QuestionBank>) {
    let base = base_qs();
    let (mut bank, mut link) = linked_from(base, false);
    link.base_version = 3;
    link.head_seen = 3;
    link.role_seen = Some("owner".into());
    link.author_known = true;
    link.is_author = true;
    edit(&mut bank);
    bank.modified += 5;
    let s = Script::new();
    s.keep("Post", "/circles/heads", 200, heads(3));
    s.on("Get", &version_path(3), status, body);
    let mut banks = vec![bank];
    let mut m = model();
    m.edit.add_link(link).unwrap();
    m.edit.frame(now(), &ready(&s));
    m.edit.sync(&mut banks);
    assert!(m.edit.open_send(LOCAL, owner()));
    m.edit.frame(now(), &ready(&s));
    m.edit.sync(&mut banks);
    m.edit.set_attest(true);
    (m, s, banks)
}

/// The same, for a base body in another order (the circle's own order).
fn open_page_with_base(base: Vec<Value>, edit: &dyn Fn(&mut QuestionBank), held_back: &[&str]) -> (CirclesModel, Arc<Script>, Vec<QuestionBank>) {
    let (mut bank, mut link) = linked_from(base.clone(), false);
    link.base_version = 3;
    link.head_seen = 3;
    link.role_seen = Some("owner".into());
    link.author_known = true;
    link.is_author = true;
    link.held_back = held_back.iter().map(|qid| local(&link, qid)).collect();
    edit(&mut bank);
    bank.modified += 5;
    let s = Script::new();
    s.keep("Post", "/circles/heads", 200, heads(3));
    s.keep_raw("Get", &version_path(3), served_qs(3, base, false));
    let mut banks = vec![bank];
    let mut m = model();
    m.edit.add_link(link).unwrap();
    m.edit.frame(now(), &ready(&s));
    m.edit.sync(&mut banks);
    assert!(m.edit.open_send(LOCAL, owner()));
    m.edit.frame(now(), &ready(&s));
    m.edit.sync(&mut banks);
    m.edit.set_attest(true);
    (m, s, banks)
}

/// Frames and syncs until the send summary is ready (and the licence is known).
fn wait_ready(m: &mut CirclesModel, s: &Arc<Script>, banks: &mut [QuestionBank]) {
    for _ in 0..500 {
        m.edit.frame(now(), &ready(s));
        m.edit.sync(banks);
        if matches!(m.edit.partial_send(), Some(PartialSend::Ready { .. })) && matches!(&m.edit.view, EditView::Send(v) if matches!(v.licence, LicenceState::Known(_))) {
            return;
        }
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
    panic!("not ready: {:?}", m.edit.view);
}

fn ticked(m: &CirclesModel) -> BTreeSet<String> {
    m.edit.send_ticked().expect("the summary is ready").clone()
}

fn changes(m: &CirclesModel) -> BodyChanges {
    m.edit.send_changes().expect("the summary is ready").clone()
}

fn puts(s: &Arc<Script>) -> Vec<super::circles_tests::Sent> {
    s.seen().into_iter().filter(|x| x.method == Method::Put).collect()
}

/// The payload of the last PUT (the body is the meta line, a newline, then the payload).
fn put_payload(x: &super::circles_tests::Sent) -> Value {
    let text = String::from_utf8(x.body.clone()).expect("a UTF-8 send body");
    let (_, payload) = text.split_once('\n').expect("a meta line and a payload");
    serde_json::from_str(payload).expect("the payload is JSON")
}

fn question_stems(v: &Value) -> Vec<String> {
    v.pointer("/bank/questions").and_then(Value::as_array).expect("bank.questions").iter().map(|q| q["stem"].as_str().unwrap().to_string()).collect()
}

fn qid_of(q: &Value) -> Option<&str> {
    q.pointer("/published/qid").and_then(Value::as_str)
}

/// The answer to a send that made `version` from the composed questions, in payload order.
fn sent_with(version: u64, qids: &[&str]) -> Value {
    json!({"circle_id": super::circles_tests::CID, "bank_id": super::circles_tests::BID, "version": version, "version_id": hex(version), "qids": qids, "added": 0, "changed": 0, "removed": 0})
}

/// Runs the send to its answer.
fn send(m: &mut CirclesModel, s: &Arc<Script>, banks: &mut [QuestionBank], answer: Value) {
    s.on("Put", &super::bank_links_fix8_tests::put_path(), 201, answer);
    m.edit.send();
    m.edit.sync(banks);
    m.edit.frame(now(), &ready(s));
}

// ---- opening the page: one read, every change ticked but held_back ---------------------------------------------------

#[test]
fn d3_the_send_page_reads_the_base_body_once_and_starts_every_change_ticked_but_held_back() {
    // base [A, B, C]; this bank [A', B, D] (A changed, B unchanged, C removed, D added)
    let (m, s, _banks) = open_page(
        &|b| {
            b.questions[0].stem = "A (changed)".into();
            b.questions.pop();
            b.questions.push(added_question());
        },
        &[],
    );
    let read = s.paths().iter().filter(|p| **p == format!("Get {}", version_path(3))).count();
    assert_eq!(read, 1, "the base body is read once: {:?}", s.paths());
    let c = changes(&m);
    assert_eq!((c.changed.len(), c.added.len(), c.removed.len()), (1, 1, 1), "{c:?}");
    assert_eq!(ticked(&m), c.ids(), "all changes start ticked");
    assert!(matches!(m.edit.partial_send(), Some(PartialSend::Ready { .. })));
}

#[test]
fn d3_a_held_back_id_starts_unticked_and_one_no_longer_changed_is_not_ticked() {
    // base [A, B, C]; this bank [A', B] (A changed, C removed); held back: B (unchanged now) and C (still changed)
    let (mut m, s, mut banks) = open_page(
        &|b| {
            b.questions[0].stem = "A (changed)".into();
            b.questions.pop();
        },
        &[QB, QC],
    );
    let link = m.edit.link_for_local(LOCAL).unwrap();
    let (a, b, c) = (local(&link, QA), local(&link, QB), local(&link, QC));
    let t = ticked(&m);
    assert_eq!(t, BTreeSet::from([a]), "only the change not held back is ticked: {t:?}");
    assert!(!t.contains(&b), "an id no longer changed is never ticked");
    assert!(!t.contains(&c), "a held back change starts unticked");
    // The send keeps only the unticked id that still differs: B is dropped from `held_back`.
    send(&mut m, &s, &mut banks, sent_with(4, &[QA, QB, QC]));
    assert_eq!(m.edit.link_for_local(LOCAL).unwrap().held_back, vec![c], "a held back id no longer changed is dropped");
    assert_eq!(changes_since(&m.edit.link_for_local(LOCAL).unwrap(), &banks[0]).removed, 1);
}

// ---- unticking a change: the base's raw question, and the link afterwards -------------------------------------------

#[test]
fn d3_an_unticked_change_goes_as_the_bases_raw_question_and_stays_unsent_after_the_send() {
    let (mut m, s, mut banks) = open_page(
        &|b| {
            b.questions[0].stem = "A (changed)".into();
            b.questions.pop();
            b.questions.push(added_question());
        },
        &[],
    );
    let link = m.edit.link_for_local(LOCAL).unwrap();
    let (a, x, d) = (local(&link, QA), local(&link, QC), banks[0].questions[2].id.clone());
    m.edit.toggle_change(&a, false);
    assert!(!m.edit.send_ticked().unwrap().contains(&a));
    // composed: [the base's raw A, B, D]; C is a ticked removal, so it is omitted
    send(&mut m, &s, &mut banks, sent_with(4, &[QA, QB, QNEW]));
    let payload = put_payload(&puts(&s).last().expect("the send went out"));
    assert_eq!(question_stems(&payload), ["A", "B", "D"], "{payload}");
    let qs = payload.pointer("/bank/questions").and_then(Value::as_array).unwrap();
    assert_eq!(qs[0]["id"], json!(QA), "the base's raw question, its own circle id");
    assert_eq!(qid_of(&qs[0]), Some(QA), "and the block that keeps it unchanged");
    assert_eq!(qid_of(&qs[2]), None, "the added question is new to the circle");

    let link = m.edit.link_for_local(LOCAL).unwrap();
    assert_eq!(link.base_version, 4, "the link is at the version the composed payload made");
    assert_eq!(link.held_back, vec![a.clone()], "the unticked change is kept for the next summary");
    let count = changes_since(&link, &banks[0]);
    assert_eq!((count.changed, count.added, count.removed), (1, 0, 0), "the unticked change stays unsent: {count:?}");
    assert_eq!(link.qid_of(&d), Some(QNEW), "the ticked addition is in step");
    assert!(link.base.iter().any(|b| b.local_id == d && b.snap.stem == "D"), "the added question's base is what was sent");
    assert!(link.base.iter().any(|b| b.local_id == a && b.snap.stem == "A"), "the unticked change's base is the base's raw question");
    assert_eq!(link.qid_of(&x), None, "the ticked removal's pair is dropped");
}

// ---- removals: unticked restored with its pair, ticked omitted ---------------------------------------------------------

#[test]
fn d3_an_unticked_removal_is_put_back_with_its_pair_and_a_ticked_one_is_omitted() {
    for untick in [true, false] {
        let (mut m, s, mut banks) = open_page(
            &|b| {
                b.questions[0].stem = "A (changed)".into();
                b.questions.pop();
            },
            &[],
        );
        let link = m.edit.link_for_local(LOCAL).unwrap();
        let x = local(&link, QC);
        if untick {
            m.edit.toggle_change(&x, false);
        }
        let qids: &[&str] = if untick { &[QA, QB, QC] } else { &[QA, QB] };
        send(&mut m, &s, &mut banks, sent_with(4, qids));
        let payload = put_payload(&puts(&s).last().expect("the send went out"));
        if untick {
            assert_eq!(question_stems(&payload), ["A (changed)", "B", "C"], "the removal is put back: {payload}");
            let qs = payload.pointer("/bank/questions").and_then(Value::as_array).unwrap();
            assert_eq!(qs[2]["id"], json!(QC), "as the base has it");
            assert_eq!(qid_of(&qs[2]), Some(QC), "and keeps its identity");
        } else {
            assert_eq!(question_stems(&payload), ["A (changed)", "B"], "the ticked removal is omitted: {payload}");
        }
        let link = m.edit.link_for_local(LOCAL).unwrap();
        assert_eq!(link.base_version, 4);
        if untick {
            assert_eq!(link.qid_of(&x), Some(QC), "the restored removal keeps its pair");
            assert_eq!(link.held_back, vec![x.clone()], "and is unticked for the next summary");
            assert_eq!(changes_since(&link, &banks[0]).removed, 1, "still a removal here");
        } else {
            assert_eq!(link.qid_of(&x), None, "the sent removal's pair is dropped");
            assert!(link.held_back.is_empty());
            assert_eq!(changes_since(&link, &banks[0]).removed, 0);
        }
    }
}

// ---- the base can not be had: the whole bank, said plainly ---------------------------------------------------------------

#[test]
fn d3_a_base_no_longer_kept_or_withheld_sends_the_whole_bank_and_the_plain_line_is_there() {
    for body in [json!({"error": "version_not_retained"}), json!({"error": "version_withheld"})] {
        let (mut m, s, mut banks) = open_page_base_error(404, body.clone(), &|b| b.questions[0].stem = "A (changed)".into());
        let shown = match m.edit.partial_send().unwrap() {
            PartialSend::Unavailable(why) => why.clone(),
            other => panic!("{body}: {other:?}"),
        };
        assert_eq!(shown, PARTIAL_UNAVAILABLE, "the plan's plain line: {body}");
        assert!(shown.contains("needs the version your copy is based on") && shown.contains("no longer keeps it"), "{shown}");
        assert!(m.edit.send_blocker().is_none(), "nothing is disabled: the whole bank is sent");
        let link = m.edit.link_for_local(LOCAL).unwrap();
        let bank = banks[0].clone();
        let expected: Value = serde_json::from_slice(&payload_of(&link, &bank).unwrap()).unwrap();
        send(&mut m, &s, &mut banks, sent_with(4, &[QA]));
        let payload = put_payload(&puts(&s).last().expect("the send went out"));
        assert_eq!(payload, expected, "the whole bank, as before: {body}");
        assert!(matches!(&m.edit.view, EditView::Send(v) if matches!(v.stage, SendStage::Done(_))), "{body}");
        assert_eq!(m.edit.link_for_local(LOCAL).unwrap().base_version, 4);
    }
}

// ---- nothing ticked ------------------------------------------------------------------------------------------------------

#[test]
fn d3_nothing_ticked_refuses_with_tick_at_least_one_change_and_sends_nothing() {
    let (mut m, s, mut banks) = open_page(&|b| b.questions[0].stem = "A (changed)".into(), &[]);
    m.edit.tick_all(false);
    assert!(m.edit.send_ticked().unwrap().is_empty());
    assert!(m.edit.send_blocker().is_none(), "the button is not disabled for having no tick yet");
    m.edit.send();
    let stage = match &m.edit.view {
        EditView::Send(v) => &v.stage,
        other => panic!("not on the send page: {other:?}"),
    };
    assert!(matches!(stage, SendStage::Refused { message, merge_again: false } if message == TICK_ONE), "{stage:?}");
    m.edit.sync(&mut banks);
    m.edit.frame(now(), &ready(&s));
    assert!(puts(&s).is_empty(), "nothing was sent");
    assert!(matches!(m.edit.partial_send(), Some(PartialSend::Ready { ticked, .. }) if ticked.is_empty()), "the ticks stay as they were");
}

// ---- an edit while the send was in flight ---------------------------------------------------------------------------------

#[test]
fn d3_an_edit_made_while_the_send_was_in_flight_is_not_stale_and_shows_as_unsent_after_the_answer() {
    let base = base_qs();
    let (mut bank, mut link) = linked_from(base.clone(), false);
    link.base_version = 3;
    link.head_seen = 3;
    link.role_seen = Some("owner".into());
    link.author_known = true;
    link.is_author = true;
    bank.questions[0].stem = "A (changed)".into();
    bank.modified += 5;
    let s = Script::new();
    s.keep("Post", "/circles/heads", 200, heads(3));
    s.keep_raw("Get", &version_path(3), served_qs(3, base, false));
    let mut banks = vec![bank];
    let mut m = model();
    m.edit.add_link(link).unwrap();
    m.edit.frame(now(), &ready(&s));
    m.edit.sync(&mut banks);
    // the base read runs on its own thread
    background(&mut m);
    m.edit.frame(now(), &ready(&s));
    assert!(m.edit.open_send(LOCAL, owner()));
    let mut reads = 0;
    for _ in 0..500 {
        m.edit.frame(now(), &ready(&s));
        m.edit.sync(&mut banks);
        reads = s.paths().iter().filter(|p| **p == format!("Get {}", version_path(3))).count();
        if matches!(m.edit.partial_send(), Some(PartialSend::Ready { .. })) && matches!(&m.edit.view, EditView::Send(v) if matches!(v.licence, LicenceState::Known(_))) {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
    assert!(matches!(m.edit.partial_send(), Some(PartialSend::Ready { .. })), "the base read answered: {:?}", s.paths());
    assert_eq!(reads, 1, "one read per page: {:?}", s.paths());
    m.edit.set_attest(true);
    s.on("Put", &super::bank_links_fix8_tests::put_path(), 201, sent_with(4, &[QA]));
    // the send is held at the server (its first request, the licence check, stops there)
    let n = s.paths().len();
    let release = s.hold();
    m.edit.send();
    m.edit.sync(&mut banks);
    for _ in 0..500 {
        if s.paths().len() > n {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
    assert!(s.paths().len() > n, "the send went out: {:?}", s.paths());
    // she edits the same question again while it is on its way
    banks[0].questions[0].stem = "A (changed again)".into();
    banks[0].modified += 5;
    release.send(()).unwrap();
    for _ in 0..500 {
        m.edit.frame(now(), &ready(&s));
        m.edit.sync(&mut banks);
        if matches!(&m.edit.view, EditView::Send(v) if matches!(v.stage, SendStage::Done(_))) {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
    let link = m.edit.link_for_local(LOCAL).unwrap();
    assert!(!link.stale_for(&banks[0]), "newer than the link: never put back");
    assert!(!m.edit.status_of(&banks[0]).unwrap().stale);
    let count = changes_since(&link, &banks[0]);
    assert_eq!(count.changed, 1, "the edit made in flight is unsent: {count:?}");
    assert_eq!(link.base_version, 4);
}

// ---- the whole-bank path unchanged -----------------------------------------------------------------------------------------

#[test]
fn d3_the_whole_bank_path_is_unchanged_when_the_base_can_not_be_had() {
    // The same shape as the existing send tests: one change, no base body read scripted (the circle no longer
    // keeps it). The payload is the bank exactly as `payload_of` makes it today.
    let (mut m, s, mut banks) = open_page_base_error(404, json!({"error": "version_not_retained"}), &|b| b.questions[0].stem = "A (changed)".into());
    let link = m.edit.link_for_local(LOCAL).unwrap();
    let bank = banks[0].clone();
    let expected: Value = serde_json::from_slice(&payload_of(&link, &bank).unwrap()).unwrap();
    send(&mut m, &s, &mut banks, sent_with(4, &[QA]));
    let payload = put_payload(&puts(&s).last().expect("the send went out"));
    assert_eq!(payload, expected);
}

// ---- the summary is a snapshot: a bank changed since is listed again, nothing is sent yet -------------------------------

#[test]
fn d3_a_bank_changed_after_the_body_arrived_is_listed_again_and_nothing_is_sent() {
    let (mut m, s, mut banks) = open_page(&|b| b.questions[0].stem = "A (changed)".into(), &[]);
    let link = m.edit.link_for_local(LOCAL).unwrap();
    let (a, b) = (local(&link, QA), local(&link, QB));
    // A question that was unchanged when the body was read is edited after it.
    banks[0].questions[1].stem = "B (changed later)".into();
    banks[0].modified += 5;
    m.edit.send();
    m.edit.sync(&mut banks);
    assert!(puts(&s).is_empty(), "nothing is sent while the list is out of date: {:?}", s.paths());
    let (stage, notice) = match &m.edit.view {
        EditView::Send(v) => (format!("{:?}", v.stage), v.notice.clone()),
        other => panic!("not on the send page: {other:?}"),
    };
    assert!(stage.contains("Editing"), "{stage}");
    assert_eq!(notice.as_deref(), Some(SEND_LIST_AGAIN));
    let c = changes(&m);
    assert!(c.changed.contains(&a) && c.changed.contains(&b), "the list is worked out again: {c:?}");
    let t = ticked(&m);
    assert!(t.contains(&a) && t.contains(&b), "a change not seen yet starts ticked: {t:?}");
    // The next press goes, with the new change.
    send(&mut m, &s, &mut banks, sent_with(4, &[QA, QB, QC]));
    let payload = put_payload(&puts(&s).last().expect("the send went out"));
    assert_eq!(question_stems(&payload), ["A (changed)", "B (changed later)", "C"]);
}

#[test]
fn d3_deleting_a_ticked_changed_question_makes_it_a_removal_whose_tick_decides() {
    for keep_tick in [true, false] {
        // A is changed (ticked, then deleted while the page is open); B is changed and stays ticked, so the send
        // still has something to send either way.
        let (mut m, s, mut banks) = open_page(
            &|b| {
                b.questions[0].stem = "A (changed)".into();
                b.questions[1].stem = "B (changed)".into();
            },
            &[],
        );
        let link = m.edit.link_for_local(LOCAL).unwrap();
        let a = local(&link, QA);
        banks[0].questions.retain(|q| q.id != a);
        banks[0].modified += 5;
        m.edit.send();
        m.edit.sync(&mut banks);
        assert!(puts(&s).is_empty(), "nothing is sent while the list is out of date: {keep_tick} {:?}", s.paths());
        let c = changes(&m);
        assert!(c.removed.contains(&a) && !c.changed.contains(&a), "the deleted change is a removal now: {c:?}");
        assert!(ticked(&m).contains(&a), "the tick carries by local id: {:?}", ticked(&m));
        if !keep_tick {
            m.edit.toggle_change(&a, false);
            assert!(!ticked(&m).contains(&a));
        }
        let qids: &[&str] = if keep_tick { &[QB, QC] } else { &[QA, QB, QC] };
        send(&mut m, &s, &mut banks, sent_with(4, qids));
        let payload = put_payload(&puts(&s).last().expect("the send went out"));
        if keep_tick {
            assert_eq!(question_stems(&payload), ["B (changed)", "C"], "a ticked removal goes: {payload}");
        } else {
            assert_eq!(question_stems(&payload), ["A", "B (changed)", "C"], "an unticked removal is put back: {payload}");
        }
    }
}

// ---- a 409 then Get latest keeps the ticks by local id --------------------------------------------------------------------

#[test]
fn d3_after_a_conflict_and_get_latest_an_unticked_change_is_still_unticked() {
    // A is unticked; D is a new question and stays ticked, so the send has something to send.
    let (mut m, s, mut banks) = open_page(
        &|b| {
            b.questions[0].stem = "A (changed)".into();
            b.questions.push(added_question());
        },
        &[],
    );
    let link = m.edit.link_for_local(LOCAL).unwrap();
    let (a, d) = (local(&link, QA), banks[0].questions[3].id.clone());
    m.edit.toggle_change(&a, false);
    s.on("Put", &super::bank_links_fix8_tests::put_path(), 409, json!({"error": "conflict", "head_version": 4, "head_at": super::circles_tests::NOW - 1000, "head_by": "Bob Teacher"}));
    m.edit.send();
    m.edit.sync(&mut banks);
    m.edit.frame(now(), &ready(&s));
    assert!(matches!(&m.edit.view, EditView::Send(v) if matches!(v.stage, SendStage::Conflict(_))), "{:?}", m.edit.view);
    // Get latest: Bob changed A; she keeps hers by default, so A is still an unsent change.
    s.on("Post", &format!("{}/merge", super::bank_links_fix8_tests::put_path()), 200, super::bank_links_fix8_tests::merge_start(3, 4));
    s.keep_raw("Get", &super::bank_links_fix8_tests::put_path(), served_qs(4, vec![qfull(QA, "A (Bob)"), qfull(QB, "B"), qfull(QC, "C")], false));
    assert!(m.edit.open_merge(LOCAL, owner()));
    for _ in 0..500 {
        m.edit.frame(now(), &ready(&s));
        m.edit.sync(&mut banks);
        if matches!(&m.edit.view, EditView::Merge(v) if matches!(v.phase, MergePhase::Review(_))) {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
    assert!(matches!(&m.edit.view, EditView::Merge(v) if matches!(v.phase, MergePhase::Review(_))), "{:?}", m.edit.view);
    let conflict = match &m.edit.view {
        EditView::Merge(v) => match &v.phase {
            MergePhase::Review(r) => r.plan.rows.iter().position(|x| x.kind == RowKind::Conflict),
            _ => None,
        },
        _ => None,
    };
    let conflict = conflict.expect("A was changed by both");
    m.edit.set_merge_choice(conflict, Choice::Ours);
    m.edit.apply_merge();
    for _ in 0..500 {
        m.edit.frame(now(), &ready(&s));
        m.edit.sync(&mut banks);
        if matches!(&m.edit.view, EditView::Merge(v) if matches!(v.phase, MergePhase::Applied(_))) {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
    assert!(matches!(&m.edit.view, EditView::Merge(v) if matches!(v.phase, MergePhase::Applied(_))), "{:?}", m.edit.view);
    m.edit.cancel_merge();
    // The next Send page reads the new base body; the unticked mark is kept by local id.
    s.keep_raw("Get", &version_path(4), served_qs(4, vec![qfull(QA, "A (Bob)"), qfull(QB, "B"), qfull(QC, "C")], false));
    assert!(m.edit.open_send(LOCAL, owner()));
    wait_ready(&mut m, &s, &mut banks);
    let link = m.edit.link_for_local(LOCAL).unwrap();
    let a = local(&link, QA);
    let d = banks[0].questions.iter().find(|q| link.qid_of(&q.id).is_none()).expect("the added question").id.clone();
    let c = changes(&m);
    assert!(c.changed.contains(&a) && c.added.contains(&d), "A is still unsent and D is still new: {c:?}");
    assert!(!m.edit.send_ticked().unwrap().contains(&a), "A is still unticked after the conflict and Get latest");
    assert!(m.edit.send_ticked().unwrap().contains(&d), "a change not seen before the conflict is ticked");
    let read4 = s.paths().iter().filter(|p| **p == format!("Get {}", version_path(4))).count();
    assert_eq!(read4, 1, "the rebuilt list reads the new base body once: {:?}", s.paths());
}

// ---- a transient base-read failure: the page waits, Try again reads, never the whole bank ---------------------------------

#[test]
fn d3_an_offline_or_limited_base_read_keeps_the_page_and_retry_reads_again() {
    for (status, body) in [(503, json!({"error": "unavailable"})), (429, json!({"error": "rate_limited"}))] {
        let (mut m, s, mut banks) = open_page_base_error(status, body.clone(), &|b| b.questions[0].stem = "A (changed)".into());
        let why = match m.edit.partial_send().unwrap() {
            PartialSend::ReadFailed(why) => why.clone(),
            other => panic!("{body}: {other:?}"),
        };
        assert_ne!(why, PARTIAL_UNAVAILABLE, "{body}");
        assert!(!why.contains("no longer keeps it"), "a read that may work is not said to be gone: {why}");
        assert_eq!(m.edit.send_blocker(), Some(PARTIAL_READ_FAILED), "{body}");
        m.edit.send();
        m.edit.sync(&mut banks);
        m.edit.frame(now(), &ready(&s));
        assert!(puts(&s).is_empty(), "no whole-bank send for a read that may pass: {status} {:?}", s.paths());
        // The circle answers the one read when it is asked again.
        s.keep_raw("Get", &version_path(3), served_qs(3, base_qs(), false));
        m.edit.retry_partial();
        wait_ready(&mut m, &s, &mut banks);
        let reads = s.paths().iter().filter(|p| **p == format!("Get {}", version_path(3))).count();
        assert_eq!(reads, 2, "Try again runs the one read again: {status} {:?}", s.paths());
    }
}

// ---- an answer that could not be read: reopening keeps the selection (and the commit id) --------------------------------

#[test]
fn d3_after_an_unread_answer_reopening_keeps_the_unticked_selection_and_the_commit_id() {
    for mode in ["timeout", "version 0"] {
        // A is unticked; D is new and ticked, so the send has something to send.
        let (mut m, s, mut banks) = open_page(
            &|b| {
                b.questions[0].stem = "A (changed)".into();
                b.questions.push(added_question());
            },
            &[],
        );
        let link = m.edit.link_for_local(LOCAL).unwrap();
        let (a, d) = (local(&link, QA), banks[0].questions[3].id.clone());
        m.edit.toggle_change(&a, false);
        // (the licence check's own answer is queued first, so the answer or failure lands on the send itself)
        s.on("Post", "/circles/heads", 200, heads(3));
        match mode {
            "timeout" => {
                s.fail(super::bank_links_fix8_tests::timed_out().unwrap());
            }
            _ => {
                s.on("Put", &super::bank_links_fix8_tests::put_path(), 201, super::bank_links_fix9_tests::version_zero());
            }
        }
        m.edit.send();
        m.edit.sync(&mut banks);
        m.edit.frame(now(), &ready(&s));
        let first = puts(&s).pop().expect("the send went out");
        let first: Value = serde_json::from_str(String::from_utf8(first.body).unwrap().split_once('\n').unwrap().0).unwrap();
        m.edit.close();
        // Reopening starts from the retry record: the same selection, and the same commit id once sent again.
        assert!(m.edit.open_send(LOCAL, owner()));
        wait_ready(&mut m, &s, &mut banks);
        assert!(!m.edit.send_ticked().unwrap().contains(&a), "{mode}: the unticked change is still unticked after reopening");
        assert!(m.edit.send_ticked().unwrap().contains(&d), "{mode}: and the other change is still ticked");
        m.edit.set_attest(true);
        send(&mut m, &s, &mut banks, sent_with(4, &[QA, QB, QC, QNEW]));
        let second = puts(&s).pop().expect("the send went out again");
        let second: Value = serde_json::from_str(String::from_utf8(second.body).unwrap().split_once('\n').unwrap().0).unwrap();
        assert_eq!(second["commit_id"], first["commit_id"], "{mode}: the same selection reuses the commit id: nothing is sent twice");
    }
}

// ---- refusal positions are the bank's question numbers --------------------------------------------------------------------

#[test]
fn d3_a_keyless_refusal_names_the_banks_question_not_the_payload_position() {
    // base order A, C, B; this bank A, B (C removed and unticked, so it goes back between A and B); B has no key.
    let base = vec![qfull(QA, "A"), qfull(QC, "C"), qfull(QB, "B")];
    let (mut m, _s, mut banks) = open_page_with_base(
        base,
        &|b| {
            b.questions.remove(1); // C
            b.questions[1].explanation = Some(crate::model::KEYLESS_SENTINEL.into()); // B
        },
        &[QC],
    );
    m.edit.send();
    m.edit.sync(&mut banks);
    let stage = match &m.edit.view {
        EditView::Send(v) => &v.stage,
        other => panic!("not on the send page: {other:?}"),
    };
    // The keyless question is payload position 2 (after the restored C), the bank's question 1.
    assert!(matches!(stage, SendStage::Keyless { positions, .. } if positions == &vec![1]), "{stage:?}");
}

// ---- send-on-top never composes against another base ----------------------------------------------------------------------

#[test]
fn d3_a_send_over_a_held_version_goes_as_the_whole_bank() {
    let (mut m, s, mut banks) = open_page(
        &|b| {
            b.questions[0].stem = "A (changed)".into();
            b.questions.push(added_question());
        },
        &[],
    );
    let link = m.edit.link_for_local(LOCAL).unwrap();
    let d = banks[0].questions[3].id.clone();
    m.edit.toggle_change(&d, false);
    // A hidden newest version (what `send_on_top` sets): the whole bank goes, never a selection.
    if let EditView::Send(v) = &mut m.edit.view {
        v.base_override = Some(6);
    }
    let bank = banks[0].clone();
    let expected: Value = serde_json::from_slice(&payload_of(&link, &bank).unwrap()).unwrap();
    send(&mut m, &s, &mut banks, sent_with(4, &[QA, QB, QC, QNEW]));
    let payload = put_payload(&puts(&s).last().expect("the send went out"));
    assert_eq!(payload, expected, "the whole bank goes over the held version");
    assert_eq!(question_stems(&payload).len(), 4, "the unticked addition is still in the whole bank: {payload}");
}
