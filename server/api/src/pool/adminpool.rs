//! The pool admin routes, first half (worker/src/pool/admin.ts), as HTTP-free cores behind the admin guard (the route layer checks the admin role, the
//! device and the sudo token for writes, and hands the cores the admin's id). Every write is audited twice: `pool_audit` and the auth package's
//! `auth_audit`. Reads that stream a body (GET /admin/pool/item) are the route layer's; `item_head` is its JSON head.
use super::circleadmin::{cursor, parse_json_col, rows};
use super::ctx::{AuditEntry, Fail, PoolCtx, PoolError, Res, audit};
use super::modpurge::Actor;
use super::modprivacy::{Decided, admin_unremove};
use super::moderation::{UNDO_MS, key_leak_qualifies};
use super::sqlfrag::{flag_severity_sql, is_id, new_id};
use crate::auth::audit::{AuditFields, audit as auth_audit};
use crate::auth::validate::clean_text;
use rusqlite::params;
use serde_json::{Map, Value, json};

pub const HOLD_REASONS: [&str; 4] = ["reports", "student_data", "blocklist", "admin"];
/// A takedown's re-upload blocks last this long unless the admin chooses block_days.
pub const DMCA_BLOCK_DEFAULT_DAYS: i64 = 60;

fn bad(field: &str) -> Fail {
    PoolError::new(400, "invalid_request").with("field", json!(field)).into()
}
pub fn id_field(body: &Map<String, Value>, field: &str) -> Res<String> {
    match body.get(field).and_then(Value::as_str) {
        Some(s) if is_id(s) => Ok(s.to_string()),
        _ => Err(bad(field)),
    }
}
pub fn note_field(body: &Map<String, Value>) -> Res<Option<String>> {
    match body.get("note") {
        None | Some(Value::Null) => Ok(None),
        Some(Value::String(s)) if s.is_empty() => Ok(None),
        Some(v) => clean_text(v, 1000).map(Some).ok_or_else(|| bad("note")),
    }
}
fn is_source(s: &str) -> bool {
    matches!(s, "" | "manual" | "privacy" | "privacy_near" | "probe_cap" | "key_leak" | "needs_reupload")
        || ["dmca:", "carried:"].iter().any(|p| s.strip_prefix(p).is_some_and(|rest| rest.len() == 32 && rest.bytes().all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f'))))
}
fn is_human_source(s: &str) -> bool {
    s.is_empty() || s == "manual" || s == "key_leak" || s.starts_with("carried:")
}

/// The pool_audit line and the admin trail (`h.audit` in the Worker).
fn both(x: &PoolCtx, admin: &str, pool_action: &str, item: Option<&str>, auth_action: &str, target: &str, detail: Option<Value>, note: Option<&str>) -> Res<()> {
    audit(x.conn, &AuditEntry { actor_id: Some(admin), actor_kind: "admin", action: pool_action, item_id: item, target_user_id: None, detail: detail.clone(), note }, x.now, None)?;
    auth_audit(x.conn, Some(admin), "admin", &AuditFields { action: auth_action, target_id: Some(target), note, detail, ..Default::default() }, x.now, None, false, &new_id())?;
    Ok(())
}
/// Only the admin trail (the pool_audit line was written elsewhere, as `adminUnremove` does).
fn trail(x: &PoolCtx, admin: &str, auth_action: &str, target: &str, note: Option<&str>) -> Res<()> {
    auth_audit(x.conn, Some(admin), "admin", &AuditFields { action: auth_action, target_id: Some(target), note, ..Default::default() }, x.now, None, false, &new_id())?;
    Ok(())
}

const REPORTS_PAGE: usize = 500;

/// GET /admin/pool/reports: open reports grouped by item, oldest first, with the reporters and their history; paged by a cursor (`after` = the last
/// report's at:id).
pub fn list_reports(x: &PoolCtx, after: Option<&str>) -> Res<Value> {
    let cur = cursor(after);
    let rs = rows(
        x.conn,
        "SELECT r.id, r.item_id, r.reason, r.note, r.at, r.reporter_id, ru.display_name AS reporter_name,
                (SELECT COUNT(*) FROM reports r2 WHERE r2.reporter_id = r.reporter_id) AS reporter_reports,
                (SELECT COUNT(*) FROM reports r3 WHERE r3.reporter_id = r.reporter_id AND r3.action = 'dismissed') AS reporter_dismissed,
                (SELECT COUNT(DISTINCT im.item_id) FROM imports im WHERE im.user_id = r.reporter_id AND im.kind = 'import') AS reporter_imports,
                (SELECT COUNT(*) FROM items it WHERE it.author_id = r.reporter_id) AS reporter_items,
                (SELECT created_at FROM users WHERE id = r.reporter_id) AS reporter_since,
                i.title, i.author_id, (SELECT json_group_array(reason) FROM item_holds h WHERE h.item_id = r.item_id) AS holds
           FROM reports r JOIN items i ON i.id = r.item_id LEFT JOIN users ru ON ru.id = r.reporter_id
          WHERE r.resolved_at IS NULL AND (?1 IS NULL OR r.at > ?1 OR (r.at = ?1 AND r.id > ?2)) ORDER BY r.at, r.id LIMIT 500",
        &[&cur.as_ref().map(|c| c.0), &cur.as_ref().map(|c| c.1.clone()).unwrap_or_default()],
    )?;
    let mut order: Vec<String> = Vec::new();
    let mut by: std::collections::HashMap<String, Map<String, Value>> = Default::default();
    for mut r in rs.clone() {
        let k = r["item_id"].as_str().unwrap_or("").to_string();
        if !by.contains_key(&k) {
            parse_json_col(&mut r, "holds");
            let mut g = Map::new();
            g.insert("item_id".into(), json!(k));
            g.insert("title".into(), r["title"].clone());
            g.insert("author_id".into(), r["author_id"].clone());
            g.insert("holds".into(), r["holds"].clone());
            g.insert("reports".into(), json!([]));
            order.push(k.clone());
            by.insert(k.clone(), g);
        }
        let rep: Map<String, Value> = ["id", "reason", "note", "at", "reporter_id", "reporter_name", "reporter_reports", "reporter_dismissed", "reporter_imports", "reporter_items", "reporter_since"].iter().map(|f| (f.to_string(), r[*f].clone())).collect();
        by.get_mut(&k).unwrap()["reports"].as_array_mut().unwrap().push(Value::Object(rep));
    }
    let items: Vec<Value> = order.iter().map(|k| Value::Object(by[k].clone())).collect();
    let next = if rs.len() == REPORTS_PAGE { rs.last().map(|l| json!(format!("{}:{}", l["at"], l["id"].as_str().unwrap_or("")))) } else { None };
    Ok(json!({ "items": items, "next_cursor": next.unwrap_or(Value::Null) }))
}

/// GET /admin/pool/flags: open flags, most severe first, then newest, paged: `limit` (<= 200) and `cursor` (the last row's "severity:at:id").
pub fn list_flags(x: &PoolCtx, limit: Option<i64>, cursor_s: Option<&str>) -> Res<Value> {
    let limit = limit.filter(|l| *l != 0).unwrap_or(100).clamp(1, 200);
    let m: Option<(i64, i64, String)> = match cursor_s {
        None => None,
        Some(c) => {
            let p: Vec<&str> = c.splitn(3, ':').collect();
            let ok = p.len() == 3 && (1..=2).contains(&p[0].len()) && p[0].bytes().all(|b| b.is_ascii_digit()) && (1..=16).contains(&p[1].len()) && p[1].bytes().all(|b| b.is_ascii_digit()) && is_id(p[2]);
            if !ok {
                return Err(bad("cursor"));
            }
            Some((p[0].parse().unwrap(), p[1].parse().unwrap(), p[2].to_string()))
        }
    };
    let mut rs = rows(
        x.conn,
        &format!(
            "SELECT * FROM (SELECT id, kind, item_id, publish_id, user_id, detail, at, {} AS severity FROM admin_flags WHERE resolved_at IS NULL)
              WHERE ?1 IS NULL OR severity > ?1 OR (severity = ?1 AND (at < ?2 OR (at = ?2 AND id < ?3)))
              ORDER BY severity, at DESC, id DESC LIMIT ?4",
            flag_severity_sql()
        ),
        &[&m.as_ref().map(|c| c.0), &m.as_ref().map(|c| c.1), &m.as_ref().map(|c| c.2.clone()), &(limit + 1)],
    )?;
    let more = rs.len() as i64 > limit;
    rs.truncate(limit as usize);
    let total: i64 = x.conn.query_row("SELECT COUNT(*) FROM admin_flags WHERE resolved_at IS NULL", [], |r| r.get(0))?;
    let next = if more { rs.last().map(|l| json!(format!("{}:{}:{}", l["severity"], l["at"], l["id"].as_str().unwrap_or("")))) } else { None };
    Ok(json!({ "flags": rs, "total_open": total, "next_cursor": next.unwrap_or(Value::Null) }))
}

/// The JSON head of GET /admin/pool/item (the item in any state, its publishes, holds and takedown actions) and the current publish's body key, which
/// the route layer streams under a read reservation. None: no such item.
pub fn item_head(x: &PoolCtx, item_id: &str) -> Res<Option<(Value, Option<String>)>> {
    let item = rows(x.conn, "SELECT * FROM items WHERE id = ?1", &[&item_id])?;
    let Some(item) = item.into_iter().next() else { return Ok(None) };
    let pubs = rows(
        x.conn,
        "SELECT p.publish_id, p.version, p.status, p.hide_key, p.license, p.attested_at, p.attest_text_rev, p.full_key, p.committed_at, a.at AS terms_attested_at, a.terms_version
           FROM item_publishes p LEFT JOIN attestations a ON a.publish_id = p.publish_id WHERE p.item_id = ?1 ORDER BY p.version DESC",
        &[&item_id],
    )?;
    let holds = rows(x.conn, "SELECT reason, at, by, source FROM item_holds WHERE item_id = ?1", &[&item_id])?;
    let dmca = rows(
        x.conn,
        "SELECT id, notice_id, disabled_at, counter_notice_id, restore_not_before, court_action_at, restored_at, purged_at, outcome, rightful_item_id FROM dmca_actions WHERE item_id = ?1 ORDER BY disabled_at DESC",
        &[&item_id],
    )?;
    let current_id = item.get("current_publish_id").and_then(Value::as_str).map(str::to_string);
    let current_key = pubs.iter().find(|p| Some(p["publish_id"].as_str().unwrap_or("")) == current_id.as_deref()).and_then(|p| p["full_key"].as_str().map(str::to_string));
    let publishes: Vec<Value> = pubs.into_iter().map(|mut p| {
        p.remove("full_key");
        Value::Object(p)
    }).collect();
    Ok(Some((json!({ "item": item, "publishes": publishes, "holds": holds, "dmca_actions": dmca }), current_key)))
}

/// POST /admin/pool/hold: a manual hold (the item must exist).
pub fn hold(x: &PoolCtx, admin: &str, body: &Map<String, Value>) -> Res<(u16, Value)> {
    let item_id = id_field(body, "item_id")?;
    let n = note_field(body)?;
    let ins = x.conn.execute("INSERT OR IGNORE INTO item_holds (item_id, reason, at, by, source) SELECT ?1, 'admin', ?2, ?3, 'manual' WHERE EXISTS (SELECT 1 FROM items WHERE id = ?1)", params![item_id, x.now, admin])?;
    both(x, admin, "item.hold", Some(&item_id), "pool.hold", &item_id, None, n.as_deref())?;
    Ok((200, json!({ "ok": true, "changed": ins == 1 })))
}

fn remaining_holds(x: &PoolCtx, item_id: &str) -> Res<Vec<Map<String, Value>>> {
    Ok(rows(x.conn, "SELECT reason, source FROM item_holds WHERE item_id = ?1 ORDER BY reason, source", &[&item_id])?)
}

/// POST /admin/pool/release: lifting a hold; for student_data it is the admin rejecting the report. Source-aware: by default only the holds human
/// judgements placed (reports, a manual hold, and those carried to an evasion copy) go. Privacy, DMCA and needs_reupload holds belong to their own routes;
/// lifting one here needs its exact `source` and `force: true`, and is audited as forced.
pub fn release(x: &PoolCtx, admin: &str, body: &Map<String, Value>) -> Res<(u16, Value)> {
    let item_id = id_field(body, "item_id")?;
    let reason = match body.get("reason").and_then(Value::as_str) {
        Some(r) if HOLD_REASONS.contains(&r) => r.to_string(),
        _ => return Err(bad("reason")),
    };
    let source: Option<String> = match body.get("source") {
        None => None,
        Some(Value::String(s)) if is_source(s) => Some(s.clone()),
        Some(_) => return Err(bad("source")),
    };
    let forced = source.as_deref().is_some_and(|s| !is_human_source(s));
    if forced && body.get("force") != Some(&Value::Bool(true)) {
        return Err(PoolError::new(409, "hold_has_own_route").with("message", json!("That hold follows a block or takedown: lift it with its own route, or send force: true.")).into());
    }
    let n = note_field(body)?;
    let resolve = match reason.as_str() {
        "student_data" => "reason = 'student_data'",
        "reports" => "reason <> 'student_data'",
        _ => "0",
    };
    let released = match &source {
        None => x.conn.execute("DELETE FROM item_holds WHERE item_id = ?1 AND reason = ?2 AND (source IN ('', 'manual', 'key_leak') OR source LIKE 'carried:%')", params![item_id, reason])?,
        Some(s) => x.conn.execute("DELETE FROM item_holds WHERE item_id = ?1 AND reason = ?2 AND source = ?3", params![item_id, reason, s])?,
    };
    x.conn.execute(&format!("UPDATE reports SET resolved_at = ?2, resolved_by = ?3, action = 'dismissed' WHERE item_id = ?1 AND resolved_at IS NULL AND {resolve}"), params![item_id, x.now, admin])?;
    // Resolved: the keys a deleted held item left stop holding.
    x.conn.execute("DELETE FROM carried_keys WHERE item_id = ?1 AND reason = ?2", params![item_id, reason])?;
    let detail = json!({ "reason": reason, "source": source.clone().unwrap_or_else(|| "human".into()) });
    audit(x.conn, &AuditEntry { actor_id: Some(admin), actor_kind: "admin", action: if forced { "item.release_forced" } else { "item.release" }, item_id: Some(&item_id), target_user_id: None, detail: Some(detail.clone()), note: n.as_deref() }, x.now, None)?;
    auth_audit(x.conn, Some(admin), "admin", &AuditFields { action: "pool.release", target_id: Some(&item_id), note: n.as_deref(), detail: Some(detail), ..Default::default() }, x.now, None, false, &new_id())?;
    Ok((200, json!({ "ok": true, "released": released, "remaining": remaining_holds(x, &item_id)? })))
}

/// POST /admin/pool/release-hold: release ONE hold of one item, whatever placed it (a student-data or copyright block that hit an innocent item), with the
/// admin's reason (required, audited). The block keys the item uses now are recorded (`hold_releases`): its later publishes are not held again by them; new
/// blocked material still holds. The release names the exact publish the admin reviewed.
pub fn release_hold(x: &PoolCtx, admin: &str, body: &Map<String, Value>) -> Res<(u16, Value)> {
    let item_id = id_field(body, "item_id")?;
    let source = match body.get("source").and_then(Value::as_str) {
        Some(s) if is_source(s) => s.to_string(),
        _ => return Err(bad("source")),
    };
    let n = note_field(body)?;
    let Some(n) = n.filter(|n| n.chars().count() >= 10) else {
        return Err(PoolError::new(400, "invalid_request").with("field", json!("note")).with("message", json!("Say why this item is released (at least 10 characters).")).into());
    };
    let publish: Option<Option<String>> = x.conn.query_row("SELECT current_publish_id FROM items WHERE id = ?1 AND purged_at IS NULL", [&item_id], |r| r.get(0)).map(Some).or_else(|e| if e == rusqlite::Error::QueryReturnedNoRows { Ok(None) } else { Err(e) })?;
    let Some(pubn) = publish else { return Ok((404, json!({ "error": "not_found" }))) };
    let Some(named) = body.get("publish_id").and_then(Value::as_str) else {
        return Err(PoolError::new(400, "invalid_request").with("field", json!("publish_id")).with("message", json!("Name the publish you reviewed (publish_id).")).into());
    };
    if Some(named) != pubn.as_deref() {
        return Ok((409, json!({ "error": "publish_changed", "current_publish_id": pubn })));
    }
    let keys_sql = if source == "privacy" {
        format!("SELECT pb.hash FROM privacy_blocks pb WHERE (pb.kind = 'match' AND {}) OR (pb.kind = 'blob' AND pb.hash IN {})", super::sqlfrag::in_q_keys_of("pb.hash", "?2"), super::served::served_blobs_of("?2"))
    } else if source.starts_with("dmca:") {
        format!("SELECT bh.hash FROM blocked_hashes bh WHERE bh.action_id = substr(?3, 6) AND ((bh.kind = 'match' AND {}) OR (bh.kind = 'blob' AND bh.hash IN {}))", super::sqlfrag::in_q_keys_of("bh.hash", "?2"), super::served::served_blobs_of("?2"))
    } else {
        "SELECT NULL AS hash WHERE 0".to_string()
    };
    let tx = x.conn.unchecked_transaction()?;
    let released = tx.execute("DELETE FROM item_holds WHERE item_id = ?1 AND source = ?2", params![item_id, source])?;
    tx.execute(
        &format!(
            "INSERT INTO hold_releases (item_id, source, keys_json, at, by, note) SELECT ?1, ?3, COALESCE((SELECT json_group_array(hash) FROM ({keys_sql})), '[]'), ?4, ?5, ?6
             ON CONFLICT (item_id, source) DO UPDATE SET keys_json = (SELECT json_group_array(DISTINCT v) FROM (SELECT value AS v FROM json_each(hold_releases.keys_json) UNION SELECT value FROM json_each(excluded.keys_json))), at = excluded.at, by = excluded.by, note = excluded.note"
        ),
        params![item_id, pubn, source, x.now, admin, n],
    )?;
    let detail = json!({ "source": source });
    audit(&tx, &AuditEntry { actor_id: Some(admin), actor_kind: "admin", action: "item.release_hold", item_id: Some(&item_id), target_user_id: None, detail: Some(detail.clone()), note: Some(&n) }, x.now, None)?;
    auth_audit(&tx, Some(admin), "admin", &AuditFields { action: "pool.release_hold", target_id: Some(&item_id), note: Some(&n), detail: Some(detail), ..Default::default() }, x.now, None, false, &new_id())?;
    tx.commit()?;
    Ok((200, json!({ "ok": true, "released": released, "remaining": remaining_holds(x, &item_id)? })))
}

/// POST /admin/pool/dismiss: dismisses one report. A key_leak hold needs two QUALIFYING open reports on its publish; below that it goes.
pub fn dismiss(x: &PoolCtx, admin: &str, body: &Map<String, Value>) -> Res<(u16, Value)> {
    let rid = id_field(body, "report_id")?;
    let n = note_field(body)?;
    let tx = x.conn.unchecked_transaction()?;
    let upd = tx.execute("UPDATE reports SET resolved_at = ?2, resolved_by = ?3, action = 'dismissed' WHERE id = ?1 AND resolved_at IS NULL", params![rid, x.now, admin])?;
    tx.execute(
        &format!(
            "DELETE FROM item_holds WHERE source = 'key_leak' AND item_id = (SELECT item_id FROM reports WHERE id = ?1)
               AND (SELECT COUNT(DISTINCT r.reporter_id) FROM reports r JOIN users ru ON ru.id = r.reporter_id
                     WHERE {}) < 2",
            key_leak_qualifies("item_holds.item_id", "(item_holds.detail ->> '$.publish_id')", "?2")
        ),
        params![rid, x.now],
    )?;
    audit(&tx, &AuditEntry { actor_id: Some(admin), actor_kind: "admin", action: "report.dismiss", item_id: None, target_user_id: None, detail: Some(json!({ "report_id": rid })), note: n.as_deref() }, x.now, None)?;
    auth_audit(&tx, Some(admin), "admin", &AuditFields { action: "pool.report_dismiss", target_id: Some(&rid), note: n.as_deref(), ..Default::default() }, x.now, None, false, &new_id())?;
    tx.commit()?;
    Ok(if upd == 1 { (200, json!({ "ok": true })) } else { (404, json!({ "error": "not_found" })) })
}

/// POST /admin/pool/remove: removal for other policy reasons: hidden now, purged after 14 days.
pub fn remove(x: &PoolCtx, admin: &str, body: &Map<String, Value>) -> Res<(u16, Value)> {
    let item_id = id_field(body, "item_id")?;
    let n = note_field(body)?;
    let tx = x.conn.unchecked_transaction()?;
    let upd = tx.execute(
        "UPDATE items SET prior_removed_reason = removed_reason, prior_purge_after = purge_after, removed_at = COALESCE(removed_at, ?2),
           removed_reason = 'admin', purge_after = ?3, updated_at = ?2 WHERE id = ?1 AND purged_at IS NULL AND (removed_reason IS NULL OR removed_reason = 'author_deleted')",
        params![item_id, x.now, x.now + UNDO_MS],
    )?;
    tx.execute("UPDATE reports SET resolved_at = ?2, resolved_by = ?3, action = 'removed' WHERE item_id = ?1 AND resolved_at IS NULL AND changes() = 1", params![item_id, x.now, admin])?;
    audit(&tx, &AuditEntry { actor_id: Some(admin), actor_kind: "admin", action: "item.remove", item_id: Some(&item_id), target_user_id: None, detail: None, note: n.as_deref() }, x.now, None)?;
    auth_audit(&tx, Some(admin), "admin", &AuditFields { action: "pool.remove", target_id: Some(&item_id), note: n.as_deref(), ..Default::default() }, x.now, None, false, &new_id())?;
    tx.commit()?;
    Ok(if upd == 1 { (200, json!({ "ok": true })) } else { (409, json!({ "error": "not_removable" })) })
}

/// POST /admin/pool/unremove: reverses an admin removal within its 14 days, back to what it replaced.
pub fn unremove(x: &PoolCtx, admin: &str, body: &Map<String, Value>) -> Res<(u16, Value)> {
    let item_id = id_field(body, "item_id")?;
    let n = note_field(body)?;
    match admin_unremove(x, &item_id, &Actor { id: Some(admin), kind: "admin" }, n.as_deref())? {
        Decided::NotFound => Ok((404, json!({ "error": "not_found" }))),
        Decided::Ok => {
            trail(x, admin, "pool.unremove", &item_id, n.as_deref())?;
            Ok((200, json!({ "ok": true })))
        }
        Decided::OverLimits => Ok((409, json!({ "error": "over_limits", "message": "Restoring it would take the author over their storage quota or question cap." }))),
    }
}

#[cfg(test)]
#[path = "adminpool_tests.rs"]
mod tests;
