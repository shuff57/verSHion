//! Tests for `maintenance.rs`: purge, tombstone lookup and the bootstrap.
use super::*;
use crate::auth::crypto::derive_keys;
use crate::db;
use std::path::Path;

const NOW: i64 = 1_800_000_000_000;
const DAY_MS: i64 = 86_400_000;

fn open() -> (Connection, tempfile::TempDir) {
    let dir = tempfile::tempdir().unwrap();
    let mut conn = db::open(&dir.path().join("t.db")).unwrap();
    db::migrate(&mut conn, &Path::new(env!("CARGO_MANIFEST_DIR")).join("../worker/migrations")).unwrap();
    (conn, dir)
}
fn cfg() -> AuthConfig {
    AuthConfig::new(&"s".repeat(32), vec![]).unwrap()
}
fn count(c: &Connection, sql: &str) -> i64 {
    c.query_row(sql, [], |r| r.get(0)).unwrap()
}
fn user(c: &Connection, id: &str, email: &str, trust: &str, verified: bool, pw: bool, created: i64) {
    c.execute(
        "INSERT INTO users (id, email, email_verified_at, pw_hash, trust, created_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        params![id, email, if verified { Some(1) } else { None }, if pw { Some("phc") } else { None }, trust, created],
    )
    .unwrap();
}

#[test]
fn purge_removes_only_what_is_expired_and_keeps_a_days_grace_for_the_caps() {
    let (mut c, _d) = open();
    user(&c, "u1", "a@x.org", "pending", true, true, NOW);
    c.execute_batch(&format!(
        "INSERT INTO rate_limits (key, window_start, count, expires_at) VALUES ('old', 0, 1, {}), ('live', 0, 1, {});
         INSERT INTO sessions (id, family_id, user_id, refresh_hash, created_at, auth_at, expires_at) VALUES ('s1', 'f1', 'u1', 'h1', 1, 1, {}), ('s2', 'f2', 'u1', 'h2', 1, 1, {});
         INSERT INTO email_tokens (id, user_id, kind, token_hash, created_at, expires_at) VALUES
           ('t1', 'u1', 'reset', 'a', {}, {}), ('t2', 'u1', 'reset', 'b', {}, {}), ('t3', 'u1', 'reset', 'c', {}, {});",
        NOW - 1,
        NOW + 1000,
        NOW - 1,
        NOW + 1000,
        NOW - 2 * DAY_MS,
        NOW - DAY_MS - 1000, // expired and older than a day: goes
        NOW - 3600_000,
        NOW - 1000, // expired but under a day old: kept for the daily ceiling
        NOW - 2 * DAY_MS,
        NOW + DAY_MS, // not expired: kept
    ))
    .unwrap();
    let r = purge_expired(&mut c, &cfg(), NOW).unwrap();
    assert_eq!((r.rate_limits, r.sessions, r.email_tokens), (1, 1, 1));
    assert_eq!(count(&c, "SELECT COUNT(*) FROM email_tokens"), 2);
    assert_eq!(count(&c, "SELECT COUNT(*) FROM sessions WHERE id = 's2'"), 1);
}

#[test]
fn purge_drops_tombstone_hashes_when_due_and_accounts_that_never_confirmed() {
    let (mut c, _d) = open();
    c.execute_batch(&format!(
        "INSERT INTO users (id, trust, created_at, deleted_at, deleted_email_hmac, deleted_email_hmac_expires_at) VALUES ('gone', 'pending', 1, 2, 'h1', {}), ('keep', 'pending', 1, 2, 'h2', {});",
        NOW - 1,
        NOW + 1000
    ))
    .unwrap();
    user(&c, "stale", "stale@x.org", "unverified", false, true, NOW - 8 * DAY_MS);
    user(&c, "fresh", "fresh@x.org", "unverified", false, true, NOW - DAY_MS);
    user(&c, "active", "active@x.org", "pending", true, true, NOW - 30 * DAY_MS);
    c.execute("INSERT INTO email_tokens (id, user_id, kind, token_hash, created_at, expires_at) VALUES ('t', 'stale', 'verify', 'x', ?1, ?2)", params![NOW, NOW + 1000]).unwrap();
    c.execute("INSERT INTO approval_requests (id, user_id, requested_at) VALUES ('r', 'stale', 1)", []).unwrap();
    let r = purge_expired(&mut c, &cfg(), NOW).unwrap();
    assert_eq!((r.tombstone_hashes, r.unverified_accounts), (1, 1));
    assert_eq!(count(&c, "SELECT COUNT(*) FROM users WHERE id IN ('stale')"), 0);
    assert_eq!(count(&c, "SELECT COUNT(*) FROM users WHERE id IN ('fresh', 'active')"), 2);
    assert_eq!(count(&c, "SELECT COUNT(*) FROM email_tokens WHERE user_id = 'stale'") + count(&c, "SELECT COUNT(*) FROM approval_requests"), 0);
    assert_eq!(count(&c, "SELECT COUNT(*) FROM users WHERE id = 'gone' AND deleted_email_hmac IS NULL"), 1);
    assert_eq!(count(&c, "SELECT COUNT(*) FROM users WHERE id = 'keep' AND deleted_email_hmac = 'h2'"), 1);
    let mut off = cfg();
    off.purge_unverified_after = 0;
    user(&c, "stale2", "stale2@x.org", "unverified", false, true, 1);
    assert_eq!(purge_expired(&mut c, &off, NOW).unwrap().unverified_accounts, 0, "zero turns the purge off");
}

