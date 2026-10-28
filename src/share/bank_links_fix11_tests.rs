//! Phase D1, round 11 (critics 83 and 84): each test written first against the code it pins. A question's difficulty is
//! merged, counted and shown like its other fields; the person's own version right after the one a send was based on is
//! the merge's base also when colleagues sent more since; a note that a send or restore may not have arrived yet is
//! rewritten by the check that settles it; "You sent this" is said only of what that send changed; topics keep the joiners
//! the server keeps; figure values the server drops are not compared; an earlier build's base and a send by an account not
//! known are read as they were meant.

use std::sync::Arc;

use serde_json::{Value, json};

use super::bank_links_fix10_tests::*;
use super::bank_links_fix4_tests::*;
use super::bank_links_fix8_tests::*;
use super::bank_links_fix9_tests::*;
use super::bank_links_fix_tests::*;
use super::bank_links_tests::*;
use super::canon;
use super::circle_edit::*;
use super::circle_links::{CircleLink, changes_since, circle_theirs};
use super::circles::CirclesModel;
use super::circles_tests::*;
use super::merge::{RowKind, build_plan};
use super::publish::AccountSnapshot;
use crate::model::{Difficulty, QuestionBank};

/// A's page (at version 3), olga signed in with her display name known.
pub(super) fn rig() -> Rig {
    let (mut m, s, mut banks) = two_at_three();
    s.keep("Get", &format!("/circles/{CID}"), 200, detail("owner", json!({})));
    let ctx = ctx();
    let mut acct = FakeAccount { open: true, ..FakeAccount::default() };
    frames(&mut m, &mut banks, &ctx, &mut acct, &olga_named(&s), 2);
    (m, s, banks, ctx, acct)
}

pub(super) fn rated(id: &str, stem: &str, d: Option<&str>) -> Value {
    let mut q = qjson(id, stem);
    q["difficulty"] = json!(d);
    q
}

pub(super) fn host_a(m: &mut CirclesModel) {
    m.edit.host = Some(EditHost { local_id: LOCAL.into(), back: "< Back to Unit 2 quiz".into() });
}

pub(super) fn diff_of(m: &CirclesModel, field: &str) -> Option<(String, String, String)> {
    match &m.edit.view {
        EditView::Merge(v) => match &v.phase {
            MergePhase::Review(r) => r.plan.rows.iter().flat_map(|r| r.diffs.iter()).find(|d| d.field == field).map(|d| (d.base.clone(), d.ours.clone(), d.theirs.clone())),
            _ => None,
        },
        _ => None,
    }
}

pub(super) fn notes(m: &CirclesModel) -> Vec<String> {
    match &m.edit.view {
        EditView::Merge(v) => match &v.phase {
            MergePhase::Review(r) => r.plan.rows.iter().filter_map(|r| r.note.clone()).collect(),
            _ => Vec::new(),
        },
        _ => Vec::new(),
    }
}

// ---- c83 I1: a question's difficulty ----------------------------------------------------------------------------------

/// A colleague made question One "High": Get latest takes it (never "Unchanged"), and the bank does not send it back.
#[test]
fn c83_i1_a_colleagues_difficulty_change_is_got_and_never_sent_back() {
    let (mut m, s, mut banks, ctx, mut acct) = rig();
    let page = get_latest(&mut m, &s, &mut banks, &ctx, &mut acct, served_qs(4, vec![rated(QA, "One", Some("High"))], false));
    let k = kinds(&m).unwrap();
    assert_eq!((count(&k, RowKind::TheirsChanged), count(&k, RowKind::Unchanged)), (1, 0), "{page}");
    assert_eq!(diff_of(&m, "Difficulty"), Some(("none".into(), "none".into(), "Hard".into())));
    assert!(page.contains("Difficulty") && page.contains("Hard"), "{page}");
    apply(&mut m, &s, &mut banks, &ctx, &mut acct);
    assert_eq!(banks[0].questions[0].difficulty, Some(Difficulty::High));
    assert_eq!(changes(&mut m, &banks[0].clone()), (0, 0, 0), "nothing to send back");
}

/// The teacher's own difficulty-only change is an unsent change: counted on the strip and on the Send page.
#[test]
fn c83_i1_the_teachers_own_difficulty_change_is_counted_and_offered_for_sending() {
    let (mut m, s, mut banks, ctx, mut acct) = rig();
    s.send_base_404(true);
    banks[0].questions[0].difficulty = Some(Difficulty::Low);
    banks[0].modified += 5;
    assert_eq!(changes(&mut m, &banks[0].clone()), (0, 1, 0));
    let t = frames(&mut m, &mut banks, &ctx, &mut acct, &olga_named(&s), 2);
    assert!(t.contains("1 unsent change"), "{t}");
    assert!(m.edit.open_send(LOCAL, owner()));
    host_a(&mut m);
    let page = frames(&mut m, &mut banks, &ctx, &mut acct, &olga_named(&s), 3);
    assert!(page.contains("0 added, 1 changed, 0 removed"), "{page}");
}

