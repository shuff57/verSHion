//! Tests for `account.rs`: me, export and delete over a real migrated database.
use super::*;
use crate::auth::config::AuthConfig;
use crate::auth::crypto::{derive_keys, sha256};
use crate::auth::device::issue_sudo;
use crate::auth::password::{Argon2Params, hash_password, params_json};
use crate::auth::session::login;
use crate::db;
use axum::http::{HeaderName, HeaderValue};
use std::path::Path;
use std::sync::Mutex;

const T0: i64 = 1_800_000_000_000;
const FAST: Argon2Params = Argon2Params { m: 8, t: 1, p: 1 };
const PW: &str = "violet-harbor-tractor";

struct World {
    db: Mutex<Connection>,
    cfg: AuthConfig,
    keys: super::super::crypto::Keys,
    _dir: tempfile::TempDir,
}

fn world() -> World {
    let dir = tempfile::tempdir().unwrap();
    let mut conn = db::open(&dir.path().join("t.db")).unwrap();
    db::migrate(&mut conn, &Path::new(env!("CARGO_MANIFEST_DIR")).join("../worker/migrations")).unwrap();
    let mut cfg = AuthConfig::new(&"s".repeat(32), vec![]).unwrap();
    cfg.argon2 = FAST;
    World { db: Mutex::new(conn), keys: derive_keys(&cfg.secret), cfg, _dir: dir }
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
fn bytes(r: Response) -> Value {
    serde_json::from_slice(&tokio::runtime::Builder::new_current_thread().build().unwrap().block_on(async { axum::body::to_bytes(r.into_body(), 1_000_000).await.unwrap() })).unwrap_or(Value::Null)
}
fn code(r: Result<Response, HttpError>) -> (u16, String) {
    let e = r.unwrap_err();
    (e.status, e.code)
}

impl World {
    fn ctx(&self) -> Ctx<'_> {
        Ctx { db: &self.db, cfg: &self.cfg, keys: &self.keys, now: T0 }
    }
    fn count(&self, sql: &str) -> i64 {
        self.db.lock().unwrap().query_row(sql, [], |r| r.get(0)).unwrap()
    }
    /// Creates and signs in an account; returns (id, access token, device token).
    fn user(&self, email: &str, trust: &str) -> (String, String, String) {
        let id = uuid_v4();
        let phc = hash_password(PW, FAST).unwrap().phc;
        self.db
            .lock()
            .unwrap()
            .execute(
                "INSERT INTO users (id, email, email_verified_at, pw_hash, pw_params, display_name, profile, trust, created_at) VALUES (?1, ?2, 1, ?3, ?4, 'Ms R', '{\"school\":\"PV\"}', ?5, 1)",
                params![id, email, phc, params_json(&FAST), trust],
            )
            .unwrap();
        let b = bytes(login(&self.ctx(), &hdrs(&[]), &native(), &obj(json!({ "email": email, "password": PW }))).unwrap());
        (id, b["access_token"].as_str().unwrap().into(), b["device_token"].as_str().unwrap().into())
    }
}

fn bearer(at: &str) -> String {
    format!("Bearer {at}")
}
fn nothing(_: &Transaction, _: &str, _: i64) -> rusqlite::Result<()> {
    Ok(())
}

#[test]
fn me_shows_the_account_and_its_standing() {
    let w = world();
    let (id, at, _) = w.user("a@x.org", "pending");
    let b = bytes(me(&w.ctx(), &hdrs(&[("authorization", &bearer(&at))])).unwrap());
    assert_eq!((b["id"].as_str(), b["email"].as_str(), b["email_verified"].clone(), b["trust"].as_str()), (Some(id.as_str()), Some("a@x.org"), json!(true), Some("pending")));
    assert_eq!((b["display_name"].as_str(), b["profile"]["school"].as_str(), b["status"]["state"].as_str()), (Some("Ms R"), Some("PV"), Some("pending")));
    assert_eq!(code(me(&w.ctx(), &hdrs(&[]))).0, 401);
    w.db
        .lock()
        .unwrap()
        .execute_batch(&format!("INSERT INTO approval_requests (id, user_id, requested_at, decided_at, decision, reason) VALUES ('r1', '{id}', 1, 1000, 'rejected', 'no evidence')"))
        .unwrap();
    let b = bytes(me(&w.ctx(), &hdrs(&[("authorization", &bearer(&at))])).unwrap());
    assert_eq!((b["status"]["state"].as_str(), b["status"]["reason"].as_str(), b["status"]["can_reapply"].clone()), (Some("rejected"), Some("no evidence"), json!(true)));
    w.db.lock().unwrap().execute("UPDATE users SET trust = 'teacher'", []).unwrap();
    let b = bytes(me(&w.ctx(), &hdrs(&[("authorization", &bearer(&at))])).unwrap());
    assert_eq!(b["status"]["state"], "approved");
}

