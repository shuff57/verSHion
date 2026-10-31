//! The plan for blocking one item (worker/src/pool/blocking.ts `blockPlan`): which of its questions and images a privacy purge or a
//! DMCA takedown covers, by default and as chosen by the admin, with a preview of every other item the block would hold.
use super::blocking::{Affected, Affects, BlobPlan, BlockPlan, BlockSelection, First, GENERIC_STEM_AUTHORS, MatchPlan, STEM_ONLY, is_scope, uses_blocked};
use super::ctx::{PoolCtx, PoolError, Res, bad};
use super::served::served_form;
use super::sqlfrag::{is_id, q_key_eq, q_key_in};
use rusqlite::{OptionalExtension, ToSql, params};
use std::collections::{HashMap, HashSet};

struct Q {
    pos: i64,
    match_hash: String,
    seq: i64,
    qkind: String,
    bk_norm: Option<String>,
    bk_stem: Option<String>,
    bk_full: Option<String>,
    bk_expl: Option<String>,
    bk_ans: Option<String>,
}

type Stat = (i64, Option<i64>);

fn min_opt(a: Option<i64>, b: Option<i64>) -> Option<i64> {
    match (a, b) {
        (None, b) => b,
        (a, None) => a,
        (Some(a), Some(b)) => Some(a.min(b)),
    }
}

/// JS `Number(k)` for a position key that must be an integer in range.
fn pos_of_key(k: &str) -> Option<i64> {
    let t = k.trim();
    let f: f64 = if t.is_empty() { 0.0 } else { t.parse().ok()? };
    (f.fract() == 0.0 && f.is_finite()).then_some(f as i64)
}

