//! Phase D1, round 19 (critics 99 and 100): each test written first against the code it pins. The owner's rule stays (after one
//! successful read of the circle's version the copy takes the circle's difficulty; a later deliberate change of hers supersedes).

use std::sync::atomic::Ordering;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use serde_json::json;

use super::bank_links::StripButton;
use super::bank_links_fix11_tests::{rig, version_path};
use super::bank_links_fix15_tests::phase_c;
use super::bank_links_fix16_tests::{rebuilt, sent_q, v3};
use super::bank_links_fix18_tests::many_news;
use super::bank_links_fix4_tests::*;
use super::bank_links_fix8_tests::*;
use super::bank_links_fix9_tests::*;
use super::bank_links_fix_tests::*;
use super::bank_links_tests::*;
use super::circle_edit::*;
use super::circle_links::{CircleLink, LinkLog};
use super::circles::CirclesModel;
use super::circles_tests::*;
use super::publish::AccountSnapshot;
use crate::model::{Difficulty, QuestionBank};

fn strip_view(m: &mut CirclesModel, banks: &[QuestionBank], who: &AccountSnapshot) -> super::bank_links::StripView {
    let bank = banks[0].clone();
    let CirclesModel { links, edit, .. } = m;
    links.strip(edit, &bank, who, now(), NOW).unwrap()
}

/// Clicks the `nth` drawn `label` on A's page.
fn click(ctx: &egui::Context, m: &mut CirclesModel, banks: &mut [QuestionBank], acct: &mut FakeAccount, who: &AccountSnapshot, label: &str, nth: usize) -> bool {
    let mut b = browse();
    let mut save = false;
    let hit = super::circles_phasec_tests::click_text(ctx, 1280.0, label, nth, &mut |ui| {
        save |= super::bank_links_ui::strip_with(ui, m, banks, &mut b, acct, who, now(), NOW, LOCAL).save;
    });
    assert!(hit, "no {label:?} drawn");
    save
}

fn badge_warns(m: &mut CirclesModel, banks: &[QuestionBank]) -> bool {
    let st = m.edit.status_of(&banks[0]);
    m.edit.with_link(LOCAL, |l| super::bank_links::badge_warns_with(l, st.as_ref())).unwrap()
}

/// The advice for a banks file that can't be written: one she can act on.
const ADVICE: &str = "Keep verSHion open and press Try again; if it keeps failing, free some disk space.";

/// A's page after the read of version 3 (One rated High there, Two none) replaced her Easy on both; banks files counted.
fn with_news_counted() -> (Rig, super::bank_links_fix16_tests::Sinks) {
    let (mut m, s, mut banks, ctx, mut acct) = rig();
    let k = rebuilt(&mut m);
    s.keep("Post", "/circles/heads", 200, heads(3));
    phase_c(&mut m, &mut banks, 3, Some(Difficulty::Low), Some(Difficulty::Low));
    s.keep_raw("Get", &version_path(3), v3(Some("High"), None));
    let who = olga_named(&s);
    frames(&mut m, &mut banks, &ctx, &mut acct, &who, 3);
    assert_eq!(m.edit.with_link(LOCAL, |l| l.difficulty_news.as_ref().map(|n| n.questions.len())).flatten(), Some(2));
    ((m, s, banks, ctx, acct), k)
}

// ---- c100 I2: a save failure that loses her edits looks like what it is ----------------------------------------------------------

/// Put mine back while the banks file can't be written: the strip is in the warning colour, never "up to date", with Try again as
/// its big button and advice she can act on (never "data folder"); the badge warns and says so; there is one Dismiss (the list's),
/// and dismissing the list leaves the warning. It lasts until a save works: Try again that fails again says so, Try again that
/// works writes the banks and the strip is calm again.
#[test]
fn c100_i2_a_put_back_that_cant_be_saved_warns_until_a_save_works() {
    let ((mut m, s, mut banks, ctx, mut acct), k) = with_news_counted();
    let who = olga_named(&s);
    k.save_fails.store(true, Ordering::SeqCst);
    click(&ctx, &mut m, &mut banks, &mut acct, &who, "Put mine back", 1);
    assert_eq!(banks[0].questions[1].difficulty, None, "nothing changed");
    let strip = frames(&mut m, &mut banks, &ctx, &mut acct, &who, 2);
    let v = strip_view(&mut m, &banks, &who);
    assert!(v.warn, "{v:?}");
    assert!(!v.line.contains("up to date"), "{}", v.line);
    assert!(v.line.contains("your own edits aren't saved") && v.line.contains(ADVICE), "{}", v.line);
    assert!(!v.line.contains("data folder") && !strip.contains("data folder"), "{strip}");
    assert_eq!(v.buttons.first(), Some(&StripButton::TryAgain), "{v:?}");
    assert!(badge_warns(&mut m, &banks), "the badge warns");
    assert!(badge(&mut m, &banks[0].clone()).contains("couldn't save"), "{}", badge(&mut m, &banks[0].clone()));
    // (c100 minor 7) one Dismiss: the list's
    assert_eq!(strip.lines().filter(|l| *l == "Dismiss").count(), 1, "{strip}");
    // the list dismissed, time passing: still said
    m.edit.dismiss_news(LOCAL);
    for i in 1..=3u64 {
        page_frame(&mut m, &mut banks, &mut acct, &who, now() + Duration::from_secs(61 * i));
    }
    let v = strip_view(&mut m, &banks, &who);
    assert!(v.warn && v.line.contains("your own edits aren't saved"), "{v:?}");
    assert!(badge_warns(&mut m, &banks));
    // Try again, still failing: tried again, still said
    let writes = k.saves.lock().unwrap().len();
    click(&ctx, &mut m, &mut banks, &mut acct, &who, "Try again", 0);
    frames(&mut m, &mut banks, &ctx, &mut acct, &who, 2);
    let v = strip_view(&mut m, &banks, &who);
    assert!(v.warn && v.line.contains("Tried 2 times"), "{v:?}");
    // Try again once saving works: the banks are written, and the strip is calm again
    k.save_fails.store(false, Ordering::SeqCst);
    click(&ctx, &mut m, &mut banks, &mut acct, &who, "Try again", 0);
    frames(&mut m, &mut banks, &ctx, &mut acct, &who, 2);
    assert_eq!(k.saves.lock().unwrap().len(), writes + 1, "the banks file was written");
    let v = strip_view(&mut m, &banks, &who);
    assert!(!v.warn && !v.line.contains("aren't saved"), "{v:?}");
    assert!(!badge_warns(&mut m, &banks));
    assert!(!badge(&mut m, &banks[0].clone()).contains("couldn't save"));
}

/// A save that works for another reason (Put mine back pressed again) ends the warning too.
#[test]
fn c100_i2_any_save_that_works_ends_the_warning() {
    let ((mut m, s, mut banks, ctx, mut acct), k) = with_news_counted();
    let who = olga_named(&s);
    k.save_fails.store(true, Ordering::SeqCst);
    click(&ctx, &mut m, &mut banks, &mut acct, &who, "Put mine back", 1);
    frames(&mut m, &mut banks, &ctx, &mut acct, &who, 1);
    assert!(strip_view(&mut m, &banks, &who).warn);
    k.save_fails.store(false, Ordering::SeqCst);
    click(&ctx, &mut m, &mut banks, &mut acct, &who, "Put mine back", 1);
    assert_eq!(banks[0].questions[1].difficulty, Some(Difficulty::Low));
    frames(&mut m, &mut banks, &ctx, &mut acct, &who, 1);
    let v = strip_view(&mut m, &banks, &who);
    assert!(!v.warn && !v.line.contains("aren't saved"), "{v:?}");
}