#[test]
fn export_lists_the_account_sessions_applications_and_the_apps_extra_section() {
    let w = world();
    let (id, at, _) = w.user("a@x.org", "pending");
    w.db
        .lock()
        .unwrap()
        .execute_batch(&format!("INSERT INTO approval_requests (id, user_id, requested_at, note) VALUES ('r1', '{id}', 5, 'reviewer note')"))
        .unwrap();
    let extra = |_: &Connection, uid: &str| -> Map<String, Value> { obj(json!({ "items": [{ "owner": uid }] })) };
    let b = bytes(export(&w.ctx(), &hdrs(&[("authorization", &bearer(&at))]), &extra).unwrap());
    assert_eq!((b["account"]["email"].as_str(), b["account"]["profile"]["school"].as_str()), (Some("a@x.org"), Some("PV")));
    assert_eq!((b["sessions"].as_array().unwrap().len(), b["sessions"][0]["current"].clone()), (1, json!(true)));
    assert_eq!(b["approval"]["requested_at"], 5);
    assert!(!b.to_string().contains("reviewer note"), "never reviewer notes");
    assert_eq!(b["items"][0]["owner"].as_str(), Some(id.as_str()));
}

#[test]
fn delete_needs_the_password_tombstones_the_account_and_ends_everything() {
    let w = world();
    let (id, at, _) = w.user("a@x.org", "pending");
    w.db
        .lock()
        .unwrap()
        .execute_batch(&format!(
            "INSERT INTO approval_requests (id, user_id, requested_at, note, display_name_snapshot) VALUES ('r1', '{id}', 5, 'pii', 'Ms R');
             INSERT INTO email_tokens (id, user_id, kind, token_hash, created_at, expires_at) VALUES ('t1', '{id}', 'reset', 'h', 1, 2);"
        ))
        .unwrap();
    let h = hdrs(&[("authorization", &bearer(&at))]);
    assert_eq!(code(delete_account(&w.ctx(), &h, &native(), &obj(json!({ "password": "wrong-password-here" })), &nothing)), (401, "invalid_credentials".into()));
    assert_eq!(w.count("SELECT COUNT(*) FROM users WHERE deleted_at IS NULL"), 1);
    let called = std::cell::Cell::new(0);
    let hook = |_: &Transaction, uid: &str, _: i64| -> rusqlite::Result<()> {
        assert!(!uid.is_empty());
        called.set(called.get() + 1);
        Ok(())
    };
    let r = delete_account(&w.ctx(), &h, &native(), &obj(json!({ "password": PW })), &hook).unwrap();
    assert_eq!(r.status(), 204);
    assert_eq!(called.get(), 1);
    assert_eq!(w.count("SELECT COUNT(*) FROM users WHERE email IS NULL AND pw_hash IS NULL AND display_name IS NULL AND profile IS NULL AND deleted_at IS NOT NULL AND deleted_email_hmac IS NOT NULL"), 1);
    assert_eq!(w.count("SELECT COUNT(*) FROM sessions") + w.count("SELECT COUNT(*) FROM email_tokens"), 0);
    assert_eq!(w.count("SELECT COUNT(*) FROM approval_requests WHERE note IS NULL AND display_name_snapshot IS NULL AND decision = 'account_deleted'"), 1);
    assert_eq!(w.count("SELECT COUNT(*) FROM auth_audit WHERE action = 'account_deleted' AND prev_trust = 'pending'"), 1);
    assert_eq!(w.count("SELECT COUNT(*) FROM rate_limits WHERE key LIKE 'a:%'"), 0, "the account's counters go with it");
    assert_eq!(code(me(&w.ctx(), &h)).0, 401, "the old access token is dead");
    assert_eq!(code(delete_account(&w.ctx(), &h, &native(), &obj(json!({ "password": PW })), &nothing)).0, 401);
}

#[test]
fn a_failing_cleanup_hook_rolls_the_whole_deletion_back() {
    let w = world();
    let (_, at, _) = w.user("a@x.org", "pending");
    let boom = |_: &Transaction, _: &str, _: i64| -> rusqlite::Result<()> { Err(rusqlite::Error::InvalidQuery) };
    assert_eq!(code(delete_account(&w.ctx(), &hdrs(&[("authorization", &bearer(&at))]), &native(), &obj(json!({ "password": PW })), &boom)).0, 500);
    assert_eq!(w.count("SELECT COUNT(*) FROM users WHERE deleted_at IS NULL AND email = 'a@x.org'"), 1);
    assert_eq!(w.count("SELECT COUNT(*) FROM sessions WHERE revoked_at IS NULL"), 1);
}

