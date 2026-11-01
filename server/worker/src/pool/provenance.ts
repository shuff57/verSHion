// Provenance and licence grants, shared by the publish check and the import response so that the
// two can never disagree (critic A4). Everything here is built from server records: the account's
// own import_questions rows and the item's previous publish. A client-declared origin is a hint
// that counts only when it names a question this account really imported, at that version.
import type { QuestionJson } from "./core.ts";
import { chainGrants, type Grant, type LicenceUnit } from "./license.ts";
import { fingerprint, keyedBody, keyedOne, keyedSet, ID_RE, limitOf, reserveBytes, sha256Hex, VISIBLE_TO_OTHERS, type Ctx } from "./shared.ts";
import { nearSql, ownShown, sameSql, type OwnShown, type SubjectParam } from "./visible.ts";

export interface ChainLink {
  author_name: string;
  item_id: string;
  qid: string;
  version: number;
  license: string;
  modified: boolean;
  /** A placeholder for links left out (no item): the licences they carried still grant. */
  collapsed_licenses?: string[];
  /** Round 14 (B5): a source the publisher attested independence of (a short or generic read). */
  possible_source?: boolean;
  /** Round 14 (B3): an earlier identical publication by another author, with no evidence of a serve. */
  earlier_match?: boolean;
  /** Round 17 (critic22c M6): what of the source this question uses when it is not the question
   *  itself: its image ("image also used by"), its explanation or model answer ("text"), or one of
   *  its questions as a part of this one, or this question as one of its parts ("part"). */
  kind?: "image" | "text" | "part";
}

/**
 * One source ITEM this account was served (imports and plain reads alike) that shares a question with
 * ONE position of the publish, as the evidence query groups it (round 8: by source item, not by
 * (item, qid), so a bank repeating a question counts once). Only admissible sources reach here: every
 * rule that does not compare content with the subject is applied in SQL before anything is ranked or
 * limited; the content comparisons are visible.ts' sameSql and nearSql.
 */
export interface ImportHit {
  pos: number;
  match_hash: string;
  qid: string;
  item_id: string;
  /** Links in the chain the source carried at its latest serve (which source a chain follows). */
  chain_len: number;
  /** The licences the source's chains grant (every serve's links; own links dropped, void links mapped to their rightful stand-in or dropped: in SQL). */
  chain_lics: string[];
  author_name: string;
  /** The licence at the latest serve (shown in chain links). */
  license: string;
  version: number;
  versions: number[];
  /** Earliest per-content seq of the source (for the "it came first" rule). */
  source_first_seq: number;
  stem_hash: string | null;
  kind: "import" | "view";
  /** Some serve equals the subject (sameSql); and in full form; and that serve was an import. */
  same: boolean;
  same_full: boolean;
  same_imported: boolean;
  /** Some serve is the same question in the licence sense (nearSql: points, difficulty, topics aside). */
  near: boolean;
  /** Some serve has the subject's key-free facts (admin flag "origin_modified" only). */
  kf_same: boolean;
  /** Every licence shown for any serve; for the serves equal to the subject; for the near ones. */
  lic_all: string[];
  lic_same: string[];
  lic_near: string[];
  /** Every licence shown, per served content (identical content across versions is one material: M-4). */
  lic_groups: string[][];
  /** Hashes of the contents served (sources sharing one were served the same question). */
  contents: string[];
  /** Matched by a declared or recorded origin (item, qid) rather than by match_hash. */
  by_ref: boolean;
  by_match: boolean;
  /** Found by a near-rewording of the stem (round 10 M-3), not by match_hash. */
  fuzzy?: boolean;
  /** Round 14 (B5): an attested-independent read of a short or generic question: a possible source only. */
  claimed?: boolean;
}

/** Source items examined per question: a FIXED allowance, whatever the size of the bank (round 8 I1). */
export const EVIDENCE_PER_POSITION = 32;
/** Credits kept per question (round 9 J6): distinct contributors, whatever the size of the bank. */
export const CREDITS_MAX = 64;
/** ... of which the first CREDITS_HEAD (the originators) are always kept, then the most recent (round 10 M-7). */
export const CREDITS_HEAD = 8;
/** Kept for callers that report the bound: groups loaded at most = EVIDENCE_PER_POSITION per question. */
export const EVIDENCE_GROUPS_MAX = 4000;
export const ATTRIBUTION_CHAINS_MAX = EVIDENCE_GROUPS_MAX;

export interface Evidence {
  byPos: Map<number, ImportHit[]>;
  /** Positions with more admissible source items than were examined, with how many there were. */
  overflow: Map<number, number>;
  /** Round 21: per question, the hard sources left out past the allowance (their licences). */
  cut: Map<number, FuzzyCut>;
  /**
   * Per position, the merged credits of the sources a chain follows (the identical ones if any, else
   * all; own items left out), oldest first, at most CREDITS_MAX, each item once (its earliest version,
   * "modified" if any path modified it), the sources' own links included; and how many there were.
   */
  credits: Map<number, { links: ChainLink[]; total: number; more: string[]; missing: number }>;
}

/** What the evidence query needs to know about one position, besides its Subject (all key-free). */
export interface PositionFacts {
  /** The publisher's first publish of this question (author_firsts / retained), or null. */
  first: number | null;
  /** The subject's full content is the publisher's own earlier SHOWN content (OwnShown). */
  ownshown: boolean;
  /** A short statement (treated as common for every source). */
  common: boolean;
  /** The key is itself a work (a Short's model answer): key-free equality never makes the author first (round 12 M1). */
  keywork?: number;
  /** The served question is 40 words or fewer (round 14 B5: independence may then cover a read). */
  short40?: number;
  /** Independent roots with their first publish (round 11 P7): common for a source only if K of them, not its own, came first. */
  croots?: { r: string; s: number; n: string }[];
  ck?: number;
  /** The publisher published this question (key-free) before, hidden or purged included (round 10 M-5). */
  ownkf?: boolean;
}

/**
 * The evidence for every position of a publish (round 7 H4, round 8 I1), aggregated in SQL. Applied
 * in SQL BEFORE ranking or limiting: imports always count; a read counts only for a distinctive or
 * declared question; never the publisher's own items, nor sources derived from them or first
 * published after the publisher's own first publish (unless the publisher copies ITS additions
 * verbatim); never void items, nor sources whose lineage runs through void material without a
 * rightful stand-in (unless copied verbatim); never items the publisher attests independence of
 * (a read always; an import when the question is common or the subject is not a copy of it in the
 * licence sense). Then per question at most EVIDENCE_PER_POSITION source items are examined (origins
 * first, then the same question, identical, imported, earliest). Credits (round 9 J6) and chain
 * licences are aggregated in SQL too, so neither depends on the size of the bank.
 */
export async function evidence(c: Ctx, me: string, subjects: (SubjectParam & PositionFacts)[], o: { mine?: string[]; independent?: string[]; reserve?: Reserver } = {}): Promise<Evidence> {
  // Round 11 P2: D1 results count against the isolate budget. The query is per position, so a big
  // bank is read EVIDENCE_CHUNK positions at a time, each chunk's worst-case result reserved first.
  const out: Evidence = { byPos: new Map(), overflow: new Map(), credits: new Map(), cut: new Map() };
  const chunk = Math.max(1, Math.floor(EVIDENCE_RESERVE / (EVIDENCE_ROW_BYTES * (EVIDENCE_PER_POSITION + 1))));
  for (let off = 0; off < subjects.length; off += chunk) {
    const part = subjects.slice(off, off + chunk);
    const release = (o.reserve ?? reserveBytes)(part.length * EVIDENCE_ROW_BYTES * (EVIDENCE_PER_POSITION + 1));
    try {
      const e = await evidenceChunk(c, me, part, o);
      for (const [k, v] of e.byPos) out.byPos.set(k + off, v.map((h) => ({ ...h, pos: h.pos + off })));
      for (const [k, v] of e.overflow) out.overflow.set(k + off, v);
      for (const [k, v] of e.cut) out.cut.set(k + off, v);
      for (const [k, v] of e.credits) out.credits.set(k + off, v);
    } finally {
      release();
    }
  }
  return out;
}
/**
 * The worst-case result one evidence query may hold (each position's EVIDENCE_PER_POSITION sources
 * and its credits row, EVIDENCE_ROW_BYTES each: measured 6.4 KB with 50 contents and 64 credits),
 * reserved before it runs; the positions per query follow from it (31 at the defaults).
 */
export const EVIDENCE_RESERVE = 8 * 1024 * 1024;
export const EVIDENCE_ROW_BYTES = 8 * 1024;

