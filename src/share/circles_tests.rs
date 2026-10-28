//! Circles against a scripted session (no network, no egui window): every route and the errors it
//! can meet, the role matrix, the state machines (join, create, copy, ownership offer), the copy
//! recorded like an import, the share-to-circle destination with the public-publish refusal (D13),
//! and headless smoke frames of every page wide and narrow.

use std::collections::{HashMap, VecDeque};
use std::sync::mpsc::{Receiver, Sender, channel};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use serde_json::{Value, json};
use web_time::Instant;

use super::api::{ApiError, Status};
use super::browse::{BrowseModel, ImageSink, ImportLog};
use super::circle_send::{CircleList, Refused, SendStage};
use super::circles::*;
use super::circles_api::*;
use super::model::Runner;
use super::pool::*;
use super::publish::{AccountSnapshot, Gate, LogIo, PublishModel, ShareSource, Step};
use super::transport::{LocalFuture, Method, Request, Response};
use crate::model::{ImageRef, QuestionBank};

pub(super) const CID: &str = "0123456789abcdef0123456789abcdef";
pub(super) const BID: &str = "fedcba9876543210fedcba9876543210";
pub(super) const NOW: i64 = 1_760_000_000_000;
pub(super) const DAY: i64 = 86_400_000;
const CODE: &str = "C-ABCDEFGHJKMNPQRSTVWXYZ0";

// ---- a scripted session -------------------------------------------------------------------------------------

#[derive(Clone, Debug)]
pub(super) struct Sent {
    pub(super) method: Method,
    pub(super) path: String,
    pub(super) body: Vec<u8>,
}

impl Sent {
    pub(super) fn json(&self) -> Value {
        serde_json::from_slice(&self.body).unwrap_or(Value::Null)
    }
}

#[derive(Default)]
pub(super) struct Script {
    replies: Mutex<VecDeque<Result<Response, ApiError>>>,
    seen: Mutex<Vec<Sent>>,
    /// Replies by `"Method /path"`: used before the general queue.
    routes: Mutex<HashMap<String, VecDeque<Response>>>,
    /// Answers that stay (the page's own loads), used when nothing else is queued for the route.
    sticky: Mutex<HashMap<String, (u16, String)>>,
    gate: Mutex<Option<Receiver<()>>>,
    /// (phase D3) Only a Send page's base read may be answered 404 when a test does not script it; every other
    /// unscripted kept-version read (Compare, the History's version view) still panics, as before.
    send_base_404: std::sync::atomic::AtomicBool,
}

impl Script {
    pub(super) fn new() -> Arc<Script> {
        Arc::new(Script::default())
    }
    pub(super) fn say(&self, status: u16, body: Value) -> &Self {
        self.replies.lock().unwrap().push_back(Ok(Response::new(status, &body.to_string())));
        self
    }
    /// Queues a reply for one route (`m` is `Get`, `Post`, `Patch` or `Delete`).
    pub(super) fn on(&self, m: &str, path: &str, status: u16, body: Value) -> &Self {
        self.routes.lock().unwrap().entry(format!("{m} {path}")).or_default().push_back(Response::new(status, &body.to_string()));
        self
    }
    /// A queued reply for one route whose body is read as bytes (a circle bank).
    pub(super) fn on_raw(&self, m: &str, path: &str, body: Vec<u8>) -> &Self {
        let mut r = Response::new(200, &String::from_utf8_lossy(&body));
        r.bytes = body;
        self.routes.lock().unwrap().entry(format!("{m} {path}")).or_default().push_back(r);
        self
    }
    /// A queued reply for one route that also says how long to wait (`Retry-After`, in seconds).
    pub(super) fn on_wait(&self, m: &str, path: &str, status: u16, body: Value, secs: u64) -> &Self {
        let mut r = Response::new(status, &body.to_string());
        r.retry_after = Some(secs);
        self.routes.lock().unwrap().entry(format!("{m} {path}")).or_default().push_back(r);
        self
    }
    /// The same under the circle's own path.
    pub(super) fn act(&self, m: &str, suffix: &str, status: u16, body: Value) -> &Self {
        self.on(m, &format!("/circles/{CID}{suffix}"), status, body)
    }
    pub(super) fn keep(&self, m: &str, path: &str, status: u16, body: Value) -> &Self {
        self.sticky.lock().unwrap().insert(format!("{m} {path}"), (status, body.to_string()));
        self
    }
    pub(super) fn keep_raw(&self, m: &str, path: &str, body: Vec<u8>) -> &Self {
        self.sticky.lock().unwrap().insert(format!("{m} {path}"), (200, String::from_utf8(body).unwrap()));
        self
    }
    pub(super) fn say_bytes(&self, bytes: Vec<u8>) -> &Self {
        let mut r = Response::new(200, "");
        r.bytes = bytes;
        self.replies.lock().unwrap().push_back(Ok(r));
        self
    }
    pub(super) fn fail(&self, e: ApiError) -> &Self {
        self.replies.lock().unwrap().push_back(Err(e));
        self
    }
    pub(super) fn seen(&self) -> Vec<Sent> {
        self.seen.lock().unwrap().clone()
    }
    pub(super) fn paths(&self) -> Vec<String> {
        self.seen().iter().map(|s| format!("{:?} {}", s.method, s.path)).collect()
    }
    pub(super) fn last(&self) -> Sent {
        self.seen().last().cloned().expect("a request")
    }
    #[allow(dead_code)]
    pub(super) fn hold(&self) -> Sender<()> {
        let (tx, rx) = channel();
        *self.gate.lock().unwrap() = Some(rx);
        tx
    }
    /// (phase D3) This test's Send page reads the base version itself: an unscripted kept-version read is the circle
    /// no longer keeping it (404, the whole-bank path), not a missing script. Off by default: every other version read
    /// still panics when unscripted.
    pub(super) fn send_base_404(&self, on: bool) -> &Self {
        self.send_base_404.store(on, std::sync::atomic::Ordering::Relaxed);
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
            let path = r.url.trim_start_matches("https://pool.test").to_string();
            let body = r.bytes.clone().or_else(|| r.body.as_ref().map(|b| b.expose().as_bytes().to_vec())).unwrap_or_default();
            self.seen.lock().unwrap().push(Sent { method: r.method, path: path.clone(), body });
            let g = self.gate.lock().unwrap().take();
            if let Some(rx) = g {
                let _ = rx.recv_timeout(Duration::from_secs(10));
            }
            let key = format!("{:?} {}", r.method, r.url.trim_start_matches("https://pool.test").split('?').next().unwrap_or_default());
            let queued = self.routes.lock().unwrap().get_mut(&key).and_then(|q| q.pop_front());
            if let Some(q) = queued {
                return Ok(q);
            }
            if let Some(q) = self.replies.lock().unwrap().pop_front() {
                return q;
            }
            let kept = self.sticky.lock().unwrap().get(&key).cloned();
            if let Some((st, b)) = kept {
                let mut r = Response::new(st, &b);
                r.bytes = b.clone().into_bytes();
                return Ok(r);
            }
            // (phase D3) A Send page reads the version its copy is based on once. Only a test that says so may leave
            // that read unscripted (a circle that no longer keeps it: the whole bank, as before phase D3); every
            // other unscripted kept-version read is a missing script and still panics.
            if self.send_base_404.load(std::sync::atomic::Ordering::Relaxed) && r.method == Method::Get && is_kept_version_path(&path) {
                return Ok(Response::new(404, "{\"error\":\"version_not_retained\"}"));
            }
            panic!("no scripted reply for {key}")
        })
    }
}

/// `/circles/:cid/banks/:bid/versions/:n` (not the list, `/versions`).
fn is_kept_version_path(path: &str) -> bool {
    let p: Vec<&str> = path.trim_end_matches('/').split('/').collect();
    p.len() == 7 && p[1] == "circles" && p[3] == "banks" && p[5] == "versions" && p[6].parse::<u64>().is_ok()
}

#[derive(Default)]
pub(super) struct Mem(Mutex<Vec<Vec<u8>>>);

impl ImageSink for Mem {
    fn put(&self, bytes: &[u8]) -> Result<ImageRef, String> {
        self.0.lock().unwrap().push(bytes.to_vec());
        Ok(ImageRef { file: format!("media/{}.webp", sha256_hex(bytes)), width: 2, height: 2 })
    }
}

// ---- fixtures ---------------------------------------------------------------------------------------------------

pub(super) fn now() -> Instant {
    Instant::now()
}

pub(super) fn ready(s: &Arc<Script>) -> AccountSnapshot {
    AccountSnapshot::ready(s.clone())
}

/// A CONFIRMED account still waiting for approval (round 54): signed in, and allowed to use circles.
pub(super) fn pending(s: &Arc<Script>) -> AccountSnapshot {
    AccountSnapshot {
        gate: Gate::NotApproved(Status::Pending),
        session: Some(s.clone()),
        account_open: false,
        account: Some("https://pool.test\npat@school.example.org".into()),
        name: Some("Pat".into()),
    }
}

/// A restore point kept in memory (tests never write the app's real data folder).
pub(super) struct NoDiskRestore;

impl super::browse::RestoreSink for NoDiskRestore {
    fn save(&self, banks: &[QuestionBank]) -> Result<String, String> {
        Ok(format!("memory ({} banks)", banks.len()))
    }
}

/// The circles model with its working-copy links, pictures and restore points in memory: a test run never
/// reads or writes `share-circles.json` (or a backup) in the app's real data folder.
pub(super) fn model() -> CirclesModel {
    let mut m = CirclesModel::new(Runner::Inline, Arc::new(Mem::default())).with_retry(RetryPolicy::none());
    let media: super::publish::SharedMedia = Arc::new(HashMap::<String, Vec<u8>>::new());
    m.edit = super::circle_edit::CircleEditModel::new(Runner::Inline, Arc::new(Mem::default()), media, Arc::new(NoDiskRestore), super::circle_links::LinkLog::memory()).with_retry(RetryPolicy::none());
    m.set_clock(NOW);
    m
}

pub(super) fn summary(role: &str, extra: Value) -> Value {
    let mut v = json!({"id": CID, "name": "Biology team", "role": role, "joined_at": NOW - DAY, "closed_at": null, "updated_at": NOW, "members": 3, "banks": 1});
    merge(&mut v, extra);
    v
}

