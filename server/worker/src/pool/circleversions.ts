// Round 37: circles phase B, the history side of shared editing (docs/circles-plan.md 4.1, 6.2, 13.2; D16).
//
// GET /circles/:cid/banks/:bid/versions (a member who may read the bank): the history, newest first,
// HISTORY_PAGE a page (`?before=<version>`), each line by key from the (bank_id, version) index.
// GET /circles/:cid/banks/:bid/versions/:n: a KEPT version's body (the merge base), streamed like the head
// (circles.ts getBank); a pruned one is `404 version_not_retained {oldest_kept}`.
// POST /circles/:cid/banks/:bid/merge {base_version} (owner, editor): merge-initiate. Informational: the head,
// whether the base is still kept, the versions in between, and a merge token (HMAC, MERGE_TOKEN_MS, binding
// the account, the bank, the base and the head). Nothing is stored and nothing is locked; the token only
// lets the next send (based on that head) record "merged from the base" in the history.
import { b64url, fromB64url, hmac, timingSafeEqual, utf8 } from "../../../auth/src/crypto.ts";
import { ACTIVE_SQL, BLOCKING_HOLD_SQL, bankOrMiss, getBank, KEPT_VERSIONS, keptInfo, nameOf, OPEN_SQL, requireOpen, requireReadable, requireRole, text } from "./circles.ts";
import { ALIVE_SQL, audit, json, PoolError, readJson, throwIfErased, type Ctx, type PoolPrincipal } from "./shared.ts";
import { PRUNED_NOW_SQL } from "./circlesend.ts";

/** History lines a page. */
export const HISTORY_PAGE = 50;
/** How long a merge token is good for. */
export const MERGE_TOKEN_MS = 30 * 60_000;
/** Versions between the base and the head a merge-initiate lists at most (`between_total` says how many there are). */
export const MERGE_BETWEEN_MAX = 50;

interface Line {
  version: number;
  id: string;
  committed_at: number;
  message: string | null;
  added: number;
  changed: number;
  removed: number;
  question_count: number;
  license: string;
  rollback_of: number | null;
  merged_from: number | null;
  status: string;
  full_key: string;
  withheld_at: number | null;
  display_name: string | null;
  deleted_at: number | null;
}
const LINE_COLS = `v.version, v.id, v.committed_at, v.message, v.added, v.changed, v.removed, v.question_count, v.license, v.rollback_of, v.merged_from, v.status, v.full_key, v.withheld_at, u.display_name, u.deleted_at`;
/** A version members may read (kept in full and not withheld). */
const available = (r: { status: string; full_key: string; withheld_at: number | null }) => (r.status === "current" || r.status === "previous") && r.full_key !== "" && r.withheld_at === null;
/** One history line as members see it: who (a name, "deleted teacher" once erased), when, the message and counts; no ids of people.
 *  Round 38: a withheld version (it held confirmed student data) is `withheld: true`, `kept: false`, and its message is not shown. */
const lineOf = (r: Line) => ({
  version: r.version,
  version_id: r.id,
  by: nameOf(r.display_name, r.deleted_at),
  at: r.committed_at,
  message: r.withheld_at !== null ? "" : text(r.message),
  added: r.added,
  changed: r.changed,
  removed: r.removed,
  question_count: r.question_count,
  license: r.license,
  kept: available(r),
  withheld: r.withheld_at !== null,
  ...(r.rollback_of !== null ? { rollback_of: r.rollback_of } : {}),
  ...(r.merged_from !== null ? { merged_from: r.merged_from } : {}),
});

/** GET /circles/:cid/banks/:bid/versions?before=<n>: the history, newest first. */
export async function history(c: Ctx, p: PoolPrincipal, cid: string, bid: string, url: URL): Promise<Response> {
  const me = p.userId;
  const { m, b } = await bankOrMiss(c, cid, bid, me);
  await requireReadable(c, m, b, me);
  const bs = url.searchParams.get("before");
  if (bs !== null && !/^[1-9][0-9]{0,8}$/.test(bs)) throw new PoolError(400, "invalid_request", { field: "before" });
  const before = bs === null ? b.head_version + 1 : Number(bs);
  const rows = await c.db
    .prepare(`SELECT ${LINE_COLS} FROM circle_versions v LEFT JOIN users u ON u.id = v.committer_id WHERE v.bank_id = ?1 AND v.version < ?2 ORDER BY v.version DESC LIMIT ?3`)
    .bind(bid, before, HISTORY_PAGE + 1)
    .all<Line>();
  const page = rows.results.slice(0, HISTORY_PAGE);
  const last = page[page.length - 1];
  return json(200, {
    bank_id: bid,
    head_version: b.head_version,
    ...(await keptInfo(c, bid, b.head_version)),
    versions: page.map(lineOf),
    next_before: rows.results.length > HISTORY_PAGE && last ? last.version : null,
  });
}

