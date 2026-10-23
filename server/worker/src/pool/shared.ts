// Shared plumbing for the pool routes: responses, body reading, ids, the visibility rule, daily caps,
// the storage quota, rate limits and the audit log.
import { scanJson } from "./jsonscan.ts";
import { b64url, deriveKeys, fromB64url, hmac, utf8 } from "../../../auth/src/crypto.ts";
import type { RateLimitRule, RateLimitStore } from "../../../auth/src/types.ts";
import type { Core } from "./core.ts";

export const MB = 1024 * 1024;
export const DAY_MS = 86_400_000;
export const HOUR_MS = 3_600_000;

export interface PoolPrincipal {
  userId: string;
  trust: string;
  /** The session's login family (the auth package's principal has it; the export checks its age). */
  sessionFamily?: string;
}

/** Something the pool wants a human to hear about. Wired to mail in src/index.ts. */
export type PoolNotice =
  | { kind: "student_data_report"; itemId: string; reportId: string }
  | { kind: "takedown_author"; itemId: string; authorId: string; actionId: string }
  | { kind: "key_mismatch"; keyId: string; storedKeyId: string }
  | { kind: "counter_notice"; actionId: string; complainantContact: string; restoreNotBefore: number }
  | { kind: "cap"; userId: string; metric: string }
  | { kind: "repeat_infringer"; userId: string; strikes: number }
  | { kind: "privacy_text_match"; itemId: string; held: boolean }
  | { kind: "cron_failure"; stage: string }
  /** Round 21: share-core trapped more than WASM_TRAPS_PER_HOUR times in an hour. */
  | { kind: "wasm_trap"; traps: number }
  /** Round 19 (critic24d D7): the database reached a share of D1's size limit (its own subject). */
  | { kind: "d1_size"; percent: number; bytes: number }
  /** Round 11: the author of a copy an admin action held (ids only; the reason class, never the matched text). */
  | { kind: "copy_held"; itemId: string; authorId: string; cause: "student_data" | "copyright" }
  /** Round 17: an importer reported that an item's served text gives its hidden key away. */
  | { kind: "key_leak_report"; itemId: string; authorId: string };

export interface PoolDeps {
  db: D1Database;
  /** The R2 bucket for bodies and images. Missing (not configured yet) -> every pool route answers 503. */
  r2: R2Bucket | undefined;
  core: Core;
  /** AUTH_SECRET: rate-limit keys are derived exactly as the auth package derives them, so deleting an account purges the pool's counters too. */
  secret: string;
  allowedOrigins: string[];
  /** The auth package's authenticate (revocation and current trust checked against D1). */
  authenticate(req: Request): Promise<PoolPrincipal | null>;
  now?: () => number;
  /** The auth package's D1RateLimitStore in the Worker (injected, so this module loads under node's strip-only TypeScript). */
  rateStore: RateLimitStore;
  /** The auth package's ipBucket (IPv6 grouped by /64). */
  ipBucket: (ip: string) => string;
  notify?: (n: PoolNotice) => Promise<void>;
  /** Publish body cap in bytes (default 4 MB). See README "Limits" for the measured memory per publish. */
  maxPublishBytes?: number;
  /** How long a publish body may take to arrive (default 15 s). */
  bodyDeadlineMs?: number;
  /** A streamed read nobody pulls from for this long is cancelled (default 10 s). */
  readIdleMs?: number;
  /** No streamed read lives longer than this (default 60 s). */
  readLifetimeMs?: number;
  /** How long a publish may wait for the heavy slot (default 20 s). */
  heavyQueueMs?: number;
  /** Affected items a block plan lists (default 500; more needs confirm_large). Lowered by tests. */
  affectedList?: number;
  /** Student-data band rows one publish compares (default PRIVACY_BAND_ROWS_MAX; lowered by tests). */
  privacyBandRowsMax?: number;
  /** Distinct unit keys one publish may have (default UNIT_KEYS_MAX; lowered by tests). */
  unitKeysMax?: number;
  /** Tests only: runs just before a serve is recorded (round 20 F2: interleaves a republish). */
  beforeServe?: () => Promise<void>;
  /** Browse search shortlist size (tests). */
  searchCandidates?: number;
  /** Items the round-12 backfill re-derives per cron run (tests). */
  backfillItems?: number;
  /** Account-deletion chain rewrites: rows per batch and batches per cron run (tests). */
  rewriteBatch?: number;
  rewriteBatches?: number;
  /** Runs a promise after the response (ctx.waitUntil). */
  background?: (p: Promise<unknown>) => void;
}

export interface Ctx {
  db: D1Database;
  r2: R2Bucket;
  core: Core;
  now: () => number;
  deps: PoolDeps;
  keys: Promise<{ limit: CryptoKey; cursor: CryptoKey; preview: CryptoKey }>;
  store: RateLimitStore;
}

// ---- responses -------------------------------------------------------------------------------------

const BASE: Record<string, string> = {
  "Content-Type": "application/json; charset=utf-8",
  "Cache-Control": "no-store",
  Pragma: "no-cache",
  "X-Content-Type-Options": "nosniff",
  "Referrer-Policy": "no-referrer",
  // A shared school browser or proxy must never cache one user's body for the next (ASVS V14).
  Vary: "Authorization, Cookie, Origin",
  // Round 18 (critic23d): a JSON body is never a document: nothing loads, nothing frames it.
  "Content-Security-Policy": "default-src 'none'; frame-ancestors 'none'",
};

/** What a log line says of an unexpected error: its kind, never its message (a JSON.parse message
 *  quotes the input it choked on; round 18). D1 errors keep their (input-free) code. */
export function errKind(e: unknown): string {
  if (!(e instanceof Error)) return typeof e;
  const d1 = /D1_[A-Z_]+|SQLITE_[A-Z_]+/.exec(e.message)?.[0];
  return d1 ? `${e.name} ${d1}` : e.name;
}

export function json(status: number, body: unknown, headers: Record<string, string> = {}): Response {
  return rawJson(status, JSON.stringify(body), headers);
}

/** A JSON response whose text was assembled by the caller (to embed share-core's bytes as they are). */
export function rawJson(status: number, text: string, headers: Record<string, string> = {}): Response {
  const h = new Headers(BASE);
  for (const [k, v] of Object.entries(headers)) h.set(k, v);
  return new Response(text, { status, headers: h });
}

export class PoolError extends Error {
  status: number;
  code: string;
  extra: Record<string, unknown>;
  headers: Record<string, string>;
  constructor(status: number, code: string, extra: Record<string, unknown> = {}, headers: Record<string, string> = {}) {
    super(code);
    this.status = status;
    this.code = code;
    this.extra = extra;
    this.headers = headers;
  }
}

export const notFound = () => new PoolError(404, "not_found");
export const bad = (field: string) => new PoolError(400, "invalid_request", { field });

// ---- bodies ----------------------------------------------------------------------------------------

function contentType(req: Request): string {
  return (req.headers.get("Content-Type") ?? "").split(";")[0]!.trim().toLowerCase();
}

/**
 * Reads at most `max` bytes; a longer body (declared or actual) is a 413 and reading stops there.
 * With `exact`, the body must be exactly that many bytes (its declared Content-Length): more or
 * fewer is a 400 `length_mismatch`, so nothing larger than what was reserved is ever processed.
 */
