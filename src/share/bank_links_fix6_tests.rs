//! Phase D1, round 6 (critics 73 and 74): each test written first against the code it pins. A send's outcome is said
//! for its bank when its page was closed under it (the Shared page opened, another bank's "Circle page", a sign-out);
//! a send whose answer could not be read is never counted as unsent and offers Get latest; the strip never says "up
//! to date" while a restore with no answer may still land; a signed-out answer for an earlier session of the account
//! signed in now does not ask it to sign in; an account back after another one is told how its own restore went; a
//! result about a bank with no copy here is read on the Banks list; results that stopped being true go. And the
//! behaviour critic 73's surviving mutants changed, pinned.

use std::sync::Arc;

use serde_json::json;


use super::bank_links::Check;
use super::bank_links_fix4_tests::*;
use super::bank_links_fix_tests::*;
use super::bank_links_tests::*;
use super::circle_edit::*;
use super::circles::CirclesModel;
use super::circles_tests::*;
use crate::model::QuestionBank;

fn put_path() -> String {
    format!("/circles/{CID}/banks/{BID}")
}

fn rollback_path(bid: &str) -> String {
    format!("/circles/{CID}/banks/{bid}/rollback")
}

fn trim_path(bid: &str) -> String {
    format!("/circles/{CID}/banks/{bid}/trim")
}

/// A linked bank's page, as the account in `snap` sees it.
fn strip_as(ctx: &egui::Context, m: &mut CirclesModel, banks: &mut [QuestionBank], acct: &mut FakeAccount, snap: &super::publish::AccountSnapshot, local: &str) -> String {
    let mut b = browse();
    frame_text(ctx, &mut |ui| {
        super::bank_links_ui::strip_with(ui, m, banks, &mut b, acct, snap, now(), NOW, local);
    })
}

