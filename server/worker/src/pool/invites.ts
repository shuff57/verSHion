// Round 28 (owner): the invite-code gate (docs/shared-pool.md "Approval").
//
// Where it sits, and why there. Pool use needs BOTH the admin's approval (trust `teacher`, decided in the
// auth package's queue, unchanged) AND a redeemed invite code an admin issued. The gate is in the pool,
// not in sign-up:
// * The auth package's sign-up answers the same whether or not the address has an account, and nothing a
//   registrant types survives confirmation (the mailbox owner enters the profile again: a student can
//   sign up with a teacher's address). A code checked or used up at sign-up could be burned by whoever
//   typed someone else's address, and refusing a sign-up for a bad code would add an answer that differs.
// * The auth package is generic (published separately); it stays as it is.
// So an account signs up and confirms as before, then redeems its code (`POST /me/invite`, open to a
// pending account too, so the admin can see it before approving: `GET /admin/pool/invites?user_id=`).
// Until it holds a redemption every other pool route answers `403 invite_required`. Admins are exempt;
// `GET /me/export` stays open (an account's own data). Accounts already teacher or admin when migration
// 0106 ran hold a redemption with no invite (`invite_id` NULL), so existing approved accounts keep
// working.
//
// Codes: 120 random bits as 24 Crockford base32 characters (shown as six groups of four), returned ONCE
// when issued; only the SHA-256 of the normalised form is stored. Single use by default, `max_uses` up
// to 1,000, an optional expiry, revocable (a revocation stops further redemptions; it never undoes one:
// use revoke-teacher or suspend for an account). Redemption attempts are a daily allowance per account
// (`invite_tries`), on top of the per-account request limits.
import type { AdminRoute } from "../../../auth/src/index.js";
import { cleanText } from "../../../auth/src/validate.ts";
import { ALIVE_SQL, audit, DAY_MS, ID_RE, json, newId, PoolError, readJson, requireDaily, sha256Hex, throwIfErased, type Ctx, type PoolPrincipal } from "./shared.ts";

/** Crockford base32 (no I, L, O, U). */
export const INVITE_ALPHABET = "0123456789ABCDEFGHJKMNPQRSTVWXYZ";
/** Random bytes per code: 15 bytes = 120 bits = 24 characters. */
export const INVITE_BYTES = 15;
export const INVITE_CHARS = (INVITE_BYTES * 8) / 5;
export const INVITE_MAX_USES = 1000;
export const INVITE_MAX_DAYS = 365;
/** Invites one page of `GET /admin/pool/invites` lists (newest first, cursor paged). */
export const INVITE_PAGE = 100;

/** Routes an account without a redeemed invite may still call (redeeming one; its own data export). */
export const INVITE_FREE_PATHS: ReadonlySet<string> = new Set(["/me/invite", "/me/export"]);

/** A new code, grouped for reading aloud: XXXX-XXXX-XXXX-XXXX-XXXX-XXXX. */
export function newInviteCode(): string {
  const b = crypto.getRandomValues(new Uint8Array(INVITE_BYTES));
  let bits = 0;
  let acc = 0;
  let out = "";
  for (const x of b) {
    acc = (acc << 8) | x;
    bits += 8;
    while (bits >= 5) {
      bits -= 5;
      out += INVITE_ALPHABET[(acc >> bits) & 31];
    }
    acc &= (1 << bits) - 1;
  }
  return out.match(/.{4}/g)!.join("-");
}

/** The normalised form of a typed code (case, spaces and hyphens ignored; I/L read as 1, O as 0), or null. */
export function normaliseInviteCode(raw: unknown): string | null {
  if (typeof raw !== "string" || raw.length > 100) return null;
  const s = raw.toUpperCase().replace(/[\s-]/g, "").replace(/[IL]/g, "1").replace(/O/g, "0");
  if (s.length !== INVITE_CHARS) return null;
  for (const ch of s) if (!INVITE_ALPHABET.includes(ch)) return null;
  return s;
}

export const inviteHash = (normalised: string): Promise<string> => sha256Hex(new TextEncoder().encode(normalised));

const INVITE_REQUIRED_MESSAGE = "The shared pool is invite-only. Enter the invite code an admin gave you.";

/** 403 `invite_required` unless the account holds a redemption (an erased account: the 401 an unauthenticated request gets). */
export async function requireInvite(c: Ctx, userId: string): Promise<void> {
  const r = await c.db.prepare(`SELECT 1 AS x FROM pool_invite_redemptions WHERE user_id = ?1`).bind(userId).first();
  if (r) return;
  await throwIfErased(c, userId);
  throw new PoolError(403, "invite_required", { message: INVITE_REQUIRED_MESSAGE });
}

