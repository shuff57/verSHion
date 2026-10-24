// Round 32: sending a version of a circle bank (docs/circles-plan.md 4.1, 6.2; phase A).
//
// POST /circles/:cid/banks (owner, editor): the first version of a new bank.
// PUT /circles/:cid/banks/:bid (phase A: the bank's AUTHOR, while owner or editor): the next version,
// accepted only if the head is still `base_version` (compare-and-set, like a public republish's
// base_publish_id); otherwise `409 conflict` with the true head, and nothing is stored.
//
// The body is the publish format (`<meta JSON>\n<payload>`, application/vnd.vershion.publish), read by the
// one body reader. A send is publish-sized work: it holds the account's publish lock (pool_locks, so a
// teacher cannot publish and send at once: 409 publish_in_progress), waits for the isolate's heavy slot
// and reserves PUBLISH_COST_PER_BYTE per body byte. share-core validates it once (publish_prepare, no
// strip: every member sees the keys, D12) and gives the key-free match hashes the public re-share check
// needs (circle_qkeys). Order: stage the version row (status 'staging', guarded by ALIVE_SQL), put the
// body and the per-question index to R2, then ONE commit batch whose first statement is the
// compare-and-set (with every cap of the circle checked in it: D2-D4); every other statement of the
// batch follows it by "this version is now the head". A lost commit deletes the staging row and its R2
// objects; one the isolate never finished is collected by the hourly cron.
import { PARSE_BYTES_PER_VALUE, scanJson } from "./jsonscan.ts";
import { isFail, questionsOf, type Fail, type PayloadJson, type QuestionJson } from "./core.ts";
import { EXT } from "./image.ts";
import { allowedByAll, ATTEST_STATEMENT, isLicense, POOL_TERMS_VERSION, type License } from "./license.ts";
import { deleteKeys } from "./moderation.ts";
import { acquireAccountLock, acquireHeavy, BODY_DEADLINE_MS, HEAVY_QUEUE_WAIT_MS, IN_FLIGHT_MS, MAX_META, MAX_PAYLOAD, refreshAccountLock, releaseAccountLock } from "./publish.ts";
import { ACTIVE_SQL, bankOrMiss, circleOrMiss, KEPT_VERSIONS, nameOf, OPEN_SQL, requireOpen, requireRole } from "./circles.ts";
import {
  ALIVE_SQL,
  audit,
  DAY_MS,
  giveDaily,
  ID_RE,
  isPublishType,
  json,
  limitOf,
  newId,
  parsePublish,
  PoolError,
  PUBLISH_COST_PER_BYTE,
  PUBLISH_ID_RE,
  readBody,
  requireDaily,
  reserveBytes,
  sha256Hex,
  throwIfErased,
  visibleTo,
  type Ctx,
  type PoolPrincipal,
} from "./shared.ts";
import { cleanText } from "../../../auth/src/validate.ts";

export const MESSAGE_MAX = 500;
const MEDIA_RE = /^media\/([0-9a-f]{64})\.(png|jpg|gif|webp)$/;
const enc = new TextEncoder();

/** The circle's kept bytes (D3): one indexed SUM over its current and previous versions. */
export const BYTES_SQL = (cid: string) => `(SELECT COALESCE(SUM(sv.bytes), 0) FROM circle_versions sv WHERE sv.circle_id = ${cid} AND sv.status IN ('current', 'previous'))`;
/** Questions across the circle's current versions, every bank not purged but `except` (D4). */
export const QUESTIONS_SQL = (cid: string, except: string) =>
  `(SELECT COALESCE(SUM(sb.question_count), 0) FROM circle_banks sb WHERE sb.circle_id = ${cid} AND sb.purged_at IS NULL AND sb.id <> ${except})`;
/** Banks of the circle that are not purged (removed ones count until purged). */
export const BANKS_SQL = (cid: string) => `(SELECT COUNT(*) FROM circle_banks nb WHERE nb.circle_id = ${cid} AND nb.purged_at IS NULL)`;

interface SendMeta {
  commitId: string;
  license: License;
  message: string | null;
  base: number | null;
  meta: unknown;
}

