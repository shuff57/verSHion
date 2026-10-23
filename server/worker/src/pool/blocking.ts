// Block plans for privacy purges and DMCA takedowns (critic D2): which question texts (match_hash)
// and images a block covers, chosen per question by the admin, never a common question by default,
// with a preview of every item the block would hold, confirmed by a short-lived token so the
// confirm acts on exactly what was previewed.
import { b64url, fromB64url, hmac, timingSafeEqual, utf8 } from "../../../auth/src/crypto.ts";
import { ID_RE, limitOf, newId, notifyHeld, PoolError, qKeyEq, qKeyIn, type Ctx } from "./shared.ts";
import { itemServesBlob, SERVED_FORM } from "./served.ts";

/** What a question's block covers (round 12 M4): its student-visible skeleton (exact and normalised),
 *  its whole content, its explanation, or its model / accepted answers (all normalised). */
export type BlockScope = "default" | "skeleton" | "stem" | "full" | "explanation" | "answer" | `part:${number}`;
export const BLOCK_SCOPES: BlockScope[] = ["default", "skeleton", "stem", "full", "explanation", "answer"];
/** Round 16 (critic21c B5): "part:<k>" blocks only part k of a Multipart (its stem, explanation and
 *  answers, wherever they appear: a standalone question or another Multipart's part). */
const isScope = (v: unknown): v is BlockScope => BLOCK_SCOPES.includes(v as BlockScope) || (typeof v === "string" && /^part:(0|[1-9][0-9]{0,2})$/.test(v));
/** Round 13: "skeleton" = the student-visible skeleton (exact and normalised: stem, choices, columns);
 *  "stem" = the skeleton AND the normalised stem alone (any answer, any explanation); "default" (a
 *  privacy block's default) = "stem" and the full content. Student data may be anywhere in a
 *  question, so a privacy block holds every question with that stem unless the admin narrows it
 *  (to "explanation", "answer" or "full") because the data is only there. */
const STEM_KEYS = (q: { match_hash: string; bk_norm: string | null; bk_stem: string | null }) => [q.match_hash, q.bk_norm, q.bk_stem];
/** Other authors' visible items sharing a stem from which it counts as generic (the preview warns). */
export const GENERIC_STEM_AUTHORS = 3;
/** Kinds whose skeleton is the bare stem: a "stem" block of one is stem-wide (any answer, any explanation). */
const STEM_ONLY = new Set(["Tf", "Short", "Blank"]);

export interface BlockSelection {
  /** Question positions in the item's current publish; absent = every distinctive question. */
  questions?: number[];
  /** Per position, what its block covers (default: "stem", except a privacy block of a Tf, Short or
   *  Blank question, whose default is "full": a generic stem must not hold unrelated questions). */
  scopes?: Record<string, BlockScope>;
  /** Image hashes; absent = every image of the item. */
  images?: string[];
  /** Round 23 (critic30b A2): a takedown of a purged item (a ghost) selects NOTHING by default; `all`
   *  (sent as `questions: "all"`) applies the live default to every position, explicitly. */
  all?: boolean;
}

export interface BlockPlan {
  item_id: string;
  match: {
    pos: number;
    hash: string;
    /** The block keys this position's block holds (shared.ts QKEY_COLS), by its scope. */
    keys: string[];
    scope: BlockScope;
    /** A "stem" block of a stem-only kind: it holds every question with this stem, whatever its answer or explanation. */
    stem_wide: boolean;
    /** Up to 20 other items a selected position's block alone would hold. */
    affects?: { item_id: string; title: string }[];
    other_authors: number;
    introduced: boolean;
    selected: boolean;
    suggested?: boolean;
    /** Round 16: the item (and author) that had this content first, when not this one. */
    first?: { item_id: string; author_id: string };
  }[];
  blob: { sha256: string; other_authors: number; introduced: boolean; selected: boolean; suggested?: boolean }[];
  /** Other items (any state) the block would hold: they use a selected question or SERVE a selected image (first AFFECTED_LIST). */
  affected: { item_id: string; author_id: string; title: string; via: "question" | "image" }[];
  /** How many there are in all: more than AFFECTED_LIST needs an explicit confirm_large (fails closed). */
  affected_total: number;
  /** Round 23: planned from a purged item's ghost; nothing selected yet (the admin must choose). */
  ghost?: boolean;
  selection_required?: boolean;
}

