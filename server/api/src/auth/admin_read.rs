//! The approval workflow, part 3: the reads — queue, user detail, audit log, queue stats and the review reminder (port
//! of `queue`, `userDetail`, `auditLog`, `stats` and `reviewReminders` in auth/src/admin.ts).
use super::admin::{dberr, guard_admin, lock};
use super::config::DAY;
use super::http::{ClientInfo, HttpError, json};
use super::session::Ctx;
use axum::http::HeaderMap;
use axum::response::Response;
use rusqlite::{Connection, OptionalExtension, Row, params, types::ValueRef};
use serde_json::{Map, Value, json};
use std::collections::HashMap;

/// Server-generated labels for email domains (the pool's staff-domain list), never mixed into applicant text.
pub type DomainLabels<'a> = &'a dyn Fn(&Connection, &[String]) -> HashMap<String, String>;

pub struct QueueStats {
    pub open: i64,
    pub oldest_age_seconds: i64,
}

pub fn stats(conn: &Connection, now: i64) -> Result<QueueStats, HttpError> {
    let (n, oldest): (i64, Option<i64>) = conn
        .query_row(
            "SELECT COUNT(*), MIN(r.requested_at) FROM approval_requests r JOIN users u ON u.id = r.user_id WHERE r.decided_at IS NULL AND u.deleted_at IS NULL",
            [],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .map_err(dberr)?;
    Ok(QueueStats { open: n, oldest_age_seconds: oldest.map_or(0, |o| ((now - o) / 1000).max(0)) })
}

fn domain_of(email: &str) -> Option<String> {
    email.rfind('@').map(|i| email[i + 1..].to_string()).filter(|d| !d.is_empty())
}

fn parse_json(s: Option<String>) -> Value {
    s.and_then(|s| serde_json::from_str(&s).ok()).unwrap_or(Value::Null)
}

fn invalid(field: &str) -> HttpError {
    HttpError::new(400, "invalid_request").with("field", json!(field))
}

/// A whole row as a JSON object, columns by name (the equivalent of `SELECT *` into an object).
fn row_json(r: &Row) -> rusqlite::Result<Map<String, Value>> {
    let mut m = Map::new();
    for (i, name) in r.as_ref().column_names().iter().enumerate() {
        let v = match r.get_ref(i)? {
            ValueRef::Null => Value::Null,
            ValueRef::Integer(n) => json!(n),
            ValueRef::Real(f) => json!(f),
            ValueRef::Text(t) => Value::String(String::from_utf8_lossy(t).into_owned()),
            ValueRef::Blob(_) => Value::Null,
        };
        m.insert((*name).to_string(), v);
    }
    Ok(m)
}

fn age(q: &HashMap<String, String>, key: &str) -> Result<Option<i64>, HttpError> {
    match q.get(key) {
        None => Ok(None),
        Some(v) if !v.is_empty() && v.len() <= 10 && v.bytes().all(|b| b.is_ascii_digit()) => Ok(v.parse::<i64>().ok().map(|n| n * 1000)),
        Some(_) => Err(invalid(key)),
    }
}

/// The review queue, oldest first. Applicant-typed fields (display name, profile) are the SNAPSHOT submitted with that
/// application; `profile_changed` says whether the live profile differs now. Server-generated labels are in
/// `server_labels`, never mixed into applicant text. Render every applicant field as plain text.
pub fn queue(c: &Ctx, h: &HeaderMap, client: &ClientInfo, q: &HashMap<String, String>, labels: DomainLabels) -> Result<Response, HttpError> {
    guard_admin(c, h, client, false)?;
    let status = q.get("status").map_or("open", String::as_str);
    let filter = match status {
        "open" => "r.decided_at IS NULL",
        "approved" => "r.decision = 'approved'",
        "rejected" => "r.decision = 'rejected'",
        "revoked" => "r.decision = 'revoked'",
        "all" => "1 = 1",
        _ => return Err(invalid("status")),
    };
    let limit = q.get("limit").and_then(|v| v.parse::<i64>().ok()).filter(|n| *n != 0).unwrap_or(50).clamp(1, 200);
    let domain = match q.get("domain") {
        Some(d) if !d.is_empty() && d.len() <= 253 && d.bytes().all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'.' || b == b'-') => Some(d.clone()),
        Some(_) => return Err(invalid("domain")),
        None => None,
    };
    let min_age = age(q, "min_age_seconds")?;
    let max_age = age(q, "max_age_seconds")?;
    let (mut after_at, mut after_id) = (-1i64, String::new());
    if let Some(cur) = q.get("cursor").filter(|s| !s.is_empty()) {
        let parsed = cur.split_once('.').filter(|(a, b)| {
            !a.is_empty() && a.len() <= 15 && a.bytes().all(|x| x.is_ascii_digit()) && !b.is_empty() && b.len() <= 36 && b.bytes().all(|x| x.is_ascii_digit() || (b'a'..=b'f').contains(&x) || x == b'-')
        });
        let Some((a, b)) = parsed else { return Err(invalid("cursor")) };
        after_at = a.parse().unwrap_or(-1);
        after_id = b.to_string();
    }
    let conn = lock(c);
    let sql = format!(
        "SELECT r.id, r.user_id, r.requested_at, r.decided_at, r.decision, r.email_domain, r.display_name_snapshot, r.profile_snapshot,
                u.email, u.display_name, u.profile, u.trust, u.suspended_at,
                (SELECT COUNT(*) FROM approval_requests x WHERE x.user_id = r.user_id AND x.id != r.id AND x.decision IN ('rejected', 'revoked'))
         FROM approval_requests r JOIN users u ON u.id = r.user_id
         WHERE u.deleted_at IS NULL AND {filter}
           AND (?4 IS NULL OR r.email_domain = ?4)
           AND (?5 IS NULL OR r.requested_at <= ?6 - ?5)
           AND (?7 IS NULL OR r.requested_at >= ?6 - ?7)
           AND (r.requested_at > ?1 OR (r.requested_at = ?1 AND r.id > ?2))
         ORDER BY r.requested_at, r.id LIMIT ?3"
    );
    struct Q {
        id: String,
        user_id: String,
        requested_at: i64,
        decided_at: Option<i64>,
        decision: Option<String>,
        email_domain: Option<String>,
        name_snap: Option<String>,
        profile_snap: Option<String>,
        email: String,
        name: Option<String>,
        profile: Option<String>,
        trust: String,
        suspended_at: Option<i64>,
        prior: i64,
    }
    let mut st = conn.prepare(&sql).map_err(dberr)?;
    let rows: Vec<Q> = st
        .query_map(params![after_at, after_id, limit + 1, domain, min_age, c.now, max_age], |r| {
            Ok(Q { id: r.get(0)?, user_id: r.get(1)?, requested_at: r.get(2)?, decided_at: r.get(3)?, decision: r.get(4)?, email_domain: r.get(5)?, name_snap: r.get(6)?, profile_snap: r.get(7)?, email: r.get(8)?, name: r.get(9)?, profile: r.get(10)?, trust: r.get(11)?, suspended_at: r.get(12)?, prior: r.get(13)? })
        })
        .map_err(dberr)?
        .collect::<Result<_, _>>()
        .map_err(dberr)?;
    let more = rows.len() as i64 > limit;
    let page = &rows[..rows.len().min(limit as usize)];
    let mut domains: Vec<String> = page.iter().filter_map(|x| x.email_domain.clone().or_else(|| domain_of(&x.email))).collect();
    domains.sort();
    domains.dedup();
    let mut history: HashMap<String, (i64, i64)> = HashMap::new();
    for d in &domains {
        if let Some(row) = conn.query_row("SELECT approved, rejected FROM domain_decisions WHERE domain = ?1", [d], |r| Ok((r.get(0)?, r.get(1)?))).optional().map_err(dberr)? {
            history.insert(d.clone(), row);
        }
    }
    let label_map = if domains.is_empty() { HashMap::new() } else { labels(&conn, &domains) };
    let s = stats(&conn, c.now)?;
    let items: Vec<Value> = page
        .iter()
        .map(|x| {
            let dom = x.email_domain.clone().or_else(|| domain_of(&x.email));
            let hist = dom.as_ref().map(|d| history.get(d).copied().unwrap_or((0, 0)));
            json!({
                "request_id": x.id, "user_id": x.user_id,
                "status": if x.decided_at.is_none() { json!("open") } else { json!(x.decision) },
                "activated_at": x.requested_at, "age_seconds": ((c.now - x.requested_at) / 1000).max(0), "decided_at": x.decided_at,
                "email": x.email, "email_domain": dom,
                // Server-generated; never mixed with applicant text.
                "server_labels": {
                    "domain_hint": dom.as_ref().and_then(|d| label_map.get(d)),
                    "domain_history": hist.map(|(a, r)| json!({ "approved": a, "rejected": r })),
                    "prior_rejections": x.prior,
                    "profile_changed": x.name != x.name_snap || x.profile != x.profile_snap,
                    "suspended": x.suspended_at.is_some(),
                },
                // Applicant-typed, as submitted with this application. Render as plain text.
                "applicant": { "display_name": x.name_snap, "profile": parse_json(x.profile_snap.clone()) },
                "trust": x.trust,
            })
        })
        .collect();
    let next = match (more, page.last()) {
        (true, Some(l)) => json!(format!("{}.{}", l.requested_at, l.id)),
        _ => Value::Null,
    };
    Ok(json(200, &json!({ "open": s.open, "oldestAgeSeconds": s.oldest_age_seconds, "items": items, "next_cursor": next }), &[]))
}