#[test]
fn the_last_admin_cannot_delete_themselves_and_an_admin_needs_a_device_and_sudo() {
    let w = world();
    let (id, at, dev) = w.user("root@x.org", "admin");
    let auth = bearer(&at);
    let body = obj(json!({ "password": PW }));
    assert_eq!(code(delete_account(&w.ctx(), &hdrs(&[("authorization", &auth)]), &native(), &body, &nothing)), (403, "known_device_required".into()));
    assert_eq!(code(delete_account(&w.ctx(), &hdrs(&[("authorization", &auth), ("x-device-token", &dev)]), &native(), &body, &nothing)), (403, "sudo_required".into()));
    let fam: String = w.db.lock().unwrap().query_row("SELECT family_id FROM sessions", [], |r| r.get(0)).unwrap();
    let (sudo, _) = issue_sudo(&w.keys, &Principal { user_id: id.clone(), session_family: fam, trust: "admin".into() }, 0, T0);
    let full = hdrs(&[("authorization", &auth), ("x-device-token", &dev), ("x-sudo-token", &sudo)]);
    assert_eq!(code(delete_account(&w.ctx(), &full, &native(), &body, &nothing)), (409, "last_admin".into()));
    assert_eq!(w.count("SELECT COUNT(*) FROM users WHERE deleted_at IS NULL"), 1);
    // With a second active admin the first can go.
    w.db.lock().unwrap().execute("INSERT INTO users (id, email, email_verified_at, trust, created_at) VALUES ('other', 'o@x.org', 1, 'admin', 1)", []).unwrap();
    assert_eq!(delete_account(&w.ctx(), &full, &native(), &body, &nothing).unwrap().status(), 204);
    let _ = sha256("");
}

#[test]
fn deletion_attempts_are_limited_per_account() {
    let w = world();
    let (_, at, _) = w.user("a@x.org", "pending");
    let h = hdrs(&[("authorization", &bearer(&at))]);
    let mut stopped = false;
    for _ in 0..8 {
        stopped |= delete_account(&w.ctx(), &h, &native(), &obj(json!({ "password": "wrong-password-here" })), &nothing).is_err_and(|e| e.status == 429);
    }
    assert!(stopped, "five an hour");
}

#[test]
fn a_rejected_applicant_reapplies_after_the_cooldown_with_a_new_row() {
    let w = world();
    let (id, at, _) = w.user("a@x.org", "pending");
    let h = hdrs(&[("authorization", &bearer(&at))]);
    let profile = obj(json!({ "display_name": "Ms Rivera", "school": "New School", "adult_school_employee": true }));
    assert_eq!(code(reapply(&w.ctx(), &h, &profile)), (409, "not_eligible".into()), "nothing to reapply to");
    w.db.lock().unwrap().execute_batch(&format!("INSERT INTO approval_requests (id, user_id, requested_at, decided_at, decision) VALUES ('r1', '{id}', 1, {}, 'rejected')", T0 - 86_400_000)).unwrap();
    let e = reapply(&w.ctx(), &h, &profile).unwrap_err();
    assert_eq!((e.status, e.code.as_str(), e.extra.get("reapply_after")), (429, "cooldown", Some(&json!(T0 - 86_400_000 + 7 * 86_400_000))));
    assert!(e.headers.iter().any(|(k, v)| k == "Retry-After" && v == "518400"));
    w.db.lock().unwrap().execute("UPDATE approval_requests SET decided_at = 1000", []).unwrap();
    let r = reapply(&w.ctx(), &h, &profile).unwrap();
    assert_eq!(r.status(), 202);
    assert_eq!(w.count("SELECT COUNT(*) FROM approval_requests"), 2);
    assert_eq!(w.count("SELECT COUNT(*) FROM approval_requests WHERE decided_at IS NULL AND display_name_snapshot = 'Ms Rivera'"), 1);
    assert_eq!(w.count("SELECT COUNT(*) FROM auth_audit WHERE action = 'reapply' AND actor_kind = 'user'"), 1);
    assert_eq!(code(reapply(&w.ctx(), &h, &profile)), (409, "not_eligible".into()), "one open application at a time");
    assert_eq!(code(reapply(&w.ctx(), &h, &obj(json!({ "display_name": "x" })))).0, 400);
}
