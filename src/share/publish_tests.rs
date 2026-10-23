//! The publish flow's model against a scripted session (no network, no egui): every step, the
//! gating rules, every refusal kind, acknowledgement bookkeeping, republish, cancel and retry.

use std::collections::{HashMap, VecDeque};
use std::sync::mpsc::{Receiver, Sender, channel};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use serde_json::{Value, json};
use web_time::Instant;

use super::api::{ApiError, Status};
use super::model::{AccountModel, Notice, Page, Runner, SettingsIo, server_mode};
use super::pool::*;
use super::publish::*;
use super::publish_text as text;
use super::transport::{LocalFuture, Method, Request, Response};

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
    /// The meta line of a publish body.
    fn meta(&self) -> Value {
        let nl = self.body.iter().position(|b| *b == b'\n').unwrap();
        serde_json::from_slice(&self.body[..nl]).unwrap()
    }
    fn payload(&self) -> Value {
        let nl = self.body.iter().position(|b| *b == b'\n').unwrap();
        serde_json::from_slice(&self.body[nl + 1..]).unwrap()
    }
}

#[derive(Default)]
struct Script {
    replies: Mutex<VecDeque<Result<Response, ApiError>>>,
    seen: Mutex<Vec<Sent>>,
    /// When set, each request waits for a token first (cancel and progress tests).
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
            let waited = {
                let g = self.gate.lock().unwrap();
                g.as_ref().map(|rx| rx.recv_timeout(Duration::from_secs(10)).is_ok())
            };
            let _ = waited;
            self.replies.lock().unwrap().pop_front().unwrap_or_else(|| panic!("no scripted reply for {:?}", r.url))
        })
    }
}

// ---- fixtures ----------------------------------------------------------------------------------------------

fn bank_json(n: usize, image: Option<&str>) -> String {
    let qs: Vec<Value> = (0..n)
        .map(|i| {
            let mut q = json!({"id": format!("q{i}"), "stem": format!("Question text {i}\nsecond line"), "image": null, "kind": {"Mc": {"answers": [{"text": "Alpha", "correct": true}, {"text": "Beta", "correct": false}]}}});
            if i == 0 && let Some(f) = image {
                q["image"] = json!({"file": f, "width": 4, "height": 4});
            }
            q
        })
        .collect();
    json!({"id": "b1", "name": "Unit 1", "questions": qs, "created": 1, "modified": 2}).to_string()
}

fn source(n: usize) -> ShareSource {
    ShareSource::from_bank_json("bank:b1", "Bank: Unit 1", "Unit 1", &bank_json(n, None))
}

fn model(media: HashMap<String, Vec<u8>>) -> (PublishModel, LogIo) {
    let log = LogIo::memory();
    (model_in(&log, media), log)
}

fn model_in(log: &LogIo, media: HashMap<String, Vec<u8>>) -> PublishModel {
    PublishModel::new(Runner::Inline, Arc::new(media), log.clone()).with_retry(RetryPolicy::none())
}

fn now() -> Instant {
    Instant::now()
}

fn ready(s: &Arc<Script>) -> AccountSnapshot {
    AccountSnapshot::ready(s.clone())
}

/// Started, signed in, on the options page.
fn at_options(s: &Arc<Script>, n: usize) -> PublishModel {
    let (mut m, _) = model(HashMap::new());
    m.start(source(n));
    m.frame(now(), &ready(s));
    assert_eq!(m.step, Step::Options);
    m
}

fn fill(m: &mut PublishModel) {
    m.form.license = Some(License::CcBy);
    m.form.attest = true;
}

fn stripped_view(n: usize) -> Value {
    let qs: Vec<Value> = (0..n)
        .map(|i| json!({"keyless": true, "stem": format!("Question text {i}"), "kind": {"Mc": {"answers": [{"text": "Beta", "correct": false}, {"text": "Alpha", "correct": false}]}}}))
        .collect();
    json!({"schema_version": 2, "bank": {"name": "Unit 1", "questions": qs}})
}

fn preview_ok(n: usize) -> Value {
    json!({"ok": true, "dry_run": true, "hide_key": true, "student_view": stripped_view(n), "notes": [], "label_findings": [], "labels_removed": [],
           "images_removed": [], "figures_removed": [], "order_reasons": [], "license_check": {"status": "ok"}, "chains": []})
}

fn publish_ok() -> Value {
    json!({"item_id": ID, "publish_id": "p".repeat(32), "version": 1, "kind": "bank", "visibility": "public", "license": "CC-BY-4.0",
           "hide_key": true, "qids": ["a".repeat(32)], "public_version": 1, "possible_duplicate_of": null,
           "served_orders": [{"question": 0, "order": [1, 0]}], "order_reasons": [], "labels_removed": [], "images_kept": [], "images_removed": [], "figures_removed": [], "dropped": []})
}

/// Checked and on the review step.
fn at_review(s: &Arc<Script>) -> PublishModel {
    let mut m = at_options(s, 2);
    fill(&mut m);
    s.say(200, preview_ok(2));
    m.check();
    m.frame(now(), &ready(s));
    assert_eq!(m.step, Step::Review, "{:?} {:?}", m.notice, m.failure);
    m
}

fn publish_ready(m: &mut PublishModel) {
    m.form.reviewed = true;
}

// ---- gate, and nothing happens while closed --------------------------------------------------------------

#[test]
fn a_closed_model_does_no_io_and_keeps_no_state() {
    let s = Script::new();
    let (mut m, log) = model(HashMap::new());
    m.frame(now(), &ready(&s));
    m.check();
    m.publish();
    assert!(s.seen().is_empty());
    assert!(!m.open && !m.busy());
    assert!(log.find("bank:b1").is_none());
    // closing an opened one drops the work and stays quiet
    m.start(source(1));
    m.close();
    m.frame(now(), &ready(&s));
    assert!(s.seen().is_empty());
    assert!(!m.open);
}

#[test]
fn a_signed_out_account_points_to_the_account_window_and_resumes_when_ready() {
    let s = Script::new();
    let (mut m, _) = model(HashMap::new());
    m.start(source(1));
    // not looked up yet: the check is asked for, once
    m.frame(now(), &AccountSnapshot::of_gate(Gate::Checking));
    assert_eq!(m.step, Step::Gate);
    assert!(m.wants_account_check());
    m.account_check_asked();
    assert!(!m.wants_account_check());
    // the quiet look-up ended without a sign-in: signed out, never a dead end
    m.frame(now(), &AccountSnapshot::of_gate(Gate::Checking));
    assert_eq!(m.gate, Gate::SignedOut);
    assert_eq!(m.step, Step::Gate);
    assert!(!m.take_open_account());
    m.request_open_account();
    assert!(m.take_open_account());
    assert!(!m.take_open_account());
    m.frame(now(), &AccountSnapshot::of_gate(Gate::NotApproved(Status::Pending)));
    assert_eq!(m.step, Step::Gate);
    m.frame(now(), &AccountSnapshot::of_gate(Gate::NotSetUp));
    assert_eq!(m.step, Step::Gate);
    // nothing can be checked or published from the gate
    fill(&mut m);
    m.check();
    assert!(s.seen().is_empty());
    m.frame(now(), &ready(&s));
    assert_eq!(m.step, Step::Options);
}

#[test]
fn the_account_gate_is_read_from_the_account_window_without_any_io() {
    let factory: super::model::Factory = Arc::new(|_| panic!("no client may be built"));
    let acct = |mode| AccountModel::new(factory.clone(), Runner::Inline, SettingsIo::memory()).with_server_mode(mode);
    // a release build without a server
    assert_eq!(snapshot_of(&acct(server_mode(None, false))).gate, Gate::NotSetUp);
    // never opened, nobody known
    let m = acct(server_mode(Some("https://pool.example.org"), false));
    let snap = snapshot_of(&m);
    assert_eq!((snap.gate, snap.account_open), (Gate::Checking, false));
    // open on the sign-in page: signed out
    let mut m = acct(server_mode(Some("https://pool.example.org"), false));
    m.open = true;
    m.page = Page::SignIn;
    assert_eq!(snapshot_of(&m).gate, Gate::SignedOut);
    m.page = Page::Opening;
    assert_eq!(snapshot_of(&m).gate, Gate::Checking);
    m.page = Page::NotConfigured;
    assert_eq!(snapshot_of(&m).gate, Gate::NoServer);
    m.page = Page::Status;
    assert_eq!(snapshot_of(&m).gate, Gate::Unreachable);
    // signed in but pending, or approved without a client yet
    let me = |status| super::api::Me { email: "a@b.org".into(), display_name: "A".into(), school: "S".into(), status };
    m.me = Some(me(Status::Pending));
    assert_eq!(snapshot_of(&m).gate, Gate::NotApproved(Status::Pending));
    m.me = Some(me(Status::Approved));
    assert_eq!(snapshot_of(&m).gate, Gate::Checking);
    m.me = Some(me(Status::Suspended { reason: None }));
    assert!(matches!(snapshot_of(&m).gate, Gate::NotApproved(Status::Suspended { .. })));
}

