//! Phase D1, round 16 (critics 93 and 94): each test written first against the code it pins. The owner's rule stays (after
//! one successful read of the circle's version the copy takes the circle's difficulty; a later deliberate change of hers
//! supersedes), and it is made recoverable: a restore point before any rating of hers is replaced, a line that says so, a
//! rating she already pressed Send for kept, the banks file written before the links file says it was done, and "Use my bank
//! as it is" settling the read whatever arrives first.

use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use serde_json::{Value, json};

use super::bank_links_fix10_tests::*;
use super::bank_links_fix11_tests::{host_a, rated, rig, version_path, version_reads};
use super::bank_links_fix15_tests::phase_c;
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

/// The circle's version 3: One rated `one`, Two rated `two`.
pub(super) fn v3(one: Option<&str>, two: Option<&str>) -> Vec<u8> {
    served_qs(3, vec![rated(QA, "One", one), rated(QB, "Two", two)], false)
}

/// What A's next send carries as question `i`'s difficulty.
pub(super) fn sent_q(m: &mut CirclesModel, banks: &[QuestionBank], i: usize) -> Value {
    let link = m.edit.link_for_local(LOCAL).unwrap();
    let sent: Value = serde_json::from_slice(&super::circle_links::payload_of(&link, &banks[0]).unwrap()).unwrap();
    sent["bank"]["questions"][i]["difficulty"].clone()
}

/// Restore points, counted with what they hold; `fail` makes the next ones fail (each attempt is counted too).
struct Counting {
    n: Arc<Mutex<usize>>,
    fail: Arc<AtomicBool>,
    tries: Arc<Mutex<usize>>,
    held: Arc<Mutex<Vec<Vec<QuestionBank>>>>,
}

impl super::browse::RestoreSink for Counting {
    fn save(&self, banks: &[QuestionBank]) -> Result<String, String> {
        *self.tries.lock().unwrap() += 1;
        if self.fail.load(Ordering::SeqCst) {
            return Err("the disk is full".into());
        }
        *self.n.lock().unwrap() += 1;
        self.held.lock().unwrap().push(banks.to_vec());
        Ok("backups/banks-before-merge-test.json".into())
    }
}

/// What the rebuilt model was given: its restore points, the banks files written (with whether the links file still said the
/// ratings were to be taken when each was written), and switches to make either fail.
pub(super) struct Sinks {
    pub(super) restores: Arc<Mutex<usize>>,
    pub(super) restore_tries: Arc<Mutex<usize>>,
    pub(super) restored: Arc<Mutex<Vec<Vec<QuestionBank>>>>,
    pub(super) restore_fails: Arc<AtomicBool>,
    pub(super) saves: Arc<Mutex<Vec<(Vec<QuestionBank>, bool)>>>,
    pub(super) save_fails: Arc<AtomicBool>,
    pub(super) log: Arc<Mutex<Vec<CircleLink>>>,
}

/// The same links in an Inline model whose restore points and banks files are counted, its links file kept in `log`.
pub(super) fn rebuilt(m: &mut CirclesModel) -> Sinks {
    let k = Sinks { restores: Arc::default(), restore_tries: Arc::default(), restored: Arc::default(), restore_fails: Arc::default(), saves: Arc::default(), save_fails: Arc::default(), log: Arc::default() };
    let media: super::publish::SharedMedia = Arc::new(HashMap::<String, Vec<u8>>::new());
    let (saves, fails, log) = (k.saves.clone(), k.save_fails.clone(), k.log.clone());
    let saver: BankSaver = Arc::new(move |banks: &[QuestionBank]| {
        if fails.load(Ordering::SeqCst) {
            return Err("the disk is full".into());
        }
        let pending = log.lock().unwrap().first().is_some_and(|l| l.inherit);
        saves.lock().unwrap().push((banks.to_vec(), pending));
        Ok(())
    });
    let restore = Counting { n: k.restores.clone(), fail: k.restore_fails.clone(), tries: k.restore_tries.clone(), held: k.restored.clone() };
    let mut e = CircleEditModel::new(super::model::Runner::Inline, Arc::new(Mem::default()), media, Arc::new(restore), LinkLog::Memory(k.log.clone())).with_retry(super::pool::RetryPolicy::none()).with_bank_saver(saver);
    e.set_clock(NOW);
    for l in m.edit.links().to_vec() {
        e.add_link(l).unwrap();
    }
    m.edit = e;
    k
}

pub(super) fn said(m: &CirclesModel) -> Vec<String> {
    m.edit.results_of(LOCAL).into_iter().map(|(_, t)| t).collect()
}

pub(super) fn quiet(m: &mut CirclesModel, banks: &mut [QuestionBank], who: &AccountSnapshot, at: std::time::Instant, n: usize) {
    for _ in 0..n {
        m.edit.frame(at, who);
        m.edit.sync(banks);
    }
}

// ---- c93 I1, c94 O1: a restore point first, and a line that says it ---------------------------------------------------------------

/// (p93_1) Her own ratings are replaced by the circle's at the read (the owner's rule: Two's Low by the circle's none, One's Low
/// by High), after a restore point, and the strip says so once, with the version, beside "up to date". A merge after it takes
/// its own restore point as before.
#[test]
fn c93_i1_the_read_saves_a_restore_point_first_and_the_strip_says_so() {
    let (mut m, s, mut banks, ctx, mut acct) = rig();
    let k = rebuilt(&mut m);
    s.keep("Post", "/circles/heads", 200, heads(3));
    phase_c(&mut m, &mut banks, 3, Some(Difficulty::Low), Some(Difficulty::Low));
    s.keep_raw("Get", &version_path(3), v3(Some("High"), None));
    let who = olga_named(&s);
    let strip = frames(&mut m, &mut banks, &ctx, &mut acct, &who, 3);
    assert_eq!((banks[0].questions[0].difficulty, banks[0].questions[1].difficulty), (Some(Difficulty::High), None), "{strip}");
    assert_eq!(*k.restores.lock().unwrap(), 1, "one restore point before the overwrite");
    let held = k.restored.lock().unwrap()[0][0].clone();
    assert_eq!((held.questions[0].difficulty, held.questions[1].difficulty), (Some(Difficulty::Low), Some(Difficulty::Low)), "it holds her ratings");
    // (round 17, critic 96 I1) said on the link's news (no file named as if it could be put back here)
    assert!(strip.contains("verSHion gave 2 questions the circle's difficulty from its version 3, in place of yours") && !strip.contains(".json"), "{strip}");
    assert!(strip.contains("up to date"), "{strip}");
    assert!(said(&m).is_empty(), "{:?}", said(&m));
    // once: later passes say nothing new and take no other restore point
    frames(&mut m, &mut banks, &ctx, &mut acct, &who, 3);
    assert_eq!(*k.restores.lock().unwrap(), 1);
    assert_eq!(m.edit.with_link(LOCAL, |l| l.difficulty_news.as_ref().map(|n| n.questions.len())), Some(Some(2)));
    get_latest(&mut m, &s, &mut banks, &ctx, &mut acct, served_qs(4, vec![rated(QA, "One (b)", Some("High")), rated(QB, "Two", None)], false));
    apply(&mut m, &s, &mut banks, &ctx, &mut acct);
    assert_eq!(*k.restores.lock().unwrap(), 2);
}