/**
 * POST /me/invite {code}: the account redeems an invite code. One redemption per account: an account that
 * already holds one is told so and nothing is used up. Every refusal of a code (unknown, expired, revoked,
 * used up) is the same `422 invite_invalid`.
 */
export async function redeemInvite(c: Ctx, req: Request, p: PoolPrincipal): Promise<Response> {
  const me = p.userId;
  const b = await readJson(req, `u:${me}`, { deadlineMs: c.deps.bodyDeadlineMs });
  const code = normaliseInviteCode(b.code);
  if (!code) throw new PoolError(400, "invalid_request", { field: "code" });
  const held = () => c.db.prepare(`SELECT 1 AS x FROM pool_invite_redemptions WHERE user_id = ?1`).bind(me).first();
  if (await held()) return json(200, { ok: true, already: true });
  await requireDaily(c, me, "invite_tries");
  const now = c.now();
  const invalid = () => new PoolError(422, "invite_invalid", { message: "This invite code is not valid: it may have expired, been used up or been withdrawn. Ask an admin for a new one." });
  const inv = await c.db.prepare(`SELECT id FROM pool_invites WHERE code_hash = ?1`).bind(await inviteHash(code)).first<{ id: string }>();
  if (!inv) throw invalid();
  // One batch: the use is taken only if the code is still good and the account holds no redemption and
  // stands (ALIVE_SQL, round 27: nothing is written for an erased account); the redemption and its audit
  // follow it by changes().
  const [use] = await c.db.batch([
    c.db
      .prepare(
        `UPDATE pool_invites SET uses = uses + 1
          WHERE id = ?1 AND revoked_at IS NULL AND (expires_at IS NULL OR expires_at > ?2) AND uses < max_uses
            AND NOT EXISTS (SELECT 1 FROM pool_invite_redemptions WHERE user_id = ?3) AND ${ALIVE_SQL("?3")}`,
      )
      .bind(inv.id, now, me),
    c.db.prepare(`INSERT INTO pool_invite_redemptions (user_id, invite_id, at) SELECT ?1, ?2, ?3 WHERE changes() = 1`).bind(me, inv.id, now),
    audit(c.db, { actorId: me, actorKind: "user", action: "invite.redeem", targetUserId: me, detail: { invite_id: inv.id } }, now, `changes() = 1`),
  ]);
  if ((use!.meta.changes ?? 0) === 1) return json(200, { ok: true, redeemed: true });
  // Nothing taken: a concurrent redemption of this account won, the account was erased, or the code is spent.
  if (await held()) return json(200, { ok: true, already: true });
  await throwIfErased(c, me);
  throw invalid();
}

function noteOf(v: unknown): string | null {
  if (v === undefined || v === null || v === "") return null;
  const s = cleanText(v, 1000);
  if (!s) throw new PoolError(400, "invalid_request", { field: "note" });
  return s;
}
function intIn(v: unknown, lo: number, hi: number, field: string): number {
  if (typeof v !== "number" || !Number.isInteger(v) || v < lo || v > hi) throw new PoolError(400, "invalid_request", { field, min: lo, max: hi });
  return v;
}
/** The state an invite is in, as the admin sees it. */
const STATE_SQL = `CASE WHEN revoked_at IS NOT NULL THEN 'revoked' WHEN uses >= max_uses THEN 'used_up' WHEN expires_at IS NOT NULL AND expires_at <= ?1 THEN 'expired' ELSE 'active' END`;
const INVITE_COLS = `id, created_at, created_by, max_uses, uses, expires_at, revoked_at, revoked_by, note, ${STATE_SQL} AS state`;

