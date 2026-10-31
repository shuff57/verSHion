//! Tests for `session.rs`: the Worker's login, refresh, logout and authenticate cases over a real migrated database.
use super::*;
use crate::auth::config::AuthConfig;
use crate::auth::crypto::derive_keys;
use crate::auth::password::Argon2Params;
use crate::db;
use axum::http::{HeaderName, HeaderValue};
use std::path::Path;

const T0: i64 = 1_800_000_000_000;
const FAST: Argon2Params = Argon2Params { m: 8, t: 1, p: 1 };
const PW: &str = "violet-harbor-tractor";
const ORIGIN: &str = "https://app.example.org";

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
    let mut cfg = AuthConfig::new(&"s".repeat(32), vec![ORIGIN.to_string()]).unwrap();
    cfg.argon2 = FAST;
    World { db: Mutex::new(conn), keys: derive_keys(&cfg.secret), cfg, now: std::cell::Cell::new(T0), _dir: dir }
}

fn hm(pairs: &[(&str, &str)]) -> HeaderMap {
    let mut m = HeaderMap::new();
    for (k, v) in pairs {
        m.append(HeaderName::from_bytes(k.as_bytes()).unwrap(), HeaderValue::from_str(v).unwrap());
    }
    m
}
fn web() -> ClientInfo {
    ClientInfo { mode: ClientMode::Web, origin: Some(ORIGIN.into()) }
}
fn native() -> ClientInfo {
    ClientInfo { mode: ClientMode::Native, origin: None }
}
fn obj(v: Value) -> Map<String, Value> {
    v.as_object().unwrap().clone()
}

impl World {
    fn ctx(&self) -> Ctx<'_> {
        Ctx { db: &self.db, cfg: &self.cfg, keys: &self.keys, now: self.now.get() }
    }
    fn advance(&self, ms: i64) {
        self.now.set(self.now.get() + ms);
    }
    fn exec(&self, sql: &str) {
        self.db.lock().unwrap().execute_batch(sql).unwrap();
    }
    fn count(&self, sql: &str) -> i64 {
        self.db.lock().unwrap().query_row(sql, [], |r| r.get(0)).unwrap()
    }
    /// An active account (verified, with the given trust).
    fn user(&self, email: &str, trust: &str) -> String {
        let id = uuid_v4();
        let phc = hash_password(PW, FAST).unwrap().phc;
        self.db
            .lock()
            .unwrap()
            .execute("INSERT INTO users (id, email, email_verified_at, pw_hash, pw_params, trust, created_at) VALUES (?1, ?2, 1, ?3, ?4, ?5, 1)", params![id, email, phc, params_json(&FAST), trust])
            .unwrap();
        id
    }
    fn login(&self, email: &str, pw: &str, ip: &str, client: &ClientInfo, extra: &[(&str, &str)]) -> Result<Response, HttpError> {
        let mut h = vec![("x-forwarded-for", ip)];
        h.extend_from_slice(extra);
        login(&self.ctx(), &hm(&h), client, &obj(json!({ "email": email, "password": pw })))
    }
}

async fn body(r: Response) -> Value {
    serde_json::from_slice(&axum::body::to_bytes(r.into_body(), 100_000).await.unwrap()).unwrap_or(Value::Null)
}
fn cookie(r: &Response, name: &str) -> Option<String> {
    r.headers().get_all("set-cookie").iter().filter_map(|v| v.to_str().ok()).find_map(|c| c.strip_prefix(&format!("{name}=")).map(|rest| rest.split(';').next().unwrap().to_string()))
}
fn err(r: Result<Response, HttpError>) -> (u16, String) {
    let e = r.unwrap_err();
    (e.status, e.code)
}

