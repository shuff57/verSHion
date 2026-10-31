//! Circles: create, list, read, edit, close and reopen (worker/src/pool/circles.ts, "POST /circles" through "reopen"). HTTP-free cores:
//! each takes the caller's id and the parsed body and returns the JSON the route sends (with its status where it is not 200).
use super::circlebase::*;
use super::circlesql::*;
use super::ctx::{AuditEntry, Fail, PoolCtx, PoolError, Res, audit};
use super::sqlfrag::{DAY_MS, alive_sql, new_id};
use crate::auth::validate::clean_text;
use rusqlite::params;
use serde_json::{Map, Value, json};

pub fn bad_field(field: &str, extra: &[(&str, Value)]) -> Fail {
    let mut e = PoolError::new(400, "invalid_request").with("field", field.into());
    for (k, v) in extra {
        e = e.with(k, v.clone());
    }
    e.into()
}
/// Optional cleaned text: absent, null or "" is None; anything else must clean (else 400 on `field`).
pub fn opt_text(v: Option<&Value>, max: usize, field: &str) -> Res<Option<String>> {
    match v {
        None | Some(Value::Null) => Ok(None),
        Some(Value::String(s)) if s.is_empty() => Ok(None),
        Some(v) => clean_text(v, max).filter(|s| !s.is_empty()).map(Some).ok_or_else(|| bad_field(field, &[("max", max.into())])),
    }
}
/// A JS integer within [lo, hi].
pub fn int_in(v: &Value, lo: i64, hi: i64, field: &str) -> Res<i64> {
    match v.as_f64().filter(|f| f.fract() == 0.0).map(|f| f as i64) {
        Some(n) if (lo..=hi).contains(&n) => Ok(n),
        _ => Err(bad_field(field, &[("min", lo.into()), ("max", hi.into())])),
    }
}
/// A user id (the 36-character form `[0-9a-f-]`).
pub fn user_id_of(v: Option<&Value>) -> Res<String> {
    match v.and_then(Value::as_str) {
        Some(s) if s.len() == 36 && s.bytes().all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b) || b == b'-') => Ok(s.into()),
        _ => Err(bad_field("user_id", &[])),
    }
}

/// POST /circles {name, description?}: 201 with the circle.
pub fn create_circle(x: &PoolCtx, me: &str, b: &Map<String, Value>) -> Res<(u16, Value)> {
    let name = b.get("name").and_then(|v| clean_text(v, CIRCLE_NAME_MAX)).filter(|s| !s.is_empty()).ok_or_else(|| bad_field("name", &[("max", CIRCLE_NAME_MAX.into())]))?;
    let description = opt_text(b.get("description"), CIRCLE_DESC_MAX, "description")?;
    x.require_daily(me, "circle_creates", 1)?;
    let caps = caps_of(x, me)?;
    let id = new_id();
    let seq = next_seq(x)?;
    let tx = x.conn.unchecked_transaction()?;
    let ins = tx.execute(
        &format!(
            "INSERT INTO circles (id, name, description, created_by, created_at, updated_at) SELECT ?1, ?2, ?3, ?4, ?5, ?5
          WHERE {} AND {} < ?6 AND {} < ?7 AND {} < ?8",
            alive_sql("?4"),
            owned_sql("?4"),
            memberships_sql("?4"),
            membership_rows_sql("?4")
        ),
        params![id, name, description, me, x.now, caps.owned, caps.memberships, caps.rows],
    )?;
    if ins == 1 {
        tx.execute("INSERT INTO circle_members (circle_id, user_id, role, joined_at, join_seq) VALUES (?1, ?2, 'owner', ?3, ?4)", params![id, me, x.now, seq])?;
        audit(&tx, &AuditEntry { actor_id: Some(me), actor_kind: "user", action: "circle.create", item_id: Some(&id), target_user_id: Some(me), ..Default::default() }, x.now, None)?;
    }
    tx.commit()?;
    if ins != 1 {
        x.throw_if_erased(me)?;
        x.give_daily(me, "circle_creates", 1)?; // (not a create)
        return Err(limit_error(x, me, caps, true)?.into());
    }
    Ok((
        201,
        json!({ "circle": { "id": id, "name": name, "description": description.unwrap_or_default(), "role": "owner", "created_at": x.now, "updated_at": x.now, "members": 1, "banks": 0,
            "closed_at": null, "close_reason": null, "closed_by": null, "undo_until": null } }),
    ))
}

