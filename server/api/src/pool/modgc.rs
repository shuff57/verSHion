//! The storage and strike machinery of worker/src/pool/moderation.ts: blob gc, R2 orphan deletes with back-off, the repeat-infringer policy and
//! evidence voiding. (The purge, takedown and put-back flows build on these.)
use super::ctx::{PoolCtx, PoolNotice, Res};
use super::moderation::{STRIKES_TO_SUSPEND, STRIKE_WINDOW_MS};
use super::sqlfrag::{HOUR_MS, new_id};
use crate::blobs::BlobStore;
use rusqlite::{Connection, params};
use serde_json::json;

/// Deletes blobs nothing references and nobody has pending (rows first, then the bytes). `only`: just these hashes.
pub fn gc_blobs(x: &PoolCtx, store: &BlobStore, only: Option<&[String]>) -> Res<usize> {
    let now = x.now;
    let only_json = only.map(|o| json!(o).to_string());
    let mut st = x.conn.prepare(
        "DELETE FROM blobs WHERE sha256 IN (
           SELECT b.sha256 FROM blobs b
            WHERE (?1 IS NULL OR b.sha256 IN (SELECT value FROM json_each(?1)))
              AND NOT EXISTS (SELECT 1 FROM item_blobs ib WHERE ib.sha256 = b.sha256)
              AND NOT EXISTS (SELECT 1 FROM circle_version_blobs cvb WHERE cvb.sha256 = b.sha256)
              AND NOT EXISTS (SELECT 1 FROM pending_blobs pb WHERE pb.sha256 = b.sha256 AND pb.until >= ?2)
            LIMIT 500)
         RETURNING sha256",
    )?;
    let gone: Vec<String> = st.query_map(params![only_json, now - HOUR_MS], |r| r.get(0))?.collect::<Result<_, _>>()?;
    drop(st);
    if gone.is_empty() {
        return Ok(0);
    }
    // An upload that landed in between wrote a live pending row (and re-inserts the blobs row after its put): keep those bytes.
    let mut st = x.conn.prepare(
        "SELECT sha256 FROM pending_blobs WHERE sha256 IN (SELECT value FROM json_each(?1)) AND until >= ?2
         UNION SELECT sha256 FROM blobs WHERE sha256 IN (SELECT value FROM json_each(?1))",
    )?;
    let keep: std::collections::HashSet<String> = st.query_map(params![json!(gone).to_string(), now - HOUR_MS], |r| r.get(0))?.collect::<Result<_, _>>()?;
    drop(st);
    let mut dropped = 0;
    for s in gone.iter().filter(|s| !keep.contains(*s)) {
        let _ = store.delete(&format!("blobs/{s}"));
        dropped += 1;
    }
    Ok(dropped)
}

/// Deletes stored keys recorded in r2_orphans and forgets them; a failure leaves them for the hourly retry.
pub fn delete_keys(x: &PoolCtx, store: &BlobStore, keys: &[String]) -> Res<bool> {
    for k in keys {
        if store.delete(k).is_err() {
            return Ok(false);
        }
    }
    x.conn.execute("DELETE FROM r2_orphans WHERE key IN (SELECT value FROM json_each(?1))", [json!(keys).to_string()])?;
    Ok(true)
}

/// Hourly: deletes a purge could not finish. Each key backs off (1 h, 2 h, 4 h ... at most a day) on failure.
pub fn retry_orphans(x: &PoolCtx, store: &BlobStore) -> Res<usize> {
    let mut st = x.conn.prepare("SELECT key FROM r2_orphans WHERE next_at <= ?1 ORDER BY attempts, at LIMIT 500")?;
    let keys: Vec<String> = st.query_map([x.now], |r| r.get(0))?.collect::<Result<_, _>>()?;
    drop(st);
    if keys.is_empty() {
        return Ok(0);
    }
    if delete_keys(x, store, &keys)? {
        return Ok(keys.len());
    }
    x.conn.execute(
        "UPDATE r2_orphans SET attempts = attempts + 1, next_at = ?2 + MIN(86400000, 3600000 * (1 << MIN(attempts, 5))) WHERE key IN (SELECT value FROM json_each(?1))",
        params![json!(keys).to_string(), x.now],
    )?;
    Ok(0)
}

