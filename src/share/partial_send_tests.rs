//! Circles phase D3 (docs/circles-phaseD-plan.md sections 4.7 and 6): composing the payload for a partial
//! send. Each test pins one rule: a ticked change is ours, an unticked one is the base's raw question, an
//! unticked removal goes back where the base had it, an unchanged question is sent either way, a bounded
//! link (no `base`, full `qids`) still restores its removals, and the errors that make the caller fall back
//! to a whole-bank send.
//!
//! `changed` is the caller's list, worked out against the base body; the composer never re-derives it (a
//! `merge::Snap` has no `figure`, and a bounded link has no snapshots at all).

use std::collections::BTreeSet;

use serde_json::{Value, json};

use super::bank_links_fix_tests::{body_of, linked_from, qfull, QC};
use super::bank_links_tests::{LOCAL, QA, QB};
use super::circle_links::{CircleLink, SEND_SCHEMA, circle_theirs};
use super::circles_tests::NOW;
use super::partial_send::compose_partial;
use crate::model::{Difficulty, QuestionBank};

/// A question only this bank has (added: no circle id).
const QD: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa4";

fn qd(stem: &str) -> Value {
    qfull(QD, stem)
}

/// The plan's example: base [A, B, C]; this bank [A', B, D] (A changed, B unchanged, C removed, D added).
struct Case {
    bank: QuestionBank,
    link: CircleLink,
    base: Vec<u8>,
}

fn case() -> Case {
    let base_qs = vec![qfull(QA, "A"), qfull(QB, "B"), qfull(QC, "C")];
    let base = body_of(base_qs.clone());
    let (mut bank, link) = linked_from(base_qs, false);
    bank.questions[0].stem = "A (changed)".into();
    let mut added = circle_theirs(&body_of(vec![qd("D")]), NOW).remove(0).q;
    added.id = crate::model::new_id();
    bank.questions.pop();
    bank.questions.push(added);
    Case { bank, link, base }
}

/// The local id the bank has for a circle question.
fn local_of(link: &CircleLink, qid: &str) -> String {
    link.qids.iter().find(|q| q.qid == qid).unwrap().local.clone()
}

fn added_local(c: &Case) -> String {
    c.bank.questions.iter().find(|q| c.link.qid_of(&q.id).is_none()).unwrap().id.clone()
}

fn set(ids: &[&str]) -> BTreeSet<String> {
    ids.iter().map(|s| s.to_string()).collect()
}

fn composed(c: &Case, changed: &[&str], ticked: &[&str]) -> Value {
    let out = compose_partial(&c.link, &c.bank, &c.base, &set(changed), &set(ticked)).expect("the composed payload");
    serde_json::from_slice(&out).expect("the payload is JSON")
}

fn questions(v: &Value) -> Vec<Value> {
    v.pointer("/bank/questions").and_then(Value::as_array).cloned().expect("bank.questions")
}

fn stems(v: &Value) -> Vec<String> {
    questions(v).iter().map(|q| q["stem"].as_str().unwrap().to_string()).collect()
}

fn qid_of(q: &Value) -> Option<&str> {
    q.pointer("/published/qid").and_then(Value::as_str)
}

/// The question as content: what the server hashes (it drops `published` before comparing).
fn content(q: &Value) -> Value {
    let mut q = q.clone();
    if let Some(o) = q.as_object_mut() {
        o.remove("published");
    }
    q
}

fn base_questions(c: &Case) -> Vec<Value> {
    serde_json::from_slice::<Value>(&c.base).unwrap().pointer("/bank/questions").unwrap().as_array().unwrap().clone()
}

// ---- the plan's example, one tick at a time ------------------------------------------------------------

