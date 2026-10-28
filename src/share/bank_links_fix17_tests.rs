//! Phase D1, round 17 (critics 95 and 96): each test written first against the code it pins. The owner's rule stays (after one
//! successful read of the circle's version the copy takes the circle's difficulty; a later deliberate change of hers
//! supersedes): her change made while the difficulties wait to be saved is kept, in the session and after a restart; what the
//! circle's difficulty replaced is kept on the link until she dismisses it, with "Put mine back" per question; "Use my bank as
//! it is" has its own page, and on a link an earlier build wrote it leaves the base's difficulties unknown.

use std::sync::atomic::Ordering;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use serde_json::json;

use super::bank_links::StripButton;
use super::bank_links_fix11_tests::{host_a, rated, rig, version_path, version_reads};
use super::bank_links_fix15_tests::phase_c;
use super::bank_links_fix16_tests::{quiet, rebuilt, sent_q, v3};
use super::bank_links_fix4_tests::*;
use super::bank_links_fix8_tests::*;
use super::bank_links_fix9_tests::*;
use super::bank_links_fix_tests::*;
use super::bank_links_tests::*;
use super::circle_edit::*;
use super::circle_links::{CircleLink, LinkLog};
use super::circles::CirclesModel;
use super::circles_tests::*;
use super::merge::RowKind;
use super::publish::AccountSnapshot;
use crate::model::{Difficulty, QuestionBank};

fn news(m: &mut CirclesModel) -> Option<super::circle_links::DifficultyNews> {
    m.edit.with_link(LOCAL, |l| l.difficulty_news.clone()).flatten()
}

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

/// A link the round-12 build wrote: no record of a read, its base difficulties filled from the bank (her unsent Low on One).
fn round12(m: &mut CirclesModel, banks: &mut [QuestionBank]) {
    phase_c(m, banks, 3, Some(Difficulty::Low), None);
    let b = banks[0].clone();
    m.edit.update_link(LOCAL, |l| {
        for (x, q) in l.base.iter_mut().zip(&b.questions) {
            x.snap.difficulty = Some(q.difficulty);
        }
        true
    });
}

// ---- c95 I1: her change made while the difficulties wait is kept ----------------------------------------------------------------

/// (p95_2 turned around) The read lands, the restore point can't be saved (held). She then rates One Medium herself. When it is
/// tried again, One keeps her Medium (her later change), Two (untouched) takes the circle's, and the news names only Two.
#[test]
fn c95_i1_a_change_made_while_held_is_kept_when_it_is_tried_again() {
    let (mut m, s, mut banks, ctx, mut acct) = rig();
    let k = rebuilt(&mut m);
    s.keep("Post", "/circles/heads", 200, heads(3));
    phase_c(&mut m, &mut banks, 3, Some(Difficulty::Low), Some(Difficulty::Low));
    k.restore_fails.store(true, Ordering::SeqCst);
    s.keep_raw("Get", &version_path(3), v3(Some("High"), None));
    let who = olga_named(&s);
    frames(&mut m, &mut banks, &ctx, &mut acct, &who, 3);
    assert_eq!(m.edit.status(LOCAL).unwrap().learning, Some(Learning::Held));
    // the bank as the first pass found it is on the link (the links file)
    let at = k.log.lock().unwrap()[0].at_read.clone().expect("recorded");
    assert_eq!(at.iter().map(|a| a.difficulty).collect::<Vec<_>>(), vec![Some(Difficulty::Low), Some(Difficulty::Low)]);
    banks[0].questions[0].difficulty = Some(Difficulty::Medium);
    banks[0].modified += 5;
    frames(&mut m, &mut banks, &ctx, &mut acct, &who, 2);
    k.restore_fails.store(false, Ordering::SeqCst);
    for _ in 0..3 {
        page_frame(&mut m, &mut banks, &mut acct, &who, now() + Duration::from_secs(61));
    }
    assert_eq!(banks[0].questions[0].difficulty, Some(Difficulty::Medium), "her later change stays");
    assert_eq!(banks[0].questions[1].difficulty, None, "the question she did not touch takes the circle's");
    assert_eq!(changes(&mut m, &banks[0].clone()), (0, 1, 0));
    assert_eq!(sent_q(&mut m, &banks, 0), json!("Medium"));
    let n = news(&mut m).expect("news");
    assert_eq!(n.questions.len(), 1, "{n:?}");
    assert_eq!((n.questions[0].id.clone(), n.questions[0].before, n.questions[0].after), (banks[0].questions[1].id.clone(), Some(Difficulty::Low), None));
    assert!(m.edit.with_link(LOCAL, |l| l.at_read.is_none() && !l.inherit).unwrap());
}

/// (p95_2b turned around) The same across a restart: the links file says what the bank was at the read, so the first pass after
/// the restart keeps her Medium.
#[test]
fn c95_i1_after_a_restart_a_change_made_while_held_is_kept() {
    let (mut m, s, mut banks, ctx, mut acct) = rig();
    let k = rebuilt(&mut m);
    s.keep("Post", "/circles/heads", 200, heads(3));
    phase_c(&mut m, &mut banks, 3, Some(Difficulty::Low), Some(Difficulty::Low));
    k.restore_fails.store(true, Ordering::SeqCst);
    s.keep_raw("Get", &version_path(3), v3(Some("High"), None));
    let who = olga_named(&s);
    frames(&mut m, &mut banks, &ctx, &mut acct, &who, 3);
    assert!(k.log.lock().unwrap()[0].inherit);
    banks[0].questions[0].difficulty = Some(Difficulty::Medium);
    banks[0].modified += 5;
    // the app quits; next start
    let mut m = restarted(k.log.clone());
    m.edit.frame(now(), &who);
    assert!(m.edit.sync(&mut banks));
    assert_eq!(banks[0].questions[0].difficulty, Some(Difficulty::Medium), "her Medium, made after the read, stays");
    assert_eq!(banks[0].questions[1].difficulty, None);
}

// ---- c95 I2: "Use my bank as it is" on a round-12 link ------------------------------------------------------------------------

/// (p95_1 turned around) A round-12 link put back from a saved copy: "Use my bank as it is" leaves the base's difficulties
/// unknown (never the bank's taken for the circle's): the count is partial, Send is offered and carries her Low, and Get latest
/// asks about One (a Conflict), with no "In version 3: Easy".
#[test]
fn c95_i2_round12_use_as_is_leaves_the_base_difficulties_unknown() {
    let (mut m, s, mut banks, ctx, mut acct) = rig();
    s.keep("Post", "/circles/heads", 200, heads(3));
    round12(&mut m, &mut banks);
    make_stale(&mut m, LOCAL);
    s.on("Get", &version_path(3), 503, json!({"error": "unavailable"}));
    s.keep_raw("Get", &version_path(3), v3(None, None));
    treat_in_step(&mut m, &s, &mut banks, LOCAL, BID, false);
    let who = olga_named(&s);
    let strip = frames(&mut m, &mut banks, &ctx, &mut acct, &who, 3);
    assert_eq!(m.edit.with_link(LOCAL, |l| (l.difficulty_read, l.base[0].snap.difficulty)).unwrap(), (Some(3), None));
    let c = m.edit.status_of(&banks[0].clone()).unwrap().changes;
    assert!(c.partial(), "{c:?}");
    assert!(!strip.contains("up to date") && strip.contains("Send my changes"), "{strip}");
    assert!(m.edit.open_send(LOCAL, owner()));
    m.edit.close();
    assert_eq!(sent_q(&mut m, &banks, 0), json!("Low"));
    let page = get_latest(&mut m, &s, &mut banks, &ctx, &mut acct, served_qs(4, vec![rated(QA, "One", None), rated(QB, "Two", None)], false));
    assert!(kinds(&m).unwrap().contains(&RowKind::Conflict), "{page}");
    assert!(!page.contains("In version 3: Easy"), "{page}");
}

