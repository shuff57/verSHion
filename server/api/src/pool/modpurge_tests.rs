//! Tests for `modpurge.rs`: a purge is claimed only when due, leaves a tombstone, ghosts what was served, and an upheld takedown strikes and holds copies.
use super::*;
use crate::db;
use rusqlite::Connection;
use std::path::Path;

fn world() -> (Connection, tempfile::TempDir, BlobStore) {
    let d = tempfile::tempdir().unwrap();
    let mut c = db::open(&d.path().join("t.db")).unwrap();
    db::migrate(&mut c, &Path::new(env!("CARGO_MANIFEST_DIR")).join("../worker/migrations")).unwrap();
    for u in ["a", "b"] {
        c.execute("INSERT INTO users (id, email, display_name, trust, created_at) VALUES (?1, ?2, ?3, 'teacher', 1)", params![u, format!("{u}@x.org"), u]).unwrap();
    }
    let s = BlobStore::new(d.path().join("blobs")).unwrap();
    (c, d, s)
}
fn item(c: &Connection, id: &str, author: &str, publish: &str, hashes: &[&str], origin: Option<&str>) {
    c.execute("INSERT INTO items (id, author_id, kind, visibility, current_publish_id, license, title, created_at, updated_at) VALUES (?1, ?2, 'bank', 'public', ?3, 'cc-by', ?1, 1, 1)", params![id, author, publish]).unwrap();
    c.execute(
        "INSERT INTO item_publishes (publish_id, item_id, author_id, version, status, schema_version, hide_key, license, attested_at, attest_text_rev, acks_json, request_sha256, stage_token,
           full_key, full_bytes, title, subject, course, grade, kinds_mask, question_count, has_images, created_at, committed_at)
         VALUES (?1, ?2, ?3, 1, 'current', 1, 0, 'cc-by', 1, 'r', '{}', 'x', 't', ?4, 1, 'T', '', '', '', 0, 0, 0, 1, 1)",
        params![publish, id, author, format!("items/{id}/{publish}.json")],
    )
    .unwrap();
    for (i, h) in hashes.iter().enumerate() {
        c.execute("INSERT INTO item_questions (publish_id, qid, pos, qkind, match_hash, full_sha256, full_content, keyfree_content, origin_item_id) VALUES (?1, ?2, ?3, 'mc', ?4, 'f', 'fc', 'kc', ?5)", params![publish, format!("{publish}q{i}"), i as i64, h, origin]).unwrap();
    }
}
fn n(c: &Connection, sql: &str) -> i64 {
    c.query_row(sql, [], |r| r.get(0)).unwrap()
}
fn notice(c: &Connection) {
    c.execute("INSERT INTO dmca_notices (id, received_at, complainant_name, complainant_contact, work_desc, material_desc, has_signature, has_good_faith, has_perjury_stmt, status, recorded_by) VALUES ('n', 1, 'C', 'c@x', 'w', 'm', 1, 1, 1, 'actioned', 'admin')", []).unwrap();
}
fn takedown(c: &Connection, rightful: Option<&str>) {
    notice(c);
    c.execute("INSERT INTO dmca_actions (id, notice_id, item_id, author_id, disabled_at, author_notified_at, rightful_item_id) VALUES ('act', 'n', 'src', 'a', 1, 1, ?1)", [rightful]).unwrap();
    c.execute("INSERT INTO strikes (id, user_id, action_id, at) VALUES ('st', 'a', 'act', 1)", []).unwrap();
    c.execute("UPDATE items SET removed_at = 1, removed_reason = 'takedown', purge_after = 100 WHERE id = 'src'", []).unwrap();
}
const SYS: Actor = Actor { id: None, kind: "system" };

