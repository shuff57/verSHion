//! The approval workflow, part 1: the admin guard and the decisions on applications — approve, reject, bulk reject and
//! revoke (port of `guard`, `decideOne`, `approve`, `reject`, `rejectBulk` and `revoke` in auth/src/admin.ts).
//! Every decision is one transaction gated by a single UPDATE, so concurrent decisions cannot both win and a stale
//! queue page changes nothing. Suspension, proposals, the queue and the audit log follow in `admin_suspend.rs`.
use super::audit::{AuditFields, audit};
use super::config::{Action, APPROVED_LEVEL, AFTER_VERIFY};
use super::crypto::uuid_v4;
use super::device::sudo_valid;
use super::guard::{self, DeviceUser};
use super::http::{ClientInfo, HttpError, json};
use super::mail::{MailKind, MailMessage, Outbox};
use super::session::{Ctx, Principal, require_principal, valid_device};
use super::account::is_admin_level;
use axum::http::HeaderMap;
use axum::response::Response;
use rusqlite::{Connection, OptionalExtension, params};
use serde_json::{Map, Value, json};

pub const EVIDENCE_KINDS: [&str; 2] = ["directory", "school_confirmed"];
const MAX_TEXT: usize = 2000;

pub struct AdminPrincipal {
    pub user_id: String,
    pub sudo: bool,
}

pub fn dberr(e: rusqlite::Error) -> HttpError {
    tracing::error!("auth: database error: {e}");
    HttpError::new(500, "server_error")
}

pub fn lock<'a>(c: &'a Ctx) -> std::sync::MutexGuard<'a, Connection> {
    c.db.lock().unwrap_or_else(|p| p.into_inner())
}

/// A required (or optional) trimmed text field of at most 2000 characters.
pub fn text(body: &Map<String, Value>, key: &str, required: bool) -> Result<Option<String>, HttpError> {
    let bad = || HttpError::new(400, "invalid_request").with("field", json!(key));
    match body.get(key) {
        None | Some(Value::Null) => {
            if required { Err(bad()) } else { Ok(None) }
        }
        Some(Value::String(s)) if s.trim().is_empty() => {
            if required { Err(bad()) } else { Ok(None) }
        }
        Some(Value::String(s)) if s.encode_utf16().count() <= MAX_TEXT => Ok(Some(s.trim().to_string())),
        _ => Err(bad()),
    }
}

pub fn id_field(body: &Map<String, Value>, key: &str) -> Result<String, HttpError> {
    match body.get(key).and_then(Value::as_str) {
        Some(s) if s.len() == 36 && s.bytes().all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b) || b == b'-') => Ok(s.to_string()),
        _ => Err(HttpError::new(400, "invalid_request").with("field", json!(key))),
    }
}

/// Authenticates an admin: admin level, a known device, and (for writes) a sudo token from a recent password re-entry.
pub fn guard_admin(c: &Ctx, h: &HeaderMap, client: &ClientInfo, write: bool) -> Result<AdminPrincipal, HttpError> {
    let p: Principal = require_principal(c, h)?;
    let ip = guard::client_ip(h)?;
    guard::limit(&lock(c), c.keys, Action::Admin, &ip, Some(&format!("user:{}", p.user_id)), c.now)?;
    if !is_admin_level(&p.trust) {
        return Err(HttpError::new(403, "forbidden"));
    }
    let epoch: Option<i64> = lock(c)
        .query_row("SELECT device_epoch FROM users WHERE id = ?1 AND deleted_at IS NULL AND suspended_at IS NULL", [&p.user_id], |r| r.get(0))
        .optional()
        .map_err(dberr)?;
    let Some(epoch) = epoch else { return Err(HttpError::new(401, "unauthorized")) };
    if valid_device(c, h, client, Some(&DeviceUser { id: &p.user_id, device_epoch: epoch })).is_none() {
        return Err(HttpError::new(403, "known_device_required"));
    }
    let mut sudo = false;
    if write {
        sudo = sudo_valid(c.keys, h.get("x-sudo-token").and_then(|v| v.to_str().ok()), &p, epoch, c.now);
        if !sudo {
            return Err(HttpError::new(403, "sudo_required"));
        }
    }
    Ok(AdminPrincipal { user_id: p.user_id, sudo })
}

/// Queues a mail to the account's current address, if it still has one.
pub fn notify(conn: &Connection, id: &str, kind: MailKind, reason: Option<&str>, out: &mut Outbox) {
    let to: Option<String> = conn.query_row("SELECT email FROM users WHERE id = ?1 AND deleted_at IS NULL", [id], |r| r.get(0)).optional().ok().flatten();
    if let Some(to) = to {
        let mut m = MailMessage::new(kind, &to);
        m.reason = reason.map(str::to_string);
        out.push(m);
    }
}

struct Fields<'a> {
    note: &'a str,
    evidence: Option<&'a str>,
    evidence_note: Option<&'a str>,
    reason: Option<&'a str>,
}