#[test]
fn all_ticked_sends_every_change_and_omits_the_removal() {
    let c = case();
    let (a, b, x, d) = (local_of(&c.link, QA), local_of(&c.link, QB), local_of(&c.link, QC), added_local(&c));
    let v = composed(&c, &[&a], &[&a, &b, &x, &d]);
    assert_eq!(stems(&v), ["A (changed)", "B", "D"]);
    let qs = questions(&v);
    // A is ours: the bank's id and the circle's qid in `published`.
    assert_eq!(qs[0]["id"].as_str(), Some(a.as_str()));
    assert_eq!(qid_of(&qs[0]), Some(QA));
    assert_eq!(qid_of(&qs[1]), Some(QB));
    // D is new to the circle: no `published`.
    assert_eq!(qs[2].get("published"), None);
    assert_eq!(qs[2]["id"].as_str(), Some(d.as_str()));
    // C is gone (its removal was ticked).
    assert!(!qs.iter().any(|q| q["stem"].as_str() == Some("C")));
    // The envelope is the send payload's.
    assert_eq!(v["schema_version"].as_u64(), Some(u64::from(SEND_SCHEMA)));
    assert_eq!(v.pointer("/bank/id").and_then(Value::as_str), Some(LOCAL));
    assert_eq!(v.pointer("/bank/name").and_then(Value::as_str), Some("Unit 2 quiz"));
}

#[test]
fn an_unticked_changed_question_is_sent_as_the_base_has_it() {
    let c = case();
    let (a, b, x, d) = (local_of(&c.link, QA), local_of(&c.link, QB), local_of(&c.link, QC), added_local(&c));
    let v = composed(&c, &[&a], &[&b, &x, &d]);
    assert_eq!(stems(&v), ["A", "B", "D"]);
    let q = &questions(&v)[0];
    // The base's raw question: its own id (the circle's qid), kept by `published`, and the base's stem.
    assert_eq!(q["id"].as_str(), Some(QA));
    assert_eq!(qid_of(q), Some(QA));
    assert_eq!(content(q), base_questions(&c)[0], "byte for byte the base's question");
}

#[test]
fn an_unticked_added_question_is_left_out() {
    let c = case();
    let (a, b, x, d) = (local_of(&c.link, QA), local_of(&c.link, QB), local_of(&c.link, QC), added_local(&c));
    let v = composed(&c, &[&a], &[&a, &b, &x]);
    assert_eq!(stems(&v), ["A (changed)", "B"]);
    assert!(!questions(&v).iter().any(|q| q["id"].as_str() == Some(d.as_str())));
}

#[test]
fn an_unticked_removal_is_put_back_after_the_question_before_it() {
    let c = case();
    let (a, d) = (local_of(&c.link, QA), added_local(&c));
    // A ticked; C's removal and D's addition unticked.
    let v = composed(&c, &[&a], &[&a]);
    assert_eq!(stems(&v), ["A (changed)", "B", "C"], "C_base comes after B, the question before it in the base");
    let q = questions(&v).remove(2);
    assert_eq!(q["id"].as_str(), Some(QC));
    assert_eq!(qid_of(&q), Some(QC));
    assert_eq!(content(&q), base_questions(&c)[2]);
    // The same with D ticked too: C_base still goes straight after B, before D.
    let v = composed(&c, &[&a], &[&a, &d]);
    assert_eq!(stems(&v), ["A (changed)", "B", "C", "D"]);
}

#[test]
fn all_unticked_sends_the_unchanged_question_and_restores_the_removal() {
    let c = case();
    let a = local_of(&c.link, QA);
    let v = composed(&c, &[&a], &[]);
    assert_eq!(stems(&v), ["A", "B", "C"]);
    let qs = questions(&v);
    // A and C are the base's; B is ours (it was unchanged, so it is sent either way).
    assert_eq!(qid_of(&qs[0]), Some(QA));
    assert_eq!(qs[0]["id"].as_str(), Some(QA));
    assert_eq!(qs[1]["id"].as_str(), Some(local_of(&c.link, QB).as_str()));
    assert_eq!(qid_of(&qs[2]), Some(QC));
}

#[test]
fn an_leading_unticked_removal_goes_to_the_front() {
    // Base [C, A, B]; the bank dropped C and changed A.
    let base_qs = vec![qfull(QC, "C"), qfull(QA, "A"), qfull(QB, "B")];
    let base = body_of(base_qs.clone());
    let (mut bank, link) = linked_from(base_qs, false);
    bank.questions[1].stem = "A (changed)".into();
    bank.questions.remove(0);
    let a = local_of(&link, QA);
    let v = composed(&Case { bank, link, base }, &[&a], &[&a]);
    assert_eq!(stems(&v), ["C", "A (changed)", "B"]);
    assert_eq!(questions(&v)[0]["id"].as_str(), Some(QC));
}

