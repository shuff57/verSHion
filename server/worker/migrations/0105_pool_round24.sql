-- Round 24 (critics 31a / 31b): the canonical "same question" key for the licence history and the
-- relicence grant, and the offered-only rule for an item's licence now.
--
-- A question's licence window and "the very same content" are keyed by its canonical SERVED content,
-- the licence-sense near hash (item_questions.near_full, or near_stripped for a hidden-key publish):
-- stem, answers and explanation, choices and Matching columns as multisets, never points, difficulty,
-- topics or the shuffle order. The exact keyed content hash used before included points, difficulty
-- and topics and, for a hidden-key publish, the shuffled choice order, so an unrelated edit (a point
-- value, a revert) closed the reader's window and refused a licence the author had publicly relaxed.
--
-- Also, the item's licence NOW grants only while the item OFFERS that content publicly: an unlisted,
-- held or removed item grants nothing (round 23 B4 already applied this to its history windows).
--
-- The author's durable stem record also keeps the block key of her FULL form's EXPLANATION (round 24,
-- B-1): a hidden-key publish's served form has none, so when she later shows the same question (or
-- uses it in a second bank) her own earlier version must still count. It is a keyed HMAC, server-side
-- only, never a block for anyone. (The ANSWER is deliberately not kept from the full form: a hidden
-- key must never change an observable outcome, not even her own flags; see the H1 property.)
ALTER TABLE author_stems ADD COLUMN bk_expl_full TEXT;
-- Round 25 (critic-moodle-24 B-1): the derivation of the item a takedown acts on (the item ids its
-- publish names as origins or carries as derivation chain links), captured when the action is made.
-- A taken-down copy upheld with no serve leaves no ghost, so without this its source could not be
-- recognised when the original author republishes later.
ALTER TABLE dmca_actions ADD COLUMN derivation_json TEXT;
-- Round 26 (critic-moodle-33c B-1): what each item the taken-down item came from CARRIED of the
-- action's blocked hashes when the action was made. The upstream is exempt for exactly those: material
-- it adds after the block is held like any re-upload. `upstream_snap` = 1 when the snapshot was taken
-- (actions made before round 26 fall back to "in a publish committed by the action's time").
ALTER TABLE dmca_actions ADD COLUMN upstream_snap INTEGER NOT NULL DEFAULT 0;
CREATE TABLE dmca_upstream_keys (action_id TEXT NOT NULL, item_id TEXT NOT NULL, key TEXT NOT NULL, PRIMARY KEY (action_id, item_id, key)) WITHOUT ROWID;

-- Existing question windows and offers: re-key from the exact content hash to the canonical hash.
-- Both are 64 hex characters, so this is matched through the stored rows, never by shape. The mapping
-- reads RETAINED publishes AND GHOSTS: a version retired before the upgrade keeps its licence window
-- through its ghost (the exact content hash alone cannot be re-keyed once the version is gone).
-- Rows whose near hash is still NULL (fold_rev = 0, waiting for the cron backfill) are left for the
-- hourly licence_rekey stage, which runs right after the backfill; nothing is silently orphaned.
--
-- Several exact contents can map to one canonical key. The rows are MERGED (earliest from_at; open if
-- any is open, otherwise the latest until_at), so no grant is lost; the merge can only widen a window
-- across a gap between two offers of the same canonical question (a CC grant the author made is
-- irrevocable, and the old exact-keyed rows could not express the merged history anyway).
INSERT INTO content_licences (item_id, ckey, license, from_at, until_at)
SELECT cl.item_id,
       (SELECT 'q:' || CASE WHEN p.hide_key = 1 THEN q.near_stripped ELSE q.near_full END
          FROM item_questions q JOIN item_publishes p ON p.publish_id = q.publish_id
         WHERE p.item_id = cl.item_id
           AND (CASE WHEN p.hide_key = 1 THEN q.stripped_content ELSE q.full_content END) = substr(cl.ckey, 3)
           AND (CASE WHEN p.hide_key = 1 THEN q.near_stripped ELSE q.near_full END) IS NOT NULL
        UNION ALL
        SELECT 'q:' || CASE WHEN gp.hide_key = 1 THEN gq.near_stripped ELSE gq.near_full END
          FROM ghost_questions gq JOIN ghost_publishes gp ON gp.publish_id = gq.publish_id
         WHERE gp.item_id = cl.item_id
           AND (CASE WHEN gp.hide_key = 1 THEN gq.stripped_content ELSE gq.full_content END) = substr(cl.ckey, 3)
           AND (CASE WHEN gp.hide_key = 1 THEN gq.near_stripped ELSE gq.near_full END) IS NOT NULL
        LIMIT 1),
       cl.license, cl.from_at, cl.until_at
  FROM content_licences cl
 WHERE cl.ckey LIKE 'q:%'
   AND EXISTS (
     SELECT 1 FROM item_questions q JOIN item_publishes p ON p.publish_id = q.publish_id
      WHERE p.item_id = cl.item_id
        AND (CASE WHEN p.hide_key = 1 THEN q.stripped_content ELSE q.full_content END) = substr(cl.ckey, 3)
        AND (CASE WHEN p.hide_key = 1 THEN q.near_stripped ELSE q.near_full END) IS NOT NULL
     UNION ALL
     SELECT 1 FROM ghost_questions gq JOIN ghost_publishes gp ON gp.publish_id = gq.publish_id
      WHERE gp.item_id = cl.item_id
        AND (CASE WHEN gp.hide_key = 1 THEN gq.stripped_content ELSE gq.full_content END) = substr(cl.ckey, 3)
        AND (CASE WHEN gp.hide_key = 1 THEN gq.near_stripped ELSE gq.near_full END) IS NOT NULL)
