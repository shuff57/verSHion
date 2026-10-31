//! Circle invite codes and joining (worker/src/pool/circles.ts "invite codes" and "join"). HTTP-free cores.
use super::circlebase::*;
use super::circles::{bad_field, int_in, opt_text};
use super::circlesql::*;
use super::ctx::{AuditEntry, Fail, PoolCtx, PoolError, Res, audit, not_found};
use super::sqlfrag::{DAY_MS, alive_sql, is_id, new_id};
use rusqlite::{OptionalExtension, params};
use serde_json::{Map, Value, json};

/// POST /circles/:cid/invites {role, max_uses?, expires_in_days?, note?} (owner): the code is in this answer only.
pub fn create_invite(x: &PoolCtx, me: &str, cid: &str, b: &Map<String, Value>) -> Res<(u16, Value)> {
    let m = circle_or_miss(x, cid, me)?;
    require_open(&m)?;
    require_role(&m, &["owner"])?;
    let role = match b.get("role").and_then(Value::as_str) {
        Some(r @ ("editor" | "viewer")) => r,
        _ => return Err(bad_field("role", &[])),
    };
    let opt = |k: &str| b.get(k).filter(|v| !v.is_null());
    let max_uses = match opt("max_uses") {
        None => 1,
        Some(v) => int_in(v, 1, CIRCLE_CODE_MAX_USES, "max_uses")?,
    };
    let days = match opt("expires_in_days") {
        None => CIRCLE_CODE_DAYS,
        Some(v) => int_in(v, 1, CIRCLE_CODE_MAX_DAYS, "expires_in_days")?,
    };
    let note = opt_text(b.get("note"), CIRCLE_NOTE_MAX, "note")?;
    x.require_daily(me, "circle_invites", 1)?;
    let cap = x.limit_of("*", "circle_open_invites")?;
    let code = new_circle_code();
    let id = new_id();
    let expires_at = x.now + days * DAY_MS;
    let tx = x.conn.unchecked_transaction()?;
    let ins = tx.execute(
        &format!(
            "INSERT INTO circle_invites (id, circle_id, code_hash, role, created_by, created_at, max_uses, uses, expires_at, note)
         SELECT ?1, ?2, ?3, ?4, ?5, ?6, ?7, 0, ?8, ?9 WHERE {} AND {} AND {} AND {} < ?10",
            open_sql("?2"),
            active_sql("?2", "?5", &["owner"]),
            alive_sql("?5"),
            open_invites_sql("?2", "?6", false)
        ),
        params![id, cid, circle_code_hash(&normalise_circle_code(&json!(code)).unwrap()), role, me, x.now, max_uses, expires_at, note, cap],
    )?;
    if ins == 1 {
        // (the code itself is never written anywhere: neither the row nor the audit has it)
        audit(
            &tx,
            &AuditEntry { actor_id: Some(me), actor_kind: "user", action: "circle.invite_create", item_id: Some(cid), target_user_id: Some(me), detail: Some(json!({ "invite_id": id, "role": role, "max_uses": max_uses, "expires_at": expires_at })), ..Default::default() },
            x.now,
            None,
        )?;
    }
    tx.commit()?;
    if ins != 1 {
        x.throw_if_erased(me)?;
        x.give_daily(me, "circle_invites", 1)?;
        let n: i64 = x.conn.query_row(&format!("SELECT {}", open_invites_sql("?1", "?2", false)), params![cid, x.now], |r| r.get(0))?;
        if n >= cap {
            return Err(PoolError::new(422, "circles_limit")
                .with("metric", "circle_open_invites".into())
                .with("used", n.into())
                .with("limit", cap.into())
                .with("message", format!("This circle has {cap} open codes. Withdraw one first.").into())
                .into());
        }
        return Err(explain(x, cid, me, Some(&["owner"])));
    }
    Ok((201, json!({ "invite_id": id, "code": code, "role": role, "max_uses": max_uses, "expires_at": expires_at, "note": text(note.as_deref()), "shown_once": true })))
}

