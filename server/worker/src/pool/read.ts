// Every read path. One rule decides what a caller is served (servedForm): the author gets the full
// form; anyone else gets the stripped form of a hide_key publish and the full form otherwise. Every
// non-author read joins items.current_publish_id and the author's standing (VISIBLE_TO_OTHERS).
// full_sha256 / full_content / full_key never leave this module for a non-author of a hide_key publish.
import { counterWindowOpen, QUARANTINE_MS } from "./moderation.ts";
import { allowedForBank, decideLicence, isLicense, LICENSES } from "./license.ts";
import { authorContext, evidence, isHard, unitOf, voidInfo, type ChainLink } from "./provenance.ts";
import { collapseChain, fold, searchKey } from "./publish.ts";
import { displayChains, licenseCheck, licenseCheckOf, standInsOf, storedAllowed } from "./licensecheck.ts";
import { subjectFor, subjectParam } from "./visible.ts";
import { giveDaily,
  ALIVE_SQL,
  throwIfErased,
  assertNotMigrating,
  countMiss,
  DEFAULT_LIMITS,
  ID_RE,
  json,
  missGuard,
  notFound,
  PoolError,
  rawJson,
  readCursor,
  readOptionalJson,
  requireDaily,
  SHA_RE,
  sha256Hex,
  signCursor,
  usage,
  limitOf,
  READ_COST,
  READ_HEAD_COST,
  reserveBytes,
  USAGE_SQL,
  DAY_MS,
  visibleTo,
  VISIBLE_TO_OTHERS,
  type Ctx,
  type PoolPrincipal,
  anonChain,
} from "./shared.ts";

const KINDS: [string, number][] = [
  ["Mc", 1],
  ["Tf", 2],
  ["Matching", 4],
  ["Blank", 8],
  ["Short", 16],
  ["Multipart", 32],
];
const kindsOf = (mask: number) => KINDS.filter(([, b]) => mask & b).map(([k]) => k);

interface ItemRow {
  id: string;
  author_id: string;
  kind: string;
  visibility: string;
  version: number;
  title: string;
  subject: string;
  course: string;
  grade: string;
  kinds_mask: number;
  question_count: number;
  license: string;
  has_images: number;
  committed_at: number;
  current_publish_id: string;
  full_key: string;
  stripped_key: string | null;
  p_hide: number;
  p_license: string;
  p_version: number;
  p_public_version: number;
  public_at: number | null;
  public_publish_id: string;
  schema_version: number;
  author_name: string | null;
}

/** The item and its current publish, iff the caller may see it (one statement). */
async function loadVisible(c: Ctx, id: string, me: string): Promise<ItemRow | null> {
  return c.db
    .prepare(
      `SELECT i.id, i.author_id, i.kind, i.visibility, i.version, i.title, i.subject, i.course, i.grade, i.kinds_mask, i.question_count,
              i.license, i.has_images, i.committed_at, i.current_publish_id,
              p.full_key, p.stripped_key, p.hide_key AS p_hide, p.license AS p_license, p.version AS p_version, p.schema_version,
              p.public_version AS p_public_version, i.public_at, COALESCE(i.public_publish_id, i.current_publish_id) AS public_publish_id,
              u.display_name AS author_name
         FROM items i JOIN users u ON u.id = i.author_id JOIN item_publishes p ON p.publish_id = i.current_publish_id
        WHERE i.id = ?1 AND ${visibleTo("?2")}`,
    )
    .bind(id, me)
    .first<ItemRow>();
}

/** THE served-form rule. */
export function servedForm(row: { author_id: string; p_hide: number }, me: string): "full" | "stripped" {
  return row.author_id !== me && row.p_hide === 1 ? "stripped" : "full";
}

function itemView(r: ItemRow, me: string, form: "full" | "stripped") {
  const isAuthor = r.author_id === me;
  return {
    id: r.id,
    kind: r.kind,
    title: r.title,
    subject: r.subject,
    course: r.course,
    grade: r.grade,
    kinds: kindsOf(r.kinds_mask),
    question_count: r.question_count,
    // Non-authors see the public version: it does not move for a change they are not served.
    version: isAuthor ? r.p_version : r.p_public_version,
    // Non-authors only ever see the public publish id: a hidden-only republish moves nothing (D4).
    publish_id: isAuthor ? r.current_publish_id : r.public_publish_id,
    license: r.p_license,
    hide_key: r.p_hide === 1,
    has_images: r.has_images === 1,
    author_name: r.author_name ?? "deleted teacher",
    committed_at: isAuthor ? r.committed_at : (r.public_at ?? r.committed_at),
    form,
    is_author: isAuthor,
    ...(isAuthor ? { visibility: r.visibility } : {}),
  };
}

interface QRow {
  qid: string;
  pos: number;
  match_hash: string;
  full_sha256: string;
  stripped_sha256: string | null;
  chain_json: string;
  allowed_json: string | null;
}

/** Per-question provenance for the served form (never the full hash of a stripped serve). */
async function questionsFor(c: Ctx, publishId: string, form: "full" | "stripped", license: string): Promise<{ questions: object[]; license_check: ReturnType<typeof licenseCheck> }> {
  const rows = await c.db
    .prepare(`SELECT qid, pos, match_hash, full_sha256, stripped_sha256, chain_json, allowed_json FROM item_questions WHERE publish_id = ?1 ORDER BY pos`)
    .bind(publishId)
    .all<QRow>();
  // Displayed chains (licensecheck.ts): void links dropped, rightful stand-ins, deleted names.
  const shown = await displayChains(c, rows.results.map((q) => JSON.parse(q.chain_json) as ChainLink[]));
  // Round 10 M-6: the reader-visible marker, from the displayed chains alone.
  const stored = rows.results.map((q) => storedAllowed(q.allowed_json));
  const license_check = licenseCheck(license, shown, stored, await standInsOf(c, stored));
  const questions = rows.results.map((q, k) => ({
    pos: q.pos,
    qid: q.qid,
    served_sha256: form === "stripped" ? q.stripped_sha256 : q.full_sha256,
    match_hash: q.match_hash,
    // The stored credits are complete (round 9 J6); at most CHAIN_MAX are shown, with "and N earlier contributors".
    chain: collapseChain(shown[k]!),
  }));
  return { questions, license_check };
}

/** The stored body's R2 object (a handle: its bytes are only streamed, never held whole). */
async function body(c: Ctx, key: string | null): Promise<R2ObjectBody | null> {
  if (!key) return null;
  return c.r2.get(key);
}

/**
 * `{...head, "payload": <the stored canonical bytes, unchanged>}` as a STREAM (round 5 E6): the head,
 * then the R2 body piped through, then "}". Nothing concatenates the body into a string; the read's
 * byte reservation is released when the stream ends or is cancelled.
 */
