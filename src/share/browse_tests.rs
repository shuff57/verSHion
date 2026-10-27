//! The pool browser's model against a scripted session (no network, no egui window): gating,
//! search and paging, the item preview, import into banks, reports, update checks, the rule that
//! nothing is overwritten, and headless smoke frames of every screen.

use std::collections::VecDeque;
use std::sync::mpsc::{Receiver, Sender, channel};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use serde_json::{Value, json};
use web_time::Instant;

use super::api::{ApiError, Status};
use super::browse::*;
use super::browse_text::{self as text, ReportChoice};
use super::merge::{Choice, RowKind, Row};
use super::model::{AccountModel, Runner, SettingsIo, server_mode};
use super::pool::*;
use super::publish::{AccountSnapshot, Gate};
use super::transport::{LocalFuture, Method, Request, Response};
use crate::model::{BankDetails, ImageRef, KEYLESS_SENTINEL, Question, QuestionBank, is_keyless};

const ID: &str = "0123456789abcdef0123456789abcdef";
const ID2: &str = "fedcba9876543210fedcba9876543210";

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
    fn say_item(&self, body: Vec<u8>) -> &Self {
        self.say_bytes(body)
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
    fn left(&self) -> usize {
        self.replies.lock().unwrap().len()
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

/// Pictures kept in memory: the "file name" is the hash of what was put.
#[derive(Default)]
struct Mem(Mutex<Vec<Vec<u8>>>);

impl ImageSink for Mem {
    fn put(&self, bytes: &[u8]) -> Result<ImageRef, String> {
        self.0.lock().unwrap().push(bytes.to_vec());
        Ok(ImageRef { file: format!("media/{}.webp", sha256_hex(bytes)), width: 2, height: 2 })
    }
}

// ---- fixtures -----------------------------------------------------------------------------------------------

fn now() -> Instant {
    Instant::now()
}

fn ready(s: &Arc<Script>) -> AccountSnapshot {
    AccountSnapshot::ready(s.clone())
}

fn list_item(id: &str, title: &str) -> Value {
    json!({"id": id, "kind": "bank", "title": title, "subject": "Biology", "course": "Bio 1", "grade": "9", "kinds": ["Mc", "Tf"], "question_count": 3,
           "version": 2, "hide_key": true, "license": "CC-BY-4.0", "license_check": {"status": "ok"}, "has_images": false,
           "author_name": "Ms Frizzle", "committed_at": 1760000000000i64, "is_author": false})
}

fn page(items: Vec<Value>, next: Option<&str>) -> Value {
    json!({"items": items, "next_cursor": next})
}

fn mc(stem: &str, key: bool) -> Value {
    json!({"id": format!("src-{stem}"), "stem": stem, "kind": {"Mc": {"answers": [{"text": "A", "correct": key}, {"text": "B", "correct": false}]}},
           "points": 1.0, "difficulty": null, "created": 5, "modified": 6, "keyless": !key})
}

fn bank_payload(stems: &[&str], key: bool) -> Value {
    json!({"schema_version": 2, "bank": {"id": "b-src", "name": "Cells", "questions": stems.iter().map(|s| mc(s, key)).collect::<Vec<_>>(), "created": 1, "modified": 2}})
}

fn head(form: &str, is_author: bool, version: u64) -> Value {
    json!({
        "item": {"id": ID, "kind": "bank", "title": "Cells", "subject": "Biology", "course": "Bio 1", "grade": "9", "kinds": ["Mc"], "question_count": 2,
                 "version": version, "publish_id": "pub", "license": "CC-BY-4.0", "hide_key": form == "stripped", "has_images": false,
                 "author_name": "Ms Frizzle", "committed_at": 1760000000000i64, "form": form, "is_author": is_author, "tags": [], "standards": []},
        "questions": [
            {"pos": 0, "qid": "q0", "chain": [
                {"author_name": "Mr A", "item_id": ID2, "qid": "x", "version": 1, "license": "CC-BY-4.0", "modified": false},
                {"author_name": "and 3 contributors between", "item_id": "", "qid": "", "version": 0, "license": "", "modified": true, "collapsed_licenses": ["CC0-1.0"]}]},
            {"pos": 1, "qid": "q1", "chain": []}
        ],
        "license_check": {"status": "ok", "basis": "recorded_sources"}
    })
}

fn streamed(head: &Value, payload: &Value, extra: Option<Value>) -> Vec<u8> {
    let mut h = head.clone();
    if let Some(Value::Object(e)) = extra {
        h.as_object_mut().unwrap().extend(e);
    }
    let h = h.to_string();
    format!("{},\"payload\":{}}}", &h[..h.len() - 1], payload).into_bytes()
}

fn import_extra(previously: &[u64]) -> Value {
    json!({"license": "CC-BY-4.0", "may_republish_as": ["CC-BY-4.0"], "previously_imported_versions": previously, "images": {"held": 0}, "recorded": true})
}

fn stripped_item(stems: &[&str]) -> Vec<u8> {
    streamed(&head("stripped", false, 2), &bank_payload(stems, false), None)
}

fn stripped_import(stems: &[&str], previously: &[u64]) -> Vec<u8> {
    streamed(&head("stripped", false, 2), &bank_payload(stems, false), Some(import_extra(previously)))
}

fn local_bank(id: &str, name: &str, stems: &[&str]) -> QuestionBank {
    QuestionBank {
        id: id.into(),
        name: name.into(),
        questions: stems.iter().enumerate().map(|(i, s)| serde_json::from_value(mc(s, true)).map(|mut q: Question| { q.id = format!("{id}-{i}"); q }).unwrap()).collect(),
        created: 1,
        modified: 2,
        details: BankDetails::default(),
    }
}

fn fresh(log: &ImportLog, images: &Arc<Mem>) -> BrowseModel {
    BrowseModel::new(Runner::Inline, images.clone(), log.clone()).with_retry(RetryPolicy::none())
}

/// Opened on the search tab, signed in, the first page (script `page`) loaded.
fn browsing(s: &Arc<Script>, items: Vec<Value>, next: Option<&str>) -> (BrowseModel, ImportLog, Arc<Mem>) {
    let (log, images) = (ImportLog::memory(), Arc::new(Mem::default()));
    let mut m = fresh(&log, &images);
    m.open_browse();
    s.say(200, page(items, next));
    m.frame(now(), &ready(s));
    m.frame(now(), &ready(s));
    assert_eq!(m.screen, Screen::Browse, "{:?}", m.notice);
    assert!(m.results.loaded, "{:?}", m.results.problem);
    (m, log, images)
}

/// One frame and one sync, the way the window does it.
fn settle(m: &mut BrowseModel, s: &Arc<Script>, banks: &mut Vec<QuestionBank>) -> bool {
    m.frame(now(), &ready(s));
    m.sync(banks)
}

/// The item is open and loaded.
fn viewing(m: &mut BrowseModel, s: &Arc<Script>, body: Vec<u8>, banks: &mut Vec<QuestionBank>) {
    s.say_item(body);
    m.open_item(ID, None);
    settle(m, s, banks);
    assert!(m.loaded().is_some(), "{:?}", m.item.as_ref().map(|i| format!("{:?}", i.load)));
}

fn problem_of_search(m: &BrowseModel) -> Problem {
    m.results.problem.clone().expect("a problem")
}

fn item_problem(m: &BrowseModel) -> Problem {
    match &m.item.as_ref().unwrap().load {
        Load::Failed(p) => p.clone(),
        other => panic!("{other:?}"),
    }
}

fn import_problem(m: &BrowseModel) -> Problem {
    match &m.item.as_ref().unwrap().import {
        ImportPanel::Failed(p) => p.clone(),
        other => panic!("{other:?}"),
    }
}

fn done(m: &BrowseModel) -> ImportDone {
    match &m.item.as_ref().unwrap().import {
        ImportPanel::Done(d) => (**d).clone(),
        other => panic!("{other:?}"),
    }
}

// ---- a closed model does nothing -------------------------------------------------------------------------

#[test]
fn a_closed_model_does_no_io_and_asks_nothing() {
    let s = Script::new();
    let (log, images) = (ImportLog::memory(), Arc::new(Mem::default()));
    let mut m = fresh(&log, &images);
    let mut banks = vec![local_bank("b1", "Mine", &["x"])];
    m.frame(now(), &ready(&s));
    assert!(!m.sync(&mut banks));
    m.search();
    m.check_updates();
    assert!(s.seen().is_empty());
    assert!(!m.open && !m.busy());
    assert!(m.banks.is_empty(), "the bank list is not even read while closed");
    // opening alone, before any account is ready, sends nothing either
    m.open_browse();
    m.frame(now(), &AccountSnapshot::of_gate(Gate::SignedOut));
    assert!(s.seen().is_empty());
    m.close();
    m.frame(now(), &ready(&s));
    assert!(s.seen().is_empty() && !m.busy());
}

// ---- the account gate -----------------------------------------------------------------------------------------

#[test]
fn nothing_is_sent_until_the_account_is_signed_in_and_approved() {
    let s = Script::new();
    for gate in [Gate::Checking, Gate::NotSetUp, Gate::NoServer, Gate::SignedOut, Gate::Unreachable, Gate::NotApproved(Status::Pending)] {
        let (log, images) = (ImportLog::memory(), Arc::new(Mem::default()));
        let mut m = fresh(&log, &images);
        m.open_browse();
        m.frame(now(), &AccountSnapshot::of_gate(gate.clone()));
        assert_eq!(m.screen, Screen::Gate, "{gate:?}");
        assert_eq!(m.gate, gate);
        assert!(!m.busy());
    }
    assert!(s.seen().is_empty());
}

#[test]
fn the_gate_asks_the_account_window_once_and_a_finished_lookup_reads_as_signed_out() {
    let (log, images) = (ImportLog::memory(), Arc::new(Mem::default()));
    let mut m = fresh(&log, &images);
    m.open_browse();
    m.frame(now(), &AccountSnapshot::of_gate(Gate::Checking));
    assert!(m.wants_account_check());
    m.account_check_asked();
    assert!(!m.wants_account_check());
    m.frame(now(), &AccountSnapshot::of_gate(Gate::Checking));
    assert_eq!(m.gate, Gate::SignedOut, "the quiet lookup finished without a sign-in");
    m.request_open_account();
    assert!(m.take_open_account());
    assert!(!m.take_open_account(), "once");
}

#[test]
fn becoming_ready_opens_the_browser_and_runs_the_first_search_once() {
    let s = Script::new();
    let (log, images) = (ImportLog::memory(), Arc::new(Mem::default()));
    let mut m = fresh(&log, &images);
    m.open_browse();
    m.frame(now(), &AccountSnapshot::of_gate(Gate::SignedOut));
    s.say(200, page(vec![list_item(ID, "Cells")], None));
    m.frame(now(), &ready(&s));
    m.frame(now(), &ready(&s));
    m.frame(now(), &ready(&s));
    assert_eq!(m.screen, Screen::Browse);
    assert_eq!(s.paths(), vec!["Get /items?limit=20"], "one search, nothing else (the imports list is empty)");
    assert_eq!(m.results.items.len(), 1);
}

#[test]
fn a_session_that_ends_mid_search_goes_back_to_the_gate_and_resumes() {
    let s = Script::new();
    let (mut m, _, _) = browsing(&s, vec![], None);
    s.fail(ApiError::SignedOut);
    m.search();
    m.frame(now(), &ready(&s));
    assert_eq!(m.screen, Screen::Gate);
    assert!(matches!(m.notice, Some(super::model::Notice::Info(_))));
    assert!(m.results.problem.is_none(), "it is not a search failure");
    s.say(200, page(vec![], None));
    m.retry_search();
    m.frame(now(), &ready(&s));
    assert_eq!(m.screen, Screen::Browse, "back where the person was");
}

#[test]
fn losing_the_sign_in_while_looking_at_an_item_comes_back_to_the_item() {
    let s = Script::new();
    let (mut m, _, _) = browsing(&s, vec![list_item(ID, "Cells")], None);
    let mut banks = vec![];
    viewing(&mut m, &s, stripped_item(&["a"]), &mut banks);
    m.frame(now(), &AccountSnapshot::of_gate(Gate::SignedOut));
    assert_eq!(m.screen, Screen::Gate);
    m.frame(now(), &ready(&s));
    assert_eq!(m.screen, Screen::Item);
    assert!(m.loaded().is_some(), "what was loaded is kept");
}

#[test]
fn closing_drops_a_running_request_and_its_late_answer_goes_nowhere() {
    let s = Script::new();
    let (log, images) = (ImportLog::memory(), Arc::new(Mem::default()));
    let mut m = BrowseModel::new(Runner::Background, images, log).with_retry(RetryPolicy::none());
    m.open_browse();
    let release = s.hold();
    s.say(200, page(vec![list_item(ID, "Late")], None));
    m.frame(now(), &ready(&s));
    assert!(m.busy());
    m.close();
    assert!(!m.busy() && !m.open);
    release.send(()).unwrap();
    std::thread::sleep(Duration::from_millis(150));
    m.frame(now(), &ready(&s));
    assert!(!m.results.loaded, "the late answer was not applied");
    s.say(200, page(vec![list_item(ID2, "Fresh")], None));
    m.open_browse();
    for _ in 0..200 {
        m.frame(now(), &ready(&s));
        if m.results.loaded {
            break;
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    assert_eq!(m.results.items[0].title, "Fresh");
}

// ---- search and paging -----------------------------------------------------------------------------------------

#[test]
fn the_form_becomes_the_pool_query_and_grade_stays_on_this_side() {
    let s = Script::new();
    let (mut m, _, _) = browsing(&s, vec![], None);
    m.form = SearchForm {
        q: " cell ".into(),
        subject: "Biology".into(),
        kind: "Mc".into(),
        tag: "t".into(),
        standard: "HS-LS1".into(),
        license: "CC-BY-4.0".into(),
        grade: "8".into(),
    };
    s.say(200, page(vec![], None));
    m.search();
    m.frame(now(), &ready(&s));
    let p = s.seen().last().unwrap().path.clone();
    for part in ["q=cell", "subject=Biology", "kind=Mc", "tag=t", "standard=HS-LS1", "license=CC-BY-4.0", "limit=20"] {
        assert!(p.contains(part), "{p} lacks {part}");
    }
    assert!(!p.contains("grade") && !p.contains("cursor"), "{p}");
}

#[test]
fn paging_follows_the_cursor_forward_and_back_with_the_search_that_made_the_page() {
    let s = Script::new();
    let (mut m, _, _) = browsing(&s, vec![list_item(ID, "One")], Some("c1"));
    assert!(m.can_next() && !m.can_prev());
    s.say(200, page(vec![list_item(ID2, "Two")], Some("c2")));
    m.next_page();
    m.frame(now(), &ready(&s));
    assert_eq!((m.results.page, m.results.items[0].title.as_str()), (1, "Two"));
    assert!(s.seen().last().unwrap().path.contains("cursor=c1"));
    // editing the form without searching does not change what next page asks
    m.form.q = "changed".into();
    s.say(200, page(vec![list_item(ID, "Three")], None));
    m.next_page();
    m.frame(now(), &ready(&s));
    let p = s.seen().last().unwrap().path.clone();
    assert!(p.contains("cursor=c2") && !p.contains("changed"), "{p}");
    assert!(!m.can_next(), "no cursor, no next page");
    s.say(200, page(vec![list_item(ID2, "Two")], Some("c2")));
    m.prev_page();
    m.frame(now(), &ready(&s));
    assert!(s.seen().last().unwrap().path.contains("cursor=c1"));
    assert_eq!(m.results.page, 1);
    s.say(200, page(vec![list_item(ID, "One")], Some("c1")));
    m.prev_page();
    m.frame(now(), &ready(&s));
    assert!(!s.seen().last().unwrap().path.contains("cursor"));
    assert!(!m.can_prev());
    m.prev_page();
    assert_eq!(s.left(), 0, "previous on the first page does nothing");
    // a new search starts again from the first page
    s.say(200, page(vec![], None));
    m.form.q = "new".into();
    m.search();
    m.frame(now(), &ready(&s));
    assert_eq!(m.results.page, 0);
}

#[test]
fn an_empty_page_and_the_grade_box_say_so() {
    let s = Script::new();
    let (mut m, _, _) = browsing(&s, vec![], None);
    assert!(m.results.items.is_empty() && m.results.loaded);
    s.say(200, page(vec![list_item(ID, "Cells"), {
        let mut v = list_item(ID2, "Atoms");
        v["grade"] = json!("11");
        v["course"] = json!("Chem");
        v
    }], None));
    m.search();
    m.frame(now(), &ready(&s));
    assert_eq!(m.visible().len(), 2);
    m.form.grade = "11".into();
    assert_eq!(m.visible().iter().map(|i| i.title.as_str()).collect::<Vec<_>>(), vec!["Atoms"]);
    m.form.grade = "chem".into();
    assert_eq!(m.visible().len(), 1, "course matches too, any case");
    m.form.grade = "7".into();
    assert!(m.visible().is_empty());
}

#[test]
fn search_failures_are_worded_and_retry_or_point_to_the_account() {
    let s = Script::new();
    let (mut m, _, _) = browsing(&s, vec![], None);
    // offline
    s.fail(ApiError::Offline("no route".into()));
    m.search();
    m.frame(now(), &ready(&s));
    let p = problem_of_search(&m);
    assert!(p.retry && p.retry_at.is_none() && !p.account);
    assert!(!m.results.loaded);
    s.say(200, page(vec![list_item(ID, "Cells")], None));
    m.retry_search();
    m.frame(now(), &ready(&s));
    assert!(m.results.loaded && m.results.problem.is_none());
    // too many requests: a wait
    s.say_after(429, 40, json!({"error": "rate_limited"}));
    m.search();
    m.frame(now(), &ready(&s));
    let p = problem_of_search(&m);
    assert!(p.retry && p.message.contains("Too many requests"), "{}", p.message);
    assert!(m.retry_wait(&p).is_some_and(|w| w.as_secs() > 0 && w.as_secs() <= 40));
    // today's browsing limit
    s.say_after(429, 7200, json!({"error": "daily_limit", "metric": "browse"}));
    m.search();
    m.frame(now(), &ready(&s));
    let p = problem_of_search(&m);
    assert!(p.message.contains("limit") && p.message.contains("hours"), "{}", p.message);
    // not approved (any more)
    s.say(403, json!({"error": "account_not_approved", "message": "Your account is on hold."}));
    m.search();
    m.frame(now(), &ready(&s));
    let p = problem_of_search(&m);
    assert!(p.account && !p.retry);
    assert_eq!(p.message, "Your account is on hold.");
    // the pool is not set up on the server
    s.say(503, json!({"error": "pool_not_configured"}));
    m.search();
    m.frame(now(), &ready(&s));
    let p = problem_of_search(&m);
    assert!(!p.retry && p.message.contains("not available"), "{}", p.message);
    // busy, and being upgraded
    s.say(503, json!({"error": "busy"}));
    m.search();
    m.frame(now(), &ready(&s));
    assert!(problem_of_search(&m).retry);
    s.say_after(503, 120, json!({"error": "pool_migrating"}));
    m.search();
    m.frame(now(), &ready(&s));
    let p = problem_of_search(&m);
    assert!(p.retry && p.message.contains("upgraded") && p.retry_at.is_some());
}

#[test]
fn an_unusable_search_is_stopped_before_anything_is_sent() {
    let s = Script::new();
    let (mut m, _, _) = browsing(&s, vec![], None);
    let before = s.seen().len();
    m.form.q = "x".repeat(101);
    m.search();
    m.frame(now(), &ready(&s));
    assert_eq!(s.seen().len(), before);
    let p = problem_of_search(&m);
    assert!(!p.retry && p.message.contains("search text"), "{}", p.message);
}

// ---- one item ---------------------------------------------------------------------------------------------------------

#[test]
fn an_item_opens_with_its_questions_licence_and_credits() {
    let s = Script::new();
    let (mut m, _, _) = browsing(&s, vec![list_item(ID, "Cells")], None);
    let mut banks = vec![];
    let summary = m.results.items[0].clone();
    s.say_item(stripped_item(&["Which organelle?", "Which stain?"]));
    m.open_item(ID, Some(summary));
    assert_eq!(m.screen, Screen::Item);
    assert!(matches!(m.item.as_ref().unwrap().load, Load::Loading));
    settle(&mut m, &s, &mut banks);
    assert_eq!(s.paths().last().unwrap(), &format!("Get /items/{ID}"));
    let l = m.loaded().unwrap();
    let v = l.view.as_ref().unwrap();
    assert_eq!(v.questions.len(), 2);
    assert!(v.is_clean());
    assert_eq!(l.fingerprints.len(), 2);
    assert_eq!(l.doc.head.questions[0].chain.len(), 2);
    assert!(m.import_blocker().is_none());
    m.back_to_list();
    assert_eq!(m.screen, Screen::Browse);
    assert!(m.item.is_none() && m.results.loaded, "the results are still there");
}

#[test]
fn an_item_that_is_gone_or_limited_says_so_and_a_retry_works() {
    let s = Script::new();
    let (mut m, _, _) = browsing(&s, vec![list_item(ID, "Cells")], None);
    let mut banks = vec![];
    s.say(404, json!({"error": "not_found"}));
    m.open_item(ID, None);
    settle(&mut m, &s, &mut banks);
    let p = item_problem(&m);
    assert!(!p.retry && p.message.contains("not available"), "{}", p.message);
    s.say_after(429, 30, json!({"error": "daily_limit", "metric": "item_reads"}));
    m.reload_item();
    settle(&mut m, &s, &mut banks);
    let p = item_problem(&m);
    assert!(p.retry && p.retry_at.is_some());
    s.say_item(stripped_item(&["a"]));
    m.reload_item();
    settle(&mut m, &s, &mut banks);
    assert!(m.loaded().is_some());
    // offline
    s.fail(ApiError::Offline("x".into()));
    m.reload_item();
    settle(&mut m, &s, &mut banks);
    assert!(item_problem(&m).retry);
}

#[test]
fn an_item_that_claims_a_hidden_key_but_shows_one_cannot_be_imported() {
    let s = Script::new();
    let (mut m, _, _) = browsing(&s, vec![], None);
    let mut banks = vec![];
    let leaky = streamed(&head("stripped", false, 2), &bank_payload(&["a"], true), None);
    viewing(&mut m, &s, leaky, &mut banks);
    assert!(m.import_blocker().unwrap().contains("hidden"));
    m.begin_import();
    assert!(matches!(m.item.as_ref().unwrap().import, ImportPanel::Closed));
}

#[test]
fn your_own_item_can_be_neither_imported_nor_reported() {
    let s = Script::new();
    let (mut m, _, _) = browsing(&s, vec![], None);
    let mut banks = vec![];
    viewing(&mut m, &s, streamed(&head("full", true, 2), &bank_payload(&["a"], true), None), &mut banks);
    assert!(m.import_blocker().unwrap().contains("your own"));
    m.begin_import();
    m.open_report();
    let it = m.item.as_ref().unwrap();
    assert!(matches!(it.import, ImportPanel::Closed) && it.report.is_none());
}

#[test]
fn the_credit_and_licence_wording_follows_the_pool() {
    let link = |f: &dyn Fn(&mut ChainLink)| {
        let mut l = ChainLink { author_name: "Mr A".into(), item_id: ID2.into(), version: 2, license: "CC-BY-4.0".into(), ..Default::default() };
        f(&mut l);
        text::chain_line(&l)
    };
    assert_eq!(link(&|_| {}), "Derived from the work of Mr A (version 2, CC BY 4.0 (credit me))");
    assert!(link(&|l| l.modified = true).ends_with("changed by the next author"));
    assert!(link(&|l| l.earlier_match = true).starts_with("Another author published these questions earlier: Mr A"));
    assert!(link(&|l| l.possible_source = true).starts_with("Possible source"));
    assert!(link(&|l| l.author_name = String::new()).contains("deleted teacher"));
    let ph = text::chain_line(&ChainLink { author_name: "and 3 contributors between".into(), collapsed_licenses: vec!["CC0-1.0".into()], ..Default::default() });
    assert_eq!(ph, "and 3 contributors between (licences: CC0 (public domain))");
    // server text is never markup
    assert_eq!(link(&|l| l.author_name = "<b>Mr\u{202e}A</b>\n".into()), "Derived from the work of <b>MrA</b> (version 2, CC BY 4.0 (credit me))");
    let check = |st: LicenseStatus| LicenseCheck { status: st, ..Default::default() };
    assert!(text::check_marker(&check(LicenseStatus::Ok)).is_none());
    for st in [LicenseStatus::Mismatch, LicenseStatus::Unverified, LicenseStatus::ClaimedIndependent, LicenseStatus::UnverifiedEarlierMatch, LicenseStatus::Other] {
        assert!(text::check_marker(&check(st)).is_some());
        assert!(!text::check_text(Some(&check(st))).is_empty());
    }
    assert_eq!(text::check_marker(&check(LicenseStatus::UnverifiedEarlierMatch)), Some("Another author published these questions earlier."));
    assert!(text::check_text(None).contains("did not say"));
    assert_eq!(text::author(""), "deleted teacher");
    assert_eq!(text::license_url("CC-BY-SA-4.0"), "https://creativecommons.org/licenses/by-sa/4.0/");
    assert_eq!(text::license_url("circle-only"), "");
}

// ---- import ---------------------------------------------------------------------------------------------------------------

#[test]
fn importing_into_the_bank_you_came_from_adds_to_it_and_keeps_the_credit() {
    let s = Script::new();
    let (log, images) = (ImportLog::memory(), Arc::new(Mem::default()));
    let mut m = fresh(&log, &images);
    let mut banks = vec![local_bank("b1", "Mine", &["old one", "old two"]), local_bank("b2", "Other", &["z"])];
    banks[0].details.description = "My notes".into();
    banks[0].details.author = "Me".into();
    m.open_for_bank("b1");
    s.say(200, page(vec![list_item(ID, "Cells")], None));
    settle(&mut m, &s, &mut banks);
    settle(&mut m, &s, &mut banks);
    viewing(&mut m, &s, stripped_item(&["new one", "new two"]), &mut banks);
    m.begin_import();
    let ImportPanel::Choosing(c) = &m.item.as_ref().unwrap().import else { panic!() };
    assert_eq!(c.dest, Dest::Existing("b1".into()), "the bank you came from is the default");
    assert_eq!(m.banks.len(), 2);
    let before = banks[0].questions.iter().map(|q| (q.id.clone(), q.stem.clone(), q.modified)).collect::<Vec<_>>();
    s.say_item(stripped_import(&["new one", "new two"], &[]));
    m.confirm_import();
    assert!(matches!(m.item.as_ref().unwrap().import, ImportPanel::Working));
    assert_eq!(s.paths().last().unwrap(), &format!("Post /items/{ID}/import"));
    let changed = settle(&mut m, &s, &mut banks);
    assert!(changed, "the caller saves the banks");
    let b = &banks[0];
    assert_eq!(b.questions.len(), 4);
    assert_eq!(b.questions[..2].iter().map(|q| (q.id.clone(), q.stem.clone(), q.modified)).collect::<Vec<_>>(), before, "what was there is untouched");
    assert_eq!(b.questions[2].stem, "new one");
    assert_ne!(b.questions[2].id, "src-new one", "new local ids");
    assert!(is_keyless(&b.questions[2]) && b.questions[2].explanation.as_deref() == Some(KEYLESS_SENTINEL), "hidden key: marked keyless");
    assert_eq!(b.details.author, "Me", "the bank's own details stay");
    assert!(b.details.description.starts_with("My notes\n") && b.details.description.contains("Ms Frizzle") && b.details.description.contains(ID), "{}", b.details.description);
    assert_eq!(banks[1].questions.len(), 1);
    let d = done(&m);
    assert_eq!((d.added, d.new_bank, d.bank_name.as_str(), d.version), (2, false, "Mine", 2));
    assert_eq!(d.license, "CC-BY-4.0");
    assert!(d.keyless && d.recorded);
    let recs = m.imported().to_vec();
    assert_eq!(recs.len(), 1);
    assert_eq!((recs[0].item_id.as_str(), recs[0].bank_id.as_str(), recs[0].version), (ID, "b1", 2));
    assert_eq!(recs[0].question_ids.len(), 2);
    assert_eq!(log.load().len(), 1, "saved");
}

#[test]
fn importing_as_a_new_bank_stores_author_and_licence_on_it() {
    let s = Script::new();
    let (mut m, log, _) = browsing(&s, vec![list_item(ID, "Cells")], None);
    let mut banks = vec![local_bank("b1", "Mine", &["x"])];
    settle(&mut m, &s, &mut banks);
    viewing(&mut m, &s, stripped_item(&["a", "b"]), &mut banks);
    m.begin_import();
    let ImportPanel::Choosing(c) = &m.item.as_ref().unwrap().import else { panic!() };
    assert_eq!(c.dest, Dest::New, "no bank to come from");
    assert_eq!(c.new_name, "Cells");
    m.set_new_name("Biology unit");
    s.say_item(stripped_import(&["a", "b"], &[]));
    m.confirm_import();
    assert!(settle(&mut m, &s, &mut banks));
    assert_eq!(banks.len(), 2);
    let b = &banks[1];
    assert_eq!(b.name, "Biology unit");
    assert_eq!(b.questions.len(), 2);
    assert_eq!(b.details.author, "Ms Frizzle");
    assert_eq!(b.details.license_name, "CC BY 4.0 (credit me)");
    assert_eq!(b.details.license_url, "https://creativecommons.org/licenses/by/4.0/");
    assert!(b.details.description.contains("Cells") && b.details.description.contains("version 2"));
    assert_eq!(banks[0].questions.len(), 1);
    assert!(done(&m).new_bank);
    assert_eq!(log.load()[0].bank_id, b.id);
}

#[test]
fn a_shared_answer_key_arrives_as_a_key_and_is_not_marked_keyless() {
    let s = Script::new();
    let (mut m, _, _) = browsing(&s, vec![], None);
    let mut banks = vec![];
    viewing(&mut m, &s, streamed(&head("full", false, 2), &bank_payload(&["a"], true), None), &mut banks);
    m.begin_import();
    s.say_item(streamed(&head("full", false, 2), &bank_payload(&["a"], true), Some(import_extra(&[]))));
    m.confirm_import();
    settle(&mut m, &s, &mut banks);
    let q = &banks[0].questions[0];
    assert!(!is_keyless(q) && q.explanation.is_none());
    assert!(matches!(&q.kind, crate::model::Kind::Mc { answers } if answers[0].correct));
    assert!(!done(&m).keyless);
}

#[test]
fn pictures_are_fetched_checked_stored_and_a_missing_one_is_reported_not_fatal() {
    let s = Script::new();
    let (mut m, _, images) = browsing(&s, vec![], None);
    let mut banks = vec![];
    let (pic_a, pic_b) = (b"PICTURE-A".to_vec(), b"PICTURE-B".to_vec());
    let (sha_a, sha_b) = (sha256_hex(&pic_a), sha256_hex(&pic_b));
    let mut payload = bank_payload(&["a", "b", "c"], false);
    for (i, sha) in [(0usize, &sha_a), (1, &sha_b), (2, &sha_a)] {
        payload["bank"]["questions"][i]["image"] = json!({"file": format!("media/{sha}.png"), "width": 4, "height": 4});
    }
    let mut h = head("stripped", false, 2);
    h["item"]["has_images"] = json!(true);
    viewing(&mut m, &s, streamed(&h, &payload, None), &mut banks);
    m.begin_import();
    s.say_item(streamed(&h, &payload, Some(import_extra(&[]))));
    s.say_bytes(pic_a.clone());
    s.say(404, json!({"error": "not_found"}));
    m.confirm_import();
    settle(&mut m, &s, &mut banks);
    let paths = s.paths();
    assert_eq!(paths.iter().filter(|p| p.contains("/blobs/")).count(), 2, "one fetch per distinct picture: {paths:?}");
    let qs = &banks[0].questions;
    assert_eq!(qs[0].image.as_ref().unwrap().file, format!("media/{}.webp", sha256_hex(&pic_a)));
    assert_eq!(qs[2].image.as_ref().map(|i| i.file.clone()), qs[0].image.as_ref().map(|i| i.file.clone()), "the same picture is stored once");
    assert!(qs[1].image.is_none(), "the lost one is left out");
    assert_eq!(images.0.lock().unwrap().len(), 1);
    let d = done(&m);
    assert_eq!((d.pictures, d.pictures_missing), (3, 1));
}

#[test]
fn a_picture_that_does_not_match_its_name_is_not_stored() {
    let s = Script::new();
    let (mut m, _, images) = browsing(&s, vec![], None);
    let mut banks = vec![];
    let sha = sha256_hex(b"REAL");
    let mut payload = bank_payload(&["a"], false);
    payload["bank"]["questions"][0]["image"] = json!({"file": format!("media/{sha}.png"), "width": 4, "height": 4});
    viewing(&mut m, &s, streamed(&head("stripped", false, 2), &payload, None), &mut banks);
    m.begin_import();
    s.say_item(streamed(&head("stripped", false, 2), &payload, Some(import_extra(&[]))));
    s.say_bytes(b"SOMETHING ELSE".to_vec());
    m.confirm_import();
    settle(&mut m, &s, &mut banks);
    assert!(images.0.lock().unwrap().is_empty());
    assert_eq!(done(&m).pictures_missing, 1);
    assert!(banks[0].questions[0].image.is_none());
}

#[test]
fn importing_again_is_a_second_copy_and_says_what_was_imported_before() {
    let s = Script::new();
    let (mut m, log, _) = browsing(&s, vec![], None);
    let mut banks = vec![local_bank("b1", "Mine", &["x"])];
    m.default_bank = Some("b1".into());
    for round in 0..2 {
        viewing(&mut m, &s, stripped_item(&["a"]), &mut banks);
        let prior = m.imports_of(ID);
        assert_eq!(prior.len(), round, "the screen can warn before the second import");
        m.begin_import();
        s.say_item(stripped_import(&["a"], if round == 0 { &[] } else { &[2] }));
        m.confirm_import();
        settle(&mut m, &s, &mut banks);
        m.back_to_list();
    }
    assert_eq!(banks[0].questions.len(), 3, "added twice, nothing replaced");
    let recs = log.load();
    assert_eq!(recs.len(), 1);
    assert_eq!(recs[0].question_ids.len(), 2, "one record per item and bank holds both copies");
    assert_eq!(recs[0].fingerprints.len(), 2);
}

#[test]
fn import_refusals_and_failures_are_worded_and_nothing_is_added() {
    let s = Script::new();
    let (mut m, log, _) = browsing(&s, vec![], None);
    let mut banks = vec![local_bank("b1", "Mine", &["x"])];
    viewing(&mut m, &s, stripped_item(&["a"]), &mut banks);
    let try_import = |m: &mut BrowseModel, banks: &mut Vec<QuestionBank>| {
        if matches!(m.item.as_ref().unwrap().import, ImportPanel::Failed(_)) {
            m.cancel_import();
        }
        m.begin_import();
        m.set_dest(Dest::Existing("b1".into()));
        m.confirm_import();
        settle(m, &s, banks);
    };
    // daily limit with a wait
    s.say_after(429, 5400, json!({"error": "daily_limit", "metric": "imports"}));
    try_import(&mut m, &mut banks);
    let p = import_problem(&m);
    assert!(p.retry && p.retry_at.is_some() && p.message.contains("copying items") && p.message.contains("hours"), "{}", p.message);
    // lifetime copy limit, evidence limit: no retry
    s.say(413, json!({"error": "imports_quota"}));
    try_import(&mut m, &mut banks);
    let p = import_problem(&m);
    assert!(!p.retry && p.message.contains("as many different items"), "{}", p.message);
    s.say(413, json!({"error": "evidence_quota_exceeded"}));
    try_import(&mut m, &mut banks);
    assert!(!import_problem(&m).retry);
    // being upgraded
    s.say_after(503, 90, json!({"error": "pool_migrating"}));
    try_import(&mut m, &mut banks);
    let p = import_problem(&m);
    assert!(p.retry && p.message.contains("upgraded"));
    // gone
    s.say(404, json!({"error": "not_found"}));
    try_import(&mut m, &mut banks);
    assert!(import_problem(&m).message.contains("not available"));
    // account
    s.say(403, json!({"error": "account_not_approved"}));
    try_import(&mut m, &mut banks);
    assert!(import_problem(&m).account);
    // offline
    s.fail(ApiError::Offline("x".into()));
    try_import(&mut m, &mut banks);
    assert!(import_problem(&m).retry);
    assert_eq!(banks[0].questions.len(), 1, "nothing was added by any of them");
    assert!(log.load().is_empty());
    // and the retry goes through
    s.say_item(stripped_import(&["a"], &[]));
    m.retry_import();
    assert!(settle(&mut m, &s, &mut banks));
    assert_eq!(banks[0].questions.len(), 2);
}

#[test]
fn a_sign_in_that_ends_during_an_import_keeps_the_item_and_adds_nothing() {
    let s = Script::new();
    let (mut m, _, _) = browsing(&s, vec![], None);
    let mut banks = vec![local_bank("b1", "Mine", &["x"])];
    viewing(&mut m, &s, stripped_item(&["a"]), &mut banks);
    m.begin_import();
    m.set_dest(Dest::Existing("b1".into()));
    s.fail(ApiError::SignedOut);
    m.confirm_import();
    settle(&mut m, &s, &mut banks);
    assert_eq!(m.screen, Screen::Gate);
    assert_eq!(banks[0].questions.len(), 1);
    m.frame(now(), &ready(&s));
    assert_eq!(m.screen, Screen::Item);
}

#[test]
fn a_bank_deleted_before_or_during_the_import_is_not_silently_replaced() {
    let s = Script::new();
    let (mut m, _, _) = browsing(&s, vec![], None);
    let mut banks = vec![local_bank("b1", "Mine", &["x"])];
    settle(&mut m, &s, &mut banks);
    viewing(&mut m, &s, stripped_item(&["a"]), &mut banks);
    m.begin_import();
    m.set_dest(Dest::Existing("b1".into()));
    // gone before pressing Import
    banks.clear();
    settle(&mut m, &s, &mut banks);
    m.confirm_import();
    assert!(import_problem(&m).message.contains("not there any more"));
    assert_eq!(s.paths().iter().filter(|p| p.contains("import")).count(), 0, "nothing was asked of the pool");
    // gone while the import ran
    banks.push(local_bank("b1", "Mine", &["x"]));
    settle(&mut m, &s, &mut banks);
    m.cancel_import();
    m.begin_import();
    m.set_dest(Dest::Existing("b1".into()));
    s.say_item(stripped_import(&["a"], &[]));
    m.confirm_import();
    banks.clear();
    assert!(!settle(&mut m, &s, &mut banks));
    assert!(import_problem(&m).message.contains("not there any more"));
    assert!(banks.is_empty());
}

#[test]
fn the_name_of_a_new_bank_is_cleaned_and_limited() {
    let s = Script::new();
    let (mut m, _, _) = browsing(&s, vec![], None);
    let mut banks = vec![];
    viewing(&mut m, &s, stripped_item(&["a"]), &mut banks);
    m.begin_import();
    m.set_new_name(&"n".repeat(500));
    let ImportPanel::Choosing(c) = &m.item.as_ref().unwrap().import else { panic!() };
    assert_eq!(c.new_name.chars().count(), 120);
    m.set_new_name("  ");
    s.say_item(stripped_import(&["a"], &[]));
    m.confirm_import();
    settle(&mut m, &s, &mut banks);
    assert_eq!(banks[0].name, "Cells", "an empty name falls back to the item's title");
}

#[test]
fn apply_import_writes_seconds_into_a_bank_not_milliseconds() {
    let q = |s: &str| serde_json::from_value::<Question>(mc(s, true)).unwrap();
    let head: ItemHead = serde_json::from_value(head("full", false, 4)).unwrap();
    let imp = Imported { head, info: ImportInfo::default(), questions: vec![q("a")], fingerprints: vec!["f".into()], pictures: 0, pictures_missing: 0, keyless: false, at_ms: 1_792_864_237_304, theirs: vec![] };
    let mut banks = Vec::new();
    let a = apply_import(&mut banks, &Dest::New, "x", &imp).unwrap();
    assert_eq!((banks[0].created, banks[0].modified), (1_792_864_237, 1_792_864_237), "the app keeps bank times in seconds");
    banks[0].modified = 1_700_000_000;
    apply_import(&mut banks, &Dest::Existing(a.bank_id), "", &imp).unwrap();
    assert_eq!(banks[0].modified, 1_792_864_237);
}

#[test]
fn apply_import_is_pure_and_checks_its_destination() {
    let q = |s: &str| serde_json::from_value::<Question>(mc(s, true)).unwrap();
    let head: ItemHead = serde_json::from_value(head("full", false, 4)).unwrap();
    let imp = Imported { head, info: ImportInfo::default(), questions: vec![q("a")], fingerprints: vec!["f".into()], pictures: 0, pictures_missing: 0, keyless: false, at_ms: 99_000, theirs: vec![] };
    let mut banks = vec![local_bank("b1", "Mine", &["x"])];
    assert!(apply_import(&mut banks, &Dest::Existing("nope".into()), "", &imp).is_err());
    assert_eq!(banks[0].questions.len(), 1);
    let a = apply_import(&mut banks, &Dest::Existing("b1".into()), "", &imp).unwrap();
    assert_eq!((a.added, a.new_bank, banks[0].modified), (1, false, 99));
    assert!(banks[0].details.description.contains("version 4"), "an empty details block gets the credit");
    assert_eq!(banks[0].details.license_name, "", "a bank that already had questions is not given one licence");
    let a = apply_import(&mut banks, &Dest::New, "N", &imp).unwrap();
    assert!(a.new_bank && banks.len() == 2 && banks[1].name == "N" && !banks[1].details.license_name.is_empty());
    let line = attribution_line("<b>T</b>", "", "CC0-1.0", ID, 3);
    assert!(line.contains("deleted teacher") && line.contains("CC0 (public domain)"));
}

#[test]
fn a_question_alone_is_imported_too() {
    let s = Script::new();
    let (mut m, _, _) = browsing(&s, vec![], None);
    let mut banks = vec![];
    let mut h = head("stripped", false, 1);
    h["item"]["kind"] = json!("question");
    let payload = json!({"schema_version": 1, "question": mc("single", false)});
    viewing(&mut m, &s, streamed(&h, &payload, None), &mut banks);
    m.begin_import();
    s.say_item(streamed(&h, &payload, Some(import_extra(&[]))));
    m.confirm_import();
    settle(&mut m, &s, &mut banks);
    assert_eq!(banks[0].questions.len(), 1);
    assert_eq!(banks[0].questions[0].stem, "single");
}

#[test]
fn fingerprints_ignore_ids_and_dates_but_not_the_text() {
    let a = bank_payload(&["same", "other"], false);
    let mut b = a.clone();
    b["bank"]["questions"][0]["id"] = json!("different");
    b["bank"]["questions"][0]["modified"] = json!(12345);
    assert_eq!(fingerprints_of(a.to_string().as_bytes()), fingerprints_of(b.to_string().as_bytes()));
    b["bank"]["questions"][0]["stem"] = json!("edited");
    assert_ne!(fingerprints_of(a.to_string().as_bytes())[0], fingerprints_of(b.to_string().as_bytes())[0]);
    assert!(fingerprints_of(b"not json").is_empty());
}

// ---- reports ----------------------------------------------------------------------------------------------------------------

fn reporting(s: &Arc<Script>) -> (BrowseModel, Vec<QuestionBank>) {
    let (mut m, _, _) = browsing(s, vec![], None);
    let mut banks = vec![];
    viewing(&mut m, s, stripped_item(&["a"]), &mut banks);
    m.open_report();
    assert!(m.item.as_ref().unwrap().report.is_some());
    (m, banks)
}

#[test]
fn every_report_reason_goes_out_as_the_pool_expects() {
    let cases = [
        (ReportChoice::WrongAnswer, "wrong_answer", None),
        (ReportChoice::StudentData, "student_data", None),
        (ReportChoice::Copyright, "copyright", None),
        (ReportChoice::Inappropriate, "inappropriate", None),
        (ReportChoice::Other, "other", Some("see question 2")),
        (ReportChoice::KeyLeak, "key_leak", None),
    ];
    for (choice, wire, note) in cases {
        let s = Script::new();
        let (mut m, mut banks) = reporting(&s);
        assert!(m.report_blocker().is_some(), "nothing chosen yet");
        m.send_report();
        assert_eq!(s.left(), 0);
        m.set_report_choice(choice);
        if let Some(n) = note {
            m.set_report_note(n);
        }
        assert!(m.report_blocker().is_none());
        s.say(200, json!({"ok": true}));
        m.send_report();
        assert!(matches!(m.item.as_ref().unwrap().report.as_ref().unwrap().state, ReportState::Sending));
        settle(&mut m, &s, &mut banks);
        let sent = s.seen().last().unwrap().clone();
        assert_eq!(sent.path, format!("/items/{ID}/report"));
        assert_eq!(sent.json()["reason"], wire);
        if choice == ReportChoice::KeyLeak {
            assert!(sent.json().get("note").is_none(), "no marker note any more");
        }
        if let Some(n) = note {
            assert_eq!(sent.json()["note"], n);
        }
        let ReportState::Sent(msg) = &m.item.as_ref().unwrap().report.as_ref().unwrap().state else { panic!() };
        assert_eq!(msg, text::THANKS);
        let n = s.seen().len();
        m.send_report();
        assert_eq!(s.seen().len(), n, "a sent report is not sent twice");
    }
}

#[test]
fn the_other_reason_needs_a_note_and_the_note_is_limited() {
    let s = Script::new();
    let (mut m, _) = reporting(&s);
    m.set_report_choice(ReportChoice::Other);
    assert_eq!(m.report_blocker(), Some("Say what is wrong in the note."));
    m.set_report_note("   ");
    assert!(m.report_blocker().is_some());
    m.set_report_note(&"w".repeat(900));
    assert_eq!(m.item.as_ref().unwrap().report.as_ref().unwrap().note.chars().count(), 500);
    assert!(m.report_blocker().is_none());
    m.close_report();
    assert!(m.item.as_ref().unwrap().report.is_none());
}

#[test]
fn report_problems_are_worded_and_a_report_is_never_resent_by_itself() {
    let s = Script::new();
    let (mut m, mut banks) = reporting(&s);
    m.set_report_choice(ReportChoice::WrongAnswer);
    let state = |m: &BrowseModel| m.item.as_ref().unwrap().report.as_ref().unwrap().state.clone();
    s.say_after(429, 3600, json!({"error": "daily_limit", "metric": "reports"}));
    m.send_report();
    settle(&mut m, &s, &mut banks);
    let ReportState::Failed(p) = state(&m) else { panic!() };
    assert!(p.message.contains("limit") && p.message.contains("reports"), "{}", p.message);
    assert!(!p.retry, "the person presses Send again, nothing retries a report on its own");
    s.say(400, json!({"error": "own_item"}));
    m.send_report();
    settle(&mut m, &s, &mut banks);
    let ReportState::Failed(p) = state(&m) else { panic!() };
    assert_eq!(p.message, text::REPORT_OWN);
    s.fail(ApiError::Offline("x".into()));
    m.send_report();
    settle(&mut m, &s, &mut banks);
    assert!(matches!(state(&m), ReportState::Failed(_)));
    assert_eq!(s.paths().iter().filter(|p| p.ends_with("/report")).count(), 3, "one request per press");
    s.say(404, json!({"error": "not_found"}));
    m.send_report();
    settle(&mut m, &s, &mut banks);
    let ReportState::Failed(p) = state(&m) else { panic!() };
    assert!(p.message.contains("not available"));
    // the sign-in ended: back to the gate, the report form is kept
    s.fail(ApiError::SignedOut);
    m.send_report();
    settle(&mut m, &s, &mut banks);
    assert_eq!(m.screen, Screen::Gate);
    m.frame(now(), &ready(&s));
    assert_eq!(m.screen, Screen::Item);
    assert!(m.item.as_ref().unwrap().report.is_some());
}

// ---- update checks ---------------------------------------------------------------------------------------------------------------

fn record(item: &str, bank: &str, version: u64) -> ImportedRecord {
    ImportedRecord {
        circle: None,
        item_id: item.into(),
        bank_id: bank.into(),
        bank_name: "Mine".into(),
        version,
        license: "CC-BY-4.0".into(),
        title: "Cells".into(),
        author: "Ms Frizzle".into(),
        imported_at: 1000,
        question_ids: vec![],
        fingerprints: vec![],
        newer: None,
        status: None,
        checked_at: now_ms(),
        note: None,
        base: vec![],
    }
}

fn model_with(records: Vec<ImportedRecord>) -> (BrowseModel, ImportLog) {
    let log = ImportLog::memory();
    log.save(&records).unwrap();
    (fresh(&log, &Arc::new(Mem::default())), log)
}

#[test]
fn the_update_check_asks_once_for_all_distinct_items_and_records_every_answer() {
    let s = Script::new();
    let (mut m, log) = model_with(vec![record(ID, "b1", 1), record(ID, "b2", 1), record(ID2, "b1", 3), record("aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa", "b1", 1), record("bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb", "b1", 1)]);
    m.open_imported();
    m.frame(now(), &ready(&s));
    assert!(s.seen().is_empty(), "everything was checked a moment ago");
    s.say(200, json!({"items": [
        {"id": ID, "status": "newer", "version": 2, "license": "CC-BY-SA-4.0", "license_version": 2},
        {"id": ID2, "status": "current", "version": 3},
        {"id": "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa", "status": "under_review", "reason": "held"},
        {"id": "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb", "status": "removed", "reason": "deleted"},
    ]}));
    m.check_updates();
    assert!(m.checking());
    m.frame(now(), &ready(&s));
    let sent = s.seen().last().unwrap().clone();
    assert_eq!(sent.path, "/items/versions");
    let ids: Vec<String> = sent.json()["ids"].as_array().unwrap().iter().map(|v| v.as_str().unwrap().to_string()).collect();
    assert_eq!(ids.len(), 4, "distinct ids");
    let recs = log.load();
    let by = |item: &str, bank: &str| recs.iter().find(|r| r.item_id == item && r.bank_id == bank).unwrap().clone();
    assert_eq!(by(ID, "b1").newer, Some(NewerInfo { version: 2, license: Some("CC-BY-SA-4.0".into()), license_version: Some(2) }));
    assert!(by(ID, "b2").has_update(), "every bank imported from it hears about it");
    assert!(!by(ID2, "b1").has_update());
    assert_eq!(by("aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa", "b1").status.as_deref(), Some("under_review"));
    assert_eq!(by("bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb", "b1").status.as_deref(), Some("removed"));
    assert_eq!(m.update_count(), 2);
    assert!(m.update_for_bank("b1").is_some() && m.update_for_bank("zz").is_none());
    // the next check clears what is no longer true
    s.say(200, json!({"items": [{"id": ID, "status": "current", "version": 1}]}));
    m.check_updates();
    m.frame(now(), &ready(&s));
    assert_eq!(m.update_count(), 0);
}

#[test]
fn a_newer_version_that_is_not_newer_than_the_import_shows_no_badge() {
    let mut r = record(ID, "b1", 3);
    r.newer = Some(NewerInfo { version: 3, license: None, license_version: None });
    assert!(!r.has_update());
    r.newer.as_mut().unwrap().version = 4;
    assert!(r.has_update());
}

#[test]
fn a_failed_check_is_quiet_and_changes_nothing() {
    let s = Script::new();
    let (mut m, log) = model_with(vec![record(ID, "b1", 1)]);
    m.open_imported();
    m.frame(now(), &ready(&s));
    s.fail(ApiError::Offline("x".into()));
    m.check_updates();
    m.frame(now(), &ready(&s));
    let p = m.check_problem.clone().unwrap();
    assert!(p.retry);
    assert_eq!(log.load(), vec![record(ID, "b1", 1)].into_iter().map(|mut r| { r.checked_at = log.load()[0].checked_at; r }).collect::<Vec<_>>());
    assert_eq!(m.screen, Screen::Browse, "the window carries on");
    s.say_after(429, 60, json!({"error": "rate_limited"}));
    m.check_updates();
    m.frame(now(), &ready(&s));
    assert!(m.check_problem.as_ref().unwrap().retry_at.is_some());
    // signed out during a check: the gate
    s.fail(ApiError::SignedOut);
    m.check_updates();
    m.frame(now(), &ready(&s));
    assert_eq!(m.screen, Screen::Gate);
    assert!(m.check_problem.is_none() || m.screen == Screen::Gate);
}

#[test]
fn opening_checks_stale_imports_once_and_never_fresh_ones() {
    let s = Script::new();
    let mut old = record(ID, "b1", 1);
    old.checked_at = 0;
    let (mut m, log) = model_with(vec![old]);
    s.say(200, json!({"items": [{"id": ID, "status": "newer", "version": 2}]}));
    m.open_imported();
    m.frame(now(), &ready(&s));
    m.frame(now(), &ready(&s));
    m.frame(now(), &ready(&s));
    assert_eq!(s.paths(), vec!["Post /items/versions"], "one check per opening");
    assert!(log.load()[0].has_update());
    assert!(log.load()[0].checked_at > 0);
    // opened again straight away: still fresh
    m.close();
    m.open_imported();
    m.frame(now(), &ready(&s));
    assert_eq!(s.seen().len(), 1);
}

#[test]
fn an_empty_imports_list_asks_nothing() {
    let s = Script::new();
    let (mut m, _) = model_with(vec![]);
    m.open_imported();
    m.frame(now(), &ready(&s));
    m.check_updates();
    assert!(s.seen().is_empty());
}

#[test]
fn the_badge_for_a_bank_needs_no_network_and_a_corrupt_log_is_ignored() {
    let (mut m, _) = model_with(vec![{
        let mut r = record(ID, "b1", 1);
        r.newer = Some(NewerInfo { version: 2, license: None, license_version: None });
        r
    }]);
    assert!(m.update_for_bank("b1").is_some());
    assert!(m.update_for_bank("b2").is_none());
    let (mut empty, _) = model_with(vec![]);
    assert!(empty.update_for_bank("b1").is_none() && empty.imported().is_empty());
    let mut ui = super::browse_ui::BrowseUi::default();
    ui.model = fresh(&ImportLog::memory(), &Arc::new(Mem::default()));
    assert!(!ui.has_update("b1"));
}

// ---- update from the pool: what changed, and nothing overwritten ---------------------------------------------------------

/// A bank imported at version 1 (two questions) and a record of it, the pool now at version 4.
fn updating() -> (Arc<Script>, BrowseModel, ImportLog, Vec<QuestionBank>) {
    let s = Script::new();
    let old = bank_payload(&["keep me", "will be edited"], false);
    let mut bank = local_bank("b1", "Mine", &["keep me", "will be edited"]);
    bank.questions[0].id = "q-a".into();
    bank.questions[1].id = "q-b".into();
    bank.questions[0].modified = 5000;
    let mut r = record(ID, "b1", 1);
    r.question_ids = vec!["q-a".into(), "q-b".into()];
    r.fingerprints = fingerprints_of(old.to_string().as_bytes());
    r.newer = Some(NewerInfo { version: 4, license: Some("CC-BY-SA-4.0".into()), license_version: Some(2) });
    let (mut m, log) = model_with(vec![r]);
    let mut banks = vec![bank];
    let new = streamed(&{
        let mut h = head("stripped", false, 4);
        h["item"]["license"] = json!("CC-BY-SA-4.0");
        h
    }, &bank_payload(&["keep me", "edited in the pool", "brand new"], false), None);
    m.open_update("b1");
    s.say_item(new);
    settle(&mut m, &s, &mut banks);
    settle(&mut m, &s, &mut banks);
    (s, m, log, banks)
}

#[test]
fn update_from_pool_opens_the_new_version_and_says_what_changed_before_anything_else() {
    let (s, m, _, _) = updating();
    assert_eq!(m.screen, Screen::Item);
    assert_eq!(s.paths(), vec![format!("Get /items/{ID}")], "reading, nothing imported yet");
    let u = m.item.as_ref().unwrap().update.as_ref().unwrap();
    let c = u.changes.clone().unwrap();
    assert_eq!(
        c,
        Changes {
            old_version: 1,
            new_version: 4,
            added: 2,
            removed: 1,
            unchanged: 1,
            edited_locally: 1,
            deleted_locally: 0,
            license: Some(("CC-BY-4.0".into(), "CC-BY-SA-4.0".into())),
            bank_missing: false,
            nothing_new: false,
        }
    );
}

#[test]
fn an_update_is_always_a_new_bank_and_the_old_one_is_never_touched() {
    let (s, mut m, log, mut banks) = updating();
    let before: Vec<(String, String, i64)> = banks[0].questions.iter().map(|q| (q.id.clone(), q.stem.clone(), q.modified)).collect();
    let (details_before, name_before) = (banks[0].details.clone(), banks[0].name.clone());
    m.begin_import();
    let ImportPanel::Choosing(c) = &m.item.as_ref().unwrap().import else { panic!() };
    assert_eq!(c.dest, Dest::New);
    assert_eq!(c.new_name, "Cells (version 4)");
    m.set_dest(Dest::Existing("b1".into()));
    let ImportPanel::Choosing(c) = &m.item.as_ref().unwrap().import else { panic!() };
    assert_eq!(c.dest, Dest::New, "an update can not be pointed at the old bank");
    s.say_item(streamed(&{
        let mut h = head("stripped", false, 4);
        h["item"]["license"] = json!("CC-BY-SA-4.0");
        h
    }, &bank_payload(&["keep me", "edited in the pool", "brand new"], false), Some(json!({"license": "CC-BY-SA-4.0", "may_republish_as": ["CC-BY-SA-4.0"], "previously_imported_versions": [1], "images": {"held": 0}, "recorded": true}))));
    m.confirm_import();
    assert!(settle(&mut m, &s, &mut banks));
    assert_eq!(banks.len(), 2);
    let old: Vec<(String, String, i64)> = banks[0].questions.iter().map(|q| (q.id.clone(), q.stem.clone(), q.modified)).collect();
    assert_eq!(old, before, "the old bank, edits included, is exactly as it was");
    assert_eq!((banks[0].details.clone(), banks[0].name.clone()), (details_before, name_before));
    assert_eq!(banks[1].name, "Cells (version 4)");
    assert_eq!(banks[1].questions.len(), 3);
    assert_eq!(banks[1].details.license_name, "CC BY-SA 4.0 (credit me, share alike)", "the new licence travels with the new bank");
    // the old record's badge is settled and says where the new version went; the new bank is tracked
    let recs = log.load();
    assert_eq!(recs.len(), 2);
    let old_rec = recs.iter().find(|r| r.bank_id == "b1").unwrap();
    assert!(old_rec.newer.is_none() && old_rec.note.as_deref().unwrap().contains("Cells (version 4)"));
    let new_rec = recs.iter().find(|r| r.bank_id == banks[1].id).unwrap();
    assert_eq!((new_rec.version, new_rec.license.as_str()), (4, "CC-BY-SA-4.0"));
    assert!(m.update_for_bank("b1").is_none());
}

#[test]
fn local_deletions_and_a_missing_bank_are_noticed_in_the_changes() {
    let (_s, mut m, _, mut banks) = updating();
    banks[0].questions.remove(1);
    m.item.as_mut().unwrap().update.as_mut().unwrap().changes = None;
    m.sync(&mut banks);
    let c = m.item.as_ref().unwrap().update.as_ref().unwrap().changes.clone().unwrap();
    assert_eq!((c.deleted_locally, c.edited_locally), (1, 1));
    banks.clear();
    m.item.as_mut().unwrap().update.as_mut().unwrap().changes = None;
    m.sync(&mut banks);
    assert!(m.item.as_ref().unwrap().update.as_ref().unwrap().changes.as_ref().unwrap().bank_missing);
}

#[test]
fn open_update_with_two_sources_stays_on_the_imports_list() {
    let s = Script::new();
    let mut a = record(ID, "b1", 1);
    a.newer = Some(NewerInfo { version: 2, license: None, license_version: None });
    let mut b = record(ID2, "b1", 1);
    b.newer = Some(NewerInfo { version: 2, license: None, license_version: None });
    let (mut m, _) = model_with(vec![a, b]);
    m.open_update("b1");
    m.frame(now(), &ready(&s));
    m.frame(now(), &ready(&s));
    assert_eq!((m.screen, m.tab), (Screen::Browse, Tab::Imported));
    assert!(s.seen().is_empty());
    let rec = m.imported().iter().find(|r| r.item_id == ID2).unwrap().clone();
    s.say_item(stripped_item(&["a"]));
    m.start_update(&rec);
    assert_eq!(m.item.as_ref().unwrap().id, ID2);
    assert!(m.item.as_ref().unwrap().update.is_some());
}

// ---- the window and the account ----------------------------------------------------------------------------------------------

#[test]
#[allow(clippy::field_reassign_with_default)]
fn the_browser_is_asked_for_through_the_context_and_defers_to_the_account_window() {
    use super::browse_ui::{BrowseRequest, BrowseUi, request};
    use super::ui::AccountUi;
    let ctx = egui::Context::default();
    crate::theme::setup(&ctx);
    let mut account = AccountUi::default();
    account.model = AccountModel::new(Arc::new(|_| panic!("no client may be built")), Runner::Inline, SettingsIo::memory()).with_server_mode(server_mode(None, false));
    let mut ui = BrowseUi::default();
    ui.model = fresh(&ImportLog::memory(), &Arc::new(Mem::default()));
    let mut banks = vec![local_bank("b1", "Mine", &["x"])];
    let frame = |ctx: &egui::Context, ui: &mut BrowseUi, account: &mut AccountUi, banks: &mut Vec<QuestionBank>| {
        let mut changed = false;
        let out = ctx.run_ui(egui::RawInput::default(), |_| {
            changed = ui.show(ctx, account, banks);
        });
        out.drop_without_applying_deltas();
        changed
    };
    assert!(!frame(&ctx, &mut ui, &mut account, &mut banks));
    assert!(!ui.is_open() && !account.is_open());
    request(&ctx, BrowseRequest::ForBank("b1".into()));
    frame(&ctx, &mut ui, &mut account, &mut banks);
    assert!(ui.is_open());
    assert_eq!(ui.model.screen, Screen::Gate);
    assert_eq!(ui.model.gate, Gate::NotSetUp);
    assert_eq!(ui.model.default_bank.as_deref(), Some("b1"));
    assert!(!account.is_open(), "the Account window is not opened behind the person's back");
    ui.model.request_open_account();
    frame(&ctx, &mut ui, &mut account, &mut banks);
    assert!(account.visible());
    for r in [BrowseRequest::Browse, BrowseRequest::Imported, BrowseRequest::Update("b1".into())] {
        request(&ctx, r);
        frame(&ctx, &mut ui, &mut account, &mut banks);
        assert!(ui.is_open());
    }
    ui.open_for_bank("b1");
    assert_eq!(ui.model.default_bank.as_deref(), Some("b1"));
    ui.open_update("b1");
    ui.model.close();
    assert!(!ui.is_open());
}

// ---- headless frames of every screen and state ---------------------------------------------------------------------------------

fn draw(ctx: &egui::Context, m: &mut BrowseModel) -> bool {
    let mut closed = false;
    for i in 0..2 {
        let input = egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(900.0, 1400.0))),
            time: Some(i as f64 * 0.1),
            ..Default::default()
        };
        let out = ctx.run_ui(input, |ui| {
            closed |= super::browse_ui::page(ui, m);
        });
        out.drop_without_applying_deltas();
    }
    closed
}

#[test]
fn every_screen_and_state_draws_without_panicking() {
    let ctx = egui::Context::default();
    crate::theme::setup(&ctx);
    let s = Script::new();

    // gate pages
    for g in [Gate::Checking, Gate::NotSetUp, Gate::NoServer, Gate::SignedOut, Gate::Unreachable, Gate::NotApproved(Status::Pending), Gate::NotApproved(Status::Unverified)] {
        let (log, images) = (ImportLog::memory(), Arc::new(Mem::default()));
        let mut m = fresh(&log, &images);
        m.open_browse();
        m.frame(now(), &AccountSnapshot::of_gate(g));
        assert!(!draw(&ctx, &mut m));
    }

    // search tab: results with every marker, empty, grade mismatch, problems, loading
    let mut odd = list_item(ID2, "<script>alert(1)</script> \u{202e}Cells");
    odd["license_check"] = json!({"status": "unverified_earlier_match"});
    odd["author_name"] = json!("");
    odd["hide_key"] = json!(false);
    let (mut m, _, _) = browsing(&s, vec![list_item(ID, "Cells"), odd], Some("c1"));
    draw(&ctx, &mut m);
    m.form.grade = "zz".into();
    draw(&ctx, &mut m);
    m.form = SearchForm { q: "x".into(), subject: "a".into(), kind: "Mc".into(), tag: "t".into(), standard: "s".into(), license: "CC0-1.0".into(), grade: String::new() };
    s.say(200, page(vec![], None));
    m.search();
    m.frame(now(), &ready(&s));
    draw(&ctx, &mut m);
    for (st, body, secs) in [(429u16, json!({"error": "rate_limited"}), 30u64), (403, json!({"error": "account_not_approved"}), 0), (503, json!({"error": "pool_not_configured"}), 0)] {
        if secs > 0 { s.say_after(st, secs, body) } else { s.say(st, body) };
        m.search();
        m.frame(now(), &ready(&s));
        draw(&ctx, &mut m);
    }
    m.results.loaded = false;
    m.results.problem = None;
    draw(&ctx, &mut m);

    // imports tab: empty, and with every kind of record
    m.tab = Tab::Imported;
    draw(&ctx, &mut m);
    let mut recs = vec![record(ID, "b1", 1), record(ID2, "b2", 1), record("aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa", "b1", 1), record("bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb", "b1", 1)];
    recs[0].newer = Some(NewerInfo { version: 2, license: Some("CC-BY-SA-4.0".into()), license_version: Some(1) });
    recs[1].note = Some("Version 2 was imported as the bank \"X\".".into());
    recs[2].status = Some("under_review".into());
    recs[3].status = Some("removed".into());
    let (mut m2, _) = model_with(recs);
    m2.open_imported();
    m2.frame(now(), &ready(&s));
    draw(&ctx, &mut m2);
    m2.check_problem = Some(Problem { message: "offline".into(), retry: true, retry_at: None, account: false });
    draw(&ctx, &mut m2);

    // an item: loading, failed, loaded (stripped with credits, full, mismatch), own
    let (mut m, _, _) = browsing(&s, vec![list_item(ID, "Cells")], None);
    let mut banks = vec![local_bank("b1", "Mine", &["x"]), local_bank("b2", "Other <b>", &["y"])];
    let summary = m.results.items[0].clone();
    s.say(404, json!({"error": "not_found"}));
    m.open_item(ID, Some(summary));
    draw(&ctx, &mut m);
    settle(&mut m, &s, &mut banks);
    draw(&ctx, &mut m);
    let mut mismatch = head("stripped", false, 2);
    mismatch["license_check"] = json!({"status": "mismatch", "reason": "<b>reason</b>"});
    for (body, own) in [
        (stripped_item(&["Which organelle?\n\n**bold** and `code`", "b"]), false),
        (streamed(&head("full", false, 2), &bank_payload(&["a", "b"], true), None), false),
        (streamed(&mismatch, &bank_payload(&["a"], false), None), false),
        (streamed(&head("full", true, 2), &bank_payload(&["a"], true), None), true),
        (streamed(&head("stripped", false, 2), &bank_payload(&["a"], true), None), false),
        (streamed(&head("stripped", false, 2), &json!({"nonsense": 1}), None), false),
    ] {
        s.say_item(body);
        m.reload_item();
        settle(&mut m, &s, &mut banks);
        draw(&ctx, &mut m);
        let _ = own;
    }
    // a good one: every import panel state
    s.say_item(stripped_item(&["a", "b"]));
    m.reload_item();
    settle(&mut m, &s, &mut banks);
    m.begin_import();
    draw(&ctx, &mut m);
    m.set_dest(Dest::Existing("b2".into()));
    draw(&ctx, &mut m);
    m.set_dest(Dest::New);
    draw(&ctx, &mut m);
    m.item.as_mut().unwrap().import = ImportPanel::Working;
    draw(&ctx, &mut m);
    m.item.as_mut().unwrap().import = ImportPanel::Failed(Problem { message: "failed".into(), retry: true, retry_at: Some(now() + Duration::from_secs(20)), account: true });
    draw(&ctx, &mut m);
    m.item.as_mut().unwrap().import = ImportPanel::Closed;
    s.say_item(stripped_import(&["a", "b"], &[1]));
    m.begin_import();
    m.confirm_import();
    settle(&mut m, &s, &mut banks);
    let mut d = done(&m);
    draw(&ctx, &mut m);
    d.pictures = 2;
    d.pictures_missing = 1;
    d.held_pictures_quota = true;
    d.may_republish_as.clear();
    d.recorded = false;
    m.item.as_mut().unwrap().import = ImportPanel::Done(Box::new(d));
    draw(&ctx, &mut m);
    m.item.as_mut().unwrap().import = ImportPanel::Closed;
    // report states
    m.open_report();
    draw(&ctx, &mut m);
    m.set_report_choice(ReportChoice::Other);
    m.set_report_note("note");
    draw(&ctx, &mut m);
    for st in [ReportState::Sending, ReportState::Sent("Thank you.".into()), ReportState::Failed(Problem { message: "x".into(), retry: false, retry_at: None, account: true })] {
        m.item.as_mut().unwrap().report.as_mut().unwrap().state = st;
        draw(&ctx, &mut m);
    }
    // an update: with and without its changes
    let (s2, mut m3, _, mut b3) = updating();
    draw(&ctx, &mut m3);
    m3.begin_import();
    draw(&ctx, &mut m3);
    m3.item.as_mut().unwrap().update.as_mut().unwrap().changes = None;
    draw(&ctx, &mut m3);
    m3.sync(&mut b3);
    let _ = s2;
    // the close button reports back
    assert!(m3.open);
}


// ---- merging a newer version into the bank --------------------------------------------------------------------------------------

#[derive(Default)]
struct MemRestore {
    saved: Mutex<Vec<String>>,
    fail: Mutex<bool>,
}

impl RestoreSink for MemRestore {
    fn save(&self, banks: &[QuestionBank]) -> Result<String, String> {
        if *self.fail.lock().unwrap() {
            return Err("disk full".into());
        }
        let mut v = self.saved.lock().unwrap();
        v.push(serde_json::to_string(banks).unwrap());
        Ok(format!("memory copy {}", v.len()))
    }
}

fn mcq(stem: &str, key: bool) -> Value {
    let mut answers = vec![json!({"text": "A", "correct": key}), json!({"text": "B", "correct": false})];
    if stem.contains("!shape") {
        answers.push(json!({"text": "C", "correct": false}));
    }
    json!({"id": format!("src-{stem}"), "stem": stem, "kind": {"Mc": {"answers": answers}}, "points": 1.0, "difficulty": null, "created": 5, "modified": 6, "keyless": !key})
}

/// An item as the pool serves it: `pairs` are (pool question id, stem); `images` are (index, bytes).
fn item_q(form: &str, version: u64, license: &str, pairs: &[(&str, &str)], import: bool, images: &[(usize, &[u8])]) -> Vec<u8> {
    let key = form == "full";
    let mut qs: Vec<Value> = pairs.iter().map(|(_, s)| mcq(s, key)).collect();
    for (i, bytes) in images {
        qs[*i]["image"] = json!({"file": format!("media/{}.png", sha256_hex(bytes)), "width": 4, "height": 4});
    }
    let payload = json!({"schema_version": 2, "bank": {"id": "b-src", "name": "Cells", "questions": qs, "created": 1, "modified": 2}});
    let mut h = head(form, false, version);
    h["item"]["license"] = json!(license);
    h["questions"] = json!(pairs.iter().enumerate().map(|(i, (q, _))| json!({"pos": i, "qid": q, "chain": []})).collect::<Vec<_>>());
    let extra = import.then(|| json!({"license": license, "may_republish_as": [license], "previously_imported_versions": [], "images": {"held": 0}, "recorded": true}));
    streamed(&h, &payload, extra)
}

struct Scn {
    s: Arc<Script>,
    log: ImportLog,
    restore: Arc<MemRestore>,
    images: Arc<Mem>,
    banks: Vec<QuestionBank>,
}

impl Scn {
    /// Version 1 imported into a new bank "Cells" (and an unrelated bank "Other" beside it).
    fn new(pairs: &[(&str, &str)], form: &str, images: &[(usize, &[u8])]) -> Scn {
        let sc = Scn { s: Script::new(), log: ImportLog::memory(), restore: Arc::new(MemRestore::default()), images: Arc::new(Mem::default()), banks: vec![] };
        let mut sc = sc;
        let mut m = sc.model();
        m.open_browse();
        sc.s.say(200, page(vec![], None));
        sc.step(&mut m);
        sc.step(&mut m);
        sc.s.say_item(item_q(form, 1, "CC-BY-4.0", pairs, false, images));
        m.open_item(ID, None);
        sc.step(&mut m);
        m.begin_import();
        m.set_new_name("Cells");
        sc.s.say_item(item_q(form, 1, "CC-BY-4.0", pairs, true, images));
        for (_, bytes) in images {
            sc.s.say_bytes(bytes.to_vec());
        }
        m.confirm_import();
        sc.step(&mut m);
        assert_eq!(sc.banks.len(), 1, "{:?}", m.item.as_ref().map(|i| format!("{:?}", i.import)));
        sc.banks.push(local_bank("other", "Other", &["unrelated"]));
        sc
    }

    fn model(&self) -> BrowseModel {
        BrowseModel::new(Runner::Inline, self.images.clone(), self.log.clone()).with_retry(RetryPolicy::none()).with_restore(self.restore.clone())
    }

    fn step(&mut self, m: &mut BrowseModel) -> bool {
        m.frame(now(), &ready(&self.s));
        m.sync(&mut self.banks)
    }

    fn bank_id(&self) -> String {
        self.banks[0].id.clone()
    }

    fn mark_newer(&self, version: u64) {
        let mut r = self.log.load();
        r[0].newer = Some(NewerInfo { version, license: None, license_version: None });
        self.log.save(&r).unwrap();
    }

    /// Opens the update of the first bank to version `version` and builds the review.
    fn review(&mut self, version: u64, license: &str, form: &str, pairs: &[(&str, &str)], images: &[(usize, &[u8])]) -> BrowseModel {
        self.mark_newer(version);
        let mut m = self.model();
        m.open_update(&self.bank_id());
        self.s.say_item(item_q(form, version, license, pairs, false, images));
        self.step(&mut m);
        self.step(&mut m);
        assert!(m.loaded().is_some());
        m.begin_merge();
        self.step(&mut m);
        assert!(matches!(m.item.as_ref().unwrap().merge.as_ref().unwrap().phase, MergePhase::Review), "{:?}", m.item.as_ref().unwrap().merge);
        m
    }

    /// The reply the pool gives to the import that Apply makes.
    fn reply_import(&self, version: u64, license: &str, form: &str, pairs: &[(&str, &str)], images: &[(usize, &[u8])]) {
        self.s.say_item(item_q(form, version, license, pairs, true, images));
        for (_, bytes) in images {
            self.s.say_bytes(bytes.to_vec());
        }
    }

    fn stems(&self) -> Vec<String> {
        self.banks[0].questions.iter().map(|q| q.stem.clone()).collect()
    }
}

fn plan(m: &BrowseModel) -> &super::merge::MergePlan {
    m.merge_plan().unwrap()
}

fn row_of<'a>(m: &'a BrowseModel, title: &str) -> (usize, &'a Row) {
    plan(m).rows.iter().enumerate().find(|(_, r)| r.title == title).unwrap_or_else(|| panic!("no row {title}: {:?}", plan(m).rows.iter().map(|r| &r.title).collect::<Vec<_>>()))
}

