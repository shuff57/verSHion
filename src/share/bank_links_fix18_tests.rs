//! Phase D1, round 18 (critics 97 and 98): each test written first against the code it pins. The owner's rule stays (after one
//! successful read of the circle's version the copy takes the circle's difficulty; a later deliberate change of hers
//! supersedes). What the circle's difficulty replaced settles as the copy moves on: a question changed since says so and offers
//! nothing, one put back and sent says it was sent, and "Put mine back" never writes over a newer difficulty.

use std::sync::atomic::Ordering;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use serde_json::json;

use super::bank_links::StripButton;
use super::bank_links_fix11_tests::{host_a, rated, rig, version_path};
use super::bank_links_fix15_tests::phase_c;
use super::bank_links_fix16_tests::{rebuilt, sent_q, v3};
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

fn put_backs(strip: &str) -> usize {
    strip.lines().filter(|l| *l == "Put mine back").count()
}

/// A send of A that went through as version 4 (as `finish_send` moves the link on).
fn sent_as_v4(m: &mut CirclesModel, banks: &[QuestionBank]) {
    let bank = banks[0].clone();
    m.edit.update_link(LOCAL, |l| {
        let sent = super::circles_api::SentBank { version: 4, version_id: hex(4), qids: vec![QA.into(), QB.into()], ..Default::default() };
        *l = super::circle_links::after_send(l, &bank, &sent);
        true
    });
}

/// A's page after the read of version 3 (One rated High there, Two none) replaced her Easy on both: the news lists both.
fn with_news() -> Rig {
    let (mut m, s, mut banks, ctx, mut acct) = rig();
    s.keep("Post", "/circles/heads", 200, heads(3));
    phase_c(&mut m, &mut banks, 3, Some(Difficulty::Low), Some(Difficulty::Low));
    s.keep_raw("Get", &version_path(3), v3(Some("High"), None));
    let who = olga_named(&s);
    frames(&mut m, &mut banks, &ctx, &mut acct, &who, 3);
    assert_eq!(news(&mut m).map(|n| n.questions.len()), Some(2));
    (m, s, banks, ctx, acct)
}

fn qid(banks: &[QuestionBank], i: usize) -> String {
    banks[0].questions[i].id.clone()
}

// ---- c98 I1: the news settles as the copy moves on ------------------------------------------------------------------------------

/// (p97_1 turned around) Put mine back on Two, then a send that went through: Two's row says hers was sent (never "not sent yet"),
/// One (the circle's, the copy now at version 4) offers nothing any more, and the words over the list do not ask her to press a
/// button that is not there.
#[test]
fn c98_i1_a_put_back_that_was_sent_says_sent() {
    let (mut m, s, mut banks, ctx, mut acct) = with_news();
    let who = olga_named(&s);
    assert!(click(&ctx, &mut m, &mut banks, &mut acct, &who, "Put mine back", 1));
    assert_eq!(sent_q(&mut m, &banks, 1), json!("Low"));
    sent_as_v4(&mut m, &banks);
    s.keep("Post", "/circles/heads", 200, heads(4));
    let strip = frames(&mut m, &mut banks, &ctx, &mut acct, &who, 3);
    assert_eq!(changes(&mut m, &banks[0].clone()), (0, 0, 0), "{strip}");
    assert!(strip.contains("Question 2, \"Two\": yours (Easy) was sent"), "{strip}");
    assert!(!strip.contains("not sent yet"), "{strip}");
    assert!(strip.contains("Question 1, \"One\": Easy -> Hard"), "{strip}");
    assert_eq!(put_backs(&strip), 0, "{strip}");
    assert!(!strip.contains("press Put mine back"), "{strip}");
}

/// After Get latest to a version where a colleague rated One again (Medium): One's row says it changed since, with no Put mine
/// back, and pressing it anyway (a stale click) leaves the colleague's Medium. Two (still the circle's, but the copy moved on)
/// offers nothing either.
#[test]
fn c98_i1_after_get_latest_a_colleagues_newer_difficulty_is_never_put_back_over() {
    let (mut m, s, mut banks, ctx, mut acct) = with_news();
    let who = olga_named(&s);
    m.edit.update_link(LOCAL, |l| {
        l.head_seen = 4;
        true
    });
    get_latest(&mut m, &s, &mut banks, &ctx, &mut acct, served_qs(4, vec![rated(QA, "One", Some("Medium")), rated(QB, "Two", None)], false));
    apply(&mut m, &s, &mut banks, &ctx, &mut acct);
    assert_eq!(banks[0].questions[0].difficulty, Some(Difficulty::Medium), "the colleague's");
    let strip = frames(&mut m, &mut banks, &ctx, &mut acct, &who, 3);
    assert!(strip.contains("Question 1, \"One\": Easy -> Hard, changed since (now Medium)"), "{strip}");
    assert_eq!(put_backs(&strip), 0, "{strip}");
    let one = qid(&banks, 0);
    assert!(!m.edit.put_mine_back(&mut banks, LOCAL, &one));
    let two = qid(&banks, 1);
    assert!(!m.edit.put_mine_back(&mut banks, LOCAL, &two), "the copy moved on: not offered");
    assert_eq!((banks[0].questions[0].difficulty, banks[0].questions[1].difficulty), (Some(Difficulty::Medium), None));
}

/// Her own edit settles the row: One rated Medium by her says "changed since" with no Put mine back, and stays so when she rates it
/// Hard again (her newer choice is never written over). Two still offers it.
#[test]
fn c98_i1_her_own_edit_settles_the_row() {
    let (mut m, s, mut banks, ctx, mut acct) = with_news();
    let who = olga_named(&s);
    banks[0].questions[0].difficulty = Some(Difficulty::Medium);
    banks[0].modified += 5;
    let strip = frames(&mut m, &mut banks, &ctx, &mut acct, &who, 2);
    assert!(strip.contains("Question 1, \"One\": Easy -> Hard, changed since (now Medium)"), "{strip}");
    assert_eq!(put_backs(&strip), 1, "only Two's: {strip}");
    banks[0].questions[0].difficulty = Some(Difficulty::High);
    banks[0].modified += 5;
    let strip = frames(&mut m, &mut banks, &ctx, &mut acct, &who, 2);
    assert!(strip.contains("Question 1, \"One\": Easy -> Hard, changed since (now Hard)"), "{strip}");
    assert_eq!(put_backs(&strip), 1, "{strip}");
    let one = qid(&banks, 0);
    assert!(!m.edit.put_mine_back(&mut banks, LOCAL, &one));
    assert_eq!(banks[0].questions[0].difficulty, Some(Difficulty::High));
}

/// (survivor k81) Put back, then she sets Two to the circle's difficulty herself: that is her newer choice ("changed since"), and
/// Put mine back is not offered again.
#[test]
fn s18_k81_put_back_then_changed_is_changed_since() {
    let (mut m, s, mut banks, ctx, mut acct) = with_news();
    let who = olga_named(&s);
    assert!(click(&ctx, &mut m, &mut banks, &mut acct, &who, "Put mine back", 1));
    // (before any other frame: what she put back is what is expected of it now, not the circle's)
    banks[0].questions[1].difficulty = None;
    banks[0].modified += 5;
    let strip = frames(&mut m, &mut banks, &ctx, &mut acct, &who, 2);
    assert!(strip.contains("Question 2, \"Two\": Easy -> no difficulty, changed since (now no difficulty)"), "{strip}");
    assert_eq!(put_backs(&strip), 1, "One's only: {strip}");
    let two = qid(&banks, 1);
    assert!(!m.edit.put_mine_back(&mut banks, LOCAL, &two));
    assert_eq!(banks[0].questions[1].difficulty, None);
}

