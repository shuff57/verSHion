//! Known devices and the sudo step: enrol (redeem a mailed code), enrolment_code (ask for another), reauth (a 5-minute
//! sudo token) and the sudo token itself (port of the handlers of the same names in auth/src/auth.ts and `issueSudo` /
//! `sudoValid` in auth/src/admin.ts).
use super::config::{Action, ENROL_MAX_FAILURES, LEASE_MS};
use super::crypto::{Keys, hmac, is_token, sha256, timing_safe_equal};
use super::guard::{self, DeviceUser};
use super::http::{ClientInfo, HttpError, json};
use super::mail::Outbox;
use super::password::verify_password;
use super::session::{Ctx, Principal, read_password, require_principal, valid_device};
use super::signup::ok_body;
use super::tokens::{issue_enrolment, with_device};
use axum::http::HeaderMap;
use axum::response::Response;
use rusqlite::{OptionalExtension, params};
use serde_json::{Map, Value, json};

pub const SUDO_TTL_S: i64 = 5 * 60;

fn dberr(e: rusqlite::Error) -> HttpError {
    tracing::error!("auth: database error: {e}");
    HttpError::new(500, "server_error")
}

fn lock<'a>(c: &'a Ctx) -> std::sync::MutexGuard<'a, rusqlite::Connection> {
    c.db.lock().unwrap_or_else(|p| p.into_inner())
}

// ---- sudo tokens -----------------------------------------------------------------------------
// `s1.<userId>.<sessionFamily>.<deviceEpoch>.<expSeconds>.<mac>`: 5 minutes, bound to one session family (so a token
// lifted from another session is useless) and to the device epoch (so a reset or logout-everywhere voids it).
// Stateless; its own HKDF key.

pub fn issue_sudo(keys: &Keys, p: &Principal, epoch: i64, now_ms: i64) -> (String, i64) {
    let payload = format!("s1.{}.{}.{}.{}", p.user_id, p.session_family, epoch, now_ms / 1000 + SUDO_TTL_S);
    (format!("{payload}.{}", hmac(&keys.sudo, &payload)), SUDO_TTL_S)
}

fn is_uuid_like(s: &str) -> bool {
    s.len() == 36 && s.bytes().all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b) || b == b'-')
}

pub fn sudo_valid(keys: &Keys, token: Option<&str>, p: &Principal, epoch: i64, now_ms: i64) -> bool {
    let Some(token) = token else { return false };
    let parts: Vec<&str> = token.split('.').collect();
    let [tag, uid, sid, e, exp, mac] = parts[..] else { return false };
    let digits = |s: &str, max: usize| !s.is_empty() && s.len() <= max && s.bytes().all(|b| b.is_ascii_digit());
    if tag != "s1" || !is_uuid_like(uid) || !is_uuid_like(sid) || !digits(e, 9) || !digits(exp, 12) || mac.len() != 43 || !mac.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_') {
        return false;
    }
    if uid != p.user_id || sid != p.session_family || e.parse::<i64>() != Ok(epoch) || exp.parse::<i64>().unwrap_or(0) <= now_ms / 1000 {
        return false;
    }
    let payload = format!("s1.{uid}.{sid}.{e}.{exp}");
    timing_safe_equal(mac.as_bytes(), hmac(&keys.sudo, &payload).as_bytes())
}

// ---- handlers --------------------------------------------------------------------------------

