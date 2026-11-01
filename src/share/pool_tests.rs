//! `share::pool` against a small in-process HTTP server (std::net only, 127.0.0.1, an OS picked
//! port; nothing here touches a real service). The JSON answers are copied from the Worker's
//! README and code (`server/worker/src/pool`) and share-core's wire types.

use std::collections::{HashMap, HashSet};
use std::io::{BufRead, BufReader, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use serde_json::{Value, json};

use super::api::ApiError;
use super::pool::*;
use super::secret::Secret;
use super::transport::{LocalFuture, Mode, NativeTransport, Request, Response, Transport, block_on};
use super::vault::MemoryVault;

const ID: &str = "0123456789abcdef0123456789abcdef";
const ID2: &str = "fedcba9876543210fedcba9876543210";

// ---- the server ---------------------------------------------------------------------------------

#[derive(Clone, Debug, Default)]
struct Seen {
    line: String,
    headers: Vec<(String, String)>,
    body: Vec<u8>,
}

impl Seen {
    fn header(&self, name: &str) -> Option<&str> {
        self.headers.iter().find(|(k, _)| k.eq_ignore_ascii_case(name)).map(|(_, v)| v.as_str())
    }
    fn method(&self) -> &str {
        self.line.split(' ').next().unwrap()
    }
    fn target(&self) -> &str {
        self.line.split(' ').nth(1).unwrap()
    }
    fn json(&self) -> Value {
        serde_json::from_slice(&self.body).unwrap()
    }
}

struct Server {
    port: u16,
    seen: Arc<Mutex<Vec<Seen>>>,
}

impl Server {
    fn base(&self) -> String {
        format!("http://127.0.0.1:{}", self.port)
    }
    fn seen(&self) -> Vec<Seen> {
        self.seen.lock().unwrap().clone()
    }
}

fn serve(handler: impl Fn(usize, &Seen, &mut TcpStream) + Send + Sync + 'static) -> Server {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    let seen = Arc::new(Mutex::new(Vec::new()));
    let (log, handler, n) = (seen.clone(), Arc::new(handler), Arc::new(AtomicUsize::new(0)));
    std::thread::spawn(move || {
        for stream in listener.incoming() {
            let Ok(stream) = stream else { break };
            let (log, handler, n) = (log.clone(), handler.clone(), n.clone());
            std::thread::spawn(move || {
                let mut reader = BufReader::new(stream.try_clone().unwrap());
                let mut s = Seen::default();
                reader.read_line(&mut s.line).unwrap();
                s.line = s.line.trim_end().to_string();
                loop {
                    let mut l = String::new();
                    if reader.read_line(&mut l).unwrap_or(0) == 0 || l == "\r\n" {
                        break;
                    }
                    if let Some((k, v)) = l.trim_end().split_once(':') {
                        s.headers.push((k.to_string(), v.trim().to_string()));
                    }
                }
                if let Some(len) = s.header("content-length").and_then(|v| v.parse::<usize>().ok()) {
                    s.body = vec![0; len];
                    reader.read_exact(&mut s.body).unwrap();
                }
                let i = n.fetch_add(1, Ordering::SeqCst);
                log.lock().unwrap().push(s.clone());
                let mut out = stream;
                handler(i, &s, &mut out);
            });
        }
    });
    Server { port, seen }
}

fn reply(out: &mut TcpStream, status: &str, headers: &[(&str, &str)], body: &[u8]) {
    let mut head = format!("HTTP/1.1 {status}\r\nConnection: close\r\n");
    for (k, v) in headers {
        head.push_str(&format!("{k}: {v}\r\n"));
    }
    head.push_str(&format!("Content-Length: {}\r\n\r\n", body.len()));
    let _ = out.write_all(head.as_bytes());
    let _ = out.write_all(body);
}

fn ok_json(out: &mut TcpStream, status: &str, v: Value) {
    reply(out, status, &[("Content-Type", "application/json")], v.to_string().as_bytes());
}

/// Answers every request with the same canned reply.
fn canned(status: &'static str, headers: &'static [(&'static str, &'static str)], body: Value) -> Server {
    serve(move |_, _, out| reply(out, status, &[&[("Content-Type", "application/json")], headers].concat(), body.to_string().as_bytes()))
}

// ---- the session ---------------------------------------------------------------------------------

/// A session with a fixed bearer token over the real native transport.
struct TestSession {
    base: String,
    transport: NativeTransport,
}

impl TestSession {
    fn new(srv: &Server) -> TestSession {
        TestSession { base: srv.base(), transport: NativeTransport::default() }
    }
}

impl PoolSession for TestSession {
    fn base(&self) -> String {
        self.base.clone()
    }
    fn send<'a>(&'a self, build: &'a dyn Fn() -> Request) -> LocalFuture<'a, Result<Response, ApiError>> {
        Box::pin(async move {
            let mut r = build();
            r.headers.push(("Authorization".into(), Secret::new("Bearer test-token")));
            Ok(self.transport.send(r).await?)
        })
    }
}

fn run<T>(srv: &Server, f: impl for<'a> FnOnce(Pool<'a>) -> LocalFuture<'a, T>) -> T {
    let s = TestSession::new(srv);
    block_on(f(Pool::new(&s).with_retry(RetryPolicy::instant(3))))
}

fn no_progress() -> impl FnMut(PublishProgress) {
    |_| {}
}

// ---- payloads --------------------------------------------------------------------------------------

fn bank(images: &[&str]) -> Vec<u8> {
    let qs: Vec<Value> = images
        .iter()
        .enumerate()
        .map(|(i, f)| json!({"id": format!("q{i}"), "stem": "Pick one", "image": {"file": f, "width": 4, "height": 4}, "kind": {"Short": {"sample": "x"}}}))
        .collect();
    let qs = if qs.is_empty() { vec![json!({"id": "q0", "stem": "Pick one", "image": null, "kind": {"Short": {"sample": "x"}}})] } else { qs };
    json!({"schema_version": 2, "bank": {"name": "Unit 1", "questions": qs}}).to_string().into_bytes()
}

fn opts() -> PublishOptions {
    PublishOptions::new(License::CcBy, Visibility::Public, true)
}

fn media(files: &[(&[u8],)]) -> (HashMap<String, Vec<u8>>, Vec<String>) {
    let mut m = HashMap::new();
    let mut names = Vec::new();
    for (b,) in files {
        let name = format!("media/{}.webp", sha256_hex(b));
        m.insert(name.clone(), b.to_vec());
        names.push(name);
    }
    (m, names)
}

fn publish_ok() -> Value {
    json!({
        "item_id": ID, "publish_id": "p".repeat(32), "version": 1, "kind": "bank", "visibility": "public",
        "license": "CC-BY-4.0", "hide_key": true, "qids": ["a".repeat(32)], "public_version": 1,
        "possible_duplicate_of": null,
        "served_orders": [{"question": 0, "order": [2, 0, 1]}],
        "order_reasons": [{"question": 3, "reason": "sibling_refs"}],
        "labels_removed": [], "images_kept": [], "images_removed": [], "figures_removed": [],
        "dropped": [{"reason": "x", "message": "Dropped"}]
    })
}

fn publish(srv: &Server, req: &PublishRequest, m: &HashMap<String, Vec<u8>>, order: BlobOrder) -> Result<PublishResult, PoolError> {
    let s = TestSession::new(srv);
    block_on(Pool::new(&s).with_retry(RetryPolicy::instant(3)).publish(req, m, order, &mut no_progress()))
}

fn refusal_of(status: &'static str, headers: &'static [(&'static str, &'static str)], body: Value) -> (PoolError, Server) {
    let srv = canned(status, headers, body);
    let m = HashMap::new();
    let req = PublishRequest::new_item(bank(&[]), opts());
    let e = publish(&srv, &req, &m, BlobOrder::First).unwrap_err();
    (e, srv)
}

// ---- publish: request shape -----------------------------------------------------------------------

#[test]
fn a_new_publish_is_a_post_with_the_publish_type_an_exact_length_and_the_meta_line() {
    let srv = serve(|_, _, out| ok_json(out, "201 Created", publish_ok()));
    let payload = bank(&[]);
    let mut o = opts();
    o.meta = ItemMeta { title: "Cells".into(), subject: "Biology".into(), tags: vec!["cells".into()], ..Default::default() };
    o.decide_label(Pos { question: 1, part: Some(0) }, LabelChoice::Strip);
    o.decide_label(Pos::question(2), LabelChoice::Keep);
    o.order_acks = vec![Pos::question(0)];
    o.ack_formatting = vec![4, 5];
    o.image_ok = vec![0];
    o.independent_of = Some(IndependentOf { items: vec![ID2.into()], statement: "I wrote these myself last year.".into() });
    let req = PublishRequest::new_item(payload.clone(), o);
    let r = publish(&srv, &req, &HashMap::new(), BlobOrder::First).unwrap();
    assert_eq!(r.item_id, ID);
    assert_eq!(r.version, 1);
    assert_eq!(r.qids.len(), 1);
    assert!(!r.replayed);
    assert_eq!(r.order_reasons[0].reason, "sibling_refs");
    // The author-only orders are not printed.
    assert!(!format!("{r:?}").contains("order\":"), "{r:?}");
    assert!(format!("{r:?}").contains("author only"));

    let seen = srv.seen();
    assert_eq!(seen.len(), 1);
    let s = &seen[0];
    assert_eq!(s.line, "POST /items HTTP/1.1");
    assert_eq!(s.header("Content-Type"), Some("application/vnd.vershion.publish"));
    assert_eq!(s.header("Authorization"), Some("Bearer test-token"));
    assert_eq!(s.header("X-Client"), Some("native"));
    assert_eq!(s.header("Content-Length").unwrap().parse::<usize>().unwrap(), s.body.len());
    // <meta JSON>\n<payload bytes>, the payload untouched.
    let nl = s.body.iter().position(|b| *b == b'\n').unwrap();
    assert_eq!(&s.body[nl + 1..], &payload[..]);
    let meta: Value = serde_json::from_slice(&s.body[..nl]).unwrap();
    assert_eq!(meta["publish_id"], req.publish_id);
    assert_eq!(meta["hide_key"], true);
    assert_eq!(meta["license"], "CC-BY-4.0");
    assert_eq!(meta["visibility"], "public");
    assert_eq!(meta["attestation"], json!({"accepted": true, "text_rev": "2026-10-19"}));
    assert_eq!(meta["labels"], json!([{"question": 1, "part": 0, "choice": "strip"}, {"question": 2, "choice": "keep"}]));
    assert_eq!(meta["order_acks"], json!([{"question": 0}]));
    assert_eq!(meta["ack_formatting"], json!([4, 5]));
    assert_eq!(meta["image_ok"], json!([0]));
    assert_eq!(meta["meta"], json!({"title": "Cells", "subject": "Biology", "tags": ["cells"]}));
    assert_eq!(meta["independent_of"], json!({"items": [ID2], "accepted": true, "statement": "I wrote these myself last year."}));
    assert!(meta.get("base_publish_id").is_none());
}

