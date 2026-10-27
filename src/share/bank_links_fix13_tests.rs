//! Phase D1, round 13 (critics 87 and 88): each test written first against the code it pins. A link an earlier build wrote
//! (no difficulty in its base) learns its difficulties only from the circle's own version at its base version, read once,
//! never from the bank; when that version can't be read, the count is not complete and a question whose difficulty differs
//! is the teacher's decision, with no choice made for her.

use serde_json::{Value, json};

use super::bank_links_fix10_tests::*;
use super::bank_links_fix11_tests::{host_a, own_then_bob, rated, rig, version_path, version_reads};
use super::bank_links_fix4_tests::*;
use super::bank_links_fix8_tests::*;
use super::bank_links_fix9_tests::*;
use super::bank_links_fix_tests::*;
use super::bank_links_tests::*;
use super::circle_edit::*;
use super::circles::CirclesModel;
use super::circles_tests::*;
use super::merge::{Choice, RowKind};
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

fn rows_of(m: &CirclesModel) -> Vec<super::merge::Row> {
    match &m.edit.view {
        EditView::Merge(v) => match &v.phase {
            MergePhase::Review(r) => r.plan.rows.clone(),
            _ => Vec::new(),
        },
        _ => Vec::new(),
    }
}

fn row_at(m: &CirclesModel, k: RowKind) -> (usize, super::merge::Row) {
    rows_of(m).into_iter().enumerate().find(|(_, r)| r.kind == k).unwrap_or_else(|| panic!("no {k:?} row: {:?}", rows_of(m).iter().map(|r| r.kind).collect::<Vec<_>>()))
}

/// A's link and bank as the phase C build (HEAD) left them after a Get latest to `version` (critic 87's head_link.json):
/// questions One and Two, a base that kept no difficulty (HEAD's merge ignored difficulty, so it kept the bank's as it
/// was), the bank's One rated `one`, and the bank marked with the link (its print matches).
fn phase_c(m: &mut CirclesModel, banks: &mut [QuestionBank], version: u64, one: Option<Difficulty>) {
    let (mut bank, mut link) = linked(&[(QA, "One"), (QB, "Two")]);
    bank.questions[0].difficulty = one;
    for b in &mut link.base {
        b.snap.difficulty = None;
    }
    link.base_version = version;
    link.base_version_id = hex(version);
    link.head_seen = version;
    link.mark_bank(&bank);
    m.edit.update_link(LOCAL, |l| {
        link.role_seen = l.role_seen.clone();
        link.author_known = l.author_known;
        link.is_author = l.is_author;
        *l = link;
        true
    });
    banks[0] = bank;
}

fn send_page(m: &mut CirclesModel, s: &std::sync::Arc<Script>, banks: &mut [QuestionBank], ctx: &egui::Context, acct: &mut FakeAccount) -> String {
    assert!(m.edit.open_send(LOCAL, owner()));
    host_a(m);
    frames(m, banks, ctx, acct, &olga_named(s), 3)
}

// ---- c87 B1: a link an earlier build wrote learns its difficulties from the circle only -----------------------------------

/// (P1x) HEAD's Get latest kept a "Hard" on One (it ignored difficulty) and marked the bank. This build reads the circle's
/// version 4 (One has none). (round 15, the owner's rule) After the read the copy's difficulty is the circle's: nothing is
/// counted, and Bob's version 5, which left One alone, is plain.
#[test]
fn c87_b1_p1x_after_the_read_the_copy_has_the_circles_difficulty() {
    let (mut m, s, mut banks, ctx, mut acct) = rig();
    s.keep("Post", "/circles/heads", 200, heads(4));
    phase_c(&mut m, &mut banks, 4, Some(Difficulty::High));
    s.keep_raw("Get", &version_path(4), served_qs(4, vec![rated(QA, "One", None), rated(QB, "Two", None)], false));
    let who = olga_named(&s);
    let strip = frames(&mut m, &mut banks, &ctx, &mut acct, &who, 3);
    assert_eq!(banks[0].questions[0].difficulty, None, "{strip}");
    assert_eq!(changes(&mut m, &banks[0].clone()), (0, 0, 0), "{strip}");
    assert!(strip.contains("up to date"), "{strip}");
    // (round 17, critic 96 I1) her High replaced by the circle's none is said until she dismisses it
    assert_eq!(badge(&mut m, &banks[0].clone()), "difficulty updated");
    frames(&mut m, &mut banks, &ctx, &mut acct, &who, 3);
    assert_eq!(version_reads(&s, 4), 1, "read once");
    let page = latest_as(&mut m, &s, &mut banks, &ctx, &mut acct, &who, merge_start_by(4, 5, "Bob Teacher"), served_qs(5, vec![rated(QA, "One", None), rated(QB, "Two", None)], false));
    assert_eq!(kinds(&m).unwrap(), vec![RowKind::Unchanged, RowKind::Unchanged], "{page}");
    apply(&mut m, &s, &mut banks, &ctx, &mut acct);
    assert_eq!(banks[0].questions[0].difficulty, None);
    assert_eq!(changes(&mut m, &banks[0].clone()), (0, 0, 0));
}