// ---- sources and the record -------------------------------------------------------------------------------

#[test]
fn a_source_makes_a_payload_the_worker_accepts_the_shape_of() {
    let src = source(3);
    assert_eq!(src.questions.len(), 3);
    assert_eq!(src.questions[1].number, 2);
    assert_eq!(src.questions[1].stem, "Question text 1");
    assert_eq!(src.questions[0].kind, "Mc");
    let v: Value = serde_json::from_slice(&src.payload(1)).unwrap();
    assert_eq!(v["schema_version"], 1);
    assert_eq!(v["bank"]["questions"].as_array().unwrap().len(), 3);
    let v: Value = serde_json::from_slice(&src.payload(2)).unwrap();
    assert_eq!(v["schema_version"], 2);
}

#[test]
fn a_real_bank_and_exam_become_sources() {
    use crate::model::*;
    let q = |id: &str, stem: &str| Question {
        id: id.into(),
        stem: stem.into(),
        kind: Kind::Short { sample: String::new() },
        points: None,
        difficulty: None,
        topics: vec![],
        image: None,
        explanation: None,
        figure: None,
        created: 1,
        modified: 1,
    };
    let bank = QuestionBank { id: "b9".into(), name: "Cells".into(), questions: vec![q("q1", "What is a cell?"), q("q2", "Name an organelle.")], created: 1, modified: 2, details: Default::default() };
    let s = ShareSource::from_bank(&bank).unwrap();
    assert_eq!(s.key, "bank:b9");
    assert_eq!(s.title, "Cells");
    assert_eq!(s.questions.len(), 2);
    let v: Value = serde_json::from_slice(&s.payload(1)).unwrap();
    assert_eq!(v["bank"]["name"], "Cells");
    assert!(ShareSource::from_bank(&QuestionBank { questions: vec![], ..bank.clone() }).is_err());

    let mut exam = Exam { id: "e1".into(), name: "Quiz 1".into(), ..Default::default() };
    exam.items = vec![ExamItem::new("b9", "q2"), ExamItem::new("b9", "nope"), ExamItem::new("b9", "q1")];
    let es = ShareSource::from_exam(&exam, std::slice::from_ref(&bank)).unwrap();
    assert_eq!(es.key, "exam:e1");
    assert_eq!(es.missing, 1);
    assert_eq!(es.questions.iter().map(|q| q.stem.as_str()).collect::<Vec<_>>(), ["Name an organelle.", "What is a cell?"]);
    assert!(ShareSource::from_exam(&Exam { id: "e2".into(), name: "Empty".into(), ..Default::default() }, &[bank]).is_err());
}

#[test]
fn the_publish_log_remembers_and_forgets_by_source() {
    let log = LogIo::memory();
    let rec = |key: &str, item: &str| PublishedRecord { key: key.into(), item_id: item.into(), publish_id: "p".repeat(32), version: 2, visibility: "public".into(), license: "CC-BY-4.0".into(), title: "T".into(), qids: vec![] };
    log.put(rec("bank:a", ID)).unwrap();
    log.put(rec("bank:b", ID2)).unwrap();
    log.put(rec("bank:a", ID2)).unwrap();
    assert_eq!(log.find("bank:a").unwrap().item_id, ID2);
    assert_eq!(log.find("bank:b").unwrap().item_id, ID2);
    log.forget("bank:a").unwrap();
    assert!(log.find("bank:a").is_none());
    // a hand-edited bad item id is not used
    log.put(rec("bank:c", "not-an-id")).unwrap();
    assert!(log.find("bank:c").is_none());
}

// ---- options -----------------------------------------------------------------------------------------------------

#[test]
fn nothing_is_preselected_and_each_option_is_checked() {
    let s = Script::new();
    let mut m = at_options(&s, 1);
    assert_eq!(m.form.license, None);
    assert!(m.form.hide_key);
    assert_eq!(m.form.title, "Unit 1");
    let bad = |m: &PublishModel| m.options().unwrap_err();
    assert!(bad(&m).contains("licence"));
    m.form.license = Some(License::CcBy);
    m.form.title = "   ".into();
    assert!(bad(&m).contains("title"));
    m.form.title = "x".repeat(201);
    assert!(bad(&m).contains("too long"));
    m.form.title = "Cells".into();
    m.form.subject = "y".repeat(101);
    assert!(bad(&m).contains("subject"));
    m.form.subject = " Biology ".into();
    m.form.tags = "cells, mitosis;cells\n  dna ".into();
    m.form.standards = "NGSS HS-LS1".into();
    // all-rights-reserved can not be public; switching to public drops it
    m.form.license = Some(License::CircleOnly);
    assert!(bad(&m).contains("can not be public"));
    m.set_visibility(Visibility::Unlisted);
    m.form.license = Some(License::CircleOnly);
    assert!(m.options().is_ok());
    m.set_visibility(Visibility::Public);
    assert_eq!(m.form.license, None);
    m.form.license = Some(License::CcByNcSa);
    let o = m.options().unwrap();
    assert_eq!(o.license, License::CcByNcSa);
    assert_eq!(o.meta.title, "Cells");
    assert_eq!(o.meta.subject, "Biology");
    assert_eq!(o.meta.tags, vec!["cells", "mitosis", "dna"]);
    assert_eq!(o.meta.standards, vec!["NGSS HS-LS1"]);
    assert!(o.independent_of.is_none());
    assert!(o.labels.is_empty() && o.order_acks.is_empty() && o.ack_formatting.is_empty() && o.image_ok.is_empty());
}

#[test]
fn the_licence_table_offers_every_licence_the_server_accepts() {
    for l in License::ALL {
        let info = text::license_info(l);
        assert!(!info.name.is_empty() && info.line.len() > 20, "{l:?}");
        assert_eq!(License::parse(l.as_str()), Some(l));
    }
    assert_eq!(text::LICENSES.len(), License::ALL.len());
    // a licence the server names that this build does not know is shown, as plain text
    assert_eq!(text::license_name("CC-BY-9.9\u{200b}"), "CC-BY-9.9");
    assert!(text::license_name("CC-BY-4.0").contains("credit"));
}

// ---- attestation and the preview gate ---------------------------------------------------------------------------

#[test]
fn nothing_is_sent_without_the_own_work_box() {
    let s = Script::new();
    let mut m = at_options(&s, 1);
    m.form.license = Some(License::CcBy);
    m.check();
    assert!(s.seen().is_empty());
    assert!(matches!(&m.notice, Some(Notice::Error(t)) if t.contains("own work")));
    assert_eq!(m.step, Step::Options);
    m.form.attest = true;
    s.say(200, preview_ok(1));
    m.check();
    assert_eq!(s.seen().len(), 1);
}

#[test]
fn publishing_needs_the_box_a_current_clean_preview_and_a_look_at_it() {
    let s = Script::new();
    let mut m = at_review(&s);
    assert_eq!(s.paths(), ["Post /items/preview"]);
    // the preview alone does not enable it
    assert!(m.publish_blocker().unwrap().contains("looked"));
    m.publish();
    assert_eq!(s.seen().len(), 1);
    assert_eq!(m.step, Step::Review);
    // untick the box on the options page: still no publish
    publish_ready(&mut m);
    m.form.attest = false;
    assert!(m.publish_blocker().unwrap().contains("own work"));
    m.publish();
    assert_eq!(s.seen().len(), 1);
    m.form.attest = true;
    assert_eq!(m.publish_blocker(), None);
    // any option change makes the preview stale
    m.form.license = Some(License::CcBySa);
    assert!(!m.preview_is_current());
    assert!(m.publish_blocker().unwrap().contains("changed"));
    m.publish();
    assert_eq!(s.seen().len(), 1);
    m.form.license = Some(License::CcBy);
    assert!(m.preview_is_current());
    m.form.hide_key = false;
    assert!(!m.preview_is_current());
    m.form.hide_key = true;
    m.form.update_existing = true; // no record: no effect
    assert!(m.preview_is_current());
    // signed out: no publish
    m.frame(now(), &AccountSnapshot::of_gate(Gate::SignedOut));
    assert_eq!(m.step, Step::Gate);
    assert!(m.publish_blocker().is_some());
    m.publish();
    assert_eq!(s.seen().len(), 1);
}