export const PREVIEW_MS = 10 * 60_000;
/** Affected items listed (and held in the confirm's own batch); the rest are held in batches after it. */
export const AFFECTED_LIST = 500;

/**
 * SQL: the snapshot a hold keeps of its cause at the admin action (round 11 P3): the item's current
 * publish and the positions of its questions in the JSON array `mh`. Notices show it, never a
 * re-evaluation against later content (no block-membership probe).
 */
export const holdSnapshot = (item: string, mh: string) =>
  `json_object('publish_id', (SELECT current_publish_id FROM items WHERE id = ${item}),
               'positions', (SELECT json_group_array(iq.pos) FROM item_questions iq WHERE iq.publish_id = (SELECT current_publish_id FROM items WHERE id = ${item}) AND ${qKeyIn("iq", `(SELECT value FROM json_each(${mh}))`)}))`;

/**
 * SQL condition on item row `i`: it uses a question in the JSON array `mh` (any retained publish) or
 * SERVES an image in the JSON array `bh` (served.ts): what a block holds.
 */
export const usesBlocked = (mh: string, bh: string) =>
  `(EXISTS (SELECT 1 FROM item_publishes p JOIN item_questions iq ON iq.publish_id = p.publish_id WHERE p.item_id = i.id AND ${qKeyIn("iq", `(SELECT value FROM json_each(${mh}))`)})
    OR ${itemServesBlob("i.id", (sha) => `${sha} IN (SELECT value FROM json_each(${bh}))`)})`;

