//! Author actions, reports, appeals and the counter-notice (worker/src/pool/moderation.ts, the user-facing half): visibility, delete with a
//! 14-day undo, undelete, report, appeal, counter-notice, and `recheck_blocks`. HTTP-free cores returning `(status, body)`; the Worker's D1
//! batches are one transaction here, with later statements keyed on `changes()` of the one before, as there.
use super::bizdays::add_business_days;
use super::blobs::usage;
use super::ctx::{AuditEntry, Fail, PoolCtx, PoolError, PoolNotice, Res, audit, not_found};
use super::offer::offer_sync;
use super::served::{rightful_exempts, served_blobs_of, upstream_carried};
use super::sqlfrag::{DAY_MS, alive_sql, in_q_keys_of, is_id, new_id, shared_sql, usage_sql, visible_to};
use crate::auth::validate::clean_text;
use crate::blobs::BlobStore;
use rusqlite::{OptionalExtension, params};
use serde_json::{Map, Value, json};

pub use super::mine::QUARANTINE_MS;
pub const UNDO_MS: i64 = 14 * DAY_MS;
/// Privacy-blocked image bytes stay (served to nobody) this long before they are deleted.
pub const PRIVACY_BYTES_GRACE_MS: i64 = 14 * DAY_MS;
/// An open appeal stops the purge; the admin is reminded after this long ...
pub const APPEAL_REMINDER_MS: i64 = 7 * DAY_MS;
/// ... and an appeal still unanswered after this long RESTORES the item (never a silent purge).
pub const APPEAL_MAX_MS: i64 = 60 * DAY_MS;
pub const STRIKE_WINDOW_MS: i64 = 365 * DAY_MS;
pub const STRIKES_TO_SUSPEND: i64 = 3;
pub const INSTANT_HOLDS_PER_DAY: i64 = 3;
/// Accounts younger than this cannot hide an item instantly (their reports wait for the admin).
pub const NEW_ACCOUNT_MS: i64 = 7 * DAY_MS;
pub const THROTTLE_MS: i64 = 30 * DAY_MS;
pub const REPORT_REASONS: [&str; 6] = ["wrong_answer", "copyright", "student_data", "inappropriate", "other", "key_leak"];
/// key_leak reports an account may file a day, and holds one reporter may take part in a day.
pub const KEY_LEAK_REPORTS_PER_DAY: i64 = 5;
pub const KEY_LEAK_HOLDS_PER_DAY: i64 = 3;
/// How long a deleted held item's keys keep holding the same content.
pub const CARRY_KEYS_MS: i64 = 90 * DAY_MS;

/// The item has an appeal the admin has not decided.
pub fn open_appeal_sql(item: &str) -> String {
    format!("EXISTS (SELECT 1 FROM admin_flags fa WHERE fa.kind = 'appeal' AND fa.item_id = {item} AND fa.resolved_at IS NULL)")
}

/// SQL: open key_leak report `r` (reporter `ru`) on item `item`, publish `pub`, QUALIFIES at time `now`: a teacher in good standing, approved at
/// least 7 days, served that publish at least a day before reporting, no report of theirs dismissed on the item in 90 days.
pub fn key_leak_qualifies(item: &str, pub_: &str, now: &str) -> String {
    format!(
        "(r.item_id = {item} AND r.publish_id = {pub_} AND r.reason = 'key_leak' AND r.resolved_at IS NULL
    AND ru.trust IN ('teacher', 'admin') AND ru.deleted_at IS NULL AND ru.suspended_at IS NULL
    AND COALESCE((SELECT MAX(a.decided_at) FROM approval_requests a WHERE a.user_id = r.reporter_id AND a.decision = 'approved'), ru.created_at) <= {now} - {}
    AND EXISTS (SELECT 1 FROM imports im WHERE im.user_id = r.reporter_id AND im.item_id = {item} AND {pub_} IN (im.publish_id, im.real_publish_id) AND COALESCE(im.first_at, im.at) <= r.at - {DAY_MS})
    AND NOT EXISTS (SELECT 1 FROM reports d WHERE d.item_id = {item} AND d.reporter_id = r.reporter_id AND d.reason = 'key_leak' AND d.action = 'dismissed' AND d.resolved_at > {now} - {}))",
        7 * DAY_MS,
        90 * DAY_MS
    )
}

