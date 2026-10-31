//! The approval workflow, part 2: suspend and restore, with the two-person rule for admins (port of `executeSuspend`,
//! `suspend`, `confirm`, `cancel`, `proposals` and `restore` in auth/src/admin.ts).
//!
//! Against an admin-level account a suspension only PROPOSES: a different admin must confirm. If no other admin could
//! confirm, the proposer may confirm alone after `admin_self_confirm_after`. Every step logs an alert line, so a
//! compromised admin cannot quietly suspend the others one by one. (Mailing the alert to the admins and the escalation
//! contact is the notices stage, not this module.)
use super::account::{is_admin_level, not_last_admin};
use super::admin::{AdminPrincipal, dberr, guard_admin, id_field, lock, notify, text};
use super::audit::{AuditFields, audit};
use super::config::ADMIN_LEVELS;
use super::crypto::uuid_v4;
use super::http::{ClientInfo, HttpError, json};
use super::mail::{MailKind, Outbox};
use super::session::Ctx;
use axum::http::HeaderMap;
use axum::response::Response;
use rusqlite::{Connection, OptionalExtension, Transaction, params};
use serde_json::{Map, Value, json};

/// The pool's content standing changes (suspended or restored authors), applied inside the same transaction.
pub type StandingHook<'a> = &'a dyn Fn(&Transaction, &str, i64) -> rusqlite::Result<()>;

fn admins_json() -> String {
    serde_json::to_string(&ADMIN_LEVELS).unwrap_or_else(|_| "[]".into())
}

/// SQL: admins that are active and not one of `exclude`.
fn other_active_admins(exclude: &str, a: &str) -> String {
    format!("(SELECT COUNT(*) FROM users o WHERE o.trust IN (SELECT value FROM json_each({a})) AND o.deleted_at IS NULL AND o.suspended_at IS NULL AND o.id NOT IN ({exclude}))")
}

fn alert(action: &str, actor: &str, target: &str, proposal: Option<&str>) {
    tracing::warn!(action, actor, target, proposal, "auth alert: admin action");
}

/// Suspends now. The gate, the last-admin rule and the audit are one transaction, so nothing about it can race.
fn execute_suspend(c: &Ctx, actor: &AdminPrincipal, target: &str, note: Option<&str>, reason: &str, proposal: Option<&str>, hook: StandingHook, out: &mut Outbox) -> Result<bool, HttpError> {
    let now = c.now;
    let mut conn = lock(c);
    let tx = conn.transaction().map_err(dberr)?;
    let proposal_open = if proposal.is_some() { "AND EXISTS (SELECT 1 FROM admin_proposals WHERE id = ?5 AND decided_at IS NULL AND expires_at > ?1)" } else { "" };
    let sql = format!(
        "UPDATE users SET suspended_at = ?1, suspension_reason = ?2, device_epoch = device_epoch + 1
         WHERE id = ?3 AND suspended_at IS NULL AND deleted_at IS NULL AND {} {proposal_open} RETURNING trust",
        not_last_admin("?4")
    );
    let was: Option<String> = match proposal {
        Some(p) => tx.query_row(&sql, params![now, reason, target, admins_json(), p], |r| r.get(0)),
        None => tx.query_row(&sql, params![now, reason, target, admins_json()], |r| r.get(0)),
    }
    .optional()
    .map_err(dberr)?;
    let Some(was_trust) = was else { return Ok(false) };
    let e = AuditFields { action: "suspend", target_id: Some(target), note, reason: Some(reason), detail: proposal.map(|p| json!({ "proposal_id": p })), ..Default::default() };
    audit(&tx, Some(&actor.user_id), "admin", &e, now, None, true, &uuid_v4()).map_err(dberr)?;
    if let Some(p) = proposal {
        tx.execute(
            "UPDATE admin_proposals SET decided_by = ?1, decided_at = ?2, outcome = 'confirmed'
             WHERE id = ?3 AND decided_at IS NULL AND EXISTS (SELECT 1 FROM users WHERE id = ?4 AND suspended_at = ?2)",
            params![actor.user_id, now, p, target],
        )
        .map_err(dberr)?;
    }
    // The flip above already blocks every login, refresh and live check; this ends the sessions too.
    tx.execute("UPDATE sessions SET revoked_at = ?1 WHERE user_id = ?2 AND revoked_at IS NULL", params![now, target]).map_err(dberr)?;
    hook(&tx, target, now).map_err(dberr)?;
    tx.commit().map_err(dberr)?;
    if is_admin_level(&was_trust) {
        alert("admin_suspended", &actor.user_id, target, proposal);
    }
    notify(&conn, target, MailKind::AccountSuspended, Some(reason), out);
    Ok(true)
}