fn decide(m: &mut BrowseModel, title: &str, c: Choice) {
    let i = row_of(m, title).0;
    m.set_merge_choice(i, c);
}

fn phase(m: &BrowseModel) -> &MergePhase {
    &m.item.as_ref().unwrap().merge.as_ref().unwrap().phase
}

fn merge_problem(m: &BrowseModel) -> String {
    m.item.as_ref().unwrap().merge.as_ref().unwrap().problem.clone().expect("a problem").message
}

const V1: [(&str, &str); 6] = [("q0", "zero"), ("q1", "one"), ("q2", "two"), ("q3", "three"), ("q4", "four"), ("q5", "five")];

/// Edits made by the teacher, and the pool's version 2.
fn busy_scene() -> (Scn, BrowseModel) {
    let mut sc = Scn::new(&V1, "full", &[]);
    sc.banks[0].questions[1].stem = "one (mine)".into();
    sc.banks[0].questions.remove(2);
    sc.banks[0].questions[2].stem = "three (mine)".into();
    let mut own = sc.banks[0].questions[0].clone();
    own.id = "mine-1".into();
    own.stem = "my own".into();
    sc.banks[0].questions.push(own);
    let v2 = [("q0", "zero"), ("q1", "one"), ("q2", "two (pool)"), ("q3", "three (pool)"), ("q4", "four (pool)"), ("q6", "six")];
    let m = sc.review(2, "CC-BY-4.0", "full", &v2, &[]);
    (sc, m)
}