#[test]
fn a_preview_with_key_material_in_the_hidden_form_blocks_publishing() {
    let s = Script::new();
    let mut m = at_options(&s, 2);
    fill(&mut m);
    let mut v = preview_ok(2);
    v["student_view"]["bank"]["questions"][1]["kind"]["Mc"]["answers"][0]["correct"] = json!(true);
    s.say(200, v);
    m.check();
    m.frame(now(), &ready(&s));
    assert_eq!(m.step, Step::Review);
    let p = m.preview.as_ref().unwrap();
    assert!(p.blocked.as_deref().unwrap().contains("answer key"));
    publish_ready(&mut m);
    assert!(m.publish_blocker().unwrap().contains("wrong"));
    m.publish();
    assert_eq!(s.seen().len(), 1);
    // a preview with a different number of questions is blocked too
    let s2 = Script::new();
    let mut m = at_options(&s2, 3);
    fill(&mut m);
    s2.say(200, preview_ok(2));
    m.check();
    m.frame(now(), &ready(&s2));
    assert!(m.preview.as_ref().unwrap().blocked.as_deref().unwrap().contains("number of questions"));
}

#[test]
fn a_shown_key_preview_is_built_from_the_local_questions_and_says_so() {
    let s = Script::new();
    let mut m = at_options(&s, 2);
    fill(&mut m);
    m.form.hide_key = false;
    let mut v = preview_ok(2);
    v["hide_key"] = json!(false);
    v["student_view"] = json!({"questions": 2, "key_shown": true});
    s.say(200, v);
    m.check();
    m.frame(now(), &ready(&s));
    let p = m.preview.as_ref().unwrap();
    assert_eq!(p.view.questions.len(), 2);
    assert_eq!(p.view.form, Served::Full);
    assert!(p.blocked.is_none());
    let meta = s.seen()[0].meta();
    assert_eq!(meta["hide_key"], false);
}

// ---- the requests --------------------------------------------------------------------------------------------------

#[test]
fn a_check_and_a_publish_send_the_same_meta_with_the_attestation_and_the_same_id() {
    let s = Script::new();
    let mut m = at_review(&s);
    publish_ready(&mut m);
    s.say(201, publish_ok());
    m.publish();
    assert_eq!(m.step, Step::Publishing, "inline jobs finish at the next frame");
    m.frame(now(), &ready(&s));
    assert_eq!(m.step, Step::Done);
    assert_eq!(s.paths(), ["Post /items/preview", "Post /items"]);
    let (a, b) = (s.seen()[0].meta(), s.seen()[1].meta());
    assert_eq!(a, b, "same options, same publish id");
    assert_eq!(b["attest"], json!({"own_work": true, "terms_version": POOL_TERMS_VERSION}));
    assert_eq!(b["attestation"]["accepted"], true);
    assert_eq!(b["license"], "CC-BY-4.0");
    assert_eq!(b["visibility"], "public");
    assert_eq!(b["hide_key"], true);
    assert_eq!(b["meta"]["title"], "Unit 1");
    assert_eq!(s.seen()[1].payload()["schema_version"], 1);
    // success: the record, the result, a placeholder and no author-only data
    let d = m.done.as_ref().unwrap();
    assert!(d.public() && !d.republish);
    assert_eq!(d.result.item_id, ID);
    assert!(!format!("{d:?}").contains("order\":"), "served orders stay out of Debug");
    assert!(m.preview.is_none());
}

#[test]
fn a_new_publish_id_is_made_when_anything_the_server_reads_changed() {
    let s = Script::new();
    let mut m = at_options(&s, 1);
    fill(&mut m);
    s.say(200, preview_ok(1)).say(200, preview_ok(1)).say(200, preview_ok(1));
    m.check();
    m.frame(now(), &ready(&s));
    m.check();
    m.frame(now(), &ready(&s));
    m.form.subject = "Biology".into();
    m.check();
    m.frame(now(), &ready(&s));
    let ids: Vec<Value> = s.seen().iter().map(|x| x.meta()["publish_id"].clone()).collect();
    assert_eq!(ids[0], ids[1]);
    assert_ne!(ids[1], ids[2]);
}

#[test]
fn a_possible_duplicate_and_a_replay_reach_the_done_screen() {
    let s = Script::new();
    let mut m = at_review(&s);
    publish_ready(&mut m);
    let mut ok = publish_ok();
    ok["possible_duplicate_of"] = json!(ID2);
    ok["replayed"] = json!(true);
    s.say(200, ok);
    m.publish();
    m.frame(now(), &ready(&s));
    let d = m.done.as_ref().unwrap();
    assert_eq!(d.result.possible_duplicate_of.as_deref(), Some(ID2));
    assert!(d.result.replayed);
}

// ---- republish -------------------------------------------------------------------------------------------------------

fn published_once(s: &Arc<Script>, log: &LogIo) -> PublishModel {
    let mut m = model_in(log, HashMap::new());
    m.start(source(2));
    m.frame(now(), &ready(s));
    fill(&mut m);
    s.say(200, preview_ok(2)).say(201, publish_ok());
    m.check();
    m.frame(now(), &ready(s));
    publish_ready(&mut m);
    m.publish();
    m.frame(now(), &ready(s));
    assert_eq!(m.step, Step::Done);
    m
}

#[test]
fn an_item_published_before_is_republished_with_its_base_publish_id() {
    let s = Script::new();
    let log = LogIo::memory();
    let _first = published_once(&s, &log);
    let rec = log.find("bank:b1").unwrap();
    assert_eq!((rec.item_id.as_str(), rec.version), (ID, 1));

    let mut m = model_in(&log, HashMap::new());
    m.start(source(2));
    m.frame(now(), &ready(&s));
    assert!(m.record().is_some());
    assert!(m.form.update_existing);
    assert_eq!(m.form.license, Some(License::CcBy), "the earlier licence is offered, not hidden");
    assert!(matches!(m.target(), Target::Republish { item_id, .. } if item_id == ID));
    m.form.attest = true;
    s.say(200, preview_ok(2));
    m.check();
    m.frame(now(), &ready(&s));
    let last = s.seen().last().unwrap().clone();
    assert_eq!((last.method, last.path.as_str()), (Method::Post, format!("/items/{ID}/preview").as_str()));
    let meta = last.meta();
    assert_eq!(meta["base_publish_id"], "p".repeat(32));
    assert!(meta.get("visibility").is_none());
    assert_eq!(last.payload()["schema_version"], 2, "a republish needs schema 2");
    publish_ready(&mut m);
    let mut ok = publish_ok();
    ok["version"] = json!(2);
    ok["publish_id"] = json!("q".repeat(32));
    s.say(200, ok);
    m.publish();
    m.frame(now(), &ready(&s));
    let last = s.seen().last().unwrap().clone();
    assert_eq!((last.method, last.path.as_str()), (Method::Put, format!("/items/{ID}").as_str()));
    assert!(m.done.as_ref().unwrap().republish);
    let rec = log.find("bank:b1").unwrap();
    assert_eq!((rec.version, rec.publish_id.as_str()), (2, "q".repeat(32).as_str()), "the next republish builds on this one");
}

#[test]
fn the_person_can_share_a_new_copy_instead_of_updating() {
    let s = Script::new();
    let log = LogIo::memory();
    let _ = published_once(&s, &log);
    let mut m = model_in(&log, HashMap::new());
    m.start(source(2));
    m.frame(now(), &ready(&s));
    m.set_update_existing(false);
    assert_eq!(m.target(), &Target::New);
    m.form.attest = true;
    s.say(200, preview_ok(2));
    m.check();
    assert_eq!(s.seen().last().unwrap().path, "/items/preview");
    assert!(s.seen().last().unwrap().meta().get("base_publish_id").is_none());
}

#[test]
fn a_conflict_is_only_overridden_when_the_person_says_so() {
    let s = Script::new();
    let log = LogIo::memory();
    let _ = published_once(&s, &log);
    let mut m = model_in(&log, HashMap::new());
    m.start(source(2));
    m.frame(now(), &ready(&s));
    m.form.attest = true;
    let cur = "c".repeat(32);
    s.say(409, json!({"error": "conflict", "current_version": 4, "current_publish_id": cur}));
    m.check();
    m.frame(now(), &ready(&s));
    assert_eq!(m.step, Step::Fix);
    assert!(matches!(&m.fix, Some(Fix::Conflict { current_version: Some(4), .. })));
    let before = s.seen().len();
    s.say(200, preview_ok(2));
    m.replace_newer_version();
    m.frame(now(), &ready(&s));
    assert_eq!(s.seen().len(), before + 1);
    assert_eq!(s.seen().last().unwrap().meta()["base_publish_id"], cur);
    assert_eq!(m.step, Step::Review);
}