pub(super) fn merge(v: &mut Value, extra: Value) {
    if let (Some(o), Some(e)) = (v.as_object_mut(), extra.as_object()) {
        for (k, x) in e {
            o.insert(k.clone(), x.clone());
        }
    }
}

pub(super) fn detail(role: &str, extra: Value) -> Value {
    let mut v = json!({
        "circle": {"id": CID, "name": "Biology team", "description": "Unit banks", "role": role, "joined_at": NOW - DAY, "created_at": NOW - 5 * DAY, "updated_at": NOW, "closed_at": null},
        "members": [
            {"user_id": "u-owner", "display_name": "Ms Frizzle", "role": "owner", "joined_at": NOW - 5 * DAY, "is_me": role == "owner"},
            {"user_id": "u-ed", "display_name": "Mr Keesler", "role": "editor", "joined_at": NOW - DAY, "is_me": role == "editor"},
            {"user_id": "u-view", "display_name": "Ms Viewer", "role": "viewer", "joined_at": NOW - DAY, "is_me": role == "viewer"}
        ],
        "banks": [{"id": BID, "title": "Cells", "subject": "Biology", "license": "CC-BY-4.0", "author_name": "Mr Keesler", "is_author": false, "head_version": 1, "question_count": 2, "created_at": NOW - DAY, "updated_at": NOW - DAY}],
        "pending_transfer": null
    });
    merge(&mut v, extra);
    v
}

/// The list, then the circle page (and for an owner its invites; for everyone the activity).
pub(super) fn open_circle(role: &str, extra: Value) -> (CirclesModel, Arc<Script>) {
    let s = Script::new();
    s.keep("Get", "/circles", 200, json!({"circles": [summary(role, json!({}))]}));
    s.keep("Get", &format!("/circles/{CID}"), 200, detail(role, extra));
    s.keep("Get", &format!("/circles/{CID}/invites"), 200, json!({"invites": [], "next_cursor": null}));
    s.keep("Get", &format!("/circles/{CID}/activity"), 200, json!({"activity": [], "next_before": null}));
    let mut m = model();
    m.open_list();
    m.frame(now(), &ready(&s));
    m.open_circle(CID);
    for _ in 0..4 {
        m.frame(now(), &ready(&s));
    }
    (m, s)
}

fn problem(m: &CirclesModel) -> String {
    m.circle.as_ref().and_then(|c| c.act_problem.as_ref()).map(|p| p.message.clone()).unwrap_or_default()
}

pub(super) fn err(code: &str) -> Value {
    json!({"error": code})
}

pub(super) fn bank_body(stems: &[&str]) -> Vec<u8> {
    let qs: Vec<Value> = stems
        .iter()
        .map(|s| json!({"id": format!("src-{s}"), "stem": s, "kind": {"Mc": {"answers": [{"text": "A", "correct": true}, {"text": "B", "correct": false}]}}, "points": 1.0, "difficulty": null, "created": 5, "modified": 6, "keyless": false}))
        .collect();
    let payload = json!({"schema_version": 2, "bank": {"id": "b-src", "name": "Cells", "questions": qs, "created": 1, "modified": 2}}).to_string();
    let head = json!({"circle": {"id": CID, "name": "Biology team"}, "bank": {"id": BID, "title": "Cells", "subject": "Biology", "license": "CC-BY-4.0", "version_license": "CC-BY-4.0", "author_name": "Mr Keesler", "is_author": false, "version": 2, "version_id": "v2", "question_count": stems.len(), "has_images": false, "committed_at": NOW - DAY}, "origins": null});
    let h = head.to_string();
    format!("{},\"payload\":{}}}", &h[..h.len() - 1], payload).into_bytes()
}

// ---- gating -------------------------------------------------------------------------------------------------------

#[test]
fn a_closed_model_does_no_io() {
    let s = Script::new();
    let mut m = model();
    for _ in 0..3 {
        m.frame(now(), &ready(&s));
    }
    assert!(!m.open && !m.busy());
    assert!(s.seen().is_empty());
}

#[test]
fn nothing_is_asked_until_signed_in_and_then_the_list_loads_once() {
    let s = Script::new();
    s.say(200, json!({"circles": [summary("editor", json!({}))]}));
    let mut m = model();
    m.open_list();
    m.frame(now(), &AccountSnapshot { gate: Gate::SignedOut, session: None, account_open: false, account: None, name: None });
    assert!(s.seen().is_empty());
    assert_eq!(m.screen, Screen::Gate);
    for _ in 0..3 {
        m.frame(now(), &ready(&s));
    }
    assert_eq!(s.paths(), vec!["Get /circles"]);
    assert_eq!(m.screen, Screen::List);
    assert_eq!(m.list.items.len(), 1);
}

#[test]
fn a_confirmed_pending_account_uses_circles() {
    // Round 54: a confirmed account waiting for approval may join and use circles; the model is not sent to its gate.
    let s = Script::new();
    s.say(200, json!({"circles": [summary("editor", json!({}))]}));
    let mut m = model();
    m.open_list();
    let snap = pending(&s);
    m.frame(now(), &snap);
    m.frame(now(), &snap);
    assert_eq!(m.screen, Screen::List);
    assert_eq!(s.paths(), vec!["Get /circles"]);
}

#[test]
fn unverified_rejected_and_suspended_accounts_stay_at_the_gate() {
    for gate in [
        Gate::NotApproved(Status::Unverified),
        Gate::NotApproved(Status::Suspended { reason: None }),
        Gate::NotApproved(Status::Rejected { revoked: false, reason: None, reapply_after_ms: None, can_reapply: false }),
    ] {
        let s = Script::new();
        let mut m = model();
        m.open_list();
        let snap = AccountSnapshot { gate, session: Some(s.clone()), account_open: false, account: Some("u-no\nn@school.example.org".into()), name: None };
        m.frame(now(), &snap);
        assert_eq!(m.screen, Screen::Gate);
        assert!(s.seen().is_empty(), "nothing is asked of the pool");
    }
}

#[test]
fn a_pending_account_cannot_start_a_circle() {
    // Round 54: creating a circle waits for approval (the server refuses it); the list offers only the code.
    let s = Script::new();
    s.say(200, json!({"circles": []}));
    let mut m = model();
    m.open_list();
    m.frame(now(), &pending(&s));
    assert_eq!(m.screen, Screen::List);
    m.start_create();
    assert_eq!(m.screen, Screen::List, "no create form for a pending account");
}

#[test]
fn an_empty_list_is_loaded_and_empty() {
    let s = Script::new();
    s.say(200, json!({"circles": []}));
    let mut m = model();
    m.open_list();
    m.frame(now(), &ready(&s));
    m.frame(now(), &ready(&s));
    assert!(m.list.loaded && m.list.items.is_empty());
}

#[test]
fn a_failed_list_offers_try_again_and_a_session_that_ended_goes_to_the_gate() {
    let s = Script::new();
    s.fail(ApiError::Offline("down".into()));
    let mut m = model();
    m.open_list();
    m.frame(now(), &ready(&s));
    m.frame(now(), &ready(&s));
    assert!(m.list.problem.as_ref().is_some_and(|p| p.retry));
    s.fail(ApiError::SignedOut);
    m.retry_list();
    m.frame(now(), &ready(&s));
    m.frame(now(), &ready(&s));
    assert_eq!(m.screen, Screen::Gate);
    assert_eq!(m.gate, Gate::SignedOut);
}

#[test]
fn the_invite_code_error_is_passed_on_to_the_shared_page() {
    let s = Script::new();
    s.say(403, err("invite_required"));
    let mut m = model();
    m.open_list();
    m.frame(now(), &ready(&s));
    m.frame(now(), &ready(&s));
    assert!(m.invite_required);
}

// ---- create ---------------------------------------------------------------------------------------------------------

#[test]
fn a_circle_is_created_and_opened() {
    let s = Script::new();
    s.say(200, json!({"circles": []}));
    let mut m = model();
    m.open_list();
    m.frame(now(), &ready(&s));
    m.start_create();
    m.create.name = "  ".into();
    assert!(m.create_blocker().is_some());
    m.create_circle();
    assert_eq!(s.seen().len(), 1, "nothing was sent for an empty name");
    m.create.name = " Biology team ".into();
    m.create.description = "Unit banks".into();
    s.on("Post", "/circles", 201, json!({"circle": {"id": CID, "name": "Biology team", "role": "owner"}}));
    s.keep("Get", "/circles", 200, json!({"circles": []}));
    s.keep("Get", &format!("/circles/{CID}"), 200, detail("owner", json!({})));
    s.keep("Get", &format!("/circles/{CID}/invites"), 200, json!({"invites": [], "next_cursor": null}));
    s.keep("Get", &format!("/circles/{CID}/activity"), 200, json!({"activity": [], "next_before": null}));
    m.create_circle();
    for _ in 0..4 {
        m.frame(now(), &ready(&s));
    }
    let post = s.seen().into_iter().find(|x| x.method == Method::Post && x.path == "/circles").unwrap();
    assert_eq!(post.json(), json!({"name": "Biology team", "description": "Unit banks"}));
    assert_eq!(m.screen, Screen::Circle);
}

#[test]
fn creating_too_many_circles_says_so_in_plain_words() {
    let s = Script::new();
    s.say(200, json!({"circles": []}));
    let mut m = model();
    m.open_list();
    m.frame(now(), &ready(&s));
    m.start_create();
    m.create.name = "x".into();
    s.on("Post", "/circles", 422, json!({"error": "circles_limit", "metric": "circles_owned", "used": 5, "limit": 5}));
    m.create_circle();
    m.frame(now(), &ready(&s));
    let p = m.create.problem.as_ref().unwrap().message.clone();
    assert!(p.contains("5") && p.contains("14 days"), "{p}");
    assert!(!m.create.working);
}

// ---- join -----------------------------------------------------------------------------------------------------------

