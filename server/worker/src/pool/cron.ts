// The pool's scheduled work. Ages, not states, guard in-flight work (docs/shared-pool.md "GC").
import { holdPrivacyBatch, PREVIEW_MS } from "./blocking.ts";
import { appealsDue, finishQuarantines, gcBlobs, ghostUpholds, privacyFinish, putbackFinish, releaseRestoredHolds, repeatInfringerSweep, OPEN_APPEAL_SQL, purgeItem, QUARANTINE_MS, restore, retryOrphans } from "./moderation.ts";
import { usedUnserved } from "./served.ts";
import { deleteContentStatements } from "./index.ts";
import { offerSync, deletePublishStatements, discardQidsStatement, fold, licenceRekey, searchKey, servedWords, stemBackfill, titleSearch } from "./publish.ts";
import { bodyTokens, unitKeysOf, fieldKeys, keyfreeHash, nearHash, nearKfHash, normSkeletonKey, partKeysOf, stemBands, stemKey, stemTokens } from "./provenance.ts";
import { questionsOf, takeWasmTraps, WASM_TRAPS_PER_HOUR, type PayloadJson, type QuestionJson } from "./core.ts";
import { alertBudgeted, flushOutbox, keyedBands, keyedBody, keyedSet, DAY_MS, HOUR_MS, ISOLATE_BYTES_BUDGET, newId, PoolError, PUBLISH_COST_PER_BYTE, reserveBytes, type Ctx } from "./shared.ts";

/**
 * One cron stage (round 7): a failure is logged, flagged (cron_failure, once per stage and hour)
 * and mailed within the alert budget, and the other stages still run. Returns `fallback` on failure.
 */
async function stage<T>(c: Ctx, name: string, fallback: T, fn: () => Promise<T>): Promise<T> {
  try {
    return await fn();
  } catch (e) {
    console.error(`pool cron: stage ${name} failed: ${e instanceof Error ? e.message : String(e)}`);
    try {
      const ins = await c.db
        .prepare(`INSERT OR IGNORE INTO admin_flags (id, kind, detail, dedup, at) VALUES (?1, 'cron_failure', ?2, ?3, ?4)`)
        .bind(newId(), JSON.stringify({ stage: name }), `cron:${name}:${Math.floor(c.now() / HOUR_MS)}`, c.now())
        .run();
      if ((ins.meta.changes ?? 0) === 1) await alertBudgeted(c, { kind: "cron_failure", stage: name });
    } catch {
      /* the database itself is failing: the log line above is all we can do */
    }
    return fallback;
  }
}

export interface HourlyReport {
  staged: number;
  pending: number;
  blobs: number;
  missing: number;
}
export const RECONCILE_BATCH = 200;
/** The daily purge works through due items in batches of 200 until done or this budget is spent. */
export const PURGE_BATCHES_MAX = 50;
export const PURGE_TIME_BUDGET_MS = 20_000;

/** Hourly: abandoned staged publishes, expired pending uploads, unreferenced blobs, the reconciler, and large privacy purges' remaining holds. */
export async function hourly(c: Ctx): Promise<HourlyReport> {
  const now = c.now();
  const staged = await stage(c, "staged", 0, () => discardAbandoned(c, now));
  const pending = await stage(c, "pending", 0, async () => (await c.db.prepare(`DELETE FROM pending_blobs WHERE until < ?1`).bind(now - HOUR_MS).run()).meta.changes ?? 0);
  const blobs = await stage(c, "gc", 0, () => gcBlobs(c));
  const missing = await stage(c, "reconcile", 0, () => reconcile(c, now));
  await stage(c, "r2_orphans", 0, () => retryOrphans(c));
  // Round 23 (critic30b F2/F3): notices written with their action, sent until sent.
  await stage(c, "outbox", 0, () => flushOutbox(c));
  // A privacy purge's holds that one request could not place: continued every hour, never waiting a day.
  await stage(c, "privacy_backlog", undefined, () => privacyBacklog(c, now));
  // Account deletions' chain rewrites, in bounded batches (round 11).
  await stage(c, "deletion_rewrites", undefined, () => deletionRewrites(c, now));
  // Round 21 (critic27b G1): takedowns whose bodies were not all quarantined are finished.
  await stage(c, "quarantine", undefined, () => finishQuarantines(c));
  // Items published before rounds 11-12: folds, search words, block keys, body hashes (round 12).
  await stage(c, "backfill", undefined, () => backfill(c));
  // Round 25 (critic-moodle-24 B-2): legacy stem records get their hidden full-form explanation key
  // (bounded; a few authors a run, after the backfill has computed the keyed bands).
  await stage(c, "stem_backfill", undefined, () => stemBackfill(c));
  // Round 25 (critic-asvs-24 B3): old exact-content licence keys the 0105 migration could not map yet
  // (its near hashes were NULL before the backfill) are re-keyed to the canonical key now.
  await stage(c, "licence_rekey", undefined, () => licenceRekey(c));
  // Round 25 (critic-moodle-24, new6): the licence history is reconciled HOURLY, as the README says
  // (a licence re-opened by an admin hold release is not refused for up to a day); bounded per run.
  await stage(c, "offer_sync", undefined, () => offerReconcile(c));
  // Round 18 (critic23d V4): the database's size, against D1's limit (shared with the accounts).
  await stage(c, "d1_size", undefined, () => d1SizeCheck(c));
  // Round 21: share-core traps in the last hour.
  await stage(c, "wasm_traps", undefined, () => wasmTrapCheck(c));
  return { staged, pending, blobs, missing };
}

async function privacyBacklog(c: Ctx, now: number): Promise<void> {
  const backlog = await c.db.prepare(`SELECT id, item_id, detail FROM admin_flags WHERE kind = 'privacy_backlog' AND resolved_at IS NULL LIMIT 20`).all<{ id: string; item_id: string; detail: string }>();
  for (const b of backlog.results) {
    const d = JSON.parse(b.detail) as { match: string[]; blob: string[] };
    // (Round 22 H2: the copies, then the bodies' move, then resolved: privacyFinish.)
    await privacyFinish(c, b.item_id, d.match, d.blob, null, 20);
  }
}

async function discardAbandoned(c: Ctx, now: number): Promise<number> {
  const stale = await c.db
    .prepare(`SELECT publish_id, item_id, full_key, stripped_key, created_at FROM item_publishes WHERE status IN ('staging', 'staged') AND created_at < ?1 LIMIT 200`)
    .bind(now - HOUR_MS)
    .all<{ publish_id: string; item_id: string; full_key: string; stripped_key: string | null; created_at: number }>();
  let staged = 0;
  for (const r of stale.results) {
    const del = await c.db.prepare(`DELETE FROM item_publishes WHERE publish_id = ?1 AND status IN ('staging', 'staged') AND created_at = ?2`).bind(r.publish_id, r.created_at).run();
    if ((del.meta.changes ?? 0) !== 1) continue;
    await c.db.batch([...deletePublishStatements(c.db, [r.publish_id]), discardQidsStatement(c.db, r.publish_id, r.item_id)]);
    await c.r2.delete([r.full_key, ...(r.stripped_key ? [r.stripped_key] : [])]);
    staged++;
  }
  return staged;
}

