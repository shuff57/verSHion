//! Phase D1, round 12 (critics 85 and 86): each test written first against the code it pins. A link an earlier build
//! wrote (no difficulty in its base) learns its difficulties from the bank it last wrote, or says its changes can not be
//! counted; a merge writes a base that knows every difficulty, and the same change on both sides leaves nothing to send; a
//! question whose difficulty this computer has no record of is said so, never "both changed", and a colleague's rating is
//! kept unless the teacher keeps hers; a conflict on top of the person's own older version shows all three versions; a send
//! refused because the person's earlier send landed counts only what is left; the person's own version is used only when it
//! is the version asked for, is read without failing Get latest, and is "your own send" only on real evidence.

use serde_json::{Value, json};

use super::bank_links_fix10_tests::*;
use super::bank_links_fix11_tests::{diff_of, host_a, notes, own_then_bob, rated, rig, version_path, version_reads};
use super::bank_links_fix4_tests::*;
use super::bank_links_fix8_tests::*;
use super::bank_links_fix9_tests::*;
use super::bank_links_fix_tests::*;
use super::bank_links_tests::*;
use super::circle_edit::*;
use super::circle_links::{changes_since, circle_theirs, learn_difficulty};
use super::circles::CirclesModel;
use super::circles_tests::*;
use super::merge::{Choice, NOT_RECORDED, RowKind, apply_merge, build_plan};
use crate::model::{Difficulty, QuestionBank};

/// The link of A as an earlier build wrote it: its base kept no difficulty.
fn old_base(m: &mut CirclesModel) {
    m.edit.update_link(LOCAL, |l| {
        for b in &mut l.base {
            b.snap.difficulty = None;
        }
        true
    });
}

/// (round 13) The circle's version `n` is no longer kept: an earlier build's base learns nothing from it.
pub(super) fn not_kept(s: &std::sync::Arc<Script>, n: u64) {
    s.keep("Get", &version_path(n), 404, json!({"error": "version_not_retained", "oldest_kept": n + 1}));
}

fn base_difficulties(m: &mut CirclesModel) -> Vec<Option<Option<Difficulty>>> {
    m.edit.with_link(LOCAL, |l| l.base.iter().map(|b| b.snap.difficulty).collect()).unwrap()
}

fn rows_of(m: &CirclesModel) -> Vec<super::merge::Row> {
    match &m.edit.view {
        EditView::Merge(v) => match &v.phase {
            MergePhase::Review(r) => r.plan.rows.clone(),
            _ => Vec::new(),
        },
        _ => Vec::new(),
    }
}

// ---- c85 I1: a link an earlier build wrote ------------------------------------------------------------------------------

/// (round 13, critic 87 B1: never from the bank) The circle's own version 3 says what the link is in step with: its
/// difficulties are learnt (and kept), and a later difficulty-only edit is an unsent change on the strip and the badge.
#[test]
fn c85_i1_an_earlier_builds_link_learns_its_difficulties_from_the_circles_version() {
    let (mut m, s, mut banks, ctx, mut acct) = rig();
    old_base(&mut m);
    // the Banks list's own read of it (no page drawn, nothing read yet): never complete
    assert!(!m.edit.status_of(&banks[0].clone()).unwrap().changes.known);
    m.edit.sync(&mut banks);
    assert_eq!(base_difficulties(&mut m), vec![None], "never from the bank");
    s.keep_raw("Get", &version_path(3), served_qs(3, vec![rated(QA, "One", None)], false));
    frames(&mut m, &mut banks, &ctx, &mut acct, &olga_named(&s), 2);
    assert_eq!(base_difficulties(&mut m), vec![Some(None)], "learnt from version 3");
    banks[0].questions[0].difficulty = Some(Difficulty::High);
    banks[0].modified += 5;
    assert_eq!(changes(&mut m, &banks[0].clone()), (0, 1, 0));
    let t = frames(&mut m, &mut banks, &ctx, &mut acct, &olga_named(&s), 2);
    assert!(t.contains("You have 1 unsent change") && t.contains("Send my changes"), "{t}");
    assert_eq!(badge(&mut m, &banks[0].clone()), "1 unsent change");
}

/// The circle's version 3 can't be read: nothing is learnt (never from the bank), and what is unsent is not counted, so the
/// strip never says the copy is up to date and offers Send.
#[test]
fn c85_i1_an_earlier_builds_link_edited_since_is_never_said_to_be_up_to_date() {
    let (mut m, s, mut banks, ctx, mut acct) = rig();
    old_base(&mut m);
    not_kept(&s, 3);
    banks[0].questions[0].difficulty = Some(Difficulty::Low);
    banks[0].modified += 5;
    let c = m.edit.status_of(&banks[0].clone()).unwrap().changes;
    assert!(!c.known, "{c:?}");
    let t = frames(&mut m, &mut banks, &ctx, &mut acct, &olga_named(&s), 2);
    assert!(!t.contains("up to date") && t.contains("Send my changes"), "{t}");
    assert_eq!(badge(&mut m, &banks[0].clone()), "changes not counted");
    assert_eq!(base_difficulties(&mut m), vec![None], "never filled in from a bank changed since");
}