#[test]
fn the_review_sorts_every_question_into_its_category_with_the_safe_defaults() {
    let (_sc, m) = busy_scene();
    let p = plan(&m);
    let k = |t: &str| (row_of(&m, t).1.kind, row_of(&m, t).1.choice);
    assert_eq!(k("zero"), (RowKind::Unchanged, Choice::Undecided));
    assert_eq!(k("one (mine)"), (RowKind::OursChanged, Choice::Keep));
    assert_eq!(k("two"), (RowKind::RemovedLocally, Choice::Undecided));
    assert_eq!(k("three (mine)"), (RowKind::Conflict, Choice::Undecided), "a conflict has no default");
    assert_eq!(k("four"), (RowKind::TheirsChanged, Choice::Take));
    assert_eq!(k("five"), (RowKind::RemovedInTheirs, Choice::Keep));
    assert_eq!(k("six"), (RowKind::NewInTheirs, Choice::Add));
    assert_eq!(p.unlinked, 1, "the question the teacher added is not part of the merge");
    assert_eq!((p.old_version, p.new_version), (1, 2));
    assert_eq!(p.undecided(), 1);
    assert!(!p.ready() && !m.can_apply_merge(), "apply waits for the conflict");
    // the field detail shows what differs, for every side
    let d = &row_of(&m, "three (mine)").1.diffs;
    assert_eq!(d.len(), 1);
    assert_eq!((d[0].field, d[0].base.as_str(), d[0].ours.as_str(), d[0].theirs.as_str()), ("Question text", "three", "three (mine)", "three (pool)"));
    assert!(row_of(&m, "five").1.options.contains(&Choice::Remove));
    assert_eq!(row_of(&m, "two").1.options, vec![], "nothing to decide for what you deleted");
}

