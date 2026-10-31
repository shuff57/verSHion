//! Leaving, removing, roles, ownership transfer and succession (worker/src/pool/circles.ts "leave, remove, role, transfer" and
//! `successionStatements`). HTTP-free cores.
use super::circlebase::*;
use super::circles::{bad_field, user_id_of};
use super::circlesql::*;
use super::ctx::{AuditEntry, PoolCtx, PoolError, Res, audit, not_found};
use super::sqlfrag::alive_sql;
use rusqlite::{Connection, OptionalExtension, ToSql, params};
use serde_json::{Map, Value, json};

/// Revokes `who`'s open codes of circle `cid` when `cond` holds (SQL; ?1 = cid, ?2 = who, ?3 = now).
fn revoke_codes_of(conn: &Connection, cid: &str, who: &str, now: i64, cond: &str) -> rusqlite::Result<usize> {
    conn.execute(&format!("UPDATE circle_invites SET revoked_at = ?3, revoked_by = ?2 WHERE circle_id = ?1 AND created_by = ?2 AND revoked_at IS NULL AND {cond}"), params![cid, who, now])
}

/// Ownership succession for every circle in `scope` (a SQL subquery of circle ids using the placeholders bound by `scope_params`) that
/// is OPEN, or closed by its owner and still inside the undo window, and has no active owner: the active editor in good standing with
/// the smallest join_seq becomes owner; where there is none, such a viewer; an open circle with no member in good standing left
/// closes ('last_member_gone', purged after `purge_at`). Each pick's condition is "no active owner", so a rerun finds exactly what is
/// left to do. Audited (system) for the activity list. `now` and `purge_at` are numbers.
pub fn succession(conn: &Connection, scope: &str, scope_params: &[&dyn ToSql], now: i64, purge_at: i64) -> rusqlite::Result<()> {
    let needs = format!(
        "c.id IN ({scope}) AND c.purged_at IS NULL AND (c.closed_at IS NULL OR (c.close_reason = 'owner_closed' AND c.undo_until > {now}))
      AND NOT EXISTS (SELECT 1 FROM circle_members o WHERE o.circle_id = c.id AND o.role = 'owner' AND o.left_at IS NULL)"
    );
    let pick = |role: &str, col: &str| format!("(SELECT m.{col} FROM circle_members m WHERE m.circle_id = c.id AND m.left_at IS NULL AND m.role = '{role}' AND {} ORDER BY m.join_seq LIMIT 1)", in_standing_sql("m.user_id"));
    for role in ["editor", "viewer"] {
        conn.execute(
            &format!(
                "INSERT INTO pool_audit (id, at, actor_id, actor_kind, action, item_id, target_user_id, detail)
       SELECT lower(hex(randomblob(16))), {now}, NULL, 'system', 'circle.owner_succession', c.id, {}, json_object('from', '{role}')
         FROM circles c WHERE {needs} AND {} IS NOT NULL",
                pick(role, "user_id"),
                pick(role, "user_id")
            ),
            scope_params,
        )?;
        conn.execute(&format!("UPDATE circle_members SET role = 'owner' WHERE rowid IN (SELECT {} FROM circles c WHERE {needs})", pick(role, "rowid")), scope_params)?;
    }
    // (an open circle with no owner and no member in good standing left closes)
    let empty = format!(
        "id IN ({scope}) AND closed_at IS NULL AND purged_at IS NULL
      AND NOT EXISTS (SELECT 1 FROM circle_members o WHERE o.circle_id = circles.id AND o.role = 'owner' AND o.left_at IS NULL)
      AND NOT EXISTS (SELECT 1 FROM circle_members o WHERE o.circle_id = circles.id AND o.left_at IS NULL AND {})",
        in_standing_sql("o.user_id")
    );
    conn.execute(
        &format!("INSERT INTO pool_audit (id, at, actor_id, actor_kind, action, item_id, detail) SELECT lower(hex(randomblob(16))), {now}, NULL, 'system', 'circle.close', id, json_object('reason', 'last_member_gone') FROM circles WHERE {empty}"),
        scope_params,
    )?;
    conn.execute(&format!("UPDATE circles SET closed_at = {now}, close_reason = 'last_member_gone', undo_until = NULL, purge_after = {purge_at}, updated_at = {now} WHERE {empty}"), scope_params)?;
    Ok(())
}