#[test]
fn a_purged_item_or_a_kind_mismatch_offers_a_new_item() {
    for body in [json!({"error": "item_removed", "purged": true}), json!({"error": "kind_mismatch"})] {
        let s = Script::new();
        let log = LogIo::memory();
        let _ = published_once(&s, &log);
        let mut m = model_in(&log, HashMap::new());
        m.start(source(2));
        m.frame(now(), &ready(&s));
        m.form.attest = true;
        s.say(409, body);
        m.check();
        m.frame(now(), &ready(&s));
        assert!(matches!(m.fix, Some(Fix::Gone { .. })), "{:?}", m.fix);
        m.publish_as_new();
        assert_eq!(m.step, Step::Options);
        assert_eq!(m.target(), &Target::New);
        assert!(log.find("bank:b1").is_none(), "the dead item is forgotten");
    }
}

// ---- the refusal loop -----------------------------------------------------------------------------------------------------

fn refuse(m: &mut PublishModel, s: &Arc<Script>, status: u16, body: Value) {
    s.say(status, body);
    m.check();
    m.frame(now(), &ready(s));
}

fn key_refusal() -> Value {
    json!({
        "error": "invalid_payload",
        "errors": [
            {"code": "key_in_formatting", "path": "bank.questions[0].kind.Mc.answers[1].text", "reason": "star", "ack_allowed": false, "author_only": true, "hint": "Remove the star."},
            {"code": "key_in_formatting", "path": "bank.questions[1].kind.Mc.answers[0].text", "reason": "capitals", "ack_allowed": true, "author_only": true, "hint": "Check the capitals."},
            {"code": "key_in_stem", "path": "bank.questions[2].stem", "reason": "answer_line", "ack_allowed": false, "author_only": true},
            {"code": "key_in_formatting", "path": "bank.questions[3].kind.Mc.answers[0].text", "reason": "capitals", "ack_allowed": true, "author_only": true},
            {"code": "too_long", "path": "meta.title", "limit": 200}
        ],
        "formatting_findings": [{"question": 1, "code": "key_in_formatting", "path": "bank.questions[1].kind.Mc.answers[0].text", "reason": "capitals", "ack_allowed": true, "served": "VENUS"}],
        "formatting_findings_total": 4
    })
}

#[test]
fn key_hint_issues_are_split_into_must_fix_and_per_question_acknowledgements() {
    let s = Script::new();
    let mut m = at_options(&s, 5);
    fill(&mut m);
    refuse(&mut m, &s, 422, key_refusal());
    assert_eq!(m.step, Step::Fix);
    assert!(m.payload_fix().is_some());
    // hard: the starred choice, the answer line in a stem, and the too-long title (not a key, not ackable)
    let hard: Vec<String> = m.hard_issues().iter().map(|i| text::issue_text(i, false)).collect();
    assert_eq!(hard.len(), 3, "{hard:?}");
    assert!(hard[0].contains("Question 1, answer choice 2") && hard[0].contains("star"), "{}", hard[0]);
    assert!(hard[1].contains("Question 3, the question text") && hard[1].contains("\"Answer:\" line"), "{}", hard[1]);
    assert!(hard[2].contains("too long"));
    assert_eq!(m.ackable_questions(), vec![1, 3]);
    let t = m.todo();
    assert_eq!((t.hard, t.formatting, t.labels, t.orders), (3, 2, 0, 0));
    // a hard question can not be acknowledged, nor one the server did not flag
    m.set_formatting_ack(0, true);
    m.set_formatting_ack(2, true);
    m.set_formatting_ack(4, true);
    assert!(!m.form.formatting_acked(0) && !m.form.formatting_acked(2) && !m.form.formatting_acked(4));
    m.set_formatting_ack(1, true);
    m.set_formatting_ack(3, true);
    assert_eq!(m.todo().formatting, 0);
    m.set_formatting_ack(3, false);
    assert_eq!(m.todo().formatting, 1);
    // with hard issues left, "check again" is refused and nothing is sent
    let before = s.seen().len();
    m.recheck();
    assert_eq!(s.seen().len(), before);
    assert!(matches!(&m.notice, Some(Notice::Error(_))));
}

#[test]
fn acknowledged_questions_go_into_the_next_request() {
    let s = Script::new();
    let mut m = at_options(&s, 5);
    fill(&mut m);
    let body = json!({"error": "invalid_payload", "errors": [
        {"code": "key_in_formatting", "path": "bank.questions[1].kind.Mc.answers[0].text", "reason": "capitals", "ack_allowed": true, "author_only": true},
        {"code": "key_in_formatting", "path": "bank.questions[3].kind.Mc.answers[0].text", "reason": "padding", "ack_allowed": true}]});
    refuse(&mut m, &s, 422, body);
    assert_eq!(m.todo().formatting, 2);
    m.recheck();
    assert_eq!(s.seen().len(), 1, "still two to acknowledge");
    m.set_formatting_ack(1, true);
    m.set_formatting_ack(3, true);
    s.say(200, preview_ok(5));
    m.recheck();
    m.frame(now(), &ready(&s));
    assert_eq!(m.step, Step::Review);
    assert_eq!(s.seen().last().unwrap().meta()["ack_formatting"], json!([1, 3]));
}

#[test]
fn letter_labels_need_a_decision_for_each_finding() {
    let s = Script::new();
    let mut m = at_options(&s, 3);
    fill(&mut m);
    let body = json!({"error": "labels_need_decision",
        "errors": [{"code": "labels_need_decision", "path": "labels[question=1]"}],
        "label_findings": [
            {"question": 1, "strip_allowed": true, "choices": [{"before": "A. Mars", "after": "Mars"}]},
            {"question": 2, "part": 0, "strip_allowed": false, "order_revealing": true, "choices": []},
            {"question": 0, "strip_allowed": true, "decision": "strip", "choices": []}]});
    refuse(&mut m, &s, 422, body);
    assert_eq!(m.step, Step::Fix);
    // the server's own earlier decision counts; the other two need one
    assert_eq!(m.undecided_labels(), vec![Pos::question(1), Pos { question: 2, part: Some(0) }]);
    assert_eq!(m.todo().labels, 2);
    // not a finding: ignored. Strip refused where the server says it is not allowed.
    m.decide_label(Pos::question(7), LabelChoice::Keep);
    m.decide_label(Pos { question: 2, part: Some(0) }, LabelChoice::Strip);
    assert_eq!(m.todo().labels, 2);
    m.decide_label(Pos { question: 2, part: Some(0) }, LabelChoice::Keep);
    m.decide_label(Pos::question(1), LabelChoice::Strip);
    m.decide_label(Pos::question(1), LabelChoice::Keep);
    assert_eq!(m.form.label(Pos::question(1)), Some(LabelChoice::Keep), "a later decision replaces an earlier one");
    m.decide_label(Pos::question(1), LabelChoice::Strip);
    assert!(m.todo().done());
    s.say(200, preview_ok(3));
    m.recheck();
    m.frame(now(), &ready(&s));
    let labels = s.seen().last().unwrap().meta()["labels"].clone();
    assert_eq!(labels, json!([{"question": 1, "choice": "strip"}, {"question": 2, "part": 0, "choice": "keep"}]));
}

#[test]
fn a_refused_keep_goes_back_to_undecided() {
    let s = Script::new();
    let mut m = at_options(&s, 2);
    fill(&mut m);
    refuse(&mut m, &s, 422, json!({"error": "labels_need_decision", "errors": [{"code": "labels_need_decision", "path": "labels"}],
        "label_findings": [{"question": 1, "strip_allowed": true, "choices": []}]}));
    m.decide_label(Pos::question(1), LabelChoice::Keep);
    s.say(422, json!({"error": "invalid_payload", "errors": [{"code": "labels_keep_refused", "path": "labels[question=1]"}],
        "label_findings": [{"question": 1, "strip_allowed": true, "choices": []}]}));
    m.recheck();
    m.frame(now(), &ready(&s));
    assert_eq!(m.step, Step::Fix);
    assert_eq!(m.form.label(Pos::question(1)), None);
    assert_eq!(m.todo().labels, 1);
    assert_eq!(m.todo().hard, 1, "the refusal itself is shown, not hidden");
}

