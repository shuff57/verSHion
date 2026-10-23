// Round 16 (critic21d non-blocker 1): a scan of the RAW publish bytes before any JavaScript parse.
// JSON.parse silently keeps the last of duplicate keys and recurses without a depth limit, so the
// Worker's own read of a payload (to rewrite ids, dates and provenance) could see something other
// than what share-core validates, or exhaust memory or the stack first. The scan applies
// share-core's structural limits (depth, values, objects, duplicate keys: share-core/src/limits.rs
// defaults) to the bytes; a payload that fails them goes to share-core unparsed, whose refusal is the
// answer. It also counts the values, so the parse can be reserved for what it really costs.

export interface JsonScan {
  ok: boolean;
  /** share-core's code for the first problem (too_deep, duplicate_key, too_many_values, too_many_objects, invalid). */
  code?: string;
  values: number;
  objects: number;
  depth: number;
}

/** share-core's defaults (limits.rs): the Worker never accepts more than share-core would. */
export const SCAN_MAX_DEPTH = 16;
export const SCAN_MAX_VALUES = 200_000;
export const SCAN_MAX_OBJECTS = 60_000;
/** JS heap per parsed value (an object slot, a number or a short string), measured order of magnitude. */
export const PARSE_BYTES_PER_VALUE = 96;

const WS = new Set([0x20, 0x09, 0x0a, 0x0d]);
/** The first byte of a number, true, false or null; and the bytes such a token may contain. */
const TOKEN_START = new Set([..."-0123456789tfn"].map((c) => c.charCodeAt(0)));
const TOKEN_BYTES = new Set([..."-+.0123456789eEtruefalsn"].map((c) => c.charCodeAt(0)));

/** Scans `b` (UTF-8 JSON) without building it. Linear time; memory: the keys of the open objects only. */
export function scanJson(b: Uint8Array): JsonScan {
  let values = 0;
  let objects = 0;
  let maxDepth = 0;
  // Per open container: whether it is an object, the keys seen, and whether a key is expected next.
  const stack: { obj: boolean; keys: Set<string> | null; wantKey: boolean }[] = [];
  const fail = (code: string): JsonScan => ({ ok: false, code, values, objects, depth: maxDepth });
  const dec = new TextDecoder();
  let i = 0;
  const n = b.length;
  // Round 17 (critic22e): a byte-order mark (or anything but JSON at the top) is refused here: the
  // Worker's own parse must never accept bytes share-core refuses (TextDecoder drops a BOM).
  if (n >= 3 && b[0] === 0xef && b[1] === 0xbb && b[2] === 0xbf) return fail("bom");
  let tops = 0;
  while (i < n) {
    const ch = b[i]!;
    if (WS.has(ch) || ch === 0x3a /* : */) {
      i++;
      continue;
    }
    const top = stack[stack.length - 1];
    if (ch === 0x2c /* , */) {
      if (top?.obj) top.wantKey = true;
      i++;
      continue;
    }
    if (ch === 0x22 /* " */) {
      const start = i;
      i++;
      while (i < n && b[i] !== 0x22) i += b[i] === 0x5c /* \ */ ? 2 : 1;
      if (i >= n) return fail("invalid");
      i++;
      if (top?.obj && top.wantKey) {
        // A key: compared decoded, as share-core compares them ("a" and "a" are one key).
        let key: string;
        try {
          key = JSON.parse(dec.decode(b.subarray(start, i))) as string;
        } catch {
          return fail("invalid");
        }
        if (top.keys!.has(key)) return fail("duplicate_key");
        top.keys!.add(key);
        top.wantKey = false;
        continue;
      }
      if (!top && ++tops > 1) return fail("invalid");
      if (++values > SCAN_MAX_VALUES) return fail("too_many_values");
      continue;
    }
    if (ch === 0x7b /* { */ || ch === 0x5b /* [ */) {
      if (!top && ++tops > 1) return fail("invalid");
      if (++values > SCAN_MAX_VALUES) return fail("too_many_values");
      const obj = ch === 0x7b;
      if (obj && ++objects > SCAN_MAX_OBJECTS) return fail("too_many_objects");
      stack.push({ obj, keys: obj ? new Set() : null, wantKey: obj });
      if (stack.length > maxDepth) maxDepth = stack.length;
      if (stack.length > SCAN_MAX_DEPTH) return fail("too_deep");
      i++;
      continue;
    }
    if (ch === 0x7d /* } */ || ch === 0x5d /* ] */) {
      if (!top || top.obj !== (ch === 0x7d)) return fail("invalid");
      stack.pop();
      i++;
      continue;
    }
    // A number, true, false or null: one value up to the next delimiter, made only of the bytes those
    // tokens use (round 17: anything else, e.g. a stray letter or a control byte, is refused).
    if (!TOKEN_START.has(ch)) return fail("invalid");
    if (++values > SCAN_MAX_VALUES) return fail("too_many_values");
    const t0 = i;
    while (i < n && !WS.has(b[i]!) && b[i] !== 0x2c && b[i] !== 0x7d && b[i] !== 0x5d && b[i] !== 0x3a) {
      if (!TOKEN_BYTES.has(b[i]!)) return fail("invalid");
      i++;
    }
    const tok = String.fromCharCode(...b.subarray(t0, Math.min(i, t0 + 40)));
    if (!(tok === "true" || tok === "false" || tok === "null" || /^-?(0|[1-9][0-9]*)(\.[0-9]+)?([eE][+-]?[0-9]+)?$/.test(tok))) return fail("invalid");
    if (stack.length === 0 && ++tops > 1) return fail("invalid");
  }
  if (stack.length !== 0) return fail("invalid");
  return { ok: true, values, objects, depth: maxDepth };
}