/// GET /circles: the caller's circles, newest membership first; members and banks counted by index.
pub fn list_circles(x: &PoolCtx, me: &str) -> Res<Value> {
    let mut st = x.conn.prepare(&format!(
        "SELECT ci.id, COALESCE(ci.name, '') AS name, m.role, m.joined_at, ci.closed_at, ci.close_reason, {} AS closed_by, ci.undo_until, ci.updated_at,
              {} AS members,
              (SELECT COUNT(*) FROM circle_banks b WHERE b.circle_id = ci.id AND b.purged_at IS NULL AND b.removed_at IS NULL AND b.head_version > 0) AS banks,
              (SELECT t.expires_at FROM circle_transfers t WHERE t.circle_id = ci.id AND t.to_user = ?1 AND t.expires_at > ?2) AS ownership_offered_until
         FROM circle_members m JOIN circles ci ON ci.id = m.circle_id
        WHERE m.user_id = ?1 AND m.left_at IS NULL AND ci.purged_at IS NULL
        ORDER BY m.joined_at DESC, ci.id",
        closed_by_sql("ci.close_reason"),
        members_sql("ci.id")
    ))?;
    let rows: Vec<Value> = st
        .query_map(params![me, x.now], |r| {
            Ok(json!({ "id": r.get::<_, String>(0)?, "name": r.get::<_, String>(1)?, "role": r.get::<_, String>(2)?, "joined_at": r.get::<_, i64>(3)?, "closed_at": r.get::<_, Option<i64>>(4)?,
                "close_reason": r.get::<_, Option<String>>(5)?, "closed_by": r.get::<_, Option<String>>(6)?, "undo_until": r.get::<_, Option<i64>>(7)?, "updated_at": r.get::<_, i64>(8)?,
                "members": r.get::<_, i64>(9)?, "banks": r.get::<_, i64>(10)?, "ownership_offered_until": r.get::<_, Option<i64>>(11)? }))
        })?
        .collect::<Result<_, _>>()?;
    Ok(json!({ "circles": rows }))
}

