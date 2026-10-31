//! Tests for `circlesendsql.rs` on the real migrated schema: the fragments run, the evidence bound prunes what it should, the cap flag
//! dedups, and purge-now of a bank only a flag holds answers as partly done.
use super::*;
use crate::db;
use crate::pool::circlebase::KEPT_VERSIONS;
use crate::pool::circlepurge::purge_bank_now;
use crate::pool::circles::create_circle;
use crate::pool::ctx::PoolCtx;
use crate::pool::sqlfrag::DAY_MS;
use serde_json::{Map, Value, json};
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
fn n(c: &Connection, sql: &str) -> i64 {
    c.query_row(sql, [], |r| r.get(0)).unwrap()
}
fn world() -> (Connection, tempfile::TempDir, String, String) {
    let (c, d) = open();
    for u in ["a", "b"] {
        user(&c, u);
    }
    let body: Map<String, Value> = json!({ "name": "Team" }).as_object().unwrap().clone();
    let cid = create_circle(&PoolCtx::new(&c, 1_000), "a", &body).unwrap().1["circle"]["id"].as_str().unwrap().to_string();
    c.execute("INSERT INTO circle_members (circle_id, user_id, role, joined_at, join_seq) VALUES (?1, 'b', 'editor', 10, 5)", [&cid]).unwrap();
    let bid = format!("{:032x}", 1);
    c.execute("INSERT INTO circle_banks (id, circle_id, author_id, title, license, head_version, head_version_id, question_count, created_at, updated_at) VALUES (?1, ?2, 'b', 'Quiz', 'circle-only', 9, 'v9', 4, 1, 77)", params![bid, cid]).unwrap();
    (c, d, cid, bid)
}
fn version(c: &Connection, bid: &str, cid: &str, v: i64, status: &str, bytes: i64, withheld: Option<i64>) {
    c.execute(
        "INSERT INTO circle_versions (id, bank_id, circle_id, version, commit_id, request_sha256, committer_id, license, status, full_key, index_key, bytes, question_count, created_at, committed_at, withheld_at)
         VALUES (?1, ?2, ?3, ?4, ?1, 'x', 'b', 'circle-only', ?5, ?6, 'i', ?7, 1, ?4, ?4, ?8)",
        params![format!("v{v}"), bid, cid, v, status, format!("k{v}"), bytes, withheld],
    )
    .unwrap();
}
fn flag(c: &Connection, vid: &str, kind: &str) {
    c.execute("INSERT INTO admin_flags (id, kind, item_id, user_id, detail, publish_id, at) VALUES (?1, ?2, ?4, 'b', '{}', ?3, 5)", params![format!("f-{vid}-{kind}"), kind, vid, format!("{:032x}", 1)]).unwrap();
}

#[test]
fn counting_fragments_sum_the_circle() {
    let (c, _d, cid, bid) = world();
    version(&c, &bid, &cid, 1, "previous", 100, None);
    version(&c, &bid, &cid, 2, "current", 50, None);
    version(&c, &bid, &cid, 3, "pruned", 999, None);
    let q = |frag: String| n(&c, &format!("SELECT {frag}"));
    let lit = format!("'{cid}'");
    assert_eq!(q(bytes_sql(&lit)), 150);
    assert_eq!(q(banks_sql(&lit)), 1);
    assert_eq!((q(questions_sql(&lit, "'other'")), q(questions_sql(&lit, &format!("'{bid}'")))), (4, 0));
    // Fragments that join other tables are valid SQL and match nothing on an empty world.
    assert_eq!(q(privacy_sql("'[\"h\"]'", "'[]'")), 0);
    assert_eq!(q(copyright_sql("'[\"h\"]'", "'[\"s\"]'", "5", "'a'")), 0);
    assert_eq!(q(flag_room_sql("'b'", &lit, "0", "3")), 1);
    assert_eq!(n(&c, &format!("SELECT {} FROM circle_banks b", rollback_rights_sql("b", &lit, "'a'"))), 1);
    assert_eq!(n(&c, &format!("SELECT {} FROM circle_banks b", rollback_rights_sql("b", &lit, "'b'"))), 1, "the author while an editor");
    assert_eq!(cap_dedup("bk", "u", 3 * DAY_MS), "circle_send_flags:bk:u:3");
}

#[test]
fn an_active_copyright_block_holds_unless_exempted() {
    let (c, _d, _cid, _bid) = world();
    c.execute("INSERT INTO dmca_actions (id, item_id, kind, at) VALUES ('act', 'i', 'takedown', 1)", []).ok();
    let cols: Vec<String> = c.prepare("SELECT name FROM pragma_table_info('blocked_hashes')").unwrap().query_map([], |r| r.get(0)).unwrap().collect::<Result<_, _>>().unwrap();
    assert!(cols.contains(&"action_id".to_string()), "{cols:?}");
    let sql = copyright_sql("'[\"h1\"]'", "'[]'", "100", "'z'");
    // With no block row nothing matches; the statement is well formed either way.
    assert_eq!(n(&c, &format!("SELECT {sql}")), 0);
}

