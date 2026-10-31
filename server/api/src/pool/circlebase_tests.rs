//! Tests for `circlebase.rs` on the real migrated schema: codes, membership lookups, the closed/role refusals, caps and the sequence.
use super::*;
use crate::db;
use crate::pool::ctx::Fail;
use rusqlite::Connection;
use serde_json::json;
use std::path::Path;

fn open() -> (Connection, tempfile::TempDir) {
    let dir = tempfile::tempdir().unwrap();
    let mut conn = db::open(&dir.path().join("t.db")).unwrap();
    db::migrate(&mut conn, &Path::new(env!("CARGO_MANIFEST_DIR")).join("../worker/migrations")).unwrap();
    (conn, dir)
}
fn id(n: u8) -> String {
    format!("{:032x}", n)
}
fn user(c: &Connection, u: &str) {
    c.execute("INSERT INTO users (id, email, display_name, trust, created_at) VALUES (?1, ?2, ?1, 'teacher', 1)", params![u, format!("{u}@x.org")]).unwrap();
}
fn circle(c: &Connection, n: u8) {
    c.execute("INSERT INTO circles (id, name, created_by, created_at, updated_at) VALUES (?1, ?2, 'a', 1, 1)", params![id(n), format!("Circle {n}")]).unwrap();
}
fn member(c: &Connection, circ: u8, u: &str, role: &str, left: bool) {
    c.execute(
        "INSERT INTO circle_members (circle_id, user_id, role, joined_at, join_seq, left_at, left_reason) VALUES (?1, ?2, ?3, 7, 1, ?4, ?5)",
        params![id(circ), u, role, left.then_some(9), left.then_some("left")],
    )
    .unwrap();
}
fn pool_err(f: Fail) -> (u16, String, Value) {
    match f {
        Fail::Pool(e) => (e.status, e.code.clone(), e.body()),
        other => panic!("{other:?}"),
    }
}

#[test]
fn a_new_circle_code_normalises_back_and_hashes_with_its_own_prefix() {
    let code = new_circle_code();
    assert!(code.starts_with("C-") && code.len() == 2 + 24 + 5, "{code}");
    let n = normalise_circle_code(&json!(code)).unwrap();
    assert_eq!(n.len(), 25);
    assert!(n.starts_with('C'));
    // Case, spaces and hyphens are ignored; I/L read as 1, O as 0.
    assert_eq!(normalise_circle_code(&json!(code.to_lowercase().replace('-', " "))).unwrap(), n);
    let raw = "c-0000-0000-0000-0000-0000-0000";
    assert_eq!(normalise_circle_code(&json!(raw)).unwrap(), format!("C{}", "0".repeat(24)));
    assert_eq!(normalise_circle_code(&json!("C-OOOO-IIII-LLLL-0000-0000-0000")).unwrap(), "C000011111111000000000000");
    for bad in [json!(5), json!(null), json!("X-0000-0000-0000-0000-0000-0000"), json!("C-0000"), json!("C-UUUU-0000-0000-0000-0000-0000"), json!("C".repeat(101))] {
        assert!(normalise_circle_code(&bad).is_none(), "{bad}");
    }
    // The hash is not the pool invite hash of the same text (the prefix keeps the two code spaces apart).
    assert_eq!(circle_code_hash(&n).len(), 64);
    assert_ne!(circle_code_hash(&n), super::super::invites::invite_hash(&n));
    assert_eq!(circle_code_hash("Cabc"), crate::pool::sqlfrag::hex(&sha2::Sha256::digest(b"circle:Cabc")));
}

#[test]
fn display_names_text_and_closers() {
    assert_eq!(name_of(Some("Ann"), None), "Ann");
    assert_eq!(name_of(Some("Ann"), Some(5)), "deleted teacher");
    assert_eq!(name_of(None, None), "deleted teacher");
    assert_eq!((text(None), text(Some("x"))), ("".to_string(), "x".to_string()));
    assert_eq!((closed_by(Some("owner_closed")), closed_by(Some("admin")), closed_by(Some("last_member_gone")), closed_by(Some("x")), closed_by(None)), (Some("owner"), Some("admin"), Some("last_member"), None, None));
    let (c, _d) = open();
    let v: Option<String> = c.query_row(&format!("SELECT {}", closed_by_sql("'owner_closed'")), [], |r| r.get(0)).unwrap();
    assert_eq!(v.as_deref(), Some("owner"));
}

#[test]
fn the_sequence_counts_up() {
    let (c, _d) = open();
    let x = PoolCtx::new(&c, 1);
    let a = next_seq(&x).unwrap();
    assert_eq!(next_seq(&x).unwrap(), a + 1);
}