/// Every message for a file that can't be written gives the same advice she can act on, never "check the data folder".
#[test]
fn c100_i2_the_advice_is_one_she_can_act_on() {
    use super::circles_text as t;
    for raw in ["No space left on device (os error 28)", "Permission denied (os error 13)", "something else", "file is locked by another process"] {
        for line in [t::banks_not_saved(raw), t::put_back_not_saved(raw), t::safety_copy_not_saved(raw), t::links_not_saved(raw)] {
            assert!(line.contains(ADVICE) && !line.contains("Check verSHion's data folder") && !line.contains("Check that verSHion may write"), "{line}");
        }
        assert!(t::banks_not_saved(raw).contains("your own edits aren't saved"), "{raw}");
        assert!(t::put_back_not_saved(raw).contains("your own edits aren't saved"), "{raw}");
    }
}

// ---- c100 I1: Check now gives visible feedback -------------------------------------------------------------------------------------

/// A's page whose version 3 the circle no longer keeps (an earlier build's link), one question reworded.
fn not_kept_rig() -> Rig {
    let (mut m, s, mut banks, ctx, mut acct) = rig();
    s.keep("Post", "/circles/heads", 200, heads(3));
    phase_c(&mut m, &mut banks, 3, Some(Difficulty::Low), None);
    banks[0].questions[1].stem = "Two (my fix)".into();
    banks[0].modified += 30;
    super::bank_links_fix12_tests::not_kept(&s, 3);
    let who = olga_named(&s);
    frames(&mut m, &mut banks, &ctx, &mut acct, &who, 3);
    assert_eq!(m.edit.status(LOCAL).unwrap().learning, Some(Learning::NotKept));
    (m, s, banks, ctx, acct)
}

/// Not kept: the strip says what Check now does. Pressed, the strip says it is checking, then that it checked just now and nothing
/// changed; a minute later it says when.
#[test]
fn c100_i1_check_now_says_what_it_does_and_what_it_found() {
    let (mut m, s, mut banks, ctx, mut acct) = not_kept_rig();
    let who = olga_named(&s);
    let v = strip_view(&mut m, &banks, &who);
    assert_eq!(v.buttons.first(), Some(&StripButton::CheckNow), "{v:?}");
    assert!(v.line.contains("Check now asks the circle again."), "{}", v.line);
    let before = v.line.clone();
    click(&ctx, &mut m, &mut banks, &mut acct, &who, "Check now", 0);
    let seen: Vec<String> = (0..4).map(|_| frames(&mut m, &mut banks, &ctx, &mut acct, &who, 1)).collect();
    assert!(seen.iter().take(2).any(|t| t.contains("Checking")), "{seen:?}");
    let v = strip_view(&mut m, &banks, &who);
    assert!(v.line.starts_with(&before) && v.line.contains("Checked just now: still the same."), "{}", v.line);
    let (_, later) = page_frame(&mut m, &mut banks, &mut acct, &who, now() + Duration::from_secs(61));
    assert!(later.contains("Checked 1 minute ago: still the same."), "{later}");
    // (not said for ever: after the next due check it is gone)
    let (_, much_later) = page_frame(&mut m, &mut banks, &mut acct, &who, now() + Duration::from_secs(11 * 60));
    assert!(!much_later.contains("still the same"), "{much_later}");
}

/// Pressed when something did change (the circle has a newer version now): the new result is said, never "still the same".
#[test]
fn c100_i1_check_now_that_finds_something_says_the_new_result() {
    let (mut m, s, mut banks, ctx, mut acct) = not_kept_rig();
    let who = olga_named(&s);
    s.keep("Post", "/circles/heads", 200, heads(4));
    click(&ctx, &mut m, &mut banks, &mut acct, &who, "Check now", 0);
    frames(&mut m, &mut banks, &ctx, &mut acct, &who, 4);
    let v = strip_view(&mut m, &banks, &who);
    assert!(v.line.contains("You have version 3") && !v.line.contains("still the same"), "{}", v.line);
}

// ---- c99 m1, m2: a pass never writes the banks file while the links file is not in step -------------------------------------------

fn signed_out() -> AccountSnapshot {
    AccountSnapshot { gate: super::publish::Gate::SignedOut, session: None, account_open: true, account: None, name: None }
}

