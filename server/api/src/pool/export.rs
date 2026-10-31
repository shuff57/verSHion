//! Self-service account export, GET /me/export (worker/src/pool/export.ts). Everything the pool holds about the account that is
//! the account's own: its profile, its items (the FULL form of each retained publish: the author's own hidden keys included), its
//! imports (ids, titles, the licence shown, dates; no one else's content or personal data), its reports, the notices it received,
//! its attestations, counter-notices and appeals, its sessions (no tokens). Never: another user's personal data, admin notes,
//! internal flags, hashes or sketches.
//!
//! HTTP-free: the route sets the attachment headers (`Content-Disposition`, `Cache-Control: no-store`, `nosniff`) from
//! `ExportOut::filename`. The circles section is supplied by the caller (`circles`), until circles.ts is ported.
use super::ctx::{AuditEntry, PoolCtx, PoolError, Res, audit, bad};
use super::sqlfrag::visible_to_others;
use crate::blobs::BlobStore;
use rusqlite::{OptionalExtension, params, types::ValueRef};
use serde_json::{Map, Value, json};
use std::collections::HashMap;

/// A whole export above `whole` bytes of item bodies goes out as a manifest and parts of at most `part` (tests lower them).
#[derive(Clone, Copy, Debug)]
pub struct ExportLimits {
    pub whole: i64,
    pub part: i64,
}
pub const EXPORT_LIMITS: ExportLimits = ExportLimits { whole: 8 * 1024 * 1024, part: 4 * 1024 * 1024 };
/// The session must have logged in this recently.
pub const EXPORT_FRESH_MS: i64 = 24 * 3600_000;
pub const EXPORT_FORMAT: &str = "vershion-pool-export/v1";

/// The caller as the auth layer resolved them.
pub struct ExportWho<'a> {
    pub user_id: &'a str,
    pub trust: &'a str,
    /// The session's login family (its age is checked).
    pub session_family: Option<&'a str>,
}

pub struct ExportOut {
    pub body: Value,
    pub filename: String,
}

/// Every row of `sql` (bound at `?1`) as a JSON object keyed by column name.
fn rows(x: &PoolCtx, sql: &str, me: &str) -> rusqlite::Result<Vec<Value>> {
    let mut st = x.conn.prepare(sql)?;
    let names: Vec<String> = st.column_names().iter().map(|s| s.to_string()).collect();
    let out = st
        .query_map([me], |r| {
            let mut m = Map::new();
            for (i, n) in names.iter().enumerate() {
                m.insert(
                    n.clone(),
                    match r.get_ref(i)? {
                        ValueRef::Null => Value::Null,
                        ValueRef::Integer(v) => json!(v),
                        ValueRef::Real(v) => json!(v),
                        ValueRef::Text(t) => json!(String::from_utf8_lossy(t)),
                        ValueRef::Blob(_) => Value::Null,
                    },
                );
            }
            Ok(Value::Object(m))
        })?
        .collect::<Result<_, _>>()?;
    Ok(out)
}

struct ItemRow {
    id: String,
    title: String,
    subject: String,
    license: String,
    visibility: String,
    removed_reason: Option<String>,
    created_at: i64,
    updated_at: i64,
    current_publish_id: Option<String>,
    full_key: Option<String>,
    full_bytes: Option<i64>,
}
struct PubRow {
    item_id: String,
    publish_id: String,
    version: i64,
    status: String,
    license: String,
    committed_at: Option<i64>,
    full_key: Option<String>,
    full_bytes: Option<i64>,
}

fn headers_name(n: &str) -> String {
    n.to_string()
}

