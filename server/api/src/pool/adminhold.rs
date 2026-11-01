//! The pool admin routes, last part (worker/src/pool/admin.ts): court action and resolution of a takedown, holding and releasing its copies, naming the
//! rightful author, withdrawing a notice, and the pool key check. HTTP-free cores behind the admin guard.
use super::adminblock::copies_json;
use super::adminpool::{id_field, note_field};
use super::blocking::{PREVIEW_MS, check_token, hold_snapshot, release_orphan_holds, token_for};
use super::ctx::{AuditEntry, Fail, PoolCtx, PoolError, Res, audit};
use super::modlineage::{LINEAGE_COPIES_MAX, LineageOpts, lineage_copies};
use super::modpurge::Actor;
use super::modrestore::{restore, reverse_uphold, uphold_takedown};
use super::poolio::notify_held;
use super::poolkeys::PoolKeys;
use super::sqlfrag::new_id;
use crate::auth::audit::{AuditFields, audit as auth_audit};
use crate::blobs::BlobStore;
use rusqlite::{OptionalExtension, params};
use serde_json::{Map, Value, json};

fn bad(field: &str) -> Fail {
    PoolError::new(400, "invalid_request").with("field", json!(field)).into()
}
fn is_uuid(s: &str) -> bool {
    s.len() == 36 && s.bytes().all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f' | b'-'))
}
fn is_block_key(s: &str) -> bool {
    let h = match s.as_bytes() {
        [b'n' | b'f' | b'e' | b'a', b':', ..] => &s[2..],
        _ => s,
    };
    h.len() == 64 && h.bytes().all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f'))
}
fn trail(x: &PoolCtx, admin: &str, action: &str, target: &str, note: Option<&str>, detail: Option<Value>) -> Res<()> {
    auth_audit(x.conn, Some(admin), "admin", &AuditFields { action, target_id: Some(target), note, detail, ..Default::default() }, x.now, None, false, &new_id())?;
    Ok(())
}
fn both(x: &PoolCtx, e: &AuditEntry, admin: &str, action: &str, target: &str, note: Option<&str>, detail: Option<Value>) -> Res<()> {
    audit(x.conn, e, x.now, None)?;
    trail(x, admin, action, target, note, detail)
}

/// POST /admin/dmca/court-action: the complainant's notice of a court action keeps the material disabled until resolved.
pub fn court_action(x: &PoolCtx, admin: &str, body: &Map<String, Value>) -> Res<(u16, Value)> {
    let aid = id_field(body, "action_id")?;
    let tx = x.conn.unchecked_transaction()?;
    let upd = tx.execute("UPDATE dmca_actions SET court_action_at = ?2 WHERE id = ?1 AND restored_at IS NULL AND purged_at IS NULL AND court_action_at IS NULL", params![aid, x.now])?;
    audit(&tx, &AuditEntry { actor_id: Some(admin), actor_kind: "admin", action: "dmca.court_action", item_id: None, target_user_id: None, detail: Some(json!({ "action_id": aid })), note: None }, x.now, None)?;
    auth_audit(&tx, Some(admin), "admin", &AuditFields { action: "pool.dmca_court_action", target_id: Some(&aid), ..Default::default() }, x.now, None, false, &new_id())?;
    tx.commit()?;
    Ok(if upd == 1 { (200, json!({ "ok": true })) } else { (404, json!({ "error": "not_found" })) })
}