function readSendMeta(m: Record<string, unknown>, isNew: boolean): SendMeta {
  const bad = (field: string) => new PoolError(400, "invalid_request", { field });
  if (typeof m.commit_id !== "string" || !PUBLISH_ID_RE.test(m.commit_id)) throw bad("commit_id");
  // Chosen explicitly on every send, like a public publish's.
  if (!isLicense(m.license)) throw bad("license");
  // D1: the own-work attestation is ticked on every send (private circles are where copied publisher
  // test banks would hide). Recorded, not verified.
  const at = m.attest as Record<string, unknown> | undefined;
  if (!at || typeof at !== "object" || Array.isArray(at) || at.own_work !== true || at.terms_version !== POOL_TERMS_VERSION)
    throw new PoolError(422, "attestation_required", { terms_version: POOL_TERMS_VERSION, statement: ATTEST_STATEMENT });
  let message: string | null = null;
  if (m.message !== undefined && m.message !== null && m.message !== "") {
    message = cleanText(m.message, MESSAGE_MAX);
    if (!message) throw bad("message");
  }
  let base: number | null = null;
  if (isNew) {
    if (m.base_version !== undefined && m.base_version !== null) throw bad("base_version");
  } else {
    if (typeof m.base_version !== "number" || !Number.isInteger(m.base_version) || m.base_version < 1) throw bad("base_version");
    base = m.base_version;
  }
  return { commitId: m.commit_id, license: m.license, message, base, meta: m.meta ?? {} };
}

function invalid(f: Fail): PoolError {
  const extra: Record<string, unknown> = { errors: f.errors };
  if (f.formatting_findings) {
    extra.formatting_findings = f.formatting_findings;
    extra.formatting_findings_total = f.formatting_findings_total ?? f.formatting_findings.length;
  }
  return new PoolError(422, "invalid_payload", extra);
}

const isObj = (x: unknown): x is Record<string, unknown> => typeof x === "object" && x !== null && !Array.isArray(x);

/** POST /circles/:cid/banks (bid null) and PUT /circles/:cid/banks/:bid. */
export async function sendVersion(c: Ctx, req: Request, p: PoolPrincipal, cid: string, pathBid: string | null): Promise<Response> {
  const me = p.userId;
  const isNew = pathBid === null;
  // Membership and role first (cheap; a non-member is a counted 404 whatever the body).
  const found = isNew ? { m: await circleOrMiss(c, cid, me), b: null } : await bankOrMiss(c, cid, pathBid, me);
  const m = found.m;
  requireOpen(m);
  requireRole(m, "owner", "editor");
  if (found.b) {
    // Phase A: only the bank's author sends new versions of it (phase B: editors and the owner).
    if (found.b.author_id !== me) throw new PoolError(403, "circle_role_required", { needed: "author", role: m.role });
    if (found.b.removed_at !== null) throw new PoolError(409, "bank_removed", { reason: found.b.removed_reason });
  }
  if (!isPublishType(req)) throw new PoolError(415, "unsupported_media_type");
  await requireDaily(c, me, "circle_commits");
  const refund = () => giveDaily(c, me, "circle_commits").catch(() => {});
  const lock = await acquireAccountLock(c, me);
  if (!lock) {
    await refund();
    await throwIfErased(c, me);
    throw new PoolError(409, "publish_in_progress", { message: "Another publish or send from this account is still running." }, { "Retry-After": "5" });
  }
  const held: (() => void)[] = [];
  const refresh = () => refreshAccountLock(c, me, lock);
  try {
    const maxPayload = c.deps.maxPublishBytes ?? MAX_PAYLOAD;
    const body = await readBody(req, { max: maxPayload + MAX_META + 1, key: `u:${me}`, deadlineMs: c.deps.bodyDeadlineMs ?? BODY_DEADLINE_MS });
    held.push(body.release);
    const declared = body.bytes.length;
    const read = parsePublish(body.bytes, maxPayload, MAX_META);
    await refresh();
    held.push(await acquireHeavy(c.deps.heavyQueueMs ?? HEAVY_QUEUE_WAIT_MS, refresh));
    // Round 33 (critic40b B4): the publish's own formula (publish.ts, round 16 critic21d 1): a payload of
    // many tiny values costs more to parse than its bytes say, so it is reserved by its value count when that
    // is more.
    const scan = scanJson(read.payload);
    held.push(reserveBytes(Math.max(declared * (PUBLISH_COST_PER_BYTE - 1), scan.values * PARSE_BYTES_PER_VALUE * 2)));
    const res = await sendOnce(c, me, cid, found.b, read, refresh, scan.ok);
    if (res.replayed || res.status === 409) await refund();
    return res.response;
  } catch (e) {
    if (e instanceof PoolError && (e.status === 409 || e.status === 503)) await refund();
    throw e;
  } finally {
    for (const r of held) r();
    await releaseAccountLock(c, me, lock);
  }
}

