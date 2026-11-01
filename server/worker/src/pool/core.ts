// The ONE adapter between the pool and share-core's wasm API (server/share-core/src/wasm.rs).
//
// share-core takes UTF-8 **bytes** (a Uint8Array; a JS string is refused with `expected_bytes`) and a
// JSON options string, and returns one JSON object: `{"ok": true, ...}` or `{"ok": false, "errors":
// [{code, path, limit?, offset?, offset16?, cp?}]}`. Errors never contain input text. If share-core's
// API changes (names, option keys, result fields), this file is the only one to change.
//
// The pool never builds, edits or trusts a stripped form itself: `strip` is share-core's, with a seed
// share-core draws internally. Payload JSON is passed on as the exact canonical bytes share-core wrote
// (sliced out of its result, never re-serialised by JavaScript).
import { checkImage, type ImageInfo } from "./image.ts";
import { setFingerprintKey } from "./shared.ts";

/** The share-core wasm exports the pool uses (the package's module namespace, already initialised). */
export interface ShareCoreExports {
  validate(payload: Uint8Array, options: string): string;
  validate_for_publish(payload: Uint8Array, options: string): string;
  strip(payload: Uint8Array, options: string): string;
  /**
   * Round 6: the author-only data of the LAST successful strip in this instance, handed out once
   * (`{"ok": true, "served_orders": [...]}` or `no_author_data`). Called synchronously right after
   * strip, inside the same adapter call (no await between), so concurrent publishes cannot swap it.
   */
  strip_author_data?(): string;
  label_findings(payload: Uint8Array, options: string): string;
  publish_prepare?(payload: Uint8Array, options: string): string;
  match_hash(payload: Uint8Array, options: string): string;
  full_hash(payload: Uint8Array, options: string): string;
  validate_meta(payload: Uint8Array, options: string): string;
  schema_versions(): string;
  /** Not in share-core yet: when it lands (`{"ok", "content_type", "width", "height"}`), it replaces ./image.ts. */
  image_check?(bytes: Uint8Array, options: string): string;
  /** A fresh instance and memory from the same compiled module (after any exception from an export). */
  reinit?(): unknown;
  /** 1 when the instance is sound. */
  health?(): number;
}

// ---- traps (round 21: share-core's reinit/health contract) ----------------------------------------
// Any exception from a share-core export (a RuntimeError such as `unreachable` or an out-of-bounds
// access, or anything) leaves the instance unusable: the guard re-creates it at once (`reinit`: fresh
// instance and memory from the same compiled module, so argon2id and the author data start clean),
// fails THAT request with a plain 500 `server_error` (no retry on the new instance, nothing from the
// exception), logs only the kind, and counts the trap (flushed per hour to pool_kv; the hourly cron
// raises `wasm_trap` past WASM_TRAPS_PER_HOUR). After a trap, the next call first checks `health()`.

/** A trap, answered as a plain 500 by the pool's error handler. */
export class WasmTrap extends Error {
  constructor() {
    super("share-core trap");
    this.name = "WasmTrap";
  }
}
/** Traps an hour past which the admin is flagged (`wasm_trap`). */
export const WASM_TRAPS_PER_HOUR = 5;
const trapState = { recent: false, pending: 0 };
/** Traps counted in this isolate since the last flush (the pool flushes them to D1). */
export function takeWasmTraps(): number {
  const n = trapState.pending;
  trapState.pending = 0;
  return n;
}
function errKindOf(e: unknown): string {
  return e instanceof Error ? e.name : typeof e;
}
/** Wraps one share-core export (and argon2id) in the trap contract. */
export function guardCall<A extends unknown[], R>(life: Pick<ShareCoreExports, "reinit" | "health">, f: (...a: A) => R, name: string): (...a: A) => R {
  return (...a: A): R => {
    if (trapState.recent && life.reinit) {
      let ok = false;
      try {
        ok = life.health?.() === 1;
      } catch {
        ok = false;
      }
      if (!ok) life.reinit();
      trapState.recent = false;
    }
    try {
      return f(...a);
    } catch (e) {
      console.error(`share-core: ${name} trapped: ${errKindOf(e)}`);
      trapState.pending++;
      trapState.recent = true;
      try {
        life.reinit?.();
      } catch (e2) {
        console.error(`share-core: reinit failed: ${errKindOf(e2)}`);
      }
      throw new WasmTrap();
    }
  };
}
/** Every export of `m` guarded (reinit and health themselves stay as they are). */
export function guardExports(m: ShareCoreExports): ShareCoreExports {
  const out: Record<string, unknown> = {};
  for (const [k, v] of Object.entries(m)) out[k] = typeof v === "function" && k !== "reinit" && k !== "health" ? guardCall(m, (v as (...a: unknown[]) => unknown).bind(m), k) : v;
  return out as unknown as ShareCoreExports;
}

