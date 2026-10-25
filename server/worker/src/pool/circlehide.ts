// Round 43: circles phase C, "hide keys from viewers" (docs/circles-plan.md D12).
//
// A bank's `hide_key` (set by its author while an editor or the owner, or by the circle's owner: PATCH .../banks/:bid
// {hide_key}) serves its VIEWERS share-core's stripped form of every version: the public pool's strip and key-hint
// gate, the same function a hidden-key publish uses (core.ts strip), never a rule of the Worker's own. Editors and the
// owner read the full text; the stored bodies never change.
//
// * Decisions are recorded per version (`circle_versions.strip_opts`): the author's label decisions, acknowledged
//   preserved orders, acknowledged low-risk formatting findings and the images the viewers' copy keeps (`image_ok`,
//   and those images' hashes for the image route). A send to a bank that hides its keys passes the gate with its own
//   decisions or stores nothing (the publish's refusals, to the sender: an editor or the owner, who reads the keys
//   anyway); the PATCH that turns it on strips the head with the decisions it carries (refused the same way) and
//   records them; a rollback copies its target's.
// * Fail closed: a version is stripped at each viewer read with the decisions it recorded (none: none). A version the
//   gate refuses that way (a key in its text, an undecided label, an unacknowledged order) is `409
//   keys_hidden_unavailable` to viewers, with no text and no findings; nothing is ever served whole instead.
// * Cost: a viewer's read is publish-sized work: the isolate's heavy slot (shared with publishes and sends; 503 busy
//   past its queue), the send's reservation for the body (max(15 x bytes, values x 96 x 2)), the read's own
//   reservation for the stripped bytes until they are streamed, and its own daily cap (circle_hidden_reads) on top of
//   item_reads. A 304 (If-None-Match) strips nothing. Nothing is cached: each read strips again (the choice order is a
//   fresh uniform shuffle each time, share-core's own seed; the author-only served orders are dropped at once).
// * D13: what viewers are served is circle material too: the key-free match hashes of the stripped form (they differ
//   from the full form's where a label was stripped) are recorded in circle_qkeys under the first sender of the full
//   text they come from, so a viewer's copy published publicly meets the same check.
import { PARSE_BYTES_PER_VALUE, scanJson } from "./jsonscan.ts";
import { isFail, questionsOf, type Fail, type LabelDecision, type PayloadJson, type Pos, type QuestionJson, type Stripped } from "./core.ts";
import { acquireHeavy, HEAVY_QUEUE_WAIT_MS } from "./publish.ts";
import { giveDaily, PoolError, PUBLISH_COST_PER_BYTE, requireDaily, reserveBytes, type Ctx } from "./shared.ts";

/** The author's strip decisions for one version (share-core's options; the publish's meta fields). */
export interface StripDecisions {
  labels: LabelDecision[];
  orderAcks: Pos[];
  imageOk: number[];
  ackFormatting: number[];
}
export const NO_DECISIONS: StripDecisions = { labels: [], orderAcks: [], imageOk: [], ackFormatting: [] };
export const DECISION_FIELDS = ["labels", "order_acks", "ack_formatting", "image_ok"] as const;

const isNat = (x: unknown): x is number => typeof x === "number" && Number.isInteger(x) && x >= 0 && x < 1_000_000;

/** The decisions in a request body or a send's meta, checked as a publish's are (publish.ts readMeta); 400 on the field. */
export function readStripDecisions(m: Record<string, unknown>): StripDecisions {
  const bad = (field: string) => new PoolError(400, "invalid_request", { field });
  const labels = m.labels ?? [];
  if (!Array.isArray(labels) || labels.length > 60_000) throw bad("labels");
  const out: StripDecisions = { labels: [], orderAcks: [], imageOk: [], ackFormatting: [] };
  for (const l of labels as Record<string, unknown>[]) {
    if (!l || typeof l !== "object" || !isNat(l.question) || (l.part !== undefined && !isNat(l.part)) || (l.choice !== "strip" && l.choice !== "keep")) throw bad("labels");
    out.labels.push(l.part === undefined ? { question: l.question, choice: l.choice } : { question: l.question, part: l.part as number, choice: l.choice });
  }
  const acks = m.order_acks ?? [];
  if (!Array.isArray(acks) || acks.length > 60_000) throw bad("order_acks");
  for (const a of acks as Record<string, unknown>[]) {
    if (!a || typeof a !== "object" || !isNat(a.question) || (a.part !== undefined && !isNat(a.part))) throw bad("order_acks");
    out.orderAcks.push(a.part === undefined ? { question: a.question } : { question: a.question, part: a.part as number });
  }
  const img = m.image_ok ?? [];
  if (!Array.isArray(img) || img.length > 1000 || !img.every(isNat)) throw bad("image_ok");
  out.imageOk = [...new Set(img as number[])];
  const ackF = m.ack_formatting ?? [];
  if (!Array.isArray(ackF) || ackF.length > 60_000 || !ackF.every(isNat)) throw bad("ack_formatting");
  out.ackFormatting = [...new Set(ackF as number[])];
  return out;
}

