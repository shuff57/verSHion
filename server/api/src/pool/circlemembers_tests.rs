//! Tests for `circlemembers.rs` on the real migrated schema: leaving (and the succession it triggers), removal, roles, and the
//! offer/accept/cancel ownership transfer.
use super::*;
use crate::db;
use crate::pool::circleinvites::create_invite;
use crate::pool::circles::create_circle;
use crate::pool::ctx::Fail;
use std::path::Path;

fn open() -> (Connection, tempfile::TempDir) {
    let dir = tempfile::tempdir().unwrap();
    let mut conn = db::open(&dir.path().join("t.db")).unwrap();
    db::migrate(&mut conn, &Path::new(env!("CARGO_MANIFEST_DIR")).join("../worker/migrations")).unwrap();
    (conn, dir)
}
/// User ids are the 36-character form the routes accept.
fn uid(n: u8) -> String {
    format!("{:08x}-0000-0000-0000-{:012x}", n, n)
}
fn user(c: &Connection, n: u8) -> String {
    let u = uid(n);
    c.execute("INSERT INTO users (id, email, display_name, trust, created_at) VALUES (?1, ?2, ?3, 'teacher', 1)", params![u, format!("u{n}@x.org"), format!("Teacher {n}")]).unwrap();
    u
}
fn m(v: Value) -> Map<String, Value> {
    v.as_object().unwrap().clone()
}
fn err(f: crate::pool::ctx::Fail) -> (u16, String, Value) {
    match f {
        Fail::Pool(e) => (e.status, e.code.clone(), e.body()),
        other => panic!("{other:?}"),
    }
}
fn make(c: &Connection, u: &str) -> String {
    create_circle(&PoolCtx::new(c, 1_000), u, &m(json!({ "name": "Team" }))).unwrap().1["circle"]["id"].as_str().unwrap().to_string()
}
fn join(c: &Connection, cid: &str, u: &str, role: &str, seq: i64) {
    c.execute("INSERT INTO circle_members (circle_id, user_id, role, joined_at, join_seq) VALUES (?1, ?2, ?3, 10, ?4)", params![cid, u, role, seq]).unwrap();
}
fn role_of(c: &Connection, cid: &str, u: &str) -> Option<String> {
    c.query_row("SELECT role FROM circle_members WHERE circle_id = ?1 AND user_id = ?2 AND left_at IS NULL", params![cid, u], |r| r.get(0)).optional().unwrap()
}
fn count(c: &Connection, sql: &str) -> i64 {
    c.query_row(sql, [], |r| r.get(0)).unwrap()
}

#[test]
fn a_member_leaves_and_their_codes_and_offers_go() {
    let (c, _d) = open();
    let (a, b) = (user(&c, 1), user(&c, 2));
    let cid = make(&c, &a);
    join(&c, &cid, &b, "editor", 5);
    let x = PoolCtx::new(&c, 3_000);
    c.execute("INSERT INTO circle_transfers (circle_id, from_user, to_user, created_at, expires_at) VALUES (?1, ?2, ?3, 1, 99999)", params![cid, a, b]).unwrap();
    assert_eq!(leave_circle(&x, &b, &cid).unwrap(), json!({ "ok": true }));
    assert!(role_of(&c, &cid, &b).is_none());
    assert_eq!(count(&c, "SELECT COUNT(*) FROM circle_transfers"), 0, "an offer to the leaver goes");
    assert_eq!(count(&c, "SELECT COUNT(*) FROM pool_audit WHERE action = 'circle.leave'"), 1);
    let reason: String = c.query_row("SELECT left_reason FROM circle_members WHERE user_id = ?1", [&b], |r| r.get(0)).unwrap();
    assert_eq!(reason, "left");
    // Having left, the circle is a 404 for them.
    assert_eq!(err(leave_circle(&x, &b, &cid).unwrap_err()).1, "not_found");
}

