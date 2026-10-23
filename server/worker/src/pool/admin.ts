// Pool admin routes, served behind the auth package's admin guard (admin trust, a known device, and a
// sudo token for every write). The guard only matches exact paths, so ids travel in the body or the
// query string. Every write is audited twice: pool_audit and the auth package's auth_audit.
import type { AdminRoute } from "../../../auth/src/index.js";
import { cleanText } from "../../../auth/src/validate.ts";
import { ALIVE_SQL, assertPoolKey, audit, DEFAULT_LIMITS, poolKeyId, FLAG_SEVERITY_SQL, ID_RE, json, limitOf, METRICS, newId, notifyHeld, PoolError, inQKeysOf, READ_HEAD_COST, SHARED_SQL, usageSql, type Ctx, type Metric } from "./shared.ts";
import { blockPlan, checkPreview, holdSnapshot, checkToken, PREVIEW_MS, previewToken, readSelection, releaseOrphanHolds, selectedBlobs, selectedMatch, tokenFor } from "./blocking.ts";
import { currentServesBlob, itemServesBlob, servedBlobsOf } from "./served.ts";
import { keyLeakQualifies, voidEvidence } from "./moderation.ts";
import { readHold, streamedPayload } from "./read.ts";
import { adminUnremove, decideAppeal, LINEAGE_COPIES_MAX, lineageCopies, purgeHeldCopies, rightfulCheck, privacyPurge, privacyRestore, purgeItem, restore, reverseUphold, takedown, UNDO_MS, upholdTakedown } from "./moderation.ts";

const HOLD_REASONS = ["reports", "student_data", "blocklist", "admin"] as const;
/** A takedown's re-upload blocks last this long unless the admin chooses block_days (round 8 I3). */
export const DMCA_BLOCK_DEFAULT_DAYS = 60;

function id(v: unknown, field: string): string {
  if (typeof v !== "string" || !ID_RE.test(v)) throw new PoolError(400, "invalid_request", { field });
  return v;
}
function note(v: unknown): string | null {
  if (v === undefined || v === null || v === "") return null;
  const s = cleanText(v, 1000);
  if (!s) throw new PoolError(400, "invalid_request", { field: "note" });
  return s;
}
const fail = (e: unknown): Response => {
  if (e instanceof PoolError) return json(e.status, { error: e.code, ...e.extra }, e.headers);
  throw e;
};