/// The day's miss lockout never stops an author managing their own items (`own_ok`).
fn id_or_miss(x: &PoolCtx, id: &str, me: &str, own_ok: bool) -> Res<()> {
    if own_ok && is_id(id) {
        let own: bool = x.conn.query_row("SELECT EXISTS (SELECT 1 FROM items WHERE id = ?1 AND author_id = ?2)", params![id, me], |r| r.get(0))?;
        if own {
            return Ok(());
        }
    }
    x.miss_guard(me, "item_miss")?;
    if !is_id(id) {
        x.count_miss(me, "item_miss")?;
        return Err(not_found().into());
    }
    Ok(())
}
fn miss(x: &PoolCtx, me: &str) -> Fail {
    match x.count_miss(me, "item_miss") {
        Err(e) => e,
        Ok(()) => not_found().into(),
    }
}
fn lit(s: &str) -> String {
    format!("'{}'", s.replace('\'', "''"))
}
fn item_audit(tx: &rusqlite::Connection, now: i64, me: &str, action: &str, id: &str, detail: Option<Value>, note: Option<&str>, cond: &str) -> Res<()> {
    audit(tx, &AuditEntry { actor_id: Some(me), actor_kind: "user", action, item_id: Some(id), target_user_id: Some(me), detail, note }, now, Some(cond))?;
    Ok(())
}

/// PATCH /items/:id {visibility}: one UPDATE; public is refused while the current licence is circle-only.
pub fn patch_item(x: &PoolCtx, me: &str, id: &str, body: &Map<String, Value>) -> Res<(u16, Value)> {
    id_or_miss(x, id, me, false)?;
    let vis = match body.get("visibility").and_then(Value::as_str) {
        Some(v @ ("public" | "unlisted")) => v,
        _ => return Err(PoolError::new(400, "invalid_request").with("field", json!("visibility")).into()),
    };
    let now = x.now;
    let tx = x.conn.unchecked_transaction()?;
    let n = tx.execute(
        "UPDATE items SET visibility = ?3, updated_at = ?4 WHERE id = ?1 AND author_id = ?2 AND removed_at IS NULL AND current_publish_id IS NOT NULL
           AND (?3 = 'unlisted' OR license <> 'circle-only')",
        params![id, me, vis, now],
    )?;
    item_audit(&tx, now, me, "item.visibility", id, Some(json!({ "visibility": vis })), None, "changes() = 1")?;
    tx.commit()?;
    if n == 1 {
        // A visibility change re-runs the block checks on what the item now serves.
        if vis == "public" {
            recheck_blocks(x, id)?;
        }
        // Unlisted offers nothing: its licence history closes; public re-opens it.
        offer_sync(x.conn, id, now, None)?;
        return Ok((200, json!({ "ok": true, "visibility": vis })));
    }
    let own: Option<(String, Option<i64>)> = x.conn.query_row("SELECT license, removed_at FROM items WHERE id = ?1 AND author_id = ?2", params![id, me], |r| Ok((r.get(0)?, r.get(1)?))).optional()?;
    if let Some((license, removed_at)) = &own {
        if removed_at.is_none() && license == "circle-only" {
            return Err(PoolError::new(422, "license_not_public").into());
        }
        if removed_at.is_some() {
            return Err(PoolError::new(409, "item_removed").into());
        }
    }
    Err(miss(x, me))
}

/// DELETE /items/:id: hidden at once, purged after 14 days unless undone.
pub fn delete_item(x: &PoolCtx, me: &str, id: &str) -> Res<(u16, Value)> {
    id_or_miss(x, id, me, true)?;
    let now = x.now;
    let tx = x.conn.unchecked_transaction()?;
    let n = tx.execute("UPDATE items SET removed_at = ?3, removed_reason = 'author_deleted', purge_after = ?4, updated_at = ?3 WHERE id = ?1 AND author_id = ?2 AND removed_at IS NULL", params![id, me, now, now + UNDO_MS])?;
    item_audit(&tx, now, me, "item.delete", id, None, None, "changes() = 1")?;
    // A held item's keys outlive its deletion and purge (hash-only), so the same content published again is held until the reports are resolved
    // (or CARRY_KEYS_MS passes); a key_leak hold too.
    tx.execute(
        "INSERT OR IGNORE INTO carried_keys (author_id, item_id, reason, key, until)
         SELECT ?2, ?1, h.reason, k.value, ?3 FROM item_holds h JOIN items i ON i.id = h.item_id
           JOIN item_questions iq ON iq.publish_id = i.current_publish_id, json_each(json_array(iq.match_hash, iq.bk_norm, iq.bk_full)) k
          WHERE h.item_id = ?1 AND i.author_id = ?2 AND h.source IN ('', 'manual', 'key_leak') AND k.value IS NOT NULL
            AND (h.by IS NOT NULL OR (SELECT COUNT(DISTINCT r.reporter_id) FROM reports r WHERE r.item_id = h.item_id AND r.resolved_at IS NULL) >= 2)",
        params![id, me, now + CARRY_KEYS_MS],
    )?;
    tx.commit()?;
    if n == 1 {
        return Ok((200, json!({ "ok": true, "undo_until": now + UNDO_MS })));
    }
    let own: Option<(Option<String>, Option<i64>)> =
        x.conn.query_row("SELECT removed_reason, purge_after FROM items WHERE id = ?1 AND author_id = ?2 AND purged_at IS NULL", params![id, me], |r| Ok((r.get(0)?, r.get(1)?))).optional()?;
    // Already deleted: the same answer (the window is never extended). Privacy and DMCA removals follow their own rules.
    match own {
        Some((Some(r), purge_after)) if r == "author_deleted" => Ok((200, json!({ "ok": true, "undo_until": purge_after }))),
        Some(_) => Err(PoolError::new(409, "under_moderation").into()),
        None => Err(miss(x, me)),
    }
}

