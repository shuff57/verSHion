//! The pool's scheduled work, first part (worker/src/pool/cron.ts): the stage runner and the stages whose pieces exist in this crate. Ages, not states, guard
//! in-flight work. Each stage is isolated: a failure is logged, flagged (`cron_failure`, once per stage and hour) and mailed within the alert budget, and
//! the other stages still run. The runner (timers) calls `hourly`; `daily_early` is the first half of the daily run.
use super::circlepurge::discard_circle_staging;
use super::core::{WASM_TRAPS_PER_HOUR, take_wasm_traps};
use super::ctx::{Fail, PoolCtx, PoolNotice, Res};
use super::modgc::{gc_blobs, repeat_infringer_sweep, retry_orphans};
use super::modlineage::finish_quarantines;
use super::modprivacy::privacy_finish;
use super::modpurge::{Actor, ghost_upholds};
use super::modrestore::{putback_finish, release_restored_holds, restore};
use super::mine::QUARANTINE_MS;
use super::offer::offer_sync;
use super::poolio::flush_outbox;
use super::publishdel::delete_publishes;
use super::sqlfrag::{DAY_MS, HOUR_MS, new_id};
use crate::blobs::BlobStore;
use rusqlite::params;
use serde_json::{Value, json};

fn io(e: std::io::Error) -> rusqlite::Error {
    rusqlite::Error::ToSqlConversionFailure(Box::new(e))
}

pub const RECONCILE_BATCH: i64 = 200;
pub const OFFER_SYNC_ITEMS: i64 = 200;
/// Closed licence rows no serve can reach are collected, this many a run.
pub const CL_GC_ROWS: i64 = 5000;
/// Rows past which the admin is told (the table is bounded by contents x licences, so this means growth).
pub const CL_ALERT_ROWS: i64 = 5_000_000;

/// One cron stage: a failure is logged, flagged and mailed within the alert budget, and the other stages still run. Returns `fallback` on failure.
pub fn stage<T>(x: &PoolCtx, name: &str, fallback: T, f: impl FnOnce() -> Res<T>) -> T {
    match f() {
        Ok(v) => v,
        Err(e) => {
            let msg = match &e {
                Fail::Pool(p) => format!("{} {}", p.status, p.code),
                other => format!("{other:?}"),
            };
            eprintln!("pool cron: stage {name} failed: {msg}");
            // (the database itself may be failing: the log line above is then all we can do)
            let _ = (|| -> Res<()> {
                let ins = x.conn.execute(
                    "INSERT OR IGNORE INTO admin_flags (id, kind, detail, dedup, at) VALUES (?1, 'cron_failure', ?2, ?3, ?4)",
                    params![new_id(), json!({ "stage": name }).to_string(), format!("cron:{name}:{}", x.now / HOUR_MS), x.now],
                )?;
                if ins == 1 {
                    x.alert_budgeted(PoolNotice::CronFailure { stage: name.to_string() })?;
                }
                Ok(())
            })();
            fallback
        }
    }
}

#[derive(Debug, Default, PartialEq)]
pub struct HourlyReport {
    pub staged: usize,
    pub pending: usize,
    pub blobs: usize,
    pub missing: usize,
}

/// Hourly: abandoned staged publishes, expired pending uploads, unreferenced blobs, the reconciler, circle staging, orphan retries, the notice outbox, large
/// privacy purges' remaining holds, quarantines, the licence reconcile and the trap check. (The backfills, deletion rewrites and the size check follow
/// with the code they depend on.)
pub fn hourly(x: &PoolCtx, store: &BlobStore, notify: &mut dyn FnMut(&PoolNotice) -> Result<(), String>) -> HourlyReport {
    let now = x.now;
    let staged = stage(x, "staged", 0, || discard_abandoned(x, store));
    let pending = stage(x, "pending", 0, || Ok(x.conn.execute("DELETE FROM pending_blobs WHERE until < ?1", [now - HOUR_MS])?));
    let blobs = stage(x, "gc", 0, || gc_blobs(x, store, None));
    let missing = stage(x, "reconcile", 0, || reconcile(x, store));
    stage(x, "circle_staged", 0, || Ok(discard_circle_staging(x, store)?));
    stage(x, "r2_orphans", 0, || retry_orphans(x, store));
    stage(x, "outbox", 0, || flush_outbox(x.conn, now, None, notify));
    stage(x, "privacy_backlog", (), || privacy_backlog(x, store));
    stage(x, "quarantine", 0, || finish_quarantines(x, store));
    stage(x, "offer_sync", 0, || offer_reconcile(x));
    stage(x, "wasm_traps", (), || wasm_trap_check(x));
    HourlyReport { staged, pending, blobs, missing }
}