export function poolAdminRoutes(ctx: () => Ctx | null): AdminRoute[] {
  const wrap = (r: AdminRoute): AdminRoute => ({
    ...r,
    handler: async (req, h) => {
      try {
        if (!ctx()) return json(503, { error: "pool_not_configured" });
        // (Round 23 B1: no admin write under a key other than the pool's own.)
        if (r.method !== "GET") await assertPoolKey(ctx()!);
        return await r.handler(req, h);
      } catch (e) {
        return fail(e);
      }
    },
  });
  const C = () => ctx()!;
  const routes: AdminRoute[] = [
    {
      method: "GET",
      path: "/admin/pool/reports",
      write: false,
      // Open reports grouped by item, oldest first, with the reporters and their history. Round 21
      // (critic27b): paged by a cursor (`after` = the last report's at:id), never only the first 500.
      handler: async (req) => {
        const cur = /^(\d{1,16}):([0-9a-f]{32})$/.exec(new URL(req.url).searchParams.get("after") ?? "");
        const rows = await C()
          .db.prepare(
            `SELECT r.id, r.item_id, r.reason, r.note, r.at, r.reporter_id, ru.display_name AS reporter_name,
                    (SELECT COUNT(*) FROM reports r2 WHERE r2.reporter_id = r.reporter_id) AS reporter_reports,
                    (SELECT COUNT(*) FROM reports r3 WHERE r3.reporter_id = r.reporter_id AND r3.action = 'dismissed') AS reporter_dismissed,
                    (SELECT COUNT(DISTINCT im.item_id) FROM imports im WHERE im.user_id = r.reporter_id AND im.kind = 'import') AS reporter_imports,
                    (SELECT COUNT(*) FROM items it WHERE it.author_id = r.reporter_id) AS reporter_items,
                    (SELECT created_at FROM users WHERE id = r.reporter_id) AS reporter_since,
                    i.title, i.author_id, (SELECT json_group_array(reason) FROM item_holds h WHERE h.item_id = r.item_id) AS holds
               FROM reports r JOIN items i ON i.id = r.item_id LEFT JOIN users ru ON ru.id = r.reporter_id
              WHERE r.resolved_at IS NULL AND (?1 IS NULL OR r.at > ?1 OR (r.at = ?1 AND r.id > ?2)) ORDER BY r.at, r.id LIMIT 500`,
          )
          .bind(cur ? Number(cur[1]) : null, cur ? cur[2] : "")
          .all<Record<string, unknown>>();
        const by = new Map<string, { item_id: string; title: unknown; author_id: unknown; holds: unknown; reports: unknown[] }>();
        for (const r of rows.results) {
          const k = r.item_id as string;
          if (!by.has(k)) by.set(k, { item_id: k, title: r.title, author_id: r.author_id, holds: JSON.parse((r.holds as string) ?? "[]"), reports: [] });
          by.get(k)!.reports.push({ id: r.id, reason: r.reason, note: r.note, at: r.at, reporter_id: r.reporter_id, reporter_name: r.reporter_name, reporter_reports: r.reporter_reports, reporter_dismissed: r.reporter_dismissed, reporter_imports: r.reporter_imports, reporter_items: r.reporter_items, reporter_since: r.reporter_since });
        }
        const last = rows.results[rows.results.length - 1];
        return json(200, { items: [...by.values()], next_cursor: rows.results.length === 500 && last ? `${last.at}:${last.id}` : null });
      },
    },
    {
      method: "GET",
      path: "/admin/pool/flags",
      write: false,
      // Open flags, most severe first, then newest (round 8 I6d), paged: ?limit (<= 200) and ?cursor
      // (the last row's "severity:at:id"). Never only "the oldest 500".
      handler: async (req) => {
        const u = new URL(req.url);
        const limit = Math.min(200, Math.max(1, Number(u.searchParams.get("limit") ?? 100) || 100));
        const cur = u.searchParams.get("cursor");
        const m = cur ? /^(\d{1,2}):(\d{1,16}):([0-9a-f]{32})$/.exec(cur) : null;
        if (cur && !m) throw new PoolError(400, "invalid_request", { field: "cursor" });
        const rows = await C()
          .db.prepare(
            `SELECT * FROM (SELECT id, kind, item_id, publish_id, user_id, detail, at, ${FLAG_SEVERITY_SQL} AS severity FROM admin_flags WHERE resolved_at IS NULL)
              WHERE ?1 IS NULL OR severity > ?1 OR (severity = ?1 AND (at < ?2 OR (at = ?2 AND id < ?3)))
              ORDER BY severity, at DESC, id DESC LIMIT ?4`,
          )
          .bind(m ? Number(m[1]) : null, m ? Number(m[2]) : null, m ? m[3] : null, limit + 1)
          .all<{ id: string; at: number; severity: number }>();
        const page = rows.results.slice(0, limit);
        const last = page[page.length - 1];
        const total = (await C().db.prepare(`SELECT COUNT(*) AS n FROM admin_flags WHERE resolved_at IS NULL`).first<{ n: number }>())?.n ?? 0;
        return json(200, { flags: page, total_open: total, next_cursor: rows.results.length > limit && last ? `${last.severity}:${last.at}:${last.id}` : null });
      },
    },
    {
      method: "GET",
      path: "/admin/pool/item",
      write: false,
      // The full form, in any state (quarantine included). Rendered by the admin console as plain text.
      // Streamed from R2 under a read reservation (round 9), never loaded whole.
      handler: async (req, h) => {
        const c = C();
        const itemId = id(new URL(req.url).searchParams.get("id"), "id");
        const item = await c.db.prepare(`SELECT * FROM items WHERE id = ?1`).bind(itemId).first<Record<string, unknown>>();
        if (!item) throw new PoolError(404, "not_found");
        const pubs = await c.db
          .prepare(
            `SELECT p.publish_id, p.version, p.status, p.hide_key, p.license, p.attested_at, p.attest_text_rev, p.full_key, p.committed_at, a.at AS terms_attested_at, a.terms_version
               FROM item_publishes p LEFT JOIN attestations a ON a.publish_id = p.publish_id WHERE p.item_id = ?1 ORDER BY p.version DESC`,
          )
          .bind(itemId)
          .all<{ publish_id: string; full_key: string }>();
        const holds = await c.db.prepare(`SELECT reason, at, by, source FROM item_holds WHERE item_id = ?1`).bind(itemId).all();
        const dmca = await c.db
          .prepare(`SELECT id, notice_id, disabled_at, counter_notice_id, restore_not_before, court_action_at, restored_at, purged_at, outcome, rightful_item_id FROM dmca_actions WHERE item_id = ?1 ORDER BY disabled_at DESC`)
          .bind(itemId)
          .all();
        const current = pubs.results.find((p) => p.publish_id === item.current_publish_id);
        const head = JSON.stringify({ item, publishes: pubs.results.map(({ full_key: _k, ...rest }) => rest), holds: holds.results, dmca_actions: dmca.results });
        const release = readHold(c, req, h.admin.userId);
        try {
          release.grow(READ_HEAD_COST * head.length);
          const body = current ? await c.r2.get(current.full_key) : null;
          if (!body) {
            release();
            return json(200, { ...(JSON.parse(head) as object), payload_full: null });
          }
          return streamedPayload(head, body, release, {}, undefined, undefined, "payload_full");
        } catch (e) {
          release();
          throw e;
        }
      },
    },
    {
      method: "POST",
      path: "/admin/pool/hold",
      write: true,
      handler: async (_req, h) => {
        const c = C();
        const itemId = id(h.body?.item_id, "item_id");
        const n = note(h.body?.note);
        const [ins] = await c.db.batch([
          c.db.prepare(`INSERT OR IGNORE INTO item_holds (item_id, reason, at, by, source) SELECT ?1, 'admin', ?2, ?3, 'manual' WHERE EXISTS (SELECT 1 FROM items WHERE id = ?1)`).bind(itemId, h.now, h.admin.userId),
          audit(c.db, { actorId: h.admin.userId, actorKind: "admin", action: "item.hold", itemId, note: n }, h.now),
          h.audit({ action: "pool.hold", targetId: itemId, note: n }),
        ]);
        return json(200, { ok: true, changed: (ins!.meta.changes ?? 0) === 1 });
      },
    },
    {
      method: "POST",
      path: "/admin/pool/release",
      write: true,
      // Lifting a hold; for student_data it is the admin rejecting the report. Source-aware (round 6
      // F4): by default only the holds human judgements placed (reports, a manual hold, and those
      // carried to an evasion copy) go. Privacy, DMCA and needs_reupload holds belong to their own
      // routes (unblock, withdraw, release-copies, a republish); lifting one here needs its exact
      // `source` and `force: true`, and is audited as forced.
      handler: async (_req, h) => {
        const c = C();
        const itemId = id(h.body?.item_id, "item_id");
        const reason = h.body?.reason;
        if (typeof reason !== "string" || !(HOLD_REASONS as readonly string[]).includes(reason)) throw new PoolError(400, "invalid_request", { field: "reason" });
        const source = h.body?.source;
        if (source !== undefined && (typeof source !== "string" || !/^(|manual|privacy|privacy_near|probe_cap|key_leak|needs_reupload|dmca:[0-9a-f]{32}|carried:[0-9a-f]{32})$/.test(source))) throw new PoolError(400, "invalid_request", { field: "source" });
        const human = (x: string) => x === "" || x === "manual" || x === "key_leak" || x.startsWith("carried:");
        if (source !== undefined && !human(source) && h.body?.force !== true)
          throw new PoolError(409, "hold_has_own_route", { message: "That hold follows a block or takedown: lift it with its own route, or send force: true." });
        const n = note(h.body?.note);
        const resolve =
          reason === "student_data"
            ? `reason = 'student_data'`
            : reason === "reports"
              ? `reason <> 'student_data'`
              : `0`;
        const [del] = await c.db.batch([
          source === undefined
            ? c.db.prepare(`DELETE FROM item_holds WHERE item_id = ?1 AND reason = ?2 AND (source IN ('', 'manual', 'key_leak') OR source LIKE 'carried:%')`).bind(itemId, reason)
            : c.db.prepare(`DELETE FROM item_holds WHERE item_id = ?1 AND reason = ?2 AND source = ?3`).bind(itemId, reason, source),
          c.db.prepare(`UPDATE reports SET resolved_at = ?2, resolved_by = ?3, action = 'dismissed' WHERE item_id = ?1 AND resolved_at IS NULL AND ${resolve}`).bind(itemId, h.now, h.admin.userId),
          // Resolved: the keys a deleted held item left stop holding (round 16).
          c.db.prepare(`DELETE FROM carried_keys WHERE item_id = ?1 AND reason = ?2`).bind(itemId, reason),
          audit(c.db, { actorId: h.admin.userId, actorKind: "admin", action: source !== undefined && !human(source) ? "item.release_forced" : "item.release", itemId, detail: { reason, source: source ?? "human" }, note: n }, h.now),
          h.audit({ action: "pool.release", targetId: itemId, detail: { reason, source: source ?? "human" }, note: n }),
        ]);
        const left = await c.db.prepare(`SELECT reason, source FROM item_holds WHERE item_id = ?1 ORDER BY reason, source`).bind(itemId).all<{ reason: string; source: string }>();
        return json(200, { ok: true, released: del!.meta.changes ?? 0, remaining: left.results });
      },
    },
    {
      method: "POST",
      path: "/admin/pool/release-hold",
      write: true,
      // Round 12 M4: release ONE hold of one item, whatever placed it (a student-data or copyright
      // block that hit an innocent item), with the admin's reason (required, audited). The block keys
      // the item uses now are recorded (hold_releases): its later publishes are not held again by
      // them; new blocked material still holds. Never automatic: student-data holds are only ever
      // released this way or by an unblock.
      handler: async (_req, h) => {
        const c = C();
        const itemId = id(h.body?.item_id, "item_id");
        const source = h.body?.source;
        if (typeof source !== "string" || !/^(|manual|privacy|privacy_near|probe_cap|key_leak|needs_reupload|dmca:[0-9a-f]{32}|carried:[0-9a-f]{32})$/.test(source)) throw new PoolError(400, "invalid_request", { field: "source" });
        const n = note(h.body?.note);
        if (!n || n.length < 10) throw new PoolError(400, "invalid_request", { field: "note", message: "Say why this item is released (at least 10 characters)." });
        const it = await c.db.prepare(`SELECT current_publish_id AS pub FROM items WHERE id = ?1 AND purged_at IS NULL`).bind(itemId).first<{ pub: string | null }>();
        if (!it) return json(404, { error: "not_found" });
        // Round 16 (critic21d 7): the release names the exact publish the admin reviewed (like a
        // purge or takedown confirm): one published since is not released unseen.
        if (typeof h.body?.publish_id !== "string") throw new PoolError(400, "invalid_request", { field: "publish_id", message: "Name the publish you reviewed (publish_id)." });
        if (h.body.publish_id !== it.pub) return json(409, { error: "publish_changed", current_publish_id: it.pub });
        const keysSql =
          source === "privacy"
            ? `SELECT pb.hash FROM privacy_blocks pb WHERE (pb.kind = 'match' AND ${inQKeysOf("pb.hash", "?2")}) OR (pb.kind = 'blob' AND pb.hash IN ${servedBlobsOf("?2")})`
            : source.startsWith("dmca:")
              ? `SELECT bh.hash FROM blocked_hashes bh WHERE bh.action_id = substr(?3, 6) AND ((bh.kind = 'match' AND ${inQKeysOf("bh.hash", "?2")}) OR (bh.kind = 'blob' AND bh.hash IN ${servedBlobsOf("?2")}))`
              : `SELECT NULL WHERE 0`;
        const [del] = await c.db.batch([
          c.db.prepare(`DELETE FROM item_holds WHERE item_id = ?1 AND source = ?2`).bind(itemId, source),
          c.db
            .prepare(
              `INSERT INTO hold_releases (item_id, source, keys_json, at, by, note) SELECT ?1, ?3, COALESCE((SELECT json_group_array(hash) FROM (${keysSql})), '[]'), ?4, ?5, ?6
               ON CONFLICT (item_id, source) DO UPDATE SET keys_json = (SELECT json_group_array(DISTINCT v) FROM (SELECT value AS v FROM json_each(hold_releases.keys_json) UNION SELECT value FROM json_each(excluded.keys_json))), at = excluded.at, by = excluded.by, note = excluded.note`,
            )
            .bind(itemId, it.pub, source, h.now, h.admin.userId, n),
          audit(c.db, { actorId: h.admin.userId, actorKind: "admin", action: "item.release_hold", itemId, detail: { source }, note: n }, h.now),
          h.audit({ action: "pool.release_hold", targetId: itemId, detail: { source }, note: n }),
        ]);
        const left = await c.db.prepare(`SELECT reason, source FROM item_holds WHERE item_id = ?1 ORDER BY reason, source`).bind(itemId).all<{ reason: string; source: string }>();
        return json(200, { ok: true, released: del!.meta.changes ?? 0, remaining: left.results });
      },
    },
    {
      method: "POST",
      path: "/admin/pool/dismiss",
      write: true,
      handler: async (_req, h) => {
        const c = C();
        const rid = id(h.body?.report_id, "report_id");
        const n = note(h.body?.note);
        const [upd] = await c.db.batch([
          c.db.prepare(`UPDATE reports SET resolved_at = ?2, resolved_by = ?3, action = 'dismissed' WHERE id = ?1 AND resolved_at IS NULL`).bind(rid, h.now, h.admin.userId),
          // Round 18: a key_leak hold needs two QUALIFYING open reports on its publish; below that it goes
          // (round 19 critic24d D2: the same predicate that placed it).
          c.db
            .prepare(
              `DELETE FROM item_holds WHERE source = 'key_leak' AND item_id = (SELECT item_id FROM reports WHERE id = ?1)
                 AND (SELECT COUNT(DISTINCT r.reporter_id) FROM reports r JOIN users ru ON ru.id = r.reporter_id
                       WHERE ${keyLeakQualifies("item_holds.item_id", "(item_holds.detail ->> '$.publish_id')", "?2")}) < 2`,
            )
            .bind(rid, h.now),
          audit(c.db, { actorId: h.admin.userId, actorKind: "admin", action: "report.dismiss", detail: { report_id: rid }, note: n }, h.now),
          h.audit({ action: "pool.report_dismiss", targetId: rid, note: n }),
        ]);
        return json((upd!.meta.changes ?? 0) === 1 ? 200 : 404, (upd!.meta.changes ?? 0) === 1 ? { ok: true } : { error: "not_found" });
      },
    },
    {
      method: "POST",
      path: "/admin/pool/remove",
      write: true,
      // Removal for other policy reasons: hidden now, purged after 14 days.
      handler: async (_req, h) => {
        const c = C();
        const itemId = id(h.body?.item_id, "item_id");
        const n = note(h.body?.note);
        const [upd] = await c.db.batch([
          c.db
            .prepare(
              `UPDATE items SET prior_removed_reason = removed_reason, prior_purge_after = purge_after, removed_at = COALESCE(removed_at, ?2),
                 removed_reason = 'admin', purge_after = ?3, updated_at = ?2 WHERE id = ?1 AND purged_at IS NULL AND (removed_reason IS NULL OR removed_reason = 'author_deleted')`,
            )
            .bind(itemId, h.now, h.now + UNDO_MS),
          c.db.prepare(`UPDATE reports SET resolved_at = ?2, resolved_by = ?3, action = 'removed' WHERE item_id = ?1 AND resolved_at IS NULL AND changes() = 1`).bind(itemId, h.now, h.admin.userId),
          audit(c.db, { actorId: h.admin.userId, actorKind: "admin", action: "item.remove", itemId, note: n }, h.now),
          h.audit({ action: "pool.remove", targetId: itemId, note: n }),
        ]);
        return (upd!.meta.changes ?? 0) === 1 ? json(200, { ok: true }) : json(409, { error: "not_removable" });
      },
    },
    {
      method: "POST",
      path: "/admin/pool/unremove",
      write: true,
      // Reverses an admin removal within its 14 days, back to what it replaced (live, or the
      // author's own deletion). Holds stay; a privacy block on its content adds one; going live is
      // subject to the author's quota and question cap, like an undelete (critic C6).
      handler: async (_req, h) => {
        const c = C();
        const itemId = id(h.body?.item_id, "item_id");
        const n = note(h.body?.note);
        const res = await adminUnremove(c, itemId, { id: h.admin.userId, kind: "admin" }, n);
        if (res === "not_found") return json(404, { error: "not_found" });
        if (res === "ok") {
          await c.db.batch([h.audit({ action: "pool.unremove", targetId: itemId, note: n })]);
          return json(200, { ok: true });
        }
        return json(409, { error: "over_limits", message: "Restoring it would take the author over their storage quota or question cap." });
      },
    },
    {
      method: "POST",
      path: "/admin/pool/purge-preview",
      write: true,
      // What a privacy purge would block and hold; the purge must quote its confirm_preview token.
      handler: async (_req, h) => {
        const c = C();
        const plan = await blockPlan(c, id(h.body?.item_id, "item_id"), readSelection(h.body), "privacy");
        return json(200, { ...plan, large: plan.affected_total > plan.affected.length, confirm_preview: await previewToken(c, "privacy", plan, { admin: h.admin.userId }), expires_in: PREVIEW_MS / 1000 });
      },
    },
    {
      method: "POST",
      path: "/admin/dmca/takedown-preview",
      write: true,
      handler: async (_req, h) => {
        const c = C();
        const plan = await blockPlan(c, id(h.body?.item_id, "item_id"), readSelection(h.body), "dmca", typeof h.body?.rightful_item_id === "string" ? h.body.rightful_item_id : null);
        // The token binds the admin, the notice and the block's end too: the confirm acts on exactly this.
        const bind = { admin: h.admin.userId, notice_id: h.body?.notice_id ?? null, block_days: h.body?.block_days ?? null, exempt: exemptList(h.body?.exempt_authors), rightful: h.body?.rightful_item_id ?? null };
        // Round 11 P4: for each blocked hash, the earliest publish in the rightful item and in this one.
        const rc = typeof h.body?.rightful_item_id === "string" && ID_RE.test(h.body.rightful_item_id) ? await rightfulCheck(c, h.body.rightful_item_id, plan.item_id, { match: selectedMatch(plan), blob: selectedBlobs(plan) }) : null;
        return json(200, { ...plan, ...(rc ? { rightful_check: rc } : {}), copies: await lineageCopies(c, plan.item_id, selectedMatch(plan), false, selectedBlobs(plan)), confirm_preview: await previewToken(c, "dmca", plan, bind), expires_in: PREVIEW_MS / 1000 });
      },
    },
    {
      method: "POST",
      path: "/admin/pool/evidence",
      write: true,
      // Void or restore licence/attribution evidence: everything copied from an item, or everything
      // an account was served (e.g. an infringer's item, or a sock puppet's reads). Audited.
      handler: async (_req, h) => {
        const c = C();
        const itemId = h.body?.item_id === undefined ? undefined : id(h.body.item_id, "item_id");
        const userId = h.body?.user_id;
        if (userId !== undefined && (typeof userId !== "string" || !/^[0-9a-f-]{36}$/.test(userId))) throw new PoolError(400, "invalid_request", { field: "user_id" });
        if ((itemId === undefined) === (userId === undefined)) throw new PoolError(400, "invalid_request", { field: "item_id" });
        const act = h.body?.action;
        if (act !== "void" && act !== "restore") throw new PoolError(400, "invalid_request", { field: "action" });
        const n = note(h.body?.note);
        // Voiding an item may name the rightful author's item that its links stand for (round 7 M1).
        const rb = h.body?.replaced_by === undefined || h.body?.replaced_by === null ? null : id(h.body.replaced_by, "replaced_by");
        if (rb !== null && (itemId === undefined || act !== "void")) throw new PoolError(400, "invalid_request", { field: "replaced_by" });
        // Round 11 P4: the same validation as a takedown's rightful item (over the material they share).
        if (rb !== null && itemId !== undefined) {
          const rc = await rightfulCheck(c, rb, itemId, { match: [], blob: [] });
          if (!rc.ok) throw new PoolError(409, "rightful_item_invalid", { reasons: rc.reasons, hashes: rc.hashes.filter((x) => !x.ok).slice(0, 50) });
        }
        if (rb !== null && itemId !== undefined)
          await c.db
            .prepare(
              `UPDATE void_items SET replaced_by = ?2, replaced_license = (SELECT license FROM items WHERE id = ?2),
                 replaced_author_id = (SELECT author_id FROM items WHERE id = ?2) WHERE item_id = ?1`,
            )
            .bind(itemId, rb)
            .run();
        const [upd] = await c.db.batch([
          ...voidEvidence(c.db, { itemId, userId: userId as string | undefined }, h.now, act === "restore", h.admin.userId, "admin", rb),
          audit(c.db, { actorId: h.admin.userId, actorKind: "admin", action: `evidence.${act}`, itemId: itemId ?? null, targetUserId: (userId as string | undefined) ?? null, note: n }, h.now),
          h.audit({ action: `pool.evidence_${act}`, targetId: itemId ?? (userId as string), note: n }),
        ]);
        return json(200, { ok: true, rows: upd!.meta.changes ?? 0 });
      },
    },
    {
      method: "POST",
      path: "/admin/pool/unblock",
      write: true,
      // Lifts one privacy block (a false positive); holds that only it caused are released.
      handler: async (_req, h) => {
        const c = C();
        const kind = h.body?.kind;
        // "hash", or "sha256" as the purge preview names an image (round 8: both accepted).
        const hash = h.body?.hash ?? h.body?.sha256;
        // A block key (round 12: `n:`, `f:`, `e:`, `a:` keys too), or a question's match_hash: then every
        // key blocked for that question goes (the preview names questions by match_hash).
        if ((kind !== "match" && kind !== "blob") || typeof hash !== "string" || !BLOCK_KEY_RE.test(hash)) throw new PoolError(400, "invalid_request", { field: "hash" });
        const n = note(h.body?.note);
        const blk = await c.db
          .prepare(`SELECT MAX(bytes_purged_at) AS bytes_purged_at, MIN(item_id) AS item_id FROM privacy_blocks WHERE kind = ?1 AND (hash = ?2 OR qhash = ?2) HAVING COUNT(*) > 0`)
          .bind(kind, hash)
          .first<{ bytes_purged_at: number | null; item_id: string }>();
        if (!blk) return json(404, { error: "not_found" });
        // Never a silent loss (round 5 E5): within the grace window the bytes are intact and the unblock
        // restores everything; after it, every copy that used the image is held as needs_reupload,
        // flagged for the admin, and named in the answer.
        const lost = blk.bytes_purged_at !== null;
        const needs = lost
          ? (
              await c.db
                .prepare(
                  // Only items whose CURRENT publish SERVES the image (served.ts): a hidden full-form use is
                  // its author's private material and never becomes a hold others can see.
                  `SELECT DISTINCT i.id FROM items i WHERE i.purged_at IS NULL AND ${currentServesBlob("i", (sha) => `${sha} = ?1`)} ORDER BY i.id`,
                )
                .bind(hash)
                .all<{ id: string }>()
            ).results.map((r) => r.id)
          : [];
        await c.db.batch([
          c.db.prepare(`DELETE FROM privacy_blocks WHERE kind = ?1 AND (hash = ?2 OR qhash = ?2)`).bind(kind, hash),
          c.db.prepare(`DELETE FROM privacy_bands WHERE qhash NOT IN (SELECT qhash FROM privacy_blocks WHERE qhash IS NOT NULL)`),
          c.db
            .prepare(`INSERT OR IGNORE INTO item_holds (item_id, reason, at, by, source) SELECT value, 'admin', ?1, ?2, 'needs_reupload' FROM json_each(?3)`)
            .bind(h.now, h.admin.userId, JSON.stringify(needs)),
          c.db
            .prepare(`INSERT INTO admin_flags (id, kind, item_id, detail, at) SELECT ?1, 'needs_reupload', NULL, ?2, ?3 WHERE ?4 > 0`)
            .bind(newId(), JSON.stringify({ hash, items: needs }), h.now, needs.length),
          audit(c.db, { actorId: h.admin.userId, actorKind: "admin", action: "privacy.unblock", detail: { kind, hash, bytes_lost: lost, needs_reupload: needs.length }, note: n }, h.now),
          h.audit({ action: "pool.privacy_unblock", targetId: hash, note: n }),
        ]);
        const released = await releaseOrphanHolds(c, "privacy", "student_data");
        // The purged item's last own block lifted within its grace window: the item itself is put back (round 7 M2).
        const left = await c.db.prepare(`SELECT 1 AS x FROM privacy_blocks WHERE item_id = ?1`).bind(blk.item_id).first();
        const restored = !left && (await privacyRestore(c, { id: h.admin.userId, kind: "admin" }, blk.item_id, n));
        return json(200, { ok: true, released, bytes_lost: lost, needs_reupload: needs, item_restored: restored });
      },
    },
    {
      method: "POST",
      path: "/admin/dmca/unblock",
      write: true,
      // Lifts an action's re-upload blocks (all, or one hash), upheld or not; holds that only they caused are released.
      handler: async (_req, h) => {
        const c = C();
        const aid = id(h.body?.action_id, "action_id");
        const hash = h.body?.hash;
        if (hash !== undefined && (typeof hash !== "string" || !BLOCK_KEY_RE.test(hash))) throw new PoolError(400, "invalid_request", { field: "hash" });
        const n = note(h.body?.note);
        const [upd] = await c.db.batch([
          c.db.prepare(`UPDATE blocked_hashes SET lifted_at = ?2 WHERE action_id = ?1 AND lifted_at IS NULL AND (?3 IS NULL OR hash = ?3 OR qhash = ?3)`).bind(aid, h.now, hash ?? null),
          audit(c.db, { actorId: h.admin.userId, actorKind: "admin", action: "dmca.unblock", detail: { action_id: aid, hash: hash ?? "all" }, note: n }, h.now, `changes() > 0`),
          h.audit({ action: "pool.dmca_unblock", targetId: aid, note: n }),
        ]);
        return json(200, { ok: true, lifted: upd!.meta.changes ?? 0, released: await releaseOrphanHolds(c, `dmca:${aid}`, "blocklist") });
      },
    },
    {
      method: "POST",
      path: "/admin/pool/privacy-restore",
      write: true,
      // An upheld appeal (or any wrong privacy decision) within the 14-day grace window: every block
      // the item caused is lifted and the item is put back, same id, versions and chains.
      handler: async (_req, h) => {
        const c = C();
        const itemId = id(h.body?.item_id, "item_id");
        const n = note(h.body?.note);
        const ok = await privacyRestore(c, { id: h.admin.userId, kind: "admin" }, itemId, n);
        if (ok) await c.db.batch([h.audit({ action: "pool.privacy_restore", targetId: itemId, note: n })]);
        return ok ? json(200, { ok: true }) : json(409, { error: "not_restorable", message: "Only a privacy removal within its 14-day grace window, or with an open appeal, can be restored." });
      },
    },
    {
      method: "POST",
      path: "/admin/pool/purge",
      write: true,
      // Confirmed student data: purged immediately (no quarantine), image hashes blocklisted.
      handler: async (_req, h) => {
        const c = C();
        const itemId = id(h.body?.item_id, "item_id");
        const n = note(h.body?.note);
        const plan = await blockPlan(c, itemId, readSelection(h.body), "privacy");
        // More affected items than one list: fails closed until the admin confirms the size (checked
        // before the token is consumed, so the admin can confirm with the same preview).
        if (plan.affected_total > plan.affected.length && h.body?.confirm_large !== true)
          throw new PoolError(428, "confirm_large_required", { affected_total: plan.affected_total, message: "This block holds more items than are listed; confirm with confirm_large: true." });
        // The preview token is checked BEFORE anything is written (round 7): no audit row for a refused confirm.
        await checkPreview(c, "privacy", plan, h.body?.confirm_preview, { admin: h.admin.userId });
        // Round 10: the admin must tick what holds student data (no blanket default).
        if (selectedMatch(plan).length === 0 && selectedBlobs(plan).length === 0)
          throw new PoolError(400, "selection_required", { message: "Tick the questions or images that contain student data." });
        await c.db.batch([h.audit({ action: "pool.purge_privacy", targetId: itemId, note: n })]);
        const ok = await privacyPurge(c, h.admin.userId, itemId, plan);
        return ok
          ? json(200, { ok: true, blocked: { questions: plan.match.filter((x) => x.selected).length, keys: selectedMatch(plan).length, images: selectedBlobs(plan).length }, held: plan.affected.map((a) => a.item_id), held_total: plan.affected_total })
          : json(409, { error: "not_purgeable" });
      },
    },
    // Round 10: the DMCA records, paged newest first (`?limit&cursor` where cursor is `at:id`).
    ...(
      [
        ["/admin/dmca/notices", `SELECT id, received_at AS at, complainant_name, complainant_contact, work_desc, material_desc, status, recorded_by FROM dmca_notices`],
        // (round 23: with the taken-down item's latest publishing-terms attestation, date and version)
        ["/admin/dmca/actions", `SELECT id, disabled_at AS at, notice_id, item_id, author_id, counter_notice_id, restore_not_before, restore_by, court_action_at, restored_at, purged_at, outcome, rightful_item_id,
                                        (SELECT json_object('at', t.at, 'terms_version', t.terms_version) FROM attestations t WHERE t.item_id = dmca_actions.item_id AND t.at <= dmca_actions.disabled_at ORDER BY t.at DESC LIMIT 1) AS attestation FROM dmca_actions`],
        ["/admin/dmca/counter-notices", `SELECT id, received_at AS at, action_id, name, address, phone, signature, material_desc, forwarded_at FROM dmca_counter_notices`],
      ] as const
    ).map(([path, sql]) => ({
      method: "GET" as const,
      path,
      write: false,
      handler: async (req: Request) => {
        const c = C();
        const u = new URL(req.url).searchParams;
        const limit = Math.min(Math.max(Number(u.get("limit") ?? 50) || 50, 1), 200);
        const cur = /^(\d+):([0-9a-f]{32})$/.exec(u.get("cursor") ?? "");
        const rows = await c.db
          .prepare(`SELECT * FROM (${sql}) WHERE (?1 IS NULL OR at < ?1 OR (at = ?1 AND id < ?2)) ORDER BY at DESC, id DESC LIMIT ?3`)
          .bind(cur ? Number(cur[1]) : null, cur ? cur[2] : "", limit + 1)
          .all<{ id: string; at: number }>();
        const page = rows.results.slice(0, limit);
        const last = page[page.length - 1];
        return json(200, { items: page, next_cursor: rows.results.length > limit && last ? `${last.at}:${last.id}` : null });
      },
    })),
    {
      method: "POST",
      path: "/admin/pool/purge-held-copies",
      write: true,
      // Round 9: the copies a confirmed student-data purge holds, listed (no confirm), then removed in
      // bulk (confirm: true), each quarantined for the grace window like the original.
      handler: async (_req, h) => {
        const c = C();
        const itemId = id(h.body?.item_id, "item_id");
        const confirm = h.body?.confirm === true;
        const r = await purgeHeldCopies(c, h.admin.userId, itemId, confirm);
        if (confirm) await c.db.batch([h.audit({ action: "pool.purge_held_copies", targetId: itemId, note: `${r.removed} removed` })]);
        return json(200, r);
      },
    },
    {
      method: "POST",
      path: "/admin/pool/appeal-decision",
      write: true,
      // Round 9 J5: an open appeal stops the purge until the admin decides here; unanswered for 60
      // days, the item is restored (the cron). "purge" confirms the removal (purged at its date).
      handler: async (_req, h) => {
        const c = C();
        const itemId = id(h.body?.item_id, "item_id");
        const decision = h.body?.decision;
        if (decision !== "restore" && decision !== "purge") throw new PoolError(400, "invalid_request", { field: "decision" });
        const n = note(h.body?.note);
        const res = await decideAppeal(c, itemId, decision, { id: h.admin.userId, kind: "admin" }, n);
        if (res === "ok") await c.db.batch([h.audit({ action: `pool.appeal_${decision}`, targetId: itemId, note: n })]);
        return res === "ok" ? json(200, { ok: true }) : res === "over_limits" ? json(409, { error: "over_limits" }) : json(404, { error: "not_found" });
      },
    },
    {
      method: "POST",
      path: "/admin/pool/resolve-flag",
      write: true,
      handler: async (_req, h) => {
        const c = C();
        const fid = id(h.body?.flag_id, "flag_id");
        await c.db.batch([
          c.db.prepare(`UPDATE admin_flags SET resolved_at = ?2, resolved_by = ?3 WHERE id = ?1 AND resolved_at IS NULL`).bind(fid, h.now, h.admin.userId),
          h.audit({ action: "pool.flag_resolve", targetId: fid }),
        ]);
        return json(200, { ok: true });
      },
    },
    {
      method: "PUT",
      path: "/admin/pool/limits",
      write: true,
      handler: async (_req, h) => {
        const c = C();
        const user = h.body?.user_id;
        if (user !== "*" && (typeof user !== "string" || !/^[0-9a-f-]{36}$/.test(user))) throw new PoolError(400, "invalid_request", { field: "user_id" });
        const metric = h.body?.metric;
        if (typeof metric !== "string" || !(METRICS as string[]).includes(metric)) throw new PoolError(400, "invalid_request", { field: "metric" });
        const value = h.body?.value;
        if (typeof value !== "number" || !Number.isInteger(value) || value < 0 || value > 1e12) throw new PoolError(400, "invalid_request", { field: "value" });
        await c.db.batch([
          c.db
            .prepare(`INSERT INTO pool_limits (user_id, metric, value, updated_at, updated_by) SELECT ?1, ?2, ?3, ?4, ?5 WHERE (?1 = '*' OR ${ALIVE_SQL("?1")}) ON CONFLICT (user_id, metric) DO UPDATE SET value = ?3, updated_at = ?4, updated_by = ?5`)
            .bind(user, metric, value, h.now, h.admin.userId),
          audit(c.db, { actorId: h.admin.userId, actorKind: "admin", action: "limits.set", targetUserId: user === "*" ? null : user, detail: { metric, value, default: DEFAULT_LIMITS[metric as Metric] } }, h.now),
          h.audit({ action: "pool.limits", targetId: user === "*" ? null : user, detail: { metric, value } }),
        ]);
        return json(200, { ok: true });
      },
    },
    {
      method: "POST",
      path: "/admin/dmca/notices",
      write: true,
      // One field or tick per 512(c)(3)(A) element; a notice missing one is stored as deficient.
      handler: async (_req, h) => {
        const c = C();
        const b = h.body ?? {};
        const text = (k: string, max: number) => {
          const v = cleanText(b[k], max);
          if (!v) throw new PoolError(400, "invalid_request", { field: k });
          return v;
        };
        const name = text("complainant_name", 200);
        const contact = text("complainant_contact", 500);
        const work = text("work_desc", 2000);
        const material = text("material_desc", 2000);
        const sig = b.has_signature === true;
        const gf = b.has_good_faith === true;
        const pj = b.has_perjury_stmt === true;
        const nid = newId();
        const status = sig && gf && pj ? "open" : "deficient";
        await c.db.batch([
          c.db
            .prepare(
              `INSERT INTO dmca_notices (id, received_at, complainant_name, complainant_contact, work_desc, material_desc, has_signature, has_good_faith, has_perjury_stmt, status, recorded_by)
               VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)`,
            )
            .bind(nid, h.now, name, contact, work, material, sig ? 1 : 0, gf ? 1 : 0, pj ? 1 : 0, status, h.admin.userId),
          audit(c.db, { actorId: h.admin.userId, actorKind: "admin", action: "dmca.notice", detail: { notice_id: nid, status } }, h.now),
          h.audit({ action: "pool.dmca_notice", targetId: nid, detail: { status } }),
        ]);
        return json(201, { notice_id: nid, status });
      },
    },
    {
      method: "POST",
      path: "/admin/dmca/takedown",
      write: true,
      handler: async (_req, h) => {
        const c = C();
        const nid = id(h.body?.notice_id, "notice_id");
        const itemId = id(h.body?.item_id, "item_id");
        const plan = await blockPlan(c, itemId, readSelection(h.body), "dmca", typeof h.body?.rightful_item_id === "string" ? h.body.rightful_item_id : null);
        const days = h.body?.block_days;
        if (days !== undefined && days !== null && (typeof days !== "number" || !Number.isInteger(days) || days < 1 || days > 3650)) throw new PoolError(400, "invalid_request", { field: "block_days" });
        const exempt = exemptList(h.body?.exempt_authors);
        const rightful = h.body?.rightful_item_id === undefined || h.body?.rightful_item_id === null ? null : id(h.body.rightful_item_id, "rightful_item_id");
        // Rounds 10 L2 / 11 P4: the rightful item must be another author's visible item, not derived from
        // this one, containing every blocked hash in a publish committed before this item's first.
        if (rightful !== null) {
          const rc = await rightfulCheck(c, rightful, itemId, { match: selectedMatch(plan), blob: selectedBlobs(plan) });
          if (!rc.ok)
            throw new PoolError(409, "rightful_item_invalid", { reasons: rc.reasons, hashes: rc.hashes.filter((x) => !x.ok).slice(0, 50), message: "The rightful item must be another author's visible item that had this material first." });
        }
        await checkPreview(c, "dmca", plan, h.body?.confirm_preview, { admin: h.admin.userId, notice_id: nid, block_days: days ?? null, exempt, rightful });
        await c.db.batch([h.audit({ action: "pool.takedown", targetId: itemId, detail: { notice_id: nid, block_days: days ?? null, exempt_authors: exempt } })]);
        // Re-upload blocks expire (round 8 I3): block_days, or DMCA_BLOCK_DEFAULT_DAYS when not given.
        const r = await takedown(c, h.admin.userId, nid, itemId, plan, h.now + (typeof days === "number" ? days : DMCA_BLOCK_DEFAULT_DAYS) * 86_400_000, exempt, rightful);
        // Copies (same questions, or recorded as derived from it) are listed; "hold-copies" hides them.
        // (Round 22 V-b: the first page, and whether there are more.)
        return json(200, { ok: true, action_id: r.actionId, copies: r.copies.slice(0, LINEAGE_COPIES_MAX), copies_more: r.copies.length > LINEAGE_COPIES_MAX });
      },
    },
    {
      method: "POST",
      path: "/admin/dmca/court-action",
      write: true,
      // The complainant's notice of a court action keeps the material disabled until resolved.
      handler: async (_req, h) => {
        const c = C();
        const aid = id(h.body?.action_id, "action_id");
        const [upd] = await c.db.batch([
          c.db.prepare(`UPDATE dmca_actions SET court_action_at = ?2 WHERE id = ?1 AND restored_at IS NULL AND purged_at IS NULL AND court_action_at IS NULL`).bind(aid, h.now),
          audit(c.db, { actorId: h.admin.userId, actorKind: "admin", action: "dmca.court_action", detail: { action_id: aid } }, h.now),
          h.audit({ action: "pool.dmca_court_action", targetId: aid }),
        ]);
        return (upd!.meta.changes ?? 0) === 1 ? json(200, { ok: true }) : json(404, { error: "not_found" });
      },
    },
    {
      method: "POST",
      path: "/admin/dmca/resolve",
      write: true,
      // The way out of a court action: the court sided with the author (restore) or the complainant (remove).
      handler: async (_req, h) => {
        const c = C();
        const aid = id(h.body?.action_id, "action_id");
        const outcome = h.body?.outcome;
        if (outcome !== "restore" && outcome !== "remove" && outcome !== "put_back") throw new PoolError(400, "invalid_request", { field: "outcome" });
        const n = note(h.body?.note);
        // (Round 23 A3: "put_back" reverses an UPHELD action: strike voided, evidence restored, blocks and holds lifted.)
        if (outcome === "put_back") {
          const ok = await reverseUphold(c, h.admin.userId, aid);
          if (ok) await c.db.batch([h.audit({ action: "pool.dmca_resolve", targetId: aid, detail: { outcome }, note: n })]);
          return ok ? json(200, { ok: true }) : json(409, { error: "not_resolvable" });
        }
        const act = await c.db.prepare(`SELECT court_action_at FROM dmca_actions WHERE id = ?1 AND restored_at IS NULL AND purged_at IS NULL`).bind(aid).first<{ court_action_at: number | null }>();
        if (!act) return json(404, { error: "not_found" });
        // (Round 23 A1: the decision is recorded only once it took effect.)
        const ok = outcome === "restore" ? await restore(c, aid, { id: h.admin.userId, kind: "admin" }, "court_restore") : await upholdTakedown(c, h.admin.userId, aid);
        if (ok)
          await c.db.batch([
            audit(c.db, { actorId: h.admin.userId, actorKind: "admin", action: "dmca.resolve", detail: { action_id: aid, outcome }, note: n }, h.now),
            h.audit({ action: "pool.dmca_resolve", targetId: aid, detail: { outcome }, note: n }),
          ]);
        return ok ? json(200, { ok: true }) : json(409, { error: "not_resolvable" });
      },
    },
    {
      method: "POST",
      path: "/admin/dmca/hold-copies-preview",
      write: true,
      // The copies hold-copies would hold (designated questions that K or more unrelated authors use
      // are dropped unless include_common), with the token the confirm must quote.
      handler: async (_req, h) => {
        const c = C();
        const q = await copiesQuery(c, h.body);
        if (!q) return json(404, { error: "not_found" });
        return json(200, { copies: q.copies, dropped_common: q.dropped, confirm_preview: await tokenFor(c, q.digest(h.admin.userId)), expires_in: PREVIEW_MS / 1000 });
      },
    },
    {
      method: "POST",
      path: "/admin/dmca/hold-copies",
      write: true,
      // Hold every previewed copy of the taken-down item, tagged with the action.
      handler: async (_req, h) => {
        const c = C();
        const aid = id(h.body?.action_id, "action_id");
        const q = await copiesQuery(c, h.body);
        if (!q) return json(404, { error: "not_found" });
        await checkToken(c, q.digest(h.admin.userId), h.body?.confirm_preview);
        const copies = q.copies;
        const a = { item_id: q.itemId };
        await c.db.batch([
          c.db
            .prepare(
              `INSERT OR IGNORE INTO item_holds (item_id, reason, at, by, source, detail)
               SELECT j.value, 'admin', ?2, ?3, ?1, ${holdSnapshot("j.value", "(SELECT json_group_array(hash) FROM blocked_hashes WHERE action_id = ?5 AND kind = 'match')")} FROM json_each(?4) j`,
            )
            .bind(`dmca:${aid}`, h.now, h.admin.userId, JSON.stringify(copies.map((x) => x.item_id)), aid),
          audit(c.db, { actorId: h.admin.userId, actorKind: "admin", action: "dmca.hold_copies", itemId: a.item_id, detail: { action_id: aid, copies: copies.length } }, h.now),
          h.audit({ action: "pool.dmca_hold_copies", targetId: aid, detail: { copies: copies.length } }),
        ]);
        await notifyHeld(c, copies.map((x) => x.item_id), "copyright");
        return json(200, { ok: true, held: copies.map((x) => x.item_id), more: q.more });
      },
    },
    {
      method: "GET",
      path: "/admin/dmca/copies",
      write: false,
      // The copies held because of an action (offered for release together after a put-back).
      handler: async (req) => {
        const c = C();
        const aid = id(new URL(req.url).searchParams.get("action_id"), "action_id");
        const a = await c.db.prepare(`SELECT item_id, restored_at FROM dmca_actions WHERE id = ?1`).bind(aid).first<{ item_id: string; restored_at: number | null }>();
        if (!a) return json(404, { error: "not_found" });
        // (Rounds 21-22 D2: paged; each list with its OWN cursor (held_after / candidates_after), so
        // neither skips what the other's page ended past.)
        const sp = new URL(req.url).searchParams;
        const heldAfter = sp.get("held_after") ?? sp.get("after") ?? "";
        const candAfter = sp.get("candidates_after") ?? sp.get("after") ?? "";
        const held = await c.db.prepare(`SELECT item_id, reason FROM item_holds WHERE source = ?1 AND item_id > ?2 ORDER BY item_id LIMIT ${LINEAGE_COPIES_MAX + 1}`).bind(`dmca:${aid}`, heldAfter).all<{ item_id: string }>();
        const heldPage = held.results.slice(0, LINEAGE_COPIES_MAX);
        // (a closed action's candidates are not offered for holding)
        const cand = a.restored_at === null ? await lineageCopies(c, a.item_id, null, false, null, { heldBy: `dmca:${aid}`, after: candAfter }) : [];
        const page = cand.slice(0, LINEAGE_COPIES_MAX);
        return json(200, {
          restored: a.restored_at !== null,
          held: heldPage,
          candidates: page,
          next_held: held.results.length > LINEAGE_COPIES_MAX ? heldPage[heldPage.length - 1]!.item_id : null,
          next_candidates: cand.length > LINEAGE_COPIES_MAX ? page[page.length - 1]!.item_id : null,
        });
      },
    },
    {
      method: "GET",
      path: "/admin/pool/key",
      write: false,
      // Round 23 (critic30a B1): which POOL_HASH_KEY this pool's identity hashes are under. The key is
      // permanent: a different key is refused (503 pool_key_mismatch), never re-keyed.
      handler: async () => {
        const c = C();
        const stored = (await c.db.prepare(`SELECT value FROM pool_kv WHERE key = 'pool_key_id'`).first<{ value: string }>())?.value ?? null;
        const current = await poolKeyId();
        return json(200, { key_id: current, stored_key_id: stored, match: stored === null || stored === current });
      },
    },
    {
      method: "POST",
      path: "/admin/dmca/release-copies",
      write: true,
      handler: async (_req, h) => {
        const c = C();
        const aid = id(h.body?.action_id, "action_id");
        const [del] = await c.db.batch([
          c.db.prepare(`DELETE FROM item_holds WHERE source = ?1`).bind(`dmca:${aid}`),
          audit(c.db, { actorId: h.admin.userId, actorKind: "admin", action: "dmca.release_copies", detail: { action_id: aid } }, h.now),
          h.audit({ action: "pool.dmca_release_copies", targetId: aid }),
        ]);
        return json(200, { ok: true, released: del!.meta.changes ?? 0 });
      },
    },
    {
      method: "POST",
      path: "/admin/dmca/exempt",
      write: true,
      // Names (or un-names) an author whose publishes an action's re-upload blocks never hold: the
      // rightful author of the taken-down material. Adding one releases the holds the action placed
      // on that author's items. Audited.
      handler: async (_req, h) => {
        const c = C();
        const aid = id(h.body?.action_id, "action_id");
        const uid = h.body?.user_id;
        if (typeof uid !== "string" || !/^[0-9a-f-]{36}$/.test(uid)) throw new PoolError(400, "invalid_request", { field: "user_id" });
        const act = h.body?.action;
        if (act !== "add" && act !== "remove") throw new PoolError(400, "invalid_request", { field: "action" });
        const n = note(h.body?.note);
        if (!(await c.db.prepare(`SELECT 1 AS x FROM dmca_actions WHERE id = ?1`).bind(aid).first())) return json(404, { error: "not_found" });
        const [, rel] = await c.db.batch([
          act === "add"
            ? c.db.prepare(`INSERT OR IGNORE INTO dmca_exempt (action_id, user_id, at) VALUES (?1, ?2, ?3)`).bind(aid, uid, h.now)
            : c.db.prepare(`DELETE FROM dmca_exempt WHERE action_id = ?1 AND user_id = ?2`).bind(aid, uid),
          act === "add"
            ? c.db.prepare(`DELETE FROM item_holds WHERE source = ?1 AND item_id IN (SELECT id FROM items WHERE author_id = ?2)`).bind(`dmca:${aid}`, uid)
            : c.db.prepare(`SELECT 0 AS released`),
          audit(c.db, { actorId: h.admin.userId, actorKind: "admin", action: `dmca.exempt_${act}`, targetUserId: uid, detail: { action_id: aid }, note: n }, h.now),
          h.audit({ action: `pool.dmca_exempt_${act}`, targetId: aid, detail: { user_id: uid }, note: n }),
        ]);
        return json(200, { ok: true, released: act === "add" ? (rel!.meta.changes ?? 0) : 0 });
      },
    },
    {
      method: "POST",
      path: "/admin/dmca/withdraw",
      write: true,
      // A withdrawn notice: everything taken down under it is put back.
      handler: async (_req, h) => {
        const c = C();
        const nid = id(h.body?.notice_id, "notice_id");
        const n = note(h.body?.note);
        const exists = await c.db.prepare(`SELECT status FROM dmca_notices WHERE id = ?1`).bind(nid).first<{ status: string }>();
        if (!exists) return json(404, { error: "notice_not_found" });
        await c.db.batch([
          c.db.prepare(`UPDATE dmca_notices SET status = 'withdrawn' WHERE id = ?1`).bind(nid),
          audit(c.db, { actorId: h.admin.userId, actorKind: "admin", action: "dmca.withdraw", detail: { notice_id: nid, was: exists.status }, note: n }, h.now),
          h.audit({ action: "pool.dmca_withdraw", targetId: nid, note: n }),
        ]);
        const acts = await c.db.prepare(`SELECT id FROM dmca_actions WHERE notice_id = ?1 AND restored_at IS NULL AND purged_at IS NULL`).bind(nid).all<{ id: string }>();
        for (const a of acts.results) await restore(c, a.id, { id: h.admin.userId, kind: "admin" }, "withdrawn");
        // (Round 23, critic30b G1: a retry re-runs the release of the holds its blocks caused, idempotently.)
        for (const a of (await c.db.prepare(`SELECT id FROM dmca_actions WHERE notice_id = ?1 AND restored_at IS NOT NULL`).bind(nid).all<{ id: string }>()).results)
          await releaseOrphanHolds(c, `dmca:${a.id}`, "blocklist");
        return json(200, { ok: true, restored: acts.results.length });
      },
    },
  ];
  return routes.map(wrap);
}