#[test]
fn a_republish_is_a_put_with_the_base_publish_and_no_visibility() {
    let srv = serve(|_, _, out| ok_json(out, "200 OK", publish_ok()));
    let base = "b".repeat(32);
    let req = PublishRequest::republish(ID, &base, bank(&[]), opts());
    publish(&srv, &req, &HashMap::new(), BlobOrder::OnDemand).unwrap();
    let s = &srv.seen()[0];
    assert_eq!(s.line, format!("PUT /items/{ID} HTTP/1.1"));
    let nl = s.body.iter().position(|b| *b == b'\n').unwrap();
    let meta: Value = serde_json::from_slice(&s.body[..nl]).unwrap();
    assert_eq!(meta["base_publish_id"], base);
    assert!(meta.get("visibility").is_none());
}

#[test]
fn a_replayed_answer_says_so() {
    let mut v = publish_ok();
    v["replayed"] = json!(true);
    let srv = serve(move |_, _, out| ok_json(out, "200 OK", v.clone()));
    let r = publish(&srv, &PublishRequest::new_item(bank(&[]), opts()), &HashMap::new(), BlobOrder::First).unwrap();
    assert!(r.replayed);
}

#[test]
fn publish_problems_are_caught_before_anything_is_sent() {
    let srv = serve(|_, _, out| ok_json(out, "201 Created", publish_ok()));
    let m = HashMap::new();
    let local = |req: &PublishRequest| match publish(&srv, req, &m, BlobOrder::First) {
        Err(PoolError::Local(l)) => l,
        other => panic!("{other:?}"),
    };
    // circle-only can not be public
    let mut o = opts();
    o.license = License::CircleOnly;
    assert_eq!(local(&PublishRequest::new_item(bank(&[]), o.clone())), LocalProblem::LicenseNotPublic);
    o.visibility = Visibility::Unlisted;
    assert!(publish(&srv, &PublishRequest::new_item(bank(&[]), o), &m, BlobOrder::First).is_ok());
    // independence claims need items and a statement
    let mut o = opts();
    o.independent_of = Some(IndependentOf { items: vec![ID.into()], statement: "short".into() });
    assert!(matches!(local(&PublishRequest::new_item(bank(&[]), o.clone())), LocalProblem::IndependenceClaim(_)));
    o.independent_of = Some(IndependentOf { items: vec!["nope".into()], statement: "long enough statement".into() });
    assert!(matches!(local(&PublishRequest::new_item(bank(&[]), o)), LocalProblem::IndependenceClaim(_)));
    // ids
    let mut r = PublishRequest::new_item(bank(&[]), opts());
    r.publish_id = "short".into();
    assert_eq!(local(&r), LocalProblem::BadPublishId);
    let r = PublishRequest::republish("not-an-id", &"b".repeat(32), bank(&[]), opts());
    assert!(matches!(local(&r), LocalProblem::BadId(_)));
    // size and shape
    let big = PublishRequest::new_item(vec![b' '; MAX_PAYLOAD + 1], opts());
    assert!(matches!(local(&big), LocalProblem::PayloadTooLarge { .. }));
    assert_eq!(local(&PublishRequest::new_item(b"[1]".to_vec(), opts())), LocalProblem::PayloadNotJson);
    // the one request that went out is the unlisted circle-only one
    assert_eq!(srv.seen().len(), 1);
}

#[test]
fn publish_ids_are_valid_and_do_not_repeat() {
    let ids: std::collections::HashSet<String> = (0..200).map(|_| new_publish_id()).collect();
    assert_eq!(ids.len(), 200);
    assert!(ids.iter().all(|i| is_publish_id(i) && i.len() == 32));
}

// ---- images and the upload order ---------------------------------------------------------------------

#[test]
fn pictures_go_up_before_the_publish_each_once() {
    let srv = serve(|_, s, out| {
        if s.target().starts_with("/blobs/") {
            ok_json(out, "201 Created", json!({"sha256": s.target().trim_start_matches("/blobs/"), "size": s.body.len(), "width": 4, "height": 4}));
        } else {
            ok_json(out, "201 Created", publish_ok());
        }
    });
    let (m, names) = media(&[(b"first picture bytes",), (b"second picture bytes",)]);
    // three questions, the first picture twice
    let payload = bank(&[&names[0], &names[1], &names[0]]);
    let req = PublishRequest::new_item(payload, opts());
    let mut events = Vec::new();
    let s = TestSession::new(&srv);
    block_on(Pool::new(&s).publish(&req, &m, BlobOrder::First, &mut |p| events.push(p))).unwrap();

    let seen = srv.seen();
    let lines: Vec<&str> = seen.iter().map(|s| s.line.as_str()).collect();
    assert_eq!(lines.len(), 3, "{lines:?}");
    assert!(lines[0].starts_with("PUT /blobs/") && lines[1].starts_with("PUT /blobs/") && lines[2] == "POST /items HTTP/1.1", "{lines:?}");
    for (s, want) in seen[..2].iter().zip([&b"first picture bytes"[..], &b"second picture bytes"[..]]) {
        assert_eq!(s.body, want, "raw bytes");
        assert_eq!(s.header("Content-Length").unwrap().parse::<usize>().unwrap(), want.len());
        assert_eq!(s.header("Content-Type"), Some("application/octet-stream"));
        assert_eq!(s.header("Authorization"), Some("Bearer test-token"));
        assert!(s.line.contains(&sha256_hex(want)), "named by its sha256");
    }
    assert_eq!(events.first(), Some(&PublishProgress::UploadingImage { done: 0, total: 2, file: names[0].clone() }));
    assert_eq!(events.last(), Some(&PublishProgress::Sending));
}

#[test]
fn on_demand_publishes_first_and_uploads_only_when_the_server_asks() {
    let srv = serve(|i, s, out| match i {
        0 => ok_json(out, "422 Unprocessable Entity", json!({"error": "blob_missing", "positions": [1]})),
        _ if s.target().starts_with("/blobs/") => ok_json(out, "201 Created", json!({"sha256": "x", "size": 1, "width": 1, "height": 1})),
        _ => ok_json(out, "200 OK", publish_ok()),
    });
    let (m, names) = media(&[(b"one",), (b"two",)]);
    let req = PublishRequest::republish(ID, &"b".repeat(32), bank(&[&names[0], &names[1]]), opts());
    let mut events = Vec::new();
    let s = TestSession::new(&srv);
    block_on(Pool::new(&s).publish(&req, &m, BlobOrder::OnDemand, &mut |p| events.push(p))).unwrap();
    let seen = srv.seen();
    let lines: Vec<String> = seen.iter().map(|s| s.line.clone()).collect();
    assert_eq!(lines.len(), 3, "{lines:?}");
    assert!(lines[0].starts_with("PUT /items/") && lines[2].starts_with("PUT /items/"));
    // only the picture of question 1 was sent
    assert!(lines[1].contains(&sha256_hex(b"two")), "{lines:?}");
    // the retry is the same publish: same id, same bytes
    assert_eq!(seen[0].body, seen[2].body);
    assert!(events.contains(&PublishProgress::ResendingImages { count: 1 }));
}

#[test]
fn a_lost_picture_is_sent_again_once_then_the_refusal_stands() {
    let srv = serve(|_, s, out| {
        if s.target().starts_with("/blobs/") {
            ok_json(out, "201 Created", json!({"sha256": "x", "size": 1, "width": 1, "height": 1}));
        } else {
            ok_json(out, "422 Unprocessable Entity", json!({"error": "blob_missing", "reason": "reupload", "positions": [0]}));
        }
    });
    let (m, names) = media(&[(b"pic",)]);
    let e = publish(&srv, &PublishRequest::new_item(bank(&[&names[0]]), opts()), &m, BlobOrder::First).unwrap_err();
    assert_eq!(e.kind(), Some(&RefusalKind::BlobMissing { positions: vec![0], reupload: true }));
    // blob, publish, blob again, publish again, and no more
    assert_eq!(srv.seen().len(), 4);
}

#[test]
fn picture_problems_stop_the_publish_before_the_network() {
    let srv = canned("201 Created", &[], publish_ok());
    let (mut m, names) = media(&[(b"pic",)]);
    let local = |m: &HashMap<String, Vec<u8>>, payload: Vec<u8>| match publish(&srv, &PublishRequest::new_item(payload, opts()), m, BlobOrder::First) {
        Err(PoolError::Local(l)) => l,
        other => panic!("{other:?}"),
    };
    assert!(matches!(local(&HashMap::new(), bank(&[&names[0]])), LocalProblem::MissingMedia { .. }));
    assert!(matches!(local(&m, bank(&["media/zz.webp"])), LocalProblem::BadMediaName { .. }));
    let bad = format!("media/{}.webp", "0".repeat(64));
    m.insert(bad.clone(), b"not what the name says".to_vec());
    assert!(matches!(local(&m, bank(&[&bad])), LocalProblem::MediaHashMismatch { .. }));
    let huge = vec![7u8; MAX_IMAGE_BYTES + 1];
    let (mh, nh) = media(&[(&huge,)]);
    assert!(matches!(local(&mh, bank(&[&nh[0]])), LocalProblem::MediaTooLarge { .. }));
    assert_eq!(srv.seen().len(), 0);
}

