//! The approval queue against a scripted session: reading it, the password gate, what an approval and
//! a refusal actually send, and the rules about who sees what.

use std::collections::VecDeque;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use serde_json::{Value, json};
use web_time::Instant;

use super::admin::*;
use super::api::ApiError;
use super::model::Runner;
use super::pool::*;
use super::publish::{AccountSnapshot, Gate, SharedSession};
use super::transport::{LocalFuture, Method, Request, Response};

#[derive(Clone, Debug)]
struct Seen {
    method: Method,
    path: String,
    body: String,
    sudo: Option<String>,
    device: Option<String>,
}

#[derive(Default)]
struct Script {
    replies: Mutex<VecDeque<Result<Response, ApiError>>>,
    seen: Mutex<Vec<Seen>>,
    device: Mutex<Option<String>>,
}

impl Script {
    fn new() -> Arc<Script> {
        Arc::new(Script::default())
    }
    fn say(&self, status: u16, body: Value) -> &Self {
        self.replies.lock().unwrap().push_back(Ok(Response::new(status, &body.to_string())));
        self
    }
    fn seen(&self) -> Vec<Seen> {
        self.seen.lock().unwrap().clone()
    }
    fn known_device(&self, token: &str) -> &Self {
        *self.device.lock().unwrap() = Some(token.to_string());
        self
    }
}

impl PoolSession for Script {
    fn base(&self) -> String {
        "https://pool.test".into()
    }
    fn send<'a>(&'a self, build: &'a dyn Fn() -> Request) -> LocalFuture<'a, Result<Response, ApiError>> {
        Box::pin(async move {
            let r = build();
            self.seen.lock().unwrap().push(Seen {
                method: r.method,
                path: r.url.trim_start_matches("https://pool.test").to_string(),
                body: r.body.as_ref().map(|b| b.expose().to_string()).unwrap_or_default(),
                sudo: r.header("x-sudo-token").map(str::to_string),
                device: r.header("x-device-token").map(str::to_string),
            });
            self.replies.lock().unwrap().pop_front().unwrap_or_else(|| panic!("no scripted reply for {:?}", r.url))
        })
    }
    fn device_token(&self, _account: Option<&str>) -> Option<super::secret::Secret> {
        self.device.lock().unwrap().clone().map(super::secret::Secret::new)
    }
}

fn session(s: &Arc<Script>) -> Option<SharedSession> {
    Some(s.clone())
}

fn model() -> AdminModel {
    AdminModel::new(Runner::Inline).with_retry(RetryPolicy::none())
}

/// One open application, as the server lists it.
fn queue_json() -> Value {
    json!({
        "open": 2,
        "oldestAgeSeconds": 300_000,
        "next_cursor": null,
        "items": [
            {
                "request_id": "11111111-1111-1111-1111-111111111111",
                "user_id": "aaaaaaaa-aaaa-aaaa-aaaa-aaaaaaaaaaaa",
                "status": "open",
                "activated_at": 1,
                "age_seconds": 300_000,
                "decided_at": null,
                "email": "olga@school.example.org",
                "email_domain": "school.example.org",
                "server_labels": {
                    "domain_hint": "Algebra team\nnot a hint",
                    "domain_history": { "approved": 12, "rejected": 1 },
                    "prior_rejections": 2,
                    "profile_changed": true,
                    "suspended": false
                },
                "applicant": { "display_name": "Olga \u{202e}Teacher", "profile": { "school": "North High\nSchool" } },
                "trust": "pending"
            },
            {
                "request_id": "22222222-2222-2222-2222-222222222222",
                "user_id": "bbbbbbbb-bbbb-bbbb-bbbb-bbbbbbbbbbbb",
                "status": "open",
                "activated_at": 2,
                "age_seconds": 3600,
                "decided_at": null,
                "email": "sam@other.example.org",
                "email_domain": "other.example.org",
                "server_labels": { "domain_hint": null, "domain_history": { "approved": 0, "rejected": 0 }, "prior_rejections": 0, "profile_changed": false, "suspended": true },
                "applicant": { "display_name": "Sam", "profile": { "school": "South High" } },
                "trust": "pending"
            }
        ]
    })
}

