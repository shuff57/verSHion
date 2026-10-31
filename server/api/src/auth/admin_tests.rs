//! Tests for `admin.rs`: the guard and the decisions on applications.
use super::*;
use crate::auth::config::AuthConfig;
use crate::auth::crypto::{Keys, derive_keys};
use crate::auth::device::issue_sudo;
use crate::auth::http::ClientMode;
use crate::auth::password::{Argon2Params, hash_password, params_json};
use crate::auth::session::login;
use crate::db;
use axum::http::{HeaderName, HeaderValue};
use std::path::Path;
use std::sync::Mutex;

const T0: i64 = 1_800_000_000_000;
const FAST: Argon2Params = Argon2Params { m: 8, t: 1, p: 1 };
const PW: &str = "violet-harbor-tractor";

pub(crate) struct World {
    pub(crate) db: Mutex<Connection>,
    pub(crate) cfg: AuthConfig,
    pub(crate) keys: Keys,
    pub(crate) _dir: tempfile::TempDir,
}

pub(crate) fn world() -> World {
    let dir = tempfile::tempdir().unwrap();
    let mut conn = db::open(&dir.path().join("t.db")).unwrap();
    db::migrate(&mut conn, &Path::new(env!("CARGO_MANIFEST_DIR")).join("../worker/migrations")).unwrap();
    let mut cfg = AuthConfig::new(&"s".repeat(32), vec![]).unwrap();
    cfg.argon2 = FAST;
    World { db: Mutex::new(conn), keys: derive_keys(&cfg.secret), cfg, _dir: dir }
}

pub(crate) fn native() -> ClientInfo {
    ClientInfo { mode: ClientMode::Native, origin: None }
}
pub(crate) fn obj(v: Value) -> Map<String, Value> {
    v.as_object().unwrap().clone()
}
pub(crate) fn hdrs(pairs: &[(&str, &str)]) -> HeaderMap {
    let mut m = HeaderMap::new();
    m.append(HeaderName::from_static("x-forwarded-for"), HeaderValue::from_static("10.0.0.1"));
    for (k, v) in pairs {
        m.append(HeaderName::from_bytes(k.as_bytes()).unwrap(), HeaderValue::from_str(v).unwrap());
    }
    m
}
pub(crate) fn body_of(r: Response) -> Value {
    serde_json::from_slice(&tokio::runtime::Builder::new_current_thread().build().unwrap().block_on(async { axum::body::to_bytes(r.into_body(), 1_000_000).await.unwrap() })).unwrap_or(Value::Null)
}
pub(crate) fn code(r: Result<Response, HttpError>) -> (u16, String) {
    let e = r.unwrap_err();
    (e.status, e.code)
}

pub(crate) struct Admin {
    pub(crate) id: String,
    pub(crate) full: HeaderMap,
    pub(crate) read_only: HeaderMap,
}

impl World {
    pub(crate) fn ctx(&self) -> Ctx<'_> {
        Ctx { db: &self.db, cfg: &self.cfg, keys: &self.keys, now: T0 }
    }
    pub(crate) fn count(&self, sql: &str) -> i64 {
        self.db.lock().unwrap().query_row(sql, [], |r| r.get(0)).unwrap()
    }
    pub(crate) fn account(&self, email: &str, trust: &str) -> String {
        let id = uuid_v4();
        let phc = hash_password(PW, FAST).unwrap().phc;
        self.db
            .lock()
            .unwrap()
            .execute("INSERT INTO users (id, email, email_verified_at, pw_hash, pw_params, trust, created_at) VALUES (?1, ?2, 1, ?3, ?4, ?5, 1)", params![id, email, phc, params_json(&FAST), trust])
            .unwrap();
        id
    }
    pub(crate) fn admin(&self, email: &str) -> Admin {
        let id = self.account(email, "admin");
        let b = body_of(login(&self.ctx(), &hdrs(&[]), &native(), &obj(json!({ "email": email, "password": PW }))).unwrap());
        let auth = format!("Bearer {}", b["access_token"].as_str().unwrap());
        let dev = b["device_token"].as_str().unwrap().to_string();
        let fam: String = self.db.lock().unwrap().query_row("SELECT s.family_id FROM sessions s WHERE s.user_id = ?1", [&id], |r| r.get(0)).unwrap();
        let (sudo, _) = issue_sudo(&self.keys, &Principal { user_id: id.clone(), session_family: fam, trust: "admin".into() }, 0, T0);
        Admin { full: hdrs(&[("authorization", &auth), ("x-device-token", &dev), ("x-sudo-token", &sudo)]), read_only: hdrs(&[("authorization", &auth), ("x-device-token", &dev)]), id }
    }
    /// A pending applicant with an open application; returns (user id, request id).
    pub(crate) fn applicant(&self, email: &str) -> (String, String) {
        let id = self.account(email, "pending");
        let req = uuid_v4();
        self.db
            .lock()
            .unwrap()
            .execute("INSERT INTO approval_requests (id, user_id, requested_at, email_domain) VALUES (?1, ?2, 1, substr(?3, instr(?3, '@') + 1))", params![req, id, email])
            .unwrap();
        (id, req)
    }
}

