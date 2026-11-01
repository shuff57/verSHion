//! The pool admin routes, second half (worker/src/pool/admin.ts): previews and confirms of privacy purges, evidence, unblocks, the DMCA records, appeals,
//! flags, limits and notices, as HTTP-free cores behind the admin guard (the route layer checks admin role, device and sudo, and flushes `pool_outbox`).
//! `takedown` itself is `modtakedown`; its route (preview token, block days) is `takedown_route` here.
use super::adminpool::{id_field, note_field};
use super::blocking::*;
use super::blockplan::block_plan;
use super::circleadmin::{circle_unblock, rows};
use super::circlesendprep::SendEnv;
use super::ctx::{AuditEntry, Fail, PoolCtx, PoolError, Res, audit};
use super::modgc::{Who, void_evidence};
use super::modlineage::{LINEAGE_COPIES_MAX, LineageOpts, lineage_copies};
use super::modprivacy::{Decided, decide_appeal, privacy_purge, privacy_restore, purge_held_copies};
use super::modpurge::Actor;
use super::modtakedown::{RightfulCheck, rightful_check, takedown};
use super::poolkeys::PoolKeys;
use super::served::current_serves_blob;
use super::sqlfrag::{DAY_MS, alive_sql, default_limit, is_id, new_id};
use crate::auth::audit::{AuditFields, audit as auth_audit};
use crate::auth::validate::clean_text;
use crate::blobs::BlobStore;
use rusqlite::{OptionalExtension, params};
use serde_json::{Map, Value, json};

fn bad(field: &str) -> Fail {
    PoolError::new(400, "invalid_request").with("field", json!(field)).into()
}
/// `^[0-9a-f-]{36}$`
fn is_uuid(s: &str) -> bool {
    s.len() == 36 && s.bytes().all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f' | b'-'))
}
/// `^(?:[nfea]:)?[0-9a-f]{64}$`
fn is_block_key(s: &str) -> bool {
    let h = match s.as_bytes() {
        [b'n' | b'f' | b'e' | b'a', b':', ..] => &s[2..],
        _ => s,
    };
    h.len() == 64 && h.bytes().all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f'))
}
/// The rightful authors named on a takedown (at most 20 user ids), sorted and unique.
fn exempt_list(v: Option<&Value>) -> Res<Vec<String>> {
    match v {
        None | Some(Value::Null) => Ok(vec![]),
        Some(Value::Array(a)) if a.len() <= 20 && a.iter().all(|x| x.as_str().is_some_and(is_uuid)) => {
            let mut l: Vec<String> = a.iter().map(|x| x.as_str().unwrap().to_string()).collect();
            l.sort();
            l.dedup();
            Ok(l)
        }
        Some(_) => Err(bad("exempt_authors")),
    }
}
fn auth_line(x: &PoolCtx, admin: &str, action: &str, target: Option<&str>, note: Option<&str>, detail: Option<Value>) -> Res<()> {
    auth_audit(x.conn, Some(admin), "admin", &AuditFields { action, target_id: target, note, detail, ..Default::default() }, x.now, None, false, &new_id())?;
    Ok(())
}
fn rc_json(rc: &RightfulCheck) -> Value {
    let hashes: Vec<Value> = rc
        .hashes
        .iter()
        .map(|h| {
            let mut m = Map::new();
            m.insert("kind".into(), json!(h.kind));
            m.insert("hash".into(), json!(h.hash));
            m.insert("rightful_seq".into(), json!(h.rightful_seq));
            m.insert("item_seq".into(), json!(h.item_seq));
            if h.derived {
                m.insert("derived".into(), json!(true));
            }
            m.insert("ok".into(), json!(h.ok));
            Value::Object(m)
        })
        .collect();
    json!({ "ok": rc.ok, "reasons": rc.reasons, "hashes": hashes })
}
fn plan_value(plan: &BlockPlan) -> Map<String, Value> {
    match serde_json::to_value(plan).unwrap() {
        Value::Object(m) => m,
        _ => unreachable!(),
    }
}
fn bound(pairs: Vec<(&str, Value)>) -> Map<String, Value> {
    pairs.into_iter().map(|(k, v)| (k.to_string(), v)).collect()
}

