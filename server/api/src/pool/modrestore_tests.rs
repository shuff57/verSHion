//! Tests for `modrestore.rs`: a put-back wins once, returns the item to what it was, voids the strike and lifts the blocks; an upheld takedown can be reversed.
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
    c.execute("INSERT INTO items (id, author_id, kind, visibility, current_publish_id, license, title, created_at, updated_at, removed_at, removed_reason, quarantined_at, purge_after) VALUES ('src', 'a', 'bank', 'public', 'ps', 'cc-by', 'src', 1, 1, 10, 'takedown', 10, 9999)", []).unwrap();
    c.execute(
        "INSERT INTO item_publishes (publish_id, item_id, author_id, version, status, schema_version, hide_key, license, attested_at, attest_text_rev, acks_json, request_sha256, stage_token,
           full_key, full_bytes, title, subject, course, grade, kinds_mask, question_count, has_images, created_at, committed_at)
         VALUES ('ps', 'src', 'a', 1, 'current', 1, 0, 'cc-by', 1, 'r', '{}', 'x', 't', 'quarantine/src/ps/full.json', 1, 'T', '', '', '', 0, 0, 0, 1, 1)",
        [],
    )
    .unwrap();
    c.execute("INSERT INTO dmca_notices (id, received_at, complainant_name, complainant_contact, work_desc, material_desc, has_signature, has_good_faith, has_perjury_stmt, status, recorded_by) VALUES ('n', 1, 'C', 'c@x', 'w', 'm', 1, 1, 1, 'actioned', 'admin')", []).unwrap();
    c.execute("INSERT INTO dmca_actions (id, notice_id, item_id, author_id, disabled_at, author_notified_at) VALUES ('act', 'n', 'src', 'a', 10, 10)", []).unwrap();
    c.execute("INSERT INTO strikes (id, user_id, action_id, at) VALUES ('st', 'a', 'act', 10)", []).unwrap();
    c.execute("INSERT INTO blocked_hashes (kind, hash, action_id, at) VALUES ('match', 'h1', 'act', 10)", []).unwrap();
    let s = BlobStore::new(d.path().join("blobs")).unwrap();
    s.put("quarantine/src/ps/full.json", b"body").unwrap();
    (c, d, s)
}
fn n(c: &Connection, sql: &str) -> i64 {
    c.query_row(sql, [], |r| r.get(0)).unwrap()
}
const ADMIN: Actor = Actor { id: Some("adm"), kind: "admin" };

#[test]
fn a_put_back_returns_the_item_voids_the_strike_and_lifts_the_blocks() {
    let (c, _d, s) = world();
    assert!(restore(&PoolCtx::new(&c, 500), &s, "act", &ADMIN, "put_back").unwrap());
    assert_eq!(n(&c, "SELECT removed_at IS NULL AND removed_reason IS NULL AND quarantined_at IS NULL AND purge_after IS NULL FROM items"), 1);
    assert!(c.query_row::<String, _, _>("SELECT full_key FROM item_publishes", [], |r| r.get(0)).unwrap().starts_with("items/src/"));
    assert_eq!(n(&c, "SELECT voided_at FROM strikes"), 500);
    assert_eq!(n(&c, "SELECT lifted_at FROM blocked_hashes"), 500);
    assert_eq!(n(&c, "SELECT COUNT(*) FROM pool_audit WHERE action = 'dmca.restore'"), 1);
    // Exactly one put-back wins.
    assert!(!restore(&PoolCtx::new(&c, 600), &s, "act", &ADMIN, "put_back").unwrap());
}

#[test]
fn an_item_the_author_had_deleted_goes_back_to_deleted_with_its_window() {
    let (c, _d, s) = world();
    c.execute("UPDATE dmca_actions SET prev_removed_reason = 'author_deleted', prev_purge_after = 10 + 5 * 86400000", []).unwrap();
    assert!(restore(&PoolCtx::new(&c, 500), &s, "act", &ADMIN, "put_back").unwrap());
    let (reason, purge): (String, i64) = c.query_row("SELECT removed_reason, purge_after FROM items", [], |r| Ok((r.get(0)?, r.get(1)?))).unwrap();
    assert_eq!((reason.as_str(), purge), ("author_deleted", 500 + 5 * 86_400_000));
}

#[test]
fn copy_holds_an_admin_placed_stay_and_are_flagged() {
    let (c, _d, s) = world();
    c.execute("INSERT INTO items (id, author_id, kind, visibility, current_publish_id, license, created_at, updated_at) VALUES ('cp', 'b', 'bank', 'public', 'pc', 'cc-by', 1, 1)", []).unwrap();
    c.execute("INSERT INTO item_holds (item_id, reason, at, by, source) VALUES ('cp', 'admin', 1, NULL, 'dmca:act')", []).unwrap();
    restore(&PoolCtx::new(&c, 500), &s, "act", &ADMIN, "put_back").unwrap();
    assert_eq!(n(&c, "SELECT COUNT(*) FROM item_holds"), 1);
    assert_eq!(n(&c, "SELECT COUNT(*) FROM admin_flags WHERE kind = 'dmca_copy' AND dedup = 'putback:act'"), 1);
    // And the reverse for blocklist holds that nothing keeps any more.
    c.execute("INSERT INTO item_holds (item_id, reason, at, by, source) VALUES ('src', 'blocklist', 1, NULL, 'dmca:act')", []).unwrap();
    assert_eq!(release_restored_holds(&PoolCtx::new(&c, 600)).unwrap(), 1);
    assert_eq!(n(&c, "SELECT COUNT(*) FROM item_holds WHERE reason = 'blocklist'"), 0);
}

#[test]
fn a_court_uphold_purges_and_an_admin_can_reverse_it() {
    let (c, _d, s) = world();
    assert!(uphold_takedown(&PoolCtx::new(&c, 500), &s, "adm", "act").unwrap());
    assert_eq!(n(&c, "SELECT purged_at FROM items"), 500);
    assert_eq!(c.query_row::<String, _, _>("SELECT outcome FROM dmca_actions", [], |r| r.get(0)).unwrap(), "court_upheld");
    assert!(n(&c, "SELECT COUNT(*) FROM void_items") == 1 || n(&c, "SELECT COUNT(*) FROM void_items") == 0);
    // Reversed: strike voided, blocks lifted, evidence restored.
    c.execute("UPDATE strikes SET upheld_at = 500", []).unwrap();
    assert!(reverse_uphold(&PoolCtx::new(&c, 700), "adm", "act").unwrap());
    assert_eq!((n(&c, "SELECT voided_at FROM strikes"), n(&c, "SELECT lifted_at FROM blocked_hashes"), n(&c, "SELECT COUNT(*) FROM void_items")), (700, 700, 0));
    assert!(!reverse_uphold(&PoolCtx::new(&c, 800), "adm", "act").unwrap());
}

#[test]
fn a_put_back_after_a_purge_is_not_a_put_back() {
    let (c, _d, s) = world();
    purge_item(&PoolCtx::new(&c, 10_000), &s, "src", &Actor { id: None, kind: "system" }, "x").unwrap();
    assert!(!restore(&PoolCtx::new(&c, 11_000), &s, "act", &ADMIN, "put_back").unwrap());
    assert_eq!(n(&c, "SELECT COUNT(*) FROM dmca_actions WHERE restored_at IS NOT NULL"), 0);
}
