// POST /items (new item) and PUT /items/:id (republish): docs/shared-pool.md "Publish order".
//
// validate (share-core) -> server rewrite (ids, times, image refs from the verified blobs, client
// origin/published dropped) -> validate again -> strip (share-core, hide_key only) -> hashes ->
// licence hard check against the author's own imports, soft check, blocklist -> staged rows (quota
// checked in the same INSERT) -> R2 bodies -> derived rows -> one compare-and-set commit batch ->
// retention (current + previous). A lost CAS is a 409 and the staged rows were never reachable.
import { cleanText } from "../../../auth/src/validate.ts";
import { scanJson, PARSE_BYTES_PER_VALUE } from "./jsonscan.ts";
import { licenseCheck, storedAllowed } from "./licensecheck.ts";
import { canonical, isFail, kindTag, questionsOf, type Fail, type LabelDecision, type PayloadJson, type Pos, type QuestionJson, type Stripped, type Validated } from "./core.ts";
import { EXT } from "./image.ts";
import { allowedForBank, ATTEST_STATEMENT, ATTEST_TEXT_REVS, POOL_TERMS_VERSION, decideLicence, grantOptions, isLicense, LICENSES, mayRepublish, reliedLicence, unitAllows, type License, type LicenceUnit } from "./license.ts";
import { subjectFor, subjectParam } from "./visible.ts";
import { checkCircleMaterial, readCircleClaim } from "./circlecheck.ts";
import { privacyNearPositions } from "./privacynear.ts";
import { rightfulExempts, SERVED_FORM, servedBlobsOf, upstreamCarried } from "./served.ts";
import {
  authorContext,
  chainOf,
  chainOfLinks,
  CREDITS_HEAD,
  CREDITS_MAX,
  evidence,
  isHard,
  keyfreeHash,
  linkGrants,
  nearHash,
  nearKfHash,
  fuzzyHits,
  fieldKeys,
  normSkeletonKey,
  stemKey,
  ownContentFirst,
  minFirst,
  ownFuzzyFirst,
  bodyTokens,
  unitKeysOf,
  partKeysOf,
  textFieldKeys,
  publicNearEarlier,
  FUZZY_JACCARD,
  FUZZY_ROWS_MAX,
  jaccard,
  normText,
  stemBands,
  stemTokens,
  unitOf,
  type FuzzyCut,
  ownItemSet,
  stemHash,
  stemHits,
  voidInfo,
  voidedRefs,
  type ChainLink,
  type ImportHit,
} from "./provenance.ts";
import {
  REMOVED_SQL,
  audit,
  chunks,
  ALIVE_SQL,
  throwIfErased,
  countMiss,
  refundMiss,
  DAY_MS,
  missGuard,
  ID_RE,
  json,
  limitOf,
  MB,
  newId,
  PoolError,
  PUBLISH_ID_RE,
  dayOf,
  capAlert,
  FLAG_DETAIL_MAX,
  FLAGS_PER_ACCOUNT_DAY,
  COLLAPSED_FLAGS,
  INTEGRITY_FLAGS,
  alertBudgeted,
  readBody,
  parsePublish,
  isPublishType,
  PUBLISH_COST_PER_BYTE,
  reserveBytes,
  lender,
  requireDaily,
  sha256Hex,
  SHARED_SQL,
  usage,
  USAGE_SQL,
  VISIBLE_TO_OTHERS,
  type Ctx,
  type HeavySlots,
  type PoolPrincipal,
  anonChain,
  qKeyEq,
  qKeyIn,
  inQKeysOf,
  assertNotMigrating,
  errKind,
  keyedBands,
  keyedBody,
  keyedSet,
} from "./shared.ts";

export const MAX_PAYLOAD = 4 * MB;
export const MAX_META = 1 * MB;
/** A staging/staged publish younger than this is assumed to be in flight (a concurrent retry gets 409). */
export const IN_FLIGHT_MS = 60_000;
/** Links SHOWN in a chain (round 9 J6: the stored credit list keeps up to CREDITS_MAX + 2). */
export const CHAIN_MAX = 32;
export const STORED_CHAIN_MAX = CREDITS_MAX + 2;
/** Publishes of one account a day that may end up held by a content block (round 9). */
export const HELD_PUBLISHES_PER_DAY = 20;
/** How long an admin's release of one item's hold covers that item's later publishes (round 13). */
export const HOLD_RELEASE_MS = 90 * 86_400_000;
/** A qid's lineage holds only while its stem stays this close (token Jaccard) to the lineage root (round 14 B6). */
export const LINEAGE_ROOT_JACCARD = 0.6;

const KIND_BITS: Record<string, number> = { Mc: 1, Tf: 2, Matching: 4, Blank: 8, Short: 16, Multipart: 32 };
const MEDIA_RE = /^media\/([0-9a-f]{64})\.(png|jpg|gif|webp)$/;
const enc = new TextEncoder();


interface PublishMeta {
  publishId: string;
  base: string | null;
  hideKey: boolean;
  license: License;
  attestRev: string;
  /** Round 23: the publishing-terms affirmation (checked at publish, recorded; null when absent or malformed). */
  attest: { ownWork: boolean; termsVersion: string } | null;
  labels: LabelDecision[];
  orderAcks: Pos[];
  imageOk: number[];
  /** Questions whose low-risk formatting findings the author acknowledged (share-core round 6). */
  ackFormatting: number[];
  visibility: "public" | "unlisted" | null;
  meta: unknown;
  /** Items whose plain reads the author attests had no part in this publish (round 6 F5). */
  independent: string[];
  /** The author's own words for an independence claim (stored in the audit and the admin flag). */
  statement: string | null;
}

const isNat = (x: unknown): x is number => typeof x === "number" && Number.isInteger(x) && x >= 0 && x < 1_000_000;

function readMeta(m: Record<string, unknown>, isNew: boolean): PublishMeta {
  const bad = (field: string) => new PoolError(400, "invalid_request", { field });
  if (typeof m.publish_id !== "string" || !PUBLISH_ID_RE.test(m.publish_id)) throw bad("publish_id");
  let base: string | null = null;
  if (isNew) {
    if (m.base_publish_id !== undefined && m.base_publish_id !== null) throw bad("base_publish_id");
  } else {
    if (typeof m.base_publish_id !== "string" || !PUBLISH_ID_RE.test(m.base_publish_id)) throw bad("base_publish_id");
    base = m.base_publish_id;
  }
  if (typeof m.hide_key !== "boolean") throw bad("hide_key");
  // Nothing is preselected: the licence must be chosen explicitly on every publish.
  if (!isLicense(m.license)) throw bad("license");
  const at = m.attestation as Record<string, unknown> | undefined;
  if (!at || typeof at !== "object" || at.accepted !== true || !(ATTEST_TEXT_REVS as readonly unknown[]).includes(at.text_rev)) throw bad("attestation");
  const labels = m.labels ?? [];
  if (!Array.isArray(labels) || labels.length > 60_000) throw bad("labels");
  const outLabels: LabelDecision[] = [];
  for (const l of labels as Record<string, unknown>[]) {
    if (!l || typeof l !== "object" || !isNat(l.question) || (l.part !== undefined && !isNat(l.part)) || (l.choice !== "strip" && l.choice !== "keep")) throw bad("labels");
    outLabels.push(l.part === undefined ? { question: l.question, choice: l.choice } : { question: l.question, part: l.part as number, choice: l.choice });
  }
  const acks = m.order_acks ?? [];
  if (!Array.isArray(acks) || acks.length > 60_000) throw bad("order_acks");
  const outAcks: Pos[] = [];
  for (const a of acks as Record<string, unknown>[]) {
    if (!a || typeof a !== "object" || !isNat(a.question) || (a.part !== undefined && !isNat(a.part))) throw bad("order_acks");
    outAcks.push(a.part === undefined ? { question: a.question } : { question: a.question, part: a.part as number });
  }
  const img = m.image_ok ?? [];
  if (!Array.isArray(img) || img.length > 1000 || !img.every(isNat)) throw bad("image_ok");
  const ackF = m.ack_formatting ?? [];
  if (!Array.isArray(ackF) || ackF.length > 60_000 || !ackF.every(isNat)) throw bad("ack_formatting");
  let visibility: PublishMeta["visibility"] = null;
  if (isNew) {
    if (m.visibility !== "public" && m.visibility !== "unlisted") throw bad("visibility");
    visibility = m.visibility;
    if (visibility === "public" && m.license === "circle-only") throw new PoolError(422, "license_not_public");
  }
  // "I wrote these independently of items I only looked at": voids those READS (never imports) as
  // evidence for this publish. Attested, audited and flagged for the admin.
  let independent: string[] = [];
  let statement: string | null = null;
  const ind = m.independent_of as Record<string, unknown> | undefined | null;
  if (ind !== undefined && ind !== null) {
    const items = ind && typeof ind === "object" && !Array.isArray(ind) ? ind.items : undefined;
    if (ind.accepted !== true || !Array.isArray(items) || items.length === 0 || items.length > 50 || !items.every((x) => typeof x === "string" && ID_RE.test(x))) throw bad("independent_of");
    independent = [...new Set(items as string[])];
    statement = cleanText(ind.statement, 1000);
    if (!statement || statement.length < 10) throw bad("independent_of.statement");
  }
  return {
    publishId: m.publish_id,
    base,
    hideKey: m.hide_key,
    license: m.license,
    attestRev: at.text_rev as string,
    attest:
      m.attest && typeof m.attest === "object" && !Array.isArray(m.attest) && typeof (m.attest as Record<string, unknown>).terms_version === "string"
        ? { ownWork: (m.attest as Record<string, unknown>).own_work === true, termsVersion: String((m.attest as Record<string, unknown>).terms_version).slice(0, 40) }
        : null,
    labels: outLabels,
    orderAcks: outAcks,
    imageOk: img as number[],
    ackFormatting: [...new Set(ackF as number[])],
    visibility,
    meta: m.meta ?? {},
    independent,
    statement,
  };
}

function invalid(f: Fail, status = 422): PoolError {
  const extra: Record<string, unknown> = { errors: f.errors };
  if (f.label_findings) extra.label_findings = f.label_findings;
  if (f.needs_order_ack) extra.needs_order_ack = f.needs_order_ack;
  if (f.order_reasons) extra.order_reasons = f.order_reasons;
  // The author's own previews (share-core round 6); this response reaches the publishing author only.
  if (f.order_preview) extra.order_preview = f.order_preview;
  if (f.formatting_findings) {
    extra.formatting_findings = f.formatting_findings;
    extra.formatting_findings_total = f.formatting_findings_total ?? f.formatting_findings.length;
  }
  const code = f.errors.some((e) => e.code === "order_ack_required") ? "order_ack_required" : f.errors.some((e) => e.code.startsWith("labels_")) ? "labels_need_decision" : "invalid_payload";
  return new PoolError(status, code, extra);
}


function hashesOrThrow(x: string[] | Fail): string[] {
  if (isFail(x)) throw invalid(x);
  return x;
}

/** Statements deleting a publish's derived rows and its row (bodies are deleted by the caller). */
/** New authorship records a republish of a HELD item may add past author_records (round 20 variant 3). */
export const HELD_FIX_RECORDS = 1000;
/** Student-data band rows one publish compares (round 20: past it, every question sharing a band is flagged). */
export const PRIVACY_BAND_ROWS_MAX = 2000;
/** Distinct unit keys (texts, images, parts) one publish may have; past it the publish is refused (round 21). */
export const UNIT_KEYS_MAX = 60_000;
/** Occurrences of a key counted pool-wide; past it the key is matched from the reader's own served content (round 21). */
export const OCC_SCAN = 64;
/** Round 22: a hot key's second count, up to the account's own served rows (at most this), ... */
export const HOT_SCAN = 4096;
/** ... within this many index steps per publish in all. */
export const HOT_BUDGET = 262_144;
/** Served sources kept per (question, unit key), earliest first (round 21: one material, its earliest decides). */
export const UNIT_SOURCES_PER_KEY = 8;
/** Retained occurrences looked at per unit key, earliest first (round 21). */
export const UNIT_OCC_WINDOW = 8;

/** SQL: item `item`'s licence now when its public version has unit `key` AND the item is offered to
 *  the pool (round 24, critic31a/31b: an unlisted, held or removed item's licence grants nothing;
 *  round 23 B4 already made this true of its licence-history windows). */
const NOW_UNIT = (item: string, key: string) => `(SELECT json_array(ni.license) FROM items ni WHERE ni.id = ${item} AND (EXISTS (SELECT 1 FROM item_questions nq WHERE nq.publish_id = ni.public_publish_id AND (nq.bk_expl = ${key} OR nq.bk_ans = ${key}))
                    OR EXISTS (SELECT 1 FROM item_part_keys npk WHERE npk.publish_id = ni.public_publish_id AND npk.key = ${key})
                    OR EXISTS (SELECT 1 FROM item_blobs nb LEFT JOIN blobs nbx ON nbx.sha256 = nb.sha256 WHERE nb.publish_id = ni.public_publish_id AND (nb.sha256 = ${key} OR nbx.pixel_hash = ${key})))
                    AND ${OFFERED("ni")})`;

/** The aggregation of the served-unit query (round 21: shared by its key-driven and served-driven halves). */
/** SQL on item row `i`: it is OFFERED to the pool now (public, live, not held): only then does a licence count. */
export const OFFERED = (i: string) => `(${i}.visibility = 'public' AND ${i}.removed_at IS NULL AND ${i}.purged_at IS NULL AND ${i}.current_publish_id IS NOT NULL AND NOT EXISTS (SELECT 1 FROM item_holds oh WHERE oh.item_id = ${i}.id))`;
/**
 * Rounds 22-23 (critic29a V-a, critic30b W1, critic30a B4): the item's licence history at each content
 * it offers, one row per (item, content, licence) (re-opened, never duplicated: bounded by distinct
 * contents x licences): a row is open while the item OFFERS that content under that licence, publicly
 * (unlisted, held or removed offers nothing), and closes when it stops. A serve counts every licence
 * whose row is open or closed after it. Run at a publish (end of its batch), a visibility change, and
 * the hourly reconcile (holds, removals). `publishId`: only while it is the item's current publish.
 */
export function offerSync(db: D1Database, itemId: string, now: number, publishId: string | null = null): D1PreparedStatement[] {
  const cond = publishId === null ? "?3 IS NULL" : "EXISTS (SELECT 1 FROM items gi WHERE gi.id = ?2 AND gi.current_publish_id = ?3)";

  return [
    db
      .prepare(
        `UPDATE content_licences SET until_at = ?1 WHERE item_id = ?2 AND until_at IS NULL AND ${cond}
            AND NOT EXISTS (SELECT 1 FROM items i JOIN item_offer o ON o.item_id = i.id WHERE i.id = ?2 AND o.ckey = content_licences.ckey AND i.license = content_licences.license AND ${OFFERED("i")})`,
      )
      .bind(now, itemId, publishId),
    db
      .prepare(
        `INSERT INTO content_licences (item_id, ckey, license, from_at) SELECT i.id, o.ckey, i.license, ?1 FROM items i JOIN item_offer o ON o.item_id = i.id
          WHERE i.id = ?2 AND ${OFFERED("i")} AND ${cond}
         ON CONFLICT (item_id, ckey, license) DO UPDATE SET until_at = NULL WHERE content_licences.until_at IS NOT NULL`,
      )
      .bind(now, itemId, publishId),
  ];
}
/** Old-style question licence/offer rows one hourly licence_rekey run maps (bounded, idempotent). */
export const LICENCE_REKEY_ROWS = 1000;
/**
 * Round 25 (critic-asvs-24 B3): finish re-keying the licence history from the exact content hash
 * (`q:<content>`) to the canonical served-content key (`q:<near>`) for rows the 0105 migration could
 * not map yet (their near hashes were still NULL before the cron backfill). RETAINED publishes and
 * GHOSTS are read: a version retired before the upgrade keeps its window through its ghost. Runs
 * after the backfill every hour until no mappable old-style row is left. Windows that meet are
 * merged: earliest from_at, open if any is open, else the latest until_at (no grant is lost).
 *
 * Round 25 (critic-moodle-24 B-3): the batch is selected from MAPPABLE rows only. An unmappable
 * old-style row (its content is gone) can never sit at the front of the queue and hide a mappable row
 * behind it; it is simply left at rest (it can grant nothing: no view looks its key up).
 */
export async function licenceRekey(c: Ctx): Promise<number> {
  const map = (a: string) => `(SELECT 'q:' || CASE WHEN p.hide_key = 1 THEN q.near_stripped ELSE q.near_full END
      FROM item_questions q JOIN item_publishes p ON p.publish_id = q.publish_id
     WHERE p.item_id = ${a}.item_id
       AND (CASE WHEN p.hide_key = 1 THEN q.stripped_content ELSE q.full_content END) = substr(${a}.ckey, 3)
       AND (CASE WHEN p.hide_key = 1 THEN q.near_stripped ELSE q.near_full END) IS NOT NULL
    UNION ALL
    SELECT 'q:' || CASE WHEN gp.hide_key = 1 THEN gq.near_stripped ELSE gq.near_full END
      FROM ghost_questions gq JOIN ghost_publishes gp ON gp.publish_id = gq.publish_id
     WHERE gp.item_id = ${a}.item_id
       AND (CASE WHEN gp.hide_key = 1 THEN gq.stripped_content ELSE gq.full_content END) = substr(${a}.ckey, 3)
       AND (CASE WHEN gp.hide_key = 1 THEN gq.near_stripped ELSE gq.near_full END) IS NOT NULL
    LIMIT 1)`;
  // Only rows it can map enter the bounded batch: a `q:` row (LIKE: any case) whose rest is the content
  // of a question of its item, live or ghost, with a near hash. Round 31 (staging D1 insights): found
  // from the questions' side, each content looked up by its row's key (the primary keys lead with
  // item_id, ckey), instead of testing every `q:` row of the pool against every question of its item
  // every hour (about 5.9 million rows a statement with one 990-question bank: rows x questions).
  const mappableRows = (t: string, cols: string) => `
      SELECT ${cols} FROM item_questions q CROSS JOIN item_publishes p ON p.publish_id = q.publish_id
             CROSS JOIN ${t} x ON x.item_id = p.item_id
               AND x.ckey IN ('q:' || (CASE WHEN p.hide_key = 1 THEN q.stripped_content ELSE q.full_content END), 'Q:' || (CASE WHEN p.hide_key = 1 THEN q.stripped_content ELSE q.full_content END))
       WHERE (CASE WHEN p.hide_key = 1 THEN q.near_stripped ELSE q.near_full END) IS NOT NULL
      UNION
      SELECT ${cols} FROM ghost_questions gq CROSS JOIN ghost_publishes gp ON gp.publish_id = gq.publish_id
             CROSS JOIN ${t} x ON x.item_id = gp.item_id
               AND x.ckey IN ('q:' || (CASE WHEN gp.hide_key = 1 THEN gq.stripped_content ELSE gq.full_content END), 'Q:' || (CASE WHEN gp.hide_key = 1 THEN gq.stripped_content ELSE gq.full_content END))
       WHERE (CASE WHEN gp.hide_key = 1 THEN gq.near_stripped ELSE gq.near_full END) IS NOT NULL`;
  const batchCl = `SELECT rowid, * FROM content_licences cx WHERE cx.rowid IN (${mappableRows("content_licences", "x.rowid")})
                ORDER BY cx.rowid LIMIT ${LICENCE_REKEY_ROWS}`;
  const batchOffer = `SELECT item_id, ckey FROM (${mappableRows("item_offer", "x.item_id, x.ckey")})
                                  ORDER BY item_id, ckey LIMIT ${LICENCE_REKEY_ROWS}`;
  const m = map("cl");
  const mo = map("o");
  const results = await c.db.batch([
    c.db.prepare(
      `INSERT INTO content_licences (item_id, ckey, license, from_at, until_at)
       SELECT cl.item_id, ${m}, cl.license, cl.from_at, cl.until_at
         FROM (${batchCl}) cl
        WHERE ${m} IS NOT NULL AND ${m} <> cl.ckey
       ON CONFLICT (item_id, ckey, license) DO UPDATE SET
         from_at = MIN(content_licences.from_at, excluded.from_at),
         until_at = CASE WHEN content_licences.until_at IS NULL OR excluded.until_at IS NULL THEN NULL
                         ELSE MAX(content_licences.until_at, excluded.until_at) END`,
    ),
    c.db.prepare(
      `DELETE FROM content_licences WHERE rowid IN (
         SELECT cl.rowid FROM (${batchCl}) cl
          WHERE ${m} IS NOT NULL AND ${m} <> cl.ckey)`,
    ),
    c.db.prepare(
      `WITH todo AS MATERIALIZED (${batchOffer})
       INSERT OR IGNORE INTO item_offer (item_id, ckey)
       SELECT o.item_id, ${mo} FROM todo o WHERE ${mo} IS NOT NULL AND ${mo} <> o.ckey`,
    ),
    c.db.prepare(
      `WITH todo AS MATERIALIZED (${batchOffer})
       DELETE FROM item_offer WHERE EXISTS (
         SELECT 1 FROM todo o WHERE o.item_id = item_offer.item_id AND o.ckey = item_offer.ckey AND ${mo} IS NOT NULL AND ${mo} <> o.ckey)`,
    ),
  ]);
  return (results[1]!.meta.changes ?? 0) + (results[3]!.meta.changes ?? 0);
}
/** Retained publishes one stem-backfill pass rebuilds (bounded; idempotent; resumes on the next run). */
export const STEM_BACKFILL_PUBLISHES = 4;
/**
 * Round 25 (critic-moodle-24 B-2): fill `author_stems.bk_expl_full` for records written before round
 * 24 (the column was added, never backfilled), and rebuild stem records a pre-round-23 database never
 * had. For a hidden publish the full form's explanation key is computed from its retained full body
 * in R2; a SHOWN publish's served explanation key is already in `item_questions.bk_expl`. An
 * unreadable body leaves the key NULL ("unknown"), never a reason to credit a copier. Bounded and
 * idempotent: run at a real publish and every hour by the cron.
 */
/** Round 26 (critic-moodle-33c V4): failures before an unreadable body is skipped (and the admin told). */
export const STEM_BACKFILL_FAILS = 3;
async function stemBackfillFailed(c: Ctx, publishId: string): Promise<void> {
  const r = await c.db
    .prepare(`INSERT INTO pool_kv (key, value) VALUES (?1, '1') ON CONFLICT (key) DO UPDATE SET value = CAST(CAST(value AS INTEGER) + 1 AS TEXT) RETURNING value`)
    .bind(`stem_bf_fail:${publishId}`)
    .first<{ value: string }>();
  if (Number(r?.value ?? 0) >= STEM_BACKFILL_FAILS)
    await c.db.batch([
      c.db.prepare(`INSERT OR IGNORE INTO pool_kv (key, value) VALUES (?1, ?2)`).bind(`stem_bf_skip:${publishId}`, String(c.now())),
      c.db
        .prepare(`INSERT OR IGNORE INTO admin_flags (id, kind, detail, dedup, at) VALUES (?1, 'cron_failure', ?2, ?3, ?4)`)
        .bind(newId(), JSON.stringify({ stage: "stem_backfill", publish_id: publishId, why: "body unreadable; skipped" }), `stem_bf_skip:${publishId}`, c.now()),
    ]);
}
export async function stemBackfillAuthor(c: Ctx, author: string): Promise<number> {
  const cand = await c.db
    .prepare(
      `SELECT p.publish_id, p.hide_key, p.full_key, p.full_bytes FROM item_publishes p
        WHERE p.author_id = ?1 AND p.status IN ('current', 'previous') AND p.full_key IS NOT NULL
          AND NOT EXISTS (SELECT 1 FROM users du WHERE du.id = ?1 AND du.deleted_at IS NOT NULL)
          AND NOT EXISTS (SELECT 1 FROM pool_kv sk WHERE sk.key = 'stem_bf_skip:' || p.publish_id)
          AND EXISTS (SELECT 1 FROM item_questions q WHERE q.publish_id = p.publish_id AND q.tok_sk IS NOT NULL
                       AND NOT EXISTS (SELECT 1 FROM author_stems s WHERE s.author_id = ?1 AND s.qhash = q.match_hash AND s.bk_expl_full IS NOT NULL))
        ORDER BY p.publish_id LIMIT ${STEM_BACKFILL_PUBLISHES}`,
    )
    .bind(author)
    .all<{ publish_id: string; hide_key: number; full_key: string; full_bytes: number }>();
  let done = 0;
  for (const p of cand.results) {
    const qs = (
      await c.db
        .prepare(`SELECT pos, match_hash, kbands, tok_sk, body_sk, bk_expl, bk_ans, seq FROM item_questions WHERE publish_id = ?1 AND tok_sk IS NOT NULL ORDER BY pos`)
        .bind(p.publish_id)
        .all<{ pos: number; match_hash: string; kbands: string | null; tok_sk: string; body_sk: string | null; bk_expl: string | null; bk_ans: string | null; seq: number }>()
    ).results;
    if (qs.length === 0) continue;
    const n = Math.max(...qs.map((q) => q.pos)) + 1;
    const expl: (string | null)[] = Array.from({ length: n }, () => null);
    if (p.hide_key === 1) {
      let release: (() => void) | null = null;
      try {
        release = reserveBytes(2 * (p.full_bytes ?? 0));
      } catch {
        continue; // no room in this isolate: the cron retries
      }
      try {
        const o = await c.r2.get(p.full_key);
        if (!o) {
          await stemBackfillFailed(c, p.publish_id); // unknown, never a credit
          continue;
        }
        let full: QuestionJson[];
        try {
          full = questionsOf(JSON.parse(await o.text()) as PayloadJson); // r2-object
        } catch {
          await stemBackfillFailed(c, p.publish_id);
          continue;
        }
        const keys = await Promise.all(full.map((q) => fieldKeys(q)));
        for (const q of qs) expl[q.pos] = keys[q.pos]?.expl ?? "";
      } finally {
        release();
      }
    } else {
      for (const q of qs) expl[q.pos] = q.bk_expl ?? "";
    }
    await c.db
      .prepare(
        // (Round 30: the question's own entry of ?3 is read by its position, `?3 ->> '$[pos]'`, the same
        // value json_each's row with that key gave; the cross join over every entry read the whole array
        // once per question and band, quadratic in the bank size.)
        `INSERT INTO author_stems (author_id, band, qhash, tok_sk, body_sk, bk_expl, bk_ans, bk_expl_full, first_seq)
         SELECT ?1, kb.value, q.match_hash, q.tok_sk, q.body_sk, COALESCE(q.bk_expl, ''), COALESCE(q.bk_ans, ''), ?3 ->> ('$[' || q.pos || ']'), q.seq
           FROM item_questions q CROSS JOIN json_each(COALESCE(q.kbands, '[]')) kb
          WHERE q.publish_id = ?2 AND q.tok_sk IS NOT NULL AND (?3 ->> ('$[' || q.pos || ']')) IS NOT NULL
            -- (round 26, security critic 34d: the SELECT above skipped an erased author, but the R2 body
            -- read can outlive an erasure that commits mid-flight; re-check in the INSERT itself, so the
            -- guard and the write are one statement.)
            AND NOT EXISTS (SELECT 1 FROM users du WHERE du.id = ?1 AND du.deleted_at IS NOT NULL)
         ON CONFLICT (author_id, band, qhash, bk_expl, bk_ans) DO UPDATE SET
           first_seq = MIN(first_seq, excluded.first_seq),
           bk_expl_full = COALESCE(author_stems.bk_expl_full, excluded.bk_expl_full)`,
      )
      .bind(author, p.publish_id, JSON.stringify(expl))
      .run();
    done++;
  }
  return done;
}
/**
 * Round 25 (B-2): cron sweep, a few authors a run (bounded; the next run continues). Authors with a
 * retained publish whose stem record lacks the full explanation key, or whose record is missing.
 */
export const STEM_BACKFILL_AUTHORS = 5;
export async function stemBackfill(c: Ctx): Promise<number> {
  const authors = await c.db
    .prepare(
      `SELECT DISTINCT p.author_id AS a FROM item_publishes p JOIN items i ON i.id = p.item_id AND i.purged_at IS NULL
        WHERE p.status IN ('current', 'previous') AND p.full_key IS NOT NULL
          -- (round 26, critic-moodle-33c B-3: never an erased account: erasure deleted its records;
          -- V4: never a publish whose body was unreadable three times: skipped, the admin told)
          AND NOT EXISTS (SELECT 1 FROM users du WHERE du.id = p.author_id AND du.deleted_at IS NOT NULL)
          AND NOT EXISTS (SELECT 1 FROM pool_kv sk WHERE sk.key = 'stem_bf_skip:' || p.publish_id)
          AND EXISTS (SELECT 1 FROM item_questions q WHERE q.publish_id = p.publish_id AND q.tok_sk IS NOT NULL
                       AND NOT EXISTS (SELECT 1 FROM author_stems s WHERE s.author_id = p.author_id AND s.qhash = q.match_hash AND s.bk_expl_full IS NOT NULL))
        LIMIT ${STEM_BACKFILL_AUTHORS}`,
    )
    .all<{ a: string }>();
  let n = 0;
  for (const x of authors.results) n += await stemBackfillAuthor(c, x.a);
  return n;
}
/**
 * Round 31 (staging D1 insights): the earlier-publication lookup (round 14 B3, in publishPrepare) in two
 * steps, so no near hash is followed more than once per chunk.
 *
 * EARLIER_FIRSTS_SQL (?1 the publisher, ?2 the chunk's distinct near hashes, ?3 / ?4 the pairs hash +
 * U+001F + her explanation key / answer key): for each hash, the questions it finds in the served sense
 * (the lookup's own column test, one column at a time, each by its index), on an item visible to others
 * by another author, and of those the FIRST in the lookup's order (seq, publish, lineage index, rowid):
 * overall, and among those with each asked-for explanation key, and each asked-for answer key.
 *
 * EARLIER_SQL then runs the lookup's own test on each position's candidates (`$.c`) only. Why that is the
 * same answer: a position's question counts when `seq` is below her first (F) or, for a source that lacks
 * what she added, below her looser first L (minFirst: L <= F; NULL passes). So the first question overall
 * is the answer if it passes; if it fails, every later one lacking her additions fails too (its seq is no
 * smaller, L <= F), and the answer is the first one sharing her added explanation or answer key, if it
 * passes. Both are among the candidates. Before, every position walked every question with any of its
 * hashes: two 990-question banks of one stem with different explanations read 26.5 million rows here.
 */