/// (p95_1b turned around) The same not stale, version 3 no longer kept: the strip after "Use my bank as it is" is not "up to
/// date", says the difficulty can't be counted, and offers Send.
#[test]
fn c95_i2_round12_not_kept_use_as_is_is_not_up_to_date() {
    let (mut m, s, mut banks, ctx, mut acct) = rig();
    s.keep("Post", "/circles/heads", 200, heads(3));
    round12(&mut m, &mut banks);
    super::bank_links_fix12_tests::not_kept(&s, 3);
    let who = olga_named(&s);
    let strip = frames(&mut m, &mut banks, &ctx, &mut acct, &who, 3);
    assert!(strip.contains("Use my bank as it is..."), "{strip}");
    assert!(m.edit.open_merge_to_rebase(LOCAL, owner()));
    host_a(&mut m);
    frames(&mut m, &mut banks, &ctx, &mut acct, &who, 1);
    m.edit.rebaseline();
    m.edit.sync(&mut banks);
    m.edit.close();
    let strip = frames(&mut m, &mut banks, &ctx, &mut acct, &who, 3);
    // (round 18, critic 98 minor 5) nothing contradicts "difficulties included"
    assert!(!strip.contains("up to date") && !strip.contains("can't be counted") && strip.contains("Send my changes"), "{strip}");
    assert_eq!(sent_q(&mut m, &banks, 0), json!("Low"));
}

// ---- c96 I1: what the circle's difficulty replaced, kept until dismissed -------------------------------------------------------

/// The news is on the link: the badge says "difficulty updated", the strip names each question with before -> after (also a
/// difficulty the circle's version has none of), says plainly hers was replaced, offers "Put mine back", and names no .json file
/// or merge. It is still there after a restart.
#[test]
fn c96_i1_the_news_is_kept_on_the_link_and_drawn_on_the_badge_and_strip() {
    let (mut m, s, mut banks, ctx, mut acct) = rig();
    let k = rebuilt(&mut m);
    s.keep("Post", "/circles/heads", 200, heads(3));
    phase_c(&mut m, &mut banks, 3, Some(Difficulty::Low), Some(Difficulty::Low));
    s.keep_raw("Get", &version_path(3), v3(Some("High"), None));
    let who = olga_named(&s);
    let strip = frames(&mut m, &mut banks, &ctx, &mut acct, &who, 3);
    assert_eq!(banks[0].questions[0].difficulty, Some(Difficulty::High));
    let n = k.log.lock().unwrap()[0].difficulty_news.clone().expect("kept in the links file");
    assert_eq!(n.version, 3);
    assert_eq!(n.questions.iter().map(|q| (q.before, q.after)).collect::<Vec<_>>(), vec![(Some(Difficulty::Low), Some(Difficulty::High)), (Some(Difficulty::Low), None)]);
    assert_eq!(badge(&mut m, &banks[0].clone()), "difficulty updated");
    assert!(strip.contains("verSHion gave 2 questions the circle's difficulty from its version 3, in place of yours"), "{strip}");
    assert!(strip.contains("Question 1, \"One\": Easy -> Hard") && strip.contains("Question 2, \"Two\": Easy -> no difficulty"), "{strip}");
    assert_eq!(strip.lines().filter(|l| *l == "Put mine back").count(), 2, "{strip}");
    assert!(!strip.contains(".json") && !strip.contains("merge") && !strip.to_lowercase().contains("rating"), "{strip}");
    // after a restart
    let mut m = restarted(k.log.clone());
    let strip = frames(&mut m, &mut banks, &ctx, &mut acct, &who, 2);
    assert!(strip.contains("Question 1, \"One\": Easy -> Hard"), "{strip}");
    assert_eq!(badge(&mut m, &banks[0].clone()), "difficulty updated");
}

/// "Put mine back" on Two: her Easy comes back as an ordinary change she has not sent (counted, sent), the bank is to be saved,
/// and nothing takes the circle's again. "Dismiss" then clears the news from the strip, the badge and the links file.
#[test]
fn c96_i1_put_mine_back_is_an_unsent_change_and_dismiss_clears_the_news() {
    let (mut m, s, mut banks, ctx, mut acct) = rig();
    let k = rebuilt(&mut m);
    s.keep("Post", "/circles/heads", 200, heads(3));
    phase_c(&mut m, &mut banks, 3, Some(Difficulty::Low), Some(Difficulty::Low));
    s.keep_raw("Get", &version_path(3), v3(Some("High"), None));
    let who = olga_named(&s);
    frames(&mut m, &mut banks, &ctx, &mut acct, &who, 3);
    let before = banks[0].modified;
    assert!(click(&ctx, &mut m, &mut banks, &mut acct, &who, "Put mine back", 1), "the bank is saved");
    assert_eq!(banks[0].questions[1].difficulty, Some(Difficulty::Low));
    assert!(banks[0].modified > before);
    assert_eq!(changes(&mut m, &banks[0].clone()), (0, 1, 0));
    assert_eq!(sent_q(&mut m, &banks, 1), json!("Low"));
    let strip = frames(&mut m, &mut banks, &ctx, &mut acct, &who, 2);
    assert!(strip.contains("Question 2, \"Two\": yours (Easy) is back, not sent yet") && strip.lines().filter(|l| *l == "Put mine back").count() == 1, "{strip}");
    assert!(k.log.lock().unwrap()[0].difficulty_news.as_ref().unwrap().questions[1].put_back);
    // a minute later nothing takes the circle's again
    for _ in 0..3 {
        page_frame(&mut m, &mut banks, &mut acct, &who, now() + Duration::from_secs(61));
    }
    assert_eq!(banks[0].questions[1].difficulty, Some(Difficulty::Low));
    // the open question editor on Two is told
    let mut d = crate::qedit::Draft::from_question(LOCAL.into(), &banks[0].questions[0]);
    assert_eq!(d.difficulty, Some(Difficulty::High));
    let mut b = browse();
    super::circles_phasec_tests::click_text(&ctx, 1280.0, "Put mine back", 0, &mut |ui| {
        super::bank_links_ui::strip_for_editor(ui, &mut m, &mut banks, &mut b, &mut acct, &who, now(), NOW, LOCAL, Some(&mut d));
    });
    assert_eq!(d.difficulty, Some(Difficulty::Low), "the draft takes the difficulty put back");
    frames(&mut m, &mut banks, &ctx, &mut acct, &who, 1);
    click(&ctx, &mut m, &mut banks, &mut acct, &who, "Dismiss", 0);
    let strip = frames(&mut m, &mut banks, &ctx, &mut acct, &who, 2);
    assert!(news(&mut m).is_none() && k.log.lock().unwrap()[0].difficulty_news.is_none());
    assert!(!strip.contains("the circle's difficulty from its version"), "{strip}");
    assert_ne!(badge(&mut m, &banks[0].clone()), "difficulty updated");
}