/// (P7) HEAD's Get latest dropped Bob's "Hard" on One (the bank kept none) and marked the bank. This build reads the
/// circle's version 4 (One is Hard there). (round 14, the owner's decision of 2026-10-26, critic 90 I1) The copy inherits his
/// rating: nothing is counted as hers, and the Send page and the strip say the same.
#[test]
fn c87_b1_p7_a_rating_a_phase_c_merge_dropped_is_inherited() {
    let (mut m, s, mut banks, ctx, mut acct) = rig();
    s.keep("Post", "/circles/heads", 200, heads(4));
    phase_c(&mut m, &mut banks, 4, None);
    s.keep_raw("Get", &version_path(4), served_qs(4, vec![rated(QA, "One", Some("High")), rated(QB, "Two", None)], false));
    let strip = frames(&mut m, &mut banks, &ctx, &mut acct, &olga_named(&s), 3);
    assert!(strip.contains("up to date"), "{strip}");
    assert_eq!(banks[0].questions[0].difficulty, Some(Difficulty::High));
    assert_eq!(changes(&mut m, &banks[0].clone()), (0, 0, 0), "{strip}");
    let page = send_page(&mut m, &s, &mut banks, &ctx, &mut acct);
    assert!(page.contains("Nothing in your copy differs"), "{page}");
}

/// (P3) "Use my bank as it is" on an earlier build's link whose copy was put back with One rated "Easy": the circle's version
/// 3 has none. The bank's rating is never taken for the circle's. (round 16, critic 93 I2) Her choice settles the read: the
/// bank is sent as it is (Low), whatever the read would have brought; what version 3 rated stays unknown, so the count says a
/// change to a difficulty can't be counted, and Get latest asks her about One instead of choosing for her.
#[test]
fn c87_b1_p3_using_the_bank_as_it_is_never_takes_its_rating_for_the_circles() {
    let (mut m, s, mut banks, ctx, mut acct) = rig();
    old_base(&mut m);
    s.keep_raw("Get", &version_path(3), served_qs(3, vec![rated(QA, "One", None)], false));
    banks[0].questions[0].difficulty = Some(Difficulty::Low);
    make_stale(&mut m, LOCAL);
    treat_in_step(&mut m, &s, &mut banks, LOCAL, BID, false);
    let strip = frames(&mut m, &mut banks, &ctx, &mut acct, &olga_named(&s), 3);
    assert_eq!(m.edit.with_link(LOCAL, |l| l.base[0].snap.difficulty).unwrap(), None, "{strip}");
    assert_eq!(banks[0].questions[0].difficulty, Some(Difficulty::Low), "{strip}");
    let c = m.edit.status_of(&banks[0].clone()).unwrap().changes;
    assert!(c.partial() && c.total() == 0, "{c:?}: {strip}");
    assert!(strip.contains("Send my changes"), "{strip}");
    let page = get_latest(&mut m, &s, &mut banks, &ctx, &mut acct, served_qs(4, vec![rated(QA, "One", None)], false));
    assert_eq!(kinds(&m).unwrap(), vec![RowKind::Conflict], "{page}");
    assert!(page.contains("Choose one to continue"), "nothing chosen for her: {page}");
}

/// The bank exactly as the app last wrote it is no evidence: a link whose bank print matches learns nothing from the bank,
/// only from the circle's version (here One is "Hard" in the bank and none in version 3). (round 15) The copy then has the
/// circle's.
#[test]
fn c87_b1_the_bank_as_last_written_is_never_taken_for_the_circles_version() {
    let (mut m, s, mut banks, ctx, mut acct) = rig();
    old_base(&mut m);
    banks[0].questions[0].difficulty = Some(Difficulty::High);
    let b = banks[0].clone();
    m.edit.update_link(LOCAL, |l| {
        l.mark_bank(&b);
        true
    });
    // before the circle's version is read (no sign-in yet): never complete
    let c = m.edit.status_of(&banks[0].clone()).unwrap().changes;
    assert!(!c.known, "{c:?}");
    s.keep_raw("Get", &version_path(3), served_qs(3, vec![rated(QA, "One", None)], false));
    let strip = frames(&mut m, &mut banks, &ctx, &mut acct, &olga_named(&s), 3);
    assert_eq!(m.edit.with_link(LOCAL, |l| l.base[0].snap.difficulty).unwrap(), Some(None), "the circle's, kept in the link");
    assert_eq!(banks[0].questions[0].difficulty, None, "{strip}");
    assert_eq!(changes(&mut m, &banks[0].clone()), (0, 0, 0), "{strip}");
}