/** The plan for blocking `itemId` with `sel`, as of now. */
export async function blockPlan(c: Ctx, itemId: string, sel: BlockSelection, kind: "privacy" | "dmca" = "dmca", rightful: string | null = null): Promise<BlockPlan> {
  let item = await c.db
    .prepare(`SELECT author_id, current_publish_id FROM items WHERE id = ?1 AND purged_at IS NULL`)
    .bind(itemId)
    .first<{ author_id: string; current_publish_id: string | null }>();
  // Round 23 (critic30b A2): a takedown of an item purged before its notice is planned from its ghost
  // (the keyed block keys its last ghost publish kept) by the SAME rules as a live item: the admin's
  // selection, scopes and block_common_k, the same preview. With no selection NOTHING is selected
  // (`selection_required`): the admin picks positions, or sends `questions: "all"` for the default.
  let ghost = false;
  if (!item) {
    const gone = kind === "dmca" ? await c.db.prepare(`SELECT author_id FROM items WHERE id = ?1 AND purged_at IS NOT NULL`).bind(itemId).first<{ author_id: string }>() : null;
    if (!gone) throw new PoolError(404, "not_found");
    const gp = await c.db.prepare(`SELECT publish_id AS p FROM ghost_publishes WHERE item_id = ?1 ORDER BY at DESC, publish_id DESC LIMIT 1`).bind(itemId).first<{ p: string }>();
    if (!gp) return { item_id: itemId, match: [], blob: [], affected: [], affected_total: 0, ghost: true, selection_required: false };
    item = { author_id: gone.author_id, current_publish_id: gp.p };
    ghost = true;
  }
  const k = await limitOf(c, "*", "block_common_k");
  // Each question's seq (when THIS content first appeared under its qid, round 6 F2): an item's own
  // introductions are the default.
  const qs = await c.db
    .prepare(
      ghost
        ? `SELECT gq.pos, gq.match_hash, gq.seq, COALESCE(gq.qkind, '') AS qkind, gq.bk_norm, gq.bk_stem, gq.bk_full, gq.bk_expl, gq.bk_ans FROM ghost_questions gq WHERE gq.publish_id = ?1 ORDER BY gq.pos`
        : `SELECT iq.pos, iq.match_hash, iq.seq, iq.qkind, iq.bk_norm, iq.bk_stem, iq.bk_full, iq.bk_expl, iq.bk_ans FROM item_questions iq WHERE iq.publish_id = ?1 ORDER BY iq.pos`,
    )
    .bind(item.current_publish_id)
    .all<{ pos: number; match_hash: string; seq: number; qkind: string; bk_norm: string | null; bk_stem: string | null; bk_full: string | null; bk_expl: string | null; bk_ans: string | null }>();
  // Each Multipart part's keys (round 16 B5).
  const pk = await c.db
    .prepare(`SELECT pos, part, key FROM ${ghost ? "ghost_block_keys" : "item_part_keys"} WHERE publish_id = ?1 ORDER BY pos, part, key`)
    .bind(item.current_publish_id)
    .all<{ pos: number; part: number; key: string }>();
  const partsOf = (pos: number, pred: (key: string, part: number) => boolean) => pk.results.filter((x) => x.pos === pos && pred(x.key, x.part)).map((x) => x.key);
  // Each position's scope and keys (round 12 M4).
  for (const [k, v] of Object.entries(sel.scopes ?? {})) {
    const pos = Number(k);
    if (!Number.isInteger(pos) || pos < 0 || pos >= qs.results.length || !isScope(v)) throw new PoolError(400, "invalid_request", { field: "scopes" });
  }
  const scopeOf = (q: (typeof qs.results)[number]): BlockScope => sel.scopes?.[String(q.pos)] ?? (kind === "privacy" ? "default" : "skeleton");
  const keysOf = (q: (typeof qs.results)[number], sc: BlockScope): string[] => {
    // (round 13 scopes; see BlockScope)
    // A Multipart's parts (round 16 B5): the default and "stem" cover every part STEM (like a
    // question's own: an answer or explanation key alone is too common to hold by default);
    // "explanation" / "answer" the parts' too; "part:<k>" all of one part.
    const part = /^part:(\d+)$/.exec(sc);
    const ks = part
      ? partsOf(q.pos, (_k, p) => p === Number(part[1]))
      : sc === "default"
        ? [...STEM_KEYS(q), q.bk_full, ...partsOf(q.pos, (k) => k.startsWith("s:"))]
        : sc === "stem"
          ? [...STEM_KEYS(q), ...partsOf(q.pos, (k) => k.startsWith("s:"))]
          : sc === "skeleton"
            ? [q.match_hash, q.bk_norm]
            : sc === "full"
              ? [q.bk_full]
              : sc === "explanation"
                ? [q.bk_expl, ...partsOf(q.pos, (k) => k.startsWith("e:"))]
                : [q.bk_ans, ...partsOf(q.pos, (k) => k.startsWith("a:"))];
    const out = ks.filter((x): x is string => typeof x === "string" && x !== "");
    if (out.length === 0) throw new PoolError(400, "invalid_request", { field: "scopes", pos: q.pos, message: `Question ${q.pos} has no ${sc} to block.` });
    return out;
  };
  const posKeys = qs.results.map((q) => ({ scope: scopeOf(q), keys: keysOf(q, scopeOf(q)) }));
  const allKeys = [...new Set(posKeys.flatMap((x) => x.keys))];
  const maxOf = (m: Map<string, { n: number; first: number | null }>, keys: string[]) =>
    keys.reduce<{ n: number; first: number | null }>((a, k) => {
      const v = m.get(k);
      return v ? { n: Math.max(a.n, v.n), first: a.first === null ? v.first : v.first === null ? a.first : Math.min(a.first, v.first) } : a;
    }, { n: 0, first: null });
  // Only what non-authors were SERVED (served.ts): an image only in a hidden publish's full form is
  // never blocked, so no block or hold can turn a guess at it into an oracle. Its seq is the first
  // publish of THIS item that served it (never the item's creation time).
  const imgs = await c.db
    .prepare(
      ghost
        ? // (a ghost's served images: its image units, the content hash, not the pixel key)
          `SELECT gu.key AS sha256, MIN(COALESCE(gu.seq, 0)) AS seq FROM ghost_units gu JOIN ghost_publishes gp ON gp.publish_id = gu.publish_id
            WHERE gp.item_id = ?1 AND gu.kind = 'image' AND gu.key NOT LIKE 'px:%' GROUP BY gu.key ORDER BY gu.key`
        : `SELECT ib.sha256, MIN(ib.seq) AS seq FROM item_blobs ib JOIN item_publishes p ON p.publish_id = ib.publish_id
        WHERE p.item_id = ?1 AND ${SERVED_FORM()} GROUP BY ib.sha256 ORDER BY ib.sha256`,
    )
    .bind(itemId)
    .all<{ sha256: string; seq: number }>();
  // How many OTHER authors' VISIBLE items (live, public) use each question / serve each image, and
  // the earliest of them: unlisted or removed items (sock puppets) never influence the default.
  const visible = `i.removed_at IS NULL AND i.visibility = 'public' AND i.purged_at IS NULL AND i.author_id <> ?2`;
  const stats = async (sql: string, hashes: string[]) => {
    if (hashes.length === 0) return new Map<string, { n: number; first: number | null }>();
    const r = await c.db.prepare(sql).bind(JSON.stringify(hashes), item.author_id).all<{ h: string; n: number; first: number | null }>();
    return new Map(r.results.map((x) => [x.h, { n: x.n, first: x.first }]));
  };
  const mCommon = await stats(
    `SELECT k.value AS h, COUNT(DISTINCT i.author_id) AS n, MIN(iq.seq) AS first FROM json_each(?1) k CROSS JOIN item_questions iq ON ${qKeyEq("iq", "k.value")}
       JOIN items i ON i.current_publish_id = iq.publish_id
      WHERE ${visible} GROUP BY k.value`,
    allKeys,
  );
  const bCommon = await stats(
    `SELECT ib.sha256 AS h, COUNT(DISTINCT i.author_id) AS n, MIN(ib.seq) AS first FROM item_blobs ib
       JOIN items i ON i.current_publish_id = ib.publish_id JOIN item_publishes p ON p.publish_id = ib.publish_id
      WHERE ib.sha256 IN (SELECT value FROM json_each(?1)) AND ${visible} AND ${SERVED_FORM()} GROUP BY ib.sha256`,
    imgs.results.map((x) => x.sha256),
  );
  // A privacy block (round 9) by default selects only what no OTHER author uses on their own: any
  // item, any visibility or state, except copies that trace to this item (their author was served
  // it, or their question names it as its origin or in its chain), which the block is meant to
  // hold. A question or image someone else has independently is never selected unless the admin
  // selects it. K (block_common_k) applies to DMCA blocks only.
  const notCopy = `NOT EXISTS (SELECT 1 FROM imports im WHERE im.user_id = i.author_id AND im.item_id = ?3)
                   AND NOT EXISTS (SELECT 1 FROM item_publishes p2 JOIN item_questions q2 ON q2.publish_id = p2.publish_id
                                    WHERE p2.item_id = i.id AND (q2.origin_item_id = ?3 OR instr(q2.chain_json, ?3) > 0))`;
  const anyOther = async (sql: string, hashes: string[]) =>
    kind !== "privacy" || hashes.length === 0
      ? new Map<string, number>()
      : new Map((await c.db.prepare(sql).bind(JSON.stringify(hashes), item.author_id, itemId).all<{ h: string; n: number }>()).results.map((x) => [x.h, x.n]));
  const mAny = await anyOther(
    `SELECT k.value AS h, COUNT(DISTINCT i.author_id) AS n FROM json_each(?1) k CROSS JOIN item_questions iq ON ${qKeyEq("iq", "k.value")} JOIN item_publishes p ON p.publish_id = iq.publish_id JOIN items i ON i.id = p.item_id
      WHERE i.author_id <> ?2 AND i.purged_at IS NULL AND ${notCopy} GROUP BY k.value`,
    allKeys,
  );
  const bAny = await anyOther(
    `SELECT ib.sha256 AS h, COUNT(DISTINCT i.author_id) AS n FROM item_blobs ib JOIN item_publishes p ON p.publish_id = ib.publish_id JOIN items i ON i.id = p.item_id
      WHERE ib.sha256 IN (SELECT value FROM json_each(?1)) AND i.author_id <> ?2 AND i.purged_at IS NULL AND ${SERVED_FORM()} AND ${notCopy} GROUP BY ib.sha256`,
    imgs.results.map((x) => x.sha256),
  );
  // A takedown naming the rightful item (round 10 L2): the infringing copies of the rightful item's
  // own material are selected by default (copied from it is exactly what is infringing).
  const rHashes = new Set<string>(
    rightful && ID_RE.test(rightful)
      ? [
          ...(
            await c.db
              .prepare(`SELECT DISTINCT j.value AS h FROM item_publishes rp JOIN item_questions rq ON rq.publish_id = rp.publish_id, json_each(json_array(rq.match_hash, rq.bk_norm, rq.bk_full, rq.bk_expl, rq.bk_ans)) j
                 WHERE rp.item_id = ?1 AND rp.status IN ('current', 'previous') AND j.value IS NOT NULL`)
              .bind(rightful)
              .all<{ h: string }>()
          ).results.map((x) => x.h),
          ...(await c.db.prepare(`SELECT DISTINCT sb.sha256 AS h FROM item_publishes sp JOIN item_blobs sb ON sb.publish_id = sp.publish_id WHERE sp.item_id = ?1 AND ${SERVED_FORM("sb", "sp")}`).bind(rightful).all<{ h: string }>()).results.map((x) => x.h),
        ]
      : [],
  );
  // Round 10: a privacy block has NO default selection: the admin ticks the questions and images that
  // hold student data (the preview marks `suggested` what only this item has). DMCA keeps its default.
  const suggestedOf = (h: string, any: Map<string, number>) => (any.get(h) ?? 0) === 0;
  const byDefault = (h: string, other: number, copied: boolean, _any: Map<string, number>) => (kind === "privacy" ? false : other < k && (!copied || rHashes.has(h)));
  if (sel.questions && !sel.questions.every((p) => Number.isInteger(p) && p >= 0 && p < qs.results.length)) throw new PoolError(400, "invalid_request", { field: "questions" });
  const known = new Set(imgs.results.map((x) => x.sha256));
  if (sel.images && !sel.images.every((h) => known.has(h))) throw new PoolError(400, "invalid_request", { field: "images" });
  // Default: only what this item introduced (no visible item of another author had it first) and
  // what is distinctive (fewer than K other authors). An explicit selection overrides both.
  // Who published each question's content first (any item, any visibility), for the preview.
  const firstOf = new Map<number, { item_id: string; author_id: string }>();
  if (kind === "privacy")
    for (const [i, q] of qs.results.entries()) {
      if (i >= 200) break; // (bounded like `affects`)
      const r = await c.db
        .prepare(
          `SELECT i.id AS item_id, i.author_id FROM json_each(?1) k CROSS JOIN item_questions iq ON ${qKeyEq("iq", "k.value")}
             JOIN item_publishes p ON p.publish_id = iq.publish_id JOIN items i ON i.id = p.item_id
            WHERE i.purged_at IS NULL ORDER BY iq.seq, i.id LIMIT 1`,
        )
        .bind(JSON.stringify(posKeys[i]!.keys))
        .first<{ item_id: string; author_id: string }>();
      if (r && r.item_id !== itemId) firstOf.set(q.pos, r);
    }
  const match: BlockPlan["match"] = qs.results.map((q, i) => {
    const { scope, keys } = posKeys[i]!;
    const st = maxOf(mCommon, keys);
    const other = st.n;
    const copied = st.first != null && st.first < q.seq;
    const anyN = Math.max(0, ...keys.map((k) => mAny.get(k) ?? 0));
    const stemWide = keys.some((k) => k.startsWith("s:")) || (scope === "skeleton" && STEM_ONLY.has(q.qkind));
    // A stem many other authors' visible items share is generic: the preview says so and suggests a
    // narrower scope (the data is then only in the explanation or the answer).
    const genericN = q.bk_stem ? (mCommon.get(q.bk_stem)?.n ?? 0) : 0;
    const generic = stemWide && genericN >= GENERIC_STEM_AUTHORS;
    return {
      pos: q.pos,
      hash: q.match_hash,
      keys,
      scope,
      stem_wide: stemWide,
      ...(generic ? { generic_stem: true, generic_authors: genericN, suggest_scope: q.bk_expl ? "explanation" : q.bk_ans ? "answer" : "full" } : {}),
      other_authors: kind === "privacy" ? anyN : other,
      introduced: !copied,
      selected: sel.questions ? sel.questions.includes(q.pos) : ghost && !sel.all ? false : byDefault(q.match_hash, other, copied, mAny),
      // Round 14: student data is suggested whoever else pasted it, unless the stem is generic; round
      // 16 (critic21d 3): and only where this item introduced it (another author had it first: the
      // preview names who, and the admin decides).
      ...(kind === "privacy" ? { suggested: !generic && !copied, ...(copied && firstOf.get(q.pos) ? { first: firstOf.get(q.pos) } : {}) } : {}),
    };
  });
  const blob = imgs.results.map((b) => {
    const st = bCommon.get(b.sha256);
    const other = st?.n ?? 0;
    const copied = st?.first != null && st.first < b.seq;
    return { sha256: b.sha256, other_authors: kind === "privacy" ? (bAny.get(b.sha256) ?? 0) : other, introduced: !copied, selected: sel.images ? sel.images.includes(b.sha256) : ghost && !sel.all ? false : byDefault(b.sha256, other, copied, bAny), ...(kind === "privacy" ? { suggested: suggestedOf(b.sha256, bAny) && !copied } : {}) };
  });
  const mh = JSON.stringify([...new Set(match.filter((x) => x.selected).flatMap((x) => x.keys))]);
  const bh = JSON.stringify(blob.filter((x) => x.selected).map((x) => x.sha256));
  const where = `i.id <> ?1 AND i.purged_at IS NULL AND ${usesBlocked("?2", "?3")}`;
  const listMax = c.deps.affectedList ?? AFFECTED_LIST;
  const aff = await c.db
    .prepare(
      `SELECT i.id AS item_id, i.author_id, i.title,
              CASE WHEN EXISTS (SELECT 1 FROM item_publishes p JOIN item_questions iq ON iq.publish_id = p.publish_id WHERE p.item_id = i.id AND ${qKeyIn("iq", "(SELECT value FROM json_each(?2))")})
                   THEN 'question' ELSE 'image' END AS via
         FROM items i WHERE ${where} ORDER BY i.id LIMIT ${listMax}`,
    )
    .bind(itemId, mh, bh)
    .all<{ item_id: string; author_id: string; title: string; via: "question" | "image" }>();
  const total = aff.results.length < listMax ? aff.results.length : ((await c.db.prepare(`SELECT COUNT(*) AS n FROM items i WHERE ${where}`).bind(itemId, mh, bh).first<{ n: number }>())?.n ?? 0);
  // Who each selected question's block alone would hold (round 12 M4: the preview says it), for up
  // to 200 selected positions, 20 items each.
  for (const m of match.filter((x) => x.selected).slice(0, 200)) {
    const r = await c.db
      .prepare(`SELECT i.id AS item_id, i.title FROM items i WHERE i.id <> ?1 AND i.purged_at IS NULL AND ${usesBlocked("?2", "'[]'")} ORDER BY i.id LIMIT 20`)
      .bind(itemId, JSON.stringify(m.keys))
      .all<{ item_id: string; title: string }>();
    m.affects = r.results;
  }
  const none = !match.some((x) => x.selected) && !blob.some((x) => x.selected);
  return { item_id: itemId, match, blob, affected: aff.results, affected_total: total, ...(ghost ? { ghost: true, selection_required: none && !sel.questions && !sel.images && !sel.all } : {}) };
}

