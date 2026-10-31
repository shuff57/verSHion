//! Housekeeping and operator tools: the cron purge, the tombstone lookup, and the bootstrap that creates and demotes
//! admins (port of `purgeExpired`, `findTombstoneByEmail` in auth/src/auth.ts and auth/src/bootstrap.ts). There is no
//! API route for bootstrap: an operator runs it from the command line, and the rules are tested where they are used.
use super::config::{ADMIN_LEVELS, AuthConfig, DAY, TRUST};
use super::crypto::{Keys, hmac, uuid_v4};
use super::http::normalize_email;
use rusqlite::{Connection, OptionalExtension, params};

#[derive(Debug, Default, PartialEq, Eq)]
pub struct PurgeReport {
    pub rate_limits: usize,
    pub sessions: usize,
    pub device_enrolments: usize,
    pub email_tokens: usize,
    pub tombstone_hashes: usize,
    pub unverified_accounts: usize,
}

/// Deletes expired counters, sessions and tokens, tombstone email hashes past their time, and accounts that never
/// confirmed. Run from a timer.
pub fn purge_expired(conn: &mut Connection, cfg: &AuthConfig, now: i64) -> rusqlite::Result<PurgeReport> {
    let day_ms = DAY * 1000;
    let tx = conn.transaction()?;
    let mut r = PurgeReport {
        rate_limits: tx.execute("DELETE FROM rate_limits WHERE expires_at <= ?1", [now])?,
        // Rotated rows are kept until they expire: they are what detects a replayed token.
        sessions: tx.execute("DELETE FROM sessions WHERE expires_at <= ?1 OR (revoked_at IS NOT NULL AND revoked_at <= ?2)", params![now, now - day_ms])?,
        // Kept a day so the daily enrolment-code cap and the daily mail ceiling can still count them.
        device_enrolments: tx.execute("DELETE FROM device_enrolments WHERE created_at <= ?2 AND (expires_at <= ?1 OR used_at IS NOT NULL)", params![now, now - day_ms])?,
        email_tokens: tx.execute("DELETE FROM email_tokens WHERE created_at <= ?2 AND (expires_at <= ?1 OR used_at IS NOT NULL)", params![now, now - day_ms])?,
        tombstone_hashes: tx.execute(
            "UPDATE users SET deleted_email_hmac = NULL, deleted_email_hmac_expires_at = NULL WHERE deleted_email_hmac IS NOT NULL AND deleted_email_hmac_expires_at <= ?1",
            [now],
        )?,
        unverified_accounts: 0,
    };
    if cfg.purge_unverified_after > 0 {
        let cutoff = now - cfg.purge_unverified_after * 1000;
        let stale = "SELECT id FROM users WHERE trust = ?2 AND deleted_at IS NULL AND created_at <= ?1";
        for table in ["sessions", "email_tokens", "device_enrolments", "approval_requests"] {
            tx.execute(&format!("DELETE FROM {table} WHERE user_id IN ({stale})"), params![cutoff, TRUST[0]])?;
        }
        r.unverified_accounts = tx.execute(&format!("DELETE FROM users WHERE id IN ({stale})"), params![cutoff, TRUST[0]])?;
    }
    tx.commit()?;
    Ok(r)
}

/// Finds a deleted account whose email hash is still held (for re-linking content).
pub fn find_tombstone_by_email(conn: &Connection, keys: &Keys, raw: &str, now: i64) -> Option<String> {
    let email = normalize_email(raw).ok()?;
    conn.query_row("SELECT id FROM users WHERE deleted_email_hmac = ?1 AND deleted_email_hmac_expires_at > ?2", params![hmac(&keys.email, &email), now], |r| r.get(0)).optional().ok().flatten()
}

fn level_ok(l: &str) -> bool {
    !l.is_empty() && l.len() <= 32 && l.bytes().all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'_' || b == b'-')
}

#[derive(Debug, PartialEq, Eq)]
pub struct Outcome {
    pub email: String,
    pub trust: String,
    pub suspended: bool,
}

fn outcome(conn: &Connection, email: &str) -> rusqlite::Result<Option<Outcome>> {
    conn.query_row("SELECT email, trust, suspended_at FROM users WHERE email = ?1", [email], |r| Ok(Outcome { email: r.get(0)?, trust: r.get(1)?, suspended: r.get::<_, Option<i64>>(2)?.is_some() })).optional()
}

