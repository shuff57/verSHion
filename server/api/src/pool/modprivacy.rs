//! Privacy purge and its undoing, admin removals and appeal decisions (worker/src/pool/moderation.ts `privacyPurge`, `privacyFinish`, `privacyRestore`,
//! `purgeHeldCopies`, `adminUnremove`, `decideAppeal`, `appealsDue`).
use super::blocking::{AFFECTED_LIST, BlockPlan, hold_privacy_batch, hold_snapshot, release_orphan_holds, selected_blobs, selected_key_pairs, selected_match};
use super::ctx::{AuditEntry, PoolCtx, Res, audit};
use super::modlineage::move_bodies;
use super::modpurge::Actor;
use super::moderation::{APPEAL_MAX_MS, APPEAL_REMINDER_MS, PRIVACY_BYTES_GRACE_MS, open_appeal_sql};
use super::poolio::notify_held;
use super::sqlfrag::{DAY_MS, new_id, q_key_eq, shared_sql, usage_sql};
use crate::blobs::BlobStore;
use rusqlite::{OptionalExtension, params};
use serde_json::json;

fn actor_audit(tx: &rusqlite::Connection, now: i64, a: &Actor, action: &str, item: &str, target: Option<&str>, detail: Option<serde_json::Value>, note: Option<&str>, when: Option<&str>) -> rusqlite::Result<usize> {
    audit(tx, &AuditEntry { actor_id: a.id, actor_kind: a.kind, action, item_id: Some(item), target_user_id: target, detail, note }, now, when)
}

/// The rest of a privacy purge, idempotent: every copy held (fail closed, before anything is marked done), the bodies under quarantine/, then the
/// backlog resolved. The student's image is served from no item at once; its bytes, like the item's own bodies, are deleted only after
/// PRIVACY_BYTES_GRACE_MS (cron), so an unblock or upheld appeal in that window loses nothing.
pub fn privacy_finish(x: &PoolCtx, store: &BlobStore, item_id: &str, mh: &[String], bh: &[String], by: Option<&str>, batches: usize) -> Res<bool> {
    if hold_privacy_batch(x, 500, item_id, mh, bh, by, batches)? {
        return Ok(false);
    }
    let still: bool = x.conn.query_row("SELECT EXISTS (SELECT 1 FROM items WHERE id = ?1 AND removed_reason = 'privacy' AND purged_at IS NULL)", [item_id], |r| r.get(0))?;
    if still {
        move_bodies(x, store, item_id, true)?;
    }
    x.conn.execute("UPDATE admin_flags SET resolved_at = ?2 WHERE kind = 'privacy_backlog' AND item_id = ?1 AND resolved_at IS NULL", params![item_id, x.now])?;
    Ok(true)
}