/** The admin routes (behind the auth package's admin guard; every write audited in pool_audit and auth_audit). */
export function inviteAdminRoutes(C: () => Ctx): AdminRoute[] {
  return [
    {
      method: "POST",
      path: "/admin/pool/invites",
      write: true,
      // {max_uses?: 1..1000 (default 1), expires_in_days?: 1..365, note?}: the code is in this answer only.
      handler: async (_req, h) => {
        const c = C();
        const maxUses = h.body?.max_uses === undefined || h.body?.max_uses === null ? 1 : intIn(h.body.max_uses, 1, INVITE_MAX_USES, "max_uses");
        const days = h.body?.expires_in_days === undefined || h.body?.expires_in_days === null ? null : intIn(h.body.expires_in_days, 1, INVITE_MAX_DAYS, "expires_in_days");
        const note = noteOf(h.body?.note);
        const code = newInviteCode();
        const id = newId();
        const expiresAt = days === null ? null : h.now + days * DAY_MS;
        await c.db.batch([
          c.db
            .prepare(`INSERT INTO pool_invites (id, code_hash, created_at, created_by, max_uses, uses, expires_at, note) VALUES (?1, ?2, ?3, ?4, ?5, 0, ?6, ?7)`)
            .bind(id, await inviteHash(normaliseInviteCode(code)!), h.now, h.admin.userId, maxUses, expiresAt, note),
          // (the code itself is never written anywhere: neither audit has it)
          audit(c.db, { actorId: h.admin.userId, actorKind: "admin", action: "invite.issue", detail: { invite_id: id, max_uses: maxUses, expires_at: expiresAt }, note }, h.now),
          h.audit({ action: "pool.invite_issue", targetId: id, detail: { max_uses: maxUses, expires_at: expiresAt }, note }),
        ]);
        return json(201, { invite_id: id, code, max_uses: maxUses, expires_at: expiresAt, note, shown_once: true });
      },
    },
    {
      method: "GET",
      path: "/admin/pool/invites",
      write: false,
      // ?id=<invite id>: that invite with every account that redeemed it. ?user_id=<account>: that
      // account's redemption (null: none; invite_id null: approved before the gate). Otherwise the
      // invites, newest first, paged (`after` = the last one's created_at:id).
      handler: async (req, h) => {
        const c = C();
        const u = new URL(req.url).searchParams;
        const one = u.get("id");
        if (one !== null) {
          if (!ID_RE.test(one)) throw new PoolError(400, "invalid_request", { field: "id" });
          const inv = await c.db.prepare(`SELECT ${INVITE_COLS} FROM pool_invites WHERE id = ?2`).bind(h.now, one).first<Record<string, unknown>>();
          if (!inv) return json(404, { error: "not_found" });
          // (at most max_uses, 1,000, rows: the whole list)
          const red = await c.db.prepare(`SELECT user_id, at FROM pool_invite_redemptions WHERE invite_id = ?1 ORDER BY at, user_id`).bind(one).all<{ user_id: string; at: number }>();
          return json(200, { invite: { ...inv, redeemed_by: red.results } });
        }
        const who = u.get("user_id");
        if (who !== null) {
          if (!/^[0-9a-f-]{36}$/.test(who)) throw new PoolError(400, "invalid_request", { field: "user_id" });
          const r = await c.db.prepare(`SELECT invite_id, at FROM pool_invite_redemptions WHERE user_id = ?1`).bind(who).first<{ invite_id: string | null; at: number }>();
          return json(200, { user_id: who, redemption: r ? { invite_id: r.invite_id, at: r.at, grandfathered: r.invite_id === null } : null });
        }
        const cur = /^(\d{1,16}):([0-9a-f]{32})$/.exec(u.get("after") ?? "");
        const limit = INVITE_PAGE;
        const rows = await c.db
          .prepare(
            `SELECT ${INVITE_COLS} FROM pool_invites WHERE ?2 IS NULL OR created_at < ?2 OR (created_at = ?2 AND id < ?3)
              ORDER BY created_at DESC, id DESC LIMIT ?4`,
          )
          .bind(h.now, cur ? Number(cur[1]) : null, cur ? cur[2] : "", limit + 1)
          .all<Record<string, unknown> & { id: string; created_at: number }>();
        // (one more than shown: the cursor says there is more)
        const page = rows.results.slice(0, limit);
        const last = page[page.length - 1];
        return json(200, { invites: page, next_cursor: rows.results.length > limit && last ? `${last.created_at}:${last.id}` : null });
      },
    },
    {
      method: "POST",
      path: "/admin/pool/invites/revoke",
      write: true,
      // {invite_id, note?}: no further redemptions. Accounts that already redeemed it keep their access.
      handler: async (_req, h) => {
        const c = C();
        const id = h.body?.invite_id;
        if (typeof id !== "string" || !ID_RE.test(id)) throw new PoolError(400, "invalid_request", { field: "invite_id" });
        const note = noteOf(h.body?.note);
        const there = await c.db.prepare(`SELECT revoked_at FROM pool_invites WHERE id = ?1`).bind(id).first<{ revoked_at: number | null }>();
        if (!there) return json(404, { error: "not_found" });
        if (there.revoked_at !== null) return json(409, { error: "already_revoked", revoked_at: there.revoked_at });
        const [upd] = await c.db.batch([
          c.db.prepare(`UPDATE pool_invites SET revoked_at = ?2, revoked_by = ?3 WHERE id = ?1 AND revoked_at IS NULL`).bind(id, h.now, h.admin.userId),
          audit(c.db, { actorId: h.admin.userId, actorKind: "admin", action: "invite.revoke", detail: { invite_id: id }, note }, h.now, `changes() = 1`),
          h.audit({ action: "pool.invite_revoke", targetId: id, note }),
        ]);
        // (a concurrent revoke won: same answer as above)
        if ((upd!.meta.changes ?? 0) !== 1) return json(409, { error: "already_revoked" });
        return json(200, { ok: true, invite_id: id });
      },
    },
  ];
}