fn fresh_dir(tag: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!("vershion-r19-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

/// The links file can't be written (a sync tool holds its temporary name) until `unblock_links`.
fn block_links(dir: &std::path::Path) {
    std::fs::create_dir_all(dir.join("share-circles.json.tmp").join("held")).unwrap();
}

fn unblock_links(dir: &std::path::Path) {
    let _ = std::fs::remove_dir_all(dir.join("share-circles.json.tmp"));
}

type Saved = Arc<Mutex<Vec<Vec<QuestionBank>>>>;

fn dir_model(dir: &std::path::Path, saver: BankSaver) -> CircleEditModel {
    let media: super::publish::SharedMedia = Arc::new(std::collections::HashMap::<String, Vec<u8>>::new());
    let mut e = CircleEditModel::new(super::model::Runner::Inline, Arc::new(Mem::default()), media, Arc::new(NoDiskRestore), LinkLog::Dir(dir.to_path_buf())).with_retry(super::pool::RetryPolicy::none()).with_bank_saver(saver);
    e.set_clock(NOW);
    e
}

fn recording(saved: &Saved) -> BankSaver {
    let sv = saved.clone();
    Arc::new(move |b: &[QuestionBank]| {
        sv.lock().unwrap().push(b.to_vec());
        Ok(())
    })
}

/// Records each banks file, then makes the links file unwritable (a sync tool takes it right then).
fn recording_then_blocking(saved: &Saved, dir: &std::path::Path) -> BankSaver {
    let (sv, d) = (saved.clone(), dir.to_path_buf());
    Arc::new(move |b: &[QuestionBank]| {
        sv.lock().unwrap().push(b.to_vec());
        block_links(&d);
        Ok(())
    })
}

/// A's bank (One, Two both her Low) whose link has just learnt version 1 (One High there, Two none): the pass is to run.
fn pending() -> (QuestionBank, CircleLink) {
    let (mut bank, mut l) = linked(&[(QA, "One"), (QB, "Two")]);
    bank.questions[0].difficulty = Some(Difficulty::Low);
    bank.questions[1].difficulty = Some(Difficulty::Low);
    l.base[0].snap.difficulty = Some(Some(Difficulty::High));
    l.base[1].snap.difficulty = Some(None);
    l.difficulty_read = Some(l.base_version);
    l.inherit = true;
    l.at_read = None;
    l.mark_bank(&bank);
    (bank, l)
}

fn news_of(e: &mut CircleEditModel) -> super::circle_links::DifficultyNews {
    e.with_link(LOCAL, |l| l.difficulty_news.clone()).flatten().expect("the news")
}

/// (p99_1 turned around) The links file can't be written from before the pass: the first try, the retry a minute later and Try
/// again all hold, and the banks file is never written. She rates One Medium meanwhile; once the links file can be written, the pass
/// keeps her Medium (the bank as the read found it was kept in memory) and Two takes the circle's.
#[test]
fn c99_m1_a_retry_never_writes_the_banks_file_before_the_links_file() {
    let dir = fresh_dir("m1");
    let saved: Saved = Arc::default();
    let mut e = dir_model(&dir, recording(&saved));
    let (bank, l) = pending();
    e.add_link(l).unwrap();
    let mut banks = vec![bank];
    block_links(&dir);
    let out = signed_out();
    let t0 = now();
    e.frame(t0, &out);
    assert!(!e.sync(&mut banks));
    e.frame(t0 + Duration::from_secs(61), &out);
    assert!(!e.sync(&mut banks));
    e.read_base_again(LOCAL);
    e.frame(t0 + Duration::from_secs(62), &out);
    assert!(!e.sync(&mut banks));
    assert!(saved.lock().unwrap().is_empty(), "never the banks file before the links file");
    assert_eq!(e.status(LOCAL).unwrap().learning, Some(Learning::Held));
    banks[0].questions[0].difficulty = Some(Difficulty::Medium);
    banks[0].modified += 5;
    e.frame(t0 + Duration::from_secs(123), &out);
    assert!(!e.sync(&mut banks), "still held");
    unblock_links(&dir);
    e.frame(t0 + Duration::from_secs(184), &out);
    assert!(e.sync(&mut banks));
    assert_eq!((banks[0].questions[0].difficulty, banks[0].questions[1].difficulty), (Some(Difficulty::Medium), None), "her Medium stays");
    let file = LinkLog::Dir(dir.clone()).load();
    assert!(!file[0].inherit, "done, in the file");
    let _ = std::fs::remove_dir_all(&dir);
}

/// The same, quit before the links file could ever be written: after a restart the pass runs as new, and no news is lost (Two's
/// Low -> none is listed, and One's, whatever she had, can be put back).
#[test]
fn c99_m1_after_a_restart_the_pass_runs_as_new_and_loses_no_news() {
    let dir = fresh_dir("m1r");
    let saved: Saved = Arc::default();
    let mut e = dir_model(&dir, recording(&saved));
    let (bank, l) = pending();
    e.add_link(l).unwrap();
    let mut banks = vec![bank];
    block_links(&dir);
    let out = signed_out();
    let t0 = now();
    e.frame(t0, &out);
    e.sync(&mut banks);
    e.frame(t0 + Duration::from_secs(61), &out);
    e.sync(&mut banks);
    assert!(saved.lock().unwrap().is_empty());
    banks[0].questions[0].difficulty = Some(Difficulty::Medium);
    banks[0].modified += 5;
    unblock_links(&dir);
    let mut e2 = dir_model(&dir, recording(&Arc::default()));
    e2.frame(t0 + Duration::from_secs(600), &out);
    assert!(e2.sync(&mut banks));
    let n = news_of(&mut e2);
    let two = n.questions.iter().find(|q| q.id == banks[0].questions[1].id).expect("Two's news");
    assert_eq!((two.before, two.after), (Some(Difficulty::Low), None));
    let one = n.questions.iter().find(|q| q.id == banks[0].questions[0].id).expect("One's news").clone();
    assert_eq!(one.before, Some(Difficulty::Medium));
    let link = e2.link_for_local(LOCAL).unwrap();
    assert_eq!(super::circle_links::news_state(&link, &one, &banks[0]), super::circle_links::NewsState::Replaced, "she can put hers back");
    let _ = std::fs::remove_dir_all(&dir);
}

/// (p99_1c turned around) The pass wrote the banks file, then the links file could not be written until she quit; meanwhile she pressed
/// Put mine back on Two. After a restart the pass is done (One already has the circle's): her Two stays, the news lists both, and Two
/// says hers is back.
#[test]
fn c99_m2_put_back_while_the_links_file_cant_be_written_survives_a_restart() {
    let dir = fresh_dir("m2");
    let saved: Saved = Arc::default();
    let mut e = dir_model(&dir, recording_then_blocking(&saved, &dir));
    let (bank, l) = pending();
    e.add_link(l).unwrap();
    let mut banks = vec![bank];
    let out = signed_out();
    e.frame(now(), &out);
    assert!(e.sync(&mut banks));
    let two = banks[0].questions[1].id.clone();
    assert!(e.put_mine_back(&mut banks, LOCAL, &two));
    assert_eq!(saved.lock().unwrap().last().unwrap()[0].questions[1].difficulty, Some(Difficulty::Low), "banks.json has her Low");
    unblock_links(&dir);
    let saved2: Saved = Arc::default();
    let mut e2 = dir_model(&dir, recording(&saved2));
    e2.frame(now() + Duration::from_secs(600), &out);
    e2.sync(&mut banks);
    assert_eq!((banks[0].questions[0].difficulty, banks[0].questions[1].difficulty), (Some(Difficulty::High), Some(Difficulty::Low)), "her Put mine back stays");
    assert!(saved2.lock().unwrap().is_empty(), "nothing to write again");
    let n = news_of(&mut e2);
    assert_eq!(n.questions.len(), 2, "{n:?}");
    let link = e2.link_for_local(LOCAL).unwrap();
    let item = n.questions.iter().find(|q| q.id == two).unwrap();
    assert_eq!(super::circle_links::news_state(&link, item, &banks[0]), super::circle_links::NewsState::Back, "{n:?}");
    assert!(!link.inherit);
    let _ = std::fs::remove_dir_all(&dir);
}

// ---- c99 m4, m7: a held Undo pass survives a restart and holds Get latest's review ----------------------------------------------

/// One (her Low) deleted before the read, the read settled (One High in the circle); then One put back (Undo) while no safety copy
/// can be made: held. Returns the rig and its sinks.
fn undo_held_by_the_safety_copy() -> (Rig, super::bank_links_fix16_tests::Sinks) {
    let (mut m, s, mut banks, ctx, mut acct) = rig();
    let k = rebuilt(&mut m);
    s.keep("Post", "/circles/heads", 200, heads(3));
    phase_c(&mut m, &mut banks, 3, Some(Difficulty::Low), None);
    let removed = banks[0].questions.remove(0);
    banks[0].modified += 5;
    s.keep_raw("Get", &version_path(3), v3(Some("High"), None));
    let who = olga_named(&s);
    frames(&mut m, &mut banks, &ctx, &mut acct, &who, 3);
    assert!(!m.edit.with_link(LOCAL, |l| l.inherit).unwrap());
    k.restore_fails.store(true, Ordering::SeqCst);
    banks[0].questions.insert(0, removed);
    banks[0].modified += 5;
    frames(&mut m, &mut banks, &ctx, &mut acct, &who, 3);
    assert_eq!(m.edit.status(LOCAL).unwrap().learning, Some(Learning::Held));
    assert_eq!(banks[0].questions[0].difficulty, Some(Difficulty::Low));
    ((m, s, banks, ctx, acct), k)
}

/// (p99_7 turned around) After a restart the held pass is still waited for: it runs (the safety copy can be made now), One takes the
/// circle's High, and the next send carries High, never her pre-read Low over the colleague's; no stale `at_read` is left.
#[test]
fn c99_m4_a_held_undo_pass_survives_a_restart() {
    let ((_m, s, mut banks, ctx, mut acct), k) = undo_held_by_the_safety_copy();
    let who = olga_named(&s);
    let mut m2 = restarted(k.log.clone());
    frames(&mut m2, &mut banks, &ctx, &mut acct, &who, 3);
    assert_eq!(banks[0].questions[0].difficulty, Some(Difficulty::High), "the pass ran after the restart");
    assert_eq!(sent_q(&mut m2, &banks, 0), json!("High"));
    assert_eq!(m2.edit.status(LOCAL).unwrap().learning, None);
    assert!(m2.edit.with_link(LOCAL, |l| l.at_read.is_none()).unwrap(), "no stale at_read");
}

/// The same, the restart before any safety copy can be made: still held, and Send is not offered.
#[test]
fn c99_m4_still_held_after_a_restart_send_waits() {
    let ((_m, s, mut banks, ctx, mut acct), k) = undo_held_by_the_safety_copy();
    let who = olga_named(&s);
    let mut m2 = restarted(k.log.clone());
    let restore_fails: super::browse::SharedRestore = Arc::new(FailingRestore);
    let log = k.log.clone();
    let media: super::publish::SharedMedia = Arc::new(std::collections::HashMap::<String, Vec<u8>>::new());
    m2.edit = CircleEditModel::new(super::model::Runner::Inline, Arc::new(Mem::default()), media, restore_fails, LinkLog::Memory(log)).with_retry(super::pool::RetryPolicy::none());
    m2.edit.set_clock(NOW);
    let strip = frames(&mut m2, &mut banks, &ctx, &mut acct, &who, 3);
    assert_eq!(banks[0].questions[0].difficulty, Some(Difficulty::Low));
    assert_eq!(m2.edit.status(LOCAL).unwrap().learning, Some(Learning::Held), "{strip}");
    assert!(!strip.contains("Send my changes"), "{strip}");
}

struct FailingRestore;
impl super::browse::RestoreSink for FailingRestore {
    fn save(&self, _banks: &[QuestionBank]) -> Result<String, String> {
        Err("No space left on device (os error 28)".into())
    }
}

/// (p99_8 turned around) A held Undo pass, and an unclear send of hers: Get latest waits for the pass before it makes its review
/// (it would call her pre-read Low hers), and says why.
#[test]
fn c99_m7_a_held_undo_pass_holds_get_latests_review() {
    let (mut m, s, mut banks, ctx, mut acct) = rig();
    let k = rebuilt(&mut m);
    s.keep("Post", "/circles/heads", 200, heads(3));
    phase_c(&mut m, &mut banks, 3, Some(Difficulty::Low), None);
    let removed = banks[0].questions.remove(0);
    banks[0].modified += 5;
    s.keep_raw("Get", &version_path(3), v3(Some("High"), None));
    let who = olga_named(&s);
    frames(&mut m, &mut banks, &ctx, &mut acct, &who, 3);
    let modified = banks[0].modified;
    m.edit.update_link(LOCAL, |l| {
        l.sent_unread = Some(super::circle_links::SentUnread { by: String::new(), modified });
        true
    });
    k.save_fails.store(true, Ordering::SeqCst);
    banks[0].questions.insert(0, removed);
    banks[0].modified += 5;
    frames(&mut m, &mut banks, &ctx, &mut acct, &who, 3);
    assert_eq!(m.edit.status(LOCAL).unwrap().learning, Some(Learning::Held));
    s.on("Post", &format!("{}/merge", put_path()), 200, merge_start(3, 4));
    s.keep_raw("Get", &put_path(), served_qs(4, vec![super::bank_links_fix11_tests::rated(QB, "Two", None)], false));
    assert!(m.edit.open_merge(LOCAL, owner()));
    m.edit.host = Some(EditHost { local_id: LOCAL.into(), back: "< Back to Unit 2 quiz".into() });
    let mut page = String::new();
    for _ in 0..30 {
        page = frames(&mut m, &mut banks, &ctx, &mut acct, &who, 1);
        std::thread::sleep(Duration::from_millis(5));
    }
    assert!(kinds(&m).is_none(), "no review while it is held: {page}");
    assert!(m.edit.merge_wait_note().is_some_and(|n| n.contains("couldn't save")), "{page}");
}

// ---- c99 m5: one safety copy per state of the banks, whatever the number of held banks -------------------------------------------

struct CountRestore(Arc<Mutex<usize>>);
impl super::browse::RestoreSink for CountRestore {
    fn save(&self, _banks: &[QuestionBank]) -> Result<String, String> {
        *self.0.lock().unwrap() += 1;
        Ok("backups/banks-before-difficulty-test.json".into())
    }
}

/// (p99_2 turned around) Three copies wait for their pass and the banks file can't be written: one safety copy at the first try, and
/// one more each minute she has changed something (never one per held bank); none while nothing changed.
#[test]
fn c99_m5_one_safety_copy_per_state_of_the_banks() {
    let n = Arc::new(Mutex::new(0usize));
    let media: super::publish::SharedMedia = Arc::new(std::collections::HashMap::<String, Vec<u8>>::new());
    let fails: BankSaver = Arc::new(|_b: &[QuestionBank]| Err("Permission denied (os error 13)".into()));
    let mut e = CircleEditModel::new(super::model::Runner::Inline, Arc::new(Mem::default()), media, Arc::new(CountRestore(n.clone())), LinkLog::memory()).with_retry(super::pool::RetryPolicy::none()).with_bank_saver(fails);
    e.set_clock(NOW);
    let mut banks = Vec::new();
    for (i, local) in [LOCAL, "local-bank-b19", "local-bank-c19"].iter().enumerate() {
        let (mut bank, mut l) = pending();
        bank.id = local.to_string();
        l.local_bank_id = local.to_string();
        l.circle_bank_id = hex(70 + i as u64);
        l.mark_bank(&bank);
        e.add_link(l).unwrap();
        banks.push(bank);
    }
    let (mut other, _) = linked(&[(QA, "Unlinked")]);
    other.id = "unlinked".into();
    banks.push(other);
    let out = signed_out();
    let t0 = now();
    e.frame(t0, &out);
    e.sync(&mut banks);
    assert_eq!(*n.lock().unwrap(), 1, "one for the banks as they are");
    for i in 1..=10u64 {
        banks[3].questions[0].stem = format!("Unlinked, edit {i}");
        banks[3].modified += 5;
        e.frame(t0 + Duration::from_secs(61 * i), &out);
        e.sync(&mut banks);
    }
    assert_eq!(*n.lock().unwrap(), 11, "one per state");
    e.frame(t0 + Duration::from_secs(61 * 11), &out);
    e.sync(&mut banks);
    assert_eq!(*n.lock().unwrap(), 11, "nothing changed: none");
}

// ---- c99 m3: a pass run again after a restart moves an unclear send's time with it -----------------------------------------------

/// (p99_4 turned around) The pass wrote the banks file, then the links file could not be written until she quit; she had an unclear
/// send. After a restart the pass is taken as done and the send's time moves with it: no changes after the send are made up.
#[test]
fn c99_m3_a_restart_moves_the_unclear_send_time_with_the_pass() {
    let dir = fresh_dir("m3");
    let saved: Saved = Arc::default();
    let mut e = dir_model(&dir, recording_then_blocking(&saved, &dir));
    let (bank, mut l) = pending();
    l.sent_unread = Some(super::circle_links::SentUnread { by: String::new(), modified: bank.modified });
    l.sent_record = Some(super::circle_links::SentRecord { modified: bank.modified, ..Default::default() });
    e.add_link(l).unwrap();
    let mut banks = vec![bank];
    let out = signed_out();
    e.frame(now(), &out);
    assert!(e.sync(&mut banks));
    assert_eq!(e.sent_unread_of(&banks[0]), Some(false));
    unblock_links(&dir);
    let mut e2 = dir_model(&dir, recording(&Arc::default()));
    e2.frame(now() + Duration::from_secs(600), &out);
    e2.sync(&mut banks);
    assert!(e2.with_link(LOCAL, |l| l.difficulty_news.is_some() && !l.inherit).unwrap());
    assert_eq!(e2.sent_unread_of(&banks[0]), Some(false), "no changes after the send are made up");
    assert_eq!(e2.with_link(LOCAL, |l| l.sent_record.as_ref().map(|r| r.modified)).flatten(), Some(banks[0].modified));
    let _ = std::fs::remove_dir_all(&dir);
}

/// The same, with an edit of hers after the pass before she quit: that edit is still a change after the send.
#[test]
fn c99_m3_her_edit_after_the_pass_is_still_unsent_after_a_restart() {
    let dir = fresh_dir("m3b");
    let saved: Saved = Arc::default();
    let mut e = dir_model(&dir, recording_then_blocking(&saved, &dir));
    let (bank, mut l) = pending();
    l.sent_unread = Some(super::circle_links::SentUnread { by: String::new(), modified: bank.modified });
    e.add_link(l).unwrap();
    let mut banks = vec![bank];
    let out = signed_out();
    e.frame(now(), &out);
    assert!(e.sync(&mut banks));
    banks[0].questions[0].stem = "One (mine)".into();
    banks[0].modified += 5;
    unblock_links(&dir);
    let mut e2 = dir_model(&dir, recording(&Arc::default()));
    e2.frame(now() + Duration::from_secs(600), &out);
    e2.sync(&mut banks);
    assert_eq!(e2.sent_unread_of(&banks[0]), Some(true));
    let _ = std::fs::remove_dir_all(&dir);
}

// ---- c99 m6, m8, m9 ----------------------------------------------------------------------------------------------------------------

/// A model on links kept in memory whose banks saves fail while `fail` is set.
fn failing_model(fail: Arc<std::sync::atomic::AtomicBool>, log: Arc<Mutex<Vec<CircleLink>>>) -> CircleEditModel {
    let saver: BankSaver = Arc::new(move |_b: &[QuestionBank]| if fail.load(Ordering::SeqCst) { Err("disk full".into()) } else { Ok(()) });
    let media: super::publish::SharedMedia = Arc::new(std::collections::HashMap::<String, Vec<u8>>::new());
    let mut e = CircleEditModel::new(super::model::Runner::Inline, Arc::new(Mem::default()), media, Arc::new(NoDiskRestore), LinkLog::Memory(log)).with_retry(super::pool::RetryPolicy::none()).with_bank_saver(saver);
    e.set_clock(NOW);
    e
}

/// (p99_3 turned around) During a hold she sets One to the circle's own difficulty herself: that is her choice, never listed as the
/// app's, and Put mine back is not offered for it (it would revert her choice); Two still takes the circle's.
#[test]
fn c99_m6_her_own_choice_of_the_circles_difficulty_during_a_hold_is_hers() {
    let fail = Arc::new(std::sync::atomic::AtomicBool::new(true));
    let mut e = failing_model(fail.clone(), Arc::default());
    let (bank, l) = pending();
    e.add_link(l).unwrap();
    let mut banks = vec![bank];
    let out = signed_out();
    let t0 = now();
    e.frame(t0, &out);
    e.sync(&mut banks);
    assert_eq!(e.status(LOCAL).unwrap().learning, Some(Learning::Held));
    banks[0].questions[0].difficulty = Some(Difficulty::High);
    banks[0].modified += 5;
    fail.store(false, Ordering::SeqCst);
    e.frame(t0 + Duration::from_secs(61), &out);
    e.sync(&mut banks);
    assert_eq!((banks[0].questions[0].difficulty, banks[0].questions[1].difficulty), (Some(Difficulty::High), None));
    let n = news_of(&mut e);
    assert!(!n.questions.iter().any(|q| q.id == banks[0].questions[0].id), "One is hers: {n:?}");
    assert!(n.questions.iter().any(|q| q.id == banks[0].questions[1].id), "{n:?}");
    let one = banks[0].questions[0].id.clone();
    assert!(!e.put_mine_back(&mut banks, LOCAL, &one));
    assert_eq!(banks[0].questions[0].difficulty, Some(Difficulty::High));
}

/// The same with a restart during the hold: the pass is not taken as done (Two still takes the circle's), and One is hers.
#[test]
fn c99_m6_after_a_restart_during_the_hold_her_choice_is_hers_and_the_pass_still_runs() {
    let fail = Arc::new(std::sync::atomic::AtomicBool::new(true));
    let log: Arc<Mutex<Vec<CircleLink>>> = Arc::default();
    let mut e = failing_model(fail.clone(), log.clone());
    let (bank, l) = pending();
    e.add_link(l).unwrap();
    let mut banks = vec![bank];
    let out = signed_out();
    e.frame(now(), &out);
    e.sync(&mut banks);
    assert_eq!(e.status(LOCAL).unwrap().learning, Some(Learning::Held));
    banks[0].questions[0].difficulty = Some(Difficulty::High);
    banks[0].modified += 5;
    let mut e2 = failing_model(Arc::new(std::sync::atomic::AtomicBool::new(false)), log.clone());
    e2.frame(now() + Duration::from_secs(600), &out);
    assert!(e2.sync(&mut banks));
    assert_eq!((banks[0].questions[0].difficulty, banks[0].questions[1].difficulty), (Some(Difficulty::High), None), "Two took the circle's");
    let n = news_of(&mut e2);
    assert_eq!(n.questions.iter().map(|q| q.id.clone()).collect::<Vec<_>>(), vec![banks[0].questions[1].id.clone()], "{n:?}");
}

/// (p99_5, p99_9 turned around) A non-bool `inherit` drops only that field (the link stays, nothing kept as `.bad`); a non-bool
/// `changed` (or `put_back`) drops only that field of one question, never the news; an `at_read` entry with no `difficulty` is not
/// known (dropped), never "none": that question takes the circle's.
#[test]
fn c99_m8_bad_fields_drop_only_themselves() {
    let dir = fresh_dir("m8");
    let (_, l) = linked(&[(QA, "One")]);
    let mut a = serde_json::to_value(&l).unwrap();
    a["inherit"] = json!("yes");
    let mut b = serde_json::to_value(&l).unwrap();
    b["local_bank_id"] = json!("other-bank");
    b["circle_bank_id"] = json!(hex(78));
    b["difficulty_news"] = json!({"version": 1, "questions": [{"id": "q1", "before": "Low", "after": "High", "put_back": "no"}, {"id": "q2", "before": "Low", "after": null, "put_back": false, "changed": "yes"}]});
    std::fs::write(dir.join("share-circles.json"), serde_json::to_vec(&json!({"links": [a, b]})).unwrap()).unwrap();
    let links = LinkLog::Dir(dir.clone()).load();
    assert_eq!(links.len(), 2, "{links:?}");
    assert!(!links[0].inherit);
    assert!(!dir.join("share-circles.json.bad").exists());
    let n = links[1].difficulty_news.as_ref().expect("the news is kept");
    assert_eq!(n.questions.len(), 2);
    assert!(!n.questions[0].put_back && !n.questions[1].changed);
    let _ = std::fs::remove_dir_all(&dir);
    // an at_read entry with no difficulty
    let dir = fresh_dir("m8b");
    let (bank, l) = pending();
    let mut v = serde_json::to_value(&l).unwrap();
    v["at_read"] = json!([{"id": bank.questions[0].id}, {"id": bank.questions[1].id, "difficulty": "Low"}]);
    std::fs::write(dir.join("share-circles.json"), serde_json::to_vec(&json!({"links": [v]})).unwrap()).unwrap();
    let links = LinkLog::Dir(dir.clone()).load();
    assert_eq!(links[0].at_read.as_ref().map(|a| a.len()), Some(1), "dropped, never 'none'");
    let mut e = dir_model(&dir, recording(&Arc::default()));
    let mut banks = vec![bank];
    e.frame(now(), &signed_out());
    e.sync(&mut banks);
    assert_eq!((banks[0].questions[0].difficulty, banks[0].questions[1].difficulty), (Some(Difficulty::High), None), "One takes the circle's");
    let _ = std::fs::remove_dir_all(&dir);
}

/// (p99_6 turned around) A colleague's version that happens to give One her old difficulty (she never put it back): "changed since",
/// never "yours ... was sent".
#[test]
fn c99_m9_a_colleagues_equal_difficulty_is_changed_since_not_hers_sent() {
    use super::circle_links::{DifficultyNews, NewsItem, NewsState, news_state};
    let (mut bank, mut l) = linked(&[(QA, "One")]);
    l.base_version = 4;
    l.base[0].snap.difficulty = Some(Some(Difficulty::Low));
    bank.questions[0].difficulty = Some(Difficulty::Low);
    let item = NewsItem { id: bank.questions[0].id.clone(), before: Some(Difficulty::Low), after: Some(Difficulty::High), put_back: false, changed: true };
    l.difficulty_news = Some(DifficultyNews { version: 3, questions: vec![item.clone()] });
    assert_eq!(news_state(&l, &item, &bank), NewsState::Changed(Some(Difficulty::Low)));
    // hers, put back and sent: still "was sent"
    let mine = NewsItem { put_back: true, ..item };
    assert_eq!(news_state(&l, &mine, &bank), NewsState::Sent);
}

// ---- c100 minors ------------------------------------------------------------------------------------------------------------------

/// The raw text and colour of each drawn text, `width` wide.
fn drawn(width: f32, draw: &mut dyn FnMut(&mut egui::Ui)) -> Vec<(String, egui::Rect, egui::Color32)> {
    let ctx = ctx();
    let mut all = Vec::new();
    for _ in 0..2 {
        let input = egui::RawInput { screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(width, 6000.0))), ..Default::default() };
        let out = ctx.run_ui(input, |ui| {
            egui::CentralPanel::default().show(ui, |ui| draw(ui));
        });
        all = out
            .shapes
            .iter()
            .filter_map(|s| if let egui::epaint::Shape::Text(t) = &s.shape { Some((t.galley.text().to_string(), t.galley.rect.translate(t.pos.to_vec2()), t.galley.job.sections.first().map_or(t.fallback_color, |x| x.format.color))) } else { None })
            .collect();
        out.drop_without_applying_deltas();
    }
    all
}