fn active_row(x: &PoolCtx, cid: &str, who: &str) -> rusqlite::Result<Option<String>> {
    x.conn.query_row("SELECT role FROM circle_members WHERE circle_id = ?1 AND user_id = ?2 AND left_at IS NULL", params![cid, who], |r| r.get(0)).optional()
}

/// POST /circles/:cid/leave: any member. An owner leaving hands the circle on or closes it.
pub fn leave_circle(x: &PoolCtx, me: &str, cid: &str) -> Res<Value> {
    circle_or_miss(x, cid, me)?;
    let tx = x.conn.unchecked_transaction()?;
    audit(&tx, &AuditEntry { actor_id: Some(me), actor_kind: "user", action: "circle.leave", item_id: Some(cid), target_user_id: Some(me), ..Default::default() }, x.now, Some(&format!("{} AND {}", active_sql("?6", "?3", &[]), alive_sql("?3"))))?;
    let upd = tx.execute(&format!("UPDATE circle_members SET left_at = ?3, left_reason = 'left' WHERE circle_id = ?1 AND user_id = ?2 AND left_at IS NULL AND {}", alive_sql("?2")), params![cid, me, x.now])?;
    // (codes never outlive their issuer's owner role)
    revoke_codes_of(&tx, cid, me, x.now, "EXISTS (SELECT 1 FROM circle_members lm WHERE lm.circle_id = ?1 AND lm.user_id = ?2 AND lm.left_at = ?3)")?;
    // (an ownership offer from or to the leaver goes)
    tx.execute(
        "DELETE FROM circle_transfers WHERE circle_id = ?1 AND (from_user = ?2 OR to_user = ?2) AND EXISTS (SELECT 1 FROM circle_members lm WHERE lm.circle_id = ?1 AND lm.user_id = ?2 AND lm.left_at = ?3)",
        params![cid, me, x.now],
    )?;
    succession(&tx, "SELECT ?1", &[&cid], x.now, x.now + CIRCLE_UNDO_MS)?;
    tx.commit()?;
    if upd != 1 {
        x.throw_if_erased(me)?;
        return Err(not_found().into());
    }
    Ok(json!({ "ok": true }))
}

/// POST /circles/:cid/members/remove {user_id} (owner; never yourself: leave instead). Immediate.
pub fn remove_member(x: &PoolCtx, me: &str, cid: &str, b: &Map<String, Value>) -> Res<Value> {
    let m = circle_or_miss(x, cid, me)?;
    require_open(&m)?;
    require_role(&m, &["owner"])?;
    let who = user_id_of(b.get("user_id"))?;
    if who == me {
        return Err(bad_field("user_id", &[("message", "To stop being a member yourself, leave the circle.".into())]));
    }
    let tx = x.conn.unchecked_transaction()?;
    let upd = tx.execute(
        &format!(
            "UPDATE circle_members SET left_at = ?4, left_reason = 'removed' WHERE circle_id = ?1 AND user_id = ?3 AND left_at IS NULL AND role <> 'owner'
           AND {} AND {} AND {}",
            open_sql("?1"),
            active_sql("?1", "?2", &["owner"]),
            alive_sql("?2")
        ),
        params![cid, me, who, x.now],
    )?;
    if upd == 1 {
        audit(&tx, &AuditEntry { actor_id: Some(me), actor_kind: "user", action: "circle.member_remove", item_id: Some(cid), target_user_id: Some(&who), ..Default::default() }, x.now, None)?;
        revoke_codes_of(&tx, cid, &who, x.now, "EXISTS (SELECT 1 FROM circle_members rm2 WHERE rm2.circle_id = ?1 AND rm2.user_id = ?2 AND rm2.left_reason = 'removed' AND rm2.left_at = ?3)")?;
        tx.execute(
            "DELETE FROM circle_transfers WHERE circle_id = ?1 AND to_user = ?2 AND EXISTS (SELECT 1 FROM circle_members rm3 WHERE rm3.circle_id = ?1 AND rm3.user_id = ?2 AND rm3.left_reason = 'removed' AND rm3.left_at = ?3)",
            params![cid, who, x.now],
        )?;
    }
    tx.commit()?;
    if upd != 1 {
        x.throw_if_erased(me)?;
        if active_row(x, cid, &who)?.is_none() {
            return Err(not_found().into());
        }
        return Err(explain(x, cid, me, Some(&["owner"])));
    }
    Ok(json!({ "ok": true }))
}