/// POST /admin/dmca/resolve: the way out of a court action: the court sided with the author (restore) or the complainant (remove); `put_back` reverses an
/// UPHELD action (strike voided, evidence restored, blocks and holds lifted). The decision is recorded only once it took effect.
pub fn resolve(x: &PoolCtx, store: &BlobStore, admin: &str, body: &Map<String, Value>) -> Res<(u16, Value)> {
    let aid = id_field(body, "action_id")?;
    let outcome = match body.get("outcome").and_then(Value::as_str) {
        Some(o @ ("restore" | "remove" | "put_back")) => o,
        _ => return Err(bad("outcome")),
    };
    let n = note_field(body)?;
    if outcome == "put_back" {
        let ok = reverse_uphold(x, admin, &aid)?;
        if ok {
            trail(x, admin, "pool.dmca_resolve", &aid, n.as_deref(), Some(json!({ "outcome": outcome })))?;
        }
        return Ok(if ok { (200, json!({ "ok": true })) } else { (409, json!({ "error": "not_resolvable" })) });
    }
    let open: bool = x.conn.query_row("SELECT EXISTS (SELECT 1 FROM dmca_actions WHERE id = ?1 AND restored_at IS NULL AND purged_at IS NULL)", [&aid], |r| r.get(0))?;
    if !open {
        return Ok((404, json!({ "error": "not_found" })));
    }
    let ok = if outcome == "restore" { restore(x, store, &aid, &Actor { id: Some(admin), kind: "admin" }, "court_restore")? } else { uphold_takedown(x, store, admin, &aid)? };
    if ok {
        let e = AuditEntry { actor_id: Some(admin), actor_kind: "admin", action: "dmca.resolve", item_id: None, target_user_id: None, detail: Some(json!({ "action_id": aid, "outcome": outcome })), note: n.as_deref() };
        both(x, &e, admin, "pool.dmca_resolve", &aid, n.as_deref(), Some(json!({ "outcome": outcome })))?;
    }
    Ok(if ok { (200, json!({ "ok": true })) } else { (409, json!({ "error": "not_resolvable" })) })
}

struct CopiesQuery {
    item_id: String,
    copies: Vec<super::modlineage::Copy>,
    more: bool,
    dropped: Vec<String>,
    digest_tail: Value,
    ids: Vec<String>,
    aid: String,
}
impl CopiesQuery {
    fn digest(&self, admin: &str) -> String {
        let mut v = vec![json!("hold_copies"), json!(self.aid), json!(admin), json!(self.ids)];
        if let Value::Array(t) = &self.digest_tail {
            v.extend(t.clone());
        }
        Value::Array(v).to_string()
    }
}

/// The copies hold-copies acts on: lineage copies of the action's item that copy it whole or a designated question; designated questions that K or more
/// authors WITHOUT lineage to the item use (a common question) are dropped unless include_common.
fn copies_query(x: &PoolCtx, b: &Map<String, Value>) -> Res<Option<CopiesQuery>> {
    let aid = id_field(b, "action_id")?;
    let a: Option<(String, Option<i64>)> = x.conn.query_row("SELECT item_id, restored_at FROM dmca_actions WHERE id = ?1", [&aid], |r| Ok((r.get(0)?, r.get(1)?))).optional()?;
    let Some((item_id, restored)) = a else { return Ok(None) };
    if restored.is_some() {
        return Err(PoolError::new(409, "action_closed").with("message", json!("This takedown was put back or withdrawn; its copies are not held.")).into());
    }
    // By default the action's own blocked keys, so content copies (any lineage, any visibility) are found, not only lineage copies.
    let given = b.get("questions").and_then(Value::as_array);
    let mut designated: Option<Vec<String>> = match given {
        Some(a) => Some(a.iter().filter_map(Value::as_str).filter(|s| is_block_key(s)).map(str::to_string).collect()),
        None => {
            let mut st = x.conn.prepare("SELECT hash FROM blocked_hashes WHERE action_id = ?1 AND kind = 'match'")?;
            let v: Vec<String> = st.query_map([&aid], |r| r.get(0))?.collect::<Result<_, _>>()?;
            Some(v)
        }
    };
    if designated.as_ref().is_some_and(Vec::is_empty) && given.is_none() {
        designated = None;
    }
    let mut dropped: Vec<String> = vec![];
    let include_common = b.get("include_common") == Some(&Value::Bool(true));
    if let (Some(d), false) = (designated.clone(), include_common) {
        if !d.is_empty() {
            let k = x.limit_of("*", "block_common_k")?;
            let mut st = x.conn.prepare(
                "SELECT iq.match_hash FROM item_questions iq JOIN items i ON i.current_publish_id = iq.publish_id
                  WHERE iq.match_hash IN (SELECT value FROM json_each(?2)) AND i.id <> ?1 AND i.removed_at IS NULL AND i.visibility = 'public' AND i.purged_at IS NULL
                    AND NOT EXISTS (SELECT 1 FROM imports im WHERE im.user_id = i.author_id AND im.item_id = ?1)
                    AND NOT EXISTS (SELECT 1 FROM item_questions o WHERE o.publish_id = i.current_publish_id AND o.origin_item_id = ?1)
                  GROUP BY iq.match_hash HAVING COUNT(DISTINCT i.author_id) >= ?3",
            )?;
            let common: std::collections::HashSet<String> = st.query_map(params![item_id, json!(d).to_string(), k], |r| r.get(0))?.collect::<Result<_, _>>()?;
            dropped = d.iter().filter(|h| common.contains(*h)).cloned().collect();
            designated = Some(d.into_iter().filter(|h| !common.contains(h)).collect());
        }
    }
    let blobs: Vec<String> = {
        let mut st = x.conn.prepare("SELECT hash FROM blocked_hashes WHERE action_id = ?1 AND kind = 'blob'")?;
        let v = st.query_map([&aid], |r| r.get(0))?.collect::<Result<_, _>>()?;
        v
    };
    let include_unlinked = b.get("include_unlinked") == Some(&Value::Bool(true));
    let held_by = format!("dmca:{aid}");
    let page = lineage_copies(x, &item_id, designated.as_deref(), include_unlinked, Some(&blobs), &LineageOpts { held_by: &held_by, ..Default::default() })?;
    let more = page.len() > LINEAGE_COPIES_MAX;
    let copies: Vec<_> = page.into_iter().take(LINEAGE_COPIES_MAX).collect();
    let mut ids: Vec<String> = copies.iter().map(|c| c.item_id.clone()).collect();
    ids.sort();
    Ok(Some(CopiesQuery { item_id, copies, more, dropped, digest_tail: json!([designated, include_unlinked, include_common]), ids, aid }))
}

