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
use super::circle_links::{CircleLink, SEND_SCHEMA, after_partial_send, changes_since, circle_theirs};
use super::circles_api::SentBank;
use super::circles_tests::{BID, CID, NOW};
use super::partial_send::{BodyChanges, ComposedPartial, changes_against_body, compose_partial, compose_partial_parts};
use super::publish::QidLink;
use crate::model::{Difficulty, ImageRef, Kind, Question, QuestionBank};

/// A question only this bank has (added: no circle id).
const QD: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa4";
/// A fifth circle question, for the matrix's "extra unchanged question" variation.
const QE: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa5";

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

// ---- the change list against the base body (phase D3, piece 2) -----------------------------------------

/// The change list for a case.
fn changes_of(c: &Case) -> BodyChanges {
    changes_against_body(&c.link, &c.bank, &c.base).expect("the change list")
}

/// The changed ids when nothing was added or removed.
fn changed_ids(bank: &QuestionBank, link: &CircleLink, base: &[u8]) -> BTreeSet<String> {
    let got = changes_against_body(link, bank, base).expect("the change list");
    assert!(got.added.is_empty() && got.removed.is_empty(), "only a change: {got:?}");
    got.changed
}

#[test]
fn an_unchanged_bank_lists_no_changes() {
    let base_qs = vec![qfull(QA, "A"), qfull(QB, "B")];
    let base = body_of(base_qs.clone());
    let (bank, link) = linked_from(base_qs, false);
    assert_eq!(changes_against_body(&link, &bank, &base).unwrap(), BodyChanges::default());
}

#[test]
fn a_difference_the_server_stores_the_same_is_not_a_change() {
    // The server stores every text in NFC, without a byte-order mark, and a topic with its runs of white space
    // collapsed, trimmed, and the blank and repeated ones dropped (share-core `text::clean`; `merge`'s helpers
    // mirror it). The bank holds the same texts in the form the person typed.
    let mut base_a = qfull(QA, "Cafe\u{301}"); // decomposed
    base_a["explanation"] = json!("\u{FEFF}Because A."); // a byte-order mark the server drops
    base_a["topics"] = json!(["  Fractions  ", "Fractions"]); // collapsed, then a repeat
    base_a["kind"]["Mc"]["answers"][0]["text"] = json!("Cafe\u{301}"); // decomposed
    let base_qs = vec![base_a, qfull(QB, "B")];
    let base = body_of(base_qs.clone());
    let (mut bank, link) = linked_from(base_qs, false);
    bank.questions[0].stem = "Café".into(); // the composed form
    bank.questions[0].explanation = Some("Because A.".into());
    bank.questions[0].topics = vec!["Fractions".into()];
    if let Kind::Mc { answers } = &mut bank.questions[0].kind {
        answers[0].text = "Café".into();
    }
    assert_eq!(changes_against_body(&link, &bank, &base).unwrap(), BodyChanges::default());
}

#[test]
fn each_hashed_field_change_lists_the_question() {
    let base_qs = vec![qfull(QA, "A"), qfull(QB, "B")];
    let base = body_of(base_qs.clone());
    let (bank, link) = linked_from(base_qs, false);
    let a = bank.questions[0].id.clone();
    let mut edits: Vec<(&str, Question)> = Vec::new();
    let mut q = bank.questions[0].clone();
    q.stem = "A (changed)".into();
    edits.push(("stem", q));
    let mut q = bank.questions[0].clone();
    if let Kind::Mc { answers } = &mut q.kind {
        answers[1].text = "C".into();
    }
    edits.push(("choices", q));
    let mut q = bank.questions[0].clone();
    if let Kind::Mc { answers } = &mut q.kind {
        answers[0].correct = false;
        answers[1].correct = true;
    }
    edits.push(("answer key", q));
    let mut q = bank.questions[0].clone();
    q.points = Some(2.0);
    edits.push(("points", q));
    let mut q = bank.questions[0].clone();
    q.topics = vec!["Fractions".into()];
    edits.push(("topics", q));
    let mut q = bank.questions[0].clone();
    q.explanation = Some("Because B.".into());
    edits.push(("explanation", q));
    for (what, q) in edits {
        let mut bank = bank.clone();
        bank.questions[0] = q;
        assert_eq!(changed_ids(&bank, &link, &base), set(&[&a]), "{what}");
    }
}