fn target_row(conn: &Connection, id: &str) -> Result<Option<(String, Option<i64>)>, HttpError> {
    conn.query_row("SELECT trust, suspended_at FROM users WHERE id = ?1 AND deleted_at IS NULL", [id], |r| Ok((r.get(0)?, r.get(1)?))).optional().map_err(dberr)
}

pub fn suspend(c: &Ctx, h: &HeaderMap, client: &ClientInfo, body: &Map<String, Value>, hook: StandingHook, out: &mut Outbox) -> Result<Response, HttpError> {
    let admin = guard_admin(c, h, client, true)?;
    let target = id_field(body, "user_id")?;
    if target == admin.user_id {
        return Err(HttpError::new(403, "self_action"));
    }
    let note = text(body, "note", true)?.unwrap_or_default();
    let reason = text(body, "reason", true)?.unwrap_or_default();
    let t = target_row(&lock(c), &target)?;
    let Some((trust, _)) = t else { return Err(HttpError::new(404, "not_found")) };
    if is_admin_level(&trust) {
        let id = uuid_v4();
        let mut conn = lock(c);
        let tx = conn.transaction().map_err(dberr)?;
        tx.execute("UPDATE admin_proposals SET decided_at = ?1, outcome = 'expired' WHERE target_id = ?2 AND decided_at IS NULL AND expires_at <= ?1", params![c.now, target]).map_err(dberr)?;
        // Self-confirmation is allowed later only when nobody but the proposer and the target is an active admin.
        let row: Option<(Option<i64>, i64)> = tx
            .query_row(
                &format!(
                    "INSERT OR IGNORE INTO admin_proposals (id, action, target_id, proposed_by, proposed_at, expires_at, self_confirm_after, note, reason)
                     SELECT ?1, 'suspend', ?2, ?3, ?4, ?5, CASE WHEN {} = 0 THEN ?6 ELSE NULL END, ?7, ?9
                     WHERE EXISTS (SELECT 1 FROM users WHERE id = ?2 AND suspended_at IS NULL AND deleted_at IS NULL)
                     RETURNING self_confirm_after, expires_at",
                    other_active_admins("?2, ?3", "?8")
                ),
                params![id, target, admin.user_id, c.now, c.now + c.cfg.admin_proposal_ttl * 1000, c.now + c.cfg.admin_self_confirm_after * 1000, note, admins_json(), reason],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .optional()
            .map_err(dberr)?;
        let Some((self_confirm_after, expires_at)) = row else { return Err(HttpError::new(409, "proposal_exists")) };
        let e = AuditFields { action: "suspend_proposed", target_id: Some(&target), note: Some(&note), reason: Some(&reason), detail: Some(json!({ "proposal_id": id })), ..Default::default() };
        audit(&tx, Some(&admin.user_id), "admin", &e, c.now, None, true, &uuid_v4()).map_err(dberr)?;
        tx.commit().map_err(dberr)?;
        alert("admin_suspension_proposed", &admin.user_id, &target, Some(&id));
        return Ok(json(202, &json!({ "ok": true, "proposal_id": id, "expires_at": expires_at, "self_confirm_after": self_confirm_after }), &[]));
    }
    if !execute_suspend(c, &admin, &target, Some(&note), &reason, None, hook, out)? {
        return Err(HttpError::new(409, "not_suspendable"));
    }
    Ok(json(200, &json!({ "ok": true, "user_id": target }), &[]))
}

/// Confirms a proposal: a different admin (never the target), or the proposer alone once self-confirmation is open.
pub fn confirm(c: &Ctx, h: &HeaderMap, client: &ClientInfo, body: &Map<String, Value>, hook: StandingHook, out: &mut Outbox) -> Result<Response, HttpError> {
    let admin = guard_admin(c, h, client, true)?;
    let id = id_field(body, "proposal_id")?;
    #[allow(clippy::type_complexity)]
    let p: Option<(String, String, Option<i64>, Option<String>, Option<String>)> = lock(c)
        .query_row(
            "SELECT target_id, proposed_by, self_confirm_after, note, reason FROM admin_proposals WHERE id = ?1 AND decided_at IS NULL AND expires_at > ?2",
            params![id, c.now],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?)),
        )
        .optional()
        .map_err(dberr)?;
    let Some((target, proposed_by, self_after, note, reason)) = p else { return Err(HttpError::new(409, "no_open_proposal")) };
    if admin.user_id == target {
        return Err(HttpError::new(403, "self_action"));
    }
    if admin.user_id == proposed_by {
        // Alone only if nobody else could confirm, and only after the delay everyone was alerted about.
        let others: i64 = lock(c).query_row(&format!("SELECT {}", other_active_admins("?1, ?2", "?3")), params![target, proposed_by, admins_json()], |r| r.get(0)).map_err(dberr)?;
        if self_after.is_none_or(|t| c.now < t) || others > 0 {
            return Err(HttpError::new(403, "second_admin_required"));
        }
    }
    if !execute_suspend(c, &admin, &target, note.as_deref(), reason.as_deref().unwrap_or(""), Some(&id), hook, out)? {
        return Err(HttpError::new(409, "not_suspendable"));
    }
    Ok(json(200, &json!({ "ok": true, "user_id": target, "proposal_id": id }), &[]))
}

