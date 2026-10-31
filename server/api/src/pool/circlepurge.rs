//! Circle purges, pruning and the account export (worker/src/pool/circles.ts "cron: abandoned sends, pruned versions, purges", "export",
//! `purgeBankNow`; moderation.ts `deleteKeys` and `gcBlobs`, which they and the later moderation port share). Stored objects live in the
//! `BlobStore`; a delete that fails stays in `r2_orphans` for the hourly retry.
use super::circlebanks::*;
use super::circlebase::*;
use super::circlesendsql::*;
use super::circlesql::*;
use super::ctx::{AuditEntry, PoolCtx, PoolError, Res, audit};
use crate::blobs::BlobStore;
use rusqlite::{OptionalExtension, params};
use serde_json::{Value, json};

const HOUR_MS: i64 = 3_600_000;
/// Pruned versions whose objects a daily run deletes at most.
pub const PRUNE_BATCH: usize = 500;
/// Banks and circles one daily purge run takes at most.
pub const CIRCLE_PURGE_BANKS: usize = 200;
pub const CIRCLE_PURGE_CIRCLES: usize = 50;

pub fn open_flag_bank(b: &str) -> String {
    format!("EXISTS (SELECT 1 FROM admin_flags pf WHERE pf.item_id = {b} AND {})", evidence_flag_sql("pf"))
}
/// An open report, a blocking hold or an unjudged flag keeps a bank until the admin has decided.
pub fn open_report_bank(b: &str) -> String {
    format!("(EXISTS (SELECT 1 FROM circle_reports r WHERE r.bank_id = {b} AND r.resolved_at IS NULL) OR {} OR {})", blocking_hold_sql(b), open_flag_bank(b))
}
pub fn open_report_circle(ci: &str) -> String {
    format!(
        "(EXISTS (SELECT 1 FROM circle_reports r WHERE r.circle_id = {ci} AND r.resolved_at IS NULL)
    OR EXISTS (SELECT 1 FROM circle_banks hb JOIN circle_holds hh ON hh.bank_id = hb.id WHERE hb.circle_id = {ci} AND hb.purged_at IS NULL AND hh.source NOT IN {SEND_HOLD_SOURCES})
    OR EXISTS (SELECT 1 FROM circle_banks fb WHERE fb.circle_id = {ci} AND fb.purged_at IS NULL AND {}))",
        open_flag_bank("fb.id")
    )
}

/// Deletes stored objects; those that cannot be deleted stay in `r2_orphans` (true when all went).
pub fn delete_keys(x: &PoolCtx, store: &BlobStore, keys: &[String]) -> rusqlite::Result<bool> {
    let mut all = true;
    for k in keys {
        if store.delete(k).is_err() {
            all = false;
        }
    }
    if !all {
        return Ok(false);
    }
    x.conn.execute("DELETE FROM r2_orphans WHERE key IN (SELECT value FROM json_each(?1))", [serde_json::to_string(keys).unwrap()])?;
    Ok(true)
}

/// Deletes the rows and stored bytes of images nothing uses any more (`only` limits it to those hashes); returns how many went.
pub fn gc_blobs(x: &PoolCtx, store: &BlobStore, only: Option<&[String]>) -> rusqlite::Result<usize> {
    let only_json = only.map(|o| serde_json::to_string(o).unwrap());
    let gone: Vec<String> = x
        .conn
        .prepare(
            "DELETE FROM blobs WHERE sha256 IN (
         SELECT b.sha256 FROM blobs b
          WHERE (?1 IS NULL OR b.sha256 IN (SELECT value FROM json_each(?1)))
            AND NOT EXISTS (SELECT 1 FROM item_blobs ib WHERE ib.sha256 = b.sha256)
            AND NOT EXISTS (SELECT 1 FROM circle_version_blobs cvb WHERE cvb.sha256 = b.sha256)
            AND NOT EXISTS (SELECT 1 FROM pending_blobs pb WHERE pb.sha256 = b.sha256 AND pb.until >= ?2)
          LIMIT 500)
       RETURNING sha256",
        )?
        .query_map(params![only_json, x.now - HOUR_MS], |r| r.get(0))?
        .collect::<Result<_, _>>()?;
    if gone.is_empty() {
        return Ok(0);
    }
    // An upload that landed in between wrote a live pending row (and re-inserts the blobs row): keep those bytes.
    let back: Vec<String> = x
        .conn
        .prepare("SELECT sha256 FROM pending_blobs WHERE sha256 IN (SELECT value FROM json_each(?1)) AND until >= ?2 UNION SELECT sha256 FROM blobs WHERE sha256 IN (SELECT value FROM json_each(?1))")?
        .query_map(params![serde_json::to_string(&gone).unwrap(), x.now - HOUR_MS], |r| r.get(0))?
        .collect::<Result<_, _>>()?;
    let drop: Vec<&String> = gone.iter().filter(|s| !back.contains(s)).collect();
    for s in &drop {
        let _ = store.delete(&format!("blobs/{s}"));
    }
    Ok(drop.len())
}

