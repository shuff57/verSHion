      WITH b AS MATERIALIZED (SELECT CAST(j.value ->> '$.pos' AS INTEGER) AS pos, j.value ->> '$.mh' AS mh, j.value ->> '$.nfull' AS nfull, j.value ->> '$.nbare' AS nbare,
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
            rows AS MATERIALIZED (SELECT * FROM (