/// The circle's version can't be read (no longer kept): nothing is learnt and it is not asked again on every frame; the
/// count is never complete, and Get latest leaves a differing difficulty undecided.
#[test]
fn c87_b1_a_base_version_that_cannot_be_read_leaves_the_count_not_complete() {
    let (mut m, s, mut banks, ctx, mut acct) = rig();
    old_base(&mut m);
    s.keep("Get", &version_path(3), 404, json!({"error": "version_not_retained", "oldest_kept": 4}));
    banks[0].questions[0].difficulty = Some(Difficulty::High);
    banks[0].modified += 5;
    let who = olga_named(&s);
    let strip = frames(&mut m, &mut banks, &ctx, &mut acct, &who, 3);
    frames(&mut m, &mut banks, &ctx, &mut acct, &who, 3);
    assert_eq!(version_reads(&s, 3), 1, "asked once");
    assert!(!m.edit.status_of(&banks[0].clone()).unwrap().changes.known);
    assert!(!strip.contains("up to date"), "{strip}");
    assert_eq!(m.edit.with_link(LOCAL, |l| l.base[0].snap.difficulty).unwrap(), None);
    let page = get_latest(&mut m, &s, &mut banks, &ctx, &mut acct, served_qs(4, vec![rated(QA, "One", None)], false));
    let (_, r) = row_at(&m, RowKind::Conflict);
    assert!(r.unrecorded && r.choice == Choice::Undecided, "{page}");
    assert!(!m.edit.can_apply_merge(), "{page}");
}

/// The circle's version read for Get latest (the quiet read had not happened yet) is what the review compares with, and the
/// link keeps what it learnt. (round 15) The copy takes the circle's difficulty before the review is made.
#[test]
fn c87_b1_get_latest_learns_the_difficulties_from_the_version_it_reads() {
    let (mut m, s, mut banks, _ctx, _acct) = rig();
    old_base(&mut m);
    banks[0].questions[0].difficulty = Some(Difficulty::High);
    banks[0].modified += 5;
    s.keep_raw("Get", &version_path(3), served_qs(3, vec![rated(QA, "One", None)], false));
    s.on("Post", &format!("{}/merge", put_path()), 200, merge_start(3, 4));
    s.keep_raw("Get", &put_path(), served_qs(4, vec![rated(QA, "One", None)], false));
    // (the merge's own read, the answers taken while the account is being looked up: no quiet read starts)
    assert!(m.edit.open_merge(LOCAL, owner()));
    let checking = super::publish::AccountSnapshot::of_gate(super::publish::Gate::Checking);
    for _ in 0..3 {
        m.edit.frame(now(), &checking);
        m.edit.sync(&mut banks);
    }
    assert_eq!(kinds(&m).unwrap(), vec![RowKind::Unchanged]);
    assert_eq!(banks[0].questions[0].difficulty, None);
    assert_eq!(version_reads(&s, 3), 1);
    assert_eq!(m.edit.with_link(LOCAL, |l| l.base[0].snap.difficulty).unwrap(), Some(None));
}

// ---- c87 I1, c88 I1: a difficulty this computer has no record of is never decided for the teacher ---------------------------

/// (P2) The teacher's own rating, version 3 not readable, Bob's version 4 not touching One: the row waits for her choice
/// (Apply with nothing chosen can't undo her rating), and Keep mine keeps it.
#[test]
fn c87_i1_p2_the_teachers_rating_is_never_undone_by_a_default() {
    let (mut m, s, mut banks, ctx, mut acct) = rig();
    old_base(&mut m);
    super::bank_links_fix12_tests::not_kept(&s, 3);
    banks[0].questions[0].difficulty = Some(Difficulty::High);
    banks[0].modified += 5;
    let page = get_latest(&mut m, &s, &mut banks, &ctx, &mut acct, served_qs(4, vec![rated(QA, "One", None)], false));
    let (i, r) = row_at(&m, RowKind::Conflict);
    assert!(r.unrecorded && r.choice == Choice::Undecided, "{page}");
    assert!(!m.edit.can_apply_merge(), "{page}");
    // (survivor w5) a question, never "a conflict", still needs a decision
    assert!(page.contains("1 question still needs a decision") && !page.contains("1 conflict still needs"), "{page}");
    assert!(page.contains("Choose which to keep") && page.contains("Choose one to continue"), "{page}");
    m.edit.set_merge_choice(i, Choice::Ours);
    apply(&mut m, &s, &mut banks, &ctx, &mut acct);
    assert_eq!(banks[0].questions[0].difficulty, Some(Difficulty::High));
    // (round 15) version 3 readable: One had none there, so after the read the copy has none either: nothing to decide
    let (mut m, s, mut banks, ctx, mut acct) = rig();
    old_base(&mut m);
    s.keep_raw("Get", &version_path(3), served_qs(3, vec![rated(QA, "One", None)], false));
    banks[0].questions[0].difficulty = Some(Difficulty::High);
    banks[0].modified += 5;
    let page = get_latest(&mut m, &s, &mut banks, &ctx, &mut acct, served_qs(4, vec![rated(QA, "One", None)], false));
    assert_eq!(kinds(&m).unwrap(), vec![RowKind::Unchanged], "{page}");
    apply(&mut m, &s, &mut banks, &ctx, &mut acct);
    assert_eq!(banks[0].questions[0].difficulty, None);
}

