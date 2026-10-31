//! GET /items/mine (worker/src/pool/read.ts `mine`): the caller's own items, newest first, paged by a cursor, each with the
//! state the author needs to act on (live, deleted with its undo window, taken down with the counter-notice window, removed
//! for privacy with its appeal window, held for review, ...). HTTP-free: the route passes the `after` query value.
use super::blobs::usage;
use super::ctx::{PoolCtx, Res};
use super::recordserved::PRIVACY_APPEAL_MS;
use super::sqlfrag::DAY_MS;
use rusqlite::params;
use serde_json::{Map, Value, json};
use std::collections::HashMap;

/// Items one page of /items/mine lists (never cut at 1,000).
pub const MINE_PAGE: usize = 1000;
/// A takedown's counter-notice window: the author was notified less than this long ago.
pub const QUARANTINE_MS: i64 = 30 * DAY_MS;

/// Whether a takedown's counter-notice window is open: the same rule as the counter-notice route's SQL
/// (`author_notified_at > now - QUARANTINE_MS`); the daily cron upholds from `author_notified_at <= now - QUARANTINE_MS`.
pub fn counter_window_open(notified_at: Option<i64>, now: i64) -> bool {
    notified_at.is_some_and(|n| n > now - QUARANTINE_MS)
}

fn counter_state(counter_id: Option<&str>, action_id: Option<&str>, notified: Option<i64>, now: i64) -> &'static str {
    if counter_id.is_some() {
        "received"
    } else if action_id.is_some() && counter_window_open(notified, now) {
        "available"
    } else {
        "closed"
    }
}

/// `^(\d{1,16}):([0-9a-f]{32})$`
fn parse_after(after: Option<&str>) -> Option<(i64, String)> {
    let (n, id) = after?.split_once(':')?;
    if n.is_empty() || n.len() > 16 || !n.bytes().all(|b| b.is_ascii_digit()) || !super::sqlfrag::is_id(id) {
        return None;
    }
    Some((n.parse().ok()?, id.to_string()))
}

struct Row {
    id: String,
    kind: String,
    title: String,
    subject: String,
    version: i64,
    visibility: String,
    license: String,
    hide_key: i64,
    question_count: i64,
    committed_at: Option<i64>,
    removed_at: Option<i64>,
    removed_reason: Option<String>,
    purge_after: Option<i64>,
    current_publish_id: Option<String>,
    updated_at: i64,
    held: bool,
    needs_reupload: bool,
    key_leak: bool,
    import_count: i64,
    action_id: Option<String>,
    counter_notice_id: Option<String>,
    notified_at: Option<i64>,
}