export async function readBytes(req: Request, max: number, deadlineMs?: number, exact?: number, onChunk?: (n: number) => void): Promise<Uint8Array> {
  const declared = req.headers.get("Content-Length");
  if (declared !== null && (!/^\d+$/.test(declared) || Number(declared) > max)) throw new PoolError(413, "body_too_large", { limit: max });
  if (!req.body) return new Uint8Array(0);
  const reader = req.body.getReader();
  const parts: Uint8Array[] = [];
  let total = 0;
  // A body that never finishes cannot pin anything: past the deadline the request is refused.
  const until = deadlineMs === undefined ? Infinity : Date.now() + deadlineMs;
  for (;;) {
    let timer: ReturnType<typeof setTimeout> | undefined;
    const next =
      until === Infinity
        ? reader.read()
        : Promise.race([
            reader.read(),
            new Promise<never>((_, rej) => {
              timer = setTimeout(() => rej(new PoolError(408, "body_timeout", { message: "The request body did not arrive in time." })), Math.max(0, until - Date.now()));
            }),
          ]);
    let chunk: ReadableStreamReadResult<Uint8Array>;
    try {
      chunk = await next;
    } catch (e) {
      await reader.cancel().catch(() => {});
      // A client that goes away mid-body is a clean 400 (round 9), never an unhandled 500.
      if (e instanceof PoolError) throw e;
      throw new PoolError(400, "body_aborted", { message: "The request body was cut off." });
    } finally {
      if (timer) clearTimeout(timer);
    }
    const { done, value } = chunk;
    if (done) break;
    total += value.byteLength;
    if (exact !== undefined && total > exact) {
      await reader.cancel().catch(() => {});
      throw new PoolError(400, "length_mismatch", { message: "The body is longer than its Content-Length." });
    }
    if (total > max) {
      await reader.cancel().catch(() => {});
      throw new PoolError(413, "body_too_large", { limit: max });
    }
    // Bytes are accounted as they ARRIVE (round 8 I5): a stalled body holds only what it really sent.
    if (onChunk)
      try {
        onChunk(value.byteLength);
      } catch (e) {
        await reader.cancel().catch(() => {});
        throw e;
      }
    parts.push(value);
  }
  if (exact !== undefined && total !== exact) throw new PoolError(400, "length_mismatch", { message: "The body is shorter than its Content-Length." });
  const out = new Uint8Array(total);
  let o = 0;
  for (const p of parts) {
    out.set(p, o);
    o += p.byteLength;
  }
  return out;
}

// ---- THE body reader (round 7 H3): every route that reads a request body goes through readBody ----

/** Default time a body may take to arrive. */
export const BODY_DEADLINE_DEFAULT_MS = 15_000;
/** Bodies being read at once per account (or per address before authentication). */
export const BODY_READS_PER_KEY = 4;
/** Bytes one account's bodies may hold at once, across its concurrent reads (round 8). */
export const BODY_BYTES_PER_KEY = 8 * MB;
/**
 * Unauthenticated bodies (round 8 I5, round 9 K2): their own small budgets, never the isolate's, so
 * unauthenticated traffic can never refuse a signed-in request; a tiny body cap; a 2 s deadline (an
 * 8 KB body that does not arrive at that rate is cut); and nested per-address caps (an IPv6 /64, /56,
 * /48 and /32, or an IPv4 address), so filling a pool takes many real networks, not many /64s.
 * Sign-in (login, refresh, reauth) and sign-up each have their own pool, so other unauthenticated
 * traffic cannot starve them; they are rate limited by the auth package after the read.
 */
export type UnauthPool = "general" | "signin" | "signup";
export const UNAUTH_POOLS: Record<UnauthPool, number> = { general: 1 * MB, signin: 1 * MB, signup: 512 * 1024 };
export const UNAUTH_BUDGET = UNAUTH_POOLS.general + UNAUTH_POOLS.signin + UNAUTH_POOLS.signup;
export const UNAUTH_BODY_MAX = 8 * 1024;
export const SIGNIN_BODY_MAX = 2 * 1024;
export const UNAUTH_DEADLINE_MS = 2_000;
/** Kept for callers: unauthenticated bodies at once on one /64 (the finest prefix cap). */
export const UNAUTH_READS_PER_KEY = 2;
const openBodies = new Map<string, number>();
const keyBytes = new Map<string, number>();
const unauthHeld: Record<UnauthPool, number> = { general: 0, signin: 0, signup: 0 };

/**
 * Reads a request body: its Content-Length is required (411; an absent body is allowed only when
 * `optional`) and must be at most `max` (413) and exact (400 length_mismatch); at most
 * BODY_READS_PER_KEY bodies per `key` are read at once (429); bytes are reserved AS THEY ARRIVE
 * against the isolate budget (503 busy) and against the key's BODY_BYTES_PER_KEY (429), so a stalled
 * body holds only what it sent; the body must arrive within the deadline (408). `unauth` reads use
 * their own UNAUTH_BUDGET and per-address cap, so unauthenticated traffic can never take memory from
 * signed-in work. The returned release frees everything (idempotent); on any error all is freed.
 */
export async function readBody(
  req: Request,
  o: { max: number; key: string; deadlineMs?: number; optional?: boolean; unauth?: { pool: UnauthPool; caps: { key: string; cap: number }[] } },
): Promise<{ bytes: Uint8Array; release: () => void }> {
  const declared = req.headers.get("Content-Length");
  if (declared === null) {
    if (o.optional && req.body === null) return { bytes: new Uint8Array(0), release: () => {} };
    throw new PoolError(411, "length_required", { message: "Send the body with a Content-Length." });
  }
  if (!/^\d{1,12}$/.test(declared)) throw new PoolError(400, "invalid_request", { field: "Content-Length" });
  const n = Number(declared);
  const ua = o.unauth;
  const max = ua ? Math.min(o.max, ua.pool === "signin" ? SIGNIN_BODY_MAX : UNAUTH_BODY_MAX) : o.max;
  if (n > max) throw new PoolError(413, "body_too_large", { limit: max });
  if (n === 0) return { bytes: new Uint8Array(0), release: () => {} };
  // Slots: per account; or for an unauthenticated read, at every prefix level of its address.
  const caps = ua ? ua.caps.map((x) => ({ key: `${ua.pool}|${x.key}`, cap: x.cap })) : [{ key: o.key, cap: BODY_READS_PER_KEY }];
  if (caps.some((x) => (openBodies.get(x.key) ?? 0) >= x.cap)) throw new PoolError(429, "too_many_uploads", { message: "Finish your other uploads first." }, { "Retry-After": "2" });
  for (const x of caps) openBodies.set(x.key, (openBodies.get(x.key) ?? 0) + 1);
  let held = 0;
  let done = false;
  const release = () => {
    if (done) return;
    done = true;
    if (ua) unauthHeld[ua.pool] -= held;
    else bytesInFlight -= held;
    const kb = (keyBytes.get(o.key) ?? held) - held;
    if (kb <= 0) keyBytes.delete(o.key);
    else keyBytes.set(o.key, kb);
    for (const x of caps) {
      const k = (openBodies.get(x.key) ?? 1) - 1;
      if (k <= 0) openBodies.delete(x.key);
      else openBodies.set(x.key, k);
    }
  };
  const grow = (k: number) => {
    if ((keyBytes.get(o.key) ?? 0) + k > BODY_BYTES_PER_KEY) throw new PoolError(429, "too_many_uploads", { message: "Your uploads in progress are too large together; finish one first." }, { "Retry-After": "2" });
    if (ua) {
      if (unauthHeld[ua.pool] + k > UNAUTH_POOLS[ua.pool]) throw new PoolError(503, "busy", { message: "The server is busy; try again shortly." }, { "Retry-After": "5" });
      unauthHeld[ua.pool] += k;
    } else {
      if (bytesInFlight + k > ISOLATE_BYTES_BUDGET) throw new PoolError(503, "busy", { message: "The server is busy; try again shortly." }, { "Retry-After": "5" });
      bytesInFlight += k;
    }
    held += k;
    keyBytes.set(o.key, (keyBytes.get(o.key) ?? 0) + k);
  };
  try {
    const out = await readBytes(req, max, ua ? Math.min(o.deadlineMs ?? UNAUTH_DEADLINE_MS, UNAUTH_DEADLINE_MS) : o.deadlineMs ?? BODY_DEADLINE_DEFAULT_MS, n, grow);
    return { bytes: out, release };
  } catch (e) {
    release();
    throw e;
  }
}