/// (p96_1 turned around) Bank B's difficulty is replaced while she is on bank A's page: B's badge says so at once, and after a
/// restart B's strip still lists it.
#[test]
fn c96_i1_another_banks_news_waits_on_its_badge_and_strip_across_a_restart() {
    let (mut m, s, mut banks) = two_at_three();
    let ctx = ctx();
    let mut acct = FakeAccount { open: true, ..FakeAccount::default() };
    s.keep("Get", &format!("/circles/{CID}"), 200, detail("owner", json!({})));
    let who = olga_named(&s);
    banks[1].questions[0].difficulty = Some(Difficulty::Low);
    let b = banks[1].clone();
    let bv = m.edit.with_link(LOCAL_B, |l| l.base_version).unwrap();
    m.edit.update_link(LOCAL_B, |l| {
        for q in &mut l.base {
            q.snap.difficulty = None;
        }
        l.difficulty_read = None;
        l.mark_bank(&b);
        true
    });
    s.keep_raw("Get", &format!("/circles/{CID}/banks/{}/versions/{bv}", hex(77)), served_qs(bv, vec![rated(QB, "Two", Some("High"))], false));
    frames(&mut m, &mut banks, &ctx, &mut acct, &who, 6);
    assert_eq!(banks[1].questions[0].difficulty, Some(Difficulty::High));
    let bb = banks[1].clone();
    assert_eq!(badge(&mut m, &bb), "difficulty updated");
    let log = Arc::new(Mutex::new(m.edit.links().to_vec()));
    let mut m2 = restarted(log);
    assert_eq!(badge(&mut m2, &bb), "difficulty updated");
    let t = strip(&ctx, &mut m2, &mut banks, &mut acct, &s, LOCAL_B);
    assert!(t.contains("Question 1, \"Two\": Easy -> Hard") && t.contains("Put mine back"), "{t}");
}

/// The restore point taken before the circle's difficulty goes in is named for it, not for a merge.
#[test]
fn c96_i1_the_safety_copy_is_named_for_the_difficulty() {
    struct Named(Arc<Mutex<Vec<String>>>);
    impl super::browse::RestoreSink for Named {
        fn save(&self, _b: &[QuestionBank]) -> Result<String, String> {
            self.0.lock().unwrap().push("merge".into());
            Ok("x".into())
        }
        fn save_named(&self, _b: &[QuestionBank], what: &str) -> Result<String, String> {
            self.0.lock().unwrap().push(what.into());
            Ok("x".into())
        }
    }
    let (mut m, s, mut banks, _ctx, _acct) = rig();
    let seen = Arc::new(Mutex::new(Vec::new()));
    let media: super::publish::SharedMedia = Arc::new(std::collections::HashMap::<String, Vec<u8>>::new());
    let mut e = CircleEditModel::new(super::model::Runner::Inline, Arc::new(Mem::default()), media, Arc::new(Named(seen.clone())), LinkLog::memory()).with_retry(super::pool::RetryPolicy::none());
    e.set_clock(NOW);
    for l in m.edit.links().to_vec() {
        e.add_link(l).unwrap();
    }
    m.edit = e;
    phase_c(&mut m, &mut banks, 3, Some(Difficulty::Low), None);
    s.keep_raw("Get", &version_path(3), v3(Some("High"), None));
    quiet(&mut m, &mut banks, &olga_named(&s), now(), 3);
    assert_eq!(*seen.lock().unwrap(), vec!["difficulty".to_string()]);
}

// ---- c96 I2: "Use my bank as it is" has its own page ---------------------------------------------------------------------------

/// Not kept: the page has its own title and says what the next send does (difficulties included, colleagues' difficulty the
/// earlier build dropped overwritten); no "saved copy" box, no capital THIS. The Send page after it says the same in one line.
#[test]
fn c96_i2_use_as_is_has_its_own_page_and_the_send_page_says_it() {
    let (mut m, s, mut banks, ctx, mut acct) = rig();
    s.keep("Post", "/circles/heads", 200, heads(3));
    phase_c(&mut m, &mut banks, 3, Some(Difficulty::Low), None);
    super::bank_links_fix12_tests::not_kept(&s, 3);
    let who = olga_named(&s);
    frames(&mut m, &mut banks, &ctx, &mut acct, &who, 3);
    click(&ctx, &mut m, &mut banks, &mut acct, &who, "Use my bank as it is...", 0);
    let page = frames(&mut m, &mut banks, &ctx, &mut acct, &who, 1);
    assert!(page.contains("Use my bank as it is: \"") && !page.contains("Get latest:"), "{page}");
    assert!(page.contains(super::circles_text::AS_IS_TEXT) && page.contains(super::circles_text::AS_IS_DROPPED), "{page}");
    assert!(!page.contains("saved copy") && !page.contains("THIS") && !page.contains("Are you sure"), "{page}");
    // "No, go back" goes back to the bank
    click(&ctx, &mut m, &mut banks, &mut acct, &who, "No, go back", 0);
    assert!(!m.edit.active());
    click(&ctx, &mut m, &mut banks, &mut acct, &who, "Use my bank as it is...", 0);
    click(&ctx, &mut m, &mut banks, &mut acct, &who, "Yes, use it as it is", 0);
    let page = frames(&mut m, &mut banks, &ctx, &mut acct, &who, 2);
    assert!(page.contains(super::circles_text::AS_IS_DONE) && !page.contains("You already have the circle's newest version"), "{page}");
    m.edit.close();
    assert!(m.edit.open_send(LOCAL, owner()));
    host_a(&mut m);
    let page = frames(&mut m, &mut banks, &ctx, &mut acct, &who, 2);
    assert_eq!(page.matches(super::circles_text::AS_IS_SEND_LINE).count(), 1, "{page}");
    // a copy whose difficulties were read says nothing of colleagues' dropped difficulty
    let (mut m, s, mut banks, ctx, mut acct) = rig();
    // (opened before the bank is found put back: nothing to decide, an ordinary Get latest page)
    s.on("Post", &format!("/circles/{CID}/banks/{BID}/merge"), 400, json!({"error": "invalid_request", "field": "base_version"}));
    assert!(m.edit.open_merge_to_rebase(LOCAL, owner()));
    host_a(&mut m);
    let page = frames(&mut m, &mut banks, &ctx, &mut acct, &olga_named(&s), 3);
    assert!(page.contains("Get latest: \"") && page.contains("You already have the circle's newest version") && !page.contains(super::circles_text::AS_IS_DONE), "{page}");
    m.edit.close();
    make_stale(&mut m, LOCAL);
    m.edit.sync(&mut banks);
    s.on("Post", &format!("/circles/{CID}/banks/{BID}/merge"), 400, json!({"error": "invalid_request", "field": "base_version"}));
    assert!(m.edit.open_merge_to_rebase(LOCAL, owner()));
    host_a(&mut m);
    let mut page = String::new();
    for _ in 0..20 {
        page = frames(&mut m, &mut banks, &ctx, &mut acct, &olga_named(&s), 1);
        if matches!(&m.edit.view, EditView::Merge(v) if matches!(v.phase, MergePhase::StaleUpToDate { .. })) {
            page = frames(&mut m, &mut banks, &ctx, &mut acct, &olga_named(&s), 1);
            break;
        }
    }
    assert!(page.contains("Use my bank as it is: \"") && page.contains(super::circles_text::AS_IS_TEXT), "{page}");
    assert!(!page.contains(super::circles_text::AS_IS_DROPPED) && !page.contains("saved copy") && !page.contains("THIS"), "{page}");
}

