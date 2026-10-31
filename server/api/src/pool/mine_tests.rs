//! Tests for `mine.rs` on the real migrated schema: the states an author sees and the windows each one carries.
use super::*;
use crate::db;
use rusqlite::Connection;
use std::path::Path;

fn open() -> (Connection, tempfile::TempDir) {
    let dir = tempfile::tempdir().unwrap();
    let mut conn = db::open(&dir.path().join("t.db")).unwrap();
    db::migrate(&mut conn, &Path::new(env!("CARGO_MANIFEST_DIR")).join("../worker/migrations")).unwrap();
    (conn, dir)
}
fn user(c: &Connection, id: &str) {
    c.execute("INSERT INTO users (id, email, display_name, trust, created_at) VALUES (?1, ?2, ?1, 'teacher', 1)", params![id, format!("{id}@x.org")]).unwrap();
}
fn item(c: &Connection, id: &str, updated: i64) {
    c.execute(
        "INSERT INTO items (id, author_id, kind, visibility, current_publish_id, public_publish_id, public_version, version, title, license, committed_at, created_at, updated_at, fold_rev)
         VALUES (?1, 'a', 'bank', 'public', ?2, ?2, 1, 1, 'T', 'CC-BY-4.0', 100, 1, ?3, 1)",
        params![id, format!("p{id}"), updated],
    )
    .unwrap();
}
fn id(n: u8) -> String {
    format!("{:032x}", n)
}
fn find<'a>(v: &'a Value, id: &str) -> &'a Value {
    v["items"].as_array().unwrap().iter().find(|x| x["id"] == id).unwrap_or_else(|| panic!("{id} not in {v}"))
}
fn setup() -> (Connection, tempfile::TempDir) {
    let (c, d) = open();
    user(&c, "a");
    user(&c, "b");
    (c, d)
}

#[test]
fn a_live_item_lists_with_its_import_count_and_the_storage_used() {
    let (c, _d) = setup();
    item(&c, &id(1), 10);
    for u in ["b", "a"] {
        c.execute("INSERT INTO imports (user_id, item_id, publish_id, version, form, license, author_id, at, kind, real_publish_id, licenses_json, first_at) VALUES (?1, ?2, 'px', 1, 'full', 'CC-BY-4.0', 'a', 5, 'import', 'px', '[]', 5)", params![u, id(1)]).unwrap();
    }
    let x = PoolCtx::new(&c, 1_000);
    let out = mine(&x, "a", None).unwrap();
    let it = find(&out, &id(1));
    assert_eq!((it["state"].clone(), it["import_count"].clone(), it["hide_key"].clone()), (json!("live"), json!(1), json!(false)));
    assert_eq!(out["next_cursor"], Value::Null);
    assert_eq!(out["storage"]["limit"], json!(200 * 1024 * 1024));
    assert!(out["storage"]["used"].as_i64().unwrap() >= 1024, "1 KB per item at least");
    assert!(mine(&x, "b", None).unwrap()["items"].as_array().unwrap().is_empty(), "nobody else's items");
}

#[test]
fn newest_first_and_a_cursor_pages_on_updated_at_then_id() {
    let (c, _d) = setup();
    for (n, u) in [(1u8, 10), (2, 30), (3, 20), (4, 30)] {
        item(&c, &id(n), u);
    }
    let x = PoolCtx::new(&c, 1_000);
    let out = mine(&x, "a", None).unwrap();
    let ids: Vec<_> = out["items"].as_array().unwrap().iter().map(|i| i["id"].as_str().unwrap().to_string()).collect();
    assert_eq!(ids, vec![id(4), id(2), id(3), id(1)]);
    let out = mine(&x, "a", Some(&format!("30:{}", id(4)))).unwrap();
    let ids: Vec<_> = out["items"].as_array().unwrap().iter().map(|i| i["id"].as_str().unwrap().to_string()).collect();
    assert_eq!(ids, vec![id(2), id(3), id(1)]);
    // A malformed cursor is the first page.
    assert_eq!(mine(&x, "a", Some("junk")).unwrap()["items"].as_array().unwrap().len(), 4);
}

#[test]
fn deletion_takedown_privacy_and_admin_removal_each_carry_their_own_window() {
    let (c, _d) = setup();
    for n in 1..=5u8 {
        item(&c, &id(n), n as i64);
    }
    c.execute("UPDATE items SET removed_at = 50, removed_reason = 'author_deleted', purge_after = 5000 WHERE id = ?1", [id(1)]).unwrap();
    c.execute("UPDATE items SET removed_at = 50, removed_reason = 'takedown' WHERE id = ?1", [id(2)]).unwrap();
    c.execute("INSERT INTO dmca_actions (id, notice_id, item_id, author_id, disabled_at, author_notified_at) VALUES ('act', 'n', ?1, 'a', 40, 900)", [id(2)]).unwrap();
    c.execute("UPDATE items SET removed_at = 900, removed_reason = 'privacy', purged_at = 901, current_publish_id = NULL WHERE id = ?1", [id(3)]).unwrap();
    c.execute("UPDATE items SET removed_at = 50, removed_reason = 'admin' WHERE id = ?1", [id(4)]).unwrap();
    let x = PoolCtx::new(&c, 1_000);
    let out = mine(&x, "a", None).unwrap();
    assert_eq!(find(&out, &id(1))["state"], "deleted");
    assert_eq!(find(&out, &id(1))["undo_until"], 5000);
    let td = find(&out, &id(2));
    assert_eq!((td["state"].clone(), td["counter_notice"].clone(), td["counter_notice_by"].clone()), (json!("taken_down"), json!("available"), json!(900 + QUARANTINE_MS)));
    let pv = find(&out, &id(3));
    assert_eq!((pv["state"].clone(), pv["appeal"].clone(), pv["appeal_until"].clone()), (json!("removed_privacy"), json!("available"), json!(900 + PRIVACY_APPEAL_MS)));
    let ad = find(&out, &id(4));
    assert_eq!((ad["state"].clone(), ad["removed_reason"].clone(), ad["appeal"].clone()), (json!("removed"), json!("admin"), json!("available")));
    // The undo window is only offered while it is open; the counter-notice form only inside its 30 days or once sent.
    let late = PoolCtx::new(&c, 900 + QUARANTINE_MS + 1);
    let out = mine(&late, "a", None).unwrap();
    assert_eq!(find(&out, &id(1)).get("undo_until"), None);
    assert_eq!(find(&out, &id(2))["counter_notice"], "closed");
    assert!(find(&out, &id(2)).get("counter_notice_by").is_none());
    c.execute("UPDATE dmca_actions SET counter_notice_id = 'cn'", []).unwrap();
    assert_eq!(find(&mine(&late, "a", None).unwrap(), &id(2))["counter_notice"], "received");
}