/// (P2b) The same, Bob also reworded One: no choice is made for her either way. (round 14, critic 90 I2) His rewording is
/// said as his ("Changed in the circle"), and Keep mine keeps her wording and her rating.
#[test]
fn c87_i1_p2b_the_teachers_rating_against_a_colleagues_rewording_is_hers_to_decide() {
    let (mut m, s, mut banks, ctx, mut acct) = rig();
    old_base(&mut m);
    super::bank_links_fix12_tests::not_kept(&s, 3);
    banks[0].questions[0].difficulty = Some(Difficulty::High);
    banks[0].modified += 5;
    let page = get_latest(&mut m, &s, &mut banks, &ctx, &mut acct, served_qs(4, vec![rated(QA, "One (b)", None)], false));
    let (i, r) = row_at(&m, RowKind::TheirsChanged);
    assert!(!r.unrecorded && r.choice == Choice::Undecided, "{page}");
    assert!(!m.edit.can_apply_merge(), "{page}");
    m.edit.set_merge_choice(i, Choice::Keep);
    apply(&mut m, &s, &mut banks, &ctx, &mut acct);
    assert_eq!((banks[0].questions[0].stem.as_str(), banks[0].questions[0].difficulty), ("One", Some(Difficulty::High)));
}

/// (c87 m1, P6) "Keep both" would add a second copy that differs only in difficulty: never offered for such a row (a base
/// that kept none, or one where both rated it differently); still offered where the questions differ otherwise.
#[test]
fn c87_m1_keep_both_is_not_offered_where_only_the_difficulty_differs() {
    use super::circle_links::circle_theirs;
    use super::merge::build_plan;
    let (bank, link) = linked(&[(QA, "One")]);
    let plan_of = |base_d: Option<Option<Difficulty>>, ours: (&str, Option<Difficulty>), theirs: (&str, Option<Difficulty>)| {
        let mut base = link.base.clone();
        base[0].snap.difficulty = base_d;
        let mut b = bank.clone();
        b.questions[0].stem = ours.0.into();
        b.questions[0].difficulty = ours.1;
        let mut t = circle_theirs(&payload(&[(QA, theirs.0)]), NOW);
        t[0].q.difficulty = theirs.1;
        build_plan(BID, &base, &b, &t, 3, 4, None, String::new()).rows[0].clone()
    };
    let r = plan_of(None, ("One", None), ("One", Some(Difficulty::High)));
    assert!(r.kind == RowKind::Conflict && r.options == vec![Choice::Ours, Choice::Theirs], "{r:?}");
    let r = plan_of(Some(None), ("One", Some(Difficulty::Low)), ("One", Some(Difficulty::High)));
    assert!(r.kind == RowKind::Conflict && r.options == vec![Choice::Ours, Choice::Theirs], "{r:?}");
    let r = plan_of(Some(None), ("One (a)", Some(Difficulty::Low)), ("One", Some(Difficulty::High)));
    assert!(r.kind == RowKind::Conflict && r.options.contains(&Choice::Both), "{r:?}");
    let r = plan_of(None, ("One (a)", None), ("One (b)", Some(Difficulty::High)));
    assert!(r.kind == RowKind::Conflict && r.options.contains(&Choice::Both), "{r:?}");
    // (round 14, critic 90 I2) only the circle's wording changed: its change to take or not, never a second copy
    let r = plan_of(None, ("One", None), ("One (b)", Some(Difficulty::High)));
    assert!(r.kind == RowKind::TheirsChanged && r.options == vec![Choice::Keep, Choice::Take], "{r:?}");
}

// ---- c87 m2: the person's own version not read because the circle could not be asked ---------------------------------------