/// Promotes one account to `admin_level`, or recovers a SUSPENDED admin (when every admin is suspended, this is the way
/// back in). Eligible: confirmed address, a password set by the mailbox owner, not deleted, and either at one of
/// `from_levels` and not suspended, or an admin who is suspended. Audited with actor 'bootstrap'. Running it twice
/// changes nothing.
pub fn make_admin(conn: &mut Connection, email: &str, admin_level: &str, from_levels: &[&str], now: i64) -> Result<Option<Outcome>, String> {
    let email = email.to_lowercase();
    if !level_ok(admin_level) || from_levels.iter().any(|l| !level_ok(l)) {
        return Err("refusing level".into());
    }
    normalize_email(&email).map_err(|_| format!("refusing email: {email:?}"))?;
    let from = serde_json::to_string(from_levels).map_err(|e| e.to_string())?;
    let eligible = "email = ?1 AND email_verified_at IS NOT NULL AND pw_hash IS NOT NULL AND deleted_at IS NULL
        AND ((trust IN (SELECT value FROM json_each(?3)) AND suspended_at IS NULL) OR (trust = ?2 AND suspended_at IS NOT NULL))";
    let tx = conn.transaction().map_err(|e| e.to_string())?;
    // The audit row first, while the previous level is still on the row.
    tx.execute(
        &format!(
            "INSERT INTO auth_audit (id, at, actor_id, actor_kind, action, target_id, prev_trust, new_trust, note)
             SELECT ?4, ?5, 'bootstrap', 'bootstrap', CASE WHEN suspended_at IS NULL THEN 'make_admin' ELSE 'restore_admin' END, id, trust, ?2, 'operator bootstrap script'
             FROM users WHERE {eligible}"
        ),
        params![email, admin_level, from, uuid_v4(), now],
    )
    .map_err(|e| e.to_string())?;
    tx.execute(
        &format!("UPDATE approval_requests SET decided_at = ?4, decided_by = 'bootstrap', decision = 'bootstrap' WHERE decided_at IS NULL AND user_id IN (SELECT id FROM users WHERE {eligible})"),
        params![email, admin_level, from, now],
    )
    .map_err(|e| e.to_string())?;
    tx.execute(&format!("UPDATE users SET trust = ?2, suspended_at = NULL, suspension_reason = NULL WHERE {eligible}"), params![email, admin_level, from]).map_err(|e| e.to_string())?;
    let o = outcome(&tx, &email).map_err(|e| e.to_string())?;
    tx.commit().map_err(|e| e.to_string())?;
    Ok(o)
}

/// Demotes one admin to `to_level`, revoking their sessions and device tokens. Never the last active admin. Audited with
/// actor 'bootstrap'.
pub fn demote_admin(conn: &mut Connection, email: &str, to_level: &str, now: i64) -> Result<Option<Outcome>, String> {
    let email = email.to_lowercase();
    if !level_ok(to_level) {
        return Err("refusing level".into());
    }
    normalize_email(&email).map_err(|_| format!("refusing email: {email:?}"))?;
    let admins = serde_json::to_string(&ADMIN_LEVELS).map_err(|e| e.to_string())?;
    let eligible = "email = ?1 AND trust IN (SELECT value FROM json_each(?2)) AND deleted_at IS NULL
        AND (SELECT COUNT(*) FROM users o WHERE o.trust IN (SELECT value FROM json_each(?2)) AND o.id != users.id AND o.deleted_at IS NULL AND o.suspended_at IS NULL) >= 1";
    let tx = conn.transaction().map_err(|e| e.to_string())?;
    tx.execute(
        &format!(
            "INSERT INTO auth_audit (id, at, actor_id, actor_kind, action, target_id, prev_trust, new_trust, note)
             SELECT ?4, ?5, 'bootstrap', 'bootstrap', 'demote_admin', id, trust, ?3, 'operator bootstrap script' FROM users WHERE {eligible}"
        ),
        params![email, admins, to_level, uuid_v4(), now],
    )
    .map_err(|e| e.to_string())?;
    tx.execute(&format!("UPDATE sessions SET revoked_at = ?3 WHERE revoked_at IS NULL AND user_id IN (SELECT id FROM users WHERE {eligible})"), params![email, admins, now]).map_err(|e| e.to_string())?;
    tx.execute(&format!("UPDATE users SET trust = ?3, device_epoch = device_epoch + 1 WHERE {eligible}"), params![email, admins, to_level]).map_err(|e| e.to_string())?;
    let o = outcome(&tx, &email).map_err(|e| e.to_string())?;
    tx.commit().map_err(|e| e.to_string())?;
    Ok(o)
}

#[cfg(test)]
#[path = "maintenance_tests.rs"]
mod tests;