export function unauthReserved(pool?: UnauthPool): number {
  return pool ? unauthHeld[pool] : unauthHeld.general + unauthHeld.signin + unauthHeld.signup;
}

/** A small JSON object body (`application/json` only, so an HTML form cannot send it cross-site), read by readBody. */
export async function readJson(req: Request, key: string, o: { max?: number; deadlineMs?: number; optional?: boolean } = {}): Promise<Record<string, unknown>> {
  // No content type and no (or a zero) length: no body, whatever the runtime puts in req.body (workerd
  // gives a bodiless POST an empty stream). Nothing is read.
  // Round 9: an empty body (Content-Length 0) declared as JSON is no body too, on a route whose body is optional.
  // Round 10: on a route whose body is optional, no body is no body: Content-Length 0 (whatever the
  // Content-Type a runtime added), or neither a length nor a type (a bare fetch POST). 415/411 only
  // when a body is really sent.
  const cl = req.headers.get("Content-Length");
  if (o.optional && (cl === "0" || (cl === null && (req.headers.get("Content-Type") === null || req.body === null)))) return {};
  if (contentType(req) !== "application/json") throw new PoolError(415, "unsupported_media_type");
  const { bytes: b, release } = await readBody(req, { max: o.max ?? JSON_BODY_MAX, key, deadlineMs: o.deadlineMs });
  try {
    // Round 19 (critic24d D5): duplicate keys (JSON.parse keeps the last) and the other structural
    // limits are refused before the parse, as for a publish payload.
    const sc = scanJson(b);
    if (!sc.ok) throw new PoolError(400, "invalid_json", { reason: sc.code });
    let v: unknown;
    try {
      v = JSON.parse(new TextDecoder("utf-8", { fatal: true, ignoreBOM: false }).decode(b));
    } catch {
      throw new PoolError(400, "invalid_json");
    }
    if (typeof v !== "object" || v === null || Array.isArray(v)) throw new PoolError(400, "invalid_json");
    return v as Record<string, unknown>;
  } finally {
    release();
  }
}
/** JSON route bodies at most (reports, appeals, prefs, versions, counter-notices). */
export const JSON_BODY_MAX = 64 * 1024;

/** An optional small JSON body: none at all is `{}`. */
export async function readOptionalJson(req: Request, key: string, deadlineMs?: number): Promise<Record<string, unknown>> {
  return readJson(req, key, { optional: true, deadlineMs });
}

export const PUBLISH_CONTENT_TYPE = "application/vnd.vershion.publish";

/**
 * A publish request: `<meta JSON>\n<payload bytes>` (already read by readBody). The Worker DOES read
 * the payload before share-core (publish.ts: to rewrite ids, dates, its own provenance and image
 * references, then re-serialise it), so the raw bytes are first held to share-core's structural
 * rules by jsonscan.ts (depth, counts, duplicate keys, a BOM or anything but JSON tokens refused);
 * a payload that fails them reaches share-core unparsed, whose refusal is the answer.
 */
export function parsePublish(raw: Uint8Array, maxPayload: number, maxMeta: number): { raw: Uint8Array; meta: Record<string, unknown>; payload: Uint8Array } {
  const nl = raw.indexOf(0x0a);
  if (nl < 0 || nl > maxMeta) throw new PoolError(400, "invalid_request", { field: "meta" });
  if (raw.length - nl - 1 > maxPayload) throw new PoolError(413, "body_too_large", { limit: maxPayload });
  let meta: unknown;
  const sc = scanJson(raw.subarray(0, nl));
  if (!sc.ok) throw new PoolError(400, "invalid_json", { field: "meta", reason: sc.code });
  try {
    meta = JSON.parse(new TextDecoder("utf-8", { fatal: true, ignoreBOM: false }).decode(raw.subarray(0, nl)));
  } catch {
    throw new PoolError(400, "invalid_json", { field: "meta" });
  }
  if (typeof meta !== "object" || meta === null || Array.isArray(meta)) throw new PoolError(400, "invalid_json", { field: "meta" });
  return { raw, meta: meta as Record<string, unknown>, payload: raw.subarray(nl + 1) };
}

export function isPublishType(req: Request): boolean {
  return contentType(req) === PUBLISH_CONTENT_TYPE;
}

// ---- ids and hashes --------------------------------------------------------------------------------

/** 128 random bits, hex: item ids, qids, R2 key parts, report ids. */
export function newId(): string {
  const b = crypto.getRandomValues(new Uint8Array(16));
  return [...b].map((x) => x.toString(16).padStart(2, "0")).join("");
}
export const ID_RE = /^[0-9a-f]{32}$/;
/** Client-generated publish ids: at least 128 bits of base64url (or hex). */
export const PUBLISH_ID_RE = /^[A-Za-z0-9_-]{22,64}$/;
export const SHA_RE = /^[0-9a-f]{64}$/;

export async function sha256Hex(b: Uint8Array): Promise<string> {
  const d = new Uint8Array(await crypto.subtle.digest("SHA-256", b));
  return [...d].map((x) => x.toString(16).padStart(2, "0")).join("");
}

export function dayOf(ms: number): number {
  return Math.floor(ms / DAY_MS);
}

// ---- visibility ------------------------------------------------------------------------------------

/** The author's standing (docs/shared-pool.md "Content follows the author's standing"). `u` = the author's users row. */
export const STANDING = `u.trust IN ('teacher', 'admin') AND u.suspended_at IS NULL AND u.deleted_at IS NULL`;

/** Visible to someone other than its author: public, committed, not removed, not held, author in good standing. */
export const VISIBLE_TO_OTHERS = `i.visibility = 'public' AND i.current_publish_id IS NOT NULL AND i.removed_at IS NULL
  AND NOT EXISTS (SELECT 1 FROM item_holds h WHERE h.item_id = i.id)
  AND ((${STANDING}) OR (i.anonymized = 1 AND u.deleted_at IS NOT NULL AND u.trust IN ('teacher', 'admin') AND u.suspended_at IS NULL))`;

/** Visible to the caller bound at `me` (a parameter reference such as `?2`). */
export const visibleTo = (me: string) =>
  `((i.author_id = ${me} AND i.removed_at IS NULL AND i.current_publish_id IS NOT NULL) OR (i.author_id <> ${me} AND ${VISIBLE_TO_OTHERS}))`;

// ---- limits ----------------------------------------------------------------------------------------

/** Defaults (docs/shared-pool.md "Limits"); each can be raised per user or site-wide by an admin. */
export const DEFAULT_LIMITS = {
  publishes: 50,
  // (Round 18, critic23d V4: import evidence is never evicted, so imports are bounded per day.)
  imports: 100,
  item_reads: 2000,
  browse: 500,
  blob_uploads: 500,
  blob_bytes: 200 * MB,
  blob_miss: 100,
  item_miss: 100,
  reports: 20,
  storage_bytes: 200 * MB,
  shared_questions: 5000,
  report_threshold: 3,
  /** Bytes of removed items not purged yet (they leave the quota at once, but their bodies stay 14 days). */
  removed_bytes: 200 * MB,
  /** Refused publishes per day: each costs the server a full validation. */
  publish_refusals: 300,
  /** A question or image used by at least this many OTHER authors is never blocked unless selected. */
  block_common_k: 3,
  /** Appeals per account per day. */
  appeals: 10,
  /** Account exports (GET /me/export) per account per day (round 23). */
  exports: 3,
  /** Instant student-data holds per day across all accounts (then they wait for the admin). */
  instant_holds_global: 50,
  /** Dry-run publishes (POST /items/preview) per account per day (round 17). */
  previews: 200,
  /** Unit lookups the dry runs of one day may make (round 20 N4: preview CPU, weighted by bank size). */
  preview_units: 2_000_000,
  /** Distinct publishes an account may be served in all (round 19: one tiny `served` row each; past it,
   *  reads of publishes not served before are refused; nothing is ever evicted). */
  reads_max: 200_000,
  /** Distinct items an account may import in all (round 19; besides the daily `imports`). */
  imports_max: 20_000,
  /** Durable authorship records an account keeps (rounds 18-19): only NEW records count; never a
   *  republish adding nothing new, never a fix of a held item. */
  author_records: 500_000,
} as const;
export type Metric = keyof typeof DEFAULT_LIMITS;
export const METRICS = Object.keys(DEFAULT_LIMITS) as Metric[];