#[test]
fn a_pasted_link_or_sentence_holds_the_code() {
    assert_eq!(code_in("join: https://x.test/c?code=C-abcd-efgh-jkmn-pqrs-tvwx-yz01 thanks").as_deref(), Some("C-ABCDEFGHJKMNPQRSTVWXYZ01"));
    assert_eq!(code_in("C-ABCDEFGHJKMNPQRSTVWXYZ01").as_deref(), Some("C-ABCDEFGHJKMNPQRSTVWXYZ01"));
    assert!(code_in("C-ABCD").is_none());
    assert!(code_in("nothing here").is_none());
    assert_eq!(code_groups("C-ABCDEFGH"), "C-ABCD-EFGH");
    // A new code comes from the server already in groups: shown as it is, not split again.
    assert_eq!(code_groups("C-AT6S-A85Q-BTXY-W6RG-MT3H-VH18"), "C-AT6S-A85Q-BTXY-W6RG-MT3H-VH18");
}

#[test]
fn a_code_is_looked_up_before_it_is_used_and_then_joined() {
    let s = Script::new();
    s.keep("Get", "/circles", 200, json!({"circles": []}));
    let mut m = model();
    m.open_list();
    m.frame(now(), &ready(&s));
    m.start_join();
    m.join.text = "no code here".into();
    m.look_up_code();
    assert_eq!(s.seen().len(), 1);
    m.join.text = "see C-ABCD-EFGH-JKMN-PQRS-TVWX-YZ01 ok".into();
    s.on("Post", "/circles/join/preview", 200, json!({"circle": {"id": CID, "name": "Biology team", "owner_name": "Ms Frizzle", "members": 3}, "role": "editor", "already_member": false}));
    m.look_up_code();
    m.frame(now(), &ready(&s));
    assert!(matches!(m.join.stage, JoinStage::Preview(..)));
    assert_eq!(s.last().path, "/circles/join/preview");
    s.on("Post", "/circles/join", 200, json!({"circle_id": CID, "role": "editor", "already": false}));
    m.confirm_join();
    m.frame(now(), &ready(&s));
    assert!(matches!(m.join.stage, JoinStage::Done { role: Role::Editor, .. }));
    let sent = s.seen().into_iter().find(|x| x.path == "/circles/join").unwrap();
    assert_eq!(sent.json()["code"], "C-ABCDEFGHJKMNPQRSTVWXYZ01");
}

#[test]
fn every_join_error_has_its_own_plain_words() {
    let mut said: Vec<String> = Vec::new();
    for (status, body) in [
        (404, err("circle_invite_invalid")),
        (409, err("circle_full")),
        (422, json!({"error": "circles_limit", "metric": "circle_memberships"})),
        (409, err("circle_closed")),
        (429, json!({"error": "daily_limit", "metric": "circle_join_tries", "retry_after": 3600})),
    ] {
        let s = Script::new();
        s.say(200, json!({"circles": []}));
        let mut m = model();
        m.open_list();
        m.frame(now(), &ready(&s));
        m.start_join();
        m.join.text = "C-ABCDEFGHJKMNPQRSTVWXYZ01".into();
        s.on("Post", "/circles/join/preview", status, body);
        m.look_up_code();
        m.frame(now(), &ready(&s));
        let JoinStage::Failed(p) = &m.join.stage else { panic!("{:?}", m.join.stage) };
        assert!(!p.message.is_empty() && !p.message.contains("circle_"), "{}", p.message);
        said.push(p.message.clone());
    }
    let mut u = said.clone();
    u.sort();
    u.dedup();
    assert_eq!(u.len(), said.len(), "{said:?}");
}

// ---- the circle page --------------------------------------------------------------------------------------------

#[test]
fn the_circle_page_loads_members_banks_invites_and_activity() {
    let (m, s) = open_circle("owner", json!({}));
    assert_eq!(s.paths(), vec!["Get /circles", format!("Get /circles/{CID}").as_str(), &format!("Get /circles/{CID}/invites"), &format!("Get /circles/{CID}/activity")]);
    let c = m.circle.as_ref().unwrap();
    assert_eq!(c.detail.as_ref().unwrap().members.len(), 3);
    assert!(c.invites.is_some() && c.activity_loaded);
    assert_eq!(m.screen, Screen::Circle);
}

#[test]
fn a_member_who_is_not_the_owner_never_asks_for_invites() {
    let (_, s) = open_circle("editor", json!({}));
    assert!(!s.paths().iter().any(|p| p.contains("invites")));
}

#[test]
fn a_circle_the_person_is_not_in_is_not_found() {
    let s = Script::new();
    s.say(200, json!({"circles": []}));
    let mut m = model();
    m.open_list();
    m.frame(now(), &ready(&s));
    s.act("Get", "", 404, err("not_found"));
    m.open_circle(CID);
    for _ in 0..3 {
        m.frame(now(), &ready(&s));
    }
    assert_eq!(m.screen, Screen::List, "back to the list with a note");
    assert!(m.notice.is_some());
}

#[test]
fn the_role_matrix() {
    for (role, closed, a, want) in [
        (Role::Owner, false, Action::MakeCode, true),
        (Role::Editor, false, Action::MakeCode, false),
        (Role::Viewer, false, Action::AddBank, false),
        (Role::Editor, false, Action::AddBank, true),
        (Role::Owner, true, Action::AddBank, false),
        (Role::Owner, true, Action::Reopen, true),
        (Role::Editor, true, Action::Reopen, false),
        (Role::Viewer, false, Action::Leave, true),
        (Role::Owner, false, Action::Leave, true),
    ] {
        assert_eq!(can(role, closed, a), want, "{role:?} closed={closed} {a:?}");
    }
}

#[test]
fn a_code_is_made_shown_once_listed_and_withdrawn() {
    let (mut m, s) = open_circle("owner", json!({}));
    m.begin_code();
    if let Panel::NewCode(f) = &mut m.panel {
        f.role = Role::Viewer;
        f.people = 3;
        f.days = 14;
        f.note = "for the new hires".into();
    } else {
        panic!("{:?}", m.panel);
    }
    s.act("Post", "/invites", 201, json!({"invite_id": "inv1", "code": CODE, "role": "viewer", "max_uses": 3, "expires_at": NOW + 14 * DAY, "note": "for the new hires"}));
    s.act("Get", "/invites", 200, json!({"invites": [{"invite_id": "inv1", "role": "viewer", "created_at": NOW, "created_by": "u-owner", "created_by_name": "Ms Frizzle", "max_uses": 3, "uses": 0, "expires_at": NOW + 14 * DAY, "note": "for the new hires", "state": "active"}], "next_cursor": null}));
    m.make_code();
    m.frame(now(), &ready(&s));
    m.frame(now(), &ready(&s));
    let post = s.seen().into_iter().find(|x| x.path.ends_with("/invites") && x.method == Method::Post).unwrap();
    assert_eq!(post.json()["role"], "viewer");
    assert_eq!(post.json()["max_uses"], 3);
    assert!(matches!(&m.panel, Panel::Code(c) if c.role == Role::Viewer));
    let dbg = format!("{:?}", m.panel);
    assert!(!dbg.contains("ABCDEFGH"), "the code is not in Debug output");
    m.done_with_code();
    assert!(matches!(m.panel, Panel::None));
    assert_eq!(m.circle.as_ref().unwrap().invites.as_ref().unwrap().len(), 1);
    s.act("Post", "/invites/revoke", 200, json!({"ok": true}));
    m.withdraw_code("inv1");
    m.frame(now(), &ready(&s));
    m.frame(now(), &ready(&s));
    let rev = s.seen().into_iter().find(|x| x.path.ends_with("/invites/revoke")).unwrap();
    assert_eq!(rev.json(), json!({"invite_id": "inv1"}));
}

#[test]
fn an_invite_for_the_owner_role_or_an_editor_is_not_possible() {
    let (mut m, s) = open_circle("editor", json!({}));
    let n = s.seen().len();
    m.begin_code();
    m.make_code();
    assert!(matches!(m.panel, Panel::None));
    assert_eq!(s.seen().len(), n);
}

#[test]
fn leaving_removing_and_changing_roles() {
    let (mut m, s) = open_circle("owner", json!({}));
    s.act("Post", "/members/remove", 200, json!({"ok": true}));
    m.ask_remove("u-view");
    assert!(matches!(m.panel, Panel::ConfirmRemove(_)));
    m.remove_member();
    for _ in 0..4 {
        m.frame(now(), &ready(&s));
    }
    let rm = s.seen().into_iter().find(|x| x.path.ends_with("/members/remove")).unwrap();
    assert_eq!(rm.json(), json!({"user_id": "u-view"}));
    s.act("Post", "/members/role", 200, json!({"ok": true, "changed": true}));
    m.set_role("u-view", Role::Editor);
    for _ in 0..4 {
        m.frame(now(), &ready(&s));
    }
    let rl = s.seen().into_iter().find(|x| x.path.ends_with("/members/role")).unwrap();
    assert_eq!(rl.json(), json!({"user_id": "u-view", "role": "editor"}));
    // leaving: the list again
    s.act("Post", "/leave", 200, json!({"ok": true}));
    s.keep("Get", "/circles", 200, json!({"circles": []}));
    m.ask_leave();
    m.leave();
    for _ in 0..3 {
        m.frame(now(), &ready(&s));
    }
    assert!(s.paths().iter().any(|p| p.ends_with("/leave")));
    assert_eq!(m.screen, Screen::List);
}

#[test]
fn the_owner_closing_the_circle_gets_the_fourteen_day_undo_and_can_reopen() {
    let (mut m, s) = open_circle("owner", json!({}));
    s.act("Delete", "", 200, json!({"ok": true, "undo_until": NOW + 14 * DAY}));
    let mut closed = detail("owner", json!({}));
    closed["circle"]["closed_at"] = json!(NOW);
    closed["circle"]["undo_until"] = json!(NOW + 14 * DAY);
    closed["circle"]["close_reason"] = json!("owner");
    s.act("Get", "", 200, closed.clone());
    m.ask_close();
    m.close_the_circle();
    for _ in 0..4 {
        m.frame(now(), &ready(&s));
    }
    assert_eq!(s.seen().iter().filter(|x| x.method == Method::Delete).count(), 1);
    assert!(m.circle.as_ref().unwrap().closed());
    assert!(!can(Role::Owner, true, Action::AddBank));
    s.act("Post", "/reopen", 200, json!({"ok": true}));
    m.reopen();
    for _ in 0..4 {
        m.frame(now(), &ready(&s));
    }
    assert!(s.paths().iter().any(|p| p.ends_with("/reopen")));
    assert!(!m.circle.as_ref().unwrap().closed());
}

