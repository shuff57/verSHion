//! Sessions: login, refresh rotation with the reuse grace, logout, and `authenticate` (port of the handlers of the
//! same names in auth/src/auth.ts). All of it is synchronous SQLite behind a mutex that is NEVER held while a password
//! is hashed; the router runs these on a blocking thread.
//!
//! A login starts a session *family*; each refresh adds the successor row. The successor token is derived, never
//! stored: HMAC(refresh key, predecessor). Presenting an already-rotated token outside the grace window means it was
//! copied, and the whole family is ended.
use super::config::{Action, AuthConfig, TRUST, UA_MAX};
use super::crypto::{Keys, hmac, is_token, random_token, sha256, uuid_v4};
use super::guard::{self, Device, DeviceUser, Refund};
use super::http::{ClientInfo, ClientMode, HttpError, bearer, clear_cookie, empty, json, normalize_email, read_cookie, refresh_cookie, str_field};
use super::jwt::{AccessClaims, Expect, sign_access_token, verify_access_token};
use super::password::{MAX_PASSWORD_CHARS, dummy_verify, hash_password, normalize_password, parse_phc, params_json, same_params, verify_password};
use axum::http::HeaderMap;
use axum::response::Response;
use rusqlite::{Connection, OptionalExtension, params};
use serde_json::{Map, Value, json};
use std::sync::{Mutex, MutexGuard};

#[derive(Debug, Clone, PartialEq)]
pub struct Principal {
    pub user_id: String,
    pub session_family: String,
    pub trust: String,
}

pub struct Ctx<'a> {
    pub db: &'a Mutex<Connection>,
    pub cfg: &'a AuthConfig,
    pub keys: &'a Keys,
    /// Epoch milliseconds.
    pub now: i64,
}

impl Ctx<'_> {
    fn conn(&self) -> MutexGuard<'_, Connection> {
        self.db.lock().unwrap_or_else(|p| p.into_inner())
    }
}

fn dberr(e: rusqlite::Error) -> HttpError {
    tracing::error!("auth: database error: {e}");
    HttpError::new(500, "server_error")
}

pub fn user_agent(h: &HeaderMap) -> Option<String> {
    h.get("user-agent").and_then(|v| v.to_str().ok()).filter(|s| !s.is_empty()).map(|s| s.encode_utf16().take(UA_MAX).collect::<Vec<u16>>()).map(|u| String::from_utf16_lossy(&u))
}

/// The password field, normalised. Bounded before normalising; NFKC can grow a string.
pub fn read_password(body: &Map<String, Value>, key: &str) -> Result<String, HttpError> {
    Ok(normalize_password(&str_field(body, key, MAX_PASSWORD_CHARS * 4, true)?.unwrap_or_default()))
}

// ---- tokens ----------------------------------------------------------------------------------

pub(crate) fn access_token(c: &Ctx, user_id: &str, family: &str, trust: &str) -> String {
    let iat = c.now / 1000;
    sign_access_token(
        &c.keys.access,
        &AccessClaims { iss: c.cfg.issuer.clone(), aud: c.cfg.audience.clone(), sub: user_id.into(), sid: family.into(), trust: trust.into(), iat, exp: iat + c.cfg.access_ttl, jti: uuid_v4() },
    )
}

/// Web: the refresh token only in the cookie. Native: in the body (and the device token with it).
pub fn token_response(c: &Ctx, client: &ClientInfo, user_id: &str, trust: &str, family: &str, refresh: &str, refresh_expires_at: i64, device_token: Option<&str>) -> Response {
    let mut body = json!({
        "access_token": access_token(c, user_id, family, trust),
        "token_type": "Bearer",
        "expires_in": c.cfg.access_ttl,
        "user": { "id": user_id, "trust": trust },
    });
    let max_age = ((refresh_expires_at - c.now) / 1000).max(0);
    if client.mode == ClientMode::Native {
        body["refresh_token"] = json!(refresh);
        body["refresh_expires_in"] = json!(max_age);
        if let Some(d) = device_token {
            body["device_token"] = json!(d); // keep in the keychain; send as X-Device-Token
        }
        return json(200, &body, &[]);
    }
    let cookie = refresh_cookie(&c.cfg.cookie_name, refresh, max_age);
    let mut res = json(200, &body, &[("set-cookie", &cookie)]);
    if let Some(d) = device_token {
        if let Ok(v) = refresh_cookie(&c.cfg.device_cookie_name, d, c.cfg.device_ttl).parse() {
            res.headers_mut().append("set-cookie", v);
        }
    }
    res
}