/// POST /admin/pool/purge-preview: what a privacy purge would block and hold; the purge must quote its confirm_preview token.
pub fn purge_preview(x: &PoolCtx, keys: &PoolKeys, admin: &str, body: &Map<String, Value>) -> Res<Value> {
    let plan = block_plan(x, AFFECTED_LIST, &id_field(body, "item_id")?, &read_selection(Some(body))?, "privacy", None)?;
    let mut m = plan_value(&plan);
    m.insert("large".into(), json!(plan.affected_total > plan.affected.len() as i64));
    m.insert("confirm_preview".into(), json!(preview_token(x, keys, "privacy", &plan, &bound(vec![("admin", json!(admin))]))));
    m.insert("expires_in".into(), json!(PREVIEW_MS / 1000));
    Ok(Value::Object(m))
}

fn rightful_of(body: &Map<String, Value>) -> Option<String> {
    body.get("rightful_item_id").and_then(Value::as_str).map(str::to_string)
}
fn takedown_bind(admin: &str, body: &Map<String, Value>, exempt: &[String]) -> Map<String, Value> {
    bound(vec![
        ("admin", json!(admin)),
        ("notice_id", body.get("notice_id").cloned().unwrap_or(Value::Null)),
        ("block_days", body.get("block_days").cloned().unwrap_or(Value::Null)),
        ("exempt", json!(exempt)),
        ("rightful", body.get("rightful_item_id").cloned().unwrap_or(Value::Null)),
    ])
}

/// POST /admin/dmca/takedown-preview: the plan, the rightful-item check (per blocked hash, the earliest publish in the rightful item and in this one), the
/// lineage copies, and a token binding the admin, the notice, the block's end and the exempt authors.
pub fn takedown_preview(x: &PoolCtx, keys: &PoolKeys, admin: &str, body: &Map<String, Value>) -> Res<Value> {
    let plan = block_plan(x, AFFECTED_LIST, &id_field(body, "item_id")?, &read_selection(Some(body))?, "dmca", rightful_of(body).as_deref())?;
    let exempt = exempt_list(body.get("exempt_authors"))?;
    let rc = match rightful_of(body) {
        Some(r) if is_id(&r) => Some(rc_json(&rightful_check(x, &r, &plan.item_id, &selected_match(&plan), &selected_blobs(&plan))?)),
        _ => None,
    };
    let copies = lineage_copies(x, &plan.item_id, Some(&selected_match(&plan)), false, Some(&selected_blobs(&plan)), &LineageOpts::default())?;
    let mut m = plan_value(&plan);
    if let Some(rc) = rc {
        m.insert("rightful_check".into(), rc);
    }
    m.insert("copies".into(), copies_json(&copies));
    m.insert("confirm_preview".into(), json!(preview_token(x, keys, "dmca", &plan, &takedown_bind(admin, body, &exempt))));
    m.insert("expires_in".into(), json!(PREVIEW_MS / 1000));
    Ok(Value::Object(m))
}
pub fn copies_json(c: &[super::modlineage::Copy]) -> Value {
    Value::Array(c.iter().map(|c| json!({ "item_id": c.item_id, "author_id": c.author_id, "title": c.title, "visible": c.visible })).collect())
}