#[test]
fn a_changed_picture_lists_the_question() {
    let base_qs = vec![qfull(QA, "A"), qfull(QB, "B")];
    let (mut bank, link) = linked_from(base_qs, false);
    let a = bank.questions[0].id.clone();
    let same_pic = ImageRef { file: "media/aaaa.png".into(), width: 4, height: 3 };
    let mut base_a = qfull(QA, "A");
    base_a["image"] = json!({"file": "media/aaaa.png", "width": 4, "height": 3});
    let base = body_of(vec![base_a, qfull(QB, "B")]);
    bank.questions[0].image = Some(same_pic.clone());
    assert!(changed_ids(&bank, &link, &base).is_empty(), "the same picture is not a change");
    // A different picture of the same size (the server hashes the file name and the size).
    bank.questions[0].image = Some(ImageRef { file: "media/bbbb.png".into(), ..same_pic });
    assert_eq!(changed_ids(&bank, &link, &base), set(&[&a]), "a different picture");
}

#[test]
fn a_figure_only_change_is_listed_though_the_snapshot_can_not_see_it() {
    let mut base_a = qfull(QA, "A");
    base_a["figure"] = json!({"id": "number-line", "params": {"min": 1, "max": 9}});
    let base_qs = vec![base_a, qfull(QB, "B")];
    let base = body_of(base_qs.clone());
    let (mut bank, link) = linked_from(base_qs, false);
    let a = bank.questions[0].id.clone();
    let entry = link.base.iter().find(|b| b.local_id == a).unwrap();
    assert!(!super::merge::ours_changed(entry, &bank.questions[0]), "the snapshot has no figure");
    bank.questions[0].figure = Some(serde_json::from_value(json!({"id": "number-line", "params": {"min": 1, "max": 5}})).unwrap());
    assert!(!super::merge::ours_changed(entry, &bank.questions[0]), "the snapshot still can not see it");
    assert_eq!(changed_ids(&bank, &link, &base), set(&[&a]), "the body comparison sees the figure");
}

#[test]
fn a_difficulty_only_change_is_listed_also_when_the_snapshot_does_not_know_it() {
    let mut base_a = qfull(QA, "A");
    base_a["difficulty"] = json!("High");
    let base_qs = vec![base_a, qfull(QB, "B")];
    let base = body_of(base_qs.clone());
    let (mut bank, mut link) = linked_from(base_qs, false);
    assert_eq!(bank.questions[0].difficulty, Some(Difficulty::High));
    bank.questions[0].difficulty = Some(Difficulty::Low);
    let a = bank.questions[0].id.clone();

    // The snapshot knows the difficulty and sees the change.
    let entry = link.base.iter().find(|b| b.local_id == a).unwrap();
    assert!(super::merge::ours_changed(entry, &bank.questions[0]));
    assert_eq!(changed_ids(&bank, &link, &base), set(&[&a]));

    // A base an earlier build wrote has no record of it: `ours_changed` can not see the change, the body can.
    link.base.iter_mut().find(|b| b.local_id == a).unwrap().snap.difficulty = None;
    let entry = link.base.iter().find(|b| b.local_id == a).unwrap();
    assert!(!super::merge::ours_changed(entry, &bank.questions[0]), "the snapshot does not know the difficulty");
    assert_eq!(changed_ids(&bank, &link, &base), set(&[&a]), "the body has the difficulty");
}

#[test]
fn an_added_question_is_listed_as_added_not_changed() {
    let c = case();
    let d = added_local(&c);
    let got = changes_of(&c);
    assert_eq!(got.added, set(&[&d]));
    assert_eq!(got.changed, set(&[&local_of(&c.link, QA)]));
    assert_eq!(got.removed, set(&[&local_of(&c.link, QC)]));
}

#[test]
fn a_removed_question_is_listed_also_when_the_link_keeps_no_base() {
    let base_qs = vec![qfull(QA, "A"), qfull(QB, "B"), qfull(QC, "C")];
    let base = body_of(base_qs.clone());
    let (mut bank, mut link) = linked_from(base_qs, false);
    let (a, x) = (local_of(&link, QA), local_of(&link, QC));
    bank.questions.retain(|q| q.id != x);
    bank.questions.iter_mut().find(|q| q.id == a).unwrap().stem = "A (changed)".into();
    link.base.clear(); // too big to keep: only the local id -> qid pairs remain
    let got = changes_against_body(&link, &bank, &base).unwrap();
    assert_eq!(got.removed, set(&[&x]));
    assert_eq!(got.changed, set(&[&a]));
    assert!(got.added.is_empty());
}

#[test]
fn an_unreadable_base_body_is_an_error_and_says_everything_is_sent() {
    let c = case();
    for bad in [&b"not json"[..], &b"{\"schema_version\":2}"[..], &b"{\"bank\":{}}"[..]] {
        let err = changes_against_body(&c.link, &c.bank, bad).unwrap_err();
        assert!(err.contains("could not be read"), "{err}");
        assert!(err.contains("Send everything instead"), "{err}");
    }
}

