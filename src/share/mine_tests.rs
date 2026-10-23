//! "My shared items" against a scripted session (no network, no egui window): gating, the list and
//! its paging, the state -> actions matrix, every action and the errors it can meet, the appeal
//! and the counter-notice rules, the data download with its parts, the legal links, and headless
//! smoke frames of every screen and state.

use std::collections::VecDeque;
use std::path::{Path, PathBuf};
use std::sync::mpsc::{Receiver, Sender, channel};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use serde_json::{Value, json};
use web_time::Instant;

use super::api::{ApiError, Status};
use super::browse::Problem;
use super::mine::*;
use super::mine::Issue;
use super::mine_text as text;
use super::model::{Notice, Runner};
use super::pool::*;
use super::publish::{AccountSnapshot, Gate, LogIo, PublishedRecord};
use super::transport::{LocalFuture, Method, Request, Response};

const ID: &str = "0123456789abcdef0123456789abcdef";
const ID2: &str = "fedcba9876543210fedcba9876543210";
const ID3: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
const NOW: i64 = 1_760_000_000_000;
const DAY: i64 = 86_400_000;

// ---- a scripted session ----------------------------------------------------------------------------

#[derive(Clone, Debug)]
struct Sent {
    method: Method,
    path: String,
    body: Vec<u8>,
}

impl Sent {
    fn json(&self) -> Value {
        serde_json::from_slice(&self.body).unwrap_or(Value::Null)
    }
}

#[derive(Default)]
struct Script {
    replies: Mutex<VecDeque<Result<Response, ApiError>>>,
    seen: Mutex<Vec<Sent>>,
    gate: Mutex<Option<Receiver<()>>>,
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
    fn say_bytes(&self, bytes: Vec<u8>) -> &Self {
        let mut r = Response::new(200, "");
        r.bytes = bytes;
        self.replies.lock().unwrap().push_back(Ok(r));
        self
    }
    fn fail(&self, e: ApiError) -> &Self {
        self.replies.lock().unwrap().push_back(Err(e));
        self
    }
    fn seen(&self) -> Vec<Sent> {
        self.seen.lock().unwrap().clone()
    }
    fn paths(&self) -> Vec<String> {
        self.seen().iter().map(|s| format!("{:?} {}", s.method, s.path)).collect()
    }
    fn hold(&self) -> Sender<()> {
        let (tx, rx) = channel();
        *self.gate.lock().unwrap() = Some(rx);
        tx
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
            self.seen.lock().unwrap().push(Sent { method: r.method, path, body });
            let g = self.gate.lock().unwrap().take();
            if let Some(rx) = g {
                let _ = rx.recv_timeout(Duration::from_secs(10));
            }
            self.replies.lock().unwrap().pop_front().unwrap_or_else(|| panic!("no scripted reply for {:?}", r.url))
        })
    }
}

/// A save dialog and a disk in memory.
#[derive(Default)]
struct MemSink {
    pick: Mutex<Option<PathBuf>>,
    written: Mutex<Vec<(PathBuf, Vec<u8>)>>,
    fail: Mutex<bool>,
    asked: Mutex<Vec<String>>,
}

impl MemSink {
    fn picking(p: &str) -> Arc<MemSink> {
        let s = MemSink::default();
        *s.pick.lock().unwrap() = Some(PathBuf::from(p));
        Arc::new(s)
    }
    fn names(&self) -> Vec<String> {
        self.written.lock().unwrap().iter().map(|(p, _)| p.display().to_string()).collect()
    }
}

impl ExportSink for MemSink {
    fn choose(&self, default_name: &str) -> Option<PathBuf> {
        self.asked.lock().unwrap().push(default_name.to_string());
        self.pick.lock().unwrap().clone()
    }
    fn write(&self, path: &Path, bytes: &[u8]) -> std::io::Result<()> {
        if *self.fail.lock().unwrap() {
            return Err(std::io::Error::other("disk full"));
        }
        self.written.lock().unwrap().push((path.to_path_buf(), bytes.to_vec()));
        Ok(())
    }
}

// ---- fixtures -----------------------------------------------------------------------------------------------

fn now() -> Instant {
    Instant::now()
}

fn ready(s: &Arc<Script>) -> AccountSnapshot {
    AccountSnapshot::ready(s.clone())
}

fn item(id: &str, state: &str, extra: Value) -> Value {
    let mut v = json!({"id": id, "kind": "bank", "title": "Cell parts", "subject": "Biology", "version": 2, "visibility": "public",
                       "license": "CC-BY-4.0", "hide_key": true, "question_count": 3, "committed_at": NOW - DAY, "import_count": 1, "state": state});
    if let (Some(o), Some(e)) = (v.as_object_mut(), extra.as_object()) {
        for (k, x) in e {
            o.insert(k.clone(), x.clone());
        }
    }
    v
}

fn mine(items: Vec<Value>, next: Option<&str>) -> Value {
    json!({"items": items, "next_cursor": next, "storage": {"used": 2048, "limit": 10485760}})
}

fn model(sink: &Arc<MemSink>, log: &LogIo) -> MineModel {
    let mut m = MineModel::new(Runner::Inline, log.clone(), sink.clone()).with_retry(RetryPolicy::none());
    m.set_clock(NOW);
    m
}

fn plain_model() -> (MineModel, Arc<MemSink>) {
    let sink = MemSink::picking("/home/t/vershion-pool-export.json");
    (model(&sink, &LogIo::memory()), sink)
}

/// Opens the list with these items and selects the first.
fn opened(items: Vec<Value>) -> (MineModel, Arc<Script>, Arc<MemSink>) {
    let s = Script::new();
    s.say(200, mine(items.clone(), None));
    let (mut m, sink) = plain_model();
    m.open_list();
    m.frame(now(), &ready(&s));
    m.frame(now(), &ready(&s));
    let first = items[0]["id"].as_str().unwrap().to_string();
    m.select(&first);
    (m, s, sink)
}

fn mine_item(state: &str, extra: Value) -> MineItem {
    serde_json::from_value(item(ID, state, extra)).unwrap()
}

