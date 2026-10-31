//! Tests for `signup.rs`: sign-up, resend and forgot over a real migrated database.
use super::*;
use crate::auth::config::AuthConfig;
use crate::auth::crypto::{Keys, derive_keys};
use crate::auth::password::Argon2Params;
use crate::db;
use axum::http::{HeaderMap, HeaderName, HeaderValue};
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

fn ip(v: &str) -> HeaderMap {
    let mut m = HeaderMap::new();
    m.append(HeaderName::from_static("x-forwarded-for"), HeaderValue::from_str(v).unwrap());
    m
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
    fn signup(&self, email: &str, extra: Value, out: &mut Outbox) -> Result<Response, HttpError> {
        let mut b = json!({ "email": email, "password": PW, "display_name": "  Ms   Rivera ", "school": "Pleasant Valley", "adult_school_employee": true });
        if let (Some(o), Some(e)) = (b.as_object_mut(), extra.as_object()) {
            for (k, v) in e {
                o.insert(k.clone(), v.clone());
            }
        }
        signup(&self.ctx(), &ip("10.0.0.1"), b.as_object().unwrap(), out)
    }
}

fn obj(v: Value) -> Map<String, Value> {
    v.as_object().unwrap().clone()
}

#[test]
fn iso8601_matches_to_iso_string() {
    assert_eq!(iso8601(0), "1970-01-01T00:00:00.000Z");
    assert_eq!(iso8601(1_800_000_000_123), "2027-01-15T08:00:00.123Z");
    assert_eq!(iso8601(951_782_400_000), "2000-02-29T00:00:00.000Z");
}

#[test]
fn signup_creates_an_unverified_account_stores_only_a_hash_and_mails_a_verify_link() {
    let w = world();
    let mut out = Outbox::new();
    let r = w.signup("Ms.Rivera@School.org", json!({}), &mut out).unwrap();
    assert_eq!(r.status(), 202);
    assert_eq!(w.count("SELECT COUNT(*) FROM users WHERE email = 'ms.rivera@school.org' AND trust = 'unverified' AND email_verified_at IS NULL"), 1);
    let (name, profile): (String, String) = w.db.lock().unwrap().query_row("SELECT display_name, profile FROM users", [], |r| Ok((r.get(0)?, r.get(1)?))).unwrap();
    assert_eq!(name, "Ms Rivera", "whitespace collapsed");
    let p: Value = serde_json::from_str(&profile).unwrap();
    assert_eq!((p["school"].as_str(), p["adult_school_employee_at"].as_str()), (Some("Pleasant Valley"), Some("2027-01-15T08:00:00.000Z")));
    assert_eq!(out.len(), 1);
    assert_eq!((out[0].kind, out[0].to.as_str()), (MailKind::Verify, "ms.rivera@school.org"));
    let token = out[0].token.clone().unwrap();
    assert_eq!(w.count(&format!("SELECT COUNT(*) FROM email_tokens WHERE kind = 'verify' AND token_hash = '{}'", sha256(&token))), 1);
    assert_eq!(w.count(&format!("SELECT COUNT(*) FROM email_tokens WHERE token_hash = '{token}'")), 0, "the token itself is never stored");
    assert_eq!(w.count(&format!("SELECT COUNT(*) FROM users WHERE pw_hash LIKE '%{PW}%'")), 0);
}

#[test]
fn signing_up_again_with_an_existing_address_looks_identical_but_mails_the_owner_instead() {
    let w = world();
    let mut out = Outbox::new();
    w.signup("a@x.org", json!({}), &mut out).unwrap();
    let mut again = Outbox::new();
    let r = w.signup("A@x.org", json!({ "password": "another-long-password" }), &mut again).unwrap();
    assert_eq!(r.status(), 202);
    assert_eq!(w.count("SELECT COUNT(*) FROM users"), 1);
    assert_eq!(w.count("SELECT COUNT(*) FROM email_tokens"), 1, "no second link for an existing account");
    assert_eq!((again[0].kind, again[0].account_verified), (MailKind::SignupExisting, Some(false)));
    w.db.lock().unwrap().execute("UPDATE users SET email_verified_at = 1", []).unwrap();
    let mut third = Outbox::new();
    w.signup("a@x.org", json!({}), &mut third).unwrap();
    assert_eq!(third[0].account_verified, Some(true));
}

#[test]
fn signup_rejects_bad_input_before_touching_the_database() {
    let w = world();
    let mut out = Outbox::new();
    let e = w.signup("a@x.org", json!({ "adult_school_employee": false }), &mut out).unwrap_err();
    assert_eq!((e.status, e.code.as_str(), e.extra.get("field")), (400, "invalid_request", Some(&json!("adult_school_employee"))));
    assert_eq!(e.extra.get("reason"), Some(&json!("must_confirm")));
    let e = w.signup("a@x.org", json!({ "display_name": "\u{202e}" }), &mut out).unwrap_err();
    assert_eq!(e.extra.get("field"), Some(&json!("display_name")));
    let e = w.signup("a@x.org", json!({ "school": "" }), &mut out).unwrap_err();
    assert_eq!(e.extra.get("field"), Some(&json!("school")));
    let e = w.signup("a@x.org", json!({ "password": "short" }), &mut out).unwrap_err();
    assert_eq!((e.status, e.code.as_str()), (400, "weak_password"));
    let e = w.signup("a@x.org", json!({ "password": "a@x.org-a@x.org" }), &mut out).unwrap_err();
    assert_eq!(e.code, "weak_password", "built on the email");
    assert_eq!(w.signup("not-an-email", json!({}), &mut out).unwrap_err().status, 400);
    assert_eq!(signup(&w.ctx(), &HeaderMap::new(), &obj(json!({ "email": "a@x.org", "password": PW, "display_name": "A", "school": "S", "adult_school_employee": true })), &mut out).unwrap_err().code, "client_ip_required");
    assert_eq!(w.count("SELECT COUNT(*) FROM users"), 0);
    assert!(out.is_empty());
}

