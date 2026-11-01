//! Tests for `routes.rs`: the pool over real HTTP requests (a fake sign-in by header), through the gate, the body readers and the cores.
use super::*;
use crate::db;
use axum::http::Request as HttpRequest;
use http_body_util::BodyExt;
use std::path::Path;
use tower::ServiceExt;

const KEY: &str = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";
const ROOT: &str = "7f757fb737085dcaaad0727deb89a14c18141681dc56be0a215a592c715f8e3b";
const NOW: i64 = 5 * 86_400_000;

/// `x-test-user: <id>:<trust>`
struct HeaderAuth;
impl PoolAuth for HeaderAuth {
    fn caller(&self, h: &HeaderMap) -> Option<Caller> {
        let v = h.get("x-test-user")?.to_str().ok()?;
        let (u, t) = v.split_once(':')?;
        Some(Caller { user_id: u.into(), trust: t.into() })
    }
}

struct W {
    app: Router,
    st: Arc<PoolState>,
    notices: Arc<Mutex<Vec<PoolNotice>>>,
    _d: tempfile::TempDir,
}
fn world() -> W {
    let dir = tempfile::tempdir().unwrap();
    let mut c = db::open(&dir.path().join("t.db")).unwrap();
    db::migrate(&mut c, &Path::new(env!("CARGO_MANIFEST_DIR")).join("../worker/migrations")).unwrap();
    c.execute("INSERT INTO users (id, email, display_name, trust, created_at) VALUES ('t', 't@x.org', 'T', 'teacher', 1)", []).unwrap();
    c.execute("INSERT INTO pool_invite_redemptions (user_id, invite_id, at) VALUES ('t', 'i', 1)", []).unwrap();
    let notices = Arc::new(Mutex::new(vec![]));
    let sink = notices.clone();
    let st = Arc::new(PoolState {
        conn: Mutex::new(c),
        store: BlobStore::new(dir.path().join("blobs")).unwrap(),
        core: Core::new(Some(KEY.into())),
        fp: Fingerprinter::from_root_hex(ROOT).unwrap(),
        keys: PoolKeys::from_secret("test-secret"),
        limit_key: [3u8; 32],
        ip_prefix: 64,
        allowed_origins: vec!["https://app.example".into()],
        auth: Box::new(HeaderAuth),
        clock: Arc::new(|| NOW),
        heavy: HeavySlots::default(),
        heavy_wait: Duration::from_secs(1),
        body_deadline: Duration::from_secs(2),
        notify: Arc::new(move |n| sink.lock().unwrap().extend(n)),
    });
    W { app: router(st.clone()), st, notices, _d: dir }
}
fn req(method: &str, uri: &str, user: Option<&str>, ct: Option<&str>, body: Vec<u8>) -> HttpRequest<Body> {
    let mut b = HttpRequest::builder().method(method).uri(uri).header("x-forwarded-for", "203.0.113.5");
    if let Some(u) = user {
        b = b.header("x-test-user", u);
    }
    if let Some(c) = ct {
        b = b.header("content-type", c);
    }
    b.header("content-length", body.len().to_string()).body(Body::from(body)).unwrap()
}
async fn send(w: &W, r: HttpRequest<Body>) -> (u16, Value, HeaderMap) {
    let res = w.app.clone().oneshot(r).await.unwrap();
    let (status, headers) = (res.status().as_u16(), res.headers().clone());
    let bytes = res.into_body().collect().await.unwrap().to_bytes();
    (status, serde_json::from_slice(&bytes).unwrap_or(Value::Null), headers)
}
fn publish_body(publish_id: &str) -> Vec<u8> {
    let words = ["apple", "river", "stone", "cloud", "ember", "maple", "orbit", "quartz"];
    let answers: Vec<Value> = words.iter().enumerate().map(|(i, t)| json!({ "text": t, "correct": i == 3, "locked": false })).collect();
    let q = json!({ "id": "q", "stem": "What is the capital of France?", "kind": { "Mc": { "answers": answers } }, "points": null, "difficulty": null, "topics": [], "image": null, "explanation": "because", "figure": null, "created": 1, "modified": 1 });
    let meta = json!({
        "publish_id": publish_id, "hide_key": false, "license": "CC-BY-4.0", "visibility": "unlisted",
        "attestation": { "accepted": true, "text_rev": "2026-10-19" },
        "attest": { "own_work": true, "terms_version": crate::pool::license::POOL_TERMS_VERSION },
        "meta": { "title": "Q", "subject": "S" }
    });
    let payload = json!({ "schema_version": 2, "bank": { "id": "local", "name": "B", "questions": [q], "created": 1, "modified": 1 } });
    let mut out = meta.to_string().into_bytes();
    out.push(b'\n');
    out.extend(payload.to_string().into_bytes());
    out
}
const T: Option<&str> = Some("t:teacher");