/** A share-core error, reduced to the fields that carry no input text. */
export interface CoreError {
  code: string;
  path: string;
  limit?: number;
  offset16?: number;
  cp?: string;
  /** What the author should do (share-core round 5 codes; never input text). */
  message?: string;
  /** share-core round 6: the marker feature found (`key_in_formatting`), whether `ack_formatting`
   *  may accept it, and that it names the key (show it to the author only). Publish errors only ever
   *  reach the publishing author. */
  reason?: string;
  ack_allowed?: boolean;
  author_only?: boolean;
  /** share-core round 8: what the author can do (share-core's own English text, never input text). */
  hint?: string;
}

/** Author-facing messages for share-core codes and notes (round 5). */
export const CORE_MESSAGES: Record<string, string> = {
  labels_keep_refused: "These answer labels give the answer away (the right column counts off one by one). Strip them instead of keeping them.",
  placeholder_key: "This copy was imported without its key. Set the True/False answer yourself: the placeholder value is not a key.",
  symbol_font_char: "This text contains a symbol-font character that cannot be converted. Retype it with a standard character.",
  object_replacement: "This text contains a placeholder for a pasted object (U+FFFC). Remove it.",
  keyless_key_material: "A question shared without its key cannot contain key material (a correct answer, a sample, an explanation or the Matching answers).",
  symbol_font_mapped: "Symbol-font characters were converted to standard Unicode.",
  locked_key: "Only the correct choice is locked, which would give the answer away. Lock none, or lock every choice whose place matters.",
  key_in_stem: "The question text (or a left item) gives the answer away (an answer line, GIFT or Cloze answer syntax, a True/False value, or a left item naming its right item). Remove it from the question.",
  key_in_formatting: "The correct choice looks different from the others (spacing, decoration, capitals or a note), which would give the answer away. Make the choices look alike.",
  hash_key_required: "The server is not configured to accept publishes yet.",
  v2_field_in_v1: "This file uses fields of a newer format. Update verSHion and export it again.",
  // Round 16 (critic21c c14): the source now shares this question without its key.
  keyless_needs_hide_key: "This copy came from a version shared without its key. Publish it with the key hidden, or set the correct answers yourself before publishing it with the key shown.",
};

/** A question (and optionally a Multipart part) by position in the payload. */
export interface Pos {
  question: number;
  part?: number;
}

export interface LabelDecision {
  question: number;
  part?: number;
  choice: "strip" | "keep";
}

export type Fail = {
  ok: false;
  errors: CoreError[];
  /** Present when the problem is an undecided label: the author's preview (their own text). */
  label_findings?: unknown[];
  /** Present when a preserved order was not acknowledged. */
  needs_order_ack?: Pos[];
  order_reasons?: unknown[];
  /** The author's preview of what strip will move (share-core round 6). */
  order_preview?: unknown[];
  formatting_findings?: unknown[];
  formatting_findings_total?: number;
};