/// The three-way merge of a difficulty: who changed it decides, a different change on both sides is a decision, and the
/// review shows it in the words the question editor uses.
#[test]
fn c83_i1_difficulty_in_the_three_way_merge() {
    let (bank, link) = linked(&[(QA, "One")]);
    let theirs_as = |d: Option<Difficulty>| {
        let mut t = circle_theirs(&payload(&[(QA, "One")]), NOW);
        t[0].q.difficulty = d;
        t
    };
    let ours_as = |d: Option<Difficulty>| {
        let mut b = bank.clone();
        b.questions[0].difficulty = d;
        b
    };
    let kind = |o: Option<Difficulty>, t: Option<Difficulty>| build_plan(BID, &link.base, &ours_as(o), &theirs_as(t), 1, 2, None, String::new()).rows[0].kind;
    assert_eq!(kind(None, None), RowKind::Unchanged);
    assert_eq!(kind(Some(Difficulty::High), None), RowKind::OursChanged);
    assert_eq!(kind(None, Some(Difficulty::Medium)), RowKind::TheirsChanged);
    assert_eq!(kind(Some(Difficulty::Low), Some(Difficulty::Low)), RowKind::Unchanged, "the same change on both sides");
    let plan = build_plan(BID, &link.base, &ours_as(Some(Difficulty::High)), &theirs_as(Some(Difficulty::Low)), 1, 2, None, String::new());
    assert_eq!(plan.rows[0].kind, RowKind::Conflict);
    let d = plan.rows[0].diffs.iter().find(|d| d.field == "Difficulty").expect("shown");
    assert_eq!((d.base.as_str(), d.ours.as_str(), d.theirs.as_str()), ("none", "Hard", "Easy"));
    // the same text and key with another difficulty is not "the same change"
    let mut ours = ours_as(Some(Difficulty::High));
    ours.questions[0].stem = "One (b)".into();
    let mut theirs = theirs_as(Some(Difficulty::Medium));
    theirs[0].q.stem = "One (b)".into();
    assert_eq!(build_plan(BID, &link.base, &ours, &theirs, 1, 2, None, String::new()).rows[0].kind, RowKind::Conflict);
    // and a question only someone else changed keeps that difficulty when another field changes too
    let mut theirs = theirs_as(Some(Difficulty::Medium));
    theirs[0].q.stem = "One (c)".into();
    let plan = build_plan(BID, &link.base, &ours_as(None), &theirs, 1, 2, None, String::new());
    assert_eq!(plan.rows[0].kind, RowKind::TheirsChanged);
    assert!(plan.rows[0].diffs.iter().any(|d| d.field == "Difficulty" && d.theirs == "Medium"));
}

/// A copy made without the answer keys: the teacher typed the key and changed the difficulty; the circle changed only the
/// wording. Taking the circle's wording with her key would lose her difficulty: it is a decision, not taken for her.
#[test]
fn c83_i1_a_typed_key_and_a_difficulty_are_not_carried_over_the_circles_wording() {
    let (mut bank, link) = linked_from(vec![qstripped(QA, "One")], true);
    if let crate::model::Kind::Mc { answers } = &mut bank.questions[0].kind {
        answers[0].correct = true;
    }
    let theirs = super::circle_links::circle_theirs_as(&body_of(vec![qstripped(QA, "One (b)")]), NOW, true);
    assert_eq!(build_plan(BID, &link.base, &bank, &theirs, 1, 2, None, String::new()).rows[0].kind, RowKind::TheirsChanged, "the key alone is carried");
    bank.questions[0].difficulty = Some(Difficulty::High);
    let plan = build_plan(BID, &link.base, &bank, &theirs, 1, 2, None, String::new());
    assert_eq!(plan.rows[0].kind, RowKind::Conflict);
}

/// A link an earlier build wrote kept no difficulty: no question counts as changed for it, (critic 85 I1) nor is the count
/// said to be complete, written again it is still not known, and a difference with the circle is a decision for the person
/// (never taken silently either way).
#[test]
fn c83_i1_a_base_written_before_difficulty_was_kept() {
    let (mut bank, link) = linked(&[(QA, "One")]);
    let mut v = serde_json::to_value(&link).unwrap();
    assert!(v["base"][0]["snap"].get("difficulty").is_some_and(Value::is_null), "a known \"none\" is written: {}", v["base"][0]["snap"]);
    v["base"][0]["snap"].as_object_mut().unwrap().remove("difficulty");
    let old: CircleLink = serde_json::from_value(v).unwrap();
    bank.questions[0].difficulty = Some(Difficulty::Medium);
    // (critic 85 I1) not counted as changed, and never said to be nothing: whether it changed can not be told
    let c = changes_since(&old, &bank);
    assert_eq!((c.changed, c.known), (0, false), "no phantom change, and no false \"up to date\"");
    let again: CircleLink = serde_json::from_value(serde_json::to_value(&old).unwrap()).unwrap();
    assert!(serde_json::to_value(&again).unwrap()["base"][0]["snap"].get("difficulty").is_none(), "still not known");
    // one this build can not read is not known either, and the link is kept
    let mut v = serde_json::to_value(&link).unwrap();
    v["base"][0]["snap"]["difficulty"] = json!("Extreme");
    let odd: CircleLink = serde_json::from_value(v).expect("the link is read");
    assert_eq!((odd.base.len(), odd.base[0].snap.difficulty), (1, None));
    let mut v = serde_json::to_value(&link).unwrap();
    v["base"][0]["snap"]["difficulty"] = json!("High");
    assert_eq!(serde_json::from_value::<CircleLink>(v).unwrap().base[0].snap.difficulty, Some(Some(Difficulty::High)));
    let c = changes_since(&again, &bank);
    assert_eq!((c.changed, c.known), (0, false));
    let mut theirs = circle_theirs(&payload(&[(QA, "One")]), NOW);
    theirs[0].q.difficulty = Some(Difficulty::High);
    assert_eq!(build_plan(BID, &old.base, &bank, &theirs, 1, 2, None, String::new()).rows[0].kind, RowKind::Conflict);
    theirs[0].q.difficulty = Some(Difficulty::Medium);
    assert_eq!(build_plan(BID, &old.base, &bank, &theirs, 1, 2, None, String::new()).rows[0].kind, RowKind::Unchanged);
    // a copy with no record of its earlier version (two-way) treats a difficulty the two sides differ on as a decision too
    theirs[0].q.difficulty = None;
    let fallback = super::merge::unknown_base(&bank, &[(bank.questions[0].id.clone(), QA.to_string())], &theirs);
    assert_eq!(build_plan(BID, &fallback, &bank, &theirs, 1, 2, None, String::new()).rows[0].kind, RowKind::Conflict);
}