/// (minor 1) Not kept: the strip never calls the version "the circle's newest" and "no longer kept" in one breath.
#[test]
fn c100_m1_not_kept_says_it_once_without_contradiction() {
    let (mut m, s, banks, _ctx, _acct) = not_kept_rig();
    let v = strip_view(&mut m, &banks, &olga_named(&s));
    assert!(v.line.contains("Your copy last matched version 3, but the circle no longer keeps it"), "{}", v.line);
    assert!(!v.line.contains("newest version, but the circle no longer keeps"), "{}", v.line);
}

/// (minor 2) Sending off: the strip warns as the badge and the circle page's row do (not kept, and held back for review).
#[test]
fn c100_m2_sending_off_warns_on_the_strip_too() {
    let (mut m, s, banks, _ctx, _acct) = not_kept_rig();
    let v = strip_view(&mut m, &banks, &olga_named(&s));
    assert!(v.warn, "{v:?}");
    assert!(badge_warns(&mut m, &banks));
    let (mut m, s, mut banks, ctx, mut acct) = rig();
    s.keep("Post", "/circles/heads", 200, heads(3));
    phase_c(&mut m, &mut banks, 3, Some(Difficulty::Low), None);
    s.keep("Get", &version_path(3), 404, json!({"error": "version_withheld"}));
    let who = olga_named(&s);
    frames(&mut m, &mut banks, &ctx, &mut acct, &who, 3);
    let v = strip_view(&mut m, &banks, &who);
    assert!(v.warn && badge_warns(&mut m, &banks), "{v:?}");
}