pub(crate) fn approve_body(user: &str, req: &str) -> Map<String, Value> {
    obj(json!({ "user_id": user, "request_id": req, "note": "checked", "evidence": "directory", "evidence_note": "on the staff list" }))
}

#[test]
pub(crate) fn the_guard_needs_an_admin_a_known_device_and_for_writes_a_sudo_token() {
    let w = world();
    let a = w.admin("root@x.org");
    let (u, r) = w.applicant("t@x.org");
    let body = approve_body(&u, &r);
    let mut out = Outbox::new();
    assert_eq!(code(approve(&w.ctx(), &hdrs(&[]), &native(), &body, &mut out)).0, 401);
    assert_eq!(code(approve(&w.ctx(), &a.read_only, &native(), &body, &mut out)), (403, "sudo_required".into()));
    let mut no_device = a.full.clone();
    no_device.remove("x-device-token");
    assert_eq!(code(approve(&w.ctx(), &no_device, &native(), &body, &mut out)), (403, "known_device_required".into()));
    // A teacher is not an admin.
    let t = w.account("teacher@x.org", "teacher");
    let b = body_of(login(&w.ctx(), &hdrs(&[]), &native(), &obj(json!({ "email": "teacher@x.org", "password": PW }))).unwrap());
    let th = hdrs(&[("authorization", &format!("Bearer {}", b["access_token"].as_str().unwrap()))]);
    assert_eq!(code(approve(&w.ctx(), &th, &native(), &body, &mut out)), (403, "forbidden".into()));
    let _ = t;
    assert_eq!(w.count("SELECT COUNT(*) FROM approval_requests WHERE decided_at IS NOT NULL"), 0);
}

#[test]
pub(crate) fn approving_promotes_the_applicant_audits_counts_the_domain_and_mails_them() {
    let w = world();
    let a = w.admin("root@x.org");
    let (u, r) = w.applicant("t@school.org");
    let mut out = Outbox::new();
    let res = approve(&w.ctx(), &a.full, &native(), &approve_body(&u, &r), &mut out).unwrap();
    assert_eq!(body_of(res)["decision"], "approved");
    assert_eq!(w.count("SELECT COUNT(*) FROM users WHERE trust = 'teacher' AND email = 't@school.org'"), 1);
    assert_eq!(w.count(&format!("SELECT COUNT(*) FROM approval_requests WHERE id = '{r}' AND decision = 'approved' AND decided_by = '{}' AND evidence = 'directory'", a.id)), 1);
    assert_eq!(w.count("SELECT COUNT(*) FROM domain_decisions WHERE domain = 'school.org' AND approved = 1 AND rejected = 0"), 1);
    assert_eq!(w.count("SELECT COUNT(*) FROM auth_audit WHERE action = 'approve' AND prev_trust = 'pending' AND new_trust = 'teacher' AND actor_kind = 'admin'"), 1);
    assert_eq!((out.len(), out[0].kind, out[0].to.as_str()), (1, MailKind::ApplicationApproved, "t@school.org"));
    assert_eq!(code(approve(&w.ctx(), &a.full, &native(), &approve_body(&u, &r), &mut out)), (409, "not_pending".into()), "a decision is made once");
}

#[test]
pub(crate) fn approval_input_is_validated_and_an_admin_cannot_decide_on_themselves() {
    let w = world();
    let a = w.admin("root@x.org");
    let (u, r) = w.applicant("t@x.org");
    let mut out = Outbox::new();
    let mut b = approve_body(&u, &r);
    b.insert("evidence".into(), json!("name_match"));
    let e = approve(&w.ctx(), &a.full, &native(), &b, &mut out).unwrap_err();
    assert_eq!((e.status, e.extra.get("field")), (400, Some(&json!("evidence"))), "a name match is never evidence");
    let mut b = approve_body(&u, &r);
    b.remove("note");
    assert_eq!(approve(&w.ctx(), &a.full, &native(), &b, &mut out).unwrap_err().extra.get("field"), Some(&json!("note")));
    let mut b = approve_body(&u, &r);
    b.insert("request_id".into(), json!("not-a-uuid"));
    assert_eq!(approve(&w.ctx(), &a.full, &native(), &b, &mut out).unwrap_err().extra.get("field"), Some(&json!("request_id")));
    assert_eq!(code(approve(&w.ctx(), &a.full, &native(), &approve_body(&a.id, &r), &mut out)), (403, "self_action".into()));
    // A stale request id (for a different application of the same user) changes nothing.
    assert_eq!(code(approve(&w.ctx(), &a.full, &native(), &approve_body(&u, &uuid_v4()), &mut out)), (409, "not_pending".into()));
    assert_eq!(w.count("SELECT COUNT(*) FROM users WHERE trust = 'teacher'"), 0);
}