// ---- c83 I2: the person's own version, then a colleague's ---------------------------------------------------------------

/// merge-initiate from 3 to 5: olga's own version 4, then Bob Teacher's 5.
pub(super) fn own_then_bob() -> Value {
    let mut v = merge_start_by(3, 5, "Bob Teacher");
    let mut four = v["between"][0].clone();
    four["version"] = json!(4);
    four["version_id"] = json!(hex(4));
    four["by"] = json!("Olga Teacher");
    v["between"] = json!([four, v["between"][0].clone()]);
    v
}

pub(super) fn version_path(n: u64) -> String {
    format!("{}/versions/{n}", put_path())
}

pub(super) fn version_reads(s: &Arc<Script>, n: u64) -> usize {
    s.paths().iter().filter(|p| **p == format!("Get {}", version_path(n))).count()
}

/// The teacher's send got no answer; the check found her own version 4 ("sent by you"); Bob then changed the same question
/// in version 5. Get latest is Bob's change to take, never a conflict with the teacher's own send ("Keep mine" would undo
/// Bob's), and the review says version 4 was hers.
#[test]
fn c83_i2_a_colleagues_change_after_the_persons_own_send_is_theirs_to_take() {
    let fix = |b: &mut QuestionBank| b.questions[0].stem = "One (fix)".into();
    let (mut m, s, mut banks, ctx, mut acct) = sent_with(&fix, true, timed_out(), 0, Value::Null, (200, heads_by(4, "Olga Teacher")));
    let who = olga_named(&s);
    let t = frames(&mut m, &mut banks, &ctx, &mut acct, &who, 3);
    assert!(said(&m, LOCAL).contains("sent by you"), "{t}");
    s.keep_raw("Get", &version_path(4), served(4, &[(QA, "One (fix)")], false));
    let page = latest_as(&mut m, &s, &mut banks, &ctx, &mut acct, &who, own_then_bob(), served(5, &[(QA, "One (fix) by Bob")], false));
    let k = kinds(&m).unwrap();
    assert_eq!((count(&k, RowKind::TheirsChanged), count(&k, RowKind::Conflict)), (1, 0), "{page}");
    assert!(page.contains("Version 4 is your own send"), "{page}");
    apply(&mut m, &s, &mut banks, &ctx, &mut acct);
    assert_eq!(banks[0].questions[0].stem, "One (fix) by Bob");
    assert_eq!(changes(&mut m, &banks[0].clone()), (0, 0, 0));
}

/// The same for a question the send added: Bob's correction of it in version 5 is taken, the question is never added a
/// second time, and nothing is left to send.
#[test]
fn c83_i2_a_colleagues_change_to_a_question_the_persons_own_send_added() {
    let (mut m, s, mut banks, ctx, mut acct) = sent_with(&add_question, true, timed_out(), 0, Value::Null, (200, heads_by(4, "Olga Teacher")));
    let who = olga_named(&s);
    frames(&mut m, &mut banks, &ctx, &mut acct, &who, 3);
    s.keep_raw("Get", &version_path(4), served(4, &[(QA, "One"), (QN, ADDED)], false));
    let page = latest_as(&mut m, &s, &mut banks, &ctx, &mut acct, &who, own_then_bob(), served(5, &[(QA, "One"), (QN, "Solve 5x = 21")], false));
    let k = kinds(&m).unwrap();
    assert_eq!((count(&k, RowKind::TheirsChanged), count(&k, RowKind::NewInTheirs), count(&k, RowKind::Conflict)), (1, 0, 0), "{page}");
    assert!(page.contains("Version 4 is your own send") && !page.contains("yours alone"), "{page}");
    apply(&mut m, &s, &mut banks, &ctx, &mut acct);
    assert_eq!(banks[0].questions.iter().map(|q| q.stem.as_str()).collect::<Vec<_>>(), vec!["One", "Solve 5x = 21"]);
    assert_eq!(changes(&mut m, &banks[0].clone()), (0, 0, 0));
}

