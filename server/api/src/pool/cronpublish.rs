//! The maintenance passes of worker/src/pool/publish.ts that the cron runs: the licence re-key and the stem backfill. Bounded and idempotent.
use super::ctx::{PoolCtx, Res};
use super::fingerprint::Fingerprinter;
use super::qhash::field_keys;
use super::qjson::Json;
use super::sqlfrag::new_id;
use crate::blobs::BlobStore;
use rusqlite::{OptionalExtension, params};

/// Old-style question licence/offer rows one hourly licence_rekey run maps (bounded, idempotent).
pub const LICENCE_REKEY_ROWS: usize = 1000;
/// Retained publishes one stem-backfill pass rebuilds (bounded; idempotent; resumes on the next run).
pub const STEM_BACKFILL_PUBLISHES: usize = 4;
/// Failures before an unreadable body is skipped (and the admin told).
pub const STEM_BACKFILL_FAILS: i64 = 3;
/// Authors a stem-backfill cron run sweeps.
pub const STEM_BACKFILL_AUTHORS: usize = 5;

/// Finish re-keying the licence history from the exact content hash (`q:<content>`) to the canonical served-content key (`q:<near>`) for rows the 0105
/// migration could not map yet. RETAINED publishes and GHOSTS are read: a version retired before the upgrade keeps its window through its ghost. Windows that
/// meet are merged: earliest from_at, open if any is open, else the latest until_at (no grant is lost). The batch is selected from MAPPABLE rows only, so an
/// unmappable old-style row (its content is gone) never hides a mappable one behind it.
pub fn licence_rekey(x: &PoolCtx) -> Res<usize> {
    let map = |a: &str| {
        format!(
            "(SELECT 'q:' || CASE WHEN p.hide_key = 1 THEN q.near_stripped ELSE q.near_full END
      FROM item_questions q JOIN item_publishes p ON p.publish_id = q.publish_id
     WHERE p.item_id = {a}.item_id
       AND (CASE WHEN p.hide_key = 1 THEN q.stripped_content ELSE q.full_content END) = substr({a}.ckey, 3)
       AND (CASE WHEN p.hide_key = 1 THEN q.near_stripped ELSE q.near_full END) IS NOT NULL
    UNION ALL
    SELECT 'q:' || CASE WHEN gp.hide_key = 1 THEN gq.near_stripped ELSE gq.near_full END
      FROM ghost_questions gq JOIN ghost_publishes gp ON gp.publish_id = gq.publish_id
     WHERE gp.item_id = {a}.item_id
       AND (CASE WHEN gp.hide_key = 1 THEN gq.stripped_content ELSE gq.full_content END) = substr({a}.ckey, 3)
       AND (CASE WHEN gp.hide_key = 1 THEN gq.near_stripped ELSE gq.near_full END) IS NOT NULL
    LIMIT 1)"
        )
    };
    // Only rows it can map enter the bounded batch: a `q:` row (any case) whose rest is the content of a question of its item, live or ghost, with a near
    // hash, found from the questions' side (each content looked up by its row's key).
    let mappable = |t: &str, cols: &str| {
        format!(
            "SELECT {cols} FROM item_questions q CROSS JOIN item_publishes p ON p.publish_id = q.publish_id
             CROSS JOIN {t} x ON x.item_id = p.item_id
               AND x.ckey IN ('q:' || (CASE WHEN p.hide_key = 1 THEN q.stripped_content ELSE q.full_content END), 'Q:' || (CASE WHEN p.hide_key = 1 THEN q.stripped_content ELSE q.full_content END))
       WHERE (CASE WHEN p.hide_key = 1 THEN q.near_stripped ELSE q.near_full END) IS NOT NULL
      UNION
      SELECT {cols} FROM ghost_questions gq CROSS JOIN ghost_publishes gp ON gp.publish_id = gq.publish_id
             CROSS JOIN {t} x ON x.item_id = gp.item_id
               AND x.ckey IN ('q:' || (CASE WHEN gp.hide_key = 1 THEN gq.stripped_content ELSE gq.full_content END), 'Q:' || (CASE WHEN gp.hide_key = 1 THEN gq.stripped_content ELSE gq.full_content END))
       WHERE (CASE WHEN gp.hide_key = 1 THEN gq.near_stripped ELSE gq.near_full END) IS NOT NULL"
        )
    };
    let batch_cl = format!("SELECT rowid, * FROM content_licences cx WHERE cx.rowid IN ({}) ORDER BY cx.rowid LIMIT {LICENCE_REKEY_ROWS}", mappable("content_licences", "x.rowid"));
    let batch_offer = format!("SELECT item_id, ckey FROM ({}) ORDER BY item_id, ckey LIMIT {LICENCE_REKEY_ROWS}", mappable("item_offer", "x.item_id, x.ckey"));
    let (m, mo) = (map("cl"), map("o"));
    let tx = x.conn.unchecked_transaction()?;
    tx.execute(
        &format!(
            "INSERT INTO content_licences (item_id, ckey, license, from_at, until_at)
       SELECT cl.item_id, {m}, cl.license, cl.from_at, cl.until_at
         FROM ({batch_cl}) cl
        WHERE {m} IS NOT NULL AND {m} <> cl.ckey
       ON CONFLICT (item_id, ckey, license) DO UPDATE SET
         from_at = MIN(content_licences.from_at, excluded.from_at),
         until_at = CASE WHEN content_licences.until_at IS NULL OR excluded.until_at IS NULL THEN NULL
                         ELSE MAX(content_licences.until_at, excluded.until_at) END"
        ),
        [],
    )?;
    let a = tx.execute(&format!("DELETE FROM content_licences WHERE rowid IN (SELECT cl.rowid FROM ({batch_cl}) cl WHERE {m} IS NOT NULL AND {m} <> cl.ckey)"), [])?;
    tx.execute(
        &format!("WITH todo AS MATERIALIZED ({batch_offer}) INSERT OR IGNORE INTO item_offer (item_id, ckey) SELECT o.item_id, {mo} FROM todo o WHERE {mo} IS NOT NULL AND {mo} <> o.ckey"),
        [],
    )?;
    let b = tx.execute(
        &format!(
            "WITH todo AS MATERIALIZED ({batch_offer})
       DELETE FROM item_offer WHERE EXISTS (
         SELECT 1 FROM todo o WHERE o.item_id = item_offer.item_id AND o.ckey = item_offer.ckey AND {mo} IS NOT NULL AND {mo} <> o.ckey)"
        ),
        [],
    )?;
    tx.commit()?;
    Ok(a + b)
}