export function streamedPayload(h: string, obj: R2ObjectBody, release: () => void, extra: Record<string, string> = {}, idleMs = READ_IDLE_MS, lifetimeMs = READ_LIFETIME_MS, key = "payload"): Response {
  const pre = new TextEncoder().encode(`${h.slice(0, -1)}${h.length > 2 ? "," : ""}"${key}":`);
  const tail = new TextEncoder().encode("}");
  const src = obj.body.getReader(); // r2-object: a stored body, never a request
  let started = false;
  // A reader that stops reading cannot pin its reservation (round 6): past idleMs without a pull the
  // stream is cancelled and everything it holds is released.
  let timer: ReturnType<typeof setTimeout> | undefined;
  let ctl: ReadableStreamDefaultController<Uint8Array> | null = null;
  const t0 = Date.now();
  let sent = 0;
  const finish = () => {
    if (timer) clearTimeout(timer);
    clearTimeout(life);
    release();
  };
  const kill = () => {
    finish();
    src.cancel("timeout").catch(() => {});
    try {
      ctl?.error(new Error("read timeout"));
    } catch {
      /* already closed */
    }
  };
  // A bound on its life (round 8): at lifetimeMs a stream still moving at READ_MIN_RATE or more keeps
  // going (a large body on a slow but honest link), up to READ_LIFETIME_FACTOR x lifetimeMs; a
  // trickle below that rate is cut. Nothing holds a reservation forever.
  let life: ReturnType<typeof setTimeout>;
  const check = () => {
    const elapsed = Date.now() - t0;
    if (elapsed < lifetimeMs * READ_LIFETIME_FACTOR && sent / Math.max(1, elapsed / 1000) >= READ_MIN_RATE) {
      life = setTimeout(check, Math.max(100, Math.floor(lifetimeMs / 4)));
      (life as { unref?: () => void }).unref?.();
    } else kill();
  };
  life = setTimeout(check, lifetimeMs);
  (life as { unref?: () => void }).unref?.();
  const arm = () => {
    if (timer) clearTimeout(timer);
    timer = setTimeout(kill, idleMs);
    (timer as { unref?: () => void }).unref?.();
  };
  const stream = new ReadableStream<Uint8Array>({
    start(c) {
      ctl = c;
      arm();
    },
    async pull(ctrl) {
      arm();
      try {
        if (!started) {
          started = true;
          ctrl.enqueue(pre);
          return;
        }
        const { done, value } = await src.read();
        if (value) sent += value.byteLength;
        if (done) {
          ctrl.enqueue(tail);
          ctrl.close();
          finish();
        } else ctrl.enqueue(value);
      } catch (e) {
        finish();
        ctrl.error(e);
      }
    },
    cancel(reason) {
      finish();
      return src.cancel(reason);
    },
  });
  const headers = rawJson(200, "", extra).headers;
  const length = pre.length + obj.size + tail.length;
  headers.set("Content-Length", String(length));
  // workerd sends a Content-Length for a stream only through a FixedLengthStream (node keeps the header).
  const FLS = (globalThis as { FixedLengthStream?: typeof FixedLengthStream }).FixedLengthStream;
  if (FLS) {
    const fixed = new FLS(length);
    stream.pipeTo(fixed.writable).catch(() => {});
    return new Response(fixed.readable, { status: 200, headers });
  }
  return new Response(stream, { status: 200, headers });
}

/**
 * Unfinished streamed reads (isolate-local caps: each isolate counts its own; round 6, round 7):
 * per account 4; per account within one address 2; per address 8, so one account behind a school's
 * NAT can never take all of an address's slots from the other teachers on it.
 */
export const OPEN_READS_MAX = 4;
export const OPEN_READS_PER_ACCOUNT_ADDRESS = 2;
export const OPEN_READS_PER_ADDRESS = 8;
/** A streamed read nobody pulls from for this long is cancelled. */
export const READ_IDLE_MS = 10_000;
/** ... and a stream slower than READ_MIN_RATE ends here; a faster one may live READ_LIFETIME_FACTOR times longer. */
export const READ_LIFETIME_MS = 60_000;
export const READ_MIN_RATE = 20 * 1024;
export const READ_LIFETIME_FACTOR = 5;
const openReads = new Map<string, number>();

/** Takes a streamed-read slot for each [key, cap], or 429; returns the release (idempotent). */
export function takeReadSlots(keys: [string, number][] | string[]): () => void {
  const ks: [string, number][] = (keys as unknown[]).map((k) => (typeof k === "string" ? [k, OPEN_READS_MAX] : (k as [string, number])));
  if (ks.some(([k, cap]) => (openReads.get(k) ?? 0) >= cap))
    throw new PoolError(429, "too_many_open_reads", { message: "Too many downloads are open at once (from your account or your network). Retry in a moment." }, { "Retry-After": "2" });
  for (const [k] of ks) openReads.set(k, (openReads.get(k) ?? 0) + 1);
  let done = false;
  return () => {
    if (done) return;
    done = true;
    for (const [k] of ks) {
      const n = (openReads.get(k) ?? 1) - 1;
      if (n <= 0) openReads.delete(k);
      else openReads.set(k, n);
    }
  };
}

/**
 * The isolate byte reservation and the per-account / per-IP stream slots of one read, released
 * together. `grow(n)` adds to the reservation once the head's size is known (or 503 busy).
 */
export function readHold(c: Ctx, req: Request, me: string): (() => void) & { grow: (n: number) => void } {
  const ip = (req.headers.get("CF-Connecting-IP") ?? "").trim();
  const addr = c.deps.ipBucket(ip);
  const slots = takeReadSlots([
    [`u:${me}`, OPEN_READS_MAX],
    [`ua:${me}@${addr}`, OPEN_READS_PER_ACCOUNT_ADDRESS],
    [`i:${addr}`, OPEN_READS_PER_ADDRESS],
  ]);
  const parts: (() => void)[] = [];
  try {
    parts.push(reserveBytes(READ_COST));
  } catch (e) {
    slots();
    throw e;
  }
  const release = Object.assign(
    () => {
      for (const p of parts) p();
      slots();
    },
    { grow: (n: number) => void parts.push(reserveBytes(n)) },
  );
  return release;
}

/** An ETag for what a read serves: a hash of the head (which names the served publish), nothing more. */
async function etagOf(h: string): Promise<string> {
  return `"${(await sha256Hex(new TextEncoder().encode(h))).slice(0, 32)}"`;
}