/// (round 13) `learn_difficulty` and `changes_since` on such a link: learnt only from the circle's version, by the circle's
/// question id (and only once); a question deleted since, or one already counted as changed, does not stop the count.
#[test]
fn c85_i1_learning_needs_the_circles_version() {
    let (bank, link) = linked(&[(QA, "One"), (QB, "Two")]);
    let mut old = link.clone();
    for b in &mut old.base {
        b.snap.difficulty = None;
    }
    let mut v = circle_theirs(&payload(&[(QB, "Two"), (QA, "One")]), NOW);
    v[0].q.difficulty = Some(Difficulty::Medium);
    let mut l = old.clone();
    assert!(learn_difficulty(&mut l, &v));
    assert_eq!(l.base.iter().map(|b| b.snap.difficulty).collect::<Vec<_>>(), vec![Some(None), Some(Some(Difficulty::Medium))], "by id");
    let c = changes_since(&l, &bank);
    assert!(c.known && c.changed == 1, "Two is Medium in the circle's version: {c:?}");
    assert!(!learn_difficulty(&mut l, &v), "once");
    // a version without the question: nothing learnt for it, and the count is not complete
    let mut l = old.clone();
    assert!(learn_difficulty(&mut l, &v[..1]));
    assert_eq!(l.base.iter().map(|b| b.snap.difficulty).collect::<Vec<_>>(), vec![None, Some(Some(Difficulty::Medium))]);
    let mut edited = bank.clone();
    edited.questions[1].difficulty = Some(Difficulty::Medium);
    let c = changes_since(&old, &edited);
    assert!(!c.known && c.changed == 0, "{c:?}");
    // a question deleted since: the others are counted
    let mut fewer = bank.clone();
    fewer.questions.remove(1);
    let mut l = old.clone();
    l.base[0].snap.difficulty = Some(None);
    let c = changes_since(&l, &fewer);
    assert!(c.known && (c.changed, c.removed) == (0, 1), "{c:?}");
    // one already counted as changed is counted, whatever its difficulty
    let (mut one, link) = linked(&[(QA, "One")]);
    let mut l = link.clone();
    l.base[0].snap.difficulty = None;
    one.questions[0].stem = "One (b)".into();
    let c = changes_since(&l, &one);
    assert!(c.known && c.changed == 1, "{c:?}");
}

/// A merge writes a base that knows every difficulty: a question nobody changed, or only the teacher, takes the circle's.
#[test]
fn c85_i1_a_merge_writes_a_base_that_knows_every_difficulty() {
    let (mut bank, link) = linked(&[(QA, "One"), (QB, "Two")]);
    let mut old = link.base.clone();
    for b in &mut old {
        b.snap.difficulty = None;
    }
    bank.questions[1].stem = "Two (mine)".into();
    bank.questions[1].difficulty = Some(Difficulty::High);
    let mut theirs = circle_theirs(&payload(&[(QA, "One"), (QB, "Two")]), NOW);
    theirs[1].q.difficulty = Some(Difficulty::High);
    let plan = build_plan(BID, &old, &bank, &theirs, 1, 2, None, String::new());
    assert_eq!(plan.rows.iter().map(|r| r.kind).collect::<Vec<_>>(), vec![RowKind::Unchanged, RowKind::OursChanged]);
    let (_, nb) = apply_merge(&mut bank.clone(), &plan, &theirs, &old, "Bob", NOW).unwrap();
    assert_eq!(nb.iter().map(|b| b.snap.difficulty).collect::<Vec<_>>(), vec![Some(None), Some(Some(Difficulty::High))]);
    assert_eq!(nb[1].snap.stem, "Two", "the circle's, so the teacher's change is still to send");
}

/// End to end: after Get latest on such a link (edited since), a difficulty-only edit made afterwards is counted.
#[test]
fn c85_i1_after_a_merge_a_difficulty_only_edit_is_counted() {
    let (mut m, s, mut banks, ctx, mut acct) = rig();
    old_base(&mut m);
    not_kept(&s, 3);
    add_question(&mut banks[0]);
    banks[0].modified += 5;
    get_latest(&mut m, &s, &mut banks, &ctx, &mut acct, served_qs(4, vec![qjson(QA, "One"), qjson(QB, "Two")], false));
    apply(&mut m, &s, &mut banks, &ctx, &mut acct);
    assert_eq!(changes(&mut m, &banks[0].clone()), (1, 0, 0));
    let one = banks[0].questions.iter().position(|q| q.stem == "One").unwrap();
    banks[0].questions[one].difficulty = Some(Difficulty::Medium);
    banks[0].modified += 5;
    assert_eq!(changes(&mut m, &banks[0].clone()), (1, 1, 0));
}

// ---- c85 I2: the same change on both sides --------------------------------------------------------------------------------

