// Licences and the inheritance table (docs/shared-pool.md "Sharing model", "Licenses inherit, per
// question"). A CC grant is per version; a copy keeps the licence of the publish it came from.

export const LICENSES = ["CC-BY-4.0", "CC0-1.0", "CC-BY-SA-4.0", "CC-BY-NC-4.0", "CC-BY-NC-SA-4.0", "circle-only"] as const;
export type License = (typeof LICENSES)[number];

export function isLicense(x: unknown): x is License {
  return typeof x === "string" && (LICENSES as readonly string[]).includes(x);
}

/**
 * Origin licence -> licences a derived publish may carry. "Stricter" never means a different
 * condition set: BY-SA cannot become BY-NC-SA. circle-only material cannot be re-shared by an
 * importer at all (its author can still republish their own).
 */
const MAY: Record<License, readonly License[]> = {
  "CC0-1.0": LICENSES,
  "CC-BY-4.0": ["CC-BY-4.0", "CC-BY-SA-4.0", "CC-BY-NC-4.0", "CC-BY-NC-SA-4.0"],
  "CC-BY-SA-4.0": ["CC-BY-SA-4.0"],
  "CC-BY-NC-4.0": ["CC-BY-NC-4.0", "CC-BY-NC-SA-4.0"],
  "CC-BY-NC-SA-4.0": ["CC-BY-NC-SA-4.0"],
  "circle-only": [],
};

/** May material under `origin` be published by someone else under `target`? */
export function mayRepublish(origin: string, target: string): boolean {
  return isLicense(origin) && isLicense(target) && MAY[origin].includes(target);
}

/** The licences an importer may publish a copy under (for the import response). */
export function republishOptions(origin: string): readonly License[] {
  return isLicense(origin) ? MAY[origin] : [];
}

/** Licences allowed by every one of `origins` (empty when none is). */
export function allowedByAll(origins: Iterable<string>): License[] {
  let out: License[] = [...LICENSES];
  for (const o of origins) out = out.filter((t) => mayRepublish(o, t));
  return out;
}

/**
 * Revisions of the rights-attestation wording the server accepts. The app shows the newest; an
 * older one stays accepted until the app versions that show it are gone.
 */
export const ATTEST_TEXT_REVS = ["2026-10-19"] as const;
/**
 * Round 23 (owner): the publishing terms the teacher affirms on every publish and republish
 * (`attest: { own_work: true, terms_version }`). An affirmation by the teacher, recorded, never
 * verified by the server; a preview does not need it.
 */
export const POOL_TERMS_VERSION = "2026-10-22";
export const ATTEST_STATEMENT =
  "I confirm this content is my own original work, or I have the right to share it under the licence I chose. It is not copied from a published source (textbooks, publisher test banks, exam boards, paid worksheets) without permission.";

/**
 * One grant a question carries: the licence of an upstream publish of item `item`, and whether the
 * copy changed it. A verbatim copy keeps its licence (CC 4.0 lets you add terms only to your own
 * adaptation), except that CC0 material may be relabelled freely; an adaptation follows the table.
 */
export interface Grant {
  item: string;
  license: string;
  modified: boolean;
}

export function grantOptions(g: Grant): readonly License[] {
  if (!isLicense(g.license) || g.license === "circle-only") return [];
  if (g.modified || g.license === "CC0-1.0") return MAY[g.license];
  return [g.license];
}

/**
 * THE licence rule, used by both the import response (`may_republish_as`) and the publish check, so
 * the two can never disagree. Within one item: union of the verbatim grants if there are any,
 * otherwise intersection; across different items: intersection. No grants: any licence.
 */
export function allowedFor(grants: Grant[]): License[] {
  const byItem = new Map<string, Grant[]>();
  for (const g of grants) byItem.set(g.item, [...(byItem.get(g.item) ?? []), g]);
  let out: License[] = [...LICENSES];
  for (const gs of byItem.values()) {
    // Within one item (critic C4): a verbatim copy of some version may use any of the grants of the
    // versions it is identical to (union of the verbatim grants only). An adaptation must satisfy
    // every version it drew on (intersection): adding an NC-only explanation to a v2 never makes it
    // CC-BY just because v1 was.
    const verbatim = gs.filter((g) => !g.modified);
    let ok: Set<License>;
    if (verbatim.length > 0) ok = new Set(verbatim.flatMap((g) => [...grantOptions(g)]));
    else {
      ok = new Set(LICENSES);
      for (const g of gs) {
        const o = new Set(grantOptions(g));
        ok = new Set([...ok].filter((l) => o.has(l)));
      }
    }
    out = out.filter((l) => ok.has(l));
  }
  return out;
}