#[test]
fn a_refused_picture_is_the_same_answer_for_every_reason() {
    let srv = canned("422 Unprocessable Entity", &[], json!({"error": "image_refused", "reason": "not_accepted"}));
    let e = run(&srv, |p| Box::pin(async move { p.put_blob(&sha256_hex(b"abc"), b"abc").await }));
    let e = e.unwrap_err();
    assert_eq!(e.kind(), Some(&RefusalKind::ImageRefused));
    assert_eq!(e.refusal().unwrap().detail.reason.as_deref(), Some("not_accepted"));
    // the bytes must match the name, locally
    let e = run(&srv, |p| Box::pin(async move { p.put_blob(&sha256_hex(b"abc"), b"abd").await })).unwrap_err();
    assert_eq!(e, PoolError::Local(LocalProblem::BlobHashMismatch));
    assert_eq!(srv.seen().len(), 1);
}

#[test]
fn a_blob_answer_is_parsed() {
    let sha = sha256_hex(b"abc");
    let s2 = sha.clone();
    let srv = serve(move |_, _, out| ok_json(out, "201 Created", json!({"sha256": s2, "size": 3, "width": 10, "height": 20})));
    let info = run(&srv, |p| Box::pin(async move { p.put_blob(&sha256_hex(b"abc"), b"abc").await })).unwrap();
    assert_eq!(info, BlobInfo { sha256: sha, size: 3, width: 10, height: 20 });
}

// ---- refusals ----------------------------------------------------------------------------------------

#[test]
fn a_formatting_refusal_keeps_the_served_text_reasons_hints_and_what_can_be_acknowledged() {
    let body = json!({
        "error": "invalid_payload",
        "errors": [
            {"code": "key_in_formatting", "path": "bank.questions[2].kind.Mc.answers[1].text", "reason": "star", "ack_allowed": false, "author_only": true, "hint": "Remove the star from the correct choice."},
            {"code": "key_in_formatting", "path": "bank.questions[5].kind.Mc.answers[0].text", "reason": "capitals", "ack_allowed": true, "author_only": true, "hint": "Check the capitals."},
            {"code": "key_in_stem", "path": "bank.questions[7].stem", "reason": "answer_line", "ack_allowed": false, "author_only": true, "hint": "Take the answer out of the question."}
        ],
        "formatting_findings": [
            {"question": 2, "code": "key_in_formatting", "path": "bank.questions[2].kind.Mc.answers[1].text", "reason": "star", "ack_allowed": false, "served": "Mars *"},
            {"question": 5, "code": "key_in_formatting", "path": "bank.questions[5].kind.Mc.answers[0].text", "reason": "capitals", "ack_allowed": true, "served": "VENUS"}
        ],
        "formatting_findings_total": 9,
        "order_preview": [{"question": 2, "movable": [0, 1, 2, 3]}, {"question": 4, "matching": true, "movable": [0, 1, 2]}, {"question": 6, "movable": [], "frozen": "sibling_refs", "frozen_if_keep": true}]
    });
    let (e, _srv) = refusal_of("422 Unprocessable Entity", &[], body.clone());
    // ErrorDetail is whole: status, code, the first error's reason, ack_allowed, author_only, and the raw JSON.
    let d = &e.refusal().unwrap().detail;
    assert_eq!((d.status, d.code.as_str()), (422, "invalid_payload"));
    assert_eq!(d.reason.as_deref(), Some("star"));
    assert_eq!(d.ack_allowed, Some(false));
    assert!(d.author_only);
    assert_eq!(d.json, body);
    let Some(RefusalKind::InvalidPayload(p)) = e.kind() else { panic!("{:?}", e.kind()) };
    assert_eq!(p.issues.len(), 3);
    assert_eq!(p.issues[0].hint.as_deref(), Some("Remove the star from the correct choice."));
    assert_eq!(p.issues[0].question_index(), Some(2));
    assert_eq!(p.issues[2].question_index(), Some(7));
    assert!(p.issues.iter().all(Issue::is_key_leak));
    assert_eq!(p.formatting_findings.len(), 2);
    assert_eq!(p.formatting_findings_total, 9);
    assert_eq!(p.formatting_findings[0].served.as_deref(), Some("Mars *"));
    assert!(p.formatting_findings[1].ack_allowed);
    // only question 5 can be acknowledged; 2 and 7 have to be edited
    assert_eq!(p.acknowledgeable_questions(), vec![5]);
    assert_eq!(p.blocking().len(), 2);
    assert_eq!(p.order_preview[1].movable, vec![0, 1, 2]);
    assert!(p.order_preview[1].matching);
    assert_eq!(p.order_preview[2].frozen.as_deref(), Some("sibling_refs"));
    assert!(p.order_preview[2].frozen_if_keep);
    // taking the acknowledgement over into the options
    let mut o = opts();
    o.acknowledge_formatting(&p.acknowledgeable_questions());
    o.acknowledge_formatting(&[5]);
    assert_eq!(o.ack_formatting, vec![5]);
    assert!(e.message().contains("listed below"));
}

#[test]
fn a_label_refusal_lists_the_findings_to_decide() {
    let body = json!({
        "error": "labels_need_decision",
        "errors": [{"code": "labels_need_decision", "path": "labels[question=1]"}],
        "label_findings": [
            {"question": 1, "order_revealing": false, "strip_allowed": true, "choices": [{"before": "A. Mars", "after": "Mars"}, {"before": "B. Venus", "after": "Venus"}]},
            {"question": 3, "part": 1, "matching": true, "order_revealing": true, "strip_allowed": true, "decision": "keep", "truncated": true}
        ],
        "label_findings_total": 2,
        "order_preview": []
    });
    let (e, _s) = refusal_of("422 Unprocessable Entity", &[], body);
    let Some(RefusalKind::LabelsNeedDecision(p)) = e.kind() else { panic!("{:?}", e.kind()) };
    assert_eq!(p.label_findings[0].choices[1], LabelPreview { before: "B. Venus".into(), after: "Venus".into() });
    assert!(p.label_findings[1].matching && p.label_findings[1].order_revealing && p.label_findings[1].truncated);
    assert_eq!(p.label_findings[1].decision, Some(LabelChoice::Keep));
    assert_eq!(p.undecided_labels(), vec![Pos::question(1)]);
    assert!(p.blocking().is_empty());
    let mut o = opts();
    o.decide_label(Pos::question(1), LabelChoice::Strip);
    o.decide_label(Pos::question(1), LabelChoice::Keep);
    assert_eq!(o.labels, vec![LabelDecision { question: 1, part: None, choice: LabelChoice::Keep }]);
}

#[test]
fn an_order_refusal_carries_the_positions_and_why() {
    let body = json!({
        "error": "order_ack_required",
        "errors": [{"code": "order_ack_required", "path": "order_acks[question=0]"}],
        "needs_order_ack": [{"question": 0}, {"question": 4, "part": 2}],
        "order_reasons": [{"question": 0, "reason": "sibling_refs"}, {"question": 4, "part": 2, "reason": "few_unlocked"}],
        "order_preview": [{"question": 0, "movable": [], "frozen": "sibling_refs"}]
    });
    let (e, _s) = refusal_of("422 Unprocessable Entity", &[], body);
    let Some(RefusalKind::OrderAckRequired(p)) = e.kind() else { panic!("{:?}", e.kind()) };
    assert_eq!(p.needs_order_ack, vec![Pos::question(0), Pos { question: 4, part: Some(2) }]);
    assert_eq!(p.order_reasons[1], OrderNote { question: 4, part: Some(2), reason: "few_unlocked".into() });
    let mut o = opts();
    o.acknowledge_orders(p);
    o.acknowledge_orders(p);
    assert_eq!(o.order_acks.len(), 2);
}

#[test]
fn a_licence_refusal_names_who_binds_each_question() {
    let body = json!({
        "error": "license_incompatible",
        "relicensed": [{"item_id": ID2, "earlier": ["CC-BY-4.0"], "now": "CC-BY-NC-4.0"}],
        "message": "A source was relicensed after you received it (now CC-BY-NC-4.0).",
        "positions": [0, 3],
        "allowed": ["CC-BY-NC-4.0"],
        "per_question": [
            {"pos": 0, "allowed": ["CC-BY-NC-4.0"], "bound_by": [{"item_id": ID2, "title": "Cells", "author_name": "Ms Frizzle", "license": "CC-BY-NC-4.0", "via": null}]},
            {"pos": 3, "allowed": [], "bound_by": [{"item_id": null, "title": "", "author_name": "", "note": "an item that is no longer shown"}], "evidence_overflow": true}
        ]
    });
    let (e, _s) = refusal_of("422 Unprocessable Entity", &[], body);
    let Some(RefusalKind::LicenseIncompatible(l)) = e.kind() else { panic!("{:?}", e.kind()) };
    assert_eq!(l.positions, vec![0, 3]);
    assert_eq!(l.allowed, vec!["CC-BY-NC-4.0"]);
    assert_eq!(l.per_question[0].bound_by[0].title, "Cells");
    assert_eq!(l.per_question[0].bound_by[0].item_id.as_deref(), Some(ID2));
    assert_eq!(l.per_question[0].bound_by[0].license.as_deref(), Some("CC-BY-NC-4.0"));
    assert!(l.per_question[1].evidence_overflow);
    assert_eq!(l.per_question[1].bound_by[0].item_id, None);
    assert_eq!(l.per_question[1].bound_by[0].note.as_deref(), Some("an item that is no longer shown"));
    assert_eq!(l.relicensed[0], Relicensed { item_id: ID2.into(), earlier: vec!["CC-BY-4.0".into()], now: "CC-BY-NC-4.0".into() });
    assert_eq!(l.binders().len(), 2);
    assert!(e.message().contains("relicensed"));
    assert_eq!(e.refusal().unwrap().detail.json["positions"], json!([0, 3]));
}