#[test]
fn a_linked_question_missing_from_the_base_is_an_error() {
    // A bank question whose circle id the body does not have: its content can not be compared. (A removal the body
    // lacks is different: it is still listed, and only an unticked one makes the composer refuse — see
    // `a_removal_the_base_body_lacks_is_still_listed`.)
    let base_qs = vec![qfull(QA, "A"), qfull(QB, "B")];
    let (bank, link) = linked_from(base_qs, false);
    let short = body_of(vec![qfull(QA, "A")]);
    let err = changes_against_body(&link, &bank, &short).unwrap_err();
    assert!(err.contains("does not have one of this copy's linked questions"), "{err}");
    assert!(err.contains("Send everything instead"), "{err}");
}

/// The plan's matrix, optionally with a fifth unchanged question: base [A, B, C(, E)]; bank [A', B, D(, E)].
fn matrix_case(with_e: bool) -> Case {
    let mut base_qs = vec![qfull(QA, "A"), qfull(QB, "B"), qfull(QC, "C")];
    if with_e {
        base_qs.push(qfull(QE, "E"));
    }
    let base = body_of(base_qs.clone());
    let (mut bank, link) = linked_from(base_qs, false);
    let (a, x) = (local_of(&link, QA), local_of(&link, QC));
    bank.questions.iter_mut().find(|q| q.id == a).unwrap().stem = "A (changed)".into();
    bank.questions.retain(|q| q.id != x);
    let mut added = circle_theirs(&body_of(vec![qd("D")]), NOW).remove(0).q;
    added.id = crate::model::new_id();
    bank.questions.push(added);
    Case { bank, link, base }
}

#[test]
fn the_matrix_lists_exactly_the_changed_added_and_removed_questions() {
    for with_e in [false, true] {
        let c = matrix_case(with_e);
        let (a, x, d) = (local_of(&c.link, QA), local_of(&c.link, QC), added_local(&c));
        let got = changes_of(&c);
        assert_eq!(got, BodyChanges { changed: set(&[&a]), added: set(&[&d]), removed: set(&[&x]) }, "with_e {with_e}");
        // A move (and the extra unchanged question) changes nothing.
        let mut moved = c;
        moved.bank.questions.rotate_left(1);
        assert_eq!(changes_of(&moved), got, "rotated, with_e {with_e}");
        moved.bank.questions.reverse();
        assert_eq!(changes_of(&moved), got, "reversed, with_e {with_e}");
    }
}

#[test]
fn a_removal_the_base_body_lacks_is_still_listed() {
    // (critic minor) The list does not need the base's raw question to name a removal; the composer refuses only
    // when the removal is unticked and so would have to be put back.
    let c = case();
    let (a, x, d) = (local_of(&c.link, QA), local_of(&c.link, QC), added_local(&c));
    let short = body_of(vec![qfull(QA, "A"), qfull(QB, "B")]);
    let got = changes_against_body(&c.link, &c.bank, &short).expect("a removal without a base raw is still a removal");
    assert!(got.removed.contains(&x), "{got:?}");
    // ticked: the removal needs nothing from the base, so the send is composed
    let out = compose_partial(&c.link, &c.bank, &short, &got.changed, &set(&[&a, &x, &d])).expect("ticked: no base raw needed");
    let v: Value = serde_json::from_slice(&out).unwrap();
    assert_eq!(stems(&v), ["A (changed)", "B", "D"]);
    // unticked: it would have to be put back, and the base body lacks it
    let err = compose_partial(&c.link, &c.bank, &short, &got.changed, &set(&[&a, &d])).unwrap_err();
    assert!(err.contains("does not have one of the questions"), "{err}");
}

#[test]
fn an_empty_bank_is_refused_with_the_composers_words() {
    // (critic minor) The summary cannot offer a send that the composer would refuse.
    let Case { mut bank, link, base } = case();
    bank.questions.clear();
    let err = changes_against_body(&link, &bank, &base).unwrap_err();
    assert_eq!(err, "The bank has no questions. A circle bank needs at least one.");
}

// ---- what the link keeps after a partial send (phase D3, piece 3) ---------------------------------------

/// The circle id the send's answer gives the question this bank added (at D's place in the payload).
const QNEW: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa6";

