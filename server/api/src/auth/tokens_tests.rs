//! Tests for `tokens.rs`: verify and reset, driven end to end from signup and forgot.
use super::*;
use crate::auth::config::AuthConfig;
use crate::auth::crypto::{Keys, derive_keys};
use crate::auth::password::Argon2Params;
use crate::auth::signup;
use crate::db;
use axum::http::{HeaderName, HeaderValue};
use std::path::Path;
use std::sync::Mutex;

const T0: i64 = 1_800_000_000_000;
const FAST: Argon2Params = Argon2Params { m: 8, t: 1, p: 1 };
const PW: &str = "violet-harbor-tractor";
const NEW_PW: &str = "copper-lantern-meadow";

struct World {
    db: Mutex<Connection>,
    cfg: AuthConfig,
    keys: Keys,
    now: std::cell::Cell<i64>,
    _dir: tempfile::TempDir,
}

fn world() -> World {
    let dir = tempfile::tempdir().unwrap();
    let mut conn = db::open(&dir.path().join("t.db")).unwrap();
    db::migrate(&mut conn, &Path::new(env!("CARGO_MANIFEST_DIR")).join("../worker/migrations")).unwrap();
    let mut cfg = AuthConfig::new(&"s".repeat(32), vec![]).unwrap();
    cfg.argon2 = FAST;
    World { db: Mutex::new(conn), keys: derive_keys(&cfg.secret), cfg, now: std::cell::Cell::new(T0), _dir: dir }
}

fn ip(v: &str) -> HeaderMap {
    let mut m = HeaderMap::new();
    m.append(HeaderName::from_static("x-forwarded-for"), HeaderValue::from_str(v).unwrap());
    m
}
fn native() -> ClientInfo {
    ClientInfo { mode: ClientMode::Native, origin: None }
}
fn web() -> ClientInfo {
    ClientInfo { mode: ClientMode::Web, origin: Some("https://app.example.org".into()) }
}
fn obj(v: Value) -> Map<String, Value> {
    v.as_object().unwrap().clone()
}
fn profile(extra: Value) -> Value {
    let mut b = json!({ "display_name": "Ms Rivera", "school": "Pleasant Valley", "adult_school_employee": true });
    for (k, v) in extra.as_object().unwrap() {
        b[k] = v.clone();
    }
    b
}

impl World {
    fn ctx(&self) -> Ctx<'_> {
        Ctx { db: &self.db, cfg: &self.cfg, keys: &self.keys, now: self.now.get() }
    }
    fn advance(&self, ms: i64) {
        self.now.set(self.now.get() + ms);
    }
    fn count(&self, sql: &str) -> i64 {
        self.db.lock().unwrap().query_row(sql, [], |r| r.get(0)).unwrap()
    }
    fn exec(&self, sql: &str) {
        self.db.lock().unwrap().execute_batch(sql).unwrap();
    }
    /// Signs up and returns the verify token from the mail.
    fn signed_up(&self, email: &str) -> String {
        let mut out = Outbox::new();
        let mut b = profile(json!({ "email": email, "password": PW }));
        b["email"] = json!(email);
        signup::signup(&self.ctx(), &ip("10.0.0.1"), b.as_object().unwrap(), &mut out).unwrap();
        out[0].token.clone().unwrap()
    }
    fn forgot(&self, email: &str) -> String {
        let mut out = Outbox::new();
        signup::forgot(&self.ctx(), &ip("10.0.0.1"), &obj(json!({ "email": email })), &mut out).unwrap();
        out[0].token.clone().unwrap()
    }
    fn verify(&self, body: Value, client: &ClientInfo, out: &mut Outbox) -> Result<Response, HttpError> {
        verify(&self.ctx(), &ip("10.0.0.1"), client, &obj(body), out)
    }
    fn reset(&self, body: Value, client: &ClientInfo, out: &mut Outbox) -> Result<Response, HttpError> {
        reset(&self.ctx(), &ip("10.0.0.1"), client, &obj(body), out)
    }
}

