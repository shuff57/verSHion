//! Put-back, court uphold and its reversal (worker/src/pool/moderation.ts `restore`, `restoreFinish`, `putbackFinish`, `releaseRestoredHolds`,
//! `upholdTakedown`, `reverseUphold`).
use super::blocking::release_orphan_holds;
use super::ctx::{AuditEntry, PoolCtx, Res, audit};
use super::modgc::{Who, void_evidence};
use super::modlineage::move_bodies;
use super::modpurge::{Actor, purge_item, uphold_ghost};
use super::sqlfrag::{DAY_MS, HOUR_MS, new_id};
use crate::blobs::BlobStore;
use rusqlite::{OptionalExtension, params};
use serde_json::json;

/// Blocklist holds of actions put back or withdrawn, released by the cron (bounded).
pub fn release_restored_holds(x: &PoolCtx) -> Res<usize> {
    let mut st = x.conn.prepare("SELECT DISTINCT a.id FROM dmca_actions a JOIN item_holds h ON h.source = 'dmca:' || a.id AND h.reason = 'blocklist' WHERE a.restored_at IS NOT NULL LIMIT 50")?;
    let ids: Vec<String> = st.query_map([], |r| r.get(0))?.collect::<Result<_, _>>()?;
    drop(st);
    for id in &ids {
        release_orphan_holds(x, &format!("dmca:{id}"), "blocklist")?;
    }
    Ok(ids.len())
}

/// Copy holds an action placed that remain after its put-back or withdrawal: a flag offers their release.
fn flag_left_holds(x: &PoolCtx, action_id: &str, item_id: &str) -> Res<()> {
    let left: i64 = x.conn.query_row("SELECT COUNT(*) FROM item_holds WHERE source = ?1", [format!("dmca:{action_id}")], |r| r.get(0))?;
    if left > 0 {
        x.conn.execute(
            "INSERT OR IGNORE INTO admin_flags (id, kind, item_id, detail, dedup, at) VALUES (?1, 'dmca_copy', ?2, ?3, ?4, ?5)",
            params![new_id(), item_id, json!({ "action_id": action_id, "held_copies": left, "hint": "POST /admin/dmca/release-copies" }).to_string(), format!("putback:{action_id}"), x.now],
        )?;
    }
    Ok(())
}

/// Put-back: bodies return from quarantine, the item shows again, the strike and blocklist entries are lifted. False when another put-back or a purge
/// won the claim.
pub fn restore(x: &PoolCtx, store: &BlobStore, action_id: &str, actor: &Actor, outcome: &str) -> Res<bool> {
    // Claim first: exactly one put-back wins, and clearing purge_after stops a purge from claiming the item while its bodies are moved back.
    let tx = x.conn.unchecked_transaction()?;
    let won = tx.execute("UPDATE dmca_actions SET restored_at = ?2, outcome = ?3 WHERE id = ?1 AND restored_at IS NULL AND purged_at IS NULL", params![action_id, x.now, outcome])?;
    let cleared = tx.execute(
        "UPDATE items SET purge_after = NULL WHERE id = (SELECT item_id FROM dmca_actions WHERE id = ?1) AND removed_reason = 'takedown' AND purged_at IS NULL AND changes() = 1",
        [action_id],
    )?;
    tx.commit()?;
    if won != 1 {
        return Ok(false);
    }
    restore_finish(x, store, action_id, actor, outcome, cleared == 1)
}