/// GET /circles/:cid/invites?after= (owner): newest first, paged by `after` (= the last one's created_at:id).
pub fn list_invites(x: &PoolCtx, me: &str, cid: &str, after: Option<&str>) -> Res<Value> {
    let m = circle_or_miss(x, cid, me)?;
    require_role(&m, &["owner"])?;
    let cur = after.and_then(|a| {
        let (t, i) = a.split_once(':')?;
        (!t.is_empty() && t.len() <= 16 && t.bytes().all(|b| b.is_ascii_digit()) && is_id(i)).then(|| t.parse::<i64>().ok().map(|n| (n, i.to_string()))).flatten()
    });
    struct Row {
        v: Map<String, Value>,
        created_at: i64,
        id: String,
    }
    let mut rows: Vec<Row> = x
        .conn
        .prepare(
            "SELECT i.id AS invite_id, i.kind, COALESCE(i.email_label, '') AS address_label, i.role, i.created_at, i.max_uses, i.uses, i.expires_at, i.revoked_at, COALESCE(i.note, '') AS note, u.display_name AS cn, u.deleted_at AS cd,
              CASE WHEN i.revoked_at IS NOT NULL THEN 'revoked' WHEN i.uses >= i.max_uses THEN 'used_up' WHEN i.expires_at <= ?2 THEN 'expired' ELSE 'active' END AS state, i.created_by
         FROM circle_invites i LEFT JOIN users u ON u.id = i.created_by
        WHERE i.circle_id = ?1 AND (?3 IS NULL OR i.created_at < ?3 OR (i.created_at = ?3 AND i.id < ?4)) ORDER BY i.created_at DESC, i.id DESC LIMIT ?5",
        )?
        .query_map(params![cid, x.now, cur.as_ref().map(|c| c.0), cur.as_ref().map(|c| c.1.clone()).unwrap_or_default(), (CIRCLE_INVITE_PAGE + 1) as i64], |r| {
            let mut v = Map::new();
            let id: String = r.get(0)?;
            let created_at: i64 = r.get(4)?;
            v.insert("invite_id".into(), id.clone().into());
            v.insert("kind".into(), r.get::<_, String>(1)?.into());
            v.insert("address_label".into(), r.get::<_, String>(2)?.into());
            v.insert("role".into(), r.get::<_, String>(3)?.into());
            v.insert("created_at".into(), created_at.into());
            v.insert("max_uses".into(), r.get::<_, i64>(5)?.into());
            v.insert("uses".into(), r.get::<_, i64>(6)?.into());
            v.insert("expires_at".into(), r.get::<_, i64>(7)?.into());
            v.insert("revoked_at".into(), r.get::<_, Option<i64>>(8)?.into());
            v.insert("note".into(), r.get::<_, String>(9)?.into());
            v.insert("state".into(), r.get::<_, String>(12)?.into());
            v.insert("created_by".into(), r.get::<_, String>(13)?.into());
            // (who issued each code; a former owner's codes are revoked when the role passes on)
            v.insert("created_by_name".into(), name_of(r.get::<_, Option<String>>(10)?.as_deref(), r.get(11)?).into());
            Ok(Row { v, created_at, id })
        })?
        .collect::<Result<_, _>>()?;
    let more = rows.len() > CIRCLE_INVITE_PAGE;
    rows.truncate(CIRCLE_INVITE_PAGE);
    let next = if more { rows.last().map(|l| format!("{}:{}", l.created_at, l.id)) } else { None };
    Ok(json!({ "invites": rows.into_iter().map(|r| Value::Object(r.v)).collect::<Vec<_>>(), "next_cursor": next }))
}

/// POST /circles/:cid/invites/revoke {invite_id} (owner): no further joins; members who joined stay.
pub fn revoke_invite(x: &PoolCtx, me: &str, cid: &str, b: &Map<String, Value>) -> Res<Value> {
    let m = circle_or_miss(x, cid, me)?;
    require_role(&m, &["owner"])?;
    let iid = match b.get("invite_id").and_then(Value::as_str) {
        Some(i) if is_id(i) => i.to_string(),
        _ => return Err(bad_field("invite_id", &[])),
    };
    let there: Option<Option<i64>> = x.conn.query_row("SELECT revoked_at FROM circle_invites WHERE id = ?1 AND circle_id = ?2", params![iid, cid], |r| r.get(0)).optional()?;
    let Some(there) = there else { return Err(not_found().into()) };
    let already = |at: i64| -> Fail { PoolError::new(409, "already_revoked").with("revoked_at", at.into()).into() };
    if let Some(at) = there {
        return Err(already(at));
    }
    let tx = x.conn.unchecked_transaction()?;
    let upd = tx.execute(
        &format!("UPDATE circle_invites SET revoked_at = ?3, revoked_by = ?4 WHERE id = ?1 AND circle_id = ?2 AND revoked_at IS NULL AND {} AND {}", active_sql("?2", "?4", &["owner"]), alive_sql("?4")),
        params![iid, cid, x.now, me],
    )?;
    if upd == 1 {
        audit(&tx, &AuditEntry { actor_id: Some(me), actor_kind: "user", action: "circle.invite_revoke", item_id: Some(cid), target_user_id: Some(me), detail: Some(json!({ "invite_id": iid })), ..Default::default() }, x.now, None)?;
    }
    tx.commit()?;
    if upd != 1 {
        x.throw_if_erased(me)?;
        let again: Option<Option<i64>> = x.conn.query_row("SELECT revoked_at FROM circle_invites WHERE id = ?1", [&iid], |r| r.get(0)).optional()?;
        if let Some(Some(at)) = again {
            return Err(already(at));
        }
        return Err(explain(x, cid, me, Some(&["owner"])));
    }
    Ok(json!({ "ok": true, "invite_id": iid }))
}

