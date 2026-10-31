//! A circle's activity list and the bank heads route (worker/src/pool/circles.ts "activity (D8)" and "POST /circles/heads"). HTTP-free cores.
use super::circlebase::*;
use super::circles::bad_field;
use super::circlesql::*;
use super::ctx::{PoolCtx, Res};
use super::sqlfrag::is_id;
use rusqlite::{ToSql, params};
use serde_json::{Map, Value, json};
use std::collections::{HashMap, HashSet};

/// What members see: joins, leaves, removals, role changes, transfers, banks and versions. Never codes, reports or admin opens.
const ACTIVITY: [(&str, &str); 22] = [
    ("circle.create", "created"),
    ("circle.join", "joined"),
    ("circle.leave", "left"),
    ("circle.member_remove", "removed"),
    ("circle.member_erased", "left"),
    ("circle.role", "role_changed"),
    ("circle.transfer", "ownership_transferred"),
    ("circle.owner_succession", "ownership_passed_on"),
    ("circle.close", "closed"),
    ("circle.reopen", "reopened"),
    ("circle.edit", "renamed"),
    ("circle.bank_add", "bank_shared"),
    ("circle.version", "version_sent"),
    ("circle.rollback", "rolled_back"),
    ("circle.versions_trimmed", "versions_trimmed"),
    ("circle.bank_remove", "bank_removed"),
    ("circle.bank_undelete", "bank_restored"),
    ("circle.bank_license", "license_changed"),
    ("circle.admin_close", "closed"),
    ("circle.admin_bank_remove", "bank_removed"),
    ("circle.email_invite", "email_invited"),
    ("circle.bank_hide_key", "hide_key_changed"),
];

fn cursor_error() -> super::ctx::Fail {
    bad_field("before", &[("message", "Use the next_before of the previous page.".into())])
}