#[test]
fn an_owner_leaving_hands_the_circle_to_the_longest_standing_editor_in_good_standing() {
    let (c, _d) = open();
    let (a, b, d, e) = (user(&c, 1), user(&c, 2), user(&c, 3), user(&c, 4));
    let cid = make(&c, &a);
    join(&c, &cid, &b, "viewer", 4);
    join(&c, &cid, &d, "editor", 6);
    join(&c, &cid, &e, "editor", 5);
    c.execute("UPDATE users SET suspended_at = 1 WHERE id = ?1", [&e]).unwrap();
    // The owner's open code goes with the role.
    let inv = create_invite(&PoolCtx::new(&c, 2_000), &a, &cid, &m(json!({ "role": "viewer" }))).unwrap().1;
    let x = PoolCtx::new(&c, 3_000);
    leave_circle(&x, &a, &cid).unwrap();
    assert_eq!(role_of(&c, &cid, &d).as_deref(), Some("owner"), "the suspended editor is passed over");
    assert_eq!(role_of(&c, &cid, &b).as_deref(), Some("viewer"));
    let revoked: Option<i64> = c.query_row("SELECT revoked_at FROM circle_invites WHERE id = ?1", [inv["invite_id"].as_str().unwrap()], |r| r.get(0)).unwrap();
    assert_eq!(revoked, Some(3_000));
    assert_eq!(count(&c, "SELECT COUNT(*) FROM pool_audit WHERE action = 'circle.owner_succession' AND actor_kind = 'system'"), 1);
    // Next time, with no editor in good standing, a viewer takes over.
    leave_circle(&x, &d, &cid).unwrap();
    assert_eq!(role_of(&c, &cid, &b).as_deref(), Some("owner"));
}

#[test]
fn the_last_member_leaving_closes_the_circle_for_purge() {
    let (c, _d) = open();
    let a = user(&c, 1);
    let cid = make(&c, &a);
    leave_circle(&PoolCtx::new(&c, 3_000), &a, &cid).unwrap();
    let row: (Option<i64>, Option<String>, Option<i64>, Option<i64>) = c.query_row("SELECT closed_at, close_reason, undo_until, purge_after FROM circles WHERE id = ?1", [&cid], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?))).unwrap();
    assert_eq!(row, (Some(3_000), Some("last_member_gone".into()), None, Some(3_000 + CIRCLE_UNDO_MS)));
    assert_eq!(count(&c, "SELECT COUNT(*) FROM pool_audit WHERE action = 'circle.close' AND actor_kind = 'system'"), 1);
}

#[test]
fn succession_leaves_a_circle_with_an_active_owner_alone_and_reruns_safely() {
    let (c, _d) = open();
    let (a, b) = (user(&c, 1), user(&c, 2));
    let cid = make(&c, &a);
    join(&c, &cid, &b, "editor", 5);
    succession(&c, "SELECT ?1", &[&cid], 4_000, 9_000).unwrap();
    assert_eq!(role_of(&c, &cid, &a).as_deref(), Some("owner"));
    assert_eq!(count(&c, "SELECT COUNT(*) FROM pool_audit WHERE action = 'circle.owner_succession'"), 0);
    // An owner who is gone without leaving (erased): the editor takes over once, and a rerun does nothing more.
    c.execute("UPDATE circle_members SET left_at = 1, left_reason = 'account_deleted' WHERE user_id = ?1", [&a]).unwrap();
    succession(&c, "SELECT ?1", &[&cid], 4_000, 9_000).unwrap();
    succession(&c, "SELECT ?1", &[&cid], 4_000, 9_000).unwrap();
    assert_eq!(role_of(&c, &cid, &b).as_deref(), Some("owner"));
    assert_eq!(count(&c, "SELECT COUNT(*) FROM pool_audit WHERE action = 'circle.owner_succession'"), 1);
    // An owner-closed circle inside its undo window keeps its closure but gets a successor.
    c.execute("UPDATE circle_members SET role = 'editor' WHERE user_id = ?1", [&b]).unwrap();
    c.execute("UPDATE circles SET closed_at = 1, close_reason = 'owner_closed', undo_until = 5000 WHERE id = ?1", [&cid]).unwrap();
    succession(&c, "SELECT ?1", &[&cid], 4_000, 9_000).unwrap();
    assert_eq!(role_of(&c, &cid, &b).as_deref(), Some("owner"));
    let reason: String = c.query_row("SELECT close_reason FROM circles WHERE id = ?1", [&cid], |r| r.get(0)).unwrap();
    assert_eq!(reason, "owner_closed");
}