async function reconcile(c: Ctx, now: number): Promise<number> {
  // Reconciler: a blob row whose bytes are gone (a GC that raced an upload) is marked missing, so
  // the next publish names the positions and the author uploads again; found again, it is cleared.
  // Images a privacy purge removed are gone on purpose (whether or not bytes were kept for an
  // author's unserved use): not "missing", and not checked, so nothing here depends on that.
  const check = await c.db
    .prepare(`SELECT sha256 FROM blobs WHERE privacy_purged_at IS NULL ORDER BY COALESCE(checked_at, 0) LIMIT ?1`)
    .bind(RECONCILE_BATCH)
    .all<{ sha256: string }>();
  let missing = 0;
  for (const r of check.results) {
    const there = (await c.r2.head(`blobs/${r.sha256}`)) !== null;
    if (!there) missing++;
    await c.db
      .prepare(`UPDATE blobs SET checked_at = ?2, missing_at = CASE WHEN ?3 = 1 THEN NULL ELSE COALESCE(missing_at, ?2) END WHERE sha256 = ?1`)
      .bind(r.sha256, now, there ? 1 : 0)
      .run();
  }
  return missing;
}

export interface DailyReport {
  purged: number;
  restored: number;
  quarantineExpired: number;
}

/** Daily: DMCA put-backs and quarantine expiry, then the purge of everything past purge_after. Each stage isolated. */
export async function daily(c: Ctx): Promise<DailyReport> {
  const now = c.now();
  // A complete counter-notice and no court action: restored no sooner than 10 business days after receipt.
  const restored = await stage(c, "putbacks", 0, async () => {
    const due = await c.db
      .prepare(`SELECT id FROM dmca_actions WHERE counter_notice_id IS NOT NULL AND restore_not_before <= ?1 AND court_action_at IS NULL AND restored_at IS NULL AND purged_at IS NULL LIMIT 100`)
      .bind(now)
      .all<{ id: string }>();
    let n = 0;
    for (const a of due.results) if (await restore(c, a.id, { id: null, kind: "system" }, "put_back")) n++;
    return n;
  });
  // Round 22 (critic29a B1): ghost takedowns whose window passed are upheld.
  await stage(c, "ghost_upholds", 0, () => ghostUpholds(c));
  await stage(c, "restored_holds", 0, () => releaseRestoredHolds(c));
  // Round 25 (critic-moodle-24 B-2 / critic-asvs-24 B3): finish the stem backfill and any old
  // exact-content licence keys first (a daily run may be the only cron a deployment gets).
  await stage(c, "stem_backfill", 0, () => stemBackfill(c));
  await stage(c, "licence_rekey", 0, () => licenceRekey(c));
  // Round 23 (critic30b W1, critic30a B4): the licence history reconciled with what each item offers
  // (holds and removals since), and collected where no serve can reach it.
  await stage(c, "offer_sync", 0, () => offerReconcile(c));
  await stage(c, "content_licences_gc", 0, () => contentLicencesGc(c));
  // (critic29b H3: put-backs that stopped after their claim)
  await stage(c, "putback_finish", 0, () => putbackFinish(c));
  // (critic29b: a repeat-infringer check that failed, redone from the strikes)
  await stage(c, "repeat_infringers", 0, () => repeatInfringerSweep(c));
  // No counter-notice within 30 days of notifying the author: purge.
  const quarantineExpired = await stage(c, "quarantine_expiry", 0, async () => (await c.db
    .prepare(
      `UPDATE items SET purge_after = ?1 WHERE removed_reason = 'takedown' AND purged_at IS NULL AND purge_after IS NULL
         AND id IN (SELECT item_id FROM dmca_actions WHERE counter_notice_id IS NULL AND court_action_at IS NULL AND restored_at IS NULL AND purged_at IS NULL AND author_notified_at <= ?2)`,
    )
    .bind(now, now - QUARANTINE_MS)
    .run()).meta.changes ?? 0);
  // Round 22 (critic29b): an account whose erasure batch failed (deleted, no erasure_done row): the whole
  // batch again (each step idempotent), until it commits.
  await stage(c, "erasure_finish", 0, async () => {
    const due = await c.db.prepare(`SELECT id FROM users WHERE deleted_at IS NOT NULL AND id NOT IN (SELECT user_id FROM erasure_done) LIMIT 20`).all<{ id: string }>();
    for (const u of due.results) await c.db.batch(deleteContentStatements(c.db, u.id, now));
    return due.results.length;
  });
  // Round 26 (critic-moodle-33c B-3), widened in round 27 (critic35d B-1): defence in depth. Every write a
  // user request or job makes under an account id is guarded IN the write (ALIVE_SQL: nothing is written
  // for an erased account), but a request that was already past that guard when the erasure batch ran is
  // the one race a guard cannot see, and an older build may have written before it existed; so every
  // table such a writer touches is cleaned again daily for erased accounts. Bounded (the imports
  // re-key takes at most 50 accounts a run; the rest are indexed lookups on rows that exist only after a
  // straggler) and idempotent (each statement matches only what is not yet clean). Never touched: kept
  // anonymised items, the users tombstone, erasure_done, audit rows without notes, retired_publish_ids,
  // attestations (the documented residue).
  await stage(c, "erasure_resweep", undefined, async () => {
    const done = `SELECT user_id FROM erasure_done`;
    // The importer's per-question records are re-keyed to one anonymous key per account, as the erasure
    // batch does (their counts stay, the account's id does not).
    const stragglers = await c.db.prepare(`SELECT DISTINCT user_id FROM imports WHERE user_id IN (${done}) LIMIT 50`).all<{ user_id: string }>();
    const now = c.now();
    await c.db.batch([
      ...stragglers.results.map((r) => c.db.prepare(`UPDATE imports SET user_id = ?2 WHERE user_id = ?1`).bind(r.user_id, `deleted:${crypto.randomUUID().replaceAll("-", "")}`)),
      // (the key_leak flag's quoted text goes before the reports lose their reporter)
      c.db.prepare(
        `UPDATE admin_flags SET detail = json_remove(detail, '$.served') WHERE kind = 'key_leak' AND json_valid(detail) AND json_type(detail, '$.served') IS NOT NULL
            AND detail ->> '$.report_id' IN (SELECT id FROM reports WHERE reporter_id IN (${done}))`,
      ),
      c.db.prepare(`UPDATE reports SET note = NULL WHERE reporter_id IN (${done}) AND note IS NOT NULL`),
      c.db.prepare(`UPDATE reports SET reporter_id = NULL WHERE reporter_id IN (${done})`),
      c.db.prepare(
        `UPDATE admin_flags SET detail = json_remove(detail, '$.statement', '$.note')
          WHERE user_id IN (${done}) AND json_valid(detail) AND (json_type(detail, '$.note') IS NOT NULL OR json_type(detail, '$.statement') IS NOT NULL)`,
      ),
      // an open appeal of an erased account (or on its removed items) closes, so the 14-day purge proceeds
      c.db
        .prepare(
          `UPDATE admin_flags SET resolved_at = ?1, detail = json_set(COALESCE(CASE WHEN json_valid(detail) THEN detail END, '{}'), '$.outcome', 'author_erased')
            WHERE kind = 'appeal' AND resolved_at IS NULL AND (user_id IN (${done}) OR item_id IN (SELECT id FROM items WHERE author_id IN (${done})))`,
        )
        .bind(now),
      c.db.prepare(
        `UPDATE dmca_counter_notices SET name = '[deleted]', address = '[deleted]', phone = '[deleted]', signature = '[deleted]', material_desc = '[deleted]'
          WHERE action_id IN (SELECT id FROM dmca_actions WHERE author_id IN (${done}))
            AND (name <> '[deleted]' OR address <> '[deleted]' OR phone <> '[deleted]' OR signature <> '[deleted]' OR material_desc <> '[deleted]')`,
      ),
      c.db.prepare(`UPDATE pool_audit SET note = NULL WHERE note IS NOT NULL AND (actor_id IN (${done}) OR (target_user_id IN (${done}) AND actor_kind <> 'admin'))`),
      c.db.prepare(`UPDATE pool_audit SET actor_id = NULL, detail = json_remove(detail, '$.report_id') WHERE action = 'item.report' AND actor_id IN (${done})`),
      c.db.prepare(`DELETE FROM pool_prefs WHERE user_id IN (${done})`),
      c.db.prepare(`DELETE FROM usage_daily WHERE user_id IN (${done})`),
      c.db.prepare(`DELETE FROM pool_limits WHERE user_id IN (${done})`),
      c.db.prepare(`DELETE FROM pending_blobs WHERE uploader_id IN (${done})`),
      c.db.prepare(`DELETE FROM pool_locks WHERE user_id IN (${done})`),
      // (round 28: POST /me/invite writes under the caller's id)
      c.db.prepare(`DELETE FROM pool_invite_redemptions WHERE user_id IN (${done})`),
      c.db.prepare(`DELETE FROM served WHERE reader_id IN (${done})`),
      c.db.prepare(`DELETE FROM author_contents WHERE author_id IN (${done})`),
      c.db.prepare(`DELETE FROM author_firsts WHERE author_id IN (${done})`),
      c.db.prepare(`DELETE FROM author_stems WHERE author_id IN (${done})`),
      c.db.prepare(`DELETE FROM stem_bands WHERE user_id IN (${done}) OR user_id IN (SELECT 'own:' || user_id FROM erasure_done)`),
      c.db.prepare(`DELETE FROM carried_keys WHERE author_id IN (${done})`),
      c.db.prepare(`DELETE FROM blob_uploads WHERE uploader_id IN (${done})`),
    ]);
  });
  // An account deletion whose content hook failed after the tombstone committed: caught up here.
  await stage(c, "account_deleted", undefined, async () => void (await c.db
    .prepare(
      `UPDATE items SET removed_at = ?1, removed_reason = 'account_deleted', purge_after = ?2, updated_at = ?1
        WHERE removed_at IS NULL AND anonymized = 0 AND author_id IN (SELECT id FROM users WHERE deleted_at IS NOT NULL)
          -- (round 23, critic30b F4: never before the erasure batch committed: it applies the author's
          -- keep_on_delete choice; until then erasure_finish retries it)
          AND author_id IN (SELECT user_id FROM erasure_done)`,
    )
    .bind(now, now + 14 * DAY_MS)
    .run()));
  // Open appeals (round 9 J5): a reminder after a week; unanswered for 60 days, the item is restored.
  await stage(c, "appeals", undefined, async () => void (await appealsDue(c)));
  await stage(c, "privacy_bytes", undefined, () => privacyBytes(c, now));
  // Round 18 (critic23d V4): evidence of sources purged more than 90 days ago goes, unless it is what
  // binds a copy the account published (its chain names the source). Bounded batches.
  // Round 19 (critic24c C3): a ghost (the key-only record of a publish someone was served) goes only
  // when no served row references it any more; served rows themselves never expire (they go with
  // their reader's account).
  await stage(c, "ghost_gc", undefined, async () => {
    // Round 20 (critic25b N4): bounded by ROWS per run (GHOST_GC_ROWS each), resumed the next day; a
    // ghost's row goes only once its questions and units are gone.
    const gone = `SELECT g.publish_id FROM ghost_publishes g WHERE NOT EXISTS (SELECT 1 FROM served s WHERE s.publish_id = g.publish_id)`;
    await c.db.batch([
      c.db.prepare(`DELETE FROM ghost_units WHERE rowid IN (SELECT rowid FROM ghost_units WHERE publish_id IN (${gone}) LIMIT ${GHOST_GC_ROWS})`),
      c.db.prepare(`DELETE FROM ghost_questions WHERE rowid IN (SELECT rowid FROM ghost_questions WHERE publish_id IN (${gone}) LIMIT ${GHOST_GC_ROWS})`),
      // (round 22 B2: the keyed bands of a question, once no ghost of it is left)
      c.db.prepare(
        `DELETE FROM ghost_bands WHERE rowid IN (SELECT gb.rowid FROM ghost_bands gb
            WHERE gb.item_id IN (SELECT gp.item_id FROM ghost_publishes gp WHERE gp.publish_id IN (${gone}))
              AND NOT EXISTS (SELECT 1 FROM ghost_publishes g2 JOIN ghost_questions q2 ON q2.publish_id = g2.publish_id
                               WHERE g2.item_id = gb.item_id AND q2.qid = gb.qid AND g2.publish_id NOT IN (${gone})) LIMIT ${GHOST_GC_ROWS})`,
      ),
      c.db.prepare(`DELETE FROM attestations WHERE publish_id IN (SELECT g.publish_id FROM (${gone}) g WHERE NOT EXISTS (SELECT 1 FROM ghost_units u WHERE u.publish_id = g.publish_id) AND NOT EXISTS (SELECT 1 FROM ghost_questions q WHERE q.publish_id = g.publish_id) LIMIT ${GHOST_GC_ROWS})`),
      c.db.prepare(
        `DELETE FROM ghost_publishes WHERE publish_id IN (SELECT g.publish_id FROM (${gone}) g
           WHERE NOT EXISTS (SELECT 1 FROM ghost_units u WHERE u.publish_id = g.publish_id) AND NOT EXISTS (SELECT 1 FROM ghost_questions q WHERE q.publish_id = g.publish_id) LIMIT ${GHOST_GC_ROWS})`,
      ),
    ]);
  });
  // Large privacy purges are continued hourly; once more here for a deployment without the hourly cron.
  await stage(c, "privacy_backlog", undefined, () => privacyBacklog(c, now));
  await stage(c, "deletion_rewrites", undefined, () => deletionRewrites(c, now));
  // Purges in batches until none is due or the time budget is spent (round 8); an item that fails is
  // skipped (flagged once) and the rest still go: one bad item never stalls the queue.
  const purged = await stage(c, "purges", 0, async () => {
    const started = Date.now();
    const failed: string[] = [];
    let n = 0;
    for (let batch = 0; batch < PURGE_BATCHES_MAX && Date.now() - started < PURGE_TIME_BUDGET_MS; batch++) {
      const items = await c.db
        // Oldest due first; never one with an open appeal (round 9 J5); a failed one waits for its backoff.
        .prepare(
          `SELECT id FROM items WHERE removed_at IS NOT NULL AND id NOT IN (SELECT value FROM json_each(?2))
              AND ((purge_after IS NOT NULL AND purge_after <= ?1 AND purged_at IS NULL)
                   -- (round 21 G1: a purge claimed earlier whose batch failed: finished now)
                   OR (purged_at IS NOT NULL AND EXISTS (SELECT 1 FROM item_publishes p WHERE p.item_id = items.id)))
              AND (purge_retry_at IS NULL OR purge_retry_at <= ?1) AND NOT ${OPEN_APPEAL_SQL("items.id")}
            ORDER BY COALESCE(purge_after, purged_at), id LIMIT 200`,
        )
        .bind(now, JSON.stringify(failed))
        .all<{ id: string }>();
      if (items.results.length === 0) break;
      for (const it of items.results) {
        try {
          if (await purgeItem(c, it.id, { id: null, kind: "system" }, "purge_after")) n++;
          else failed.push(it.id); // not claimable now (e.g. a put-back claimed it): not retried this run
        } catch (e) {
          failed.push(it.id);
          console.error(`pool cron: purge of ${it.id} failed: ${e instanceof Error ? e.message : String(e)}`);
          // Backoff (round 9): 1, 2, 4 ... days, at most 7, so one bad item is retried without clogging the queue.
          await c.db
            .prepare(`UPDATE items SET purge_failures = purge_failures + 1, purge_retry_at = ?2 + MIN(7, 1 << MIN(purge_failures, 3)) * ?3 WHERE id = ?1`)
            .bind(it.id, now, DAY_MS)
            .run();
          await c.db
            .prepare(`INSERT OR IGNORE INTO admin_flags (id, kind, item_id, detail, dedup, at) VALUES (?1, 'cron_failure', ?2, ?3, ?4, ?5)`)
            .bind(newId(), it.id, JSON.stringify({ stage: "purge_item" }), `purge:${it.id}:${Math.floor(now / DAY_MS)}`, now)
            .run();
        }
      }
    }
    return n;
  });
  // Round 28 (owner): audit and takedown records older than 3 years go (bounded; resumed next day).
  await stage(c, "retention", undefined, () => retention(c));
  await stage(c, "cleanup", undefined, async () =>
    void (await c.db.batch([
      c.db.prepare(`DELETE FROM usage_daily WHERE day < ?1`).bind(Math.floor(now / DAY_MS) - 2),
      c.db.prepare(`DELETE FROM pool_alert_budget WHERE hour < ?1`).bind(Math.floor(now / HOUR_MS) - 48),
      c.db.prepare(`DELETE FROM preview_tokens_used WHERE at < ?1`).bind(now - PREVIEW_MS),
    ])),
  );
  return { purged, restored, quarantineExpired };
}

