//! Completing a mailed link: verify (confirm the address) and reset (set a new password), plus the device enrolment
//! codes they mail (port of `verify`, `reset`, `claimEmailToken`, `activate`, `issueEnrolment` and `withDevice` in
//! auth/src/auth.ts).
//!
//! A valid link is leased for `LEASE_MS` BEFORE any password hash, so N concurrent requests with one link cost one hash;
//! only the holder of the link can see the lease. The consuming statements run in one transaction, keyed by a nonce
//! written on the token row, so each statement sees exactly the user the token belonged to.
use super::config::{Action, DAY, ENROL_DAILY, LEASE_MS, TRUST};
use super::crypto::{is_token, random_token, sha256, uuid_v4};
use super::guard;
use super::http::{ClientInfo, ClientMode, HttpError, clear_cookie, json, refresh_cookie};
use super::mail::{MailKind, MailMessage, Outbox};
use super::password::{hash_password, params_json};
use super::session::{Ctx, read_password};
use super::signup::{ok_body, policy, read_profile};
use axum::http::HeaderMap;
use axum::response::Response;
use rusqlite::{Connection, OptionalExtension, params};
use serde_json::{Map, Value, json};

fn dberr(e: rusqlite::Error) -> HttpError {
    tracing::error!("auth: database error: {e}");
    HttpError::new(500, "server_error")
}

/// Leases a still-valid link. A link that is valid but leased is someone holding it right now (a double click): 409.
fn claim_email_token(conn: &Connection, token_hash: &str, kind: &str, now: i64) -> Result<(), HttpError> {
    let got: Option<String> = conn
        .query_row(
            "UPDATE email_tokens SET claimed_until = ?3
             WHERE token_hash = ?1 AND kind = ?4 AND used_at IS NULL AND expires_at > ?2 AND (claimed_until IS NULL OR claimed_until <= ?2)
             RETURNING id",
            params![token_hash, now, now + LEASE_MS, kind],
            |r| r.get(0),
        )
        .optional()
        .map_err(dberr)?;
    if got.is_some() {
        return Ok(());
    }
    let held: Option<i64> = conn.query_row("SELECT 1 FROM email_tokens WHERE token_hash = ?1 AND used_at IS NULL AND claimed_until > ?2", params![token_hash, now], |r| r.get(0)).optional().map_err(dberr)?;
    Err(if held.is_some() { HttpError::new(409, "in_progress") } else { HttpError::new(400, "invalid_token") })
}

fn release_email_token(conn: &Connection, token_hash: &str) {
    let _ = conn.execute("UPDATE email_tokens SET claimed_until = NULL WHERE token_hash = ?1 AND used_at IS NULL", [token_hash]);
}

/// Mails a one-time enrolment code that a native app redeems (with the password) for a device token. With `limited`,
/// the insert itself enforces the mail gap and the daily cap, and returns whether a code was issued.
pub fn issue_enrolment(c: &Ctx, conn: &Connection, user_id: &str, email: &str, epoch: i64, limited: bool, out: &mut Outbox) -> Result<bool, HttpError> {
    let code = random_token();
    let expires_at = c.now + c.cfg.enrol_ttl * 1000;
    let guard_sql = if limited {
        "WHERE NOT EXISTS (SELECT 1 FROM device_enrolments g WHERE g.user_id = ?2 AND g.created_at > ?5 - ?7)
           AND (SELECT COUNT(*) FROM device_enrolments d WHERE d.user_id = ?2 AND d.created_at > ?5 - ?8) < ?9"
    } else {
        ""
    };
    let sql = format!("INSERT INTO device_enrolments (id, user_id, code_hash, epoch, created_at, expires_at) SELECT ?1, ?2, ?3, ?4, ?5, ?6 {guard_sql} RETURNING id");
    let id = uuid_v4();
    let hash = sha256(&code);
    let got: Option<String> = if limited {
        conn.query_row(&sql, params![id, user_id, hash, epoch, c.now, expires_at, c.cfg.mail_gap_ms, DAY * 1000, ENROL_DAILY], |r| r.get(0))
    } else {
        conn.query_row(&sql, params![id, user_id, hash, epoch, c.now, expires_at], |r| r.get(0))
    }
    .optional()
    .map_err(dberr)?;
    if got.is_none() {
        return Ok(false);
    }
    out.push(MailMessage::new(MailKind::DeviceEnrol, email).with_token(&code, expires_at));
    Ok(true)
}