async function evidenceChunk(c: Ctx, me: string, subjects: (SubjectParam & PositionFacts)[], o: { mine?: string[]; independent?: string[] }): Promise<Evidence> {
  const byPos = new Map<number, ImportHit[]>();
  const overflow = new Map<number, number>();
  const cut = new Map<number, FuzzyCut>();
  const credits = new Map<number, { links: ChainLink[]; total: number; more: string[]; missing: number }>();
  if (subjects.length === 0) return { byPos, overflow, credits, cut };
  const per = EVIDENCE_PER_POSITION;
  const sql = `WITH subj AS MATERIALIZED (SELECT CAST(j.key AS INTEGER) AS pos, j.value ->> '$.mh' AS mh, j.value ->> '$.full' AS full, j.value ->> '$.kf' AS kf, j.value -> '$.refs' AS refs,
                                          j.value ->> '$.nfull' AS nfull, j.value ->> '$.nkf' AS nkf, j.value ->> '$.nbare' AS nbare, COALESCE(j.value ->> '$.noexpl', 0) AS noexpl,
                                          j.value ->> '$.first' AS first, COALESCE(j.value ->> '$.ownshown', 0) AS ownshown, COALESCE(j.value ->> '$.common', 0) AS common,
                                          COALESCE(j.value ->> '$.ownkf', 0) AS ownkf, COALESCE(j.value -> '$.croots', '[]') AS croots, COALESCE(j.value ->> '$.ck', 3) AS ck,
                                          COALESCE(j.value ->> '$.keywork', 0) AS keywork, COALESCE(j.value ->> '$.short40', 0) AS short40 FROM json_each(?2) j),
    mine AS MATERIALIZED (SELECT value AS id FROM json_each(?4)),
    indep AS MATERIALIZED (SELECT value AS id FROM json_each(?5)),
    bare AS MATERIALIZED (SELECT item_id AS id FROM void_items WHERE replaced_by IS NULL),
    -- (Round 19: the served publishes' own rows, live or ghost, each driven from its hash index.)
    hit AS (${["import_questions_live", "import_questions_ghost"]
      .map(
        (v, k) => `
      SELECT s.pos, 0 AS by_ref, ${sameSql("s", "iq")} AS same, ${nearSql("s", "iq")} AS near, (iq.keyfree_hash = s.kf) AS kf_same, s.first AS sfirst, s.full AS sfull, s.ownshown AS sown, s.common AS sshort, s.croots AS scroots, s.ck AS sck, s.ownkf AS sownkf, s.noexpl AS snoexpl, s.keywork AS skeywork, s.short40 AS sshort40, iq.*
        FROM subj s CROSS JOIN ${v} iq ON iq.user_id = ?1 AND iq.match_hash = s.mh
      UNION ALL
      SELECT s.pos, 1, ${sameSql("s", "iq")}, ${nearSql("s", "iq")}, (iq.keyfree_hash = s.kf), s.first, s.full, s.ownshown, s.common, s.croots, s.ck, s.ownkf, s.noexpl, s.keywork, s.short40, iq.*
        FROM subj s CROSS JOIN json_each(s.refs) r CROSS JOIN served sv ON sv.reader_id = ?1 AND sv.item_id = r.value ->> '$.item'
             CROSS JOIN ${v} iq ON iq.src_publish_id = sv.publish_id AND iq.user_id = ?1 AND iq.qid = r.value ->> '$.qid'${k === 0 ? "\n      UNION ALL" : ""}`,
      )
      .join("")}),
    -- "Common" (rounds 11 P7, 12 M2): K independent roots other than the source's own that served the
    -- SAME content as this serve (licence sense: choices, shown keys, explanation, sample) before it,
    -- never the bare stem. "First" (round 12 M1): the publisher's own first publish of THIS content
    -- (shown, or key-free unless the key is itself a work: a Short's model answer), never of a stem.
    hitc AS (
      SELECT h.*, (h.sshort = 1 OR (SELECT COUNT(DISTINCT cr.value ->> '$.r') FROM json_each(h.scroots) cr
                                     WHERE CAST(cr.value ->> '$.s' AS INTEGER) < h.source_first_seq AND cr.value ->> '$.n' = h.near_hash
                                       AND cr.value ->> '$.r' <> COALESCE((SELECT ri.author_id FROM items ri WHERE ri.id = NULLIF(CASE WHEN COALESCE(h.chain_json ->> '$[0].earlier_match', 0) THEN '' ELSE h.chain_json ->> '$[0].item_id' END, '')), h.author_id)) >= h.sck) AS scommon,
                  (SELECT MIN(ac.first_seq) FROM author_contents ac WHERE ac.author_id = ?1
                      AND ((ac.kind = 'nshown' AND ac.hash = h.near_hash) OR (h.skeywork = 0 AND ac.kind = 'kf' AND ac.hash = h.near_kf))) AS owncf,
                  -- The publisher served THIS content (licence sense) first: the source added nothing of its own.
                  EXISTS (SELECT 1 FROM author_contents ac WHERE ac.author_id = ?1 AND ac.kind = 'nshown' AND ac.hash = h.near_hash AND ac.first_seq <= h.source_first_seq) AS ownnear,
                  -- Round 16 (critic21c B3): the publisher's first publish of this question key-free (its
                  -- served form, hidden key or not): a copy served to her later never binds her.
                  (SELECT MIN(ac.first_seq) FROM author_contents ac WHERE ac.author_id = ?1 AND ac.kind = 'kf' AND ac.hash = h.near_kf) AS ownkfseq,
                  -- Round 14 (B5): an exact copy of something only READ, of a short or generic question (40
                  -- words or fewer as served, or 2+ independent roots have it), CAN be attested independent:
                  -- it then binds nothing and is credited as a possible source (badge claimed_independent).
                  (h.item_id IN (SELECT id FROM indep) AND h.kind = 'view' AND h.near = 1
                     AND (h.sshort40 = 1 OR (SELECT COUNT(DISTINCT cr.value ->> '$.r') FROM json_each(h.scroots) cr WHERE cr.value ->> '$.n' = h.near_hash) >= 2)) AS claimed
        FROM hit h),
    rows AS MATERIALIZED (
      SELECT h.*, EXISTS (SELECT 1 FROM json_each(h.chain_json) cl WHERE cl.value ->> '$.item_id' IN (SELECT id FROM mine)) AS derived FROM hitc h
       WHERE h.author_id <> ?1 AND h.void_at IS NULL
         AND h.item_id NOT IN (SELECT id FROM mine)
         AND NOT EXISTS (SELECT 1 FROM void_items v WHERE v.item_id = h.item_id)
         AND (h.same OR NOT EXISTS (SELECT 1 FROM json_each(h.chain_json) cl WHERE cl.value ->> '$.item_id' IN (SELECT id FROM bare)))
         AND (h.kind = 'import' OR h.by_ref = 1 OR h.scommon = 0)
         -- Independence (round 12 M5: reads and imports alike): never for an exact copy (in the licence
         -- sense) of what the account was served; only for a common question (or a short statement), a
         -- changed copy, or the publisher's own question first (round 10 M-5: the original author).
         -- (A B5 claimed row passes this: an exact, uncommon copy that is not the publisher's own first.)
         AND NOT (h.item_id IN (SELECT id FROM indep) AND (h.scommon = 1 OR h.near = 0 OR h.sownkf = 1))),
    adm AS MATERIALIZED (
      SELECT * FROM rows
       WHERE NOT ((derived OR (owncf IS NOT NULL AND COALESCE(source_first_seq, 0) >= owncf))
                  -- The exception (the publisher copies a later source's ADDITIONS) never applies when the
                  -- publisher published this question first and the source added no explanation, only the
                  -- key (round 10 M-5: key-free, so a hidden key decides nothing).
                  -- Round 12 M1: a copy in the licence sense (points or difficulty changed) too, not only a byte-identical one.
                  AND NOT (sfull IS NOT NULL AND near AND form = 'full' AND sown = 0 AND ownnear = 0 AND NOT (sownkf = 1 AND snoexpl = 1)))
         -- (Round 16 B3: whatever it added; copying its explanation or model answer is a lift, publish.ts.)
         AND NOT (skeywork = 0 AND ownkfseq IS NOT NULL AND COALESCE(source_first_seq, 0) >= ownkfseq)),
    g AS (SELECT pos, item_id, MIN(qid) AS qid, MIN(match_hash) AS match_hash, MAX(by_ref) AS by_ref, MIN(by_ref) = 0 AS by_match, MAX(same) AS same, MAX(near) AS near,
                 MAX(same AND form = 'full') AS same_full, MAX(same AND kind = 'import') AS same_imported, MAX(kf_same) AS kf_same,
                 MIN(COALESCE(source_first_seq, 0)) AS first_seq, MAX(kind = 'import') AS imported, MAX(version) AS version,
                 MAX(printf('%016d|%s', at, author_name)) AS an, MAX(printf('%016d|%s', at, license)) AS ll, MAX(stem_hash) AS stem_hash,
                 MAX(printf('%016d|', at) || chain_json) AS chain_latest, MIN(at) AS first_at, json_group_array(DISTINCT content_hash) AS contents, MIN(claimed) AS claimed
            FROM adm GROUP BY pos, item_id),
    -- Round 20 (critic25b F1): copies (licence sense) first, EARLIEST first: past the allowance only later
    -- copies or adaptations are left out, which never change a copy's outcome (its earliest original
    -- decides; adaptations beside a copy do not narrow it). Without a copy, an overflow is refused
    -- (publish.ts): a cap never skips.
    ranked AS (SELECT g.*, ROW_NUMBER() OVER (PARTITION BY pos ORDER BY near DESC, CASE WHEN near THEN first_seq END, by_ref DESC, same DESC, imported DESC, first_seq, first_at, item_id) AS rn,
                      COUNT(*) OVER (PARTITION BY pos) AS cnt FROM g),
    keep AS MATERIALIZED (SELECT * FROM ranked WHERE rn <= ?3),
    -- Round 21: the hard sources left out (a copy, an import or a declared origin): their licences (as
    -- shown and in their chains), so the publish can tell whether one would forbid the licence chosen.
    cutx0 AS MATERIALIZED (SELECT pos, item_id, v FROM (
                 SELECT a.pos, a.item_id, l.value AS v FROM ranked rk JOIN adm a ON a.pos = rk.pos AND a.item_id = rk.item_id, json_each(a.licenses_json) l
                  WHERE rk.rn > ?3 AND (rk.near = 1 OR rk.imported = 1 OR rk.by_ref = 1)
                 UNION ALL
                 SELECT a.pos, a.item_id, cl.value ->> '$.license' FROM ranked rk JOIN adm a ON a.pos = rk.pos AND a.item_id = rk.item_id, json_each(a.chain_json) cl
                  WHERE rk.rn > ?3 AND (rk.near = 1 OR rk.imported = 1 OR rk.by_ref = 1) AND COALESCE(cl.value ->> '$.license', '') <> '')),
    cutx AS MATERIALIZED (SELECT pos, json_group_array(v) AS lics, json_group_object(v, item) AS byl, MIN(item) AS item, (SELECT COUNT(DISTINCT c2.item_id) FROM cutx0 c2 WHERE c2.pos = cl1.pos) AS n
                            FROM (SELECT pos, v, MIN(item_id) AS item FROM cutx0 GROUP BY pos, v) cl1 GROUP BY pos),
    ka AS MATERIALIZED (SELECT a.* FROM adm a JOIN keep k ON k.pos = a.pos AND k.item_id = a.item_id),
    la AS MATERIALIZED (SELECT pos, item_id, json_group_array(DISTINCT l.value) AS lic_all, json_group_array(DISTINCT CASE WHEN same THEN l.value END) AS lic_same,
                               json_group_array(DISTINCT CASE WHEN near THEN l.value END) AS lic_near
                          FROM ka, json_each(ka.licenses_json) l GROUP BY pos, item_id),
    lg AS MATERIALIZED (SELECT pos, item_id, json_group_array(json(g)) AS lic_groups FROM (
                          SELECT pos, item_id, content_hash, json_group_array(DISTINCT l.value) AS g FROM ka, json_each(ka.licenses_json) l GROUP BY pos, item_id, content_hash)
                         GROUP BY pos, item_id),
    vs AS MATERIALIZED (SELECT pos, item_id, json_group_array(DISTINCT v.value) AS versions FROM ka, json_each(ka.versions_json) v GROUP BY pos, item_id),
    -- Chain licences per source (every serve's chain): own links grant nothing; a void link grants
    -- as its rightful stand-in (the licence at void time), or nothing without one.
    cl0 AS MATERIALIZED (SELECT ka.pos, ka.item_id AS src, cl.value ->> '$.item_id' AS lid, cl.value ->> '$.license' AS lic, cl.value -> '$.collapsed_licenses' AS col FROM ka, json_each(ka.chain_json) cl),
    cl1 AS (SELECT c0.pos, c0.src, c0.col,
                   CASE WHEN v.item_id IS NULL THEN c0.lic
                        WHEN v.replaced_by IS NULL OR v.replaced_by IN (SELECT id FROM mine) THEN NULL
                        ELSE COALESCE(v.replaced_license, (SELECT license FROM items WHERE id = v.replaced_by)) END AS lic
              FROM cl0 c0 LEFT JOIN void_items v ON v.item_id = c0.lid
             WHERE c0.lid IS NULL OR c0.lid = '' OR c0.lid NOT IN (SELECT id FROM mine)),
    cg AS MATERIALIZED (SELECT pos, src AS item_id, json_group_array(DISTINCT x) AS chain_lics FROM (
                          SELECT pos, src, lic AS x FROM cl1 WHERE lic IS NOT NULL AND lic <> ''
                          UNION ALL SELECT cl1.pos, cl1.src, cv.value FROM cl1, json_each(COALESCE(cl1.col, '[]')) cv) GROUP BY pos, src),
    -- Credits (round 9 J6): the chains of the sources a chain follows, merged per item.
    ps AS (SELECT pos, MAX(same) AS any_same FROM keep GROUP BY pos),
    cr0 AS (
      SELECT k.pos, printf('%020d|%s|%06d', k.first_seq, k.item_id, CAST(l.key AS INTEGER)) AS ord, l.value AS link, l.value ->> '$.item_id' AS lid
        FROM keep k JOIN ps ON ps.pos = k.pos CROSS JOIN json_each(substr(k.chain_latest, 18)) l WHERE k.same = 1 OR ps.any_same = 0
      UNION ALL
      SELECT k.pos, printf('%020d|%s|%06d', k.first_seq, k.item_id, 999999),
             json_object('author_name', substr(k.an, 18), 'item_id', k.item_id, 'qid', k.qid, 'version', k.version, 'license', substr(k.ll, 18), 'modified', json(CASE WHEN k.same THEN 'false' ELSE 'true' END)), k.item_id
        FROM keep k JOIN ps ON ps.pos = k.pos WHERE k.same = 1 OR ps.any_same = 0),
    cr1 AS MATERIALIZED (SELECT pos, ord, link, CASE WHEN COALESCE(lid, '') = '' THEN link ELSE lid END AS gk FROM cr0 WHERE COALESCE(lid, '') = '' OR lid NOT IN (SELECT id FROM mine)),
    cm AS MATERIALIZED (SELECT pos, gk, MIN(ord) AS ord, MAX(COALESCE(link ->> '$.modified', 0)) AS anymod FROM cr1 GROUP BY pos, gk),
    cp AS (SELECT pos, gk, link, ROW_NUMBER() OVER (PARTITION BY pos, gk ORDER BY CAST(COALESCE(link ->> '$.version', 0) AS INTEGER), ord) AS r FROM cr1),
    cred AS (SELECT cm.pos, cm.ord, json_set(cp.link, '$.modified', json(CASE WHEN cm.anymod THEN 'true' ELSE 'false' END)) AS link,
                    ROW_NUMBER() OVER (PARTITION BY cm.pos ORDER BY cm.ord) AS n, COUNT(*) OVER (PARTITION BY cm.pos) AS total
               FROM cm JOIN cp ON cp.pos = cm.pos AND cp.gk = cm.gk AND cp.r = 1),
    -- Past CREDITS_MAX (round 10 M-7): the first CREDITS_HEAD (the originators) and the most recent
    -- rest are kept; what is between is one placeholder with the licences it carried.
    credj AS (SELECT pos, json_group_array(json(link)) AS links, MAX(total) AS total FROM (SELECT * FROM cred WHERE n <= ?7 OR n > total - (?6 - ?7) ORDER BY pos, ord) GROUP BY pos),
    -- Past the credit allowance, what the left-out links granted is kept (a placeholder's licences).
    -- How many contributors the left-out links stand for (a placeholder among them counts its own number, round 11).
    credn AS (SELECT pos, SUM(CASE WHEN COALESCE(link ->> '$.item_id', '') = '' AND link ->> '$.author_name' GLOB 'and [0-9]* contributors between'
                                   THEN CAST(substr(link ->> '$.author_name', 5) AS INTEGER) ELSE 1 END) AS missing
                FROM cred WHERE n > ?7 AND n <= total - (?6 - ?7) GROUP BY pos),
    credm AS (SELECT pos, json_group_array(DISTINCT x) AS more FROM (
                SELECT pos, link ->> '$.license' AS x FROM cred WHERE n > ?7 AND n <= total - (?6 - ?7)
                UNION ALL SELECT cred.pos, cv.value FROM cred, json_each(COALESCE(cred.link -> '$.collapsed_licenses', '[]')) cv WHERE cred.n > ?7 AND cred.n <= cred.total - (?6 - ?7))
               WHERE x IS NOT NULL AND x <> '' GROUP BY pos)
    SELECT k.pos, k.item_id, k.qid, k.match_hash, k.by_ref, k.by_match, k.same, k.near, k.same_full, k.same_imported, k.kf_same, k.first_seq, k.imported, k.version, k.an, k.ll,
           k.stem_hash, json_array_length(substr(k.chain_latest, 18)) AS chain_len, k.contents, k.rn, k.cnt, k.claimed,
           la.lic_all, la.lic_same, la.lic_near, lg.lic_groups, vs.versions, cg.chain_lics,
           CASE WHEN k.rn = 1 THEN (SELECT json_object('lics', json(cx.lics), 'item', cx.item, 'n', cx.n, 'by', json(cx.byl)) FROM cutx cx WHERE cx.pos = k.pos) END AS cutj,
           CASE WHEN k.rn = 1 THEN credj.links END AS credits, CASE WHEN k.rn = 1 THEN credj.total END AS credits_total, CASE WHEN k.rn = 1 THEN credm.more END AS credits_more, CASE WHEN k.rn = 1 THEN credn.missing END AS credits_missing FROM keep k
      LEFT JOIN la ON la.pos = k.pos AND la.item_id = k.item_id
      LEFT JOIN lg ON lg.pos = k.pos AND lg.item_id = k.item_id
      LEFT JOIN vs ON vs.pos = k.pos AND vs.item_id = k.item_id
      LEFT JOIN cg ON cg.pos = k.pos AND cg.item_id = k.item_id
      LEFT JOIN credj ON credj.pos = k.pos
      LEFT JOIN credm ON credm.pos = k.pos
      LEFT JOIN credn ON credn.pos = k.pos
     ORDER BY k.pos, k.rn`;
  const r = await c.db
    .prepare(sql)
    .bind(me, JSON.stringify(subjects), per, JSON.stringify(o.mine ?? []), JSON.stringify(o.independent ?? []), CREDITS_MAX, CREDITS_HEAD)
    .all<Record<string, any>>();
  const strs = (v: unknown) => (JSON.parse((v as string) ?? "[]") as (string | null)[]).filter((l): l is string => typeof l === "string");
  for (const x of r.results) {
    const h: ImportHit = {
      pos: x.pos,
      match_hash: x.match_hash,
      qid: x.qid,
      item_id: x.item_id,
      chain_len: x.chain_len ?? 0,
      chain_lics: strs(x.chain_lics),
      author_name: String(x.an).slice(17),
      license: String(x.ll).slice(17),
      version: x.version,
      versions: JSON.parse(x.versions ?? "[]") as number[],
      source_first_seq: x.first_seq,
      stem_hash: x.stem_hash,
      kind: x.imported ? "import" : "view",
      same: x.same === 1,
      same_full: x.same_full === 1,
      same_imported: x.same_imported === 1,
      near: x.near === 1,
      kf_same: x.kf_same === 1,
      lic_all: strs(x.lic_all),
      lic_same: strs(x.lic_same),
      lic_near: strs(x.lic_near),
      lic_groups: (JSON.parse(x.lic_groups ?? "[]") as unknown[]).map((g) => (Array.isArray(g) ? g : []).filter((l): l is string => typeof l === "string")),
      contents: strs(x.contents),
      by_ref: x.by_ref === 1,
      by_match: x.by_match === 1,
      ...(x.claimed === 1 ? { claimed: true } : {}),
    };
    byPos.set(h.pos, [...(byPos.get(h.pos) ?? []), h]);
    if (x.cnt > per) overflow.set(h.pos, x.cnt);
    if (x.cutj) cut.set(h.pos, JSON.parse(x.cutj) as FuzzyCut);
    if (x.credits) credits.set(h.pos, { links: JSON.parse(x.credits) as ChainLink[], total: x.credits_total ?? 0, more: strs(x.credits_more), missing: x.credits_missing ?? 0 });
  }
  return { byPos, overflow, credits, cut };
}

