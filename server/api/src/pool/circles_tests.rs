//! Tests for `circles.rs` on the real migrated schema: create (caps, daily allowance, refund), list, read (bank masking, pending offer),
//! edit, close (undo window, the repeated-close flag) and reopen.
use super::*;
use crate::db;
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
fn make(c: &Connection, u: &str, name: &str, now: i64) -> String {
    let (s, v) = create_circle(&PoolCtx::new(c, now), u, &m(json!({ "name": name }))).unwrap();
    assert_eq!(s, 201);
    v["circle"]["id"].as_str().unwrap().to_string()
}
fn join(c: &Connection, cid: &str, u: &str, role: &str, seq: i64) {
    c.execute("INSERT INTO circle_members (circle_id, user_id, role, joined_at, join_seq) VALUES (?1, ?2, ?3, 10, ?4)", params![cid, u, role, seq]).unwrap();
}
fn bank(c: &Connection, cid: &str, n: u8, author: &str, title: &str) -> String {
    let id = format!("{:032x}", n);
    c.execute("INSERT INTO circle_banks (id, circle_id, author_id, title, license, head_version, created_at, updated_at) VALUES (?1, ?2, ?3, ?4, 'circle-only', 1, ?5, ?5)", params![id, cid, author, title, n as i64]).unwrap();
    id
}

#[test]
fn creating_a_circle_makes_the_caller_its_owner_and_audits_it() {
    let (c, _d) = open();
    user(&c, "a");
    let x = PoolCtx::new(&c, 5_000);
    let (s, v) = create_circle(&x, "a", &m(json!({ "name": "  Algebra team ", "description": "" }))).unwrap();
    assert_eq!(s, 201);
    assert_eq!((v["circle"]["name"].clone(), v["circle"]["description"].clone(), v["circle"]["role"].clone(), v["circle"]["members"].clone()), (json!("Algebra team"), json!(""), json!("owner"), json!(1)));
    let cid = v["circle"]["id"].as_str().unwrap();
    let role: String = c.query_row("SELECT role FROM circle_members WHERE circle_id = ?1 AND user_id = 'a'", [cid], |r| r.get(0)).unwrap();
    assert_eq!(role, "owner");
    let n: i64 = c.query_row("SELECT COUNT(*) FROM pool_audit WHERE action = 'circle.create' AND item_id = ?1", [cid], |r| r.get(0)).unwrap();
    assert_eq!(n, 1);
    // A missing or over-long name is a 400 on `name`, and does not use up the day's allowance.
    for body in [json!({}), json!({ "name": "" }), json!({ "name": "x".repeat(81) }), json!({ "name": 5 })] {
        assert_eq!(err(create_circle(&x, "a", &m(body)).unwrap_err()).2["field"], json!("name"));
    }
    assert_eq!(err(create_circle(&x, "a", &m(json!({ "name": "ok", "description": "d".repeat(1001) }))).unwrap_err()).2["field"], json!("description"));
}

#[test]
fn the_owned_cap_refuses_and_gives_the_day_back() {
    let (c, _d) = open();
    user(&c, "a");
    let x = PoolCtx::new(&c, 5_000);
    c.execute("INSERT INTO pool_limits (user_id, metric, value, updated_at, updated_by) VALUES ('*', 'circle_creates', 50, 1, 'adm')", []).unwrap();
    let owned = x.limit_of("a", "circles_owned").unwrap();
    for i in 0..owned {
        make(&c, "a", &format!("c{i}"), 5_000);
    }
    let used_before: i64 = c.query_row("SELECT count FROM usage_daily WHERE user_id = 'a' AND metric = 'circle_creates'", [], |r| r.get(0)).unwrap();
    let (s, code, b) = err(create_circle(&x, "a", &m(json!({ "name": "one more" }))).unwrap_err());
    assert_eq!((s, code.as_str(), b["metric"].clone(), b["limit"].clone()), (422, "circles_limit", json!("circles_owned"), json!(owned)));
    let used_after: i64 = c.query_row("SELECT count FROM usage_daily WHERE user_id = 'a' AND metric = 'circle_creates'", [], |r| r.get(0)).unwrap();
    assert_eq!(used_after, used_before, "a refused create is given back");
}

#[test]
fn an_erased_account_cannot_create() {
    let (c, _d) = open();
    user(&c, "gone");
    c.execute("UPDATE users SET deleted_at = 1, email = NULL, pw_hash = NULL, pw_params = NULL, display_name = NULL, profile = NULL WHERE id = 'gone'", []).unwrap();
    assert_eq!(err(create_circle(&PoolCtx::new(&c, 5_000), "gone", &m(json!({ "name": "x" }))).unwrap_err()).0, 401);
}