/** A bank: every question's grants must allow the licence. */
export function allowedForAll(perQuestion: Grant[][]): License[] {
  let out: License[] = [...LICENSES];
  for (const g of perQuestion) {
    const a = new Set(allowedFor(g));
    out = out.filter((l) => a.has(l));
  }
  return out;
}

/** Grants carried by a stored chain (oldest first): every earlier link counts as adapted. */
export function chainGrants(chain: { item_id: string; license: string; collapsed_licenses?: string[] }[]): Grant[] {
  return chain.flatMap((l) => [
    ...(l.item_id !== "" && l.license !== "" ? [{ item: l.item_id, license: l.license, modified: true }] : []),
    // A collapsed run of links (a chain longer than CHAIN_MAX) still grants what its links did.
    ...(l.collapsed_licenses ?? []).map((license) => ({ item: `collapsed:${license}`, license, modified: true })),
  ]);
}

/**
 * One source as the licence rule sees it (rounds 9 and 10): its chain's licences (each an
 * adaptation grant), its own licences grouped by served content (`own`: every licence shown for
 * that content; identical content across versions of one item is ONE material, so its licences are
 * united: round 10 M-4), whether the subject is a copy of it (licence sense), the contents it served
 * (sources that served the same content are one material) and when its content was first published
 * (`seq`: the earliest of its serves).
 */
export interface LicenceUnit {
  item: string;
  verbatim: boolean;
  chain: string[];
  own: string[][];
  contents: string[];
  seq: number;
}

export interface LicenceDecision {
  /** The licences the question may be published under. */
  allowed: License[];
  /** The sources contradict each other: decided by publication order (flag license_conflict). */
  conflict: boolean;
  /** The items that decide it (named in a refusal's bound_by), earliest first. */
  binders: string[];
}

const opts = (license: string, modified: boolean): readonly License[] => grantOptions({ item: "", license, modified });
const cutBy = (set: Set<License>, o: readonly License[]) => new Set([...set].filter((l) => o.includes(l)));

/**
 * What one source allows by itself: its own terms (a copy: any of the licences shown for that
 * content; an adaptation: for each content it served, any of them, and every content's), narrowed
 * by its chain (oldest material first). If its own grants contradict each other (a relabel against
 * its chain), the older terms win: chain first, then its own, skipping what would leave nothing
 * (never a licence no single grant allows). Empty only for circle-only material.
 */
export function unitAllows(u: LicenceUnit): { allowed: Set<License>; conflict: boolean } {
  const own = u.verbatim
    ? new Set(u.own.flat().flatMap((l) => [...opts(l, false)]))
    : u.own.reduce((acc, group) => cutBy(acc, [...new Set(group.flatMap((l) => [...opts(l, true)]))]), new Set<License>(LICENSES));
  let full = own;
  for (const l of u.chain) full = cutBy(full, opts(l, true));
  if (full.size > 0) return { allowed: full, conflict: false };
  let out = new Set<License>(LICENSES);
  let any = false;
  const steps: (readonly License[])[] = [...[...u.chain].sort().map((l) => opts(l, true)), [...own]];
  for (const o of steps) {
    const n = cutBy(out, o);
    if (n.size > 0) {
      out = n;
      any = true;
    }
  }
  return { allowed: any ? out : new Set(), conflict: true };
}