#[test]
fn a_tombstone_is_found_by_email_until_it_expires() {
    let (c, _d) = open();
    let k = derive_keys(&"s".repeat(32));
    c.execute("INSERT INTO users (id, trust, created_at, deleted_at, deleted_email_hmac, deleted_email_hmac_expires_at) VALUES ('gone', 'teacher', 1, 2, ?1, ?2)", params![hmac(&k.email, "old@x.org"), NOW + 1000]).unwrap();
    assert_eq!(find_tombstone_by_email(&c, &k, "Old@X.org", NOW).as_deref(), Some("gone"));
    assert_eq!(find_tombstone_by_email(&c, &k, "other@x.org", NOW), None);
    assert_eq!(find_tombstone_by_email(&c, &k, "old@x.org", NOW + 2000), None, "past its time");
    assert_eq!(find_tombstone_by_email(&c, &k, "not an email", NOW), None);
}

#[test]
fn make_admin_promotes_an_eligible_account_audits_it_and_is_idempotent() {
    let (mut c, _d) = open();
    user(&c, "t", "t@x.org", "teacher", true, true, 1);
    user(&c, "unconfirmed", "u@x.org", "teacher", false, true, 1);
    user(&c, "nopass", "n@x.org", "teacher", true, false, 1);
    user(&c, "student", "s@x.org", "unverified", true, true, 1);
    c.execute("INSERT INTO approval_requests (id, user_id, requested_at) VALUES ('r', 't', 1)", []).unwrap();
    let o = make_admin(&mut c, "T@x.org", "admin", &["teacher"], NOW).unwrap().unwrap();
    assert_eq!((o.trust.as_str(), o.suspended), ("admin", false));
    assert_eq!(count(&c, "SELECT COUNT(*) FROM auth_audit WHERE action = 'make_admin' AND prev_trust = 'teacher' AND new_trust = 'admin' AND actor_kind = 'bootstrap'"), 1);
    assert_eq!(count(&c, "SELECT COUNT(*) FROM approval_requests WHERE decision = 'bootstrap'"), 1);
    make_admin(&mut c, "t@x.org", "admin", &["teacher"], NOW).unwrap();
    assert_eq!(count(&c, "SELECT COUNT(*) FROM auth_audit"), 1, "a second run changes nothing");
    for (e, id) in [("u@x.org", "unconfirmed"), ("n@x.org", "nopass"), ("s@x.org", "student")] {
        make_admin(&mut c, e, "admin", &["teacher"], NOW).unwrap();
        assert_eq!(count(&c, &format!("SELECT COUNT(*) FROM users WHERE id = '{id}' AND trust = 'admin'")), 0, "{id} is not eligible");
    }
    assert!(make_admin(&mut c, "bad email", "admin", &["teacher"], NOW).is_err());
    assert!(make_admin(&mut c, "t@x.org", "ad min", &["teacher"], NOW).is_err());
    assert!(make_admin(&mut c, "t@x.org", "admin", &["x'; DROP TABLE users;--"], NOW).is_err());
}

#[test]
fn make_admin_recovers_a_suspended_admin() {
    let (mut c, _d) = open();
    user(&c, "a", "a@x.org", "admin", true, true, 1);
    c.execute("UPDATE users SET suspended_at = 5, suspension_reason = 'x'", []).unwrap();
    let o = make_admin(&mut c, "a@x.org", "admin", &["teacher"], NOW).unwrap().unwrap();
    assert!(!o.suspended);
    assert_eq!(count(&c, "SELECT COUNT(*) FROM auth_audit WHERE action = 'restore_admin'"), 1);
}

#[test]
fn demote_admin_never_removes_the_last_active_admin_and_ends_sessions() {
    let (mut c, _d) = open();
    user(&c, "a", "a@x.org", "admin", true, true, 1);
    let o = demote_admin(&mut c, "a@x.org", "teacher", NOW).unwrap().unwrap();
    assert_eq!(o.trust, "admin", "the only admin stays");
    user(&c, "b", "b@x.org", "admin", true, true, 1);
    c.execute("INSERT INTO sessions (id, family_id, user_id, refresh_hash, created_at, auth_at, expires_at) VALUES ('s', 'f', 'a', 'h', 1, 1, 9999999999999)", []).unwrap();
    let o = demote_admin(&mut c, "a@x.org", "teacher", NOW).unwrap().unwrap();
    assert_eq!(o.trust, "teacher");
    assert_eq!(count(&c, "SELECT COUNT(*) FROM sessions WHERE revoked_at IS NULL"), 0);
    assert_eq!(count(&c, "SELECT COUNT(*) FROM users WHERE id = 'a' AND device_epoch = 1"), 1);
    assert_eq!(count(&c, "SELECT COUNT(*) FROM auth_audit WHERE action = 'demote_admin' AND prev_trust = 'admin' AND new_trust = 'teacher'"), 1);
    assert_eq!(demote_admin(&mut c, "b@x.org", "teacher", NOW).unwrap().unwrap().trust, "admin", "now b is the last one");
}
