//! The invite-code section against a scripted session: redeeming, every refusal in words, the
//! rule that the code is kept nowhere, `invite_required` from every pool window, and smoke frames.

use std::collections::VecDeque;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use serde_json::{Value, json};
use web_time::Instant;

use super::api::{ApiError, Status};
use super::invite::*;
use super::model::Runner;
use super::pool::*;
use super::publish::{AccountSnapshot, SharedSession};
use super::transport::{LocalFuture, Method, Request, Response};

#[derive(Default)]
struct Script {
    replies: Mutex<VecDeque<Result<Response, ApiError>>>,
    seen: Mutex<Vec<(Method, String, String)>>,
}

impl Script {
    fn new() -> Arc<Script> {
        Arc::new(Script::default())
    }
    fn say(&self, status: u16, body: Value) -> &Self {
        self.replies.lock().unwrap().push_back(Ok(Response::new(status, &body.to_string())));
        self
    }
    fn say_after(&self, status: u16, secs: u64, body: Value) -> &Self {
        let mut r = Response::new(status, &body.to_string());
        r.retry_after = Some(secs);
        self.replies.lock().unwrap().push_back(Ok(r));
        self
    }
    fn fail(&self, e: ApiError) -> &Self {
        self.replies.lock().unwrap().push_back(Err(e));
        self
    }
    fn seen(&self) -> Vec<(Method, String, String)> {
        self.seen.lock().unwrap().clone()
    }
}

impl PoolSession for Script {
    fn base(&self) -> String {
        "https://pool.test".into()
    }
    fn send<'a>(&'a self, build: &'a dyn Fn() -> Request) -> LocalFuture<'a, Result<Response, ApiError>> {
        Box::pin(async move {
            let r = build();
            let body = r.body.as_ref().map(|b| b.expose().to_string()).unwrap_or_default();
            self.seen.lock().unwrap().push((r.method, r.url.trim_start_matches("https://pool.test").to_string(), body));
            self.replies.lock().unwrap().pop_front().unwrap_or_else(|| panic!("no scripted reply for {:?}", r.url))
        })
    }
}

const CODE: &str = "ABCD-EFGH-JKMN-PQRS-TVWX-YZ01";

fn session(s: &Arc<Script>) -> Option<SharedSession> {
    Some(s.clone())
}

fn model() -> InviteModel {
    InviteModel::new(Runner::Inline).with_retry(RetryPolicy::none())
}

fn typed(m: &mut InviteModel, code: &str) {
    m.code = code.to_string();
}

fn problem(m: &InviteModel) -> String {
    match &m.state {
        InviteState::Failed(p) => p.message.clone(),
        other => panic!("{other:?}"),
    }
}

#[test]
fn nothing_is_sent_until_the_button_is_pressed_with_a_code() {
    let s = Script::new();
    let mut m = model();
    m.frame(Instant::now());
    assert!(!m.can_redeem());
    m.redeem(session(&s));
    typed(&mut m, "   ");
    assert!(!m.can_redeem());
    m.redeem(session(&s));
    assert!(s.seen().is_empty());
    assert_eq!(m.state, InviteState::Unknown);
}

#[test]
fn a_good_code_is_sent_once_and_then_forgotten() {
    let s = Script::new();
    s.say(200, json!({"ok": true, "redeemed": true}));
    let mut m = model();
    typed(&mut m, &format!("  {CODE} "));
    m.redeem(session(&s));
    m.frame(Instant::now());
    let seen = s.seen();
    assert_eq!(seen.len(), 1);
    assert_eq!((seen[0].0, seen[0].1.as_str()), (Method::Post, "/me/invite"));
    assert_eq!(serde_json::from_str::<Value>(&seen[0].2).unwrap(), json!({"code": CODE}));
    assert_eq!(m.state, InviteState::Redeemed { already: false });
    assert!(m.code.is_empty(), "cleared as soon as the pool took it");
    assert!(m.redeemed() && !m.can_redeem());
    typed(&mut m, CODE);
    m.redeem(session(&s));
    assert_eq!(s.seen().len(), 1, "a redeemed account does not send another");
}