#[test]
fn limits_and_states_are_sorted() {
    let kind = |status: &'static str, body: Value| refusal_of(status, &[], body).0.kind().cloned().unwrap();
    assert_eq!(kind("413 Payload Too Large", json!({"error": "quota_exceeded", "used": 100, "needed": 30, "limit": 120})), RefusalKind::QuotaExceeded(LimitInfo { used: 100, needed: 30, limit: 120 }));
    assert_eq!(kind("422 Unprocessable Entity", json!({"error": "shared_questions_limit", "used": 4990, "needed": 20, "limit": 5000})), RefusalKind::SharedQuestionsLimit(LimitInfo { used: 4990, needed: 20, limit: 5000 }));
    assert_eq!(kind("409 Conflict", json!({"error": "conflict", "current_version": 4, "current_publish_id": "cur"})), RefusalKind::Conflict { current_version: Some(4), current_publish_id: Some("cur".into()) });
    assert_eq!(kind("409 Conflict", json!({"error": "item_removed", "purged": true, "message": "gone"})), RefusalKind::ItemRemoved { purged: true, message: Some("gone".into()) });
    assert_eq!(kind("409 Conflict", json!({"error": "publish_id_conflict"})), RefusalKind::PublishIdConflict);
    assert_eq!(kind("422 Unprocessable Entity", json!({"error": "license_not_public"})), RefusalKind::LicenseNotPublic);
    assert_eq!(kind("422 Unprocessable Entity", json!({"error": "kind_mismatch"})), RefusalKind::KindMismatch);
    assert_eq!(kind("422 Unprocessable Entity", json!({"error": "too_many_tags", "limit": 200})), RefusalKind::TooManyTags { limit: 200 });
    assert_eq!(kind("422 Unprocessable Entity", json!({"error": "schema_upgrade_required", "message": "Republishing needs schema version 2 (update verSHion)."})), RefusalKind::SchemaUpgradeRequired { message: "Republishing needs schema version 2 (update verSHion).".into() });
    assert_eq!(kind("413 Payload Too Large", json!({"error": "body_too_large", "limit": 4194304})), RefusalKind::BodyTooLarge { limit: Some(4194304) });
    assert_eq!(kind("400 Bad Request", json!({"error": "invalid_request", "field": "visibility"})), RefusalKind::InvalidRequest { field: Some("visibility".into()) });
    assert_eq!(kind("403 Forbidden", json!({"error": "account_not_approved", "message": "Your account is waiting for approval."})), RefusalKind::NotApproved { message: Some("Your account is waiting for approval.".into()) });
    assert_eq!(kind("404 Not Found", json!({"error": "not_found"})), RefusalKind::NotFound);
    assert_eq!(kind("418 Teapot", json!({"error": "weird"})), RefusalKind::Other);
}

#[test]
fn a_daily_limit_is_not_slept_on_and_keeps_its_wait() {
    let (e, srv) = refusal_of("429 Too Many Requests", &[("Retry-After", "3600")], json!({"error": "daily_limit", "metric": "publishes"}));
    assert_eq!(e.kind(), Some(&RefusalKind::DailyLimit { metric: Some("publishes".into()) }));
    assert_eq!(e.retry_after(), Some(3600));
    assert_eq!(srv.seen().len(), 1, "no retry");
    assert!(e.message().contains("publishing") && e.message().contains("minutes"), "{}", e.message());
}

#[test]
fn pool_not_configured_is_its_own_answer_and_is_not_retried() {
    let (e, srv) = refusal_of("503 Service Unavailable", &[], json!({"error": "pool_not_configured"}));
    assert!(e.is_pool_not_configured());
    assert_eq!(e.status(), 503);
    assert_eq!(srv.seen().len(), 1);
    assert!(e.message().contains("not available"));
}