pub fn user_detail(c: &Ctx, h: &HeaderMap, client: &ClientInfo, id: &str) -> Result<Response, HttpError> {
    guard_admin(c, h, client, false)?;
    if !(id.len() == 36 && id.bytes().all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b) || b == b'-')) {
        return Err(HttpError::new(404, "not_found"));
    }
    let conn = lock(c);
    let mut user = conn
        .query_row(
            "SELECT id, email, display_name, profile, trust, created_at, email_verified_at, suspended_at, suspension_reason, deleted_at FROM users WHERE id = ?1",
            [id],
            row_json,
        )
        .optional()
        .map_err(dberr)?
        .ok_or_else(|| HttpError::new(404, "not_found"))?;
    user["profile"] = parse_json(user["profile"].as_str().map(str::to_string));
    let mut rq = conn
        .prepare(
            "SELECT id, requested_at, decided_at, decided_by, decision, evidence, evidence_note, note, reason, email_domain, display_name_snapshot, profile_snapshot
             FROM approval_requests WHERE user_id = ?1 ORDER BY requested_at, rowid",
        )
        .map_err(dberr)?;
    let requests: Vec<Value> = rq
        .query_map([id], row_json)
        .map_err(dberr)?
        .map(|r| {
            r.map(|mut m| {
                let p = parse_json(m["profile_snapshot"].as_str().map(str::to_string));
                m.insert("profile_snapshot".into(), p);
                Value::Object(m)
            })
        })
        .collect::<Result<_, _>>()
        .map_err(dberr)?;
    let mut au = conn.prepare("SELECT * FROM auth_audit WHERE target_id = ?1 ORDER BY at DESC, rowid DESC LIMIT 200").map_err(dberr)?;
    let audit: Vec<Value> = au
        .query_map([id], row_json)
        .map_err(dberr)?
        .map(|r| {
            r.map(|mut m| {
                let d = parse_json(m["detail"].as_str().map(str::to_string));
                m.insert("detail".into(), d);
                Value::Object(m)
            })
        })
        .collect::<Result<_, _>>()
        .map_err(dberr)?;
    Ok(json(200, &json!({ "user": user, "requests": requests, "audit": audit }), &[]))
}