/// Version 4 is read only when it is needed: not when it is the newest (that read is the review's own), and not when it is a
/// colleague's. One that can no longer be read leaves the review as before (matched to the newest version only).
#[test]
fn c83_i2_the_persons_own_version_is_read_only_when_needed() {
    // the newest
    let (mut m, s, mut banks, ctx, mut acct) = sent_with(&add_question, true, timed_out(), 0, Value::Null, (200, heads_by(4, "Olga Teacher")));
    let who = olga_named(&s);
    frames(&mut m, &mut banks, &ctx, &mut acct, &who, 3);
    latest_as(&mut m, &s, &mut banks, &ctx, &mut acct, &who, merge_start_by(3, 4, "Olga Teacher"), served(4, &[(QA, "One"), (QN, ADDED)], false));
    assert_eq!(version_reads(&s, 4), 0);
    // a colleague's version 4, then the teacher's 5
    let (mut m, s, mut banks, ctx, mut acct) = sent_with(&add_question, true, timed_out(), 0, Value::Null, (503, json!({"error": "unavailable"})));
    let who = olga_named(&s);
    frames(&mut m, &mut banks, &ctx, &mut acct, &who, 2);
    latest_as(&mut m, &s, &mut banks, &ctx, &mut acct, &who, two_since(), served(5, &[(QA, "One"), (QN, ADDED)], false));
    assert_eq!(version_reads(&s, 4), 0);
    // no longer kept: the review still opens
    let (mut m, s, mut banks, ctx, mut acct) = sent_with(&add_question, true, timed_out(), 0, Value::Null, (200, heads_by(4, "Olga Teacher")));
    let who = olga_named(&s);
    frames(&mut m, &mut banks, &ctx, &mut acct, &who, 3);
    s.keep("Get", &version_path(4), 404, json!({"error": "version_not_retained", "oldest_kept": 5}));
    let page = latest_as(&mut m, &s, &mut banks, &ctx, &mut acct, &who, own_then_bob(), served(5, &[(QA, "One (b)"), (QN, ADDED)], false));
    assert_eq!(version_reads(&s, 4), 1);
    let k = kinds(&m).unwrap();
    assert_eq!((count(&k, RowKind::TheirsChanged), count(&k, RowKind::NewInTheirs)), (1, 0), "{page}");
    // after a check named a colleague's version right after the send's (it did not go through), not even while the
    // person's name is not known
    let (mut m, s, mut banks, ctx, mut acct) = sent_with(&add_question, true, timed_out(), 0, Value::Null, (200, heads_by(4, "Bob Teacher")));
    frames(&mut m, &mut banks, &ctx, &mut acct, &olga_named(&s), 3);
    assert!(m.edit.with_link(LOCAL, |l| l.sent_record.as_ref().is_some_and(|r| r.not_through)).unwrap());
    frames(&mut m, &mut banks, &ctx, &mut acct, &olga(&s), 1);
    latest_as(&mut m, &s, &mut banks, &ctx, &mut acct, &olga(&s), two_since(), served(5, &[(QA, "One"), (QN, ADDED)], false));
    assert_eq!(version_reads(&s, 4), 0);
    // one the circle lists as no longer kept, or held back, is not asked for
    for (kept, withheld) in [(false, false), (true, true)] {
        let (mut m, s, mut banks, ctx, mut acct) = sent_with(&add_question, true, timed_out(), 0, Value::Null, (200, heads_by(4, "Olga Teacher")));
        let who = olga_named(&s);
        frames(&mut m, &mut banks, &ctx, &mut acct, &who, 3);
        let mut start = own_then_bob();
        start["between"][0]["kept"] = json!(kept);
        start["between"][0]["withheld"] = json!(withheld);
        latest_as(&mut m, &s, &mut banks, &ctx, &mut acct, &who, start, served(5, &[(QA, "One (b)"), (QN, ADDED)], false));
        assert_eq!(version_reads(&s, 4), 0, "kept {kept}, withheld {withheld}");
    }
}

// ---- c84 I2: "You sent this in version 4" only of what that send changed ---------------------------------------------------

/// The send added Solve 5x = 20 and carried One unchanged; One is corrected afterwards. One is "changed by you", never
/// "You sent this in version 4": whether the check found the teacher's version 4, or still version 3 and a later Send met
/// a conflict with her own version 4.
#[test]
fn c84_i2_a_question_the_send_carried_unchanged_is_not_said_to_have_been_sent() {
    for path in ["check found it", "conflict after still"] {
        let check = if path == "check found it" { heads_by(4, "Olga Teacher") } else { heads(3) };
        let (mut m, s, mut banks, ctx, mut acct) = sent_with(&add_question, true, timed_out(), 0, Value::Null, (200, check));
        let who = olga_named(&s);
        frames(&mut m, &mut banks, &ctx, &mut acct, &who, 3);
        if path == "check found it" {
            banks[0].questions[0].stem = "One (13)".into();
            banks[0].modified += 5;
        } else {
            conflict_by(&mut m, &s, &mut banks, &ctx, &mut acct, &who, 4, "Olga Teacher");
            m.edit.close();
        }
        let page = latest_as(&mut m, &s, &mut banks, &ctx, &mut acct, &who, merge_start_by(3, 4, "Olga Teacher"), served(4, &[(QA, "One"), (QN, ADDED)], false));
        let k = kinds(&m).unwrap();
        assert_eq!(count(&k, RowKind::OursChanged), 1, "{path}: {page}");
        assert!(page.contains("Version 4 is your own send"), "{path}: {page}");
        assert!(!page.contains("You sent this"), "{path}: {page}");
        assert!(notes(&m).iter().any(|n| n.contains("Only you changed this")), "{path}: {:?}", notes(&m));
    }
}

/// A question the send changed, and changed again since, keeps the note (critic 82's case); one the send added and that
/// was corrected since keeps it too.
#[test]
fn c84_i2_a_question_the_send_changed_and_changed_again_keeps_the_note() {
    let fix = |b: &mut QuestionBank| b.questions[0].stem = "One (a)".into();
    let (mut m, s, mut banks, ctx, mut acct) = sent_with(&fix, true, timed_out(), 0, Value::Null, (200, heads_by(4, "Olga Teacher")));
    let who = olga_named(&s);
    frames(&mut m, &mut banks, &ctx, &mut acct, &who, 3);
    banks[0].questions[0].stem = "One (b)".into();
    banks[0].modified += 5;
    let page = latest_as(&mut m, &s, &mut banks, &ctx, &mut acct, &who, merge_start_by(3, 4, "Olga Teacher"), served(4, &[(QA, "One (a)")], false));
    assert_eq!(count(&kinds(&m).unwrap(), RowKind::OursChanged), 1, "{page}");
    assert!(page.contains("You sent this in version 4 and changed it since"), "{page}");
    // added, then corrected
    let (mut m, s, mut banks, ctx, mut acct) = sent_with(&add_question, true, timed_out(), 0, Value::Null, (200, heads_by(4, "Olga Teacher")));
    let who = olga_named(&s);
    frames(&mut m, &mut banks, &ctx, &mut acct, &who, 3);
    banks[0].questions[1].stem = FIXED.into();
    banks[0].modified += 5;
    let page = latest_as(&mut m, &s, &mut banks, &ctx, &mut acct, &who, merge_start_by(3, 4, "Olga Teacher"), served(4, &[(QA, "One"), (QN, ADDED)], false));
    let k = kinds(&m).unwrap();
    assert_eq!((count(&k, RowKind::OursChanged), count(&k, RowKind::NewInTheirs)), (1, 0), "{page}");
    assert!(page.contains("You sent this in version 4 and changed it since"), "{page}");
    apply(&mut m, &s, &mut banks, &ctx, &mut acct);
    assert_eq!(banks[0].questions.len(), 2);
    assert_eq!(changes(&mut m, &banks[0].clone()), (0, 1, 0), "the correction is still to send");
}