/// Both sides made the same change: after Apply nothing is left to send (the strip and the badge agree with Send).
#[test]
fn c85_i2_the_same_change_on_both_sides_leaves_nothing_to_send() {
    let (mut m, s, mut banks, ctx, mut acct) = rig();
    banks[0].questions[0].stem = "One (x)".into();
    banks[0].modified += 5;
    frames(&mut m, &mut banks, &ctx, &mut acct, &olga_named(&s), 2);
    let page = get_latest(&mut m, &s, &mut banks, &ctx, &mut acct, served_qs(4, vec![qjson(QA, "One (x)")], false));
    assert_eq!(kinds(&m).unwrap(), vec![RowKind::Unchanged], "{page}");
    apply(&mut m, &s, &mut banks, &ctx, &mut acct);
    assert_eq!(changes(&mut m, &banks[0].clone()), (0, 0, 0));
    let t = frames(&mut m, &mut banks, &ctx, &mut acct, &olga_named(&s), 2);
    assert!(!t.contains("unsent") && t.contains("up to date"), "{t}");
    assert_eq!(badge(&mut m, &banks[0].clone()), "");
}

// ---- c86 I1: a difficulty this computer has no record of ------------------------------------------------------------------

/// The teacher changed nothing; a colleague rated question One "Hard": it is the colleague's change to take (the base learnt
/// from the circle's version 3), never "both changed" or "the earlier version is not kept".
#[test]
fn c86_i1_a_colleagues_rating_on_an_earlier_builds_link_is_theirs_to_take() {
    let (mut m, s, mut banks, ctx, mut acct) = rig();
    old_base(&mut m);
    s.keep_raw("Get", &version_path(3), served_qs(3, vec![rated(QA, "One", None)], false));
    let before = frames(&mut m, &mut banks, &ctx, &mut acct, &olga_named(&s), 2);
    assert!(!before.contains("unsent") && !before.contains("Press Send"), "{before}");
    let page = get_latest(&mut m, &s, &mut banks, &ctx, &mut acct, served_qs(4, vec![rated(QA, "One", Some("High"))], false));
    assert_eq!(kinds(&m).unwrap(), vec![RowKind::TheirsChanged], "{page}");
    assert!(!page.contains("both changed") && !page.contains("is not kept"), "{page}");
    assert_eq!(diff_of(&m, "Difficulty"), Some(("none".into(), "none".into(), "Hard".into())));
    apply(&mut m, &s, &mut banks, &ctx, &mut acct);
    assert_eq!(banks[0].questions[0].difficulty, Some(Difficulty::High));
    assert_eq!(changes(&mut m, &banks[0].clone()), (0, 0, 0));
}

/// Version 3 can't be read, so this computer has no record of One's difficulty: the review says so in its own words, with
/// "not recorded on this computer" for version 3; (round 13, critic 87 I1) no choice is made for the teacher, and the
/// colleague's rating is taken only when she takes it.
#[test]
fn c86_i1_a_difficulty_with_no_record_is_said_so_and_the_colleagues_rating_is_hers_to_take() {
    let (mut m, s, mut banks, ctx, mut acct) = rig();
    old_base(&mut m);
    not_kept(&s, 3);
    add_question(&mut banks[0]);
    banks[0].modified += 5;
    let page = get_latest(&mut m, &s, &mut banks, &ctx, &mut acct, served_qs(4, vec![rated(QA, "One", Some("High"))], false));
    assert!(!page.contains("both changed") && !page.contains("CONFLICT") && !page.contains("is not kept"), "{page}");
    assert!(page.contains("Difficulty not recorded") && page.contains("1 difficulty not recorded"), "{page}");
    assert!(page.contains("no record of this question's difficulty in version 3"), "{page}");
    assert!(page.contains("In version 3: not recorded on this computer"), "{page}");
    assert!(page.contains("Yours now: none") && page.contains("In version 4 (the circle's newest): Hard"), "{page}");
    assert!(!m.edit.can_apply_merge(), "{page}");
    let row = rows_of(&m).iter().position(|r| r.unrecorded).unwrap();
    m.edit.set_merge_choice(row, Choice::Theirs);
    apply(&mut m, &s, &mut banks, &ctx, &mut acct);
    assert_eq!(banks[0].questions[0].difficulty, Some(Difficulty::High));
    assert_eq!(changes(&mut m, &banks[0].clone()), (1, 0, 0), "only the added question is left to send");
    // Keep mine: the teacher's is kept, and it is what goes with the next send
    let (mut m, s, mut banks, ctx, mut acct) = rig();
    old_base(&mut m);
    not_kept(&s, 3);
    add_question(&mut banks[0]);
    banks[0].modified += 5;
    get_latest(&mut m, &s, &mut banks, &ctx, &mut acct, served_qs(4, vec![rated(QA, "One", Some("High"))], false));
    let row = rows_of(&m).iter().position(|r| r.kind != RowKind::Unchanged).unwrap();
    m.edit.set_merge_choice(row, Choice::Ours);
    apply(&mut m, &s, &mut banks, &ctx, &mut acct);
    assert_eq!(banks[0].questions[0].difficulty, None);
    assert_eq!(changes(&mut m, &banks[0].clone()), (1, 1, 0));
}