// ---- c95 minors ------------------------------------------------------------------------------------------------------------

/// (p95_3b turned around) Get latest pressed while the read is on its way; the read lands but the bank can't be saved (held).
/// The review waits (it would call the circle's difficulty hers) and says why; once saved, it is built from the bank with the
/// circle's difficulty: One is not "Only you changed this", and the next send keeps the colleague's High.
#[test]
fn c95_m1_a_review_waits_while_the_circles_difficulty_cant_be_saved() {
    let (mut m, s, mut banks, ctx, mut acct) = rig();
    let k = rebuilt(&mut m);
    s.keep("Post", "/circles/heads", 200, heads(4));
    phase_c(&mut m, &mut banks, 3, None, None);
    k.save_fails.store(true, Ordering::SeqCst);
    m.edit.update_link(LOCAL, |l| {
        l.head_seen = 4;
        true
    });
    s.on("Get", &version_path(3), 503, json!({"error": "unavailable"}));
    s.keep_raw("Get", &version_path(3), v3(Some("High"), None));
    let who = olga_named(&s);
    frames(&mut m, &mut banks, &ctx, &mut acct, &who, 2);
    s.on("Post", &format!("{}/merge", put_path()), 200, merge_start(3, 4));
    s.keep_raw("Get", &put_path(), served_qs(4, vec![rated(QA, "One", Some("High")), rated(QB, "Two (b)", None)], false));
    assert!(m.edit.open_merge(LOCAL, owner()));
    host_a(&mut m);
    let page = frames(&mut m, &mut banks, &ctx, &mut acct, &who, 4);
    assert_eq!(m.edit.status(LOCAL).unwrap().learning, Some(Learning::Held));
    assert!(kinds(&m).is_none(), "no review yet: {page}");
    assert!(page.contains("couldn't save your banks") && page.contains("The review is made once it is saved"), "{page}");
    assert!(!page.contains("Only you changed this"), "{page}");
    k.save_fails.store(false, Ordering::SeqCst);
    for _ in 0..3 {
        page_frame(&mut m, &mut banks, &mut acct, &who, now() + Duration::from_secs(61));
    }
    let k2 = kinds(&m).expect("the review once saved");
    assert!(!k2.contains(&RowKind::OursChanged), "{k2:?}");
    apply(&mut m, &s, &mut banks, &ctx, &mut acct);
    assert_eq!(banks[0].questions[0].difficulty, Some(Difficulty::High));
    assert_eq!(sent_q(&mut m, &banks, 0), json!("High"));
}

fn held_with_record(by: String) -> (Vec<QuestionBank>, Arc<Mutex<Vec<CircleLink>>>) {
    let (mut m, s, mut banks, ctx, mut acct) = rig();
    let k = rebuilt(&mut m);
    s.keep("Post", "/circles/heads", 200, heads(3));
    phase_c(&mut m, &mut banks, 3, None, Some(Difficulty::Low));
    let b = banks[0].clone();
    m.edit.update_link(LOCAL, |l| {
        let mut r = super::circle_links::sent_record(l, &b, 3, by.clone());
        r.not_through = true;
        l.sent_record = Some(r);
        true
    });
    k.save_fails.store(true, Ordering::SeqCst);
    s.keep_raw("Get", &version_path(3), v3(Some("High"), None));
    let who = olga_named(&s);
    frames(&mut m, &mut banks, &ctx, &mut acct, &who, 3);
    assert_eq!(m.edit.status(LOCAL).unwrap().learning, Some(Learning::Held));
    (banks, k.log.clone())
}

/// (p95_4 turned around) Signed out after a restart (no account known yet): her own send record still keeps the Low she
/// pressed Send for.
#[test]
fn c95_m2_signed_out_after_a_restart_her_send_record_still_counts() {
    let s = Script::new();
    let tag = super::bank_links::account_tag(olga_named(&s).account.as_deref()).unwrap();
    let (mut banks, log) = held_with_record(tag);
    let mut m = restarted(log.clone());
    m.edit.sync(&mut banks);
    assert_eq!(banks[0].questions[1].difficulty, Some(Difficulty::Low));
    assert_eq!(banks[0].questions[0].difficulty, Some(Difficulty::High));
}

/// (p95_5 turned around) The banks file can't be written for ten minutes: one restore point for the bank as it is, not one a
/// minute. A change of hers makes the next try save a new one (the bank's state changed).
#[test]
fn c95_m3_one_restore_point_per_state_while_the_banks_file_cant_be_written() {
    let (mut m, s, mut banks, ctx, mut acct) = rig();
    let k = rebuilt(&mut m);
    s.keep("Post", "/circles/heads", 200, heads(3));
    phase_c(&mut m, &mut banks, 3, Some(Difficulty::Low), None);
    k.save_fails.store(true, Ordering::SeqCst);
    s.keep_raw("Get", &version_path(3), v3(Some("High"), None));
    let who = olga_named(&s);
    frames(&mut m, &mut banks, &ctx, &mut acct, &who, 3);
    for i in 1..=10u64 {
        page_frame(&mut m, &mut banks, &mut acct, &who, now() + Duration::from_secs(61 * i));
    }
    assert_eq!(*k.restores.lock().unwrap(), 1);
    banks[0].questions[1].stem = "Two (mine)".into();
    banks[0].modified += 5;
    page_frame(&mut m, &mut banks, &mut acct, &who, now() + Duration::from_secs(61 * 12));
    assert_eq!(*k.restores.lock().unwrap(), 2);
}

/// (p95_8 turned around) A daily-limit refusal with a huge Retry-After does not panic; it is waited out (capped), and the
/// strip says when the limit resets. The six older sites take the same capped conversion.
#[test]
fn c95_m5_a_huge_retry_after_never_panics() {
    let (mut m, s, mut banks, ctx, mut acct) = rig();
    s.keep("Post", "/circles/heads", 200, heads(3));
    phase_c(&mut m, &mut banks, 3, None, None);
    s.on_wait("Get", &version_path(3), 429, json!({"error": "daily_limit", "metric": "item_reads"}), u64::MAX);
    let who = olga_named(&s);
    quiet(&mut m, &mut banks, &who, now(), 2);
    assert_eq!(m.edit.status(LOCAL).unwrap().learning, Some(Learning::Limited));
    let strip = frames(&mut m, &mut banks, &ctx, &mut acct, &who, 1);
    assert!(strip.contains("It resets in about 48 hours"), "{strip}");
    let t0 = now();
    let later = super::api::later(t0, u64::MAX);
    assert!(later >= t0 && later <= t0 + Duration::from_secs(super::api::MAX_WAIT_SECS));
    assert_eq!(super::api::later(t0, 600), t0 + Duration::from_secs(600));
}