/// `part`: the raw `?part=` value. `circles`: the circles section of the export (memberships, circles owned, codes created (never
/// the codes), versions sent (no bodies), banks authored, reports filed).
pub fn export_account(
    x: &PoolCtx,
    store: &BlobStore,
    who: &ExportWho,
    part: Option<&str>,
    limits: ExportLimits,
    circles: &dyn Fn(&PoolCtx, &str) -> Res<Value>,
) -> Res<ExportOut> {
    let me = who.user_id;
    type U = (String, Option<String>, Option<String>, Option<String>, String, i64, Option<i64>, Option<i64>);
    let user: Option<U> = x
        .conn
        .query_row(
            "SELECT id, email, display_name, profile, trust, created_at, email_verified_at, deleted_at FROM users WHERE id = ?1",
            [me],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?, r.get(5)?, r.get(6)?, r.get(7)?)),
        )
        .optional()?;
    let Some((uid, email, display_name, profile_json, trust, created_at, verified_at, _)) = user.filter(|u| u.7.is_none()) else {
        return Err(PoolError::new(404, "not_found").into());
    };
    let full = who.trust == "teacher" || who.trust == "admin";
    if !full && who.trust != "pending" {
        return Err(PoolError::new(403, "account_not_approved").into());
    }
    // A fresh session: logged in within the last 24 hours (no re-authentication otherwise).
    let fresh: Option<i64> = x
        .conn
        .query_row("SELECT MAX(auth_at) FROM sessions WHERE family_id = ?1 AND user_id = ?2 AND revoked_at IS NULL", params![who.session_family.unwrap_or(""), me], |r| r.get(0))?;
    if fresh.is_none_or(|a| a < x.now - EXPORT_FRESH_MS) {
        return Err(PoolError::new(403, "reauth_required").with("message", "Sign in again to export your account (an export needs a login in the last 24 hours).".into()).into());
    }
    // `^[1-9][0-9]{0,4}$`
    let part_no: Option<usize> = match part {
        None => None,
        Some(p) if (1..=5).contains(&p.len()) && !p.starts_with('0') && p.bytes().all(|b| b.is_ascii_digit()) => Some(p.parse().unwrap()),
        Some(_) => return Err(bad("part").into()),
    };
    // Three exports a day (a part of one already counted does not count again).
    if part_no.is_none() {
        x.require_daily(me, "exports", 1)?;
    }
    let profile: Value = profile_json.as_deref().and_then(|p| serde_json::from_str(p).ok()).unwrap_or(Value::Null);
    let head = json!({
        "format": EXPORT_FORMAT,
        "exported_at": x.now,
        "profile": { "id": uid, "email": email, "display_name": display_name, "profile": profile, "approval": trust, "created_at": created_at, "email_verified_at": verified_at },
    });
    let audit_row = |detail: Value| -> rusqlite::Result<()> {
        audit(x.conn, &AuditEntry { actor_id: Some(me), actor_kind: "user", action: "account.export", target_user_id: Some(me), detail: Some(detail), ..Default::default() }, x.now, None)?;
        Ok(())
    };
    if !full {
        if part_no.is_some() {
            return Err(PoolError::new(404, "not_found").into());
        }
        audit_row(json!({ "profile_only": true }))?;
        let mut body = head;
        body["profile_only"] = json!(true);
        return Ok(ExportOut { body, filename: headers_name("vershion-export.json") });
    }
    let items: Vec<ItemRow> = x
        .conn
        .prepare(
            "SELECT i.id, i.title, i.subject, i.license, i.visibility, i.removed_reason, i.created_at, i.updated_at, i.current_publish_id, p.full_key, p.full_bytes
               FROM items i LEFT JOIN item_publishes p ON p.publish_id = i.current_publish_id
              WHERE i.author_id = ?1 AND i.purged_at IS NULL ORDER BY i.created_at, i.id",
        )?
        .query_map([me], |r| {
            Ok(ItemRow { id: r.get(0)?, title: r.get(1)?, subject: r.get(2)?, license: r.get(3)?, visibility: r.get(4)?, removed_reason: r.get(5)?, created_at: r.get(6)?, updated_at: r.get(7)?, current_publish_id: r.get(8)?, full_key: r.get(9)?, full_bytes: r.get(10)? })
        })?
        .collect::<Result<_, _>>()?;
    // Every RETAINED version, with its own full body, not only the current one (`retain()` keeps current, the previous one and
    // the one non-authors are served).
    let retained: Vec<PubRow> = x
        .conn
        .prepare(
            "SELECT item_id, publish_id, version, status, license, committed_at, full_key, full_bytes FROM item_publishes
              WHERE author_id = ?1 AND status IN ('current', 'previous') ORDER BY item_id, version",
        )?
        .query_map([me], |r| Ok(PubRow { item_id: r.get(0)?, publish_id: r.get(1)?, version: r.get(2)?, status: r.get(3)?, license: r.get(4)?, committed_at: r.get(5)?, full_key: r.get(6)?, full_bytes: r.get(7)? }))?
        .collect::<Result<_, _>>()?;
    let mut retained_of: HashMap<&str, Vec<&PubRow>> = HashMap::new();
    for v in &retained {
        retained_of.entry(&v.item_id).or_default().push(v);
    }
    let item_bytes: Vec<i64> = items
        .iter()
        .map(|it| match retained_of.get(it.id.as_str()) {
            Some(vs) => vs.iter().map(|v| v.full_bytes.unwrap_or(0)).sum(),
            None => it.full_bytes.unwrap_or(0),
        })
        .collect();
    // Parts: the items' retained bodies in order, at most `limits.part` stored bytes each (one oversized item is its own part).
    let mut parts: Vec<Vec<&str>> = Vec::new();
    let (mut cur, mut size): (Vec<&str>, i64) = (Vec::new(), 0);
    for (it, b) in items.iter().zip(&item_bytes) {
        if !cur.is_empty() && size + b > limits.part {
            parts.push(std::mem::take(&mut cur));
            size = 0;
        }
        cur.push(&it.id);
        size += b;
    }
    if !cur.is_empty() {
        parts.push(cur);
    }
    let total: i64 = item_bytes.iter().sum();
    let read_bodies = |pairs: Vec<(String, Option<String>)>| -> Value {
        let mut out = Map::new();
        for (id, key) in pairs {
            let Some(key) = key else { continue };
            let Ok(Some(bytes)) = store.get(&key) else { continue };
            out.insert(id, serde_json::from_slice::<Value>(&bytes).unwrap_or(Value::Null));
        }
        Value::Object(out)
    };
    let current_pub: std::collections::HashSet<&str> = items.iter().filter_map(|i| i.current_publish_id.as_deref()).collect();
    let current_pairs = |ids: &std::collections::HashSet<&str>| -> Vec<(String, Option<String>)> { items.iter().filter(|i| ids.contains(i.id.as_str())).map(|i| (i.id.clone(), i.full_key.clone())).collect() };
    // The current publish's body is `bodies[item_id]`; it is not repeated in version_bodies.
    let version_pairs = |ids: &std::collections::HashSet<&str>| -> Vec<(String, Option<String>)> {
        retained.iter().filter(|v| ids.contains(v.item_id.as_str()) && !current_pub.contains(v.publish_id.as_str())).map(|v| (v.publish_id.clone(), v.full_key.clone())).collect()
    };
    if let Some(n) = part_no {
        let ids = parts.get(n - 1).ok_or_else(|| PoolError::new(404, "not_found"))?;
        let set: std::collections::HashSet<&str> = ids.iter().copied().collect();
        let body = json!({ "format": EXPORT_FORMAT, "part": n, "of": parts.len(), "bodies": read_bodies(current_pairs(&set)), "version_bodies": read_bodies(version_pairs(&set)) });
        return Ok(ExportOut { body, filename: format!("vershion-export-part-{n}.json") });
    }
    let notices: Vec<Value> = rows(x, "SELECT kind, at, payload FROM pool_outbox WHERE json_extract(payload, '$.authorId') = ?1 ORDER BY at", me)?
        .into_iter()
        .map(|n| {
            let p: Value = n["payload"].as_str().and_then(|s| serde_json::from_str(s).ok()).unwrap_or(Value::Null);
            json!({ "kind": n["kind"], "at": n["at"], "item_id": p.get("itemId").cloned().unwrap_or(Value::Null), "action_id": p.get("actionId").cloned().unwrap_or(Value::Null) })
        })
        .collect();
    let appeals: Vec<Value> = rows(x, "SELECT item_id, at, detail, resolved_at FROM admin_flags WHERE kind = 'appeal' AND user_id = ?1 ORDER BY at", me)?
        .into_iter()
        .map(|a| {
            let note = a["detail"].as_str().and_then(|d| serde_json::from_str::<Value>(d).ok()).and_then(|d| d.get("note").cloned()).unwrap_or(Value::Null);
            json!({ "item_id": a["item_id"], "at": a["at"], "note": note, "resolved_at": a["resolved_at"] })
        })
        .collect();
    let mut data = head;
    let o = data.as_object_mut().unwrap();
    o.insert(
        "items".into(),
        Value::Array(
            items
                .iter()
                .map(|it| {
                    let versions: Vec<Value> = retained_of
                        .get(it.id.as_str())
                        .map(|vs| {
                            vs.iter().map(|v| json!({ "publish_id": v.publish_id, "version": v.version, "status": v.status, "license": v.license, "committed_at": v.committed_at })).collect()
                        })
                        .unwrap_or_default();
                    json!({
                        "id": it.id, "title": it.title, "subject": it.subject, "license": it.license, "visibility": it.visibility,
                        "state": it.removed_reason.clone().unwrap_or_else(|| "live".into()), "created_at": it.created_at, "updated_at": it.updated_at, "versions": versions,
                    })
                })
                .collect(),
        ),
    );
    // What it imported or read: the item, its title, the licence it was shown, when. No content. An item that is no longer visible
    // to others (unlisted, held, removed) has no title here: its author's title is private, and an importer is not its author.
    let imports = rows(
        x,
        &format!(
            "SELECT im.item_id, (SELECT title FROM items i JOIN users u ON u.id = i.author_id WHERE i.id = im.item_id AND {}) AS title, im.kind, im.version, im.license, im.at FROM imports im WHERE im.user_id = ?1 ORDER BY im.at",
            visible_to_others()
        ),
        me,
    )?;
    o.insert("imports".into(), Value::Array(imports));
    o.insert("reports".into(), Value::Array(rows(x, "SELECT item_id, reason, note, at, resolved_at, action FROM reports WHERE reporter_id = ?1 ORDER BY at", me)?));
    // Notices it received: takedowns of its items (no complainant identity) and the mails sent to it.
    o.insert(
        "takedowns".into(),
        Value::Array(rows(
            x,
            "SELECT id AS action_id, item_id, disabled_at, counter_notice_id IS NOT NULL AS countered, restored_at, purged_at, outcome FROM dmca_actions WHERE author_id = ?1 ORDER BY disabled_at",
            me,
        )?),
    );
    o.insert("notices".into(), Value::Array(notices));
    o.insert("attestations".into(), Value::Array(rows(x, "SELECT publish_id, item_id, at, terms_version FROM attestations WHERE user_id = ?1 ORDER BY at", me)?));
    o.insert(
        "counter_notices".into(),
        Value::Array(rows(
            x,
            "SELECT cn.action_id, cn.received_at, cn.name, cn.address, cn.phone, cn.signature, cn.material_desc FROM dmca_counter_notices cn JOIN dmca_actions a ON a.id = cn.action_id WHERE a.author_id = ?1 ORDER BY cn.received_at",
            me,
        )?),
    );
    o.insert("appeals".into(), Value::Array(appeals));
    o.insert("sessions".into(), Value::Array(rows(x, "SELECT created_at, auth_at, expires_at, revoked_at, user_agent FROM sessions WHERE user_id = ?1 ORDER BY created_at", me)?));
    o.insert("circles".into(), circles(x, me)?);
    let too_large = total > limits.whole;
    audit_row(json!({ "items": items.len(), "parts": if too_large { parts.len() } else { 1 } }))?;
    if too_large {
        o.insert("export_too_large".into(), json!(true));
        o.insert("manifest".into(), json!({ "parts": parts.iter().enumerate().map(|(i, ids)| json!({ "part": i + 1, "items": ids, "url": format!("/me/export?part={}", i + 1) })).collect::<Vec<_>>() }));
        return Ok(ExportOut { body: data, filename: "vershion-export.json".into() });
    }
    let all: std::collections::HashSet<&str> = items.iter().map(|i| i.id.as_str()).collect();
    // Each retained version's own full body, its hidden keys included: it is the author's.
    o.insert("bodies".into(), read_bodies(current_pairs(&all)));
    o.insert("version_bodies".into(), read_bodies(version_pairs(&all)));
    Ok(ExportOut { body: data, filename: "vershion-export.json".into() })
}

#[cfg(test)]
#[path = "export_tests.rs"]
mod tests;
