//! Tests for `circlepurge.rs` on the real migrated schema with a real on-disk `BlobStore`: purging banks and circles, pruning, abandoned
//! sends, image GC, purge-now and the export.
use super::*;
use crate::db;
use crate::pool::circles::create_circle;
use crate::pool::ctx::Fail;
use crate::pool::sqlfrag::DAY_MS;
use serde_json::Map;
use std::path::Path;

const NOW: i64 = 100 * DAY_MS;
const SHA: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";

fn open() -> (rusqlite::Connection, tempfile::TempDir, BlobStore) {
    let dir = tempfile::tempdir().unwrap();
    let mut conn = db::open(&dir.path().join("t.db")).unwrap();
    db::migrate(&mut conn, &Path::new(env!("CARGO_MANIFEST_DIR")).join("../worker/migrations")).unwrap();
    let store = BlobStore::new(dir.path().join("blobs")).unwrap();
    (conn, dir, store)
}
fn user(c: &rusqlite::Connection, u: &str) {
    c.execute("INSERT INTO users (id, email, display_name, trust, created_at) VALUES (?1, ?2, ?3, 'teacher', 1)", params![u, format!("{u}@x.org"), format!("Teacher {u}")]).unwrap();
}
fn err(f: Fail) -> (u16, String) {
    match f {
        Fail::Pool(e) => (e.status, e.code.clone()),
        other => panic!("{other:?}"),
    }
}
fn world() -> (rusqlite::Connection, tempfile::TempDir, BlobStore, String, String) {
    let (c, d, s) = open();
    for u in ["a", "b"] {
        user(&c, u);
    }
    let body: Map<String, Value> = json!({ "name": "Team" }).as_object().unwrap().clone();
    let cid = create_circle(&PoolCtx::new(&c, 1_000), "a", &body).unwrap().1["circle"]["id"].as_str().unwrap().to_string();
    c.execute("INSERT INTO circle_members (circle_id, user_id, role, joined_at, join_seq) VALUES (?1, 'b', 'editor', 10, 5)", [&cid]).unwrap();
    let bid = format!("{:032x}", 1);
    c.execute("INSERT INTO circle_banks (id, circle_id, author_id, title, subject, license, head_version, head_version_id, created_at, updated_at) VALUES (?1, ?2, 'b', 'Quiz', 'Math', 'circle-only', 1, 'v1', 1, 77)", params![bid, cid]).unwrap();
    c.execute("INSERT INTO circle_versions (id, bank_id, circle_id, version, commit_id, request_sha256, committer_id, license, status, full_key, index_key, bytes, question_count, created_at, committed_at) VALUES ('v1', ?1, ?2, 1, 'c1', 'x', 'b', 'circle-only', 'current', 'circles/k1', 'circles/i1', 1, 1, 600, 600)", params![bid, cid]).unwrap();
    s.put("circles/k1", b"full").unwrap();
    s.put("circles/i1", b"index").unwrap();
    c.execute("INSERT INTO blobs (sha256, size, content_type, width, height, first_uploaded_at) VALUES (?1, 3, 'image/png', 1, 1, 1)", [SHA]).unwrap();
    c.execute("INSERT INTO circle_version_blobs (version_id, sha256) VALUES ('v1', ?1)", [SHA]).unwrap();
    s.put(&format!("blobs/{SHA}"), b"png").unwrap();
    (c, d, s, cid, bid)
}
fn exists(s: &BlobStore, k: &str) -> bool {
    s.get(k).unwrap().is_some()
}

#[test]
fn a_purge_leaves_a_tombstone_and_takes_the_bytes_and_unused_images() {
    let (c, _d, s, cid, bid) = world();
    let x = PoolCtx::new(&c, NOW);
    purge_bank(&x, &s, &bid, &cid, "purge_after", None).unwrap();
    let t: (Option<i64>, String, String, Option<String>, Option<String>) = c.query_row("SELECT purged_at, title, subject, head_version_id, removed_reason FROM circle_banks", [], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?))).unwrap();
    assert_eq!(t, (Some(NOW), "".into(), "".into(), None, Some("circle_closed".into())));
    let n: i64 = c.query_row("SELECT (SELECT COUNT(*) FROM circle_versions) + (SELECT COUNT(*) FROM circle_version_blobs) + (SELECT COUNT(*) FROM blobs)", [], |r| r.get(0)).unwrap();
    assert_eq!(n, 0);
    assert!(!exists(&s, "circles/k1") && !exists(&s, "circles/i1") && !exists(&s, &format!("blobs/{SHA}")));
    let orphans: i64 = c.query_row("SELECT COUNT(*) FROM r2_orphans", [], |r| r.get(0)).unwrap();
    assert_eq!(orphans, 0, "deleted keys leave no orphan rows");
    let a: (String, String) = c.query_row("SELECT actor_kind, detail FROM pool_audit WHERE action = 'circle.bank_purge'", [], |r| Ok((r.get(0)?, r.get(1)?))).unwrap();
    assert_eq!(a.0, "system");
    assert!(a.1.contains("purge_after"));
}

