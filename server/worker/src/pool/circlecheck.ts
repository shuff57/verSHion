// Round 32: the one link between circles and the public pool (docs/circles-plan.md section 7, D13).
//
// Circle content stays out of the public licence engine (no served rows, no authorship records). Without
// a check, a member could copy a circle question into a public publish and the engine, having no record
// of it, would treat her as its original author: the material would leak against its author's wish and
// the copier would be falsely credited as first. So every publish through /items (POST, PUT and their
// dry runs; public or unlisted, since an unlisted item can be made public later without a new publish)
// runs this check after share-core has produced the key-free match hashes:
//
// * the DISTINCT hashes (of the full and the served form) are looked up in circle_qkeys for every circle
//   the publisher is or was a member of (her circle_members rows, active or left, at most
//   `circle_memberships_total`: round 33) that holds records; per hash the earliest hit (smallest first_seq
//   across those circles) is kept: one indexed seek per (hash, circle), one result row per hash;
// * a position passes when that earliest hit was first sent by the publisher herself; or when she had
//   published the same text in the public pool before (author_firsts, the same pool_seq scale); or when
//   it is a short statement (the public engine's isShort); or when she names the circle bank in an
//   independence statement (circle_independent_of: {banks, accepted, statement}: audited and flagged as an
//   independent_claim, the third in 30 days flags the account) and no circle-only bank she can still
//   read holds that text first sent by someone else (a statement never covers an exact copy of
//   something she can read);
// * otherwise the publish is refused: 422 circle_material {positions, hits}, whatever the bank's licence
//   (members' re-sharing of CC circle banks waits for phase C). The circle and bank names go only to the
//   publisher, who is or was a member. A publisher who never joined a circle costs one indexed probe.
import { cleanText } from "../../../auth/src/validate.ts";
import { ID_RE, limitOf, PoolError, reserveBytes, type Ctx } from "./shared.ts";
import { ACTIVE_SQL, AUTHOR_OK_SQL } from "./circles.ts";

export interface CircleClaim {
  banks: string[];
  statement: string;
}

/** The meta's `circle_independent_of` (null when absent; 400 when malformed). */
export function readCircleClaim(m: Record<string, unknown>): CircleClaim | null {
  const x = m.circle_independent_of as Record<string, unknown> | undefined | null;
  if (x === undefined || x === null) return null;
  const banks = typeof x === "object" && !Array.isArray(x) ? x.banks : undefined;
  if (x.accepted !== true || !Array.isArray(banks) || banks.length === 0 || banks.length > 50 || !banks.every((b) => typeof b === "string" && ID_RE.test(b)))
    throw new PoolError(400, "invalid_request", { field: "circle_independent_of" });
  const statement = cleanText(x.statement, 1000);
  if (!statement || statement.length < 10) throw new PoolError(400, "invalid_request", { field: "circle_independent_of.statement" });
  return { banks: [...new Set(banks as string[])], statement };
}

/**
 * The earliest sender of each hash across the publisher's circles, ONE row per hash that has a hit (round 33,
 * critic40b B3: never a row per hash and membership). The circles considered (`my`): her memberships, active
 * first, then the most recently left, at most ?3 (`circle_memberships_total`, the cap joins enforce, so a
 * real account never has more), and only circles that hold records. Per (hash, circle) one seek of the
 * first_seq-ordered covering index (x is MATERIALIZED, so the probe runs once per pair, not once per
 * reference); the window keeps the earliest per hash.
 */
export const CIRCLE_HITS_SQL = `WITH h AS MATERIALIZED (SELECT DISTINCT value AS mh FROM json_each(?2)),
     my AS MATERIALIZED (SELECT m.circle_id FROM circle_members m JOIN circles ci ON ci.id = m.circle_id AND ci.qkeys > 0 AND ci.purged_at IS NULL
                          WHERE m.user_id = ?1 ORDER BY m.left_at IS NOT NULL, m.left_at DESC, m.circle_id LIMIT ?3),
     x AS MATERIALIZED (SELECT h.mh AS mh, my.circle_id AS circle_id,
                  (SELECT json_array(k.bank_id, k.first_seq, k.first_by) FROM circle_qkeys k INDEXED BY circle_qkeys_first
                    WHERE k.circle_id = my.circle_id AND k.match_hash = h.mh ORDER BY k.first_seq, k.bank_id LIMIT 1) AS hit
             FROM h CROSS JOIN my)
SELECT mh, circle_id, hit FROM (SELECT mh, circle_id, hit, ROW_NUMBER() OVER (PARTITION BY mh ORDER BY hit ->> '$[1]', circle_id, hit ->> '$[0]') AS rn FROM x WHERE hit IS NOT NULL)
 WHERE rn = 1`;
