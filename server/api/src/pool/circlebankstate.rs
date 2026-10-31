//! Removing, restoring and reporting a circle bank (worker/src/pool/circles.ts `deleteBank`, `undeleteBank`, `reportBank`). HTTP-free cores.
//! Purge-now waits for the purge machinery (`purgeBank`).
use super::circlebanks::*;
use super::circlebase::*;
use super::circles::{bad_field, opt_text};
use super::circlesql::*;
use super::ctx::{AuditEntry, PoolCtx, PoolError, PoolNotice, Res, audit};
use super::sqlfrag::{DAY_MS, alive_sql, day_of, new_id};
use rusqlite::params;
use serde_json::{Map, Value, json};

/// DELETE /circles/:cid/banks/:bid (owner or the bank's author): hidden at once, 14 days to undo, then purged.
pub fn delete_bank(x: &PoolCtx, me: &str, cid: &str, bid: &str) -> Res<Value> {
    let (m, b) = bank_or_miss(x, cid, bid, me)?;
    let author = b.author_id == me;
    if !author && m.role != "owner" {
        return Err(PoolError::new(403, "circle_role_required").with("needed", "owner".into()).with("role", m.role.clone().into()).into());
    }
    let log = |tx: &rusqlite::Connection| audit(tx, &AuditEntry { actor_id: Some(me), actor_kind: "user", action: "circle.bank_remove", item_id: Some(cid), target_user_id: Some(me), detail: Some(json!({ "bank_id": bid })), ..Default::default() }, x.now, None);
    if b.removed_at.is_some() {
        // The owner's delete of a bank its author removed makes it the owner's removal (the author can no longer undo it); the window
        // is not extended.
        if b.removed_reason.as_deref() == Some("author_removed") && m.role == "owner" && !author {
            let tx = x.conn.unchecked_transaction()?;
            let up = tx.execute(
                &format!(
                    "UPDATE circle_banks SET removed_reason = 'owner_removed', updated_at = ?4 WHERE id = ?1 AND circle_id = ?2 AND removed_reason = 'author_removed' AND purged_at IS NULL
               AND {} AND {}",
                    active_sql("?2", "?3", &["owner"]),
                    alive_sql("?3")
                ),
                params![bid, cid, me, x.now],
            )?;
            if up == 1 {
                log(&tx)?;
            }
            tx.commit()?;
            if up != 1 {
                x.throw_if_erased(me)?;
            }
        }
        if matches!(b.removed_reason.as_deref(), Some("author_removed") | Some("owner_removed")) {
            return Ok(json!({ "ok": true, "undo_until": b.purge_after }));
        }
        return Err(PoolError::new(409, "under_moderation").into());
    }
    let tx = x.conn.unchecked_transaction()?;
    let upd = tx.execute(
        &format!(
            "UPDATE circle_banks SET removed_at = ?4, removed_reason = ?5, purge_after = ?6, updated_at = ?4
          WHERE id = ?1 AND circle_id = ?2 AND removed_at IS NULL AND purged_at IS NULL AND (author_id = ?3 OR {}) AND {} AND {}",
            active_sql("?2", "?3", &["owner"]),
            active_sql("?2", "?3", &[]),
            alive_sql("?3")
        ),
        params![bid, cid, me, x.now, if author { "author_removed" } else { "owner_removed" }, x.now + CIRCLE_UNDO_MS],
    )?;
    if upd == 1 {
        log(&tx)?;
    }
    tx.commit()?;
    if upd != 1 {
        return Err(explain(x, cid, me, None));
    }
    Ok(json!({ "ok": true, "undo_until": x.now + CIRCLE_UNDO_MS }))
}

