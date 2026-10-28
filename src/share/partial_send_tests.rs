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
use super::partial_send::{BodyChanges, changes_against_body, compose_partial};
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