/// POST /items/:id/undelete: only an author deletion, only within its 14 days; undoing cannot take an account past its quota or shared-question cap.
pub fn undelete_item(x: &PoolCtx, me: &str, id: &str) -> Res<(u16, Value)> {
    id_or_miss(x, id, me, true)?;
    let now = x.now;
    let quota = x.limit_of(me, "storage_bytes")?;
    let cap = x.limit_of(me, "shared_questions")?;
    let tx = x.conn.unchecked_transaction()?;
    let n = tx.execute(
        &format!(
            "UPDATE items SET removed_at = NULL, removed_reason = NULL, purge_after = NULL, updated_at = ?2
              WHERE id = ?3 AND author_id = ?1 AND removed_reason = 'author_deleted' AND purge_after > ?2 AND purged_at IS NULL
                AND {} <= ?4
                AND {} + question_count <= ?5",
            usage_sql(Some("?3")),
            shared_sql("?1", "?3")
        ),
        params![me, now, id, quota, cap],
    )?;
    item_audit(&tx, now, me, "item.undelete", id, None, None, "changes() = 1")?;
    tx.commit()?;
    if n == 1 {
        return Ok((200, json!({ "ok": true })));
    }
    let it: Option<i64> = x
        .conn
        .query_row("SELECT question_count FROM items WHERE id = ?1 AND author_id = ?2 AND removed_reason = 'author_deleted' AND purge_after > ?3 AND purged_at IS NULL", params![id, me, now], |r| r.get(0))
        .optional()?;
    let Some(question_count) = it else { return Err(miss(x, me)) };
    let (used, shared): (i64, i64) = x.conn.query_row(&format!("SELECT {}, {}", usage_sql(Some("?3")), shared_sql("?1", "?3")), params![me, now, id], |r| Ok((r.get(0)?, r.get(1)?)))?;
    if used > quota {
        let now_used = usage(x, me)?;
        return Err(PoolError::new(413, "quota_exceeded").with("used", json!(now_used)).with("needed", json!(used - now_used)).with("limit", json!(quota)).into());
    }
    Err(PoolError::new(422, "shared_questions_limit").with("used", json!(shared)).with("needed", json!(question_count)).with("limit", json!(cap)).into())
}