/// Admin confirmation of a student-data report: the item is quarantined for the grace window, its selected question and image hashes blocklisted, the
/// affected copies held. False when it was already privacy-removed and has no unfinished backlog.
pub fn privacy_purge(x: &PoolCtx, store: &BlobStore, admin_id: &str, item_id: &str, plan: &BlockPlan) -> Res<bool> {
    let now = x.now;
    let mh = selected_match(plan);
    let bh = selected_blobs(plan);
    let pairs: Vec<serde_json::Value> = selected_key_pairs(plan).into_iter().map(|(h, q)| json!({ "h": h, "q": q })).collect();
    let stem_hashes: Vec<&String> = plan.matches.iter().filter(|m| m.selected && m.keys.iter().any(|k| k.starts_with("s:"))).map(|m| &m.hash).collect();
    let stem_keys: Vec<&String> = plan.matches.iter().filter(|m| m.selected).flat_map(|m| m.keys.iter().filter(|k| k.starts_with("s:"))).collect();
    let affected: Vec<&String> = plan.affected.iter().map(|a| &a.item_id).collect();
    let tx = x.conn.unchecked_transaction()?;
    // Quarantined for the grace window, not purged at once: invisible to everyone but its author now, restorable by an unblock or an upheld appeal,
    // purged by the cron after PRIVACY_BYTES_GRACE_MS. What it replaced is kept for a restore.
    let upd = tx.execute(
        "UPDATE items SET prior_removed_reason = removed_reason, prior_purge_after = purge_after, removed_at = ?2, removed_reason = 'privacy',
           purge_after = ?3, quarantined_at = COALESCE(quarantined_at, ?2), updated_at = ?2
         WHERE id = ?1 AND purged_at IS NULL AND (removed_reason IS NULL OR removed_reason <> 'privacy')",
        params![item_id, now, now + PRIVACY_BYTES_GRACE_MS],
    )?;
    // The served text a key_leak flag quoted goes now (it may hold the data).
    tx.execute("UPDATE admin_flags SET detail = json_set(detail, '$.served', 'purged') WHERE item_id = ?1 AND kind = 'key_leak' AND json_valid(detail) AND json_extract(detail, '$.served') IS NOT NULL", [item_id])?;
    // Its own table, and only what the admin designated and previewed.
    tx.execute("INSERT OR IGNORE INTO privacy_blocks (kind, hash, item_id, at, by, qhash) SELECT 'match', j.value ->> '$.h', ?1, ?2, ?3, j.value ->> '$.q' FROM json_each(?4) j", params![item_id, now, admin_id, json!(pairs).to_string()])?;
    // The selected questions' stem bands (keyed bands and keyed token set, never the words), so a later near-rewording is flagged for the admin. Only for
    // blocks that cover the stem.
    tx.execute(
        "INSERT OR IGNORE INTO privacy_bands (band, qhash, tokens, item_id, at)
         SELECT b.value, iq.match_hash, iq.tok_sk, ?1, ?2 FROM items i JOIN item_questions iq ON iq.publish_id = i.current_publish_id, json_each(COALESCE(iq.kbands, '[]')) b
          WHERE i.id = ?1 AND iq.match_hash IN (SELECT value FROM json_each(?3))",
        params![item_id, now, json!(stem_hashes).to_string()],
    )?;
    // ... and the bands of each blocked PART stem.
    tx.execute(
        "INSERT OR IGNORE INTO privacy_bands (band, qhash, tokens, item_id, at)
         SELECT b.value, iq.match_hash, NULL, ?1, ?2 FROM items i JOIN item_questions iq ON iq.publish_id = i.current_publish_id
           JOIN item_part_keys pk ON pk.publish_id = iq.publish_id AND pk.pos = iq.pos, json_each(COALESCE(pk.kbands, '[]')) b
          WHERE i.id = ?1 AND pk.key IN (SELECT value FROM json_each(?3))",
        params![item_id, now, json!(stem_keys).to_string()],
    )?;
    tx.execute("INSERT OR IGNORE INTO privacy_blocks (kind, hash, item_id, at, by, bytes_purge_after) SELECT 'blob', value, ?1, ?2, ?3, ?5 FROM json_each(?4)", params![item_id, now, admin_id, json!(bh).to_string(), now + PRIVACY_BYTES_GRACE_MS])?;
    // Exactly the previewed items are held (any state, so an undelete or put-back keeps them held).
    tx.execute(
        &format!("INSERT OR IGNORE INTO item_holds (item_id, reason, at, by, source, detail) SELECT j.value, 'student_data', ?1, ?2, 'privacy', {} FROM json_each(?3) j", hold_snapshot("j.value", "?4")),
        params![now, admin_id, json!(affected).to_string(), json!(mh).to_string()],
    )?;
    // The purged item's author's pending holds on those images go too; never another account's.
    tx.execute("DELETE FROM pending_blobs WHERE sha256 IN (SELECT value FROM json_each(?1)) AND uploader_id = (SELECT author_id FROM items WHERE id = ?2)", params![json!(bh).to_string(), item_id])?;
    tx.execute("UPDATE reports SET resolved_at = ?2, resolved_by = ?3, action = 'purged' WHERE item_id = ?1 AND resolved_at IS NULL", params![item_id, now, admin_id])?;
    tx.execute("DELETE FROM item_holds WHERE item_id = ?1", [item_id])?;
    actor_audit(&tx, now, &Actor { id: Some(admin_id), kind: "admin" }, "privacy.purge", item_id, None, Some(json!({ "questions": mh.len(), "images": bh.len(), "held": plan.affected.len() })), None, None)?;
    // What is left to do (the copies past the list, the bodies' move) is a backlog from the claim on, in the same batch: a failure part way is finished
    // by the cron or by the admin's retry, never left with student data public. Resolved once it is all done.
    tx.execute(
        "INSERT OR IGNORE INTO admin_flags (id, kind, item_id, detail, dedup, at) SELECT ?1, 'privacy_backlog', ?2, ?3, ?4, ?5
         WHERE EXISTS (SELECT 1 FROM items WHERE id = ?2 AND removed_reason = 'privacy' AND purged_at IS NULL AND removed_at = ?5)",
        params![new_id(), item_id, json!({ "match": mh, "blob": bh, "batch": AFFECTED_LIST, "by": admin_id }).to_string(), format!("privacy_backlog:{item_id}:{now}"), now],
    )?;
    tx.commit()?;
    if upd != 1 {
        // The admin's retry of a purge that stopped part way finishes it: idempotent.
        let open: Option<String> = x
            .conn
            .query_row(
                "SELECT f.id FROM admin_flags f JOIN items i ON i.id = f.item_id WHERE f.kind = 'privacy_backlog' AND f.item_id = ?1 AND i.removed_reason = 'privacy' AND i.purged_at IS NULL LIMIT 1",
                [item_id],
                |r| r.get(0),
            )
            .optional()?;
        if open.is_none() {
            return Ok(false);
        }
        privacy_finish(x, store, item_id, &mh, &bh, Some(admin_id), 10)?;
        return Ok(true);
    }
    let others: Vec<String> = plan.affected.iter().map(|a| a.item_id.clone()).filter(|i| i != item_id).collect();
    notify_held(x, &others, "student_data")?;
    // More than the listed affected items: the rest are held in batches now; what a request cannot finish the daily cron continues.
    privacy_finish(x, store, item_id, &mh, &bh, Some(admin_id), if plan.affected_total > plan.affected.len() as i64 { 10 } else { 1 })?;
    Ok(true)
}