fn err(status: u16, code: &str, extra: Value) -> Value {
    let mut v = json!({"error": code});
    if let (Some(o), Some(e)) = (v.as_object_mut(), extra.as_object()) {
        for (k, x) in e {
            o.insert(k.clone(), x.clone());
        }
    }
    let _ = status;
    v
}

fn problem(m: &MineModel) -> String {
    m.problem.as_ref().map(|p| p.message.clone()).unwrap_or_default()
}

// ---- gating and the list ----------------------------------------------------------------------------------

#[test]
fn a_closed_model_does_nothing_at_all() {
    let s = Script::new();
    let (mut m, _) = plain_model();
    for _ in 0..3 {
        m.frame(now(), &ready(&s));
    }
    assert!(!m.open && m.jobs_idle());
    assert!(s.seen().is_empty());
    assert!(!m.wants_account_check());
}

#[test]
fn nothing_is_asked_until_the_account_is_ready_and_then_the_list_loads_once() {
    let s = Script::new();
    s.say(200, mine(vec![item(ID, "live", json!({}))], None));
    let (mut m, _) = plain_model();
    m.open_list();
    assert_eq!(m.screen, Screen::Gate);
    m.frame(now(), &AccountSnapshot { gate: Gate::SignedOut, session: None, account_open: false });
    assert!(s.seen().is_empty());
    assert_eq!(m.screen, Screen::Gate);
    m.frame(now(), &ready(&s));
    m.frame(now(), &ready(&s));
    m.frame(now(), &ready(&s));
    assert_eq!(s.paths(), vec!["Get /items/mine"]);
    assert_eq!(m.screen, Screen::List);
    assert_eq!(m.list.items.len(), 1);
    assert_eq!(m.list.storage.used, 2048);
}

#[test]
fn not_approved_and_signed_out_stop_at_the_gate_without_a_request() {
    let s = Script::new();
    let (mut m, _) = plain_model();
    m.open_list();
    m.frame(now(), &AccountSnapshot { gate: Gate::NotApproved(Status::Pending), session: None, account_open: false });
    assert_eq!(m.screen, Screen::Gate);
    assert!(s.seen().is_empty());
}

#[test]
fn paging_follows_the_cursor_and_never_shows_an_item_twice() {
    let s = Script::new();
    s.say(200, mine(vec![item(ID, "live", json!({}))], Some("1760000000000:0123456789abcdef0123456789abcdef")));
    s.say(200, mine(vec![item(ID, "live", json!({})), item(ID2, "live", json!({}))], None));
    let (mut m, _) = plain_model();
    m.open_list();
    m.frame(now(), &ready(&s));
    m.frame(now(), &ready(&s));
    assert!(m.can_load_more());
    m.load_more();
    m.frame(now(), &ready(&s));
    assert_eq!(s.paths(), vec!["Get /items/mine", "Get /items/mine?after=1760000000000:0123456789abcdef0123456789abcdef"]);
    assert_eq!(m.list.items.len(), 2);
    assert!(!m.can_load_more());
}

#[test]
fn a_cursor_with_odd_characters_is_never_sent() {
    let s = Script::new();
    let (mut m, _) = plain_model();
    m.open_list();
    m.screen = Screen::List;
    m.list.loaded = true;
    m.list.next = Some("1:x&admin=1".into());
    let sess: Arc<dyn PoolSession + Send + Sync> = s.clone();
    m.session_for_test(&sess);
    m.load_more();
    m.frame(now(), &ready(&s));
    assert!(s.seen().is_empty());
    assert!(m.list.problem.is_some());
}

#[test]
fn items_with_a_bad_id_are_dropped_from_the_list() {
    let s = Script::new();
    s.say(200, mine(vec![item("../etc", "live", json!({})), item(ID, "live", json!({}))], None));
    let (mut m, _) = plain_model();
    m.open_list();
    m.frame(now(), &ready(&s));
    m.frame(now(), &ready(&s));
    assert_eq!(m.list.items.len(), 1);
}

// ---- the state -> actions matrix -----------------------------------------------------------------------------------

#[test]
fn every_state_allows_exactly_these_actions() {
    use Action::*;
    let held_class = json!({"hold_class": "student_data", "appeal": "available"});
    let cases: Vec<(&str, Value, Vec<Action>)> = vec![
        ("live", json!({}), vec![MakePrivate, Delete, Republish]),
        ("live", json!({"visibility": "unlisted"}), vec![MakePublic, Delete, Republish]),
        ("deleted", json!({"undo_until": NOW + DAY}), vec![Undo]),
        ("deleted", json!({"undo_until": NOW - 1}), vec![]),
        ("deleted", json!({}), vec![]),
        ("taken_down", json!({"counter_notice": "available"}), vec![CounterNotice]),
        ("taken_down", json!({"counter_notice": "received"}), vec![]),
        ("taken_down", json!({"counter_notice": "closed"}), vec![]),
        ("taken_down", json!({}), vec![]),
        ("takedown_pending", json!({"action_id": "act1", "counter_notice_by": NOW + DAY}), vec![CounterNotice]),
        ("takedown_pending", json!({"action_id": "act1", "counter_notice_by": NOW + DAY, "counter_notice_sent": true}), vec![]),
        ("takedown_pending", json!({"action_id": "act1", "counter_notice_by": NOW - 1}), vec![]),
        ("takedown_pending", json!({"counter_notice_by": NOW + DAY}), vec![]),
        ("removed_privacy", json!({"appeal": "available", "appeal_until": NOW + DAY}), vec![Appeal]),
        ("removed_privacy", json!({"appeal": "available", "appeal_until": NOW - 1}), vec![]),
        ("removed_privacy", json!({}), vec![]),
        ("removed", json!({"removed_reason": "admin", "appeal": "available"}), vec![Appeal]),
        ("removed", json!({"removed_reason": "takedown"}), vec![]),
        ("needs_reupload", json!({}), vec![Republish, Delete]),
        ("key_leak_review", json!({}), vec![Republish, Delete]),
        ("held_for_review", json!({"appeal": "available"}), vec![Appeal]),
        ("held_for_review", held_class, vec![Appeal, Republish]),
        ("held_for_review", json!({}), vec![]),
        ("something_new", json!({}), vec![]),
    ];
    for (state, extra, want) in cases {
        let it = mine_item(state, extra.clone());
        assert_eq!(actions_for(&it, NOW), want, "state {state} with {extra}");
    }
}