// ---- c96 minors ----------------------------------------------------------------------------------------------------------------

/// (minor 1) Not kept: the strip says plainly what she can do; its first (big) button is "Use my bank as it is...", never Check
/// now (which only finds a newer version someone sent: it stays in the "..." menu).
#[test]
fn c96_m1_not_kept_offers_use_as_is_first() {
    let (mut m, s, mut banks, ctx, mut acct) = rig();
    s.keep("Post", "/circles/heads", 200, heads(3));
    phase_c(&mut m, &mut banks, 3, Some(Difficulty::Low), None);
    super::bank_links_fix12_tests::not_kept(&s, 3);
    let who = olga_named(&s);
    frames(&mut m, &mut banks, &ctx, &mut acct, &who, 3);
    let v = strip_view(&mut m, &banks, &who);
    // (round 18, critic 98 minor 4) the safe action is the big button; "Use my bank as it is..." a plain one after it
    assert_eq!(v.buttons, vec![StripButton::CheckNow, StripButton::TreatInStep, StripButton::History], "{v:?}");
    assert!(v.menu_check);
    assert!(v.line.contains("the circle no longer keeps it") && v.line.contains(super::circles_text::NOT_KEPT_WAYS) && !v.line.contains("until the circle can be checked"), "{}", v.line);
}

/// (minor 1) Held back for review and not kept: not a dead end: "Use my bank as it is..." is offered, and it opens its page.
#[test]
fn c96_m1_held_for_review_and_not_kept_has_a_way_out() {
    let (mut m, s, mut banks, ctx, mut acct) = rig();
    s.keep("Post", "/circles/heads", 200, json!({"banks": [{"bank_id": BID, "status": "under_review"}]}));
    phase_c(&mut m, &mut banks, 3, Some(Difficulty::Low), None);
    super::bank_links_fix12_tests::not_kept(&s, 3);
    m.links.check_now(LOCAL);
    let who = olga_named(&s);
    frames(&mut m, &mut banks, &ctx, &mut acct, &who, 6);
    let v = strip_view(&mut m, &banks, &who);
    assert!(v.line.contains("held back for review"), "{}", v.line);
    assert_eq!(v.buttons, vec![StripButton::CheckNow, StripButton::TreatInStep], "{v:?}");
    click(&ctx, &mut m, &mut banks, &mut acct, &who, "Use my bank as it is...", 0);
    assert!(matches!(&m.edit.view, EditView::Merge(v) if matches!(v.phase, MergePhase::StaleUpToDate { ask: Some(StaleAsk::Rebase) })));
}

/// (minors 1, 2) The leave, close and remove panels never say "stay until then": a waiting copy is named with where to look.
#[test]
fn c96_m2_panels_never_say_stay_until_then() {
    for n in [Some(1), Some(2), None] {
        let t = super::circles_text::leave_unsent("Unit 2 quiz", n, true);
        assert!(!t.contains("stay until then") && t.contains("Its page in My Banks says why and what you can do."), "{t}");
    }
}

/// (minor 3) The daily limit: no blame, no "items", when it resets, and no Try again, Get latest or Check now anywhere (the
/// "..." menu too); behind, the strip names the newer version as the badge does. Get latest's own page says the circle's limit
/// with no Try again.
#[test]
fn c96_m3_the_daily_limit_says_when_it_resets_and_offers_nothing_that_would_meet_it() {
    let (mut m, s, mut banks, ctx, mut acct) = rig();
    s.keep("Post", "/circles/heads", 200, heads(4));
    phase_c(&mut m, &mut banks, 3, None, None);
    m.edit.update_link(LOCAL, |l| {
        l.head_seen = 4;
        true
    });
    s.on_wait("Get", &version_path(3), 429, json!({"error": "daily_limit", "metric": "item_reads"}), 36_000);
    let who = olga_named(&s);
    frames(&mut m, &mut banks, &ctx, &mut acct, &who, 3);
    let v = strip_view(&mut m, &banks, &who);
    assert_eq!(v.buttons, vec![StripButton::History], "{v:?}");
    assert!(!v.menu_check);
    assert!(v.line.contains("It resets in about 10 hours") && v.line.contains("Version 4 is the latest. You have version 3."), "{}", v.line);
    assert!(!v.line.contains("You have reached") && !v.line.contains("items"), "{}", v.line);
    assert_eq!(badge(&mut m, &banks[0].clone()), "newer version");
    // the "..." menu offers no Check now
    let mut b = browse();
    assert!(super::circles_phasec_tests::click_text(&ctx, 1280.0, "...", 0, &mut |ui| {
        super::bank_links_ui::strip_with(ui, &mut m, &mut banks, &mut b, &mut acct, &who, now(), NOW, LOCAL);
    }));
    let menu = strip_as(&ctx, &mut m, &mut banks, &mut acct, &who, LOCAL);
    assert!(menu.contains("Circle page") && !menu.contains("Check now"), "{menu}");
    // Get latest (from elsewhere) meets the limit: the circle's words, no Try again
    s.on("Post", &format!("{}/merge", put_path()), 200, merge_start(3, 4));
    s.on_wait("Get", &put_path(), 429, json!({"error": "daily_limit", "metric": "item_reads"}), 36_000);
    assert!(m.edit.open_merge(LOCAL, owner()));
    host_a(&mut m);
    let page = frames(&mut m, &mut banks, &ctx, &mut acct, &who, 3);
    assert!(page.contains("Your daily limit for reading banks has been reached. It resets in about 10 hours"), "{page}");
    assert!(!page.contains("Try again") && !page.contains("items") && !page.contains("You have reached"), "{page}");
}

/// (minor 4) The banks file can't be written: the strip says why in plain words (no system error text), in the warning colour,
/// the badge says it too, and Try again that fails again says it tried.
#[test]
fn c96_m4_a_save_failure_is_said_plainly_and_try_again_answers() {
    let (mut m, s, mut banks, ctx, mut acct) = rig();
    let media: super::publish::SharedMedia = Arc::new(std::collections::HashMap::<String, Vec<u8>>::new());
    let saver: BankSaver = Arc::new(|_b: &[QuestionBank]| Err("Permission denied (os error 13)".into()));
    let mut e = CircleEditModel::new(super::model::Runner::Inline, Arc::new(Mem::default()), media, Arc::new(NoDiskRestore), LinkLog::memory()).with_retry(super::pool::RetryPolicy::none()).with_bank_saver(saver);
    e.set_clock(NOW);
    for l in m.edit.links().to_vec() {
        e.add_link(l).unwrap();
    }
    m.edit = e;
    s.keep("Post", "/circles/heads", 200, heads(3));
    phase_c(&mut m, &mut banks, 3, Some(Difficulty::Low), None);
    s.keep_raw("Get", &version_path(3), v3(Some("High"), None));
    let who = olga_named(&s);
    let strip = frames(&mut m, &mut banks, &ctx, &mut acct, &who, 3);
    let v = strip_view(&mut m, &banks, &who);
    assert!(v.warn, "{v:?}");
    assert!(strip.contains("verSHion couldn't save your banks (verSHion isn't allowed to write to its data folder)"), "{strip}");
    assert!(!strip.contains("os error") && !strip.contains("Permission denied"), "{strip}");
    assert_eq!(badge(&mut m, &banks[0].clone()), "couldn't save");
    let st = m.edit.status(LOCAL).cloned();
    assert!(m.edit.with_link(LOCAL, |l| super::bank_links::badge_warns_with(l, st.as_ref())).unwrap());
    // (round 20, critic 100 minor 12) "verSHion" once in the line, not four times
    assert!(strip.contains("tries again every minute"), "{strip}");
    click(&ctx, &mut m, &mut banks, &mut acct, &who, "Try again", 0);
    let strip = frames(&mut m, &mut banks, &ctx, &mut acct, &who, 2);
    assert!(strip.contains("Tried 2 times; it still couldn't be saved."), "{strip}");
    assert_eq!(banks[0].questions[0].difficulty, Some(Difficulty::Low));
}