#[test]
fn nothing_is_written_to_review_or_to_cancel() {
    let (mut sc, mut m) = busy_scene();
    let (banks, log, seen) = (serde_json::to_string(&sc.banks).unwrap(), sc.log.load(), sc.s.seen().len());
    decide(&mut m, "three (mine)", Choice::Theirs);
    m.cancel_merge();
    sc.step(&mut m);
    assert!(m.item.as_ref().unwrap().merge.is_none());
    assert_eq!(serde_json::to_string(&sc.banks).unwrap(), banks);
    assert_eq!(sc.log.load(), log);
    assert!(sc.restore.saved.lock().unwrap().is_empty());
    assert_eq!(sc.s.seen().len(), seen, "the review asked the pool for nothing more");
}

#[test]
fn apply_needs_every_conflict_decided_and_only_offered_choices_count() {
    let (_sc, mut m) = busy_scene();
    let i = row_of(&m, "three (mine)").0;
    m.set_merge_choice(i, Choice::Take);
    assert_eq!(plan(&m).rows[i].choice, Choice::Undecided, "Take is not a choice for a conflict");
    m.apply_merge();
    assert!(matches!(phase(&m), MergePhase::Review), "refused");
    m.set_merge_choice(i, Choice::Ours);
    assert!(m.can_apply_merge());
    let j = row_of(&m, "six").0;
    m.set_merge_choice(j, Choice::Both);
    assert_eq!(plan(&m).rows[j].choice, Choice::Add);
}

