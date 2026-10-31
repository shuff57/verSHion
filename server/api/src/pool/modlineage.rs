//! Quarantine moves, lineage copies and ghost lookups (worker/src/pool/moderation.ts): the read-side helpers the takedown, uphold and purge flows share.
use super::ctx::{PoolCtx, Res};
use super::served::{item_serves_blob, served_blobs_of, upstream_carried};
use super::sqlfrag::{DAY_MS, in_q_keys_of, new_id, q_key_in, visible_to_others};
use crate::blobs::BlobStore;
use rusqlite::{OptionalExtension, params};
use serde_json::json;

/// Copies one lineage lookup returns; one more says there are more (never a silent cut).
pub const LINEAGE_COPIES_MAX: usize = 500;

#[derive(Debug, Clone, PartialEq)]
pub struct Copy {
    pub item_id: String,
    pub author_id: String,
    pub title: String,
    pub visible: bool,
}

/// Options of `lineage_copies`: the infringing item's publish to compare with (read before a purge's claim clears it; `Some(None)` = none),
/// copies a hold of `held_by` already holds left out, and paging after an item id.
#[derive(Default, Clone)]
pub struct LineageOpts<'a> {
    pub src_publish: Option<Option<String>>,
    pub held_by: &'a str,
    pub after: &'a str,
}

/// Items that copy `item_id`: any other item whose current publish has one of its questions (match_hash, whatever the key) or records it as an
/// origin. A copy must have lineage AND copy the whole item or a designated question; an admin can widen it explicitly (`include_unlinked`).
pub fn lineage_copies(x: &PoolCtx, item_id: &str, designated: Option<&[String]>, include_unlinked: bool, blobs: Option<&[String]>, o: &LineageOpts) -> Res<Vec<Copy>> {
    let src: Option<String> = match &o.src_publish {
        Some(p) => p.clone(),
        None => x.conn.query_row("SELECT current_publish_id FROM items WHERE id = ?1", [item_id], |r| r.get(0)).optional()?.flatten(),
    };
    let sql = format!(
        "SELECT i.id, i.author_id, i.title,
                EXISTS (SELECT 1 FROM items vi JOIN users u ON u.id = vi.author_id WHERE vi.id = i.id AND {}) FROM items i
          WHERE i.id <> ?1 AND i.purged_at IS NULL AND i.current_publish_id IS NOT NULL AND i.id > ?6
            AND NOT EXISTS (SELECT 1 FROM item_holds hd WHERE hd.item_id = i.id AND hd.source = ?7)
            AND NOT EXISTS (SELECT 1 FROM dmca_exempt e JOIN dmca_actions a ON a.id = e.action_id WHERE a.item_id = ?1 AND e.user_id = i.author_id)
            AND NOT EXISTS (SELECT 1 FROM dmca_actions a JOIN items ri ON ri.id = a.rightful_item_id WHERE a.item_id = ?1 AND (ri.id = i.id OR ri.author_id = i.author_id))
            AND NOT EXISTS (
              SELECT 1 FROM (
                SELECT chain_json, origin_item_id FROM item_questions WHERE publish_id = ?5
                UNION ALL SELECT chain_json, NULL AS origin_item_id FROM ghost_questions WHERE publish_id = ?5) sr
               WHERE (sr.origin_item_id = i.id
                  OR EXISTS (SELECT 1 FROM json_each(COALESCE(sr.chain_json, '[]')) cl WHERE cl.value ->> '$.item_id' = i.id
                               AND COALESCE(cl.value ->> '$.earlier_match', 0) = 0 AND COALESCE(cl.value ->> '$.possible_source', 0) = 0))
                 AND NOT EXISTS (SELECT 1 FROM dmca_actions la JOIN blocked_hashes bh ON bh.action_id = la.id AND bh.lifted_at IS NULL
                                  WHERE la.item_id = ?1 AND la.restored_at IS NULL
                                    AND ((bh.kind = 'match' AND {}) OR (bh.kind = 'blob' AND bh.hash IN {}))
                                    AND NOT {}))
            AND (
              (?2 IS NOT NULL AND EXISTS (SELECT 1 FROM item_questions cm WHERE cm.publish_id = i.current_publish_id AND {}))
              OR (?4 IS NOT NULL AND {})
              OR EXISTS (SELECT 1 FROM item_questions o WHERE o.publish_id = i.current_publish_id AND o.origin_item_id = ?1
                        AND (?2 IS NULL OR {}))
              OR ((?3 = 1 OR EXISTS (SELECT 1 FROM imports im WHERE im.user_id = i.author_id AND im.item_id = ?1 AND im.kind = 'import'))
                  AND (CASE WHEN ?2 IS NULL
                         THEN EXISTS (SELECT 1 FROM item_questions src0 WHERE src0.publish_id = ?5 UNION ALL SELECT 1 FROM ghost_questions g0 WHERE g0.publish_id = ?5)
                          AND NOT EXISTS (SELECT 1 FROM (SELECT match_hash FROM item_questions WHERE publish_id = ?5 UNION ALL SELECT match_hash FROM ghost_questions WHERE publish_id = ?5) src
                                           WHERE NOT EXISTS (SELECT 1 FROM item_questions cq WHERE cq.publish_id = i.current_publish_id AND cq.match_hash = src.match_hash))
                         ELSE EXISTS (SELECT 1 FROM item_questions cq WHERE cq.publish_id = i.current_publish_id AND {}) END)))
          ORDER BY i.id LIMIT {}",
        visible_to_others().replace("i.", "vi."),
        in_q_keys_of("bh.hash", "i.current_publish_id"),
        served_blobs_of("i.current_publish_id"),
        upstream_carried("la.id", "i.id", "bh.hash"),
        q_key_in("cm", "(SELECT value FROM json_each(?2))"),
        item_serves_blob("i.id", |sha| format!("{sha} IN (SELECT value FROM json_each(?4))")),
        q_key_in("o", "(SELECT value FROM json_each(?2))"),
        q_key_in("cq", "(SELECT value FROM json_each(?2))"),
        LINEAGE_COPIES_MAX + 1
    );
    let designated_json = designated.map(|d| json!(d).to_string());
    let blobs_json = blobs.filter(|b| !b.is_empty()).map(|b| json!(b).to_string());
    let mut st = x.conn.prepare(&sql)?;
    let rows = st.query_map(params![item_id, designated_json, include_unlinked as i64, blobs_json, src, o.after, o.held_by], |r| {
        Ok(Copy { item_id: r.get(0)?, author_id: r.get(1)?, title: r.get(2)?, visible: r.get(3)? })
    })?;
    Ok(rows.collect::<Result<_, _>>()?)
}

