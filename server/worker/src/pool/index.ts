// The shared pool's routes (docs/shared-pool.md, phase 2). The PUBLIC pool needs `teacher` (or `admin`)
// trust, read from D1 on every request by the auth package's authenticate, plus a redeemed invite code;
// `pending` and `unverified` accounts are refused there. A confirmed `pending` account that is genuinely
// awaiting its first decision may use circles (round 54: see pendingCirclePath), and may upload the images a
// circle send carries; a rejected or revoked one may not. Items the caller may not see are 404, never 403.
import { flushWasmTraps } from "./cron.ts";
import { poolAdminRoutes } from "./admin.ts";
import { ADMIN_BODY_MAX } from "./preread.ts";
import { putBlob } from "./blobs.ts";
import { daily, hourly } from "./cron.ts";
import { appeal, counterNotice, deleteItem, patchItem, report, undeleteItem } from "./moderation.ts";
import { publish } from "./publish.ts";
import { exportAccount } from "./export.ts";
import { INVITE_FREE_PATHS, redeemInvite, requireInvite } from "./invites.ts";
import { getBlob, getItem, importItem, list, mine, versions } from "./read.ts";
import { activity, circleErasureStatements, closeCircle, createCircle, createInvite, deleteBank, getBank, getBankBlob, getCircle, heads, joinCircle, joinPreview, leaveCircle, listCircles, listInvites, patchBank, patchCircle, purgeBankNow, removeMember, reopenCircle, reportBank, revokeInvite, setRole, undeleteBank, offerTransfer, acceptTransfer, cancelTransfer } from "./circles.ts";
import { rollbackBank, sendVersion } from "./circlesend.ts";
import { emailInvite } from "./circlemail.ts";
import { getVersion, history, mergeInitiate, trimVersions } from "./circleversions.ts";
import { ALIVE_SQL, assertPoolKey, unauthorized, errKind, json, makeCtx, PoolError, rateLimit, readJson, type Ctx, type PoolDeps, type PoolPrincipal } from "./shared.ts";

/** GET/PUT /items/prefs {keep_on_delete}: what happens to my shared items if I delete my account. */
async function getPrefs(c: Ctx, p: PoolPrincipal): Promise<Response> {
  const r = await c.db.prepare(`SELECT keep_on_delete FROM pool_prefs WHERE user_id = ?1`).bind(p.userId).first<{ keep_on_delete: number }>();
  // null: no choice made yet (the app asks at account deletion); the default then keeps, anonymised,
  // the items other teachers imported and deletes the rest.
  return json(200, { keep_on_delete: r ? r.keep_on_delete === 1 : null, default: "keep_imported_delete_rest" });
}
async function putPrefs(c: Ctx, req: Request, p: PoolPrincipal): Promise<Response> {
  const b = await readJson(req, `u:${p.userId}`, { deadlineMs: c.deps.bodyDeadlineMs });
  if (typeof b.keep_on_delete !== "boolean") throw new PoolError(400, "invalid_request", { field: "keep_on_delete" });
  // Round 27 (critic35d B-1): written only while the account stands (same statement); else the 401 an
  // unauthenticated request gets.
  const w = await c.db
    .prepare(
      `INSERT INTO pool_prefs (user_id, keep_on_delete, updated_at) SELECT ?1, ?2, ?3 WHERE ${ALIVE_SQL("?1")}
       ON CONFLICT (user_id) DO UPDATE SET keep_on_delete = ?2, updated_at = ?3`,
    )
    .bind(p.userId, b.keep_on_delete ? 1 : 0, c.now())
    .run();
  if ((w.meta.changes ?? 0) === 0) throw unauthorized();
  return json(200, { keep_on_delete: b.keep_on_delete });
}

export const POOL_METHODS = "GET, HEAD, POST, PUT, PATCH, DELETE";
const POOL_TRUST = new Set(["teacher", "admin"]);
/**
 * Round 54 (owner decision): a CONFIRMED account still waiting for approval (`pending`) may use circles: it joins
 * with a circle code and then acts by its role there. The public pool stays approval- and invite-gated. Creation
 * stays behind approval too (POST /circles): a new circle confers ownership and invites, which the role model is
 * designed around for approved members. These are the circle paths open to a pending account; the handlers'
 * membership and role checks remain the real gate.
 */
