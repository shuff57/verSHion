//! The pool's scheduled work, second part (worker/src/pool/cron.ts): privacy bytes, retention of audit and takedown records, and account deletions' chain
//! rewrites. Bounded and idempotent; what one run leaves, the next continues.
use super::ctx::{PoolCtx, Res};
use super::moderation::open_appeal_sql;
use super::served::used_unserved;
use super::sqlfrag::DAY_MS;
use crate::blobs::BlobStore;
use rusqlite::{OptionalExtension, params};
use serde_json::json;

/// Audit and takedown records are kept 3 years. 1,096 days is never less than three calendar years. The pool_audit trigger refuses to delete a younger row by
/// the DATABASE's clock, and the delete below carries the same floor, so a skewed application clock can neither delete early nor make the stage fail.
pub const RETENTION_MS: i64 = 1096 * DAY_MS;
/// Takedown actions removed (with their own rows) per daily run; the rest resume the next day.
pub const RETENTION_ACTIONS: i64 = 100;
/// Rows of one table (audit rows, notices, an action's block and upstream rows) removed per daily run.
pub const RETENTION_ROWS: i64 = 5000;
/// Rows per batch and batches per cron run for account deletions' chain rewrites.
pub const REWRITE_BATCH: i64 = 500;
pub const REWRITE_BATCHES: usize = 20;

fn io(e: std::io::Error) -> rusqlite::Error {
    rusqlite::Error::ToSqlConversionFailure(Box::new(e))
}

/// Privacy-blocked images past their grace window: from now on they are gone for every SERVED use (privacy_purged_at; only a fresh upload after an unblock
/// brings one back), and their bytes are deleted, except where an author still uses them unserved (only in a hidden publish's full form): that author's
/// private material is not destroyed by someone else's block.
pub fn privacy_bytes(x: &PoolCtx, store: &BlobStore) -> Res<()> {
    // An open appeal on the blocking item keeps the bytes until the admin decides.
    let doomed: Vec<String> = x
        .conn
        .prepare(&format!(
            "UPDATE privacy_blocks SET bytes_purged_at = ?1 WHERE kind = 'blob' AND bytes_purged_at IS NULL AND bytes_purge_after <= ?1 AND NOT {} RETURNING hash",
            open_appeal_sql("privacy_blocks.item_id")
        ))?
        .query_map([x.now], |r| r.get(0))?
        .collect::<Result<_, _>>()?;
    if doomed.is_empty() {
        return Ok(());
    }
    let hs = json!(doomed).to_string();
    x.conn.execute("UPDATE blobs SET privacy_purged_at = ?2 WHERE sha256 IN (SELECT value FROM json_each(?1))", params![hs, x.now])?;
    let del: Vec<String> = x
        .conn
        .prepare(&format!("UPDATE blobs SET missing_at = ?2 WHERE sha256 IN (SELECT value FROM json_each(?1)) AND NOT {} RETURNING sha256", used_unserved("blobs.sha256")))?
        .query_map(params![hs, x.now], |r| r.get(0))?
        .collect::<Result<_, _>>()?;
    for sha in del {
        store.delete(&format!("blobs/{sha}")).map_err(io)?;
    }
    Ok(())
}

fn db_retention_floor() -> String {
    format!("(CAST(strftime('%s', 'now') AS INTEGER) * 1000 - {RETENTION_MS})")
}