fn refresh_failure(c: &Ctx, client: &ClientInfo) -> HttpError {
    let e = HttpError::new(401, "invalid_refresh_token");
    if client.mode == ClientMode::Web { e.header("Set-Cookie", &clear_cookie(&c.cfg.cookie_name)) } else { e }
}

/// Cookie-carrying endpoints in web mode must come from an allowed browser origin (CSRF defence in depth).
pub fn require_origin_for_cookie(client: &ClientInfo) -> Result<(), HttpError> {
    if client.mode == ClientMode::Web && client.origin.is_none() { Err(HttpError::new(403, "origin_required")) } else { Ok(()) }
}

/// Web clients never send the token in a body and native clients never use the cookie.
fn presented_refresh(c: &Ctx, h: &HeaderMap, client: &ClientInfo, body: Option<&Map<String, Value>>) -> Option<String> {
    let raw = if client.mode == ClientMode::Native { body.and_then(|b| b.get("refresh_token")).and_then(Value::as_str).map(str::to_string) } else { read_cookie(h, &c.cfg.cookie_name) };
    raw.filter(|r| is_token(r))
}

// ---- authenticate ----------------------------------------------------------------------------

/// Checks the bearer access token, then that its session has not been revoked and the account still exists (one indexed
/// read), and returns the trust level as it is now in the database. There is deliberately no token-only mode: nothing
/// can read a stale trust level or a revoked session.
pub fn authenticate(c: &Ctx, h: &HeaderMap) -> Option<Principal> {
    let tok = bearer(h)?;
    let claims = verify_access_token(&c.keys.access, &tok, &Expect { iss: &c.cfg.issuer, aud: &c.cfg.audience, now_seconds: c.now / 1000 })?;
    let trust: Option<String> = c
        .conn()
        .query_row(
            "SELECT u.trust FROM sessions s JOIN users u ON u.id = s.user_id
             WHERE s.family_id = ?1 AND s.user_id = ?2 AND s.revoked_at IS NULL AND s.rotated_at IS NULL
               AND s.expires_at > ?3 AND u.deleted_at IS NULL AND u.suspended_at IS NULL LIMIT 1",
            params![claims.sid, claims.sub, c.now],
            |r| r.get(0),
        )
        .optional()
        .unwrap_or_else(|e| {
            tracing::error!("auth: authenticate failed: {e}");
            None
        });
    trust.map(|trust| Principal { user_id: claims.sub, session_family: claims.sid, trust })
}

