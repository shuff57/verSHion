// Round 32: circles, phase A (docs/circles-plan.md; decisions D1-D19). A circle is a small private group
// of approved teachers: any approved teacher (past the pool's invite-code gate) creates one and invites
// others by code; members see the circle's banks and take copies. Phase A: only a bank's author sends
// new versions of it (shared editing, history, merge and rollback are phase B).
//
// Rules every route here keeps (plan section 6.1):
// * The routes sit in the pool's route table (index.ts), so they inherit a live session, `teacher` or
//   `admin` trust, the round-28 invite gate, the per-account and per-address rate limits and the pool-key
//   check on writes.
// * A circle or bank the caller is not an ACTIVE member of answers 404 not_found (never 403; former
//   members included), and counts as a miss. Admins are not members by being admins (admin routes in
//   circleadmin.ts). Inside a circle the caller belongs to, a missing role is `403 circle_role_required`.
// * Every write the caller's request makes is guarded in the same statement or atomic batch by ALIVE_SQL
//   (round 27): a request authenticated just before an erasure writes nothing and answers 401.
// * Circle reads write no `served` rows, no authorship records and no licence history: circles stay out
//   of the public licence engine (plan section 7); the one link is the public publish check
//   (circlecheck.ts).
// * Every statement reads rows linear in its inputs through an index chosen on purpose (test
//   round32.test.ts checks plans and rows visited at the caps).
import { cleanText } from "../../../auth/src/validate.ts";
import { INVITE_ALPHABET, INVITE_BYTES, INVITE_CHARS } from "./invites.ts";
import { allowedByAll, isLicense, LICENSES } from "./license.ts";
import { deleteKeys, gcBlobs } from "./moderation.ts";
import { readHold, streamedPayload } from "./read.ts";
import { bytesBody, DECISION_FIELDS, decisionsJson, decisionsOf, hashPairs, keysHiddenUnavailable, NO_DECISIONS, readStripDecisions, sameDecisions, STRIPPED_QKEYS_SQL, stripStored, strippedForViewer, withStoredFull, type StripDecisions } from "./circlehide.ts";
import { CAP_FLAG_SQL, capDedup, chargeHeldPublishes, COPYRIGHT_SQL, EVIDENCE_FLAG_SQL, evidenceBound, FLAG_ROOM_SQL, imagesAt, LISTING_REFRESH_SQL, PINNED_EVIDENCE_SQL, PRIVACY_SQL, privacyScan, PRUNED_NOW_SQL, type Scan } from "./circlesend.ts";
import { questionsOf, type Stripped } from "./core.ts";
import { invalid as stripRefusal } from "./publish.ts";
import {
  ALIVE_SQL,
  audit,
  capAlert,
  countMiss,
  DAY_MS,
  dayOf,
  FLAG_DETAIL_MAX,
  giveDaily,
  ID_RE,
  json,
  limitOf,
  missGuard,
  newId,
  notFound,
  PoolError,
  rawJson,
  READ_HEAD_COST,
  readJson,
  readOptionalJson,
  requireDaily,
  alertBudgeted,
  SHA_RE,
  sha256Hex,
  takeDaily,
  throwIfErased,
  type Ctx,
  type PoolPrincipal,
} from "./shared.ts";

export const CIRCLE_NAME_MAX = 80;
export const CIRCLE_DESC_MAX = 1000;
export const CIRCLE_NOTE_MAX = 200;
export const CIRCLE_CODE_MAX_USES = 50;
export const CIRCLE_CODE_DAYS = 7;
export const CIRCLE_CODE_MAX_DAYS = 30;
/** An owner close, a bank deletion: hidden at once, undoable this long, then purged (D7). */
export const CIRCLE_UNDO_MS = 14 * DAY_MS;
/** Banks one POST /circles/heads may ask about. */
export const HEADS_MAX = 200;
/** Activity entries a page (D8). */
export const ACTIVITY_PAGE = 50;
/** Invites one page of GET /circles/:cid/invites lists. */
export const CIRCLE_INVITE_PAGE = 100;
/** Versions of a bank kept in full (D16); older ones keep their history line only. */
export const KEPT_VERSIONS = 20;
/** Owner closes in 30 days past which the account is flagged for the admin (D11). */
export const CIRCLE_CLOSES_FLAG = 5;
/** A reporter must have been a member this long for a student-data report to hide a bank at once (D1). */
export const ESTABLISHED_MEMBER_MS = DAY_MS;
/** ... and an account approved this long (the public pool's NEW_ACCOUNT_MS). */
export const ESTABLISHED_ACCOUNT_MS = 7 * DAY_MS;
/** Instant student-data holds one reporter may cause a day (the public pool's INSTANT_HOLDS_PER_DAY). */
export const CIRCLE_INSTANT_HOLDS_PER_DAY = 3;
/** A reporter whose student-data report was dismissed cannot hide a bank instantly for this long. */
export const CIRCLE_THROTTLE_MS = 30 * DAY_MS;
export const CIRCLE_REPORT_REASONS = ["student_data", "copyright", "inappropriate", "other"] as const;
export const DELETED_NAME = "deleted teacher";

// ---- small helpers ------------------------------------------------------------------------------------

const bad = (field: string, extra: Record<string, unknown> = {}) => new PoolError(400, "invalid_request", { field, ...extra });

/** Optional cleaned text: absent, null or "" is null; anything else must clean (else 400 on `field`). */
function optText(v: unknown, max: number, field: string): string | null {
  if (v === undefined || v === null || v === "") return null;
  const s = cleanText(v, max);
  if (!s) throw bad(field, { max });
  return s;
}
function intIn(v: unknown, lo: number, hi: number, field: string): number {
  if (typeof v !== "number" || !Number.isInteger(v) || v < lo || v > hi) throw bad(field, { min: lo, max: hi });
  return v;
}
const USER_RE = /^[0-9a-f-]{36}$/;
function userIdOf(v: unknown): string {
  if (typeof v !== "string" || !USER_RE.test(v)) throw bad("user_id");
  return v;
}
export const nameOf = (display: string | null | undefined, deletedAt: number | null | undefined): string => (deletedAt !== null && deletedAt !== undefined ? DELETED_NAME : display ?? DELETED_NAME);

/** Round 36 (critic44 B1): a circle response answers missing text as "", never null (the app reads plain strings). */
export const text = (v: string | null | undefined): string => v ?? "";
/** Round 36 (critic44 variant 2): who closed a circle, from its close reason (null while open). */
export const closedBy = (reason: string | null): "owner" | "admin" | "last_member" | null =>
  reason === "owner_closed" ? "owner" : reason === "admin" ? "admin" : reason === "last_member_gone" ? "last_member" : null;
const CLOSED_BY_SQL = (r: string) => `CASE ${r} WHEN 'owner_closed' THEN 'owner' WHEN 'admin' THEN 'admin' WHEN 'last_member_gone' THEN 'last_member' END`;

/** The next pool_seq value (orders joins and versions without clock ties). */
export async function nextSeq(c: Ctx): Promise<number> {
  const r = await c.db.prepare(`UPDATE pool_seq SET n = n + 1 WHERE id = 1 RETURNING n`).first<{ n: number }>();
  if (!r) throw new Error("pool_seq missing");
  return r.n;
}

// ---- SQL fragments ----------------------------------------------------------------------------------------

/** `me` (a parameter reference) is an ACTIVE member of circle `cid` with one of `roles` (all roles when omitted). */
export const ACTIVE_SQL = (cid: string, me: string, roles?: readonly string[]) =>
  `EXISTS (SELECT 1 FROM circle_members am WHERE am.circle_id = ${cid} AND am.user_id = ${me} AND am.left_at IS NULL${roles ? ` AND am.role IN (${roles.map((r) => `'${r}'`).join(", ")})` : ""})`;
/** Circle `cid` is open (not closed, not purged). */
export const OPEN_SQL = (cid: string) => `EXISTS (SELECT 1 FROM circles oc WHERE oc.id = ${cid} AND oc.closed_at IS NULL AND oc.purged_at IS NULL)`;
/** Circles `me` owns now (an owner-closed circle counts until it is purged or reopened). */
export const OWNED_SQL = (me: string) =>
  `(SELECT COUNT(*) FROM circle_members om JOIN circles oc2 ON oc2.id = om.circle_id WHERE om.user_id = ${me} AND om.left_at IS NULL AND om.role = 'owner' AND oc2.purged_at IS NULL)`;
/** Circles `me` is an active member of (closed ones included until purged or left). */
export const MEMBERSHIPS_SQL = (me: string) =>
  `(SELECT COUNT(*) FROM circle_members mm JOIN circles mc ON mc.id = mm.circle_id WHERE mm.user_id = ${me} AND mm.left_at IS NULL AND mc.purged_at IS NULL)`;
/** Round 34 (critic41b F2): the account `uid` (SQL) is in good standing: approved, not suspended, not erased. */
export const IN_STANDING_SQL = (uid: string) =>
  `EXISTS (SELECT 1 FROM users su WHERE su.id = ${uid} AND su.trust IN ('teacher', 'admin') AND su.suspended_at IS NULL AND su.deleted_at IS NULL)`;
/** Round 34: bank row `b`'s content follows its author's standing (as public items do): served only while the author
 *  is in good standing, or erased with the bank kept anonymised (D15). A suspended author's banks are hidden like
 *  held ones until the account is restored. */
/**
 * The hold sources a SEND's own content check raises (round 37: student data, `privacy` / `privacy_near`; round 44: a
 * copyright re-upload block, `blocklist`): each withholds the matching version, which is what protects the content,
 * so none of them freezes the bank (BLOCKING_HOLD_SQL). The admin flags they come with: SEND_FLAG_KINDS.
 */
export const SEND_HOLD_SOURCES = `('privacy', 'privacy_near', 'blocklist')`;
export const SEND_FLAG_KINDS = `('privacy_text_match', 'blocklist')`;
/**
 * Round 39 (critic 46a M1): a hold that freezes a bank (its pruning, trim and purge, and, round 40, hides it from its
 * owner): a report's or an administrator's, kept as evidence. A student-data hold a send raised (`privacy`,
 * `privacy_near`) does not: the matching version is withheld, which is what protects the data.
 */
export const BLOCKING_HOLD_SQL = (bid: string) => `EXISTS (SELECT 1 FROM circle_holds xh WHERE xh.bank_id = ${bid} AND xh.source NOT IN ${SEND_HOLD_SOURCES})`;
export const AUTHOR_OK_SQL = (b: string) =>
  `EXISTS (SELECT 1 FROM users au WHERE au.id = ${b}.author_id AND au.trust IN ('teacher', 'admin') AND au.suspended_at IS NULL AND (au.deleted_at IS NULL OR ${b}.anonymized = 1))`;
/** Membership rows of `me`, former ones included (rows live until their circle is purged): the cap that
 *  bounds what the public publish check reads (round 33). */
export const MEMBERSHIP_ROWS_SQL = (me: string) => `(SELECT COUNT(*) FROM circle_members rm WHERE rm.user_id = ${me})`;
/**
 * Round 33 (critics 40a B1, 40b B1): a code invite `i` lets `me` (re)join only if it was issued AFTER `me`
 * was removed from its circle (a removal is undone only by a code the owner issued since), and only while
 * its issuer is still the circle's active owner (a code never outlives its issuer's owner role).
 */
export const CODE_USABLE_SQL = (i: string, me: string) =>
  `(NOT EXISTS (SELECT 1 FROM circle_members rr WHERE rr.circle_id = ${i}.circle_id AND rr.user_id = ${me} AND rr.left_reason = 'removed' AND rr.left_at >= ${i}.created_at)
    AND EXISTS (SELECT 1 FROM circle_members io WHERE io.circle_id = ${i}.circle_id AND io.user_id = ${i}.created_by AND io.role = 'owner' AND io.left_at IS NULL)
    AND ${IN_STANDING_SQL(`${i}.created_by`)})`;
// (round 34: the last line, the issuer in good standing: a suspended owner's codes admit nobody. Round 35: no SQL
// comment inside a fragment other statements embed.)
/** The role `me` (re)joins with by code `i`: a member who left (or was demoted, then left) after the code was
 *  issued comes back at most as the role held when leaving; a newer code gives its own role. */
export const JOIN_ROLE_SQL = (i: string, me: string) =>
  `CASE WHEN EXISTS (SELECT 1 FROM circle_members jr WHERE jr.circle_id = ${i}.circle_id AND jr.user_id = ${me} AND jr.left_at IS NOT NULL AND jr.left_at >= ${i}.created_at AND jr.role = 'viewer') THEN 'viewer' ELSE ${i}.role END`;
/** Active members of circle `cid` (the partial index). */
export const MEMBERS_SQL = (cid: string) => `(SELECT COUNT(*) FROM circle_members xm WHERE xm.circle_id = ${cid} AND xm.left_at IS NULL)`;
/** Usable codes of circle `cid` at `now` (a code lives at most CIRCLE_CODE_MAX_DAYS: only recent rows are read). Round
 *  47 (critic 54a variant 2): of one kind, plain codes (circle_open_invites) or email invites (circle_open_email_invites). */
export const OPEN_INVITES_SQL = (cid: string, now: string, kind: "code" | "email" = "code") =>
  `(SELECT COUNT(*) FROM circle_invites oi WHERE oi.circle_id = ${cid} AND oi.created_at > ${now} - ${CIRCLE_CODE_MAX_DAYS * DAY_MS} AND oi.revoked_at IS NULL AND oi.expires_at > ${now} AND oi.uses < oi.max_uses AND oi.kind = '${kind}')`;

/**
 * Ownership succession (decision 3, D14, D18; plan 6.3) for every circle in `scope` (a SQL subquery of
 * circle ids) that is OPEN, or closed by its owner and still inside the undo window (round 34: it stays
 * closed, the successor may reopen it), and has no active owner: the active editor in good standing with
 * the smallest join_seq becomes owner; where there is still none, such a viewer; an open circle with no
 * member in good standing left closes ('last_member_gone', purged after `purgeAt`). Each pick is one statement whose
 * condition is "no active owner", so a rerun (erasure_finish, the daily resweep) finds exactly what is
 * left to do, and the partial unique index allows one active owner. Audited (system) for the activity
 * list. `now` and `purgeAt` are parameter references.
 */
export function successionStatements(db: D1Database, scope: string, binds: unknown[], now: string, purgeAt: string): D1PreparedStatement[] {
  // (Each statement binds exactly the parameters it uses: D1 refuses extra bindings.)
  const prep = (sql: string) => db.prepare(sql).bind(...binds.slice(0, Math.max(0, ...[...sql.matchAll(/\?(\d+)/g)].map((x) => Number(x[1])))));
  // (Round 34: also an owner-closed circle inside its undo window, which stays closed: the successor may reopen it;
  // and only members in good standing are picked, critic41a 4, critic41b F2.)
  const needs = `c.id IN (${scope}) AND c.purged_at IS NULL AND (c.closed_at IS NULL OR (c.close_reason = 'owner_closed' AND c.undo_until > ${now}))
      AND NOT EXISTS (SELECT 1 FROM circle_members o WHERE o.circle_id = c.id AND o.role = 'owner' AND o.left_at IS NULL)`;
  const pick = (role: string, col: string) =>
    `(SELECT m.${col} FROM circle_members m WHERE m.circle_id = c.id AND m.left_at IS NULL AND m.role = '${role}' AND ${IN_STANDING_SQL("m.user_id")} ORDER BY m.join_seq LIMIT 1)`;
  const step = (role: "editor" | "viewer") => [
    prep(
      `INSERT INTO pool_audit (id, at, actor_id, actor_kind, action, item_id, target_user_id, detail)
       SELECT lower(hex(randomblob(16))), ${now}, NULL, 'system', 'circle.owner_succession', c.id, ${pick(role, "user_id")}, json_object('from', '${role}')
         FROM circles c WHERE ${needs} AND ${pick(role, "user_id")} IS NOT NULL`,
    ),
    prep(`UPDATE circle_members SET role = 'owner' WHERE rowid IN (SELECT ${pick(role, "rowid")} FROM circles c WHERE ${needs})`),
  ];
  // (an open circle with no owner and no member in good standing left closes)
  const empty = `id IN (${scope}) AND closed_at IS NULL AND purged_at IS NULL
      AND NOT EXISTS (SELECT 1 FROM circle_members o WHERE o.circle_id = circles.id AND o.role = 'owner' AND o.left_at IS NULL)
      AND NOT EXISTS (SELECT 1 FROM circle_members o WHERE o.circle_id = circles.id AND o.left_at IS NULL AND ${IN_STANDING_SQL("o.user_id")})`;
  return [
    ...step("editor"),
    ...step("viewer"),
    prep(
      `INSERT INTO pool_audit (id, at, actor_id, actor_kind, action, item_id, detail)
       SELECT lower(hex(randomblob(16))), ${now}, NULL, 'system', 'circle.close', id, json_object('reason', 'last_member_gone') FROM circles WHERE ${empty}`,
    ),
    prep(`UPDATE circles SET closed_at = ${now}, close_reason = 'last_member_gone', undo_until = NULL, purge_after = ${purgeAt}, updated_at = ${now} WHERE ${empty}`),
  ];
}

// ---- membership lookups -----------------------------------------------------------------------------------

export interface Membership {
  id: string;
  name: string;
  description: string | null;
  created_at: number;
  updated_at: number;
  closed_at: number | null;
  close_reason: string | null;
  undo_until: number | null;
  purge_after: number | null;
  role: "owner" | "editor" | "viewer";
  joined_at: number;
}

/** The circle and the caller's ACTIVE membership of it, or null (not a member, former member, purged). */
export async function memberOf(c: Ctx, cid: string, me: string): Promise<Membership | null> {
  return c.db
    .prepare(
      `SELECT ci.id, ci.name, ci.description, ci.created_at, ci.updated_at, ci.closed_at, ci.close_reason, ci.undo_until, ci.purge_after, m.role, m.joined_at
         FROM circles ci JOIN circle_members m ON m.circle_id = ci.id AND m.user_id = ?2 AND m.left_at IS NULL
        WHERE ci.id = ?1 AND ci.purged_at IS NULL`,
    )
    .bind(cid, me)
    .first<Membership>();
}

/** The caller's membership, or a counted 404 (the same for a circle that does not exist). */
export async function circleOrMiss(c: Ctx, cid: string, me: string): Promise<Membership> {
  await missGuard(c, me, "item_miss");
  const m = ID_RE.test(cid) ? await memberOf(c, cid, me) : null;
  if (m) return m;
  await countMiss(c, me, "item_miss");
  throw notFound();
}

export function closedError(m: { closed_at: number | null; close_reason: string | null; undo_until: number | null }): PoolError {
  return new PoolError(409, "circle_closed", {
    reason: m.close_reason,
    undo_until: m.undo_until,
    message: m.close_reason === "owner_closed" ? "The owner closed this circle." : m.close_reason === "admin" ? "An administrator closed this circle." : "This circle closed when its last member left.",
  });
}
export function requireOpen(m: Membership): void {
  if (m.closed_at !== null) throw closedError(m);
}
export function requireRole(m: Membership, ...roles: ("owner" | "editor" | "viewer")[]): void {
  if (!roles.includes(m.role)) throw new PoolError(403, "circle_role_required", { needed: roles.includes("editor") ? "editor" : roles[0], role: m.role });
}

/** After a guarded write changed nothing: 401 if the account is erased, else what the state says now. */
export async function explain(c: Ctx, cid: string, me: string, roles?: ("owner" | "editor" | "viewer")[]): Promise<never> {
  await throwIfErased(c, me);
  const m = await memberOf(c, cid, me);
  if (!m) throw notFound();
  requireOpen(m);
  if (roles) requireRole(m, ...roles);
  throw new PoolError(409, "conflict", { message: "Something changed meanwhile; reload the circle and try again." });
}

// ---- codes ------------------------------------------------------------------------------------------------

/** A circle code: "C" and 24 Crockford base32 characters (120 bits), shown as C-XXXX-XXXX-XXXX-XXXX-XXXX-XXXX.
 *  The prefix tells the app (and the server) a circle code from a pool invite code typed into the wrong box. */
export function newCircleCode(): string {
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
  return `C-${out.match(/.{4}/g)!.join("-")}`;
}
/** The normalised form of a typed circle code (case, spaces and hyphens ignored; I/L read as 1, O as 0), or null. */
export function normaliseCircleCode(raw: unknown): string | null {
  if (typeof raw !== "string" || raw.length > 100) return null;
  const s = raw.toUpperCase().replace(/[\s-]/g, "");
  if (s.length !== INVITE_CHARS + 1 || s[0] !== "C") return null;
  const body = s.slice(1).replace(/[IL]/g, "1").replace(/O/g, "0");
  for (const ch of body) if (!INVITE_ALPHABET.includes(ch)) return null;
  return `C${body}`;
}
export const circleCodeHash = (normalised: string): Promise<string> => sha256Hex(new TextEncoder().encode(`circle:${normalised}`));