/// (minor 2) A red "1 unsent change" says on hover why it is red; "newer" (and "newer version") says what it means; a bank put back
/// from a safety copy has words on its badge, and a hint.
#[test]
fn c100_m2_badge_hints_and_the_put_back_badge() {
    use super::bank_links::note_hint_for;
    assert!(note_hint_for("1 unsent change", true).is_some_and(|h| h.contains("Sending is turned off")), "{:?}", note_hint_for("1 unsent change", true));
    assert!(note_hint_for("newer", false).is_some_and(|h| h.contains("newer version")));
    assert!(note_hint_for("newer version · 1 unsent", false).is_some_and(|h| h.contains("newer version")));
    let (mut m, s, mut banks, ctx, mut acct) = rig();
    make_stale(&mut m, LOCAL);
    m.edit.sync(&mut banks);
    frames(&mut m, &mut banks, &ctx, &mut acct, &olga_named(&s), 2);
    let note = badge(&mut m, &banks[0].clone());
    assert!(note.contains("put back from a safety copy"), "{note:?}");
    assert!(badge_warns(&mut m, &banks));
    assert!(note_hint_for(&note, true).is_some_and(|h| h.contains("safety copy")));
    assert_eq!(super::bank_links::short_note("put back from a safety copy"), "put back");
}

