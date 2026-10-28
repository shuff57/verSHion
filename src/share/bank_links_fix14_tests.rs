//! Phase D1, round 14 (critics 89 and 90, and the owner's decision of 2026-10-26): each test written first against the code
//! it pins. A link an earlier build wrote learns its difficulties once from the circle's own version at its base version, and
//! the copy inherits a rating the circle has there that it lacks, unless the teacher edited the question since (a later
//! deliberate change of hers supersedes); a colleague's change hidden behind an unrecorded difficulty is said as his; the
//! quiet read of a base version is one at a time, never beside the merge's own, never for a viewer's copy, and asked again
//! only when that can help.

use std::sync::Arc;
use std::time::Duration;

use serde_json::{Value, json};

use super::bank_links_fix10_tests::*;
use super::bank_links_fix11_tests::{host_a, rated, rig, version_path, version_reads};
use super::bank_links_fix4_tests::*;
use super::bank_links_fix8_tests::*;
use super::bank_links_fix9_tests::*;
use super::bank_links_fix_tests::*;
use super::bank_links_tests::*;
use super::circle_edit::*;
use super::circle_links::{CircleLink, LinkMode};
use super::circles::CirclesModel;
use super::circles_tests::*;
use super::merge::{Choice, RowKind};
use super::publish::{AccountSnapshot, Gate};
use crate::model::{Difficulty, QuestionBank};

/// A's link and bank as the phase C build left them after a Get latest to `version`: questions One and Two, a base that kept
/// no difficulty, the bank's One rated `one` and Two rated `two`, and the bank marked with the link (its print matches).
fn phase_c(m: &mut CirclesModel, banks: &mut [QuestionBank], version: u64, one: Option<Difficulty>, two: Option<Difficulty>) {
    let (mut bank, mut link) = linked(&[(QA, "One"), (QB, "Two")]);
    bank.questions[0].difficulty = one;
    bank.questions[1].difficulty = two;
    for b in &mut link.base {
        b.snap.difficulty = None;
    }
    link.base_version = version;
    link.base_version_id = hex(version);
    link.head_seen = version;
    link.mark_bank(&bank);
    // (a file an earlier build wrote says nothing of a read)
    link.difficulty_read = None;
    m.edit.update_link(LOCAL, |l| {
        link.role_seen = l.role_seen.clone();
        link.author_known = l.author_known;
        link.is_author = l.is_author;
        *l = link;
        true
    });
    banks[0] = bank;
}

