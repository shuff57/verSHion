// Author actions (visibility, delete with 14-day undo), reports, the counter-notice, and the
// moderation machinery: holds, takedown with quarantine, put-back, purge (docs/shared-pool.md
// "Reports", "Takedown, DMCA and moderation").
import { offerSync } from "./publish.ts";
import { cleanText } from "../../../auth/src/validate.ts";
import { addBusinessDays } from "./bizdays.ts";
import { AFFECTED_LIST, holdPrivacyBatch, holdSnapshot, releaseOrphanHolds, selectedBlobs, selectedKeyPairs, selectedMatch, type BlockPlan } from "./blocking.ts";
import { deletePublishStatements } from "./publish.ts";
import { itemServesBlob, rightfulExempts, servedBlobsOf, UPSTREAM_SNAPSHOT_SQL, upstreamCarried, upstreamOf } from "./served.ts";
import { licenseCheckOf } from "./licensecheck.ts";
import {
  alertBudgeted,
  audit,
  ALIVE_SQL,
  throwIfErased,
  countMiss,
  SHARED_SQL,
  usage,
  usageSql,
  DAY_MS,
  HOUR_MS,
  ID_RE,
  json,
  limitOf,
  missGuard,
  newId,
  notFound,
  PoolError,
  readJson,
  requireDaily,
  send,
  visibleTo,
  VISIBLE_TO_OTHERS,
  notifyHeld,
  type Ctx,
  type PoolPrincipal,
  qKeyEq,
  qKeyIn,
  inQKeysOf,
  flushOutbox,
  outboxStatement,
} from "./shared.ts";

export const UNDO_MS = 14 * DAY_MS;
/** Privacy-blocked image bytes stay (served to nobody) this long before they are deleted. */
export const PRIVACY_BYTES_GRACE_MS = 14 * DAY_MS;
export const QUARANTINE_MS = 30 * DAY_MS;
/** An open appeal stops the purge (round 9 J5); the admin is reminded after this long ... */
export const APPEAL_REMINDER_MS = 7 * DAY_MS;
/** ... and an appeal still unanswered after this long RESTORES the item (never a silent purge). */
export const APPEAL_MAX_MS = 60 * DAY_MS;
/** The item has an appeal the admin has not decided. */
export const OPEN_APPEAL_SQL = (item: string) => `EXISTS (SELECT 1 FROM admin_flags fa WHERE fa.kind = 'appeal' AND fa.item_id = ${item} AND fa.resolved_at IS NULL)`;
export const STRIKE_WINDOW_MS = 365 * DAY_MS;
export const STRIKES_TO_SUSPEND = 3;
export const INSTANT_HOLDS_PER_DAY = 3;
/** Accounts younger than this cannot hide an item instantly (their reports wait for the admin). */
export const NEW_ACCOUNT_MS = 7 * DAY_MS;
export const THROTTLE_MS = 30 * DAY_MS;
export const REPORT_REASONS = ["wrong_answer", "copyright", "student_data", "inappropriate", "other", "key_leak"] as const;

async function idOrMiss(c: Ctx, id: string, me: string, ownOk = false): Promise<void> {
  // The day's miss lockout never stops an author managing their own items.
  if (ownOk && ID_RE.test(id) && (await c.db.prepare(`SELECT 1 AS x FROM items WHERE id = ?1 AND author_id = ?2`).bind(id, me).first())) return;
  await missGuard(c, me, "item_miss");
  if (!ID_RE.test(id)) {
    await countMiss(c, me, "item_miss");
    throw notFound();
  }
}
async function miss(c: Ctx, me: string): Promise<never> {
  await countMiss(c, me, "item_miss");
  throw notFound();
}

// ---- author actions ---------------------------------------------------------------------------------

/** PATCH /items/:id {visibility}: one UPDATE; public is refused while the current licence is circle-only. */
export async function patchItem(c: Ctx, req: Request, p: PoolPrincipal, id: string): Promise<Response> {
  const me = p.userId;
  await idOrMiss(c, id, me);
  const b = await readJson(req, `u:${me}`, { deadlineMs: c.deps.bodyDeadlineMs });
  const vis = b.visibility;
  if (vis !== "public" && vis !== "unlisted") throw new PoolError(400, "invalid_request", { field: "visibility" });
  const now = c.now();
  const [upd] = await c.db.batch([
    c.db
      .prepare(
        `UPDATE items SET visibility = ?3, updated_at = ?4 WHERE id = ?1 AND author_id = ?2 AND removed_at IS NULL AND current_publish_id IS NOT NULL
           AND (?3 = 'unlisted' OR license <> 'circle-only')`,
      )
      .bind(id, me, vis, now),
    audit(c.db, { actorId: me, actorKind: "user", action: "item.visibility", itemId: id, targetUserId: me, detail: { visibility: vis } }, now, `changes() = 1`),
  ]);
  if ((upd!.meta.changes ?? 0) === 1) {
    // Round 14 (B4): a visibility change re-runs the block checks on what the item now serves.
    if (vis === "public") await recheckBlocks(c, id);
    // (Round 23, critic30a B4: unlisted offers nothing: its licence history closes; public re-opens it.)
    await c.db.batch(offerSync(c.db, id, now));
    return json(200, { ok: true, visibility: vis });
  }
  const own = await c.db.prepare(`SELECT license, removed_at FROM items WHERE id = ?1 AND author_id = ?2`).bind(id, me).first<{ license: string; removed_at: number | null }>();
  if (own && own.removed_at === null && own.license === "circle-only") throw new PoolError(422, "license_not_public");
  if (own && own.removed_at !== null) throw new PoolError(409, "item_removed");
  return miss(c, me);
}

/**
 * SQL: open key_leak report `r` (reporter `ru`) on item `item`, publish `pub`, QUALIFIES at time `now`
 * (round 18; round 19 critic24d D2: the one predicate a hold is placed and kept by): a teacher in good
 * standing, approved at least 7 days, served that publish at least a day before reporting, no report of
 * theirs dismissed on the item in 90 days.
 */
export const keyLeakQualifies = (item: string, pub: string, now: string) =>
  `(r.item_id = ${item} AND r.publish_id = ${pub} AND r.reason = 'key_leak' AND r.resolved_at IS NULL
    AND ru.trust IN ('teacher', 'admin') AND ru.deleted_at IS NULL AND ru.suspended_at IS NULL
    AND COALESCE((SELECT MAX(a.decided_at) FROM approval_requests a WHERE a.user_id = r.reporter_id AND a.decision = 'approved'), ru.created_at) <= ${now} - ${7 * DAY_MS}
    AND EXISTS (SELECT 1 FROM imports im WHERE im.user_id = r.reporter_id AND im.item_id = ${item} AND ${pub} IN (im.publish_id, im.real_publish_id) AND COALESCE(im.first_at, im.at) <= r.at - ${DAY_MS})
    AND NOT EXISTS (SELECT 1 FROM reports d WHERE d.item_id = ${item} AND d.reporter_id = r.reporter_id AND d.reason = 'key_leak' AND d.action = 'dismissed' AND d.resolved_at > ${now} - ${90 * DAY_MS}))`;

/** key_leak reports an account may file a day, and holds one reporter may take part in a day (round 18). */
export const KEY_LEAK_REPORTS_PER_DAY = 5;
export const KEY_LEAK_HOLDS_PER_DAY = 3;

/** How long a deleted held item's keys keep holding the same content (critic21c B4). */
export const CARRY_KEYS_MS = 90 * DAY_MS;

/** DELETE /items/:id: hidden at once, purged after 14 days unless undone. */
export async function deleteItem(c: Ctx, p: PoolPrincipal, id: string): Promise<Response> {
  const me = p.userId;
  await idOrMiss(c, id, me, true);
  const now = c.now();
  const [upd] = await c.db.batch([
    c.db
      .prepare(`UPDATE items SET removed_at = ?3, removed_reason = 'author_deleted', purge_after = ?4, updated_at = ?3 WHERE id = ?1 AND author_id = ?2 AND removed_at IS NULL`)
      .bind(id, me, now, now + UNDO_MS),
    audit(c.db, { actorId: me, actorKind: "user", action: "item.delete", itemId: id, targetUserId: me }, now, `changes() = 1`),
    // Round 16 (critic21c B4): a held item's keys outlive its deletion and purge (hash-only), so the
    // same content published again is held until the reports are resolved (or CARRY_KEYS_MS passes).
    // Round 19 (critic24c variant c): a key_leak hold too (the same served content with the same key).
    c.db
      .prepare(
        `INSERT OR IGNORE INTO carried_keys (author_id, item_id, reason, key, until)
         SELECT ?2, ?1, h.reason, k.value, ?3 FROM item_holds h JOIN items i ON i.id = h.item_id
           JOIN item_questions iq ON iq.publish_id = i.current_publish_id, json_each(json_array(iq.match_hash, iq.bk_norm, iq.bk_full)) k
          WHERE h.item_id = ?1 AND i.author_id = ?2 AND h.source IN ('', 'manual', 'key_leak') AND k.value IS NOT NULL
            AND (h.by IS NOT NULL OR (SELECT COUNT(DISTINCT r.reporter_id) FROM reports r WHERE r.item_id = h.item_id AND r.resolved_at IS NULL) >= 2)`,
      )
      .bind(id, me, now + CARRY_KEYS_MS),
  ]);
  if ((upd!.meta.changes ?? 0) === 1) return json(200, { ok: true, undo_until: now + UNDO_MS });
  const own = await c.db.prepare(`SELECT removed_reason, purge_after FROM items WHERE id = ?1 AND author_id = ?2 AND purged_at IS NULL`).bind(id, me).first<{ removed_reason: string | null; purge_after: number | null }>();
  // Already deleted: the same answer (the window is never extended). Privacy and DMCA removals
  // follow their own rules: deleting does not start or shorten anything.
  if (own?.removed_reason === "author_deleted") return json(200, { ok: true, undo_until: own.purge_after });
  if (own) throw new PoolError(409, "under_moderation");
  return miss(c, me);
}

/** POST /items/:id/undelete: only an author deletion, only within its 14 days. */
export async function undeleteItem(c: Ctx, p: PoolPrincipal, id: string): Promise<Response> {
  const me = p.userId;
  await idOrMiss(c, id, me, true);
  const now = c.now();
  const quota = await limitOf(c, me, "storage_bytes");
  const cap = await limitOf(c, me, "shared_questions");
  // Storage quota and the shared-question cap are part of the same UPDATE (critic A6): undoing a
  // deletion cannot take an account past either.
  const [upd] = await c.db.batch([
    c.db
      .prepare(
        `UPDATE items SET removed_at = NULL, removed_reason = NULL, purge_after = NULL, updated_at = ?2
          WHERE id = ?3 AND author_id = ?1 AND removed_reason = 'author_deleted' AND purge_after > ?2 AND purged_at IS NULL
            AND ${usageSql("?3")} <= ?4
            AND ${SHARED_SQL("?1", "?3")} + question_count <= ?5`,
      )
      .bind(me, now, id, quota, cap),
    audit(c.db, { actorId: me, actorKind: "user", action: "item.undelete", itemId: id, targetUserId: me }, now, `changes() = 1`),
  ]);
  if ((upd!.meta.changes ?? 0) === 1) return json(200, { ok: true });
  const it = await c.db
    .prepare(`SELECT question_count FROM items WHERE id = ?1 AND author_id = ?2 AND removed_reason = 'author_deleted' AND purge_after > ?3 AND purged_at IS NULL`)
    .bind(id, me, now)
    .first<{ question_count: number }>();
  if (!it) return miss(c, me);
  const after = await c.db.prepare(`SELECT ${usageSql("?3")} AS used, ${SHARED_SQL("?1", "?3")} AS shared`).bind(me, now, id).first<{ used: number; shared: number }>();
  if ((after?.used ?? 0) > quota) throw new PoolError(413, "quota_exceeded", { used: await usage(c, me), needed: (after?.used ?? 0) - (await usage(c, me)), limit: quota });
  throw new PoolError(422, "shared_questions_limit", { used: after?.shared ?? 0, needed: it.question_count, limit: cap });
}

// ---- reports ----------------------------------------------------------------------------------------