fn loaded(s: &Arc<Script>) -> AdminModel {
    let mut m = model();
    m.load(session(s));
    m.frame(Instant::now());
    m
}

fn app(m: &AdminModel, i: usize) -> Application {
    match &m.state {
        State::Loaded(q) => q.items[i].clone(),
        other => panic!("{other:?}"),
    }
}

#[test]
fn the_queue_is_read_from_the_admin_route_and_applicant_text_is_plain() {
    let s = Script::new();
    s.say(200, queue_json());
    let m = loaded(&s);
    let seen = s.seen();
    assert_eq!(seen.len(), 1);
    assert_eq!(seen[0].method, Method::Get);
    assert_eq!(seen[0].path, "/admin/queue?status=open");
    let State::Loaded(q) = &m.state else { panic!("{:?}", m.state) };
    assert_eq!(q.open, 2);
    assert_eq!(q.oldest_seconds, 300_000);
    let first = &q.items[0];
    // The name and the school are the application's snapshot, and a bidi or newline character never survives.
    assert_eq!(first.display_name, "Olga Teacher");
    assert_eq!(first.school, "North High School");
    assert_eq!(first.email, "olga@school.example.org");
    assert_eq!(first.email_domain, "school.example.org");
    assert_eq!(first.prior_rejections, 2);
    assert!(first.profile_changed && !first.suspended);
    assert_eq!(first.domain_history, Some((12, 1)));
    assert_eq!(first.domain_hint.as_deref(), Some("Algebra team not a hint"));
    assert_eq!(summary(q), "2 applications are waiting. The oldest has waited 3 days 11 hours.");
    assert!(q.items[1].suspended);
}

#[test]
fn an_approval_asks_for_the_password_first_then_goes_with_the_evidence() {
    let s = Script::new();
    s.say(200, queue_json());
    let mut m = loaded(&s);
    let first = app(&m, 0);
    m.open_decision(&first, true);
    if let Some(d) = m.decision.as_mut() {
        d.evidence_note = "staff directory".into();
    }
    m.frame(Instant::now());
    // Pressing Approve without a token opens the password box and sends nothing.
    m.decide(session(&s));
    assert!(m.confirming);
    assert_eq!(s.seen().len(), 1, "nothing but the read has gone yet");
    // The password is sent once, and cleared from the form.
    m.password = "correct horse battery staple".into();
    s.say(200, json!({ "sudo_token": "sudo-1", "expires_in": 300 }));
    s.say(200, json!({ "ok": true, "decision": "approved" }));
    m.confirm_password(session(&s));
    assert!(m.password.is_empty(), "the password never stays in the form");
    m.frame(Instant::now());
    // The waiting decision went by itself.
    m.frame(Instant::now());
    let seen = s.seen();
    let reauth = seen.iter().find(|x| x.path == "/auth/reauth").expect("reauth");
    assert_eq!(reauth.body, json!({ "password": "correct horse battery staple" }).to_string());
    let approve = seen.iter().find(|x| x.path == "/admin/approve").expect("approve");
    let body: Value = serde_json::from_str(&approve.body).unwrap();
    assert_eq!(body["user_id"], "aaaaaaaa-aaaa-aaaa-aaaa-aaaaaaaaaaaa");
    assert_eq!(body["request_id"], "11111111-1111-1111-1111-111111111111");
    assert_eq!(body["evidence"], "directory");
    assert_eq!(approve.sudo.as_deref(), Some("sudo-1"), "every change carries the sudo token");
    assert!(m.decision.is_none(), "the form closes once it is done");
    match &m.state {
        State::Loaded(q) => {
            assert_eq!(q.open, 1);
            assert_eq!(q.items.len(), 1, "the approved application left the queue");
            assert_eq!(q.items[0].display_name, "Sam");
        }
        other => panic!("{other:?}"),
    }
    assert!(m.notice.as_deref().unwrap_or("").contains("Approved Olga"));
}