/// The read of the person's own version met a network failure, or a problem of the circle's own: Get latest says that (to try
/// again), never that the version "can't be read now".
#[test]
fn c87_m2_a_failure_to_reach_the_circle_is_said_as_that_never_as_an_unreadable_version() {
    for (name, fail) in [("offline", None), ("server", Some(503u16))] {
        let fix = |b: &mut QuestionBank| b.questions[0].stem = "One (fix)".into();
        let (mut m, s, mut banks, ctx, mut acct) = sent_with(&fix, true, timed_out(), 0, Value::Null, (200, heads_by(4, "Olga Teacher")));
        let who = olga_named(&s);
        frames(&mut m, &mut banks, &ctx, &mut acct, &who, 3);
        s.on("Post", &format!("{}/merge", put_path()), 200, own_then_bob());
        s.on_raw("Get", &put_path(), served(5, &[(QA, "One (fix) by Bob")], false));
        match fail {
            Some(st) => {
                s.on("Get", &version_path(4), st, json!({"error": "server_error"}));
            }
            None => {
                s.fail(timed_out().unwrap());
            }
        }
        assert!(m.edit.open_merge(LOCAL, owner()));
        host_a(&mut m);
        let page = frames(&mut m, &mut banks, &ctx, &mut acct, &who, 3);
        assert_eq!(version_reads(&s, 4), 1, "{name}");
        assert!(matches!(&m.edit.view, EditView::Merge(v) if matches!(v.phase, MergePhase::Failed(_))), "{name}: {page}");
        assert!(!page.contains("can't be read now") && page.to_lowercase().contains("try again"), "{name}: {page}");
    }
}

// ---- c88 I3: an own version that can't be read ------------------------------------------------------------------------------

/// The check's note says the send went through; the review can't read that version and says it "may be" the send: the note
/// is not shown above that review (it is not lost: the strip keeps it).
#[test]
fn c88_i3_the_review_and_the_note_above_it_never_say_opposite_things() {
    let fix = |b: &mut QuestionBank| b.questions[0].stem = "One (fix)".into();
    let ((m, _, _, _, _), page) = super::bank_links_fix12_tests::own_unreadable(&fix, &[(QA, "One")]);
    assert!(page.contains("Version 4 may be your earlier send"), "{page}");
    assert!(!page.contains("went through"), "{page}");
    assert!(m.edit.results_of(LOCAL).iter().any(|(_, t)| t.contains("went through")), "kept for the strip: {:?}", m.edit.results_of(LOCAL));
}

/// A change the unreadable send carried, which the circle's newest no longer has (someone may have changed it since): the row
/// offers Keep mine (chosen) and Take the circle's version, which takes it.
#[test]
fn c88_i3_a_change_the_unreadable_send_carried_can_be_given_up_for_the_circles() {
    let fix = |b: &mut QuestionBank| b.questions[0].stem = "One (fix)".into();
    let ((mut m, s, mut banks, ctx, mut acct), page) = super::bank_links_fix12_tests::own_unreadable(&fix, &[(QA, "One")]);
    let (i, r) = row_at(&m, RowKind::OursChanged);
    assert!(r.note.as_deref().is_some_and(|n| n.contains("not known whether someone changed it since")), "{r:?}");
    assert_eq!((r.options.clone(), r.choice), (vec![Choice::Keep, Choice::Take], Choice::Keep), "{page}");
    assert!(page.contains("Keep mine") && page.contains("Take the circle's version"), "{page}");
    assert!(m.edit.can_apply_merge());
    m.edit.set_merge_choice(i, Choice::Take);
    apply(&mut m, &s, &mut banks, &ctx, &mut acct);
    assert_eq!(banks[0].questions[0].stem, "One");
    assert_eq!(changes(&mut m, &banks[0].clone()), (0, 0, 0));
}

// ---- c88 I2: a count that is complete except for a difficulty ------------------------------------------------------------------

/// An earlier build's link whose version 3 lacked its questions' difficulties (round 15: read once, nothing more to learn), one
/// question added: the badge, the strip and the Send page each say 1 unsent / 1 added, and the strip and the Send page say why
/// a change to a difficulty may be missing; never "Press Send to see what you changed", nor "no record of what it looked like"
/// (the copy has every question but the difficulties).
#[test]
fn c88_i2_what_is_counted_is_said_with_the_difficulty_caveat() {
    let (mut m, s, mut banks, ctx, mut acct) = rig();
    old_base(&mut m);
    super::bank_links_fix12_tests::read_lacking(&mut m);
    super::bank_links_fix12_tests::not_kept(&s, 3);
    add_question(&mut banks[0]);
    banks[0].modified += 5;
    let strip = frames(&mut m, &mut banks, &ctx, &mut acct, &olga_named(&s), 3);
    let caveat = "This copy has no record of the difficulties in version 3, so a change to a difficulty can't be counted here.";
    assert!(strip.contains("You have 1 unsent change.") && strip.contains(caveat), "{strip}");
    assert!(!strip.contains("Press Send to see") && !strip.contains("up to date"), "{strip}");
    assert_eq!(badge(&mut m, &banks[0].clone()), "1 unsent change");
    let page = send_page(&mut m, &s, &mut banks, &ctx, &mut acct);
    assert!(page.contains("Since your copy last matched the circle: 1 added, 0 changed, 0 removed.") && page.contains(caveat), "{page}");
    assert!(!page.contains("no record of what it looked like"), "{page}");
}