/** POST /items/:id/report: the reporter only ever hears "thanks". */
export async function report(c: Ctx, req: Request, p: PoolPrincipal, id: string): Promise<Response> {
  const me = p.userId;
  await idOrMiss(c, id, me);
  const b = await readJson(req, `u:${me}`, { deadlineMs: c.deps.bodyDeadlineMs });
  const reason = b.reason;
  if (typeof reason !== "string" || !(REPORT_REASONS as readonly string[]).includes(reason)) throw new PoolError(400, "invalid_request", { field: "reason" });
  let note: string | null = null;
  if (b.note !== undefined && b.note !== null && b.note !== "") {
    note = cleanText(b.note, 1000);
    if (!note) throw new PoolError(400, "invalid_request", { field: "note" });
  }
  const it = await c.db
    .prepare(`SELECT i.id, i.author_id, i.current_publish_id FROM items i JOIN users u ON u.id = i.author_id WHERE i.id = ?1 AND ${visibleTo("?2")}`)
    .bind(id, me)
    .first<{ id: string; author_id: string; current_publish_id: string }>();
  if (!it) return miss(c, me);
  if (it.author_id === me) throw new PoolError(400, "own_item");
  await requireDaily(c, me, "reports");
  const now = c.now();
  const rid = newId();
  const threshold = await limitOf(c, "*", "report_threshold");
  const stmts: D1PreparedStatement[] = [
    c.db
      // Round 27 (critic35d B-1): the reporter must still stand, in this very statement (and in each one below).
      .prepare(`INSERT OR IGNORE INTO reports (id, item_id, publish_id, reporter_id, reason, note, at) SELECT ?1, ?2, ?3, ?4, ?5, ?6, ?7 WHERE ${ALIVE_SQL("?4")}`)
      .bind(rid, id, it.current_publish_id, me, reason, note, now),
  ];
  if (reason === "student_data")
    // One report hides the item at once, but one reporter can do that to at most
    // INSTANT_HOLDS_PER_DAY items a day, and not at all within THROTTLE_MS of having a student_data
    // report dismissed anywhere (the report is still recorded for the admin).
    stmts.push(
      c.db
        .prepare(
          `INSERT OR IGNORE INTO item_holds (item_id, reason, at, by) SELECT ?1, 'student_data', ?2, NULL
            WHERE NOT EXISTS (SELECT 1 FROM reports WHERE reporter_id = ?3 AND reason = 'student_data' AND action = 'dismissed' AND resolved_at > ?2 - ?4)
              AND (SELECT COUNT(*) FROM item_holds h WHERE h.reason = 'student_data' AND h.source = '' AND h.at >= ?5
                     AND EXISTS (SELECT 1 FROM reports r WHERE r.item_id = h.item_id AND r.reporter_id = ?3 AND r.reason = 'student_data' AND r.at = h.at)) < ?6
              -- a site-wide daily cap, and no instant holds from accounts younger than NEW_ACCOUNT_MS:
              -- those reports wait for the admin
              AND (SELECT COUNT(*) FROM item_holds h WHERE h.reason = 'student_data' AND h.source = '' AND h.at >= ?5) < ?7
              -- past half the site cap, an instant hold needs two distinct reporters
              AND ((SELECT COUNT(*) FROM item_holds h WHERE h.reason = 'student_data' AND h.source = '' AND h.at >= ?5) < ?7 / 2
                   OR (SELECT COUNT(DISTINCT reporter_id) FROM reports WHERE item_id = ?1 AND reason = 'student_data' AND resolved_at IS NULL) >= 2)
              AND (SELECT created_at FROM users WHERE id = ?3) <= ?2 - ?8 AND ${ALIVE_SQL("?3")}
              -- a reporter with some history in the pool (an import or a published item), never a bare account (round 8)
              AND (EXISTS (SELECT 1 FROM imports im WHERE im.user_id = ?3 AND im.kind = 'import') OR EXISTS (SELECT 1 FROM items it WHERE it.author_id = ?3))`,
        )
        .bind(id, now, me, THROTTLE_MS, Math.floor(now / DAY_MS) * DAY_MS, INSTANT_HOLDS_PER_DAY, await limitOf(c, "*", "instant_holds_global"), NEW_ACCOUNT_MS),
    );
  else
    // 3 distinct teacher reporters (open, non-privacy reasons) hide it pending review.
    stmts.push(
      c.db
        .prepare(
          `INSERT OR IGNORE INTO item_holds (item_id, reason, at, by)
           SELECT ?1, 'reports', ?2, NULL WHERE (SELECT COUNT(DISTINCT r.reporter_id) FROM reports r JOIN users ru ON ru.id = r.reporter_id
             WHERE r.item_id = ?1 AND r.resolved_at IS NULL AND r.reason NOT IN ('student_data', 'key_leak') AND ru.trust IN ('teacher', 'admin') AND ru.deleted_at IS NULL AND ru.suspended_at IS NULL
               -- after an admin release, the same reporters cannot re-hide it for THROTTLE_MS
               AND NOT EXISTS (SELECT 1 FROM reports d WHERE d.item_id = ?1 AND d.reporter_id = r.reporter_id AND d.action = 'dismissed' AND d.resolved_at > ?2 - ?4)) >= ?3
             AND ${ALIVE_SQL("?5")}`,
        )
        .bind(id, now, threshold, THROTTLE_MS, me),
    );
  if (reason === "key_leak") {
    // Round 18 (critic23c U4 / 23d): a key_leak report is only for a HIDDEN-key publish, and counts
    // only from a qualifying reporter: one served THAT publish (an import or full read recorded at least
    // a day before), approved at least 7 days, at most one such report per item per 30 days and 5 a day,
    // never within 90 days of having one dismissed on this item.
    const ok = await c.db
      .prepare(
        `SELECT
           EXISTS (SELECT 1 FROM item_publishes p WHERE p.publish_id = ?3 AND p.hide_key = 1) AS hidden,
           EXISTS (SELECT 1 FROM imports im WHERE im.user_id = ?1 AND im.item_id = ?2 AND ?3 IN (im.publish_id, im.real_publish_id) AND COALESCE(im.first_at, im.at) <= ?4 - ${DAY_MS}) AS served,
           COALESCE((SELECT MAX(a.decided_at) FROM approval_requests a WHERE a.user_id = ?1 AND a.decision = 'approved'), (SELECT u.created_at FROM users u WHERE u.id = ?1)) <= ?4 - ${7 * DAY_MS} AS seasoned,
           (SELECT COUNT(*) FROM reports r WHERE r.reporter_id = ?1 AND r.item_id = ?2 AND r.reason = 'key_leak' AND r.at > ?4 - ${30 * DAY_MS}) AS recent_item,
           (SELECT COUNT(*) FROM reports r WHERE r.reporter_id = ?1 AND r.reason = 'key_leak' AND r.at > ?4 - ${DAY_MS}) AS today`,
      )
      .bind(me, id, it.current_publish_id, now)
      .first<{ hidden: number; served: number; seasoned: number; recent_item: number; today: number }>();
    if (!ok || ok.hidden !== 1) throw new PoolError(400, "invalid_request", { field: "reason", message: "This item shows its answer key: report a wrong answer instead." });
    if (ok.recent_item > 0 || ok.today >= KEY_LEAK_REPORTS_PER_DAY) throw new PoolError(429, "daily_limit", { metric: "key_leak_reports" });
    // (A report that does not qualify is still recorded for the admin; it never counts toward a hold.)
    if (ok.served === 1 && ok.seasoned === 1)
      stmts.push(
        c.db
          .prepare(
            `INSERT OR IGNORE INTO item_holds (item_id, reason, at, by, source, detail)
             SELECT ?1, 'reports', ?2, NULL, 'key_leak', json_object('publish_id', ?3)
              WHERE (SELECT COUNT(DISTINCT r.reporter_id) FROM reports r JOIN users ru ON ru.id = r.reporter_id
                      WHERE ${keyLeakQualifies("?1", "?3", "?2")}
                        -- a reporter takes part in at most KEY_LEAK_HOLDS_PER_DAY holds a day
                        AND (SELECT COUNT(*) FROM item_holds kh JOIN reports kr ON kr.item_id = kh.item_id AND kr.reporter_id = r.reporter_id AND kr.reason = 'key_leak'
                              WHERE kh.source = 'key_leak' AND kh.at > ?2 - ${DAY_MS}) < ${KEY_LEAK_HOLDS_PER_DAY}) >= 2
                AND ${ALIVE_SQL("?4")}`,
          )
          .bind(id, now, it.current_publish_id, me),
      );
    stmts.push(
      c.db
        .prepare(`INSERT OR IGNORE INTO admin_flags (id, kind, item_id, publish_id, detail, dedup, at) SELECT ?1, 'key_leak', ?2, ?3, ?4, ?5, ?6 WHERE ${ALIVE_SQL("?7")}`)
        .bind(newId(), id, it.current_publish_id, JSON.stringify({ report_id: rid, qualifying: ok.served === 1 && ok.seasoned === 1 }), `key_leak:${id}:${it.current_publish_id}`, now, me),
    );
  }
  stmts.push(audit(c.db, { actorId: me, actorKind: "user", action: "item.report", itemId: id, detail: { reason, report_id: rid } }, now, ALIVE_SQL("?3")));
  const res = await c.db.batch(stmts);
  // (A report whose reporter was erased meanwhile wrote nothing: the answer an unauthenticated request gets.)
  if ((res[0]!.meta.changes ?? 0) === 0) await throwIfErased(c, me);
  // The author is told (budgeted like every notice): what an importer reported, never who.
  if (reason === "key_leak" && (res[0]!.meta.changes ?? 0) === 1) {
    await alertBudgeted(c, { kind: "key_leak_report", itemId: id, authorId: it.author_id });
    // The admin queue entry carries what students are served (the stems and choices, at most 4 KB).
    const pub = await c.db.prepare(`SELECT stripped_key, full_key, hide_key FROM item_publishes WHERE publish_id = ?1`).bind(it.current_publish_id).first<{ stripped_key: string | null; full_key: string; hide_key: number }>();
    const o = pub ? await c.r2.get(pub.hide_key ? (pub.stripped_key ?? pub.full_key) : pub.full_key) : null;
    if (o) {
      let served = "";
      try {
        served = (JSON.parse(await o.text()) as { bank?: { questions?: { stem?: string; kind?: Record<string, { answers?: { text?: string }[] }> }[] } }).bank?.questions // r2-object: the served body
          ?.map((q) => [q.stem ?? "", ...Object.values(q.kind ?? {}).flatMap((k) => (k?.answers ?? []).map((a) => `- ${a.text ?? ""}`))].join("\n"))
          .join("\n\n")
          .slice(0, 4096) ?? "";
      } catch {
        served = "";
      }
      await c.db
        .prepare(`UPDATE admin_flags SET detail = json_set(COALESCE(detail, '{}'), '$.served', ?2) WHERE dedup = ?1`)
        .bind(`key_leak:${id}:${it.current_publish_id}`, served)
        .run();
    }
  }
  // student_data: the admin is mailed immediately, outside the notice budget.
  // Admin mail for an instant hold goes through the pool's hourly alert budget (the hold and the flag do not).
  if (reason === "student_data" && (res[1]!.meta.changes ?? 0) === 1) await alertBudgeted(c, { kind: "student_data_report", itemId: id, reportId: rid });
  if (reason === "student_data" && (res[1]!.meta.changes ?? 0) === 0 && (res[0]!.meta.changes ?? 0) === 1) {
    // The site-wide cap was reached (or the reporter is new / throttled): the admin is told once a day.
    const today = Math.floor(now / DAY_MS);
    const flagged = await c.db
      .prepare(`INSERT OR IGNORE INTO admin_flags (id, kind, item_id, detail, dedup, at) SELECT ?1, 'instant_hold_cap', ?2, ?3, ?4, ?5
                 WHERE (SELECT COUNT(*) FROM item_holds h WHERE h.reason = 'student_data' AND h.source = '' AND h.at >= ?6) >= ?7`)
      .bind(newId(), id, JSON.stringify({ report_id: rid }), `instant_hold_cap:${today}`, now, today * DAY_MS, await limitOf(c, "*", "instant_holds_global"))
      .run();
    if ((flagged.meta.changes ?? 0) === 1) await alertBudgeted(c, { kind: "student_data_report", itemId: id, reportId: rid });
  }
  return json(202, { ok: true, message: "Thanks, the admin will review it." });
}

// ---- appeal ---------------------------------------------------------------------------------------

/**
 * POST /items/:id/appeal {note}: the author of an item an admin removed or a hold hides asks for a
 * review. It raises an admin flag (at most one a day per item) and says nothing else.
 */
export async function appeal(c: Ctx, req: Request, p: PoolPrincipal, id: string): Promise<Response> {
  const me = p.userId;
  await idOrMiss(c, id, me, true);
  const b = await readJson(req, `u:${me}`, { deadlineMs: c.deps.bodyDeadlineMs });
  const n = cleanText(b.note, 500);
  if (!n) throw new PoolError(400, "invalid_request", { field: "note" });
  // A privacy purge is appealable for PRIVACY_BYTES_GRACE_MS (while an unblock still restores the
  // copies and images); the item itself is gone, so the author republishes it if the appeal succeeds.
  const it = await c.db
    .prepare(
      `SELECT removed_reason, removed_at, appeal_decided_at, (SELECT MAX(h.at) FROM item_holds h WHERE h.item_id = items.id) AS held_at,
              ${OPEN_APPEAL_SQL("items.id")} AS open FROM items
        WHERE id = ?1 AND author_id = ?2 AND (purged_at IS NULL OR (removed_reason = 'privacy' AND removed_at > ?3))`,
    )
    .bind(id, me, c.now() - PRIVACY_BYTES_GRACE_MS)
    .first<{ removed_reason: string | null; removed_at: number | null; appeal_decided_at: number | null; held_at: number | null; open: number }>();
  if (!it) return miss(c, me);
  if (it.removed_reason !== "admin" && it.removed_reason !== "privacy" && it.held_at === null) throw new PoolError(409, "nothing_to_appeal");
  // Round 10 L1: one appeal per removal (or hold), never one per UTC day. Once the admin has decided
  // on this removal, it cannot be appealed again; while one is open, a repeat changes nothing.
  const instance = it.removed_at ?? it.held_at ?? 0;
  if (it.appeal_decided_at !== null && it.appeal_decided_at >= instance)
    throw new PoolError(409, "decided", { message: "The admin has already decided on this. It cannot be appealed again." });
  if (it.open) return json(202, { ok: true, message: "Your appeal is already with the admin." });
  await requireDaily(c, me, "appeals");
  const now = c.now();
  // Round 27 (critic35d B-1): the flag is written only while the appellant stands (same statement; the
  // audit follows it by changes()); an erased account's late appeal would be an OPEN appeal that blocks
  // the purge of its own removed item, so it is refused like an unauthenticated request.
  const [wrote] = await c.db.batch([
    c.db
      .prepare(`INSERT OR IGNORE INTO admin_flags (id, kind, item_id, user_id, detail, dedup, at) SELECT ?1, 'appeal', ?2, ?3, ?4, ?5, ?6 WHERE ${ALIVE_SQL("?3")}`)
      .bind(newId(), id, me, JSON.stringify({ note: n, removed_reason: it.removed_reason, held: it.held_at !== null }), `appeal:${id}:${instance}`, now),
    audit(c.db, { actorId: me, actorKind: "user", action: "item.appeal", itemId: id, targetUserId: me, note: n }, now, `changes() = 1`),
  ]);
  if ((wrote!.meta.changes ?? 0) === 0) await throwIfErased(c, me);
  return json(202, { ok: true, message: "The admin will review your item." });
}

// ---- counter-notice (512(g)(3)) ---------------------------------------------------------------------

/** POST /items/:id/counter-notice: the author of a taken-down item. Incomplete notices are refused. */
export async function counterNotice(c: Ctx, req: Request, p: PoolPrincipal, id: string): Promise<Response> {
  const me = p.userId;
  // The day's miss lockout never blocks an author's counter-notice on their own item.
  await idOrMiss(c, id, me, true);
  const b = await readJson(req, `u:${me}`, { deadlineMs: c.deps.bodyDeadlineMs });
  const action = await c.db
    .prepare(
      `SELECT a.id, a.notice_id FROM dmca_actions a JOIN items i ON i.id = a.item_id
        WHERE a.item_id = ?1 AND a.author_id = ?2 AND i.author_id = ?2 AND i.removed_reason = 'takedown'
          AND a.restored_at IS NULL AND a.purged_at IS NULL AND a.counter_notice_id IS NULL
          AND a.author_notified_at > ?3 - ?4 -- the 30-day window ends with the quarantine
        ORDER BY a.disabled_at DESC LIMIT 1`,
    )
    .bind(id, me, c.now(), QUARANTINE_MS)
    .first<{ id: string; notice_id: string }>();
  if (!action) {
    // The author's own taken-down item whose 30-day window has ended: say so (round 8), not a 404.
    const late = await c.db
      .prepare(`SELECT 1 AS x FROM dmca_actions a JOIN items i ON i.id = a.item_id WHERE a.item_id = ?1 AND a.author_id = ?2 AND i.author_id = ?2 AND a.counter_notice_id IS NULL AND a.restored_at IS NULL`)
      .bind(id, me)
      .first();
    if (late) throw new PoolError(409, "window_closed", { message: "The 30-day counter-notice window for this takedown has ended." });
    return miss(c, me);
  }
  const missing: string[] = [];
  const text = (k: string, max: number) => {
    const v = cleanText(b[k], max);
    if (!v) missing.push(k);
    return v ?? "";
  };
  const tick = (k: string) => {
    if (b[k] !== true) missing.push(k);
  };
  const signature = text("signature", 200);
  const material = text("material_desc", 2000);
  const name = text("name", 200);
  const address = text("address", 500);
  const phone = text("phone", 50);
  tick("perjury_stmt");
  tick("consent_jurisdiction");
  tick("accept_service");
  if (missing.length > 0) throw new PoolError(400, "counter_notice_incomplete", { missing });
  const now = c.now();
  const cid = newId();
  const notBefore = addBusinessDays(now, 10) - DAY_MS + 1; // the start of the 10th business day after receipt
  const by = addBusinessDays(now, 14);
  // Round 23 (critic30b A3): the action takes the counter-notice only if it is still open in the same
  // statement (not upheld, put back or past its window): an uphold that committed meanwhile wins, and
  // the author is told the window closed, never a 202 for an action that ends upheld.
  const res = await c.db.batch([
    c.db
      .prepare(
        `UPDATE dmca_actions SET counter_notice_id = ?2, restore_not_before = ?3, restore_by = ?4
          WHERE id = ?1 AND counter_notice_id IS NULL AND purged_at IS NULL AND restored_at IS NULL AND author_notified_at > ?5 - ?6
            AND ${ALIVE_SQL("?7")}`, // (round 27, critic35d B-1: nothing of an erased sender's counter-notice is written)
      )
      .bind(action.id, cid, notBefore, by, now, QUARANTINE_MS, me),
    c.db
      .prepare(
        `INSERT INTO dmca_counter_notices (id, action_id, received_at, signature, material_desc, perjury_stmt, name, address, phone, consent_jurisdiction, accept_service, forwarded_at)
         SELECT ?1, ?2, ?3, ?4, ?5, 1, ?6, ?7, ?8, 1, 1, ?3 WHERE changes() = 1 AND NOT EXISTS (SELECT 1 FROM dmca_counter_notices WHERE action_id = ?2)`,
      )
      .bind(cid, action.id, now, signature, material, name, address, phone),
    audit(c.db, { actorId: me, actorKind: "user", action: "dmca.counter_notice", itemId: id, targetUserId: me, detail: { action_id: action.id } }, now, `changes() = 1`),
    // Round 10: a counter-notice is an admin task (forward it to the complainant; restore after 10 business days).
    c.db
      .prepare(`INSERT OR IGNORE INTO admin_flags (id, kind, item_id, user_id, detail, dedup, at) SELECT ?1, 'counter_notice', ?2, ?3, ?4, ?5, ?6 WHERE changes() = 1`)
      .bind(newId(), id, me, JSON.stringify({ action_id: action.id, restore_not_before: notBefore }), `counter_notice:${action.id}`, now),
  ]);
  if ((res[0]!.meta.changes ?? 0) !== 1) {
    await throwIfErased(c, me);
    throw new PoolError(409, "window_closed", { message: "The 30-day counter-notice window for this takedown has ended." });
  }
  const notice = await c.db.prepare(`SELECT complainant_contact FROM dmca_notices WHERE id = ?1`).bind(action.notice_id).first<{ complainant_contact: string }>();
  // Forwarded to the complainant at once, with the restore date.
  if (notice) send(c, { kind: "counter_notice", actionId: action.id, complainantContact: notice.complainant_contact, restoreNotBefore: notBefore });
  return json(202, { ok: true, restore_not_before: notBefore, restore_by: by });
}