async function privacyBytes(c: Ctx, now: number): Promise<void> {
  // Privacy-blocked images past their grace window: from now on they are gone for every SERVED use
  // (privacy_purged_at; only a fresh upload after an unblock brings one back), and their bytes are
  // deleted, except where an author still uses them unserved (only in a hidden publish's full form):
  // that author's private material is not destroyed by someone else's block, and since nothing
  // served depends on whether the bytes were kept, keeping them is observable by no one else (F1).
  const doomed = await c.db
    // An open appeal on the blocking item keeps the bytes until the admin decides (round 9 J5).
    .prepare(`UPDATE privacy_blocks SET bytes_purged_at = ?1 WHERE kind = 'blob' AND bytes_purged_at IS NULL AND bytes_purge_after <= ?1 AND NOT ${OPEN_APPEAL_SQL("privacy_blocks.item_id")} RETURNING hash`)
    .bind(now)
    .all<{ hash: string }>();
  if (doomed.results.length > 0) {
    const hs = JSON.stringify(doomed.results.map((r) => r.hash));
    await c.db.prepare(`UPDATE blobs SET privacy_purged_at = ?2 WHERE sha256 IN (SELECT value FROM json_each(?1))`).bind(hs, now).run();
    const del = await c.db
      .prepare(`UPDATE blobs SET missing_at = ?2 WHERE sha256 IN (SELECT value FROM json_each(?1)) AND NOT ${usedUnserved("blobs.sha256")} RETURNING sha256`)
      .bind(hs, now)
      .all<{ sha256: string }>();
    if (del.results.length > 0) await c.r2.delete(del.results.map((r) => `blobs/${r.sha256}`));
  }
}