interface Bank {
  id: string;
  head_version: number;
  head_version_id: string | null;
  author_id: string;
}

async function sendOnce(
  c: Ctx,
  me: string,
  cid: string,
  bank: Bank | null,
  read: { raw: Uint8Array; meta: Record<string, unknown>; payload: Uint8Array },
  refresh: () => Promise<void>,
  scanOk: boolean,
): Promise<{ response: Response; replayed?: boolean; status: number }> {
  const isNew = bank === null;
  const sm = readSendMeta(read.meta, isNew);
  const requestSha = await sha256Hex(read.raw);
  const now = c.now();

  // ---- idempotency (commit_id) ------------------------------------------------------------------------
  const prior = await c.db
    .prepare(`SELECT id, bank_id, circle_id, committer_id, request_sha256, result_json, status, full_key, index_key, created_at FROM circle_versions WHERE commit_id = ?1`)
    .bind(sm.commitId)
    .first<{ id: string; bank_id: string; circle_id: string; committer_id: string; request_sha256: string; result_json: string | null; status: string; full_key: string; index_key: string; created_at: number }>();
  if (prior) {
    // Someone else's id, or the same id with another body or for another bank: never resumed, never revealed.
    if (prior.committer_id !== me || prior.request_sha256 !== requestSha || prior.circle_id !== cid || (bank !== null && prior.bank_id !== bank.id)) throw new PoolError(409, "commit_id_conflict");
    if (prior.status !== "staging" && prior.result_json) return { response: json(200, { ...(JSON.parse(prior.result_json) as object), replayed: true }), replayed: true, status: 200 };
    if (prior.created_at > now - IN_FLIGHT_MS) throw new PoolError(409, "publish_in_progress", {}, { "Retry-After": "5" });
    // An abandoned attempt: start again with the same commit_id.
    const del = await c.db.prepare(`DELETE FROM circle_versions WHERE id = ?1 AND status = 'staging' AND created_at = ?2`).bind(prior.id, prior.created_at).run();
    if ((del.meta.changes ?? 0) !== 1) throw new PoolError(409, "publish_in_progress", {}, { "Retry-After": "5" });
    await deleteKeys(c, [prior.full_key, prior.index_key].filter((k) => k !== ""));
  }
  // The head the sender based on (checked again in the commit's compare-and-set).
  if (bank && bank.head_version !== sm.base) throw await conflict(c, bank.id);

  // ---- the payload: shape, qids, images, origins, rewrite ------------------------------------------------
  const payload = read.payload;
  const refuseRaw = (): never => {
    const f = c.core.publishPrepare(payload, { hideKey: false, labels: [], orderAcks: [], imageOk: [] });
    if (isFail(f)) throw invalid(f);
    throw new PoolError(422, "invalid_payload", { errors: [{ code: "invalid", path: "" }] });
  };
  if (!scanOk) refuseRaw();
  let raw: PayloadJson | null = null;
  try {
    raw = JSON.parse(new TextDecoder("utf-8", { fatal: true, ignoreBOM: false }).decode(payload)) as PayloadJson;
  } catch {
    refuseRaw();
  }
  // A circle bank is a bank (one question is a bank of one).
  if (!(isObj(raw) && typeof raw.schema_version === "number" && isObj(raw.bank) && Array.isArray(raw.bank.questions) && raw.bank.questions.length > 0 && raw.bank.questions.every(isObj) && raw.question === undefined))
    throw new PoolError(422, "invalid_payload", { errors: [{ code: "bank_required", path: "bank" }] });
  const rb = raw!.bank!;
  const qs = rb.questions as QuestionJson[];
  const n = qs.length;
  const bid = bank ? bank.id : newId();

  // qids: a sent qid (question.published.qid) is kept only if the head held it and it appears once.
  let prevIdx: { qids: string[]; h: string[] } | null = null;
  if (bank?.head_version_id) {
    const v = await c.db.prepare(`SELECT index_key FROM circle_versions WHERE id = ?1`).bind(bank.head_version_id).first<{ index_key: string }>();
    const o = v && v.index_key ? await c.r2.get(v.index_key) : null;
    if (o) {
      try {
        const x = JSON.parse(await o.text()) as { qids?: unknown; h?: unknown }; // r2-object: our own index
        if (Array.isArray(x.qids) && Array.isArray(x.h) && x.qids.length === x.h.length) prevIdx = { qids: x.qids as string[], h: x.h as string[] };
      } catch {
        prevIdx = null;
      }
    }
  }
  const prevQids = new Set(prevIdx?.qids ?? []);
  const sent = qs.map((q) => (isObj(q.published) && typeof q.published.qid === "string" ? q.published.qid : null));
  const counts = new Map<string, number>();
  for (const s of sent) if (s) counts.set(s, (counts.get(s) ?? 0) + 1);
  const qids = sent.map((s) => (s && ID_RE.test(s) && prevQids.has(s) && counts.get(s) === 1 ? s : newId()));

  // Images: verified, and the sender's (pending, in one of their publishes) or already in this bank's kept versions.
  const imgOf = (q: QuestionJson) => (q.image && typeof q.image === "object" && typeof q.image.file === "string" ? MEDIA_RE.exec(q.image.file)?.[1] ?? null : null);
  const shas = [...new Set(qs.map(imgOf).filter((x): x is string => x !== null))];
  const blobInfo = new Map<string, { content_type: keyof typeof EXT; width: number; height: number; size: number; privacy_purged_at: number | null }>();
  if (shas.length > 0) {
    const rows = await c.db
      .prepare(
        `SELECT b.sha256, b.content_type, b.width, b.height, b.size, b.privacy_purged_at FROM blobs b
          WHERE b.sha256 IN (SELECT value FROM json_each(?1))
            AND (EXISTS (SELECT 1 FROM pending_blobs pb WHERE pb.sha256 = b.sha256 AND pb.uploader_id = ?2 AND pb.until >= ?3)
              OR EXISTS (SELECT 1 FROM item_blobs ib JOIN item_publishes ip ON ip.publish_id = ib.publish_id WHERE ib.sha256 = b.sha256 AND ip.author_id = ?2)
              OR EXISTS (SELECT 1 FROM circle_version_blobs vb JOIN circle_versions cv ON cv.id = vb.version_id WHERE vb.sha256 = b.sha256 AND cv.bank_id = ?4 AND cv.status IN ('current', 'previous')))`,
      )
      .bind(JSON.stringify(shas), me, now, bid)
      .all<{ sha256: string; content_type: keyof typeof EXT; width: number; height: number; size: number; privacy_purged_at: number | null }>();
    for (const r of rows.results) blobInfo.set(r.sha256, r);
    const missing = qs.map((q, i) => [i, imgOf(q)] as const).filter(([, s]) => s !== null && !blobInfo.has(s!)).map(([i]) => i);
    if (missing.length > 0) throw new PoolError(422, "blob_missing", { positions: missing.slice(0, 100) });
    // The sender's hold on each image is extended until the commit (the GC cannot collect it meanwhile).
    await c.db
      .prepare(
        `INSERT INTO pending_blobs (sha256, uploader_id, size, until) SELECT sha256, ?2, size, ?3 FROM blobs WHERE sha256 IN (SELECT value FROM json_each(?1)) AND ${ALIVE_SQL("?2")}
         ON CONFLICT (sha256, uploader_id) DO UPDATE SET until = MAX(until, excluded.until)`,
      )
      .bind(JSON.stringify(shas), me, now + DAY_MS)
      .run();
    // The bytes must really be in R2 (a privacy-blocked image's never are: never refused here, as at a
    // public publish, so a send cannot confirm a block; the circle's blob route never serves it).
    const blockedImg = new Set(
      (await c.db.prepare(`SELECT hash FROM privacy_blocks WHERE kind = 'blob' AND hash IN (SELECT value FROM json_each(?1))`).bind(JSON.stringify(shas)).all<{ hash: string }>()).results.map((x) => x.hash),
    );
    for (const s of shas)
      if (!blockedImg.has(s) && blobInfo.get(s)!.privacy_purged_at === null && !(await c.r2.head(`blobs/${s}`))) {
        await c.db.prepare(`UPDATE blobs SET missing_at = COALESCE(missing_at, ?2) WHERE sha256 = ?1`).bind(s, now).run();
        throw new PoolError(422, "blob_missing", { reason: "reupload", positions: qs.map((q, i) => (imgOf(q) === s ? i : -1)).filter((i) => i >= 0).slice(0, 100) });
      }
  }

  // Public material sent into a circle bank (plan section 7): the bank's licence must be one each DECLARED
  // origin allows (the server checks declared origins only; an origin the sender cannot see is ignored;
  // her own items allow anything). Recorded with the version (members see the credits).
  const declared = qs.map((q, i) => (isObj(q.origin) && typeof q.origin.item_id === "string" && ID_RE.test(q.origin.item_id) ? { pos: i, item_id: q.origin.item_id as string, qid: typeof q.origin.qid === "string" ? q.origin.qid : null, version: typeof q.origin.version === "number" ? q.origin.version : null } : null)).filter((x) => x !== null);
  const origins: { pos: number; item_id: string; qid: string | null; version: number | null; license: string; author_name: string }[] = [];
  if (declared.length > 0) {
    const its = await c.db
      .prepare(`SELECT i.id, i.license, i.author_id, i.anonymized, u.display_name, u.deleted_at FROM items i JOIN users u ON u.id = i.author_id WHERE i.id IN (SELECT value FROM json_each(?1)) AND ${visibleTo("?2")}`)
      .bind(JSON.stringify([...new Set(declared.map((d) => d.item_id))]), me)
      .all<{ id: string; license: string; author_id: string; anonymized: number; display_name: string | null; deleted_at: number | null }>();
    const by = new Map(its.results.map((r) => [r.id, r]));
    for (const d of declared) {
      const it = by.get(d.item_id);
      if (!it || it.author_id === me) continue;
      origins.push({ ...d, license: it.license, author_name: it.anonymized === 1 ? "deleted teacher" : nameOf(it.display_name, it.deleted_at) });
    }
    const allowed = allowedByAll(origins.map((o) => o.license));
    if (!allowed.includes(sm.license)) {
      const bad = origins.filter((o) => !allowedByAll([o.license]).includes(sm.license)).map((o) => o.pos);
      throw new PoolError(422, "license_incompatible", { positions: bad.slice(0, 100), allowed, message: "Some questions came from the public pool under a licence that does not allow the one chosen for this bank." });
    }
  }

  // Rewritten in place: ids, dates, provenance dropped (the server records the checked origins), image
  // references from the verified blobs.
  const day = Math.floor(now / DAY_MS) * DAY_MS;
  const bankName = typeof rb.name === "string" ? rb.name : "";
  rb.id = bid;
  rb.created = day;
  rb.modified = day;
  delete rb.origin;
  delete rb.published;
  qs.forEach((q, i) => {
    q.id = qids[i]!;
    q.created = day;
    q.modified = day;
    delete q.origin;
    delete q.published;
    const s = imgOf(q);
    if (s && q.image) {
      const b = blobInfo.get(s)!;
      q.image = { file: `media/${s}.${EXT[b.content_type]}`, width: b.width, height: b.height };
    }
  });
  let rBytes: Uint8Array | null = enc.encode(JSON.stringify(raw));
  raw = null;
  (read as { payload: Uint8Array | null }).payload = null;

  // ---- share-core: one validation (no strip), the key-free match hashes ----------------------------------
  const vm = c.core.validateMeta(sm.meta);
  if (isFail(vm)) throw invalid(vm);
  const prep = c.core.publishPrepare(rBytes!, { hideKey: false, labels: [], orderAcks: [], imageOk: [] });
  rBytes = null;
  if (isFail(prep)) throw invalid(prep);
  const full = prep.full;
  const fullQs = questionsOf(full.payload);
  if (!full.payload.bank || fullQs.length !== n) throw new PoolError(422, "invalid_payload", { errors: [{ code: "invalid", path: "" }] });
  const contents = await c.core.contentHashes(fullQs);
  const hasImages = fullQs.some((q) => q.image) ? 1 : 0;
  (full as { payload: unknown }).payload = null;
  fullQs.length = 0;
  await refresh();

  // ---- what changed (per qid), the bytes, the records ------------------------------------------------------
  const prevH = new Map((prevIdx?.qids ?? []).map((q, k) => [q, prevIdx!.h[k]!]));
  const newSet = new Set(qids);
  const added = qids.filter((q) => !prevH.has(q)).length;
  const changed = qids.filter((q, k) => prevH.has(q) && prevH.get(q) !== contents[k]).length;
  const removed = [...prevH.keys()].filter((q) => !newSet.has(q)).length;
  const index = enc.encode(JSON.stringify({ qids, h: contents }));
  const imageBytes = shas.reduce((a, s) => a + (blobInfo.get(s)?.size ?? 0), 0);
  const bytes = full.bytes.length + index.length + imageBytes;
  const hashes = [...new Set(prep.matchHashes)];
  const newKeys =
    (
      await c.db
        .prepare(`SELECT COUNT(*) AS n FROM json_each(?1) j WHERE NOT EXISTS (SELECT 1 FROM circle_qkeys k WHERE k.circle_id = ?2 AND k.match_hash = j.value AND k.bank_id = ?3)`)
        .bind(JSON.stringify(hashes), cid, bid)
        .first<{ n: number }>()
    )?.n ?? hashes.length;
  const caps = {
    bytes: await limitOf(c, "*", "circle_bytes"),
    questions: await limitOf(c, "*", "circle_questions"),
    records: await limitOf(c, "*", "circle_records"),
    banks: await limitOf(c, "*", "circle_banks"),
  };
  const meta = { title: (vm.title || cleanText(bankName, 200) || "").slice(0, 200), subject: vm.subject, course: vm.course, grade: vm.grade };

  // ---- stage, R2, commit -------------------------------------------------------------------------------------
  const vid = newId();
  const fullKey = `circles/${cid}/${bid}/${vid}.json`;
  const indexKey = `circles/${cid}/${bid}/${vid}.idx.json`;
  const version = isNew ? 1 : sm.base! + 1;
  const result = { circle_id: cid, bank_id: bid, version, version_id: vid, qids, added, changed, removed };
  const staged = await c.db
    .prepare(
      `INSERT INTO circle_versions (id, bank_id, circle_id, version, commit_id, request_sha256, committer_id, base_version, message, license, status, full_key, index_key, bytes, question_count, added, changed, removed, origins, created_at)
       SELECT ?1, ?2, ?3, NULL, ?4, ?5, ?6, ?7, ?8, ?9, 'staging', ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18 WHERE ${ALIVE_SQL("?6")}`,
    )
    .bind(vid, bid, cid, sm.commitId, requestSha, me, sm.base, sm.message, sm.license, fullKey, indexKey, bytes, n, added, changed, removed, origins.length ? JSON.stringify(origins) : null, now)
    .run();
  if ((staged.meta.changes ?? 0) !== 1) {
    await throwIfErased(c, me);
    throw new PoolError(409, "publish_in_progress", {}, { "Retry-After": "5" });
  }
  await c.r2.put(fullKey, full.bytes, { httpMetadata: { contentType: "application/json" } });
  await c.r2.put(indexKey, index, { httpMetadata: { contentType: "application/json" } });
  await refresh();
  const won = `EXISTS (SELECT 1 FROM circle_banks wb WHERE wb.id = '${bid}' AND wb.head_version_id = '${vid}')`;
  const cas = isNew
    ? c.db
        .prepare(
          `INSERT INTO circle_banks (id, circle_id, author_id, title, subject, course, grade, license, head_version, head_version_id, question_count, has_images, created_at, updated_at)
           SELECT ?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, 1, ?9, ?10, ?11, ?12, ?12
            WHERE ${OPEN_SQL("?2")} AND ${ACTIVE_SQL("?2", "?3", ["owner", "editor"])} AND ${ALIVE_SQL("?3")}
              AND ${BANKS_SQL("?2")} < ?13 AND ${BYTES_SQL("?2")} + ?14 <= ?15 AND ${QUESTIONS_SQL("?2", "?1")} + ?10 <= ?16
              AND (SELECT qkeys FROM circles WHERE id = ?2) + ?17 <= ?18
              AND EXISTS (SELECT 1 FROM circle_versions sv WHERE sv.id = ?9 AND sv.status = 'staging')`,
        )
        .bind(bid, cid, me, meta.title, meta.subject, meta.course, meta.grade, sm.license, vid, n, hasImages, now, caps.banks, bytes, caps.bytes, caps.questions, newKeys, caps.records)
    : c.db
        .prepare(
          `UPDATE circle_banks SET head_version = head_version + 1, head_version_id = ?4, title = ?5, subject = ?6, course = ?7, grade = ?8, license = ?9, question_count = ?10, has_images = ?11, updated_at = ?12
            WHERE id = ?1 AND circle_id = ?2 AND author_id = ?3 AND head_version = ?13 AND removed_at IS NULL AND purged_at IS NULL
              AND ${OPEN_SQL("?2")} AND ${ACTIVE_SQL("?2", "?3", ["owner", "editor"])} AND ${ALIVE_SQL("?3")}
              AND ${BYTES_SQL("?2")} + ?14 <= ?15 AND ${QUESTIONS_SQL("?2", "?1")} + ?10 <= ?16
              AND (SELECT qkeys FROM circles WHERE id = ?2) + ?17 <= ?18
              AND EXISTS (SELECT 1 FROM circle_versions sv WHERE sv.id = ?4 AND sv.status = 'staging')`,
        )
        .bind(bid, cid, me, vid, meta.title, meta.subject, meta.course, meta.grade, sm.license, n, hasImages, now, sm.base, bytes, caps.bytes, caps.questions, newKeys, caps.records);
  const res = await c.db.batch([
    // (the sequence number this commit's "first sent" records carry; a lost commit leaves a harmless gap)
    c.db.prepare(`UPDATE pool_seq SET n = n + 1 WHERE id = 1`),
    cas,
    c.db.prepare(`UPDATE circle_versions SET status = 'previous' WHERE bank_id = ?1 AND status = 'current' AND id <> ?2 AND ${won}`).bind(bid, vid),
    c.db
      .prepare(`UPDATE circle_versions SET status = 'current', version = ?2, seq = (SELECT n FROM pool_seq WHERE id = 1), committed_at = ?3, result_json = ?4 WHERE id = ?1 AND status = 'staging' AND ${won}`)
      .bind(vid, version, now, JSON.stringify(result)),
    c.db.prepare(`INSERT OR IGNORE INTO circle_version_blobs (version_id, sha256) SELECT ?1, value FROM json_each(?2) WHERE ${won}`).bind(vid, JSON.stringify(shas)),
    c.db
      .prepare(
        `INSERT OR IGNORE INTO circle_qkeys (circle_id, match_hash, bank_id, first_version, first_seq, first_by)
         SELECT ?1, j.value, ?2, ?3, (SELECT n FROM pool_seq WHERE id = 1), ?4 FROM json_each(?5) j WHERE ${won}`,
      )
      .bind(cid, bid, version, me, JSON.stringify(hashes)),
    c.db.prepare(`UPDATE circles SET qkeys = qkeys + changes() WHERE id = ?1 AND ${won}`).bind(cid),
    // D16: the last KEPT_VERSIONS versions are kept in full; older ones keep their history line (their
    // images' rows go now; the daily cron deletes their R2 objects). Round 33 (critic40b V2): never while
    // the bank is held, and never a version an open report names (the evidence stays for the admin; the
    // next send after the case is resolved prunes it).
    c.db
      .prepare(
        `UPDATE circle_versions SET status = 'pruned', pruned_at = ?3 WHERE bank_id = ?1 AND status = 'previous' AND version <= ?2 AND ${won}
           AND NOT EXISTS (SELECT 1 FROM circle_holds ph WHERE ph.bank_id = ?1)
           AND NOT EXISTS (SELECT 1 FROM circle_reports pr WHERE pr.bank_id = ?1 AND pr.resolved_at IS NULL AND pr.version = circle_versions.version)`,
      )
      .bind(bid, version - KEPT_VERSIONS, now),
    c.db.prepare(`DELETE FROM circle_version_blobs WHERE version_id IN (SELECT id FROM circle_versions WHERE bank_id = ?1 AND status = 'pruned') AND ${won}`).bind(bid),
    // (the own-work affirmation, recorded with the version as a publish's is)
    c.db.prepare(`INSERT OR IGNORE INTO attestations (publish_id, item_id, user_id, at, terms_version) SELECT ?1, ?2, ?3, ?4, ?5 WHERE ${won}`).bind(vid, bid, me, now, POOL_TERMS_VERSION),
    audit(c.db, { actorId: me, actorKind: "user", action: isNew ? "circle.bank_add" : "circle.version", itemId: cid, targetUserId: me, detail: { bank_id: bid, version, version_id: vid, added, changed, removed, license: sm.license } }, now, won),
  ]);
  if ((res[1]!.meta.changes ?? 0) !== 1) {
    // Lost: the staging row and its objects go now; the reason is read from the state.
    await c.db.prepare(`DELETE FROM circle_versions WHERE id = ?1 AND status = 'staging'`).bind(vid).run();
    await deleteKeys(c, [fullKey, indexKey]);
    throw await whyLost(c, me, cid, bank ? bid : null, { base: sm.base, bytes, n, newKeys, caps });
  }
  return { response: json(isNew ? 201 : 200, result), status: isNew ? 201 : 200 };
}