/// The same with the teacher's own change to the question: taking the circle's would undo it, so it is a decision (Apply
/// waits), still in its own words.
#[test]
fn c86_i1_with_the_teachers_own_change_too_it_is_her_decision() {
    let (mut m, s, mut banks, ctx, mut acct) = rig();
    old_base(&mut m);
    not_kept(&s, 3);
    banks[0].questions[0].stem = "One (mine)".into();
    banks[0].modified += 5;
    let page = get_latest(&mut m, &s, &mut banks, &ctx, &mut acct, served_qs(4, vec![rated(QA, "One", Some("High"))], false));
    assert!(!m.edit.can_apply_merge(), "{page}");
    assert!(page.contains("no record of this question's difficulty") && page.contains("Choose one to continue"), "{page}");
    assert!(!page.contains("both changed") && !page.contains("CONFLICT"), "{page}");
    assert!(notes(&m).iter().any(|n| n.contains("Choose which to keep")), "{:?}", notes(&m));
}

/// `build_plan` with a base that kept no difficulty, the two sides differing on it: the row says why, and (round 13, critic 87
/// I1) no choice is made for the person, so neither side's rating is undone by a default; a real conflict stays one, its base
/// shown as not recorded.
#[test]
fn c86_i1_the_plan_for_a_difficulty_with_no_record() {
    let (bank, link) = linked(&[(QA, "One")]);
    let mut old = link.base.clone();
    old[0].snap.difficulty = None;
    let plan_of = |ours_stem: &str, theirs_stem: &str| {
        let mut b = bank.clone();
        b.questions[0].stem = ours_stem.into();
        let mut t = circle_theirs(&payload(&[(QA, theirs_stem)]), NOW);
        t[0].q.difficulty = Some(Difficulty::High);
        build_plan(BID, &old, &b, &t, 3, 4, None, String::new())
    };
    for (ours, theirs, choice) in [("One", "One", Choice::Undecided), ("One (a)", "One", Choice::Undecided)] {
        let p = plan_of(ours, theirs);
        let r = &p.rows[0];
        assert!(r.unrecorded && r.kind == RowKind::Conflict && r.choice == choice, "{ours}/{theirs}: {r:?}");
        assert!(r.note.as_deref().is_some_and(|n| n.contains("no record of this question's difficulty in version 3")), "{r:?}");
        let d = r.diffs.iter().find(|d| d.field == "Difficulty").unwrap();
        assert_eq!((d.base.as_str(), d.ours.as_str(), d.theirs.as_str()), (NOT_RECORDED, "none", "Hard"));
    }
    // (round 14, critic 90 I2) only the circle changed the question otherwise: its change, said so, nothing chosen, the
    // difficulty's base still shown as not recorded
    let p = plan_of("One", "One (b)");
    let r = &p.rows[0];
    assert!(!r.unrecorded && r.kind == RowKind::TheirsChanged && r.choice == Choice::Undecided && r.needs_decision(), "{r:?}");
    assert!(r.note.as_deref().is_some_and(|n| n.contains("no record of its difficulty in version 3")), "{r:?}");
    let d = r.diffs.iter().find(|d| d.field == "Difficulty").unwrap();
    assert_eq!((d.base.as_str(), d.ours.as_str(), d.theirs.as_str()), (NOT_RECORDED, "none", "Hard"));
    // both changed the question itself: a conflict as before (no default), its difficulty's base not recorded
    let p = plan_of("One (a)", "One (b)");
    let r = &p.rows[0];
    assert!(!r.unrecorded && r.kind == RowKind::Conflict && r.choice == Choice::Undecided, "{r:?}");
    assert_eq!(r.diffs.iter().find(|d| d.field == "Difficulty").map(|d| d.base.as_str()), Some("not recorded on this computer"));
    // a known base: as before
    let mut t = circle_theirs(&payload(&[(QA, "One")]), NOW);
    t[0].q.difficulty = Some(Difficulty::High);
    let r = &build_plan(BID, &link.base, &bank, &t, 3, 4, None, String::new()).rows[0];
    assert!(!r.unrecorded && r.kind == RowKind::TheirsChanged, "{r:?}");
}

// ---- c86 I2: a conflict on top of the person's own older version ---------------------------------------------------------

/// The teacher's send of "One (fix)" lost its answer and the check found her version 4; she changed it again, and Bob's
/// version 5 changed it too. The conflict shows the version both started from, her version 4.
#[test]
fn c86_i2_a_conflict_on_top_of_the_persons_own_older_version_shows_all_three() {
    let fix = |b: &mut QuestionBank| b.questions[0].stem = "One (fix)".into();
    let (mut m, s, mut banks, ctx, mut acct) = sent_with(&fix, true, timed_out(), 0, Value::Null, (200, heads_by(4, "Olga Teacher")));
    let who = olga_named(&s);
    frames(&mut m, &mut banks, &ctx, &mut acct, &who, 3);
    banks[0].questions[0].stem = "One (fix 2)".into();
    banks[0].modified += 5;
    s.keep_raw("Get", &version_path(4), served(4, &[(QA, "One (fix)")], false));
    let page = latest_as(&mut m, &s, &mut banks, &ctx, &mut acct, &who, own_then_bob(), served(5, &[(QA, "One (fix) by Bob")], false));
    assert_eq!(count(&kinds(&m).unwrap(), RowKind::Conflict), 1, "{page}");
    assert!(page.contains("In version 4 (your send): One (fix)"), "{page}");
    assert!(page.contains("Yours now: One (fix 2)") && page.contains("In version 5 (the circle's newest): One (fix) by Bob"), "{page}");
    assert!(!page.contains("In version 3:"), "{page}");
    // her own version the newest (critic 80's case): no line for an older version
    let (mut m, s, mut banks, ctx, mut acct) = sent_with(&fix, true, timed_out(), 0, Value::Null, (200, heads_by(4, "Olga Teacher")));
    let who = olga_named(&s);
    frames(&mut m, &mut banks, &ctx, &mut acct, &who, 3);
    banks[0].questions[0].stem = "One (fix 2)".into();
    banks[0].modified += 5;
    let page = latest_as(&mut m, &s, &mut banks, &ctx, &mut acct, &who, merge_start_by(3, 4, "Olga Teacher"), served(4, &[(QA, "One (fix)")], false));
    assert!(page.contains("Yours now: One (fix 2)") && page.contains("In version 4 (the circle's newest): One (fix)"), "{page}");
    assert!(!page.contains("In version 3:") && !page.contains("(your send)"), "{page}");
}