#[test]
fn two_adjacent_unticked_removals_keep_their_base_order() {
    // Base [A, B, C]; the bank dropped both B and C and added D.
    let base_qs = vec![qfull(QA, "A"), qfull(QB, "B"), qfull(QC, "C")];
    let base = body_of(base_qs.clone());
    let (mut bank, link) = linked_from(base_qs, false);
    bank.questions[0].stem = "A (changed)".into();
    let mut added = circle_theirs(&body_of(vec![qd("D")]), NOW).remove(0).q;
    added.id = crate::model::new_id();
    bank.questions.truncate(1);
    bank.questions.push(added);
    let (a, d) = (local_of(&link, QA), added_local(&Case { bank: bank.clone(), link: link.clone(), base: base.clone() }));
    let v = composed(&Case { bank, link, base }, &[&a], &[&a, &d]);
    assert_eq!(stems(&v), ["A (changed)", "B", "C", "D"]);
}

#[test]
fn an_unticked_unchanged_question_is_still_sent() {
    let c = case();
    let (a, b, x, d) = (local_of(&c.link, QA), local_of(&c.link, QB), local_of(&c.link, QC), added_local(&c));
    // B is unchanged and unticked; C's removal is ticked, so only B can show whether it was sent.
    let v = composed(&c, &[&a], &[&a, &x, &d]);
    assert_eq!(stems(&v), ["A (changed)", "B", "D"]);
    let q = &questions(&v)[1];
    assert_eq!(q["id"].as_str(), Some(b.as_str()), "ours: the bank's own question");
    assert_eq!(qid_of(q), Some(QB));
}

#[test]
fn a_difficulty_only_change_ticked_is_ours_and_unticked_is_the_bases_json() {
    let mut low_a = qfull(QA, "A");
    low_a["difficulty"] = json!("Low");
    let base_qs = vec![low_a, qfull(QB, "B")];
    let base = body_of(base_qs.clone());
    let (mut bank, link) = linked_from(base_qs, false);
    assert_eq!(bank.questions[0].difficulty, Some(Difficulty::Low));
    bank.questions[0].difficulty = Some(Difficulty::High);
    let a = local_of(&link, QA);
    let b = local_of(&link, QB);

    let ticked = composed(&Case { bank: bank.clone(), link: link.clone(), base: base.clone() }, &[&a], &[&a]);
    let q = &questions(&ticked)[0];
    assert_eq!(q["id"].as_str(), Some(a.as_str()), "ticked: ours");
    assert_eq!(q["difficulty"].as_str(), Some("High"), "ticked: the new difficulty");
    assert_eq!(qid_of(q), Some(QA));

    let unticked = composed(&Case { bank, link, base }, &[&a], &[&b]);
    let q = &questions(&unticked)[0];
    assert_eq!(q["id"].as_str(), Some(QA));
    assert_eq!(q["difficulty"].as_str(), Some("Low"), "unticked: the base's difficulty");
    assert_eq!(qid_of(q), Some(QA));
}

#[test]
fn a_figure_only_change_ticked_is_ours_and_unticked_is_the_bases_json() {
    // A change the link's snapshot can not see: `ours_changed` says unchanged, but the caller's body
    // comparison (which includes `figure`) lists it. The composer follows `changed`, never the snapshot.
    let mut base_a = qfull(QA, "A");
    base_a["figure"] = json!({"id": "number-line", "params": {"min": 1, "max": 9}});
    let base_qs = vec![base_a, qfull(QB, "B")];
    let base = body_of(base_qs.clone());
    let (mut bank, link) = linked_from(base_qs, false);
    let a = local_of(&link, QA);
    let b = local_of(&link, QB);
    let base_entry = link.base.iter().find(|x| x.local_id == a).unwrap();
    assert!(!super::merge::ours_changed(base_entry, &bank.questions[0]), "the snapshot does not see a figure");
    bank.questions[0].figure = Some(serde_json::from_value(json!({"id": "number-line", "params": {"min": 1, "max": 5}})).unwrap());
    let c = Case { bank, link, base };

    let ticked = composed(&c, &[&a], &[&a]);
    let q = &questions(&ticked)[0];
    assert_eq!(q["id"].as_str(), Some(a.as_str()), "ticked: ours");
    assert_eq!(q["figure"]["params"]["max"].as_u64(), Some(5), "ticked: the new figure");

    let unticked = composed(&c, &[&a], &[&b]);
    let q = &questions(&unticked)[0];
    assert_eq!(q["id"].as_str(), Some(QA));
    assert_eq!(q["figure"]["params"]["max"].as_u64(), Some(9), "unticked: the base's figure");
    assert_eq!(content(q), base_questions(&c)[0]);
}

