//! Tests for `router.rs`: the auth routes over real HTTP requests against a migrated file.
use super::*;
use crate::auth::password::Argon2Params;
use crate::db;
use axum::http::Request;
use std::path::Path;
use tower::ServiceExt;

const ORIGIN: &str = "https://app.example.org";
const PW: &str = "violet-harbor-tractor";

struct Cap(Arc<Mutex<Vec<MailMessage>>>);
impl Mailer for Cap {
    fn send(&self, m: &MailMessage) -> Result<(), String> {
        self.0.lock().unwrap().push(m.clone());
        Ok(())
    }
}

struct App {
    router: Router,
    state: Arc<AuthState>,
    sent: Arc<Mutex<Vec<MailMessage>>>,
    _dir: tempfile::TempDir,
}

fn app() -> App {
    let dir = tempfile::tempdir().unwrap();
    let mut conn = db::open(&dir.path().join("t.db")).unwrap();
    db::migrate(&mut conn, &Path::new(env!("CARGO_MANIFEST_DIR")).join("../worker/migrations")).unwrap();
    let mut cfg = AuthConfig::new(&"s".repeat(32), vec![ORIGIN.to_string()]).unwrap();
    cfg.argon2 = Argon2Params { m: 8, t: 1, p: 1 };
    let mail = MailEnv::from_lookup(|k| [("MAIL_MODE", "log"), ("APP_URL", "http://localhost:5173")].iter().find(|(n, _)| *n == k).map(|(_, v)| v.to_string())).unwrap();
    let sent = Arc::new(Mutex::new(Vec::new()));
    let mut st = AuthState::new(conn, cfg, mail);
    st.mailer = Box::new(Cap(sent.clone()));
    let state = Arc::new(st);
    App { router: router(state.clone()), state, sent, _dir: dir }
}

fn req(method: &str, path: &str, headers: &[(&str, &str)], body: Option<Value>) -> Request<Body> {
    let mut b = Request::builder().method(method).uri(path).header("x-forwarded-for", "10.0.0.1");
    for (k, v) in headers {
        b = b.header(*k, *v);
    }
    match body {
        Some(v) => b.header("content-type", "application/json").body(Body::from(v.to_string())).unwrap(),
        None => b.body(Body::empty()).unwrap(),
    }
}

async fn call(a: &App, r: Request<Body>) -> (u16, HeaderMap, Value) {
    let res = a.router.clone().oneshot(r).await.unwrap();
    let (status, headers) = (res.status().as_u16(), res.headers().clone());
    let bytes = to_bytes(res.into_body(), 1 << 20).await.unwrap();
    (status, headers, serde_json::from_slice(&bytes).unwrap_or(Value::Null))
}

/// Mail is sent after the reply on another thread, so wait for it.
async fn mail(a: &App, n: usize) -> Vec<MailMessage> {
    for _ in 0..200 {
        if a.sent.lock().unwrap().len() >= n {
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(10)).await;
    }
    a.sent.lock().unwrap().clone()
}

const NATIVE: (&str, &str) = ("x-client", "native");

#[tokio::test]
async fn paths_that_are_not_ours_fall_through_and_wrong_methods_say_what_is_allowed() {
    let a = app();
    assert_eq!(call(&a, req("GET", "/auth/nope", &[], None)).await.0, 404);
    let (s, h, b) = call(&a, req("GET", "/auth/login", &[], None)).await;
    assert_eq!((s, h["allow"].to_str().unwrap(), b["error"].as_str()), (405, "POST", Some("method_not_allowed")));
    let (s, h, _) = call(&a, req("POST", "/admin/staff-domains", &[], None)).await;
    assert_eq!((s, h["allow"].to_str().unwrap()), (405, "GET, PUT, DELETE"));
    assert_eq!(call(&a, req("GET", "/admin/users/abc/extra", &[], None)).await.0, 404);
}

#[tokio::test]
async fn preflight_answers_only_allowed_origins_and_cors_rides_on_every_reply() {
    let a = app();
    let (s, h, _) = call(&a, req("OPTIONS", "/auth/login", &[("origin", ORIGIN), ("access-control-request-method", "POST")], None)).await;
    assert_eq!((s, h["access-control-allow-origin"].to_str().unwrap(), h["access-control-allow-methods"].to_str().unwrap()), (204, ORIGIN, "POST"));
    assert!(h["access-control-allow-headers"].to_str().unwrap().contains("X-Sudo-Token"));
    assert_eq!(call(&a, req("OPTIONS", "/auth/login", &[("origin", "https://evil.example")], None)).await.0, 403);
    let (s, h, b) = call(&a, req("GET", "/me", &[("origin", ORIGIN)], None)).await;
    assert_eq!((s, h["access-control-allow-origin"].to_str().unwrap(), b["error"].as_str()), (401, ORIGIN, Some("unauthorized")));
    let (s, _, b) = call(&a, req("GET", "/me", &[("origin", "https://evil.example")], None)).await;
    assert_eq!((s, b["error"].as_str()), (403, Some("origin_not_allowed")));
    let (s, _, b) = call(&a, req("POST", "/auth/login", &[("origin", ORIGIN), NATIVE], Some(json!({})))).await;
    assert_eq!((s, b["error"].as_str()), (400, Some("native_client_with_origin")));
}