async function itemIdOrMiss(c: Ctx, id: string, me: string): Promise<void> {
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

// ---- GET /items/:id ---------------------------------------------------------------------------------

export async function getItem(c: Ctx, req: Request, p: PoolPrincipal, id: string, headOnly = false): Promise<Response> {
  const me = p.userId;
  await itemIdOrMiss(c, id, me);
  await requireDaily(c, me, "item_reads");
  const release = readHold(c, req, me);
  try {
    const r = await loadVisible(c, id, me);
    if (!r) return await miss(c, me);
    const form = servedForm(r, me);
    const key = form === "stripped" ? r.stripped_key : r.full_key;
    const { questions, license_check } = await questionsFor(c, r.current_publish_id, form, r.p_license);
    const head = { item: { ...itemView(r, me, form), ...(await tagsFor(c, r.current_publish_id)) }, questions, license_check };
    const h = JSON.stringify(head);
    release.grow(READ_HEAD_COST * h.length);
    const etag = await etagOf(h);
    // HEAD and a matching If-None-Match serve no body: nothing is recorded (the caller was served it before, or not at all).
    // If-None-Match uses the weak comparison (RFC 9110 13.1.2): W/"x" matches "x".
    if (headOnly || (req.headers.get("If-None-Match") ?? "").split(",").some((t) => t.trim().replace(/^W\//, "") === etag || t.trim() === "*")) {
      const size = key ? (await c.r2.head(key))?.size : undefined;
      if (size === undefined) return await miss(c, me);
      const hs = rawJson(200, "", { ETag: etag }).headers;
      if (headOnly) hs.set("Content-Length", String(new TextEncoder().encode(`${h.slice(0, -1)},"payload":`).length + size + 1));
      release();
      return new Response(null, { status: headOnly ? 200 : 304, headers: hs });
    }
    // Round 18 (critic23d V3): a non-author's full read records evidence: none while the backfill runs.
    if (r.author_id !== me && !headOnly) await assertNotMigrating(c);
    const obj = await body(c, key);
    if (obj === null) return await miss(c, me);
    // A read is licence evidence too (critic X6): a verbatim copy of something only read is still a
    // copy, so the hard check must see it. Recorded as a 'view', which never counts as an import. A
    // takedown or hold that landed meanwhile turns the read into a 404 (round 6): no body is served.
    // Any error here cancels the R2 body stream.
    let recorded = true;
    try {
      if (r.author_id !== me) recorded = await recordServed(c, me, r, form, "view");
    } catch (e) {
      await obj.body.cancel().catch(() => {});
      throw e;
    }
    if (!recorded) {
      await obj.body.cancel().catch(() => {});
      return await miss(c, me);
    }
    return streamedPayload(h, obj, release, { ETag: etag }, c.deps.readIdleMs, c.deps.readLifetimeMs);
  } catch (e) {
    release();
    throw e;
  }
}

// ---- POST /items/:id/import -------------------------------------------------------------------------

export async function importItem(c: Ctx, req: Request, p: PoolPrincipal, id: string): Promise<Response> {
  await assertNotMigrating(c);
  const me = p.userId;
  await itemIdOrMiss(c, id, me);
  await readOptionalJson(req, `u:${me}`, c.deps.bodyDeadlineMs);
  await requireDaily(c, me, "imports");
  // (Round 22, critic29b: an import that fails gives its import allowance back; only successes count.)
  const refund = () => giveDaily(c, me, "imports").catch(() => {});
  try {
    await requireDaily(c, me, "item_reads");
  } catch (e) {
    await refund();
    throw e;
  }
  const release = readHold(c, req, me);
  try {
    const res = await importInner(c, me, id, release);
    if (res.status >= 300) await refund();
    return res;
  } catch (e) {
    release();
    await refund();
    throw e;
  }
}

async function importInner(c: Ctx, me: string, id: string, release: (() => void) & { grow: (n: number) => void }): Promise<Response> {
  const r = await loadVisible(c, id, me);
  if (!r) return miss(c, me);
  const form = servedForm(r, me);
  const isAuthor = r.author_id === me;
  // circle-only material is copied only inside its circles (phase 3); in phase 2 only its author.
  if (!isAuthor && r.p_license === "circle-only") return miss(c, me);
  const before = await c.db
    .prepare(`SELECT version FROM imports WHERE user_id = ?1 AND item_id = ?2 AND kind = 'import' ORDER BY version`)
    .bind(me, id)
    .all<{ version: number }>();
  const obj = await body(c, form === "stripped" ? r.stripped_key : r.full_key);
  if (obj === null) return miss(c, me);
  // Every error path from here cancels the R2 body stream (nothing is left half-open).
  try {
    return await importServe(c, me, r, form, isAuthor, before.results, obj, release);
  } catch (e) {
    await obj.body.cancel().catch(() => {});
    throw e;
  }
}

async function importServe(
  c: Ctx,
  me: string,
  r: ItemRow,
  form: "full" | "stripped",
  isAuthor: boolean,
  before: { version: number }[],
  obj: R2ObjectBody,
  release: (() => void) & { grow: (n: number) => void },
): Promise<Response> {
  let images: { held: number; until?: number; reason?: string } = { held: 0 };
  if (!isAuthor) {
    // Recorded only if the item is STILL visible after the body was read: a takedown or hold that
    // landed meanwhile turns the import into a 404 with nothing recorded (one atomic batch).
    const res = await recordServed(c, me, r, form, "import");
    if (!res) {
      await obj.body.cancel().catch(() => {});
      return miss(c, me);
    }
    images = await holdImportedImages(c, me, r.current_publish_id, form);
  }
  const { questions, license_check } = await questionsFor(c, r.current_publish_id, form, r.p_license);
  const head = {
    item: { ...itemView(r, me, form), ...(await tagsFor(c, r.current_publish_id)) },
    questions,
    license_check,
    license: r.p_license,
    // The same function the publish check uses, over the same grants, for a verbatim copy (critic A4).
    may_republish_as: isAuthor ? [...LICENSES] : await mayRepublishAs(c, me, r.current_publish_id, form),
    previously_imported_versions: before.map((x) => x.version),
    // Copy-on-import (IMG-1): the images served are held for this account (charged to its quota)
    // for IMPORT_IMAGE_HOLD_MS, so a republish of the copy needs no re-upload even if the source changes.
    images,
    recorded: !isAuthor,
  };
  const h = JSON.stringify(head);
  release.grow(READ_HEAD_COST * h.length);
  return streamedPayload(h, obj, release, {}, c.deps.readIdleMs, c.deps.readLifetimeMs);
}

// ---- POST /items/versions ---------------------------------------------------------------------------

export async function versions(c: Ctx, req: Request, p: PoolPrincipal): Promise<Response> {
  const me = p.userId;
  const b = await readOptionalJson(req, `u:${me}`, c.deps.bodyDeadlineMs);
  const ids = b.ids;
  if (!Array.isArray(ids) || ids.length === 0 || ids.length > 200 || !ids.every((x) => typeof x === "string" && x.length <= 64)) throw new PoolError(400, "invalid_request", { field: "ids" });
  await requireDaily(c, me, "item_reads");
  const uniq = [...new Set(ids as string[])].filter((x) => ID_RE.test(x));
  const rows = await c.db
    .prepare(
      `SELECT j.value AS id, i.author_id, CASE WHEN i.author_id = ?1 THEN i.version ELSE i.public_version END AS version, i.removed_reason,
              (SELECT MAX(version) FROM imports im WHERE im.user_id = ?1 AND im.item_id = j.value AND im.kind = 'import') AS mine,
              EXISTS (SELECT 1 FROM imports im WHERE im.user_id = ?1 AND im.item_id = j.value) AS seen,
              (SELECT MIN(h.at) FROM item_holds h WHERE h.item_id = j.value AND h.source = 'privacy') AS privacy_held_at, i.purged_at,
              i.license, i.license_version, i.purge_after, EXISTS (SELECT 1 FROM item_holds h WHERE h.item_id = j.value) AS held,
              CASE WHEN i.id IS NULL THEN 0
                   WHEN i.author_id = ?1 THEN (i.removed_at IS NULL AND i.current_publish_id IS NOT NULL)
                   ELSE (${VISIBLE_TO_OTHERS}) END AS visible
         FROM json_each(?2) j LEFT JOIN items i ON i.id = j.value LEFT JOIN users u ON u.id = i.author_id`,
    )
    .bind(me, JSON.stringify(uniq))
    .all<{ id: string; author_id: string | null; version: number | null; removed_reason: string | null; mine: number | null; seen: number; privacy_held_at: number | null; purged_at: number | null; license: string | null; license_version: number | null; purge_after: number | null; held: number; visible: number }>();
  const by = new Map(rows.results.map((r) => [r.id, r]));
  const out = (ids as string[]).map((id) => {
    const r = by.get(id);
    // Only for an item the caller authored or imported: no oracle for arbitrary ids.
    // Only for an item the caller authored, imported or read: no oracle for arbitrary ids. A preview
    // read never counts as having the newest version (it must not hide an update notice).
    if (!r || r.author_id === null || (r.author_id !== me && !r.seen)) return { id, status: "unavailable" };
    // A HELD item, whatever the hold (round 9 J4), is "under_review" until it is purged: a hold can
    // always be lifted (an unblock, a put-back, an admin release), so the client keeps its copy.
    // "removed" only for what is gone for good.
    // Round 12: a HELD item says only "held", to everyone, whatever the hold (privacy, copyright, a
    // report): which kind of block an item hit is never observable through another account. The
    // author's own held item is "under_review" too (never "current" while others cannot read it).
    if (r.removed_reason === null && r.held && r.purged_at === null) return { id, status: "under_review", reason: "held" };
    if (r.removed_reason === "privacy" && r.purged_at === null) return { id, status: "under_review", reason: "privacy" };
    if (r.removed_reason !== null) {
      const reason = r.removed_reason === "takedown" ? "takedown" : r.removed_reason === "privacy" ? "privacy" : r.removed_reason === "admin" ? "admin" : "deleted";
      // Every still-reversible state (round 8): an author deletion within its undo window, an admin
      // removal before it can be undone, a takedown in quarantine: "under_review", with the date it
      // becomes final when there is one. "removed" only once it is purged.
      if (r.purged_at === null) return { id, status: "under_review", reason, ...(r.purge_after !== null ? { until: r.purge_after } : {}) };
      return { id, status: "removed", reason };
    }
    if (!r.visible) return { id, status: "unavailable" };
    // "newer" only against an imported copy; a caller who only read it holds no copy to update.
    // The licence and its own counter (round 7): a licence-only change (even a hidden-only republish)
    // is visible as such, without implying anything else changed.
    const lic = { license: r.license, license_version: r.license_version };
    if (r.author_id !== me && r.mine !== null && r.mine < (r.version ?? 0)) return { id, status: "newer", version: r.version, ...lic };
    return { id, status: "current", version: r.version, ...lic };
  });
  return json(200, { items: out });
}

// ---- GET /items -------------------------------------------------------------------------------------

/** Publishes the word index may hand one browse query (round 12): the bound on a search's walk. */
export const SEARCH_CANDIDATES = 5000;
/** Word-index rows one browse query looks at before the shortlist (round 13). */
export const SEARCH_EXAMINED = 50_000;

export async function list(c: Ctx, url: URL, p: PoolPrincipal): Promise<Response> {
  const me = p.userId;
  const q = url.searchParams;
  const scope = q.get("scope") ?? "public";
  if (scope !== "public" && scope !== "mine") throw new PoolError(400, "invalid_request", { field: "scope" });
  const limitRaw = q.get("limit");
  const limit = limitRaw === null ? 20 : Number(limitRaw);
  if (!Number.isInteger(limit) || limit < 1) throw new PoolError(400, "invalid_request", { field: "limit" });
  const lim = Math.min(limit, 50);
  const subject = q.get("subject");
  if (subject !== null && (subject.length === 0 || subject.length > 100)) throw new PoolError(400, "invalid_request", { field: "subject" });
  const kind = q.get("kind");
  const bit = kind === null ? 0 : (KINDS.find(([k]) => k === kind)?.[1] ?? -1);
  if (bit < 0) throw new PoolError(400, "invalid_request", { field: "kind" });
  const tagQ = q.get("tag");
  if (tagQ !== null && (tagQ.length === 0 || tagQ.length > 64)) throw new PoolError(400, "invalid_request", { field: "tag" });
  const tag = tagQ?.trim().toLowerCase() ?? null;
  const stdQ = q.get("standard");
  if (stdQ !== null && (stdQ.length === 0 || stdQ.length > 64)) throw new PoolError(400, "invalid_request", { field: "standard" });
  const standard = stdQ?.replace(/\s+/g, " ").trim().toUpperCase() ?? null;
  const prefix = q.get("q");
  if (prefix !== null && (prefix.length === 0 || prefix.length > 100)) throw new PoolError(400, "invalid_request", { field: "q" });
  // A case-insensitive title prefix over the stored case fold (any script, not only ASCII); % and _ escaped.
  // The start of any word of the title (the stored fold starts with a space), case- and accent-insensitive.
  // Round 9: the query folds like the stored column (searchKey); it also matches the start of a tag.
  const qKey = prefix === null ? null : searchKey(prefix);
  if (qKey === "") throw new PoolError(400, "invalid_request", { field: "q" });
  const qWords = qKey === null ? [] : [...new Set(qKey.split(" ").filter((x) => x.length > 0))].slice(0, 6);
  // A licence filter (round 11).
  const license = q.get("license");
  if (license !== null && !isLicense(license)) throw new PoolError(400, "invalid_request", { field: "license" });
  let cur: [number, string] | null = null;
  const cs = q.get("cursor");
  if (cs !== null) {
    cur = await readCursor(c, cs);
    if (!cur) throw new PoolError(400, "invalid_request", { field: "cursor" });
  }
  await requireDaily(c, me, "browse");
  // Visibility, holds, removal, author standing, the filters and the LIMIT are one statement.
  // (removed_at IS NULL stated at the top so the partial browse index applies, round 11.)
  const where = [scope === "mine" ? `i.author_id = ?1 AND i.removed_at IS NULL AND i.current_publish_id IS NOT NULL` : `i.visibility = 'public' AND i.removed_at IS NULL AND ${visibleTo("?1")}`];
  // Pages on the immutable creation time (a republish never moves an item across a cursor, PAGE-1).
  where.push(
    `(?2 IS NULL OR i.subject_fold = ?2)`,
    `(?3 = 0 OR (i.kinds_mask & ?3) <> 0)`,
    `(?4 IS NULL OR i.created_at < ?4 OR (i.created_at = ?4 AND i.id < ?5))`,
    `(?7 IS NULL OR EXISTS (SELECT 1 FROM item_tags t WHERE t.publish_id = i.current_publish_id AND (t.tag = ?7 OR t.fold = ?11)))`,
    `(?8 IS NULL OR EXISTS (SELECT 1 FROM item_standards s WHERE s.publish_id = i.current_publish_id AND s.code = ?8))`,
    // q (round 12): every query word starts a word of the title, the subject, a tag, or the SERVED
    // question text (served_words, keyed on the publish non-authors are served; the full form is
    // never indexed). The walk starts from the word index (the longest query word, at most
    // SEARCH_CANDIDATES publishes), never from the whole item list.
    `(json_array_length(?12) = 0 OR NOT EXISTS (SELECT 1 FROM json_each(?12) qw WHERE NOT EXISTS (
            SELECT 1 FROM served_words sw WHERE sw.publish_id = ${scope === "mine" ? "i.current_publish_id" : "i.public_publish_id"}
               AND sw.word >= qw.value AND sw.word < qw.value || char(1114111))))`,
    `(?13 IS NULL OR i.license = ?13)`,
  );
  const driver = qWords.reduce((a, b) => (b.length > a.length ? b : a), "");
  const from =
    qWords.length > 0
      ? // Round 13: the shortlist keeps only publishes that can be shown (the served publish of a public,
        // live, unheld item; for "mine", the author's current ones), newest first, so neither the
        // prefix nor unlisted, held or old publishes decide which SEARCH_CANDIDATES are kept. At most
        // SEARCH_EXAMINED word rows are looked at.
        `(SELECT ci.${scope === "mine" ? "current_publish_id" : "public_publish_id"} AS publish_id
            FROM (SELECT DISTINCT publish_id FROM served_words WHERE word >= ?9 AND word < ?9 || char(1114111) LIMIT ${SEARCH_EXAMINED}) x
            JOIN items ci ON ci.${scope === "mine" ? "current_publish_id" : "public_publish_id"} = x.publish_id
           WHERE ci.removed_at IS NULL AND ci.current_publish_id IS NOT NULL
             AND ${scope === "mine" ? "ci.author_id = ?1" : "ci.visibility = 'public' AND NOT EXISTS (SELECT 1 FROM item_holds hh WHERE hh.item_id = ci.id)"}
           ORDER BY ci.created_at DESC, ci.id DESC LIMIT ${c.deps.searchCandidates ?? SEARCH_CANDIDATES}) cand
         CROSS JOIN items i ON i.${scope === "mine" ? "current_publish_id" : "public_publish_id"} = cand.publish_id JOIN users u ON u.id = i.author_id`
      : tag !== null
        ? // A tag filter walks from the tag index too (round 12), never the whole item list; round 16
          // (critic21d 2): like the word shortlist, only publishes that can be shown, newest first, so
          // unlisted, held or old publishes never decide which SEARCH_CANDIDATES are kept.
          `(SELECT ci.${scope === "mine" ? "current_publish_id" : "public_publish_id"} AS publish_id
              FROM (SELECT DISTINCT publish_id FROM item_tags WHERE fold = ?11 LIMIT ${SEARCH_EXAMINED}) x
              JOIN items ci ON ci.${scope === "mine" ? "current_publish_id" : "public_publish_id"} = x.publish_id
             WHERE ci.removed_at IS NULL AND ci.current_publish_id IS NOT NULL
               AND ${scope === "mine" ? "ci.author_id = ?1" : "ci.visibility = 'public' AND NOT EXISTS (SELECT 1 FROM item_holds hh WHERE hh.item_id = ci.id)"}
             ORDER BY ci.created_at DESC, ci.id DESC LIMIT ${c.deps.searchCandidates ?? SEARCH_CANDIDATES}) cand
           CROSS JOIN items i ON i.${scope === "mine" ? "current_publish_id" : "public_publish_id"} = cand.publish_id JOIN users u ON u.id = i.author_id`
        : `items i JOIN users u ON u.id = i.author_id`;
  const rows = await c.db
    .prepare(
      `SELECT i.id, i.kind, i.title, i.subject, i.course, i.grade, i.kinds_mask, i.question_count,
              CASE WHEN i.author_id = ?1 THEN i.version ELSE i.public_version END AS version, i.hide_key, i.license,
              i.has_images, CASE WHEN i.author_id = ?1 THEN i.committed_at ELSE COALESCE(i.public_at, i.committed_at) END AS committed_at,
              i.created_at, i.visibility, i.author_id, u.display_name AS author_name
         FROM ${from}
        WHERE ${where.join(" AND ")}
        ORDER BY i.created_at DESC, i.id DESC LIMIT ?6`,
    )
    .bind(me, subject === null ? null : fold(subject), bit, cur?.[0] ?? null, cur?.[1] ?? "", lim + 1, tag, standard, driver, null, tagQ === null ? null : searchKey(tagQ), JSON.stringify(qWords), license)
    .all<{ id: string; kind: string; title: string; subject: string; course: string; grade: string; kinds_mask: number; question_count: number; version: number; hide_key: number; license: string; has_images: number; committed_at: number; created_at: number; visibility: string; author_id: string; author_name: string | null }>();
  const page = rows.results.slice(0, lim);
  // Each result carries the reader-visible licence marker (round 11), as a read would.
  const checks = await licenseCheckOf(c, page.map((r) => r.id));
  const next = rows.results.length > lim ? await signCursor(c, page[page.length - 1]!.created_at, page[page.length - 1]!.id) : null;
  return json(200, {
    items: page.map((r) => ({
      id: r.id,
      kind: r.kind,
      title: r.title,
      subject: r.subject,
      course: r.course,
      grade: r.grade,
      kinds: kindsOf(r.kinds_mask),
      question_count: r.question_count,
      version: r.version,
      hide_key: r.hide_key === 1,
      license: r.license,
      ...(checks.has(r.id) ? { license_check: checks.get(r.id) } : {}),
      has_images: r.has_images === 1,
      author_name: r.author_name ?? "deleted teacher",
      committed_at: r.committed_at,
      ...(r.author_id === me ? { visibility: r.visibility, is_author: true } : {}),
    })),
    next_cursor: next,
  });
}

// ---- GET /items/mine --------------------------------------------------------------------------------

/** Items one page of /items/mine lists (round 21: paged, never cut at 1,000). */
export const MINE_PAGE = 1000;

export async function mine(c: Ctx, p: PoolPrincipal, after: string | null = null): Promise<Response> {
  const me = p.userId;
  // Round 21 (critic27b): newest first, paged by a cursor (`after` = the last item's updated_at:id).
  const cur = /^(\d{1,16}):([0-9a-f]{32})$/.exec(after ?? "");
  const rows = await c.db
    .prepare(
      `SELECT i.id, i.kind, i.title, i.subject, i.version, i.visibility, i.license, i.hide_key, i.question_count, i.committed_at,
              i.removed_at, i.removed_reason, i.purge_after, i.current_publish_id, i.updated_at,
              EXISTS (SELECT 1 FROM item_holds h WHERE h.item_id = i.id) AS held,
              EXISTS (SELECT 1 FROM item_holds h WHERE h.item_id = i.id AND h.source = 'needs_reupload') AS needs_reupload,
              -- Round 18: held ONLY because teachers reported a visible hint of the hidden key.
              (EXISTS (SELECT 1 FROM item_holds h WHERE h.item_id = i.id AND h.source = 'key_leak') AND NOT EXISTS (SELECT 1 FROM item_holds h WHERE h.item_id = i.id AND h.source <> 'key_leak')) AS key_leak,
              (SELECT COUNT(DISTINCT im.user_id) FROM imports im WHERE im.item_id = i.id AND im.user_id <> i.author_id AND im.kind = 'import') AS import_count,
              (SELECT a.id FROM dmca_actions a WHERE a.item_id = i.id AND a.restored_at IS NULL AND a.purged_at IS NULL ORDER BY a.disabled_at DESC LIMIT 1) AS action_id,
              (SELECT a.counter_notice_id FROM dmca_actions a WHERE a.item_id = i.id AND a.restored_at IS NULL AND a.purged_at IS NULL ORDER BY a.disabled_at DESC LIMIT 1) AS counter_notice_id,
              (SELECT a.author_notified_at FROM dmca_actions a WHERE a.item_id = i.id AND a.restored_at IS NULL AND a.purged_at IS NULL ORDER BY a.disabled_at DESC LIMIT 1) AS notified_at
         FROM items i WHERE i.author_id = ?1 AND (i.purged_at IS NULL OR (i.removed_reason = 'privacy' AND i.removed_at > ?2))
          AND (?3 IS NULL OR i.updated_at < ?3 OR (i.updated_at = ?3 AND i.id < ?4))
        ORDER BY i.updated_at DESC, i.id DESC LIMIT ${MINE_PAGE}`,
    )
    .bind(me, c.now() - PRIVACY_APPEAL_MS, cur ? Number(cur[1]) : null, cur ? cur[2] : "")
    .all<{ id: string; kind: string; title: string; subject: string; version: number; visibility: string; license: string; hide_key: number; question_count: number; committed_at: number | null; removed_at: number | null; removed_reason: string | null; purge_after: number | null; current_publish_id: string | null; held: number; needs_reupload: number; key_leak: number; import_count: number; action_id: string | null; counter_notice_id: string | null; notified_at: number | null }>();
  const now = c.now();
  // Round 28 (owner; spec appendix 14): the counter-notice is offered only inside its 30-day window, by
  // the SAME rule POST /items/:id/counter-notice applies (author_notified_at > now - QUARANTINE_MS), and
  // the window's end is given; past it the answer is "closed" (the POST refuses 409 window_closed).
  const counterState = (counterId: string | null, actionId: string | null, notified: number | null) =>
    counterId !== null ? "received" : actionId !== null && counterWindowOpen(notified, now) ? "available" : "closed";
  // Round 10: a hold an ADMIN placed on an existing item (a confirmed student-data purge or a
  // takedown holding its copies) says what it is about and which questions; a hold placed by a
  // publish's own content check keeps the neutral notice (no block-membership oracle).
  const heldIds = rows.results.filter((r) => r.held && r.removed_reason === null && r.current_publish_id).map((r) => r.id);
  const held = new Map<string, Record<string, unknown>>();
  if (heldIds.length > 0) {
    // Round 11 P3: the positions come from the hold's snapshot at the admin action, and only while
    // the item still has the content the admin saw (no re-evaluation against later content).
    const hr = await c.db
      .prepare(
        `SELECT h.item_id, h.source, h.detail, i.current_publish_id AS pub FROM item_holds h JOIN items i ON i.id = h.item_id
          WHERE h.item_id IN (SELECT value FROM json_each(?1)) AND h.by IS NOT NULL AND (h.source = 'privacy' OR h.source LIKE 'dmca:%')`,
      )
      .bind(JSON.stringify(heldIds))
      .all<{ item_id: string; source: string; detail: string | null; pub: string }>();
    for (const x of hr.results) {
      const priv = x.source === "privacy";
      let snap: { publish_id?: string; positions?: number[] } = {};
      try {
        snap = x.detail ? (JSON.parse(x.detail) as typeof snap) : {};
      } catch {
        /* no snapshot */
      }
      held.set(x.item_id, {
        hold_class: priv ? "student_data" : "copyright",
        ...(snap.publish_id === x.pub && Array.isArray(snap.positions) ? { positions: snap.positions } : {}),
        notice: priv
          ? "An admin confirmed that a question this item shares with another item contains student data. Remove those questions and republish, or appeal if your copy is different."
          : "A copyright takedown covers questions this item shares with the item taken down. Remove those questions and republish, or appeal if you have the right to share them.",
      });
    }
  }
  const items = rows.results
    .filter((r) => r.current_publish_id !== null || r.removed_reason === "privacy")
    .map((r) => {
      let state = "live";
      const extra: Record<string, unknown> = {};
      if (r.removed_reason === "author_deleted") {
        state = "deleted";
        if (r.purge_after !== null && r.purge_after > now) extra.undo_until = r.purge_after;
      } else if (r.removed_reason === "takedown") {
        state = "taken_down"; // "taken down after a copyright notice", with the counter-notice form
        extra.counter_notice = counterState(r.counter_notice_id, r.action_id, r.notified_at);
        if (extra.counter_notice === "available") extra.counter_notice_by = r.notified_at! + QUARANTINE_MS;
      } else if (r.removed_reason === "privacy") {
        // Purged at once (no quarantine for student data); shown to its author while it can be appealed.
        state = "removed_privacy";
        extra.notice = "An admin confirmed this item contained student data and removed it. If that is wrong, appeal within 14 days; you can then publish it again.";
        extra.appeal = "available";
        extra.appeal_until = (r.removed_at ?? now) + PRIVACY_APPEAL_MS;
      }
      else if (r.removed_reason !== null) {
        state = "removed";
        extra.removed_reason = r.removed_reason;
        if (r.removed_reason === "admin") extra.appeal = "available";
      }
      // A hold never says why or who reported: "held for review".
      else if (r.needs_reupload) state = "needs_reupload"; // an image was lost: upload it again and republish
      else if (r.key_leak) {
        state = "key_leak_review";
        extra.notice = "Teachers who use this item reported that what students see gives the hidden answer key away (a hint in a question or its choices). Fix the wording and republish: the hold ends with the changed version.";
      }
      else if (r.held) {
        state = "held_for_review";
        extra.appeal = "available";
        const n = held.get(r.id);
        if (n) Object.assign(extra, n);
      }
      return {
        id: r.id,
        kind: r.kind,
        title: r.title,
        subject: r.subject,
        version: r.version,
        visibility: r.visibility,
        license: r.license,
        hide_key: r.hide_key === 1,
        question_count: r.question_count,
        committed_at: r.committed_at,
        import_count: r.import_count,
        state,
        ...extra,
      };
    });
  const lastRow = rows.results[rows.results.length - 1] as { id: string; updated_at?: number } | undefined;
  // Round 23 (critic30b Z1): a takedown of an item the author deleted (purged before the notice) is
  // listed (first page) so she can counter it: its id, the action and the window's end; no content.
  const ghostTd =
    after === null
      ? (
          await c.db
            .prepare(
              `SELECT a.item_id AS id, a.id AS action_id, a.author_notified_at AS n, a.counter_notice_id FROM dmca_actions a JOIN items i ON i.id = a.item_id
                WHERE i.author_id = ?1 AND i.purged_at IS NOT NULL AND i.purged_at < a.disabled_at AND a.restored_at IS NULL AND a.purged_at IS NULL
                ORDER BY a.disabled_at DESC LIMIT 100`,
            )
            .bind(me)
            .all<{ id: string; action_id: string; n: number | null; counter_notice_id: string | null }>()
        ).results.map((x) => ({
          id: x.id,
          state: "takedown_pending",
          action_id: x.action_id,
          counter_notice_sent: x.counter_notice_id !== null,
          // (round 28: the server says whether the form is offered, by the POST's own rule)
          counter_notice: counterState(x.counter_notice_id, x.action_id, x.n),
          ...(x.n !== null ? { counter_notice_by: x.n + QUARANTINE_MS } : {}),
        }))
      : [];
  return json(200, {
    items: [...ghostTd, ...items],
    next_cursor: rows.results.length === MINE_PAGE && lastRow ? `${(rows.results[rows.results.length - 1] as unknown as { updated_at: number }).updated_at}:${lastRow.id}` : null,
    storage: { used: await usage(c, me), limit: await limitOf(c, me, "storage_bytes") },
  });
}

// ---- GET /items/:id/blobs/:sha256 -------------------------------------------------------------------

const EXT: Record<string, string> = { "image/png": "png", "image/jpeg": "jpg", "image/gif": "gif", "image/webp": "webp" };

export async function getBlob(c: Ctx, p: PoolPrincipal, id: string, sha: string): Promise<Response> {
  const me = p.userId;
  await missGuard(c, me, "blob_miss");
  const missBlob = async (): Promise<never> => {
    await countMiss(c, me, "blob_miss");
    throw notFound();
  };
  if (!ID_RE.test(id) || !SHA_RE.test(sha)) return missBlob();
  // Readable iff the caller may see the item AND the blob is listed under the caller's served form of
  // the CURRENT publish. A miss and a forbidden read are the same 404.
  const row = await c.db
    .prepare(
      `SELECT b.content_type FROM items i JOIN users u ON u.id = i.author_id
         JOIN item_publishes p ON p.publish_id = i.current_publish_id
         JOIN item_blobs ib ON ib.publish_id = i.current_publish_id AND ib.sha256 = ?3
              AND ib.form = CASE WHEN i.author_id = ?2 OR p.hide_key = 0 THEN 'full' ELSE 'stripped' END
         JOIN blobs b ON b.sha256 = ib.sha256
        WHERE i.id = ?1 AND ${visibleTo("?2")}
          -- An image confirmed as student data is served to nobody, from any item (critic A5).
          AND NOT EXISTS (SELECT 1 FROM privacy_blocks pb WHERE pb.kind = 'blob' AND pb.hash = ?3)
          -- Past its grace window it stays gone for every served use until uploaded again; only its
          -- author's own unserved (hidden full-form) use may still read it (round 6 F1).
          AND (b.privacy_purged_at IS NULL OR (i.author_id = ?2 AND p.hide_key = 1
               AND NOT EXISTS (SELECT 1 FROM item_blobs us WHERE us.publish_id = i.current_publish_id AND us.form = 'stripped' AND us.sha256 = ?3)))`,
    )
    .bind(id, me, sha)
    .first<{ content_type: string }>();
  if (!row) return missBlob();
  const o = await c.r2.get(`blobs/${sha}`);
  if (!o) return missBlob();
  return new Response(o.body, { // r2-object: a stored image, never a request
    status: 200,
    headers: {
      // Only ever the type verified from the bytes at upload.
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

export { DEFAULT_LIMITS };

/** Tags (incl. merged topics) and standards of a publish, served with the item and kept on import copies. */
async function tagsFor(c: Ctx, publishId: string): Promise<{ tags: string[]; standards: string[] }> {
  const [t, s] = await c.db.batch([
    c.db.prepare(`SELECT tag FROM item_tags WHERE publish_id = ?1 ORDER BY tag`).bind(publishId),
    c.db.prepare(`SELECT code FROM item_standards WHERE publish_id = ?1 ORDER BY code`).bind(publishId),
  ]);
  return { tags: (t!.results as { tag: string }[]).map((x) => x.tag), standards: (s!.results as { code: string }[]).map((x) => x.code) };
}

/**
 * Licences under which this account may publish a verbatim copy of what it was just served: per
 * question, every import it holds of a question with that match_hash (this one included) with the
 * chains they carry, combined by the one rule in license.ts. Exactly what publish computes.
 */
async function mayRepublishAs(c: Ctx, me: string, publishId: string, form: "full" | "stripped"): Promise<string[]> {
  const rows = await c.db
    .prepare(
      `SELECT match_hash, CASE WHEN ?2 = 'stripped' THEN stripped_content ELSE full_content END AS full, keyfree_content AS keyfree,
              CASE WHEN ?2 = 'stripped' THEN near_stripped ELSE near_full END AS near, near_kf, short,
              (?2 = 'full' AND qkind = 'Short' AND COALESCE(full_content, '') <> '') AS keywork FROM item_questions WHERE publish_id = ?1 ORDER BY pos`,
    )
    .bind(publishId, form)
    .all<{ match_hash: string; full: string; keyfree: string; near: string | null; near_kf: string | null; short: number; keywork: number }>();
  const hashes = rows.results.map((r) => r.match_hash);
  // A verbatim copy of what was served, published the only way it can be (hidden when the serve
  // was stripped): the same Subject, query, rules and licence decision publish uses (round 9).
  const subjects = rows.results.map((q) => subjectFor(form === "stripped", q.keyfree, q.full, q.near ?? "", q.near_kf ?? ""));
  const actx = await authorContext(c, me, hashes);
  const ev = await evidence(
    c,
    me,
    subjects.map((s, i) => ({ ...subjectParam(s, hashes[i]!), first: null, ownshown: s.full !== null && (actx.own.get(hashes[i]!)?.shown.has(s.full) ?? false), common: rows.results[i]!.short === 1, croots: actx.roots.get(hashes[i]!) ?? [], ck: actx.k, keywork: rows.results[i]!.keywork, ownkf: rows.results[i]!.keywork === 0 && actx.ownKf.has(rows.results[i]!.near_kf ?? "") })),
    { mine: [...actx.mine] },
  );
  return allowedForBank(rows.results.map((_, i) => decideLicence((ev.byPos.get(i) ?? []).filter((h) => h.by_match && isHard(h)).map(unitOf)).allowed));
}

/**
 * Records what a non-author was served (an import, or a 'view' for a plain read), only if the item
 * is STILL visible after the body was read: a takedown or hold that landed meanwhile turns it into
 * a 404 with nothing recorded (one atomic batch). A view never downgrades an import.
 */
async function recordServed(c: Ctx, me: string, r: ItemRow, form: "full" | "stripped", kind: "import" | "view"): Promise<boolean> {
  const now = c.now();
  // Round 19 (critic24c C1/C2): binding never depends on a per-account row cap. Reads are bounded by
  // the number of DISTINCT publishes read (`served`, one tiny row each; past reads_max a read of a
  // publish not served before is refused), imports by the daily allowance and a total of distinct
  // items (imports_max); neither ever blocks the other, and nothing recorded is ever evicted.
  // (The totals are counted only for something new to this account: a repeat read costs one lookup.)
  const q = await c.db
    .prepare(
      `SELECT x.seen, x.imported,
              CASE WHEN x.seen = 0 AND ?4 = 'view' THEN (SELECT COUNT(*) FROM served WHERE reader_id = ?1 AND kind = 'view') ELSE 0 END AS reads,
              CASE WHEN x.imported = 0 AND ?4 = 'import' THEN (SELECT COUNT(DISTINCT item_id) FROM imports WHERE user_id = ?1 AND kind = 'import') ELSE 0 END AS imports
         -- (Round 20 variant 2: an item already read, in any version, is no new read: a corrected
         -- version can always be read again.)
         FROM (SELECT EXISTS (SELECT 1 FROM served WHERE item_id = ?2 AND reader_id = ?1) AS seen,
                      EXISTS (SELECT 1 FROM imports WHERE user_id = ?1 AND item_id = ?2 AND kind = 'import') AS imported) x`,
    )
    .bind(me, r.id, r.current_publish_id, kind)
    .first<{ seen: number; reads: number; imported: number; imports: number }>();
  // (An import is never refused by the reads count: its own allowance bounds it.)
  if (kind === "view" && q && q.seen === 0 && q.reads >= (await limitOf(c, me, "reads_max")))
    throw new PoolError(413, "reads_quota", { message: "Your account has read more shared items than it can keep a record of. Ask an admin to raise your limit (reads_max)." });
  if (kind === "import" && q && q.imported === 0 && q.imports >= (await limitOf(c, me, "imports_max")))
    throw new PoolError(413, "imports_quota", { message: "Your account has imported as many shared items as it can (imports_max). Ask an admin to raise your limit." });
  await c.deps.beforeServe?.();
  // Round 20 (critic25b F2): recorded only while the publish read still exists (an author who
  // republishes during the read can prune it): checked in the same atomic batch, so a serve is never
  // recorded against a publish that left no ghost; the read is then a 404 (retry) with nothing recorded.
  const live = (p: string) => `EXISTS (SELECT 1 FROM item_publishes sp WHERE sp.publish_id = ${p} AND sp.status IN ('current', 'previous'))`;
  const cond = `EXISTS (SELECT 1 FROM items i JOIN users u ON u.id = i.author_id WHERE i.id = ?2 AND ${VISIBLE_TO_OTHERS})`;
  const stmts = [
    // Keyed on the PUBLIC publish id, so re-importing after a hidden-only republish is the same row.
    c.db
      .prepare(
        `INSERT INTO imports (user_id, item_id, publish_id, version, form, license, author_id, at, kind, real_publish_id, licenses_json, first_at)
         SELECT ?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, json_array(?6), ?8 WHERE ${cond} AND ${live("?10")} AND ${ALIVE_SQL("?1")}
         ON CONFLICT (user_id, item_id, publish_id) DO UPDATE SET at = excluded.at, real_publish_id = excluded.real_publish_id, license = excluded.license,
           licenses_json = (SELECT json_group_array(v) FROM (SELECT DISTINCT value AS v FROM json_each(COALESCE(imports.licenses_json, json_array(imports.license)))
                                                               UNION SELECT excluded.license)),
           kind = CASE WHEN excluded.kind = 'import' THEN 'import' ELSE imports.kind END`,
      )
      .bind(me, r.id, r.public_publish_id, r.p_public_version, form, r.p_license, r.author_id, now, kind, r.current_publish_id),
    // The binding evidence: this reader was served THIS publish, under this licence (round 19).
    c.db
      .prepare(
        `INSERT INTO served (reader_id, item_id, publish_id, kind, license, licenses_json, first_at, at)
         SELECT ?1, ?2, ?3, ?4, ?5, json_array(?5), ?6, ?6 WHERE ${cond} AND ${live("?3")} AND ${ALIVE_SQL("?1")}
         ON CONFLICT (reader_id, publish_id) DO UPDATE SET at = excluded.at, license = excluded.license,
           kind = CASE WHEN excluded.kind = 'import' THEN 'import' ELSE served.kind END,
           licenses_json = (SELECT json_group_array(DISTINCT v) FROM (SELECT value AS v FROM json_each(served.licenses_json) UNION SELECT excluded.license))`,
      )
      .bind(me, r.id, r.current_publish_id, kind, r.p_license, now),
  ];
  stmts.push(c.db.prepare(`SELECT 1 AS ok WHERE ${cond.replaceAll("?2", "?1")} AND ${live("?2")}`).bind(r.id, r.current_publish_id));
  const res = await c.db.batch(stmts);
  // Round 27 (critic35d B-1): both inserts carry the account's own "not erased" guard; a request that
  // was authenticated before the erasure committed records nothing and answers like an unauthenticated one.
  if ((res[0]!.meta.changes ?? 0) === 0) await throwIfErased(c, me);
  return (res[res.length - 1]!.results as unknown[]).length > 0;
}

export const IMPORT_IMAGE_HOLD_MS = 30 * DAY_MS;
/** How long a privacy-purged item is listed to its author and appealable (the bytes' grace window). */
export const PRIVACY_APPEAL_MS = 14 * DAY_MS;

/** Holds the served images of an import for the importer, if their quota allows; one statement. */
async function holdImportedImages(c: Ctx, me: string, publishId: string, form: "full" | "stripped"): Promise<{ held: number; until?: number; reason?: string }> {
  const now = c.now();
  const until = now + IMPORT_IMAGE_HOLD_MS;
  const quota = await limitOf(c, me, "storage_bytes");
  const n = await c.db.prepare(`SELECT COUNT(*) AS n FROM item_blobs WHERE publish_id = ?1 AND form = ?2`).bind(publishId, form).first<{ n: number }>();
  if (!n?.n) return { held: 0 };
  const r = await c.db
    .prepare(
      `INSERT INTO pending_blobs (sha256, uploader_id, size, until)
       SELECT b.sha256, ?1, b.size, ?5 FROM item_blobs ib JOIN blobs b ON b.sha256 = ib.sha256
        WHERE ib.publish_id = ?3 AND ib.form = ?4 AND ${ALIVE_SQL("?1")}
          AND ${USAGE_SQL} + (SELECT COALESCE(SUM(b2.size), 0) FROM item_blobs ib2 JOIN blobs b2 ON b2.sha256 = ib2.sha256
                               WHERE ib2.publish_id = ?3 AND ib2.form = ?4
                                 AND NOT EXISTS (SELECT 1 FROM pending_blobs pb WHERE pb.sha256 = b2.sha256 AND pb.uploader_id = ?1 AND pb.until >= ?2)) <= ?6
       ON CONFLICT (sha256, uploader_id) DO UPDATE SET until = MAX(until, excluded.until)`,
    )
    .bind(me, now, publishId, form, until, quota)
    .run();
  if ((r.meta.changes ?? 0) === 0) await throwIfErased(c, me); // (round 27)
  return (r.meta.changes ?? 0) > 0 ? { held: n.n, until } : { held: 0, reason: "quota" };
}