/** A validated payload: the canonical JSON share-core wrote, and the same parsed (read-only use). */
export interface Validated {
  ok: true;
  readonly json: string;
  bytes: Uint8Array;
  payload: PayloadJson;
  dropped: unknown[];
}

export interface Stripped extends Validated {
  matchHashes: string[];
  strippedHashes: string[];
  needsOrderAck: Pos[];
  orderReasons: unknown[];
  labelsRemoved: Pos[];
  labelFindings: unknown[];
  servedOrders: unknown[];
  imagesKept: number[];
  imagesRemoved: number[];
  figuresRemoved: number[];
}

/** The wire envelope (docs/shared-pool.md "Import": `{"schema_version", "bank" | "question"}`). */
export interface PayloadJson {
  schema_version: number;
  bank?: BankJson;
  question?: QuestionJson;
}
export interface BankJson {
  id: string;
  name: string;
  questions: QuestionJson[];
  created: number;
  modified: number;
  [k: string]: unknown;
}
export interface QuestionJson {
  id: string;
  kind: Record<string, unknown>;
  image?: { file: string; width: number; height: number } | null;
  difficulty?: string | null;
  topics?: string[];
  published?: { item_id: string; qid?: string; publish_id: string; version: number };
  origin?: { item_id: string; qid?: string; publish_id: string; version: number };
  created: number;
  modified: number;
  [k: string]: unknown;
}

export interface Meta {
  title: string;
  subject: string;
  course: string;
  grade: string;
  tags: string[];
  standards: string[];
}

export interface Core {
  schema(): { min: number; max: number; stripped: number };
  /** For uploads: strict parse, limits, characters, key completeness, label decisions (hide_key). */
  validateForPublish(bytes: Uint8Array, o: { hideKey: boolean; labels: LabelDecision[]; ackFormatting?: number[] }): Validated | Fail;
  /** Validates for a hide_key publish and strips (share-core draws the shuffle seed; client provenance dropped). */
  strip(bytes: Uint8Array, o: { labels: LabelDecision[]; orderAcks: Pos[]; imageOk: number[]; ackFormatting?: number[] }): Stripped | Fail;
  /** share-core round 7: the whole publish in one call (validated full form, its match and keyed
   *  full hashes, and the stripped form when the key is hidden). */
  publishPrepare(
    bytes: Uint8Array,
    o: { hideKey: boolean; labels: LabelDecision[]; orderAcks: Pos[]; imageOk: number[]; ackFormatting?: number[] },
  ): { full: Validated; matchHashes: string[]; fullHashes: string[]; stripped: Stripped | null } | Fail;
  /** Keyed content hashes of questions, ids, dates and provenance aside (what "the same content"
   *  compares, round 12): HMAC-SHA256 with the pool hash key over a canonical JSON. */
  contentHashes(qs: QuestionJson[]): Promise<string[]>;
  /** match_hash per question (key-free; safe to compare across items). */
  matchHashes(bytes: Uint8Array): string[] | Fail;
  /** full_hash per question (a key oracle for small answer spaces: author-only, never compared). */
  fullHashes(bytes: Uint8Array): string[] | Fail;
  validateMeta(meta: unknown): Meta | Fail;
  /** Structural image check (format sniffed from the bytes); null when refused. */
  imageInfo(bytes: Uint8Array): Promise<ImageInfo | { refused: string }>;
}

const enc = new TextEncoder();

/**
 * The raw JSON text of top-level `key` in a JSON object `json`, or null. A small scanner (strings
 * and nesting only), so the payload share-core wrote is reused byte for byte: JavaScript's
 * JSON.stringify would reorder integer-like keys and print some numbers differently.
 */