#[test]
fn every_act_error_has_plain_words() {
    for (status, body) in [
        (403, err("circle_role_required")),
        (409, err("circle_closed")),
        (409, err("member_unavailable")),
        (409, err("no_pending_transfer")),
        (404, err("not_found")),
        (409, err("already_revoked")),
        (404, err("circle_invite_invalid")),
        (409, err("circle_full")),
        (410, err("bank_removed")),
        (409, err("version_not_retained")),
        (422, json!({"error": "circle_questions_limit", "limit": 5000})),
        (422, err("circle_records_full")),
        (413, json!({"error": "quota_exceeded", "scope": "circle", "used": 90, "limit": 100})),
        (409, err("commit_id_conflict")),
        (422, json!({"error": "circles_limit", "metric": "circles_owned", "used": 5, "limit": 5})),
        (429, json!({"error": "daily_limit", "retry_after": 60})),
    ] {
        let (mut m, s) = open_circle("owner", json!({}));
        s.act("Post", "/members/remove", status, body.clone());
        m.ask_remove("u-view");
        m.remove_member();
        m.frame(now(), &ready(&s));
        let p = if problem(&m).is_empty() { m.notice.as_ref().map(|n| format!("{n:?}")).unwrap_or_default() } else { problem(&m) };
        assert!(!p.is_empty() && !p.contains("circle_") && !p.contains("_required") && !p.contains("bank_removed") && !p.contains("version_not"), "{body}: {p}");
    }
}

// ---- ownership offers (round 34) ------------------------------------------------------------------------------

#[test]
fn an_ownership_transfer_is_an_offer_the_owner_keeps_the_circle_until_it_is_accepted() {
    let (mut m, s) = open_circle("owner", json!({}));
    s.act("Post", "/transfer", 202, json!({"ok": true, "pending": {"to_user_id": "u-ed", "to_name": "Mr Keesler", "expires_at": NOW + 7 * DAY}}));
    s.act("Get", "", 200, detail("owner", json!({"pending_transfer": {"to_user_id": "u-ed", "to_name": "Mr Keesler", "expires_at": NOW + 7 * DAY}})));
    m.ask_transfer("u-ed");
    m.offer_ownership();
    for _ in 0..4 {
        m.frame(now(), &ready(&s));
    }
    let t = s.seen().into_iter().find(|x| x.path.ends_with("/transfer")).unwrap();
    assert_eq!(t.json(), json!({"user_id": "u-ed"}));
    assert!(m.can_withdraw_offer() && !m.offered_to_me());
    assert_eq!(m.circle.as_ref().unwrap().role(), Role::Owner, "still the owner");
    s.act("Post", "/transfer/cancel", 200, json!({"ok": true, "cancelled": true}));
    s.act("Get", "", 200, detail("owner", json!({})));
    m.cancel_offer();
    for _ in 0..4 {
        m.frame(now(), &ready(&s));
    }
    assert!(s.paths().iter().any(|p| p.ends_with("/transfer/cancel")));
    assert!(m.pending_offer().is_none());
}

#[test]
fn the_offered_member_can_accept_or_decline_and_nobody_else_can() {
    let offer = json!({"pending_transfer": {"to_user_id": "u-ed", "to_name": "Mr Keesler", "expires_at": NOW + 7 * DAY}});
    let (mut m, s) = open_circle("editor", offer.clone());
    assert!(m.offered_to_me() && !m.can_withdraw_offer());
    let (_, _) = (&m, &s);
    s.act("Post", "/transfer/accept", 200, json!({"ok": true, "role": "owner"}));
    s.act("Get", "", 200, detail("owner", json!({})));
    m.accept_offer();
    for _ in 0..4 {
        m.frame(now(), &ready(&s));
    }
    assert!(s.paths().iter().any(|p| p.ends_with("/transfer/accept")));
    assert_eq!(m.circle.as_ref().unwrap().role(), Role::Owner);
    // someone the offer is not for
    let (mut m, s) = open_circle("viewer", offer);
    assert!(!m.offered_to_me() && !m.can_withdraw_offer());
    let n = s.seen().len();
    m.accept_offer();
    m.cancel_offer();
    assert_eq!(s.seen().len(), n);
}

#[test]
fn accepting_when_the_limit_is_reached_or_the_offer_is_gone_says_why() {
    let offer = json!({"pending_transfer": {"to_user_id": "u-ed", "to_name": "Mr Keesler", "expires_at": NOW + 7 * DAY}});
    let (mut m, s) = open_circle("editor", offer.clone());
    s.act("Post", "/transfer/accept", 422, json!({"error": "circles_limit", "metric": "circles_owned", "used": 5, "limit": 5}));
    m.accept_offer();
    m.frame(now(), &ready(&s));
    assert!(problem(&m).contains("14 days"), "{}", problem(&m));
    let (mut m, s) = open_circle("editor", offer);
    s.act("Post", "/transfer/accept", 409, err("no_pending_transfer"));
    m.accept_offer();
    m.frame(now(), &ready(&s));
    assert!(problem(&m).contains("offer"), "{}", problem(&m));
}

#[test]
fn the_list_marks_a_circle_offered_to_me() {
    let s = Script::new();
    s.say(200, json!({"circles": [summary("editor", json!({"ownership_offered_until": NOW + DAY}))]}));
    let mut m = model();
    m.open_list();
    m.frame(now(), &ready(&s));
    m.frame(now(), &ready(&s));
    assert!(m.list.items[0].ownership_offered_until.is_some());
}

// ---- banks, copy ---------------------------------------------------------------------------------------------------

#[test]
fn a_bank_is_read_once_and_shown() {
    let (mut m, s) = open_circle("viewer", json!({}));
    s.say_bytes(bank_body(&["First", "Second"]));
    m.open_bank(BID);
    m.frame(now(), &ready(&s));
    m.frame(now(), &ready(&s));
    assert_eq!(m.screen, Screen::Bank);
    let b = m.bank.as_ref().unwrap();
    assert!(matches!(&b.load, BankLoad::Ready(l) if l.doc.bank.question_count == 2));
    assert_eq!(s.last().path, format!("/circles/{CID}/banks/{BID}"));
}

#[test]
fn a_copy_makes_a_new_bank_and_is_recorded_like_an_import() {
    let (mut m, s) = open_circle("viewer", json!({}));
    s.say_bytes(bank_body(&["First", "Second"]));
    m.open_bank(BID);
    m.frame(now(), &ready(&s));
    m.frame(now(), &ready(&s));
    m.begin_copy();
    assert!(matches!(m.bank.as_ref().unwrap().copy, CopyPanel::Choosing { .. }));
    m.get_copy();
    m.frame(now(), &ready(&s));
    let mut banks: Vec<QuestionBank> = Vec::new();
    let mut browse = BrowseModel::new(Runner::Inline, Arc::new(Mem::default()), ImportLog::memory());
    let changed = m.sync(&mut banks, &mut browse);
    assert!(changed);
    assert_eq!(banks.len(), 1);
    assert_eq!(banks[0].questions.len(), 2);
    assert!(banks[0].details.description.contains("Biology team"), "{}", banks[0].details.description);
    assert_eq!(browse.circle_bank_ids(), vec![BID.to_string()]);
    assert!(matches!(m.bank.as_ref().unwrap().copy, CopyPanel::Done(_)));
    // the same sync again changes nothing
    assert!(!m.sync(&mut banks, &mut browse));
    assert_eq!(banks.len(), 1);
}

#[test]
fn a_viewer_may_copy_but_not_remove_and_a_closed_circle_may_not_be_changed() {
    let b = BankEntry { id: BID.into(), is_author: false, ..Default::default() };
    assert!(bank_can(Role::Viewer, false, &b, BankAction::Copy));
    assert!(!bank_can(Role::Viewer, false, &b, BankAction::Remove));
    assert!(bank_can(Role::Owner, false, &b, BankAction::Remove));
    assert!(!bank_can(Role::Owner, true, &b, BankAction::Remove));
}

#[test]
fn the_other_bank_routes_send_what_the_pool_expects() {
    let bank = json!({"id": BID, "title": "Cells", "license": "CC-BY-4.0", "author_name": "Me", "is_author": true, "head_version": 1, "question_count": 1});
    let (mut m, s) = open_circle("owner", json!({"banks": [bank]}));
    s.keep_raw("Get", &format!("/circles/{CID}/banks/{BID}"), bank_body(&["First"]));
    m.open_bank(BID);
    m.frame(now(), &ready(&s));
    m.frame(now(), &ready(&s));
    s.act("Patch", &format!("/banks/{BID}"), 200, json!({"ok": true}));
    m.set_bank_license(License::CcBy);
    m.frame(now(), &ready(&s));
    m.frame(now(), &ready(&s));
    m.frame(now(), &ready(&s));
    let p = s.seen().into_iter().find(|x| x.method == Method::Patch && x.path.contains("/banks/")).unwrap();
    assert!(p.json()["license"].is_string());
    s.act("Delete", &format!("/banks/{BID}"), 200, json!({"ok": true, "undo_until": NOW + 14 * DAY}));
    m.ask_remove_bank();
    m.remove_bank();
    m.frame(now(), &ready(&s));
    m.frame(now(), &ready(&s));
    m.frame(now(), &ready(&s));
    assert!(s.seen().iter().any(|x| x.method == Method::Delete && x.path.contains("/banks/")));
}

#[test]
fn a_removed_bank_is_brought_back_by_its_author_or_the_owner() {
    let bank = json!({"id": BID, "title": "Cells", "license": "CC-BY-4.0", "author_name": "Me", "is_author": true, "head_version": 1, "question_count": 1, "removed": {"reason": "author_removed", "undo_until": NOW + DAY}});
    let (mut m, s) = open_circle("owner", json!({"banks": [bank]}));
    s.act("Post", &format!("/banks/{BID}/undelete"), 200, json!({"ok": true}));
    m.restore_bank(BID);
    m.frame(now(), &ready(&s));
    assert!(s.paths().iter().any(|p| p.ends_with("/undelete")));
}

