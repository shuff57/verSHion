//! Tests for `readmisc.rs` on the real migrated schema: the standing of an id for a caller, and who may read a blob.
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
fn user(c: &Connection, id: &str) {
    c.execute("INSERT INTO users (id, email, display_name, trust, created_at) VALUES (?1, ?2, ?1, 'teacher', 1)", params![id, format!("{id}@x.org")]).unwrap();
}
fn item(c: &Connection, id: &str, author: &str, publish: &str, version: i64) {
    c.execute(
        "INSERT INTO items (id, author_id, kind, visibility, current_publish_id, public_publish_id, public_version, version, license, license_version, created_at, updated_at, fold_rev)
         VALUES (?1, ?2, 'bank', 'public', ?3, ?3, ?4, ?4, 'CC-BY-4.0', 1, 1, 1, 1)",
        params![id, author, publish, version],
    )
    .unwrap();
    c.execute(
        "INSERT INTO item_publishes (publish_id, item_id, author_id, version, public_version, status, schema_version, hide_key, license, attested_at, attest_text_rev,
            acks_json, request_sha256, stage_token, full_key, stripped_key, full_bytes, title, subject, course, grade, kinds_mask, question_count, has_images, created_at)
         VALUES (?1, ?2, ?3, ?4, ?4, 'current', 2, 0, 'CC-BY-4.0', 1, 'r', '{}', 'x', ?1, 'k', 's', 10, '', '', '', '', 0, 1, 1, 1)",
        params![publish, id, author, version],
    )
    .unwrap();
}
fn imported(c: &Connection, reader: &str, id: &str, version: i64, kind: &str) {
    c.execute(
        "INSERT INTO imports (user_id, item_id, publish_id, version, form, license, author_id, at, kind, real_publish_id, licenses_json, first_at)
         VALUES (?1, ?2, ?3, ?4, 'full', 'CC-BY-4.0', 'a', 5, ?5, ?3, '[\"CC-BY-4.0\"]', 5)",
        params![reader, id, format!("p{version}{kind}"), version, kind],
    )
    .unwrap();
}
fn status(v: &Value, id: &str) -> Value {
    v["items"].as_array().unwrap().iter().find(|x| x["id"] == id).unwrap().clone()
}
fn setup() -> (Connection, tempfile::TempDir) {
    let (c, d) = open();
    for u in ["a", "r", "z"] {
        user(&c, u);
    }
    (c, d)
}

#[test]
fn the_ids_must_be_a_short_list_of_short_strings() {
    assert!(check_ids(None).is_err() && check_ids(Some(&json!([]))).is_err() && check_ids(Some(&json!("x"))).is_err() && check_ids(Some(&json!([1]))).is_err());
    assert!(check_ids(Some(&json!(["x".repeat(65)]))).is_err());
    assert!(check_ids(Some(&json!(vec!["x"; 201]))).is_err());
    assert_eq!(check_ids(Some(&json!(["a", "b"]))).unwrap(), vec!["a".to_string(), "b".to_string()]);
    assert_eq!(check_ids(Some(&json!(vec!["x"; 200]))).unwrap().len(), 200);
}

#[test]
fn an_id_the_caller_never_touched_is_unavailable_whatever_it_is() {
    let (c, _d) = setup();
    let ia = "1".repeat(32);
    item(&c, &ia, "a", "pa", 3);
    let x = PoolCtx::new(&c, 1_000);
    let ghost = "9".repeat(32);
    let out = versions(&x, "r", &[ia.clone(), ghost.clone(), "not-an-id".into()]).unwrap();
    for id in [&ia, &ghost, &"not-an-id".to_string()] {
        assert_eq!(status(&out, id), json!({"id": id, "status": "unavailable"}));
    }
}

#[test]
fn an_import_reports_newer_when_the_public_version_moved_and_current_when_it_did_not() {
    let (c, _d) = setup();
    let ia = "1".repeat(32);
    item(&c, &ia, "a", "pa", 3);
    imported(&c, "r", &ia, 2, "import");
    let x = PoolCtx::new(&c, 1_000);
    let out = versions(&x, "r", &[ia.clone()]).unwrap();
    assert_eq!(status(&out, &ia), json!({"id": ia, "status": "newer", "version": 3, "license": "CC-BY-4.0", "license_version": 1}));
    imported(&c, "r", &ia, 3, "import");
    assert_eq!(status(&versions(&x, "r", &[ia.clone()]).unwrap(), &ia)["status"], "current");
    // The author is never told "newer" of their own item.
    assert_eq!(status(&versions(&x, "a", &[ia.clone()]).unwrap(), &ia)["status"], "current");
}

