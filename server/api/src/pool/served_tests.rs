//! Tests for `served.rs`: every builder is prepared against the migrated schema, and the served-form rule is run over a
//! hidden-key publish, where the full form carries an image the stripped form does not.
use super::*;
use crate::db;
use rusqlite::{Connection, params};
use std::path::Path;

fn open() -> (Connection, tempfile::TempDir) {
    let dir = tempfile::tempdir().unwrap();
    let mut conn = db::open(&dir.path().join("t.db")).unwrap();
    db::migrate(&mut conn, &Path::new(env!("CARGO_MANIFEST_DIR")).join("../worker/migrations")).unwrap();
    (conn, dir)
}

fn publish(c: &Connection, id: &str, item: &str, hide_key: bool, status: &str) {
    c.execute(
        "INSERT INTO item_publishes (publish_id, item_id, author_id, version, status, schema_version, hide_key, license, attested_at, attest_text_rev,
            acks_json, request_sha256, stage_token, full_key, full_bytes, title, subject, course, grade, kinds_mask, question_count, has_images, created_at)
         VALUES (?1, ?2, 'a', 1, ?3, 2, ?4, 'CC0-1.0', 1, 'r', '{}', 'x', 't', 'k', 10, '', '', '', '', 0, 1, 1, 1)",
        params![id, item, status, hide_key as i64],
    )
    .unwrap();
}
fn blob(c: &Connection, publish_id: &str, form: &str, sha: &str) {
    c.execute("INSERT INTO item_blobs (publish_id, form, sha256) VALUES (?1, ?2, ?3)", params![publish_id, form, sha]).unwrap();
}
fn col(c: &Connection, sql: &str, p: impl rusqlite::Params) -> Vec<String> {
    let mut st = c.prepare(sql).unwrap_or_else(|e| panic!("{e}: {sql}"));
    st.query_map(p, |r| r.get(0)).unwrap().map(|x| x.unwrap()).collect()
}

#[test]
fn a_hidden_key_publish_serves_only_its_stripped_images_and_a_plain_one_its_full_set() {
    let (c, _d) = open();
    publish(&c, "hid", "i1", true, "current");
    blob(&c, "hid", "full", "aaa");
    blob(&c, "hid", "full", "bbb");
    blob(&c, "hid", "stripped", "bbb");
    publish(&c, "plain", "i2", false, "current");
    blob(&c, "plain", "full", "ccc");
    blob(&c, "plain", "stripped", "ddd");

    assert_eq!(col(&c, &format!("SELECT sha256 FROM {} ORDER BY 1", served_blobs_of("'hid'")), []), vec!["bbb"], "a hidden publish serves only what its author ticked");
    assert_eq!(col(&c, &format!("SELECT sha256 FROM {} ORDER BY 1", served_blobs_of("'plain'")), []), vec!["ccc"]);

    let serves = |item: &str, sha: &str| -> bool {
        let sql = format!("SELECT {}", item_serves_blob("?1", |s| format!("{s} = ?2")));
        c.query_row(&sql, params![item, sha], |r| r.get::<_, i64>(0)).unwrap() == 1
    };
    assert!(serves("i1", "bbb") && !serves("i1", "aaa"), "an image only the full form carries is invisible to the check");
    assert!(serves("i2", "ccc") && !serves("i2", "ddd"));
}

#[test]
fn an_image_only_a_hidden_publishs_full_form_carries_counts_as_used_unserved() {
    let (c, _d) = open();
    c.execute("INSERT INTO users (id, email, trust, created_at) VALUES ('a', 'a@x.org', 'teacher', 1)", []).unwrap();
    c.execute("INSERT INTO items (id, author_id, kind, visibility, license, created_at, updated_at) VALUES ('i1', 'a', 'bank', 'public', 'CC0-1.0', 1, 1)", []).unwrap();
    publish(&c, "hid", "i1", true, "current");
    blob(&c, "hid", "full", "aaa");
    blob(&c, "hid", "full", "bbb");
    blob(&c, "hid", "stripped", "bbb");
    let used = |sha: &str| c.query_row(&format!("SELECT {}", used_unserved("?1")), [sha], |r| r.get::<_, i64>(0)).unwrap() == 1;
    assert!(used("aaa"), "private material the byte purge must keep");
    assert!(!used("bbb"), "served too, so not private");
    assert!(!used("zzz"));
    c.execute("UPDATE items SET purged_at = 5 WHERE id = 'i1'", []).unwrap();
    assert!(!used("aaa"), "a purged item no longer protects its bytes");
}

#[test]
fn the_current_publish_check_reads_the_current_publish_of_the_row() {
    let (c, _d) = open();
    c.execute("INSERT INTO users (id, email, trust, created_at) VALUES ('a', 'a@x.org', 'teacher', 1)", []).unwrap();
    c.execute("INSERT INTO items (id, author_id, kind, visibility, current_publish_id, license, created_at, updated_at) VALUES ('i1', 'a', 'bank', 'public', 'p2', 'CC0-1.0', 1, 1)", []).unwrap();
    publish(&c, "p1", "i1", false, "previous");
    blob(&c, "p1", "full", "old");
    publish(&c, "p2", "i1", false, "current");
    blob(&c, "p2", "full", "new");
    let sql = |sha: &str| format!("SELECT i.id FROM items i WHERE {}", current_serves_blob("i", |s| format!("{s} = '{sha}'")));
    assert_eq!(col(&c, &sql("new"), []), vec!["i1"]);
    assert!(col(&c, &sql("old"), []).is_empty(), "only the current publish counts");
}

#[test]
fn every_takedown_builder_compiles_against_the_schema() {
    let (c, _d) = open();
    for sql in [
        format!("SELECT 1 FROM blocked_hashes bh WHERE {}", rightful_has("'i'", "bh")),
        format!("SELECT 1 FROM blocked_hashes bh WHERE {}", rightful_exempts("'r'", "bh", "'u'", "'p'")),
        format!("SELECT {}", upstream_of("'act'", "'item'")),
        format!("SELECT {}", upstream_carried("'act'", "'item'", "'h'")),
        upstream_snapshot_sql(),
    ] {
        c.prepare(&sql).unwrap_or_else(|e| panic!("{e}: {sql}"));
    }
}

#[test]
fn the_snapshot_records_what_each_upstream_carried_and_nothing_else() {
    let (c, _d) = open();
    c.execute("INSERT INTO users (id, email, trust, created_at) VALUES ('a', 'a@x.org', 'teacher', 1)", []).unwrap();
    publish(&c, "up1", "upstream", false, "current");
    blob(&c, "up1", "full", "imgA");
    publish(&c, "up2", "other", false, "current");
    blob(&c, "up2", "full", "imgB");
    let keys = r#"[{"k":"blob","h":"imgA"}]"#;
    let wrote = c.execute(&upstream_snapshot_sql(), params!["act", r#"["upstream","other"]"#, keys]).unwrap();
    assert_eq!(wrote, 0, "no action row, no snapshot");
    c.execute("INSERT INTO dmca_actions (id, notice_id, item_id, author_id, disabled_at) VALUES ('act', 'n', 'taken', 'a', 1)", []).unwrap();
    c.execute(&upstream_snapshot_sql(), params!["act", r#"["upstream","other"]"#, keys]).unwrap();
    assert_eq!(col(&c, "SELECT item_id FROM dmca_upstream_keys WHERE action_id = 'act'", []), vec!["upstream"]);
}
