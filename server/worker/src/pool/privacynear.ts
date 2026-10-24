// Round 37: the near-rewording check of confirmed student-data text (round 12 M6; rounds 16, 20-22), moved
// here unchanged from publish.ts so a public publish and a circle send (docs/circles-plan.md 13.2: "the
// publish path's privacy-block check reused") run the same code.
import { FUZZY_JACCARD, jaccard } from "./provenance.ts";
import { keyedOne, keyedSet, type Ctx } from "./shared.ts";

/** One stem as the check sees it: its words and its MinHash bands (unkeyed; keyed here). */
export interface StemSketch {
  tokens: string[];
  bands: string[];
}

/**
 * The positions whose stem (or one of its Multipart parts' stems) comes near a confirmed student-data
 * block: a keyed band in common with a privacy row and a token Jaccard >= FUZZY_JACCARD (a band row with no
 * token sketch matches on the band). A position one of whose exact or normalised keys (`keysAt`) is itself
 * blocked is not "near" (it is held by the exact check). Past `max` band rows every question sharing a
 * band counts (fail closed, round 20 F1). `exactOf` runs only when some band row matched.
 */
export async function privacyNearPositions(
  c: Ctx,
  stems: StemSketch[][],
  keysAt: (string | null)[][],
  exactOf: () => Promise<Set<string>>,
  skMemo: Map<string, string>,
  max: number,
): Promise<number[]> {
  const out: number[] = [];
  // (Round 22 B2: privacy rows keep keyed bands and keyed token sets only.)
  const rawBands = [...new Set(stems.flatMap((ss) => ss.flatMap((s) => s.bands)))];
  const kOf = new Map(await Promise.all(rawBands.map(async (b) => [b, await keyedOne("b:", b, skMemo)] as const)));
  const allBands = rawBands.map((b) => kOf.get(b)!);
  const kToks = new Map<string[], string[]>();
  for (const ss of stems) for (const s of ss) kToks.set(s.tokens, await keyedSet(s.tokens, skMemo));
  if (allBands.length === 0) return out;
  const pb = await c.db
    .prepare(`SELECT DISTINCT band, qhash, tokens FROM privacy_bands WHERE band IN (SELECT value FROM json_each(?1)) LIMIT ${max + 1}`)
    .bind(JSON.stringify(allBands))
    .all<{ band: string; qhash: string; tokens: string | null }>();
  if (pb.results.length === 0) return out;
  const exact = await exactOf();
  const cut = pb.results.length > max;
  stems.forEach((ss, i) => {
    if ((keysAt[i] ?? []).some((k) => k !== null && exact.has(k))) return;
    const near = ss.some((s) => {
      const set = new Set(s.bands.map((b) => kOf.get(b)!));
      // (Rounds 21-22 E1/B2: the bands keep a keyed sketch, never words; a band with none matches on the band.)
      return pb.results.some((x) => set.has(x.band) && (cut || x.tokens === null || jaccard(kToks.get(s.tokens)!, JSON.parse(x.tokens) as string[]) >= FUZZY_JACCARD));
    });
    if (near) out.push(i);
  });
  return out;
}