/// The answer a send of `composed` gets: version 2, one qid per composed question in payload order, naming a
/// question new to the circle.
fn sent_of(composed: &ComposedPartial) -> SentBank {
    let qids = composed.questions.iter().map(|c| c.qid.clone().unwrap_or_else(|| QNEW.to_string())).collect();
    SentBank { version: 2, version_id: format!("{:064x}", 2), qids, ..SentBank::default() }
}

/// The body the circle would serve for a version made from `composed`: every question's `id` is the circle id it
/// got (`published` is not part of a served question). The next summary in the app reads this, not the payload.
fn served(composed: &ComposedPartial, sent: &SentBank) -> Vec<u8> {
    let mut v: Value = serde_json::from_slice(&composed.payload).expect("the payload is JSON");
    let qs = v.pointer_mut("/bank/questions").and_then(Value::as_array_mut).expect("bank.questions");
    for (i, q) in qs.iter_mut().enumerate() {
        let qid = composed.questions.get(i).and_then(|c| c.qid.clone()).or_else(|| sent.qids.get(i).cloned()).expect("a qid for each composed question");
        let o = q.as_object_mut().expect("a question object");
        o.remove("published");
        o.insert("id".into(), Value::String(qid));
    }
    serde_json::to_vec(&v).expect("the served body")
}

/// Base [A, B, C]; this bank [A', B] (A changed, C removed): no added question, for the edit-during-send case.
fn abc_case() -> Case {
    let base_qs = vec![qfull(QA, "A"), qfull(QB, "B"), qfull(QC, "C")];
    let base = body_of(base_qs.clone());
    let (mut bank, link) = linked_from(base_qs, false);
    bank.questions[0].stem = "A (changed)".into();
    bank.questions.pop();
    Case { bank, link, base }
}

#[test]
fn after_a_partial_send_the_next_summary_against_the_composed_payload_lists_the_unticked_change() {
    // Everything but D (the added question) ticked: the composed payload holds only linked questions, which keep
    // their circle ids in `published`, so it can be read as the next base.
    let c = case();
    let (a, x, d) = (local_of(&c.link, QA), local_of(&c.link, QC), added_local(&c));
    let composed = compose_partial_parts(&c.link, &c.bank, &c.base, &set(&[&a, &d]), &set(&[&a, &x])).expect("the composed payload");
    let sent = sent_of(&composed);
    let link = after_partial_send(&c.link, &c.bank, &composed.questions, &sent, vec![d.clone()]);
    let next = changes_against_body(&link, &c.bank, &composed.payload).expect("the next summary");
    assert_eq!(next, BodyChanges { added: set(&[&d]), ..Default::default() });
    let count = changes_since(&link, &c.bank);
    assert_eq!((count.added, count.changed, count.removed), (1, 0, 0), "changes_since counts the same one: {count:?}");
}

#[test]
fn after_a_partial_send_the_next_summary_lists_each_kind_of_unticked_change() {
    // The same invariant for a changed question, a removal and an added one left unticked. The next base is the
    // version the circle serves (a question the send added is named by the answer's qid, so the payload's own local
    // id is not what the next read compares against).
    for which in ["changed", "removed", "added"] {
        let c = case();
        let (a, x, d) = (local_of(&c.link, QA), local_of(&c.link, QC), added_local(&c));
        let (ticked, unticked) = match which {
            "changed" => (set(&[&x, &d]), a.clone()),
            "removed" => (set(&[&a, &d]), x.clone()),
            _ => (set(&[&a, &x]), d.clone()),
        };
        let composed = compose_partial_parts(&c.link, &c.bank, &c.base, &set(&[&a, &d]), &ticked).expect("the composed payload");
        let sent = sent_of(&composed);
        let link = after_partial_send(&c.link, &c.bank, &composed.questions, &sent, vec![unticked]);
        let next = changes_against_body(&link, &c.bank, &served(&composed, &sent)).expect("the next summary");
        let want = match which {
            "changed" => BodyChanges { changed: set(&[&a]), ..Default::default() },
            "removed" => BodyChanges { removed: set(&[&x]), ..Default::default() },
            _ => BodyChanges { added: set(&[&d]), ..Default::default() },
        };
        assert_eq!(next, want, "{which}");
        let count = changes_since(&link, &c.bank);
        let got = match which {
            "changed" => (count.changed, count.added, count.removed),
            "removed" => (count.removed, count.changed, count.added),
            _ => (count.added, count.changed, count.removed),
        };
        assert_eq!(got, (1, 0, 0), "{which}: changes_since counts the same one: {count:?}");
    }
}