export function rawField(json: string, key: string): string | null {
  const want = JSON.stringify(key);
  let i = 0;
  const n = json.length;
  const ws = () => {
    while (i < n && " \t\r\n".includes(json[i]!)) i++;
  };
  const skipString = () => {
    // json[i] === '"'
    i++;
    while (i < n) {
      const c = json[i]!;
      if (c === "\\") i += 2;
      else if (c === '"') {
        i++;
        return;
      } else i++;
    }
    throw new Error("unterminated string");
  };
  const skipValue = () => {
    ws();
    const c = json[i];
    if (c === '"') return skipString();
    if (c === "{" || c === "[") {
      let depth = 0;
      while (i < n) {
        const d = json[i]!;
        if (d === '"') {
          skipString();
          continue;
        }
        if (d === "{" || d === "[") depth++;
        else if (d === "}" || d === "]") {
          depth--;
          if (depth === 0) {
            i++;
            return;
          }
        }
        i++;
      }
      throw new Error("unbalanced");
    }
    while (i < n && !",}]".includes(json[i]!)) i++;
  };
  ws();
  if (json[i] !== "{") return null;
  i++;
  for (;;) {
    ws();
    if (json[i] === "}") return null;
    const ks = i;
    skipString();
    const k = json.slice(ks, i);
    ws();
    if (json[i] !== ":") return null;
    i++;
    ws();
    const vs = i;
    skipValue();
    if (k === want) return json.slice(vs, i);
    ws();
    if (json[i] === ",") i++;
    else return null;
  }
}

function cleanErrors(raw: unknown): CoreError[] {
  if (!Array.isArray(raw)) return [{ code: "share_core_error", path: "" }];
  return raw.slice(0, 100).map((e) => {
    const x = (e ?? {}) as Record<string, unknown>;
    const out: CoreError = { code: typeof x.code === "string" ? x.code : "invalid", path: typeof x.path === "string" ? x.path : "" };
    if (typeof x.limit === "number") out.limit = x.limit;
    if (typeof x.offset16 === "number") out.offset16 = x.offset16;
    if (typeof x.cp === "string" && /^U\+[0-9A-F]{4,6}$/.test(x.cp)) out.cp = x.cp;
    if (CORE_MESSAGES[out.code]) out.message = CORE_MESSAGES[out.code];
    if (typeof x.reason === "string" && /^[a-z0-9_]{1,64}$/.test(x.reason)) out.reason = x.reason;
    if (typeof x.ack_allowed === "boolean") out.ack_allowed = x.ack_allowed;
    if (x.author_only === true) out.author_only = true;
    // A short fixed English hint; anything that looks like more than that is not passed on.
    if (typeof x.hint === "string" && x.hint.length <= 500) out.hint = x.hint;
    return out;
  });
}

function fail(r: Record<string, unknown>): Fail {
  const f: Fail = { ok: false, errors: cleanErrors(r.errors) };
  if (Array.isArray(r.label_findings)) f.label_findings = r.label_findings;
  if (Array.isArray(r.needs_order_ack)) f.needs_order_ack = r.needs_order_ack as Pos[];
  if (Array.isArray(r.order_reasons)) f.order_reasons = r.order_reasons;
  // share-core round 8: refusals carry the author's previews inline (author only; bounded here).
  if (Array.isArray(r.order_preview)) f.order_preview = (r.order_preview as unknown[]).slice(0, 2000);
  if (Array.isArray(r.formatting_findings)) f.formatting_findings = (r.formatting_findings as unknown[]).slice(0, 200);
  if (typeof r.formatting_findings_total === "number") f.formatting_findings_total = r.formatting_findings_total;
  return f;
}

function opts(o: Record<string, unknown>): string {
  return JSON.stringify(o);
}

/** Labels as share-core expects them (`part` omitted when absent). */
function labelsOut(labels: LabelDecision[]): LabelDecision[] {
  return labels.map((l) => (l.part === undefined ? { question: l.question, choice: l.choice } : { question: l.question, part: l.part, choice: l.choice }));
}

function posOut(p: Pos[]): Pos[] {
  return p.map((x) => (x.part === undefined ? { question: x.question } : { question: x.question, part: x.part }));
}