// ---- POST /circles, GET /circles --------------------------------------------------------------------------

/** Round 34 (critic41a 2): the circles_owned refusal says a closed circle still counts for 14 days. */
export const ownedMessage = (cap: number, mine: boolean) =>
  `${mine ? "You own" : "That would make"} ${cap}${mine ? "" : "+"} circles, the most one account may own. A closed circle still counts for 14 days, until it is purged; you can also pass one on.`;
async function capsOf(c: Ctx, me: string) {
  return { owned: await limitOf(c, me, "circles_owned"), memberships: await limitOf(c, me, "circle_memberships"), rows: await limitOf(c, me, "circle_memberships_total") };
}
async function limitError(c: Ctx, me: string, caps: { owned: number; memberships: number; rows: number }, owning: boolean): Promise<PoolError> {
  const r = await c.db.prepare(`SELECT ${OWNED_SQL("?1")} AS owned, ${MEMBERSHIPS_SQL("?1")} AS memberships, ${MEMBERSHIP_ROWS_SQL("?1")} AS rows_`).bind(me).first<{ owned: number; memberships: number; rows_: number }>();
  if (owning && (r?.owned ?? 0) >= caps.owned) return new PoolError(422, "circles_limit", { metric: "circles_owned", used: r?.owned ?? 0, limit: caps.owned, message: ownedMessage(caps.owned, true) });
  if ((r?.memberships ?? 0) >= caps.memberships)
    return new PoolError(422, "circles_limit", { metric: "circle_memberships", used: r?.memberships ?? 0, limit: caps.memberships, message: `You are in ${caps.memberships} circles, the most allowed. Leave one first.` });
  return new PoolError(422, "circles_limit", {
    metric: "circle_memberships_total",
    used: r?.rows_ ?? 0,
    limit: caps.rows,
    message: `You have been in ${caps.rows} circles that still exist. You can join another once some of them close.`,
  });
}

export async function createCircle(c: Ctx, req: Request, p: PoolPrincipal): Promise<Response> {
  const me = p.userId;
  const b = await readJson(req, `u:${me}`, { deadlineMs: c.deps.bodyDeadlineMs });
  const name = cleanText(b.name, CIRCLE_NAME_MAX);
  if (!name) throw bad("name", { max: CIRCLE_NAME_MAX });
  const description = optText(b.description, CIRCLE_DESC_MAX, "description");
  await requireDaily(c, me, "circle_creates");
  const caps = await capsOf(c, me);
  const now = c.now();
  const id = newId();
  const seq = await nextSeq(c);
  const [ins] = await c.db.batch([
    c.db
      .prepare(
        `INSERT INTO circles (id, name, description, created_by, created_at, updated_at) SELECT ?1, ?2, ?3, ?4, ?5, ?5
          WHERE ${ALIVE_SQL("?4")} AND ${OWNED_SQL("?4")} < ?6 AND ${MEMBERSHIPS_SQL("?4")} < ?7 AND ${MEMBERSHIP_ROWS_SQL("?4")} < ?8`,
      )
      .bind(id, name, description, me, now, caps.owned, caps.memberships, caps.rows),
    c.db.prepare(`INSERT INTO circle_members (circle_id, user_id, role, joined_at, join_seq) SELECT ?1, ?2, 'owner', ?3, ?4 WHERE changes() = 1`).bind(id, me, now, seq),
    audit(c.db, { actorId: me, actorKind: "user", action: "circle.create", itemId: id, targetUserId: me }, now, `changes() = 1`),
  ]);
  if ((ins!.meta.changes ?? 0) !== 1) {
    await throwIfErased(c, me);
    await giveDaily(c, me, "circle_creates"); // (not a create)
    throw await limitError(c, me, caps, true);
  }
  return json(201, { circle: { id, name, description: text(description), role: "owner", created_at: now, updated_at: now, members: 1, banks: 0, closed_at: null, close_reason: null, closed_by: null, undo_until: null } });
}

export async function listCircles(c: Ctx, p: PoolPrincipal): Promise<Response> {
  const me = p.userId;
  // At most circle_memberships rows; members and banks counted by index (not stored, so they cannot drift).
  const rows = await c.db
    .prepare(
      `SELECT ci.id, COALESCE(ci.name, '') AS name, m.role, m.joined_at, ci.closed_at, ci.close_reason, ${CLOSED_BY_SQL("ci.close_reason")} AS closed_by, ci.undo_until, ci.updated_at,
              ${MEMBERS_SQL("ci.id")} AS members,
              (SELECT COUNT(*) FROM circle_banks b WHERE b.circle_id = ci.id AND b.purged_at IS NULL AND b.removed_at IS NULL AND b.head_version > 0) AS banks,
              (SELECT t.expires_at FROM circle_transfers t WHERE t.circle_id = ci.id AND t.to_user = ?1 AND t.expires_at > ?2) AS ownership_offered_until
         FROM circle_members m JOIN circles ci ON ci.id = m.circle_id
        WHERE m.user_id = ?1 AND m.left_at IS NULL AND ci.purged_at IS NULL
        ORDER BY m.joined_at DESC, ci.id`,
    )
    .bind(me, c.now())
    .all<Record<string, unknown>>();
  return json(200, { circles: rows.results });
}

// ---- GET / PATCH / DELETE /circles/:cid, POST reopen -------------------------------------------------------

export async function getCircle(c: Ctx, p: PoolPrincipal, cid: string): Promise<Response> {
  const me = p.userId;
  const m = await circleOrMiss(c, cid, me);
  const members = await c.db
    .prepare(
      `SELECT m.user_id, m.role, m.joined_at, u.display_name, u.deleted_at FROM circle_members m LEFT JOIN users u ON u.id = m.user_id
        WHERE m.circle_id = ?1 AND m.left_at IS NULL ORDER BY m.join_seq`,
    )
    .bind(cid)
    .all<{ user_id: string; role: string; joined_at: number; display_name: string | null; deleted_at: number | null }>();
  const banks = await c.db
    .prepare(
      `SELECT b.id, b.title, b.subject, b.course, b.grade, b.license, b.head_version, b.question_count, b.has_images, b.created_at, b.updated_at, b.hide_key,
              b.author_id, b.anonymized, b.removed_at, b.removed_reason, b.purge_after, u.display_name, u.deleted_at,
              EXISTS (SELECT 1 FROM circle_holds h WHERE h.bank_id = b.id) AS held, ${BLOCKING_HOLD_SQL("b.id")} AS blocking, ${AUTHOR_OK_SQL("b")} AS author_ok
         FROM circle_banks b LEFT JOIN users u ON u.id = b.author_id
        WHERE b.circle_id = ?1 AND b.purged_at IS NULL AND b.head_version > 0 ORDER BY b.created_at, b.id`,
    )
    .bind(cid)
    .all<Record<string, unknown> & { id: string; author_id: string; anonymized: number; removed_at: number | null; removed_reason: string | null; purge_after: number | null; held: number; blocking: number; author_ok: number; display_name: string | null; deleted_at: number | null }>();
  const owner = m.role === "owner";
  // (round 34: the pending ownership offer, to the owner and to the member offered it)
  const tr = await c.db
    .prepare(`SELECT t.from_user, t.to_user, t.expires_at, u.display_name, u.deleted_at FROM circle_transfers t LEFT JOIN users u ON u.id = t.to_user WHERE t.circle_id = ?1 AND t.expires_at > ?2`)
    .bind(cid, c.now())
    .first<{ from_user: string; to_user: string; expires_at: number; display_name: string | null; deleted_at: number | null }>();
  const pending = tr && (owner || tr.to_user === me) ? { to_user_id: tr.to_user, to_name: nameOf(tr.display_name, tr.deleted_at), expires_at: tr.expires_at } : null;
  const shown = banks.results
    .filter((b) => {
      const mine = b.author_id === me;
      // Round 35 (critic42 B1): a REMOVED bank is listed to the owner and its author whatever the author's standing
      // (an erased or suspended author's removed banks still count toward the caps until purged: the owner must
      // find them to purge them); a live one only when its author is in good standing and it is not held (but to
      // its author).
      if (b.removed_at !== null) return mine || owner;
      if (b.author_ok !== 1) return false;
      // (round 40, critic 47a V1: a bank held only for student data is listed to the owner too, who can repair it)
      return b.held === 0 || mine || (owner && b.blocking === 0);
    })
    .map((b) => {
      const mine = b.author_id === me;
      // (a removed bank that is held, or whose author is not in good standing, is listed without its text: F1's
      // generic presentation; the owner gets the id to purge or restore it, nothing a hold hides; round 40: a bank
      // held only for student data shows the owner its listing, which never comes from a withheld version)
      const masked = !mine && (b.author_ok !== 1 || (b.held === 1 && !(owner && b.blocking === 0)));
      return {
        id: b.id,
        title: masked ? null : b.title,
        subject: masked ? null : b.subject,
        course: masked ? null : b.course,
        grade: masked ? null : b.grade,
        license: b.license,
        author_name: masked ? null : b.anonymized === 1 ? DELETED_NAME : nameOf(b.display_name, b.deleted_at),
        is_author: mine,
        head_version: b.head_version,
        question_count: b.question_count,
        has_images: b.has_images === 1,
        // (round 43, D12: its viewers read it without the answer keys)
        hide_key: b.hide_key === 1,
        created_at: b.created_at,
        updated_at: b.updated_at,
        ...(masked ? { hidden: true } : b.held === 1 ? { held: true } : {}),
        ...(b.removed_at !== null ? { removed: { reason: b.removed_reason, undo_until: b.removed_reason === "author_removed" || b.removed_reason === "owner_removed" ? b.purge_after : null } } : {}),
      };
    });
  return json(200, {
    circle: {
      id: m.id,
      name: text(m.name),
      description: text(m.description),
      role: m.role,
      joined_at: m.joined_at,
      created_at: m.created_at,
      updated_at: m.updated_at,
      closed_at: m.closed_at,
      close_reason: m.close_reason,
      closed_by: closedBy(m.close_reason),
      undo_until: m.undo_until,
    },
    members: members.results.map((x) => ({ user_id: x.user_id, display_name: nameOf(x.display_name, x.deleted_at), role: x.role, joined_at: x.joined_at, is_me: x.user_id === me })),
    banks: shown,
    pending_transfer: pending,
  });
}

export async function patchCircle(c: Ctx, req: Request, p: PoolPrincipal, cid: string): Promise<Response> {
  const me = p.userId;
  const m = await circleOrMiss(c, cid, me);
  const b = await readJson(req, `u:${me}`, { deadlineMs: c.deps.bodyDeadlineMs });
  requireOpen(m);
  requireRole(m, "owner");
  const name = b.name === undefined ? m.name : cleanText(b.name, CIRCLE_NAME_MAX);
  if (!name) throw bad("name", { max: CIRCLE_NAME_MAX });
  const description = b.description === undefined ? m.description : optText(b.description, CIRCLE_DESC_MAX, "description");
  const now = c.now();
  const [upd] = await c.db.batch([
    c.db
      .prepare(`UPDATE circles SET name = ?3, description = ?4, updated_at = ?5 WHERE id = ?1 AND closed_at IS NULL AND purged_at IS NULL AND ${ACTIVE_SQL("?1", "?2", ["owner"])} AND ${ALIVE_SQL("?2")}`)
      .bind(cid, me, name, description, now),
    audit(c.db, { actorId: me, actorKind: "user", action: "circle.edit", itemId: cid, targetUserId: me }, now, `changes() = 1`),
  ]);
  if ((upd!.meta.changes ?? 0) !== 1) await explain(c, cid, me, ["owner"]);
  return json(200, { ok: true, name, description: text(description) });
}

/** DELETE /circles/:cid (owner): closed at once for everyone, 14 days to undo, then purged (D7). */
export async function closeCircle(c: Ctx, p: PoolPrincipal, cid: string): Promise<Response> {
  const me = p.userId;
  const m = await circleOrMiss(c, cid, me);
  requireRole(m, "owner");
  // Already closed by the owner: the same answer (the window is never extended).
  if (m.closed_at !== null) {
    if (m.close_reason === "owner_closed") return json(200, { ok: true, undo_until: m.undo_until });
    throw closedError(m);
  }
  const now = c.now();
  const [upd] = await c.db.batch([
    c.db
      .prepare(
        `UPDATE circles SET closed_at = ?3, close_reason = 'owner_closed', undo_until = ?4, purge_after = ?4, updated_at = ?3
          WHERE id = ?1 AND closed_at IS NULL AND purged_at IS NULL AND ${ACTIVE_SQL("?1", "?2", ["owner"])} AND ${ALIVE_SQL("?2")}`,
      )
      .bind(cid, me, now, now + CIRCLE_UNDO_MS),
    audit(c.db, { actorId: me, actorKind: "user", action: "circle.close", itemId: cid, targetUserId: me, detail: { reason: "owner_closed" } }, now, `changes() = 1`),
  ]);
  if ((upd!.meta.changes ?? 0) !== 1) await explain(c, cid, me, ["owner"]);
  // D11: an account that creates and closes many circles is flagged for the admin.
  const closes = await c.db
    .prepare(`SELECT COUNT(*) AS n FROM pool_audit WHERE actor_id = ?1 AND action = 'circle.close' AND at > ?2`)
    .bind(me, now - 30 * DAY_MS)
    .first<{ n: number }>();
  if ((closes?.n ?? 0) >= CIRCLE_CLOSES_FLAG) await capAlert(c, me, "circle_closes");
  return json(200, { ok: true, undo_until: now + CIRCLE_UNDO_MS });
}

export async function reopenCircle(c: Ctx, p: PoolPrincipal, cid: string): Promise<Response> {
  const me = p.userId;
  const m = await circleOrMiss(c, cid, me);
  requireRole(m, "owner");
  if (m.closed_at === null) return json(200, { ok: true, already: true });
  const now = c.now();
  const [upd] = await c.db.batch([
    c.db
      .prepare(
        `UPDATE circles SET closed_at = NULL, close_reason = NULL, undo_until = NULL, purge_after = NULL, updated_at = ?3
          WHERE id = ?1 AND close_reason = 'owner_closed' AND undo_until > ?3 AND purged_at IS NULL AND ${ACTIVE_SQL("?1", "?2", ["owner"])} AND ${ALIVE_SQL("?2")}`,
      )
      .bind(cid, me, now),
    audit(c.db, { actorId: me, actorKind: "user", action: "circle.reopen", itemId: cid, targetUserId: me }, now, `changes() = 1`),
  ]);
  if ((upd!.meta.changes ?? 0) !== 1) {
    await throwIfErased(c, me);
    throw new PoolError(409, "circle_closed", { reason: m.close_reason, undo_until: m.undo_until, message: "This circle can no longer be reopened." });
  }
  return json(200, { ok: true });
}

// ---- invite codes -------------------------------------------------------------------------------------------

/** POST /circles/:cid/invites {role, max_uses?, expires_in_days?, note?} (owner): the code is in this answer only. */
export async function createInvite(c: Ctx, req: Request, p: PoolPrincipal, cid: string): Promise<Response> {
  const me = p.userId;
  const m = await circleOrMiss(c, cid, me);
  const b = await readJson(req, `u:${me}`, { deadlineMs: c.deps.bodyDeadlineMs });
  requireOpen(m);
  requireRole(m, "owner");
  const role = b.role;
  if (role !== "editor" && role !== "viewer") throw bad("role");
  const maxUses = b.max_uses === undefined || b.max_uses === null ? 1 : intIn(b.max_uses, 1, CIRCLE_CODE_MAX_USES, "max_uses");
  const days = b.expires_in_days === undefined || b.expires_in_days === null ? CIRCLE_CODE_DAYS : intIn(b.expires_in_days, 1, CIRCLE_CODE_MAX_DAYS, "expires_in_days");
  const note = optText(b.note, CIRCLE_NOTE_MAX, "note");
  await requireDaily(c, me, "circle_invites");
  const cap = await limitOf(c, "*", "circle_open_invites");
  const now = c.now();
  const code = newCircleCode();
  const id = newId();
  const expiresAt = now + days * DAY_MS;
  const [ins] = await c.db.batch([
    c.db
      .prepare(
        `INSERT INTO circle_invites (id, circle_id, code_hash, role, created_by, created_at, max_uses, uses, expires_at, note)
         SELECT ?1, ?2, ?3, ?4, ?5, ?6, ?7, 0, ?8, ?9
          WHERE ${OPEN_SQL("?2")} AND ${ACTIVE_SQL("?2", "?5", ["owner"])} AND ${ALIVE_SQL("?5")} AND ${OPEN_INVITES_SQL("?2", "?6")} < ?10`,
      )
      .bind(id, cid, await circleCodeHash(normaliseCircleCode(code)!), role, me, now, maxUses, expiresAt, note, cap),
    // (the code itself is never written anywhere: neither the row nor the audit has it)
    audit(c.db, { actorId: me, actorKind: "user", action: "circle.invite_create", itemId: cid, targetUserId: me, detail: { invite_id: id, role, max_uses: maxUses, expires_at: expiresAt } }, now, `changes() = 1`),
  ]);
  if ((ins!.meta.changes ?? 0) !== 1) {
    await throwIfErased(c, me);
    await giveDaily(c, me, "circle_invites");
    const n = await c.db.prepare(`SELECT ${OPEN_INVITES_SQL("?1", "?2")} AS n`).bind(cid, now).first<{ n: number }>();
    if ((n?.n ?? 0) >= cap) throw new PoolError(422, "circles_limit", { metric: "circle_open_invites", used: n?.n ?? 0, limit: cap, message: `This circle has ${cap} open codes. Withdraw one first.` });
    await explain(c, cid, me, ["owner"]);
  }
  return json(201, { invite_id: id, code, role, max_uses: maxUses, expires_at: expiresAt, note: text(note), shown_once: true });
}

/** GET /circles/:cid/invites (owner): newest first, paged by `after` (= the last one's created_at:id). Never who typed a wrong code. */
export async function listInvites(c: Ctx, p: PoolPrincipal, cid: string, url: URL): Promise<Response> {
  const me = p.userId;
  const m = await circleOrMiss(c, cid, me);
  requireRole(m, "owner");
  const cur = /^(\d{1,16}):([0-9a-f]{32})$/.exec(url.searchParams.get("after") ?? "");
  const rows = await c.db
    .prepare(
      // (round 42: `kind` says which codes were mailed; a mailed code's address is never stored, so never listed)
      // (round 46: an email invite's masked address, "j***@gmail.com", so the owner can tell invites apart)
      `SELECT i.id AS invite_id, i.kind, COALESCE(i.email_label, '') AS address_label, i.role, i.created_at, i.max_uses, i.uses, i.expires_at, i.revoked_at, COALESCE(i.note, '') AS note, i.created_by, u.display_name AS cn, u.deleted_at AS cd,
              CASE WHEN i.revoked_at IS NOT NULL THEN 'revoked' WHEN i.uses >= i.max_uses THEN 'used_up' WHEN i.expires_at <= ?2 THEN 'expired' ELSE 'active' END AS state
         FROM circle_invites i LEFT JOIN users u ON u.id = i.created_by
        WHERE i.circle_id = ?1 AND (?3 IS NULL OR i.created_at < ?3 OR (i.created_at = ?3 AND i.id < ?4)) ORDER BY i.created_at DESC, i.id DESC LIMIT ?5`,
    )
    .bind(cid, c.now(), cur ? Number(cur[1]) : null, cur ? cur[2] : "", CIRCLE_INVITE_PAGE + 1)
    .all<{ invite_id: string; created_at: number; created_by: string; cn: string | null; cd: number | null }>();
  // (round 33: who issued each code; a former owner's codes are revoked when the role passes on)
  const page = rows.results.slice(0, CIRCLE_INVITE_PAGE).map(({ cn, cd, ...x }) => ({ ...x, created_by_name: nameOf(cn, cd) }));
  const last = page[page.length - 1];
  return json(200, { invites: page, next_cursor: rows.results.length > CIRCLE_INVITE_PAGE && last ? `${last.created_at}:${last.invite_id}` : null });
}

