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
        Some(Caller { user_id: u.into(), trust: t.into(), family: None })
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
        opens: OpenReads::new(),
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
const OID: &str = "00000000-0000-4000-8000-000000000001";
const OIDT: &str = "00000000-0000-4000-8000-000000000001:teacher";

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

fn add_user(w: &W, id: &str) {
    let c = w.st.conn.lock().unwrap();
    c.execute("INSERT INTO users (id, email, display_name, trust, created_at) VALUES (?1, ?2, 'O', 'teacher', 1)", rusqlite::params![id, format!("{id}@x.org")]).unwrap();
    c.execute("INSERT INTO pool_invite_redemptions (user_id, invite_id, at) VALUES (?1, 'i', 1)", [id]).unwrap();
}
async fn published(w: &W, user: &str, publish_id: &str, visibility: &str) -> String {
    let mut body = publish_body(publish_id);
    if visibility == "public" {
        let nl = body.iter().position(|b| *b == b'\n').unwrap();
        let mut meta: Value = serde_json::from_slice(&body[..nl]).unwrap();
        meta["visibility"] = json!("public");
        let mut out = meta.to_string().into_bytes();
        out.extend_from_slice(&body[nl..]);
        body = out;
    }
    let (s, b, _) = send(w, req("POST", "/items", Some(user), Some("application/vnd.vershion.publish"), body)).await;
    assert_eq!(s, 201, "{b}");
    b["item_id"].as_str().unwrap().to_string()
}

#[tokio::test]
async fn an_item_is_read_with_an_etag_head_and_not_modified() {
    let w = world();
    let item = published(&w, "t:teacher", "AAAAAAAAAAAAAAAAAAAAAA", "unlisted").await;
    let uri = format!("/items/{item}");
    let res = w.app.clone().oneshot(req("GET", &uri, T, None, vec![])).await.unwrap();
    assert_eq!(res.status(), 200);
    let etag = res.headers().get("etag").unwrap().to_str().unwrap().to_string();
    let bytes = res.into_body().collect().await.unwrap().to_bytes();
    let v: Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(v["item"]["id"], json!(item));
    assert!(v["payload"]["bank"].is_object());
    // HEAD sends the headers a GET would, including the exact length.
    let head = w.app.clone().oneshot(req("HEAD", &uri, T, None, vec![])).await.unwrap();
    assert_eq!(head.status(), 200);
    assert_eq!(head.headers().get("etag").unwrap().to_str().unwrap(), etag);
    assert_eq!(head.headers().get("content-length").unwrap().to_str().unwrap(), bytes.len().to_string());
    // A matching If-None-Match (weak form too) is a 304 with no body.
    let mut r = req("GET", &uri, T, None, vec![]);
    r.headers_mut().insert("if-none-match", format!("W/{etag}").parse().unwrap());
    assert_eq!(w.app.clone().oneshot(r).await.unwrap().status(), 304);
    // Someone else's unlisted item is a 404.
    add_user(&w, "o");
    let (s, b, _) = send(&w, req("GET", &uri, Some("o:teacher"), None, vec![])).await;
    assert_eq!((s, b["error"].clone()), (404, json!("not_found")));
    let (s, _, _) = send(&w, req("GET", "/items/nothex", Some("o:teacher"), None, vec![])).await;
    assert_eq!(s, 404);
}

#[tokio::test]
async fn another_teacher_imports_a_public_item_and_the_import_is_recorded() {
    let w = world();
    let item = published(&w, "t:teacher", "AAAAAAAAAAAAAAAAAAAAAA", "public").await;
    add_user(&w, "o");
    let (s, b, _) = send(&w, req("POST", &format!("/items/{item}/import"), Some("o:teacher"), None, vec![])).await;
    assert_eq!(s, 200, "{b}");
    assert_eq!((b["recorded"].clone(), b["previously_imported_versions"].clone()), (json!(true), json!([])));
    assert!(b["payload"].is_object());
    let n: i64 = w.st.conn.lock().unwrap().query_row("SELECT COUNT(*) FROM imports WHERE user_id = 'o' AND kind = 'import'", [], |r| r.get(0)).unwrap();
    assert_eq!(n, 1);
}

#[tokio::test]
async fn blob_uploads_check_the_sha_and_the_hash_before_anything_is_stored() {
    let w = world();
    let (s, b, _) = send(&w, req("PUT", "/blobs/xyz", T, Some("image/png"), vec![1, 2, 3])).await;
    assert_eq!((s, b["error"].clone(), b["field"].clone()), (400, json!("invalid_request"), json!("sha256")));
    let sha = "a".repeat(64);
    let (s, b, _) = send(&w, req("PUT", &format!("/blobs/{sha}"), T, Some("image/png"), vec![1, 2, 3])).await;
    assert_eq!((s, b["error"].clone()), (422, json!("hash_mismatch")));
    let n: i64 = w.st.conn.lock().unwrap().query_row("SELECT COUNT(*) FROM pending_blobs", [], |r| r.get(0)).unwrap();
    assert_eq!(n, 0);
}