/// A privacy purge's holds that one request could not place: continued every hour, never waiting a day.
pub fn privacy_backlog(x: &PoolCtx, store: &BlobStore) -> Res<()> {
    let backlog: Vec<(String, String)> = x
        .conn
        .prepare("SELECT item_id, detail FROM admin_flags WHERE kind = 'privacy_backlog' AND resolved_at IS NULL LIMIT 20")?
        .query_map([], |r| Ok((r.get(0)?, r.get(1)?)))?
        .collect::<Result<_, _>>()?;
    for (item_id, detail) in backlog {
        let d: Value = serde_json::from_str(&detail).unwrap_or(Value::Null);
        let list = |k: &str| -> Vec<String> { d[k].as_array().map(|a| a.iter().filter_map(|v| v.as_str().map(str::to_string)).collect()).unwrap_or_default() };
        // The copies, then the bodies' move, then resolved.
        privacy_finish(x, store, &item_id, &list("match"), &list("blob"), None, 20)?;
    }
    Ok(())
}

/// Staged publishes older than an hour never committed: their rows, new qids and bodies go.
pub fn discard_abandoned(x: &PoolCtx, store: &BlobStore) -> Res<usize> {
    let stale: Vec<(String, String, String, Option<String>, i64)> = x
        .conn
        .prepare("SELECT publish_id, item_id, full_key, stripped_key, created_at FROM item_publishes WHERE status IN ('staging', 'staged') AND created_at < ?1 LIMIT 200")?
        .query_map([x.now - HOUR_MS], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?)))?
        .collect::<Result<_, _>>()?;
    let mut staged = 0;
    for (publish, item, full, stripped, created) in stale {
        if discard_staged(x, store, &publish, &item, &full, stripped.as_deref(), created)? {
            staged += 1;
        }
    }
    Ok(staged)
}

/// Removes a staged publish that will never commit: its rows, its new qids and its bodies. False when another discard (or a commit) got there first.
pub fn discard_staged(x: &PoolCtx, store: &BlobStore, publish: &str, item: &str, full_key: &str, stripped_key: Option<&str>, created_at: i64) -> Res<bool> {
    // Claiming the row first (only a never-committed one) makes two concurrent discards safe.
    let del = x.conn.execute("DELETE FROM item_publishes WHERE publish_id = ?1 AND status IN ('staging', 'staged') AND created_at = ?2", params![publish, created_at])?;
    if del != 1 {
        return Ok(false);
    }
    let tx = x.conn.unchecked_transaction()?;
    delete_publishes(&tx, &[publish.to_string()])?;
    // qids first introduced by a publish that never committed belong to nothing.
    tx.execute("DELETE FROM question_ids WHERE first_publish_id = ?1 AND item_id = ?2 AND NOT EXISTS (SELECT 1 FROM item_questions iq WHERE iq.qid = question_ids.qid)", params![publish, item])?;
    tx.commit()?;
    for k in std::iter::once(full_key).chain(stripped_key) {
        store.delete(k).map_err(io)?;
    }
    Ok(true)
}