const PENDING_CIRCLE_PATHS: ReadonlySet<string> = new Set(["/circles/join/preview", "/circles/join", "/circles/heads"]);
const CIRCLE_ID_PATH = /^\/circles\/[0-9a-f]{32}(\/|$)/;
/** A circle path a pending account may call: the list (GET /circles), join and its preview, heads, and every /circles/:cid route. */
function pendingCirclePath(method: string, path: string): boolean {
  if (path === "/circles") return method === "GET";
  return PENDING_CIRCLE_PATHS.has(path) || CIRCLE_ID_PATH.test(path);
}
/** The image upload a circle send makes (`PUT /blobs/<sha256>`): content-addressed, and its quota/caps still apply. */
const BLOB_PUT_PATH = /^\/blobs\/[0-9a-f]{64}$/;
function pendingBlobPath(method: string, path: string): boolean {
  return method === "PUT" && BLOB_PUT_PATH.test(path);
}
/**
 * True when the account's latest approval request is genuinely undecided. `trust === "pending"` alone is not
 * enough: the auth package puts a revoked teacher back on `pending`, and a rejected applicant stays there
 * (admin.ts `statusOf`). A re-application after the cooldown is a new row with no decision, so it counts as
 * waiting again. One indexed D1 read, and only for a pending account on these paths.
 */
async function awaitingFirstDecision(c: Ctx, userId: string): Promise<boolean> {
  const decided = await c.db
    .prepare(
      `SELECT 1 AS x FROM approval_requests ar
        WHERE ar.rowid = (SELECT ar2.rowid FROM approval_requests ar2 WHERE ar2.user_id = ?1 ORDER BY ar2.requested_at DESC, ar2.rowid DESC LIMIT 1)
          AND ar.decided_at IS NOT NULL AND ar.decision IN ('rejected', 'revoked')`,
    )
    .bind(userId)
    .first();
  return decided === null;
}

type Route = (c: Ctx, req: Request, p: PoolPrincipal, m: string[], url: URL) => Promise<Response>;