#[test]
fn an_account_that_already_holds_a_code_is_told_so() {
    let s = Script::new();
    s.say(200, json!({"ok": true, "already": true}));
    let mut m = model();
    typed(&mut m, CODE);
    m.redeem(session(&s));
    m.frame(Instant::now());
    assert_eq!(m.state, InviteState::Redeemed { already: true });
}

#[test]
fn every_refusal_is_in_plain_words_and_the_box_keeps_the_typo_to_fix() {
    let cases: Vec<(u16, Value, Option<u64>, &str)> = vec![
        (422, json!({"error": "invite_invalid", "message": "<b>server words</b>"}), None, "not valid"),
        (400, json!({"error": "invalid_request", "field": "code"}), None, "24 letters and digits"),
        (429, json!({"error": "daily_limit", "metric": "invite_tries"}), Some(3600), "invite codes today"),
        (403, json!({"error": "account_not_approved"}), None, ""),
        (401, json!({"error": "unauthorized"}), None, ""),
    ];
    for (status, body, after, want) in cases {
        let s = Script::new();
        match after {
            Some(a) => s.say_after(status, a, body),
            None => s.say(status, body),
        };
        if status == 401 {
            s.fail(ApiError::SignedOut);
        }
        let mut m = model();
        typed(&mut m, "WRONG");
        m.redeem(session(&s));
        m.frame(Instant::now());
        let msg = problem(&m);
        assert!(msg.contains(want) && !msg.contains('<'), "{status}: {msg}");
        assert_eq!(m.code, "WRONG", "{status}: the box still holds what was typed");
        assert!(!msg.contains("WRONG"));
    }
}

#[test]
fn a_429_shows_the_wait_and_blocks_the_button_until_it_is_over() {
    let s = Script::new();
    s.say_after(429, 600, json!({"error": "daily_limit", "metric": "invite_tries"}));
    let mut m = model();
    typed(&mut m, CODE);
    m.redeem(session(&s));
    let t = Instant::now();
    m.frame(t);
    assert!(m.wait().is_some());
    assert!(!m.can_redeem());
    m.redeem(session(&s));
    assert_eq!(s.seen().len(), 1, "no hammering while the wait is on");
    m.frame(t + Duration::from_secs(601));
    assert!(m.wait().is_none() && m.can_redeem());
}

#[test]
fn a_session_that_ended_and_a_lost_connection_say_what_to_do() {
    let s = Script::new();
    s.fail(ApiError::SignedOut);
    let mut m = model();
    typed(&mut m, CODE);
    m.redeem(session(&s));
    m.frame(Instant::now());
    assert!(problem(&m).contains("Sign in again"));
    let s = Script::new();
    s.fail(ApiError::Offline("no network".into()));
    m.redeem(session(&s));
    m.frame(Instant::now());
    assert!(matches!(&m.state, InviteState::Failed(p) if p.retry));
    // not signed in at all: nothing is sent
    let mut m = model();
    typed(&mut m, CODE);
    m.redeem(None);
    assert!(matches!(m.state, InviteState::Failed(_)));
}

#[test]
fn the_code_is_never_printed_and_reset_forgets_it() {
    let mut m = model();
    typed(&mut m, CODE);
    assert!(!format!("{m:?}").contains("ABCD"));
    m.reset();
    assert!(m.code.is_empty());
    assert_eq!(m.state, InviteState::Unknown);
}

#[test]
fn a_code_that_cannot_be_one_never_leaves_the_machine() {
    let s = Script::new();
    let mut m = model();
    typed(&mut m, &"X".repeat(101));
    m.redeem(session(&s));
    m.frame(Instant::now());
    assert!(s.seen().is_empty());
    assert!(problem(&m).contains("24 letters and digits"));
}