/// The steps of a put-back after its claim, each idempotent, so a put-back that failed part way is finished by the cron (`putback_finish`) instead of
/// being stuck with the item removed. `cleared`: the claim cleared the item's purge (it is still the taken-down item, not purged).
fn restore_finish(x: &PoolCtx, store: &BlobStore, action_id: &str, actor: &Actor, outcome: &str, cleared: bool) -> Res<bool> {
    let now = x.now;
    let (item_id, prev_reason, prev_purge_after, disabled_at): (String, Option<String>, Option<i64>, i64) = x
        .conn
        .query_row("SELECT item_id, prev_removed_reason, prev_purge_after, disabled_at FROM dmca_actions WHERE id = ?1", [action_id], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)))?;
    let audit_restore = |tx: &rusqlite::Connection, back_to: &str| -> rusqlite::Result<usize> {
        audit(tx, &AuditEntry { actor_id: actor.id, actor_kind: actor.kind, action: "dmca.restore", item_id: Some(&item_id), target_user_id: None, detail: Some(json!({ "action_id": action_id, "outcome": outcome, "back_to": back_to })), note: None }, now, None)
    };
    // An item purged before its notice (a ghost takedown): nothing to move back; the put-back or withdrawal voids the pending strike, lifts the blocks and
    // the holds this action caused, and returns the tombstone to what it was.
    let ghost: bool = x.conn.query_row("SELECT EXISTS (SELECT 1 FROM items WHERE id = ?1 AND purged_at IS NOT NULL AND purged_at < ?2)", params![item_id, disabled_at], |r| r.get(0))?;
    if !cleared && ghost {
        let tx = x.conn.unchecked_transaction()?;
        tx.execute("UPDATE items SET removed_reason = COALESCE(?2, removed_reason) WHERE id = ?1 AND removed_reason = 'takedown'", params![item_id, prev_reason])?;
        tx.execute("UPDATE strikes SET voided_at = ?2 WHERE action_id = ?1 AND voided_at IS NULL", params![action_id, now])?;
        tx.execute("UPDATE blocked_hashes SET lifted_at = ?2 WHERE action_id = ?1 AND lifted_at IS NULL", params![action_id, now])?;
        audit_restore(&tx, prev_reason.as_deref().unwrap_or("deleted"))?;
        tx.commit()?;
        release_orphan_holds(x, &format!("dmca:{action_id}"), "blocklist")?;
        flag_left_holds(x, action_id, &item_id)?;
        return Ok(true);
    }
    if !cleared {
        // The item was purged first: the action is not a put-back after all.
        x.conn.execute("UPDATE dmca_actions SET restored_at = NULL, outcome = NULL WHERE id = ?1 AND restored_at = ?2", params![action_id, now])?;
        return Ok(false);
    }
    let written = move_bodies(x, store, &item_id, false)?;
    let was_deleted = matches!(prev_reason.as_deref(), Some("author_deleted" | "admin" | "account_deleted"));
    let tx = x.conn.unchecked_transaction()?;
    // The author's undo window resumes with what was left of it when the takedown began (never purged in the same run).
    let new_purge = was_deleted.then(|| now + DAY_MS.max(prev_purge_after.unwrap_or(now) - disabled_at));
    tx.execute(
        "UPDATE items SET quarantined_at = NULL, updated_at = ?2,
            removed_at = CASE WHEN ?3 = 1 THEN removed_at ELSE NULL END,
            removed_reason = CASE WHEN ?3 = 1 THEN ?5 ELSE NULL END,
            purge_after = CASE WHEN ?3 = 1 THEN ?4 ELSE NULL END
          WHERE id = ?1 AND removed_reason = 'takedown' AND purged_at IS NULL",
        params![item_id, now, was_deleted as i64, new_purge, prev_reason],
    )?;
    tx.execute("UPDATE strikes SET voided_at = ?2 WHERE action_id = ?1 AND voided_at IS NULL", params![action_id, now])?;
    tx.execute("UPDATE blocked_hashes SET lifted_at = ?2 WHERE action_id = ?1 AND lifted_at IS NULL", params![action_id, now])?;
    audit_restore(&tx, if was_deleted { prev_reason.as_deref().unwrap_or("deleted") } else { "live" })?;
    tx.commit()?;
    // Holds that only this action's re-upload blocks caused go with them; copy holds an admin placed stay, flagged so they are offered for release.
    release_orphan_holds(x, &format!("dmca:{action_id}"), "blocklist")?;
    flag_left_holds(x, action_id, &item_id)?;
    // A purge that claimed the item while the bodies were moving (e.g. a privacy purge) wins: anything this put-back wrote is removed, so no body outlives
    // its rows.
    let purged: Option<i64> = x.conn.query_row("SELECT purged_at FROM items WHERE id = ?1", [&item_id], |r| r.get(0)).optional()?.flatten();
    if purged.is_some() {
        for k in &written {
            let _ = store.delete(k);
        }
        return Ok(false);
    }
    Ok(true)
}