/// POST /circles/:cid/members/role {user_id, role: editor|viewer} (owner).
pub fn set_role(x: &PoolCtx, me: &str, cid: &str, b: &Map<String, Value>) -> Res<Value> {
    let m = circle_or_miss(x, cid, me)?;
    require_open(&m)?;
    require_role(&m, &["owner"])?;
    let who = user_id_of(b.get("user_id"))?;
    let role = match b.get("role").and_then(Value::as_str) {
        Some(r @ ("editor" | "viewer")) => r,
        _ => return Err(bad_field("role", &[])),
    };
    if who == me {
        return Err(bad_field("user_id", &[("message", "To change your own role, pass ownership on first.".into())]));
    }
    let tx = x.conn.unchecked_transaction()?;
    let upd = tx.execute(
        &format!(
            "UPDATE circle_members SET role = ?4 WHERE circle_id = ?1 AND user_id = ?3 AND left_at IS NULL AND role IN ('editor', 'viewer') AND role <> ?4
           AND {} AND {} AND {}",
            open_sql("?1"),
            active_sql("?1", "?2", &["owner"]),
            alive_sql("?2")
        ),
        params![cid, me, who, role],
    )?;
    if upd == 1 {
        audit(&tx, &AuditEntry { actor_id: Some(me), actor_kind: "user", action: "circle.role", item_id: Some(cid), target_user_id: Some(&who), detail: Some(json!({ "role": role })), ..Default::default() }, x.now, None)?;
    }
    tx.commit()?;
    if upd != 1 {
        x.throw_if_erased(me)?;
        match active_row(x, cid, &who)? {
            None => return Err(not_found().into()),
            Some(r) if r == role => return Ok(json!({ "ok": true, "changed": false })),
            Some(_) => return Err(explain(x, cid, me, Some(&["owner"]))),
        }
    }
    Ok(json!({ "ok": true, "changed": true }))
}

fn pending_of(x: &PoolCtx, to: &str, expires: i64) -> rusqlite::Result<Value> {
    let u: Option<(Option<String>, Option<i64>)> = x.conn.query_row("SELECT display_name, deleted_at FROM users WHERE id = ?1", [to], |r| Ok((r.get(0)?, r.get(1)?))).optional()?;
    let name = match &u {
        Some((d, del)) => name_of(d.as_deref(), *del),
        None => name_of(None, Some(1)),
    };
    Ok(json!({ "to_user_id": to, "to_name": name, "expires_at": expires }))
}