/** Bytes reserved per hash a check statement may return (a row: the hash, the circle, the hit's JSON). */
export const CIRCLE_CHECK_ROW_BYTES = 512;

/** Hashes a circle-only bank the publisher can still read holds, first sent by someone else (a statement never covers these). */
export const READABLE_OTHERS_SQL = `SELECT DISTINCT j.value AS mh FROM json_each(?2) j
 WHERE EXISTS (SELECT 1 FROM circle_members m JOIN circles ci ON ci.id = m.circle_id AND ci.closed_at IS NULL AND ci.purged_at IS NULL
                 JOIN circle_qkeys k ON k.circle_id = m.circle_id AND k.match_hash = j.value
                 JOIN circle_banks b ON b.id = k.bank_id AND b.removed_at IS NULL AND b.purged_at IS NULL AND b.license = 'circle-only'
                WHERE m.user_id = ?1 AND m.left_at IS NULL AND k.first_by <> ?1)`;

/**
 * The check. `hashesAt[i]`: the key-free match hashes of position i (its full and served forms);
 * `shortAt[i]`: a short statement. Throws 422 circle_material; returns the banks an independence
 * statement was actually needed for (to audit and flag).
 */
export async function checkCircleMaterial(c: Ctx, me: string, hashesAt: string[][], shortAt: boolean[], claim: CircleClaim | null): Promise<{ claimed: string[] }> {
  if (!(await c.db.prepare(`SELECT 1 AS x FROM circle_members WHERE user_id = ?1 LIMIT 1`).bind(me).first())) return { claimed: [] };
  const all = [...new Set(hashesAt.flat())];
  const circlesMax = await limitOf(c, me, "circle_memberships_total");
  const best = new Map<string, { circle: string; bank: string; seq: number; by: string }>();
  for (let k = 0; k < all.length; k += 1000) {
    const part = all.slice(k, k + 1000);
    // (the round-29 discipline: a statement's worst-case results are reserved before it runs)
    const release = reserveBytes(part.length * CIRCLE_CHECK_ROW_BYTES);
    try {
      const rows = await c.db.prepare(CIRCLE_HITS_SQL).bind(me, JSON.stringify(part), circlesMax).all<{ mh: string; circle_id: string; hit: string }>();
      for (const r of rows.results) {
        const [bank, seq, by] = JSON.parse(r.hit) as [string, number, string];
        const b = best.get(r.mh);
        if (!b || seq < b.seq || (seq === b.seq && (r.circle_id < b.circle || (r.circle_id === b.circle && bank < b.bank)))) best.set(r.mh, { circle: r.circle_id, bank, seq, by });
      }
    } finally {
      release();
    }
  }
  if (best.size === 0) return { claimed: [] };
  // Per position: the earliest hit over its hashes.
  const hitAt = hashesAt.map((hs) => {
    let out: { mh: string; circle: string; bank: string; seq: number; by: string } | null = null;
    for (const h of hs) {
      const b = best.get(h);
      if (b && (!out || b.seq < out.seq)) out = { mh: h, ...b };
    }
    return out;
  });
  // "Already hers in the pool": her own public first of the text, before the circle's first sender sent it.
  const hitHashes = [...new Set(hashesAt.filter((_, i) => hitAt[i] && hitAt[i]!.by !== me && !shortAt[i]).flat())];
  const firsts = new Map<string, number>();
  for (let k = 0; k < hitHashes.length; k += 1000) {
    const part = hitHashes.slice(k, k + 1000);
    const release = reserveBytes(part.length * CIRCLE_CHECK_ROW_BYTES);
    try {
      for (const r of (await c.db.prepare(`SELECT match_hash, first_seq FROM author_firsts WHERE author_id = ?1 AND match_hash IN (SELECT value FROM json_each(?2))`).bind(me, JSON.stringify(part)).all<{ match_hash: string; first_seq: number }>()).results)
        firsts.set(r.match_hash, r.first_seq);
    } finally {
      release();
    }
  }
  const open: number[] = [];
  hitAt.forEach((h, i) => {
    if (!h || h.by === me || shortAt[i]) return;
    if (hashesAt[i]!.some((x) => (firsts.get(x) ?? Infinity) < h.seq)) return;
    open.push(i);
  });
  if (open.length === 0) return { claimed: [] };
  // What an independence statement may cover: never a text a circle-only bank she can still read holds,
  // first sent by someone else. Asked only where it matters (bounded): the positions her statement names
  // the bank of, and the (at most 100) refused positions the answer reports.
  const readable = new Set<string>();
  const asked = new Set<string>();
  const ask = async (hs: string[]) => {
    const todo = [...new Set(hs)].filter((h) => !asked.has(h));
    for (const h of todo) asked.add(h);
    for (let k = 0; k < todo.length; k += 1000) {
      const part = todo.slice(k, k + 1000);
      const release = reserveBytes(part.length * CIRCLE_CHECK_ROW_BYTES);
      try {
        for (const r of (await c.db.prepare(READABLE_OTHERS_SQL).bind(me, JSON.stringify(part)).all<{ mh: string }>()).results) readable.add(r.mh);
      } finally {
        release();
      }
    }
  };
  const mayClaim = (i: number) => !hashesAt[i]!.some((x) => readable.has(x));
  const claimed = new Set<string>();
  if (claim) await ask(open.filter((i) => claim.banks.includes(hitAt[i]!.bank)).flatMap((i) => hashesAt[i]!));
  const refused = open.filter((i) => {
    const h = hitAt[i]!;
    if (claim && claim.banks.includes(h.bank) && mayClaim(i)) {
      claimed.add(h.bank);
      return false;
    }
    return true;
  });
  if (refused.length === 0) return { claimed: [...claimed] };
  await ask(refused.slice(0, 100).flatMap((i) => hashesAt[i]!));
  const banks = [...new Set(refused.map((i) => hitAt[i]!.bank))];
  // Round 34 (critic41b F1): a hit names the bank and circle only when the publisher can see that bank now (an
  // active member of the open circle, the bank live, not held, its author in good standing); a held bank (or a
  // suspended author's) gives a generic hit: no ids, no names (a hold exists to hide that title); otherwise
  // (a former member, a removed bank, a closed circle) the ids, to name in a statement, without names.
  const names = new Map(
    (
      await c.db
        .prepare(
          `SELECT b.id, b.title, ci.id AS circle_id, ci.name,
                  (EXISTS (SELECT 1 FROM circle_holds h WHERE h.bank_id = b.id) OR NOT ${AUTHOR_OK_SQL("b")}) AS hidden,
                  (b.removed_at IS NULL AND b.purged_at IS NULL AND ci.closed_at IS NULL AND ${ACTIVE_SQL("ci.id", "?2")}) AS visible
             FROM circle_banks b JOIN circles ci ON ci.id = b.circle_id WHERE b.id IN (SELECT value FROM json_each(?1))`,
        )
        .bind(JSON.stringify(banks.slice(0, 100)), me)
        .all<{ id: string; title: string; circle_id: string; name: string | null; hidden: number; visible: number }>()
    ).results.map((r) => [r.id, r]),
  );
  throw new PoolError(422, "circle_material", {
    positions: refused.slice(0, 1000),
    hits: refused.slice(0, 100).map((i) => {
      const h = hitAt[i]!;
      const nm = names.get(h.bank);
      if (!nm || nm.hidden === 1) return { pos: i, circle_id: null, circle_name: null, bank_id: null, bank_title: null, may_claim_independence: false };
      const shown = nm.visible === 1;
      return { pos: i, circle_id: h.circle, circle_name: shown ? nm.name : null, bank_id: h.bank, bank_title: shown ? nm.title : null, may_claim_independence: mayClaim(i) };
    }),
    message: "Some questions were first shared by another member of a circle. They cannot be shared publicly from here.",
  });
}