/// A body that could not be read counts against the publish; the third time it is skipped for good and the admin is told.
fn stem_backfill_failed(x: &PoolCtx, publish_id: &str) -> rusqlite::Result<()> {
    let v: String = x.conn.query_row(
        "INSERT INTO pool_kv (key, value) VALUES (?1, '1') ON CONFLICT (key) DO UPDATE SET value = CAST(CAST(value AS INTEGER) + 1 AS TEXT) RETURNING value",
        [format!("stem_bf_fail:{publish_id}")],
        |r| r.get(0),
    )?;
    if v.parse::<i64>().unwrap_or(0) >= STEM_BACKFILL_FAILS {
        let tx = x.conn.unchecked_transaction()?;
        tx.execute("INSERT OR IGNORE INTO pool_kv (key, value) VALUES (?1, ?2)", params![format!("stem_bf_skip:{publish_id}"), x.now.to_string()])?;
        tx.execute(
            "INSERT OR IGNORE INTO admin_flags (id, kind, detail, dedup, at) VALUES (?1, 'cron_failure', ?2, ?3, ?4)",
            params![new_id(), serde_json::json!({ "stage": "stem_backfill", "publish_id": publish_id, "why": "body unreadable; skipped" }).to_string(), format!("stem_bf_skip:{publish_id}"), x.now],
        )?;
        tx.commit()?;
    }
    Ok(())
}