/// (minor 5) The news retires itself: once every row is sent or settled, the badge no longer says "difficulty updated" and the list
/// no longer reads as fresh news.
#[test]
fn c100_m5_the_news_retires_itself() {
    let ((mut m, s, mut banks, ctx, mut acct), _k) = with_news_counted();
    let who = olga_named(&s);
    assert!(badge(&mut m, &banks[0].clone()).contains("difficulty updated"));
    click(&ctx, &mut m, &mut banks, &mut acct, &who, "Put mine back", 1);
    let bank = banks[0].clone();
    m.edit.update_link(LOCAL, |l| {
        let sent = super::circles_api::SentBank { version: 4, version_id: hex(4), qids: vec![QA.into(), QB.into()], ..Default::default() };
        *l = super::circle_links::after_send(l, &bank, &sent);
        true
    });
    s.keep("Post", "/circles/heads", 200, heads(4));
    let strip = frames(&mut m, &mut banks, &ctx, &mut acct, &who, 3);
    assert!(!badge(&mut m, &banks[0].clone()).contains("difficulty updated"), "{}", badge(&mut m, &banks[0].clone()));
    assert!(strip.contains("Earlier, verSHion gave 2 questions the circle's difficulty") && !strip.contains("in place of yours"), "{strip}");
    assert!(strip.contains("was sent"), "the rows stay until dismissed: {strip}");
}