#[test]
fn the_owner_removes_a_member_immediately_and_revokes_their_codes() {
    let (c, _d) = open();
    let (a, b, d) = (user(&c, 1), user(&c, 2), user(&c, 3));
    let cid = make(&c, &a);
    join(&c, &cid, &b, "editor", 5);
    join(&c, &cid, &d, "viewer", 6);
    c.execute("INSERT INTO circle_invites (id, circle_id, code_hash, role, created_by, created_at, expires_at) VALUES ('i1', ?1, 'h', 'viewer', ?2, 1, 99999)", params![cid, b]).unwrap();
    let x = PoolCtx::new(&c, 3_000);
    let go = |who: &str| remove_member(&x, &a, &cid, &m(json!({ "user_id": who })));
    assert_eq!(go(&b).unwrap(), json!({ "ok": true }));
    assert!(role_of(&c, &cid, &b).is_none());
    let (reason, at): (String, i64) = c.query_row("SELECT left_reason, left_at FROM circle_members WHERE user_id = ?1", [&b], |r| Ok((r.get(0)?, r.get(1)?))).unwrap();
    assert_eq!((reason.as_str(), at), ("removed", 3_000));
    let rev: Option<i64> = c.query_row("SELECT revoked_at FROM circle_invites WHERE id = 'i1'", [], |r| r.get(0)).unwrap();
    assert_eq!(rev, Some(3_000));
    // Refusals: yourself, a non-member, a malformed id, a non-owner.
    assert_eq!(err(go(&a).unwrap_err()).2["field"], json!("user_id"));
    assert_eq!(err(go(&b).unwrap_err()).1, "not_found");
    assert_eq!(err(go("nope").unwrap_err()).2["field"], json!("user_id"));
    assert_eq!(err(remove_member(&x, &d, &cid, &m(json!({ "user_id": a }))).unwrap_err()).1, "circle_role_required");
}

#[test]
fn roles_change_between_editor_and_viewer_only() {
    let (c, _d) = open();
    let (a, b) = (user(&c, 1), user(&c, 2));
    let cid = make(&c, &a);
    join(&c, &cid, &b, "editor", 5);
    let x = PoolCtx::new(&c, 3_000);
    let set = |who: &str, role: &str| set_role(&x, &a, &cid, &m(json!({ "user_id": who, "role": role })));
    assert_eq!(set(&b, "viewer").unwrap(), json!({ "ok": true, "changed": true }));
    assert_eq!(role_of(&c, &cid, &b).as_deref(), Some("viewer"));
    assert_eq!(set(&b, "viewer").unwrap(), json!({ "ok": true, "changed": false }));
    assert_eq!(count(&c, "SELECT COUNT(*) FROM pool_audit WHERE action = 'circle.role'"), 1);
    assert_eq!(err(set(&b, "owner").unwrap_err()).2["field"], json!("role"));
    assert_eq!(err(set(&a, "viewer").unwrap_err()).2["field"], json!("user_id"));
    assert_eq!(err(set(&uid(9), "viewer").unwrap_err()).1, "not_found");
    assert_eq!(err(set_role(&x, &b, &cid, &m(json!({ "user_id": a, "role": "viewer" }))).unwrap_err()).1, "circle_role_required");
}

#[test]
fn an_ownership_offer_is_idempotent_replaceable_and_needs_a_member_in_good_standing() {
    let (c, _d) = open();
    let (a, b, d, o) = (user(&c, 1), user(&c, 2), user(&c, 3), user(&c, 4));
    let cid = make(&c, &a);
    join(&c, &cid, &b, "editor", 5);
    join(&c, &cid, &d, "viewer", 6);
    let x = PoolCtx::new(&c, 3_000);
    let offer = |who: &str| offer_transfer(&x, &a, &cid, &m(json!({ "user_id": who })));
    let (s, v) = offer(&b).unwrap();
    assert_eq!((s, v["pending"]["to_user_id"].clone(), v["pending"]["to_name"].clone(), v["pending"]["expires_at"].clone()), (202, json!(b), json!("Teacher 2"), json!(3_000 + TRANSFER_OFFER_MS)));
    // The same open offer again writes nothing and is not charged.
    let used = |c: &Connection| count(c, "SELECT COALESCE((SELECT count FROM usage_daily WHERE metric = 'circle_transfer_offers'), 0)");
    let before = used(&c);
    offer(&b).unwrap();
    assert_eq!(used(&c), before);
    assert_eq!(count(&c, "SELECT COUNT(*) FROM pool_audit WHERE action = 'circle.transfer_offer'"), 1);
    // A different member replaces the pending offer.
    offer(&d).unwrap();
    let to: String = c.query_row("SELECT to_user FROM circle_transfers WHERE circle_id = ?1", [&cid], |r| r.get(0)).unwrap();
    assert_eq!(to, d);
    // Refusals: yourself, a non-member, a suspended member (the day is given back).
    assert_eq!(err(offer(&a).unwrap_err()).2["field"], json!("user_id"));
    assert_eq!(err(offer(&o).unwrap_err()).1, "not_found");
    c.execute("UPDATE users SET suspended_at = 1 WHERE id = ?1", [&b]).unwrap();
    let before = used(&c);
    assert_eq!(err(offer(&b).unwrap_err()).1, "member_unavailable");
    assert_eq!(used(&c), before);
}