/** The selected block keys with the match_hash of the question each came from (block rows keep it as `qhash`). */
export const selectedKeyPairs = (p: BlockPlan) => {
  const seen = new Set<string>();
  return p.match.filter((x) => x.selected).flatMap((x) => x.keys.filter((k) => (seen.has(k) ? false : (seen.add(k), true))).map((k) => ({ h: k, q: x.hash })));
};
/** The block keys of the selected positions (by their scopes). */
export const selectedMatch = (p: BlockPlan) => [...new Set(p.match.filter((x) => x.selected).flatMap((x) => x.keys))];
export const selectedBlobs = (p: BlockPlan) => p.blob.filter((x) => x.selected).map((x) => x.sha256);

/** What a block-plan confirm binds: the selection, the affected items, the admin and the action's parameters. */
export const planDigest = (kind: string, p: BlockPlan, bind: Record<string, unknown>) =>
  JSON.stringify([kind, p.item_id, selectedMatch(p).sort(), selectedBlobs(p).sort(), p.affected.map((a) => a.item_id).sort(), p.affected_total, Object.entries(bind).sort(([a], [b]) => (a < b ? -1 : 1))]);

/** A token binding a confirm to exactly the previewed digest, valid PREVIEW_MS and usable once. */
export async function tokenFor(c: Ctx, digest: string): Promise<string> {
  const exp = c.now() + PREVIEW_MS;
  const key = (await c.keys).preview;
  const body = b64url(utf8(JSON.stringify([exp, await hmac(key, digest), newId()])));
  return `${body}.${await hmac(key, body)}`;
}

