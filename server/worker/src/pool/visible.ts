// THE visible-derivation boundary (critic round 5, structural rule).
//
// Anything a non-author can see, or that decides an outcome a non-author can observe (which source a
// chain credits, the "modified" flag, the licence a copy may carry, may_republish_as, whether a
// source counts as evidence), is computed ONLY by comparing a `Subject` with a served source here.
// When the publish hides its key, its Subject carries no full-content hash at all: there is nothing
// key-bearing to compare, so no visible result can depend on the key. Full-content comparisons are
// possible only for a publish that shows its key (whose key is public anyway).

/** The new question, as far as visible derivations may look at it. */
export interface Subject {
  /** Key-free facts (match_hash, points, difficulty, topics, served image). */
  readonly keyfree: string;
  /** Full content (key included): only for a publish that shows its key; null when hidden. */
  readonly full: string | null;
  /** Licence sense (round 9 J1): the full content without points, difficulty and topics; null when hidden. */
  readonly near: string | null;
  /** ... and its key-free part (match_hash and the served image): what a hidden subject is compared on. */
  readonly nearKf: string;
  /** The licence-sense hash without the explanation (round 10 M-3), and whether the subject has none. */
  readonly nearBare: string | null;
  readonly noExpl: boolean;
}

/** A served source question as the evidence query groups it (one per source item and qid). */
export interface Served {
  /** Some serve of it equals the subject (sameSql, decided in SQL from the Subject alone). */
  same: boolean;
  /** ... and that serve was the source's full form. */
  same_full: boolean;
}

export function subjectFor(hideKey: boolean, keyfree: string, full: string, near: string, nearKf: string, nearBare: string = near, noExpl = false): Subject {
  return Object.freeze({ keyfree, full: hideKey ? null : full, near: hideKey ? null : near, nearKf, nearBare: hideKey ? null : nearBare, noExpl: hideKey ? false : noExpl });
}

/** The subject as the evidence query receives it: a hidden subject's full content is null (by construction). */
export interface SubjectParam {
  mh: string;
  full: string | null;
  kf: string;
  nfull: string | null;
  nkf: string;
  nbare: string | null;
  noexpl: number;
  refs: { item: string; qid: string }[];
}
export function subjectParam(s: Subject, mh: string, refs: { item: string; qid: string }[] = []): SubjectParam {
  return { mh, full: s.full, kf: s.keyfree, nfull: s.near, nkf: s.nearKf, nbare: s.nearBare, noexpl: s.noExpl ? 1 : 0, refs };
}

/**
 * THE comparison, as SQL over a subject row `s` (from subjectParam) and an evidence row `iq`: is the
 * subject the same question as that serve? A subject that shows its key is compared in full with
 * exactly what was served (the full form, or the stripped form of a hidden source: adding a key or an
 * explanation to a stripped serve is a modification, round 6 F6); a hidden subject (full IS NULL)
 * only on key-free facts. Nothing else anywhere compares questions for a visible outcome.
 */
export const sameSql = (s: string, iq: string) => `(CASE WHEN ${s}.full IS NOT NULL THEN ${iq}.content_hash = ${s}.full ELSE ${iq}.keyfree_hash = ${s}.kf END)`;

/**
 * The licence-sense comparison (round 9 J1): the same question as that serve, points, difficulty and
 * topics aside (changing those is not an adaptation, so it cannot relabel a copy). Same boundary as
 * sameSql: a hidden subject (nfull IS NULL) is compared on key-free facts only (match_hash and the
 * served image). Whenever sameSql holds, so does this.
 */
export const nearSql = (s: string, iq: string) =>
  // Round 10: a shown subject is also a copy of a STRIPPED serve it matches key-free (adding the key
  // is not an adaptation, so the show/hide choice cannot flip the outcome: s4), and of a serve it
  // matches without the explanation when it has none (dropping an explanation is not one either).
  `COALESCE(CASE WHEN ${s}.nfull IS NOT NULL THEN (${iq}.near_hash = ${s}.nfull OR (${s}.noexpl = 1 AND ${iq}.near_bare = ${s}.nbare)
     OR (${iq}.form = 'stripped' AND ${s}.noexpl = 1 AND ${iq}.near_kf = ${s}.nkf)) ELSE ${iq}.near_kf = ${s}.nkf END, 0)`;

/**
 * The author's own earlier work that was SHOWN (round 7 H1): full contents of publishes that did
 * not hide their key. Built only by ownShown, which drops every hidden publish's content, so a hidden
 * key of the author's other items can never decide an evidence outcome others see.
 */
export interface OwnShown {
  readonly shown: ReadonlySet<string>;
  readonly __brand: "own-shown";
}
export function ownShown(rows: { content: string; hidden: boolean }[]): OwnShown {
  return Object.freeze({ shown: new Set(rows.filter((r) => !r.hidden).map((r) => r.content)), __brand: "own-shown" as const });
}

/**
 * Evidence admissibility (critics round 4 D3 + round 5 E3d). A source counts unless it is the
 * publisher's own lineage (its item, or a chain through one) or came later than their own first
 * publish of the question. Such a source still counts when the publisher took ITS additions: a
 * publish that shows its key and equals the source's full served form, which is not the
 * publisher's own earlier SHOWN content (an original author copying an adapter's explanation verbatim).
 */
export function admissibleWith(
  h: Served & { item_id: string; chain_items: string[]; first_seq: number },
  s: Subject,
  mine: Set<string>,
  myFirstSeq: number | undefined,
  own: OwnShown | undefined,
  voided: Set<string> = new Set(),
  voidedLineage: Set<string> = voided,
): boolean {
  if (voided.has(h.item_id)) return false;
  // A source whose lineage runs through void (infringing) material counts only for a verbatim copy
  // of it, i.e. of its own additions (round 6 F3b): what else it shares came from the void item.
  if (h.chain_items.some((x) => voidedLineage.has(x)) && !h.same) return false;
  const derivedFromMine = mine.has(h.item_id) || h.chain_items.some((x) => mine.has(x));
  const later = myFirstSeq !== undefined && h.first_seq >= myFirstSeq;
  if (!derivedFromMine && !later) return true;
  if (mine.has(h.item_id)) return false;
  return s.full !== null && h.same_full && !(own?.shown.has(s.full) ?? false);
}