/** POST /circles/:cid/invites/revoke {invite_id} (owner): no further joins; members who joined stay. */
export async function revokeInvite(c: Ctx, req: Request, p: PoolPrincipal, cid: string): Promise<Response> {
  const me = p.userId;
  const m = await circleOrMiss(c, cid, me);
  const b = await readJson(req, `u:${me}`, { deadlineMs: c.deps.bodyDeadlineMs });
  requireRole(m, "owner");
  const iid = b.invite_id;
  if (typeof iid !== "string" || !ID_RE.test(iid)) throw bad("invite_id");
  const there = await c.db.prepare(`SELECT revoked_at FROM circle_invites WHERE id = ?1 AND circle_id = ?2`).bind(iid, cid).first<{ revoked_at: number | null }>();
  if (!there) throw notFound();
  if (there.revoked_at !== null) throw new PoolError(409, "already_revoked", { revoked_at: there.revoked_at });
  const now = c.now();
  const [upd] = await c.db.batch([
    c.db
      .prepare(`UPDATE circle_invites SET revoked_at = ?3, revoked_by = ?4 WHERE id = ?1 AND circle_id = ?2 AND revoked_at IS NULL AND ${ACTIVE_SQL("?2", "?4", ["owner"])} AND ${ALIVE_SQL("?4")}`)
      .bind(iid, cid, now, me),
    audit(c.db, { actorId: me, actorKind: "user", action: "circle.invite_revoke", itemId: cid, targetUserId: me, detail: { invite_id: iid } }, now, `changes() = 1`),
  ]);
  if ((upd!.meta.changes ?? 0) !== 1) {
    await throwIfErased(c, me);
    const again = await c.db.prepare(`SELECT revoked_at FROM circle_invites WHERE id = ?1`).bind(iid).first<{ revoked_at: number | null }>();
    if (again?.revoked_at !== null && again?.revoked_at !== undefined) throw new PoolError(409, "already_revoked", { revoked_at: again.revoked_at });
    await explain(c, cid, me, ["owner"]);
  }
  return json(200, { ok: true, invite_id: iid });
}

// ---- join -------------------------------------------------------------------------------------------------

const invalidCode = () =>
  new PoolError(422, "circle_invite_invalid", { message: "This circle code is not valid: it may have expired, been used up or been withdrawn, or the circle closed. Ask the circle's owner for a new one." });

interface CodeRow {
  id: string;
  circle_id: string;
  /** The role this caller would (re)join with (JOIN_ROLE_SQL). */
  role: "editor" | "viewer";
  /** Round 53: the invite's own code hash and role as read, which the join's write compares and sets. */
  code_hash: string;
  invite_role: string;
  created_at: number;
  valid: number;
  member: number;
}
/**
 * The code behind `raw`: `code` is null for every refusal alike (unknown, expired, used up, revoked, its
 * circle closed). A caller who is already an active member of the code's circle is told so whatever the
 * code's state (nothing is used). One failed try is charged first (D10: failed codes a day); `refund`
 * gives it back for an outcome that is not a failure.
 */
async function lookupCode(c: Ctx, me: string, raw: unknown): Promise<{ code: CodeRow | null; refund: () => Promise<void> }> {
  const norm = normaliseCircleCode(raw);
  if (!norm) throw bad("code");
  await requireDaily(c, me, "circle_join_tries");
  const row = await c.db
    .prepare(
      `SELECT i.id, i.circle_id, ${JOIN_ROLE_SQL("i", "?3")} AS role, i.code_hash, i.role AS invite_role, i.created_at,
              (i.revoked_at IS NULL AND i.expires_at > ?2 AND i.uses < i.max_uses AND ci.closed_at IS NULL AND ci.purged_at IS NULL AND ${CODE_USABLE_SQL("i", "?3")}) AS valid,
              ${ACTIVE_SQL("i.circle_id", "?3")} AS member
         FROM circle_invites i JOIN circles ci ON ci.id = i.circle_id WHERE i.code_hash = ?1 AND ci.purged_at IS NULL`,
    )
    .bind(await circleCodeHash(norm), c.now(), me)
    .first<CodeRow>();
  return { code: row && (row.valid === 1 || row.member === 1) ? row : null, refund: () => giveDaily(c, me, "circle_join_tries") };
}

/** POST /circles/join/preview {code}: what joining would mean. Uses nothing; a bad code counts as a failed try. */
export async function joinPreview(c: Ctx, req: Request, p: PoolPrincipal): Promise<Response> {
  const me = p.userId;
  const b = await readJson(req, `u:${me}`, { deadlineMs: c.deps.bodyDeadlineMs });
  const { code, refund } = await lookupCode(c, me, b.code);
  if (!code) throw invalidCode();
  await refund();
  const r = await c.db
    .prepare(
      `SELECT ci.name, ${MEMBERS_SQL("ci.id")} AS members, u.display_name, u.deleted_at
         FROM circles ci LEFT JOIN circle_members o ON o.circle_id = ci.id AND o.role = 'owner' AND o.left_at IS NULL LEFT JOIN users u ON u.id = o.user_id
        WHERE ci.id = ?1`,
    )
    .bind(code.circle_id)
    .first<{ name: string; members: number; display_name: string | null; deleted_at: number | null }>();
  if (!r) throw invalidCode();
  return json(200, { circle: { id: code.circle_id, name: r.name, owner_name: nameOf(r.display_name, r.deleted_at), members: r.members }, role: code.role, already_member: code.member === 1 });
}

/** POST /circles/join {code}: joins with the code's role. Every refused code is the same 422. */
export async function joinCircle(c: Ctx, req: Request, p: PoolPrincipal): Promise<Response> {
  const me = p.userId;
  const b = await readJson(req, `u:${me}`, { deadlineMs: c.deps.bodyDeadlineMs });
  const { code, refund } = await lookupCode(c, me, b.code);
  if (!code) throw invalidCode();
  const already = async () => (await c.db.prepare(`SELECT role FROM circle_members WHERE circle_id = ?1 AND user_id = ?2 AND left_at IS NULL`).bind(code.circle_id, me).first<{ role: string }>()) ?? null;
  const was = code.member === 1 ? await already() : null;
  if (was) {
    await refund();
    return json(200, { ok: true, already: true, circle_id: code.circle_id, role: was.role });
  }
  const caps = await capsOf(c, me);
  const membersCap = await limitOf(c, "*", "circle_members");
  const now = c.now();
  const seq = await nextSeq(c);
  // One batch: the use is taken only while the code is good, the circle open and not full, the caller
  // under the membership cap, not already a member, and not erased (ALIVE_SQL); the membership (a new
  // row, or the old row of a former member reused) and its audit follow it by changes(). Round 53: and only while the
  // invite still has the code and the role that were read (an email invite rotated meanwhile has a new code, role,
  // issuer and expiry on the same row: the old code is dead, and the role read is no longer the invite's).
  const [use] = await c.db.batch([
    c.db
      .prepare(
        `UPDATE circle_invites SET uses = uses + 1
          WHERE id = ?1 AND code_hash = ?8 AND role = ?9 AND revoked_at IS NULL AND expires_at > ?3 AND uses < max_uses AND ${OPEN_SQL("?2")} AND ${ALIVE_SQL("?4")}
            AND NOT ${ACTIVE_SQL("?2", "?4")} AND ${MEMBERS_SQL("?2")} < ?5 AND ${MEMBERSHIPS_SQL("?4")} < ?6
            AND (${MEMBERSHIP_ROWS_SQL("?4")} < ?7 OR EXISTS (SELECT 1 FROM circle_members fr WHERE fr.circle_id = ?2 AND fr.user_id = ?4))
            AND ${CODE_USABLE_SQL("circle_invites", "?4")}`,
      )
      .bind(code.id, code.circle_id, now, me, membersCap, caps.memberships, caps.rows, code.code_hash, code.invite_role),
    // (Round 33: a former member's row is reused; the role is the code's, capped at the role held when
    // leaving if the code was issued before that: JOIN_ROLE_SQL, evaluated on the row as it was.)
    c.db
      .prepare(
        `INSERT INTO circle_members (circle_id, user_id, role, joined_at, join_seq, invite_id) SELECT ?1, ?2, ?3, ?4, ?5, ?6 WHERE changes() = 1
         ON CONFLICT (circle_id, user_id) DO UPDATE SET
           role = CASE WHEN circle_members.left_at IS NOT NULL AND circle_members.left_at >= ?7 AND circle_members.role = 'viewer' THEN 'viewer' ELSE excluded.role END,
           joined_at = excluded.joined_at, join_seq = excluded.join_seq, left_at = NULL, left_reason = NULL, invite_id = excluded.invite_id`,
      )
      .bind(code.circle_id, me, code.role, now, seq, code.id, code.created_at),
    audit(c.db, { actorId: me, actorKind: "user", action: "circle.join", itemId: code.circle_id, targetUserId: me, detail: { role: code.role, invite_id: code.id } }, now, `changes() = 1`),
  ]);
  if ((use!.meta.changes ?? 0) === 1) {
    await refund();
    return json(200, { ok: true, circle_id: code.circle_id, role: code.role });
  }
  // Nothing taken: a concurrent join of this account won, the account was erased, a cap, or the code is spent.
  const now2 = await already();
  if (now2) {
    await refund();
    return json(200, { ok: true, already: true, circle_id: code.circle_id, role: now2.role });
  }
  await throwIfErased(c, me);
  const st = await c.db.prepare(`SELECT ${MEMBERS_SQL("?1")} AS members, ${MEMBERSHIPS_SQL("?2")} AS mine, ${OPEN_SQL("?1")} AS open`).bind(code.circle_id, me).first<{ members: number; mine: number; open: number }>();
  if (st?.open === 1 && ((st?.mine ?? 0) >= caps.memberships || (await c.db.prepare(`SELECT ${MEMBERSHIP_ROWS_SQL("?1")} AS n`).bind(me).first<{ n: number }>())!.n >= caps.rows)) {
    await refund();
    throw await limitError(c, me, caps, false);
  }
  if (st?.open === 1 && (st?.members ?? 0) >= membersCap) {
    await refund();
    throw new PoolError(409, "circle_full", { limit: membersCap, message: `This circle has ${membersCap} members, the most allowed.` });
  }
  throw invalidCode();
}

// ---- leave, remove, role, transfer ----------------------------------------------------------------------------

/** Revokes `who`'s open codes of circle `cid` when `cond` holds (SQL; ?1 = cid, ?2 = who, ?3 = now). */
function revokeCodesOf(db: D1Database, cid: string, who: string, now: number, cond: string): D1PreparedStatement {
  return db.prepare(`UPDATE circle_invites SET revoked_at = ?3, revoked_by = ?2 WHERE circle_id = ?1 AND created_by = ?2 AND revoked_at IS NULL AND ${cond}`).bind(cid, who, now);
}

/** POST /circles/:cid/leave: any member. An owner leaving hands the circle on (decision 3, D14) or closes it. */
export async function leaveCircle(c: Ctx, p: PoolPrincipal, cid: string): Promise<Response> {
  const me = p.userId;
  await circleOrMiss(c, cid, me);
  const now = c.now();
  const res = await c.db.batch([
    audit(c.db, { actorId: me, actorKind: "user", action: "circle.leave", itemId: cid, targetUserId: me }, now, `${ACTIVE_SQL("?6", "?3")} AND ${ALIVE_SQL("?3")}`),
    c.db.prepare(`UPDATE circle_members SET left_at = ?3, left_reason = 'left' WHERE circle_id = ?1 AND user_id = ?2 AND left_at IS NULL AND ${ALIVE_SQL("?2")}`).bind(cid, me, now),
    // (round 33: codes never outlive their issuer's owner role)
    revokeCodesOf(c.db, cid, me, now, `EXISTS (SELECT 1 FROM circle_members lm WHERE lm.circle_id = ?1 AND lm.user_id = ?2 AND lm.left_at = ?3)`),
    // (round 34: an ownership offer from or to the leaver goes)
    c.db.prepare(`DELETE FROM circle_transfers WHERE circle_id = ?1 AND (from_user = ?2 OR to_user = ?2) AND EXISTS (SELECT 1 FROM circle_members lm WHERE lm.circle_id = ?1 AND lm.user_id = ?2 AND lm.left_at = ?3)`).bind(cid, me, now),
    ...successionStatements(c.db, `SELECT ?1`, [cid, now, now + CIRCLE_UNDO_MS], "?2", "?3"),
  ]);
  if ((res[1]!.meta.changes ?? 0) !== 1) {
    await throwIfErased(c, me);
    throw notFound();
  }
  return json(200, { ok: true });
}

/** POST /circles/:cid/members/remove {user_id} (owner; never yourself: leave instead). Immediate (D6). */
export async function removeMember(c: Ctx, req: Request, p: PoolPrincipal, cid: string): Promise<Response> {
  const me = p.userId;
  const m = await circleOrMiss(c, cid, me);
  const b = await readJson(req, `u:${me}`, { deadlineMs: c.deps.bodyDeadlineMs });
  requireOpen(m);
  requireRole(m, "owner");
  const who = userIdOf(b.user_id);
  if (who === me) throw bad("user_id", { message: "To stop being a member yourself, leave the circle." });
  const now = c.now();
  const [upd] = await c.db.batch([
    c.db
      .prepare(
        `UPDATE circle_members SET left_at = ?4, left_reason = 'removed' WHERE circle_id = ?1 AND user_id = ?3 AND left_at IS NULL AND role <> 'owner'
           AND ${OPEN_SQL("?1")} AND ${ACTIVE_SQL("?1", "?2", ["owner"])} AND ${ALIVE_SQL("?2")}`,
      )
      .bind(cid, me, who, now),
    audit(c.db, { actorId: me, actorKind: "user", action: "circle.member_remove", itemId: cid, targetUserId: who }, now, `changes() = 1`),
    revokeCodesOf(c.db, cid, who, now, `EXISTS (SELECT 1 FROM circle_members rm2 WHERE rm2.circle_id = ?1 AND rm2.user_id = ?2 AND rm2.left_reason = 'removed' AND rm2.left_at = ?3)`),
    c.db.prepare(`DELETE FROM circle_transfers WHERE circle_id = ?1 AND to_user = ?2 AND EXISTS (SELECT 1 FROM circle_members rm3 WHERE rm3.circle_id = ?1 AND rm3.user_id = ?2 AND rm3.left_reason = 'removed' AND rm3.left_at = ?3)`).bind(cid, who, now),
  ]);
  if ((upd!.meta.changes ?? 0) !== 1) {
    await throwIfErased(c, me);
    if (!(await c.db.prepare(`SELECT 1 AS x FROM circle_members WHERE circle_id = ?1 AND user_id = ?2 AND left_at IS NULL`).bind(cid, who).first())) throw notFound();
    await explain(c, cid, me, ["owner"]);
  }
  return json(200, { ok: true });
}

/** POST /circles/:cid/members/role {user_id, role: editor|viewer} (owner). */
export async function setRole(c: Ctx, req: Request, p: PoolPrincipal, cid: string): Promise<Response> {
  const me = p.userId;
  const m = await circleOrMiss(c, cid, me);
  const b = await readJson(req, `u:${me}`, { deadlineMs: c.deps.bodyDeadlineMs });
  requireOpen(m);
  requireRole(m, "owner");
  const who = userIdOf(b.user_id);
  const role = b.role;
  if (role !== "editor" && role !== "viewer") throw bad("role");
  if (who === me) throw bad("user_id", { message: "To change your own role, pass ownership on first." });
  const now = c.now();
  const [upd] = await c.db.batch([
    c.db
      .prepare(
        `UPDATE circle_members SET role = ?4 WHERE circle_id = ?1 AND user_id = ?3 AND left_at IS NULL AND role IN ('editor', 'viewer') AND role <> ?4
           AND ${OPEN_SQL("?1")} AND ${ACTIVE_SQL("?1", "?2", ["owner"])} AND ${ALIVE_SQL("?2")}`,
      )
      .bind(cid, me, who, role),
    audit(c.db, { actorId: me, actorKind: "user", action: "circle.role", itemId: cid, targetUserId: who, detail: { role } }, now, `changes() = 1`),
  ]);
  if ((upd!.meta.changes ?? 0) !== 1) {
    await throwIfErased(c, me);
    const t = await c.db.prepare(`SELECT role FROM circle_members WHERE circle_id = ?1 AND user_id = ?2 AND left_at IS NULL`).bind(cid, who).first<{ role: string }>();
    if (!t) throw notFound();
    if (t.role === role) return json(200, { ok: true, changed: false });
    await explain(c, cid, me, ["owner"]);
  }
  return json(200, { ok: true, changed: true });
}

/** How long an ownership offer stays open (round 34). */
export const TRANSFER_OFFER_MS = 7 * DAY_MS;

/**
 * POST /circles/:cid/transfer {user_id} (owner): OFFERS the circle to an active member in good standing
 * (round 34, critic41b F3: ownership is never forced on anyone). Nothing changes until they accept within 7
 * days (POST .../transfer/accept); a new offer replaces the pending one. -> 202 {ok, pending: {to_user_id,
 * to_name, expires_at}}; 409 member_unavailable for a suspended or no longer approved member.
 */
export async function offerTransfer(c: Ctx, req: Request, p: PoolPrincipal, cid: string): Promise<Response> {
  const me = p.userId;
  const m = await circleOrMiss(c, cid, me);
  const b = await readJson(req, `u:${me}`, { deadlineMs: c.deps.bodyDeadlineMs });
  requireOpen(m);
  requireRole(m, "owner");
  const who = userIdOf(b.user_id);
  if (who === me) throw bad("user_id", { message: "You already own this circle." });
  const now = c.now();
  const pendingOf = async (to: string, expires: number) => {
    const u = await c.db.prepare(`SELECT display_name, deleted_at FROM users WHERE id = ?1`).bind(to).first<{ display_name: string | null; deleted_at: number | null }>();
    return { to_user_id: to, to_name: nameOf(u?.display_name, u ? u.deleted_at : 1), expires_at: expires };
  };
  // Round 35 (critic42 B2): the same open offer again changes nothing and writes nothing (it is not charged).
  const open = await c.db.prepare(`SELECT from_user, to_user, expires_at FROM circle_transfers WHERE circle_id = ?1 AND expires_at > ?2`).bind(cid, now).first<{ from_user: string; to_user: string; expires_at: number }>();
  if (open && open.from_user === me && open.to_user === who) return json(202, { ok: true, pending: await pendingOf(who, open.expires_at) });
  // ... and offers are a daily allowance (cancels write only when they remove an offer, so they are bounded by it).
  await requireDaily(c, me, "circle_transfer_offers");
  const until = now + TRANSFER_OFFER_MS;
  const [ins] = await c.db.batch([
    c.db
      .prepare(
        `INSERT INTO circle_transfers (circle_id, from_user, to_user, created_at, expires_at) SELECT ?1, ?2, ?3, ?4, ?5
          WHERE ${OPEN_SQL("?1")} AND ${ACTIVE_SQL("?1", "?2", ["owner"])} AND ${ALIVE_SQL("?2")}
            AND EXISTS (SELECT 1 FROM circle_members t WHERE t.circle_id = ?1 AND t.user_id = ?3 AND t.left_at IS NULL AND t.role <> 'owner') AND ${IN_STANDING_SQL("?3")}
         ON CONFLICT (circle_id) DO UPDATE SET from_user = excluded.from_user, to_user = excluded.to_user, created_at = excluded.created_at, expires_at = excluded.expires_at`,
      )
      .bind(cid, me, who, now, until),
    // (round 35: keyed by the account, the circle in the detail: offers and cancels are not members' activity and
    // stay out of the activity list's scan of the circle's rows)
    audit(c.db, { actorId: me, actorKind: "user", action: "circle.transfer_offer", itemId: null, targetUserId: who, detail: { circle_id: cid } }, now, `changes() = 1`),
  ]);
  if ((ins!.meta.changes ?? 0) !== 1) {
    await giveDaily(c, me, "circle_transfer_offers");
    await throwIfErased(c, me);
    if (!(await c.db.prepare(`SELECT 1 AS x FROM circle_members WHERE circle_id = ?1 AND user_id = ?2 AND left_at IS NULL`).bind(cid, who).first())) throw notFound();
    if (!(await c.db.prepare(`SELECT ${IN_STANDING_SQL("?1")} AS ok`).bind(who).first<{ ok: number }>())?.ok)
      throw new PoolError(409, "member_unavailable", { message: "That member's account cannot take over a circle now." });
    await explain(c, cid, me, ["owner"]);
  }
  return json(202, { ok: true, pending: await pendingOf(who, until) });
}

/**
 * POST /circles/:cid/transfer/accept (the member offered the circle): takes over while the offer is open (not
 * expired, its offerer still the active owner, the circle open). The taker's circles_owned cap is checked in
 * the step-down statement itself, so two accepts at once cannot pass it. The old owner becomes an editor and
 * their open codes are revoked. -> 200 {ok, role: "owner"}; 409 no_pending_transfer; 422 circles_limit.
 */