/// POST /circles/:cid/banks/:bid/undelete: within its 14 days, the owner's removal by the owner, the author's own by the author or the
/// owner. (A removed bank still counts toward the circle's caps until it is purged, so undoing needs no cap check.)
pub fn undelete_bank(x: &PoolCtx, me: &str, cid: &str, bid: &str) -> Res<Value> {
    let (m, b) = bank_or_miss(x, cid, bid, me)?;
    require_open(&m)?;
    if b.removed_at.is_none() {
        return Ok(json!({ "ok": true, "already": true }));
    }
    let gone = || -> super::ctx::Fail { PoolError::new(409, "bank_removed").with("reason", b.removed_reason.clone().map(Value::from).unwrap_or(Value::Null)).with("message", "This bank can no longer be restored.".into()).into() };
    if !matches!(b.removed_reason.as_deref(), Some("author_removed") | Some("owner_removed")) {
        return Err(gone());
    }
    // The owner's removal is undone by the owner only; the author's own by either.
    if m.role != "owner" && (b.author_id != me || b.removed_reason.as_deref() != Some("author_removed")) {
        return Err(PoolError::new(403, "circle_role_required").with("needed", "owner".into()).with("role", m.role.clone().into()).into());
    }
    let tx = x.conn.unchecked_transaction()?;
    let upd = tx.execute(
        &format!(
            "UPDATE circle_banks SET removed_at = NULL, removed_reason = NULL, purge_after = NULL, updated_at = ?4
          WHERE id = ?1 AND circle_id = ?2 AND removed_reason IN ('author_removed', 'owner_removed') AND purge_after > ?4 AND purged_at IS NULL
            AND ({} OR (removed_reason = 'author_removed' AND author_id = ?3)) AND {} AND {} AND {}",
            active_sql("?2", "?3", &["owner"]),
            active_sql("?2", "?3", &[]),
            open_sql("?2"),
            alive_sql("?3")
        ),
        params![bid, cid, me, x.now],
    )?;
    if upd == 1 {
        audit(&tx, &AuditEntry { actor_id: Some(me), actor_kind: "user", action: "circle.bank_undelete", item_id: Some(cid), target_user_id: Some(me), detail: Some(json!({ "bank_id": bid })), ..Default::default() }, x.now, None)?;
    }
    tx.commit()?;
    if upd != 1 {
        x.throw_if_erased(me)?;
        return Err(gone());
    }
    Ok(json!({ "ok": true }))
}