const EARLIER_MATCH: [string, string][] = [
  ["near_full", "p.hide_key = 0 AND "],
  ["near_bare_full", "p.hide_key = 0 AND "],
  ["near_stripped", ""],
  ["near_bare_stripped", ""],
  ["near_kf", ""],
];
export const EARLIER_FIRSTS_SQL = `WITH vals AS MATERIALIZED (SELECT DISTINCT value AS hv FROM json_each(?2)),
     cand AS MATERIALIZED (${EARLIER_MATCH.map(
       ([col, hide]) => `
       SELECT v.hv, iq.rowid AS rid, iq.seq, iq.publish_id, iq.lineage_seq, iq.bk_expl, iq.bk_ans
         FROM vals v CROSS JOIN item_questions iq INDEXED BY item_questions_${col} ON iq.${col} = v.hv
              CROSS JOIN item_publishes p ON p.publish_id = iq.publish_id CROSS JOIN items i ON i.public_publish_id = iq.publish_id CROSS JOIN users u ON u.id = i.author_id
        WHERE ${hide}i.author_id <> ?1 AND ${VISIBLE_TO_OTHERS}`,
     ).join("\n       UNION ALL")}),
     ranked AS MATERIALIZED (SELECT hv, rid, bk_expl, bk_ans,
                                    ROW_NUMBER() OVER (PARTITION BY hv ORDER BY seq, publish_id, lineage_seq, rid) AS r0,
                                    ROW_NUMBER() OVER (PARTITION BY hv, bk_expl ORDER BY seq, publish_id, lineage_seq, rid) AS re,
                                    ROW_NUMBER() OVER (PARTITION BY hv, bk_ans ORDER BY seq, publish_id, lineage_seq, rid) AS ra FROM cand)
SELECT hv, rid, bk_expl, bk_ans, r0 = 1 AS f0, re = 1 AS fe, ra = 1 AS fa FROM ranked
 WHERE r0 = 1 OR (re = 1 AND hv || char(31) || bk_expl IN (SELECT value FROM json_each(?3))) OR (ra = 1 AND hv || char(31) || bk_ans IN (SELECT value FROM json_each(?4)))`;
export const EARLIER_SQL = `SELECT CAST(j.value ->> '$.i' AS INTEGER) AS pos,
                  (SELECT json_object('item_id', i.id, 'qid', iq.qid, 'version', i.public_version, 'license', i.license, 'author_name', COALESCE(u.display_name, 'deleted teacher'))
                     FROM item_questions iq JOIN item_publishes p ON p.publish_id = iq.publish_id JOIN items i ON i.public_publish_id = iq.publish_id JOIN users u ON u.id = i.author_id
                    WHERE iq.rowid IN (SELECT value FROM json_each(j.value -> '$.c'))
                      AND ((p.hide_key = 0 AND (iq.near_full IN (SELECT value FROM json_each(j.value -> '$.n')) OR iq.near_bare_full IN (SELECT value FROM json_each(j.value -> '$.n'))))
                           OR iq.near_stripped IN (SELECT value FROM json_each(j.value -> '$.n')) OR iq.near_bare_stripped IN (SELECT value FROM json_each(j.value -> '$.n'))
                           OR iq.near_kf IN (SELECT value FROM json_each(j.value -> '$.n')))
                      AND i.author_id <> ?1 AND ${VISIBLE_TO_OTHERS}
                      -- (round 26 B-2: against a source that lacks her added explanation or answer, her earliest
                      -- own version without them counts. Critic 34c: PER FIELD, only what her later version
                      -- ADDED to her earlier one must be absent from the source; a field she did not add is
                      -- hers either way, and requiring the source to differ there wrongly kept the strict
                      -- first for Short/Blank, whose sample or answer is usually unchanged.)
                      AND COALESCE(iq.seq < CAST(CASE
                             WHEN ((j.value ->> '$.expl') IS NULL OR (j.value ->> '$.expl') = ''
                                    OR ((j.value ->> '$.lexpl') IS NOT NULL AND (j.value ->> '$.lexpl') <> '' AND (j.value ->> '$.lexpl') IS (j.value ->> '$.expl'))
                                    OR iq.bk_expl IS NULL OR iq.bk_expl IS NOT (j.value ->> '$.expl'))
                               AND ((j.value ->> '$.ans') IS NULL OR (j.value ->> '$.ans') = ''
                                    OR ((j.value ->> '$.lans') IS NOT NULL AND (j.value ->> '$.lans') <> '' AND (j.value ->> '$.lans') IS (j.value ->> '$.ans'))
                                    OR iq.bk_ans IS NULL OR iq.bk_ans IS NOT (j.value ->> '$.ans'))
                             THEN COALESCE(j.value ->> '$.loose', j.value ->> '$.first') ELSE j.value ->> '$.first' END AS INTEGER), 1)
                    ORDER BY iq.seq, iq.publish_id, iq.lineage_seq, iq.rowid LIMIT 1) AS m
             FROM json_each(?2) j`;
const US = "\u001f";
/** Each position's candidates (`c`): the first question of each of its hashes, and the first sharing its explanation or answer key. */
export async function earlierCandidates<T extends { n: string[]; expl: string | null; ans: string | null }>(c: Ctx, me: string, want: T[]): Promise<(T & { c: number[] })[]> {
  const vals = [...new Set(want.flatMap((x) => x.n))];
  if (vals.length === 0) return want.map((x) => ({ ...x, c: [] }));
  const pairs = (k: "expl" | "ans") => JSON.stringify([...new Set(want.flatMap((x) => (x[k] ? x.n.map((h) => h + US + x[k]) : [])))]);
  const r = await c.db
    .prepare(EARLIER_FIRSTS_SQL)
    .bind(me, JSON.stringify(vals), pairs("expl"), pairs("ans"))
    .all<{ hv: string; rid: number; bk_expl: string | null; bk_ans: string | null; f0: number; fe: number; fa: number }>();
  const first = new Map<string, number>();
  for (const x of r.results) {
    if (x.f0) first.set(x.hv, x.rid);
    if (x.fe && x.bk_expl !== null) first.set(`e${US}${x.hv}${US}${x.bk_expl}`, x.rid);
    if (x.fa && x.bk_ans !== null) first.set(`a${US}${x.hv}${US}${x.bk_ans}`, x.rid);
  }
  return want.map((x) => ({
    ...x,
    c: [...new Set(x.n.flatMap((h) => [first.get(h), x.expl ? first.get(`e${US}${h}${US}${x.expl}`) : undefined, x.ans ? first.get(`a${US}${h}${US}${x.ans}`) : undefined]))].filter((y): y is number => y !== undefined),
  }));
}
const OUTER_SQL = `         SELECT o.pos, o.key, o.kind, o.hidden, o.stem_only, i.id AS item_id, MIN(o.qid) AS qid,
                CASE WHEN MAX(s.kind = 'import') = 1 THEN 'import' ELSE 'view' END AS ev_kind,
                -- Round 20: the licences of the serves of publishes HOLDING this unit (never another
                -- version's), each ghost's frozen licence now, and the item's licence now when its public
                -- version has this very unit (listed or not: the offer was public; a void cuts it).
                json_group_array(json(s.licenses_json)) AS lics_nested, json_group_array(json(COALESCE(o.now_lic, '[]'))) AS now_lics,
                -- (round 22 V-a: every licence the item had at this unit since this account was first served it)
                (SELECT json_group_array(DISTINCT cl.license) FROM content_licences cl WHERE cl.item_id = i.id AND cl.ckey = 'u:' || o.key
                    AND (cl.until_at IS NULL OR cl.until_at >= (SELECT MIN(s4.first_at) FROM served s4 WHERE s4.reader_id = ?1 AND s4.item_id = i.id AND s4.void_at IS NULL))) AS hist_lics,
                (SELECT json_group_array(DISTINCT v) FROM (SELECT i.license AS v WHERE (EXISTS (SELECT 1 FROM item_questions cq WHERE cq.publish_id = i.public_publish_id AND (cq.bk_expl = o.key OR cq.bk_ans = o.key))
                                                                 OR EXISTS (SELECT 1 FROM item_part_keys cpk WHERE cpk.publish_id = i.public_publish_id AND cpk.key = o.key)
                                                                 OR EXISTS (SELECT 1 FROM item_blobs cb LEFT JOIN blobs cbx ON cbx.sha256 = cb.sha256 WHERE cb.publish_id = i.public_publish_id AND (cb.sha256 = o.key OR cbx.pixel_hash = o.key)))
                                                           -- (round 24: an item not offered publicly now grants nothing)
                                                           AND ${OFFERED("i")})) AS licenses_json,
                MIN(o.seq) AS seq, MAX(o.chain_json) AS chain, i.public_version AS version, i.license, COALESCE(u.display_name, 'deleted teacher') AS author_name,
                (SELECT MIN(ac.first_seq) FROM author_contents ac WHERE ac.author_id = ?1 AND ac.kind = 'unit' AND ac.hash = o.key) AS own_seq,
                (i.visibility = 'public' AND i.current_publish_id IS NOT NULL AND i.removed_at IS NULL AND NOT EXISTS (SELECT 1 FROM item_holds hh WHERE hh.item_id = i.id)) AS visible,
                (SELECT MIN(sp.committed_at) FROM item_publishes sp WHERE sp.item_id = i.id) AS src_at,
                (SELECT MIN(bu.first_at) FROM blob_uploads bu WHERE bu.uploader_id = ?1 AND (bu.sha256 = o.key OR bu.sha256 IN (SELECT bx.sha256 FROM blobs bx WHERE bx.pixel_hash = o.key))) AS my_upload_at`;

export function deletePublishStatements(db: D1Database, publishIds: string[]): D1PreparedStatement[] {
  if (publishIds.length === 0) return [];
  const ids = JSON.stringify(publishIds);
  const inIds = `IN (SELECT value FROM json_each(?1))`;
  return [
    // Committed ids stay reserved forever (a staged id that never committed may be retried).
    db
      .prepare(`INSERT OR IGNORE INTO retired_publish_ids (publish_id, author_id) SELECT publish_id, author_id FROM item_publishes WHERE publish_id ${inIds} AND status IN ('current', 'previous')`)
      .bind(ids),
    // Round 19 (critic24c C3): a publish anyone was served leaves a key-only ghost (hashes, credits,
    // licence; no text, no tokens), so what bound its readers still binds after it is gone.
    db
      .prepare(
        `INSERT OR IGNORE INTO ghost_publishes (publish_id, item_id, author_id, hide_key, public_version, license, at)
         SELECT p.publish_id, p.item_id, p.author_id, p.hide_key, p.public_version, p.license, COALESCE(p.committed_at, p.created_at) FROM item_publishes p
          WHERE p.publish_id ${inIds} AND EXISTS (SELECT 1 FROM served s WHERE s.publish_id = p.publish_id)`,
      )
      .bind(ids),
    db
      .prepare(
        `INSERT OR IGNORE INTO ghost_questions (publish_id, qid, pos, match_hash, full_content, stripped_content, keyfree_content, near_full, near_stripped, near_kf,
                                                near_bare_full, near_bare_stripped, chain_json, seq, stem_hash, bk_expl, bk_ans, tok_sk, body_sk, qkind, bk_norm, bk_stem, bk_full, now_licenses)
         SELECT iq.publish_id, iq.qid, iq.pos, iq.match_hash, iq.full_content, iq.stripped_content, iq.keyfree_content, iq.near_full, iq.near_stripped, iq.near_kf,
                iq.near_bare_full, iq.near_bare_stripped, iq.chain_json, iq.seq, iq.stem_hash, iq.bk_expl, iq.bk_ans, iq.tok_sk, iq.body_sk,
                -- (round 23 A2: what a takedown of the ghost can block, as a live plan would: keyed block keys only)
                iq.qkind, iq.bk_norm, iq.bk_stem, iq.bk_full,
                -- (round 20: the item's licence now, when its public version serves this very question;
                --  round 24: by its canonical served content, and only while the item is publicly offered)
                (SELECT json_array(i.license) FROM items i JOIN item_publishes cp ON cp.publish_id = i.public_publish_id JOIN item_questions cq ON cq.publish_id = cp.publish_id AND cq.qid = iq.qid
                  WHERE i.id = p.item_id AND (CASE WHEN cp.hide_key = 1 THEN cq.near_stripped ELSE cq.near_full END) = (CASE WHEN p.hide_key = 1 THEN iq.near_stripped ELSE iq.near_full END) AND ${OFFERED("i")})
           FROM item_questions iq JOIN item_publishes p ON p.publish_id = iq.publish_id WHERE iq.publish_id ${inIds} AND iq.publish_id IN (SELECT publish_id FROM ghost_publishes)`,
      )
      .bind(ids),
    db
      .prepare(
        `INSERT OR IGNORE INTO ghost_units (publish_id, pos, part, key, kind, seq, now_licenses)
         SELECT iq.publish_id, iq.pos, -1, k.value, 'text', COALESCE(json_extract(iq.unit_seqs, '$."' || k.value || '"'), iq.seq), ${NOW_UNIT("(SELECT gp.item_id FROM ghost_publishes gp WHERE gp.publish_id = iq.publish_id)", "k.value")} FROM item_questions iq, json_each(json_array(iq.bk_expl, iq.bk_ans)) k
          WHERE iq.publish_id ${inIds} AND iq.publish_id IN (SELECT publish_id FROM ghost_publishes) AND k.value IS NOT NULL`,
      )
      .bind(ids),
    db
      .prepare(
        `INSERT OR IGNORE INTO ghost_units (publish_id, pos, part, key, kind, seq, now_licenses)
         SELECT pk.publish_id, pk.pos, pk.part, pk.key, CASE WHEN pk.key LIKE 'u:%' THEN 'part' ELSE 'text' END,
                (SELECT uq.seq FROM item_questions uq WHERE uq.publish_id = pk.publish_id AND uq.pos = pk.pos),
                ${NOW_UNIT("(SELECT gp.item_id FROM ghost_publishes gp WHERE gp.publish_id = pk.publish_id)", "pk.key")} FROM item_part_keys pk
          WHERE pk.publish_id ${inIds} AND pk.publish_id IN (SELECT publish_id FROM ghost_publishes) AND (pk.key LIKE 'u:%' OR pk.key LIKE 'e:%' OR pk.key LIKE 'a:%')`,
      )
      .bind(ids),
    db
      .prepare(
        `INSERT OR IGNORE INTO ghost_units (publish_id, pos, part, key, kind, seq, now_licenses)
         SELECT ib.publish_id, -1, -1, k.value, 'image', ib.seq, ${NOW_UNIT("p.item_id", "k.value")} FROM item_blobs ib JOIN item_publishes p ON p.publish_id = ib.publish_id LEFT JOIN blobs b ON b.sha256 = ib.sha256,
                json_each(json_array(ib.sha256, b.pixel_hash)) k
          WHERE ib.publish_id ${inIds} AND ib.publish_id IN (SELECT publish_id FROM ghost_publishes) AND ${SERVED_FORM("ib", "p")} AND k.value IS NOT NULL`,
      )
      .bind(ids),
    // Round 21 (critic27a V1): every ghost of these items also takes the item's licence NOW, where its
    // public version still has the very question or unit (a later relicence is a grant too).
    db
      .prepare(
        `UPDATE ghost_questions SET now_licenses = (SELECT json_group_array(DISTINCT v) FROM (
            SELECT nl.value AS v FROM json_each(COALESCE(ghost_questions.now_licenses, '[]')) nl
            UNION SELECT i.license FROM ghost_publishes gp JOIN items i ON i.id = gp.item_id JOIN item_publishes cp ON cp.publish_id = i.public_publish_id
                    JOIN item_questions cq ON cq.publish_id = cp.publish_id AND cq.qid = ghost_questions.qid
             WHERE gp.publish_id = ghost_questions.publish_id
               AND (CASE WHEN cp.hide_key = 1 THEN cq.near_stripped ELSE cq.near_full END) = (CASE WHEN gp.hide_key = 1 THEN ghost_questions.near_stripped ELSE ghost_questions.near_full END)
               AND ${OFFERED("i")}))
          WHERE publish_id IN (SELECT gp.publish_id FROM ghost_publishes gp WHERE gp.item_id IN (SELECT item_id FROM item_publishes WHERE publish_id ${inIds}))`,
      )
      .bind(ids),
    db
      .prepare(
        `UPDATE ghost_units SET now_licenses = (SELECT json_group_array(DISTINCT v) FROM (
            SELECT nl.value AS v FROM json_each(COALESCE(ghost_units.now_licenses, '[]')) nl
            UNION SELECT nn.value FROM json_each(COALESCE(${NOW_UNIT("(SELECT gp.item_id FROM ghost_publishes gp WHERE gp.publish_id = ghost_units.publish_id)", "ghost_units.key")}, '[]')) nn))
          WHERE publish_id IN (SELECT gp.publish_id FROM ghost_publishes gp WHERE gp.item_id IN (SELECT item_id FROM item_publishes WHERE publish_id ${inIds}))`,
      )
      .bind(ids),
    // Round 23 (critic30b A2): each part's block keys (keyed), so a ghost takedown's plan has its parts.
    db
      .prepare(
        `INSERT OR IGNORE INTO ghost_block_keys (publish_id, pos, part, key) SELECT pk.publish_id, pk.pos, pk.part, pk.key FROM item_part_keys pk
          WHERE pk.publish_id ${inIds} AND pk.publish_id IN (SELECT publish_id FROM ghost_publishes)`,
      )
      .bind(ids),
    // Round 22 (critic29a B2): a ghost is found by its KEYED bands only (HMAC under 'sketch-v1').
    db
      .prepare(
        `INSERT OR IGNORE INTO ghost_bands (band, item_id, qid)
         SELECT kb.value, p.item_id, iq.qid FROM item_questions iq JOIN item_publishes p ON p.publish_id = iq.publish_id, json_each(COALESCE(iq.kbands, '[]')) kb
          WHERE iq.publish_id ${inIds} AND iq.publish_id IN (SELECT publish_id FROM ghost_publishes)`,
      )
      .bind(ids),
    db.prepare(`DELETE FROM item_questions WHERE publish_id ${inIds}`).bind(ids),
    db.prepare(`DELETE FROM item_part_keys WHERE publish_id ${inIds}`).bind(ids),
    // Round 17 (critic22e A3): the public and own stem bands of those items that no retained publish
    // still has go with them (an account's served bands are evidence and stay).
    db
      .prepare(
        `DELETE FROM stem_bands WHERE (user_id = 'pub' OR user_id LIKE 'own:%') AND item_id IN (SELECT item_id FROM item_publishes WHERE publish_id ${inIds})
           AND NOT EXISTS (SELECT 1 FROM item_publishes rp JOIN item_questions rq ON rq.publish_id = rp.publish_id, json_each(COALESCE(rq.stem_bands, '[]')) b
                            WHERE rp.item_id = stem_bands.item_id AND rp.status IN ('current', 'previous') AND rq.qid = stem_bands.qid AND b.value = stem_bands.band)
           -- (round 22 B2: a ghost keeps no unkeyed band: its keyed bands, below, find it)`,
      )
      .bind(ids),
    db.prepare(`DELETE FROM item_tags WHERE publish_id ${inIds}`).bind(ids),
    db.prepare(`DELETE FROM served_words WHERE publish_id ${inIds}`).bind(ids),
    db.prepare(`DELETE FROM item_standards WHERE publish_id ${inIds}`).bind(ids),
    db.prepare(`DELETE FROM item_blobs WHERE publish_id ${inIds}`).bind(ids),
    // (round 23: an attestation stays while its publish or its ghost does)
    db.prepare(`DELETE FROM attestations WHERE publish_id ${inIds} AND publish_id NOT IN (SELECT publish_id FROM ghost_publishes)`).bind(ids),
    db.prepare(`DELETE FROM item_publishes WHERE publish_id ${inIds}`).bind(ids),
  ];
}

/** qids first introduced by a publish that never committed, of that publish's own item, used nowhere. */
export function discardQidsStatement(db: D1Database, publishId: string, itemId: string): D1PreparedStatement {
  return db
    .prepare(`DELETE FROM question_ids WHERE first_publish_id = ?1 AND item_id = ?2 AND NOT EXISTS (SELECT 1 FROM item_questions iq WHERE iq.qid = question_ids.qid)`)
    .bind(publishId, itemId);
}

/** Removes a staged publish that will never commit: its rows, its new qids and its bodies. */
async function discardStaged(c: Ctx, row: { publish_id: string; item_id: string; full_key: string; stripped_key: string | null; created_at: number }): Promise<boolean> {
  // Claiming the row first (only a never-committed one) makes two concurrent discards safe.
  const del = await c.db
    .prepare(`DELETE FROM item_publishes WHERE publish_id = ?1 AND status IN ('staging', 'staged') AND created_at = ?2`)
    .bind(row.publish_id, row.created_at)
    .run();
  if ((del.meta.changes ?? 0) !== 1) return false;
  await c.db.batch([
    ...deletePublishStatements(c.db, [row.publish_id]),
    // qids first introduced by a publish that never committed belong to nothing.
    discardQidsStatement(c.db, row.publish_id, row.item_id),
  ]);
  await c.r2.delete([row.full_key, ...(row.stripped_key ? [row.stripped_key] : [])]);
  return true;
}

interface PubRow {
  publish_id: string;
  item_id: string;
  author_id: string;
  status: string;
  request_sha256: string;
  result_json: string | null;
  full_key: string;
  stripped_key: string | null;
  created_at: number;
}

/**
 * Refused publishes are counted per day (each costs a full validation): past the allowance the
 * account gets 429 before any work is done. Conflicts, retries and rate limits do not count.
 */
export async function publish(c: Ctx, req: Request, p: PoolPrincipal, pathItemId: string | null, dry = false): Promise<Response> {
  const me = p.userId;
  // Round 17: a dry run (POST /items/preview) counts against its own daily allowance only (never the
  // publish allowance, the refusal counter or the held cap) and stores nothing.
  if (dry) await requireDaily(c, me, "previews");
  else await missGuard(c, me, "publish_refusals");
  // The day's publish allowance is checked BEFORE any work (cheap); a request refused for it still
  // counts as a refused publish (it is not refunded), so retries cannot buy free validations.
  const used = await c.db
    .prepare(`SELECT count FROM usage_daily WHERE user_id = ?1 AND day = ?2 AND metric = 'publishes'`)
    .bind(me, dayOf(c.now()))
    .first<{ count: number }>();
  if ((used?.count ?? 0) >= (await limitOf(c, me, "publishes"))) {
    if (dry) throw new PoolError(429, "daily_limit", { metric: "publishes" }, { "Retry-After": "3600" });
    await countMiss(c, me, "publish_refusals");
    await capAlert(c, me, "publishes");
    throw new PoolError(429, "daily_limit", { metric: "publishes" }, { "Retry-After": "3600" });
  }
  // Reserved BEFORE any work and refunded only on a definite non-refusal: a killed isolate, a wasm
  // trap or any unexpected exception still counts (critic D1d).
  if (!dry) await countMiss(c, me, "publish_refusals");
  const refund = async () => (dry ? undefined : refundMiss(c, me, "publish_refusals"));
  // One publish at a time per account, across isolates (critic D1a).
  const lock = await acquireAccountLock(c, me);
  if (!lock) {
    await refund();
    await throwIfErased(c, me); // (round 27: the guarded lock insert wrote nothing because the account is erased)
    throw new PoolError(409, "publish_in_progress", { message: "Another publish from this account is still running." }, { "Retry-After": "5" });
  }
  // The heavy slot, and (round 29) the bytes the heavy phase reserved (its lookups borrow the unused part).
  const heavy = { release: () => {}, bytes: 0 };
  const budget: (() => void)[] = [];
  const refresh = () => refreshAccountLock(c, me, lock);
  try {
    // The body is read in full BEFORE any heavy slot is taken (round 5 E7): its length must be
    // declared (it is reserved against the isolate's byte budget first), and it must arrive within
    // the deadline, so a hung upload pins nothing but its own reservation for a bounded time.
    if (!isPublishType(req)) throw new PoolError(415, "unsupported_media_type");
    const maxPayload = c.deps.maxPublishBytes ?? MAX_PAYLOAD;
    // The one body reader: length required and exact, reserved first, deadline, per-account cap.
    const body = await readBody(req, { max: maxPayload + MAX_META + 1, key: `u:${me}`, deadlineMs: c.deps.bodyDeadlineMs ?? BODY_DEADLINE_MS });
    budget.push(body.release);
    const declared = body.bytes.length;
    const read = parsePublish(body.bytes, maxPayload, MAX_META);
    await refresh();
    const res = await publishOnce(c, p, pathItemId, heavy, read, dry, async () => {
      // The heavy phase: one per isolate, and its measured peak reserved against the byte budget.
      heavy.release = await acquireHeavy(c.deps.heavyQueueMs ?? HEAVY_QUEUE_WAIT_MS, refresh, c.deps.heavySlots);
      // Round 16 (critic21d 1): a payload of many tiny values costs more to parse than its bytes say:
      // reserved by its value count when that is more (at most share-core's value limit).
      const scan = read.payload ? scanJson(read.payload) : null;
      const want = Math.max(declared * (PUBLISH_COST_PER_BYTE - 1), (scan?.values ?? 0) * PARSE_BYTES_PER_VALUE * 2);
      budget.push(reserveBytes(want));
      heavy.bytes = want;
      await refresh();
    }, refresh);
    await refund();
    return res;
  } catch (e) {
    if (e instanceof PoolError && (e.status === 409 || e.status === 503)) await refund();
    throw e;
  } finally {
    heavy.release();
    for (const r of budget) r();
    await releaseAccountLock(c, me, lock);
  }
}

/**
 * Round 29: an upper bound of what a publish still holds once its payload trees are dropped, during
 * its evidence and fuzzy lookups (README "Round 29"):
 * - the bodies it will store (full, stripped, a reused stripped body): exact bytes. The request body
 *   keeps its own reservation (readBody);
 * - the meta's parsed tree, costed per byte as the heavy phase costs it;
 * - per question: its hashes, keys and the D1 rows read for it (author context, the previous
 *   publish's row in a republish);
 * - per stem / choice / part-stem word (stemTokens, bodyTokens: at most 64 a text): the word, its
 *   keyed sketch and, in a republish, its copies in the previous rows; plus its characters (two-byte
 *   strings included);
 * - per Multipart part: its keys and bands.
 * Fitted on node measurements (--expose-gc; node has no pointer compression, so workerd holds less)
 * of fresh publishes and served-body-keeping republishes of eleven bank shapes (Matching 990 x 50,
 * Mc 1,000 x 26 with every word distinct, 440 x 64 long words, 1,000 tiny Tf, Short, 180 x 50
 * Multipart parts): every measured holding is at least 20% below the bound.
 */
export const POST_HEAVY_BYTES_PER_QUESTION = 12 * 1024;
export const POST_HEAVY_BYTES_PER_WORD = 224;
export const POST_HEAVY_BYTES_PER_WORD_CHAR = 8;
export const POST_HEAVY_BYTES_PER_PART = 2 * 1024;
export function postHeavyBytes(o: { bodies: number; metaBytes: number; questions: number; parts: number; words: number; wordChars: number }): number {
  return (
    o.bodies +
    o.metaBytes * PUBLISH_COST_PER_BYTE +
    o.questions * POST_HEAVY_BYTES_PER_QUESTION +
    o.words * POST_HEAVY_BYTES_PER_WORD +
    o.wordChars * POST_HEAVY_BYTES_PER_WORD_CHAR +
    o.parts * POST_HEAVY_BYTES_PER_PART
  );
}

type Reused = { bytes: Uint8Array; hashes: string[]; contents: string[]; near: (string | null)[]; bare: (string | null)[]; servedOrders: unknown[] };
/**
 * A hidden republish whose served form is unchanged (HOLD-1): the previous stripped body, its rows'
 * served facts and the author's served orders remapped onto it; null when it cannot be kept. (Round 29:
 * its own function, so the previous body's text and parsed tree are gone once it returns, before the
 * publish's evidence and fuzzy lookups.)
 */
