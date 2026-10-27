//! Phase D1, round 8 (critics 77 and 78): each test written first against the code it pins, from the critics' probes.
//! A send with no clear answer that went through is matched, not doubled, when the latest is got, and its note says
//! what the check found; the commit is retried as the same request (its note kept); a Send page after an unread send
//! says its counts still include it, and a conflict with the person's own earlier send never blames someone else.

use std::sync::{Arc, Mutex};

use serde_json::{Value, json};

use super::bank_links_fix4_tests::*;
use super::bank_links_fix_tests::*;
use super::bank_links_tests::*;
use super::circle_edit::*;
use super::circle_links::LinkLog;
use super::circles::CirclesModel;
use super::circles_tests::*;
use super::merge::RowKind;
use super::publish::AccountSnapshot;
use crate::model::QuestionBank;

/// The circle's id for the question the teacher added and sent.
pub(super) const QN: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa9";
pub(super) const ADDED: &str = "Solve 5x = 20";

pub(super) fn put_path() -> String {
    format!("/circles/{CID}/banks/{BID}")
}

pub(super) fn said(m: &CirclesModel, local: &str) -> String {
    m.edit.results_of(local).into_iter().map(|r| r.1).collect::<Vec<_>>().join(" | ")
}

pub(super) fn olga(s: &Arc<Script>) -> AccountSnapshot {
    as_who(s, "olga")
}

pub(super) fn strip_as(ctx: &egui::Context, m: &mut CirclesModel, banks: &mut [QuestionBank], acct: &mut FakeAccount, snap: &AccountSnapshot, local: &str) -> String {
    let mut b = browse();
    frame_text(ctx, &mut |ui| {
        super::bank_links_ui::strip_with(ui, m, banks, &mut b, acct, snap, now(), NOW, local);
    })
}