#[tokio::test]
async fn a_web_login_sets_the_refresh_and_device_cookies_and_a_native_one_returns_them_in_the_body() {
    let w = world();
    let uid = w.user("a@x.org", "pending");
    let r = w.login("A@X.org", PW, "10.0.0.1", &web(), &[]).unwrap();
    let (refresh, device) = (cookie(&r, "__Host-refresh").unwrap(), cookie(&r, "__Host-device").unwrap());
    assert!(is_token(&refresh) && device.starts_with("v1."));
    let b = body(r).await;
    assert!(b.get("refresh_token").is_none() && b.get("device_token").is_none(), "the web body never carries a token");
    assert_eq!((b["token_type"].as_str(), b["expires_in"].as_i64(), b["user"].clone()), (Some("Bearer"), Some(900), json!({ "id": uid, "trust": "pending" })));
    let claims = verify_access_token(&w.keys.access, b["access_token"].as_str().unwrap(), &Expect { iss: "vershion-pool", aud: "vershion-pool", now_seconds: T0 / 1000 }).unwrap();
    assert_eq!((claims.sub, claims.trust), (uid.clone(), "pending".into()));

    let n = w.login("a@x.org", PW, "10.0.0.2", &native(), &[]).unwrap();
    let nb = body(n).await;
    assert!(is_token(nb["refresh_token"].as_str().unwrap()) && nb["device_token"].as_str().unwrap().starts_with("v1.") && nb["refresh_expires_in"].as_i64().unwrap() > 0);
    assert_eq!(w.count("SELECT COUNT(*) FROM sessions"), 2);
    assert_eq!(w.count(&format!("SELECT COUNT(*) FROM sessions WHERE refresh_hash = '{}'", sha256(&refresh))), 1, "the hash is stored");
    assert_eq!(w.count(&format!("SELECT COUNT(*) FROM sessions WHERE refresh_hash = '{refresh}'")), 0, "the token itself never is");
}

#[tokio::test]
async fn wrong_unknown_unconfirmed_and_unapproved_accounts_all_get_the_same_401() {
    let w = world();
    w.user("a@x.org", "pending");
    w.user("fresh@x.org", "unverified");
    w.user("unconfirmed@x.org", "pending");
    w.exec("UPDATE users SET email_verified_at = NULL WHERE email = 'unconfirmed@x.org'");
    for (email, pw) in [("a@x.org", "wrong-password-here"), ("nobody@x.org", PW), ("fresh@x.org", PW), ("unconfirmed@x.org", PW)] {
        assert_eq!(err(w.login(email, pw, "10.0.0.1", &web(), &[])), (401, "invalid_credentials".into()), "{email}");
    }
    assert_eq!(w.count("SELECT COUNT(*) FROM sessions"), 0);
    // A bad request is a 400 before anything else, and a missing client address fails closed.
    assert_eq!(login(&w.ctx(), &hm(&[("x-forwarded-for", "10.0.0.1")]), &web(), &obj(json!({ "email": "nope", "password": PW }))).unwrap_err().status, 400);
    assert_eq!(login(&w.ctx(), &hm(&[]), &web(), &obj(json!({ "email": "a@x.org", "password": PW }))).unwrap_err().code, "client_ip_required");
}

#[tokio::test]
async fn a_suspended_account_gets_403_only_with_the_right_password() {
    let w = world();
    w.user("a@x.org", "pending");
    w.exec("UPDATE users SET suspended_at = 5");
    assert_eq!(err(w.login("a@x.org", PW, "10.0.0.1", &web(), &[])), (403, "account_suspended".into()));
    assert_eq!(err(w.login("a@x.org", "wrong-password-here", "10.0.0.1", &web(), &[])), (401, "invalid_credentials".into()));
}

#[tokio::test]
async fn five_wrong_passwords_from_one_network_lock_it_but_a_success_gives_the_slot_back() {
    let w = world();
    w.user("a@x.org", "pending");
    for _ in 0..4 {
        assert_eq!(w.login("a@x.org", "wrong-password-here", "10.0.0.1", &native(), &[]).unwrap_err().status, 401);
    }
    assert!(w.login("a@x.org", PW, "10.0.0.1", &native(), &[]).is_ok(), "the 5th attempt is allowed and succeeds");
    let e = w.login("a@x.org", "wrong-password-here", "10.0.0.1", &native(), &[]).unwrap_err();
    assert_eq!((e.status, e.code.as_str()), (401, "invalid_credentials"), "the success returned its slot, so this is only the 5th failure");
    let locked = w.login("a@x.org", PW, "10.0.0.1", &native(), &[]).unwrap_err();
    assert_eq!((locked.status, locked.code.as_str()), (429, "rate_limited"), "now even the right password is refused from that network");
    assert!(locked.headers.iter().any(|(k, _)| k == "Retry-After"));
    assert!(w.login("a@x.org", PW, "10.0.5.1", &native(), &[]).is_ok(), "another network still works");
}