const MEDIA_RE = /^media\/([0-9a-f]{64})\.(png|jpg|gif|webp)$/;
/** The image hashes a stripped payload keeps (what the image route may serve a viewer). */
export function keptImages(payload: PayloadJson): string[] {
  const out = new Set<string>();
  for (const q of questionsOf(payload)) {
    const f = q.image && typeof q.image === "object" ? q.image.file : null;
    const m = typeof f === "string" ? MEDIA_RE.exec(f) : null;
    if (m) out.add(m[1]!);
  }
  return [...out];
}

/**
 * The decisions a successful strip actually used, as they are recorded with a version: a decision per label finding
 * it made, the preserved orders it froze (each acknowledged, or it would have refused), the images it kept, and the
 * formatting acknowledgements of questions that exist. So what is stored is bounded by the bank, never by what a
 * request sent (a body may carry up to 60,000 decisions; a 1,000-question bank's are a few thousand at most), and
 * the same strip again with them gives the same result.
 */
export function recordedDecisions(d: StripDecisions, st: Pick<Stripped, "labelFindings" | "needsOrderAck" | "imagesKept">, questions: number): StripDecisions {
  const key = (p: { question: number; part?: number | null }) => `${p.question}/${p.part ?? ""}`;
  const labels = new Map<string, LabelDecision>();
  for (const f of st.labelFindings as { question?: unknown; part?: unknown; decision?: unknown }[]) {
    if (!isNat(f.question) || (f.decision !== "strip" && f.decision !== "keep")) continue;
    const part = isNat(f.part) ? f.part : undefined;
    labels.set(key({ question: f.question, part }), part === undefined ? { question: f.question, choice: f.decision } : { question: f.question, part, choice: f.decision });
  }
  const acks = new Map<string, Pos>();
  for (const a of st.needsOrderAck) if (isNat(a.question)) acks.set(key(a), a.part === undefined ? { question: a.question } : { question: a.question, part: a.part });
  return {
    labels: [...labels.values()],
    orderAcks: [...acks.values()],
    imageOk: [...new Set(st.imagesKept.filter((x) => isNat(x) && x < questions))],
    ackFormatting: d.ackFormatting.filter((x) => x < questions),
  };
}

/** The JSON recorded in circle_versions.strip_opts. */
export function decisionsJson(d: StripDecisions, imageShas: string[]): string {
  return JSON.stringify({ labels: d.labels, order_acks: d.orderAcks, ack_formatting: d.ackFormatting, image_ok: d.imageOk, image_shas: imageShas });
}
/** The decisions a version recorded (none when it recorded none, or the record does not read). */
export function decisionsOf(json: string | null): StripDecisions {
  if (!json) return NO_DECISIONS;
  try {
    const x = JSON.parse(json) as Record<string, unknown>;
    return readStripDecisions(x);
  } catch {
    return NO_DECISIONS;
  }
}

/** Positions whose stripped key-free hash differs from the full form's (a label stripped): [full, stripped] pairs. */
export function hashPairs(full: string[], stripped: string[]): [string, string][] {
  const out: [string, string][] = [];
  const seen = new Set<string>();
  full.forEach((f, k) => {
    const s = stripped[k];
    if (s && s !== f && !seen.has(`${f}|${s}`)) {
      seen.add(`${f}|${s}`);
      out.push([f, s]);
    }
  });
  return out;
}

/**
 * SQL (one statement): the stripped-form hashes `?pairs` ([[full, stripped], ...]) recorded in circle_qkeys under the
 * FIRST sender of the full text each comes from (its existing row), so the true first sender keeps passing D13 with
 * her own words and nobody else does. `cid`, `bid`, `pairs` are parameter references. (CROSS JOIN: one primary-key
 * probe per pair, never the bank's records times the pairs.)
 */