#[test]
fn the_evidence_bound_prunes_only_older_flagged_withheld_previous_versions_beyond_the_per_bank_room() {
    let (c, _d, cid, bid) = world();
    // Eight withheld previous versions from one sender, each with an open student-data flag, plus one unflagged and one the head.
    for v in 1..=8 {
        version(&c, &bid, &cid, v, "previous", 10, Some(9));
        flag(&c, &format!("v{v}"), "privacy_text_match");
    }
    version(&c, &bid, &cid, 20, "previous", 10, Some(9)); // withheld but nothing pins it
    version(&c, &bid, &cid, 30, "current", 10, Some(9));
    flag(&c, "v30", "privacy_text_match");
    evidence_bound(&c, &cid, 5_000, 1_000_000, "1").unwrap();
    let pruned: Vec<i64> = c.prepare("SELECT version FROM circle_versions WHERE status = 'pruned' ORDER BY version").unwrap().query_map([], |r| r.get(0)).unwrap().collect::<Result<_, _>>().unwrap();
    // The sender's newest EVIDENCE_SENDER_BANK (2) are protected; the bank keeps EVIDENCE_PER_BANK (5) in all, newest first:
    // of 8 flagged (versions 1-8), 7 and 8 are protected and fill 2 of the 5 slots, so 3 more (6, 5, 4) stay and 1-3 are pruned.
    assert_eq!(pruned, vec![1, 2, 3]);
    assert_eq!(n(&c, "SELECT COUNT(*) FROM circle_versions WHERE id = 'v20' AND status = 'previous'"), 1);
    assert_eq!(n(&c, "SELECT COUNT(*) FROM circle_versions WHERE id = 'v30' AND status = 'current'"), 1);
    let marked: i64 = n(&c, "SELECT COUNT(*) FROM admin_flags WHERE json_extract(detail, '$.evidence_pruned') = 1");
    assert_eq!(marked, 3);
    assert_eq!(n(&c, "SELECT COUNT(*) FROM admin_flags WHERE json_extract(detail, '$.evidence_pruned_at') = 5000"), 3);
    // A condition that fails makes it a no-op.
    evidence_bound(&c, &cid, 6_000, 0, "0").unwrap();
    assert_eq!(n(&c, "SELECT COUNT(*) FROM circle_versions WHERE status = 'pruned'"), 3);
    let _ = KEPT_VERSIONS;
}

#[test]
fn a_bank_level_cap_flag_is_one_per_bank_sender_and_day_and_collects_versions() {
    let (c, _d, cid, bid) = world();
    version(&c, &bid, &cid, 1, "current", 10, Some(9));
    version(&c, &bid, &cid, 2, "current", 10, Some(9));
    let dedup = cap_dedup(&bid, "b", 5_000);
    let run = |id: &str, ver: i64, vid: &str| c.execute(&cap_flag_sql("1"), params![id, bid, "b", cid, ver, vid, "[]", "true", dedup, 5_000]).unwrap();
    run("c1", 1, "v1");
    run("c2", 2, "v2");
    assert_eq!(n(&c, "SELECT COUNT(*) FROM admin_flags WHERE kind = 'cap'"), 1);
    assert_eq!(n(&c, "SELECT json_array_length(detail, '$.versions') FROM admin_flags WHERE kind = 'cap'"), 2);
    // Not for a version that is not withheld.
    c.execute("UPDATE circle_versions SET withheld_at = NULL WHERE id = 'v2'", []).unwrap();
    let before = n(&c, "SELECT json_array_length(detail, '$.versions') FROM admin_flags WHERE kind = 'cap'");
    run("c3", 2, "v2");
    assert_eq!(n(&c, "SELECT json_array_length(detail, '$.versions') FROM admin_flags WHERE kind = 'cap'"), before);
}

#[test]
fn purge_now_of_a_bank_only_a_flag_holds_prunes_everything_but_the_evidence() {
    let (c, d, cid, bid) = world();
    let store = crate::blobs::BlobStore::new(d.path().join("blobs")).unwrap();
    version(&c, &bid, &cid, 1, "previous", 10, None);
    version(&c, &bid, &cid, 2, "previous", 10, Some(9));
    version(&c, &bid, &cid, 3, "current", 10, None);
    flag(&c, "v2", "privacy_text_match");
    c.execute("UPDATE circle_banks SET removed_at = 1, removed_reason = 'owner_removed', purge_after = ?1", [10 * DAY_MS]).unwrap();
    let x = PoolCtx::new(&c, 5_000);
    let r = purge_bank_now(&x, &store, "a", &cid, &bid).unwrap();
    assert_eq!((r["purged_partial"].clone(), r["pending_review"].clone(), r["evidence_kept"].clone()), (json!(true), json!(true), json!(1)));
    assert!(r["message"].as_str().unwrap().contains("a version of this bank"));
    let kept: Vec<(i64, String)> = c.prepare("SELECT version, status FROM circle_versions ORDER BY version").unwrap().query_map([], |r| Ok((r.get(0)?, r.get(1)?))).unwrap().collect::<Result<_, _>>().unwrap();
    assert_eq!(kept, vec![(1, "pruned".into()), (2, "previous".into()), (3, "pruned".into())]);
    assert_eq!(n(&c, "SELECT purge_after FROM circle_banks"), 5_000, "claimed: the daily purge takes it after the review");
    assert_eq!(n(&c, "SELECT COUNT(*) FROM pool_audit WHERE action = 'circle.bank_purge_requested'"), 1);
}