/// POST /admin/dmca/takedown: confirms a previewed takedown. The preview token binds the admin, notice, block days and exempt authors; the rightful item is
/// validated over exactly the previewed hashes.
pub fn takedown_route(x: &PoolCtx, store: &BlobStore, keys: &PoolKeys, admin: &str, body: &Map<String, Value>) -> Res<(u16, Value)> {
    let nid = id_field(body, "notice_id")?;
    let item_id = id_field(body, "item_id")?;
    let plan = block_plan(x, AFFECTED_LIST, &item_id, &read_selection(Some(body))?, "dmca", rightful_of(body).as_deref())?;
    let days = match body.get("block_days") {
        None | Some(Value::Null) => None,
        Some(v) => match v.as_i64().filter(|d| (1..=3650).contains(d)) {
            Some(d) => Some(d),
            None => return Err(bad("block_days")),
        },
    };
    let exempt = exempt_list(body.get("exempt_authors"))?;
    let rightful = match body.get("rightful_item_id") {
        None | Some(Value::Null) => None,
        Some(_) => Some(id_field(body, "rightful_item_id")?),
    };
    if let Some(r) = rightful.as_deref() {
        let rc = rightful_check(x, r, &plan.item_id, &selected_match(&plan), &selected_blobs(&plan))?;
        if !rc.ok {
            let bad_hashes: Vec<Value> = rc_json(&rc)["hashes"].as_array().unwrap().iter().filter(|h| h["ok"] == false).take(50).cloned().collect();
            return Err(PoolError::new(409, "rightful_item_invalid")
                .with("reasons", json!(rc.reasons))
                .with("hashes", Value::Array(bad_hashes))
                .with("message", json!("The rightful item must be another author's visible item that had this material first."))
                .into());
        }
    }
    let bind = bound(vec![("admin", json!(admin)), ("notice_id", json!(nid)), ("block_days", json!(days)), ("exempt", json!(exempt)), ("rightful", json!(rightful))]);
    check_preview(x, keys, "dmca", &plan, body.get("confirm_preview").unwrap_or(&Value::Null), &bind)?;
    auth_line(x, admin, "pool.takedown", Some(&item_id), None, Some(json!({ "notice_id": nid, "block_days": days, "exempt_authors": exempt })))?;
    // Re-upload blocks expire: block_days, or DMCA_BLOCK_DEFAULT_DAYS when not given.
    let until = Some(x.now + days.unwrap_or(super::adminpool::DMCA_BLOCK_DEFAULT_DAYS) * DAY_MS);
    let out = takedown(x, store, admin, &nid, &item_id, &plan, until, &exempt, rightful.as_deref())?;
    // Copies (same questions, or recorded as derived from it) are listed; "hold-copies" hides them: the first page, and whether there are more.
    let more = out.copies.len() > LINEAGE_COPIES_MAX;
    Ok((200, json!({ "ok": true, "action_id": out.action_id, "copies": copies_json(&out.copies[..out.copies.len().min(LINEAGE_COPIES_MAX)]), "copies_more": more })))
}