// ---- retention (round 28) ---------------------------------------------------------------------------

/**
 * Round 28 (owner decision, docs/shared-pool.md "Retention"): audit and takedown records are kept 3
 * years. 1,096 days is never less than three calendar years. Migration 0106's pool_audit trigger refuses
 * to delete a younger row by the DATABASE's clock, and the delete below carries the same floor, so a
 * skewed application clock can neither delete early nor make the stage fail.
 */
export const RETENTION_MS = 1096 * DAY_MS;
/** Takedown actions removed (with their own rows) per daily run; the rest resume the next day. */
export const RETENTION_ACTIONS = 100;
/** Rows of one table (audit rows, notices, an action's block and upstream rows) removed per daily run. */
export const RETENTION_ROWS = 5000;
const DB_RETENTION_FLOOR_SQL = `(CAST(strftime('%s', 'now') AS INTEGER) * 1000 - ${RETENTION_MS})`;

/**
 * SQL condition: the takedown action `a` is a record past retention (`cut` = now - RETENTION_MS, `now`:
 * SQL parameter references). It is CLOSED (put back, withdrawn, reversed, or upheld and purged), with no
 * uphold or put-back still finishing; NOTHING about it is newer than the cutoff (its own dates, its
 * counter-notice, strike, exempt authors, hold releases, the DMCA audit rows of its item); and nothing
 * still DEPENDS on it: no re-upload block of it in force (the upstream exemption snapshot,
 * dmca_upstream_keys, and its exempt authors matter exactly while one is), no copy it holds, no open
 * appeal or unresolved takedown flag (counter-notice, copies) on its item.
 */
const RETAINABLE_ACTION_SQL = (a: string, cut: string, now: string) => `(
  (${a}.restored_at IS NOT NULL OR ${a}.purged_at IS NOT NULL) AND ${a}.uphold_pending = 0
  AND ${a}.disabled_at < ${cut} AND COALESCE(${a}.author_notified_at, 0) < ${cut} AND COALESCE(${a}.restored_at, 0) < ${cut}
  AND COALESCE(${a}.purged_at, 0) < ${cut} AND COALESCE(${a}.court_action_at, 0) < ${cut}
  AND NOT EXISTS (SELECT 1 FROM dmca_counter_notices rcn WHERE rcn.action_id = ${a}.id AND rcn.received_at >= ${cut})
  AND NOT EXISTS (SELECT 1 FROM strikes rs WHERE rs.action_id = ${a}.id AND (COALESCE(rs.upheld_at, 0) >= ${cut} OR COALESCE(rs.voided_at, 0) >= ${cut}))
  AND NOT EXISTS (SELECT 1 FROM dmca_exempt rx WHERE rx.action_id = ${a}.id AND rx.at >= ${cut})
  AND NOT EXISTS (SELECT 1 FROM hold_releases rhr WHERE rhr.source = 'dmca:' || ${a}.id AND rhr.at >= ${cut})
  AND NOT EXISTS (SELECT 1 FROM pool_audit rpa WHERE rpa.item_id = ${a}.item_id AND rpa.at >= ${cut} AND rpa.action LIKE 'dmca.%')
  AND NOT EXISTS (SELECT 1 FROM blocked_hashes rbh WHERE rbh.action_id = ${a}.id AND rbh.lifted_at IS NULL AND (rbh.expires_at IS NULL OR rbh.expires_at > ${now}))
  AND NOT EXISTS (SELECT 1 FROM item_holds rh WHERE rh.source = 'dmca:' || ${a}.id)
  AND NOT EXISTS (SELECT 1 FROM admin_flags rf WHERE rf.item_id = ${a}.item_id AND rf.resolved_at IS NULL AND rf.kind IN ('appeal', 'counter_notice', 'dmca_copy'))
  -- (a put-back whose finish is still pending: putbackFinish's own predicate)
  AND NOT (${a}.restored_at IS NOT NULL
           AND EXISTS (SELECT 1 FROM items ri WHERE ri.id = ${a}.item_id AND ri.removed_reason = 'takedown' AND (ri.purged_at IS NULL OR ri.purged_at < ${a}.disabled_at))
           AND NOT EXISTS (SELECT 1 FROM dmca_actions rb WHERE rb.item_id = ${a}.item_id AND rb.disabled_at > ${a}.disabled_at)))`;