/// A blob row whose bytes are gone (a GC that raced an upload) is marked missing, so the next publish names the positions and the author uploads again;
/// found again, it is cleared. Images a privacy purge removed are gone on purpose and not checked.
pub fn reconcile(x: &PoolCtx, store: &BlobStore) -> Res<usize> {
    let check: Vec<String> = x
        .conn
        .prepare("SELECT sha256 FROM blobs WHERE privacy_purged_at IS NULL ORDER BY COALESCE(checked_at, 0) LIMIT ?1")?
        .query_map([RECONCILE_BATCH], |r| r.get(0))?
        .collect::<Result<_, _>>()?;
    let mut missing = 0;
    for sha in check {
        let there = store.len(&format!("blobs/{sha}")).map_err(io)?.is_some();
        if !there {
            missing += 1;
        }
        x.conn.execute(
            "UPDATE blobs SET checked_at = ?2, missing_at = CASE WHEN ?3 = 1 THEN NULL ELSE COALESCE(missing_at, ?2) END WHERE sha256 = ?1",
            params![sha, x.now, there as i64],
        )?;
    }
    Ok(missing)
}

/// Items whose open licence rows disagree with what they offer now (bounded per run), reconciled.
pub fn offer_reconcile(x: &PoolCtx) -> Res<usize> {
    let ids: Vec<String> = x
        .conn
        .prepare(&format!(
            "SELECT DISTINCT item_id FROM (
               SELECT cl.item_id FROM content_licences cl JOIN items i ON i.id = cl.item_id WHERE cl.until_at IS NULL
                  AND NOT (i.visibility = 'public' AND i.removed_at IS NULL AND i.purged_at IS NULL AND NOT EXISTS (SELECT 1 FROM item_holds oh WHERE oh.item_id = i.id))
               UNION ALL
               SELECT o.item_id FROM item_offer o JOIN items i ON i.id = o.item_id
                WHERE i.visibility = 'public' AND i.removed_at IS NULL AND i.purged_at IS NULL AND NOT EXISTS (SELECT 1 FROM item_holds oh WHERE oh.item_id = i.id)
                  AND NOT EXISTS (SELECT 1 FROM content_licences cl WHERE cl.item_id = o.item_id AND cl.ckey = o.ckey AND cl.license = i.license AND cl.until_at IS NULL))
             LIMIT {OFFER_SYNC_ITEMS}"
        ))?
        .query_map([], |r| r.get(0))?
        .collect::<Result<_, _>>()?;
    for id in &ids {
        offer_sync(x.conn, id, x.now, None)?;
    }
    Ok(ids.len())
}

/// Closed licence rows no serve can reach (every serve of the item is after it closed, or none): collected; the admin is told when the table grows huge.
pub fn content_licences_gc(x: &PoolCtx) -> Res<usize> {
    let del = x.conn.execute(
        &format!(
            "DELETE FROM content_licences WHERE rowid IN (SELECT cl.rowid FROM content_licences cl WHERE cl.until_at IS NOT NULL
               AND NOT EXISTS (SELECT 1 FROM served s WHERE s.item_id = cl.item_id AND s.first_at <= cl.until_at) LIMIT {CL_GC_ROWS})"
        ),
        [],
    )?;
    let n: i64 = x.conn.query_row(&format!("SELECT COUNT(*) FROM (SELECT 1 FROM content_licences LIMIT {})", CL_ALERT_ROWS + 1), [], |r| r.get(0))?;
    if n > CL_ALERT_ROWS {
        x.conn.execute(
            "INSERT OR IGNORE INTO admin_flags (id, kind, detail, dedup, at) VALUES (?1, 'd1_size', ?2, ?3, ?4)",
            params![new_id(), json!({ "table": "content_licences", "rows": n }).to_string(), format!("content_licences:{}", x.now / DAY_MS), x.now],
        )?;
    }
    Ok(del)
}

/// Share-core traps counted in this process, added to the hour's count in pool_kv.
pub fn flush_wasm_traps(x: &PoolCtx) -> Res<()> {
    let n = take_wasm_traps();
    if n == 0 {
        return Ok(());
    }
    x.conn.execute(
        "INSERT INTO pool_kv (key, value) VALUES (?1, ?2) ON CONFLICT (key) DO UPDATE SET value = CAST(CAST(value AS INTEGER) + ?3 AS TEXT)",
        params![format!("wasm_traps:{}", x.now / HOUR_MS), n.to_string(), n as i64],
    )?;
    Ok(())
}