#[test]
fn an_image_another_kept_version_uses_survives_the_purge() {
    let (c, _d, s, cid, bid) = world();
    let x = PoolCtx::new(&c, NOW);
    let other = format!("{:032x}", 2);
    c.execute("INSERT INTO circle_banks (id, circle_id, author_id, title, license, head_version, head_version_id, created_at, updated_at) VALUES (?1, ?2, 'b', 'Other', 'circle-only', 1, 'v2', 1, 77)", params![other, cid]).unwrap();
    c.execute("INSERT INTO circle_versions (id, bank_id, circle_id, version, commit_id, request_sha256, committer_id, license, status, full_key, index_key, bytes, question_count, created_at) VALUES ('v2', ?1, ?2, 1, 'c2', 'x', 'b', 'circle-only', 'current', 'circles/k2', '', 1, 1, 600)", params![other, cid]).unwrap();
    c.execute("INSERT INTO circle_version_blobs (version_id, sha256) VALUES ('v2', ?1)", [SHA]).unwrap();
    purge_bank(&x, &s, &bid, &cid, "owner_purge", Some("a")).unwrap();
    assert!(exists(&s, &format!("blobs/{SHA}")));
    let who: (String, String) = c.query_row("SELECT actor_kind, actor_id FROM pool_audit WHERE action = 'circle.bank_purge'", [], |r| Ok((r.get(0)?, r.get(1)?))).unwrap();
    assert_eq!(who, ("user".into(), "a".into()));
}

#[test]
fn the_daily_purge_takes_what_is_due_and_spares_reported_or_held_banks_and_reopened_circles() {
    let (c, _d, s, cid, bid) = world();
    let x = PoolCtx::new(&c, NOW);
    c.execute("UPDATE circle_banks SET removed_at = 1, removed_reason = 'author_removed', purge_after = ?1", [NOW - 1]).unwrap();
    c.execute("INSERT INTO circle_reports (id, circle_id, bank_id, reporter_id, reason, at) VALUES ('r1', ?1, ?2, 'a', 'other', 5)", params![cid, bid]).unwrap();
    assert_eq!(purge_circles(&x, &s).unwrap(), (0, 0), "an open report keeps it");
    c.execute("UPDATE circle_reports SET resolved_at = 9", []).unwrap();
    c.execute("INSERT INTO circle_holds (bank_id, reason, source, at) VALUES (?1, 'student_data', 'privacy', 1)", [&bid]).unwrap();
    assert_eq!(purge_circles(&x, &s).unwrap().0, 1, "a hold a send raised does not keep it");
    // A closed circle past its date goes, with its invites, members and banks; one reopened meanwhile stays.
    c.execute("UPDATE circles SET closed_at = 5, close_reason = 'owner_closed', undo_until = 6, purge_after = ?1", [NOW - 1]).unwrap();
    c.execute("INSERT INTO circle_transfers (circle_id, from_user, to_user, created_at, expires_at) VALUES (?1, 'a', 'b', 1, ?2)", params![cid, NOW + 99]).unwrap();
    assert_eq!(purge_circles(&x, &s).unwrap(), (0, 1));
    let row: (Option<i64>, Option<String>) = c.query_row("SELECT purged_at, name FROM circles", [], |r| Ok((r.get(0)?, r.get(1)?))).unwrap();
    assert_eq!(row, (Some(NOW), None));
    let left: i64 = c.query_row("SELECT (SELECT COUNT(*) FROM circle_members) + (SELECT COUNT(*) FROM circle_invites) + (SELECT COUNT(*) FROM circle_transfers)", [], |r| r.get(0)).unwrap();
    assert_eq!(left, 0);
    let n: i64 = c.query_row("SELECT COUNT(*) FROM pool_audit WHERE action = 'circle.purge' AND item_id = ?1", [&cid], |r| r.get(0)).unwrap();
    assert_eq!(n, 1);
}

#[test]
fn expired_ownership_offers_go_in_the_daily_purge() {
    let (c, _d, s, cid, _bid) = world();
    let x = PoolCtx::new(&c, NOW);
    c.execute("INSERT INTO circle_transfers (circle_id, from_user, to_user, created_at, expires_at) VALUES (?1, 'a', 'b', 1, ?2)", params![cid, NOW]).unwrap();
    purge_circles(&x, &s).unwrap();
    let n: i64 = c.query_row("SELECT COUNT(*) FROM circle_transfers", [], |r| r.get(0)).unwrap();
    assert_eq!(n, 0);
}