/// One decision on one NAMED application. The gate is a single UPDATE on (request id, user id, still open, account
/// still pending, active, unsuspended, not deleted), so concurrent decisions cannot both win and a stale queue page
/// (after a re-application) changes nothing.
fn decide_one(c: &Ctx, admin: &AdminPrincipal, target: &str, request_id: &str, approved: bool, f: &Fields, out: &mut Outbox) -> Result<bool, HttpError> {
    if target == admin.user_id {
        return Err(HttpError::new(403, "self_action"));
    }
    let nonce = uuid_v4();
    let decision = if approved { "approved" } else { "rejected" };
    let mut conn = lock(c);
    let tx = conn.transaction().map_err(dberr)?;
    let won: Option<String> = tx
        .query_row(
            "UPDATE approval_requests SET decided_at = ?1, decided_by = ?2, decision = ?3, evidence = ?4, evidence_note = ?5,
               note = ?6, reason = ?7, decision_nonce = ?8
             WHERE id = ?11 AND user_id = ?9 AND decided_at IS NULL
               AND EXISTS (SELECT 1 FROM users WHERE id = ?9 AND trust = ?10 AND deleted_at IS NULL AND suspended_at IS NULL
                             AND email_verified_at IS NOT NULL AND pw_hash IS NOT NULL)
             RETURNING id",
            params![c.now, admin.user_id, decision, f.evidence, f.evidence_note, f.note, f.reason, nonce, target, AFTER_VERIFY, request_id],
            |r| r.get(0),
        )
        .optional()
        .map_err(dberr)?;
    if won.is_none() {
        return Ok(false);
    }
    if approved {
        tx.execute("UPDATE users SET trust = ?1 WHERE id = ?2 AND trust = ?3", params![APPROVED_LEVEL, target, AFTER_VERIFY]).map_err(dberr)?;
    }
    // The per-domain counters move with the decision, in the same transaction.
    tx.execute(
        "INSERT INTO domain_decisions (domain, approved, rejected)
         SELECT email_domain, ?1, ?2 FROM approval_requests WHERE decision_nonce = ?3 AND email_domain IS NOT NULL
         ON CONFLICT (domain) DO UPDATE SET approved = approved + ?1, rejected = rejected + ?2",
        params![i64::from(approved), i64::from(!approved), nonce],
    )
    .map_err(dberr)?;
    let e = AuditFields {
        action: if approved { "approve" } else { "reject" },
        target_id: Some(target),
        prev_trust: Some(AFTER_VERIFY),
        new_trust: Some(if approved { APPROVED_LEVEL } else { AFTER_VERIFY }),
        evidence: f.evidence,
        evidence_note: f.evidence_note,
        note: Some(f.note),
        reason: f.reason,
        detail: Some(json!({ "request_id": request_id })),
    };
    audit(&tx, Some(&admin.user_id), "admin", &e, c.now, None, false, &uuid_v4()).map_err(dberr)?;
    tx.commit().map_err(dberr)?;
    tracing::info!(admin = admin.user_id.as_str(), user = target, decision, "auth: admin action");
    if approved {
        notify(&conn, target, MailKind::ApplicationApproved, None, out);
    } else {
        notify(&conn, target, MailKind::ApplicationRejected, f.reason, out);
    }
    Ok(true)
}

pub fn approve(c: &Ctx, h: &HeaderMap, client: &ClientInfo, body: &Map<String, Value>, out: &mut Outbox) -> Result<Response, HttpError> {
    let admin = guard_admin(c, h, client, true)?;
    let target = id_field(body, "user_id")?;
    let request_id = id_field(body, "request_id")?;
    let note = text(body, "note", true)?.unwrap_or_default();
    let ev = match body.get("evidence").and_then(Value::as_str) {
        Some(e) if EVIDENCE_KINDS.contains(&e) => e.to_string(),
        _ => return Err(HttpError::new(400, "invalid_request").with("field", json!("evidence"))),
    };
    let evidence_note = text(body, "evidence_note", true)?.unwrap_or_default();
    let f = Fields { note: &note, evidence: Some(&ev), evidence_note: Some(&evidence_note), reason: None };
    if !decide_one(c, &admin, &target, &request_id, true, &f, out)? {
        return Err(HttpError::new(409, "not_pending"));
    }
    Ok(json(200, &json!({ "ok": true, "user_id": target, "request_id": request_id, "decision": "approved" }), &[]))
}

pub fn reject(c: &Ctx, h: &HeaderMap, client: &ClientInfo, body: &Map<String, Value>, out: &mut Outbox) -> Result<Response, HttpError> {
    let admin = guard_admin(c, h, client, true)?;
    let target = id_field(body, "user_id")?;
    let request_id = id_field(body, "request_id")?;
    let note = text(body, "note", true)?.unwrap_or_default();
    let reason = text(body, "reason", true)?.unwrap_or_default();
    let f = Fields { note: &note, evidence: None, evidence_note: None, reason: Some(&reason) };
    if !decide_one(c, &admin, &target, &request_id, false, &f, out)? {
        return Err(HttpError::new(409, "not_pending"));
    }
    Ok(json(200, &json!({ "ok": true, "user_id": target, "request_id": request_id, "decision": "rejected" }), &[]))
}

