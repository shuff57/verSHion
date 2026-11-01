//! Tests for `gate.rs`: the order of the Worker's checks before a handler runs, and the pool-key pin and verdict.
use super::*;
use crate::db;
use crate::pool::sqlfrag::DAY_MS;
use rusqlite::Connection;
use std::path::Path;

const KEY: &str = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";
const ROOT: &str = "7f757fb737085dcaaad0727deb89a14c18141681dc56be0a215a592c715f8e3b";
const OTHER_ROOT: &str = "1111111111111111111111111111111111111111111111111111111111111111";
const NOW: i64 = 5 * DAY_MS;
const LIMIT_KEY: Key = [7u8; 32];

struct W {
    c: Connection,
    s: BlobStore,
    core: Core,
    fp: Fingerprinter,
    origins: Vec<String>,
    _d: tempfile::TempDir,
}
fn world() -> W {
    let dir = tempfile::tempdir().unwrap();
    let mut c = db::open(&dir.path().join("t.db")).unwrap();
    db::migrate(&mut c, &Path::new(env!("CARGO_MANIFEST_DIR")).join("../worker/migrations")).unwrap();
    for (id, trust) in [("t", "teacher"), ("a", "admin"), ("p", "pending"), ("u", "unverified")] {
        c.execute("INSERT INTO users (id, email, display_name, trust, created_at) VALUES (?1, ?2, 'N', ?3, 1)", rusqlite::params![id, format!("{id}@x.org"), trust]).unwrap();
    }
    let s = BlobStore::new(dir.path().join("blobs")).unwrap();
    W { c, s, core: Core::new(Some(KEY.into())), fp: Fingerprinter::from_root_hex(ROOT).unwrap(), origins: vec!["https://app.example".into()], _d: dir }
}
fn run(w: &W, route: &Route, method: &str, path: &str, who: Option<(&str, &str)>, origin: Option<&str>) -> Res<Caller> {
    let g = Gate { allowed_origins: &w.origins, limit_key: &LIMIT_KEY, ip_prefix: 64, store: &w.s, core: &w.core, fp: &w.fp };
    let req = Req { method, path, origin, x_client: None, ip: Some("203.0.113.5") };
    let caller = who.map(|(u, t)| Caller { user_id: u.into(), trust: t.into(), family: None });
    gate(&g, &PoolCtx::new(&w.c, NOW), route, &req, caller.as_ref())
}
fn err<T: std::fmt::Debug>(r: Res<T>) -> PoolError {
    match r {
        Err(crate::pool::ctx::Fail::Pool(e)) => e,
        other => panic!("expected a pool error, got {other:?}"),
    }
}
fn invite(w: &W, user: &str) {
    w.c.execute("INSERT INTO pool_invite_redemptions (user_id, invite_id, at) VALUES (?1, 'i', 1)", [user]).unwrap();
}

#[test]
fn a_wrong_method_is_a_405_with_allow_and_comes_before_anything_else() {
    let w = world();
    let e = err(run(&w, &Route::Items, "DELETE", "/items", None, Some("https://evil.example")));
    assert_eq!((e.status, e.code.as_str()), (405, "method_not_allowed"));
    assert_eq!(e.headers, vec![("Allow".to_string(), "GET, POST".to_string())]);
}