#[test]
fn the_list_has_memberships_newest_first_with_counts_and_offers() {
    let (c, _d) = open();
    for u in ["a", "b"] {
        user(&c, u);
    }
    let c1 = make(&c, "a", "First", 1_000);
    let c2 = make(&c, "a", "Second", 2_000);
    join(&c, &c1, "b", "editor", 50);
    bank(&c, &c1, 1, "a", "Quiz");
    c.execute("INSERT INTO circle_transfers (circle_id, from_user, to_user, created_at, expires_at) VALUES (?1, 'a', 'b', 1, 99999)", [&c1]).unwrap();
    let x = PoolCtx::new(&c, 5_000);
    let l = list_circles(&x, "a").unwrap()["circles"].as_array().unwrap().clone();
    assert_eq!(l.iter().map(|r| r["id"].clone()).collect::<Vec<_>>(), vec![json!(c2), json!(c1)]);
    assert_eq!((l[1]["members"].clone(), l[1]["banks"].clone(), l[1]["role"].clone(), l[1]["ownership_offered_until"].clone()), (json!(2), json!(1), json!("owner"), json!(null)));
    let lb = list_circles(&x, "b").unwrap()["circles"].as_array().unwrap().clone();
    assert_eq!((lb.len(), lb[0]["ownership_offered_until"].clone(), lb[0]["closed_by"].clone()), (1, json!(99999), json!(null)));
    assert!(list_circles(&x, "nobody").unwrap()["circles"].as_array().unwrap().is_empty());
}

#[test]
fn reading_a_circle_masks_held_banks_and_shows_removed_ones_to_those_who_may() {
    let (c, _d) = open();
    for u in ["a", "b", "c"] {
        user(&c, u);
    }
    let cid = make(&c, "a", "Team", 1_000);
    join(&c, &cid, "b", "editor", 50);
    join(&c, &cid, "c", "viewer", 51);
    let live = bank(&c, &cid, 1, "b", "Live bank");
    let held = bank(&c, &cid, 2, "b", "Held bank");
    let stu = bank(&c, &cid, 3, "b", "Student-data hold");
    let removed = bank(&c, &cid, 4, "b", "Removed bank");
    c.execute("INSERT INTO circle_holds (bank_id, reason, source, at) VALUES (?1, 'reports', '', 1)", [&held]).unwrap();
    c.execute("INSERT INTO circle_holds (bank_id, reason, source, at) VALUES (?1, 'student_data', 'privacy', 1)", [&stu]).unwrap();
    c.execute("UPDATE circle_banks SET removed_at = 5, removed_reason = 'author_removed', purge_after = 777 WHERE id = ?1", [&removed]).unwrap();
    let x = PoolCtx::new(&c, 5_000);
    let by_id = |v: &Value, id: &str| v["banks"].as_array().unwrap().iter().find(|b| b["id"] == json!(id)).cloned();
    // Owner a: sees live; the reports-held bank only masked? No: a blocking hold is hidden from the owner except as masked "hidden"-less omission.
    let a = get_circle(&x, "a", &cid).unwrap();
    assert_eq!(a["circle"]["role"], json!("owner"));
    assert_eq!(a["members"].as_array().unwrap().len(), 3);
    assert!(by_id(&a, &live).is_some());
    assert!(by_id(&a, &held).is_none(), "a report's hold hides the bank from the owner");
    let s = by_id(&a, &stu).unwrap();
    assert_eq!((s["title"].clone(), s["held"].clone()), (json!("Student-data hold"), json!(true)), "an owner may repair a bank held only for student data");
    assert_eq!(by_id(&a, &removed).unwrap()["removed"], json!({ "reason": "author_removed", "undo_until": 777 }));
    // The author b: sees their own held banks in full.
    let b = get_circle(&x, "b", &cid).unwrap();
    assert_eq!(by_id(&b, &held).unwrap()["title"], json!("Held bank"));
    assert_eq!(by_id(&b, &held).unwrap()["is_author"], json!(true));
    // A viewer c: no held banks, no removed ones.
    let v = get_circle(&x, "c", &cid).unwrap();
    assert_eq!(v["banks"].as_array().unwrap().len(), 1);
    assert!(by_id(&v, &live).is_some() && by_id(&v, &stu).is_none() && by_id(&v, &removed).is_none());
    // A suspended author's live bank is not listed; their removed bank is, masked, to the owner.
    c.execute("UPDATE users SET suspended_at = 9 WHERE id = 'b'", []).unwrap();
    let a = get_circle(&x, "a", &cid).unwrap();
    assert!(by_id(&a, &live).is_none());
    let r = by_id(&a, &removed).unwrap();
    assert_eq!((r["title"].clone(), r["hidden"].clone()), (json!(null), json!(true)));
    // The pending offer is shown to the owner and to the member offered it, not to others.
    c.execute("INSERT INTO circle_transfers (circle_id, from_user, to_user, created_at, expires_at) VALUES (?1, 'a', 'c', 1, 99999)", [&cid]).unwrap();
    assert_eq!(get_circle(&x, "a", &cid).unwrap()["pending_transfer"]["to_user_id"], json!("c"));
    assert_eq!(get_circle(&x, "c", &cid).unwrap()["pending_transfer"]["to_name"], json!("Teacher c"));
    assert_eq!(get_circle(&x, "b", &cid).unwrap()["pending_transfer"], json!(null));
    assert_eq!(err(get_circle(&x, "nobody", &cid).unwrap_err()).1, "not_found");
}

