//! Phase D1, round 7 (critics 75 and 76): each test written first against the code it pins. A send's answer that
//! landed on a Send page the person had left undrawn is said for its bank when that page is closed, whatever closes
//! it; a send whose answer does not say whether it went is never called "not sent"; a conflict answered with the page
//! closed keeps what a check learnt meanwhile; "sent, but the answer could not be read" survives a restart and never
//! hides later edits; a restore answered without a version is "when last checked" and checked at once; a send refused
//! because the person can only view now has the bank checked at once and says "send changes". And the behaviour critic
//! 75's surviving mutants changed, pinned.

use std::sync::{Arc, Mutex};

use serde_json::{Value, json};

use super::bank_links::Check;
use super::bank_links_fix4_tests::*;
use super::bank_links_fix_tests::*;
use super::bank_links_tests::*;
use super::circle_edit::*;
use super::circle_links::LinkLog;
use super::circles::CirclesModel;
use super::circles_tests::*;
use super::publish::{AccountSnapshot, Gate};
use crate::model::QuestionBank;

fn put_path() -> String {
    format!("/circles/{CID}/banks/{BID}")
}

fn rollback_path() -> String {
    format!("/circles/{CID}/banks/{BID}/rollback")
}

fn said(m: &CirclesModel, local: &str) -> String {
    m.edit.results_of(local).into_iter().map(|r| r.1).collect::<Vec<_>>().join(" | ")
}