async fn body(r: Response) -> Value {
    serde_json::from_slice(&axum::body::to_bytes(r.into_body(), 100_000).await.unwrap()).unwrap_or(Value::Null)
}
fn code(r: Result<Response, HttpError>) -> (u16, String) {
    let e = r.unwrap_err();
    (e.status, e.code)
}

#[tokio::test]
async fn verify_with_a_password_activates_the_account_opens_an_approval_and_makes_this_a_known_device() {
    let w = world();
    let token = w.signed_up("a@x.org");
    // The student who registered chose PW; the mailbox owner now chooses their own and the profile.
    let mut out = Outbox::new();
    let r = w.verify(profile(json!({ "token": token, "password": NEW_PW, "display_name": "Real Owner" })), &native(), &mut out).unwrap();
    assert_eq!(r.status(), 200);
    let b = body(r).await;
    assert_eq!((b["ok"].clone(), b["password_set"].clone()), (json!(true), json!(true)));
    assert!(b["device_token"].as_str().unwrap().starts_with("v1."));
    assert_eq!(w.count("SELECT COUNT(*) FROM users WHERE trust = 'pending' AND email_verified_at IS NOT NULL AND display_name = 'Real Owner' AND device_epoch = 1"), 1);
    assert_eq!(w.count("SELECT COUNT(*) FROM approval_requests WHERE email_domain = 'x.org' AND display_name_snapshot = 'Real Owner'"), 1);
    assert_eq!(out.len(), 2);
    assert_eq!(out[0].kind, MailKind::NewApplication, "the admins hear about it exactly once");
    assert_eq!(out[1].kind, MailKind::DeviceEnrol);
    let phc: String = w.db.lock().unwrap().query_row("SELECT pw_hash FROM users", [], |r| r.get(0)).unwrap();
    assert!(crate::auth::password::verify_password(&phc, NEW_PW) && !crate::auth::password::verify_password(&phc, PW), "the pre-confirmation password is gone");
    assert_eq!(w.count("SELECT COUNT(*) FROM email_tokens WHERE used_at IS NULL"), 0);
    assert_eq!(code(w.verify(profile(json!({ "token": token, "password": NEW_PW })), &native(), &mut out)), (400, "invalid_token".into()), "a link works once");
}

#[tokio::test]
async fn the_web_gets_the_device_cookie_and_the_body_has_no_token() {
    let w = world();
    let token = w.signed_up("a@x.org");
    let mut out = Outbox::new();
    let r = w.verify(profile(json!({ "token": token, "password": NEW_PW })), &web(), &mut out).unwrap();
    assert!(r.headers().get_all("set-cookie").iter().any(|c| c.to_str().unwrap().starts_with("__Host-device=v1.")));
    assert!(body(r).await.get("device_token").is_none());
}

#[tokio::test]
async fn verify_without_a_password_confirms_but_drops_what_the_registrant_chose_and_does_not_activate() {
    let w = world();
    let token = w.signed_up("a@x.org");
    let mut out = Outbox::new();
    let b = body(w.verify(json!({ "token": token }), &native(), &mut out).unwrap()).await;
    assert_eq!(b["password_set"], json!(false));
    assert_eq!(w.count("SELECT COUNT(*) FROM users WHERE trust = 'unverified' AND email_verified_at IS NOT NULL AND pw_hash IS NULL AND display_name IS NULL AND profile IS NULL"), 1);
    assert_eq!(w.count("SELECT COUNT(*) FROM approval_requests"), 0);
    assert!(out.is_empty());
    // It is finished through forgot/reset, which supplies the profile and activates.
    let rt = w.forgot("a@x.org");
    let r = w.reset(profile(json!({ "token": rt, "password": NEW_PW })), &native(), &mut out).unwrap();
    assert_eq!(r.status(), 200);
    assert_eq!(w.count("SELECT COUNT(*) FROM users WHERE trust = 'pending'"), 1);
    assert_eq!(w.count("SELECT COUNT(*) FROM approval_requests"), 1);
}

