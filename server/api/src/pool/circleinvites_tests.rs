//! Tests for `circleinvites.rs` on the real migrated schema: issuing, listing and withdrawing codes, previewing and joining (caps, full
//! circles, former members, every refused code the same 422).
use super::*;
use crate::db;
use crate::pool::circles::create_circle;
use crate::pool::ctx::Fail;
use rusqlite::Connection;
use std::path::Path;

fn open() -> (Connection, tempfile::TempDir) {
    let dir = tempfile::tempdir().unwrap();
    let mut conn = db::open(&dir.path().join("t.db")).unwrap();
    db::migrate(&mut conn, &Path::new(env!("CARGO_MANIFEST_DIR")).join("../worker/migrations")).unwrap();
    (conn, dir)
}
fn user(c: &Connection, u: &str) {
    c.execute("INSERT INTO users (id, email, display_name, trust, created_at) VALUES (?1, ?2, ?3, 'teacher', 1)", params![u, format!("{u}@x.org"), format!("Teacher {u}")]).unwrap();
}
fn m(v: Value) -> Map<String, Value> {
    v.as_object().unwrap().clone()
}
fn err(f: Fail) -> (u16, String, Value) {
    match f {
        Fail::Pool(e) => (e.status, e.code.clone(), e.body()),
        other => panic!("{other:?}"),
    }
}
fn make(c: &Connection, u: &str, name: &str) -> String {
    create_circle(&PoolCtx::new(c, 1_000), u, &m(json!({ "name": name }))).unwrap().1["circle"]["id"].as_str().unwrap().to_string()
}
fn invite(c: &Connection, owner: &str, cid: &str, body: Value) -> Value {
    let (s, v) = create_invite(&PoolCtx::new(c, 2_000), owner, cid, &m(body)).unwrap();
    assert_eq!(s, 201);
    v
}
fn tries(c: &Connection, u: &str) -> i64 {
    c.query_row("SELECT COALESCE((SELECT count FROM usage_daily WHERE user_id = ?1 AND metric = 'circle_join_tries'), 0)", [u], |r| r.get(0)).unwrap()
}

#[test]
fn an_owner_issues_a_code_shown_once_with_defaults_and_limits() {
    let (c, _d) = open();
    for u in ["a", "b"] {
        user(&c, u);
    }
    let cid = make(&c, "a", "Team");
    let v = invite(&c, "a", &cid, json!({ "role": "editor" }));
    assert_eq!((v["max_uses"].clone(), v["expires_at"].clone(), v["note"].clone(), v["shown_once"].clone()), (json!(1), json!(2_000 + 7 * DAY_MS), json!(""), json!(true)));
    let code = v["code"].as_str().unwrap();
    assert!(code.starts_with("C-"));
    // Only the hash is stored (never the code); the audit has no code either.
    let stored: String = c.query_row("SELECT code_hash FROM circle_invites WHERE id = ?1", [v["invite_id"].as_str().unwrap()], |r| r.get(0)).unwrap();
    assert_eq!(stored, circle_code_hash(&normalise_circle_code(&json!(code)).unwrap()));
    let audit: String = c.query_row("SELECT detail FROM pool_audit WHERE action = 'circle.invite_create'", [], |r| r.get(0)).unwrap();
    assert!(!audit.contains(code));
    let x = PoolCtx::new(&c, 2_000);
    let field = |b: Value| err(create_invite(&x, "a", &cid, &m(b)).unwrap_err()).2["field"].clone();
    assert_eq!(field(json!({ "role": "owner" })), json!("role"));
    assert_eq!(field(json!({})), json!("role"));
    assert_eq!(field(json!({ "role": "viewer", "max_uses": 51 })), json!("max_uses"));
    assert_eq!(field(json!({ "role": "viewer", "expires_in_days": 31 })), json!("expires_in_days"));
    assert_eq!(field(json!({ "role": "viewer", "note": "n".repeat(201) })), json!("note"));
    // Only the owner of an open circle issues codes.
    c.execute("INSERT INTO circle_members (circle_id, user_id, role, joined_at, join_seq) VALUES (?1, 'b', 'editor', 1, 9)", [&cid]).unwrap();
    assert_eq!(err(create_invite(&x, "b", &cid, &m(json!({ "role": "viewer" }))).unwrap_err()).1, "circle_role_required");
}