fn invalid_code() -> PoolError {
    PoolError::new(422, "circle_invite_invalid").with("message", "This circle code is not valid: it may have expired, been used up or been withdrawn, or the circle closed. Ask the circle's owner for a new one.".into())
}

struct CodeRow {
    id: String,
    circle_id: String,
    /// The role this caller would (re)join with.
    role: String,
    code_hash: String,
    invite_role: String,
    created_at: i64,
    member: bool,
}
/// The code behind `raw`: None for every refusal alike (unknown, expired, used up, revoked, its circle closed); an active member of
/// the code's circle is told so whatever the code's state. One failed try is charged first; the caller refunds it for a non-failure.
fn lookup_code(x: &PoolCtx, me: &str, raw: Option<&Value>) -> Res<Option<CodeRow>> {
    let norm = raw.and_then(normalise_circle_code).ok_or_else(|| bad_field("code", &[]))?;
    x.require_daily(me, "circle_join_tries", 1)?;
    let row = x
        .conn
        .query_row(
            &format!(
                "SELECT i.id, i.circle_id, {} AS role, i.code_hash, i.role AS invite_role, i.created_at,
              (i.revoked_at IS NULL AND i.expires_at > ?2 AND i.uses < i.max_uses AND ci.closed_at IS NULL AND ci.purged_at IS NULL AND {}) AS valid,
              {} AS member
         FROM circle_invites i JOIN circles ci ON ci.id = i.circle_id WHERE i.code_hash = ?1 AND ci.purged_at IS NULL",
                join_role_sql("i", "?3"),
                code_usable_sql("i", "?3"),
                active_sql("i.circle_id", "?3", &[])
            ),
            params![circle_code_hash(&norm), x.now, me],
            |r| Ok((CodeRow { id: r.get(0)?, circle_id: r.get(1)?, role: r.get(2)?, code_hash: r.get(3)?, invite_role: r.get(4)?, created_at: r.get(5)?, member: r.get::<_, i64>(7)? == 1 }, r.get::<_, i64>(6)? == 1)),
        )
        .optional()?;
    Ok(row.filter(|(c, valid)| *valid || c.member).map(|(c, _)| c))
}

/// POST /circles/join/preview {code}: what joining would mean. Uses nothing; a bad code counts as a failed try.
pub fn join_preview(x: &PoolCtx, me: &str, b: &Map<String, Value>) -> Res<Value> {
    let code = lookup_code(x, me, b.get("code"))?.ok_or_else(invalid_code)?;
    x.give_daily(me, "circle_join_tries", 1)?;
    let r: Option<(String, i64, Option<String>, Option<i64>)> = x
        .conn
        .query_row(
            &format!(
                "SELECT ci.name, {} AS members, u.display_name, u.deleted_at
         FROM circles ci LEFT JOIN circle_members o ON o.circle_id = ci.id AND o.role = 'owner' AND o.left_at IS NULL LEFT JOIN users u ON u.id = o.user_id
        WHERE ci.id = ?1",
                members_sql("ci.id")
            ),
            [&code.circle_id],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)),
        )
        .optional()?;
    let (name, members, dn, del) = r.ok_or_else(invalid_code)?;
    Ok(json!({ "circle": { "id": code.circle_id, "name": name, "owner_name": name_of(dn.as_deref(), del), "members": members }, "role": code.role, "already_member": code.member }))
}