/// Where the read changes nothing (her ratings are the circle's) there is no restore point and nothing to say.
#[test]
fn c93_i1_a_read_that_changes_nothing_says_nothing() {
    let (mut m, s, mut banks, ctx, mut acct) = rig();
    let k = rebuilt(&mut m);
    s.keep("Post", "/circles/heads", 200, heads(3));
    phase_c(&mut m, &mut banks, 3, Some(Difficulty::High), None);
    s.keep_raw("Get", &version_path(3), v3(Some("High"), None));
    let who = olga_named(&s);
    let strip = frames(&mut m, &mut banks, &ctx, &mut acct, &who, 3);
    assert_eq!(*k.restores.lock().unwrap(), 0);
    assert!(said(&m).is_empty() && !strip.contains("the circle's difficulty from its version"), "{strip}");
    assert_eq!(m.edit.with_link(LOCAL, |l| l.difficulty_news.is_none()), Some(true));
    assert_eq!(m.edit.with_link(LOCAL, |l| l.inherit), Some(false));
}

/// (p93_1b) A rating she already pressed Send for, whose send did not go through: the link records what she sent (Two: Low,
/// the circle's version 3: none). The read keeps it: Two stays Low, it is her unsent change, and her next send carries it.
#[test]
fn c93_i1b_a_rating_she_pressed_send_for_is_kept() {
    for not_through in [true, false] {
        let (mut m, s, mut banks, ctx, mut acct) = rig();
        s.keep("Post", "/circles/heads", 200, heads(3));
        phase_c(&mut m, &mut banks, 3, None, Some(Difficulty::Low));
        let b = banks[0].clone();
        m.edit.update_link(LOCAL, |l| {
            let mut r = super::circle_links::sent_record(l, &b, 3, String::new());
            r.not_through = not_through;
            l.sent_record = Some(r);
            true
        });
        s.keep_raw("Get", &version_path(3), v3(Some("High"), None));
        let who = olga_named(&s);
        let strip = frames(&mut m, &mut banks, &ctx, &mut acct, &who, 3);
        assert_eq!(banks[0].questions[1].difficulty, Some(Difficulty::Low), "{not_through}: {strip}");
        assert_eq!(banks[0].questions[0].difficulty, Some(Difficulty::High), "One was not hers to send: the circle's");
        assert_eq!(sent_q(&mut m, &banks, 1), json!("Low"));
    }
}

/// Another account's send record is not hers: the rule applies.
#[test]
fn c93_i1b_another_accounts_send_record_is_not_kept() {
    let (mut m, s, mut banks, ctx, mut acct) = rig();
    s.keep("Post", "/circles/heads", 200, heads(3));
    phase_c(&mut m, &mut banks, 3, None, Some(Difficulty::Low));
    let b = banks[0].clone();
    m.edit.update_link(LOCAL, |l| {
        let mut r = super::circle_links::sent_record(l, &b, 3, "another account".into());
        r.not_through = true;
        l.sent_record = Some(r);
        true
    });
    s.keep_raw("Get", &version_path(3), v3(None, None));
    let who = olga_named(&s);
    frames(&mut m, &mut banks, &ctx, &mut acct, &who, 3);
    assert_eq!(banks[0].questions[1].difficulty, None);
}

/// The restore point can't be saved: nothing in the bank changes, the links file still says the ratings are to be taken, Send
/// waits (it would send her ratings over the circle's), and the strip says why. A minute later it is tried again and works.
#[test]
fn c93_i1_no_restore_point_no_change_and_no_send_until_it_works() {
    let (mut m, s, mut banks, ctx, mut acct) = rig();
    let k = rebuilt(&mut m);
    s.keep("Post", "/circles/heads", 200, heads(3));
    phase_c(&mut m, &mut banks, 3, Some(Difficulty::Low), None);
    banks[0].questions[1].stem = "Two (mine)".into();
    banks[0].modified += 30;
    k.restore_fails.store(true, Ordering::SeqCst);
    s.keep_raw("Get", &version_path(3), v3(Some("High"), None));
    let who = olga_named(&s);
    let strip = frames(&mut m, &mut banks, &ctx, &mut acct, &who, 3);
    assert_eq!(banks[0].questions[0].difficulty, Some(Difficulty::Low), "{strip}");
    assert!(k.saves.lock().unwrap().is_empty());
    assert_eq!(k.log.lock().unwrap()[0].inherit, true);
    // (round 17, critic 96 minor 4) in plain words, and the badge says it too
    assert!(strip.contains("verSHion couldn't save a safety copy of your banks first (the disk is full)"), "{strip}");
    assert!(!strip.contains("Send my changes") && !strip.contains("up to date"), "{strip}");
    assert_eq!(m.edit.status(LOCAL).unwrap().learning, Some(Learning::Held));
    assert!(!m.edit.open_send(LOCAL, owner()));
    assert_eq!(badge(&mut m, &banks[0].clone()), "couldn't save");
    assert!(strip.contains("Try again"), "{strip}");
    let l = m.edit.link_for_local(LOCAL).unwrap();
    assert_eq!(super::circle_edit_ui::standing(&l, m.edit.status(LOCAL), false, super::circles_api::Role::Owner), vec![super::circles_text::DIFFICULTY_HELD.to_string()]);
    assert_eq!(*k.restore_tries.lock().unwrap(), 1, "not tried again at every frame");
    k.restore_fails.store(false, Ordering::SeqCst);
    let mut strip = String::new();
    for _ in 0..3 {
        strip = page_frame(&mut m, &mut banks, &mut acct, &who, now() + Duration::from_secs(61)).1;
    }
    assert_eq!(banks[0].questions[0].difficulty, Some(Difficulty::High), "{strip}");
    assert!(strip.contains("verSHion gave 1 question the circle's difficulty from its version 3, in place of yours") && strip.contains("Send my changes"), "{strip}");
    assert!(!strip.contains("couldn't save a safety copy"), "the line is replaced: {strip}");
}