#[test]
fn applying_writes_a_restore_point_first_then_the_bank_then_advances_the_base() {
    let (mut sc, mut m) = busy_scene();
    decide(&mut m, "three (mine)", Choice::Both);
    decide(&mut m, "five", Choice::Remove);
    let before = serde_json::to_string(&sc.banks).unwrap();
    let other_before = serde_json::to_string(&sc.banks[1]).unwrap();
    sc.reply_import(2, "CC-BY-4.0", "full", &[("q0", "zero"), ("q1", "one"), ("q2", "two (pool)"), ("q3", "three (pool)"), ("q4", "four (pool)"), ("q6", "six")], &[]);
    m.apply_merge();
    assert!(matches!(phase(&m), MergePhase::Applying));
    assert_eq!(serde_json::to_string(&sc.banks).unwrap(), before, "nothing yet: the pool's version is still being fetched");
    assert!(sc.step(&mut m), "the caller saves the banks once");
    assert_eq!(sc.stems(), vec!["zero", "one (mine)", "three (mine)", "three (pool)", "four (pool)", "my own", "six"]);
    assert_eq!(*sc.restore.saved.lock().unwrap(), vec![before.clone()], "the copy is of the banks as they were");
    assert_eq!(serde_json::to_string(&sc.banks[1]).unwrap(), other_before, "no other bank is touched");
    let copy = &sc.banks[0].questions[3];
    assert!(copy.topics.iter().any(|t| t.contains("From the pool") && t.contains("Ms Frizzle") && t.contains("version 2")), "{:?}", copy.topics);
    let MergePhase::Done(d) = phase(&m).clone() else { panic!("{:?}", phase(&m)) };
    assert_eq!((d.outcome.taken, d.outcome.kept, d.outcome.added, d.outcome.removed, d.outcome.both), (1, 1, 1, 1, 1));
    assert_eq!((d.new_version, d.restore.as_str()), (2, "memory copy 1"));
    // the record moved on: version, base, badge
    let r = sc.log.load().remove(0);
    assert_eq!(r.version, 2);
    assert!(r.newer.is_none() && r.note.as_deref().unwrap().contains("merged"));
    let mut base: Vec<String> = r.base.iter().map(|b| b.snap.stem.clone()).collect();
    base.sort();
    assert_eq!(base, vec!["four (pool)", "one", "six", "three (pool)", "two (pool)", "zero"], "base := the pool's version; a question you deleted stays remembered as deleted");
    assert_eq!(r.base.iter().filter(|b| !sc.banks[0].questions.iter().any(|q| q.id == b.local_id)).count(), 1, "only that one is not in the bank");
    assert!(m.update_for_bank(&sc.bank_id()).is_none());
    assert_eq!(sc.s.paths().iter().filter(|p| p.ends_with("/import")).count(), 2);
}

