) LIMIT ?6),
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
         LEFT JOIN lic ON lic.pos = g.pos AND lic.item_id = g.item_id AND lic.qid = g.qid