async function reuseServed(c: Ctx, baseOf: string, qids: string[], st: Stripped, orderAcks: Pos[]): Promise<Reused | null> {
  const prevQ = await c.db
    .prepare(`SELECT qid, stripped_sha256, stripped_content, near_stripped, near_bare_stripped FROM item_questions WHERE publish_id = ?1 ORDER BY pos`)
    .bind(baseOf)
    .all<{ qid: string; stripped_sha256: string; stripped_content: string; near_stripped: string | null; near_bare_stripped: string | null }>();
  const prevKey = (await c.db.prepare(`SELECT stripped_key FROM item_publishes WHERE publish_id = ?1`).bind(baseOf).first<{ stripped_key: string | null }>())?.stripped_key;
  if (!prevKey || prevQ.results.length !== qids.length || !prevQ.results.every((q, k) => q.qid === qids[k])) return null;
  const o = await c.r2.get(prevKey);
  if (!o) return null;
  const text = await o.text(); // r2-object: the previous stripped body
  const prevBody = JSON.parse(text) as PayloadJson;
  // The author's key review must describe the body served (round 14: remapped onto it).
  const so = sameServed(prevBody, st.payload, st.servedOrders, [...orderAcks, ...(st.orderReasons as unknown[])]) ? remapServedOrders(prevBody, st.payload, st.servedOrders) : null;
  if (!so) return null;
  return { bytes: enc.encode(text), hashes: prevQ.results.map((q) => q.stripped_sha256), contents: prevQ.results.map((q) => q.stripped_content), near: prevQ.results.map((q) => q.near_stripped), bare: prevQ.results.map((q) => q.near_bare_stripped), servedOrders: so };
}

/** How long a publish body may take to arrive. */
export const BODY_DEADLINE_MS = 15_000;

/** Keeps the account lock alive while this request is (round 5 E7): refreshed at every phase. */
export async function refreshAccountLock(c: Ctx, me: string, token: string): Promise<void> {
  await c.db.prepare(`UPDATE pool_locks SET until = ?3 WHERE user_id = ?1 AND token = ?2`).bind(me, token, c.now() + ACCOUNT_LOCK_MS).run();
}

// ---- memory: one heavy publish per isolate ----------------------------------------------------------

/** Concurrent publishes that may parse, validate and strip at once in one isolate (critic D1a). */
export const HEAVY_CONCURRENCY = 1;
/** Publishes allowed to wait for it; beyond that, 503 with Retry-After. */
export const HEAVY_QUEUE_MAX = 4;
/** The isolate's heavy slot (round 37: a test gives a second simulated isolate its own, PoolDeps.heavySlots). */
const ISOLATE_HEAVY: HeavySlots = { active: 0, queue: [] };

/** How long a publish may wait for the heavy slot (then 503 busy). */
export const HEAVY_QUEUE_WAIT_MS = 20_000;

/**
 * Waits for the isolate's heavy-work slot, at most `waitMs` (503 busy after it), calling `beat` every
 * few seconds meanwhile so the account lock lives exactly as long as the request; returns the
 * release (idempotent).
 */
export async function acquireHeavy(waitMs = HEAVY_QUEUE_WAIT_MS, beat: () => Promise<void> = async () => {}, slots: HeavySlots = ISOLATE_HEAVY): Promise<() => void> {
  const heavyQueue = slots.queue;
  if (slots.active >= HEAVY_CONCURRENCY) {
    if (heavyQueue.length >= HEAVY_QUEUE_MAX) throw new PoolError(503, "busy", { message: "The server is busy with large publishes; try again shortly." }, { "Retry-After": "10" });
    let wake!: () => void;
    const turn = new Promise<void>((r) => (wake = r));
    heavyQueue.push(wake);
    const hb = setInterval(() => void beat().catch(() => {}), Math.max(1000, Math.floor(ACCOUNT_LOCK_MS / 3)));
    (hb as { unref?: () => void }).unref?.();
    let timer: ReturnType<typeof setTimeout> | undefined;
    try {
      await Promise.race([
        turn,
        new Promise<never>((_, rej) => {
          timer = setTimeout(() => rej(new PoolError(503, "busy", { message: "The server is busy with large publishes; try again shortly." }, { "Retry-After": "10" })), waitMs);
        }),
      ]);
    } catch (e) {
      const k = heavyQueue.indexOf(wake);
      if (k >= 0) heavyQueue.splice(k, 1);
      // Woken in the same instant as the timeout: pass the free slot on.
      else if (slots.active < HEAVY_CONCURRENCY) heavyQueue.shift()?.();
      throw e;
    } finally {
      clearInterval(hb);
      if (timer) clearTimeout(timer);
    }
  }
  slots.active++;
  let done = false;
  return () => {
    if (done) return;
    done = true;
    slots.active--;
    heavyQueue.shift()?.();
  };
}

export const ACCOUNT_LOCK_MS = 60_000;

export async function acquireAccountLock(c: Ctx, me: string): Promise<string | null> {
  const token = newId();
  const now = c.now();
  const r = await c.db
    .prepare(
      `INSERT INTO pool_locks (user_id, token, until) SELECT ?1, ?2, ?3 WHERE ${ALIVE_SQL("?1")}
       ON CONFLICT (user_id) DO UPDATE SET token = excluded.token, until = excluded.until WHERE pool_locks.until < ?4
       RETURNING token`,
    )
    .bind(me, token, now + ACCOUNT_LOCK_MS, now)
    .first<{ token: string }>();
  return r?.token === token ? token : null;
}

export async function releaseAccountLock(c: Ctx, me: string, token: string): Promise<void> {
  await c.db.prepare(`DELETE FROM pool_locks WHERE user_id = ?1 AND token = ?2`).bind(me, token).run();
}