/// The same with only a difficulty changed (nothing counted): the strip and the Send page say the same thing, and never that
/// the copy is up to date.
#[test]
fn c88_i2_with_nothing_counted_the_strip_and_the_send_page_agree() {
    let (mut m, s, mut banks, ctx, mut acct) = rig();
    old_base(&mut m);
    super::bank_links_fix12_tests::read_lacking(&mut m);
    super::bank_links_fix12_tests::not_kept(&s, 3);
    banks[0].questions[0].difficulty = Some(Difficulty::Low);
    banks[0].modified += 5;
    let strip = frames(&mut m, &mut banks, &ctx, &mut acct, &olga_named(&s), 3);
    let caveat = "This copy has no record of the difficulties in version 3, so a change to a difficulty can't be counted here.";
    assert!(strip.contains("Version 3 is the latest. This copy has no record") && strip.contains("Send my changes"), "{strip}");
    assert!(!strip.contains("up to date") && !strip.contains("unsent change"), "{strip}");
    let page = send_page(&mut m, &s, &mut banks, &ctx, &mut acct);
    assert!(page.contains(caveat) && !page.contains("Nothing in your copy differs"), "{page}");
}

/// Behind, and not countable: "You have version 3" stays on the strip, and Get latest comes first.
#[test]
fn c88_i2_behind_and_not_countable_keeps_you_have_version_and_get_latest_first() {
    let (mut m, s, mut banks, ctx, mut acct) = rig();
    old_base(&mut m);
    super::bank_links_fix12_tests::read_lacking(&mut m);
    super::bank_links_fix12_tests::not_kept(&s, 3);
    s.keep("Post", "/circles/heads", 200, heads(4));
    m.edit.update_link(LOCAL, |l| {
        l.head_seen = 4;
        true
    });
    add_question(&mut banks[0]);
    banks[0].modified += 5;
    let strip = frames(&mut m, &mut banks, &ctx, &mut acct, &olga_named(&s), 3);
    assert!(strip.contains("Version 4 is the latest. You have version 3 and 1 unsent change.") && strip.contains("Get the latest first, then send."), "{strip}");
    let (get, send) = (strip.find("Get latest").expect(&strip), strip.find("Send my changes").expect(&strip));
    assert!(get < send, "Get latest first: {strip}");
    assert_eq!(badge(&mut m, &banks[0].clone()), "newer version · 1 unsent");
    // a copy with no record at all of version 3: the same order, and words the Send page bears out
    m.edit.update_link(LOCAL, |l| {
        l.base.clear();
        true
    });
    let strip = frames(&mut m, &mut banks, &ctx, &mut acct, &olga_named(&s), 3);
    assert!(strip.contains("You have version 3.") && strip.contains("no record of what it looked like") && !strip.contains("Press Send to see"), "{strip}");
    let (get, send) = (strip.find("Get latest").expect(&strip), strip.find("Send my changes").expect(&strip));
    assert!(get < send, "Get latest first: {strip}");
}

// ---- round 12 survivors -----------------------------------------------------------------------------------------------------

/// (survivor e4) The unreadable send's added question is matched to the circle's (linked, not added twice): a colleague's new
/// question is then added by default as usual, never held back as "maybe yours".
#[test]
fn r12_e4_a_matched_added_question_does_not_hold_back_a_colleagues_new_one() {
    let ((m, _, _, _, _), page) = super::bank_links_fix12_tests::own_unreadable(&add_question, &[(QA, "One"), (QN, ADDED), (QC, "Bob's question")]);
    let rows = rows_of(&m);
    let bob = rows.iter().find(|r| r.title == "Bob's question").expect("listed");
    assert_eq!((bob.kind, bob.choice), (RowKind::NewInTheirs, Choice::Add), "{page}");
    assert!(m.edit.can_apply_merge(), "{page}");
}