#[test]
fn a_ticked_removals_pair_is_dropped_and_an_unticked_ones_kept() {
    let c = case();
    let (a, x, d) = (local_of(&c.link, QA), local_of(&c.link, QC), added_local(&c));

    // C's removal ticked: no pair, no snapshot entry.
    let composed = compose_partial_parts(&c.link, &c.bank, &c.base, &set(&[&a]), &set(&[&a, &x, &d])).expect("ticked, composed");
    let sent = sent_of(&composed);
    let link = after_partial_send(&c.link, &c.bank, &composed.questions, &sent, vec![]);
    assert!(link.qid_of(&x).is_none(), "a ticked removal's pair is gone");
    assert!(!link.qids.iter().any(|p| p.local == x));
    assert!(!link.base.iter().any(|b| b.local_id == x));

    // C's removal unticked: the pair stays and the base snapshot holds the base's own question.
    let composed = compose_partial_parts(&c.link, &c.bank, &c.base, &set(&[&a]), &set(&[&a, &d])).expect("unticked, composed");
    let sent = sent_of(&composed);
    let link = after_partial_send(&c.link, &c.bank, &composed.questions, &sent, vec![x.clone()]);
    assert_eq!(link.qid_of(&x), Some(QC), "an unticked removal keeps its pair");
    let base_c = link.base.iter().find(|b| b.local_id == x).expect("its snapshot entry");
    assert_eq!(base_c.snap.stem, "C");
}

#[test]
fn a_bank_edited_during_the_send_is_not_stale_and_its_edit_shows_as_unsent() {
    let c = abc_case();
    let (a, b, x) = (local_of(&c.link, QA), local_of(&c.link, QB), local_of(&c.link, QC));
    // The bank was edited before the send started: its mark moves on with it.
    let mut started = c.bank.clone();
    started.modified += 100;
    let composed = compose_partial_parts(&c.link, &started, &c.base, &set(&[&a]), &set(&[&a])).expect("the composed payload");
    let sent = sent_of(&composed);

    // The bank moves on again while the send is in flight.
    let mut edited = started.clone();
    edited.modified += 10;
    edited.questions.iter_mut().find(|q| q.id == b).unwrap().stem = "B (edited during the send)".into();

    // The link is told the bank as it was when the send started, never the edited one.
    let link = after_partial_send(&c.link, &started, &composed.questions, &sent, vec![]);
    assert_eq!(link.bank_modified, started.modified, "the bank as the send started");
    assert_eq!(link.bank_print, super::circle_links::bank_print(&started), "its print, not the composed version's");
    assert!(!link.stale_for(&edited), "a bank newer than the send started is not put back");
    // The send does not leave a print of a version the bank never had: its edit is unsent, not a stale bank.
    let next = changes_against_body(&link, &edited, &served(&composed, &sent)).expect("the next summary");
    assert!(next.changed.contains(&b), "the edit made during the send is unsent: {next:?}");
    assert!(!next.changed.contains(&a), "what was sent is not listed again: {next:?}");
    assert!(next.removed.contains(&x), "the unticked removal is still unsent too: {next:?}");
    assert_eq!(changes_since(&link, &edited).changed, 1);
}

#[test]
fn the_base_snapshot_is_the_composed_version_and_held_back_the_unticked_set() {
    let c = case();
    let (a, b, x, d) = (local_of(&c.link, QA), local_of(&c.link, QB), local_of(&c.link, QC), added_local(&c));
    // A unticked; C's removal and D ticked.
    let composed = compose_partial_parts(&c.link, &c.bank, &c.base, &set(&[&a, &d]), &set(&[&x, &d])).expect("the composed payload");
    let sent = sent_of(&composed);
    let held = vec![a.clone()];
    let link = after_partial_send(&c.link, &c.bank, &composed.questions, &sent, held.clone());

    assert_eq!(link.held_back, held, "exactly the unticked set");
    let base_a = link.base.iter().find(|x| x.local_id == a).expect("A's snapshot entry");
    assert_eq!(base_a.snap.stem, "A", "A unticked: the base's version, not the bank's");
    assert_ne!(c.bank.questions[0].stem, base_a.snap.stem);
    assert_eq!(link.base.iter().find(|x| x.local_id == b).expect("B's snapshot entry").snap.stem, "B", "B unchanged: ours");
    // The unticked change still compares as changed against the snapshot.
    assert_eq!(changes_since(&link, &c.bank).changed, 1);
    // A ticked added question is linked by the qid the send's answer gave it.
    assert_eq!(link.qid_of(&d), Some(QNEW));
    // And the copy is in step with the version the send made.
    assert_eq!((link.base_version, link.base_version_id.as_str()), (sent.version, sent.version_id.as_str()));
}