/// Puts a freshly issued device token on a response and records it as the account's current device: web cookie, or
/// native body field.
pub fn with_device(c: &Ctx, conn: &Connection, mut res: Response, client: &ClientInfo, body: &Value, user_id: &str, epoch: i64) -> Result<Response, HttpError> {
    let d = guard::issue_device_token(c.keys, user_id, epoch, c.now, c.cfg.device_ttl);
    conn.execute("UPDATE users SET last_device = ?1 WHERE id = ?2 AND device_epoch = ?3", params![d.nonce, user_id, epoch]).map_err(dberr)?;
    if client.mode == ClientMode::Native {
        let mut b = body.clone();
        b["device_token"] = json!(d.token);
        let mut out = json(res.status().as_u16(), &b, &[]);
        for (k, v) in res.headers() {
            if k != "content-type" && k != "content-length" {
                out.headers_mut().append(k.clone(), v.clone());
            }
        }
        return Ok(out);
    }
    if let Ok(v) = refresh_cookie(&c.cfg.device_cookie_name, &d.token, c.cfg.device_ttl).parse() {
        res.headers_mut().append("set-cookie", v);
    }
    Ok(res)
}

/// Confirms the address of the account picked by `user_sql` (idempotent), then moves it from the initial trust level
/// to the after-verify one and opens its approval request. Returns whether this call activated the account.
fn confirm_and_maybe_activate(tx: &rusqlite::Transaction, nonce: &str, now: i64, activate: bool) -> Result<bool, HttpError> {
    let consumed = "SELECT user_id FROM email_tokens WHERE used_nonce = ?1";
    tx.execute(&format!("UPDATE users SET email_verified_at = COALESCE(email_verified_at, ?2) WHERE id = ({consumed}) AND deleted_at IS NULL"), params![nonce, now]).map_err(dberr)?;
    if !activate {
        return Ok(false);
    }
    let n = tx
        .execute(
            &format!(
                "UPDATE users SET trust = ?2 WHERE id = ({consumed}) AND deleted_at IS NULL AND trust = ?3 AND email_verified_at IS NOT NULL AND pw_hash IS NOT NULL"
            ),
            params![nonce, super::config::AFTER_VERIFY, TRUST[0]],
        )
        .map_err(dberr)?;
    if n == 1 {
        tx.execute(
            &format!(
                "INSERT OR IGNORE INTO approval_requests (id, user_id, requested_at, email_domain, display_name_snapshot, profile_snapshot)
                 SELECT ?2, id, ?3, substr(email, instr(email, '@') + 1), display_name, profile
                 FROM users WHERE id = ({consumed}) AND deleted_at IS NULL AND trust = ?4
                   AND NOT EXISTS (SELECT 1 FROM approval_requests WHERE user_id = users.id)"
            ),
            params![nonce, uuid_v4(), now, super::config::AFTER_VERIFY],
        )
        .map_err(dberr)?;
    }
    Ok(n == 1)
}

fn token_field(body: &Map<String, Value>) -> Result<String, HttpError> {
    match body.get("token").and_then(Value::as_str) {
        Some(t) if is_token(t) => Ok(t.to_string()),
        _ => Err(HttpError::new(400, "invalid_token")),
    }
}

fn lock<'a>(c: &'a Ctx) -> std::sync::MutexGuard<'a, Connection> {
    c.db.lock().unwrap_or_else(|p| p.into_inner())
}