/// POST /admin/dmca/hold-copies-preview: the copies hold-copies would hold (designated questions that K or more unrelated authors use are dropped unless
/// include_common), with the token the confirm must quote.
pub fn hold_copies_preview(x: &PoolCtx, keys: &PoolKeys, admin: &str, body: &Map<String, Value>) -> Res<(u16, Value)> {
    let Some(q) = copies_query(x, body)? else { return Ok((404, json!({ "error": "not_found" }))) };
    Ok((200, json!({ "copies": copies_json(&q.copies), "dropped_common": q.dropped, "confirm_preview": token_for(x, keys, &q.digest(admin)), "expires_in": PREVIEW_MS / 1000 })))
}

/// POST /admin/dmca/hold-copies: hold every previewed copy of the taken-down item, tagged with the action.
pub fn hold_copies(x: &PoolCtx, keys: &PoolKeys, admin: &str, body: &Map<String, Value>) -> Res<(u16, Value)> {
    let aid = id_field(body, "action_id")?;
    let Some(q) = copies_query(x, body)? else { return Ok((404, json!({ "error": "not_found" }))) };
    check_token(x, keys, &q.digest(admin), body.get("confirm_preview").unwrap_or(&Value::Null))?;
    let ids: Vec<String> = q.copies.iter().map(|c| c.item_id.clone()).collect();
    let tx = x.conn.unchecked_transaction()?;
    tx.execute(
        &format!(
            "INSERT OR IGNORE INTO item_holds (item_id, reason, at, by, source, detail)
             SELECT j.value, 'admin', ?2, ?3, ?1, {} FROM json_each(?4) j",
            hold_snapshot("j.value", "(SELECT json_group_array(hash) FROM blocked_hashes WHERE action_id = ?5 AND kind = 'match')")
        ),
        params![format!("dmca:{aid}"), x.now, admin, json!(ids).to_string(), aid],
    )?;
    audit(&tx, &AuditEntry { actor_id: Some(admin), actor_kind: "admin", action: "dmca.hold_copies", item_id: Some(&q.item_id), target_user_id: None, detail: Some(json!({ "action_id": aid, "copies": ids.len() })), note: None }, x.now, None)?;
    auth_audit(&tx, Some(admin), "admin", &AuditFields { action: "pool.dmca_hold_copies", target_id: Some(&aid), detail: Some(json!({ "copies": ids.len() })), ..Default::default() }, x.now, None, false, &new_id())?;
    tx.commit()?;
    notify_held(x, &ids, "copyright")?;
    Ok((200, json!({ "ok": true, "held": ids, "more": q.more })))
}