#[test]
fn the_evidence_choice_and_notes_are_what_the_server_documents() {
    let s = Script::new();
    s.say(200, queue_json());
    let mut m = loaded(&s);
    s.say(200, json!({ "sudo_token": "sudo-1", "expires_in": 300 }));
    m.password = "pw-for-the-test".into();
    m.confirm_password(session(&s));
    m.frame(Instant::now());
    let first = app(&m, 0);
    m.open_decision(&first, true);
    if let Some(d) = m.decision.as_mut() {
        d.evidence_note = "staff directory".into();
    }
    if let Some(d) = m.decision.as_mut() {
        d.evidence = 1;
        d.evidence_note = "Ms Rivera in the office confirmed it".into();
        d.note = "known from the algebra team".into();
    }
    s.say(200, json!({ "ok": true }));
    m.decide(session(&s));
    m.frame(Instant::now());
    let seen = s.seen();
    let approve = seen.iter().find(|x| x.path == "/admin/approve").expect("approve");
    let body: Value = serde_json::from_str(&approve.body).unwrap();
    assert_eq!(body["evidence"], "school_confirmed");
    assert_eq!(body["evidence_note"], "Ms Rivera in the office confirmed it");
    assert_eq!(body["note"], "known from the algebra team");
}

#[test]
fn a_refusal_needs_a_reason_and_sends_it_with_the_private_note_apart() {
    let s = Script::new();
    s.say(200, queue_json());
    let mut m = loaded(&s);
    s.say(200, json!({ "sudo_token": "sudo-1", "expires_in": 300 }));
    m.password = "pw".into();
    m.confirm_password(session(&s));
    m.frame(Instant::now());
    let first = app(&m, 0);
    m.open_decision(&first, false);
    // No reason: nothing is sent and the form says why.
    m.decide(session(&s));
    assert!(m.notice.as_deref().unwrap_or("").contains("why"));
    assert!(s.seen().iter().all(|x| x.path != "/admin/reject"), "a refusal without a reason does not go");
    if let Some(d) = m.decision.as_mut() {
        d.reason = "We could not find you in the staff directory.".into();
        d.note = "third try this month".into();
    }
    s.say(200, json!({ "ok": true }));
    m.decide(session(&s));
    m.frame(Instant::now());
    let seen = s.seen();
    let reject = seen.iter().find(|x| x.path == "/admin/reject").expect("reject");
    let body: Value = serde_json::from_str(&reject.body).unwrap();
    assert_eq!(body["reason"], "We could not find you in the staff directory.");
    assert_eq!(body["note"], "third try this month");
    assert!(m.decision.is_none());
    assert!(m.notice.as_deref().unwrap_or("").contains("Refused Olga"));
}

#[test]
fn a_lapsed_sudo_token_asks_for_the_password_again_and_keeps_the_form() {
    let s = Script::new();
    s.say(200, queue_json());
    let mut m = loaded(&s);
    s.say(200, json!({ "sudo_token": "sudo-1", "expires_in": 300 }));
    m.password = "pw".into();
    m.confirm_password(session(&s));
    m.frame(Instant::now());
    let first = app(&m, 0);
    m.open_decision(&first, true);
    if let Some(d) = m.decision.as_mut() {
        d.evidence_note = "staff directory".into();
    }
    if let Some(d) = m.decision.as_mut() {
        d.note = "keep me".into();
    }
    s.say(403, json!({ "error": "sudo_required" }));
    m.decide(session(&s));
    m.frame(Instant::now());
    assert!(m.confirming, "the password box opens again");
    assert!(m.decision.is_some(), "what was typed is not lost");
    assert_eq!(m.decision.as_ref().unwrap().note, "keep me");
    assert!(!m.sudo_ready());
}

