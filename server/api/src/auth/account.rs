//! The signed-in user's own account: `me`, `export` and `delete` (port of the handlers of the same names in
//! auth/src/auth.ts), plus the small admin-side pieces they lean on (`status_of`, the last-admin check, the deletion
//! guard and the audit row), which the admin routes reuse.
use super::audit::{AuditFields, audit};
use super::signup::read_profile;
use super::config::{ADMIN_LEVELS, Action, AFTER_VERIFY, APPROVED_LEVEL};
use super::crypto::{hmac, uuid_v4};
use super::device::sudo_valid;
use super::guard::{self, acct_prefix};
use super::http::{ClientInfo, ClientMode, HttpError, clear_cookie, empty, json};
use super::password::verify_password;
use super::session::{Ctx, Principal, read_password, require_principal, valid_device};
use axum::http::HeaderMap;
use axum::response::Response;
use rusqlite::{Connection, OptionalExtension, Transaction, params};
use serde_json::{Map, Value, json};

fn dberr(e: rusqlite::Error) -> HttpError {
    tracing::error!("auth: database error: {e}");
    HttpError::new(500, "server_error")
}

fn lock<'a>(c: &'a Ctx) -> std::sync::MutexGuard<'a, Connection> {
    c.db.lock().unwrap_or_else(|p| p.into_inner())
}

pub fn is_admin_level(trust: &str) -> bool {
    ADMIN_LEVELS.contains(&trust)
}

fn admins_json() -> String {
    serde_json::to_string(&ADMIN_LEVELS).unwrap_or_else(|_| "[]".into())
}

/// SQL: true unless `users` (the row being changed) is an admin with no other active admin left. `?a` is the admin JSON.
pub fn not_last_admin(a: &str) -> String {
    format!(
        "(users.trust NOT IN (SELECT value FROM json_each({a})) OR (SELECT COUNT(*) FROM users o WHERE o.trust IN (SELECT value FROM json_each({a}))
           AND o.deleted_at IS NULL AND o.suspended_at IS NULL AND o.id NOT IN (users.id)) >= 1)"
    )
}

/// The account's standing as the applicant sees it.
pub fn status_of(conn: &Connection, id: &str, trust: &str, suspended_at: Option<i64>, suspension_reason: Option<&str>, now: i64, cooldown_ms: i64) -> Result<Value, HttpError> {
    if suspended_at.is_some() {
        return Ok(json!({ "state": "suspended", "reason": suspension_reason }));
    }
    if is_admin_level(trust) || trust == APPROVED_LEVEL {
        return Ok(json!({ "state": "approved" }));
    }
    if trust != AFTER_VERIFY {
        return Ok(json!({ "state": "unverified" }));
    }
    let last: Option<(Option<i64>, Option<String>, Option<String>)> = conn
        .query_row("SELECT decided_at, decision, reason FROM approval_requests WHERE user_id = ?1 ORDER BY requested_at DESC, rowid DESC LIMIT 1", [id], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))
        .optional()
        .map_err(dberr)?;
    Ok(match last {
        Some((Some(decided), Some(decision), reason)) if decision == "rejected" || decision == "revoked" => {
            let after = decided + cooldown_ms;
            json!({ "state": decision, "reason": reason, "reapply_after": after, "can_reapply": now >= after })
        }
        _ => json!({ "state": "pending" }),
    })
}

pub fn cooldown_ms(c: &Ctx) -> i64 {
    c.cfg.reapply_cooldown * 1000
}

pub fn me(c: &Ctx, h: &HeaderMap) -> Result<Response, HttpError> {
    let p = require_principal(c, h)?;
    let conn = lock(c);
    #[allow(clippy::type_complexity)]
    let u: Option<(String, String, Option<i64>, Option<String>, Option<String>, String, i64, Option<i64>, Option<String>)> = conn
        .query_row(
            "SELECT id, email, email_verified_at, display_name, profile, trust, created_at, suspended_at, suspension_reason FROM users WHERE id = ?1 AND deleted_at IS NULL",
            [&p.user_id],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?, r.get(5)?, r.get(6)?, r.get(7)?, r.get(8)?)),
        )
        .optional()
        .map_err(dberr)?;
    let Some((id, email, verified, name, profile, trust, created, susp, reason)) = u else { return Err(HttpError::new(401, "unauthorized")) };
    let status = status_of(&conn, &id, &trust, susp, reason.as_deref(), c.now, cooldown_ms(c))?;
    Ok(json(
        200,
        &json!({
            "status": status, "id": id, "email": email, "email_verified": verified.is_some(), "display_name": name,
            "profile": profile.and_then(|p| serde_json::from_str::<Value>(&p).ok()), "trust": trust, "created_at": created,
        }),
        &[],
    ))
}