// ---- c86 minor: a send refused because the person's earlier send landed ---------------------------------------------------

/// The check found version 3 ("may not have arrived yet"); the teacher changed One and sent; the circle refused it because
/// her earlier send had landed as version 4. The strip says as the check path does: the earlier changes were sent, and only
/// what changed since is unsent (never "2 unsent changes", one of which went in version 4).
#[test]
fn c86_minor_a_send_refused_because_the_earlier_send_landed_counts_only_what_is_left() {
    let (mut m, s, mut banks, ctx, mut acct) = sent_with(&add_question, true, timed_out(), 0, Value::Null, (200, heads(3)));
    let who = olga_named(&s);
    frames(&mut m, &mut banks, &ctx, &mut acct, &who, 3);
    conflict_by(&mut m, &s, &mut banks, &ctx, &mut acct, &who, 4, "Olga Teacher");
    m.edit.close();
    let t = frames(&mut m, &mut banks, &ctx, &mut acct, &who, 2);
    assert!(t.contains("Your earlier changes were sent") && !t.contains("2 unsent"), "{t}");
    assert_eq!(badge(&mut m, &banks[0].clone()), "newer version · unsent changes");
    // a colleague's version 4 is not the person's: as before
    let (mut m, s, mut banks, ctx, mut acct) = sent_with(&add_question, true, timed_out(), 0, Value::Null, (200, heads(3)));
    let who = olga_named(&s);
    frames(&mut m, &mut banks, &ctx, &mut acct, &who, 3);
    conflict_by(&mut m, &s, &mut banks, &ctx, &mut acct, &who, 4, "Bob Teacher");
    m.edit.close();
    let t = frames(&mut m, &mut banks, &ctx, &mut acct, &who, 2);
    assert!(!t.contains("earlier changes were sent") && t.contains("2 unsent changes"), "{t}");
}

// ---- the person's own version ---------------------------------------------------------------------------------------------

/// (c85 m2) The person's own version could not be read at all (an answer that is not a version): Get latest still opens,
/// matched with the newest version, as when that version is no longer kept.
#[test]
fn c85_m2_an_error_reading_the_persons_own_version_never_stops_get_latest() {
    let (mut m, s, mut banks, ctx, mut acct) = sent_with(&add_question, true, timed_out(), 0, Value::Null, (200, heads_by(4, "Olga Teacher")));
    let who = olga_named(&s);
    frames(&mut m, &mut banks, &ctx, &mut acct, &who, 3);
    s.keep_raw("Get", &version_path(4), b"not a version".to_vec());
    let page = latest_as(&mut m, &s, &mut banks, &ctx, &mut acct, &who, own_then_bob(), served(5, &[(QA, "One (b)"), (QN, ADDED)], false));
    assert_eq!(version_reads(&s, 4), 1);
    let k = kinds(&m).unwrap();
    assert_eq!((count(&k, RowKind::TheirsChanged), count(&k, RowKind::NewInTheirs)), (1, 0), "{page}");
}

/// (survivor c2) An answer for another version than the one asked for is not taken for the person's own version.
#[test]
fn c85_the_persons_own_version_is_only_the_version_asked_for() {
    let fix = |b: &mut QuestionBank| b.questions[0].stem = "One (fix)".into();
    let (mut m, s, mut banks, ctx, mut acct) = sent_with(&fix, true, timed_out(), 0, Value::Null, (200, heads_by(4, "Olga Teacher")));
    let who = olga_named(&s);
    frames(&mut m, &mut banks, &ctx, &mut acct, &who, 3);
    s.keep_raw("Get", &version_path(4), served(6, &[(QA, "One (fix)")], false));
    let page = latest_as(&mut m, &s, &mut banks, &ctx, &mut acct, &who, own_then_bob(), served(5, &[(QA, "One (fix) by Bob")], false));
    assert_eq!(version_reads(&s, 4), 1);
    assert_eq!(count(&kinds(&m).unwrap(), RowKind::Conflict), 1, "{page}");
    assert!(!page.contains("is your own send"), "{page}");
}