#[test]
fn invite_required_is_one_clear_message_with_the_way_to_the_account_window() {
    let s = Script::new();
    s.say(403, json!({"error": "invite_required", "message": "The shared pool is invite-only."}));
    let srv = Pool::new(&*s);
    let e = super::transport::block_on(srv.mine()).unwrap_err();
    assert!(matches!(e.kind(), Some(RefusalKind::InviteRequired)));
    let m = e.message();
    assert!(m.contains("invite code") && m.contains("Account window") && m.contains("Invite code"));
    assert_eq!(m, INVITE_REQUIRED_TEXT);
    let mut inv = model();
    inv.note_required();
    assert!(problem(&inv).contains("invite code"));
    inv.state = InviteState::Redeemed { already: false };
    inv.note_required();
    assert!(inv.redeemed(), "a redeemed account is not told otherwise");
}

fn invite_required_everywhere(s: &Arc<Script>) -> AccountSnapshot {
    s.say(403, json!({"error": "invite_required"}));
    AccountSnapshot::ready(s.clone())
}

#[test]
fn the_browser_the_list_and_the_publish_window_all_point_to_the_invite_code() {
    // browse
    let s = Script::new();
    let mut b = super::browse::BrowseModel::new(Runner::Inline, Arc::new(super::browse::StoreImages), super::browse::ImportLog::memory()).with_retry(RetryPolicy::none());
    b.open_browse();
    let snap = invite_required_everywhere(&s);
    b.frame(Instant::now(), &snap);
    b.frame(Instant::now(), &snap);
    let p = b.results.problem.clone().expect("a problem");
    assert!(p.account && p.message.contains("invite code"), "{p:?}");

    // my shared items
    let s = Script::new();
    let mut m = super::mine::MineModel::new(Runner::Inline, super::publish::LogIo::memory(), Arc::new(super::mine::PlatformSink)).with_retry(RetryPolicy::none());
    m.open_list();
    let snap = invite_required_everywhere(&s);
    m.frame(Instant::now(), &snap);
    m.frame(Instant::now(), &snap);
    let p = m.list.problem.clone().expect("a problem");
    assert!(p.account && p.message.contains("invite code"), "{p:?}");

    // publish
    let s = Script::new();
    let mut p = super::publish::PublishModel::new(Runner::Inline, Arc::new(std::collections::HashMap::new()), super::publish::LogIo::memory()).with_retry(RetryPolicy::none());
    let bank = json!({"id": "b1", "name": "Unit 1", "questions": [{"id": "q0", "stem": "Q", "image": null, "kind": {"Mc": {"answers": [{"text": "A", "correct": true}, {"text": "B", "correct": false}]}}}], "created": 1, "modified": 2}).to_string();
    p.start(super::publish::ShareSource::from_bank_json("bank:b1", "Bank", "Unit 1", &bank));
    let snap = AccountSnapshot::ready(s.clone());
    p.frame(Instant::now(), &snap);
    p.form.license = Some(License::CcBy);
    p.form.attest = true;
    s.say(403, json!({"error": "invite_required"}));
    p.check();
    p.frame(Instant::now(), &snap);
    let f = p.failure.clone().expect("a failure");
    assert!(f.account && f.message.contains("invite code"), "{f:?}");
}

#[test]
fn other_account_statuses_do_not_change_the_text() {
    let e = PoolError::Refused(Box::new(Refusal {
        detail: super::api::ErrorDetail::from_response(&Response::new(403, r#"{"error":"account_not_approved"}"#)),
        kind: RefusalKind::NotApproved { message: None },
    }));
    assert!(!e.message().contains("invite"));
    let _ = Status::Pending;
}

#[test]
fn the_section_draws_in_every_state() {
    let ctx = egui::Context::default();
    crate::theme::setup(&ctx);
    let mut m = model();
    for state in [
        InviteState::Unknown,
        InviteState::Redeeming,
        InviteState::Redeemed { already: false },
        InviteState::Redeemed { already: true },
        InviteState::Failed(super::browse::Problem { message: "nope".into(), retry: true, retry_at: Some(Instant::now() + Duration::from_secs(30)), account: false }),
    ] {
        m.state = state;
        m.code = "ABCD".into();
        for i in 0..2 {
            let input = egui::RawInput { screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(600.0, 800.0))), time: Some(i as f64 * 0.1), ..Default::default() };
            let out = ctx.run_ui(input, |ui| super::ui::invite_section(ui, &mut m, None, true));
            out.drop_without_applying_deltas();
        }
    }
}
