//! What a read leaves behind (worker/src/pool/read.ts `recordServed`, `holdImportedImages`, `tagsFor`): the `imports` and
//! `served` rows that bind a reader to what they were shown, the hold on an import's images, and the tags of a publish.
//!
//! The Worker's `beforeServe` test hook (a seam to land a takedown between the body read and the record) has no counterpart
//! here: tests change the database between the two calls instead.
use super::ctx::{Fail, PoolCtx, PoolError, Res};
use super::itemview::{Form, ItemRow};
use super::sqlfrag::{DAY_MS, alive_sql, usage_sql, visible_to_others};
use rusqlite::params;

pub const IMPORT_IMAGE_HOLD_MS: i64 = 30 * DAY_MS;
/// How long a privacy-purged item is listed to its author and appealable (the bytes' grace window).
pub const PRIVACY_APPEAL_MS: i64 = 14 * DAY_MS;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ServeKind {
    Import,
    View,
}
impl ServeKind {
    fn as_str(self) -> &'static str {
        match self {
            ServeKind::Import => "import",
            ServeKind::View => "view",
        }
    }
}

/// Tags (incl. merged topics) and standards of a publish, served with the item and kept on import copies.
pub fn tags_for(x: &PoolCtx, publish_id: &str) -> Res<(Vec<String>, Vec<String>)> {
    let col = |sql: &str| -> rusqlite::Result<Vec<String>> {
        let mut st = x.conn.prepare(sql)?;
        st.query_map([publish_id], |r| r.get(0))?.collect()
    };
    Ok((col("SELECT tag FROM item_tags WHERE publish_id = ?1 ORDER BY tag")?, col("SELECT code FROM item_standards WHERE publish_id = ?1 ORDER BY code")?))
}

/// Records what a non-author was served (an import, or a 'view' for a plain read), only if the item is STILL visible after
/// the body was read: a takedown or hold that landed meanwhile turns it into a 404 with nothing recorded (one atomic
/// transaction). A view never downgrades an import. Returns whether the item was still visible.
pub fn record_served(x: &PoolCtx, me: &str, r: &ItemRow, form: Form, kind: ServeKind) -> Res<bool> {
    let now = x.now;
    // Binding never depends on a per-account row cap. Reads are bounded by the number of DISTINCT publishes read
    // (`served`, one tiny row each; past reads_max a read of a publish not served before is refused), imports by the
    // daily allowance and a total of distinct items (imports_max); neither ever blocks the other, and nothing recorded is
    // ever evicted. (The totals are counted only for something new to this account: a repeat read costs one lookup.)
    let (seen, reads, imported, imports): (i64, i64, i64, i64) = x.conn.query_row(
        "SELECT x.seen,
              CASE WHEN x.seen = 0 AND ?4 = 'view' THEN (SELECT COUNT(*) FROM served WHERE reader_id = ?1 AND kind = 'view') ELSE 0 END AS reads,
              x.imported,
              CASE WHEN x.imported = 0 AND ?4 = 'import' THEN (SELECT COUNT(DISTINCT item_id) FROM imports WHERE user_id = ?1 AND kind = 'import') ELSE 0 END AS imports
         FROM (SELECT EXISTS (SELECT 1 FROM served WHERE item_id = ?2 AND reader_id = ?1) AS seen,
                      EXISTS (SELECT 1 FROM imports WHERE user_id = ?1 AND item_id = ?2 AND kind = 'import') AS imported) x",
        params![me, r.id, r.current_publish_id, kind.as_str()],
        |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
    )?;
    // (An import is never refused by the reads count: its own allowance bounds it.)
    if kind == ServeKind::View && seen == 0 && reads >= x.limit_of(me, "reads_max")? {
        return Err(PoolError::new(413, "reads_quota").with("message", "Your account has read more shared items than it can keep a record of. Ask an admin to raise your limit (reads_max).".into()).into());
    }
    if kind == ServeKind::Import && imported == 0 && imports >= x.limit_of(me, "imports_max")? {
        return Err(PoolError::new(413, "imports_quota").with("message", "Your account has imported as many shared items as it can (imports_max). Ask an admin to raise your limit.".into()).into());
    }
    // Recorded only while the publish read still exists (an author who republishes during the read can prune it), checked
    // in the same transaction, so a serve is never recorded against a publish that left no ghost; the read is then a 404
    // (retry) with nothing recorded.
    let live = |p: &str| format!("EXISTS (SELECT 1 FROM item_publishes sp WHERE sp.publish_id = {p} AND sp.status IN ('current', 'previous'))");
    let cond = |item: &str| format!("EXISTS (SELECT 1 FROM items i JOIN users u ON u.id = i.author_id WHERE i.id = {item} AND {})", visible_to_others());
    let tx = x.conn.unchecked_transaction()?;
    // Keyed on the PUBLIC publish id, so re-importing after a hidden-only republish is the same row.
    let imports_changed = tx.execute(
        &format!(
            "INSERT INTO imports (user_id, item_id, publish_id, version, form, license, author_id, at, kind, real_publish_id, licenses_json, first_at)
         SELECT ?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, json_array(?6), ?8 WHERE {} AND {} AND {}
         ON CONFLICT (user_id, item_id, publish_id) DO UPDATE SET at = excluded.at, real_publish_id = excluded.real_publish_id, license = excluded.license,
           licenses_json = (SELECT json_group_array(v) FROM (SELECT DISTINCT value AS v FROM json_each(COALESCE(imports.licenses_json, json_array(imports.license)))
                                                               UNION SELECT excluded.license)),
           kind = CASE WHEN excluded.kind = 'import' THEN 'import' ELSE imports.kind END",
            cond("?2"),
            live("?10"),
            alive_sql("?1")
        ),
        params![me, r.id, r.public_publish_id, r.p_public_version, form.as_str(), r.p_license, r.author_id, now, kind.as_str(), r.current_publish_id],
    )?;
    // The binding evidence: this reader was served THIS publish, under this licence.
    tx.execute(
        &format!(
            "INSERT INTO served (reader_id, item_id, publish_id, kind, license, licenses_json, first_at, at)
         SELECT ?1, ?2, ?3, ?4, ?5, json_array(?5), ?6, ?6 WHERE {} AND {} AND {}
         ON CONFLICT (reader_id, publish_id) DO UPDATE SET at = excluded.at, license = excluded.license,
           kind = CASE WHEN excluded.kind = 'import' THEN 'import' ELSE served.kind END,
           licenses_json = (SELECT json_group_array(DISTINCT v) FROM (SELECT value AS v FROM json_each(served.licenses_json) UNION SELECT excluded.license))",
            cond("?2"),
            live("?3"),
            alive_sql("?1")
        ),
        params![me, r.id, r.current_publish_id, kind.as_str(), r.p_license, now],
    )?;
    let still: bool = tx.query_row(&format!("SELECT EXISTS (SELECT 1 WHERE {} AND {})", cond("?1"), live("?2")), params![r.id, r.current_publish_id], |row| row.get(0))?;
    tx.commit()?;
    // Both inserts carry the account's own "not erased" guard; a request that was authenticated before the erasure
    // committed records nothing and answers like an unauthenticated one.
    if imports_changed == 0 {
        x.throw_if_erased(me)?;
    }
    Ok(still)
}