/**
 * Throws 428 unless `token` was issued for this very digest (same selection, affected items, admin
 * and parameters), is unexpired, and was never used before (it is consumed here).
 */
export async function checkToken(c: Ctx, digest: string, token: unknown): Promise<void> {
  const fail = () => new PoolError(428, "preview_required", { message: "Preview first and confirm with its confirm_preview token (it binds the confirm to what you saw, once)." });
  if (typeof token !== "string") throw fail();
  const m = /^([A-Za-z0-9_-]{1,600})\.([A-Za-z0-9_-]{43})$/.exec(token);
  const key = (await c.keys).preview;
  const same = (a: string, b: string) => a.length === b.length && timingSafeEqual(utf8(a), utf8(b));
  if (!m || !same(await hmac(key, m[1]!), m[2]!)) throw fail();
  let exp = 0;
  let h = "";
  try {
    [exp, h] = JSON.parse(new TextDecoder().decode(fromB64url(m[1]!))) as [number, string];
  } catch {
    throw fail();
  }
  if (exp < c.now() || !same(h, await hmac(key, digest))) throw fail();
  const used = await c.db.prepare(`INSERT OR IGNORE INTO preview_tokens_used (token_hash, at) VALUES (?1, ?2)`).bind(await hmac(key, `used:${m[2]}`), c.now()).run();
  if ((used.meta.changes ?? 0) !== 1) throw fail();
}