/// POST /circles/:cid/transfer {user_id} (owner): OFFERS the circle to an active member in good standing; nothing changes until they
/// accept within 7 days; a new offer replaces the pending one. -> (202, {ok, pending}); 409 member_unavailable.
pub fn offer_transfer(x: &PoolCtx, me: &str, cid: &str, b: &Map<String, Value>) -> Res<(u16, Value)> {
    let m = circle_or_miss(x, cid, me)?;
    require_open(&m)?;
    require_role(&m, &["owner"])?;
    let who = user_id_of(b.get("user_id"))?;
    if who == me {
        return Err(bad_field("user_id", &[("message", "You already own this circle.".into())]));
    }
    // The same open offer again changes nothing and writes nothing (it is not charged).
    let open: Option<(String, String, i64)> =
        x.conn.query_row("SELECT from_user, to_user, expires_at FROM circle_transfers WHERE circle_id = ?1 AND expires_at > ?2", params![cid, x.now], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?))).optional()?;
    if let Some((from, to, exp)) = &open {
        if from == me && *to == who {
            return Ok((202, json!({ "ok": true, "pending": pending_of(x, &who, *exp)? })));
        }
    }
    x.require_daily(me, "circle_transfer_offers", 1)?;
    let until = x.now + TRANSFER_OFFER_MS;
    let tx = x.conn.unchecked_transaction()?;
    let ins = tx.execute(
        &format!(
            "INSERT INTO circle_transfers (circle_id, from_user, to_user, created_at, expires_at) SELECT ?1, ?2, ?3, ?4, ?5
          WHERE {} AND {} AND {}
            AND EXISTS (SELECT 1 FROM circle_members t WHERE t.circle_id = ?1 AND t.user_id = ?3 AND t.left_at IS NULL AND t.role <> 'owner') AND {}
         ON CONFLICT (circle_id) DO UPDATE SET from_user = excluded.from_user, to_user = excluded.to_user, created_at = excluded.created_at, expires_at = excluded.expires_at",
            open_sql("?1"),
            active_sql("?1", "?2", &["owner"]),
            alive_sql("?2"),
            in_standing_sql("?3")
        ),
        params![cid, me, who, x.now, until],
    )?;
    if ins == 1 {
        // (keyed by the account, the circle in the detail: offers and cancels are not members' activity)
        audit(&tx, &AuditEntry { actor_id: Some(me), actor_kind: "user", action: "circle.transfer_offer", item_id: None, target_user_id: Some(&who), detail: Some(json!({ "circle_id": cid })), ..Default::default() }, x.now, None)?;
    }
    tx.commit()?;
    if ins != 1 {
        x.give_daily(me, "circle_transfer_offers", 1)?;
        x.throw_if_erased(me)?;
        if active_row(x, cid, &who)?.is_none() {
            return Err(not_found().into());
        }
        let ok: i64 = x.conn.query_row(&format!("SELECT {}", in_standing_sql("?1")), [&who], |r| r.get(0))?;
        if ok == 0 {
            return Err(PoolError::new(409, "member_unavailable").with("message", "That member's account cannot take over a circle now.".into()).into());
        }
        return Err(explain(x, cid, me, Some(&["owner"])));
    }
    Ok((202, json!({ "ok": true, "pending": pending_of(x, &who, until)? })))
}