/// Frames of `local`'s page as olga until `done`; the last frame's text.
pub(super) fn until_as(ctx: &egui::Context, m: &mut CirclesModel, banks: &mut [QuestionBank], acct: &mut FakeAccount, s: &Arc<Script>, local: &str, done: &dyn Fn(&CirclesModel) -> bool) -> String {
    let snap = olga(s);
    for _ in 0..500 {
        let t = strip_as(ctx, m, banks, acct, &snap, local);
        if done(m) {
            return t;
        }
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
    panic!("timed out: {:?}", m.edit.view);
}

pub(super) fn merge_start(base: u64, head: u64) -> Value {
    json!({"bank_id": BID, "base_version": base, "base_kept": true, "oldest_kept": 1, "head_version": head, "head_version_id": hex(head), "head_at": NOW - 1000, "head_by": "Olga Teacher",
        "between": [{"version": head, "version_id": hex(head), "by": "Olga Teacher", "at": NOW - 1000, "message": "Added a question", "added": 1, "changed": 0, "removed": 0, "question_count": 2, "license": "CC-BY-4.0", "kept": true, "withheld": false}],
        "between_total": head - base, "merge_token": "tok.en", "merge_token_expires_at": NOW + 1_800_000})
}

pub(super) type Rig = (CirclesModel, Arc<Script>, Vec<QuestionBank>, egui::Context, FakeAccount);

/// A (at version 3, olga signed in): olga adds "Solve 5x = 20" and sends it with a note; no clear answer comes
/// (`fail`, or `status`/`body`); the circle's newest version is `head` then (4: it went through). The Send page is then
/// left (Back), before the check the strip asks for at once has answered.
pub(super) fn unclear_send(fail: Option<super::api::ApiError>, status: u16, body: Value, head: u64) -> Rig {
    let (mut m, s, banks, ctx, acct) = unclear_send_with(fail, status, body, head);
    m.edit.close();
    (m, s, banks, ctx, acct)
}

/// The same, the Send page still open.
pub(super) fn unclear_send_open(fail: Option<super::api::ApiError>, head: u64) -> Rig {
    unclear_send_with(fail, 0, Value::Null, head)
}

pub(super) fn unclear_send_with(fail: Option<super::api::ApiError>, status: u16, body: Value, head: u64) -> Rig {
    let (mut m, s, mut banks) = two_at_three();
    s.keep("Get", &format!("/circles/{CID}"), 200, detail("owner", json!({})));
    let ctx = ctx();
    let mut acct = FakeAccount { open: true, ..FakeAccount::default() };
    let who = olga(&s);
    strip_as(&ctx, &mut m, &mut banks, &mut acct, &who, LOCAL);
    banks[0].questions.push(serde_json::from_value(qjson("new-local", ADDED)).unwrap());
    banks[0].modified += 5;
    assert!(m.edit.open_send(LOCAL, owner()));
    m.edit.host = Some(EditHost { local_id: LOCAL.into(), back: "< Back to Unit 2 quiz".into() });
    for _ in 0..3 {
        strip_as(&ctx, &mut m, &mut banks, &mut acct, &who, LOCAL);
    }
    m.edit.set_message("Added a question");
    m.edit.set_attest(true);
    match fail {
        Some(e) => {
            // (the send reads the bank's licence first: the failure is the send's own)
            s.on("Post", "/circles/heads", 200, heads(3));
            s.fail(e);
        }
        None => {
            s.on("Put", &put_path(), status, body);
        }
    }
    m.edit.send();
    m.edit.sync(&mut banks);
    assert!(s.paths().iter().any(|p| p.starts_with("Put ")), "the send went out: {:?}", s.paths());
    s.keep("Post", "/circles/heads", 200, heads(head));
    let page = strip_as(&ctx, &mut m, &mut banks, &mut acct, &who, LOCAL);
    assert!(page.contains("may have gone through"), "{page}");
    (m, s, banks, ctx, acct)
}

pub(super) fn timed_out() -> Option<super::api::ApiError> {
    Some(super::api::ApiError::Offline("Can't reach pool.test: the connection timed out.".into()))
}

/// (critic 78 I1) The check answered while the Send page was still open: what the page leaves on the strip when it is
/// closed is what the check found, never "is checked again".
#[test]
fn c78_i1_a_check_answered_while_the_send_page_is_open_is_what_the_strip_says_when_it_closes() {
    let (mut m, s, mut banks, ctx, mut acct) = unclear_send_open(timed_out(), 4);
    let who = olga(&s);
    for _ in 0..3 {
        strip_as(&ctx, &mut m, &mut banks, &mut acct, &who, LOCAL);
    }
    assert!(matches!(&m.edit.view, EditView::Send(_)), "still on the Send page");
    m.edit.close();
    let r = said(&m, LOCAL);
    assert!(!r.contains("is checked again") && r.contains("may be the one you sent"), "{r}");
}

/// The check the strip asked for at once answers: frames of A's page.
pub(super) fn checked(m: &mut CirclesModel, s: &Arc<Script>, banks: &mut [QuestionBank], ctx: &egui::Context, acct: &mut FakeAccount) -> String {
    let who = olga(s);
    strip_as(ctx, m, banks, acct, &who, LOCAL);
    strip_as(ctx, m, banks, acct, &who, LOCAL);
    strip_as(ctx, m, banks, acct, &who, LOCAL)
}

pub(super) fn review_rows(m: &CirclesModel) -> Option<(usize, usize, usize)> {
    match &m.edit.view {
        EditView::Merge(v) => match &v.phase {
            MergePhase::Review(r) => Some((r.plan.count(RowKind::NewInTheirs), r.plan.count(RowKind::Unchanged), r.plan.unlinked)),
            _ => None,
        },
        _ => None,
    }
}

// ---- c78 I1: a send that went through without a clear answer, then Get latest ------------------------------------------

#[test]
fn c78_i1_get_latest_after_an_unclear_send_that_went_through_matches_the_sent_question_not_adds_it_again() {
    for (fail, status, body) in [(timed_out(), 0, Value::Null), (None, 500, json!({"error": "server_error"})), (None, 201, json!({"circle_id": CID, "bank_id": BID, "version": "four"}))] {
        let (mut m, s, mut banks, ctx, mut acct) = unclear_send(fail, status, body, 4);
        // it went through: version 4 has olga's question
        checked(&mut m, &s, &mut banks, &ctx, &mut acct);
        s.on("Post", &format!("{}/merge", put_path()), 200, merge_start(3, 4));
        s.keep_raw("Get", &put_path(), served(4, &[(QA, "One"), (QN, ADDED)], false));
        assert!(m.edit.open_merge(LOCAL, owner()));
        m.edit.host = Some(EditHost { local_id: LOCAL.into(), back: "< Back to Unit 2 quiz".into() });
        let page = until_as(&ctx, &mut m, &mut banks, &mut acct, &s, LOCAL, &|m| review_rows(m).is_some());
        let (new, same, unlinked) = review_rows(&m).unwrap();
        assert_eq!((new, unlinked), (0, 0), "the question olga sent is matched, not new in the circle: {page}");
        assert_eq!(same, 2, "{page}");
        let page = strip_as(&ctx, &mut m, &mut banks, &mut acct, &olga(&s), LOCAL);
        assert!(!page.contains("New in the circle") && !page.contains("yours alone"), "{page}");
        assert!(page.contains("added the same question"), "{page}");
        m.edit.apply_merge();
        until_as(&ctx, &mut m, &mut banks, &mut acct, &s, LOCAL, &|m| matches!(&m.edit.view, EditView::Merge(v) if matches!(v.phase, MergePhase::Applied(_))));
        assert_eq!(banks[0].questions.iter().filter(|q| q.stem == ADDED).count(), 1, "the bank has it once");
        assert_eq!(banks[0].questions.len(), 2);
        assert_eq!(m.edit.with_link(LOCAL, |l| (l.base_version, l.qids.len())), Some((4, 2)));
        m.edit.cancel_merge();
        let t = strip_as(&ctx, &mut m, &mut banks, &mut acct, &olga(&s), LOCAL);
        assert!(!t.contains("unsent") && t.contains("up to date"), "{t}");
        assert!(!t.contains("is checked again") && !said(&m, LOCAL).contains("is checked again"), "{t}");
    }
}

#[test]
fn c78_i1_once_the_check_finds_a_newer_version_the_note_says_so_and_no_longer_says_it_is_checked_again() {
    let (mut m, s, mut banks, ctx, mut acct) = unclear_send(timed_out(), 0, Value::Null, 4);
    assert!(said(&m, LOCAL).contains("is checked again"), "{}", said(&m, LOCAL));
    let t = checked(&mut m, &s, &mut banks, &ctx, &mut acct);
    let r = said(&m, LOCAL);
    assert!(!r.contains("is checked again"), "{r}");
    assert!(r.contains("version 4") && r.contains("may be the one you sent") && r.contains("Get latest"), "{r}");
    assert!(t.contains(&r.split(" | ").next().unwrap().to_string()), "on the strip: {t}");
}

#[test]
fn c78_i1_once_the_check_finds_no_newer_version_the_note_says_the_send_has_not_arrived() {
    let (mut m, s, mut banks, ctx, mut acct) = unclear_send(timed_out(), 0, Value::Null, 3);
    checked(&mut m, &s, &mut banks, &ctx, &mut acct);
    let r = said(&m, LOCAL);
    assert!(!r.contains("is checked again") && !r.contains("may have gone through"), "{r}");
    // (critic 81 minor 5: not "did not go through")
    assert!(r.contains("may not have arrived yet") && r.contains("version 3") && r.contains("Send my changes"), "{r}");
}

#[test]
fn c78_i1_send_my_changes_after_an_unclear_send_goes_as_the_same_request_with_its_note() {
    let (mut m, s, mut banks, ctx, mut acct) = unclear_send(timed_out(), 0, Value::Null, 4);
    checked(&mut m, &s, &mut banks, &ctx, &mut acct);
    assert!(m.edit.open_send(LOCAL, owner()));
    m.edit.host = Some(EditHost { local_id: LOCAL.into(), back: "< Back to Unit 2 quiz".into() });
    let who = olga(&s);
    for _ in 0..3 {
        strip_as(&ctx, &mut m, &mut banks, &mut acct, &who, LOCAL);
    }
    assert!(matches!(&m.edit.view, EditView::Send(v) if v.message == "Added a question"), "the note is kept for the retry");
    m.edit.set_attest(true);
    assert!(m.edit.send_blocker().is_none(), "{:?}", m.edit.send_blocker());
    s.on("Put", &put_path(), 201, json!({"circle_id": CID, "bank_id": BID, "version": 4, "version_id": hex(4), "qids": [QA, QN], "added": 1, "changed": 0, "removed": 0, "replayed": true}));
    m.edit.send();
    m.edit.sync(&mut banks);
    strip_as(&ctx, &mut m, &mut banks, &mut acct, &who, LOCAL);
    let puts: Vec<Value> = s.seen().into_iter().filter(|x| x.method == super::transport::Method::Put).map(|x| serde_json::from_str(String::from_utf8(x.body).unwrap().split_once('\n').unwrap().0).unwrap()).collect();
    assert_eq!(puts.len(), 2, "{:?}", s.paths());
    assert_eq!(puts[0]["commit_id"], puts[1]["commit_id"], "the circle answers the first send again: nothing is sent twice");
    assert_eq!(m.edit.with_link(LOCAL, |l| (l.base_version, l.qids.len())), Some((4, 2)));
}

// ---- c78 I2: the Send page and a conflict after a send whose answer could not be read ------------------------------------

/// A's send as olga answered with no version the app can read (it went through), then one more edit.
pub(super) fn unread_then_edit() -> Rig {
    let (mut m, s, mut banks) = two_at_three();
    s.keep("Get", &format!("/circles/{CID}"), 200, detail("owner", json!({})));
    let ctx = ctx();
    let mut acct = FakeAccount { open: true, ..FakeAccount::default() };
    let who = olga(&s);
    strip_as(&ctx, &mut m, &mut banks, &mut acct, &who, LOCAL);
    banks[0].questions[0].stem = "One (my fix)".into();
    banks[0].modified += 5;
    assert!(m.edit.open_send(LOCAL, editor()));
    m.edit.host = Some(EditHost { local_id: LOCAL.into(), back: "< Back to Unit 2 quiz".into() });
    for _ in 0..3 {
        strip_as(&ctx, &mut m, &mut banks, &mut acct, &who, LOCAL);
    }
    m.edit.set_attest(true);
    s.on("Put", &put_path(), 201, json!({"circle_id": CID, "bank_id": BID, "version": 0, "qids": [], "added": 0, "changed": 0, "removed": 0}));
    m.edit.send();
    m.edit.sync(&mut banks);
    strip_as(&ctx, &mut m, &mut banks, &mut acct, &who, LOCAL);
    m.edit.close();
    assert!(m.edit.sent_unread(LOCAL));
    banks[0].questions[0].stem = "One (my second fix)".into();
    banks[0].modified += 5;
    (m, s, banks, ctx, acct)
}

#[test]
fn c78_i2_the_send_page_after_an_unread_send_says_its_counts_still_include_it_and_offers_get_latest() {
    let (mut m, s, mut banks, ctx, mut acct) = unread_then_edit();
    let who = olga(&s);
    let t = strip_as(&ctx, &mut m, &mut banks, &mut acct, &who, LOCAL);
    assert!(t.contains("Your earlier changes were sent"), "{t}");
    assert!(m.edit.open_send(LOCAL, editor()));
    m.edit.host = Some(EditHost { local_id: LOCAL.into(), back: "< Back to Unit 2 quiz".into() });
    let mut page = String::new();
    for _ in 0..3 {
        page = strip_as(&ctx, &mut m, &mut banks, &mut acct, &who, LOCAL);
    }
    assert!(page.contains("Since your copy last matched the circle"), "{page}");
    assert!(page.contains("Your earlier send went through") && page.contains("these counts still include"), "{page}");
    assert!(page.contains("Get latest"), "{page}");
}

#[test]
fn c78_i2_a_conflict_with_the_persons_own_earlier_send_never_says_someone_else_sent_it() {
    let (mut m, s, mut banks, ctx, mut acct) = unread_then_edit();
    let who = olga(&s);
    assert!(m.edit.open_send(LOCAL, editor()));
    m.edit.host = Some(EditHost { local_id: LOCAL.into(), back: "< Back to Unit 2 quiz".into() });
    for _ in 0..3 {
        strip_as(&ctx, &mut m, &mut banks, &mut acct, &who, LOCAL);
    }
    m.edit.set_attest(true);
    s.on("Put", &put_path(), 409, json!({"error": "conflict", "head_version": 4, "head_at": NOW - 60_000, "head_by": "Olga Teacher"}));
    m.edit.send();
    m.edit.sync(&mut banks);
    let page = strip_as(&ctx, &mut m, &mut banks, &mut acct, &who, LOCAL);
    assert!(matches!(&m.edit.view, EditView::Send(v) if matches!(v.stage, SendStage::Conflict(_))), "{page}");
    assert!(!page.contains("Someone else") && !page.contains("someone") && !page.contains("their changes"), "{page}");
    assert!(page.contains("Your earlier send went through"), "{page}");
    m.edit.close();
    let r = said(&m, LOCAL);
    assert!(!r.contains("someone") && !r.contains("Someone"), "{r}");
    assert!(r.contains("Your changes were not sent") && r.contains("earlier send"), "{r}");
}

#[test]
fn c78_i2_a_conflict_after_an_unclear_send_says_it_may_be_the_persons_own() {
    let (mut m, s, mut banks, ctx, mut acct) = unclear_send(timed_out(), 0, Value::Null, 4);
    checked(&mut m, &s, &mut banks, &ctx, &mut acct);
    // a change since: the retry is a new send, and the circle has version 4
    banks[0].questions[0].stem = "One (my fix)".into();
    banks[0].modified += 5;
    assert!(m.edit.open_send(LOCAL, owner()));
    m.edit.host = Some(EditHost { local_id: LOCAL.into(), back: "< Back to Unit 2 quiz".into() });
    let who = olga(&s);
    for _ in 0..3 {
        strip_as(&ctx, &mut m, &mut banks, &mut acct, &who, LOCAL);
    }
    m.edit.set_attest(true);
    s.on("Put", &put_path(), 409, json!({"error": "conflict", "head_version": 4, "head_at": NOW - 60_000, "head_by": "Olga Teacher"}));
    m.edit.send();
    m.edit.sync(&mut banks);
    let page = strip_as(&ctx, &mut m, &mut banks, &mut acct, &who, LOCAL);
    assert!(!page.contains("Someone else") && !page.contains("their changes"), "{page}");
    assert!(page.contains("may be your earlier send"), "{page}");
    m.edit.close();
    let r = said(&m, LOCAL);
    assert!(!r.contains("someone") && r.contains("may be your earlier send"), "{r}");
}


// ---- c78 I3 (for the owner): which invite code is meant ---------------------------------------------------------------

/// The account's invite code (from an administrator) as a circle screen says it: never as if the circle's own code
/// were wrong, and still no "pool".
pub(super) const CIRCLE_INVITE_REQUIRED: &str = "Your verSHion account needs an invite code from an administrator before it can use circles. The code it needs is not a circle's code. Open the Account window, find \"Invite code\" and enter it. Everything else in verSHion keeps working.";

#[test]
fn c78_i3_a_circle_screen_says_the_account_needs_its_own_invite_code_not_that_the_circles_code_is_wrong() {
    use super::circles_text::circle_words;
    use super::pool::INVITE_REQUIRED_TEXT;
    assert_eq!(circle_words(INVITE_REQUIRED_TEXT), CIRCLE_INVITE_REQUIRED);
    assert!(INVITE_REQUIRED_TEXT.starts_with("The shared pool"), "the public pool's screens keep their words");
    // the join page: the code found is the circle's, and the refusal says it is not that code that is missing
    let s = Script::new();
    s.say(200, json!({"circles": []}));
    let mut m = model();
    m.open_list();
    m.frame(now(), &ready(&s));
    m.start_join();
    m.join.text = "C-ABCDEFGHJKMNPQRSTVWXYZ01".into();
    s.on("Post", "/circles/join/preview", 403, json!({"error": "invite_required"}));
    m.look_up_code();
    m.frame(now(), &ready(&s));
    let super::circles::JoinStage::Failed(p) = &m.join.stage else { panic!("{:?}", m.join.stage) };
    assert_eq!(p.message, CIRCLE_INVITE_REQUIRED);
    assert!(!p.message.contains("The circle needs an invite code"), "{}", p.message);
}

#[test]
fn c78_i3_the_email_invite_help_says_the_account_code_and_the_circles_code_apart() {
    assert_eq!(
        super::circles_text::EMAIL_NEEDS_ACCOUNT,
        "To join, they need a verSHion account that an administrator has approved, with its invite code entered. Until then the circle's code in the mail can't be used."
    );
}

#[test]
fn c78_i3_circles_not_set_up_on_the_server_says_circles() {
    let detail = super::api::ErrorDetail { status: 503, code: "pool_not_configured".into(), field: None, reason: None, ack_allowed: None, author_only: false, retry_after: None, json: json!({"error": "pool_not_configured"}) };
    let kind = super::pool::RefusalKind::classify(&detail);
    let t = super::circles_text::circle_words(&super::pool::PoolError::Refused(Box::new(super::pool::Refusal { kind, detail })).message());
    assert_eq!(t, "Circles are not available on this server yet. Nothing you can change; everything else works.");
}

// ---- c77 I2: a restore or trim with no clear answer -----------------------------------------------------------------

pub(super) fn rollback_path() -> String {
    format!("/circles/{CID}/banks/{BID}/rollback")
}

pub(super) fn no_clear_answers() -> Vec<(&'static str, Option<super::api::ApiError>, u16, Value)> {
    vec![
        ("timeout", timed_out(), 0, Value::Null),
        ("500", None, 500, json!({"error": "server_error"})),
        ("502", None, 502, json!({"error": "bad_gateway"})),
        ("unreadable", None, 200, json!({"circle_id": CID, "bank_id": BID, "version": "four"})),
    ]
}

/// A's History on A's page (olga), a restore of version 1 answered as `fail` or `status`/`body`; `leave`: the History is
/// left (Back) before the answer is read.
pub(super) fn restore_answered(fail: Option<super::api::ApiError>, status: u16, body: Value, leave: bool) -> Rig {
    let (mut m, s, mut banks) = two_at_three();
    s.keep("Get", &format!("/circles/{CID}"), 200, detail("owner", json!({})));
    let ctx = ctx();
    let mut acct = FakeAccount { open: true, ..FakeAccount::default() };
    let who = olga(&s);
    strip_as(&ctx, &mut m, &mut banks, &mut acct, &who, LOCAL);
    s.on("Get", &versions(), 200, page(3, &[3, 2, 1]));
    history_on_a(&mut m);
    strip_as(&ctx, &mut m, &mut banks, &mut acct, &who, LOCAL);
    strip_as(&ctx, &mut m, &mut banks, &mut acct, &who, LOCAL);
    assert!(loaded(&m));
    m.edit.ask_restore(1);
    match fail {
        Some(e) => {
            s.fail(e);
        }
        None => {
            s.on("Post", &rollback_path(), status, body);
        }
    }
    m.edit.confirm_restore(false);
    if leave {
        m.edit.back();
    }
    (m, s, banks, ctx, acct)
}

#[test]
fn c77_i2_a_restore_with_no_clear_answer_may_have_gone_through_is_unsure_and_checked_at_once() {
    for (name, fail, status, body) in no_clear_answers() {
        let (mut m, s, mut banks, ctx, mut acct) = restore_answered(fail, status, body, true);
        let before = heads_count(&s);
        // the answer is read on A's page, the History left
        let t = strip_as(&ctx, &mut m, &mut banks, &mut acct, &olga(&s), LOCAL);
        let r = said(&m, LOCAL);
        assert!(r.contains("may have gone through") && r.contains("version 1"), "{name}: {r}");
        assert!(!r.contains("did not go through") && !r.contains("Nothing changed") && !r.contains("Nothing was changed"), "{name}: {r}");
        assert!(m.edit.unsure(LOCAL), "{name}: the newest version is not known until the check answers");
        assert!(!t.contains("up to date") && !t.contains("is the latest"), "{name}: {t}");
        assert_eq!(heads_count(&s), before + 1, "{name}: A is checked at once");
    }
}

#[test]
fn c77_i2_the_history_itself_says_a_restore_with_no_clear_answer_may_have_gone_through_and_try_again_replays_it() {
    for (name, fail, status, body) in no_clear_answers() {
        let (mut m, s, mut banks, ctx, mut acct) = restore_answered(fail, status, body, false);
        let page = strip_as(&ctx, &mut m, &mut banks, &mut acct, &olga(&s), LOCAL);
        assert!(page.contains("may have gone through") && !page.contains("did not go through") && !page.contains("Nothing changed"), "{name}: {page}");
        assert!(page.contains("Try again"), "{name}: {page}");
        assert!(m.edit.unsure(LOCAL), "{name}");
        // Try again goes as the same restore (the circle answers the first one again)
        s.on("Post", &rollback_path(), 200, json!({"circle_id": CID, "bank_id": BID, "version": 4, "version_id": hex(4), "qids": [QA], "added": 0, "changed": 1, "removed": 0, "rollback_of": 1, "replayed": true}));
        s.on("Get", &versions(), 200, page_of(4));
        m.edit.retry_act();
        strip_as(&ctx, &mut m, &mut banks, &mut acct, &olga(&s), LOCAL);
        let ids: Vec<Value> = s.seen().into_iter().filter(|x| x.path == rollback_path()).map(|x| x.json()["commit_id"].clone()).collect();
        assert_eq!(ids.len(), 2, "{name}");
        assert_eq!(ids[0], ids[1], "{name}: the same restore, never a second one");
        // and the History, left now, does not turn it into "did not go through"
        m.edit.back();
        assert!(!said(&m, LOCAL).contains("did not go through"), "{name}: {}", said(&m, LOCAL));
    }
}

pub(super) fn page_of(head: u64) -> Value {
    page(head, &(1..=head).rev().collect::<Vec<_>>())
}

#[test]
fn c77_i2_a_trim_with_no_clear_answer_is_never_said_not_to_have_gone_through() {
    for (name, fail, status, body) in no_clear_answers() {
        let (mut m, s, mut banks) = two_at_three();
        s.keep("Get", &format!("/circles/{CID}"), 200, detail("owner", json!({})));
        let ctx = ctx();
        let mut acct = FakeAccount { open: true, ..FakeAccount::default() };
        let who = olga(&s);
        strip_as(&ctx, &mut m, &mut banks, &mut acct, &who, LOCAL);
        s.keep("Get", &versions(), 200, page(3, &[3, 2, 1]));
        history_on_a(&mut m);
        strip_as(&ctx, &mut m, &mut banks, &mut acct, &who, LOCAL);
        strip_as(&ctx, &mut m, &mut banks, &mut acct, &who, LOCAL);
        m.edit.open_trim();
        m.edit.ask_trim();
        match fail {
            Some(e) => {
                s.fail(e);
            }
            // (a trim's answer is read leniently: only one that is not JSON at all can not be read)
            None if name == "unreadable" => {
                s.on_raw("Post", &format!("/circles/{CID}/banks/{BID}/trim"), b"<html>gateway</html>".to_vec());
            }
            None => {
                s.on("Post", &format!("/circles/{CID}/banks/{BID}/trim"), status, body);
            }
        }
        m.edit.confirm_trim();
        let page = strip_as(&ctx, &mut m, &mut banks, &mut acct, &who, LOCAL);
        assert!(page.contains("may have gone through") && !page.contains("did not go through"), "{name}: {page}");
        m.edit.back();
        let r = said(&m, LOCAL);
        assert!(r.contains("may have gone through") && !r.contains("did not go through"), "{name}: {r}");
    }
}

// ---- c77 minors 3 and 4: "sent, but the answer could not be read" and the account that sent it -------------------------

pub(super) fn restarted(log: Arc<Mutex<Vec<super::circle_links::CircleLink>>>) -> CirclesModel {
    let mut m = model();
    m.edit = super::circle_edit::CircleEditModel::new(super::model::Runner::Inline, Arc::new(Mem::default()), Arc::new(std::collections::HashMap::<String, Vec<u8>>::new()), Arc::new(NoDiskRestore), LinkLog::Memory(log)).with_retry(super::pool::RetryPolicy::none());
    m.edit.set_clock(NOW);
    m
}

pub(super) fn badge(m: &mut CirclesModel, bank: &QuestionBank) -> String {
    let CirclesModel { links, edit, .. } = m;
    links.badge(edit, bank).and_then(|b| b.note).unwrap_or_default()
}

/// (critic 77 minor 3, probe U2) olga's send whose answer could not be read is kept in the links file; the app restarts
/// and olga is signed in. The Banks list's badge, before A's page is ever drawn, does not count those changes as unsent.
#[test]
fn c77_m3_u2_after_a_restart_the_banks_list_badge_keeps_a_sent_but_unread_change_as_sent() {
    let log = Arc::new(Mutex::new(Vec::new()));
    let (mut m0, s, mut banks) = two_at_three();
    s.keep("Get", &format!("/circles/{CID}"), 200, detail("owner", json!({})));
    let mut links = m0.edit.links().to_vec();
    banks[0].questions[0].stem = "One (my fix)".into();
    banks[0].modified += 5;
    links[0].sent_unread = Some(super::circle_links::SentUnread { by: super::bank_links::account_tag(Some("https://pool.test\nolga@school.org")).unwrap(), modified: banks[0].modified });
    *log.lock().unwrap() = links;
    let mut m = restarted(log);
    let bank = banks[0].clone();
    // the Banks list's frames, olga signed in (the Account window looked her up quietly)
    m.tick_background(now(), &olga(&s));
    let b = badge(&mut m, &bank);
    assert!(!b.contains("unsent"), "{b}");
    // and A's page, drawn later, says the same
    let ctx = ctx();
    let mut acct = FakeAccount { open: true, ..FakeAccount::default() };
    strip_as(&ctx, &mut m, &mut banks, &mut acct, &olga(&s), LOCAL);
    let t = strip_as(&ctx, &mut m, &mut banks, &mut acct, &olga(&s), LOCAL);
    assert!(t.contains("Your changes were sent") && !t.contains("unsent"), "{t}");
    assert_eq!(badge(&mut m, &bank), b, "the badge did not change once the page was drawn");
    // another account is never told it
    m.tick_background(now(), &as_who(&s, "pete"));
    assert!(!m.edit.sent_unread(LOCAL));
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

/// olga's send from A (owner), held at the server; pete signs in on this computer before the answer `reply` comes.
fn olgas_send_then_pete(status: u16, reply: Value) -> Rig {
    let (mut m, s, mut banks) = two_at_three();
    background(&mut m);
    let ctx = ctx();
    let mut acct = FakeAccount { open: true, ..FakeAccount::default() };
    let (who, pete) = (olga(&s), as_who(&s, "pete"));
    s.keep("Get", &format!("/circles/{CID}"), 200, detail("owner", json!({})));
    strip_as(&ctx, &mut m, &mut banks, &mut acct, &who, LOCAL);
    until_as(&ctx, &mut m, &mut banks, &mut acct, &s, LOCAL, &|m| m.links.check_of(LOCAL) != super::bank_links::Check::Checking);
    banks[0].questions[0].stem = "One (my fix)".into();
    banks[0].modified += 5;
    strip_as(&ctx, &mut m, &mut banks, &mut acct, &who, LOCAL);
    assert!(m.edit.open_send(LOCAL, owner()));
    m.edit.host = Some(EditHost { local_id: LOCAL.into(), back: "< Back to Unit 2 quiz".into() });
    until_as(&ctx, &mut m, &mut banks, &mut acct, &s, LOCAL, &|m| matches!(&m.edit.view, EditView::Send(v) if matches!(v.licence, LicenceState::Known(_))));
    m.edit.set_attest(true);
    s.on("Put", &put_path(), status, reply);
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

/// (critic 77 minor 4, probe U3) olga's send answered with no readable version after pete signed in: it went, and is
/// kept for olga (never told to pete). olga, back, is not told her changes are unsent (Send would conflict with her own
/// version).
#[test]
fn c77_m4_u3_an_unread_send_answered_after_another_account_signed_in_is_kept_for_its_sender() {
    let (mut m, s, mut banks, ctx, mut acct) = olgas_send_then_pete(201, json!({"circle_id": CID, "bank_id": BID, "version": 0, "qids": [], "added": 0, "changed": 0, "removed": 0}));
    let pete = as_who(&s, "pete");
    strip_as(&ctx, &mut m, &mut banks, &mut acct, &pete, LOCAL);
    assert!(!m.edit.sent_unread(LOCAL), "not told to pete");
    let who = olga(&s);
    strip_as(&ctx, &mut m, &mut banks, &mut acct, &who, LOCAL);
    let t = strip_as(&ctx, &mut m, &mut banks, &mut acct, &who, LOCAL);
    assert!(m.edit.sent_unread(LOCAL), "kept for olga: {t}");
    assert!(!t.contains("unsent") && t.contains("Your changes were sent"), "{t}");
}

/// (critic 78 I1) What is matched: only a question of the bank not linked yet, the same in everything, its picture too.
/// A question the circle added that is the same as one already linked here is still new (the circle has it twice).
#[test]
fn c78_i1_only_an_unlinked_question_the_same_in_everything_and_picture_is_matched() {
    use super::merge::{build_plan, match_new_to_unlinked};
    const QC: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa3";
    const QD: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa4";
    let (mut bank, link) = linked(&[(QA, "One")]);
    let q = |id: &str, stem: &str| -> crate::model::Question { serde_json::from_value(qjson(id, stem)).unwrap() };
    bank.questions.push(q("mine-two", "Two"));
    let mut three = q("mine-three", "Three");
    three.image = Some(crate::model::ImageRef { file: format!("media/{}.webp", "b".repeat(64)), width: 2, height: 2 });
    bank.questions.push(three);
    bank.questions.push(q("mine-four", "Four, my way"));
    let theirs = super::circle_links::circle_theirs(&payload(&[(QA, "One"), (QB, "Two"), (QC, "Three"), (QD, "Four"), (QN, "One")]), NOW);
    let mut plan = build_plan(BID, &link.base, &bank, &theirs, 1, 2, None, String::new());
    assert_eq!((plan.count(RowKind::NewInTheirs), plan.unlinked), (4, 3));
    match_new_to_unlinked(&mut plan, &bank, &theirs, &same_picture, "same");
    let new: Vec<String> = plan.rows.iter().filter(|r| r.kind == RowKind::NewInTheirs).map(|r| r.theirs_qid.clone()).collect();
    assert_eq!(new, vec![QC.to_string(), QD.to_string(), QN.to_string()], "Three (another picture), Four (other words) and the second One stay new");
    let matched: Vec<(Option<String>, String)> = plan.rows.iter().filter(|r| r.note.as_deref() == Some("same")).map(|r| (r.local_id.clone(), r.theirs_qid.clone())).collect();
    assert_eq!(matched, vec![(Some("mine-two".to_string()), QB.to_string())]);
    assert_eq!((plan.unlinked, plan.matched), (2, 1));
    // the picture rule itself
    let t = |sha: Option<&str>| super::merge::Theirs { index: 0, qid: QC.into(), fp: String::new(), sha: sha.map(str::to_string), q: q("x", "Three") };
    let with = |file: &str| {
        let mut x = q("y", "Three");
        x.image = Some(crate::model::ImageRef { file: file.into(), width: 2, height: 2 });
        x
    };
    let b = "b".repeat(64);
    assert!(same_picture(&q("y", "Three"), &t(None)));
    assert!(same_picture(&with(&format!("media/{b}.webp")), &t(Some(&b))));
    assert!(!same_picture(&with(&format!("media/{b}.webp")), &t(Some(&"c".repeat(64)))));
    assert!(!same_picture(&with(&format!("media/{b}.webp")), &t(None)));
    assert!(!same_picture(&q("y", "Three"), &t(Some(&b))));
}

/// (critic 77 I2, as critic 78 I1 for sends) Once the check asked for at once answers, the restore's note says what it
/// found: never "may have gone through" beside "up to date", whether the History was left before or after.
#[test]
fn c77_i2_once_the_check_answers_the_restore_note_says_what_it_found() {
    for (head, leave_first) in [(3, true), (4, true), (3, false), (4, false)] {
        let (mut m, s, mut banks, ctx, mut acct) = restore_answered(timed_out(), 0, Value::Null, leave_first);
        s.keep("Post", "/circles/heads", 200, heads(head));
        let who = olga(&s);
        let mut t = String::new();
        for _ in 0..3 {
            t = strip_as(&ctx, &mut m, &mut banks, &mut acct, &who, LOCAL);
        }
        if !leave_first {
            m.edit.back();
            t = strip_as(&ctx, &mut m, &mut banks, &mut acct, &who, LOCAL);
        }
        let r = said(&m, LOCAL);
        assert!(!r.contains("may have gone through"), "{head} {leave_first}: {r} / {t}");
        if head == 3 {
            // (critic 81 minor 5: not "did not go through")
            assert!(r.contains("may not have arrived yet") && r.contains("still has version 3") && t.contains("up to date"), "{r} / {t}");
        } else {
            assert!(r.contains("version 4") && r.contains("may be the restore of version 1") && !t.contains("up to date"), "{r} / {t}");
        }
    }
}

/// A send from A's page as `who` (owner) answered 409 for version `head` sent by Mr Keesler; the page's text.
pub(super) fn conflict_as(m: &mut CirclesModel, s: &Arc<Script>, banks: &mut [QuestionBank], ctx: &egui::Context, acct: &mut FakeAccount, who: &AccountSnapshot, head: u64) -> String {
    banks[0].questions[0].stem = format!("One (fix {head})");
    banks[0].modified += 5;
    assert!(m.edit.open_send(LOCAL, owner()));
    m.edit.host = Some(EditHost { local_id: LOCAL.into(), back: "< Back to Unit 2 quiz".into() });
    for _ in 0..3 {
        strip_as(ctx, m, banks, acct, who, LOCAL);
    }
    m.edit.set_attest(true);
    s.on("Put", &put_path(), 409, json!({"error": "conflict", "head_version": head, "head_at": NOW - 60_000, "head_by": "Mr Keesler"}));
    m.edit.send();
    m.edit.sync(banks);
    let page = strip_as(ctx, m, banks, acct, who, LOCAL);
    assert!(matches!(&m.edit.view, EditView::Send(v) if matches!(v.stage, SendStage::Conflict(_))), "{page}");
    page
}

/// (critic 78 I2) "It may be your earlier send" lasts only while it can be: not after the copy caught up with the circle,
/// not after a later send's answer was read, and never for another account.
#[test]
fn c78_i2_may_be_your_earlier_send_ends_when_the_copy_catches_up() {
    let (mut m, s, mut banks, ctx, mut acct) = unclear_send(timed_out(), 0, Value::Null, 4);
    checked(&mut m, &s, &mut banks, &ctx, &mut acct);
    s.on("Post", &format!("{}/merge", put_path()), 200, merge_start(3, 4));
    s.keep_raw("Get", &put_path(), served(4, &[(QA, "One"), (QN, ADDED)], false));
    assert!(m.edit.open_merge(LOCAL, owner()));
    m.edit.host = Some(EditHost { local_id: LOCAL.into(), back: "< Back to Unit 2 quiz".into() });
    until_as(&ctx, &mut m, &mut banks, &mut acct, &s, LOCAL, &|m| review_rows(m).is_some());
    m.edit.apply_merge();
    until_as(&ctx, &mut m, &mut banks, &mut acct, &s, LOCAL, &|m| matches!(&m.edit.view, EditView::Merge(v) if matches!(v.phase, MergePhase::Applied(_))));
    m.edit.cancel_merge();
    let page = conflict_as(&mut m, &s, &mut banks, &ctx, &mut acct, &olga(&s), 5);
    assert!(page.contains("Someone else sent") && !page.contains("earlier send"), "{page}");
}

#[test]
fn c78_i2_may_be_your_earlier_send_ends_when_a_later_send_is_read() {
    let (mut m, s, mut banks, ctx, mut acct) = unclear_send(timed_out(), 0, Value::Null, 4);
    checked(&mut m, &s, &mut banks, &ctx, &mut acct);
    // Send my changes again: the circle answers the first send again (read this time)
    assert!(m.edit.open_send(LOCAL, owner()));
    m.edit.host = Some(EditHost { local_id: LOCAL.into(), back: "< Back to Unit 2 quiz".into() });
    let who = olga(&s);
    for _ in 0..3 {
        strip_as(&ctx, &mut m, &mut banks, &mut acct, &who, LOCAL);
    }
    m.edit.set_attest(true);
    s.on("Put", &put_path(), 201, json!({"circle_id": CID, "bank_id": BID, "version": 4, "version_id": hex(4), "qids": [QA, QN], "added": 1, "changed": 0, "removed": 0, "replayed": true}));
    m.edit.send();
    m.edit.sync(&mut banks);
    strip_as(&ctx, &mut m, &mut banks, &mut acct, &who, LOCAL);
    m.edit.close();
    let page = conflict_as(&mut m, &s, &mut banks, &ctx, &mut acct, &who, 5);
    assert!(page.contains("Someone else sent") && !page.contains("earlier send"), "{page}");
}

#[test]
fn c78_i2_another_account_is_never_told_it_may_be_its_earlier_send() {
    let (mut m, s, mut banks, ctx, mut acct) = unclear_send(timed_out(), 0, Value::Null, 4);
    let pete = as_who(&s, "pete");
    for _ in 0..3 {
        strip_as(&ctx, &mut m, &mut banks, &mut acct, &pete, LOCAL);
    }
    let page = conflict_as(&mut m, &s, &mut banks, &ctx, &mut acct, &pete, 5);
    assert!(page.contains("Someone else sent") && !page.contains("earlier send"), "{page}");
}