async function publishOnce(
  c: Ctx,
  p: PoolPrincipal,
  pathItemId: string | null,
  heavy: { release: () => void; bytes: number },
  read: { raw: Uint8Array; meta: Record<string, unknown>; payload: Uint8Array },
  dry: boolean,
  enterHeavy: () => Promise<void>,
  refresh: () => Promise<void>,
): Promise<Response> {
  const isNew = pathItemId === null;
  // Round 9: at most HELD_PUBLISHES_PER_DAY of an account's publishes a day may end up held by a
  // content block (any kind alike, so the cap says nothing about which): probing whether texts are
  // confirmed student data is bounded to that many guesses a day.
  // Round 11 P3: EVERY publish that ends up held (a new hold or a republish of an item still held,
  // whatever the hold) counts, so a held item cannot be re-published as a probe without limit.
  const heldToday = await c.db
    .prepare(`SELECT count AS n FROM usage_daily WHERE user_id = ?1 AND day = ?2 AND metric = 'held_publishes'`)
    .bind(p.userId, dayOf(c.now()))
    .first<{ n: number }>();
  // Round 14 (B1): never a refusal. Past the cap the account's publishes that match a block are held
  // (neutrally, source probe_cap) and the admin is alerted; unrelated publishes always go through.
  const overCap = (heldToday?.n ?? 0) >= HELD_PUBLISHES_PER_DAY;
  if (overCap) await capAlert(c, p.userId, "held_publishes");
  if (!isNew) await missGuard(c, p.userId, "item_miss");
  if (!isNew && !ID_RE.test(pathItemId)) {
    await countMiss(c, p.userId, "item_miss");
    throw new PoolError(404, "not_found");
  }
  await assertNotMigrating(c);
  await enterHeavy();
  // (Round 29: the meta's parsed tree lives as long as the publish: see postHeavyBytes.)
  const metaBytes = read.raw.length - read.payload.length;
  const rawMeta = read.meta;
  let payload: Uint8Array | null = read.payload;
  const m = readMeta(rawMeta, isNew);
  // (Round 32: a circle independence statement, checked with the circle material below.)
  const circleClaim = readCircleClaim(rawMeta);
  // Round 23 (owner): a publish or republish needs the teacher's affirmation of the current terms
  // (recorded with the publish; not verified). A preview does not.
  if (!dry && !(m.attest?.ownWork === true && m.attest.termsVersion === POOL_TERMS_VERSION))
    throw new PoolError(422, "attestation_required", { terms_version: POOL_TERMS_VERSION, statement: ATTEST_STATEMENT });
  const requestSha = await sha256Hex(read.raw);
  (read as { raw: Uint8Array | null }).raw = null;
  const me = p.userId;

  // ---- idempotency ---------------------------------------------------------------------------------
  const prior = await c.db
    .prepare(`SELECT publish_id, item_id, author_id, status, request_sha256, result_json, full_key, stripped_key, created_at FROM item_publishes WHERE publish_id = ?1`)
    .bind(m.publishId)
    .first<PubRow>();
  const retired = await c.db.prepare(`SELECT 1 AS x FROM retired_publish_ids WHERE publish_id = ?1`).bind(m.publishId).first();
  if (retired) throw new PoolError(409, "publish_id_conflict");
  if (prior) {
    // Someone else's id, or the same id with a different body: never resumed, never revealed.
    if (prior.author_id !== me || prior.request_sha256 !== requestSha || (!isNew && prior.item_id !== pathItemId)) throw new PoolError(409, "publish_id_conflict");
    if ((prior.status === "current" || prior.status === "previous") && prior.result_json) return json(200, { ...(JSON.parse(prior.result_json) as object), replayed: true });
    if (prior.created_at > c.now() - IN_FLIGHT_MS) throw new PoolError(409, "publish_in_progress", {}, { "Retry-After": "5" });
    // An abandoned attempt: start again from scratch with the same publish_id.
    if (!(await discardStaged(c, prior))) throw new PoolError(409, "publish_in_progress", {}, { "Retry-After": "5" });
  }

  // ---- the item ------------------------------------------------------------------------------------
  const now = c.now();
  let itemId: string;
  let version: number;
  let visibility: "public" | "unlisted";
  let itemKind: string | null = null;
  let baseOf: string | null = null;
  let prevPublic = { version: 0, at: null as number | null };
  let prevHidden = false;
  let prevLicense: string | null = null;
  if (isNew) {
    itemId = newId();
    version = 1;
    visibility = m.visibility!;
  } else {
    const it = await c.db
      .prepare(`SELECT id, kind, visibility, current_publish_id, version, removed_at, purged_at, public_version, public_at, hide_key, license FROM items WHERE id = ?1 AND author_id = ?2`)
      .bind(pathItemId, me)
      .first<{ id: string; kind: string; visibility: "public" | "unlisted"; current_publish_id: string | null; version: number; removed_at: number | null; purged_at: number | null; public_version: number; public_at: number | null; hide_key: number; license: string }>();
    if (!it) {
      await countMiss(c, me, "item_miss");
      throw new PoolError(404, "not_found");
    }
    if (it.removed_at !== null)
      throw new PoolError(409, "item_removed", it.purged_at !== null ? { purged: true, message: "This item was deleted for good. Publish it again as a new item." } : { message: "This item is removed. Undo the deletion first, or publish it as a new item." });
    if (it.current_publish_id !== m.base) throw new PoolError(409, "conflict", { current_version: it.version, current_publish_id: it.current_publish_id });
    if (it.visibility === "public" && m.license === "circle-only") throw new PoolError(422, "license_not_public");
    itemId = it.id;
    version = it.version + 1;
    visibility = it.visibility;
    itemKind = it.kind;
    baseOf = it.current_publish_id;
    prevPublic = { version: it.public_version, at: it.public_at };
    prevHidden = it.hide_key === 1;
    prevLicense = it.license;
  }

  // ---- validate, rewrite, validate again -----------------------------------------------------------
  const meta = c.core.validateMeta(m.meta);
  if (isFail(meta)) throw invalid(meta, 422);
  // Display text is the author's own (round 9): share-core's metadata cleaner folds NFKC (x² -> x2,
  // ½ -> 1⁄2); the stored title, subject, course, grade and tags keep the author's NFC text whenever
  // share-core's checks found nothing else to change; the NFKC fold is only the search column's.
  Object.assign(meta, authorsMeta(m.meta, meta));
  // share-core round 7: ONE share-core call validates, strips and hashes the publish (below). What
  // the server must rewrite first (ids, dates, its own provenance, image references) is read from
  // the author's JSON here, type-checked field by field; anything malformed goes straight to
  // share-core, whose refusal is the answer. Only share-core's validated output is ever stored.
  const refuseRaw = (): never => {
    const f = c.core.publishPrepare(payload!, { hideKey: m.hideKey, labels: m.labels, orderAcks: m.orderAcks, imageOk: m.imageOk, ackFormatting: m.ackFormatting });
    if (isFail(f)) throw invalid(f);
    throw new PoolError(422, "invalid_payload", { errors: [{ code: "invalid", path: "" }] });
  };
  let raw: PayloadJson | null = null;
  // Round 16 (critic21d 1): share-core's structural limits on the RAW bytes first (depth, values,
  // objects, duplicate keys), so the Worker's own parse never sees a different document than
  // share-core validates (JSON.parse keeps the last duplicate key) and never recurses without bound.
  if (!scanJson(payload!).ok) refuseRaw();
  try {
    raw = JSON.parse(new (TextDecoder as unknown as new (l: string, o: { fatal: boolean }) => TextDecoder)("utf-8", { fatal: true }).decode(payload!)) as PayloadJson;
  } catch {
    refuseRaw();
  }
  const isObj = (x: unknown): x is Record<string, unknown> => typeof x === "object" && x !== null && !Array.isArray(x);
  const shaped =
    isObj(raw) &&
    typeof raw.schema_version === "number" &&
    (isObj(raw.bank) ? Array.isArray(raw.bank.questions) && raw.bank.questions.length > 0 && raw.bank.questions.every(isObj) && raw.question === undefined : isObj(raw.question) && raw.bank === undefined);
  if (!shaped) refuseRaw();
  const kind = raw!.bank ? "bank" : "question";
  // A republish must be schema v2: v1 cannot carry `published`, so its questions would lose their
  // identity (and the hidden-change and provenance rules with it).
  if (!isNew && raw!.schema_version < 2)
    throw new PoolError(422, "schema_upgrade_required", { message: "Republishing needs schema version 2 (update verSHion)." });
  if (itemKind !== null && itemKind !== kind) throw new PoolError(422, "kind_mismatch");
  const qs = questionsOf(raw!);
  const n = qs.length;
  if (n === 0) throw new PoolError(422, "invalid_payload", { errors: [{ code: "no_questions", path: "bank.questions" }] });


  // qids: a sent qid is kept only if it already belongs to this item and appears once.
  const sent = qs.map((q) => (typeof q.published?.qid === "string" ? q.published.qid : null));
  const owned = new Set<string>();
  if (!isNew && sent.some((x) => x !== null)) {
    const rows = await c.db
      .prepare(`SELECT qid FROM question_ids WHERE item_id = ?1 AND qid IN (SELECT value FROM json_each(?2))`)
      .bind(itemId, JSON.stringify([...new Set(sent.filter((x): x is string => x !== null))]))
      .all<{ qid: string }>();
    for (const r of rows.results) owned.add(r.qid);
  }
  const counts = new Map<string, number>();
  for (const s of sent) if (s) counts.set(s, (counts.get(s) ?? 0) + 1);
  const qids = sent.map((s) => (s && owned.has(s) && counts.get(s) === 1 ? s : newId()));
  const newQids = qids.filter((q, i) => q !== sent[i] || !owned.has(q));

  // Images: every referenced blob must be verified and the author's (pending, or in one of their publishes).
  const imgOf = (q: QuestionJson) => (q.image && typeof q.image === "object" && typeof q.image.file === "string" ? MEDIA_RE.exec(q.image.file)?.[1] ?? null : null);
  const shas = [...new Set(qs.map(imgOf).filter((x): x is string => x !== null))];
  const blobInfo = new Map<string, { content_type: keyof typeof EXT; width: number; height: number; privacy_purged_at: number | null }>();
  if (shas.length > 0) {
    const rows = await c.db
      .prepare(
        `SELECT b.sha256, b.content_type, b.width, b.height, b.privacy_purged_at FROM blobs b
          WHERE b.sha256 IN (SELECT value FROM json_each(?1))
            AND (EXISTS (SELECT 1 FROM pending_blobs pb WHERE pb.sha256 = b.sha256 AND pb.uploader_id = ?2 AND pb.until >= ?3)
              OR EXISTS (SELECT 1 FROM item_blobs ib JOIN item_publishes ip ON ip.publish_id = ib.publish_id WHERE ib.sha256 = b.sha256 AND ip.author_id = ?2))`,
      )
      .bind(JSON.stringify(shas), me, now)
      .all<{ sha256: string; content_type: keyof typeof EXT; width: number; height: number; privacy_purged_at: number | null }>();
    for (const r of rows.results) blobInfo.set(r.sha256, r);
    const missing = qs.map((q, i) => [i, imgOf(q)] as const).filter(([, s]) => s !== null && !blobInfo.has(s!)).map(([i]) => i);
    if (missing.length > 0) throw new PoolError(422, "blob_missing", { positions: missing.slice(0, 100) });
    // Validation extends the author's hold on every referenced blob (spec "Publish order" step 4),
    // so the GC cannot collect one between this check and the commit. Already charged: no quota change.
    if (!dry)
      await c.db
        .prepare(
          `INSERT INTO pending_blobs (sha256, uploader_id, size, until) SELECT sha256, ?2, size, ?3 FROM blobs WHERE sha256 IN (SELECT value FROM json_each(?1)) AND ${ALIVE_SQL("?2")}
           ON CONFLICT (sha256, uploader_id) DO UPDATE SET until = MAX(until, excluded.until)`,
        )
        .bind(JSON.stringify(shas), me, now + DAY_MS)
        .run();
    // A blob GC could have raced an upload: the bytes must really be in R2. An image whose privacy
    // block's grace window ended is decided later, by served-ness alone (never by whether its bytes
    // happen to be kept for someone's unserved use: that must not be observable).
    // Round 17 (critic22e A1): a privacy-blocked image's bytes are never required (they may be gone,
    // or never stored for this upload): such a publish is held at commit, exactly like a control
    // whose bytes are there would be published, never refused here.
    const blockedImg = new Set(
      (await c.db.prepare(`SELECT hash FROM privacy_blocks WHERE kind = 'blob' AND hash IN (SELECT value FROM json_each(?1))`).bind(JSON.stringify(shas)).all<{ hash: string }>()).results.map((x) => x.hash),
    );
    for (const s of shas)
      if (!blockedImg.has(s) && blobInfo.get(s)!.privacy_purged_at === null && !(await c.r2.head(`blobs/${s}`))) {
        if (!dry) await c.db.prepare(`UPDATE blobs SET missing_at = COALESCE(missing_at, ?2) WHERE sha256 = ?1`).bind(s, now).run();
        throw new PoolError(422, "blob_missing", { reason: "reupload", positions: qs.map((q, i) => (imgOf(q) === s ? i : -1)).filter((i) => i >= 0).slice(0, 100) });
      }
  }

  // Declared origins are recorded for the hard check, then dropped: provenance is the server's.
  const declared = qs.map((q) => (q.origin && typeof q.origin === "object" && typeof q.origin.item_id === "string" ? { item: q.origin.item_id, qid: typeof q.origin.qid === "string" ? q.origin.qid : "", version: typeof q.origin.version === "number" ? q.origin.version : -1 } : null));
  const day = Math.floor(now / DAY_MS) * DAY_MS;
  // Rewritten in place (no second tree of a 4 MB bank).
  let rewritten: PayloadJson | null = raw;
  raw = null;
  payload = null;
  (read as { payload: Uint8Array | null }).payload = null;
  const fixQ = (q: QuestionJson, i: number) => {
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
  };
  const rw = rewritten!;
  if (rw.bank) {
    rw.bank.id = itemId;
    rw.bank.created = day;
    rw.bank.modified = day;
    delete rw.bank.origin;
    delete rw.bank.published;
    rw.bank.questions.forEach(fixQ);
  } else fixQ(rw.question!, 0);
  let rBytes: Uint8Array | null = enc.encode(JSON.stringify(rewritten));
  // Peak memory (a 4 MB bank is ~100 MB of JS objects per copy): every intermediate is dropped as
  // soon as it is used. The first validation's tree goes now; the rewrite after the second pass.
  rewritten = null;
  qs.length = 0;

  // The one share-core call (round 7): validation, the stripped form, match and keyed full hashes;
  // its author-only data is taken synchronously inside the adapter call.
  const prep = c.core.publishPrepare(rBytes!, { hideKey: m.hideKey, labels: m.labels, orderAcks: m.orderAcks, imageOk: m.imageOk, ackFormatting: m.ackFormatting });
  // share-core round 8: the refusal carries the author's previews itself (no second call).
  if (isFail(prep)) throw invalid(prep);
  rBytes = null;
  const full = prep.full;
  const st: Stripped | null = prep.stripped;
  if ((full.payload.bank ? "bank" : "question") !== kind || questionsOf(full.payload).length !== n) throw new PoolError(422, "invalid_payload", { errors: [{ code: "invalid", path: "" }] });
  // A hidden republish whose served form is unchanged (a key correction, an explanation edit) keeps
  // the previous stripped body byte for byte and the public version: nothing a non-author sees,
  // not even the shuffle or a "newer" notice, reveals that something hidden changed (HOLD-1).
  const reused = st && prevHidden && baseOf ? await reuseServed(c, baseOf, qids, st, m.orderAcks) : null;
  const publicVersion = isNew ? 1 : reused ? prevPublic.version : prevPublic.version + 1;
  const publicAt = reused ? prevPublic.at : null; // null: this commit's time
  const fullQs = questionsOf(full.payload);
  const mh = st ? st.matchHashes : prep.matchHashes;
  const fh = prep.fullHashes;
  // Content hashes (what "the same content" compares, ids and dates aside): keyed, computed here
  // (round 12) instead of two more share-core passes.
  const fullContent = await c.core.contentHashes(questionsOf(full.payload));
  const strippedContent = reused ? reused.contents : st ? await c.core.contentHashes(questionsOf(st.payload)) : null;
  if (mh.length !== n || fh.length !== n || fullContent.length !== n) throw new Error("share-core hash count mismatch");

  // Key-free facts per question as served to non-authors: what "modified" is decided on when this
  // publish hides its key, so nothing a non-author sees can depend on the key (critic A1).
  const servedQs = st ? questionsOf(st.payload) : fullQs;
  const keyfree = await Promise.all(servedQs.map((q, i) => keyfreeHash(mh[i]!, q, imgOf)));
  // The licence sense of "the same question" (round 9 J1): points, difficulty and topics aside.
  const nearFull = await Promise.all(fullQs.map((q) => nearHash(q)));
  const nearStripped = reused ? reused.near : st ? await Promise.all(questionsOf(st.payload).map((q) => nearHash(q))) : null;
  const bareFull = await Promise.all(fullQs.map((q) => nearHash(q, true)));
  const bareStripped = reused ? reused.bare : st ? await Promise.all(questionsOf(st.payload).map((q) => nearHash(q, true))) : null;
  const noExpl = fullQs.map((q) => typeof q.explanation !== "string" || q.explanation.trim() === "");
  const nearKf = await Promise.all(servedQs.map((q, i) => nearKfHash(mh[i]!, q, imgOf)));
  // Two different questions are asked of every source:
  // * shown `modified` (in the chain, seen by everyone): key-free facts whenever this publish hides its
  //   key, so the flag can never reveal a key (critic A1);
  // * licence `modified` (decides the grant, seen only by this publisher): against exactly what the
  //   source served, explanation, points and image included (critic C4), key-free only for a
  //   keyless (stripped) serve.
  // Every non-author-visible derivation below compares through visible.ts: a hidden publish's
  // Subject has no full-content hash, so nothing visible can depend on its key (round 5).
  const subjects = keyfree.map((k, i) => subjectFor(m.hideKey, k, fullContent[i]!, nearFull[i]!, nearKf[i]!, bareFull[i]!, noExpl[i]!));
  // "Identical" is decided in SQL by visible.ts' sameSql, from the Subject alone (h.same).
  const differs = (_i: number, h: ImportHit) => !h.same;

  // A loose stem-only hash: a near copy of something served (edited choices, punctuation) raises an
  // "origin_suspect" flag for the admin. Never a refusal.
  const stems = await Promise.all(servedQs.map((q) => stemHash((q as { stem?: unknown }).stem)));
  // Normalised stem words and MinHash bands (round 10 M-3): the fuzzy-copy lookup, key-free.
  const tokens = servedQs.map((q) => stemTokens((q as { stem?: unknown }).stem));
  // The served stems' search words (round 11), taken now: the payload objects are released before staging.
  const searchWords = servedWords(servedQs);
  const bands = tokens.map((t) => stemBands(t));
  // Round 17 (critic22c M3): the served body's words (with the kind), so a near stem is only the same
  // question when the rest of it is too.
  const bodyToks = servedQs.map((q) => bodyTokens(q));
  // (Rounds 21-22 V3/B2: the keyed sketches stored with each question, ghosts keep them: HMAC-SHA256
  // under the 'sketch-v1' subkey, 64 bits, unordered sorted sets; and the keyed MinHash bands.)
  const skMemo = new Map<string, string>();
  const tokSk = await Promise.all(tokens.map((t) => keyedSet(t, skMemo)));
  const bodySk = await Promise.all(bodyToks.map((b) => keyedBody(b, skMemo)));
  const kBands = await Promise.all(bands.map((b) => keyedBands(b, skMemo)));

  // ---- licence and provenance: built by the server, never taken from the client --------------------
  // Evidence rules (critic D3): a source counts only if it is not the publisher's own work (nor
  // derived from it) and was first published before the publisher first published such a question.
  // A kept qid carries its previous provenance: dropping `origin` cannot clear it (critic A3).
  type PrevRow = { chain_json: string; origin_item_id: string | null; origin_qid: string | null; full_content: string; keyfree_content: string; match_hash: string; seq: number; lineage_seq: number | null; stem_tokens: string | null; allowed_json: string | null; body_hash: string | null; lineage_root: string | null; unit_seqs: string | null };
  const prev = new Map<string, PrevRow>();
  if (!isNew && baseOf !== null) {
    const kept = qids.filter((q, i) => q === sent[i]);
    if (kept.length > 0) {
      const rows = await c.db
        .prepare(`SELECT qid, chain_json, origin_item_id, origin_qid, full_content, keyfree_content, match_hash, seq, lineage_seq, stem_tokens, allowed_json, body_hash, lineage_root, unit_seqs FROM item_questions WHERE publish_id = ?1 AND qid IN (SELECT value FROM json_each(?2))`)
        .bind(baseOf, JSON.stringify(kept))
        .all<PrevRow & { qid: string }>();
      for (const r of rows.results) prev.set(r.qid, r);
    }
  }
  const carriedRefs = [...prev.values()].filter((x) => x.origin_item_id && x.origin_qid).map((x) => ({ item: x.origin_item_id!, qid: x.origin_qid! }));
  // All evidence in one bounded, aggregated query (round 7 H4): per position, the sources sharing its
  // question, its declared origin (a hint that counts only if it names a question this account was
  // served, at that version: critic A2) and its kept qid's recorded origin, each compared with the
  // position's Subject in SQL (visible.ts).
  const refsOf = (i: number) => {
    const out: { item: string; qid: string }[] = [];
    const d = declared[i];
    if (d && ID_RE.test(d.item) && ID_RE.test(d.qid)) out.push({ item: d.item, qid: d.qid });
    const pv = prev.get(qids[i]!);
    if (pv?.origin_item_id && pv.origin_qid) out.push({ item: pv.origin_item_id, qid: pv.origin_qid });
    return out;
  };
  // The author's own record first (key-free facts per position), so every admissibility rule that
  // does not compare content runs in SQL before anything is ranked or limited (round 8 I1).
  // Block keys (round 12 M4 / M6): normalised skeleton of the served form, normalised full content,
  // explanation and model / accepted answers of the full form (shared.ts QKEY_COLS).
  // Round 13: every block key is of the SERVED form (the stripped form of a hidden-key publish), so
  // no block can act on, and no hold can reveal, a hidden answer or explanation.
  const bkNorm = await Promise.all(servedQs.map((q) => normSkeletonKey(q)));
  const bkStem = await Promise.all(servedQs.map((q) => stemKey(q)));
  const bkFields = await Promise.all(servedQs.map((q) => fieldKeys(q)));
  // Round 24 (critic32a B-1): the FULL form's explanation block key, kept in the author's durable stem
  // record. A hidden publish does not serve its explanation, so without it a later SHOWN version of the
  // same question could not see that she is the same author who already had that explanation (and a
  // copier who edited before her would be credited as the earlier publisher). Keyed hash only,
  // server-side, never a block for anyone. (The ANSWER is not kept: a hidden key must not change an
  // observable outcome, and the H1 property holds the author's own flags to that too.)
  const bkFieldsFull = st ? await Promise.all(fullQs.map((q) => fieldKeys(q))) : bkFields;
  const stemRecord = bkFieldsFull.map((f, i) => ({ p: i, e: f.expl ?? "" }));
  const bkFull = (st ? (nearStripped ?? []) : nearFull).map((h) => `f:${h}`);
  // Round 16 (critic21c B5): each Multipart part's stem, explanation and answer keys (served form).
  const partKeys = await Promise.all(servedQs.map((q) => partKeysOf(q)));
  // Round 17 (critic22c M4): key-free unit keys of the question and of each part (licence evidence).
  const unitKeys = await Promise.all(servedQs.map((q) => unitKeysOf(q)));
  // ... and each part stem's words and MinHash bands (near-rewordings of a blocked part stem).
  const partStems = servedQs.map((q) =>
    (((q as unknown as { kind?: { Multipart?: { parts?: { stem?: unknown }[] } } }).kind?.Multipart?.parts ?? []) as { stem?: unknown }[]).map((p) => {
      const t = stemTokens(p?.stem);
      return { tokens: t, bands: stemBands(t) };
    }),
  );
  const partKBands = await Promise.all(partStems.map((ps) => Promise.all(ps.map((p) => keyedBands(p.bands, skMemo)))));
  const blockKeys = [...new Set([...mh, ...bkNorm, ...bkStem, ...bkFull, ...bkFields.flatMap((f) => [f.expl, f.ans]).filter((x): x is string => x !== null), ...partKeys.flat().map((x) => x.key)])];
  // Everything but the stem, as served (round 12 M1): what a question ADDS to its stem.
  const body = await Promise.all(servedQs.map((q) => nearHash({ ...(q as object), stem: "" } as never)));
  // The key is itself a work (a Short's model answer): key-free equality never makes anyone first.
  const keywork = fullQs.map((q) => (kindTag(q) === "Short" && String((q as { kind?: { Short?: { sample?: unknown } } }).kind?.Short?.sample ?? "").trim() !== "" ? 1 : 0));
  // The author's own lineage (rounds 11 P1, 12 M1): a kept qid whose new content changes ONLY the stem
  // (the same body; the same match_hash or a stem Jaccard >= FUZZY_JACCARD) keeps the qid's lineage
  // seq. A body that changes (a stem-only question turned into a rich one) starts a new lineage.
  // Round 14 (B6): and every step stays within LINEAGE_ROOT_JACCARD of the lineage's ROOT stem, so
  // no walk of one word at a time reaches someone else's stem.
  const lineageRoot = qids.map((q, i) => {
    const pv = prev.get(q);
    return pv?.lineage_root ? (JSON.parse(pv.lineage_root) as string[]) : pv ? (JSON.parse(pv.stem_tokens ?? "[]") as string[]) : tokens[i]!;
  });
  const lineage = qids.map((q, i) => {
    const pv = prev.get(q);
    if (!pv || pv.body_hash === null || pv.body_hash !== body[i]) return null;
    const cont = (pv.match_hash === mh[i] || jaccard(JSON.parse(pv.stem_tokens ?? "[]") as string[], tokens[i]!) >= FUZZY_JACCARD) && jaccard(lineageRoot[i]!, tokens[i]!) >= LINEAGE_ROOT_JACCARD;
    return cont ? pv.lineage_seq ?? pv.seq : null;
  });
  // Round 25 (critic-moodle-24 B-2): a real publish repairs a legacy author's stem records first
  // (bounded; no writes at all in a dry run), so her hidden earlier version's explanation key is
  // known before the evidence rules read it.
  if (!dry) await stemBackfillAuthor(c, me);
  const actx = await authorContext(c, me, mh);
  // The author's first publish of THIS content (round 12 M1: shown content, or key-free content
  // unless the key is a work), never of a bare stem or match_hash: a planted stem makes no one first.
  const ownCf = await ownContentFirst(
    c,
    me,
    servedQs.map((_, i) => ({ n: (m.hideKey ? (nearStripped?.[i] ?? null) : nearFull[i]) ?? null, kf: nearKf[i]!, keywork: keywork[i]! })),
  );
  const firstOf = (i: number) => ownCf.get(i) ?? null;
  // A short statement (round 10 M-5): Tf, Short or Blank with a stem of at most SHORT_WORDS words.
  const shortQ = servedQs.map((q) => isShort(q));
  // Round 32 (circles, D13): material first sent into a circle by someone else stays out of the pool
  // (every form's key-free hash; dry runs too). circlecheck.ts says when a position passes.
  const circleClaimed = (await checkCircleMaterial(c, me, mh.map((h, i) => [h, prep.matchHashes[i]!]), shortQ, circleClaim)).claimed;
  // Round 14 (B5): 40 words or fewer as served (stem, choices, columns, served answers, explanation).
  const short40 = servedQs.map((q) => (servedWordCount(q) <= 40 ? 1 : 0));
  // Round 18 (U3/U6) text units, from the payload trees (used below with the evidence): see "Texts".
  const words = (s: unknown) => (typeof s === "string" ? normText(s).split(" ").filter(Boolean).length : 0);
  const textKeysOf = async (q: QuestionJson): Promise<string[]> => {
    const x = q as unknown as { explanation?: unknown; kind?: { Short?: { sample?: unknown }; Multipart?: { parts?: Record<string, any>[] } } };
    const texts: unknown[] = [x.explanation, x.kind?.Short?.sample];
    for (const p0 of x.kind?.Multipart?.parts ?? []) texts.push(p0?.explanation, p0?.kind?.Short?.sample);
    const long = texts.filter((t): t is string => typeof t === "string" && words(t) >= LIFT_WORDS);
    return [...new Set((await Promise.all(long.map((t) => textFieldKeys(t)))).flat())];
  };
  const servedText = await Promise.all(servedQs.map(textKeysOf));
  const hiddenText = st ? (await Promise.all(fullQs.map(textKeysOf))).map((ks, i) => ks.filter((k) => !servedText[i]!.includes(k))) : servedQs.map(() => [] as string[]);
  const imgAt = servedQs.map((q) => imgOf(q));
  // ---- round 29: the payload trees go BEFORE the D1-heavy phase ----------------------------------
  // The evidence and fuzzy lookups below reserve their worst-case D1 results (8 MB per evidence query,
  // up to 16 MB per fuzzy lookup). They were reserved ON TOP of this publish's whole heavy-phase
  // reservation, while both payload trees were still alive: measured (node, --expose-gc) on a 3.8 MB
  // hidden-key Matching bank (990 x 50 pairs), 47 MB of the publish's 61 MB was then live, so the
  // lookup's 16 MB did not fit in what was left, and on top of it the 61 + 16 MB passed the 72 MB
  // budget on an idle isolate. So the few facts needed later are taken now, the trees (and the
  // sketch memo, a pure cache) are dropped, and the lookups BORROW the part of the heavy reservation
  // the publish provably no longer uses (`lend`, below): the reservation minus postHeavyBytes, an
  // upper bound of what it still holds (README "Round 29"). The rest of a lookup's worst case is
  // reserved from the isolate as before. The heavy reservation itself is kept whole until the end:
  // staging builds large statements again.
  const servedKind = servedQs.map((q) => kindTag(q));
  const fullImg = fullQs.map((q) => imgOf(q));
  const kindsMask = fullQs.reduce((acc, q) => acc | (KIND_BITS[kindTag(q)] ?? 0), 0);
  const hasImages = servedQs.some((q) => q.image) ? 1 : 0;
  const lite = fullQs.map((q) => ({ kind: kindTag(q), difficulty: typeof q.difficulty === "string" ? q.difficulty : null, topics: (q.topics ?? []) as string[] }));
  const bankName = full.payload.bank ? String(full.payload.bank.name ?? "") : "";
  const schemaVersion = full.payload.schema_version;
  const strippedShas = st ? [...new Set(questionsOf(st.payload).map(imgOf).filter((x): x is string => x !== null))] : [];
  const fullTags = tagsOf(meta.tags, fullQs);
  let parts = 0, wordCount = 0, wordChars = 0;
  const countWords = (ts: readonly string[]) => {
    wordCount += ts.length;
    for (const t of ts) wordChars += t.length;
  };
  tokens.forEach(countWords);
  bodyToks.forEach(countWords);
  for (const ps of partStems) for (const p of ps) (parts++, countWords(p.tokens));
  (full as { payload: unknown }).payload = null;
  if (st) (st as { payload: unknown }).payload = null;
  fullQs.length = 0;
  servedQs.length = 0;
  skMemo.clear();
  const lend = lender(heavy.bytes - postHeavyBytes({ bodies: full.bytes.length + (st ? st.bytes.length : 0) + (reused ? reused.bytes.length : 0), metaBytes, questions: n, parts, words: wordCount, wordChars }));
  const params = subjects.map((sj, i) => ({
    ...subjectParam(sj, mh[i]!, refsOf(i)),
    first: firstOf(i),
    ownshown: sj.full !== null && (actx.own.get(mh[i]!)?.shown.has(sj.full) ?? false),
    common: shortQ[i]!,
    croots: actx.roots.get(mh[i]!) ?? [],
    ck: actx.k,
    ownkf: actx.ownKf.has(nearKf[i]!) && keywork[i] === 0,
    keywork: keywork[i]!,
    short40: short40[i]!,
  }));
  const ev = await evidence(c, me, params, { mine: [...actx.mine], independent: m.independent, reserve: lend });
  // Round 16 (B6, B7) and round 17 (critic22c M1-M6): material a question uses besides itself is a
  // licence unit of its own: an explanation or Short model answer of LIFT_WORDS+ words ("text": its
  // explanation / answer keys, either field), its image ("image": exact bytes, or the same decoded
  // pixels), one of the source's questions as a part of this one or this one as one of its parts
  // ("part": key-free unit keys, both ways). It binds and is credited (typed link) when the account
  // was SERVED the publish that contains it (an import or a full read of that version; a later version
  // it never saw does not count), unless the account had the material first (its own earlier publish,
  // durable: author_contents 'unit'), the source derives from the account's own item, or it is stock
  // (GENERIC_UNIT_AUTHORS independent authors' visible items have it). An independence claim never
  // covers it, except a text or image the claim attributes to a public source (attribution words in
  // the statement): then a possible source, flagged. Material equal to an EARLIER public item the
  // account was never served: credited as an earlier publication (unverified, flagged), never a refusal.
  // Texts (round 18 U3/U6): the explanation and Short model answer of the question AND of each of its
  // parts, LIFT_WORDS+ words. Built from the SERVED form (key-free when the key is hidden); a hidden
  // publish's hidden-only texts are units too, but only privately (below).
  // (Round 29: servedText, hiddenText and imgAt are taken above, before the payload trees go.)
  const pixelOf = new Map<string, string>();
  {
    const shasServed = [...new Set(imgAt.filter((x): x is string => x !== null))];
    if (shasServed.length > 0)
      for (const r0 of (await c.db.prepare(`SELECT sha256, pixel_hash FROM blobs WHERE sha256 IN (SELECT value FROM json_each(?1)) AND pixel_hash IS NOT NULL`).bind(JSON.stringify(shasServed)).all<{ sha256: string; pixel_hash: string }>()).results)
        pixelOf.set(r0.sha256, r0.pixel_hash);
  }
  type UnitKind = "text" | "image" | "part";
  const unitsAt: { key: string; kind: UnitKind; part?: number; hidden?: boolean; stemOnly?: boolean }[][] = imgAt.map((_, i) => [
    ...servedText[i]!.map((key) => ({ key, kind: "text" as const })),
    ...hiddenText[i]!.map((key) => ({ key, kind: "text" as const, hidden: true })),
    ...(imgAt[i] ? [{ key: imgAt[i]!, kind: "image" as const }, ...(pixelOf.has(imgAt[i]!) ? [{ key: pixelOf.get(imgAt[i]!)!, kind: "image" as const }] : [])] : []),
    ...unitKeys[i]!.map((u) => ({ key: u.key, kind: "part" as const, part: u.part, stemOnly: u.stemOnly })),
  ]);
  type Lift = {
    item_id: string; qid: string; version: number; license: string; author_name: string; seq: number; chain: string; kind: UnitKind; key: string;
    served: boolean; evKind: "import" | "view" | null; received: string[]; stock: boolean; ownFirst: boolean; visible: boolean; uploadedFirst: boolean; hidden: boolean; stemOnly: boolean;
  };
  const lifts = new Map<number, Lift[]>();
  const want = unitsAt.flatMap((us, pos) => us.map((u) => ({ pos, key: u.key, kind: u.kind, part: u.part ?? -1, hidden: u.hidden ? 1 : 0, stemOnly: u.stemOnly ? 1 : 0 })));
  // Round 21 (critic27b B-1/B-5): the cap is on the publisher's OWN distinct unit keys (no third party
  // can grow it); the lookups below are bounded per key, so nothing anyone else publishes multiplies
  // their work.
  const wantKeys = [...new Set(want.map((x) => x.key))];
  const keyMax = c.deps.unitKeysMax ?? UNIT_KEYS_MAX;
  if (wantKeys.length > keyMax)
    throw new PoolError(422, "too_many_units", { limit: keyMax, message: "This bank has more distinct texts, images and parts than one publish can check. Publish it as several smaller banks." });
  // Round 20 (critic25b N4): a dry run's unit lookups count against their own daily allowance, weighted
  // by the units looked up (a big multipart bank costs what it costs), before they run.
  if (dry && want.length > 0) await requireDaily(c, me, "preview_units", want.length);
  if (want.length > 0) {
    // Round 21: how often each key occurs anywhere, counted only up to OCC_SCAN. A key past that ("hot":
    // a stock phrase, or planted copies) is matched from this account's OWN served content instead,
    // whose size no one else controls; the rest by key. (The choice changes the plan, never the answer.)
    const occSql = `SELECT k.value AS key,
                (SELECT COUNT(*) FROM (SELECT 1 FROM item_questions x WHERE x.bk_expl = k.value LIMIT ?2))
              + (SELECT COUNT(*) FROM (SELECT 1 FROM item_questions x WHERE x.bk_ans = k.value LIMIT ?2))
              + (SELECT COUNT(*) FROM (SELECT 1 FROM item_part_keys x WHERE x.key = k.value LIMIT ?2))
              + (SELECT COUNT(*) FROM (SELECT 1 FROM item_blobs x WHERE x.sha256 = k.value LIMIT ?2))
              + (SELECT COUNT(*) FROM (SELECT 1 FROM blobs bx JOIN item_blobs x ON x.sha256 = bx.sha256 WHERE bx.pixel_hash = k.value LIMIT ?2))
              + (SELECT COUNT(*) FROM (SELECT 1 FROM ghost_units x WHERE x.key = k.value LIMIT ?2)) AS n
           FROM json_each(?1) k`;
    const occN = await c.db.prepare(occSql).bind(JSON.stringify(wantKeys), OCC_SCAN + 1).all<{ key: string; n: number }>();
    // Round 22 (critic29b): planted copies alone (65 of them) must not put this account on the
    // served-driven path, whose cost is its OWN reads: a key past OCC_SCAN is counted again up to the
    // size of that path (B, the account's served rows, at most HOT_SCAN), within HOT_BUDGET in all; a
    // key with no more than B occurrences is matched by key (cost <= B). Only a key with more copies
    // than the account has serves (or past the budget) goes served-driven.
    const over = occN.results.filter((x) => x.n > OCC_SCAN).map((x) => x.key);
    const byKeyAfterAll = new Set<string>();
    if (over.length > 0) {
      const sn = (await c.db.prepare(`SELECT COUNT(*) AS n FROM (SELECT 1 FROM served WHERE reader_id = ?1 LIMIT ${HOT_SCAN + 1})`).bind(me).first<{ n: number }>())?.n ?? 0;
      const B = Math.min(sn, HOT_SCAN);
      const recount = B > OCC_SCAN ? over.slice(0, Math.floor(HOT_BUDGET / B)) : [];
      if (recount.length > 0) {
        const again = new Map((await c.db.prepare(occSql).bind(JSON.stringify(recount), B + 1).all<{ key: string; n: number }>()).results.map((x) => [x.key, x.n]));
        for (const k of recount) if ((again.get(k) ?? 0) <= B) byKeyAfterAll.add(k);
      }
    }
    const hotKeys = new Set(occN.results.filter((x) => x.n > OCC_SCAN && !byKeyAfterAll.has(x.key)).map((x) => x.key));
    // (Never weighted by those counts: they include items the caller may not see, and a quota that
    // moved with them would tell whether a private text exists (critic27a V4). Each key's lookups are
    // bounded by OCC_SCAN, so the per-unit charge above is already proportional to the work.)
    const wj = JSON.stringify(want.map((x) => ({ ...x, hot: hotKeys.has(x.key) ? 1 : 0 })));
    const wCte = `w AS MATERIALIZED (SELECT CAST(j.value ->> '$.pos' AS INTEGER) AS pos, j.value ->> '$.key' AS key, j.value ->> '$.kind' AS kind, CAST(j.value ->> '$.part' AS INTEGER) AS part,
                                           CAST(j.value ->> '$.hidden' AS INTEGER) AS hidden, CAST(j.value ->> '$.stemOnly' AS INTEGER) AS stem_only,
                                           CAST(j.value ->> '$.hot' AS INTEGER) AS hot FROM json_each(?2) j)`;
    const common = `(SELECT MIN(ac.first_seq) FROM author_contents ac WHERE ac.author_id = ?1 AND ac.kind = 'unit' AND ac.hash = w.key) AS own_seq,
                    (i.visibility = 'public' AND i.current_publish_id IS NOT NULL AND i.removed_at IS NULL AND NOT EXISTS (SELECT 1 FROM item_holds hh WHERE hh.item_id = i.id)) AS visible,
                    (SELECT MIN(sp.committed_at) FROM item_publishes sp WHERE sp.item_id = i.id) AS src_at,
                    (SELECT MIN(bu.first_at) FROM blob_uploads bu WHERE bu.uploader_id = ?1 AND (bu.sha256 = w.key OR bu.sha256 IN (SELECT bx.sha256 FROM blobs bx WHERE bx.pixel_hash = w.key))) AS my_upload_at`;
    // (1) What this account was SERVED (rounds 19-21): the units of every publish it was served (its
    // `served` rows), read from that publish's own key set, or its ghost once it is gone; with every
    // licence the account was shown (and the item's licence now). Per (question, key) the EARLIEST
    // UNIT_SOURCES_PER_KEY sources are kept: every source of one unit is one material, decided by its
    // earliest source, so the rest can change nothing.
    const servedOuter = (occ: string) => `WITH ${wCte}, occ AS MATERIALIZED (${occ}),
         g AS (
__OUTER__
           FROM occ o CROSS JOIN served s ON s.reader_id = ?1 AND s.publish_id = o.publish_id AND s.void_at IS NULL
           JOIN items i ON i.id = s.item_id JOIN users u ON u.id = i.author_id
          WHERE i.author_id <> ?1 AND i.id NOT IN (SELECT value FROM json_each(?3)) AND NOT EXISTS (SELECT 1 FROM void_items v WHERE v.item_id = i.id)
          GROUP BY o.pos, o.key, i.id)
       SELECT * FROM (SELECT g.*, ROW_NUMBER() OVER (PARTITION BY g.pos, g.key ORDER BY g.seq, g.item_id) AS rk FROM g) WHERE rk <= ${UNIT_SOURCES_PER_KEY}`;
    // Keys counted at most OCC_SCAN times: by key.
    const coldOcc = `
                SELECT w.pos, w.key, w.kind, w.hidden, w.stem_only, iq.publish_id, COALESCE(json_extract(iq.unit_seqs, '$."' || w.key || '"'), iq.seq) AS seq, iq.qid, iq.chain_json, NULL AS now_lic FROM w CROSS JOIN item_questions iq ON w.hot = 0 AND w.kind = 'text' AND (iq.bk_expl = w.key OR iq.bk_ans = w.key)
                UNION ALL
                SELECT w.pos, w.key, w.kind, w.hidden, w.stem_only, pk.publish_id, uq.seq, uq.qid, uq.chain_json, NULL FROM w CROSS JOIN item_part_keys pk
                  ON w.hot = 0 AND pk.key = w.key AND (w.kind = 'text' OR (w.kind = 'part' AND (pk.part >= 0 OR w.part >= 0)))
                  JOIN item_questions uq ON uq.publish_id = pk.publish_id AND uq.pos = pk.pos
                UNION ALL
                SELECT w.pos, w.key, w.kind, w.hidden, w.stem_only, ib.publish_id, ib.seq, '' AS qid, '[]' AS chain_json, NULL FROM w CROSS JOIN item_blobs ib
                  ON w.hot = 0 AND w.kind = 'image' AND (ib.sha256 = w.key OR (w.key LIKE 'px:%' AND ib.sha256 IN (SELECT bx.sha256 FROM blobs bx WHERE bx.pixel_hash = w.key)))
                  CROSS JOIN item_publishes ip ON ip.publish_id = ib.publish_id AND ${SERVED_FORM("ib", "ip")}
                UNION ALL
                SELECT w.pos, w.key, w.kind, w.hidden, w.stem_only, gu.publish_id, COALESCE(gu.seq, gq.seq, 0), COALESCE(gq.qid, ''), COALESCE(gq.chain_json, '[]'), gu.now_licenses FROM w CROSS JOIN ghost_units gu
                  ON w.hot = 0 AND gu.key = w.key AND gu.kind = w.kind AND (w.kind <> 'part' OR gu.part >= 0 OR w.part >= 0)
                  LEFT JOIN ghost_questions gq ON gq.publish_id = gu.publish_id AND gq.pos = gu.pos`;
    // Hot keys: from this account's served publishes (and their ghosts), never from the whole pool.
    const hotOcc = `
                SELECT w.pos, w.key, w.kind, w.hidden, w.stem_only, iq.publish_id, COALESCE(json_extract(iq.unit_seqs, '$."' || hk.value || '"'), iq.seq) AS seq, iq.qid, iq.chain_json, NULL AS now_lic
                  FROM served sh CROSS JOIN item_questions iq ON iq.publish_id = sh.publish_id CROSS JOIN json_each(json_array(iq.bk_expl, iq.bk_ans)) hk
                  CROSS JOIN w ON w.hot = 1 AND w.kind = 'text' AND w.key = hk.value WHERE sh.reader_id = ?1 AND sh.void_at IS NULL
                UNION ALL
                SELECT w.pos, w.key, w.kind, w.hidden, w.stem_only, pk.publish_id, uq.seq, uq.qid, uq.chain_json, NULL
                  FROM served sh CROSS JOIN item_part_keys pk ON pk.publish_id = sh.publish_id
                  CROSS JOIN w ON w.hot = 1 AND w.key = pk.key AND (w.kind = 'text' OR (w.kind = 'part' AND (pk.part >= 0 OR w.part >= 0)))
                  JOIN item_questions uq ON uq.publish_id = pk.publish_id AND uq.pos = pk.pos WHERE sh.reader_id = ?1 AND sh.void_at IS NULL
                UNION ALL
                SELECT w.pos, w.key, w.kind, w.hidden, w.stem_only, ib.publish_id, ib.seq, '' AS qid, '[]' AS chain_json, NULL
                  FROM served sh CROSS JOIN item_blobs ib ON ib.publish_id = sh.publish_id CROSS JOIN item_publishes ip ON ip.publish_id = ib.publish_id AND ${SERVED_FORM("ib", "ip")}
                  LEFT JOIN blobs bx ON bx.sha256 = ib.sha256
                  CROSS JOIN w ON w.hot = 1 AND w.kind = 'image' AND (w.key = ib.sha256 OR w.key = bx.pixel_hash) WHERE sh.reader_id = ?1 AND sh.void_at IS NULL
                UNION ALL
                SELECT w.pos, w.key, w.kind, w.hidden, w.stem_only, gu.publish_id, COALESCE(gu.seq, gq.seq, 0), COALESCE(gq.qid, ''), COALESCE(gq.chain_json, '[]'), gu.now_licenses
                  FROM served sh CROSS JOIN ghost_units gu ON gu.publish_id = sh.publish_id
                  CROSS JOIN w ON w.hot = 1 AND gu.key = w.key AND gu.kind = w.kind AND (w.kind <> 'part' OR gu.part >= 0 OR w.part >= 0)
                  LEFT JOIN ghost_questions gq ON gq.publish_id = gu.publish_id AND gq.pos = gu.pos WHERE sh.reader_id = ?1 AND sh.void_at IS NULL`;
    const servedQ = (occ: string) =>
      c.db.prepare(servedOuter(occ).replace("__OUTER__", OUTER_SQL)).bind(me, wj, JSON.stringify([...actx.mine])).all<Record<string, any>>();
    const served = { results: [...(await servedQ(coldOcc)).results, ...(hotKeys.size > 0 ? (await servedQ(hotOcc)).results : [])] };
    // (2) Where else it occurs (retained publishes, as served to non-authors): stock, and relays of
    // material this account was never served. Never for hidden-only texts. Round 21 (critic27b B-1):
    // per key only the EARLIEST UNIT_OCC_WINDOW occurrences (by when they were stored) are looked at,
    // and of those only listed items or ones this account was served, earlier than its own first use
    // of the unit (later copies of an author's own material never weigh on her).
    const rows = await c.db
      .prepare(
        `WITH ${wCte},
              hit AS MATERIALIZED (
                SELECT w.pos, w.key, w.kind, iq.publish_id, COALESCE(json_extract(iq.unit_seqs, '$."' || w.key || '"'), iq.seq) AS seq, iq.qid, iq.chain_json FROM w CROSS JOIN item_questions iq
                  ON w.kind = 'text' AND w.hidden = 0
                 AND (iq.rowid IN (SELECT x.rowid FROM item_questions x WHERE x.bk_expl = w.key ORDER BY x.rowid LIMIT ?4)
                      OR iq.rowid IN (SELECT x.rowid FROM item_questions x WHERE x.bk_ans = w.key ORDER BY x.rowid LIMIT ?4))
                UNION ALL
                SELECT w.pos, w.key, w.kind, pk.publish_id, uq.seq, uq.qid, uq.chain_json FROM w CROSS JOIN item_part_keys pk
                  ON ((w.kind = 'text' AND w.hidden = 0) OR (w.kind = 'part' AND (pk.part >= 0 OR w.part >= 0)))
                 AND pk.rowid IN (SELECT x.rowid FROM item_part_keys x WHERE x.key = w.key ORDER BY x.rowid LIMIT ?4)
                  JOIN item_questions uq ON uq.publish_id = pk.publish_id AND uq.pos = pk.pos
                UNION ALL
                SELECT w.pos, w.key, w.kind, ib.publish_id, ib.seq, '' AS qid, '[]' AS chain_json FROM w CROSS JOIN item_blobs ib
                  ON w.kind = 'image'
                 AND (ib.rowid IN (SELECT x.rowid FROM item_blobs x WHERE x.sha256 = w.key ORDER BY x.rowid LIMIT ?4)
                      OR ib.rowid IN (SELECT x.rowid FROM item_blobs x WHERE x.sha256 IN (SELECT bx.sha256 FROM blobs bx WHERE bx.pixel_hash = w.key) ORDER BY x.rowid LIMIT ?4)))
         SELECT h.pos, h.key, h.kind, i.id AS item_id, MIN(h.qid) AS qid, i.public_version AS version, i.license, COALESCE(u.display_name, 'deleted teacher') AS author_name,
                MIN(h.seq) AS seq, MAX(h.chain_json) AS chain, MAX(${common.split(" AS own_seq")[0]}) AS own_seq,
                MAX(i.visibility = 'public' AND i.current_publish_id IS NOT NULL AND i.removed_at IS NULL AND NOT EXISTS (SELECT 1 FROM item_holds hh WHERE hh.item_id = i.id)) AS visible
           FROM hit h CROSS JOIN item_publishes p ON p.publish_id = h.publish_id AND p.status IN ('current', 'previous') JOIN w ON w.pos = h.pos AND w.key = h.key
           JOIN items i ON i.id = p.item_id JOIN users u ON u.id = i.author_id
          WHERE i.author_id <> ?1 AND i.purged_at IS NULL AND i.id NOT IN (SELECT value FROM json_each(?3))
            AND (h.kind <> 'image' OR EXISTS (SELECT 1 FROM item_blobs sb WHERE sb.publish_id = p.publish_id AND ${SERVED_FORM("sb", "p")}))
            AND ((i.visibility = 'public' AND i.removed_at IS NULL) OR EXISTS (SELECT 1 FROM served sv WHERE sv.item_id = i.id AND sv.reader_id = ?1))
            AND NOT EXISTS (SELECT 1 FROM author_contents ac WHERE ac.author_id = ?1 AND ac.kind = 'unit' AND ac.hash = h.key AND ac.first_seq <= h.seq)
          GROUP BY h.pos, h.key, i.id`,
      )
      .bind(me, wj, JSON.stringify([...actx.mine]), UNIT_OCC_WINDOW)
      .all<{ pos: number; key: string; kind: UnitKind; item_id: string; qid: string; version: number; license: string; author_name: string; seq: number; chain: string | null; own_seq: number | null; visible: number }>();
    // Stock: GENERIC_UNIT_AUTHORS or more authors who published the unit INDEPENDENTLY (round 21,
    // critic27a M-B3): in a listed item, never served (read or imported) an earlier holder of it, and
    // not descending from one by lineage. Copies and socks of an earlier holder never make it stock.
    type Occ = { key: string; item_id: string; seq: number; visible: number; chain: string | null };
    const occAll = [...(rows.results as Occ[]), ...(served.results as Occ[])];
    const byKey = new Map<string, Occ[]>();
    for (const r0 of occAll) byKey.set(r0.key, [...(byKey.get(r0.key) ?? []), r0]);
    const allItems = [...new Set(occAll.map((x) => x.item_id))];
    const itemAuthor = new Map<string, string>();
    const servedPairs = new Set<string>();
    if (allItems.length > 0) {
      for (const r0 of (await c.db.prepare(`SELECT id, author_id FROM items WHERE id IN (SELECT value FROM json_each(?1))`).bind(JSON.stringify(allItems)).all<{ id: string; author_id: string }>()).results)
        itemAuthor.set(r0.id, r0.author_id);
      const authors = [...new Set(itemAuthor.values())];
      for (const r0 of (
        await c.db
          .prepare(`SELECT s.reader_id, s.item_id FROM json_each(?1) a CROSS JOIN served s ON s.reader_id = a.value WHERE s.item_id IN (SELECT value FROM json_each(?2))`)
          .bind(JSON.stringify(authors), JSON.stringify(allItems))
          .all<{ reader_id: string; item_id: string }>()
      ).results)
        servedPairs.add(`${r0.reader_id}/${r0.item_id}`);
    }
    const stockMemo = new Map<string, boolean>();
    const stock = (key: string) => {
      const memo = stockMemo.get(key);
      if (memo !== undefined) return memo;
      const occs = [...new Map((byKey.get(key) ?? []).map((x) => [x.item_id, x])).values()].sort((a, b) => a.seq - b.seq);
      const indep = new Map<string, number>();
      // Round 22 (critic29b B2): and it is stock only if at least GENERIC_UNIT_AUTHORS - 1 of those
      // published it BEFORE the earliest source this account was served (or, served none, before its
      // first holder): a text that first appeared with one author and was later copied by unrelated
      // accounts (socks or not) stays that author's.
      const servedSeqs = (served.results as Occ[]).filter((x) => x.key === key).map((x) => x.seq);
      const firstSource = servedSeqs.length > 0 ? Math.min(...servedSeqs) : (occs[0]?.seq ?? 0);
      occs.forEach((o, k) => {
        const author = itemAuthor.get(o.item_id);
        if (!author || o.visible !== 1) return;
        const earlier = occs.slice(0, k).filter((e) => e.seq < o.seq && itemAuthor.get(e.item_id) !== author);
        // (Lineage only: an `earlier_match` link is the server noting an earlier publication, not a derivation.)
        let lineage: string[] = [];
        try {
          lineage = (JSON.parse(o.chain ?? "[]") as ChainLink[]).filter((l) => !l.earlier_match).map((l) => l.item_id);
        } catch {
          lineage = [];
        }
        if (earlier.some((e) => servedPairs.has(`${author}/${e.item_id}`) || lineage.includes(e.item_id))) return;
        indep.set(author, Math.min(indep.get(author) ?? Infinity, o.seq));
      });
      const out = indep.size >= GENERIC_UNIT_AUTHORS && [...indep.values()].filter((s) => s < firstSource).length >= GENERIC_UNIT_AUTHORS - 1;
      stockMemo.set(key, out);
      return out;
    };
    const seen = new Set<string>();
    for (const x of served.results) {
      const l: Lift = {
        item_id: x.item_id, qid: x.qid, version: x.version, license: x.license, author_name: x.author_name, seq: x.seq, chain: x.chain ?? "[]", kind: x.kind, key: x.key,
        served: true, evKind: x.ev_kind, received: [
          ...new Set([
            ...(JSON.parse(x.lics_nested) as string[][]).flat(),
            ...(JSON.parse(x.now_lics) as string[][]).flat(),
            ...(JSON.parse(x.hist_lics ?? "[]") as (string | null)[]).filter((l): l is string => typeof l === "string"),
            ...(JSON.parse(x.licenses_json) as (string | null)[]).filter((l): l is string => typeof l === "string"),
          ]),
        ], stock: stock(x.key), ownFirst: x.own_seq !== null && x.own_seq <= x.seq,
        visible: x.visible === 1, uploadedFirst: x.kind === "image" && x.my_upload_at !== null && x.src_at !== null && x.my_upload_at < x.src_at, hidden: x.hidden === 1, stemOnly: x.stem_only === 1,
      };
      seen.add(`${x.pos}/${x.item_id}/${x.key}`);
      // (A source deriving from this account's item may still add its OWN material: only the
      // account's own earlier use of the very unit exempts it.)
      if (l.ownFirst || l.stock) continue;
      lifts.set(x.pos, [...(lifts.get(x.pos) ?? []), l]);
    }
    // Round 19 (critic24d D1): a retained occurrence in an item this account has a serve record of
    // whose publish's units are not known (a pre-0104 serve of a publish gone since: its ghost kept
    // question hashes only) binds as SERVED, with the licences that record holds. A serve whose
    // publish's units ARE known was examined exactly in (1): a version never seen does not bind
    // (round 17 M1 S1b); an item never served is the soft "earlier publication" path.
    const rec = new Map<string, { kind: "import" | "view"; lics: string[] }>();
    const relayItems = [...new Set(rows.results.map((x) => x.item_id))];
    if (relayItems.length > 0)
      for (const r0 of (
        await c.db
          .prepare(
            `SELECT item_id, MAX(kind = 'import') AS imp, json_group_array(DISTINCT l) AS lics FROM (
               SELECT s.item_id, s.kind, l.value AS l FROM json_each(?2) it CROSS JOIN served s ON s.item_id = it.value AND s.reader_id = ?1, json_each(s.licenses_json) l
                WHERE s.void_at IS NULL
                  AND NOT EXISTS (SELECT 1 FROM item_publishes kp WHERE kp.publish_id = s.publish_id)
                  AND NOT EXISTS (SELECT 1 FROM ghost_publishes kg WHERE kg.publish_id = s.publish_id AND kg.units_known = 1)
               UNION ALL
               -- (an import record with no served row at all: the same)
               SELECT im.item_id, im.kind, l.value FROM imports im, json_each(COALESCE(im.licenses_json, json_array(im.license))) l WHERE im.user_id = ?1 AND im.item_id IN (SELECT value FROM json_each(?2))
                  AND NOT EXISTS (SELECT 1 FROM served v WHERE v.reader_id = ?1 AND v.item_id = im.item_id))
             GROUP BY item_id`,
          )
          .bind(me, JSON.stringify(relayItems))
          .all<{ item_id: string; imp: number; lics: string }>()
      ).results)
        rec.set(r0.item_id, { kind: r0.imp === 1 ? "import" : "view", lics: JSON.parse(r0.lics) as string[] });
    for (const x of rows.results) {
      if (seen.has(`${x.pos}/${x.item_id}/${x.key}`)) continue;
      const had = rec.get(x.item_id);
      const l: Lift = {
        ...x, chain: x.chain ?? "[]", served: had !== undefined, evKind: had?.kind ?? null, received: had?.lics ?? [], stock: stock(x.key),
        ownFirst: x.own_seq !== null && x.own_seq <= x.seq, visible: x.visible === 1, uploadedFirst: false, hidden: false, stemOnly: false,
      };
      if (l.ownFirst || l.stock) continue;
      lifts.set(x.pos, [...(lifts.get(x.pos) ?? []), l]);
    }
  }
  const attributed = typeof m.statement === "string" && ATTRIBUTION_RE.test(m.statement);
  // Near-rewordings of what this account was served (round 10 M-3): an import becomes an adaptation
  // (hard), a read soft; both are flagged origin_suspect.
  // The author's own near-rewordings published before (round 11 P1): a later fuzzy source is no origin.
  const ownCut = new Set<number>();
  const ownLoose = new Map<number, { seq: number; expl: string | null; ans: string | null }>();
  const ownFuzzy = await ownFuzzyFirst(c, me, mh.map((_, i) => ({ pos: i, tokens: tokens[i]!, bands: bands[i]!, body: body[i]!, bodyToks: bodyToks[i]!, expl: bkFields[i]!.expl, ans: bkFields[i]!.ans })), ownCut, ownLoose, lend);
  // For fuzzy sources and origin_suspect: also the qid's lineage and the author's own near-rewordings.
  const fuzzyFirst = (i: number) => {
    const xs = [firstOf(i), lineage[i] ?? undefined, ownFuzzy.get(i)].filter((x): x is number => typeof x === "number" && x > 0);
    return xs.length ? Math.min(...xs) : null;
  };
  // Round 14 (B6): the lineage never hides a source the account was served (every fuzzy source was):
  // a source it would have covered is kept SOFT (never credited, never binding) and flagged
  // (origin_suspect, license_soft), so a walk to a served stem is always seen.
  const firstNoLineage = (i: number) => {
    const xs = [firstOf(i), ownFuzzy.get(i)].filter((x): x is number => typeof x === "number" && x > 0);
    return xs.length ? Math.min(...xs) : null;
  };
  const fuzzy = await fuzzyHits(
    c,
    me,
    mh.map((h, i) => ({ pos: i, mh: h, tokens: tokens[i]!, bands: bands[i]!, first: firstNoLineage(i), nfull: subjects[i]!.near, nbare: subjects[i]!.nearBare, noexpl: subjects[i]!.noExpl, body: bodyToks[i]! })),
    // (Round 21: never for a question with a source matched on itself; independence lifts near-only ones.)
    { mine: [...actx.mine], independent: m.independent, exact: [...ev.byPos].filter(([, l]) => l.length > 0).map(([k]) => k), reserve: lend },
  );
  const fuzzyPos = new Set<number>();
  // Past the fuzzy caps: every question that had candidates is flagged for the admin (conservative).
  // Round 21 (critic27a M-B1, critic27b B-2): sources left out past an allowance never decide by being
  // left out: a question is refused only if one of them forbids the licence chosen (bound_by names it);
  // otherwise it is published with a placeholder link carrying their licences, and flagged.
  const cutRefused = new Map<number, string>();
  if (fuzzy.capped) bands.forEach((bs, i) => bs.length > 0 && !ev.overflow.has(i) && ev.overflow.set(i, FUZZY_ROWS_MAX));
  const lineageSoft = new Set<number>();
  // Round 17 (M3): a near stem with another body: the admin is told (origin_suspect), nothing else.
  for (const k of fuzzy.stemOnly) fuzzyPos.add(k);
  for (const h of fuzzy.hits) {
    const ls = lineage[h.pos];
    if (ls !== null && ls !== undefined && h.source_first_seq >= ls) {
      // Never credited or binding on the author's own lineage; the admin is told (flags below).
      lineageSoft.add(h.pos);
      continue;
    }
    const list = ev.byPos.get(h.pos) ?? [];
    if (list.some((x) => x.item_id === h.item_id)) continue;
    ev.byPos.set(h.pos, [...list, h]);
    fuzzyPos.add(h.pos);
  }
  const allHits = [...ev.byPos.values()].flat();
  // Kept provenance whose record was voided is void, never "a record that is merely gone" (F3a);
  // chain links to the publisher's own items or to void items are dropped (F3b, diamond imports).
  const voidRefs = await voidedRefs(c, me, carriedRefs);
  const vi = await voidInfo(c, [
    ...allHits.map((h) => h.item_id),
    ...[...ev.credits.values()].flatMap((x) => x.links.map((l) => l.item_id)),
    ...[...prev.values()].flatMap((x) => [x.origin_item_id ?? "", ...(JSON.parse(x.chain_json) as ChainLink[]).map((l) => l.item_id)]),
  ]);
  const drop = new Set([...actx.mine, ...vi.voided]);
  // Void links the admin tied to the rightful author's item stand for that item (round 7 M1).
  const replaced = vi.replaced;
  const ownItems = await ownItemSet(c, me, declared.filter((d) => d !== null).map((d) => d!.item));
  const incompatible: number[] = [];
  const conflicts: number[] = [];
  const grandfathered: number[] = [];
  const overflowed: number[] = [];
  const softOnly: number[] = [];
  // Round 18 U3: positions whose HIDDEN-only text (served to this account elsewhere) forbids the licence:
  // refused to the author alone; nothing else depends on it.
  const hiddenRefused: number[] = [];
  // ... and the items it was shown that material in, with the licences it was shown (variant b).
  const hiddenFrom = new Map<string, string[]>();
  const binders: string[][] = [];
  const perQuestion: License[][] = [];
  const unverified: { pos: number; item: string }[] = [];
  const declaredModified: { pos: number; item: string }[] = [];
  const derived: { chain: ChainLink[]; originItem: string | null; originQid: string | null; modified: boolean | null }[] = [];
  // What the rule allowed per question and which items decided it (round 11 P5), stored with it.
  const allowedJson: string[] = [];
  const trueBound: { via: string; link: ChainLink }[][] = [];
  // What readers are told about each question beyond its recorded sources (round 14): an
  // unverified soft source (B8), an attested-independent possible source (B5), an earlier identical
  // publication by someone else with no evidence of a serve (B3).
  const marks: { soft?: boolean; claimed?: boolean; earlier?: boolean }[] = qids.map(() => ({}));
  const byKey = (a: ImportHit, b: ImportHit) => (a.item_id + a.qid < b.item_id + b.qid ? -1 : 1);
  for (let i = 0; i < n; i++) {
    const d = declared[i];
    // Every source here is admissible already (SQL); only the content comparisons remain.
    const found = ev.byPos.get(i) ?? [];
    const hits = found.filter((h) => h.by_match);
    const dHit = d ? found.find((h) => h.by_ref && h.item_id === d.item && h.versions.includes(d.version)) ?? null : null;
    if (d && !dHit && !hits.some((h) => h.item_id === d.item) && !ownItems.has(d.item)) unverified.push({ pos: i, item: ID_RE.test(d.item) ? d.item : "" });
    // A declared origin whose content differs from what was imported is kept (an edit) but the admin sees it.
    if (dHit && !dHit.kf_same) declaredModified.push({ pos: i, item: dHit.item_id });
    const pv = prev.get(qids[i]!);
    const carried = pv && pv.origin_item_id && pv.origin_qid ? found.find((h) => h.by_ref && h.item_id === pv.origin_item_id) ?? null : null;
    const seen = new Set<string>();
    const sources0 = [...hits, ...(dHit ? [dHit] : []), ...(carried ? [carried] : [])].filter((h) => (seen.has(h.item_id) ? false : (seen.add(h.item_id), true)));
    // Round 16 (critic21c c4): a near-rewording found only by its stem is no credit when the question
    // has a source matched on the question itself: it is flagged (origin_suspect), not credited.
    const sources = sources0.some((h) => !h.fuzzy) ? sources0.filter((h) => !h.fuzzy) : sources0;
    if (ev.overflow.has(i)) overflowed.push(i);
    // (Round 21: the sources left out of this question: the exact ones matter only without a copy kept,
    // copies being kept earliest first; the near ones always.)
    const cutHere = [...(found.some((h) => h.near && !h.fuzzy) ? [] : [ev.cut.get(i)]), fuzzy.cut.get(i)].filter((x): x is FuzzyCut => x !== undefined);
    // THE licence rule (round 9, license.ts decideLicence): a copy of a source in the licence sense
    // (read or imported) and an adaptation of an import or of a declared / carried origin are hard;
    // identical sources are united, distinct materials intersected, a contradiction never locks out.
    // A changed copy of something only read is soft: credited, flagged, never a refusal.
    const hardOf = (h: ImportHit) => isHard(h, h === dHit);
    const units: LicenceUnit[] = sources.filter(hardOf).map(unitOf);
    const softUnits = sources.filter((h) => !hardOf(h)).map(unitOf);
    let entry: (typeof derived)[number];
    if (sources.length > 0) {
      // Attribution: the identical sources (visible.ts: key-free when hidden) are all credited, in the
      // order they were first published; with none identical, every candidate is merged. Never by
      // version numbers across items, never by import order, never by the hidden key. The merged
      // credits come from SQL (round 9 J6), so no contributor drops out of a big bank.
      const sorted = [...sources].sort((a, b) => a.source_first_seq - b.source_first_seq || byKey(a, b));
      const identical = sorted.filter((h) => h.same);
      const pool = identical.length > 0 ? identical : sorted;
      const src = pool.reduce((a, b) => (b.chain_len > a.chain_len ? b : a));
      const cred = ev.credits.get(i);
      let links = cred?.links ?? [];
      // Round 10 M-7: the dropped middle is one placeholder between the originators and the most recent.
      if (cred && cred.total > links.length)
        links = [...links.slice(0, CREDITS_HEAD), { author_name: `and ${cred.missing || cred.total - links.length} contributors between`, item_id: "", qid: "", version: 0, license: "", modified: true, collapsed_licenses: cred.more }, ...links.slice(CREDITS_HEAD)];
      let chain = chainOfLinks(links, drop, replaced);
      // The direct source is always credited.
      if (!chain.some((l) => l.item_id === src.item_id)) chain.push({ author_name: src.author_name, item_id: src.item_id, qid: src.qid, version: src.version, license: src.license, modified: !src.same });
      for (const h of sorted) if (h.fuzzy && !chain.some((l) => l.item_id === h.item_id)) chain.push({ author_name: h.author_name, item_id: h.item_id, qid: h.qid, version: h.version, license: h.license, modified: true });
      // Round 14 (B5): a source the author attested independence of (a short or generic read) stays
      // credited, as a possible source; it binds nothing.
      for (const h of sorted) if (h.claimed) for (const l of chain) if (l.item_id === h.item_id) l.possible_source = true;
      chain = collapseChain(chain, STORED_CHAIN_MAX);
      entry = { chain, originItem: src.item_id, originQid: src.qid, modified: !src.same };
    } else if (pv && pv.chain_json !== "[]" && !(pv.origin_item_id && pv.origin_qid && voidRefs.has(`${pv.origin_item_id}/${pv.origin_qid}`)) && chainOf(pv, drop, replaced).length > 0) {
      // Provenance whose import record is gone: kept as it was, and an edit counts as a modification.
      // Key-free comparison whenever either version hides its key: a public v2 must not reveal
      // whether a hidden v1's key differed (critic round 3, latent leak). A VOIDED record is not
      // "gone": its provenance is dropped (F3a), and so are links to void or own items. Carried
      // provenance is explicit lineage: hard.
      const chain = chainOf(pv, drop, replaced);
      const last = chain[chain.length - 1];
      const changed = m.hideKey || prevHidden ? pv.keyfree_content !== keyfree[i] : pv.full_content !== fullContent[i];
      if (last && changed) last.modified = true;
      units.push({
        item: last?.item_id || "carried",
        verbatim: !(last?.modified ?? true),
        contents: [],
        chain: chain.slice(0, -1).flatMap((l) => [...(l.license ? [l.license] : []), ...(l.collapsed_licenses ?? [])]),
        own: last && last.license ? [[last.license]] : [],
        seq: 0,
      });
      entry = { chain, originItem: pv.origin_item_id, originQid: pv.origin_qid, modified: last ? last.modified : null };
    } else entry = { chain: [], originItem: null, originQid: null, modified: null };
    // Units used besides the question itself (rounds 16-18): see `lifts` above.
    const hiddenUnits: LicenceUnit[] = [];
    for (const l of lifts.get(i) ?? []) {
      if (sources.some((h) => h.item_id === l.item_id && hardOf(h))) continue;
      const upstream = (JSON.parse(l.chain) as ChainLink[]).flatMap((x) => [...(x.license ? [x.license] : []), ...(x.collapsed_licenses ?? [])]);
      // Bound by the licences the account RECEIVED (round 18 U2), never the source's current one.
      const unit: LicenceUnit = { item: l.item_id, verbatim: false, contents: [`unit:${l.key}`], chain: upstream, own: [l.received.length ? l.received : [l.license]], seq: l.seq };
      if (l.hidden) {
        // Round 18 U3: a hidden-only text (not served by this publish) may bind its author privately
        // (a refusal only she sees, from material she was served), never anything others observe.
        if (l.served) hiddenUnits.push(unit);
        continue;
      }
      const known = entry.chain.some((x) => x.item_id === l.item_id);
      const link: ChainLink = { author_name: l.author_name, item_id: l.item_id, qid: l.qid, version: l.version, license: l.license, modified: true, kind: l.kind };
      if (!l.served) {
        // Never served: an earlier public text, image or question unit (a question wrapped as a part,
        // or a part taken out) is credited as an earlier publication (unverified, flagged, no refusal).
        if (!l.visible) continue;
        if (!known) entry.chain = [...entry.chain, { ...link, earlier_match: true }];
        marks[i]!.earlier = true;
        if (!softOnly.includes(i)) softOnly.push(i);
        continue;
      }
      const claimed = m.independent.includes(l.item_id);
      // Round 18 U5: a stem-only question unit (a Tf / Short / Blank prompt) READ is soft, like a
      // changed copy of a read question: credited, flagged, never binding.
      if (l.kind === "part" && l.stemOnly && l.evKind === "view") {
        if (!known) entry.chain = [...entry.chain, claimed ? { ...link, possible_source: true } : link];
        if (claimed) marks[i]!.claimed = true;
        else {
          marks[i]!.soft = true;
          if (!softOnly.includes(i)) softOnly.push(i);
        }
        continue;
      }
      // Claims (round 18): never over an IMPORTED text or image; a read one only when attributed to a
      // public source, or an image the claimant uploaded before that item was first published. Question
      // units follow the question rule: a stem-only unit (a changed copy at best) may be attested, read or
      // imported; a unit with a body only when read and short (40 words or fewer as served).
      const mayClaim =
        claimed &&
        (l.kind === "part"
          ? l.stemOnly || (l.evKind === "view" && short40[i] === 1)
          : l.evKind === "view" && (attributed || (l.kind === "image" && l.uploadedFirst)));
      if (mayClaim) {
        if (!known) entry.chain = [...entry.chain, { ...link, possible_source: true }];
        marks[i]!.claimed = true;
        continue;
      }
      // An adaptation of the source (the material goes into a new question); the same material from
      // several sources is one material (`contents`), so their grants are united.
      units.push(unit);
      if (!known) entry.chain = [...entry.chain, link];
    }
    if (hiddenUnits.length > 0 && !decideLicence(hiddenUnits).allowed.includes(m.license)) {
      hiddenRefused.push(i);
      for (const u of hiddenUnits) hiddenFrom.set(u.item, [...new Set([...(hiddenFrom.get(u.item) ?? []), ...u.own.flat()])]);
    }
    derived.push(entry);
    const dec = decideLicence(units);
    // Round 20 (critic25a E1): each link to material this publish drew on records the licence that
    // ALLOWED the use: the most permissive licence available to this account for that material (what
    // it received, or the source's licence now for the very same content) that permits this publish.
    // Downstream copies are bound by exactly that (a CC grant is irrevocable), never by an older or
    // stricter label the account did not rely on.
    if (dec.allowed.includes(m.license))
      for (const link of entry.chain) {
        if (!link.item_id || link.earlier_match || link.possible_source) continue;
        const h = sources.find((x) => x.item_id === link.item_id && hardOf(x));
        const lf = h ? undefined : (lifts.get(i) ?? []).find((x) => x.item_id === link.item_id && x.served);
        const groups = h ? unitOf(h).own : lf ? [lf.received.length ? lf.received : [lf.license]] : null;
        const relied = groups ? reliedLicence(groups, m.license, h ? !unitOf(h).verbatim : true) : null;
        if (relied) link.license = relied;
      }
    perQuestion.push(dec.allowed);
    binders.push(dec.binders);
    // Round 14 (B8): when a binder's own licence allows this one, what binds is a link of its chain
    // (an upstream licence it relabelled): name that link, not the relabel.
    trueBound.push(
      dec.allowed.includes(m.license)
        ? []
        : dec.binders.flatMap((b) => {
            const h = sources.find((x) => x.item_id === b);
            if (!h || !mayRepublish(h.license, m.license)) return [];
            return entry.chain.filter((l) => l.item_id !== "" && l.item_id !== b && l.license && !mayRepublish(l.license, m.license)).map((l) => ({ via: b, link: l }));
          }),
    );
    // A republish never refuses a question its author did not touch (round 9 J2): same qid, same
    // content (key-free whenever either version hides its key, so no outcome depends on a hidden
    // key) and the same licence keep the outcome their earlier publish had; the admin is told.
    const untouched = pv !== undefined && prevLicense === m.license && (m.hideKey || prevHidden ? pv.keyfree_content === keyfree[i] : pv.full_content === fullContent[i]);
    allowedJson.push(untouched && pv.allowed_json ? pv.allowed_json : JSON.stringify({ allowed: dec.allowed, binders: dec.binders }));
    if (cutHere.length > 0 && !untouched) {
      const forbidsLic = (l: string) => !grantOptions({ item: "", license: l, modified: true }).includes(m.license as License);
      const forbids = cutHere.find((cx) => cx.lics.some(forbidsLic));
      if (forbids) {
        const lic0 = forbids.lics.find(forbidsLic)!;
        const item0 = forbids.by?.[lic0] ?? forbids.item;
        cutRefused.set(i, item0);
        binders[i] = [...new Set([...(binders[i] ?? []), item0])];
      } else {
        const n0 = cutHere.reduce((acc, cx) => acc + cx.n, 0);
        entry.chain.push({ author_name: `and ${n0} earlier similar sources`, item_id: "", qid: "", version: 0, license: "", modified: true, collapsed_licenses: [...new Set(cutHere.flatMap((cx) => cx.lics))] });
        fuzzyPos.add(i);
      }
    }
    if (sources.some((h) => h.claimed)) marks[i]!.claimed = true;
    if (!dec.allowed.includes(m.license) || cutRefused.has(i)) (untouched ? grandfathered : incompatible).push(i);
    else {
      if (dec.conflict) conflicts.push(i);
      // The soft flag (round 9): only an adaptation (no source is the same question) with no
      // declared origin, of something read whose terms do not allow the licence.
      if (!d && !sources.some((h) => h.near) && softUnits.some((u) => !unitAllows(u).allowed.has(m.license))) {
        softOnly.push(i);
        // Round 14 (B8): readers are told the licence is unverified for it.
        marks[i]!.soft = true;
      }
    }
  }
  // Round 14 (B3): a question that credits nobody but equals (licence sense) what another author
  // published, visibly, earlier: credited as "earlier identical publication" (it binds downstream
  // copies like any link, never this publish), and readers see license_check unverified_earlier_match.
  // Public data only; short statements and common questions are left alone.
  {
    // Round 16 (critic21c B2): compared in every served form, never through a hidden key: this
    // publish's served full (shown) or stripped (hidden) content, its key-free content and its content
    // without the explanation, against a source's shown full, stripped, key-free and bare forms.
    const mine = (i: number) => [...new Set([st ? nearStripped?.[i] : nearFull[i], nearKf[i], st ? bareStripped?.[i] : bareFull[i]].filter((x): x is string => typeof x === "string"))];
    // "Earlier" than the publisher's own first publish of this question (its match_hash, its content,
    // its lineage or its own near-rewordings).
    const earlyFirst = (i: number) => {
      const xs = [fuzzyFirst(i), actx.firstAt.get(mh[i]!)].filter((x): x is number => typeof x === "number" && x > 0);
      return xs.length ? Math.min(...xs) : null;
    };
    // Round 26 (critic-moodle-33c B-2): her earliest own version with the same choices even when this
    // one ADDS an explanation or answer: what counts against a source that lacks the addition.
    const looseFirst = (i: number) => minFirst(earlyFirst(i), ownLoose.get(i)?.seq ?? null);
    const want = derived
      .map((d0, i) => ({ i, n: mine(i), first: earlyFirst(i), loose: looseFirst(i), expl: bkFields[i]!.expl, ans: bkFields[i]!.ans, lexpl: ownLoose.get(i)?.expl ?? null, lans: ownLoose.get(i)?.ans ?? null }))
      .filter((x) => derived[x.i]!.chain.length === 0 && !shortQ[x.i] && !actx.common.has(mh[x.i]!) && x.n.length > 0);
    // (Round 21: every question, in chunks, never only the first 1,000.)
    // Round 30: candidates are QUESTIONS, never whole publishes. Round 31: each near hash is followed once
    // per chunk (EARLIER_FIRSTS_SQL), and a position's test runs on its few candidates (EARLIER_SQL).
    for (const wch of chunks(want, 1000)) {
      const r = await c.db
        .prepare(EARLIER_SQL)
        .bind(me, JSON.stringify(await earlierCandidates(c, me, wch)))
        .all<{ pos: number; m: string | null }>();
      for (const x of r.results) {
        if (!x.m) continue;
        const l = JSON.parse(x.m) as ChainLink;
        // An item the publisher attests independence of stays credited as a possible source (B5).
        const ind = m.independent.includes(l.item_id);
        derived[x.pos]!.chain = [{ ...l, modified: false, earlier_match: true, ...(ind ? { possible_source: true } : {}) }];
        if (ind) marks[x.pos]!.claimed = true;
        else marks[x.pos]!.earlier = true;
        if (!softOnly.includes(x.pos)) softOnly.push(x.pos);
      }
      // ... and a near-rewording of a served stem of the same kind (one added sentence, a changed
      // choice, a distractor more or less): credited the same way, marked as modified.
      const left = want.filter((x) => derived[x.i]!.chain.length === 0);
      const near = await publicNearEarlier(c, me, left.map((x) => ({ pos: x.i, tokens: tokens[x.i]!, bands: bands[x.i]!, qkind: servedKind[x.i]!, first: x.first ?? null, body: bodyToks[x.i]!, looseFirst: x.loose ?? null, looseExpl: x.lexpl, looseAns: x.lans, expl: x.expl, ans: x.ans })), lend);
      // (Round 21 B-4: a question whose earlier candidates were cut is flagged; the earliest examined credited.)
      for (const k of near.cut) if (!overflowed.includes(k)) overflowed.push(k);
      for (const [pos, l] of near.near) {
        const ind = m.independent.includes(l.item_id);
        derived[pos]!.chain = [{ item_id: l.item_id, qid: l.qid, version: l.version, license: l.license, author_name: l.author_name, modified: true, earlier_match: true, ...(ind ? { possible_source: true } : {}) } as ChainLink];
        if (ind) marks[pos]!.claimed = true;
        else marks[pos]!.earlier = true;
        if (!softOnly.includes(pos)) softOnly.push(pos);
      }
    }
  }
  if (incompatible.length === 0 && hiddenRefused.length > 0) {
    // Round 18 U3: only this author sees it (she knows her hidden key and what she was shown).
    // Round 19 (critic24c variant b): naming the items she was shown it in and their licences (an item
    // no longer listed is named only as such).
    const ids = [...hiddenFrom.keys()].filter((x) => ID_RE.test(x)).slice(0, 50);
    const shown = new Map(
      (
        await c.db
          .prepare(`SELECT i.id, i.title FROM items i JOIN users u ON u.id = i.author_id WHERE i.id IN (SELECT value FROM json_each(?1)) AND ${VISIBLE_TO_OTHERS}`)
          .bind(JSON.stringify(ids))
          .all<{ id: string; title: string }>()
      ).results.map((x) => [x.id, x.title]),
    );
    const sources = ids.map((x) => (shown.has(x) ? { item_id: x, title: shown.get(x)!, link: `/items/${x}`, licenses: hiddenFrom.get(x)! } : { item_id: null, title: "an item no longer listed", licenses: hiddenFrom.get(x)! }));
    throw new PoolError(422, "license_incompatible", {
      positions: hiddenRefused.slice(0, 100),
      hidden_material: true,
      sources,
      message: `The hidden explanation or model answer of these questions is material you were shown in ${sources.map((s) => `"${s.title}" (${s.licenses.join(", ")})`).join("; ")}, under a licence that does not allow this one.`,
    });
  }
  if (incompatible.length > 0) {
    // Which questions force which licences, and which items bind them (round 7 M4: an honest author can
    // then say "I wrote this independently" of exactly those, or split the bank).
    const named = [...new Set(incompatible.slice(0, 100).flatMap((pos) => [...(binders[pos] ?? []), ...(trueBound[pos] ?? []).map((t) => t.link.item_id)]))].filter((x) => ID_RE.test(x)).slice(0, 200);
    const info = new Map<string, { item_id: string; title: string; author_name: string }>();
    if (named.length > 0) {
      const r = await c.db
        // Only items visible to others are named (round 8): an unlisted, held or removed source is a
        // neutral entry (it still binds; its id, title and author are not shown).
        .prepare(`SELECT i.id AS item_id, i.title, COALESCE(u.display_name, 'deleted teacher') AS author_name FROM items i JOIN users u ON u.id = i.author_id WHERE i.id IN (SELECT value FROM json_each(?1)) AND ${VISIBLE_TO_OTHERS}`)
        .bind(JSON.stringify(named))
        .all<{ item_id: string; title: string; author_name: string }>();
      for (const x of r.results) info.set(x.item_id, x);
    }
    // Round 16 (critic21c c14): a binder this account was earlier SHOWN under the licence it asks for
    // (the author relicensed it since): say so, and what to do (CC grants are irrevocable: the version
    // received under that licence stays usable; a later version is bound by its own licence).
    const relicensed: { item_id: string; earlier: string[]; now: string }[] = [];
    if (named.length > 0) {
      const r = await c.db
        .prepare(
          // (Round 20 variant 1: what the account was served, from its served rows.)
          `SELECT s.item_id, json_group_array(DISTINCT l.value) AS earlier, i.license AS now FROM json_each(?2) it CROSS JOIN served s ON s.item_id = it.value AND s.reader_id = ?1
             JOIN items i ON i.id = s.item_id, json_each(s.licenses_json) l
            WHERE s.void_at IS NULL GROUP BY s.item_id`,
        )
        .bind(me, JSON.stringify(named))
        .all<{ item_id: string; earlier: string; now: string }>();
      for (const x of r.results) {
        const earlier = JSON.parse(x.earlier) as string[];
        if (earlier.includes(m.license) && x.now !== m.license && info.has(x.item_id)) relicensed.push({ item_id: x.item_id, earlier, now: x.now });
      }
    }
    throw new PoolError(422, "license_incompatible", {
      ...(relicensed.length > 0
        ? {
            relicensed: relicensed.slice(0, 20),
            message: `A source was relicensed after you received it (now ${relicensed[0]!.now}). The version you received under ${m.license} stays usable under it: keep your copy as it was, or publish this update under ${relicensed[0]!.now}.`,
          }
        : {}),
      positions: incompatible.slice(0, 100),
      allowed: allowedForBank(perQuestion),
      per_question: incompatible.slice(0, 100).map((pos) => ({
        pos,
        allowed: perQuestion[pos]!,
        bound_by: (binders[pos] ?? []).flatMap((id): Record<string, unknown>[] => {
          const up = (trueBound[pos] ?? []).filter((t) => t.via === id);
          // The upstream restriction itself (its item, author and licence), the relabel only as "via".
          if (up.length > 0) return up.map((t) => ({ ...(info.get(t.link.item_id) ?? { item_id: null, title: "", author_name: t.link.author_name }), license: t.link.license, via: info.get(id)?.item_id ?? null }));
          return [info.get(id) ?? { item_id: null, title: "", author_name: "", note: "an item that is no longer shown" }];
        }),
        ...(overflowed.includes(pos) ? { evidence_overflow: true } : {}),
      })),
    });
  }
  const suspectStems = await stemHits(c, me, stems.filter((x, k): x is string => x !== null && derived[k]!.chain.length === 0));
  // Never for a source the author's own first publish of the question precedes (round 11 P1).
  const before = (k: number, seq: number | undefined) => seq !== undefined && (fuzzyFirst(k) === null || seq < fuzzyFirst(k)!);
  const suspect = stems.map((x, k) => ((x !== null && derived[k]!.chain.length === 0 && before(k, suspectStems.get(x))) || fuzzyPos.has(k) || lineageSoft.has(k) ? k : -1)).filter((k) => k >= 0);
  for (const k of lineageSoft) if (!softOnly.includes(k)) softOnly.push(k);

  // ---- soft check, blocklist, duplicates ------------------------------------------------------------
  const mhJson = JSON.stringify([...new Set(mh)]);
  // The pool-wide soft check (spec): visible items with the same question that this account was never
  // served, under a licence that does not allow this one. Silent (the admin's queue only); round 9:
  // a match whose licence allows the publish is not noise worth a flag.
  const soft = {
    results: (
      await c.db
        .prepare(
          `SELECT DISTINCT i.id, i.license FROM item_questions iq
             JOIN items i ON i.current_publish_id = iq.publish_id JOIN users u ON u.id = i.author_id
            WHERE iq.match_hash IN (SELECT value FROM json_each(?2)) AND i.author_id <> ?1 AND ${VISIBLE_TO_OTHERS}
              AND NOT EXISTS (SELECT 1 FROM imports im WHERE im.user_id = ?1 AND im.item_id = i.id)
              -- (round 26, critic-moodle-33c V3: never a copy of this account's own question: its chain
              -- runs to one of her items, or she published that very question first)
              AND NOT EXISTS (SELECT 1 FROM json_each(COALESCE(iq.chain_json, '[]')) cl JOIN items mi ON mi.id = cl.value ->> '$.item_id' WHERE mi.author_id = ?1)
              AND NOT EXISTS (SELECT 1 FROM author_firsts af WHERE af.author_id = ?1 AND af.match_hash = iq.match_hash AND af.first_seq <= iq.seq)
              -- (round 20 F1: the licence test before the cut, so the first 50 listed are offenders)
              AND i.license NOT IN (SELECT value FROM json_each(?3)) LIMIT 50`,
        )
        .bind(me, mhJson, JSON.stringify(LICENSES.filter((l) => l === m.license || mayRepublish(l, m.license))))
        .all<{ id: string; license: string }>()
    ).results,
  };
  // Holds that follow content (none is ever a refusal: common questions would give false positives):
  // privacy blocks (student data, critic C2), DMCA re-upload blocks, and a hold on one of this
  // author's own items with the same questions (a held item deleted and published again, critic X5).
  // Images only in the full form of a hidden publish are never checked against blocks: a hit on
  // material nobody else is served must not create state its author can see (round 5 E4).
  const servedShas = [...new Set(imgAt.filter((x): x is string => x !== null))];
  // An image whose privacy block's grace window ended (bytes possibly gone) is usable only where it
  // is NOT served, and only by an author who already uses it there: no served use, and no other
  // caller, can depend on whether its bytes were kept (round 6 F1).
  const purgedServed = shas.filter((s) => blobInfo.get(s)!.privacy_purged_at !== null);
  if (purgedServed.length > 0) {
    const ownUnserved = new Set(
      (
        await c.db
          .prepare(
            `SELECT DISTINCT ub.sha256 FROM item_blobs ub JOIN item_publishes up ON up.publish_id = ub.publish_id
              WHERE up.author_id = ?1 AND up.status IN ('current', 'previous') AND ub.form = 'full' AND up.hide_key = 1 AND ub.sha256 IN (SELECT value FROM json_each(?2))
                AND NOT EXISTS (SELECT 1 FROM item_blobs us WHERE us.publish_id = ub.publish_id AND us.form = 'stripped' AND us.sha256 = ub.sha256)`,
          )
          .bind(me, JSON.stringify(purgedServed))
          .all<{ sha256: string }>()
      ).results.map((r) => r.sha256),
    );
    const served = new Set(servedShas);
    // Round 16 (critic21d 5): an image still privacy-blocked is held at commit (student_data), never
    // refused here: a refusal would confirm the block to the uploader.
    const stillBlocked = new Set(
      (await c.db.prepare(`SELECT hash FROM privacy_blocks WHERE kind = 'blob' AND hash IN (SELECT value FROM json_each(?1))`).bind(JSON.stringify(purgedServed)).all<{ hash: string }>()).results.map((x) => x.hash),
    );
    const gone = purgedServed.filter((s) => !stillBlocked.has(s) && (served.has(s) || !ownUnserved.has(s)));
    for (const s of purgedServed) if (!gone.includes(s) && !stillBlocked.has(s) && !(await c.r2.head(`blobs/${s}`))) gone.push(s);
    if (gone.length > 0) throw new PoolError(422, "blob_missing", { reason: "reupload", positions: fullImg.map((s, i) => (gone.includes(s ?? "") ? i : -1)).filter((i) => i >= 0).slice(0, 100) });
  }
  const blocked = await c.db
    .prepare(
      `SELECT kind, hash, action_id FROM blocked_hashes WHERE lifted_at IS NULL AND (expires_at IS NULL OR expires_at > ?3)
         AND ((kind = 'match' AND hash IN (SELECT value FROM json_each(?1))) OR (kind = 'blob' AND hash IN (SELECT value FROM json_each(?2)))) LIMIT 1`,
    )
    .bind(JSON.stringify(blockKeys), JSON.stringify(servedShas), now)
    .first<{ kind: string; hash: string; action_id: string | null }>();
  // Round 16 (critic21c B4): matched by the normalised skeleton and full content too (a case or
  // punctuation change is the same question), and through the keys a held item left when it was
  // deleted (carried_keys, until the reports are resolved or CARRY_KEYS_MS passes). Critic21d (4):
  // never a common or short question, and only a hold an admin placed or that two or more distinct
  // reporters stand behind (one reporter never holds more than the item reported).
  const carryKeys = JSON.stringify([...new Set(mh.flatMap((h, i) => (shortQ[i] || actx.common.has(h) ? [] : [h, bkNorm[i], bkFull[i]])).filter((x): x is string => typeof x === "string"))]);
  const carried = await c.db
    .prepare(
      `SELECT reason, item_id FROM (
         SELECT h.reason, h.item_id FROM item_holds h JOIN items i ON i.id = h.item_id AND i.author_id = ?1 AND i.purged_at IS NULL
          WHERE h.item_id <> ?2 AND h.source IN ('', 'manual', 'key_leak') -- human judgements (and key leaks, round 19); content blocks are re-checked anyway
            AND (h.by IS NOT NULL OR (SELECT COUNT(DISTINCT r.reporter_id) FROM reports r WHERE r.item_id = h.item_id AND r.resolved_at IS NULL) >= 2)
            AND EXISTS (SELECT 1 FROM item_publishes p JOIN item_questions iq ON iq.publish_id = p.publish_id
                         WHERE p.item_id = h.item_id AND (iq.match_hash IN (SELECT value FROM json_each(?3)) OR iq.bk_norm IN (SELECT value FROM json_each(?3)) OR iq.bk_full IN (SELECT value FROM json_each(?3))))
         UNION ALL
         SELECT ck.reason, ck.item_id FROM carried_keys ck
          WHERE ck.author_id = ?1 AND ck.item_id <> ?2 AND ck.until > ?4 AND ck.key IN (SELECT value FROM json_each(?3)))
       ORDER BY reason LIMIT 1`,
    )
    .bind(me, itemId, carryKeys, c.now())
    .first<{ reason: string; item_id: string }>();
  let possibleDuplicate: string | null = null;
  if (isNew) {
    const dup = await c.db
      .prepare(
        `SELECT i.id FROM items i JOIN item_questions iq ON iq.publish_id = i.current_publish_id
          WHERE i.author_id = ?1 AND i.removed_at IS NULL
          GROUP BY i.id HAVING COUNT(*) = ?2 AND SUM(iq.match_hash IN (SELECT value FROM json_each(?3))) = ?2 LIMIT 1`,
      )
      .bind(me, n, mhJson)
      .first<{ id: string }>();
    possibleDuplicate = dup?.id ?? null;
  }

  // ---- stage ---------------------------------------------------------------------------------------
  // (The payload trees went before the evidence lookups, round 29.) The isolate's heavy slot is handed
  // to the next publish.
  heavy.release();
  await refresh();
  const fullKey = `items/${itemId}/${newId()}.json`;
  const strippedKey = st ? `items/${itemId}/${newId()}.json` : null;
  const fullBytes = full.bytes.length;
  const strippedBytes = reused ? reused.bytes.length : st ? st.bytes.length : 0;
  const charge = fullBytes + strippedBytes + (isNew ? 1024 : 0);
  const quota = await limitOf(c, me, "storage_bytes");
  const title = meta.title || bankName;
  const acks = JSON.stringify({ labels: st ? st.labelFindings : [], order_acks: m.orderAcks, image_ok: m.imageOk, ack_formatting: m.ackFormatting, order_reasons: st ? st.orderReasons : [] });
  // Tags (with the merged topics) are never silently dropped: too many is an error.
  if (fullTags.size > MAX_TAGS) throw new PoolError(422, "too_many_tags", { limit: MAX_TAGS, message: "Too many distinct tags and topics in this item." });
  const sharedCap = await limitOf(c, me, "shared_questions");
  const removedCap = await limitOf(c, me, "removed_bytes");
  // Round 18 (critic23d V4): the account's durable records (what keeps it "first" and its own units)
  // are bounded: a publish that would pass author_records is refused, plainly.
  // Round 19 (critic24c C4): only NEW records count (a republish that adds nothing new never does), and
  // never for a fix of a held item.
  {
    const held = !isNew && (await c.db.prepare(`SELECT 1 AS x FROM item_holds WHERE item_id = ?1 LIMIT 1`).bind(itemId).first()) !== null;
    {
      // (Critic24d D6: exactly the records the commit writes, as (kind, hash, match_hash), less those kept.)
      const recKeys = new Map<string, [string, string, string]>();
      const add = (k: string, h: string | null | undefined, x: string) => h && recKeys.set(`${k}|${h}|${x}`, [k, h, x]);
      mh.forEach((h, i) => {
        add("nshown", m.hideKey ? nearStripped?.[i] : nearFull[i], h);
        add("kf", nearKf[i], h);
        if (!m.hideKey) add("shown", fullContent[i], h);
      });
      for (const u of unitsAt.flat()) add("unit", u.key, "");
      const r0 = await c.db
        .prepare(
          `SELECT (SELECT COUNT(*) FROM author_contents WHERE author_id = ?1) AS recs,
                  (SELECT COUNT(*) FROM json_each(?2) k
                    WHERE NOT EXISTS (SELECT 1 FROM author_contents ac WHERE ac.author_id = ?1 AND ac.kind = k.value ->> '$[0]' AND ac.hash = k.value ->> '$[1]' AND ac.match_hash = k.value ->> '$[2]')) AS fresh`,
        )
        .bind(me, JSON.stringify([...recKeys.values()]))
        .first<{ recs: number; fresh: number }>();
      const cap = await limitOf(c, me, "author_records");
      // (Round 20 variant 3: a fix of a held item is exempt up to HELD_FIX_RECORDS new records.)
      if (r0 && r0.fresh > 0 && r0.recs + r0.fresh > cap && !(held && r0.fresh <= HELD_FIX_RECORDS))
        throw new PoolError(413, "author_records_exceeded", { used: r0.recs, limit: cap, message: "Your account's record of the questions it has published is full. Republishing your existing items still works; ask an admin to raise the limit (author_records) to publish new questions." });
    }
  }
  if (dry) {
    // ---- the dry run ends here: every refusal a publish would give has been given (the quotas are
    // checked as the staging INSERT would), nothing is stored, nothing about holds is said.
    const used = await usage(c, me);
    if (used + charge > quota) throw new PoolError(413, "quota_exceeded", { used, needed: charge, limit: quota });
    const removed = (await c.db.prepare(`SELECT ${REMOVED_SQL} AS n`).bind(me).first<{ n: number }>())?.n ?? 0;
    if (removed > removedCap)
      throw new PoolError(413, "removed_storage_exceeded", { used: removed, limit: removedCap, message: "Deleted items still hold storage until they are purged (14 days). Undo a deletion or wait." });
    const qn = (await c.db.prepare(`SELECT ${SHARED_SQL("?1", "?2")} AS n`).bind(me, itemId).first<{ n: number }>())?.n ?? 0;
    if (qn + n > sharedCap) throw new PoolError(422, "shared_questions_limit", { used: qn, needed: n, limit: sharedCap });
    const servedText = reused ? new TextDecoder().decode(reused.bytes) : st ? new TextDecoder().decode(st.bytes) : null;
    const lc = licenseCheck(m.license, derived.map((d) => d.chain), allowedJson.map((a, i) => storedAllowed(withMarks(a, marks[i]!))));
    return json(200, {
      ok: true,
      dry_run: true,
      hide_key: m.hideKey,
      // What students are served: the exact stripped payload of a hidden-key publish (the shuffle of
      // a real publish is drawn again: the orders are never returned here); otherwise the full form.
      student_view: servedText !== null ? JSON.parse(servedText) : { questions: n, key_shown: true },
      notes: full.dropped,
      label_findings: st ? st.labelFindings : [],
      labels_removed: st ? st.labelsRemoved : [],
      images_removed: st ? st.imagesRemoved : [],
      figures_removed: st ? st.figuresRemoved : [],
      order_reasons: st ? st.orderReasons : [],
      license_check: lc,
      chains: derived.map((d) => d.chain),
    });
  }
  // Charged only now: a payload refused by any check above does not use up the day's publishes.
  await requireDaily(c, me, "publishes");
  // The publish sequence (round 6 F2): one value per publish, strictly increasing (ties between
  // concurrent publishes are harmless: neither then counts as having copied the other). A kept qid
  // keeps its seq only while its content (match_hash) is unchanged; an image keeps the seq of the
  // first publish of THIS item that served it.
  // The counter (round 7 H2): one atomic increment, never a MAX()+1 over rows that can be deleted.
  const seqRow = await c.db.prepare(`UPDATE pool_seq SET n = n + 1 WHERE id = 1 RETURNING n`).first<{ n: number }>();
  const newSeq = seqRow?.n ?? 1;
  const qSeq = qids.map((q, i) => {
    const pv = prev.get(q);
    return pv && pv.match_hash === mh[i] && pv.seq > 0 ? pv.seq : newSeq;
  });
  // Round 22 (critic29b B1): a TEXT unit (an explanation, a Short sample) has its OWN seq: carried only
  // when this very key was in the previous publish of the qid, else this publish's. The question's seq
  // (its match_hash, which leaves those texts out) never dates a text a republish adds.
  const unitSeqs = qids.map((q, i) => {
    const pv = prev.get(q);
    let was: Record<string, number> = {};
    try {
      was = pv?.unit_seqs ? (JSON.parse(pv.unit_seqs) as Record<string, number>) : {};
    } catch {}
    return Object.fromEntries([...servedText[i]!, ...hiddenText[i]!].map((k) => [k, typeof was[k] === "number" && was[k]! > 0 ? was[k]! : newSeq]));
  });
  // (Round 17 M2) every unit key of each question, with its seq (a text's own; the rest the question's).
  const unitRecord = unitsAt.flatMap((us, i) => us.map((u) => ({ k: u.key, s: u.kind === "text" ? (unitSeqs[i]![u.key] ?? newSeq) : qSeq[i] })));
  const blobCarry = new Map<string, number>();
  if (baseOf !== null) {
    const r = await c.db
      .prepare(`SELECT ib.sha256, MIN(ib.seq) AS seq FROM item_blobs ib JOIN item_publishes p ON p.publish_id = ib.publish_id WHERE ib.publish_id = ?1 AND ${SERVED_FORM()} GROUP BY ib.sha256`)
      .bind(baseOf)
      .all<{ sha256: string; seq: number }>();
    for (const x of r.results) if (x.seq > 0) blobCarry.set(x.sha256, x.seq);
  }
  const servedSet = new Set(servedShas);
  const blobSeq = (sha: string) => (servedSet.has(sha) ? blobCarry.get(sha) ?? newSeq : newSeq);
  const token = newId();
  // Quota, the shared-question cap (counting other items' in-flight publishes) and the id reservation
  // are all in this one INSERT, so concurrent publishes cannot race past them (critic A7).
  const staged = await c.db
    .prepare(
      `INSERT OR IGNORE INTO item_publishes (publish_id, item_id, author_id, base_publish_id, version, status, schema_version, hide_key, license,
         attested_at, attest_text_rev, acks_json, request_sha256, full_key, stripped_key, full_bytes, stripped_bytes,
         title, subject, course, grade, kinds_mask, question_count, has_images, created_at, stage_token, public_version)
       SELECT ?3, ?4, ?1, ?5, ?6, 'staging', ?7, ?8, ?9, ?2, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18, ?19, ?20, ?21, ?22, ?23, ?2, ?26, ?29
        WHERE ${USAGE_SQL} + ?24 <= ?25 AND NOT EXISTS (SELECT 1 FROM retired_publish_ids WHERE publish_id = ?3)
          AND ${SHARED_SQL("?1", "?4")} + ?22 <= ?27 AND ${REMOVED_SQL} <= ?28`,
    )
    .bind(
      me,
      now,
      m.publishId,
      itemId,
      m.base,
      version,
      schemaVersion,
      m.hideKey ? 1 : 0,
      m.license,
      m.attestRev,
      acks,
      requestSha,
      fullKey,
      strippedKey,
      fullBytes,
      strippedBytes,
      title,
      meta.subject,
      meta.course,
      meta.grade,
      kindsMask,
      n,
      hasImages,
      charge,
      quota,
      token,
      sharedCap,
      removedCap,
      publicVersion,
    )
    .run();
  if ((staged.meta.changes ?? 0) !== 1) {
    const again = await c.db.prepare(`SELECT 1 AS x FROM item_publishes WHERE publish_id = ?1`).bind(m.publishId).first();
    if (again) throw new PoolError(409, "publish_in_progress", {}, { "Retry-After": "5" });
    const used = await usage(c, me);
    if (used + charge > quota) throw new PoolError(413, "quota_exceeded", { used, needed: charge, limit: quota });
    const removed = (await c.db.prepare(`SELECT ${REMOVED_SQL} AS n`).bind(me).first<{ n: number }>())?.n ?? 0;
    if (removed > removedCap)
      throw new PoolError(413, "removed_storage_exceeded", { used: removed, limit: removedCap, message: "Deleted items still hold storage until they are purged (14 days). Undo a deletion or wait." });
    const q = await c.db.prepare(`SELECT ${SHARED_SQL("?1", "?2")} AS n`).bind(me, itemId).first<{ n: number }>();
    throw new PoolError(422, "shared_questions_limit", { used: q?.n ?? 0, needed: n, limit: sharedCap });
  }
  const mine = `EXISTS (SELECT 1 FROM item_publishes WHERE publish_id = ?1 AND stage_token = ?3 AND status = 'staging')`;
  const stagedRow = { publish_id: m.publishId, item_id: itemId, full_key: fullKey, stripped_key: strippedKey, created_at: now };
  try {
    await c.r2.put(fullKey, full.bytes, { httpMetadata: { contentType: "application/json" } });
    if (st && strippedKey) await c.r2.put(strippedKey, reused ? reused.bytes : st.bytes, { httpMetadata: { contentType: "application/json" } });

    const qRows = lite.map((q, i) => ({
      qid: qids[i],
      pos: i,
      qkind: q.kind,
      difficulty: q.difficulty,
      mh: mh[i],
      fh: fh[i],
      sh: reused ? reused.hashes[i] : st ? st.strippedHashes[i] : null,
      fc: fullContent[i],
      kf: keyfree[i],
      st: stems[i],
      sc: strippedContent ? strippedContent[i] : null,
      nf: nearFull[i],
      ns: nearStripped ? nearStripped[i] : null,
      bf: bareFull[i],
      sh2: shortQ[i] ? 1 : 0,
      tk: JSON.stringify(tokens[i]),
      bd: JSON.stringify(bands[i]),
      bs: bareStripped ? bareStripped[i] : null,
      nk: nearKf[i],
      chain: JSON.stringify(derived[i]!.chain),
      ls: lineage[i] ?? qSeq[i],
      aj: withMarks(allowedJson[i]!, marks[i]!),
      bh: body[i],
      kn: bkNorm[i],
      ks: bkStem[i],
      lr: JSON.stringify(lineage[i] !== null ? lineageRoot[i] : tokens[i]),
      bt: JSON.stringify(bodyToks[i]),
      tsk: JSON.stringify(tokSk[i]),
      bsk: JSON.stringify(bodySk[i]),
      kb: JSON.stringify(kBands[i]),
      kfl: bkFull[i],
      ke: bkFields[i]!.expl,
      ka: bkFields[i]!.ans,
      oi: derived[i]!.originItem,
      oq: derived[i]!.originQid,
      mod: derived[i]!.modified === null ? null : derived[i]!.modified ? 1 : 0,
      seq: qSeq[i],
      us: JSON.stringify(unitSeqs[i]),
    }));
    const tags = fullTags;
    const standards = new Set(meta.standards.map((s) => s.replace(/\s+/g, " ").trim().toUpperCase()).filter(Boolean));
    const stmts: D1PreparedStatement[] = [];
    for (const ch of chunks(qRows, 150))
      stmts.push(
        c.db
          .prepare(
            `INSERT INTO item_questions (publish_id, qid, pos, qkind, difficulty, match_hash, full_sha256, stripped_sha256, full_content, stripped_content, keyfree_content, near_full, near_stripped, near_kf, near_bare_full, near_bare_stripped, short, stem_tokens, stem_bands, stem_hash, chain_json, origin_item_id, origin_qid, modified, seq, lineage_seq, allowed_json, body_hash, bk_norm, bk_full, bk_expl, bk_ans, bk_stem, lineage_root, body_tokens, tok_sk, body_sk, kbands, unit_seqs)
             SELECT ?1, j.value ->> '$.qid', j.value ->> '$.pos', j.value ->> '$.qkind', j.value ->> '$.difficulty', j.value ->> '$.mh', j.value ->> '$.fh',
                    j.value ->> '$.sh', j.value ->> '$.fc', j.value ->> '$.sc', j.value ->> '$.kf', j.value ->> '$.nf', j.value ->> '$.ns', j.value ->> '$.nk', j.value ->> '$.bf', j.value ->> '$.bs', j.value ->> '$.sh2', j.value ->> '$.tk', j.value ->> '$.bd', j.value ->> '$.st', ${anonChain("j.value ->> '$.chain'")}, j.value ->> '$.oi', j.value ->> '$.oq', j.value ->> '$.mod', j.value ->> '$.seq', j.value ->> '$.ls', j.value ->> '$.aj', j.value ->> '$.bh', j.value ->> '$.kn', j.value ->> '$.kfl', j.value ->> '$.ke', j.value ->> '$.ka', j.value ->> '$.ks', j.value ->> '$.lr', j.value ->> '$.bt', j.value ->> '$.tsk', j.value ->> '$.bsk', j.value ->> '$.kb', j.value ->> '$.us'
               FROM json_each(?2) j WHERE ${mine}`,
          )
          .bind(m.publishId, JSON.stringify(ch), token),
      );
    // Question-text search (round 11): the words of the SERVED stems only (what a non-author is
    // served: the stripped form when the key is hidden), never the full form; keyed on this publish,
    // which browse reaches through items.public_publish_id.
    // ... and the words of the title (its search fold, variants included), the subject and the tags (round 12).
    const metaWords = [...titleSearch(title).split(" "), ...searchKey(meta.subject).split(" "), ...[...fullTags].flatMap((t) => searchKey(t).split(" "))].filter((x) => x.length > 0 && x.length <= 40);
    for (const ch of chunks([...new Set([...searchWords, ...metaWords])].slice(0, SEARCH_WORDS_MAX + 500), 500))
      stmts.push(c.db.prepare(`INSERT OR IGNORE INTO served_words (publish_id, word) SELECT ?1, value FROM json_each(?2) WHERE ${mine}`).bind(m.publishId, JSON.stringify(ch), token));
    for (const ch of chunks([...tags], 200))
      stmts.push(
        c.db
          .prepare(`INSERT OR IGNORE INTO item_tags (publish_id, tag, fold) SELECT ?1, j.value ->> '$[0]', j.value ->> '$[1]' FROM json_each(?2) j WHERE ${mine}`)
          .bind(m.publishId, JSON.stringify(ch.map((t) => [t, searchKey(t)])), token),
      );
    if (standards.size > 0) stmts.push(c.db.prepare(`INSERT OR IGNORE INTO item_standards (publish_id, code) SELECT ?1, value FROM json_each(?2) WHERE ${mine}`).bind(m.publishId, JSON.stringify([...standards]), token));
    const blobRows = (form: string, list: string[]) =>
      c.db
        .prepare(`INSERT OR IGNORE INTO item_blobs (publish_id, form, sha256, seq) SELECT ?1, ?4, j.value ->> '$.s', j.value ->> '$.q' FROM json_each(?2) j WHERE ${mine}`)
        .bind(m.publishId, JSON.stringify(list.map((x) => ({ s: x, q: blobSeq(x) }))), token, form);
    if (shas.length > 0) stmts.push(blobRows("full", shas));
    if (strippedShas.length > 0) stmts.push(blobRows("stripped", strippedShas));
    if (newQids.length > 0)
      stmts.push(
        c.db
          .prepare(`INSERT OR IGNORE INTO question_ids (qid, item_id, first_publish_id, created_at, seq) SELECT value, ?4, ?1, ?5, ?6 FROM json_each(?2) WHERE ${mine}`)
          .bind(m.publishId, JSON.stringify(newQids), token, itemId, now, newSeq),
      );
    const pkRows = [
      ...partKeys.flatMap((list, pos) =>
        list.map((x) => ({ pos, part: x.part, key: x.key, ...(x.key.startsWith("s:") ? { tk: JSON.stringify(partStems[pos]![x.part]?.tokens ?? []), bd: JSON.stringify(partStems[pos]![x.part]?.bands ?? []), kb: JSON.stringify(partKBands[pos]![x.part] ?? []) } : {}) })),
      ),
      ...unitKeys.flatMap((list, pos) => list.map((x) => ({ pos, part: x.part, key: x.key }))),
    ];
    // (At most PART_KEYS_MAX rows a publish: a 1000-question bank of 50-part Multiparts cannot write
    // tens of thousands of key rows; parts past the cap are covered by their question's own keys.)
    pkRows.splice(PART_KEYS_MAX);
    for (const ch of chunks(pkRows, 300))
      stmts.push(
        c.db
          .prepare(`INSERT OR IGNORE INTO item_part_keys (publish_id, pos, part, key, tokens, bands, kbands) SELECT ?1, j.value ->> '$.pos', j.value ->> '$.part', j.value ->> '$.key', j.value ->> '$.tk', j.value ->> '$.bd', j.value ->> '$.kb' FROM json_each(?2) j WHERE ${mine}`)
          .bind(m.publishId, JSON.stringify(ch), token),
      );
    for (const ch of chunks(stmts, 20)) await c.db.batch(ch);
    await c.db.prepare(`UPDATE item_publishes SET status = 'staged' WHERE publish_id = ?1 AND status = 'staging' AND stage_token = ?2`).bind(m.publishId, token).run();
  } catch (e) {
    await discardStaged(c, stagedRow).catch(() => {});
    throw e;
  }

  // ---- commit (one batch, compare-and-set) ---------------------------------------------------------
  const result = {
    item_id: itemId,
    publish_id: m.publishId,
    version,
    kind,
    visibility,
    license: m.license,
    hide_key: m.hideKey,
    qids,
    public_version: publicVersion,
    possible_duplicate_of: possibleDuplicate,
    // The author's review of what non-authors will be served (positions and orders, no text).
    served_orders: reused ? reused.servedOrders : (st?.servedOrders ?? []),
    order_reasons: st?.orderReasons ?? [],
    labels_removed: st?.labelsRemoved ?? [],
    images_kept: st?.imagesKept ?? [],
    images_removed: st?.imagesRemoved ?? [],
    figures_removed: st?.figuresRemoved ?? [],
    dropped: full.dropped,
  };
  const resultJson = JSON.stringify(result);
  const listing = [title, meta.subject, meta.course, meta.grade, kindsMask, n, m.hideKey ? 1 : 0, m.license, hasImages];
  // The title fold starts with a space, so one LIKE '% word%' finds a title prefix and a word prefix.
  const folds = [titleSearch(title), fold(meta.subject)];
  const guard = (a: string, b: string) => `EXISTS (SELECT 1 FROM items WHERE id = ${a} AND current_publish_id = ${b})`;
  // (Round 22 V-a: this publish's served contents and units, as the licence history records them.)
  // Round 24 (critic31a/31b): a question's licence-history key is the CANONICAL served content: its
  // licence-sense hash (stem, answers and explanation; choices and parts as multisets), never points,
  // difficulty, topics or the shuffle order. The exact keyed content hash used before included all of
  // those, so changing a point value (or reverting an edit) closed the reader's licence window and
  // refused a licence the author had publicly relaxed.
  const qcanon = (i: number) => `q:${(m.hideKey ? (nearStripped?.[i] ?? nearFull[i]) : nearFull[i]) ?? nearFull[i]}`;
  const ckeys = [
    ...new Set([
      ...fullContent.map((_, i) => qcanon(i)),
      ...unitsAt.flatMap((us) => us.filter((u) => !u.hidden).map((u) => `u:${u.key}`)),
    ]),
  ];
  // Independence claims are always flagged; the third in 30 days flags the account too (round 7 M4).
  // Round 16 (critic21c non-blocker): a claim the account already made, about the same items (its
  // own bank maintenance), is not a new one: only claims naming an item it never claimed before count.
  const claimedBefore = m.independent.length
    ? new Set(
        (
          await c.db
            .prepare(
              `SELECT DISTINCT v.value AS id FROM pool_audit a, json_each(json_extract(a.detail, '$.independent_of')) v
                WHERE a.actor_id = ?1 AND a.action IN ('item.publish', 'item.republish') AND json_extract(a.detail, '$.independent_of') IS NOT NULL`,
            )
            .bind(me)
            .all<{ id: string }>()
        ).results.map((x) => x.id),
      )
    : new Set<string>();
  const freshClaim = m.independent.some((x) => !claimedBefore.has(x));
  // (Counted by the distinct items claimed in 30 days, not by publishes.)
  const priorClaims = freshClaim
    ? ((await c.db
        .prepare(
          `SELECT COUNT(DISTINCT v.value) AS n FROM pool_audit a, json_each(json_extract(a.detail, '$.independent_of')) v
            WHERE a.actor_id = ?1 AND a.action IN ('item.publish', 'item.republish') AND a.at > ?2 AND json_extract(a.detail, '$.independent_of') IS NOT NULL`,
        )
        .bind(me, now - 30 * DAY_MS)
        .first<{ n: number }>())?.n ?? 0)
    : 0;
  // Round 32: circle independence statements count with these (distinct banks named in 30 days); a bank
  // the account named before is not a new claim.
  const circleStatement = circleClaimed.length ? circleClaim!.statement : null;
  const priorCircleClaims = circleClaimed.length
    ? await (async () => {
        const before = new Set(
          (
            await c.db
              .prepare(
                `SELECT DISTINCT v.value AS id FROM pool_audit a, json_each(json_extract(a.detail, '$.circle_independent_of')) v
                  WHERE a.actor_id = ?1 AND a.action IN ('item.publish', 'item.republish') AND json_extract(a.detail, '$.circle_independent_of') IS NOT NULL`,
              )
              .bind(me)
              .all<{ id: string }>()
          ).results.map((x) => x.id),
        );
        if (!circleClaimed.some((x) => !before.has(x))) return 0;
        return (
          (await c.db
            .prepare(
              `SELECT COUNT(DISTINCT v.value) AS n FROM pool_audit a, json_each(json_extract(a.detail, '$.circle_independent_of')) v
                WHERE a.actor_id = ?1 AND a.action IN ('item.publish', 'item.republish') AND a.at > ?2 AND json_extract(a.detail, '$.circle_independent_of') IS NOT NULL`,
            )
            .bind(me, now - 30 * DAY_MS)
            .first<{ n: number }>())?.n ?? 0
        );
      })()
    : 0;
  // Round 12 M6: a near-rewording of confirmed student-data text (MinHash bands of the blocked stems,
  // token Jaccard >= FUZZY_JACCARD) is flagged for the admin. Not held (the exact and normalised keys
  // hold), and nothing about it reaches the publisher.
  // (Round 37: the check itself is privacynear.ts, shared with a circle send.)
  const privacyNear = await privacyNearPositions(
    c,
    // Every stem of a position: its own and its Multipart parts' (round 16 B5).
    mh.map((_, i) => [{ tokens: tokens[i]!, bands: bands[i]! }, ...partStems[i]!]),
    mh.map((_, i) => [mh[i]!, bkNorm[i]!, bkStem[i]!, bkFull[i]!, bkFields[i]!.expl, bkFields[i]!.ans, ...partKeys[i]!.map((x) => x.key)]),
    // Positions an exact or normalised key already blocks are held (and flagged) at commit.
    async () =>
      new Set(
        (
          await c.db
            .prepare(`SELECT hash FROM privacy_blocks WHERE kind = 'match' AND hash IN (SELECT value FROM json_each(?1))`)
            .bind(JSON.stringify(blockKeys))
            .all<{ hash: string }>()
        ).results.map((x) => x.hash),
      ),
    skMemo,
    c.deps.privacyBandRowsMax ?? PRIVACY_BAND_ROWS_MAX,
  );
  // (Round 21: a cut of the author's own near-rewordings is flagged too.)
  for (const k of ownCut) if (!overflowed.includes(k)) overflowed.push(k);
  // ONE flag per kind per publish (round 8 I6b): a count and the first few, never one per question.
  const few = <T,>(xs: T[]) => ({ count: xs.length, first: xs.slice(0, FLAG_DETAIL_MAX) });
  const flagsAll = [
    ...(priorClaims + priorCircleClaims >= 2 ? [{ kind: "independent_repeat", detail: JSON.stringify({ claims_in_30_days: priorClaims + priorCircleClaims + 1 }) }] : []),
    ...(circleClaimed.length > 0 ? [{ kind: "independent_claim", detail: JSON.stringify({ circle_banks: few(circleClaimed), statement: circleStatement }) }] : []),
    ...(softOnly.length > 0 || soft.results.length > 0 ? [{ kind: "license_soft", detail: JSON.stringify({ positions: few(softOnly), matches_items: few(soft.results.map((r) => r.id)), license: m.license }) }] : []),
    ...(conflicts.length > 0 ? [{ kind: "license_conflict", detail: JSON.stringify({ positions: few(conflicts), license: m.license }) }] : []),
    ...(grandfathered.length > 0 ? [{ kind: "license_grandfathered", detail: JSON.stringify({ positions: few(grandfathered), license: m.license }) }] : []),
    ...(unverified.length > 0 ? [{ kind: "origin_unverified", detail: JSON.stringify(few(unverified)) }] : []),
    ...(declaredModified.length > 0 ? [{ kind: "origin_modified", detail: JSON.stringify(few(declaredModified)) }] : []),
    ...(suspect.length > 0 ? [{ kind: "origin_suspect", detail: JSON.stringify(few(suspect)) }] : []),
    ...(blocked ? [{ kind: "blocklist", detail: JSON.stringify({ hash_kind: (blocked as { kind: string }).kind }) }] : []),
    ...(m.independent.length > 0 ? [{ kind: "independent_claim", detail: JSON.stringify({ items: few(m.independent), statement: m.statement }) }] : []),
    ...(overflowed.length > 0 ? [{ kind: "evidence_overflow", detail: JSON.stringify(few(overflowed)) }] : []),
    ...(privacyNear.length > 0 ? [{ kind: "privacy_text_match", detail: JSON.stringify({ near: few(privacyNear) }) }] : []),
  ];
  // Round 9 K3: rows per (account, kind, day); past FLAGS_PER_ACCOUNT_DAY a kind folds into one
  // aggregated row with a count. Integrity kinds never fold (no burst of junk can silence them), and
  // origin_unverified always does. Nothing is dropped.
  const dayStart = Math.floor(now / DAY_MS) * DAY_MS;
  const perKind = new Map(
    (await c.db.prepare(`SELECT kind, COUNT(*) AS n FROM admin_flags WHERE user_id = ?1 AND at >= ?2 AND dedup IS NULL GROUP BY kind`).bind(me, dayStart).all<{ kind: string; n: number }>()).results.map((x) => [x.kind, x.n]),
  );
  const foldsKind = (k: string) => COLLAPSED_FLAGS.has(k) || (!INTEGRITY_FLAGS.has(k) && (perKind.get(k) ?? 0) >= FLAGS_PER_ACCOUNT_DAY);
  const flags = flagsAll.filter((f) => !foldsKind(f.kind)).map((f) => ({ id: newId(), ...f }));
  const folded = flagsAll.filter((f) => foldsKind(f.kind)).map((f) => ({ id: newId(), ...f, dedup: `agg:${f.kind}:${me}:${dayStart / DAY_MS}` }));
  const stmts: D1PreparedStatement[] = [];
  if (isNew)
    stmts.push(
      c.db
        .prepare(
          `INSERT INTO items (id, author_id, kind, visibility, current_publish_id, prev_publish_id, version, title, subject, course, grade,
             kinds_mask, question_count, hide_key, license, has_images, committed_at, created_at, updated_at, public_version, public_at, public_publish_id, title_fold, subject_fold)
           SELECT ?1, ?2, ?3, ?4, ?5, NULL, 1, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?15, ?15, 1, ?15, ?5, ?17, ?18
            WHERE EXISTS (SELECT 1 FROM item_publishes WHERE publish_id = ?5 AND status = 'staged' AND author_id = ?2 AND stage_token = ?16)
              AND NOT EXISTS (SELECT 1 FROM items WHERE id = ?1)`,
        )
        .bind(itemId, me, kind, visibility, m.publishId, ...listing, now, token, ...folds),
    );
  else
    stmts.push(
      c.db
        .prepare(
          `UPDATE items SET current_publish_id = ?4, prev_publish_id = current_publish_id, version = ?5, title = ?6, subject = ?7, course = ?8, grade = ?9,
             kinds_mask = ?10, question_count = ?11, hide_key = ?12, license = ?13, has_images = ?14, committed_at = ?15, updated_at = ?15,
             public_version = ?17, public_at = COALESCE(?18, ?15), public_publish_id = CASE WHEN ?19 = 1 THEN COALESCE(public_publish_id, ?4) ELSE ?4 END,
             title_fold = ?20, subject_fold = ?21, license_version = license_version + (license <> ?13)
           WHERE id = ?1 AND author_id = ?2 AND current_publish_id IS ?3 AND removed_at IS NULL
             AND EXISTS (SELECT 1 FROM item_publishes WHERE publish_id = ?4 AND status = 'staged' AND author_id = ?2 AND stage_token = ?16)`,
        )
        .bind(itemId, me, m.base, m.publishId, version, ...listing, now, token, publicVersion, publicAt, reused ? 1 : 0, ...folds),
    );
  stmts.push(
    // Written with every round-12 column: nothing for the cron backfill to redo.
    c.db.prepare(`UPDATE items SET fold_rev = 1 WHERE id = ?1 AND ${guard("?1", "?2")}`).bind(itemId, m.publishId),
    c.db.prepare(`UPDATE item_publishes SET status = 'previous' WHERE item_id = ?1 AND status = 'current' AND publish_id <> ?2 AND ${guard("?1", "?2")}`).bind(itemId, m.publishId),
    c.db
      .prepare(`UPDATE item_publishes SET status = 'current', committed_at = ?3, result_json = ?4 WHERE publish_id = ?2 AND status = 'staged' AND ${guard("?1", "?2")}`)
      .bind(itemId, m.publishId, now, resultJson),
    c.db.prepare(`DELETE FROM pending_blobs WHERE uploader_id = ?3 AND sha256 IN (SELECT value FROM json_each(?4)) AND ${guard("?1", "?2")}`).bind(itemId, m.publishId, me, JSON.stringify(shas)),
    // ... their stems' bands (round 11 P1: their own near-rewordings, under "own:<id>") ...
    c.db
      .prepare(`INSERT OR IGNORE INTO stem_bands (user_id, band, item_id, qid) SELECT 'own:' || ?3, b.value, ?1, iq.qid FROM item_questions iq, json_each(COALESCE(iq.stem_bands, '[]')) b WHERE iq.publish_id = ?2 AND ${guard("?1", "?2")}`)
      .bind(itemId, m.publishId, me),
    // ... every publish's served stem bands under "pub" (round 16 B2: earlier near publications;
    // visibility is decided when read) ...
    c.db
      .prepare(`INSERT OR IGNORE INTO stem_bands (user_id, band, item_id, qid) SELECT 'pub', b.value, ?1, iq.qid FROM item_questions iq, json_each(COALESCE(iq.stem_bands, '[]')) b WHERE iq.publish_id = ?2 AND ${guard("?1", "?2")}`)
      .bind(itemId, m.publishId),
    // ... and their SERVED contents in the licence sense (round 12 M1: what makes them first).
    c.db
      .prepare(
        `INSERT INTO author_contents (author_id, match_hash, kind, hash, first_seq)
         SELECT ?3, match_hash, 'nshown', CASE WHEN ?4 = 1 THEN near_stripped ELSE near_full END, seq FROM item_questions
          WHERE publish_id = ?2 AND CASE WHEN ?4 = 1 THEN near_stripped ELSE near_full END IS NOT NULL AND ${guard("?1", "?2")}
         ON CONFLICT (author_id, kind, hash, match_hash) DO UPDATE SET first_seq = MIN(first_seq, excluded.first_seq)`,
      )
      .bind(itemId, m.publishId, me, m.hideKey ? 1 : 0),
    // Round 17 (critic22c M2): the units it used (texts, images, question units), durable, so the
    // author's own material stays hers after a delete or purge.
    c.db
      .prepare(
        `INSERT INTO author_contents (author_id, match_hash, kind, hash, first_seq)
         SELECT ?3, '', 'unit', j.value ->> '$.k', CAST(j.value ->> '$.s' AS INTEGER) FROM json_each(?4) j WHERE ${guard("?1", "?2")}
         ON CONFLICT (author_id, kind, hash, match_hash) DO UPDATE SET first_seq = MIN(first_seq, excluded.first_seq)`,
      )
      .bind(itemId, m.publishId, me, JSON.stringify(unitRecord)),
    // The author's own contents, durable (round 10 M-5): key-free always, the full content when shown.
    c.db
      .prepare(
        `INSERT INTO author_contents (author_id, match_hash, kind, hash, first_seq)
         SELECT ?3, match_hash, 'kf', near_kf, seq FROM item_questions WHERE publish_id = ?2 AND near_kf IS NOT NULL AND ${guard("?1", "?2")}
         UNION ALL SELECT ?3, match_hash, 'shown', full_content, seq FROM item_questions WHERE publish_id = ?2 AND ?4 = 0 AND ${guard("?1", "?2")}
         ON CONFLICT (author_id, kind, hash, match_hash) DO UPDATE SET first_seq = MIN(first_seq, excluded.first_seq)`,
      )
      .bind(itemId, m.publishId, me, m.hideKey ? 1 : 0),
    // Rounds 22-23 (critic29a V-a, critic30b W1, critic30a B4): what this publish OFFERS (its served
    // contents and units); the licence history is synced from it at the end of the batch.
    c.db.prepare(`DELETE FROM item_offer WHERE item_id = ?1 AND ${guard("?1", "?2")}`).bind(itemId, m.publishId),
    c.db.prepare(`INSERT OR IGNORE INTO item_offer (item_id, ckey) SELECT ?1, value FROM json_each(?3) WHERE ${guard("?1", "?2")}`).bind(itemId, m.publishId, JSON.stringify(ckeys)),
    // Round 23 (owner): the publishing-terms affirmation, per publish (admin views only).
    c.db
      .prepare(`INSERT OR IGNORE INTO attestations (publish_id, item_id, user_id, at, terms_version) SELECT ?2, ?1, ?3, ?4, ?5 WHERE ${guard("?1", "?2")}`)
      .bind(itemId, m.publishId, me, now, m.attest!.termsVersion),
    // Round 23 (critic30a B3): the author's durable KEYED record of each stem (its keyed bands, keyed
    // token and body sets, first seq), so her own earlier versions stay hers after retention or purge.
    // Round 30 (staging D1 insights): the question's own entry of ?4 is read by its position (entry i is
    // `{p: i, e}`, stemRecord above); the cross join over every entry, matched on `p` afterwards, read
    // 7.9 million rows for a 990-question bank. The `p` check keeps the old join's meaning.
    c.db
      .prepare(
        `INSERT INTO author_stems (author_id, band, qhash, tok_sk, body_sk, bk_expl, bk_ans, bk_expl_full, first_seq)
         SELECT ?3, kb.value, iq.match_hash, iq.tok_sk, iq.body_sk, COALESCE(iq.bk_expl, ''), COALESCE(iq.bk_ans, ''), COALESCE(?4 ->> ('$[' || iq.pos || '].e'), ''), MIN(iq.seq)
           FROM item_questions iq CROSS JOIN json_each(COALESCE(iq.kbands, '[]')) kb
          WHERE iq.publish_id = ?2 AND iq.tok_sk IS NOT NULL AND CAST(?4 ->> ('$[' || iq.pos || '].p') AS INTEGER) = iq.pos AND ${guard("?1", "?2")}
          GROUP BY kb.value, iq.match_hash, iq.bk_expl, iq.bk_ans, ?4 ->> ('$[' || iq.pos || '].e')
         ON CONFLICT (author_id, band, qhash, bk_expl, bk_ans) DO UPDATE SET
           first_seq = MIN(first_seq, excluded.first_seq),
           bk_expl_full = COALESCE(author_stems.bk_expl_full, excluded.bk_expl_full)`,
      )
      .bind(itemId, m.publishId, me, JSON.stringify(stemRecord)),
    // The author's durable first publish of each question (never raised by retention or purge).
    c.db
      .prepare(
        `INSERT INTO author_firsts (author_id, match_hash, first_seq)
         SELECT ?3, match_hash, MIN(seq) FROM item_questions WHERE publish_id = ?2 AND ${guard("?1", "?2")} GROUP BY match_hash
         ON CONFLICT (author_id, match_hash) DO UPDATE SET first_seq = MIN(first_seq, excluded.first_seq)`,
      )
      .bind(itemId, m.publishId, me),
    // Round 18 (critic23c U4): a key_leak hold is the publish's: a republish whose SERVED form changed
    // (a new stripped body, or the key now shown) answers it; its key_leak reports are resolved as fixed.
    c.db.prepare(`DELETE FROM item_holds WHERE item_id = ?1 AND source = 'key_leak' AND ?3 = 0 AND ${guard("?1", "?2")}`).bind(itemId, m.publishId, reused ? 1 : 0),
    c.db
      .prepare(`UPDATE reports SET resolved_at = ?3, action = 'fixed' WHERE item_id = ?1 AND reason = 'key_leak' AND resolved_at IS NULL AND ?4 = 0 AND ${guard("?1", "?2")}`)
      .bind(itemId, m.publishId, now, reused ? 1 : 0),
    // The images were checked present in R2 above: a needs_reupload hold has been answered.
    c.db.prepare(`DELETE FROM item_holds WHERE item_id = ?1 AND source = 'needs_reupload' AND ${guard("?1", "?2")}`).bind(itemId, m.publishId),
  );
  // Privacy and DMCA blocks are re-checked INSIDE the commit batch (below), against this publish's
  // own rows: a block confirmed after the checks above still holds it (no gap). Carried holds are
  // computed above (they depend on other items, not on blocks).
  const holds: { reason: string; source: string }[] = [...(carried ? [{ reason: carried.reason, source: `carried:${carried.item_id}` }] : [])];
  const rightAuthor = `EXISTS (SELECT 1 FROM dmca_actions ra JOIN items ri ON ri.id = ra.rightful_item_id WHERE ra.id = bh.action_id AND ri.author_id = ?4)`;
  stmts.push(
    // A republish whose content no longer uses any privacy block lifts the privacy hold its earlier
    // content caused (round 8); one that still does keeps (or gets) it, just below.
    c.db
      .prepare(
        `DELETE FROM item_holds WHERE item_id = ?1 AND source = 'privacy' AND ${guard("?1", "?2")} AND NOT EXISTS (SELECT 1 FROM privacy_blocks pb WHERE
           (pb.kind = 'match' AND ${inQKeysOf("pb.hash", "?2")})
           OR (pb.kind = 'blob' AND pb.hash IN ${servedBlobsOf("?2")}))`,
      )
      .bind(itemId, m.publishId),
    c.db
      .prepare(
        `INSERT OR IGNORE INTO item_holds (item_id, reason, at, by, source)
         SELECT ?1, 'student_data', ?3, NULL, 'privacy' WHERE ${guard("?1", "?2")} AND EXISTS (SELECT 1 FROM privacy_blocks pb WHERE
           ((pb.kind = 'match' AND ${inQKeysOf("pb.hash", "?2")})
            OR (pb.kind = 'blob' AND pb.hash IN ${servedBlobsOf("?2")}))
           -- Round 12 M4: an admin's release of this item covers the keys it used then.
           -- (Round 13: for 90 days, and never for a block placed after the release.)
           AND NOT EXISTS (SELECT 1 FROM hold_releases hr, json_each(hr.keys_json) r WHERE hr.item_id = ?1 AND hr.source = 'privacy' AND r.value = pb.hash
                             AND hr.at > ?3 - ${HOLD_RELEASE_MS} AND pb.at <= hr.at))`,
      )
      .bind(itemId, m.publishId, now),
    // Round 13: a near-rewording of confirmed student-data text is HELD too (its own source, released
    // by an admin or by a republish that no longer comes near it), never only flagged.
    c.db.prepare(`DELETE FROM item_holds WHERE item_id = ?1 AND source = 'privacy_near' AND ?3 = 0 AND ${guard("?1", "?2")}`).bind(itemId, m.publishId, privacyNear.length),
    c.db
      .prepare(`INSERT OR IGNORE INTO item_holds (item_id, reason, at, by, source) SELECT ?1, 'student_data', ?3, NULL, 'privacy_near' WHERE ?4 > 0 AND ${guard("?1", "?2")}`)
      .bind(itemId, m.publishId, now, privacyNear.length),
    // Confirmed student-data text IS held at publish again (round 8 I6a): the only thing the hold can
    // tell its publisher is that text they hold verbatim and are publishing is confirmed student data,
    // and their item shows the same "held for review" as any other hold, with no detail. One
    // aggregated flag per publish goes to the admin queue as well.
    c.db
      .prepare(
        `INSERT INTO admin_flags (id, kind, item_id, publish_id, user_id, detail, at)
         SELECT ?4, 'privacy_text_match', ?1, ?2, ?5, json_object('positions', (SELECT json_group_array(iq.pos) FROM item_questions iq JOIN privacy_blocks pb ON pb.kind = 'match' AND ${qKeyEq("iq", "pb.hash")} WHERE iq.publish_id = ?2)), ?3
          WHERE ${guard("?1", "?2")} AND EXISTS (SELECT 1 FROM item_questions iq JOIN privacy_blocks pb ON pb.kind = 'match' AND ${qKeyEq("iq", "pb.hash")} WHERE iq.publish_id = ?2)`,
      )
      .bind(itemId, m.publishId, now, newId(), me),
    // Round 11: a copyright hold (an admin's hold-copies or a re-upload block) whose action's blocked
    // material this publish no longer contains is cleared, and the admin sees that it was (flag).
    c.db
      .prepare(
        `INSERT INTO admin_flags (id, kind, item_id, publish_id, user_id, detail, at)
         SELECT ?3 || '-' || h.rowid, 'hold_cleared', ?1, ?2, ?4, json_object('source', h.source, 'reason', h.reason), ?5 FROM item_holds h
          WHERE h.item_id = ?1 AND h.source LIKE 'dmca:%' AND ${guard("?1", "?2")} AND EXISTS (SELECT 1 FROM blocked_hashes x WHERE x.action_id = substr(h.source, 6)) AND NOT ${dmcaStillUsed("h", "?2")}`,
      )
      .bind(itemId, m.publishId, newId(), me, now),
    c.db.prepare(`DELETE FROM item_holds WHERE item_id = ?1 AND source LIKE 'dmca:%' AND ${guard("?1", "?2")} AND EXISTS (SELECT 1 FROM blocked_hashes x WHERE x.action_id = substr(item_holds.source, 6)) AND NOT ${dmcaStillUsed("item_holds", "?2")}`).bind(itemId, m.publishId),
    // (rightAuthor: the publisher authored the action's rightful item.)
    // DMCA re-upload blocks hold material that ENTERS the pool after the block: never an author the
    // admin exempted (the rightful author, F3c), and never what this item already carried before the
    // block (pre-existing copies are the admin's hold-copies decision, not a re-upload).
    c.db
      .prepare(
        `INSERT OR IGNORE INTO item_holds (item_id, reason, at, by, source)
         SELECT DISTINCT ?1, 'blocklist', ?3, NULL, 'dmca:' || bh.action_id FROM blocked_hashes bh
          WHERE ${guard("?1", "?2")} AND bh.lifted_at IS NULL AND (bh.expires_at IS NULL OR bh.expires_at > ?3) AND bh.action_id IS NOT NULL
            AND NOT EXISTS (SELECT 1 FROM hold_releases hr, json_each(hr.keys_json) r WHERE hr.item_id = ?1 AND hr.source = 'dmca:' || bh.action_id AND r.value = bh.hash
                              AND hr.at > ?3 - ${HOLD_RELEASE_MS} AND bh.at <= hr.at)
            AND NOT EXISTS (SELECT 1 FROM dmca_exempt e WHERE e.action_id = bh.action_id AND e.user_id = ?4)
            -- A takedown that names the rightful item never blocks the rightful item's own material
            -- (round 9 K1): a blocked hash the rightful item itself contains. Decided by the CONTENT,
            -- never by whether the publisher has some link to the rightful item.
            AND NOT EXISTS (SELECT 1 FROM dmca_actions a WHERE a.id = bh.action_id AND a.rightful_item_id IS NOT NULL AND ${rightfulExempts("a.rightful_item_id", "bh", "?4", "?2")})
            AND (
              -- What the item already carried before the block is exempt only for the rightful author
              -- (round 14 B4): a pre-staged copy is held at its next publish like any re-upload.
              -- (Round 24/25: the item the taken-down item itself came FROM is exempt outright — the
              -- derivation, not a publish timestamp, is the evidence, so an ordinary republish (whose
              -- base is now newer than the block) never makes the original a "re-upload"; an unlisted
              -- original cannot be named as the rightful item anyway.)
              (bh.kind = 'match' AND ${inQKeysOf("bh.hash", "?2")}
                AND NOT (${upstreamCarried("bh.action_id", "?1", "bh.hash")}
                         OR (${rightAuthor}
                             AND EXISTS (SELECT 1 FROM item_questions bq JOIN item_publishes bp ON bp.publish_id = bq.publish_id
                                          WHERE bq.publish_id = ?5 AND ${qKeyEq("bq", "bh.hash")} AND bp.committed_at <= bh.at))))
              OR (bh.kind = 'blob' AND bh.hash IN ${servedBlobsOf("?2")}
                AND NOT (${upstreamCarried("bh.action_id", "?1", "bh.hash")}
                         OR (${rightAuthor} AND ?5 IS NOT NULL AND bh.hash IN ${servedBlobsOf("?5")} AND (SELECT committed_at FROM item_publishes WHERE publish_id = ?5) <= bh.at))))`,
      )
      .bind(itemId, m.publishId, now, me, baseOf),
  );
  if (holds.length > 0)
    stmts.push(
      c.db
        .prepare(`INSERT OR IGNORE INTO item_holds (item_id, reason, at, by, source) SELECT ?1, j.value ->> '$.reason', ?3, NULL, j.value ->> '$.source' FROM json_each(?4) j WHERE ${guard("?1", "?2")}`)
        .bind(itemId, m.publishId, now, JSON.stringify(holds)),
    );
  if (flags.length > 0)
    stmts.push(
      c.db
        .prepare(
          `INSERT INTO admin_flags (id, kind, item_id, publish_id, user_id, detail, at)
           SELECT j.value ->> '$.id', j.value ->> '$.kind', ?1, ?2, ?3, j.value ->> '$.detail', ?4 FROM json_each(?5) j WHERE ${guard("?1", "?2")}`,
        )
        .bind(itemId, m.publishId, me, now, JSON.stringify(flags)),
    );
  for (const f of folded)
    stmts.push(
      c.db
        .prepare(
          `INSERT INTO admin_flags (id, kind, item_id, publish_id, user_id, detail, dedup, at)
           SELECT ?5, ?6, ?1, ?2, ?3, json_object('aggregated', 1, 'publishes', 1, 'count', COALESCE(json_extract(?7, '$.count'), 1), 'last', json(?7)), ?8, ?4 WHERE ${guard("?1", "?2")}
           ON CONFLICT (dedup) DO UPDATE SET
             detail = json_set(admin_flags.detail, '$.publishes', COALESCE(json_extract(admin_flags.detail, '$.publishes'), 1) + 1,
                               '$.count', COALESCE(json_extract(admin_flags.detail, '$.count'), 1) + COALESCE(json_extract(?7, '$.count'), 1), '$.last', json(?7)),
             item_id = excluded.item_id, publish_id = excluded.publish_id, at = excluded.at, resolved_at = NULL, resolved_by = NULL`,
        )
        .bind(itemId, m.publishId, me, now, f.id, f.kind, f.detail, f.dedup),
    );
  stmts.push(
    audit(
      c.db,
      { actorId: me, actorKind: "user", action: isNew ? "item.publish" : "item.republish", itemId, targetUserId: me, detail: { publish_id: m.publishId, version, license: m.license, hide_key: m.hideKey, ...(m.independent.length ? { independent_of: m.independent } : {}), ...(circleClaimed.length ? { circle_independent_of: circleClaimed } : {}) }, note: circleStatement === null ? m.statement : [m.statement, circleStatement].filter((x) => x !== null).join(" / ") },
      now,
      guard(`'${itemId}'`, `'${m.publishId}'`), // both match [A-Za-z0-9_-]+
    ),
    // (the licence history, last: after every hold this batch may place)
    ...offerSync(c.db, itemId, now, m.publishId),
    c.db.prepare(`SELECT current_publish_id, version FROM items WHERE id = ?1`).bind(itemId),
  );
  let won = false;
  let current: { current_publish_id: string | null; version: number } | undefined;
  try {
    const res = await c.db.batch(stmts);
    current = (res[res.length - 1]!.results as { current_publish_id: string | null; version: number }[])[0];
    won = current?.current_publish_id === m.publishId;
  } catch (e) {
    // e.g. the CHECK (public needs a public licence) after a concurrent visibility change.
    console.error(`pool: commit batch failed: ${errKind(e)}`);
  }
  if (!won) {
    const discarded = await discardStaged(c, { ...stagedRow, created_at: now }).catch(() => false);
    if (!discarded) {
      // A retry re-staged this publish_id meanwhile: our bodies belong to nothing (its row has its own keys).
      const row = await c.db.prepare(`SELECT full_key, stripped_key FROM item_publishes WHERE publish_id = ?1`).bind(m.publishId).first<{ full_key: string; stripped_key: string | null }>();
      const held = new Set([row?.full_key, row?.stripped_key].filter(Boolean));
      const ours = [fullKey, strippedKey].filter((k): k is string => k !== null && !held.has(k));
      if (ours.length) await c.r2.delete(ours);
    }
    // Staged rows were never reachable: every read joins items.current_publish_id.
    throw new PoolError(409, "conflict", { current_version: current?.version ?? null, current_publish_id: current?.current_publish_id ?? null });
  }

  // ---- after commit: the held-publish count, then retention (current + previous) --------------------
  // Rounds 12/14: charged PER QUESTION that would be held by a block (privacy, or an active copyright
  // block, by any key; and near-rewordings of blocked student-data text), whatever else happens.
  // Never charged: what an exemption covers (the rightful author and those served the rightful item,
  // exempt_authors, an admin's release of this item): honest work after a takedown is never throttled.
  const charged =
    privacyNear.length +
    ((
      await c.db
        .prepare(
          `SELECT COUNT(*) AS n FROM item_questions iq WHERE iq.publish_id = ?3 AND (
             EXISTS (SELECT 1 FROM privacy_blocks pb WHERE pb.kind = 'match' AND ${qKeyEq("iq", "pb.hash")}
                       AND NOT EXISTS (SELECT 1 FROM hold_releases hr, json_each(hr.keys_json) r WHERE hr.item_id = ?6 AND hr.source = 'privacy' AND r.value = pb.hash AND hr.at > ?4 - ${HOLD_RELEASE_MS} AND pb.at <= hr.at))
             OR EXISTS (SELECT 1 FROM blocked_hashes bh WHERE bh.kind = 'match' AND bh.lifted_at IS NULL AND (bh.expires_at IS NULL OR bh.expires_at > ?4) AND ${qKeyEq("iq", "bh.hash")}
                       AND NOT EXISTS (SELECT 1 FROM dmca_exempt e WHERE e.action_id = bh.action_id AND e.user_id = ?1)
                       -- (round 25: the item the taken-down item came from is exempt from the hold, so
                       -- it is never charged as a held publish either)
                       AND NOT ${upstreamCarried("bh.action_id", "?6", "bh.hash")}
                       AND NOT EXISTS (SELECT 1 FROM dmca_actions a WHERE a.id = bh.action_id AND a.rightful_item_id IS NOT NULL AND ${rightfulExempts("a.rightful_item_id", "bh", "?1", "?3")})
                       AND NOT EXISTS (SELECT 1 FROM hold_releases hr, json_each(hr.keys_json) r WHERE hr.item_id = ?6 AND hr.source = 'dmca:' || bh.action_id AND r.value = bh.hash AND hr.at > ?4 - ${HOLD_RELEASE_MS} AND bh.at <= hr.at)))`,
        )
        .bind(me, dayOf(now), m.publishId, now, 0, itemId)
        .first<{ n: number }>())?.n ?? 0);
  if (charged > 0) {
    await c.db.batch([
      c.db
        .prepare(`INSERT INTO usage_daily (user_id, day, metric, count) SELECT ?1, ?2, 'held_publishes', ?3 WHERE ${ALIVE_SQL("?1")} ON CONFLICT (user_id, day, metric) DO UPDATE SET count = count + excluded.count`)
        .bind(me, dayOf(now), charged),
      // Past the cap: held for review whatever happens to the blocks later (the admin was alerted).
      c.db.prepare(`INSERT OR IGNORE INTO item_holds (item_id, reason, at, by, source) SELECT ?1, 'admin', ?2, NULL, 'probe_cap' WHERE ?3 = 1`).bind(itemId, now, overCap ? 1 : 0),
    ]);
  }
  await retain(c, itemId);
  if (await c.db.prepare(`SELECT 1 AS x FROM admin_flags WHERE publish_id = ?1 AND kind = 'privacy_text_match'`).bind(m.publishId).first())
    await alertBudgeted(c, { kind: "privacy_text_match", itemId, held: !!(await c.db.prepare(`SELECT 1 AS x FROM item_holds WHERE item_id = ?1 AND source = 'privacy'`).bind(itemId).first()) });
  return json(isNew ? 201 : 200, result);
}