ON CONFLICT (item_id, ckey, license) DO UPDATE SET
  from_at = MIN(content_licences.from_at, excluded.from_at),
  until_at = CASE WHEN content_licences.until_at IS NULL OR excluded.until_at IS NULL THEN NULL
                  ELSE MAX(content_licences.until_at, excluded.until_at) END;

DELETE FROM content_licences WHERE rowid IN (
  SELECT cl.rowid FROM content_licences cl
   WHERE cl.ckey LIKE 'q:%'
     AND EXISTS (
       SELECT 1 FROM item_questions q JOIN item_publishes p ON p.publish_id = q.publish_id
        WHERE p.item_id = cl.item_id
          AND (CASE WHEN p.hide_key = 1 THEN q.stripped_content ELSE q.full_content END) = substr(cl.ckey, 3)
          AND (CASE WHEN p.hide_key = 1 THEN q.near_stripped ELSE q.near_full END) IS NOT NULL
       UNION ALL
       SELECT 1 FROM ghost_questions gq JOIN ghost_publishes gp ON gp.publish_id = gq.publish_id
        WHERE gp.item_id = cl.item_id
          AND (CASE WHEN gp.hide_key = 1 THEN gq.stripped_content ELSE gq.full_content END) = substr(cl.ckey, 3)
          AND (CASE WHEN gp.hide_key = 1 THEN gq.near_stripped ELSE gq.near_full END) IS NOT NULL)
     -- (a canonical row re-written by the merge above is untouched: its key is not an exact hash)
     AND NOT EXISTS (
       SELECT 1 FROM item_questions q JOIN item_publishes p ON p.publish_id = q.publish_id
        WHERE p.item_id = cl.item_id
          AND 'q:' || (CASE WHEN p.hide_key = 1 THEN q.near_stripped ELSE q.near_full END) = cl.ckey));

INSERT OR IGNORE INTO item_offer (item_id, ckey)
SELECT o.item_id,
       (SELECT 'q:' || CASE WHEN p.hide_key = 1 THEN q.near_stripped ELSE q.near_full END
          FROM item_questions q JOIN item_publishes p ON p.publish_id = q.publish_id
         WHERE p.item_id = o.item_id
           AND (CASE WHEN p.hide_key = 1 THEN q.stripped_content ELSE q.full_content END) = substr(o.ckey, 3)
           AND (CASE WHEN p.hide_key = 1 THEN q.near_stripped ELSE q.near_full END) IS NOT NULL
        UNION ALL
        SELECT 'q:' || CASE WHEN gp.hide_key = 1 THEN gq.near_stripped ELSE gq.near_full END
          FROM ghost_questions gq JOIN ghost_publishes gp ON gp.publish_id = gq.publish_id
         WHERE gp.item_id = o.item_id
           AND (CASE WHEN gp.hide_key = 1 THEN gq.stripped_content ELSE gq.full_content END) = substr(o.ckey, 3)
           AND (CASE WHEN gp.hide_key = 1 THEN gq.near_stripped ELSE gq.near_full END) IS NOT NULL
        LIMIT 1)
  FROM item_offer o
 WHERE o.ckey LIKE 'q:%'
   AND EXISTS (
     SELECT 1 FROM item_questions q JOIN item_publishes p ON p.publish_id = q.publish_id
      WHERE p.item_id = o.item_id
        AND (CASE WHEN p.hide_key = 1 THEN q.stripped_content ELSE q.full_content END) = substr(o.ckey, 3)
        AND (CASE WHEN p.hide_key = 1 THEN q.near_stripped ELSE q.near_full END) IS NOT NULL
     UNION ALL
     SELECT 1 FROM ghost_questions gq JOIN ghost_publishes gp ON gp.publish_id = gq.publish_id
      WHERE gp.item_id = o.item_id
        AND (CASE WHEN gp.hide_key = 1 THEN gq.stripped_content ELSE gq.full_content END) = substr(o.ckey, 3)
        AND (CASE WHEN gp.hide_key = 1 THEN gq.near_stripped ELSE gq.near_full END) IS NOT NULL);