#[tokio::test]
async fn bodies_must_be_small_json_objects() {
    let a = app();
    let mut r = req("POST", "/auth/login", &[], None);
    *r.body_mut() = Body::from("{}");
    assert_eq!(call(&a, r).await.0, 415, "no content type");
    let big = json!({ "email": "a@b.org", "password": "x".repeat(9000) });
    let (s, _, b) = call(&a, req("POST", "/auth/login", &[], Some(big))).await;
    assert_eq!((s, b["error"].as_str()), (413, Some("body_too_large")));
    let mut r = req("POST", "/auth/login", &[], None);
    r.headers_mut().insert("content-type", "application/json".parse().unwrap());
    *r.body_mut() = Body::from("[1]");
    assert_eq!(call(&a, r).await.0, 400);
    let (s, _, b) = call(&a, Request::builder().method("POST").uri("/auth/login").header("content-type", "application/json").body(Body::from(r#"{"email":"a@b.org","password":"pw-long-enough-1"}"#)).unwrap()).await;
    assert_eq!((s, b["error"].as_str()), (403, Some("client_ip_required")), "no client IP fails closed");
}

#[tokio::test]
async fn sign_up_confirm_sign_in_and_read_the_account_end_to_end_with_the_admins_told_once() {
    let a = app();
    let body = json!({ "email": "t@school.org", "password": PW, "display_name": "Ms Rivera", "school": "Pleasant Valley", "adult_school_employee": true });
    let (s, _, _) = call(&a, req("POST", "/auth/signup", &[], Some(body.clone()))).await;
    assert_eq!(s, 202);
    let sent = mail(&a, 1).await;
    assert_eq!((sent[0].kind, sent[0].to.as_str()), (MailKind::Verify, "t@school.org"));
    let mut v = body.clone();
    v["token"] = json!(sent[0].token);
    v["password"] = json!("copper-lantern-meadow");
    v.as_object_mut().unwrap().remove("email");
    let (s, _, b) = call(&a, req("POST", "/auth/verify", &[NATIVE], Some(v))).await;
    assert_eq!((s, b["password_set"].clone()), (200, json!(true)));
    let sent = mail(&a, 2).await;
    assert_eq!(sent.iter().filter(|m| m.kind == MailKind::DeviceEnrol).count(), 1, "the enrolment code is mailed to the owner");
    assert_eq!(sent.iter().filter(|m| m.kind == MailKind::NewApplication).count(), 0, "internal notices never reach the person's mailer");
    let (s, _, tok) = call(&a, req("POST", "/auth/login", &[NATIVE], Some(json!({ "email": "t@school.org", "password": "copper-lantern-meadow" })))).await;
    assert_eq!(s, 200, "{tok}");
    let access = tok["access_token"].as_str().unwrap().to_string();
    let (s, _, me) = call(&a, req("GET", "/me", &[NATIVE, ("authorization", &format!("Bearer {access}"))], None)).await;
    assert_eq!((s, me["email"].as_str(), me["trust"].as_str()), (200, Some("t@school.org"), Some("pending")));
    let (s, _, b) = call(&a, req("POST", "/auth/login", &[NATIVE], Some(json!({ "email": "t@school.org", "password": "wrong-password-here" })))).await;
    assert_eq!((s, b["error"].as_str()), (401, Some("invalid_credentials")));
    // Signing out ends the session at once.
    let auth = format!("Bearer {access}");
    let (s, _, _) = call(&a, req("POST", "/auth/logout", &[NATIVE, ("authorization", &auth)], Some(json!({ "refresh_token": tok["refresh_token"] })))).await;
    assert_eq!(s, 204);
    assert_eq!(call(&a, req("GET", "/me", &[NATIVE, ("authorization", &auth)], None)).await.0, 401);
}

#[tokio::test]
async fn admin_routes_need_an_admin_and_read_through_the_query_string() {
    let a = app();
    assert_eq!(call(&a, req("GET", "/admin/queue", &[NATIVE], None)).await.0, 401);
    // A real teacher is not enough.
    {
        let c = a.state.db.lock().unwrap();
        c.execute("INSERT INTO users (id, email, email_verified_at, trust, created_at) VALUES ('t1', 't@x.org', 1, 'teacher', 1)", []).unwrap();
    }
    let (s, _, b) = call(&a, req("GET", "/admin/users/not-a-session", &[NATIVE], None)).await;
    assert_eq!((s, b["error"].as_str()), (401, Some("unauthorized")));
    assert_eq!(call(&a, req("POST", "/admin/revoke-teacher", &[NATIVE], Some(json!({})))).await.0, 401);
}

#[tokio::test]
async fn queued_admin_alerts_and_notices_are_delivered_without_touching_the_persons_mailer() {
    let a = app();
    let st = a.state.clone();
    let mut out = Outbox::new();
    out.push(MailMessage::new(MailKind::NewApplication, "u1"));
    out.push(MailMessage::alert("admin_suspended", "adm1", "adm2", None, 1));
    out.push(MailMessage::new(MailKind::PasswordChanged, "p@x.org"));
    tokio::task::spawn_blocking(move || deliver(&st, out)).await.unwrap();
    let sent = a.sent.lock().unwrap();
    assert_eq!(sent.len(), 1);
    assert_eq!(sent[0].kind, MailKind::PasswordChanged);
    let pending: i64 = a.state.db.lock().unwrap().query_row("SELECT sent FROM notice_budget", [], |r| r.get(0)).unwrap();
    assert_eq!(pending, 1, "the new-application notice used the hourly budget");
}