#[test]
fn pruned_versions_lose_their_objects_and_staging_rows_older_than_an_hour_are_discarded() {
    let (c, _d, s, cid, bid) = world();
    let x = PoolCtx::new(&c, NOW);
    c.execute("UPDATE circle_versions SET status = 'pruned', pruned_at = 5", []).unwrap();
    assert_eq!(prune_circle_versions(&x, &s).unwrap(), 1);
    let keys: (String, String) = c.query_row("SELECT full_key, index_key FROM circle_versions", [], |r| Ok((r.get(0)?, r.get(1)?))).unwrap();
    assert_eq!(keys, ("".into(), "".into()));
    assert!(!exists(&s, "circles/k1"));
    assert_eq!(prune_circle_versions(&x, &s).unwrap(), 0, "a rerun finds nothing");
    for (id, at) in [("s1", NOW - 2 * HOUR_MS), ("s2", NOW - 10)] {
        c.execute("INSERT INTO circle_versions (id, bank_id, circle_id, commit_id, request_sha256, committer_id, license, status, full_key, index_key, bytes, question_count, created_at) VALUES (?1, ?2, ?3, ?1, 'x', 'b', 'circle-only', 'staging', ?4, '', 1, 1, ?5)", params![id, bid, cid, format!("circles/{id}"), at]).unwrap();
        s.put(&format!("circles/{id}"), b"s").unwrap();
    }
    assert_eq!(discard_circle_staging(&x, &s).unwrap(), 1);
    assert!(!exists(&s, "circles/s1") && exists(&s, "circles/s2"));
}

#[test]
fn a_failed_delete_leaves_the_key_in_the_orphan_list() {
    let (c, _d, s, _cid, _bid) = world();
    let x = PoolCtx::new(&c, NOW);
    // A key the store refuses to touch.
    c.execute("INSERT INTO r2_orphans (key, at) VALUES ('../x', 1)", []).unwrap();
    assert!(!delete_keys(&x, &s, &["../x".to_string()]).unwrap());
    let n: i64 = c.query_row("SELECT COUNT(*) FROM r2_orphans", [], |r| r.get(0)).unwrap();
    assert_eq!(n, 1);
    assert!(delete_keys(&x, &s, &["circles/gone".to_string()]).unwrap());
}

#[test]
fn only_the_owner_purges_a_removed_bank_now_and_never_one_under_moderation() {
    let (c, _d, s, cid, bid) = world();
    let x = PoolCtx::new(&c, NOW);
    assert_eq!(err(purge_bank_now(&x, &s, "b", &cid, &bid).unwrap_err()).1, "circle_role_required");
    assert_eq!(err(purge_bank_now(&x, &s, "a", &cid, &bid).unwrap_err()).1, "bank_not_removed");
    c.execute("UPDATE circle_banks SET removed_at = 1, removed_reason = 'admin'", []).unwrap();
    assert_eq!(err(purge_bank_now(&x, &s, "a", &cid, &bid).unwrap_err()).1, "under_moderation");
    c.execute("UPDATE circle_banks SET removed_reason = 'author_removed', purge_after = ?1", [NOW + DAY_MS]).unwrap();
    c.execute("INSERT INTO circle_reports (id, circle_id, bank_id, reporter_id, reason, at) VALUES ('r1', ?1, ?2, 'a', 'other', 5)", params![cid, bid]).unwrap();
    assert_eq!(err(purge_bank_now(&x, &s, "a", &cid, &bid).unwrap_err()), (409, "under_moderation".into()));
    c.execute("UPDATE circle_reports SET resolved_at = 9", []).unwrap();
    assert_eq!(purge_bank_now(&x, &s, "a", &cid, &bid).unwrap(), json!({ "ok": true }));
    let p: Option<i64> = c.query_row("SELECT purged_at FROM circle_banks", [], |r| r.get(0)).unwrap();
    assert_eq!(p, Some(NOW));
    assert!(!exists(&s, "circles/k1"));
}

#[test]
fn the_export_lists_the_accounts_circles_without_codes_or_bodies() {
    let (c, _d, _s, cid, bid) = world();
    let x = PoolCtx::new(&c, NOW);
    c.execute("INSERT INTO circle_invites (id, circle_id, code_hash, role, created_by, created_at, expires_at, note) VALUES ('i1', ?1, 'secrethash', 'viewer', 'a', 2, 99999, 'for Sam')", [&cid]).unwrap();
    c.execute("INSERT INTO circle_transfers (circle_id, from_user, to_user, created_at, expires_at) VALUES (?1, 'a', 'b', 1, ?2)", params![cid, NOW + 5]).unwrap();
    let a = export_circles(&x, "a").unwrap();
    assert_eq!(a["memberships"][0]["role"], json!("owner"));
    assert_eq!(a["owned"][0]["circle_name"], json!("Team"));
    assert_eq!(a["invites_created"][0]["note"], json!("for Sam"));
    assert!(!a.to_string().contains("secrethash"));
    assert_eq!(a["transfer_offers"][0]["offered_by_me"], json!(1));
    let b = export_circles(&x, "b").unwrap();
    assert_eq!((b["banks_authored"][0]["bank_id"].clone(), b["versions_sent"][0]["version"].clone(), b["transfer_offers"][0]["offered_to_me"].clone()), (json!(bid), json!(1), json!(1)));
}