#[test]
fn a_409_means_someone_else_decided_it_and_the_row_leaves() {
    let s = Script::new();
    s.say(200, queue_json());
    let mut m = loaded(&s);
    s.say(200, json!({ "sudo_token": "sudo-1", "expires_in": 300 }));
    m.password = "pw".into();
    m.confirm_password(session(&s));
    m.frame(Instant::now());
    let first = app(&m, 0);
    m.open_decision(&first, true);
    if let Some(d) = m.decision.as_mut() {
        d.evidence_note = "staff directory".into();
    }
    s.say(409, json!({ "error": "not_pending" }));
    m.decide(session(&s));
    m.frame(Instant::now());
    assert!(m.decision.is_none());
    match &m.state {
        State::Loaded(q) => {
            assert_eq!(q.items.len(), 1);
            assert!(q.items.iter().all(|a| a.request_id != first.request_id));
            assert_eq!(q.open, 1);
        }
        other => panic!("{other:?}"),
    }
    assert!(m.notice.as_deref().unwrap_or("").contains("Someone else decided"));
}

#[test]
fn the_token_is_good_for_five_minutes_only() {
    let s = Script::new();
    s.say(200, queue_json());
    let mut m = loaded(&s);
    let t0 = Instant::now();
    s.say(200, json!({ "sudo_token": "sudo-1" }));
    m.password = "pw".into();
    m.confirm_password(session(&s));
    m.frame(t0);
    assert!(m.sudo_ready());
    m.frame(t0 + Duration::from_secs(299));
    assert!(m.sudo_ready());
    m.frame(t0 + Duration::from_secs(301));
    assert!(!m.sudo_ready(), "the server's five minutes are not assumed longer here");
}

#[test]
fn nothing_goes_out_without_a_session_and_an_admin_sees_a_refusal_in_words() {
    let mut m = model();
    m.load(None);
    assert!(matches!(m.state, State::Failed(_)));
    let State::Failed(p) = &m.state else { panic!() };
    assert!(p.message.contains("Sign in first"));

    // A teacher's account (or a route that says so) is told plainly, and the read is not retried in a loop.
    let s = Script::new();
    s.say(403, json!({ "error": "forbidden" }));
    let mut m = model();
    m.load(session(&s));
    m.frame(Instant::now());
    let State::Failed(p) = &m.state else { panic!("{:?}", m.state) };
    assert!(p.message.contains("not an admin"), "{}", p.message);
    assert!(!p.retry);
}

#[test]
fn another_account_never_sees_what_this_one_read() {
    let s = Script::new();
    s.say(200, queue_json());
    let mut m = loaded(&s);
    let snap = |who: &str| AccountSnapshot { account: Some(format!("https://pool.test\n{who}")), ..AccountSnapshot::of_gate(Gate::Ready) };
    m.note_account(&snap("ada@school.org"));
    assert!(matches!(m.state, State::Loaded(_)), "the same account keeps its list");
    m.note_account(&snap("bob@school.org"));
    assert_eq!(m.state, State::Closed, "another admin starts from nothing");
    assert!(m.password.is_empty() && m.decision.is_none());
}

#[test]
fn the_device_token_rides_on_the_admin_routes() {
    let s = Script::new();
    s.known_device("device-1");
    s.say(200, queue_json());
    let m = loaded(&s);
    assert!(matches!(m.state, State::Loaded(_)));
    assert_eq!(s.seen()[0].device.as_deref(), Some("device-1"), "the queue read names the device");
}

#[test]
fn the_queue_age_is_said_in_whole_units() {
    assert_eq!(age_text(0), "under an hour");
    assert_eq!(age_text(3_599), "under an hour");
    assert_eq!(age_text(3_600), "1 hour");
    assert_eq!(age_text(7_200), "2 hours");
    assert_eq!(age_text(86_400), "1 day");
    assert_eq!(age_text(90_000), "1 day 1 hour");
    assert_eq!(age_text(183_600), "2 days 3 hours");
    let q = Queue { open: 1, oldest_seconds: 90_000, items: vec![] };
    assert_eq!(summary(&q), "1 application is waiting. The oldest has waited 1 day 1 hour.");
    assert_eq!(summary(&Queue::default()), "Nothing is waiting. Every application has been decided.");
}

// ---- the screen itself ---------------------------------------------------------------------------

fn frame(ctx: &egui::Context, width: f32, f: &mut dyn FnMut(&mut egui::Ui)) {
    for i in 0..2 {
        let input = egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(width, 1600.0))),
            time: Some(i as f64 * 0.1),
            ..Default::default()
        };
        let out = ctx.run_ui(input, |ui| f(ui));
        out.drop_without_applying_deltas();
    }
}