/// POST /items/:id/report: the reporter only ever hears "thanks".
pub fn report(x: &PoolCtx, store: &BlobStore, me: &str, id: &str, body: &Map<String, Value>) -> Res<(u16, Value)> {
    id_or_miss(x, id, me, false)?;
    let reason = match body.get("reason").and_then(Value::as_str) {
        Some(r) if REPORT_REASONS.contains(&r) => r,
        _ => return Err(PoolError::new(400, "invalid_request").with("field", json!("reason")).into()),
    };
    let note: Option<String> = match body.get("note") {
        None | Some(Value::Null) => None,
        Some(Value::String(s)) if s.is_empty() => None,
        Some(v) => Some(clean_text(v, 1000).ok_or_else(|| Fail::from(PoolError::new(400, "invalid_request").with("field", json!("note"))))?),
    };
    let it: Option<(String, String)> = x
        .conn
        .query_row(&format!("SELECT i.author_id, i.current_publish_id FROM items i JOIN users u ON u.id = i.author_id WHERE i.id = ?1 AND {}", visible_to("?2")), params![id, me], |r| Ok((r.get(0)?, r.get(1)?)))
        .optional()?;
    let Some((author_id, current_publish_id)) = it else { return Err(miss(x, me)) };
    if author_id == me {
        return Err(PoolError::new(400, "own_item").into());
    }
    x.require_daily(me, "reports", 1)?;
    let now = x.now;
    let rid = new_id();
    let threshold = x.limit_of("*", "report_threshold")?;
    let global = x.limit_of("*", "instant_holds_global")?;
    let tx = x.conn.unchecked_transaction()?;
    // The reporter must still stand, in this very statement (and in each one below).
    let recorded = tx.execute(&format!("INSERT OR IGNORE INTO reports (id, item_id, publish_id, reporter_id, reason, note, at) SELECT ?1, ?2, ?3, ?4, ?5, ?6, ?7 WHERE {}", alive_sql("?4")), params![rid, id, current_publish_id, me, reason, note, now])?;
    let held = if reason == "student_data" {
        // One report hides the item at once, but one reporter can do that to at most INSTANT_HOLDS_PER_DAY items a day, and not at all within
        // THROTTLE_MS of having a student_data report dismissed anywhere (the report is still recorded for the admin).
        tx.execute(
            &format!(
                "INSERT OR IGNORE INTO item_holds (item_id, reason, at, by) SELECT ?1, 'student_data', ?2, NULL
                  WHERE NOT EXISTS (SELECT 1 FROM reports WHERE reporter_id = ?3 AND reason = 'student_data' AND action = 'dismissed' AND resolved_at > ?2 - ?4)
                    AND (SELECT COUNT(*) FROM item_holds h WHERE h.reason = 'student_data' AND h.source = '' AND h.at >= ?5
                           AND EXISTS (SELECT 1 FROM reports r WHERE r.item_id = h.item_id AND r.reporter_id = ?3 AND r.reason = 'student_data' AND r.at = h.at)) < ?6
                    AND (SELECT COUNT(*) FROM item_holds h WHERE h.reason = 'student_data' AND h.source = '' AND h.at >= ?5) < ?7
                    AND ((SELECT COUNT(*) FROM item_holds h WHERE h.reason = 'student_data' AND h.source = '' AND h.at >= ?5) < ?7 / 2
                         OR (SELECT COUNT(DISTINCT reporter_id) FROM reports WHERE item_id = ?1 AND reason = 'student_data' AND resolved_at IS NULL) >= 2)
                    AND (SELECT created_at FROM users WHERE id = ?3) <= ?2 - ?8 AND {}
                    AND (EXISTS (SELECT 1 FROM imports im WHERE im.user_id = ?3 AND im.kind = 'import') OR EXISTS (SELECT 1 FROM items it WHERE it.author_id = ?3))",
                alive_sql("?3")
            ),
            params![id, now, me, THROTTLE_MS, now.div_euclid(DAY_MS) * DAY_MS, INSTANT_HOLDS_PER_DAY, global, NEW_ACCOUNT_MS],
        )?
    } else {
        // 3 distinct teacher reporters (open, non-privacy reasons) hide it pending review.
        tx.execute(
            &format!(
                "INSERT OR IGNORE INTO item_holds (item_id, reason, at, by)
                 SELECT ?1, 'reports', ?2, NULL WHERE (SELECT COUNT(DISTINCT r.reporter_id) FROM reports r JOIN users ru ON ru.id = r.reporter_id
                   WHERE r.item_id = ?1 AND r.resolved_at IS NULL AND r.reason NOT IN ('student_data', 'key_leak') AND ru.trust IN ('teacher', 'admin') AND ru.deleted_at IS NULL AND ru.suspended_at IS NULL
                     AND NOT EXISTS (SELECT 1 FROM reports d WHERE d.item_id = ?1 AND d.reporter_id = r.reporter_id AND d.action = 'dismissed' AND d.resolved_at > ?2 - ?4)) >= ?3
                   AND {}",
                alive_sql("?5")
            ),
            params![id, now, threshold, THROTTLE_MS, me],
        )?
    };
    if reason == "key_leak" {
        // A key_leak report is only for a HIDDEN-key publish, and counts only from a qualifying reporter: one served THAT publish (an import or
        // full read recorded at least a day before), approved at least 7 days, at most one such report per item per 30 days and 5 a day.
        let (hidden, served, seasoned, recent_item, today): (bool, bool, bool, i64, i64) = tx.query_row(
            &format!(
                "SELECT
                   EXISTS (SELECT 1 FROM item_publishes p WHERE p.publish_id = ?3 AND p.hide_key = 1),
                   EXISTS (SELECT 1 FROM imports im WHERE im.user_id = ?1 AND im.item_id = ?2 AND ?3 IN (im.publish_id, im.real_publish_id) AND COALESCE(im.first_at, im.at) <= ?4 - {DAY_MS}),
                   COALESCE((SELECT MAX(a.decided_at) FROM approval_requests a WHERE a.user_id = ?1 AND a.decision = 'approved'), (SELECT u.created_at FROM users u WHERE u.id = ?1)) <= ?4 - {},
                   (SELECT COUNT(*) FROM reports r WHERE r.reporter_id = ?1 AND r.item_id = ?2 AND r.reason = 'key_leak' AND r.at > ?4 - {}),
                   (SELECT COUNT(*) FROM reports r WHERE r.reporter_id = ?1 AND r.reason = 'key_leak' AND r.at > ?4 - {DAY_MS})",
                7 * DAY_MS,
                30 * DAY_MS
            ),
            params![me, id, current_publish_id, now],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?)),
        )?;
        if !hidden {
            return Err(PoolError::new(400, "invalid_request").with("field", json!("reason")).with("message", json!("This item shows its answer key: report a wrong answer instead.")).into());
        }
        if recent_item > 0 || today >= KEY_LEAK_REPORTS_PER_DAY {
            return Err(PoolError::new(429, "daily_limit").with("metric", json!("key_leak_reports")).into());
        }
        // A report that does not qualify is still recorded for the admin; it never counts toward a hold.
        if served && seasoned {
            tx.execute(
                &format!(
                    "INSERT OR IGNORE INTO item_holds (item_id, reason, at, by, source, detail)
                     SELECT ?1, 'reports', ?2, NULL, 'key_leak', json_object('publish_id', ?3)
                      WHERE (SELECT COUNT(DISTINCT r.reporter_id) FROM reports r JOIN users ru ON ru.id = r.reporter_id
                              WHERE {}
                                AND (SELECT COUNT(*) FROM item_holds kh JOIN reports kr ON kr.item_id = kh.item_id AND kr.reporter_id = r.reporter_id AND kr.reason = 'key_leak'
                                      WHERE kh.source = 'key_leak' AND kh.at > ?2 - {DAY_MS}) < {KEY_LEAK_HOLDS_PER_DAY}) >= 2
                        AND {}",
                    key_leak_qualifies("?1", "?3", "?2"),
                    alive_sql("?4")
                ),
                params![id, now, current_publish_id, me],
            )?;
        }
        tx.execute(
            &format!("INSERT OR IGNORE INTO admin_flags (id, kind, item_id, publish_id, detail, dedup, at) SELECT ?1, 'key_leak', ?2, ?3, ?4, ?5, ?6 WHERE {}", alive_sql("?7")),
            params![new_id(), id, current_publish_id, json!({ "report_id": rid, "qualifying": served && seasoned }).to_string(), format!("key_leak:{id}:{current_publish_id}"), now, me],
        )?;
    }
    item_audit_for_report(&tx, now, me, id, reason, &rid)?;
    tx.commit()?;
    // A report whose reporter was erased meanwhile wrote nothing: the answer an unauthenticated request gets.
    if recorded == 0 {
        x.throw_if_erased(me)?;
    }
    // The author is told (budgeted like every notice): what an importer reported, never who.
    if reason == "key_leak" && recorded == 1 {
        x.alert_budgeted(PoolNotice::KeyLeakReport { item_id: id.into(), author_id: author_id.clone() })?;
        // The admin queue entry carries what students are served (the stems and choices, at most 4 KB).
        let publish: Option<(Option<String>, String, bool)> = x
            .conn
            .query_row("SELECT stripped_key, full_key, hide_key FROM item_publishes WHERE publish_id = ?1", [&current_publish_id], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))
            .optional()?;
        if let Some((stripped_key, full_key, hide_key)) = publish {
            let key = if hide_key { stripped_key.unwrap_or(full_key) } else { full_key };
            if let Ok(Some(bytes)) = store.get(&key) {
                let served = served_text(&bytes);
                x.conn.execute("UPDATE admin_flags SET detail = json_set(COALESCE(detail, '{}'), '$.served', ?2) WHERE dedup = ?1", params![format!("key_leak:{id}:{current_publish_id}"), served])?;
            }
        }
    }
    // student_data: the admin is mailed immediately, outside the notice budget, through the pool's hourly alert budget (the hold and the flag do not).
    if reason == "student_data" && held == 1 {
        x.alert_budgeted(PoolNotice::StudentDataReport { item_id: id.into(), report_id: rid.clone() })?;
    }
    if reason == "student_data" && held == 0 && recorded == 1 {
        // The site-wide cap was reached (or the reporter is new / throttled): the admin is told once a day.
        let today = now.div_euclid(DAY_MS);
        let flagged = x.conn.execute(
            "INSERT OR IGNORE INTO admin_flags (id, kind, item_id, detail, dedup, at) SELECT ?1, 'instant_hold_cap', ?2, ?3, ?4, ?5
              WHERE (SELECT COUNT(*) FROM item_holds h WHERE h.reason = 'student_data' AND h.source = '' AND h.at >= ?6) >= ?7",
            params![new_id(), id, json!({ "report_id": rid }).to_string(), format!("instant_hold_cap:{today}"), now, today * DAY_MS, global],
        )?;
        if flagged == 1 {
            x.alert_budgeted(PoolNotice::StudentDataReport { item_id: id.into(), report_id: rid.clone() })?;
        }
    }
    Ok((202, json!({ "ok": true, "message": "Thanks, the admin will review it." })))
}
fn item_audit_for_report(tx: &rusqlite::Connection, now: i64, me: &str, id: &str, reason: &str, rid: &str) -> Res<()> {
    audit(tx, &AuditEntry { actor_id: Some(me), actor_kind: "user", action: "item.report", item_id: Some(id), target_user_id: None, detail: Some(json!({ "reason": reason, "report_id": rid })), note: None }, now, Some(&alive_sql(&lit(me))))?;
    Ok(())
}
/// The stems and choices of a served body, at most 4 KB, for the admin's key-leak entry.
fn served_text(bytes: &[u8]) -> String {
    let Ok(v) = serde_json::from_slice::<Value>(bytes) else { return String::new() };
    let Some(qs) = v.get("bank").and_then(|b| b.get("questions")).and_then(Value::as_array) else { return String::new() };
    let text: Vec<String> = qs
        .iter()
        .map(|q| {
            let mut lines = vec![q.get("stem").and_then(Value::as_str).unwrap_or("").to_string()];
            if let Some(kind) = q.get("kind").and_then(Value::as_object) {
                for k in kind.values() {
                    for a in k.get("answers").and_then(Value::as_array).into_iter().flatten() {
                        lines.push(format!("- {}", a.get("text").and_then(Value::as_str).unwrap_or("")));
                    }
                }
            }
            lines.join("\n")
        })
        .collect();
    text.join("\n\n").chars().take(4096).collect()
}