// ---- machinery --------------------------------------------------------------------------------------

/** Deletes blobs nothing references and nobody has pending (rows first, then the bytes). */
export async function gcBlobs(c: Ctx, only?: string[]): Promise<number> {
  const now = c.now();
  const del = await c.db
    .prepare(
      `DELETE FROM blobs WHERE sha256 IN (
         SELECT b.sha256 FROM blobs b
          WHERE (?1 IS NULL OR b.sha256 IN (SELECT value FROM json_each(?1)))
            AND NOT EXISTS (SELECT 1 FROM item_blobs ib WHERE ib.sha256 = b.sha256)
            AND NOT EXISTS (SELECT 1 FROM pending_blobs pb WHERE pb.sha256 = b.sha256 AND pb.until >= ?2)
          LIMIT 500)
       RETURNING sha256`,
    )
    .bind(only ? JSON.stringify(only) : null, now - HOUR_MS)
    .all<{ sha256: string }>();
  const gone = del.results.map((r) => r.sha256);
  if (gone.length === 0) return 0;
  // An upload that landed in between wrote a live pending row (and re-inserts the blobs row after
  // its R2 put): keep those bytes. An expired pending row protects nothing.
  const back = await c.db
    .prepare(
      `SELECT sha256 FROM pending_blobs WHERE sha256 IN (SELECT value FROM json_each(?1)) AND until >= ?2
       UNION SELECT sha256 FROM blobs WHERE sha256 IN (SELECT value FROM json_each(?1))`,
    )
    .bind(JSON.stringify(gone), now - HOUR_MS)
    .all<{ sha256: string }>();
  const keep = new Set(back.results.map((r) => r.sha256));
  const drop = gone.filter((s) => !keep.has(s));
  if (drop.length > 0) await c.r2.delete(drop.map((s) => `blobs/${s}`));
  return drop.length;
}

/** Purge: every publish's bodies and derived rows; the items row stays as a tombstone. */
export async function purgeItem(c: Ctx, itemId: string, actor: { id: string | null; kind: "admin" | "system" }, why: string): Promise<boolean> {
  const now = c.now();
  // (Round 21 B-3: the publish the copies are compared with: the item's current one, read before the
  // claim clears the pointer, or after a failed run from the publish rows themselves.)
  const before = await c.db
    .prepare(`SELECT COALESCE((SELECT current_publish_id FROM items WHERE id = ?1), (SELECT publish_id FROM item_publishes WHERE item_id = ?1 AND status = 'current')) AS p`)
    .bind(itemId)
    .first<{ p: string | null }>();
  // Claimed first, and only an item whose purge is due: a put-back clears purge_after when it claims
  // its action, so the two can never both proceed (critic A8). Keys are read only after the claim.
  const claim = await c.db
    .prepare(
      // (Round 21, critic27b G1: a purge claimed earlier whose batch failed is claimed again and finished:
      // the claim never strands publishes, bodies or tokens.)
      `UPDATE items SET purged_at = COALESCE(purged_at, ?2), current_publish_id = NULL, prev_publish_id = NULL, purge_after = NULL
        WHERE id = ?1 AND removed_at IS NOT NULL
          AND ((purged_at IS NULL AND purge_after IS NOT NULL AND purge_after <= ?2)
               OR (purged_at IS NOT NULL AND EXISTS (SELECT 1 FROM item_publishes p WHERE p.item_id = items.id)))`,
    )
    .bind(itemId, now)
    .run();
  if ((claim.meta.changes ?? 0) !== 1) return false;
  const pubs = await c.db
    .prepare(`SELECT publish_id, full_key, stripped_key FROM item_publishes WHERE item_id = ?1`)
    .bind(itemId)
    .all<{ publish_id: string; full_key: string; stripped_key: string | null }>();
  const ids = pubs.results.map((r) => r.publish_id);
  const shas = ids.length
    ? (await c.db.prepare(`SELECT DISTINCT sha256 FROM item_blobs WHERE publish_id IN (SELECT value FROM json_each(?1))`).bind(JSON.stringify(ids)).all<{ sha256: string }>()).results.map((r) => r.sha256)
    : [];
  const tomb = await c.db.prepare(`SELECT removed_reason, author_id FROM items WHERE id = ?1`).bind(itemId).first<{ removed_reason: string; author_id: string }>();
  const wasTakedown = tomb?.removed_reason === "takedown";
  // The rightful author's item, if the admin named one on the takedown (round 7 M1).
  const act = wasTakedown
    ? await c.db.prepare(`SELECT id, rightful_item_id FROM dmca_actions WHERE item_id = ?1 AND restored_at IS NULL AND purged_at IS NULL ORDER BY disabled_at DESC LIMIT 1`).bind(itemId).first<{ id: string; rightful_item_id: string | null }>()
    : null;
  // Without one, the copies of the infringing item are held for review rather than left live with
  // no licence or credit at all (their evidence is about to be voided). Computed before the purge.
  // (Round 21 D2: every copy, a page at a time, never only the first.)
  const orphanCopies: { item_id: string }[] = [];
  // (Round 23, critic30a B2: the same rule as a ghost's uphold.)
  const voids = wasTakedown && tomb ? await shouldVoidEvidence(c, tomb.author_id, act?.rightful_item_id ?? null) : false;
  if (wasTakedown && act && !act.rightful_item_id)
    for (let after = ""; ; ) {
      const page = await lineageCopies(c, itemId, null, false, null, { srcPublish: before?.p ?? null, after });
      orphanCopies.push(...page.slice(0, LINEAGE_COPIES_MAX));
      if (page.length <= LINEAGE_COPIES_MAX) break;
      after = page[LINEAGE_COPIES_MAX - 1]!.item_id;
    }
  await c.db.batch([
    // An upheld takedown (purged without a put-back): what was copied from it is no longer licence
    // or attribution evidence (round 5 E3c): the rightful author is not bound to the infringer's
    // licence nor credited to them. Its strike is upheld only now (round 6).
    ...(wasTakedown
      ? [
          ...(voids ? voidEvidence(c.db, { itemId }, now, false, actor.id, "takedown_upheld", act?.rightful_item_id ?? null) : []),
          ...(orphanCopies.length > 0 && act
            ? [
                c.db
                  .prepare(`INSERT OR IGNORE INTO item_holds (item_id, reason, at, by, source) SELECT value, 'admin', ?1, NULL, ?2 FROM json_each(?3)`)
                  .bind(now, `dmca:${act.id}`, JSON.stringify(orphanCopies.map((x) => x.item_id))),
                c.db
                  .prepare(`INSERT OR IGNORE INTO admin_flags (id, kind, item_id, detail, dedup, at) VALUES (?1, 'dmca_copy', ?2, ?3, ?4, ?5)`)
                  .bind(newId(), itemId, JSON.stringify({ action_id: act.id, upheld: true, held_copies: orphanCopies.length, hint: "name the rightful item (POST /admin/pool/evidence with replaced_by) or release the copies" }), `upheld:${act.id}`, now),
              ]
            : []),
          audit(c.db, { actorId: actor.id, actorKind: actor.kind, action: "evidence.void", itemId, detail: { why: "takedown_upheld" } }, now),
          c.db
            .prepare(`UPDATE strikes SET upheld_at = ?2 WHERE upheld_at IS NULL AND voided_at IS NULL AND action_id IN (SELECT id FROM dmca_actions WHERE item_id = ?1 AND restored_at IS NULL AND purged_at IS NULL)`)
            .bind(itemId, now),
        ]
      : []),
    ...deletePublishStatements(c.db, ids),
    // Round 21 (critic27b E1): the tombstone keeps no typed text, and no flag quotes the served text.
    c.db.prepare(`UPDATE items SET title = '[removed]', title_fold = '', subject_fold = '' WHERE id = ?1`).bind(itemId),
    // (round 22 V-a: the item offers nothing from now on)
    c.db.prepare(`UPDATE content_licences SET until_at = ?2 WHERE item_id = ?1 AND until_at IS NULL`).bind(itemId, now),
    c.db.prepare(`DELETE FROM item_offer WHERE item_id = ?1`).bind(itemId),
    c.db.prepare(`UPDATE admin_flags SET detail = json_set(detail, '$.served', 'purged') WHERE item_id = ?1 AND kind = 'key_leak' AND json_valid(detail) AND json_extract(detail, '$.served') IS NOT NULL`).bind(itemId),
    // The bodies' keys are kept until R2 confirms their delete (round 9): never an orphan nobody can find.
    c.db
      .prepare(`INSERT OR IGNORE INTO r2_orphans (key, at) SELECT value, ?2 FROM json_each(?1)`)
      .bind(JSON.stringify(pubs.results.flatMap((r) => [r.full_key, ...(r.stripped_key ? [r.stripped_key] : [])])), now),
    c.db.prepare(`DELETE FROM question_ids WHERE item_id = ?1`).bind(itemId),
    c.db.prepare(`UPDATE dmca_actions SET purged_at = ?2, outcome = COALESCE(outcome, 'purged') WHERE item_id = ?1 AND restored_at IS NULL AND purged_at IS NULL`).bind(itemId, now),
    audit(c.db, { actorId: actor.id, actorKind: actor.kind, action: "item.purge", itemId, detail: { why } }, now),
  ]);
  // Upheld with a rightful item named (rounds 9 K1, 10 L2 and M-6): a hold of this action is lifted
  // only when every blocked hash of the action the held item uses is one the rightful item exempts
  // for its author (served-ness or authorship, a visible rightful item: rightfulExempts), or, for a
  // lineage hold using none of them, when its questions come from the rightful item; and only if its
  // licence is one its displayed credits now allow. Otherwise it stays held and the admin is told.
  if (wasTakedown && act?.rightful_item_id) {
    const cand = await c.db
      .prepare(
        `SELECT h.item_id FROM item_holds h JOIN items i ON i.id = h.item_id WHERE h.source = ?1 AND
           NOT EXISTS (SELECT 1 FROM blocked_hashes bh WHERE bh.action_id = ?3 AND (
               (bh.kind = 'match' AND ${inQKeysOf("bh.hash", "i.current_publish_id")})
               OR (bh.kind = 'blob' AND bh.hash IN ${servedBlobsOf("i.current_publish_id")}))
             AND NOT ${rightfulExempts("?2", "bh", "i.author_id", "i.current_publish_id")})
           AND (EXISTS (SELECT 1 FROM blocked_hashes bh WHERE bh.action_id = ?3 AND (
                  (bh.kind = 'match' AND ${inQKeysOf("bh.hash", "i.current_publish_id")})
                  OR (bh.kind = 'blob' AND bh.hash IN ${servedBlobsOf("i.current_publish_id")})))
                OR EXISTS (SELECT 1 FROM item_questions q2 WHERE q2.publish_id = i.current_publish_id AND (q2.origin_item_id = ?2 OR instr(q2.chain_json, ?2) > 0)))`,
      )
      .bind(`dmca:${act.id}`, act.rightful_item_id, act.id)
      .all<{ item_id: string }>();
    const checks = await licenseCheckOf(c, cand.results.map((x) => x.item_id));
    const ok = cand.results.map((x) => x.item_id).filter((id) => checks.get(id)?.status !== "mismatch");
    const bad = cand.results.map((x) => x.item_id).filter((id) => checks.get(id)?.status === "mismatch");
    if (ok.length) await c.db.prepare(`DELETE FROM item_holds WHERE source = ?1 AND item_id IN (SELECT value FROM json_each(?2))`).bind(`dmca:${act.id}`, JSON.stringify(ok)).run();
    for (const id of bad)
      await c.db
        .prepare(`INSERT OR IGNORE INTO admin_flags (id, kind, item_id, detail, dedup, at) VALUES (?1, 'license_mismatch', ?2, ?3, ?4, ?5)`)
        .bind(newId(), id, JSON.stringify({ action_id: act.id, check: checks.get(id) }), `license_mismatch:${act.id}:${id}`, now)
        .run();
  }
  const keys = pubs.results.flatMap((r) => [r.full_key, ...(r.stripped_key ? [r.stripped_key] : [])]);
  if (keys.length) await deleteKeys(c, keys);
  if (shas.length) await gcBlobs(c, shas);
  if (wasTakedown && tomb) await repeatInfringerCheck(c, tomb.author_id);
  return true;
}

/** Deletes R2 keys recorded in r2_orphans and forgets them; a failure leaves them for the hourly retry. */
export async function deleteKeys(c: Ctx, keys: string[]): Promise<boolean> {
  try {
    await c.r2.delete(keys);
  } catch (e) {
    console.error(`pool: R2 delete failed, retried hourly: ${e instanceof Error ? e.message : String(e)}`);
    return false;
  }
  await c.db.prepare(`DELETE FROM r2_orphans WHERE key IN (SELECT value FROM json_each(?1))`).bind(JSON.stringify(keys)).run();
  return true;
}

/** Hourly: R2 deletes a purge could not finish. */
export async function retryOrphans(c: Ctx): Promise<number> {
  // Fewest attempts first, then oldest; each key backs off (1 h, 2 h, 4 h ... at most a day) on failure.
  const now = c.now();
  const r = await c.db.prepare(`SELECT key FROM r2_orphans WHERE next_at <= ?1 ORDER BY attempts, at LIMIT 500`).bind(now).all<{ key: string }>();
  if (r.results.length === 0) return 0;
  const keys = r.results.map((x) => x.key);
  if (await deleteKeys(c, keys)) return keys.length;
  await c.db
    .prepare(`UPDATE r2_orphans SET attempts = attempts + 1, next_at = ?2 + MIN(86400000, 3600000 * (1 << MIN(attempts, 5))) WHERE key IN (SELECT value FROM json_each(?1))`)
    .bind(JSON.stringify(keys), now)
    .run();
  return 0;
}

/** The repeat-infringer policy, on UPHELD strikes only: the admin suspends through the audited /admin/suspend route. */
/**
 * Round 22 (critic29b H3): put-backs claimed (restored_at set) whose item is still the taken-down one
 * an hour later: the remaining steps are run again (idempotent).
 */