#[test]
fn a_second_merge_only_sees_what_changed_since_the_first() {
    let (mut sc, mut m) = busy_scene();
    decide(&mut m, "three (mine)", Choice::Ours);
    decide(&mut m, "four", Choice::Keep);
    let v2 = [("q0", "zero"), ("q1", "one"), ("q2", "two (pool)"), ("q3", "three (pool)"), ("q4", "four (pool)"), ("q6", "six")];
    sc.reply_import(2, "CC-BY-4.0", "full", &v2, &[]);
    m.apply_merge();
    sc.step(&mut m);
    // the pool changes nothing more: what was declined is not offered again, and yours stay yours
    let mut m2 = sc.review(3, "CC-BY-4.0", "full", &v2, &[]);
    assert_eq!(plan(&m2).count(RowKind::Conflict) + plan(&m2).count(RowKind::TheirsChanged) + plan(&m2).count(RowKind::NewInTheirs), 0);
    assert_eq!(row_of(&m2, "three (mine)").1.kind, RowKind::OursChanged);
    m2.cancel_merge();
}

#[test]
fn conflict_choices_keep_yours_take_the_pools_or_keep_both() {
    for (choice, expect) in [
        (Choice::Ours, vec!["three (mine)"]),
        (Choice::Theirs, vec!["three (pool)"]),
        (Choice::Both, vec!["three (mine)", "three (pool)"]),
    ] {
        let mut sc = Scn::new(&V1[..4], "full", &[]);
        sc.banks[0].questions[3].stem = "three (mine)".into();
        let id = sc.banks[0].questions[3].id.clone();
        let mut m = sc.review(2, "CC-BY-4.0", "full", &[("q0", "zero"), ("q1", "one"), ("q2", "two"), ("q3", "three (pool)")], &[]);
        decide(&mut m, "three (mine)", choice);
        sc.reply_import(2, "CC-BY-4.0", "full", &[("q0", "zero"), ("q1", "one"), ("q2", "two"), ("q3", "three (pool)")], &[]);
        m.apply_merge();
        assert!(sc.step(&mut m));
        assert_eq!(sc.stems()[3..].to_vec(), expect, "{choice:?}");
        assert_eq!(sc.banks[0].questions[3].id, id, "the question keeps its place and id");
    }
}

#[test]
fn what_is_new_removed_or_declined_follows_the_choice() {
    let mut sc = Scn::new(&V1[..3], "full", &[]);
    let v2 = [("q0", "zero"), ("q2", "two"), ("q7", "seven")];
    let mut m = sc.review(2, "CC-BY-4.0", "full", &v2, &[]);
    decide(&mut m, "seven", Choice::Skip);
    sc.reply_import(2, "CC-BY-4.0", "full", &v2, &[]);
    m.apply_merge();
    sc.step(&mut m);
    assert_eq!(sc.stems(), vec!["zero", "one", "two"], "kept what the pool dropped, did not add the skipped one");
    let r = sc.log.load().remove(0);
    assert!(!r.base.iter().any(|b| b.snap.stem == "one" || b.snap.stem == "seven"), "a kept question is no longer linked; a skipped one is not remembered");
    // so a later merge offers the skipped one again and no longer treats "one" as the pool's
    let m2 = sc.review(3, "CC-BY-4.0", "full", &v2, &[]);
    assert_eq!(row_of(&m2, "seven").1.kind, RowKind::NewInTheirs);
    assert_eq!(plan(&m2).unlinked, 1);
}

#[test]
fn questions_no_longer_linked_are_not_part_of_the_merge_and_deleted_ones_stay_deleted() {
    let mut sc = Scn::new(&V1[..3], "full", &[]);
    let copy = sc.banks[0].questions[0].clone();
    sc.banks[1].questions.push({
        let mut c = copy.clone();
        c.id = "copy-elsewhere".into();
        c
    });
    sc.banks[0].questions.remove(1);
    let other = serde_json::to_string(&sc.banks[1]).unwrap();
    let v2 = [("q0", "zero (pool)"), ("q1", "one (pool)"), ("q2", "two")];
    let mut m = sc.review(2, "CC-BY-4.0", "full", &v2, &[]);
    assert_eq!(row_of(&m, "one").1.kind, RowKind::RemovedLocally);
    assert!(row_of(&m, "one").1.note.as_deref().unwrap().contains("stays deleted"));
    sc.reply_import(2, "CC-BY-4.0", "full", &v2, &[]);
    m.apply_merge();
    sc.step(&mut m);
    assert_eq!(sc.stems(), vec!["zero (pool)", "two"], "the deleted one did not come back");
    assert_eq!(serde_json::to_string(&sc.banks[1]).unwrap(), other, "a copy in another bank is not touched");
}

