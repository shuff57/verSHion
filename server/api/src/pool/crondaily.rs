//! The pool's scheduled work, third part (worker/src/pool/cron.ts): the rest of the daily run and its orchestrator. Each stage is isolated by `stage`.
use super::blocking::PREVIEW_MS;
use super::circleerase::{circle_resweep, ownerless_circles};
use super::circlepurge::{prune_circle_versions, purge_circles};
use super::croncore::{DailyEarly, daily_early, privacy_backlog, stage};
use super::cronretain::{REWRITE_BATCH, REWRITE_BATCHES, deletion_rewrites, privacy_bytes, retention};
use super::ctx::{PoolCtx, Res};
use super::modpurge::{Actor, purge_item};
use super::modprivacy::appeals_due;
use super::moderation::open_appeal_sql;
use super::poolcontent::delete_content;
use super::sqlfrag::{DAY_MS, HOUR_MS, new_id};
use crate::auth::crypto::uuid_v4;
use crate::blobs::BlobStore;
use rusqlite::params;
use serde_json::json;
use std::time::{Duration, Instant};

/// The daily purge works through due items in batches of 200 until done or this budget is spent.
pub const PURGE_BATCHES_MAX: usize = 50;
pub const PURGE_TIME_BUDGET: Duration = Duration::from_secs(20);
/// Ghost rows (units, questions, publishes: each) the daily GC deletes per run.
pub const GHOST_GC_ROWS: i64 = 20_000;

#[derive(Debug, Default, PartialEq)]
pub struct DailyReport {
    pub purged: usize,
    pub restored: usize,
    pub quarantine_expired: usize,
}

/// An account whose erasure batch failed (deleted, no erasure_done row): the whole batch again (each step idempotent), until it commits.
pub fn erasure_finish(x: &PoolCtx) -> Res<usize> {
    let due: Vec<String> = x.conn.prepare("SELECT id FROM users WHERE deleted_at IS NOT NULL AND id NOT IN (SELECT user_id FROM erasure_done) LIMIT 20")?.query_map([], |r| r.get(0))?.collect::<Result<_, _>>()?;
    for u in &due {
        let tx = x.conn.unchecked_transaction()?;
        delete_content(&tx, u, x.now)?;
        tx.commit()?;
    }
    Ok(due.len())
}

