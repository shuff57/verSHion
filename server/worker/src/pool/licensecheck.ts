// The chains readers see, and the reader-visible licence check (round 10 M-6), in one place so the
// read path and the moderation release use the same view. Everything here is computed from the
// displayed chain data only (public), never from evidence rows or hidden content.
import { isLicense, LICENSES, type License, republishOptions } from "./license.ts";
import { voidInfo, type ChainLink } from "./provenance.ts";
import { ID_RE, type Ctx } from "./shared.ts";

/**
 * The displayed chain of each stored chain: void (infringing) links dropped, a void link the admin
 * tied to the rightful item shown as that item, one link per item, and names of deleted accounts
 * shown as "deleted teacher" (resolved now, after the stand-ins).
 */
export async function displayChains(c: Ctx, chains: ChainLink[][]): Promise<ChainLink[][]> {
  const linked = [...new Set(chains.flat().map((l) => l.item_id).filter((x) => ID_RE.test(x)))];
  const voided = new Set<string>();
  const replacedBy = new Map<string, ChainLink>();
  if (linked.length > 0) {
    const r = await c.db.prepare(`SELECT item_id FROM void_items WHERE item_id IN (SELECT value FROM json_each(?1))`).bind(JSON.stringify(linked)).all<{ item_id: string }>();
    for (const x of r.results) voided.add(x.item_id);
    for (const [k, v] of (await voidInfo(c, [...voided])).replaced) replacedBy.set(k, v);
  }
  const shown = chains.map((ch) =>
    ch.map((l) => (replacedBy.has(l.item_id) ? { ...replacedBy.get(l.item_id)!, modified: l.modified } : l)).filter((l, j, all) => !voided.has(l.item_id) && (l.item_id === "" || all.findIndex((x) => x.item_id === l.item_id) === j)),
  );
  const gone = new Set<string>();
  const ids = [...new Set(shown.flat().map((l) => l.item_id).filter((x) => ID_RE.test(x)))];
  if (ids.length > 0) {
    const r = await c.db
      .prepare(`SELECT i.id FROM items i JOIN users u ON u.id = i.author_id WHERE i.id IN (SELECT value FROM json_each(?1)) AND u.deleted_at IS NOT NULL`)
      .bind(JSON.stringify(ids))
      .all<{ id: string }>();
    for (const x of r.results) gone.add(x.id);
  }
  return shown.map((ch) => ch.map((l) => (gone.has(l.item_id) ? { ...l, author_name: "deleted teacher" } : l)));
}

/** What one displayed chain allows: a verbatim link its own licence (CC0: any), a modified one the table's options, a collapsed run its links' options. */
export function chainAllows(chain: ChainLink[]): License[] {
  let out = new Set<License>(LICENSES);
  const cut = (opts: readonly License[]) => (out = new Set([...out].filter((l) => opts.includes(l))));
  for (const l of chain) {
    if (isLicense(l.license)) cut(l.modified || l.license === "CC0-1.0" ? republishOptions(l.license) : [l.license as License]);
    for (const x of l.collapsed_licenses ?? []) if (isLicense(x)) cut(republishOptions(x));
  }
  return LICENSES.filter((l) => out.has(l));
}

export interface LicenseCheck {
  /** Round 14: "unverified" (a source the publisher only read restricts it and was not followed, B8),
   *  "claimed_independent" (the publisher attested independence of a possible source, B5),
   *  "unverified_earlier_match" (another author published it identically earlier, no serve on
   *  record, B3). Worst first: mismatch, unverified, claimed_independent, unverified_earlier_match, ok. */
  status: "ok" | "mismatch" | "unverified" | "claimed_independent" | "unverified_earlier_match";
  /** What it is checked against (round 11 P5): the sources the licence rule recorded at publish. */
  basis: "recorded_sources";
  /** Questions whose recorded sources do not allow the item's licence (now), and what they allow. */
  positions?: { pos: number; allowed: License[] }[];
  reason?: string;
}

/** What the licence rule allowed for a question at publish, and the items that decided it. */
export interface StoredAllowed {
  allowed: string[];
  binders: string[];
  soft?: boolean;
  claimed?: boolean;
  earlier?: boolean;
}