/**
 * How a source bears on a publish (round 9 J1-J3, one rule for reads and imports):
 * * `verbatim`: the subject is the same question in the licence sense (nearSql: what was served,
 *   points, difficulty and topics aside), whether it was read or imported. Hard: it can refuse.
 * * `adapted`: an import, or an origin the author declared now, that the subject changed. Hard.
 * * `soft`: a plain read the subject changed (the server's own record of an earlier publish's
 *   source included: it keeps the bearing it had). Credited and flagged, never a refusal.
 */
export type Bearing = "verbatim" | "adapted" | "soft";
export function bearingOf(h: ImportHit, declared = false): Bearing {
  // An attested-independent read of a short or generic question (round 14 B5) binds nothing.
  if (h.claimed) return "soft";
  if (h.near) return "verbatim";
  if (declared || h.kind === "import") return "adapted";
  return "soft";
}
export const isHard = (h: ImportHit, declared = false) => bearingOf(h, declared) !== "soft";

/** The licence unit of one source, as its bearing reads it (round 10: own licences grouped by served content). */
export function unitOf(h: ImportHit): LicenceUnit {
  const verbatim = h.near;
  return {
    item: h.item_id,
    verbatim,
    chain: h.chain_lics,
    own: verbatim ? [h.lic_near.length ? h.lic_near : h.lic_all] : h.lic_groups.length ? h.lic_groups : [h.lic_all],
    contents: h.contents,
    seq: h.source_first_seq,
  };
}

/** Served questions whose loose stem hash matches (a soft signal only). */
/** Served questions whose loose stem hash matches, with the earliest first publish of each (round 11: so the author's own earlier work is never flagged). */
export async function stemHits(c: Ctx, me: string, stems: string[]): Promise<Map<string, number>> {
  if (stems.length === 0) return new Map();
  const r = await c.db
    .prepare(`SELECT stem_hash, MIN(source_first_seq) AS s FROM import_questions WHERE user_id = ?1 AND author_id <> ?1 AND void_at IS NULL AND stem_hash IN (SELECT value FROM json_each(?2)) GROUP BY stem_hash`)
    .bind(me, JSON.stringify([...new Set(stems)]))
    .all<{ stem_hash: string; s: number }>();
  return new Map(r.results.map((x) => [x.stem_hash, x.s]));
}

/** SQL: served row `s` holds question `j.value ->> '$.qid'` (its live publish or its ghost). */
const HAS_Q = `(EXISTS (SELECT 1 FROM item_questions hq WHERE hq.publish_id = s.publish_id AND hq.qid = j.value ->> '$.qid')
               OR EXISTS (SELECT 1 FROM ghost_questions hg WHERE hg.publish_id = s.publish_id AND hg.qid = j.value ->> '$.qid'))`;

/**
 * Recorded origins (`item/qid`) whose evidence for `me` was voided (round 6 F3a): a kept question's
 * provenance from such a record is void too, never reinstated as "a record that is merely gone".
 */
export async function voidedRefs(c: Ctx, me: string, refs: { item: string; qid: string }[]): Promise<Set<string>> {
  const out = new Set<string>();
  if (refs.length === 0) return out;
  const r = await c.db
    .prepare(
      // (Round 19: per recorded origin, the reader's served publishes of that item holding the question.)
      `SELECT DISTINCT j.value ->> '$.item' AS item_id, j.value ->> '$.qid' AS qid FROM json_each(?2) j
        WHERE EXISTS (SELECT 1 FROM served s WHERE s.reader_id = ?1 AND s.item_id = j.value ->> '$.item' AND s.void_at IS NOT NULL AND ${HAS_Q})
          AND NOT EXISTS (SELECT 1 FROM served s WHERE s.reader_id = ?1 AND s.item_id = j.value ->> '$.item' AND s.void_at IS NULL AND ${HAS_Q})
          AND NOT EXISTS (SELECT 1 FROM void_items vr WHERE vr.item_id = j.value ->> '$.item' AND vr.replaced_by IS NOT NULL)
       UNION SELECT j.value ->> '$.item', j.value ->> '$.qid' FROM json_each(?2) j WHERE EXISTS (SELECT 1 FROM void_items vi WHERE vi.item_id = j.value ->> '$.item' AND vi.replaced_by IS NULL)`,
    )
    .bind(me, JSON.stringify(refs))
    .all<{ item_id: string; qid: string }>();
  for (const x of r.results) out.add(`${x.item_id}/${x.qid}`);
  return out;
}

/**
 * What `me` authored, for the evidence rules: their item ids, per match_hash their first publish
 * (seq) and their full contents; and the void (infringing) items among the ones that matter.
 */
export interface AuthorCtx {
  mine: Set<string>;
  firstAt: Map<string, number>;
  own: Map<string, OwnShown>;
  /** void_items among the hits' items and chain links (round 6 F3). */
  voided: Set<string>;
  /** ... and the rightful-author links standing in for some of them (round 7 M1). */
  replaced: Map<string, ChainLink>;
  /** Questions (match_hash) at least K other INDEPENDENT authors' visible items use: views of them are not evidence (F5, round 10 M-2). */
  common: Set<string>;
  /** Key-free contents (near_kf) the author published at all, hidden or purged included (round 10 M-5). */
  ownKf: Set<string>;
  /** Per question, its independent roots' first publishes, the K+1 earliest (round 11 P7). */
  roots: Map<string, { r: string; s: number; n: string }[]>;
  k: number;
}
/** Root contents kept per question for the "common" test (the earliest first). */
export const ROOTS_PER_QUESTION = 64;

export async function authorContext(c: Ctx, me: string, hashes: string[]): Promise<AuthorCtx> {
  const items = await c.db.prepare(`SELECT id FROM items WHERE author_id = ?1`).bind(me).all<{ id: string }>();
  const firstAt = new Map<string, number>();
  const own = new Map<string, OwnShown>();
  const common = new Set<string>();
  const roots = new Map<string, { r: string; s: number; n: string }[]>();
  let commonK = 3;
  const ownKf = new Set<string>();
  const uniq = JSON.stringify([...new Set(hashes)]);
  if (hashes.length > 0) {
    // Own work: contents of retained publishes, with whether that publish hid its key (ownShown keeps
    // only the shown ones, round 7 H1). First publish: the durable per-content record (author_firsts,
    // never lowered by retention or purge, round 7 H2) or, failing it, the retained rows.
    const r = await c.db
      .prepare(
        `SELECT iq.match_hash, MIN(iq.seq) AS at, json_group_array(json_array(iq.full_content, p.hide_key)) AS fulls FROM item_questions iq
           JOIN item_publishes p ON p.publish_id = iq.publish_id AND p.author_id = ?1 AND p.status IN ('current', 'previous')
          WHERE iq.match_hash IN (SELECT value FROM json_each(?2)) GROUP BY iq.match_hash`,
      )
      .bind(me, uniq)
      .all<{ match_hash: string; at: number; fulls: string }>();
    for (const x of r.results) {
      firstAt.set(x.match_hash, x.at);
      own.set(x.match_hash, ownShown((JSON.parse(x.fulls) as [string, number][]).map(([content, hid]) => ({ content, hidden: hid !== 0 }))));
    }
    const dur = await c.db
      .prepare(`SELECT match_hash, first_seq FROM author_firsts WHERE author_id = ?1 AND match_hash IN (SELECT value FROM json_each(?2))`)
      .bind(me, uniq)
      .all<{ match_hash: string; first_seq: number }>();
    for (const x of dur.results) firstAt.set(x.match_hash, Math.min(x.first_seq, firstAt.get(x.match_hash) ?? Infinity));
    // Durable own contents (round 10 M-5): shown ones join OwnShown (a purged or retained-away
    // original is still the author's own); key-free ones say the author published that question.
    const ac = await c.db
      .prepare(`SELECT match_hash, kind, hash FROM author_contents WHERE author_id = ?1 AND match_hash IN (SELECT value FROM json_each(?2))`)
      .bind(me, uniq)
      .all<{ match_hash: string; kind: string; hash: string }>();
    const shownMore = new Map<string, string[]>();
    for (const x of ac.results) {
      if (x.kind === "kf") ownKf.add(x.hash);
      else shownMore.set(x.match_hash, [...(shownMore.get(x.match_hash) ?? []), x.hash]);
    }
    for (const [mh, list] of shownMore) own.set(mh, ownShown([...(own.get(mh)?.shown ?? []), ...list].map((content) => ({ content, hidden: false }))));
    const k = await limitOf(c, "*", "block_common_k");
    const cm = await c.db
      .prepare(
        // (Round 14: an "earlier identical publication" link is not lineage: it never makes a root.)
        // Independent authors only (round 10 M-2): each user counts by the ROOT of its question's lineage
        // (the first link of its chain, else itself), so copies descending from one source never make
        // it "common"; the publisher's own lineage never counts.
        // Round 11 P7: each root with the first publish of the question under it, the K+1 earliest; a
        // source is "common" only if K roots other than its own published it BEFORE it (evidence SQL).
        // Round 12 M2: per root, each served CONTENT (licence sense) with its first publish.
        `SELECT match_hash, root, n, MIN(seq) AS s FROM (
           SELECT iq.match_hash, iq.seq, CASE WHEN p.hide_key = 1 THEN iq.near_stripped ELSE iq.near_full END AS n,
                  COALESCE((SELECT ri.author_id FROM items ri WHERE ri.id = NULLIF(CASE WHEN COALESCE(iq.chain_json ->> '$[0].earlier_match', 0) THEN '' ELSE iq.chain_json ->> '$[0].item_id' END, '')), i.author_id) AS root
             FROM item_questions iq JOIN items i ON i.current_publish_id = iq.publish_id JOIN item_publishes p ON p.publish_id = iq.publish_id JOIN users u ON u.id = i.author_id
            WHERE iq.match_hash IN (SELECT value FROM json_each(?2)) AND i.author_id <> ?1 AND ${VISIBLE_TO_OTHERS})
          WHERE root <> ?1 AND n IS NOT NULL GROUP BY match_hash, root, n ORDER BY match_hash, s`,
      )
      .bind(me, uniq)
      .all<{ match_hash: string; root: string; n: string; s: number }>();
    for (const x of cm.results) {
      const l = roots.get(x.match_hash) ?? [];
      if (l.length < ROOTS_PER_QUESTION) l.push({ r: x.root, s: x.s, n: x.n });
      roots.set(x.match_hash, l);
    }
    for (const [mh, l] of roots) if (new Set(l.map((x) => x.r)).size >= k) common.add(mh);
    commonK = k;
  }
  return { mine: new Set(items.results.map((x) => x.id)), firstAt, own, voided: new Set(), replaced: new Map(), common, ownKf, roots, k: commonK };
}