/// Fill `author_stems.bk_expl_full` for records written before the column existed, and rebuild stem records an older database never had. For a hidden
/// publish the full form's explanation key is computed from its retained full body; a SHOWN publish's served explanation key is already in
/// `item_questions.bk_expl`. An unreadable body leaves the key NULL ("unknown"), never a reason to credit a copier.
pub fn stem_backfill_author(x: &PoolCtx, store: &BlobStore, fp: &Fingerprinter, author: &str) -> Res<usize> {
    let cand: Vec<(String, i64, String)> = x
        .conn
        .prepare(&format!(
            "SELECT p.publish_id, p.hide_key, p.full_key FROM item_publishes p
        WHERE p.author_id = ?1 AND p.status IN ('current', 'previous') AND p.full_key IS NOT NULL
          AND NOT EXISTS (SELECT 1 FROM users du WHERE du.id = ?1 AND du.deleted_at IS NOT NULL)
          AND NOT EXISTS (SELECT 1 FROM pool_kv sk WHERE sk.key = 'stem_bf_skip:' || p.publish_id)
          AND EXISTS (SELECT 1 FROM item_questions q WHERE q.publish_id = p.publish_id AND q.tok_sk IS NOT NULL
                       AND NOT EXISTS (SELECT 1 FROM author_stems s WHERE s.author_id = ?1 AND s.qhash = q.match_hash AND s.bk_expl_full IS NOT NULL))
        ORDER BY p.publish_id LIMIT {STEM_BACKFILL_PUBLISHES}"
        ))?
        .query_map([author], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))?
        .collect::<Result<_, _>>()?;
    let mut done = 0;
    for (publish_id, hide_key, full_key) in cand {
        // (pos, bk_expl) of every question that has a stem record to fill
        let qs: Vec<(usize, Option<String>)> = x
            .conn
            .prepare("SELECT pos, bk_expl FROM item_questions WHERE publish_id = ?1 AND tok_sk IS NOT NULL ORDER BY pos")?
            .query_map([&publish_id], |r| Ok((r.get::<_, i64>(0)? as usize, r.get(1)?)))?
            .collect::<Result<_, _>>()?;
        if qs.is_empty() {
            continue;
        }
        let n = qs.iter().map(|q| q.0).max().unwrap_or(0) + 1;
        let mut expl: Vec<Option<String>> = vec![None; n];
        if hide_key == 1 {
            let body = store.get(&full_key).map_err(|e| rusqlite::Error::ToSqlConversionFailure(Box::new(e)))?;
            let Some(body) = body else {
                stem_backfill_failed(x, &publish_id)?; // unknown, never a credit
                continue;
            };
            let Some(parsed) = Json::parse(&body) else {
                stem_backfill_failed(x, &publish_id)?;
                continue;
            };
            let full: Vec<&Json> = match parsed.get("bank").and_then(|b| b.get("questions")) {
                Some(q) => q.as_arr().iter().collect(),
                None => parsed.get("question").filter(|q| !matches!(q, Json::Null)).into_iter().collect(),
            };
            let keys: Vec<Option<String>> = full.iter().map(|q| field_keys(fp, q).0).collect();
            for (pos, _) in &qs {
                expl[*pos] = Some(keys.get(*pos).cloned().flatten().unwrap_or_default());
            }
        } else {
            for (pos, e) in &qs {
                expl[*pos] = Some(e.clone().unwrap_or_default());
            }
        }
        x.conn.execute(
            "INSERT INTO author_stems (author_id, band, qhash, tok_sk, body_sk, bk_expl, bk_ans, bk_expl_full, first_seq)
         SELECT ?1, kb.value, q.match_hash, q.tok_sk, q.body_sk, COALESCE(q.bk_expl, ''), COALESCE(q.bk_ans, ''), ?3 ->> ('$[' || q.pos || ']'), q.seq
           FROM item_questions q CROSS JOIN json_each(COALESCE(q.kbands, '[]')) kb
          WHERE q.publish_id = ?2 AND q.tok_sk IS NOT NULL AND (?3 ->> ('$[' || q.pos || ']')) IS NOT NULL
            AND NOT EXISTS (SELECT 1 FROM users du WHERE du.id = ?1 AND du.deleted_at IS NOT NULL)
         ON CONFLICT (author_id, band, qhash, bk_expl, bk_ans) DO UPDATE SET
           first_seq = MIN(first_seq, excluded.first_seq),
           bk_expl_full = COALESCE(author_stems.bk_expl_full, excluded.bk_expl_full)",
            params![author, publish_id, serde_json::to_string(&expl).unwrap_or_default()],
        )?;
        done += 1;
    }
    Ok(done)
}

/// Cron sweep, a few authors a run (bounded; the next run continues): authors with a retained publish whose stem record lacks the full explanation key, or
/// whose record is missing. Never an erased account, never a publish whose body was unreadable three times.
pub fn stem_backfill(x: &PoolCtx, store: &BlobStore, fp: &Fingerprinter) -> Res<usize> {
    let authors: Vec<String> = x
        .conn
        .prepare(&format!(
            "SELECT DISTINCT p.author_id AS a FROM item_publishes p JOIN items i ON i.id = p.item_id AND i.purged_at IS NULL
        WHERE p.status IN ('current', 'previous') AND p.full_key IS NOT NULL
          AND NOT EXISTS (SELECT 1 FROM users du WHERE du.id = p.author_id AND du.deleted_at IS NOT NULL)
          AND NOT EXISTS (SELECT 1 FROM pool_kv sk WHERE sk.key = 'stem_bf_skip:' || p.publish_id)
          AND EXISTS (SELECT 1 FROM item_questions q WHERE q.publish_id = p.publish_id AND q.tok_sk IS NOT NULL
                       AND NOT EXISTS (SELECT 1 FROM author_stems s WHERE s.author_id = p.author_id AND s.qhash = q.match_hash AND s.bk_expl_full IS NOT NULL))
        LIMIT {STEM_BACKFILL_AUTHORS}"
        ))?
        .query_map([], |r| r.get(0))?
        .collect::<Result<_, _>>()?;
    let mut n = 0;
    for a in authors {
        n += stem_backfill_author(x, store, fp, &a)?;
    }
    Ok(n)
}

/// For the tests: whether a publish is marked skipped.
#[cfg(test)]
pub(super) fn skipped(conn: &rusqlite::Connection, publish_id: &str) -> bool {
    conn.query_row("SELECT 1 FROM pool_kv WHERE key = ?1", [format!("stem_bf_skip:{publish_id}")], |_| Ok(())).optional().unwrap().is_some()
}

#[cfg(test)]
#[path = "cronpublish_tests.rs"]
mod tests;