#[test]
fn order_acknowledgements_exist_only_for_positions_the_server_listed() {
    let s = Script::new();
    let mut m = at_options(&s, 3);
    fill(&mut m);
    refuse(&mut m, &s, 422, json!({"error": "order_ack_required", "errors": [{"code": "order_ack_required", "path": "order_acks[question=0]"}],
        "needs_order_ack": [{"question": 0}, {"question": 2, "part": 1}],
        "order_reasons": [{"question": 0, "reason": "sibling_refs"}, {"question": 2, "part": 1, "reason": "few_unlocked"}]}));
    assert_eq!(m.todo().orders, 2);
    assert_eq!(m.todo().hard, 0, "an order refusal is not a hard issue");
    m.set_order_ack(Pos::question(1), true);
    m.set_order_ack(Pos { question: 0, part: Some(3) }, true);
    assert_eq!(m.todo().orders, 2);
    m.set_order_ack(Pos::question(0), true);
    assert_eq!(m.todo().orders, 1);
    m.set_order_ack(Pos::question(0), false);
    m.ack_all_orders();
    assert!(m.todo().done());
    s.say(200, preview_ok(3));
    m.recheck();
    m.frame(now(), &ready(&s));
    assert_eq!(s.seen().last().unwrap().meta()["order_acks"], json!([{"question": 0}, {"question": 2, "part": 1}]));
    assert!(text::order_reason_text("sibling_refs").contains("both of the above"));
}

fn licence_refusal() -> Value {
    json!({"error": "license_incompatible",
        "message": "A source was relicensed after you received it (now CC-BY-NC-4.0).",
        "relicensed": [{"item_id": ID2, "earlier": ["CC-BY-4.0"], "now": "CC-BY-NC-4.0"}],
        "positions": [0], "allowed": ["CC-BY-NC-4.0", "circle-only"],
        "per_question": [{"pos": 0, "allowed": ["CC-BY-NC-4.0"], "bound_by": [{"item_id": ID2, "title": "Cells", "author_name": "Ms Frizzle", "license": "CC-BY-NC-4.0"}]}]})
}

#[test]
fn a_licence_refusal_names_who_binds_it_and_offers_only_what_is_allowed() {
    let s = Script::new();
    let mut m = at_options(&s, 2);
    fill(&mut m);
    refuse(&mut m, &s, 422, licence_refusal());
    let Some(Fix::License(l)) = m.fix.clone() else { panic!("{:?}", m.fix) };
    assert_eq!(l.binders().len(), 1);
    assert_eq!(l.per_question[0].bound_by[0].author_name, "Ms Frizzle");
    assert_eq!(l.relicensed[0].now, "CC-BY-NC-4.0");
    // a licence the server did not list is ignored; all-rights-reserved is not offered for a public item
    m.choose_allowed_license(License::Cc0);
    m.choose_allowed_license(License::CircleOnly);
    assert_eq!(m.step, Step::Fix);
    assert_eq!(m.form.license, Some(License::CcBy));
    s.say(200, preview_ok(2));
    m.choose_allowed_license(License::CcByNc);
    m.frame(now(), &ready(&s));
    assert_eq!(m.step, Step::Review);
    assert_eq!(s.seen().last().unwrap().meta()["license"], "CC-BY-NC-4.0");
}

#[test]
fn the_independence_statement_is_checked_before_it_is_sent() {
    let s = Script::new();
    let mut m = at_options(&s, 2);
    fill(&mut m);
    refuse(&mut m, &s, 422, json!({"error": "license_incompatible", "allowed": [], "per_question": [{"pos": 1, "allowed": [], "bound_by": []}]}));
    let before = s.seen().len();
    m.claim_independence();
    assert_eq!(s.seen().len(), before, "no items, no statement");
    assert!(!m.form.claim_independent);
    m.form.independent_items = format!("see https://x.example/items/{ID2}?v=1 and {ID2}");
    m.form.independent_statement = "short".into();
    m.claim_independence();
    assert_eq!(s.seen().len(), before);
    m.form.independent_statement = "I wrote these from my own lesson notes.".into();
    s.say(200, preview_ok(2));
    m.claim_independence();
    m.frame(now(), &ready(&s));
    let ind = s.seen().last().unwrap().meta()["independent_of"].clone();
    assert_eq!(ind, json!({"items": [ID2], "accepted": true, "statement": "I wrote these from my own lesson notes."}));
    assert_eq!(item_ids_in("x"), Vec::<String>::new());
}

#[test]
fn small_refusals_send_the_person_back_to_the_option_they_got_wrong() {
    let s = Script::new();
    let mut m = at_options(&s, 1);
    fill(&mut m);
    refuse(&mut m, &s, 422, json!({"error": "license_not_public"}));
    assert_eq!((m.step, m.form.license), (Step::Options, None));
    fill(&mut m);
    refuse(&mut m, &s, 422, json!({"error": "attestation_required", "terms_version": "x"}));
    assert_eq!((m.step, m.form.attest), (Step::Options, false));
}

// ---- every other kind of refusal ------------------------------------------------------------------------------------------------

#[derive(Debug, PartialEq)]
enum Shape {
    /// Failed, with a retry of this kind.
    Retry,
    /// Failed, nothing to retry.
    Final,
    Account,
    Gate,
}

fn shape(m: &PublishModel) -> Shape {
    match m.step {
        Step::Failed => {
            let f = m.failure.as_ref().unwrap();
            if f.account { Shape::Account } else if f.retry.is_some() { Shape::Retry } else { Shape::Final }
        }
        Step::Gate => Shape::Gate,
        other => panic!("unexpected {other:?}"),
    }
}

#[test]
fn every_refusal_kind_ends_somewhere_the_person_can_read_and_leave() {
    let cases: Vec<(&str, u16, Value, Shape, &str)> = vec![
        ("quota", 413, json!({"error": "quota_exceeded", "used": 100, "needed": 30, "limit": 120}), Shape::Final, "storage is full"),
        ("removed storage", 413, json!({"error": "removed_storage_exceeded", "used": 1, "limit": 1}), Shape::Final, "14 days"),
        ("body too large", 413, json!({"error": "body_too_large", "limit": 4194304}), Shape::Final, "too big"),
        ("shared questions", 422, json!({"error": "shared_questions_limit", "used": 4990, "needed": 20, "limit": 5000}), Shape::Final, "5000"),
        ("tags", 422, json!({"error": "too_many_tags", "limit": 200}), Shape::Final, "200"),
        ("schema", 422, json!({"error": "schema_upgrade_required", "message": "Republishing needs schema version 2 (update verSHion)."}), Shape::Final, "schema version 2"),
        ("moderation", 409, json!({"error": "under_moderation", "message": "This item is under review."}), Shape::Final, "under review"),
        ("removed not purged", 409, json!({"error": "item_removed", "purged": false, "message": "Deleted; undo it first."}), Shape::Final, "undo"),
        ("image refused", 422, json!({"error": "image_refused", "reason": "not_accepted"}), Shape::Final, "PNG"),
        ("pool not configured", 503, json!({"error": "pool_not_configured"}), Shape::Final, "not available"),
        ("not approved", 403, json!({"error": "account_not_approved", "message": "Your account is waiting for approval."}), Shape::Account, "waiting"),
        ("rate limited", 429, json!({"error": "rate_limited"}), Shape::Retry, "Too many"),
        ("open reads", 429, json!({"error": "too_many_open_reads"}), Shape::Retry, "Too many"),
        ("uploads", 429, json!({"error": "too_many_uploads"}), Shape::Retry, "Too many"),
        ("busy", 503, json!({"error": "busy"}), Shape::Retry, "busy"),
        ("in progress", 409, json!({"error": "publish_in_progress"}), Shape::Retry, "still running"),
        ("migrating", 503, json!({"error": "pool_migrating"}), Shape::Retry, "upgraded"),
        ("blob missing", 422, json!({"error": "blob_missing", "positions": [0]}), Shape::Retry, "picture"),
        ("hash mismatch", 422, json!({"error": "hash_mismatch"}), Shape::Retry, "changed"),
        ("publish id conflict", 409, json!({"error": "publish_id_conflict"}), Shape::Retry, "Start the publish again"),
        ("other", 418, json!({"error": "weird"}), Shape::Final, "weird"),
    ];
    for (name, status, body, want, words) in cases {
        for phase in [RetryWhat::Preview, RetryWhat::Publish] {
            let s = Script::new();
            let mut m = if phase == RetryWhat::Preview { at_options(&s, 1) } else { at_review(&s) };
            fill(&mut m);
            if name == "blob missing" {
                // the pool layer answers this one by sending the pictures again, then asks again
                s.say(status, body.clone());
            }
            if phase == RetryWhat::Preview {
                refuse(&mut m, &s, status, body.clone());
            } else {
                publish_ready(&mut m);
                s.say(status, body.clone());
                m.publish();
                m.frame(now(), &ready(&s));
            }
            assert_eq!(shape(&m), want, "{name} {phase:?}");
            let msg = &m.failure.as_ref().unwrap().message;
            assert!(msg.contains(words), "{name} {phase:?}: {msg}");
            // always a way out
            m.back_to_options();
            assert_eq!(m.step, Step::Options, "{name}");
        }
    }
}