#[test]
fn a_licence_change_is_shown_and_the_credit_line_changes_only_on_apply() {
    let mut sc = Scn::new(&V1[..2], "full", &[]);
    sc.banks[0].details.description = format!("My own note\n{}", sc.banks[0].details.description);
    let desc_before = sc.banks[0].details.description.clone();
    let v2 = [("q0", "zero"), ("q1", "one (pool)")];
    let mut m = sc.review(2, "CC-BY-SA-4.0", "full", &v2, &[]);
    assert_eq!(plan(&m).license, Some(("CC-BY-4.0".into(), "CC-BY-SA-4.0".into())));
    assert!(plan(&m).new_credit.contains("CC BY-SA 4.0"));
    assert_eq!(sc.banks[0].details.description, desc_before, "unchanged while reviewing");
    sc.reply_import(2, "CC-BY-SA-4.0", "full", &v2, &[]);
    m.apply_merge();
    sc.step(&mut m);
    let d = &sc.banks[0].details;
    assert!(d.description.starts_with("My own note\n") && d.description.contains("version 2") && !d.description.contains("version 1."), "{}", d.description);
    assert_eq!(d.license_name, "CC BY-SA 4.0 (credit me, share alike)");
    assert!(d.license_url.contains("by-sa"));
    assert_eq!(sc.log.load()[0].license, "CC-BY-SA-4.0");
    let MergePhase::Done(done) = phase(&m) else { panic!() };
    assert!(done.license_changed);
}

#[test]
fn a_picture_the_pool_changed_is_fetched_and_stored_when_its_question_is_taken() {
    let (a, b) = (b"PIC-A".to_vec(), b"PIC-B".to_vec());
    let mut sc = Scn::new(&V1[..2], "full", &[(0, &a)]);
    let old_file = sc.banks[0].questions[0].image.clone().unwrap().file;
    let v2 = [("q0", "zero"), ("q1", "one")];
    let mut m = sc.review(2, "CC-BY-4.0", "full", &v2, &[(0, &b)]);
    let row = row_of(&m, "zero").1.clone();
    assert_eq!(row.kind, RowKind::TheirsChanged);
    assert_eq!(row.diffs.len(), 1);
    assert_eq!(row.diffs[0].field, "Picture");
    assert_eq!(sc.banks[0].questions[0].image.as_ref().unwrap().file, old_file, "not fetched while reviewing");
    sc.reply_import(2, "CC-BY-4.0", "full", &v2, &[(0, &b)]);
    m.apply_merge();
    sc.step(&mut m);
    assert_eq!(sc.banks[0].questions[0].image.as_ref().unwrap().file, format!("media/{}.webp", sha256_hex(&b)));
}

// -- the answer key ------------------------------------------------------------------------------------------------------------------

fn rekey(q: &mut Question, correct: usize) {
    if let crate::model::Kind::Mc { answers } = &mut q.kind {
        answers[correct].correct = true;
    }
}

fn mc_key(q: &Question) -> Vec<bool> {
    match &q.kind {
        crate::model::Kind::Mc { answers } => answers.iter().map(|a| a.correct).collect(),
        _ => vec![],
    }
}

const K1: [(&str, &str); 3] = [("q0", "alpha"), ("q1", "beta"), ("q2", "gamma")];

#[test]
fn a_key_the_teacher_entered_survives_when_the_pool_changes_only_the_wording() {
    let mut sc = Scn::new(&K1, "stripped", &[]);
    assert!(sc.banks[0].questions.iter().all(is_keyless), "imported without a key");
    rekey(&mut sc.banks[0].questions[0], 1);
    rekey(&mut sc.banks[0].questions[2], 0); // re-keyed only: the pool does not touch it
    let v2 = [("q0", "alpha v2"), ("q1", "beta"), ("q2", "gamma")];
    let mut m = sc.review(2, "CC-BY-4.0", "stripped", &v2, &[]);
    let r = row_of(&m, "alpha").1.clone();
    assert_eq!((r.kind, r.choice, r.carry_key), (RowKind::TheirsChanged, Choice::Take, true), "wording is taken, the key kept");
    assert_eq!(row_of(&m, "gamma").1.kind, RowKind::OursChanged, "a key alone counts as your change");
    assert!(row_of(&m, "gamma").1.diffs.iter().any(|d| d.field == "Answer key"));
    sc.reply_import(2, "CC-BY-4.0", "stripped", &v2, &[]);
    m.apply_merge();
    sc.step(&mut m);
    let q = &sc.banks[0].questions;
    assert_eq!(q[0].stem, "alpha v2");
    assert_eq!(mc_key(&q[0]), vec![false, true], "the local key is still there");
    assert_eq!(mc_key(&q[2]), vec![true, false]);
}

#[test]
fn the_pool_changing_the_choices_of_a_rekeyed_question_cannot_replace_the_key() {
    let mut sc = Scn::new(&K1, "stripped", &[]);
    rekey(&mut sc.banks[0].questions[1], 0);
    let before = serde_json::to_string(&sc.banks).unwrap();
    let v2 = [("q0", "alpha"), ("q1", "beta !shape"), ("q2", "gamma")];
    let mut m = sc.review(2, "CC-BY-4.0", "stripped", &v2, &[]);
    let (i, r) = row_of(&m, "beta");
    assert_eq!(r.kind, RowKind::Conflict);
    assert_eq!(r.options, vec![Choice::Ours, Choice::Both], "taking the pool's version is not offered");
    m.set_merge_choice(i, Choice::Theirs);
    assert_eq!(plan(&m).rows[i].choice, Choice::Undecided);
    // even forced, the apply refuses before anything is written
    m.item.as_mut().unwrap().merge.as_mut().unwrap().plan.as_mut().unwrap().rows[i].choice = Choice::Theirs;
    sc.reply_import(2, "CC-BY-4.0", "stripped", &v2, &[]);
    m.apply_merge();
    assert!(!sc.step(&mut m));
    assert!(matches!(phase(&m), MergePhase::Review));
    assert!(merge_problem(&m).contains("answer key you entered"));
    assert_eq!(serde_json::to_string(&sc.banks).unwrap(), before);
    assert!(sc.restore.saved.lock().unwrap().is_empty(), "no restore point for a merge that did not happen");
    // keeping both leaves yours, key included, and adds the pool's keyless version
    m.set_merge_choice(i, Choice::Both);
    sc.reply_import(2, "CC-BY-4.0", "stripped", &v2, &[]);
    m.apply_merge();
    assert!(sc.step(&mut m));
    let q = &sc.banks[0].questions;
    assert_eq!(mc_key(&q[1]), vec![true, false]);
    assert_eq!(q[2].stem, "beta !shape");
    assert!(is_keyless(&q[2]) && q[2].explanation.as_deref() == Some(KEYLESS_SENTINEL) && mc_key(&q[2]) == vec![false, false, false]);
}

#[test]
fn an_untouched_keyless_question_the_pool_changed_is_replaced_by_the_new_keyless_version() {
    let mut sc = Scn::new(&K1, "stripped", &[]);
    let v2 = [("q0", "alpha v2"), ("q1", "beta"), ("q2", "gamma")];
    let mut m = sc.review(2, "CC-BY-4.0", "stripped", &v2, &[]);
    assert!(!row_of(&m, "alpha").1.carry_key);
    sc.reply_import(2, "CC-BY-4.0", "stripped", &v2, &[]);
    m.apply_merge();
    sc.step(&mut m);
    assert!(is_keyless(&sc.banks[0].questions[0]) && sc.banks[0].questions[0].stem == "alpha v2");
}

// -- when the merge can not go ahead ---------------------------------------------------------------------------------------------------

#[test]
fn without_a_stored_base_merging_is_off_and_the_new_bank_path_says_why_it_is_the_only_one() {
    let mut sc = Scn::new(&V1[..2], "full", &[]);
    let mut r = sc.log.load();
    r[0].base.clear();
    sc.log.save(&r).unwrap();
    sc.mark_newer(2);
    let mut m = sc.model();
    m.open_update(&sc.bank_id());
    sc.s.say_item(item_q("full", 2, "CC-BY-4.0", &[("q0", "zero"), ("q1", "one")], false, &[]));
    sc.step(&mut m);
    sc.step(&mut m);
    let why = m.merge_blocker().unwrap();
    assert!(why.contains("before verSHion kept a copy") && why.contains("new bank"), "{why}");
    m.begin_merge();
    assert!(m.item.as_ref().unwrap().merge.is_none());
    m.begin_import();
    let ImportPanel::Choosing(c) = &m.item.as_ref().unwrap().import else { panic!() };
    assert_eq!(c.dest, Dest::New);
}

#[test]
fn other_reasons_to_stay_with_the_new_bank_path() {
    let mut sc = Scn::new(&V1[..2], "full", &[]);
    sc.mark_newer(2);
    // the bank is gone
    let mut gone = sc.model();
    gone.open_update(&sc.bank_id());
    sc.s.say_item(item_q("full", 2, "CC-BY-4.0", &[("q0", "zero")], false, &[]));
    let mut none: Vec<QuestionBank> = vec![];
    gone.frame(now(), &ready(&sc.s));
    gone.sync(&mut none);
    gone.frame(now(), &ready(&sc.s));
    gone.sync(&mut none);
    assert!(gone.merge_blocker().unwrap().contains("not there any more"));
    // already at that version
    let mut same = sc.model();
    same.open_update(&sc.bank_id());
    sc.s.say_item(item_q("full", 1, "CC-BY-4.0", &[("q0", "zero")], false, &[]));
    sc.step(&mut same);
    sc.step(&mut same);
    assert!(same.merge_blocker().unwrap().contains("already has this version"));
}

#[test]
fn a_failed_fetch_applies_nothing_and_says_why_every_time() {
    let (mut sc, mut m) = busy_scene();
    decide(&mut m, "three (mine)", Choice::Ours);
    let before = serde_json::to_string(&sc.banks).unwrap();
    let log = sc.log.load();
    let v2 = [("q0", "zero"), ("q1", "one"), ("q2", "two (pool)"), ("q3", "three (pool)"), ("q4", "four (pool)"), ("q6", "six")];
    type Fail = Box<dyn Fn(&Script)>;
    let cases: Vec<(&str, Fail)> = vec![
        ("offline", Box::new(|s: &Script| { s.fail(ApiError::Offline("x".into())); })),
        ("limit", Box::new(|s: &Script| { s.say_after(429, 3600, json!({"error": "daily_limit", "metric": "imports"})); })),
        ("gone", Box::new(|s: &Script| { s.say(404, json!({"error": "not_found"})); })),
        ("busy", Box::new(|s: &Script| { s.say(503, json!({"error": "busy"})); })),
    ];
    for (name, fail) in cases {
        fail(&sc.s);
        m.apply_merge();
        sc.step(&mut m);
        assert!(matches!(phase(&m), MergePhase::Review), "{name}");
        assert!(!merge_problem(&m).is_empty(), "{name}");
        assert_eq!(serde_json::to_string(&sc.banks).unwrap(), before, "{name}");
        assert_eq!(sc.log.load(), log, "{name}");
        assert!(sc.restore.saved.lock().unwrap().is_empty(), "{name}");
        assert!(plan(&m).rows.iter().any(|r| r.title == "three (mine)" && r.choice == Choice::Ours), "decisions are kept for the next try");
    }
    assert!(merge_problem(&m).contains("server is busy"));
    // a restore point that can not be written stops it too
    *sc.restore.fail.lock().unwrap() = true;
    sc.reply_import(2, "CC-BY-4.0", "full", &v2, &[]);
    m.apply_merge();
    sc.step(&mut m);
    assert!(merge_problem(&m).contains("nothing was changed"));
    assert_eq!(serde_json::to_string(&sc.banks).unwrap(), before);
    // and then it works
    *sc.restore.fail.lock().unwrap() = false;
    sc.reply_import(2, "CC-BY-4.0", "full", &v2, &[]);
    m.apply_merge();
    assert!(sc.step(&mut m));
    assert!(matches!(phase(&m), MergePhase::Done(_)));
}

#[test]
fn a_session_that_ends_while_applying_keeps_the_review() {
    let (mut sc, mut m) = busy_scene();
    decide(&mut m, "three (mine)", Choice::Ours);
    sc.s.fail(ApiError::SignedOut);
    m.apply_merge();
    sc.step(&mut m);
    assert_eq!(m.screen, Screen::Gate);
    m.frame(now(), &ready(&sc.s));
    assert_eq!(m.screen, Screen::Item);
    assert!(matches!(phase(&m), MergePhase::Review));
    assert!(m.can_apply_merge());
}

#[test]
fn a_pool_version_that_changed_while_reviewing_is_refused() {
    let (mut sc, mut m) = busy_scene();
    decide(&mut m, "three (mine)", Choice::Ours);
    let before = serde_json::to_string(&sc.banks).unwrap();
    // a newer version than the one reviewed
    sc.reply_import(3, "CC-BY-4.0", "full", &[("q0", "zero")], &[]);
    m.apply_merge();
    sc.step(&mut m);
    assert!(merge_problem(&m).contains("different version"));
    // the same version number but other content
    sc.reply_import(2, "CC-BY-4.0", "full", &[("q0", "zero"), ("q1", "one"), ("q2", "two (pool)"), ("q3", "three (CHANGED)"), ("q4", "four (pool)"), ("q6", "six")], &[]);
    m.apply_merge();
    sc.step(&mut m);
    assert!(merge_problem(&m).contains("changed while you were reviewing"));
    assert_eq!(serde_json::to_string(&sc.banks).unwrap(), before);
}

#[test]
fn leaving_while_the_pool_is_being_asked_drops_the_merge() {
    let (mut sc, mut m) = busy_scene();
    decide(&mut m, "three (mine)", Choice::Ours);
    sc.reply_import(2, "CC-BY-4.0", "full", &[("q0", "zero")], &[]);
    m.apply_merge();
    m.cancel_merge();
    assert!(!m.busy());
    assert!(!sc.step(&mut m));
    assert!(sc.restore.saved.lock().unwrap().is_empty());
}

// -- base bookkeeping --------------------------------------------------------------------------------------------------------------------