/**
 * Round 28: the daily retention stage. Bounded and idempotent, oldest first; what one run leaves, the
 * next continues. In order:
 * 1. Takedown actions past retention (RETAINABLE_ACTION_SQL), with their counter-notice, strike, exempt
 *    authors, hold releases, (inactive) re-upload block rows and upstream snapshot. Every statement
 *    re-checks the condition inside the one batch, so an action that changed since it was picked (an
 *    admin's reversal) is left whole; an action goes only once its block and snapshot rows are all gone.
 * 2. DMCA notices received before the cutoff that no takedown action still names and that are not
 *    `open` (an open notice can still be acted on: withdraw it to let it go).
 * 3. pool_audit rows older than the cutoff (and the database's own floor), except the trail of an item
 *    that still has a takedown record kept, a hold (under review) or an open appeal, and rows naming a
 *    takedown action or notice still kept.
 * Never touched: void_items (evidence voids are live licensing state, not records), admin flags, the
 * outbox, auth_audit (the auth package's table).
 */
export async function retention(c: Ctx): Promise<{ actions: number; notices: number; audit: number }> {
  const now = c.now();
  const cut = now - RETENTION_MS;
  const due = await c.db
    .prepare(`SELECT a.id FROM dmca_actions a WHERE ${RETAINABLE_ACTION_SQL("a", "?1", "?2")} ORDER BY a.disabled_at, a.id LIMIT ${RETENTION_ACTIONS}`)
    .bind(cut, now)
    .all<{ id: string }>();
  let actions = 0;
  if (due.results.length > 0) {
    const ids = JSON.stringify(due.results.map((r) => r.id));
    const pick = `SELECT ra.id FROM dmca_actions ra WHERE ra.id IN (SELECT value FROM json_each(?1)) AND ${RETAINABLE_ACTION_SQL("ra", "?2", "?3")}`;
    const st = (sql: string) => c.db.prepare(sql).bind(ids, cut, now);
    const res = await c.db.batch([
      // (WITHOUT ROWID: deleted by its key, a bounded page of it)
      st(`DELETE FROM dmca_upstream_keys WHERE (action_id, item_id, key) IN (SELECT action_id, item_id, key FROM dmca_upstream_keys WHERE action_id IN (${pick}) LIMIT ${RETENTION_ROWS})`),
      st(`DELETE FROM blocked_hashes WHERE rowid IN (SELECT rowid FROM blocked_hashes WHERE action_id IN (${pick}) LIMIT ${RETENTION_ROWS})`),
      st(`DELETE FROM dmca_exempt WHERE action_id IN (${pick})`),
      st(`DELETE FROM hold_releases WHERE source IN (SELECT 'dmca:' || id FROM (${pick}))`),
      st(`DELETE FROM dmca_counter_notices WHERE action_id IN (${pick})`),
      st(`DELETE FROM strikes WHERE action_id IN (${pick})`),
      st(
        `DELETE FROM dmca_actions WHERE id IN (${pick})
           AND NOT EXISTS (SELECT 1 FROM dmca_upstream_keys k WHERE k.action_id = dmca_actions.id)
           AND NOT EXISTS (SELECT 1 FROM blocked_hashes b WHERE b.action_id = dmca_actions.id)`,
      ),
    ]);
    actions = res[res.length - 1]!.meta.changes ?? 0;
  }
  const notices = (
    await c.db
      .prepare(
        `DELETE FROM dmca_notices WHERE id IN (SELECT n.id FROM dmca_notices n WHERE n.received_at < ?1 AND n.status <> 'open'
            AND NOT EXISTS (SELECT 1 FROM dmca_actions a WHERE a.notice_id = n.id) ORDER BY n.received_at LIMIT ${RETENTION_ROWS})`,
      )
      .bind(cut)
      .run()
  ).meta.changes ?? 0;
  // (the CASE keeps json_valid ahead of the extraction: an unparsable detail is never read)
  const names = (table: string, field: string) =>
    `EXISTS (SELECT 1 FROM ${table} x WHERE x.id = CASE WHEN json_valid(p.detail) THEN p.detail ->> '$.${field}' END)`;
  const audit = (
    await c.db
      .prepare(
        `DELETE FROM pool_audit WHERE rowid IN (SELECT p.rowid FROM pool_audit p WHERE p.at < ?1 AND p.at < ${DB_RETENTION_FLOOR_SQL}
            AND NOT (p.item_id IS NOT NULL AND (EXISTS (SELECT 1 FROM dmca_actions a WHERE a.item_id = p.item_id)
                                                OR EXISTS (SELECT 1 FROM item_holds h WHERE h.item_id = p.item_id)
                                                OR ${OPEN_APPEAL_SQL("p.item_id")}))
            AND NOT ${names("dmca_actions", "action_id")} AND NOT ${names("dmca_notices", "notice_id")}
          ORDER BY p.at LIMIT ${RETENTION_ROWS})`,
      )
      .bind(cut)
      .run()
  ).meta.changes ?? 0;
  return { actions, notices, audit };
}

/** Rows per batch and batches per cron run for account deletions' chain rewrites (round 11). */
export const REWRITE_BATCH = 500;
export const REWRITE_BATCHES = 20;

/**
 * Account deletions' rewrites of others' stored chains: the account's name in each link to one of
 * its items becomes "deleted teacher". Bounded: REWRITE_BATCH rows by rowid per statement, at most
 * REWRITE_BATCHES statements per run, resumed from the stored cursor.
 */
export async function deletionRewrites(c: Ctx, now: number): Promise<void> {
  let left = c.deps.rewriteBatches ?? REWRITE_BATCHES;
  const size = c.deps.rewriteBatch ?? REWRITE_BATCH;
  while (left > 0) {
    const job = await c.db.prepare(`SELECT user_id, tbl, cursor FROM deletion_rewrites WHERE done_at IS NULL ORDER BY at, user_id, tbl LIMIT 1`).first<{ user_id: string; tbl: string; cursor: number }>();
    if (!job) return;
    const t = job.tbl === "item_questions" ? "item_questions" : "ghost_questions";
    for (; left > 0; left--) {
      const range = `SELECT rowid FROM ${t} WHERE rowid > ?2 ORDER BY rowid LIMIT ${size}`;
      const [, hi] = await c.db.batch([
        c.db
          .prepare(
            `UPDATE ${t} SET chain_json = (SELECT json_group_array(CASE WHEN l.value ->> '$.item_id' IN (SELECT id FROM items WHERE author_id = ?1)
                                                                   THEN json_set(l.value, '$.author_name', 'deleted teacher') ELSE json(l.value) END)
                                            FROM json_each(${t}.chain_json) l)
              WHERE rowid IN (${range}) AND chain_json <> '[]' AND EXISTS (SELECT 1 FROM json_each(${t}.chain_json) l JOIN items x ON x.id = l.value ->> '$.item_id' WHERE x.author_id = ?1)`,
          )
          .bind(job.user_id, job.cursor),
        c.db.prepare(`SELECT MAX(rowid) AS hi FROM (${range})`).bind(job.user_id, job.cursor),
      ]);
      const top = (hi!.results[0] as { hi: number | null } | undefined)?.hi ?? null;
      if (top === null) {
        await c.db.prepare(`UPDATE deletion_rewrites SET done_at = ?3 WHERE user_id = ?1 AND tbl = ?2`).bind(job.user_id, job.tbl, now).run();
        left--;
        break;
      }
      job.cursor = top;
      await c.db.prepare(`UPDATE deletion_rewrites SET cursor = ?3 WHERE user_id = ?1 AND tbl = ?2`).bind(job.user_id, job.tbl, top).run();
    }
  }
}