/// The audit log, newest first; ties broken by insertion order (rowid), so paging is stable.
pub fn audit_log(c: &Ctx, h: &HeaderMap, client: &ClientInfo, q: &HashMap<String, String>) -> Result<Response, HttpError> {
    guard_admin(c, h, client, false)?;
    let limit = q.get("limit").and_then(|v| v.parse::<i64>().ok()).filter(|n| *n != 0).unwrap_or(100).clamp(1, 500);
    let (mut at, mut seq) = (i64::MAX, i64::MAX);
    if let Some(b) = q.get("before").filter(|s| !s.is_empty()) {
        let parsed = b.split_once('.').filter(|(x, y)| [x, y].iter().all(|p| !p.is_empty() && p.len() <= 15 && p.bytes().all(|c| c.is_ascii_digit())));
        let Some((x, y)) = parsed else { return Err(invalid("before")) };
        at = x.parse().unwrap_or(i64::MAX);
        seq = y.parse().unwrap_or(i64::MAX);
    }
    let conn = lock(c);
    let mut st = conn.prepare("SELECT rowid AS seq, * FROM auth_audit WHERE (at < ?1 OR (at = ?1 AND rowid < ?2)) ORDER BY at DESC, rowid DESC LIMIT ?3").map_err(dberr)?;
    let rows: Vec<Map<String, Value>> = st.query_map(params![at, seq, limit + 1], row_json).map_err(dberr)?.collect::<Result<_, _>>().map_err(dberr)?;
    let more = rows.len() as i64 > limit;
    let page = &rows[..rows.len().min(limit as usize)];
    let next = match (more, page.last()) {
        (true, Some(l)) => json!(format!("{}.{}", l["at"], l["seq"])),
        _ => Value::Null,
    };
    let entries: Vec<Value> = page
        .iter()
        .map(|m| {
            let mut m = m.clone();
            let d = parse_json(m["detail"].as_str().map(str::to_string));
            m.insert("detail".into(), d);
            Value::Object(m)
        })
        .collect();
    Ok(json(200, &json!({ "entries": entries, "next_before": next }), &[]))
}