export async function acceptTransfer(c: Ctx, p: PoolPrincipal, cid: string): Promise<Response> {
  const me = p.userId;
  const m = await circleOrMiss(c, cid, me);
  requireOpen(m);
  const now = c.now();
  const none = () => new PoolError(409, "no_pending_transfer", { message: "There is no open offer of this circle to you." });
  const t = await c.db.prepare(`SELECT from_user FROM circle_transfers WHERE circle_id = ?1 AND to_user = ?2 AND expires_at > ?3`).bind(cid, me, now).first<{ from_user: string }>();
  if (!t || m.role === "owner") throw none();
  const cap = await limitOf(c, me, "circles_owned");
  // (round 35: the taker bound, never interpolated)
  const isOwner = (p: string) => `EXISTS (SELECT 1 FROM circle_members nw WHERE nw.circle_id = ?1 AND nw.user_id = ${p} AND nw.role = 'owner' AND nw.left_at IS NULL)`;
  const res = await c.db.batch([
    c.db
      .prepare(
        `UPDATE circle_members SET role = 'editor' WHERE circle_id = ?1 AND user_id = ?3 AND role = 'owner' AND left_at IS NULL AND ${OPEN_SQL("?1")}
           AND EXISTS (SELECT 1 FROM circle_transfers tr WHERE tr.circle_id = ?1 AND tr.from_user = ?3 AND tr.to_user = ?2 AND tr.expires_at > ?4)
           AND EXISTS (SELECT 1 FROM circle_members tm WHERE tm.circle_id = ?1 AND tm.user_id = ?2 AND tm.left_at IS NULL AND tm.role <> 'owner')
           AND ${ALIVE_SQL("?2")} AND ${OWNED_SQL("?2")} < ?5`,
      )
      .bind(cid, me, t.from_user, now, cap),
    c.db
      .prepare(
        `UPDATE circle_members SET role = 'owner' WHERE circle_id = ?1 AND user_id = ?2 AND left_at IS NULL AND changes() = 1
           AND NOT EXISTS (SELECT 1 FROM circle_members o WHERE o.circle_id = ?1 AND o.role = 'owner' AND o.left_at IS NULL)`,
      )
      .bind(cid, me),
    audit(c.db, { actorId: t.from_user, actorKind: "user", action: "circle.transfer", itemId: cid, targetUserId: me, detail: { accepted: true } }, now, `changes() = 1`),
    // (the old owner's open codes go with the role, round 33; the offer is used)
    c.db.prepare(`UPDATE circle_invites SET revoked_at = ?3, revoked_by = ?2 WHERE circle_id = ?1 AND created_by = ?2 AND revoked_at IS NULL AND ${isOwner("?4")}`).bind(cid, t.from_user, now, me),
    c.db.prepare(`DELETE FROM circle_transfers WHERE circle_id = ?1 AND ${isOwner("?2")}`).bind(cid, me),
  ]);
  if ((res[1]!.meta.changes ?? 0) !== 1) {
    await throwIfErased(c, me);
    const owned = (await c.db.prepare(`SELECT ${OWNED_SQL("?1")} AS n`).bind(me).first<{ n: number }>())?.n ?? 0;
    if (owned >= cap) throw new PoolError(422, "circles_limit", { metric: "circles_owned", used: owned, limit: cap, message: ownedMessage(cap, true) });
    throw none();
  }
  return json(200, { ok: true, role: "owner" });
}

/** POST /circles/:cid/transfer/cancel: the owner withdraws the pending offer, or the member offered it declines. */
export async function cancelTransfer(c: Ctx, p: PoolPrincipal, cid: string): Promise<Response> {
  const me = p.userId;
  await circleOrMiss(c, cid, me);
  const now = c.now();
  const [del] = await c.db.batch([
    c.db.prepare(`DELETE FROM circle_transfers WHERE circle_id = ?1 AND (to_user = ?2 OR ${ACTIVE_SQL("?1", "?2", ["owner"])}) AND ${ALIVE_SQL("?2")}`).bind(cid, me),
    audit(c.db, { actorId: me, actorKind: "user", action: "circle.transfer_cancel", itemId: null, targetUserId: me, detail: { circle_id: cid } }, now, `changes() = 1`),
  ]);
  if ((del!.meta.changes ?? 0) === 0) await throwIfErased(c, me);
  return json(200, { ok: true, cancelled: (del!.meta.changes ?? 0) === 1 });
}

// ---- activity (D8) ------------------------------------------------------------------------------------------

/** What members see: joins, leaves, removals, role changes, transfers, banks and versions. Never codes, reports or admin opens. */
const ACTIVITY: Record<string, string> = {
  "circle.create": "created",
  "circle.join": "joined",
  "circle.leave": "left",
  "circle.member_remove": "removed",
  "circle.member_erased": "left",
  "circle.role": "role_changed",
  "circle.transfer": "ownership_transferred",
  "circle.owner_succession": "ownership_passed_on",
  "circle.close": "closed",
  "circle.reopen": "reopened",
  "circle.edit": "renamed",
  "circle.bank_add": "bank_shared",
  "circle.version": "version_sent",
  "circle.rollback": "rolled_back",
  "circle.versions_trimmed": "versions_trimmed",
  "circle.bank_remove": "bank_removed",
  "circle.bank_undelete": "bank_restored",
  "circle.bank_license": "license_changed",
  "circle.admin_close": "closed",
  "circle.admin_bank_remove": "bank_removed",
  // Round 45 (phase C): an email invite (who sent it and the role; never the address), and a bank's answer keys hidden
  // from viewers or shown again (recording new strip decisions for a bank that already hides them is not an event).
  "circle.email_invite": "email_invited",
  "circle.bank_hide_key": "hide_key_changed",
};
const ACTIVITY_SQL_LIST = Object.keys(ACTIVITY).map((a) => `'${a}'`).join(", ");

/**
 * GET /circles/:cid/activity?before=<at>:<id>: newest first (written order within one time), ACTIVITY_PAGE a page,
 * names but no technical data. Round 45: the cursor is the last line's time and its audit row's PERMANENT id (the
 * written order within one millisecond is found from that row, by its key), so a walk shows every line that existed
 * at its first page exactly once whatever is written meanwhile, and the cursor carries no service-wide row number
 * (round 32's `at:rowid` did); a malformed cursor is a 400, never page one again.
 */
export async function activity(c: Ctx, p: PoolPrincipal, cid: string, url: URL): Promise<Response> {
  const me = p.userId;
  const m = await circleOrMiss(c, cid, me);
  const raw = url.searchParams.get("before");
  const cur = raw === null ? null : /^(\d{1,16}):([0-9a-f-]{32,36})$/.exec(raw);
  if (raw !== null && !cur) throw bad("before", { message: "Use the next_before of the previous page." });
  // (round 46, critic 53b nitpick: the cursor's line must be one of THIS circle's, at the time it names)
  if (cur && !(await c.db.prepare(`SELECT 1 AS x FROM pool_audit WHERE id = ?1 AND item_id = ?2 AND at = ?3`).bind(cur[2]!, cid, Number(cur[1])).first()))
    throw bad("before", { message: "Use the next_before of the previous page." });
  const rows = await c.db
    .prepare(
      `SELECT a.id, a.at, a.action, a.actor_kind, a.detail, au.display_name AS an, au.deleted_at AS ad, a.actor_id IS NULL AS anull, tu.display_name AS tn, tu.deleted_at AS td, a.target_user_id IS NULL AS tnull
         FROM pool_audit a LEFT JOIN users au ON au.id = a.actor_id LEFT JOIN users tu ON tu.id = a.target_user_id
        WHERE a.item_id = ?1 AND a.action IN (${ACTIVITY_SQL_LIST})
          ${cur ? "AND a.at <= ?3 AND (a.at < ?3 OR a.rowid < (SELECT ca.rowid FROM pool_audit ca WHERE ca.id = ?4 AND ca.item_id = ?1))" : ""}
        ORDER BY a.at DESC, a.rowid DESC LIMIT ?2`,
    )
    // (a later page seeks its start in the (item_id, at) index: rows read do not grow with how deep the page is)
    .bind(cid, ACTIVITY_PAGE + 1, ...(cur ? [Number(cur[1]), cur[2]!] : []))
    .all<{ id: string; at: number; action: string; actor_kind: string; detail: string | null; an: string | null; ad: number | null; anull: number; tn: string | null; td: number | null; tnull: number }>();
  const page = rows.results.slice(0, ACTIVITY_PAGE);
  const details = page.map((r) => {
    try {
      return r.detail ? (JSON.parse(r.detail) as Record<string, unknown>) : {};
    } catch {
      return {};
    }
  });
  const bankIds = [...new Set(details.map((d) => d.bank_id).filter((x): x is string => typeof x === "string" && ID_RE.test(x)))];
  // Round 33 (critic40b V1): a bank's title only to who may see the bank, as on the circle page (held:
  // its author; removed: the owner and its author; a held bank's title can be the student data held).
  const titles = new Map<string, string>();
  if (bankIds.length)
    for (const t of (
      await c.db
        .prepare(
          `SELECT b.id, b.title, b.author_id, b.removed_at, EXISTS (SELECT 1 FROM circle_holds h WHERE h.bank_id = b.id) AS held, ${BLOCKING_HOLD_SQL("b.id")} AS blocking, ${AUTHOR_OK_SQL("b")} AS author_ok
             FROM circle_banks b WHERE b.id IN (SELECT value FROM json_each(?1)) AND b.circle_id = ?2 AND b.purged_at IS NULL`,
        )
        .bind(JSON.stringify(bankIds), cid)
        .all<{ id: string; title: string; author_id: string; removed_at: number | null; held: number; blocking: number; author_ok: number }>()
    ).results) {
      const mine = t.author_id === me;
      const ownerSees = m.role === "owner" && t.blocking === 0;
      if (t.author_ok === 1 && (mine || ((t.held === 0 || ownerSees) && (t.removed_at === null || m.role === "owner")))) titles.set(t.id, t.title);
    }
  const last = page[page.length - 1];
  return json(200, {
    activity: page.map((r, k) => {
      const d = details[k]!;
      return {
        at: r.at,
        what: ACTIVITY[r.action] ?? "other",
        by: r.actor_kind === "admin" ? "an administrator" : r.anull ? null : nameOf(r.an, r.ad),
        member: r.tnull ? null : nameOf(r.tn, r.td),
        ...(typeof d.role === "string" ? { role: d.role } : {}),
        ...(typeof d.bank_id === "string" ? { bank_id: d.bank_id, bank_title: titles.get(d.bank_id) ?? null } : {}),
        ...(typeof d.version === "number" ? { version: d.version } : {}),
        ...(typeof d.rollback_of === "number" ? { rollback_of: d.rollback_of } : {}),
        ...(typeof d.merged_from === "number" ? { merged_from: d.merged_from } : {}),
        ...(typeof d.license === "string" ? { license: d.license } : {}),
        ...(typeof d.hide_key === "boolean" ? { hide_key: d.hide_key } : {}),
      };
    }),
    next_before: rows.results.length > ACTIVITY_PAGE && last ? `${last.at}:${last.id}` : null,
  });
}

// ---- POST /circles/heads ------------------------------------------------------------------------------------

/** {bank_ids} (up to HEADS_MAX): per id the head (version, when, who, licence, round 52: `hide_key`), `removed`, `under_review` or `unavailable`. */
export async function heads(c: Ctx, req: Request, p: PoolPrincipal): Promise<Response> {
  const me = p.userId;
  const b = await readOptionalJson(req, `u:${me}`, c.deps.bodyDeadlineMs);
  const ids = b.bank_ids;
  if (!Array.isArray(ids) || ids.length === 0 || ids.length > HEADS_MAX || !ids.every((x) => typeof x === "string" && x.length <= 64)) throw bad("bank_ids", { max: HEADS_MAX });
  await requireDaily(c, me, "item_reads");
  const uniq = [...new Set(ids as string[])].filter((x) => ID_RE.test(x));
  // One statement: each id by its key (CROSS JOIN keeps the ids the outer loop), the caller's active
  // membership by key, the head by key.
  const rows = await c.db
    .prepare(
      `SELECT j.value AS id, b.author_id, b.head_version, b.head_version_id, b.updated_at, b.removed_at, b.removed_reason, b.purge_after, b.license, b.hide_key, m.role, ${BLOCKING_HOLD_SQL("b.id")} AS blocking,
              ci.closed_at, v.committed_at, v.committer_id, cu.display_name AS cn, cu.deleted_at AS cd,
              EXISTS (SELECT 1 FROM circle_holds h WHERE h.bank_id = b.id) AS held, ${AUTHOR_OK_SQL("b")} AS author_ok
         FROM json_each(?2) j
         CROSS JOIN circle_banks b ON b.id = j.value AND b.purged_at IS NULL AND b.head_version > 0
         JOIN circles ci ON ci.id = b.circle_id AND ci.purged_at IS NULL
         JOIN circle_members m ON m.circle_id = b.circle_id AND m.user_id = ?1 AND m.left_at IS NULL
         LEFT JOIN circle_versions v ON v.id = b.head_version_id
         LEFT JOIN users cu ON cu.id = v.committer_id`,
    )
    .bind(me, JSON.stringify(uniq))
    .all<{ id: string; author_id: string; head_version: number; head_version_id: string; updated_at: number; removed_at: number | null; removed_reason: string | null; purge_after: number | null; license: string; hide_key: number; role: string; blocking: number; closed_at: number | null; committed_at: number | null; committer_id: string | null; cn: string | null; cd: number | null; held: number; author_ok: number }>();
  const by = new Map(rows.results.map((r) => [r.id, r]));
  return json(200, {
    banks: (ids as string[]).map((id) => {
      const r = by.get(id);
      if (!r || r.closed_at !== null) return { bank_id: id, status: "unavailable" };
      if (r.removed_at !== null) return { bank_id: id, status: "removed", ...(r.removed_reason === "author_removed" || r.removed_reason === "owner_removed" ? { until: r.purge_after } : {}) };
      if ((r.held === 1 && r.author_id !== me && !(r.role === "owner" && r.blocking === 0)) || r.author_ok !== 1) return { bank_id: id, status: "under_review" };
      // (round 52, critic 62 I5: whether its viewers read it without the answer keys, as the circle page and the bank's
      // read tell every member who reads the bank; the app asks about pictures before a send to one that does)
      return { bank_id: id, status: "current", version: r.head_version, version_id: r.head_version_id, license: r.license, at: r.committed_at ?? r.updated_at, by: nameOf(r.cn, r.committer_id === null ? 1 : r.cd), hide_key: r.hide_key === 1 };
    }),
  });
}

// ---- bank reads ------------------------------------------------------------------------------------------------

export interface BankRow {
  id: string;
  circle_id: string;
  author_id: string;
  title: string;
  subject: string;
  course: string;
  grade: string;
  license: string;
  head_version: number;
  head_version_id: string | null;
  question_count: number;
  has_images: number;
  anonymized: number;
  created_at: number;
  updated_at: number;
  removed_at: number | null;
  removed_reason: string | null;
  purge_after: number | null;
  held: number;
  /** Round 40: a report's or an admin's hold (not only a student-data hold a send raised). */
  blocking: number;
  /** Round 34: the author is in good standing (or erased with the bank kept anonymised). */
  author_ok: number;
  /** Round 43 (D12): its viewers are served the stripped form. */
  hide_key: number;
}
const BANK_COLS = `b.id, b.circle_id, b.author_id, b.title, b.subject, b.course, b.grade, b.license, b.head_version, b.head_version_id, b.question_count, b.has_images, b.anonymized, b.hide_key,
  b.created_at, b.updated_at, b.removed_at, b.removed_reason, b.purge_after, EXISTS (SELECT 1 FROM circle_holds h WHERE h.bank_id = b.id) AS held, ${BLOCKING_HOLD_SQL("b.id")} AS blocking,
  ${AUTHOR_OK_SQL("b")} AS author_ok`;

/** The bank of circle `cid`, iff the caller is an active member (a counted 404 otherwise). Not yet checked: open, removed, held. */
export async function bankOrMiss(c: Ctx, cid: string, bid: string, me: string): Promise<{ m: Membership; b: BankRow }> {
  const m = await circleOrMiss(c, cid, me);
  const b = ID_RE.test(bid)
    ? await c.db.prepare(`SELECT ${BANK_COLS} FROM circle_banks b WHERE b.id = ?1 AND b.circle_id = ?2 AND b.purged_at IS NULL AND b.head_version > 0`).bind(bid, cid).first<BankRow>()
    : null;
  if (!b) {
    await countMiss(c, me, "item_miss");
    throw notFound();
  }
  return { m, b };
}
/** A member may READ the bank: the circle open, the bank not removed, not held (unless theirs). */
export async function requireReadable(c: Ctx, m: Membership, b: BankRow, me: string): Promise<void> {
  requireOpen(m);
  if (b.removed_at !== null) throw new PoolError(409, "bank_removed", { reason: b.removed_reason, undo_until: b.removed_reason === "author_removed" || b.removed_reason === "owner_removed" ? b.purge_after : null });
  // (round 40, critic 47a V1: the owner reads and acts on a bank held only for student data, as its author does;
  // the withheld version itself stays unreadable to both)
  if ((b.held === 1 && b.author_id !== me && !(m.role === "owner" && b.blocking === 0)) || b.author_ok !== 1) {
    await countMiss(c, me, "item_miss");
    throw notFound();
  }
}

/**
 * GET / HEAD /circles/:cid/banks/:bid: the head version, streamed. Viewing and copying are the same read (the copy is made on the device).
 * Round 37: GET .../versions/:n (`version` set) is a KEPT version the same way (the merge base); one pruned
 * (D16) is `404 version_not_retained {oldest_kept}`, one that never existed `404 not_found`.
 */