/// Defence in depth. Every write a user request or job makes under an account id is guarded IN the write (nothing is written for an erased account), but a
/// request that was already past that guard when the erasure batch ran is the one race a guard cannot see; so every table such a writer touches is cleaned
/// again daily for erased accounts. Bounded (the imports re-key takes at most 50 accounts a run) and idempotent. Never touched: kept anonymised items, the
/// users tombstone, erasure_done, audit rows without notes, retired_publish_ids, attestations (the documented residue).
pub fn erasure_resweep(x: &PoolCtx) -> Res<()> {
    let done = "SELECT user_id FROM erasure_done";
    let now = x.now;
    // The importer's per-question records are re-keyed to one anonymous key per account, as the erasure batch does.
    let stragglers: Vec<String> = x.conn.prepare(&format!("SELECT DISTINCT user_id FROM imports WHERE user_id IN ({done}) LIMIT 50"))?.query_map([], |r| r.get(0))?.collect::<Result<_, _>>()?;
    // (open circles left without an owner, and owner-closed circles inside their undo window: the circle part's safety net)
    let ownerless = ownerless_circles(x.conn, now)?;
    let tx = x.conn.unchecked_transaction()?;
    for u in &stragglers {
        tx.execute("UPDATE imports SET user_id = ?2 WHERE user_id = ?1", params![u, format!("deleted:{}", uuid_v4().replace('-', ""))])?;
    }
    let stmts: Vec<(String, bool)> = vec![
        // (the key_leak flag's quoted text goes before the reports lose their reporter)
        (format!("UPDATE admin_flags SET detail = json_remove(detail, '$.served') WHERE kind = 'key_leak' AND json_valid(detail) AND json_type(detail, '$.served') IS NOT NULL AND detail ->> '$.report_id' IN (SELECT id FROM reports WHERE reporter_id IN ({done}))"), false),
        (format!("UPDATE reports SET note = NULL WHERE reporter_id IN ({done}) AND note IS NOT NULL"), false),
        (format!("UPDATE reports SET reporter_id = NULL WHERE reporter_id IN ({done})"), false),
        (format!("UPDATE admin_flags SET detail = json_remove(detail, '$.statement', '$.note') WHERE user_id IN ({done}) AND json_valid(detail) AND (json_type(detail, '$.note') IS NOT NULL OR json_type(detail, '$.statement') IS NOT NULL)"), false),
        // an open appeal of an erased account (or on its removed items) closes, so the 14-day purge proceeds
        (format!("UPDATE admin_flags SET resolved_at = ?1, detail = json_set(COALESCE(CASE WHEN json_valid(detail) THEN detail END, '{{}}'), '$.outcome', 'author_erased') WHERE kind = 'appeal' AND resolved_at IS NULL AND (user_id IN ({done}) OR item_id IN (SELECT id FROM items WHERE author_id IN ({done})))"), true),
        (format!("UPDATE dmca_counter_notices SET name = '[deleted]', address = '[deleted]', phone = '[deleted]', signature = '[deleted]', material_desc = '[deleted]' WHERE action_id IN (SELECT id FROM dmca_actions WHERE author_id IN ({done})) AND (name <> '[deleted]' OR address <> '[deleted]' OR phone <> '[deleted]' OR signature <> '[deleted]' OR material_desc <> '[deleted]')"), false),
        (format!("UPDATE pool_audit SET note = NULL WHERE note IS NOT NULL AND (actor_id IN ({done}) OR (target_user_id IN ({done}) AND actor_kind <> 'admin'))"), false),
        (format!("UPDATE pool_audit SET actor_id = NULL, detail = json_remove(detail, '$.report_id') WHERE action = 'item.report' AND actor_id IN ({done})"), false),
        (format!("DELETE FROM pool_prefs WHERE user_id IN ({done})"), false),
        (format!("DELETE FROM usage_daily WHERE user_id IN ({done})"), false),
        // (a session or a late credit written by a request that was past its guard when the erasure ran)
        (format!("DELETE FROM credit_ledger WHERE principal IN ({done})"), false),
        (format!("DELETE FROM ai_sessions WHERE principal IN ({done})"), false),
        (format!("DELETE FROM ai_usage WHERE principal IN ({done})"), false),
        (format!("DELETE FROM pool_limits WHERE user_id IN ({done})"), false),
        (format!("DELETE FROM pending_blobs WHERE uploader_id IN ({done})"), false),
        (format!("DELETE FROM pool_locks WHERE user_id IN ({done})"), false),
        (format!("DELETE FROM pool_invite_redemptions WHERE user_id IN ({done})"), false),
        (format!("DELETE FROM served WHERE reader_id IN ({done})"), false),
        (format!("DELETE FROM author_contents WHERE author_id IN ({done})"), false),
        (format!("DELETE FROM author_firsts WHERE author_id IN ({done})"), false),
        (format!("DELETE FROM author_stems WHERE author_id IN ({done})"), false),
        (format!("DELETE FROM stem_bands WHERE user_id IN ({done}) OR user_id IN (SELECT 'own:' || user_id FROM erasure_done)"), false),
        (format!("DELETE FROM carried_keys WHERE author_id IN ({done})"), false),
        (format!("DELETE FROM blob_uploads WHERE uploader_id IN ({done})"), false),
    ];
    for (sql, bind_now) in stmts {
        if bind_now {
            tx.execute(&sql, [now])?;
        } else {
            tx.execute(&sql, [])?;
        }
    }
    // Every circle table a request in flight could have written under the id, and open circles left without an owner.
    circle_resweep(&tx, now, &ownerless)?;
    tx.commit()?;
    Ok(())
}

/// An account deletion whose content hook failed after the tombstone committed: caught up here (never before the erasure batch committed: it applies the
/// author's keep_on_delete choice; until then `erasure_finish` retries it).
pub fn account_deleted_catchup(x: &PoolCtx) -> Res<usize> {
    Ok(x.conn.execute(
        "UPDATE items SET removed_at = ?1, removed_reason = 'account_deleted', purge_after = ?2, updated_at = ?1
          WHERE removed_at IS NULL AND anonymized = 0 AND author_id IN (SELECT id FROM users WHERE deleted_at IS NOT NULL)
            AND author_id IN (SELECT user_id FROM erasure_done)",
        params![x.now, x.now + 14 * DAY_MS],
    )?)
}