export const previewToken = (c: Ctx, kind: string, p: BlockPlan, bind: Record<string, unknown>) => tokenFor(c, planDigest(kind, p, bind));
export const checkPreview = (c: Ctx, kind: string, p: BlockPlan, token: unknown, bind: Record<string, unknown>) => checkToken(c, planDigest(kind, p, bind), token);

/** Reads a selection from an admin request body. */
export function readSelection(b: Record<string, unknown> | null): BlockSelection {
  const sel: BlockSelection = {};
  if (b?.questions === "all") sel.all = true;
  else if (b?.questions !== undefined) {
    if (!Array.isArray(b.questions) || b.questions.length > 1000) throw new PoolError(400, "invalid_request", { field: "questions" });
    sel.questions = b.questions as number[];
  }
  if (b?.scopes !== undefined) {
    if (!b.scopes || typeof b.scopes !== "object" || Array.isArray(b.scopes) || Object.keys(b.scopes).length > 1000) throw new PoolError(400, "invalid_request", { field: "scopes" });
    sel.scopes = b.scopes as Record<string, BlockScope>;
  }
  if (b?.images !== undefined) {
    if (!Array.isArray(b.images) || b.images.length > 1000 || !b.images.every((x) => typeof x === "string")) throw new PoolError(400, "invalid_request", { field: "images" });
    sel.images = b.images as string[];
  }
  return sel;
}

