// Statements as they were before a rewrite, kept as test oracles (not a test file: `npm test` runs
// test/pool/*.test.ts only).
import { VISIBLE_TO_OTHERS } from "../../src/pool/shared.ts";

/** The earlier-publication lookup's own test of a candidate (unchanged since round 26). */
const EARLIER_TEST = `((p.hide_key = 0 AND (iq.near_full IN (SELECT value FROM json_each(j.value -> '$.n')) OR iq.near_bare_full IN (SELECT value FROM json_each(j.value -> '$.n'))))
                           OR iq.near_stripped IN (SELECT value FROM json_each(j.value -> '$.n')) OR iq.near_bare_stripped IN (SELECT value FROM json_each(j.value -> '$.n'))
                           OR iq.near_kf IN (SELECT value FROM json_each(j.value -> '$.n')))
                      AND i.author_id <> ?1 AND ${VISIBLE_TO_OTHERS}
                      AND COALESCE(iq.seq < CAST(CASE
                             WHEN ((j.value ->> '$.expl') IS NULL OR (j.value ->> '$.expl') = ''
                                    OR ((j.value ->> '$.lexpl') IS NOT NULL AND (j.value ->> '$.lexpl') <> '' AND (j.value ->> '$.lexpl') IS (j.value ->> '$.expl'))
                                    OR iq.bk_expl IS NULL OR iq.bk_expl IS NOT (j.value ->> '$.expl'))
                               AND ((j.value ->> '$.ans') IS NULL OR (j.value ->> '$.ans') = ''
                                    OR ((j.value ->> '$.lans') IS NOT NULL AND (j.value ->> '$.lans') <> '' AND (j.value ->> '$.lans') IS (j.value ->> '$.ans'))
                                    OR iq.bk_ans IS NULL OR iq.bk_ans IS NOT (j.value ->> '$.ans'))
                             THEN COALESCE(j.value ->> '$.loose', j.value ->> '$.first') ELSE j.value ->> '$.first' END AS INTEGER), 1)`;
const earlier = (candidates: string, order: string) => `SELECT CAST(j.value ->> '$.i' AS INTEGER) AS pos,
                  (SELECT json_object('item_id', i.id, 'qid', iq.qid, 'version', i.public_version, 'license', i.license, 'author_name', COALESCE(u.display_name, 'deleted teacher'))
                     FROM item_questions iq JOIN item_publishes p ON p.publish_id = iq.publish_id JOIN items i ON i.public_publish_id = iq.publish_id JOIN users u ON u.id = i.author_id
                    WHERE ${candidates}
                      AND ${EARLIER_TEST}
                    ORDER BY ${order} LIMIT 1) AS m
             FROM json_each(?2) j`;
const OR5 = `x.near_full = n.value OR x.near_stripped = n.value OR x.near_kf = n.value OR x.near_bare_full = n.value OR x.near_bare_stripped = n.value`;
/** Rounds 21-29: every question of every publish holding a match was a candidate; ties in that scan's order. */
export const EARLIER_SQL_R29 = earlier(`iq.publish_id IN (SELECT x.publish_id FROM json_each(j.value -> '$.n') n, item_questions x WHERE ${OR5})`, `iq.seq`);
/** Round 30: the matching questions, by the five near columns at once (each position walked all of them). */
export const EARLIER_SQL_R30 = earlier(`iq.rowid IN (SELECT x.rowid FROM json_each(j.value -> '$.n') n, item_questions x WHERE ${OR5})`, `iq.seq, iq.publish_id, iq.lineage_seq, iq.rowid`);
/** The lookup's positions as the old statements took them (without round 31's candidates `c`). */
export const withoutCandidates = (json: string) => JSON.stringify((JSON.parse(json) as Record<string, unknown>[]).map(({ c: _c, ...x }) => x));