#[tokio::test]
async fn verify_input_errors_and_junk_tokens() {
    let w = world();
    let token = w.signed_up("a@x.org");
    let mut out = Outbox::new();
    assert_eq!(code(w.verify(json!({}), &native(), &mut out)), (400, "invalid_token".into()));
    assert_eq!(code(w.verify(json!({ "token": "short" }), &native(), &mut out)), (400, "invalid_token".into()));
    assert_eq!(code(w.verify(json!({ "token": random_token() }), &native(), &mut out)), (400, "invalid_token".into()));
    assert_eq!(code(w.verify(json!({ "token": token, "password": NEW_PW }), &native(), &mut out)).1, "invalid_request", "a password needs the profile too");
    assert_eq!(code(w.verify(profile(json!({ "token": token, "password": "short" })), &native(), &mut out)), (400, "weak_password".into()));
    assert_eq!(w.count("SELECT COUNT(*) FROM email_tokens WHERE used_at IS NULL"), 1, "failed attempts leave the link usable and unleased");
    assert_eq!(w.count("SELECT COUNT(*) FROM email_tokens WHERE claimed_until IS NOT NULL"), 0);
    assert_eq!(verify(&w.ctx(), &HeaderMap::new(), &native(), &obj(json!({ "token": token })), &mut out).unwrap_err().code, "client_ip_required");
    // Only junk is charged to the IP: 30 an hour, then 429.
    let mut limited = false;
    for _ in 0..40 {
        if let Err(e) = w.verify(json!({ "token": random_token() }), &native(), &mut out) {
            limited |= e.status == 429;
        }
    }
    assert!(limited);
    assert!(w.verify(json!({ "token": token }), &native(), &mut out).is_ok(), "a valid link still works from the limited address");
}

#[tokio::test]
async fn an_expired_link_and_a_leased_link_are_refused() {
    let w = world();
    let token = w.signed_up("a@x.org");
    let mut out = Outbox::new();
    w.exec(&format!("UPDATE email_tokens SET claimed_until = {} WHERE kind = 'verify'", T0 + 20_000));
    assert_eq!(code(w.verify(profile(json!({ "token": token, "password": NEW_PW })), &native(), &mut out)), (409, "in_progress".into()));
    w.advance(31_000);
    assert!(w.verify(profile(json!({ "token": token, "password": NEW_PW })), &native(), &mut out).is_ok(), "the lease lapses");
    let t2 = w.signed_up("b@x.org");
    w.advance(25 * 3_600_000);
    assert_eq!(code(w.verify(json!({ "token": t2 }), &native(), &mut out)), (400, "invalid_token".into()), "past the one-day lifetime");
}

#[tokio::test]
async fn confirming_ends_sessions_and_every_other_outstanding_link() {
    let w = world();
    let token = w.signed_up("a@x.org");
    w.exec("INSERT INTO sessions (id, family_id, user_id, refresh_hash, created_at, auth_at, expires_at) SELECT 's1', 'f1', id, 'h1', 1, 1, 9999999999999 FROM users");
    let reset_token = w.forgot("a@x.org");
    let mut out = Outbox::new();
    w.verify(profile(json!({ "token": token, "password": NEW_PW })), &native(), &mut out).unwrap();
    assert_eq!(w.count("SELECT COUNT(*) FROM sessions WHERE revoked_at IS NULL"), 0);
    assert_eq!(code(w.reset(profile(json!({ "token": reset_token, "password": "another-strong-one" })), &native(), &mut out)), (400, "invalid_token".into()));
}