/// Every copy, a page at a time (never only the first page).
pub fn all_lineage_copies(x: &PoolCtx, item_id: &str, designated: Option<&[String]>, include_unlinked: bool, blobs: Option<&[String]>, base: &LineageOpts) -> Res<Vec<Copy>> {
    let mut out = Vec::new();
    let mut after = String::new();
    loop {
        let o = LineageOpts { src_publish: base.src_publish.clone(), held_by: base.held_by, after: &after };
        let page = lineage_copies(x, item_id, designated, include_unlinked, blobs, &o)?;
        out.extend(page.iter().take(LINEAGE_COPIES_MAX).cloned());
        if page.len() <= LINEAGE_COPIES_MAX {
            return Ok(out);
        }
        after = page[LINEAGE_COPIES_MAX - 1].item_id.clone();
    }
}

/// The latest ghost publish of an item that is gone (what its copies are compared with).
pub fn latest_ghost(x: &PoolCtx, item_id: &str) -> Res<Option<String>> {
    Ok(x.conn.query_row("SELECT publish_id FROM ghost_publishes WHERE item_id = ?1 ORDER BY at DESC, publish_id DESC LIMIT 1", [item_id], |r| r.get(0)).optional()?)
}

/// The copies of an item that is gone, compared with its ghost.
pub fn ghost_copies(x: &PoolCtx, item_id: &str, held_by: &str, after: &str) -> Res<Vec<Copy>> {
    let src = latest_ghost(x, item_id)?;
    lineage_copies(x, item_id, None, false, None, &LineageOpts { src_publish: Some(src), held_by, after })
}

/// The item ids the publish derives from: its recorded origin and its derivation chain links (never an `earlier_match` credit or a possible source),
/// as a JSON array. Stored on the takedown action so it survives the copy's purge when no ghost is left.
pub fn derivation_of(x: &PoolCtx, publish_id: Option<&str>, ghost: bool) -> Res<String> {
    let Some(p) = publish_id else { return Ok("[]".into()) };
    let sql = format!(
        "SELECT DISTINCT j.id AS id FROM (
           {}
           SELECT cl.value ->> '$.item_id' AS id FROM {} q, json_each(q.chain_json) cl
            WHERE q.publish_id = ?1 AND COALESCE(cl.value ->> '$.earlier_match', 0) = 0 AND COALESCE(cl.value ->> '$.possible_source', 0) = 0) j
          WHERE j.id IS NOT NULL AND j.id <> ''",
        if ghost { "" } else { "SELECT origin_item_id AS id FROM item_questions WHERE publish_id = ?1 UNION ALL" },
        if ghost { "ghost_questions" } else { "item_questions" }
    );
    let mut st = x.conn.prepare(&sql)?;
    let ids: Vec<String> = st.query_map([p], |r| r.get(0))?.collect::<Result<_, _>>()?;
    Ok(json!(ids).to_string())
}