/**
 * The publisher's first publish of each subject's CONTENT (round 12 M1): the licence-sense content
 * they served (`n`), or its key-free form unless the key is itself a work. Never a stem or a
 * match_hash alone, so publishing a bare stem early makes no one "first" for a richer question.
 */
export async function ownContentFirst(c: Ctx, me: string, subj: { n: string | null; kf: string; keywork: number }[]): Promise<Map<number, number>> {
  const out = new Map<number, number>();
  if (subj.length === 0) return out;
  const r = await c.db
    .prepare(
      // (Two indexed lookups, never an OR over the author's rows: round 17, their units made it slow.)
      `SELECT CAST(j.key AS INTEGER) AS pos,
              NULLIF(MIN(COALESCE((SELECT MIN(ac.first_seq) FROM author_contents ac WHERE ac.author_id = ?1 AND ac.kind = 'nshown' AND ac.hash = j.value ->> '$.n'), 9e18),
                         COALESCE((SELECT MIN(ac.first_seq) FROM author_contents ac WHERE ac.author_id = ?1 AND ac.kind = 'kf' AND ac.hash = j.value ->> '$.kf' AND CAST(j.value ->> '$.w' AS INTEGER) = 0), 9e18)), 9e18) AS s
         FROM json_each(?2) j`,
    )
    .bind(me, JSON.stringify(subj.map((x) => ({ n: x.n, kf: x.kf, w: x.keywork }))))
    .all<{ pos: number; s: number | null }>();
  for (const x of r.results) if (x.s !== null && x.s > 0) out.set(x.pos, x.s);
  return out;
}

/** The void items among `ids`. */
export async function voidedItems(c: Ctx, ids: string[]): Promise<Set<string>> {
  return (await voidInfo(c, ids)).voided;
}

/**
 * The void items among `ids`, and for those the admin tied to the rightful author's item (round 7
 * M1), the link that stands in for them: that item, its author and its current licence.
 */
export async function voidInfo(c: Ctx, ids: string[]): Promise<{ voided: Set<string>; replaced: Map<string, ChainLink> }> {
  const voided = new Set<string>();
  const replaced = new Map<string, ChainLink>();
  const want = [...new Set(ids.filter((x) => ID_RE.test(x)))];
  if (want.length === 0) return { voided, replaced };
  const r = await c.db
    .prepare(
      `SELECT v.item_id, v.replaced_by, COALESCE(v.replaced_license, i.license) AS license, i.public_version,
              COALESCE((SELECT ua.display_name FROM users ua WHERE ua.id = COALESCE(v.replaced_author_id, i.author_id) AND ua.deleted_at IS NULL), 'deleted teacher') AS author_name,
              CASE WHEN i.id IS NOT NULL AND ${VISIBLE_TO_OTHERS} THEN 1 ELSE 0 END AS shown
         FROM void_items v LEFT JOIN items i ON i.id = v.replaced_by AND i.purged_at IS NULL LEFT JOIN users u ON u.id = i.author_id
        WHERE v.item_id IN (SELECT value FROM json_each(?1))`,
    )
    .bind(JSON.stringify(want))
    .all<{ item_id: string; replaced_by: string | null; license: string | null; public_version: number | null; author_name: string; shown: number }>();
  for (const x of r.results) {
    voided.add(x.item_id);
    // The stand-in carries the licence and name of the rightful item AT VOID TIME (round 8); an item
    // not visible to others (unlisted, held, removed) is never named: a neutral label, same licence.
    if (x.replaced_by && x.license)
      replaced.set(
        x.item_id,
        x.shown
          ? { author_name: x.author_name, item_id: x.replaced_by, qid: "", version: x.public_version ?? 1, license: x.license, modified: true }
          : { author_name: "the rightful author", item_id: "", qid: "", version: 0, license: x.license, modified: true },
      );
  }
  return { voided, replaced };
}

/**
 * A source's stored chain without links to the publisher's own items or to void items; a void link
 * the admin tied to the rightful author's item is replaced by a link to that item (round 7 M1).
 */
export function chainOf(h: { chain_json: string }, drop: Set<string>, replaced: Map<string, ChainLink> = new Map()): ChainLink[] {
  return chainOfLinks(JSON.parse(h.chain_json) as ChainLink[], drop, replaced);
}

export function chainOfLinks(links: ChainLink[], drop: Set<string>, replaced: Map<string, ChainLink> = new Map()): ChainLink[] {
  const out: ChainLink[] = [];
  const seen = new Set<string>();
  for (const l of links) {
    // A stand-in keeps the replaced link's "modified" (a verbatim copy of the infringing item is a
    // verbatim copy of the rightful one).
    const rep = replaced.get(l.item_id);
    const x = rep ? { ...rep, modified: l.modified } : drop.has(l.item_id) ? null : l;
    if (!x || drop.has(x.item_id) || (x.item_id !== "" && seen.has(x.item_id))) continue;
    seen.add(x.item_id);
    out.push(x);
  }
  return out;
}

/** A loose stem hash: case-folded letters and digits only. */
export async function stemHash(stem: unknown): Promise<string | null> {
  if (typeof stem !== "string") return null;
  const norm = stem.normalize("NFKC").toLowerCase().replace(/[^\p{L}\p{N}]+/gu, " ").trim();
  if (norm.length < 12) return null; // too short to mean anything
  return fingerprint("stem", `vershion/stem/v1\n${norm}`);
}

export async function ownItemSet(c: Ctx, me: string, ids: string[]): Promise<Set<string>> {
  const want = [...new Set(ids.filter((x) => ID_RE.test(x)))];
  if (want.length === 0) return new Set();
  const r = await c.db.prepare(`SELECT id FROM items WHERE author_id = ?1 AND id IN (SELECT value FROM json_each(?2))`).bind(me, JSON.stringify(want)).all<{ id: string }>();
  return new Set(r.results.map((x) => x.id));
}

/** The grants a chain carries (earlier links as adaptations) plus its immediate source link. */
export function linkGrants(earlier: ChainLink[], immediate: Grant | null): Grant[] {
  return [...chainGrants(earlier), ...(immediate && immediate.item !== "" ? [immediate] : [])];
}

/**
 * Hash of the key-free facts of a question as served to non-authors: its match_hash (key-free by
 * construction in share-core), points, difficulty, topics, and the image if it is served. Never
 * the key, the explanation, a figure or an unticked image.
 */
export async function keyfreeHash(matchHash: string, served: QuestionJson, imgOf: (q: QuestionJson) => string | null): Promise<string> {
  const topics = [...(Array.isArray(served.topics) ? served.topics : [])].sort();
  const facts = JSON.stringify(["vershion/keyfree/v1", matchHash, served.points ?? null, served.difficulty ?? null, topics, imgOf(served)]);
  return fingerprint("keyfree", facts);
}

/**
 * The licence-sense hash of a question as served (round 9 J1): its canonical content (share-core's
 * output) with what is not content and what changing does not adapt it left out (ids, times,
 * origin, published, points, difficulty, topics). Computed for both forms; a hidden subject never
 * uses the full one (visible.ts).
 */
export async function nearHash(q: QuestionJson, bare = false): Promise<string> {
  const { id: _i, created: _c, modified: _m, origin: _o, published: _p, points: _pt, difficulty: _d, topics: _t, explanation, ...rest } = q as QuestionJson & Record<string, unknown>;
  const body = bare ? rest : { ...rest, explanation };
  return fingerprint("near", JSON.stringify(["vershion/near/v3", orderFree(normDeep(body))]));
}

/**
 * Round 14 (B2): the identity of a question does not depend on an order students never rely on:
 * Mc choices (except locked positions), Matching pairs and distractors, and Multipart parts are
 * compared as sorted multisets, unless the author keeps the order (`keep_order: true`), where order
 * is content. Moodle compares restored questions the same way (answers as a set).
 */
export function orderFree(v: unknown): unknown {
  if (!v || typeof v !== "object" || Array.isArray(v)) return v;
  const q = v as Record<string, any>;
  if (q.keep_order === true || !q.kind || typeof q.kind !== "object") return q;
  const key = (x: unknown) => JSON.stringify(x);
  const mc = (answers: unknown): unknown => {
    if (!Array.isArray(answers)) return answers;
    // Locked choices keep their places; the free ones are a multiset.
    const free = answers.filter((a) => !(a && (a as { locked?: unknown }).locked)).map(key).sort();
    return answers.map((a, i) => ((a as { locked?: unknown })?.locked ? { at: i, a } : null)).filter(Boolean).concat(free.map((f) => ({ free: f })) as never[]);
  };
  const kind = { ...q.kind };
  if (kind.Mc) kind.Mc = { ...kind.Mc, answers: mc(kind.Mc.answers) };
  if (kind.Matching) kind.Matching = { ...kind.Matching, pairs: [...(kind.Matching.pairs ?? [])].map(key).sort(), distractors: [...(kind.Matching.distractors ?? [])].map(key).sort() };
  if (kind.Multipart) kind.Multipart = { ...kind.Multipart, parts: [...(kind.Multipart.parts ?? [])].map((p) => key(p && p.kind && p.kind.Mc ? { ...p, kind: { Mc: { ...p.kind.Mc, answers: mc(p.kind.Mc.answers) } } } : p)).sort() };
  return { ...q, kind };
}

/**
 * Text as the licence-sense comparison sees it (round 10 M-3): compatibility forms folded, case
 * folded, punctuation and runs of whitespace (NBSP included) as one space, trimmed. So a trailing
 * period, a doubled space or a changed case keeps a copy a copy. Rewording is not caught here
 * (fuzzy stems raise origin_suspect; determined rewording defeats any exact system).
 */
export function normText(s: string): string {
  // Round 12 (share-core round 7's hash_form): invisible format characters (ZWJ, ZWNJ, zero-width
  // space, word joiner, soft hyphen, BOM: \p{Cf}) and the combining grapheme joiner go, every kind of
  // space is one space. Stricter than hash_form (which keeps ZWJ in joining scripts): this is the
  // comparison for "the same text" and for blocks, where folding more only catches more evasions.
  return s.normalize("NFKC").replace(/[\p{Cf}\u034F]/gu, "").toLowerCase().replace(/[\p{P}]+/gu, " ").replace(/[\s\p{Zs}]+/gu, " ").trim();
}
/**
 * Round 17 (critic22c M3): the KEY-FREE words a question adds to its stem as served (choices,
 * Matching columns, part stems and choices; never an answer, a key or an explanation, which a hidden-key
 * publish does not serve), with its kind as a token: a near stem with a different body is a different
 * question. Distinct, at most 64 (plus the kind).
 */