/// POST /admin/pool/evidence: void or restore licence/attribution evidence: everything copied from an item, or everything an account was served. Audited.
pub fn evidence(x: &PoolCtx, admin: &str, body: &Map<String, Value>) -> Res<(u16, Value)> {
    let item = match body.get("item_id") {
        None => None,
        Some(_) => Some(id_field(body, "item_id")?),
    };
    let user = match body.get("user_id") {
        None => None,
        Some(Value::String(s)) if is_uuid(s) => Some(s.clone()),
        Some(_) => return Err(bad("user_id")),
    };
    if item.is_some() == user.is_some() {
        return Err(bad("item_id"));
    }
    let act = match body.get("action").and_then(Value::as_str) {
        Some(a @ ("void" | "restore")) => a,
        _ => return Err(bad("action")),
    };
    let n = note_field(body)?;
    let rb = match body.get("replaced_by") {
        None | Some(Value::Null) => None,
        Some(_) => Some(id_field(body, "replaced_by")?),
    };
    if rb.is_some() && (item.is_none() || act != "void") {
        return Err(bad("replaced_by"));
    }
    if let (Some(rb), Some(it)) = (&rb, &item) {
        let rc = rightful_check(x, rb, it, &[], &[])?;
        if !rc.ok {
            let bad_hashes: Vec<Value> = rc_json(&rc)["hashes"].as_array().unwrap().iter().filter(|h| h["ok"] == false).take(50).cloned().collect();
            return Err(PoolError::new(409, "rightful_item_invalid").with("reasons", json!(rc.reasons)).with("hashes", Value::Array(bad_hashes)).into());
        }
        x.conn.execute(
            "UPDATE void_items SET replaced_by = ?2, replaced_license = (SELECT license FROM items WHERE id = ?2), replaced_author_id = (SELECT author_id FROM items WHERE id = ?2) WHERE item_id = ?1",
            params![it, rb],
        )?;
    }
    let restore = act == "restore";
    let (scol, sid) = match (&item, &user) {
        (Some(i), _) => ("item_id", i.clone()),
        (_, Some(u)) => ("reader_id", u.clone()),
        _ => unreachable!(),
    };
    let rows: i64 = x.conn.query_row(&format!("SELECT COUNT(*) FROM served WHERE {scol} = ?1 AND void_at IS {}NULL", if restore { "NOT " } else { "" }), [&sid], |r| r.get(0))?;
    let tx = x.conn.unchecked_transaction()?;
    let who = match (&item, &user) {
        (Some(i), _) => Who::Item(i),
        (_, Some(u)) => Who::User(u),
        _ => unreachable!(),
    };
    void_evidence(&tx, who, x.now, restore, Some(admin), "admin", rb.as_deref())?;
    audit(&tx, &AuditEntry { actor_id: Some(admin), actor_kind: "admin", action: &format!("evidence.{act}"), item_id: item.as_deref(), target_user_id: user.as_deref(), detail: None, note: n.as_deref() }, x.now, None)?;
    auth_audit(&tx, Some(admin), "admin", &AuditFields { action: &format!("pool.evidence_{act}"), target_id: Some(item.as_deref().or(user.as_deref()).unwrap()), note: n.as_deref(), ..Default::default() }, x.now, None, false, &new_id())?;
    tx.commit()?;
    Ok((200, json!({ "ok": true, "rows": rows })))
}

/// POST /admin/pool/unblock: lifts one privacy block (a false positive); holds that only it caused are released. Within the grace window the bytes are
/// intact and the item is put back; after it, every copy that used the image is held as needs_reupload, flagged, and named in the answer.
pub fn privacy_unblock(x: &PoolCtx, store: &BlobStore, admin: &str, body: &Map<String, Value>) -> Res<(u16, Value)> {
    let kind = match body.get("kind").and_then(Value::as_str) {
        Some(k @ ("match" | "blob")) => k,
        _ => return Err(bad("hash")),
    };
    let hash = match body.get("hash").filter(|v| !v.is_null()).or_else(|| body.get("sha256")).and_then(Value::as_str) {
        Some(h) if is_block_key(h) => h.to_string(),
        _ => return Err(bad("hash")),
    };
    let n = note_field(body)?;
    let blk: Option<(Option<i64>, String)> = x
        .conn
        .query_row("SELECT MAX(bytes_purged_at), MIN(item_id) FROM privacy_blocks WHERE kind = ?1 AND (hash = ?2 OR qhash = ?2) HAVING COUNT(*) > 0", params![kind, hash], |r| Ok((r.get(0)?, r.get(1)?)))
        .optional()?;
    let Some((purged_at, blk_item)) = blk else { return Ok((404, json!({ "error": "not_found" }))) };
    let lost = purged_at.is_some();
    let needs: Vec<String> = if lost {
        let sql = format!("SELECT DISTINCT i.id FROM items i WHERE i.purged_at IS NULL AND {} ORDER BY i.id", current_serves_blob("i", |sha| format!("{sha} = ?1")));
        let mut st = x.conn.prepare(&sql)?;
        let v = st.query_map([&hash], |r| r.get(0))?.collect::<Result<_, _>>()?;
        v
    } else {
        vec![]
    };
    let tx = x.conn.unchecked_transaction()?;
    tx.execute("DELETE FROM privacy_blocks WHERE kind = ?1 AND (hash = ?2 OR qhash = ?2)", params![kind, hash])?;
    tx.execute("DELETE FROM privacy_bands WHERE qhash NOT IN (SELECT qhash FROM privacy_blocks WHERE qhash IS NOT NULL)", [])?;
    tx.execute("INSERT OR IGNORE INTO item_holds (item_id, reason, at, by, source) SELECT value, 'admin', ?1, ?2, 'needs_reupload' FROM json_each(?3)", params![x.now, admin, json!(needs).to_string()])?;
    tx.execute(
        "INSERT INTO admin_flags (id, kind, item_id, detail, at) SELECT ?1, 'needs_reupload', NULL, ?2, ?3 WHERE ?4 > 0",
        params![new_id(), json!({ "hash": hash, "items": needs }).to_string(), x.now, needs.len() as i64],
    )?;
    audit(&tx, &AuditEntry { actor_id: Some(admin), actor_kind: "admin", action: "privacy.unblock", item_id: None, target_user_id: None, detail: Some(json!({ "kind": kind, "hash": hash, "bytes_lost": lost, "needs_reupload": needs.len() })), note: n.as_deref() }, x.now, None)?;
    auth_audit(&tx, Some(admin), "admin", &AuditFields { action: "pool.privacy_unblock", target_id: Some(&hash), note: n.as_deref(), ..Default::default() }, x.now, None, false, &new_id())?;
    tx.commit()?;
    let released = release_orphan_holds(x, "privacy", "student_data")?;
    // The purged item's last own block lifted within its grace window: the item itself is put back.
    let left: bool = x.conn.query_row("SELECT EXISTS (SELECT 1 FROM privacy_blocks WHERE item_id = ?1)", [&blk_item], |r| r.get(0))?;
    let restored = !left && privacy_restore(x, store, &Actor { id: Some(admin), kind: "admin" }, &blk_item, n.as_deref())?;
    Ok((200, json!({ "ok": true, "released": released, "bytes_lost": lost, "needs_reupload": needs, "item_restored": restored })))
}