/// What `hold_imported_images` did: the number of images held and until when, or why none were.
#[derive(Debug, PartialEq, Eq)]
pub struct Held {
    pub held: i64,
    pub until: Option<i64>,
    pub reason: Option<&'static str>,
}

/// Holds the served images of an import for the importer, if their quota allows; one statement.
pub fn hold_imported_images(x: &PoolCtx, me: &str, publish_id: &str, form: Form) -> Res<Held> {
    let now = x.now;
    let until = now + IMPORT_IMAGE_HOLD_MS;
    let quota = x.limit_of(me, "storage_bytes")?;
    let n: i64 = x.conn.query_row("SELECT COUNT(*) FROM item_blobs WHERE publish_id = ?1 AND form = ?2", params![publish_id, form.as_str()], |r| r.get(0))?;
    if n == 0 {
        return Ok(Held { held: 0, until: None, reason: None });
    }
    let changed = x
        .conn
        .execute(
            &format!(
                "INSERT INTO pending_blobs (sha256, uploader_id, size, until)
       SELECT b.sha256, ?1, b.size, ?5 FROM item_blobs ib JOIN blobs b ON b.sha256 = ib.sha256
        WHERE ib.publish_id = ?3 AND ib.form = ?4 AND {}
          AND {} + (SELECT COALESCE(SUM(b2.size), 0) FROM item_blobs ib2 JOIN blobs b2 ON b2.sha256 = ib2.sha256
                               WHERE ib2.publish_id = ?3 AND ib2.form = ?4
                                 AND NOT EXISTS (SELECT 1 FROM pending_blobs pb WHERE pb.sha256 = b2.sha256 AND pb.uploader_id = ?1 AND pb.until >= ?2)) <= ?6
       ON CONFLICT (sha256, uploader_id) DO UPDATE SET until = MAX(until, excluded.until)",
                alive_sql("?1"),
                usage_sql(None)
            ),
            params![me, now, publish_id, form.as_str(), until, quota],
        )
        .map_err(Fail::from)?;
    if changed == 0 {
        x.throw_if_erased(me)?;
    }
    Ok(if changed > 0 { Held { held: n, until: Some(until), reason: None } } else { Held { held: 0, until: None, reason: Some("quota") } })
}

#[cfg(test)]
#[path = "recordserved_tests.rs"]
mod tests;