export function bodyTokens(q: QuestionJson): string[] {
  const x = q as unknown as { kind?: Record<string, any>; explanation?: unknown };
  const k = Object.keys(x.kind ?? {})[0] ?? "";
  const b = (x.kind ?? {})[k] ?? {};
  const texts: unknown[] = [];
  if (k === "Mc") texts.push(...((b.answers ?? []) as { text?: unknown }[]).map((a) => a?.text));
  // (Matching: the left column and the word bank, rights and distractors alike, as served.)
  if (k === "Matching") texts.push(...((b.pairs ?? []) as { left?: unknown; right?: unknown }[]).flatMap((p) => [p?.left, p?.right]), ...((b.distractors ?? []) as unknown[]));
  if (k === "Multipart") for (const p of (b.parts ?? []) as Record<string, any>[]) texts.push(p?.stem, ...bodyTokens({ kind: p?.kind ?? {} } as unknown as QuestionJson).slice(1));
  const words = [...new Set(texts.filter((t): t is string => typeof t === "string").flatMap((t) => normText(t).split(" ")).filter((t) => t.length > 0))].slice(0, 64);
  return [`kind:${k}`, ...words];
}
/** Bodies close enough for a near stem to be the same question (round 17 M3). */
export const BODY_JACCARD = 0.5;
export function sameBody(a: string[] | null, b: string[] | null): boolean {
  if (!a || !b || a[0] !== b[0]) return false;
  if (a.length === 1 && b.length === 1) return true; // a Tf, a bare Short: the stem is the question
  return jaccard(a, b) >= BODY_JACCARD;
}
function normDeep(v: unknown): unknown {
  if (typeof v === "string") return normText(v);
  if (Array.isArray(v)) return v.map(normDeep);
  if (v && typeof v === "object") return Object.fromEntries(Object.entries(v as Record<string, unknown>).map(([k, x]) => [k, normDeep(x)]));
  return v;
}

/**
 * The block key of a question's student-visible skeleton in the licence-sense normalisation
 * (round 12 M6): kind, stem, Mc choices, Matching columns, blank count, Multipart parts, every text
 * through normText (case, punctuation incl. quotes and dashes, whitespace). A privacy or copyright
 * block holds it alongside the exact match_hash, so a one-character edit does not evade the block.
 */
export async function normSkeletonKey(q: QuestionJson): Promise<string> {
  const x = q as unknown as { stem?: unknown; kind?: Record<string, any> };
  const k = Object.keys(x.kind ?? {})[0] ?? "";
  const body = (x.kind ?? {})[k] ?? {};
  const t = (v: unknown) => (typeof v === "string" ? normText(v) : "");
  const sk: Record<string, unknown> = { kind: k, stem: t(x.stem) };
  if (k === "Mc") sk.choices = ((body.answers ?? []) as { text?: unknown }[]).map((a) => t(a.text)).sort();
  if (k === "Matching") {
    sk.left = ((body.pairs ?? []) as { left?: unknown }[]).map((p) => t(p.left)).sort();
    sk.bank = [...new Set([...((body.pairs ?? []) as { right?: unknown }[]).map((p) => t(p.right)), ...((body.distractors ?? []) as unknown[]).map(t)].filter((s) => s !== ""))].sort();
  }
  if (k === "Blank") sk.blanks = Array.isArray(body.accept) ? body.accept.length : 0;
  if (k === "Multipart") {
    const parts = ((body.parts ?? []) as { stem?: unknown; kind?: Record<string, any> }[]).map((p) => {
      const pk = Object.keys(p.kind ?? {})[0] ?? "";
      return { stem: t(p.stem), kind: pk, ...(pk === "Mc" ? { choices: (((p.kind ?? {}).Mc?.answers ?? []) as { text?: unknown }[]).map((a) => t(a.text)).sort() } : {}) };
    });
    // Round 14 (B2): parts as a multiset unless the order is kept.
    sk.parts = (x as { keep_order?: unknown }).keep_order === true ? parts : parts.map((p) => JSON.stringify(p)).sort();
  }
  return `n:${await fingerprint("block", JSON.stringify(["vershion/block-norm/v1", sk]))}`;
}

/** The block key of a question's STEM alone, normalised (round 13): student data in the stem is held whatever the answer. */
export async function stemKey(q: QuestionJson): Promise<string> {
  const stem = (q as unknown as { stem?: unknown }).stem;
  return `s:${await fingerprint("block", `vershion/block-stem/v1\n${typeof stem === "string" ? normText(stem) : ""}`)}`;
}

/** Field block keys (round 12 M4): the normalised explanation, and the normalised model answer / accepted answers. */
export async function fieldKeys(q: QuestionJson): Promise<{ expl: string | null; ans: string | null }> {
  const x = q as unknown as { explanation?: unknown; kind?: Record<string, any> };
  const h = async (tag: string, s: string) => `${tag}:${await fingerprint("block", `vershion/block-field/v1\n${tag}\n${s}`)}`;
  const e = typeof x.explanation === "string" ? normText(x.explanation) : "";
  const k = Object.keys(x.kind ?? {})[0] ?? "";
  const body = (x.kind ?? {})[k] ?? {};
  const a = k === "Short" ? normText(String(body.sample ?? "")) : k === "Blank" && Array.isArray(body.accept) ? normText((body.accept as unknown[]).flat().map(String).join(" | ")) : "";
  return { expl: e ? await h("e", e) : null, ans: a ? await h("a", a) : null };
}

/** Round 16 (critic21c B5): the block keys of each Multipart part: its stem (`s:`, as a standalone
 *  question's stem key), explanation (`e:`) and model / accepted answers (`a:`). */
export async function partKeysOf(q: QuestionJson): Promise<{ part: number; key: string }[]> {
  const parts = ((q as unknown as { kind?: { Multipart?: { parts?: unknown[] } } }).kind?.Multipart?.parts ?? []) as Record<string, unknown>[];
  const out: { part: number; key: string }[] = [];
  for (const [k, p] of parts.entries()) {
    if (!p || typeof p !== "object") continue;
    const pq = p as unknown as QuestionJson;
    if (typeof p.stem === "string" && p.stem.trim() !== "") out.push({ part: k, key: await stemKey(pq) });
    const f = await fieldKeys(pq);
    if (f.expl) out.push({ part: k, key: f.expl });
    if (f.ans) out.push({ part: k, key: f.ans });
  }
  return out;
}

/** Round 16 (B6): the explanation and answer block keys a text WOULD have (a lifted explanation may be
 *  pasted as a model answer, and the other way round). */
export async function textFieldKeys(text: string): Promise<string[]> {
  const t = normText(text);
  if (!t) return [];
  const h = async (tag: string) => `${tag}:${await fingerprint("block", `vershion/block-field/v1\n${tag}\n${t}`)}`;
  return [await h("e"), await h("a")];
}

/**
 * Round 17 (critic22c M4): a question's key-free identity as a UNIT, the same whether it stands alone
 * or is a part of a Multipart: its kind, normalised stem and choices (as a set) or Matching columns.
 * For the question itself and each Multipart part; none for a short Tf / Short / Blank statement.
 */
export async function unitKeysOf(q: QuestionJson): Promise<{ part: number; key: string; stemOnly: boolean }[]> {
  const one = async (stem: unknown, kind: Record<string, any> | undefined): Promise<string | null> => {
    const k = Object.keys(kind ?? {})[0] ?? "";
    if (k === "" || k === "Multipart" || typeof stem !== "string") return null;
    const b = (kind ?? {})[k] ?? {};
    const st = normText(stem);
    if (["Tf", "Short", "Blank"].includes(k) && st.split(" ").filter(Boolean).length <= 8) return null;
    const set = (xs: unknown[]) => xs.filter((x): x is string => typeof x === "string").map(normText).sort();
    const body =
      k === "Mc"
        ? set(((b.answers ?? []) as { text?: unknown }[]).map((a) => a?.text))
        : k === "Matching"
          ? [set(((b.pairs ?? []) as { left?: unknown }[]).map((p) => p?.left)), set([...((b.pairs ?? []) as { right?: unknown }[]).map((p) => p?.right), ...((b.distractors ?? []) as unknown[])].filter((x) => x !== ""))]
          : [];
    return `u:${await fingerprint("block", JSON.stringify(["vershion/unit/v1", k, st, body]))}`;
  };
  const x = q as unknown as { stem?: unknown; kind?: Record<string, any> };
  // (A Tf / Short / Blank unit is its stem alone: round 18 U5, a stem-only unit, soft when read.)
  const stemOnly = (kind: Record<string, any> | undefined) => ["Tf", "Short", "Blank"].includes(Object.keys(kind ?? {})[0] ?? "");
  const out: { part: number; key: string; stemOnly: boolean }[] = [];
  const self = await one(x.stem, x.kind);
  if (self) out.push({ part: -1, key: self, stemOnly: stemOnly(x.kind) });
  for (const [k, p] of (((x.kind?.Multipart?.parts ?? []) as Record<string, any>[]).entries())) {
    const u = await one(p?.stem, p?.kind);
    if (u) out.push({ part: k, key: u, stemOnly: stemOnly(p?.kind) });
  }
  return out;
}

/** ... and its key-free part: the match_hash and the image as served. */
export async function nearKfHash(matchHash: string, served: QuestionJson, imgOf: (q: QuestionJson) => string | null): Promise<string> {
  return fingerprint("near", JSON.stringify(["vershion/nearkf/v1", matchHash, imgOf(served)]));
}

// ---- fuzzy stems (round 10 M-3) ---------------------------------------------------------------------

/** The stem's normalised words (normText), distinct, at most 64. */
export function stemTokens(stem: unknown): string[] {
  if (typeof stem !== "string") return [];
  return [...new Set(normText(stem).split(" ").filter((t) => t.length > 0))].slice(0, 64);
}

// (Round 22 B2: the round-21 FNV "keyed" sketch is gone: it was invertible and kept the word order.
// Sketches are HMAC-SHA256 under the 'sketch-v1' subkey, as sorted sets: shared.ts keyedSet.)
const fnv = (s: string, seed: number) => {
  let h = (0x811c9dc5 ^ Math.imul(seed, 0x9e3779b1)) >>> 0;
  for (let i = 0; i < s.length; i++) {
    h ^= s.charCodeAt(i);
    h = Math.imul(h, 0x01000193) >>> 0;
  }
  return h >>> 0;
};
/** MinHash over the tokens, 16 hashes in 8 bands of 2: two stems with Jaccard 0.8 share a band with probability > 0.999. */
export const STEM_BANDS = 8;
export function stemBands(tokens: string[]): string[] {
  if (tokens.length < 4) return [];
  const mins = Array.from({ length: STEM_BANDS * 2 }, (_, k) => Math.min(...tokens.map((t) => fnv(t, k + 1))));
  return Array.from({ length: STEM_BANDS }, (_, b) => `${b}:${mins[2 * b]!.toString(36)}:${mins[2 * b + 1]!.toString(36)}`);
}
export function jaccard(a: string[], b: string[]): number {
  const A = new Set(a);
  const B = new Set(b);
  let inter = 0;
  for (const t of A) if (B.has(t)) inter++;
  const uni = A.size + B.size - inter;
  return uni === 0 ? 0 : inter / uni;
}
export const FUZZY_JACCARD = 0.8;
/** Fuzzy candidates per question, pairs in all, and evidence rows examined per publish (round 11 P2). */
export const FUZZY_PER_POSITION = 8;
/** Near candidates examined per question, earliest first (round 21). */
export const FUZZY_WINDOW = 200;
/** Admissible near sources kept per question, earliest first; the rest only bound the licence (round 21). */
export const FUZZY_KEEP = 32;
/** Near sources left out of a question: their licences, the first such item and how many (round 21). */
export interface FuzzyCut {
  lics: string[];
  item: string;
  n: number;
  /** For each licence, the first item left out that carries it (named by a refusal). */
  by: Record<string, string>;
}
export const FUZZY_PAIRS_MAX = 2000;
export const FUZZY_ROWS_MAX = 20_000;
/** ... and in the public band index (all publishes), where a band is shared far more often (round 18). */
export const PUB_BAND_MAX = 256;
/** A MinHash band shared by more served questions than this is skipped (and the publish flagged). */
export const FUZZY_BAND_MAX = 32;

/**
 * Sources this account was served whose stem is a near-rewording of a position's stem (another
 * match_hash, token Jaccard >= FUZZY_JACCARD), found through the MinHash bands. Admissibility as in
 * the evidence query: never the publisher's own items or void ones, never later than the
 * publisher's own first publish, never one the publisher attests independence of. Imports become
 * ADAPTATIONS (hard), reads soft; every one raises origin_suspect.
 */