/// Everything held about the account, for the user. `extra` is the app's own section (the pool adds its items).
pub fn export(c: &Ctx, h: &HeaderMap, extra: &dyn Fn(&Connection, &str) -> Map<String, Value>) -> Result<Response, HttpError> {
    let p = require_principal(c, h)?;
    let conn = lock(c);
    #[allow(clippy::type_complexity)]
    let u: Option<(String, String, Option<i64>, Option<String>, Option<String>, String, i64)> = conn
        .query_row(
            "SELECT id, email, email_verified_at, display_name, profile, trust, created_at FROM users WHERE id = ?1 AND deleted_at IS NULL",
            [&p.user_id],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?, r.get(5)?, r.get(6)?)),
        )
        .optional()
        .map_err(dberr)?;
    let Some((id, email, verified, name, profile, trust, created)) = u else { return Err(HttpError::new(401, "unauthorized")) };
    let mut st = conn
        .prepare("SELECT family_id, auth_at, created_at, expires_at, user_agent FROM sessions WHERE user_id = ?1 AND revoked_at IS NULL AND rotated_at IS NULL ORDER BY auth_at")
        .map_err(dberr)?;
    let sessions: Vec<Value> = st
        .query_map([&p.user_id], |r| {
            let family: String = r.get(0)?;
            Ok(json!({
                "signed_in_at": r.get::<_, i64>(1)?, "last_refreshed_at": r.get::<_, i64>(2)?, "expires_at": r.get::<_, i64>(3)?,
                "user_agent": r.get::<_, Option<String>>(4)?, "current": family == p.session_family,
            }))
        })
        .map_err(dberr)?
        .collect::<Result<_, _>>()
        .map_err(dberr)?;
    // The applicant's own history: decisions and the reasons given to them, never reviewer notes.
    let mut at = conn.prepare("SELECT requested_at, decided_at, decision, reason FROM approval_requests WHERE user_id = ?1 ORDER BY requested_at, id").map_err(dberr)?;
    let applications: Vec<Value> = at
        .query_map([&p.user_id], |r| Ok(json!({ "requested_at": r.get::<_, i64>(0)?, "decided_at": r.get::<_, Option<i64>>(1)?, "decision": r.get::<_, Option<String>>(2)?, "reason": r.get::<_, Option<String>>(3)? })))
        .map_err(dberr)?
        .collect::<Result<_, _>>()
        .map_err(dberr)?;
    let mut body = json!({
        "account": { "id": id, "email": email, "email_verified_at": verified, "display_name": name, "profile": profile.and_then(|p| serde_json::from_str::<Value>(&p).ok()), "trust": trust, "created_at": created },
        "sessions": sessions,
        "approval": applications.last().cloned().unwrap_or(Value::Null),
        "applications": applications,
    });
    for (k, v) in extra(&conn, &p.user_id) {
        body[k] = v;
    }
    Ok(json(200, &body, &[]))
}

/// An admin-level account needs its known device and a sudo token (X-Sudo-Token) to delete itself.
fn deletion_guard(c: &Ctx, h: &HeaderMap, client: &ClientInfo, p: &Principal, id: &str, trust: &str, epoch: i64) -> Result<(), HttpError> {
    if !is_admin_level(trust) {
        return Ok(());
    }
    if valid_device(c, h, client, Some(&guard::DeviceUser { id, device_epoch: epoch })).is_none() {
        return Err(HttpError::new(403, "known_device_required"));
    }
    let sudo = h.get("x-sudo-token").and_then(|v| v.to_str().ok());
    if !sudo_valid(c.keys, sudo, p, epoch, c.now) {
        return Err(HttpError::new(403, "sudo_required"));
    }
    Ok(())
}