/// A question the send changed and that the circle has as sent, unchanged since: nothing is said of it (it is not a
/// question "you and the circle added").
#[test]
fn c84_i2_a_question_the_send_changed_is_not_said_to_have_been_added() {
    let fix = |b: &mut QuestionBank| b.questions[0].stem = "One (a)".into();
    let (mut m, s, mut banks, ctx, mut acct) = sent_with(&fix, true, timed_out(), 0, Value::Null, (200, heads_by(4, "Olga Teacher")));
    let who = olga_named(&s);
    frames(&mut m, &mut banks, &ctx, &mut acct, &who, 3);
    let page = latest_as(&mut m, &s, &mut banks, &ctx, &mut acct, &who, merge_start_by(3, 4, "Olga Teacher"), served(4, &[(QA, "One (a)")], false));
    assert_eq!(count(&kinds(&m).unwrap(), RowKind::Unchanged), 1, "{page}");
    assert!(page.contains("Version 4 is your own send") && !page.contains("added the same question"), "{page}");
}

/// What a send with a lost answer carried, matched to the version it made (`rebase_own_send`): an added question is linked
/// to a question new in that version (never to one the link already knows, even the same), each once, and never for a
/// bank question the link already knows; with no record of the version before (`known` false), no question is said to
/// have been changed by the send (and, critic 85 m3, a question carried is no evidence that the version is the send's).
#[test]
fn c83_i2_what_a_send_carried_is_matched_to_the_version_it_made() {
    use super::merge::rebase_own_send;
    let pic = |q: &crate::model::Question, t: &super::merge::Theirs| super::circle_edit::same_picture(q, t);
    let (bank, link) = linked(&[(QA, "One")]);
    let one = bank.questions[0].clone();
    let copy = |id: &str, stem: &str| {
        let mut q = one.clone();
        q.id = id.into();
        q.stem = stem.into();
        q
    };
    // an exact copy of One, added: linked to the new one, not to One
    let c = copy("copy", "One");
    let mine = circle_theirs(&payload(&[(QA, "One"), (QN, "One")]), NOW);
    let mut base = link.base.clone();
    let r = rebase_own_send(&mut base, &[QA], &[(QA, &one), ("", &c)], &mine, &pic, true);
    assert_eq!(r.added, vec!["copy".to_string()]);
    assert_eq!(base.iter().filter(|b| b.qid == QA).count(), 1);
    assert_eq!(base.iter().find(|b| b.local_id == "copy").map(|b| b.qid.as_str()), Some(QN));
    // two the same, added: one each
    let (c1, c2) = (copy("c1", "Two"), copy("c2", "Two"));
    let mine = circle_theirs(&payload(&[(QA, "One"), (QB, "Two"), (QN, "Two")]), NOW);
    let mut base = link.base.clone();
    rebase_own_send(&mut base, &[QA], &[("", &c1), ("", &c2)], &mine, &pic, true);
    let mut got: Vec<&str> = base.iter().filter(|b| b.local_id.starts_with('c')).map(|b| b.qid.as_str()).collect();
    got.sort_unstable();
    assert_eq!(got, vec![QB, QN]);
    // a bank question the link already knows is not linked again
    let known_one = copy(&one.id, "Two");
    let mut base = link.base.clone();
    let r = rebase_own_send(&mut base, &[QA], &[("", &known_one)], &mine, &pic, true);
    assert!(r.added.is_empty() && base.len() == 1, "{r:?}");
    // changed by the send: said so only when the base says what it was before
    let sent = copy(&one.id, "One (a)");
    let mine = circle_theirs(&payload(&[(QA, "One (a)")]), NOW);
    for known in [true, false] {
        let mut base = link.base.clone();
        let r = rebase_own_send(&mut base, &[QA], &[(QA, &sent)], &mine, &pic, known);
        assert_eq!(r.resent.is_empty(), !known, "known {known}");
        // (critic 85 m3) nor is the version said to be the send's on that alone; it is still what the question is matched to
        assert!(r.own == known && base[0].snap.stem == "One (a)", "known {known}");
    }
}

/// (x09) A question the send added whose picture was changed since: changed by the teacher (never "unchanged"), and still
/// to send after the merge.
#[test]
fn c83_a_question_the_send_added_whose_picture_changed_since_is_still_to_send() {
    let (mut m, s, mut banks, ctx, mut acct) = rig();
    let mut added: crate::model::Question = serde_json::from_value(qjson("new-local", ADDED)).unwrap();
    added.image = Some(crate::model::ImageRef { file: "media/aaaa.png".into(), width: 2, height: 2 });
    banks[0].questions.push(added);
    banks[0].modified += 5;
    let sent = banks[0].clone();
    let record = m.edit.with_link(LOCAL, |l| super::circle_links::sent_record(l, &sent, 3, String::new())).unwrap();
    m.edit.update_link(LOCAL, |l| {
        l.sent_record = Some(record);
        true
    });
    banks[0].questions[1].image = Some(crate::model::ImageRef { file: "media/bbbb.png".into(), width: 2, height: 2 });
    banks[0].modified += 5;
    let mut pictured = qjson(QN, ADDED);
    pictured["image"] = json!({"file": "media/aaaa.png", "width": 2, "height": 2});
    let page = get_latest(&mut m, &s, &mut banks, &ctx, &mut acct, served_qs(4, vec![qjson(QA, "One"), pictured], false));
    let k = kinds(&m).unwrap();
    assert_eq!((count(&k, RowKind::OursChanged), count(&k, RowKind::NewInTheirs), count(&k, RowKind::Unchanged)), (1, 0, 1), "{page}");
    assert!(page.contains("Version 4 is your own send"), "{page}");
}