/// POST /admin/dmca/unblock: lifts an action's re-upload blocks (all, or one hash), upheld or not; holds that only they caused are released.
pub fn dmca_unblock(env: &SendEnv, admin: &str, body: &Map<String, Value>) -> Res<(u16, Value)> {
    let x = env.x;
    let aid = id_field(body, "action_id")?;
    let hash = match body.get("hash") {
        None => None,
        Some(Value::String(h)) if is_block_key(h) => Some(h.clone()),
        Some(_) => return Err(bad("hash")),
    };
    let n = note_field(body)?;
    let release_circles = match body.get("release_circle_versions") {
        None => false,
        Some(Value::Bool(b)) => *b,
        Some(_) => return Err(bad("release_circle_versions")),
    };
    let tx = x.conn.unchecked_transaction()?;
    let lifted = tx.execute("UPDATE blocked_hashes SET lifted_at = ?2 WHERE action_id = ?1 AND lifted_at IS NULL AND (?3 IS NULL OR hash = ?3 OR qhash = ?3)", params![aid, x.now, hash])?;
    audit(&tx, &AuditEntry { actor_id: Some(admin), actor_kind: "admin", action: "dmca.unblock", item_id: None, target_user_id: None, detail: Some(json!({ "action_id": aid, "hash": hash.clone().unwrap_or_else(|| "all".into()) })), note: n.as_deref() }, x.now, Some("changes() > 0"))?;
    auth_audit(&tx, Some(admin), "admin", &AuditFields { action: "pool.dmca_unblock", target_id: Some(&aid), note: n.as_deref(), ..Default::default() }, x.now, None, false, &new_id())?;
    tx.commit()?;
    let released = release_orphan_holds(x, &format!("dmca:{aid}"), "blocklist")?;
    let (cl, cr) = circle_unblock(env, &aid, release_circles, admin)?;
    Ok((200, json!({ "ok": true, "lifted": lifted, "released": released, "circle_flags_lifted": cl, "circle_versions_released": cr })))
}