/// POST /circles/:cid/banks/:bid/report {reason, note?} (a member, not the author): always "thanks" (202).
/// One student_data report from an ESTABLISHED, approved member hides the bank from the circle at once (within the day and site-wide
/// caps); any other reason hides it once `report_threshold` distinct members report it. The admin is told (ids only).
pub fn report_bank(x: &PoolCtx, me: &str, cid: &str, bid: &str, body: &Map<String, Value>) -> Res<(u16, Value)> {
    let (m, b) = bank_or_miss(x, cid, bid, me)?;
    let reason = match body.get("reason").and_then(Value::as_str) {
        Some(r) if CIRCLE_REPORT_REASONS.contains(&r) => r,
        _ => return Err(bad_field("reason", &[("allowed", json!(CIRCLE_REPORT_REASONS))])),
    };
    let note = opt_text(body.get("note"), 1000, "note")?;
    require_readable(x, &m, &b, me)?;
    if b.author_id == me {
        return Err(PoolError::new(400, "own_item").into());
    }
    x.require_daily(me, "reports", 1)?;
    let rid = new_id();
    let day0 = day_of(x.now) * DAY_MS;
    let tx = x.conn.unchecked_transaction()?;
    let made = tx.execute(
        &format!(
            "INSERT OR IGNORE INTO circle_reports (id, circle_id, bank_id, version, reporter_id, reason, note, at) SELECT ?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8
          WHERE {} AND {}",
            active_sql("?2", "?5", &[]),
            alive_sql("?5")
        ),
        params![rid, cid, bid, b.head_version, me, reason, note, x.now],
    )?;
    let mut held = 0;
    if made == 1 {
        if reason == "student_data" {
            let global = x.limit_of("*", "instant_holds_global")?;
            held = tx.execute(
                &format!(
                    "INSERT OR IGNORE INTO circle_holds (bank_id, reason, source, at, by) SELECT ?1, 'student_data', '', ?2, NULL
            WHERE EXISTS (SELECT 1 FROM circle_reports r WHERE r.id = ?9)
              AND NOT EXISTS (SELECT 1 FROM circle_reports d WHERE d.reporter_id = ?3 AND d.reason = 'student_data' AND d.action = 'dismissed' AND d.resolved_at > ?2 - ?4)
              AND (SELECT COUNT(*) FROM circle_holds h WHERE h.reason = 'student_data' AND h.source = '' AND h.at >= ?5
                     AND EXISTS (SELECT 1 FROM circle_reports r2 WHERE r2.bank_id = h.bank_id AND r2.reporter_id = ?3 AND r2.reason = 'student_data' AND r2.at = h.at)) < ?6
              AND (SELECT COUNT(*) FROM circle_holds h WHERE h.reason = 'student_data' AND h.source = '' AND h.at >= ?5)
                  + (SELECT COUNT(*) FROM item_holds ih WHERE ih.reason = 'student_data' AND ih.source = '' AND ih.at >= ?5) < ?7
              AND COALESCE((SELECT MAX(a.decided_at) FROM approval_requests a WHERE a.user_id = ?3 AND a.decision = 'approved'), (SELECT u.created_at FROM users u WHERE u.id = ?3)) <= ?2 - ?8
              AND EXISTS (SELECT 1 FROM users ru WHERE ru.id = ?3 AND ru.trust IN ('teacher', 'admin') AND ru.suspended_at IS NULL AND ru.deleted_at IS NULL)
              AND EXISTS (SELECT 1 FROM circle_members em WHERE em.circle_id = ?10 AND em.user_id = ?3 AND em.left_at IS NULL AND em.joined_at <= ?2 - ?11)
              AND {}",
                    alive_sql("?3")
                ),
                params![bid, x.now, me, CIRCLE_THROTTLE_MS, day0, CIRCLE_INSTANT_HOLDS_PER_DAY, global, ESTABLISHED_ACCOUNT_MS, rid, cid, ESTABLISHED_MEMBER_MS],
            )?;
        } else {
            let threshold = x.limit_of("*", "report_threshold")?;
            held = tx.execute(
                &format!(
                    "INSERT OR IGNORE INTO circle_holds (bank_id, reason, source, at, by)
           SELECT ?1, 'reports', '', ?2, NULL WHERE (SELECT COUNT(DISTINCT r.reporter_id) FROM circle_reports r
               WHERE r.bank_id = ?1 AND r.resolved_at IS NULL AND r.reason <> 'student_data'
                 AND NOT EXISTS (SELECT 1 FROM circle_reports d WHERE d.bank_id = ?1 AND d.reporter_id = r.reporter_id AND d.action = 'dismissed' AND d.resolved_at > ?2 - ?4)) >= ?3
             AND {}",
                    alive_sql("?5")
                ),
                params![bid, x.now, threshold, CIRCLE_THROTTLE_MS, me],
            )?;
        }
        // (the report's id stays out of the activity list; the audit names the circle and bank only)
        audit(&tx, &AuditEntry { actor_id: Some(me), actor_kind: "user", action: "circle.report", item_id: Some(cid), detail: Some(json!({ "bank_id": bid, "reason": reason })), ..Default::default() }, x.now, None)?;
    }
    tx.commit()?;
    if made == 0 {
        x.throw_if_erased(me)?;
    } else {
        x.alert_budgeted(PoolNotice::CircleReport { circle_id: cid.into(), bank_id: bid.into(), report_id: rid, reason: reason.into(), held: held == 1 })?;
    }
    Ok((202, json!({ "ok": true, "message": "Thanks, the admin will review it." })))
}

#[cfg(test)]
#[path = "circlebankstate_tests.rs"]
mod tests;