#[test]
fn membership_is_active_only_and_a_miss_is_a_counted_404() {
    let (c, _d) = open();
    for u in ["a", "b"] {
        user(&c, u);
    }
    circle(&c, 1);
    member(&c, 1, "a", "owner", false);
    member(&c, 1, "b", "viewer", true);
    let x = PoolCtx::new(&c, 5_000);
    let m = member_of(&x, &id(1), "a").unwrap().unwrap();
    assert_eq!((m.name.as_str(), m.role.as_str(), m.joined_at), ("Circle 1", "owner", 7));
    assert!(member_of(&x, &id(1), "b").unwrap().is_none(), "a former member is not a member");
    assert_eq!(circle_or_miss(&x, &id(1), "a").unwrap().id, id(1));
    for cid in [id(1), id(2), "nope".to_string()] {
        let (s, code, _) = pool_err(circle_or_miss(&x, &cid, "b").unwrap_err());
        assert_eq!((s, code.as_str()), (404, "not_found"));
    }
    let n: i64 = c.query_row("SELECT count FROM usage_daily WHERE user_id = 'b' AND metric = 'item_miss'", [], |r| r.get(0)).unwrap();
    assert_eq!(n, 3, "each miss is counted");
    // A purged circle is gone for its members.
    c.execute("UPDATE circles SET purged_at = 1, name = NULL WHERE id = ?1", [id(1)]).unwrap();
    assert!(member_of(&x, &id(1), "a").unwrap().is_none());
}

#[test]
fn closed_and_role_refusals_say_what_is_needed() {
    let (c, _d) = open();
    user(&c, "a");
    circle(&c, 1);
    member(&c, 1, "a", "viewer", false);
    let x = PoolCtx::new(&c, 5_000);
    let mut m = member_of(&x, &id(1), "a").unwrap().unwrap();
    require_open(&m).unwrap();
    m.closed_at = Some(10);
    m.close_reason = Some("owner_closed".into());
    m.undo_until = Some(99);
    let (s, code, b) = pool_err(require_open(&m).unwrap_err());
    assert_eq!((s, code.as_str(), b["reason"].clone(), b["undo_until"].clone()), (409, "circle_closed", json!("owner_closed"), json!(99)));
    assert_eq!(b["message"], json!("The owner closed this circle."));
    m.close_reason = Some("admin".into());
    assert_eq!(pool_err(require_open(&m).unwrap_err()).2["message"], json!("An administrator closed this circle."));
    m.close_reason = Some("last_member_gone".into());
    assert_eq!(pool_err(require_open(&m).unwrap_err()).2["message"], json!("This circle closed when its last member left."));
    require_role(&m, &["viewer"]).unwrap();
    let (s, code, b) = pool_err(require_role(&m, &["owner", "editor"]).unwrap_err());
    assert_eq!((s, code.as_str(), b["needed"].clone(), b["role"].clone()), (403, "circle_role_required", json!("editor"), json!("viewer")));
    assert_eq!(pool_err(require_role(&m, &["owner"]).unwrap_err()).2["needed"], json!("owner"));
}

#[test]
fn explain_names_why_a_guarded_write_changed_nothing() {
    let (c, _d) = open();
    for u in ["a", "gone"] {
        user(&c, u);
    }
    circle(&c, 1);
    member(&c, 1, "a", "viewer", false);
    let x = PoolCtx::new(&c, 5_000);
    assert_eq!(pool_err(explain(&x, &id(1), "nobody", None)).1, "not_found");
    assert_eq!(pool_err(explain(&x, &id(1), "a", Some(&["owner"]))).1, "circle_role_required");
    assert_eq!(pool_err(explain(&x, &id(1), "a", None)).1, "conflict");
    c.execute("UPDATE circles SET closed_at = 3, close_reason = 'owner_closed', undo_until = 99 WHERE id = ?1", [id(1)]).unwrap();
    assert_eq!(pool_err(explain(&x, &id(1), "a", None)).1, "circle_closed");
    c.execute("UPDATE users SET deleted_at = 1, email = NULL, pw_hash = NULL, pw_params = NULL, display_name = NULL, profile = NULL WHERE id = 'gone'", []).unwrap();
    assert_eq!(pool_err(explain(&x, &id(1), "gone", None)).0, 401);
}

#[test]
fn the_limit_error_names_the_cap_that_was_reached() {
    let (c, _d) = open();
    user(&c, "a");
    let x = PoolCtx::new(&c, 5_000);
    let caps = Caps { owned: 1, memberships: 2, rows: 3 };
    // Own one circle: owning another hits circles_owned; merely joining hits memberships first at 2.
    circle(&c, 1);
    member(&c, 1, "a", "owner", false);
    let e = limit_error(&x, "a", caps, true).unwrap();
    assert_eq!(e.body()["metric"], json!("circles_owned"));
    assert_eq!((e.body()["used"].clone(), e.body()["limit"].clone()), (json!(1), json!(1)));
    assert_eq!(e.body()["message"], json!(owned_message(1, true)));
    assert!(owned_message(1, true).starts_with("You own 1 circles"));
    assert!(owned_message(3, false).starts_with("That would make 3+ circles"));
    circle(&c, 2);
    member(&c, 2, "a", "editor", false);
    assert_eq!(limit_error(&x, "a", caps, false).unwrap().body()["metric"], json!("circle_memberships"));
    // Two left rows more: the total of rows (3) is what remains reached.
    c.execute("UPDATE circle_members SET left_at = 5, left_reason = 'left' WHERE circle_id = ?1", [id(2)]).unwrap();
    circle(&c, 3);
    member(&c, 3, "a", "editor", true);
    let e = limit_error(&x, "a", caps, false).unwrap();
    assert_eq!(e.body()["metric"], json!("circle_memberships_total"));
    assert_eq!(e.body()["used"], json!(3));
    // The default caps are read from the limits table.
    let c2 = caps_of(&x, "a").unwrap();
    assert!(c2.owned > 0 && c2.memberships > 0 && c2.rows >= c2.memberships);
}