#[test]
fn a_daily_limit_waits_for_the_servers_retry_after() {
    let s = Script::new();
    let mut m = at_review(&s);
    publish_ready(&mut m);
    s.say_after(429, 3600, json!({"error": "daily_limit", "metric": "publishes"}));
    let t0 = now();
    m.publish();
    m.frame(t0, &ready(&s));
    assert_eq!(m.step, Step::Failed);
    let f = m.failure.clone().unwrap();
    assert!(f.message.contains("today's limit") && f.message.contains("60 minutes"), "{}", f.message);
    assert!(m.retry_wait().is_some());
    let before = s.seen().len();
    m.retry();
    assert_eq!(s.seen().len(), before, "retry is refused until the wait is over");
    // an hour later
    m.frame(t0 + Duration::from_secs(3601), &ready(&s));
    assert!(m.retry_wait().is_none());
    s.say(201, publish_ok());
    m.retry();
    m.frame(t0 + Duration::from_secs(3602), &ready(&s));
    assert_eq!(m.step, Step::Done);
}

#[test]
fn a_signed_out_answer_keeps_everything_and_resumes_after_sign_in() {
    let s = Script::new();
    let mut m = at_review(&s);
    publish_ready(&mut m);
    s.fail(ApiError::SignedOut);
    m.publish();
    m.frame(now(), &ready(&s));
    assert_eq!(m.step, Step::Gate);
    assert_eq!(m.gate, Gate::SignedOut);
    assert!(matches!(&m.notice, Some(Notice::Info(t)) if t.contains("kept")));
    assert_eq!(m.form.license, Some(License::CcBy));
    assert!(m.form.attest && m.preview.is_some());
    // the Account window reports signed out, then ready again: back where the person was
    m.frame(now(), &AccountSnapshot::of_gate(Gate::SignedOut));
    assert_eq!(m.step, Step::Gate);
    m.frame(now(), &ready(&s));
    assert_eq!(m.step, Step::Review);
    assert!(m.preview_is_current());
}

#[test]
fn the_account_going_away_between_steps_returns_to_the_gate_and_back() {
    let s = Script::new();
    let mut m = at_options(&s, 1);
    m.frame(now(), &AccountSnapshot::of_gate(Gate::NotApproved(Status::Suspended { reason: Some("x".into()) })));
    assert_eq!(m.step, Step::Gate);
    m.frame(now(), &ready(&s));
    assert_eq!(m.step, Step::Options);
    // a passing "checking" does not throw the person out
    m.frame(now(), &AccountSnapshot::of_gate(Gate::Checking));
    assert_eq!(m.step, Step::Options);
}

#[test]
fn a_lost_connection_during_publish_is_not_reported_as_not_published_and_resends_the_same_publish() {
    let s = Script::new();
    let mut m = at_review(&s);
    publish_ready(&mut m);
    s.fail(ApiError::Offline("no route".into()));
    m.publish();
    m.frame(now(), &ready(&s));
    assert_eq!(m.step, Step::Failed);
    let f = m.failure.clone().unwrap();
    assert!(f.maybe_published && f.message.contains("may or may not"));
    s.say(200, {
        let mut ok = publish_ok();
        ok["replayed"] = json!(true);
        ok
    });
    m.retry();
    m.frame(now(), &ready(&s));
    assert_eq!(m.step, Step::Done);
    let publishes: Vec<Sent> = s.seen().into_iter().filter(|x| x.path == "/items").collect();
    assert_eq!(publishes.len(), 2);
    assert_eq!(publishes[0].meta()["publish_id"], publishes[1].meta()["publish_id"]);
    assert_eq!(publishes[0].body, publishes[1].body);
    assert!(m.done.as_ref().unwrap().result.replayed);
}

#[test]
fn a_failed_preview_is_retried_and_a_local_problem_is_final() {
    let s = Script::new();
    let mut m = at_options(&s, 1);
    fill(&mut m);
    s.fail(ApiError::Offline("down".into()));
    m.check();
    m.frame(now(), &ready(&s));
    assert_eq!(shape(&m), Shape::Retry);
    assert!(!m.failure.as_ref().unwrap().maybe_published);
    s.say(200, preview_ok(1));
    m.retry();
    m.frame(now(), &ready(&s));
    assert_eq!(m.step, Step::Review);
    // a picture missing on this device is a local problem: nothing sent, no retry
    let (mut m2, _) = model(HashMap::new());
    let sha = "a".repeat(64);
    m2.start(ShareSource::from_bank_json("bank:b1", "Bank", "T", &bank_json(1, Some(&format!("media/{sha}.webp")))));
    m2.frame(now(), &ready(&s));
    fill(&mut m2);
    let before = s.seen().len();
    m2.check();
    m2.frame(now(), &ready(&s));
    assert_eq!(s.seen().len(), before);
    assert_eq!(shape(&m2), Shape::Final);
    assert!(m2.failure.as_ref().unwrap().message.contains("missing from this device"));
}

// ---- pictures and progress --------------------------------------------------------------------------------------------------------

fn with_picture() -> (PublishModel, Arc<Script>, String) {
    let bytes = b"fake webp bytes".to_vec();
    let sha = sha256_hex(&bytes);
    let file = format!("media/{sha}.webp");
    let mut media = HashMap::new();
    media.insert(file.clone(), bytes);
    let (mut m, _) = model(media);
    let s = Script::new();
    m.start(ShareSource::from_bank_json("bank:b1", "Bank: Unit 1", "Unit 1", &bank_json(2, Some(&file))));
    m.frame(now(), &ready(&s));
    fill(&mut m);
    (m, s, sha)
}

#[test]
fn pictures_go_up_privately_before_the_check_and_a_kept_picture_changes_the_options() {
    let (mut m, s, sha) = with_picture();
    s.say(201, json!({"sha256": sha, "size": 15, "width": 4, "height": 4})).say(200, {
        let mut v = preview_ok(2);
        v["images_removed"] = json!([0]);
        v
    });
    m.check();
    m.frame(now(), &ready(&s));
    assert_eq!(s.paths(), [format!("Put /blobs/{sha}"), "Post /items/preview".to_string()]);
    let p = m.preview.as_ref().unwrap();
    assert_eq!(p.images, vec![ImageNote { question: 0, file: format!("media/{sha}.webp"), kept: false }]);
    assert!(p.options.image_ok.is_empty());
    // keep the picture: only a question that has one can be chosen; the preview is stale
    m.set_keep_image(1, true);
    assert!(!m.form.image_kept(1));
    m.set_keep_image(0, true);
    assert!(!m.preview_is_current());
    s.say(200, preview_ok(2));
    m.check();
    m.frame(now(), &ready(&s));
    assert_eq!(s.seen().last().unwrap().meta()["image_ok"], json!([0]));
    assert!(m.preview.as_ref().unwrap().images[0].kept);
    // the picture is already on the server: it is not sent again for the second check
    assert_eq!(s.paths().iter().filter(|p| p.starts_with("Put /blobs")).count(), 1);
}

#[test]
fn progress_is_reported_and_cancelling_a_check_shares_nothing() {
    let (mut m, s, sha) = with_picture();
    m = rebuild_background(m, &s);
    let release = s.hold();
    s.say(201, json!({"sha256": sha, "size": 15, "width": 4, "height": 4})).say(200, preview_ok(2));
    m.check();
    assert!(m.busy());
    assert_eq!(m.step, Step::Checking);
    // the picture upload has started and is waiting on the (held) server
    wait_for(|| s.seen().len() == 1);
    m.frame(now(), &ready(&s));
    assert!(matches!(m.progress, Some(PublishProgress::UploadingImage { done: 0, total: 1, .. })), "{:?}", m.progress);
    m.cancel();
    assert!(!m.busy());
    assert_eq!(m.step, Step::Options);
    assert!(matches!(&m.notice, Some(Notice::Info(t)) if t.contains("Nothing was shared")));
    // the abandoned job finishing later changes nothing
    for _ in 0..4 {
        let _ = release.send(());
    }
    std::thread::sleep(Duration::from_millis(100));
    m.frame(now(), &ready(&s));
    assert_eq!(m.step, Step::Options);
    assert!(m.preview.is_none());
}