// ---- c93 minor 1: the banks file first, then the links file --------------------------------------------------------------------

/// (p93_2) The banks are written while the links file still says the ratings are to be taken; only then does the links file say
/// it was done. A bank file that can't be written leaves the bank and the links file as they were (nothing is half done), and
/// after a restart the read's ratings are taken into the bank as it is on disk.
#[test]
fn c93_m1_the_banks_file_is_written_before_the_links_file_says_done() {
    let (mut m, s, mut banks, _ctx, _acct) = rig();
    let k = rebuilt(&mut m);
    phase_c(&mut m, &mut banks, 3, Some(Difficulty::Low), None);
    s.keep_raw("Get", &version_path(3), v3(Some("High"), None));
    let who = olga_named(&s);
    quiet(&mut m, &mut banks, &who, now(), 3);
    let saves = k.saves.lock().unwrap().clone();
    assert_eq!(saves.len(), 1, "written once");
    assert!(saves[0].1, "the links file still said 'to be taken' when the banks were written");
    assert_eq!(saves[0].0[0].questions[0].difficulty, Some(Difficulty::High));
    assert!(!k.log.lock().unwrap()[0].inherit, "then done");
    // the same with a banks file that can't be written
    let (mut m, s, mut banks, ctx, mut acct) = rig();
    let k = rebuilt(&mut m);
    phase_c(&mut m, &mut banks, 3, Some(Difficulty::Low), None);
    let on_disk = banks.clone();
    k.save_fails.store(true, Ordering::SeqCst);
    s.keep_raw("Get", &version_path(3), v3(Some("High"), None));
    let who = olga_named(&s);
    let strip = frames(&mut m, &mut banks, &ctx, &mut acct, &who, 3);
    assert_eq!(banks[0].questions[0].difficulty, Some(Difficulty::Low), "nothing changed: {strip}");
    assert!(k.log.lock().unwrap()[0].inherit, "still to be taken");
    assert!(strip.contains("verSHion couldn't save your banks (the disk is full)"), "{strip}");
    // the app ends; on the next start the read's ratings are taken into the bank on disk
    let mut m = restarted(k.log.clone());
    let mut banks = on_disk;
    m.edit.frame(now(), &who);
    assert!(m.edit.sync(&mut banks));
    assert_eq!(banks[0].questions[0].difficulty, Some(Difficulty::High));
    assert_eq!(changes(&mut m, &banks[0].clone()), (0, 0, 0));
    assert_eq!(sent_q(&mut m, &banks, 0), json!("High"));
}

// ---- c93 I2: "Use my bank as it is" settles the read, whatever arrives first ---------------------------------------------------------

/// (p93_3 turned around) The read can't reach the circle; she uses her bank as it is; the read lands a minute later. Her bank
/// stays as it is and her next send carries her Low, as the app said.
#[test]
fn c93_i2_use_my_bank_as_it_is_then_the_read_changes_nothing() {
    let (mut m, s, mut banks, _ctx, _acct) = rig();
    s.send_base_404(true);
    phase_c(&mut m, &mut banks, 3, Some(Difficulty::Low), None);
    make_stale(&mut m, LOCAL);
    s.on("Get", &version_path(3), 503, json!({"error": "unavailable"}));
    s.keep_raw("Get", &version_path(3), v3(Some("High"), None));
    let who = olga_named(&s);
    treat_in_step(&mut m, &s, &mut banks, LOCAL, BID, false);
    for k in 1..6u64 {
        quiet(&mut m, &mut banks, &who, now() + Duration::from_secs(61 * k), 1);
    }
    assert_eq!(banks[0].questions[0].difficulty, Some(Difficulty::Low));
    assert_eq!(sent_q(&mut m, &banks, 0), json!("Low"));
    assert_eq!(m.edit.status(LOCAL).unwrap().learning, None, "nothing awaited: Send is offered");
    assert!(m.edit.open_send(LOCAL, owner()));
}