-- (item_offer is WITHOUT ROWID: delete by its own key.)
DELETE FROM item_offer WHERE EXISTS (
  SELECT 1 FROM item_offer o
   WHERE o.item_id = item_offer.item_id AND o.ckey = item_offer.ckey AND o.ckey LIKE 'q:%'
     AND EXISTS (
       SELECT 1 FROM item_questions q JOIN item_publishes p ON p.publish_id = q.publish_id
        WHERE p.item_id = o.item_id
          AND (CASE WHEN p.hide_key = 1 THEN q.stripped_content ELSE q.full_content END) = substr(o.ckey, 3)
          AND (CASE WHEN p.hide_key = 1 THEN q.near_stripped ELSE q.near_full END) IS NOT NULL
       UNION ALL
       SELECT 1 FROM ghost_questions gq JOIN ghost_publishes gp ON gp.publish_id = gq.publish_id
        WHERE gp.item_id = o.item_id
          AND (CASE WHEN gp.hide_key = 1 THEN gq.stripped_content ELSE gq.full_content END) = substr(o.ckey, 3)
          AND (CASE WHEN gp.hide_key = 1 THEN gq.near_stripped ELSE gq.near_full END) IS NOT NULL)
     AND NOT EXISTS (
       SELECT 1 FROM item_questions q JOIN item_publishes p ON p.publish_id = q.publish_id
        WHERE p.item_id = o.item_id
          AND 'q:' || (CASE WHEN p.hide_key = 1 THEN q.near_stripped ELSE q.near_full END) = o.ckey));

DROP VIEW import_questions;
DROP VIEW import_questions_live;
DROP VIEW import_questions_ghost;

CREATE VIEW import_questions_live AS
  SELECT sv.reader_id AS user_id, sv.item_id, q.qid,
         CASE WHEN p.hide_key = 1 THEN q.stripped_content ELSE q.full_content END AS content_hash,
         p.author_id, q.match_hash, q.keyfree_content AS keyfree_hash,
         CASE WHEN p.hide_key = 1 THEN q.near_stripped ELSE q.near_full END AS near_hash, q.near_kf,
         CASE WHEN p.hide_key = 1 THEN q.near_bare_stripped ELSE q.near_bare_full END AS near_bare,
         q.stem_tokens, CASE WHEN p.hide_key = 1 THEN 'stripped' ELSE 'full' END AS form, sv.kind, q.chain_json,
         COALESCE(u.display_name, 'deleted teacher') AS author_name, sv.license,
         (SELECT json_group_array(DISTINCT v) FROM (SELECT l.value AS v FROM json_each(sv.licenses_json) l
                                                   -- (round 22 V-a: every licence the item had at this very
                                                   -- content since the serve; round 24: canonical served content)
                                                   UNION SELECT cl.license FROM content_licences cl
                                                          WHERE cl.item_id = sv.item_id
                                                            AND cl.ckey = 'q:' || (CASE WHEN p.hide_key = 1 THEN q.near_stripped ELSE q.near_full END)
                                                            AND (cl.until_at IS NULL OR cl.until_at >= sv.first_at)
                                                   -- (round 20 E1: the source's licence now, when its public
                                                   -- version serves the very same question; round 24: canonical,
                                                   -- and only while the item is publicly offered)
                                                   UNION SELECT i.license FROM items i JOIN item_publishes cp ON cp.publish_id = i.public_publish_id
                                                           JOIN item_questions cq ON cq.publish_id = cp.publish_id AND cq.qid = q.qid
                                                          WHERE i.id = sv.item_id
                                                            AND (CASE WHEN cp.hide_key = 1 THEN cq.near_stripped ELSE cq.near_full END)
                                                                = (CASE WHEN p.hide_key = 1 THEN q.near_stripped ELSE q.near_full END)
                                                            AND (i.visibility = 'public' AND i.removed_at IS NULL AND i.purged_at IS NULL
                                                                 AND i.current_publish_id IS NOT NULL
                                                                 AND NOT EXISTS (SELECT 1 FROM item_holds oh WHERE oh.item_id = i.id)))) AS licenses_json,
         p.public_version AS version, json_array(p.public_version) AS versions_json, q.seq AS source_first_seq, q.stem_hash, sv.at, sv.void_at,
         q.publish_id AS src_publish_id, 1 AS rev, q.body_tokens, q.tok_sk, q.body_sk
    FROM item_questions q CROSS JOIN item_publishes p ON p.publish_id = q.publish_id CROSS JOIN served sv ON sv.publish_id = q.publish_id LEFT JOIN users u ON u.id = p.author_id;