/// (survivor e7) Her name on version 4 and one of the two questions the lost send carried as it carried them: not enough to
/// say version 4 is her send (every question it carried must be there as sent).
#[test]
fn r12_e7_a_version_holding_only_some_of_what_the_send_carried_is_not_hers() {
    let (mut m, s, mut banks) = two_at_three();
    s.keep("Get", &format!("/circles/{CID}"), 200, detail("owner", json!({})));
    let (bank, mut link) = linked(&[(QA, "One"), (QB, "Two")]);
    (link.base_version, link.base_version_id, link.head_seen) = (3, hex(3), 3);
    link.mark_bank(&bank);
    m.edit.update_link(LOCAL, |l| {
        link.role_seen = l.role_seen.clone();
        link.author_known = l.author_known;
        link.is_author = l.is_author;
        *l = link;
        true
    });
    banks[0] = bank;
    let ctx = ctx();
    let mut acct = FakeAccount { open: true, ..FakeAccount::default() };
    let who = olga_named(&s);
    frames(&mut m, &mut banks, &ctx, &mut acct, &who, 2);
    banks[0].questions[0].stem = "One (fix)".into();
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
    s.on("Post", "/circles/heads", 200, heads_by(4, "Olga Teacher"));
    frames(&mut m, &mut banks, &ctx, &mut acct, &who, 3);
    m.edit.close();
    let page = latest_as(&mut m, &s, &mut banks, &ctx, &mut acct, &who, merge_start_by(3, 4, "Olga Teacher"), served(4, &[(QA, "One (other)"), (QB, "Two")], false));
    assert!(!page.contains("is your own send"), "{page}");
    assert_eq!(count(&kinds(&m).unwrap(), RowKind::Conflict), 1, "{page}");
}

/// (c88 I2) Counted "but for a difficulty" only where the copy has every other record: a copy missing a linked question's
/// record altogether says it has no record (nothing counted), even when other questions lack only a difficulty.
#[test]
fn c88_i2_a_copy_missing_a_questions_record_is_not_counted_but_for_a_difficulty() {
    use super::circle_links::changes_since;
    let (mut bank, mut link) = linked(&[(QA, "One"), (QB, "Two")]);
    for b in &mut link.base {
        b.snap.difficulty = None;
    }
    let c = changes_since(&link, &bank);
    assert!(!c.known && c.partial(), "{c:?}");
    add_question(&mut bank);
    assert_eq!(changes_since(&link, &bank).total(), 1);
    link.base.retain(|b| b.qid == QA);
    let c = changes_since(&link, &bank);
    assert!(!c.known && !c.partial(), "{c:?}");
    // in either order of the questions
    bank.questions.reverse();
    let c = changes_since(&link, &bank);
    assert!(!c.known && !c.partial(), "{c:?}");
}

// ---- the quiet read of a base version (mutation survivors) -------------------------------------------------------------------

/// An answer for another version than the one asked for teaches nothing (the count stays not complete).
#[test]
fn c87_b1_an_answer_for_another_version_teaches_nothing() {
    let (mut m, s, mut banks, ctx, mut acct) = rig();
    old_base(&mut m);
    s.keep_raw("Get", &version_path(3), served_qs(6, vec![rated(QA, "One", Some("High"))], false));
    frames(&mut m, &mut banks, &ctx, &mut acct, &olga_named(&s), 3);
    assert_eq!(m.edit.with_link(LOCAL, |l| l.base[0].snap.difficulty).unwrap(), None);
    assert_eq!(version_reads(&s, 3), 1, "not asked again");
    // nor the same answer read for Get latest (the quiet read is not running: the account is being looked up)
    banks[0].questions[0].difficulty = Some(Difficulty::Low);
    banks[0].modified += 5;
    s.on("Post", &format!("{}/merge", put_path()), 200, merge_start(3, 4));
    s.keep_raw("Get", &put_path(), served_qs(4, vec![rated(QA, "One", None)], false));
    assert!(m.edit.open_merge(LOCAL, owner()));
    let checking = super::publish::AccountSnapshot::of_gate(super::publish::Gate::Checking);
    for _ in 0..3 {
        m.edit.frame(now(), &checking);
        m.edit.sync(&mut banks);
    }
    let (_, r) = row_at(&m, RowKind::Conflict);
    assert!(r.unrecorded, "{r:?}");
    assert_eq!(m.edit.with_link(LOCAL, |l| l.base[0].snap.difficulty).unwrap(), None);
}

/// A read that could not reach the circle (its own problem) is asked again a minute later, not given up; one this account
/// could not read is asked again for the next account.
#[test]
fn c87_b1_a_read_that_could_not_reach_the_circle_is_asked_again_later() {
    let (mut m, s, mut banks, ctx, mut acct) = rig();
    old_base(&mut m);
    s.on("Get", &version_path(3), 503, json!({"error": "server_error"}));
    s.keep_raw("Get", &version_path(3), served_qs(3, vec![rated(QA, "One", None)], false));
    let who = olga_named(&s);
    frames(&mut m, &mut banks, &ctx, &mut acct, &who, 3);
    assert_eq!(version_reads(&s, 3), 1);
    assert_eq!(m.edit.with_link(LOCAL, |l| l.base[0].snap.difficulty).unwrap(), None);
    let later = now() + std::time::Duration::from_secs(61);
    for _ in 0..2 {
        m.edit.frame(later, &who);
    }
    assert_eq!(version_reads(&s, 3), 2);
    assert_eq!(m.edit.with_link(LOCAL, |l| l.base[0].snap.difficulty).unwrap(), Some(None));
    // not readable to one account: asked again when another signs in
    let (mut m, s, mut banks, ctx, mut acct) = rig();
    old_base(&mut m);
    s.on("Get", &version_path(3), 404, json!({"error": "version_withheld"}));
    s.keep_raw("Get", &version_path(3), served_qs(3, vec![rated(QA, "One", None)], false));
    let who = olga_named(&s);
    frames(&mut m, &mut banks, &ctx, &mut acct, &who, 3);
    assert_eq!(version_reads(&s, 3), 1);
    m.edit.forget_account(true);
    frames(&mut m, &mut banks, &ctx, &mut acct, &who, 3);
    assert_eq!(m.edit.with_link(LOCAL, |l| l.base[0].snap.difficulty).unwrap(), Some(None));
}