/// POST /circles/:cid/transfer/accept (the member offered the circle): takes over while the offer is open. The old owner becomes an
/// editor and their open codes are revoked. The taker's circles_owned cap is checked in the step-down statement itself.
pub fn accept_transfer(x: &PoolCtx, me: &str, cid: &str) -> Res<Value> {
    let m = circle_or_miss(x, cid, me)?;
    require_open(&m)?;
    let none = || PoolError::new(409, "no_pending_transfer").with("message", "There is no open offer of this circle to you.".into());
    let from: Option<String> = x.conn.query_row("SELECT from_user FROM circle_transfers WHERE circle_id = ?1 AND to_user = ?2 AND expires_at > ?3", params![cid, me, x.now], |r| r.get(0)).optional()?;
    let Some(from) = from.filter(|_| m.role != "owner") else { return Err(none().into()) };
    let cap = x.limit_of(me, "circles_owned")?;
    let tx = x.conn.unchecked_transaction()?;
    let down = tx.execute(
        &format!(
            "UPDATE circle_members SET role = 'editor' WHERE circle_id = ?1 AND user_id = ?3 AND role = 'owner' AND left_at IS NULL AND {}
           AND EXISTS (SELECT 1 FROM circle_transfers tr WHERE tr.circle_id = ?1 AND tr.from_user = ?3 AND tr.to_user = ?2 AND tr.expires_at > ?4)
           AND EXISTS (SELECT 1 FROM circle_members tm WHERE tm.circle_id = ?1 AND tm.user_id = ?2 AND tm.left_at IS NULL AND tm.role <> 'owner')
           AND {} AND {} < ?5",
            open_sql("?1"),
            alive_sql("?2"),
            owned_sql("?2")
        ),
        params![cid, me, from, x.now, cap],
    )?;
    let mut up = 0;
    if down == 1 {
        up = tx.execute(
            "UPDATE circle_members SET role = 'owner' WHERE circle_id = ?1 AND user_id = ?2 AND left_at IS NULL
           AND NOT EXISTS (SELECT 1 FROM circle_members o WHERE o.circle_id = ?1 AND o.role = 'owner' AND o.left_at IS NULL)",
            params![cid, me],
        )?;
    }
    if up == 1 {
        audit(&tx, &AuditEntry { actor_id: Some(&from), actor_kind: "user", action: "circle.transfer", item_id: Some(cid), target_user_id: Some(me), detail: Some(json!({ "accepted": true })), ..Default::default() }, x.now, None)?;
        let is_owner = "EXISTS (SELECT 1 FROM circle_members nw WHERE nw.circle_id = ?1 AND nw.user_id = ?4 AND nw.role = 'owner' AND nw.left_at IS NULL)";
        // (the old owner's open codes go with the role; the offer is used)
        tx.execute(&format!("UPDATE circle_invites SET revoked_at = ?3, revoked_by = ?2 WHERE circle_id = ?1 AND created_by = ?2 AND revoked_at IS NULL AND {is_owner}"), params![cid, from, x.now, me])?;
        tx.execute("DELETE FROM circle_transfers WHERE circle_id = ?1 AND EXISTS (SELECT 1 FROM circle_members nw WHERE nw.circle_id = ?1 AND nw.user_id = ?2 AND nw.role = 'owner' AND nw.left_at IS NULL)", params![cid, me])?;
        tx.commit()?;
    } else {
        // (a step-down without its step-up is undone: never an ownerless circle)
        tx.rollback()?;
        x.throw_if_erased(me)?;
        let owned: i64 = x.conn.query_row(&format!("SELECT {}", owned_sql("?1")), [me], |r| r.get(0))?;
        if owned >= cap {
            return Err(PoolError::new(422, "circles_limit").with("metric", "circles_owned".into()).with("used", owned.into()).with("limit", cap.into()).with("message", owned_message(cap, true).into()).into());
        }
        return Err(none().into());
    }
    Ok(json!({ "ok": true, "role": "owner" }))
}

/// POST /circles/:cid/transfer/cancel: the owner withdraws the pending offer, or the member offered it declines.
pub fn cancel_transfer(x: &PoolCtx, me: &str, cid: &str) -> Res<Value> {
    circle_or_miss(x, cid, me)?;
    let tx = x.conn.unchecked_transaction()?;
    let del = tx.execute(&format!("DELETE FROM circle_transfers WHERE circle_id = ?1 AND (to_user = ?2 OR {}) AND {}", active_sql("?1", "?2", &["owner"]), alive_sql("?2")), params![cid, me])?;
    if del == 1 {
        audit(&tx, &AuditEntry { actor_id: Some(me), actor_kind: "user", action: "circle.transfer_cancel", item_id: None, target_user_id: Some(me), detail: Some(json!({ "circle_id": cid })), ..Default::default() }, x.now, None)?;
    }
    tx.commit()?;
    if del == 0 {
        x.throw_if_erased(me)?;
    }
    Ok(json!({ "ok": true, "cancelled": del == 1 }))
}

#[cfg(test)]
#[path = "circlemembers_tests.rs"]
mod tests;