export async function fuzzyHits(
  c: Ctx,
  me: string,
  pos: { pos: number; mh: string; tokens: string[]; bands: string[]; first: number | null; nfull?: string | null; nbare?: string | null; noexpl?: boolean; body?: string[] }[],
  o: { mine: string[]; independent: string[]; exact?: number[]; reserve?: Reserver },
): Promise<{ hits: ImportHit[]; capped: boolean; cut: Map<number, FuzzyCut>; stemOnly: Set<number> }> {
  const want = pos.filter((p) => p.bands.length > 0);
  if (want.length === 0) return { hits: [], capped: false, cut: new Map(), stemOnly: new Set() };
  // The worst-case result (one row per candidate pair, its tokens and licences) is reserved first
  // (round 29: a publish lends what its heavy phase reserved and no longer uses: shared.ts lender).
  const release = (o.reserve ?? reserveBytes)(Math.min(want.length * FUZZY_PER_POSITION, FUZZY_PAIRS_MAX) * FUZZY_ROW_BYTES);
  try {
    return await fuzzyQuery(c, me, want, o);
  } finally {
    release();
  }
}
/**
 * The author's OWN earlier versions of each question, fuzzily (round 11 P1): the earliest publish
 * sequence of a question the author published themselves (any of their items, any retained
 * publish) whose stem is a near-rewording (Jaccard >= FUZZY_JACCARD) of this one. Bands of the
 * author's own stems live under user "own:<id>". Bounded like the fuzzy lookup. Used ONLY to decide
 * that a later fuzzy source (or a loose stem match) is not the origin: never for exact evidence.
 */
/**
 * Round 23 (critic30a B3): the author's own earlier version counts for a later one with the same
 * choices when the later one ADDS nothing to it: no explanation or answer the earlier one did not
 * have (removing them is her own edit; adding another's is not hers, round 12 M1).
 */
/** Round 26: the earlier of two optional "first" seqs. */
export const minFirst = (a: number | null, b: number | null) => (a === null ? b : b === null ? a : Math.min(a, b));
/** Round 26 (critic 34c): the publisher's later version has a value her earlier one did not. */
const added = (loose: string | null | undefined, later: string | null | undefined) =>
  later != null && later !== "" && (loose == null || loose === "" || loose !== later);
/**
 * Round 26 (B-2): the candidate source lacks what the publisher's version ADDED to her own earlier
 * one. Per field: a field she did not add is hers either way (her earlier version already carried
 * it), so requiring the source to differ there wrongly kept the strict first for Short/Blank, whose
 * answer or sample is usually unchanged when she adds an explanation (critic-moodle-34c).
 */
export const lacksAdded = (
  p: { expl?: string | null; ans?: string | null },
  expl: string | null,
  ans: string | null,
  loose?: { expl?: string | null; ans?: string | null } | null,
) =>
  (!added(loose?.expl, p.expl) || expl == null || expl !== p.expl) &&
  (!added(loose?.ans, p.ans) || ans == null || ans !== p.ans);
const addsNothing = (p: { expl?: string | null; ans?: string | null }, expl: string | null, ans: string | null) =>
  (p.expl == null || p.expl === expl) && (p.ans == null || p.ans === ans);
/**
 * Her durable stem records sharing a keyed band with each position (ownFuzzyFirst): one row per
 * (position, content), the first FUZZY_PAIRS_MAX in (pos, qhash, bk_expl, bk_ans) order.
 */
export const OWN_STEMS_SQL = `WITH b AS MATERIALIZED (SELECT CAST(j.value ->> '$.pos' AS INTEGER) AS pos, bv.value AS band FROM json_each(?2) j, json_each(j.value -> '$.bands') bv)
         SELECT b.pos, s.first_seq, s.tok_sk, s.body_sk, s.bk_expl, s.bk_ans, s.bk_expl_full FROM b CROSS JOIN author_stems s ON s.author_id = ?1 AND s.band = b.band
          GROUP BY b.pos, s.qhash, s.bk_expl, s.bk_ans LIMIT ?3`;
/**
 * Round 30 (staging D1 insights): the positions OWN_STEMS_SQL can answer for, so it never joins the
 * rest. Its rows are cut at FUZZY_PAIRS_MAX in position order, and a position has at least as many rows
 * as its busiest band has records (one band holds a content at most once: the primary key), so once
 * those counts add up to FUZZY_PAIRS_MAX no later position can reach the result. Without this, a bank of
 * like stems ("Match each term...") republished joined every position with every record of her common
 * bands: 5.7 million rows and 9.8 s for 990 questions on workerd's SQLite, growing with the square of the
 * bank (now 41,000 rows, both queries). The result is the same
 * (round30.test.ts compares it with the full join): the kept positions' rows are joined in the same
 * order. Each band's count stops at FUZZY_PAIRS_MAX, so this reads at most that many rows per band.
 */
export async function ownStemsWindow<T extends { pos: number; bands: string[] }>(c: Ctx, me: string, kb: T[]): Promise<T[]> {
  const bands = [...new Set(kb.flatMap((x) => x.bands))];
  if (bands.length === 0) return kb;
  const r = await c.db
    .prepare(`SELECT j.value AS band, (SELECT COUNT(*) FROM (SELECT 1 FROM author_stems s WHERE s.author_id = ?1 AND s.band = j.value LIMIT ?3)) AS n FROM json_each(?2) j`)
    .bind(me, JSON.stringify(bands), FUZZY_PAIRS_MAX)
    .all<{ band: string; n: number }>();
  const n = new Map(r.results.map((x) => [x.band, x.n]));
  const least = new Map<number, number>();
  for (const x of kb) least.set(x.pos, Math.max(least.get(x.pos) ?? 0, ...x.bands.map((b) => n.get(b) ?? 0)));
  let total = 0;
  for (const [pos, m] of [...least].sort((a, b) => a[0] - b[0])) {
    total += m;
    if (total >= FUZZY_PAIRS_MAX) return kb.filter((y) => y.pos <= pos);
  }
  return kb;
}
export async function ownFuzzyFirst(
  c: Ctx,
  me: string,
  pos: { pos: number; tokens: string[]; bands: string[]; body: string; bodyToks?: string[]; expl?: string | null; ans?: string | null }[],
  cut?: Set<number>,
  // Round 26 (critic-moodle-33c B-2): her earliest own version with the same choices EVEN IF the
  // later one adds an explanation or answer to it. Used only against a candidate source that lacks
  // what she added (her earlier version predates that source just as much); never for binding.
  loose?: Map<number, { seq: number; expl: string | null; ans: string | null }>,
  reserve: Reserver = reserveBytes,
): Promise<Map<number, number>> {
  const out = new Map<number, number>();
  const want = pos.filter((p) => p.bands.length > 0);
  if (want.length === 0) return out;
  const release = reserve(Math.min(want.length * FUZZY_PER_POSITION, FUZZY_PAIRS_MAX) * FUZZY_ROW_BYTES);
  try {
    const r = await c.db
      .prepare(
        `WITH b AS MATERIALIZED (SELECT CAST(j.value ->> '$.pos' AS INTEGER) AS pos, bv.value AS band FROM json_each(?2) j, json_each(j.value -> '$.bands') bv),
              bc AS MATERIALIZED (SELECT DISTINCT band, (SELECT COUNT(*) FROM (SELECT 1 FROM stem_bands x WHERE x.user_id = ?1 AND x.band = b.band LIMIT ?5 + 1)) AS n FROM b),
              cand0 AS (SELECT b.pos, sb.item_id, sb.qid, COUNT(*) AS shared FROM b CROSS JOIN bc ON bc.band = b.band AND bc.n <= ?5 CROSS JOIN stem_bands sb ON sb.user_id = ?1 AND sb.band = b.band GROUP BY b.pos, sb.item_id, sb.qid),
              ranked AS MATERIALIZED (SELECT cand0.*, ROW_NUMBER() OVER (PARTITION BY pos ORDER BY shared DESC, item_id, qid) AS rn FROM cand0),
              cand AS MATERIALIZED (SELECT * FROM ranked WHERE rn <= ?3 LIMIT ?4)
         SELECT c.pos, iq.seq, iq.stem_tokens, iq.body_hash, iq.body_tokens, iq.bk_expl, iq.bk_ans FROM cand c CROSS JOIN item_publishes p ON p.item_id = c.item_id AND p.status IN ('current', 'previous')
           CROSS JOIN item_questions iq ON iq.publish_id = p.publish_id AND iq.qid = c.qid
          WHERE p.author_id = ?6
         -- (round 21: a cut is counted: the caller flags it; only the author's OWN items are looked at, so a
         -- cut can only keep a source binding, never free one)
         UNION ALL SELECT pos, NULL, NULL, NULL, NULL, NULL, NULL FROM ranked WHERE rn > ?3 GROUP BY pos`,
      )
      .bind(`own:${me}`, JSON.stringify(want.map((p) => ({ pos: p.pos, bands: p.bands }))), FUZZY_PER_POSITION, FUZZY_PAIRS_MAX, FUZZY_BAND_MAX, me)
      .all<{ pos: number; seq: number | null; stem_tokens: string | null; body_hash: string | null; body_tokens: string | null; bk_expl: string | null; bk_ans: string | null }>();
    const byPos = new Map(want.map((p) => [p.pos, p]));
    for (const x of r.results) {
      if (x.seq === null) {
        cut?.add(x.pos);
        continue;
      }
      // Round 12 M1: the author's own earlier question must have the same body (everything but the
      // stem): a reworded stem of their own question, never a bare stem they planted. Round 23
      // (critic30a B3): "the same body" as every other near path judges it (the kind and its choices,
      // sameBody), so dropping or adding an explanation never makes her own earlier version "another".
      const p0 = byPos.get(x.pos)!;
      const sameChoices = !!p0.bodyToks && sameBody(p0.bodyToks, x.body_tokens ? (JSON.parse(x.body_tokens) as string[]) : null);
      const same = (x.body_hash !== null && x.body_hash === p0.body) || (sameChoices && addsNothing(p0, x.bk_expl, x.bk_ans));
      if (!same && !sameChoices) continue;
      if (!(x.seq > 0) || jaccard(byPos.get(x.pos)!.tokens, JSON.parse(x.stem_tokens ?? "[]") as string[]) < FUZZY_JACCARD) continue;
      // (round 26, critic 34c: keep the loose version's own fields: what she ADDED is judged per field)
      {
        const cur = loose?.get(x.pos);
        if (loose && (cur === undefined || x.seq < cur.seq)) loose.set(x.pos, { seq: x.seq, expl: x.bk_expl ?? null, ans: x.bk_ans ?? null });
      }
      if (!same) continue;
      out.set(x.pos, Math.min(out.get(x.pos) ?? Infinity, x.seq));
    }
    // Round 23 (critic30a B3): ... and her DURABLE keyed record of every stem she published (any item,
    // retained or not: deleted, purged and re-added material stays hers), by keyed bands, keyed sets.
    const memo = new Map<string, string>();
    // Round 24 (critic31a/31b): ALWAYS consult the durable record and keep the EARLIEST of the two.
    // A newer retained version matching the subject must not hide her older record: her first
    // publication of the material is what makes a later copier "not earlier". (Before, the durable
    // lookup was skipped whenever a retained version had already answered, so after a copier edited
    // first and she reverted or published the same content in a second bank, the copier became the
    // "earlier publisher" and her own downstream copies bound to the copier's link.)
    const rest = want.filter((p) => p.bodyToks);
    const kb = await Promise.all(rest.map(async (p) => ({ pos: p.pos, bands: await Promise.all(p.bands.map((b) => keyedOne("b:", b, memo))) })));
    const d = rest.length === 0 ? { results: [] as { pos: number; first_seq: number; tok_sk: string; body_sk: string | null; bk_expl: string | null; bk_ans: string | null; bk_expl_full: string | null }[] } : await c.db
      .prepare(OWN_STEMS_SQL)
      .bind(me, JSON.stringify(await ownStemsWindow(c, me, kb)), FUZZY_PAIRS_MAX)
      .all<{ pos: number; first_seq: number; tok_sk: string; body_sk: string | null; bk_expl: string | null; bk_ans: string | null; bk_expl_full: string | null }>();
    for (const x of d.results) {
      const p0 = byPos.get(x.pos)!;
      // Round 24 (critic32a B-1): a hidden publish's served form has no explanation; compare the later
      // subject with the FULL form's explanation key when the record has it. (The answer is always the
      // served one: a hidden key must never change an observable outcome.)
      const expl = x.bk_expl_full || x.bk_expl;
      if (!p0.bodyToks || !(x.first_seq > 0)) continue;
      if (jaccard(await keyedSet(p0.tokens, memo), JSON.parse(x.tok_sk) as string[]) < FUZZY_JACCARD) continue;
      if (!sameBody(await keyedBody(p0.bodyToks, memo), x.body_sk ? (JSON.parse(x.body_sk) as string[]) : null)) continue;
      {
        const cur = loose?.get(x.pos);
        if (loose && (cur === undefined || x.first_seq < cur.seq)) loose.set(x.pos, { seq: x.first_seq, expl: expl ?? null, ans: x.bk_ans ?? null });
      }
      if (!addsNothing(p0, expl || null, x.bk_ans || null)) continue;
      out.set(x.pos, Math.min(out.get(x.pos) ?? Infinity, x.first_seq));
    }
    // Round 25 (critic-moodle-24): the durable lookup's own allowance is counted, not silent: past it
    // the caller flags each position (a cut here can only keep a source binding, never free one).
    if (d.results.length >= FUZZY_PAIRS_MAX) for (const p of rest) cut?.add(p.pos);
  } finally {
    release();
  }
  return out;
}