/** GET /circles/:cid/banks/:bid/versions/:n: a kept version's body. */
export function getVersion(c: Ctx, req: Request, p: PoolPrincipal, cid: string, bid: string, n: string): Promise<Response> {
  return getBank(c, req, p, cid, bid, false, Number(n));
}

/** POST /circles/:cid/banks/:bid/merge {base_version}: merge-initiate (informational). */
export async function mergeInitiate(c: Ctx, req: Request, p: PoolPrincipal, cid: string, bid: string): Promise<Response> {
  const me = p.userId;
  const { m, b } = await bankOrMiss(c, cid, bid, me);
  const body = await readJson(req, `u:${me}`, { deadlineMs: c.deps.bodyDeadlineMs });
  await requireReadable(c, m, b, me);
  requireRole(m, "owner", "editor");
  const base = body.base_version;
  if (typeof base !== "number" || !Number.isInteger(base) || base < 1 || base >= b.head_version)
    throw new PoolError(400, "invalid_request", { field: "base_version", head_version: b.head_version, message: "Merge from an earlier version than the circle's current one." });
  const head = b.head_version;
  const rows = await c.db
    // (round 38, critic 45a variant 6: the FIRST versions after the base, oldest first)
    .prepare(`SELECT ${LINE_COLS} FROM circle_versions v LEFT JOIN users u ON u.id = v.committer_id WHERE v.bank_id = ?1 AND v.version > ?2 AND v.version <= ?3 ORDER BY v.version LIMIT ?4`)
    .bind(bid, base, head, MERGE_BETWEEN_MAX)
    .all<Line>();
  const baseRow = await c.db.prepare(`SELECT status, full_key, withheld_at FROM circle_versions WHERE bank_id = ?1 AND version = ?2`).bind(bid, base).first<{ status: string; full_key: string; withheld_at: number | null }>();
  const top = await c.db
    .prepare(`SELECT v.committed_at, u.display_name, u.deleted_at FROM circle_versions v LEFT JOIN users u ON u.id = v.committer_id WHERE v.bank_id = ?1 AND v.version = ?2`)
    .bind(bid, head)
    .first<{ committed_at: number; display_name: string | null; deleted_at: number | null }>();
  const tok = await mergeToken(c, me, bid, base, head);
  return json(200, {
    bank_id: bid,
    base_version: base,
    base_kept: !!baseRow && available(baseRow),
    ...(await keptInfo(c, bid, head)),
    head_version: head,
    head_version_id: b.head_version_id,
    head_at: top?.committed_at ?? b.updated_at,
    head_by: top ? nameOf(top.display_name, top.deleted_at) : nameOf(null, 1),
    between: rows.results.map(lineOf),
    between_total: head - base,
    merge_token: tok.token,
    merge_token_expires_at: tok.exp,
  });
}

/** A merge token: [tag, expiry, account, bank, base, head], HMAC'd with the pool's merge key (HKDF purpose of its own). */
export async function mergeToken(c: Ctx, me: string, bid: string, base: number, head: number): Promise<{ token: string; exp: number }> {
  const exp = c.now() + MERGE_TOKEN_MS;
  const body = b64url(utf8(JSON.stringify(["m1", exp, me, bid, base, head])));
  return { token: `${body}.${await hmac((await c.keys).merge, body)}`, exp };
}

/**
 * The base a send's merge token records (`merged_from`), iff the token was issued to `me` for bank `bid` at
 * head `head` (the version the send is based on) and has not expired; else `422 merge_token_invalid`.
 */
export async function verifyMergeToken(c: Ctx, token: string, me: string, bid: string, head: number): Promise<number> {
  const fail = () => new PoolError(422, "merge_token_invalid", { message: "This merge is out of date. Get the latest changes and merge again, then send." });
  const m = /^([A-Za-z0-9_-]{1,400})\.([A-Za-z0-9_-]{43})$/.exec(token);
  if (!m) throw fail();
  const mac = await hmac((await c.keys).merge, m[1]!);
  if (mac.length !== m[2]!.length || !timingSafeEqual(utf8(mac), utf8(m[2]!))) throw fail();
  let t: unknown;
  try {
    t = JSON.parse(new TextDecoder().decode(fromB64url(m[1]!)));
  } catch {
    throw fail();
  }
  if (!Array.isArray(t) || t.length !== 6) throw fail();
  const [tag, exp, u, b, base, h] = t as [unknown, unknown, unknown, unknown, unknown, unknown];
  if (tag !== "m1" || typeof exp !== "number" || exp < c.now() || u !== me || b !== bid || h !== head) throw fail();
  if (typeof base !== "number" || !Number.isInteger(base) || base < 1 || base >= head) throw fail();
  return base;
}