fn st(m: &mut CirclesModel, b: &QuestionBank) -> super::circle_links::ChangeCount {
    m.edit.status_of(b).unwrap().changes
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

/// Frames of the model alone (no strip, so no check of the circle is asked) as `who` at `at`.
fn quiet(m: &mut CirclesModel, banks: &mut [QuestionBank], who: &AccountSnapshot, at: std::time::Instant, n: usize) {
    for _ in 0..n {
        m.edit.frame(at, who);
        m.edit.sync(banks);
    }
}

/// Frames of the model alone until `done` (jobs on their own threads), at most two seconds.
fn quiet_until(m: &mut CirclesModel, banks: &mut [QuestionBank], who: &AccountSnapshot, at: std::time::Instant, done: &dyn Fn(&CirclesModel) -> bool) {
    for _ in 0..200 {
        quiet(m, banks, who, at, 1);
        if done(m) {
            return;
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    panic!("timed out: {:?}", m.edit.view);
}

fn all_version_reads(s: &Arc<Script>) -> usize {
    s.paths().iter().filter(|p| p.starts_with("Get ") && p.contains("/versions/")).count()
}

fn one_of(m: &mut CirclesModel) -> Option<Option<Difficulty>> {
    m.edit.with_link(LOCAL, |l| l.base[0].snap.difficulty).unwrap()
}

// ---- the owner's decision, critic 90 I1: a rating the circle has and the copy lacks is the circle's ----------------------------

/// Bob rated One "High" in version 3 with a change elsewhere; the phase C build's Get latest took the other change and left One
/// at none, and marked the bank. This build reads version 3: the copy inherits his rating, nothing is counted as hers, the
/// strip and the badge say so, and her next send carries his rating (never none over it).
#[test]
fn c90_i1_a_rating_the_phase_c_merge_dropped_is_inherited_never_counted_as_hers() {
    let (mut m, s, mut banks, ctx, mut acct) = rig();
    s.keep("Post", "/circles/heads", 200, heads(3));
    phase_c(&mut m, &mut banks, 3, None, None);
    s.keep_raw("Get", &version_path(3), served_qs(3, vec![rated(QA, "One", Some("High")), rated(QB, "Two", None)], false));
    let who = olga_named(&s);
    let strip = frames(&mut m, &mut banks, &ctx, &mut acct, &who, 3);
    assert_eq!(banks[0].questions[0].difficulty, Some(Difficulty::High), "inherited: {strip}");
    assert_eq!(changes(&mut m, &banks[0].clone()), (0, 0, 0), "{strip}");
    assert_eq!(m.edit.with_link(LOCAL, |l| l.inherit), Some(false), "done: nothing left to inherit");
    assert!(!strip.contains("unsent change") && strip.contains("up to date"), "{strip}");
    // (round 17, critic 96 I1) the news of it, until dismissed
    assert_eq!(badge(&mut m, &banks[0].clone()), "difficulty updated");
    // (not an edit of hers: the question's own time is kept)
    assert_eq!(banks[0].questions[0].modified, NOW);
    // the next send carries his rating
    let link = m.edit.link_for_local(LOCAL).unwrap();
    let sent: Value = serde_json::from_slice(&super::circle_links::payload_of(&link, &banks[0]).unwrap()).unwrap();
    assert_eq!(sent["bank"]["questions"][0]["difficulty"], json!("High"));
    // once: a later frame inherits nothing again, and Bob's version 4 (a new question) is plain to take
    frames(&mut m, &mut banks, &ctx, &mut acct, &who, 2);
    assert_eq!(version_reads(&s, 3), 1);
    let mut start = merge_start_by(3, 4, "Bob Teacher");
    start["changed"] = json!(1);
    let page = latest_as(&mut m, &s, &mut banks, &ctx, &mut acct, &who, start, served_qs(4, vec![rated(QA, "One", Some("High")), rated(QB, "Two", None), rated(QC, "Three", None)], false));
    assert_eq!(kinds(&m).unwrap(), vec![RowKind::Unchanged, RowKind::Unchanged, RowKind::NewInTheirs], "{page}");
    assert!(!page.contains("Changed by you"), "{page}");
    apply(&mut m, &s, &mut banks, &ctx, &mut acct);
    assert_eq!(banks[0].questions[0].difficulty, Some(Difficulty::High));
    assert_eq!(changes(&mut m, &banks[0].clone()), (0, 0, 0));
}

/// The pass that inherits says the banks changed (so they are saved), once.
#[test]
fn c90_i1_the_bank_inherited_into_is_saved() {
    let (mut m, s, mut banks, _ctx, _acct) = rig();
    phase_c(&mut m, &mut banks, 3, None, None);
    s.keep_raw("Get", &version_path(3), served_qs(3, vec![rated(QA, "One", Some("High")), rated(QB, "Two", None)], false));
    let who = olga_named(&s);
    m.edit.frame(now(), &who);
    assert!(!m.edit.sync(&mut banks), "the read is taken at the next frame");
    m.edit.frame(now(), &who);
    assert!(m.edit.sync(&mut banks), "inherited: the banks are to be saved");
    assert_eq!(banks[0].questions[0].difficulty, Some(Difficulty::High));
    m.edit.frame(now(), &who);
    assert!(!m.edit.sync(&mut banks));
}

/// The rating learnt but not yet in the bank when the app closed (the banks were not saved): after a restart the copy still
/// inherits it, and a count worked out before that (the Banks list's badge) is worked out again.
#[test]
fn c90_i1_an_inheritance_not_saved_is_done_after_a_restart() {
    let log = Arc::new(std::sync::Mutex::new(Vec::new()));
    let (mut m, s, mut banks, _ctx, _acct) = rig();
    phase_c(&mut m, &mut banks, 3, None, None);
    s.keep_raw("Get", &version_path(3), served_qs(3, vec![rated(QA, "One", Some("High")), rated(QB, "Two", None)], false));
    let who = olga_named(&s);
    m.edit.frame(now(), &who);
    m.edit.frame(now(), &who);
    assert_eq!(one_of(&mut m), Some(Some(Difficulty::High)), "learnt");
    // (the app closes before a pass gives the bank its rating)
    *log.lock().unwrap() = m.edit.links().to_vec();
    let mut m = restarted(log);
    assert_eq!(st(&mut m, &banks[0].clone()).changed, 1, "before the pass: counted from the bank as it is");
    assert!(m.edit.sync(&mut banks));
    assert_eq!(banks[0].questions[0].difficulty, Some(Difficulty::High));
    assert_eq!(st(&mut m, &banks[0].clone()).total(), 0);
    assert_eq!(version_reads(&s, 3), 1);
}

/// The bank exactly as the app last wrote it inherits, whatever its questions' own times say (a question's time from another
/// computer's clock may be later than the bank's).
#[test]
fn c90_i1_a_bank_as_the_app_wrote_it_inherits_whatever_its_question_times() {
    let (mut m, s, mut banks, ctx, mut acct) = rig();
    s.keep("Post", "/circles/heads", 200, heads(3));
    phase_c(&mut m, &mut banks, 3, None, None);
    banks[0].questions[0].modified = NOW / 1000 + 3600;
    let b = banks[0].clone();
    m.edit.update_link(LOCAL, |l| {
        l.mark_bank(&b);
        true
    });
    s.keep_raw("Get", &version_path(3), served_qs(3, vec![rated(QA, "One", Some("High")), rated(QB, "Two", None)], false));
    let strip = frames(&mut m, &mut banks, &ctx, &mut acct, &olga_named(&s), 3);
    assert_eq!(banks[0].questions[0].difficulty, Some(Difficulty::High), "{strip}");
}

/// She edited another question since the copy last matched (the bank's print differs): One, not edited (its own time, in
/// milliseconds as a copy from the circle has it, is older), still inherits.
#[test]
fn c90_i1_an_edit_elsewhere_does_not_stop_the_inheritance() {
    let (mut m, s, mut banks, ctx, mut acct) = rig();
    s.keep("Post", "/circles/heads", 200, heads(3));
    phase_c(&mut m, &mut banks, 3, None, None);
    banks[0].questions[1].stem = "Two (mine)".into();
    banks[0].questions[1].modified = NOW / 1000 + 60;
    banks[0].modified = NOW / 1000 + 60;
    s.keep_raw("Get", &version_path(3), served_qs(3, vec![rated(QA, "One", Some("High")), rated(QB, "Two", None)], false));
    let strip = frames(&mut m, &mut banks, &ctx, &mut acct, &olga_named(&s), 3);
    assert_eq!(banks[0].questions[0].difficulty, Some(Difficulty::High), "{strip}");
    assert_eq!(changes(&mut m, &banks[0].clone()), (0, 1, 0), "only her edit of Two: {strip}");
}

/// (round 15, the owner's rule, accepted) She cleared One's rating after the copy last matched, before the read: the read
/// fills it again with the circle's. Only a change made after the read is hers.
#[test]
fn c90_i1_a_rating_cleared_before_the_read_is_filled_again() {
    let (mut m, s, mut banks, ctx, mut acct) = rig();
    s.keep("Post", "/circles/heads", 200, heads(3));
    phase_c(&mut m, &mut banks, 3, None, None);
    banks[0].questions[0].modified = NOW / 1000 + 60;
    banks[0].modified = NOW / 1000 + 60;
    s.keep_raw("Get", &version_path(3), served_qs(3, vec![rated(QA, "One", Some("High")), rated(QB, "Two", None)], false));
    let strip = frames(&mut m, &mut banks, &ctx, &mut acct, &olga_named(&s), 3);
    assert_eq!(banks[0].questions[0].difficulty, Some(Difficulty::High), "{strip}");
    assert_eq!(changes(&mut m, &banks[0].clone()), (0, 0, 0), "{strip}");
}

/// (round 15, the owner's rule; critic 91 I3) A rating the copy has is replaced by the circle's too: one the circle's version
/// 3 does not have, and one that differs from the circle's.
#[test]
fn c90_i1_a_rating_the_copy_has_is_replaced_by_the_circles() {
    let (mut m, s, mut banks, ctx, mut acct) = rig();
    s.keep("Post", "/circles/heads", 200, heads(3));
    phase_c(&mut m, &mut banks, 3, Some(Difficulty::Low), Some(Difficulty::High));
    s.keep_raw("Get", &version_path(3), served_qs(3, vec![rated(QA, "One", Some("High")), rated(QB, "Two", None)], false));
    let strip = frames(&mut m, &mut banks, &ctx, &mut acct, &olga_named(&s), 3);
    assert_eq!((banks[0].questions[0].difficulty, banks[0].questions[1].difficulty), (Some(Difficulty::High), None), "{strip}");
    assert_eq!(changes(&mut m, &banks[0].clone()), (0, 0, 0), "{strip}");
}

/// A bank put back from a saved copy (older than the link, and different): what it was last in step with is not known, so
/// nothing is inherited into it.
#[test]
fn c90_i1_nothing_is_inherited_into_a_bank_put_back() {
    let (mut m, s, mut banks, ctx, mut acct) = rig();
    s.keep("Post", "/circles/heads", 200, heads(3));
    phase_c(&mut m, &mut banks, 3, None, None);
    banks[0].questions[1].stem = "Two (old)".into();
    banks[0].modified = NOW / 1000 - 100;
    s.keep_raw("Get", &version_path(3), served_qs(3, vec![rated(QA, "One", Some("High")), rated(QB, "Two", None)], false));
    frames(&mut m, &mut banks, &ctx, &mut acct, &olga_named(&s), 3);
    assert_eq!(banks[0].questions[0].difficulty, None);
}

/// Get latest reads version 3 itself (the quiet read has not happened: the account was being looked up): the review is made
/// with his rating inherited, never "Changed by you".
#[test]
fn c90_i1_get_latest_inherits_from_the_version_it_reads() {
    let (mut m, s, mut banks, _ctx, _acct) = rig();
    phase_c(&mut m, &mut banks, 3, None, None);
    s.keep_raw("Get", &version_path(3), served_qs(3, vec![rated(QA, "One", Some("High")), rated(QB, "Two", None)], false));
    s.on("Post", &format!("{}/merge", put_path()), 200, merge_start(3, 4));
    s.keep_raw("Get", &put_path(), served_qs(4, vec![rated(QA, "One", Some("High")), rated(QB, "Two (b)", None)], false));
    assert!(m.edit.open_merge(LOCAL, owner()));
    let checking = AccountSnapshot::of_gate(Gate::Checking);
    quiet(&mut m, &mut banks, &checking, now(), 3);
    assert_eq!(kinds(&m).unwrap(), vec![RowKind::Unchanged, RowKind::TheirsChanged]);
    assert_eq!(banks[0].questions[0].difficulty, Some(Difficulty::High));
    assert_eq!(version_reads(&s, 3), 1);
}

// ---- critic 90 I2: a colleague's change beside a difficulty this computer has no record of -----------------------------------

/// Version 3 no longer kept, the teacher changed nothing, Bob reworded and rated One in version 4: "Changed in the circle",
/// counted so, nothing chosen, and the note says what Keep mine gives up. Taking it takes his wording and his rating.
#[test]
fn c90_i2_a_colleagues_rewording_is_changed_in_the_circle_and_undecided() {
    let (mut m, s, mut banks, ctx, mut acct) = rig();
    phase_c(&mut m, &mut banks, 3, None, None);
    super::bank_links_fix12_tests::not_kept(&s, 3);
    let page = get_latest(&mut m, &s, &mut banks, &ctx, &mut acct, served_qs(4, vec![rated(QA, "One (b)", Some("High")), rated(QB, "Two", None)], false));
    let rows = rows_of(&m);
    let r = &rows[0];
    assert_eq!((r.kind, r.choice, r.unrecorded), (RowKind::TheirsChanged, Choice::Undecided, false), "{page}");
    assert_eq!(r.options, vec![Choice::Keep, Choice::Take]);
    assert!(r.note.as_deref().is_some_and(|n| n.contains("not known whose difficulty is newer") && n.contains("then not kept")), "{r:?}");
    assert!(page.contains("1 changed in the circle") && !page.contains("Difficulty not recorded") && !page.contains("difficulty not recorded"), "{page}");
    assert!(page.contains("Choose one to continue") && page.contains("1 question still needs a decision"), "{page}");
    assert!(page.contains("Take the circle's version") && page.contains("Keep mine"), "{page}");
    assert!(!m.edit.can_apply_merge(), "{page}");
    m.edit.set_merge_choice(0, Choice::Take);
    assert!(m.edit.can_apply_merge());
    apply(&mut m, &s, &mut banks, &ctx, &mut acct);
    assert_eq!((banks[0].questions[0].stem.as_str(), banks[0].questions[0].difficulty), ("One (b)", Some(Difficulty::High)));
    assert_eq!(changes(&mut m, &banks[0].clone()), (0, 0, 0));
}

/// The same row, Keep mine: her wording and her (none) rating stay, and are hers to send.
#[test]
fn c90_i2_keep_mine_keeps_hers() {
    let (mut m, s, mut banks, ctx, mut acct) = rig();
    phase_c(&mut m, &mut banks, 3, None, None);
    super::bank_links_fix12_tests::not_kept(&s, 3);
    get_latest(&mut m, &s, &mut banks, &ctx, &mut acct, served_qs(4, vec![rated(QA, "One (b)", Some("High")), rated(QB, "Two", None)], false));
    m.edit.set_merge_choice(0, Choice::Keep);
    apply(&mut m, &s, &mut banks, &ctx, &mut acct);
    assert_eq!((banks[0].questions[0].stem.as_str(), banks[0].questions[0].difficulty), ("One", None));
    assert_eq!(changes(&mut m, &banks[0].clone()), (0, 1, 0));
}

/// Only the difficulty differs (nobody's other change known): the "Difficulty not recorded" row as before; and where she
/// changed something else too, the same (her change is shown, nothing chosen).
#[test]
fn c90_i2_a_difference_only_in_difficulty_keeps_the_not_recorded_row() {
    for mine in [false, true] {
        let (mut m, s, mut banks, ctx, mut acct) = rig();
        phase_c(&mut m, &mut banks, 3, None, None);
        if mine {
            banks[0].questions[0].stem = "One (a)".into();
            banks[0].modified += 5;
        }
        super::bank_links_fix12_tests::not_kept(&s, 3);
        let page = get_latest(&mut m, &s, &mut banks, &ctx, &mut acct, served_qs(4, vec![rated(QA, "One", Some("High")), rated(QB, "Two", None)], false));
        let r = rows_of(&m)[0].clone();
        assert!(r.unrecorded && r.kind == RowKind::Conflict && r.choice == Choice::Undecided, "{mine}: {page}");
        assert!(page.contains("Difficulty not recorded"), "{page}");
    }
}

// ---- critic 89 I1: a link an earlier build filled from the bank is checked once against the circle --------------------------

/// The round 12 build filled the base from the bank (her unsent "High" on One taken as the circle's). With no record of a
/// read, this build reads version 4 once and learns from it over what is there: her rating is counted as hers, and Bob's
/// version 5 (One untouched) never takes it away.
#[test]
fn c89_i1_a_base_filled_from_the_bank_is_learnt_again_from_the_circle() {
    let (mut m, s, mut banks, ctx, mut acct) = rig();
    s.keep("Post", "/circles/heads", 200, heads(4));
    phase_c(&mut m, &mut banks, 4, Some(Difficulty::High), None);
    let b = banks[0].clone();
    m.edit.update_link(LOCAL, |l| {
        for (bq, q) in l.base.iter_mut().zip(&b.questions) {
            bq.snap.difficulty = Some(q.difficulty);
        }
        true
    });
    s.keep_raw("Get", &version_path(4), served_qs(4, vec![rated(QA, "One", None), rated(QB, "Two", None)], false));
    let who = olga_named(&s);
    let strip = frames(&mut m, &mut banks, &ctx, &mut acct, &who, 3);
    assert_eq!(version_reads(&s, 4), 1, "{strip}");
    assert_eq!(one_of(&mut m), Some(None));
    // (round 15, the owner's rule) the copy then has the circle's difficulty
    assert_eq!(banks[0].questions[0].difficulty, None, "{strip}");
    assert_eq!(changes(&mut m, &banks[0].clone()), (0, 0, 0), "{strip}");
    assert!(strip.contains("up to date"), "{strip}");
    let page = latest_as(&mut m, &s, &mut banks, &ctx, &mut acct, &who, merge_start_by(4, 5, "Bob Teacher"), served_qs(5, vec![rated(QA, "One", None), rated(QB, "Two", None)], false));
    assert_eq!(kinds(&m).unwrap(), vec![RowKind::Unchanged, RowKind::Unchanged], "{page}");
    apply(&mut m, &s, &mut banks, &ctx, &mut acct);
    assert_eq!(banks[0].questions[0].difficulty, None);
    assert_eq!(version_reads(&s, 4), 1, "read once");
    // the merge wrote the base as the circle's version 5 is: nothing to read for it
    frames(&mut m, &mut banks, &ctx, &mut acct, &who, 3);
    assert_eq!(all_version_reads(&s), 1, "{:?}", s.paths());
}

/// A link this build wrote (its difficulties known, a read or not needed) is never read for; one loaded from a file an
/// earlier build wrote (no record of a read) is, once; the record is kept in the file.
#[test]
fn c89_i1_the_record_of_a_read_is_kept_and_an_old_file_has_none() {
    let (mut m, s, mut banks, _ctx, _acct) = rig();
    s.keep_raw("Get", &version_path(3), served_qs(3, vec![rated(QA, "One", None), rated(QB, "Two", None)], false));
    let who = olga_named(&s);
    quiet(&mut m, &mut banks, &who, now(), 3);
    assert_eq!(all_version_reads(&s), 0, "this build's own link");
    let link = m.edit.link_for_local(LOCAL).unwrap();
    let mut v = serde_json::to_value(&link).unwrap();
    v.as_object_mut().unwrap().remove("difficulty_read");
    let old: CircleLink = serde_json::from_value(v).unwrap();
    assert_eq!(old.difficulty_read, None);
    let back: CircleLink = serde_json::from_value(serde_json::to_value(&link).unwrap()).unwrap();
    assert_eq!(back.difficulty_read, link.difficulty_read);
    assert!(back.difficulty_read.is_some());
    m.edit.update_link(LOCAL, |l| {
        *l = old.clone();
        true
    });
    quiet(&mut m, &mut banks, &who, now(), 3);
    assert_eq!(version_reads(&s, 3), 1);
    assert_eq!(m.edit.with_link(LOCAL, |l| l.difficulty_read).unwrap(), Some(3));
}

/// What is read for: a base at a version, with questions, not read at that version yet (or never, for an earlier build's link);
/// a link this build wrote keeps what it knows where the version lacks a question.
#[test]
fn c89_i1_what_is_read_for_and_what_a_read_keeps() {
    use super::circle_links::{circle_theirs, learn_difficulty, needs_difficulty_read};
    let (_, link) = linked(&[(QA, "One"), (QB, "Two")]);
    let mut l = link.clone();
    l.difficulty_read = None;
    assert!(needs_difficulty_read(&l));
    l.base_version = 0;
    assert!(!needs_difficulty_read(&l), "no version to read");
    let mut l = link.clone();
    l.difficulty_read = None;
    l.base.clear();
    assert!(!needs_difficulty_read(&l), "nothing to learn for");
    // this build's link, one difficulty not known: read; QB's known rating is kept though the version lacks QB
    let mut l = link.clone();
    l.base[0].snap.difficulty = None;
    l.base[1].snap.difficulty = Some(Some(Difficulty::High));
    assert!(needs_difficulty_read(&l));
    let t = circle_theirs(&body_of(vec![rated(QA, "One", Some("Low"))]), NOW);
    assert!(learn_difficulty(&mut l, &t));
    assert_eq!(l.base.iter().map(|b| b.snap.difficulty).collect::<Vec<_>>(), vec![Some(Some(Difficulty::Low)), Some(Some(Difficulty::High))]);
    assert!(!needs_difficulty_read(&l));
    assert!(!learn_difficulty(&mut l, &t), "nothing new");
    // an earlier build's link (the round 12 build filled QB from the bank): what the version lacks is not known any more
    let mut l = link.clone();
    l.difficulty_read = None;
    l.base[1].snap.difficulty = Some(Some(Difficulty::High));
    assert!(learn_difficulty(&mut l, &t));
    assert_eq!(l.base.iter().map(|b| b.snap.difficulty).collect::<Vec<_>>(), vec![Some(Some(Difficulty::Low)), None]);
}

/// (critic 89 P6) The links the phase C build wrote load, learn from the version by the circle's id, and a base entry the
/// version lacks stays not known (written back so).
#[test]
fn c89_p6_a_phase_c_file_loads_and_learns() {
    use super::circle_links::{changes_since, circle_theirs, difficulty_unknown, learn_difficulty};
    let mut l: CircleLink = serde_json::from_str(HEAD_LINK).unwrap();
    assert!(difficulty_unknown(&l) && l.difficulty_read.is_none());
    let qa = l.base[0].qid.clone();
    let t = circle_theirs(&body_of(vec![rated(&qa, "One", Some("High"))]), NOW);
    assert!(learn_difficulty(&mut l, &t));
    assert_eq!(l.base[0].snap.difficulty, Some(Some(Difficulty::High)));
    assert!(difficulty_unknown(&l), "the second entry is not in the version: still unknown");
    assert_eq!(l.difficulty_read, Some(4));
    let back: CircleLink = serde_json::from_str(&serde_json::to_string(&l).unwrap()).unwrap();
    assert_eq!(back.base[0].snap.difficulty, Some(Some(Difficulty::High)));
    assert_eq!(back.base[1].snap.difficulty, None, "written back as not known");
    let bank = QuestionBank { id: l.local_bank_id.clone(), name: "x".into(), questions: vec![], created: 0, modified: 0, details: Default::default() };
    assert_eq!(changes_since(&l, &bank).removed, 2);
}

// ---- critic 89 m1: a partly learnt link's count -------------------------------------------------------------------------------

/// (P1) Version 4 holds One but Two under another id: One's difficulty is learnt, Two's not. (round 15) One takes the
/// circle's (none) at once, and the count is worked out again after the read: nothing counted, Two's difficulty not known.
#[test]
fn c89_m1_a_partly_learnt_link_counts_what_was_learnt() {
    let (mut m, s, mut banks, ctx, mut acct) = rig();
    s.keep("Post", "/circles/heads", 200, heads(4));
    phase_c(&mut m, &mut banks, 4, Some(Difficulty::High), None);
    s.keep_raw("Get", &version_path(4), served_qs(4, vec![rated(QA, "One", None), rated(QC, "Two", None)], false));
    let who = olga_named(&s);
    frames(&mut m, &mut banks, &ctx, &mut acct, &who, 4);
    let c = st(&mut m, &banks[0].clone());
    assert_eq!(banks[0].questions[0].difficulty, None);
    assert_eq!(c.changed, 0, "{c:?}");
    assert!(c.partial(), "{c:?}");
}

/// (P2) The quiet read finished before the teacher rated anything; Bob rated Two in version 5: his to take, nothing hers.
#[test]
fn c89_p2_a_learnt_base_then_a_colleagues_rating() {
    let (mut m, s, mut banks, ctx, mut acct) = rig();
    s.keep("Post", "/circles/heads", 200, heads(4));
    phase_c(&mut m, &mut banks, 4, None, None);
    s.keep_raw("Get", &version_path(4), served_qs(4, vec![rated(QA, "One", None), rated(QB, "Two", None)], false));
    let who = olga_named(&s);
    frames(&mut m, &mut banks, &ctx, &mut acct, &who, 3);
    assert_eq!(one_of(&mut m), Some(None));
    let page = latest_as(&mut m, &s, &mut banks, &ctx, &mut acct, &who, merge_start_by(4, 5, "Bob Teacher"), served_qs(5, vec![rated(QA, "One", None), rated(QB, "Two", Some("High"))], false));
    assert_eq!(kinds(&m).unwrap(), vec![RowKind::Unchanged, RowKind::TheirsChanged], "{page}");
    apply(&mut m, &s, &mut banks, &ctx, &mut acct);
    assert_eq!(banks[0].questions[1].difficulty, Some(Difficulty::High));
    assert_eq!(changes(&mut m, &banks[0].clone()), (0, 0, 0));
}

/// (P5) Keep mine on a "Difficulty not recorded" row keeps her rating, counted as hers to send.
#[test]
fn c89_p5_keep_mine_on_a_not_recorded_row_is_counted() {
    let (mut m, s, mut banks, ctx, mut acct) = rig();
    s.keep("Post", "/circles/heads", 200, heads(4));
    phase_c(&mut m, &mut banks, 4, Some(Difficulty::High), None);
    super::bank_links_fix12_tests::not_kept(&s, 4);
    let who = olga_named(&s);
    frames(&mut m, &mut banks, &ctx, &mut acct, &who, 2);
    let page = latest_as(&mut m, &s, &mut banks, &ctx, &mut acct, &who, merge_start_by(4, 5, "Bob Teacher"), served_qs(5, vec![rated(QA, "One", Some("Low")), rated(QB, "Two", None)], false));
    let i = rows_of(&m).iter().position(|r| r.unrecorded).expect(&page);
    m.edit.set_merge_choice(i, Choice::Ours);
    apply(&mut m, &s, &mut banks, &ctx, &mut acct);
    assert_eq!(banks[0].questions[0].difficulty, Some(Difficulty::High));
    assert_eq!(changes(&mut m, &banks[0].clone()), (0, 1, 0));
    // the merge wrote the base as version 5 is: not a link to learn for any more
    frames(&mut m, &mut banks, &ctx, &mut acct, &who, 3);
    assert_eq!(version_reads(&s, 5), 0, "{:?}", s.paths());
}

/// A send writes the base as the bank was sent: an earlier build's link is not one to learn for any more.
#[test]
fn c89_i1_a_send_ends_the_need_to_learn() {
    use super::circle_links::{after_send, needs_difficulty_read};
    let (bank, mut link) = linked(&[(QA, "One")]);
    link.difficulty_read = None;
    assert!(needs_difficulty_read(&link));
    let sent = super::circles_api::SentBank { version: 2, version_id: hex(2), qids: vec![QA.into()], ..Default::default() };
    let l = after_send(&link, &bank, &sent);
    assert_eq!((l.difficulty_read, l.inherit), (Some(0), false));
    assert!(!needs_difficulty_read(&l));
}

// ---- critic 89 m2: one read of the base version for Get latest ------------------------------------------------------------------

/// (P3) Get latest opened before the quiet read started: the merge's own read is the only one.
#[test]
fn c89_m2_get_latest_reads_the_base_version_once() {
    let (mut m, s, mut banks, ctx, mut acct) = rig();
    s.keep("Post", "/circles/heads", 200, heads(4));
    phase_c(&mut m, &mut banks, 4, Some(Difficulty::High), None);
    s.keep_raw("Get", &version_path(4), served_qs(4, vec![rated(QA, "One", None), rated(QB, "Two", None)], false));
    let who = olga_named(&s);
    let page = latest_as(&mut m, &s, &mut banks, &ctx, &mut acct, &who, merge_start_by(4, 5, "Bob Teacher"), served_qs(5, vec![rated(QA, "One", None), rated(QB, "Two", None)], false));
    frames(&mut m, &mut banks, &ctx, &mut acct, &who, 3);
    assert_eq!(version_reads(&s, 4), 1, "{page}");
    assert_eq!(kinds(&m).unwrap(), vec![RowKind::Unchanged, RowKind::Unchanged], "{page}");
}

/// The quiet read is on its way (held at the server) when Get latest is pressed: the merge waits for it and reads the base
/// version no second time, and nothing else starts it again.
#[test]
fn c89_m2_get_latest_uses_the_quiet_read_on_its_way() {
    let (mut m, s, mut banks, _ctx, _acct) = rig();
    phase_c(&mut m, &mut banks, 4, Some(Difficulty::High), None);
    background(&mut m);
    s.keep_raw("Get", &version_path(4), served_qs(4, vec![rated(QA, "One", None), rated(QB, "Two", None)], false));
    let who = olga_named(&s);
    let release = s.hold();
    quiet(&mut m, &mut banks, &who, now(), 1);
    for _ in 0..100 {
        if version_reads(&s, 4) == 1 {
            break;
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    assert_eq!(version_reads(&s, 4), 1);
    s.on("Post", &format!("{}/merge", put_path()), 200, merge_start_by(4, 5, "Bob Teacher"));
    s.keep_raw("Get", &put_path(), served_qs(5, vec![rated(QA, "One", None), rated(QB, "Two", None)], false));
    assert!(m.edit.open_merge(LOCAL, owner()));
    // the merge's own reads are answered; the review waits for the base version's
    for _ in 0..30 {
        quiet(&mut m, &mut banks, &who, now(), 1);
        std::thread::sleep(Duration::from_millis(10));
    }
    assert!(kinds(&m).is_none(), "waits for the read on its way: {:?}", s.paths());
    assert_eq!(version_reads(&s, 4), 1, "{:?}", s.paths());
    release.send(()).unwrap();
    quiet_until(&mut m, &mut banks, &who, now(), &|m| kinds(m).is_some());
    assert_eq!(version_reads(&s, 4), 1, "{:?}", s.paths());
    assert_eq!(kinds(&m).unwrap(), vec![RowKind::Unchanged, RowKind::Unchanged]);
}

/// A screen opened or closed while the quiet read is on its way keeps it: the base version is not asked a second time. Another
/// account signing in does not take the earlier account's read for its own: it asks again.
#[test]
fn c89_m2_a_quiet_read_on_its_way_outlives_the_screens() {
    for how in ["close", "send", "history", "account"] {
        let (mut m, s, mut banks, _ctx, _acct) = rig();
        phase_c(&mut m, &mut banks, 4, Some(Difficulty::High), None);
        background(&mut m);
        s.keep_raw("Get", &version_path(4), served_qs(4, vec![rated(QA, "One", None), rated(QB, "Two", None)], false));
        let who = olga_named(&s);
        let release = s.hold();
        quiet(&mut m, &mut banks, &who, now(), 1);
        for _ in 0..100 {
            if version_reads(&s, 4) == 1 {
                break;
            }
            std::thread::sleep(Duration::from_millis(10));
        }
        match how {
            "close" => m.edit.close(),
            // (round 15, critic 91 I2) no Send page before the read: the read on its way is left alone
            "send" => assert!(!m.edit.open_send(LOCAL, owner())),
            "history" => assert!(m.edit.open_history(CID, BID, "Cells", owner())),
            _ => m.edit.forget_account(true),
        }
        for _ in 0..10 {
            quiet(&mut m, &mut banks, &who, now(), 1);
            std::thread::sleep(Duration::from_millis(5));
        }
        release.send(()).unwrap();
        for _ in 0..100 {
            quiet(&mut m, &mut banks, &who, now(), 1);
            if one_of(&mut m).is_some() && !m.edit.busy() {
                break;
            }
            std::thread::sleep(Duration::from_millis(10));
        }
        let want = if how == "account" { 2 } else { 1 };
        assert_eq!(version_reads(&s, 4), want, "{how}: {:?}", s.paths());
        assert_eq!(one_of(&mut m), Some(None), "{how}");
    }
}

/// A Get latest that could not read the base version (no longer kept) says so for the quiet read too: it is not asked again
/// once the page is closed.
#[test]
fn c89_m4_a_base_get_latest_could_not_read_is_not_asked_again_at_once() {
    let (mut m, s, mut banks, _ctx, _acct) = rig();
    phase_c(&mut m, &mut banks, 3, None, None);
    super::bank_links_fix12_tests::not_kept(&s, 3);
    s.on("Post", &format!("{}/merge", put_path()), 200, merge_start(3, 4));
    s.keep_raw("Get", &put_path(), served_qs(4, vec![rated(QA, "One", None), rated(QB, "Two", None)], false));
    assert!(m.edit.open_merge(LOCAL, owner()));
    let checking = AccountSnapshot::of_gate(Gate::Checking);
    quiet(&mut m, &mut banks, &checking, now(), 3);
    assert!(kinds(&m).is_some());
    assert_eq!(version_reads(&s, 3), 1);
    m.edit.cancel_merge();
    quiet(&mut m, &mut banks, &olga_named(&s), now() + Duration::from_secs(61), 3);
    assert_eq!(version_reads(&s, 3), 1);
}

/// Nor does Get latest ask again for a base version the circle would not show a moment ago; it does once that may have passed.
#[test]
fn c89_m4_get_latest_does_not_ask_again_for_a_version_just_refused() {
    let (mut m, s, mut banks, _ctx, _acct) = rig();
    phase_c(&mut m, &mut banks, 3, None, None);
    super::bank_links_fix12_tests::not_kept(&s, 3);
    let who = olga_named(&s);
    let t0 = now();
    quiet(&mut m, &mut banks, &who, t0, 3);
    assert_eq!(version_reads(&s, 3), 1);
    s.keep("Post", &format!("{}/merge", put_path()), 200, merge_start(3, 4));
    s.keep_raw("Get", &put_path(), served_qs(4, vec![rated(QA, "One", None), rated(QB, "Two", None)], false));
    assert!(m.edit.open_merge(LOCAL, owner()));
    quiet(&mut m, &mut banks, &who, t0, 3);
    assert!(kinds(&m).is_some());
    assert_eq!(version_reads(&s, 3), 1, "{:?}", s.paths());
    m.edit.cancel_merge();
    // (a while later; the account being looked up, so only Get latest reads)
    let later = t0 + BASE_UNREADABLE_RETRY + Duration::from_secs(1);
    let checking = AccountSnapshot { gate: Gate::Checking, ..who.clone() };
    m.edit.frame(later, &checking);
    assert!(m.edit.open_merge(LOCAL, owner()));
    quiet(&mut m, &mut banks, &checking, later, 3);
    assert_eq!(version_reads(&s, 3), 2, "{:?}", s.paths());
}

/// (survivor q2) One quiet read at a time: two earlier builds' links, the first read held at the server: the second is not
/// asked until the first is answered.
#[test]
fn c89_q2_one_quiet_read_at_a_time() {
    let (mut m, s, mut banks) = two_at_three();
    for local in [LOCAL, LOCAL_B] {
        m.edit.update_link(local, |l| {
            for b in &mut l.base {
                b.snap.difficulty = None;
            }
            true
        });
    }
    background(&mut m);
    let bpath = format!("/circles/{CID}/banks/{}/versions/3", hex(77));
    s.keep_raw("Get", &version_path(3), served_qs(3, vec![rated(QA, "One", None), rated(QB, "Two", None)], false));
    s.keep("Get", &bpath, 404, json!({"error": "version_not_retained", "oldest_kept": 4}));
    let who = olga_named(&s);
    let release = s.hold();
    for _ in 0..20 {
        quiet(&mut m, &mut banks, &who, now(), 1);
        std::thread::sleep(Duration::from_millis(5));
    }
    assert_eq!(all_version_reads(&s), 1, "{:?}", s.paths());
    release.send(()).unwrap();
    for _ in 0..100 {
        quiet(&mut m, &mut banks, &who, now(), 1);
        if all_version_reads(&s) == 2 {
            break;
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    assert_eq!(all_version_reads(&s), 2, "{:?}", s.paths());
}

// ---- critic 89 m3: no quiet read for a copy that can't send ---------------------------------------------------------------------

/// A viewer's copy (a follow link, or a viewer's role) never shows a count: its base version is not read quietly (a bank
/// that hides its keys would spend the viewer's hidden reads).
#[test]
fn c89_m3_no_quiet_read_for_a_viewers_copy() {
    for how in ["follow", "viewer", "viewers_form"] {
        let (mut m, s, mut banks, _ctx, _acct) = rig();
        phase_c(&mut m, &mut banks, 3, None, None);
        m.edit.update_link(LOCAL, |l| {
            match how {
                "follow" => l.mode = LinkMode::Follow,
                "viewer" => l.role_seen = Some("viewer".into()),
                _ => l.viewers_form = true,
            }
            true
        });
        s.keep_raw("Get", &version_path(3), served_qs(3, vec![rated(QA, "One", None), rated(QB, "Two", None)], false));
        quiet(&mut m, &mut banks, &olga_named(&s), now(), 3);
        assert_eq!(all_version_reads(&s), 0, "{how}");
    }
}

/// (survivor q3) Nothing is read while the account is being looked up (a session is there, the gate is not open).
#[test]
fn c89_q3_no_quiet_read_before_the_sign_in_is_known() {
    let (mut m, s, mut banks, _ctx, _acct) = rig();
    phase_c(&mut m, &mut banks, 3, None, None);
    s.keep_raw("Get", &version_path(3), served_qs(3, vec![rated(QA, "One", None), rated(QB, "Two", None)], false));
    let checking = AccountSnapshot { gate: Gate::Checking, ..olga_named(&s) };
    assert!(checking.session.is_some());
    quiet(&mut m, &mut banks, &checking, now(), 3);
    assert_eq!(all_version_reads(&s), 0);
    quiet(&mut m, &mut banks, &olga_named(&s), now(), 2);
    assert_eq!(version_reads(&s, 3), 1);
}

// ---- critic 89 m4: when a read is asked again ------------------------------------------------------------------------------------

/// (survivor q7) A version the circle no longer keeps is not asked again a minute later (that is for a circle that could not
/// be reached), but after a while it is.
#[test]
fn c89_m4_an_unreadable_version_is_asked_again_after_a_while_not_at_once() {
    let (mut m, s, mut banks, _ctx, _acct) = rig();
    phase_c(&mut m, &mut banks, 3, None, None);
    super::bank_links_fix12_tests::not_kept(&s, 3);
    let who = olga_named(&s);
    let t0 = now();
    quiet(&mut m, &mut banks, &who, t0, 3);
    assert_eq!(version_reads(&s, 3), 1);
    quiet(&mut m, &mut banks, &who, t0 + Duration::from_secs(61), 3);
    assert_eq!(version_reads(&s, 3), 1, "not a minute later");
    quiet(&mut m, &mut banks, &who, t0 + BASE_UNREADABLE_RETRY + Duration::from_secs(1), 3);
    assert_eq!(version_reads(&s, 3), 2, "after a while");
}

/// A version held back (withheld) is asked again as soon as the circle's state of the bank changes (it was released).
#[test]
fn c89_m4_a_withheld_version_is_asked_again_when_the_circle_changes() {
    let (mut m, s, mut banks, _ctx, _acct) = rig();
    phase_c(&mut m, &mut banks, 3, None, None);
    m.edit.update_link(LOCAL, |l| {
        l.seen.under_review = true;
        true
    });
    s.on("Get", &version_path(3), 404, json!({"error": "version_withheld"}));
    s.keep_raw("Get", &version_path(3), served_qs(3, vec![rated(QA, "One", Some("High")), rated(QB, "Two", None)], false));
    let who = olga_named(&s);
    quiet(&mut m, &mut banks, &who, now(), 3);
    assert_eq!(version_reads(&s, 3), 1);
    assert_eq!(one_of(&mut m), None);
    m.edit.update_link(LOCAL, |l| {
        l.seen.under_review = false;
        true
    });
    quiet(&mut m, &mut banks, &who, now(), 3);
    assert_eq!(version_reads(&s, 3), 2);
    assert_eq!(one_of(&mut m), Some(Some(Difficulty::High)));
}

/// A link partly learnt (its version lacks one of its questions) is not read again: not after a restart, not for Get latest.
#[test]
fn c89_m4_a_partly_learnt_link_is_not_read_again() {
    let log = Arc::new(std::sync::Mutex::new(Vec::new()));
    let (mut m, s, mut banks, _ctx, _acct) = rig();
    phase_c(&mut m, &mut banks, 3, None, None);
    s.keep_raw("Get", &version_path(3), served_qs(3, vec![rated(QA, "One", None), rated(QC, "Two", None)], false));
    let who = olga_named(&s);
    quiet(&mut m, &mut banks, &who, now(), 3);
    assert_eq!(version_reads(&s, 3), 1);
    assert!(m.edit.with_link(LOCAL, super::circle_links::difficulty_unknown).unwrap(), "partly");
    *log.lock().unwrap() = m.edit.links().to_vec();
    let mut m = restarted(log);
    quiet(&mut m, &mut banks, &who, now(), 3);
    assert_eq!(version_reads(&s, 3), 1, "not after a restart");
    s.on("Post", &format!("{}/merge", put_path()), 200, merge_start(3, 4));
    s.keep_raw("Get", &put_path(), served_qs(4, vec![rated(QA, "One", None), rated(QB, "Two", None)], false));
    assert!(m.edit.open_merge(LOCAL, owner()));
    quiet(&mut m, &mut banks, &who, now(), 3);
    assert!(kinds(&m).is_some());
    assert_eq!(version_reads(&s, 3), 1, "not for Get latest");
}

/// (survivors q8, q9) A quiet read whose thread ended without an answer is asked again a minute later, not at once.
#[test]
fn c89_q8_a_quiet_read_whose_thread_died_is_asked_again_later() {
    let (mut m, s, mut banks, _ctx, _acct) = rig();
    phase_c(&mut m, &mut banks, 3, None, None);
    background(&mut m);
    // (no answer scripted for the version: the read's thread ends without one)
    // (phase D3: the scripted session answers an unscripted kept-version read with 404; a circle that can not be
    // reached for it says so directly, and the quiet read treats it the same as a lost job)
    s.on("Get", &version_path(3), 503, json!({"error": "unavailable"}));
    let who = olga_named(&s);
    let t0 = now();
    for _ in 0..40 {
        quiet(&mut m, &mut banks, &who, t0, 1);
        std::thread::sleep(Duration::from_millis(10));
    }
    assert_eq!(version_reads(&s, 3), 1, "{:?}", s.paths());
    s.keep_raw("Get", &version_path(3), served_qs(3, vec![rated(QA, "One", None), rated(QB, "Two", None)], false));
    let later = t0 + Duration::from_secs(61);
    for _ in 0..100 {
        quiet(&mut m, &mut banks, &who, later, 1);
        if one_of(&mut m).is_some() {
            break;
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    assert_eq!(version_reads(&s, 3), 2);
    assert_eq!(one_of(&mut m), Some(None));
}

/// (survivor n1) A reply that can't be understood is not a circle that could not be reached: not asked again a minute later.
#[test]
fn c89_n1_a_reply_that_cant_be_read_is_not_asked_again_at_once() {
    // (a body of no known shape, and one whose bank the app can't read: the server's answer, not a failure to reach it)
    for body in [&b"not a bank"[..], &br#"{"circle":{"id":"x"},"bank":7,"payload":{}}"#[..]] {
        let (mut m, s, mut banks, _ctx, _acct) = rig();
        phase_c(&mut m, &mut banks, 3, None, None);
        s.keep_raw("Get", &version_path(3), body.to_vec());
        let who = olga_named(&s);
        let t0 = now();
        quiet(&mut m, &mut banks, &who, t0, 3);
        quiet(&mut m, &mut banks, &who, t0 + Duration::from_secs(61), 3);
        assert_eq!(version_reads(&s, 3), 1, "{}", String::from_utf8_lossy(body));
    }
}

/// (survivor n4) The sign-in ended (401): asked again a minute later, and learnt then.
#[test]
fn c89_n4_a_read_refused_for_an_ended_sign_in_is_asked_again() {
    let (mut m, s, mut banks, _ctx, _acct) = rig();
    phase_c(&mut m, &mut banks, 3, None, None);
    s.on("Get", &version_path(3), 401, json!({"error": "unauthorized"}));
    s.keep_raw("Get", &version_path(3), served_qs(3, vec![rated(QA, "One", None), rated(QB, "Two", None)], false));
    let who = olga_named(&s);
    let t0 = now();
    quiet(&mut m, &mut banks, &who, t0, 3);
    assert_eq!(version_reads(&s, 3), 1);
    assert_eq!(one_of(&mut m), None);
    quiet(&mut m, &mut banks, &who, t0 + Duration::from_secs(61), 3);
    assert_eq!(version_reads(&s, 3), 2);
    assert_eq!(one_of(&mut m), Some(None));
}

/// A link as the phase C build wrote it to `share-circles.json` (critic 87's head_link.json): version 4, two questions, no
/// difficulty in its base, no record of a read.
const HEAD_LINK: &str = r#"{"circle_id":"ccccccccccccccccccccccccccccccc1","circle_name":"Algebra team","circle_bank_id":"bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb1","local_bank_id":"local-bank-1","title":"Cells","subject":"","course":"","grade":"","license":"CC-BY-4.0","base_version":4,"base_version_id":"00000000000000000000000000000003","qids":[{"local":"0011a17ae1b5cd","qid":"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa1"},{"local":"0031a17ae1b5cd","qid":"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa2"}],"base":[{"local_id":"0011a17ae1b5cd","qid":"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa1","fp":"1d29ef890481420a","snap":{"stem":"One","kind":{"Mc":{"answers":[{"text":"A","correct":true,"locked":false},{"text":"B","correct":false,"locked":false}]}},"points":1.0,"topics":[],"explanation":null,"image":null,"keyless":false},"local_image":null},{"local_id":"0031a17ae1b5cd","qid":"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa2","fp":"6419e496d27b5114","snap":{"stem":"Two","kind":{"Mc":{"answers":[{"text":"A","correct":true,"locked":false},{"text":"B","correct":false,"locked":false}]}},"points":1.0,"topics":[],"explanation":null,"image":null,"keyless":false},"local_image":null}],"head_seen":4,"head_seen_at":null,"head_seen_by":null,"linked_at":1700000000000,"bank_modified":1700000010,"bank_print":"38f8709a022db5f3c0e93e30bf93a3f592eed061d09a9ada17bb10d0af073817"}"#;

// keep the imports honest across the file
#[allow(dead_code)]
fn _uses(m: &mut CirclesModel) {
    host_a(m);
}