/// Hourly: a send whose staging row is older than an hour never committed: its row and objects go.
pub fn discard_circle_staging(x: &PoolCtx, store: &BlobStore) -> rusqlite::Result<usize> {
    let stale: Vec<(String, String, String, i64)> = x
        .conn
        .prepare("SELECT id, full_key, index_key, created_at FROM circle_versions WHERE status = 'staging' AND created_at < ?1 ORDER BY created_at LIMIT 200")?
        .query_map([x.now - HOUR_MS], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)))?
        .collect::<Result<_, _>>()?;
    let mut n = 0;
    for (id, full, index, created) in stale {
        let keys: Vec<String> = [full, index].into_iter().filter(|k| !k.is_empty()).collect();
        let tx = x.conn.unchecked_transaction()?;
        let del = tx.execute("DELETE FROM circle_versions WHERE id = ?1 AND status = 'staging' AND created_at = ?2", params![id, created])?;
        if del == 1 {
            tx.execute("INSERT OR IGNORE INTO r2_orphans (key, at) SELECT value, ?2 FROM json_each(?1)", params![serde_json::to_string(&keys).unwrap(), x.now])?;
        }
        tx.commit()?;
        if del != 1 {
            continue;
        }
        delete_keys(x, store, &keys)?;
        n += 1;
    }
    Ok(n)
}

/// Daily: the bodies and indexes of versions past the kept ones go; their history rows stay.
pub fn prune_circle_versions(x: &PoolCtx, store: &BlobStore) -> rusqlite::Result<usize> {
    let rows: Vec<(String, String, String)> = x
        .conn
        .prepare(&format!("SELECT id, full_key, index_key FROM circle_versions INDEXED BY circle_versions_prune WHERE status = 'pruned' AND full_key <> '' ORDER BY pruned_at LIMIT {PRUNE_BATCH}"))?
        .query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))?
        .collect::<Result<_, _>>()?;
    if rows.is_empty() {
        return Ok(0);
    }
    let ids = serde_json::to_string(&rows.iter().map(|r| &r.0).collect::<Vec<_>>()).unwrap();
    let keys: Vec<String> = rows.iter().flat_map(|r| [r.1.clone(), r.2.clone()]).filter(|k| !k.is_empty()).collect();
    let tx = x.conn.unchecked_transaction()?;
    tx.execute("INSERT OR IGNORE INTO r2_orphans (key, at) SELECT value, ?2 FROM json_each(?1)", params![serde_json::to_string(&keys).unwrap(), x.now])?;
    tx.execute("DELETE FROM circle_version_blobs WHERE version_id IN (SELECT value FROM json_each(?1))", [&ids])?;
    tx.execute("UPDATE circle_versions SET full_key = '', index_key = '' WHERE id IN (SELECT value FROM json_each(?1)) AND status = 'pruned'", [&ids])?;
    tx.commit()?;
    delete_keys(x, store, &keys)?;
    Ok(rows.len())
}

