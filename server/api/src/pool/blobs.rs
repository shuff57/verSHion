//! PUT /blobs/:sha256 (worker/src/pool/blobs.ts; docs/shared-pool.md "Blob rules", "Images"). Full bytes always; the hash
//! is recomputed; the format is sniffed from the bytes; the blob is charged to the uploader as pending (quota checked in the
//! same INSERT); the answer is the same whether or not the blob existed, so an upload is never an existence oracle.
//!
//! This is the HTTP-free core: `preflight` is everything the Worker did before reading the body (headers, daily
//! allowances), `store_blob` everything after it. The route reads the body between them.
use super::ctx::{PoolCtx, PoolError, Res};
use super::image::{MAX_IMAGE_BYTES, check_image, png_pixel_hash};
use super::sqlfrag::{DAY_MS, REMOVED_SQL, alive_sql, is_sha, hex, usage_sql};
use crate::blobs::BlobStore;
use rusqlite::{OptionalExtension, params};
use serde_json::json;
use sha2::{Digest, Sha256};

/// A pending upload stays charged (and publishable) this long.
pub const PENDING_MS: i64 = DAY_MS;

#[derive(Debug, PartialEq, Eq)]
pub struct Stored {
    pub sha256: String,
    pub size: usize,
    pub width: usize,
    pub height: usize,
}

/// Bytes charged to `user_id` now (storage_bytes counts against this).
pub fn usage(x: &PoolCtx, user_id: &str) -> rusqlite::Result<i64> {
    x.conn.query_row(&format!("SELECT {} AS used", usage_sql(None)), params![user_id, x.now], |r| r.get(0))
}

/// Everything before the body is read: the sha is well formed, a Content-Length is declared and within the image cap, and
/// the day's uploads and the declared bytes are charged (round 7 H3). Returns the declared length.
pub fn preflight(x: &PoolCtx, me: &str, sha: &str, content_length: Option<&str>) -> Res<usize> {
    if !is_sha(sha) {
        return Err(PoolError::new(400, "invalid_request").with("field", json!("sha256")).into());
    }
    let Some(declared) = content_length else {
        return Err(PoolError::new(411, "length_required").with("message", json!("Send the image with a Content-Length.")).into());
    };
    let n = if !declared.is_empty() && declared.len() <= 12 && declared.bytes().all(|b| b.is_ascii_digit()) { declared.parse::<usize>().unwrap_or(usize::MAX) } else { usize::MAX };
    if n > MAX_IMAGE_BYTES {
        return Err(PoolError::new(413, "body_too_large").with("limit", json!(MAX_IMAGE_BYTES)).into());
    }
    x.require_daily(me, "blob_uploads", 1)?;
    x.require_daily(me, "blob_bytes", n as i64)?;
    Ok(n)
}

pub fn sha256_hex(b: &[u8]) -> String {
    hex(&Sha256::digest(b))
}

