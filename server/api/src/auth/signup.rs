//! Sign-up and the two mailed-link requests, resend and forgot (port of `signup`, `readProfile`, `policy` and
//! `issueMailToken` in auth/src/auth.ts, with the pool Worker's `profile` hook from worker/src/index.ts baked in).
//! Every answer is the same 202 whether or not the address has an account, so none of these reveal which do.
use super::config::Action;
use super::crypto::{random_token, sha256, uuid_v4};
use super::guard;
use super::http::{HttpError, json, normalize_email, str_field};
use super::mail::{MailKind, MailMessage, Outbox};
use super::password::{check_password_policy, hash_password, params_json};
use super::session::{Ctx, read_password};
use super::validate::{clean_text, profile_is_safe};
use axum::response::Response;
use rusqlite::{OptionalExtension, params};
use serde_json::{Map, Value, json};
use std::collections::HashSet;

pub const OK_BODY: &str = r#"{"ok":true}"#;

pub fn ok_body() -> Value {
    json!({ "ok": true })
}

fn dberr(e: rusqlite::Error) -> HttpError {
    tracing::error!("auth: database error: {e}");
    HttpError::new(500, "server_error")
}

/// `new Date().toISOString()` for epoch milliseconds.
pub fn iso8601(ms: i64) -> String {
    let (secs, milli) = (ms.div_euclid(1000), ms.rem_euclid(1000));
    let (days, rem) = (secs.div_euclid(86400), secs.rem_euclid(86400));
    // Civil date from days since 1970-01-01 (Howard Hinnant's algorithm).
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = yoe + era * 400 + i64::from(m <= 2);
    format!("{y:04}-{m:02}-{d:02}T{:02}:{:02}:{:02}.{milli:03}Z", rem / 3600, rem % 3600 / 60, rem % 60)
}

pub struct Profile {
    pub display_name: Option<String>,
    pub profile: Option<Value>,
}

fn input_error(code: &str, field: &str) -> HttpError {
    HttpError::new(400, "invalid_request").with("field", json!(field)).with("reason", json!(code))
}

/// Profile fields. The pool asks for a display name and school (plain text, no control or format characters, whitespace
/// collapsed) and the adult-school-employee statement (COPPA/SOPIPA). Then the package's own checks run again: the name
/// is at most 100 characters and every string in the profile JSON is free of control and format characters, and small.
pub fn read_profile(body: &Map<String, Value>, now: i64) -> Result<Profile, HttpError> {
    let name = body.get("display_name").and_then(|v| clean_text(v, 100)).ok_or_else(|| input_error("required_plain_text", "display_name"))?;
    let school = body.get("school").and_then(|v| clean_text(v, 200)).ok_or_else(|| input_error("required_plain_text", "school"))?;
    if body.get("adult_school_employee") != Some(&Value::Bool(true)) {
        return Err(input_error("must_confirm", "adult_school_employee"));
    }
    let profile = json!({ "school": school, "adult_school_employee_at": iso8601(now) });
    let display_name = clean_text(&json!(name), 100).ok_or_else(|| input_error("invalid_characters_or_length", "display_name"))?;
    if !profile_is_safe(&profile) {
        return Err(input_error("invalid_characters_or_length", "profile"));
    }
    Ok(Profile { display_name: Some(display_name), profile: Some(profile) })
}

pub fn policy(c: &Ctx, password: &str, email: &str) -> Result<(), HttpError> {
    match check_password_policy(password, email, c.cfg.min_password, &HashSet::new()) {
        Some(why) => Err(HttpError::new(400, "weak_password").with("reason", json!(why))),
        None => Ok(()),
    }
}

pub fn signup(c: &Ctx, h: &axum::http::HeaderMap, body: &Map<String, Value>, out: &mut Outbox) -> Result<Response, HttpError> {
    let email = normalize_email(&str_field(body, "email", 320, true)?.unwrap_or_default())?;
    let password = read_password(body, "password")?;
    let prof = read_profile(body, c.now)?;
    policy(c, &password, &email)?;
    let ip = guard::client_ip(h)?;
    guard::limit(&c.db.lock().unwrap_or_else(|p| p.into_inner()), c.keys, Action::Signup, &ip, Some(&format!("email:{email}")), c.now)?;
    // Always hash, whether or not the address has an account, so timing is the same.
    let hashed = hash_password(&password, c.cfg.argon2).map_err(|_| HttpError::new(500, "server_error"))?;
    let user_id = uuid_v4();
    let token = random_token();
    let expires_at = c.now + c.cfg.verify_ttl * 1000;
    let (inserted, existing_verified) = {
        let mut conn = c.db.lock().unwrap_or_else(|p| p.into_inner());
        let tx = conn.transaction().map_err(dberr)?;
        // One transaction in both cases: the insert is a no-op for an existing address.
        let ins = tx
            .execute(
                "INSERT INTO users (id, email, pw_hash, pw_params, display_name, profile, trust, created_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8) ON CONFLICT (email) DO NOTHING",
                params![user_id, email, hashed.phc, params_json(&hashed.params), prof.display_name, prof.profile.as_ref().map(Value::to_string), super::config::TRUST[0], c.now],
            )
            .map_err(dberr)?;
        tx.execute(
            "INSERT INTO email_tokens (id, user_id, kind, token_hash, created_at, expires_at) SELECT ?1, id, 'verify', ?2, ?3, ?4 FROM users WHERE id = ?5",
            params![uuid_v4(), sha256(&token), c.now, expires_at, user_id],
        )
        .map_err(dberr)?;
        let existing: Option<Option<i64>> = tx.query_row("SELECT email_verified_at FROM users WHERE email = ?1", [&email], |r| r.get(0)).optional().map_err(dberr)?;
        tx.commit().map_err(dberr)?;
        (ins == 1, existing.is_some_and(|v| v.is_some()))
    };
    if inserted {
        out.push(MailMessage::new(MailKind::Verify, &email).with_token(&token, expires_at));
        tracing::info!(user = user_id.as_str(), "auth: signup");
    } else {
        let mut m = MailMessage::new(MailKind::SignupExisting, &email);
        m.account_verified = Some(existing_verified);
        out.push(m);
    }
    Ok(json(202, &ok_body(), &[]))
}