/// Purges one bank: stored bodies and indexes, versions, their images' rows, its qkeys, holds; the row stays as a tombstone.
pub fn purge_bank(x: &PoolCtx, store: &BlobStore, bid: &str, cid: &str, why: &str, actor: Option<&str>) -> rusqlite::Result<()> {
    let keys: Vec<String> = x
        .conn
        .prepare("SELECT full_key, index_key FROM circle_versions WHERE bank_id = ?1")?
        .query_map([bid], |r| Ok([r.get::<_, String>(0)?, r.get::<_, String>(1)?]))?
        .collect::<Result<Vec<_>, _>>()?
        .into_iter()
        .flatten()
        .filter(|k| !k.is_empty())
        .collect();
    let shas: Vec<String> = x
        .conn
        .prepare("SELECT DISTINCT vb.sha256 FROM circle_version_blobs vb WHERE vb.version_id IN (SELECT id FROM circle_versions WHERE bank_id = ?1)")?
        .query_map([bid], |r| r.get(0))?
        .collect::<Result<_, _>>()?;
    let tx = x.conn.unchecked_transaction()?;
    tx.execute("INSERT OR IGNORE INTO r2_orphans (key, at) SELECT value, ?2 FROM json_each(?1)", params![serde_json::to_string(&keys).unwrap(), x.now])?;
    tx.execute("DELETE FROM circle_version_blobs WHERE version_id IN (SELECT id FROM circle_versions WHERE bank_id = ?1)", [bid])?;
    tx.execute("DELETE FROM circle_versions WHERE bank_id = ?1", [bid])?;
    let q = tx.execute("DELETE FROM circle_qkeys WHERE bank_id = ?1", [bid])?;
    tx.execute("UPDATE circles SET qkeys = MAX(0, qkeys - ?2) WHERE id = ?1", params![cid, q as i64])?;
    tx.execute("DELETE FROM circle_holds WHERE bank_id = ?1", [bid])?;
    // The tombstone keeps no typed text.
    tx.execute(
        "UPDATE circle_banks SET purged_at = COALESCE(purged_at, ?2), title = '', subject = '', course = '', grade = '', head_version_id = NULL, purge_after = NULL, removed_at = COALESCE(removed_at, ?2), removed_reason = COALESCE(removed_reason, 'circle_closed') WHERE id = ?1",
        params![bid, x.now],
    )?;
    audit(&tx, &AuditEntry { actor_id: actor, actor_kind: if actor.is_some() { "user" } else { "system" }, action: "circle.bank_purge", item_id: Some(cid), target_user_id: actor, detail: Some(json!({ "bank_id": bid, "why": why })), ..Default::default() }, x.now, None)?;
    tx.commit()?;
    if !keys.is_empty() {
        delete_keys(x, store, &keys)?;
    }
    if !shas.is_empty() {
        gc_blobs(x, store, Some(&shas))?;
    }
    Ok(())
}

/// Daily: removed banks and closed circles past purge_after (never one with an open report, a hold or an unjudged flag), in bounded batches.
pub fn purge_circles(x: &PoolCtx, store: &BlobStore) -> rusqlite::Result<(usize, usize)> {
    x.conn.execute("DELETE FROM circle_transfers WHERE expires_at <= ?1", [x.now])?;
    let due: Vec<(String, String)> = x
        .conn
        .prepare(&format!(
            "SELECT b.id, b.circle_id FROM circle_banks b WHERE b.purge_after IS NOT NULL AND b.purge_after <= ?1 AND b.purged_at IS NULL AND NOT {} ORDER BY b.purge_after LIMIT {CIRCLE_PURGE_BANKS}",
            open_report_bank("b.id")
        ))?
        .query_map([x.now], |r| Ok((r.get(0)?, r.get(1)?)))?
        .collect::<Result<_, _>>()?;
    for (bid, cid) in &due {
        purge_bank(x, store, bid, cid, "purge_after", None)?;
    }
    let circles: Vec<String> = x
        .conn
        .prepare(&format!(
            "SELECT ci.id FROM circles ci WHERE ci.purge_after IS NOT NULL AND ci.purge_after <= ?1 AND ci.purged_at IS NULL AND NOT {} ORDER BY ci.purge_after LIMIT {CIRCLE_PURGE_CIRCLES}",
            open_report_circle("ci.id")
        ))?
        .query_map([x.now], |r| r.get(0))?
        .collect::<Result<_, _>>()?;
    let mut n = 0;
    for ci in &circles {
        // A reopen that won meanwhile: the circle is open again and stays.
        let still: Option<i64> = x.conn.query_row("SELECT 1 FROM circles WHERE id = ?1 AND closed_at IS NOT NULL AND purge_after <= ?2 AND purged_at IS NULL", params![ci, x.now], |r| r.get(0)).optional()?;
        if still.is_none() {
            continue;
        }
        let banks: Vec<String> = x.conn.prepare("SELECT id FROM circle_banks WHERE circle_id = ?1 AND purged_at IS NULL")?.query_map([ci], |r| r.get(0))?.collect::<Result<_, _>>()?;
        for b in &banks {
            purge_bank(x, store, b, ci, "circle_closed", None)?;
        }
        let tx = x.conn.unchecked_transaction()?;
        tx.execute("DELETE FROM circle_invites WHERE circle_id = ?1", [ci])?;
        tx.execute("DELETE FROM circle_transfers WHERE circle_id = ?1", [ci])?;
        tx.execute("DELETE FROM circle_members WHERE circle_id = ?1", [ci])?;
        tx.execute("UPDATE circles SET purged_at = ?2, name = NULL, description = NULL, purge_after = NULL, undo_until = NULL, qkeys = 0 WHERE id = ?1 AND closed_at IS NOT NULL AND purged_at IS NULL", params![ci, x.now])?;
        audit(&tx, &AuditEntry { actor_kind: "system", action: "circle.purge", item_id: Some(ci), ..Default::default() }, x.now, None)?;
        tx.commit()?;
        n += 1;
    }
    Ok((due.len(), n))
}