/// Cancels an open proposal (any admin, including its target).
pub fn cancel(c: &Ctx, h: &HeaderMap, client: &ClientInfo, body: &Map<String, Value>) -> Result<Response, HttpError> {
    let admin = guard_admin(c, h, client, true)?;
    let id = id_field(body, "proposal_id")?;
    let mut conn = lock(c);
    let tx = conn.transaction().map_err(dberr)?;
    let target: Option<String> = tx
        .query_row("UPDATE admin_proposals SET decided_by = ?1, decided_at = ?2, outcome = 'cancelled' WHERE id = ?3 AND decided_at IS NULL RETURNING target_id", params![admin.user_id, c.now, id], |r| r.get(0))
        .optional()
        .map_err(dberr)?;
    let Some(target) = target else { return Err(HttpError::new(409, "no_open_proposal")) };
    let e = AuditFields { action: "suspend_proposal_cancelled", target_id: Some(&target), detail: Some(json!({ "proposal_id": id })), ..Default::default() };
    audit(&tx, Some(&admin.user_id), "admin", &e, c.now, None, false, &uuid_v4()).map_err(dberr)?;
    tx.commit().map_err(dberr)?;
    alert("admin_suspension_cancelled", &admin.user_id, &target, Some(&id));
    Ok(json(200, &json!({ "ok": true }), &[]))
}

pub fn proposals(c: &Ctx, h: &HeaderMap, client: &ClientInfo) -> Result<Response, HttpError> {
    guard_admin(c, h, client, false)?;
    let conn = lock(c);
    let mut st = conn
        .prepare(
            "SELECT id, action, target_id, proposed_by, proposed_at, expires_at, self_confirm_after, decided_by, decided_at, outcome
             FROM admin_proposals ORDER BY proposed_at DESC, id DESC LIMIT 100",
        )
        .map_err(dberr)?;
    let rows: Vec<Value> = st
        .query_map([], |r| {
            Ok(json!({
                "id": r.get::<_, String>(0)?, "action": r.get::<_, String>(1)?, "target_id": r.get::<_, String>(2)?, "proposed_by": r.get::<_, String>(3)?,
                "proposed_at": r.get::<_, i64>(4)?, "expires_at": r.get::<_, i64>(5)?, "self_confirm_after": r.get::<_, Option<i64>>(6)?,
                "decided_by": r.get::<_, Option<String>>(7)?, "decided_at": r.get::<_, Option<i64>>(8)?, "outcome": r.get::<_, Option<String>>(9)?,
            }))
        })
        .map_err(dberr)?
        .collect::<Result<_, _>>()
        .map_err(dberr)?;
    Ok(json(200, &json!({ "proposals": rows }), &[]))
}

pub fn restore(c: &Ctx, h: &HeaderMap, client: &ClientInfo, body: &Map<String, Value>, hook: StandingHook, out: &mut Outbox) -> Result<Response, HttpError> {
    let admin = guard_admin(c, h, client, true)?;
    let target = id_field(body, "user_id")?;
    if target == admin.user_id {
        return Err(HttpError::new(403, "self_action"));
    }
    let note = text(body, "note", true)?.unwrap_or_default();
    let mut conn = lock(c);
    let tx = conn.transaction().map_err(dberr)?;
    let restored: Option<String> = tx
        .query_row("UPDATE users SET suspended_at = NULL, suspension_reason = NULL WHERE id = ?1 AND suspended_at IS NOT NULL AND deleted_at IS NULL RETURNING trust", [&target], |r| r.get(0))
        .optional()
        .map_err(dberr)?;
    if restored.is_none() {
        return Err(HttpError::new(409, "not_suspended"));
    }
    let e = AuditFields { action: "restore", target_id: Some(&target), note: Some(&note), ..Default::default() };
    audit(&tx, Some(&admin.user_id), "admin", &e, c.now, None, true, &uuid_v4()).map_err(dberr)?;
    hook(&tx, &target, c.now).map_err(dberr)?;
    tx.commit().map_err(dberr)?;
    notify(&conn, &target, MailKind::AccountRestored, None, out);
    Ok(json(200, &json!({ "ok": true, "user_id": target }), &[]))
}

#[cfg(test)]
#[path = "admin_suspend_tests.rs"]
mod tests;