/// An upheld takedown voids the infringing item's evidence only when its author could have countered: an erased author cannot, so the takedown stands
/// but its licence keeps binding copies, unless the admin named the rightful item.
pub fn should_void_evidence(x: &PoolCtx, author_id: &str, rightful_item: Option<&str>) -> Res<bool> {
    if rightful_item.is_some() {
        return Ok(true);
    }
    Ok(x.conn.query_row("SELECT EXISTS (SELECT 1 FROM users WHERE id = ?1 AND deleted_at IS NULL)", [author_id], |r| r.get(0))?)
}

/// Moves every retained body of `item_id` under quarantine/ (or back), rewriting the keys. Returns the keys written.
pub fn move_bodies(x: &PoolCtx, store: &BlobStore, item_id: &str, to_quarantine: bool) -> Res<Vec<String>> {
    let mut written = Vec::new();
    let mut st = x.conn.prepare("SELECT publish_id, full_key, stripped_key FROM item_publishes WHERE item_id = ?1")?;
    let pubs: Vec<(String, String, Option<String>)> = st.query_map([item_id], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))?.collect::<Result<_, _>>()?;
    drop(st);
    for (publish_id, full_key, stripped_key) in pubs {
        let mut mv = |key: &Option<String>, form: &str| -> Res<Option<String>> {
            let Some(key) = key else { return Ok(None) };
            if key.starts_with("quarantine/") == to_quarantine {
                return Ok(Some(key.clone()));
            }
            let Some(bytes) = store.get(key).map_err(|e| super::ctx::Fail::Db(rusqlite::Error::ToSqlConversionFailure(Box::new(e))))? else { return Ok(Some(key.clone())) };
            let to = if to_quarantine { format!("quarantine/{item_id}/{publish_id}/{form}.json") } else { format!("items/{item_id}/{}.json", new_id()) };
            // A copy no row will reference if what follows fails is recorded as an orphan first, a day out; the row's update forgets it.
            x.conn.execute("INSERT INTO r2_orphans (key, at, next_at) VALUES (?1, ?2, ?3) ON CONFLICT (key) DO UPDATE SET next_at = excluded.next_at", params![to, x.now, x.now + DAY_MS])?;
            store.put(&to, &bytes).map_err(|e| super::ctx::Fail::Db(rusqlite::Error::ToSqlConversionFailure(Box::new(e))))?;
            written.push(to.clone());
            Ok(Some(to))
        };
        let nf = mv(&Some(full_key.clone()), "full")?;
        let ns = mv(&stripped_key, "stripped")?;
        let tx = x.conn.unchecked_transaction()?;
        tx.execute("UPDATE item_publishes SET full_key = ?2, stripped_key = ?3 WHERE publish_id = ?1", params![publish_id, nf, ns])?;
        let new_keys: Vec<&String> = [&nf, &ns].into_iter().flatten().collect();
        tx.execute("DELETE FROM r2_orphans WHERE key IN (SELECT value FROM json_each(?1))", [json!(new_keys).to_string()])?;
        tx.commit()?;
        for (old, new) in [(Some(full_key), nf), (stripped_key, ns)] {
            if let (Some(old), new) = (old, new) {
                if Some(&old) != new.as_ref() {
                    let _ = store.delete(&old);
                }
            }
        }
    }
    Ok(written)
}

/// Takedowns whose bodies were not all moved to quarantine (a run that failed part way) are finished.
pub fn finish_quarantines(x: &PoolCtx, store: &BlobStore) -> Res<usize> {
    let mut st = x.conn.prepare(
        "SELECT DISTINCT i.id FROM items i JOIN item_publishes p ON p.item_id = i.id
          WHERE i.removed_reason = 'takedown' AND i.purged_at IS NULL
            AND EXISTS (SELECT 1 FROM dmca_actions a WHERE a.item_id = i.id AND a.restored_at IS NULL AND a.purged_at IS NULL)
            AND (p.full_key NOT LIKE 'quarantine/%' OR (p.stripped_key IS NOT NULL AND p.stripped_key NOT LIKE 'quarantine/%'))
          LIMIT 50",
    )?;
    let ids: Vec<String> = st.query_map([], |r| r.get(0))?.collect::<Result<_, _>>()?;
    drop(st);
    for id in &ids {
        move_bodies(x, store, id, true)?;
    }
    Ok(ids.len())
}

#[cfg(test)]
#[path = "modlineage_tests.rs"]
mod tests;