const ROUTES: { re: RegExp; methods: Record<string, Route> }[] = [
  { re: /^\/items$/, methods: { GET: (c, _r, p, _m, u) => list(c, u, p), POST: (c, r, p) => publish(c, r, p, null) } },
  // Round 17: a dry-run publish (new item, or a republish of :id): the same checks, nothing stored.
  { re: /^\/items\/preview$/, methods: { POST: (c, r, p) => publish(c, r, p, null, true) } },
  { re: /^\/items\/([^/]{1,80})\/preview$/, methods: { POST: (c, r, p, m) => publish(c, r, p, m[1]!, true) } },
  { re: /^\/items\/mine$/, methods: { GET: (c, r, p) => mine(c, p, new URL(r.url).searchParams.get("after")) } },
  { re: /^\/items\/prefs$/, methods: { GET: (c, _r, p) => getPrefs(c, p), PUT: (c, r, p) => putPrefs(c, r, p) } },
  { re: /^\/items\/versions$/, methods: { POST: (c, r, p) => versions(c, r, p) } },
  {
    re: /^\/items\/([^/]{1,80})$/,
    methods: {
      GET: (c, r, p, m) => getItem(c, r, p, m[1]!),
      // Headers only (ETag, Content-Length): nothing is served, nothing is recorded.
      HEAD: (c, r, p, m) => getItem(c, r, p, m[1]!, true),
      PUT: (c, r, p, m) => publish(c, r, p, m[1]!),
      PATCH: (c, r, p, m) => patchItem(c, r, p, m[1]!),
      DELETE: (c, _r, p, m) => deleteItem(c, p, m[1]!),
    },
  },
  { re: /^\/items\/([^/]{1,80})\/undelete$/, methods: { POST: (c, _r, p, m) => undeleteItem(c, p, m[1]!) } },
  { re: /^\/items\/([^/]{1,80})\/import$/, methods: { POST: (c, r, p, m) => importItem(c, r, p, m[1]!) } },
  { re: /^\/items\/([^/]{1,80})\/report$/, methods: { POST: (c, r, p, m) => report(c, r, p, m[1]!) } },
  { re: /^\/items\/([^/]{1,80})\/appeal$/, methods: { POST: (c, r, p, m) => appeal(c, r, p, m[1]!) } },
  { re: /^\/items\/([^/]{1,80})\/counter-notice$/, methods: { POST: (c, r, p, m) => counterNotice(c, r, p, m[1]!) } },
  { re: /^\/items\/([^/]{1,80})\/blobs\/([^/]{1,80})$/, methods: { GET: (c, _r, p, m) => getBlob(c, p, m[1]!, m[2]!) } },
  { re: /^\/blobs\/([^/]{1,80})$/, methods: { PUT: (c, r, p, m) => putBlob(c, r, p, m[1]!) } },
  // Round 23 (owner): the account's own export (pending accounts get their profile only).
  { re: /^\/me\/export$/, methods: { GET: (c, r, p) => exportAccount(c, r, p) } },
  // Round 28 (owner): redeem an admin-issued invite code (pending accounts too; invites.ts).
  { re: /^\/me\/invite$/, methods: { POST: (c, r, p) => redeemInvite(c, r, p) } },
  // Round 32: circles, phase A (circles.ts, circlesend.ts). Circle and bank ids are 32 hex characters
  // in the patterns, so /circles/join and /circles/heads can never be read as a circle id.
  { re: /^\/circles$/, methods: { GET: (c, _r, p) => listCircles(c, p), POST: (c, r, p) => createCircle(c, r, p) } },
  { re: /^\/circles\/join\/preview$/, methods: { POST: (c, r, p) => joinPreview(c, r, p) } },
  { re: /^\/circles\/join$/, methods: { POST: (c, r, p) => joinCircle(c, r, p) } },
  { re: /^\/circles\/heads$/, methods: { POST: (c, r, p) => heads(c, r, p) } },
  {
    re: /^\/circles\/([0-9a-f]{32})$/,
    methods: { GET: (c, _r, p, m) => getCircle(c, p, m[1]!), PATCH: (c, r, p, m) => patchCircle(c, r, p, m[1]!), DELETE: (c, _r, p, m) => closeCircle(c, p, m[1]!) },
  },
  { re: /^\/circles\/([0-9a-f]{32})\/reopen$/, methods: { POST: (c, _r, p, m) => reopenCircle(c, p, m[1]!) } },
  { re: /^\/circles\/([0-9a-f]{32})\/activity$/, methods: { GET: (c, _r, p, m, u) => activity(c, p, m[1]!, u) } },
  { re: /^\/circles\/([0-9a-f]{32})\/invites$/, methods: { GET: (c, _r, p, m, u) => listInvites(c, p, m[1]!, u), POST: (c, r, p, m) => createInvite(c, r, p, m[1]!) } },
  { re: /^\/circles\/([0-9a-f]{32})\/invites\/revoke$/, methods: { POST: (c, r, p, m) => revokeInvite(c, r, p, m[1]!) } },
  // Round 42 (phase C, D5): an invite mailed by the server (circlemail.ts).
  { re: /^\/circles\/([0-9a-f]{32})\/invites\/email$/, methods: { POST: (c, r, p, m) => emailInvite(c, r, p, m[1]!) } },
  { re: /^\/circles\/([0-9a-f]{32})\/leave$/, methods: { POST: (c, _r, p, m) => leaveCircle(c, p, m[1]!) } },
  { re: /^\/circles\/([0-9a-f]{32})\/members\/remove$/, methods: { POST: (c, r, p, m) => removeMember(c, r, p, m[1]!) } },
  { re: /^\/circles\/([0-9a-f]{32})\/members\/role$/, methods: { POST: (c, r, p, m) => setRole(c, r, p, m[1]!) } },
  // (Round 34: a transfer is an offer the member accepts within 7 days.)
  { re: /^\/circles\/([0-9a-f]{32})\/transfer$/, methods: { POST: (c, r, p, m) => offerTransfer(c, r, p, m[1]!) } },
  { re: /^\/circles\/([0-9a-f]{32})\/transfer\/accept$/, methods: { POST: (c, _r, p, m) => acceptTransfer(c, p, m[1]!) } },
  { re: /^\/circles\/([0-9a-f]{32})\/transfer\/cancel$/, methods: { POST: (c, _r, p, m) => cancelTransfer(c, p, m[1]!) } },
  { re: /^\/circles\/([0-9a-f]{32})\/banks$/, methods: { POST: (c, r, p, m) => sendVersion(c, r, p, m[1]!, null) } },
  {
    re: /^\/circles\/([0-9a-f]{32})\/banks\/([0-9a-f]{32})$/,
    methods: {
      GET: (c, r, p, m) => getBank(c, r, p, m[1]!, m[2]!),
      HEAD: (c, r, p, m) => getBank(c, r, p, m[1]!, m[2]!, true),
      PUT: (c, r, p, m) => sendVersion(c, r, p, m[1]!, m[2]!),
      PATCH: (c, r, p, m) => patchBank(c, r, p, m[1]!, m[2]!),
      DELETE: (c, _r, p, m) => deleteBank(c, p, m[1]!, m[2]!),
    },
  },
  { re: /^\/circles\/([0-9a-f]{32})\/banks\/([0-9a-f]{32})\/undelete$/, methods: { POST: (c, _r, p, m) => undeleteBank(c, p, m[1]!, m[2]!) } },
  { re: /^\/circles\/([0-9a-f]{32})\/banks\/([0-9a-f]{32})\/report$/, methods: { POST: (c, r, p, m) => reportBank(c, r, p, m[1]!, m[2]!) } },
  { re: /^\/circles\/([0-9a-f]{32})\/banks\/([0-9a-f]{32})\/purge$/, methods: { POST: (c, _r, p, m) => purgeBankNow(c, p, m[1]!, m[2]!) } },
  { re: /^\/circles\/([0-9a-f]{32})\/banks\/([0-9a-f]{32})\/blobs\/([^/]{1,80})$/, methods: { GET: (c, _r, p, m) => getBankBlob(c, p, m[1]!, m[2]!, m[3]!) } },
  // Round 37: circles phase B (circleversions.ts, circlesend.ts): history, kept versions, merge-initiate, rollback.
  { re: /^\/circles\/([0-9a-f]{32})\/banks\/([0-9a-f]{32})\/versions$/, methods: { GET: (c, _r, p, m, u) => history(c, p, m[1]!, m[2]!, u) } },
  { re: /^\/circles\/([0-9a-f]{32})\/banks\/([0-9a-f]{32})\/versions\/(0|[1-9][0-9]{0,9})$/, methods: { GET: (c, r, p, m) => getVersion(c, r, p, m[1]!, m[2]!, m[3]!) } },
  { re: /^\/circles\/([0-9a-f]{32})\/banks\/([0-9a-f]{32})\/merge$/, methods: { POST: (c, r, p, m) => mergeInitiate(c, r, p, m[1]!, m[2]!) } },
  { re: /^\/circles\/([0-9a-f]{32})\/banks\/([0-9a-f]{32})\/rollback$/, methods: { POST: (c, r, p, m) => rollbackBank(c, r, p, m[1]!, m[2]!) } },
  // Round 38: the owner (or the bank's author) trims a bank's older versions (a full circle's way back).
  { re: /^\/circles\/([0-9a-f]{32})\/banks\/([0-9a-f]{32})\/trim$/, methods: { POST: (c, r, p, m) => trimVersions(c, r, p, m[1]!, m[2]!) } },
];

