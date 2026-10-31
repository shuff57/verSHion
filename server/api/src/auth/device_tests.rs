//! Tests for `device.rs`: sudo tokens, enrol, enrolment_code and reauth over a real migrated database.
use super::*;
use crate::auth::config::AuthConfig;
use crate::auth::crypto::{derive_keys, random_token, uuid_v4};
use crate::auth::http::ClientMode;
use crate::auth::password::{Argon2Params, hash_password, params_json};
use crate::db;
use axum::http::{HeaderName, HeaderValue};
use rusqlite::Connection;
use std::path::Path;
use std::sync::Mutex;

const T0: i64 = 1_800_000_000_000;
const FAST: Argon2Params = Argon2Params { m: 8, t: 1, p: 1 };
const PW: &str = "violet-harbor-tractor";

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

fn hdrs(pairs: &[(&str, &str)]) -> HeaderMap {
    let mut m = HeaderMap::new();
    m.append(HeaderName::from_static("x-forwarded-for"), HeaderValue::from_static("10.0.0.1"));
    for (k, v) in pairs {
        m.append(HeaderName::from_bytes(k.as_bytes()).unwrap(), HeaderValue::from_str(v).unwrap());
    }
    m
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
    fn count(&self, sql: &str) -> i64 {
        self.db.lock().unwrap().query_row(sql, [], |r| r.get(0)).unwrap()
    }
    fn exec(&self, sql: &str) {
        self.db.lock().unwrap().execute_batch(sql).unwrap();
    }
    /// An active teacher with a live session; returns (user id, access token, device token).
    fn user(&self) -> (String, String, String) {
        let id = uuid_v4();
        let phc = hash_password(PW, FAST).unwrap().phc;
        self.db
            .lock()
            .unwrap()
            .execute("INSERT INTO users (id, email, email_verified_at, pw_hash, pw_params, trust, created_at) VALUES (?1, 'a@x.org', 1, ?2, ?3, 'teacher', 1)", params![id, phc, params_json(&FAST)])
            .unwrap();
        let login = crate::auth::session::login(&self.ctx(), &hdrs(&[]), &native(), &obj(json!({ "email": "a@x.org", "password": PW }))).unwrap();
        let b: Value = serde_json::from_slice(&futures_body(login)).unwrap();
        (id, b["access_token"].as_str().unwrap().into(), b["device_token"].as_str().unwrap().into())
    }
    fn enrolment(&self, uid: &str, epoch: i64) -> String {
        let code = random_token();
        self.db
            .lock()
            .unwrap()
            .execute("INSERT INTO device_enrolments (id, user_id, code_hash, epoch, created_at, expires_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6)", params![uuid_v4(), uid, sha256(&code), epoch, self.now.get(), self.now.get() + 900_000])
            .unwrap();
        code
    }
}

fn futures_body(r: Response) -> Vec<u8> {
    tokio::runtime::Builder::new_current_thread().build().unwrap().block_on(async { axum::body::to_bytes(r.into_body(), 100_000).await.unwrap().to_vec() })
}
fn code(r: Result<Response, HttpError>) -> (u16, String) {
    let e = r.unwrap_err();
    (e.status, e.code)
}

fn principal() -> Principal {
    Principal { user_id: uuid_v4(), session_family: uuid_v4(), trust: "admin".into() }
}

#[test]
fn a_sudo_token_is_bound_to_the_session_the_epoch_and_five_minutes() {
    let keys = derive_keys(&"s".repeat(32));
    let p = principal();
    let (tok, ttl) = issue_sudo(&keys, &p, 3, T0);
    assert_eq!(ttl, 300);
    assert!(sudo_valid(&keys, Some(&tok), &p, 3, T0 + 299_000));
    assert!(!sudo_valid(&keys, Some(&tok), &p, 3, T0 + 301_000), "expired");
    assert!(!sudo_valid(&keys, Some(&tok), &p, 4, T0), "a reset or logout-everywhere voids it");
    assert!(!sudo_valid(&keys, Some(&tok), &Principal { session_family: uuid_v4(), ..p.clone() }, 3, T0), "another session's token is useless");
    assert!(!sudo_valid(&keys, Some(&tok), &Principal { user_id: uuid_v4(), ..p.clone() }, 3, T0));
    assert!(!sudo_valid(&keys, None, &p, 3, T0));
    let tampered = tok.replace(".3.", ".9.");
    assert!(!sudo_valid(&keys, Some(&tampered), &p, 9, T0));
    let other = derive_keys(&"t".repeat(32));
    assert!(!sudo_valid(&other, Some(&tok), &p, 3, T0), "signed by a different secret");
    assert!(!sudo_valid(&keys, Some("s1.a.b.c.d.e"), &p, 3, T0));
}