#[test]
fn a_link_written_before_held_back_loads_with_none() {
    let c = case();
    let mut v = serde_json::to_value(&c.link).expect("the link as JSON");
    assert!(v.as_object_mut().unwrap().remove("held_back").is_some(), "this build writes held_back");
    let old: CircleLink = serde_json::from_value(v).expect("a link from before phase D3");
    assert!(old.held_back.is_empty(), "absent reads as none");
    assert_eq!(old, c.link, "the rest of the link is the same");

    // Lenient, as the other new fields: a value this build can not read is none, or its readable entries; never the
    // link with it.
    let mut v = serde_json::to_value(&c.link).expect("the link as JSON");
    v["held_back"] = json!("not a list");
    let odd: CircleLink = serde_json::from_value(v).expect("the link still loads");
    assert!(odd.held_back.is_empty());
    let mut v = serde_json::to_value(&c.link).expect("the link as JSON");
    v["held_back"] = json!([1, "kept", null]);
    let odd: CircleLink = serde_json::from_value(v).expect("the link still loads");
    assert_eq!(odd.held_back, ["kept"]);
}

#[test]
fn after_partial_send_is_pure_and_writes_nothing() {
    let c = case();
    let (a, x, d) = (local_of(&c.link, QA), local_of(&c.link, QC), added_local(&c));
    let link_before = c.link.clone();
    let bank_before = serde_json::to_value(&c.bank).expect("the bank as JSON");
    let composed = compose_partial_parts(&c.link, &c.bank, &c.base, &set(&[&a, &d]), &set(&[&a, &x, &d])).expect("the composed payload");
    let sent = sent_of(&composed);
    let one = after_partial_send(&c.link, &c.bank, &composed.questions, &sent, vec![a.clone()]);
    let two = after_partial_send(&c.link, &c.bank, &composed.questions, &sent, vec![a.clone()]);
    assert_eq!(c.link, link_before, "the link it was given is untouched");
    assert_eq!(serde_json::to_value(&c.bank).unwrap(), bank_before, "the bank it was given is untouched");
    assert_eq!(one, two, "the same inputs give the same link");
    assert_eq!(one.held_back, vec![a]);
}

#[test]
fn a_whole_bank_send_clears_held_back() {
    let c = case();
    let mut link = c.link.clone();
    link.held_back = vec![c.bank.questions[0].id.clone()];
    let sent = SentBank { version: 3, version_id: format!("{:064x}", 3), qids: c.bank.questions.iter().map(|_| QNEW.to_string()).collect(), ..SentBank::default() };
    let after = super::circle_links::after_send(&link, &c.bank, &sent);
    assert!(after.held_back.is_empty(), "everything was sent, so nothing is held back");
}

/// The qid the server's answer gives a question the composed pair knew under another id (the answer is authoritative).
const QSERVER: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa7";

#[test]
fn a_restored_removal_this_build_cannot_read_still_composes_and_keeps_its_pair() {
    // A newer client wrote C with a Kind this build does not know. The payload must still carry C raw, byte for
    // byte; only the base snapshot is left out.
    let mut c_bad = qfull(QC, "C");
    c_bad["kind"] = json!({"NewerKind": {"parts": []}});
    assert!(serde_json::from_value::<Question>(c_bad.clone()).is_err(), "this build can not read it");
    let base = body_of(vec![qfull(QA, "A"), qfull(QB, "B"), c_bad.clone()]);
    let (bank, mut link) = linked_from(vec![qfull(QA, "A"), qfull(QB, "B")], false);
    let gone = "local-c-gone".to_string();
    link.qids.push(QidLink { local: gone.clone(), qid: QC.into() });

    let composed = compose_partial_parts(&link, &bank, &base, &set(&[]), &set(&[])).expect("the payload is still composed");
    let qs = questions(&serde_json::from_slice::<Value>(&composed.payload).expect("the payload is JSON"));
    let c = qs.iter().find(|q| q["id"].as_str() == Some(QC)).expect("C is restored");
    assert_eq!(content(c), c_bad, "the raw question, although this build can not read it");
    let cq = composed.questions.iter().find(|c| c.local == gone).expect("C's composed question");
    assert!(cq.question.is_none(), "no Question this build can read");
    assert_eq!(cq.qid.as_deref(), Some(QC), "but its pair is known");

    let sent = sent_of(&composed);
    let link = after_partial_send(&link, &bank, &composed.questions, &sent, vec![gone.clone()]);
    assert_eq!(link.qid_of(&gone), Some(QC), "the pair is kept");
    assert!(!link.base.iter().any(|b| b.local_id == gone), "no snapshot for a question this build can not read");
    // The next summary still lists it as removed.
    let next = changes_against_body(&link, &bank, &composed.payload).expect("the next summary");
    assert_eq!(next.removed, set(&[&gone]));
}