#[test]
fn accepting_swaps_the_roles_revokes_the_old_owners_codes_and_uses_the_offer() {
    let (c, _d) = open();
    let (a, b, d) = (user(&c, 1), user(&c, 2), user(&c, 3));
    let cid = make(&c, &a);
    join(&c, &cid, &b, "editor", 5);
    join(&c, &cid, &d, "viewer", 6);
    let inv = create_invite(&PoolCtx::new(&c, 2_000), &a, &cid, &m(json!({ "role": "viewer" }))).unwrap().1;
    let x = PoolCtx::new(&c, 3_000);
    offer_transfer(&x, &a, &cid, &m(json!({ "user_id": b }))).unwrap();
    // Only the member offered it can accept.
    assert_eq!(err(accept_transfer(&x, &d, &cid).unwrap_err()).1, "no_pending_transfer");
    assert_eq!(err(accept_transfer(&x, &a, &cid).unwrap_err()).1, "no_pending_transfer");
    assert_eq!(accept_transfer(&x, &b, &cid).unwrap(), json!({ "ok": true, "role": "owner" }));
    assert_eq!((role_of(&c, &cid, &b).as_deref(), role_of(&c, &cid, &a).as_deref()), (Some("owner"), Some("editor")));
    assert_eq!(count(&c, "SELECT COUNT(*) FROM circle_transfers"), 0);
    let revoked: Option<i64> = c.query_row("SELECT revoked_at FROM circle_invites WHERE id = ?1", [inv["invite_id"].as_str().unwrap()], |r| r.get(0)).unwrap();
    assert_eq!(revoked, Some(3_000));
    let who: String = c.query_row("SELECT actor_id FROM pool_audit WHERE action = 'circle.transfer'", [], |r| r.get(0)).unwrap();
    assert_eq!(who, a, "the audit names the old owner as actor");
    assert_eq!(err(accept_transfer(&x, &b, &cid).unwrap_err()).1, "no_pending_transfer");
}

#[test]
fn an_expired_offer_cannot_be_accepted_and_the_owned_cap_stops_a_taker() {
    let (c, _d) = open();
    let (a, b) = (user(&c, 1), user(&c, 2));
    let cid = make(&c, &a);
    join(&c, &cid, &b, "editor", 5);
    let x = PoolCtx::new(&c, 3_000);
    offer_transfer(&x, &a, &cid, &m(json!({ "user_id": b }))).unwrap();
    let late = PoolCtx::new(&c, 3_000 + TRANSFER_OFFER_MS);
    assert_eq!(err(accept_transfer(&late, &b, &cid).unwrap_err()).1, "no_pending_transfer");
    // The taker already owns the most circles allowed.
    let cap = x.limit_of(&b, "circles_owned").unwrap();
    c.execute("INSERT INTO pool_limits (user_id, metric, value, updated_at, updated_by) VALUES ('*', 'circle_creates', 50, 1, 'adm')", []).unwrap();
    for _ in 0..cap {
        make(&c, &b);
    }
    let (s, code, body) = err(accept_transfer(&x, &b, &cid).unwrap_err());
    assert_eq!((s, code.as_str(), body["metric"].clone()), (422, "circles_limit", json!("circles_owned")));
    assert_eq!(role_of(&c, &cid, &a).as_deref(), Some("owner"), "nothing changed, the step-down was undone");
}

#[test]
fn either_side_cancels_a_pending_offer() {
    let (c, _d) = open();
    let (a, b, d) = (user(&c, 1), user(&c, 2), user(&c, 3));
    let cid = make(&c, &a);
    join(&c, &cid, &b, "editor", 5);
    join(&c, &cid, &d, "viewer", 6);
    let x = PoolCtx::new(&c, 3_000);
    let offer = || offer_transfer(&x, &a, &cid, &m(json!({ "user_id": b }))).unwrap();
    offer();
    assert_eq!(cancel_transfer(&x, &d, &cid).unwrap(), json!({ "ok": true, "cancelled": false }), "an uninvolved member cancels nothing");
    assert_eq!(cancel_transfer(&x, &b, &cid).unwrap(), json!({ "ok": true, "cancelled": true }), "the member offered it declines");
    offer();
    assert_eq!(cancel_transfer(&x, &a, &cid).unwrap(), json!({ "ok": true, "cancelled": true }), "the owner withdraws it");
    assert_eq!(cancel_transfer(&x, &a, &cid).unwrap(), json!({ "ok": true, "cancelled": false }));
    assert_eq!(count(&c, "SELECT COUNT(*) FROM pool_audit WHERE action = 'circle.transfer_cancel'"), 2);
}
