              SELECT c.pos, iq.item_id, iq.qid, iq.match_hash, iq.content_hash, iq.kind, iq.author_name, iq.license, iq.licenses_json, iq.version, iq.versions_json,
                     iq.source_first_seq, iq.chain_json, iq.stem_tokens, iq.stem_hash, iq.at, iq.body_tokens, iq.tok_sk, iq.body_sk,
                     (c.nfull IS NOT NULL AND (iq.near_hash = c.nfull OR (c.noexpl = 1 AND iq.near_bare = c.nbare))) AS nr
                FROM cand c CROSS JOIN served sv ON sv.reader_id = ?1 AND sv.item_id = c.item_id
                CROSS JOIN ${v} iq ON iq.src_publish_id = sv.publish_id AND iq.user_id = ?1 AND iq.qid = c.qid
               WHERE iq.match_hash <> c.mh AND iq.void_at IS NULL AND iq.author_id <> ?1 AND (c.first IS NULL OR iq.source_first_seq < CAST(c.first AS INTEGER))
                 AND iq.item_id NOT IN (SELECT value FROM json_each(?3)) AND NOT EXISTS (SELECT 1 FROM void_items v WHERE v.item_id = iq.item_id)