/// The same with a read on its way when she confirms: its answer is not learnt into the link.
#[test]
fn c93_i2_a_read_on_its_way_when_she_confirms_is_not_learnt() {
    let (mut m, s, mut banks, _ctx, _acct) = rig();
    phase_c(&mut m, &mut banks, 3, Some(Difficulty::Low), None);
    make_stale(&mut m, LOCAL);
    background(&mut m);
    s.keep_raw("Get", &version_path(3), v3(Some("High"), None));
    let who = olga_named(&s);
    let release = s.hold();
    quiet(&mut m, &mut banks, &who, now(), 1);
    for _ in 0..200 {
        if version_reads(&s, 3) >= 1 {
            break;
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    // she confirms while the read is still on its way
    treat_in_step(&mut m, &s, &mut banks, LOCAL, BID, true);
    assert_eq!(version_reads(&s, 3), 1, "{:?}", s.paths());
    release.send(()).unwrap();
    for _ in 0..200 {
        quiet(&mut m, &mut banks, &who, now(), 1);
        if !m.edit.busy() {
            break;
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    quiet(&mut m, &mut banks, &who, now(), 2);
    assert_eq!(banks[0].questions[0].difficulty, Some(Difficulty::Low));
    assert_eq!(sent_q(&mut m, &banks, 0), json!("Low"));
}

/// (p93_3b) The read landing first: her Low stands and is sent. The same outcome as above.
#[test]
fn c93_i2_the_read_first_then_use_my_bank_keeps_hers_too() {
    let (mut m, s, mut banks, _ctx, _acct) = rig();
    phase_c(&mut m, &mut banks, 3, Some(Difficulty::Low), None);
    make_stale(&mut m, LOCAL);
    s.keep_raw("Get", &version_path(3), v3(Some("High"), None));
    let who = olga_named(&s);
    quiet(&mut m, &mut banks, &who, now(), 3);
    treat_in_step(&mut m, &s, &mut banks, LOCAL, BID, false);
    quiet(&mut m, &mut banks, &who, now() + Duration::from_secs(61), 3);
    assert_eq!(sent_q(&mut m, &banks, 0), json!("Low"));
}

// ---- c93 minors ---------------------------------------------------------------------------------------------------------------------

/// (p93_4) She deleted One before the read, then pressed Undo after it: One comes back and takes the circle's rating (with a
/// restore point and the line), so nothing is counted as hers and her next send keeps the colleague's "High".
#[test]
fn c93_m2_a_question_put_back_after_the_read_takes_the_circles_rating() {
    let (mut m, s, mut banks, ctx, mut acct) = rig();
    let k = rebuilt(&mut m);
    s.keep("Post", "/circles/heads", 200, heads(3));
    phase_c(&mut m, &mut banks, 3, None, None);
    let removed = banks[0].questions.remove(0);
    banks[0].modified += 5;
    s.keep_raw("Get", &version_path(3), v3(Some("High"), None));
    let who = olga_named(&s);
    frames(&mut m, &mut banks, &ctx, &mut acct, &who, 3);
    assert_eq!(*k.restores.lock().unwrap(), 0, "nothing of hers replaced yet");
    // Undo
    banks[0].questions.insert(0, removed);
    banks[0].modified += 5;
    let strip = frames(&mut m, &mut banks, &ctx, &mut acct, &who, 3);
    assert_eq!(banks[0].questions[0].difficulty, Some(Difficulty::High), "{strip}");
    assert_eq!(changes(&mut m, &banks[0].clone()), (0, 0, 0), "{strip}");
    assert_eq!(sent_q(&mut m, &banks, 0), json!("High"));
    assert_eq!(*k.restores.lock().unwrap(), 1);
    assert!(strip.contains("Question 1, \"One\": no difficulty -> Hard"), "{strip}");
    // her deliberate change after that is hers
    banks[0].questions[0].difficulty = Some(Difficulty::Low);
    banks[0].modified += 5;
    frames(&mut m, &mut banks, &ctx, &mut acct, &who, 3);
    assert_eq!(banks[0].questions[0].difficulty, Some(Difficulty::Low));
    assert_eq!(changes(&mut m, &banks[0].clone()), (0, 1, 0));
}

/// A question deleted before the read and put back after a send is not the circle's any more: nothing is taken into it.
#[test]
fn c93_m2_after_a_send_a_question_put_back_is_left_alone() {
    let (mut m, s, mut banks, _ctx, _acct) = rig();
    phase_c(&mut m, &mut banks, 3, None, None);
    let removed = banks[0].questions.remove(0);
    banks[0].modified += 5;
    s.keep_raw("Get", &version_path(3), v3(Some("High"), None));
    let who = olga_named(&s);
    quiet(&mut m, &mut banks, &who, now(), 3);
    // the send of the bank without One: its base is now the bank as sent
    let bank = banks[0].clone();
    m.edit.update_link(LOCAL, |l| {
        let sent = super::circles_api::SentBank { version: 4, version_id: hex(4), qids: vec![QB.into()], ..Default::default() };
        *l = super::circle_links::after_send(l, &bank, &sent);
        true
    });
    banks[0].questions.insert(0, removed);
    banks[0].modified += 5;
    quiet(&mut m, &mut banks, &who, now(), 3);
    assert_eq!(banks[0].questions[0].difficulty, None);
}

/// The leave panel, for a copy still waiting for the read: Send is not offered, so the panel never says "Send it first"; a
/// waiting copy is named whether or not something is counted, with what it waits for.
#[test]
fn c93_m3_the_leave_panel_names_a_waiting_copy_without_send_it_first() {
    for unsent in [1usize, 0] {
        let (mut m, _s) = open_circle("editor", json!({}));
        let (mut bank, mut l) = linked(&[(QA, "One"), (QB, "Two")]);
        l.role_seen = Some("editor".into());
        for b in &mut l.base {
            b.snap.difficulty = None;
        }
        l.difficulty_read = None;
        m.edit.add_link(l).unwrap();
        for q in bank.questions.iter_mut().take(unsent) {
            q.stem.push_str(" (mine)");
        }
        let mut banks = vec![bank];
        m.edit.sync(&mut banks);
        assert!(!m.edit.open_send(LOCAL, owner()));
        m.ask_leave();
        let t = text_of(760.0, &mut |ui| {
            super::circles_ui::body_of(ui, &mut m);
        });
        assert!(!t.contains("Send it first") && !t.contains("Send them first"), "{unsent}: {t}");
        // (round 17, critic 96 minor 2) never "stay until then": where she can act
        assert!(t.contains("\"Unit 2 quiz\" may have changes that are not sent, and they can't be sent yet. Its page in My Banks says why and what you can do."), "{unsent}: {t}");
    }
}

/// (p93_7) The circle's daily limit on the read: it is not "can't reach". Asked again when the circle said, not every minute; the
/// strip, the badge and the circle page row say the limit.
#[test]
fn c93_m4_the_daily_limit_on_the_read_is_said_and_waited_out() {
    let (mut m, s, mut banks, ctx, mut acct) = rig();
    s.keep("Post", "/circles/heads", 200, heads(3));
    phase_c(&mut m, &mut banks, 3, None, None);
    // (round 17, critic 95 minor 6) the wait in the Retry-After header, where the app reads it (a body value is not read)
    s.on_wait("Get", &version_path(3), 429, json!({"error": "daily_limit", "metric": "item_reads"}), 36_000);
    s.keep("Get", &version_path(3), 429, json!({"error": "daily_limit", "metric": "item_reads"}));
    let who = olga_named(&s);
    let t0 = now();
    for k in 0..5u64 {
        quiet(&mut m, &mut banks, &who, t0 + Duration::from_secs(k * 61), 1);
    }
    assert_eq!(version_reads(&s, 3), 1, "not every minute");
    // (survivor m25) nor at the 30 minutes used when the circle gives no time
    quiet(&mut m, &mut banks, &who, t0 + BASE_UNREADABLE_RETRY + Duration::from_secs(1), 1);
    assert_eq!(version_reads(&s, 3), 1, "the circle's time, not 30 minutes");
    assert_eq!(m.edit.status(LOCAL).unwrap().learning, Some(Learning::Limited));
    let strip = frames(&mut m, &mut banks, &ctx, &mut acct, &who, 1);
    // (round 18, critic 98 minor 1) the account's limit
    assert!(strip.contains("Your daily limit for reading banks has been reached. It resets in about") && !strip.contains("Couldn't check") && !strip.contains("Try again"), "{strip}");
    assert!(!m.edit.open_send(LOCAL, owner()));
    let l = m.edit.link_for_local(LOCAL).unwrap();
    let row = super::circle_edit_ui::standing(&l, m.edit.status(LOCAL), false, super::circles_api::Role::Owner);
    assert_eq!(row, vec![super::circles_text::READ_LIMIT.to_string()]);
    // when the limit resets it is asked again (the answer was read at the next frame, t0 + 61 s)
    quiet(&mut m, &mut banks, &who, t0 + Duration::from_secs(36_000 + 60), 1);
    assert_eq!(version_reads(&s, 3), 1, "not before");
    quiet(&mut m, &mut banks, &who, t0 + Duration::from_secs(36_000 + 62), 1);
    assert_eq!(version_reads(&s, 3), 2);
}

/// (q93_4) "Not kept" with one reworded question: the strip, the badge, the circle page row and the leave panel say the same
/// (1 unsent change, sending turned off until the circle can be checked).
#[test]
fn c93_m5_not_kept_strip_badge_row_and_leave_panel_agree() {
    let (mut m, s, mut banks, ctx, mut acct) = rig();
    s.keep("Post", "/circles/heads", 200, heads(3));
    phase_c(&mut m, &mut banks, 3, None, None);
    super::bank_links_fix12_tests::not_kept(&s, 3);
    banks[0].questions[1].stem = "Two (mine)".into();
    banks[0].modified += 30;
    let who = olga_named(&s);
    let strip = frames(&mut m, &mut banks, &ctx, &mut acct, &who, 3);
    // (round 17, critic 96 minor 1) why sending is off, plainly, and where she can act
    // (round 18, critic 98 minor 3) said once, as the circle's newest version
    assert!(strip.contains("You have 1 unsent change.") && strip.contains(&super::circles_text::cant_read_line(3, Unread::NotKept, true)), "{strip}");
    assert_eq!(badge(&mut m, &banks[0].clone()), "1 unsent change");
    let l = m.edit.link_for_local(LOCAL).unwrap();
    let row = super::circle_edit_ui::standing(&l, m.edit.status(LOCAL), false, super::circles_api::Role::Owner).join(" ");
    assert!(row.contains("Not sent yet: 0 added, 1 changed, 0 removed.") && row.contains(super::circles_text::NOT_KEPT_ROW), "{row}");
    assert_eq!(m.edit.unsent_in(CID), vec![("Unit 2 quiz".to_string(), Some(1), true)]);
    assert_eq!(super::circles_text::leave_unsent("Unit 2 quiz", Some(1), true), "\"Unit 2 quiz\" has 1 unsent change that can't be sent yet. Its page in My Banks says why and what you can do.");
}

// ---- critic 94 minors -------------------------------------------------------------------------------------------------------------

/// (m1) The version the copy last matched can't be read and nothing newer is there: the strip says what she can do, and "Use my
/// bank as it is..." settles it (her bank is sent as it is; a difficulty change is said not to be counted).
#[test]
fn c94_m1_a_version_that_cant_be_read_says_what_she_can_do() {
    let (mut m, s, mut banks, ctx, mut acct) = rig();
    s.send_base_404(true);
    s.keep("Post", "/circles/heads", 200, heads(3));
    phase_c(&mut m, &mut banks, 3, Some(Difficulty::Low), None);
    super::bank_links_fix12_tests::not_kept(&s, 3);
    let who = olga_named(&s);
    let strip = frames(&mut m, &mut banks, &ctx, &mut acct, &who, 3);
    assert!(strip.contains(super::circles_text::NOT_KEPT_WAYS) && strip.contains("Use my bank as it is..."), "{strip}");
    // (survivor m37) the "..." menu does not repeat the strip's Check now
    let mut b = browse();
    assert!(super::circles_phasec_tests::click_text(&ctx, 1280.0, "...", 0, &mut |ui| {
        super::bank_links_ui::strip_with(ui, &mut m, &mut banks, &mut b, &mut acct, &who, now(), NOW, LOCAL);
    }));
    let menu = strip_as(&ctx, &mut m, &mut banks, &mut acct, &who, LOCAL);
    assert!(menu.contains("Circle page"), "the menu is open: {menu}");
    // (round 19: the strip's words name Check now too; one button)
    assert_eq!(menu.lines().filter(|l| *l == "Check now").count(), 1, "{menu}");
    s.on("Post", &format!("/circles/{CID}/banks/{BID}/merge"), 400, json!({"error": "invalid_request", "field": "base_version"}));
    assert!(m.edit.open_merge_to_rebase(LOCAL, owner()));
    host_a(&mut m);
    let mut page = String::new();
    for _ in 0..20 {
        page = frames(&mut m, &mut banks, &ctx, &mut acct, &who, 1);
        if matches!(&m.edit.view, EditView::Merge(v) if matches!(v.phase, MergePhase::StaleUpToDate { .. })) {
            break;
        }
    }
    // (round 17, critic 96 I2) asked at once, on its own page
    assert!(page.contains(super::circles_text::AS_IS_TEXT) && page.contains("Yes, use it as it is"), "{page}");
    m.edit.rebaseline();
    m.edit.sync(&mut banks);
    m.edit.close();
    let strip = frames(&mut m, &mut banks, &ctx, &mut acct, &who, 3);
    assert!(strip.contains("Send my changes") || strip.contains("can't be counted"), "{strip}");
    assert_eq!(m.edit.status(LOCAL).unwrap().learning, None);
    assert!(m.edit.open_send(LOCAL, owner()));
    assert_eq!(sent_q(&mut m, &banks, 0), json!("Low"));
}

/// (m2) A first failure of the read (the page just loaded) still says "Checking"; it is asked again within seconds, and only a
/// second failure says it couldn't check, in the same words as the strip's offline line.
#[test]
fn c94_m2_one_failure_is_not_yet_cant_check() {
    let (mut m, s, mut banks, ctx, mut acct) = rig();
    s.keep("Post", "/circles/heads", 200, heads(3));
    phase_c(&mut m, &mut banks, 3, None, None);
    s.keep("Get", &version_path(3), 503, json!({"error": "unavailable"}));
    let who = olga_named(&s);
    let t0 = now();
    let strip = (0..3).map(|_| page_frame(&mut m, &mut banks, &mut acct, &who, t0).1).last().unwrap();
    assert_eq!(version_reads(&s, 3), 1);
    assert!(strip.contains("Checking the circle...") && !strip.contains("Couldn't check"), "{strip}");
    let strip = (0..3).map(|_| page_frame(&mut m, &mut banks, &mut acct, &who, t0 + Duration::from_secs(6)).1).last().unwrap();
    assert_eq!(version_reads(&s, 3), 2);
    assert!(strip.contains(super::circles_text::CANT_CHECK) && strip.contains("Try again"), "{strip}");
    assert!(super::circles_text::CANT_CHECK.starts_with("Couldn't check the circle just now"), "one wording for offline");
    let _ = ctx;
}

/// (m2) Get latest waiting more than 15 s for a read that is only slow says it is still checking; one that has failed says it
/// couldn't check. (m7) Either way "nothing in your bank" is said once.
#[test]
fn c94_m2_m7_get_latest_waiting_says_still_checking_unless_it_failed() {
    for failed in [false, true] {
        let (mut m, s, mut banks, _ctx, mut acct) = rig();
        s.keep("Post", "/circles/heads", 200, heads(4));
        phase_c(&mut m, &mut banks, 3, None, None);
        background(&mut m);
        if failed {
            s.on("Get", &version_path(3), 503, json!({"error": "unavailable"}));
        }
        s.keep_raw("Get", &version_path(3), v3(Some("High"), None));
        let who = olga_named(&s);
        let t0 = now();
        if failed {
            quiet(&mut m, &mut banks, &who, t0, 1);
            for _ in 0..200 {
                quiet(&mut m, &mut banks, &who, t0, 1);
                if !m.edit.busy() {
                    break;
                }
                std::thread::sleep(Duration::from_millis(10));
            }
        }
        let release = s.hold();
        let at = if failed { t0 + Duration::from_secs(6) } else { t0 };
        quiet(&mut m, &mut banks, &who, at, 1);
        for _ in 0..200 {
            if version_reads(&s, 3) >= if failed { 2 } else { 1 } {
                break;
            }
            std::thread::sleep(Duration::from_millis(10));
        }
        s.on("Post", &format!("{}/merge", put_path()), 200, merge_start(3, 4));
        s.keep_raw("Get", &put_path(), served_qs(4, vec![rated(QA, "One", Some("High")), rated(QB, "Two", None)], false));
        assert!(m.edit.open_merge(LOCAL, owner()));
        host_a(&mut m);
        for _ in 0..200 {
            quiet(&mut m, &mut banks, &who, at, 1);
            if matches!(&m.edit.view, EditView::Merge(v) if matches!(v.phase, MergePhase::Ready(_))) {
                break;
            }
            std::thread::sleep(Duration::from_millis(10));
        }
        let page = page_frame(&mut m, &mut banks, &mut acct, &who, at + Duration::from_secs(20)).1;
        let want = if failed { super::circles_text::CHECKING_LONG } else { super::circles_text::STILL_CHECKING };
        assert!(page.contains(want), "{failed}: {page}");
        assert_eq!(page.matches("nothing in your bank").count() + page.matches("Nothing in your bank").count(), 1, "{failed}: {page}");
        release.send(()).unwrap();
    }
}

/// (m4) The circle's newest version held back for review while the copy waits for the read: no Send, and the strip says why.
#[test]
fn c94_m4_no_send_while_waiting_is_said() {
    let (mut m, s, mut banks, ctx, mut acct) = rig();
    phase_c(&mut m, &mut banks, 3, None, None);
    super::bank_links_fix12_tests::not_kept(&s, 3);
    banks[0].questions[1].stem = "Two (mine)".into();
    banks[0].modified += 30;
    s.keep("Post", "/circles/heads", 200, json!({"banks": [{"bank_id": BID, "status": "under_review"}]}));
    m.links.check_now(LOCAL);
    let strip = frames(&mut m, &mut banks, &ctx, &mut acct, &olga_named(&s), 3);
    assert!(strip.contains("held back for review") && !strip.contains("Send my changes"), "{strip}");
    assert!(strip.contains(super::circles_text::SEND_WAITS), "{strip}");
}

/// (m9) Signed out: the strip says the unsent change the badge counts, and the date as the page says dates.
#[test]
fn c94_m9_signed_out_strip_says_the_unsent_change_and_the_pages_date() {
    use super::bank_links::{Check, SignIn, StripInput, strip_view};
    let (mut m, _s, mut banks, _ctx, _acct) = rig();
    banks[0].questions[0].stem = "One (mine)".into();
    banks[0].modified += 30;
    let st = m.edit.status_of(&banks[0].clone()).unwrap();
    let mut l = m.edit.link_for_local(LOCAL).unwrap();
    let at = NOW - 90 * 86_400_000;
    l.role_seen_at = Some(at);
    let v = strip_view(&StripInput { link: &l, status: Some(&st), check: Check::Done, sign_in: SignIn::Out, now_ms: NOW, act: None, sent_unread: false, changed_since: false, unsure: false });
    let date = crate::bankui::fmt_date(at / 1000);
    assert!(v.line.contains(&format!("when last checked, {date}.")) && v.line.contains("You have 1 unsent change."), "{}", v.line);
    assert_eq!(super::bank_links::badge_of(&l, Some(&st)).note.as_deref(), Some("1 unsent change"));
    // as the page: a month ago is a date
    let at = NOW - 45 * 86_400_000;
    assert_eq!(super::circles_text::as_on_page(at, NOW), crate::bankui::fmt_date(at / 1000));
    assert_eq!(super::circles_text::as_on_page(NOW - 2 * 86_400_000, NOW), "2 days ago");
}

/// (m6) Closing the circle, removing a bank and removing a member say what happens in My Banks words and mention changes not
/// sent; closing and removing a bank name her own bank with an unsent change, as leaving does.
#[test]
fn c94_m6_close_remove_bank_and_remove_member_use_my_banks_words() {
    let (mut m, s) = open_circle("owner", json!({}));
    let (mut bank, mut l) = linked(&[(QA, "One"), (QB, "Two")]);
    l.role_seen = Some("owner".into());
    m.edit.add_link(l).unwrap();
    bank.questions[0].stem.push_str(" (mine)");
    let mut banks = vec![bank];
    m.edit.sync(&mut banks);
    let draw = |m: &mut CirclesModel| {
        text_of(760.0, &mut |ui| {
            super::circles_ui::body_of(ui, m);
        })
    };
    let unsent = "\"Unit 2 quiz\" has 1 unsent change. Send it first if the circle should have it.";
    m.ask_close();
    let t = draw(&mut m);
    assert!(t.contains("Close this circle?") && t.contains("Banks people added to My Banks stay there") && t.contains(unsent) && !t.contains("Copies"), "{t}");
    m.cancel_panel();
    m.ask_remove("u-ed");
    let t = draw(&mut m);
    assert!(t.contains("Banks they added to My Banks stay there") && t.contains("changes they have not sent") && !t.contains("Copies"), "{t}");
    m.cancel_panel();
    // another bank of this circle with an unsent change is not named when this one is removed
    let (mut other, mut lo) = linked(&[(QC, "Three")]);
    other.id = "other-bank".into();
    other.name = "Plants".into();
    other.questions[0].stem.push_str(" (mine)");
    lo.local_bank_id = other.id.clone();
    lo.circle_bank_id = hex(98);
    lo.role_seen = Some("owner".into());
    m.edit.add_link(lo).unwrap();
    banks.push(other);
    m.edit.sync(&mut banks);
    m.ask_remove_on(BID);
    let t = draw(&mut m);
    assert!(t.contains("Anyone who added it to My Banks keeps it there") && t.contains(unsent) && !t.contains("Copies people made"), "{t}");
    assert!(!t.contains("\"Plants\" has"), "{t}");
    // the same on the bank's settings page
    m.cancel_remove_bank();
    s.keep_raw("Get", &format!("/circles/{CID}/banks/{BID}"), served(1, &[(QA, "One"), (QB, "Two")], false));
    m.open_bank(BID);
    for _ in 0..3 {
        m.frame(now(), &ready(&s));
    }
    m.ask_remove_bank();
    let t = draw(&mut m);
    assert!(t.contains("Anyone who added it to My Banks keeps it there") && t.contains(unsent) && !t.contains("\"Plants\" has"), "{t}");
}

/// (m5) The Banks list's row is wider than a 420 px window (its columns do not shrink): the badge under it shortens the circle's
/// name to what is on screen, and keeps its note whole and on screen.
#[test]
fn c94_m5_the_list_badge_shortens_the_circle_name_to_the_window() {
    let b = super::bank_links::Badge { circle: "Biology department, Pleasant Valley High School".into(), note: Some("newer version · 1 unsent".into()) };
    let ctx = egui::Context::default();
    crate::theme::setup(&ctx);
    let input = egui::RawInput { screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(420.0, 800.0))), time: Some(0.0), ..Default::default() };
    let mut found = Vec::new();
    for _ in 0..2 {
        let out = ctx.run_ui(input.clone(), |ui| {
            egui::CentralPanel::default().show(ui, |ui| {
                // (a row as the list draws it at 420 px: wider than the window)
                ui.horizontal(|ui| {
                    ui.allocate_space(egui::vec2(900.0, 10.0));
                });
                super::bank_links_ui::badge_line(ui, &b, false);
            });
        });
        found = out.shapes.iter().filter_map(|s| if let egui::epaint::Shape::Text(t) = &s.shape { Some((t.galley.text().to_string(), t.galley.elided, t.pos.x + t.galley.size().x)) } else { None }).collect();
        out.drop_without_applying_deltas();
    }
    let name = found.iter().find(|f| f.0.starts_with("Biology")).unwrap();
    let note = found.iter().find(|f| f.0.starts_with("· newer")).unwrap();
    assert!(name.1 && name.2 <= 420.0, "{found:?}");
    assert!(!note.1 && note.2 <= 420.0, "{found:?}");
}

// ---- critic 93's surviving mutants ---------------------------------------------------------------------------------------------------

/// (survivor m25) A copy whose changes can't be counted at all (no record of the version it matched) is named on the leave panel
/// with "may have changes that are not sent", never left out.
#[test]
fn s16_m25_the_leave_panel_names_a_copy_whose_changes_cant_be_counted() {
    let (mut m, _s) = open_circle("editor", json!({}));
    let (bank, mut l) = linked(&[(QA, "One"), (QB, "Two")]);
    l.role_seen = Some("editor".into());
    l.base.clear();
    m.edit.add_link(l).unwrap();
    let mut banks = vec![bank];
    m.edit.sync(&mut banks);
    let st = m.edit.status(LOCAL).unwrap().clone();
    assert!(!st.changes.known && !st.changes.partial() && st.learning.is_none(), "{st:?}");
    m.ask_leave();
    let t = text_of(760.0, &mut |ui| {
        super::circles_ui::body_of(ui, &mut m);
    });
    assert!(t.contains("\"Unit 2 quiz\" may have changes that are not sent (they can't be counted here). Send them first if the circle should have them."), "{t}");
}

/// (survivor m33) Checking the circle while it has a newer version: Get latest is offered beside the spinner (it settles the copy).
#[test]
fn s16_m33_checking_while_behind_offers_get_latest() {
    use super::bank_links::{Check, SignIn, StripButton, StripInput, strip_view};
    let (mut m, _s, mut banks, _ctx, _acct) = rig();
    phase_c(&mut m, &mut banks, 3, None, None);
    m.edit.update_link(LOCAL, |l| {
        l.head_seen = 4;
        true
    });
    let st = m.edit.status_of(&banks[0].clone()).unwrap();
    assert_eq!(st.learning, Some(Learning::Checking));
    let l = m.edit.link_for_local(LOCAL).unwrap();
    let v = strip_view(&StripInput { link: &l, status: Some(&st), check: Check::Done, sign_in: SignIn::Ready, now_ms: NOW, act: None, sent_unread: false, changed_since: false, unsure: false });
    assert_eq!(v.buttons, vec![StripButton::GetLatest, StripButton::History], "{v:?}");
    assert!(v.busy && v.line.contains("Checking the circle..."), "{v:?}");
}

/// (survivor m38) The bank page's strip tells the open question editor of a difficulty just taken from the circle, in the same
/// frame (the page's own wiring, not only `refresh_draft` by hand).
#[test]
fn s16_m38_the_strip_refreshes_the_open_editors_draft() {
    let (mut m, s, mut banks, _ctx, mut acct) = rig();
    s.keep("Post", "/circles/heads", 200, heads(3));
    phase_c(&mut m, &mut banks, 3, Some(Difficulty::Low), None);
    let mut draft = crate::qedit::Draft::from_question(LOCAL.into(), &banks[0].questions[0]);
    s.keep_raw("Get", &version_path(3), v3(Some("High"), None));
    let who = olga_named(&s);
    let mut b = browse();
    for _ in 0..3 {
        text_of(1280.0, &mut |ui| {
            super::bank_links_ui::strip_for_editor(ui, &mut m, &mut banks, &mut b, &mut acct, &who, now(), NOW, LOCAL, Some(&mut draft));
        });
    }
    assert_eq!(banks[0].questions[0].difficulty, Some(Difficulty::High));
    assert_eq!(draft.difficulty, Some(Difficulty::High));
    assert!(m.edit.take_inherited().is_empty(), "taken by the page");
}

/// (survivor m18) Get latest still fetching (its own answer not in) while the quiet read is on its way: it does not say it is
/// waiting for that read, whatever the time.
#[test]
fn s16_m18_a_merge_still_fetching_says_nothing_of_the_read() {
    let (mut m, s, mut banks, _ctx, _acct) = rig();
    s.keep("Post", "/circles/heads", 200, heads(4));
    phase_c(&mut m, &mut banks, 3, None, None);
    background(&mut m);
    s.keep_raw("Get", &version_path(3), v3(Some("High"), None));
    let who = olga_named(&s);
    let t0 = now();
    let read = s.hold();
    quiet(&mut m, &mut banks, &who, t0, 1);
    for _ in 0..200 {
        if version_reads(&s, 3) >= 1 {
            break;
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    let fetch = s.hold();
    s.on("Post", &format!("{}/merge", put_path()), 200, merge_start(3, 4));
    s.keep_raw("Get", &put_path(), served_qs(4, vec![rated(QA, "One", Some("High")), rated(QB, "Two", None)], false));
    assert!(m.edit.open_merge(LOCAL, owner()));
    for _ in 0..200 {
        if s.paths().iter().any(|p| p.contains("/merge")) {
            break;
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    m.edit.frame(t0 + Duration::from_secs(20), &who);
    assert!(matches!(&m.edit.view, EditView::Merge(v) if matches!(v.phase, MergePhase::Fetching)), "{:?}", m.edit.view);
    assert_eq!(m.edit.merge_wait_note(), None);
    fetch.send(()).unwrap();
    read.send(()).unwrap();
}

/// (survivor m48) A link that learns some of its base's difficulties and still lacks others (its `unknown` flag the same before
/// and after), in a bank put back from a saved copy (nothing is taken into it, so the bank's time stays): its count is worked out
/// again from what was learnt.
#[test]
fn s16_m48_what_a_read_teaches_is_counted_at_once() {
    let (mut m, s, mut banks, _ctx, _acct) = rig();
    phase_c(&mut m, &mut banks, 3, Some(Difficulty::Low), None);
    m.edit.update_link(LOCAL, |l| {
        l.difficulty_read = Some(2);
        true
    });
    make_stale(&mut m, LOCAL);
    // version 3 as served lacks Two: Two stays unknown
    s.keep_raw("Get", &version_path(3), served_qs(3, vec![rated(QA, "One", Some("High"))], false));
    let who = olga_named(&s);
    quiet(&mut m, &mut banks, &who, now(), 1);
    assert_eq!(m.edit.status(LOCAL).unwrap().changes.changed, 0);
    quiet(&mut m, &mut banks, &who, now(), 2);
    assert_eq!(m.edit.with_link(LOCAL, |l| (l.base[0].snap.difficulty, l.base[1].snap.difficulty)).unwrap(), (Some(Some(Difficulty::High)), None));
    assert_eq!(banks[0].questions[0].difficulty, Some(Difficulty::Low), "put back: nothing taken into it");
    assert_eq!(m.edit.status(LOCAL).unwrap().changes.changed, 1, "One's Low against the circle's High, counted at once");
}

/// (round 16) Try again on that strip tries at once, not a minute later.
#[test]
fn c93_i1_try_again_takes_the_ratings_at_once() {
    let (mut m, s, mut banks, ctx, mut acct) = rig();
    let k = rebuilt(&mut m);
    s.keep("Post", "/circles/heads", 200, heads(3));
    phase_c(&mut m, &mut banks, 3, Some(Difficulty::Low), None);
    k.restore_fails.store(true, Ordering::SeqCst);
    s.keep_raw("Get", &version_path(3), v3(Some("High"), None));
    let who = olga_named(&s);
    frames(&mut m, &mut banks, &ctx, &mut acct, &who, 3);
    k.restore_fails.store(false, Ordering::SeqCst);
    let mut b = browse();
    assert!(super::circles_phasec_tests::click_text(&ctx, 1280.0, "Try again", 0, &mut |ui| {
        super::bank_links_ui::strip_with(ui, &mut m, &mut banks, &mut b, &mut acct, &who, now(), NOW, LOCAL);
    }));
    frames(&mut m, &mut banks, &ctx, &mut acct, &who, 2);
    assert_eq!(banks[0].questions[0].difficulty, Some(Difficulty::High));
}

/// (round 16) The banks file could not be written when the ratings were to be taken; she then merges version 4 and rates One
/// herself. The merge settles the copy: her later rating is never replaced by what was waiting.
#[test]
fn c93_i1_a_merge_settles_ratings_still_waiting() {
    let (mut m, s, mut banks, ctx, mut acct) = rig();
    let k = rebuilt(&mut m);
    s.keep("Post", "/circles/heads", 200, heads(3));
    phase_c(&mut m, &mut banks, 3, None, None);
    k.save_fails.store(true, Ordering::SeqCst);
    s.keep_raw("Get", &version_path(3), v3(Some("High"), None));
    let who = olga_named(&s);
    frames(&mut m, &mut banks, &ctx, &mut acct, &who, 3);
    assert_eq!(m.edit.status(LOCAL).unwrap().learning, Some(Learning::Held));
    // (round 17, critic 95 minor 1) the review waits until the circle's difficulty is in the bank, then the merge settles it
    s.on("Post", &format!("{}/merge", put_path()), 200, merge_start(3, 4));
    s.keep_raw("Get", &put_path(), served_qs(4, vec![rated(QA, "One", Some("High")), rated(QB, "Two", None), rated(QC, "Three", None)], false));
    assert!(m.edit.open_merge(LOCAL, owner()));
    host_a(&mut m);
    frames(&mut m, &mut banks, &ctx, &mut acct, &who, 3);
    assert!(kinds(&m).is_none());
    k.save_fails.store(false, Ordering::SeqCst);
    for _ in 0..3 {
        page_frame(&mut m, &mut banks, &mut acct, &who, now() + Duration::from_secs(61));
    }
    assert!(kinds(&m).is_some());
    apply(&mut m, &s, &mut banks, &ctx, &mut acct);
    assert_eq!(m.edit.with_link(LOCAL, |l| l.inherit), Some(false));
    banks[0].questions[0].difficulty = Some(Difficulty::Low);
    banks[0].modified += 5;
    for _ in 0..3 {
        page_frame(&mut m, &mut banks, &mut acct, &who, now() + Duration::from_secs(61));
    }
    assert_eq!(banks[0].questions[0].difficulty, Some(Difficulty::Low), "her change after the merge is hers");
}

/// (round 16) A send's answer settles the copy too: nothing is left to take.
#[test]
fn c93_i1_a_send_settles_ratings_still_waiting() {
    let (bank, mut link) = linked(&[(QA, "One")]);
    link.inherit = true;
    let sent = super::circles_api::SentBank { version: 2, version_id: hex(2), qids: vec![QA.into()], ..Default::default() };
    let l = super::circle_links::after_send(&link, &bank, &sent);
    assert!(!l.inherit);
}