#[test]
fn the_open_code_cap_refuses_and_gives_the_day_back() {
    let (c, _d) = open();
    user(&c, "a");
    let cid = make(&c, "a", "Team");
    let x = PoolCtx::new(&c, 2_000);
    let cap = x.limit_of("*", "circle_open_invites").unwrap();
    for _ in 0..cap {
        invite(&c, "a", &cid, json!({ "role": "viewer" }));
    }
    let before: i64 = c.query_row("SELECT count FROM usage_daily WHERE user_id = 'a' AND metric = 'circle_invites'", [], |r| r.get(0)).unwrap();
    let (s, code, b) = err(create_invite(&x, "a", &cid, &m(json!({ "role": "viewer" }))).unwrap_err());
    assert_eq!((s, code.as_str(), b["metric"].clone(), b["limit"].clone()), (422, "circles_limit", json!("circle_open_invites"), json!(cap)));
    let after: i64 = c.query_row("SELECT count FROM usage_daily WHERE user_id = 'a' AND metric = 'circle_invites'", [], |r| r.get(0)).unwrap();
    assert_eq!(after, before);
}

#[test]
fn the_list_is_newest_first_with_states_and_pages_by_cursor() {
    let (c, _d) = open();
    user(&c, "a");
    let cid = make(&c, "a", "Team");
    let first = invite(&c, "a", &cid, json!({ "role": "viewer", "note": "for ann" }));
    let second = invite(&c, "a", &cid, json!({ "role": "editor", "max_uses": 2 }));
    c.execute("UPDATE circle_invites SET created_at = 1999 WHERE id = ?1", [first["invite_id"].as_str().unwrap()]).unwrap();
    c.execute("UPDATE circle_invites SET uses = 1, max_uses = 1 WHERE id = ?1", [second["invite_id"].as_str().unwrap()]).unwrap();
    let x = PoolCtx::new(&c, 3_000);
    let l = list_invites(&x, "a", &cid, None).unwrap();
    let rows = l["invites"].as_array().unwrap();
    assert_eq!(rows.iter().map(|r| r["invite_id"].clone()).collect::<Vec<_>>(), vec![second["invite_id"].clone(), first["invite_id"].clone()]);
    assert_eq!((rows[0]["state"].clone(), rows[1]["state"].clone(), rows[1]["note"].clone(), rows[0]["created_by_name"].clone()), (json!("used_up"), json!("active"), json!("for ann"), json!("Teacher a")));
    assert!(l["next_cursor"].is_null());
    assert!(rows[0].get("code").is_none() && rows[0].get("code_hash").is_none());
    // Expired once its time has passed; a cursor after the first row leaves the second.
    let late = PoolCtx::new(&c, 2_000 + 8 * DAY_MS);
    assert_eq!(list_invites(&late, "a", &cid, None).unwrap()["invites"][1]["state"], json!("expired"));
    let cur = format!("{}:{}", rows[0]["created_at"], rows[0]["invite_id"].as_str().unwrap());
    let after = list_invites(&x, "a", &cid, Some(&cur)).unwrap();
    assert_eq!(after["invites"].as_array().unwrap().len(), 1);
    assert_eq!(list_invites(&x, "a", &cid, Some("junk")).unwrap()["invites"].as_array().unwrap().len(), 2, "a malformed cursor is ignored");
    user(&c, "b");
    assert_eq!(err(list_invites(&x, "b", &cid, None).unwrap_err()).1, "not_found");
}

#[test]
fn revoking_stops_further_joins_and_is_said_once() {
    let (c, _d) = open();
    for u in ["a", "b"] {
        user(&c, u);
    }
    let cid = make(&c, "a", "Team");
    let v = invite(&c, "a", &cid, json!({ "role": "viewer" }));
    let iid = v["invite_id"].as_str().unwrap();
    let x = PoolCtx::new(&c, 3_000);
    assert_eq!(err(revoke_invite(&x, "a", &cid, &m(json!({ "invite_id": "nope" }))).unwrap_err()).2["field"], json!("invite_id"));
    assert_eq!(err(revoke_invite(&x, "a", &cid, &m(json!({ "invite_id": format!("{:032x}", 9) }))).unwrap_err()).1, "not_found");
    assert_eq!(revoke_invite(&x, "a", &cid, &m(json!({ "invite_id": iid }))).unwrap(), json!({ "ok": true, "invite_id": iid }));
    let (s, code, b) = err(revoke_invite(&x, "a", &cid, &m(json!({ "invite_id": iid }))).unwrap_err());
    assert_eq!((s, code.as_str(), b["revoked_at"].clone()), (409, "already_revoked", json!(3_000)));
    assert_eq!(err(join_circle(&x, "b", &m(json!({ "code": v["code"] }))).unwrap_err()).1, "circle_invite_invalid");
}

