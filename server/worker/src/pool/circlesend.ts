// Round 32: sending a version of a circle bank (docs/circles-plan.md 4.1, 6.2; phase A).
// Round 37: phase B, shared editing (plan 4.1, 6.2, 13.2; D16, D17).
//
// POST /circles/:cid/banks (owner, editor): the first version of a new bank.
// PUT /circles/:cid/banks/:bid (owner, editor; phase A: only the bank's author): the next version,
// accepted only if the head is still `base_version` (compare-and-set, like a public republish's
// base_publish_id); otherwise `409 conflict` with the true head, and nothing is stored. Round 37: any
// editor or the owner sends to a bank they may read (a held bank: its author only); only the author
// changes the licence (D19: an editor sends with the bank's licence, else `422 license_author_only`); a
// version that drops more than half of the head's questions needs `confirm_large_change` (`428
// confirm_large_required {removed, total}`); a `merge_token` from POST .../merge records `merged_from`.
// POST /circles/:cid/banks/:bid/rollback (owner, or the bank's author while an editor; D17): a NEW version
// equal to a kept one (the body is copied, not re-validated), with the same compare-and-set.
//
// The body is the publish format (`<meta JSON>\n<payload>`, application/vnd.vershion.publish), read by the
// one body reader. A send is publish-sized work: it holds the account's publish lock (pool_locks, so a
// teacher cannot publish and send at once: 409 publish_in_progress), waits for the isolate's heavy slot
// and reserves PUBLISH_COST_PER_BYTE per body byte. share-core validates it once (publish_prepare, no
// strip: every member sees the keys, D12) and gives the key-free match hashes the public re-share check
// needs (circle_qkeys). Round 37: the confirmed-student-data check of a public publish runs on every send
// and rollback (the same block keys, the same near-rewording check, privacynear.ts): a match HOLDS the
// bank at commit (never a refusal, which would confirm the block) and flags the admin; a later version
// without it lifts that hold. Order: stage the version row (status 'staging', guarded by ALIVE_SQL), put
// the body and the per-question index to R2, then ONE commit batch whose first statement is the
// compare-and-set (with every cap of the circle checked in it: D2-D4); every other statement of the
// batch follows it by "this version is now the head". A lost commit deletes the staging row and its R2
// objects; one the isolate never finished is collected by the hourly cron.
import { PARSE_BYTES_PER_VALUE, scanJson } from "./jsonscan.ts";
import { isFail, questionsOf, type Fail, type PayloadJson, type QuestionJson, type Validated } from "./core.ts";
import { EXT } from "./image.ts";
import { allowedByAll, ATTEST_STATEMENT, isLicense, POOL_TERMS_VERSION, type License } from "./license.ts";
import { deleteKeys } from "./moderation.ts";
import {
  acquireAccountLock,
  acquireHeavy,
  BODY_DEADLINE_MS,
  HEAVY_QUEUE_WAIT_MS,
  HELD_PUBLISHES_PER_DAY,
  IN_FLIGHT_MS,
  MAX_META,
  MAX_PAYLOAD,
  PRIVACY_BAND_ROWS_MAX,
  refreshAccountLock,
  releaseAccountLock,
} from "./publish.ts";
import { ACTIVE_SQL, AUTHOR_OK_SQL, BLOCKING_HOLD_SQL, bankOrMiss, circleOrMiss, KEPT_VERSIONS, nameOf, notRetained, OPEN_SQL, OWNER_OR_AUTHOR, requireOpen, requireReadable, requireRole, SEND_FLAG_KINDS, SEND_HOLD_SOURCES, withheldError } from "./circles.ts";
import { rightfulExempts } from "./served.ts";
import { verifyMergeToken } from "./circleversions.ts";
import { decisionsJson, decisionsOf, hashPairs, keptImages, NO_DECISIONS, readStripDecisions, recordedDecisions, STRIPPED_QKEYS_SQL, type StripDecisions } from "./circlehide.ts";
import { invalid as stripRefusal } from "./publish.ts";
import { privacyNearPositions, type StemSketch } from "./privacynear.ts";
import { fieldKeys, nearHash, normSkeletonKey, partKeysOf, stemBands, stemKey, stemTokens } from "./provenance.ts";
import {
  ALIVE_SQL,
  alertBudgeted,
  audit,
  capAlert,
  DAY_MS,
  dayOf,
  FLAG_DETAIL_MAX,
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
  readJson,
  requireDaily,
  reserveBytes,
  sha256Hex,
  throwIfErased,
  visibleTo,
  type Ctx,
  type Metric,
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
/** Round 37: a confirmed student-data block matches one of the block keys (JSON array `keys`) or images (`shas`); each by the primary key. */
export const PRIVACY_SQL = (keys: string, shas: string) =>
  `(EXISTS (SELECT 1 FROM privacy_blocks pk WHERE pk.kind = 'match' AND pk.hash IN (SELECT value FROM json_each(${keys})))
    OR EXISTS (SELECT 1 FROM privacy_blocks pi WHERE pi.kind = 'blob' AND pi.hash IN (SELECT value FROM json_each(${shas}))))`;
/**
 * Round 44 (plan 13.3): SQL condition on blocked_hashes row `bh`: an ACTIVE copyright re-upload block (a takedown's,
 * not lifted, not expired) that holds content sent by `sender` (a parameter reference), by the public path's rules
 * (publish.ts, served.ts): never one the administrator exempted the sender from, and never the rightful item's own
 * material for its author or an account it was served to while visible (rightfulExempts; a circle version has no
 * pool provenance of its own, so only the import record counts as "served"). `now` is a parameter reference.
 */
const COPYRIGHT_ACTIVE = (bh: string, now: string, sender: string) =>
  `${bh}.lifted_at IS NULL AND (${bh}.expires_at IS NULL OR ${bh}.expires_at > ${now}) AND ${bh}.action_id IS NOT NULL
   AND NOT EXISTS (SELECT 1 FROM dmca_exempt ce WHERE ce.action_id = ${bh}.action_id AND ce.user_id = ${sender})
   AND NOT EXISTS (SELECT 1 FROM dmca_actions ca WHERE ca.id = ${bh}.action_id AND ca.rightful_item_id IS NOT NULL AND ${rightfulExempts("ca.rightful_item_id", bh, sender, "NULL")})`;
/** Round 44: a copyright re-upload block holds one of the block keys (JSON array `keys`) or images (`shas`); each by the primary key. */
export const COPYRIGHT_SQL = (keys: string, shas: string, now: string, sender: string) =>
  `(EXISTS (SELECT 1 FROM blocked_hashes ck WHERE ck.kind = 'match' AND ck.hash IN (SELECT value FROM json_each(${keys})) AND ${COPYRIGHT_ACTIVE("ck", now, sender)})
    OR EXISTS (SELECT 1 FROM blocked_hashes ci WHERE ci.kind = 'blob' AND ci.hash IN (SELECT value FROM json_each(${shas})) AND ${COPYRIGHT_ACTIVE("ci", now, sender)}))`;
/**
 * Round 39 (critic 46b N1): the bank's listing fields from its newest version that is not withheld (a version that
 * held confirmed student data never names the bank; with none, the fields are blank). `?1` is the bank.
 */
export const LISTING_REFRESH_SQL = (cond: string) =>
  // (round 40, critic 47b V2: one search of the history for all four fields)
  `UPDATE circle_banks SET (title, subject, course, grade) = (
     SELECT COALESCE(json_extract(x.l, '$.title'), ''), COALESCE(json_extract(x.l, '$.subject'), ''), COALESCE(json_extract(x.l, '$.course'), ''), COALESCE(json_extract(x.l, '$.grade'), '')
       FROM (SELECT (SELECT lv.listing FROM circle_versions lv WHERE lv.bank_id = ?1 AND lv.version IS NOT NULL AND lv.status <> 'staging' AND lv.withheld_at IS NULL AND lv.listing IS NOT NULL
                      ORDER BY lv.version DESC LIMIT 1) AS l) x)
    WHERE id = ?1 AND ${cond}`;
/**
 * Round 38 (critic 45a B2): the bytes of bank `bid`'s versions that a commit making version `upTo + KEPT_VERSIONS`
 * prunes in the same batch (D16: past the 20 kept, never while the bank is held, never one an open report names).
 * The cap is checked net of them: the circle's bytes AFTER the commit, so a bank at the cap keeps taking versions.
 */
const PRUNABLE_SQL = (bid: string, upTo: string) =>
  `(SELECT COALESCE(SUM(pv.bytes), 0) FROM circle_versions pv WHERE pv.bank_id = ${bid} AND pv.status = 'previous' AND pv.version <= ${upTo}
      AND NOT ${BLOCKING_HOLD_SQL(bid)}
      AND NOT EXISTS (SELECT 1 FROM circle_reports xr WHERE xr.bank_id = ${bid} AND xr.resolved_at IS NULL AND xr.version = pv.version)
      AND NOT ${PINNED_EVIDENCE_SQL("pv")})`;
/**
 * Round 46 (critic 53b V7): a withheld version whose student-data or copyright flag is still open is the admin's
 * evidence: no commit prunes it (as no trim and no purge does, rounds 39-41), and the cap does not count it as room a
 * commit frees; once the flag is resolved the next commit prunes it. `v` is the version row (alias or table).
 */
export const PINNED_EVIDENCE_SQL = (v: string) => `(${v}.withheld_at IS NOT NULL AND EXISTS (SELECT 1 FROM admin_flags ef WHERE ef.publish_id = ${v}.id AND ${EVIDENCE_FLAG_SQL("ef")}))`;
/**
 * SQL on admin_flags row `f`: an open student-data or copyright flag whose evidence the administrator still needs.
 * Round 48 (critic 55b B2): a flag the administrator marked `evidence_captured` (POST
 * /admin/pool/circle-evidence-captured: the case file has what the review needs) stays open for the decision, with its
 * positions, hashes and takedowns, but pins nothing: its version is pruned and its bank purged as any other.
 */
export const EVIDENCE_FLAG_SQL = (f: string) => `(${f}.kind IN ${SEND_FLAG_KINDS} AND ${f}.resolved_at IS NULL AND COALESCE(${f}.detail ->> '$.evidence_captured', 0) = 0)`;
/**
 * Round 47 (critic 54b N1): the evidence kept in full is bounded; round 48 (critic 55b B2, V1): by the rules every other
 * prune follows, and per sender. The candidates are a circle's withheld PREVIOUS versions whose flag pins them (a head
 * is never pruned by it: it is the bank's version until the next one), never one of a bank a report or an administrator
 * holds, never one an open report names (those are neither pruned nor counted). Each sender's newest
 * EVIDENCE_SENDER_BANK in a bank and newest EVIDENCE_SENDER_CIRCLE in the circle are protected: no other sender's
 * versions can displace them. The rest fill what is left of EVIDENCE_PER_BANK a bank and EVIDENCE_PER_CIRCLE a circle,
 * newest first, within a quarter of `circle_bytes`; an older one is pruned like any version past the 20 (its body, index
 * and images go, its history line stays) and its flags say `evidence_pruned: true`, keeping what they recorded: the
 * positions, the matching block hashes, the takedowns. Run in every batch that can add evidence (a send's or rollback's
 * commit, an answer-key PATCH).
 */
export const EVIDENCE_PER_BANK = 5;
export const EVIDENCE_PER_CIRCLE = 20;
export const EVIDENCE_SENDER_BANK = 2;
export const EVIDENCE_SENDER_CIRCLE = 5;
/** The block hashes a flag records (round 47). */
export const EVIDENCE_HASHES = 20;
/** The statements that keep the bound for circle `cid` at `now` (`cond`: the batch's own winning marker). */
export function evidenceBound(c: Ctx, cid: string, now: number, circleBytes: number, cond: string): D1PreparedStatement[] {
  const prunedNow = `SELECT pv.id FROM circle_versions pv INDEXED BY circle_versions_prune WHERE pv.pruned_at = ?2 AND pv.status = 'pruned' AND pv.full_key <> '' AND pv.circle_id = ?1 AND pv.withheld_at IS NOT NULL`;
  return [
    c.db
      .prepare(
        `UPDATE circle_versions SET status = 'pruned', pruned_at = ?2 WHERE id IN (
           SELECT x.id FROM (
             SELECT h.id, h.pb, h.pc,
                    SUM(h.pb) OVER (PARTITION BY h.bank_id) AS npb,
                    ROW_NUMBER() OVER (PARTITION BY h.bank_id, h.pb ORDER BY h.version DESC) AS rub,
                    SUM(h.pc) OVER () AS npc,
                    ROW_NUMBER() OVER (PARTITION BY h.pc ORDER BY h.committed_at DESC, h.id DESC) AS ruc,
                    SUM(h.bytes) OVER (PARTITION BY h.pc ORDER BY h.committed_at DESC, h.id DESC ROWS UNBOUNDED PRECEDING) AS upto
               FROM (
                 SELECT ev.id, ev.bank_id, ev.version, ev.committed_at, ev.bytes,
                        ROW_NUMBER() OVER (PARTITION BY ev.bank_id, ev.committer_id ORDER BY ev.version DESC) <= ?6 AS pb,
                        ROW_NUMBER() OVER (PARTITION BY ev.committer_id ORDER BY ev.committed_at DESC, ev.id DESC) <= ?7 AS pc
                   FROM circle_versions ev
                  WHERE ev.circle_id = ?1 AND ev.withheld_at IS NOT NULL AND ev.full_key <> '' AND ev.status = 'previous'
                    AND EXISTS (SELECT 1 FROM admin_flags ef WHERE ef.publish_id = ev.id AND ${EVIDENCE_FLAG_SQL("ef")})
                    AND NOT ${BLOCKING_HOLD_SQL("ev.bank_id")}
                    AND NOT EXISTS (SELECT 1 FROM circle_reports er WHERE er.bank_id = ev.bank_id AND er.resolved_at IS NULL AND er.version = ev.version)) h) x
            WHERE (x.pb = 0 AND x.rub > MAX(0, ?3 - x.npb)) OR (x.pc = 0 AND (x.ruc > MAX(0, ?4 - x.npc) OR x.upto > ?5)))
           AND ${cond}`,
      )
      .bind(cid, now, EVIDENCE_PER_BANK, EVIDENCE_PER_CIRCLE, Math.floor(circleBytes / 4), EVIDENCE_SENDER_BANK, EVIDENCE_SENDER_CIRCLE),
    c.db
      .prepare(
        `UPDATE admin_flags SET detail = json_set(detail, '$.evidence_pruned', json('true'), '$.evidence_pruned_at', ?2)
          WHERE kind IN ${SEND_FLAG_KINDS} AND resolved_at IS NULL AND json_valid(detail) AND publish_id IN (${prunedNow}) AND ${cond}`,
      )
      .bind(cid, now),
    c.db.prepare(`DELETE FROM circle_version_blobs WHERE version_id IN (${prunedNow}) AND ${cond}`).bind(cid, now),
  ];
}
/**
 * Round 48 (critic 55a V3, 55b V3): SQL, the bank-level `cap` flag that names a version withheld past the sender's
 * circle_send_flags (no flag of its own): one flag per bank, sender and day (`dedup`), its detail listing each such
 * version with the takedowns that matched it and whether student data did, so an unblock finds and releases them like
 * flagged ones and the administrator sees which bank. Binds: ?1 id, ?2 bank, ?3 sender, ?4 circle, ?5 version, ?6
 * version id, ?7 actions (JSON), ?8 student data ('true'|'false'), ?9 dedup, ?10 now. Only when the version is withheld
 * and no flag of its own was raised in this batch.
 */
export const CAP_FLAG_SQL = (cond: string) =>
  `INSERT INTO admin_flags (id, kind, item_id, user_id, detail, dedup, at)
   SELECT ?1, 'cap', ?2, ?3, json_object('metric', 'circle_send_flags', 'circle_id', ?4, 'bank_id', ?2, 'versions', json_array(json_object('version', ?5, 'version_id', ?6, 'actions', json(?7), 'student_data', json(?8)))), ?9, ?10
    WHERE ${cond} AND EXISTS (SELECT 1 FROM circle_versions cv WHERE cv.id = ?6 AND cv.withheld_at IS NOT NULL)
      AND NOT EXISTS (SELECT 1 FROM admin_flags vf WHERE vf.publish_id = ?6 AND vf.kind IN ${SEND_FLAG_KINDS} AND vf.at = ?10)
   ON CONFLICT (dedup) DO UPDATE SET detail = json_set(detail, '$.versions[#]', json(excluded.detail -> '$.versions[0]'))`;
export const capDedup = (bid: string, me: string, now: number) => `circle_send_flags:${bid}:${me}:${dayOf(now)}`;
/** Round 47 (critic 54b N1): SQL, true while account `me` has raised fewer than `cap` send flags in circle `cid` since
 *  `day0` (circle_send_flags; the flag's sender is its user_id, its circle in its detail). */
export const FLAG_ROOM_SQL = (me: string, cid: string, day0: string, cap: string) =>
  `((SELECT COUNT(*) FROM admin_flags sf INDEXED BY admin_flags_user_at WHERE sf.user_id = ${me} AND sf.at >= ${day0} AND sf.kind IN ${SEND_FLAG_KINDS} AND sf.detail ->> '$.circle_id' = ${cid}) < ${cap})`;
const QUOTA_MESSAGE =
  "This circle's storage is full: every kept version of every bank counts, with its images. The owner (or a bank's author) can trim a bank's older versions, or remove a bank and purge it.";
/** The versions of bank `?1` pruned at `?2` (a commit's or a trim's own), by the prune index. */
export const PRUNED_NOW_SQL = `SELECT pn.id FROM circle_versions pn INDEXED BY circle_versions_prune WHERE pn.pruned_at = ?2 AND pn.status = 'pruned' AND pn.full_key <> '' AND pn.bank_id = ?1`;
/** Who may roll a bank back (D17): the owner, or the bank's author while an editor. `b` is the bank row (alias or table). */
const ROLLBACK_RIGHTS_SQL = (b: string, cid: string, me: string) => `(${ACTIVE_SQL(cid, me, ["owner"])} OR (${b}.author_id = ${me} AND ${ACTIVE_SQL(cid, me, ["editor"])}))`;

interface SendMeta {
  commitId: string;
  license: License;
  message: string | null;
  base: number | null;
  meta: unknown;
  /** Round 37. */
  mergeToken: string | null;
  mergedFrom: number | null;
  confirm: boolean;
  /** Round 43 (D12): a new bank that hides its keys from viewers from its first version (POST only). */
  hideKey: boolean;
  /** Round 43: the strip decisions for viewers (used when the bank hides its keys). */
  decisions: StripDecisions;
}

const bad = (field: string) => new PoolError(400, "invalid_request", { field });
const intAtLeast1 = (v: unknown) => typeof v === "number" && Number.isInteger(v) && v >= 1 && v <= 2 ** 31;

function readSendMeta(m: Record<string, unknown>, isNew: boolean): SendMeta {
  if (typeof m.commit_id !== "string" || !PUBLISH_ID_RE.test(m.commit_id)) throw bad("commit_id");
  // Chosen explicitly on every send, like a public publish's.
  if (!isLicense(m.license)) throw bad("license");
  // D1: the own-work attestation is ticked on every send (private circles are where copied publisher
  // test banks would hide). Recorded, not verified.
  const at = m.attest as Record<string, unknown> | undefined;
  if (!at || typeof at !== "object" || Array.isArray(at) || at.own_work !== true || at.terms_version !== POOL_TERMS_VERSION)
    throw new PoolError(422, "attestation_required", { terms_version: POOL_TERMS_VERSION, statement: ATTEST_STATEMENT });
  const message = messageOf(m.message);
  let base: number | null = null;
  if (isNew) {
    if (m.base_version !== undefined && m.base_version !== null) throw bad("base_version");
    if (m.merge_token !== undefined || m.merged_from !== undefined) throw bad("merge_token");
  } else {
    if (!intAtLeast1(m.base_version)) throw bad("base_version");
    base = m.base_version as number;
  }
  if (m.merge_token !== undefined && m.merge_token !== null && (typeof m.merge_token !== "string" || m.merge_token.length > 500)) throw bad("merge_token");
  const mergeToken = typeof m.merge_token === "string" ? m.merge_token : null;
  // (merged_from is what the token records; given alone it is refused, given with the token it must agree)
  if (m.merged_from !== undefined && m.merged_from !== null && (!intAtLeast1(m.merged_from) || mergeToken === null)) throw bad("merged_from");
  if (m.confirm_large_change !== undefined && typeof m.confirm_large_change !== "boolean") throw bad("confirm_large_change");
  // Round 43 (D12): only a new bank's first version may say it hides its keys; later, PATCH changes the setting.
  if (m.hide_key !== undefined && (!isNew || typeof m.hide_key !== "boolean")) throw bad("hide_key");
  return {
    commitId: m.commit_id,
    license: m.license,
    message,
    base,
    meta: m.meta ?? {},
    mergeToken,
    mergedFrom: typeof m.merged_from === "number" ? m.merged_from : null,
    confirm: m.confirm_large_change === true,
    hideKey: m.hide_key === true,
    decisions: readStripDecisions(m),
  };
}

function messageOf(v: unknown): string | null {
  if (v === undefined || v === null || v === "") return null;
  const s = cleanText(v, MESSAGE_MAX);
  if (!s) throw bad("message");
  return s;
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
const few = <T,>(xs: T[]) => ({ count: xs.length, first: xs.slice(0, FLAG_DETAIL_MAX) });
const largeError = (removed: number, total: number) =>
  new PoolError(428, "confirm_large_required", {
    removed,
    total,
    message: `This version removes ${removed} of the bank's ${total} questions. Send it again with confirm_large_change if that is what you meant.`,
  });
/** More than half of the head's questions dropped (exactly half is not). */
const isLarge = (removed: number, total: number) => total > 0 && removed * 2 > total;

/** Round 47: the questions share-core's publish check (without hide_key) refuses as having no real key: a keyless
 *  question (`keyless_needs_hide_key`) or a stripped copy's placeholder True/False (`placeholder_key`), by position. */
export function keylessPositions(v: Validated | Fail): number[] {
  if (!isFail(v)) return [];
  const out = new Set<number>();
  for (const e of v.errors) {
    if (e.code !== "keyless_needs_hide_key" && e.code !== "placeholder_key") continue;
    const m = /^bank\.questions\[(\d+)\]/.exec(e.path) ?? (/^question\b/.test(e.path) ? ["", "0"] : null);
    if (m) out.add(Number(m[1]));
  }
  return [...out].sort((a, b) => a - b);
}

/**
 * `422 keyless_question {positions, kinds, message}`. Round 48 (critic 55a V1): the words fit the questions named: a
 * short-answer or essay question (`Short`) has no correct answer to set, only a sample answer to type; every keyless
 * copy can be replaced by the full bank, which an editor reads.
 */
function keylessQuestion(positions: number[], qs: QuestionJson[]): PoolError {
  const named = positions.slice(0, 100);
  const kinds = named.map((i) => {
    const k = (qs[i] as { kind?: unknown } | undefined)?.kind;
    return k && typeof k === "object" ? (Object.keys(k)[0] ?? "") : "";
  });
  const short = kinds.filter((k) => k === "Short").length;
  const fix =
    short === kinds.length
      ? "type a sample answer for each of them"
      : short === 0
        ? "mark the correct answer of each of them"
        : "give each of them its answer: mark the correct one, or type a sample answer";
  return new PoolError(422, "keyless_question", {
    positions: named,
    kinds,
    message: `These questions were copied without their answer keys (a viewer's copy of a bank that hides its keys), so they cannot be sent: the circle keeps the full questions with their keys and explanations. Now that you can edit this bank, get the full bank again and make your changes there, or ${fix}.`,
  });
}

/** Statuses after which the day's counter is given back (nothing new was stored, or the request is retried as is). */
const REFUNDED = new Set([409, 428, 503]);

/** POST /circles/:cid/banks (bid null) and PUT /circles/:cid/banks/:bid. */
export async function sendVersion(c: Ctx, req: Request, p: PoolPrincipal, cid: string, pathBid: string | null): Promise<Response> {
  const me = p.userId;
  const isNew = pathBid === null;
  // Membership and role first (cheap; a non-member is a counted 404 whatever the body).
  const found = isNew ? { m: await circleOrMiss(c, cid, me), b: null } : await bankOrMiss(c, cid, pathBid, me);
  const m = found.m;
  requireOpen(m);
  requireRole(m, "owner", "editor");
  // Round 37: any bank the sender may read (removed: 409 bank_removed; held or under review: its author only, else 404).
  if (found.b) await requireReadable(c, m, found.b, me);
  if (!isPublishType(req)) throw new PoolError(415, "unsupported_media_type");
  return locked(c, me, "circle_commits", async (refresh, held) => {
    const maxPayload = c.deps.maxPublishBytes ?? MAX_PAYLOAD;
    const body = await readBody(req, { max: maxPayload + MAX_META + 1, key: `u:${me}`, deadlineMs: c.deps.bodyDeadlineMs ?? BODY_DEADLINE_MS });
    held.push(body.release);
    const declared = body.bytes.length;
    const read = parsePublish(body.bytes, maxPayload, MAX_META);
    // Round 39 (critic 46b V6): a replay and a send based on an old head are answered before the heavy slot (and
    // before its reservation): a stale send costs a lookup, never a place in the queue public publishes share.
    const pre = await precheck(c, me, cid, isNew ? null : pathBid, read);
    if ("response" in pre) return pre;
    await refresh();
    held.push(await acquireHeavy(c.deps.heavyQueueMs ?? HEAVY_QUEUE_WAIT_MS, refresh, c.deps.heavySlots));
    // Round 33 (critic40b B4): the publish's own formula (publish.ts, round 16 critic21d 1): a payload of
    // many tiny values costs more to parse than its bytes say, so it is reserved by its value count when that
    // is more.
    const scan = scanJson(read.payload);
    held.push(reserveBytes(Math.max(declared * (PUBLISH_COST_PER_BYTE - 1), scan.values * PARSE_BYTES_PER_VALUE * 2)));
    return sendOnce(c, me, cid, isNew ? null : pathBid, pre, read, refresh, scan.ok);
  });
}

/**
 * The account lock and the day's counter around a send or a rollback: the counter is given back when the
 * answer is a replay, a conflict, a large-change confirm or busy (nothing new stored).
 */
async function locked(
  c: Ctx,
  me: string,
  metric: Metric,
  run: (refresh: () => Promise<void>, held: (() => void)[]) => Promise<{ response: Response; replayed?: boolean; status: number }>,
): Promise<Response> {
  await requireDaily(c, me, metric);
  const refund = () => giveDaily(c, me, metric).catch(() => {});
  const lock = await acquireAccountLock(c, me);
  if (!lock) {
    await refund();
    await throwIfErased(c, me);
    throw new PoolError(409, "publish_in_progress", { message: "Another publish or send from this account is still running." }, { "Retry-After": "5" });
  }
  const held: (() => void)[] = [];
  try {
    const res = await run(() => refreshAccountLock(c, me, lock), held);
    if (res.replayed || REFUNDED.has(res.status)) await refund();
    return res.response;
  } catch (e) {
    if (e instanceof PoolError && REFUNDED.has(e.status)) await refund();
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
  license: string;
  updated_at: number;
  hide_key: number;
}
/** The bank as it is NOW (read again after the heavy slot: a send that waited sees the head another one committed). */
async function freshBank(c: Ctx, cid: string, bid: string): Promise<Bank> {
  const b = await c.db
    .prepare(`SELECT id, head_version, head_version_id, author_id, license, updated_at, hide_key FROM circle_banks WHERE id = ?1 AND circle_id = ?2 AND purged_at IS NULL AND head_version > 0`)
    .bind(bid, cid)
    .first<Bank>();
  if (!b) throw new PoolError(404, "not_found");
  return b;
}

/**
 * Idempotency by commit_id: the same id, account, circle, bank and request replays the first answer;
 * another request under it is `409 commit_id_conflict` (never resumed, never revealed); an attempt still in
 * flight is `409 publish_in_progress`; an abandoned one is discarded and the request runs again.
 */
async function replayOf(c: Ctx, me: string, cid: string, bid: string | null, commitId: string, requestSha: string, now: number): Promise<Response | null> {
  const prior = await c.db
    .prepare(`SELECT id, bank_id, circle_id, committer_id, request_sha256, result_json, status, full_key, index_key, created_at FROM circle_versions WHERE commit_id = ?1`)
    .bind(commitId)
    .first<{ id: string; bank_id: string; circle_id: string; committer_id: string; request_sha256: string; result_json: string | null; status: string; full_key: string; index_key: string; created_at: number }>();
  if (!prior) return null;
  if (prior.committer_id !== me || prior.request_sha256 !== requestSha || prior.circle_id !== cid || (bid !== null && prior.bank_id !== bid)) throw new PoolError(409, "commit_id_conflict");
  if (prior.status !== "staging" && prior.result_json) return json(200, { ...(JSON.parse(prior.result_json) as object), replayed: true });
  if (prior.created_at > now - IN_FLIGHT_MS) throw new PoolError(409, "publish_in_progress", {}, { "Retry-After": "5" });
  // An abandoned attempt: start again with the same commit_id. Round 39 (critic 46b N2): its R2 keys are recorded
  // as orphans in the same batch, so a failed delete is retried hourly instead of leaving them forever.
  const keys = [prior.full_key, prior.index_key].filter((k) => k !== "");
  const [del] = await c.db.batch([
    c.db.prepare(`DELETE FROM circle_versions WHERE id = ?1 AND status = 'staging' AND created_at = ?2`).bind(prior.id, prior.created_at),
    c.db.prepare(`INSERT OR IGNORE INTO r2_orphans (key, at) SELECT value, ?2 FROM json_each(?1) WHERE changes() = 1`).bind(JSON.stringify(keys), now),
  ]);
  if ((del!.meta.changes ?? 0) !== 1) throw new PoolError(409, "publish_in_progress", {}, { "Retry-After": "5" });
  await deleteKeys(c, keys);
  return null;
}

/** A version's per-question index in R2: qids, content hashes and (round 37) the key-free match hashes. */
interface Index {
  qids: string[];
  h: string[];
  m?: string[];
}
async function readIndex(c: Ctx, key: string): Promise<Index | null> {
  const o = key ? await c.r2.get(key) : null;
  if (!o) return null;
  try {
    const x = JSON.parse(await o.text()) as { qids?: unknown; h?: unknown; m?: unknown }; // r2-object: our own index
    if (!Array.isArray(x.qids) || !Array.isArray(x.h) || x.qids.length !== x.h.length) return null;
    return { qids: x.qids as string[], h: x.h as string[], ...(Array.isArray(x.m) && x.m.length === x.qids.length ? { m: x.m as string[] } : {}) };
  } catch {
    return null;
  }
}

/** What changed from `prev` to `next`, per qid. */
function diff(prev: Index | null, next: { qids: string[]; h: string[] }): { added: number; changed: number; removed: number } {
  const prevH = new Map((prev?.qids ?? []).map((q, k) => [q, prev!.h[k]!]));
  const nextSet = new Set(next.qids);
  return {
    added: next.qids.filter((q) => !prevH.has(q)).length,
    changed: next.qids.filter((q, k) => prevH.has(q) && prevH.get(q) !== next.h[k]).length,
    removed: [...prevH.keys()].filter((q) => !nextSet.has(q)).length,
  };
}

/** A recorded origin (public material a send declared, checked then). Round 37: `cq` is the circle qid it belongs
 *  to (so an editor's later version carries it), and no author name is stored (it is read at each read). */
interface Origin {
  pos: number;
  item_id: string;
  qid: string | null;
  version: number | null;
  license: string;
  cq?: string;
}

/**
 * Round 37: the student-data check of a public publish (publish.ts round 8 I6a, 12 M4/M6, 13, 16 B5), on the
 * questions of a circle version: each question's block keys (the key-free match hash, the normalised
 * skeleton, the stem, the full content, the explanation and the answer, each Multipart part's stem,
 * explanation and answer) and the near-rewording check of its stems. A circle bank is never stripped (D12),
 * so its served form is its full form. `exact`: positions whose key or image a block holds.
 */
/** One form of a version's questions a member may be served (round 46: the full form, and for a bank that hides its
 *  keys the stripped form its viewers read), with its key-free match hashes and each question's image. */
export interface ScanForm {
  qs: QuestionJson[];
  mh: string[];
  imgAt: (string | null)[];
}
export type Scan = {
  keys: string[];
  shas: string[];
  exact: number[];
  near: number[];
  copyright: number[];
  actions: string[];
  /** Round 47 (critic 54b N1): the block hashes that matched (student data, copyright), the evidence a flag keeps when
   *  its version's body is pruned. */
  hits: string[];
  chits: string[];
};
/** Each question's image hash (`media/<sha>.<ext>`), or null. */
export const imagesAt = (qs: QuestionJson[]): (string | null)[] =>
  qs.map((q) => (q.image && typeof q.image === "object" && typeof q.image.file === "string" ? MEDIA_RE.exec(q.image.file)?.[1] ?? null : null));
/**
 * Round 37/44: the student-data and copyright checks of a version (positions are question indices). Round 46 (critic
 * 53b B2): over EVERY form a member may be served (the full form, and the stripped form a bank that hides its keys
 * serves its viewers), each position's keys and stems the union of its forms', as the public path checks the form it
 * serves.
 */
export async function privacyScan(c: Ctx, forms: ScanForm[], sender: string): Promise<Scan> {
  const keysAt: (string | null)[][] = [];
  const stems: StemSketch[][] = [];
  const imgSets: (string | null)[][] = [];
  const n = forms[0]?.qs.length ?? 0;
  for (let i = 0; i < n; i++) {
    const ks: (string | null)[] = [];
    const ss: StemSketch[] = [];
    const im: (string | null)[] = [];
    for (const fm of forms) {
      const q = fm.qs[i];
      if (!q) continue;
      const f = await fieldKeys(q);
      ks.push(fm.mh[i] ?? null, await normSkeletonKey(q), await stemKey(q), `f:${await nearHash(q)}`, f.expl, f.ans, ...(await partKeysOf(q)).map((x) => x.key));
      const t = stemTokens((q as { stem?: unknown }).stem);
      const parts = (((q as unknown as { kind?: { Multipart?: { parts?: { stem?: unknown }[] } } }).kind?.Multipart?.parts ?? []) as { stem?: unknown }[]).map((p) => {
        const pt = stemTokens(p?.stem);
        return { tokens: pt, bands: stemBands(pt) };
      });
      ss.push({ tokens: t, bands: stemBands(t) }, ...parts);
      im.push(fm.imgAt[i] ?? null);
    }
    keysAt.push([...new Set(ks)]);
    stems.push(ss);
    imgSets.push([...new Set(im)]);
  }
  const imgAt = imgSets;
  const keys = [...new Set(keysAt.flat().filter((k): k is string => k !== null))];
  const shas = [...new Set(imgAt.flat().filter((s): s is string => s !== null))];
  const hit = new Set(
    (await c.db.prepare(`SELECT hash FROM privacy_blocks WHERE kind = 'match' AND hash IN (SELECT value FROM json_each(?1))`).bind(JSON.stringify(keys)).all<{ hash: string }>()).results.map((x) => x.hash),
  );
  const blobHit = new Set(
    shas.length > 0
      ? (await c.db.prepare(`SELECT hash FROM privacy_blocks WHERE kind = 'blob' AND hash IN (SELECT value FROM json_each(?1))`).bind(JSON.stringify(shas)).all<{ hash: string }>()).results.map((x) => x.hash)
      : [],
  );
  const exact = keysAt.map((ks, i) => (ks.some((k) => k !== null && hit.has(k)) || imgAt[i]!.some((x) => x !== null && blobHit.has(x)) ? i : -1)).filter((i) => i >= 0);
  const near = await privacyNearPositions(c, stems, keysAt, async () => hit, new Map(), c.deps.privacyBandRowsMax ?? PRIVACY_BAND_ROWS_MAX);
  // Round 44: the same keys and images against the public pool's copyright re-upload blocks (the positions are the
  // admin's flag detail and the held-publish charge; the commit decides again, against the blocks as they are then).
  const now = c.now();
  const cRows = (await c.db.prepare(`SELECT ck.hash, ck.action_id FROM blocked_hashes ck WHERE ck.kind = 'match' AND ck.hash IN (SELECT value FROM json_each(?1)) AND ${COPYRIGHT_ACTIVE("ck", "?2", "?3")}`).bind(JSON.stringify(keys), now, sender).all<{ hash: string; action_id: string }>()).results;
  const bRows =
    shas.length > 0
      ? (await c.db.prepare(`SELECT ci.hash, ci.action_id FROM blocked_hashes ci WHERE ci.kind = 'blob' AND ci.hash IN (SELECT value FROM json_each(?1)) AND ${COPYRIGHT_ACTIVE("ci", "?2", "?3")}`).bind(JSON.stringify(shas), now, sender).all<{ hash: string; action_id: string }>()).results
      : [];
  const cHit = new Set(cRows.map((x) => x.hash));
  const cBlob = new Set(bRows.map((x) => x.hash));
  const copyright = keysAt.map((ks, i) => (ks.some((k) => k !== null && cHit.has(k)) || imgAt[i]!.some((x) => x !== null && cBlob.has(x)) ? i : -1)).filter((i) => i >= 0);
  // (round 46: the takedowns whose blocks matched, so an unblock can tell the admin which circle versions it freed)
  const actions = [...new Set([...cRows, ...bRows].map((x) => x.action_id))].sort().slice(0, 50);
  return { keys, shas, exact, near, copyright, actions, hits: [...hit, ...blobHit].sort().slice(0, EVIDENCE_HASHES), chits: [...cHit, ...cBlob].sort().slice(0, EVIDENCE_HASHES) };
}

type Result = { response: Response; replayed?: boolean; status: number };

interface Pre {
  sm: SendMeta;
  requestSha: string;
  mergedFrom: number | null;
}
/** The cheap part of a send: its meta, idempotency (commit_id), the head it is based on, the licence rule and the merge token. */
async function precheck(c: Ctx, me: string, cid: string, pathBid: string | null, read: { raw: Uint8Array; meta: Record<string, unknown> }): Promise<Pre | Result> {
  const sm = readSendMeta(read.meta, pathBid === null);
  const requestSha = await sha256Hex(read.raw);
  // ---- idempotency (commit_id) ------------------------------------------------------------------------
  const replay = await replayOf(c, me, cid, pathBid, sm.commitId, requestSha, c.now());
  if (replay) return { response: replay, replayed: true, status: 200 };
  const bank = pathBid === null ? null : await freshBank(c, cid, pathBid);
  if (checkHead(bank, sm, me)) throw await conflict(c, bank!.id);
  const mergedFrom = sm.mergeToken !== null ? await verifyMergeToken(c, sm.mergeToken, me, bank!.id, sm.base!) : null;
  if (sm.mergedFrom !== null && sm.mergedFrom !== mergedFrom) throw bad("merged_from");
  return { sm, requestSha, mergedFrom };
}
/** True when the head moved past the send's base (a 409); throws the licence rule (D19, round 37: the licence is the author's to choose; an editor sends with the bank's). */
function checkHead(bank: Bank | null, sm: SendMeta, me: string): boolean {
  if (!bank) return false;
  if (bank.head_version !== sm.base) return true;
  if (bank.author_id !== me && sm.license !== bank.license) throw licenseAuthorOnly(bank.license);
  return false;
}

async function sendOnce(
  c: Ctx,
  me: string,
  cid: string,
  pathBid: string | null,
  pre: Pre,
  read: { raw: Uint8Array; meta: Record<string, unknown>; payload: Uint8Array },
  refresh: () => Promise<void>,
  scanOk: boolean,
): Promise<Result> {
  const isNew = pathBid === null;
  const { sm, requestSha, mergedFrom } = pre;
  const now = c.now();
  // The head the sender based on, read again after the heavy slot (checked once more in the commit's compare-and-set).
  const bank = isNew ? null : await freshBank(c, cid, pathBid);
  if (checkHead(bank, sm, me)) throw await conflict(c, bank!.id);

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
  // Round 46 (critic 53a B1): a keyless question (a copy made without the answer keys, such as a viewer's copy of a
  // bank that hides its keys) is never a circle version, whatever the bank's setting: the stored full text is the only
  // copy of the keys every editor and the owner work from, so a send must not replace them with blanks. Round 47
  // (critic 54a B1): decided by share-core's own rule, not a copy of it: its publish check without hide_key names every
  // question it treats as keyless (`keyless_needs_hide_key`: the flag, or the sentinel explanation the app stores in
  // place of it) and every stripped copy's placeholder True/False (`placeholder_key`), on the payload as sent (its
  // `origin` still on it). A payload that check refuses for something else (a field the send drops, such as a malformed
  // `origin`, never reaches the keyless rule) is checked again in the form the send stores, below.
  const kv = c.core.validateForPublish(payload, { hideKey: false, labels: [] });
  const keyless = keylessPositions(kv);
  if (keyless.length > 0) throw keylessQuestion(keyless, qs);
  const keylessAgain = isFail(kv);
  const bid = bank ? bank.id : newId();

  // qids: a sent qid (question.published.qid) is kept only if the head held it and it appears once.
  let prevIdx: Index | null = null;
  let prevOrigins: Origin[] = [];
  if (bank?.head_version_id) {
    const v = await c.db.prepare(`SELECT index_key, origins FROM circle_versions WHERE id = ?1`).bind(bank.head_version_id).first<{ index_key: string; origins: string | null }>();
    prevIdx = v ? await readIndex(c, v.index_key) : null;
    try {
      prevOrigins = v?.origins ? (JSON.parse(v.origins) as Origin[]) : [];
    } catch {
      prevOrigins = [];
    }
  }
  const prevQids = new Set(prevIdx?.qids ?? []);
  const sent = qs.map((q) => (isObj(q.published) && typeof q.published.qid === "string" ? q.published.qid : null));
  // Round 38 (critic 45a B3): a qid sent more than once is a copy (the app's Duplicate keeps `published`): one
  // occurrence keeps the qid, each other one is a new question. Round 40 (critic 47a B1): the one that keeps it is the
  // occurrence whose content is unchanged from the head (else the first), so where a copy sits changes nothing.
  const usable = (x: string | null): x is string => x !== null && ID_RE.test(x) && prevQids.has(x);
  const repeated = sent.some((x, k) => usable(x) && sent.indexOf(x) !== k);
  const rawH = repeated && prevIdx ? await c.core.contentHashes(qs) : null;
  const headH = new Map((prevIdx?.qids ?? []).map((x, k) => [x, prevIdx!.h[k]!]));
  const holder = new Map<string, number>();
  sent.forEach((x, k) => {
    if (!usable(x)) return;
    const cur = holder.get(x);
    if (cur === undefined || (rawH && rawH[k] === headH.get(x) && rawH[cur] !== headH.get(x))) holder.set(x, k);
  });
  const qids = sent.map((x, k) => (usable(x) && holder.get(x) === k ? x : newId()));
  /** The qid a later occurrence copies (round 40: the bank's author's copies carry its credit, below). */
  const copyOf = sent.map((x, k) => (usable(x) && holder.get(x) !== k ? x : null));
  // Round 37: a version that drops more than half of the head's questions is confirmed (a mistake guard, plan 4.1).
  if (prevIdx && !sm.confirm) {
    const kept = new Set(qids);
    const dropped = prevIdx.qids.filter((q) => !kept.has(q)).length;
    if (isLarge(dropped, prevIdx.qids.length)) throw largeError(dropped, prevIdx.qids.length);
  }

  // Images: verified, and the sender's (pending, in one of their publishes) or already in this bank's kept versions.
  const imgOf = (q: QuestionJson) => (q.image && typeof q.image === "object" && typeof q.image.file === "string" ? MEDIA_RE.exec(q.image.file)?.[1] ?? null : null);
  const imgAt = qs.map(imgOf);
  const shas = [...new Set(imgAt.filter((x): x is string => x !== null))];
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
    const missing = imgAt.map((s, i) => [i, s] as const).filter(([, s]) => s !== null && !blobInfo.has(s!)).map(([i]) => i);
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
    // public publish, so a send cannot confirm a block; the circle's blob route never serves it; round 37:
    // the bank is held at commit instead).
    const blockedImg = new Set(
      (await c.db.prepare(`SELECT hash FROM privacy_blocks WHERE kind = 'blob' AND hash IN (SELECT value FROM json_each(?1))`).bind(JSON.stringify(shas)).all<{ hash: string }>()).results.map((x) => x.hash),
    );
    for (const s of shas)
      if (!blockedImg.has(s) && blobInfo.get(s)!.privacy_purged_at === null && !(await c.r2.head(`blobs/${s}`))) {
        await c.db.prepare(`UPDATE blobs SET missing_at = COALESCE(missing_at, ?2) WHERE sha256 = ?1`).bind(s, now).run();
        throw new PoolError(422, "blob_missing", { reason: "reupload", positions: imgAt.map((x, i) => (x === s ? i : -1)).filter((i) => i >= 0).slice(0, 100) });
      }
  }

  // Public material sent into a circle bank (plan section 7): the bank's licence must be one each DECLARED
  // origin allows (the server checks declared origins only; an origin the sender cannot see is ignored;
  // her own items allow anything). Recorded with the version (members see the credits). Round 37: an
  // origin recorded for a question the head held stays with it in every later version (an editor's copy
  // has no origin fields: the body members read carries none), unless the sender declares it again. Round 38:
  // the same text sent again (a copy, or without its qid) carries it too (matched by the key-free match hash,
  // below, once share-core has given the hashes); round 39 (critic 46b V1): only the same text, so a changed copy
  // gets no credit, and an origin a NON-author declares on a question the head already holds is ignored (an editor
  // cannot put a public item's credit and licence bound on the author's own question; on a new one it is recorded).
  const kept = (k: number) => sent[k] !== null && qids[k] === sent[k];
  const declaredAt = new Set<number>();
  const declared = qs.map((q, i) => (isObj(q.origin) && typeof q.origin.item_id === "string" && ID_RE.test(q.origin.item_id) ? { pos: i, item_id: q.origin.item_id as string, qid: typeof q.origin.qid === "string" ? q.origin.qid : null, version: typeof q.origin.version === "number" ? q.origin.version : null } : null)).filter((x) => x !== null);
  const origins: Origin[] = [];
  if (declared.length > 0) {
    const its = await c.db
      .prepare(`SELECT i.id, i.license, i.author_id FROM items i JOIN users u ON u.id = i.author_id WHERE i.id IN (SELECT value FROM json_each(?1)) AND ${visibleTo("?2")}`)
      .bind(JSON.stringify([...new Set(declared.map((d) => d.item_id))]), me)
      .all<{ id: string; license: string; author_id: string }>();
    const by = new Map(its.results.map((r) => [r.id, r]));
    for (const d of declared) {
      const it = by.get(d.item_id);
      if (!it || it.author_id === me) continue;
      if (bank && bank.author_id !== me && kept(d.pos)) continue;
      origins.push({ ...d, license: it.license, cq: qids[d.pos]! });
      declaredAt.add(d.pos);
    }
  }
  const carried = new Map<string, Origin>();
  for (const o of prevOrigins) {
    const cq = o.cq ?? prevIdx?.qids[o.pos];
    if (cq && typeof o.item_id === "string" && typeof o.license === "string") carried.set(cq, o);
  }
  const carry = (o: Origin, k: number) => origins.push({ pos: k, item_id: o.item_id, qid: o.qid ?? null, version: o.version ?? null, license: o.license, cq: qids[k]! });
  // (round 40, critic 47a B1: the bank's AUTHOR's copies of a credited question, exact or adapted, keep its credit, so
  // "duplicate, edit the copy, drop the original" keeps the attribution and the licence bound; a non-author's copy
  // keeps it only as an exact copy, by the match hash below, so nobody else can put the credit on other text)
  const authorSends = bank === null || bank.author_id === me;
  qids.forEach((q, k) => {
    const o = carried.get(q) ?? (authorSends && copyOf[k] ? carried.get(copyOf[k]!) : undefined);
    if (o && !declaredAt.has(k)) carry(o, k);
  });
  // (the head's origins by their key-free match hash: from the head index's `m`, round 37 on)
  const byHash = new Map<string, Origin>();
  if (prevIdx?.m) for (const [cq, o] of carried) {
    const k = prevIdx.qids.indexOf(cq);
    if (k >= 0) byHash.set(prevIdx.m[k]!, o);
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
    const s = imgAt[i];
    if (s && q.image) {
      const b = blobInfo.get(s)!;
      q.image = { file: `media/${s}.${EXT[b.content_type]}`, width: b.width, height: b.height };
    }
  });
  let rBytes: Uint8Array | null = enc.encode(JSON.stringify(raw));
  if (keylessAgain) {
    const k2 = keylessPositions(c.core.validateForPublish(rBytes, { hideKey: false, labels: [] }));
    if (k2.length > 0) throw keylessQuestion(k2, qs);
  }
  raw = null;
  (read as { payload: Uint8Array | null }).payload = null;

  // ---- share-core: one validation (no strip), the key-free match hashes ----------------------------------
  const vm = c.core.validateMeta(sm.meta);
  if (isFail(vm)) throw invalid(vm);
  // Round 43 (D12): a bank that hides its keys from viewers is checked as a hidden-key publish is (share-core's strip
  // and key-hint gate with the sender's decisions): a refusal has the publish's shape and stores nothing; the decisions
  // are recorded with the version (what viewers are served of it), the stripped form itself is let go.
  const hide = isNew ? sm.hideKey : bank!.hide_key === 1;
  const d = hide ? sm.decisions : NO_DECISIONS;
  const prep = c.core.publishPrepare(rBytes!, { hideKey: hide, labels: d.labels, orderAcks: d.orderAcks, imageOk: d.imageOk, ackFormatting: d.ackFormatting });
  rBytes = null;
  if (isFail(prep)) throw hide ? stripRefusal(prep) : invalid(prep);
  let stripOpts: string | null = null;
  let pairs: [string, string][] = [];
  let viewerForm: ScanForm | null = null;
  if (hide && prep.stripped) {
    stripOpts = decisionsJson(recordedDecisions(d, prep.stripped, n), keptImages(prep.stripped.payload));
    pairs = hashPairs(prep.matchHashes, prep.stripped.matchHashes);
    const vq = questionsOf(prep.stripped.payload);
    viewerForm = { qs: vq, mh: prep.stripped.matchHashes, imgAt: imagesAt(vq) };
    prep.stripped = null;
  }
  const full = prep.full;
  const fullQs = questionsOf(full.payload);
  if (!full.payload.bank || fullQs.length !== n) throw new PoolError(422, "invalid_payload", { errors: [{ code: "invalid", path: "" }] });
  const contents = await c.core.contentHashes(fullQs);
  const hasImages = fullQs.some((q) => q.image) ? 1 : 0;
  // Round 37: the student-data check (on the validated form, before it is let go). Round 46 (critic 53b B2): and on the
  // stripped form a bank that hides its keys serves its viewers.
  const privacy = await privacyScan(c, [{ qs: fullQs, mh: prep.matchHashes, imgAt }, ...(viewerForm ? [viewerForm] : [])], me);
  viewerForm = null;
  // Round 38 (critic 45b variant 1): the same text sent without its qid keeps the origin the head recorded for it.
  if (byHash.size > 0) {
    const has = new Set(origins.map((o) => o.pos));
    prep.matchHashes.forEach((h, k) => {
      const o = byHash.get(h);
      if (o && !has.has(k)) carry(o, k);
    });
  }
  origins.sort((a, b) => a.pos - b.pos);
  if (origins.length > 0) {
    const allowed = allowedByAll(origins.map((o) => o.license));
    if (!allowed.includes(sm.license)) {
      const badPos = origins.filter((o) => !allowedByAll([o.license]).includes(sm.license)).map((o) => o.pos);
      throw new PoolError(422, "license_incompatible", { positions: badPos.slice(0, 100), allowed, message: "Some questions came from the public pool under a licence that does not allow the one chosen for this bank." });
    }
  }
  (full as { payload: unknown }).payload = null;
  fullQs.length = 0;
  await refresh();

  // ---- what changed (per qid), the bytes, the records ------------------------------------------------------
  const { added, changed, removed } = diff(prevIdx, { qids, h: contents });
  const index = enc.encode(JSON.stringify({ qids, h: contents, m: prep.matchHashes }));
  const imageBytes = shas.reduce((a, s) => a + (blobInfo.get(s)?.size ?? 0), 0);
  const bytes = full.bytes.length + index.length + imageBytes;
  const hashes = [...new Set(prep.matchHashes)];
  // (round 43: and the served form's, where a label stripped for viewers changes it)
  const counted = [...new Set([...hashes, ...pairs.map((x) => x[1])])];
  const newKeys =
    (
      await c.db
        .prepare(`SELECT COUNT(*) AS n FROM json_each(?1) j WHERE NOT EXISTS (SELECT 1 FROM circle_qkeys k WHERE k.circle_id = ?2 AND k.match_hash = j.value AND k.bank_id = ?3)`)
        .bind(JSON.stringify(counted), cid, bid)
        .first<{ n: number }>()
    )?.n ?? counted.length;
  const meta = { title: (vm.title || cleanText(bankName, 200) || "").slice(0, 200), subject: vm.subject, course: vm.course, grade: vm.grade };
  const vid = newId();
  const version = isNew ? 1 : sm.base! + 1;
  return commitVersion(c, {
    kind: "send",
    me,
    cid,
    bid,
    vid,
    isNew,
    base: sm.base,
    version,
    commitId: sm.commitId,
    requestSha,
    message: sm.message,
    license: sm.license,
    meta,
    listing: JSON.stringify(meta),
    n,
    hasImages,
    bytes,
    newKeys,
    hashes,
    shas,
    origins,
    mergedFrom,
    rollbackOf: null,
    privacy,
    stripOpts,
    pairs,
    hideKey: hide ? 1 : 0,
    result: { circle_id: cid, bank_id: bid, version, version_id: vid, qids, added, changed, removed, ...(mergedFrom !== null ? { merged_from: mergedFrom } : {}), ...(hide ? { keys_hidden: true } : {}) },
    auditDetail: { added, changed, removed, license: sm.license, ...(mergedFrom !== null ? { merged_from: mergedFrom } : {}) },
    put: async (fullKey, indexKey) => {
      await c.r2.put(fullKey, full.bytes, { httpMetadata: { contentType: "application/json" } });
      await c.r2.put(indexKey, index, { httpMetadata: { contentType: "application/json" } });
    },
    refresh,
  });
}

interface CommitSpec {
  kind: "send" | "rollback";
  me: string;
  cid: string;
  bid: string;
  vid: string;
  isNew: boolean;
  base: number | null;
  version: number;
  commitId: string;
  requestSha: string;
  message: string | null;
  license: string;
  /** The bank's listing fields (a rollback keeps the bank's: null). */
  meta: { title: string; subject: string; course: string; grade: string } | null;
  /** Round 39: this version's own listing fields (JSON), recorded with it; a rollback's are its target's. */
  listing: string | null;
  n: number;
  hasImages: number;
  bytes: number;
  newKeys: number;
  hashes: string[];
  shas: string[];
  origins: Origin[];
  mergedFrom: number | null;
  rollbackOf: number | null;
  privacy: Scan;
  /** Round 43: the strip decisions recorded with the version (a rollback copies its target's). */
  stripOpts: string | null;
  /** Round 43: [full, served] key-free hashes of positions a label stripped for viewers changes (D13 records). */
  pairs: [string, string][];
  /** Round 43: the bank's hide_key the send was checked for (null: a rollback, which strips nothing). */
  hideKey: number | null;
  result: Record<string, unknown>;
  auditDetail: Record<string, unknown>;
  put: (fullKey: string, indexKey: string) => Promise<void>;
  refresh: () => Promise<void>;
}

/**
 * Stage, put, commit (one batch whose first statement is the compare-and-set), or clean up and say why it
 * lost. Shared by a send and a rollback. The answer is the stored result (`held`: the bank is hidden from its
 * members now; set in the same batch).
 */
async function commitVersion(c: Ctx, x: CommitSpec): Promise<Result> {
  const { me, cid, bid, vid, version } = x;
  const now = c.now();
  const caps = {
    bytes: await limitOf(c, "*", "circle_bytes"),
    questions: await limitOf(c, "*", "circle_questions"),
    records: await limitOf(c, "*", "circle_records"),
    banks: await limitOf(c, "*", "circle_banks"),
  };
  const day0 = dayOf(now) * DAY_MS;
  const flagCap = await limitOf(c, me, "circle_send_flags");
  const fullKey = `circles/${cid}/${bid}/${vid}.json`;
  const indexKey = `circles/${cid}/${bid}/${vid}.idx.json`;
  const staged = await c.db
    .prepare(
      `INSERT INTO circle_versions (id, bank_id, circle_id, version, commit_id, request_sha256, committer_id, base_version, merged_from, rollback_of, message, license, status, full_key, index_key, bytes, question_count, added, changed, removed, origins, created_at, listing, strip_opts)
       SELECT ?1, ?2, ?3, NULL, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, 'staging', ?12, ?13, ?14, ?15, ?16, ?17, ?18, ?19, ?20, ?21, ?22 WHERE ${ALIVE_SQL("?6")}`,
    )
    .bind(
      vid,
      bid,
      cid,
      x.commitId,
      x.requestSha,
      me,
      x.base,
      x.mergedFrom,
      x.rollbackOf,
      x.message,
      x.license,
      fullKey,
      indexKey,
      x.bytes,
      x.n,
      x.result.added as number,
      x.result.changed as number,
      x.result.removed as number,
      x.origins.length ? JSON.stringify(x.origins) : null,
      now,
      x.listing,
      x.stripOpts,
    )
    .run();
  if ((staged.meta.changes ?? 0) !== 1) {
    await throwIfErased(c, me);
    throw new PoolError(409, "publish_in_progress", {}, { "Retry-After": "5" });
  }
  await x.put(fullKey, indexKey);
  await x.refresh();
  const won = `EXISTS (SELECT 1 FROM circle_banks wb WHERE wb.id = '${bid}' AND wb.head_version_id = '${vid}')`;
  const rights = x.kind === "send" ? ACTIVE_SQL("?2", "?3", ["owner", "editor"]) : ROLLBACK_RIGHTS_SQL("circle_banks", "?2", "?3");
  const cas = x.isNew
    ? c.db
        .prepare(
          `INSERT INTO circle_banks (id, circle_id, author_id, title, subject, course, grade, license, head_version, head_version_id, question_count, has_images, created_at, updated_at, hide_key)
           SELECT ?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, 1, ?9, ?10, ?11, ?12, ?12, ?19
            WHERE ${OPEN_SQL("?2")} AND ${ACTIVE_SQL("?2", "?3", ["owner", "editor"])} AND ${ALIVE_SQL("?3")}
              AND ${BANKS_SQL("?2")} < ?13 AND ${BYTES_SQL("?2")} + ?14 <= ?15 AND ${QUESTIONS_SQL("?2", "?1")} + ?10 <= ?16
              AND (SELECT qkeys FROM circles WHERE id = ?2) + ?17 <= ?18
              AND EXISTS (SELECT 1 FROM circle_versions sv WHERE sv.id = ?9 AND sv.status = 'staging')`,
        )
        .bind(bid, cid, me, x.meta!.title, x.meta!.subject, x.meta!.course, x.meta!.grade, x.license, vid, x.n, x.hasImages, now, caps.banks, x.bytes, caps.bytes, caps.questions, x.newKeys, caps.records, x.hideKey ?? 0)
    : c.db
        .prepare(
          // Round 37: any editor or the owner (a rollback: the owner, or the author while an editor); the licence
          // is the author's to change (D19); a held bank, or one whose author is under review, takes versions from
          // its author only.
          `UPDATE circle_banks SET head_version = head_version + 1, head_version_id = ?4, title = COALESCE(?5, title), subject = COALESCE(?6, subject), course = COALESCE(?7, course), grade = COALESCE(?8, grade),
                  license = ?9, question_count = ?10, has_images = ?11, updated_at = ?12
            WHERE id = ?1 AND circle_id = ?2 AND head_version = ?13 AND removed_at IS NULL AND purged_at IS NULL
              AND (author_id = ?3 OR license = ?9)
              AND (author_id = ?3 OR ((NOT EXISTS (SELECT 1 FROM circle_holds hh WHERE hh.bank_id = ?1) OR (NOT ${BLOCKING_HOLD_SQL("?1")} AND ${ACTIVE_SQL("?2", "?3", ["owner"])})) AND ${AUTHOR_OK_SQL("circle_banks")}))
              AND ${rights} AND ${OPEN_SQL("?2")} AND ${ALIVE_SQL("?3")}
              AND ${BYTES_SQL("?2")} - ${PRUNABLE_SQL("?1", "?19")} + ?14 <= ?15 AND ${QUESTIONS_SQL("?2", "?1")} + ?10 <= ?16
              AND (SELECT qkeys FROM circles WHERE id = ?2) + ?17 <= ?18
              AND (?20 IS NULL OR hide_key = ?20)
              AND EXISTS (SELECT 1 FROM circle_versions sv WHERE sv.id = ?4 AND sv.status = 'staging')`,
        )
        .bind(bid, cid, me, vid, x.meta?.title ?? null, x.meta?.subject ?? null, x.meta?.course ?? null, x.meta?.grade ?? null, x.license, x.n, x.hasImages, now, x.base, x.bytes, caps.bytes, caps.questions, x.newKeys, caps.records, version - KEPT_VERSIONS, x.hideKey);
  const keysJson = JSON.stringify(x.privacy.keys);
  const pShas = JSON.stringify(x.privacy.shas);
  const nearN = x.privacy.near.length;
  const res = await c.db.batch([
    // (the sequence number this commit's "first sent" records carry; a lost commit leaves a harmless gap)
    c.db.prepare(`UPDATE pool_seq SET n = n + 1 WHERE id = 1`),
    cas,
    c.db.prepare(`UPDATE circle_versions SET status = 'previous' WHERE bank_id = ?1 AND status = 'current' AND id <> ?2 AND ${won}`).bind(bid, vid),
    // D16: the last KEPT_VERSIONS versions are kept in full; older ones keep their history line (their
    // images' rows go now; the daily cron deletes their R2 objects). Round 33 (critic40b V2): never while
    // the bank is held, and never a version an open report names (the evidence stays for the admin; the
    // next send after the case is resolved prunes it). Round 38: before this commit's own holds are set, so it prunes
    // exactly what the compare-and-set's cap counted out (PRUNABLE_SQL). A withheld version is pruned like any other once
    // its admin flag is resolved (round 46: never while it is open, PINNED_EVIDENCE_SQL).
    c.db
      .prepare(
        `UPDATE circle_versions SET status = 'pruned', pruned_at = ?3 WHERE bank_id = ?1 AND status = 'previous' AND version <= ?2 AND ${won}
           AND NOT ${BLOCKING_HOLD_SQL("?1")}
           AND NOT EXISTS (SELECT 1 FROM circle_reports pr WHERE pr.bank_id = ?1 AND pr.resolved_at IS NULL AND pr.version = circle_versions.version)
           AND NOT ${PINNED_EVIDENCE_SQL("circle_versions")}`,
      )
      .bind(bid, version - KEPT_VERSIONS, now),
    // (round 39, critic 46b V4: only the versions this commit pruned, by the prune index, not every pruned version of the bank)
    c.db.prepare(`DELETE FROM circle_version_blobs WHERE version_id IN (${PRUNED_NOW_SQL}) AND ${won}`).bind(bid, now),
    // Round 37: confirmed student data is re-checked INSIDE the commit, against this version's own keys (a block
    // confirmed after the scan above still holds it: no gap). A version without it lifts the hold an earlier one
    // caused; an admin's own hold and report holds stay.
    c.db.prepare(`DELETE FROM circle_holds WHERE bank_id = ?1 AND source = 'privacy' AND ${won} AND NOT ${PRIVACY_SQL("?2", "?3")}`).bind(bid, keysJson, pShas),
    c.db
      .prepare(`INSERT OR IGNORE INTO circle_holds (bank_id, reason, source, at, by) SELECT ?1, 'student_data', 'privacy', ?4, NULL WHERE ${won} AND ${PRIVACY_SQL("?2", "?3")}`)
      .bind(bid, keysJson, pShas, now),
    c.db.prepare(`DELETE FROM circle_holds WHERE bank_id = ?1 AND source = 'privacy_near' AND ?2 = 0 AND ${won}`).bind(bid, nearN),
    c.db.prepare(`INSERT OR IGNORE INTO circle_holds (bank_id, reason, source, at, by) SELECT ?1, 'student_data', 'privacy_near', ?3, NULL WHERE ?2 > 0 AND ${won}`).bind(bid, nearN, now),
    // Round 44 (plan 13.3): a copyright re-upload block, by the public path's rules, re-checked here (no gap): the same
    // pattern as student data (the version withheld, the bank held while it is the head, lifted by a later version
    // without it), so nothing a member sees tells the two apart.
    c.db.prepare(`DELETE FROM circle_holds WHERE bank_id = ?1 AND source = 'blocklist' AND ${won} AND NOT ${COPYRIGHT_SQL("?2", "?3", "?4", "?5")}`).bind(bid, keysJson, pShas, now, me),
    c.db
      .prepare(`INSERT OR IGNORE INTO circle_holds (bank_id, reason, source, at, by) SELECT ?1, 'copyright', 'blocklist', ?4, NULL WHERE ${won} AND ${COPYRIGHT_SQL("?2", "?3", "?4", "?5")}`)
      .bind(bid, keysJson, pShas, now, me),
    c.db
      .prepare(
        `UPDATE circle_versions SET status = 'current', version = ?2, seq = (SELECT n FROM pool_seq WHERE id = 1), committed_at = ?3,
                -- Round 38 (critics 45a/45b B1): this version matched confirmed student data (the privacy holds above say
                -- so exactly): it is withheld until an administrator releases it; no later version lifts that.
                withheld_at = CASE WHEN EXISTS (SELECT 1 FROM circle_holds w1 WHERE w1.bank_id = ?5 AND w1.source IN ${SEND_HOLD_SOURCES}) THEN ?3 END,
                result_json = json_set(?4,
                  '$.held', json(CASE WHEN EXISTS (SELECT 1 FROM circle_holds h WHERE h.bank_id = ?5) THEN 'true' ELSE 'false' END),
                  '$.withheld', json(CASE WHEN EXISTS (SELECT 1 FROM circle_holds w2 WHERE w2.bank_id = ?5 AND w2.source IN ${SEND_HOLD_SOURCES}) THEN 'true' ELSE 'false' END))
          WHERE id = ?1 AND status = 'staging' AND ${won}`,
      )
      .bind(vid, version, now, JSON.stringify(x.result), bid),
    // Round 39 (critic 46b N1): the bank's listing from its newest version that is not withheld (this one, unless it is).
    c.db.prepare(LISTING_REFRESH_SQL(won)).bind(bid),
    c.db.prepare(`INSERT OR IGNORE INTO circle_version_blobs (version_id, sha256) SELECT ?1, value FROM json_each(?2) WHERE ${won}`).bind(vid, JSON.stringify(x.shas)),
    c.db
      .prepare(
        `INSERT OR IGNORE INTO circle_qkeys (circle_id, match_hash, bank_id, first_version, first_seq, first_by)
         SELECT ?1, j.value, ?2, ?3, (SELECT n FROM pool_seq WHERE id = 1), ?4 FROM json_each(?5) j WHERE ${won}`,
      )
      .bind(cid, bid, version, me, JSON.stringify(x.hashes)),
    c.db.prepare(`UPDATE circles SET qkeys = qkeys + changes() WHERE id = ?1 AND ${won}`).bind(cid),
    // Round 43 (D12, D13): the served form's key-free hashes where a label stripped for viewers changes them, under the
    // first sender of each full text (just recorded above, or an earlier version's), so a viewer's copy meets D13 too.
    c.db.prepare(STRIPPED_QKEYS_SQL("?1", "?2", "?3", won)).bind(cid, bid, JSON.stringify(x.pairs)),
    c.db.prepare(`UPDATE circles SET qkeys = qkeys + changes() WHERE id = ?1 AND ${won}`).bind(cid),
    // (the own-work affirmation, recorded with the version as a publish's is; a rollback sends no new work)
    ...(x.kind === "send" ? [c.db.prepare(`INSERT OR IGNORE INTO attestations (publish_id, item_id, user_id, at, terms_version) SELECT ?1, ?2, ?3, ?4, ?5 WHERE ${won}`).bind(vid, bid, me, now, POOL_TERMS_VERSION)] : []),
    // Round 37: one flag per version that matches confirmed student data (ids, positions and counts only). Round 47
    // (critic 54b N1): with the matching block hashes, and at most circle_send_flags a day from one sender in one circle.
    c.db
      .prepare(
        `INSERT INTO admin_flags (id, kind, item_id, publish_id, user_id, detail, at)
         SELECT ?1, 'privacy_text_match', ?2, ?3, ?4, ?5, ?6 WHERE ${won} AND EXISTS (SELECT 1 FROM circle_holds h WHERE h.bank_id = ?2 AND h.source IN ('privacy', 'privacy_near'))
           AND ${FLAG_ROOM_SQL("?4", "?7", "?8", "?9")}`,
      )
      .bind(newId(), bid, vid, me, JSON.stringify({ circle_id: cid, version, positions: few(x.privacy.exact), near: few(x.privacy.near), hashes: x.privacy.hits, ...(x.rollbackOf !== null ? { rollback_of: x.rollbackOf } : {}) }), now, cid, day0, flagCap),
    // Round 44: one `blocklist` flag per version a copyright re-upload block holds (ids, positions, counts only).
    c.db
      .prepare(
        `INSERT INTO admin_flags (id, kind, item_id, publish_id, user_id, detail, at)
         SELECT ?1, 'blocklist', ?2, ?3, ?4, ?5, ?6 WHERE ${won} AND EXISTS (SELECT 1 FROM circle_holds h WHERE h.bank_id = ?2 AND h.source = 'blocklist')
           AND ${FLAG_ROOM_SQL("?4", "?7", "?8", "?9")}`,
      )
      .bind(newId(), bid, vid, me, JSON.stringify({ circle_id: cid, version, positions: few(x.privacy.copyright), actions: x.privacy.actions, hashes: x.privacy.chits, ...(x.rollbackOf !== null ? { rollback_of: x.rollbackOf } : {}) }), now, cid, day0, flagCap),
    // Round 48 (critic 55a V3): a version withheld past the day's flags is named on the bank's cap flag.
    c.db
      .prepare(CAP_FLAG_SQL(won))
      .bind(newId(), bid, me, cid, version, vid, JSON.stringify(x.privacy.actions), x.privacy.exact.length + x.privacy.near.length > 0 ? "true" : "false", capDedup(bid, me, now), now),
    // Round 47 (critic 54b N1): the evidence kept in full stays within its bound.
    ...evidenceBound(c, cid, now, caps.bytes, won),
    audit(
      c.db,
      {
        actorId: me,
        actorKind: "user",
        action: x.kind === "rollback" ? "circle.rollback" : x.isNew ? "circle.bank_add" : "circle.version",
        itemId: cid,
        targetUserId: me,
        detail: { bank_id: bid, version, version_id: vid, ...x.auditDetail },
      },
      now,
      won,
    ),
    c.db
      .prepare(
        `SELECT result_json, EXISTS (SELECT 1 FROM admin_flags f WHERE f.publish_id = ?1 AND f.kind = 'privacy_text_match') AS flagged,
                EXISTS (SELECT 1 FROM admin_flags f WHERE f.publish_id = ?1 AND f.kind = 'blocklist') AS cflagged,
                EXISTS (SELECT 1 FROM circle_holds h WHERE h.bank_id = ?2 AND h.source IN ('privacy', 'privacy_near')) AS pheld,
                EXISTS (SELECT 1 FROM circle_holds h WHERE h.bank_id = ?2 AND h.source = 'blocklist') AS cheld,
                (SELECT cf.at = ?3 AND json_array_length(cf.detail, '$.versions') = 1 FROM admin_flags cf WHERE cf.dedup = ?4) AS capnew
           FROM circle_versions WHERE id = ?1 AND status = 'current'`,
      )
      .bind(vid, bid, now, capDedup(bid, me, now)),
  ]);
  if ((res[1]!.meta.changes ?? 0) !== 1) {
    // Lost: the staging row and its objects go now (recorded as orphans first, round 39: a failed R2 delete is
    // retried hourly); the reason is read from the state.
    await c.db.batch([
      c.db.prepare(`DELETE FROM circle_versions WHERE id = ?1 AND status = 'staging'`).bind(vid),
      c.db.prepare(`INSERT OR IGNORE INTO r2_orphans (key, at) SELECT value, ?2 FROM json_each(?1)`).bind(JSON.stringify([fullKey, indexKey]), now),
    ]);
    await deleteKeys(c, [fullKey, indexKey]);
    throw await whyLost(c, x.kind, me, cid, x.isNew ? null : bid, { base: x.base, license: x.license, bytes: x.bytes, n: x.n, newKeys: x.newKeys, hideKey: x.hideKey, caps });
  }
  const out = (res[res.length - 1]!.results as { result_json: string; flagged: number; cflagged: number; pheld: number; cheld: number; capnew: number | null }[])[0];
  if (out?.flagged === 1) await alertBudgeted(c, { kind: "privacy_text_match", itemId: bid, circleId: cid, version, held: true });
  if (out?.cflagged === 1) await alertBudgeted(c, { kind: "circle_copyright_match", itemId: bid, circleId: cid, version });
  // Round 44: the admin is told (ids only, its own hourly budget), and the matching questions count toward the
  // account's held-publish budget, the one the public path keeps (publish.ts HELD_PUBLISHES_PER_DAY): past it the
  // administrator is flagged, so probing the block list through circles is bounded and seen as through publishes.
  // (round 47: charged for every match, flagged or past the day's flags)
  if (out?.cheld === 1) await chargeHeldPublishes(c, me, Math.max(1, x.privacy.copyright.length));
  // Round 47 (critic 54b N1): past circle_send_flags the version is withheld without a flag; the admin is told once a day
  // per bank (round 48: the bank's cap flag, CAP_FLAG_SQL, names it).
  if (out?.capnew === 1) await alertBudgeted(c, { kind: "cap", userId: me, metric: "circle_send_flags" });
  const status = x.isNew ? 201 : 200;
  return { response: json(status, out ? (JSON.parse(out.result_json) as object) : { ...x.result, held: false }), status };
}

/** Round 44: questions a copyright re-upload block held count toward the account's held-publish budget (the public
 *  path's, publish.ts HELD_PUBLISHES_PER_DAY); past it the administrator is flagged. */
export async function chargeHeldPublishes(c: Ctx, me: string, n: number): Promise<void> {
  const held = await c.db
    .prepare(
      `INSERT INTO usage_daily (user_id, day, metric, count) SELECT ?1, ?2, 'held_publishes', ?3 WHERE ${ALIVE_SQL("?1")}
       ON CONFLICT (user_id, day, metric) DO UPDATE SET count = count + excluded.count RETURNING count`,
    )
    .bind(me, dayOf(c.now()), n)
    .first<{ count: number }>();
  if ((held?.count ?? 0) >= HELD_PUBLISHES_PER_DAY) await capAlert(c, me, "held_publishes");
}

const licenseAuthorOnly = (license: string) =>
  new PoolError(422, "license_author_only", { license, message: "Only the bank's author chooses its licence. Send your version with the bank's current licence." });

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
async function whyLost(
  c: Ctx,
  kind: "send" | "rollback",
  me: string,
  cid: string,
  bid: string | null,
  w: { base: number | null; license: string; bytes: number; n: number; newKeys: number; hideKey: number | null; caps: { bytes: number; questions: number; records: number; banks: number } },
): Promise<PoolError> {
  await throwIfErased(c, me);
  const s = await c.db
    .prepare(
      `SELECT ci.closed_at, ci.close_reason, ci.undo_until, ci.qkeys, m.role, ${BYTES_SQL("?1")} AS used, ${PRUNABLE_SQL("?3", "?4")} AS prunable, ${QUESTIONS_SQL("?1", "?3")} AS questions, ${BANKS_SQL("?1")} AS banks,
              b.head_version, b.removed_at, b.removed_reason, b.author_id, b.license, b.hide_key, EXISTS (SELECT 1 FROM circle_holds h WHERE h.bank_id = b.id) AS held, ${BLOCKING_HOLD_SQL("b.id")} AS blocking, ${AUTHOR_OK_SQL("b")} AS author_ok
         FROM circles ci LEFT JOIN circle_members m ON m.circle_id = ci.id AND m.user_id = ?2 AND m.left_at IS NULL LEFT JOIN circle_banks b ON b.id = ?3 AND b.circle_id = ci.id
        WHERE ci.id = ?1`,
    )
    .bind(cid, me, bid ?? "", (w.base ?? 0) + 1 - KEPT_VERSIONS)
    .first<{
      closed_at: number | null;
      prunable: number;
      close_reason: string | null;
      undo_until: number | null;
      qkeys: number;
      role: string | null;
      used: number;
      questions: number;
      banks: number;
      head_version: number | null;
      removed_at: number | null;
      removed_reason: string | null;
      author_id: string | null;
      license: string | null;
      hide_key: number | null;
      held: number | null;
      blocking: number | null;
      author_ok: number | null;
    }>();
  if (!s || s.role === null) return new PoolError(404, "not_found");
  if (s.closed_at !== null) return new PoolError(409, "circle_closed", { reason: s.close_reason, undo_until: s.undo_until });
  if (kind === "rollback" && !(s.role === "owner" || (s.author_id === me && s.role === "editor"))) return new PoolError(403, "circle_role_required", { needed: OWNER_OR_AUTHOR, role: s.role });
  if (s.role !== "owner" && s.role !== "editor") return new PoolError(403, "circle_role_required", { needed: "editor", role: s.role });
  if (bid !== null) {
    if (s.removed_at !== null) return new PoolError(409, "bank_removed", { reason: s.removed_reason });
    if (s.author_id !== me && ((s.held === 1 && !(s.role === "owner" && s.blocking === 0)) || s.author_ok !== 1)) return new PoolError(404, "not_found");
    if (s.head_version !== w.base) return conflict(c, bid);
    if (s.author_id !== me && s.license !== w.license) return licenseAuthorOnly(s.license ?? "");
    // Round 48 (critic 55a V2): the head did not move, the answer-key setting did: send again (not a merge).
    if (w.hideKey !== null && s.hide_key !== null && s.hide_key !== w.hideKey)
      return new PoolError(409, "hide_key_changed", {
        hide_key: s.hide_key === 1,
        head_version: s.head_version,
        message:
          s.hide_key === 1
            ? "The bank's answer keys were hidden from viewers while this was being sent. Nothing was stored: send it again, with the decisions for its viewers' form."
            : "The bank's answer-key setting changed while this was being sent. Nothing was stored: send it again.",
      });
  }
  if (bid === null && s.banks >= w.caps.banks) return new PoolError(422, "circles_limit", { metric: "circle_banks", used: s.banks, limit: w.caps.banks, message: `This circle has ${w.caps.banks} banks, the most allowed.` });
  if (s.used - s.prunable + w.bytes > w.caps.bytes) return new PoolError(413, "quota_exceeded", { scope: "circle", used: s.used - s.prunable, needed: w.bytes, limit: w.caps.bytes, message: QUOTA_MESSAGE });
  if (s.questions + w.n > w.caps.questions) return new PoolError(422, "circle_questions_limit", { used: s.questions, needed: w.n, limit: w.caps.questions, message: `A circle holds at most ${w.caps.questions} questions across its banks.` });
  if (s.qkeys + w.newKeys > w.caps.records) return new PoolError(413, "circle_records_full", { used: s.qkeys, needed: w.newKeys, limit: w.caps.records, message: "This circle has recorded as many different questions as it can keep." });
  if (bid !== null) return conflict(c, bid);
  return new PoolError(409, "conflict", { message: "Something changed meanwhile; try again." });
}

// ---- rollback (round 37, D17) -------------------------------------------------------------------------------------

/**
 * POST /circles/:cid/banks/:bid/rollback {to_version, base_version, commit_id, message?, confirm_large_change?}:
 * a NEW version whose body is a kept version's (copied from R2, never re-validated; its qids, images and
 * recorded origins with it), accepted only if the head is still `base_version`. The owner, or the bank's
 * author while an editor (D17: a rollback takes others' later work out of the head; it stays in the history).
 * Never to a pruned version (`404 version_not_retained {oldest_kept}`) or to the head itself (400). Dropping
 * more than half of the head's questions needs `confirm_large_change`. CIRCLE_ROLLBACKS a day (D10).
 */
export async function rollbackBank(c: Ctx, req: Request, p: PoolPrincipal, cid: string, bid: string): Promise<Response> {
  const me = p.userId;
  const { m, b } = await bankOrMiss(c, cid, bid, me);
  const body = await readJson(req, `u:${me}`, { deadlineMs: c.deps.bodyDeadlineMs });
  await requireReadable(c, m, b, me);
  if (!(m.role === "owner" || (b.author_id === me && m.role === "editor"))) throw new PoolError(403, "circle_role_required", { needed: OWNER_OR_AUTHOR, role: m.role });
  if (typeof body.commit_id !== "string" || !PUBLISH_ID_RE.test(body.commit_id)) throw bad("commit_id");
  if (!intAtLeast1(body.base_version)) throw bad("base_version");
  if (!intAtLeast1(body.to_version) || (body.to_version as number) >= (body.base_version as number))
    throw new PoolError(400, "invalid_request", { field: "to_version", message: "Roll back to an earlier version than the one you are looking at." });
  if (body.confirm_large_change !== undefined && typeof body.confirm_large_change !== "boolean") throw bad("confirm_large_change");
  const r = { commitId: body.commit_id, to: body.to_version as number, base: body.base_version as number, message: messageOf(body.message), confirm: body.confirm_large_change === true };
  return locked(c, me, "circle_rollbacks", (refresh, held) => rollbackOnce(c, me, cid, bid, r, refresh, held));
}

async function currentListing(c: Ctx, bid: string, title: string): Promise<string> {
  const b = await c.db
    .prepare(`SELECT json_object('title', CASE WHEN ?2 <> '' THEN ?2 ELSE title END, 'subject', subject, 'course', course, 'grade', grade) AS l FROM circle_banks WHERE id = ?1`)
    .bind(bid, title)
    .first<{ l: string }>();
  return b?.l ?? JSON.stringify({ title, subject: "", course: "", grade: "" });
}

async function rollbackOnce(
  c: Ctx,
  me: string,
  cid: string,
  bid: string,
  r: { commitId: string; to: number; base: number; message: string | null; confirm: boolean },
  refresh: () => Promise<void>,
  held: (() => void)[],
): Promise<Result> {
  const now = c.now();
  const requestSha = await sha256Hex(enc.encode(JSON.stringify(["circle-rollback/v1", bid, r.to, r.base, r.message ?? ""])));
  const replay = await replayOf(c, me, cid, bid, r.commitId, requestSha, now);
  if (replay) return { response: replay, replayed: true, status: 200 };
  const bank = await freshBank(c, cid, bid);
  if (bank.head_version !== r.base) throw await conflict(c, bid);
  const target = await c.db
    .prepare(`SELECT id, status, full_key, index_key, bytes, question_count, origins, withheld_at, listing, strip_opts FROM circle_versions WHERE bank_id = ?1 AND version = ?2`)
    .bind(bid, r.to)
    .first<{ id: string; status: string; full_key: string; index_key: string; bytes: number; question_count: number; origins: string | null; withheld_at: number | null; listing: string | null; strip_opts: string | null }>();
  if (!target) throw new PoolError(404, "not_found");
  const notKept = () => notRetained(c, bid, bank.head_version);
  if ((target.status !== "current" && target.status !== "previous") || target.full_key === "") throw await notKept();
  // Round 38 (B1): never back to a version that held confirmed student data (until an administrator releases it).
  if (target.withheld_at !== null) throw withheldError();
  const headRow = bank.head_version_id ? await c.db.prepare(`SELECT index_key FROM circle_versions WHERE id = ?1`).bind(bank.head_version_id).first<{ index_key: string }>() : null;
  const [tIdx, hIdx] = [await readIndex(c, target.index_key), headRow ? await readIndex(c, headRow.index_key) : null];
  if (!tIdx) throw await notKept();
  const { added, changed, removed } = diff(hIdx, tIdx);
  if (!r.confirm && hIdx && isLarge(removed, hIdx.qids.length)) throw largeError(removed, hIdx.qids.length);
  // The bank's licence stays (the author's choice, D19); the copied questions' recorded origins must allow it.
  let origins: Origin[] = [];
  try {
    origins = target.origins ? (JSON.parse(target.origins) as Origin[]) : [];
  } catch {
    origins = [];
  }
  const allowed = allowedByAll(origins.map((o) => o.license));
  if (origins.length > 0 && !allowed.includes(bank.license as License))
    throw new PoolError(422, "license_incompatible", { allowed, message: "That version holds public questions whose licence does not allow this bank's current licence." });
  // Origins keep their circle qid (a later send carries them on).
  origins = origins.map((o) => ({ ...o, cq: o.cq ?? tIdx.qids[o.pos] }));

  // The body is copied (R2 get to put). Its student-data check needs its questions: publish-sized work, so the
  // heavy slot and a reservation as for a send of that size.
  held.push(await acquireHeavy(c.deps.heavyQueueMs ?? HEAVY_QUEUE_WAIT_MS, refresh, c.deps.heavySlots));
  const obj = await c.r2.get(target.full_key);
  if (!obj) throw await notKept();
  const size = (obj as R2ObjectBody).size;
  held.push(reserveBytes(size * (PUBLISH_COST_PER_BYTE - 1)));
  const bodyBytes = new Uint8Array(await (obj as R2ObjectBody).arrayBuffer()); // r2-object: a kept version's body (our own write)
  // Round 38 (critic 45b variant 4): reserved as a send of it would be: by its value count when that is more.
  const extra = scanJson(bodyBytes).values * PARSE_BYTES_PER_VALUE * 2 - size * (PUBLISH_COST_PER_BYTE - 1);
  if (extra > 0) held.push(reserveBytes(extra));
  const shas = (await c.db.prepare(`SELECT sha256 FROM circle_version_blobs WHERE version_id = ?1`).bind(target.id).all<{ sha256: string }>()).results.map((x) => x.sha256);
  // (round 39, critic 46b V9: the images it copies are held for the account until the commit, as a send's are)
  if (shas.length > 0)
    await c.db
      .prepare(
        `INSERT INTO pending_blobs (sha256, uploader_id, size, until) SELECT sha256, ?2, size, ?3 FROM blobs WHERE sha256 IN (SELECT value FROM json_each(?1)) AND ${ALIVE_SQL("?2")}
         ON CONFLICT (sha256, uploader_id) DO UPDATE SET until = MAX(until, excluded.until)`,
      )
      .bind(JSON.stringify(shas), me, now + DAY_MS)
      .run();
  let qs: QuestionJson[];
  let bodyName = "";
  try {
    const pj = JSON.parse(new TextDecoder().decode(bodyBytes)) as PayloadJson;
    qs = questionsOf(pj);
    bodyName = typeof pj.bank?.name === "string" ? (cleanText(pj.bank.name, 200) ?? "") : "";
  } catch {
    throw await notKept();
  }
  // The key-free match hashes were kept in the index from round 37 on; an older version's are computed again.
  let mh = tIdx.m;
  if (!mh) {
    const x = c.core.matchHashes(bodyBytes);
    if (isFail(x)) throw await notKept();
    mh = x;
  }
  const imgAt = imagesAt(qs);
  // Round 46 (critic 53b B2): a bank that hides its keys serves its viewers the target's stripped form with the
  // decisions it recorded (copied with it): that form is checked too. (No decisions recorded: viewers are refused it.)
  let viewerForm: ScanForm | null = null;
  if (bank.hide_key === 1 && target.strip_opts) {
    const st = c.core.strip(bodyBytes, decisionsOf(target.strip_opts));
    if (!isFail(st)) {
      st.servedOrders = [];
      const vq = questionsOf(st.payload);
      viewerForm = { qs: vq, mh: st.matchHashes, imgAt: imagesAt(vq) };
    }
  }
  const privacy = await privacyScan(c, [{ qs, mh, imgAt }, ...(viewerForm ? [viewerForm] : [])], me);
  viewerForm = null;
  qs.length = 0;
  await refresh();
  const index = enc.encode(JSON.stringify({ qids: tIdx.qids, h: tIdx.h, m: mh }));
  // (charged per version as a send is, D3: the body, the index and each image it references)
  const imageBytes = shas.length
    ? ((await c.db.prepare(`SELECT COALESCE(SUM(size), 0) AS n FROM blobs WHERE sha256 IN (SELECT value FROM json_each(?1))`).bind(JSON.stringify(shas)).first<{ n: number }>())?.n ?? 0)
    : 0;
  const vid = newId();
  const version = r.base + 1;
  return commitVersion(c, {
    kind: "rollback",
    me,
    cid,
    bid,
    vid,
    isNew: false,
    base: r.base,
    version,
    commitId: r.commitId,
    requestSha,
    message: r.message,
    license: bank.license,
    meta: null,
    // (round 39: the target's listing comes back with its body; a version from before round 39 has none: round 40,
    // critic 47a V3, its own bank name from its body as the title, the bank's current subject, course and grade)
    listing: target.listing ?? (await currentListing(c, bid, bodyName)),
    n: target.question_count,
    hasImages: shas.length > 0 ? 1 : 0,
    bytes: bodyBytes.length + index.length + imageBytes,
    newKeys: 0,
    hashes: [],
    shas,
    origins,
    mergedFrom: null,
    rollbackOf: r.to,
    privacy,
    // (round 43: the target's strip decisions come back with its body; its served-form records exist already)
    stripOpts: target.strip_opts,
    pairs: [],
    // (round 47, critic 54b V9: compared at the commit, as a send's: hidden keys turned on meanwhile is a conflict)
    hideKey: bank.hide_key,
    result: { circle_id: cid, bank_id: bid, version, version_id: vid, qids: tIdx.qids, added, changed, removed, rollback_of: r.to },
    auditDetail: { added, changed, removed, rollback_of: r.to },
    put: async (fullKey, indexKey) => {
      await c.r2.put(fullKey, bodyBytes, { httpMetadata: { contentType: "application/json" } });
      await c.r2.put(indexKey, index, { httpMetadata: { contentType: "application/json" } });
    },
    refresh,
  });
}