#[test]
fn a_report_goes_with_a_reason_and_a_note() {
    let (mut m, s) = open_circle("viewer", json!({}));
    s.say_bytes(bank_body(&["First"]));
    m.open_bank(BID);
    m.frame(now(), &ready(&s));
    m.frame(now(), &ready(&s));
    m.begin_report();
    s.act("Post", &format!("/banks/{BID}/report"), 200, json!({"ok": true}));
    m.send_report();
    m.frame(now(), &ready(&s));
    assert!(s.paths().iter().any(|p| p.ends_with("/report")));
}

#[test]
fn heads_give_the_update_badges_for_copies_and_skip_other_banks() {
    let s = Script::new();
    s.keep("Get", "/circles", 200, json!({"circles": []}));
    s.on("Post", "/circles/heads", 200, json!({"banks": [{"bank_id": BID, "status": "current", "version": 3, "license": "CC-BY-4.0", "at": NOW, "by": "Mr Keesler"}]}));
    let mut m = model();
    m.open_list();
    m.frame(now(), &ready(&s));
    m.want_heads(vec![BID.to_string()]);
    m.frame(now(), &ready(&s));
    m.frame(now(), &ready(&s));
    let h = m.take_heads().expect("heads");
    assert_eq!(h.len(), 1);
    assert_eq!(s.last().path, "/circles/heads");
    assert_eq!(s.last().json()["bank_ids"], json!([BID]));
}

// ---- share to a circle (publish window) -----------------------------------------------------------------------

fn bank_src() -> ShareSource {
    let b = json!({"id": "b1", "name": "Unit 1", "questions": [{"id": "q0", "stem": "Question text 0", "image": null, "kind": {"Mc": {"answers": [{"text": "Alpha", "correct": true}, {"text": "Beta", "correct": false}]}}}], "created": 1, "modified": 2});
    ShareSource::from_bank_json("bank:b1", "Bank: Unit 1", "Unit 1", &b.to_string())
}

fn publisher(s: &Arc<Script>) -> PublishModel {
    let mut p = PublishModel::new(Runner::Inline, Arc::new(HashMap::<String, Vec<u8>>::new()), LogIo::memory()).with_retry(RetryPolicy::none());
    p.start(bank_src());
    p.frame(now(), &ready(s));
    assert_eq!(p.step, Step::Options);
    p
}

fn to_circle_ready(s: &Arc<Script>, role: &str) -> PublishModel {
    let mut p = publisher(s);
    s.keep("Get", "/circles", 200, json!({"circles": [summary(role, json!({}))]}));
    p.circle.activate(None);
    p.frame(now(), &ready(s));
    p.frame(now(), &ready(s));
    assert!(matches!(p.circle.list, CircleList::Ready(_)), "{:?}", p.circle.list);
    p
}

#[test]
fn nothing_is_sent_to_a_circle_until_circle_licence_and_the_own_work_box_are_chosen() {
    let s = Script::new();
    let mut p = to_circle_ready(&s, "editor");
    let n = s.seen().len();
    p.send_to_circle();
    assert_eq!(s.seen().len(), n);
    assert!(p.circle.blocker().is_some());
    p.circle.choose(CID);
    p.send_to_circle();
    assert_eq!(s.seen().len(), n, "no licence yet");
    p.circle.license = Some(License::CcBy);
    p.send_to_circle();
    assert_eq!(s.seen().len(), n, "not attested");
}

#[test]
fn a_viewer_can_not_pick_a_circle_to_add_to() {
    let s = Script::new();
    let p = to_circle_ready(&s, "viewer");
    assert!(p.circle.choices().is_empty());
}

#[test]
fn a_bank_is_sent_to_the_chosen_circle_with_its_licence_and_the_note() {
    let s = Script::new();
    let mut p = to_circle_ready(&s, "editor");
    p.circle.choose(CID);
    p.circle.license = Some(License::CcBy);
    p.circle.attest = true;
    p.circle.message = "for unit 1".into();
    s.act("Post", "/banks", 201, json!({"circle_id": CID, "bank_id": BID, "version": 1, "version_id": "v1", "qids": ["a"], "added": 1, "changed": 0, "removed": 0, "replayed": false}));
    p.send_to_circle();
    p.frame(now(), &ready(&s));
    let post = s.last();
    assert_eq!(post.path, format!("/circles/{CID}/banks"));
    let head = String::from_utf8_lossy(&post.body).lines().next().unwrap_or_default().to_string();
    assert!(head.contains("CC-BY-4.0") && head.contains("for unit 1"), "{head}");
    assert!(matches!(&p.circle.stage, SendStage::Done(i) if i.sent.added == 1), "{:?}", p.circle.stage);
}

#[test]
fn a_confirmed_pending_editor_sends_to_a_circle() {
    // Round 54: the circle-send button must not stop at the gate for a confirmed pending account.
    let s = Script::new();
    s.keep("Get", "/circles", 200, json!({"circles": [summary("editor", json!({}))]}));
    let mut p = PublishModel::new(Runner::Inline, Arc::new(HashMap::<String, Vec<u8>>::new()), LogIo::memory()).with_retry(RetryPolicy::none());
    p.start_for_circle(bank_src(), Some(CID.into()));
    p.frame(now(), &pending(&s));
    p.frame(now(), &pending(&s));
    assert_eq!(p.step, Step::Options);
    p.circle.choose(CID);
    p.circle.license = Some(License::CcBy);
    p.circle.attest = true;
    s.act("Post", "/banks", 201, json!({"circle_id": CID, "bank_id": BID, "version": 1, "version_id": "v1", "qids": ["a"], "added": 1, "changed": 0, "removed": 0, "replayed": false}));
    p.send_to_circle();
    p.frame(now(), &pending(&s));
    assert_eq!(s.last().path, format!("/circles/{CID}/banks"), "{:?}", p.notice);
    assert!(matches!(&p.circle.stage, SendStage::Done(i) if i.sent.added == 1), "{:?}", p.circle.stage);
}

#[test]
fn a_lost_answer_is_retried_as_the_same_send() {
    let s = Script::new();
    let mut p = to_circle_ready(&s, "editor");
    p.circle.choose(CID);
    p.circle.license = Some(License::CcBy);
    p.circle.attest = true;
    s.fail(ApiError::Offline("lost".into()));
    p.send_to_circle();
    p.frame(now(), &ready(&s));
    assert!(matches!(p.circle.stage, SendStage::Failed(_)));
    let first = String::from_utf8_lossy(&s.last().body).to_string();
    s.act("Post", "/banks", 200, json!({"circle_id": CID, "bank_id": BID, "version": 1, "version_id": "v1", "qids": ["a"], "added": 1, "changed": 0, "removed": 0, "replayed": true}));
    p.circle.back_to_editing();
    p.send_to_circle();
    p.frame(now(), &ready(&s));
    assert_eq!(String::from_utf8_lossy(&s.last().body), first, "same commit id and bytes");
    assert!(matches!(&p.circle.stage, SendStage::Done(i) if i.sent.replayed));
}

#[test]
fn a_circle_send_refused_by_the_checks_shows_what_to_change() {
    let s = Script::new();
    let mut p = to_circle_ready(&s, "editor");
    p.circle.choose(CID);
    p.circle.license = Some(License::CcBy);
    p.circle.attest = true;
    s.act("Post", "/banks", 422, json!({"error": "invalid_payload", "errors": [{"code": "too_long", "path": "meta.title", "limit": 200}]}));
    p.send_to_circle();
    p.frame(now(), &ready(&s));
    assert!(matches!(p.circle.stage, SendStage::Refused(Refused::Payload(_))), "{:?}", p.circle.stage);
}

#[test]
fn a_session_that_ends_during_a_circle_send_goes_back_to_sign_in() {
    let s = Script::new();
    let mut p = to_circle_ready(&s, "editor");
    p.circle.choose(CID);
    p.circle.license = Some(License::CcBy);
    p.circle.attest = true;
    s.fail(ApiError::SignedOut);
    p.send_to_circle();
    p.frame(now(), &ready(&s));
    assert_eq!(p.step, Step::Gate);
}

fn material(hits: Value) -> Value {
    json!({"error": "circle_material", "positions": [0], "hits": hits})
}

fn public_ready(s: &Arc<Script>) -> PublishModel {
    let mut p = publisher(s);
    p.form.license = Some(License::CcBy);
    p.form.attest = true;
    p
}

#[test]
fn a_public_publish_that_meets_circle_material_offers_the_statement_way_out() {
    let s = Script::new();
    let mut p = public_ready(&s);
    s.say(422, material(json!([{"pos": 0, "circle_id": CID, "circle_name": "Biology team", "bank_id": BID, "bank_title": "Cells", "may_claim_independence": true}])));
    p.check();
    p.frame(now(), &ready(&s));
    assert_eq!(p.step, Step::Fix);
    assert!(matches!(p.fix, Some(super::publish::Fix::CircleMaterial(_))));
    // too short a statement is not sent
    let n = s.seen().len();
    p.form.circle_independent_statement = "short".into();
    p.claim_circle_independence();
    assert_eq!(s.seen().len(), n);
    assert!(matches!(p.fix, Some(super::publish::Fix::CircleMaterial(_))));
    // a real one goes with the next check
    p.form.circle_independent_statement = "I wrote these questions myself from my own lessons.".into();
    s.say(422, material(json!([{"pos": 0, "circle_id": CID, "circle_name": "Biology team", "bank_id": BID, "bank_title": "Cells", "may_claim_independence": true}])));
    p.claim_circle_independence();
    p.frame(now(), &ready(&s));
    let text = String::from_utf8_lossy(&s.last().body).to_string();
    let body: Value = serde_json::from_str(text.lines().next().unwrap_or("")).unwrap_or(Value::Null);
    let meta = &body["circle_independent_of"];
    assert_eq!(meta["banks"], json!([BID]), "{body}");
    assert!(meta["statement"].as_str().unwrap().len() >= 10);
}