/// (minor 6) Signed out: the strip says the version she has and the unsent changes exactly as the badge does (behind: "newer
/// version" and the version she has; waiting for the read: not counted, never a count).
#[test]
fn c96_m6_signed_out_strip_and_badge_agree() {
    let out = |_s: &Arc<Script>| AccountSnapshot { gate: super::publish::Gate::SignedOut, session: None, account_open: true, account: None, name: None };
    // behind, one unsent change
    let (mut m, s, mut banks, ctx, mut acct) = rig();
    banks[0].questions[0].stem = "One (mine)".into();
    banks[0].modified += 5;
    m.edit.update_link(LOCAL, |l| {
        l.head_seen = 5;
        true
    });
    let who = out(&s);
    let t = frames(&mut m, &mut banks, &ctx, &mut acct, &who, 2);
    assert_eq!(badge(&mut m, &banks[0].clone()), "newer version · 1 unsent");
    assert!(t.contains("Version 5 when last checked") && t.contains("; you have version 3. You have 1 unsent change."), "{t}");
    // an earlier build's link, not read: the badge does not count, nor does the strip
    let (mut m, s, mut banks, ctx, mut acct) = rig();
    phase_c(&mut m, &mut banks, 3, None, None);
    banks[0].questions[1].stem = "Two (mine)".into();
    banks[0].modified += 5;
    let who = out(&s);
    let t = frames(&mut m, &mut banks, &ctx, &mut acct, &who, 2);
    // (round 18, critic 98 minor 8) said as what it is, the same on both
    assert_eq!(badge(&mut m, &banks[0].clone()), super::bank_links::WAITING_NOTE);
    assert!(!t.contains("unsent change") && t.contains("Your changes are counted once verSHion can check the circle."), "{t}");
}

// ---- mutation gaps ---------------------------------------------------------------------------------------------------------------

/// (survivor m05) The banks file the links file's store keeps is really written, and reads back as the banks.
#[test]
fn s17_m05_the_link_logs_banks_file_is_written() {
    let dir = std::env::temp_dir().join(format!("vershion-r17-banks-{}-{}", std::process::id(), NOW));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let (bank, _) = linked(&[(QA, "One")]);
    LinkLog::Dir(dir.clone()).save_banks(std::slice::from_ref(&bank)).unwrap();
    let back: Vec<QuestionBank> = serde_json::from_slice(&std::fs::read(dir.join(super::circle_links::BANKS_DOC)).unwrap()).unwrap();
    assert_eq!(back.len(), 1);
    assert_eq!((back[0].id.as_str(), back[0].questions[0].stem.as_str()), (LOCAL, "One"));
    let _ = std::fs::remove_dir_all(&dir);
}

/// (survivor m42) Get latest during the daily limit does not read the base version again (it would meet the same limit).
#[test]
fn s17_m42_get_latest_during_the_limit_does_not_read_the_base() {
    let (mut m, s, mut banks, _ctx, _acct) = rig();
    s.keep("Post", "/circles/heads", 200, heads(4));
    phase_c(&mut m, &mut banks, 3, None, None);
    s.on_wait("Get", &version_path(3), 429, json!({"error": "daily_limit", "metric": "item_reads"}), 36_000);
    let who = olga_named(&s);
    quiet(&mut m, &mut banks, &who, now(), 2);
    assert_eq!(version_reads(&s, 3), 1);
    s.on("Post", &format!("{}/merge", put_path()), 200, merge_start(3, 4));
    s.keep_raw("Get", &put_path(), served_qs(4, vec![rated(QA, "One", None), rated(QB, "Two", None)], false));
    assert!(m.edit.open_merge(LOCAL, owner()));
    quiet(&mut m, &mut banks, &who, now(), 3);
    assert_eq!(version_reads(&s, 3), 1, "{:?}", s.paths());
}

/// (survivor m43) Get latest on a copy whose version can't be read, answered "nothing newer" by the follow read (a viewer's
/// permission on the page): the page asks her to decide, not "nothing to get".
#[test]
fn s17_m43_nothing_newer_for_a_copy_that_cant_be_read_asks() {
    let (mut m, s, mut banks, ctx, mut acct) = rig();
    s.keep("Post", "/circles/heads", 200, heads(3));
    phase_c(&mut m, &mut banks, 3, None, None);
    super::bank_links_fix12_tests::not_kept(&s, 3);
    let who = olga_named(&s);
    frames(&mut m, &mut banks, &ctx, &mut acct, &who, 3);
    assert_eq!(m.edit.status(LOCAL).unwrap().learning, Some(Learning::NotKept));
    s.keep_raw("Get", &put_path(), served_qs(3, vec![rated(QA, "One", None), rated(QB, "Two", None)], false));
    let viewer = Perm { role: super::circles_api::Role::Viewer, is_author: false, closed: false };
    assert!(m.edit.open_merge(LOCAL, viewer));
    quiet(&mut m, &mut banks, &who, now(), 3);
    assert!(matches!(&m.edit.view, EditView::Merge(v) if matches!(v.phase, MergePhase::StaleUpToDate { .. })), "{:?}", m.edit.view);
}

/// (survivor m53) A quiet read whose job is lost twice (its thread died) is "couldn't check" after the second, not "checking"
/// for ever.
#[test]
fn s17_m53_a_read_lost_twice_says_it_couldnt_check() {
    let (mut m, s, mut banks, _ctx, _acct) = rig();
    s.keep("Post", "/circles/heads", 200, heads(3));
    phase_c(&mut m, &mut banks, 3, None, None);
    background(&mut m);
    // no reply is scripted for version 3: the job's thread dies
    let who = olga_named(&s);
    let t0 = now();
    for at in [t0, t0 + Duration::from_secs(6)] {
        quiet(&mut m, &mut banks, &who, at, 1);
        for _ in 0..300 {
            quiet(&mut m, &mut banks, &who, at, 1);
            if !m.edit.busy() {
                break;
            }
            std::thread::sleep(Duration::from_millis(10));
        }
    }
    assert_eq!(version_reads(&s, 3), 2);
    assert_eq!(m.edit.status(LOCAL).unwrap().learning, Some(Learning::CantReach));
}

