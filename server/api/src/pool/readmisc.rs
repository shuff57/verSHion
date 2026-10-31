//! POST /items/versions and GET /items/:id/blobs/:sha (worker/src/pool/read.ts `versions`, `getBlob`), HTTP-free. The
//! route reads the optional JSON body of `versions` (and passes its `ids` here) and writes the blob reply with
//! `blob_headers`.
use super::ctx::{PoolCtx, PoolError, Res, bad, not_found};
use super::sqlfrag::{is_id, is_sha, visible_to, visible_to_others};
use crate::blobs::BlobStore;
use rusqlite::{OptionalExtension, params};
use serde_json::{Value, json};
use std::collections::HashMap;

/// The `ids` of a versions request: 1 to 200 strings of at most 64 UTF-16 units.
pub fn check_ids(ids: Option<&Value>) -> Result<Vec<String>, PoolError> {
    let arr = ids.and_then(|v| v.as_array()).ok_or_else(|| bad("ids"))?;
    if arr.is_empty() || arr.len() > 200 {
        return Err(bad("ids"));
    }
    arr.iter().map(|v| v.as_str().filter(|s| s.encode_utf16().count() <= 64).map(String::from).ok_or_else(|| bad("ids"))).collect()
}

struct Row {
    author_id: Option<String>,
    version: Option<i64>,
    removed_reason: Option<String>,
    mine: Option<i64>,
    seen: bool,
    purged_at: Option<i64>,
    license: Option<String>,
    license_version: Option<i64>,
    purge_after: Option<i64>,
    held: bool,
    visible: bool,
}

/// For each id asked about, where it stands for the caller: `current`, `newer` (than the imported copy), `under_review`,
/// `removed` or `unavailable`. Only for an item the caller authored, imported or read: no oracle for arbitrary ids.
pub fn versions(x: &PoolCtx, me: &str, ids: &[String]) -> Res<Value> {
    x.require_daily(me, "item_reads", 1)?;
    let mut uniq: Vec<&String> = Vec::new();
    for id in ids {
        if is_id(id) && !uniq.contains(&id) {
            uniq.push(id);
        }
    }
    let sql = format!(
        "SELECT j.value AS id, i.author_id, CASE WHEN i.author_id = ?1 THEN i.version ELSE i.public_version END AS version, i.removed_reason,
              (SELECT MAX(version) FROM imports im WHERE im.user_id = ?1 AND im.item_id = j.value AND im.kind = 'import') AS mine,
              EXISTS (SELECT 1 FROM imports im WHERE im.user_id = ?1 AND im.item_id = j.value) AS seen,
              (SELECT MIN(h.at) FROM item_holds h WHERE h.item_id = j.value AND h.source = 'privacy') AS privacy_held_at, i.purged_at,
              i.license, i.license_version, i.purge_after, EXISTS (SELECT 1 FROM item_holds h WHERE h.item_id = j.value) AS held,
              CASE WHEN i.id IS NULL THEN 0
                   WHEN i.author_id = ?1 THEN (i.removed_at IS NULL AND i.current_publish_id IS NOT NULL)
                   ELSE ({}) END AS visible
         FROM json_each(?2) j LEFT JOIN items i ON i.id = j.value LEFT JOIN users u ON u.id = i.author_id",
        visible_to_others()
    );
    let mut st = x.conn.prepare(&sql)?;
    let by: HashMap<String, Row> = st
        .query_map(params![me, serde_json::to_string(&uniq).unwrap()], |r| {
            Ok((
                r.get::<_, String>("id")?,
                Row {
                    author_id: r.get("author_id")?,
                    version: r.get("version")?,
                    removed_reason: r.get("removed_reason")?,
                    mine: r.get("mine")?,
                    seen: r.get::<_, i64>("seen")? != 0,
                    purged_at: r.get("purged_at")?,
                    license: r.get("license")?,
                    license_version: r.get("license_version")?,
                    purge_after: r.get("purge_after")?,
                    held: r.get::<_, i64>("held")? != 0,
                    visible: r.get::<_, i64>("visible")? != 0,
                },
            ))
        })?
        .collect::<Result<_, _>>()?;
    let out: Vec<Value> = ids.iter().map(|id| standing(id, by.get(id), me)).collect();
    Ok(json!({ "items": out }))
}

