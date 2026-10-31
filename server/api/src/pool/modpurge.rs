//! Purge and the ghost uphold (worker/src/pool/moderation.ts `purgeItem`, `ghostUpholds`, `upholdGhost`): the item row stays as a tombstone; every
//! publish's bodies and derived rows go; an upheld takedown voids evidence, upholds its strike and holds the copies.
use super::ctx::{AuditEntry, PoolCtx, Res, audit};
use super::licensecheck::license_check_of;
use super::modgc::{Who, delete_keys, gc_blobs, repeat_infringer_check, void_evidence};
use super::modlineage::{LineageOpts, all_lineage_copies, ghost_copies, should_void_evidence};
use super::moderation::QUARANTINE_MS;
use super::poolio::notify_held;
use super::publishdel::delete_publishes;
use super::served::{rightful_exempts, served_blobs_of};
use super::sqlfrag::{in_q_keys_of, new_id};
use crate::blobs::BlobStore;
use rusqlite::{OptionalExtension, params};
use serde_json::json;

pub struct Actor<'a> {
    pub id: Option<&'a str>,
    /// "admin" or "system".
    pub kind: &'a str,
}

const COPY_HINT: &str = "name the rightful item (POST /admin/pool/evidence with replaced_by) or release the copies";

/// Holds the copies of an upheld takedown for review and raises the one flag per action.
fn hold_copies(x: &PoolCtx, action_id: &str, item_id: &str, copies: &[String]) -> Res<()> {
    if copies.is_empty() {
        return Ok(());
    }
    x.conn.execute("INSERT OR IGNORE INTO item_holds (item_id, reason, at, by, source) SELECT value, 'admin', ?1, NULL, ?2 FROM json_each(?3)", params![x.now, format!("dmca:{action_id}"), json!(copies).to_string()])?;
    x.conn.execute(
        "INSERT OR IGNORE INTO admin_flags (id, kind, item_id, detail, dedup, at) VALUES (?1, 'dmca_copy', ?2, ?3, ?4, ?5)",
        params![new_id(), item_id, json!({ "action_id": action_id, "upheld": true, "held_copies": copies.len(), "hint": COPY_HINT }).to_string(), format!("upheld:{action_id}"), x.now],
    )?;
    Ok(())
}