/// A copy no longer in the circle, or whose bank was removed from it, is not read for.
#[test]
fn c87_b1_a_link_that_can_no_longer_reach_its_bank_is_not_read_for() {
    for gone in [true, false] {
        let (mut m, s, mut banks, ctx, mut acct) = rig();
        old_base(&mut m);
        m.edit.update_link(LOCAL, |l| {
            if gone {
                l.role_seen = Some(super::circle_links::ROLE_NONE.into());
            } else {
                l.seen.removed = true;
            }
            true
        });
        frames(&mut m, &mut banks, &ctx, &mut acct, &olga_named(&s), 3);
        assert_eq!(version_reads(&s, 3), 0, "gone {gone}");
    }
}

/// An answer that comes after the copy moved on to another version teaches that version's base nothing.
#[test]
fn c87_b1_a_late_answer_for_an_older_version_teaches_the_newer_base_nothing() {
    let (mut m, s, _banks, _ctx, _acct) = rig();
    old_base(&mut m);
    s.keep_raw("Get", &version_path(3), served_qs(3, vec![rated(QA, "One", Some("High"))], false));
    super::bank_links_fix12_tests::not_kept(&s, 4);
    let who = olga_named(&s);
    // the read is asked (and answered) in this frame; its answer is taken at the next one
    m.edit.frame(now(), &who);
    assert_eq!(version_reads(&s, 3), 1);
    m.edit.update_link(LOCAL, |l| {
        l.base_version = 4;
        true
    });
    m.edit.frame(now(), &who);
    assert_eq!(m.edit.with_link(LOCAL, |l| l.base[0].snap.difficulty).unwrap(), None);
}

/// (c88 I2) The circle's page says the same as the strip: what is counted, and that a change to a difficulty can't be.
#[test]
fn c88_i2_the_circle_page_line_says_the_same() {
    let (mut m, s, mut banks, ctx, mut acct) = rig();
    old_base(&mut m);
    super::bank_links_fix12_tests::not_kept(&s, 3);
    frames(&mut m, &mut banks, &ctx, &mut acct, &olga_named(&s), 2);
    let line = |m: &mut CirclesModel, b: &QuestionBank| {
        let st = m.edit.status_of(b);
        let l = m.edit.link_for_local(LOCAL).unwrap();
        super::circle_edit_ui::standing(&l, st.as_ref(), false, super::circles_api::Role::Owner).join(" ")
    };
    // (round 16, q93_4) and, as the strip says, that sending is off; (round 17, critic 96 minor 1) and where she can act
    assert_eq!(line(&mut m, &banks[0].clone()), format!("A change to a difficulty can't be counted for this copy. {}", super::circles_text::NOT_KEPT_ROW));
    add_question(&mut banks[0]);
    banks[0].modified += 5;
    assert_eq!(line(&mut m, &banks[0].clone()), format!("Not sent yet: 1 added, 0 changed, 0 removed. A change to a difficulty can't be counted for this copy. {}", super::circles_text::NOT_KEPT_ROW));
}

/// (c87 m1) A difficulty and a picture both differ: the two are different questions, so "Keep both" is offered.
#[test]
fn c87_m1_keep_both_is_offered_where_a_picture_differs_too() {
    use super::circle_links::circle_theirs;
    use super::merge::build_plan;
    let (mut bank, link) = linked(&[(QA, "One")]);
    bank.questions[0].image = Some(crate::model::ImageRef { file: "media/abc.webp".into(), width: 2, height: 2 });
    bank.questions[0].difficulty = Some(Difficulty::Low);
    let mut t = circle_theirs(&payload(&[(QA, "One")]), NOW);
    t[0].q.difficulty = Some(Difficulty::High);
    let r = build_plan(BID, &link.base, &bank, &t, 3, 4, None, String::new()).rows[0].clone();
    assert!(r.kind == RowKind::Conflict && r.options.contains(&Choice::Both), "{r:?}");
}