/// GET /admin/dmca/copies?action_id=&held_after=&candidates_after=&after=: the copies held because of an action (offered for release together after a
/// put-back); each list has its OWN cursor, so neither skips what the other's page ended past.
pub fn copies_of_action(x: &PoolCtx, action_id: Option<&str>, held_after: Option<&str>, candidates_after: Option<&str>, after: Option<&str>) -> Res<(u16, Value)> {
    let aid = id_field(&json!({ "action_id": action_id }).as_object().unwrap().clone(), "action_id")?;
    let a: Option<(String, Option<i64>)> = x.conn.query_row("SELECT item_id, restored_at FROM dmca_actions WHERE id = ?1", [&aid], |r| Ok((r.get(0)?, r.get(1)?))).optional()?;
    let Some((item_id, restored)) = a else { return Ok((404, json!({ "error": "not_found" }))) };
    let held_after = held_after.or(after).unwrap_or("");
    let cand_after = candidates_after.or(after).unwrap_or("");
    let mut held = super::circleadmin::rows(x.conn, "SELECT item_id, reason FROM item_holds WHERE source = ?1 AND item_id > ?2 ORDER BY item_id LIMIT ?3", &[&format!("dmca:{aid}"), &held_after, &(LINEAGE_COPIES_MAX as i64 + 1)])?;
    let more_held = held.len() > LINEAGE_COPIES_MAX;
    held.truncate(LINEAGE_COPIES_MAX);
    // (a closed action's candidates are not offered for holding)
    let held_by = format!("dmca:{aid}");
    let cand = if restored.is_none() { lineage_copies(x, &item_id, None, false, None, &LineageOpts { held_by: &held_by, after: cand_after, ..Default::default() })? } else { vec![] };
    let more_cand = cand.len() > LINEAGE_COPIES_MAX;
    let page: Vec<_> = cand.into_iter().take(LINEAGE_COPIES_MAX).collect();
    Ok((
        200,
        json!({
            "restored": restored.is_some(), "held": held, "candidates": copies_json(&page),
            "next_held": if more_held { held.last().map(|h| h["item_id"].clone()).unwrap_or(Value::Null) } else { Value::Null },
            "next_candidates": if more_cand { json!(page.last().map(|c| c.item_id.clone())) } else { Value::Null },
        }),
    ))
}

/// GET /admin/pool/key: which POOL_HASH_KEY this pool's identity hashes are under. The key is permanent: a different key is refused (503
/// `pool_key_mismatch`), never re-keyed. `current` is the id of the configured key.
pub fn pool_key(x: &PoolCtx, current: &str) -> Res<Value> {
    let stored: Option<String> = x.conn.query_row("SELECT value FROM pool_kv WHERE key = 'pool_key_id'", [], |r| r.get(0)).optional()?;
    let matches = stored.as_deref().is_none_or(|s| s == current);
    Ok(json!({ "key_id": current, "stored_key_id": stored, "match": matches }))
}

/// POST /admin/dmca/release-copies: releases every copy an action holds.
pub fn release_copies(x: &PoolCtx, admin: &str, body: &Map<String, Value>) -> Res<(u16, Value)> {
    let aid = id_field(body, "action_id")?;
    let tx = x.conn.unchecked_transaction()?;
    let del = tx.execute("DELETE FROM item_holds WHERE source = ?1", [format!("dmca:{aid}")])?;
    audit(&tx, &AuditEntry { actor_id: Some(admin), actor_kind: "admin", action: "dmca.release_copies", item_id: None, target_user_id: None, detail: Some(json!({ "action_id": aid })), note: None }, x.now, None)?;
    auth_audit(&tx, Some(admin), "admin", &AuditFields { action: "pool.dmca_release_copies", target_id: Some(&aid), ..Default::default() }, x.now, None, false, &new_id())?;
    tx.commit()?;
    Ok((200, json!({ "ok": true, "released": del })))
}