/// GET /circles/:cid/activity?before=<at>:<id>: newest first (written order within one time), ACTIVITY_PAGE a page, names but no technical
/// data. The cursor is the last line's time and its audit row's permanent id; a malformed cursor is a 400, never page one again.
pub fn activity(x: &PoolCtx, me: &str, cid: &str, before: Option<&str>) -> Res<Value> {
    let m = circle_or_miss(x, cid, me)?;
    let cur: Option<(i64, String)> = match before {
        None => None,
        Some(raw) => {
            let parsed = raw.split_once(':').and_then(|(t, i)| {
                let ok = !t.is_empty() && t.len() <= 16 && t.bytes().all(|b| b.is_ascii_digit()) && (32..=36).contains(&i.len()) && i.bytes().all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b) || b == b'-');
                if ok { t.parse::<i64>().ok().map(|n| (n, i.to_string())) } else { None }
            });
            Some(parsed.ok_or_else(cursor_error)?)
        }
    };
    // The cursor's line must be one of THIS circle's, at the time it names.
    if let Some((at, id)) = &cur {
        let there: Option<i64> = x.conn.query_row("SELECT 1 FROM pool_audit WHERE id = ?1 AND item_id = ?2 AND at = ?3", params![id, cid, at], |r| r.get(0)).ok();
        if there.is_none() {
            return Err(cursor_error());
        }
    }
    let list = ACTIVITY.iter().map(|(a, _)| format!("'{a}'")).collect::<Vec<_>>().join(", ");
    let sql = format!(
        "SELECT a.id, a.at, a.action, a.actor_kind, a.detail, au.display_name AS an, au.deleted_at AS ad, a.actor_id IS NULL AS anull, tu.display_name AS tn, tu.deleted_at AS td, a.target_user_id IS NULL AS tnull
         FROM pool_audit a LEFT JOIN users au ON au.id = a.actor_id LEFT JOIN users tu ON tu.id = a.target_user_id
        WHERE a.item_id = ?1 AND a.action IN ({list})
          {}
        ORDER BY a.at DESC, a.rowid DESC LIMIT ?2",
        if cur.is_some() { "AND a.at <= ?3 AND (a.at < ?3 OR a.rowid < (SELECT ca.rowid FROM pool_audit ca WHERE ca.id = ?4 AND ca.item_id = ?1))" } else { "" }
    );
    struct Row {
        id: String,
        at: i64,
        action: String,
        actor_kind: String,
        detail: Option<String>,
        by: Option<String>,
        member: Option<String>,
    }
    let page_plus = (ACTIVITY_PAGE + 1) as i64;
    let mut args: Vec<&dyn ToSql> = vec![&cid, &page_plus];
    if let Some((at, id)) = &cur {
        args.push(at);
        args.push(id);
    }
    let mut rows: Vec<Row> = x
        .conn
        .prepare(&sql)?
        .query_map(args.as_slice(), |r| {
            let anull: i64 = r.get(7)?;
            let tnull: i64 = r.get(10)?;
            Ok(Row {
                id: r.get(0)?,
                at: r.get(1)?,
                action: r.get(2)?,
                actor_kind: r.get(3)?,
                detail: r.get(4)?,
                by: if anull == 1 { None } else { Some(name_of(r.get::<_, Option<String>>(5)?.as_deref(), r.get(6)?)) },
                member: if tnull == 1 { None } else { Some(name_of(r.get::<_, Option<String>>(8)?.as_deref(), r.get(9)?)) },
            })
        })?
        .collect::<Result<_, _>>()?;
    let more = rows.len() > ACTIVITY_PAGE;
    rows.truncate(ACTIVITY_PAGE);
    let details: Vec<Map<String, Value>> = rows.iter().map(|r| r.detail.as_deref().and_then(|d| serde_json::from_str::<Value>(d).ok()).and_then(|v| v.as_object().cloned()).unwrap_or_default()).collect();
    let mut bank_ids: Vec<String> = Vec::new();
    for d in &details {
        if let Some(b) = d.get("bank_id").and_then(Value::as_str).filter(|b| is_id(b)) {
            if !bank_ids.iter().any(|x| x == b) {
                bank_ids.push(b.into());
            }
        }
    }
    // A bank's title only to who may see the bank, as on the circle page (a held bank's title can be the student data held).
    let mut titles: HashMap<String, String> = HashMap::new();
    if !bank_ids.is_empty() {
        let mut st = x.conn.prepare(&format!(
            "SELECT b.id, b.title, b.author_id, b.removed_at, EXISTS (SELECT 1 FROM circle_holds h WHERE h.bank_id = b.id) AS held, {} AS blocking, {} AS author_ok
             FROM circle_banks b WHERE b.id IN (SELECT value FROM json_each(?1)) AND b.circle_id = ?2 AND b.purged_at IS NULL",
            blocking_hold_sql("b.id"),
            author_ok_sql("b")
        ))?;
        let mut q = st.query(params![serde_json::to_string(&bank_ids).unwrap(), cid])?;
        while let Some(t) = q.next()? {
            let (id, title, author, removed_at): (String, String, String, Option<i64>) = (t.get(0)?, t.get(1)?, t.get(2)?, t.get(3)?);
            let (held, blocking, author_ok) = (t.get::<_, i64>(4)? == 1, t.get::<_, i64>(5)? == 1, t.get::<_, i64>(6)? == 1);
            let mine = author == me;
            let owner_sees = m.role == "owner" && !blocking;
            if author_ok && (mine || ((!held || owner_sees) && (removed_at.is_none() || m.role == "owner"))) {
                titles.insert(id, title);
            }
        }
    }
    let next = if more { rows.last().map(|l| format!("{}:{}", l.at, l.id)) } else { None };
    let out: Vec<Value> = rows
        .iter()
        .zip(&details)
        .map(|(r, d)| {
            let mut o = Map::new();
            o.insert("at".into(), r.at.into());
            o.insert("what".into(), ACTIVITY.iter().find(|(a, _)| *a == r.action).map(|(_, w)| *w).unwrap_or("other").into());
            o.insert("by".into(), if r.actor_kind == "admin" { "an administrator".into() } else { r.by.clone().map(Value::from).unwrap_or(Value::Null) });
            o.insert("member".into(), r.member.clone().map(Value::from).unwrap_or(Value::Null));
            if let Some(v) = d.get("role").filter(|v| v.is_string()) {
                o.insert("role".into(), v.clone());
            }
            if let Some(b) = d.get("bank_id").and_then(Value::as_str) {
                o.insert("bank_id".into(), b.into());
                o.insert("bank_title".into(), titles.get(b).cloned().map(Value::from).unwrap_or(Value::Null));
            }
            for k in ["version", "rollback_of", "merged_from"] {
                if let Some(v) = d.get(k).filter(|v| v.is_number()) {
                    o.insert(k.into(), v.clone());
                }
            }
            if let Some(v) = d.get("license").filter(|v| v.is_string()) {
                o.insert("license".into(), v.clone());
            }
            if let Some(v) = d.get("hide_key").filter(|v| v.is_boolean()) {
                o.insert("hide_key".into(), v.clone());
            }
            Value::Object(o)
        })
        .collect();
    Ok(json!({ "activity": out, "next_before": next }))
}