/// A send of other changes moves the copy on: a question still at the circle's difficulty from the read offers nothing any more.
#[test]
fn c98_i1_after_a_send_nothing_is_offered() {
    let (mut m, s, mut banks, ctx, mut acct) = with_news();
    let who = olga_named(&s);
    banks[0].questions[0].stem = "One (mine)".into();
    banks[0].modified += 5;
    sent_as_v4(&mut m, &banks);
    s.keep("Post", "/circles/heads", 200, heads(4));
    let strip = frames(&mut m, &mut banks, &ctx, &mut acct, &who, 2);
    assert!(strip.contains("Question 1, \"One (mine)\": Easy -> Hard") && strip.contains("Question 2, \"Two\": Easy -> no difficulty"), "{strip}");
    assert_eq!(put_backs(&strip), 0, "{strip}");
    let one = qid(&banks, 0);
    assert!(!m.edit.put_mine_back(&mut banks, LOCAL, &one));
    assert_eq!(banks[0].questions[0].difficulty, Some(Difficulty::High));
}

// ---- c98 I2, c97 m8: the badge adds the news to what else it says -------------------------------------------------------------

/// (p97_9 turned around) A bank with news, behind the circle, with an unsent change: the badge says all three, the newer version
/// first; with nothing unsent it still says "newer version".
#[test]
fn c98_i2_the_badge_adds_the_news_to_newer_version_and_unsent() {
    let (mut m, s, mut banks, ctx, mut acct) = with_news();
    let who = olga_named(&s);
    s.keep("Post", "/circles/heads", 200, heads(5));
    m.edit.update_link(LOCAL, |l| {
        l.head_seen = 5;
        true
    });
    frames(&mut m, &mut banks, &ctx, &mut acct, &who, 2);
    assert_eq!(badge(&mut m, &banks[0].clone()), "newer version · difficulty updated");
    banks[0].questions[0].stem = "One (mine)".into();
    banks[0].modified += 5;
    frames(&mut m, &mut banks, &ctx, &mut acct, &who, 2);
    assert_eq!(badge(&mut m, &banks[0].clone()), "newer version · difficulty updated · 1 unsent");
}

/// Signed out: the badge and the strip say the same (the version she has, the unsent change) and the news is not left out.
#[test]
fn c98_i2_signed_out_badge_and_strip_agree_with_news() {
    let (mut m, s, mut banks, ctx, mut acct) = with_news();
    m.edit.update_link(LOCAL, |l| {
        l.head_seen = 5;
        true
    });
    banks[0].questions[0].stem = "One (mine)".into();
    banks[0].modified += 5;
    let out = AccountSnapshot { gate: super::publish::Gate::SignedOut, session: None, account_open: true, account: None, name: None };
    let t = frames(&mut m, &mut banks, &ctx, &mut acct, &out, 2);
    assert_eq!(badge(&mut m, &banks[0].clone()), "newer version · difficulty updated · 1 unsent");
    assert!(t.contains("Version 5 when last checked") && t.contains("; you have version 3. You have 1 unsent change."), "{t}");
    let _ = s;
}

/// Where the Banks list's card is narrow (a 420 px window), the badge says the same in short words, whole and on screen, and the circle's
/// name keeps some room.
#[test]
fn c98_i2_the_badge_is_shortened_when_narrow() {
    let b = super::bank_links::Badge { circle: "Biology department, Pleasant Valley High School".into(), note: Some("newer version · difficulty updated · 1 unsent".into()) };
    let shapes = |width: f32| {
        let ctx = egui::Context::default();
        crate::theme::setup(&ctx);
        let input = egui::RawInput { screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(width, 800.0))), time: Some(0.0), ..Default::default() };
        let mut found = Vec::new();
        for _ in 0..2 {
            let out = ctx.run_ui(input.clone(), |ui| {
                // (as the Banks list's grid card draws it: the page's side margins, then the card's)
                egui::CentralPanel::default().show(ui, |ui| {
                    egui::Frame::new().inner_margin(egui::Margin::symmetric(20, 0)).show(ui, |ui| {
                        crate::kit::card().inner_margin(egui::Margin::same(14)).show(ui, |ui| {
                            ui.set_width(ui.available_width());
                            super::bank_links_ui::badge_line(ui, &b, false);
                        });
                    });
                });
            });
            found = out.shapes.iter().filter_map(|s| if let egui::epaint::Shape::Text(t) = &s.shape { Some((t.galley.text().to_string(), t.galley.elided, t.pos.x, t.pos.x + t.galley.size().x)) } else { None }).collect::<Vec<_>>();
            out.drop_without_applying_deltas();
        }
        found
    };
    let wide = shapes(1280.0);
    assert!(wide.iter().any(|f| f.0 == "· newer version · difficulty updated · 1 unsent"), "{wide:?}");
    let narrow = shapes(420.0);
    let note = narrow.iter().find(|f| f.0.starts_with("· newer")).unwrap();
    assert_eq!(note.0, "· newer · difficulty · 1 unsent", "the news part is shortened too: {narrow:?}");
    assert!(!note.1 && note.3 <= 420.0, "{narrow:?}");
    let name = narrow.iter().find(|f| f.0.starts_with("Biology")).unwrap();
    assert!(name.3 - name.2 >= 60.0, "{narrow:?}");
}

// ---- c97 m1: a held Undo pass is tried again, and Send waits for it -----------------------------------------------------------

/// One (her Low) deleted before the read, the read settled, then One put back (Undo) while the banks file can't be written. A
/// question put back after the read waits for the circle's difficulty like the first pass: Held (Send waits), the pass is kept and
/// tried again, and once saving works One takes the circle's High (her next send keeps the colleague's High).
fn undo_while_held() -> (Rig, super::bank_links_fix16_tests::Sinks) {
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
    k.save_fails.store(true, Ordering::SeqCst);
    banks[0].questions.insert(0, removed);
    banks[0].modified += 5;
    ((m, s, banks, ctx, acct), k)
}

#[test]
fn c97_m1_a_held_undo_pass_holds_send_and_is_tried_again() {
    let ((mut m, s, mut banks, ctx, mut acct), k) = undo_while_held();
    let who = olga_named(&s);
    let strip = frames(&mut m, &mut banks, &ctx, &mut acct, &who, 3);
    assert_eq!(banks[0].questions[0].difficulty, Some(Difficulty::Low), "not saved: nothing changed");
    assert_eq!(m.edit.status(LOCAL).unwrap().learning, Some(Learning::Held), "{strip}");
    assert!(!strip.contains("Send my changes"), "{strip}");
    k.save_fails.store(false, Ordering::SeqCst);
    for i in 1..=3u64 {
        page_frame(&mut m, &mut banks, &mut acct, &who, now() + Duration::from_secs(61 * i));
    }
    assert_eq!(banks[0].questions[0].difficulty, Some(Difficulty::High), "tried again once saving works");
    assert_eq!(changes(&mut m, &banks[0].clone()), (0, 0, 0));
    assert_eq!(sent_q(&mut m, &banks, 0), json!("High"));
    assert_eq!(m.edit.status(LOCAL).unwrap().learning, None);
}

/// Her change made while that pass is held (One rated Medium) is hers: it is kept when the pass is tried again.
#[test]
fn c97_m1_her_change_during_a_held_undo_pass_is_kept() {
    let ((mut m, s, mut banks, ctx, mut acct), k) = undo_while_held();
    let who = olga_named(&s);
    frames(&mut m, &mut banks, &ctx, &mut acct, &who, 3);
    assert_eq!(m.edit.status(LOCAL).unwrap().learning, Some(Learning::Held));
    banks[0].questions[0].difficulty = Some(Difficulty::Medium);
    banks[0].modified += 5;
    page_frame(&mut m, &mut banks, &mut acct, &who, now() + Duration::from_secs(61));
    k.save_fails.store(false, Ordering::SeqCst);
    for i in 2..=4u64 {
        page_frame(&mut m, &mut banks, &mut acct, &who, now() + Duration::from_secs(61 * i));
    }
    assert_eq!(banks[0].questions[0].difficulty, Some(Difficulty::Medium), "her later change stays");
    assert_eq!(m.edit.status(LOCAL).unwrap().learning, None, "nothing waits any more");
    assert_eq!(sent_q(&mut m, &banks, 0), json!("Medium"));
}

// ---- c97 m2/m3: Put mine back writes the banks file first ---------------------------------------------------------------------