/// The repeat-infringer policy, on UPHELD strikes only: raises an admin flag (once per count) and mails; the admin suspends through the audited route.
pub fn repeat_infringer_check(x: &PoolCtx, user_id: &str) -> Res<()> {
    let n: i64 = x.conn.query_row("SELECT COUNT(*) FROM strikes WHERE user_id = ?1 AND upheld_at IS NOT NULL AND voided_at IS NULL AND at > ?2", params![user_id, x.now - STRIKE_WINDOW_MS], |r| r.get(0))?;
    if n < STRIKES_TO_SUSPEND {
        return Ok(());
    }
    let ins = x.conn.execute(
        "INSERT OR IGNORE INTO admin_flags (id, kind, user_id, detail, dedup, at) VALUES (?1, 'repeat_infringer', ?2, ?3, ?4, ?5)",
        params![new_id(), user_id, json!({ "strikes": n }).to_string(), format!("strikes:{user_id}:{n}"), x.now],
    )?;
    if ins == 1 {
        x.send(PoolNotice::RepeatInfringer { user_id: user_id.into(), strikes: n });
    }
    Ok(())
}

/// A repeat-infringer check that failed is redone by the cron, from the strikes.
pub fn repeat_infringer_sweep(x: &PoolCtx) -> Res<usize> {
    let mut st = x.conn.prepare(
        "SELECT user_id FROM (SELECT user_id, COUNT(*) AS n FROM strikes WHERE upheld_at IS NOT NULL AND voided_at IS NULL AND at > ?1 GROUP BY user_id) s
          WHERE s.n >= ?2 AND NOT EXISTS (SELECT 1 FROM admin_flags f WHERE f.dedup = 'strikes:' || s.user_id || ':' || s.n) LIMIT 50",
    )?;
    let due: Vec<String> = st.query_map(params![x.now - STRIKE_WINDOW_MS, STRIKES_TO_SUSPEND], |r| r.get(0))?.collect::<Result<_, _>>()?;
    drop(st);
    for u in &due {
        repeat_infringer_check(x, u)?;
    }
    Ok(due.len())
}

/// Whose evidence: everything recorded from an item, or from everything an account was served.
pub enum Who<'a> {
    Item(&'a str),
    User(&'a str),
}

/// Voids (or restores) the evidence recorded from an item, or from everything an account was served. An item's void also marks the item itself
/// (`void_items`): links to it in anyone's chain stop granting and stop being shown, and sources whose lineage runs through it count only for a
/// verbatim copy of their own additions.
pub fn void_evidence(conn: &Connection, who: Who, now: i64, restore: bool, by: Option<&str>, reason: &str, replaced_by: Option<&str>) -> rusqlite::Result<()> {
    let (scol, id) = match who {
        Who::Item(i) => ("item_id", i),
        Who::User(u) => ("reader_id", u),
    };
    if restore {
        conn.execute(&format!("UPDATE served SET void_at = NULL WHERE {scol} = ?1 AND void_at IS NOT NULL"), [id])?;
    } else {
        conn.execute(&format!("UPDATE served SET void_at = ?2 WHERE {scol} = ?1 AND void_at IS NULL"), params![id, now])?;
    }
    if let Who::Item(item) = who {
        if restore {
            conn.execute("DELETE FROM void_items WHERE item_id = ?1", [item])?;
        } else {
            conn.execute(
                "INSERT OR IGNORE INTO void_items (item_id, at, by, reason, replaced_by, replaced_license, replaced_author_id)
                 SELECT ?1, ?2, ?3, ?4, ?5, ri.license, ri.author_id FROM (SELECT 1) LEFT JOIN items ri ON ri.id = ?5",
                params![item, now, by, reason, replaced_by],
            )?;
        }
    }
    Ok(())
}

#[cfg(test)]
#[path = "modgc_tests.rs"]
mod tests;