/// Redeems a mailed enrolment code for a device token. The code proves control of the mailbox and the password proves
/// identity, so a native app can become a known device even while the account is locked down. Codes are single use,
/// expire after 15 minutes, are bound to the account and its device epoch, are leased before the hash (one hash per
/// code at a time), and die after ENROL_MAX_FAILURES wrong passwords. Only an unknown code is charged to the IP.
pub fn enrol(c: &Ctx, h: &HeaderMap, client: &ClientInfo, body: &Map<String, Value>) -> Result<Response, HttpError> {
    let code = match body.get("code").and_then(Value::as_str) {
        Some(x) if is_token(x) => x,
        _ => return Err(HttpError::new(400, "invalid_token")),
    };
    let password = read_password(body, "password")?;
    let ip = guard::client_ip(h)?;
    let ch = sha256(code);
    let row: Option<(String, String, String, i64)> = lock(c)
        .query_row(
            "SELECT e.id, e.user_id, u.pw_hash, u.device_epoch FROM device_enrolments e JOIN users u ON u.id = e.user_id
             WHERE e.code_hash = ?1 AND e.used_at IS NULL AND e.expires_at > ?2 AND e.epoch = u.device_epoch
               AND e.failures < ?3 AND u.deleted_at IS NULL AND u.suspended_at IS NULL AND u.pw_hash IS NOT NULL",
            params![ch, c.now, ENROL_MAX_FAILURES],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)),
        )
        .optional()
        .map_err(dberr)?;
    let Some((id, user_id, pw_hash, epoch)) = row else {
        guard::limit(&lock(c), c.keys, Action::Enrol, &ip, None, c.now)?;
        return Err(HttpError::new(400, "invalid_token"));
    };
    // Every condition is re-checked inside the claim itself, so nothing that changed since the read above (more wrong
    // passwords, expiry, a reset bumping the epoch) can slip a hash through.
    {
        let conn = lock(c);
        let claimed: Option<String> = conn
            .query_row(
                "UPDATE device_enrolments SET claimed_until = ?3
                 WHERE id = ?1 AND used_at IS NULL AND (claimed_until IS NULL OR claimed_until <= ?2)
                   AND failures < ?4 AND expires_at > ?2
                   AND epoch = (SELECT device_epoch FROM users WHERE id = device_enrolments.user_id AND deleted_at IS NULL)
                 RETURNING id",
                params![id, c.now, c.now + LEASE_MS, ENROL_MAX_FAILURES],
                |r| r.get(0),
            )
            .optional()
            .map_err(dberr)?;
        if claimed.is_none() {
            let held: Option<i64> = conn.query_row("SELECT 1 FROM device_enrolments WHERE id = ?1 AND used_at IS NULL AND claimed_until > ?2", params![id, c.now], |r| r.get(0)).optional().map_err(dberr)?;
            return Err(if held.is_some() { HttpError::new(409, "in_progress") } else { HttpError::new(400, "invalid_token") });
        }
    }
    let ok = verify_password(&pw_hash, &password);
    if !ok {
        let _ = lock(c).execute("UPDATE device_enrolments SET claimed_until = NULL, failures = failures + 1 WHERE id = ?1", [&id]);
        return Err(HttpError::new(401, "invalid_credentials"));
    }
    let conn = lock(c);
    let used: Option<String> = conn
        .query_row(
            "UPDATE device_enrolments SET used_at = ?2, claimed_until = NULL
             WHERE id = ?1 AND used_at IS NULL
               AND EXISTS (SELECT 1 FROM users WHERE id = ?3 AND pw_hash = ?4 AND device_epoch = ?5 AND deleted_at IS NULL)
             RETURNING id",
            params![id, c.now, user_id, pw_hash, epoch],
            |r| r.get(0),
        )
        .optional()
        .map_err(dberr)?;
    if used.is_none() {
        return Err(HttpError::new(400, "invalid_token"));
    }
    tracing::info!(user = user_id.as_str(), "auth: device enrolled");
    with_device(c, &conn, json(200, &ok_body(), &[]), client, &ok_body(), &user_id, epoch)
}

/// A signed-in owner on a known device asks for a fresh enrolment code, to add another device (a third laptop, a new
/// phone) while the account is locked down. It needs a live access token AND a valid device token for the account, is
/// held to the mail gap and a daily cap, mails the code to the account's address, and does NOT bump the epoch, so
/// existing devices keep working.
pub fn enrolment_code(c: &Ctx, h: &HeaderMap, client: &ClientInfo, out: &mut Outbox) -> Result<Response, HttpError> {
    let p = require_principal(c, h)?;
    guard::client_ip(h)?;
    let u: Option<(String, String, i64)> = lock(c)
        .query_row("SELECT id, email, device_epoch FROM users WHERE id = ?1 AND deleted_at IS NULL", [&p.user_id], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))
        .optional()
        .map_err(dberr)?;
    let Some((id, email, epoch)) = u else { return Err(HttpError::new(401, "unauthorized")) };
    if valid_device(c, h, client, Some(&DeviceUser { id: &id, device_epoch: epoch })).is_none() {
        return Err(HttpError::new(403, "known_device_required"));
    }
    if !issue_enrolment(c, &lock(c), &id, &email, epoch, true, out)? {
        return Err(HttpError::new(429, "rate_limited").header("Retry-After", &((c.cfg.mail_gap_ms + 999) / 1000).to_string()));
    }
    Ok(json(202, &ok_body(), &[]))
}

/// The sudo step: a signed-in user on a known device re-enters the password and gets a 5-minute sudo token bound to this
/// session family and device epoch. Admin writes need it (X-Sudo-Token), so a stolen plain access token can never
/// approve, reject, suspend, restore or revoke.
pub fn reauth(c: &Ctx, h: &HeaderMap, client: &ClientInfo, body: &Map<String, Value>) -> Result<Response, HttpError> {
    let p = require_principal(c, h)?;
    let password = read_password(body, "password")?;
    let ip = guard::client_ip(h)?;
    // Only wrong passwords count (an active admin re-authenticates every few minutes).
    let refund = guard::limit(&lock(c), c.keys, Action::Reauth, &ip, Some(&format!("user:{}", p.user_id)), c.now)?;
    let u: Option<(String, Option<String>, i64)> = lock(c)
        .query_row("SELECT id, pw_hash, device_epoch FROM users WHERE id = ?1 AND deleted_at IS NULL AND suspended_at IS NULL", [&p.user_id], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))
        .optional()
        .map_err(dberr)?;
    let Some((id, Some(pw_hash), epoch)) = u else { return Err(HttpError::new(401, "unauthorized")) };
    if valid_device(c, h, client, Some(&DeviceUser { id: &id, device_epoch: epoch })).is_none() {
        refund.refund(&lock(c));
        return Err(HttpError::new(403, "known_device_required"));
    }
    if !verify_password(&pw_hash, &password) {
        return Err(HttpError::new(401, "invalid_credentials"));
    }
    refund.refund(&lock(c));
    let (token, expires_in) = issue_sudo(c.keys, &p, epoch, c.now);
    tracing::info!(user = p.user_id.as_str(), "auth: reauthenticated");
    Ok(json(200, &json!({ "sudo_token": token, "expires_in": expires_in }), &[]))
}

#[cfg(test)]
#[path = "device_tests.rs"]
mod tests;