#[test]
fn a_view_alone_is_seen_but_never_newer() {
    let (c, _d) = setup();
    let ia = "1".repeat(32);
    item(&c, &ia, "a", "pa", 3);
    imported(&c, "r", &ia, 1, "view");
    let x = PoolCtx::new(&c, 1_000);
    assert_eq!(status(&versions(&x, "r", &[ia.clone()]).unwrap(), &ia)["status"], "current");
}

#[test]
fn held_and_removed_items_say_only_what_a_reversible_state_allows() {
    let (c, _d) = setup();
    let (held, gone, kept, tomb) = ("1".repeat(32), "2".repeat(32), "3".repeat(32), "4".repeat(32));
    for (i, p) in [(&held, "p1"), (&gone, "p2"), (&kept, "p3"), (&tomb, "p4")] {
        item(&c, i, "a", p, 2);
        imported(&c, "r", i, 2, "import");
    }
    c.execute("INSERT INTO item_holds (item_id, reason, at) VALUES (?1, 'reports', 1)", [&held]).unwrap();
    c.execute("UPDATE items SET removed_at = 9, removed_reason = 'takedown', purge_after = 777 WHERE id = ?1", [&gone]).unwrap();
    c.execute("UPDATE items SET removed_at = 9, removed_reason = 'author_deleted', purged_at = NULL WHERE id = ?1", [&kept]).unwrap_or_else(|_| 0);
    c.execute("UPDATE items SET removed_at = 9, removed_reason = 'admin', purged_at = 10 WHERE id = ?1", [&tomb]).unwrap();
    let x = PoolCtx::new(&c, 1_000);
    let out = versions(&x, "r", &[held.clone(), gone.clone(), kept.clone(), tomb.clone()]).unwrap();
    assert_eq!(status(&out, &held), json!({"id": held, "status": "under_review", "reason": "held"}));
    assert_eq!(status(&out, &gone), json!({"id": gone, "status": "under_review", "reason": "takedown", "until": 777}));
    assert_eq!(status(&out, &tomb), json!({"id": tomb, "status": "removed", "reason": "admin"}));
    assert_eq!(status(&out, &kept)["reason"], "deleted", "an author deletion reads as deleted");
}

#[test]
fn a_privacy_removal_is_under_review_until_it_is_purged() {
    let (c, _d) = setup();
    let ia = "1".repeat(32);
    item(&c, &ia, "a", "pa", 2);
    imported(&c, "r", &ia, 2, "import");
    c.execute("UPDATE items SET removed_at = 9, removed_reason = 'privacy' WHERE id = ?1", [&ia]).unwrap();
    let x = PoolCtx::new(&c, 1_000);
    assert_eq!(status(&versions(&x, "r", &[ia.clone()]).unwrap(), &ia), json!({"id": ia, "status": "under_review", "reason": "privacy"}));
    c.execute("UPDATE items SET purged_at = 20 WHERE id = ?1", [&ia]).unwrap();
    assert_eq!(status(&versions(&x, "r", &[ia.clone()]).unwrap(), &ia), json!({"id": ia, "status": "removed", "reason": "privacy"}));
}

#[test]
fn a_duplicate_id_is_answered_each_time_it_is_asked_and_the_allowance_is_charged() {
    let (c, _d) = setup();
    let ia = "1".repeat(32);
    item(&c, &ia, "a", "pa", 2);
    imported(&c, "r", &ia, 2, "import");
    let x = PoolCtx::new(&c, 1_000);
    let out = versions(&x, "r", &[ia.clone(), ia.clone()]).unwrap();
    assert_eq!(out["items"].as_array().unwrap().len(), 2);
    let n: i64 = c.query_row("SELECT count FROM usage_daily WHERE user_id = 'r' AND metric = 'item_reads'", [], |r| r.get(0)).unwrap();
    assert_eq!(n, 1);
}

// ---- blobs ----