// ---- a bounded link: no `base`, the full `qids` ------------------------------------------------------

#[test]
fn a_bounded_link_restores_an_unticked_removal_and_omits_a_ticked_one() {
    let c = case();
    let (a, x) = (local_of(&c.link, QA), local_of(&c.link, QC));
    let mut link = c.link.clone();
    link.base.clear(); // too big to keep: only the local id -> qid pairs remain
    assert_eq!(link.qids.len(), 3);
    let c = Case { bank: c.bank, link, base: c.base };

    // A's change ticked; C's removal unticked: C comes back from the base body.
    let v = composed(&c, &[&a], &[&a]);
    assert_eq!(stems(&v), ["A (changed)", "B", "C"]);
    assert_eq!(content(&questions(&v)[2]), base_questions(&c)[2]);

    // C ticked: omitted.
    let v = composed(&c, &[&a], &[&a, &x]);
    assert_eq!(stems(&v), ["A (changed)", "B"]);
}

#[test]
fn a_bounded_link_sends_an_unticked_changed_question_as_the_base_has_it() {
    let c = case();
    let a = local_of(&c.link, QA);
    let mut link = c.link.clone();
    link.base.clear();
    let c = Case { bank: c.bank, link, base: c.base };
    let v = composed(&c, &[&a], &[]);
    assert_eq!(stems(&v), ["A", "B", "C"]);
    assert_eq!(content(&questions(&v)[0]), base_questions(&c)[0]);
    assert_eq!(questions(&v)[0]["id"].as_str(), Some(QA));
}

#[test]
fn a_restored_removal_follows_its_base_predecessor_even_after_a_move() {
    // Base [A, B, C]; the bank moved B before A and dropped C. A move is not a change.
    let base_qs = vec![qfull(QA, "A"), qfull(QB, "B"), qfull(QC, "C")];
    let base = body_of(base_qs.clone());
    let (mut bank, link) = linked_from(base_qs, false);
    let qa = bank.questions.remove(0);
    let qb = bank.questions.remove(0);
    let _qc = bank.questions.remove(0);
    bank.questions.push(qb);
    bank.questions.push(qa);
    let v = composed(&Case { bank, link, base }, &[], &[]);
    assert_eq!(stems(&v), ["B", "C", "A"], "C goes straight after B, its base predecessor, although B moved");
}

// ---- errors: the caller falls back to a whole-bank send ------------------------------------------------

#[test]
fn a_changed_or_removed_question_with_no_base_question_is_an_error() {
    let c = case();
    let (a, b, d) = (local_of(&c.link, QA), local_of(&c.link, QB), added_local(&c));
    // The base body lacks C, and C's removal is unticked: there is nothing to put back.
    let short_base = body_of(vec![qfull(QA, "A"), qfull(QB, "B")]);
    let err = compose_partial(&c.link, &c.bank, &short_base, &set(&[&a]), &set(&[&a, &b, &d])).unwrap_err();
    assert!(err.contains("does not have one of the questions"), "{err}");

    // A changed question whose original is missing, unticked, is the same refusal.
    let base_qs = vec![qfull(QA, "A"), qfull(QB, "B"), qfull(QC, "C")];
    let (mut bank, link) = linked_from(base_qs, false);
    bank.questions[0].stem = "A (changed)".into();
    let short_base = body_of(vec![qfull(QB, "B"), qfull(QC, "C")]);
    let (a, b, x) = (local_of(&link, QA), local_of(&link, QB), local_of(&link, QC));
    let err = compose_partial(&link, &bank, &short_base, &set(&[&a]), &set(&[&b, &x])).unwrap_err();
    assert!(err.contains("does not have one of the questions"), "{err}");
}