/// Puts a privacy-removed item back within its grace window (an unblock of its last own block, or an upheld appeal): its own blocks are lifted, holds only
/// they caused are released, its bodies return from quarantine and it is what it was before (live, or the removal it replaced).
pub fn privacy_restore(x: &PoolCtx, store: &BlobStore, actor: &Actor, item_id: &str, note: Option<&str>) -> Res<bool> {
    let now = x.now;
    // Within the grace window, or later while an appeal is open (the purge waits for it).
    let restorable = format!("removed_reason = 'privacy' AND purged_at IS NULL AND (purge_after > ?2 OR {})", open_appeal_sql("items.id"));
    let it: Option<Option<String>> = x.conn.query_row(&format!("SELECT prior_removed_reason FROM items WHERE id = ?1 AND {restorable}"), params![item_id, now], |r| r.get(0)).optional()?;
    let Some(prior) = it else { return Ok(false) };
    let tx = x.conn.unchecked_transaction()?;
    let upd = tx.execute(
        &format!(
            "UPDATE items SET removed_reason = prior_removed_reason, purge_after = prior_purge_after,
               removed_at = CASE WHEN prior_removed_reason IS NULL THEN NULL ELSE removed_at END,
               quarantined_at = CASE WHEN prior_removed_reason = 'takedown' THEN quarantined_at ELSE NULL END,
               prior_removed_reason = NULL, prior_purge_after = NULL, updated_at = ?2
             WHERE id = ?1 AND {restorable}"
        ),
        params![item_id, now],
    )?;
    tx.execute("DELETE FROM privacy_blocks WHERE item_id = ?1 AND changes() = 1", [item_id])?;
    tx.execute("DELETE FROM privacy_bands WHERE qhash NOT IN (SELECT qhash FROM privacy_blocks WHERE qhash IS NOT NULL)", [])?;
    // Restored: its open appeals are answered.
    tx.execute(
        "UPDATE admin_flags SET resolved_at = ?2, resolved_by = ?3 WHERE kind = 'appeal' AND item_id = ?1 AND resolved_at IS NULL AND EXISTS (SELECT 1 FROM items WHERE id = ?1 AND removed_reason IS NOT 'privacy')",
        params![item_id, now, actor.id.unwrap_or("system")],
    )?;
    actor_audit(&tx, now, actor, "privacy.restore", item_id, None, None, note, None)?;
    tx.commit()?;
    if upd != 1 {
        return Ok(false);
    }
    if prior.as_deref() != Some("takedown") {
        move_bodies(x, store, item_id, false)?;
    }
    release_orphan_holds(x, "privacy", "student_data")?;
    Ok(true)
}

/// Copies per `purge_held_copies` call.
pub const COPIES_PER_CALL: usize = 50;

#[derive(Debug, PartialEq)]
pub struct HeldCopies {
    pub items: Vec<String>,
    pub total: i64,
    pub removed: i64,
    pub remaining: i64,
}