/// Confirms an address. Whoever registered the account may not be whoever owns the mailbox (a student can sign up with
/// a teacher's address), so confirming discards everything chosen before confirmation: the password, the display name
/// and profile, every session and device token. The person who clicked the link either sends `password` plus the
/// profile fields with the token (activating the account in one step) or, without them, completes it through
/// forgot/reset.
pub fn verify(c: &Ctx, h: &HeaderMap, client: &ClientInfo, body: &Map<String, Value>, out: &mut Outbox) -> Result<Response, HttpError> {
    let token = token_field(body)?;
    let password = match body.get("password") {
        None | Some(Value::Null) => None,
        Some(_) => Some(read_password(body, "password")?),
    };
    // Choosing a password here means entering the profile here too (the old one may be a student's).
    let prof = match &password {
        Some(_) => Some(read_profile(body, c.now)?),
        None => None,
    };
    let ip = guard::client_ip(h)?;
    let th = sha256(&token);
    // The token is looked up first (by its hash). Only an invalid token is charged to the IP, so junk from a shared
    // school address can never make a valid link fail.
    let email: Option<String> = lock(c)
        .query_row(
            "SELECT u.email FROM email_tokens t JOIN users u ON u.id = t.user_id
             WHERE t.token_hash = ?1 AND t.kind = 'verify' AND t.used_at IS NULL AND t.expires_at > ?2 AND u.deleted_at IS NULL",
            params![th, c.now],
            |r| r.get(0),
        )
        .optional()
        .map_err(dberr)?;
    let Some(email) = email else {
        guard::limit(&lock(c), c.keys, Action::Verify, &ip, None, c.now)?;
        return Err(HttpError::new(400, "invalid_token"));
    };
    let mut leased = false;
    let result = (|| -> Result<Response, HttpError> {
        let hashed = match &password {
            Some(pw) => {
                policy(c, pw, &email)?;
                claim_email_token(&lock(c), &th, "verify", c.now)?; // concurrent duplicates stop here, unhashed
                leased = true;
                Some(hash_password(pw, c.cfg.argon2).map_err(|_| HttpError::new(500, "server_error"))?)
            }
            None => None,
        };
        let nonce = uuid_v4();
        let mut conn = lock(c);
        let tx = conn.transaction().map_err(dberr)?;
        let user_id: Option<String> = tx
            .query_row(
                "UPDATE email_tokens SET used_at = ?3, used_nonce = ?1 WHERE token_hash = ?2 AND kind = 'verify' AND used_at IS NULL AND expires_at > ?3 RETURNING user_id",
                params![nonce, th, c.now],
                |r| r.get(0),
            )
            .optional()
            .map_err(dberr)?;
        let Some(user_id) = user_id else { return Err(HttpError::new(400, "invalid_token")) };
        // Everything set before confirmation is replaced (or dropped), in one statement.
        let epoch: Option<i64> = tx
            .query_row(
                "UPDATE users SET pw_hash = ?2, pw_params = ?3, display_name = ?4, profile = ?5, device_epoch = device_epoch + 1
                 WHERE id = (SELECT user_id FROM email_tokens WHERE used_nonce = ?1) AND deleted_at IS NULL AND email_verified_at IS NULL RETURNING device_epoch",
                params![
                    nonce,
                    hashed.as_ref().map(|x| x.phc.clone()),
                    hashed.as_ref().map(|x| params_json(&x.params)),
                    prof.as_ref().and_then(|p| p.display_name.clone()),
                    prof.as_ref().and_then(|p| p.profile.as_ref().map(Value::to_string)),
                ],
                |r| r.get(0),
            )
            .optional()
            .map_err(dberr)?;
        let activated = confirm_and_maybe_activate(&tx, &nonce, c.now, true)?;
        // Any session from before confirmation ends, and every other outstanding link stops working too, including
        // reset links requested before confirmation.
        tx.execute("UPDATE sessions SET revoked_at = ?2 WHERE revoked_at IS NULL AND user_id = (SELECT user_id FROM email_tokens WHERE used_nonce = ?1)", params![nonce, c.now]).map_err(dberr)?;
        tx.execute("UPDATE email_tokens SET used_at = ?2 WHERE used_at IS NULL AND user_id = (SELECT user_id FROM email_tokens WHERE used_nonce = ?1)", params![nonce, c.now]).map_err(dberr)?;
        tx.commit().map_err(dberr)?;
        leased = false; // consumed
        tracing::info!(user = user_id.as_str(), "auth: verified");
        let reply = json!({ "ok": true, "password_set": hashed.is_some() });
        let res = json(200, &reply, &[]);
        if !activated {
            return Ok(res);
        }
        tracing::info!(user = user_id.as_str(), "auth: activated");
        out.push(MailMessage::new(MailKind::NewApplication, &user_id));
        // The mailbox owner just set the password: the epoch bump re-keyed every failure counter, this becomes a known
        // device, and an enrolment code is mailed for the owner's other devices (the native app).
        let Some(epoch) = epoch else { return Ok(res) };
        issue_enrolment(c, &conn, &user_id, &email, epoch, false, out)?;
        with_device(c, &conn, res, client, &reply, &user_id, epoch)
    })();
    if leased {
        release_email_token(&lock(c), &th);
    }
    result
}