/// `kind`: "privacy" or "dmca". `size`: the affected-list cap (AFFECTED_LIST). `rightful`: the rightful item of a takedown.
pub fn block_plan(x: &PoolCtx, size: usize, item_id: &str, sel: &BlockSelection, kind: &str, rightful: Option<&str>) -> Res<BlockPlan> {
    let privacy = kind == "privacy";
    let live: Option<(String, Option<String>)> =
        x.conn.query_row("SELECT author_id, current_publish_id FROM items WHERE id = ?1 AND purged_at IS NULL", [item_id], |r| Ok((r.get(0)?, r.get(1)?))).optional()?;
    // A takedown of an item purged before its notice is planned from its ghost (the keyed block keys its last ghost publish kept)
    // by the SAME rules as a live item. With no selection NOTHING is selected (`selection_required`).
    let (author_id, current_publish_id, ghost) = match live {
        Some((a, p)) => (a, p, false),
        None => {
            let gone: Option<String> = if kind == "dmca" {
                x.conn.query_row("SELECT author_id FROM items WHERE id = ?1 AND purged_at IS NOT NULL", [item_id], |r| r.get(0)).optional()?
            } else {
                None
            };
            let gone = gone.ok_or_else(|| PoolError::new(404, "not_found"))?;
            let gp: Option<String> =
                x.conn.query_row("SELECT publish_id FROM ghost_publishes WHERE item_id = ?1 ORDER BY at DESC, publish_id DESC LIMIT 1", [item_id], |r| r.get(0)).optional()?;
            let Some(gp) = gp else {
                return Ok(BlockPlan { item_id: item_id.into(), ghost: Some(true), selection_required: Some(false), ..Default::default() });
            };
            (gone, Some(gp), true)
        }
    };
    let k = x.limit_of("*", "block_common_k")?;
    let qs: Vec<Q> = x
        .conn
        .prepare(if ghost {
            "SELECT gq.pos, gq.match_hash, gq.seq, COALESCE(gq.qkind, '') AS qkind, gq.bk_norm, gq.bk_stem, gq.bk_full, gq.bk_expl, gq.bk_ans FROM ghost_questions gq WHERE gq.publish_id = ?1 ORDER BY gq.pos"
        } else {
            "SELECT iq.pos, iq.match_hash, iq.seq, iq.qkind, iq.bk_norm, iq.bk_stem, iq.bk_full, iq.bk_expl, iq.bk_ans FROM item_questions iq WHERE iq.publish_id = ?1 ORDER BY iq.pos"
        })?
        .query_map([&current_publish_id], |r| {
            Ok(Q { pos: r.get(0)?, match_hash: r.get(1)?, seq: r.get(2)?, qkind: r.get(3)?, bk_norm: r.get(4)?, bk_stem: r.get(5)?, bk_full: r.get(6)?, bk_expl: r.get(7)?, bk_ans: r.get(8)? })
        })?
        .collect::<Result<_, _>>()?;
    // Each Multipart part's keys.
    let pk: Vec<(i64, i64, String)> = x
        .conn
        .prepare(&format!("SELECT pos, part, key FROM {} WHERE publish_id = ?1 ORDER BY pos, part, key", if ghost { "ghost_block_keys" } else { "item_part_keys" }))?
        .query_map([&current_publish_id], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))?
        .collect::<Result<_, _>>()?;
    let parts_of = |pos: i64, pred: &dyn Fn(&str, i64) -> bool| -> Vec<String> { pk.iter().filter(|(p, part, key)| *p == pos && pred(key, *part)).map(|(_, _, key)| key.clone()).collect() };
    // Each position's scope and keys.
    if let Some(scopes) = &sel.scopes {
        for (key, v) in scopes {
            if !pos_of_key(key).is_some_and(|p| p >= 0 && (p as usize) < qs.len()) || !is_scope(v) {
                return Err(bad("scopes").into());
            }
        }
    }
    let scope_of = |q: &Q| -> String {
        sel.scopes.as_ref().and_then(|s| s.get(&q.pos.to_string())).and_then(|v| v.as_str()).map(String::from).unwrap_or_else(|| if privacy { "default".into() } else { "skeleton".into() })
    };
    let keys_of = |q: &Q, sc: &str| -> Res<Vec<String>> {
        let stem_keys = || vec![Some(q.match_hash.clone()), q.bk_norm.clone(), q.bk_stem.clone()];
        let ks: Vec<Option<String>> = if let Some(part) = sc.strip_prefix("part:") {
            let n: i64 = part.parse().unwrap_or(-1);
            parts_of(q.pos, &|_, p| p == n).into_iter().map(Some).collect()
        } else {
            match sc {
                "default" => stem_keys().into_iter().chain([q.bk_full.clone()]).chain(parts_of(q.pos, &|k, _| k.starts_with("s:")).into_iter().map(Some)).collect(),
                "stem" => stem_keys().into_iter().chain(parts_of(q.pos, &|k, _| k.starts_with("s:")).into_iter().map(Some)).collect(),
                "skeleton" => vec![Some(q.match_hash.clone()), q.bk_norm.clone()],
                "full" => vec![q.bk_full.clone()],
                "explanation" => [q.bk_expl.clone()].into_iter().chain(parts_of(q.pos, &|k, _| k.starts_with("e:")).into_iter().map(Some)).collect(),
                _ => [q.bk_ans.clone()].into_iter().chain(parts_of(q.pos, &|k, _| k.starts_with("a:")).into_iter().map(Some)).collect(),
            }
        };
        let out: Vec<String> = ks.into_iter().flatten().filter(|s| !s.is_empty()).collect();
        if out.is_empty() {
            return Err(PoolError::new(400, "invalid_request").with("field", "scopes".into()).with("pos", q.pos.into()).with("message", format!("Question {} has no {sc} to block.", q.pos).into()).into());
        }
        Ok(out)
    };
    let mut pos_keys: Vec<(String, Vec<String>)> = Vec::new();
    for q in &qs {
        let sc = scope_of(q);
        let ks = keys_of(q, &sc)?;
        pos_keys.push((sc, ks));
    }
    let mut all_keys: Vec<String> = Vec::new();
    for (_, ks) in &pos_keys {
        for k in ks {
            if !all_keys.contains(k) {
                all_keys.push(k.clone());
            }
        }
    }
    let max_of = |m: &HashMap<String, Stat>, keys: &[String]| -> Stat {
        keys.iter().fold((0, None), |a, k| match m.get(k) {
            Some(v) => (a.0.max(v.0), min_opt(a.1, v.1)),
            None => a,
        })
    };
    // Only what non-authors were SERVED: an image only in a hidden publish's full form is never blocked, so no block or hold can turn
    // a guess at it into an oracle. Its seq is the first publish of THIS item that served it.
    let imgs: Vec<(String, i64)> = if ghost {
        // (a ghost's served images: its image units, the content hash, not the pixel key)
        x.conn
            .prepare(
                "SELECT gu.key AS sha256, MIN(COALESCE(gu.seq, 0)) AS seq FROM ghost_units gu JOIN ghost_publishes gp ON gp.publish_id = gu.publish_id
                  WHERE gp.item_id = ?1 AND gu.kind = 'image' AND gu.key NOT LIKE 'px:%' GROUP BY gu.key ORDER BY gu.key",
            )?
            .query_map([item_id], |r| Ok((r.get(0)?, r.get(1)?)))?
            .collect::<Result<_, _>>()?
    } else {
        x.conn
            .prepare(&format!(
                "SELECT ib.sha256, MIN(ib.seq) AS seq FROM item_blobs ib JOIN item_publishes p ON p.publish_id = ib.publish_id
        WHERE p.item_id = ?1 AND {} GROUP BY ib.sha256 ORDER BY ib.sha256",
                served_form("ib", "p")
            ))?
            .query_map([item_id], |r| Ok((r.get(0)?, r.get(1)?)))?
            .collect::<Result<_, _>>()?
    };
    let img_hashes: Vec<String> = imgs.iter().map(|(h, _)| h.clone()).collect();
    // How many OTHER authors' VISIBLE items (live, public) use each question / serve each image, and the earliest of them.
    let visible = "i.removed_at IS NULL AND i.visibility = 'public' AND i.purged_at IS NULL AND i.author_id <> ?2";
    let stats = |sql: &str, hashes: &[String]| -> rusqlite::Result<HashMap<String, Stat>> {
        if hashes.is_empty() {
            return Ok(HashMap::new());
        }
        x.conn.prepare(sql)?.query_map(params![serde_json::to_string(hashes).unwrap(), author_id], |r| Ok((r.get::<_, String>(0)?, (r.get::<_, i64>(1)?, r.get::<_, Option<i64>>(2)?))))?.collect()
    };
    let m_common = stats(
        &format!(
            "SELECT k.value AS h, COUNT(DISTINCT i.author_id) AS n, MIN(iq.seq) AS first FROM json_each(?1) k CROSS JOIN item_questions iq ON {}
       JOIN items i ON i.current_publish_id = iq.publish_id
      WHERE {visible} GROUP BY k.value",
            q_key_eq("iq", "k.value")
        ),
        &all_keys,
    )?;
    let b_common = stats(
        &format!(
            "SELECT ib.sha256 AS h, COUNT(DISTINCT i.author_id) AS n, MIN(ib.seq) AS first FROM item_blobs ib
       JOIN items i ON i.current_publish_id = ib.publish_id JOIN item_publishes p ON p.publish_id = ib.publish_id
      WHERE ib.sha256 IN (SELECT value FROM json_each(?1)) AND {visible} AND {} GROUP BY ib.sha256",
            served_form("ib", "p")
        ),
        &img_hashes,
    )?;
    // A privacy block by default selects only what no OTHER author uses on their own: any item, any visibility or state, except copies
    // that trace to this item. K (block_common_k) applies to DMCA blocks only.
    let not_copy = "NOT EXISTS (SELECT 1 FROM imports im WHERE im.user_id = i.author_id AND im.item_id = ?3)
                   AND NOT EXISTS (SELECT 1 FROM item_publishes p2 JOIN item_questions q2 ON q2.publish_id = p2.publish_id
                                    WHERE p2.item_id = i.id AND (q2.origin_item_id = ?3 OR instr(q2.chain_json, ?3) > 0))";
    let any_other = |sql: &str, hashes: &[String]| -> rusqlite::Result<HashMap<String, i64>> {
        if !privacy || hashes.is_empty() {
            return Ok(HashMap::new());
        }
        x.conn.prepare(sql)?.query_map(params![serde_json::to_string(hashes).unwrap(), author_id, item_id], |r| Ok((r.get::<_, String>(0)?, r.get::<_, i64>(1)?)))?.collect()
    };
    let m_any = any_other(
        &format!(
            "SELECT k.value AS h, COUNT(DISTINCT i.author_id) AS n FROM json_each(?1) k CROSS JOIN item_questions iq ON {} JOIN item_publishes p ON p.publish_id = iq.publish_id JOIN items i ON i.id = p.item_id
      WHERE i.author_id <> ?2 AND i.purged_at IS NULL AND {not_copy} GROUP BY k.value",
            q_key_eq("iq", "k.value")
        ),
        &all_keys,
    )?;
    let b_any = any_other(
        &format!(
            "SELECT ib.sha256 AS h, COUNT(DISTINCT i.author_id) AS n FROM item_blobs ib JOIN item_publishes p ON p.publish_id = ib.publish_id JOIN items i ON i.id = p.item_id
      WHERE ib.sha256 IN (SELECT value FROM json_each(?1)) AND i.author_id <> ?2 AND i.purged_at IS NULL AND {} AND {not_copy} GROUP BY ib.sha256",
            served_form("ib", "p")
        ),
        &img_hashes,
    )?;
    // A takedown naming the rightful item: the infringing copies of the rightful item's own material are selected by default.
    let mut r_hashes: HashSet<String> = HashSet::new();
    if let Some(r) = rightful.filter(|r| is_id(r)) {
        let a: Vec<String> = x
            .conn
            .prepare(
                "SELECT DISTINCT j.value AS h FROM item_publishes rp JOIN item_questions rq ON rq.publish_id = rp.publish_id, json_each(json_array(rq.match_hash, rq.bk_norm, rq.bk_full, rq.bk_expl, rq.bk_ans)) j
                 WHERE rp.item_id = ?1 AND rp.status IN ('current', 'previous') AND j.value IS NOT NULL",
            )?
            .query_map([r], |row| row.get(0))?
            .collect::<Result<_, _>>()?;
        let b: Vec<String> = x
            .conn
            .prepare(&format!("SELECT DISTINCT sb.sha256 AS h FROM item_publishes sp JOIN item_blobs sb ON sb.publish_id = sp.publish_id WHERE sp.item_id = ?1 AND {}", served_form("sb", "sp")))?
            .query_map([r], |row| row.get(0))?
            .collect::<Result<_, _>>()?;
        r_hashes.extend(a);
        r_hashes.extend(b);
    }
    // A privacy block has NO default selection: the admin ticks the questions and images that hold student data (the preview marks
    // `suggested` what only this item has). DMCA keeps its default.
    let by_default = |h: &str, other: i64, copied: bool| -> bool { !privacy && other < k && (!copied || r_hashes.contains(h)) };
    if let Some(qsel) = &sel.questions {
        if !qsel.iter().all(|p| *p >= 0 && (*p as usize) < qs.len()) {
            return Err(bad("questions").into());
        }
    }
    let known: HashSet<&String> = img_hashes.iter().collect();
    if let Some(isel) = &sel.images {
        if !isel.iter().all(|h| known.contains(h)) {
            return Err(bad("images").into());
        }
    }
    // Who published each question's content first (any item, any visibility), for the privacy preview.
    let mut first_of: HashMap<i64, First> = HashMap::new();
    if privacy {
        for (i, q) in qs.iter().enumerate() {
            if i >= 200 {
                break; // (bounded like `affects`)
            }
            let r: Option<(String, String)> = x
                .conn
                .query_row(
                    &format!(
                        "SELECT i.id AS item_id, i.author_id FROM json_each(?1) k CROSS JOIN item_questions iq ON {}
             JOIN item_publishes p ON p.publish_id = iq.publish_id JOIN items i ON i.id = p.item_id
            WHERE i.purged_at IS NULL ORDER BY iq.seq, i.id LIMIT 1",
                        q_key_eq("iq", "k.value")
                    ),
                    [serde_json::to_string(&pos_keys[i].1).unwrap()],
                    |row| Ok((row.get(0)?, row.get(1)?)),
                )
                .optional()?;
            if let Some((it, au)) = r.filter(|(it, _)| it != item_id) {
                first_of.insert(q.pos, First { item_id: it, author_id: au });
            }
        }
    }
    let mut matches: Vec<MatchPlan> = Vec::new();
    for (i, q) in qs.iter().enumerate() {
        let (scope, keys) = &pos_keys[i];
        let (other, first) = max_of(&m_common, keys);
        let copied = first.is_some_and(|f| f < q.seq);
        let any_n = keys.iter().map(|k| m_any.get(k).copied().unwrap_or(0)).max().unwrap_or(0).max(0);
        let stem_wide = keys.iter().any(|k| k.starts_with("s:")) || (scope == "skeleton" && STEM_ONLY.contains(&q.qkind.as_str()));
        // A stem many other authors' visible items share is generic: the preview says so and suggests a narrower scope.
        let generic_n = q.bk_stem.as_ref().and_then(|s| m_common.get(s)).map(|v| v.0).unwrap_or(0);
        let generic = stem_wide && generic_n >= GENERIC_STEM_AUTHORS;
        let selected = match &sel.questions {
            Some(qsel) => qsel.contains(&q.pos),
            None if ghost && !sel.all => false,
            None => by_default(&q.match_hash, other, copied),
        };
        matches.push(MatchPlan {
            pos: q.pos,
            hash: q.match_hash.clone(),
            keys: keys.clone(),
            scope: scope.clone(),
            stem_wide,
            generic_stem: generic.then_some(true),
            generic_authors: generic.then_some(generic_n),
            suggest_scope: generic.then(|| if q.bk_expl.is_some() { "explanation" } else if q.bk_ans.is_some() { "answer" } else { "full" }),
            affects: None,
            other_authors: if privacy { any_n } else { other },
            introduced: !copied,
            selected,
            // Student data is suggested whoever else pasted it, unless the stem is generic, and only where this item introduced it
            // (another author had it first: the preview names who, and the admin decides).
            suggested: privacy.then_some(!generic && !copied),
            first: if privacy && copied { first_of.get(&q.pos).cloned() } else { None },
        });
    }
    let blob: Vec<BlobPlan> = imgs
        .iter()
        .map(|(sha, seq)| {
            let st = b_common.get(sha);
            let other = st.map(|s| s.0).unwrap_or(0);
            let copied = st.and_then(|s| s.1).is_some_and(|f| f < *seq);
            let selected = match &sel.images {
                Some(isel) => isel.contains(sha),
                None if ghost && !sel.all => false,
                None => by_default(sha, other, copied),
            };
            BlobPlan {
                sha256: sha.clone(),
                other_authors: if privacy { b_any.get(sha).copied().unwrap_or(0) } else { other },
                introduced: !copied,
                selected,
                suggested: privacy.then_some(b_any.get(sha).copied().unwrap_or(0) == 0 && !copied),
            }
        })
        .collect();
    let mut mh_keys: Vec<String> = Vec::new();
    for k in matches.iter().filter(|m| m.selected).flat_map(|m| m.keys.iter()) {
        if !mh_keys.contains(k) {
            mh_keys.push(k.clone());
        }
    }
    let mh = serde_json::to_string(&mh_keys).unwrap();
    let bh = serde_json::to_string(&blob.iter().filter(|b| b.selected).map(|b| b.sha256.clone()).collect::<Vec<_>>()).unwrap();
    let wher = format!("i.id <> ?1 AND i.purged_at IS NULL AND {}", uses_blocked("?2", "?3"));
    let affected: Vec<Affected> = x
        .conn
        .prepare(&format!(
            "SELECT i.id AS item_id, i.author_id, i.title,
              CASE WHEN EXISTS (SELECT 1 FROM item_publishes p JOIN item_questions iq ON iq.publish_id = p.publish_id WHERE p.item_id = i.id AND {})
                   THEN 'question' ELSE 'image' END AS via
         FROM items i WHERE {wher} ORDER BY i.id LIMIT {size}",
            q_key_in("iq", "(SELECT value FROM json_each(?2))")
        ))?
        .query_map(params![item_id, mh, bh], |r| Ok(Affected { item_id: r.get(0)?, author_id: r.get(1)?, title: r.get(2)?, via: r.get(3)? }))?
        .collect::<Result<_, _>>()?;
    let total: i64 = if affected.len() < size {
        affected.len() as i64
    } else {
        x.conn.query_row(&format!("SELECT COUNT(*) FROM items i WHERE {wher}"), params![item_id, mh, bh], |r| r.get(0))?
    };
    // Who each selected question's block alone would hold, for up to 200 selected positions, 20 items each.
    let mut n_done = 0;
    for m in matches.iter_mut().filter(|m| m.selected) {
        if n_done >= 200 {
            break;
        }
        n_done += 1;
        let args: [&dyn ToSql; 2] = [&item_id, &serde_json::to_string(&m.keys).unwrap()];
        let r: Vec<Affects> = x
            .conn
            .prepare(&format!("SELECT i.id AS item_id, i.title FROM items i WHERE i.id <> ?1 AND i.purged_at IS NULL AND {} ORDER BY i.id LIMIT 20", uses_blocked("?2", "'[]'")))?
            .query_map(args.as_slice(), |r| Ok(Affects { item_id: r.get(0)?, title: r.get(1)? }))?
            .collect::<Result<_, _>>()?;
        m.affects = Some(r);
    }
    let none = !matches.iter().any(|m| m.selected) && !blob.iter().any(|b| b.selected);
    Ok(BlockPlan {
        item_id: item_id.into(),
        matches,
        blob,
        affected,
        affected_total: total,
        ghost: ghost.then_some(true),
        selection_required: ghost.then_some(none && sel.questions.is_none() && sel.images.is_none() && !sel.all),
    })
}

#[cfg(test)]
#[path = "blockplan_tests.rs"]
mod tests;