/// (p97_2 turned around) Put mine back while the banks file can't be written: nothing changes (the bank keeps the circle's, the
/// links file does not say it was put back), the button stays, and the strip says it could not be saved. Once it can, the banks
/// file is written before the links file says so.
#[test]
fn c97_m3_put_back_saves_the_banks_first_and_keeps_the_button_when_it_cant() {
    let (mut m, s, mut banks, ctx, mut acct) = rig();
    let k = rebuilt(&mut m);
    s.keep("Post", "/circles/heads", 200, heads(3));
    phase_c(&mut m, &mut banks, 3, Some(Difficulty::Low), Some(Difficulty::Low));
    s.keep_raw("Get", &version_path(3), v3(Some("High"), None));
    let who = olga_named(&s);
    frames(&mut m, &mut banks, &ctx, &mut acct, &who, 3);
    let writes = k.saves.lock().unwrap().len();
    k.save_fails.store(true, Ordering::SeqCst);
    click(&ctx, &mut m, &mut banks, &mut acct, &who, "Put mine back", 1);
    assert_eq!(banks[0].questions[1].difficulty, None, "nothing changed");
    assert!(!k.log.lock().unwrap()[0].difficulty_news.as_ref().unwrap().questions[1].put_back, "the links file says nothing was put back");
    let strip = frames(&mut m, &mut banks, &ctx, &mut acct, &who, 2);
    assert_eq!(put_backs(&strip), 2, "the button stays: {strip}");
    assert!(strip.contains("Your difficulty couldn't be put back") && strip.contains("couldn't save your banks"), "{strip}");
    // after a restart (the bank as on disk) the row offers it still
    let mut m2 = restarted(k.log.clone());
    let strip = frames(&mut m2, &mut banks.clone(), &ctx, &mut acct, &who, 2);
    assert_eq!(put_backs(&strip), 2, "{strip}");
    // once it can be saved: the banks file first, then the links file
    k.save_fails.store(false, Ordering::SeqCst);
    click(&ctx, &mut m, &mut banks, &mut acct, &who, "Put mine back", 1);
    assert_eq!(banks[0].questions[1].difficulty, Some(Difficulty::Low));
    let saves = k.saves.lock().unwrap().clone();
    assert_eq!(saves.len(), writes + 1, "the banks file was written by Put mine back itself");
    assert_eq!(saves.last().unwrap().0[0].questions[1].difficulty, Some(Difficulty::Low));
    assert!(k.log.lock().unwrap()[0].difficulty_news.as_ref().unwrap().questions[1].put_back);
    let strip = frames(&mut m, &mut banks, &ctx, &mut acct, &who, 2);
    assert!(!strip.contains("couldn't be put back"), "the line is out of date: {strip}");
}

/// The banks file is written before the links file says it was put back (a crash between the two leaves the button, never a
/// false "is back").
#[test]
fn c97_m3_the_links_file_says_put_back_only_after_the_banks_file_has_it() {
    let (mut m, s, mut banks, ctx, mut acct) = with_news();
    let log: Arc<Mutex<Vec<CircleLink>>> = Arc::new(Mutex::new(m.edit.links().to_vec()));
    let seen: Arc<Mutex<Vec<bool>>> = Arc::default();
    let (l2, s2) = (log.clone(), seen.clone());
    let saver: BankSaver = Arc::new(move |_b: &[QuestionBank]| {
        s2.lock().unwrap().push(l2.lock().unwrap()[0].difficulty_news.as_ref().is_some_and(|n| n.questions.iter().any(|q| q.put_back)));
        Ok(())
    });
    let media: super::publish::SharedMedia = Arc::new(std::collections::HashMap::<String, Vec<u8>>::new());
    let mut e = CircleEditModel::new(super::model::Runner::Inline, Arc::new(Mem::default()), media, Arc::new(NoDiskRestore), LinkLog::Memory(log.clone())).with_retry(super::pool::RetryPolicy::none()).with_bank_saver(saver);
    e.set_clock(NOW);
    m.edit = e;
    let who = olga_named(&s);
    frames(&mut m, &mut banks, &ctx, &mut acct, &who, 2);
    click(&ctx, &mut m, &mut banks, &mut acct, &who, "Put mine back", 0);
    assert_eq!(*seen.lock().unwrap(), vec![false], "banks written once, before the links file said put back");
    assert!(log.lock().unwrap()[0].difficulty_news.as_ref().unwrap().questions[0].put_back);
}

// ---- c97 m4: the safety copy covers the other banks' work done during a hold ----------------------------------------------------

/// (p97_3 turned around) While the banks file can't be written she works on bank B: the next try makes a new safety copy (it holds
/// B's work), and the try that works writes the banks file after a copy that has everything as it was just before. A try with
/// nothing changed makes no new copy.
#[test]
fn c97_m4_the_safety_copy_is_made_again_when_another_bank_changed_during_the_hold() {
    let (mut m, s, mut banks, ctx, mut acct) = rig();
    let k = rebuilt(&mut m);
    s.keep("Post", "/circles/heads", 200, heads(3));
    phase_c(&mut m, &mut banks, 3, Some(Difficulty::Low), None);
    k.save_fails.store(true, Ordering::SeqCst);
    s.keep_raw("Get", &version_path(3), v3(Some("High"), None));
    let who = olga_named(&s);
    frames(&mut m, &mut banks, &ctx, &mut acct, &who, 3);
    assert_eq!(m.edit.status(LOCAL).unwrap().learning, Some(Learning::Held));
    assert_eq!(*k.restores.lock().unwrap(), 1);
    banks[1].questions[0].stem = "Two, reworded during the hold".into();
    banks[1].modified += 5;
    page_frame(&mut m, &mut banks, &mut acct, &who, now() + Duration::from_secs(61));
    assert_eq!(*k.restores.lock().unwrap(), 2, "B changed: a new safety copy");
    page_frame(&mut m, &mut banks, &mut acct, &who, now() + Duration::from_secs(122));
    assert_eq!(*k.restores.lock().unwrap(), 2, "nothing changed: no new copy");
    k.save_fails.store(false, Ordering::SeqCst);
    page_frame(&mut m, &mut banks, &mut acct, &who, now() + Duration::from_secs(183));
    assert_eq!(banks[0].questions[0].difficulty, Some(Difficulty::High), "done");
    assert_eq!(*k.restores.lock().unwrap(), 2);
    let copy = k.restored.lock().unwrap().last().cloned().unwrap();
    assert_eq!(copy[1].questions[0].stem, "Two, reworded during the hold", "the copy has B's work");
    assert_eq!(copy[0].questions[0].difficulty, Some(Difficulty::Low), "and A as it was before");
}

// ---- c97 m5: signed out, another account's send record is not hers ----------------------------------------------------------------

/// (p97_4 turned around) After a restart, signed out (no account known yet), a send record of another account is not taken for
/// hers: the pass waits for a sign-in. Signed in as olga, the circle's is taken (the record is not hers).
#[test]
fn c97_m5_signed_out_another_accounts_record_waits_for_a_sign_in() {
    let (mut m, s, mut banks, ctx, mut acct) = rig();
    let k = rebuilt(&mut m);
    s.keep("Post", "/circles/heads", 200, heads(3));
    phase_c(&mut m, &mut banks, 3, None, Some(Difficulty::Low));
    let b = banks[0].clone();
    m.edit.update_link(LOCAL, |l| {
        let mut r = super::circle_links::sent_record(l, &b, 3, "someone-else".into());
        r.not_through = true;
        l.sent_record = Some(r);
        true
    });
    k.save_fails.store(true, Ordering::SeqCst);
    s.keep_raw("Get", &version_path(3), v3(Some("High"), None));
    let who = olga_named(&s);
    frames(&mut m, &mut banks, &ctx, &mut acct, &who, 3);
    assert_eq!(m.edit.status(LOCAL).unwrap().learning, Some(Learning::Held));
    let mut m2 = restarted(k.log.clone());
    let mut b2 = banks.clone();
    assert!(!m2.edit.sync(&mut b2));
    assert_eq!((b2[0].questions[0].difficulty, b2[0].questions[1].difficulty), (None, Some(Difficulty::Low)), "nothing yet");
    assert!(m2.edit.with_link(LOCAL, |l| l.inherit).unwrap(), "still waits");
    m2.edit.frame(now(), &who);
    assert!(m2.edit.sync(&mut b2));
    assert_eq!((b2[0].questions[0].difficulty, b2[0].questions[1].difficulty), (Some(Difficulty::High), None), "signed in: the circle's");
}