/// (minor 9) The Send page says a version can be restored later only to someone who can restore (the owner, the bank's author).
#[test]
fn c100_m9_restore_later_only_where_true() {
    use super::circles_api::Role;
    for (perm, says) in [(owner(), true), (Perm { role: Role::Editor, is_author: false, closed: false }, false), (Perm { role: Role::Editor, is_author: true, closed: false }, true)] {
        let (mut m, s, mut banks, ctx, mut acct) = rig();
        let who = olga_named(&s);
        frames(&mut m, &mut banks, &ctx, &mut acct, &who, 2);
        assert!(m.edit.open_send(LOCAL, perm));
        super::bank_links_fix11_tests::host_a(&mut m);
        let page = frames(&mut m, &mut banks, &ctx, &mut acct, &who, 2);
        assert!(page.contains("This adds a new version to the circle's bank."), "{page}");
        assert_eq!(page.contains("restore one later"), says, "{perm:?}: {page}");
    }
}

/// The circle's page with A's bank on it, as olga (owner) sees it.
fn circle_page(m: &mut CirclesModel, s: &Arc<Script>, banks: &mut Vec<QuestionBank>, who: &AccountSnapshot) {
    let bank = json!([{"id": BID, "title": "Unit 2 quiz", "subject": "Algebra", "license": "CC-BY-4.0", "author_name": "Olga Teacher", "is_author": true, "head_version": 3, "question_count": banks[0].questions.len(), "created_at": NOW - DAY, "updated_at": NOW - DAY}]);
    s.keep("Get", &format!("/circles/{CID}"), 200, detail("owner", json!({"banks": bank})));
    s.keep("Get", &format!("/circles/{CID}/invites"), 200, json!({"invites": [], "next_cursor": null}));
    s.keep("Get", &format!("/circles/{CID}/activity"), 200, json!({"activity": [], "next_before": null}));
    let mut b = browse();
    m.open_list();
    m.frame(now(), who);
    m.sync(banks, &mut b);
    m.open_circle(CID);
    for _ in 0..4 {
        m.frame(now(), who);
        m.sync(banks, &mut b);
    }
}

/// (minor 10) Removing a bank from its row on the circle's page: the question has its heading, and "Remove..." is not offered again
/// while it is asked.
#[test]
fn c100_m10_the_row_remove_question_has_a_heading() {
    let (mut m, s, mut banks, _ctx, _acct) = rig();
    let who = olga_named(&s);
    circle_page(&mut m, &s, &mut banks, &who);
    let d = drawn(1280.0, &mut |ui| {
        super::circles_ui::body_of(ui, &mut m);
    });
    // (the member rows have a "Remove..." of their own: only the bank row's is at issue here)
    let bank_row_removes = |d: &Vec<(String, egui::Rect, egui::Color32)>| {
        let members = d.iter().find(|t| t.0 == "Members").expect("the Members heading").1.top();
        d.iter().filter(|t| t.0 == "Remove..." && t.1.top() < members).count()
    };
    assert_eq!(bank_row_removes(&d), 1, "{d:?}");
    m.ask_remove_on(BID);
    let d = drawn(1280.0, &mut |ui| {
        super::circles_ui::body_of(ui, &mut m);
    });
    let head = d.iter().find(|t| t.0 == super::circles_text::REMOVE_BANK_HEAD).unwrap_or_else(|| panic!("{d:?}"));
    let body = d.iter().find(|t| t.0 == super::circles_text::REMOVE_BANK).unwrap_or_else(|| panic!("{d:?}"));
    assert!(head.1.bottom() <= body.1.top(), "{head:?} {body:?}");
    assert_eq!(bank_row_removes(&d), 0, "not offered again while it is asked: {d:?}");
}

/// (minor 12) Leftover words: no "merge" in what the circle's screens say, no "REPLACES" in capitals, no "copy that file over
/// banks.json", and the invite panel without "(UTC)" or "redeemed".
#[test]
fn c100_m12_leftover_words() {
    let said = |src: &str| -> Vec<String> {
        let mut out = Vec::new();
        for line in src.lines().filter(|l| !l.trim_start().starts_with("//")) {
            let mut rest = line;
            while let Some(a) = rest.find('"') {
                let after = &rest[a + 1..];
                let Some(b) = after.find('"') else { break };
                let lit = &after[..b];
                if lit.contains(' ') {
                    out.push(lit.to_string());
                }
                rest = &after[b + 1..];
            }
        }
        out
    };
    for (name, src) in [("circle_edit_ui.rs", include_str!("circle_edit_ui.rs")), ("circle_edit.rs", include_str!("circle_edit.rs"))] {
        for lit in said(src) {
            assert!(!lit.to_lowercase().contains("merg"), "{name}: {lit}");
            assert!(!lit.contains("REPLACES"), "{name}: {lit}");
        }
    }
    for (name, src) in [("circle_edit_ui.rs", include_str!("circle_edit_ui.rs")), ("browse_ui.rs", include_str!("browse_ui.rs"))] {
        assert!(!src.contains("copy that file over banks.json"), "{name}");
    }
    assert!(!super::circles_text::code_until(1_792_972_800_000).contains("UTC"));
    assert!(!super::circles_text::EMAIL_NEEDS_ACCOUNT.contains("redeemed"));
    // what Get latest's review says on a circle screen
    assert!(!super::circles_text::merge_words("The pool's version changed while you were reviewing. Nothing was applied; start the merge again.").contains("merge"));
}

// ---- item 7: tests for the mutants that survived round 18 ----------------------------------------------------------------

/// (survivor m24) A news row that turns "changed since" is written with the links as it happens, not only kept in memory: settling
/// the news saves the file, so a restart shows the same thing.
#[test]
fn s19_m24_settling_the_news_writes_the_links_file() {
    let dir = fresh_dir("m24");
    let saved: Saved = Arc::default();
    let mut e = dir_model(&dir, recording(&saved));
    let (bank, l) = pending();
    e.add_link(l).unwrap();
    let mut banks = vec![bank];
    let out = signed_out();
    let t0 = now();
    e.frame(t0, &out);
    assert!(e.sync(&mut banks), "the pass ran");
    let two = banks[0].questions[1].id.clone();
    let row_in_file = |dir: &std::path::Path, id: &str| -> bool {
        let file = LinkLog::Dir(dir.to_path_buf()).load();
        file[0].difficulty_news.as_ref().and_then(|n| n.questions.iter().find(|q| q.id == id)).map(|q| q.changed).unwrap_or(false)
    };
    assert!(!row_in_file(&dir, &two), "not changed yet");
    // she rates Two after the pass: the row is "changed since", and settled into the links file
    banks[0].questions[1].difficulty = Some(Difficulty::Low);
    banks[0].modified += 5;
    e.sync(&mut banks);
    assert!(row_in_file(&dir, &two), "in the file, not only in memory");
    let _ = std::fs::remove_dir_all(&dir);
}