/// (c85 m3, survivor c15) A copy with no record of the version it was made from (two-way), the person's name not known:
/// a colleague's version 4 that holds a question as the lost send carried it (unchanged) is never "your own send", and the
/// question is never "sent by you".
#[test]
fn c85_m3_in_a_two_way_merge_a_colleagues_version_is_never_taken_for_the_persons_own_send() {
    let (mut m, s, mut banks, ctx, mut acct) = sent_with(&add_question, false, timed_out(), 0, Value::Null, (503, json!({"error": "unavailable"})));
    let who = olga(&s);
    frames(&mut m, &mut banks, &ctx, &mut acct, &who, 2);
    m.edit.update_link(LOCAL, |l| {
        l.base.clear();
        true
    });
    banks[0].questions[0].stem = "One (13)".into();
    banks[0].modified += 5;
    let mut start = merge_start_by(3, 4, "Bob Teacher");
    start["base_kept"] = json!(false);
    let page = latest_as(&mut m, &s, &mut banks, &ctx, &mut acct, &who, start, served(4, &[(QA, "One")], false));
    assert!(page.contains("no longer kept"), "two-way: {page}");
    assert!(!page.contains("is your own send") && !page.contains("You sent this"), "{page}");
}

/// (c85 m4) A link an earlier build wrote, a lost send that changed only a difficulty, and the check found the person's
/// version 4 by name: the review says version 4 is her own send.
#[test]
fn c85_m4_a_lost_send_that_changed_only_a_difficulty_is_the_persons_own() {
    let (mut m, s, mut banks) = two_at_three();
    not_kept(&s, 3);
    let who = olga_named(&s);
    s.keep("Get", &format!("/circles/{CID}"), 200, detail("owner", json!({})));
    let ctx = ctx();
    let mut acct = FakeAccount { open: true, ..FakeAccount::default() };
    old_base(&mut m);
    banks[0].questions[0].difficulty = Some(Difficulty::High);
    banks[0].modified += 5;
    frames(&mut m, &mut banks, &ctx, &mut acct, &who, 2);
    assert!(m.edit.open_send(LOCAL, owner()));
    host_a(&mut m);
    frames(&mut m, &mut banks, &ctx, &mut acct, &who, 3);
    m.edit.set_attest(true);
    s.on("Post", "/circles/heads", 200, heads(3));
    s.fail(timed_out().unwrap());
    m.edit.send();
    m.edit.sync(&mut banks);
    assert!(s.paths().iter().any(|p| p.starts_with("Put ")), "{:?}", s.paths());
    s.on("Post", "/circles/heads", 200, heads_by(4, "Olga Teacher"));
    frames(&mut m, &mut banks, &ctx, &mut acct, &who, 3);
    m.edit.close();
    let page = latest_as(&mut m, &s, &mut banks, &ctx, &mut acct, &who, merge_start_by(3, 4, "Olga Teacher"), served_qs(4, vec![rated(QA, "One", Some("High"))], false));
    assert!(page.contains("Version 4 is your own send"), "{page}");
    assert_eq!(kinds(&m).unwrap(), vec![RowKind::Unchanged], "{page}");
}

// ---- c85 P7: a difficulty as the server stores it ---------------------------------------------------------------------------

/// Every difficulty this app writes (none, Low, Medium, High, the key left out) comes back from what verSHion's server stores
/// as the same question; one this app can not read is not a question it would send.
#[test]
fn c85_p7_a_difficulty_round_trips_through_what_the_server_stores() {
    let stored = |q: &Value| -> Option<Value> {
        let body = json!({"schema_version": 2, "bank": {"id": BID, "name": "Cells", "questions": [q], "created": 1, "modified": 2}}).to_string();
        let out: Value = serde_json::from_str(&share_core::wasm::publish_prepare_json(body.as_bytes(), "{}")).unwrap();
        (out["ok"] == true).then(|| out["payload"]["bank"]["questions"][0].clone())
    };
    for d in [json!(null), json!("Low"), json!("Medium"), json!("High")] {
        let mut q = qjson(QA, "One");
        q["difficulty"] = d.clone();
        let local: crate::model::Question = serde_json::from_value(q.clone()).unwrap();
        let back: crate::model::Question = serde_json::from_value(stored(&q).expect("stored")).unwrap();
        assert_eq!(back.difficulty, local.difficulty, "{d}");
        assert!(super::merge::same_question(&local, &back), "{d}");
    }
    let mut q = qjson(QA, "One");
    q.as_object_mut().unwrap().remove("difficulty");
    assert_eq!(stored(&q).expect("stored").get("difficulty"), Some(&Value::Null));
    for d in [json!("low"), json!("Hard"), json!(2), json!("")] {
        let mut q = qjson(QA, "One");
        q["difficulty"] = d.clone();
        assert!(serde_json::from_value::<crate::model::Question>(q).is_err(), "{d}");
    }
}

// ---- c85 m1: the person's own version can not be read -----------------------------------------------------------------------

