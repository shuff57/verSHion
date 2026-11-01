//! The commit of a staged publish (worker/src/pool/publish.ts 2507-2911): ONE transaction, every statement guarded by "this publish is still the item's
//! current one" (compare-and-set). It creates or updates the item, flips the statuses, records the durable author records, places or lifts the holds the
//! privacy and copyright blocks call for, writes the flags and the audit row, and syncs the licence history. If the item moved on meanwhile (or any statement
//! fails, e.g. the CHECK that a public item needs a public licence), nothing is kept: the staged publish is discarded and the caller gets 409 `conflict`.
use super::croncore::discard_staged;
use super::ctx::{AuditEntry, Fail, PoolCtx, PoolError, Res, audit};
use super::offer::offer_sync;
use super::publishchecks::Carried;
use super::publishflags::Flag;
use super::publishinsert::Plan;
use super::publishmeta::PublishMeta;
use super::served::{rightful_exempts, served_blobs_of, upstream_carried};
use super::sqlfrag::{DAY_MS, in_q_keys_of, new_id, q_key_eq};
use super::textutil::{fold, title_search};
use crate::blobs::BlobStore;
use rusqlite::{OptionalExtension, params};
use serde_json::{Value, json};

/// An admin's release of a hold covers the keys it used for this long (and never a block placed after the release).
pub const HOLD_RELEASE_MS: i64 = 90 * DAY_MS;

/// SQL: does publish `pub_` still contain material the DMCA action of hold row `h` blocked (a match or a served image)?
pub fn dmca_still_used(h: &str, pub_: &str) -> String {
    format!(
        "EXISTS (SELECT 1 FROM blocked_hashes bh WHERE bh.action_id = substr({h}.source, 6)
             AND ((bh.kind = 'match' AND {}) OR (bh.kind = 'blob' AND bh.hash IN {})))",
        in_q_keys_of("bh.hash", pub_),
        served_blobs_of(pub_)
    )
}

fn guard(a: &str, b: &str) -> String {
    format!("EXISTS (SELECT 1 FROM items WHERE id = {a} AND current_publish_id = {b})")
}

pub struct CommitIn<'a> {
    pub me: &'a str,
    pub item_id: &'a str,
    pub is_new: bool,
    pub kind: &'a str,
    pub visibility: &'a str,
    pub version: i64,
    pub m: &'a PublishMeta,
    pub plan: &'a Plan,
    pub subject: &'a str,
    pub course: &'a str,
    pub grade: &'a str,
    pub n: usize,
    pub public_version: i64,
    pub public_at: Option<i64>,
    /// A hidden republish whose served form did not change (the previous stripped body is reused).
    pub reused: bool,
    pub token: &'a str,
    pub base_of: Option<&'a str>,
    pub result_json: &'a str,
    /// Licence-history keys of what this publish offers: `q:<canonical served content>` and `u:<unit key>`.
    pub ckeys: &'a [String],
    pub unit_record: &'a [(String, i64)],
    pub stem_record: &'a [(usize, String)],
    pub carried: Option<&'a Carried>,
    pub flags: &'a [Flag],
    pub folded: &'a [Flag],
    pub circle_claimed: &'a [String],
    pub circle_statement: Option<&'a str>,
    pub privacy_near: usize,
    pub full_key: &'a str,
    pub stripped_key: Option<&'a str>,
}

