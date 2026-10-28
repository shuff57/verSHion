//! Phase D1, scoped fix round (critics 101 and 102): each test written first against the code it pins. The merge writes the
//! banks file before the link says it was made; a save failure is said on every strip state; "Put all of mine back" survives a
//! restart; Dismiss's words match what it does; the badge does not promise a button the page cannot offer.

use std::sync::atomic::Ordering;
use std::sync::Arc;
use std::time::Duration;

use super::bank_links::StripButton;
use super::bank_links_fix11_tests::{rated, rig, version_path};
use super::bank_links_fix15_tests::phase_c;
use super::bank_links_fix16_tests::{rebuilt, v3};
use super::bank_links_fix19_tests::{dir_model, fresh_dir, pending, recording, recording_then_blocking, signed_out, unblock_links, Saved};
use super::bank_links_fix4_tests::*;
use super::bank_links_fix8_tests::*;
use super::bank_links_fix9_tests::*;
use super::bank_links_fix_tests::*;
use super::bank_links_tests::*;
use super::circle_edit::*;
use super::circles::CirclesModel;
use super::circles_tests::*;
use super::publish::AccountSnapshot;
use crate::model::{Difficulty, QuestionBank};

fn strip_view(m: &mut CirclesModel, banks: &[QuestionBank], who: &AccountSnapshot) -> super::bank_links::StripView {
    let bank = banks[0].clone();
    let CirclesModel { links, edit, .. } = m;
    links.strip(edit, &bank, who, now(), NOW).unwrap()
}

// ---- c101 I1: the merged bank reaches the disk before the link says it was merged ---------------------------------------

/// The banks file can't be written when she presses Apply changes: nothing is kept (the bank as it was, the link not
/// advanced), the page says so and offers Apply again, and once a save works the merge is made and the link advances.
#[test]
fn c101_i1_a_merge_whose_banks_write_fails_does_not_advance_the_link() {
    let (mut m, s, mut banks, ctx, mut acct) = rig();
    let k = rebuilt(&mut m);
    // the rebuilt model learns the signed-in session from a frame, as the other rebuilt-model tests do
    let who = olga_named(&s);
    frames(&mut m, &mut banks, &ctx, &mut acct, &who, 1);
    get_latest(&mut m, &s, &mut banks, &ctx, &mut acct, served_qs(4, vec![rated(QA, "One (b)", None)], false));
    let before = banks[0].clone();
    let base_before = m.edit.link_for_local(LOCAL).unwrap().base_version;
    k.save_fails.store(true, Ordering::SeqCst);
    m.edit.apply_merge();
    until_as(&ctx, &mut m, &mut banks, &mut acct, &s, LOCAL, &|m| matches!(&m.edit.view, EditView::Merge(v) if matches!(v.phase, MergePhase::Review(_))));
    // nothing was applied: the bank is as it was, the link is not advanced, and she is told why
    assert_eq!(banks[0].modified, before.modified, "the bank as it was");
    assert_eq!(banks[0].questions[0].stem, before.questions[0].stem, "the merged question is not kept");
    assert_eq!(m.edit.link_for_local(LOCAL).unwrap().base_version, base_before, "the link is not advanced");
    let problem = match &m.edit.view {
        EditView::Merge(v) => match &v.phase {
            MergePhase::Review(r) => r.problem.clone(),
            _ => None,
        },
        _ => None,
    };
    assert!(problem.as_deref().is_some_and(|p| p.contains("couldn't be saved") && p.contains("nothing was changed") && p.contains("Apply changes")), "{problem:?}");
    assert!(!matches!(&m.edit.view, EditView::Merge(v) if matches!(v.phase, MergePhase::Applied(_))));
    // once the banks file can be written the same review applies: the bank changes and the link advances
    k.save_fails.store(false, Ordering::SeqCst);
    m.edit.apply_merge();
    until_as(&ctx, &mut m, &mut banks, &mut acct, &s, LOCAL, &|m| matches!(&m.edit.view, EditView::Merge(v) if matches!(v.phase, MergePhase::Applied(_))));
    assert!(m.edit.link_for_local(LOCAL).unwrap().base_version > base_before, "the link advanced");
    assert_ne!(banks[0].questions[0].stem, before.questions[0].stem, "the merged question is kept");
}

// ---- c102 I1: a save failure is said even where another state would return first ---------------------------------------