#[test]
fn origins_x_client_and_a_missing_session_are_refused_in_that_order() {
    let w = world();
    let e = err(run(&w, &Route::Items, "GET", "/items", Some(("t", "teacher")), Some("https://evil.example")));
    assert_eq!((e.status, e.code.as_str()), (403, "origin_not_allowed"));
    let g = Gate { allowed_origins: &w.origins, limit_key: &LIMIT_KEY, ip_prefix: 64, store: &w.s, core: &w.core, fp: &w.fp };
    let req = Req { method: "GET", path: "/items", origin: Some("https://app.example"), x_client: Some("native"), ip: Some("203.0.113.5") };
    let e = err(gate(&g, &PoolCtx::new(&w.c, NOW), &Route::Items, &req, None));
    assert_eq!((e.status, e.code.as_str()), (400, "native_client_with_origin"));
    let e = err(run(&w, &Route::Items, "GET", "/items", None, Some("https://app.example")));
    assert_eq!((e.status, e.code.as_str()), (401, "unauthorized"));
    assert_eq!(e.headers, vec![("WWW-Authenticate".to_string(), r#"Bearer error="invalid_token""#.to_string())]);
}

#[test]
fn a_request_without_a_client_address_is_a_403() {
    let w = world();
    let g = Gate { allowed_origins: &w.origins, limit_key: &LIMIT_KEY, ip_prefix: 64, store: &w.s, core: &w.core, fp: &w.fp };
    let req = Req { method: "GET", path: "/items", origin: None, x_client: None, ip: None };
    let c = Caller { user_id: "t".into(), trust: "teacher".into(), family: None };
    let e = err(gate(&g, &PoolCtx::new(&w.c, NOW), &Route::Items, &req, Some(&c)));
    assert_eq!((e.status, e.code.as_str()), (403, "client_ip_required"));
}

#[test]
fn the_public_pool_needs_an_approved_account_and_a_redeemed_invite() {
    let w = world();
    let e = err(run(&w, &Route::Items, "GET", "/items", Some(("u", "unverified")), None));
    assert_eq!((e.status, e.code.as_str()), (403, "account_not_approved"));
    let e = err(run(&w, &Route::Items, "GET", "/items", Some(("p", "pending")), None));
    assert_eq!(e.code, "account_not_approved");
    assert!(e.extra["message"].as_str().unwrap().contains("waiting for approval"));
    let e = err(run(&w, &Route::Items, "GET", "/items", Some(("t", "teacher")), None));
    assert_eq!((e.status, e.code.as_str()), (403, "invite_required"));
    invite(&w, "t");
    assert_eq!(run(&w, &Route::Items, "GET", "/items", Some(("t", "teacher")), None).unwrap().user_id, "t");
    // Admins are exempt from the invite.
    assert!(run(&w, &Route::Items, "GET", "/items", Some(("a", "admin")), None).is_ok());
}

#[test]
fn a_pending_account_may_export_redeem_and_use_circles_while_its_request_is_open() {
    let w = world();
    assert!(run(&w, &Route::MeExport, "GET", "/me/export", Some(("p", "pending")), None).is_ok());
    assert!(run(&w, &Route::MeInvite, "POST", "/me/invite", Some(("p", "pending")), None).is_ok());
    assert!(run(&w, &Route::CircleJoin, "POST", "/circles/join", Some(("p", "pending")), None).is_ok());
    assert!(run(&w, &Route::Circles, "GET", "/circles", Some(("p", "pending")), None).is_ok());
    // Creating a circle stays behind approval.
    let e = err(run(&w, &Route::Circles, "POST", "/circles", Some(("p", "pending")), None));
    assert_eq!(e.code, "account_not_approved");
    // A rejected applicant is refused even there.
    w.c.execute("INSERT INTO approval_requests (id, user_id, requested_at, decided_at, decision) VALUES ('r', 'p', 1, 2, 'rejected')", []).unwrap();
    let e = err(run(&w, &Route::CircleJoin, "POST", "/circles/join", Some(("p", "pending")), None));
    assert_eq!(e.code, "account_not_approved");
    // An unverified account has no such exemption.
    assert_eq!(err(run(&w, &Route::MeExport, "GET", "/me/export", Some(("u", "unverified")), None)).code, "account_not_approved");
}

#[test]
fn the_burst_limit_answers_429_with_retry_after() {
    let w = world();
    invite(&w, "t");
    let mut last = None;
    for _ in 0..301 {
        last = Some(run(&w, &Route::Items, "GET", "/items", Some(("t", "teacher")), None));
    }
    let e = err(last.unwrap());
    assert_eq!((e.status, e.code.as_str()), (429, "rate_limited"));
    assert!(e.headers.iter().any(|h| h.0 == "Retry-After"));
}

#[test]
fn writes_pin_the_pool_key_and_a_different_key_is_a_503_while_reads_continue() {
    let mut w = world();
    invite(&w, "t");
    // The first write records the key's id.
    assert!(run(&w, &Route::Items, "POST", "/items", Some(("t", "teacher")), None).is_ok());
    let pinned: String = w.c.query_row("SELECT value FROM pool_kv WHERE key = 'pool_key_id'", [], |r| r.get(0)).unwrap();
    assert_eq!(pinned, w.fp.pool_key_id());
    w.fp = Fingerprinter::from_root_hex(OTHER_ROOT).unwrap();
    let e = err(run(&w, &Route::Items, "POST", "/items", Some(("t", "teacher")), None));
    assert_eq!((e.status, e.code.as_str()), (503, "pool_key_mismatch"));
    assert!(run(&w, &Route::Items, "GET", "/items", Some(("t", "teacher")), None).is_ok());
    // Bulk reads sent as POST continue too.
    assert!(run(&w, &Route::ItemVersions, "POST", "/items/versions", Some(("t", "teacher")), None).is_ok());
    assert!(run(&w, &Route::CircleHeads, "POST", "/circles/heads", Some(("t", "teacher")), None).is_ok());
}

#[test]
fn the_verdict_is_empty_for_a_new_pool_and_unknown_for_data_with_nothing_to_check() {
    let w = world();
    let x = PoolCtx::new(&w.c, NOW);
    assert_eq!(pool_key_verdict(&x, &w.s, &w.core).unwrap(), KeyVerdict::Empty);
    w.c.execute("INSERT INTO items (id, author_id, kind, visibility, license, created_at, updated_at) VALUES ('0123456789abcdef0123456789abcdef', 't', 'bank', 'unlisted', 'CC-BY-4.0', 1, 1)", []).unwrap();
    assert_eq!(pool_key_verdict(&x, &w.s, &w.core).unwrap(), KeyVerdict::Unknown);
}