/**
 * Round 16 (critic21c B2): the earliest visible question by ANOTHER author whose served stem is a
 * near-rewording (token Jaccard >= FUZZY_JACCARD) of each position's, of the same kind, first
 * published before `first`. Bands of every publish's served stems live under user "pub"; a band
 * shared by more than FUZZY_BAND_MAX questions is skipped (common stems). Public data only.
 */
export async function publicNearEarlier(
  c: Ctx,
  me: string,
  pos: { pos: number; tokens: string[]; bands: string[]; qkind: string; first: number | null; body?: string[]; looseFirst?: number | null; looseExpl?: string | null; looseAns?: string | null; expl?: string | null; ans?: string | null }[],
  reserve: Reserver = reserveBytes,
): Promise<{ near: Map<number, { item_id: string; qid: string; version: number; license: string; author_name: string }>; cut: Set<number> }> {
  const out = new Map<number, { item_id: string; qid: string; version: number; license: string; author_name: string; seq: number }>();
  const cut = new Set<number>();
  const want = pos.filter((p) => p.bands.length > 0);
  if (want.length === 0) return { near: out, cut };
  const release = reserve(Math.min(want.length * FUZZY_PER_POSITION, FUZZY_PAIRS_MAX) * FUZZY_ROW_BYTES);
  try {
    const r = await c.db
      .prepare(
        // Round 21 (critic27b B-4): the admissible candidates (visible, someone else's, the same kind,
        // published before the author's first) EARLIEST first, before any cut: a later copy never takes
        // an earlier item's place; a cut is counted and flagged, never silent.
        `WITH b AS MATERIALIZED (SELECT CAST(j.value ->> '$.pos' AS INTEGER) AS pos, j.value ->> '$.qkind' AS qkind, j.value ->> '$.first' AS first, bv.value AS band
                                  FROM json_each(?2) j, json_each(j.value -> '$.bands') bv),
              bc AS MATERIALIZED (SELECT DISTINCT band, (SELECT COUNT(*) FROM (SELECT 1 FROM stem_bands x WHERE x.user_id = 'pub' AND x.band = b.band LIMIT ?5 + 1)) AS n FROM b),
              cand0 AS MATERIALIZED (SELECT b.pos, MIN(b.qkind) AS qkind, MIN(b.first) AS first, sb.item_id, sb.qid FROM b CROSS JOIN bc ON bc.band = b.band AND bc.n <= ?5
                                       CROSS JOIN stem_bands sb ON sb.user_id = 'pub' AND sb.band = b.band GROUP BY b.pos, sb.item_id, sb.qid),
              adm AS MATERIALIZED (SELECT c0.pos, i.id AS item_id, iq.qid, i.public_version AS version, i.license, COALESCE(u.display_name, 'deleted teacher') AS author_name,
                                          iq.stem_tokens, iq.qkind, iq.seq, iq.body_tokens, iq.bk_expl, iq.bk_ans
                                     FROM cand0 c0 CROSS JOIN items i ON i.id = c0.item_id JOIN users u ON u.id = i.author_id
                                     CROSS JOIN item_questions iq ON iq.publish_id = i.public_publish_id AND iq.qid = c0.qid
                                    WHERE i.author_id <> ?1 AND ${VISIBLE_TO_OTHERS} AND iq.qkind = c0.qkind AND (c0.first IS NULL OR iq.seq < CAST(c0.first AS INTEGER))
                                      -- (round 23, critic30a B3: never a copy of the publisher's OWN earlier question:
                                      -- its content was hers first, or its chain runs to one of her items)
                                      AND NOT EXISTS (SELECT 1 FROM author_firsts af WHERE af.author_id = ?1 AND af.match_hash = iq.match_hash AND af.first_seq <= iq.seq)
                                      AND NOT EXISTS (SELECT 1 FROM json_each(COALESCE(iq.chain_json, '[]')) cl JOIN items mi ON mi.id = cl.value ->> '$.item_id' WHERE mi.author_id = ?1)),
              ranked AS MATERIALIZED (SELECT adm.*, ROW_NUMBER() OVER (PARTITION BY pos ORDER BY seq, item_id, qid) AS rn FROM adm)
         SELECT * FROM ranked WHERE rn <= ?3
         UNION ALL SELECT pos, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, COUNT(*) FROM ranked WHERE rn > ?3 GROUP BY pos`,
      )
      // (round 26 B-2: the SQL window is the looser of the two "firsts"; each candidate is judged below)
      .bind(me, JSON.stringify(want.map((p) => ({ pos: p.pos, bands: p.bands, qkind: p.qkind, first: minFirst(p.first, p.looseFirst ?? null) }))), FUZZY_WINDOW, FUZZY_PAIRS_MAX, PUB_BAND_MAX)
      .all<{ pos: number; item_id: string | null; qid: string; version: number; license: string; author_name: string; stem_tokens: string | null; qkind: string; seq: number; body_tokens: string | null; bk_expl: string | null; bk_ans: string | null }>();
    const byPos = new Map(want.map((p) => [p.pos, p]));
    for (const x of r.results) {
      if (x.item_id === null) {
        cut.add(x.pos);
        continue;
      }
      const p = byPos.get(x.pos)!;
      // Round 26 (critic-moodle-33c B-2): a source that lacks what her current version ADDED (her
      // explanation or answer) is judged against her earliest own version without it.
      const first = lacksAdded(p, x.bk_expl, x.bk_ans, { expl: p.looseExpl ?? null, ans: p.looseAns ?? null })
        ? minFirst(p.first, p.looseFirst ?? null)
        : p.first;
      if (x.qkind !== p.qkind || (first !== null && !(x.seq < first))) continue;
      if (jaccard(p.tokens, JSON.parse(x.stem_tokens ?? "[]") as string[]) < FUZZY_JACCARD) continue;
      // Round 17 (M3): the same body too (a near stem alone is another question).
      if (!sameBody(p.body ?? null, x.body_tokens ? (JSON.parse(x.body_tokens) as string[]) : null)) continue;
      const prev = out.get(x.pos);
      if (!prev || x.seq < prev.seq || (x.seq === prev.seq && x.item_id < prev.item_id)) out.set(x.pos, { item_id: x.item_id, qid: x.qid, version: x.version, license: x.license, author_name: x.author_name, seq: x.seq });
    }
  } finally {
    release();
  }
  return { near: out, cut };
}

/** Bytes one aggregated fuzzy row may take (at most 64 stem tokens, licences, chain). */
export const FUZZY_ROW_BYTES = 8 * 1024;
/** Reserves a worst case against the isolate budget and returns its release (reserveBytes, or a publish's lender). */
export type Reserver = (n: number) => () => void;