/// Past WASM_TRAPS_PER_HOUR traps in the last hour, the admin is flagged and told.
pub fn wasm_trap_check(x: &PoolCtx) -> Res<()> {
    flush_wasm_traps(x)?;
    let hour = x.now / HOUR_MS;
    for h in [hour - 1, hour] {
        let v: Option<String> = x.conn.query_row("SELECT value FROM pool_kv WHERE key = ?1", [format!("wasm_traps:{h}")], |r| r.get(0)).ok();
        let n: i64 = v.and_then(|s| s.parse().ok()).unwrap_or(0);
        if n <= WASM_TRAPS_PER_HOUR as i64 {
            continue;
        }
        let f = x.conn.execute("INSERT OR IGNORE INTO admin_flags (id, kind, detail, dedup, at) VALUES (?1, 'wasm_trap', ?2, ?3, ?4)", params![new_id(), json!({ "hour": h, "traps": n }).to_string(), format!("wasm_trap:{h}"), x.now])?;
        if f == 1 {
            x.alert_budgeted(PoolNotice::WasmTrap { traps: n })?;
        }
    }
    x.conn.execute("DELETE FROM pool_kv WHERE key LIKE 'wasm_traps:%' AND CAST(substr(key, 12) AS INTEGER) < ?1", [hour - 24])?;
    Ok(())
}

#[derive(Debug, Default, PartialEq)]
pub struct DailyEarly {
    pub restored: usize,
    pub quarantine_expired: usize,
}

/// The first half of the daily run: DMCA put-backs, ghost upholds, restored holds, the licence history, interrupted put-backs, the repeat-infringer sweep
/// and the quarantine expiry. (The purge, the erasure sweeps and the retention follow with the code they depend on.)
pub fn daily_early(x: &PoolCtx, store: &BlobStore) -> DailyEarly {
    let now = x.now;
    // A complete counter-notice and no court action: restored no sooner than 10 business days after receipt.
    let restored = stage(x, "putbacks", 0, || {
        let due: Vec<String> = x
            .conn
            .prepare("SELECT id FROM dmca_actions WHERE counter_notice_id IS NOT NULL AND restore_not_before <= ?1 AND court_action_at IS NULL AND restored_at IS NULL AND purged_at IS NULL LIMIT 100")?
            .query_map([now], |r| r.get(0))?
            .collect::<Result<_, _>>()?;
        let mut n = 0;
        for a in due {
            if restore(x, store, &a, &Actor { id: None, kind: "system" }, "put_back")? {
                n += 1;
            }
        }
        Ok(n)
    });
    // Ghost takedowns whose window passed are upheld.
    stage(x, "ghost_upholds", 0, || ghost_upholds(x));
    stage(x, "restored_holds", 0, || release_restored_holds(x));
    stage(x, "offer_sync", 0, || offer_reconcile(x));
    stage(x, "content_licences_gc", 0, || content_licences_gc(x));
    // (put-backs that stopped after their claim)
    stage(x, "putback_finish", 0, || putback_finish(x, store));
    // (a repeat-infringer check that failed, redone from the strikes)
    stage(x, "repeat_infringers", 0, || repeat_infringer_sweep(x));
    // No counter-notice within 30 days of notifying the author: purge.
    let quarantine_expired = stage(x, "quarantine_expiry", 0, || {
        Ok(x.conn.execute(
            "UPDATE items SET purge_after = ?1 WHERE removed_reason = 'takedown' AND purged_at IS NULL AND purge_after IS NULL
               AND id IN (SELECT item_id FROM dmca_actions WHERE counter_notice_id IS NULL AND court_action_at IS NULL AND restored_at IS NULL AND purged_at IS NULL AND author_notified_at <= ?2)",
            params![now, now - QUARANTINE_MS],
        )?)
    });
    DailyEarly { restored, quarantine_expired }
}

#[cfg(test)]
#[path = "croncore_tests.rs"]
mod tests;