/** Round 21: share-core traps counted in this isolate, added to the hour's count in pool_kv. */
export async function flushWasmTraps(c: Pick<Ctx, "db" | "now">): Promise<void> {
  const n = takeWasmTraps();
  if (n === 0) return;
  await c.db
    .prepare(`INSERT INTO pool_kv (key, value) VALUES (?1, ?2) ON CONFLICT (key) DO UPDATE SET value = CAST(CAST(value AS INTEGER) + ?2 AS TEXT)`)
    .bind(`wasm_traps:${Math.floor(c.now() / HOUR_MS)}`, String(n))
    .run();
}

/** Round 21: past WASM_TRAPS_PER_HOUR traps in the last hour, the admin is flagged and told. */
async function wasmTrapCheck(c: Ctx): Promise<void> {
  await flushWasmTraps(c);
  const hour = Math.floor(c.now() / HOUR_MS);
  for (const h of [hour - 1, hour]) {
    const v = await c.db.prepare(`SELECT value FROM pool_kv WHERE key = ?1`).bind(`wasm_traps:${h}`).first<{ value: string }>();
    const n = Number(v?.value ?? 0);
    if (n <= WASM_TRAPS_PER_HOUR) continue;
    const f = await c.db
      .prepare(`INSERT OR IGNORE INTO admin_flags (id, kind, detail, dedup, at) VALUES (?1, 'wasm_trap', ?2, ?3, ?4)`)
      .bind(newId(), JSON.stringify({ hour: h, traps: n }), `wasm_trap:${h}`, c.now())
      .run();
    if ((f.meta.changes ?? 0) === 1) await alertBudgeted(c, { kind: "wasm_trap", traps: n });
  }
  await c.db.prepare(`DELETE FROM pool_kv WHERE key LIKE 'wasm_traps:%' AND CAST(substr(key, 12) AS INTEGER) < ?1`).bind(hour - 24).run();
}

/** Ghost rows (units, questions, publishes: each) the daily GC deletes per run (round 20 N4). */
export const GHOST_GC_ROWS = 20_000;

/** D1's database size limit, and the shares at which the admin is alerted (round 18, critic23d V4). */
export const D1_LIMIT_BYTES = 10 * 1024 ** 3;
export const D1_ALERT_SHARES = [0.6, 0.8];

async function d1SizeCheck(c: Ctx): Promise<void> {
  const r = await c.db.prepare(`SELECT 1 AS x`).run();
  const size = (r.meta as { size_after?: number }).size_after;
  if (typeof size !== "number") return; // (not D1: tests)
  for (const share of [...D1_ALERT_SHARES].reverse())
    if (size >= share * D1_LIMIT_BYTES) {
      const day = Math.floor(c.now() / DAY_MS);
      const f = await c.db
        .prepare(`INSERT OR IGNORE INTO admin_flags (id, kind, detail, dedup, at) VALUES (?1, 'd1_size', ?2, ?3, ?4)`)
        .bind(newId(), JSON.stringify({ bytes: size, share }), `d1_size:${share}:${day}`, c.now())
        .run();
      if ((f.meta.changes ?? 0) === 1) await alertBudgeted(c, { kind: "d1_size", percent: Math.round(share * 100), bytes: size });
      break;
    }
}

/** Items re-derived per cron run (round 12 backfill). */
export const BACKFILL_ITEMS = 200;

/**
 * Round 12: items written before rounds 11-12 (items.fold_rev = 0) get, from their stored bodies,
 * what those rounds compute at publish: the exact search folds of the title, subject and tags, the
 * search words (served stems, title, subject, tags) of every retained publish, and each question's
 * block keys and body hash. Bounded (BACKFILL_ITEMS per run); an item whose body is missing is marked
 * done with what SQL could give it (0103's ASCII folds; blocks then match its exact hashes only).
 */
export async function backfill(c: Ctx): Promise<void> {
  const items = await c.db
    .prepare(`SELECT id, title, subject FROM items WHERE fold_rev = 0 AND purged_at IS NULL ORDER BY id LIMIT ${c.deps.backfillItems ?? BACKFILL_ITEMS}`)
    .all<{ id: string; title: string; subject: string }>();
  if (items.results.length > 0) {
    // Progress (round 17 A2): what is left, so the operator sees the window close.
    const left = await c.db.prepare(`SELECT COUNT(*) AS n FROM items WHERE fold_rev = 0 AND purged_at IS NULL`).first<{ n: number }>();
    console.log(`pool cron: backfill ${items.results.length} item(s) this run, ${left?.n ?? 0} waiting`);
  }
  // Round 13: one item at a time, its bodies' memory reserved first; an item that fails is marked
  // (fold_rev 2) and flagged for the admin, never retried forever.
  // Round 18 (critic23d V5): the window stays visible: open for more than a day, the admin is alerted.
  if (items.results.length > 0) {
    await c.db.prepare(`INSERT OR IGNORE INTO pool_kv (key, value) VALUES ('migration_open_at', ?1)`).bind(String(c.now())).run();
    const open = await c.db.prepare(`SELECT value FROM pool_kv WHERE key = 'migration_open_at'`).first<{ value: string }>();
    if (open && c.now() - Number(open.value) > DAY_MS) {
      const f = await c.db
        .prepare(`INSERT OR IGNORE INTO admin_flags (id, kind, detail, dedup, at) VALUES (?1, 'migration_stalled', ?2, ?3, ?4)`)
        .bind(newId(), JSON.stringify({ open_since: Number(open.value) }), `migration_stalled:${Math.floor(c.now() / DAY_MS)}`, c.now())
        .run();
      if ((f.meta.changes ?? 0) === 1) await alertBudgeted(c, { kind: "cron_failure", stage: "migration window open for more than a day" });
    }
  } else await c.db.prepare(`DELETE FROM pool_kv WHERE key = 'migration_open_at'`).run();
  for (const it of items.results) {
    try {
      await backfillItem(c, it);
    } catch (e) {
      // An item whose bodies can never fit the isolate's budget is skipped and flagged (never a gate
      // that stays closed forever); a merely busy isolate stops here, the next run takes it again.
      if (e instanceof BackfillTooLarge) {
        await c.db.batch([
          c.db.prepare(`UPDATE items SET fold_rev = 2 WHERE id = ?1`).bind(it.id),
          c.db.prepare(`INSERT OR IGNORE INTO admin_flags (id, kind, item_id, detail, dedup, at) VALUES (?1, 'backfill_skipped', ?2, ?3, ?4, ?5)`).bind(newId(), it.id, JSON.stringify({ bytes: e.bytes }), `backfill_skipped:${it.id}`, c.now()),
        ]);
        continue;
      }
      if (e instanceof PoolError && e.status === 503) break;
      console.error(`pool cron: backfill of ${it.id} failed: ${e instanceof Error ? e.message : String(e)}`);
      await c.db.batch([
        c.db.prepare(`UPDATE items SET fold_rev = 2 WHERE id = ?1`).bind(it.id),
        c.db.prepare(`INSERT OR IGNORE INTO admin_flags (id, kind, item_id, detail, dedup, at) VALUES (?1, 'cron_failure', ?2, ?3, ?4, ?5)`).bind(newId(), it.id, JSON.stringify({ stage: "backfill" }), `backfill:${it.id}`, c.now()),
      ]);
    }
  }
}

/** The served image of a question (its media reference's hash), as a publish reads it. */
const MEDIA_RE = /^media\/([0-9a-f]{64})\.(png|jpg|gif|webp)$/;
const imgOfQ = (q: QuestionJson) => (q.image && typeof q.image === "object" && typeof q.image.file === "string" ? MEDIA_RE.exec(q.image.file)?.[1] ?? null : null);