#[test]
fn the_selected_items_actions_leave_out_an_appeal_already_sent() {
    let (mut m, s, _) = opened(vec![item(ID, "removed", json!({"removed_reason": "admin", "appeal": "available"}))]);
    assert_eq!(m.actions(), vec![Action::Appeal]);
    m.start_appeal();
    m.appeal_note = "It is my own work.".into();
    s.say(202, json!({"ok": true, "message": "The admin will review your item."}));
    m.send_appeal();
    m.frame(now(), &ready(&s));
    assert!(m.actions().is_empty(), "one appeal at a time");
    assert_eq!(m.appeal_answer(ID), Some("The admin will review your item."));
}

#[test]
fn an_action_the_state_does_not_allow_is_refused_without_a_request() {
    let (mut m, s, _) = opened(vec![item(ID, "removed", json!({"removed_reason": "takedown"}))]);
    let before = s.seen().len();
    m.set_visibility(Visibility::Public);
    m.ask_delete();
    m.confirm_delete();
    m.undo_delete();
    m.start_appeal();
    m.send_appeal();
    m.start_counter();
    m.send_counter();
    m.share_again();
    assert_eq!(s.seen().len(), before);
    assert!(matches!(m.notice, Some(Notice::Error(_))));
    assert_eq!(m.panel, Panel::None);
}

// ---- visibility, delete, undo --------------------------------------------------------------------------------

#[test]
fn visibility_is_a_patch_and_the_list_follows() {
    let (mut m, s, _) = opened(vec![item(ID, "live", json!({}))]);
    s.say(200, json!({"ok": true, "visibility": "unlisted"}));
    m.set_visibility(Visibility::Unlisted);
    m.frame(now(), &ready(&s));
    let sent = s.seen().last().unwrap().clone();
    assert_eq!((sent.method, sent.path.as_str()), (Method::Patch, "/items/0123456789abcdef0123456789abcdef"));
    assert_eq!(sent.json(), json!({"visibility": "unlisted"}));
    assert!(m.item(ID).unwrap().is_unlisted());
    assert_eq!(m.actions(), vec![Action::MakePublic, Action::Delete, Action::Republish]);
}

#[test]
fn a_circle_only_licence_can_not_go_public_and_the_pool_says_so() {
    let (mut m, s, _) = opened(vec![item(ID, "live", json!({"visibility": "unlisted", "license": "circle-only"}))]);
    s.say(422, err(422, "license_not_public", json!({})));
    m.set_visibility(Visibility::Public);
    m.frame(now(), &ready(&s));
    assert!(!problem(&m).is_empty());
    assert!(m.item(ID).unwrap().is_unlisted());
}

#[test]
fn delete_needs_the_confirmation_and_then_offers_the_undo() {
    let (mut m, s, _) = opened(vec![item(ID, "live", json!({}))]);
    let n = s.seen().len();
    m.confirm_delete();
    assert_eq!(s.seen().len(), n, "no confirmation, no delete");
    m.ask_delete();
    assert_eq!(m.panel, Panel::ConfirmDelete);
    m.cancel_panel();
    assert_eq!(m.panel, Panel::None);
    m.ask_delete();
    s.say(200, json!({"ok": true, "undo_until": NOW + 14 * DAY}));
    m.confirm_delete();
    m.frame(now(), &ready(&s));
    let sent = s.seen().last().unwrap().clone();
    assert_eq!((sent.method, sent.path.as_str()), (Method::Delete, "/items/0123456789abcdef0123456789abcdef"));
    let it = m.item(ID).unwrap();
    assert_eq!(it.state, ItemState::Deleted);
    assert_eq!(it.undo_until, Some(NOW + 14 * DAY));
    assert_eq!(m.actions(), vec![Action::Undo]);
    assert!(matches!(&m.notice, Some(Notice::Info(t)) if t.contains("undo")));
}

#[test]
fn undo_inside_the_window_restores_and_refreshes_the_list() {
    let (mut m, s, _) = opened(vec![item(ID, "deleted", json!({"undo_until": NOW + DAY}))]);
    s.say(200, json!({"ok": true}));
    s.say(200, mine(vec![item(ID, "live", json!({}))], None));
    m.undo_delete();
    m.frame(now(), &ready(&s));
    m.frame(now(), &ready(&s));
    let p = s.paths();
    assert_eq!(&p[p.len() - 2..], ["Post /items/0123456789abcdef0123456789abcdef/undelete", "Get /items/mine"]);
    assert_eq!(m.item(ID).unwrap().state, ItemState::Live);
}

#[test]
fn undo_after_the_window_is_refused_locally() {
    let (mut m, s, _) = opened(vec![item(ID, "deleted", json!({"undo_until": NOW + DAY}))]);
    assert_eq!(m.actions(), vec![Action::Undo]);
    m.set_clock(NOW + DAY + 1);
    assert!(m.actions().is_empty());
    let n = s.seen().len();
    m.undo_delete();
    assert_eq!(s.seen().len(), n);
    let it = m.selected_item().unwrap().clone();
    assert!(text::explain(&it, NOW + DAY + 1).contains("passed"));
    assert!(text::explain(&it, NOW).contains(&text::date(NOW + DAY)));
}

#[test]
fn undo_the_pool_refuses_after_the_window_reloads_the_list() {
    let (mut m, s, _) = opened(vec![item(ID, "deleted", json!({"undo_until": NOW + DAY}))]);
    s.say(409, err(409, "window_closed", json!({"message": "The 14 days to undo this have ended."})));
    s.say(200, mine(vec![], None));
    m.undo_delete();
    m.frame(now(), &ready(&s));
    m.frame(now(), &ready(&s));
    assert!(problem(&m).contains("14 days") || m.screen == Screen::List);
    assert!(s.paths().last().unwrap().ends_with("/items/mine"));
}

// ---- appeals --------------------------------------------------------------------------------------------------

fn removed_admin() -> Value {
    item(ID, "removed", json!({"removed_reason": "admin", "appeal": "available"}))
}