/// Put-backs claimed (restored_at set) whose item is still the taken-down one an hour later: the remaining steps are run again (idempotent).
pub fn putback_finish(x: &PoolCtx, store: &BlobStore) -> Res<usize> {
    let mut st = x.conn.prepare(
        "SELECT a.id, a.outcome, i.purged_at IS NULL FROM dmca_actions a JOIN items i ON i.id = a.item_id
          WHERE a.restored_at IS NOT NULL AND a.restored_at < ?1 AND i.removed_reason = 'takedown'
            AND (i.purged_at IS NULL OR i.purged_at < a.disabled_at)
            AND NOT EXISTS (SELECT 1 FROM dmca_actions b WHERE b.item_id = a.item_id AND b.disabled_at > a.disabled_at) LIMIT 50",
    )?;
    let due: Vec<(String, Option<String>, bool)> = st.query_map([x.now - HOUR_MS], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))?.collect::<Result<_, _>>()?;
    drop(st);
    for (id, outcome, live) in &due {
        restore_finish(x, store, id, &Actor { id: None, kind: "system" }, outcome.as_deref().unwrap_or("put_back"), *live)?;
    }
    Ok(due.len())
}

/// Court action resolved for the complainant: the material stays down and is purged; the strike stands.
pub fn uphold_takedown(x: &PoolCtx, store: &BlobStore, admin_id: &str, action_id: &str) -> Res<bool> {
    // A ghost takedown (the item purged before the notice) is upheld by the same claim-and-finish as at the end of its window.
    let ghost: bool = x.conn.query_row(
        "SELECT EXISTS (SELECT 1 FROM dmca_actions a JOIN items i ON i.id = a.item_id WHERE a.id = ?1 AND i.purged_at IS NOT NULL AND i.purged_at < a.disabled_at)",
        [action_id],
        |r| r.get(0),
    )?;
    if ghost {
        return uphold_ghost(x, action_id, "court_upheld", true);
    }
    let tx = x.conn.unchecked_transaction()?;
    let n = tx.execute("UPDATE dmca_actions SET outcome = 'court_upheld' WHERE id = ?1 AND restored_at IS NULL AND purged_at IS NULL", [action_id])?;
    tx.execute("UPDATE items SET purge_after = ?2 WHERE id = (SELECT item_id FROM dmca_actions WHERE id = ?1) AND removed_reason = 'takedown' AND purged_at IS NULL AND changes() = 1", params![action_id, x.now])?;
    tx.commit()?;
    if n != 1 {
        return Ok(false);
    }
    let item_id: String = x.conn.query_row("SELECT item_id FROM dmca_actions WHERE id = ?1", [action_id], |r| r.get(0))?;
    purge_item(x, store, &item_id, &Actor { id: Some(admin_id), kind: "admin" }, "court_upheld")
}

/// An admin's reversal of an UPHELD takedown (a court decision for the author, or an uphold that should not have happened): the strike is voided, the
/// item's voided evidence restored, its blocks lifted and the copies it held released. Content already purged stays purged.
pub fn reverse_uphold(x: &PoolCtx, admin_id: &str, action_id: &str) -> Res<bool> {
    let now = x.now;
    let claim = x.conn.execute("UPDATE dmca_actions SET restored_at = ?2, outcome = 'reversed', uphold_pending = 0 WHERE id = ?1 AND purged_at IS NOT NULL AND restored_at IS NULL", params![action_id, now])?;
    if claim != 1 {
        return Ok(false);
    }
    let item_id: String = x.conn.query_row("SELECT item_id FROM dmca_actions WHERE id = ?1", [action_id], |r| r.get(0))?;
    let tx = x.conn.unchecked_transaction()?;
    tx.execute("UPDATE strikes SET voided_at = ?2 WHERE action_id = ?1 AND voided_at IS NULL", params![action_id, now])?;
    tx.execute("UPDATE blocked_hashes SET lifted_at = ?2 WHERE action_id = ?1 AND lifted_at IS NULL", params![action_id, now])?;
    let voided: bool = tx.query_row("SELECT EXISTS (SELECT 1 FROM void_items WHERE item_id = ?1 AND reason = 'takedown_upheld')", [&item_id], |r| r.get(0))?;
    if voided {
        void_evidence(&tx, Who::Item(&item_id), now, true, Some(admin_id), "takedown_reversed", None)?;
    }
    tx.execute("DELETE FROM item_holds WHERE source = ?1", [format!("dmca:{action_id}")])?;
    audit(&tx, &AuditEntry { actor_id: Some(admin_id), actor_kind: "admin", action: "dmca.reverse", item_id: Some(&item_id), target_user_id: None, detail: Some(json!({ "action_id": action_id })), note: None }, now, None)?;
    tx.commit()?;
    Ok(true)
}

#[cfg(test)]
#[path = "modrestore_tests.rs"]
mod tests;