/**
 * The adapter's own configuration. `hashKey`: the server's secret for share-core's keyed
 * `full_hash` (HMAC-SHA256; hex, at least 32 bytes), derived from the POOL_HASH_KEY secret by HKDF
 * (deriveHashKey). Without it every full hash is refused (`hash_key_required`).
 */
export interface CoreConfig {
  hashKey: string | null;
}

/** HKDF-SHA256 of the POOL_HASH_KEY secret for one purpose (e.g. "full-hash"), as hex (32 bytes). */
export async function deriveHashKey(secret: string, purpose: string): Promise<string> {
  const ikm = await crypto.subtle.importKey("raw", new TextEncoder().encode(secret), "HKDF", false, ["deriveBits"]);
  const bits = await crypto.subtle.deriveBits({ name: "HKDF", hash: "SHA-256", salt: new TextEncoder().encode("vershion-pool/v1"), info: new TextEncoder().encode(`pool-hash-key:${purpose}`) }, ikm, 256);
  return [...new Uint8Array(bits)].map((b) => b.toString(16).padStart(2, "0")).join("");
}

export function makeCore(m0: ShareCoreExports, cfg: CoreConfig = { hashKey: null }): Core {
  // (Round 21: every export behind the trap guard.)
  const m = guardExports(m0);
  // Round 13: the Worker's own content fingerprints are keyed by the same derived key.
  if (cfg.hashKey !== null) setFingerprintKey(cfg.hashKey);
  const call = (out: string): Record<string, unknown> => {
    const r = JSON.parse(out) as Record<string, unknown>;
    if (typeof r !== "object" || r === null) throw new Error("share-core returned a non-object");
    return r;
  };
  const validated = (out: string, r: Record<string, unknown>): Validated => {
    const json = rawField(out, "payload");
    if (json === null) throw new Error("share-core result has no payload");
    // Only the bytes are kept (the JSON text is decoded on demand): a 4 MB payload is not held twice.
    const bytes = enc.encode(json);
    return {
      ok: true,
      get json() {
        return new TextDecoder().decode(bytes);
      },
      bytes,
      payload: r.payload as PayloadJson,
      // Notes (round 5: symbol_font_mapped) carry an author-facing message too.
      // (share-core names a note by `reason`; older packages by `code`.)
      dropped: ((r.dropped as unknown[]) ?? []).map((d) => {
        if (!d || typeof d !== "object") return d;
        const x = d as { code?: unknown; reason?: unknown };
        const k = typeof x.reason === "string" ? x.reason : typeof x.code === "string" ? x.code : null;
        return k && CORE_MESSAGES[k] ? { ...(d as object), message: CORE_MESSAGES[k] } : d;
      }),
    };
  };
  const hashes = (out: string): string[] | Fail => {
    const r = call(out);
    if (r.ok !== true) return fail(r);
    const h = r.hashes;
    if (!Array.isArray(h) || !h.every((x) => typeof x === "string" && /^[0-9a-f]{64}$/.test(x))) throw new Error("share-core hashes malformed");
    return h as string[];
  };
  const strippedOf = (out: string, r: Record<string, unknown>, authorData: Record<string, unknown> | null): Stripped => {
    const v = validated(out, r);
    const arr = <T>(k: string) => (Array.isArray(r[k]) ? (r[k] as T[]) : []);
    const s: Stripped = {
      // (Round 29: v's fields one by one. A spread calls v's `json` getter and kept a decoded copy of the
      // whole stripped body for the rest of the publish, against "only the bytes are kept" above.)
      ok: true,
      get json() {
        return v.json;
      },
      bytes: v.bytes,
      payload: v.payload,
      dropped: v.dropped,
      matchHashes: arr<string>("match_hashes"),
      strippedHashes: arr<string>("stripped_hashes"),
      needsOrderAck: arr<Pos>("needs_order_ack"),
      orderReasons: arr("order_reasons"),
      labelsRemoved: arr<Pos>("labels_removed"),
      labelFindings: arr("label_findings"),
      // The orders strip served are author-only (strip_author_data): they stay in the publishing
      // author's response and sameServed's comparison, nowhere else.
      servedOrders: (() => {
        if (authorData && authorData.ok === true && Array.isArray(authorData.served_orders)) return authorData.served_orders as unknown[];
        const ao = r.author_only as { served_orders?: unknown } | undefined;
        return ao && Array.isArray(ao.served_orders) ? (ao.served_orders as unknown[]) : arr("served_orders");
      })(),
      imagesKept: arr<number>("images_kept"),
      imagesRemoved: arr<number>("images_removed"),
      figuresRemoved: arr<number>("figures_removed"),
    };
    const n = v.payload.bank?.questions.length ?? (v.payload.question ? 1 : 0);
    if (s.matchHashes.length !== n || s.strippedHashes.length !== n) throw new Error("share-core strip hashes do not match the question count");
    return s;
  };
  let schema: { min: number; max: number; stripped: number } | null = null;
  return {
    schema() {
      if (!schema) {
        const r = JSON.parse(m.schema_versions()) as { min: number; max: number; stripped: number };
        schema = { min: r.min, max: r.max, stripped: r.stripped };
      }
      return schema;
    },
    async contentHashes(qs) {
      if (cfg.hashKey === null) throw new Error("pool hash key not configured (POOL_HASH_KEY)");
      const raw = new Uint8Array(cfg.hashKey.match(/../g)!.map((h) => parseInt(h, 16)));
      const key = await crypto.subtle.importKey("raw", raw, { name: "HMAC", hash: "SHA-256" }, false, ["sign"]);
      return Promise.all(
        qs.map(async (q) => {
          const sig = await crypto.subtle.sign("HMAC", key, enc.encode(`vershion/content/v2\n${canonical(contentOf(q))}`));
          return [...new Uint8Array(sig)].map((b) => b.toString(16).padStart(2, "0")).join("");
        }),
      );
    },
    validateForPublish(bytes, o) {
      const out = m.validate_for_publish(bytes, opts({ hide_key: o.hideKey, labels: labelsOut(o.labels), ack_formatting: o.ackFormatting ?? [] }));
      const r = call(out);
      return r.ok === true ? validated(out, r) : fail(r);
    },
    strip(bytes, o) {
      // Round 12: the client's `origin` is never served (the Worker owns provenance).
      const out = m.strip(bytes, opts({ labels: labelsOut(o.labels), order_acks: posOut(o.orderAcks), image_ok: o.imageOk, ack_formatting: o.ackFormatting ?? [], drop_client_provenance: true }));
      // Round 6: the author-only data is taken NOW, synchronously, in this same call: nothing can run
      // between strip and strip_author_data (no await), so two publishes never swap it.
      const authorData = typeof m.strip_author_data === "function" ? (JSON.parse(m.strip_author_data()) as Record<string, unknown>) : null;
      const r = call(out);
      if (r.ok !== true) return fail(r);
      return strippedOf(out, r, authorData);
    },
    publishPrepare(bytes, o) {
      // share-core round 7: the whole publish in ONE read and validation (validate_for_publish,
      // strip when the key is hidden, match and keyed full hashes). No key, no hashing.
      if (cfg.hashKey === null) throw new Error("pool hash key not configured (POOL_HASH_KEY)");
      if (typeof m.publish_prepare !== "function") throw new Error("share-core has no publish_prepare (round 7 package required)");
      const out = m.publish_prepare(
        bytes,
        opts({ hide_key: o.hideKey, labels: labelsOut(o.labels), order_acks: posOut(o.orderAcks), image_ok: o.imageOk, ack_formatting: o.ackFormatting ?? [], drop_client_provenance: true, hash_key: cfg.hashKey }),
      );
      // The author-only data of the strip inside it is taken NOW, synchronously (no await between).
      const authorData = o.hideKey && typeof m.strip_author_data === "function" ? (JSON.parse(m.strip_author_data()) as Record<string, unknown>) : null;
      const r = call(out);
      if (r.ok !== true) return fail(r);
      const full = validated(out, r);
      const arr = (k: string) => (Array.isArray(r[k]) && (r[k] as unknown[]).every((x) => typeof x === "string" && /^[0-9a-f]{64}$/.test(x)) ? (r[k] as string[]) : null);
      const matchHashes = arr("match_hashes");
      const fullHashes = arr("full_hashes");
      const n = full.payload.bank?.questions.length ?? (full.payload.question ? 1 : 0);
      if (!matchHashes || !fullHashes || matchHashes.length !== n || fullHashes.length !== n) throw new Error("share-core publish_prepare hashes malformed");
      let stripped: Stripped | null = null;
      if (o.hideKey) {
        const sOut = rawField(out, "stripped");
        if (sOut === null || !r.stripped || typeof r.stripped !== "object") throw new Error("share-core publish_prepare has no stripped form");
        stripped = strippedOf(sOut, r.stripped as Record<string, unknown>, authorData);
      }
      return { full, matchHashes, fullHashes, stripped };
    },
    matchHashes: (bytes) => hashes(m.match_hash(bytes, "")),
    fullHashes: (bytes) => {
      // No key, no hashing (an unkeyed full hash of a hidden-key question is a guessable oracle).
      if (cfg.hashKey === null) throw new Error("pool hash key not configured (POOL_HASH_KEY)");
      return hashes(m.full_hash(bytes, opts({ hash_key: cfg.hashKey })));
    },
    validateMeta(meta) {
      const r = call(m.validate_meta(enc.encode(JSON.stringify(meta ?? {})), ""));
      if (r.ok !== true) return fail(r);
      const x = r.meta as Meta;
      return { title: x.title ?? "", subject: x.subject ?? "", course: x.course ?? "", grade: x.grade ?? "", tags: x.tags ?? [], standards: x.standards ?? [] };
    },
    async imageInfo(bytes) {
      if (typeof m.image_check === "function") {
        const r = call(m.image_check(bytes, ""));
        if (r.ok !== true) return { refused: cleanErrors(r.errors)[0]?.code ?? "image_refused" };
        return { contentType: r.content_type as ImageInfo["contentType"], width: Number(r.width), height: Number(r.height) };
      }
      return checkImage(bytes);
    },
  };
}