// ---- c84 I1: a note that a send or restore may not have arrived yet is rewritten by the check that settles it -----------

/// The check right after the send still found version 3 ("may not have arrived yet"); a later check finds the teacher's
/// version 4, or Bob's. The note then says what that check found, never "still has version 3 ... press Send my changes".
#[test]
fn c84_i1_a_later_check_settles_a_send_that_had_not_arrived() {
    for (by, says) in [("Olga Teacher", "sent by you"), ("Bob Teacher", "Bob Teacher sent version 4 first")] {
        let (mut m, s, mut banks, ctx, mut acct) = sent_with(&add_question, true, timed_out(), 0, Value::Null, (200, heads(3)));
        let who = olga_named(&s);
        frames(&mut m, &mut banks, &ctx, &mut acct, &who, 3);
        assert!(said(&m, LOCAL).contains("may not have arrived yet"), "{by}: {}", said(&m, LOCAL));
        // a second check that still finds version 3 says the same
        m.links.check_now(LOCAL);
        frames(&mut m, &mut banks, &ctx, &mut acct, &who, 3);
        assert!(said(&m, LOCAL).contains("may not have arrived yet"), "{by}: {}", said(&m, LOCAL));
        s.keep("Post", "/circles/heads", 200, heads_by(4, by));
        m.links.check_now(LOCAL);
        let t = frames(&mut m, &mut banks, &ctx, &mut acct, &who, 3);
        let r = said(&m, LOCAL);
        assert!(r.contains(says), "{by}: {r}");
        assert!(!r.contains("still has version") && !r.contains("press Send my changes") && !r.contains("may not have arrived"), "{by}: {r}");
        assert!(!t.contains("may not have arrived"), "{by}: {t}");
        if by == "Bob Teacher" {
            assert!(r.contains("did not go through"), "{r}");
        }
    }
}

/// The same for a restore: "may not have arrived yet" is replaced once a later check finds a newer version.
#[test]
fn c84_i1_a_later_check_settles_a_restore_that_had_not_arrived() {
    let (mut m, s, mut banks, ctx, mut acct) = restore_answered(timed_out(), 0, Value::Null, true);
    s.keep("Post", "/circles/heads", 200, heads(3));
    frames(&mut m, &mut banks, &ctx, &mut acct, &olga(&s), 3);
    assert!(said(&m, LOCAL).contains("may not have arrived yet"), "{}", said(&m, LOCAL));
    s.keep("Post", "/circles/heads", 200, heads(4));
    m.links.check_now(LOCAL);
    let t = frames(&mut m, &mut banks, &ctx, &mut acct, &olga(&s), 3);
    let r = said(&m, LOCAL);
    assert!(r.contains("version 4") && r.contains("may be the restore of version 1"), "{r}");
    assert!(!r.contains("still has version") && !r.contains("may not have arrived"), "{r}");
    assert!(!t.contains("may not have arrived"), "{t}");
}

/// A send pressed again after "may not have arrived yet" and refused as a conflict: what the strip says is that answer, and
/// a later check does not write the old note's news over it.
#[test]
fn c84_i1_a_later_send_answer_is_not_written_over_by_the_check() {
    let (mut m, s, mut banks, ctx, mut acct) = sent_with(&add_question, true, timed_out(), 0, Value::Null, (200, heads(3)));
    let who = olga_named(&s);
    frames(&mut m, &mut banks, &ctx, &mut acct, &who, 3);
    conflict_by(&mut m, &s, &mut banks, &ctx, &mut acct, &who, 4, "Bob Teacher");
    m.edit.close();
    let before = said(&m, LOCAL);
    assert!(before.contains("someone sent version 4 first"), "{before}");
    s.keep("Post", "/circles/heads", 200, heads_by(4, "Bob Teacher"));
    m.links.check_now(LOCAL);
    frames(&mut m, &mut banks, &ctx, &mut acct, &who, 3);
    assert_eq!(said(&m, LOCAL), before);
}

// ---- c83 minors -----------------------------------------------------------------------------------------------------

/// What verSHion's server stores of these questions (share-core's `publish_prepare`, as for a circle send).
fn stored(qs: &[Value]) -> Option<Vec<Value>> {
    let body = json!({"schema_version": 2, "bank": {"id": BID, "name": "Cells", "questions": qs, "created": 1, "modified": 2}}).to_string();
    let out: Value = serde_json::from_str(&share_core::wasm::publish_prepare_json(body.as_bytes(), "{}")).unwrap();
    (out["ok"] == true).then(|| out["payload"]["bank"]["questions"].as_array().unwrap().clone())
}

fn topics_of(list: &[&str]) -> Vec<String> {
    canon::topics(&list.iter().map(|s| s.to_string()).collect::<Vec<_>>())
}

/// A topic as the server keeps it: share-core's Line cleaning, then the form students are served.
fn server_topic(s: &str) -> Option<Vec<String>> {
    use share_core::text::{Class, Max, check, clean, has_visible};
    check(s, Class::Line).ok()?;
    let t = share_core::format::served_text(&clean(s, Class::Line, Max::Chars(256)).ok()?);
    Some(if has_visible(&t) { vec![t] } else { Vec::new() })
}