export async function putbackFinish(c: Ctx): Promise<number> {
  const due = await c.db
    .prepare(
      `SELECT a.id, a.outcome, i.purged_at IS NULL AS live FROM dmca_actions a JOIN items i ON i.id = a.item_id
        WHERE a.restored_at IS NOT NULL AND a.restored_at < ?1 AND i.removed_reason = 'takedown'
          AND (i.purged_at IS NULL OR i.purged_at < a.disabled_at)
          AND NOT EXISTS (SELECT 1 FROM dmca_actions b WHERE b.item_id = a.item_id AND b.disabled_at > a.disabled_at) LIMIT 50`,
    )
    .bind(c.now() - HOUR_MS)
    .all<{ id: string; outcome: string | null; live: number }>();
  for (const a of due.results) await restoreFinish(c, a.id, { id: null, kind: "system" }, a.outcome ?? "put_back", a.live === 1);
  return due.results.length;
}
/** Round 22 (critic29b): a repeat-infringer check that failed is redone by the cron, from the strikes. */
export async function repeatInfringerSweep(c: Ctx): Promise<number> {
  const due = await c.db
    .prepare(
      `SELECT user_id FROM (SELECT user_id, COUNT(*) AS n FROM strikes WHERE upheld_at IS NOT NULL AND voided_at IS NULL AND at > ?1 GROUP BY user_id) s
        WHERE s.n >= ?2 AND NOT EXISTS (SELECT 1 FROM admin_flags f WHERE f.dedup = 'strikes:' || s.user_id || ':' || s.n) LIMIT 50`,
    )
    .bind(c.now() - STRIKE_WINDOW_MS, STRIKES_TO_SUSPEND)
    .all<{ user_id: string }>();
  for (const u of due.results) await repeatInfringerCheck(c, u.user_id);
  return due.results.length;
}
export async function repeatInfringerCheck(c: Ctx, userId: string): Promise<void> {
  const now = c.now();
  const strikes = await c.db
    .prepare(`SELECT COUNT(*) AS n FROM strikes WHERE user_id = ?1 AND upheld_at IS NOT NULL AND voided_at IS NULL AND at > ?2`)
    .bind(userId, now - STRIKE_WINDOW_MS)
    .first<{ n: number }>();
  if ((strikes?.n ?? 0) < STRIKES_TO_SUSPEND) return;
  const ins = await c.db
    .prepare(`INSERT OR IGNORE INTO admin_flags (id, kind, user_id, detail, dedup, at) VALUES (?1, 'repeat_infringer', ?2, ?3, ?4, ?5)`)
    .bind(newId(), userId, JSON.stringify({ strikes: strikes!.n }), `strikes:${userId}:${strikes!.n}`, now)
    .run();
  if ((ins.meta.changes ?? 0) === 1) send(c, { kind: "repeat_infringer", userId, strikes: strikes!.n });
}

/**
 * Voids (or restores) the evidence recorded from an item, or from everything an account was served.
 * An item's void also marks the item itself (void_items): links to it in anyone's chain stop
 * granting and stop being shown, and sources whose lineage runs through it count only for a
 * verbatim copy of their own additions (round 6 F3).
 */
export function voidEvidence(db: D1Database, who: { itemId?: string; userId?: string }, now: number, restore: boolean, by: string | null = null, reason = "admin", replacedBy: string | null = null): D1PreparedStatement[] {
  const col = who.itemId ? "item_id" : "user_id";
  // (Round 19: the evidence is the reader's served rows.)
  const scol = who.itemId ? "item_id" : "reader_id";
  const rows = restore
    ? db.prepare(`UPDATE served SET void_at = NULL WHERE ${scol} = ?1 AND void_at IS NOT NULL`).bind(who.itemId ?? who.userId)
    : db.prepare(`UPDATE served SET void_at = ?2 WHERE ${scol} = ?1 AND void_at IS NULL`).bind(who.itemId ?? who.userId, now);
  if (!who.itemId) return [rows];
  return [
    rows,
    restore
      ? db.prepare(`DELETE FROM void_items WHERE item_id = ?1`).bind(who.itemId)
      : db
          .prepare(
            `INSERT OR IGNORE INTO void_items (item_id, at, by, reason, replaced_by, replaced_license, replaced_author_id)
             SELECT ?1, ?2, ?3, ?4, ?5, ri.license, ri.author_id FROM (SELECT 1) LEFT JOIN items ri ON ri.id = ?5`,
          )
          .bind(who.itemId, now, by, reason, replacedBy),
  ];
}

/** Moves every retained body of `itemId` under quarantine/ (or back), rewriting the keys in D1. */
/** Round 21 (critic27b G1): takedowns whose bodies were not all moved to quarantine (a run that failed part way) are finished. */
export async function finishQuarantines(c: Ctx): Promise<number> {
  const r = await c.db
    .prepare(
      `SELECT DISTINCT i.id FROM items i JOIN item_publishes p ON p.item_id = i.id
        WHERE i.removed_reason = 'takedown' AND i.purged_at IS NULL
          AND EXISTS (SELECT 1 FROM dmca_actions a WHERE a.item_id = i.id AND a.restored_at IS NULL AND a.purged_at IS NULL)
          AND (p.full_key NOT LIKE 'quarantine/%' OR (p.stripped_key IS NOT NULL AND p.stripped_key NOT LIKE 'quarantine/%'))
        LIMIT 50`,
    )
    .all<{ id: string }>();
  for (const x of r.results) await moveBodies(c, x.id, true);
  return r.results.length;
}

async function moveBodies(c: Ctx, itemId: string, toQuarantine: boolean): Promise<string[]> {
  const written: string[] = [];
  const pubs = await c.db
    .prepare(`SELECT publish_id, full_key, stripped_key FROM item_publishes WHERE item_id = ?1`)
    .bind(itemId)
    .all<{ publish_id: string; full_key: string; stripped_key: string | null }>();
  for (const r of pubs.results) {
    const dest = (form: string) => (toQuarantine ? `quarantine/${itemId}/${r.publish_id}/${form}.json` : `items/${itemId}/${newId()}.json`);
    const move = async (key: string | null, form: string): Promise<string | null> => {
      if (!key) return null;
      if (key.startsWith("quarantine/") === toQuarantine) return key;
      const o = await c.r2.get(key);
      if (!o) return key;
      const to = dest(form);
      // (Round 22, critic29b op 36: a copy no row will reference if what follows fails is recorded as
      // an orphan first, a day out; the row's update forgets it in the same batch.)
      await c.db.prepare(`INSERT INTO r2_orphans (key, at, next_at) VALUES (?1, ?2, ?3) ON CONFLICT (key) DO UPDATE SET next_at = excluded.next_at`).bind(to, c.now(), c.now() + DAY_MS).run();
      await c.r2.put(to, await o.arrayBuffer(), { httpMetadata: { contentType: "application/json" } }); // r2-object
      written.push(to);
      return to;
    };
    const nf = await move(r.full_key, "full");
    const ns = await move(r.stripped_key, "stripped");
    await c.db.batch([
      c.db.prepare(`UPDATE item_publishes SET full_key = ?2, stripped_key = ?3 WHERE publish_id = ?1`).bind(r.publish_id, nf, ns),
      c.db.prepare(`DELETE FROM r2_orphans WHERE key IN (SELECT value FROM json_each(?1))`).bind(JSON.stringify([nf, ns].filter((k) => k !== null))),
    ]);
    const old = [r.full_key, r.stripped_key].filter((k, i): k is string => k !== null && k !== [nf, ns][i]);
    if (old.length) await c.r2.delete(old);
  }
  return written;
}

/**
 * Items that copy `itemId`: any other item whose current publish has one of its questions
 * (match_hash, whatever the key) or records it as an origin. Listed for the admin on a takedown.
 */
/** Copies one lineage lookup returns; one more says there are more (round 20 F1: never a silent cut). */
export const LINEAGE_COPIES_MAX = 500;
export async function lineageCopies(
  c: Ctx,
  itemId: string,
  designated: string[] | null = null,
  includeUnlinked = false,
  blobs: string[] | null = null,
  // Round 21 (critic27b B-3/D2): the infringing item's publish to compare with (read before a purge's
  // claim clears it), copies a hold of `heldBy` already holds left out, and paging after an item id.
  o: { srcPublish?: string | null; heldBy?: string; after?: string } = {},
): Promise<{ item_id: string; author_id: string; title: string; visible: boolean }[]> {
  // A copy must have lineage (its author imported the item, or recorded it as an origin) AND copy
  // the whole item or a designated question (critic D2 5): an independent item that only shares a
  // common question is not a copy. An admin can widen it explicitly (includeUnlinked).
  const r = await c.db
    .prepare(
      `SELECT i.id AS item_id, i.author_id, i.title,
              EXISTS (SELECT 1 FROM items vi JOIN users u ON u.id = vi.author_id WHERE vi.id = i.id AND ${VISIBLE_TO_OTHERS.replaceAll("i.", "vi.")}) AS visible FROM items i
        WHERE i.id <> ?1 AND i.purged_at IS NULL AND i.current_publish_id IS NOT NULL AND i.id > ?6
          AND NOT EXISTS (SELECT 1 FROM item_holds hd WHERE hd.item_id = i.id AND hd.source = ?7)
          -- Never an author a takedown of this item exempts (the rightful author, round 7 M3).
          AND NOT EXISTS (SELECT 1 FROM dmca_exempt e JOIN dmca_actions a ON a.id = e.action_id WHERE a.item_id = ?1 AND e.user_id = i.author_id)
          -- ... nor the rightful item a takedown of it names, nor its author's items (round 10).
          AND NOT EXISTS (SELECT 1 FROM dmca_actions a JOIN items ri ON ri.id = a.rightful_item_id WHERE a.item_id = ?1 AND (ri.id = i.id OR ri.author_id = i.author_id))
          -- Round 24 (critic31a/31b): nor the item the taken-down item itself came from. Its visibility
          -- may have ended since (unlisted), so it cannot be named as the rightful item, but it is not
          -- a copy of its own copy: holding the author's own original would punish the original.
          AND NOT EXISTS (
            SELECT 1 FROM (
              SELECT chain_json, origin_item_id FROM item_questions WHERE publish_id = ?5
              UNION ALL SELECT chain_json, NULL AS origin_item_id FROM ghost_questions WHERE publish_id = ?5) sr
             WHERE (sr.origin_item_id = i.id
                OR EXISTS (SELECT 1 FROM json_each(COALESCE(sr.chain_json, '[]')) cl WHERE cl.value ->> '$.item_id' = i.id
                             AND COALESCE(cl.value ->> '$.earlier_match', 0) = 0 AND COALESCE(cl.value ->> '$.possible_source', 0) = 0))
               -- Round 26 (critic-moodle-33c B-1): ... only while it carries no blocked hash of a takedown
               -- of this item that it did not carry when that action was made (material added later is
               -- held like any copy).
               AND NOT EXISTS (SELECT 1 FROM dmca_actions la JOIN blocked_hashes bh ON bh.action_id = la.id AND bh.lifted_at IS NULL
                                WHERE la.item_id = ?1 AND la.restored_at IS NULL
                                  AND ((bh.kind = 'match' AND ${inQKeysOf("bh.hash", "i.current_publish_id")}) OR (bh.kind = 'blob' AND bh.hash IN ${servedBlobsOf("i.current_publish_id")}))
                                  AND NOT ${upstreamCarried("la.id", "i.id", "bh.hash")}))
          -- (Round 25: no "the taken-down item's author imported it" branch: a plain GET is an import
          -- row too, so it let an infringer shield any copy with one read. Origin/chain only.)
          AND (
            -- Round 14 (B4): a copy of a designated question by CONTENT (any block key), whatever its
            -- lineage and visibility (unlisted pre-staged pastes included).
            (?2 IS NOT NULL AND EXISTS (SELECT 1 FROM item_questions cm WHERE cm.publish_id = i.current_publish_id AND ${qKeyIn("cm", "(SELECT value FROM json_each(?2))")}))
            -- Round 16 (critic21d B7): ... or an image the takedown blocks, by its exact bytes (served).
            OR (?4 IS NOT NULL AND ${itemServesBlob("i.id", (sha) => `${sha} IN (SELECT value FROM json_each(?4))`)})
            OR EXISTS (SELECT 1 FROM item_questions o WHERE o.publish_id = i.current_publish_id AND o.origin_item_id = ?1
                      AND (?2 IS NULL OR ${qKeyIn("o", "(SELECT value FROM json_each(?2))")}))
            OR ((?3 = 1 OR EXISTS (SELECT 1 FROM imports im WHERE im.user_id = i.author_id AND im.item_id = ?1 AND im.kind = 'import'))
                AND (CASE WHEN ?2 IS NULL
                       -- (round 21 B-3: against the infringing item's publish, never vacuously: it has questions)
                       -- (round 22: or its ghost's, when the item is gone)
                       THEN EXISTS (SELECT 1 FROM item_questions src0 WHERE src0.publish_id = ?5 UNION ALL SELECT 1 FROM ghost_questions g0 WHERE g0.publish_id = ?5)
                        AND NOT EXISTS (SELECT 1 FROM (SELECT match_hash FROM item_questions WHERE publish_id = ?5 UNION ALL SELECT match_hash FROM ghost_questions WHERE publish_id = ?5) src
                                         WHERE NOT EXISTS (SELECT 1 FROM item_questions cq WHERE cq.publish_id = i.current_publish_id AND cq.match_hash = src.match_hash))
                       ELSE EXISTS (SELECT 1 FROM item_questions cq WHERE cq.publish_id = i.current_publish_id AND ${qKeyIn("cq", "(SELECT value FROM json_each(?2))")}) END)))
        ORDER BY i.id LIMIT ${LINEAGE_COPIES_MAX + 1}`,
    )
    .bind(
      itemId,
      designated ? JSON.stringify(designated) : null,
      includeUnlinked ? 1 : 0,
      blobs && blobs.length > 0 ? JSON.stringify(blobs) : null,
      o.srcPublish !== undefined ? o.srcPublish : ((await c.db.prepare(`SELECT current_publish_id AS p FROM items WHERE id = ?1`).bind(itemId).first<{ p: string | null }>())?.p ?? null),
      o.after ?? "",
      o.heldBy ?? "",
    )
    .all<{ item_id: string; author_id: string; title: string; visible: number }>();
  return r.results.map((x) => ({ ...x, visible: x.visible === 1 }));
}

/** A DMCA takedown: disabled at once (D1), then the bodies go to quarantine/. */
/** Round 26: the action's selected keys, as the upstream snapshot reads them. */
const blockKeysOf = (plan: BlockPlan) => [...selectedKeyPairs(plan).map((x) => ({ k: "match", h: x.h })), ...selectedBlobs(plan).map((h) => ({ k: "blob", h }))];
/** Round 22: the latest ghost publish of an item that is gone (what its copies are compared with). */
async function latestGhost(c: Ctx, itemId: string): Promise<string | null> {
  return (await c.db.prepare(`SELECT publish_id AS p FROM ghost_publishes WHERE item_id = ?1 ORDER BY at DESC, publish_id DESC LIMIT 1`).bind(itemId).first<{ p: string }>())?.p ?? null;
}
/**
 * Round 25 (critic-moodle-24 B-1): the item ids the publish `publishId` derives from: its recorded
 * origin and its derivation chain links (never an `earlier_match` credit or a possible source).
 * Stored on the takedown action so it survives the copy's purge when NO ghost is left (nobody was
 * served): the original author's later republish is still recognised as the source, not a re-upload.
 */
async function derivationOf(c: Ctx, publishId: string | null, ghost: boolean): Promise<string> {
  if (!publishId) return "[]";
  const r = await c.db
    .prepare(
      `SELECT DISTINCT j.id AS id FROM (
         ${ghost ? "" : "SELECT origin_item_id AS id FROM item_questions WHERE publish_id = ?1 UNION ALL"}
         SELECT cl.value ->> '$.item_id' AS id FROM ${ghost ? "ghost_questions" : "item_questions"} q, json_each(q.chain_json) cl
          WHERE q.publish_id = ?1 AND COALESCE(cl.value ->> '$.earlier_match', 0) = 0 AND COALESCE(cl.value ->> '$.possible_source', 0) = 0) j
        WHERE j.id IS NOT NULL AND j.id <> ''`,
    )
    .bind(publishId)
    .all<{ id: string }>();
  return JSON.stringify(r.results.map((x) => x.id));
}
/** Round 22: the copies of an item that is gone, compared with its ghost. */
async function ghostCopies(c: Ctx, itemId: string, o: { heldBy?: string; after?: string } = {}): Promise<Awaited<ReturnType<typeof lineageCopies>>> {
  return lineageCopies(c, itemId, null, false, null, { srcPublish: await latestGhost(c, itemId), ...o });
}