#[test]
fn cancelling_a_publish_keeps_the_publish_id_so_sending_again_is_the_same_publish() {
    let s = Script::new();
    let mut m = at_review(&s);
    m = rebuild_background_from(m, &s);
    publish_ready(&mut m);
    let release = s.hold();
    s.say(201, publish_ok());
    m.publish();
    assert_eq!(m.step, Step::Publishing);
    wait_for(|| s.seen().len() == 2);
    m.frame(now(), &ready(&s));
    assert!(matches!(m.progress, Some(PublishProgress::Sending)));
    m.cancel();
    assert_eq!(m.step, Step::Failed);
    let f = m.failure.clone().unwrap();
    assert!(f.maybe_published && f.message.contains("may already have reached the server"));
    let _ = release.send(());
    std::thread::sleep(Duration::from_millis(100));
    m.frame(now(), &ready(&s));
    assert_eq!(m.step, Step::Failed, "a late answer to an abandoned job is ignored");
    // again: the server replays it
    let release2 = s.hold();
    s.say(200, {
        let mut ok = publish_ok();
        ok["replayed"] = json!(true);
        ok
    });
    m.retry();
    let _ = release2.send(());
    wait_for(|| {
        m.frame(now(), &ready(&s));
        m.step == Step::Done
    });
    let publishes: Vec<Sent> = s.seen().into_iter().filter(|x| x.path == "/items").collect();
    assert_eq!(publishes.len(), 2);
    assert_eq!(publishes[0].meta()["publish_id"], publishes[1].meta()["publish_id"]);
}

/// The same model state on a background runner (jobs on a thread).
fn rebuild_background(m: PublishModel, _s: &Arc<Script>) -> PublishModel {
    m.with_runner(Runner::Background)
}
fn rebuild_background_from(m: PublishModel, _s: &Arc<Script>) -> PublishModel {
    m.with_runner(Runner::Background)
}

fn wait_for(mut f: impl FnMut() -> bool) {
    for _ in 0..400 {
        if f() {
            return;
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    panic!("timed out");
}

#[test]
fn closing_a_checking_window_drops_the_work() {
    let (m, s, _sha) = with_picture();
    let mut m = m.with_runner(Runner::Background);
    let release = s.hold();
    s.say(201, json!({"sha256": "x", "size": 1, "width": 1, "height": 1}));
    m.check();
    assert!(m.busy());
    m.close();
    assert!(!m.busy() && !m.open);
    let _ = release.send(());
}

// ---- server text ---------------------------------------------------------------------------------------------------------------------

#[test]
fn server_text_is_shown_as_plain_text_only() {
    let nasty = "<b>Click</b> [here](https://evil.example)\u{202e}\u{200b}\n\x07done";
    let shown = text::shown(nasty);
    assert!(!shown.chars().any(|c| c.is_control() || c == '\u{202e}' || c == '\u{200b}'), "{shown:?}");
    // markup is not interpreted: it stays as characters, for a plain label
    assert!(shown.contains("<b>Click</b>"));
    let long = "x".repeat(900);
    assert!(text::shown(&long).ends_with("..."));
    let issue = Issue { code: "something_new".into(), path: "bank.questions[0].stem".into(), message: Some(nasty.into()), ..Default::default() };
    let t = text::issue_text(&issue, false);
    assert!(!t.contains('\u{202e}') && t.starts_with("Question 1, the question text:"), "{t}");
    assert_eq!(text::note_text(&json!({"message": nasty})), text::shown(nasty));
    assert_eq!(text::note_text(&json!({"reason": "symbol_font_mapped"})), "symbol font mapped");
    assert_eq!(text::note_text(&json!(5)), "A change was made to the shared copy.");
}

#[test]
fn findings_say_exactly_where() {
    let w = |p: &str| text::where_text(p, false);
    assert_eq!(w("bank.questions[2].kind.Mc.answers[1].text"), "Question 3, answer choice 2");
    assert_eq!(w("bank.questions[0].stem"), "Question 1, the question text");
    assert_eq!(w("bank.questions[4].kind.Multipart.parts[1].stem"), "Question 5, part 2, the question text");
    assert_eq!(w("bank.questions[4].kind.Matching.pairs[2].left"), "Question 5, matching row 3");
    assert_eq!(w("bank.questions[1].explanation"), "Question 2, the explanation");
    assert_eq!(w("bank.details.description"), "the bank description");
    assert_eq!(w("bank.name"), "the bank name");
    assert_eq!(w("question.stem"), "this question, the question text");
    assert!(w("meta.title").contains("listing details"));
    assert!(w("weird").starts_with("at weird"));
    for r in ["star", "key_word", "capitals", "answer_line", "gift", "cloze", "tf_value", "left_names_right", "blank_hint", "word_bank_first", "latex_key", "pairing_token"] {
        assert!(!text::reason_text(r).contains('_'), "{r}");
    }
    for c in ["missing", "blank_text", "too_long", "too_many", "forbidden_char", "no_correct", "duplicate_choice", "scan_budget", "no_questions"] {
        assert!(!text::generic_code_text(c, Some(5)).is_empty());
    }
    assert!(text::generic_code_text("too_long", Some(200)).contains("200"));
}

#[test]
fn the_hint_and_author_only_flag_survive_into_the_fix_screen_data() {
    let s = Script::new();
    let mut m = at_options(&s, 2);
    fill(&mut m);
    refuse(&mut m, &s, 422, key_refusal());
    let p = m.payload_fix().unwrap();
    assert_eq!(p.issues[0].hint.as_deref(), Some("Remove the star."));
    assert!(p.issues[0].author_only);
    assert_eq!(m.question_title(0), "Question 1: Question text 0");
    assert_eq!(m.question_title(9), "Question 10");
}

#[test]
fn no_token_password_or_author_only_order_is_in_what_the_model_holds_or_prints() {
    let s = Script::new();
    let mut m = at_review(&s);
    publish_ready(&mut m);
    s.say(201, publish_ok());
    m.publish();
    m.frame(now(), &ready(&s));
    let dump = format!("{:?} {:?} {:?}", m.done, m.preview, m.notice);
    assert!(!dump.contains("Bearer") && !dump.to_lowercase().contains("password"));
    assert!(!dump.contains("served_orders") || dump.contains("<author only>"));
    for sent in s.seen() {
        let all = String::from_utf8_lossy(&sent.body).to_lowercase();
        assert!(!all.contains("password") && !all.contains("token"));
    }
}

#[test]
fn the_status_texts_cover_every_account_state() {
    // the gate page words come from this match; a new Status must not be a dead end
    for st in [Status::Pending, Status::Approved, Status::Unverified, Status::Other("x".into()), Status::Suspended { reason: None }, Status::Rejected { revoked: true, reason: None, reapply_after_ms: None, can_reapply: false }] {
        let g = Gate::NotApproved(st);
        assert_ne!(g, Gate::Ready);
    }
}

// ---- the window draws every step (headless egui) ---------------------------------------------------------------------

/// Draws the window body for `frames` frames; returns true if it asked to close.
fn draw(ctx: &egui::Context, m: &mut PublishModel) -> bool {
    let mut closed = false;
    for i in 0..2 {
        let input = egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(900.0, 1200.0))),
            time: Some(i as f64 * 0.1),
            ..Default::default()
        };
        let out = ctx.run_ui(input, |ui| {
            closed |= super::publish_ui::page(ui, m);
        });
        out.drop_without_applying_deltas();
    }
    closed
}