export interface Pool {
  /** Handles a pool route; null for any other path. */
  handle(req: Request): Promise<Response | null>;
  /** Admin routes for the auth package's approval config (`routes`). */
  adminRoutes: ReturnType<typeof poolAdminRoutes>;
  hourly(): Promise<unknown>;
  daily(): Promise<unknown>;
}

async function keyOk(c: Ctx): Promise<boolean> {
  try {
    await assertPoolKey(c);
    return true;
  } catch (e) {
    console.error(`pool cron: ${e instanceof PoolError ? e.code : "key check failed"}`);
    return false;
  }
}
export function createPool(d: PoolDeps): Pool {
  const ctx = d.r2 ? makeCtx(d, d.r2) : null;
  const allowed = new Set(d.allowedOrigins);
  async function handle(req: Request): Promise<Response | null> {
    const url = new URL(req.url);
    const route = ROUTES.find((r) => r.re.test(url.pathname));
    if (!route) return null;
    try {
      const h = route.methods[req.method];
      if (!h) return json(405, { error: "method_not_allowed" }, { Allow: Object.keys(route.methods).join(", ") });
      const origin = req.headers.get("Origin");
      if (origin !== null && !allowed.has(origin)) throw new PoolError(403, "origin_not_allowed");
      if (req.headers.get("X-Client") !== null && origin !== null) throw new PoolError(400, "native_client_with_origin");
      if (!ctx) throw new PoolError(503, "pool_not_configured");
      const p = await d.authenticate(req);
      if (!p) throw new PoolError(401, "unauthorized", {}, { "WWW-Authenticate": 'Bearer error="invalid_token"' });
      // Round 16 (critic21d 8): every authenticated request is counted first, approved or not, so a
      // pending account cannot send unlimited requests (each one a D1 read of its trust).
      await rateLimit(ctx, req, p.userId, req.method === "GET" || req.method === "HEAD" || /\/(import|versions)$/.test(url.pathname) || url.pathname === "/circles/heads");
      // pending and unverified accounts (and anything else) cannot use the public pool. A CONFIRMED pending
      // account may export its profile, redeem a pool invite code (round 28), and use circles (round 54: join
      // with a circle code, then act by its role there) — but only while its latest approval decision is still
      // open: a rejected applicant and a revoked teacher also have trust "pending" (admin.ts statusOf).
      const uploadPath = pendingBlobPath(req.method, url.pathname);
      const pendingAllowed =
        p.trust === "pending" &&
        (pendingCirclePath(req.method, url.pathname) || uploadPath) &&
        (await awaitingFirstDecision(ctx, p.userId));
      if (!POOL_TRUST.has(p.trust) && !pendingAllowed && !(INVITE_FREE_PATHS.has(url.pathname) && p.trust === "pending"))
        throw new PoolError(403, "account_not_approved", {
          message: p.trust === "pending" ? "Your account is waiting for approval. The shared pool opens once an admin approves it; you can already join a circle with a circle code." : "This account cannot use the shared pool.",
        });
      // Round 28 (owner): the PUBLIC pool also needs a redeemed invite code (admins are exempt; the export and
      // the redemption itself are open). Round 54 (owner): circle paths and the image uploads of a circle send
      // are exempt — a circle code, not a pool invite, admits a member to a circle; the handlers' membership
      // and role checks remain the real gate. invites.ts says why the gate sits here.
      if (p.trust !== "admin" && !pendingCirclePath(req.method, url.pathname) && !uploadPath && !INVITE_FREE_PATHS.has(url.pathname)) await requireInvite(ctx, p.userId);
      // Round 23 (critic30a B1): no write under a key other than the pool's own (reads continue).
      // Round 24: POST /items/versions is a bulk READ (it returns item metadata, never a hash): it
      // continues under a mismatch like any GET, as "plain reads continue" promises.
      // (Round 32: POST /circles/heads is a bulk read too.)
      if (req.method !== "GET" && req.method !== "HEAD" && url.pathname !== "/items/versions" && url.pathname !== "/circles/heads") await assertPoolKey(ctx);
      return await h(ctx, req, p, route.re.exec(url.pathname)!, url);
    } catch (e) {
      if (e instanceof PoolError) return json(e.status, { error: e.code, ...e.extra }, e.headers);
      // (Round 21: a share-core trap is a plain 500 too; its instance was re-created already.)
      console.error(`pool: unhandled error on ${req.method} ${url.pathname}: ${errKind(e)}`);
      return json(500, { error: "server_error" });
    } finally {
      if (ctx) await flushWasmTraps(ctx).catch(() => {});
    }
  }
  return {
    handle,
    // Round 10 M-8: admin bodies (hash lists, long CJK notices) up to ADMIN_BODY_MAX, read after the guard.
    adminRoutes: poolAdminRoutes(() => ctx).map((r) => ({ ...r, maxBodyBytes: ADMIN_BODY_MAX })),
    // (Round 23 B1: the cron, too, does nothing under another key.)
    hourly: async () => (ctx && (await keyOk(ctx)) ? hourly(ctx) : null),
    daily: async () => (ctx && (await keyOk(ctx)) ? daily(ctx) : null),
  };
}