#[test]
fn the_admin_tab_draws_its_list_form_and_password_box_at_every_width() {
    use super::model::{AccountModel, Runner as ModelRunner, SettingsIo};
    use super::shared::Tab;
    use super::shared_ui::SharedUi;
    use super::api::{Me, Status};
    use super::ui::AccountUi;

    let ctx = egui::Context::default();
    crate::theme::setup(&ctx);
    let s = Script::new();
    s.say(200, queue_json());
    let mut a = AccountUi::in_page();
    a.model = AccountModel::new(Arc::new(|_| panic!("no client may be built")), ModelRunner::Inline, SettingsIo::memory());
    a.model.me = Some(Me {
        email: "admin@school.org".into(),
        display_name: "Admin".into(),
        school: "North High".into(),
        status: Status::Approved,
        trust: "admin".into(),
    });
    assert!(a.model.is_admin());
    let mut page = SharedUi::default();
    page.admin = loaded(&s);
    page.model.goto(Tab::Admin);
    let (mut b, mut m): (super::browse_ui::BrowseUi, super::mine_ui::MineUi) = Default::default();
    let mut banks = vec![];
    for width in [1000.0, 420.0] {
        frame(&ctx, width, &mut |x| {
            page.tabs(x, &mut a, &mut b, &mut m, &mut banks, &[], false);
        });
    }
    // The decision form and the password box draw too.
    let first = app(&page.admin, 0);
    page.admin.open_decision(&first, true);
    if let Some(d) = page.admin.decision.as_mut() {
        d.evidence_note = "staff directory".into();
    }
    page.admin.confirming = true;
    for width in [1000.0, 420.0] {
        frame(&ctx, width, &mut |x| {
            page.tabs(x, &mut a, &mut b, &mut m, &mut banks, &[], false);
        });
    }
    // And the empty queue has its own line.
    page.admin.state = State::Loaded(Queue::default());
    page.admin.decision = None;
    page.admin.confirming = false;
    frame(&ctx, 1000.0, &mut |x| {
        page.tabs(x, &mut a, &mut b, &mut m, &mut banks, &[], false);
    });
}

#[test]
fn the_capture_fixture_shows_the_queue_and_its_forms_without_a_request() {
    let fixture: Value = serde_json::from_str(include_str!("../../tools/ui-grading/fixtures/admin-queue.json")).unwrap();
    let q = parse_queue(&fixture).expect("the capture fixture is a queue");
    assert_eq!(q.items.len(), 5);
    let mut m = AdminModel::new(Runner::Inline);
    m.qa_show(q.clone(), "");
    assert_eq!(m.state, State::Loaded(q.clone()));
    assert!(m.decision.is_none() && !m.confirming && !m.busy());
    m.qa_show(q.clone(), "refuse");
    assert_eq!(m.decision.as_ref().map(|d| (d.who.as_str(), d.approve)), Some(("Rosa Delgado", false)));
    m.qa_show(q, "confirm");
    assert!(m.confirming && m.decision.as_ref().is_some_and(|d| d.approve) && m.password.is_empty());
}

// ---- the growth report ---------------------------------------------------------------------------

fn growth_json(days: i64, visits: Value) -> Value {
    json!({
        "days": days,
        "sources": [
            { "source": "fb-math", "signups": 12, "approved": 9, "purchases": 3, "revenue_cents": 4500, "refunds": 1 },
            { "source": null, "signups": 5, "approved": 2, "purchases": 0, "revenue_cents": 0, "refunds": 0 },
            { "source": "reddit\u{202e}x", "signups": 1, "approved": 0, "purchases": 1, "revenue_cents": 123_456, "refunds": 0 }
        ],
        "totals": { "signups": 18, "approved": 11, "purchases": 4, "revenue_cents": 127_956, "refunds": 1 },
        "visits": visits
    })
}