pub fn require_principal(c: &Ctx, h: &HeaderMap) -> Result<Principal, HttpError> {
    authenticate(c, h).ok_or_else(|| HttpError::new(401, "unauthorized").header("WWW-Authenticate", r#"Bearer error="invalid_token""#))
}

// ---- login -----------------------------------------------------------------------------------

struct LoginUser {
    id: String,
    pw_hash: Option<String>,
    trust: String,
    email_verified_at: Option<i64>,
    device_epoch: i64,
    last_device: Option<String>,
    suspended_at: Option<i64>,
}

pub fn login(c: &Ctx, h: &HeaderMap, client: &ClientInfo, body: &Map<String, Value>) -> Result<Response, HttpError> {
    let email = normalize_email(&str_field(body, "email", 320, true)?.unwrap_or_default())?;
    let password = read_password(body, "password")?;
    let account = format!("email:{email}");
    // Per IP every attempt counts (CPU bound); per account only failures, checked before the hash.
    let ip = guard::client_ip(h)?; // fail closed with no client IP, device or not
    let (user, device, refund_ip, release) = {
        let conn = c.conn();
        let user: Option<LoginUser> = conn
            .query_row(
                "SELECT id, pw_hash, trust, email_verified_at, device_epoch, last_device, suspended_at FROM users WHERE email = ?1 AND deleted_at IS NULL",
                [&email],
                |r| Ok(LoginUser { id: r.get(0)?, pw_hash: r.get(1)?, trust: r.get(2)?, email_verified_at: r.get(3)?, device_epoch: r.get(4)?, last_device: r.get(5)?, suspended_at: r.get(6)? }),
            )
            .optional()
            .map_err(dberr)?;
        let du = user.as_ref().map(|u| DeviceUser { id: &u.id, device_epoch: u.device_epoch });
        let device = guard::parse_device(c.keys, guard::presented_device(h, client.mode == ClientMode::Native, c.cfg).as_deref(), du.as_ref(), c.now);
        // The owner's known device is exempt from the per-IP limit (a shared school address may be saturated by
        // others); its own failures are counted per device instead.
        let refund_ip = if device.is_some() { Refund::none() } else { guard::limit(&conn, c.keys, Action::Login, &ip, Some(&account), c.now)? };
        // Reserve a failure slot (refused, without a hash, when over the limits); given back on success. A refusal also
        // refunds the IP hit, so a shared address's budget counts only hashed attempts.
        let release = match guard::reserve_failure(&conn, c.keys, &ip, &account, user.as_ref().map_or(0, |u| u.device_epoch), device.as_ref(), user.as_ref().and_then(|u| u.last_device.as_deref()), c.now) {
            Ok(r) => r,
            Err(e) => {
                refund_ip.refund(&conn);
                return Err(e);
            }
        };
        (user, device, refund_ip, release)
    };
    // Unknown address or no password: a full-cost hash anyway, so every failure takes the same time. Known accounts
    // hash with their own stored parameters.
    let matches = match user.as_ref().and_then(|u| u.pw_hash.as_deref()) {
        Some(phc) => verify_password(phc, &password),
        None => dummy_verify(&password, c.cfg.argon2),
    };
    let logged_id = user.as_ref().map(|u| u.id.clone());
    let fail = || -> HttpError {
        // Every failed login was charged to the per-IP budget like any login; within a separate per-IP "failed login"
        // allowance the charge is given back, so a flood of wrong passwords from a school NAT cannot starve real teachers.
        if device.is_none() {
            guard::refund_failed(&c.conn(), c.keys, &ip, c.now, &refund_ip);
        }
        tracing::info!(user = logged_id.as_deref(), "auth: login failed");
        HttpError::new(401, "invalid_credentials")
    };
    // An account that is not active never logs in: before confirmation its password may belong to someone who does not
    // own the address. Same answer as a wrong password, so this reveals nothing.
    let Some(user) = user.filter(|u| matches && u.pw_hash.is_some() && u.email_verified_at.is_some() && u.trust != TRUST[0]) else { return Err(fail()) };
    let pw_hash = user.pw_hash.clone().unwrap_or_default();
    if user.suspended_at.is_some() {
        // Only reachable with the right password, so it reveals nothing to anyone else.
        release.refund(&c.conn());
        return Err(HttpError::new(403, "account_suspended"));
    }

    let family = uuid_v4();
    let session_id = uuid_v4();
    let refresh = random_token();
    let expires_at = (c.now + c.cfg.refresh_ttl * 1000).min(c.now + c.cfg.session_max * 1000);
    let dev = guard::issue_device_token(c.keys, &user.id, user.device_epoch, c.now, c.cfg.device_ttl);
    // Parameters were raised since this hash was made: upgrade it now that we have the password (outside the lock).
    let upgrade = parse_phc(&pw_hash).filter(|s| !same_params(&s.params, &c.cfg.argon2)).and_then(|_| hash_password(&password, c.cfg.argon2).ok());
    let old_cookie = if client.mode == ClientMode::Web { read_cookie(h, &c.cfg.cookie_name).filter(|o| is_token(o)) } else { None };
    let inserted = {
        let mut conn = c.conn();
        let tx = conn.transaction().map_err(dberr)?;
        // The session exists only if the account is exactly as it was when the password was checked: a reset or
        // deletion that committed in between leaves no row, and the login fails.
        let ins = tx
            .execute(
                "INSERT INTO sessions (id, family_id, user_id, refresh_hash, created_at, auth_at, expires_at, user_agent)
                 SELECT ?1, ?2, id, ?3, ?4, ?4, ?5, ?6 FROM users
                 WHERE id = ?7 AND pw_hash = ?8 AND deleted_at IS NULL AND email_verified_at IS NOT NULL AND trust != ?9 AND suspended_at IS NULL",
                params![session_id, family, sha256(&refresh), c.now, expires_at, user_agent(h), user.id, pw_hash, TRUST[0]],
            )
            .map_err(dberr)?;
        // This device becomes the account's current device.
        tx.execute("UPDATE users SET last_device = ?1 WHERE id = ?2 AND EXISTS (SELECT 1 FROM sessions WHERE id = ?3)", params![dev.nonce, user.id, session_id]).map_err(dberr)?;
        if let Some(up) = &upgrade {
            tx.execute(
                "UPDATE users SET pw_hash = ?1, pw_params = ?2 WHERE id = ?3 AND pw_hash = ?4 AND EXISTS (SELECT 1 FROM sessions WHERE id = ?5)",
                params![up.phc, params_json(&up.params), user.id, pw_hash, session_id],
            )
            .map_err(dberr)?;
        }
        if let Some(old) = old_cookie {
            // A browser that still holds an older session gets that session ended, not orphaned.
            tx.execute(
                "UPDATE sessions SET revoked_at = ?1 WHERE revoked_at IS NULL AND family_id != ?3
                   AND family_id = (SELECT family_id FROM sessions WHERE refresh_hash = ?2)
                   AND EXISTS (SELECT 1 FROM sessions WHERE id = ?4)",
                params![c.now, sha256(&old), family, session_id],
            )
            .map_err(dberr)?;
        }
        tx.commit().map_err(dberr)?;
        ins
    };
    if inserted != 1 {
        return Err(fail());
    }
    release.refund(&c.conn()); // a success never counts against the account
    tracing::info!(user = user.id.as_str(), family = family.as_str(), "auth: login succeeded");
    Ok(token_response(c, client, &user.id, &user.trust, &family, &refresh, expires_at, Some(&dev.token)))
}

// ---- refresh ---------------------------------------------------------------------------------

struct RefreshRow {
    id: String,
    family_id: String,
    user_id: String,
    auth_at: i64,
    expires_at: i64,
    rotated_at: Option<i64>,
    revoked_at: Option<i64>,
    trust: String,
    deleted_at: Option<i64>,
}

pub fn refresh(c: &Ctx, h: &HeaderMap, client: &ClientInfo, body: Option<&Map<String, Value>>) -> Result<Response, HttpError> {
    require_origin_for_cookie(client)?;
    let raw = presented_refresh(c, h, client, body);
    let ip = guard::client_ip(h)?; // fail closed with no client IP
    let Some(raw) = raw else {
        guard::limit(&c.conn(), c.keys, Action::Refresh, &ip, None, c.now)?;
        return Err(refresh_failure(c, client));
    };
    let now = c.now;
    // Looked up first; only an unknown token is charged to the IP, so junk tokens from a shared school address can
    // never make the school's real refreshes fail.
    let row: Option<RefreshRow> = c
        .conn()
        .query_row(
            "SELECT s.id, s.family_id, s.user_id, s.auth_at, s.expires_at, s.rotated_at, s.revoked_at, u.trust, COALESCE(u.deleted_at, u.suspended_at)
             FROM sessions s JOIN users u ON u.id = s.user_id WHERE s.refresh_hash = ?1",
            [sha256(&raw)],
            |r| Ok(RefreshRow { id: r.get(0)?, family_id: r.get(1)?, user_id: r.get(2)?, auth_at: r.get(3)?, expires_at: r.get(4)?, rotated_at: r.get(5)?, revoked_at: r.get(6)?, trust: r.get(7)?, deleted_at: r.get(8)? }),
        )
        .optional()
        .map_err(dberr)?;
    let Some(row) = row else {
        guard::limit(&c.conn(), c.keys, Action::Refresh, &ip, None, now)?;
        return Err(refresh_failure(c, client));
    };
    if row.revoked_at.is_some() || row.deleted_at.is_some() || row.expires_at <= now || row.auth_at + c.cfg.session_max * 1000 <= now {
        return Err(refresh_failure(c, client));
    }
    // The successor is derived, never stored: HMAC(server key, predecessor).
    let successor = hmac(&c.keys.refresh, &raw);
    let successor_hash = sha256(&successor);

    if row.rotated_at.is_none() {
        let expires_at = (now + c.cfg.refresh_ttl * 1000).min(row.auth_at + c.cfg.session_max * 1000);
        let rotated = {
            let mut conn = c.conn();
            let tx = conn.transaction().map_err(dberr)?;
            // The one atomic rotation: only one caller can flip rotated_at from NULL.
            let n = tx.execute("UPDATE sessions SET rotated_at = ?1 WHERE id = ?2 AND rotated_at IS NULL AND revoked_at IS NULL", params![now, row.id]).map_err(dberr)?;
            // Inserted only if this batch did the rotation; OR IGNORE makes racing winners converge.
            tx.execute(
                "INSERT OR IGNORE INTO sessions (id, family_id, user_id, refresh_hash, created_at, auth_at, expires_at, user_agent)
                 SELECT ?1, family_id, user_id, ?2, ?3, auth_at, ?4, ?5 FROM sessions WHERE id = ?6 AND rotated_at = ?3 AND revoked_at IS NULL",
                params![uuid_v4(), successor_hash, now, expires_at, user_agent(h), row.id],
            )
            .map_err(dberr)?;
            tx.commit().map_err(dberr)?;
            n == 1
        };
        if rotated {
            tracing::info!(user = row.user_id.as_str(), family = row.family_id.as_str(), "auth: refresh rotated");
            return Ok(token_response(c, client, &row.user_id, &row.trust, &row.family_id, &successor, expires_at, None));
        }
    }

    // Already rotated (by us earlier, or by a concurrent request a moment ago).
    let cur: Option<(Option<i64>, Option<i64>)> = c.conn().query_row("SELECT rotated_at, revoked_at FROM sessions WHERE id = ?1", [&row.id], |r| Ok((r.get(0)?, r.get(1)?))).optional().map_err(dberr)?;
    let Some((Some(rotated_at), None)) = cur else { return Err(refresh_failure(c, client)) };
    if now - rotated_at <= c.cfg.grace_ms {
        // Inside the grace window: hand back the same successor (two tabs, or a retried request). If that successor has
        // itself been rotated meanwhile, follow the derived chain to the live head, so the late caller is not handed a
        // token that would trip reuse detection later.
        let (mut tok, mut tok_hash) = (successor, successor_hash);
        for _ in 0..8 {
            let s: Option<(i64, Option<i64>, Option<i64>)> = c
                .conn()
                .query_row("SELECT expires_at, revoked_at, rotated_at FROM sessions WHERE refresh_hash = ?1", [&tok_hash], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))
                .optional()
                .map_err(dberr)?;
            let Some((expires_at, revoked_at, next_rotated)) = s else { break };
            if revoked_at.is_some() || expires_at <= now {
                break;
            }
            if next_rotated.is_none() {
                tracing::info!(user = row.user_id.as_str(), family = row.family_id.as_str(), "auth: refresh grace reuse");
                return Ok(token_response(c, client, &row.user_id, &row.trust, &row.family_id, &tok, expires_at, None));
            }
            tok = hmac(&c.keys.refresh, &tok);
            tok_hash = sha256(&tok);
        }
        return Err(refresh_failure(c, client));
    }
    // Outside the window, a rotated token showing up means it was copied: end the whole family.
    c.conn().execute("UPDATE sessions SET revoked_at = ?1 WHERE family_id = ?2 AND revoked_at IS NULL", params![now, row.family_id]).map_err(dberr)?;
    tracing::warn!(user = row.user_id.as_str(), family = row.family_id.as_str(), "auth alert: refresh_reuse_revoked");
    Err(refresh_failure(c, client))
}

// ---- logout ----------------------------------------------------------------------------------

/// `body` is None when the request sent no Content-Type.
pub fn logout(c: &Ctx, h: &HeaderMap, client: &ClientInfo, body: Option<&Map<String, Value>>) -> Result<Response, HttpError> {
    require_origin_for_cookie(client)?;
    let ip = guard::client_ip(h)?; // fail closed with no client IP
    let everywhere = body.and_then(|b| b.get("scope")).and_then(Value::as_str) == Some("global");
    // The device cookie survives a plain logout (the owner stays known on their own computer) unless the user asks to
    // forget this device, e.g. on a shared machine.
    let forget_device = body.and_then(|b| b.get("forget_device")) == Some(&Value::Bool(true));
    let mut found: Option<(String, String)> = None;
    let mut via_refresh = false;
    if let Some(raw) = presented_refresh(c, h, client, body) {
        found = c
            .conn()
            .query_row("SELECT family_id, user_id FROM sessions WHERE refresh_hash = ?1 AND revoked_at IS NULL", [sha256(&raw)], |r| Ok((r.get(0)?, r.get(1)?)))
            .optional()
            .map_err(dberr)?;
        via_refresh = found.is_some();
    }
    if found.is_none() {
        found = authenticate(c, h).map(|p| (p.session_family, p.user_id));
    }
    // Only a request that presented nothing valid is charged to the IP (junk from a shared address must not make the
    // school's real logouts fail).
    if found.is_none() {
        guard::limit(&c.conn(), c.keys, Action::Refresh, &ip, None, c.now)?;
    }
    if let (true, Some((_, user_id))) = (everywhere, &found) {
        // Logging out everywhere also revokes every known device and re-keys the account's failure counters, so it
        // needs the refresh token, not just a (possibly stolen) 15-minute access token.
        if !via_refresh {
            return Err(HttpError::new(401, "refresh_token_required"));
        }
        let mut conn = c.conn();
        let tx = conn.transaction().map_err(dberr)?;
        tx.execute("UPDATE sessions SET revoked_at = ?1 WHERE user_id = ?2 AND revoked_at IS NULL", params![c.now, user_id]).map_err(dberr)?;
        tx.execute("UPDATE users SET device_epoch = device_epoch + 1 WHERE id = ?1", [user_id]).map_err(dberr)?;
        tx.commit().map_err(dberr)?;
    } else if let Some((family, _)) = &found {
        c.conn().execute("UPDATE sessions SET revoked_at = ?1 WHERE family_id = ?2 AND revoked_at IS NULL", params![c.now, family]).map_err(dberr)?;
    }
    if let Some((family, user_id)) = &found {
        tracing::info!(user = user_id.as_str(), family = family.as_str(), "auth: logout");
    }
    // Always the same answer, so logout reveals nothing about the token.
    let mut res = empty(204, &[]);
    if client.mode == ClientMode::Web {
        let mut cookies = vec![clear_cookie(&c.cfg.cookie_name)];
        if everywhere || forget_device {
            cookies.push(clear_cookie(&c.cfg.device_cookie_name));
        }
        for ck in cookies {
            if let Ok(v) = ck.parse() {
                res.headers_mut().append("set-cookie", v);
            }
        }
    }
    Ok(res)
}

/// The AI relay's and billing's `Authenticator`: the bearer access token checked against the live session (any trust
/// level, as in the Worker). It owns its own connection to the database file.
pub struct SessionAuth {
    pub db: Mutex<Connection>,
    pub cfg: std::sync::Arc<AuthConfig>,
    pub keys: Keys,
}

impl SessionAuth {
    pub fn new(db: Connection, cfg: std::sync::Arc<AuthConfig>) -> Self {
        let keys = super::crypto::derive_keys(&cfg.secret);
        Self { db: Mutex::new(db), cfg, keys }
    }
}

impl crate::ai::routes::Authenticator for SessionAuth {
    fn user(&self, headers: &HeaderMap) -> Option<String> {
        let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_millis() as i64).unwrap_or(0);
        authenticate(&Ctx { db: &self.db, cfg: &self.cfg, keys: &self.keys, now }, headers).map(|p| p.user_id)
    }
}

#[cfg(test)]
#[path = "session_tests.rs"]
mod tests;

/// A device that is valid for a user, from the request (used by enrolment and the other device-gated routes).
pub fn valid_device(c: &Ctx, h: &HeaderMap, client: &ClientInfo, user: Option<&DeviceUser>) -> Option<Device> {
    guard::parse_device(c.keys, guard::presented_device(h, client.mode == ClientMode::Native, c.cfg).as_deref(), user, c.now)
}