class BackfillTooLarge extends Error {
  readonly bytes: number;
  constructor(bytes: number) {
    super("backfill: bodies larger than the isolate budget");
    this.bytes = bytes;
  }
}

async function backfillItem(c: Ctx, it: { id: string; title: string; subject: string }): Promise<void> {
  {
    const pubs = await c.db
      .prepare(`SELECT p.publish_id, p.full_key, p.stripped_key, p.hide_key, p.full_bytes, p.stripped_bytes FROM item_publishes p WHERE p.item_id = ?1 AND p.status IN ('current', 'previous') ORDER BY p.publish_id`)
      .bind(it.id)
      .all<{ publish_id: string; full_key: string; stripped_key: string | null; hide_key: number; full_bytes: number; stripped_bytes: number }>();
    const stmts: D1PreparedStatement[] = [c.db.prepare(`UPDATE items SET title_fold = ?2, subject_fold = ?3, fold_rev = 1 WHERE id = ?1`).bind(it.id, titleSearch(it.title), fold(it.subject))];
    for (const p of pubs.results) {
      // The same reservation as the publish that wrote it (its stripped form is no larger than the full).
      const need = Math.max(p.full_bytes, p.stripped_bytes) * PUBLISH_COST_PER_BYTE;
      if (need > ISOLATE_BYTES_BUDGET) throw new BackfillTooLarge(need);
      const release = reserveBytes(need);
      try {
      const parse = async (key: string | null): Promise<QuestionJson[] | null> => {
        if (!key) return null;
        const o = await c.r2.get(key);
        if (!o) return null;
        try {
          return questionsOf(JSON.parse(await o.text()) as PayloadJson); // r2-object: a stored body
        } catch {
          return null;
        }
      };
      const full = await parse(p.full_key);
      if (!full) continue;
      const served = p.hide_key ? ((await parse(p.stripped_key)) ?? null) : full;
      if (!served || served.length !== full.length) continue;
      const tags = (await c.db.prepare(`SELECT tag FROM item_tags WHERE publish_id = ?1`).bind(p.publish_id).all<{ tag: string }>()).results.map((x) => x.tag);
      for (const t of tags) stmts.push(c.db.prepare(`UPDATE item_tags SET fold = ?2 WHERE publish_id = ?1 AND tag = ?3`).bind(p.publish_id, searchKey(t), t));
      const words = [...new Set([...servedWords(served), ...titleSearch(it.title).split(" "), ...searchKey(it.subject).split(" "), ...tags.flatMap((t) => searchKey(t).split(" "))].filter((x) => x.length > 0 && x.length <= 40))];
      stmts.push(c.db.prepare(`INSERT OR IGNORE INTO served_words (publish_id, word) SELECT ?1, value FROM json_each(?2)`).bind(p.publish_id, JSON.stringify(words)));
      // Every key of the SERVED form, keyed like a publish computes it (round 13).
      const rows = await Promise.all(
        served.map(async (sq, i) => {
          const f = await fieldKeys(sq);
          return { pos: i, kn: await normSkeletonKey(sq), ks: await stemKey(sq), kf: `f:${await nearHash(sq)}`, ke: f.expl, ka: f.ans, bh: await nearHash({ ...(sq as object), stem: "" } as never) };
        }),
      );
      stmts.push(
        c.db
          .prepare(
            `UPDATE item_questions SET bk_norm = j.value ->> '$.kn', bk_stem = j.value ->> '$.ks', bk_full = j.value ->> '$.kf', bk_expl = j.value ->> '$.ke', bk_ans = j.value ->> '$.ka', body_hash = j.value ->> '$.bh'
               FROM json_each(?2) j WHERE item_questions.publish_id = ?1 AND item_questions.pos = CAST(j.value ->> '$.pos' AS INTEGER)`,
          )
          .bind(p.publish_id, JSON.stringify(rows)),
      );
      // Round 16 (critic21d B8): rows from before rounds 6-12 also get every licence-sense hash a
      // publish now writes (near, key-free, bare, content, stem words and bands), their Multipart part
      // keys and their public stem bands; then the evidence rows rebuilt from the committed schema
      // (import_questions rev 0) take their hashes from them.
      const mhs = (await c.db.prepare(`SELECT pos, match_hash FROM item_questions WHERE publish_id = ?1`).bind(p.publish_id).all<{ pos: number; match_hash: string }>()).results;
      const mhAt = new Map(mhs.map((x) => [x.pos, x.match_hash]));
      const fc = await c.core.contentHashes(full);
      const sc = p.hide_key ? await c.core.contentHashes(served) : null;
      const lic = await Promise.all(
        served.map(async (sq, i) => {
          const mh = mhAt.get(i) ?? "";
          const tk = stemTokens((sq as { stem?: unknown }).stem);
          return {
            pos: i,
            nf: await nearHash(full[i]!),
            ns: p.hide_key ? await nearHash(sq) : null,
            bf: await nearHash(full[i]!, true),
            bs: p.hide_key ? await nearHash(sq, true) : null,
            nk: await nearKfHash(mh, sq, imgOfQ),
            kc: await keyfreeHash(mh, sq, imgOfQ),
            fc: fc[i],
            sc: sc ? sc[i] : null,
            tk: JSON.stringify(tk),
            bd: JSON.stringify(stemBands(tk)),
            bt: JSON.stringify(bodyTokens(sq)),
            // (round 22 B2: the keyed sketches and bands a ghost of it keeps)
            tsk: JSON.stringify(await keyedSet(tk)),
            bsk: JSON.stringify(await keyedBody(bodyTokens(sq))),
            kb: JSON.stringify(await keyedBands(stemBands(tk))),
          };
        }),
      );
      stmts.push(
        c.db
          .prepare(
            `UPDATE item_questions SET near_full = j.value ->> '$.nf', near_stripped = j.value ->> '$.ns', near_bare_full = j.value ->> '$.bf', near_bare_stripped = j.value ->> '$.bs',
                    near_kf = j.value ->> '$.nk', keyfree_content = j.value ->> '$.kc', full_content = j.value ->> '$.fc', stripped_content = COALESCE(j.value ->> '$.sc', stripped_content),
                    stem_tokens = j.value ->> '$.tk', stem_bands = j.value ->> '$.bd', body_tokens = j.value ->> '$.bt',
                    tok_sk = j.value ->> '$.tsk', body_sk = j.value ->> '$.bsk', kbands = j.value ->> '$.kb'
               FROM json_each(?2) j WHERE item_questions.publish_id = ?1 AND item_questions.pos = CAST(j.value ->> '$.pos' AS INTEGER)`,
          )
          .bind(p.publish_id, JSON.stringify(lic)),
      );
      const pk = (await Promise.all(served.map(async (sq, pos) => [...(await partKeysOf(sq)), ...(await unitKeysOf(sq))].map((x) => ({ pos, part: x.part, key: x.key }))))).flat();
      if (pk.length > 0)
        stmts.push(c.db.prepare(`INSERT OR IGNORE INTO item_part_keys (publish_id, pos, part, key) SELECT ?1, j.value ->> '$.pos', j.value ->> '$.part', j.value ->> '$.key' FROM json_each(?2) j`).bind(p.publish_id, JSON.stringify(pk)));
      stmts.push(
        c.db
          .prepare(`INSERT OR IGNORE INTO stem_bands (user_id, band, item_id, qid) SELECT 'pub', b.value, ?2, iq.qid FROM item_questions iq, json_each(COALESCE(iq.stem_bands, '[]')) b WHERE iq.publish_id = ?1`)
          .bind(p.publish_id, it.id),
        // (Round 19: evidence is read through the served publish's own rows: nothing to repair.)
        // Round 17 (critic22e A2): the author's durable records, as a publish writes them (served
        // contents, key-free contents, shown full contents, first publishes, used units), so the
        // original author of a 0102-era question stays "first" after the migration.
        // (Round 26, critic-moodle-33c B-3: never an erased account's records: erasure deleted them.)
        c.db
          .prepare(
            `INSERT INTO author_contents (author_id, match_hash, kind, hash, first_seq)
             SELECT p.author_id, iq.match_hash, 'nshown', CASE WHEN p.hide_key = 1 THEN iq.near_stripped ELSE iq.near_full END, iq.seq
               FROM item_questions iq JOIN item_publishes p ON p.publish_id = iq.publish_id
              WHERE iq.publish_id = ?1 AND NOT EXISTS (SELECT 1 FROM users du WHERE du.id = p.author_id AND du.deleted_at IS NOT NULL) AND CASE WHEN p.hide_key = 1 THEN iq.near_stripped ELSE iq.near_full END IS NOT NULL
             UNION ALL SELECT p.author_id, iq.match_hash, 'kf', iq.near_kf, iq.seq FROM item_questions iq JOIN item_publishes p ON p.publish_id = iq.publish_id WHERE iq.publish_id = ?1 AND NOT EXISTS (SELECT 1 FROM users du WHERE du.id = p.author_id AND du.deleted_at IS NOT NULL) AND iq.near_kf IS NOT NULL
             UNION ALL SELECT p.author_id, iq.match_hash, 'shown', iq.full_content, iq.seq FROM item_questions iq JOIN item_publishes p ON p.publish_id = iq.publish_id WHERE iq.publish_id = ?1 AND NOT EXISTS (SELECT 1 FROM users du WHERE du.id = p.author_id AND du.deleted_at IS NOT NULL) AND p.hide_key = 0
             ON CONFLICT (author_id, kind, hash, match_hash) DO UPDATE SET first_seq = MIN(first_seq, excluded.first_seq)`,
          )
          .bind(p.publish_id),
        c.db
          .prepare(
            `INSERT INTO author_firsts (author_id, match_hash, first_seq)
             SELECT p.author_id, iq.match_hash, MIN(iq.seq) FROM item_questions iq JOIN item_publishes p ON p.publish_id = iq.publish_id WHERE iq.publish_id = ?1 AND NOT EXISTS (SELECT 1 FROM users du WHERE du.id = p.author_id AND du.deleted_at IS NOT NULL) GROUP BY p.author_id, iq.match_hash
             ON CONFLICT (author_id, match_hash) DO UPDATE SET first_seq = MIN(first_seq, excluded.first_seq)`,
          )
          .bind(p.publish_id),
        c.db
          .prepare(
            `INSERT INTO author_contents (author_id, match_hash, kind, hash, first_seq)
             SELECT p.author_id, '', 'unit', k.value, MIN(COALESCE(json_extract(iq.unit_seqs, '$."' || k.value || '"'), iq.seq)) FROM item_questions iq JOIN item_publishes p ON p.publish_id = iq.publish_id, json_each(json_array(iq.bk_expl, iq.bk_ans)) k
              WHERE iq.publish_id = ?1 AND NOT EXISTS (SELECT 1 FROM users du WHERE du.id = p.author_id AND du.deleted_at IS NOT NULL) AND k.value IS NOT NULL GROUP BY p.author_id, k.value
             UNION ALL SELECT p.author_id, '', 'unit', pk.key, MIN(iq.seq) FROM item_part_keys pk JOIN item_questions iq ON iq.publish_id = pk.publish_id AND iq.pos = pk.pos JOIN item_publishes p ON p.publish_id = pk.publish_id
              WHERE pk.publish_id = ?1 AND NOT EXISTS (SELECT 1 FROM users du WHERE du.id = p.author_id AND du.deleted_at IS NOT NULL) AND pk.key LIKE 'u:%' GROUP BY p.author_id, pk.key
             UNION ALL SELECT p.author_id, '', 'unit', ib.sha256, MIN(ib.seq) FROM item_blobs ib JOIN item_publishes p ON p.publish_id = ib.publish_id
              WHERE ib.publish_id = ?1 AND NOT EXISTS (SELECT 1 FROM users du WHERE du.id = p.author_id AND du.deleted_at IS NOT NULL) AND ib.form = CASE WHEN p.hide_key = 1 THEN 'stripped' ELSE 'full' END GROUP BY p.author_id, ib.sha256
             ON CONFLICT (author_id, kind, hash, match_hash) DO UPDATE SET first_seq = MIN(first_seq, excluded.first_seq)`,
          )
          .bind(p.publish_id),
      );
      } finally {
        release();
      }
    }
    await c.db.batch(stmts);
  }
}