pub fn reset(c: &Ctx, h: &HeaderMap, client: &ClientInfo, body: &Map<String, Value>, out: &mut Outbox) -> Result<Response, HttpError> {
    let token = token_field(body)?;
    let password = read_password(body, "password")?;
    let ip = guard::client_ip(h)?;
    let th = sha256(&token);
    // Looked up first: a bad token never costs a hash, and only a bad token is charged to the IP.
    let row: Option<(String, String, String)> = lock(c)
        .query_row(
            "SELECT t.user_id, u.email, u.trust FROM email_tokens t JOIN users u ON u.id = t.user_id
             WHERE t.token_hash = ?1 AND t.kind = 'reset' AND t.used_at IS NULL AND t.expires_at > ?2 AND u.deleted_at IS NULL",
            params![th, c.now],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
        )
        .optional()
        .map_err(dberr)?;
    let Some((_, email, trust)) = row else {
        guard::limit(&lock(c), c.keys, Action::Reset, &ip, None, c.now)?;
        return Err(HttpError::new(400, "invalid_token"));
    };
    // An account not yet active gets its profile from the mailbox owner here, never from sign-up.
    let needs_profile = trust == TRUST[0];
    let prof = if needs_profile { Some(read_profile(body, c.now)?) } else { None };
    policy(c, &password, &email)?;
    claim_email_token(&lock(c), &th, "reset", c.now)?; // concurrent duplicates stop here, unhashed
    let mut leased = true;
    let result = (|| -> Result<Response, HttpError> {
        let hashed = hash_password(&password, c.cfg.argon2).map_err(|_| HttpError::new(500, "server_error"))?;
        let nonce = uuid_v4();
        let mut conn = lock(c);
        let tx = conn.transaction().map_err(dberr)?;
        let user_id: Option<String> = tx
            .query_row(
                "UPDATE email_tokens SET used_at = ?3, used_nonce = ?1 WHERE token_hash = ?2 AND kind = 'reset' AND used_at IS NULL AND expires_at > ?3 RETURNING user_id",
                params![nonce, th, c.now],
                |r| r.get(0),
            )
            .optional()
            .map_err(dberr)?;
        let Some(user_id) = user_id else { return Err(HttpError::new(400, "invalid_token")) };
        let epoch: Option<i64> = tx
            .query_row(
                "UPDATE users SET pw_hash = ?2, pw_params = ?3, display_name = COALESCE(?4, display_name), profile = COALESCE(?5, profile), device_epoch = device_epoch + 1
                 WHERE id = (SELECT user_id FROM email_tokens WHERE used_nonce = ?1) AND deleted_at IS NULL RETURNING device_epoch",
                params![
                    nonce,
                    hashed.phc,
                    params_json(&hashed.params),
                    prof.as_ref().and_then(|p| p.display_name.clone()),
                    prof.as_ref().and_then(|p| p.profile.as_ref().map(Value::to_string)),
                ],
                |r| r.get(0),
            )
            .optional()
            .map_err(dberr)?;
        let activated = confirm_and_maybe_activate(&tx, &nonce, c.now, needs_profile)?;
        // Every session ends, on every device, and every other outstanding link (reset or verify) stops working.
        tx.execute("UPDATE sessions SET revoked_at = ?2 WHERE revoked_at IS NULL AND user_id = (SELECT user_id FROM email_tokens WHERE used_nonce = ?1)", params![nonce, c.now]).map_err(dberr)?;
        tx.execute("UPDATE email_tokens SET used_at = ?2 WHERE used_at IS NULL AND user_id = (SELECT user_id FROM email_tokens WHERE used_nonce = ?1)", params![nonce, c.now]).map_err(dberr)?;
        tx.commit().map_err(dberr)?;
        leased = false; // consumed
        tracing::info!(user = user_id.as_str(), "auth: password reset");
        if activated {
            tracing::info!(user = user_id.as_str(), "auth: activated");
            out.push(MailMessage::new(MailKind::NewApplication, &user_id));
        }
        out.push(MailMessage::new(MailKind::PasswordChanged, &email));
        // Completing a reset proves control of the mailbox, so it restores login even during a lockdown: the epoch
        // bump re-keyed every failure counter and killed old device tokens; this device becomes known, and an
        // enrolment code is mailed for the owner's other devices.
        let mut res = json(200, &ok_body(), &[]);
        if client.mode == ClientMode::Web {
            if let Ok(v) = clear_cookie(&c.cfg.cookie_name).parse() {
                res.headers_mut().append("set-cookie", v);
            }
        }
        let Some(epoch) = epoch else { return Ok(res) };
        issue_enrolment(c, &conn, &user_id, &email, epoch, false, out)?;
        with_device(c, &conn, res, client, &ok_body(), &user_id, epoch)
    })();
    if leased {
        release_email_token(&lock(c), &th);
    }
    result
}

#[cfg(test)]
#[path = "tokens_tests.rs"]
mod tests;