/// SQL condition: the takedown action `a` is a record past retention (`cut` = now - RETENTION_MS, `now`: SQL parameter references). It is CLOSED (put back,
/// withdrawn, reversed, or upheld and purged), with no uphold or put-back still finishing; NOTHING about it is newer than the cutoff; and nothing still DEPENDS
/// on it: no re-upload block of it in force, no copy it holds, no open appeal or unresolved takedown flag on its item.
pub fn retainable_action_sql(a: &str, cut: &str, now: &str) -> String {
    format!(
        "(
  ({a}.restored_at IS NOT NULL OR {a}.purged_at IS NOT NULL) AND {a}.uphold_pending = 0
  AND {a}.disabled_at < {cut} AND COALESCE({a}.author_notified_at, 0) < {cut} AND COALESCE({a}.restored_at, 0) < {cut}
  AND COALESCE({a}.purged_at, 0) < {cut} AND COALESCE({a}.court_action_at, 0) < {cut}
  AND NOT EXISTS (SELECT 1 FROM dmca_counter_notices rcn WHERE rcn.action_id = {a}.id AND rcn.received_at >= {cut})
  AND NOT EXISTS (SELECT 1 FROM strikes rs WHERE rs.action_id = {a}.id AND (COALESCE(rs.upheld_at, 0) >= {cut} OR COALESCE(rs.voided_at, 0) >= {cut}))
  AND NOT EXISTS (SELECT 1 FROM dmca_exempt rx WHERE rx.action_id = {a}.id AND rx.at >= {cut})
  AND NOT EXISTS (SELECT 1 FROM hold_releases rhr WHERE rhr.source = 'dmca:' || {a}.id AND rhr.at >= {cut})
  AND NOT EXISTS (SELECT 1 FROM pool_audit rpa WHERE rpa.item_id = {a}.item_id AND rpa.at >= {cut} AND rpa.action LIKE 'dmca.%')
  AND NOT EXISTS (SELECT 1 FROM blocked_hashes rbh WHERE rbh.action_id = {a}.id AND rbh.lifted_at IS NULL AND (rbh.expires_at IS NULL OR rbh.expires_at > {now}))
  AND NOT EXISTS (SELECT 1 FROM item_holds rh WHERE rh.source = 'dmca:' || {a}.id)
  AND NOT EXISTS (SELECT 1 FROM admin_flags rf WHERE rf.item_id = {a}.item_id AND rf.resolved_at IS NULL AND rf.kind IN ('appeal', 'counter_notice', 'dmca_copy'))
  AND NOT ({a}.restored_at IS NOT NULL
           AND EXISTS (SELECT 1 FROM items ri WHERE ri.id = {a}.item_id AND ri.removed_reason = 'takedown' AND (ri.purged_at IS NULL OR ri.purged_at < {a}.disabled_at))
           AND NOT EXISTS (SELECT 1 FROM dmca_actions rb WHERE rb.item_id = {a}.item_id AND rb.disabled_at > {a}.disabled_at)))"
    )
}

#[derive(Debug, Default, PartialEq)]
pub struct Retained {
    pub actions: usize,
    pub notices: usize,
    pub audit: usize,
}

/// The daily retention stage, oldest first. In order: (1) takedown actions past retention, with their counter-notice, strike, exempt authors, hold releases,
/// (inactive) re-upload block rows and upstream snapshot (every statement re-checks the condition inside the one transaction, so an action that changed since
/// it was picked is left whole; an action goes only once its block and snapshot rows are all gone); (2) DMCA notices received before the cutoff that no
/// takedown action still names and that are not `open`; (3) pool_audit rows older than the cutoff (and the database's own floor), except the trail of an
/// item that still has a takedown record kept, a hold or an open appeal, and rows naming a takedown action or notice still kept. Never touched: void_items,
/// admin flags, the outbox, auth_audit.
pub fn retention(x: &PoolCtx) -> Res<Retained> {
    let (now, cut) = (x.now, x.now - RETENTION_MS);
    let due: Vec<String> = x
        .conn
        .prepare(&format!("SELECT a.id FROM dmca_actions a WHERE {} ORDER BY a.disabled_at, a.id LIMIT {RETENTION_ACTIONS}", retainable_action_sql("a", "?1", "?2")))?
        .query_map(params![cut, now], |r| r.get(0))?
        .collect::<Result<_, _>>()?;
    let mut actions = 0;
    if !due.is_empty() {
        let ids = json!(due).to_string();
        let pick = format!("SELECT ra.id FROM dmca_actions ra WHERE ra.id IN (SELECT value FROM json_each(?1)) AND {}", retainable_action_sql("ra", "?2", "?3"));
        let p = params![ids, cut, now];
        let tx = x.conn.unchecked_transaction()?;
        // (WITHOUT ROWID: deleted by its key, a bounded page of it)
        tx.execute(&format!("DELETE FROM dmca_upstream_keys WHERE (action_id, item_id, key) IN (SELECT action_id, item_id, key FROM dmca_upstream_keys WHERE action_id IN ({pick}) LIMIT {RETENTION_ROWS})"), p)?;
        tx.execute(&format!("DELETE FROM blocked_hashes WHERE rowid IN (SELECT rowid FROM blocked_hashes WHERE action_id IN ({pick}) LIMIT {RETENTION_ROWS})"), p)?;
        tx.execute(&format!("DELETE FROM dmca_exempt WHERE action_id IN ({pick})"), p)?;
        tx.execute(&format!("DELETE FROM hold_releases WHERE source IN (SELECT 'dmca:' || id FROM ({pick}))"), p)?;
        tx.execute(&format!("DELETE FROM dmca_counter_notices WHERE action_id IN ({pick})"), p)?;
        tx.execute(&format!("DELETE FROM strikes WHERE action_id IN ({pick})"), p)?;
        actions = tx.execute(
            &format!(
                "DELETE FROM dmca_actions WHERE id IN ({pick})
                   AND NOT EXISTS (SELECT 1 FROM dmca_upstream_keys k WHERE k.action_id = dmca_actions.id)
                   AND NOT EXISTS (SELECT 1 FROM blocked_hashes b WHERE b.action_id = dmca_actions.id)"
            ),
            p,
        )?;
        tx.commit()?;
    }
    let notices = x.conn.execute(
        &format!(
            "DELETE FROM dmca_notices WHERE id IN (SELECT n.id FROM dmca_notices n WHERE n.received_at < ?1 AND n.status <> 'open'
                AND NOT EXISTS (SELECT 1 FROM dmca_actions a WHERE a.notice_id = n.id) ORDER BY n.received_at LIMIT {RETENTION_ROWS})"
        ),
        [cut],
    )?;
    // (the CASE keeps json_valid ahead of the extraction: an unparsable detail is never read)
    let names = |table: &str, field: &str| format!("EXISTS (SELECT 1 FROM {table} x WHERE x.id = CASE WHEN json_valid(p.detail) THEN p.detail ->> '$.{field}' END)");
    let audit = x.conn.execute(
        &format!(
            "DELETE FROM pool_audit WHERE rowid IN (SELECT p.rowid FROM pool_audit p WHERE p.at < ?1 AND p.at < {}
                AND NOT (p.item_id IS NOT NULL AND (EXISTS (SELECT 1 FROM dmca_actions a WHERE a.item_id = p.item_id)
                                                    OR EXISTS (SELECT 1 FROM item_holds h WHERE h.item_id = p.item_id)
                                                    OR {}
                                                    OR EXISTS (SELECT 1 FROM circle_reports cr WHERE cr.circle_id = p.item_id AND cr.resolved_at IS NULL)
                                                    OR EXISTS (SELECT 1 FROM circle_banks cb JOIN circle_holds ch ON ch.bank_id = cb.id WHERE cb.circle_id = p.item_id)))
                AND NOT {} AND NOT {}
              ORDER BY p.at LIMIT {RETENTION_ROWS})",
            db_retention_floor(),
            open_appeal_sql("p.item_id"),
            names("dmca_actions", "action_id"),
            names("dmca_notices", "notice_id")
        ),
        [cut],
    )?;
    Ok(Retained { actions, notices, audit })
}