/// (c83 minor) A topic edit that only adds or removes a joiner the server keeps (Persian, Hindi, an emoji sequence) is a
/// change; one the server drops is not.
#[test]
fn c83_a_joiner_the_server_keeps_in_a_topic_is_compared() {
    // Persian "mi-ravam" with its ZWNJ, Hindi half-form with ZWJ, an emoji family
    assert_ne!(topics_of(&["\u{645}\u{6CC}\u{200C}\u{631}\u{648}\u{645}"]), topics_of(&["\u{645}\u{6CC}\u{631}\u{648}\u{645}"]));
    assert_ne!(topics_of(&["\u{915}\u{94D}\u{200D}\u{937}"]), topics_of(&["\u{915}\u{94D}\u{937}"]));
    assert_ne!(topics_of(&["\u{1F468}\u{200D}\u{1F469}"]), topics_of(&["\u{1F468}\u{1F469}"]));
    // between Latin letters, after an alef (which never joins on its left) or at the ends: dropped by the server
    assert_eq!(topics_of(&["Alg\u{200D}ebra"]), topics_of(&["Algebra"]));
    assert_eq!(topics_of(&["\u{627}\u{200C}\u{628}"]), topics_of(&["\u{627}\u{628}"]));
    assert_eq!(topics_of(&["\u{627}\u{653}\u{200C}\u{628}"]), topics_of(&["\u{622}\u{628}"]), "alef with madda, decomposed");
    assert_eq!(topics_of(&["\u{628}\u{200C} x"]), topics_of(&["\u{628} x"]));
    assert_eq!(topics_of(&["\u{200D}\u{628}"]), topics_of(&["\u{628}"]));
    // a joiner right before a combining grapheme joiner goes with it
    assert_eq!(topics_of(&["\u{628}\u{200C}\u{34F}\u{628}"]), topics_of(&["\u{628}\u{628}"]));
    // a joiner the server keeps, as stored: the same topic. (The last two: hamza above after an acute accent of the same
    // class does not compose with the alef, so the joiner follows a mark of a joining script and stays; after an alef with
    // hamza that did compose, a non-joiner goes.)
    for s in [
        "\u{645}\u{6CC}\u{200C}\u{631}\u{648}\u{645}",
        "\u{915}\u{94D}\u{200D}\u{937}",
        "\u{1F468}\u{200D}\u{1F469}\u{200D}\u{1F467}",
        "\u{628}\u{64E}\u{200C}\u{628}",
        "\u{627}\u{301}\u{654}\u{200D}\u{6C1}",
        "\u{627}\u{654}\u{200C}\u{6C1}",
    ] {
        let theirs = server_topic(s).unwrap();
        assert_eq!(topics_of(&[s]), canon::topics(&theirs), "{s:?}");
        assert_eq!(topics_of(&[s]), theirs.iter().map(|t| canon::line(t)).collect::<Vec<_>>(), "{s:?}: as the server keeps it");
    }
    assert!(topics_of(&["\u{627}\u{301}\u{654}\u{200D}\u{6C1}"])[0].contains('\u{200D}'));
    assert!(!topics_of(&["\u{627}\u{654}\u{200C}\u{6C1}"])[0].contains('\u{200C}'));
}

/// (c83 minor) Topics with joiners in every context, against share-core itself: each is compared exactly as the server
/// keeps it (its joiners where the server keeps them), and the server's own copy is the same topic.
#[test]
fn c83_topics_with_joiners_compare_exactly_as_the_server_keeps_them() {
    const POOL: &[&str] = &[
        "\u{628}", "\u{627}", "\u{622}", "\u{627}\u{653}", "\u{627}\u{654}", "\u{648}", "\u{648}\u{654}", "\u{6CC}", "\u{645}", "\u{64A}", "\u{64A}\u{654}", "\u{6D5}",
        "\u{6D2}", "\u{6C1}", "\u{64E}", "\u{651}", "\u{654}", "\u{653}", "\u{200C}", "\u{200D}", "\u{200C}", "\u{200D}", "\u{34F}", "\u{915}", "\u{94D}", "\u{937}",
        "\u{93C}", "\u{958}", "\u{1F468}", "\u{1F469}", "\u{2764}", "\u{FE0F}", "\u{1F3FD}", " ", "a", "\u{301}", "\u{323}", "\u{FEFF}", "\u{200E}", "\u{9C7}", "\u{9BE}",
        "\u{DD9}", "\u{DCF}", "\u{1025}", "\u{102E}", "\u{F40}", "\u{FB7}",
    ];
    let mut r = 0x2545_F491_4F6C_DD1Du64;
    let mut next = || {
        r ^= r << 13;
        r ^= r >> 7;
        r ^= r << 17;
        r
    };
    let (mut tried, mut joined) = (0, 0);
    for i in 0..20_000 {
        let n = 1 + next() % 7;
        let s: String = (0..n).map(|_| POOL[(next() as usize) % POOL.len()]).collect();
        let Some(theirs) = server_topic(&s) else { continue };
        tried += 1;
        let ours = topics_of(&[&s]);
        assert_eq!(ours, theirs.iter().map(|t| canon::line(t)).collect::<Vec<_>>(), "#{i} {s:?}: the server keeps {theirs:?}");
        assert_eq!(ours, canon::topics(&theirs), "#{i} {s:?}");
        if theirs.iter().any(|t| t.contains(['\u{200C}', '\u{200D}'])) {
            joined += 1;
        }
    }
    assert!(tried > 10_000 && joined > 500, "enough topics, and enough with a kept joiner: {tried}, {joined}");
}

/// (c83 minor) Figure values the server drops (a value that is not a number, a text or true/false: what this app writes
/// for a number that is not finite, a list) are not compared: the question is the same as its stored copy.
#[test]
fn c83_figure_values_the_server_drops_are_not_compared() {
    use super::merge::same_question;
    let fig = |params: Value| {
        let mut q: crate::model::Question = serde_json::from_value(qjson("q", "Stem")).unwrap();
        q.figure = Some(serde_json::from_value(json!({"id": "number-line", "params": params})).unwrap());
        q
    };
    let mine = fig(json!({"min": 1, "max": null, "marks": [1, 2], "style": {"a": 1}, "label": "x", "open": true}));
    assert!(same_question(&mine, &fig(json!({"min": 1, "label": "x", "open": true}))));
    assert!(!same_question(&mine, &fig(json!({"min": 1, "label": "x"}))), "true/false is kept");
    assert!(!same_question(&mine, &fig(json!({"min": 2, "label": "x", "open": true}))));
    let mut v = qjson("q", "Stem");
    v["figure"] = json!({"id": "number-line", "params": {"min": 1, "max": null, "marks": [1, 2], "label": "x", "open": true}});
    let s = stored(&[v.clone()]).expect("stored");
    let theirs: crate::model::Question = serde_json::from_value(s[0].clone()).unwrap();
    assert!(same_question(&serde_json::from_value(v).unwrap(), &theirs), "as stored: {}", s[0]);
}