#[derive(Debug, PartialEq)]
pub enum ReminderKind {
    Digest,
    Escalation,
}

pub struct Reminder {
    pub open: i64,
    pub oldest_age_seconds: i64,
    pub kind: ReminderKind,
    pub approved_last_day: i64,
    pub rejected_last_day: i64,
    /// (admin id, approved, rejected)
    pub by_admin: Vec<(String, i64, i64)>,
}

/// For the cron: queue state; an escalation when overdue, or a digest (with the last day's decisions per admin) if
/// enabled. None when there is nothing to send.
pub fn review_reminder(conn: &Connection, now: i64, escalate_after_s: i64, digest: bool) -> Result<Option<Reminder>, HttpError> {
    let s = stats(conn, now)?;
    let kind = if s.open > 0 && s.oldest_age_seconds * 1000 >= escalate_after_s * 1000 {
        ReminderKind::Escalation
    } else if digest {
        ReminderKind::Digest
    } else {
        return Ok(None);
    };
    let mut st = conn
        .prepare(
            "SELECT decided_by, SUM(decision = 'approved'), SUM(decision IN ('rejected', 'revoked')) FROM approval_requests
             WHERE decided_at > ?1 AND decided_by IS NOT NULL GROUP BY decided_by",
        )
        .map_err(dberr)?;
    let by_admin: Vec<(String, i64, i64)> = st.query_map([now - DAY * 1000], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?))).map_err(dberr)?.collect::<Result<_, _>>().map_err(dberr)?;
    Ok(Some(Reminder {
        open: s.open,
        oldest_age_seconds: s.oldest_age_seconds,
        kind,
        approved_last_day: by_admin.iter().map(|x| x.1).sum(),
        rejected_last_day: by_admin.iter().map(|x| x.2).sum(),
        by_admin,
    }))
}

#[cfg(test)]
#[path = "admin_read_tests.rs"]
mod tests;