/// Account deletions' rewrites of others' stored chains: the account's name in each link to one of its items becomes "deleted teacher". Bounded:
/// `batch` rows by rowid per statement, at most `batches` statements per run, resumed from the stored cursor.
pub fn deletion_rewrites(x: &PoolCtx, batch: i64, batches: usize) -> Res<()> {
    let mut left = batches;
    while left > 0 {
        let job: Option<(String, String, i64)> = x
            .conn
            .query_row("SELECT user_id, tbl, cursor FROM deletion_rewrites WHERE done_at IS NULL ORDER BY at, user_id, tbl LIMIT 1", [], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))
            .optional()?;
        let Some((user, tbl, mut cursor)) = job else { return Ok(()) };
        let t = if tbl == "item_questions" { "item_questions" } else { "ghost_questions" };
        while left > 0 {
            let range = format!("SELECT rowid FROM {t} WHERE rowid > ?2 ORDER BY rowid LIMIT {batch}");
            let tx = x.conn.unchecked_transaction()?;
            tx.execute(
                &format!(
                    "UPDATE {t} SET chain_json = (SELECT json_group_array(CASE WHEN l.value ->> '$.item_id' IN (SELECT id FROM items WHERE author_id = ?1)
                                                                       THEN json_set(l.value, '$.author_name', 'deleted teacher') ELSE json(l.value) END)
                                                FROM json_each({t}.chain_json) l)
                      WHERE rowid IN ({range}) AND chain_json <> '[]' AND EXISTS (SELECT 1 FROM json_each({t}.chain_json) l JOIN items x ON x.id = l.value ->> '$.item_id' WHERE x.author_id = ?1)"
                ),
                params![user, cursor],
            )?;
            let top: Option<i64> = tx.query_row(&format!("SELECT MAX(rowid) FROM ({range})"), params![user, cursor], |r| r.get(0))?;
            tx.commit()?;
            match top {
                None => {
                    x.conn.execute("UPDATE deletion_rewrites SET done_at = ?3 WHERE user_id = ?1 AND tbl = ?2", params![user, tbl, x.now])?;
                    left -= 1;
                    break;
                }
                Some(top) => {
                    cursor = top;
                    x.conn.execute("UPDATE deletion_rewrites SET cursor = ?3 WHERE user_id = ?1 AND tbl = ?2", params![user, tbl, top])?;
                }
            }
            left -= 1;
        }
    }
    Ok(())
}

#[cfg(test)]
#[path = "cronretain_tests.rs"]
mod tests;