#[tokio::test]
async fn a_known_device_is_not_locked_out_by_other_peoples_failures() {
    let w = world();
    w.user("a@x.org", "pending");
    let first = w.login("a@x.org", PW, "10.0.0.1", &native(), &[]).unwrap();
    let device = body(first).await["device_token"].as_str().unwrap().to_string();
    for i in 0..30 {
        let _ = w.login("a@x.org", "wrong-password-here", &format!("10.1.{i}.1"), &native(), &[]);
    }
    assert_eq!(w.login("a@x.org", PW, "10.1.99.1", &native(), &[]).unwrap_err().status, 429, "locked down for every network");
    assert!(w.login("a@x.org", PW, "10.1.99.1", &native(), &[("x-device-token", &device)]).is_ok(), "the owner's device still gets in");
}

#[tokio::test]
async fn a_login_after_the_hash_cost_was_raised_upgrades_the_stored_hash() {
    let mut w = world();
    let uid = w.user("a@x.org", "pending");
    w.cfg.argon2 = Argon2Params { m: 16, t: 1, p: 1 };
    w.login("a@x.org", PW, "10.0.0.1", &native(), &[]).unwrap();
    let (phc, params): (String, String) = w.db.lock().unwrap().query_row("SELECT pw_hash, pw_params FROM users WHERE id = ?1", [&uid], |r| Ok((r.get(0)?, r.get(1)?))).unwrap();
    assert!(phc.contains("m=16,t=1,p=1") && params.contains("\"m\":16"));
    assert!(verify_password(&phc, PW));
}

#[tokio::test]
async fn a_web_login_ends_the_browsers_older_session() {
    let w = world();
    w.user("a@x.org", "pending");
    let r1 = w.login("a@x.org", PW, "10.0.0.1", &web(), &[]).unwrap();
    let old = cookie(&r1, "__Host-refresh").unwrap();
    w.login("a@x.org", PW, "10.0.0.1", &web(), &[("cookie", &format!("__Host-refresh={old}"))]).unwrap();
    assert_eq!(w.count("SELECT COUNT(*) FROM sessions WHERE revoked_at IS NOT NULL"), 1);
    assert_eq!(w.count("SELECT COUNT(*) FROM sessions WHERE revoked_at IS NULL"), 1);
}

/// Logs in natively and returns (refresh token, access token).
async fn native_login(w: &World) -> (String, String) {
    let b = body(w.login("a@x.org", PW, "10.0.0.1", &native(), &[]).unwrap()).await;
    (b["refresh_token"].as_str().unwrap().into(), b["access_token"].as_str().unwrap().into())
}
fn refresh_native(w: &World, tok: &str) -> Result<Response, HttpError> {
    refresh(&w.ctx(), &hm(&[("x-forwarded-for", "10.0.0.1")]), &native(), Some(&obj(json!({ "refresh_token": tok }))))
}

#[tokio::test]
async fn refresh_rotates_to_the_derived_successor_and_the_old_row_stops_authenticating() {
    let w = world();
    w.user("a@x.org", "pending");
    let (r1, at1) = native_login(&w).await;
    w.advance(60_000);
    let b = body(refresh_native(&w, &r1).unwrap()).await;
    let r2 = b["refresh_token"].as_str().unwrap().to_string();
    assert_eq!(r2, hmac(&w.keys.refresh, &r1), "the successor is derived, never random");
    assert_eq!(w.count("SELECT COUNT(*) FROM sessions"), 2);
    let h = hm(&[("authorization", &format!("Bearer {at1}"))]);
    let h2 = hm(&[("authorization", &format!("Bearer {}", b["access_token"].as_str().unwrap()))]);
    assert_eq!(authenticate(&w.ctx(), &h2).unwrap().trust, "pending");
    // Both tokens carry the same family, and the family's live head is the new row.
    assert_eq!(authenticate(&w.ctx(), &h).map(|p| p.session_family), authenticate(&w.ctx(), &h2).map(|p| p.session_family));
    assert!(refresh_native(&w, &r2).is_ok(), "the new token rotates in turn");
}