#[test]
fn a_short_or_empty_sent_qids_list_keeps_linked_qids_and_never_borrows_one() {
    let c = case();
    let (a, b, x, d) = (local_of(&c.link, QA), local_of(&c.link, QB), local_of(&c.link, QC), added_local(&c));
    // C's removal unticked and D ticked: composed = [A', B, C_base, D'].
    let composed = compose_partial_parts(&c.link, &c.bank, &c.base, &set(&[&a, &d]), &set(&[&a, &d])).expect("the composed payload");
    assert_eq!(composed.questions.len(), 4);
    for qids in [Vec::new(), vec![QB.to_string()], vec![QB.to_string(), QC.to_string()]] {
        let mut sent = sent_of(&composed);
        sent.qids = qids.clone();
        let link = after_partial_send(&c.link, &c.bank, &composed.questions, &sent, vec![x.clone()]);
        assert_eq!(link.qid_of(&a), Some(QA), "a linked question keeps its own qid, list {qids:?}");
        assert_eq!(link.qid_of(&b), Some(QB), "list {qids:?}");
        assert_eq!(link.qid_of(&x), Some(QC), "a restored removal keeps its pair, list {qids:?}");
        assert!(link.qid_of(&d).is_none(), "an addition never borrows another question's qid, list {qids:?}");
    }
}

#[test]
fn the_sends_qid_wins_over_the_composed_pair_when_the_arrays_align() {
    let c = case();
    let (a, x, d) = (local_of(&c.link, QA), local_of(&c.link, QC), added_local(&c));
    // Everything ticked: composed = [A', B, D], three questions for the answer's three qids.
    let composed = compose_partial_parts(&c.link, &c.bank, &c.base, &set(&[&a, &d]), &set(&[&a, &x, &d])).expect("the composed payload");
    let mut sent = sent_of(&composed);
    assert_eq!(sent.qids.len(), composed.questions.len());
    assert_eq!(sent.qids[0], QA);
    sent.qids[0] = QSERVER.to_string();
    let link = after_partial_send(&c.link, &c.bank, &composed.questions, &sent, vec![]);
    assert_eq!(link.qid_of(&a), Some(QSERVER), "the answer's qid is authoritative");
    assert_eq!(link.base.iter().find(|b| b.local_id == a).expect("A's snapshot entry").qid, QSERVER);
}

// ---- round 13 (product critic 11 A/B): a fetched, re-encoded picture is the circle's picture ----

fn reencoded() -> (QuestionBank, CircleLink, Vec<u8>, String, String) {
    // The circle's PNG (circle_sha) was fetched and kept here as WebP (local_sha): different file names, same picture.
    let circle_sha = format!("{:064x}", 0xa1u8);
    let local_sha = format!("{:064x}", 0xb2u8);
    let local_file = format!("media/{local_sha}.webp");
    let mut base_q = qfull(QA, "A");
    base_q["image"] = json!({ "file": format!("media/{circle_sha}.png"), "width": 1, "height": 1 });
    let base = body_of(vec![base_q.clone()]);
    let (mut bank, mut link) = linked_from(vec![base_q], false);
    bank.questions[0].image = Some(ImageRef { file: local_file.clone(), width: 1, height: 1 });
    // the link records the pair the fetch wrote: this store's WebP, the circle's sha
    let id = bank.questions[0].id.clone();
    let b = link.base.iter_mut().find(|b| b.local_id == id).unwrap();
    b.local_image = Some(local_file.clone());
    b.snap.image = Some(circle_sha.clone());
    (bank, link, base, local_file, circle_sha)
}

#[test]
fn a_fetched_reencoded_picture_is_not_a_change() {
    let (bank, link, base, _, _) = reencoded();
    let ch = changes_against_body(&link, &bank, &base).expect("the change list");
    assert!(ch.changed.is_empty(), "a picture the link knows is the circle's is listed as a change: {:?}", ch.changed);
}

#[test]
fn a_fetched_reencoded_picture_is_sent_as_the_bases_picture() {
    // Fixing only the summary would hide the rewrite: the payload must carry the base body's picture identity, or the
    // server stores the WebP under a new name and the circle's picture changes for everyone.
    let (mut bank, link, base, local_file, circle_sha) = reencoded();
    bank.questions[0].stem = "A (changed)".into();
    let changed = changes_against_body(&link, &bank, &base).expect("the change list").changed;
    assert!(changed.contains(&bank.questions[0].id), "the text change is still a change");
    let composed = compose_partial_parts(&link, &bank, &base, &changed, &changed).expect("the composed payload");
    let v: Value = serde_json::from_slice(&composed.payload).unwrap();
    assert_eq!(
        v.pointer("/bank/questions/0/image/file").and_then(Value::as_str),
        Some(format!("media/{circle_sha}.png").as_str()),
        "the payload carries this store's {local_file}, so the server counts the picture as changed"
    );
    assert_eq!(v.pointer("/bank/questions/0/stem").and_then(Value::as_str), Some("A (changed)"));
}