/// A ghost (the key-only record of a publish someone was served) goes only when no served row references it any more; served rows themselves never expire
/// (they go with their reader's account). Bounded by ROWS per run (GHOST_GC_ROWS each), resumed the next day; a ghost's row goes only once its questions and
/// units are gone.
pub fn ghost_gc(x: &PoolCtx, rows: i64) -> Res<()> {
    let gone = "SELECT g.publish_id FROM ghost_publishes g WHERE NOT EXISTS (SELECT 1 FROM served s WHERE s.publish_id = g.publish_id)";
    let tx = x.conn.unchecked_transaction()?;
    tx.execute(&format!("DELETE FROM ghost_units WHERE rowid IN (SELECT rowid FROM ghost_units WHERE publish_id IN ({gone}) LIMIT {rows})"), [])?;
    tx.execute(&format!("DELETE FROM ghost_questions WHERE rowid IN (SELECT rowid FROM ghost_questions WHERE publish_id IN ({gone}) LIMIT {rows})"), [])?;
    // (the keyed bands of a question, once no ghost of it is left)
    tx.execute(
        &format!(
            "DELETE FROM ghost_bands WHERE rowid IN (SELECT gb.rowid FROM ghost_bands gb
                WHERE gb.item_id IN (SELECT gp.item_id FROM ghost_publishes gp WHERE gp.publish_id IN ({gone}))
                  AND NOT EXISTS (SELECT 1 FROM ghost_publishes g2 JOIN ghost_questions q2 ON q2.publish_id = g2.publish_id
                                   WHERE g2.item_id = gb.item_id AND q2.qid = gb.qid AND g2.publish_id NOT IN ({gone})) LIMIT {rows})"
        ),
        [],
    )?;
    let empty = "NOT EXISTS (SELECT 1 FROM ghost_units u WHERE u.publish_id = g.publish_id) AND NOT EXISTS (SELECT 1 FROM ghost_questions q WHERE q.publish_id = g.publish_id)";
    tx.execute(&format!("DELETE FROM attestations WHERE publish_id IN (SELECT g.publish_id FROM ({gone}) g WHERE {empty} LIMIT {rows})"), [])?;
    tx.execute(&format!("DELETE FROM ghost_publishes WHERE publish_id IN (SELECT g.publish_id FROM ({gone}) g WHERE {empty} LIMIT {rows})"), [])?;
    tx.commit()?;
    Ok(())
}

/// Purges in batches until none is due or the time budget is spent; an item that fails is skipped (flagged once, with a 1, 2, 4 ... day backoff, at most 7) and
/// the rest still go: one bad item never stalls the queue.
pub fn purges(x: &PoolCtx, store: &BlobStore, max_batches: usize, budget: Duration) -> Res<usize> {
    let started = Instant::now();
    let now = x.now;
    let mut failed: Vec<String> = vec![];
    let mut n = 0;
    let mut batch = 0;
    while batch < max_batches && started.elapsed() < budget {
        batch += 1;
        let items: Vec<String> = x
            .conn
            .prepare(&format!(
                // Oldest due first; never one with an open appeal; a failed one waits for its backoff.
                "SELECT id FROM items WHERE removed_at IS NOT NULL AND id NOT IN (SELECT value FROM json_each(?2))
                    AND ((purge_after IS NOT NULL AND purge_after <= ?1 AND purged_at IS NULL)
                         OR (purged_at IS NOT NULL AND EXISTS (SELECT 1 FROM item_publishes p WHERE p.item_id = items.id)))
                    AND (purge_retry_at IS NULL OR purge_retry_at <= ?1) AND NOT {}
                  ORDER BY COALESCE(purge_after, purged_at), id LIMIT 200",
                open_appeal_sql("items.id")
            ))?
            .query_map(params![now, json!(failed).to_string()], |r| r.get(0))?
            .collect::<Result<_, _>>()?;
        if items.is_empty() {
            break;
        }
        for id in items {
            match purge_item(x, store, &id, &Actor { id: None, kind: "system" }, "purge_after") {
                Ok(true) => n += 1,
                // not claimable now (e.g. a put-back claimed it): not retried this run
                Ok(false) => failed.push(id),
                Err(e) => {
                    failed.push(id.clone());
                    eprintln!("pool cron: purge of {id} failed: {e:?}");
                    x.conn.execute("UPDATE items SET purge_failures = purge_failures + 1, purge_retry_at = ?2 + MIN(7, 1 << MIN(purge_failures, 3)) * ?3 WHERE id = ?1", params![id, now, DAY_MS])?;
                    x.conn.execute(
                        "INSERT OR IGNORE INTO admin_flags (id, kind, item_id, detail, dedup, at) VALUES (?1, 'cron_failure', ?2, ?3, ?4, ?5)",
                        params![new_id(), id, json!({ "stage": "purge_item" }).to_string(), format!("purge:{id}:{}", now / DAY_MS), now],
                    )?;
                }
            }
        }
    }
    Ok(n)
}