#[test]
fn busy_and_publish_in_progress_are_retried_with_the_same_publish_id() {
    let srv = serve(|i, _, out| match i {
        0 => reply(out, "503 Service Unavailable", &[("Content-Type", "application/json"), ("Retry-After", "10")], br#"{"error":"busy"}"#),
        1 => reply(out, "409 Conflict", &[("Content-Type", "application/json"), ("Retry-After", "5")], br#"{"error":"publish_in_progress"}"#),
        _ => ok_json(out, "201 Created", publish_ok()),
    });
    let r = publish(&srv, &PublishRequest::new_item(bank(&[]), opts()), &HashMap::new(), BlobOrder::First).unwrap();
    assert_eq!(r.item_id, ID);
    let seen = srv.seen();
    assert_eq!(seen.len(), 3);
    assert_eq!(seen[0].body, seen[1].body);
    assert_eq!(seen[1].body, seen[2].body);
}

#[test]
fn retries_stop_at_the_attempt_limit() {
    let srv = canned("503 Service Unavailable", &[("Retry-After", "1")], json!({"error": "busy"}));
    let e = publish(&srv, &PublishRequest::new_item(bank(&[]), opts()), &HashMap::new(), BlobOrder::First).unwrap_err();
    assert_eq!(e.kind(), Some(&RefusalKind::Busy));
    assert_eq!(srv.seen().len(), 3);
}

#[test]
fn a_wait_longer_than_the_limit_is_not_slept() {
    let p = RetryPolicy::default();
    assert_eq!(p.wait(0, None), Some(Duration::from_millis(500)));
    assert_eq!(p.wait(2, None), Some(Duration::from_secs(2)));
    assert_eq!(p.wait(0, Some(10)), Some(Duration::from_secs(10)));
    assert_eq!(p.wait(0, Some(3600)), None);
    assert_eq!(RetryPolicy::none().attempts, 1);
}

/// A session whose first sends fail as if offline.
struct Flaky {
    fails: AtomicUsize,
    sent: AtomicUsize,
}

impl PoolSession for Flaky {
    fn base(&self) -> String {
        "https://pool.example.org".into()
    }
    fn send<'a>(&'a self, _build: &'a dyn Fn() -> Request) -> LocalFuture<'a, Result<Response, ApiError>> {
        self.sent.fetch_add(1, Ordering::SeqCst);
        let n = self.fails.load(Ordering::SeqCst);
        Box::pin(std::future::ready(if n > 0 {
            self.fails.store(n - 1, Ordering::SeqCst);
            Err(ApiError::Offline("Can't reach pool.example.org: the connection failed.".into()))
        } else {
            let mut r = Response::new(200, r#"{"ok":true,"message":"Thanks"}"#);
            r.content_type = Some("application/json".into());
            Ok(r)
        }))
    }
}

#[test]
fn a_lost_connection_is_retried_only_for_calls_that_are_safe_to_repeat() {
    let f = Flaky { fails: AtomicUsize::new(2), sent: AtomicUsize::new(0) };
    let pool = Pool::new(&f).with_retry(RetryPolicy::instant(4));
    // a report is not safe to repeat blindly
    let e = block_on(pool.report(ID, ReportReason::Other, None)).unwrap_err();
    assert!(matches!(e, PoolError::Api(ApiError::Offline(_))));
    assert_eq!(f.sent.load(Ordering::SeqCst), 1);
    // a read is
    f.fails.store(2, Ordering::SeqCst);
    f.sent.store(0, Ordering::SeqCst);
    assert!(block_on(pool.mine()).is_ok());
    assert_eq!(f.sent.load(Ordering::SeqCst), 3);
}

// ---- browse -------------------------------------------------------------------------------------------

#[test]
fn browse_builds_its_query_and_reads_a_page() {
    let page = json!({
        "items": [{
            "id": ID, "kind": "bank", "title": "Cells", "subject": "Biology", "course": "Bio 1", "grade": "9", "kinds": ["Mc", "Tf"],
            "question_count": 12, "version": 3, "hide_key": true, "license": "CC-BY-4.0",
            "license_check": {"status": "unverified_earlier_match", "basis": "recorded_sources", "positions": [{"pos": 2, "allowed": []}], "reason": "r"},
            "has_images": true, "author_name": "Ms Frizzle", "committed_at": 1760000000000i64
        }, {
            "id": ID2, "kind": "question", "title": "Mine", "subject": "", "course": "", "grade": "", "kinds": ["Short"], "question_count": 1,
            "version": 1, "hide_key": false, "license": "CC0-1.0", "has_images": false, "author_name": "Me", "committed_at": 1,
            "visibility": "unlisted", "is_author": true
        }],
        "next_cursor": "abc.def"
    });
    let srv = serve(move |_, _, out| ok_json(out, "200 OK", page.clone()));
    let q = BrowseQuery {
        q: Some("cell wall".into()),
        subject: Some("Biology & Life".into()),
        kind: Some("Mc".into()),
        tag: Some("cells".into()),
        standard: Some("HS-LS1-2".into()),
        license: Some("CC-BY-4.0".into()),
        limit: Some(500),
        cursor: Some("prev.cur".into()),
        ..Default::default()
    };
    let p = run(&srv, |pool| Box::pin(async move { pool.browse(&q).await })).unwrap();
    let s = &srv.seen()[0];
    assert_eq!(s.method(), "GET");
    let target = url::Url::parse(&format!("http://x{}", s.target())).unwrap();
    assert_eq!(target.path(), "/items");
    let qs: HashMap<_, _> = target.query_pairs().into_owned().collect();
    assert_eq!(qs["q"], "cell wall");
    assert_eq!(qs["subject"], "Biology & Life");
    assert_eq!(qs["kind"], "Mc");
    assert_eq!(qs["tag"], "cells");
    assert_eq!(qs["standard"], "HS-LS1-2");
    assert_eq!(qs["license"], "CC-BY-4.0");
    assert_eq!(qs["cursor"], "prev.cur");
    assert_eq!(qs["limit"], "50", "capped like the Worker");
    assert!(!qs.contains_key("scope"));
    assert_eq!(s.header("Content-Length"), None, "a GET has no body");

    assert_eq!(p.next_cursor.as_deref(), Some("abc.def"));
    assert_eq!(p.items.len(), 2);
    assert_eq!(p.items[0].kinds, vec!["Mc", "Tf"]);
    let lc = p.items[0].license_check.as_ref().unwrap();
    assert_eq!(lc.status, LicenseStatus::UnverifiedEarlierMatch);
    assert_eq!(lc.positions[0].pos, 2);
    assert!(p.items[1].is_author);
    assert_eq!(p.items[1].visibility.as_deref(), Some("unlisted"));
    assert!(p.items[1].license_check.is_none());
}

#[test]
fn browse_my_own_and_bad_queries() {
    let srv = serve(|_, _, out| ok_json(out, "200 OK", json!({"items": [], "next_cursor": null})));
    let p = run(&srv, |pool| Box::pin(async move { pool.browse(&BrowseQuery { scope: Scope::Mine, ..Default::default() }).await })).unwrap();
    assert!(p.items.is_empty() && p.next_cursor.is_none());
    assert_eq!(srv.seen()[0].target(), "/items?scope=mine");
    assert_eq!(BrowseQuery::default().to_path().unwrap(), "/items");
    assert!(BrowseQuery { q: Some("  ".into()), ..Default::default() }.to_path().is_err());
    assert!(BrowseQuery { tag: Some("x".repeat(65)), ..Default::default() }.to_path().is_err());
    assert!(BrowseQuery { limit: Some(0), ..Default::default() }.to_path().is_err());
    assert_eq!(srv.seen().len(), 1);
}

#[test]
fn an_unknown_licence_status_does_not_break_a_page() {
    let srv = serve(|_, _, out| {
        ok_json(out, "200 OK", json!({"items": [{"id": ID, "license_check": {"status": "from_the_future", "basis": "recorded_sources"}}], "next_cursor": null}))
    });
    let p = run(&srv, |pool| Box::pin(async move { pool.browse(&BrowseQuery::default()).await })).unwrap();
    assert_eq!(p.items[0].license_check.as_ref().unwrap().status, LicenseStatus::Other);
}

// ---- reading an item ---------------------------------------------------------------------------------------

fn item_head() -> Value {
    json!({
        "item": {
            "id": ID, "kind": "bank", "title": "Cells", "subject": "Biology", "course": "", "grade": "", "kinds": ["Mc"], "question_count": 2,
            "version": 3, "publish_id": "pub-3", "license": "CC-BY-SA-4.0", "hide_key": true, "has_images": false, "author_name": "Ms Frizzle",
            "committed_at": 1760000000000i64, "form": "stripped", "is_author": false, "tags": ["cells"], "standards": ["HS-LS1-2"]
        },
        "questions": [
            {"pos": 0, "qid": "q0", "served_sha256": "s0", "match_hash": "m0", "chain": [
                {"author_name": "Mr A", "item_id": ID2, "qid": "qq", "version": 1, "license": "CC-BY-4.0", "modified": false},
                {"author_name": "and 3 contributors between", "item_id": "", "qid": "", "version": 0, "license": "", "modified": true, "collapsed_licenses": ["CC-BY-4.0", "CC0-1.0"]}
            ]},
            {"pos": 1, "qid": "q1", "served_sha256": "s1", "match_hash": "m1", "chain": [{"author_name": "Ms B", "item_id": ID2, "qid": "q9", "version": 2, "license": "CC0-1.0", "modified": true, "earlier_match": true, "possible_source": true}]}
        ],
        "license_check": {"status": "ok", "basis": "recorded_sources"}
    })
}

/// What the Worker streams: the head with its closing brace cut off, `,"payload":`, the stored bytes, `}`.
fn streamed(head: &Value, payload: &str, extra: Option<Value>) -> Vec<u8> {
    let mut h = head.clone();
    if let Some(Value::Object(e)) = extra {
        h.as_object_mut().unwrap().extend(e);
    }
    let h = h.to_string();
    format!("{},\"payload\":{}}}", &h[..h.len() - 1], payload).into_bytes()
}

const PAYLOAD: &str = "{ \"schema_version\":2,\n \"bank\" : {\"name\":\"a \\\"payload\\\": b\",\"questions\":[]} , \"dup\":1,\"dup\":2}";

#[test]
fn an_item_read_splits_the_head_from_the_payload_bytes_and_uses_the_etag_on_the_second_look() {
    let body = streamed(&item_head(), PAYLOAD, None);
    let b2 = body.clone();
    let srv = serve(move |_, s, out| {
        if s.header("If-None-Match") == Some("\"abc123\"") {
            reply(out, "304 Not Modified", &[("ETag", "\"abc123\"")], b"");
        } else {
            reply(out, "200 OK", &[("Content-Type", "application/json"), ("ETag", "\"abc123\"")], &b2);
        }
    });
    let mut cache = ItemCache::new(1024 * 1024);
    let s = TestSession::new(&srv);
    let pool = Pool::new(&s);
    let d1 = block_on(pool.read_item(ID, Some(&mut cache))).unwrap();
    assert!(!d1.from_cache);
    assert_eq!(d1.etag.as_deref(), Some("\"abc123\""));
    assert_eq!(d1.payload, PAYLOAD.as_bytes(), "the payload is byte for byte what was stored (spacing, duplicate keys)");
    assert!(d1.import.is_none());
    let h = &d1.head;
    assert_eq!(h.item.title, "Cells");
    assert_eq!(h.item.form, "stripped");
    assert_eq!(h.item.publish_id, "pub-3");
    assert_eq!(h.item.standards, vec!["HS-LS1-2"]);
    assert_eq!(h.license_check.as_ref().unwrap().status, LicenseStatus::Ok);
    assert_eq!(h.questions[0].served_sha256.as_deref(), Some("s0"));
    assert!(h.questions[0].chain[1].is_placeholder());
    assert_eq!(h.questions[0].chain[1].collapsed_licenses, vec!["CC-BY-4.0", "CC0-1.0"]);
    assert!(!h.questions[0].chain[0].is_placeholder());
    assert!(h.questions[1].chain[0].earlier_match && h.questions[1].chain[0].possible_source && h.questions[1].chain[0].modified);

    let d2 = block_on(pool.read_item(ID, Some(&mut cache))).unwrap();
    assert!(d2.from_cache);
    assert_eq!(d2.payload, d1.payload);
    let seen = srv.seen();
    assert_eq!(seen[0].header("If-None-Match"), None);
    assert_eq!(seen[1].header("If-None-Match"), Some("\"abc123\""));
    assert_eq!(seen[0].line, format!("GET /items/{ID} HTTP/1.1"));
    assert_eq!(cache.len(), 1);
}

#[test]
fn the_item_cache_is_bounded_and_keeps_only_items_with_an_etag() {
    let doc = |n: usize, etag: Option<&str>| ItemDoc { head: ItemHead::default(), import: None, payload: vec![b'x'; n], etag: etag.map(str::to_string), from_cache: false };
    // the cache has no public insert: go through reads
    let srv = serve(|_, s, out| {
        let big = "x".repeat(600);
        let body = streamed(&item_head(), &format!("\"{big}\""), None);
        if s.target().ends_with(ID2) {
            reply(out, "200 OK", &[("ETag", "\"2\"")], &body);
        } else if s.target().ends_with("ffffffffffffffffffffffffffffffff") {
            reply(out, "200 OK", &[], &body); // no ETag: nothing to cache
        } else {
            reply(out, "200 OK", &[("ETag", "\"1\"")], &body);
        }
    });
    let _ = doc;
    let mut cache = ItemCache::new(1000);
    let s = TestSession::new(&srv);
    let pool = Pool::new(&s);
    block_on(pool.read_item(ID, Some(&mut cache))).unwrap();
    assert_eq!(cache.len(), 1);
    block_on(pool.read_item(ID2, Some(&mut cache))).unwrap();
    assert_eq!(cache.len(), 1, "600 + 600 bytes do not fit 1000: the older one went");
    assert!(cache.get(ID).is_none() && cache.get(ID2).is_some());
    block_on(pool.read_item("ffffffffffffffffffffffffffffffff", Some(&mut cache))).unwrap();
    assert_eq!(cache.len(), 1);
    cache.clear();
    assert!(cache.is_empty());
}

#[test]
fn an_import_returns_the_licence_check_what_may_be_republished_and_the_chain() {
    let body = streamed(
        &item_head(),
        r#"{"schema_version":2,"question":{}}"#,
        Some(json!({"license": "CC-BY-SA-4.0", "may_republish_as": ["CC-BY-SA-4.0", "CC-BY-NC-SA-4.0"], "previously_imported_versions": [1, 2], "images": {"held": 2, "until": 1762592000000i64}, "recorded": true})),
    );
    let srv = serve(move |_, _, out| reply(out, "200 OK", &[("Content-Type", "application/json")], &body));
    let d = run(&srv, |p| Box::pin(async move { p.import_item(ID).await })).unwrap();
    let s = &srv.seen()[0];
    assert_eq!(s.line, format!("POST /items/{ID}/import HTTP/1.1"));
    assert_eq!(s.header("Content-Length"), Some("0"));
    let i = d.import.unwrap();
    assert_eq!(i.license, "CC-BY-SA-4.0");
    assert_eq!(i.may_republish_as, vec!["CC-BY-SA-4.0", "CC-BY-NC-SA-4.0"]);
    assert_eq!(i.previously_imported_versions, vec![1, 2]);
    assert_eq!(i.images, ImageHold { held: 2, until: Some(1762592000000), reason: None });
    assert!(i.recorded);
    assert_eq!(d.head.license_check.unwrap().status, LicenseStatus::Ok);
    assert_eq!(d.head.questions[1].chain[0].license, "CC0-1.0");
    assert_eq!(d.payload, br#"{"schema_version":2,"question":{}}"#);
}

#[test]
fn an_import_of_an_item_with_no_room_for_pictures_says_why() {
    let body = streamed(&item_head(), "{}", Some(json!({"license": "CC0-1.0", "may_republish_as": [], "previously_imported_versions": [], "images": {"held": 0, "reason": "quota"}, "recorded": true})));
    let srv = serve(move |_, _, out| reply(out, "200 OK", &[], &body));
    let d = run(&srv, |p| Box::pin(async move { p.import_item(ID).await })).unwrap();
    assert_eq!(d.import.unwrap().images.reason.as_deref(), Some("quota"));
}

#[test]
fn item_ids_are_checked_before_a_request_that_would_count_as_a_miss() {
    let srv = canned("200 OK", &[], json!({}));
    let e = run(&srv, |p| Box::pin(async move { p.read_item("../../etc", None).await })).unwrap_err();
    assert!(matches!(e, PoolError::Local(LocalProblem::BadId(_))));
    let e = run(&srv, |p| Box::pin(async move { p.import_item("short").await })).unwrap_err();
    assert!(matches!(e, PoolError::Local(LocalProblem::BadId(_))));
    assert_eq!(srv.seen().len(), 0);
}

#[test]
fn a_hidden_item_is_a_not_found() {
    let srv = canned("404 Not Found", &[], json!({"error": "not_found"}));
    let e = run(&srv, |p| Box::pin(async move { p.read_item(ID, None).await })).unwrap_err();
    assert_eq!(e.kind(), Some(&RefusalKind::NotFound));
}

#[test]
fn an_item_stamp_is_a_head_request() {
    let srv = serve(|_, _, out| reply(out, "200 OK", &[("ETag", "\"zz\"")], b""));
    let st = run(&srv, |p| Box::pin(async move { p.item_stamp(ID).await })).unwrap();
    assert_eq!(st.etag.as_deref(), Some("\"zz\""));
    assert_eq!(srv.seen()[0].method(), "HEAD");
}

#[test]
fn a_served_picture_is_checked_against_its_name() {
    let bytes = b"RIFF....WEBPVP8L picture".to_vec();
    let sha = sha256_hex(&bytes);
    let b2 = bytes.clone();
    let srv = serve(move |_, s, out| {
        if s.target().ends_with(&sha256_hex(b"RIFF....WEBPVP8L picture")) {
            reply(out, "200 OK", &[("Content-Type", "image/webp")], &b2);
        } else {
            reply(out, "200 OK", &[("Content-Type", "image/webp")], b"other bytes");
        }
    });
    let b = run(&srv, |p| {
        let sha = sha.clone();
        Box::pin(async move { p.get_blob(ID, &sha).await })
    })
    .unwrap();
    assert_eq!(b.bytes, bytes);
    assert_eq!(b.content_type.as_deref(), Some("image/webp"));
    assert_eq!(srv.seen()[0].line, format!("GET /items/{ID}/blobs/{} HTTP/1.1", sha256_hex(&bytes)));
    let wrong = sha256_hex(b"something else");
    let e = run(&srv, |p| Box::pin(async move { p.get_blob(ID, &wrong).await })).unwrap_err();
    assert_eq!(e, PoolError::Local(LocalProblem::BlobHashMismatch));
}

// ---- versions, mine, prefs ---------------------------------------------------------------------------------

#[test]
fn versions_covers_every_state_and_splits_big_lists() {
    let srv = serve(|_, s, out| {
        let ids: Vec<String> = s.json()["ids"].as_array().unwrap().iter().map(|x| x.as_str().unwrap().to_string()).collect();
        let items: Vec<Value> = if ids.len() > 5 {
            ids.iter().map(|i| json!({"id": i, "status": "current", "version": 1})).collect()
        } else {
            vec![
                json!({"id": "a", "status": "current", "version": 3, "license": "CC-BY-4.0", "license_version": 1}),
                json!({"id": "b", "status": "newer", "version": 5, "license": "CC0-1.0", "license_version": 2}),
                json!({"id": "c", "status": "under_review", "reason": "held"}),
                json!({"id": "d", "status": "under_review", "reason": "takedown", "until": 1762592000000i64}),
                json!({"id": "e", "status": "removed", "reason": "deleted"}),
                json!({"id": "f", "status": "unavailable"}),
            ]
        };
        ok_json(out, "200 OK", json!({"items": items}));
    });
    let ids: Vec<String> = ["a", "b", "c", "d", "e"].iter().map(|s| s.to_string()).collect();
    let v = run(&srv, |p| Box::pin(async move { p.versions(&ids).await })).unwrap();
    assert_eq!(v[0].state, VersionState::Current { version: 3, license: Some("CC-BY-4.0".into()), license_version: Some(1) });
    assert_eq!(v[1].state, VersionState::Newer { version: 5, license: Some("CC0-1.0".into()), license_version: Some(2) });
    assert_eq!(v[2].state, VersionState::UnderReview { reason: "held".into(), until: None });
    assert_eq!(v[3].state, VersionState::UnderReview { reason: "takedown".into(), until: Some(1762592000000) });
    assert_eq!(v[4].state, VersionState::Removed { reason: "deleted".into() });
    assert_eq!(v[5].state, VersionState::Unavailable);
    assert_eq!(srv.seen()[0].line, "POST /items/versions HTTP/1.1");
    assert_eq!(srv.seen()[0].header("Content-Type"), Some("application/json"));

    let many: Vec<String> = (0..450).map(|i| format!("{i:032x}")).collect();
    let v = run(&srv, |p| Box::pin(async move { p.versions(&many).await })).unwrap();
    assert_eq!(v.len(), 450);
    assert_eq!(srv.seen().len(), 1 + 3, "200 + 200 + 50");
}

#[test]
fn mine_shows_held_removed_and_unlisted_states_with_their_notices() {
    let mine = json!({
        "items": [
            {"id": ID, "kind": "bank", "title": "Live one", "subject": "Bio", "version": 2, "visibility": "public", "license": "CC-BY-4.0", "hide_key": true, "question_count": 10, "committed_at": 1760000000000i64, "import_count": 4, "state": "live"},
            {"id": "1".repeat(32), "kind": "bank", "title": "Private", "subject": "", "version": 1, "visibility": "unlisted", "license": "CC0-1.0", "hide_key": false, "question_count": 3, "committed_at": 1, "import_count": 0, "state": "live"},
            {"id": "2".repeat(32), "kind": "bank", "title": "Held", "subject": "", "version": 1, "visibility": "public", "license": "CC0-1.0", "hide_key": false, "question_count": 3, "committed_at": 1, "import_count": 0, "state": "held_for_review", "appeal": "available",
             "hold_class": "student_data", "positions": [1, 4], "notice": "An admin confirmed that a question this item shares with another item contains student data."},
            {"id": "3".repeat(32), "kind": "question", "title": "Deleted", "subject": "", "version": 1, "visibility": "public", "license": "CC0-1.0", "hide_key": false, "question_count": 1, "committed_at": 1, "import_count": 0, "state": "deleted", "undo_until": 1762592000000i64},
            {"id": "4".repeat(32), "kind": "bank", "title": "Taken down", "subject": "", "version": 1, "visibility": "public", "license": "CC0-1.0", "hide_key": false, "question_count": 1, "committed_at": 1, "import_count": 0, "state": "taken_down", "counter_notice": "available"},
            {"id": "5".repeat(32), "kind": "bank", "title": "Privacy", "subject": "", "version": 1, "visibility": "public", "license": "CC0-1.0", "hide_key": false, "question_count": 1, "committed_at": null, "import_count": 0, "state": "removed_privacy", "appeal": "available", "appeal_until": 1762592000000i64, "notice": "An admin confirmed this item contained student data and removed it."},
            {"id": "6".repeat(32), "kind": "bank", "title": "Admin", "subject": "", "version": 1, "visibility": "public", "license": "CC0-1.0", "hide_key": false, "question_count": 1, "committed_at": 1, "import_count": 0, "state": "removed", "removed_reason": "admin", "appeal": "available"},
            {"id": "7".repeat(32), "kind": "bank", "title": "Image lost", "subject": "", "version": 1, "visibility": "public", "license": "CC0-1.0", "hide_key": false, "question_count": 1, "committed_at": 1, "import_count": 0, "state": "needs_reupload"},
            {"id": "8".repeat(32), "kind": "bank", "title": "New state", "subject": "", "version": 1, "visibility": "public", "license": "CC0-1.0", "hide_key": false, "question_count": 1, "committed_at": 1, "import_count": 0, "state": "something_new"}
        ],
        "storage": {"used": 1234, "limit": 209715200}
    });
    let srv = serve(move |_, _, out| ok_json(out, "200 OK", mine.clone()));
    let m = run(&srv, |p| Box::pin(async move { p.mine().await })).unwrap();
    assert_eq!(srv.seen()[0].line, "GET /items/mine HTTP/1.1");
    assert_eq!(m.storage, Storage { used: 1234, limit: 209715200 });
    let st: Vec<ItemState> = m.items.iter().map(|i| i.state).collect();
    assert_eq!(
        st,
        [ItemState::Live, ItemState::Live, ItemState::HeldForReview, ItemState::Deleted, ItemState::TakenDown, ItemState::RemovedPrivacy, ItemState::Removed, ItemState::NeedsReupload, ItemState::Other]
    );
    assert!(!m.items[0].is_hidden() && m.items[2].is_hidden());
    assert!(m.items[1].is_unlisted() && !m.items[0].is_unlisted());
    assert_eq!(m.items[0].import_count, 4);
    assert_eq!(m.items[2].hold_class.as_deref(), Some("student_data"));
    assert_eq!(m.items[2].positions, vec![1, 4]);
    assert_eq!(m.items[2].appeal.as_deref(), Some("available"));
    assert_eq!(m.items[3].undo_until, Some(1762592000000));
    assert_eq!(m.items[4].counter_notice.as_deref(), Some("available"));
    assert_eq!(m.items[5].appeal_until, Some(1762592000000));
    assert_eq!(m.items[5].committed_at, None);
    assert_eq!(m.items[6].removed_reason.as_deref(), Some("admin"));
}

#[test]
fn author_actions_are_patch_delete_and_post() {
    let srv = serve(|_, s, out| match s.method() {
        "PATCH" => ok_json(out, "200 OK", json!({"ok": true, "visibility": "unlisted"})),
        "DELETE" => ok_json(out, "200 OK", json!({"ok": true, "undo_until": 99})),
        _ => ok_json(out, "200 OK", json!({"ok": true})),
    });
    let s = TestSession::new(&srv);
    let pool = Pool::new(&s);
    assert_eq!(block_on(pool.set_visibility(ID, Visibility::Unlisted)).unwrap(), Visibility::Unlisted);
    assert_eq!(block_on(pool.delete_item(ID)).unwrap(), Some(99));
    block_on(pool.undelete_item(ID)).unwrap();
    let seen = srv.seen();
    assert_eq!(seen[0].line, format!("PATCH /items/{ID} HTTP/1.1"));
    assert_eq!(seen[0].json(), json!({"visibility": "unlisted"}));
    assert_eq!(seen[1].line, format!("DELETE /items/{ID} HTTP/1.1"));
    assert_eq!(seen[1].header("Content-Length"), Some("0"));
    assert_eq!(seen[2].line, format!("POST /items/{ID}/undelete HTTP/1.1"));
}

#[test]
fn making_a_circle_only_item_public_is_refused_by_the_server() {
    let srv = canned("422 Unprocessable Entity", &[], json!({"error": "license_not_public"}));
    let e = run(&srv, |p| Box::pin(async move { p.set_visibility(ID, Visibility::Public).await })).unwrap_err();
    assert_eq!(e.kind(), Some(&RefusalKind::LicenseNotPublic));
}

#[test]
fn prefs_round_trip() {
    let srv = serve(|_, s, out| {
        if s.method() == "GET" {
            ok_json(out, "200 OK", json!({"keep_on_delete": null, "default": "keep_imported_delete_rest"}));
        } else {
            ok_json(out, "200 OK", json!({"keep_on_delete": true}));
        }
    });
    let s = TestSession::new(&srv);
    let pool = Pool::new(&s);
    assert_eq!(block_on(pool.get_prefs()).unwrap(), Prefs { keep_on_delete: None });
    block_on(pool.set_prefs(true)).unwrap();
    assert_eq!(srv.seen()[1].line, "PUT /items/prefs HTTP/1.1");
    assert_eq!(srv.seen()[1].json(), json!({"keep_on_delete": true}));
}

// ---- reports, appeals, counter-notices ---------------------------------------------------------------------

#[test]
fn reports_use_the_workers_reasons_and_a_key_leak_goes_out_as_key_leak() {
    assert_eq!(ReportReason::ALL.map(ReportReason::as_str), ["wrong_answer", "copyright", "student_data", "inappropriate", "other", "key_leak"]);
    let srv = serve(|_, _, out| ok_json(out, "202 Accepted", json!({"ok": true, "message": "Thanks, the admin will review it."})));
    let s = TestSession::new(&srv);
    let pool = Pool::new(&s);
    let ack = block_on(pool.report(ID, ReportReason::StudentData, Some("  names in question 3  "))).unwrap();
    assert!(ack.ok);
    assert_eq!(ack.message.as_deref(), Some("Thanks, the admin will review it."));
    block_on(pool.report(ID, ReportReason::Copyright, None)).unwrap();
    block_on(pool.report(ID, ReportReason::KeyLeak, Some("question 2 has a star"))).unwrap();
    block_on(pool.report(ID, ReportReason::KeyLeak, None)).unwrap();
    block_on(pool.report(ID, ReportReason::Other, Some(&"x".repeat(5000)))).unwrap();
    let seen = srv.seen();
    assert_eq!(seen[0].line, format!("POST /items/{ID}/report HTTP/1.1"));
    assert_eq!(seen[0].json(), json!({"reason": "student_data", "note": "names in question 3"}));
    assert_eq!(seen[1].json(), json!({"reason": "copyright"}));
    assert_eq!(seen[2].json(), json!({"reason": "key_leak", "note": "question 2 has a star"}));
    assert_eq!(seen[3].json(), json!({"reason": "key_leak"}));
    assert_eq!(seen[4].json()["note"].as_str().unwrap().chars().count(), 1000);
}

#[test]
fn reporting_your_own_item_and_the_daily_cap_are_refusals() {
    let srv = canned("400 Bad Request", &[], json!({"error": "own_item"}));
    let e = run(&srv, |p| Box::pin(async move { p.report(ID, ReportReason::Other, None).await })).unwrap_err();
    assert_eq!(e.kind(), Some(&RefusalKind::OwnItem));
    let srv = canned("429 Too Many Requests", &[("Retry-After", "7200")], json!({"error": "daily_limit", "metric": "reports"}));
    let e = run(&srv, |p| Box::pin(async move { p.report(ID, ReportReason::Other, None).await })).unwrap_err();
    assert_eq!(e.kind(), Some(&RefusalKind::DailyLimit { metric: Some("reports".into()) }));
    assert_eq!(srv.seen().len(), 1);
}

#[test]
fn a_report_is_not_repeated_after_a_server_error_but_is_after_busy() {
    let srv = serve(|i, _, out| match i {
        0 => reply(out, "503 Service Unavailable", &[("Content-Type", "application/json"), ("Retry-After", "1")], br#"{"error":"busy"}"#),
        _ => reply(out, "500 Internal Server Error", &[("Content-Type", "application/json")], br#"{"error":"server_error"}"#),
    });
    let e = run(&srv, |p| Box::pin(async move { p.report(ID, ReportReason::Other, None).await })).unwrap_err();
    assert_eq!(e.status(), 500);
    assert_eq!(srv.seen().len(), 2);
}

#[test]
fn appeals_and_counter_notices() {
    let srv = serve(|_, s, out| {
        if s.target().ends_with("/counter-notice") {
            if s.json()["perjury_stmt"] == json!(false) {
                ok_json(out, "400 Bad Request", json!({"error": "counter_notice_incomplete", "missing": ["perjury_stmt"]}));
            } else {
                ok_json(out, "202 Accepted", json!({"ok": true, "restore_not_before": 1000, "restore_by": 2000}));
            }
        } else if s.json()["note"] == "again" {
            ok_json(out, "409 Conflict", json!({"error": "decided", "message": "The admin has already decided on this. It cannot be appealed again."}));
        } else {
            ok_json(out, "202 Accepted", json!({"ok": true, "message": "The admin will review your item."}));
        }
    });
    let s = TestSession::new(&srv);
    let pool = Pool::new(&s);
    assert!(block_on(pool.appeal(ID, " I have the rights ")).unwrap().ok);
    assert_eq!(srv.seen()[0].line, format!("POST /items/{ID}/appeal HTTP/1.1"));
    assert_eq!(srv.seen()[0].json(), json!({"note": "I have the rights"}));
    let e = block_on(pool.appeal(ID, "again")).unwrap_err();
    assert!(matches!(e.kind(), Some(RefusalKind::ModerationState { message: Some(m) }) if m.contains("already decided")));
    assert!(matches!(block_on(pool.appeal(ID, "   ")), Err(PoolError::Local(LocalProblem::BadField(_)))));

    let mut n = CounterNotice {
        signature: "Ms Frizzle".into(),
        material_desc: "My own questions".into(),
        name: "Valerie Frizzle".into(),
        address: "1 Bus Lane".into(),
        phone: "555-0100".into(),
        perjury_stmt: false,
        consent_jurisdiction: true,
        accept_service: true,
    };
    let e = block_on(pool.counter_notice(ID, &n)).unwrap_err();
    assert_eq!(e.kind(), Some(&RefusalKind::CounterNoticeIncomplete { missing: vec!["perjury_stmt".into()] }));
    n.perjury_stmt = true;
    let a = block_on(pool.counter_notice(ID, &n)).unwrap();
    assert_eq!(a, CounterNoticeAck { restore_not_before: 1000, restore_by: 2000 });
}

// ---- the student view --------------------------------------------------------------------------------------

fn fixture(dir: &str, name: &str) -> Vec<u8> {
    std::fs::read(format!("{}/server/share-core/fixtures/{dir}/{name}", env!("CARGO_MANIFEST_DIR"))).unwrap()
}

#[test]
fn the_student_view_of_a_stripped_bank_shows_what_students_get_and_no_key() {
    let v = StudentView::from_payload(&fixture("stripped", "bank_all_kinds.json"), Served::Stripped).unwrap();
    assert!(v.is_clean(), "{:?}", v.warnings);
    assert_eq!(v.name.as_deref(), Some("Unit 3: Cells and Space"));
    assert_eq!(v.description.as_deref(), Some("Mixed review.\nAll kinds."));
    assert_eq!(v.questions.len(), 8);
    assert_eq!(v.questions[0].number, 1);
    assert_eq!(v.questions[0].topics, vec!["Planets", "Solar system"]);
    assert_eq!(v.questions[0].points, Some(2.0));
    let StudentBody::Mc { choices } = &v.questions[0].body else { panic!("{:?}", v.questions[0].body) };
    assert_eq!(choices.len(), 4);
    assert!(choices.iter().all(|c| c.correct.is_none()), "no key in the student form");
    assert!(choices[3].locked);
    // True/false shows no answer, matching shows the left items and the word bank, blank counts blanks
    assert_eq!(v.questions[2].body, StudentBody::TrueFalse { answer: None });
    let StudentBody::Matching { left, word_bank, pairs } = &v.questions[3].body else { panic!() };
    assert_eq!(left.len(), 3);
    assert!(word_bank.contains(&"Builds proteins".to_string()));
    assert!(pairs.is_empty());
    assert_eq!(v.questions[4].body, StudentBody::Blank { blanks: 2, accepted: vec![] });
    assert_eq!(v.questions[5].body, StudentBody::Short { sample: None });
    let StudentBody::Multipart { parts } = &v.questions[6].body else { panic!() };
    assert_eq!(parts.len(), 3);
    assert_eq!(parts[1].body, StudentBody::TrueFalse { answer: None });
    // nothing the key could be read from anywhere in the Debug text of the view
    assert!(!format!("{v:?}").contains("correct: Some"));
}

/// A payload with its key (what the author has locally).
fn keyed() -> Vec<u8> {
    json!({"schema_version": 2, "bank": {"name": "Keyed", "questions": [
        {"id": "q1", "stem": "Closest planet?", "explanation": "Mercury orbits closest.", "kind": {"Mc": {"answers": [
            {"text": "Mars", "correct": false, "locked": false}, {"text": "Mercury", "correct": true, "locked": false}]}}},
        {"id": "q2", "stem": "Water boils at 100 C.", "kind": {"Tf": {"correct": true}}},
        {"id": "q3", "stem": "Match.", "kind": {"Matching": {"pairs": [{"left": "France", "right": "Paris"}, {"left": "Japan", "right": "Tokyo"}], "distractors": ["Rome"]}}},
        {"id": "q4", "stem": "The ___ is big.", "kind": {"Blank": {"accept": [["whale", "Whale"]]}}},
        {"id": "q5", "stem": "Why?", "kind": {"Short": {"sample": "Because."}}}
    ]}}).to_string().into_bytes()
}

#[test]
fn a_stripped_view_of_a_payload_that_still_has_its_key_warns() {
    let v = StudentView::from_payload(&keyed(), Served::Stripped).unwrap();
    assert!(!v.is_clean());
    let whats: HashSet<&str> = v.warnings.iter().filter_map(|w| if let ViewWarning::KeyDataPresent { what, .. } = w { Some(*what) } else { None }).collect();
    for want in ["correct choice", "matching pair", "accepted answer", "sample answer", "explanation"] {
        assert!(whats.contains(want), "{whats:?}");
    }
    assert!(v.warnings.contains(&ViewWarning::NotMarkedKeyless { question: 0 }));
    // the view itself still carries no key
    for q in &v.questions {
        assert!(q.explanation.is_none());
        match &q.body {
            StudentBody::Mc { choices } => assert!(choices.iter().all(|c| c.correct.is_none())),
            StudentBody::TrueFalse { answer } => assert!(answer.is_none()),
            StudentBody::Matching { pairs, .. } => assert!(pairs.is_empty()),
            StudentBody::Blank { accepted, .. } => assert!(accepted.is_empty()),
            StudentBody::Short { sample } => assert!(sample.is_none()),
            StudentBody::Multipart { .. } => {}
        }
    }
}

#[test]
fn a_full_view_for_a_publish_that_shows_its_key() {
    let v = StudentView::from_payload(&keyed(), Served::Full).unwrap();
    assert!(v.is_clean());
    assert_eq!(v.questions.len(), 5);
    let StudentBody::Mc { choices } = &v.questions[0].body else { panic!() };
    assert_eq!(choices.iter().map(|c| c.correct).collect::<Vec<_>>(), vec![Some(false), Some(true)]);
    assert_eq!(v.questions[0].explanation.as_deref(), Some("Mercury orbits closest."));
    assert_eq!(v.questions[1].body, StudentBody::TrueFalse { answer: Some(true) });
    let StudentBody::Matching { pairs, .. } = &v.questions[2].body else { panic!() };
    assert_eq!(pairs[1], ("Japan".to_string(), "Tokyo".to_string()));
    assert_eq!(v.questions[3].body, StudentBody::Blank { blanks: 1, accepted: vec![vec!["whale".into(), "Whale".into()]] });
    assert_eq!(v.questions[4].body, StudentBody::Short { sample: Some("Because.".into()) });
}

#[test]
fn a_stripped_question_not_marked_keyless_is_flagged_and_junk_is_refused() {
    let v = StudentView::from_payload(br#"{"schema_version":2,"question":{"id":"q","stem":"s","kind":{"Tf":{"correct":false}}}}"#, Served::Stripped).unwrap();
    assert_eq!(v.warnings, vec![ViewWarning::NotMarkedKeyless { question: 0 }]);
    assert!(StudentView::from_payload(b"not json", Served::Stripped).is_err());
    assert!(StudentView::from_payload(br#"{"schema_version":2}"#, Served::Stripped).is_err());
}

// ---- splitting and finding things ---------------------------------------------------------------------------

#[test]
fn split_item_cuts_at_the_payload_key_only() {
    let body = streamed(&json!({"item": {"title": "x \"payload\": y"}, "questions": []}), r#"{"a":1}"#, None);
    let (head, payload) = split_item(&body).unwrap();
    assert_eq!(head["item"]["title"], "x \"payload\": y");
    assert_eq!(payload, br#"{"a":1}"#);
    assert!(split_item(b"{\"item\":{}}").is_err());
    assert!(split_item(b"{\"item\":{},\"payload\":{\"a\":1").is_err());
}

#[test]
fn images_are_read_from_questions_only() {
    let a = format!("media/{}.webp", "a".repeat(64));
    let b = format!("media/{}.png", "b".repeat(64));
    let p = json!({"schema_version": 2, "bank": {"questions": [{"image": null}, {"image": {"file": a}}, {"stem": "x"}, {"image": {"file": b}}]}});
    let u = images_in_payload(p.to_string().as_bytes()).unwrap();
    assert_eq!(u, vec![ImageUse { position: 1, file: a, sha256: "a".repeat(64) }, ImageUse { position: 3, file: b, sha256: "b".repeat(64) }]);
    let one = json!({"schema_version": 2, "question": {"image": {"file": format!("media/{}.gif", "c".repeat(64))}}});
    assert_eq!(images_in_payload(one.to_string().as_bytes()).unwrap()[0].position, 0);
    assert!(images_in_payload(b"{}").is_err());
}

#[test]
fn licences_round_trip_their_wire_names() {
    for l in License::ALL {
        assert_eq!(License::parse(l.as_str()), Some(l));
        assert_eq!(serde_json::to_value(l).unwrap(), json!(l.as_str()));
    }
    assert_eq!(License::parse("MIT"), None);
}

// ---- through the real Client -------------------------------------------------------------------------------------

#[test]
fn the_real_client_adds_its_bearer_and_hands_back_the_whole_error() {
    use super::client::Client;
    let srv = serve(|_, s, out| {
        if s.target() == "/auth/login" {
            ok_json(out, "200 OK", json!({"access_token": "at-1", "refresh_token": "rt-1", "expires_in": 900, "user": {"id": "0f0f0f0f-0000-4000-8000-000000000001"}}));
        } else if s.target().starts_with("/items/mine") {
            ok_json(out, "200 OK", json!({"items": [], "storage": {"used": 5, "limit": 10}}));
        } else {
            ok_json(out, "422 Unprocessable Entity", json!({"error": "invalid_payload", "errors": [{"code": "too_long", "path": "bank.name", "limit": 200}]}));
        }
    });
    let client = Client::new(&srv.base(), Mode::Native, Box::new(NativeTransport::default()), Box::new(MemoryVault::default()));
    block_on(client.sign_in("t@example.org", &Secret::new("correct horse battery"))).unwrap();
    let pool = Pool::new(&client).with_retry(RetryPolicy::none());
    let m = block_on(pool.mine()).unwrap();
    assert_eq!(m.storage.used, 5);
    let e = block_on(pool.publish(&PublishRequest::new_item(bank(&[]), opts()), &HashMap::new(), BlobOrder::First, &mut no_progress())).unwrap_err();
    let Some(RefusalKind::InvalidPayload(p)) = e.kind() else { panic!("{e:?}") };
    assert_eq!(p.issues[0].limit, Some(200));
    let seen = srv.seen();
    let mine = seen.iter().find(|s| s.target() == "/items/mine").unwrap();
    assert_eq!(mine.header("Authorization"), Some("Bearer at-1"));
    assert_eq!(mine.header("X-Client"), Some("native"));
    let publish = seen.iter().find(|s| s.line.starts_with("POST /items ")).unwrap();
    assert_eq!(publish.header("Authorization"), Some("Bearer at-1"));
    assert_eq!(publish.header("Content-Type"), Some("application/vnd.vershion.publish"));
}

// ---- the dry run and the terms affirmation (publish flow) -------------------------------------------

#[test]
fn every_publish_carries_the_terms_affirmation_the_worker_checks() {
    let srv = serve(|_, _, out| ok_json(out, "201 Created", publish_ok()));
    let req = PublishRequest::new_item(bank(&[]), opts());
    publish(&srv, &req, &HashMap::new(), BlobOrder::First).unwrap();
    let s = &srv.seen()[0];
    let nl = s.body.iter().position(|b| *b == b'\n').unwrap();
    let meta: Value = serde_json::from_slice(&s.body[..nl]).unwrap();
    assert_eq!(meta["attest"], json!({"own_work": true, "terms_version": POOL_TERMS_VERSION}));
    assert!(ATTEST_STATEMENT.contains("my own original work"));
}

#[test]
fn a_preview_posts_to_the_preview_route_and_reads_the_served_form() {
    let srv = serve(|_, _, out| {
        ok_json(
            out,
            "200 OK",
            json!({"ok": true, "dry_run": true, "hide_key": true,
                   "student_view": {"schema_version": 2, "bank": {"name": "Unit 1", "questions": []}},
                   "notes": [{"reason": "x", "message": "Dropped"}],
                   "labels_removed": [{"question": 1}], "images_removed": [0], "figures_removed": [2],
                   "order_reasons": [{"question": 3, "reason": "few_unlocked"}],
                   "license_check": {"status": "ok"}, "chains": []}),
        )
    });
    let s = TestSession::new(&srv);
    let req = PublishRequest::new_item(bank(&[]), opts());
    let r = block_on(Pool::new(&s).with_retry(RetryPolicy::instant(1)).preview(&req, &HashMap::new(), BlobOrder::First, &mut no_progress())).unwrap();
    assert!(r.ok && r.dry_run && r.hide_key);
    assert_eq!(r.images_removed, vec![0]);
    assert_eq!(r.figures_removed, vec![2]);
    assert_eq!(r.labels_removed, vec![Pos::question(1)]);
    assert_eq!(r.order_reasons[0].reason, "few_unlocked");
    assert_eq!(r.student_view["bank"]["name"], "Unit 1");
    assert_eq!(srv.seen()[0].line, "POST /items/preview HTTP/1.1");
    // a republish previews over the item
    let req = PublishRequest::republish(ID, &"b".repeat(32), bank(&[]), opts());
    let srv2 = serve(|_, _, out| ok_json(out, "200 OK", json!({"ok": true, "dry_run": true})));
    let s2 = TestSession::new(&srv2);
    block_on(Pool::new(&s2).with_retry(RetryPolicy::instant(1)).preview(&req, &HashMap::new(), BlobOrder::OnDemand, &mut no_progress())).unwrap();
    assert_eq!(srv2.seen()[0].line, format!("POST /items/{ID}/preview HTTP/1.1"));
}

#[test]
fn an_attestation_refusal_is_its_own_kind() {
    let (e, _s) = refusal_of("422 Unprocessable Entity", &[], json!({"error": "attestation_required", "terms_version": "2026-10-22", "statement": "x"}));
    assert_eq!(e.kind(), Some(&RefusalKind::AttestationRequired));
    assert!(e.message().contains("own work"));
}

#[test]
fn every_worker_error_code_has_a_plain_message_and_the_default_still_says_something() {
    let codes = [
        "unauthorized", "reauth_required", "invalid_json", "body_aborted", "length_mismatch", "length_required", "unsupported_media_type", "body_timeout",
        "pool_key_mismatch", "pool_migrating", "too_many_units", "author_records_exceeded", "imports_quota", "reads_quota", "evidence_quota_exceeded",
        "origin_not_allowed", "client_ip_required", "native_client_with_origin", "notice_not_found", "method_not_allowed", "server_error",
        "preview_required", "confirm_large_required", "selection_required", "hold_has_own_route", "action_closed", "rightful_item_invalid",
    ];
    for c in codes {
        let m = other_message(400, c, "later".into());
        assert!(!m.is_empty() && !m.contains(c) && !m.contains("refused the request"), "{c}: {m}");
    }
    assert_eq!(other_message(418, "teapot", String::new()), "The pool refused the request (teapot).");
    assert!(other_message(503, "", String::new()).contains("problem of its own"));
    assert!(other_message(429, "", "in about a minute".into()).contains("Try again in about a minute"));
    for metric in ["exports", "previews", "preview_units", "publish_refusals", "item_miss", "reports", "appeals", "key_leak_reports"] {
        let r = Refusal {
            detail: super::api::ErrorDetail { status: 429, code: "daily_limit".into(), retry_after: Some(60), ..super::api::ErrorDetail::from_response(&Response::new(429, "{}")) },
            kind: RefusalKind::DailyLimit { metric: Some(metric.into()) },
        };
        assert!(r.message().contains("limit") || r.message().contains("once per item"), "{metric}");
    }
}