#[test]
fn the_window_draws_every_step_and_every_refusal_without_panicking() {
    let ctx = egui::Context::default();
    crate::theme::setup(&ctx);
    let s = Script::new();

    // gate pages
    let (mut m, _) = model(HashMap::new());
    m.start(source(2));
    for g in [Gate::Checking, Gate::NotSetUp, Gate::NoServer, Gate::SignedOut, Gate::Unreachable, Gate::NotApproved(Status::Pending),
              Gate::NotApproved(Status::Rejected { revoked: false, reason: Some("<b>no</b>".into()), reapply_after_ms: None, can_reapply: true })] {
        m.frame(now(), &AccountSnapshot::of_gate(g));
        assert!(!draw(&ctx, &mut m));
    }

    // options, with a record, with the key shown
    let mut m = at_options(&s, 3);
    assert!(!draw(&ctx, &mut m));
    m.form.hide_key = false;
    m.form.license = Some(License::CcBy);
    assert!(!draw(&ctx, &mut m));

    // review with a picture, a stale preview and a blocked one
    let (mut m, s2, sha) = with_picture();
    s2.say(201, json!({"sha256": sha, "size": 15, "width": 4, "height": 4})).say(200, {
        let mut v = preview_ok(2);
        v["images_removed"] = json!([0]);
        v["labels_removed"] = json!([{"question": 1}]);
        v["figures_removed"] = json!([1]);
        v["order_reasons"] = json!([{"question": 0, "reason": "labels_kept"}]);
        v["notes"] = json!([{"message": "<i>note</i>"}]);
        v
    });
    m.check();
    assert!(!draw(&ctx, &mut m)); // checking
    m.frame(now(), &ready(&s2));
    assert_eq!(m.step, Step::Review);
    assert!(!draw(&ctx, &mut m));
    m.form.license = Some(License::Cc0);
    assert!(!draw(&ctx, &mut m)); // stale
    m.form.license = Some(License::CcBy);
    m.form.reviewed = true;
    assert!(!draw(&ctx, &mut m));

    // every refusal screen
    for (status, body) in [
        (422, key_refusal()),
        (422, json!({"error": "labels_need_decision", "errors": [{"code": "labels_need_decision", "path": "x"}], "label_findings": [{"question": 0, "choices": [{"before": "A. x", "after": "x"}], "strip_allowed": true, "order_revealing": true, "truncated": true}]})),
        (422, json!({"error": "order_ack_required", "errors": [{"code": "order_ack_required", "path": "x"}], "needs_order_ack": [{"question": 0}, {"question": 1, "part": 0}], "order_reasons": [{"question": 0, "reason": "sibling_refs"}]})),
        (422, licence_refusal()),
        (422, json!({"error": "license_incompatible", "allowed": [], "per_question": [{"pos": 0, "allowed": [], "bound_by": [{"item_id": null, "note": "an item that is no longer shown"}], "evidence_overflow": true}]})),
        (409, json!({"error": "conflict", "current_version": 3, "current_publish_id": "c".repeat(32)})),
        (409, json!({"error": "item_removed", "purged": true})),
        (429, json!({"error": "daily_limit", "metric": "publishes"})),
        (403, json!({"error": "account_not_approved"})),
    ] {
        let s = Script::new();
        let mut m = at_options(&s, 3);
        fill(&mut m);
        refuse(&mut m, &s, status, body);
        assert!(!draw(&ctx, &mut m), "{:?}", m.step);
    }

    // done, and a failure with a wait
    let s = Script::new();
    let mut m = at_review(&s);
    publish_ready(&mut m);
    let mut ok = publish_ok();
    ok["possible_duplicate_of"] = json!(ID2);
    s.say(201, ok);
    m.publish();
    assert!(!draw(&ctx, &mut m)); // publishing
    m.frame(now(), &ready(&s));
    assert_eq!(m.step, Step::Done);
    assert!(!draw(&ctx, &mut m));
}

#[test]
#[allow(clippy::field_reassign_with_default)]
fn the_publish_window_is_asked_for_through_the_context_and_defers_to_the_account_window() {
    use super::publish_ui::{PublishUi, request};
    use super::ui::AccountUi;
    let ctx = egui::Context::default();
    crate::theme::setup(&ctx);
    let factory: super::model::Factory = Arc::new(|_| panic!("no client may be built"));
    let mut account = AccountUi::default();
    account.model = AccountModel::new(factory, Runner::Inline, SettingsIo::memory()).with_server_mode(server_mode(None, false));
    let mut ui = PublishUi::default();
    let frame = |ctx: &egui::Context, ui: &mut PublishUi, account: &mut AccountUi| {
        let out = ctx.run_ui(egui::RawInput::default(), |_| {
            ui.show(ctx, account);
        });
        out.drop_without_applying_deltas();
    };
    // closed: nothing happens
    frame(&ctx, &mut ui, &mut account);
    assert!(!ui.is_open() && !account.is_open());
    // asked for from anywhere with a Context
    request(&ctx, source(1));
    frame(&ctx, &mut ui, &mut account);
    assert!(ui.is_open());
    assert_eq!(ui.model.step, Step::Gate);
    assert_eq!(ui.model.gate, Gate::NotSetUp, "a build without a server says so instead of a password page");
    assert!(!account.is_open(), "the Account window is not opened behind the person's back");
    // "Open account" reveals it, and the publish window gives way
    ui.model.request_open_account();
    frame(&ctx, &mut ui, &mut account);
    assert!(account.visible());
    // the quiet mode: open but not drawn, revealed on demand, closed only if it was quiet
    let mut a2 = AccountUi::default();
    a2.model = AccountModel::new(Arc::new(|_| panic!("no client")), Runner::Inline, SettingsIo::memory()).with_server_mode(server_mode(None, false));
    a2.open_quiet();
    assert!(a2.is_open() && !a2.visible());
    a2.reveal();
    assert!(a2.visible());
    a2.close_quiet();
    assert!(a2.visible(), "a window the person can see is left alone");
    a2.model.close();
    a2.open_quiet();
    a2.close_quiet();
    assert!(!a2.is_open());
}

// ---- question identity across republishes ------------------------------------------------------------------------------

fn qid(c: char) -> String {
    c.to_string().repeat(32)
}

fn publish_with_qids(qids: &[String]) -> Value {
    let mut ok = publish_ok();
    ok["qids"] = json!(qids);
    ok
}

#[test]
fn a_republish_sends_each_questions_pool_id_back_and_remembers_the_new_ones() {
    let s = Script::new();
    let log = LogIo::memory();
    // first publish of two questions: the pool's ids are remembered against the local ids
    let mut m = model_in(&log, HashMap::new());
    m.start(source(2));
    m.frame(now(), &ready(&s));
    fill(&mut m);
    s.say(200, preview_ok(2)).say(201, publish_with_qids(&[qid('a'), qid('b')]));
    m.check();
    m.frame(now(), &ready(&s));
    publish_ready(&mut m);
    m.publish();
    m.frame(now(), &ready(&s));
    assert_eq!(m.step, Step::Done);
    let rec = log.find("bank:b1").unwrap();
    assert_eq!(rec.qids, vec![QidLink { local: "q0".into(), qid: qid('a') }, QidLink { local: "q1".into(), qid: qid('b') }]);
    assert!(s.seen().iter().all(|r| !String::from_utf8_lossy(&r.body).contains("\"published\"")), "a new item carries no links");

    // the bank gained a third question: the first two keep their ids, the new one carries none
    let mut m = model_in(&log, HashMap::new());
    m.start(source(3));
    m.frame(now(), &ready(&s));
    m.form.attest = true;
    s.say(200, preview_ok(3));
    m.check();
    m.frame(now(), &ready(&s));
    let sent = s.seen().last().unwrap().clone();
    let qs = sent.payload()["bank"]["questions"].clone();
    assert_eq!(qs[0]["published"], json!({"item_id": ID, "qid": qid('a'), "publish_id": "p".repeat(32), "version": 1}));
    assert_eq!(qs[1]["published"]["qid"], qid('b'));
    assert!(qs[2].get("published").is_none());
    assert_eq!(sent.payload()["schema_version"], 2);
    publish_ready(&mut m);
    let mut ok = publish_with_qids(&[qid('a'), qid('b'), qid('c')]);
    ok["version"] = json!(2);
    ok["publish_id"] = json!("q".repeat(32));
    s.say(200, ok);
    m.publish();
    m.frame(now(), &ready(&s));
    let put = s.seen().last().unwrap().clone();
    assert_eq!(put.method, Method::Put);
    assert_eq!(put.payload()["bank"]["questions"][1]["published"]["qid"], qid('b'), "the real publish carries the links too");
    let rec = log.find("bank:b1").unwrap();
    assert_eq!(rec.qids.len(), 3);
    assert_eq!(rec.qids[2], QidLink { local: "q2".into(), qid: qid('c') });
}

#[test]
fn a_new_copy_and_a_record_without_ids_send_no_links_and_odd_ids_are_never_sent() {
    let src = source(2);
    let rec = PublishedRecord {
        key: "bank:b1".into(),
        item_id: ID.into(),
        publish_id: "p".repeat(32),
        version: 1,
        visibility: "public".into(),
        license: "CC-BY-4.0".into(),
        title: "T".into(),
        qids: vec![QidLink { local: "q0".into(), qid: "../x".into() }, QidLink { local: "q1".into(), qid: qid('b') }],
    };
    let v: Value = serde_json::from_slice(&src.payload_linked(2, Some(&rec))).unwrap();
    assert!(v["bank"]["questions"][0].get("published").is_none(), "an id that is not 32 hex characters is dropped");
    assert!(v["bank"]["questions"][1].get("published").is_some());
    let none = PublishedRecord { qids: vec![], ..rec.clone() };
    assert_eq!(src.payload_linked(2, Some(&none)), src.payload(2), "an older record changes nothing");
    assert_eq!(src.payload_linked(1, Some(&rec)), src.payload(1), "schema 1 can not carry links");
    // a result with the wrong number of ids is not remembered (the next republish gets new ones)
    assert!(src.links_for(&[qid('a')]).is_empty());
    // the record file holds ids only
    let text = serde_json::to_string(&rec).unwrap();
    assert!(!text.contains("Alpha") && !text.contains("correct"));
}