/// Shared body of resend and forgot: one transaction whatever the address, and always the same reply. There is no
/// per-IP cap (no hashing happens here, and a per-IP cap could be filled from a shared school address with random
/// addresses to block the owner). The only rate limit is per (IP, address). Per account there is no 429 to fill up: a
/// mail goes out unless one of this kind went out in the last `mail_gap` or `mail_ceiling` went out within one link
/// lifetime, and the newest `outstanding_links` links all stay valid.
fn issue_mail_token(c: &Ctx, h: &axum::http::HeaderMap, body: &Map<String, Value>, action: Action, kind: MailKind, user_filter: &str, ttl_seconds: i64, out: &mut Outbox) -> Result<Response, HttpError> {
    let email = normalize_email(&str_field(body, "email", 320, true)?.unwrap_or_default())?;
    let ip = guard::client_ip(h)?;
    let mut conn = c.db.lock().unwrap_or_else(|p| p.into_inner());
    guard::limit(&conn, c.keys, action, &ip, Some(&format!("email:{email}")), c.now)?;
    let token = random_token();
    let expires_at = c.now + ttl_seconds * 1000;
    let kind_s = if kind == MailKind::Verify { "verify" } else { "reset" };
    let target = format!("SELECT id FROM users WHERE email = ?1 AND deleted_at IS NULL {user_filter}");
    let tx = conn.transaction().map_err(dberr)?;
    let user_id: Option<String> = tx
        .query_row(
            &format!(
                "INSERT INTO email_tokens (id, user_id, kind, token_hash, created_at, expires_at)
                 SELECT ?2, u.id, ?3, ?4, ?5, ?6 FROM users u WHERE u.email = ?1 AND u.deleted_at IS NULL {user_filter}
                   AND NOT EXISTS (SELECT 1 FROM email_tokens g WHERE g.user_id = u.id AND g.kind = ?3 AND g.created_at > ?5 - ?7)
                   AND (SELECT COUNT(*) FROM email_tokens d WHERE d.user_id = u.id AND d.kind = ?3 AND d.created_at > ?5 - ?8) < ?9
                 RETURNING user_id"
            ),
            params![email, uuid_v4(), kind_s, sha256(&token), c.now, expires_at, c.cfg.mail_gap_ms, ttl_seconds * 1000, c.cfg.mail_ceiling],
            |r| r.get(0),
        )
        .optional()
        .map_err(dberr)?;
    // Only the newest few links work; older ones are retired.
    tx.execute(
        &format!(
            "UPDATE email_tokens SET used_at = ?2 WHERE kind = ?3 AND used_at IS NULL AND user_id = ({target})
               AND rowid NOT IN (SELECT rowid FROM email_tokens WHERE kind = ?3 AND used_at IS NULL AND user_id = ({target})
                                 ORDER BY created_at DESC, rowid DESC LIMIT ?4)"
        ),
        params![email, c.now, kind_s, c.cfg.outstanding_links],
    )
    .map_err(dberr)?;
    tx.commit().map_err(dberr)?;
    if let Some(id) = user_id {
        out.push(MailMessage::new(kind, &email).with_token(&token, expires_at));
        if kind == MailKind::Reset {
            tracing::info!(user = id.as_str(), "auth: reset requested");
        }
    }
    Ok(json(202, &ok_body(), &[]))
}

pub fn resend(c: &Ctx, h: &axum::http::HeaderMap, body: &Map<String, Value>, out: &mut Outbox) -> Result<Response, HttpError> {
    issue_mail_token(c, h, body, Action::Resend, MailKind::Verify, "AND email_verified_at IS NULL", c.cfg.verify_ttl, out)
}

/// A reset is allowed for an account not yet active too: completing it proves control of the address, and it is how a
/// real owner takes back an address someone else registered.
pub fn forgot(c: &Ctx, h: &axum::http::HeaderMap, body: &Map<String, Value>, out: &mut Outbox) -> Result<Response, HttpError> {
    issue_mail_token(c, h, body, Action::Forgot, MailKind::Reset, "", c.cfg.reset_ttl, out)
}

#[cfg(test)]
#[path = "signup_tests.rs"]
mod tests;