/**
 * Deletes every publish of `itemId` that is neither current, nor the previous one, nor the one
 * non-authors are served (`public_publish_id`, round 12 A1: hidden-only republishes keep serving an
 * older publish, whose rows, search words and body must stay).
 */
export async function retain(c: Ctx, itemId: string): Promise<void> {
  const old = await c.db
    .prepare(
      `SELECT p.publish_id, p.full_key, p.stripped_key FROM item_publishes p JOIN items i ON i.id = p.item_id
        WHERE p.item_id = ?1 AND p.status = 'previous' AND p.publish_id IS NOT i.prev_publish_id AND p.publish_id IS NOT i.public_publish_id`,
    )
    .bind(itemId)
    .all<{ publish_id: string; full_key: string; stripped_key: string | null }>();
  if (old.results.length === 0) return;
  await c.db.batch(deletePublishStatements(c.db, old.results.map((r) => r.publish_id)));
  await c.r2.delete(old.results.flatMap((r) => [r.full_key, ...(r.stripped_key ? [r.stripped_key] : [])]));
}

export type { Validated };

export const MAX_TAGS = 200;

/** SQL: does publish `pub` still contain material the DMCA action of hold row `h` blocked (match or served image)? */
export const dmcaStillUsed = (h: string, pub: string) =>
  `EXISTS (SELECT 1 FROM blocked_hashes bh WHERE bh.action_id = substr(${h}.source, 6)
             AND ((bh.kind = 'match' AND ${inQKeysOf("bh.hash", pub)}) OR (bh.kind = 'blob' AND bh.hash IN ${servedBlobsOf(pub)})))`;