/** Round 23: items whose open licence rows disagree with what they offer now (bounded per run). */
export const OFFER_SYNC_ITEMS = 200;
async function offerReconcile(c: Ctx): Promise<number> {
  const r = await c.db
    .prepare(
      `SELECT DISTINCT item_id FROM (
         SELECT cl.item_id FROM content_licences cl JOIN items i ON i.id = cl.item_id WHERE cl.until_at IS NULL
            AND NOT (i.visibility = 'public' AND i.removed_at IS NULL AND i.purged_at IS NULL AND NOT EXISTS (SELECT 1 FROM item_holds oh WHERE oh.item_id = i.id))
         UNION ALL
         SELECT o.item_id FROM item_offer o JOIN items i ON i.id = o.item_id
          WHERE i.visibility = 'public' AND i.removed_at IS NULL AND i.purged_at IS NULL AND NOT EXISTS (SELECT 1 FROM item_holds oh WHERE oh.item_id = i.id)
            AND NOT EXISTS (SELECT 1 FROM content_licences cl WHERE cl.item_id = o.item_id AND cl.ckey = o.ckey AND cl.license = i.license AND cl.until_at IS NULL))
       LIMIT ${OFFER_SYNC_ITEMS}`,
    )
    .all<{ item_id: string }>();
  for (const x of r.results) await c.db.batch(offerSync(c.db, x.item_id, c.now()));
  return r.results.length;
}
/** Closed licence rows no serve can reach (every serve of the item is after it closed, or none): collected. */
export const CL_GC_ROWS = 5000;
/** Rows past which the admin is told (the table is bounded by contents x licences, so this means growth). */
export const CL_ALERT_ROWS = 5_000_000;
async function contentLicencesGc(c: Ctx): Promise<number> {
  const del = await c.db
    .prepare(
      `DELETE FROM content_licences WHERE rowid IN (SELECT cl.rowid FROM content_licences cl WHERE cl.until_at IS NOT NULL
          AND NOT EXISTS (SELECT 1 FROM served s WHERE s.item_id = cl.item_id AND s.first_at <= cl.until_at) LIMIT ${CL_GC_ROWS})`,
    )
    .run();
  const n = (await c.db.prepare(`SELECT COUNT(*) AS n FROM (SELECT 1 FROM content_licences LIMIT ${CL_ALERT_ROWS + 1})`).first<{ n: number }>())?.n ?? 0;
  if (n > CL_ALERT_ROWS) {
    const day = Math.floor(c.now() / DAY_MS);
    await c.db
      .prepare(`INSERT OR IGNORE INTO admin_flags (id, kind, detail, dedup, at) VALUES (?1, 'd1_size', ?2, ?3, ?4)`)
      .bind(newId(), JSON.stringify({ table: "content_licences", rows: n }), `content_licences:${day}`, c.now())
      .run();
  }
  return del.meta.changes ?? 0;
}