/** 409 conflict with the true head (who sent it, when). */
async function conflict(c: Ctx, bid: string): Promise<PoolError> {
  const h = await c.db
    .prepare(
      `SELECT b.head_version, v.committed_at, u.display_name, u.deleted_at FROM circle_banks b LEFT JOIN circle_versions v ON v.id = b.head_version_id LEFT JOIN users u ON u.id = v.committer_id WHERE b.id = ?1`,
    )
    .bind(bid)
    .first<{ head_version: number; committed_at: number | null; display_name: string | null; deleted_at: number | null }>();
  return new PoolError(409, "conflict", {
    head_version: h?.head_version ?? null,
    head_at: h?.committed_at ?? null,
    head_by: h ? nameOf(h.display_name, h.deleted_at) : null,
    message: "The circle bank changed since you got it. Get the latest changes and merge them, then send again.",
  });
}

/** Why a commit's compare-and-set changed nothing, from the state now. */
async function whyLost(c: Ctx, me: string, cid: string, bid: string | null, w: { base: number | null; bytes: number; n: number; newKeys: number; caps: { bytes: number; questions: number; records: number; banks: number } }): Promise<PoolError> {
  await throwIfErased(c, me);
  const s = await c.db
    .prepare(
      `SELECT ci.closed_at, ci.close_reason, ci.undo_until, ci.qkeys, m.role, ${BYTES_SQL("?1")} AS used, ${QUESTIONS_SQL("?1", "?3")} AS questions, ${BANKS_SQL("?1")} AS banks,
              b.head_version, b.removed_at, b.removed_reason, b.author_id
         FROM circles ci LEFT JOIN circle_members m ON m.circle_id = ci.id AND m.user_id = ?2 AND m.left_at IS NULL LEFT JOIN circle_banks b ON b.id = ?3 AND b.circle_id = ci.id
        WHERE ci.id = ?1`,
    )
    .bind(cid, me, bid ?? "")
    .first<{ closed_at: number | null; close_reason: string | null; undo_until: number | null; qkeys: number; role: string | null; used: number; questions: number; banks: number; head_version: number | null; removed_at: number | null; removed_reason: string | null; author_id: string | null }>();
  if (!s || s.role === null) return new PoolError(404, "not_found");
  if (s.closed_at !== null) return new PoolError(409, "circle_closed", { reason: s.close_reason, undo_until: s.undo_until });
  if (s.role !== "owner" && s.role !== "editor") return new PoolError(403, "circle_role_required", { needed: "editor", role: s.role });
  if (bid !== null) {
    if (s.removed_at !== null) return new PoolError(409, "bank_removed", { reason: s.removed_reason });
    if (s.author_id !== me) return new PoolError(403, "circle_role_required", { needed: "author", role: s.role });
    if (s.head_version !== w.base) return conflict(c, bid);
  }
  if (bid === null && s.banks >= w.caps.banks) return new PoolError(422, "circles_limit", { metric: "circle_banks", used: s.banks, limit: w.caps.banks, message: `This circle has ${w.caps.banks} banks, the most allowed.` });
  if (s.used + w.bytes > w.caps.bytes) return new PoolError(413, "quota_exceeded", { scope: "circle", used: s.used, needed: w.bytes, limit: w.caps.bytes, message: "This circle's storage is full. Remove a bank or an older version first." });
  if (s.questions + w.n > w.caps.questions) return new PoolError(422, "circle_questions_limit", { used: s.questions, needed: w.n, limit: w.caps.questions, message: `A circle holds at most ${w.caps.questions} questions across its banks.` });
  if (s.qkeys + w.newKeys > w.caps.records) return new PoolError(413, "circle_records_full", { used: s.qkeys, needed: w.newKeys, limit: w.caps.records, message: "This circle has recorded as many different questions as it can keep." });
  if (bid !== null) return conflict(c, bid);
  return new PoolError(409, "conflict", { message: "Something changed meanwhile; try again." });
}