/// Runs the batch. `Ok(())`: this publish is now the item's current one. A lost race (or a failed statement) discards the staged publish and returns 409.
pub fn commit(x: &PoolCtx, store: &BlobStore, p: &CommitIn) -> Res<()> {
    let won = match batch(x, p) {
        Ok(current) => current.0.as_deref() == Some(p.m.publish_id.as_str()),
        Err(_) => false,
    };
    if won {
        return Ok(());
    }
    let current: Option<(Option<String>, i64)> = x.conn.query_row("SELECT current_publish_id, version FROM items WHERE id = ?1", [p.item_id], |r| Ok((r.get(0)?, r.get(1)?))).optional()?;
    let discarded = discard_staged(x, store, &p.m.publish_id, p.item_id, p.full_key, p.stripped_key, x.now).unwrap_or(false);
    if !discarded {
        // A retry re-staged this publish_id meanwhile: our bodies belong to nothing (its row has its own keys).
        let row: Option<(String, Option<String>)> = x.conn.query_row("SELECT full_key, stripped_key FROM item_publishes WHERE publish_id = ?1", [&p.m.publish_id], |r| Ok((r.get(0)?, r.get(1)?))).optional()?;
        for k in std::iter::once(p.full_key).chain(p.stripped_key) {
            if row.as_ref().is_none_or(|(f, s)| f != k && s.as_deref() != Some(k)) {
                let _ = store.delete(k);
            }
        }
    }
    let (cur, ver) = current.map_or((Value::Null, Value::Null), |(c, v)| (c.map_or(Value::Null, Value::String), json!(v)));
    Err(Fail::Pool(PoolError::new(409, "conflict").with("current_version", ver).with("current_publish_id", cur)))
}

