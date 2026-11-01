//! Tests for `authbridge.rs`: a signed-in caller as the pool sees it, and the admin guard's refusals mapped to pool errors.
use super::*;
use crate::auth::session::access_token;
use crate::db;
use axum::http::{HeaderName, HeaderValue};
use std::path::Path;

const NOW: i64 = 1_800_000_000_000;

fn world() -> (AuthBridge, Arc<AuthConfig>, tempfile::TempDir) {
    let dir = tempfile::tempdir().unwrap();
    let mut c = db::open(&dir.path().join("t.db")).unwrap();
    db::migrate(&mut c, &Path::new(env!("CARGO_MANIFEST_DIR")).join("../worker/migrations")).unwrap();
    for (id, trust) in [("u1", "teacher"), ("a1", "admin")] {
        c.execute("INSERT INTO users (id, email, display_name, trust, created_at) VALUES (?1, ?2, 'N', ?3, 1)", rusqlite::params![id, format!("{id}@x.org"), trust]).unwrap();
        c.execute("INSERT INTO sessions (id, family_id, user_id, refresh_hash, created_at, auth_at, expires_at) VALUES (?1, ?2, ?3, ?1, 1, 1, ?4)", rusqlite::params![format!("s{id}"), format!("f{id}"), id, NOW + 86_400_000]).unwrap();
    }
    let cfg = Arc::new(AuthConfig::new(&"s".repeat(32), vec!["https://app.example.org".to_string()]).unwrap());
    (AuthBridge::new(c, cfg.clone(), Box::new(|| NOW)), cfg, dir)
}

fn headers(b: &AuthBridge, user: &str, trust: &str) -> HeaderMap {
    let mut h = HeaderMap::new();
    let tok = access_token(&b.ctx(), user, &format!("f{user}"), trust);
    h.append(HeaderName::from_static("authorization"), HeaderValue::from_str(&format!("Bearer {tok}")).unwrap());
    h.append(HeaderName::from_static("x-forwarded-for"), HeaderValue::from_static("203.0.113.5"));
    h
}

#[test]
fn a_bearer_token_names_the_caller_with_the_trust_in_the_database() {
    let (b, _cfg, _d) = world();
    let c = b.caller(&headers(&b, "u1", "admin")).unwrap();
    // The token claims admin; the database says teacher.
    assert_eq!((c.user_id.as_str(), c.trust.as_str(), c.family.as_deref()), ("u1", "teacher", Some("fu1")));
    assert!(b.caller(&HeaderMap::new()).is_none());
}

#[test]
fn the_admin_guard_refuses_a_teacher_and_an_admin_on_an_unknown_device_with_pool_errors() {
    let (b, _cfg, _d) = world();
    let e = b.admin(&headers(&b, "u1", "teacher"), false).unwrap_err();
    assert_eq!((e.status, e.code.as_str()), (403, "forbidden"));
    let e = b.admin(&headers(&b, "a1", "admin"), false).unwrap_err();
    assert_eq!((e.status, e.code.as_str()), (403, "known_device_required"));
    let e = b.admin(&HeaderMap::new(), false).unwrap_err();
    assert_eq!(e.status, 401);
    assert!(e.headers.iter().any(|h| h.0 == "WWW-Authenticate"));
}