export { purgeItem };

/** A block key: a hex hash, or a round-12 prefixed key (`n:`, `f:`, `e:`, `a:`). */
const BLOCK_KEY_RE = /^(?:[nfea]:)?[0-9a-f]{64}$/;

/** The rightful authors named on a takedown (at most 20 user ids). */
function exemptList(v: unknown): string[] {
  if (v === undefined || v === null) return [];
  if (!Array.isArray(v) || v.length > 20 || !v.every((x) => typeof x === "string" && /^[0-9a-f-]{36}$/.test(x))) throw new PoolError(400, "invalid_request", { field: "exempt_authors" });
  return [...new Set(v as string[])].sort();
}

/**
 * The copies hold-copies acts on: lineage copies of the action's item that copy it whole or a
 * designated question; designated questions that K or more authors WITHOUT lineage to the item use
 * (a common question) are dropped unless include_common.
 */
async function copiesQuery(c: Ctx, b: Record<string, unknown> | null) {
  const aid = id(b?.action_id, "action_id");
  const a = await c.db.prepare(`SELECT item_id, restored_at FROM dmca_actions WHERE id = ?1`).bind(aid).first<{ item_id: string; restored_at: number | null }>();
  if (!a) return null;
  // (Round 22, critic29b: an action put back or withdrawn holds nothing more.)
  if (a.restored_at !== null) throw new PoolError(409, "action_closed", { message: "This takedown was put back or withdrawn; its copies are not held." });
  // Round 14 (B4): by default the action's own blocked keys, so content copies (any lineage, any
  // visibility) are found, not only lineage copies.
  let designated: string[] | null = Array.isArray(b?.questions)
    ? ((b!.questions as unknown[]).filter((x) => typeof x === "string" && BLOCK_KEY_RE.test(x)) as string[])
    : (await c.db.prepare(`SELECT hash FROM blocked_hashes WHERE action_id = ?1 AND kind = 'match'`).bind(aid).all<{ hash: string }>()).results.map((x) => x.hash);
  if (designated.length === 0 && !Array.isArray(b?.questions)) designated = null;
  const dropped: string[] = [];
  if (designated && designated.length > 0 && b?.include_common !== true) {
    const k = await limitOf(c, "*", "block_common_k");
    const common = await c.db
      .prepare(
        `SELECT iq.match_hash FROM item_questions iq JOIN items i ON i.current_publish_id = iq.publish_id
          WHERE iq.match_hash IN (SELECT value FROM json_each(?2)) AND i.id <> ?1 AND i.removed_at IS NULL AND i.visibility = 'public' AND i.purged_at IS NULL
            AND NOT EXISTS (SELECT 1 FROM imports im WHERE im.user_id = i.author_id AND im.item_id = ?1)
            AND NOT EXISTS (SELECT 1 FROM item_questions o WHERE o.publish_id = i.current_publish_id AND o.origin_item_id = ?1)
          GROUP BY iq.match_hash HAVING COUNT(DISTINCT i.author_id) >= ?3`,
      )
      .bind(a.item_id, JSON.stringify(designated), k)
      .all<{ match_hash: string }>();
    const cs = new Set(common.results.map((x) => x.match_hash));
    dropped.push(...designated.filter((x) => cs.has(x)));
    designated = designated.filter((x) => !cs.has(x));
  }
  const blobs = (await c.db.prepare(`SELECT hash FROM blocked_hashes WHERE action_id = ?1 AND kind = 'blob'`).bind(aid).all<{ hash: string }>()).results.map((x) => x.hash);
  // Round 21 (critic27b D2): a page at a time, leaving out what this action already holds: holding is
  // repeated (preview, confirm) until `more` is false; it never fails for size.
  const page = await lineageCopies(c, a.item_id, designated, b?.include_unlinked === true, blobs, { heldBy: `dmca:${aid}` });
  const copies = page.slice(0, LINEAGE_COPIES_MAX);
  const ids = copies.map((x) => x.item_id).sort();
  return {
    itemId: a.item_id,
    copies,
    more: page.length > LINEAGE_COPIES_MAX,
    dropped,
    digest: (admin: string) => JSON.stringify(["hold_copies", aid, admin, ids, designated, b?.include_unlinked === true, b?.include_common === true]),
  };
}