/**
 * Round 27 (critic35d B-1): the guard that lives IN a write. A request authenticated before its account
 * was erased keeps running; every row it then writes under the caller's id must be conditioned, in the
 * SAME statement (or the same atomic batch), on the account not being erased, or the erasure sweep
 * (which ran once) never sees it. `p` is the SQL for the caller's user id.
 */
export const ALIVE_SQL = (p: string): string => `NOT EXISTS (SELECT 1 FROM users du WHERE du.id = ${p} AND du.deleted_at IS NOT NULL)`;
/** What an unauthenticated request gets (the answer for a write skipped because the account is erased). */
export function unauthorized(): PoolError {
  return new PoolError(401, "unauthorized", {}, { "WWW-Authenticate": 'Bearer error="invalid_token"' });
}
/** True when the account's tombstone has committed. */
export async function isErased(c: Ctx, userId: string): Promise<boolean> {
  return (await c.db.prepare(`SELECT 1 AS x FROM users WHERE id = ?1 AND deleted_at IS NOT NULL`).bind(userId).first()) !== null;
}
/** A write whose guarded statement changed nothing: 401 when the reason is that the account is erased. */
export async function throwIfErased(c: Ctx, userId: string): Promise<void> {
  if (await isErased(c, userId)) throw unauthorized();
}

export async function limitOf(c: Ctx, userId: string, metric: Metric): Promise<number> {
  const r = await c.db
    .prepare(`SELECT value FROM pool_limits WHERE metric = ?1 AND user_id IN (?2, '*') ORDER BY user_id = '*' LIMIT 1`)
    .bind(metric, userId)
    .first<{ value: number }>();
  return r ? r.value : DEFAULT_LIMITS[metric];
}

/**
 * Takes `n` from today's allowance of `metric`; false (and nothing taken) when it would cross the
 * cap. One atomic UPSERT, so concurrent requests cannot overshoot.
 */
export async function takeDaily(c: Ctx, userId: string, metric: Metric, n = 1): Promise<boolean> {
  const cap = await limitOf(c, userId, metric);
  const day = dayOf(c.now());
  const r = await c.db
    .prepare(
      `INSERT INTO usage_daily (user_id, day, metric, count) SELECT ?1, ?2, ?3, ?4 WHERE ?4 <= ?5 AND ${ALIVE_SQL("?1")}
       ON CONFLICT (user_id, day, metric) DO UPDATE SET count = count + excluded.count WHERE count + excluded.count <= ?5
       RETURNING count`,
    )
    .bind(userId, day, metric, n, cap)
    .first<{ count: number }>();
  if (r) return true;
  await throwIfErased(c, userId); // (round 27: not a cap hit: the account is gone)
  await capAlert(c, userId, metric);
  return false;
}

/** Round 22 (critic29b): gives back what a request took and did not use (an import that failed). */
export async function giveDaily(c: Ctx, userId: string, metric: Metric, n = 1): Promise<void> {
  await c.db.prepare(`UPDATE usage_daily SET count = MAX(0, count - ?4) WHERE user_id = ?1 AND day = ?2 AND metric = ?3`).bind(userId, dayOf(c.now()), metric, n).run();
}
/** Throws 429 when today's allowance of `metric` is used up. */
export async function requireDaily(c: Ctx, userId: string, metric: Metric, n = 1): Promise<void> {
  if (!(await takeDaily(c, userId, metric, n))) throw new PoolError(429, "daily_limit", { metric }, { "Retry-After": String(secondsToMidnight(c.now())) });
}

function secondsToMidnight(now: number): number {
  return Math.max(1, Math.ceil(((dayOf(now) + 1) * DAY_MS - now) / 1000));
}

/** Counts a miss (404) of `metric` (item_miss / blob_miss). */
export async function countMiss(c: Ctx, userId: string, metric: "item_miss" | "blob_miss" | "publish_refusals"): Promise<void> {
  const r = await c.db
    .prepare(
      `INSERT INTO usage_daily (user_id, day, metric, count) SELECT ?1, ?2, ?3, 1 WHERE ${ALIVE_SQL("?1")}
       ON CONFLICT (user_id, day, metric) DO UPDATE SET count = count + 1`,
    )
    .bind(userId, dayOf(c.now()), metric)
    .run();
  if ((r.meta.changes ?? 0) === 0) await throwIfErased(c, userId);
}

// ---- memory: one byte budget per isolate (reads and publishes together, round 5 E6) ---------------

/**
 * JS bytes the isolate may commit to requests at once: 128 MB isolate - 30 MB for the share-core
 * wasm memory (one instance, shared by publishes and Argon2 login hashing; measured 20-21 MB after a
 * 3.8-3.9 MB bank or after one Argon2 hash, 29.4 MB on the critic's Matching bank; wasm memory never
 * shrinks) - about 20 MB of runtime baseline. 78 + 30 + 20 = 128 left no headroom (round 9): 72 MB
 * until a staging measurement on the final share-core build says otherwise.
 */
export const ISOLATE_BYTES_BUDGET = 72 * MB;
/** A publish reserves this many bytes per body byte: measured JS heap + buffers (wasm excluded) of a 3.9 MB hidden-key bank is about 62 MB server-side, 16x. */
export const PUBLISH_COST_PER_BYTE = 16;
/** What a streamed read costs at least (chunks in flight, the head): it never holds the whole body. */
export const READ_COST = 1 * MB;
/** A streamed read also reserves this many bytes per byte of its JSON head (the per-question list grows with the bank). */
export const READ_HEAD_COST = 3;
let bytesInFlight = 0;

/** Reserves `n` bytes of the isolate budget, or 503 busy; returns the release (idempotent). */
export function reserveBytes(n: number): () => void {
  if (bytesInFlight + n > ISOLATE_BYTES_BUDGET) throw new PoolError(503, "busy", { message: "The server is busy; try again shortly." }, { "Retry-After": "5" });
  bytesInFlight += n;
  let done = false;
  return () => {
    if (done) return;
    done = true;
    bytesInFlight -= n;
  };
}
export function bytesReserved(): number {
  return bytesInFlight;
}

/** Takes back one counted miss (a reserved refusal that turned out not to be one). */
export async function refundMiss(c: Ctx, userId: string, metric: "publish_refusals"): Promise<void> {
  await c.db.prepare(`UPDATE usage_daily SET count = count - 1 WHERE user_id = ?1 AND day = ?2 AND metric = ?3 AND count > 0`).bind(userId, dayOf(c.now()), metric).run();
}

/** Past the day's miss allowance, every id-taking route answers 429 for the rest of the day. */
export async function missGuard(c: Ctx, userId: string, metric: "item_miss" | "blob_miss" | "publish_refusals"): Promise<void> {
  const r = await c.db
    .prepare(`SELECT count FROM usage_daily WHERE user_id = ?1 AND day = ?2 AND metric = ?3`)
    .bind(userId, dayOf(c.now()), metric)
    .first<{ count: number }>();
  if (r && r.count >= (await limitOf(c, userId, metric))) {
    await capAlert(c, userId, metric);
    throw new PoolError(429, "daily_limit", { metric }, { "Retry-After": String(secondsToMidnight(c.now())) });
  }
}