#[test]
fn a_purge_is_claimed_only_when_due_and_leaves_a_tombstone() {
    let (c, _d, s) = world();
    item(&c, "i1", "a", "p1", &["h1"], None);
    s.put("items/i1/p1.json", b"x").unwrap();
    c.execute("UPDATE items SET removed_at = 1, removed_reason = 'author_deleted', purge_after = 500 WHERE id = 'i1'", []).unwrap();
    assert!(!purge_item(&PoolCtx::new(&c, 100), &s, "i1", &SYS, "retention").unwrap());
    assert!(purge_item(&PoolCtx::new(&c, 600), &s, "i1", &SYS, "retention").unwrap());
    assert_eq!((n(&c, "SELECT purged_at FROM items"), n(&c, "SELECT COUNT(*) FROM item_publishes"), n(&c, "SELECT COUNT(*) FROM item_questions")), (600, 0, 0));
    assert_eq!(c.query_row::<String, _, _>("SELECT title FROM items", [], |r| r.get(0)).unwrap(), "[removed]");
    assert!(s.get("items/i1/p1.json").unwrap().is_none());
    assert_eq!(n(&c, "SELECT COUNT(*) FROM r2_orphans"), 0);
    assert_eq!(n(&c, "SELECT COUNT(*) FROM pool_audit WHERE action = 'item.purge'"), 1);
    // Not claimable twice.
    assert!(!purge_item(&PoolCtx::new(&c, 700), &s, "i1", &SYS, "retention").unwrap());
    // A live item is never purged.
    item(&c, "i2", "a", "p2", &["h2"], None);
    assert!(!purge_item(&PoolCtx::new(&c, 700), &s, "i2", &SYS, "x").unwrap());
}

#[test]
fn an_upheld_takedown_voids_evidence_upholds_the_strike_and_holds_the_copies() {
    let (c, _d, s) = world();
    item(&c, "src", "a", "ps", &["h1"], None);
    item(&c, "cp", "b", "pc", &["h1"], Some("src"));
    takedown(&c, None);
    c.execute("INSERT INTO served (reader_id, item_id, publish_id, kind, license, licenses_json, first_at, at) VALUES ('b', 'src', 'ps', 'import', 'cc-by', '[]', 1, 1)", []).unwrap();
    assert!(purge_item(&PoolCtx::new(&c, 200), &s, "src", &SYS, "uphold").unwrap());
    assert_eq!(n(&c, "SELECT upheld_at FROM strikes"), 200);
    assert_eq!(n(&c, "SELECT COUNT(*) FROM void_items WHERE item_id = 'src'"), 1);
    assert_eq!(n(&c, "SELECT COUNT(*) FROM item_holds WHERE item_id = 'cp' AND source = 'dmca:act'"), 1);
    assert_eq!(n(&c, "SELECT COUNT(*) FROM admin_flags WHERE kind = 'dmca_copy'"), 1);
    assert_eq!(n(&c, "SELECT COUNT(*) FROM ghost_publishes WHERE publish_id = 'ps'"), 1, "a served publish leaves a ghost");
    assert_eq!(n(&c, "SELECT purged_at FROM dmca_actions"), 200);
}

#[test]
fn with_a_rightful_item_named_the_copies_are_not_held() {
    let (c, _d, s) = world();
    item(&c, "src", "a", "ps", &["h1"], None);
    item(&c, "orig", "b", "po", &["h1"], None);
    takedown(&c, Some("orig"));
    assert!(purge_item(&PoolCtx::new(&c, 200), &s, "src", &SYS, "uphold").unwrap());
    assert_eq!(n(&c, "SELECT COUNT(*) FROM item_holds"), 0);
    assert_eq!(n(&c, "SELECT COUNT(*) FROM void_items WHERE replaced_by = 'orig'"), 1);
}

#[test]
fn a_ghost_takedown_is_upheld_after_its_window() {
    let (c, _d, _s) = world();
    item(&c, "src", "a", "ps", &["h1"], None);
    c.execute("UPDATE items SET purged_at = 5, removed_at = 5, removed_reason = 'takedown', current_publish_id = NULL WHERE id = 'src'", []).unwrap();
    notice(&c);
    c.execute("INSERT INTO dmca_actions (id, notice_id, item_id, author_id, disabled_at, author_notified_at) VALUES ('act', 'n', 'src', 'a', 10, 10)", []).unwrap();
    c.execute("INSERT INTO strikes (id, user_id, action_id, at) VALUES ('st', 'a', 'act', 10)", []).unwrap();
    // Inside the window nothing happens.
    assert_eq!(ghost_upholds(&PoolCtx::new(&c, 10 + QUARANTINE_MS - 1)).unwrap(), 0);
    assert_eq!(ghost_upholds(&PoolCtx::new(&c, 10 + QUARANTINE_MS)).unwrap(), 1);
    assert_eq!((n(&c, "SELECT upheld_at IS NOT NULL FROM strikes"), n(&c, "SELECT uphold_pending FROM dmca_actions"), n(&c, "SELECT purged_at IS NOT NULL FROM dmca_actions")), (1, 0, 1));
    assert_eq!(ghost_upholds(&PoolCtx::new(&c, 10 + QUARANTINE_MS + 5)).unwrap(), 0);
}