// ---- c97 m6: a links-file write that fails after the pass ------------------------------------------------------------------------

/// A model on a links file in `dir` whose banks are saved by `saver`.
fn on_dir(dir: &std::path::Path, saver: BankSaver) -> CircleEditModel {
    let media: super::publish::SharedMedia = Arc::new(std::collections::HashMap::<String, Vec<u8>>::new());
    let mut e = CircleEditModel::new(super::model::Runner::Inline, Arc::new(Mem::default()), media, Arc::new(NoDiskRestore), LinkLog::Dir(dir.to_path_buf())).with_retry(super::pool::RetryPolicy::none()).with_bank_saver(saver);
    e.set_clock(NOW);
    e
}

/// The read learnt into a links file in a fresh folder; the banks file, once written, makes the links file unwritable (a sync tool
/// takes it right then). Returns the folder and the banks files written.
fn learnt_then_links_held(m: &mut CirclesModel, s: &Arc<Script>, banks: &mut Vec<QuestionBank>, tag: &str) -> (std::path::PathBuf, Arc<Mutex<Vec<Vec<QuestionBank>>>>) {
    let dir = std::env::temp_dir().join(format!("vershion-r18-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    s.keep("Post", "/circles/heads", 200, heads(3));
    phase_c(m, banks, 3, Some(Difficulty::Low), Some(Difficulty::Low));
    let saved: Arc<Mutex<Vec<Vec<QuestionBank>>>> = Arc::default();
    let (sv, d) = (saved.clone(), dir.clone());
    let saver: BankSaver = Arc::new(move |b: &[QuestionBank]| {
        sv.lock().unwrap().push(b.to_vec());
        std::fs::create_dir_all(d.join("share-circles.json.tmp").join("held")).unwrap();
        Ok(())
    });
    let mut e = on_dir(&dir, saver);
    for l in m.edit.links().to_vec() {
        e.add_link(l).unwrap();
    }
    m.edit = e;
    s.keep_raw("Get", &version_path(3), v3(Some("High"), None));
    let who = olga_named(s);
    m.edit.frame(now(), &who);
    m.edit.frame(now(), &who);
    assert!(LinkLog::Dir(dir.clone()).load()[0].inherit, "learnt, in the file");
    (dir, saved)
}

/// (p97_8 turned around) The pass writes the banks file, then the links file can't be written. She then rates One Medium, and the
/// app is quit. After a restart her Medium stays (the links file knew the bank as the read found it) and Two's news is still there.
#[test]
fn c97_m6_after_a_links_write_that_failed_a_restart_keeps_her_change_and_the_news() {
    let (mut m, s, mut banks, ctx, mut acct) = rig();
    let (dir, saved) = learnt_then_links_held(&mut m, &s, &mut banks, "m6a");
    assert!(m.edit.sync(&mut banks));
    assert_eq!((banks[0].questions[0].difficulty, banks[0].questions[1].difficulty), (Some(Difficulty::High), None));
    assert_eq!(saved.lock().unwrap().len(), 1);
    let who = olga_named(&s);
    let strip = frames(&mut m, &mut banks, &ctx, &mut acct, &who, 1);
    assert!(strip.contains("could not be saved"), "{strip}");
    banks[0].questions[0].difficulty = Some(Difficulty::Medium);
    banks[0].modified += 5;
    std::fs::remove_dir_all(dir.join("share-circles.json.tmp")).unwrap();
    let mut m2 = restarted(Arc::new(Mutex::new(Vec::new())));
    let sv = saved.clone();
    m2.edit = on_dir(&dir, Arc::new(move |b: &[QuestionBank]| {
        sv.lock().unwrap().push(b.to_vec());
        Ok(())
    }));
    m2.edit.sync(&mut banks);
    assert_eq!(banks[0].questions[0].difficulty, Some(Difficulty::Medium), "her later change stays");
    assert_eq!(banks[0].questions[1].difficulty, None);
    let n = m2.edit.with_link(LOCAL, |l| l.difficulty_news.clone()).flatten().expect("the news");
    assert!(n.questions.iter().any(|q| q.id == banks[0].questions[1].id && q.before == Some(Difficulty::Low) && q.after.is_none()), "{n:?}");
    assert!(!m2.edit.with_link(LOCAL, |l| l.inherit).unwrap());
    let _ = std::fs::remove_dir_all(&dir);
}

/// The same, the links file writable again before she quits: the next frame writes it (in step with the bank), so a restart has
/// nothing to do again.
#[test]
fn c97_m6_the_links_file_is_written_again_once_it_can_be() {
    let (mut m, s, mut banks, _ctx, mut acct) = rig();
    let (dir, _saved) = learnt_then_links_held(&mut m, &s, &mut banks, "m6b");
    assert!(m.edit.sync(&mut banks));
    assert!(LinkLog::Dir(dir.clone()).load()[0].inherit, "not written yet");
    std::fs::remove_dir_all(dir.join("share-circles.json.tmp")).unwrap();
    let who = olga_named(&s);
    page_frame(&mut m, &mut banks, &mut acct, &who, now() + Duration::from_secs(6));
    let file = LinkLog::Dir(dir.clone()).load();
    assert!(!file[0].inherit && file[0].difficulty_news.is_some(), "written again: {:?}", file[0].difficulty_news);
    let _ = std::fs::remove_dir_all(&dir);
}

// ---- c97 m7: the new fields are read leniently -------------------------------------------------------------------------------------

/// (p97_6 turned around) A non-bool `as_is` drops only that field (the link stays, not "as is"); one `at_read` entry this build can't
/// read drops only that entry.
#[test]
fn c97_m7_a_bad_as_is_or_at_read_entry_drops_only_itself() {
    let dir = std::env::temp_dir().join(format!("vershion-r18-m7-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let (_, l) = linked(&[(QA, "One")]);
    let mut a = serde_json::to_value(&l).unwrap();
    a["as_is"] = json!("yes");
    let mut b = serde_json::to_value(&l).unwrap();
    b["local_bank_id"] = json!("other-bank");
    b["circle_bank_id"] = json!(hex(77));
    b["at_read"] = json!([{"id": "q1", "difficulty": "Low"}, {"id": "q2", "difficulty": "VeryHard"}]);
    std::fs::write(dir.join("share-circles.json"), serde_json::to_vec(&json!({"links": [a, b]})).unwrap()).unwrap();
    let links = LinkLog::Dir(dir.clone()).load();
    assert_eq!(links.len(), 2, "{links:?}");
    assert!(!links[0].as_is);
    assert_eq!(links[1].at_read, Some(vec![super::circle_links::AtRead { id: "q1".into(), difficulty: Some(Difficulty::Low) }]));
    assert!(!dir.join("share-circles.json.bad").exists());
    let _ = std::fs::remove_dir_all(&dir);
}

// ---- critic 97's surviving mutants ----------------------------------------------------------------------------------------------

/// (survivor k11) Two failed tries in a row around her change: her Medium, made after the first failed try, stays when a later try
/// succeeds (the bank as the read found it is taken once, at the first try).
#[test]
fn s18_k11_her_change_survives_two_failed_tries() {
    let (mut m, s, mut banks, ctx, mut acct) = rig();
    let k = rebuilt(&mut m);
    s.keep("Post", "/circles/heads", 200, heads(3));
    phase_c(&mut m, &mut banks, 3, Some(Difficulty::Low), Some(Difficulty::Low));
    k.save_fails.store(true, Ordering::SeqCst);
    s.keep_raw("Get", &version_path(3), v3(Some("High"), None));
    let who = olga_named(&s);
    frames(&mut m, &mut banks, &ctx, &mut acct, &who, 3);
    assert_eq!(m.edit.status(LOCAL).unwrap().learning, Some(Learning::Held));
    banks[0].questions[0].difficulty = Some(Difficulty::Medium);
    banks[0].modified += 5;
    page_frame(&mut m, &mut banks, &mut acct, &who, now() + Duration::from_secs(61));
    assert_eq!(m.edit.status(LOCAL).unwrap().learning, Some(Learning::Held), "second try failed too");
    k.save_fails.store(false, Ordering::SeqCst);
    page_frame(&mut m, &mut banks, &mut acct, &who, now() + Duration::from_secs(122));
    assert_eq!(banks[0].questions[1].difficulty, None, "the pass worked");
    assert_eq!(banks[0].questions[0].difficulty, Some(Difficulty::Medium), "her change stays");
}

/// (survivor k63) A daily limit's `Retry-After` is capped at two days, exactly, wherever it is turned into a time: a huge one is never
/// "try again now".
#[test]
fn s18_k63_retry_after_is_capped_at_two_days_at_every_site() {
    let t0 = now();
    assert_eq!(super::api::later(t0, u64::MAX), t0 + Duration::from_secs(super::api::MAX_WAIT_SECS));
    assert_eq!(super::api::later(t0, super::api::MAX_WAIT_SECS + 1), t0 + Duration::from_secs(2 * 86_400));
    assert_eq!(super::api::later(t0, 600), t0 + Duration::from_secs(600));
    // every site turns a Retry-After into a time with it, and none adds it to the clock itself
    let sites = [("browse.rs", include_str!("browse.rs")), ("invite.rs", include_str!("invite.rs")), ("circles.rs", include_str!("circles.rs")), ("circle_send.rs", include_str!("circle_send.rs")), ("mine.rs", include_str!("mine.rs")), ("publish.rs", include_str!("publish.rs")), ("circle_edit.rs", include_str!("circle_edit.rs"))];
    let mut used = 0;
    for (name, src) in sites {
        for line in src.lines().filter(|l| l.contains("retry_after") && !l.trim_start().starts_with("//")) {
            assert!(!line.contains("checked_add") && !(line.contains('+') && line.contains("from_secs")), "{name}: {line}");
        }
        used += src.matches("later(").count();
    }
    assert!(used >= 7, "{used}");
}

/// (survivor k95) The safety copy made before the circle's difficulty goes in is really named for it on disk
/// (`backups/banks-before-difficulty-*.json`), and holds the banks.
#[test]
fn s18_k95_the_safety_copy_file_is_named_for_the_difficulty() {
    let dir = std::env::temp_dir().join(format!("vershion-r18-k95-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let (bank, _) = linked(&[(QA, "One")]);
    let fs = crate::store::FsBackend::new(dir.clone());
    let at = super::browse::save_copy_to(&fs, std::slice::from_ref(&bank), "difficulty", NOW / 1000).unwrap();
    let names: Vec<String> = std::fs::read_dir(dir.join("backups")).unwrap().map(|e| e.unwrap().file_name().to_string_lossy().to_string()).collect();
    assert_eq!(names.len(), 1, "{names:?}");
    assert!(names[0].starts_with("banks-before-difficulty-") && names[0].ends_with(".json"), "{names:?}");
    assert!(at.contains(&names[0]), "{at}");
    let back: Vec<QuestionBank> = serde_json::from_slice(&std::fs::read(dir.join("backups").join(&names[0])).unwrap()).unwrap();
    assert_eq!(back[0].questions[0].stem, "One");
    // a second one the same second is not written over
    super::browse::save_copy_to(&fs, std::slice::from_ref(&bank), "difficulty", NOW / 1000).unwrap();
    assert_eq!(std::fs::read_dir(dir.join("backups")).unwrap().count(), 2);
    let _ = std::fs::remove_dir_all(&dir);
}

/// (survivor k56) "Use my bank as it is" on a bank put back from a saved copy (the stale path): its own title while it reads, and
/// "No, go back" goes back to the bank, never to Get latest's question.
#[test]
fn s18_k56_use_as_is_on_the_stale_path_says_so_and_goes_back_to_the_bank() {
    let (mut m, s, mut banks, ctx, mut acct) = rig();
    make_stale(&mut m, LOCAL);
    m.edit.sync(&mut banks);
    s.on("Post", &format!("/circles/{CID}/banks/{BID}/merge"), 400, json!({"error": "invalid_request", "field": "base_version"}));
    assert!(m.edit.open_merge_to_rebase(LOCAL, owner()));
    host_a(&mut m);
    assert!(m.edit.asking_as_is(), "titled for it while it reads");
    let who = olga_named(&s);
    let mut page = String::new();
    for _ in 0..20 {
        page = frames(&mut m, &mut banks, &ctx, &mut acct, &who, 1);
        if matches!(&m.edit.view, EditView::Merge(v) if matches!(v.phase, MergePhase::StaleUpToDate { .. })) {
            page = frames(&mut m, &mut banks, &ctx, &mut acct, &who, 1);
            break;
        }
    }
    assert!(page.contains("Use my bank as it is: \""), "{page}");
    click(&ctx, &mut m, &mut banks, &mut acct, &who, "No, go back", 0);
    assert!(!m.edit.active(), "back to the bank");
}

// ---- c98 minors ------------------------------------------------------------------------------------------------------------------

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

/// (minor 1) The daily limit is the account's (it counts every circle and the public pool), never "the circle's".
#[test]
fn c98_m1_the_daily_limit_is_yours() {
    use super::circles_text as t;
    for line in [t::READ_LIMIT.to_string(), t::read_limit(Some(36_000)), t::read_limit_page(Some(36_000))] {
        assert!(line.starts_with("Your daily limit for reading banks") && !line.contains("circle's daily limit"), "{line}");
    }
}

/// (minor 2) The banks file can't be written: the strip says her own edits are not saved either, and what she can do.
#[test]
fn c98_m2_a_save_failure_says_her_edits_are_not_saved_and_what_to_do() {
    use super::circles_text as t;
    // (round 19, critic 100 I2: one piece of advice she can act on, whatever the cause)
    let full = t::banks_not_saved("No space left on device (os error 28)");
    assert!(full.contains("your own edits aren't saved either") && full.contains("free some disk space"), "{full}");
    let denied = t::banks_not_saved("Permission denied (os error 13)");
    assert!(denied.contains("your own edits aren't saved either") && denied.contains("press Try again"), "{denied}");
    let other = t::banks_not_saved("something else");
    assert!(other.contains("press Try again") && !other.contains("Check verSHion's data folder"), "{other}");
    // the safety copy alone failing does not say her edits are lost
    assert!(!t::safety_copy_not_saved("No space left on device (os error 28)").contains("your own edits"));
    // on the strip
    let (mut m, s, mut banks, ctx, mut acct) = rig();
    let k = rebuilt(&mut m);
    s.keep("Post", "/circles/heads", 200, heads(3));
    phase_c(&mut m, &mut banks, 3, Some(Difficulty::Low), None);
    k.save_fails.store(true, Ordering::SeqCst);
    s.keep_raw("Get", &version_path(3), v3(Some("High"), None));
    let strip = frames(&mut m, &mut banks, &ctx, &mut acct, &olga_named(&s), 3);
    assert!(strip.contains("your own edits aren't saved either") && strip.contains("free some disk space"), "{strip}");
}

/// (minor 3) Not kept: the strip never says "Version 3 is the latest" beside "no longer keeps version 3"; a version held back for
/// review says waiting helps; one whose reply can't be read says so (never "no longer keeps").
#[test]
fn c98_m3_cant_read_is_said_per_reason() {
    let (mut m, s, banks, _ctx, _acct) = not_kept_rig();
    let who = olga_named(&s);
    let v = strip_view(&mut m, &banks, &who);
    assert!(!v.line.contains("Version 3 is the latest") && v.line.contains("Your copy last matched version 3, but the circle no longer keeps it"), "{}", v.line);
    // held back while an administrator reviews it
    let (mut m, s, mut banks, ctx, mut acct) = rig();
    s.keep("Post", "/circles/heads", 200, heads(3));
    phase_c(&mut m, &mut banks, 3, Some(Difficulty::Low), None);
    s.keep("Get", &version_path(3), 404, json!({"error": "version_withheld"}));
    let who = olga_named(&s);
    frames(&mut m, &mut banks, &ctx, &mut acct, &who, 3);
    let v = strip_view(&mut m, &banks, &who);
    assert!(v.line.contains("is held back while an administrator reviews it") && v.line.contains("Waiting is enough") && !v.line.contains("no longer keeps"), "{}", v.line);
    // a reply that can't be read
    let (mut m, s, mut banks, ctx, mut acct) = rig();
    s.keep("Post", "/circles/heads", 200, heads(3));
    phase_c(&mut m, &mut banks, 3, Some(Difficulty::Low), None);
    s.keep_raw("Get", &version_path(3), b"not json".to_vec());
    let who = olga_named(&s);
    frames(&mut m, &mut banks, &ctx, &mut acct, &who, 3);
    let v = strip_view(&mut m, &banks, &who);
    assert!(v.line.contains("verSHion couldn't read it") && !v.line.contains("no longer keeps"), "{}", v.line);
    // the newest version held back for review, and the one she matched not kept: waiting for the review helps
    let (mut m, s, mut banks, ctx, mut acct) = rig();
    s.keep("Post", "/circles/heads", 200, json!({"banks": [{"bank_id": BID, "status": "under_review"}]}));
    phase_c(&mut m, &mut banks, 3, Some(Difficulty::Low), None);
    super::bank_links_fix12_tests::not_kept(&s, 3);
    m.links.check_now(LOCAL);
    let who = olga_named(&s);
    frames(&mut m, &mut banks, &ctx, &mut acct, &who, 6);
    let v = strip_view(&mut m, &banks, &who);
    // (round 20, critic 100 minor 12) the same in three sentences: held back, waiting is enough, Check now asks again
    assert!(v.line.contains("held back for review") && v.line.contains("Waiting is enough; Check now asks the circle again"), "{}", v.line);
}

/// (minor 4) Not kept: the big button is the safe one (Check now); "Use my bank as it is..." is a plain one after it. The badge and
/// the circle page's row are in the warning colour while sending is off.
#[test]
fn c98_m4_not_kept_offers_the_safe_action_first_and_warns() {
    let (mut m, s, mut banks, _ctx, _acct) = not_kept_rig();
    let who = olga_named(&s);
    let v = strip_view(&mut m, &banks, &who);
    assert_eq!(v.buttons, vec![StripButton::CheckNow, StripButton::TreatInStep, StripButton::History], "{v:?}");
    let st = m.edit.status(LOCAL).cloned();
    assert!(m.edit.with_link(LOCAL, |l| super::bank_links::badge_warns_with(l, st.as_ref())).unwrap(), "the badge warns");
    circle_page(&mut m, &s, &mut banks, &who);
    let d = drawn(1280.0, &mut |ui| {
        super::circles_ui::body_of(ui, &mut m);
    });
    let row = d.iter().find(|t| t.0.contains(super::circles_text::NOT_KEPT_ROW)).unwrap_or_else(|| panic!("{d:?}"));
    assert_eq!(row.2, crate::theme::bad(), "the row warns");
}

/// (minor 5) After "Use my bank as it is" (a round-12 link whose version is not kept): the Send page repeats what the send
/// overwrites, and says nothing that contradicts "difficulties included"; nor does the strip.
#[test]
fn c98_m5_after_use_as_is_the_send_page_warns_again_and_nothing_contradicts_it() {
    let (mut m, s, mut banks, ctx, mut acct) = not_kept_rig();
    let who = olga_named(&s);
    assert!(m.edit.open_merge_to_rebase(LOCAL, owner()));
    host_a(&mut m);
    frames(&mut m, &mut banks, &ctx, &mut acct, &who, 1);
    m.edit.rebaseline();
    m.edit.sync(&mut banks);
    m.edit.close();
    let strip = frames(&mut m, &mut banks, &ctx, &mut acct, &who, 2);
    assert!(!strip.contains("can't be counted"), "{strip}");
    assert!(m.edit.open_send(LOCAL, owner()));
    host_a(&mut m);
    let page = frames(&mut m, &mut banks, &ctx, &mut acct, &who, 2);
    assert!(page.contains("Anything others sent that this bank doesn't have will no longer be in the newest version"), "{page}");
    assert!(!page.contains("can't be counted"), "{page}");
}

/// A's page after the read of a version that rated `n` questions differently from her (each hers Easy, the circle's Hard).
pub(super) fn many_news(n: usize) -> Rig {
    let (mut m, s, mut banks) = two_at_three();
    s.keep("Get", &format!("/circles/{CID}"), 200, detail("owner", json!({})));
    let ctx = ctx();
    let mut acct = FakeAccount { open: true, ..FakeAccount::default() };
    let ids: Vec<String> = (0..n).map(|i| format!("{:032x}", 0xabc0 + i)).collect();
    let items: Vec<(&str, String)> = ids.iter().enumerate().map(|(i, id)| (id.as_str(), format!("Question number {} about solving linear equations with fractions", i + 1))).collect();
    let refs: Vec<(&str, &str)> = items.iter().map(|(a, b)| (*a, b.as_str())).collect();
    let (mut bank, mut link) = linked(&refs);
    for q in &mut bank.questions {
        q.difficulty = Some(Difficulty::Low);
    }
    for b in &mut link.base {
        b.snap.difficulty = None;
    }
    link.base_version = 3;
    link.base_version_id = hex(3);
    link.head_seen = 3;
    link.difficulty_read = None;
    link.mark_bank(&bank);
    m.edit.update_link(LOCAL, |l| {
        link.role_seen = l.role_seen.clone();
        link.role_seen_by = l.role_seen_by.clone();
        *l = link;
        true
    });
    banks[0] = bank;
    s.keep("Post", "/circles/heads", 200, heads(3));
    let served: Vec<serde_json::Value> = items.iter().map(|(id, stem)| rated(id, stem, Some("High"))).collect();
    s.keep_raw("Get", &version_path(3), served_qs(3, served, false));
    frames(&mut m, &mut banks, &ctx, &mut acct, &olga_named(&s), 4);
    assert_eq!(news(&mut m).map(|n| n.questions.len()), Some(n));
    (m, s, banks, ctx, acct)
}

/// (minor 6) The news list: "Easy -> Hard" never split (also at 420), the Put mine back buttons in one column, "Put all of mine
/// back" when there are more than two, a long list shortened with "Show all", and Dismiss says what it does; the strip's own
/// buttons are set apart below it.
#[test]
fn c98_m6_the_news_list_layout() {
    let (mut m, s, mut banks, ctx, mut acct) = many_news(8);
    let who = olga_named(&s);
    for width in [1280.0, 420.0] {
        let d = drawn(width, &mut |ui| {
            let mut b = browse();
            super::bank_links_ui::strip_with(ui, &mut m, &mut banks, &mut b, &mut acct, &who, now(), NOW, LOCAL);
        });
        let rows: Vec<&(String, egui::Rect, egui::Color32)> = d.iter().filter(|t| t.0.starts_with("Question ")).collect();
        assert_eq!(rows.len(), 5, "a long list is shortened at {width}: {d:?}");
        assert!(rows.iter().all(|r| r.0.contains("Easy\u{a0}->\u{a0}Hard")), "{rows:?}");
        let buttons: Vec<&(String, egui::Rect, egui::Color32)> = d.iter().filter(|t| t.0 == "Put mine back").collect();
        assert_eq!(buttons.len(), 5);
        assert!(buttons.iter().all(|b| (b.1.right() - buttons[0].1.right()).abs() < 1.0), "one column at {width}: {buttons:?}");
        assert!(d.iter().any(|t| t.0 == "Put all of mine back") && d.iter().any(|t| t.0 == "Show all 8"), "{d:?}");
        let dismiss = d.iter().find(|t| t.0 == "Dismiss").unwrap();
        // (critic 102 m1) the words say the list goes for good, with the offer, not that it only hides
        assert!(d.iter().any(|t| t.0.contains("Dismiss removes this list for good")), "{d:?}");
        let history = d.iter().find(|t| t.0 == "History").unwrap();
        assert!(history.1.top() - dismiss.1.bottom() >= 12.0, "set apart at {width}: {dismiss:?} {history:?}");
    }
    click(&ctx, &mut m, &mut banks, &mut acct, &who, "Show all 8", 0);
    let strip = frames(&mut m, &mut banks, &ctx, &mut acct, &who, 1);
    assert_eq!(put_backs(&strip), 8, "{strip}");
    click(&ctx, &mut m, &mut banks, &mut acct, &who, "Put all of mine back", 0);
    // (round 20, critic 100 minor 4) it asks first: nothing has changed yet
    assert!(banks[0].questions.iter().all(|q| q.difficulty == Some(Difficulty::High)), "still the circle's: nothing put back before the yes");
    let asking = frames(&mut m, &mut banks, &ctx, &mut acct, &who, 1);
    assert!(asking.contains("Put yours back on 8 questions?") && asking.contains("Yes, put all back") && asking.contains("Keep them"), "{asking}");
    click(&ctx, &mut m, &mut banks, &mut acct, &who, "Yes, put all back", 0);
    assert!(banks[0].questions.iter().all(|q| q.difficulty == Some(Difficulty::Low)), "all hers again");
    let strip = frames(&mut m, &mut banks, &ctx, &mut acct, &who, 2);
    assert_eq!(put_backs(&strip), 0, "{strip}");
    assert!(!strip.contains("Put all of mine back"), "{strip}");
}

/// (minor 7) The news's words: "An earlier version of the app", never "verSHion had not kept"; a question the circle gives no
/// difficulty says "no difficulty", never "none".
#[test]
fn c98_m7_the_news_words() {
    let (mut m, s, mut banks, ctx, mut acct) = with_news();
    let strip = frames(&mut m, &mut banks, &ctx, &mut acct, &olga_named(&s), 2);
    assert!(strip.contains("An earlier version of the app had not kept") && !strip.contains("An earlier verSHion"), "{strip}");
    assert!(strip.contains("Easy -> no difficulty") && !strip.contains("-> none"), "{strip}");
}

/// (minor 8) While the copy waits for the circle to be checked the badge says that, not "changes not counted"; signed out, the strip
/// says the same.
#[test]
fn c98_m8_waiting_for_the_circle_check() {
    let (mut m, s, mut banks, ctx, mut acct) = rig();
    phase_c(&mut m, &mut banks, 3, None, None);
    banks[0].questions[1].stem = "Two (mine)".into();
    banks[0].modified += 5;
    let out = AccountSnapshot { gate: super::publish::Gate::SignedOut, session: None, account_open: true, account: None, name: None };
    let t = frames(&mut m, &mut banks, &ctx, &mut acct, &out, 2);
    assert_eq!(badge(&mut m, &banks[0].clone()), super::bank_links::WAITING_NOTE);
    assert!(t.contains("Your changes are counted once verSHion can check the circle.") && !t.contains("not counted"), "{t}");
    let _ = s;
    // a copy with no record at all: "changes not counted", with what that means
    assert!(super::bank_links::note_hint("changes not counted").is_some_and(|h| h.contains("can't count")));
}

/// (minor 9) Bank settings: the remove question has its own heading, set apart from "Report this bank...".
#[test]
fn c98_m9_the_remove_panel_has_a_heading_set_apart() {
    let (mut m, s) = open_circle("owner", json!({}));
    let (bank, mut l) = linked(&[(QA, "One"), (QB, "Two")]);
    l.role_seen = Some("owner".into());
    m.edit.add_link(l).unwrap();
    let mut banks = vec![bank];
    m.edit.sync(&mut banks);
    s.keep_raw("Get", &format!("/circles/{CID}/banks/{BID}"), served(1, &[(QA, "One"), (QB, "Two")], false));
    m.open_bank(BID);
    for _ in 0..3 {
        m.frame(now(), &ready(&s));
    }
    m.ask_remove_bank();
    for width in [1280.0, 420.0] {
        let d = drawn(width, &mut |ui| {
            super::circles_ui::body_of(ui, &mut m);
        });
        let report = d.iter().find(|t| t.0 == "Report this bank...").unwrap_or_else(|| panic!("{d:?}"));
        let head = d.iter().find(|t| t.0 == "Remove this bank from the circle?").unwrap_or_else(|| panic!("{d:?}"));
        assert!(head.1.top() - report.1.bottom() >= 12.0, "{width}: {report:?} {head:?}");
    }
}

/// (minor 10) The invite-by-email panel says the code's last day as every circle screen says dates.
#[test]
fn c98_m10_the_invite_date_in_one_form() {
    let line = super::circles_text::code_until(1_792_972_800_000); // 2026-10-26
    assert_eq!(line, "The code in it works once, until Oct 26, 2026.");
}

/// (minor 11) "Use my bank as it is" is titled with her bank's name; the "..." menu's hint names what it offers; the copy saved
/// first is called a safety copy everywhere.
#[test]
fn c98_m11_use_as_is_names_her_bank_the_menu_hint_matches_and_one_word() {
    let (mut m, s, mut banks, ctx, mut acct) = not_kept_rig();
    let who = olga_named(&s);
    click(&ctx, &mut m, &mut banks, &mut acct, &who, "Use my bank as it is...", 0);
    let page = frames(&mut m, &mut banks, &ctx, &mut acct, &who, 1);
    // (round 20, critic 100 minor 8) the circle's title, the same as Get latest, never her bank's own name here and the circle's there
    assert!(page.contains("Use my bank as it is: \"Cells\"") && !page.contains("as it is: \"Unit 2 quiz\""), "{page}");
    // the "..." hint names only what the menu has
    let mut v = super::bank_links::StripView { line: String::new(), buttons: vec![StripButton::History], busy: false, warn: false, menu_history: true, menu_circle_page: true, menu_check: false };
    assert_eq!(super::bank_links_ui::more_hint(&v), "More: circle page, unlink");
    v.menu_check = true;
    v.buttons.clear();
    assert_eq!(super::bank_links_ui::more_hint(&v), "More: history, circle page, check now, unlink");
    // one word for the copy saved first
    let (mut m, s, mut banks, ctx, mut acct) = rig();
    make_stale(&mut m, LOCAL);
    m.edit.sync(&mut banks);
    let strip = frames(&mut m, &mut banks, &ctx, &mut acct, &olga_named(&s), 2);
    assert!(strip.contains("safety copy") && !strip.contains("saved copy"), "{strip}");
}

/// (round 18 mutant h01) A held pass for a question put back after the read ends when that question is deleted again: nothing is
/// left to try, so the bank is not said to be waiting for ever and Send is not held back.
#[test]
fn s18_h01_a_held_undo_pass_ends_when_its_question_is_deleted_again() {
    let ((mut m, s, mut banks, ctx, mut acct), _k) = undo_while_held();
    let who = olga_named(&s);
    frames(&mut m, &mut banks, &ctx, &mut acct, &who, 3);
    assert_eq!(m.edit.status(LOCAL).unwrap().learning, Some(Learning::Held));
    banks[0].questions.remove(0);
    banks[0].modified += 5;
    let strip = frames(&mut m, &mut banks, &ctx, &mut acct, &who, 2);
    assert_eq!(m.edit.status(LOCAL).unwrap().learning, None, "{strip}");
    assert!(!strip.contains("couldn't save") && strip.contains("Send my changes"), "{strip}");
}

/// (round 18 mutant h03) Once a held pass for a question put back after the read has worked, that question is no longer waited
/// for: her change right after it is hers, never taken over by the circle's again.
#[test]
fn s18_h03_after_the_held_undo_pass_worked_her_next_change_is_hers() {
    let ((mut m, s, mut banks, ctx, mut acct), k) = undo_while_held();
    let who = olga_named(&s);
    frames(&mut m, &mut banks, &ctx, &mut acct, &who, 3);
    k.save_fails.store(false, Ordering::SeqCst);
    // (one pass, then her edit in the same frame: the question editor is drawn after the strip)
    super::bank_links_fix16_tests::quiet(&mut m, &mut banks, &who, now() + Duration::from_secs(61), 1);
    assert_eq!(banks[0].questions[0].difficulty, Some(Difficulty::High), "the pass worked");
    banks[0].questions[0].difficulty = Some(Difficulty::Medium);
    banks[0].modified += 5;
    for i in 2..=3u64 {
        page_frame(&mut m, &mut banks, &mut acct, &who, now() + Duration::from_secs(61 * i));
    }
    assert_eq!(banks[0].questions[0].difficulty, Some(Difficulty::Medium), "her change stays");
}

/// (round 18 mutant h11) The links file can't be written before the pass (the bank as the read found it can't be recorded): the
/// pass waits, nothing goes into the banks file, and the strip says why.
#[test]
fn s18_h11_a_links_file_that_cant_be_written_first_holds_the_pass() {
    let (mut m, s, mut banks, ctx, mut acct) = rig();
    let dir = std::env::temp_dir().join(format!("vershion-r18-h11-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    s.keep("Post", "/circles/heads", 200, heads(3));
    phase_c(&mut m, &mut banks, 3, Some(Difficulty::Low), Some(Difficulty::Low));
    let saved: Arc<Mutex<Vec<Vec<QuestionBank>>>> = Arc::default();
    let sv = saved.clone();
    let mut e = on_dir(&dir, Arc::new(move |b: &[QuestionBank]| {
        sv.lock().unwrap().push(b.to_vec());
        Ok(())
    }));
    for l in m.edit.links().to_vec() {
        e.add_link(l).unwrap();
    }
    m.edit = e;
    s.keep_raw("Get", &version_path(3), v3(Some("High"), None));
    let who = olga_named(&s);
    m.edit.frame(now(), &who);
    m.edit.frame(now(), &who);
    assert!(LinkLog::Dir(dir.clone()).load()[0].inherit, "learnt, in the file");
    std::fs::create_dir_all(dir.join("share-circles.json.tmp").join("held")).unwrap();
    assert!(!m.edit.sync(&mut banks));
    assert_eq!(banks[0].questions[0].difficulty, Some(Difficulty::Low), "nothing changed");
    assert!(saved.lock().unwrap().is_empty(), "the banks file was not written");
    let strip = frames(&mut m, &mut banks, &ctx, &mut acct, &who, 1);
    assert_eq!(m.edit.status(LOCAL).unwrap().learning, Some(Learning::Held), "{strip}");
    assert!(strip.contains("couldn't save the list of shared banks"), "{strip}");
    // (round 19, critic 99 m1) the retry a minute later, and Try again, the links file still unwritable: still held, the banks file
    // still not written (the bank as the read found it is in memory only)
    page_frame(&mut m, &mut banks, &mut acct, &who, now() + Duration::from_secs(61));
    assert!(saved.lock().unwrap().is_empty(), "the retry never writes the banks file before the links file");
    m.edit.read_base_again(LOCAL);
    page_frame(&mut m, &mut banks, &mut acct, &who, now() + Duration::from_secs(62));
    assert!(saved.lock().unwrap().is_empty(), "nor does Try again");
    assert_eq!(m.edit.status(LOCAL).unwrap().learning, Some(Learning::Held));
    assert_eq!(banks[0].questions[0].difficulty, Some(Difficulty::Low));
    // once it can be written: the links file first, then the banks file, then the links file says it was done
    std::fs::remove_dir_all(dir.join("share-circles.json.tmp")).unwrap();
    page_frame(&mut m, &mut banks, &mut acct, &who, now() + Duration::from_secs(123));
    assert_eq!((banks[0].questions[0].difficulty, banks[0].questions[1].difficulty), (Some(Difficulty::High), None));
    assert_eq!(saved.lock().unwrap().len(), 1);
    let file = LinkLog::Dir(dir.clone()).load();
    assert!(!file[0].inherit && file[0].difficulty_news.as_ref().is_some_and(|n| n.questions.len() == 2), "{:?}", file[0].difficulty_news);
    let _ = std::fs::remove_dir_all(&dir);
}

/// (round 18 mutant u09) A line sets the news apart from the strip's own buttons below it.
#[test]
fn s18_u09_a_line_sets_the_news_apart_from_the_strip_buttons() {
    let (mut m, s, mut banks, _ctx, mut acct) = with_news();
    let who = olga_named(&s);
    let ctx = ctx();
    let mut found = (None, None, Vec::new());
    for _ in 0..2 {
        let input = egui::RawInput { screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(760.0, 4000.0))), ..Default::default() };
        let out = ctx.run_ui(input, |ui| {
            egui::CentralPanel::default().show(ui, |ui| {
                let mut b = browse();
                super::bank_links_ui::strip_with(ui, &mut m, &mut banks, &mut b, &mut acct, &who, now(), NOW, LOCAL);
            });
        });
        let mut dismiss = None;
        let mut history = None;
        let mut lines = Vec::new();
        for c in &out.shapes {
            match &c.shape {
                egui::epaint::Shape::Text(t) if t.galley.text() == "Dismiss" => dismiss = Some(t.galley.rect.translate(t.pos.to_vec2())),
                egui::epaint::Shape::Text(t) if t.galley.text() == "History" => history = Some(t.galley.rect.translate(t.pos.to_vec2())),
                egui::epaint::Shape::LineSegment { points, .. } if (points[0].y - points[1].y).abs() < 0.5 && (points[1].x - points[0].x).abs() > 200.0 => lines.push(points[0].y),
                _ => {}
            }
        }
        found = (dismiss, history, lines);
        out.drop_without_applying_deltas();
    }
    let (Some(d), Some(h), lines) = found else { panic!("not drawn") };
    assert!(lines.iter().any(|y| *y > d.bottom() && *y < h.top()), "{d:?} {h:?} {lines:?}");
}

/// (round 18 mutant u11) Short rows too: each "Put mine back" in one column, whatever the length of the row's words.
#[test]
fn s18_u11_short_rows_keep_their_buttons_in_one_column() {
    let (mut m, s, mut banks, _ctx, mut acct) = with_news();
    let who = olga_named(&s);
    for width in [1280.0, 420.0] {
        let d = drawn(width, &mut |ui| {
            let mut b = browse();
            super::bank_links_ui::strip_with(ui, &mut m, &mut banks, &mut b, &mut acct, &who, now(), NOW, LOCAL);
        });
        let buttons: Vec<egui::Rect> = d.iter().filter(|t| t.0 == "Put mine back").map(|t| t.1).collect();
        assert_eq!(buttons.len(), 2, "{d:?}");
        assert!((buttons[0].left() - buttons[1].left()).abs() < 1.0, "one column at {width}: {buttons:?}");
    }
}

/// (round 18 mutant u12) Bank settings: a line sets the remove question apart from "Report this bank...".
#[test]
fn s18_u12_a_line_sets_the_remove_question_apart() {
    let (mut m, s) = open_circle("owner", json!({}));
    let (bank, mut l) = linked(&[(QA, "One"), (QB, "Two")]);
    l.role_seen = Some("owner".into());
    m.edit.add_link(l).unwrap();
    let mut banks = vec![bank];
    m.edit.sync(&mut banks);
    s.keep_raw("Get", &format!("/circles/{CID}/banks/{BID}"), served(1, &[(QA, "One"), (QB, "Two")], false));
    m.open_bank(BID);
    for _ in 0..3 {
        m.frame(now(), &ready(&s));
    }
    m.ask_remove_bank();
    let ctx = ctx();
    let mut found = (None, None, Vec::new());
    for _ in 0..2 {
        let input = egui::RawInput { screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(760.0, 6000.0))), ..Default::default() };
        let out = ctx.run_ui(input, |ui| {
            egui::CentralPanel::default().show(ui, |ui| {
                super::circles_ui::body_of(ui, &mut m);
            });
        });
        let (mut report, mut head, mut lines) = (None, None, Vec::new());
        for c in &out.shapes {
            match &c.shape {
                egui::epaint::Shape::Text(t) if t.galley.text() == "Report this bank..." => report = Some(t.galley.rect.translate(t.pos.to_vec2())),
                egui::epaint::Shape::Text(t) if t.galley.text() == "Remove this bank from the circle?" => head = Some(t.galley.rect.translate(t.pos.to_vec2())),
                egui::epaint::Shape::LineSegment { points, .. } if (points[0].y - points[1].y).abs() < 0.5 && (points[1].x - points[0].x).abs() > 200.0 => lines.push(points[0].y),
                _ => {}
            }
        }
        found = (report, head, lines);
        out.drop_without_applying_deltas();
    }
    let (Some(r), Some(h), lines) = found else { panic!("not drawn") };
    assert!(lines.iter().any(|y| *y > r.bottom() && *y < h.top()), "{r:?} {h:?} {lines:?}");
}