CREATE VIEW import_questions_ghost AS
  SELECT sv.reader_id AS user_id, sv.item_id, q.qid,
         CASE WHEN p.hide_key = 1 THEN q.stripped_content ELSE q.full_content END AS content_hash,
         p.author_id, q.match_hash, q.keyfree_content AS keyfree_hash,
         CASE WHEN p.hide_key = 1 THEN q.near_stripped ELSE q.near_full END AS near_hash, q.near_kf,
         CASE WHEN p.hide_key = 1 THEN q.near_bare_stripped ELSE q.near_bare_full END AS near_bare,
         NULL AS stem_tokens, CASE WHEN p.hide_key = 1 THEN 'stripped' ELSE 'full' END AS form, sv.kind, q.chain_json,
         COALESCE(u.display_name, 'deleted teacher') AS author_name, sv.license,
         (SELECT json_group_array(DISTINCT v) FROM (SELECT l.value AS v FROM json_each(sv.licenses_json) l
                                                   -- (round 22 V-a; round 24: canonical served content)
                                                   UNION SELECT cl.license FROM content_licences cl
                                                          WHERE cl.item_id = sv.item_id
                                                            AND cl.ckey = 'q:' || (CASE WHEN p.hide_key = 1 THEN q.near_stripped ELSE q.near_full END)
                                                            AND (cl.until_at IS NULL OR cl.until_at >= sv.first_at)
                                                   -- (round 20 E1; round 24: canonical and offered-only)
                                                   UNION SELECT i.license FROM items i JOIN item_publishes cp ON cp.publish_id = i.public_publish_id
                                                           JOIN item_questions cq ON cq.publish_id = cp.publish_id AND cq.qid = q.qid
                                                          WHERE i.id = sv.item_id
                                                            AND (CASE WHEN cp.hide_key = 1 THEN cq.near_stripped ELSE cq.near_full END)
                                                                = (CASE WHEN p.hide_key = 1 THEN q.near_stripped ELSE q.near_full END)
                                                            AND (i.visibility = 'public' AND i.removed_at IS NULL AND i.purged_at IS NULL
                                                                 AND i.current_publish_id IS NOT NULL
                                                                 AND NOT EXISTS (SELECT 1 FROM item_holds oh WHERE oh.item_id = i.id))
                                                   UNION SELECT nl.value FROM json_each(COALESCE(q.now_licenses, '[]')) nl)) AS licenses_json,
         p.public_version AS version, json_array(p.public_version) AS versions_json, q.seq AS source_first_seq, q.stem_hash, sv.at, sv.void_at,
         q.publish_id AS src_publish_id, 1 AS rev, NULL AS body_tokens, q.tok_sk, q.body_sk
    FROM ghost_questions q CROSS JOIN ghost_publishes p ON p.publish_id = q.publish_id CROSS JOIN served sv ON sv.publish_id = q.publish_id LEFT JOIN users u ON u.id = p.author_id;
CREATE VIEW import_questions AS
  SELECT * FROM import_questions_live UNION ALL SELECT * FROM import_questions_ghost;