/// GET /circles/:cid: the circle, its members, the banks the caller may see (masked where a hold or the author's standing hides them)
/// and the pending ownership offer (to the owner and to the member offered it).
pub fn get_circle(x: &PoolCtx, me: &str, cid: &str) -> Res<Value> {
    let m = circle_or_miss(x, cid, me)?;
    let members: Vec<Value> = x
        .conn
        .prepare(
            "SELECT m.user_id, m.role, m.joined_at, u.display_name, u.deleted_at FROM circle_members m LEFT JOIN users u ON u.id = m.user_id
        WHERE m.circle_id = ?1 AND m.left_at IS NULL ORDER BY m.join_seq",
        )?
        .query_map([cid], |r| {
            let uid: String = r.get(0)?;
            Ok(json!({ "is_me": uid == me, "user_id": uid, "display_name": name_of(r.get::<_, Option<String>>(3)?.as_deref(), r.get(4)?), "role": r.get::<_, String>(1)?, "joined_at": r.get::<_, i64>(2)? }))
        })?
        .collect::<Result<_, _>>()?;
    let owner = m.role == "owner";
    struct B {
        id: String,
        author: String,
        anonymized: bool,
        removed_at: Option<i64>,
        removed_reason: Option<String>,
        purge_after: Option<i64>,
        held: bool,
        blocking: bool,
        author_ok: bool,
        title: String,
        subject: String,
        course: String,
        grade: String,
        license: String,
        head_version: i64,
        question_count: i64,
        has_images: bool,
        hide_key: bool,
        created_at: i64,
        updated_at: i64,
        display: Option<String>,
        deleted_at: Option<i64>,
    }
    let banks: Vec<B> = x
        .conn
        .prepare(&format!(
            "SELECT b.id, b.title, b.subject, b.course, b.grade, b.license, b.head_version, b.question_count, b.has_images, b.created_at, b.updated_at, b.hide_key,
              b.author_id, b.anonymized, b.removed_at, b.removed_reason, b.purge_after, u.display_name, u.deleted_at,
              EXISTS (SELECT 1 FROM circle_holds h WHERE h.bank_id = b.id) AS held, {} AS blocking, {} AS author_ok
         FROM circle_banks b LEFT JOIN users u ON u.id = b.author_id
        WHERE b.circle_id = ?1 AND b.purged_at IS NULL AND b.head_version > 0 ORDER BY b.created_at, b.id",
            blocking_hold_sql("b.id"),
            author_ok_sql("b")
        ))?
        .query_map([cid], |r| {
            Ok(B {
                id: r.get(0)?,
                title: r.get(1)?,
                subject: r.get(2)?,
                course: r.get(3)?,
                grade: r.get(4)?,
                license: r.get(5)?,
                head_version: r.get(6)?,
                question_count: r.get(7)?,
                has_images: r.get::<_, i64>(8)? == 1,
                created_at: r.get(9)?,
                updated_at: r.get(10)?,
                hide_key: r.get::<_, i64>(11)? == 1,
                author: r.get(12)?,
                anonymized: r.get::<_, i64>(13)? == 1,
                removed_at: r.get(14)?,
                removed_reason: r.get(15)?,
                purge_after: r.get(16)?,
                display: r.get(17)?,
                deleted_at: r.get(18)?,
                held: r.get::<_, i64>(19)? == 1,
                blocking: r.get::<_, i64>(20)? == 1,
                author_ok: r.get::<_, i64>(21)? == 1,
            })
        })?
        .collect::<Result<_, _>>()?;
    let tr: Option<(String, i64, Option<String>, Option<i64>)> = x
        .conn
        .query_row(
            "SELECT t.to_user, t.expires_at, u.display_name, u.deleted_at FROM circle_transfers t LEFT JOIN users u ON u.id = t.to_user WHERE t.circle_id = ?1 AND t.expires_at > ?2",
            params![cid, x.now],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)),
        )
        .ok();
    let pending = tr.filter(|t| owner || t.0 == me).map(|t| json!({ "to_user_id": t.0, "to_name": name_of(t.2.as_deref(), t.3), "expires_at": t.1 }));
    let shown: Vec<Value> = banks
        .iter()
        .filter(|b| {
            let mine = b.author == me;
            // A REMOVED bank is listed to the owner and its author whatever the author's standing; a live one only when its author is in
            // good standing and it is not held (but to its author, and to the owner when only a student-data hold a send raised).
            if b.removed_at.is_some() {
                return mine || owner;
            }
            if !b.author_ok {
                return false;
            }
            !b.held || mine || (owner && !b.blocking)
        })
        .map(|b| {
            let mine = b.author == me;
            let masked = !mine && (!b.author_ok || (b.held && !(owner && !b.blocking)));
            let mut o = Map::new();
            let t = |s: &String| if masked { Value::Null } else { Value::from(s.clone()) };
            o.insert("id".into(), b.id.clone().into());
            o.insert("title".into(), t(&b.title));
            o.insert("subject".into(), t(&b.subject));
            o.insert("course".into(), t(&b.course));
            o.insert("grade".into(), t(&b.grade));
            o.insert("license".into(), b.license.clone().into());
            o.insert("author_name".into(), if masked { Value::Null } else if b.anonymized { DELETED_NAME.into() } else { name_of(b.display.as_deref(), b.deleted_at).into() });
            o.insert("is_author".into(), mine.into());
            o.insert("head_version".into(), b.head_version.into());
            o.insert("question_count".into(), b.question_count.into());
            o.insert("has_images".into(), b.has_images.into());
            o.insert("hide_key".into(), b.hide_key.into());
            o.insert("created_at".into(), b.created_at.into());
            o.insert("updated_at".into(), b.updated_at.into());
            if masked {
                o.insert("hidden".into(), true.into());
            } else if b.held {
                o.insert("held".into(), true.into());
            }
            if b.removed_at.is_some() {
                let undo = matches!(b.removed_reason.as_deref(), Some("author_removed") | Some("owner_removed")).then_some(b.purge_after).flatten();
                o.insert("removed".into(), json!({ "reason": b.removed_reason, "undo_until": undo }));
            }
            Value::Object(o)
        })
        .collect();
    Ok(json!({
        "circle": { "id": m.id, "name": text(Some(&m.name)), "description": text(m.description.as_deref()), "role": m.role, "joined_at": m.joined_at, "created_at": m.created_at,
            "updated_at": m.updated_at, "closed_at": m.closed_at, "close_reason": m.close_reason, "closed_by": closed_by(m.close_reason.as_deref()), "undo_until": m.undo_until },
        "members": members,
        "banks": shown,
        "pending_transfer": pending,
    }))
}