#[test]
fn an_empty_bank_is_refused_with_payload_ofs_words() {
    let c = case();
    let mut bank = c.bank;
    bank.questions.clear();
    let err = compose_partial(&c.link, &bank, &c.base, &BTreeSet::new(), &BTreeSet::new()).unwrap_err();
    assert_eq!(err, "The bank has no questions. A circle bank needs at least one.");
    // The same words as the whole-bank send.
    assert_eq!(err, super::circle_links::payload_of(&c.link, &bank).unwrap_err());
}

#[test]
fn a_base_body_that_cannot_be_read_is_an_error() {
    let c = case();
    let empty = BTreeSet::new();
    for bad in [&b"not json"[..], &b"{\"schema_version\":2}"[..], &b"{\"bank\":{}}"[..]] {
        let err = compose_partial(&c.link, &c.bank, bad, &empty, &empty).unwrap_err();
        assert!(err.contains("could not be read"), "{err}");
    }
}

#[test]
fn a_changed_id_that_is_not_in_the_bank_is_ignored() {
    let c = case();
    let (a, b, x, d) = (local_of(&c.link, QA), local_of(&c.link, QB), local_of(&c.link, QC), added_local(&c));
    // A stray id (a question edited and removed since the summary was built) changes nothing.
    let v = composed(&c, &[&a, "local-bank-gone"], &[&a, &b, &x, &d]);
    assert_eq!(stems(&v), ["A (changed)", "B", "D"]);
}

// ---- the matrix ----------------------------------------------------------------------------------------

#[test]
fn every_tick_pattern_keeps_the_bank_questions_and_restores_the_rest() {
    let c = case();
    let (a, b, x, d) = (local_of(&c.link, QA), local_of(&c.link, QB), local_of(&c.link, QC), added_local(&c));
    let ours: Vec<Value> = c.bank.questions.iter().map(|q| serde_json::to_value(q).unwrap()).collect();
    let base = base_questions(&c);
    // The three changes: A changed, C removed, D added. (B is unchanged and never a choice.)
    for mask in 0..8u8 {
        let ticked = set(&[
            if mask & 1 != 0 { a.as_str() } else { "" },
            if mask & 2 != 0 { x.as_str() } else { "" },
            if mask & 4 != 0 { d.as_str() } else { "" },
            b.as_str(),
        ]);
        let ticked: BTreeSet<String> = ticked.into_iter().filter(|s| !s.is_empty()).collect();
        let out = compose_partial(&c.link, &c.bank, &c.base, &set(&[&a]), &ticked).expect("the composed payload");
        let v: Value = serde_json::from_slice(&out).unwrap();
        let qs = questions(&v);
        // Bits: 1 = A ticked, 2 = C's removal ticked, 4 = D ticked.
        let want: &[&str] = match mask {
            0b000 => &["A", "B", "C"],
            0b001 => &["A (changed)", "B", "C"],
            0b010 => &["A", "B"],
            0b011 => &["A (changed)", "B"],
            0b100 => &["A", "B", "C", "D"],
            0b101 => &["A (changed)", "B", "C", "D"],
            0b110 => &["A", "B", "D"],
            _ => &["A (changed)", "B", "D"],
        };
        assert_eq!(stems(&v), want, "mask {mask:03b}");
        // Kept: every bank question but an unticked added one. Restored: an unticked removal.
        let kept = 3 - usize::from(mask & 4 == 0);
        let restored = usize::from(mask & 2 == 0);
        assert_eq!(qs.len(), kept + restored, "mask {mask:03b}");
        for q in &qs {
            let is_ours = ours.iter().any(|o| content(o) == content(q));
            let is_base = base.iter().any(|b| *b == content(q));
            assert!(is_ours || is_base, "mask {mask:03b}: {q}");
            if let Some(qid) = qid_of(q) {
                assert!(qid == QA || qid == QB || qid == QC, "mask {mask:03b}: a qid the link knows");
            }
        }
    }
}