/// POST /items/:id/appeal {note}: the author of an item an admin removed or a hold hides asks for a review. It raises an admin flag (at most one a day
/// per item) and says nothing else.
pub fn appeal(x: &PoolCtx, me: &str, id: &str, body: &Map<String, Value>) -> Res<(u16, Value)> {
    id_or_miss(x, id, me, true)?;
    let Some(n) = body.get("note").and_then(|n| clean_text(n, 500)) else { return Err(PoolError::new(400, "invalid_request").with("field", json!("note")).into()) };
    // A privacy purge is appealable for PRIVACY_BYTES_GRACE_MS (while an unblock still restores the copies and images).
    type Row = (Option<String>, Option<i64>, Option<i64>, Option<i64>, bool);
    let it: Option<Row> = x
        .conn
        .query_row(
            &format!(
                "SELECT removed_reason, removed_at, appeal_decided_at, (SELECT MAX(h.at) FROM item_holds h WHERE h.item_id = items.id) AS held_at, {} AS open FROM items
                  WHERE id = ?1 AND author_id = ?2 AND (purged_at IS NULL OR (removed_reason = 'privacy' AND removed_at > ?3))",
                open_appeal_sql("items.id")
            ),
            params![id, me, x.now - PRIVACY_BYTES_GRACE_MS],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?)),
        )
        .optional()?;
    let Some((removed_reason, removed_at, appeal_decided_at, held_at, open)) = it else { return Err(miss(x, me)) };
    if removed_reason.as_deref() != Some("admin") && removed_reason.as_deref() != Some("privacy") && held_at.is_none() {
        return Err(PoolError::new(409, "nothing_to_appeal").into());
    }
    // One appeal per removal (or hold), never one per UTC day. Once the admin has decided on this removal, it cannot be appealed again; while one is
    // open, a repeat changes nothing.
    let instance = removed_at.or(held_at).unwrap_or(0);
    if appeal_decided_at.is_some_and(|d| d >= instance) {
        return Err(PoolError::new(409, "decided").with("message", json!("The admin has already decided on this. It cannot be appealed again.")).into());
    }
    if open {
        return Ok((202, json!({ "ok": true, "message": "Your appeal is already with the admin." })));
    }
    x.require_daily(me, "appeals", 1)?;
    let now = x.now;
    // The flag is written only while the appellant stands (same statement; the audit follows it by changes()); an erased account's late appeal would be
    // an OPEN appeal that blocks the purge of its own removed item, so it is refused like an unauthenticated request.
    let tx = x.conn.unchecked_transaction()?;
    let wrote = tx.execute(
        &format!("INSERT OR IGNORE INTO admin_flags (id, kind, item_id, user_id, detail, dedup, at) SELECT ?1, 'appeal', ?2, ?3, ?4, ?5, ?6 WHERE {}", alive_sql("?3")),
        params![new_id(), id, me, json!({ "note": n, "removed_reason": removed_reason, "held": held_at.is_some() }).to_string(), format!("appeal:{id}:{instance}"), now],
    )?;
    item_audit(&tx, now, me, "item.appeal", id, None, Some(&n), "changes() = 1")?;
    tx.commit()?;
    if wrote == 0 {
        x.throw_if_erased(me)?;
    }
    Ok((202, json!({ "ok": true, "message": "The admin will review your item." })))
}