/**
 * Round 22 (critic29a B1): ghost takedowns (an item purged before its notice) whose 30-day window has
 * passed with no counter-notice, court action or withdrawal: upheld as an upheld purge is (evidence
 * void, the strike upheld, the repeat-infringer check, every lineage copy held when no rightful item is
 * named). A batch a run.
 */
export async function ghostUpholds(c: Ctx): Promise<number> {
  const now = c.now();
  // Due: window over with no counter-notice, court action or withdrawal; and (round 23 F1) any uphold
  // claimed whose finish did not complete.
  const due = await c.db
    .prepare(
      `SELECT a.id FROM dmca_actions a JOIN items i ON i.id = a.item_id
        WHERE i.purged_at IS NOT NULL AND i.purged_at < a.disabled_at
          AND ((a.counter_notice_id IS NULL AND a.court_action_at IS NULL AND a.restored_at IS NULL AND a.purged_at IS NULL AND a.author_notified_at <= ?1)
               OR a.uphold_pending = 1) LIMIT 50`,
    )
    .bind(now - QUARANTINE_MS)
    .all<{ id: string }>();
  let n = 0;
  for (const a of due.results) if (await upholdGhost(c, a.id, "upheld", false)) n++;
  return n;
}

/**
 * Round 23 (critic30b A1/F1, critic30a B2): the uphold of a ghost takedown (the 30-day window over, or
 * a court decision for the complainant): one claim (purged_at, the outcome, `uphold_pending`) in one
 * statement, then a finish that is idempotent and resumed by the cron until it clears the marker:
 * evidence void (unless the author is erased and no rightful item is named: shouldVoidEvidence), the
 * strike upheld, every lineage copy held (paged), the repeat-infringer check. `court`: the court
 * decided (a counter-notice may exist; the court action must).
 */
export async function upholdGhost(c: Ctx, actionId: string, outcome: string, court: boolean): Promise<boolean> {
  const now = c.now();
  await c.db
    .prepare(
      `UPDATE dmca_actions SET purged_at = ?2, outcome = ?3, uphold_pending = 1
        WHERE id = ?1 AND purged_at IS NULL AND restored_at IS NULL
          AND ${court ? "court_action_at IS NOT NULL" : "counter_notice_id IS NULL AND court_action_at IS NULL"}`,
    )
    .bind(actionId, now, outcome)
    .run();
  const a = await c.db
    .prepare(`SELECT id, item_id, author_id, rightful_item_id FROM dmca_actions WHERE id = ?1 AND uphold_pending = 1`)
    .bind(actionId)
    .first<{ id: string; item_id: string; author_id: string; rightful_item_id: string | null }>();
  if (!a) return false;
  const copies: { item_id: string }[] = [];
  if (!a.rightful_item_id)
    for (let after = ""; ; ) {
      const page = await ghostCopies(c, a.item_id, { after });
      copies.push(...page.slice(0, LINEAGE_COPIES_MAX));
      if (page.length <= LINEAGE_COPIES_MAX) break;
      after = page[LINEAGE_COPIES_MAX - 1]!.item_id;
    }
  const voids = await shouldVoidEvidence(c, a.author_id, a.rightful_item_id);
  await c.db.batch([
    ...(voids ? voidEvidence(c.db, { itemId: a.item_id }, now, false, null, "takedown_upheld", a.rightful_item_id) : []),
    c.db.prepare(`UPDATE strikes SET upheld_at = ?2 WHERE action_id = ?1 AND upheld_at IS NULL AND voided_at IS NULL`).bind(a.id, now),
    ...(copies.length > 0
      ? [
          c.db.prepare(`INSERT OR IGNORE INTO item_holds (item_id, reason, at, by, source) SELECT value, 'admin', ?1, NULL, ?2 FROM json_each(?3)`).bind(now, `dmca:${a.id}`, JSON.stringify(copies.map((x) => x.item_id))),
          c.db
            .prepare(`INSERT OR IGNORE INTO admin_flags (id, kind, item_id, detail, dedup, at) VALUES (?1, 'dmca_copy', ?2, ?3, ?4, ?5)`)
            .bind(newId(), a.item_id, JSON.stringify({ action_id: a.id, upheld: true, held_copies: copies.length, hint: "name the rightful item (POST /admin/pool/evidence with replaced_by) or release the copies" }), `upheld:${a.id}`, now),
        ]
      : []),
    audit(c.db, { actorId: null, actorKind: "system", action: "evidence.void", itemId: a.item_id, detail: { why: "takedown_upheld", action_id: a.id, voided: voids } }, now),
  ]);
  await notifyHeld(c, copies.map((x) => x.item_id), "copyright");
  await repeatInfringerCheck(c, a.author_id);
  await c.db.prepare(`UPDATE dmca_actions SET uphold_pending = 0 WHERE id = ?1`).bind(a.id).run();
  return true;
}

/**
 * Round 23 (critic30a B2): on every path an upheld takedown voids the infringing item's evidence only
 * when its author could have countered: an erased author cannot, so the takedown stands but its licence
 * keeps binding copies, unless the admin named the rightful item (whose links then stand for it).
 */
export async function shouldVoidEvidence(c: Ctx, authorId: string, rightfulItem: string | null): Promise<boolean> {
  if (rightfulItem) return true;
  return !!(await c.db.prepare(`SELECT 1 AS x FROM users WHERE id = ?1 AND deleted_at IS NULL`).bind(authorId).first());
}

export async function takedown(c: Ctx, adminId: string, noticeId: string, itemId: string, plan: BlockPlan, blockUntil: number | null, exempt: string[] = [], rightfulItem: string | null = null): Promise<{ actionId: string; copies: Awaited<ReturnType<typeof lineageCopies>> }> {
  const now = c.now();
  const notice = await c.db.prepare(`SELECT status, has_signature, has_good_faith, has_perjury_stmt FROM dmca_notices WHERE id = ?1`).bind(noticeId).first<{ status: string; has_signature: number; has_good_faith: number; has_perjury_stmt: number }>();
  if (!notice) throw new PoolError(404, "notice_not_found");
  if (notice.status === "deficient" || notice.status === "withdrawn") throw new PoolError(409, "notice_not_actionable");
  const item = await c.db
    .prepare(`SELECT author_id, current_publish_id, removed_reason, purge_after FROM items WHERE id = ?1 AND purged_at IS NULL`)
    .bind(itemId)
    .first<{ author_id: string; current_publish_id: string | null; removed_reason: string | null; purge_after: number | null }>();
  if (!item) {
    // Rounds 21-22 (critic27b strike evasion, critic29a B1): an item its author deleted (and the cron
    // purged) before the notice is taken down by the SAME state machine as a live one: the author is
    // told, the 30-day counter-notice window opens, the strike is pending (voided by a counter-notice
    // put-back or a withdrawal), and only once the window has passed is it upheld (ghostUpholds: the
    // evidence void, the strike, the repeat-infringer check, the copies held). Nothing is left to
    // quarantine; re-uploads of its questions are blocked meanwhile.
    const gone = await c.db.prepare(`SELECT author_id, removed_reason, purge_after FROM items WHERE id = ?1 AND purged_at IS NOT NULL`).bind(itemId).first<{ author_id: string; removed_reason: string | null; purge_after: number | null }>();
    if (!gone) throw new PoolError(404, "not_found");
    const open = await c.db.prepare(`SELECT id, notice_id FROM dmca_actions WHERE item_id = ?1 AND restored_at IS NULL AND purged_at IS NULL ORDER BY disabled_at DESC LIMIT 1`).bind(itemId).first<{ id: string; notice_id: string }>();
    if (open && open.notice_id === noticeId) {
      await c.db.prepare(`UPDATE dmca_actions SET derivation_json = COALESCE(derivation_json, ?2) WHERE id = ?1`).bind(open.id, await derivationOf(c, await latestGhost(c, itemId), true)).run();
      await flushOutbox(c, [`takedown_author:${open.id}`]);
      return { actionId: open.id, copies: await ghostCopies(c, itemId) };
    }
    if (open || (await c.db.prepare(`SELECT 1 AS x FROM dmca_actions WHERE item_id = ?1 AND restored_at IS NULL`).bind(itemId).first())) throw new PoolError(409, "already_removed");
    // Round 23 (critic30b A2): exactly the plan's selection (made from the ghost by the live rules),
    // with its exempt authors; nothing selected is refused (the admin must choose, or send "all").
    if (plan.selection_required) throw new PoolError(409, "selection_required", { message: "Select the questions (or images) to block, or send questions: \"all\" for the default; the preview lists what each would hold." });
    const actionId = newId();
    const guard = `EXISTS (SELECT 1 FROM dmca_actions WHERE id = ?1)`;
    // Round 25 (critic-moodle-24 B-1): keep the taken-down item's derivation ON the action; the copy
    // may be purged after its uphold with no serve, leaving no ghost to read later.
    const der = await derivationOf(c, await latestGhost(c, itemId), true);
    await c.db.batch([
      c.db
        .prepare(`INSERT INTO dmca_actions (id, notice_id, item_id, author_id, disabled_at, author_notified_at, prev_removed_reason, rightful_item_id, derivation_json) VALUES (?1, ?2, ?3, ?4, ?5, ?5, ?6, ?7, ?8)`)
        .bind(actionId, noticeId, itemId, gone.author_id, now, gone.removed_reason, rightfulItem, der),
      // (the tombstone says what it is now: the counter-notice route and the author's list read it)
      c.db.prepare(`UPDATE items SET removed_reason = 'takedown' WHERE id = ?1 AND purged_at IS NOT NULL`).bind(itemId),
      c.db.prepare(`INSERT INTO strikes (id, user_id, action_id, at, upheld_at) VALUES (?1, ?2, ?3, ?4, NULL)`).bind(newId(), gone.author_id, actionId, now),
      c.db.prepare(`INSERT OR IGNORE INTO dmca_exempt (action_id, user_id, at) SELECT ?1, value, ?2 FROM json_each(?3) WHERE ${guard}`).bind(actionId, now, JSON.stringify(exempt)),
      // Re-uploads of what was selected are held for review meanwhile (as for a live item).
      c.db
        .prepare(
          `INSERT INTO blocked_hashes (kind, hash, action_id, at, expires_at, qhash) SELECT j.value ->> '$.k', j.value ->> '$.h', ?1, ?2, ?3, j.value ->> '$.q' FROM json_each(?4) j WHERE ${guard}
           ON CONFLICT (kind, hash) DO UPDATE SET action_id = excluded.action_id, at = excluded.at, expires_at = excluded.expires_at, lifted_at = NULL, qhash = excluded.qhash WHERE blocked_hashes.lifted_at IS NOT NULL`,
        )
        .bind(actionId, now, blockUntil, JSON.stringify([...selectedKeyPairs(plan).map((x) => ({ k: "match", h: x.h, q: x.q })), ...selectedBlobs(plan).map((h) => ({ k: "blob", h, q: h }))])),
      // (round 26 B-1: what the item it came from carried of them, now; later additions are not exempt)
      c.db.prepare(UPSTREAM_SNAPSHOT_SQL).bind(actionId, der, JSON.stringify(blockKeysOf(plan))),
      c.db.prepare(`UPDATE dmca_actions SET upstream_snap = 1 WHERE id = ?1`).bind(actionId),
      c.db.prepare(`UPDATE dmca_notices SET status = 'actioned' WHERE id = ?1`).bind(noticeId),
      // The author is told as for a live item, from the same batch (an erased account has no address:
      // the admin alone sees it).
      outboxStatement(c.db, `takedown_author:${actionId}`, { kind: "takedown_author", itemId, authorId: gone.author_id, actionId }, now, `EXISTS (SELECT 1 FROM users WHERE id = '${gone.author_id.replace(/'/g, "")}' AND deleted_at IS NULL)`),
      audit(c.db, { actorId: adminId, actorKind: "admin", action: "dmca.takedown", itemId, targetUserId: gone.author_id, detail: { notice_id: noticeId, action_id: actionId, deleted_by_author: true } }, now),
    ]);
    await flushOutbox(c, [`takedown_author:${actionId}`]);
    return { actionId, copies: await ghostCopies(c, itemId) };
  }
  // Round 21 (critic27b G1): a retry of the same notice's takedown that stopped part way resumes it
  // (the bodies' move is idempotent), never a 409 that leaves them unquarantined.
  if (item.removed_reason === "takedown") {
    const open = await c.db
      .prepare(`SELECT id FROM dmca_actions WHERE item_id = ?1 AND notice_id = ?2 AND restored_at IS NULL AND purged_at IS NULL ORDER BY disabled_at DESC LIMIT 1`)
      .bind(itemId, noticeId)
      .first<{ id: string }>();
    if (open) {
      await c.db.prepare(`UPDATE dmca_actions SET derivation_json = COALESCE(derivation_json, ?2) WHERE id = ?1`).bind(open.id, await derivationOf(c, item.current_publish_id, false)).run();
      await moveBodies(c, itemId, true);
      await flushOutbox(c, [`takedown_author:${open.id}`]);
      return { actionId: open.id, copies: await lineageCopies(c, itemId, selectedMatch(plan), false, selectedBlobs(plan)) };
    }
  }
  // A live item, or one its author deleted (the takedown takes over: undelete is refused while it
  // stands, and a put-back returns it to "deleted", never to live). Items removed by an admin, an
  // account deletion or a privacy purge are already disabled for good.
  // An admin-removed item can still be taken down (round 7), so the strike is recorded; a put-back
  // returns it to the admin removal (with what was left of its window).
  // (Round 23, critic30a: an erased account's item inside its 14-day purge window too, as after it.)
  if (item.removed_reason !== null && item.removed_reason !== "author_deleted" && item.removed_reason !== "admin" && item.removed_reason !== "account_deleted") throw new PoolError(409, "already_removed");
  const actionId = newId();
  // Keyed on THIS action (not the time): two takedowns in one millisecond cannot both strike.
  const guard = `EXISTS (SELECT 1 FROM dmca_actions WHERE id = ?3)`;
  // Round 25 (critic-moodle-24 B-1): the derivation is captured from the live publish now; after an
  // uphold with no serve the item leaves no ghost, and the original's republish must not be held.
  const der = await derivationOf(c, item.current_publish_id, false);
  await c.db.batch([
    c.db
      .prepare(
        `UPDATE items SET removed_at = ?2, removed_reason = 'takedown', quarantined_at = ?2, purge_after = NULL, updated_at = ?2
          WHERE id = ?1 AND purged_at IS NULL AND (removed_reason IS NULL OR removed_reason IN ('author_deleted', 'admin', 'account_deleted'))`,
      )
      .bind(itemId, now),
    c.db
      .prepare(`INSERT INTO dmca_actions (id, notice_id, item_id, author_id, disabled_at, author_notified_at, prev_removed_reason, prev_purge_after, rightful_item_id, derivation_json) SELECT ?3, ?4, ?1, ?5, ?2, ?2, ?6, ?7, ?8, ?9 WHERE changes() = 1`)
      .bind(itemId, now, actionId, noticeId, item.author_id, item.removed_reason, item.removed_reason ? item.purge_after : null, rightfulItem, der),
    // A strike counts only once UPHELD (no counter-notice by the end of the quarantine, or a court
    // decision for the complainant): upheld_at is set then, never at takedown time.
    c.db.prepare(`INSERT INTO strikes (id, user_id, action_id, at, upheld_at) SELECT ?5, ?4, ?3, ?2, NULL WHERE ${guard}`).bind(itemId, now, actionId, item.author_id, newId()),
    // Authors the admin names as the rightful ones: the re-upload blocks never hold their publishes.
    c.db.prepare(`INSERT OR IGNORE INTO dmca_exempt (action_id, user_id, at) SELECT ?3, value, ?2 FROM json_each(?4) WHERE ${guard}`).bind(itemId, now, actionId, JSON.stringify(exempt)),
    // Re-uploads of the designated (previewed) questions and images are held for review, until the
    // admin's chosen end or until lifted (critic D2 1/7). A lifted older row is taken over.
    c.db
      .prepare(
        `INSERT INTO blocked_hashes (kind, hash, action_id, at, expires_at, qhash) SELECT j.value ->> '$.k', j.value ->> '$.h', ?3, ?2, ?4, j.value ->> '$.q' FROM json_each(?5) j WHERE ${guard}
         ON CONFLICT (kind, hash) DO UPDATE SET action_id = excluded.action_id, at = excluded.at, expires_at = excluded.expires_at, lifted_at = NULL, qhash = excluded.qhash WHERE blocked_hashes.lifted_at IS NOT NULL`,
      )
      .bind(itemId, now, actionId, blockUntil, JSON.stringify([...selectedKeyPairs(plan).map((x) => ({ k: "match", h: x.h, q: x.q })), ...selectedBlobs(plan).map((h) => ({ k: "blob", h, q: h }))])),
    // (round 26 B-1: what the item it came from carried of them, now; later additions are not exempt)
    c.db.prepare(UPSTREAM_SNAPSHOT_SQL).bind(actionId, der, JSON.stringify(blockKeysOf(plan))),
    c.db.prepare(`UPDATE dmca_actions SET upstream_snap = 1 WHERE id = ?1`).bind(actionId),
    // (Round 22, critic29b: the guard names the ACTION; this binds the notice, so it is checked by notice.)
    c.db.prepare(`UPDATE dmca_notices SET status = 'actioned' WHERE id = ?1 AND EXISTS (SELECT 1 FROM dmca_actions WHERE id = ?2 AND notice_id = ?1)`).bind(noticeId, actionId),
    // (Round 23 F3: the author's notice is written with the action, so no later failure loses it.)
    outboxStatement(c.db, `takedown_author:${actionId}`, { kind: "takedown_author", itemId, authorId: item.author_id, actionId }, now, `EXISTS (SELECT 1 FROM dmca_actions WHERE id = '${actionId}')`),
    audit(c.db, { actorId: adminId, actorKind: "admin", action: "dmca.takedown", itemId, targetUserId: item.author_id, detail: { notice_id: noticeId, action_id: actionId } }, now),
  ]);
  const ok = await c.db.prepare(`SELECT 1 AS x FROM dmca_actions WHERE id = ?1`).bind(actionId).first();
  if (!ok) throw new PoolError(409, "already_removed");
  await moveBodies(c, itemId, true);
  await flushOutbox(c, [`takedown_author:${actionId}`]);
  // (Round 22 V-b: the same copies hold-copies would hold: by the designated questions, or (none
  // designated) the whole item, never an empty designation.)
  const sel = selectedMatch(plan);
  return { actionId, copies: await lineageCopies(c, itemId, sel.length > 0 ? sel : null, false, selectedBlobs(plan)) };
}