fn batch(x: &PoolCtx, p: &CommitIn) -> rusqlite::Result<(Option<String>, i64)> {
    let (m, pl, me, item) = (p.m, p.plan, p.me, p.item_id);
    let (pid, now) = (m.publish_id.as_str(), x.now);
    let tx = x.conn.unchecked_transaction()?;
    let g = guard("?1", "?2");
    let hide = m.hide_key as i64;
    let (f_title, f_subject) = (title_search(&pl.title), fold(p.subject));
    let n = p.n as i64;
    if p.is_new {
        tx.execute(
            "INSERT INTO items (id, author_id, kind, visibility, current_publish_id, prev_publish_id, version, title, subject, course, grade,
               kinds_mask, question_count, hide_key, license, has_images, committed_at, created_at, updated_at, public_version, public_at, public_publish_id, title_fold, subject_fold)
             SELECT ?1, ?2, ?3, ?4, ?5, NULL, 1, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?15, ?15, 1, ?15, ?5, ?17, ?18
              WHERE EXISTS (SELECT 1 FROM item_publishes WHERE publish_id = ?5 AND status = 'staged' AND author_id = ?2 AND stage_token = ?16)
                AND NOT EXISTS (SELECT 1 FROM items WHERE id = ?1)",
            params![item, me, p.kind, p.visibility, pid, pl.title, p.subject, p.course, p.grade, pl.kinds_mask, n, hide, m.license, pl.has_images, now, p.token, f_title, f_subject],
        )?;
    } else {
        tx.execute(
            "UPDATE items SET current_publish_id = ?4, prev_publish_id = current_publish_id, version = ?5, title = ?6, subject = ?7, course = ?8, grade = ?9,
               kinds_mask = ?10, question_count = ?11, hide_key = ?12, license = ?13, has_images = ?14, committed_at = ?15, updated_at = ?15,
               public_version = ?17, public_at = COALESCE(?18, ?15), public_publish_id = CASE WHEN ?19 = 1 THEN COALESCE(public_publish_id, ?4) ELSE ?4 END,
               title_fold = ?20, subject_fold = ?21, license_version = license_version + (license <> ?13)
             WHERE id = ?1 AND author_id = ?2 AND current_publish_id IS ?3 AND removed_at IS NULL
               AND EXISTS (SELECT 1 FROM item_publishes WHERE publish_id = ?4 AND status = 'staged' AND author_id = ?2 AND stage_token = ?16)",
            params![item, me, m.base, pid, p.version, pl.title, p.subject, p.course, p.grade, pl.kinds_mask, n, hide, m.license, pl.has_images, now, p.token, p.public_version, p.public_at, p.reused as i64, f_title, f_subject],
        )?;
    }
    // Written with every round-12 column: nothing for the cron backfill to redo.
    tx.execute(&format!("UPDATE items SET fold_rev = 1 WHERE id = ?1 AND {g}"), params![item, pid])?;
    tx.execute(&format!("UPDATE item_publishes SET status = 'previous' WHERE item_id = ?1 AND status = 'current' AND publish_id <> ?2 AND {g}"), params![item, pid])?;
    tx.execute(&format!("UPDATE item_publishes SET status = 'current', committed_at = ?3, result_json = ?4 WHERE publish_id = ?2 AND status = 'staged' AND {g}"), params![item, pid, now, p.result_json])?;
    tx.execute(&format!("DELETE FROM pending_blobs WHERE uploader_id = ?3 AND sha256 IN (SELECT value FROM json_each(?4)) AND {g}"), params![item, pid, me, json!(pl.shas).to_string()])?;
    // Stem bands: their own near-rewordings under "own:<id>", every publish's served stems under "pub".
    tx.execute(
        &format!("INSERT OR IGNORE INTO stem_bands (user_id, band, item_id, qid) SELECT 'own:' || ?3, b.value, ?1, iq.qid FROM item_questions iq, json_each(COALESCE(iq.stem_bands, '[]')) b WHERE iq.publish_id = ?2 AND {g}"),
        params![item, pid, me],
    )?;
    tx.execute(
        &format!("INSERT OR IGNORE INTO stem_bands (user_id, band, item_id, qid) SELECT 'pub', b.value, ?1, iq.qid FROM item_questions iq, json_each(COALESCE(iq.stem_bands, '[]')) b WHERE iq.publish_id = ?2 AND {g}"),
        params![item, pid],
    )?;
    // The author's SERVED contents in the licence sense (what makes them first).
    tx.execute(
        &format!(
            "INSERT INTO author_contents (author_id, match_hash, kind, hash, first_seq)
             SELECT ?3, match_hash, 'nshown', CASE WHEN ?4 = 1 THEN near_stripped ELSE near_full END, seq FROM item_questions
              WHERE publish_id = ?2 AND CASE WHEN ?4 = 1 THEN near_stripped ELSE near_full END IS NOT NULL AND {g}
             ON CONFLICT (author_id, kind, hash, match_hash) DO UPDATE SET first_seq = MIN(first_seq, excluded.first_seq)"
        ),
        params![item, pid, me, hide],
    )?;
    // The units it used (texts, images, question units), durable.
    let unit_rec: Vec<Value> = p.unit_record.iter().map(|(k, s)| json!({ "k": k, "s": s })).collect();
    tx.execute(
        &format!(
            "INSERT INTO author_contents (author_id, match_hash, kind, hash, first_seq)
             SELECT ?3, '', 'unit', j.value ->> '$.k', CAST(j.value ->> '$.s' AS INTEGER) FROM json_each(?4) j WHERE {g}
             ON CONFLICT (author_id, kind, hash, match_hash) DO UPDATE SET first_seq = MIN(first_seq, excluded.first_seq)"
        ),
        params![item, pid, me, json!(unit_rec).to_string()],
    )?;
    // Key-free always, the full content when shown.
    tx.execute(
        &format!(
            "INSERT INTO author_contents (author_id, match_hash, kind, hash, first_seq)
             SELECT ?3, match_hash, 'kf', near_kf, seq FROM item_questions WHERE publish_id = ?2 AND near_kf IS NOT NULL AND {g}
             UNION ALL SELECT ?3, match_hash, 'shown', full_content, seq FROM item_questions WHERE publish_id = ?2 AND ?4 = 0 AND {g}
             ON CONFLICT (author_id, kind, hash, match_hash) DO UPDATE SET first_seq = MIN(first_seq, excluded.first_seq)"
        ),
        params![item, pid, me, hide],
    )?;
    // What this publish OFFERS; the licence history is synced from it at the end.
    tx.execute(&format!("DELETE FROM item_offer WHERE item_id = ?1 AND {g}"), params![item, pid])?;
    tx.execute(&format!("INSERT OR IGNORE INTO item_offer (item_id, ckey) SELECT ?1, value FROM json_each(?3) WHERE {g}"), params![item, pid, json!(p.ckeys).to_string()])?;
    tx.execute(
        &format!("INSERT OR IGNORE INTO attestations (publish_id, item_id, user_id, at, terms_version) SELECT ?2, ?1, ?3, ?4, ?5 WHERE {g}"),
        params![item, pid, me, now, m.attest.as_ref().map(|a| a.terms_version.as_str()).unwrap_or("")],
    )?;
    // The author's durable KEYED record of each stem; the question's own entry is read by its position.
    let stems: Vec<Value> = p.stem_record.iter().map(|(pos, e)| json!({ "p": pos, "e": e })).collect();
    tx.execute(
        &format!(
            "INSERT INTO author_stems (author_id, band, qhash, tok_sk, body_sk, bk_expl, bk_ans, bk_expl_full, first_seq)
             SELECT ?3, kb.value, iq.match_hash, iq.tok_sk, iq.body_sk, COALESCE(iq.bk_expl, ''), COALESCE(iq.bk_ans, ''), COALESCE(?4 ->> ('$[' || iq.pos || '].e'), ''), MIN(iq.seq)
               FROM item_questions iq CROSS JOIN json_each(COALESCE(iq.kbands, '[]')) kb
              WHERE iq.publish_id = ?2 AND iq.tok_sk IS NOT NULL AND CAST(?4 ->> ('$[' || iq.pos || '].p') AS INTEGER) = iq.pos AND {g}
              GROUP BY kb.value, iq.match_hash, iq.bk_expl, iq.bk_ans, ?4 ->> ('$[' || iq.pos || '].e')
             ON CONFLICT (author_id, band, qhash, bk_expl, bk_ans) DO UPDATE SET
               first_seq = MIN(first_seq, excluded.first_seq),
               bk_expl_full = COALESCE(author_stems.bk_expl_full, excluded.bk_expl_full)"
        ),
        params![item, pid, me, json!(stems).to_string()],
    )?;
    tx.execute(
        &format!(
            "INSERT INTO author_firsts (author_id, match_hash, first_seq)
             SELECT ?3, match_hash, MIN(seq) FROM item_questions WHERE publish_id = ?2 AND {g} GROUP BY match_hash
             ON CONFLICT (author_id, match_hash) DO UPDATE SET first_seq = MIN(first_seq, excluded.first_seq)"
        ),
        params![item, pid, me],
    )?;
    // A key_leak hold is the publish's: a republish whose SERVED form changed answers it; its reports are resolved as fixed.
    tx.execute(&format!("DELETE FROM item_holds WHERE item_id = ?1 AND source = 'key_leak' AND ?3 = 0 AND {g}"), params![item, pid, p.reused as i64])?;
    tx.execute(
        &format!("UPDATE reports SET resolved_at = ?3, action = 'fixed' WHERE item_id = ?1 AND reason = 'key_leak' AND resolved_at IS NULL AND ?4 = 0 AND {g}"),
        params![item, pid, now, p.reused as i64],
    )?;
    // The images were checked present above: a needs_reupload hold has been answered.
    tx.execute(&format!("DELETE FROM item_holds WHERE item_id = ?1 AND source = 'needs_reupload' AND {g}"), params![item, pid])?;
    // Privacy and DMCA blocks are re-checked INSIDE the batch against this publish's own rows: a block confirmed after the checks still holds it.
    let privacy_keys = format!("(pb.kind = 'match' AND {}) OR (pb.kind = 'blob' AND pb.hash IN {})", in_q_keys_of("pb.hash", "?2"), served_blobs_of("?2"));
    tx.execute(&format!("DELETE FROM item_holds WHERE item_id = ?1 AND source = 'privacy' AND {g} AND NOT EXISTS (SELECT 1 FROM privacy_blocks pb WHERE {privacy_keys})"), params![item, pid])?;
    tx.execute(
        &format!(
            "INSERT OR IGNORE INTO item_holds (item_id, reason, at, by, source)
             SELECT ?1, 'student_data', ?3, NULL, 'privacy' WHERE {g} AND EXISTS (SELECT 1 FROM privacy_blocks pb WHERE
               ({privacy_keys})
               AND NOT EXISTS (SELECT 1 FROM hold_releases hr, json_each(hr.keys_json) r WHERE hr.item_id = ?1 AND hr.source = 'privacy' AND r.value = pb.hash
                                 AND hr.at > ?3 - {HOLD_RELEASE_MS} AND pb.at <= hr.at))"
        ),
        params![item, pid, now],
    )?;
    // A near-rewording of confirmed student-data text is HELD too (its own source), never only flagged.
    let near = p.privacy_near as i64;
    tx.execute(&format!("DELETE FROM item_holds WHERE item_id = ?1 AND source = 'privacy_near' AND ?3 = 0 AND {g}"), params![item, pid, near])?;
    tx.execute(&format!("INSERT OR IGNORE INTO item_holds (item_id, reason, at, by, source) SELECT ?1, 'student_data', ?3, NULL, 'privacy_near' WHERE ?4 > 0 AND {g}"), params![item, pid, now, near])?;
    let exact = q_key_eq("iq", "pb.hash");
    tx.execute(
        &format!(
            "INSERT INTO admin_flags (id, kind, item_id, publish_id, user_id, detail, at)
             SELECT ?4, 'privacy_text_match', ?1, ?2, ?5, json_object('positions', (SELECT json_group_array(iq.pos) FROM item_questions iq JOIN privacy_blocks pb ON pb.kind = 'match' AND {exact} WHERE iq.publish_id = ?2)), ?3
              WHERE {g} AND EXISTS (SELECT 1 FROM item_questions iq JOIN privacy_blocks pb ON pb.kind = 'match' AND {exact} WHERE iq.publish_id = ?2)"
        ),
        params![item, pid, now, new_id(), me],
    )?;
    // A copyright hold whose action's blocked material this publish no longer contains is cleared, and the admin sees that it was.
    tx.execute(
        &format!(
            "INSERT INTO admin_flags (id, kind, item_id, publish_id, user_id, detail, at)
             SELECT ?3 || '-' || h.rowid, 'hold_cleared', ?1, ?2, ?4, json_object('source', h.source, 'reason', h.reason), ?5 FROM item_holds h
              WHERE h.item_id = ?1 AND h.source LIKE 'dmca:%' AND {g} AND EXISTS (SELECT 1 FROM blocked_hashes x WHERE x.action_id = substr(h.source, 6)) AND NOT {}",
            dmca_still_used("h", "?2")
        ),
        params![item, pid, new_id(), me, now],
    )?;
    tx.execute(
        &format!(
            "DELETE FROM item_holds WHERE item_id = ?1 AND source LIKE 'dmca:%' AND {g} AND EXISTS (SELECT 1 FROM blocked_hashes x WHERE x.action_id = substr(item_holds.source, 6)) AND NOT {}",
            dmca_still_used("item_holds", "?2")
        ),
        params![item, pid],
    )?;
    // DMCA re-upload blocks hold material that ENTERS the pool after the block: never an author the admin exempted, never what this item already carried.
    let right_author = "EXISTS (SELECT 1 FROM dmca_actions ra JOIN items ri ON ri.id = ra.rightful_item_id WHERE ra.id = bh.action_id AND ri.author_id = ?4)";
    tx.execute(
        &format!(
            "INSERT OR IGNORE INTO item_holds (item_id, reason, at, by, source)
             SELECT DISTINCT ?1, 'blocklist', ?3, NULL, 'dmca:' || bh.action_id FROM blocked_hashes bh
              WHERE {g} AND bh.lifted_at IS NULL AND (bh.expires_at IS NULL OR bh.expires_at > ?3) AND bh.action_id IS NOT NULL
                AND NOT EXISTS (SELECT 1 FROM hold_releases hr, json_each(hr.keys_json) r WHERE hr.item_id = ?1 AND hr.source = 'dmca:' || bh.action_id AND r.value = bh.hash
                                  AND hr.at > ?3 - {HOLD_RELEASE_MS} AND bh.at <= hr.at)
                AND NOT EXISTS (SELECT 1 FROM dmca_exempt e WHERE e.action_id = bh.action_id AND e.user_id = ?4)
                AND NOT EXISTS (SELECT 1 FROM dmca_actions a WHERE a.id = bh.action_id AND a.rightful_item_id IS NOT NULL AND {})
                AND (
                  (bh.kind = 'match' AND {}
                    AND NOT ({}
                             OR ({right_author}
                                 AND EXISTS (SELECT 1 FROM item_questions bq JOIN item_publishes bp ON bp.publish_id = bq.publish_id
                                              WHERE bq.publish_id = ?5 AND {} AND bp.committed_at <= bh.at))))
                  OR (bh.kind = 'blob' AND bh.hash IN {}
                    AND NOT ({}
                             OR ({right_author} AND ?5 IS NOT NULL AND bh.hash IN {} AND (SELECT committed_at FROM item_publishes WHERE publish_id = ?5) <= bh.at))))",
            rightful_exempts("a.rightful_item_id", "bh", "?4", "?2"),
            in_q_keys_of("bh.hash", "?2"),
            upstream_carried("bh.action_id", "?1", "bh.hash"),
            q_key_eq("bq", "bh.hash"),
            served_blobs_of("?2"),
            upstream_carried("bh.action_id", "?1", "bh.hash"),
            served_blobs_of("?5")
        ),
        params![item, pid, now, me, p.base_of],
    )?;
    if let Some(c) = p.carried {
        let holds = json!([{ "reason": c.reason, "source": format!("carried:{}", c.item_id) }]);
        tx.execute(
            &format!("INSERT OR IGNORE INTO item_holds (item_id, reason, at, by, source) SELECT ?1, j.value ->> '$.reason', ?3, NULL, j.value ->> '$.source' FROM json_each(?4) j WHERE {g}"),
            params![item, pid, now, holds.to_string()],
        )?;
    }
    if !p.flags.is_empty() {
        let rows: Vec<Value> = p.flags.iter().map(|f| json!({ "id": f.id, "kind": f.kind, "detail": f.detail })).collect();
        tx.execute(
            &format!("INSERT INTO admin_flags (id, kind, item_id, publish_id, user_id, detail, at) SELECT j.value ->> '$.id', j.value ->> '$.kind', ?1, ?2, ?3, j.value ->> '$.detail', ?4 FROM json_each(?5) j WHERE {g}"),
            params![item, pid, me, now, json!(rows).to_string()],
        )?;
    }
    for f in p.folded {
        tx.execute(
            &format!(
                "INSERT INTO admin_flags (id, kind, item_id, publish_id, user_id, detail, dedup, at)
                 SELECT ?5, ?6, ?1, ?2, ?3, json_object('aggregated', 1, 'publishes', 1, 'count', COALESCE(json_extract(?7, '$.count'), 1), 'last', json(?7)), ?8, ?4 WHERE {g}
                 ON CONFLICT (dedup) DO UPDATE SET
                   detail = json_set(admin_flags.detail, '$.publishes', COALESCE(json_extract(admin_flags.detail, '$.publishes'), 1) + 1,
                                     '$.count', COALESCE(json_extract(admin_flags.detail, '$.count'), 1) + COALESCE(json_extract(?7, '$.count'), 1), '$.last', json(?7)),
                   item_id = excluded.item_id, publish_id = excluded.publish_id, at = excluded.at, resolved_at = NULL, resolved_by = NULL"
            ),
            params![item, pid, me, now, f.id, f.kind, f.detail, f.dedup],
        )?;
    }
    let mut detail = json!({ "publish_id": pid, "version": p.version, "license": m.license, "hide_key": m.hide_key });
    if !m.independent.is_empty() {
        detail["independent_of"] = json!(m.independent);
    }
    if !p.circle_claimed.is_empty() {
        detail["circle_independent_of"] = json!(p.circle_claimed);
    }
    let note: Option<String> = match (m.statement.as_deref(), p.circle_statement) {
        (s, None) => s.map(String::from),
        (s, Some(c)) => Some(s.into_iter().chain(Some(c)).collect::<Vec<_>>().join(" / ")),
    };
    audit(
        &tx,
        &AuditEntry { actor_id: Some(me), actor_kind: "user", action: if p.is_new { "item.publish" } else { "item.republish" }, item_id: Some(item), target_user_id: Some(me), detail: Some(detail), note: note.as_deref() },
        now,
        Some(&guard(&format!("'{item}'"), &format!("'{pid}'"))),
    )?;
    // The licence history, last: after every hold this batch may place.
    offer_sync(&tx, item, now, Some(pid))?;
    let cur = tx.query_row("SELECT current_publish_id, version FROM items WHERE id = ?1", [item], |r| Ok((r.get::<_, Option<String>>(0)?, r.get::<_, i64>(1)?))).optional()?.unwrap_or((None, 0));
    if cur.0.as_deref() == Some(pid) {
        tx.commit()?;
    }
    Ok(cur)
}

#[cfg(test)]
#[path = "publishcommit_tests.rs"]
mod tests;