#[test]
fn a_hit_that_can_not_be_lifted_or_has_no_names_offers_no_statement() {
    let s = Script::new();
    let mut p = public_ready(&s);
    s.say(422, material(json!([{"pos": 0, "circle_id": null, "circle_name": null, "bank_id": null, "bank_title": null, "may_claim_independence": false}])));
    p.check();
    p.frame(now(), &ready(&s));
    assert_eq!(p.step, Step::Fix);
    let n = s.seen().len();
    p.form.circle_independent_statement = "I wrote these questions myself from my own lessons.".into();
    p.claim_circle_independence();
    assert_eq!(s.seen().len(), n, "nothing is sent for a hit the pool says a statement can not lift");
}

// ---- smoke frames -------------------------------------------------------------------------------------------------

pub(super) fn frame(ctx: &egui::Context, width: f32, f: &mut dyn FnMut(&mut egui::Ui)) {
    let input = egui::RawInput { screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(width, 900.0))), time: Some(0.0), ..Default::default() };
    let out = ctx.run_ui(input, |ui| {
        egui::CentralPanel::default().show(ui, |ui| f(ui));
    });
    out.drop_without_applying_deltas();
}

#[test]
fn every_page_and_state_draws_wide_and_narrow() {
    let ctx = egui::Context::default();
    crate::theme::setup(&ctx);
    for width in [1000.0, 420.0] {
        // gate and empty list
        let mut m = model();
        m.open_list();
        frame(&ctx, width, &mut |ui| {
            super::circles_ui::body_of(ui, &mut m);
        });
        let s = Script::new();
        s.say(200, json!({"circles": []}));
        m.frame(now(), &ready(&s));
        m.frame(now(), &ready(&s));
        frame(&ctx, width, &mut |ui| {
            super::circles_ui::body_of(ui, &mut m);
        });
        m.start_create();
        frame(&ctx, width, &mut |ui| {
            super::circles_ui::body_of(ui, &mut m);
        });
        m.start_join();
        m.join.text = "C-ABCDEFGHJKMNPQRSTVWXYZ01".into();
        frame(&ctx, width, &mut |ui| {
            super::circles_ui::body_of(ui, &mut m);
        });
        // every circle state
        for (role, extra) in [
            ("owner", json!({})),
            ("editor", json!({})),
            ("viewer", json!({})),
            ("owner", json!({"pending_transfer": {"to_user_id": "u-ed", "to_name": "Mr Keesler", "expires_at": NOW + 7 * DAY}})),
            ("editor", json!({"pending_transfer": {"to_user_id": "u-ed", "to_name": "Mr Keesler", "expires_at": NOW + 7 * DAY}})),
        ] {
            let (mut m, s) = open_circle(role, extra);
            frame(&ctx, width, &mut |ui| {
                super::circles_ui::body_of(ui, &mut m);
            });
            if role == "owner" {
                m.begin_code();
                frame(&ctx, width, &mut |ui| {
                    super::circles_ui::body_of(ui, &mut m);
                });
                m.cancel_panel();
                m.ask_close();
                frame(&ctx, width, &mut |ui| {
                    super::circles_ui::body_of(ui, &mut m);
                });
                m.cancel_panel();
                m.ask_transfer("u-ed");
                frame(&ctx, width, &mut |ui| {
                    super::circles_ui::body_of(ui, &mut m);
                });
                m.cancel_panel();
            }
            s.say_bytes(bank_body(&["First", "Second"]));
            m.open_bank(BID);
            m.frame(now(), &ready(&s));
            m.frame(now(), &ready(&s));
            frame(&ctx, width, &mut |ui| {
                super::circles_ui::body_of(ui, &mut m);
            });
            m.begin_copy();
            frame(&ctx, width, &mut |ui| {
                super::circles_ui::body_of(ui, &mut m);
            });
            m.begin_report();
            frame(&ctx, width, &mut |ui| {
                super::circles_ui::body_of(ui, &mut m);
            });
        }
        // a closed circle
        let mut closed = json!({});
        closed["circle"] = json!({"id": CID, "name": "Biology team", "description": "", "role": "owner", "joined_at": NOW, "created_at": NOW, "updated_at": NOW, "closed_at": NOW, "undo_until": NOW + DAY, "close_reason": "owner"});
        let (mut m, _) = open_circle("owner", closed);
        frame(&ctx, width, &mut |ui| {
            super::circles_ui::body_of(ui, &mut m);
        });
    }
}

#[test]
fn the_publish_window_draws_both_destinations_and_the_circle_refusal() {
    let ctx = egui::Context::default();
    crate::theme::setup(&ctx);
    for width in [700.0, 420.0] {
        let s = Script::new();
        let mut p = to_circle_ready(&s, "editor");
        frame(&ctx, width, &mut |ui| {
            super::publish_ui::page(ui, &mut p);
        });
        p.circle.choose(CID);
        frame(&ctx, width, &mut |ui| {
            super::publish_ui::page(ui, &mut p);
        });
        p.circle.deactivate();
        frame(&ctx, width, &mut |ui| {
            super::publish_ui::page(ui, &mut p);
        });
        let s = Script::new();
        let mut p = public_ready(&s);
        s.say(422, material(json!([{"pos": 0, "circle_id": CID, "circle_name": "Biology team", "bank_id": BID, "bank_title": "Cells", "may_claim_independence": true}, {"pos": 1, "circle_id": null, "bank_id": null, "may_claim_independence": false}])));
        p.check();
        p.frame(now(), &ready(&s));
        frame(&ctx, width, &mut |ui| {
            super::publish_ui::page(ui, &mut p);
        });
    }
}

#[test]
fn a_rename_and_the_paging_of_invites_and_activity_use_their_routes() {
    let (mut m, s) = open_circle("owner", json!({}));
    s.act("Patch", "", 200, json!({"ok": true}));
    m.begin_rename();
    if let Panel::Rename { name } = &mut m.panel {
        *name = "Chemistry team".into();
    }
    m.rename();
    m.frame(now(), &ready(&s));
    let p = s.seen().into_iter().find(|x| x.method == Method::Patch).unwrap();
    assert_eq!(p.json()["name"], "Chemistry team");
    // more activity follows the cursor
    s.act("Get", "/activity", 200, json!({"activity": [{"at": NOW, "what": "member_joined", "member": "Ms Viewer"}], "next_before": "123:abc"}));
    s.act("Get", "/activity", 200, json!({"activity": [], "next_before": null}));
    m.circle.as_mut().unwrap().activity_loaded = false;
    m.frame(now(), &ready(&s));
    m.frame(now(), &ready(&s));
    m.more_activity();
    m.frame(now(), &ready(&s));
    assert!(s.paths().iter().any(|p| p.contains("/activity?before=123:abc")), "{:?}", s.paths());
}

#[test]
fn a_blob_is_checked_against_its_name() {
    use super::pool::sha256_hex;
    let s = Script::new();
    let data = b"picture bytes".to_vec();
    let name = sha256_hex(&data);
    s.say_bytes(data.clone());
    let sess: Arc<Script> = s.clone();
    let blob = super::transport::block_on(async { Pool::new(sess.as_ref()).circle_blob(CID, BID, &name).await });
    assert_eq!(blob.unwrap().bytes, data);
    s.say_bytes(b"other".to_vec());
    let bad = super::transport::block_on(async { Pool::new(sess.as_ref()).circle_blob(CID, BID, &name).await });
    assert!(bad.is_err(), "bytes that do not match their name are refused");
}

// ---- critic 43 ---------------------------------------------------------------------------------------------------

/// The owner's page with one active code in the list.
fn open_owner_with_code() -> (CirclesModel, Arc<Script>) {
    let (mut m, s) = open_circle("owner", json!({}));
    s.keep("Get", &format!("/circles/{CID}/invites"), 200, json!({"invites": [{"invite_id": "inv1", "role": "viewer", "created_at": NOW, "created_by": "u-owner", "created_by_name": "Ms Frizzle", "max_uses": 3, "uses": 0, "expires_at": NOW + 14 * DAY, "note": null, "state": "active"}], "next_cursor": null}));
    m.circle.as_mut().unwrap().invites = None;
    for _ in 0..3 {
        m.frame(now(), &ready(&s));
    }
    (m, s)
}

fn masked_bank(reason: &str, undo: Value) -> Value {
    json!({"id": BID, "title": null, "subject": null, "course": null, "grade": null, "license": "CC-BY-4.0", "author_name": null, "is_author": false, "head_version": 1, "question_count": 2, "has_images": false,
           "created_at": NOW - DAY, "updated_at": NOW - DAY, "hidden": true, "removed": {"reason": reason, "undo_until": undo}})
}

pub(super) fn removed_bank(reason: &str, is_author: bool, undo: Value) -> Value {
    json!({"id": BID, "title": "Cells", "subject": "Biology", "license": "CC-BY-4.0", "author_name": "Mr Keesler", "is_author": is_author, "head_version": 1, "question_count": 2,
           "created_at": NOW - DAY, "updated_at": NOW - DAY, "removed": {"reason": reason, "undo_until": undo}})
}

#[test]
fn c43_b1_a_masked_bank_with_nulls_still_opens_the_owner_page() {
    let (m, _s) = open_circle("owner", json!({"banks": [masked_bank("account_deleted", Value::Null)]}));
    let c = m.circle.as_ref().unwrap();
    let d = c.detail.as_ref().unwrap_or_else(|| panic!("owner page failed: {:?}", c.problem.as_ref().map(|p| &p.message)));
    assert!(d.banks[0].hidden && d.banks[0].title.is_empty());
}

#[test]
fn c43_b1_the_hidden_bank_offers_only_what_the_server_allows() {
    // an erasure removal can be purged but never brought back; an owner removal can be both
    let (m, _) = open_circle("owner", json!({"banks": [masked_bank("account_deleted", Value::Null)]}));
    let b = &m.circle.as_ref().unwrap().detail.as_ref().unwrap().banks[0];
    assert!(!bank_can(Role::Owner, false, b, BankAction::Restore));
    assert!(bank_can(Role::Owner, false, b, BankAction::Purge));
    let (m, _) = open_circle("owner", json!({"banks": [masked_bank("admin", Value::Null)]}));
    let b = &m.circle.as_ref().unwrap().detail.as_ref().unwrap().banks[0];
    assert!(!bank_can(Role::Owner, false, b, BankAction::Purge), "an admin removal is under review");
    let (m, _) = open_circle("owner", json!({"banks": [masked_bank("owner_removed", json!(NOW + DAY))]}));
    let b = &m.circle.as_ref().unwrap().detail.as_ref().unwrap().banks[0];
    assert!(bank_can(Role::Owner, false, b, BankAction::Restore) && bank_can(Role::Owner, false, b, BankAction::Purge));
}