/** Round 23 (critic30b G1): blocklist holds of actions put back or withdrawn, released by the cron (bounded). */
export async function releaseRestoredHolds(c: Ctx): Promise<number> {
  const r = await c.db
    .prepare(`SELECT DISTINCT a.id FROM dmca_actions a JOIN item_holds h ON h.source = 'dmca:' || a.id AND h.reason = 'blocklist' WHERE a.restored_at IS NOT NULL LIMIT 50`)
    .all<{ id: string }>();
  for (const a of r.results) await releaseOrphanHolds(c, `dmca:${a.id}`, "blocklist");
  return r.results.length;
}
/** Copy holds an action placed that remain after its put-back or withdrawal: a flag offers their release. */
async function flagLeftHolds(c: Ctx, actionId: string, itemId: string, now: number): Promise<void> {
  const left = await c.db.prepare(`SELECT COUNT(*) AS n FROM item_holds WHERE source = ?1`).bind(`dmca:${actionId}`).first<{ n: number }>();
  if ((left?.n ?? 0) > 0)
    await c.db
      .prepare(`INSERT OR IGNORE INTO admin_flags (id, kind, item_id, detail, dedup, at) VALUES (?1, 'dmca_copy', ?2, ?3, ?4, ?5)`)
      .bind(newId(), itemId, JSON.stringify({ action_id: actionId, held_copies: left!.n, hint: "POST /admin/dmca/release-copies" }), `putback:${actionId}`, now)
      .run();
}

/** Put-back: bodies return from quarantine, the item shows again, the strike and blocklist entries are lifted. */
export async function restore(c: Ctx, actionId: string, actor: { id: string | null; kind: "admin" | "system" }, outcome: string): Promise<boolean> {
  const now = c.now();
  // Claim first (critic A8): exactly one put-back wins, and clearing purge_after stops a purge from
  // claiming the item while its bodies are moved back.
  const claim = await c.db.batch([
    c.db.prepare(`UPDATE dmca_actions SET restored_at = ?2, outcome = ?3 WHERE id = ?1 AND restored_at IS NULL AND purged_at IS NULL`).bind(actionId, now, outcome),
    c.db
      .prepare(`UPDATE items SET purge_after = NULL WHERE id = (SELECT item_id FROM dmca_actions WHERE id = ?1) AND removed_reason = 'takedown' AND purged_at IS NULL AND changes() = 1`)
      .bind(actionId),
  ]);
  if ((claim[0]!.meta.changes ?? 0) !== 1) return false;
  return restoreFinish(c, actionId, actor, outcome, (claim[1]!.meta.changes ?? 0) === 1);
}

/**
 * Round 22 (critic29b H3): the steps of a put-back after its claim, each idempotent, so a put-back that
 * failed part way is finished by the cron (putbackFinish) instead of being stuck with the item removed.
 * `cleared`: the claim cleared the item's purge (it is still the taken-down item, not purged).
 */
async function restoreFinish(c: Ctx, actionId: string, actor: { id: string | null; kind: "admin" | "system" }, outcome: string, cleared: boolean): Promise<boolean> {
  const now = c.now();
  const a = (await c.db
    .prepare(`SELECT item_id, prev_removed_reason, prev_purge_after, disabled_at FROM dmca_actions WHERE id = ?1`)
    .bind(actionId)
    .first<{ item_id: string; prev_removed_reason: string | null; prev_purge_after: number | null; disabled_at: number }>())!;
  // Round 22 (critic29a B1): an item purged before its notice (a ghost takedown): nothing to move back;
  // the put-back or withdrawal voids the pending strike, lifts the blocks and the holds this action
  // caused, and returns the tombstone to what it was.
  if (!cleared && (await c.db.prepare(`SELECT 1 AS x FROM items WHERE id = ?1 AND purged_at IS NOT NULL AND purged_at < ?2`).bind(a.item_id, a.disabled_at).first())) {
    await c.db.batch([
      c.db.prepare(`UPDATE items SET removed_reason = COALESCE(?2, removed_reason) WHERE id = ?1 AND removed_reason = 'takedown'`).bind(a.item_id, a.prev_removed_reason),
      c.db.prepare(`UPDATE strikes SET voided_at = ?2 WHERE action_id = ?1 AND voided_at IS NULL`).bind(actionId, now),
      c.db.prepare(`UPDATE blocked_hashes SET lifted_at = ?2 WHERE action_id = ?1 AND lifted_at IS NULL`).bind(actionId, now),
      audit(c.db, { actorId: actor.id, actorKind: actor.kind, action: "dmca.restore", itemId: a.item_id, detail: { action_id: actionId, outcome, back_to: a.prev_removed_reason ?? "deleted" } }, now),
    ]);
    await releaseOrphanHolds(c, `dmca:${actionId}`, "blocklist");
    // (Round 23, critic30a: copy holds an admin placed stay, flagged for release, as for a live item.)
    await flagLeftHolds(c, actionId, a.item_id, now);
    return true;
  }
  if (!cleared) {
    // The item was purged first: the action is not a put-back after all.
    await c.db.prepare(`UPDATE dmca_actions SET restored_at = NULL, outcome = NULL WHERE id = ?1 AND restored_at = ?2`).bind(actionId, now).run();
    return false;
  }
  const written = await moveBodies(c, a.item_id, false);
  const wasDeleted = a.prev_removed_reason === "author_deleted" || a.prev_removed_reason === "admin" || a.prev_removed_reason === "account_deleted";
  await c.db.batch([
    c.db
      .prepare(
        `UPDATE items SET quarantined_at = NULL, updated_at = ?2,
            removed_at = CASE WHEN ?3 = 1 THEN removed_at ELSE NULL END,
            removed_reason = CASE WHEN ?3 = 1 THEN ?5 ELSE NULL END,
            purge_after = CASE WHEN ?3 = 1 THEN ?4 ELSE NULL END
          WHERE id = ?1 AND removed_reason = 'takedown' AND purged_at IS NULL`,
      )
      // The author's undo window resumes with what was left of it when the takedown began (never purged in the same run).
      .bind(a.item_id, now, wasDeleted ? 1 : 0, wasDeleted ? now + Math.max(DAY_MS, (a.prev_purge_after ?? now) - a.disabled_at) : null, a.prev_removed_reason),
    c.db.prepare(`UPDATE strikes SET voided_at = ?2 WHERE action_id = ?1 AND voided_at IS NULL`).bind(actionId, now),
    c.db.prepare(`UPDATE blocked_hashes SET lifted_at = ?2 WHERE action_id = ?1 AND lifted_at IS NULL`).bind(actionId, now),
    audit(c.db, { actorId: actor.id, actorKind: actor.kind, action: "dmca.restore", itemId: a.item_id, detail: { action_id: actionId, outcome, back_to: wasDeleted ? a.prev_removed_reason : "live" } }, now),
  ]);
  // Holds that only this action's re-upload blocks caused go with them; copy holds an admin placed
  // (hold-copies) stay, flagged so they are offered for release.
  await releaseOrphanHolds(c, `dmca:${actionId}`, "blocklist");
  await flagLeftHolds(c, actionId, a.item_id, now);
  // A purge that claimed the item while the bodies were moving (e.g. a privacy purge) wins: anything
  // this put-back wrote is removed, so no body outlives its rows.
  const gone = await c.db.prepare(`SELECT purged_at FROM items WHERE id = ?1`).bind(a.item_id).first<{ purged_at: number | null }>();
  if (gone?.purged_at != null) {
    if (written.length) await c.r2.delete(written);
    return false;
  }
  return true;
}

/** Court action resolved for the complainant: the material stays down and is purged; the strike stands. */
export async function upholdTakedown(c: Ctx, adminId: string, actionId: string): Promise<boolean> {
  const now = c.now();
  // Round 23 (critic30b A1): a ghost takedown (the item purged before the notice) is upheld by the same
  // claim-and-finish as at the end of its window; nothing is written unless the claim holds.
  const g = await c.db
    .prepare(`SELECT 1 AS x FROM dmca_actions a JOIN items i ON i.id = a.item_id WHERE a.id = ?1 AND i.purged_at IS NOT NULL AND i.purged_at < a.disabled_at`)
    .bind(actionId)
    .first();
  if (g) return upholdGhost(c, actionId, "court_upheld", true);
  const [upd] = await c.db.batch([
    c.db.prepare(`UPDATE dmca_actions SET outcome = 'court_upheld' WHERE id = ?1 AND restored_at IS NULL AND purged_at IS NULL`).bind(actionId),
    c.db.prepare(`UPDATE items SET purge_after = ?2 WHERE id = (SELECT item_id FROM dmca_actions WHERE id = ?1) AND removed_reason = 'takedown' AND purged_at IS NULL AND changes() = 1`).bind(actionId, now),
  ]);
  if ((upd!.meta.changes ?? 0) !== 1) return false;
  const a = (await c.db.prepare(`SELECT item_id FROM dmca_actions WHERE id = ?1`).bind(actionId).first<{ item_id: string }>())!;
  return purgeItem(c, a.item_id, { id: adminId, kind: "admin" }, "court_upheld");
}

/**
 * Round 23 (critic30b A3): an admin's reversal of an UPHELD takedown (a court decision for the
 * author, or an uphold that should not have happened): the strike is voided, the item's voided
 * evidence restored, its blocks lifted and the copies it held released. Content already purged stays
 * purged. `POST /admin/dmca/resolve {outcome: "put_back"}` on an upheld action.
 */
export async function reverseUphold(c: Ctx, adminId: string, actionId: string): Promise<boolean> {
  const now = c.now();
  const [claim] = await c.db.batch([
    c.db.prepare(`UPDATE dmca_actions SET restored_at = ?2, outcome = 'reversed', uphold_pending = 0 WHERE id = ?1 AND purged_at IS NOT NULL AND restored_at IS NULL`).bind(actionId, now),
  ]);
  if ((claim!.meta.changes ?? 0) !== 1) return false;
  const a = (await c.db.prepare(`SELECT item_id FROM dmca_actions WHERE id = ?1`).bind(actionId).first<{ item_id: string }>())!;
  await c.db.batch([
    c.db.prepare(`UPDATE strikes SET voided_at = ?2 WHERE action_id = ?1 AND voided_at IS NULL`).bind(actionId, now),
    c.db.prepare(`UPDATE blocked_hashes SET lifted_at = ?2 WHERE action_id = ?1 AND lifted_at IS NULL`).bind(actionId, now),
    ...(await c.db.prepare(`SELECT 1 AS x FROM void_items WHERE item_id = ?1 AND reason = 'takedown_upheld'`).bind(a.item_id).first() ? voidEvidence(c.db, { itemId: a.item_id }, now, true, adminId, "takedown_reversed") : []),
    c.db.prepare(`DELETE FROM item_holds WHERE source = ?1`).bind(`dmca:${actionId}`),
    audit(c.db, { actorId: adminId, actorKind: "admin", action: "dmca.reverse", itemId: a.item_id, detail: { action_id: actionId } }, now),
  ]);
  return true;
}