/// POST /circles/:cid/banks/:bid/purge (the owner): a REMOVED bank is purged now instead of after its 14 days. Never a bank under an admin
/// removal, a hold or an open report. Not undoable. (A bank whose only obstacle is an open flag waits for `purgeWhenReviewed`.)
pub fn purge_bank_now(x: &PoolCtx, store: &BlobStore, me: &str, cid: &str, bid: &str) -> Res<Value> {
    let (m, b) = bank_or_miss(x, cid, bid, me)?;
    require_role(&m, &["owner"])?;
    if b.removed_at.is_none() {
        return Err(PoolError::new(409, "bank_not_removed").with("message", "Remove the bank first.".into()).into());
    }
    if matches!(b.removed_reason.as_deref(), Some("admin") | Some("privacy") | Some("takedown")) {
        return Err(PoolError::new(409, "under_moderation").into());
    }
    // The claim (guarded: the owner, alive; no hold, no open report): purge_after now, so an undelete racing it fails.
    let claim = x.conn.execute(
        &format!(
            "UPDATE circle_banks SET purge_after = ?4 WHERE id = ?1 AND circle_id = ?2 AND removed_at IS NOT NULL AND purged_at IS NULL
         AND removed_reason IN ('author_removed', 'owner_removed', 'account_deleted', 'circle_closed') AND NOT {}
         AND {} AND {}",
            open_report_bank("circle_banks.id"),
            active_sql("?2", "?3", &["owner"]),
            super::sqlfrag::alive_sql("?3")
        ),
        params![bid, cid, me, x.now],
    )?;
    if claim != 1 {
        x.throw_if_erased(me)?;
        // A report or an admin's hold is moderation; an open student-data flag alone is a review to wait for.
        let (moderated, flagged): (i64, i64) = x.conn.query_row(
            &format!("SELECT (EXISTS (SELECT 1 FROM circle_reports r WHERE r.bank_id = ?1 AND r.resolved_at IS NULL) OR {}) AS moderated, {} AS flagged", blocking_hold_sql("?1"), open_flag_bank("?1")),
            [bid],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )?;
        if moderated == 0 && flagged == 1 {
            return purge_when_reviewed(x, me, cid, bid);
        }
        return Err(PoolError::new(409, "under_moderation").with("message", "This bank is under review by an administrator.".into()).into());
    }
    purge_bank(x, store, bid, cid, "owner_purge", Some(me))?;
    Ok(json!({ "ok": true }))
}

/// Purge-now of a removed bank whose only obstacle is an open flag (an administrator's review of a withheld version): the purge is
/// claimed (the bank can no longer be restored; the daily purge takes it once the review is done) and every version but the evidence
/// kept for the administrator is pruned now, so its bytes stop counting at once. Answered as done: `purged_partial`.
fn purge_when_reviewed(x: &PoolCtx, me: &str, cid: &str, bid: &str) -> Res<Value> {
    let tx = x.conn.unchecked_transaction()?;
    let claim = tx.execute(
        &format!(
            "UPDATE circle_banks SET purge_after = ?4 WHERE id = ?1 AND circle_id = ?2 AND removed_at IS NOT NULL AND purged_at IS NULL
           AND removed_reason IN ('author_removed', 'owner_removed', 'account_deleted', 'circle_closed')
           AND NOT EXISTS (SELECT 1 FROM circle_reports r WHERE r.bank_id = ?1 AND r.resolved_at IS NULL) AND NOT {}
           AND {} AND {}",
            blocking_hold_sql("?1"),
            active_sql("?2", "?3", &["owner"]),
            super::sqlfrag::alive_sql("?3")
        ),
        params![bid, cid, me, x.now],
    )?;
    if claim == 1 {
        tx.execute(&format!("UPDATE circle_versions SET status = 'pruned', pruned_at = ?2 WHERE bank_id = ?1 AND status IN ('current', 'previous') AND NOT {}", pinned_evidence_sql("circle_versions")), params![bid, x.now])?;
        tx.execute(&format!("DELETE FROM circle_version_blobs WHERE version_id IN ({PRUNED_NOW_SQL})"), params![bid, x.now])?;
        audit(&tx, &AuditEntry { actor_id: Some(me), actor_kind: "user", action: "circle.bank_purge_requested", item_id: Some(cid), target_user_id: Some(me), detail: Some(json!({ "bank_id": bid })), ..Default::default() }, x.now, None)?;
    }
    let n: i64 = tx.query_row(
        &format!("SELECT (SELECT COUNT(*) FROM circle_versions ev INDEXED BY circle_versions_bank WHERE ev.bank_id = ?1 AND ev.status IN ('current', 'previous') AND ev.full_key <> '' AND {})", pinned_evidence_sql("ev")),
        [bid],
        |r| r.get(0),
    )?;
    tx.commit()?;
    if claim != 1 {
        x.throw_if_erased(me)?;
        return Err(PoolError::new(409, "under_moderation").with("message", "This bank is under review by an administrator.".into()).into());
    }
    let what = if n == 1 { "a version".to_string() } else { format!("{n} versions") };
    Ok(json!({
        "ok": true, "purged_partial": true, "pending_review": true, "evidence_kept": n,
        "message": format!("An administrator is reviewing {what} of this bank. Everything else in it has been deleted now and the bank cannot be restored; the rest is deleted once that review is done.")
    }))
}