/**
 * A chain longer than CHAIN_MAX keeps its FIRST link (the original author is always credited), a
 * placeholder carrying the licences of the links it stands for (so their grants still apply), and
 * the most recent links.
 */
export function collapseChain(chain: ChainLink[], max = CHAIN_MAX): ChainLink[] {
  // Round 11: the first CREDITS_HEAD (the originators), one placeholder, the most recent max - CREDITS_HEAD.
  const keepHead = Math.min(CREDITS_HEAD, Math.max(1, max >> 2));
  const keepTail = max - keepHead;
  if (chain.length <= keepHead + keepTail + 1) return chain;
  const head = chain.slice(0, keepHead);
  const tail = chain.slice(chain.length - keepTail);
  const middle = chain.slice(keepHead, chain.length - keepTail);
  const lic = [...new Set(middle.flatMap((l) => [l.license, ...(l.collapsed_licenses ?? [])]).filter((x) => x !== ""))].sort();
  // Round 10: the links left out are BETWEEN the first and the most recent (a placeholder's own count included).
  const count = middle.reduce((n, l) => n + (l.item_id === "" && /^and (\d+) contributors between$/.test(l.author_name) ? Number(/\d+/.exec(l.author_name)![0]) : 1), 0);
  return [...head, { author_name: `and ${count} contributors between`, item_id: "", qid: "", version: 0, license: "", modified: true, collapsed_licenses: lic }, ...tail];
}