/**
 * Round 38 (critic 45a B2): POST /circles/:cid/banks/:bid/trim {keep} (the owner, or the bank's author while an
 * editor; D17's rights): the bank's versions older than its newest `keep` (1 to KEPT_VERSIONS - 1) are pruned now, as
 * D16 prunes past the 20: their history lines stay, their bodies stop counting toward the circle's bytes at once (the
 * daily stage deletes them). How a circle whose storage is full gets room back without removing a bank. Never while
 * the bank is held, never a version an open report names (the evidence stays for the administrator).
 */
export async function trimVersions(c: Ctx, req: Request, p: PoolPrincipal, cid: string, bid: string): Promise<Response> {
  const me = p.userId;
  const { m, b } = await bankOrMiss(c, cid, bid, me);
  const body = await readJson(req, `u:${me}`, { deadlineMs: c.deps.bodyDeadlineMs });
  await requireReadable(c, m, b, me);
  requireOpen(m);
  if (!(m.role === "owner" || (b.author_id === me && m.role === "editor"))) throw new PoolError(403, "circle_role_required", { needed: "owner", role: m.role });
  const keep = body.keep;
  if (typeof keep !== "number" || !Number.isInteger(keep) || keep < 1 || keep >= KEPT_VERSIONS) throw new PoolError(400, "invalid_request", { field: "keep", min: 1, max: KEPT_VERSIONS - 1 });
  // (round 39, critic 46a M1: a student-data hold a send raised does not stop a trim, a report's or an admin's does)
  if (b.held === 1 && (await c.db.prepare(`SELECT 1 AS x WHERE ${BLOCKING_HOLD_SQL("?1")}`).bind(bid).first()))
    throw new PoolError(409, "under_moderation", { message: "This bank is under review; its versions stay until an administrator decides." });
  const now = c.now();
  const res = await c.db.batch([
    c.db
      .prepare(
        `UPDATE circle_versions SET status = 'pruned', pruned_at = ?4 WHERE bank_id = ?1 AND status = 'previous'
            AND version <= (SELECT head_version FROM circle_banks WHERE id = ?1) - ?3
            AND EXISTS (SELECT 1 FROM circle_banks tb WHERE tb.id = ?1 AND tb.circle_id = ?2 AND tb.removed_at IS NULL AND tb.purged_at IS NULL
                          AND (${ACTIVE_SQL("?2", "?5", ["owner"])} OR (tb.author_id = ?5 AND ${ACTIVE_SQL("?2", "?5", ["editor"])})))
            AND ${OPEN_SQL("?2")} AND ${ALIVE_SQL("?5")}
            AND NOT ${BLOCKING_HOLD_SQL("?1")}
            AND NOT EXISTS (SELECT 1 FROM circle_reports pr WHERE pr.bank_id = ?1 AND pr.resolved_at IS NULL AND pr.version = circle_versions.version)
            -- (round 39, critic 46a variant 3: a withheld version stays while its student-data flag is open: the admin's evidence)
            AND NOT (circle_versions.withheld_at IS NOT NULL AND EXISTS (SELECT 1 FROM admin_flags tf WHERE tf.publish_id = circle_versions.id AND tf.kind = 'privacy_text_match' AND tf.resolved_at IS NULL))`,
      )
      .bind(bid, cid, keep, now, me),
    audit(c.db, { actorId: me, actorKind: "user", action: "circle.versions_trimmed", itemId: cid, targetUserId: me, detail: { bank_id: bid, keep } }, now, `changes() > 0`),
    c.db.prepare(`DELETE FROM circle_version_blobs WHERE version_id IN (${PRUNED_NOW_SQL})`).bind(bid, now),
  ]);
  const pruned = res[0]!.meta.changes ?? 0;
  if (pruned === 0) await throwIfErased(c, me);
  const head = (await c.db.prepare(`SELECT head_version FROM circle_banks WHERE id = ?1`).bind(bid).first<{ head_version: number }>())?.head_version ?? b.head_version;
  return json(200, { ok: true, pruned, ...(await keptInfo(c, bid, head)) });
}