/**
 * Account deletion (the auth package's onDelete, run once the account is certainly gone): the
 * author's items are removed at once and purged after 14 days (an item under a DMCA action follows
 * the same date), filed reports are kept anonymised, pending uploads, counters, limits and the
 * importer's per-question records go, and audit notes about the account are nulled.
 */
export function deleteContentStatements(db: D1Database, userId: string, now: number): D1PreparedStatement[] {
  const anon = `deleted:${crypto.randomUUID().replaceAll("-", "")}`;
  const purgeAt = now + 14 * 86_400_000;
  // Keep (anonymised) when the author chose so; with no choice made, keep exactly the items other
  // teachers imported (round 8), delete the rest. An explicit "no" deletes everything.
  const keep = `(EXISTS (SELECT 1 FROM pool_prefs WHERE user_id = ?1 AND keep_on_delete = 1)
    OR (NOT EXISTS (SELECT 1 FROM pool_prefs WHERE user_id = ?1)
        AND EXISTS (SELECT 1 FROM imports im WHERE im.item_id = items.id AND im.user_id <> ?1 AND im.kind = 'import')))`;
  return [
    // The author chose to keep their shared items: they stay, shown as by "deleted teacher", with
    // licences and attribution intact (the chains already hold the display name).
    db.prepare(`UPDATE items SET anonymized = 1, updated_at = ?2 WHERE author_id = ?1 AND removed_at IS NULL AND ${keep}`).bind(userId, now),
    db
      .prepare(`UPDATE items SET removed_at = ?2, removed_reason = 'account_deleted', purge_after = ?3, updated_at = ?2 WHERE author_id = ?1 AND removed_at IS NULL AND NOT ${keep}`)
      .bind(userId, now, purgeAt),
    db
      .prepare(`UPDATE items SET purge_after = MIN(COALESCE(purge_after, ?2), ?2) WHERE author_id = ?1 AND removed_at IS NOT NULL AND purged_at IS NULL`)
      .bind(userId, purgeAt),
    // Free text the account wrote goes with it (round 8): report notes, and the statements and notes
    // in admin flags (independence statements, appeal notes). Audit notes are nulled below.
    db.prepare(`UPDATE reports SET note = NULL WHERE reporter_id = ?1 AND note IS NOT NULL`).bind(userId),
    db.prepare(`UPDATE admin_flags SET detail = json_remove(detail, '$.statement', '$.note') WHERE user_id = ?1 AND detail IS NOT NULL AND json_valid(detail)`).bind(userId),
    // Round 21 (critic27b E1): the served text a key_leak flag of this reporter quoted goes (by report id).
    db
      .prepare(`UPDATE admin_flags SET detail = json_remove(detail, '$.served') WHERE kind = 'key_leak' AND json_valid(detail) AND detail ->> '$.report_id' IN (SELECT id FROM reports WHERE reporter_id = ?1)`)
      .bind(userId),
    db.prepare(`UPDATE reports SET reporter_id = NULL WHERE reporter_id = ?1`).bind(userId),
    db.prepare(`DELETE FROM pending_blobs WHERE uploader_id = ?1`).bind(userId),
    // (Round 19: the account's served rows go: it can publish nothing a record could bind.)
    db.prepare(`DELETE FROM served WHERE reader_id = ?1`).bind(userId),
    db.prepare(`DELETE FROM usage_daily WHERE user_id = ?1`).bind(userId),
    db.prepare(`DELETE FROM pool_limits WHERE user_id = ?1`).bind(userId),
    // (Round 27: the account's publish lock row goes too; the cron's erasure_resweep repeats all of this
    // for anything a request that was already running wrote after this batch.)
    db.prepare(`DELETE FROM pool_locks WHERE user_id = ?1`).bind(userId),
    // (Round 28: its invite redemption goes; the invite keeps its use count.)
    db.prepare(`DELETE FROM pool_invite_redemptions WHERE user_id = ?1`).bind(userId),
    // Round 32 (circles, plan 8.1): step down, hand on or close, banks kept anonymised or removed (its
    // keep_on_delete choice, read before pool_prefs goes below), messages and notes, codes, memberships.
    ...circleErasureStatements(db, userId, now),
    // (Round 23, critic30b T1: what the account typed, and system notes about it; an admin's note is
    // evidence and stays, its personal data scrubbed when written.)
    db.prepare(`UPDATE pool_audit SET note = NULL WHERE target_user_id = ?1 AND note IS NOT NULL AND actor_kind <> 'admin'`).bind(userId),
    // Round 9: the counter-notices the account sent keep their ids and dates, not the person's
    // name, address, phone or signature; their stated names in others' stored chains and evidence
    // rows become "deleted teacher" (reads resolve names anyway); their preferences go.
    db
      .prepare(
        `UPDATE dmca_counter_notices SET name = '[deleted]', address = '[deleted]', phone = '[deleted]', signature = '[deleted]', material_desc = '[deleted]'
          WHERE action_id IN (SELECT id FROM dmca_actions WHERE author_id = ?1)`,
      )
      .bind(userId),
    // Others' stored chains naming the account: rewritten by the cron in bounded batches (round 11;
    // reads already show "deleted teacher" from the account's state, so the lag is invisible).
    db.prepare(`INSERT OR IGNORE INTO deletion_rewrites (user_id, tbl, at) VALUES (?1, 'item_questions', ?2), (?1, 'ghost_questions', ?2)`).bind(userId, now),
    db.prepare(`DELETE FROM pool_prefs WHERE user_id = ?1`).bind(userId),
    // Round 10 S-1: the account's serves stay counted (import counts) but no longer name it; its
    // report audits no longer link back to it.
    // One anonymous key per deleted account (round 11), so its serves are still counted once per item.
    db.prepare(`UPDATE imports SET user_id = ?2 WHERE user_id = ?1`).bind(userId, anon),
    db.prepare(`UPDATE pool_audit SET actor_id = NULL, detail = json_remove(detail, '$.report_id') WHERE actor_id = ?1 AND action = 'item.report'`).bind(userId),
    // Round 20 (critic25b N3): the account's durable authorship records go with it (they only ever
    // served its own publishes, and nothing can be published by it again).
    db.prepare(`DELETE FROM author_contents WHERE author_id = ?1`).bind(userId),
    db.prepare(`DELETE FROM author_firsts WHERE author_id = ?1`).bind(userId),
    db.prepare(`DELETE FROM author_stems WHERE author_id = ?1`).bind(userId),
    db.prepare(`DELETE FROM stem_bands WHERE user_id IN (?1, 'own:' || ?1)`).bind(userId),
    db.prepare(`DELETE FROM carried_keys WHERE author_id = ?1`).bind(userId),
    db.prepare(`DELETE FROM blob_uploads WHERE uploader_id = ?1`).bind(userId),
    // Round 22 (critic29b): the account's open appeals close (nobody is left to answer), so its removed
    // items are purged at the normal time, not kept to the appeal's end.
    db
      .prepare(
        `UPDATE admin_flags SET resolved_at = ?2, detail = json_set(COALESCE(CASE WHEN json_valid(detail) THEN detail END, '{}'), '$.outcome', 'author_erased')
          WHERE kind = 'appeal' AND resolved_at IS NULL AND (user_id = ?1 OR item_id IN (SELECT id FROM items WHERE author_id = ?1))`,
      )
      .bind(userId, now),
    // ... and every note the account itself wrote.
    db.prepare(`UPDATE pool_audit SET note = NULL WHERE note IS NOT NULL AND actor_id = ?1`).bind(userId),
    db
      .prepare(`INSERT INTO pool_audit (id, at, actor_id, actor_kind, action, target_user_id) VALUES (?1, ?2, ?3, 'user', 'account_deleted.content_removed', ?3)`)
      .bind(crypto.randomUUID(), now, userId),
    // (Round 22: done, in the same batch; a deleted account without this row is finished by the cron.)
    db.prepare(`INSERT OR IGNORE INTO erasure_done (user_id, at) VALUES (?1, ?2)`).bind(userId, now),
  ];
}

/**
 * Suspension and restore change nothing in the pool: every non-author read joins the author's
 * standing, so the items hide and show at once. Only the audit row.
 */
export function standingStatements(db: D1Database, userId: string, now: number, action: "author_suspended" | "author_restored"): D1PreparedStatement[] {
  return [db.prepare(`INSERT INTO pool_audit (id, at, actor_kind, action, target_user_id) VALUES (?1, ?2, 'system', ?3, ?4)`).bind(crypto.randomUUID(), now, action, userId)];
}

/** Round 22 (critic29b): a trap's count leaves with the request that trapped (login isolates too), via waitUntil. */
export function flushTrapsLater(db: D1Database, waitUntil: (p: Promise<unknown>) => void): void {
  waitUntil(flushWasmTraps({ db, now: () => Date.now() }).catch(() => {}));
}
export type { PoolDeps, PoolPrincipal } from "./shared.ts";
export { deriveHashKey, makeCore, type ShareCoreExports } from "./core.ts";