/// (survivor m10) Signed out with a zero "last checked" time: no date from 1970.
#[test]
fn s17_m10_signed_out_with_no_time_says_no_date() {
    let (mut m, _s, mut banks, ctx, mut acct) = rig();
    m.edit.update_link(LOCAL, |l| {
        l.role_seen_at = Some(0);
        l.head_seen_at = Some(0);
        true
    });
    let who = AccountSnapshot { gate: super::publish::Gate::SignedOut, session: None, account_open: true, account: None, name: None };
    let t = frames(&mut m, &mut banks, &ctx, &mut acct, &who, 2);
    assert!(t.contains("Version 3 when last checked.") && !t.contains("1970"), "{t}");
}

/// (survivor m38) Once the difficulties are saved, nothing holds the bank any more: a question put back (Undo) right after takes
/// the circle's difficulty at once, not a minute later.
#[test]
fn s17_m38_a_hold_that_worked_does_not_delay_the_next_pass() {
    let (mut m, s, mut banks, ctx, mut acct) = rig();
    let k = rebuilt(&mut m);
    s.keep("Post", "/circles/heads", 200, heads(3));
    phase_c(&mut m, &mut banks, 3, None, Some(Difficulty::Low));
    let removed = banks[0].questions.remove(0);
    banks[0].modified += 5;
    k.save_fails.store(true, Ordering::SeqCst);
    s.keep_raw("Get", &version_path(3), v3(Some("High"), None));
    let who = olga_named(&s);
    frames(&mut m, &mut banks, &ctx, &mut acct, &who, 3);
    assert_eq!(m.edit.status(LOCAL).unwrap().learning, Some(Learning::Held));
    k.save_fails.store(false, Ordering::SeqCst);
    let t1 = now() + Duration::from_secs(61);
    page_frame(&mut m, &mut banks, &mut acct, &who, t1);
    assert_eq!(banks[0].questions[0].difficulty, None, "Two took the circle's");
    banks[0].questions.insert(0, removed);
    banks[0].modified += 5;
    page_frame(&mut m, &mut banks, &mut acct, &who, t1 + Duration::from_secs(1));
    assert_eq!(banks[0].questions[0].difficulty, Some(Difficulty::High), "at once");
}

/// (survivor m47) After "Use my bank as it is", a question deleted before the read and put back (Undo) keeps hers: nothing of
/// the circle's is taken into the bank any more.
#[test]
fn s17_m47_after_use_as_is_a_question_put_back_keeps_hers() {
    let (mut m, s, mut banks, _ctx, _acct) = rig();
    phase_c(&mut m, &mut banks, 3, Some(Difficulty::Low), None);
    let removed = banks[0].questions.remove(0);
    banks[0].modified += 5;
    s.keep_raw("Get", &version_path(3), v3(Some("High"), None));
    let who = olga_named(&s);
    quiet(&mut m, &mut banks, &who, now(), 3);
    make_stale(&mut m, LOCAL);
    treat_in_step(&mut m, &s, &mut banks, LOCAL, BID, false);
    banks[0].questions.insert(0, removed);
    banks[0].modified += 5;
    quiet(&mut m, &mut banks, &who, now(), 3);
    assert_eq!(banks[0].questions[0].difficulty, Some(Difficulty::Low));
}

/// (critic 96 minor 8) Dates in one form ("Oct 8, 2025").
#[test]
fn c96_m8_dates_in_one_form() {
    let ms = 1_759_881_600_000; // 2025-10-08
    assert_eq!(super::bank_links::day_month_year(ms), "Oct 8, 2025");
    assert_eq!(super::bank_links::own_copy_line("Algebra", "Unit 2", "A. Teacher", 9, ms), "From the circle \"Algebra\": \"Unit 2\" by A. Teacher, version 9, Oct 8, 2025.");
    assert_eq!(super::circles_text::ago(ms, ms + 90 * super::circles_tests::DAY), "Oct 8, 2025");
}

/// Where each drawn text is, `width` wide: (its text, its rectangle).
fn text_rects(width: f32, draw: &mut dyn FnMut(&mut egui::Ui)) -> Vec<(String, egui::Rect)> {
    let ctx = ctx();
    let mut all = Vec::new();
    for _ in 0..2 {
        let input = egui::RawInput { screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(width, 6000.0))), ..Default::default() };
        let out = ctx.run_ui(input, |ui| {
            egui::CentralPanel::default().show(ui, |ui| draw(ui));
        });
        all = out.shapes.iter().filter_map(|s| if let egui::epaint::Shape::Text(t) = &s.shape { Some((t.galley.text().to_string(), t.galley.rect.translate(t.pos.to_vec2()))) } else { None }).collect();
        out.drop_without_applying_deltas();
    }
    all
}

/// (critic 96 minor 7) Bank settings, "Remove from the circle...": the question and the unsent-changes line are each on their
/// own line under "Report this bank...", never run on after it.
#[test]
fn c96_m7_the_remove_question_is_on_its_own_lines() {
    let (mut m, s) = open_circle("owner", json!({}));
    let (mut bank, mut l) = linked(&[(QA, "One"), (QB, "Two")]);
    l.role_seen = Some("owner".into());
    m.edit.add_link(l).unwrap();
    bank.questions[0].stem.push_str(" (mine)");
    let mut banks = vec![bank];
    m.edit.sync(&mut banks);
    s.keep_raw("Get", &format!("/circles/{CID}/banks/{BID}"), served(1, &[(QA, "One"), (QB, "Two")], false));
    m.open_bank(BID);
    for _ in 0..3 {
        m.frame(now(), &ready(&s));
    }
    m.ask_remove_bank();
    for width in [1280.0, 760.0, 420.0] {
        let r = text_rects(width, &mut |ui| {
            super::circles_ui::body_of(ui, &mut m);
        });
        let at = |p: &str| r.iter().find(|(t, _)| t.starts_with(p)).map(|(_, r)| *r).unwrap_or_else(|| panic!("no {p:?} at {width}: {r:?}"));
        let report = at("Report this bank...");
        let ask = at("The bank disappears for everyone");
        let unsent = at("\"Unit 2 quiz\" has 1 unsent change");
        assert!(ask.top() >= report.bottom() && (ask.left() - report.left()).abs() < 30.0, "{width}: {report:?} {ask:?}");
        assert!(unsent.top() >= ask.bottom(), "{width}: {ask:?} {unsent:?}");
    }
}

/// (critic 96 minor 3) A circle screen's wait before Try again works is said in minutes or hours when it is long.
#[test]
fn c96_m3_a_long_wait_is_said_in_hours() {
    let p = super::browse::Problem { message: "The circle's limit.".into(), retry: true, retry_at: None, account: false };
    let t = text_of(760.0, &mut |ui| {
        let (mut r, mut a) = (false, false);
        super::circles_ui::problem_box(ui, &p, Some(Duration::from_secs(36_000)), &mut r, &mut a);
    });
    assert!(t.contains("You can try again in about 10 hours.") && !t.contains("seconds"), "{t}");
    let t = text_of(760.0, &mut |ui| {
        let (mut r, mut a) = (false, false);
        super::circles_ui::problem_box(ui, &p, Some(Duration::from_secs(42)), &mut r, &mut a);
    });
    assert!(t.contains("You can try again in about 42 seconds."), "{t}");
}