#[tokio::test]
async fn unknown_paths_are_404_and_unsigned_requests_are_401() {
    let w = world();
    assert_eq!(send(&w, req("GET", "/items/a/b/c", T, None, vec![])).await.0, 404);
    let (s, b, h) = send(&w, req("GET", "/items", None, None, vec![])).await;
    assert_eq!((s, b["error"].clone()), (401, json!("unauthorized")));
    assert!(h.get("www-authenticate").is_some());
}

#[tokio::test]
async fn a_wrong_method_is_a_405_with_allow() {
    let w = world();
    let (s, b, h) = send(&w, req("DELETE", "/items", T, None, vec![])).await;
    assert_eq!((s, b["error"].clone()), (405, json!("method_not_allowed")));
    assert_eq!(h.get("allow").unwrap(), "GET, POST");
}

#[tokio::test]
async fn a_request_without_a_forwarded_address_is_a_403() {
    let w = world();
    let r = HttpRequest::builder().method("GET").uri("/items").header("x-test-user", "t:teacher").body(Body::empty()).unwrap();
    let (s, b, _) = send(&w, r).await;
    assert_eq!((s, b["error"].clone()), (403, json!("client_ip_required")));
}

#[tokio::test]
async fn prefs_round_trip_and_a_bad_body_is_refused() {
    let w = world();
    let (s, b, _) = send(&w, req("GET", "/items/prefs", T, None, vec![])).await;
    assert_eq!((s, b["keep_on_delete"].clone()), (200, Value::Null));
    let (s, b, _) = send(&w, req("PUT", "/items/prefs", T, Some("application/json"), br#"{"keep_on_delete":true}"#.to_vec())).await;
    assert_eq!((s, b["keep_on_delete"].clone()), (200, json!(true)));
    let (_, b, _) = send(&w, req("GET", "/items/prefs", T, None, vec![])).await;
    assert_eq!(b["keep_on_delete"], json!(true));
    let (s, b, _) = send(&w, req("PUT", "/items/prefs", T, Some("application/json"), br#"{"keep_on_delete":"yes"}"#.to_vec())).await;
    assert_eq!((s, b["error"].clone()), (400, json!("invalid_request")));
    let (s, b, _) = send(&w, req("PUT", "/items/prefs", T, Some("text/plain"), b"{}".to_vec())).await;
    assert_eq!((s, b["error"].clone()), (415, json!("unsupported_media_type")));
}

#[tokio::test]
async fn a_body_needs_a_length_and_a_small_enough_one() {
    let w = world();
    let r = HttpRequest::builder().method("PUT").uri("/items/prefs").header("x-forwarded-for", "1.1.1.1").header("x-test-user", "t:teacher").header("content-type", "application/json").body(Body::empty()).unwrap();
    let (s, b, _) = send(&w, r).await;
    assert_eq!((s, b["error"].clone()), (411, json!("length_required")));
    let big = vec![b' '; 70 * 1024];
    let (s, b, _) = send(&w, req("PUT", "/items/prefs", T, Some("application/json"), big)).await;
    assert_eq!((s, b["error"].clone()), (413, json!("body_too_large")));
}

#[tokio::test]
async fn publish_list_mine_and_delete_work_over_http() {
    let w = world();
    let ct = Some("application/vnd.vershion.publish");
    let (s, b, _) = send(&w, req("POST", "/items", T, ct, publish_body("AAAAAAAAAAAAAAAAAAAAAA"))).await;
    assert_eq!(s, 201, "{b}");
    let item = b["item_id"].as_str().unwrap().to_string();
    let (s, b, _) = send(&w, req("GET", "/items?scope=mine", T, None, vec![])).await;
    assert_eq!(s, 200);
    assert_eq!(b["items"].as_array().map(Vec::len), Some(1), "{b}");
    let (s, b, _) = send(&w, req("GET", "/items/mine", T, None, vec![])).await;
    assert_eq!((s, b["items"].as_array().map(Vec::len)), (200, Some(1)));
    // A preview stores nothing; a replay answers the first result.
    let (s, b, _) = send(&w, req("POST", "/items/preview", T, ct, publish_body("BBBBBBBBBBBBBBBBBBBBBB"))).await;
    assert_eq!((s, b["dry_run"].clone()), (200, json!(true)));
    let (s, b, _) = send(&w, req("POST", "/items", T, ct, publish_body("AAAAAAAAAAAAAAAAAAAAAA"))).await;
    assert_eq!((s, b["replayed"].clone()), (200, json!(true)));
    let (s, _, _) = send(&w, req("DELETE", &format!("/items/{item}"), T, None, vec![])).await;
    assert_eq!(s, 200);
    let n: i64 = w.st.conn.lock().unwrap().query_row("SELECT COUNT(*) FROM items WHERE removed_at IS NOT NULL", [], |r| r.get(0)).unwrap();
    assert_eq!(n, 1);
    let _ = &w.notices;
}

#[tokio::test]
async fn routes_not_wired_yet_say_so() {
    let w = world();
    let (s, b, _) = send(&w, req("GET", "/circles", T, None, vec![])).await;
    assert_eq!((s, b["error"].clone()), (501, json!("not_ported")));
}