async function fuzzyQuery(
  c: Ctx,
  me: string,
  want: { pos: number; mh: string; tokens: string[]; bands: string[]; first: number | null; nfull?: string | null; nbare?: string | null; noexpl?: boolean; body?: string[] }[],
  o: { mine: string[]; independent: string[]; exact?: number[] },
): Promise<{ hits: ImportHit[]; capped: boolean; cut: Map<number, FuzzyCut>; stemOnly: Set<number> }> {
  // Round 11 P2: everything is bounded in SQL. At most FUZZY_PER_POSITION candidate (item, qid)
  // pairs per question (those sharing the most bands) and FUZZY_PAIRS_MAX in all; at most
  // FUZZY_ROWS_MAX evidence rows examined (one per served content), aggregated per pair, so a source
  // republished many times cannot multiply what reaches the Worker. Past a cap: flagged, conservative.
  // (Round 22 B2: each band with its keyed form, which is what a ghost is found by.)
  const skMemo = new Map<string, string>();
  const kb = await Promise.all(want.map((p) => Promise.all(p.bands.map(async (b) => [b, await keyedOne("b:", b, skMemo)]))));
  const r = await c.db
    .prepare(
      `WITH b AS MATERIALIZED (SELECT CAST(j.value ->> '$.pos' AS INTEGER) AS pos, j.value ->> '$.mh' AS mh, j.value ->> '$.nfull' AS nfull, j.value ->> '$.nbare' AS nbare,
                         COALESCE(j.value ->> '$.noexpl', 0) AS noexpl, j.value ->> '$.first' AS first, bv.value ->> '$[0]' AS band, bv.value ->> '$[1]' AS kband
                         FROM json_each(?2) j, json_each(j.value -> '$.bands') bv),
            -- A band shared by more than FUZZY_BAND_MAX served questions (a stock phrase) is not
            -- discriminating: skipped, and the publish is flagged (conservative), never enumerated.
            -- Round 18 (critic23d V4): reads no longer write the reader's own band rows; candidates come
            -- from the public band index, limited to (item, qid) pairs this account has evidence of.
            -- (round 22 B2: live questions by their public bands, ghosts by their KEYED bands only)
            bc AS MATERIALIZED (SELECT DISTINCT band, (SELECT COUNT(*) FROM (SELECT 1 FROM stem_bands x WHERE x.user_id = 'pub' AND x.band = b.band
                                                                              UNION ALL SELECT 1 FROM ghost_bands gx WHERE gx.band = b.kband LIMIT ?7 + 1)) AS n FROM b),
            bsrc AS MATERIALIZED (SELECT b.*, sb0.item_id, sb0.qid FROM b CROSS JOIN bc ON bc.band = b.band AND bc.n <= ?7 CROSS JOIN stem_bands sb0 ON sb0.user_id = 'pub' AND sb0.band = b.band
                                  UNION SELECT b.*, gb.item_id, gb.qid FROM b CROSS JOIN bc ON bc.band = b.band AND bc.n <= ?7 CROSS JOIN ghost_bands gb ON gb.band = b.kband),
            cand1 AS MATERIALIZED (SELECT sb.pos, MIN(sb.mh) AS mh, MIN(sb.nfull) AS nfull, MIN(sb.nbare) AS nbare, MIN(sb.noexpl) AS noexpl, MIN(sb.first) AS first, sb.item_id, sb.qid, COUNT(DISTINCT sb.band) AS shared,
                               -- (an IMPORTED candidate binds, a read one is soft: imports first)
                               (SELECT MAX(si.kind = 'import') FROM served si WHERE si.reader_id = ?1 AND si.item_id = sb.item_id AND si.void_at IS NULL) AS imp
                        FROM bsrc sb
                       -- (Rounds 20-21: the admissibility of the rows below, applied before anything is counted or
                       -- cut: never the publisher's own items, items derived from them, or void ones.)
                       WHERE sb.item_id NOT IN (SELECT value FROM json_each(?3)) AND NOT EXISTS (SELECT 1 FROM void_items v WHERE v.item_id = sb.item_id)
                         AND NOT EXISTS (SELECT 1 FROM items oi WHERE oi.id = sb.item_id AND oi.author_id = ?1)
                         -- (round 21 M-B1: an item the publisher attests independence of is no near candidate)
                         AND sb.item_id NOT IN (SELECT value FROM json_each(?9))
                       GROUP BY sb.pos, sb.item_id, sb.qid),
            -- ... and the earliest served version with another match_hash, published before the publisher's own first.
            cand0 AS MATERIALIZED (SELECT c1.*, COALESCE(
                                     (SELECT MIN(cq.seq) FROM served sv CROSS JOIN item_questions cq ON cq.publish_id = sv.publish_id AND cq.qid = c1.qid
                                       WHERE sv.reader_id = ?1 AND sv.item_id = c1.item_id AND sv.void_at IS NULL AND cq.match_hash <> c1.mh
                                         AND (c1.first IS NULL OR cq.seq < CAST(c1.first AS INTEGER))),
                                     -- (round 21 V3: or a ghost of a served publish, through its sketch)
                                     (SELECT MIN(gq.seq) FROM served sv CROSS JOIN ghost_questions gq ON gq.publish_id = sv.publish_id AND gq.qid = c1.qid
                                       WHERE sv.reader_id = ?1 AND sv.item_id = c1.item_id AND sv.void_at IS NULL AND gq.match_hash <> c1.mh AND gq.tok_sk IS NOT NULL
                                         AND (c1.first IS NULL OR gq.seq < CAST(c1.first AS INTEGER)))) AS fseq FROM cand1 c1),
            -- Round 21 (critic27a M-B1, critic27b B-2): EARLIEST first (imports first), FUZZY_WINDOW per
            -- question and FUZZY_PAIRS_MAX in all are examined; what is left out is never skipped: its
            -- licences (served and chain) come back, and the publish is refused only if one forbids the
            -- licence chosen (publish.ts).
            ranked AS MATERIALIZED (SELECT * FROM (SELECT cand0.*, ROW_NUMBER() OVER (PARTITION BY pos ORDER BY imp DESC, fseq, item_id, qid) AS rn FROM cand0 WHERE fseq IS NOT NULL)),
            -- (round 21: a question with a source matched on itself comes last and its cut never counts:
            -- fuzzy sources are dropped beside an exact one; only a flag can follow from them)
            ranked2 AS MATERIALIZED (SELECT ranked.*, ROW_NUMBER() OVER (ORDER BY pos IN (SELECT value FROM json_each(?8)), imp DESC, pos, rn) AS gr FROM ranked WHERE rn <= ?4),
            cand AS MATERIALIZED (SELECT * FROM ranked2 WHERE gr <= ?5),
            cutc AS MATERIALIZED (SELECT pos, item_id, qid FROM (SELECT pos, item_id, qid FROM ranked WHERE imp = 1 AND rn > ?4 UNION ALL SELECT pos, item_id, qid FROM ranked2 WHERE imp = 1 AND gr > ?5)
                                   WHERE pos NOT IN (SELECT value FROM json_each(?8))),
            -- (the window's licences too: used only if the row allowance below is ever reached)
            wl AS MATERIALIZED (SELECT pos, json_group_array(v) AS lics, json_group_object(v, item) AS byl, MIN(item) AS item, MAX(n0) AS n FROM (
                                    SELECT cw.pos, l.value AS v, MIN(cw.item_id) AS item, (SELECT COUNT(DISTINCT z.item_id) FROM cand z WHERE z.pos = cw.pos AND z.imp = 1) AS n0 FROM cand cw
                                    CROSS JOIN served sv ON sv.reader_id = ?1 AND sv.item_id = cw.item_id AND sv.void_at IS NULL,
                                    json_each(sv.licenses_json) l WHERE cw.imp = 1 GROUP BY cw.pos, l.value) GROUP BY pos),
            cutl AS MATERIALIZED (SELECT pos, json_group_array(v) AS lics, json_group_object(v, item) AS byl, MIN(item) AS item, MAX(n0) AS n FROM (
                                    SELECT cc.pos, l.value AS v, MIN(cc.item_id) AS item, (SELECT COUNT(DISTINCT z.item_id) FROM cutc z WHERE z.pos = cc.pos) AS n0 FROM cutc cc
                                    CROSS JOIN served sv ON sv.reader_id = ?1 AND sv.item_id = cc.item_id AND sv.void_at IS NULL,
                                    json_each(sv.licenses_json) l GROUP BY cc.pos, l.value) GROUP BY pos),
            rows AS MATERIALIZED (SELECT * FROM (${["import_questions_live", "import_questions_ghost"]
              .map(
                (v) => `
              SELECT c.pos, iq.item_id, iq.qid, iq.match_hash, iq.content_hash, iq.kind, iq.author_name, iq.license, iq.licenses_json, iq.version, iq.versions_json,
                     iq.source_first_seq, iq.chain_json, iq.stem_tokens, iq.stem_hash, iq.at, iq.body_tokens, iq.tok_sk, iq.body_sk,
                     (c.nfull IS NOT NULL AND (iq.near_hash = c.nfull OR (c.noexpl = 1 AND iq.near_bare = c.nbare))) AS nr
                FROM cand c CROSS JOIN served sv ON sv.reader_id = ?1 AND sv.item_id = c.item_id
                CROSS JOIN ${v} iq ON iq.src_publish_id = sv.publish_id AND iq.user_id = ?1 AND iq.qid = c.qid
               WHERE iq.match_hash <> c.mh AND iq.void_at IS NULL AND iq.author_id <> ?1 AND (c.first IS NULL OR iq.source_first_seq < CAST(c.first AS INTEGER))
                 AND iq.item_id NOT IN (SELECT value FROM json_each(?3)) AND NOT EXISTS (SELECT 1 FROM void_items v WHERE v.item_id = iq.item_id)`,
              )
              .join("\n              UNION ALL")}) LIMIT ?6),
            lic AS MATERIALIZED (SELECT r.pos, r.item_id, r.qid, json_group_array(DISTINCT l.value) AS lics, json_group_array(DISTINCT CASE WHEN r.nr THEN l.value END) AS nlics
                      FROM rows r, json_each(r.licenses_json) l GROUP BY r.pos, r.item_id, r.qid),
            stats AS MATERIALIZED (SELECT (SELECT COUNT(*) FROM rows) AS examined, (SELECT COUNT(*) FROM cand) AS pairs, (SELECT COUNT(*) FROM bc WHERE n > ?7) AS hot,
                                          (SELECT json_group_array(json_object('pos', pos, 'lics', json(lics), 'item', item, 'n', n, 'by', json(byl))) FROM cutl) AS cut,
                                          (SELECT json_group_array(json_object('pos', pos, 'lics', json(lics), 'item', item, 'n', n, 'by', json(byl))) FROM wl) AS win,
                                          (SELECT json_group_array(DISTINCT pos) FROM cand WHERE imp = 1) AS cand_pos)
       SELECT g.*, lic.lics, lic.nlics, stats.examined, stats.pairs, stats.hot, stats.cut, stats.win, stats.cand_pos FROM stats LEFT JOIN (
         SELECT pos, item_id, qid, MIN(match_hash) AS match_hash, MAX(kind = 'import') AS imported, MIN(source_first_seq) AS fs, MAX(printf('%016d|%s', at, author_name)) AS an,
                MAX(printf('%016d|%s', at, license)) AS ll, MAX(version) AS version, MAX(nr) AS near, MAX(printf('%016d|', at) || chain_json) AS chain, MAX(stem_tokens) AS toks, MAX(body_tokens) AS btoks, MAX(tok_sk) AS tsk, MAX(body_sk) AS bsk,
                MAX(stem_hash) AS sh, json_group_array(DISTINCT content_hash) AS contents, json_group_array(DISTINCT version) AS versions
           FROM rows GROUP BY pos, item_id, qid) g ON 1
         LEFT JOIN lic ON lic.pos = g.pos AND lic.item_id = g.item_id AND lic.qid = g.qid`,
    )
    .bind(
      me,
      JSON.stringify(want.map((p, wi) => ({ pos: p.pos, mh: p.mh, bands: kb[wi], nfull: p.nfull ?? null, nbare: p.nbare ?? null, noexpl: p.noexpl ? 1 : 0, first: p.first }))),
      JSON.stringify(o.mine),
      FUZZY_WINDOW,
      FUZZY_PAIRS_MAX,
      FUZZY_ROWS_MAX,
      PUB_BAND_MAX,
      JSON.stringify(o.exact ?? []),
      JSON.stringify(o.independent),
    )
    .all<Record<string, any>>();
  // No row at all still says whether a cap or a hot band applied (the stats row is joined to each).
  const capped = r.results.some((x) => x.examined >= FUZZY_ROWS_MAX || x.pairs >= FUZZY_PAIRS_MAX || x.hot > 0);
  // Round 21 (critic27a M-B1, critic27b B-2): imported near candidates left out (past the window or the
  // pair allowance, and every candidate if the row allowance was reached): their licences, per question.
  const cut = new Map<number, FuzzyCut>();
  const addCut = (pos: number, lics: string[], item: string, n: number, by: Record<string, string> = {}) => {
    const prev = cut.get(pos);
    cut.set(pos, prev ? { lics: [...new Set([...prev.lics, ...lics])], item: prev.item, n: prev.n + n, by: { ...by, ...prev.by } } : { lics: [...new Set(lics)], item, n, by });
  };
  const st0 = r.results[0];
  if (st0) {
    for (const x of JSON.parse(st0.cut ?? "[]") as { pos: number; lics: string[]; item: string; n: number; by: Record<string, string> }[]) addCut(x.pos, x.lics, x.item, x.n, x.by);
    if (st0.examined >= FUZZY_ROWS_MAX) for (const x of JSON.parse(st0.win ?? "[]") as { pos: number; lics: string[]; item: string; n: number; by: Record<string, string> }[]) if (!(o.exact ?? []).includes(x.pos)) addCut(x.pos, x.lics, x.item, x.n, x.by);
  }
  const byPos = new Map(want.map((p) => [p.pos, p]));
  const mine = new Set(o.mine);
  const indep = new Set(o.independent);
  const out = new Map<string, ImportHit>();
  const strs = (v: unknown) => (JSON.parse((v as string) ?? "[]") as unknown[]).filter((l): l is string => typeof l === "string");
  const stemOnly = new Set<number>();
  for (const x of r.results) {
    if (x.pos === null) continue; // the stats row alone
    const p = byPos.get(x.pos)!;
    // (Rounds 21-22 V3/B2: a ghost has no words, only its keyed sketch: compared keyed set to keyed set.)
    const viaSketch = !x.toks && x.tsk;
    const mineToks = viaSketch ? await keyedSet(p.tokens, skMemo) : p.tokens;
    if (jaccard(mineToks, JSON.parse((viaSketch ? x.tsk : x.toks) ?? "[]") as string[]) < FUZZY_JACCARD) continue;
    // Round 17 (critic22c M3): a near STEM with another body (choices, answers, kind) is another
    // question: an admin flag at most (origin_suspect), never a credit, a binding or a label.
    const theirBody = viaSketch ? (x.bsk ? (JSON.parse(x.bsk) as string[]) : null) : x.btoks ? (JSON.parse(x.btoks) as string[]) : null;
    if (!sameBody(viaSketch && p.body ? await keyedBody(p.body, skMemo) : (p.body ?? null), theirBody)) {
      stemOnly.add(x.pos);
      continue;
    }
    // Independence never covers a copy in the licence sense (round 14: order-free, e.g. parts reordered).
    if (indep.has(x.item_id) && x.near !== 1) continue;
    const chain = JSON.parse(String(x.chain).slice(17) || "[]") as ChainLink[];
    if (chain.some((l) => mine.has(l.item_id))) continue;
    const key = `${x.pos}/${x.item_id}`;
    const lics = strs(x.lics);
    const near = x.near === 1;
    const prev = out.get(key);
    if (prev) {
      prev.near ||= near;
      prev.lic_near = [...new Set([...prev.lic_near, ...strs(x.nlics)])];
      prev.lic_all = [...new Set([...prev.lic_all, ...lics])];
      prev.lic_groups.push(lics);
      prev.contents.push(...strs(x.contents));
      if (x.imported) prev.kind = "import";
      prev.source_first_seq = Math.min(prev.source_first_seq, x.fs ?? 0);
      continue;
    }
    out.set(key, {
      pos: x.pos,
      match_hash: x.match_hash,
      qid: x.qid,
      item_id: x.item_id,
      chain_len: chain.length,
      chain_lics: [...new Set(chain.flatMap((l) => [...(l.license ? [l.license] : []), ...(l.collapsed_licenses ?? [])]))],
      author_name: String(x.an).slice(17),
      license: String(x.ll).slice(17),
      version: x.version,
      versions: (JSON.parse(x.versions ?? "[]") as number[]).filter((v) => typeof v === "number"),
      source_first_seq: x.fs ?? 0,
      stem_hash: x.sh,
      kind: x.imported ? "import" : "view",
      same: false,
      same_full: false,
      same_imported: false,
      near,
      kf_same: false,
      lic_all: lics,
      lic_same: [],
      lic_near: near ? strs(x.nlics) : [],
      lic_groups: [lics],
      contents: strs(x.contents),
      by_ref: false,
      by_match: true,
      fuzzy: true,
    });
  }
  // ... and past FUZZY_KEEP ADMISSIBLE sources per question (earliest kept): the rest, likewise.
  const byQ = new Map<number, ImportHit[]>();
  const exact = new Set(o.exact ?? []);
  for (const h of out.values()) byQ.set(h.pos, [...(byQ.get(h.pos) ?? []), h]);
  const kept: ImportHit[] = [];
  for (const [pos, hs] of byQ) {
    hs.sort((a, b) => a.source_first_seq - b.source_first_seq || (a.item_id < b.item_id ? -1 : 1));
    kept.push(...hs.slice(0, FUZZY_KEEP));
    const rest = hs.slice(FUZZY_KEEP).filter((h) => h.kind === "import");
    if (rest.length > 0 && !exact.has(pos)) {
      const by: Record<string, string> = {};
      for (const h of rest) for (const l of [...h.lic_all, ...h.chain_lics]) by[l] ??= h.item_id;
      addCut(pos, Object.keys(by), rest[0]!.item_id, rest.length, by);
    }
  }
  return { hits: kept, capped, cut, stemOnly };
}