/// Rejects up to `bulk_reject_max` named applications with one note and reason; each is gated and audited on its own.
pub fn reject_bulk(c: &Ctx, h: &HeaderMap, client: &ClientInfo, body: &Map<String, Value>, out: &mut Outbox) -> Result<Response, HttpError> {
    let admin = guard_admin(c, h, client, true)?;
    let max = c.cfg.bulk_reject_max;
    let items = match body.get("items").and_then(Value::as_array) {
        Some(a) if !a.is_empty() && a.len() as i64 <= max => a,
        _ => return Err(HttpError::new(400, "invalid_request").with("field", json!("items")).with("max", json!(max))),
    };
    let note = text(body, "note", true)?.unwrap_or_default();
    let reason = text(body, "reason", true)?.unwrap_or_default();
    let mut parsed = vec![];
    for x in items {
        let Some(o) = x.as_object() else { return Err(HttpError::new(400, "invalid_request").with("field", json!("items"))) };
        parsed.push((id_field(o, "user_id")?, id_field(o, "request_id")?));
    }
    let f = Fields { note: &note, evidence: None, evidence_note: None, reason: Some(&reason) };
    let mut results = vec![];
    for (user_id, request_id) in parsed {
        let ok = user_id != admin.user_id && decide_one(c, &admin, &user_id, &request_id, false, &f, out)?;
        results.push(json!({ "user_id": user_id, "request_id": request_id, "status": if ok { "rejected" } else { "not_pending" } }));
    }
    Ok(json(200, &json!({ "results": results }), &[]))
}

/// Revokes the approved level: back to pending, sessions and device tokens revoked, applicant may re-apply after the
/// cooldown.
pub fn revoke(c: &Ctx, h: &HeaderMap, client: &ClientInfo, body: &Map<String, Value>, out: &mut Outbox) -> Result<Response, HttpError> {
    let admin = guard_admin(c, h, client, true)?;
    let target = id_field(body, "user_id")?;
    if target == admin.user_id {
        return Err(HttpError::new(403, "self_action"));
    }
    let note = text(body, "note", true)?.unwrap_or_default();
    let reason = text(body, "reason", true)?.unwrap_or_default();
    let nonce = uuid_v4();
    let mut conn = lock(c);
    let tx = conn.transaction().map_err(dberr)?;
    // The gate: only an account at the approved level, flipped once.
    let flipped: Option<String> = tx
        .query_row(
            "UPDATE users SET trust = ?1, device_epoch = device_epoch + 1 WHERE id = ?2 AND trust = ?3 AND deleted_at IS NULL RETURNING id",
            params![AFTER_VERIFY, target, APPROVED_LEVEL],
            |r| r.get(0),
        )
        .optional()
        .map_err(dberr)?;
    if flipped.is_none() {
        return Err(HttpError::new(409, "not_approved"));
    }
    tx.execute(
        "INSERT INTO approval_requests (id, user_id, requested_at, decided_at, decided_by, decision, note, reason, decision_nonce, email_domain)
         SELECT ?1, ?2, ?3, ?3, ?4, 'revoked', ?5, ?6, ?7, (SELECT substr(email, instr(email, '@') + 1) FROM users WHERE id = ?2)",
        params![uuid_v4(), target, c.now, admin.user_id, note, reason, nonce],
    )
    .map_err(dberr)?;
    tx.execute("UPDATE sessions SET revoked_at = ?1 WHERE user_id = ?2 AND revoked_at IS NULL", params![c.now, target]).map_err(dberr)?;
    tx.execute(
        "INSERT INTO domain_decisions (domain, approved, rejected)
         SELECT email_domain, 0, 1 FROM approval_requests WHERE decision_nonce = ?1 AND email_domain IS NOT NULL
         ON CONFLICT (domain) DO UPDATE SET rejected = rejected + 1",
        [&nonce],
    )
    .map_err(dberr)?;
    let e = AuditFields { action: "revoke", target_id: Some(&target), prev_trust: Some(APPROVED_LEVEL), new_trust: Some(AFTER_VERIFY), note: Some(&note), reason: Some(&reason), ..Default::default() };
    audit(&tx, Some(&admin.user_id), "admin", &e, c.now, None, false, &uuid_v4()).map_err(dberr)?;
    tx.commit().map_err(dberr)?;
    tracing::info!(admin = admin.user_id.as_str(), user = target.as_str(), "auth: approval revoked");
    notify(&conn, &target, MailKind::ApprovalRevoked, Some(&reason), out);
    Ok(json(200, &json!({ "ok": true, "user_id": target }), &[]))
}

#[cfg(test)]
#[path = "admin_tests.rs"]
mod tests;