#[test]
fn a_genuine_picture_change_is_still_a_change_and_is_sent_as_ours() {
    let (mut bank, link, base, _, _) = reencoded();
    let other = format!("media/{}.webp", format!("{:064x}", 0xc3u8));
    bank.questions[0].image = Some(ImageRef { file: other.clone(), width: 1, height: 1 });
    let changed = changes_against_body(&link, &bank, &base).expect("the change list").changed;
    assert!(changed.contains(&bank.questions[0].id), "a different local picture is a change");
    let composed = compose_partial_parts(&link, &bank, &base, &changed, &changed).expect("the composed payload");
    let v: Value = serde_json::from_slice(&composed.payload).unwrap();
    assert_eq!(v.pointer("/bank/questions/0/image/file").and_then(Value::as_str), Some(other.as_str()), "the bank's own picture is sent");
}

#[test]
fn a_sent_reencoded_picture_keeps_the_circles_name_in_the_link() {
    // The composer sent this store's WebP under the base body's picture identity; the rebuilt record must say so,
    // or the next summary and Compare call the picture changed again.
    let (bank, link, base, local_file, circle_sha) = reencoded();
    let composed = compose_partial_parts(&link, &bank, &base, &BTreeSet::new(), &BTreeSet::new()).expect("the composed payload");
    let sent = sent_of(&composed);
    let after = after_partial_send(&link, &bank, &composed.questions, &sent, vec![]);
    let b = after.base.iter().find(|b| b.local_id == bank.questions[0].id).expect("a record");
    assert_eq!(b.local_image.as_deref(), Some(local_file.as_str()));
    assert_eq!(b.snap.image.as_deref(), Some(circle_sha.as_str()), "the link recorded the WebP's own hash, not the identity the send used");
    assert_eq!(changes_since(&after, &bank).changed, 0);
    assert_eq!(super::question_history::picture_sha_on_circle(&after, &bank.questions[0]).as_deref(), Some(circle_sha.as_str()));
}

#[test]
fn a_bounded_link_names_a_fetched_reencoded_picture() {
    // (product critic 11 B) A circle bank over the base cap stores no base record, so the record that named the
    // fetched picture is discarded; nothing repopulated the link map and Compare fell back to the WebP's own hash.
    let n = super::merge::BASE_MAX_QUESTIONS + 1;
    let circle_sha = format!("{:064x}", 0xa1u8);
    let local_sha = format!("{:064x}", 0xb2u8);
    let local_file = format!("media/{local_sha}.webp");
    let mut questions = Vec::new();
    let mut base = Vec::new();
    for i in 0..n {
        let qid = format!("{:032x}", i + 1);
        let mut q = qfull(&qid, &format!("Q{i}"));
        q["id"] = json!(format!("local-{i}"));
        let q: Question = serde_json::from_value(q).unwrap();
        if i == 0 {
            let mut q = q.clone();
            q.image = Some(ImageRef { file: local_file.clone(), width: 1, height: 1 });
            base.push(super::merge::base_of(&q, &qid, "fp", Some(circle_sha.clone())));
            questions.push(q);
        } else {
            base.push(super::merge::base_of(&q, &qid, "fp", None));
            questions.push(q);
        }
    }
    let head = super::circles_api::BankHead { id: BID.into(), title: "Cells".into(), license: "CC-BY-4.0".into(), version: 9, version_id: format!("{:032x}", 9), ..super::circles_api::BankHead::default() };
    let link = CircleLink::from_head(CID, "C", &head, LOCAL, base, NOW);
    assert!(link.base.is_empty(), "the base is over the cap");
    assert!(link.picture_files.iter().any(|(f, s)| f == &local_file && s == &circle_sha), "the fetched picture has no name: {:?}", link.picture_files);
    let bank = QuestionBank { id: LOCAL.into(), name: "B".into(), questions, created: 0, modified: 5, details: Default::default() };
    assert_eq!(super::question_history::picture_sha_on_circle(&link, &bank.questions[0]).as_deref(), Some(circle_sha.as_str()), "Compare calls the circle's own picture a different one");
}