#[tokio::test]
async fn invite_redemption_and_the_export_freshness_rule_answer_over_http() {
    let w = world();
    w.st.conn.lock().unwrap().execute("INSERT INTO users (id, email, display_name, trust, created_at) VALUES ('n', 'n@x.org', 'N', 'teacher', 1)", []).unwrap();
    // (An account that already holds a redemption is told so.)
    let (s, b, _) = send(&w, req("POST", "/me/invite", T, Some("application/json"), br#"{"code":"ABCDEFGH23456789ABCDEFGH"}"#.to_vec())).await;
    assert_eq!((s, b["already"].clone()), (200, json!(true)));
    let (s, b, _) = send(&w, req("POST", "/me/invite", Some("n:teacher"), Some("application/json"), br#"{"code":"ABCDEFGH23456789ABCDEFGH"}"#.to_vec())).await;
    assert_eq!((s, b["error"].clone()), (422, json!("invite_invalid")), "{b}");
    // The export needs a login in the last 24 hours.
    let (s, b, _) = send(&w, req("GET", "/me/export", T, None, vec![])).await;
    assert_eq!((s, b["error"].clone()), (403, json!("reauth_required")));
}

fn jreq(method: &str, uri: &str, user: &str, body: Value) -> HttpRequest<Body> {
    req(method, uri, Some(user), Some("application/json"), body.to_string().into_bytes())
}

#[tokio::test]
async fn a_circle_is_created_joined_with_a_code_and_managed_over_http() {
    let w = world();
    add_user(&w, OID);
    let (s, b, _) = send(&w, jreq("POST", "/circles", "t:teacher", json!({ "name": "Algebra team" }))).await;
    assert_eq!(s, 201, "{b}");
    let cid = b["circle"]["id"].as_str().unwrap().to_string();
    let (s, b, _) = send(&w, req("GET", "/circles", T, None, vec![])).await;
    assert_eq!((s, b["circles"].as_array().map(Vec::len)), (200, Some(1)), "{b}");
    let (s, b, _) = send(&w, req("GET", &format!("/circles/{cid}"), T, None, vec![])).await;
    assert_eq!((s, b["circle"]["role"].clone()), (200, json!("owner")), "{b}");
    // An invite code, then the other teacher previews and joins with it.
    let (s, b, _) = send(&w, jreq("POST", &format!("/circles/{cid}/invites"), "t:teacher", json!({ "role": "editor" }))).await;
    assert_eq!(s, 201, "{b}");
    let code = b["code"].as_str().unwrap().to_string();
    let (s, b, _) = send(&w, jreq("POST", "/circles/join/preview", OIDT, json!({ "code": code }))).await;
    assert_eq!((s, b["role"].clone()), (200, json!("editor")), "{b}");
    let (s, b, _) = send(&w, jreq("POST", "/circles/join", OIDT, json!({ "code": code }))).await;
    assert_eq!((s, b["ok"].clone()), (200, json!(true)), "{b}");
    let (s, b, _) = send(&w, req("GET", &format!("/circles/{cid}/activity"), T, None, vec![])).await;
    assert_eq!(s, 200, "{b}");
    // The owner changes the member's role, the member leaves, and the owner closes and reopens the circle.
    let (s, b, _) = send(&w, jreq("POST", &format!("/circles/{cid}/members/role"), "t:teacher", json!({ "user_id": OID, "role": "viewer" }))).await;
    assert_eq!(s, 200, "{b}");
    let (s, _, _) = send(&w, req("POST", &format!("/circles/{cid}/leave"), Some(OIDT), None, vec![])).await;
    assert_eq!(s, 200);
    let (s, _, _) = send(&w, req("DELETE", &format!("/circles/{cid}"), T, None, vec![])).await;
    assert_eq!(s, 200);
    let (s, _, _) = send(&w, req("POST", &format!("/circles/{cid}/reopen"), T, None, vec![])).await;
    assert_eq!(s, 200);
    // A stranger sees nothing of it.
    let (s, b, _) = send(&w, req("GET", &format!("/circles/{cid}"), Some(OIDT), None, vec![])).await;
    assert_eq!((s, b["error"].clone()), (404, json!("not_found")));
}

#[tokio::test]
async fn circle_bodies_are_checked_and_a_pending_account_may_join_but_not_create() {
    let w = world();
    let (s, b, _) = send(&w, jreq("POST", "/circles", "t:teacher", json!({ "name": "" }))).await;
    assert_eq!((s, b["error"].clone(), b["field"].clone()), (400, json!("invalid_request"), json!("name")));
    w.st.conn.lock().unwrap().execute("INSERT INTO users (id, email, display_name, trust, created_at) VALUES ('p', 'p@x.org', 'P', 'pending', 1)", []).unwrap();
    let (s, b, _) = send(&w, jreq("POST", "/circles", "p:pending", json!({ "name": "Mine" }))).await;
    assert_eq!((s, b["error"].clone()), (403, json!("account_not_approved")));
    let (s, b, _) = send(&w, jreq("POST", "/circles/join", "p:pending", json!({ "code": "C-ABCD-EFGH-2345-6789-ABCD-EFGH" }))).await;
    assert_eq!((s, b["error"].clone()), (422, json!("circle_invite_invalid")), "{b}");
    // An empty heads list is fine without a body.
    let (s, _, _) = send(&w, req("POST", "/circles/heads", Some("p:pending"), None, vec![])).await;
    assert_ne!(s, 501);
}

#[tokio::test]
async fn bank_routes_are_not_wired_yet() {
    let w = world();
    let cid = "0123456789abcdef0123456789abcdef";
    let (s, b, _) = send(&w, req("GET", &format!("/circles/{cid}/banks/{cid}"), T, None, vec![])).await;
    assert_eq!((s, b["error"].clone()), (501, json!("not_ported")));
}