/// PATCH /circles/:cid {name?, description?} (owner).
pub fn patch_circle(x: &PoolCtx, me: &str, cid: &str, b: &Map<String, Value>) -> Res<Value> {
    let m = circle_or_miss(x, cid, me)?;
    require_open(&m)?;
    require_role(&m, &["owner"])?;
    let name = match b.get("name") {
        None => Some(m.name.clone()),
        Some(v) => clean_text(v, CIRCLE_NAME_MAX),
    }
    .filter(|s| !s.is_empty())
    .ok_or_else(|| bad_field("name", &[("max", CIRCLE_NAME_MAX.into())]))?;
    let description = match b.get("description") {
        None => m.description.clone(),
        Some(v) => opt_text(Some(v), CIRCLE_DESC_MAX, "description")?,
    };
    let tx = x.conn.unchecked_transaction()?;
    let upd = tx.execute(
        &format!("UPDATE circles SET name = ?3, description = ?4, updated_at = ?5 WHERE id = ?1 AND closed_at IS NULL AND purged_at IS NULL AND {} AND {}", active_sql("?1", "?2", &["owner"]), alive_sql("?2")),
        params![cid, me, name, description, x.now],
    )?;
    if upd == 1 {
        audit(&tx, &AuditEntry { actor_id: Some(me), actor_kind: "user", action: "circle.edit", item_id: Some(cid), target_user_id: Some(me), ..Default::default() }, x.now, None)?;
    }
    tx.commit()?;
    if upd != 1 {
        return Err(explain(x, cid, me, Some(&["owner"])));
    }
    Ok(json!({ "ok": true, "name": name, "description": description.unwrap_or_default() }))
}

/// DELETE /circles/:cid (owner): closed at once for everyone, 14 days to undo, then purged.
pub fn close_circle(x: &PoolCtx, me: &str, cid: &str) -> Res<Value> {
    let m = circle_or_miss(x, cid, me)?;
    require_role(&m, &["owner"])?;
    // Already closed by the owner: the same answer (the window is never extended).
    if m.closed_at.is_some() {
        if m.close_reason.as_deref() == Some("owner_closed") {
            return Ok(json!({ "ok": true, "undo_until": m.undo_until }));
        }
        return Err(closed_error(m.close_reason.as_deref(), m.undo_until).into());
    }
    let tx = x.conn.unchecked_transaction()?;
    let upd = tx.execute(
        &format!(
            "UPDATE circles SET closed_at = ?3, close_reason = 'owner_closed', undo_until = ?4, purge_after = ?4, updated_at = ?3
          WHERE id = ?1 AND closed_at IS NULL AND purged_at IS NULL AND {} AND {}",
            active_sql("?1", "?2", &["owner"]),
            alive_sql("?2")
        ),
        params![cid, me, x.now, x.now + CIRCLE_UNDO_MS],
    )?;
    if upd == 1 {
        audit(&tx, &AuditEntry { actor_id: Some(me), actor_kind: "user", action: "circle.close", item_id: Some(cid), target_user_id: Some(me), detail: Some(json!({ "reason": "owner_closed" })), ..Default::default() }, x.now, None)?;
    }
    tx.commit()?;
    if upd != 1 {
        return Err(explain(x, cid, me, Some(&["owner"])));
    }
    // An account that creates and closes many circles is flagged for the admin.
    let closes: i64 = x.conn.query_row("SELECT COUNT(*) FROM pool_audit WHERE actor_id = ?1 AND action = 'circle.close' AND at > ?2", params![me, x.now - 30 * DAY_MS], |r| r.get(0))?;
    if closes >= CIRCLE_CLOSES_FLAG {
        x.cap_alert(me, "circle_closes")?;
    }
    Ok(json!({ "ok": true, "undo_until": x.now + CIRCLE_UNDO_MS }))
}

/// POST /circles/:cid/reopen (owner): within the undo window.
pub fn reopen_circle(x: &PoolCtx, me: &str, cid: &str) -> Res<Value> {
    let m = circle_or_miss(x, cid, me)?;
    require_role(&m, &["owner"])?;
    if m.closed_at.is_none() {
        return Ok(json!({ "ok": true, "already": true }));
    }
    let tx = x.conn.unchecked_transaction()?;
    let upd = tx.execute(
        &format!(
            "UPDATE circles SET closed_at = NULL, close_reason = NULL, undo_until = NULL, purge_after = NULL, updated_at = ?3
          WHERE id = ?1 AND close_reason = 'owner_closed' AND undo_until > ?3 AND purged_at IS NULL AND {} AND {}",
            active_sql("?1", "?2", &["owner"]),
            alive_sql("?2")
        ),
        params![cid, me, x.now],
    )?;
    if upd == 1 {
        audit(&tx, &AuditEntry { actor_id: Some(me), actor_kind: "user", action: "circle.reopen", item_id: Some(cid), target_user_id: Some(me), ..Default::default() }, x.now, None)?;
    }
    tx.commit()?;
    if upd != 1 {
        x.throw_if_erased(me)?;
        return Err(PoolError::new(409, "circle_closed")
            .with("reason", m.close_reason.map(Value::from).unwrap_or(Value::Null))
            .with("undo_until", m.undo_until.map(Value::from).unwrap_or(Value::Null))
            .with("message", "This circle can no longer be reopened.".into())
            .into());
    }
    Ok(json!({ "ok": true }))
}

#[cfg(test)]
#[path = "circles_tests.rs"]
mod tests;