#[test]
fn only_the_owner_edits_an_open_circle() {
    let (c, _d) = open();
    for u in ["a", "b"] {
        user(&c, u);
    }
    let cid = make(&c, "a", "Team", 1_000);
    join(&c, &cid, "b", "editor", 50);
    let x = PoolCtx::new(&c, 5_000);
    let r = patch_circle(&x, "a", &cid, &m(json!({ "name": "Renamed", "description": "About" }))).unwrap();
    assert_eq!(r, json!({ "ok": true, "name": "Renamed", "description": "About" }));
    // Omitted fields stay; a null description clears it.
    assert_eq!(patch_circle(&x, "a", &cid, &m(json!({ "description": null }))).unwrap(), json!({ "ok": true, "name": "Renamed", "description": "" }));
    assert_eq!(err(patch_circle(&x, "a", &cid, &m(json!({ "name": "" }))).unwrap_err()).2["field"], json!("name"));
    let (s, code, b) = err(patch_circle(&x, "b", &cid, &m(json!({ "name": "Mine" }))).unwrap_err());
    assert_eq!((s, code.as_str(), b["needed"].clone()), (403, "circle_role_required", json!("owner")));
    let n: i64 = c.query_row("SELECT COUNT(*) FROM pool_audit WHERE action = 'circle.edit'", [], |r| r.get(0)).unwrap();
    assert_eq!(n, 2);
    c.execute("UPDATE circles SET closed_at = 1, close_reason = 'admin' WHERE id = ?1", [&cid]).unwrap();
    assert_eq!(err(patch_circle(&x, "a", &cid, &m(json!({ "name": "x" }))).unwrap_err()).1, "circle_closed");
}

#[test]
fn closing_hides_for_14_days_and_reopening_undoes_it_inside_the_window() {
    let (c, _d) = open();
    for u in ["a", "b"] {
        user(&c, u);
    }
    let cid = make(&c, "a", "Team", 1_000);
    join(&c, &cid, "b", "editor", 50);
    let x = PoolCtx::new(&c, 5_000);
    assert_eq!(err(close_circle(&x, "b", &cid).unwrap_err()).1, "circle_role_required");
    let r = close_circle(&x, "a", &cid).unwrap();
    assert_eq!(r, json!({ "ok": true, "undo_until": 5_000 + CIRCLE_UNDO_MS }));
    let row: (Option<String>, Option<i64>, Option<i64>) = c.query_row("SELECT close_reason, undo_until, purge_after FROM circles WHERE id = ?1", [&cid], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?))).unwrap();
    assert_eq!(row, (Some("owner_closed".into()), Some(5_000 + CIRCLE_UNDO_MS), Some(5_000 + CIRCLE_UNDO_MS)));
    // Closing again later gives the same window, never a longer one.
    let later = PoolCtx::new(&c, 9_000);
    assert_eq!(close_circle(&later, "a", &cid).unwrap(), json!({ "ok": true, "undo_until": 5_000 + CIRCLE_UNDO_MS }));
    // An admin close is not undoable by the owner, and says so.
    // Reopen: inside the window it works; reopening an open circle is "already".
    assert_eq!(reopen_circle(&later, "a", &cid).unwrap(), json!({ "ok": true }));
    assert_eq!(reopen_circle(&later, "a", &cid).unwrap(), json!({ "ok": true, "already": true }));
    assert_eq!(err(reopen_circle(&later, "b", &cid).unwrap_err()).1, "circle_role_required");
    // After the window it cannot be reopened.
    close_circle(&later, "a", &cid).unwrap();
    let past = PoolCtx::new(&c, 9_000 + CIRCLE_UNDO_MS + 1);
    let (s, code, b) = err(reopen_circle(&past, "a", &cid).unwrap_err());
    assert_eq!((s, code.as_str(), b["message"].clone()), (409, "circle_closed", json!("This circle can no longer be reopened.")));
    // A circle an admin closed is reported as such to the owner who tries to close it.
    c.execute("UPDATE circles SET close_reason = 'admin', undo_until = NULL WHERE id = ?1", [&cid]).unwrap();
    assert_eq!(err(close_circle(&past, "a", &cid).unwrap_err()).2["reason"], json!("admin"));
}

#[test]
fn the_fifth_close_in_a_month_flags_the_account() {
    let (c, _d) = open();
    user(&c, "a");
    let mut ids = vec![];
    for i in 0..CIRCLE_CLOSES_FLAG {
        ids.push(make(&c, "a", &format!("c{i}"), 1_000 + i));
    }
    let x = PoolCtx::new(&c, 5_000);
    for (i, id) in ids.iter().enumerate() {
        close_circle(&x, "a", id).unwrap();
        let flags: i64 = c.query_row("SELECT COUNT(*) FROM admin_flags WHERE kind = 'cap' AND user_id = 'a'", [], |r| r.get(0)).unwrap();
        assert_eq!(flags, if i as i64 + 1 >= CIRCLE_CLOSES_FLAG { 1 } else { 0 }, "after close {}", i + 1);
    }
}