/// POST /admin/dmca/exempt: names (or un-names) an author whose publishes an action's re-upload blocks never hold: the rightful author of the taken-down
/// material. Adding one releases the holds the action placed on that author's items.
pub fn exempt(x: &PoolCtx, admin: &str, body: &Map<String, Value>) -> Res<(u16, Value)> {
    let aid = id_field(body, "action_id")?;
    let uid = match body.get("user_id").and_then(Value::as_str) {
        Some(u) if is_uuid(u) => u.to_string(),
        _ => return Err(bad("user_id")),
    };
    let act = match body.get("action").and_then(Value::as_str) {
        Some(a @ ("add" | "remove")) => a,
        _ => return Err(bad("action")),
    };
    let n = note_field(body)?;
    let known: bool = x.conn.query_row("SELECT EXISTS (SELECT 1 FROM dmca_actions WHERE id = ?1)", [&aid], |r| r.get(0))?;
    if !known {
        return Ok((404, json!({ "error": "not_found" })));
    }
    let tx = x.conn.unchecked_transaction()?;
    let mut released = 0;
    if act == "add" {
        tx.execute("INSERT OR IGNORE INTO dmca_exempt (action_id, user_id, at) VALUES (?1, ?2, ?3)", params![aid, uid, x.now])?;
        released = tx.execute("DELETE FROM item_holds WHERE source = ?1 AND item_id IN (SELECT id FROM items WHERE author_id = ?2)", params![format!("dmca:{aid}"), uid])?;
    } else {
        tx.execute("DELETE FROM dmca_exempt WHERE action_id = ?1 AND user_id = ?2", params![aid, uid])?;
    }
    audit(&tx, &AuditEntry { actor_id: Some(admin), actor_kind: "admin", action: &format!("dmca.exempt_{act}"), item_id: None, target_user_id: Some(&uid), detail: Some(json!({ "action_id": aid })), note: n.as_deref() }, x.now, None)?;
    auth_audit(&tx, Some(admin), "admin", &AuditFields { action: &format!("pool.dmca_exempt_{act}"), target_id: Some(&aid), note: n.as_deref(), detail: Some(json!({ "user_id": uid })), ..Default::default() }, x.now, None, false, &new_id())?;
    tx.commit()?;
    Ok((200, json!({ "ok": true, "released": released })))
}

/// POST /admin/dmca/withdraw: a withdrawn notice: everything taken down under it is put back. A retry re-runs the release of the holds its blocks caused,
/// idempotently.
pub fn withdraw(x: &PoolCtx, store: &BlobStore, admin: &str, body: &Map<String, Value>) -> Res<(u16, Value)> {
    let nid = id_field(body, "notice_id")?;
    let n = note_field(body)?;
    let was: Option<String> = x.conn.query_row("SELECT status FROM dmca_notices WHERE id = ?1", [&nid], |r| r.get(0)).optional()?;
    let Some(was) = was else { return Ok((404, json!({ "error": "notice_not_found" }))) };
    let tx = x.conn.unchecked_transaction()?;
    tx.execute("UPDATE dmca_notices SET status = 'withdrawn' WHERE id = ?1", [&nid])?;
    audit(&tx, &AuditEntry { actor_id: Some(admin), actor_kind: "admin", action: "dmca.withdraw", item_id: None, target_user_id: None, detail: Some(json!({ "notice_id": nid, "was": was })), note: n.as_deref() }, x.now, None)?;
    auth_audit(&tx, Some(admin), "admin", &AuditFields { action: "pool.dmca_withdraw", target_id: Some(&nid), note: n.as_deref(), ..Default::default() }, x.now, None, false, &new_id())?;
    tx.commit()?;
    let ids = |sql: &str| -> rusqlite::Result<Vec<String>> { x.conn.prepare(sql)?.query_map([&nid], |r| r.get(0))?.collect() };
    let acts = ids("SELECT id FROM dmca_actions WHERE notice_id = ?1 AND restored_at IS NULL AND purged_at IS NULL")?;
    for a in &acts {
        restore(x, store, a, &Actor { id: Some(admin), kind: "admin" }, "withdrawn")?;
    }
    for a in ids("SELECT id FROM dmca_actions WHERE notice_id = ?1 AND restored_at IS NOT NULL")? {
        release_orphan_holds(x, &format!("dmca:{a}"), "blocklist")?;
    }
    Ok((200, json!({ "ok": true, "restored": acts.len() })))
}

#[cfg(test)]
#[path = "adminhold_tests.rs"]
mod tests;