/** The hourly licence re-key's four statements before round 31 (src/pool/publish.ts `licenceRekey`). */
export function licenceRekeyR30(limit: number): string[] {
  const map = (a: string) => `(SELECT 'q:' || CASE WHEN p.hide_key = 1 THEN q.near_stripped ELSE q.near_full END
      FROM item_questions q JOIN item_publishes p ON p.publish_id = q.publish_id
     WHERE p.item_id = ${a}.item_id
       AND (CASE WHEN p.hide_key = 1 THEN q.stripped_content ELSE q.full_content END) = substr(${a}.ckey, 3)
       AND (CASE WHEN p.hide_key = 1 THEN q.near_stripped ELSE q.near_full END) IS NOT NULL
    UNION ALL
    SELECT 'q:' || CASE WHEN gp.hide_key = 1 THEN gq.near_stripped ELSE gq.near_full END
      FROM ghost_questions gq JOIN ghost_publishes gp ON gp.publish_id = gq.publish_id
     WHERE gp.item_id = ${a}.item_id
       AND (CASE WHEN gp.hide_key = 1 THEN gq.stripped_content ELSE gq.full_content END) = substr(${a}.ckey, 3)
       AND (CASE WHEN gp.hide_key = 1 THEN gq.near_stripped ELSE gq.near_full END) IS NOT NULL
    LIMIT 1)`;
  const mappable = (a: string) => `EXISTS (SELECT 1 FROM item_questions q JOIN item_publishes p ON p.publish_id = q.publish_id
      WHERE p.item_id = ${a}.item_id
        AND (CASE WHEN p.hide_key = 1 THEN q.stripped_content ELSE q.full_content END) = substr(${a}.ckey, 3)
        AND (CASE WHEN p.hide_key = 1 THEN q.near_stripped ELSE q.near_full END) IS NOT NULL
     UNION ALL
     SELECT 1 FROM ghost_questions gq JOIN ghost_publishes gp ON gp.publish_id = gq.publish_id
      WHERE gp.item_id = ${a}.item_id
        AND (CASE WHEN gp.hide_key = 1 THEN gq.stripped_content ELSE gq.full_content END) = substr(${a}.ckey, 3)
        AND (CASE WHEN gp.hide_key = 1 THEN gq.near_stripped ELSE gq.near_full END) IS NOT NULL)`;
  const m = map("cl");
  const mo = map("o");
  return [
    `INSERT INTO content_licences (item_id, ckey, license, from_at, until_at)
       SELECT cl.item_id, ${m}, cl.license, cl.from_at, cl.until_at
         FROM (SELECT * FROM content_licences cx WHERE cx.ckey LIKE 'q:%' AND ${mappable("cx")}
                ORDER BY cx.rowid LIMIT ${limit}) cl
        WHERE ${m} IS NOT NULL AND ${m} <> cl.ckey
       ON CONFLICT (item_id, ckey, license) DO UPDATE SET
         from_at = MIN(content_licences.from_at, excluded.from_at),
         until_at = CASE WHEN content_licences.until_at IS NULL OR excluded.until_at IS NULL THEN NULL
                         ELSE MAX(content_licences.until_at, excluded.until_at) END`,
    `DELETE FROM content_licences WHERE rowid IN (
         SELECT cl.rowid FROM (SELECT rowid, item_id, ckey FROM content_licences cx
                                WHERE cx.ckey LIKE 'q:%' AND ${mappable("cx")}
                                ORDER BY cx.rowid LIMIT ${limit}) cl
          WHERE ${m} IS NOT NULL AND ${m} <> cl.ckey)`,
    `WITH todo AS MATERIALIZED (SELECT item_id, ckey FROM item_offer ox
                                  WHERE ox.ckey LIKE 'q:%' AND ${mappable("ox")}
                                  ORDER BY ox.item_id, ox.ckey LIMIT ${limit})
       INSERT OR IGNORE INTO item_offer (item_id, ckey)
       SELECT o.item_id, ${mo} FROM todo o WHERE ${mo} IS NOT NULL AND ${mo} <> o.ckey`,
    `WITH todo AS MATERIALIZED (SELECT item_id, ckey FROM item_offer ox
                                  WHERE ox.ckey LIKE 'q:%' AND ${mappable("ox")}
                                  ORDER BY ox.item_id, ox.ckey LIMIT ${limit})
       DELETE FROM item_offer WHERE EXISTS (
         SELECT 1 FROM todo o WHERE o.item_id = item_offer.item_id AND o.ckey = item_offer.ckey AND ${mo} IS NOT NULL AND ${mo} <> o.ckey)`,
  ];
}