/// The account's circles section of GET /me/export: memberships, circles owned, codes created (no codes), versions sent (no bodies), reports filed.
pub fn export_circles(x: &PoolCtx, me: &str) -> rusqlite::Result<Value> {
    let q = |sql: &str, with_now: bool| -> rusqlite::Result<Vec<Value>> {
        let mut st = x.conn.prepare(sql)?;
        let cols: Vec<String> = st.column_names().iter().map(|s| s.to_string()).collect();
        let map = |r: &rusqlite::Row| -> rusqlite::Result<Value> {
            let mut o = serde_json::Map::new();
            for (i, c) in cols.iter().enumerate() {
                let v: rusqlite::types::Value = r.get(i)?;
                o.insert(
                    c.clone(),
                    match v {
                        rusqlite::types::Value::Null => Value::Null,
                        rusqlite::types::Value::Integer(n) => n.into(),
                        rusqlite::types::Value::Real(f) => json!(f),
                        rusqlite::types::Value::Text(s) => s.into(),
                        rusqlite::types::Value::Blob(_) => Value::Null,
                    },
                );
            }
            Ok(Value::Object(o))
        };
        let rows = if with_now { st.query_map(params![me, x.now], map)?.collect::<Result<_, _>>()? } else { st.query_map([me], map)?.collect::<Result<_, _>>()? };
        Ok(rows)
    };
    Ok(json!({
        "memberships": q("SELECT m.circle_id, COALESCE(ci.name, '') AS circle_name, m.role, m.joined_at, m.left_at, m.left_reason FROM circle_members m JOIN circles ci ON ci.id = m.circle_id WHERE m.user_id = ?1 ORDER BY m.joined_at", false)?,
        "owned": q("SELECT m.circle_id, COALESCE(ci.name, '') AS circle_name, ci.created_at, ci.closed_at FROM circle_members m JOIN circles ci ON ci.id = m.circle_id WHERE m.user_id = ?1 AND m.role = 'owner' AND m.left_at IS NULL ORDER BY ci.created_at", false)?,
        "invites_created": q("SELECT id AS invite_id, circle_id, kind, COALESCE(email_label, '') AS address_label, role, created_at, max_uses, uses, expires_at, revoked_at, COALESCE(note, '') AS note FROM circle_invites WHERE created_by = ?1 ORDER BY created_at", false)?,
        "versions_sent": q("SELECT circle_id, bank_id, version, committed_at, COALESCE(message, '') AS message, license FROM circle_versions WHERE committer_id = ?1 AND status <> 'staging' ORDER BY committed_at", false)?,
        "banks_authored": q("SELECT circle_id, id AS bank_id, title, license, head_version, created_at, removed_at FROM circle_banks WHERE author_id = ?1 AND purged_at IS NULL ORDER BY created_at", false)?,
        "reports": q("SELECT circle_id, bank_id, reason, COALESCE(note, '') AS note, at, resolved_at, action FROM circle_reports WHERE reporter_id = ?1 ORDER BY at", false)?,
        "transfer_offers": q("SELECT circle_id, from_user = ?1 AS offered_by_me, to_user = ?1 AS offered_to_me, created_at, expires_at FROM circle_transfers WHERE (from_user = ?1 OR to_user = ?1) AND expires_at > ?2 ORDER BY created_at", true)?,
    }))
}

#[cfg(test)]
#[path = "circlepurge_tests.rs"]
mod tests;