#[test]
fn joining_takes_the_code_role_once_and_refunds_the_try() {
    let (c, _d) = open();
    for u in ["a", "b", "c"] {
        user(&c, u);
    }
    let cid = make(&c, "a", "Team");
    let v = invite(&c, "a", &cid, json!({ "role": "editor" }));
    let code = v["code"].clone();
    let x = PoolCtx::new(&c, 3_000);
    // Preview uses nothing and returns the circle, its owner and the role.
    let p = join_preview(&x, "b", &m(json!({ "code": code }))).unwrap();
    assert_eq!(p, json!({ "circle": { "id": cid, "name": "Team", "owner_name": "Teacher a", "members": 1 }, "role": "editor", "already_member": false }));
    assert_eq!(tries(&c, "b"), 0, "a good preview refunds its try");
    let j = join_circle(&x, "b", &m(json!({ "code": code }))).unwrap();
    assert_eq!(j, json!({ "ok": true, "circle_id": cid, "role": "editor" }));
    assert_eq!(tries(&c, "b"), 0);
    assert_eq!(c.query_row::<i64, _, _>("SELECT uses FROM circle_invites", [], |r| r.get(0)).unwrap(), 1);
    let n: i64 = c.query_row("SELECT COUNT(*) FROM pool_audit WHERE action = 'circle.join'", [], |r| r.get(0)).unwrap();
    assert_eq!(n, 1);
    // Joining again says so and uses nothing; a used-up single-use code refuses a newcomer, and the failed try is charged.
    assert_eq!(join_circle(&x, "b", &m(json!({ "code": code }))).unwrap(), json!({ "ok": true, "already": true, "circle_id": cid, "role": "editor" }));
    assert_eq!(err(join_circle(&x, "c", &m(json!({ "code": code }))).unwrap_err()).1, "circle_invite_invalid");
    assert_eq!(tries(&c, "c"), 1);
    // Malformed codes are a 400 before any try is charged.
    assert_eq!(err(join_circle(&x, "c", &m(json!({ "code": "nonsense" }))).unwrap_err()).2["field"], json!("code"));
    assert_eq!(tries(&c, "c"), 1);
    // The preview of a code the caller already holds a seat for says so.
    assert_eq!(join_preview(&x, "b", &m(json!({ "code": code }))).unwrap()["already_member"], json!(true));
}

#[test]
fn every_refused_code_is_the_same_422() {
    let (c, _d) = open();
    for u in ["a", "b"] {
        user(&c, u);
    }
    let cid = make(&c, "a", "Team");
    let v = invite(&c, "a", &cid, json!({ "role": "viewer", "max_uses": 5 }));
    let x = PoolCtx::new(&c, 3_000);
    let unknown = json!(new_circle_code());
    let one = |code: &Value| err(join_circle(&x, "b", &m(json!({ "code": code }))).unwrap_err());
    let a = one(&unknown);
    assert_eq!((a.0, a.1.as_str()), (422, "circle_invite_invalid"));
    // Expired, circle closed: identical answers.
    let late = PoolCtx::new(&c, 3_000 + 8 * DAY_MS);
    let e = err(join_circle(&late, "b", &m(json!({ "code": v["code"] }))).unwrap_err());
    assert_eq!(e.2, a.2);
    c.execute("UPDATE circles SET closed_at = 1, close_reason = 'owner_closed' WHERE id = ?1", [&cid]).unwrap();
    assert_eq!(one(&v["code"]).2, a.2);
    c.execute("UPDATE circles SET closed_at = NULL, close_reason = NULL WHERE id = ?1", [&cid]).unwrap();
    // A suspended issuer's codes admit nobody.
    c.execute("UPDATE users SET suspended_at = 1 WHERE id = 'a'", []).unwrap();
    assert_eq!(one(&v["code"]).2, a.2);
    // The daily allowance of tries runs out.
    let cap = x.limit_of("b", "circle_join_tries").unwrap();
    for _ in 0..cap {
        let _ = join_circle(&x, "b", &m(json!({ "code": unknown })));
    }
    assert_eq!(one(&unknown).1, "daily_limit");
}