export async function getBank(c: Ctx, req: Request, p: PoolPrincipal, cid: string, bid: string, headOnly = false, version: number | null = null): Promise<Response> {
  const me = p.userId;
  const { m, b } = await bankOrMiss(c, cid, bid, me);
  await requireReadable(c, m, b, me);
  if (version !== null && (version < 1 || version > b.head_version)) {
    await countMiss(c, me, "item_miss");
    throw notFound();
  }
  // Circle reads count toward the one read budget (D10).
  await requireDaily(c, me, "item_reads");
  const release = readHold(c, req, me);
  try {
    const v = await c.db
      .prepare(
        `SELECT v.id, v.version, v.full_key, v.committed_at, v.license, v.origins, v.message, v.rollback_of, v.merged_from, v.withheld_at, v.strip_opts, u.display_name, u.deleted_at FROM circle_versions v LEFT JOIN users u ON u.id = v.committer_id
          WHERE ${version === null ? "v.id = ?1 AND v.bank_id = ?2 AND v.status = 'current'" : "v.bank_id = ?2 AND v.version = ?3 AND v.status IN ('current', 'previous') AND v.full_key <> ''"}`,
      )
      .bind(...(version === null ? [b.head_version_id, bid] : [null, bid, version]))
      .first<{ id: string; version: number; full_key: string; committed_at: number; license: string; origins: string | null; message: string | null; rollback_of: number | null; merged_from: number | null; withheld_at: number | null; strip_opts: string | null; display_name: string | null; deleted_at: number | null }>();
    if (!v && version !== null) {
      // (round 48, critic 55a V4: a withheld version pruned is not "past the last 20": its words say what it is)
      const gone = await c.db.prepare(`SELECT withheld_at FROM circle_versions WHERE bank_id = ?1 AND version = ?2`).bind(bid, version).first<{ withheld_at: number | null }>();
      if (gone && gone.withheld_at !== null)
        throw new PoolError(404, "version_not_retained", {
          ...(await keptInfo(c, bid, b.head_version)),
          withheld: true,
          review_copy: "hashes_only",
          message: "This version was withheld for an administrator's review and its text is no longer kept: only a record of what matched is kept, for the administrator.",
        });
      throw await notRetained(c, bid, b.head_version);
    }
    if (!v) throw notFound();
    // Round 38 (critics 45a/45b B1): a version that matched confirmed student data is served to no member (its
    // author included) until an administrator releases it, whichever version is the head now.
    // Round 49 (critic 59 B1): the circle's owner and the bank's author while an editor (who may change the answer-key
    // setting) are told whether turning hiding off would release this version; nobody else is.
    if (v.withheld_at !== null) throw withheldError(m.role === "owner" || (b.author_id === me && m.role === "editor") ? { hide_off_releases: b.hide_key === 1 && (await hideOffReleases(c, me, bid, v.id)) } : {});
    // Round 43 (D12): a VIEWER of a bank that hides its keys is served share-core's stripped form of this version
    // (circlehide.ts); the owner and editors the full text. Editors' version messages are not shown with it.
    const hidden = b.hide_key === 1 && m.role === "viewer";
    // Round 46 (critic 53b B2): viewers read only a version whose served form was checked, i.e. one with recorded strip
    // decisions (a send to the bank while it hid its keys, the PATCH that turned it on, a rollback copying them);
    // any other is refused before anything is stripped (a HEAD and a 304 too).
    if (hidden && v.strip_opts === null) throw keysHiddenUnavailable();
    const head = {
      circle: { id: m.id, name: m.name, role: m.role },
      bank: {
        id: b.id,
        title: b.title,
        subject: b.subject,
        course: b.course,
        grade: b.grade,
        license: b.license,
        version_license: v.license,
        author_name: b.anonymized === 1 ? DELETED_NAME : await authorName(c, b.author_id),
        is_author: b.author_id === me,
        version: v.version,
        version_id: v.id,
        // (round 37: the head, so a read of an older version says how far behind it is)
        head_version: b.head_version,
        question_count: b.question_count,
        has_images: b.has_images === 1,
        committed_at: v.committed_at,
        committed_by: nameOf(v.display_name, v.deleted_at),
        message: hidden ? "" : text(v.message),
        hide_key: b.hide_key === 1,
        ...(hidden ? { keys_hidden: true } : {}),
        // (round 37: present only when they apply; no circle response carries a null outside its listed fields)
        ...(v.rollback_of !== null ? { rollback_of: v.rollback_of } : {}),
        ...(v.merged_from !== null ? { merged_from: v.merged_from } : {}),
        ...(b.held === 1 ? { held: true } : {}),
      },
      origins: await originsForRead(c, v.origins),
    };
    const h = JSON.stringify(head);
    release.grow(READ_HEAD_COST * h.length);
    // (round 46, critic 53a variant 3: a viewer's ETag also covers the decisions the version is stripped with, so
    // re-recorded decisions are a new body to revalidating viewers)
    const etag = `"${(await sha256Hex(new TextEncoder().encode(hidden ? `${h}\n${v.strip_opts}` : h))).slice(0, 32)}"`;
    if (headOnly || (req.headers.get("If-None-Match") ?? "").split(",").some((t) => t.trim().replace(/^W\//, "") === etag || t.trim() === "*")) {
      const size = (await c.r2.head(v.full_key))?.size;
      if (size === undefined) throw notFound();
      const hs = rawJson(200, "", { ETag: etag }).headers;
      // (round 43: a stripped body's length is not known without stripping it: a viewer's HEAD has none)
      if (headOnly && !hidden) hs.set("Content-Length", String(new TextEncoder().encode(`${h.slice(0, -1)},"payload":`).length + size + 1));
      release();
      return new Response(null, { status: headOnly ? 200 : 304, headers: hs });
    }
    if (hidden) {
      const out = await strippedForViewer(c, me, v.full_key, v.strip_opts, (n) => release.grow(n));
      return streamedPayload(h, bytesBody(out), release, { ETag: etag }, c.deps.readIdleMs, c.deps.readLifetimeMs);
    }
    const obj = await c.r2.get(v.full_key);
    if (!obj) throw notFound();
    return streamedPayload(h, obj as R2ObjectBody, release, { ETag: etag }, c.deps.readIdleMs, c.deps.readLifetimeMs);
  } catch (e) {
    release();
    throw e;
  }
}
/**
 * Which versions of a bank are kept in full (D16), as members need it (round 38, critic 45a variant 1):
 * `oldest_kept`, where the contiguous run of kept versions that ends at the head starts (every version from it
 * to the head is kept; a line's `withheld` says which of them no member may read), and `pinned_versions`, older
 * versions kept outside that run (a report or a hold keeps them; at most 50 listed). Reads the kept versions only,
 * by the (bank_id, status) index (the (bank_id, version) one would walk every pruned version first).
 */
export async function keptInfo(c: Ctx, bid: string, head: number): Promise<{ oldest_kept: number | null; pinned_versions: number[] }> {
  // Round 39 (critic 46a variant 8): READABLE versions only (kept in full and not withheld): a withheld version breaks
  // the run, and the run starts from the newest readable version (a withheld head is not "kept" for members).
  const kept = new Set(
    (
      await c.db
        .prepare(`SELECT version FROM circle_versions INDEXED BY circle_versions_bank WHERE bank_id = ?1 AND status IN ('current', 'previous') AND full_key <> '' AND withheld_at IS NULL`)
        .bind(bid)
        .all<{ version: number }>()
    ).results.map((r) => r.version),
  );
  if (kept.size === 0) return { oldest_kept: null, pinned_versions: [] };
  let start = Math.min(head, Math.max(...kept));
  while (!kept.has(start)) start--;
  while (kept.has(start - 1)) start--;
  return { oldest_kept: start, pinned_versions: [...kept].filter((v) => v < start).sort((a, b) => a - b).slice(0, 50) };
}
/** `404 version_not_retained {oldest_kept, pinned_versions}`. */
export async function notRetained(c: Ctx, bid: string, head: number): Promise<PoolError> {
  return new PoolError(404, "version_not_retained", { ...(await keptInfo(c, bid, head)), message: "This version is no longer kept (a bank keeps its last 20 versions in full)." });
}
/** Round 38: `404 version_withheld` (a version that matched confirmed student data, round 44: or a copyright re-upload
 *  block; an administrator may release it). The words name no reason, so members cannot tell one from the other.
 *  Round 49 (critic 59 B1): `extra` is `{hide_off_releases}` on a bank read by a member who may change the answer-key
 *  setting (see hideOffReleases). */
export const withheldError = (extra: Record<string, unknown> = {}) =>
  new PoolError(404, "version_withheld", { ...extra, message: "This version is withheld until an administrator has reviewed it. Only an administrator can release it." });

/**
 * Round 37: a version's recorded origins (public material, checked at send) with their authors' names read
 * NOW (a name is never stored with a circle version: an erased public author shows as "deleted teacher").
 * Bounded by the version's questions; each item by its key.
 */
export async function originsForRead(c: Ctx, json: string | null): Promise<{ pos: number; item_id: string; qid: string | null; version: number | null; license: string; author_name: string }[]> {
  if (!json) return [];
  let rows: { pos: number; item_id: string; qid?: string | null; version?: number | null; license: string }[];
  try {
    rows = JSON.parse(json) as typeof rows;
  } catch {
    return [];
  }
  if (!Array.isArray(rows) || rows.length === 0) return [];
  const names = new Map(
    (
      await c.db
        .prepare(`SELECT i.id, i.anonymized, u.display_name, u.deleted_at FROM json_each(?1) j CROSS JOIN items i ON i.id = j.value LEFT JOIN users u ON u.id = i.author_id`)
        .bind(JSON.stringify([...new Set(rows.map((r) => r.item_id))]))
        .all<{ id: string; anonymized: number; display_name: string | null; deleted_at: number | null }>()
    ).results.map((r) => [r.id, r.anonymized === 1 ? DELETED_NAME : nameOf(r.display_name, r.deleted_at)]),
  );
  return rows.map((r) => ({ pos: r.pos, item_id: r.item_id, qid: r.qid ?? null, version: r.version ?? null, license: r.license, author_name: names.get(r.item_id) ?? DELETED_NAME }));
}

async function authorName(c: Ctx, id: string): Promise<string> {
  const u = await c.db.prepare(`SELECT display_name, deleted_at FROM users WHERE id = ?1`).bind(id).first<{ display_name: string | null; deleted_at: number | null }>();
  return nameOf(u?.display_name, u ? u.deleted_at : 1);
}

const EXT: Record<string, string> = { "image/png": "png", "image/jpeg": "jpg", "image/gif": "gif", "image/webp": "webp" };

/** GET /circles/:cid/banks/:bid/blobs/:sha256: an image a KEPT version of the bank uses, to a member who may read the bank. */
export async function getBankBlob(c: Ctx, p: PoolPrincipal, cid: string, bid: string, sha: string): Promise<Response> {
  const me = p.userId;
  await missGuard(c, me, "blob_miss");
  const missBlob = async (): Promise<never> => {
    await countMiss(c, me, "blob_miss");
    throw notFound();
  };
  if (!SHA_RE.test(sha)) return missBlob();
  const m = ID_RE.test(cid) ? await memberOf(c, cid, me) : null;
  if (!m || m.closed_at !== null) return missBlob();
  const row = await c.db
    .prepare(
      `SELECT bl.content_type FROM circle_banks b
         JOIN circle_versions v ON v.bank_id = b.id AND v.status IN ('current', 'previous') AND v.withheld_at IS NULL
         JOIN circle_version_blobs vb ON vb.version_id = v.id AND vb.sha256 = ?3
         JOIN blobs bl ON bl.sha256 = vb.sha256
        WHERE b.id = ?1 AND b.circle_id = ?2 AND b.purged_at IS NULL AND b.removed_at IS NULL
          AND (b.author_id = ?4 OR NOT EXISTS (SELECT 1 FROM circle_holds h WHERE h.bank_id = b.id) OR (NOT ${BLOCKING_HOLD_SQL("b.id")} AND ${ACTIVE_SQL("b.circle_id", "?4", ["owner"])})) AND ${AUTHOR_OK_SQL("b")}
          -- an image confirmed as student data is served to nobody, from anywhere (as the public pool)
          AND NOT EXISTS (SELECT 1 FROM privacy_blocks pb WHERE pb.kind = 'blob' AND pb.hash = ?3) AND bl.privacy_purged_at IS NULL
          -- (round 43, D12: a viewer of a bank that hides its keys gets only an image its stripped form keeps)
          AND (?5 = 0 OR b.hide_key = 0 OR ?3 IN (SELECT value FROM json_each(COALESCE(json_extract(v.strip_opts, '$.image_shas'), '[]'))))
        LIMIT 1`,
    )
    .bind(bid, cid, sha, me, m.role === "viewer" ? 1 : 0)
    .first<{ content_type: string }>();
  if (!row) return missBlob();
  const o = await c.r2.get(`blobs/${sha}`);
  if (!o) return missBlob();
  return new Response((o as R2ObjectBody).body, {
    status: 200,
    headers: {
      "Content-Type": row.content_type,
      "X-Content-Type-Options": "nosniff",
      "Content-Security-Policy": "sandbox; default-src 'none'",
      "Content-Disposition": `attachment; filename="${sha}.${EXT[row.content_type] ?? "bin"}"`,
      "Cache-Control": "no-store",
      "Cross-Origin-Resource-Policy": "same-site",
      Vary: "Authorization, Cookie, Origin",
    },
  });
}

// ---- bank settings, delete, undelete, report ------------------------------------------------------------------

/**
 * PATCH /circles/:cid/banks/:bid {license} (the bank's author): the re-share setting (D19). Earlier versions keep their
 * own licence. Round 43 (D12): or {hide_key, labels?, order_acks?, ack_formatting?, image_ok?} (patchHideKey), one
 * setting per request.
 */
export async function patchBank(c: Ctx, req: Request, p: PoolPrincipal, cid: string, bid: string): Promise<Response> {
  const me = p.userId;
  const { m, b } = await bankOrMiss(c, cid, bid, me);
  // Round 46 (critic 53b nitpick): who may change nothing here (neither the circle's owner nor the bank's author) is
  // refused before the body is read (up to a megabyte of decisions).
  // (round 48, critic 55a: one hint for the route, as trim and rollback give: the owner, or the bank's author)
  if (m.role !== "owner" && b.author_id !== me) throw new PoolError(403, "circle_role_required", { needed: OWNER_OR_AUTHOR, role: m.role });
  // (round 43: the strip decisions of a 1,000-question bank can pass the 64 KB of a plain JSON body: a send's meta size)
  const body = await readJson(req, `u:${me}`, { deadlineMs: c.deps.bodyDeadlineMs, max: PATCH_BODY_MAX });
  for (const k of Object.keys(body)) if (k !== "license" && k !== "hide_key" && k !== "base_version" && !(DECISION_FIELDS as readonly string[]).includes(k)) throw bad(k);
  if (body.hide_key !== undefined) {
    if (body.license !== undefined) throw bad("license", { message: "Change the licence and the answer-key setting one at a time." });
    if (typeof body.hide_key !== "boolean") throw bad("hide_key");
    if (body.hide_key === false) for (const k of DECISION_FIELDS) if (body[k] !== undefined) throw bad(k, { message: "Strip decisions go with hide_key: true." });
    // Round 52 (critic 62 B1, critic 57): the version the decisions were made on (they are by position).
    if (body.base_version !== undefined && !isVersionNo(body.base_version)) throw bad("base_version");
    return patchHideKey(c, m, b, me, cid, bid, body.hide_key, readStripDecisions(body), (body.base_version as number | undefined) ?? null);
  }
  if (body.base_version !== undefined) throw bad("base_version", { message: "base_version goes with hide_key." });
  for (const k of DECISION_FIELDS) if (body[k] !== undefined) throw bad(k, { message: "Strip decisions go with hide_key: true." });
  if (body.license === undefined) throw bad("license", { allowed: LICENSES });
  requireOpen(m);
  if (b.author_id !== me) throw new PoolError(403, "circle_role_required", { needed: "author", role: m.role });
  // Round 38 (critic 45a variant 3): the author while an editor (or the owner): a demoted author no longer changes it.
  if (m.role !== "owner" && m.role !== "editor") throw new PoolError(403, "circle_role_required", { needed: "editor", role: m.role });
  if (b.removed_at !== null) throw new PoolError(409, "bank_removed", { reason: b.removed_reason });
  const lic = body.license;
  if (!isLicense(lic)) throw bad("license", { allowed: LICENSES });
  // Public material inside the bank (declared origins, checked at send): the licence must be one every origin allows.
  const head = await c.db.prepare(`SELECT origins FROM circle_versions WHERE id = ?1`).bind(b.head_version_id).first<{ origins: string | null }>();
  const origins = head?.origins ? (JSON.parse(head.origins) as { license: string }[]) : [];
  const allowed = allowedByAll(origins.map((o) => o.license));
  if (!allowed.includes(lic)) throw new PoolError(422, "license_incompatible", { allowed, message: "Some questions in this bank came from the public pool under a licence that does not allow this one." });
  const now = c.now();
  const [upd] = await c.db.batch([
    c.db
      // (round 39, critic 46b V7: a compare-and-set on the head whose origins were checked above)
      .prepare(`UPDATE circle_banks SET license = ?4, updated_at = ?5 WHERE id = ?1 AND circle_id = ?2 AND author_id = ?3 AND head_version_id IS ?6 AND removed_at IS NULL AND ${OPEN_SQL("?2")} AND ${ACTIVE_SQL("?2", "?3", ["owner", "editor"])} AND ${ALIVE_SQL("?3")}`)
      .bind(bid, cid, me, lic, now, b.head_version_id),
    audit(c.db, { actorId: me, actorKind: "user", action: "circle.bank_license", itemId: cid, targetUserId: me, detail: { bank_id: bid, license: lic } }, now, `changes() = 1`),
  ]);
  if ((upd!.meta.changes ?? 0) !== 1) await explain(c, cid, me);
  return json(200, { ok: true, license: lic });
}

/** A PATCH body's size at most (round 43: strip decisions; the size of a send's meta). */
const PATCH_BODY_MAX = 1024 * 1024;
/** Round 52: a version number as a send's `base_version` is one. */
const isVersionNo = (v: unknown): v is number => typeof v === "number" && Number.isInteger(v) && v >= 1 && v <= 2 ** 31;
/** Round 46: older versions with no decisions recorded that a PATCH turning hidden keys on checks too, at most (by their stored bytes, and in number). */
export const HIDE_OLDER_BYTES = 4 * 1024 * 1024;
export const HIDE_OLDER_MAX = 19;

/** Round 46 (critic 53b V4): the day's allowance of answer-key setting changes, with plain words past it. */
async function chargeHideChange(c: Ctx, me: string): Promise<void> {
  try {
    await requireDaily(c, me, "circle_hide_changes");
  } catch (e) {
    if (e instanceof PoolError && e.code === "daily_limit") throw new PoolError(429, "daily_limit", { ...e.extra, message: "You have changed answer-key settings as many times as you may today. Try again tomorrow." }, e.headers);
    throw e;
  }
}

/**
 * Round 43 (D12): PATCH .../banks/:bid {hide_key} by the bank's author while an editor (or the owner), or by the
 * circle's owner. Off: viewers read the full text again. On (or on again with other decisions): the head is stripped
 * with the decisions given (share-core's refusals, as a hidden-key publish's: 422 invalid_payload |
 * labels_need_decision | order_ack_required with the findings; the caller reads the keys anyway), which are recorded
 * for the head, and the served form's key-free hashes join the circle's re-share records under the first sender of
 * each full text (D13). A withheld head is not stripped (nobody reads it). One compare-and-set on the head: a version
 * sent meanwhile is `409 conflict` (do it again).
 * Round 46: the same setting again (and, for on, the decisions the head already recorded) is `200 {unchanged: true}`
 * with no audit line and no charge (critic 53b V4); every change counts toward circle_hide_changes (20 a day). The
 * form viewers will be served is checked against confirmed student data and copyright re-upload blocks (critic 53b
 * B2): a match withholds the head and holds the bank exactly as a send's match does (the answer says `held` and
 * `withheld`; the admin's flag and mail). Older kept versions without recorded decisions are stripped with none and
 * checked too, newest first, within HIDE_OLDER_BYTES and HIDE_OLDER_MAX: one that passes records its (empty) decisions
 * and is readable by viewers, one that matches is withheld; the rest (refused without decisions, or past the bound)
 * stay unrecorded, and viewers are refused them (409 keys_hidden_unavailable).
 * Round 47: only a PATCH that changes something counts (critic 54a B2): the day's count is taken before the work, so
 * the cap holds under concurrent PATCHes, and given back for every other answer (a refusal for decisions or an order
 * acknowledgement, a conflict, any error). Turning hiding off restores what turning it on withheld (critic 54a variant
 * 1): a version withheld only by an answer-key PATCH (every open flag on it says `via: hide_key`) whose FULL form is
 * clean is readable again, its flags resolved, and the bank's hold lifted when it is the head (`restored`: their
 * versions); checked newest first within the same bounds, and again inside the batch. The flags a PATCH raises count
 * toward circle_send_flags and keep the evidence bound, as a send's (critic 54b N1).
 * Round 50 (critic 60): only a copyright match an answer-key PATCH raised is released that way (never a student-data
 * match, never a student-data hold), and an `on` answers `held` / `withheld` only for a head THIS PATCH withheld.
 * Round 52 (critic 62 B1, critic 57): `base` (the body's `base_version`, the version the decisions were made on): a
 * head that is not it is `409 conflict {head_version}`, as a lost batch is, before any decision is checked or
 * applied and before anything is charged; hiding on's batch compares and sets the head checked here, so a version that
 * arrives after the check is the same conflict (hiding off strips nothing: its pin is the check). Hiding on again with other decisions (the app's "Change picture
 * choices") is a change like any other: re-stripped, recorded, counted.
 */
async function patchHideKey(c: Ctx, m: Membership, b: BankRow, me: string, cid: string, bid: string, on: boolean, d: StripDecisions, base: number | null = null): Promise<Response> {
  requireOpen(m);
  if (!(m.role === "owner" || (b.author_id === me && m.role === "editor"))) throw new PoolError(403, "circle_role_required", { needed: OWNER_OR_AUTHOR, role: m.role });
  await requireReadable(c, m, b, me);
  if (base !== null && base !== b.head_version) throw new PoolError(409, "conflict", { head_version: b.head_version, message: "A new version arrived meanwhile; do it again for the new version." });
  if (!on && b.hide_key === 0) return json(200, { ok: true, hide_key: false, unchanged: true });
  // Round 48 (critic 55b B1): a decision for a question the head does not have is refused before any work.
  const outside = (field: string, xs: number[]) => {
    if (xs.some((x) => x >= b.question_count)) throw bad(field, { message: `This bank's current version has ${b.question_count} questions.` });
  };
  outside("labels", d.labels.map((x) => x.question));
  outside("order_acks", d.orderAcks.map((x) => x.question));
  outside("image_ok", d.imageOk);
  outside("ack_formatting", d.ackFormatting);
  const head = on && b.head_version_id
    ? await c.db
        .prepare(`SELECT id, version, full_key, withheld_at, strip_opts FROM circle_versions WHERE id = ?1 AND status = 'current'`)
        .bind(b.head_version_id)
        .first<{ id: string; version: number; full_key: string; withheld_at: number | null; strip_opts: string | null }>()
    : null;
  const headReadable = !!head && head.withheld_at === null && head.full_key !== "";
  if (on && b.hide_key === 1 && (!headReadable || (head!.strip_opts !== null && sameDecisions(decisionsOf(head!.strip_opts), d)))) return json(200, { ok: true, hide_key: true, unchanged: true });
  // Round 48 (critic 55b B1): every PATCH that does work (a strip of the head, checks of older versions) counts toward
  // the day's attempts, refused ones included, and is never given back (a 503 busy aside: no work was done); the
  // day's changes below count only what changed something.
  try {
    await requireDaily(c, me, "circle_hide_attempts");
  } catch (e) {
    if (e instanceof PoolError && e.code === "daily_limit")
      throw new PoolError(429, "daily_limit", { ...e.extra, message: "You have tried to change answer-key settings as many times as you may today. Try again tomorrow." }, e.headers);
    throw e;
  }
  try {
    await chargeHideChange(c, me);
  } catch (e) {
    await giveDaily(c, me, "circle_hide_attempts").catch(() => {});
    throw e;
  }
  try {
    const r = on ? await hideOn(c, b, me, cid, bid, d, head, headReadable) : await hideOff(c, me, cid, bid);
    if (r.unchanged) await giveDaily(c, me, "circle_hide_changes").catch(() => {});
    return json(200, r);
  } catch (e) {
    await giveDaily(c, me, "circle_hide_changes").catch(() => {});
    if (e instanceof PoolError && e.status === 503) await giveDaily(c, me, "circle_hide_attempts").catch(() => {});
    throw e;
  }
}

/** Round 48 (critic 55a): the role hint of the routes the circle's owner or the bank's author (while an editor) may use. */
export const OWNER_OR_AUTHOR = "owner_or_author";

/** SQL: no open send flag on version `v` but the copyright matches an answer-key PATCH raised (round 47; round 50,
 *  critic 60 B2: a student-data flag never, even one an answer-key PATCH raised: turning hiding off does not change
 *  what the full form holds, so it never releases one). */
const ONLY_HIDE_FLAGS = (v: string) =>
  `NOT EXISTS (SELECT 1 FROM admin_flags xf WHERE xf.publish_id = ${v} AND xf.kind IN ${SEND_FLAG_KINDS} AND xf.resolved_at IS NULL AND (xf.kind IS NOT 'blocklist' OR (xf.detail ->> '$.via') IS NOT 'hide_key'))`;

type HideOffCandidate = { id: string; version: number; full_key: string; bytes: number; committer_id: string };

/**
 * Round 47/49: the withheld versions turning hiding off checks for release: withheld only by an answer-key PATCH's
 * copyright match (round 50: at least one open `blocklist` flag says `via: hide_key`, and no other open send flag is
 * on it), kept in full, newest first, within HIDE_OLDER_MAX + 1 and HIDE_OLDER_BYTES. Each is released when its full
 * form is clean (fullFormClean). Round 50 (critic 60 B2): a head is never one while the bank has a student-data hold.
 */
async function hideOffCandidates(c: Ctx, bid: string): Promise<HideOffCandidate[]> {
  const cands = (
    await c.db
      .prepare(
        `SELECT v.id, v.version, v.full_key, v.bytes, v.committer_id FROM circle_versions v INDEXED BY circle_versions_bank
          WHERE v.bank_id = ?1 AND v.status IN ('current', 'previous') AND v.full_key <> '' AND v.withheld_at IS NOT NULL
            AND EXISTS (SELECT 1 FROM admin_flags hf WHERE hf.publish_id = v.id AND hf.kind = 'blocklist' AND hf.resolved_at IS NULL AND hf.detail ->> '$.via' = 'hide_key')
            AND ${ONLY_HIDE_FLAGS("v.id")}
            AND NOT (v.status = 'current' AND EXISTS (SELECT 1 FROM circle_holds ph WHERE ph.bank_id = ?1 AND ph.source IN ('privacy', 'privacy_near')))
          ORDER BY v.version DESC LIMIT ?2`,
      )
      .bind(bid, HIDE_OLDER_MAX + 1)
      .all<HideOffCandidate>()
  ).results;
  const out: typeof cands = [];
  let budget = HIDE_OLDER_BYTES;
  for (const v of cands) {
    if (v.bytes > budget) break;
    budget -= v.bytes;
    out.push(v);
  }
  return out;
}

/** Round 47/50: candidate `v`'s full form checked as hide-off checks it (the version's own sender's exemptions, as at
 *  its send: round 48, critic 55b V7): its scan when nothing matches, else null. Round 51: `now` as withStoredFull's. */
async function fullFormClean(c: Ctx, v: HideOffCandidate, now = false): Promise<Scan | null> {
  const sc = await withStoredFull(c, v.full_key, (qs, mh) => privacyScan(c, [{ qs, mh, imgAt: imagesAt(qs) }], v.committer_id), { now });
  return sc && sc.exact.length + sc.near.length + sc.copyright.length === 0 ? sc : null;
}

/**
 * Round 49 (critic 59 B1): would turning hiding off release version `vid`? It is one of hideOffCandidates (so a send's
 * flag on it, a student-data flag, a cap past the day's flags, or a version past the bound says false) and, round 50
 * (critic 60 B2), its full form is clean now, as hide-off checks it (a block on the full text too says false). A check
 * that can not run now says false: nothing is promised. The caller checks that the bank hides its keys.
 * Round 51 (critic 61 I2): the check is publish-sized work on a read, so it runs only when the isolate's heavy slot is
 * free at once (the read never waits or queues for it, so it can not hold a read up or turn a publish away: busy says
 * false), and each check that runs is one of the reader's `circle_hide_checks` today (past it: false, no check; one
 * that could not run is given back). Hide-off itself checks again, on its own budget.
 */
async function hideOffReleases(c: Ctx, me: string, bid: string, vid: string): Promise<boolean> {
  const v = (await hideOffCandidates(c, bid)).find((x) => x.id === vid);
  if (!v) return false;
  if (!(await takeDaily(c, me, "circle_hide_checks"))) return false;
  try {
    return (await fullFormClean(c, v, true)) !== null;
  } catch {
    await giveDaily(c, me, "circle_hide_checks").catch(() => {});
    return false;
  }
}

/** Round 43/47: PATCH {hide_key: false} after the checks and the charge (see patchHideKey). */
async function hideOff(c: Ctx, me: string, cid: string, bid: string): Promise<Record<string, unknown>> {
  const now = c.now();
  const rights = `(${ACTIVE_SQL("?2", "?3", ["owner"])} OR (circle_banks.author_id = ?3 AND ${ACTIVE_SQL("?2", "?3", ["editor"])}))`;
  const clean: { id: string; keys: string; shas: string; by: string }[] = [];
  for (const v of await hideOffCandidates(c, bid)) {
    const sc = await fullFormClean(c, v);
    if (sc) clean.push({ id: v.id, keys: JSON.stringify(sc.keys), shas: JSON.stringify(sc.shas), by: v.committer_id });
  }
  const aid = newId();
  const won = `EXISTS (SELECT 1 FROM pool_audit wa WHERE wa.id = '${aid}')`;
  const stmts: D1PreparedStatement[] = [
    c.db
      .prepare(`UPDATE circle_banks SET hide_key = 0, updated_at = ?4 WHERE id = ?1 AND circle_id = ?2 AND hide_key = 1 AND removed_at IS NULL AND purged_at IS NULL AND ${OPEN_SQL("?2")} AND ${rights} AND ${ALIVE_SQL("?3")}`)
      .bind(bid, cid, me, now),
    c.db
      .prepare(`INSERT INTO pool_audit (id, at, actor_id, actor_kind, action, item_id, target_user_id, detail) SELECT ?1, ?2, ?3, 'user', 'circle.bank_hide_key', ?4, ?3, ?5 WHERE changes() = 1`)
      .bind(aid, now, me, cid, JSON.stringify({ bank_id: bid, hide_key: false })),
  ];
  for (const v of clean)
    stmts.push(
      // (its full form is what everyone reads now: checked again here against the blocks as they are, no gap; its
      // recorded decisions go, so turning hiding on again checks its viewers' form afresh)
      c.db
        .prepare(
          `UPDATE circle_versions SET withheld_at = NULL, strip_opts = NULL WHERE id = ?1 AND withheld_at IS NOT NULL AND ${won} AND ${ONLY_HIDE_FLAGS("?1")}
             AND NOT (status = 'current' AND EXISTS (SELECT 1 FROM circle_holds ph WHERE ph.bank_id = ?6 AND ph.source IN ('privacy', 'privacy_near')))
             AND NOT ${PRIVACY_SQL("?2", "?3")} AND NOT ${COPYRIGHT_SQL("?2", "?3", "?4", "?5")}`,
        )
        .bind(v.id, v.keys, v.shas, now, v.by, bid),
      c.db
        .prepare(
          `UPDATE admin_flags SET resolved_at = ?2, resolved_by = ?3, detail = json_set(detail, '$.resolved_via', 'hide_key_off')
            WHERE publish_id = ?1 AND kind IN ${SEND_FLAG_KINDS} AND resolved_at IS NULL AND json_valid(detail) AND ${won} AND EXISTS (SELECT 1 FROM circle_versions rv WHERE rv.id = ?1 AND rv.withheld_at IS NULL)`,
        )
        .bind(v.id, now, me),
      c.db
        .prepare(
          // (round 50, critic 60 B2: the copyright hold only; a student-data hold is never lifted here)
          `DELETE FROM circle_holds WHERE bank_id = ?1 AND source = 'blocklist' AND ${won}
             AND EXISTS (SELECT 1 FROM circle_banks hb JOIN circle_versions hv ON hv.id = hb.head_version_id WHERE hb.id = ?1 AND hv.id = ?2 AND hv.withheld_at IS NULL)`,
        )
        .bind(bid, v.id),
    );
  if (clean.length > 0) stmts.push(c.db.prepare(LISTING_REFRESH_SQL(won)).bind(bid));
  stmts.push(
    c.db
      .prepare(`SELECT ${won} AS won, (SELECT json_group_array(rv.version) FROM circle_versions rv WHERE rv.id IN (SELECT value FROM json_each(?1)) AND rv.withheld_at IS NULL) AS restored`)
      .bind(JSON.stringify(clean.map((v) => v.id))),
  );
  const res = await c.db.batch(stmts);
  const out = (res[res.length - 1]!.results as { won: number; restored: string | null }[])[0];
  if (!out || out.won !== 1) {
    await throwIfErased(c, me);
    const now1 = await c.db.prepare(`SELECT hide_key FROM circle_banks WHERE id = ?1`).bind(bid).first<{ hide_key: number }>();
    if (now1?.hide_key === 1) await explain(c, cid, me);
    return { ok: true, hide_key: false, unchanged: true };
  }
  const restored = (JSON.parse(out.restored ?? "[]") as number[]).sort((a, b2) => a - b2);
  return { ok: true, hide_key: false, ...(restored.length > 0 ? { restored } : {}) };
}

/** Round 43/46/47: PATCH {hide_key: true} after the checks and the charge (see patchHideKey). */
async function hideOn(
  c: Ctx,
  b: BankRow,
  me: string,
  cid: string,
  bid: string,
  d: StripDecisions,
  head: { id: string; version: number; full_key: string; withheld_at: number | null; strip_opts: string | null } | null,
  headReadable: boolean,
): Promise<Record<string, unknown>> {
  const now = c.now();
  const rights = `(${ACTIVE_SQL("?2", "?3", ["owner"])} OR (circle_banks.author_id = ?3 AND ${ACTIVE_SQL("?2", "?3", ["editor"])}))`;
  let stripOpts: string | null = null;
  let pairs: [string, string][] = [];
  let report: Record<string, unknown> = {};
  let headScan: Scan | null = null;
  const scanOf = async (st: Stripped): Promise<Scan> => {
    const vq = questionsOf(st.payload);
    return privacyScan(c, [{ qs: vq, mh: st.matchHashes, imgAt: imagesAt(vq) }], me);
  };
  if (headReadable) {
    const r = await stripStored(c, head!.full_key, d, { withFullHashes: true, onStripped: async (st) => void (headScan = await scanOf(st)) });
    if (!r.ok) {
      if (r.missing) throw notFound();
      throw stripRefusal(r.fail);
    }
    stripOpts = decisionsJson(r.recorded, r.imageShas);
    pairs = hashPairs(r.fullHashes!, r.matchHashes);
    report = r.report;
  }
  // Older kept versions with no decisions recorded (sent while the keys were shown), newest first, within the bound.
  const older = (
    await c.db
      .prepare(
        `SELECT id, version, full_key, bytes FROM circle_versions INDEXED BY circle_versions_bank
          WHERE bank_id = ?1 AND status IN ('current', 'previous') AND full_key <> '' AND withheld_at IS NULL AND strip_opts IS NULL AND id IS NOT ?2 ORDER BY version DESC LIMIT ?3`,
      )
      .bind(bid, head?.id ?? null, HIDE_OLDER_MAX)
      .all<{ id: string; version: number; full_key: string; bytes: number }>()
  ).results;
  const toRecord: { id: string; json: string }[] = [];
  const toWithhold: { id: string; version: number; scan: Scan }[] = [];
  let budget = HIDE_OLDER_BYTES;
  for (const v of older) {
    if (v.bytes > budget) break;
    budget -= v.bytes;
    let sc: Scan | null = null;
    const r = await stripStored(c, v.full_key, NO_DECISIONS, { onStripped: async (st) => void (sc = await scanOf(st)) });
    if (!r.ok || !sc) continue;
    const s2 = sc as Scan;
    if (s2.exact.length + s2.near.length + s2.copyright.length > 0) toWithhold.push({ id: v.id, version: v.version, scan: s2 });
    else toRecord.push({ id: v.id, json: decisionsJson(r.recorded, r.imageShas) });
  }
  const records = await limitOf(c, "*", "circle_records");
  const fresh = pairs.length
    ? ((await c.db
        .prepare(`SELECT COUNT(*) AS n FROM json_each(?1) j WHERE NOT EXISTS (SELECT 1 FROM circle_qkeys k WHERE k.circle_id = ?2 AND k.match_hash = j.value AND k.bank_id = ?3)`)
        .bind(JSON.stringify([...new Set(pairs.map((x) => x[1]))]), cid, bid)
        .first<{ n: number }>())?.n ?? pairs.length)
    : 0;
  // (round 47, critic 54b N1: the flags this PATCH raises count toward the account's circle_send_flags in this circle)
  const day0 = dayOf(now) * DAY_MS;
  const flagCap = await limitOf(c, me, "circle_send_flags");
  const room = FLAG_ROOM_SQL("?4", "?7", "?8", "?9");
  const aid = newId();
  const won = `EXISTS (SELECT 1 FROM pool_audit wa WHERE wa.id = '${aid}')`;
  const few = <T,>(xs: T[]) => ({ count: xs.length, first: xs.slice(0, FLAG_DETAIL_MAX) });
  const stmts: D1PreparedStatement[] = [
    c.db
      .prepare(
        // (round 53: and the setting and the head's recorded decisions as this PATCH read them, so of two PATCHes that
        // read the same state only the first batch changes it: the second is `unchanged` or a conflict, below)
        `UPDATE circle_banks SET hide_key = 1, updated_at = ?4 WHERE id = ?1 AND circle_id = ?2 AND head_version_id IS ?5 AND removed_at IS NULL AND purged_at IS NULL
           AND hide_key = ?8 AND (SELECT sv.strip_opts FROM circle_versions sv WHERE sv.id = ?5) IS ?9
           AND ${OPEN_SQL("?2")} AND ${rights} AND ${ALIVE_SQL("?3")} AND (SELECT qkeys FROM circles WHERE id = ?2) + ?6 <= ?7`,
      )
      .bind(bid, cid, me, now, b.head_version_id, fresh, records, b.hide_key, head?.strip_opts ?? null),
    // (an activity line when it is turned on; recording new decisions for a bank that already hides its keys is not
    // one; its id marks this PATCH as the one that won for every later statement of the batch)
    c.db
      .prepare(`INSERT INTO pool_audit (id, at, actor_id, actor_kind, action, item_id, target_user_id, detail) SELECT ?1, ?2, ?3, 'user', ?4, ?5, ?3, ?6 WHERE changes() = 1`)
      .bind(aid, now, me, b.hide_key === 1 ? "circle.bank_strip_decisions" : "circle.bank_hide_key", cid, JSON.stringify({ bank_id: bid, hide_key: true })),
    c.db.prepare(`UPDATE circle_versions SET strip_opts = ?2 WHERE id = ?1 AND ?2 IS NOT NULL AND ${won}`).bind(head?.id ?? "", stripOpts),
    c.db.prepare(STRIPPED_QKEYS_SQL("?1", "?2", "?3", won)).bind(cid, bid, JSON.stringify(pairs)),
    c.db.prepare(`UPDATE circles SET qkeys = qkeys + changes() WHERE id = ?1 AND ${won}`).bind(cid),
  ];
  const hs = headScan as Scan | null;
  // (round 51, critic 61 M1: the statement that withholds the head, so the answer says whether THIS batch did it)
  let withholdAt = -1;
  if (hs && head) {
    // The viewers' form of the head, as a send's commit checks it (the exact keys again here: no gap).
    const kj = JSON.stringify(hs.keys);
    const sj = JSON.stringify(hs.shas);
    withholdAt = stmts.length + 3;
    stmts.push(
      c.db.prepare(`INSERT OR IGNORE INTO circle_holds (bank_id, reason, source, at, by) SELECT ?1, 'student_data', 'privacy', ?4, NULL WHERE ${won} AND ${PRIVACY_SQL("?2", "?3")}`).bind(bid, kj, sj, now),
      c.db.prepare(`INSERT OR IGNORE INTO circle_holds (bank_id, reason, source, at, by) SELECT ?1, 'student_data', 'privacy_near', ?3, NULL WHERE ?2 > 0 AND ${won}`).bind(bid, hs.near.length, now),
      c.db
        .prepare(`INSERT OR IGNORE INTO circle_holds (bank_id, reason, source, at, by) SELECT ?1, 'copyright', 'blocklist', ?4, NULL WHERE ${won} AND ${COPYRIGHT_SQL("?2", "?3", "?4", "?5")}`)
        .bind(bid, kj, sj, now, me),
      c.db
        .prepare(`UPDATE circle_versions SET withheld_at = ?2 WHERE id = ?1 AND withheld_at IS NULL AND ${won} AND EXISTS (SELECT 1 FROM circle_holds wh WHERE wh.bank_id = ?3 AND wh.source IN ${SEND_HOLD_SOURCES})`)
        .bind(head.id, now, bid),
      c.db
        .prepare(
          `INSERT INTO admin_flags (id, kind, item_id, publish_id, user_id, detail, at)
           SELECT ?1, 'privacy_text_match', ?2, ?3, ?4, ?5, ?6 WHERE ${won} AND EXISTS (SELECT 1 FROM circle_holds h WHERE h.bank_id = ?2 AND h.source IN ('privacy', 'privacy_near')) AND ${room}`,
        )
        .bind(newId(), bid, head.id, me, JSON.stringify({ circle_id: cid, version: head.version, positions: few(hs.exact), near: few(hs.near), hashes: hs.hits, via: "hide_key" }), now, cid, day0, flagCap),
      c.db
        .prepare(
          `INSERT INTO admin_flags (id, kind, item_id, publish_id, user_id, detail, at)
           SELECT ?1, 'blocklist', ?2, ?3, ?4, ?5, ?6 WHERE ${won} AND EXISTS (SELECT 1 FROM circle_holds h WHERE h.bank_id = ?2 AND h.source = 'blocklist') AND ${room}`,
        )
        .bind(newId(), bid, head.id, me, JSON.stringify({ circle_id: cid, version: head.version, positions: few(hs.copyright), actions: hs.actions, hashes: hs.chits, via: "hide_key" }), now, cid, day0, flagCap),
      // (round 48: withheld past the day's flags, named on the bank's cap flag)
      c.db
        .prepare(CAP_FLAG_SQL(won))
        .bind(newId(), bid, me, cid, head.version, head.id, JSON.stringify(hs.actions), hs.exact.length + hs.near.length > 0 ? "true" : "false", capDedup(bid, me, now), now),
      c.db.prepare(LISTING_REFRESH_SQL(won)).bind(bid),
    );
  }
  for (const v of toRecord) stmts.push(c.db.prepare(`UPDATE circle_versions SET strip_opts = ?2 WHERE id = ?1 AND strip_opts IS NULL AND withheld_at IS NULL AND ${won}`).bind(v.id, v.json));
  for (const v of toWithhold) {
    // (an older version, not the head: withheld, flagged, no bank hold; checked as of this PATCH)
    stmts.push(c.db.prepare(`UPDATE circle_versions SET withheld_at = ?2 WHERE id = ?1 AND withheld_at IS NULL AND ${won}`).bind(v.id, now));
    if (v.scan.exact.length + v.scan.near.length > 0)
      stmts.push(
        c.db
          .prepare(`INSERT INTO admin_flags (id, kind, item_id, publish_id, user_id, detail, at) SELECT ?1, 'privacy_text_match', ?2, ?3, ?4, ?5, ?6 WHERE ${won} AND ${room}`)
          .bind(newId(), bid, v.id, me, JSON.stringify({ circle_id: cid, version: v.version, positions: few(v.scan.exact), near: few(v.scan.near), hashes: v.scan.hits, via: "hide_key" }), now, cid, day0, flagCap),
      );
    if (v.scan.copyright.length > 0)
      stmts.push(
        c.db
          .prepare(`INSERT INTO admin_flags (id, kind, item_id, publish_id, user_id, detail, at) SELECT ?1, 'blocklist', ?2, ?3, ?4, ?5, ?6 WHERE ${won} AND ${room}`)
          .bind(newId(), bid, v.id, me, JSON.stringify({ circle_id: cid, version: v.version, positions: few(v.scan.copyright), actions: v.scan.actions, hashes: v.scan.chits, via: "hide_key" }), now, cid, day0, flagCap),
      );
    stmts.push(
      c.db
        .prepare(CAP_FLAG_SQL(won))
        .bind(newId(), bid, me, cid, v.version, v.id, JSON.stringify(v.scan.actions), v.scan.exact.length + v.scan.near.length > 0 ? "true" : "false", capDedup(bid, me, now), now),
    );
  }
  // (round 47: the evidence kept in full stays within its bound)
  if ((hs && head) || toWithhold.length > 0) stmts.push(...evidenceBound(c, cid, now, await limitOf(c, "*", "circle_bytes"), won));
  const withheldIds = JSON.stringify([...(hs && head ? [head.id] : []), ...toWithhold.map((v) => v.id)]);
  stmts.push(
    c.db
      .prepare(
        `SELECT ${won} AS won, EXISTS (SELECT 1 FROM circle_holds h WHERE h.bank_id = ?1) AS held,
                (SELECT withheld_at IS NOT NULL FROM circle_versions WHERE id = ?2) AS withheld,
                (SELECT json_group_array(json_object('kind', f.kind, 'version', f.detail ->> '$.version')) FROM admin_flags f WHERE f.item_id = ?1 AND f.at = ?3 AND f.detail ->> '$.via' = 'hide_key' AND ${won}) AS flagged,
                (SELECT cf.at = ?3 AND ${won} FROM admin_flags cf WHERE cf.dedup = ?5) AS capnew`,
      )
      .bind(bid, head?.id ?? "", now, withheldIds, capDedup(bid, me, now)),
  );
  const res = await c.db.batch(stmts);
  const out = (res[res.length - 1]!.results as { won: number; held: number; withheld: number | null; flagged: string | null; capnew: number | null }[])[0];
  if (!out || out.won !== 1) {
    await throwIfErased(c, me);
    const s = await c.db
      .prepare(
        `SELECT b.head_version, b.head_version_id, b.removed_at, b.removed_reason, b.hide_key, ci.qkeys, v.strip_opts, (v.withheld_at IS NULL AND v.full_key <> '') AS readable
           FROM circle_banks b JOIN circles ci ON ci.id = b.circle_id LEFT JOIN circle_versions v ON v.id = b.head_version_id WHERE b.id = ?1`,
      )
      .bind(bid)
      .first<{ head_version: number; head_version_id: string | null; removed_at: number | null; removed_reason: string | null; hide_key: number; qkeys: number; strip_opts: string | null; readable: number | null }>();
    if (s && s.removed_at !== null) throw new PoolError(409, "bank_removed", { reason: s.removed_reason });
    // (round 48, critic 55a V2: with the head it lost to, as a send's conflict)
    if (s && s.head_version_id !== b.head_version_id)
      throw new PoolError(409, "conflict", { head_version: s.head_version, message: "A new version arrived meanwhile; do it again for the new version." });
    // Round 53: another PATCH changed the setting or the head's decisions after this one read them. When the bank now
    // hides its keys as this PATCH asked (the decisions it would have recorded, or a head it could not strip now), it is
    // the same setting again: `unchanged`, with nothing written, no activity line and no change counted. Else a conflict.
    if (s && (s.hide_key !== b.hide_key || s.strip_opts !== (head?.strip_opts ?? null))) {
      if (s.hide_key === 1 && (s.readable !== 1 || (stripOpts !== null && s.strip_opts !== null && sameDecisions(decisionsOf(s.strip_opts), decisionsOf(stripOpts)))))
        return { ok: true, hide_key: true, unchanged: true };
      await explain(c, cid, me);
    }
    if (s && s.qkeys + fresh > records) throw new PoolError(413, "circle_records_full", { used: s.qkeys, needed: fresh, limit: records, message: "This circle has recorded as many different questions as it can keep." });
    await explain(c, cid, me);
  }
  // The admin is told of each version withheld here (ids only, their own hourly budgets), and copyright matches count
  // toward the account's held-publish budget, as for a send.
  const fl = JSON.parse(out?.flagged ?? "[]") as { kind: string; version: string | number }[];
  for (const f of fl) {
    const version = Number(f.version);
    if (f.kind === "privacy_text_match") await alertBudgeted(c, { kind: "privacy_text_match", itemId: bid, circleId: cid, version, held: true });
    else await alertBudgeted(c, { kind: "circle_copyright_match", itemId: bid, circleId: cid, version });
  }
  // Round 50 (critic 60 B1): `withheld` and `held` say what THIS PATCH did: it withheld the head (only a head it
  // scanned, one members could read), and the bank's hold came with it. A head withheld before (a send's match, an
  // earlier PATCH) is not scanned and is not reported: a GET says whether turning hiding off would release it.
  // Round 51 (critic 61 M1): and only when this batch's withhold changed the row: of two PATCHes that scanned the head
  // at once, the one that came second (the head already withheld) says false and is not charged for it.
  const withheldNow = hs !== null && out!.withheld === 1 && withholdAt >= 0 && (res[withholdAt]?.meta.changes ?? 0) === 1;
  const cPositions = (withheldNow ? (hs?.copyright.length ?? 0) : 0) + toWithhold.reduce((a, v) => a + v.scan.copyright.length, 0);
  if (cPositions > 0) await chargeHeldPublishes(c, me, cPositions);
  if (out!.capnew === 1) await alertBudgeted(c, { kind: "cap", userId: me, metric: "circle_send_flags" });
  return { ok: true, hide_key: true, held: withheldNow && out!.held === 1, withheld: withheldNow, ...report };
}

/** DELETE /circles/:cid/banks/:bid (owner or the bank's author, D17): hidden at once, 14 days to undo, then purged. */
export async function deleteBank(c: Ctx, p: PoolPrincipal, cid: string, bid: string): Promise<Response> {
  const me = p.userId;
  const { m, b } = await bankOrMiss(c, cid, bid, me);
  const author = b.author_id === me;
  if (!author && m.role !== "owner") throw new PoolError(403, "circle_role_required", { needed: "owner", role: m.role });
  if (b.removed_at !== null) {
    // Round 34 (critic41b F6): the owner's delete of a bank its author removed makes it the owner's removal (the
    // author can no longer undo it); the window is not extended.
    if (b.removed_reason === "author_removed" && m.role === "owner" && !author) {
      const now = c.now();
      const [up] = await c.db.batch([
        c.db
          .prepare(
            `UPDATE circle_banks SET removed_reason = 'owner_removed', updated_at = ?4 WHERE id = ?1 AND circle_id = ?2 AND removed_reason = 'author_removed' AND purged_at IS NULL
               AND ${ACTIVE_SQL("?2", "?3", ["owner"])} AND ${ALIVE_SQL("?3")}`,
          )
          .bind(bid, cid, me, now),
        audit(c.db, { actorId: me, actorKind: "user", action: "circle.bank_remove", itemId: cid, targetUserId: me, detail: { bank_id: bid } }, now, `changes() = 1`),
      ]);
      if ((up!.meta.changes ?? 0) !== 1) await throwIfErased(c, me);
    }
    if (b.removed_reason === "author_removed" || b.removed_reason === "owner_removed") return json(200, { ok: true, undo_until: b.purge_after });
    throw new PoolError(409, "under_moderation");
  }
  const now = c.now();
  const [upd] = await c.db.batch([
    c.db
      .prepare(
        `UPDATE circle_banks SET removed_at = ?4, removed_reason = ?5, purge_after = ?6, updated_at = ?4
          WHERE id = ?1 AND circle_id = ?2 AND removed_at IS NULL AND purged_at IS NULL AND (author_id = ?3 OR ${ACTIVE_SQL("?2", "?3", ["owner"])}) AND ${ACTIVE_SQL("?2", "?3")} AND ${ALIVE_SQL("?3")}`,
      )
      .bind(bid, cid, me, now, author ? "author_removed" : "owner_removed", now + CIRCLE_UNDO_MS),
    audit(c.db, { actorId: me, actorKind: "user", action: "circle.bank_remove", itemId: cid, targetUserId: me, detail: { bank_id: bid } }, now, `changes() = 1`),
  ]);
  if ((upd!.meta.changes ?? 0) !== 1) await explain(c, cid, me);
  return json(200, { ok: true, undo_until: now + CIRCLE_UNDO_MS });
}

/** POST /circles/:cid/banks/:bid/undelete: within its 14 days, the owner's removal by the owner, the author's own by the author or the owner.
 *  (A removed bank still counts toward the circle's caps until it is purged, so undoing needs no cap check.) */
export async function undeleteBank(c: Ctx, p: PoolPrincipal, cid: string, bid: string): Promise<Response> {
  const me = p.userId;
  const { m, b } = await bankOrMiss(c, cid, bid, me);
  requireOpen(m);
  if (b.removed_at === null) return json(200, { ok: true, already: true });
  if (b.removed_reason !== "author_removed" && b.removed_reason !== "owner_removed") throw new PoolError(409, "bank_removed", { reason: b.removed_reason, message: "This bank can no longer be restored." });
  // Round 33 (critic40b B2): the owner's removal is undone by the owner only; the author's own by either.
  if (m.role !== "owner" && (b.author_id !== me || b.removed_reason !== "author_removed")) throw new PoolError(403, "circle_role_required", { needed: "owner", role: m.role });
  const now = c.now();
  const [upd] = await c.db.batch([
    c.db
      .prepare(
        `UPDATE circle_banks SET removed_at = NULL, removed_reason = NULL, purge_after = NULL, updated_at = ?4
          WHERE id = ?1 AND circle_id = ?2 AND removed_reason IN ('author_removed', 'owner_removed') AND purge_after > ?4 AND purged_at IS NULL
            AND (${ACTIVE_SQL("?2", "?3", ["owner"])} OR (removed_reason = 'author_removed' AND author_id = ?3)) AND ${ACTIVE_SQL("?2", "?3")} AND ${OPEN_SQL("?2")} AND ${ALIVE_SQL("?3")}`,
      )
      .bind(bid, cid, me, now),
    audit(c.db, { actorId: me, actorKind: "user", action: "circle.bank_undelete", itemId: cid, targetUserId: me, detail: { bank_id: bid } }, now, `changes() = 1`),
  ]);
  if ((upd!.meta.changes ?? 0) !== 1) {
    await throwIfErased(c, me);
    throw new PoolError(409, "bank_removed", { reason: b.removed_reason, message: "This bank can no longer be restored." });
  }
  return json(200, { ok: true });
}

/**
 * POST /circles/:cid/banks/:bid/purge (the owner): a REMOVED bank (by its author, the owner, or its author's
 * erasure) is purged now instead of after its 14 days, so it stops counting toward the circle's caps and
 * records at once (round 33, critics 40a/40b V3: an editor's flood can be cleared). Never a bank under an
 * admin removal, a hold or an open report (the admin decides those). Not undoable.
 */
export async function purgeBankNow(c: Ctx, p: PoolPrincipal, cid: string, bid: string): Promise<Response> {
  const me = p.userId;
  const { m, b } = await bankOrMiss(c, cid, bid, me);
  requireRole(m, "owner");
  if (b.removed_at === null) throw new PoolError(409, "bank_not_removed", { message: "Remove the bank first." });
  if (b.removed_reason === "admin" || b.removed_reason === "privacy" || b.removed_reason === "takedown") throw new PoolError(409, "under_moderation");
  const now = c.now();
  // The claim (guarded: the owner, alive; no hold, no open report; not purged): purge_after now, so an
  // undelete racing it fails and the purge below is the only one.
  const claim = await c.db
    .prepare(
      `UPDATE circle_banks SET purge_after = ?4 WHERE id = ?1 AND circle_id = ?2 AND removed_at IS NOT NULL AND purged_at IS NULL
         AND removed_reason IN ('author_removed', 'owner_removed', 'account_deleted', 'circle_closed') AND NOT ${OPEN_REPORT_BANK("circle_banks.id")}
         AND ${ACTIVE_SQL("?2", "?3", ["owner"])} AND ${ALIVE_SQL("?3")}`,
    )
    .bind(bid, cid, me, now)
    .run();
  if ((claim.meta.changes ?? 0) !== 1) {
    await throwIfErased(c, me);
    // (round 41: a report or an admin's hold is moderation; an open student-data flag alone is a review to wait for)
    const why = await c.db
      .prepare(`SELECT (EXISTS (SELECT 1 FROM circle_reports r WHERE r.bank_id = ?1 AND r.resolved_at IS NULL) OR ${BLOCKING_HOLD_SQL("?1")}) AS moderated, ${OPEN_FLAG_BANK("?1")} AS flagged`)
      .bind(bid)
      .first<{ moderated: number; flagged: number }>();
    if (why && why.moderated === 0 && why.flagged === 1) return purgeWhenReviewed(c, bid, cid, me, now);
    throw new PoolError(409, "under_moderation", { message: "This bank is under review by an administrator." });
  }
  await purgeBank(c, bid, cid, "owner_purge", me);
  return json(200, { ok: true });
}

/**
 * Round 47 (critic 54b N1): purge-now of a removed bank whose only obstacle is an open flag (an administrator's review
 * of a withheld version): the purge is claimed (the bank can no longer be restored; the daily purge takes it once the
 * review is done, or once the administrator has marked its evidence captured) and every version but the evidence kept
 * for the administrator (its withheld versions whose flag pins them) is pruned now, so its bytes stop counting at once.
 * Round 48 (critics 55a V5, 55b V10): this is work done, so it is answered as done: `200 {ok, purged_partial: true,
 * pending_review: true, evidence_kept, message}` (it was a 409 that had committed). Undelete is refused afterwards.
 */
async function purgeWhenReviewed(c: Ctx, bid: string, cid: string, me: string, now: number): Promise<Response> {
  const res = await c.db.batch([
    c.db
      .prepare(
        `UPDATE circle_banks SET purge_after = ?4 WHERE id = ?1 AND circle_id = ?2 AND removed_at IS NOT NULL AND purged_at IS NULL
           AND removed_reason IN ('author_removed', 'owner_removed', 'account_deleted', 'circle_closed')
           AND NOT EXISTS (SELECT 1 FROM circle_reports r WHERE r.bank_id = ?1 AND r.resolved_at IS NULL) AND NOT ${BLOCKING_HOLD_SQL("?1")}
           AND ${ACTIVE_SQL("?2", "?3", ["owner"])} AND ${ALIVE_SQL("?3")}`,
      )
      .bind(bid, cid, me, now),
    c.db
      .prepare(`UPDATE circle_versions SET status = 'pruned', pruned_at = ?2 WHERE bank_id = ?1 AND status IN ('current', 'previous') AND changes() = 1 AND NOT ${PINNED_EVIDENCE_SQL("circle_versions")}`)
      .bind(bid, now),
    c.db.prepare(`DELETE FROM circle_version_blobs WHERE version_id IN (${PRUNED_NOW_SQL})`).bind(bid, now),
    audit(c.db, { actorId: me, actorKind: "user", action: "circle.bank_purge_requested", itemId: cid, targetUserId: me, detail: { bank_id: bid } }, now, `EXISTS (SELECT 1 FROM circle_banks pb WHERE pb.id = '${bid}' AND pb.purge_after = ${now})`),
    c.db
      .prepare(
        `SELECT EXISTS (SELECT 1 FROM circle_banks pb WHERE pb.id = ?1 AND pb.purge_after = ?2) AS claimed,
                (SELECT COUNT(*) FROM circle_versions ev INDEXED BY circle_versions_bank WHERE ev.bank_id = ?1 AND ev.status IN ('current', 'previous') AND ev.full_key <> '' AND ${PINNED_EVIDENCE_SQL("ev")}) AS n`,
      )
      .bind(bid, now),
  ]);
  const out = ((res[4]!.results as { claimed: number; n: number }[]) ?? [])[0];
  if (out?.claimed !== 1) {
    await throwIfErased(c, me);
    throw new PoolError(409, "under_moderation", { message: "This bank is under review by an administrator." });
  }
  return json(200, {
    ok: true,
    purged_partial: true,
    pending_review: true,
    evidence_kept: out.n,
    message: `An administrator is reviewing ${out.n === 1 ? "a version" : `${out.n} versions`} of this bank. Everything else in it has been deleted now and the bank cannot be restored; the rest is deleted once that review is done.`,
  });
}

/**
 * POST /circles/:cid/banks/:bid/report {reason, note?} (a member, not the author): always "thanks" (202).
 * D1: one student_data report from an ESTABLISHED member (account approved 7+ days, in this circle 1+ day,
 * no dismissed student-data report in 30 days, at most CIRCLE_INSTANT_HOLDS_PER_DAY a day, under the
 * site-wide instant-hold cap shared with the public pool) hides the bank from the circle at once; any other
 * reason hides it once `report_threshold` distinct members report it. The admin is told (ids only).
 */
export async function reportBank(c: Ctx, req: Request, p: PoolPrincipal, cid: string, bid: string): Promise<Response> {
  const me = p.userId;
  const { m, b } = await bankOrMiss(c, cid, bid, me);
  const body = await readJson(req, `u:${me}`, { deadlineMs: c.deps.bodyDeadlineMs });
  const reason = body.reason;
  if (typeof reason !== "string" || !(CIRCLE_REPORT_REASONS as readonly string[]).includes(reason)) throw bad("reason", { allowed: CIRCLE_REPORT_REASONS });
  const note = optText(body.note, 1000, "note");
  await requireReadable(c, m, b, me);
  if (b.author_id === me) throw new PoolError(400, "own_item");
  await requireDaily(c, me, "reports");
  const now = c.now();
  const rid = newId();
  const day0 = dayOf(now) * DAY_MS;
  const stmts: D1PreparedStatement[] = [
    c.db
      .prepare(
        `INSERT OR IGNORE INTO circle_reports (id, circle_id, bank_id, version, reporter_id, reason, note, at) SELECT ?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8
          WHERE ${ACTIVE_SQL("?2", "?5")} AND ${ALIVE_SQL("?5")}`,
      )
      .bind(rid, cid, bid, b.head_version, me, reason, note, now),
  ];
  if (reason === "student_data")
    stmts.push(
      c.db
        .prepare(
          `INSERT OR IGNORE INTO circle_holds (bank_id, reason, source, at, by) SELECT ?1, 'student_data', '', ?2, NULL
            WHERE EXISTS (SELECT 1 FROM circle_reports r WHERE r.id = ?9)
              AND NOT EXISTS (SELECT 1 FROM circle_reports d WHERE d.reporter_id = ?3 AND d.reason = 'student_data' AND d.action = 'dismissed' AND d.resolved_at > ?2 - ?4)
              AND (SELECT COUNT(*) FROM circle_holds h WHERE h.reason = 'student_data' AND h.source = '' AND h.at >= ?5
                     AND EXISTS (SELECT 1 FROM circle_reports r2 WHERE r2.bank_id = h.bank_id AND r2.reporter_id = ?3 AND r2.reason = 'student_data' AND r2.at = h.at)) < ?6
              AND (SELECT COUNT(*) FROM circle_holds h WHERE h.reason = 'student_data' AND h.source = '' AND h.at >= ?5)
                  + (SELECT COUNT(*) FROM item_holds ih WHERE ih.reason = 'student_data' AND ih.source = '' AND ih.at >= ?5) < ?7
              AND COALESCE((SELECT MAX(a.decided_at) FROM approval_requests a WHERE a.user_id = ?3 AND a.decision = 'approved'), (SELECT u.created_at FROM users u WHERE u.id = ?3)) <= ?2 - ?8
              AND EXISTS (SELECT 1 FROM circle_members em WHERE em.circle_id = ?10 AND em.user_id = ?3 AND em.left_at IS NULL AND em.joined_at <= ?2 - ?11)
              AND ${ALIVE_SQL("?3")}`,
        )
        .bind(bid, now, me, CIRCLE_THROTTLE_MS, day0, CIRCLE_INSTANT_HOLDS_PER_DAY, await limitOf(c, "*", "instant_holds_global"), ESTABLISHED_ACCOUNT_MS, rid, cid, ESTABLISHED_MEMBER_MS),
    );
  else
    stmts.push(
      c.db
        .prepare(
          `INSERT OR IGNORE INTO circle_holds (bank_id, reason, source, at, by)
           SELECT ?1, 'reports', '', ?2, NULL WHERE (SELECT COUNT(DISTINCT r.reporter_id) FROM circle_reports r
               WHERE r.bank_id = ?1 AND r.resolved_at IS NULL AND r.reason <> 'student_data'
                 AND NOT EXISTS (SELECT 1 FROM circle_reports d WHERE d.bank_id = ?1 AND d.reporter_id = r.reporter_id AND d.action = 'dismissed' AND d.resolved_at > ?2 - ?4)) >= ?3
             AND ${ALIVE_SQL("?5")}`,
        )
        .bind(bid, now, await limitOf(c, "*", "report_threshold"), CIRCLE_THROTTLE_MS, me),
    );
  // (the report's id stays out of the activity list; the audit names the circle and bank only)
  stmts.push(audit(c.db, { actorId: me, actorKind: "user", action: "circle.report", itemId: cid, detail: { bank_id: bid, reason } }, now, `EXISTS (SELECT 1 FROM circle_reports r WHERE r.id = '${rid}')`));
  const res = await c.db.batch(stmts);
  if ((res[0]!.meta.changes ?? 0) === 0) await throwIfErased(c, me);
  if ((res[0]!.meta.changes ?? 0) === 1) await alertBudgeted(c, { kind: "circle_report", circleId: cid, bankId: bid, reportId: rid, reason, held: (res[1]!.meta.changes ?? 0) === 1 });
  return json(202, { ok: true, message: "Thanks, the admin will review it." });
}

// ---- erasure (plan 8.1, 8.2) -------------------------------------------------------------------------------------

/**
 * The circle statements of the account-deletion batch (run by deleteContentStatements, before its
 * pool_prefs row goes), in this order, each set-based and bounded by the account's own rows:
 * 1. its active memberships step down ('account_deleted', audited for the activity list);
 * 2. succession in every open circle it was in that now has no active owner (editor, then viewer, D14);
 * 3. ... and those with no active member left close ('last_member_gone', purged in 14 days);
 * 4. banks it authored: removed (purged in 14 days) when it chose "do not keep my shared items", else
 *    kept, shown as by "deleted teacher" (D15);
 * 5. free text it wrote goes: version messages, report notes (its reports lose their reporter);
 * 6. codes it created are revoked (their notes go);
 * 7. its membership rows are deleted.
 * Versions it sent keep their (now opaque) committer id; its first_by rows in circle_qkeys stay opaque.
 * Every statement matches only what is not yet clean, so a rerun (erasure_finish) changes nothing.
 */
export function circleErasureStatements(db: D1Database, userId: string, now: number): D1PreparedStatement[] {
  const purgeAt = now + CIRCLE_UNDO_MS;
  const noKeep = `EXISTS (SELECT 1 FROM pool_prefs WHERE user_id = ?1 AND keep_on_delete = 0)`;
  return [
    db
      .prepare(
        `INSERT INTO pool_audit (id, at, actor_id, actor_kind, action, item_id, target_user_id)
         SELECT lower(hex(randomblob(16))), ?2, NULL, 'system', 'circle.member_erased', circle_id, user_id FROM circle_members WHERE user_id = ?1 AND left_at IS NULL`,
      )
      .bind(userId, now),
    db.prepare(`UPDATE circle_members SET left_at = ?2, left_reason = 'account_deleted' WHERE user_id = ?1 AND left_at IS NULL`).bind(userId, now),
    ...successionStatements(db, `SELECT circle_id FROM circle_members WHERE user_id = ?1`, [userId, now, purgeAt], "?2", "?3"),
    db
      .prepare(`UPDATE circle_banks SET removed_at = ?2, removed_reason = 'account_deleted', purge_after = ?3, updated_at = ?2 WHERE author_id = ?1 AND removed_at IS NULL AND purged_at IS NULL AND ${noKeep}`)
      .bind(userId, now, purgeAt),
    db.prepare(`UPDATE circle_banks SET anonymized = 1 WHERE author_id = ?1 AND anonymized = 0 AND purged_at IS NULL AND NOT ${noKeep}`).bind(userId),
    db.prepare(`UPDATE circle_versions SET message = NULL WHERE committer_id = ?1 AND message IS NOT NULL`).bind(userId),
    db.prepare(`UPDATE circle_reports SET note = NULL, reporter_id = NULL WHERE reporter_id = ?1`).bind(userId),
    // (round 46: and the masked addresses of its email invites)
    db
      .prepare(`UPDATE circle_invites SET revoked_at = COALESCE(revoked_at, ?2), note = NULL, email_label = NULL, email_key = NULL WHERE created_by = ?1 AND (revoked_at IS NULL OR note IS NOT NULL OR email_label IS NOT NULL OR email_key IS NOT NULL)`)
      .bind(userId, now),
    // (round 46: its hourly invitation count, the one row that names it)
    db.prepare(`DELETE FROM pool_alert_budget WHERE channel = 'circle_email_inviter:' || ?1`).bind(userId),
    // (round 48, critic 55b V4: and its share of each domain's hour, `circle_email_domain:<hmac>:<user id>`.
    // Critic 58 B1: no LIKE here. `LIKE 'circle_email_domain:%:' || ?1` is a 58-byte pattern, over D1's 50, and failed
    // the whole batch for every account; the prefix and the id after the hmac are compared exactly, as the resweep reads them.)
    db
      .prepare(`DELETE FROM pool_alert_budget WHERE substr(channel, 1, 20) = 'circle_email_domain:' AND substr(channel, 21 + instr(substr(channel, 21), ':')) = ?1`)
      .bind(userId),
    // (round 34: ownership offers from or to the account)
    db.prepare(`DELETE FROM circle_transfers WHERE from_user = ?1`).bind(userId),
    db.prepare(`DELETE FROM circle_transfers WHERE to_user = ?1`).bind(userId),
    db.prepare(`DELETE FROM circle_members WHERE user_id = ?1`).bind(userId),
  ];
}

/** Open circles without an active owner the daily resweep repairs at most a run (plan 8.2). */
export const RESWEEP_CIRCLES = 50;
/** The safety net's pick (one indexed walk of the open circles a day, each probed by the one-owner index). */
export const OWNERLESS_SQL = `SELECT c2.id FROM circles c2 INDEXED BY circles_open WHERE c2.closed_at IS NULL AND c2.purged_at IS NULL
  AND NOT EXISTS (SELECT 1 FROM circle_members o2 WHERE o2.circle_id = c2.id AND o2.role = 'owner' AND o2.left_at IS NULL) LIMIT ${RESWEEP_CIRCLES}`;
/** Round 34: owner-closed circles inside their undo window without an owner (by the purge index; ?1 = now). */
export const OWNERLESS_CLOSED_SQL = `SELECT c3.id FROM circles c3 INDEXED BY circles_purge WHERE c3.purge_after IS NOT NULL AND c3.purged_at IS NULL
  AND c3.close_reason = 'owner_closed' AND c3.undo_until > ?1
  AND NOT EXISTS (SELECT 1 FROM circle_members o3 WHERE o3.circle_id = c3.id AND o3.role = 'owner' AND o3.left_at IS NULL) LIMIT ${RESWEEP_CIRCLES}`;

/**
 * The daily erasure_resweep's circle part (plan 8.2): every circle table a request already in flight could
 * have written under an erased account's id (memberships step down, hand on and go; codes are revoked;
 * messages and report notes go; banks it authored are kept anonymised), and, as a safety net, the open
 * circles `ownerless` (ids, OWNERLESS_SQL) get their succession. Idempotent.
 */
export function circleResweepStatements(db: D1Database, now: number, ownerless: string[]): D1PreparedStatement[] {
  const done = `SELECT user_id FROM erasure_done`;
  const stray = `SELECT DISTINCT circle_id FROM circle_members WHERE user_id IN (${done})`;
  return [
    db
      .prepare(
        `INSERT INTO pool_audit (id, at, actor_id, actor_kind, action, item_id, target_user_id)
         SELECT lower(hex(randomblob(16))), ?1, NULL, 'system', 'circle.member_erased', circle_id, user_id FROM circle_members WHERE user_id IN (${done}) AND left_at IS NULL`,
      )
      .bind(now),
    db.prepare(`UPDATE circle_members SET left_at = ?1, left_reason = 'account_deleted' WHERE user_id IN (${done}) AND left_at IS NULL`).bind(now),
    ...successionStatements(db, `${stray} UNION SELECT value FROM json_each(?3)`, [now, now + CIRCLE_UNDO_MS, JSON.stringify(ownerless)], "?1", "?2"),
    db.prepare(`UPDATE circle_banks SET anonymized = 1 WHERE author_id IN (${done}) AND anonymized = 0 AND purged_at IS NULL`),
    db.prepare(`UPDATE circle_versions SET message = NULL WHERE committer_id IN (${done}) AND message IS NOT NULL`),
    db.prepare(`UPDATE circle_reports SET note = NULL, reporter_id = NULL WHERE reporter_id IN (${done})`),
    db
      .prepare(`UPDATE circle_invites SET revoked_at = COALESCE(revoked_at, ?1), note = NULL, email_label = NULL, email_key = NULL WHERE created_by IN (${done}) AND (revoked_at IS NULL OR note IS NOT NULL OR email_label IS NOT NULL OR email_key IS NOT NULL)`)
      .bind(now),
    // (round 48, critic 55b V4: the hour's mail counters keyed by an erased inviter)
    db.prepare(`DELETE FROM pool_alert_budget WHERE channel LIKE 'circle_email_inviter:%' AND substr(channel, 22) IN (${done})`),
    db.prepare(`DELETE FROM pool_alert_budget WHERE channel LIKE 'circle_email_domain:%:%' AND substr(channel, 21 + instr(substr(channel, 21), ':')) IN (${done})`),
    db.prepare(`DELETE FROM circle_transfers WHERE from_user IN (${done})`),
    db.prepare(`DELETE FROM circle_transfers WHERE to_user IN (${done})`),
    db.prepare(`DELETE FROM circle_members WHERE user_id IN (${done})`),
  ];
}

// ---- cron: abandoned sends, pruned versions, purges (plan 8.3) ---------------------------------------------------

/** Hourly: a send whose staging row is older than an hour never committed: its row and R2 objects go. */
export async function discardCircleStaging(c: Ctx): Promise<number> {
  const now = c.now();
  const stale = await c.db
    .prepare(`SELECT id, full_key, index_key, created_at FROM circle_versions WHERE status = 'staging' AND created_at < ?1 ORDER BY created_at LIMIT 200`)
    .bind(now - 3_600_000)
    .all<{ id: string; full_key: string; index_key: string; created_at: number }>();
  let n = 0;
  for (const r of stale.results) {
    const keys = [r.full_key, r.index_key].filter((k) => k !== "");
    const [del] = await c.db.batch([
      c.db.prepare(`DELETE FROM circle_versions WHERE id = ?1 AND status = 'staging' AND created_at = ?2`).bind(r.id, r.created_at),
      c.db.prepare(`INSERT OR IGNORE INTO r2_orphans (key, at) SELECT value, ?2 FROM json_each(?1) WHERE changes() = 1`).bind(JSON.stringify(keys), now),
    ]);
    if ((del!.meta.changes ?? 0) !== 1) continue;
    await deleteKeys(c, keys);
    n++;
  }
  return n;
}

/** Pruned versions whose R2 objects a daily run deletes at most. */
export const PRUNE_BATCH = 500;
/** Daily: the bodies and indexes of versions past the KEPT_VERSIONS kept (D16) go; their history rows stay. */
export async function pruneCircleVersions(c: Ctx): Promise<number> {
  const rows = await c.db
    .prepare(`SELECT id, full_key, index_key FROM circle_versions INDEXED BY circle_versions_prune WHERE status = 'pruned' AND full_key <> '' ORDER BY pruned_at LIMIT ${PRUNE_BATCH}`)
    .all<{ id: string; full_key: string; index_key: string }>();
  if (rows.results.length === 0) return 0;
  const ids = JSON.stringify(rows.results.map((r) => r.id));
  const keys = rows.results.flatMap((r) => [r.full_key, r.index_key]).filter((k) => k !== "");
  const now = c.now();
  await c.db.batch([
    c.db.prepare(`INSERT OR IGNORE INTO r2_orphans (key, at) SELECT value, ?2 FROM json_each(?1)`).bind(JSON.stringify(keys), now),
    c.db.prepare(`DELETE FROM circle_version_blobs WHERE version_id IN (SELECT value FROM json_each(?1))`).bind(ids),
    c.db.prepare(`UPDATE circle_versions SET full_key = '', index_key = '' WHERE id IN (SELECT value FROM json_each(?1)) AND status = 'pruned'`).bind(ids),
  ]);
  await deleteKeys(c, keys);
  return rows.results.length;
}

/** Banks and circles one daily purge run takes at most. */
export const CIRCLE_PURGE_BANKS = 200;
export const CIRCLE_PURGE_CIRCLES = 50;

/** Purges one bank: R2 bodies and indexes, versions, their images' rows, its qkeys, holds; the row stays as a tombstone. */
async function purgeBank(c: Ctx, bid: string, cid: string, why: string, actor: string | null = null): Promise<void> {
  const now = c.now();
  const vers = await c.db.prepare(`SELECT id, full_key, index_key FROM circle_versions WHERE bank_id = ?1`).bind(bid).all<{ id: string; full_key: string; index_key: string }>();
  const keys = vers.results.flatMap((r) => [r.full_key, r.index_key]).filter((k) => k !== "");
  const shas = (
    await c.db
      .prepare(`SELECT DISTINCT vb.sha256 FROM circle_version_blobs vb WHERE vb.version_id IN (SELECT id FROM circle_versions WHERE bank_id = ?1)`)
      .bind(bid)
      .all<{ sha256: string }>()
  ).results.map((x) => x.sha256);
  await c.db.batch([
    c.db.prepare(`INSERT OR IGNORE INTO r2_orphans (key, at) SELECT value, ?2 FROM json_each(?1)`).bind(JSON.stringify(keys), now),
    c.db.prepare(`DELETE FROM circle_version_blobs WHERE version_id IN (SELECT id FROM circle_versions WHERE bank_id = ?1)`).bind(bid),
    c.db.prepare(`DELETE FROM circle_versions WHERE bank_id = ?1`).bind(bid),
    c.db.prepare(`DELETE FROM circle_qkeys WHERE bank_id = ?1`).bind(bid),
    c.db.prepare(`UPDATE circles SET qkeys = MAX(0, qkeys - changes()) WHERE id = ?1`).bind(cid),
    c.db.prepare(`DELETE FROM circle_holds WHERE bank_id = ?1`).bind(bid),
    // (the tombstone keeps no typed text)
    c.db
      .prepare(`UPDATE circle_banks SET purged_at = COALESCE(purged_at, ?2), title = '', subject = '', course = '', grade = '', head_version_id = NULL, purge_after = NULL, removed_at = COALESCE(removed_at, ?2), removed_reason = COALESCE(removed_reason, 'circle_closed') WHERE id = ?1`)
      .bind(bid, now),
    audit(c.db, { actorId: actor, actorKind: actor ? "user" : "system", action: "circle.bank_purge", itemId: cid, targetUserId: actor, detail: { bank_id: bid, why } }, now),
  ]);
  if (keys.length) await deleteKeys(c, keys);
  if (shas.length) await gcBlobs(c, shas);
}

/** An open report or a hold keeps a bank (and its circle) until the admin has decided (the evidence stays;
 *  round 33, critic40a B4: a hold delays the purge as it does for items). */
// (round 40, critic 47a V1: a student-data hold a send raised does not keep a bank from its purge: the purge deletes
// the withheld text, which is the outcome that hold is for; the admin's flag stays with ids and positions)
// Round 41 (critic 48 V1): ... but not before the administrator has judged it: while a student-data flag on the bank
// is open, its withheld text (the admin's evidence, a near-rewording may be a false positive) is not purged.
// (round 44: and a copyright match's `blocklist` flag, the same way)
// (round 48: a flag whose evidence the administrator captured keeps nothing from a purge)
const OPEN_FLAG_BANK = (b: string) => `EXISTS (SELECT 1 FROM admin_flags pf WHERE pf.item_id = ${b} AND ${EVIDENCE_FLAG_SQL("pf")})`;
const OPEN_REPORT_BANK = (b: string) =>
  `(EXISTS (SELECT 1 FROM circle_reports r WHERE r.bank_id = ${b} AND r.resolved_at IS NULL) OR ${BLOCKING_HOLD_SQL(b)} OR ${OPEN_FLAG_BANK(b)})`;
const OPEN_REPORT_CIRCLE = (ci: string) =>
  `(EXISTS (SELECT 1 FROM circle_reports r WHERE r.circle_id = ${ci} AND r.resolved_at IS NULL)
    OR EXISTS (SELECT 1 FROM circle_banks hb JOIN circle_holds hh ON hh.bank_id = hb.id WHERE hb.circle_id = ${ci} AND hb.purged_at IS NULL AND hh.source NOT IN ${SEND_HOLD_SOURCES})
    OR EXISTS (SELECT 1 FROM circle_banks fb WHERE fb.circle_id = ${ci} AND fb.purged_at IS NULL AND ${OPEN_FLAG_BANK("fb.id")}))`;
// (round 44: the words name no reason: a copyright match and a student-data match must look the same to members)
const pendingReview = () => new PoolError(409, "pending_review", { message: "An administrator is reviewing a version of this bank; it can be purged once that review is done." });

/** Daily: removed banks and closed circles past purge_after (never one with an open report or a hold), in bounded batches. */
export async function purgeCircles(c: Ctx): Promise<{ banks: number; circles: number }> {
  const now = c.now();
  // (round 34: expired ownership offers)
  await c.db.prepare(`DELETE FROM circle_transfers WHERE expires_at <= ?1`).bind(now).run();
  let banks = 0;
  const due = await c.db
    .prepare(
      `SELECT b.id, b.circle_id FROM circle_banks b WHERE b.purge_after IS NOT NULL AND b.purge_after <= ?1 AND b.purged_at IS NULL AND NOT ${OPEN_REPORT_BANK("b.id")}
        ORDER BY b.purge_after LIMIT ${CIRCLE_PURGE_BANKS}`,
    )
    .bind(now)
    .all<{ id: string; circle_id: string }>();
  for (const b of due.results) {
    await purgeBank(c, b.id, b.circle_id, "purge_after");
    banks++;
  }
  const circles = await c.db
    .prepare(`SELECT ci.id FROM circles ci WHERE ci.purge_after IS NOT NULL AND ci.purge_after <= ?1 AND ci.purged_at IS NULL AND NOT ${OPEN_REPORT_CIRCLE("ci.id")} ORDER BY ci.purge_after LIMIT ${CIRCLE_PURGE_CIRCLES}`)
    .bind(now)
    .all<{ id: string }>();
  let n = 0;
  for (const ci of circles.results) {
    // (a reopen that won meanwhile: the circle is open again and stays)
    const still = await c.db.prepare(`SELECT 1 AS x FROM circles WHERE id = ?1 AND closed_at IS NOT NULL AND purge_after <= ?2 AND purged_at IS NULL`).bind(ci.id, now).first();
    if (!still) continue;
    const bs = await c.db.prepare(`SELECT id FROM circle_banks WHERE circle_id = ?1 AND purged_at IS NULL`).bind(ci.id).all<{ id: string }>();
    for (const b of bs.results) await purgeBank(c, b.id, ci.id, "circle_closed");
    await c.db.batch([
      c.db.prepare(`DELETE FROM circle_invites WHERE circle_id = ?1`).bind(ci.id),
      c.db.prepare(`DELETE FROM circle_transfers WHERE circle_id = ?1`).bind(ci.id),
      c.db.prepare(`DELETE FROM circle_members WHERE circle_id = ?1`).bind(ci.id),
      c.db.prepare(`UPDATE circles SET purged_at = ?2, name = NULL, description = NULL, purge_after = NULL, undo_until = NULL, qkeys = 0 WHERE id = ?1 AND closed_at IS NOT NULL AND purged_at IS NULL`).bind(ci.id, now),
      audit(c.db, { actorKind: "system", action: "circle.purge", itemId: ci.id }, now),
    ]);
    n++;
  }
  return { banks, circles: n };
}

// ---- export (plan 6.5) ---------------------------------------------------------------------------------------------

/** The account's circles section of GET /me/export: memberships, circles owned, codes created (no codes), versions sent (no bodies), reports filed. */
export async function exportCircles(c: Ctx, me: string): Promise<Record<string, unknown>> {
  const q = async <T>(sql: string) => (await c.db.prepare(sql).bind(me).all<T>()).results;
  return {
    memberships: await q(
      `SELECT m.circle_id, COALESCE(ci.name, '') AS circle_name, m.role, m.joined_at, m.left_at, m.left_reason FROM circle_members m JOIN circles ci ON ci.id = m.circle_id WHERE m.user_id = ?1 ORDER BY m.joined_at`,
    ),
    owned: await q(`SELECT m.circle_id, COALESCE(ci.name, '') AS circle_name, ci.created_at, ci.closed_at FROM circle_members m JOIN circles ci ON ci.id = m.circle_id WHERE m.user_id = ?1 AND m.role = 'owner' AND m.left_at IS NULL ORDER BY ci.created_at`),
    invites_created: await q(`SELECT id AS invite_id, circle_id, kind, COALESCE(email_label, '') AS address_label, role, created_at, max_uses, uses, expires_at, revoked_at, COALESCE(note, '') AS note FROM circle_invites WHERE created_by = ?1 ORDER BY created_at`),
    versions_sent: await q(`SELECT circle_id, bank_id, version, committed_at, COALESCE(message, '') AS message, license FROM circle_versions WHERE committer_id = ?1 AND status <> 'staging' ORDER BY committed_at`),
    banks_authored: await q(`SELECT circle_id, id AS bank_id, title, license, head_version, created_at, removed_at FROM circle_banks WHERE author_id = ?1 AND purged_at IS NULL ORDER BY created_at`),
    reports: await q(`SELECT circle_id, bank_id, reason, COALESCE(note, '') AS note, at, resolved_at, action FROM circle_reports WHERE reporter_id = ?1 ORDER BY at`),
    // (round 34: ownership offers it made or received and that are still open)
    transfer_offers: (
      await c.db
        .prepare(`SELECT circle_id, from_user = ?1 AS offered_by_me, to_user = ?1 AS offered_to_me, created_at, expires_at FROM circle_transfers WHERE (from_user = ?1 OR to_user = ?1) AND expires_at > ?2 ORDER BY created_at`)
        .bind(me, c.now())
        .all()
    ).results,
  };
}