#[test]
fn an_appeal_sends_the_trimmed_note_once() {
    let (mut m, s, _) = opened(vec![removed_admin()]);
    m.start_appeal();
    assert_eq!(m.panel, Panel::Appeal);
    m.appeal_note = "   ".into();
    m.send_appeal();
    let n = s.seen().len();
    assert_eq!(s.seen().len(), n, "an empty note is not sent");
    assert!(problem(&m).contains("500"));
    m.appeal_note = "x".repeat(501);
    assert!(!m.appeal_ready());
    m.send_appeal();
    assert_eq!(s.seen().len(), n);
    m.appeal_note = "  These are my own questions.  ".into();
    s.say(202, json!({"ok": true, "message": "The admin will review your item."}));
    m.send_appeal();
    m.frame(now(), &ready(&s));
    let sent = s.seen().last().unwrap().clone();
    assert_eq!(sent.path, "/items/0123456789abcdef0123456789abcdef/appeal");
    assert_eq!(sent.json(), json!({"note": "These are my own questions."}));
    assert_eq!(m.panel, Panel::None);
    assert!(m.appeal_note.is_empty());
    m.start_appeal();
    assert_eq!(s.seen().len(), n + 1, "the second appeal never leaves the machine");
}

#[test]
fn an_appeal_the_admin_already_decided_ends_the_offer() {
    let (mut m, s, _) = opened(vec![removed_admin()]);
    m.start_appeal();
    m.appeal_note = "Please look again.".into();
    s.say(409, err(409, "decided", json!({"message": "The admin has already decided on this. It cannot be appealed again."})));
    s.say(200, mine(vec![removed_admin()], None));
    m.send_appeal();
    m.frame(now(), &ready(&s));
    m.frame(now(), &ready(&s));
    assert!(problem(&m).contains("already decided"));
    assert!(m.appeal_answer(ID).unwrap().contains("already decided"));
    assert!(m.actions().is_empty());
}

#[test]
fn an_appeal_already_open_is_a_plain_acknowledgement() {
    let (mut m, s, _) = opened(vec![item(ID, "held_for_review", json!({"appeal": "available"}))]);
    m.start_appeal();
    m.appeal_note = "Mine.".into();
    s.say(202, json!({"ok": true, "message": "Your appeal is already with the admin."}));
    m.send_appeal();
    m.frame(now(), &ready(&s));
    assert!(m.appeal_answer(ID).unwrap().contains("already with the admin"));
}

#[test]
fn an_appeal_for_an_item_that_is_gone_says_so_and_looks_again() {
    let (mut m, s, _) = opened(vec![removed_admin()]);
    m.start_appeal();
    m.appeal_note = "Mine.".into();
    s.say(404, err(404, "not_found", json!({})));
    s.say(200, mine(vec![], None));
    m.send_appeal();
    m.frame(now(), &ready(&s));
    m.frame(now(), &ready(&s));
    assert!(s.paths().last().unwrap().ends_with("/items/mine"));
    assert!(m.item(ID).is_none());
}

#[test]
fn the_daily_appeal_limit_waits_for_the_retry_after() {
    let (mut m, s, _) = opened(vec![removed_admin()]);
    m.start_appeal();
    m.appeal_note = "Mine.".into();
    s.say_after(429, 3600, err(429, "daily_limit", json!({"metric": "appeals"})));
    m.send_appeal();
    m.frame(now(), &ready(&s));
    let p = m.problem.clone().unwrap();
    assert!(p.retry && p.retry_at.is_some());
    assert!(p.message.contains("appeals"));
    assert!(m.retry_wait(&p).is_some());
}

// ---- the counter-notice ----------------------------------------------------------------------------------------

fn taken_down() -> Value {
    item(ID, "taken_down", json!({"counter_notice": "available", "action_id": "act1"}))
}

fn fill(m: &mut MineModel) {
    m.counter.name = "  Ada  Lovelace ".into();
    m.counter.address = "1 Main St, Chico CA".into();
    m.counter.phone = "555-0100".into();
    m.counter.material_desc = "My own quiz questions.".into();
    m.counter.signature = "ada lovelace".into();
    m.counter.perjury_stmt = true;
    m.counter.consent_jurisdiction = true;
    m.counter.accept_service = true;
}

#[test]
fn the_counter_notice_form_starts_empty_and_lists_everything_missing() {
    let (mut m, _s, _) = opened(vec![taken_down()]);
    m.start_counter();
    assert_eq!(m.panel, Panel::Counter);
    assert_eq!(m.counter, CounterForm::default(), "nothing is filled in for the person");
    let issues = m.counter_issues();
    let fields: Vec<&str> = issues.iter().map(|(f, _)| *f).collect();
    assert_eq!(fields, ["name", "address", "phone", "material_desc", "signature", "perjury_stmt", "consent_jurisdiction", "accept_service"]);
    assert!(issues.iter().all(|(_, i)| matches!(i, Issue::Empty | Issue::NotTicked)));
}

#[test]
fn the_signature_must_be_the_name_and_every_statement_ticked() {
    let (mut m, s, _) = opened(vec![taken_down()]);
    m.start_counter();
    fill(&mut m);
    assert!(m.counter.ready());
    m.counter.signature = "A. Lovelace".into();
    assert_eq!(m.counter_issues(), vec![("signature", Issue::NotYourName)]);
    assert!(issue_text("signature", Issue::NotYourName).contains("full name"));
    m.counter.signature = "ADA LOVELACE".into();
    m.counter.perjury_stmt = false;
    assert_eq!(m.counter_issues(), vec![("perjury_stmt", Issue::NotTicked)]);
    let n = s.seen().len();
    m.send_counter();
    assert_eq!(s.seen().len(), n, "an incomplete form never leaves the machine");
    assert!(problem(&m).contains("Nothing was sent"));
    m.counter.perjury_stmt = true;
    m.counter.address = "x".repeat(501);
    assert_eq!(m.counter_issues(), vec![("address", Issue::TooLong(500))]);
}