/// Copies held because they use student data an admin confirmed on item `item_id`: the admin previews them (no `confirm`), then removes them in bulk, each
/// quarantined for the grace window like the original, restorable by an unblock or a privacy restore meanwhile. At most COPIES_PER_CALL per call.
pub fn purge_held_copies(x: &PoolCtx, store: &BlobStore, admin_id: &str, item_id: &str, confirm: bool) -> Res<HeldCopies> {
    let now = x.now;
    let held = format!(
        "SELECT DISTINCT h.item_id FROM item_holds h JOIN items i ON i.id = h.item_id
          WHERE h.source = 'privacy' AND i.id <> ?1 AND i.purged_at IS NULL AND (i.removed_reason IS NULL OR i.removed_reason <> 'privacy')
            AND (EXISTS (SELECT 1 FROM item_publishes p JOIN item_questions iq ON iq.publish_id = p.publish_id JOIN privacy_blocks pb ON pb.kind = 'match' AND {}
                          WHERE p.item_id = i.id AND pb.item_id = ?1)
                 OR {})",
        q_key_eq("iq", "pb.hash"),
        super::served::item_serves_blob("i.id", |sha| format!("EXISTS (SELECT 1 FROM privacy_blocks pb WHERE pb.kind = 'blob' AND pb.hash = {sha} AND pb.item_id = ?1)"))
    );
    let total: i64 = x.conn.query_row(&format!("SELECT COUNT(*) FROM ({held})"), [item_id], |r| r.get(0))?;
    let mut st = x.conn.prepare(&format!("{held} ORDER BY h.item_id LIMIT ?2"))?;
    let items: Vec<String> = st.query_map(params![item_id, COPIES_PER_CALL as i64], |r| r.get(0))?.collect::<Result<_, _>>()?;
    drop(st);
    if !confirm {
        return Ok(HeldCopies { items, total, removed: 0, remaining: total });
    }
    let mut removed = 0;
    for id in &items {
        let tx = x.conn.unchecked_transaction()?;
        let n = tx.execute(
            "UPDATE items SET prior_removed_reason = removed_reason, prior_purge_after = purge_after, removed_at = ?2, removed_reason = 'privacy',
               purge_after = ?3, quarantined_at = COALESCE(quarantined_at, ?2), updated_at = ?2
             WHERE id = ?1 AND purged_at IS NULL AND (removed_reason IS NULL OR removed_reason <> 'privacy')",
            params![id, now, now + PRIVACY_BYTES_GRACE_MS],
        )?;
        actor_audit(&tx, now, &Actor { id: Some(admin_id), kind: "admin" }, "privacy.purge_copy", id, None, Some(json!({ "source_item": item_id })), None, Some("changes() = 1"))?;
        tx.commit()?;
        if n != 1 {
            continue;
        }
        move_bodies(x, store, id, true)?;
        removed += 1;
    }
    Ok(HeldCopies { items, total, removed, remaining: (total - removed).max(0) })
}

#[derive(Debug, PartialEq, Clone, Copy)]
pub enum Decided {
    Ok,
    NotFound,
    OverLimits,
}