#[test]
fn a_purged_privacy_removal_drops_off_after_its_appeal_window() {
    let (c, _d) = setup();
    item(&c, &id(1), 1);
    c.execute("UPDATE items SET removed_at = 900, removed_reason = 'privacy', purged_at = 901, current_publish_id = NULL", []).unwrap();
    assert_eq!(mine(&PoolCtx::new(&c, 1_000), "a", None).unwrap()["items"].as_array().unwrap().len(), 1);
    assert!(mine(&PoolCtx::new(&c, 900 + PRIVACY_APPEAL_MS + 1), "a", None).unwrap()["items"].as_array().unwrap().is_empty());
}

#[test]
fn holds_are_neutral_unless_an_admin_placed_them_and_each_has_its_own_state() {
    let (c, _d) = setup();
    for n in 1..=4u8 {
        item(&c, &id(n), n as i64);
    }
    c.execute("INSERT INTO item_holds (item_id, reason, at, source) VALUES (?1, 'reports', 1, 'needs_reupload')", [id(1)]).unwrap();
    c.execute("INSERT INTO item_holds (item_id, reason, at, source) VALUES (?1, 'reports', 1, 'key_leak')", [id(2)]).unwrap();
    c.execute("INSERT INTO item_holds (item_id, reason, at, source) VALUES (?1, 'blocklist', 1, '')", [id(3)]).unwrap();
    let snap = json!({"publish_id": "p".to_string() + &id(4), "positions": [0, 2]}).to_string();
    c.execute("INSERT INTO item_holds (item_id, reason, at, by, source, detail) VALUES (?1, 'student_data', 1, 'adm', 'privacy', ?2)", params![id(4), snap]).unwrap();
    let x = PoolCtx::new(&c, 1_000);
    let out = mine(&x, "a", None).unwrap();
    assert_eq!(find(&out, &id(1))["state"], "needs_reupload");
    assert_eq!(find(&out, &id(2))["state"], "key_leak_review");
    let neutral = find(&out, &id(3));
    assert_eq!((neutral["state"].clone(), neutral["appeal"].clone()), (json!("held_for_review"), json!("available")));
    assert!(neutral.get("hold_class").is_none() && neutral.get("notice").is_none(), "no block-membership oracle");
    let admin = find(&out, &id(4));
    assert_eq!((admin["state"].clone(), admin["hold_class"].clone(), admin["positions"].clone()), (json!("held_for_review"), json!("student_data"), json!([0, 2])));
    // The snapshot only counts while the item still has the content the admin saw.
    c.execute("UPDATE items SET current_publish_id = 'other' WHERE id = ?1", [id(4)]).unwrap();
    assert!(find(&mine(&x, "a", None).unwrap(), &id(4)).get("positions").is_none());
}

#[test]
fn a_takedown_of_an_already_purged_item_is_listed_on_the_first_page_only() {
    let (c, _d) = setup();
    item(&c, &id(1), 1);
    c.execute("UPDATE items SET removed_at = 5, removed_reason = 'author_deleted', purged_at = 6, current_publish_id = NULL WHERE id = ?1", [id(1)]).unwrap();
    c.execute("INSERT INTO dmca_actions (id, notice_id, item_id, author_id, disabled_at, author_notified_at) VALUES ('act', 'n', ?1, 'a', 40, 900)", [id(1)]).unwrap();
    let x = PoolCtx::new(&c, 1_000);
    let out = mine(&x, "a", None).unwrap();
    let g = &out["items"][0];
    assert_eq!((g["state"].clone(), g["action_id"].clone(), g["counter_notice"].clone(), g["counter_notice_sent"].clone()), (json!("takedown_pending"), json!("act"), json!("available"), json!(false)));
    assert_eq!(g["counter_notice_by"], json!(900 + QUARANTINE_MS));
    let page2 = mine(&x, "a", Some(&format!("1:{}", id(9)))).unwrap();
    assert!(page2["items"].as_array().unwrap().iter().all(|i| i["state"] != "takedown_pending"));
}

#[test]
fn the_window_rule_and_the_cursor_syntax() {
    assert!(counter_window_open(Some(1_000), 1_000 + QUARANTINE_MS - 1));
    assert!(!counter_window_open(Some(1_000), 1_000 + QUARANTINE_MS));
    assert!(!counter_window_open(None, 0));
    assert_eq!(parse_after(Some(&format!("12:{}", id(1)))), Some((12, id(1))));
    for bad in ["", "x", "1:abc", "12345678901234567:00000000000000000000000000000001", ":00000000000000000000000000000001"] {
        assert_eq!(parse_after(Some(bad)), None, "{bad}");
    }
    assert_eq!(parse_after(None), None);
}
