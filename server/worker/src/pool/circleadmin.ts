// Round 32: the admin's circle routes (docs/circles-plan.md 6.4, D1). Behind the auth package's admin
// guard like every pool admin route (exact paths, ids in the body or query, a sudo token for every write,
// each write audited in pool_audit and auth_audit). No admin route makes an admin a member.
//
// D1: an admin opens circle CONTENT only on a basis: a member reported the bank (`basis=report`), a
// recorded copyright notice names it (`basis=notice&notice_id=`; phases A and B handle DMCA by hand), or a
// student-data block matches one of its questions or images (`basis=privacy`). Every open is audited,
// metadata views included. Writes need a reason of 10 or more characters.
import type { AdminRoute } from "../../../auth/src/index.js";
import { cleanText } from "../../../auth/src/validate.ts";
import { CIRCLE_UNDO_MS, keptInfo } from "./circles.ts";
import { LISTING_REFRESH_SQL } from "./circlesend.ts";
import { readHold, streamedPayload } from "./read.ts";
import { audit, ID_RE, json, PoolError, READ_HEAD_COST, type Ctx } from "./shared.ts";

export const CIRCLE_ADMIN_PAGE = 200;

function idOf(v: unknown, field: string): string {
  if (typeof v !== "string" || !ID_RE.test(v)) throw new PoolError(400, "invalid_request", { field });
  return v;
}
function reasonOf(v: unknown): string {
  const s = cleanText(v, 1000);
  if (!s || s.length < 10) throw new PoolError(400, "invalid_request", { field: "note", message: "Say why (at least 10 characters)." });
  return s;
}

/** SQL: a student-data block matches a question (key-free hash) or an image of bank `?1`; or (round 37) a
 *  send's own check matched it (by any block key, the near-rewording included): the bank holds a privacy
 *  hold, or a privacy_text_match flag names it. */
const PRIVACY_MATCH_SQL = `(EXISTS (SELECT 1 FROM circle_qkeys k JOIN privacy_blocks pb ON pb.kind = 'match' AND pb.hash = k.match_hash WHERE k.bank_id = ?1)
  OR EXISTS (SELECT 1 FROM circle_versions v JOIN circle_version_blobs vb ON vb.version_id = v.id JOIN privacy_blocks pb ON pb.kind = 'blob' AND pb.hash = vb.sha256 WHERE v.bank_id = ?1)
  OR EXISTS (SELECT 1 FROM circle_holds ph WHERE ph.bank_id = ?1 AND ph.source IN ('privacy', 'privacy_near'))
  OR EXISTS (SELECT 1 FROM admin_flags af WHERE af.kind = 'privacy_text_match' AND af.item_id = ?1))`;