#[test]
fn a_complete_counter_notice_is_sent_once_and_nothing_stays_behind() {
    let (mut m, s, _) = opened(vec![taken_down()]);
    m.start_counter();
    fill(&mut m);
    s.say(202, json!({"ok": true, "restore_not_before": NOW + 10 * DAY, "restore_by": NOW + 14 * DAY}));
    m.send_counter();
    m.frame(now(), &ready(&s));
    let sent = s.seen().last().unwrap().clone();
    assert_eq!(sent.path, "/items/0123456789abcdef0123456789abcdef/counter-notice");
    assert_eq!(
        sent.json(),
        json!({"signature": "ada lovelace", "material_desc": "My own quiz questions.", "name": "Ada  Lovelace", "address": "1 Main St, Chico CA",
               "phone": "555-0100", "perjury_stmt": true, "consent_jurisdiction": true, "accept_service": true})
    );
    assert_eq!(m.counter, CounterForm::default(), "cleared the moment it was sent");
    assert!(matches!(m.panel, Panel::CounterSent(a) if a.restore_by == NOW + 14 * DAY));
    assert_eq!(m.item(ID).unwrap().counter_notice.as_deref(), Some("received"));
    assert!(m.actions().is_empty(), "a second counter-notice is not offered");
    assert!(!format!("{m:?}", m = m.counter).contains("Ada"));
}

#[test]
fn closing_the_window_throws_away_a_half_typed_counter_notice() {
    let (mut m, _s, _) = opened(vec![taken_down()]);
    m.start_counter();
    fill(&mut m);
    m.close();
    assert_eq!(m.counter, CounterForm::default());
    assert!(!m.open);
}

#[test]
fn the_pool_can_still_refuse_a_counter_notice() {
    for (status, code, extra, want) in [
        (400, "counter_notice_incomplete", json!({"missing": ["phone", "perjury_stmt"]}), "phone number"),
        (409, "window_closed", json!({"message": "The 30-day counter-notice window for this takedown has ended."}), "30-day"),
        (409, "notice_not_actionable", json!({"message": "That notice can not be answered."}), "can not be answered"),
    ] {
        let (mut m, s, _) = opened(vec![taken_down()]);
        m.start_counter();
        fill(&mut m);
        s.say(status, err(status, code, extra));
        if status == 409 {
            s.say(200, mine(vec![taken_down()], None));
        }
        m.send_counter();
        m.frame(now(), &ready(&s));
        assert!(problem(&m).contains(want), "{code}: {}", problem(&m));
        assert_ne!(m.panel, Panel::None, "the form stays so nothing typed is lost on a refusal");
    }
}

#[test]
fn a_pending_takedown_is_answered_on_its_id_and_marked_sent() {
    let pending = item(ID, "takedown_pending", json!({"action_id": "act9", "counter_notice_by": NOW + 20 * DAY, "counter_notice_sent": false}));
    let (mut m, s, _) = opened(vec![pending]);
    assert!(text::explain(m.selected_item().unwrap(), NOW).contains(&text::date(NOW + 20 * DAY)));
    m.start_counter();
    fill(&mut m);
    s.say(202, json!({"ok": true, "restore_not_before": NOW + 10 * DAY, "restore_by": NOW + 14 * DAY}));
    m.send_counter();
    m.frame(now(), &ready(&s));
    assert_eq!(s.seen().last().unwrap().path, "/items/0123456789abcdef0123456789abcdef/counter-notice");
    assert!(m.item(ID).unwrap().counter_notice_sent);
    assert!(m.actions().is_empty());
}

#[test]
fn the_statements_are_marked_as_drafts_and_the_pool_supplies_none() {
    assert!(text::COUNTER_DRAFT.contains("DRAFT"));
    assert!(text::COUNTER_INTRO.contains("forwarded to the person who complained"));
    assert!(text::COUNTER_NOT_KEPT.contains("does not keep"));
    assert!(text::STATEMENT_PERJURY.contains("penalty of perjury"));
}

// ---- errors ------------------------------------------------------------------------------------------------

#[test]
fn a_session_that_ended_goes_to_the_gate_and_comes_back_to_the_list() {
    let s = Script::new();
    s.fail(ApiError::SignedOut);
    let (mut m, _) = plain_model();
    m.open_list();
    m.frame(now(), &ready(&s));
    m.frame(now(), &ready(&s));
    assert_eq!(m.screen, Screen::Gate);
    assert!(matches!(m.notice, Some(Notice::Info(_))));
    s.say(200, mine(vec![item(ID, "live", json!({}))], None));
    m.frame(now(), &ready(&s));
    m.frame(now(), &ready(&s));
    assert_eq!(m.screen, Screen::List);
}

#[test]
fn each_error_has_words_and_the_right_next_step() {
    // (status, body, retry_after, check)
    type Check = fn(&Problem) -> bool;
    let cases: Vec<(u16, Value, Option<u64>, Check, &str)> = vec![
        (403, err(403, "account_not_approved", json!({})), None, |p| p.account && !p.retry, "403 not approved"),
        (404, err(404, "not_found", json!({})), None, |p| p.message.contains("not available"), "404 hidden is gone"),
        (410, err(410, "gone", json!({})), None, |p| p.message.contains("removed for good"), "410"),
        (413, err(413, "body_too_large", json!({"limit": 1000})), None, |p| !p.message.is_empty() && !p.retry, "413"),
        (422, err(422, "invalid_request", json!({"field": "visibility"})), None, |p| !p.message.is_empty() && !p.retry, "422"),
        (429, err(429, "rate_limited", json!({})), Some(30), |p| p.retry && p.retry_at.is_some(), "429 with Retry-After"),
        (409, err(409, "under_moderation", json!({"message": "<b>Held</b> for review"})), None, |p| !p.message.contains('<') || p.message.contains("Held"), "409"),
    ];
    for (status, body, after, check, name) in cases {
        let (mut m, s, _) = opened(vec![item(ID, "live", json!({}))]);
        match after {
            Some(a) => s.say_after(status, a, body),
            None => s.say(status, body),
        };
        if matches!(status, 404 | 409 | 410) {
            s.say(200, mine(vec![item(ID, "live", json!({}))], None));
        }
        m.set_visibility(Visibility::Unlisted);
        m.frame(now(), &ready(&s));
        let p = m.problem.clone().unwrap_or_else(|| panic!("{name}: no problem"));
        assert!(check(&p), "{name}: {p:?}");
    }
}