/// Reverses an admin removal (within its 14 days, or later while an appeal is open), back to what it replaced (live, or the author's own deletion). Holds
/// stay; a privacy block on its content adds one; going live is subject to the author's quota and question cap, like an undelete.
pub fn admin_unremove(x: &PoolCtx, item_id: &str, actor: &Actor, note: Option<&str>) -> Res<Decided> {
    let now = x.now;
    let cond = format!("removed_reason = 'admin' AND purged_at IS NULL AND (purge_after > ?2 OR {})", open_appeal_sql("items.id"));
    let author: Option<String> = x.conn.query_row(&format!("SELECT author_id FROM items WHERE id = ?1 AND {cond}"), params![item_id, now], |r| r.get(0)).optional()?;
    let Some(author) = author else { return Ok(Decided::NotFound) };
    let quota = x.limit_of(&author, "storage_bytes")?;
    let cap = x.limit_of(&author, "shared_questions")?;
    let tx = x.conn.unchecked_transaction()?;
    let upd = tx.execute(
        &format!(
            "UPDATE items SET removed_reason = prior_removed_reason, purge_after = prior_purge_after,
               removed_at = CASE WHEN prior_removed_reason IS NULL THEN NULL ELSE removed_at END,
               prior_removed_reason = NULL, prior_purge_after = NULL, updated_at = ?2
             WHERE id = ?3 AND author_id = ?1 AND {}
               AND (prior_removed_reason IS NOT NULL OR ({} <= ?4 AND {} + question_count <= ?5))",
            cond.replace("items.id", "?3"),
            usage_sql(Some("?3")),
            shared_sql("?1", "?3")
        ),
        params![author, now, item_id, quota, cap],
    )?;
    actor_audit(&tx, now, actor, "item.unremove", item_id, Some(&author), None, note, Some("changes() = 1"))?;
    tx.execute(
        &format!(
            "INSERT OR IGNORE INTO item_holds (item_id, reason, at, by, source)
             SELECT ?1, 'student_data', ?2, ?3, 'privacy' WHERE changes() = 1 AND (EXISTS (
               SELECT 1 FROM item_publishes p WHERE p.item_id = ?1 AND (
                 EXISTS (SELECT 1 FROM item_questions iq JOIN privacy_blocks pb ON pb.kind = 'match' AND {} WHERE iq.publish_id = p.publish_id))
               ) OR {})",
            q_key_eq("iq", "pb.hash"),
            super::served::item_serves_blob("?1", |sha| format!("EXISTS (SELECT 1 FROM privacy_blocks pb WHERE pb.kind = 'blob' AND pb.hash = {sha})"))
        ),
        params![item_id, now, actor.id],
    )?;
    tx.commit()?;
    if upd != 1 {
        return Ok(Decided::OverLimits);
    }
    x.conn.execute("UPDATE admin_flags SET resolved_at = ?2, resolved_by = ?3 WHERE kind = 'appeal' AND item_id = ?1 AND resolved_at IS NULL", params![item_id, now, actor.id.unwrap_or("system")])?;
    Ok(Decided::Ok)
}

/// The admin's decision on an appealed removal: "restore" puts it back (a privacy removal or an admin removal); "purge" confirms it, and the purge goes
/// ahead at its date. Either resolves the item's open appeals.
pub fn decide_appeal(x: &PoolCtx, store: &BlobStore, item_id: &str, restore: bool, actor: &Actor, note: Option<&str>) -> Res<Decided> {
    let now = x.now;
    let it: Option<(Option<String>, bool)> = x
        .conn
        .query_row("SELECT removed_reason, EXISTS (SELECT 1 FROM item_holds h WHERE h.item_id = items.id) FROM items WHERE id = ?1 AND purged_at IS NULL", [item_id], |r| Ok((r.get(0)?, r.get(1)?)))
        .optional()?;
    let Some((reason, held)) = it else { return Ok(Decided::NotFound) };
    let mut res = Decided::Ok;
    if restore {
        match (reason.as_deref(), held) {
            (Some("privacy"), _) => res = if privacy_restore(x, store, actor, item_id, note)? { Decided::Ok } else { Decided::NotFound },
            (Some("admin"), _) => res = admin_unremove(x, item_id, actor, note)?,
            // A held item (copies too): its holds are released, an audited admin action.
            (None, true) => {
                x.conn.execute("DELETE FROM item_holds WHERE item_id = ?1", [item_id])?;
            }
            _ => res = Decided::NotFound,
        }
    } else if reason.is_none() && held {
        // Confirming a hold on a copy: removed like the original (quarantined, purged at its date).
        x.conn.execute(
            "UPDATE items SET prior_removed_reason = removed_reason, prior_purge_after = purge_after, removed_at = ?2,
               removed_reason = CASE WHEN EXISTS (SELECT 1 FROM item_holds h WHERE h.item_id = ?1 AND h.source = 'privacy') THEN 'privacy' ELSE 'admin' END,
               purge_after = ?3, updated_at = ?2 WHERE id = ?1 AND removed_at IS NULL",
            params![item_id, now, now + PRIVACY_BYTES_GRACE_MS],
        )?;
        move_bodies(x, store, item_id, true)?;
    }
    if res != Decided::Ok {
        return Ok(res);
    }
    let decision = if restore { "restore" } else { "purge" };
    let tx = x.conn.unchecked_transaction()?;
    tx.execute("UPDATE admin_flags SET resolved_at = ?2, resolved_by = ?3 WHERE kind = 'appeal' AND item_id = ?1 AND resolved_at IS NULL", params![item_id, now, actor.id.unwrap_or("system")])?;
    // Decided: this removal cannot be appealed again.
    tx.execute("UPDATE items SET appeal_decided_at = ?2, appeal_decision = ?3 WHERE id = ?1", params![item_id, now, decision])?;
    actor_audit(&tx, now, actor, &format!("appeal.{decision}"), item_id, None, None, note, None)?;
    tx.commit()?;
    Ok(Decided::Ok)
}