/// The pool's cleanup of the author's content runs inside the deletion transaction, once the account is certainly gone.
pub type OnDelete<'a> = &'a dyn Fn(&Transaction, &str, i64) -> rusqlite::Result<()>;

pub fn delete_account(c: &Ctx, h: &HeaderMap, client: &ClientInfo, body: &Map<String, Value>, on_delete: OnDelete) -> Result<Response, HttpError> {
    let p = require_principal(c, h)?;
    let password = read_password(body, "password")?;
    let ip = guard::client_ip(h)?;
    // Re-authentication: a stolen 15-minute access token alone cannot delete the account.
    guard::limit(&lock(c), c.keys, Action::Delete, &ip, Some(&format!("user:{}", p.user_id)), c.now)?;
    let u: Option<(String, String, Option<String>, String, i64)> = lock(c)
        .query_row("SELECT id, email, pw_hash, trust, device_epoch FROM users WHERE id = ?1 AND deleted_at IS NULL", [&p.user_id], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?)))
        .optional()
        .map_err(dberr)?;
    let Some((id, email, Some(pw_hash), trust, epoch)) = u else { return Err(HttpError::new(401, "unauthorized")) };
    deletion_guard(c, h, client, &p, &id, &trust, epoch)?;
    // Password attempts here count only against the per-account delete limiter above, never the login lockdown.
    if !verify_password(&pw_hash, &password) {
        return Err(HttpError::new(401, "invalid_credentials"));
    }
    let email_hmac = hmac(&c.keys.email, &email);
    let now = c.now;
    let mut conn = lock(c);
    let tx = conn.transaction().map_err(dberr)?;
    // The tombstone is written only if the password checked above is still the current one, so a reset that committed
    // in between makes this deletion fail instead of acting on stale proof.
    let upd = tx
        .execute(
            &format!(
                "UPDATE users SET email = NULL, email_verified_at = NULL, pw_hash = NULL, pw_params = NULL, display_name = NULL,
                   profile = NULL, suspension_reason = NULL, deleted_at = ?2, deleted_email_hmac = ?3, deleted_email_hmac_expires_at = ?4,
                   device_epoch = device_epoch + 1
                 WHERE id = ?1 AND deleted_at IS NULL AND pw_hash = ?5 AND {}",
                not_last_admin("?6")
            ),
            params![id, now, email_hmac, now + c.cfg.tombstone_ttl * 1000, pw_hash, admins_json()],
        )
        .map_err(dberr)?;
    if upd != 1 {
        drop(tx);
        if is_admin_level(&trust) {
            let still: Option<String> = conn.query_row("SELECT pw_hash FROM users WHERE id = ?1 AND deleted_at IS NULL", [&id], |r| r.get(0)).optional().map_err(dberr)?;
            if still.as_deref() == Some(pw_hash.as_str()) {
                return Err(HttpError::new(409, "last_admin"));
            }
        }
        return Err(HttpError::new(401, "invalid_credentials"));
    }
    for sql in [
        "DELETE FROM sessions WHERE user_id = ?1",
        "DELETE FROM email_tokens WHERE user_id = ?1",
        "DELETE FROM device_enrolments WHERE user_id = ?1",
        // Application records stay as anonymised tombstones; the free text may hold PII and goes. An open application
        // is closed so it leaves the queue.
        "UPDATE approval_requests SET note = NULL, evidence_note = NULL, reason = NULL, display_name_snapshot = NULL,
           profile_snapshot = NULL, decision = COALESCE(decision, 'account_deleted'), decided_at = COALESCE(decided_at, ?2) WHERE user_id = ?1",
        // The audit trail survives; only its free text about this account is nulled (the one change the append-only
        // triggers allow, and only for a deleted account).
        "UPDATE auth_audit SET note = NULL, evidence_note = NULL, reason = NULL WHERE target_id = ?1 AND (note IS NOT NULL OR evidence_note IS NOT NULL OR reason IS NOT NULL)",
        "UPDATE admin_proposals SET note = NULL, reason = NULL WHERE target_id = ?1",
    ] {
        if sql.contains("?2") {
            tx.execute(sql, params![id, now]).map_err(dberr)?;
        } else {
            tx.execute(sql, params![id]).map_err(dberr)?;
        }
    }
    tx.execute(
        "INSERT INTO auth_audit (id, at, actor_id, actor_kind, action, target_id, prev_trust, new_trust) VALUES (?1, ?2, ?3, 'user', 'account_deleted', ?3, ?4, NULL)",
        params![uuid_v4(), now, id, trust],
    )
    .map_err(dberr)?;
    // Rate-limit counters keyed on this account go now, not when they would have expired.
    for account in [format!("email:{email}"), format!("user:{id}")] {
        let pre = acct_prefix(c.keys, &account);
        let end = format!("{};", &pre[..pre.len() - 1]);
        tx.execute("DELETE FROM rate_limits WHERE key >= ?1 AND key < ?2", params![pre, end]).map_err(dberr)?;
    }
    on_delete(&tx, &id, now).map_err(dberr)?;
    tx.commit().map_err(dberr)?;
    tracing::info!(user = id.as_str(), "auth: account deleted");
    Ok(if client.mode == ClientMode::Web { empty(204, &[("Set-Cookie", &clear_cookie(&c.cfg.cookie_name))]) } else { empty(204, &[]) })
}