pub fn mine(x: &PoolCtx, me: &str, after: Option<&str>) -> Res<Value> {
    let now = x.now;
    // Newest first, paged by a cursor (`after` = the last item's updated_at:id).
    let cur = parse_after(after);
    let mut st = x.conn.prepare(&format!(
        "SELECT i.id, i.kind, i.title, i.subject, i.version, i.visibility, i.license, i.hide_key, i.question_count, i.committed_at,
              i.removed_at, i.removed_reason, i.purge_after, i.current_publish_id, i.updated_at,
              EXISTS (SELECT 1 FROM item_holds h WHERE h.item_id = i.id) AS held,
              EXISTS (SELECT 1 FROM item_holds h WHERE h.item_id = i.id AND h.source = 'needs_reupload') AS needs_reupload,
              -- Held ONLY because teachers reported a visible hint of the hidden key.
              (EXISTS (SELECT 1 FROM item_holds h WHERE h.item_id = i.id AND h.source = 'key_leak') AND NOT EXISTS (SELECT 1 FROM item_holds h WHERE h.item_id = i.id AND h.source <> 'key_leak')) AS key_leak,
              (SELECT COUNT(DISTINCT im.user_id) FROM imports im WHERE im.item_id = i.id AND im.user_id <> i.author_id AND im.kind = 'import') AS import_count,
              (SELECT a.id FROM dmca_actions a WHERE a.item_id = i.id AND a.restored_at IS NULL AND a.purged_at IS NULL ORDER BY a.disabled_at DESC LIMIT 1) AS action_id,
              (SELECT a.counter_notice_id FROM dmca_actions a WHERE a.item_id = i.id AND a.restored_at IS NULL AND a.purged_at IS NULL ORDER BY a.disabled_at DESC LIMIT 1) AS counter_notice_id,
              (SELECT a.author_notified_at FROM dmca_actions a WHERE a.item_id = i.id AND a.restored_at IS NULL AND a.purged_at IS NULL ORDER BY a.disabled_at DESC LIMIT 1) AS notified_at
         FROM items i WHERE i.author_id = ?1 AND (i.purged_at IS NULL OR (i.removed_reason = 'privacy' AND i.removed_at > ?2))
          AND (?3 IS NULL OR i.updated_at < ?3 OR (i.updated_at = ?3 AND i.id < ?4))
        ORDER BY i.updated_at DESC, i.id DESC LIMIT {MINE_PAGE}"
    ))?;
    let rows: Vec<Row> = st
        .query_map(params![me, now - PRIVACY_APPEAL_MS, cur.as_ref().map(|c| c.0), cur.as_ref().map(|c| c.1.as_str()).unwrap_or("")], |r| {
            Ok(Row {
                id: r.get("id")?,
                kind: r.get("kind")?,
                title: r.get("title")?,
                subject: r.get("subject")?,
                version: r.get("version")?,
                visibility: r.get("visibility")?,
                license: r.get("license")?,
                hide_key: r.get("hide_key")?,
                question_count: r.get("question_count")?,
                committed_at: r.get("committed_at")?,
                removed_at: r.get("removed_at")?,
                removed_reason: r.get("removed_reason")?,
                purge_after: r.get("purge_after")?,
                current_publish_id: r.get("current_publish_id")?,
                updated_at: r.get("updated_at")?,
                held: r.get::<_, i64>("held")? != 0,
                needs_reupload: r.get::<_, i64>("needs_reupload")? != 0,
                key_leak: r.get::<_, i64>("key_leak")? != 0,
                import_count: r.get("import_count")?,
                action_id: r.get("action_id")?,
                counter_notice_id: r.get("counter_notice_id")?,
                notified_at: r.get("notified_at")?,
            })
        })?
        .collect::<Result<_, _>>()?;
    // A hold an ADMIN placed on an existing item (a confirmed student-data purge or a takedown holding its copies) says
    // what it is about and which questions; a hold placed by a publish's own content check keeps the neutral notice (no
    // block-membership oracle).
    let held_ids: Vec<&String> = rows.iter().filter(|r| r.held && r.removed_reason.is_none() && r.current_publish_id.is_some()).map(|r| &r.id).collect();
    let mut held: HashMap<String, Map<String, Value>> = HashMap::new();
    if !held_ids.is_empty() {
        // The positions come from the hold's snapshot at the admin action, and only while the item still has the content
        // the admin saw (no re-evaluation against later content).
        let mut hs = x.conn.prepare(
            "SELECT h.item_id, h.source, h.detail, i.current_publish_id AS pub FROM item_holds h JOIN items i ON i.id = h.item_id
          WHERE h.item_id IN (SELECT value FROM json_each(?1)) AND h.by IS NOT NULL AND (h.source = 'privacy' OR h.source LIKE 'dmca:%')",
        )?;
        let found: Vec<(String, String, Option<String>, Option<String>)> =
            hs.query_map([serde_json::to_string(&held_ids).unwrap()], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)))?.collect::<Result<_, _>>()?;
        for (item_id, source, detail, publ) in found {
            let priv_ = source == "privacy";
            let snap: Value = detail.as_deref().and_then(|d| serde_json::from_str(d).ok()).unwrap_or(Value::Null);
            let mut m = Map::new();
            m.insert("hold_class".into(), json!(if priv_ { "student_data" } else { "copyright" }));
            if snap.get("publish_id").and_then(|p| p.as_str()) == publ.as_deref() && snap.get("positions").is_some_and(|p| p.is_array()) {
                m.insert("positions".into(), snap["positions"].clone());
            }
            m.insert(
                "notice".into(),
                json!(if priv_ {
                    "An admin confirmed that a question this item shares with another item contains student data. Remove those questions and republish, or appeal if your copy is different."
                } else {
                    "A copyright takedown covers questions this item shares with the item taken down. Remove those questions and republish, or appeal if you have the right to share them."
                }),
            );
            held.insert(item_id, m);
        }
    }
    let items: Vec<Value> = rows
        .iter()
        .filter(|r| r.current_publish_id.is_some() || r.removed_reason.as_deref() == Some("privacy"))
        .map(|r| {
            let mut state = "live";
            let mut extra = Map::new();
            match r.removed_reason.as_deref() {
                Some("author_deleted") => {
                    state = "deleted";
                    if let Some(p) = r.purge_after.filter(|p| *p > now) {
                        extra.insert("undo_until".into(), json!(p));
                    }
                }
                Some("takedown") => {
                    // "taken down after a copyright notice", with the counter-notice form
                    state = "taken_down";
                    let cs = counter_state(r.counter_notice_id.as_deref(), r.action_id.as_deref(), r.notified_at, now);
                    extra.insert("counter_notice".into(), json!(cs));
                    if cs == "available" {
                        extra.insert("counter_notice_by".into(), json!(r.notified_at.unwrap() + QUARANTINE_MS));
                    }
                }
                Some("privacy") => {
                    // Purged at once (no quarantine for student data); shown to its author while it can be appealed.
                    state = "removed_privacy";
                    extra.insert("notice".into(), json!("An admin confirmed this item contained student data and removed it. If that is wrong, appeal within 14 days; you can then publish it again."));
                    extra.insert("appeal".into(), json!("available"));
                    extra.insert("appeal_until".into(), json!(r.removed_at.unwrap_or(now) + PRIVACY_APPEAL_MS));
                }
                Some(other) => {
                    state = "removed";
                    extra.insert("removed_reason".into(), json!(other));
                    if other == "admin" {
                        extra.insert("appeal".into(), json!("available"));
                    }
                }
                // A hold never says why or who reported: "held for review".
                None if r.needs_reupload => state = "needs_reupload", // an image was lost: upload it again and republish
                None if r.key_leak => {
                    state = "key_leak_review";
                    extra.insert("notice".into(), json!("Teachers who use this item reported that what students see gives the hidden answer key away (a hint in a question or its choices). Fix the wording and republish: the hold ends with the changed version."));
                }
                None if r.held => {
                    state = "held_for_review";
                    extra.insert("appeal".into(), json!("available"));
                    if let Some(n) = held.get(&r.id) {
                        extra.extend(n.clone());
                    }
                }
                None => {}
            }
            let mut v = json!({
                "id": r.id, "kind": r.kind, "title": r.title, "subject": r.subject, "version": r.version, "visibility": r.visibility,
                "license": r.license, "hide_key": r.hide_key == 1, "question_count": r.question_count, "committed_at": r.committed_at,
                "import_count": r.import_count, "state": state,
            });
            v.as_object_mut().unwrap().extend(extra);
            v
        })
        .collect();
    // A takedown of an item the author deleted (purged before the notice) is listed (first page) so they can counter it:
    // its id, the action and the window's end; no content.
    let mut ghost: Vec<Value> = Vec::new();
    if after.is_none() {
        let mut gs = x.conn.prepare(
            "SELECT a.item_id AS id, a.id AS action_id, a.author_notified_at AS n, a.counter_notice_id FROM dmca_actions a JOIN items i ON i.id = a.item_id
                WHERE i.author_id = ?1 AND i.purged_at IS NOT NULL AND i.purged_at < a.disabled_at AND a.restored_at IS NULL AND a.purged_at IS NULL
                ORDER BY a.disabled_at DESC LIMIT 100",
        )?;
        let found: Vec<(String, String, Option<i64>, Option<String>)> = gs.query_map([me], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)))?.collect::<Result<_, _>>()?;
        for (id, action_id, n, counter) in found {
            let mut v = json!({
                "id": id, "state": "takedown_pending", "action_id": action_id, "counter_notice_sent": counter.is_some(),
                // (the server says whether the form is offered, by the POST's own rule)
                "counter_notice": counter_state(counter.as_deref(), Some(&action_id), n, now),
            });
            if let Some(n) = n {
                v["counter_notice_by"] = json!(n + QUARANTINE_MS);
            }
            ghost.push(v);
        }
    }
    ghost.extend(items);
    let next = if rows.len() == MINE_PAGE { rows.last().map(|l| format!("{}:{}", l.updated_at, l.id)) } else { None };
    Ok(json!({
        "items": ghost,
        "next_cursor": next,
        "storage": { "used": usage(x, me)?, "limit": x.limit_of(me, "storage_bytes")? },
    }))
}

#[cfg(test)]
#[path = "mine_tests.rs"]
mod tests;