/// Daily: open appeals remind the admin after a week; past APPEAL_MAX_MS the item is restored, except privacy / DMCA / student-data removals, which are
/// escalated and never restored by default.
pub fn appeals_due(x: &PoolCtx, store: &BlobStore) -> Res<(usize, usize)> {
    let now = x.now;
    let mut st = x.conn.prepare(
        "SELECT f.item_id, MIN(f.at) AS since FROM admin_flags f JOIN items i ON i.id = f.item_id
          WHERE f.kind = 'appeal' AND f.resolved_at IS NULL AND i.purged_at IS NULL AND f.at <= ?1 GROUP BY f.item_id LIMIT 200",
    )?;
    let open: Vec<(String, i64)> = st.query_map([now - APPEAL_REMINDER_MS], |r| Ok((r.get(0)?, r.get(1)?)))?.collect::<Result<_, _>>()?;
    drop(st);
    let (mut reminded, mut restored) = (0, 0);
    for (item_id, since) in open {
        let live: Option<(Option<String>, bool, bool)> = x
            .conn
            .query_row(
                "SELECT removed_reason, EXISTS (SELECT 1 FROM item_holds h WHERE h.item_id = items.id),
                        EXISTS (SELECT 1 FROM item_holds h WHERE h.item_id = items.id AND (h.source = 'privacy' OR h.source LIKE 'dmca:%' OR h.reason = 'student_data'))
                   FROM items WHERE id = ?1",
                [&item_id],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
            )
            .optional()?;
        // Nothing left to decide (the item was restored or its holds lifted meanwhile): the appeal is answered.
        let Some((reason, held, sensitive_hold)) = live.filter(|l| !(l.0.is_none() && !l.1)) else {
            x.conn.execute("UPDATE admin_flags SET resolved_at = ?2, resolved_by = 'system' WHERE kind IN ('appeal', 'appeal_overdue', 'appeal_escalated') AND item_id = ?1 AND resolved_at IS NULL", params![item_id, now])?;
            continue;
        };
        let _ = held;
        let sensitive = matches!(reason.as_deref(), Some("privacy" | "takedown")) || sensitive_hold;
        if since <= now - APPEAL_MAX_MS {
            if sensitive {
                reminded += x.conn.execute(
                    "INSERT OR IGNORE INTO admin_flags (id, kind, item_id, detail, dedup, at) VALUES (?1, 'appeal_escalated', ?2, ?3, ?4, ?5)",
                    params![new_id(), item_id, json!({ "open_since": since, "removed_reason": reason }).to_string(), format!("appeal_escalated:{item_id}:{since}"), now],
                )?;
            } else if decide_appeal(x, store, &item_id, true, &Actor { id: None, kind: "system" }, Some("appeal unanswered for 60 days: restored"))? == Decided::Ok {
                restored += 1;
                x.conn.execute(
                    "INSERT OR IGNORE INTO admin_flags (id, kind, item_id, detail, dedup, at) VALUES (?1, 'appeal_auto_restored', ?2, ?3, ?4, ?5)",
                    params![new_id(), item_id, json!({ "open_since": since }).to_string(), format!("appeal_auto_restored:{item_id}:{since}"), now],
                )?;
            }
            if !sensitive {
                continue;
            }
        }
        // One flag per open appeal, its week count and date brought up to date each week.
        let weeks = (now - since).div_euclid(7 * DAY_MS);
        let mut detail = json!({ "open_since": since, "weeks": weeks });
        if !sensitive {
            detail["restores_at"] = json!(since + APPEAL_MAX_MS);
        }
        reminded += x.conn.execute(
            "INSERT INTO admin_flags (id, kind, item_id, detail, dedup, at) VALUES (?1, 'appeal_overdue', ?2, ?3, ?4, ?5)
             ON CONFLICT (dedup) DO UPDATE SET detail = excluded.detail, at = excluded.at, resolved_at = NULL, resolved_by = NULL
              WHERE CAST(admin_flags.detail ->> '$.weeks' AS INTEGER) < CAST(excluded.detail ->> '$.weeks' AS INTEGER)",
            params![new_id(), item_id, detail.to_string(), format!("appeal_overdue:{item_id}:{since}"), now],
        )?;
    }
    Ok((reminded, restored))
}

#[cfg(test)]
#[path = "modprivacy_tests.rs"]
mod tests;