/// A send whose answer was lost, its version 4 found by the check ("sent by you") but no longer kept when Get latest reads,
/// and Bob's version 5 after it. `edit` is what the send carried; `newest` is Bob's version 5.
pub(super) fn own_unreadable(edit: &dyn Fn(&mut QuestionBank), newest: &[(&str, &str)]) -> (Rig, String) {
    let (mut m, s, mut banks, ctx, mut acct) = sent_with(edit, true, timed_out(), 0, Value::Null, (200, heads_by(4, "Olga Teacher")));
    let who = olga_named(&s);
    frames(&mut m, &mut banks, &ctx, &mut acct, &who, 3);
    s.keep("Get", &version_path(4), 404, json!({"error": "version_not_retained", "oldest_kept": 5}));
    let page = latest_as(&mut m, &s, &mut banks, &ctx, &mut acct, &who, own_then_bob(), served(5, newest, false));
    ((m, s, banks, ctx, acct), page)
}

/// (P3) Bob corrected the question the lost send added: the circle's question is not added by default (it may be the
/// teacher's own, which would then be in her bank twice); the row and the review say why. Not adding it leaves no duplicate.
#[test]
fn c85_m1_a_question_the_unreadable_send_may_have_added_is_not_added_by_default() {
    let ((mut m, s, mut banks, ctx, mut acct), page) = own_unreadable(&add_question, &[(QA, "One"), (QN, "Solve 5x = 21")]);
    let rows = rows_of(&m);
    let new = rows.iter().position(|r| r.kind == RowKind::NewInTheirs).expect("listed");
    assert_eq!(rows[new].choice, Choice::Undecided, "{page}");
    assert!(!m.edit.can_apply_merge(), "{page}");
    assert!(rows[new].note.as_deref().is_some_and(|n| n.contains("may be a question your earlier send added")), "{:?}", rows[new].note);
    assert!(page.contains("can't be read now") && page.contains("Choose one to continue"), "{page}");
    assert!(page.contains("1 question still needs a decision"), "{page}");
    m.edit.set_merge_choice(new, Choice::Skip);
    apply(&mut m, &s, &mut banks, &ctx, &mut acct);
    assert_eq!(banks[0].questions.iter().map(|q| q.stem.as_str()).collect::<Vec<_>>(), vec!["One", ADDED], "no duplicate");
}

/// (P4) Bob changed back what the lost send changed: the row never says "Only you changed this"; it says the send's version
/// can not be read, so whether someone changed it since is not known.
#[test]
fn c85_m1_a_change_the_unreadable_send_carried_is_not_said_to_be_only_the_teachers() {
    let fix = |b: &mut QuestionBank| b.questions[0].stem = "One (fix)".into();
    let ((m, _, _, _, _), page) = own_unreadable(&fix, &[(QA, "One")]);
    assert_eq!(kinds(&m).unwrap(), vec![RowKind::OursChanged], "{page}");
    assert!(!page.contains("Only you changed this"), "{page}");
    assert!(notes(&m).iter().any(|n| n.contains("not known whether someone changed it since")), "{:?}", notes(&m));
    assert!(page.contains("Version 4 may be your earlier send") && page.contains("can't be read now"), "{page}");
}

/// Bob removed the question the lost send added: no row can name it, so the review says so plainly; a question new in the
/// circle is added as usual when the send added nothing that is left unmatched.
#[test]
fn c85_m1_the_review_says_when_what_the_send_added_can_not_be_matched() {
    let ((m, _, _, _, _), page) = own_unreadable(&add_question, &[(QA, "One")]);
    assert!(page.contains("Version 4 may be your earlier send") && page.contains("can't be read now"), "{page}");
    assert!(page.contains("yours alone"), "{page}");
    assert!(m.edit.can_apply_merge(), "{page}");
    // a send that added nothing: a colleague's new question is added as before
    let fix = |b: &mut QuestionBank| b.questions[0].stem = "One (fix)".into();
    let ((m, _, _, _, _), page) = own_unreadable(&fix, &[(QA, "One (fix)"), (QN, "Bob's question")]);
    let rows = rows_of(&m);
    assert!(rows.iter().any(|r| r.kind == RowKind::NewInTheirs && r.choice == Choice::Add), "{page}");
    assert!(m.edit.can_apply_merge(), "{page}");
}

/// The added question is in Bob's version 5 as sent: that is evidence the send went through, so the review says version 4 is
/// the person's own send, and never that it "may be".
#[test]
fn c85_m1_an_unreadable_own_version_proven_by_what_the_newest_holds_is_said_to_be_hers() {
    let ((m, _, _, _, _), page) = own_unreadable(&add_question, &[(QA, "One"), (QN, ADDED)]);
    assert!(page.contains("Version 4 is your own send") && !page.contains("may be your earlier send"), "{page}");
    assert!(m.edit.can_apply_merge(), "{page}");
}