#[test]
fn an_import_keeps_a_base_per_question_and_too_much_is_not_kept() {
    let sc = Scn::new(&V1[..3], "full", &[]);
    let r = sc.log.load().remove(0);
    assert_eq!(r.base.len(), 3);
    assert_eq!(r.base[0].qid, "q0");
    assert_eq!(r.base[0].local_id, sc.banks[0].questions[0].id);
    assert_eq!(r.base[0].snap.stem, "zero");
    assert!(!r.base[0].snap.keyless);
    let one = r.base[0].clone();
    assert_eq!(super::merge::bounded(vec![one.clone(); 3]).len(), 3);
    assert!(super::merge::bounded(vec![one.clone(); super::merge::BASE_MAX_QUESTIONS + 1]).is_empty());
    let mut big = one;
    big.snap.stem = "x".repeat(super::merge::BASE_MAX_BYTES);
    assert!(super::merge::bounded(vec![big]).is_empty(), "past the size limit the base is dropped, and merging is off for that record");
    // an old record without the field still loads
    let old: ImportedRecord = serde_json::from_value(json!({"item_id": ID, "bank_id": "b", "version": 1, "license": "CC0-1.0", "imported_at": 1})).unwrap();
    assert!(old.base.is_empty());
}

#[test]
fn credit_lines_are_replaced_by_item_and_other_lines_are_left_alone() {
    let mut b = local_bank("b", "B", &[]);
    b.details.description = format!("Notes\n{}\nFrom the shared pool: \"Other\" by X, item {ID2} version 1.", attribution_line("Cells", "A", "CC0-1.0", ID, 1));
    super::merge::update_credit(&mut b, ID, &attribution_line("Cells", "A", "CC0-1.0", ID, 2));
    let lines: Vec<&str> = b.details.description.lines().collect();
    assert_eq!(lines.len(), 3);
    assert!(lines[0] == "Notes" && lines[1].contains("version 2") && lines[2].contains("version 1"));
    super::merge::update_credit(&mut b, "cccccccccccccccccccccccccccccccc", "NEW");
    assert!(b.details.description.ends_with("\nNEW"));
}

// -- headless frames --------------------------------------------------------------------------------------------------------------------

/// (critic 88 I4) An import recorded before this build kept no difficulty (the phase C build's record has none): where the
/// teacher's rating and the pool's differ, the pool's review says so in its own words (never "CONFLICT: both changed"),
/// counts it apart, and makes no choice for the teacher.
#[test]
fn an_import_recorded_with_no_difficulty_says_so_and_decides_nothing() {
    let mut sc = Scn::new(&V1[..2], "full", &[]);
    let mut r = sc.log.load();
    for b in &mut r[0].base {
        b.snap.difficulty = None;
    }
    sc.log.save(&r).unwrap();
    sc.banks[0].questions[0].difficulty = Some(crate::model::Difficulty::High);
    let mut m = sc.review(2, "CC-BY-4.0", "full", &V1[..2], &[]);
    let (i, row) = row_of(&m, "zero");
    assert!(row.kind == RowKind::Conflict && row.unrecorded && row.choice == Choice::Undecided, "{row:?}");
    assert_eq!(row.options, vec![Choice::Ours, Choice::Theirs], "never a second copy that differs only in difficulty");
    assert!(!m.can_apply_merge());
    let ctx = egui::Context::default();
    crate::theme::setup(&ctx);
    let page = super::bank_links_fix4_tests::frame_text(&ctx, &mut |ui| {
        super::browse_ui::page(ui, &mut m);
    });
    assert!(page.contains("Difficulty not recorded") && page.contains("1 difficulty not recorded"), "{page}");
    assert!(!page.contains("CONFLICT") && !page.contains("both changed"), "{page}");
    assert!(page.contains("no record of this question's difficulty") && page.contains("Choose which to keep"), "{page}");
    assert!(page.contains("1 question still needs a decision") && !page.contains("1 conflict still needs"), "{page}");
    // (critic 92 I3) the same words and order as every other card: Keep mine, then Take the pool's version
    assert!(!page.contains("Use the pool's version"), "{page}");
    let (keep, take) = (page.find("Keep mine").expect(&page), page.find("Take the pool's version").expect(&page));
    assert!(keep < take, "{page}");
    m.set_merge_choice(i, Choice::Ours);
    assert!(m.can_apply_merge());
}

#[test]
fn the_merge_screens_draw_in_every_state() {
    let ctx = egui::Context::default();
    crate::theme::setup(&ctx);
    // the choice between the two ways to update, with and without a base
    let mut sc = Scn::new(&V1[..2], "full", &[]);
    let (mut m, _) = (busy_scene().1, ());
    draw(&ctx, &mut m); // review with every category, a conflict undecided
    decide(&mut m, "three (mine)", Choice::Both);
    draw(&ctx, &mut m);
    let ms = m.item.as_mut().unwrap().merge.as_mut().unwrap();
    ms.problem = Some(Problem { message: "offline".into(), retry: false, retry_at: None, account: false });
    ms.plan.as_mut().unwrap().license = Some(("CC-BY-4.0".into(), "CC0-1.0".into()));
    draw(&ctx, &mut m);
    for st in [
        MergePhase::Wanted,
        MergePhase::Applying,
        MergePhase::Blocked("gone".into()),
        MergePhase::Done(Box::new(MergeDone { outcome: Default::default(), restore: "somewhere".into(), new_version: 2, bank_name: "<b>Cells</b>".into(), license_changed: true })),
    ] {
        m.item.as_mut().unwrap().merge.as_mut().unwrap().phase = st;
        draw(&ctx, &mut m);
    }
    m.cancel_merge();
    draw(&ctx, &mut m); // the two buttons
    // the same page with merging not possible
    let mut r = sc.log.load();
    r[0].base.clear();
    sc.log.save(&r).unwrap();
    sc.mark_newer(2);
    let mut m2 = sc.model();
    m2.open_update(&sc.bank_id());
    sc.s.say_item(item_q("full", 2, "CC-BY-4.0", &[("q0", "zero")], false, &[]));
    sc.step(&mut m2);
    sc.step(&mut m2);
    draw(&ctx, &mut m2);
}

#[test]
fn the_key_leak_report_is_offered_only_for_an_item_served_without_its_key() {
    let s = Script::new();
    let (m, _) = reporting(&s);
    assert!(m.key_leak_offered(), "a stripped item can be reported as leaking its key");

    let s = Script::new();
    let (mut m, _, _) = browsing(&s, vec![], None);
    let mut banks = vec![];
    viewing(&mut m, &s, streamed(&head("full", false, 2), &bank_payload(&["a"], true), None), &mut banks);
    m.open_report();
    assert!(!m.key_leak_offered());
    m.set_report_choice(ReportChoice::KeyLeak);
    assert!(m.report_blocker().unwrap().contains("hidden"));
    let n = s.seen().len();
    m.send_report();
    assert_eq!(s.seen().len(), n, "never sent: the Worker would answer 400");
}

#[test]
fn a_refused_key_leak_report_shows_the_workers_400_and_429_in_plain_words() {
    for (status, body, after, want) in [
        (400, json!({"error": "invalid_request", "field": "reason", "message": "This item shows its answer key: report a wrong answer instead."}), None, "answer key"),
        (429, json!({"error": "daily_limit", "metric": "key_leak_reports"}), Some(3600), "once per item a month"),
        (429, json!({"error": "daily_limit", "metric": "reports"}), Some(3600), "reports"),
    ] {
        let s = Script::new();
        let (mut m, mut banks) = reporting(&s);
        m.set_report_choice(ReportChoice::KeyLeak);
        match after {
            Some(a) => s.say_after(status, a, body),
            None => s.say(status, body),
        };
        m.send_report();
        settle(&mut m, &s, &mut banks);
        let ReportState::Failed(p) = &m.item.as_ref().unwrap().report.as_ref().unwrap().state else { panic!() };
        assert!(p.message.contains(want), "{status}: {}", p.message);
        assert!(!p.message.contains("refused the request"));
    }
}

#[test]
fn a_newer_version_with_nothing_different_is_recognised_and_not_offered_again() {
    let s = Script::new();
    let same = bank_payload(&["keep me", "me too"], false);
    let mut bank = local_bank("b1", "Mine", &["keep me", "me too"]);
    bank.questions[0].id = "q-a".into();
    bank.questions[1].id = "q-b".into();
    let mut r = record(ID, "b1", 1);
    r.question_ids = vec!["q-a".into(), "q-b".into()];
    r.fingerprints = fingerprints_of(same.to_string().as_bytes());
    r.newer = Some(NewerInfo { version: 2, license: None, license_version: None });
    let (mut m, log) = model_with(vec![r]);
    let mut banks = vec![bank];
    m.open_update("b1");
    s.say_item(streamed(&head("stripped", false, 2), &same, None));
    settle(&mut m, &s, &mut banks);
    settle(&mut m, &s, &mut banks);
    let c = m.item.as_ref().unwrap().update.as_ref().unwrap().changes.clone().unwrap();
    assert!(c.nothing_new && c.added == 0 && c.removed == 0);
    let saved = log.load();
    assert_eq!(saved[0].version, 2, "the new number is remembered");
    assert!(saved[0].newer.is_none(), "so the update badge goes away");
    assert!(!saved[0].has_update());
}

// ---- critic 61 I3: what one account read is never shown to the next --------------------------------------------

fn as_account(s: &Arc<Script>, email: &str) -> AccountSnapshot {
    AccountSnapshot { account: Some(format!("https://pool.test\n{email}")), ..ready(s) }
}

#[test]
fn c61_another_account_gets_none_of_the_first_accounts_results_item_or_search() {
    for sign_out_seen in [true, false] {
        let s = Script::new();
        // A's results hold A's own item (a held one only its author is shown), and A has one open.
        let (mut m, _, _) = browsing(&s, vec![list_item(ID, "Ada's held bank")], None);
        m.frame(now(), &as_account(&s, "ada@school.org"));
        let mut banks = vec![];
        viewing(&mut m, &s, stripped_item(&["a"]), &mut banks);
        m.form.q = "ada's search".into();
        if sign_out_seen {
            m.frame(now(), &AccountSnapshot::of_gate(Gate::SignedOut));
        } else {
            m.frame(now(), &AccountSnapshot::of_gate(Gate::Checking));
        }
        let s2 = Script::new();
        s2.say(200, page(vec![list_item(ID2, "Bob's search result")], None));
        let b = as_account(&s2, "bob@school.org");
        for _ in 0..3 {
            m.frame(now(), &b);
        }
        assert!(m.item.is_none(), "A's item is gone");
        assert_eq!(m.screen, Screen::Browse);
        assert!(m.form.q.is_empty(), "A's search words are gone");
        assert_eq!(m.results.items.iter().map(|i| i.title.clone()).collect::<Vec<_>>(), vec!["Bob's search result".to_string()]);
        assert_eq!(s2.seen().len(), 1, "B's own search, with B's session");
    }
}

#[test]
fn c61_a_closed_browser_forgets_the_first_account_too() {
    let s = Script::new();
    let (mut m, _, _) = browsing(&s, vec![list_item(ID, "Ada's held bank")], None);
    m.frame(now(), &as_account(&s, "ada@school.org"));
    m.close();
    let s2 = Script::new();
    m.note_account(&as_account(&s2, "bob@school.org"));
    assert!(!m.open);
    assert!(m.results.items.is_empty() && !m.results.loaded);
    assert!(s2.seen().is_empty());
}

#[test]
fn c61_the_same_account_signed_in_again_keeps_its_item() {
    let s = Script::new();
    let (mut m, _, _) = browsing(&s, vec![list_item(ID, "Cells")], None);
    let a = as_account(&s, "ada@school.org");
    m.frame(now(), &a);
    let mut banks = vec![];
    viewing(&mut m, &s, stripped_item(&["a"]), &mut banks);
    m.frame(now(), &AccountSnapshot::of_gate(Gate::SignedOut));
    assert_eq!(m.screen, Screen::Gate);
    m.frame(now(), &a);
    assert_eq!(m.screen, Screen::Item);
    assert!(m.loaded().is_some());
}

// -- round 16 (critic 93 survivor m43, critic 94 m8) --------------------------------------------------------------------------------

/// (survivor m43) The "you entered its answer key" card lists Keep mine first, then the pool's version (critic 92 I3's order).
#[test]
fn r16_the_key_only_card_lists_keep_mine_first() {
    let mut sc = Scn::new(&K1, "stripped", &[]);
    rekey(&mut sc.banks[0].questions[0], 1);
    let v2 = [("q0", "alpha v2"), ("q1", "beta"), ("q2", "gamma")];
    let m = sc.review(2, "CC-BY-4.0", "stripped", &v2, &[]);
    let r = row_of(&m, "alpha").1;
    assert_eq!((r.kind, r.carry_key, r.options.clone()), (RowKind::TheirsChanged, true, vec![Choice::Keep, Choice::Take]));
}

/// (critic 94 m8) The cards' notes name the choices as the buttons do ("Take the pool's version", "Keep mine", "Keep both"),
/// never "if you use the pool's" or "keep yours".
#[test]
fn r16_the_notes_name_the_choices_as_the_buttons_do() {
    // both changed, the key kept if the pool's version is taken
    let mut sc = Scn::new(&K1, "stripped", &[]);
    rekey(&mut sc.banks[0].questions[0], 1);
    sc.banks[0].questions[0].stem = "alpha (mine)".into();
    let v2 = [("q0", "alpha v2"), ("q1", "beta"), ("q2", "gamma")];
    let m = sc.review(2, "CC-BY-4.0", "stripped", &v2, &[]);
    let r = row_of(&m, "alpha (mine)").1;
    assert_eq!((r.kind, r.carry_key), (RowKind::Conflict, true));
    assert_eq!(r.note.as_deref(), Some("Both changed this. If you take the pool's version, your answer key is kept."));
    // the pool changed the choices of a question she keyed
    let mut sc = Scn::new(&K1, "stripped", &[]);
    rekey(&mut sc.banks[0].questions[1], 0);
    let v2 = [("q0", "alpha"), ("q1", "beta !shape"), ("q2", "gamma")];
    let m = sc.review(2, "CC-BY-4.0", "stripped", &v2, &[]);
    let note = row_of(&m, "beta").1.note.clone().unwrap();
    assert!(note.ends_with("so the pool's version can not take your key. Choose \"Keep mine\" or \"Keep both\"."), "{note}");
    for n in [note, super::merge::changed_unrecorded_note(3)] {
        assert!(!n.contains("If you use") && !n.contains("keep yours") && !n.contains("Keep yours"), "{n}");
    }
    assert!(super::merge::changed_unrecorded_note(3).contains("\"Take the pool's version\" to get its changes and its difficulty, or \"Keep mine\""));
}
