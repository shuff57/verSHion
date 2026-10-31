//! Tests for `blobs.rs`: the upload core against the real schema and a real directory.
use super::*;
use crate::db;
use rusqlite::Connection;
use std::path::Path;

struct Env {
    conn: Connection,
    store: BlobStore,
    _dir: tempfile::TempDir,
}
fn env() -> Env {
    let dir = tempfile::tempdir().unwrap();
    let mut conn = db::open(&dir.path().join("t.db")).unwrap();
    db::migrate(&mut conn, &Path::new(env!("CARGO_MANIFEST_DIR")).join("../worker/migrations")).unwrap();
    let store = BlobStore::new(dir.path().join("blobs")).unwrap();
    conn.execute("INSERT INTO users (id, email, display_name, trust, created_at) VALUES ('me', 'me@x.org', 'Me', 'teacher', 1)", []).unwrap();
    Env { conn, store, _dir: dir }
}
fn fixture(name: &str) -> Vec<u8> {
    std::fs::read(Path::new(env!("CARGO_MANIFEST_DIR")).join("../worker/test/pool/fixtures").join(name)).unwrap()
}
fn err(f: super::super::ctx::Fail) -> PoolError {
    match f {
        super::super::ctx::Fail::Pool(p) => p,
        super::super::ctx::Fail::Db(e) => panic!("db: {e}"),
    }
}
fn count(c: &Connection, sql: &str) -> i64 {
    c.query_row(sql, [], |r| r.get(0)).unwrap()
}

#[test]
fn an_upload_is_stored_charged_and_answered_with_the_images_size() {
    let e = env();
    let x = PoolCtx::new(&e.conn, 1_000_000);
    let bytes = fixture("pil.gif");
    let sha = sha256_hex(&bytes);
    let r = store_blob(&x, &e.store, "me", &sha, &bytes).unwrap();
    assert_eq!(r, Stored { sha256: sha.clone(), size: bytes.len(), width: 64, height: 48 });
    assert_eq!(e.store.get(&format!("blobs/{sha}")).unwrap().unwrap(), bytes);
    assert_eq!(count(&e.conn, "SELECT COUNT(*) FROM blobs WHERE content_type = 'image/gif' AND width = 64 AND height = 48"), 1);
    assert_eq!(usage(&x, "me").unwrap(), bytes.len() as i64, "charged as pending");
    assert_eq!(count(&e.conn, "SELECT COUNT(*) FROM blob_uploads"), 1);
    // The same upload again: the same answer, and no second charge.
    let again = store_blob(&x, &e.store, "me", &sha, &bytes).unwrap();
    assert_eq!(again, r);
    assert_eq!(usage(&x, "me").unwrap(), bytes.len() as i64);
    assert_eq!(count(&e.conn, "SELECT COUNT(*) FROM pending_blobs"), 1);
    // Another author uploading the same image pays for their own copy.
    e.conn.execute("INSERT INTO users (id, email, trust, created_at) VALUES ('you', 'you@x.org', 'teacher', 1)", []).unwrap();
    store_blob(&x, &e.store, "you", &sha, &bytes).unwrap();
    assert_eq!(usage(&x, "you").unwrap(), bytes.len() as i64);
    assert_eq!(count(&e.conn, "SELECT COUNT(*) FROM blobs"), 1, "one row for the content-addressed blob");
}

#[test]
fn a_wrong_hash_and_a_refused_image_get_the_documented_errors() {
    let e = env();
    let x = PoolCtx::new(&e.conn, 1_000);
    let bytes = fixture("pil.jpg");
    let wrong = "0".repeat(64);
    let h = err(store_blob(&x, &e.store, "me", &wrong, &bytes).unwrap_err());
    assert_eq!((h.status, h.code.as_str()), (422, "hash_mismatch"));
    let junk = b"not an image at all".to_vec();
    let r = err(store_blob(&x, &e.store, "me", &sha256_hex(&junk), &junk).unwrap_err());
    assert_eq!((r.status, r.body()), (422, json!({ "error": "image_refused", "reason": "not_accepted" })));
    let exif = fixture("pil_exif.jpg");
    assert_eq!(err(store_blob(&x, &e.store, "me", &sha256_hex(&exif), &exif).unwrap_err()).code, "image_refused", "one answer for every refusal");
    assert_eq!(usage(&x, "me").unwrap(), 0, "a refused upload costs nothing");
}