/** Positions / items listed in one flag's detail (the rest is a count). */
export const FLAG_DETAIL_MAX = 5;
/**
 * Flags per account, KIND and day kept as their own rows (round 9 K3); past it, the kind folds into
 * one aggregated row with a count (never dropped). Integrity kinds are never folded, so no burst of
 * one kind can silence another; origin_unverified always folds (one row a day with a count).
 */
export const FLAGS_PER_ACCOUNT_DAY = 20;
export const INTEGRITY_FLAGS: ReadonlySet<string> = new Set([
  "independent_claim",
  "independent_repeat",
  "license_soft",
  "license_conflict",
  "license_grandfathered",
  "blocklist",
  "privacy_text_match",
  "evidence_overflow",
  "origin_suspect",
  "origin_modified",
]);
export const COLLAPSED_FLAGS: ReadonlySet<string> = new Set(["origin_unverified"]);
/** Admin flag severity (0 = most urgent): the queue is ordered by it, then newest first. */
export const FLAG_SEVERITY: Record<string, number> = {
  privacy_text_match: 0,
  instant_hold_cap: 1,
  privacy_backlog: 1,
  cron_failure: 1,
  repeat_infringer: 1,
  blocklist: 1,
  dmca_copy: 1,
  appeal_escalated: 0,
  appeal_overdue: 1,
  appeal_auto_restored: 2,
  hold_cleared: 2,
  license_mismatch: 1,
  counter_notice: 1,
  needs_reupload: 2,
  appeal: 2,
  independent_repeat: 2,
  license_conflict: 2,
  license_soft: 2,
  cap: 3,
  independent_claim: 3,
  evidence_overflow: 3,
  license_grandfathered: 3,
  origin_unverified: 4,
  origin_modified: 4,
  origin_suspect: 3,
};
export const severityOf = (kind: string) => FLAG_SEVERITY[kind] ?? 3;
export const FLAG_SEVERITY_SQL = `CASE kind ${Object.entries(FLAG_SEVERITY).map(([k, v]) => `WHEN '${k}' THEN ${v}`).join(" ")} ELSE 3 END`;

/** One admin flag (and at most a few mails an hour) per user, day and metric. */
export async function capAlert(c: Ctx, userId: string, metric: string): Promise<void> {
  const now = c.now();
  const dedup = `cap:${userId}:${dayOf(now)}:${metric}`;
  const [ins] = await c.db.batch([
    c.db
      .prepare(`INSERT OR IGNORE INTO admin_flags (id, kind, user_id, detail, dedup, at) VALUES (?1, 'cap', ?2, ?3, ?4, ?5)`)
      .bind(newId(), userId, JSON.stringify({ metric }), dedup, now),
    audit(c.db, { actorKind: "system", action: "cap_reached", targetUserId: userId, detail: { metric } }, now, `changes() = 1`),
  ]);
  if ((ins!.meta.changes ?? 0) === 1) await alertBudgeted(c, { kind: "cap", userId, metric });
}

export const ALERTS_PER_HOUR = 5;

/** Admin alert mails (cap crossings and the like), at most ALERTS_PER_HOUR; the rest stay in admin_flags. */
export async function alertBudgeted(c: Ctx, n: PoolNotice): Promise<void> {
  if (!c.deps.notify) return;
  // Separate channels (round 8 I6c): a reported student-data hold, a re-publish of blocked text, and
  // everything else each have their own hourly budget, so no flood of one swallows the others.
  // Round 12: one hourly budget per notice kind (the old channel names kept for the stored rows),
  // so no flood of one kind ever swallows another.
  // (Round 18: key_leak notices have their own author budget, never sharing copy_held's.)
  const channel = n.kind === "student_data_report" ? "privacy_report" : n.kind === "privacy_text_match" ? "privacy_match" : n.kind === "copy_held" ? "authors" : n.kind === "key_leak_report" ? "authors_key_leak" : n.kind;
  const budget = channel === "privacy_report" ? PRIVACY_ALERTS_PER_HOUR : channel === "authors" || channel === "authors_key_leak" ? AUTHOR_NOTICES_PER_HOUR : ALERTS_PER_HOUR;
  const row = await c.db
    .prepare(`INSERT INTO pool_alert_budget (hour, channel, sent) VALUES (?1, ?2, 1) ON CONFLICT (hour, channel) DO UPDATE SET sent = sent + 1 RETURNING sent`)
    .bind(Math.floor(c.now() / HOUR_MS), channel)
    .first<{ sent: number }>();
  if ((row?.sent ?? 1) <= budget) send(c, n);
}
/** Held-copy notices to authors an hour (their own channel; the rest see it in "My shared items"). */
export const AUTHOR_NOTICES_PER_HOUR = 200;

/** Tells the authors of these items that an admin action held them (round 11). */
export async function notifyHeld(c: Ctx, itemIds: string[], cause: "student_data" | "copyright"): Promise<void> {
  if (!c.deps.notify || itemIds.length === 0) return;
  // (Round 21: every item, in chunks; the hourly budget decides what is mailed, the rest is listed.)
  for (const ch of chunks(itemIds, 500)) {
    const r = await c.db
      .prepare(`SELECT i.id, i.author_id FROM items i JOIN users u ON u.id = i.author_id WHERE i.id IN (SELECT value FROM json_each(?1)) AND u.deleted_at IS NULL`)
      .bind(JSON.stringify(ch))
      .all<{ id: string; author_id: string }>();
    for (const x of r.results) await alertBudgeted(c, { kind: "copy_held", itemId: x.id, authorId: x.author_id, cause });
  }
}
/** Student-data report mails an hour (their own channel). */
export const PRIVACY_ALERTS_PER_HOUR = 20;

/**
 * Round 23 (critic30b F2/F3): a notice that must not be lost (the author's takedown notice, whose
 * counter-notice window runs from it) is written to `pool_outbox` in the SAME batch as the action it
 * reports (`id` deterministic: a retry writes it once), then sent by flushOutbox (after the request's
 * work, and hourly by the cron until it is sent). `cond`: SQL that must hold for it to be written.
 */
export function outboxStatement(db: D1Database, id: string, n: PoolNotice, now: number, cond = "1"): D1PreparedStatement {
  return db.prepare(`INSERT OR IGNORE INTO pool_outbox (id, kind, payload, at) SELECT ?1, ?2, ?3, ?4 WHERE ${cond}`).bind(id, n.kind, JSON.stringify(n), now);
}
/** Sends unsent outbox notices (these ids, or every unsent one), marking each sent once it went. */
export async function flushOutbox(c: Ctx, ids?: string[]): Promise<number> {
  const rows = ids
    ? await c.db.prepare(`SELECT id, payload FROM pool_outbox WHERE sent_at IS NULL AND id IN (SELECT value FROM json_each(?1))`).bind(JSON.stringify(ids)).all<{ id: string; payload: string }>()
    : await c.db.prepare(`SELECT id, payload FROM pool_outbox WHERE sent_at IS NULL ORDER BY at LIMIT 200`).all<{ id: string; payload: string }>();
  let n = 0;
  for (const r of rows.results) {
    try {
      if (c.deps.notify) await c.deps.notify(JSON.parse(r.payload) as PoolNotice);
      await c.db.prepare(`UPDATE pool_outbox SET sent_at = ?2 WHERE id = ?1 AND sent_at IS NULL`).bind(r.id, c.now()).run();
      n++;
    } catch (e) {
      console.error(`pool: outbox ${r.id.split(":")[0]} failed: ${e instanceof Error ? e.name : typeof e}`);
    }
  }
  return n;
}
/** Sends a notice in the background (a failure never fails the request). */
export function send(c: Ctx, n: PoolNotice): void {
  if (!c.deps.notify) return;
  const p = c.deps.notify(n).catch((e) => console.error(`pool: notice ${n.kind} failed: ${String(e)}`));
  if (c.deps.background) c.deps.background(p);
}

// ---- storage quota ---------------------------------------------------------------------------------