/** The case fold used for browse filters (stored per item; any script, not only ASCII). */
export function fold(s: string): string {
  // Unicode case folding for search (round 8): compatibility forms, accents optional (marks
  // removed), lower case, and the folds lower-casing misses (ß -> ss, final sigma -> sigma).
  return s
    .normalize("NFKD")
    .replace(/\p{M}+/gu, "")
    .toLowerCase()
    .replace(/ß/g, "ss")
    .replace(/ς/g, "σ")
    .normalize("NFC");
}

/** Words in a short statement's stem at most (round 10 M-5). */
export const SHORT_WORDS = 8;
/** Words of the key (a Short sample, a Blank's accepted answers) or of an Mc's stem and choices together. */
export const SHORT_KEY_WORDS = 12;
const words = (t: unknown) => (typeof t === "string" ? t.trim().split(/\s+/u).filter(Boolean).length : 0);
/**
 * A short statement (rounds 10 M-5, 11 P6), decided on the SERVED form (key-free, so nothing about
 * a hidden key decides it): no explanation, image or figure, and
 * - Tf: a stem of at most SHORT_WORDS words;
 * - Short: a stem of at most SHORT_WORDS words and a SERVED sample of at most SHORT_KEY_WORDS words
 *   (a hidden-key Short, whose sample is not served, is never short);
 * - Blank: a stem of at most SHORT_WORDS words and served accepted answers of at most SHORT_KEY_WORDS
 *   words in all (hidden: never short);
 * - Mc: the stem and every choice together at most SHORT_KEY_WORDS words;
 * - Matching and Multipart: never.
 */