/// The circle's newest version is held for review while the banks file can't be written for the difficulty pass: the strip
/// says her own edits aren't saved, in the warning colour, with Try again first, and never "You can keep working here".
#[test]
fn c102_i1_a_save_failure_is_said_even_while_the_newest_version_is_held_for_review() {
    let (mut m, s, mut banks, ctx, mut acct) = rig();
    let k = rebuilt(&mut m);
    phase_c(&mut m, &mut banks, 3, Some(Difficulty::Low), Some(Difficulty::Low));
    s.keep_raw("Get", &version_path(3), v3(Some("High"), None));
    let who = olga_named(&s);
    k.save_fails.store(true, Ordering::SeqCst);
    let _ = frames(&mut m, &mut banks, &ctx, &mut acct, &who, 3);
    let st = m.edit.status_of(&banks[0]).expect("a status");
    assert_eq!(st.learning, Some(Learning::Held), "the pass held on the banks file");
    // the circle's newest version is held for review too: the state that used to return before the Held words
    m.edit.update_link(LOCAL, |l| {
        l.seen.under_review = true;
        true
    });
    let v = strip_view(&mut m, &banks, &who);
    assert!(v.warn, "{v:?}");
    assert_eq!(v.buttons.first(), Some(&StripButton::TryAgain), "{v:?}");
    assert!(v.line.contains("your own edits aren't saved"), "{}", v.line);
    assert!(!v.line.contains("You can keep working here"), "{}", v.line);
    assert!(!v.line.contains("up to date"), "{}", v.line);
}

// ---- c101 m1: "Put all of mine back" survives a restart -------------------------------------------------------------------

/// The pass wrote the banks file, then the links file could not be written until she quit; meanwhile she put back *every*
/// question the pass moved. After a restart the pass is done: her difficulties stay, the news lists them as back, and the
/// banks are not written again.
#[test]
fn c101_m1_put_all_back_while_the_links_file_cant_be_written_survives_a_restart() {
    let dir = fresh_dir("m1all");
    let saved: Saved = Arc::default();
    let mut e = dir_model(&dir, recording_then_blocking(&saved, &dir));
    let (bank, l) = pending();
    e.add_link(l).unwrap();
    let mut banks = vec![bank];
    let out = signed_out();
    e.frame(now(), &out);
    assert!(e.sync(&mut banks));
    assert!(e.put_all_mine_back(&mut banks, LOCAL), "both questions can be put back");
    assert_eq!((banks[0].questions[0].difficulty, banks[0].questions[1].difficulty), (Some(Difficulty::Low), Some(Difficulty::Low)), "hers, back");
    unblock_links(&dir);
    let saved2: Saved = Arc::default();
    let mut e2 = dir_model(&dir, recording(&saved2));
    e2.frame(now() + Duration::from_secs(600), &out);
    e2.sync(&mut banks);
    assert_eq!((banks[0].questions[0].difficulty, banks[0].questions[1].difficulty), (Some(Difficulty::Low), Some(Difficulty::Low)), "her put-back stays");
    assert!(saved2.lock().unwrap().is_empty(), "the pass does not run again");
    let link = e2.link_for_local(LOCAL).unwrap();
    assert!(!link.inherit && link.at_read.is_none() && link.pass_times.is_none(), "the pass is done");
    let news = link.difficulty_news.expect("the news");
    assert!(news.questions.iter().all(|q| q.put_back), "{news:?}");
    let _ = std::fs::remove_dir_all(&dir);
}

// ---- c102 m1, m2: Dismiss's words, and the badge's promise -----------------------------------------------------------------

/// Dismiss removes the list for good, and the words say so.
#[test]
fn c102_m1_dismiss_says_the_list_goes_for_good() {
    let t = super::circles_text::NEWS_DISMISS;
    assert!(t.contains("for good") && t.contains("offer to put yours back"), "{t}");
    assert!(!t.contains("hides"), "{t}");
}

/// "newer version" on the badge does not promise Get latest, which the page cannot offer while the version is held.
#[test]
fn c102_m2_the_badge_does_not_promise_get_latest() {
    let h = super::bank_links::note_hint_for("newer version · 1 unsent", false).expect("a hint");
    assert!(h.contains("newer version"), "{h}");
    assert!(!h.contains("Get latest"), "{h}");
}