/// Purge: every publish's bodies and derived rows; the items row stays as a tombstone. False when the purge is not due (or a put-back claimed it first).
pub fn purge_item(x: &PoolCtx, store: &BlobStore, item_id: &str, actor: &Actor, why: &str) -> Res<bool> {
    let now = x.now;
    // The publish the copies are compared with: the item's current one, read before the claim clears the pointer, or after a failed run from the
    // publish rows themselves.
    let before: Option<String> = x
        .conn
        .query_row("SELECT COALESCE((SELECT current_publish_id FROM items WHERE id = ?1), (SELECT publish_id FROM item_publishes WHERE item_id = ?1 AND status = 'current'))", [item_id], |r| r.get(0))
        .optional()?
        .flatten();
    // Claimed first, and only an item whose purge is due: a put-back clears purge_after when it claims its action, so the two can never both proceed. A
    // purge claimed earlier whose batch failed is claimed again and finished.
    let claimed = x.conn.execute(
        "UPDATE items SET purged_at = COALESCE(purged_at, ?2), current_publish_id = NULL, prev_publish_id = NULL, purge_after = NULL
          WHERE id = ?1 AND removed_at IS NOT NULL
            AND ((purged_at IS NULL AND purge_after IS NOT NULL AND purge_after <= ?2)
                 OR (purged_at IS NOT NULL AND EXISTS (SELECT 1 FROM item_publishes p WHERE p.item_id = items.id)))",
        params![item_id, now],
    )?;
    if claimed != 1 {
        return Ok(false);
    }
    let mut st = x.conn.prepare("SELECT publish_id, full_key, stripped_key FROM item_publishes WHERE item_id = ?1")?;
    let pubs: Vec<(String, String, Option<String>)> = st.query_map([item_id], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))?.collect::<Result<_, _>>()?;
    drop(st);
    let ids: Vec<String> = pubs.iter().map(|p| p.0.clone()).collect();
    let shas: Vec<String> = if ids.is_empty() {
        vec![]
    } else {
        let mut st = x.conn.prepare("SELECT DISTINCT sha256 FROM item_blobs WHERE publish_id IN (SELECT value FROM json_each(?1))")?;
        let v = st.query_map([json!(ids).to_string()], |r| r.get(0))?.collect::<Result<_, _>>()?;
        v
    };
    let tomb: Option<(Option<String>, String)> = x.conn.query_row("SELECT removed_reason, author_id FROM items WHERE id = ?1", [item_id], |r| Ok((r.get(0)?, r.get(1)?))).optional()?;
    let was_takedown = tomb.as_ref().is_some_and(|t| t.0.as_deref() == Some("takedown"));
    // The rightful author's item, if the admin named one on the takedown.
    let act: Option<(String, Option<String>)> = if was_takedown {
        x.conn
            .query_row("SELECT id, rightful_item_id FROM dmca_actions WHERE item_id = ?1 AND restored_at IS NULL AND purged_at IS NULL ORDER BY disabled_at DESC LIMIT 1", [item_id], |r| Ok((r.get(0)?, r.get(1)?)))
            .optional()?
    } else {
        None
    };
    let voids = match (&tomb, was_takedown) {
        (Some((_, author)), true) => should_void_evidence(x, author, act.as_ref().and_then(|a| a.1.as_deref()))?,
        _ => false,
    };
    // Without a rightful item, the copies of the infringing item are held for review rather than left live with no licence or credit at all.
    let orphan_copies: Vec<String> = match &act {
        Some((_, None)) if was_takedown => all_lineage_copies(x, item_id, None, false, None, &LineageOpts { src_publish: Some(before.clone()), ..Default::default() })?.into_iter().map(|c| c.item_id).collect(),
        _ => vec![],
    };
    let tx = x.conn.unchecked_transaction()?;
    if was_takedown {
        if voids {
            void_evidence(&tx, Who::Item(item_id), now, false, actor.id, "takedown_upheld", act.as_ref().and_then(|a| a.1.as_deref()))?;
        }
        if let Some((action_id, _)) = act.as_ref().filter(|_| !orphan_copies.is_empty()) {
            hold_copies(x, action_id, item_id, &orphan_copies)?;
        }
        audit(&tx, &AuditEntry { actor_id: actor.id, actor_kind: actor.kind, action: "evidence.void", item_id: Some(item_id), target_user_id: None, detail: Some(json!({ "why": "takedown_upheld" })), note: None }, now, None)?;
        tx.execute("UPDATE strikes SET upheld_at = ?2 WHERE upheld_at IS NULL AND voided_at IS NULL AND action_id IN (SELECT id FROM dmca_actions WHERE item_id = ?1 AND restored_at IS NULL AND purged_at IS NULL)", params![item_id, now])?;
    }
    delete_publishes(&tx, &ids)?;
    // The tombstone keeps no typed text, and no flag quotes the served text.
    tx.execute("UPDATE items SET title = '[removed]', title_fold = '', subject_fold = '' WHERE id = ?1", [item_id])?;
    tx.execute("UPDATE content_licences SET until_at = ?2 WHERE item_id = ?1 AND until_at IS NULL", params![item_id, now])?;
    tx.execute("DELETE FROM item_offer WHERE item_id = ?1", [item_id])?;
    tx.execute("UPDATE admin_flags SET detail = json_set(detail, '$.served', 'purged') WHERE item_id = ?1 AND kind = 'key_leak' AND json_valid(detail) AND json_extract(detail, '$.served') IS NOT NULL", [item_id])?;
    // The bodies' keys are kept until the delete is confirmed: never an orphan nobody can find.
    let keys: Vec<String> = pubs.iter().flat_map(|p| std::iter::once(p.1.clone()).chain(p.2.clone())).collect();
    tx.execute("INSERT OR IGNORE INTO r2_orphans (key, at) SELECT value, ?2 FROM json_each(?1)", params![json!(keys).to_string(), now])?;
    tx.execute("DELETE FROM question_ids WHERE item_id = ?1", [item_id])?;
    tx.execute("UPDATE dmca_actions SET purged_at = ?2, outcome = COALESCE(outcome, 'purged') WHERE item_id = ?1 AND restored_at IS NULL AND purged_at IS NULL", params![item_id, now])?;
    audit(&tx, &AuditEntry { actor_id: actor.id, actor_kind: actor.kind, action: "item.purge", item_id: Some(item_id), target_user_id: None, detail: Some(json!({ "why": why })), note: None }, now, None)?;
    tx.commit()?;
    // Upheld with a rightful item named: a hold of this action is lifted only when every blocked hash of the action the held item uses is one the
    // rightful item exempts for its author, or, for a lineage hold using none of them, when its questions come from the rightful item; and only if its
    // licence is one its displayed credits now allow. Otherwise it stays held and the admin is told.
    if let Some((action_id, Some(rightful))) = act.as_ref().filter(|_| was_takedown) {
        let sql = format!(
            "SELECT h.item_id FROM item_holds h JOIN items i ON i.id = h.item_id WHERE h.source = ?1 AND
               NOT EXISTS (SELECT 1 FROM blocked_hashes bh WHERE bh.action_id = ?3 AND (
                   (bh.kind = 'match' AND {qk}) OR (bh.kind = 'blob' AND bh.hash IN {sb}))
                 AND NOT {rx})
               AND (EXISTS (SELECT 1 FROM blocked_hashes bh WHERE bh.action_id = ?3 AND (
                      (bh.kind = 'match' AND {qk}) OR (bh.kind = 'blob' AND bh.hash IN {sb})))
                    OR EXISTS (SELECT 1 FROM item_questions q2 WHERE q2.publish_id = i.current_publish_id AND (q2.origin_item_id = ?2 OR instr(q2.chain_json, ?2) > 0)))",
            qk = in_q_keys_of("bh.hash", "i.current_publish_id"),
            sb = served_blobs_of("i.current_publish_id"),
            rx = rightful_exempts("?2", "bh", "i.author_id", "i.current_publish_id")
        );
        let mut st = x.conn.prepare(&sql)?;
        let cand: Vec<String> = st.query_map(params![format!("dmca:{action_id}"), rightful, action_id], |r| r.get(0))?.collect::<Result<_, _>>()?;
        drop(st);
        let checks = license_check_of(x.conn, &cand)?;
        let (bad, ok): (Vec<&String>, Vec<&String>) = cand.iter().partition(|id| checks.get(*id).is_some_and(|c| c.status == "mismatch"));
        if !ok.is_empty() {
            x.conn.execute("DELETE FROM item_holds WHERE source = ?1 AND item_id IN (SELECT value FROM json_each(?2))", params![format!("dmca:{action_id}"), json!(ok).to_string()])?;
        }
        for id in bad {
            x.conn.execute(
                "INSERT OR IGNORE INTO admin_flags (id, kind, item_id, detail, dedup, at) VALUES (?1, 'license_mismatch', ?2, ?3, ?4, ?5)",
                params![new_id(), id, json!({ "action_id": action_id, "check": checks.get(id) }).to_string(), format!("license_mismatch:{action_id}:{id}"), now],
            )?;
        }
    }
    if !keys.is_empty() {
        delete_keys(x, store, &keys)?;
    }
    if !shas.is_empty() {
        gc_blobs(x, store, Some(&shas))?;
    }
    if let (true, Some((_, author))) = (was_takedown, &tomb) {
        repeat_infringer_check(x, author)?;
    }
    Ok(true)
}