fn make_user(w: &World, email: &str, verified: bool) {
    w.db
        .lock()
        .unwrap()
        .execute("INSERT INTO users (id, email, email_verified_at, trust, created_at) VALUES (?1, ?2, ?3, 'pending', 1)", params![uuid_v4(), email, if verified { Some(1) } else { None }])
        .unwrap();
}

fn mail_for(w: &World, f: fn(&Ctx, &HeaderMap, &Map<String, Value>, &mut Outbox) -> Result<Response, HttpError>, email: &str) -> (Response, Outbox) {
    let mut out = Outbox::new();
    let r = f(&w.ctx(), &ip("10.0.0.1"), &obj(json!({ "email": email })), &mut out).unwrap();
    (r, out)
}

#[test]
fn resend_and_forgot_answer_202_for_everyone_and_mail_only_real_eligible_accounts() {
    let w = world();
    make_user(&w, "unconfirmed@x.org", false);
    make_user(&w, "active@x.org", true);
    for (email, resend_mail, forgot_mail) in [("unconfirmed@x.org", true, true), ("active@x.org", false, true), ("nobody@x.org", false, false)] {
        let (r, o) = mail_for(&w, resend, email);
        assert_eq!((r.status().as_u16(), !o.is_empty()), (202, resend_mail), "resend {email}");
        w.advance(200_000);
        let (r, o) = mail_for(&w, forgot, email);
        assert_eq!((r.status().as_u16(), !o.is_empty()), (202, forgot_mail), "forgot {email}");
        w.advance(200_000);
    }
    assert_eq!(w.count("SELECT COUNT(*) FROM email_tokens WHERE kind = 'verify'"), 1);
    assert_eq!(w.count("SELECT COUNT(*) FROM email_tokens WHERE kind = 'reset'"), 2);
}

#[test]
fn a_second_mail_inside_the_gap_is_silently_skipped_and_the_daily_ceiling_holds() {
    let w = world();
    make_user(&w, "a@x.org", true);
    assert_eq!(mail_for(&w, forgot, "a@x.org").1.len(), 1);
    w.advance(60_000);
    let (r, o) = mail_for(&w, forgot, "a@x.org");
    assert_eq!((r.status().as_u16(), o.len()), (202, 0), "inside the 2-minute gap: same reply, no mail");
    // The ceiling: ten reset links already inside one link lifetime and the next request mails nothing.
    w.advance(300_000);
    let uid: String = w.db.lock().unwrap().query_row("SELECT id FROM users", [], |r| r.get(0)).unwrap();
    for i in 0..9 {
        let at = w.now.get() - 200_000 - i * 1000;
        w.db.lock().unwrap().execute("INSERT INTO email_tokens (id, user_id, kind, token_hash, created_at, expires_at) VALUES (?1, ?2, 'reset', ?1, ?3, ?4)", params![uuid_v4(), uid, at, at + 3_600_000]).unwrap();
    }
    let (r, o) = mail_for(&w, forgot, "a@x.org");
    assert_eq!((r.status().as_u16(), o.len()), (202, 0), "at the ceiling: same reply, no mail");
}

#[test]
fn only_the_newest_three_links_stay_valid() {
    let w = world();
    make_user(&w, "a@x.org", true);
    for i in 0..5 {
        let mut out = Outbox::new();
        forgot(&w.ctx(), &ip(&format!("10.0.{i}.1")), &obj(json!({ "email": "a@x.org" })), &mut out).unwrap();
        assert_eq!(out.len(), 1);
        w.advance(121_000);
    }
    // reset_ttl is an hour, so all five are inside one link lifetime and the ceiling (10) is not hit.
    assert_eq!(w.count("SELECT COUNT(*) FROM email_tokens WHERE kind = 'reset' AND used_at IS NULL"), 3);
    assert_eq!(w.count("SELECT COUNT(*) FROM email_tokens WHERE kind = 'reset'"), 5);
}

#[test]
fn a_mail_request_is_limited_per_ip_and_address_and_fails_closed_without_an_ip() {
    let w = world();
    make_user(&w, "a@x.org", true);
    let mut out = Outbox::new();
    let mut limited = false;
    for _ in 0..20 {
        if let Err(e) = forgot(&w.ctx(), &ip("10.0.0.1"), &obj(json!({ "email": "a@x.org" })), &mut out) {
            assert_eq!(e.status, 429);
            limited = true;
            break;
        }
    }
    assert!(limited, "a flood of requests for one address from one network gets a 429");
    let mut out = Outbox::new();
    assert_eq!(forgot(&w.ctx(), &HeaderMap::new(), &obj(json!({ "email": "a@x.org" })), &mut out).unwrap_err().code, "client_ip_required");
    assert_eq!(forgot(&w.ctx(), &ip("10.0.0.2"), &obj(json!({ "email": "nope" })), &mut out).unwrap_err().status, 400);
}