/// Waits until the scripted server was asked for a path ending in `suffix` (a job on its own thread).
fn wait_path(s: &Arc<Script>, suffix: &str) {
    for _ in 0..500 {
        if s.paths().iter().any(|p| p.ends_with(suffix)) {
            return;
        }
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
    panic!("never reached {suffix}: {:?}", s.paths());
}

/// Waits until the scripted server was asked for more than `n` requests (a job on its own thread reached it; a held
/// one waits there).
fn wait_more(s: &Arc<Script>, n: usize) {
    for _ in 0..500 {
        if s.paths().len() > n {
            return;
        }
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
    panic!("no request after {n}: {:?}", s.paths());
}

/// A's Send page on A's page, licence known, the own-work box ticked, one change; both banks' checks answered.
fn send_ready(ctx: &egui::Context, m: &mut CirclesModel, banks: &mut Vec<QuestionBank>, acct: &mut FakeAccount, s: &Arc<Script>) {
    until(ctx, m, banks, acct, s, LOCAL, &|m| m.links.check_of(LOCAL) != Check::Checking);
    until(ctx, m, banks, acct, s, LOCAL_B, &|m| m.links.check_of(LOCAL_B) != Check::Checking);
    banks[0].questions[0].stem = "One (my fix)".into();
    banks[0].modified += 5;
    assert!(m.edit.open_send(LOCAL, owner()));
    m.edit.host = Some(EditHost { local_id: LOCAL.into(), back: "< Back to Unit 2 quiz".into() });
    m.edit.host_frame = None;
    until(ctx, m, banks, acct, s, LOCAL, &|m| matches!(&m.edit.view, EditView::Send(v) if matches!(v.licence, LicenceState::Known(_))));
    m.edit.set_attest(true);
    assert!(m.edit.send_blocker().is_none(), "blocked: {:?} {}", m.edit.send_blocker(), phase(&m.edit));
}

/// A's send held at the server; meanwhile, on B's page, "..." > "Circle page" opens the Shared page on the circle
/// (the circles model is reset, A's Send page closes); then the answer `status`/`body` comes, read on B's page.
fn send_answered_on_b(status: u16, body: serde_json::Value) -> (CirclesModel, Arc<Script>, Vec<QuestionBank>, egui::Context, FakeAccount) {
    let (mut m, s, mut banks) = two_at_three();
    background(&mut m);
    let ctx = ctx();
    let mut acct = FakeAccount { open: true, ..FakeAccount::default() };
    strip(&ctx, &mut m, &mut banks, &mut acct, &s, LOCAL);
    send_ready(&ctx, &mut m, &mut banks, &mut acct, &s);
    s.on("Put", &put_path(), status, body);
    let n = s.paths().len();
    let release = s.hold();
    m.edit.send();
    m.edit.sync(&mut banks);
    wait_more(&s, n);
    strip(&ctx, &mut m, &mut banks, &mut acct, &s, LOCAL_B);
    m.open_circle_at(CID);
    assert!(!m.edit.active(), "the Send page closed under the send");
    release.send(()).unwrap();
    until(&ctx, &mut m, &mut banks, &mut acct, &s, LOCAL_B, &|m| !m.edit.acting());
    (m, s, banks, ctx, acct)
}

/// The same, then A's strip.
fn send_left(status: u16, body: serde_json::Value) -> (CirclesModel, Arc<Script>, Vec<QuestionBank>, egui::Context, FakeAccount, String) {
    let (mut m, s, mut banks, ctx, mut acct) = send_answered_on_b(status, body);
    let t = strip(&ctx, &mut m, &mut banks, &mut acct, &s, LOCAL);
    (m, s, banks, ctx, acct, t)
}

fn said(m: &CirclesModel, local: &str) -> String {
    m.edit.results_of(local).into_iter().map(|r| r.1).collect::<Vec<_>>().join(" | ")
}

// ---- c73 I1: a send's outcome when its page was closed under it ----------------------------------------------------

#[test]
fn c73_i1_a_send_that_failed_while_its_page_was_closed_is_said_on_its_strip() {
    let (m, _s, _b, _c, _a, t) = send_left(503, json!({"error": "unavailable"}));
    let r = said(&m, LOCAL);
    assert!(r.contains("Your changes were not sent"), "{r}");
    assert!(t.contains("Your changes were not sent") && t.contains("Dismiss"), "{t}");
    // nothing about it on B's strip
    assert!(m.edit.results_of(LOCAL_B).is_empty());
}

#[test]
fn c73_i1_a_send_refused_by_the_daily_limit_while_its_page_was_closed_says_why() {
    let (m, _s, _b, _c, _a, t) = send_left(429, json!({"error": "daily_limit", "metric": "circle_commits"}));
    assert!(t.contains("Your changes were not sent") && t.contains("today"), "{t}");
    assert_eq!(said(&m, LOCAL).matches("still on this").count(), 1, "said once: {}", said(&m, LOCAL));
}

#[test]
fn c73_i1_a_send_refused_as_a_conflict_while_its_page_was_closed_names_the_newer_version_and_offers_get_latest() {
    let (mut m, _s, _b, _c, _a, t) = send_left(409, json!({"error": "conflict", "head_version": 5}));
    assert!(t.contains("someone sent version 5 first"), "{t}");
    assert!(!t.contains("Version 3 is the latest"), "the old version is not named as the latest: {t}");
    assert!(t.contains("Version 5 is the latest") && t.contains("Get latest"), "{t}");
    assert_eq!(m.edit.with_link(LOCAL, |l| l.head_seen), Some(5));
}

#[test]
fn c73_i1_a_conflict_that_names_no_version_starts_a_check_of_the_bank() {
    let (mut m, s, mut banks, ctx, mut acct) = send_answered_on_b(409, json!({"error": "conflict", "head_version": null}));
    // A was checked a moment ago: only a check asked for runs now
    let before = heads_count(&s);
    let t = strip(&ctx, &mut m, &mut banks, &mut acct, &s, LOCAL);
    assert!(t.contains("someone sent a newer version first"), "{t}");
    assert_eq!(heads_count(&s), before + 1, "A's newest version is checked at once");
}

#[test]
fn c73_i1_a_send_that_needs_choices_while_its_page_was_closed_says_to_open_send_again() {
    let (m, _s, _b, _c, _a, t) = send_left(428, json!({"error": "confirm_large_required", "removed": 2, "total": 3}));
    assert!(t.contains("Your changes were not sent") && t.contains("2 of the bank's 3 questions"), "{t}");
    let _ = m;
}

#[test]
fn c73_i1_p9_a_send_held_for_review_while_its_page_was_closed_says_so_on_the_strip() {
    let (m, _s, _b, _c, _a, t) = send_left(201, json!({"circle_id": CID, "bank_id": BID, "version": 4, "version_id": hex(4), "qids": [QA], "added": 0, "changed": 1, "removed": 0, "held": true}));
    assert!(t.contains("Version 4 was sent") && t.contains("hidden from the rest of the circle"), "{t}");
    assert!(!t.contains("unsent"), "the changes went: {t}");
    // the link moved on all the same
    let mut m = m;
    assert_eq!(m.edit.with_link(LOCAL, |l| l.base_version), Some(4));
}

#[test]
fn c73_i1_a_plain_send_that_went_through_while_its_page_was_closed_adds_no_result() {
    let (mut m, _s, _b, _c, _a, t) = send_left(201, json!({"circle_id": CID, "bank_id": BID, "version": 4, "version_id": hex(4), "qids": [QA], "added": 0, "changed": 1, "removed": 0}));
    assert!(m.edit.results_of(LOCAL).is_empty(), "{}", said(&m, LOCAL));
    assert!(t.contains("Version 4 is the latest") && t.contains("up to date"), "{t}");
    assert_eq!(m.edit.with_link(LOCAL, |l| l.base_version), Some(4));
}

#[test]
fn c73_i1_a_signed_out_answer_to_a_send_whose_page_was_closed_says_the_session_ended() {
    let (mut m, s, mut banks) = two_at_three();
    background(&mut m);
    let ctx = ctx();
    let mut acct = FakeAccount { open: true, ..FakeAccount::default() };
    strip(&ctx, &mut m, &mut banks, &mut acct, &s, LOCAL);
    send_ready(&ctx, &mut m, &mut banks, &mut acct, &s);
    s.fail(super::api::ApiError::SignedOut);
    let n = s.paths().len();
    let release = s.hold();
    m.edit.send();
    m.edit.sync(&mut banks);
    wait_more(&s, n);
    strip(&ctx, &mut m, &mut banks, &mut acct, &s, LOCAL_B);
    m.open_circle_at(CID);
    release.send(()).unwrap();
    until(&ctx, &mut m, &mut banks, &mut acct, &s, LOCAL_B, &|m| !m.edit.acting());
    let t = strip(&ctx, &mut m, &mut banks, &mut acct, &s, LOCAL);
    assert!(t.contains("Your session ended before the send was answered"), "{t}");
    assert!(t.contains("Sign in"), "{t}");
}

#[test]
fn c73_i1_a_send_whose_job_died_while_its_page_was_closed_says_it_did_not_go() {
    let (mut m, s, mut banks) = two_at_three();
    background(&mut m);
    let ctx = ctx();
    let mut acct = FakeAccount { open: true, ..FakeAccount::default() };
    strip(&ctx, &mut m, &mut banks, &mut acct, &s, LOCAL);
    send_ready(&ctx, &mut m, &mut banks, &mut acct, &s);
    // no reply scripted for the send: the scripted session panics, the job's thread dies
    let n = s.paths().len();
    let release = s.hold();
    m.edit.send();
    m.edit.sync(&mut banks);
    wait_more(&s, n);
    m.open_circle_at(CID);
    release.send(()).unwrap();
    until(&ctx, &mut m, &mut banks, &mut acct, &s, LOCAL_B, &|m| !m.edit.acting());
    let r = said(&m, LOCAL);
    assert!(r.contains("Your changes were not sent"), "{r}");
}

// ---- c74 I1: a send whose answer could not be read ------------------------------------------------------------------

const UNREAD: &str = r#"{"circle_id": "CID", "bank_id": "BID", "version": 0, "qids": [], "added": 0, "changed": 0, "removed": 0}"#;

fn unread_body() -> serde_json::Value {
    serde_json::from_str(&UNREAD.replace("CID", CID).replace("BID", BID)).unwrap()
}

/// A's Send page on A's page (inline jobs), one change, sent; the answer has no version the app can read.
fn send_unread_here() -> (CirclesModel, Arc<Script>, Vec<QuestionBank>, egui::Context, FakeAccount) {
    let (mut m, s, mut banks) = two_at_three();
    let ctx = ctx();
    let mut acct = FakeAccount { open: true, ..FakeAccount::default() };
    strip(&ctx, &mut m, &mut banks, &mut acct, &s, LOCAL);
    banks[0].questions[0].stem = "One (my fix)".into();
    banks[0].modified += 5;
    assert!(m.edit.open_send(LOCAL, editor()));
    m.edit.host = Some(EditHost { local_id: LOCAL.into(), back: "< Back to Unit 2 quiz".into() });
    for _ in 0..3 {
        strip(&ctx, &mut m, &mut banks, &mut acct, &s, LOCAL);
    }
    m.edit.set_attest(true);
    s.on("Put", &put_path(), 201, unread_body());
    m.edit.send();
    m.edit.sync(&mut banks);
    (m, s, banks, ctx, acct)
}

fn badge_text(m: &mut CirclesModel, banks: &[QuestionBank], local: &str) -> String {
    let bank = banks.iter().find(|b| b.id == local).unwrap().clone();
    let CirclesModel { links, edit, .. } = m;
    links.badge(edit, &bank).and_then(|b| b.note).unwrap_or_default()
}

#[test]
fn c74_i1_the_sent_box_never_says_version_0_and_says_it_once() {
    let (mut m, s, mut banks, ctx, mut acct) = send_unread_here();
    let t = strip(&ctx, &mut m, &mut banks, &mut acct, &s, LOCAL);
    assert!(matches!(&m.edit.view, EditView::Send(v) if matches!(v.stage, SendStage::Done(_))), "{}", phase(&m.edit));
    assert!(!t.contains("Version 0") && !t.contains("0 added"), "{t}");
    assert!(t.contains("Your changes were sent"), "{t}");
    assert_eq!(t.matches("could not be read").count(), 1, "said once on the page: {t}");
}

#[test]
fn c74_i1_after_done_the_strip_offers_get_latest_and_counts_nothing_as_unsent() {
    let (mut m, s, mut banks, ctx, mut acct) = send_unread_here();
    strip(&ctx, &mut m, &mut banks, &mut acct, &s, LOCAL);
    // "Done"
    m.edit.close();
    let t = strip(&ctx, &mut m, &mut banks, &mut acct, &s, LOCAL);
    assert!(!t.contains("unsent"), "{t}");
    assert!(t.contains("Get latest") && !t.contains("Send my changes"), "{t}");
    assert!(t.contains("Press Get latest to catch up") && t.contains("Dismiss"), "the result is said on the strip: {t}");
    let badge = badge_text(&mut m, &banks, LOCAL);
    assert!(!badge.contains("unsent") && badge.contains("newer version"), "{badge}");
    // dismissed, the strip still offers Get latest and still counts nothing as unsent
    let id = m.edit.results_of(LOCAL)[0].0;
    m.edit.dismiss_result(id);
    let t = strip(&ctx, &mut m, &mut banks, &mut acct, &s, LOCAL);
    assert!(!t.contains("unsent") && t.contains("Get latest"), "{t}");
}

#[test]
fn c74_i1_a_send_whose_answer_could_not_be_read_while_its_page_was_closed_offers_get_latest() {
    let (m, _s, _b, _c, _a, t) = send_left(201, unread_body());
    assert!(!t.contains("unsent") && t.contains("Get latest"), "{t}");
    assert!(t.contains("Press Get latest to catch up"), "{t}");
    assert_eq!(m.edit.results_of(LOCAL).len(), 1);
}

#[test]
fn c74_i1_get_latest_that_catches_up_ends_the_sent_state() {
    let (mut m, s, mut banks, ctx, mut acct) = send_unread_here();
    strip(&ctx, &mut m, &mut banks, &mut acct, &s, LOCAL);
    m.edit.close();
    assert!(m.edit.sent_unread(LOCAL));
    // Get latest: the circle has nothing newer than this copy
    s.on("Post", &format!("/circles/{CID}/banks/{BID}/merge"), 400, json!({"error": "invalid_request", "field": "base_version"}));
    assert!(m.edit.open_merge(LOCAL, editor()));
    m.edit.host = Some(EditHost { local_id: LOCAL.into(), back: "< Back to Unit 2 quiz".into() });
    for _ in 0..3 {
        strip(&ctx, &mut m, &mut banks, &mut acct, &s, LOCAL);
    }
    m.edit.cancel_merge();
    assert!(!m.edit.sent_unread(LOCAL));
    let t = strip(&ctx, &mut m, &mut banks, &mut acct, &s, LOCAL);
    assert!(!t.contains("Your changes were sent"), "{t}");
}

#[test]
fn c74_i1_a_later_send_that_is_read_ends_the_sent_state_and_unlinking_forgets_it() {
    let (mut m, s, mut banks, ctx, mut acct) = send_unread_here();
    strip(&ctx, &mut m, &mut banks, &mut acct, &s, LOCAL);
    m.edit.close();
    assert!(m.edit.sent_unread(LOCAL));
    m.edit.forget(LOCAL);
    assert!(!m.edit.sent_unread(LOCAL));
    let _ = (&s, &ctx, &mut acct, &mut banks);
}

#[test]
fn c74_minor_the_share_windows_sign_in_step_says_a_circle_when_opened_for_one() {
    let mut p = super::publish::PublishModel::new(super::model::Runner::Inline, Arc::new(std::collections::HashMap::<String, Vec<u8>>::new()), super::publish::LogIo::memory());
    let src = super::publish::ShareSource::from_bank_json("bank:b1", "Bank: Unit 1", "Unit 1", &json!({"id": "b1", "name": "Unit 1", "questions": [], "created": 1, "modified": 2}).to_string());
    p.start_for_circle(src.clone(), Some(CID.into()));
    assert_eq!(p.step, super::publish::Step::Gate);
    assert_eq!(super::publish_ui::title(&p), "Share to a circle");
    p.start(src);
    assert_eq!(super::publish_ui::title(&p), "Share to the pool");
}

// ---- c73 minors -------------------------------------------------------------------------------------------------------

/// A's History on A's page (a model whose jobs run on their own threads), loaded, A's check answered; a restore of
/// version 1 asked for and held at the server (its answer `reply`, when let go).
fn restore_held(reply: (u16, serde_json::Value)) -> (CirclesModel, Arc<Script>, Vec<QuestionBank>, egui::Context, FakeAccount, std::sync::mpsc::Sender<()>) {
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
    s.on("Post", &rollback_path(BID), reply.0, reply.1);
    let n = s.paths().len();
    let release = s.hold();
    m.edit.confirm_restore(false);
    wait_more(&s, n);
    (m, s, banks, ctx, acct, release)
}

#[test]
fn c73_minor1_after_the_bounded_wait_the_strip_never_says_up_to_date_until_the_next_check() {
    let (mut m, s, mut banks, ctx, mut acct, release) = restore_held((200, rolled(4, 1)));
    m.edit.back();
    m.edit.frame(now() + ACT_PATIENCE, &ready(&s));
    assert!(!m.edit.acting());
    let t = strip(&ctx, &mut m, &mut banks, &mut acct, &s, LOCAL);
    assert!(!t.contains("up to date") && !t.contains("is the latest"), "{t}");
    assert!(t.contains("Version 3 when last checked") && t.contains("No answer came"), "{t}");
    // the restore lands after all; the strip still does not say up to date
    release.send(()).unwrap();
    std::thread::sleep(std::time::Duration::from_millis(100));
    let t = strip(&ctx, &mut m, &mut banks, &mut acct, &s, LOCAL);
    assert!(!t.contains("up to date"), "{t}");
    // "Check now": the circle says version 4; the strip says so, and "No answer came" goes (it did go through)
    s.keep("Post", "/circles/heads", 200, heads(4));
    m.links.check_now(LOCAL);
    let t = until(&ctx, &mut m, &mut banks, &mut acct, &s, LOCAL, &|m| m.links.check_of(LOCAL) == Check::Done && !m.edit.unsure(LOCAL));
    let t = t + &strip(&ctx, &mut m, &mut banks, &mut acct, &s, LOCAL);
    assert!(t.contains("Version 4 is the latest. You have version 3."), "{t}");
    assert!(!t.contains("No answer came"), "{t}");
}

#[test]
fn c73_minor1_a_history_read_ends_the_unsure_state() {
    let (mut m, s, mut banks, ctx, mut acct, release) = restore_held((200, rolled(4, 1)));
    m.edit.back();
    m.edit.frame(now() + ACT_PATIENCE, &ready(&s));
    assert!(m.edit.unsure(LOCAL));
    let _ = release.send(());
    s.on("Get", &versions(), 200, page(3, &[3, 2, 1]));
    history_on_a(&mut m);
    until(&ctx, &mut m, &mut banks, &mut acct, &s, LOCAL, &loaded);
    assert!(!m.edit.unsure(LOCAL));
    m.edit.back();
    let t = strip(&ctx, &mut m, &mut banks, &mut acct, &s, LOCAL);
    assert!(t.contains("Version 3 is the latest") && t.contains("up to date"), "the circle said version 3: {t}");
    assert!(t.contains("No answer came"), "the head did not move: the note stays until dismissed: {t}");
}

#[test]
fn c73_minor3_the_same_account_signed_in_again_is_not_asked_to_sign_in_by_its_earlier_sessions_answer() {
    for trim in [false, true] {
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
        let n = s.paths().len();
        let release = s.hold();
        if trim {
            m.edit.open_trim();
            m.edit.set_trim_keep(2);
            m.edit.ask_trim();
            m.edit.confirm_trim();
        } else {
            m.edit.ask_restore(1);
            m.edit.confirm_restore(false);
        }
        wait_more(&s, n);
        m.edit.back();
        let out = super::publish::AccountSnapshot::of_gate(super::publish::Gate::SignedOut);
        strip_as(&ctx, &mut m, &mut banks, &mut acct, &out, LOCAL);
        strip_as(&ctx, &mut m, &mut banks, &mut acct, &olga, LOCAL);
        for _ in 0..500 {
            strip_as(&ctx, &mut m, &mut banks, &mut acct, &olga, LOCAL);
            if m.links.check_of(LOCAL) != Check::Checking {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        s.fail(super::api::ApiError::SignedOut);
        release.send(()).unwrap();
        for _ in 0..200 {
            m.edit.frame(now(), &olga);
            if !m.edit.acting() {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        let t = strip_as(&ctx, &mut m, &mut banks, &mut acct, &olga, LOCAL) + &strip_as(&ctx, &mut m, &mut banks, &mut acct, &olga, LOCAL);
        assert!(!t.contains("Sign in"), "trim {trim}: {t}");
        let want = if trim { "Your session ended before the trim was answered. Open History to see whether it went through." } else { "Your session ended before the restore of version 1 was answered. Open History to see whether it went through." };
        assert!(t.contains(want), "trim {trim}: {t}");
        // (a restore that may have gone through: the strip does not say up to date until the circle is read again)
        assert!(trim || !t.contains("up to date"), "{t}");
    }
}

#[test]
fn c73_minor3_a_send_answered_signed_out_for_an_earlier_session_offers_try_again_not_sign_in() {
    let (mut m, s, mut banks) = two_at_three();
    background(&mut m);
    let ctx = ctx();
    let mut acct = FakeAccount { open: true, ..FakeAccount::default() };
    let olga = as_who(&s, "olga");
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
    let n = s.paths().len();
    let release = s.hold();
    m.edit.send();
    m.edit.sync(&mut banks);
    wait_more(&s, n);
    // olga signs out and in again on the Shared page while it is on its way (the Send page closes)
    let out = super::publish::AccountSnapshot::of_gate(super::publish::Gate::SignedOut);
    m.note_account(&out);
    m.edit.frame(now(), &out);
    m.note_account(&olga);
    m.edit.frame(now(), &olga);
    s.fail(super::api::ApiError::SignedOut);
    release.send(()).unwrap();
    for _ in 0..200 {
        m.edit.frame(now(), &olga);
        if !m.edit.acting() {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
    let t = strip_as(&ctx, &mut m, &mut banks, &mut acct, &olga, LOCAL) + &strip_as(&ctx, &mut m, &mut banks, &mut acct, &olga, LOCAL);
    assert!(!t.contains("Sign in"), "{t}");
    assert!(t.contains("Your session ended before the send was answered. Press Send my changes to send them again"), "{t}");
}

#[test]
fn c73_minor4_an_account_back_after_another_is_told_how_its_own_restore_went() {
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
    let during = strip_as(&ctx, &mut m, &mut banks, &mut acct, &pete, LOCAL);
    assert!(!during.contains("Restoring"), "another account is not told: {during}");
    let back = strip_as(&ctx, &mut m, &mut banks, &mut acct, &olga, LOCAL);
    assert!(back.contains("Restoring version 1 in the circle"), "olga, back, is told her own restore is on its way: {back}");
    for _ in 0..500 {
        strip_as(&ctx, &mut m, &mut banks, &mut acct, &olga, LOCAL);
        if m.links.check_of(LOCAL) != Check::Checking {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
    s.on("Post", &rollback_path(BID), 503, json!({"error": "unavailable"}));
    release.send(()).unwrap();
    for _ in 0..200 {
        m.edit.frame(now(), &olga);
        if !m.edit.acting() {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
    let t = strip_as(&ctx, &mut m, &mut banks, &mut acct, &olga, LOCAL);
    assert!(t.contains("The restore of version 1 did not go through"), "{t}");
}

#[test]
fn c73_minor5_a_result_about_a_bank_with_no_copy_here_is_said_on_the_banks_list_alone() {
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
    m.edit.ask_restore(1);
    s.on("Post", &rollback_path(&z), 200, json!({"circle_id": CID, "bank_id": z, "version": 4, "version_id": hex(4), "qids": [QA], "added": 0, "changed": 1, "removed": 0, "rollback_of": 1}));
    m.edit.confirm_restore(false);
    m.edit.back();
    // My Banks, the Banks list only: the app's tick and the list's rows
    let mut seen = false;
    for _ in 0..300 {
        m.tick_background(now(), &ready(&s));
        if rows(&mut m).contains("\"Unit 3 test\": Version 4 was restored from version 1.") {
            seen = true;
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
    assert!(seen, "{}", rows(&mut m));
    assert!(!m.edit.acting());
}

#[test]
fn c73_minor6_a_restore_that_landed_takes_away_used_as_it_is() {
    let (mut m, s, mut banks) = two_at_three();
    let ctx = ctx();
    let mut acct = FakeAccount { open: true, ..FakeAccount::default() };
    strip(&ctx, &mut m, &mut banks, &mut acct, &s, LOCAL);
    make_stale(&mut m, LOCAL);
    treat_in_step(&mut m, &s, &mut banks, LOCAL, BID, false);
    assert!(strip(&ctx, &mut m, &mut banks, &mut acct, &s, LOCAL).contains("now used as it is"));
    s.on("Get", &versions(), 200, page(3, &[3, 2, 1]));
    history_on_a(&mut m);
    strip(&ctx, &mut m, &mut banks, &mut acct, &s, LOCAL);
    strip(&ctx, &mut m, &mut banks, &mut acct, &s, LOCAL);
    m.edit.ask_restore(1);
    s.on("Post", &rollback_path(BID), 200, rolled(4, 1));
    m.edit.confirm_restore(false);
    m.edit.back();
    let t = strip(&ctx, &mut m, &mut banks, &mut acct, &s, LOCAL);
    assert!(t.contains("Version 4 was restored") && !t.contains("now used as it is"), "{t}");
}

#[test]
fn c73_minor6_a_check_that_finds_a_newer_version_takes_away_used_as_it_is() {
    let (mut m, s, mut banks) = two_at_three();
    let ctx = ctx();
    let mut acct = FakeAccount { open: true, ..FakeAccount::default() };
    strip(&ctx, &mut m, &mut banks, &mut acct, &s, LOCAL);
    make_stale(&mut m, LOCAL);
    treat_in_step(&mut m, &s, &mut banks, LOCAL, BID, false);
    assert!(strip(&ctx, &mut m, &mut banks, &mut acct, &s, LOCAL).contains("now used as it is"));
    s.keep("Post", "/circles/heads", 200, heads(5));
    m.links.check_now(LOCAL);
    // (the check runs in this frame; its answer is read in the next)
    strip(&ctx, &mut m, &mut banks, &mut acct, &s, LOCAL);
    let t = strip(&ctx, &mut m, &mut banks, &mut acct, &s, LOCAL);
    assert!(t.contains("Version 5 is the latest") && !t.contains("now used as it is"), "{t}");
}

// ---- critic 73's surviving mutants, pinned ---------------------------------------------------------------------------

#[test]
fn p12_a_send_is_waited_for_past_the_bounded_wait_and_its_answer_moves_the_link_on() {
    let (mut m, s, mut banks) = two_at_three();
    background(&mut m);
    let ctx = ctx();
    let mut acct = FakeAccount { open: true, ..FakeAccount::default() };
    strip(&ctx, &mut m, &mut banks, &mut acct, &s, LOCAL);
    send_ready(&ctx, &mut m, &mut banks, &mut acct, &s);
    s.on("Put", &put_path(), 201, json!({"circle_id": CID, "bank_id": BID, "version": 4, "version_id": hex(4), "qids": [QA], "added": 0, "changed": 1, "removed": 0}));
    let n = s.paths().len();
    let release = s.hold();
    m.edit.send();
    m.edit.sync(&mut banks);
    wait_more(&s, n);
    m.edit.frame(now() + ACT_PATIENCE * 3, &ready(&s));
    assert!(m.edit.acting(), "a send is never given up on");
    release.send(()).unwrap();
    until(&ctx, &mut m, &mut banks, &mut acct, &s, LOCAL, &|m| !m.edit.acting());
    assert_eq!(m.edit.with_link(LOCAL, |l| l.base_version), Some(4), "the link moved on to the version sent");
}

/// Olga's restore of A from its History on A's page, held at the server; then pete signs in on this computer.
fn olgas_restore_held_then_pete() -> (CirclesModel, Arc<Script>, Vec<QuestionBank>, egui::Context, FakeAccount, std::sync::mpsc::Sender<()>) {
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
    (m, s, banks, ctx, acct, release)
}

#[test]
fn p13_another_account_is_not_told_no_answer_came_for_the_first_ones_restore() {
    let (mut m, s, mut banks, ctx, mut acct, release) = olgas_restore_held_then_pete();
    let pete = as_who(&s, "pete");
    m.edit.frame(now() + ACT_PATIENCE, &pete);
    assert!(!m.edit.acting());
    let t = strip_as(&ctx, &mut m, &mut banks, &mut acct, &pete, LOCAL);
    assert!(!t.contains("No answer came"), "{t}");
    assert!(m.edit.results_of(LOCAL).is_empty());
    // and its own account, back, is told (critic 73 minor 4)
    let olga = as_who(&s, "olga");
    strip_as(&ctx, &mut m, &mut banks, &mut acct, &olga, LOCAL);
    let _ = release.send(());
}

#[test]
fn p14_after_the_bounded_wait_the_history_does_not_stay_at_restoring() {
    let (mut m, s, _banks, _ctx, _acct, release) = restore_held((200, rolled(4, 1)));
    assert!(matches!(&m.edit.view, EditView::History(h) if h.restore.is_some()));
    m.edit.frame(now() + ACT_PATIENCE, &ready(&s));
    assert!(matches!(&m.edit.view, EditView::History(h) if h.restore.is_none()), "{:?}", m.edit.view);
    let page = text_of(1280.0, &mut |ui| super::circle_edit_ui::draw_page(ui, &mut m));
    assert!(!page.contains("Restoring"), "{page}");
    let _ = release.send(());
}

#[test]
fn p14_after_the_bounded_wait_the_history_does_not_stay_at_trimming() {
    let (mut m, s, mut banks) = two_at_three();
    background(&mut m);
    let ctx = ctx();
    let mut acct = FakeAccount { open: true, ..FakeAccount::default() };
    strip(&ctx, &mut m, &mut banks, &mut acct, &s, LOCAL);
    s.on("Get", &versions(), 200, page(3, &[3, 2, 1]));
    history_on_a(&mut m);
    until(&ctx, &mut m, &mut banks, &mut acct, &s, LOCAL, &loaded);
    until(&ctx, &mut m, &mut banks, &mut acct, &s, LOCAL, &|m| m.links.check_of(LOCAL) != Check::Checking);
    m.edit.open_trim();
    m.edit.set_trim_keep(2);
    m.edit.ask_trim();
    let n = s.paths().len();
    let release = s.hold();
    m.edit.confirm_trim();
    wait_more(&s, n);
    m.edit.frame(now() + ACT_PATIENCE, &ready(&s));
    assert!(matches!(&m.edit.view, EditView::History(h) if h.trim.is_none()), "{:?}", m.edit.view);
    let _ = release.send(());
}

#[test]
fn p17_a_restore_answered_after_its_bank_was_unlinked_is_said_on_the_banks_list() {
    let (mut m, s, mut banks, ctx, mut acct, release) = restore_held((200, rolled(4, 1)));
    m.edit.back();
    m.edit.forget(LOCAL);
    release.send(()).unwrap();
    until(&ctx, &mut m, &mut banks, &mut acct, &s, LOCAL_B, &|m| !m.edit.acting());
    assert!(m.edit.results_of(LOCAL).is_empty(), "nothing is said for the link that is gone");
    let r = rows(&mut m);
    assert!(r.contains("Version 4 was restored from version 1."), "{r}");
}

/// A's History left open on the Shared page and not drawn (the person is on B's page); a restore or trim from it.
fn history_left_on_the_shared_page(trim: bool, reply: (u16, serde_json::Value)) -> (CirclesModel, Arc<Script>, Vec<QuestionBank>, egui::Context, FakeAccount) {
    let (mut m, s, mut banks) = two_at_three();
    let ctx = ctx();
    let mut acct = FakeAccount { open: true, ..FakeAccount::default() };
    strip(&ctx, &mut m, &mut banks, &mut acct, &s, LOCAL);
    s.on("Get", &versions(), 200, page(3, &[3, 2, 1]));
    assert!(m.edit.open_history(CID, BID, "Unit 2 quiz", owner()));
    m.edit.host = None;
    m.edit.frame(now(), &ready(&s));
    assert!(loaded(&m));
    // the person is on B's page: the History is not drawn there
    strip(&ctx, &mut m, &mut banks, &mut acct, &s, LOCAL_B);
    strip(&ctx, &mut m, &mut banks, &mut acct, &s, LOCAL_B);
    s.on(if trim { "Post" } else { "Post" }, &if trim { trim_path(BID) } else { rollback_path(BID) }, reply.0, reply.1);
    if trim {
        m.edit.open_trim();
        m.edit.set_trim_keep(2);
        m.edit.ask_trim();
        m.edit.confirm_trim();
    } else {
        m.edit.ask_restore(1);
        m.edit.confirm_restore(false);
    }
    (m, s, banks, ctx, acct)
}

fn versions_reads(s: &Arc<Script>) -> usize {
    s.paths().iter().filter(|p| p.ends_with(&versions())).count()
}

#[test]
fn p22_a_restore_answered_while_its_history_was_not_drawn_reads_the_history_again() {
    let (mut m, s, mut banks, ctx, mut acct) = history_left_on_the_shared_page(false, (200, rolled(4, 1)));
    s.on("Get", &versions(), 200, page(4, &[4, 3, 2, 1]));
    let before = versions_reads(&s);
    strip(&ctx, &mut m, &mut banks, &mut acct, &s, LOCAL_B);
    assert!(m.edit.results_of(LOCAL).iter().any(|r| r.1.contains("Version 4 was restored")), "said on A's strip");
    m.edit.frame(now(), &ready(&s));
    assert_eq!(versions_reads(&s), before + 1, "the History is read again");
    assert!(matches!(&m.edit.view, EditView::History(h) if h.head_version == 4), "{:?}", m.edit.view);
}

#[test]
fn p23_a_trim_answered_while_its_history_was_not_drawn_reads_the_history_again() {
    let (mut m, s, mut banks, ctx, mut acct) = history_left_on_the_shared_page(true, (200, json!({"ok": true, "pruned": 1, "oldest_kept": 2, "pinned_versions": []})));
    s.on("Get", &versions(), 200, page(3, &[3, 2]));
    let before = versions_reads(&s);
    strip(&ctx, &mut m, &mut banks, &mut acct, &s, LOCAL_B);
    assert!(m.edit.results_of(LOCAL).iter().any(|r| r.1.contains("The trim deleted")), "said on A's strip");
    m.edit.frame(now(), &ready(&s));
    assert_eq!(versions_reads(&s), before + 1, "the History is read again");
}

#[test]
fn p27_a_restore_whose_job_died_while_its_history_was_not_drawn_does_not_leave_it_at_restoring() {
    let (mut m, s, mut banks) = two_at_three();
    background(&mut m);
    let ctx = ctx();
    let mut acct = FakeAccount { open: true, ..FakeAccount::default() };
    strip(&ctx, &mut m, &mut banks, &mut acct, &s, LOCAL);
    until(&ctx, &mut m, &mut banks, &mut acct, &s, LOCAL, &|m| m.links.check_of(LOCAL) != Check::Checking);
    until(&ctx, &mut m, &mut banks, &mut acct, &s, LOCAL_B, &|m| m.links.check_of(LOCAL_B) != Check::Checking);
    s.on("Get", &versions(), 200, page(3, &[3, 2, 1]));
    assert!(m.edit.open_history(CID, BID, "Unit 2 quiz", owner()));
    m.edit.host = None;
    for _ in 0..200 {
        m.edit.frame(now(), &ready(&s));
        if loaded(&m) {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
    strip(&ctx, &mut m, &mut banks, &mut acct, &s, LOCAL_B);
    strip(&ctx, &mut m, &mut banks, &mut acct, &s, LOCAL_B);
    m.edit.ask_restore(1);
    // (no answer scripted: the job's thread ends without one)
    m.edit.confirm_restore(false);
    until(&ctx, &mut m, &mut banks, &mut acct, &s, LOCAL_B, &|m| !m.edit.acting());
    assert!(matches!(&m.edit.view, EditView::History(h) if h.restore.is_none()), "{:?}", m.edit.view);
    assert!(m.edit.results_of(LOCAL).iter().any(|r| r.1.contains("did not go through")));
}

#[test]
fn p28_another_account_is_not_told_the_first_ones_send_could_not_be_read() {
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
    s.on("Put", &put_path(), 201, unread_body());
    let n = s.paths().len();
    let release = s.hold();
    m.edit.send();
    m.edit.sync(&mut banks);
    wait_more(&s, n);
    strip_as(&ctx, &mut m, &mut banks, &mut acct, &pete, LOCAL);
    release.send(()).unwrap();
    for _ in 0..500 {
        m.edit.frame(now(), &pete);
        if !m.edit.acting() {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
    let t = strip_as(&ctx, &mut m, &mut banks, &mut acct, &pete, LOCAL);
    assert!(!t.contains("could not be read") && !t.contains("Your changes were sent"), "{t}");
    assert!(m.edit.results_of(LOCAL).is_empty());
}

#[test]
fn p29_a_merge_applied_ends_the_sent_but_not_read_state() {
    let (mut m, s, mut banks, ctx, mut acct) = send_unread_here();
    strip(&ctx, &mut m, &mut banks, &mut acct, &s, LOCAL);
    m.edit.close();
    assert!(m.edit.sent_unread(LOCAL) && !m.edit.results_of(LOCAL).is_empty());
    // Get latest: version 4 is the person's own change
    s.on("Post", &format!("/circles/{CID}/banks/{BID}/merge"), 200, json!({"bank_id": BID, "base_version": 3, "base_kept": true, "oldest_kept": 1, "head_version": 4, "head_version_id": hex(4), "head_at": NOW - 1000, "head_by": "Ms Frizzle", "between": [], "between_total": 1, "merge_token": "tok.en", "merge_token_expires_at": NOW + 1_800_000}));
    s.keep_raw("Get", &format!("/circles/{CID}/banks/{BID}"), served(4, &[(QA, "One (my fix)")], false));
    assert!(m.edit.open_merge(LOCAL, editor()));
    for _ in 0..3 {
        m.edit.frame(now(), &ready(&s));
        m.edit.sync(&mut banks);
    }
    assert!(matches!(&m.edit.view, EditView::Merge(v) if matches!(v.phase, MergePhase::Review(_))), "{}", phase(&m.edit));
    m.edit.apply_merge();
    for _ in 0..3 {
        m.edit.frame(now(), &ready(&s));
        m.edit.sync(&mut banks);
    }
    assert!(phase(&m.edit).starts_with("Applied"), "{}", phase(&m.edit));
    assert!(!m.edit.sent_unread(LOCAL));
    assert!(m.edit.results_of(LOCAL).is_empty(), "{:?}", m.edit.results_of(LOCAL));
}

#[test]
fn p30_get_latest_that_finds_nothing_newer_for_a_copy_that_can_not_send_ends_the_sent_state() {
    let (mut m, s, mut banks, ctx, mut acct) = send_unread_here();
    strip(&ctx, &mut m, &mut banks, &mut acct, &s, LOCAL);
    m.edit.close();
    assert!(m.edit.sent_unread(LOCAL));
    // the person can only view now: Get latest reads the newest version directly, and it is the copy's own
    s.keep_raw("Get", &format!("/circles/{CID}/banks/{BID}"), served(3, &[(QA, "One")], false));
    let viewer = Perm { role: super::circles_api::Role::Viewer, is_author: false, closed: false };
    assert!(m.edit.open_merge(LOCAL, viewer));
    for _ in 0..3 {
        m.edit.frame(now(), &ready(&s));
        m.edit.sync(&mut banks);
    }
    assert!(matches!(&m.edit.view, EditView::Merge(v) if matches!(v.phase, MergePhase::UpToDate | MergePhase::StaleUpToDate { .. })), "{}", phase(&m.edit));
    assert!(!m.edit.sent_unread(LOCAL));
    assert!(m.edit.results_of(LOCAL).is_empty());
}

#[test]
fn p32_another_circle_banks_settings_never_say_restoring() {
    let (mut m, _s, _banks, _ctx, _acct, release) = restore_held((200, rolled(4, 1)));
    m.edit.back();
    let a = text_of(1280.0, &mut |ui| super::circle_edit_ui::history_section(ui, &mut m, CID, BID, "Unit 2 quiz"));
    assert!(a.contains("Restoring version 1 in the circle"), "{a}");
    let b = text_of(1280.0, &mut |ui| super::circle_edit_ui::history_section(ui, &mut m, CID, &hex(77), "Plants"));
    assert!(!b.contains("Restoring"), "{b}");
    let _ = release.send(());
}

/// A result for A (a restore that failed while its History was left), then A's History opened again on A's page.
fn a_result_then_as_history() -> (CirclesModel, Arc<Script>, Vec<QuestionBank>, egui::Context, FakeAccount) {
    let (mut m, s, mut banks) = two_at_three();
    let ctx = ctx();
    let mut acct = FakeAccount { open: true, ..FakeAccount::default() };
    strip(&ctx, &mut m, &mut banks, &mut acct, &s, LOCAL);
    s.on("Get", &versions(), 200, page(3, &[3, 2, 1]));
    history_on_a(&mut m);
    strip(&ctx, &mut m, &mut banks, &mut acct, &s, LOCAL);
    m.edit.ask_restore(1);
    s.on("Post", &rollback_path(BID), 503, json!({"error": "unavailable"}));
    m.edit.confirm_restore(false);
    m.edit.back();
    strip(&ctx, &mut m, &mut banks, &mut acct, &s, LOCAL);
    assert_eq!(m.edit.results_of(LOCAL).len(), 1);
    (m, s, banks, ctx, acct)
}

#[test]
fn p38_dismiss_above_a_page_takes_the_result_away() {
    let (mut m, s, _banks, ctx, _acct) = a_result_then_as_history();
    s.on("Get", &versions(), 200, page(3, &[3, 2, 1]));
    history_on_a(&mut m);
    m.edit.frame(now(), &ready(&s));
    assert!(super::circles_phasec_tests::click_text(&ctx, 1280.0, "Dismiss", 0, &mut |ui| super::circle_edit_ui::draw_page(ui, &mut m)));
    assert!(m.edit.results_of(LOCAL).is_empty());
}

#[test]
fn p39_dismiss_on_bank_settings_takes_the_result_away() {
    let (mut m, _s, _banks, ctx, _acct) = a_result_then_as_history();
    assert!(super::circles_phasec_tests::click_text(&ctx, 1280.0, "Dismiss", 0, &mut |ui| super::circle_edit_ui::history_section(ui, &mut m, CID, BID, "Unit 2 quiz")));
    assert!(m.edit.results_of(LOCAL).is_empty());
}

#[test]
fn c73_i1_another_account_is_not_told_how_the_first_ones_send_went() {
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
    s.on("Put", &put_path(), 503, json!({"error": "unavailable"}));
    let n = s.paths().len();
    let release = s.hold();
    m.edit.send();
    m.edit.sync(&mut banks);
    wait_more(&s, n);
    strip_as(&ctx, &mut m, &mut banks, &mut acct, &pete, LOCAL);
    release.send(()).unwrap();
    for _ in 0..500 {
        m.edit.frame(now(), &pete);
        if !m.edit.acting() {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
    let t = strip_as(&ctx, &mut m, &mut banks, &mut acct, &pete, LOCAL);
    assert!(!t.contains("not sent"), "{t}");
    assert!(m.edit.results_of(LOCAL).is_empty());
}

#[test]
fn c74_i1_another_account_signing_in_after_an_unread_send_is_not_told_its_changes_were_sent() {
    let (mut m, s, mut banks, ctx, mut acct) = send_unread_here();
    strip(&ctx, &mut m, &mut banks, &mut acct, &s, LOCAL);
    m.edit.close();
    assert!(m.edit.sent_unread(LOCAL));
    m.edit.forget_account(true);
    assert!(!m.edit.sent_unread(LOCAL));
}

#[test]
fn c73_minor6_the_shared_pages_quiet_check_that_finds_a_newer_version_takes_away_used_as_it_is() {
    let (mut m, s, mut banks) = two_at_three();
    let ctx = ctx();
    let mut acct = FakeAccount { open: true, ..FakeAccount::default() };
    strip(&ctx, &mut m, &mut banks, &mut acct, &s, LOCAL);
    make_stale(&mut m, LOCAL);
    treat_in_step(&mut m, &s, &mut banks, LOCAL, BID, false);
    assert!(!m.edit.results_of(LOCAL).is_empty());
    use super::circles_api::{HeadEntry, HeadState};
    m.edit.apply_heads(&[HeadEntry { bank_id: BID.into(), state: HeadState::Current { version: 5, license: None, at: Some(NOW), by: None, hide_key: None } }]);
    assert!(m.edit.results_of(LOCAL).is_empty(), "{:?}", m.edit.results_of(LOCAL));
}