/**
 * Bytes charged to `?1` at time `?2`: the distinct blobs referenced by the retained (and staged)
 * publishes of their items that are not removed, plus their live pending uploads, the body bytes of
 * those publishes, and 1 KB per item not removed (removal releases the charge, as the spec says). Computed, not counted, so it cannot drift; one statement, so it is consistent.
 */
export function usageSql(include: string | null = null): string {
  // `include`: an item id (parameter reference) counted as live although removed (the undelete check).
  const live = (alias: string) => `NOT EXISTS (SELECT 1 FROM items it WHERE it.id = ${alias}.item_id AND it.removed_at IS NOT NULL${include ? ` AND it.id <> ${include}` : ""})`;
  return `(
  COALESCE((SELECT SUM(size) FROM (
      SELECT ib.sha256, b.size FROM item_blobs ib
        JOIN item_publishes p ON p.publish_id = ib.publish_id
        JOIN blobs b ON b.sha256 = ib.sha256
       WHERE p.author_id = ?1 AND ${live("p")}
      UNION
      SELECT sha256, size FROM pending_blobs WHERE uploader_id = ?1 AND until >= ?2)), 0)
  + COALESCE((SELECT SUM(p.full_bytes + p.stripped_bytes) FROM item_publishes p WHERE p.author_id = ?1 AND ${live("p")}), 0)
  + 1024 * (SELECT COUNT(*) FROM items WHERE author_id = ?1 AND (removed_at IS NULL${include ? ` OR id = ${include}` : ""}))
)`;
}
export const USAGE_SQL = usageSql();

/** Body and image bytes still held by `?1`'s removed-but-unpurged items (storage amplification bound). */
export const REMOVED_SQL = `(
  COALESCE((SELECT SUM(p.full_bytes + p.stripped_bytes) FROM item_publishes p JOIN items it ON it.id = p.item_id
             WHERE p.author_id = ?1 AND it.removed_at IS NOT NULL AND it.purged_at IS NULL), 0)
  -- Round 17 (critic22e): and the images only those items keep (each once).
  + COALESCE((SELECT SUM(size) FROM (SELECT DISTINCT ib.sha256, b.size FROM item_blobs ib JOIN item_publishes p ON p.publish_id = ib.publish_id JOIN items it ON it.id = p.item_id JOIN blobs b ON b.sha256 = ib.sha256
             WHERE p.author_id = ?1 AND it.removed_at IS NOT NULL AND it.purged_at IS NULL)), 0))`;

/**
 * Questions shared by `me` across items other than `except`: committed live items, plus every
 * other item's publish still in flight (staging/staged): a new item's whole count, a live item's
 * republish only what it adds (MAX(0, new - current), critic C3).
 */
export const SHARED_SQL = (me: string, except: string) => `(
  (SELECT COALESCE(SUM(question_count), 0) FROM items WHERE author_id = ${me} AND removed_at IS NULL AND id <> ${except})
  + (SELECT COALESCE(SUM(question_count), 0) FROM item_publishes WHERE author_id = ${me} AND status IN ('staging', 'staged') AND item_id <> ${except}
       AND NOT EXISTS (SELECT 1 FROM items x WHERE x.id = item_publishes.item_id))
  + (SELECT COALESCE(SUM(MAX(0, p.question_count - x.question_count)), 0) FROM item_publishes p JOIN items x ON x.id = p.item_id
      WHERE p.author_id = ${me} AND p.status IN ('staging', 'staged') AND p.item_id <> ${except} AND x.removed_at IS NULL))`;

export async function usage(c: Ctx, userId: string): Promise<number> {
  const r = await c.db.prepare(`SELECT ${USAGE_SQL} AS used`).bind(userId, c.now()).first<{ used: number }>();
  return r?.used ?? 0;
}

// ---- rate limits -----------------------------------------------------------------------------------

/** Burst limits on every pool route, per account and per client IP (the daily caps come on top). */
export const RATE = {
  account: [{ limit: 300, windowSeconds: 300 }] as RateLimitRule[],
  /**
   * Per address (round 9): 20 account-shares (20 x the account limit), so a school's NAT with many
   * teachers syncing is not cut off at a handful; and only requests that passed their account's
   * limit are counted against it.
   */
  ip: [{ limit: 6000, windowSeconds: 300 }] as RateLimitRule[],
  /** Read routes, per client IP per day: generous, because a whole school can share one address. */
  ipReadDaily: [{ limit: 50_000, windowSeconds: 86_400 }] as RateLimitRule[],
  /** One account's share of an address's daily read budget (round 8): 20%. */
  accountIpReadDaily: [{ limit: 10_000, windowSeconds: 86_400 }] as RateLimitRule[],
};

export async function rateLimit(c: Ctx, req: Request, userId: string, read = false): Promise<void> {
  const ip = req.headers.get("CF-Connecting-IP");
  if (!ip || !ip.trim()) throw new PoolError(403, "client_ip_required");
  const { limit } = await c.keys;
  const now = c.now();
  // Same key scheme as the auth package (`a:<hmac(user:<id>)>:`), so account deletion purges these too.
  const acct = `a:${await hmac(limit, `user:${userId}`)}:pool:`;
  const ipH = `i:${await hmac(limit, c.deps.ipBucket(ip.trim()))}:pool:`;
  // The account's own limits first; the address's are charged only for a request that passed them
  // (round 9), so one account over its limit never spends its address's budget.
  const stages = [
    [
      ...RATE.account.map((r) => ({ key: `${acct}${r.windowSeconds}`, rule: r })),
      ...(read ? RATE.accountIpReadDaily.map((r) => ({ key: `${acct}${ipH}read:${r.windowSeconds}`, rule: r })) : []),
    ],
    [...RATE.ip.map((r) => ({ key: `${ipH}${r.windowSeconds}`, rule: r })), ...(read ? RATE.ipReadDaily.map((r) => ({ key: `${ipH}read:${r.windowSeconds}`, rule: r })) : [])],
  ];
  const charged: { key: string; rule: RateLimitRule }[] = [];
  for (const hits of stages) {
    const res = await Promise.all(hits.map((h) => c.store.hit(h.key, h.rule, now)));
    charged.push(...hits);
    const denied = res.filter((r) => !r.allowed);
    if (denied.length > 0) {
      if (c.store.undo) await Promise.all(charged.map((h) => c.store.undo!(h.key, h.rule, now)));
      throw new PoolError(429, "rate_limited", {}, { "Retry-After": String(Math.max(...denied.map((d) => d.retryAfter))) });
    }
  }
}

export function makeCtx(d: PoolDeps, r2: R2Bucket): Ctx {
  return {
    db: d.db,
    r2,
    core: d.core,
    now: d.now ?? Date.now,
    deps: d,
    store: d.rateStore,
    keys: (async () => {
      const k = await deriveKeys(d.secret);
      const ikm = await crypto.subtle.importKey("raw", utf8(d.secret), "HKDF", false, ["deriveKey"]);
      const cursor = await crypto.subtle.deriveKey(
        { name: "HKDF", hash: "SHA-256", salt: utf8("vershion-pool/v1"), info: utf8("browse-cursor") },
        ikm,
        { name: "HMAC", hash: "SHA-256", length: 256 },
        false,
        ["sign", "verify"],
      );
      // Preview tokens get their own HKDF purpose (never interchangeable with a cursor).
      const preview = await crypto.subtle.deriveKey(
        { name: "HKDF", hash: "SHA-256", salt: utf8("vershion-pool/v1"), info: utf8("block-preview") },
        ikm,
        { name: "HMAC", hash: "SHA-256", length: 256 },
        false,
        ["sign", "verify"],
      );
      return { limit: k.limit, cursor, preview };
    })(),
  };
}

// ---- cursors ---------------------------------------------------------------------------------------

export async function signCursor(c: Ctx, at: number, id: string): Promise<string> {
  const body = b64url(utf8(JSON.stringify([at, id])));
  return `${body}.${await hmac((await c.keys).cursor, body)}`;
}