/// (critic 96 minor 8) The circle page's members say when they joined in the same form.
#[test]
fn c96_m8_joined_in_the_same_form() {
    let (mut m, _s) = open_circle("owner", json!({}));
    let t = text_of(1280.0, &mut |ui| {
        super::circles_ui::body_of(ui, &mut m);
    });
    assert!(t.contains("Joined Oct 4, 2025") && !t.contains("2025-10-04"), "{t}");
}

/// (survivors k03, k20) "Use my bank as it is" is said on the Send page until the copy moves to another version: a send or a
/// merge ends it.
#[test]
fn s17_k03_k20_a_send_or_a_merge_ends_use_as_is() {
    let (mut m, s, mut banks, ctx, mut acct) = rig();
    s.keep("Post", "/circles/heads", 200, heads(3));
    phase_c(&mut m, &mut banks, 3, Some(Difficulty::Low), None);
    super::bank_links_fix12_tests::not_kept(&s, 3);
    let who = olga_named(&s);
    frames(&mut m, &mut banks, &ctx, &mut acct, &who, 3);
    assert!(m.edit.open_merge_to_rebase(LOCAL, owner()));
    m.edit.rebaseline();
    m.edit.sync(&mut banks);
    m.edit.close();
    assert!(m.edit.sends_as_is(LOCAL));
    // a send that went through
    let link = m.edit.link_for_local(LOCAL).unwrap();
    let sent = super::circles_api::SentBank { version: 4, version_id: hex(4), qids: vec![QA.into(), QB.into()], ..Default::default() };
    assert!(!super::circle_links::after_send(&link, &banks[0], &sent).as_is);
    // a merge
    get_latest(&mut m, &s, &mut banks, &ctx, &mut acct, served_qs(4, vec![rated(QA, "One", None), rated(QB, "Two (b)", None)], false));
    for i in 0..kinds(&m).unwrap().len() {
        m.edit.set_merge_choice(i, super::merge::Choice::Ours);
    }
    apply(&mut m, &s, &mut banks, &ctx, &mut acct);
    assert!(!m.edit.sends_as_is(LOCAL));
}

/// The new link fields read leniently: one this build can't read is dropped alone, never the link (nor the other links).
#[test]
fn s17_new_link_fields_read_leniently() {
    let (_, l) = linked(&[(QA, "One")]);
    let mut v = serde_json::to_value(&l).unwrap();
    v["at_read"] = json!("not a list");
    v["difficulty_news"] = json!(42);
    let back: CircleLink = serde_json::from_value(v).unwrap();
    assert_eq!((back.at_read, back.difficulty_news, back.as_is), (None, None, false));
    // and what this build writes reads back
    let mut l2 = l.clone();
    l2.at_read = Some(vec![super::circle_links::AtRead { id: "x".into(), difficulty: Some(Difficulty::High) }]);
    l2.difficulty_news = Some(super::circle_links::DifficultyNews { version: 3, questions: vec![super::circle_links::NewsItem { id: "x".into(), before: None, after: Some(Difficulty::Low), put_back: true, changed: true }] });
    l2.as_is = true;
    let back: CircleLink = serde_json::from_slice(&serde_json::to_vec(&l2).unwrap()).unwrap();
    assert_eq!(back, l2);
}

/// (survivor k54) A question whose difficulty is hers again by another way (she set it back herself, or an Undo) is said to be
/// hers, with no "Put mine back"; one no longer in the bank offers none either.
#[test]
fn s17_k54_put_back_is_offered_only_where_it_would_change_something() {
    let (mut m, s, mut banks, ctx, mut acct) = rig();
    s.keep("Post", "/circles/heads", 200, heads(3));
    phase_c(&mut m, &mut banks, 3, Some(Difficulty::Low), Some(Difficulty::Low));
    s.keep_raw("Get", &version_path(3), v3(Some("High"), None));
    let who = olga_named(&s);
    frames(&mut m, &mut banks, &ctx, &mut acct, &who, 3);
    banks[0].questions[0].difficulty = Some(Difficulty::Low);
    banks[0].modified += 5;
    let strip = frames(&mut m, &mut banks, &ctx, &mut acct, &who, 2);
    assert!(strip.contains("Question 1, \"One\": yours (Easy) is back, not sent yet"), "{strip}");
    assert_eq!(strip.lines().filter(|l| *l == "Put mine back").count(), 1, "{strip}");
    banks[0].questions.remove(1);
    banks[0].modified += 5;
    let strip = frames(&mut m, &mut banks, &ctx, &mut acct, &who, 2);
    assert!(strip.contains("A question no longer in this bank: Easy -> no difficulty"), "{strip}");
    assert_eq!(strip.lines().filter(|l| *l == "Put mine back").count(), 0, "{strip}");
}

/// (survivor k55) The Banks list's badge of a bank whose circle difficulty can't be saved is drawn in the warning colour.
#[test]
fn s17_k55_the_list_badge_warns_while_it_cant_be_saved() {
    let (mut m, s, mut banks, ctx, mut acct) = rig();
    let k = rebuilt(&mut m);
    s.keep("Post", "/circles/heads", 200, heads(3));
    phase_c(&mut m, &mut banks, 3, Some(Difficulty::Low), None);
    k.save_fails.store(true, Ordering::SeqCst);
    s.keep_raw("Get", &version_path(3), v3(Some("High"), None));
    let who = olga_named(&s);
    frames(&mut m, &mut banks, &ctx, &mut acct, &who, 3);
    let bank = banks[0].clone();
    let out = ctx.run_ui(egui::RawInput { screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(760.0, 400.0))), ..Default::default() }, |ui| {
        egui::CentralPanel::default().show(ui, |ui| super::bank_links_ui::badge_of(ui, &mut m, &bank));
    });
    let colour = out.shapes.iter().find_map(|s| match &s.shape {
        egui::epaint::Shape::Text(t) if t.galley.text() == "· couldn't save" => t.galley.job.sections.first().map(|x| x.format.color),
        _ => None,
    });
    out.drop_without_applying_deltas();
    assert_eq!(colour, Some(crate::theme::bad()));
}

/// (survivor k43) A copy whose version can't be read, with changes made after a send whose answer could not be read: Send is not
/// offered, and the strip says why in the words of "not kept", never "until the circle can be checked".
#[test]
fn s17_k43_not_kept_says_its_own_why_where_send_is_taken_away() {
    let (mut m, s, mut banks, ctx, mut acct) = rig();
    s.keep("Post", "/circles/heads", 200, heads(3));
    phase_c(&mut m, &mut banks, 3, None, None);
    super::bank_links_fix12_tests::not_kept(&s, 3);
    banks[0].questions[1].stem = "Two (mine)".into();
    banks[0].modified += 30;
    m.edit.update_link(LOCAL, |l| {
        l.sent_unread = Some(super::circle_links::SentUnread { by: String::new(), modified: 1 });
        true
    });
    let who = olga_named(&s);
    let strip = frames(&mut m, &mut banks, &ctx, &mut acct, &who, 3);
    assert!(strip.contains("Changes you made since") && !strip.contains("Send my changes"), "{strip}");
    assert!(strip.contains(&super::circles_text::not_kept_line(3)) && !strip.contains(super::circles_text::SEND_WAITS), "{strip}");
}