/** True when `x` is a share-core refusal. */
export function isFail(x: unknown): x is Fail {
  return typeof x === "object" && x !== null && (x as { ok?: unknown }).ok === false;
}

/** The questions of a payload, in order. */
export function questionsOf(p: PayloadJson): QuestionJson[] {
  return p.bank ? p.bank.questions : p.question ? [p.question] : [];
}

/** The kind tag of a question (`"Mc"`, `"Tf"`, ...). */
export function kindTag(q: QuestionJson): string {
  return Object.keys(q.kind ?? {})[0] ?? "";
}

/** A question with what is not content dropped (ids, times, provenance). */
function contentOf(q: QuestionJson): unknown {
  const { id: _i, created: _c, modified: _m, origin: _o, published: _p, ...rest } = q as QuestionJson & Record<string, unknown>;
  return rest;
}
/** JSON with object keys sorted (a stable encoding for hashing). */
export function canonical(v: unknown): string {
  if (Array.isArray(v)) return `[${v.map(canonical).join(",")}]`;
  if (v && typeof v === "object")
    return `{${Object.keys(v as object)
      .filter((k) => (v as Record<string, unknown>)[k] !== undefined)
      .sort()
      .map((k) => `${JSON.stringify(k)}:${canonical((v as Record<string, unknown>)[k])}`)
      .join(",")}}`;
  return JSON.stringify(v ?? null);
}