/// POST /admin/pool/privacy-restore: an upheld appeal (or any wrong privacy decision) within the 14-day grace window: every block the item caused is lifted
/// and the item is put back.
pub fn privacy_restore_route(x: &PoolCtx, store: &BlobStore, admin: &str, body: &Map<String, Value>) -> Res<(u16, Value)> {
    let item_id = id_field(body, "item_id")?;
    let n = note_field(body)?;
    if privacy_restore(x, store, &Actor { id: Some(admin), kind: "admin" }, &item_id, n.as_deref())? {
        auth_line(x, admin, "pool.privacy_restore", Some(&item_id), n.as_deref(), None)?;
        Ok((200, json!({ "ok": true })))
    } else {
        Ok((409, json!({ "error": "not_restorable", "message": "Only a privacy removal within its 14-day grace window, or with an open appeal, can be restored." })))
    }
}

/// POST /admin/pool/purge: confirmed student data, purged immediately (no quarantine), image hashes blocklisted.
pub fn purge_route(x: &PoolCtx, store: &BlobStore, keys: &PoolKeys, admin: &str, body: &Map<String, Value>) -> Res<(u16, Value)> {
    let item_id = id_field(body, "item_id")?;
    let n = note_field(body)?;
    let plan = block_plan(x, AFFECTED_LIST, &item_id, &read_selection(Some(body))?, "privacy", None)?;
    // More affected items than one list: fails closed until the admin confirms the size (checked before the token is consumed).
    if plan.affected_total > plan.affected.len() as i64 && body.get("confirm_large") != Some(&Value::Bool(true)) {
        return Err(PoolError::new(428, "confirm_large_required").with("affected_total", json!(plan.affected_total)).with("message", json!("This block holds more items than are listed; confirm with confirm_large: true.")).into());
    }
    // The preview token is checked BEFORE anything is written: no audit row for a refused confirm.
    check_preview(x, keys, "privacy", &plan, body.get("confirm_preview").unwrap_or(&Value::Null), &bound(vec![("admin", json!(admin))]))?;
    if selected_match(&plan).is_empty() && selected_blobs(&plan).is_empty() {
        return Err(PoolError::new(400, "selection_required").with("message", json!("Tick the questions or images that contain student data.")).into());
    }
    auth_line(x, admin, "pool.purge_privacy", Some(&item_id), n.as_deref(), None)?;
    if privacy_purge(x, store, admin, &item_id, &plan)? {
        let held: Vec<&str> = plan.affected.iter().map(|a| a.item_id.as_str()).collect();
        Ok((200, json!({ "ok": true, "blocked": { "questions": plan.matches.iter().filter(|m| m.selected).count(), "keys": selected_match(&plan).len(), "images": selected_blobs(&plan).len() }, "held": held, "held_total": plan.affected_total })))
    } else {
        Ok((409, json!({ "error": "not_purgeable" })))
    }
}