export function isShort(q: QuestionJson): boolean {
  const raw = q as unknown as { stem?: unknown; explanation?: unknown; image?: unknown; figure?: unknown; keyless?: unknown; kind?: Record<string, any> };
  if ((typeof raw.explanation === "string" && raw.explanation.trim() !== "") || raw.image || raw.figure || raw.keyless) return false;
  const k = kindTag(q);
  const kind = raw.kind ?? {};
  const stem = words(raw.stem);
  if (k === "Tf") return stem <= SHORT_WORDS;
  if (k === "Short") {
    const sample = kind.Short?.sample;
    return stem <= SHORT_WORDS && typeof sample === "string" && sample.trim() !== "" && words(sample) <= SHORT_KEY_WORDS;
  }
  if (k === "Blank") {
    const acc = kind.Blank?.accept;
    if (!Array.isArray(acc) || acc.length === 0) return false;
    const n = (acc as unknown[]).flat().reduce<number>((m, t) => m + words(t), 0);
    return stem <= SHORT_WORDS && n > 0 && n <= SHORT_KEY_WORDS;
  }
  if (k === "Mc") {
    const ans = kind.Mc?.answers;
    if (!Array.isArray(ans)) return false;
    return stem + (ans as { text?: unknown }[]).reduce((m, a) => m + words(a.text), 0) <= SHORT_KEY_WORDS;
  }
  return false;
}

/** Runs of whitespace as one space, trimmed (share-core's Line collapse). */
const collapseWs = (s: string) => s.split(/\s+/u).filter(Boolean).join(" ");

/**
 * The author's NFC text for a metadata line share-core accepted as `cleaned` (its NFKC form): used
 * only when share-core changed nothing but the compatibility fold (and so its checks hold for it),
 * and it is no longer than what share-core allowed. Otherwise share-core's text.
 */
export function authorsText(raw: unknown, cleaned: string): string {
  if (typeof raw !== "string") return cleaned;
  const nfc = collapseWs(raw.normalize("NFC"));
  return collapseWs(nfc.normalize("NFKC")) === cleaned && [...nfc].length <= [...cleaned].length ? nfc : cleaned;
}

function authorsMeta(raw: unknown, meta: { title: string; subject: string; course: string; grade: string; tags: string[] }) {
  const r = (raw && typeof raw === "object" ? raw : {}) as Record<string, unknown>;
  const cleanedTags = new Set(meta.tags);
  const tags = new Set<string>();
  const used = new Set<string>();
  for (const t of Array.isArray(r.tags) ? r.tags : []) {
    if (typeof t !== "string") continue;
    const nfc = collapseWs(t.normalize("NFC")).toLowerCase();
    const k = collapseWs(nfc.normalize("NFKC")).toLowerCase();
    if (cleanedTags.has(k) && !used.has(k) && [...nfc].length <= [...k].length) {
      tags.add(nfc);
      used.add(k);
    }
  }
  for (const k of meta.tags) if (!used.has(k)) tags.add(k);
  return { title: authorsText(r.title, meta.title), subject: authorsText(r.subject, meta.subject), course: authorsText(r.course, meta.course), grade: authorsText(r.grade, meta.grade), tags: [...tags].sort() };
}

/** Letters NFKD leaves whole, folded the way a reader types them (round 9). */
const LETTERS: Record<string, string> = { œ: "oe", æ: "ae", ø: "o", ł: "l", đ: "d", ð: "d", þ: "th", ı: "i", ĸ: "k", ŋ: "n" };
/** Part / unit key rows one publish may write (round 17, critic22e A3). */
export const PART_KEYS_MAX = 4000;

/** Distinct authors whose items have an image, a text or a question unit before it is stock (rounds 16-17). */
export const GENERIC_UNIT_AUTHORS = 3;
/** An independence statement that attributes the material to a public source (round 17 M1). */
// (Round 18: a URL alone attributes nothing: "found at https://..." is no source.)
export const ATTRIBUTION_RE = /\b(public domain|cc[- ]?by|cc0|creative commons|openstax|wikimedia|wikipedia)\b/i;

/** Words an explanation or model answer needs before copying it alone is a copy (round 16 B6). */
export const LIFT_WORDS = 25;

/** allowed_json with the reader marks of round 14 (soft, claimed, earlier). */
function withMarks(aj: string, mk: { soft?: boolean; claimed?: boolean; earlier?: boolean }): string {
  if (!mk.soft && !mk.claimed && !mk.earlier) return aj;
  try {
    return JSON.stringify({ ...(JSON.parse(aj) as object), ...(mk.soft ? { soft: true } : {}), ...(mk.claimed ? { claimed: true } : {}), ...(mk.earlier ? { earlier: true } : {}) });
  } catch {
    return aj;
  }
}

/** Words of a question as served: stem, choices, columns, served answers and explanation (round 14 B5). */
export function servedWordCount(q: unknown): number {
  const texts: string[] = [];
  const walk = (v: unknown) => {
    if (typeof v === "string") texts.push(v);
    else if (Array.isArray(v)) v.forEach(walk);
    else if (v && typeof v === "object") for (const [k, x] of Object.entries(v as Record<string, unknown>)) if (!["id", "created", "modified", "origin", "published", "image", "figure", "topics", "difficulty"].includes(k)) walk(x);
  };
  walk(q);
  return texts.join(" ").trim().split(/\s+/u).filter(Boolean).length;
}

/** Distinct search words of served stems, at most SEARCH_WORDS_MAX per publish (round 11). */
export const SEARCH_WORDS_MAX = 2000;
export function servedWords(qs: readonly unknown[]): string[] {
  const out = new Set<string>();
  for (const q of qs) {
    const stem = (q as { stem?: unknown }).stem;
    if (typeof stem !== "string") continue;
    for (const w of searchKey(stem).split(" ")) {
      if (w.length === 0 || w.length > 40) continue;
      out.add(w);
      if (out.size >= SEARCH_WORDS_MAX) return [...out];
    }
  }
  return [...out];
}

/** The search key of one text: folded, punctuation and hyphens as spaces. */
export function searchKey(s: string): string {
  return fold(s)
    .replace(/[œæøłđðþıĸŋ]/g, (c) => LETTERS[c]!)
    .replace(/[^\p{L}\p{N}]+/gu, " ")
    .trim();
}
/**
 * The stored title search column (round 9): the search key, the German spelling of umlauts (ü = ue),
 * and hyphenated words joined (co-operative = cooperative), each word-searchable (it starts with a
 * space, and `q` matches the start of any word).
 */
export function titleSearch(title: string): string {
  const german = title.replace(/[äöüÄÖÜ]/g, (c) => ({ ä: "ae", ö: "oe", ü: "ue", Ä: "Ae", Ö: "Oe", Ü: "Ue" })[c]!);
  const joined = title.replace(/(\p{L})[-\u2010\u2011'’](?=\p{L})/gu, "$1");
  const v = [...new Set([searchKey(title), searchKey(german), searchKey(joined)].filter(Boolean))];
  return ` ${v.join(" ")}`;
}

function tagsOf(metaTags: string[], qs: QuestionJson[]): Set<string> {
  const tags = new Set(metaTags.map((t) => t.toLowerCase()));
  for (const q of qs) for (const t of q.topics ?? []) if (typeof t === "string" && t.trim()) tags.add(t.trim().toLowerCase());
  return tags;
}

/**
 * The author's served orders for a REUSED stripped body (a hidden republish that changed nothing
 * served keeps the previous body, so the orders this publish's strip drew describe another
 * shuffle): each entry remapped onto the reused body by content, item for item. Null when some
 * item has no counterpart (then the body is not reused).
 */
export function remapServedOrders(reusedBody: PayloadJson, freshBody: PayloadJson, servedOrders: unknown[]): unknown[] | null {
  const canon = (v: unknown): string => canonical(v);
  const qOld = questionsOf(reusedBody);
  const qNew = questionsOf(freshBody);
  const kindAt = (qs: QuestionJson[], o: { question: number; part?: number }): Record<string, any> | null => {
    const q = qs[o.question];
    if (!q) return null;
    const k = q.kind as Record<string, any>;
    if (o.part === undefined) return k;
    return (k.Multipart?.parts?.[o.part]?.kind as Record<string, any> | undefined) ?? null;
  };
  // old[i] -> the index j of the same content in fresh (each j used once) -> map[j].
  const remap = (oldList: unknown[], newList: unknown[], map: number[]): number[] | null => {
    if (oldList.length !== newList.length || map.length !== newList.length) return null;
    const used = new Array<boolean>(newList.length).fill(false);
    const newC = newList.map(canon);
    const out: number[] = [];
    for (const x of oldList) {
      const c = canon(x);
      const j = newC.findIndex((y, k) => !used[k] && y === c);
      if (j < 0) return null;
      used[j] = true;
      out.push(map[j]!);
    }
    return out;
  };
  const strip = (v: unknown) => {
    if (!v || typeof v !== "object") return v;
    const { id: _i, ...rest } = v as Record<string, unknown>;
    return rest;
  };
  const out: unknown[] = [];
  for (const raw of servedOrders as { question: number; part?: number; matching?: boolean; order?: number[]; left_order?: number[] }[]) {
    const ko = kindAt(qOld, raw);
    const kn = kindAt(qNew, raw);
    if (!ko || !kn || !Array.isArray(raw.order)) return null;
    if (raw.matching) {
      if (!ko.Matching || !kn.Matching) return null;
      const order = remap((ko.Matching.distractors as unknown[]).map(strip), (kn.Matching.distractors as unknown[]).map(strip), raw.order);
      if (!order) return null;
      const e: Record<string, unknown> = { ...raw, order };
      if (Array.isArray(raw.left_order)) {
        const lefts = (k: Record<string, any>) => (k.Matching.pairs as { left?: unknown }[]).map((p) => p.left);
        const lo = remap(lefts(ko), lefts(kn), raw.left_order);
        if (!lo) return null;
        e.left_order = lo;
      }
      out.push(e);
    } else {
      if (!ko.Mc || !kn.Mc) return null;
      const order = remap((ko.Mc.answers as unknown[]).map(strip), (kn.Mc.answers as unknown[]).map(strip), raw.order);
      if (!order) return null;
      out.push({ ...raw, order });
    }
  }
  return out;
}

/**
 * Equal as served to non-authors, ignoring only ids, times and what strip shuffles (critic D5):
 * order matters everywhere share-core preserves it (questions, Multipart parts, locked choices,
 * acknowledged / kept orders, Matching left items); only the choices strip actually shuffled
 * (`servedOrders`: unlocked Mc choices of that question or part) and the Matching word bank are
 * compared as multisets.
 */
export function sameServed(a: PayloadJson, b: PayloadJson, servedOrders: unknown[], kept: unknown[] = []): boolean {
  const key = (o: { question: number; part?: number }) => (o.part === undefined ? `${o.question}` : `${o.question}/${o.part}`);
  const keptSet = new Set((kept as { question: number; part?: number }[]).filter((o) => o && typeof o.question === "number").map(key));
  // Without share-core's served orders (round 10 M-9), every unlocked Mc choice list whose order the
  // author did not ask to keep is one strip may shuffle: compared as a multiset (locked positions fixed).
  const known = (servedOrders as { question: number; part?: number; matching?: boolean }[]).filter((o) => !o.matching).map(key);
  // share-core round 8: the left column is served shuffled unless `keep_order` (then no `left_order`).
  const leftShuffled = new Set(
    (servedOrders as { question: number; matching?: boolean; left_order?: unknown }[]).filter((o) => o.matching && Array.isArray(o.left_order)).map((o) => `${o.question}`),
  );
  const shuffled = known.length > 0 ? new Set(known) : null;
  const isShuffled = (k: string) => (shuffled ? shuffled.has(k) : !keptSet.has(k));
  const sortKeys = (v: unknown): unknown => {
    if (Array.isArray(v)) return v.map(sortKeys);
    if (v && typeof v === "object")
      return Object.fromEntries(Object.entries(v as Record<string, unknown>).filter(([k]) => !["id", "created", "modified"].includes(k)).sort(([x], [y]) => (x < y ? -1 : 1)).map(([k, x]) => [k, sortKeys(x)]));
    return v;
  };
  const answers = (list: { locked?: boolean }[], isShuffled: boolean) =>
    isShuffled
      ? { locked: list.map((x, k) => (x.locked ? [k, JSON.stringify(sortKeys(x))] : null)).filter(Boolean), free: list.filter((x) => !x.locked).map((x) => JSON.stringify(sortKeys(x))).sort() }
      : list.map((x) => JSON.stringify(sortKeys(x)));
  const kindCanon = (kind: Record<string, any>, key: string): unknown => {
    if (kind.Mc) return { Mc: answers(kind.Mc.answers, isShuffled(key)) };
    // The served word bank is a multiset (share-core round 8 shuffles the rights among the pairs too).
    if (kind.Matching)
      return {
        Matching: {
          // Round 8: a shuffled left column (`left_order`) is a multiset too; a kept one keeps its order.
          left: ((l: string[]) => (leftShuffled.has(key) ? l.sort() : l))((kind.Matching.pairs as { left?: unknown }[]).map((p) => JSON.stringify(sortKeys(p.left)))),
          bank: [...(kind.Matching.pairs as { right?: unknown }[]).map((p) => JSON.stringify(sortKeys(p.right))), ...(kind.Matching.distractors as unknown[]).map((d) => JSON.stringify(sortKeys(d)))].sort(),
        },
      };
    if (kind.Multipart) return { Multipart: (kind.Multipart.parts as { stem: string; kind: Record<string, any> }[]).map((p, k) => ({ stem: p.stem, kind: kindCanon(p.kind, `${key}/${k}`) })) };
    return sortKeys(kind);
  };
  const qCanon = (q: QuestionJson, i: number) => JSON.stringify({ ...(sortKeys({ ...q, kind: null }) as object), kind: kindCanon(q.kind as Record<string, any>, `${i}`) });
  const qa = questionsOf(a);
  const qb = questionsOf(b);
  if (qa.length !== qb.length || !qa.every((q, i) => qCanon(q, i) === qCanon(qb[i]!, i))) return false;
  const rest = (p: PayloadJson) => JSON.stringify(sortKeys(p.bank ? { ...p.bank, questions: null } : { ...p, question: null }));
  return rest(a) === rest(b);
}