export async function readCursor(c: Ctx, s: string): Promise<[number, string] | null> {
  const m = /^([A-Za-z0-9_-]{1,200})\.([A-Za-z0-9_-]{43})$/.exec(s);
  if (!m) return null;
  const ok = await crypto.subtle.verify("HMAC", (await c.keys).cursor, fromB64url(m[2]!), utf8(m[1]!));
  if (!ok) return null;
  try {
    const v = JSON.parse(new TextDecoder().decode(fromB64url(m[1]!))) as unknown;
    if (Array.isArray(v) && typeof v[0] === "number" && typeof v[1] === "string" && ID_RE.test(v[1])) return [v[0], v[1]];
  } catch {
    /* refused below */
  }
  return null;
}

// ---- audit -----------------------------------------------------------------------------------------

export interface AuditEntry {
  actorId?: string | null;
  actorKind: "user" | "admin" | "system";
  action: string;
  itemId?: string | null;
  targetUserId?: string | null;
  detail?: Record<string, unknown> | null;
  note?: string | null;
}

/** A pool_audit insert. `when` adds a SQL condition (for "only if this batch won"). */
export function audit(db: D1Database, e: AuditEntry, now: number, when?: string): D1PreparedStatement {
  return db
    .prepare(
      // Round 23 (critic30b T1): an ADMIN's note is kept as evidence after the subject's erasure, so
      // the subject's personal data is scrubbed from it when written: the display name and email of the
      // target account (or of the item's author) become the account id.
      `INSERT INTO pool_audit (id, at, actor_id, actor_kind, action, item_id, target_user_id, detail, note)
       SELECT ?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8,
              CASE WHEN ?9 IS NULL OR ?4 <> 'admin' THEN ?9 ELSE (
                SELECT COALESCE(
                         (SELECT replace(replace(?9, CASE WHEN length(COALESCE(su.display_name, '')) >= 3 THEN su.display_name ELSE char(1) END, '[user ' || su.id || ']'),
                                         CASE WHEN length(COALESCE(su.email, '')) >= 3 THEN su.email ELSE char(1) END, '[user ' || su.id || ']')
                            FROM users su WHERE su.id = COALESCE(?7, (SELECT author_id FROM items WHERE id = ?6))), ?9)) END
       ${when ? `WHERE ${when}` : ""}`,
    )
    .bind(newId(), now, e.actorId ?? null, e.actorKind, e.action, e.itemId ?? null, e.targetUserId ?? null, e.detail ? JSON.stringify(e.detail) : null, e.note ?? null);
}

/** Splits `xs` into chunks of `n`. */
export function chunks<T>(xs: T[], n: number): T[][] {
  const out: T[][] = [];
  for (let i = 0; i < xs.length; i += n) out.push(xs.slice(i, i + n));
  return out;
}

/**
 * SQL (round 12): the chain JSON `expr` with the name of every link to a DELETED account's item
 * replaced by "deleted teacher", applied whenever a chain is stored, so a stored row never brings a
 * deleted name back (the cron's batch rewrite then only cleans what was stored before the deletion).
 */
export const anonChain = (expr: string) =>
  `(SELECT COALESCE(json_group_array(CASE WHEN EXISTS (SELECT 1 FROM items ax JOIN users au ON au.id = ax.author_id WHERE ax.id = al.value ->> '$.item_id' AND au.deleted_at IS NOT NULL)
                                          THEN json_set(al.value, '$.author_name', 'deleted teacher') ELSE json(al.value) END), '[]')
      FROM json_each(COALESCE(${expr}, '[]')) al)`;

// ---- block keys (round 12 M4 / M6) ------------------------------------------------------------------

/**
 * Every key a privacy or copyright block can name for a question: the exact match_hash (as before),
 * its normalised skeleton (`n:`), the normalised full content (`f:`), the normalised explanation
 * (`e:`) and the normalised model / accepted answers (`a:`). A block row of kind 'match' holds any of
 * them; these helpers are the ONE comparison every hold, release, plan and check uses.
 */
export const QKEY_COLS = ["match_hash", "bk_norm", "bk_stem", "bk_full", "bk_expl", "bk_ans"] as const;
/** SQL: the block keys of every question of publish `pub` (a subquery usable after IN). */
// (No compound SELECT: D1 limits compound terms per statement, round 14 workerd finding.)
// Round 16 (critic21c B5): a Multipart question also has the block keys of each PART (its stem `s:`,
// explanation `e:` and answer `a:` keys, item_part_keys), so student data in a part is held as a
// standalone question and inside any other Multipart, and the other way round.
// (No compound SELECT: D1 limits compound terms per statement, round 14 workerd finding.)
export const qKeysOf = (pub: string) =>
  `(SELECT qk.value FROM item_questions qkq, json_each(json_array(${QKEY_COLS.map((k) => `qkq.${k}`).join(", ")})) qk WHERE qkq.publish_id = ${pub} AND qk.value IS NOT NULL)`;
/** SQL: `x` is a block key of publish `pub` (its questions' keys or their parts'). */
export const inQKeysOf = (x: string, pub: string) => `(${x} IN ${qKeysOf(pub)} OR ${x} IN (SELECT pk.key FROM item_part_keys pk WHERE pk.publish_id = ${pub}))`;
/** SQL: question row `iq` has a key in `list` (a parenthesised subquery or list). */
export const qKeyIn = (iq: string, list: string) =>
  `(${QKEY_COLS.map((k) => `${iq}.${k} IN ${list}`).join(" OR ")} OR EXISTS (SELECT 1 FROM item_part_keys pk WHERE pk.publish_id = ${iq}.publish_id AND pk.pos = ${iq}.pos AND pk.key IN ${list}))`;
/** SQL: question row `iq` has the key `x`. */
export const qKeyEq = (iq: string, x: string) =>
  `(${x} IN (${QKEY_COLS.map((k) => `${iq}.${k}`).join(", ")}) OR EXISTS (SELECT 1 FROM item_part_keys pk WHERE pk.publish_id = ${iq}.publish_id AND pk.pos = ${iq}.pos AND pk.key = ${x}))`;

// ---- keyed fingerprints (round 13 hardening) -----------------------------------------------------

/**
 * Every fingerprint the Worker computes of question content (licence-sense hashes, key-free facts,
 * block keys, loose stem hashes) is HMAC-SHA256 under a subkey of the pool hash key, one per purpose
 * ("near", "keyfree", "block", "stem"), never a bare SHA-256: a full-form fingerprint covers a hidden
 * answer, and unkeyed it would let anyone holding a row test a guessed answer. The key is set once
 * per isolate by makeCore (core.ts) from the derived POOL_HASH_KEY.
 */
let fpRoot: string | null = null;
const fpKeys = new Map<string, Promise<CryptoKey>>();
export function setFingerprintKey(hexKey: string): void {
  if (fpRoot === hexKey) return;
  fpRoot = hexKey;
  fpKeys.clear();
}
const fpKey = (purpose: string): Promise<CryptoKey> => {
  if (fpRoot === null) throw new Error("pool hash key not configured (POOL_HASH_KEY)");
  let k = fpKeys.get(purpose);
  if (!k) {
    const root = new Uint8Array(fpRoot.match(/../g)!.map((h) => parseInt(h, 16)));
    k = (async () => {
      const rk = await crypto.subtle.importKey("raw", root, { name: "HMAC", hash: "SHA-256" }, false, ["sign"]);
      const sub = new Uint8Array(await crypto.subtle.sign("HMAC", rk, new TextEncoder().encode(`vershion-pool/fingerprint/${purpose}`)));
      return crypto.subtle.importKey("raw", sub, { name: "HMAC", hash: "SHA-256" }, false, ["sign"]);
    })();
    fpKeys.set(purpose, k);
  }
  return k;
};
/**
 * Round 22 (critic29a B2): the keyed token sketch of purged, erased or private content: HMAC-SHA256
 * under its own subkey of POOL_HASH_KEY (`sketch-v1`), 64 bits, as an UNORDERED, deduplicated, sorted
 * set (no positions, no first-appearance order). Without the key nothing in it can be inverted or
 * tested against a dictionary; rotating POOL_HASH_KEY makes old sketches unmatchable (ghosts then match
 * exactly only: re-keying needs the text, which is gone).
 */