const SHA: &str = "ab00000000000000000000000000000000000000000000000000000000000001";
fn blob_world(hide: bool) -> (Connection, tempfile::TempDir, BlobStore, String) {
    let (c, d) = setup();
    let id = "1".repeat(32);
    item(&c, &id, "a", "pa", 1);
    c.execute("UPDATE item_publishes SET hide_key = ?1", [hide as i64]).unwrap();
    c.execute("INSERT INTO blobs (sha256, size, content_type, width, height, first_uploaded_at) VALUES (?1, 3, 'image/png', 1, 1, 1)", [SHA]).unwrap();
    c.execute("INSERT INTO item_blobs (publish_id, form, sha256) VALUES ('pa', 'full', ?1)", [SHA]).unwrap();
    let store = BlobStore::new(d.path().join("blobs")).unwrap();
    store.put(&format!("blobs/{SHA}"), b"png").unwrap();
    (c, d, store, id)
}
fn blob_err(r: Res<BlobOut>) -> u16 {
    match r {
        Err(Fail::Pool(e)) => e.status,
        other => panic!("{other:?}"),
    }
}

#[test]
fn a_listed_image_is_served_with_the_verified_type_and_locked_down_headers() {
    let (c, _d, store, id) = blob_world(false);
    let x = PoolCtx::new(&c, 1_000);
    let b = get_blob(&x, &store, "r", &id, SHA).unwrap();
    assert_eq!(b.bytes, b"png");
    let h: HashMap<_, _> = blob_headers(&b).into_iter().collect();
    assert_eq!(h["Content-Type"], "image/png");
    assert_eq!(h["Content-Disposition"], format!("attachment; filename=\"{SHA}.png\""));
    assert_eq!((h["X-Content-Type-Options"].as_str(), h["Cache-Control"].as_str()), ("nosniff", "no-store"));
    assert_eq!(h["Content-Security-Policy"], "sandbox; default-src 'none'");
    assert_eq!(ext_of("image/jpeg"), "jpg");
    assert_eq!(ext_of("x/y"), "bin");
}

#[test]
fn a_hidden_key_item_serves_others_only_the_stripped_listing() {
    let (c, _d, store, id) = blob_world(true);
    let x = PoolCtx::new(&c, 1_000);
    assert_eq!(blob_err(get_blob(&x, &store, "r", &id, SHA)), 404, "only listed under the full form");
    assert!(get_blob(&x, &store, "a", &id, SHA).is_ok(), "its author is served the full form");
    c.execute("INSERT INTO item_blobs (publish_id, form, sha256) VALUES ('pa', 'stripped', ?1)", [SHA]).unwrap();
    assert!(get_blob(&x, &store, "r", &id, SHA).is_ok());
}

#[test]
fn a_privacy_blocked_or_purged_image_is_served_to_nobody_but_the_authors_unserved_use() {
    let (c, _d, store, id) = blob_world(false);
    let x = PoolCtx::new(&c, 1_000);
    c.execute("UPDATE blobs SET privacy_purged_at = 5", []).unwrap();
    assert_eq!(blob_err(get_blob(&x, &store, "r", &id, SHA)), 404);
    assert_eq!(blob_err(get_blob(&x, &store, "a", &id, SHA)), 404, "served uses stay gone for the author too");
    c.execute("UPDATE blobs SET privacy_purged_at = NULL", []).unwrap();
    c.execute("INSERT INTO privacy_blocks (kind, hash, item_id, at) VALUES ('blob', ?1, ?2, 1)", params![SHA, id]).unwrap();
    assert_eq!(blob_err(get_blob(&x, &store, "a", &id, SHA)), 404);
}

#[test]
fn every_way_of_missing_is_the_same_counted_404() {
    let (c, _d, store, id) = blob_world(false);
    let x = PoolCtx::new(&c, 1_000);
    assert_eq!(blob_err(get_blob(&x, &store, "r", "bad", SHA)), 404);
    assert_eq!(blob_err(get_blob(&x, &store, "r", &id, "bad")), 404);
    assert_eq!(blob_err(get_blob(&x, &store, "r", &"2".repeat(32), SHA)), 404);
    store.delete(&format!("blobs/{SHA}")).unwrap();
    assert_eq!(blob_err(get_blob(&x, &store, "r", &id, SHA)), 404);
    let n: i64 = c.query_row("SELECT count FROM usage_daily WHERE user_id = 'r' AND metric = 'blob_miss'", [], |r| r.get(0)).unwrap();
    assert_eq!(n, 4);
    c.execute("INSERT INTO item_holds (item_id, reason, at) VALUES (?1, 'admin', 1)", [&id]).unwrap();
    store.put(&format!("blobs/{SHA}"), b"png").unwrap();
    assert_eq!(blob_err(get_blob(&x, &store, "r", &id, SHA)), 404, "a held item serves nothing");
}