#[tokio::test]
async fn a_second_use_inside_the_grace_gets_the_same_successor_even_after_it_moved_on() {
    let w = world();
    w.user("a@x.org", "pending");
    let (r1, _) = native_login(&w).await;
    w.advance(1000);
    let r2 = body(refresh_native(&w, &r1).unwrap()).await["refresh_token"].as_str().unwrap().to_string();
    w.advance(1000);
    let again = body(refresh_native(&w, &r1).unwrap()).await["refresh_token"].as_str().unwrap().to_string();
    assert_eq!(again, r2, "two tabs get the same token");
    w.advance(1000);
    let r3 = body(refresh_native(&w, &r2).unwrap()).await["refresh_token"].as_str().unwrap().to_string();
    let late = body(refresh_native(&w, &r1).unwrap()).await["refresh_token"].as_str().unwrap().to_string();
    assert_eq!(late, r3, "a late caller is walked to the live head, not given a spent token");
}

#[tokio::test]
async fn reuse_outside_the_grace_ends_the_whole_family() {
    let w = world();
    w.user("a@x.org", "pending");
    let (r1, _) = native_login(&w).await;
    w.advance(1000);
    let r2 = body(refresh_native(&w, &r1).unwrap()).await["refresh_token"].as_str().unwrap().to_string();
    w.advance(11_000);
    assert_eq!(err(refresh_native(&w, &r1)), (401, "invalid_refresh_token".into()));
    assert_eq!(w.count("SELECT COUNT(*) FROM sessions WHERE revoked_at IS NULL"), 0, "the thief and the owner are both signed out");
    assert_eq!(err(refresh_native(&w, &r2)), (401, "invalid_refresh_token".into()));
}

#[tokio::test]
async fn expired_suspended_and_unknown_tokens_fail_and_web_failures_clear_the_cookie() {
    let w = world();
    w.user("a@x.org", "pending");
    let (r1, _) = native_login(&w).await;
    assert_eq!(err(refresh_native(&w, &random_token())), (401, "invalid_refresh_token".into()));
    assert_eq!(err(refresh(&w.ctx(), &hm(&[("x-forwarded-for", "10.0.0.1")]), &native(), Some(&obj(json!({}))))), (401, "invalid_refresh_token".into()));
    let e = refresh(&w.ctx(), &hm(&[("x-forwarded-for", "10.0.0.1"), ("cookie", "__Host-refresh=nope")]), &web(), None).unwrap_err();
    assert!(e.headers.iter().any(|(k, v)| k == "Set-Cookie" && v.starts_with("__Host-refresh=; ")));
    let no_origin = ClientInfo { mode: ClientMode::Web, origin: None };
    assert_eq!(err(refresh(&w.ctx(), &hm(&[("x-forwarded-for", "10.0.0.1")]), &no_origin, None)), (403, "origin_required".into()));
    w.exec("UPDATE users SET suspended_at = 5");
    assert_eq!(err(refresh_native(&w, &r1)), (401, "invalid_refresh_token".into()), "a suspended account cannot refresh");
    w.exec("UPDATE users SET suspended_at = NULL");
    w.advance(31 * 86_400_000);
    assert_eq!(err(refresh_native(&w, &r1)), (401, "invalid_refresh_token".into()), "past the refresh lifetime");
}

#[tokio::test]
async fn a_session_cannot_outlive_the_maximum_age_by_refreshing() {
    let w = world();
    w.user("a@x.org", "pending");
    let (mut tok, _) = native_login(&w).await;
    for _ in 0..3 {
        w.advance(25 * 86_400_000);
        tok = body(refresh_native(&w, &tok).unwrap()).await["refresh_token"].as_str().unwrap().to_string();
    }
    w.advance(25 * 86_400_000); // 100 days since the login; the family is capped at 90
    assert_eq!(err(refresh_native(&w, &tok)), (401, "invalid_refresh_token".into()));
}