/** Admin confirmation of a student-data report: immediate purge, image hashes blocklisted, importers told "privacy". */
export async function privacyPurge(c: Ctx, adminId: string, itemId: string, plan: BlockPlan): Promise<boolean> {
  const now = c.now();
  const mh = selectedMatch(plan);
  const bh = selectedBlobs(plan);
  const [upd] = await c.db.batch([
    c.db
      // Quarantined for the grace window (round 7 M2), not purged at once: invisible to everyone but its
      // author now, restorable (same id, versions, chains) by an unblock or an upheld appeal, purged
      // by the cron after PRIVACY_BYTES_GRACE_MS. What it replaced is kept for a restore.
      .prepare(
        `UPDATE items SET prior_removed_reason = removed_reason, prior_purge_after = purge_after, removed_at = ?2, removed_reason = 'privacy',
           purge_after = ?3, quarantined_at = COALESCE(quarantined_at, ?2), updated_at = ?2
         WHERE id = ?1 AND purged_at IS NULL AND (removed_reason IS NULL OR removed_reason <> 'privacy')`,
      )
      .bind(itemId, now, now + PRIVACY_BYTES_GRACE_MS),
    // Round 21 (critic27b E1): the served text a key_leak flag quoted goes now (it may hold the data).
    c.db.prepare(`UPDATE admin_flags SET detail = json_set(detail, '$.served', 'purged') WHERE item_id = ?1 AND kind = 'key_leak' AND json_valid(detail) AND json_extract(detail, '$.served') IS NOT NULL`).bind(itemId),
    // Its own table (C1), and only what the admin designated and previewed (D2): by default every
    // distinctive question and every image, never a question K other authors also use.
    c.db
      .prepare(`INSERT OR IGNORE INTO privacy_blocks (kind, hash, item_id, at, by, qhash) SELECT 'match', j.value ->> '$.h', ?1, ?2, ?3, j.value ->> '$.q' FROM json_each(?4) j`)
      .bind(itemId, now, adminId, JSON.stringify(selectedKeyPairs(plan))),
    // Round 12 M6: the selected questions' stem bands, so a later near-rewording is flagged for the admin.
    c.db
      .prepare(
        `INSERT OR IGNORE INTO privacy_bands (band, qhash, tokens, item_id, at)
         -- (rounds 21-22 E1/B2: the keyed bands and keyed token set, never the words of a student-data stem
         -- nor an unkeyed band; a row from before the keyed sketches has no bands here: exact blocks only)
         SELECT b.value, iq.match_hash, iq.tok_sk, ?1, ?2 FROM items i JOIN item_questions iq ON iq.publish_id = i.current_publish_id, json_each(COALESCE(iq.kbands, '[]')) b
          WHERE i.id = ?1 AND iq.match_hash IN (SELECT value FROM json_each(?3))`,
      )
      // Only for blocks that cover the stem (round 13): a block narrowed to the explanation or the
      // answer never holds near-rewordings of an innocent stem.
      .bind(itemId, now, JSON.stringify(plan.match.filter((x) => x.selected && x.keys.some((k) => k.startsWith("s:"))).map((x) => x.hash))),
    // ... and the bands of each blocked PART stem (round 16 B5).
    c.db
      .prepare(
        `INSERT OR IGNORE INTO privacy_bands (band, qhash, tokens, item_id, at)
         -- (round 21 E1: no words; a part's band alone is matched, conservatively)
         SELECT b.value, iq.match_hash, NULL, ?1, ?2 FROM items i JOIN item_questions iq ON iq.publish_id = i.current_publish_id
           JOIN item_part_keys pk ON pk.publish_id = iq.publish_id AND pk.pos = iq.pos, json_each(COALESCE(pk.kbands, '[]')) b
          WHERE i.id = ?1 AND pk.key IN (SELECT value FROM json_each(?3))`,
      )
      .bind(itemId, now, JSON.stringify(plan.match.filter((x) => x.selected).flatMap((x) => x.keys.filter((k) => k.startsWith("s:"))))),
    c.db
      .prepare(`INSERT OR IGNORE INTO privacy_blocks (kind, hash, item_id, at, by, bytes_purge_after) SELECT 'blob', value, ?1, ?2, ?3, ?5 FROM json_each(?4)`)
      .bind(itemId, now, adminId, JSON.stringify(bh), now + PRIVACY_BYTES_GRACE_MS),
    // Exactly the previewed items are held (any state, so an undelete or put-back keeps them held).
    c.db
      .prepare(`INSERT OR IGNORE INTO item_holds (item_id, reason, at, by, source, detail) SELECT j.value, 'student_data', ?1, ?2, 'privacy', ${holdSnapshot("j.value", "?4")} FROM json_each(?3) j`)
      .bind(now, adminId, JSON.stringify(plan.affected.map((a) => a.item_id)), JSON.stringify(mh)),
    // The purged item's author's pending holds on those images go too; never another account's
    // (round 17, critic22e A1: that would answer "blocked" to their next publish).
    c.db.prepare(`DELETE FROM pending_blobs WHERE sha256 IN (SELECT value FROM json_each(?1)) AND uploader_id = (SELECT author_id FROM items WHERE id = ?2)`).bind(JSON.stringify(bh), itemId),
    c.db.prepare(`UPDATE reports SET resolved_at = ?2, resolved_by = ?3, action = 'purged' WHERE item_id = ?1 AND resolved_at IS NULL`).bind(itemId, now, adminId),
    c.db.prepare(`DELETE FROM item_holds WHERE item_id = ?1`).bind(itemId),
    audit(c.db, { actorId: adminId, actorKind: "admin", action: "privacy.purge", itemId, detail: { questions: mh.length, images: bh.length, held: plan.affected.length } }, now),
    // Round 22 (critic29b H2): what is left to do (the copies past the list, the bodies' move) is a
    // backlog from the claim on, in the same batch: a failure part way is finished by the cron or by
    // the admin's retry, never left with student data public. Resolved once it is all done.
    c.db
      .prepare(`INSERT OR IGNORE INTO admin_flags (id, kind, item_id, detail, dedup, at) SELECT ?1, 'privacy_backlog', ?2, ?3, ?4, ?5
         WHERE EXISTS (SELECT 1 FROM items WHERE id = ?2 AND removed_reason = 'privacy' AND purged_at IS NULL AND removed_at = ?5)`)
      .bind(newId(), itemId, JSON.stringify({ match: mh, blob: bh, batch: AFFECTED_LIST, by: adminId }), `privacy_backlog:${itemId}:${now}`, now),
  ]);
  if ((upd!.meta.changes ?? 0) !== 1) {
    // (Round 22 H2: the admin's retry of a purge that stopped part way finishes it: 200, idempotent.)
    const open = await c.db
      .prepare(`SELECT f.id FROM admin_flags f JOIN items i ON i.id = f.item_id WHERE f.kind = 'privacy_backlog' AND f.item_id = ?1 AND i.removed_reason = 'privacy' AND i.purged_at IS NULL LIMIT 1`)
      .bind(itemId)
      .first<{ id: string }>();
    if (!open) return false;
    await privacyFinish(c, itemId, mh, bh, adminId, 10);
    return true;
  }
  await notifyHeld(c, plan.affected.map((a) => a.item_id).filter((x) => x !== itemId), "student_data");
  // More than the listed affected items (the admin confirmed with confirm_large): the rest are held
  // in batches now; what a request cannot finish the daily cron continues. Never silently skipped.
  await privacyFinish(c, itemId, mh, bh, adminId, plan.affected_total > plan.affected.length ? 10 : 1);
  return true;
}

/**
 * Round 22 (critic29b H2): the rest of a privacy purge, idempotent: every copy held (fail closed,
 * before anything is marked done), the bodies under quarantine/, then the backlog resolved. The
 * student's image is served from no item at once (getBlob refuses privacy blocks); its bytes, like the
 * item's own bodies, are deleted only after PRIVACY_BYTES_GRACE_MS (cron), so an unblock or upheld
 * appeal in that window loses nothing.
 */
export async function privacyFinish(c: Ctx, itemId: string, mh: string[], bh: string[], by: string | null, batches: number): Promise<boolean> {
  const more = await holdPrivacyBatch(c, itemId, mh, bh, by, batches);
  if (more) return false;
  if (await c.db.prepare(`SELECT 1 AS x FROM items WHERE id = ?1 AND removed_reason = 'privacy' AND purged_at IS NULL`).bind(itemId).first()) await moveBodies(c, itemId, true);
  await c.db.prepare(`UPDATE admin_flags SET resolved_at = ?2 WHERE kind = 'privacy_backlog' AND item_id = ?1 AND resolved_at IS NULL`).bind(itemId, c.now()).run();
  return true;
}

/**
 * Puts a privacy-removed item back within its grace window (an unblock of its last own block, or an
 * upheld appeal): its own blocks are lifted, holds only they caused are released, its bodies return
 * from quarantine and it is what it was before (live, or the removal it replaced).
 */
export async function privacyRestore(c: Ctx, actor: { id: string | null; kind: "admin" | "system" }, itemId: string, note: string | null = null): Promise<boolean> {
  const now = c.now();
  // Within the grace window, or later while an appeal is open (round 9 J5: the purge waits for it).
  const restorable = `removed_reason = 'privacy' AND purged_at IS NULL AND (purge_after > ?2 OR ${OPEN_APPEAL_SQL("items.id")})`;
  const it = await c.db
    .prepare(`SELECT prior_removed_reason FROM items WHERE id = ?1 AND ${restorable}`)
    .bind(itemId, now)
    .first<{ prior_removed_reason: string | null }>();
  if (!it) return false;
  const [upd] = await c.db.batch([
    c.db
      .prepare(
        `UPDATE items SET removed_reason = prior_removed_reason, purge_after = prior_purge_after,
           removed_at = CASE WHEN prior_removed_reason IS NULL THEN NULL ELSE removed_at END,
           quarantined_at = CASE WHEN prior_removed_reason = 'takedown' THEN quarantined_at ELSE NULL END,
           prior_removed_reason = NULL, prior_purge_after = NULL, updated_at = ?2
         WHERE id = ?1 AND ${restorable}`,
      )
      .bind(itemId, now),
    c.db.prepare(`DELETE FROM privacy_blocks WHERE item_id = ?1 AND changes() = 1`).bind(itemId),
    c.db.prepare(`DELETE FROM privacy_bands WHERE qhash NOT IN (SELECT qhash FROM privacy_blocks WHERE qhash IS NOT NULL)`),
    // Restored: its open appeals are answered (round 9).
    c.db
      .prepare(`UPDATE admin_flags SET resolved_at = ?2, resolved_by = ?3 WHERE kind = 'appeal' AND item_id = ?1 AND resolved_at IS NULL AND EXISTS (SELECT 1 FROM items WHERE id = ?1 AND removed_reason IS NOT 'privacy')`)
      .bind(itemId, now, actor.id ?? "system"),
    audit(c.db, { actorId: actor.id, actorKind: actor.kind, action: "privacy.restore", itemId, note }, now),
  ]);
  if ((upd!.meta.changes ?? 0) !== 1) return false;
  if (it.prior_removed_reason !== "takedown") await moveBodies(c, itemId, false);
  await releaseOrphanHolds(c, "privacy", "student_data");
  return true;
}

/**
 * Copies held because they use student data an admin confirmed on item `itemId` (round 9): the
 * admin previews them (no `confirm`), then removes them in bulk, each quarantined for the grace
 * window like the original (its text and images then purged by the cron), restorable by an unblock
 * or a privacy restore meanwhile. At most COPIES_PER_CALL per call; `remaining` says how many are left.
 */
export const COPIES_PER_CALL = 50;
export async function purgeHeldCopies(c: Ctx, adminId: string, itemId: string, confirm: boolean): Promise<{ items: string[]; total: number; removed: number; remaining: number }> {
  const now = c.now();
  const held = `SELECT DISTINCT h.item_id FROM item_holds h JOIN items i ON i.id = h.item_id
                 WHERE h.source = 'privacy' AND i.id <> ?1 AND i.purged_at IS NULL AND (i.removed_reason IS NULL OR i.removed_reason <> 'privacy')
                   AND (EXISTS (SELECT 1 FROM item_publishes p JOIN item_questions iq ON iq.publish_id = p.publish_id JOIN privacy_blocks pb ON pb.kind = 'match' AND ${qKeyEq("iq", "pb.hash")}
                                 WHERE p.item_id = i.id AND pb.item_id = ?1)
                        OR ${itemServesBlob("i.id", (sha) => `EXISTS (SELECT 1 FROM privacy_blocks pb WHERE pb.kind = 'blob' AND pb.hash = ${sha} AND pb.item_id = ?1)`)})`;
  const total = (await c.db.prepare(`SELECT COUNT(*) AS n FROM (${held})`).bind(itemId).first<{ n: number }>())?.n ?? 0;
  const items = (await c.db.prepare(`${held} ORDER BY h.item_id LIMIT ?2`).bind(itemId, COPIES_PER_CALL).all<{ item_id: string }>()).results.map((r) => r.item_id);
  if (!confirm) return { items, total, removed: 0, remaining: total };
  let removed = 0;
  for (const id of items) {
    const [upd] = await c.db.batch([
      c.db
        .prepare(
          `UPDATE items SET prior_removed_reason = removed_reason, prior_purge_after = purge_after, removed_at = ?2, removed_reason = 'privacy',
             purge_after = ?3, quarantined_at = COALESCE(quarantined_at, ?2), updated_at = ?2
           WHERE id = ?1 AND purged_at IS NULL AND (removed_reason IS NULL OR removed_reason <> 'privacy')`,
        )
        .bind(id, now, now + PRIVACY_BYTES_GRACE_MS),
      audit(c.db, { actorId: adminId, actorKind: "admin", action: "privacy.purge_copy", itemId: id, detail: { source_item: itemId } }, now, `changes() = 1`),
    ]);
    if ((upd!.meta.changes ?? 0) !== 1) continue;
    await moveBodies(c, id, true);
    removed++;
  }
  return { items, total, removed, remaining: Math.max(0, total - removed) };
}

/**
 * Reverses an admin removal (within its 14 days, or later while an appeal is open), back to what it
 * replaced (live, or the author's own deletion). Holds stay; a privacy block on its content adds one;
 * going live is subject to the author's quota and question cap, like an undelete (critic C6).
 */
export async function adminUnremove(c: Ctx, itemId: string, actor: { id: string | null; kind: "admin" | "system" }, note: string | null): Promise<"ok" | "not_found" | "over_limits"> {
  const now = c.now();
  const cond = `removed_reason = 'admin' AND purged_at IS NULL AND (purge_after > ?2 OR ${OPEN_APPEAL_SQL("items.id")})`;
  const it = await c.db.prepare(`SELECT author_id FROM items WHERE id = ?1 AND ${cond}`).bind(itemId, now).first<{ author_id: string }>();
  if (!it) return "not_found";
  const quota = await limitOf(c, it.author_id, "storage_bytes");
  const cap = await limitOf(c, it.author_id, "shared_questions");
  const [upd] = await c.db.batch([
    c.db
      .prepare(
        `UPDATE items SET removed_reason = prior_removed_reason, purge_after = prior_purge_after,
           removed_at = CASE WHEN prior_removed_reason IS NULL THEN NULL ELSE removed_at END,
           prior_removed_reason = NULL, prior_purge_after = NULL, updated_at = ?2
         WHERE id = ?3 AND author_id = ?1 AND ${cond.replaceAll("items.id", "?3")}
           AND (prior_removed_reason IS NOT NULL OR (${usageSql("?3")} <= ?4 AND ${SHARED_SQL("?1", "?3")} + question_count <= ?5))`,
      )
      .bind(it.author_id, now, itemId, quota, cap),
    audit(c.db, { actorId: actor.id, actorKind: actor.kind, action: "item.unremove", itemId, targetUserId: it.author_id, note }, now, `changes() = 1`),
    c.db
      .prepare(
        `INSERT OR IGNORE INTO item_holds (item_id, reason, at, by, source)
         SELECT ?1, 'student_data', ?2, ?3, 'privacy' WHERE changes() = 1 AND (EXISTS (
           SELECT 1 FROM item_publishes p WHERE p.item_id = ?1 AND (
             EXISTS (SELECT 1 FROM item_questions iq JOIN privacy_blocks pb ON pb.kind = 'match' AND ${qKeyEq("iq", "pb.hash")} WHERE iq.publish_id = p.publish_id))
           ) OR ${itemServesBlob("?1", (sha) => `EXISTS (SELECT 1 FROM privacy_blocks pb WHERE pb.kind = 'blob' AND pb.hash = ${sha})`)})`,
      )
      .bind(itemId, now, actor.id),
  ]);
  if ((upd!.meta.changes ?? 0) !== 1) return "over_limits";
  await c.db.prepare(`UPDATE admin_flags SET resolved_at = ?2, resolved_by = ?3 WHERE kind = 'appeal' AND item_id = ?1 AND resolved_at IS NULL`).bind(itemId, now, actor.id ?? "system").run();
  return "ok";
}

/**
 * The admin's decision on an appealed removal (round 9 J5): "restore" puts it back (a privacy
 * removal or an admin removal); "purge" confirms it, and the purge goes ahead at its date. Either
 * resolves the item's open appeals.
 */