fn standing(id: &str, r: Option<&Row>, me: &str) -> Value {
    let unavailable = || json!({ "id": id, "status": "unavailable" });
    // A preview read never counts as having the newest version (it must not hide an update notice).
    let Some(r) = r else { return unavailable() };
    let Some(author) = r.author_id.as_deref() else { return unavailable() };
    if author != me && !r.seen {
        return unavailable();
    }
    // A HELD item, whatever the hold, is "under_review" until it is purged: a hold can always be lifted, so the client
    // keeps its copy. Which kind of block an item hit is never observable through another account.
    if r.removed_reason.is_none() && r.held && r.purged_at.is_none() {
        return json!({ "id": id, "status": "under_review", "reason": "held" });
    }
    if r.removed_reason.as_deref() == Some("privacy") && r.purged_at.is_none() {
        return json!({ "id": id, "status": "under_review", "reason": "privacy" });
    }
    if let Some(why) = r.removed_reason.as_deref() {
        let reason = match why {
            "takedown" => "takedown",
            "privacy" => "privacy",
            "admin" => "admin",
            _ => "deleted",
        };
        // Every still-reversible state: an author deletion within its undo window, an admin removal before it can be
        // undone, a takedown in quarantine: "under_review", with the date it becomes final when there is one. "removed"
        // only once it is purged.
        if r.purged_at.is_none() {
            let mut v = json!({ "id": id, "status": "under_review", "reason": reason });
            if let Some(u) = r.purge_after {
                v["until"] = json!(u);
            }
            return v;
        }
        return json!({ "id": id, "status": "removed", "reason": reason });
    }
    if !r.visible {
        return unavailable();
    }
    // "newer" only against an imported copy; a caller who only read it holds no copy to update. The licence and its own
    // counter: a licence-only change is visible as such, without implying anything else changed.
    let status = if author != me && r.mine.is_some_and(|m| m < r.version.unwrap_or(0)) { "newer" } else { "current" };
    json!({ "id": id, "status": status, "version": r.version, "license": r.license, "license_version": r.license_version })
}

pub fn ext_of(content_type: &str) -> &'static str {
    match content_type {
        "image/png" => "png",
        "image/jpeg" => "jpg",
        "image/gif" => "gif",
        "image/webp" => "webp",
        _ => "bin",
    }
}

#[derive(Debug)]
pub struct BlobOut {
    pub content_type: String,
    pub bytes: Vec<u8>,
    pub sha: String,
}

/// The headers of a served image: only ever the type verified from the bytes at upload.
pub fn blob_headers(b: &BlobOut) -> Vec<(&'static str, String)> {
    vec![
        ("Content-Type", b.content_type.clone()),
        ("X-Content-Type-Options", "nosniff".into()),
        ("Content-Security-Policy", "sandbox; default-src 'none'".into()),
        ("Content-Disposition", format!("attachment; filename=\"{}.{}\"", b.sha, ext_of(&b.content_type))),
        ("Cache-Control", "no-store".into()),
        ("Cross-Origin-Resource-Policy", "same-site".into()),
        ("Vary", "Authorization, Cookie, Origin".into()),
    ]
}

/// Readable iff the caller may see the item AND the blob is listed under the caller's served form of the CURRENT publish.
/// A miss and a forbidden read are the same 404.
pub fn get_blob(x: &PoolCtx, store: &BlobStore, me: &str, id: &str, sha: &str) -> Res<BlobOut> {
    x.miss_guard(me, "blob_miss")?;
    let miss = || -> Res<BlobOut> {
        x.count_miss(me, "blob_miss")?;
        Err(not_found().into())
    };
    if !is_id(id) || !is_sha(sha) {
        return miss();
    }
    let sql = format!(
        "SELECT b.content_type FROM items i JOIN users u ON u.id = i.author_id
         JOIN item_publishes p ON p.publish_id = i.current_publish_id
         JOIN item_blobs ib ON ib.publish_id = i.current_publish_id AND ib.sha256 = ?3
              AND ib.form = CASE WHEN i.author_id = ?2 OR p.hide_key = 0 THEN 'full' ELSE 'stripped' END
         JOIN blobs b ON b.sha256 = ib.sha256
        WHERE i.id = ?1 AND {}
          -- An image confirmed as student data is served to nobody, from any item.
          AND NOT EXISTS (SELECT 1 FROM privacy_blocks pb WHERE pb.kind = 'blob' AND pb.hash = ?3)
          -- Past its grace window it stays gone for every served use until uploaded again; only its
          -- author's own unserved (hidden full-form) use may still read it.
          AND (b.privacy_purged_at IS NULL OR (i.author_id = ?2 AND p.hide_key = 1
               AND NOT EXISTS (SELECT 1 FROM item_blobs us WHERE us.publish_id = i.current_publish_id AND us.form = 'stripped' AND us.sha256 = ?3)))",
        visible_to("?2")
    );
    let Some(content_type): Option<String> = x.conn.query_row(&sql, params![id, me, sha], |r| r.get(0)).optional()? else { return miss() };
    match store.get(&format!("blobs/{sha}")).ok().flatten() {
        Some(bytes) => Ok(BlobOut { content_type, bytes, sha: sha.into() }),
        None => miss(),
    }
}

#[cfg(test)]
#[path = "readmisc_tests.rs"]
mod tests;