#[test]
fn offline_can_be_retried() {
    let (mut m, s, _) = opened(vec![item(ID, "live", json!({}))]);
    s.fail(ApiError::Offline("no network".into()));
    m.set_visibility(Visibility::Unlisted);
    m.frame(now(), &ready(&s));
    assert!(m.problem.as_ref().unwrap().retry);
}

#[test]
fn a_failed_first_page_offers_try_again() {
    let s = Script::new();
    s.fail(ApiError::Offline("x".into()));
    let (mut m, _) = plain_model();
    m.open_list();
    m.frame(now(), &ready(&s));
    m.frame(now(), &ready(&s));
    assert!(m.list.problem.as_ref().unwrap().retry);
    assert_eq!(s.seen().len(), 1, "no hammering: one try until the person asks again");
    s.say(200, mine(vec![], None));
    m.reload();
    m.frame(now(), &ready(&s));
    assert!(m.list.loaded && m.list.problem.is_none());
}

// ---- the data download -----------------------------------------------------------------------------------------

fn export_body(extra: Value) -> Vec<u8> {
    let mut v = json!({"format": "vershion-pool-export/v1", "profile": {"display_name": "Ada"}, "items": []});
    if let (Some(o), Some(e)) = (v.as_object_mut(), extra.as_object()) {
        for (k, x) in e {
            o.insert(k.clone(), x.clone());
        }
    }
    v.to_string().into_bytes()
}

fn on_account_page() -> (MineModel, Arc<Script>, Arc<MemSink>) {
    let s = Script::new();
    s.say(200, mine(vec![], None));
    let (mut m, sink) = plain_model();
    m.open_account_page();
    m.frame(now(), &ready(&s));
    m.frame(now(), &ready(&s));
    assert_eq!(m.screen, Screen::Account);
    (m, s, sink)
}

#[test]
fn a_whole_export_is_saved_to_the_file_the_person_picks_and_then_forgotten() {
    let (mut m, s, sink) = on_account_page();
    s.say_bytes(export_body(json!({"bodies": {}})));
    m.start_export();
    m.frame(now(), &ready(&s));
    assert!(matches!(m.export, ExportState::Ready { parts: 0, .. }));
    assert!(m.export_ready());
    assert_eq!(s.paths().last().unwrap(), "Get /me/export");
    assert!(sink.names().is_empty(), "nothing is written before the person saves");
    m.save_export();
    assert_eq!(sink.names(), vec!["/home/t/vershion-pool-export.json"]);
    assert_eq!(sink.asked.lock().unwrap().as_slice(), [EXPORT_NAME]);
    assert!(matches!(&m.export, ExportState::Saved { place, parts: 0 } if place.contains("vershion-pool-export.json") || place.contains("downloads")));
    assert!(!m.export_ready(), "no copy kept");
    let written = sink.written.lock().unwrap();
    assert!(String::from_utf8_lossy(&written[0].1).contains("vershion-pool-export/v1"));
}