#[test]
fn reauth_needs_a_known_device_and_the_password_and_wrong_passwords_are_the_only_charge() {
    let w = world();
    let (uid, at, dev) = w.user();
    let auth = format!("Bearer {at}");
    let ok = reauth(&w.ctx(), &hdrs(&[("authorization", &auth), ("x-device-token", &dev)]), &native(), &obj(json!({ "password": PW }))).unwrap();
    let b: Value = serde_json::from_slice(&futures_body(ok)).unwrap();
    let p = Principal { user_id: uid.clone(), session_family: w.db.lock().unwrap().query_row("SELECT family_id FROM sessions", [], |r| r.get(0)).unwrap(), trust: "teacher".into() };
    assert!(sudo_valid(&w.keys, b["sudo_token"].as_str(), &p, 0, T0) && b["expires_in"] == 300);
    // Correct passwords are refunded, so an admin can re-authenticate again and again.
    for _ in 0..8 {
        assert!(reauth(&w.ctx(), &hdrs(&[("authorization", &auth), ("x-device-token", &dev)]), &native(), &obj(json!({ "password": PW }))).is_ok());
    }
    assert_eq!(code(reauth(&w.ctx(), &hdrs(&[("authorization", &auth)]), &native(), &obj(json!({ "password": PW })))), (403, "known_device_required".into()));
    assert_eq!(code(reauth(&w.ctx(), &hdrs(&[("authorization", &auth), ("x-device-token", &dev)]), &native(), &obj(json!({ "password": "wrong-password-here" })))), (401, "invalid_credentials".into()));
    assert_eq!(code(reauth(&w.ctx(), &hdrs(&[("x-device-token", &dev)]), &native(), &obj(json!({ "password": PW })))).0, 401, "no access token");
    let mut stopped = false;
    for _ in 0..8 {
        stopped |= reauth(&w.ctx(), &hdrs(&[("authorization", &auth), ("x-device-token", &dev)]), &native(), &obj(json!({ "password": "wrong-password-here" }))).is_err_and(|e| e.status == 429);
    }
    assert!(stopped, "five wrong passwords an hour per account");
}

#[test]
fn enrol_redeems_a_code_with_the_password_once_and_returns_a_device_token() {
    let w = world();
    let (uid, _, _) = w.user();
    let c1 = w.enrolment(&uid, 0);
    let r = enrol(&w.ctx(), &hdrs(&[]), &native(), &obj(json!({ "code": c1, "password": PW }))).unwrap();
    let b: Value = serde_json::from_slice(&futures_body(r)).unwrap();
    assert!(b["device_token"].as_str().unwrap().starts_with("v1.") && b["ok"] == true);
    assert_eq!(code(enrol(&w.ctx(), &hdrs(&[]), &native(), &obj(json!({ "code": c1, "password": PW })))), (400, "invalid_token".into()), "single use");
    assert_eq!(code(enrol(&w.ctx(), &hdrs(&[]), &native(), &obj(json!({ "password": PW })))), (400, "invalid_token".into()));
    assert_eq!(code(enrol(&w.ctx(), &hdrs(&[]), &native(), &obj(json!({ "code": random_token(), "password": PW })))), (400, "invalid_token".into()));
}

#[test]
fn five_wrong_passwords_kill_a_code_and_a_stale_epoch_or_expiry_refuses_it() {
    let w = world();
    let (uid, _, _) = w.user();
    let c1 = w.enrolment(&uid, 0);
    for _ in 0..5 {
        assert_eq!(code(enrol(&w.ctx(), &hdrs(&[]), &native(), &obj(json!({ "code": c1, "password": "wrong-password-here" })))), (401, "invalid_credentials".into()));
    }
    assert_eq!(code(enrol(&w.ctx(), &hdrs(&[]), &native(), &obj(json!({ "code": c1, "password": PW })))), (400, "invalid_token".into()), "dead after 5 failures");
    assert_eq!(w.count("SELECT COUNT(*) FROM device_enrolments WHERE claimed_until IS NOT NULL"), 0, "a wrong password releases the lease");
    let c2 = w.enrolment(&uid, 0);
    w.exec("UPDATE users SET device_epoch = 1");
    assert_eq!(code(enrol(&w.ctx(), &hdrs(&[]), &native(), &obj(json!({ "code": c2, "password": PW })))), (400, "invalid_token".into()), "a reset since the code was mailed voids it");
    w.exec("UPDATE users SET device_epoch = 0");
    w.now.set(T0 + 901_000);
    assert_eq!(code(enrol(&w.ctx(), &hdrs(&[]), &native(), &obj(json!({ "code": c2, "password": PW })))), (400, "invalid_token".into()), "15 minutes");
    w.now.set(T0);
    w.exec("UPDATE device_enrolments SET claimed_until = 9999999999999");
    let c3 = w.enrolment(&uid, 0);
    w.exec(&format!("UPDATE device_enrolments SET claimed_until = {} WHERE code_hash = '{}'", T0 + 20_000, sha256(&c3)));
    assert_eq!(code(enrol(&w.ctx(), &hdrs(&[]), &native(), &obj(json!({ "code": c3, "password": PW })))), (409, "in_progress".into()));
}

#[test]
fn enrolment_code_needs_a_session_and_a_known_device_and_is_held_to_the_gap() {
    let w = world();
    let (_, at, dev) = w.user();
    let auth = format!("Bearer {at}");
    let mut out = Outbox::new();
    assert_eq!(code(enrolment_code(&w.ctx(), &hdrs(&[]), &native(), &mut out)).0, 401);
    assert_eq!(code(enrolment_code(&w.ctx(), &hdrs(&[("authorization", &auth)]), &native(), &mut out)), (403, "known_device_required".into()));
    let h = hdrs(&[("authorization", &auth), ("x-device-token", &dev)]);
    assert_eq!(enrolment_code(&w.ctx(), &h, &native(), &mut out).unwrap().status(), 202);
    assert_eq!(out.len(), 1);
    let e = enrolment_code(&w.ctx(), &h, &native(), &mut out).unwrap_err();
    assert_eq!((e.status, e.code.as_str()), (429, "rate_limited"));
    assert!(e.headers.iter().any(|(k, v)| k == "Retry-After" && v == "120"));
}
