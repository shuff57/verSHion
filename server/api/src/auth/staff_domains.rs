//! verSHion-specific admin parts: staff-domain hints (labels for the reviewer, never a grant of trust) and their admin
//! routes, behind the admin guard (port of worker/src/admin.ts).
use super::admin::{dberr, guard_admin, lock};
use super::audit::{AuditFields, audit};
use super::crypto::uuid_v4;
use super::http::{ClientInfo, HttpError, json};
use super::session::Ctx;
use super::validate::clean_text;
use axum::http::HeaderMap;
use axum::response::Response;
use rusqlite::{Connection, params};
use serde_json::{Map, Value, json};
use std::collections::HashMap;

/// Suffixes under which anyone can register, so a hint on them would label everyone. Not the full Public Suffix List: a
/// hint is only a label, so the cost of a miss is a misleading label, which the reviewer still has to check against the
/// staff directory.
const PUBLIC_SUFFIXES: [&str; 33] = [
    "com", "net", "org", "edu", "gov", "mil", "int", "info", "biz", "io", "co", "us", "uk", "ca", "au", "k12.ca.us", "ca.us", "co.uk", "ac.uk", "org.uk", "gov.uk", "sch.uk", "com.au", "edu.au", "gov.au",
    "gmail.com", "googlemail.com", "outlook.com", "hotmail.com", "live.com", "yahoo.com", "icloud.com", "me.com",
];
const MORE_SUFFIXES: [&str; 4] = ["aol.com", "proton.me", "protonmail.com", ""];

/// Normalises and validates a registrable domain (lower-case, labels, not a bare public suffix or a free-mail domain).
pub fn registrable_domain(raw: &Value) -> Option<String> {
    let d = raw.as_str()?.trim().to_lowercase();
    let d = d.strip_suffix('.').unwrap_or(&d).to_string();
    if d.len() < 4 || d.len() > 253 {
        return None;
    }
    let labels: Vec<&str> = d.split('.').collect();
    if labels.len() < 2 {
        return None;
    }
    let label_ok = |l: &str| {
        let b = l.as_bytes();
        !b.is_empty() && b.len() <= 63 && b.iter().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || *c == b'-') && b[0] != b'-' && b[b.len() - 1] != b'-'
    };
    if !labels.iter().all(|l| label_ok(l)) {
        return None;
    }
    let tld = labels[labels.len() - 1];
    let alpha = tld.len() >= 2 && tld.len() <= 63 && tld.bytes().all(|c| c.is_ascii_lowercase());
    let puny = tld.starts_with("xn--") && tld.len() >= 6 && tld.len() <= 63;
    if !(alpha || puny) {
        return None;
    }
    if PUBLIC_SUFFIXES.contains(&d.as_str()) || MORE_SUFFIXES.contains(&d.as_str()) {
        return None;
    }
    Some(d)
}

/// Labels for the review queue. The wording keeps it a hint: the reviewer still checks the directory or calls the office.
pub fn labels(conn: &Connection, domains: &[String]) -> HashMap<String, String> {
    let mut out = HashMap::new();
    for d in domains {
        if let Ok(note) = conn.query_row("SELECT note FROM staff_domain_hint WHERE domain = ?1", [d], |r| r.get::<_, String>(0)) {
            out.insert(d.clone(), format!("Staff-domain hint (not proof): {note}"));
        }
    }
    out
}

fn bad(field: &str) -> HttpError {
    HttpError::new(400, "invalid_request").with("field", json!(field))
}

pub fn list(c: &Ctx, h: &HeaderMap, client: &ClientInfo) -> Result<Response, HttpError> {
    guard_admin(c, h, client, false)?;
    let conn = lock(c);
    let mut st = conn.prepare("SELECT domain, note, updated_at FROM staff_domain_hint ORDER BY domain").map_err(dberr)?;
    let rows: Vec<Value> = st
        .query_map([], |r| Ok(json!({ "domain": r.get::<_, String>(0)?, "note": r.get::<_, String>(1)?, "updated_at": r.get::<_, i64>(2)? })))
        .map_err(dberr)?
        .collect::<Result<_, _>>()
        .map_err(dberr)?;
    Ok(json(200, &json!({ "domains": rows }), &[("Cache-Control", "no-store")]))
}

pub fn put(c: &Ctx, h: &HeaderMap, client: &ClientInfo, body: &Map<String, Value>) -> Result<Response, HttpError> {
    let admin = guard_admin(c, h, client, true)?;
    let domain = body.get("domain").and_then(registrable_domain).ok_or_else(|| bad("domain"))?;
    let note = body.get("note").and_then(|n| clean_text(n, 200)).ok_or_else(|| bad("note"))?;
    let mut conn = lock(c);
    let tx = conn.transaction().map_err(dberr)?;
    tx.execute(
        "INSERT INTO staff_domain_hint (domain, note, updated_at, updated_by) VALUES (?1, ?2, ?3, ?4)
         ON CONFLICT (domain) DO UPDATE SET note = ?2, updated_at = ?3, updated_by = ?4",
        params![domain, note, c.now, admin.user_id],
    )
    .map_err(dberr)?;
    let e = AuditFields { action: "staff_domain.put", detail: Some(json!({ "domain": domain, "note": note })), ..Default::default() };
    audit(&tx, Some(&admin.user_id), "admin", &e, c.now, None, false, &uuid_v4()).map_err(dberr)?;
    tx.commit().map_err(dberr)?;
    Ok(json(200, &json!({ "ok": true, "domain": domain }), &[("Cache-Control", "no-store")]))
}

pub fn delete(c: &Ctx, h: &HeaderMap, client: &ClientInfo, body: &Map<String, Value>) -> Result<Response, HttpError> {
    let admin = guard_admin(c, h, client, true)?;
    let domain = body.get("domain").and_then(registrable_domain).ok_or_else(|| bad("domain"))?;
    let mut conn = lock(c);
    let tx = conn.transaction().map_err(dberr)?;
    let n = tx.execute("DELETE FROM staff_domain_hint WHERE domain = ?1", [&domain]).map_err(dberr)?;
    let e = AuditFields { action: "staff_domain.delete", detail: Some(json!({ "domain": domain })), ..Default::default() };
    audit(&tx, Some(&admin.user_id), "admin", &e, c.now, None, false, &uuid_v4()).map_err(dberr)?;
    tx.commit().map_err(dberr)?;
    if n == 1 {
        Ok(json(200, &json!({ "ok": true }), &[("Cache-Control", "no-store")]))
    } else {
        Ok(json(404, &json!({ "error": "not_found" }), &[("Cache-Control", "no-store")]))
    }
}

#[cfg(test)]
#[path = "staff_domains_tests.rs"]
mod tests;