/// POST /items/:id/counter-notice: the author of a taken-down item. Incomplete notices are refused.
pub fn counter_notice(x: &PoolCtx, me: &str, id: &str, body: &Map<String, Value>) -> Res<(u16, Value)> {
    // The day's miss lockout never blocks an author's counter-notice on their own item.
    id_or_miss(x, id, me, true)?;
    let now = x.now;
    let action: Option<(String, String)> = x
        .conn
        .query_row(
            "SELECT a.id, a.notice_id FROM dmca_actions a JOIN items i ON i.id = a.item_id
              WHERE a.item_id = ?1 AND a.author_id = ?2 AND i.author_id = ?2 AND i.removed_reason = 'takedown'
                AND a.restored_at IS NULL AND a.purged_at IS NULL AND a.counter_notice_id IS NULL
                AND a.author_notified_at > ?3 - ?4
              ORDER BY a.disabled_at DESC LIMIT 1",
            params![id, me, now, QUARANTINE_MS],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .optional()?;
    let window_closed = || -> Fail { PoolError::new(409, "window_closed").with("message", json!("The 30-day counter-notice window for this takedown has ended.")).into() };
    let Some((action_id, notice_id)) = action else {
        // The author's own taken-down item whose 30-day window has ended: say so, not a 404.
        let late: bool = x.conn.query_row(
            "SELECT EXISTS (SELECT 1 FROM dmca_actions a JOIN items i ON i.id = a.item_id WHERE a.item_id = ?1 AND a.author_id = ?2 AND i.author_id = ?2 AND a.counter_notice_id IS NULL AND a.restored_at IS NULL)",
            params![id, me],
            |r| r.get(0),
        )?;
        return Err(if late { window_closed() } else { miss(x, me) });
    };
    let mut missing: Vec<&str> = Vec::new();
    let mut text = |k: &'static str, max: usize| -> String {
        match body.get(k).and_then(|v| clean_text(v, max)) {
            Some(v) => v,
            None => {
                missing.push(k);
                String::new()
            }
        }
    };
    let signature = text("signature", 200);
    let material = text("material_desc", 2000);
    let name = text("name", 200);
    let address = text("address", 500);
    let phone = text("phone", 50);
    for k in ["perjury_stmt", "consent_jurisdiction", "accept_service"] {
        if body.get(k) != Some(&Value::Bool(true)) {
            missing.push(k);
        }
    }
    if !missing.is_empty() {
        return Err(PoolError::new(400, "counter_notice_incomplete").with("missing", json!(missing)).into());
    }
    let cid = new_id();
    let not_before = add_business_days(now, 10) - DAY_MS + 1; // the start of the 10th business day after receipt
    let by = add_business_days(now, 14);
    // The action takes the counter-notice only if it is still open in the same statement (not upheld, put back or past its window): an uphold that
    // committed meanwhile wins, and the author is told the window closed, never a 202 for an action that ends upheld.
    let tx = x.conn.unchecked_transaction()?;
    let took = tx.execute(
        &format!(
            "UPDATE dmca_actions SET counter_notice_id = ?2, restore_not_before = ?3, restore_by = ?4
              WHERE id = ?1 AND counter_notice_id IS NULL AND purged_at IS NULL AND restored_at IS NULL AND author_notified_at > ?5 - ?6 AND {}",
            alive_sql("?7")
        ),
        params![action_id, cid, not_before, by, now, QUARANTINE_MS, me],
    )?;
    tx.execute(
        "INSERT INTO dmca_counter_notices (id, action_id, received_at, signature, material_desc, perjury_stmt, name, address, phone, consent_jurisdiction, accept_service, forwarded_at)
         SELECT ?1, ?2, ?3, ?4, ?5, 1, ?6, ?7, ?8, 1, 1, ?3 WHERE changes() = 1 AND NOT EXISTS (SELECT 1 FROM dmca_counter_notices WHERE action_id = ?2)",
        params![cid, action_id, now, signature, material, name, address, phone],
    )?;
    item_audit(&tx, now, me, "dmca.counter_notice", id, Some(json!({ "action_id": action_id })), None, "changes() = 1")?;
    // A counter-notice is an admin task (forward it to the complainant; restore after 10 business days).
    tx.execute(
        "INSERT OR IGNORE INTO admin_flags (id, kind, item_id, user_id, detail, dedup, at) SELECT ?1, 'counter_notice', ?2, ?3, ?4, ?5, ?6 WHERE changes() = 1",
        params![new_id(), id, me, json!({ "action_id": action_id, "restore_not_before": not_before }).to_string(), format!("counter_notice:{action_id}"), now],
    )?;
    tx.commit()?;
    if took != 1 {
        x.throw_if_erased(me)?;
        return Err(window_closed());
    }
    let contact: Option<String> = x.conn.query_row("SELECT complainant_contact FROM dmca_notices WHERE id = ?1", [&notice_id], |r| r.get(0)).optional()?;
    // Forwarded to the complainant at once, with the restore date.
    if let Some(complainant_contact) = contact {
        x.send(PoolNotice::CounterNotice { action_id: action_id.clone(), complainant_contact, restore_not_before: not_before });
    }
    Ok((202, json!({ "ok": true, "restore_not_before": not_before, "restore_by": by })))
}