/// POST /circles/join {code}: joins with the code's role. Every refused code is the same 422.
pub fn join_circle(x: &PoolCtx, me: &str, b: &Map<String, Value>) -> Res<Value> {
    let code = lookup_code(x, me, b.get("code"))?.ok_or_else(invalid_code)?;
    let refund = || x.give_daily(me, "circle_join_tries", 1);
    let already = || -> rusqlite::Result<Option<String>> { x.conn.query_row("SELECT role FROM circle_members WHERE circle_id = ?1 AND user_id = ?2 AND left_at IS NULL", params![code.circle_id, me], |r| r.get(0)).optional() };
    let was = if code.member { already()? } else { None };
    if let Some(role) = was {
        refund()?;
        return Ok(json!({ "ok": true, "already": true, "circle_id": code.circle_id, "role": role }));
    }
    let caps = caps_of(x, me)?;
    let members_cap = x.limit_of("*", "circle_members")?;
    let seq = next_seq(x)?;
    // The use is taken only while the code is good, the circle open and not full, the caller under the membership cap, not already a
    // member and not erased; the membership (a new row, or the old row of a former member reused) and its audit follow it. And only
    // while the invite still has the code and the role that were read.
    let tx = x.conn.unchecked_transaction()?;
    let used = tx.execute(
        &format!(
            "UPDATE circle_invites SET uses = uses + 1
          WHERE id = ?1 AND code_hash = ?8 AND role = ?9 AND revoked_at IS NULL AND expires_at > ?3 AND uses < max_uses AND {} AND {}
            AND NOT {} AND {} < ?5 AND {} < ?6
            AND ({} < ?7 OR EXISTS (SELECT 1 FROM circle_members fr WHERE fr.circle_id = ?2 AND fr.user_id = ?4))
            AND {}",
            open_sql("?2"),
            alive_sql("?4"),
            active_sql("?2", "?4", &[]),
            members_sql("?2"),
            memberships_sql("?4"),
            membership_rows_sql("?4"),
            code_usable_sql("circle_invites", "?4")
        ),
        params![code.id, code.circle_id, x.now, me, members_cap, caps.memberships, caps.rows, code.code_hash, code.invite_role],
    )?;
    if used == 1 {
        // A former member's row is reused; the role is the code's, capped at the role held when leaving if the code was issued before that.
        tx.execute(
            "INSERT INTO circle_members (circle_id, user_id, role, joined_at, join_seq, invite_id) VALUES (?1, ?2, ?3, ?4, ?5, ?6)
         ON CONFLICT (circle_id, user_id) DO UPDATE SET
           role = CASE WHEN circle_members.left_at IS NOT NULL AND circle_members.left_at >= ?7 AND circle_members.role = 'viewer' THEN 'viewer' ELSE excluded.role END,
           joined_at = excluded.joined_at, join_seq = excluded.join_seq, left_at = NULL, left_reason = NULL, invite_id = excluded.invite_id",
            params![code.circle_id, me, code.role, x.now, seq, code.id, code.created_at],
        )?;
        audit(&tx, &AuditEntry { actor_id: Some(me), actor_kind: "user", action: "circle.join", item_id: Some(&code.circle_id), target_user_id: Some(me), detail: Some(json!({ "role": code.role, "invite_id": code.id })), ..Default::default() }, x.now, None)?;
    }
    tx.commit()?;
    if used == 1 {
        refund()?;
        return Ok(json!({ "ok": true, "circle_id": code.circle_id, "role": code.role }));
    }
    // Nothing taken: a concurrent join of this account won, the account was erased, a cap, or the code is spent.
    if let Some(role) = already()? {
        refund()?;
        return Ok(json!({ "ok": true, "already": true, "circle_id": code.circle_id, "role": role }));
    }
    x.throw_if_erased(me)?;
    let (members, mine, open): (i64, i64, i64) = x.conn.query_row(&format!("SELECT {}, {}, {}", members_sql("?1"), memberships_sql("?2"), open_sql("?1")), params![code.circle_id, me], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))?;
    if open == 1 {
        let rows: i64 = x.conn.query_row(&format!("SELECT {}", membership_rows_sql("?1")), [me], |r| r.get(0))?;
        if mine >= caps.memberships || rows >= caps.rows {
            refund()?;
            return Err(limit_error(x, me, caps, false)?.into());
        }
        if members >= members_cap {
            refund()?;
            return Err(PoolError::new(409, "circle_full").with("limit", members_cap.into()).with("message", format!("This circle has {members_cap} members, the most allowed.").into()).into());
        }
    }
    Err(invalid_code().into())
}

#[cfg(test)]
#[path = "circleinvites_tests.rs"]
mod tests;