export async function keyedOne(prefix: string, text: string, memo?: Map<string, string>): Promise<string> {
  const k = prefix + text;
  let h = memo?.get(k);
  if (h === undefined) {
    h = (await fingerprint("sketch-v1", k)).slice(0, 16);
    memo?.set(k, h);
  }
  return h;
}
export async function keyedSet(tokens: readonly string[], memo: Map<string, string> = new Map(), prefix = "t:"): Promise<string[]> {
  const out = new Set<string>();
  for (const t of tokens) out.add(await keyedOne(prefix, t, memo));
  return [...out].sort();
}
/**
 * Round 23 (critic30a B1): the id of the POOL_HASH_KEY in use (an HMAC under its own subkey, 64 bits).
 * Every identity hash in the pool (share-core's keyed content hashes, every Worker fingerprint, unit
 * keys, sketches) is made under that key, so the key is PERMANENT: the first use records its id in
 * pool_kv; a request or cron under any other key fails closed (503 pool_key_mismatch, one admin
 * alert) and changes nothing.
 */
export async function poolKeyId(): Promise<string> {
  return (await fingerprint("key-id", "vershion-pool/key-id/v1")).slice(0, 16);
}
const keyChecked = new WeakMap<object, string>();
/** Round 24: an isolate that found a populated pre-round-23 pool whose key it could not verify (no
 *  readable retained body): it does not pin, and does not try the same R2 read on every write. */
const keyUnverified = new WeakSet<object>();
/**
 * Round 24 (critic31a/31b): could the key in use be the key the data was made under? A pool populated
 * before round 23 has identity hashes but no recorded key id. Recompute one retained publish's keyed
 * content hashes from its R2 body: they were made under the data's key, so a mismatch says this is
 * another key. "empty": no pool data at all (nothing to protect). "unknown": there is data but no
 * readable body to check against.
 */
async function poolKeyVerdict(c: Ctx): Promise<"empty" | "match" | "mismatch" | "unknown"> {
  // Round 25 (critic-moodle-24, new4): try several candidates, not one: a missing or corrupt body in
  // the first must not hide another readable body that would prove a mismatch.
  const rows = (
    await c.db
      .prepare(
        `SELECT p.publish_id AS publish_id, p.full_key AS full_key FROM item_publishes p
          JOIN items i ON i.id = p.item_id AND i.fold_rev = 1
          WHERE p.full_key IS NOT NULL AND p.status IN ('current', 'previous')
            AND EXISTS (SELECT 1 FROM item_questions q WHERE q.publish_id = p.publish_id AND q.full_content IS NOT NULL)
          ORDER BY p.publish_id LIMIT 5`,
      )
      .all<{ publish_id: string; full_key: string }>()
  ).results;
  if (rows.length === 0) return (await c.db.prepare(`SELECT 1 AS x FROM items LIMIT 1`).first()) ? "unknown" : "empty";
  let seen = false;
  let all = true;
  for (const row of rows) {
    const o = await c.r2.get(row.full_key);
    if (!o) continue;
    let qs: import("./core.ts").QuestionJson[];
    try {
      const p = JSON.parse(await o.text()) as { bank?: { questions?: import("./core.ts").QuestionJson[] }; question?: import("./core.ts").QuestionJson };
      qs = p.bank ? (p.bank.questions ?? []) : p.question ? [p.question] : [];
    } catch {
      continue;
    }
    if (qs.length === 0) continue;
    const made = await c.core.contentHashes(qs);
    const stored = (await c.db.prepare(`SELECT full_content FROM item_questions WHERE publish_id = ?1 ORDER BY pos`).bind(row.publish_id).all<{ full_content: string }>()).results;
    seen = true;
    if (!(stored.length === made.length && stored.every((x, i) => x.full_content === made[i]))) all = false;
  }
  if (!seen) return "unknown";
  return all ? "match" : "mismatch";
}
async function keyMismatch(c: Ctx, kid: string, stored: string): Promise<never> {
  const first = await c.db.prepare(`INSERT OR IGNORE INTO pool_kv (key, value) VALUES (?1, ?2)`).bind(`key_mismatch_alerted:${kid}`, String(c.now())).run();
  if ((first.meta.changes ?? 0) === 1) send(c, { kind: "key_mismatch", keyId: kid, storedKeyId: stored });
  throw new PoolError(503, "pool_key_mismatch", { message: "The pool's hash key is not the one its data was made with. The key cannot be rotated." });
}
export async function assertPoolKey(c: Ctx): Promise<void> {
  const kid = await poolKeyId();
  if (keyChecked.get(c.db) === kid) return;
  let stored = (await c.db.prepare(`SELECT value FROM pool_kv WHERE key = 'pool_key_id'`).first<{ value: string }>())?.value;
  if (stored === undefined) {
    // Round 24: never pin the first key a pre-round-23 database happens to see. Verify it against
    // data already stored under the pool's real key first; a wrong key is refused and NOT pinned, so
    // the right one still works when it is restored. (An unverifiable populated pool is not pinned:
    // an unpinned key locks nobody out; a wrongly pinned one locks the real key out for good.)
    const verdict = keyUnverified.has(c.db) ? "unknown" : await poolKeyVerdict(c);
    if (verdict === "mismatch") await keyMismatch(c, kid, "");
    if (verdict === "unknown") {
      keyUnverified.add(c.db);
      return;
    }
    await c.db.prepare(`INSERT OR IGNORE INTO pool_kv (key, value) VALUES ('pool_key_id', ?1)`).bind(kid).run();
    stored = (await c.db.prepare(`SELECT value FROM pool_kv WHERE key = 'pool_key_id'`).first<{ value: string }>())?.value;
  }
  if (stored === kid) {
    keyChecked.set(c.db, kid);
    return;
  }
  await keyMismatch(c, kid, stored ?? "");
}
/** The keyed sketch of a body (its kind tag first, then the keyed set of its words: sameBody needs the kind). */
export async function keyedBody(body: readonly string[], memo?: Map<string, string>): Promise<string[]> {
  if (body.length === 0) return [];
  return [(await keyedSet([body[0]!], memo, "k:"))[0]!, ...(await keyedSet(body.slice(1), memo))];
}
/** The keyed MinHash bands (each band tuple under the same key, round 22): what ghosts and privacy rows keep. */
export async function keyedBands(bands: readonly string[], memo?: Map<string, string>): Promise<string[]> {
  return keyedSet(bands, memo, "b:");
}
export async function fingerprint(purpose: "near" | "keyfree" | "block" | "stem" | "sketch-v1" | "key-id", text: string): Promise<string> {
  const sig = await crypto.subtle.sign("HMAC", await fpKey(purpose), new TextEncoder().encode(text));
  return [...new Uint8Array(sig)].map((b) => b.toString(16).padStart(2, "0")).join("");
}

/**
 * Round 17 (critic22e A2): while items from before rounds 6-17 still wait for the cron backfill
 * (their licence-sense hashes and their authors' durable "first" records), no publish or import may
 * create evidence against them: 503 pool_migrating (the cron finishes in bounded batches).
 */
export async function assertNotMigrating(c: Ctx): Promise<void> {
  const r = await c.db.prepare(`SELECT 1 AS x FROM items WHERE fold_rev = 0 AND current_publish_id IS NOT NULL AND purged_at IS NULL LIMIT 1`).first();
  if (r) throw new PoolError(503, "pool_migrating", { message: "The shared pool is finishing an upgrade. Try again in a little while." }, { "Retry-After": "600" });
}