#[tokio::test]
async fn reset_changes_the_password_ends_every_session_mails_a_notice_and_re_keys_devices() {
    let w = world();
    let t = w.signed_up("a@x.org");
    let mut out = Outbox::new();
    w.verify(profile(json!({ "token": t, "password": PW })), &native(), &mut out).unwrap();
    w.exec("INSERT INTO sessions (id, family_id, user_id, refresh_hash, created_at, auth_at, expires_at) SELECT 's1', 'f1', id, 'h1', 1, 1, 9999999999999 FROM users");
    w.advance(200_000);
    let rt = w.forgot("a@x.org");
    let mut out = Outbox::new();
    let r = w.reset(json!({ "token": rt, "password": NEW_PW }), &web(), &mut out).unwrap();
    assert_eq!(r.status(), 200);
    let cookies: Vec<String> = r.headers().get_all("set-cookie").iter().map(|c| c.to_str().unwrap().to_string()).collect();
    assert!(cookies.iter().any(|c| c.starts_with("__Host-refresh=; ")) && cookies.iter().any(|c| c.starts_with("__Host-device=v1.")));
    assert_eq!(w.count("SELECT COUNT(*) FROM sessions WHERE revoked_at IS NULL"), 0);
    assert_eq!(w.count("SELECT device_epoch FROM users"), 2, "once at verify, once at reset");
    let kinds: Vec<MailKind> = out.iter().map(|m| m.kind).collect();
    assert_eq!(kinds, vec![MailKind::PasswordChanged, MailKind::DeviceEnrol]);
    let phc: String = w.db.lock().unwrap().query_row("SELECT pw_hash FROM users", [], |r| r.get(0)).unwrap();
    assert!(crate::auth::password::verify_password(&phc, NEW_PW));
    assert_eq!(code(w.reset(json!({ "token": rt, "password": NEW_PW }), &web(), &mut out)), (400, "invalid_token".into()), "a reset link works once");
}

#[tokio::test]
async fn reset_for_an_inactive_account_requires_the_profile_and_a_weak_password_is_refused_without_burning_the_link() {
    let w = world();
    w.signed_up("a@x.org");
    w.advance(200_000);
    let rt = w.forgot("a@x.org");
    let mut out = Outbox::new();
    assert_eq!(w.reset(json!({ "token": rt, "password": NEW_PW }), &native(), &mut out).unwrap_err().code, "invalid_request");
    assert_eq!(code(w.reset(profile(json!({ "token": rt, "password": "short" })), &native(), &mut out)), (400, "weak_password".into()));
    assert_eq!(code(w.reset(profile(json!({ "token": rt, "password": "a@x.org-a@x.org" })), &native(), &mut out)), (400, "weak_password".into()));
    assert_eq!(w.count("SELECT COUNT(*) FROM email_tokens WHERE kind = 'reset' AND used_at IS NULL AND claimed_until IS NULL"), 1);
    assert!(w.reset(profile(json!({ "token": rt, "password": NEW_PW })), &native(), &mut out).is_ok());
    assert_eq!(code(w.reset(json!({ "token": random_token(), "password": NEW_PW }), &native(), &mut out)), (400, "invalid_token".into()));
}

#[tokio::test]
async fn the_enrolment_code_is_limited_by_gap_and_daily_count_when_asked_to_be() {
    let w = world();
    let t = w.signed_up("a@x.org");
    let mut out = Outbox::new();
    w.verify(profile(json!({ "token": t, "password": PW })), &native(), &mut out).unwrap();
    let uid: String = w.db.lock().unwrap().query_row("SELECT id FROM users", [], |r| r.get(0)).unwrap();
    let conn = w.db.lock().unwrap();
    let mut o = Outbox::new();
    assert!(!issue_enrolment(&w.ctx(), &conn, &uid, "a@x.org", 1, true, &mut o).unwrap(), "the verify code was just mailed: inside the gap");
    drop(conn);
    for _ in 0..4 {
        w.advance(121_000);
        let conn = w.db.lock().unwrap();
        assert!(issue_enrolment(&w.ctx(), &conn, &uid, "a@x.org", 1, true, &mut o).unwrap());
    }
    w.advance(121_000);
    let conn = w.db.lock().unwrap();
    assert!(!issue_enrolment(&w.ctx(), &conn, &uid, "a@x.org", 1, true, &mut o).unwrap(), "five a day");
    assert_eq!(o.len(), 4);
}