fn visits_on() -> Value {
    json!({ "available": true, "days": 30, "total": 340, "by_referrer": [
        { "referrer": "reddit.com", "visits": 200 }, { "referrer": null, "visits": 90 }, { "referrer": "facebook.com", "visits": 50 }
    ] })
}

#[test]
fn the_growth_report_parses_with_plain_text_and_the_no_tag_row() {
    let g = parse_growth(&growth_json(30, visits_on())).unwrap();
    assert_eq!(g.days, 30);
    assert_eq!(g.sources.len(), 3);
    assert_eq!(g.sources[0], SourceRow { source: Some("fb-math".into()), signups: 12, approved: 9, purchases: 3, revenue_cents: 4500, refunds: 1 });
    assert_eq!(source_label(&g.sources[1].source), "(no tag)");
    assert_eq!(g.sources[2].source.as_deref(), Some("redditx"), "server text is made plain");
    assert_eq!((g.totals.signups, g.totals.revenue_cents, g.totals.source.clone()), (18, 127_956, None));
    assert!(g.visits.available);
    assert_eq!((g.visits.total, g.visits.days, g.visits.by_referrer.len()), (340, Some(30), 3));
    assert_eq!(referrer_label(&g.visits.by_referrer[1].referrer), "(direct or not sent)");
    assert_eq!(visits_line(&g.visits), None);
    assert_eq!(growth_summary(&g), "18 sign-ups in the last 30 days, from 2 tagged links.");
    assert_eq!(super::admin_ui::growth_cells(&g.sources[2]), ["1", "0", "1", "$1,234.56", "0"].map(String::from));
    // Not a report.
    assert!(parse_growth(&json!({ "items": [] })).is_none());
    assert!(parse_growth(&json!({ "days": 30, "sources": "x", "totals": {} })).is_none());
}

#[test]
fn visits_that_are_not_connected_say_so_in_one_line() {
    let g = parse_growth(&growth_json(7, json!({ "available": false, "reason": "not_configured" }))).unwrap();
    assert!(!g.visits.available && g.visits.by_referrer.is_empty());
    assert_eq!(visits_line(&g.visits), Some(VISITS_OFF));
    assert_eq!(VISITS_OFF, "Visit counts are not connected yet.");
    let g = parse_growth(&growth_json(7, json!({ "available": false, "reason": "unavailable" }))).unwrap();
    assert_eq!(visits_line(&g.visits), Some(VISITS_UNAVAILABLE));
    // An older server without a visits part reads as not connected.
    let mut v = growth_json(7, Value::Null);
    v.as_object_mut().unwrap().remove("visits");
    assert_eq!(visits_line(&parse_growth(&v).unwrap().visits), Some(VISITS_OFF));
}

#[test]
fn revenue_is_said_in_dollars() {
    assert_eq!(dollars(0), "$0.00");
    assert_eq!(dollars(5), "$0.05");
    assert_eq!(dollars(1250), "$12.50");
    assert_eq!(dollars(99_999), "$999.99");
    assert_eq!(dollars(100_000), "$1,000.00");
    assert_eq!(dollars(123_456_789), "$1,234,567.89");
    assert_eq!(dollars(-1250), "-$12.50");
}

#[test]
fn the_growth_report_is_read_for_the_chosen_window_from_the_admin_route() {
    let s = Script::new();
    s.known_device("device-1");
    s.say(200, growth_json(30, visits_on()));
    let mut m = model();
    m.open_growth(session(&s));
    m.frame(Instant::now());
    assert_eq!(m.view, View::Growth);
    let GrowthState::Loaded(g) = &m.growth else { panic!("{:?}", m.growth) };
    assert_eq!(g.totals.signups, 18);
    let seen = s.seen();
    assert_eq!((seen[0].method, seen[0].path.as_str(), seen[0].device.as_deref()), (Method::Get, "/admin/growth?days=30", Some("device-1")));
    // Opening it again reads nothing; another window reads that window.
    m.open_growth(session(&s));
    assert_eq!(s.seen().len(), 1);
    s.say(200, growth_json(7, json!({ "available": false, "reason": "not_configured" })));
    m.set_growth_days(7, session(&s));
    m.frame(Instant::now());
    assert_eq!(s.seen()[1].path, "/admin/growth?days=7");
    assert!(matches!(&m.growth, GrowthState::Loaded(g) if g.days == 7));
    // Only the offered windows.
    m.set_growth_days(45, session(&s));
    assert_eq!((s.seen().len(), m.growth_days), (2, 7));
    // A refused re-read keeps the report on screen with a line under it.
    s.say(429, json!({ "error": "rate_limited" }));
    m.load_growth(session(&s));
    m.frame(Instant::now());
    assert!(matches!(m.growth, GrowthState::Loaded(_)));
    assert!(m.growth_notice.as_deref().is_some_and(|n| n.contains("Too many tries")), "{:?}", m.growth_notice);
}