/// GET /admin/dmca/{notices,actions,counter-notices}: the DMCA records, newest first, paged (`limit`, `cursor` = at:id).
pub fn list_dmca(x: &PoolCtx, which: &str, limit: Option<i64>, cursor: Option<&str>) -> Res<Value> {
    let sql = match which {
        "notices" => "SELECT id, received_at AS at, complainant_name, complainant_contact, work_desc, material_desc, status, recorded_by FROM dmca_notices",
        "actions" => {
            "SELECT id, disabled_at AS at, notice_id, item_id, author_id, counter_notice_id, restore_not_before, restore_by, court_action_at, restored_at, purged_at, outcome, rightful_item_id,
                    (SELECT json_object('at', t.at, 'terms_version', t.terms_version) FROM attestations t WHERE t.item_id = dmca_actions.item_id AND t.at <= dmca_actions.disabled_at ORDER BY t.at DESC LIMIT 1) AS attestation FROM dmca_actions"
        }
        "counter-notices" => "SELECT id, received_at AS at, action_id, name, address, phone, signature, material_desc, forwarded_at FROM dmca_counter_notices",
        _ => return Err(PoolError::new(404, "not_found").into()),
    };
    let limit = limit.filter(|l| *l != 0).unwrap_or(50).clamp(1, 200);
    let cur = cursor.and_then(|c| c.split_once(':')).filter(|(a, b)| !a.is_empty() && a.bytes().all(|c| c.is_ascii_digit()) && is_id(b)).and_then(|(a, b)| Some((a.parse::<i64>().ok()?, b.to_string())));
    let mut rs = rows(
        x.conn,
        &format!("SELECT * FROM ({sql}) WHERE (?1 IS NULL OR at < ?1 OR (at = ?1 AND id < ?2)) ORDER BY at DESC, id DESC LIMIT ?3"),
        &[&cur.as_ref().map(|c| c.0), &cur.as_ref().map(|c| c.1.clone()).unwrap_or_default(), &(limit + 1)],
    )?;
    let more = rs.len() as i64 > limit;
    rs.truncate(limit as usize);
    if which == "actions" {
        for r in &mut rs {
            if let Some(s) = r.get("attestation").and_then(Value::as_str).and_then(|s| serde_json::from_str::<Value>(s).ok()) {
                r.insert("attestation".into(), s);
            }
        }
    }
    let next = if more { rs.last().map(|l| json!(format!("{}:{}", l["at"], l["id"].as_str().unwrap_or("")))) } else { None };
    Ok(json!({ "items": rs, "next_cursor": next.unwrap_or(Value::Null) }))
}

/// POST /admin/pool/purge-held-copies: the copies a confirmed student-data purge holds, listed (no confirm), then removed in bulk (confirm: true).
pub fn purge_held_copies_route(x: &PoolCtx, store: &BlobStore, admin: &str, body: &Map<String, Value>) -> Res<Value> {
    let item_id = id_field(body, "item_id")?;
    let confirm = body.get("confirm") == Some(&Value::Bool(true));
    let r = purge_held_copies(x, store, admin, &item_id, confirm)?;
    if confirm {
        auth_line(x, admin, "pool.purge_held_copies", Some(&item_id), Some(&format!("{} removed", r.removed)), None)?;
    }
    Ok(json!({ "items": r.items, "total": r.total, "removed": r.removed, "remaining": r.remaining }))
}

/// POST /admin/pool/appeal-decision: an open appeal stops the purge until the admin decides here; unanswered for 60 days the item is restored (the cron).
pub fn appeal_decision(x: &PoolCtx, store: &BlobStore, admin: &str, body: &Map<String, Value>) -> Res<(u16, Value)> {
    let item_id = id_field(body, "item_id")?;
    let decision = match body.get("decision").and_then(Value::as_str) {
        Some(d @ ("restore" | "purge")) => d,
        _ => return Err(bad("decision")),
    };
    let n = note_field(body)?;
    Ok(match decide_appeal(x, store, &item_id, decision == "restore", &Actor { id: Some(admin), kind: "admin" }, n.as_deref())? {
        Decided::Ok => {
            auth_line(x, admin, &format!("pool.appeal_{decision}"), Some(&item_id), n.as_deref(), None)?;
            (200, json!({ "ok": true }))
        }
        Decided::OverLimits => (409, json!({ "error": "over_limits" })),
        Decided::NotFound => (404, json!({ "error": "not_found" })),
    })
}

/// POST /admin/pool/resolve-flag
pub fn resolve_flag(x: &PoolCtx, admin: &str, body: &Map<String, Value>) -> Res<(u16, Value)> {
    let fid = id_field(body, "flag_id")?;
    let tx = x.conn.unchecked_transaction()?;
    tx.execute("UPDATE admin_flags SET resolved_at = ?2, resolved_by = ?3 WHERE id = ?1 AND resolved_at IS NULL", params![fid, x.now, admin])?;
    auth_audit(&tx, Some(admin), "admin", &AuditFields { action: "pool.flag_resolve", target_id: Some(&fid), ..Default::default() }, x.now, None, false, &new_id())?;
    tx.commit()?;
    Ok((200, json!({ "ok": true })))
}