#[test]
fn a_blocked_image_is_accepted_like_any_other_but_only_a_zero_filled_stand_in_is_kept() {
    let e = env();
    let x = PoolCtx::new(&e.conn, 5_000);
    let bytes = fixture("pil.jpg");
    let sha = sha256_hex(&bytes);
    e.conn.execute("INSERT INTO privacy_blocks (kind, hash, item_id, at) VALUES ('blob', ?1, 'i', 1)", [&sha]).unwrap();
    let r = store_blob(&x, &e.store, "me", &sha, &bytes).unwrap();
    assert_eq!((r.size, r.width, r.height), (bytes.len(), 64, 48), "the same answer as an unblocked upload");
    assert_eq!(e.store.get(&format!("blobs/{sha}")).unwrap(), None, "the real bytes are never stored");
    let stand_in = e.store.get("quarantine/upload/me").unwrap().unwrap();
    assert_eq!(stand_in, vec![0u8; bytes.len()]);
    assert_eq!(count(&e.conn, "SELECT COUNT(*) FROM r2_orphans WHERE key = 'quarantine/upload/me' AND next_at = 5000 + 86400000"), 1, "swept within a day");
    assert_eq!(usage(&x, "me").unwrap(), bytes.len() as i64, "and charged the same");
}

#[test]
fn an_upload_past_the_storage_quota_is_413_with_the_numbers_and_an_erased_account_is_401() {
    let e = env();
    let x = PoolCtx::new(&e.conn, 1_000);
    let bytes = fixture("pil.gif");
    let sha = sha256_hex(&bytes);
    e.conn.execute("INSERT INTO pool_limits (user_id, metric, value, updated_at, updated_by) VALUES ('me', 'storage_bytes', ?1, 1, 'a')", [bytes.len() as i64 + 10]).unwrap();
    // No items yet, so only the pending image is charged: the cap leaves room for the first image and 10 bytes more.
    store_blob(&x, &e.store, "me", &sha, &bytes).unwrap();
    let other = fixture("pil_lossy.webp");
    let q = err(store_blob(&x, &e.store, "me", &sha256_hex(&other), &other).unwrap_err());
    assert_eq!((q.status, q.code.as_str()), (413, "quota_exceeded"));
    assert_eq!(q.extra["needed"], json!(other.len()));
    assert_eq!(q.extra["used"], json!(bytes.len()));
    assert_eq!(q.extra["limit"], json!(bytes.len() + 10));
    assert_eq!(e.store.get(&format!("blobs/{}", sha256_hex(&other))).unwrap(), None, "nothing is written past the quota");
    e.conn.execute("INSERT INTO users (id, trust, created_at, deleted_at) VALUES ('gone', 'teacher', 1, 5)", []).unwrap();
    assert_eq!(err(store_blob(&x, &e.store, "gone", &sha, &bytes).unwrap_err()).status, 401);
}

#[test]
fn preflight_checks_the_sha_the_declared_length_and_charges_the_day() {
    let e = env();
    let x = PoolCtx::new(&e.conn, 1_000);
    let sha = "a".repeat(64);
    assert_eq!(err(preflight(&x, "me", "nope", Some("10")).unwrap_err()).code, "invalid_request");
    let n = err(preflight(&x, "me", &sha, None).unwrap_err());
    assert_eq!((n.status, n.code.as_str()), (411, "length_required"));
    for bad in ["", "-1", "1e3", "abc", "1234567890123", "2097153"] {
        let t = err(preflight(&x, "me", &sha, Some(bad)).unwrap_err());
        assert_eq!((t.status, t.code.as_str()), (413, "body_too_large"), "{bad:?}");
        assert_eq!(t.extra["limit"], json!(MAX_IMAGE_BYTES));
    }
    assert_eq!(preflight(&x, "me", &sha, Some("2097152")).unwrap(), MAX_IMAGE_BYTES);
    assert_eq!(count(&e.conn, "SELECT count FROM usage_daily WHERE metric = 'blob_uploads'"), 1);
    assert_eq!(count(&e.conn, "SELECT count FROM usage_daily WHERE metric = 'blob_bytes'"), MAX_IMAGE_BYTES as i64);
    // The daily byte allowance is charged on the declared size, before any body is read.
    e.conn.execute("INSERT INTO pool_limits (user_id, metric, value, updated_at, updated_by) VALUES ('me', 'blob_bytes', 100, 1, 'a')", []).unwrap();
    let d = err(preflight(&x, "me", &sha, Some("101")).unwrap_err());
    assert_eq!((d.status, d.code.as_str()), (429, "daily_limit"));
}

#[test]
fn concurrent_puts_into_one_directory_do_not_share_a_temp_file() {
    let dir = tempfile::tempdir().unwrap();
    let store = std::sync::Arc::new(BlobStore::new(dir.path()).unwrap());
    let threads: Vec<_> = (0..8u8)
        .map(|t| {
            let s = store.clone();
            std::thread::spawn(move || {
                for k in 0..50u8 {
                    s.put(&format!("blobs/{t:02x}{k:02x}"), &vec![t; 2000]).unwrap();
                }
            })
        })
        .collect();
    for t in threads {
        t.join().unwrap();
    }
    for t in 0..8u8 {
        for k in 0..50u8 {
            assert_eq!(store.get(&format!("blobs/{t:02x}{k:02x}")).unwrap().unwrap(), vec![t; 2000]);
        }
    }
}