/// Ghost takedowns (an item purged before its notice) whose 30-day window has passed with no counter-notice, court action or withdrawal: upheld as an
/// upheld purge is. A batch of 50 a run.
pub fn ghost_upholds(x: &PoolCtx) -> Res<usize> {
    let mut st = x.conn.prepare(
        "SELECT a.id FROM dmca_actions a JOIN items i ON i.id = a.item_id
          WHERE i.purged_at IS NOT NULL AND i.purged_at < a.disabled_at
            AND ((a.counter_notice_id IS NULL AND a.court_action_at IS NULL AND a.restored_at IS NULL AND a.purged_at IS NULL AND a.author_notified_at <= ?1)
                 OR a.uphold_pending = 1) LIMIT 50",
    )?;
    let due: Vec<String> = st.query_map([x.now - QUARANTINE_MS], |r| r.get(0))?.collect::<Result<_, _>>()?;
    drop(st);
    let mut n = 0;
    for id in &due {
        if uphold_ghost(x, id, "upheld", false)? {
            n += 1;
        }
    }
    Ok(n)
}

/// The uphold of a ghost takedown (the 30-day window over, or a court decision for the complainant): one claim in one statement, then an idempotent
/// finish the cron resumes until it clears `uphold_pending`: evidence void (unless the author is erased and no rightful item is named), the strike
/// upheld, every lineage copy held (paged), the repeat-infringer check. `court`: the court decided (the court action must exist).
pub fn uphold_ghost(x: &PoolCtx, action_id: &str, outcome: &str, court: bool) -> Res<bool> {
    let now = x.now;
    x.conn.execute(
        &format!(
            "UPDATE dmca_actions SET purged_at = ?2, outcome = ?3, uphold_pending = 1
              WHERE id = ?1 AND purged_at IS NULL AND restored_at IS NULL AND {}",
            if court { "court_action_at IS NOT NULL" } else { "counter_notice_id IS NULL AND court_action_at IS NULL" }
        ),
        params![action_id, now, outcome],
    )?;
    let a: Option<(String, String, String, Option<String>)> = x
        .conn
        .query_row("SELECT id, item_id, author_id, rightful_item_id FROM dmca_actions WHERE id = ?1 AND uphold_pending = 1", [action_id], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)))
        .optional()?;
    let Some((id, item_id, author_id, rightful)) = a else { return Ok(false) };
    let mut copies: Vec<String> = Vec::new();
    if rightful.is_none() {
        let mut after = String::new();
        loop {
            let page = ghost_copies(x, &item_id, "", &after)?;
            copies.extend(page.iter().take(super::modlineage::LINEAGE_COPIES_MAX).map(|c| c.item_id.clone()));
            if page.len() <= super::modlineage::LINEAGE_COPIES_MAX {
                break;
            }
            after = page[super::modlineage::LINEAGE_COPIES_MAX - 1].item_id.clone();
        }
    }
    let voids = should_void_evidence(x, &author_id, rightful.as_deref())?;
    let tx = x.conn.unchecked_transaction()?;
    if voids {
        void_evidence(&tx, Who::Item(&item_id), now, false, None, "takedown_upheld", rightful.as_deref())?;
    }
    tx.execute("UPDATE strikes SET upheld_at = ?2 WHERE action_id = ?1 AND upheld_at IS NULL AND voided_at IS NULL", params![id, now])?;
    hold_copies(x, &id, &item_id, &copies)?;
    audit(&tx, &AuditEntry { actor_id: None, actor_kind: "system", action: "evidence.void", item_id: Some(&item_id), target_user_id: None, detail: Some(json!({ "why": "takedown_upheld", "action_id": id, "voided": voids })), note: None }, now, None)?;
    tx.commit()?;
    notify_held(x, &copies, "copyright")?;
    repeat_infringer_check(x, &author_id)?;
    x.conn.execute("UPDATE dmca_actions SET uphold_pending = 0 WHERE id = ?1", [&id])?;
    Ok(true)
}

#[cfg(test)]
#[path = "modpurge_tests.rs"]
mod tests;