/// POST /circles/heads {bank_ids} (up to HEADS_MAX): per id the head (version, when, who, licence, `hide_key`), `removed`,
/// `under_review` or `unavailable`.
pub fn heads(x: &PoolCtx, me: &str, b: &Map<String, Value>) -> Res<Value> {
    let ids: Vec<&str> = match b.get("bank_ids").and_then(Value::as_array) {
        Some(a) if !a.is_empty() && a.len() <= HEADS_MAX && a.iter().all(|v| v.as_str().is_some_and(|s| s.encode_utf16().count() <= 64)) => a.iter().map(|v| v.as_str().unwrap()).collect(),
        _ => return Err(bad_field("bank_ids", &[("max", HEADS_MAX.into())])),
    };
    x.require_daily(me, "item_reads", 1)?;
    let mut seen = HashSet::new();
    let uniq: Vec<&str> = ids.iter().copied().filter(|i| seen.insert(*i)).filter(|i| is_id(i)).collect();
    struct R {
        author: String,
        head_version: i64,
        head_version_id: Option<String>,
        updated_at: i64,
        removed_at: Option<i64>,
        removed_reason: Option<String>,
        purge_after: Option<i64>,
        license: String,
        hide_key: bool,
        role: String,
        blocking: bool,
        closed_at: Option<i64>,
        committed_at: Option<i64>,
        by: String,
        held: bool,
        author_ok: bool,
    }
    let mut by: HashMap<String, R> = HashMap::new();
    {
        let mut st = x.conn.prepare(&format!(
            "SELECT j.value AS id, b.author_id, b.head_version, b.head_version_id, b.updated_at, b.removed_at, b.removed_reason, b.purge_after, b.license, b.hide_key, m.role, {} AS blocking,
              ci.closed_at, v.committed_at, v.committer_id, cu.display_name AS cn, cu.deleted_at AS cd,
              EXISTS (SELECT 1 FROM circle_holds h WHERE h.bank_id = b.id) AS held, {} AS author_ok
         FROM json_each(?2) j
         CROSS JOIN circle_banks b ON b.id = j.value AND b.purged_at IS NULL AND b.head_version > 0
         JOIN circles ci ON ci.id = b.circle_id AND ci.purged_at IS NULL
         JOIN circle_members m ON m.circle_id = b.circle_id AND m.user_id = ?1 AND m.left_at IS NULL
         LEFT JOIN circle_versions v ON v.id = b.head_version_id
         LEFT JOIN users cu ON cu.id = v.committer_id",
            blocking_hold_sql("b.id"),
            author_ok_sql("b")
        ))?;
        let mut q = st.query(params![me, serde_json::to_string(&uniq).unwrap()])?;
        while let Some(r) = q.next()? {
            let committer: Option<String> = r.get(14)?;
            let cd: Option<i64> = r.get(16)?;
            by.insert(
                r.get(0)?,
                R {
                    author: r.get(1)?,
                    head_version: r.get(2)?,
                    head_version_id: r.get(3)?,
                    updated_at: r.get(4)?,
                    removed_at: r.get(5)?,
                    removed_reason: r.get(6)?,
                    purge_after: r.get(7)?,
                    license: r.get(8)?,
                    hide_key: r.get::<_, i64>(9)? == 1,
                    role: r.get(10)?,
                    blocking: r.get::<_, i64>(11)? == 1,
                    closed_at: r.get(12)?,
                    committed_at: r.get(13)?,
                    by: name_of(r.get::<_, Option<String>>(15)?.as_deref(), if committer.is_none() { Some(1) } else { cd }),
                    held: r.get::<_, i64>(17)? == 1,
                    author_ok: r.get::<_, i64>(18)? == 1,
                },
            );
        }
    }
    let banks: Vec<Value> = ids
        .iter()
        .map(|id| match by.get(*id) {
            None => json!({ "bank_id": id, "status": "unavailable" }),
            Some(r) if r.closed_at.is_some() => json!({ "bank_id": id, "status": "unavailable" }),
            Some(r) if r.removed_at.is_some() => {
                if matches!(r.removed_reason.as_deref(), Some("author_removed") | Some("owner_removed")) {
                    json!({ "bank_id": id, "status": "removed", "until": r.purge_after })
                } else {
                    json!({ "bank_id": id, "status": "removed" })
                }
            }
            Some(r) if (r.held && r.author != me && !(r.role == "owner" && !r.blocking)) || !r.author_ok => json!({ "bank_id": id, "status": "under_review" }),
            Some(r) => json!({ "bank_id": id, "status": "current", "version": r.head_version, "version_id": r.head_version_id, "license": r.license, "at": r.committed_at.unwrap_or(r.updated_at), "by": r.by, "hide_key": r.hide_key }),
        })
        .collect();
    Ok(json!({ "banks": banks }))
}

#[cfg(test)]
#[path = "circleactivity_tests.rs"]
mod tests;