export async function decideAppeal(c: Ctx, itemId: string, decision: "restore" | "purge", actor: { id: string | null; kind: "admin" | "system" }, note: string | null): Promise<"ok" | "not_found" | "over_limits"> {
  const now = c.now();
  const it = await c.db
    .prepare(`SELECT removed_reason, EXISTS (SELECT 1 FROM item_holds h WHERE h.item_id = items.id) AS held FROM items WHERE id = ?1 AND purged_at IS NULL`)
    .bind(itemId)
    .first<{ removed_reason: string | null; held: number }>();
  if (!it) return "not_found";
  let res: "ok" | "not_found" | "over_limits" = "ok";
  if (decision === "restore") {
    if (it.removed_reason === "privacy") res = (await privacyRestore(c, actor, itemId, note)) ? "ok" : "not_found";
    else if (it.removed_reason === "admin") res = await adminUnremove(c, itemId, actor, note);
    else if (it.removed_reason === null && it.held)
      // A held item (round 10: copies too): its holds are released, an audited admin action.
      await c.db.prepare(`DELETE FROM item_holds WHERE item_id = ?1`).bind(itemId).run();
    else res = "not_found";
  } else if (it.removed_reason === null && it.held) {
    // Confirming a hold on a copy: removed like the original (quarantined, purged at its date).
    await c.db
      .prepare(
        `UPDATE items SET prior_removed_reason = removed_reason, prior_purge_after = purge_after, removed_at = ?2,
           removed_reason = CASE WHEN EXISTS (SELECT 1 FROM item_holds h WHERE h.item_id = ?1 AND h.source = 'privacy') THEN 'privacy' ELSE 'admin' END,
           purge_after = ?3, updated_at = ?2 WHERE id = ?1 AND removed_at IS NULL`,
      )
      .bind(itemId, now, now + PRIVACY_BYTES_GRACE_MS)
      .run();
    await moveBodies(c, itemId, true);
  }
  if (res !== "ok") return res;
  await c.db.batch([
    c.db.prepare(`UPDATE admin_flags SET resolved_at = ?2, resolved_by = ?3 WHERE kind = 'appeal' AND item_id = ?1 AND resolved_at IS NULL`).bind(itemId, now, actor.id ?? "system"),
    // Decided (round 10 L1): this removal cannot be appealed again.
    c.db.prepare(`UPDATE items SET appeal_decided_at = ?2, appeal_decision = ?3 WHERE id = ?1`).bind(itemId, now, decision),
    audit(c.db, { actorId: actor.id, actorKind: actor.kind, action: `appeal.${decision}`, itemId, note }, now),
  ]);
  return "ok";
}

/** Daily (round 9 J5): open appeals remind the admin after a week; past APPEAL_MAX_MS the item is restored. */
export async function appealsDue(c: Ctx): Promise<{ reminded: number; restored: number }> {
  const now = c.now();
  const open = await c.db
    .prepare(
      `SELECT f.item_id, MIN(f.at) AS since, i.removed_reason FROM admin_flags f JOIN items i ON i.id = f.item_id
        WHERE f.kind = 'appeal' AND f.resolved_at IS NULL AND i.purged_at IS NULL AND f.at <= ?1 GROUP BY f.item_id LIMIT 200`,
    )
    .bind(now - APPEAL_REMINDER_MS)
    .all<{ item_id: string; since: number; removed_reason: string | null }>();
  let reminded = 0;
  let restored = 0;
  for (const a of open.results) {
    // Nothing left to decide (the item was restored or its holds lifted meanwhile): the appeal is answered.
    const live = await c.db
      // Sensitive (round 11 P8): any student-data hold (a report's included), privacy or DMCA.
      .prepare(`SELECT removed_reason, EXISTS (SELECT 1 FROM item_holds h WHERE h.item_id = items.id) AS held, EXISTS (SELECT 1 FROM item_holds h WHERE h.item_id = items.id AND (h.source = 'privacy' OR h.source LIKE 'dmca:%' OR h.reason = 'student_data')) AS sensitive FROM items WHERE id = ?1`)
      .bind(a.item_id)
      .first<{ removed_reason: string | null; held: number; sensitive: number }>();
    if (!live || (live.removed_reason === null && !live.held)) {
      await c.db.prepare(`UPDATE admin_flags SET resolved_at = ?2, resolved_by = 'system' WHERE kind IN ('appeal', 'appeal_overdue', 'appeal_escalated') AND item_id = ?1 AND resolved_at IS NULL`).bind(a.item_id, now).run();
      continue;
    }
    // Round 10 L1: a PRIVACY removal (or a privacy / DMCA hold) is never restored by default. Past
    // APPEAL_MAX_MS its appeal is escalated (the most urgent flag); the item stays quarantined and
    // its bytes held until an admin decides. Only other removals and holds (an admin's policy
    // removal, reports) are restored by default, and flagged.
    const sensitive = live.removed_reason === "privacy" || live.removed_reason === "takedown" || live.sensitive === 1;
    if (a.since <= now - APPEAL_MAX_MS) {
      if (sensitive) {
        const ins = await c.db
          .prepare(`INSERT OR IGNORE INTO admin_flags (id, kind, item_id, detail, dedup, at) VALUES (?1, 'appeal_escalated', ?2, ?3, ?4, ?5)`)
          .bind(newId(), a.item_id, JSON.stringify({ open_since: a.since, removed_reason: live.removed_reason }), `appeal_escalated:${a.item_id}:${a.since}`, now)
          .run();
        reminded += ins.meta.changes ?? 0;
      } else if ((await decideAppeal(c, a.item_id, "restore", { id: null, kind: "system" }, "appeal unanswered for 60 days: restored")) === "ok") {
        restored++;
        await c.db
          .prepare(`INSERT OR IGNORE INTO admin_flags (id, kind, item_id, detail, dedup, at) VALUES (?1, 'appeal_auto_restored', ?2, ?3, ?4, ?5)`)
          .bind(newId(), a.item_id, JSON.stringify({ open_since: a.since }), `appeal_auto_restored:${a.item_id}:${a.since}`, now)
          .run();
      }
      if (!sensitive) continue;
    }
    const ins = await c.db
      // One flag per open appeal (round 11), its week count and date brought up to date each week.
      .prepare(
        `INSERT INTO admin_flags (id, kind, item_id, detail, dedup, at) VALUES (?1, 'appeal_overdue', ?2, ?3, ?4, ?5)
         ON CONFLICT (dedup) DO UPDATE SET detail = excluded.detail, at = excluded.at, resolved_at = NULL, resolved_by = NULL
          WHERE CAST(admin_flags.detail ->> '$.weeks' AS INTEGER) < CAST(excluded.detail ->> '$.weeks' AS INTEGER)`,
      )
      .bind(newId(), a.item_id, JSON.stringify({ open_since: a.since, weeks: Math.floor((now - a.since) / (7 * DAY_MS)), ...(sensitive ? {} : { restores_at: a.since + APPEAL_MAX_MS }) }), `appeal_overdue:${a.item_id}:${a.since}`, now)
      .run();
    reminded += ins.meta.changes ?? 0;
  }
  return { reminded, restored };
}

export { idOrMiss };

/**
 * Is `rightful` a valid rightful item for material of `itemId` (round 11 P4)? It must be another
 * author's live item visible to others, not derived from the infringing item (no chain or origin
 * naming it, its author never served it), and contain EVERY hash in `hashes` in a publish committed
 * before the infringing item's first publish containing it (per-content seq, never the item's age).
 * With no hashes (no block), it must share at least one question with the infringing item, each
 * shared one earlier. Each hash's seqs are returned for the admin's preview.
 */
export async function rightfulCheck(
  c: Ctx,
  rightful: string,
  itemId: string,
  hashes: { match: string[]; blob: string[] },
): Promise<{ ok: boolean; reasons: string[]; hashes: { kind: string; hash: string; rightful_seq: number | null; item_seq: number | null; ok: boolean }[] }> {
  const reasons: string[] = [];
  const r = await c.db
    .prepare(
      `SELECT r.author_id AS ra, x.author_id AS xa, EXISTS (SELECT 1 FROM items i JOIN users u ON u.id = i.author_id WHERE i.id = r.id AND ${VISIBLE_TO_OTHERS}) AS visible,
              -- Rounds 12 M3 / 14: derivation is judged per blocked hash below; this is when R's author
              -- was first served X (null: never).
              (SELECT MIN(COALESCE(im.first_at, im.at)) FROM imports im WHERE im.user_id = r.author_id AND im.item_id = x.id) AS served_at
         FROM items r JOIN items x ON x.id = ?2 WHERE r.id = ?1 AND r.purged_at IS NULL`,
    )
    .bind(rightful, itemId)
    .first<{ ra: string; xa: string; visible: number; served_at: number | null }>();
  if (!r) return { ok: false, reasons: ["rightful_not_found"], hashes: [] };
  if (rightful === itemId || r.ra === r.xa) reasons.push("same_author");
  if (!r.visible) reasons.push("rightful_not_visible");
  const seqs = async (kind: "match" | "blob", list: string[]) => {
    if (list.length === 0) return [];
    const sql =
      kind === "match"
        ? // Round 14: per blocked hash, whether R's question with it came from X (an origin or chain
          // naming X, or a serve of X before R's first publish containing it): one copied question
          // never disqualifies the rest of R.
          `SELECT j.value AS h,
                  (SELECT MIN(q.seq) FROM item_publishes p JOIN item_questions q ON q.publish_id = p.publish_id WHERE p.item_id = ?1 AND p.status IN ('current', 'previous') AND ${qKeyEq("q", "j.value")}) AS rs,
                  (SELECT MIN(q.seq) FROM item_publishes p JOIN item_questions q ON q.publish_id = p.publish_id WHERE p.item_id = ?2 AND ${qKeyEq("q", "j.value")}) AS xs,
                  (EXISTS (SELECT 1 FROM item_publishes p JOIN item_questions q ON q.publish_id = p.publish_id WHERE p.item_id = ?1 AND ${qKeyEq("q", "j.value")} AND (q.origin_item_id = ?2 OR instr(q.chain_json, ?2) > 0))
                   OR (?4 IS NOT NULL AND ?4 < (SELECT MIN(p.committed_at) FROM item_publishes p JOIN item_questions q ON q.publish_id = p.publish_id WHERE p.item_id = ?1 AND p.committed_at IS NOT NULL AND ${qKeyEq("q", "j.value")}))) AS derived
             FROM json_each(?3) j`
        : `SELECT j.value AS h,
                  (SELECT MIN(b.seq) FROM item_publishes p JOIN item_blobs b ON b.publish_id = p.publish_id WHERE p.item_id = ?1 AND p.status IN ('current', 'previous') AND b.sha256 = j.value) AS rs,
                  (SELECT MIN(b.seq) FROM item_publishes p JOIN item_blobs b ON b.publish_id = p.publish_id WHERE p.item_id = ?2 AND b.sha256 = j.value) AS xs
             FROM json_each(?3) j`;
    return (await c.db.prepare(sql).bind(rightful, itemId, JSON.stringify(list), r.served_at).all<{ h: string; rs: number | null; xs: number | null; derived?: number }>()).results.map((x) => ({
      kind,
      hash: x.h,
      rightful_seq: x.rs,
      item_seq: x.xs,
      ...(x.derived ? { derived: true } : {}),
      ok: x.rs !== null && (x.xs === null || x.rs < x.xs) && !x.derived,
    }));
  };
  let list = [...(await seqs("match", hashes.match)), ...(await seqs("blob", hashes.blob))];
  if (list.length === 0) {
    // No block: the questions the two items share, each earlier in the rightful item.
    const shared = await c.db
      .prepare(
        `SELECT DISTINCT xq.match_hash AS h FROM item_publishes xp JOIN item_questions xq ON xq.publish_id = xp.publish_id
          WHERE xp.item_id = ?2 AND EXISTS (SELECT 1 FROM item_publishes rp JOIN item_questions rq ON rq.publish_id = rp.publish_id WHERE rp.item_id = ?1 AND rq.match_hash = xq.match_hash) LIMIT 1001`,
      )
      .bind(rightful, itemId)
      .all<{ h: string }>();
    // Round 20 (F1): more shared questions than one check covers is never a pass.
    if (shared.results.length > 1000) reasons.push("too_many_shared_questions");
    list = await seqs("match", shared.results.slice(0, 1000).map((x) => x.h));
    if (list.length === 0) reasons.push("no_shared_material");
  }
  if (list.some((x) => (x as { derived?: boolean }).derived)) reasons.push("rightful_derived_from_item");
  if (list.some((x) => !x.ok && !(x as { derived?: boolean }).derived)) reasons.push("not_earlier_in_rightful");
  return { ok: reasons.length === 0, reasons, hashes: list.slice(0, 200) };
}

/**
 * Round 14 (B4): the privacy and copyright block checks a publish runs, on an item's CURRENT publish
 * now (a visibility change to public). The same exemptions: an admin's release of this item, the
 * action's exempt authors and the rightful author (and those served the rightful item).
 */
export async function recheckBlocks(c: Ctx, itemId: string): Promise<void> {
  const it = await c.db.prepare(`SELECT author_id, current_publish_id AS pub FROM items WHERE id = ?1 AND current_publish_id IS NOT NULL`).bind(itemId).first<{ author_id: string; pub: string }>();
  if (!it) return;
  const now = c.now();
  await c.db.batch([
    c.db
      .prepare(
        `INSERT OR IGNORE INTO item_holds (item_id, reason, at, by, source)
         SELECT ?1, 'student_data', ?3, NULL, 'privacy' WHERE EXISTS (SELECT 1 FROM privacy_blocks pb WHERE
           ((pb.kind = 'match' AND ${inQKeysOf("pb.hash", "?2")}) OR (pb.kind = 'blob' AND pb.hash IN ${servedBlobsOf("?2")}))
           AND NOT EXISTS (SELECT 1 FROM hold_releases hr, json_each(hr.keys_json) r WHERE hr.item_id = ?1 AND hr.source = 'privacy' AND r.value = pb.hash AND hr.at > ?3 - ${90 * 86_400_000} AND pb.at <= hr.at))`,
      )
      .bind(itemId, it.pub, now),
    c.db
      .prepare(
        `INSERT OR IGNORE INTO item_holds (item_id, reason, at, by, source)
         SELECT DISTINCT ?1, 'blocklist', ?3, NULL, 'dmca:' || bh.action_id FROM blocked_hashes bh
          WHERE bh.lifted_at IS NULL AND (bh.expires_at IS NULL OR bh.expires_at > ?3) AND bh.action_id IS NOT NULL
            AND ((bh.kind = 'match' AND ${inQKeysOf("bh.hash", "?2")}) OR (bh.kind = 'blob' AND bh.hash IN ${servedBlobsOf("?2")}))
            AND NOT EXISTS (SELECT 1 FROM dmca_exempt e WHERE e.action_id = bh.action_id AND e.user_id = ?4)
            AND NOT EXISTS (SELECT 1 FROM dmca_actions a WHERE a.id = bh.action_id AND (a.item_id = ?1 OR (a.rightful_item_id IS NOT NULL AND ${rightfulExempts("a.rightful_item_id", "bh", "?4", "?2")})))
            -- (round 24: the item the taken-down item itself came from is never held as its copy)
            AND NOT ${upstreamCarried("bh.action_id", "?1", "bh.hash")}
            AND NOT EXISTS (SELECT 1 FROM hold_releases hr, json_each(hr.keys_json) r WHERE hr.item_id = ?1 AND hr.source = 'dmca:' || bh.action_id AND r.value = bh.hash AND hr.at > ?3 - ${90 * 86_400_000} AND bh.at <= hr.at)`,
      )
      .bind(itemId, it.pub, now, it.author_id),
  ]);
}