#[tokio::test]
async fn authenticate_reads_the_current_trust_and_refuses_revoked_and_suspended_sessions() {
    let w = world();
    w.user("a@x.org", "pending");
    let (_, at) = native_login(&w).await;
    let h = hm(&[("authorization", &format!("Bearer {at}"))]);
    assert_eq!(authenticate(&w.ctx(), &h).unwrap().trust, "pending");
    w.exec("UPDATE users SET trust = 'teacher'");
    assert_eq!(authenticate(&w.ctx(), &h).unwrap().trust, "teacher", "a promotion shows at once, not when the token expires");
    assert!(authenticate(&w.ctx(), &hm(&[])).is_none() && authenticate(&w.ctx(), &hm(&[("authorization", "Bearer a.b.c")])).is_none());
    assert_eq!(require_principal(&w.ctx(), &hm(&[])).unwrap_err().status, 401);
    w.exec("UPDATE users SET suspended_at = 1");
    assert!(authenticate(&w.ctx(), &h).is_none());
    w.exec("UPDATE users SET suspended_at = NULL");
    assert!(authenticate(&w.ctx(), &h).is_some());
    w.exec("UPDATE sessions SET revoked_at = 1");
    assert!(authenticate(&w.ctx(), &h).is_none());
    w.advance(901_000);
    w.exec("UPDATE sessions SET revoked_at = NULL");
    assert!(authenticate(&w.ctx(), &h).is_none(), "expired access token");
}

#[tokio::test]
async fn logout_ends_the_family_always_answers_204_and_clears_cookies_for_the_web() {
    let w = world();
    w.user("a@x.org", "pending");
    let r = w.login("a@x.org", PW, "10.0.0.1", &web(), &[]).unwrap();
    let rt = cookie(&r, "__Host-refresh").unwrap();
    let h = hm(&[("x-forwarded-for", "10.0.0.1"), ("cookie", &format!("__Host-refresh={rt}"))]);
    let out = logout(&w.ctx(), &h, &web(), None).unwrap();
    assert_eq!(out.status(), 204);
    assert!(cookie(&out, "__Host-refresh").is_some_and(|v| v.is_empty()) && cookie(&out, "__Host-device").is_none(), "the device cookie survives a plain logout");
    assert_eq!(w.count("SELECT COUNT(*) FROM sessions WHERE revoked_at IS NULL"), 0);
    let forget = logout(&w.ctx(), &h, &web(), Some(&obj(json!({ "forget_device": true })))).unwrap();
    assert!(cookie(&forget, "__Host-device").is_some_and(|v| v.is_empty()));
    // Junk gets the same 204 and is charged to the IP.
    assert_eq!(logout(&w.ctx(), &hm(&[("x-forwarded-for", "10.0.0.9")]), &native(), None).unwrap().status(), 204);
    assert!(w.count("SELECT COUNT(*) FROM rate_limits WHERE key LIKE 'i:%:refresh:%'") > 0);
}

#[tokio::test]
async fn logout_everywhere_needs_the_refresh_token_revokes_all_sessions_and_bumps_the_device_epoch() {
    let w = world();
    let uid = w.user("a@x.org", "pending");
    let (rt, at) = native_login(&w).await;
    native_login(&w).await;
    let h = hm(&[("x-forwarded-for", "10.0.0.1"), ("authorization", &format!("Bearer {at}"))]);
    let global = obj(json!({ "scope": "global" }));
    assert_eq!(err(logout(&w.ctx(), &h, &native(), Some(&global))), (401, "refresh_token_required".into()), "a stolen access token cannot do this");
    assert_eq!(w.count("SELECT COUNT(*) FROM sessions WHERE revoked_at IS NULL"), 2);
    let ok = logout(&w.ctx(), &hm(&[("x-forwarded-for", "10.0.0.1")]), &native(), Some(&obj(json!({ "scope": "global", "refresh_token": rt })))).unwrap();
    assert_eq!(ok.status(), 204);
    assert_eq!(w.count("SELECT COUNT(*) FROM sessions WHERE revoked_at IS NULL"), 0);
    let epoch: i64 = w.db.lock().unwrap().query_row("SELECT device_epoch FROM users WHERE id = ?1", [&uid], |r| r.get(0)).unwrap();
    assert_eq!(epoch, 1);
}