/**
 * THE reader-visible marker (round 11 P5): "consistent with recorded sources". The licences the
 * licence rule allowed when each question was published (item_questions.allowed_json), with one
 * change since: a deciding source found infringing and tied to the rightful item narrows that
 * question to what the rightful item's licence allows (its stand-in link, verbatim or modified as
 * the chain records it). A question without a record falls back to its displayed chain.
 */
export function licenseCheck(license: string, chains: ChainLink[][], stored: (StoredAllowed | null)[] = [], standIns: Map<string, ChainLink> = new Map()): LicenseCheck {
  const bad: { pos: number; allowed: License[] }[] = [];
  chains.forEach((ch, pos) => {
    const st = stored[pos];
    let allowed: License[];
    if (st) {
      let set = new Set(st.allowed.filter(isLicense) as License[]);
      for (const b of st.binders) {
        const rep = standIns.get(b);
        if (!rep || !isLicense(rep.license)) continue;
        const link = ch.find((l) => l.item_id === rep.item_id) ?? ch.find((l) => l.item_id === "") ?? rep;
        const opts = link.modified || rep.license === "CC0-1.0" ? republishOptions(rep.license) : [rep.license as License];
        set = new Set([...set].filter((l) => opts.includes(l)));
      }
      allowed = LICENSES.filter((l) => set.has(l));
    } else allowed = chainAllows(ch);
    if (!allowed.includes(license as License)) bad.push({ pos, allowed });
  });
  if (bad.length === 0) {
    const marked = (k: "soft" | "claimed" | "earlier") => stored.map((st, pos) => (st?.[k] ? pos : -1)).filter((p) => p >= 0);
    for (const [k, status, reason] of [
      ["soft", "unverified", "A source the publisher was served restricts these questions; the item's licence was not checked against it."],
      ["claimed", "claimed_independent", "The publisher says these questions were written independently of a source they had seen."],
      ["earlier", "unverified_earlier_match", "Another author published these questions earlier; there is no record of this publisher being served them."],
    ] as const) {
      const ps = marked(k);
      if (ps.length > 0) return { status, basis: "recorded_sources", positions: ps.slice(0, 20).map((pos) => ({ pos, allowed: [] })), reason };
    }
    return { status: "ok", basis: "recorded_sources" };
  }
  return { status: "mismatch", basis: "recorded_sources", positions: bad.slice(0, 20), reason: "The licence of this item is not one its recorded sources allow for these questions." };
}

/** Parses item_questions.allowed_json. */
export function storedAllowed(json: string | null | undefined): StoredAllowed | null {
  if (!json) return null;
  try {
    const v = JSON.parse(json) as StoredAllowed;
    return Array.isArray(v.allowed) && Array.isArray(v.binders) ? v : null;
  } catch {
    return null;
  }
}

/** The stand-ins of void items among the binders of these records (their rightful item, licence at void time). */
export async function standInsOf(c: Ctx, stored: (StoredAllowed | null)[]): Promise<Map<string, ChainLink>> {
  const ids = [...new Set(stored.flatMap((s) => s?.binders ?? []).filter((x) => ID_RE.test(x)))];
  return ids.length ? (await voidInfo(c, ids)).replaced : new Map();
}

/** The check for stored items (their current publish), by id. */
export async function licenseCheckOf(c: Ctx, itemIds: string[]): Promise<Map<string, LicenseCheck>> {
  const out = new Map<string, LicenseCheck>();
  if (itemIds.length === 0) return out;
  const r = await c.db
    .prepare(
      `SELECT i.id, i.license, json_group_array(json_array(iq.pos, iq.chain_json, iq.allowed_json)) AS qs FROM items i JOIN item_questions iq ON iq.publish_id = i.current_publish_id
        WHERE i.id IN (SELECT value FROM json_each(?1)) GROUP BY i.id`,
    )
    .bind(JSON.stringify(itemIds))
    .all<{ id: string; license: string; qs: string }>();
  for (const x of r.results) {
    const qs = (JSON.parse(x.qs) as [number, string, string | null][]).sort((a, b) => a[0] - b[0]);
    const chains = await displayChains(c, qs.map((q) => JSON.parse(q[1]) as ChainLink[]));
    const stored = qs.map((q) => storedAllowed(q[2]));
    out.set(x.id, licenseCheck(x.license, chains, stored, await standInsOf(c, stored)));
  }
  return out;
}