export const STRIPPED_QKEYS_SQL = (cid: string, bid: string, pairs: string, cond: string) =>
  `INSERT OR IGNORE INTO circle_qkeys (circle_id, match_hash, bank_id, first_version, first_seq, first_by)
   SELECT ${cid}, json_extract(j.value, '$[1]'), ${bid}, k.first_version, k.first_seq, k.first_by
     FROM json_each(${pairs}) j CROSS JOIN circle_qkeys k ON k.circle_id = ${cid} AND k.match_hash = json_extract(j.value, '$[0]') AND k.bank_id = ${bid}
    WHERE ${cond}`;

export const keysHiddenUnavailable = () =>
  new PoolError(409, "keys_hidden_unavailable", {
    message: "The answer keys of this bank are hidden from viewers, and this version cannot be shown without them. Ask the bank's author or the circle's owner.",
  });

/** What a strip of a stored body leaves the caller: the stripped bytes and a few small facts, or share-core's refusal. */
export type StoredStrip =
  | {
      ok: true;
      bytes: Uint8Array;
      /** The served form's key-free match hashes (share-core's `stripped.match_hashes`). */
      matchHashes: string[];
      /** The full form's (only when asked). */
      fullHashes: string[] | null;
      /** The images the viewers' copy keeps. */
      imageShas: string[];
      /** The decisions to record (recordedDecisions). */
      recorded: StripDecisions;
      report: { labels_removed: Pos[]; images_kept: number[]; images_removed: number[]; figures_removed: number[]; order_reasons: unknown[] };
    }
  | { ok: false; fail: Fail; missing: boolean };

/**
 * Strips a stored body with `d` under the isolate's heavy slot and a send's reservation for it (round 43: the same
 * work a hidden-key publish's strip is), and (when asked) computes the full form's key-free match hashes under the
 * same reservation. `hold(n)` is called with the stripped bytes' length BEFORE the reservation is released, so the
 * caller reserves what it keeps without a gap. Only the stripped bytes and small facts leave; the parsed trees and
 * the author-only served orders are dropped here.
 */
export async function stripStored(
  c: Ctx,
  key: string,
  d: StripDecisions,
  o: { withFullHashes?: boolean; hold?: (n: number) => void; onStripped?: (st: Stripped) => Promise<void> } = {},
): Promise<StoredStrip> {
  const heavy = await acquireHeavy(c.deps.heavyQueueMs ?? HEAVY_QUEUE_WAIT_MS, async () => {}, c.deps.heavySlots);
  const held: (() => void)[] = [heavy];
  try {
    const obj = await c.r2.get(key);
    if (!obj) return { ok: false, fail: { ok: false, errors: [{ code: "body_missing", path: "" }] }, missing: true };
    const size = (obj as R2ObjectBody).size;
    held.push(reserveBytes(size * (PUBLISH_COST_PER_BYTE - 1)));
    let bytes: Uint8Array | null = new Uint8Array(await (obj as R2ObjectBody).arrayBuffer()); // r2-object: a kept version's body (our own write)
    // (as a send of it would be reserved: by its value count when that is more)
    const extra = scanJson(bytes).values * PARSE_BYTES_PER_VALUE * 2 - size * (PUBLISH_COST_PER_BYTE - 1);
    if (extra > 0) held.push(reserveBytes(extra));
    const st = c.core.strip(bytes, d);
    if (isFail(st)) return { ok: false, fail: st, missing: false };
    let fullHashes: string[] | null = null;
    if (o.withFullHashes) {
      const h = c.core.matchHashes(bytes);
      if (isFail(h)) return { ok: false, fail: h, missing: false };
      fullHashes = h;
    }
    bytes = null;
    // (round 46: the caller's check of the stripped form, while it is still reserved)
    if (o.onStripped) await o.onStripped(st);
    o.hold?.(st.bytes.length);
    return {
      ok: true,
      bytes: st.bytes,
      matchHashes: st.matchHashes,
      fullHashes,
      imageShas: keptImages(st.payload),
      recorded: recordedDecisions(d, st, questionsOf(st.payload).length),
      report: { labels_removed: st.labelsRemoved, images_kept: st.imagesKept, images_removed: st.imagesRemoved, figures_removed: st.figuresRemoved, order_reasons: st.orderReasons },
    };
  } finally {
    for (const r of held.reverse()) r();
  }
}