/// Stores `bytes` as the blob `sha` for `me`.
///
/// A well-formed blocked image is ACCEPTED exactly like any other (same answer, same charge, a write of the same size), but
/// its bytes are never stored: a zero-filled stand-in goes to a quarantine key, and a publish that serves it is held by the
/// block (student_data), never refused. The uploader knows whether their own file is well formed, so refusing a blocked
/// image would confirm the block.
pub fn store_blob(x: &PoolCtx, store: &BlobStore, me: &str, sha: &str, bytes: &[u8]) -> Res<Stored> {
    if sha256_hex(bytes) != sha {
        return Err(PoolError::new(422, "hash_mismatch").into());
    }
    // ONE answer for every refused image: bytes confirmed as student data get exactly what a malformed or unsupported file
    // gets, after the same work, so an upload never confirms a privacy block.
    let info = check_image(bytes).map_err(|_| PoolError::new(422, "image_refused").with("reason", json!("not_accepted")))?;
    let blocked = x.conn.query_row("SELECT 1 FROM privacy_blocks WHERE kind = 'blob' AND hash = ?1", [sha], |_| Ok(())).optional()?.is_some();
    let now = x.now;
    let quota = x.limit_of(me, "storage_bytes")?;
    // Charged once per (blob, uploader): a blob this author already pays for (pending or in one of their publishes) costs
    // nothing more. Another author's copy of the same image costs this one too.
    let changed = x.conn.execute(
        &format!(
            "INSERT INTO pending_blobs (sha256, uploader_id, size, until)
       SELECT ?3, ?1, ?4, ?5
        WHERE {} AND (CASE WHEN EXISTS (SELECT 1 FROM pending_blobs WHERE sha256 = ?3 AND uploader_id = ?1 AND until >= ?2)
                      OR EXISTS (SELECT 1 FROM item_blobs ib JOIN item_publishes ip ON ip.publish_id = ib.publish_id WHERE ib.sha256 = ?3 AND ip.author_id = ?1)
                    THEN 0 ELSE ?4 END) + {} <= ?6
         AND {} <= ?7
       ON CONFLICT (sha256, uploader_id) DO UPDATE SET until = excluded.until, size = excluded.size",
            alive_sql("?1"),
            super::sqlfrag::usage_sql(None),
            REMOVED_SQL
        ),
        params![me, now, sha, bytes.len() as i64, now + PENDING_MS, quota, x.limit_of(me, "removed_bytes")?],
    )?;
    if changed == 0 {
        x.throw_if_erased(me)?;
        return Err(PoolError::new(413, "quota_exceeded").with("used", json!(usage(x, me)?)).with("needed", json!(bytes.len())).with("limit", json!(quota)).into());
    }
    // When this account first uploaded these bytes (ordering evidence for an image independence claim).
    x.conn.execute(&format!("INSERT OR IGNORE INTO blob_uploads (sha256, uploader_id, first_at) SELECT ?1, ?2, ?3 WHERE {}", alive_sql("?2")), params![sha, me, now])?;
    // Content-addressed: rewriting identical bytes is harmless, and it repairs a GC that raced us.
    if blocked {
        let key = format!("quarantine/upload/{me}");
        store.put(&key, &vec![0u8; bytes.len()]).map_err(io)?;
        // The stand-in is removed within a day by the orphan sweep.
        x.conn.execute("INSERT INTO r2_orphans (key, at, next_at) VALUES (?1, ?2, ?3) ON CONFLICT (key) DO UPDATE SET next_at = excluded.next_at", params![key, now, now + DAY_MS])?;
    } else {
        store.put(&format!("blobs/{sha}"), bytes).map_err(io)?;
    }
    // A fresh upload brings back an image whose privacy grace window had ended (after an unblock); never one still blocked.
    // The decoded-pixel hash (PNG) makes a re-encode the same image.
    let pixel = if info.content_type == "image/png" { png_pixel_hash(bytes) } else { None };
    x.conn.execute(
        "INSERT INTO blobs (sha256, size, content_type, width, height, first_uploaded_at, checked_at, pixel_hash) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?6, ?8)
             ON CONFLICT (sha256) DO UPDATE SET missing_at = CASE WHEN ?7 = 1 THEN missing_at ELSE NULL END, privacy_purged_at = CASE WHEN ?7 = 1 THEN privacy_purged_at ELSE NULL END, checked_at = ?6,
                                                pixel_hash = COALESCE(blobs.pixel_hash, excluded.pixel_hash)",
        params![sha, bytes.len() as i64, info.content_type, info.width as i64, info.height as i64, now, blocked as i64, pixel],
    )?;
    Ok(Stored { sha256: sha.into(), size: bytes.len(), width: info.width, height: info.height })
}

/// A disk failure is the server's, not the caller's.
fn io(e: std::io::Error) -> rusqlite::Error {
    rusqlite::Error::ToSqlConversionFailure(Box::new(e))
}

#[cfg(test)]
#[path = "blobs_tests.rs"]
mod tests;