#[test]
fn c43_b2_restore_is_offered_only_where_the_server_can_do_it() {
    let b = |reason: &str, author: bool| serde_json::from_value::<BankEntry>(removed_bank(reason, author, json!(NOW + DAY))).unwrap();
    assert!(bank_can(Role::Owner, false, &b("owner_removed", false), BankAction::Restore));
    assert!(bank_can(Role::Owner, false, &b("author_removed", false), BankAction::Restore));
    assert!(bank_can(Role::Editor, false, &b("author_removed", true), BankAction::Restore));
    assert!(!bank_can(Role::Editor, false, &b("owner_removed", true), BankAction::Restore), "the author can not undo the owner's removal");
    assert!(!bank_can(Role::Owner, false, &b("account_deleted", false), BankAction::Restore));
    assert!(!bank_can(Role::Owner, false, &b("admin", false), BankAction::Restore));
    assert!(!bank_can(Role::Owner, false, &b("circle_closed", false), BankAction::Restore));
}

#[test]
fn c43_b2_a_failed_restore_is_shown_on_the_circle_page() {
    let (mut m, s) = open_circle("owner", json!({"banks": [removed_bank("owner_removed", false, json!(NOW + DAY))]}));
    s.act("Post", &format!("/banks/{BID}/undelete"), 409, json!({"error": "under_moderation"}));
    m.restore_bank(BID);
    m.frame(now(), &ready(&s));
    m.frame(now(), &ready(&s));
    assert!(!problem(&m).is_empty(), "restore failed and nothing was shown");
}

#[test]
fn c43_b3_a_session_that_ends_mid_action_frees_create_copy_and_report() {
    let s = Script::new();
    s.keep("Get", "/circles", 200, json!({"circles": []}));
    let mut m = model();
    m.open_list();
    m.frame(now(), &ready(&s));
    m.start_create();
    m.create.name = "Team".into();
    s.fail(ApiError::SignedOut);
    m.create_circle();
    m.frame(now(), &ready(&s));
    assert!(!m.create.working, "create stuck");
    assert_eq!(m.gate, Gate::SignedOut);
    assert!(m.notice.is_some(), "the person is told");

    let (mut m, s) = open_circle("viewer", json!({}));
    // A picture, so that the copy has something to fetch (and the session can end while it does).
    let with_picture = String::from_utf8(bank_body(&["First", "Second"])).unwrap().replacen("\"keyless\":false", &format!("\"image\":{{\"file\":\"media/{}.png\",\"height\":2,\"width\":2}},\"keyless\":false", "ab".repeat(32)), 1);
    s.keep_raw("Get", &format!("/circles/{CID}/banks/{BID}"), with_picture.into_bytes());
    m.open_bank(BID);
    m.frame(now(), &ready(&s));
    m.frame(now(), &ready(&s));
    m.begin_copy();
    s.fail(ApiError::SignedOut);
    m.get_copy();
    m.frame(now(), &ready(&s));
    assert!(!matches!(m.bank.as_ref().unwrap().copy, CopyPanel::Working), "copy stuck");

    let (mut m, s) = open_circle("viewer", json!({}));
    s.keep_raw("Get", &format!("/circles/{CID}/banks/{BID}"), bank_body(&["First"]));
    m.open_bank(BID);
    m.frame(now(), &ready(&s));
    m.frame(now(), &ready(&s));
    m.begin_report();
    s.fail(ApiError::SignedOut);
    m.send_report();
    m.frame(now(), &ready(&s));
    assert!(matches!(m.bank.as_ref().unwrap().report.as_ref().unwrap().state, ReportState::Editing), "report stuck");
}

#[test]
fn c43_b4_nothing_promises_public_re_sharing() {
    for l in ["circle-only", "cc0", "CC0-1.0", "CC-BY-4.0", "CC-BY-NC-4.0", "CC-BY-SA-4.0"] {
        let t = super::circles_text::reshare_line(l);
        assert!(!t.to_lowercase().contains("may share it publicly") && !t.contains("Free to use"), "{l}: {t}");
    }
    for t in [super::circles_text::LICENSE_SETTING_NOTE, super::circle_send::RESHARE_NOTE] {
        assert!(t.contains("not available") || t.contains("Not yet") || t.contains("later"), "{t}");
        assert!(!t.contains("may later share these questions publicly"), "{t}");
    }
}

#[test]
fn c43_b5_a_missing_target_refreshes_the_page_and_keeps_the_owner_in() {
    for (path, which) in [("/members/remove", 0), ("/members/role", 1), ("/transfer", 2), ("/invites/revoke", 3)] {
        let (mut m, s) = open_owner_with_code();
        s.act("Post", path, 404, json!({"error": "not_found"}));
        match which {
            0 => {
                m.ask_remove("u-view");
                m.remove_member();
            }
            1 => m.set_role("u-view", Role::Editor),
            2 => {
                m.ask_transfer("u-view");
                m.offer_ownership();
            }
            _ => m.withdraw_code("inv1"),
        }
        m.frame(now(), &ready(&s));
        m.frame(now(), &ready(&s));
        assert_eq!(m.screen, Screen::Circle, "{path}: thrown out of the circle");
        assert!(m.circle.is_some());
        assert!(m.circle.as_ref().unwrap().detail.is_some(), "{path}: not reloaded");
        assert!(!problem(&m).contains("lost"), "{path}: {}", problem(&m));
        assert!(!problem(&m).is_empty());
    }
    // a 404 on the circle itself still goes back to the list
    let (mut m, s) = open_circle("owner", json!({}));
    s.act("Post", "/members/remove", 404, json!({"error": "not_found"}));
    s.act("Get", "", 404, json!({"error": "not_found"}));
    m.ask_remove("u-view");
    m.remove_member();
    for _ in 0..3 {
        m.frame(now(), &ready(&s));
    }
    assert_eq!(m.screen, Screen::List);
}

#[test]
fn c43_b6_try_again_repeats_the_action_and_a_refusal_refreshes_the_page() {
    let (mut m, s) = open_circle("owner", json!({}));
    s.fail(ApiError::Offline("down".into()));
    m.ask_remove("u-view");
    m.remove_member();
    m.frame(now(), &ready(&s));
    assert!(m.circle.as_ref().unwrap().act_problem.as_ref().unwrap().retry);
    s.act("Post", "/members/remove", 200, json!({"ok": true}));
    m.retry_act();
    m.frame(now(), &ready(&s));
    assert_eq!(s.seen().iter().filter(|x| x.path.ends_with("/members/remove")).count(), 2);
    assert!(problem(&m).is_empty(), "{}", problem(&m));
    // a changed role: the refusal reloads the circle (new role shown)
    let (mut m, s) = open_circle("owner", json!({}));
    let reads = |s: &Arc<Script>| s.seen().iter().filter(|x| x.method == Method::Get && x.path == format!("/circles/{CID}")).count();
    let before = reads(&s);
    s.act("Post", "/members/remove", 403, err("circle_role_required"));
    m.ask_remove("u-view");
    m.remove_member();
    m.frame(now(), &ready(&s));
    m.frame(now(), &ready(&s));
    assert!(reads(&s) > before, "the page was not refreshed");
    assert!(!problem(&m).is_empty());
}

#[test]
fn c43_b7_a_second_action_while_one_runs_is_not_sent() {
    let (mut m, s) = open_owner_with_code();
    s.act("Post", "/invites/revoke", 200, json!({"ok": true}));
    m.withdraw_code("inv1");
    m.withdraw_code("inv1");
    m.frame(now(), &ready(&s));
    assert_eq!(s.seen().iter().filter(|x| x.path.ends_with("/invites/revoke")).count(), 1);
    // remove, role, offer, accept
    let (mut m, s) = open_circle("owner", json!({}));
    s.act("Post", "/members/remove", 200, json!({"ok": true}));
    m.ask_remove("u-view");
    m.remove_member();
    m.set_role("u-ed", Role::Viewer);
    m.ask_transfer("u-ed");
    m.offer_ownership();
    assert_eq!(s.seen().iter().filter(|x| x.method == Method::Post).count(), 1);
}

#[test]
fn c43_a_busy_report_does_not_stay_sending_when_another_action_runs() {
    let (mut m, s) = open_circle("viewer", json!({}));
    s.keep_raw("Get", &format!("/circles/{CID}/banks/{BID}"), bank_body(&["First"]));
    m.open_bank(BID);
    m.frame(now(), &ready(&s));
    m.frame(now(), &ready(&s));
    m.begin_report();
    s.act("Post", &format!("/banks/{BID}/report"), 200, json!({"ok": true}));
    m.send_report();
    m.send_report();
    m.frame(now(), &ready(&s));
    assert!(matches!(m.bank.as_ref().unwrap().report.as_ref().unwrap().state, ReportState::Sent));
}

#[test]
fn c43_purge_now_uses_its_route_and_under_moderation_is_explained() {
    let (mut m, s) = open_circle("owner", json!({"banks": [removed_bank("owner_removed", false, json!(NOW + DAY))]}));
    s.act("Post", &format!("/banks/{BID}/purge"), 200, json!({"ok": true}));
    m.purge_bank(BID);
    m.frame(now(), &ready(&s));
    assert!(s.paths().iter().any(|p| p.ends_with("/purge")));
    let (mut m, s) = open_circle("owner", json!({"banks": [removed_bank("owner_removed", false, json!(NOW + DAY))]}));
    s.act("Post", &format!("/banks/{BID}/purge"), 409, err("under_moderation"));
    m.purge_bank(BID);
    m.frame(now(), &ready(&s));
    assert!(!problem(&m).is_empty());
    // an editor never purges
    let (mut m, s) = open_circle("editor", json!({"banks": [removed_bank("author_removed", true, json!(NOW + DAY))]}));
    let n = s.seen().len();
    m.purge_bank(BID);
    assert_eq!(s.seen().len(), n);
}