#[test]
pub(crate) fn rejecting_keeps_the_applicant_pending_with_a_reason_and_mails_it() {
    let w = world();
    let a = w.admin("root@x.org");
    let (u, r) = w.applicant("t@x.org");
    let mut out = Outbox::new();
    let body = obj(json!({ "user_id": u, "request_id": r, "note": "internal", "reason": "not on the staff list" }));
    assert_eq!(body_of(reject(&w.ctx(), &a.full, &native(), &body, &mut out).unwrap())["decision"], "rejected");
    assert_eq!(w.count("SELECT COUNT(*) FROM users WHERE trust = 'pending'"), 1, "the applicant stays pending");
    assert_eq!(w.count("SELECT COUNT(*) FROM domain_decisions WHERE rejected = 1"), 1);
    assert_eq!((out[0].kind, out[0].reason.as_deref()), (MailKind::ApplicationRejected, Some("not on the staff list")));
    assert_eq!(code(reject(&w.ctx(), &a.full, &native(), &body, &mut out)), (409, "not_pending".into()));
}

#[test]
pub(crate) fn bulk_reject_decides_each_item_on_its_own() {
    let w = world();
    let a = w.admin("root@x.org");
    let (u1, r1) = w.applicant("a@x.org");
    let (u2, r2) = w.applicant("b@x.org");
    let mut out = Outbox::new();
    let body = obj(json!({ "note": "n", "reason": "r", "items": [
        { "user_id": u1, "request_id": r1 }, { "user_id": u2, "request_id": uuid_v4() }, { "user_id": a.id, "request_id": r2 } ] }));
    let res = body_of(reject_bulk(&w.ctx(), &a.full, &native(), &body, &mut out).unwrap());
    let statuses: Vec<&str> = res["results"].as_array().unwrap().iter().map(|x| x["status"].as_str().unwrap()).collect();
    assert_eq!(statuses, vec!["rejected", "not_pending", "not_pending"]);
    assert_eq!(out.len(), 1);
    let too_many: Vec<Value> = (0..51).map(|_| json!({ "user_id": uuid_v4(), "request_id": uuid_v4() })).collect();
    let e = reject_bulk(&w.ctx(), &a.full, &native(), &obj(json!({ "note": "n", "reason": "r", "items": too_many })), &mut out).unwrap_err();
    assert_eq!((e.status, e.extra.get("max")), (400, Some(&json!(50))));
    assert_eq!(reject_bulk(&w.ctx(), &a.full, &native(), &obj(json!({ "note": "n", "reason": "r", "items": [] })), &mut out).unwrap_err().status, 400);
}

#[test]
pub(crate) fn revoking_returns_a_teacher_to_pending_ends_their_sessions_and_records_a_revoked_decision() {
    let w = world();
    let a = w.admin("root@x.org");
    let t = w.account("t@school.org", "teacher");
    login(&w.ctx(), &hdrs(&[]), &native(), &obj(json!({ "email": "t@school.org", "password": PW }))).unwrap();
    let mut out = Outbox::new();
    let body = obj(json!({ "user_id": t, "note": "internal", "reason": "left the school" }));
    assert_eq!(body_of(revoke(&w.ctx(), &a.full, &native(), &body, &mut out).unwrap())["ok"], true);
    assert_eq!(w.count(&format!("SELECT COUNT(*) FROM users WHERE id = '{t}' AND trust = 'pending' AND device_epoch = 1")), 1);
    assert_eq!(w.count(&format!("SELECT COUNT(*) FROM sessions WHERE user_id = '{t}' AND revoked_at IS NULL")), 0);
    assert_eq!(w.count(&format!("SELECT COUNT(*) FROM approval_requests WHERE user_id = '{t}' AND decision = 'revoked' AND reason = 'left the school'")), 1);
    assert_eq!(w.count("SELECT COUNT(*) FROM auth_audit WHERE action = 'revoke' AND prev_trust = 'teacher' AND new_trust = 'pending'"), 1);
    assert_eq!(out[0].kind, MailKind::ApprovalRevoked);
    assert_eq!(code(revoke(&w.ctx(), &a.full, &native(), &body, &mut out)), (409, "not_approved".into()), "only an approved account can be revoked");
    assert_eq!(code(revoke(&w.ctx(), &a.full, &native(), &obj(json!({ "user_id": a.id, "note": "n", "reason": "r" })), &mut out)), (403, "self_action".into()));
}