/// (c83 minor, x08) A link an earlier build wrote, whose base kept an explanation of nothing but a byte-order mark (or of
/// white space only as none): the bank's question is not changed for it.
#[test]
fn c83_an_earlier_builds_blank_explanation_is_not_a_change() {
    for (base_has, bank_has) in [(json!("\u{FEFF}"), "\u{FEFF}"), (json!(null), "   "), (json!(" \u{FEFF} "), ""), (json!(null), " \n ")] {
        let (mut bank, link) = linked(&[(QA, "One")]);
        let mut v = serde_json::to_value(&link).unwrap();
        v["base"][0]["snap"]["explanation"] = base_has.clone();
        let old: CircleLink = serde_json::from_value(v).unwrap();
        bank.questions[0].explanation = Some(bank_has.to_string());
        assert_eq!(changes_since(&old, &bank).changed, 0, "{base_has} / {bank_has:?}");
        let theirs = circle_theirs(&payload(&[(QA, "One")]), NOW);
        assert_eq!(build_plan(BID, &old.base, &bank, &theirs, 1, 2, None, String::new()).rows[0].kind, RowKind::Unchanged, "{base_has}");
    }
}

/// (x11) The explanation is part of what makes two questions the same.
#[test]
fn c83_two_questions_with_other_explanations_are_not_the_same() {
    use super::merge::same_question;
    let q = |e: Option<&str>| {
        let mut q: crate::model::Question = serde_json::from_value(qjson("q", "Stem")).unwrap();
        q.explanation = e.map(str::to_string);
        q
    };
    assert!(!same_question(&q(Some("Because A.")), &q(Some("Because B."))));
    assert!(!same_question(&q(Some("Because A.")), &q(None)));
    assert!(same_question(&q(Some("\u{FEFF}")), &q(None)));
}

/// An account whose address is not known yet but whose name is: a send with no answer is kept with no sender.
fn unknown_named(s: &Arc<Script>) -> AccountSnapshot {
    AccountSnapshot { name: Some("Olga Teacher".into()), ..ready(s) }
}

/// A send from A by an account not known yet (its name only), with no clear answer; the check asked for at once is
/// answered `check`. The Send page is then left.
fn unknown_sent(check: (u16, Value)) -> Rig {
    let (mut m, s, mut banks) = two_at_three();
    s.send_base_404(true);
    s.keep("Get", &format!("/circles/{CID}"), 200, detail("owner", json!({})));
    let ctx = ctx();
    let mut acct = FakeAccount { open: true, ..FakeAccount::default() };
    let who = unknown_named(&s);
    frames(&mut m, &mut banks, &ctx, &mut acct, &who, 2);
    add_question(&mut banks[0]);
    banks[0].modified += 5;
    assert!(m.edit.open_send(LOCAL, owner()));
    host_a(&mut m);
    frames(&mut m, &mut banks, &ctx, &mut acct, &who, 3);
    m.edit.set_attest(true);
    s.on("Post", "/circles/heads", 200, heads(3));
    s.fail(timed_out().unwrap());
    m.edit.send();
    m.edit.sync(&mut banks);
    assert!(s.paths().iter().any(|p| p.starts_with("Put ")), "{:?}", s.paths());
    // the check asked for at once (and only it) is answered `check`
    s.on("Post", "/circles/heads", check.0, check.1);
    frames(&mut m, &mut banks, &ctx, &mut acct, &who, 1);
    assert!(m.edit.with_link(LOCAL, |l| l.sent_record.as_ref().is_some_and(|r| r.by.is_empty())).unwrap(), "kept with no sender");
    m.edit.close();
    (m, s, banks, ctx, acct)
}

/// (x19) Such a send is the account's own while its turn lasts, also once its address is known: the check finding its
/// version 4 says it went through.
#[test]
fn c83_a_send_by_an_account_not_known_yet_is_its_own_in_its_turn() {
    let (mut m, s, mut banks, ctx, mut acct) = unknown_sent((200, heads_by(4, "Olga Teacher")));
    let t = frames(&mut m, &mut banks, &ctx, &mut acct, &olga_named(&s), 3);
    assert_eq!(m.edit.earlier_send(LOCAL), Some(true), "{t}");
    assert!(said(&m, LOCAL).contains("sent by you"), "{}", said(&m, LOCAL));
}

/// (c83 minor) ...but never another account's that signs in afterwards: what it carried goes on the switch.
#[test]
fn c83_a_send_by_an_account_not_known_is_not_the_next_accounts() {
    let (mut m, s, mut banks, ctx, mut acct) = unknown_sent((503, json!({"error": "unavailable"})));
    frames(&mut m, &mut banks, &ctx, &mut acct, &olga_named(&s), 2);
    let pete = AccountSnapshot { name: Some("Pete Teacher".into()), ..as_who(&s, "pete") };
    frames(&mut m, &mut banks, &ctx, &mut acct, &pete, 2);
    assert!(m.edit.with_link(LOCAL, |l| l.sent_record.is_none()).unwrap(), "dropped on the switch");
    assert_eq!(m.edit.earlier_send(LOCAL), None);
    let page = latest_as(&mut m, &s, &mut banks, &ctx, &mut acct, &pete, merge_start_by(3, 4, "Olga Teacher"), served(4, &[(QA, "One"), (QN, ADDED)], false));
    assert!(!page.contains("is your own send") && !page.contains("You sent this"), "{page}");
}