#[test]
fn a_large_export_arrives_in_parts_saved_beside_the_main_file() {
    let (mut m, s, sink) = on_account_page();
    s.say_bytes(export_body(json!({"export_too_large": true, "manifest": {"parts": [
        {"part": 1, "items": ["a"], "url": "/me/export?part=1"}, {"part": 2, "items": ["b"], "url": "/me/export?part=2"}]}})));
    s.say_bytes(br#"{"part":1,"of":2}"#.to_vec());
    s.say_bytes(br#"{"part":2,"of":2}"#.to_vec());
    m.start_export();
    m.frame(now(), &ready(&s));
    assert_eq!(s.paths()[s.paths().len() - 3..], ["Get /me/export", "Get /me/export?part=1", "Get /me/export?part=2"]);
    assert!(matches!(m.export, ExportState::Ready { parts: 2, .. }));
    m.save_export();
    assert_eq!(
        sink.names(),
        vec!["/home/t/vershion-pool-export.json", "/home/t/vershion-pool-export-part-1.json", "/home/t/vershion-pool-export-part-2.json"]
    );
    assert!(matches!(m.export, ExportState::Saved { parts: 2, .. }));
}

#[test]
fn a_manifest_with_no_parts_or_far_too_many_is_refused() {
    for parts in [json!([]), Value::Array((1..=600).map(|i| json!({"part": i})).collect())] {
        let (mut m, s, sink) = on_account_page();
        s.say_bytes(export_body(json!({"export_too_large": true, "manifest": {"parts": parts}})));
        m.start_export();
        m.frame(now(), &ready(&s));
        assert!(matches!(m.export, ExportState::Failed(_)));
        assert!(sink.names().is_empty());
    }
}

#[test]
fn a_running_download_can_be_cancelled_and_a_late_answer_is_ignored() {
    let s = Script::new();
    s.say(200, mine(vec![], None));
    let sink = MemSink::picking("/home/t/x.json");
    let mut m = MineModel::new(Runner::Background, LogIo::memory(), sink.clone()).with_retry(RetryPolicy::none());
    m.set_clock(NOW);
    m.open_account_page();
    for _ in 0..50 {
        m.frame(now(), &ready(&s));
        if m.screen == Screen::Account && m.list.loaded {
            break;
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    let release = s.hold();
    s.say_bytes(export_body(json!({})));
    m.start_export();
    assert_eq!(m.export, ExportState::Running { done: 0, total: 0 });
    assert!(m.busy());
    m.cancel_export();
    assert_eq!(m.export, ExportState::Idle);
    release.send(()).unwrap();
    std::thread::sleep(Duration::from_millis(100));
    m.frame(now(), &ready(&s));
    assert_eq!(m.export, ExportState::Idle);
    assert!(!m.export_ready());
}

#[test]
fn a_cancelled_save_dialog_keeps_the_download_and_a_failed_write_says_which_file() {
    let (mut m, s, sink) = on_account_page();
    s.say_bytes(export_body(json!({})));
    m.start_export();
    m.frame(now(), &ready(&s));
    *sink.pick.lock().unwrap() = None;
    m.save_export();
    assert!(m.export_ready() && matches!(m.export, ExportState::Ready { .. }));
    *sink.pick.lock().unwrap() = Some(PathBuf::from("/full/disk.json"));
    *sink.fail.lock().unwrap() = true;
    m.save_export();
    match &m.export {
        ExportState::Failed(p) => assert!(p.message.contains("/full/disk.json") && !p.message.contains("Ada")),
        other => panic!("{other:?}"),
    }
    assert!(m.export_ready(), "still there to try again");
}

#[test]
fn closing_the_window_drops_an_unsaved_download() {
    let (mut m, s, _) = on_account_page();
    s.say_bytes(export_body(json!({})));
    m.start_export();
    m.frame(now(), &ready(&s));
    assert!(m.export_ready());
    m.close();
    assert!(!m.export_ready());
    assert_eq!(m.export, ExportState::Idle);
}

#[test]
fn an_export_needs_a_recent_sign_in_and_is_limited_to_three_a_day() {
    let (mut m, s, _) = on_account_page();
    s.say(403, err(403, "reauth_required", json!({"message": "Sign in again to export your account"})));
    m.start_export();
    m.frame(now(), &ready(&s));
    match &m.export {
        ExportState::Failed(p) => assert!(p.account && p.message.contains("Sign in again")),
        other => panic!("{other:?}"),
    }
    s.say_after(429, 7200, err(429, "daily_limit", json!({"metric": "exports"})));
    m.start_export();
    m.frame(now(), &ready(&s));
    match &m.export {
        ExportState::Failed(p) => assert!(p.message.contains("three times") && p.retry_at.is_some()),
        other => panic!("{other:?}"),
    }
}

#[test]
fn an_export_part_out_of_range_is_a_failure_not_a_partial_file() {
    let (mut m, s, sink) = on_account_page();
    s.say_bytes(export_body(json!({"export_too_large": true, "manifest": {"parts": [{"part": 1}, {"part": 2}]}})));
    s.say_bytes(b"{}".to_vec());
    s.say(404, err(404, "not_found", json!({})));
    m.start_export();
    m.frame(now(), &ready(&s));
    assert!(matches!(m.export, ExportState::Failed(_)));
    assert!(!m.export_ready());
    assert!(sink.names().is_empty());
}

// ---- legal, deletion, strikes, share again ---------------------------------------------------------------------------

#[test]
fn legal_links_are_built_from_the_configured_server_and_absent_without_one() {
    let (mut m, _) = plain_model();
    assert!(m.legal_links().iter().all(|(_, u)| u.is_none()), "no server, no links");
    assert_eq!(m.legal_links().len(), 4);
    m.set_base_url("https://pool.example.org/");
    let links = m.legal_links();
    assert_eq!(links[0], ("Terms of use", Some("https://pool.example.org/legal/terms.html".to_string())));
    assert!(links.iter().all(|(_, u)| u.as_deref().is_some_and(|u| u.starts_with("https://pool.example.org/legal/"))));
    assert_eq!(text::legal_url("javascript:alert(1)", "terms.html"), None);
    assert_eq!(text::legal_url("", "terms.html"), None);
    assert!(text::DRAFT_LEGAL.contains("drafts"));
}

#[test]
fn deleting_the_account_is_explained_in_plain_words_and_leads_to_the_account_window() {
    let all = text::DELETION_LINES.join(" ");
    for must in ["stops working at once", "deleted teacher", "14 days", "3 years", "not touched"] {
        assert!(all.contains(must), "{must}");
    }
    let (mut m, _) = plain_model();
    assert!(!m.take_delete_page());
    m.request_delete_page();
    assert!(m.take_delete_page());
    assert!(!m.take_delete_page());
}

#[test]
fn the_strike_line_is_policy_only_and_calm() {
    assert!(text::STRIKES_LINE.contains("Three upheld strikes within twelve months"));
    assert!(text::STRIKES_LINE.contains("nothing is suspended automatically"));
    assert!(text::STRIKES_LINE.contains("does not show a count"));
}

#[test]
fn share_again_uses_the_publish_record_and_says_what_it_cannot_do() {
    let log = LogIo::memory();
    log.put(PublishedRecord { key: "bank:b1".into(), item_id: ID.into(), publish_id: "p1".into(), version: 2, visibility: "public".into(), license: "CC-BY-4.0".into(), title: "T".into(), qids: vec![] }).unwrap();
    log.put(PublishedRecord { key: "exam:e1".into(), item_id: ID2.into(), publish_id: "p2".into(), version: 1, visibility: "public".into(), license: "CC-BY-4.0".into(), title: "E".into(), qids: vec![] }).unwrap();
    let sink = MemSink::picking("/x");
    let s = Script::new();
    s.say(200, mine(vec![item(ID, "live", json!({})), item(ID2, "live", json!({})), item(ID3, "needs_reupload", json!({}))], None));
    let mut m = model(&sink, &log);
    m.open_list();
    m.frame(now(), &ready(&s));
    m.frame(now(), &ready(&s));
    m.select(ID);
    m.share_again();
    assert_eq!(m.take_republish().as_deref(), Some("bank:b1"));
    assert_eq!(m.take_republish(), None);
    m.select(ID2);
    m.share_again();
    assert!(m.take_republish().is_none());
    assert!(matches!(&m.notice, Some(Notice::Info(t)) if t.contains("exam")));
    m.select(ID3);
    m.share_again();
    assert!(matches!(&m.notice, Some(Notice::Info(t)) if t.contains("no record")));
    assert_eq!(m.shared_record("bank:b1").unwrap().item_id, ID);
}

#[test]
fn an_item_opened_from_the_done_screen_lands_on_its_page() {
    let s = Script::new();
    s.say(200, mine(vec![item(ID2, "live", json!({})), item(ID, "live", json!({}))], None));
    let (mut m, _) = plain_model();
    m.open_item(ID);
    m.frame(now(), &ready(&s));
    m.frame(now(), &ready(&s));
    assert_eq!(m.screen, Screen::Detail);
    assert_eq!(m.selected.as_deref(), Some(ID));
}

// ---- wording -----------------------------------------------------------------------------------------------------

#[test]
fn every_state_has_words_and_server_text_is_plain() {
    for state in ["live", "deleted", "taken_down", "takedown_pending", "removed_privacy", "removed", "needs_reupload", "key_leak_review", "held_for_review", "other"] {
        let it = mine_item(state, json!({"notice": "A\u{202e}B\u{7}\nC", "counter_notice": "available"}));
        assert!(!text::state_label(&it).is_empty());
        let e = text::explain(&it, NOW);
        assert!(!e.is_empty());
        assert!(!e.contains('\u{202e}') && !e.contains('\u{7}'), "{state}: {e:?}");
    }
    let hold = mine_item("held_for_review", json!({}));
    let t = text::hold_reason(&hold);
    assert!(t.contains("does not say why or who raised it"));
    let sd = text::hold_reason(&mine_item("held_for_review", json!({"hold_class": "student_data"})));
    assert!(sd.contains("student data"));
    let cr = text::hold_reason(&mine_item("held_for_review", json!({"hold_class": "copyright"})));
    assert!(cr.contains("copyright"));
}

// ---- smoke frames -------------------------------------------------------------------------------------------------

fn draw(ctx: &egui::Context, m: &mut MineModel) -> bool {
    let mut closed = false;
    for i in 0..2 {
        let input = egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(900.0, 1600.0))),
            time: Some(i as f64 * 0.1),
            ..Default::default()
        };
        let out = ctx.run_ui(input, |ui| {
            closed |= super::mine_ui::page(ui, m);
        });
        out.drop_without_applying_deltas();
    }
    closed
}

#[test]
fn every_screen_and_state_draws_without_panicking() {
    let ctx = egui::Context::default();
    crate::theme::setup(&ctx);

    // gate pages
    for gate in [Gate::Checking, Gate::NotSetUp, Gate::NoServer, Gate::SignedOut, Gate::Unreachable, Gate::NotApproved(Status::Pending)] {
        let (mut m, _) = plain_model();
        m.open_list();
        m.frame(now(), &AccountSnapshot { gate, session: None, account_open: false });
        assert!(!draw(&ctx, &mut m));
    }

    // an empty list, a loading list, a failed list
    let s = Script::new();
    let (mut m, _) = plain_model();
    m.open_list();
    m.screen = Screen::List;
    draw(&ctx, &mut m);
    s.say(200, mine(vec![], None));
    m.frame(now(), &ready(&s));
    m.frame(now(), &ready(&s));
    draw(&ctx, &mut m);
    m.list.problem = Some(Problem { message: "nope".into(), retry: true, retry_at: None, account: true });
    draw(&ctx, &mut m);

    // every state on the list and on its own page, with its panels
    let all = vec![
        item(ID, "live", json!({})),
        item(ID2, "deleted", json!({"undo_until": NOW + DAY})),
        item(ID3, "taken_down", json!({"counter_notice": "available"})),
        item("bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb", "takedown_pending", json!({"action_id": "a", "counter_notice_by": NOW + DAY})),
        item("cccccccccccccccccccccccccccccccc", "removed_privacy", json!({"appeal": "available", "appeal_until": NOW + DAY})),
        item("dddddddddddddddddddddddddddddddd", "removed", json!({"removed_reason": "admin", "appeal": "available"})),
        item("eeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee", "needs_reupload", json!({})),
        item("ffffffffffffffffffffffffffffffff", "key_leak_review", json!({})),
        item("11111111111111111111111111111111", "held_for_review", json!({"appeal": "available", "hold_class": "copyright", "positions": [0, 2]})),
        item("22222222222222222222222222222222", "mystery", json!({})),
    ];
    let s = Script::new();
    s.say(200, mine(all.clone(), Some("1760000000000:0123456789abcdef0123456789abcdef")));
    let (mut m, _) = plain_model();
    m.open_list();
    m.frame(now(), &ready(&s));
    m.frame(now(), &ready(&s));
    draw(&ctx, &mut m);
    for it in &all {
        let id = it["id"].as_str().unwrap();
        m.select(id);
        draw(&ctx, &mut m);
        for panel in [Panel::ConfirmDelete, Panel::Appeal, Panel::Counter, Panel::CounterSent(CounterNoticeAck { restore_not_before: NOW, restore_by: NOW + DAY })] {
            m.panel = panel;
            m.problem = Some(Problem { message: "failed".into(), retry: false, retry_at: None, account: false });
            m.notice = Some(Notice::Info("note".into()));
            draw(&ctx, &mut m);
            fill(&mut m);
            draw(&ctx, &mut m);
        }
        m.back();
    }

    // the account page in every export state
    m.open_account_screen();
    for st in [
        ExportState::Idle,
        ExportState::Running { done: 0, total: 0 },
        ExportState::Running { done: 1, total: 3 },
        ExportState::Ready { size: 5_000_000, parts: 2 },
        ExportState::Saved { place: "/home/t/x.json".into(), parts: 1 },
        ExportState::Failed(Problem { message: "bad".into(), retry: true, retry_at: None, account: true }),
    ] {
        m.export = st;
        assert!(!draw(&ctx, &mut m));
    }
    m.set_base_url("https://pool.example.org");
    draw(&ctx, &mut m);
}

#[test]
fn the_close_button_closes_and_a_busy_action_keeps_the_window() {
    let ctx = egui::Context::default();
    crate::theme::setup(&ctx);
    let (mut m, _s, _) = opened(vec![item(ID, "live", json!({}))]);
    assert!(!m.busy_action());
    m.close();
    assert!(!m.open);
    assert!(m.jobs_idle());
}

#[test]
fn the_legal_base_is_its_own_setting_and_never_the_api_address() {
    use super::ShareSettings;
    assert_eq!(ShareSettings::legal_base_from(None), "");
    assert_eq!(ShareSettings::legal_base_from(Some("")), "");
    assert_eq!(ShareSettings::legal_base_from(Some("javascript:alert(1)")), "");
    assert_eq!(ShareSettings::legal_base_from(Some("http://example.org")), "", "plain http to a real host is refused");
    assert!(ShareSettings::legal_base_from(Some("https://school.example.org")).starts_with("https://school.example.org"));
    // A model that was never given a legal base shows names only, whatever server the account uses.
    let (m, _) = plain_model();
    assert!(m.legal_links().iter().all(|(_, u)| u.is_none()));
}