/// PUT /admin/pool/limits: sets one limit for an account (or `*` for everyone).
pub fn set_limit(x: &PoolCtx, admin: &str, body: &Map<String, Value>) -> Res<(u16, Value)> {
    let user = match body.get("user_id").and_then(Value::as_str) {
        Some(u) if u == "*" || is_uuid(u) => u.to_string(),
        _ => return Err(bad("user_id")),
    };
    let metric = match body.get("metric").and_then(Value::as_str) {
        Some(m) if default_limit(m).is_some() => m.to_string(),
        _ => return Err(bad("metric")),
    };
    let value = match body.get("value").and_then(|v| v.as_i64().or_else(|| v.as_f64().filter(|f| f.fract() == 0.0).map(|f| f as i64))) {
        Some(v) if (0..=1_000_000_000_000).contains(&v) => v,
        _ => return Err(bad("value")),
    };
    let tx = x.conn.unchecked_transaction()?;
    tx.execute(
        &format!("INSERT INTO pool_limits (user_id, metric, value, updated_at, updated_by) SELECT ?1, ?2, ?3, ?4, ?5 WHERE (?1 = '*' OR {}) ON CONFLICT (user_id, metric) DO UPDATE SET value = ?3, updated_at = ?4, updated_by = ?5", alive_sql("?1")),
        params![user, metric, value, x.now, admin],
    )?;
    let target = (user != "*").then_some(user.as_str());
    audit(&tx, &AuditEntry { actor_id: Some(admin), actor_kind: "admin", action: "limits.set", item_id: None, target_user_id: target, detail: Some(json!({ "metric": metric, "value": value, "default": default_limit(&metric) })), note: None }, x.now, None)?;
    auth_audit(&tx, Some(admin), "admin", &AuditFields { action: "pool.limits", target_id: target, detail: Some(json!({ "metric": metric, "value": value })), ..Default::default() }, x.now, None, false, &new_id())?;
    tx.commit()?;
    Ok((200, json!({ "ok": true })))
}

/// POST /admin/dmca/notices: one field or tick per 512(c)(3)(A) element; a notice missing one is stored as deficient.
pub fn create_notice(x: &PoolCtx, admin: &str, body: &Map<String, Value>) -> Res<(u16, Value)> {
    let text = |k: &str, max: usize| -> Res<String> { body.get(k).and_then(|v| clean_text(v, max)).ok_or_else(|| bad(k)) };
    let name = text("complainant_name", 200)?;
    let contact = text("complainant_contact", 500)?;
    let work = text("work_desc", 2000)?;
    let material = text("material_desc", 2000)?;
    let tick = |k: &str| body.get(k) == Some(&Value::Bool(true));
    let (sig, gf, pj) = (tick("has_signature"), tick("has_good_faith"), tick("has_perjury_stmt"));
    let nid = new_id();
    let status = if sig && gf && pj { "open" } else { "deficient" };
    let tx = x.conn.unchecked_transaction()?;
    tx.execute(
        "INSERT INTO dmca_notices (id, received_at, complainant_name, complainant_contact, work_desc, material_desc, has_signature, has_good_faith, has_perjury_stmt, status, recorded_by)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
        params![nid, x.now, name, contact, work, material, sig as i64, gf as i64, pj as i64, status, admin],
    )?;
    audit(&tx, &AuditEntry { actor_id: Some(admin), actor_kind: "admin", action: "dmca.notice", item_id: None, target_user_id: None, detail: Some(json!({ "notice_id": nid, "status": status })), note: None }, x.now, None)?;
    auth_audit(&tx, Some(admin), "admin", &AuditFields { action: "pool.dmca_notice", target_id: Some(&nid), detail: Some(json!({ "status": status })), ..Default::default() }, x.now, None, false, &new_id())?;
    tx.commit()?;
    Ok((201, json!({ "notice_id": nid, "status": status })))
}

#[cfg(test)]
#[path = "adminblock_tests.rs"]
mod tests;