#[test]
fn c43_grade_reads_as_a_grade() {
    use super::circles_text::grade_text;
    assert_eq!(grade_text("8"), "Grade 8");
    assert_eq!(grade_text("  9-10 "), "Grade 9-10");
    assert_eq!(grade_text("Grade 8"), "Grade 8");
    assert_eq!(grade_text("K"), "Grade K");
    assert_eq!(grade_text(""), "");
}

#[test]
fn c43_the_hidden_bank_page_and_cards_draw_wide_and_narrow() {
    let ctx = egui::Context::default();
    crate::theme::setup(&ctx);
    for width in [1000.0, 420.0] {
        let (mut m, _) = open_circle("owner", json!({"banks": [masked_bank("owner_removed", json!(NOW + DAY)), removed_bank("author_removed", false, json!(NOW + DAY)), removed_bank("account_deleted", false, Value::Null)]}));
        frame(&ctx, width, &mut |ui| {
            super::circles_ui::body_of(ui, &mut m);
        });
    }
}

// ---- critic 44 ---------------------------------------------------------------------------------------------------

#[test]
fn c44_a_circle_without_a_description_is_created_and_opens() {
    let s = Script::new();
    s.keep("Get", "/circles", 200, json!({"circles": []}));
    let mut m = model();
    m.open_list();
    m.frame(now(), &ready(&s));
    m.start_create();
    m.create.name = "No description".into();
    // exactly what the pool answers (description null)
    s.on("Post", "/circles", 201, json!({"circle": {"banks": 0, "closed_at": null, "created_at": NOW, "description": null, "id": CID, "members": 1, "name": "No description", "role": "owner", "updated_at": NOW}}));
    let mut d = detail("owner", json!({}));
    d["circle"]["description"] = Value::Null;
    s.keep("Get", &format!("/circles/{CID}"), 200, d);
    s.keep("Get", &format!("/circles/{CID}/invites"), 200, json!({"invites": [], "next_cursor": null}));
    s.keep("Get", &format!("/circles/{CID}/activity"), 200, json!({"activity": [], "next_before": null}));
    m.create_circle();
    for _ in 0..4 {
        m.frame(now(), &ready(&s));
    }
    assert_eq!(m.screen, Screen::Circle);
    let c = m.circle.as_ref().unwrap();
    assert_eq!(c.id, CID);
    assert!(c.detail.is_some(), "{:?}", c.problem.as_ref().map(|p| &p.message));
}

#[test]
fn c44_every_text_the_pool_may_send_as_null_reads_as_empty() {
    let mut d = detail("owner", json!({}));
    d["circle"]["name"] = Value::Null;
    d["circle"]["description"] = Value::Null;
    d["circle"]["role"] = Value::Null;
    d["members"][1]["display_name"] = Value::Null;
    d["banks"][0]["license"] = Value::Null;
    d["banks"][0]["title"] = Value::Null;
    d["pending_transfer"] = json!({"to_user_id": "u-ed", "to_name": null, "expires_at": NOW + DAY});
    let s = Script::new();
    s.on("Get", &format!("/circles/{CID}"), 200, d);
    let got = super::transport::block_on(async { Pool::new(s.as_ref()).circle(CID).await }).expect("a null text is read as empty");
    assert!(got.circle.description.is_empty() && got.members[1].display_name.is_empty() && got.pending_transfer.unwrap().to_name.is_empty());
    // list, invites, activity, preview, join
    let s = Script::new();
    s.on("Get", "/circles", 200, json!({"circles": [summary("owner", json!({"name": null}))]}));
    assert_eq!(super::transport::block_on(async { Pool::new(s.as_ref()).circles().await }).unwrap().len(), 1);
    s.act("Get", "/invites", 200, json!({"invites": [{"invite_id": "i", "role": "viewer", "created_by": null, "created_by_name": null, "note": null, "state": null, "expires_at": 1}], "next_cursor": null}));
    assert_eq!(super::transport::block_on(async { Pool::new(s.as_ref()).invites(CID, None).await }).unwrap().invites.len(), 1);
    s.act("Get", "/activity", 200, json!({"activity": [{"at": 1, "what": null, "by": null}], "next_before": null}));
    assert_eq!(super::transport::block_on(async { Pool::new(s.as_ref()).circle_activity(CID, None).await }).unwrap().entries.len(), 1);
    s.on("Post", "/circles/join/preview", 200, json!({"circle": {"id": CID, "name": null, "owner_name": null, "members": 2}, "role": null, "already_member": false}));
    assert!(super::transport::block_on(async { Pool::new(s.as_ref()).join_preview("C-ABCDEFGHJKMNPQRSTVWXYZ01").await }).is_ok());
}

#[test]
fn c44_a_reply_the_app_cannot_read_is_never_shown_as_empty() {
    let s = Script::new();
    // one bad row among good ones: the good ones stay
    s.on("Get", "/circles", 200, json!({"circles": [summary("owner", json!({})), {"id": CID, "members": "many"}]}));
    assert_eq!(super::transport::block_on(async { Pool::new(s.as_ref()).circles().await }).unwrap().len(), 1);
    // all bad: refused, not "you are in no circle"
    s.on("Get", "/circles", 200, json!({"circles": [{"id": CID, "members": "many"}]}));
    assert!(super::transport::block_on(async { Pool::new(s.as_ref()).circles().await }).is_err());
    s.on("Get", "/circles", 200, json!({"circles": "nope"}));
    assert!(super::transport::block_on(async { Pool::new(s.as_ref()).circles().await }).is_err());
    // a create that comes back without an id
    s.on("Post", "/circles", 201, json!({"circle": {"name": "x"}}));
    assert!(super::transport::block_on(async { Pool::new(s.as_ref()).create_circle("x", "").await }).is_err());
    s.on("Post", "/circles", 201, json!({"circle": {"id": CID, "name": 5}}));
    assert!(super::transport::block_on(async { Pool::new(s.as_ref()).create_circle("x", "").await }).is_err());
    // offers, invites, activity
    s.act("Post", "/transfer", 202, json!({"ok": true, "pending": {"to_user_id": "u", "expires_at": "soon"}}));
    assert!(super::transport::block_on(async { Pool::new(s.as_ref()).offer_ownership(CID, "u").await }).is_err());
    s.act("Get", "/invites", 200, json!({"invites": [{"invite_id": 7}]}));
    assert!(super::transport::block_on(async { Pool::new(s.as_ref()).invites(CID, None).await }).is_err());
    s.act("Get", "/activity", 200, json!({"activity": [{"at": "x"}]}));
    assert!(super::transport::block_on(async { Pool::new(s.as_ref()).circle_activity(CID, None).await }).is_err());
}

#[test]
fn c44_an_admin_closed_circle_is_not_the_owners_to_reopen() {
    for extra in [json!({"close_reason": "admin"}), json!({"closed_by": "admin", "close_reason": null}), json!({"closed_by": "last_member"}), json!({"close_reason": "last_member_gone"})] {
        let mut circle = json!({"id": CID, "name": "Biology team", "description": "", "role": "owner", "joined_at": NOW - DAY, "created_at": NOW - 5 * DAY, "updated_at": NOW, "closed_at": NOW, "undo_until": null});
        merge(&mut circle, extra.clone());
        let (m, _) = open_circle("owner", json!({"circle": circle}));
        assert!(!m.allowed(Action::Reopen), "{extra}");
    }
    // an owner close (and a pool that does not say) stays reopenable
    for extra in [json!({"close_reason": "owner"}), json!({"close_reason": null})] {
        let mut circle = json!({"id": CID, "name": "Biology team", "description": "", "role": "owner", "joined_at": NOW - DAY, "created_at": NOW - 5 * DAY, "updated_at": NOW, "closed_at": NOW, "undo_until": NOW + DAY});
        merge(&mut circle, extra.clone());
        let (m, _) = open_circle("owner", json!({"circle": circle}));
        assert!(m.allowed(Action::Reopen), "{extra}");
    }
    // the refusal says who closed it
    let (mut m, s) = open_circle("owner", json!({}));
    s.act("Post", "/members/remove", 409, json!({"error": "circle_closed", "reason": "admin"}));
    m.ask_remove("u-view");
    m.remove_member();
    m.frame(now(), &ready(&s));
    assert!(problem(&m).contains("administrator") && !problem(&m).contains("owner"), "{}", problem(&m));
}

#[test]
fn c44_raw_publish_codes_have_plain_wording_and_a_circle_send_gives_no_hide_key_advice() {
    use super::pool::Issue;
    use super::publish_text as pt;
    for code in ["bad_url", "bad_license", "bad_id", "bad_figure_id", "bad_hash", "bad_form", "ambiguous_kind", "unknown_kind"] {
        let t = pt::generic_code_text(code, None);
        assert!(!t.contains(code) && !t.contains("refused it"), "{code}: {t}");
    }
    let i: Issue = serde_json::from_value(json!({"code": "keyless_needs_hide_key", "path": "bank.questions[0]"})).unwrap();
    assert!(pt::issue_text(&i, false).contains("hide the key"));
    let c = pt::issue_text_circle(&i, false);
    assert!(!c.contains("hide the key") && c.contains("set the answers"), "{c}");
}

// ---- critic 61 I3: the publish window and another account ------------------------------------------------------

fn as_account(s: &Arc<Script>, email: &str) -> AccountSnapshot {
    AccountSnapshot { account: Some(format!("https://pool.test\n{email}")), ..ready(s) }
}

#[test]
fn c61_another_account_in_the_publish_window_gets_its_own_circles_not_the_first_accounts() {
    let s = Script::new();
    let mut p = to_circle_ready(&s, "editor");
    p.frame(now(), &as_account(&s, "ada@school.org"));
    p.circle.choose(CID);
    assert!(!p.circle.choices().is_empty());
    let s2 = Script::new();
    s2.keep("Get", "/circles", 200, json!({"circles": []}));
    let b = as_account(&s2, "bob@school.org");
    for _ in 0..3 {
        p.frame(now(), &b);
    }
    assert!(p.circle.choices().is_empty(), "A's circles are not offered to B");
    assert!(p.circle.chosen.is_none());
    assert!(p.circle.active, "the destination the person picked stays a circle");
    assert_eq!(s2.paths(), vec!["Get /circles"], "B's own circles, read with B's session");
    assert_eq!(p.step, Step::Options);
}