/**
 * THE licence rule (round 10, M-1 and the design resolution), for the publish check and the import
 * response alike, over the HARD sources of one question:
 * 1. The subject is a copy of some sources (verbatim, licence sense): the EARLIEST-published of them
 *    is the original and its terms decide (a later identical relabel can neither widen nor narrow
 *    them: a CC0 original next to an NC-SA relabel still allows CC0). Adaptations beside a copy do
 *    not narrow it.
 * 2. Otherwise (an adaptation): sources that served the same content are one material (its earliest
 *    source decides it); the materials are taken in publication order, earliest first, and each
 *    narrows what the earlier ones allow; one that would leave nothing is skipped (`conflict`,
 *    flagged). So more sources never widen the set (a later-published one can only narrow it or be
 *    skipped), the outcome never depends on import order, and a contradiction never locks anyone
 *    out: the earliest material's terms bound it.
 * 3. Nothing at all only for circle-only material (allowed []).
 * No sources: any licence.
 */
export function decideLicence(units: LicenceUnit[]): LicenceDecision {
  if (units.length === 0) return { allowed: [...LICENSES], conflict: false, binders: [] };
  const order = (s: Set<License>) => LICENSES.filter((l) => s.has(l));
  const byAge = (a: LicenceUnit, b: LicenceUnit) => a.seq - b.seq || (a.item < b.item ? -1 : a.item > b.item ? 1 : 0);
  const verbatim = units.filter((u) => u.verbatim).sort(byAge);
  if (verbatim.length > 0) {
    for (const u of verbatim) {
      const a = unitAllows(u);
      if (a.allowed.size > 0) return { allowed: order(a.allowed), conflict: a.conflict, binders: [u.item] };
    }
    return { allowed: [], conflict: false, binders: [verbatim[0]!.item] };
  }
  // Materials: units connected by a shared served content (union-find over content hashes).
  const all = [...units].sort(byAge);
  const parent = all.map((_, i) => i);
  const find = (i: number): number => (parent[i] === i ? i : (parent[i] = find(parent[i]!)));
  const byContent = new Map<string, number>();
  all.forEach((u, i) => {
    for (const h of u.contents) {
      const j = byContent.get(h);
      if (j === undefined) byContent.set(h, i);
      else parent[find(i)] = find(j);
    }
  });
  const classes = new Map<number, number[]>();
  all.forEach((_, i) => classes.set(find(i), [...(classes.get(find(i)) ?? []), i]));
  // Each material: its earliest source that allows anything decides it; materials earliest first.
  const mats = [...classes.values()]
    .map((idx) => {
      const decided = idx.map((i) => ({ i, a: unitAllows(all[i]!) })).find((x) => x.a.allowed.size > 0);
      return { seq: all[idx[0]!]!.seq, items: idx.map((i) => all[i]!.item), allowed: decided?.a.allowed ?? null, inner: decided?.a.conflict ?? false };
    })
    .sort((a, b) => a.seq - b.seq);
  let out = new Set<License>(LICENSES);
  let conflict = false;
  let decided = false;
  for (const m of mats) {
    if (!m.allowed) continue;
    conflict ||= m.inner;
    const n = cutBy(out, [...m.allowed]);
    if (n.size > 0) {
      out = n;
      decided = true;
    } else conflict = true;
  }
  return { allowed: decided ? order(out) : [], conflict, binders: [...new Set(mats.flatMap((m) => m.items))] };
}

/** A bank: the licences every question allows. */
export function allowedForBank(perQuestion: License[][]): License[] {
  return LICENSES.filter((l) => perQuestion.every((a) => a.includes(l)));
}

/**
 * Round 20 (critic25a E1): the licence a stored link records, given the licences available to the
 * publisher for the material (`groups`: per served content, every licence it received for it or the
 * source's licence now for the same content). Per content: the most permissive one that permits
 * `target`; across contents (an adaptation of several): the most restrictive of those. Null when some
 * content has none (the label is then left as it was).
 */
export function reliedLicence(groups: string[][], target: string, modified: boolean): License | null {
  const reach = (l: License) => grantOptions({ item: "", license: l, modified: true }).length;
  let out: License | null = null;
  for (const g of groups) {
    const ok = [...new Set(g)].filter((l): l is License => isLicense(l) && grantOptions({ item: "", license: l, modified }).includes(target as License));
    if (ok.length === 0) return null;
    const best = ok.reduce((a, b) => (reach(b) > reach(a) ? b : a));
    if (out === null || reach(best) < reach(out)) out = best;
  }
  return out;
}