/// A rejected or revoked applicant applies again, after the cooldown, with a fresh profile. It is a NEW application row
/// (with its own snapshot); the old decision stays as it was.
pub fn reapply(c: &Ctx, h: &HeaderMap, body: &Map<String, Value>) -> Result<Response, HttpError> {
    let p = require_principal(c, h)?;
    let prof = read_profile(body, c.now)?;
    let ip = guard::client_ip(h)?;
    let mut conn = lock(c);
    guard::limit(&conn, c.keys, Action::Reapply, &ip, Some(&format!("user:{}", p.user_id)), c.now)?;
    let not_eligible = || HttpError::new(409, "not_eligible");
    let u: Option<(String, Option<i64>)> = conn.query_row("SELECT trust, suspended_at FROM users WHERE id = ?1 AND deleted_at IS NULL", [&p.user_id], |r| Ok((r.get(0)?, r.get(1)?))).optional().map_err(dberr)?;
    match u {
        Some((trust, None)) if trust == AFTER_VERIFY => {}
        _ => return Err(not_eligible()),
    }
    let last: Option<(Option<i64>, Option<String>)> = conn
        .query_row("SELECT decided_at, decision FROM approval_requests WHERE user_id = ?1 ORDER BY requested_at DESC, rowid DESC LIMIT 1", [&p.user_id], |r| Ok((r.get(0)?, r.get(1)?)))
        .optional()
        .map_err(dberr)?;
    let decided = match last {
        Some((Some(at), Some(d))) if d == "rejected" || d == "revoked" => at,
        _ => return Err(not_eligible()),
    };
    let after = decided + cooldown_ms(c);
    if c.now < after {
        return Err(HttpError::new(429, "cooldown").with("reapply_after", json!(after)).header("Retry-After", &((after - c.now + 999) / 1000).to_string()));
    }
    let req_id = uuid_v4();
    let profile_json = prof.profile.as_ref().map(Value::to_string);
    let tx = conn.transaction().map_err(dberr)?;
    // OR IGNORE + the one-open-application index: concurrent re-applications make one row.
    let n = tx
        .execute(
            "INSERT OR IGNORE INTO approval_requests (id, user_id, requested_at, email_domain, display_name_snapshot, profile_snapshot)
             SELECT ?1, id, ?2, substr(email, instr(email, '@') + 1), ?4, ?5 FROM users WHERE id = ?3",
            params![req_id, c.now, p.user_id, prof.display_name, profile_json],
        )
        .map_err(dberr)?;
    if n != 1 {
        return Err(not_eligible());
    }
    tx.execute("UPDATE users SET display_name = ?1, profile = ?2 WHERE id = ?3", params![prof.display_name, profile_json, p.user_id]).map_err(dberr)?;
    let e = AuditFields { action: "reapply", target_id: Some(&p.user_id), detail: Some(json!({ "request_id": req_id })), ..Default::default() };
    audit(&tx, Some(&p.user_id), "user", &e, c.now, None, false, &uuid_v4()).map_err(dberr)?;
    tx.commit().map_err(dberr)?;
    Ok(json(202, &json!({ "ok": true, "request_id": req_id }), &[]))
}

#[cfg(test)]
#[path = "account_tests.rs"]
mod tests;