/// The privacy and copyright block checks a publish runs, on an item's CURRENT publish now (a visibility change to public). The same exemptions: an
/// admin's release of this item, the action's exempt authors and the rightful author (and those served the rightful item).
pub fn recheck_blocks(x: &PoolCtx, item_id: &str) -> Res<()> {
    let it: Option<(String, String)> = x.conn.query_row("SELECT author_id, current_publish_id FROM items WHERE id = ?1 AND current_publish_id IS NOT NULL", [item_id], |r| Ok((r.get(0)?, r.get(1)?))).optional()?;
    let Some((author_id, publish)) = it else { return Ok(()) };
    let now = x.now;
    let tx = x.conn.unchecked_transaction()?;
    tx.execute(
        &format!(
            "INSERT OR IGNORE INTO item_holds (item_id, reason, at, by, source)
             SELECT ?1, 'student_data', ?3, NULL, 'privacy' WHERE EXISTS (SELECT 1 FROM privacy_blocks pb WHERE
               ((pb.kind = 'match' AND {}) OR (pb.kind = 'blob' AND pb.hash IN {}))
               AND NOT EXISTS (SELECT 1 FROM hold_releases hr, json_each(hr.keys_json) r WHERE hr.item_id = ?1 AND hr.source = 'privacy' AND r.value = pb.hash AND hr.at > ?3 - {} AND pb.at <= hr.at))",
            in_q_keys_of("pb.hash", "?2"),
            served_blobs_of("?2"),
            90 * DAY_MS
        ),
        params![item_id, publish, now],
    )?;
    tx.execute(
        &format!(
            "INSERT OR IGNORE INTO item_holds (item_id, reason, at, by, source)
             SELECT DISTINCT ?1, 'blocklist', ?3, NULL, 'dmca:' || bh.action_id FROM blocked_hashes bh
              WHERE bh.lifted_at IS NULL AND (bh.expires_at IS NULL OR bh.expires_at > ?3) AND bh.action_id IS NOT NULL
                AND ((bh.kind = 'match' AND {}) OR (bh.kind = 'blob' AND bh.hash IN {}))
                AND NOT EXISTS (SELECT 1 FROM dmca_exempt e WHERE e.action_id = bh.action_id AND e.user_id = ?4)
                AND NOT EXISTS (SELECT 1 FROM dmca_actions a WHERE a.id = bh.action_id AND (a.item_id = ?1 OR (a.rightful_item_id IS NOT NULL AND {})))
                AND NOT {}
                AND NOT EXISTS (SELECT 1 FROM hold_releases hr, json_each(hr.keys_json) r WHERE hr.item_id = ?1 AND hr.source = 'dmca:' || bh.action_id AND r.value = bh.hash AND hr.at > ?3 - {} AND bh.at <= hr.at)",
            in_q_keys_of("bh.hash", "?2"),
            served_blobs_of("?2"),
            rightful_exempts("a.rightful_item_id", "bh", "?4", "?2"),
            upstream_carried("bh.action_id", "?1", "bh.hash"),
            90 * DAY_MS
        ),
        params![item_id, publish, now, author_id],
    )?;
    tx.commit()?;
    Ok(())
}

#[cfg(test)]
#[path = "moderation_tests.rs"]
mod tests;