/// (survivor m12) A pass that already ran (its banks file written, its links file not) is run again after a restart:
/// the question it gave the circle's difficulty is in the news again, in the same pass that moves another one.
#[test]
fn s19_m12_a_restart_keeps_the_news_of_a_question_the_pass_already_moved() {
    let dir = fresh_dir("m12");
    let saved: Saved = Arc::default();
    let mut e = dir_model(&dir, recording_then_blocking(&saved, &dir));
    let (mut bank, mut l) = linked(&[(QA, "One"), (QB, "Two"), (QC, "Three")]);
    for q in &mut bank.questions {
        q.difficulty = Some(Difficulty::Low);
    }
    l.base[0].snap.difficulty = Some(Some(Difficulty::High));
    l.base[1].snap.difficulty = Some(None);
    l.base[2].snap.difficulty = Some(Some(Difficulty::High));
    l.difficulty_read = Some(l.base_version);
    l.inherit = true;
    l.at_read = None;
    // Two and Three deleted before the read
    let two = bank.questions.remove(1);
    let three = bank.questions.remove(1);
    l.mark_bank(&bank);
    e.add_link(l).unwrap();
    let mut banks = vec![bank];
    let out = signed_out();
    let t0 = now();
    // the whole pass: One takes the circle's; its links file is written up to the banks, then can't be written
    e.frame(t0, &out);
    assert!(e.sync(&mut banks));
    assert_eq!(banks[0].questions[0].difficulty, Some(Difficulty::High));
    assert_eq!(banks[0].questions.len(), 1);
    // Three put back: the links file can be written for this pass's first write, and not for its last
    unblock_links(&dir);
    banks[0].questions.push(three);
    e.frame(t0 + Duration::from_secs(61), &out);
    assert!(e.sync(&mut banks));
    assert_eq!(banks[0].questions[1].difficulty, Some(Difficulty::High), "Three took the circle's");
    // Two put back too, then a restart: Three (already done) and Two (still to move) are both in the news
    unblock_links(&dir);
    banks[0].questions.push(two);
    let mut e2 = dir_model(&dir, recording(&Arc::default()));
    e2.frame(t0 + Duration::from_secs(600), &out);
    assert!(e2.sync(&mut banks));
    assert_eq!((banks[0].questions[1].difficulty, banks[0].questions[2].difficulty), (Some(Difficulty::High), None), "{:?}", banks[0].questions.iter().map(|q| q.difficulty).collect::<Vec<_>>());
    let n = news_of(&mut e2);
    // (One's row is in the file too: the first pass's last write failed, but the second pass's first write carried it)
    assert!(n.questions.iter().any(|q| q.id == banks[0].questions[1].id && q.before == Some(Difficulty::Low) && q.after == Some(Difficulty::High)), "Three, already done: {n:?}");
    assert!(n.questions.iter().any(|q| q.id == banks[0].questions[2].id && q.before == Some(Difficulty::Low) && q.after.is_none()), "Two, still to move: {n:?}");
    let _ = std::fs::remove_dir_all(&dir);
}

/// (survivor m21) Signed out, a send record that carries no account tag is still used when the link was last seen by
/// another account: the question it carries keeps her difficulty, and the one it does not takes the circle's.
#[test]
fn s19_m21_a_send_record_with_no_account_tag_is_still_used() {
    let dir = fresh_dir("m21");
    let (bank, mut l) = pending();
    let mut one = bank.questions[0].clone();
    one.difficulty = Some(Difficulty::Medium);
    l.sent_record = Some(super::circle_links::SentRecord { by: String::new(), questions: vec![super::circle_links::SentQuestion { qid: l.qids[0].qid.clone(), q: one }], ..Default::default() });
    l.role_seen_by = Some("another-account-on-this-computer".into());
    let mut e = dir_model(&dir, recording(&Arc::default()));
    e.add_link(l).unwrap();
    let mut banks = vec![bank];
    e.frame(now(), &signed_out());
    assert!(e.sync(&mut banks));
    assert_eq!(banks[0].questions[0].difficulty, Some(Difficulty::Low), "the record's Medium is not the circle's High: hers stays");
    assert_eq!(banks[0].questions[1].difficulty, None, "Two takes the circle's");
    let n = news_of(&mut e);
    assert_eq!(n.questions.len(), 1, "{n:?}");
    assert_eq!(n.questions[0].id, banks[0].questions[1].id, "{n:?}");
    let _ = std::fs::remove_dir_all(&dir);
}

/// (survivor m17, equivalent: see the report) A put-back pass over a question whose place in `at_read` is already kept
/// leaves that place as it was in the file, and the question still moves. The guard `have.len() > known` can not be
/// seen from here: the caller only waits for questions the bank still has, so `have` either grows or is already the
/// value kept, and writing it again writes the same thing.
#[test]
fn s19_m17_a_put_back_pass_keeps_the_at_read_it_found() {
    let dir = fresh_dir("m17");
    let saved: Saved = Arc::default();
    let mut e = dir_model(&dir, recording_then_blocking(&saved, &dir));
    let (mut bank, mut l) = linked(&[(QA, "One")]);
    bank.questions[0].difficulty = Some(Difficulty::Low);
    l.base[0].snap.difficulty = Some(Some(Difficulty::High));
    l.difficulty_read = Some(l.base_version);
    l.inherit = false;
    // One is to be taken again, and the bank as the read found it is already kept
    l.missing_at_read = Some((l.base_version, vec![bank.questions[0].id.clone()]));
    l.at_read = Some(vec![super::circle_links::AtRead { id: bank.questions[0].id.clone(), difficulty: Some(Difficulty::Low) }]);
    l.mark_bank(&bank);
    e.add_link(l).unwrap();
    let mut banks = vec![bank];
    e.frame(now(), &signed_out());
    assert!(e.sync(&mut banks));
    assert_eq!(banks[0].questions[0].difficulty, Some(Difficulty::High), "One moved");
    // the links file was written before the banks file: the place kept for One is still there, unchanged
    let file = LinkLog::Dir(dir.clone()).load();
    let kept = file[0].at_read.clone().expect("still kept");
    assert_eq!(kept.len(), 1, "{kept:?}");
    assert_eq!(kept[0].difficulty, Some(Difficulty::Low), "{kept:?}");
    let _ = std::fs::remove_dir_all(&dir);
}

/// (survivor m61) "Put all of mine back" is offered only when more than one question does: one of three does not
/// offer it, and one "Put mine back" is drawn.
#[test]
fn s19_m61_put_all_is_not_offered_for_one_question() {
    let (mut m, s, mut banks, _ctx, mut acct) = many_news(3);
    let who = olga_named(&s);
    for q in banks[0].questions.iter_mut().take(2) {
        q.difficulty = Some(Difficulty::Low);
    }
    let d = drawn(1280.0, &mut |ui| {
        let mut b = browse();
        super::bank_links_ui::strip_with(ui, &mut m, &mut banks, &mut b, &mut acct, &who, now(), NOW, LOCAL);
    });
    assert_eq!(d.iter().filter(|t| t.0 == "Put mine back").count(), 1, "{d:?}");
    assert!(!d.iter().any(|t| t.0 == "Put all of mine back"), "{d:?}");
}

/// (survivor m80) After "Use my bank as it is" a partial count never says a difficulty can't be counted: the next send
/// includes them, and that is what is said.
#[test]
fn s19_m80_use_my_bank_as_it_is_never_says_difficulties_cannot_be_counted() {
    let (bank, mut l) = linked(&[(QA, "One"), (QB, "Two")]);
    for b in &mut l.base {
        b.snap.difficulty = None;
    }
    l.difficulty_read = Some(l.base_version);
    l.mark_bank(&bank);
    l.as_is = true;
    let mut m = edit_model();
    m.add_link(l.clone()).unwrap();
    let st = m.status_of(&bank).unwrap();
    assert!(st.changes.partial() && st.changes.total() == 0, "{:?}", st.changes);
    let lines = super::circle_edit_ui::standing(&l, Some(&st), false, super::circles_api::Role::Owner);
    assert!(lines.iter().any(|x| x.contains("Your next send makes the circle's newest version match this bank, difficulties included.")), "{lines:?}");
    assert!(!lines.iter().any(|x| x.contains("can't be counted")), "{lines:?}");
}