#[test]
fn the_growth_report_refuses_in_words_and_is_forgotten_by_the_next_account() {
    let mut m = model();
    m.open_growth(None);
    let GrowthState::Failed(p) = &m.growth else { panic!() };
    assert!(p.message.contains("Sign in first"));
    let s = Script::new();
    s.say(403, json!({ "error": "forbidden" }));
    let mut m = model();
    m.open_growth(session(&s));
    m.frame(Instant::now());
    let GrowthState::Failed(p) = &m.growth else { panic!("{:?}", m.growth) };
    assert!(p.message.contains("not an admin") && p.message.contains("growth report"), "{}", p.message);
    // A failure is read again when the screen is opened again.
    s.say(200, growth_json(30, visits_on()));
    m.open_growth(session(&s));
    m.frame(Instant::now());
    assert!(matches!(m.growth, GrowthState::Loaded(_)));
    let snap = |who: &str| AccountSnapshot { account: Some(format!("https://pool.test\n{who}")), ..AccountSnapshot::of_gate(Gate::Ready) };
    m.note_account(&snap("ada@school.org"));
    m.note_account(&snap("bob@school.org"));
    assert_eq!(m.growth, GrowthState::Closed, "another admin starts from nothing");
}

#[test]
fn the_growth_screen_draws_its_table_and_visits_at_every_width() {
    use super::model::{AccountModel, Runner as ModelRunner, SettingsIo};
    use super::shared::Tab;
    use super::shared_ui::SharedUi;
    use super::api::{Me, Status};
    use super::ui::AccountUi;

    let ctx = egui::Context::default();
    crate::theme::setup(&ctx);
    let s = Script::new();
    s.say(200, growth_json(30, visits_on()));
    let mut a = AccountUi::in_page();
    a.model = AccountModel::new(Arc::new(|_| panic!("no client may be built")), ModelRunner::Inline, SettingsIo::memory());
    a.model.me = Some(Me { email: "admin@school.org".into(), display_name: "Admin".into(), school: "North High".into(), status: Status::Approved, trust: "admin".into() });
    let mut page = SharedUi::default();
    page.admin = model();
    page.admin.open_growth(session(&s));
    page.admin.frame(Instant::now());
    page.model.goto(Tab::Admin);
    let (mut b, mut mi): (super::browse_ui::BrowseUi, super::mine_ui::MineUi) = Default::default();
    let mut banks = vec![];
    for width in [1000.0, 420.0] {
        frame(&ctx, width, &mut |x| {
            page.tabs(x, &mut a, &mut b, &mut mi, &mut banks, &[], false);
        });
    }
    assert_eq!(page.admin.view, View::Growth, "the growth screen stays chosen");
    // Not connected, and an empty window.
    let mut g = parse_growth(&growth_json(30, json!({ "available": false, "reason": "not_configured" }))).unwrap();
    g.sources.clear();
    page.admin.growth = GrowthState::Loaded(g);
    frame(&ctx, 1000.0, &mut |x| {
        page.tabs(x, &mut a, &mut b, &mut mi, &mut banks, &[], false);
    });
    // Reading and failed states draw too.
    page.admin.growth = GrowthState::Reading;
    frame(&ctx, 420.0, &mut |x| {
        page.tabs(x, &mut a, &mut b, &mut mi, &mut banks, &[], false);
    });
}