export function circleAdminRoutes(C: () => Ctx): AdminRoute[] {
  return [
    {
      method: "GET",
      path: "/admin/pool/circle-reports",
      write: false,
      // Open circle reports, oldest first, paged (`after` = the last one's at:id). The admin's queue for circle cases.
      handler: async (req) => {
        const c = C();
        const cur = /^(\d{1,16}):([0-9a-f]{32})$/.exec(new URL(req.url).searchParams.get("after") ?? "");
        const rows = await c.db
          .prepare(
            `SELECT r.id, r.circle_id, r.bank_id, r.version, r.reason, r.note, r.at, r.reporter_id, ru.display_name AS reporter_name,
                    ci.name AS circle_name, b.title AS bank_title, b.author_id, (SELECT json_group_array(h.reason) FROM circle_holds h WHERE h.bank_id = r.bank_id) AS holds
               FROM circle_reports r JOIN circles ci ON ci.id = r.circle_id JOIN circle_banks b ON b.id = r.bank_id LEFT JOIN users ru ON ru.id = r.reporter_id
              WHERE r.resolved_at IS NULL AND (?1 IS NULL OR r.at > ?1 OR (r.at = ?1 AND r.id > ?2)) ORDER BY r.at, r.id LIMIT ?3`,
          )
          .bind(cur ? Number(cur[1]) : null, cur ? cur[2] : "", CIRCLE_ADMIN_PAGE + 1)
          .all<Record<string, unknown> & { id: string; at: number; holds: string }>();
        const page = rows.results.slice(0, CIRCLE_ADMIN_PAGE).map((r) => ({ ...r, holds: JSON.parse(r.holds ?? "[]") as unknown }));
        const last = page[page.length - 1];
        return json(200, { reports: page, next_cursor: rows.results.length > CIRCLE_ADMIN_PAGE && last ? `${last.at}:${last.id}` : null });
      },
    },
    {
      method: "GET",
      path: "/admin/pool/circles",
      write: false,
      // Circles, newest first, paged (`after` = created_at:id); `user_id=` the circles an account is (or was) in.
      handler: async (req) => {
        const c = C();
        const u = new URL(req.url).searchParams;
        const who = u.get("user_id");
        if (who !== null && !/^[0-9a-f-]{36}$/.test(who)) throw new PoolError(400, "invalid_request", { field: "user_id" });
        const cur = /^(\d{1,16}):([0-9a-f]{32})$/.exec(u.get("after") ?? "");
        const rows = await c.db
          .prepare(
            `SELECT ci.id, ci.name, ci.created_by, ci.created_at, ci.closed_at, ci.close_reason, ci.purge_after, ci.purged_at,
                    (SELECT COUNT(*) FROM circle_members m WHERE m.circle_id = ci.id AND m.left_at IS NULL) AS members,
                    (SELECT COUNT(*) FROM circle_banks b WHERE b.circle_id = ci.id AND b.purged_at IS NULL) AS banks,
                    (SELECT m.user_id FROM circle_members m WHERE m.circle_id = ci.id AND m.role = 'owner' AND m.left_at IS NULL) AS owner_id
               FROM circles ci
              WHERE (?1 IS NULL OR ci.id IN (SELECT circle_id FROM circle_members WHERE user_id = ?1))
                AND (?2 IS NULL OR ci.created_at < ?2 OR (ci.created_at = ?2 AND ci.id < ?3))
              ORDER BY ci.created_at DESC, ci.id DESC LIMIT ?4`,
          )
          .bind(who, cur ? Number(cur[1]) : null, cur ? cur[2] : "", CIRCLE_ADMIN_PAGE + 1)
          .all<Record<string, unknown> & { id: string; created_at: number }>();
        const page = rows.results.slice(0, CIRCLE_ADMIN_PAGE);
        const last = page[page.length - 1];
        return json(200, { circles: page, next_cursor: rows.results.length > CIRCLE_ADMIN_PAGE && last ? `${last.created_at}:${last.id}` : null });
      },
    },
    {
      method: "GET",
      path: "/admin/pool/circle",
      write: false,
      // ?id=: metadata (members past and present, banks with holds and reports, codes' states). Audited.
      handler: async (req, h) => {
        const c = C();
        const cid = idOf(new URL(req.url).searchParams.get("id"), "id");
        const circle = await c.db.prepare(`SELECT * FROM circles WHERE id = ?1`).bind(cid).first<Record<string, unknown>>();
        if (!circle) return json(404, { error: "not_found" });
        const members = await c.db
          .prepare(`SELECT m.user_id, u.display_name, m.role, m.joined_at, m.join_seq, m.left_at, m.left_reason, m.invite_id FROM circle_members m LEFT JOIN users u ON u.id = m.user_id WHERE m.circle_id = ?1 ORDER BY m.join_seq`)
          .bind(cid)
          .all();
        const banks = await c.db
          .prepare(
            `SELECT b.id, b.title, b.author_id, b.license, b.head_version, b.question_count, b.has_images, b.anonymized, b.created_at, b.updated_at, b.removed_at, b.removed_reason, b.purge_after, b.purged_at,
                    (SELECT json_group_array(json_object('reason', h.reason, 'source', h.source, 'at', h.at)) FROM circle_holds h WHERE h.bank_id = b.id) AS holds,
                    (SELECT COUNT(*) FROM circle_reports r WHERE r.bank_id = b.id AND r.resolved_at IS NULL) AS open_reports
               FROM circle_banks b WHERE b.circle_id = ?1 ORDER BY b.created_at`,
          )
          .bind(cid)
          .all<Record<string, unknown> & { holds: string }>();
        const invites = await c.db
          .prepare(`SELECT id, role, created_by, created_at, max_uses, uses, expires_at, revoked_at FROM circle_invites WHERE circle_id = ?1 ORDER BY created_at DESC LIMIT 200`)
          .bind(cid)
          .all();
        await c.db.batch([
          audit(c.db, { actorId: h.admin.userId, actorKind: "admin", action: "circle.admin_open", itemId: cid }, h.now),
          h.audit({ action: "pool.circle_open", targetId: cid }),
        ]);
        return json(200, { circle, members: members.results, banks: banks.results.map((b) => ({ ...b, holds: JSON.parse(b.holds ?? "[]") as unknown })), invites: invites.results });
      },
    },
    {
      method: "GET",
      path: "/admin/pool/circle-bank",
      write: false,
      // ?id=<bank>&version=<n, default the head>&basis=report|notice|privacy[&notice_id=]: a kept version's
      // body, streamed under a read reservation. Audited (D1).
      handler: async (req, h) => {
        const c = C();
        const u = new URL(req.url).searchParams;
        const bid = idOf(u.get("id"), "id");
        const basis = u.get("basis");
        const b = await c.db.prepare(`SELECT id, circle_id, head_version, title, author_id, license FROM circle_banks WHERE id = ?1`).bind(bid).first<{ id: string; circle_id: string; head_version: number; title: string; author_id: string; license: string }>();
        if (!b) return json(404, { error: "not_found" });
        let noticeId: string | null = null;
        const ok =
          basis === "report"
            ? await c.db.prepare(`SELECT 1 AS x FROM circle_reports WHERE bank_id = ?1 LIMIT 1`).bind(bid).first()
            : basis === "notice"
              ? await c.db.prepare(`SELECT 1 AS x FROM dmca_notices WHERE id = ?1`).bind((noticeId = idOf(u.get("notice_id"), "notice_id"))).first()
              : basis === "privacy"
                ? await c.db.prepare(`SELECT 1 AS x WHERE ${PRIVACY_MATCH_SQL}`).bind(bid).first()
                : null;
        if (basis !== "report" && basis !== "notice" && basis !== "privacy") throw new PoolError(400, "invalid_request", { field: "basis", allowed: ["report", "notice", "privacy"] });
        if (!ok) return json(403, { error: "basis_required", message: "Circle content is opened only when a member reported it, a recorded copyright notice names it, or a student-data block matches it." });
        const vArg = u.get("version");
        const want = vArg === null ? b.head_version : /^[1-9][0-9]{0,8}$/.test(vArg) ? Number(vArg) : NaN;
        if (Number.isNaN(want)) throw new PoolError(400, "invalid_request", { field: "version" });
        const v = await c.db
          .prepare(`SELECT id, version, status, full_key, committer_id, committed_at, message, license FROM circle_versions WHERE bank_id = ?1 AND version = ?2`)
          .bind(bid, want)
          .first<{ id: string; version: number; status: string; full_key: string; committer_id: string; committed_at: number; message: string | null; license: string }>();
        if (!v || (v.status !== "current" && v.status !== "previous") || v.full_key === "") {
          return json(404, { error: "version_not_retained", ...(await keptInfo(c, bid, b.head_version)) });
        }
        const detail = { bank_id: bid, version: v.version, basis, ...(noticeId ? { notice_id: noticeId } : {}) };
        await c.db.batch([
          audit(c.db, { actorId: h.admin.userId, actorKind: "admin", action: "circle.admin_view", itemId: b.circle_id, detail }, h.now),
          h.audit({ action: "pool.circle_view", targetId: bid, detail }),
        ]);
        const head = JSON.stringify({ bank: b, version: { version: v.version, status: v.status, committer_id: v.committer_id, committed_at: v.committed_at, message: v.message, license: v.license } });
        const release = readHold(c, req, h.admin.userId);
        try {
          release.grow(READ_HEAD_COST * head.length);
          const body = await c.r2.get(v.full_key);
          if (!body) {
            release();
            return json(200, { ...(JSON.parse(head) as object), payload: null });
          }
          return streamedPayload(head, body as R2ObjectBody, release);
        } catch (e) {
          release();
          throw e;
        }
      },
    },
    {
      method: "POST",
      path: "/admin/pool/circle-hold",
      write: true,
      // {bank_id, note}: hidden from every member but its author until released.
      handler: async (_req, h) => {
        const c = C();
        const bid = idOf(h.body?.bank_id, "bank_id");
        const n = reasonOf(h.body?.note);
        const b = await c.db.prepare(`SELECT circle_id FROM circle_banks WHERE id = ?1 AND purged_at IS NULL`).bind(bid).first<{ circle_id: string }>();
        if (!b) return json(404, { error: "not_found" });
        const [ins] = await c.db.batch([
          c.db.prepare(`INSERT OR IGNORE INTO circle_holds (bank_id, reason, source, at, by) VALUES (?1, 'admin', 'manual', ?2, ?3)`).bind(bid, h.now, h.admin.userId),
          audit(c.db, { actorId: h.admin.userId, actorKind: "admin", action: "circle.admin_hold", itemId: b.circle_id, detail: { bank_id: bid }, note: n }, h.now),
          h.audit({ action: "pool.circle_hold", targetId: bid, note: n }),
        ]);
        return json(200, { ok: true, changed: (ins!.meta.changes ?? 0) === 1 });
      },
    },
    {
      method: "POST",
      path: "/admin/pool/circle-release",
      write: true,
      // {bank_id, note}: every hold of the bank goes and its open reports are dismissed (a version withheld for
      // student data stays withheld). Round 38: {bank_id, version, note} releases THAT withheld version instead
      // (readable to members again, its student-data flag resolved; if it is the head, the bank's privacy hold goes).
      handler: async (_req, h) => {
        const c = C();
        const bid = idOf(h.body?.bank_id, "bank_id");
        const n = reasonOf(h.body?.note);
        const b = await c.db.prepare(`SELECT circle_id FROM circle_banks WHERE id = ?1`).bind(bid).first<{ circle_id: string }>();
        if (!b) return json(404, { error: "not_found" });
        if (h.body?.version !== undefined) {
          const v = h.body.version;
          if (typeof v !== "number" || !Number.isInteger(v) || v < 1) throw new PoolError(400, "invalid_request", { field: "version" });
          const [up] = await c.db.batch([
            c.db.prepare(`UPDATE circle_versions SET withheld_at = NULL WHERE bank_id = ?1 AND version = ?2 AND withheld_at IS NOT NULL`).bind(bid, v),
            c.db
              .prepare(`UPDATE admin_flags SET resolved_at = ?3, resolved_by = ?4 WHERE kind = 'privacy_text_match' AND resolved_at IS NULL AND publish_id = (SELECT id FROM circle_versions WHERE bank_id = ?1 AND version = ?2) AND changes() = 1`)
              .bind(bid, v, h.now, h.admin.userId),
            c.db
              .prepare(
                `DELETE FROM circle_holds WHERE bank_id = ?1 AND source IN ('privacy', 'privacy_near') AND EXISTS (SELECT 1 FROM circle_banks hb WHERE hb.id = ?1 AND hb.head_version = ?2)
                   AND EXISTS (SELECT 1 FROM circle_versions hv WHERE hv.bank_id = ?1 AND hv.version = ?2 AND hv.withheld_at IS NULL)`,
              )
              .bind(bid, v),
            // (round 39: the bank's listing again from its newest version that is not withheld, which may be this one now)
            c.db.prepare(LISTING_REFRESH_SQL(`EXISTS (SELECT 1 FROM circle_versions rv WHERE rv.bank_id = ?1 AND rv.version = ?2 AND rv.withheld_at IS NULL)`)).bind(bid, v),
            audit(c.db, { actorId: h.admin.userId, actorKind: "admin", action: "circle.admin_version_release", itemId: b.circle_id, detail: { bank_id: bid, version: v }, note: n }, h.now),
            h.audit({ action: "pool.circle_version_release", targetId: bid, note: n, detail: { version: v } }),
          ]);
          if ((up!.meta.changes ?? 0) !== 1) return json(409, { error: "not_withheld", message: "That version is not withheld." });
          return json(200, { ok: true, released_version: v });
        }
        const [del, rep] = await c.db.batch([
          c.db.prepare(`DELETE FROM circle_holds WHERE bank_id = ?1`).bind(bid),
          c.db.prepare(`UPDATE circle_reports SET resolved_at = ?2, resolved_by = ?3, action = 'dismissed' WHERE bank_id = ?1 AND resolved_at IS NULL`).bind(bid, h.now, h.admin.userId),
          // (round 37: and the student-data matches a send raised for it: the admin has decided; round 38: except
          // those of versions still withheld, which a version release resolves)
          c.db
            .prepare(
              `UPDATE admin_flags SET resolved_at = ?2, resolved_by = ?3 WHERE kind = 'privacy_text_match' AND item_id = ?1 AND resolved_at IS NULL
                 AND NOT EXISTS (SELECT 1 FROM circle_versions wv WHERE wv.id = admin_flags.publish_id AND wv.withheld_at IS NOT NULL)`,
            )
            .bind(bid, h.now, h.admin.userId),
          audit(c.db, { actorId: h.admin.userId, actorKind: "admin", action: "circle.admin_release", itemId: b.circle_id, detail: { bank_id: bid }, note: n }, h.now),
          h.audit({ action: "pool.circle_release", targetId: bid, note: n }),
        ]);
        return json(200, { ok: true, released: del!.meta.changes ?? 0, dismissed: rep!.meta.changes ?? 0 });
      },
    },
    {
      method: "POST",
      path: "/admin/pool/circle-remove",
      write: true,
      // {bank_id, note}: removed now, purged after 14 days; its open reports are resolved as removed, its holds go.
      handler: async (_req, h) => {
        const c = C();
        const bid = idOf(h.body?.bank_id, "bank_id");
        const n = reasonOf(h.body?.note);
        const b = await c.db.prepare(`SELECT circle_id FROM circle_banks WHERE id = ?1`).bind(bid).first<{ circle_id: string }>();
        if (!b) return json(404, { error: "not_found" });
        const [upd] = await c.db.batch([
          c.db
            .prepare(
              `UPDATE circle_banks SET removed_at = COALESCE(removed_at, ?2), removed_reason = 'admin', purge_after = ?3, updated_at = ?2
                WHERE id = ?1 AND purged_at IS NULL AND (removed_reason IS NULL OR removed_reason IN ('author_removed', 'owner_removed', 'account_deleted', 'circle_closed'))`,
            )
            .bind(bid, h.now, h.now + CIRCLE_UNDO_MS),
          c.db.prepare(`UPDATE circle_reports SET resolved_at = ?2, resolved_by = ?3, action = 'removed' WHERE bank_id = ?1 AND resolved_at IS NULL AND changes() = 1`).bind(bid, h.now, h.admin.userId),
          audit(c.db, { actorId: h.admin.userId, actorKind: "admin", action: "circle.admin_bank_remove", itemId: b.circle_id, detail: { bank_id: bid }, note: n }, h.now),
          // (round 33: the admin decided: its holds go with the decision, so the purge follows in 14 days)
          c.db.prepare(`DELETE FROM circle_holds WHERE bank_id = ?1 AND EXISTS (SELECT 1 FROM circle_banks WHERE id = ?1 AND removed_reason = 'admin')`).bind(bid),
          c.db.prepare(`UPDATE admin_flags SET resolved_at = ?2, resolved_by = ?3 WHERE kind = 'privacy_text_match' AND item_id = ?1 AND resolved_at IS NULL AND EXISTS (SELECT 1 FROM circle_banks WHERE id = ?1 AND removed_reason = 'admin')`).bind(bid, h.now, h.admin.userId),
          h.audit({ action: "pool.circle_remove", targetId: bid, note: n }),
        ]);
        return (upd!.meta.changes ?? 0) === 1 ? json(200, { ok: true }) : json(409, { error: "not_removable" });
      },
    },
    {
      method: "POST",
      path: "/admin/pool/circle-close",
      write: true,
      // {circle_id, note}: closed for everyone now (no owner undo), purged after 14 days; open reports resolved, holds lifted.
      handler: async (_req, h) => {
        const c = C();
        const cid = idOf(h.body?.circle_id, "circle_id");
        const n = reasonOf(h.body?.note);
        const [upd] = await c.db.batch([
          c.db
            .prepare(`UPDATE circles SET closed_at = COALESCE(closed_at, ?2), close_reason = 'admin', undo_until = NULL, purge_after = ?3, updated_at = ?2 WHERE id = ?1 AND purged_at IS NULL AND close_reason IS NOT 'admin'`)
            .bind(cid, h.now, h.now + CIRCLE_UNDO_MS),
          c.db.prepare(`UPDATE circle_reports SET resolved_at = ?2, resolved_by = ?3, action = 'removed' WHERE circle_id = ?1 AND resolved_at IS NULL AND changes() = 1`).bind(cid, h.now, h.admin.userId),
          audit(c.db, { actorId: h.admin.userId, actorKind: "admin", action: "circle.admin_close", itemId: cid, note: n }, h.now),
          // (round 33: and the holds of its banks: the admin's close is the decision on all of it)
          c.db.prepare(`DELETE FROM circle_holds WHERE bank_id IN (SELECT id FROM circle_banks WHERE circle_id = ?1) AND EXISTS (SELECT 1 FROM circles WHERE id = ?1 AND close_reason = 'admin')`).bind(cid),
          // (round 41: and its banks' student-data flags, so the circle's purge follows in 14 days)
          c.db
            .prepare(
              `UPDATE admin_flags SET resolved_at = ?2, resolved_by = ?3 WHERE kind = 'privacy_text_match' AND resolved_at IS NULL AND item_id IN (SELECT id FROM circle_banks WHERE circle_id = ?1)
                 AND EXISTS (SELECT 1 FROM circles WHERE id = ?1 AND close_reason = 'admin')`,
            )
            .bind(cid, h.now, h.admin.userId),
          h.audit({ action: "pool.circle_close", targetId: cid, note: n }),
        ]);
        return (upd!.meta.changes ?? 0) === 1 ? json(200, { ok: true, purge_after: h.now + CIRCLE_UNDO_MS }) : json(409, { error: "not_closable" });
      },
    },
  ];
}