fn wait_more(s: &Arc<Script>, n: usize) {
    for _ in 0..500 {
        if s.paths().len() > n {
            return;
        }
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
    panic!("no request after {n}: {:?}", s.paths());
}

/// Frames of the edit model alone (the Banks list, the Shared page) as `snap`, until nothing is on its way.
fn settle(m: &mut CirclesModel, snap: &AccountSnapshot) {
    for _ in 0..500 {
        m.edit.frame(now(), snap);
        if !m.edit.acting() {
            return;
        }
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
    panic!("still on its way: {:?}", m.edit.view);
}

fn strip_as(ctx: &egui::Context, m: &mut CirclesModel, banks: &mut [QuestionBank], acct: &mut FakeAccount, snap: &AccountSnapshot, local: &str) -> String {
    let mut b = browse();
    frame_text(ctx, &mut |ui| {
        super::bank_links_ui::strip_with(ui, m, banks, &mut b, acct, snap, now(), NOW, local);
    })
}

fn sent_body(version: u64, extra: Value) -> Value {
    let mut v = json!({"circle_id": CID, "bank_id": BID, "version": version, "version_id": hex(version), "qids": [QA], "added": 0, "changed": 1, "removed": 0});
    merge(&mut v, extra);
    v
}

/// A's Send page on A's page as `perm`, licence known, the own-work box ticked, one change; both banks' checks done.
fn send_ready_as(ctx: &egui::Context, m: &mut CirclesModel, banks: &mut Vec<QuestionBank>, acct: &mut FakeAccount, s: &Arc<Script>, perm: Perm) {
    until(ctx, m, banks, acct, s, LOCAL, &|m| m.links.check_of(LOCAL) != Check::Checking);
    until(ctx, m, banks, acct, s, LOCAL_B, &|m| m.links.check_of(LOCAL_B) != Check::Checking);
    banks[0].questions[0].stem = "One (my fix)".into();
    banks[0].modified += 5;
    assert!(m.edit.open_send(LOCAL, perm));
    m.edit.host = Some(EditHost { local_id: LOCAL.into(), back: "< Back to Unit 2 quiz".into() });
    m.edit.host_frame = None;
    until(ctx, m, banks, acct, s, LOCAL, &|m| matches!(&m.edit.view, EditView::Send(v) if matches!(v.licence, LicenceState::Known(_))));
    m.edit.set_attest(true);
    assert!(m.edit.send_blocker().is_none(), "blocked: {:?} {}", m.edit.send_blocker(), phase(&m.edit));
}

type Rig = (CirclesModel, Arc<Script>, Vec<QuestionBank>, egui::Context, FakeAccount);

/// (critic 75 I1) A's send held at the server; the person goes to B's page, so A's Send page stays open but is not
/// drawn; the answer `status`/`body` (or `fail`) lands on it, read on B's page.
fn answered_while_away(perm: Perm, status: u16, body: Value, fail: Option<super::api::ApiError>) -> Rig {
    let (mut m, s, mut banks) = two_at_three();
    background(&mut m);
    let ctx = ctx();
    let mut acct = FakeAccount { open: true, ..FakeAccount::default() };
    strip(&ctx, &mut m, &mut banks, &mut acct, &s, LOCAL);
    send_ready_as(&ctx, &mut m, &mut banks, &mut acct, &s, perm);
    match fail {
        Some(e) => {
            s.fail(e);
        }
        None => {
            s.on("Put", &put_path(), status, body);
        }
    }
    let n = s.paths().len();
    let release = s.hold();
    m.edit.send();
    m.edit.sync(&mut banks);
    wait_more(&s, n);
    strip(&ctx, &mut m, &mut banks, &mut acct, &s, LOCAL_B);
    release.send(()).unwrap();
    until(&ctx, &mut m, &mut banks, &mut acct, &s, LOCAL_B, &|m| !m.edit.acting());
    assert!(matches!(&m.edit.view, EditView::Send(v) if v.local_id == LOCAL && !matches!(v.stage, SendStage::Working(_) | SendStage::Editing)), "the answer is on A's Send page: {}", phase(&m.edit));
    (m, s, banks, ctx, acct)
}

/// (critic 73 I1) The same, with A's Send page closed before the answer came ("Circle page" on B's page).
fn left_before_answer(perm: Perm, status: u16, body: Value, fail: Option<super::api::ApiError>) -> Rig {
    let (mut m, s, mut banks) = two_at_three();
    background(&mut m);
    let ctx = ctx();
    let mut acct = FakeAccount { open: true, ..FakeAccount::default() };
    strip(&ctx, &mut m, &mut banks, &mut acct, &s, LOCAL);
    send_ready_as(&ctx, &mut m, &mut banks, &mut acct, &s, perm);
    match fail {
        Some(e) => {
            s.fail(e);
        }
        None => {
            s.on("Put", &put_path(), status, body);
        }
    }
    let n = s.paths().len();
    let release = s.hold();
    m.edit.send();
    m.edit.sync(&mut banks);
    wait_more(&s, n);
    strip(&ctx, &mut m, &mut banks, &mut acct, &s, LOCAL_B);
    m.open_circle_at(CID);
    assert!(!m.edit.active());
    release.send(()).unwrap();
    until(&ctx, &mut m, &mut banks, &mut acct, &s, LOCAL_B, &|m| !m.edit.acting());
    (m, s, banks, ctx, acct)
}

/// The ways the person closes a Send page they left undrawn (critic 75 I1).
#[derive(Clone, Copy, Debug)]
enum Away {
    CirclePage,
    OtherHistory,
    OtherSend,
    OtherGetLatest,
    SharedPageFirst,
    SignOut,
}

const AWAY: [Away; 6] = [Away::CirclePage, Away::OtherHistory, Away::OtherSend, Away::OtherGetLatest, Away::SharedPageFirst, Away::SignOut];

fn go(m: &mut CirclesModel, s: &Arc<Script>, how: Away) {
    let b = hex(77);
    match how {
        Away::CirclePage => m.open_circle_at(CID),
        Away::OtherHistory => {
            s.on("Get", &format!("/circles/{CID}/banks/{b}/versions"), 200, json!({"bank_id": b, "head_version": 1, "oldest_kept": 1, "pinned_versions": [], "versions": [], "next_before": null}));
            assert!(m.edit.open_history(CID, &b, "Plants", owner()));
        }
        Away::OtherSend => assert!(m.edit.open_send(LOCAL_B, owner())),
        Away::OtherGetLatest => {
            s.on("Post", &format!("/circles/{CID}/banks/{b}/merge"), 400, json!({"error": "invalid_request", "field": "base_version"}));
            assert!(m.edit.open_merge(LOCAL_B, owner()));
        }
        Away::SharedPageFirst => m.open_list(),
        Away::SignOut => m.edit.forget_account(false),
    }
}

// ---- c75 I1: an answer on a Send page left undrawn, then the page closed --------------------------------------------

#[test]
fn c75_i1_a_failed_send_left_undrawn_is_said_on_its_strip_whatever_closes_its_page() {
    for how in AWAY {
        let (mut m, s, _b, _c, _a) = answered_while_away(owner(), 503, json!({"error": "unavailable"}), None);
        go(&mut m, &s, how);
        let r = said(&m, LOCAL);
        assert!(r.contains("Your changes were not sent"), "{how:?}: {r}");
        assert!(m.edit.results_of(LOCAL_B).is_empty(), "{how:?}: nothing on B");
    }
}

#[test]
fn c75_i1_a_held_send_left_undrawn_is_said_on_its_strip_whatever_closes_its_page() {
    for how in AWAY {
        let (mut m, s, _b, _c, _a) = answered_while_away(owner(), 201, sent_body(4, json!({"held": true})), None);
        go(&mut m, &s, how);
        let r = said(&m, LOCAL);
        assert!(r.contains("Version 4 was sent") && r.contains("hidden from the rest of the circle"), "{how:?}: {r}");
    }
}

#[test]
fn c75_i1_the_daily_limit_left_undrawn_is_said_on_its_strip_whatever_closes_its_page() {
    for how in AWAY {
        let (mut m, s, _b, _c, _a) = answered_while_away(owner(), 429, json!({"error": "daily_limit", "metric": "circle_commits"}), None);
        go(&mut m, &s, how);
        let r = said(&m, LOCAL);
        assert!(r.contains("Your changes were not sent") && r.contains("today"), "{how:?}: {r}");
    }
}

#[test]
fn c75_i1_a_conflict_left_undrawn_is_said_on_its_strip_whatever_closes_its_page() {
    for how in AWAY {
        let (mut m, s, _b, _c, _a) = answered_while_away(owner(), 409, json!({"error": "conflict", "head_version": 5}), None);
        go(&mut m, &s, how);
        let r = said(&m, LOCAL);
        assert!(r.contains("someone sent version 5 first"), "{how:?}: {r}");
    }
}

#[test]
fn c75_i1_the_strip_shows_it_after_the_circle_page_closed_the_send_page() {
    let (mut m, s, mut banks, ctx, mut acct) = answered_while_away(owner(), 503, json!({"error": "unavailable"}), None);
    go(&mut m, &s, Away::CirclePage);
    let t = strip(&ctx, &mut m, &mut banks, &mut acct, &s, LOCAL);
    assert!(t.contains("Your changes were not sent") && t.contains("Dismiss"), "{t}");
}

#[test]
fn c75_i1_back_on_the_answer_the_page_shows_still_tells_the_strip() {
    // (as a History's result left open is said on the strip when the History is left: the person may not have read it)
    let (mut m, _s, _b, _c, _a) = answered_while_away(owner(), 503, json!({"error": "unavailable"}), None);
    m.edit.close();
    assert!(said(&m, LOCAL).contains("Your changes were not sent"), "{}", said(&m, LOCAL));
}

#[test]
fn c75_i1_an_answer_the_person_acted_on_is_not_said_again() {
    // "Change the note" on the failure: the person is back at the note, nothing more to tell
    let (mut m, _s, _b, _c, _a) = answered_while_away(owner(), 503, json!({"error": "unavailable"}), None);
    m.edit.back_to_note();
    m.edit.close();
    assert!(m.edit.results_of(LOCAL).is_empty(), "{}", said(&m, LOCAL));
}

#[test]
fn c75_i1_a_plain_send_left_undrawn_adds_nothing_when_its_page_closes() {
    for how in AWAY {
        let (mut m, s, _b, _c, _a) = answered_while_away(owner(), 201, sent_body(4, json!({})), None);
        go(&mut m, &s, how);
        assert!(m.edit.results_of(LOCAL).is_empty(), "{how:?}: {}", said(&m, LOCAL));
        assert_eq!(m.edit.with_link(LOCAL, |l| l.base_version), Some(4), "{how:?}");
    }
}

/// (critic 75 r20) Closing a normal "Sent" box (the page drawn, "Done") adds nothing: never "could not be read".
#[test]
fn c75_r20_done_on_a_normal_sent_box_adds_nothing_to_the_strip() {
    let (mut m, s, mut banks) = two_at_three();
    let ctx = ctx();
    let mut acct = FakeAccount { open: true, ..FakeAccount::default() };
    strip(&ctx, &mut m, &mut banks, &mut acct, &s, LOCAL);
    banks[0].questions[0].stem = "One (my fix)".into();
    banks[0].modified += 5;
    assert!(m.edit.open_send(LOCAL, owner()));
    m.edit.host = Some(EditHost { local_id: LOCAL.into(), back: "< Back to Unit 2 quiz".into() });
    for _ in 0..3 {
        strip(&ctx, &mut m, &mut banks, &mut acct, &s, LOCAL);
    }
    m.edit.set_attest(true);
    s.on("Put", &put_path(), 201, sent_body(4, json!({})));
    m.edit.send();
    m.edit.sync(&mut banks);
    let page = strip(&ctx, &mut m, &mut banks, &mut acct, &s, LOCAL);
    assert!(matches!(&m.edit.view, EditView::Send(v) if matches!(v.stage, SendStage::Done(_))), "{}", phase(&m.edit));
    assert!(page.contains("Version 4"), "{page}");
    m.edit.close();
    assert!(m.edit.results_of(LOCAL).is_empty(), "{}", said(&m, LOCAL));
    let t = strip(&ctx, &mut m, &mut banks, &mut acct, &s, LOCAL);
    assert!(!t.contains("could not be read") && t.contains("up to date"), "{t}");
}

// ---- c75 minor 3: an answer that does not say whether the send went ------------------------------------------------

#[test]
fn c75_m3_a_timeout_with_the_page_closed_says_it_may_have_gone_through_and_checks_the_bank() {
    let (mut m, s, mut banks, ctx, mut acct) = left_before_answer(owner(), 0, Value::Null, Some(super::api::ApiError::Offline("Can't reach pool.test: the connection timed out.".into())));
    let r = said(&m, LOCAL);
    assert!(!r.contains("not sent") && !r.contains("Nothing changed"), "{r}");
    assert!(r.contains("may have gone through") && r.contains("still on this computer"), "{r}");
    let before = heads_count(&s);
    let t = strip(&ctx, &mut m, &mut banks, &mut acct, &s, LOCAL);
    assert_eq!(heads_count(&s), before + 1, "A's newest version is checked at once");
    assert!(!t.contains("up to date") && !t.contains("is the latest"), "{t}");
    assert!(t.contains("Version 3 when last checked"), "until the check answers: {t}");
}

#[test]
fn c75_m3_an_answer_that_can_not_be_read_with_the_page_closed_is_not_called_not_sent() {
    let (m, _s, _b, _c, _a) = left_before_answer(owner(), 201, json!({"circle_id": CID, "bank_id": BID, "version": "four"}), None);
    let r = said(&m, LOCAL);
    assert!(!r.contains("not sent"), "{r}");
    assert!(r.contains("may have gone through"), "{r}");
}

#[test]
fn c75_m3_a_server_failure_with_the_page_closed_is_not_called_not_sent() {
    let (m, _s, _b, _c, _a) = left_before_answer(owner(), 500, json!({"error": "server_error"}), None);
    let r = said(&m, LOCAL);
    assert!(!r.contains("not sent") && !r.contains("Nothing was changed"), "{r}");
    assert!(r.contains("may have gone through"), "{r}");
}

#[test]
fn c75_m3_the_send_page_itself_never_says_nothing_changed_after_a_timeout() {
    let (mut m, s, mut banks) = two_at_three();
    let ctx = ctx();
    let mut acct = FakeAccount { open: true, ..FakeAccount::default() };
    strip(&ctx, &mut m, &mut banks, &mut acct, &s, LOCAL);
    banks[0].questions[0].stem = "One (my fix)".into();
    banks[0].modified += 5;
    assert!(m.edit.open_send(LOCAL, owner()));
    m.edit.host = Some(EditHost { local_id: LOCAL.into(), back: "< Back to Unit 2 quiz".into() });
    for _ in 0..3 {
        strip(&ctx, &mut m, &mut banks, &mut acct, &s, LOCAL);
    }
    m.edit.set_attest(true);
    s.fail(super::api::ApiError::Offline("Can't reach pool.test: the connection timed out.".into()));
    m.edit.send();
    m.edit.sync(&mut banks);
    let page = strip(&ctx, &mut m, &mut banks, &mut acct, &s, LOCAL);
    assert!(page.contains("may have gone through") && !page.contains("Nothing changed"), "{page}");
    assert!(page.contains("Try again"), "{page}");
}

#[test]
fn c75_m3_a_refusal_the_server_gave_before_doing_anything_is_still_not_sent() {
    let (m, _s, _b, _c, _a) = left_before_answer(owner(), 503, json!({"error": "unavailable"}), None);
    assert!(said(&m, LOCAL).contains("Your changes were not sent"), "{}", said(&m, LOCAL));
}

// ---- c75 minor 4: a conflict answered with the page closed keeps what a check learnt meanwhile ---------------------

#[test]
fn c75_m4_a_conflict_answered_with_the_page_closed_keeps_what_a_check_learnt_meanwhile() {
    let (mut m, s, mut banks) = two_at_three();
    background(&mut m);
    let ctx = ctx();
    let mut acct = FakeAccount { open: true, ..FakeAccount::default() };
    strip(&ctx, &mut m, &mut banks, &mut acct, &s, LOCAL);
    send_ready_as(&ctx, &mut m, &mut banks, &mut acct, &s, owner());
    s.on("Put", &put_path(), 409, json!({"error": "conflict", "head_version": 5}));
    let n = s.paths().len();
    let release = s.hold();
    m.edit.send();
    m.edit.sync(&mut banks);
    wait_more(&s, n);
    m.open_circle_at(CID);
    // a check answered meanwhile: the person is a viewer now, and the newest version is held for review
    m.edit.update_link(LOCAL, |l| {
        l.role_seen = Some("viewer".into());
        l.seen.under_review = true;
        true
    });
    release.send(()).unwrap();
    until(&ctx, &mut m, &mut banks, &mut acct, &s, LOCAL_B, &|m| !m.edit.acting());
    assert_eq!(m.edit.with_link(LOCAL, |l| (l.head_seen, l.role_seen.clone(), l.seen.under_review)), Some((5, Some("viewer".to_string()), true)));
}

// ---- c75 minor 5: "sent, but not read" survives a restart -------------------------------------------------------------

/// A's send (inline jobs, A's page) answered with no version the app can read; `log` is the links file. (critic 77
/// minor 5) olga is signed in throughout, so what is kept names her and the account rule is exercised.
fn unread_on(log: Arc<Mutex<Vec<super::circle_links::CircleLink>>>) -> Rig {
    let (m0, s, mut banks) = two_at_three();
    s.keep("Get", &format!("/circles/{CID}"), 200, detail("owner", json!({})));
    let mut m = model();
    m.edit = super::circle_edit::CircleEditModel::new(super::model::Runner::Inline, Arc::new(Mem::default()), Arc::new(std::collections::HashMap::<String, Vec<u8>>::new()), Arc::new(NoDiskRestore), LinkLog::Memory(log)).with_retry(super::pool::RetryPolicy::none());
    m.edit.set_clock(NOW);
    let mut m0 = m0;
    for l in m0.edit.links().to_vec() {
        m.edit.add_link(l).unwrap();
    }
    let ctx = ctx();
    let mut acct = FakeAccount { open: true, ..FakeAccount::default() };
    strip_as(&ctx, &mut m, &mut banks, &mut acct, &as_who(&s, "olga"), LOCAL);
    banks[0].questions[0].stem = "One (my fix)".into();
    banks[0].modified += 5;
    assert!(m.edit.open_send(LOCAL, editor()));
    m.edit.host = Some(EditHost { local_id: LOCAL.into(), back: "< Back to Unit 2 quiz".into() });
    for _ in 0..3 {
        strip_as(&ctx, &mut m, &mut banks, &mut acct, &as_who(&s, "olga"), LOCAL);
    }
    m.edit.set_attest(true);
    s.on("Put", &put_path(), 201, json!({"circle_id": CID, "bank_id": BID, "version": 0, "qids": [], "added": 0, "changed": 0, "removed": 0}));
    m.edit.send();
    m.edit.sync(&mut banks);
    strip_as(&ctx, &mut m, &mut banks, &mut acct, &as_who(&s, "olga"), LOCAL);
    m.edit.close();
    assert!(m.edit.sent_unread(LOCAL));
    (m, s, banks, ctx, acct)
}

fn restarted(log: Arc<Mutex<Vec<super::circle_links::CircleLink>>>) -> CirclesModel {
    let mut m = model();
    m.edit = super::circle_edit::CircleEditModel::new(super::model::Runner::Inline, Arc::new(Mem::default()), Arc::new(std::collections::HashMap::<String, Vec<u8>>::new()), Arc::new(NoDiskRestore), LinkLog::Memory(log)).with_retry(super::pool::RetryPolicy::none());
    m.edit.set_clock(NOW);
    m
}

#[test]
fn c75_m5_sent_but_not_read_survives_a_restart_and_send_is_not_offered_into_a_conflict() {
    let log = Arc::new(Mutex::new(Vec::new()));
    let (_m, s, mut banks, ctx, mut acct) = unread_on(log.clone());
    let mut m = restarted(log);
    let t = strip_as(&ctx, &mut m, &mut banks, &mut acct, &as_who(&s, "olga"), LOCAL);
    assert!(m.edit.sent_unread(LOCAL), "kept in the links file");
    assert!(!t.contains("Send my changes") && !t.contains("unsent"), "{t}");
    assert!(t.contains("Get latest"), "{t}");
}

#[test]
fn c75_m5_getting_the_latest_ends_it_in_the_file_too() {
    let log = Arc::new(Mutex::new(Vec::new()));
    let (mut m, s, mut banks, ctx, mut acct) = unread_on(log.clone());
    s.on("Post", &format!("/circles/{CID}/banks/{BID}/merge"), 400, json!({"error": "invalid_request", "field": "base_version"}));
    assert!(m.edit.open_merge(LOCAL, editor()));
    m.edit.host = Some(EditHost { local_id: LOCAL.into(), back: "< Back to Unit 2 quiz".into() });
    for _ in 0..3 {
        strip_as(&ctx, &mut m, &mut banks, &mut acct, &as_who(&s, "olga"), LOCAL);
    }
    m.edit.cancel_merge();
    assert!(!m.edit.sent_unread(LOCAL));
    let mut m2 = restarted(log);
    strip_as(&ctx, &mut m2, &mut banks, &mut acct, &as_who(&s, "olga"), LOCAL);
    assert!(!m2.edit.sent_unread(LOCAL));
}

#[test]
fn c75_m5_the_link_file_without_the_new_field_reads_as_before_and_writes_nothing_new() {
    let (_, l) = linked(&[(QA, "One")]);
    let text = serde_json::to_string(&l).unwrap();
    assert!(!text.contains("sent_unread"), "{text}");
    let back: super::circle_links::CircleLink = serde_json::from_str(&text).unwrap();
    assert_eq!(back, l);
}

// ---- c75 minor 6: a History left open on the Shared page, the Banks list -------------------------------------------

#[test]
fn c75_m6_a_restore_answered_while_its_history_was_left_open_on_the_shared_page_is_said_on_the_banks_list() {
    let (mut m, s, mut banks) = two_at_three();
    background(&mut m);
    let ctx = ctx();
    let mut acct = FakeAccount { open: true, ..FakeAccount::default() };
    let z = hex(99);
    strip(&ctx, &mut m, &mut banks, &mut acct, &s, LOCAL);
    until(&ctx, &mut m, &mut banks, &mut acct, &s, LOCAL, &|m| m.links.check_of(LOCAL) != Check::Checking);
    s.on("Get", &format!("/circles/{CID}/banks/{z}/versions"), 200, json!({"bank_id": z, "head_version": 3, "oldest_kept": 1, "pinned_versions": [], "versions": [{"version": 3, "version_id": hex(3), "by": "Mr Keesler", "at": NOW, "message": "", "added": 0, "changed": 1, "removed": 0, "question_count": 1, "license": "CC-BY-4.0", "kept": true, "withheld": false}, {"version": 1, "version_id": hex(1), "by": "Mr Keesler", "at": NOW - DAY, "message": "", "added": 1, "changed": 0, "removed": 0, "question_count": 1, "license": "CC-BY-4.0", "kept": true, "withheld": false}], "next_before": null}));
    assert!(m.edit.open_history(CID, &z, "Unit 3 test", owner()));
    for _ in 0..200 {
        m.edit.frame(now(), &ready(&s));
        if loaded(&m) {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
    // drawn on the Shared page (frame 40), then left open there: the person went to My Banks
    m.edit.set_pass(40);
    m.edit.mark_drawn(40);
    m.edit.ask_restore(1);
    s.on("Post", &format!("/circles/{CID}/banks/{z}/rollback"), 200, json!({"circle_id": CID, "bank_id": z, "version": 4, "version_id": hex(4), "qids": [QA], "added": 0, "changed": 1, "removed": 0, "rollback_of": 1}));
    m.edit.confirm_restore(false);
    let mut seen = false;
    for frame in 42..342 {
        // the app's frames on My Banks: the Shared page is not drawn in them
        m.tick_background_in(now(), &ready(&s), frame);
        if rows(&mut m).contains("\"Unit 3 test\": Version 4 was restored from version 1.") {
            seen = true;
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
    assert!(seen, "{}", rows(&mut m));
}

// ---- c76 I1: later edits after a send whose answer could not be read -------------------------------------------------

#[test]
fn c76_i1_edits_made_after_an_unread_send_are_never_said_to_be_sent_and_send_is_offered() {
    let log = Arc::new(Mutex::new(Vec::new()));
    let (mut m, s, mut banks, ctx, mut acct) = unread_on(log);
    let t = strip_as(&ctx, &mut m, &mut banks, &mut acct, &as_who(&s, "olga"), LOCAL);
    assert!(t.contains("Your changes were sent") && !t.contains("Send my changes"), "nothing changed since: as before: {t}");
    // one more edit
    banks[0].questions[0].stem = "One (my second fix)".into();
    banks[0].modified += 5;
    let t = strip_as(&ctx, &mut m, &mut banks, &mut acct, &as_who(&s, "olga"), LOCAL);
    assert!(!t.contains("Your changes were sent."), "{t}");
    assert!(t.contains("Changes you made since are not sent yet") && t.contains("Get the latest first, then send"), "{t}");
    assert!(t.contains("Get latest") && t.contains("Send my changes"), "{t}");
    let badge = {
        let bank = banks[0].clone();
        let CirclesModel { links, edit, .. } = &mut m;
        links.badge(edit, &bank).and_then(|b| b.note).unwrap_or_default()
    };
    assert_eq!(badge, "newer version · unsent changes");
}

// ---- c76 I2: a restore answered without a version -------------------------------------------------------------------

fn restore_v0(leave_first: bool) -> Rig {
    let (mut m, s, mut banks) = two_at_three();
    let ctx = ctx();
    let mut acct = FakeAccount { open: true, ..FakeAccount::default() };
    strip(&ctx, &mut m, &mut banks, &mut acct, &s, LOCAL);
    s.on("Get", &versions(), 200, page(3, &[3, 2, 1]));
    history_on_a(&mut m);
    strip(&ctx, &mut m, &mut banks, &mut acct, &s, LOCAL);
    strip(&ctx, &mut m, &mut banks, &mut acct, &s, LOCAL);
    assert!(loaded(&m));
    m.edit.ask_restore(1);
    s.on("Post", &rollback_path(), 200, json!({"circle_id": CID, "bank_id": BID, "version": 0, "qids": [], "added": 0, "changed": 0, "removed": 0, "rollback_of": 1}));
    s.on("Get", &versions(), 200, page(4, &[4, 3, 2, 1]));
    m.edit.confirm_restore(false);
    if leave_first {
        m.edit.back();
    }
    (m, s, banks, ctx, acct)
}

#[test]
fn c76_i2_a_restore_answered_without_a_version_never_says_version_0_on_the_history() {
    let (mut m, s, mut banks, ctx, mut acct) = restore_v0(false);
    let page = strip(&ctx, &mut m, &mut banks, &mut acct, &s, LOCAL);
    assert!(!page.contains("Version 0") && !page.contains("version 0"), "{page}");
    assert!(page.contains("version 1") || page.contains("Version 1"), "{page}");
}

#[test]
fn c76_i2_a_restore_answered_without_a_version_is_checked_at_once_and_never_up_to_date() {
    let (mut m, s, mut banks, ctx, mut acct) = restore_v0(true);
    let before = heads_count(&s);
    let t = strip(&ctx, &mut m, &mut banks, &mut acct, &s, LOCAL);
    assert!(!t.contains("Version 0") && !t.contains("up to date"), "{t}");
    assert!(t.contains("Version 3 when last checked") || heads_count(&s) == before + 1, "{t}");
    assert_eq!(heads_count(&s), before + 1, "A's newest version is checked at once");
    assert!(said(&m, LOCAL).contains("Version 1 was restored") && said(&m, LOCAL).contains("could not be read"), "{}", said(&m, LOCAL));
}

#[test]
fn c76_i2_until_the_check_answers_the_strip_says_when_last_checked() {
    let (mut m, s, _banks, _ctx, _acct) = restore_v0(true);
    // the answer read on the Shared page or the Banks list: no bank page has checked yet
    m.edit.frame(now(), &ready(&s));
    assert!(m.edit.unsure(LOCAL), "marked unsure");
    assert_eq!(m.edit.take_check_asks(), vec![LOCAL.to_string()], "the next frame of its page checks it");
}

// ---- c76 I3: a send refused because the person can only view now --------------------------------------------------------

#[test]
fn c76_i3_a_send_refused_for_a_viewer_says_send_changes_and_checks_the_bank_at_once() {
    let (mut m, s, mut banks, ctx, mut acct) = left_before_answer(owner(), 403, json!({"error": "circle_role_required", "needed": "editor"}), None);
    let r = said(&m, LOCAL);
    assert!(r.contains("send changes") && !r.contains("add banks"), "{r}");
    s.keep("Get", "/circles", 200, circles_list("viewer"));
    let before = heads_count(&s);
    strip(&ctx, &mut m, &mut banks, &mut acct, &s, LOCAL);
    assert_eq!(heads_count(&s), before + 1, "A is checked at once");
    let t = until(&ctx, &mut m, &mut banks, &mut acct, &s, LOCAL, &|m| m.links.check_of(LOCAL) == Check::Done);
    let t = t + &strip(&ctx, &mut m, &mut banks, &mut acct, &s, LOCAL);
    assert!(t.contains("viewers can't send") && !t.contains("Send my changes"), "{t}");
}

// ---- critic 75's surviving mutants, pinned ----------------------------------------------------------------------------

/// (r10) A conflict naming a newer version, with the page closed, takes away "no answer came for the restore ... it
/// may still go through" (a newer version is known now). ("Used as it is" goes when the send starts.)
#[test]
fn c75_r10_a_conflict_with_the_page_closed_takes_away_no_answer_came() {
    let (mut m, s, mut banks) = two_at_three();
    background(&mut m);
    let ctx = ctx();
    let mut acct = FakeAccount { open: true, ..FakeAccount::default() };
    strip(&ctx, &mut m, &mut banks, &mut acct, &s, LOCAL);
    s.on("Get", &versions(), 200, page(3, &[3, 2, 1]));
    history_on_a(&mut m);
    until(&ctx, &mut m, &mut banks, &mut acct, &s, LOCAL, &loaded);
    until(&ctx, &mut m, &mut banks, &mut acct, &s, LOCAL, &|m| m.links.check_of(LOCAL) != Check::Checking);
    m.edit.ask_restore(1);
    s.on("Post", &rollback_path(), 200, rolled(4, 1));
    let n = s.paths().len();
    let release = s.hold();
    m.edit.confirm_restore(false);
    wait_more(&s, n);
    m.edit.back();
    m.edit.frame(now() + ACT_PATIENCE, &ready(&s));
    let _ = release.send(());
    assert!(said(&m, LOCAL).contains("No answer came"), "{}", said(&m, LOCAL));
    send_ready_as(&ctx, &mut m, &mut banks, &mut acct, &s, owner());
    s.on("Put", &put_path(), 409, json!({"error": "conflict", "head_version": 5}));
    let n = s.paths().len();
    let release = s.hold();
    m.edit.send();
    m.edit.sync(&mut banks);
    wait_more(&s, n);
    m.open_circle_at(CID);
    release.send(()).unwrap();
    until(&ctx, &mut m, &mut banks, &mut acct, &s, LOCAL_B, &|m| !m.edit.acting());
    let r = said(&m, LOCAL);
    assert!(r.contains("someone sent version 5 first") && !r.contains("No answer came"), "{r}");
}

/// (r12) A send the circle needs changes for, answered with the page closed, says so.
#[test]
fn c75_r12_a_send_that_needs_changes_answered_with_the_page_closed_says_so() {
    let (m, _s, _b, _c, _a) = left_before_answer(owner(), 422, json!({"error": "invalid_payload"}), None);
    let r = said(&m, LOCAL);
    assert!(r.contains("Your changes were not sent: the circle needs a few choices or changes first"), "{r}");
}

/// (r13) A refusal with its own words (the licence is the author's), answered with the page closed, says them.
#[test]
fn c75_r13_a_refusal_answered_with_the_page_closed_says_why() {
    let (m, _s, _b, _c, _a) = left_before_answer(editor(), 403, json!({"error": "license_author_only", "license": "CC-BY-4.0"}), None);
    let r = said(&m, LOCAL);
    assert!(r.contains("Your changes were not sent: Only the bank's author chooses its licence"), "{r}");
}

/// (r14) The session ended before a send whose page was closed was answered: once signed in again, that goes.
#[test]
fn c75_r14_the_session_line_of_a_left_send_goes_once_signed_in_again() {
    let (mut m, s, _b, _c, _a) = left_before_answer(owner(), 0, Value::Null, Some(super::api::ApiError::SignedOut));
    assert!(said(&m, LOCAL).contains("Your session ended before the send was answered"), "{}", said(&m, LOCAL));
    m.edit.frame(now(), &AccountSnapshot::of_gate(Gate::SignedOut));
    m.edit.frame(now(), &ready(&s));
    assert!(!said(&m, LOCAL).contains("Your session ended"), "{}", said(&m, LOCAL));
}

/// olga's send from A (owner), held; pete signs in on this computer before the answer (`reply`, or the job dies).
fn olgas_send_then_pete(reply: Option<(u16, Value)>) -> Rig {
    let (mut m, s, mut banks) = two_at_three();
    background(&mut m);
    let ctx = ctx();
    let mut acct = FakeAccount { open: true, ..FakeAccount::default() };
    let (olga, pete) = (as_who(&s, "olga"), as_who(&s, "pete"));
    s.keep("Get", &format!("/circles/{CID}"), 200, detail("owner", json!({})));
    strip_as(&ctx, &mut m, &mut banks, &mut acct, &olga, LOCAL);
    until(&ctx, &mut m, &mut banks, &mut acct, &s, LOCAL, &|m| m.links.check_of(LOCAL) != Check::Checking);
    banks[0].questions[0].stem = "One (my fix)".into();
    banks[0].modified += 5;
    strip_as(&ctx, &mut m, &mut banks, &mut acct, &olga, LOCAL);
    assert!(m.edit.open_send(LOCAL, owner()));
    m.edit.host = Some(EditHost { local_id: LOCAL.into(), back: "< Back to Unit 2 quiz".into() });
    for _ in 0..500 {
        strip_as(&ctx, &mut m, &mut banks, &mut acct, &olga, LOCAL);
        if matches!(&m.edit.view, EditView::Send(v) if matches!(v.licence, LicenceState::Known(_))) {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
    m.edit.set_attest(true);
    if let Some((st, body)) = reply {
        s.on("Put", &put_path(), st, body);
    }
    let n = s.paths().len();
    let release = s.hold();
    m.edit.send();
    m.edit.sync(&mut banks);
    wait_more(&s, n);
    strip_as(&ctx, &mut m, &mut banks, &mut acct, &pete, LOCAL);
    release.send(()).unwrap();
    settle(&mut m, &pete);
    (m, s, banks, ctx, acct)
}

/// (r15) The held note of the first account's send is not told to another account.
#[test]
fn c75_r15_another_account_is_not_told_the_first_ones_send_was_held() {
    let (mut m, s, mut banks, ctx, mut acct) = olgas_send_then_pete(Some((201, sent_body(4, json!({"held": true})))));
    let pete = as_who(&s, "pete");
    let t = strip_as(&ctx, &mut m, &mut banks, &mut acct, &pete, LOCAL);
    assert!(!t.contains("Version 4 was sent") && !t.contains("hidden from the rest"), "{t}");
    assert!(m.edit.results_of(LOCAL).is_empty(), "{}", said(&m, LOCAL));
}

/// (r19) The first account's send whose job died is not told to another account.
#[test]
fn c75_r19_another_account_is_not_told_the_first_ones_send_job_died() {
    let (m, _s, _b, _c, _a) = olgas_send_then_pete(None);
    assert!(m.edit.results_of(LOCAL).is_empty(), "{}", said(&m, LOCAL));
}

/// (r16) An editor who is not the bank's author is told only the author or the owner can lift a hold.
#[test]
fn c75_r16_a_held_send_from_an_editor_says_who_can_lift_it() {
    let (m, _s, _b, _c, _a) = left_before_answer(editor(), 201, sent_body(4, json!({"held": true})), None);
    let r = said(&m, LOCAL);
    assert!(r.contains("The bank's author or the circle's owner can lift this"), "{r}");
    assert!(!r.contains("a clean version sent later lifts this"), "{r}");
}

/// (r18) A send job that died while another bank's Send page stood in its place is said for its own bank.
#[test]
fn c75_r18_a_dead_send_job_is_said_for_its_bank_whatever_send_page_is_open() {
    let (mut m, s, mut banks) = two_at_three();
    background(&mut m);
    let ctx = ctx();
    let mut acct = FakeAccount { open: true, ..FakeAccount::default() };
    strip(&ctx, &mut m, &mut banks, &mut acct, &s, LOCAL);
    send_ready_as(&ctx, &mut m, &mut banks, &mut acct, &s, owner());
    // (no answer scripted: the job's thread ends without one)
    let n = s.paths().len();
    let release = s.hold();
    m.edit.send();
    m.edit.sync(&mut banks);
    wait_more(&s, n);
    // B's Send page in its place (a state the screens refuse to reach while a send is on its way)
    let EditView::Send(mut v) = std::mem::replace(&mut m.edit.view, EditView::Closed) else { panic!("{}", phase(&m.edit)) };
    v.local_id = LOCAL_B.into();
    v.bid = hex(77);
    v.stage = SendStage::Editing;
    m.edit.view = EditView::Send(v);
    release.send(()).unwrap();
    settle(&mut m, &ready(&s));
    assert!(said(&m, LOCAL).contains("Your changes were not sent"), "{}", said(&m, LOCAL));
    assert!(matches!(&m.edit.view, EditView::Send(v) if matches!(v.stage, SendStage::Editing)), "B's page is untouched: {}", phase(&m.edit));
}

/// (r21) A signed-out answer while the account is still signed out asks to sign in (it is not an earlier session's).
#[test]
fn c75_r21_a_signed_out_answer_while_still_signed_out_asks_to_sign_in() {
    let (mut m, s, mut banks) = two_at_three();
    background(&mut m);
    let ctx = ctx();
    let mut acct = FakeAccount { open: true, ..FakeAccount::default() };
    strip(&ctx, &mut m, &mut banks, &mut acct, &s, LOCAL);
    send_ready_as(&ctx, &mut m, &mut banks, &mut acct, &s, owner());
    s.fail(super::api::ApiError::SignedOut);
    let n = s.paths().len();
    let release = s.hold();
    m.edit.send();
    m.edit.sync(&mut banks);
    wait_more(&s, n);
    let out = AccountSnapshot::of_gate(Gate::SignedOut);
    m.edit.forget_account(false);
    m.edit.frame(now(), &out);
    release.send(()).unwrap();
    settle(&mut m, &out);
    let r = said(&m, LOCAL);
    assert!(r.contains("Sign in, then send your changes again"), "{r}");
    assert!(!r.contains("Press Send my changes to send them again"), "{r}");
}

/// (r24) A later send whose answer is read ends "sent, but not read".
#[test]
fn c75_r24_a_later_send_whose_answer_is_read_ends_the_sent_state() {
    let log = Arc::new(Mutex::new(Vec::new()));
    let (mut m, s, mut banks, ctx, mut acct) = unread_on(log);
    assert!(m.edit.open_send(LOCAL, editor()));
    m.edit.host = Some(EditHost { local_id: LOCAL.into(), back: "< Back to Unit 2 quiz".into() });
    for _ in 0..3 {
        strip(&ctx, &mut m, &mut banks, &mut acct, &s, LOCAL);
    }
    m.edit.set_attest(true);
    s.on("Put", &put_path(), 201, sent_body(4, json!({})));
    m.edit.send();
    m.edit.sync(&mut banks);
    strip(&ctx, &mut m, &mut banks, &mut acct, &s, LOCAL);
    assert!(matches!(&m.edit.view, EditView::Send(v) if matches!(v.stage, SendStage::Done(_))), "{}", phase(&m.edit));
    assert!(!m.edit.sent_unread(LOCAL));
    m.edit.close();
    let t = strip(&ctx, &mut m, &mut banks, &mut acct, &s, LOCAL);
    assert!(!t.contains("Your changes were sent"), "{t}");
}

/// olga's restore of A from its History on A's page, held at the server; then pete signs in on this computer.
fn olgas_restore_held_then_pete() -> (Rig, std::sync::mpsc::Sender<()>) {
    let (mut m, s, mut banks) = two_at_three();
    background(&mut m);
    let ctx = ctx();
    let mut acct = FakeAccount { open: true, ..FakeAccount::default() };
    let (olga, pete) = (as_who(&s, "olga"), as_who(&s, "pete"));
    s.keep("Get", &format!("/circles/{CID}"), 200, detail("owner", json!({})));
    strip_as(&ctx, &mut m, &mut banks, &mut acct, &olga, LOCAL);
    s.on("Get", &versions(), 200, page(3, &[3, 2, 1]));
    history_on_a(&mut m);
    for _ in 0..500 {
        strip_as(&ctx, &mut m, &mut banks, &mut acct, &olga, LOCAL);
        if loaded(&m) && m.links.check_of(LOCAL) != Check::Checking {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
    m.edit.ask_restore(1);
    let n = s.paths().len();
    let release = s.hold();
    m.edit.confirm_restore(false);
    wait_more(&s, n);
    m.edit.back();
    strip_as(&ctx, &mut m, &mut banks, &mut acct, &pete, LOCAL);
    ((m, s, banks, ctx, acct), release)
}

/// (r26) Another account's restore with no answer still leaves this account's newest version "when last checked".
#[test]
fn c75_r26_another_accounts_restore_with_no_answer_makes_the_newest_version_unsure_here_too() {
    let ((mut m, s, _banks, _ctx, _acct), release) = olgas_restore_held_then_pete();
    let pete = as_who(&s, "pete");
    m.edit.frame(now() + ACT_PATIENCE, &pete);
    assert!(!m.edit.acting());
    // (until the next check or History read: the circle's newest version may still change)
    assert!(m.edit.unsure(LOCAL));
    let _ = release.send(());
}

/// (r30) A check read for another account (the circle does not list it) settles nothing for this bank.
#[test]
fn c75_r30_a_check_read_for_another_account_does_not_end_unsure() {
    let (mut m, s, mut banks) = two_at_three();
    background(&mut m);
    let ctx = ctx();
    let mut acct = FakeAccount { open: true, ..FakeAccount::default() };
    strip(&ctx, &mut m, &mut banks, &mut acct, &s, LOCAL);
    s.on("Get", &versions(), 200, page(3, &[3, 2, 1]));
    history_on_a(&mut m);
    until(&ctx, &mut m, &mut banks, &mut acct, &s, LOCAL, &loaded);
    until(&ctx, &mut m, &mut banks, &mut acct, &s, LOCAL, &|m| m.links.check_of(LOCAL) != Check::Checking);
    m.edit.ask_restore(1);
    s.on("Post", &rollback_path(), 200, rolled(4, 1));
    let n = s.paths().len();
    let release = s.hold();
    m.edit.confirm_restore(false);
    wait_more(&s, n);
    m.edit.back();
    m.edit.frame(now() + ACT_PATIENCE, &ready(&s));
    let _ = release.send(());
    assert!(m.edit.unsure(LOCAL));
    // pete, whom the circle does not list, checks A
    s.keep("Get", "/circles", 200, json!({"circles": []}));
    let pete = as_who(&s, "pete");
    m.links.check_now(LOCAL);
    for _ in 0..500 {
        strip_as(&ctx, &mut m, &mut banks, &mut acct, &pete, LOCAL);
        if m.links.check_of(LOCAL) == Check::OtherAccount {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
    assert_eq!(m.links.check_of(LOCAL), Check::OtherAccount);
    assert!(m.edit.unsure(LOCAL), "another account's read says nothing of this one's newest version");
}

/// (r32) A copy that can no longer send is never told "Your changes were sent ... Get latest".
#[test]
fn c75_r32_a_copy_that_can_no_longer_send_is_not_told_its_changes_were_sent() {
    let log = Arc::new(Mutex::new(Vec::new()));
    let (mut m, s, mut banks, ctx, mut acct) = unread_on(log);
    m.edit.update_link(LOCAL, |l| {
        l.role_seen = Some("viewer".into());
        true
    });
    let t = strip(&ctx, &mut m, &mut banks, &mut acct, &s, LOCAL);
    assert!(!t.contains("Your changes were sent"), "{t}");
}

/// (r33) The Banks list reads a job's answer under the account rule: another account signing in closes the first
/// one's screens there too.
#[test]
fn c75_r33_the_banks_list_keeps_the_account_rule() {
    let ((mut m, s, _banks, _ctx, _acct), release) = {
        let (mut m, s, mut banks) = two_at_three();
        background(&mut m);
        let ctx = ctx();
        let mut acct = FakeAccount { open: true, ..FakeAccount::default() };
        let olga = as_who(&s, "olga");
        s.keep("Get", &format!("/circles/{CID}"), 200, detail("owner", json!({})));
        strip_as(&ctx, &mut m, &mut banks, &mut acct, &olga, LOCAL);
        s.on("Get", &versions(), 200, page(3, &[3, 2, 1]));
        history_on_a(&mut m);
        for _ in 0..500 {
            strip_as(&ctx, &mut m, &mut banks, &mut acct, &olga, LOCAL);
            if loaded(&m) && m.links.check_of(LOCAL) != Check::Checking {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        m.edit.ask_restore(1);
        let n = s.paths().len();
        let release = s.hold();
        m.edit.confirm_restore(false);
        wait_more(&s, n);
        ((m, s, banks, ctx, acct), release)
    };
    assert!(m.edit.active() && m.edit.acting());
    // pete signs in while the person is on the Banks list
    m.tick_background(now(), &as_who(&s, "pete"));
    assert!(!m.edit.active(), "olga's History closed for pete: {:?}", m.edit.view);
    let _ = release.send(());
}

// ---- round 7's own changes, pinned --------------------------------------------------------------------------------------

/// Get latest pressed on the Send page's own conflict: the person acted on it; nothing is added above the merge.
#[test]
fn r7_get_latest_on_the_send_pages_conflict_adds_no_result() {
    let (mut m, s, mut banks) = two_at_three();
    let ctx = ctx();
    let mut acct = FakeAccount { open: true, ..FakeAccount::default() };
    strip(&ctx, &mut m, &mut banks, &mut acct, &s, LOCAL);
    banks[0].questions[0].stem = "One (my fix)".into();
    banks[0].modified += 5;
    assert!(m.edit.open_send(LOCAL, owner()));
    m.edit.host = Some(EditHost { local_id: LOCAL.into(), back: "< Back to Unit 2 quiz".into() });
    for _ in 0..3 {
        strip(&ctx, &mut m, &mut banks, &mut acct, &s, LOCAL);
    }
    m.edit.set_attest(true);
    s.on("Put", &put_path(), 409, json!({"error": "conflict", "head_version": 5}));
    m.edit.send();
    m.edit.sync(&mut banks);
    strip(&ctx, &mut m, &mut banks, &mut acct, &s, LOCAL);
    assert!(matches!(&m.edit.view, EditView::Send(v) if matches!(v.stage, SendStage::Conflict(_))), "{}", phase(&m.edit));
    s.on("Post", &format!("/circles/{CID}/banks/{BID}/merge"), 400, json!({"error": "invalid_request", "field": "base_version"}));
    assert!(m.edit.open_merge(LOCAL, owner()));
    assert!(m.edit.results_of(LOCAL).is_empty(), "{}", said(&m, LOCAL));
}

/// A send job that died while its page was left undrawn, then the page closed: said on the strip.
#[test]
fn r7_a_dead_send_job_on_a_page_left_undrawn_is_said_when_the_page_closes() {
    let (mut m, s, mut banks) = two_at_three();
    background(&mut m);
    let ctx = ctx();
    let mut acct = FakeAccount { open: true, ..FakeAccount::default() };
    strip(&ctx, &mut m, &mut banks, &mut acct, &s, LOCAL);
    send_ready_as(&ctx, &mut m, &mut banks, &mut acct, &s, owner());
    // (no answer scripted: the job's thread ends without one)
    let n = s.paths().len();
    let release = s.hold();
    m.edit.send();
    m.edit.sync(&mut banks);
    wait_more(&s, n);
    strip(&ctx, &mut m, &mut banks, &mut acct, &s, LOCAL_B);
    release.send(()).unwrap();
    until(&ctx, &mut m, &mut banks, &mut acct, &s, LOCAL_B, &|m| !m.edit.acting());
    assert!(matches!(&m.edit.view, EditView::Send(v) if matches!(v.stage, SendStage::Failed(_))), "{}", phase(&m.edit));
    m.open_circle_at(CID);
    assert!(said(&m, LOCAL).contains("Your changes were not sent"), "{}", said(&m, LOCAL));
}

/// A send that went through, answered with the page closed, moves the link on as it is now: what a check learnt
/// meanwhile stays (critic 75 minor 4, the same for a send that went).
#[test]
fn r7_a_send_answered_with_the_page_closed_keeps_what_a_check_learnt_meanwhile() {
    let (mut m, s, mut banks) = two_at_three();
    background(&mut m);
    let ctx = ctx();
    let mut acct = FakeAccount { open: true, ..FakeAccount::default() };
    strip(&ctx, &mut m, &mut banks, &mut acct, &s, LOCAL);
    send_ready_as(&ctx, &mut m, &mut banks, &mut acct, &s, owner());
    s.on("Put", &put_path(), 201, sent_body(4, json!({})));
    let n = s.paths().len();
    let release = s.hold();
    m.edit.send();
    m.edit.sync(&mut banks);
    wait_more(&s, n);
    m.open_circle_at(CID);
    m.edit.update_link(LOCAL, |l| {
        l.role_seen = Some("viewer".into());
        true
    });
    release.send(()).unwrap();
    until(&ctx, &mut m, &mut banks, &mut acct, &s, LOCAL_B, &|m| !m.edit.acting());
    assert_eq!(m.edit.with_link(LOCAL, |l| (l.base_version, l.role_seen.clone())), Some((4, Some("viewer".to_string()))));
}

/// "Sent, but not read" is said to the account that sent it only, and again when it is back after another one.
#[test]
fn r7_sent_but_not_read_is_kept_for_the_account_that_sent_it() {
    let (mut m, s, mut banks) = two_at_three();
    let ctx = ctx();
    let mut acct = FakeAccount { open: true, ..FakeAccount::default() };
    let (olga, pete) = (as_who(&s, "olga"), as_who(&s, "pete"));
    s.keep("Get", &format!("/circles/{CID}"), 200, detail("owner", json!({})));
    strip_as(&ctx, &mut m, &mut banks, &mut acct, &olga, LOCAL);
    banks[0].questions[0].stem = "One (my fix)".into();
    banks[0].modified += 5;
    assert!(m.edit.open_send(LOCAL, editor()));
    m.edit.host = Some(EditHost { local_id: LOCAL.into(), back: "< Back to Unit 2 quiz".into() });
    for _ in 0..3 {
        strip_as(&ctx, &mut m, &mut banks, &mut acct, &olga, LOCAL);
    }
    m.edit.set_attest(true);
    s.on("Put", &put_path(), 201, json!({"circle_id": CID, "bank_id": BID, "version": 0, "qids": [], "added": 0, "changed": 0, "removed": 0}));
    m.edit.send();
    m.edit.sync(&mut banks);
    strip_as(&ctx, &mut m, &mut banks, &mut acct, &olga, LOCAL);
    m.edit.close();
    assert!(m.edit.sent_unread(LOCAL));
    let t = strip_as(&ctx, &mut m, &mut banks, &mut acct, &pete, LOCAL);
    assert!(!m.edit.sent_unread(LOCAL) && !t.contains("Your changes were sent"), "{t}");
    strip_as(&ctx, &mut m, &mut banks, &mut acct, &olga, LOCAL);
    let t = strip_as(&ctx, &mut m, &mut banks, &mut acct, &olga, LOCAL);
    assert!(m.edit.sent_unread(LOCAL) && t.contains("Your changes were sent"), "olga, back: {t}");
}

/// Sending again (here: Try again, stopped before anything went because the bank is gone) leaves no word of the last
/// answer to be said when the page closes.
#[test]
fn r7_sending_again_drops_what_the_last_answer_would_have_said() {
    let (mut m, s, mut banks) = two_at_three();
    let ctx = ctx();
    let mut acct = FakeAccount { open: true, ..FakeAccount::default() };
    strip(&ctx, &mut m, &mut banks, &mut acct, &s, LOCAL);
    banks[0].questions[0].stem = "One (my fix)".into();
    banks[0].modified += 5;
    assert!(m.edit.open_send(LOCAL, owner()));
    m.edit.host = Some(EditHost { local_id: LOCAL.into(), back: "< Back to Unit 2 quiz".into() });
    for _ in 0..3 {
        strip(&ctx, &mut m, &mut banks, &mut acct, &s, LOCAL);
    }
    m.edit.set_attest(true);
    s.on("Put", &put_path(), 503, json!({"error": "unavailable"}));
    m.edit.send();
    m.edit.sync(&mut banks);
    strip(&ctx, &mut m, &mut banks, &mut acct, &s, LOCAL);
    assert!(matches!(&m.edit.view, EditView::Send(v) if matches!(v.stage, SendStage::Failed(_))), "{}", phase(&m.edit));
    // Try again, with the bank gone from this computer meanwhile: nothing is sent
    let mut gone: Vec<QuestionBank> = banks.iter().filter(|b| b.id != LOCAL).cloned().collect();
    m.edit.retry_act();
    m.edit.sync(&mut gone);
    assert!(matches!(&m.edit.view, EditView::Send(v) if matches!(&v.stage, SendStage::Failed(p) if p.message.contains("not on this device"))), "{}", phase(&m.edit));
    m.edit.close();
    assert!(!said(&m, LOCAL).contains("not sent"), "{}", said(&m, LOCAL));
}

/// (owner decision, round 7) The "shared pool" texts a circle screen can show say "circle" there: the email-invite help
/// and the invite-code and server-not-set-up refusals. The public pool's own screens keep their words.
#[test]
fn r7_owner_circle_screens_say_circle_not_shared_pool() {
    use super::circles_text::{EMAIL_NEEDS_ACCOUNT, circle_words};
    use super::pool::{INVITE_REQUIRED_TEXT, Refusal, RefusalKind};
    // (critic 78 I3, round 8: which invite code is meant is said; still no "pool")
    assert!(!says_pool(EMAIL_NEEDS_ACCOUNT) && EMAIL_NEEDS_ACCOUNT.contains("its account invite code"), "{EMAIL_NEEDS_ACCOUNT}");
    let invite = circle_words(INVITE_REQUIRED_TEXT);
    assert!(!says_pool(&invite) && invite.starts_with("Your verSHion account needs an invite code"), "{invite}");
    let detail = super::api::ErrorDetail { status: 503, code: "pool_not_configured".into(), field: None, reason: None, ack_allowed: None, author_only: false, retry_after: None, json: json!({"error": "pool_not_configured"}) };
    let kind = RefusalKind::classify(&detail);
    let not_set_up = circle_words(&super::pool::PoolError::Refused(Box::new(Refusal { kind, detail })).message());
    assert!(!says_pool(&not_set_up) && not_set_up.starts_with("Circles are not available on this server yet."), "{not_set_up}");
    assert!(INVITE_REQUIRED_TEXT.starts_with("The shared pool"), "the public pool's screens keep their words");
}