/**
 * Holds of `source` (reason `reason`) on items that no longer use any still-active block of that
 * source are released: what an unblock leaves behind. Images count only where SERVED (served.ts).
 */
export async function releaseOrphanHolds(c: Ctx, source: string, reason: string): Promise<number> {
  const blocks =
    source === "privacy"
      ? `SELECT kind, hash FROM privacy_blocks`
      : `SELECT kind, hash FROM blocked_hashes WHERE action_id = '${source.slice(5).replace(/[^0-9a-f]/g, "")}' AND lifted_at IS NULL AND (expires_at IS NULL OR expires_at > ${c.now()})`;
  const r = await c.db
    .prepare(
      `DELETE FROM item_holds WHERE source = ?1 AND reason = ?2 AND NOT (
         EXISTS (SELECT 1 FROM item_publishes p JOIN item_questions iq ON iq.publish_id = p.publish_id JOIN (${blocks}) bl ON bl.kind = 'match' AND ${qKeyEq("iq", "bl.hash")}
                  WHERE p.item_id = item_holds.item_id)
         OR ${itemServesBlob("item_holds.item_id", (sha) => `EXISTS (SELECT 1 FROM (${blocks}) bl WHERE bl.kind = 'blob' AND bl.hash = ${sha})`)})`,
    )
    .bind(source, reason)
    .run();
  return r.meta.changes ?? 0;
}