#[test]
fn a_full_circle_and_the_membership_caps_refuse_and_refund() {
    let (c, _d) = open();
    for u in ["a", "b", "d"] {
        user(&c, u);
    }
    let cid = make(&c, "a", "Team");
    let v = invite(&c, "a", &cid, json!({ "role": "viewer", "max_uses": 5 }));
    let x = PoolCtx::new(&c, 3_000);
    c.execute("INSERT INTO pool_limits (user_id, metric, value, updated_at, updated_by) VALUES ('*', 'circle_members', 1, 1, 'adm')", []).unwrap();
    let (s, code, b) = err(join_circle(&x, "b", &m(json!({ "code": v["code"] }))).unwrap_err());
    assert_eq!((s, code.as_str(), b["limit"].clone()), (409, "circle_full", json!(1)));
    assert_eq!(tries(&c, "b"), 0, "refunded");
    c.execute("UPDATE pool_limits SET value = 50 WHERE metric = 'circle_members'", []).unwrap();
    c.execute("INSERT INTO pool_limits (user_id, metric, value, updated_at, updated_by) VALUES ('*', 'circle_memberships', 1, 1, 'adm')", []).unwrap();
    // 'd' owns another circle (one membership): the cap of 1 refuses a second.
    make(&c, "d", "Other");
    let (s, code, b) = err(join_circle(&x, "d", &m(json!({ "code": v["code"] }))).unwrap_err());
    assert_eq!((s, code.as_str(), b["metric"].clone()), (422, "circles_limit", json!("circle_memberships")));
    assert_eq!(tries(&c, "d"), 0);
}

#[test]
fn a_removed_member_needs_a_newer_code_and_a_demoted_one_comes_back_as_a_viewer() {
    let (c, _d) = open();
    for u in ["a", "b"] {
        user(&c, u);
    }
    let cid = make(&c, "a", "Team");
    let old = invite(&c, "a", &cid, json!({ "role": "editor", "max_uses": 5 }));
    let x = PoolCtx::new(&c, 3_000);
    join_circle(&x, "b", &m(json!({ "code": old["code"] }))).unwrap();
    // Removed after the code was issued: the old code no longer admits her.
    c.execute("UPDATE circle_members SET left_at = 2_500, left_reason = 'removed' WHERE user_id = 'b'", []).unwrap();
    assert_eq!(err(join_circle(&x, "b", &m(json!({ "code": old["code"] }))).unwrap_err()).1, "circle_invite_invalid");
    // A newer code (issued after the removal) readmits her; the row is reused with a new join_seq.
    let later = PoolCtx::new(&c, 4_000);
    let new = create_invite(&later, "a", &cid, &m(json!({ "role": "editor" }))).unwrap().1;
    let j = join_circle(&later, "b", &m(json!({ "code": new["code"] }))).unwrap();
    assert_eq!(j["role"], json!("editor"));
    let (left, rows): (Option<i64>, i64) = c.query_row("SELECT left_at, (SELECT COUNT(*) FROM circle_members WHERE user_id = 'b') FROM circle_members WHERE user_id = 'b'", [], |r| Ok((r.get(0)?, r.get(1)?))).unwrap();
    assert_eq!((left, rows), (None, 1));
    // Left as a viewer after the code was issued: the code gives back at most a viewer.
    c.execute("UPDATE circle_members SET role = 'viewer', left_at = 4_500, left_reason = 'left' WHERE user_id = 'b'", []).unwrap();
    let muchlater = PoolCtx::new(&c, 5_000);
    c.execute("UPDATE circle_invites SET created_at = 4_000 WHERE id = ?1", [new["invite_id"].as_str().unwrap()]).unwrap();
    c.execute("UPDATE circle_invites SET uses = 0, max_uses = 5", []).unwrap();
    assert_eq!(join_circle(&muchlater, "b", &m(json!({ "code": new["code"] }))).unwrap()["role"], json!("viewer"));
}