/// Small tables that only grow: old counters, alert budgets, used preview tokens, per-address invitation counts, and the masked address of an expired invite.
pub fn cleanup(x: &PoolCtx) -> Res<()> {
    let now = x.now;
    let tx = x.conn.unchecked_transaction()?;
    tx.execute("DELETE FROM usage_daily WHERE day < ?1", [now / DAY_MS - 2])?;
    tx.execute("DELETE FROM pool_alert_budget WHERE hour < ?1", [now / HOUR_MS - 48])?;
    tx.execute("DELETE FROM preview_tokens_used WHERE at < ?1", [now - PREVIEW_MS])?;
    // (the per-address invitation counts, an HMAC of each address, kept at most two days)
    tx.execute("DELETE FROM circle_email_sends WHERE day < ?1", [now / DAY_MS - 1])?;
    // (the masked address of an email invite, once the invite is 30 days past its expiry, with its address key; bounded a run)
    tx.execute(
        "UPDATE circle_invites SET email_label = NULL, email_key = NULL WHERE id IN (SELECT id FROM circle_invites INDEXED BY circle_invites_label WHERE email_label IS NOT NULL AND expires_at < ?1 LIMIT 500)",
        [now - 30 * DAY_MS],
    )?;
    tx.commit()?;
    Ok(())
}

/// Daily: DMCA put-backs and quarantine expiry, the erasure catch-ups, appeals, privacy bytes, the ghost GC, the purge of everything past purge_after, the
/// circle prune and purges, retention and cleanup. Each stage isolated. (The backfills and the stem/licence re-key follow with `publish.ts`.)
pub fn daily(x: &PoolCtx, store: &BlobStore) -> DailyReport {
    let DailyEarly { restored, quarantine_expired } = daily_early(x, store);
    stage(x, "erasure_finish", 0, || erasure_finish(x));
    stage(x, "erasure_resweep", (), || erasure_resweep(x));
    stage(x, "account_deleted", 0, || account_deleted_catchup(x));
    // Open appeals: a reminder after a week; unanswered for 60 days, the item is restored.
    stage(x, "appeals", (0, 0), || appeals_due(x, store));
    stage(x, "privacy_bytes", (), || privacy_bytes(x, store));
    stage(x, "ghost_gc", (), || ghost_gc(x, GHOST_GC_ROWS));
    // Large privacy purges are continued hourly; once more here for a deployment without the hourly cron.
    stage(x, "privacy_backlog", (), || privacy_backlog(x, store));
    stage(x, "deletion_rewrites", (), || deletion_rewrites(x, REWRITE_BATCH, REWRITE_BATCHES));
    let purged = stage(x, "purges", 0, || purges(x, store, PURGE_BATCHES_MAX, PURGE_TIME_BUDGET));
    // Pruned versions' bodies, then removed banks and closed circles past their 14 days (never one with an open report).
    stage(x, "circle_prune", 0, || Ok(prune_circle_versions(x, store)?));
    stage(x, "circle_purges", (0, 0), || Ok(purge_circles(x, store)?));
    // Audit and takedown records older than 3 years go (bounded; resumed next day).
    stage(x, "retention", Default::default(), || retention(x));
    stage(x, "cleanup", (), || cleanup(x));
    DailyReport { purged, restored, quarantine_expired }
}

#[cfg(test)]
#[path = "crondaily_tests.rs"]
mod tests;