/**
 * Holds (student_data, source privacy) on up to `batches` x AFFECTED_LIST more items that use the
 * given privacy-blocked hashes and are not held yet; returns whether some may remain.
 */
export async function holdPrivacyBatch(c: Ctx, exceptItem: string, mh: string[], bh: string[], by: string | null, batches: number): Promise<boolean> {
  const size = c.deps.affectedList ?? AFFECTED_LIST;
  for (let k = 0; k < batches; k++) {
    const r = await c.db
      .prepare(
        `INSERT OR IGNORE INTO item_holds (item_id, reason, at, by, source, detail)
         SELECT i.id, 'student_data', ?4, ?5, 'privacy', ${holdSnapshot("i.id", "?2")} FROM items i
          WHERE i.id <> ?1 AND i.purged_at IS NULL AND NOT EXISTS (SELECT 1 FROM item_holds h WHERE h.item_id = i.id AND h.source = 'privacy')
            AND ${usesBlocked("?2", "?3")} LIMIT ${size} RETURNING item_id`,
      )
      .bind(exceptItem, JSON.stringify(mh), JSON.stringify(bh), c.now(), by)
      .all<{ item_id: string }>();
    await notifyHeld(c, r.results.map((x) => x.item_id), "student_data");
    if (r.results.length < size) return false;
  }
  return true;
}