/**
 * A viewer's read of a version of a bank that hides its keys: the stripped bytes (reserved through `hold`), or `409
 * keys_hidden_unavailable` (no text, no findings). Charged to circle_hidden_reads, given back when the server was
 * too busy to do it, and (round 47, critic 54a variant 8) for every read that serves nothing (409, 404, an error).
 */
export async function strippedForViewer(c: Ctx, me: string, fullKey: string, stripOpts: string | null, hold: (n: number) => void): Promise<Uint8Array> {
  try {
    await requireDaily(c, me, "circle_hidden_reads");
  } catch (e) {
    // (round 46, critic 53a nitpick: in words)
    if (e instanceof PoolError && e.code === "daily_limit")
      throw new PoolError(429, "daily_limit", { ...e.extra, message: "You have opened as many banks with hidden answer keys as you may today. Try again tomorrow." }, e.headers);
    throw e;
  }
  let r: StoredStrip;
  try {
    r = await stripStored(c, fullKey, decisionsOf(stripOpts), { hold });
  } catch (e) {
    await giveDaily(c, me, "circle_hidden_reads").catch(() => {});
    throw e;
  }
  if (!r.ok) {
    await giveDaily(c, me, "circle_hidden_reads").catch(() => {});
    if (r.missing) throw new PoolError(404, "not_found");
    throw keysHiddenUnavailable();
  }
  return r.bytes;
}

/**
 * Round 47 (critic 54a variant 1): a stored body's FULL form (its questions, key-free match hashes and images), handed
 * to `use` under the isolate's heavy slot and a send's reservation for it, as stripStored does; null when the body is
 * gone or share-core refuses it.
 */
export async function withStoredFull<T>(c: Ctx, key: string, use: (qs: QuestionJson[], mh: string[]) => Promise<T>): Promise<T | null> {
  const heavy = await acquireHeavy(c.deps.heavyQueueMs ?? HEAVY_QUEUE_WAIT_MS, async () => {}, c.deps.heavySlots);
  const held: (() => void)[] = [heavy];
  try {
    const obj = await c.r2.get(key);
    if (!obj) return null;
    const size = (obj as R2ObjectBody).size;
    held.push(reserveBytes(size * (PUBLISH_COST_PER_BYTE - 1)));
    const bytes = new Uint8Array(await (obj as R2ObjectBody).arrayBuffer()); // r2-object: a kept version's body (our own write)
    const extra = scanJson(bytes).values * PARSE_BYTES_PER_VALUE * 2 - size * (PUBLISH_COST_PER_BYTE - 1);
    if (extra > 0) held.push(reserveBytes(extra));
    const mh = c.core.matchHashes(bytes);
    if (isFail(mh)) return null;
    let qs: QuestionJson[];
    try {
      qs = questionsOf(JSON.parse(new TextDecoder().decode(bytes)) as PayloadJson);
    } catch {
      return null;
    }
    return await use(qs, mh);
  } finally {
    for (const r of held.reverse()) r();
  }
}

/** A body to stream from bytes held in memory (streamedPayload's source; 64 KB views, no copy). */
export function bytesBody(b: Uint8Array): R2ObjectBody {
  let o = 0;
  const body = new ReadableStream<Uint8Array>({
    pull(ctl) {
      if (o >= b.length) return ctl.close();
      ctl.enqueue(b.subarray(o, Math.min(o + 65536, b.length)));
      o += 65536;
    },
  });
  return { size: b.length, body } as unknown as R2ObjectBody;
}

/** The same decisions, whatever their order (round 46: a PATCH that would record what is recorded is a no-op). */
export function sameDecisions(a: StripDecisions, b: StripDecisions): boolean {
  const k = (d: StripDecisions) =>
    JSON.stringify([
      d.labels.map((x) => `${x.question}/${x.part ?? ""}/${x.choice}`).sort(),
      d.orderAcks.map((x) => `${x.question}/${x.part ?? ""}`).sort(),
      [...new Set(d.imageOk)].sort((p, q) => p - q),
      [...new Set(d.ackFormatting)].sort((p, q) => p - q),
    ]);
  return k(a) === k(b);
}
