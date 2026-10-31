//! Tests for `publishdel.rs`: every statement runs against the real schema; a served publish leaves a ghost, an unserved one leaves none.
use super::*;
use crate::db;
use std::path::Path;

fn world() -> (Connection, tempfile::TempDir) {
    let d = tempfile::tempdir().unwrap();
    let mut c = db::open(&d.path().join("t.db")).unwrap();
    db::migrate(&mut c, &Path::new(env!("CARGO_MANIFEST_DIR")).join("../worker/migrations")).unwrap();
    c.execute("INSERT INTO items (id, author_id, kind, visibility, current_publish_id, license, created_at, updated_at) VALUES ('i1', 'a', 'bank', 'public', 'p2', 'cc-by', 1, 1)", []).unwrap();
    for (p, st) in [("p1", "previous"), ("p2", "current")] {
        c.execute(
            "INSERT INTO item_publishes (publish_id, item_id, author_id, version, status, schema_version, hide_key, license, attested_at, attest_text_rev, acks_json, request_sha256, stage_token,
               full_key, full_bytes, title, subject, course, grade, kinds_mask, question_count, has_images, created_at, committed_at)
             VALUES (?1, 'i1', 'a', 1, ?2, 1, 0, 'cc-by', 1, 'r', '{}', 'x', 't', ?3, 1, 'T', '', '', '', 0, 0, 0, 1, 1)",
            rusqlite::params![p, st, format!("items/{p}")],
        )
        .unwrap();
    }
    (c, d)
}
fn n(c: &Connection, sql: &str) -> i64 {
    c.query_row(sql, [], |r| r.get(0)).unwrap()
}

#[test]
fn deleting_an_unserved_publish_leaves_no_ghost_but_reserves_its_id() {
    let (c, _d) = world();
    delete_publishes(&c, &["p1".to_string()]).unwrap();
    assert_eq!(n(&c, "SELECT COUNT(*) FROM item_publishes"), 1);
    assert_eq!(n(&c, "SELECT COUNT(*) FROM retired_publish_ids WHERE publish_id = 'p1'"), 1);
    assert_eq!(n(&c, "SELECT COUNT(*) FROM ghost_publishes"), 0);
}

#[test]
fn a_served_publish_leaves_a_ghost() {
    let (c, _d) = world();
    c.execute("INSERT INTO served (reader_id, item_id, publish_id, kind, license, licenses_json, first_at, at) VALUES ('r', 'i1', 'p2', 'import', 'cc-by', '[]', 1, 1)", []).unwrap();
    delete_publishes(&c, &["p1".to_string(), "p2".to_string()]).unwrap();
    assert_eq!(n(&c, "SELECT COUNT(*) FROM item_publishes"), 0);
    assert_eq!(n(&c, "SELECT COUNT(*) FROM ghost_publishes WHERE publish_id = 'p2'"), 1);
    assert_eq!(n(&c, "SELECT COUNT(*) FROM ghost_publishes WHERE publish_id = 'p1'"), 0);
    // An empty list is a no-op.
    delete_publishes(&c, &[]).unwrap();
}