/// Her own version 4 can not be read and Bob's version 5 holds her change as she sent it; she changed it again since: it is
/// compared with version 5 only, never shown as "your send" (that is not the version it was matched to).
#[test]
fn c86_i2_a_question_matched_to_the_newest_in_place_of_the_persons_own_version_has_two_lines() {
    let fix = |b: &mut QuestionBank| b.questions[0].stem = "One (fix)".into();
    let (mut m, s, mut banks, ctx, mut acct) = sent_with(&fix, true, timed_out(), 0, Value::Null, (200, heads_by(4, "Olga Teacher")));
    let who = olga_named(&s);
    frames(&mut m, &mut banks, &ctx, &mut acct, &who, 3);
    banks[0].questions[0].stem = "One (fix 2)".into();
    banks[0].modified += 5;
    s.keep("Get", &version_path(4), 404, json!({"error": "version_not_retained", "oldest_kept": 5}));
    let page = latest_as(&mut m, &s, &mut banks, &ctx, &mut acct, &who, own_then_bob(), served(5, &[(QA, "One (fix)")], false));
    assert_eq!(kinds(&m).unwrap(), vec![RowKind::OursChanged], "{page}");
    assert!(page.contains("Yours now: One (fix 2)") && page.contains("In version 5 (the circle's newest): One (fix)"), "{page}");
    assert!(!page.contains("(your send)") && !page.contains("In version 3:"), "{page}");
}

/// (c85 m4) The person's name on version 4 alone is not enough: a version 4 that does not hold what the send carried (sent
/// from elsewhere under the same name) is never "your own send".
#[test]
fn c85_m4_a_version_by_the_persons_name_that_does_not_hold_the_send_is_not_hers() {
    let fix = |b: &mut QuestionBank| b.questions[0].stem = "One (fix)".into();
    let (mut m, s, mut banks, ctx, mut acct) = sent_with(&fix, true, timed_out(), 0, Value::Null, (200, heads_by(4, "Olga Teacher")));
    let who = olga_named(&s);
    frames(&mut m, &mut banks, &ctx, &mut acct, &who, 3);
    let page = latest_as(&mut m, &s, &mut banks, &ctx, &mut acct, &who, merge_start_by(3, 4, "Olga Teacher"), served(4, &[(QA, "One (other)")], false));
    assert!(!page.contains("is your own send"), "{page}");
    // (it is the newest, read for this review: never said to be unreadable)
    assert!(!page.contains("can't be read now"), "{page}");
    assert_eq!(kinds(&m).unwrap(), vec![RowKind::Conflict], "{page}");
}

/// (c85 m1) The person's version 4 was read (colleagues sent more since) but does not hold what the send carried: it is not
/// her send, and the review never says it can not be read.
#[test]
fn c85_m1_a_readable_own_version_is_never_said_to_be_unreadable() {
    let fix = |b: &mut QuestionBank| b.questions[0].stem = "One (fix)".into();
    let (mut m, s, mut banks, ctx, mut acct) = sent_with(&fix, true, timed_out(), 0, Value::Null, (200, heads_by(4, "Olga Teacher")));
    let who = olga_named(&s);
    frames(&mut m, &mut banks, &ctx, &mut acct, &who, 3);
    s.keep_raw("Get", &version_path(4), served(4, &[(QA, "One (other)")], false));
    let page = latest_as(&mut m, &s, &mut banks, &ctx, &mut acct, &who, own_then_bob(), served(5, &[(QA, "One (other) by Bob")], false));
    assert_eq!(version_reads(&s, 4), 1);
    assert!(!page.contains("can't be read now") && !page.contains("is your own send"), "{page}");
}

/// (c85 m1) Only what the send changed is said to have been sent: a question it carried unchanged and the teacher edited
/// afterwards is still "Only you changed this", also while her version 4 can not be read.
#[test]
fn c85_m1_a_question_the_unreadable_send_carried_unchanged_is_still_only_the_teachers() {
    let (mut m, s, mut banks, ctx, mut acct) = sent_with(&add_question, true, timed_out(), 0, Value::Null, (200, heads_by(4, "Olga Teacher")));
    let who = olga_named(&s);
    frames(&mut m, &mut banks, &ctx, &mut acct, &who, 3);
    banks[0].questions[0].stem = "One (13)".into();
    banks[0].modified += 5;
    s.keep("Get", &version_path(4), 404, json!({"error": "version_not_retained", "oldest_kept": 5}));
    let page = latest_as(&mut m, &s, &mut banks, &ctx, &mut acct, &who, own_then_bob(), served(5, &[(QA, "One"), (QN, "Solve 5x = 21")], false));
    assert_eq!(count(&kinds(&m).unwrap(), RowKind::OursChanged), 1, "{page}");
    assert!(notes(&m).iter().any(|n| n.contains("Only you changed this")), "{:?}", notes(&m));
    assert!(!page.contains("You sent a change"), "{page}");
}

/// (c85 m4) A send that carried no question the circle knew (it replaced them all) is the person's own only when its version
/// holds what it added: her name on version 4 is not enough.
#[test]
fn c85_m4_a_send_that_carried_only_new_questions_is_hers_only_by_what_it_added() {
    let replace = |b: &mut QuestionBank| {
        b.questions.clear();
        add_question(b);
    };
    let (mut m, s, mut banks, ctx, mut acct) = sent_with(&replace, true, timed_out(), 0, Value::Null, (200, heads_by(4, "Olga Teacher")));
    let who = olga_named(&s);
    frames(&mut m, &mut banks, &ctx, &mut acct, &who, 3);
    let page = latest_as(&mut m, &s, &mut banks, &ctx, &mut acct, &who, merge_start_by(3, 4, "Olga Teacher"), served(4, &[(QA, "One"), (QN, "Another question")], false));
    assert!(!page.contains("is your own send"), "{page}");
}
