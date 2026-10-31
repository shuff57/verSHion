//! The admin's circle routes (worker/src/pool/circleadmin.ts), as HTTP-free cores behind the admin guard (the route layer checks the admin
//! role and the sudo token for writes and hands the cores the admin's id). Every write is audited in `pool_audit` and `auth_audit`; no admin
//! route makes an admin a member. An admin opens circle CONTENT only on a basis: a member reported the bank, a recorded notice names it, a
//! student-data block matches it, or a copyright re-upload block does. Writes need a reason of 10 or more characters.
use super::circlebanks::not_retained;
use super::circlebase::CIRCLE_UNDO_MS;
use super::circlehide::with_stored_full;
use super::circlesendmeta::{ScanForm, images_at, privacy_scan};
use super::circlesendprep::SendEnv;
use super::circlesendsql::*;
use super::circlesql::*;
use super::ctx::{AuditEntry, Fail, PoolCtx, PoolError, Res, audit};
use super::qjson::Json;
use super::sqlfrag::{is_id, new_id};
use crate::auth::audit::{AuditFields, audit as auth_audit};
use rusqlite::types::ValueRef;
use rusqlite::{OptionalExtension, Row, params};
use serde_json::{Map, Value, json};

pub const CIRCLE_ADMIN_PAGE: usize = 200;
/// The versions one unblock releases at most (each is a body check); the rest keep their `block_lifted` flags.
pub const UNBLOCK_RELEASE_MAX: usize = 50;

fn bad(field: &str) -> Fail {
    PoolError::new(400, "invalid_request").with("field", json!(field)).into()
}
fn id_of(v: Option<&Value>, field: &str) -> Res<String> {
    match v.and_then(Value::as_str) {
        Some(s) if is_id(s) => Ok(s.to_string()),
        _ => Err(bad(field)),
    }
}
fn reason_of(v: Option<&Value>) -> Res<String> {
    match v.and_then(|v| crate::auth::validate::clean_text(v, 1000)) {
        Some(s) if s.chars().count() >= 10 => Ok(s),
        _ => Err(PoolError::new(400, "invalid_request").with("field", json!("note")).with("message", json!("Say why (at least 10 characters).")).into()),
    }
}
fn version_of(v: Option<&Value>) -> Res<Option<i64>> {
    match v {
        None => Ok(None),
        Some(v) => match v.as_i64() {
            Some(n) if n >= 1 => Ok(Some(n)),
            _ => Err(bad("version")),
        },
    }
}
fn row_json(r: &Row) -> rusqlite::Result<Map<String, Value>> {
    let mut m = Map::new();
    for (i, name) in r.as_ref().column_names().iter().enumerate() {
        let v = match r.get_ref(i)? {
            ValueRef::Null => Value::Null,
            ValueRef::Integer(n) => json!(n),
            ValueRef::Real(f) => json!(f),
            ValueRef::Text(t) => Value::String(String::from_utf8_lossy(t).into_owned()),
            ValueRef::Blob(_) => Value::Null,
        };
        m.insert((*name).to_string(), v);
    }
    Ok(m)
}
fn rows(conn: &rusqlite::Connection, sql: &str, p: &[&dyn rusqlite::ToSql]) -> rusqlite::Result<Vec<Map<String, Value>>> {
    conn.prepare(sql)?.query_map(p, row_json)?.collect()
}
/// `^(\d{1,16}):([0-9a-f]{32})$`
fn cursor(after: Option<&str>) -> Option<(i64, String)> {
    let (a, b) = after?.split_once(':')?;
    (!a.is_empty() && a.len() <= 16 && a.bytes().all(|c| c.is_ascii_digit()) && is_id(b)).then(|| (a.parse().unwrap(), b.to_string()))
}
fn parse_json_col(m: &mut Map<String, Value>, k: &str) {
    let v = m.get(k).and_then(Value::as_str).and_then(|s| serde_json::from_str::<Value>(s).ok()).unwrap_or_else(|| json!([]));
    m.insert(k.into(), v);
}
/// The admin's own audit lines for one action: `pool_audit` (inside the circle) and `auth_audit` (the admin trail).
fn both(conn: &rusqlite::Connection, admin: &str, now: i64, pool: &AuditEntry, auth_action: &str, target: &str, note: Option<&str>, detail: Option<Value>) -> Res<()> {
    audit(conn, pool, now, None)?;
    auth_audit(conn, Some(admin), "admin", &AuditFields { action: auth_action, target_id: Some(target), note, detail, ..Default::default() }, now, None, false, &new_id())?;
    Ok(())
}

/// A student-data block matches a question (key-free hash) or an image of bank `?1`; or a send's own check matched it.
const PRIVACY_MATCH_SQL: &str = "(EXISTS (SELECT 1 FROM circle_qkeys k JOIN privacy_blocks pb ON pb.kind = 'match' AND pb.hash = k.match_hash WHERE k.bank_id = ?1)
  OR EXISTS (SELECT 1 FROM circle_versions v JOIN circle_version_blobs vb ON vb.version_id = v.id JOIN privacy_blocks pb ON pb.kind = 'blob' AND pb.hash = vb.sha256 WHERE v.bank_id = ?1)
  OR EXISTS (SELECT 1 FROM circle_holds ph WHERE ph.bank_id = ?1 AND ph.source IN ('privacy', 'privacy_near'))
  OR EXISTS (SELECT 1 FROM admin_flags af WHERE af.kind = 'privacy_text_match' AND af.item_id = ?1))";
/// A copyright re-upload block matches bank `?1` (`?2` now): a send's check held it, or an active block holds a question it held.
const COPYRIGHT_MATCH_SQL: &str = "(EXISTS (SELECT 1 FROM circle_holds ch WHERE ch.bank_id = ?1 AND ch.source = 'blocklist')
  OR EXISTS (SELECT 1 FROM admin_flags cf WHERE cf.kind = 'blocklist' AND cf.item_id = ?1)
  OR EXISTS (SELECT 1 FROM circle_qkeys k JOIN blocked_hashes bh ON bh.kind = 'match' AND bh.hash = k.match_hash WHERE k.bank_id = ?1 AND bh.lifted_at IS NULL AND (bh.expires_at IS NULL OR bh.expires_at > ?2)))";

/// Version `?1` of bank `?2` was withheld for copyright only: its own questions and images (block keys `?4`, image hashes `?5`) match no
/// confirmed student data now, no report or administrator holds the bank, no report is open on it, and every open send flag on the version is
/// a `blocklist` flag whose takedowns are all lifted or expired (`?3` now).
fn copyright_only_sql() -> String {
    format!(
        "(NOT {}
  AND NOT {}
  AND NOT EXISTS (SELECT 1 FROM circle_reports cr WHERE cr.bank_id = ?2 AND cr.resolved_at IS NULL)
  AND NOT EXISTS (SELECT 1 FROM admin_flags of WHERE of.publish_id = ?1 AND of.kind IN {SEND_FLAG_KINDS} AND of.resolved_at IS NULL
                    AND (of.kind <> 'blocklist' OR NOT json_valid(of.detail)
                         OR EXISTS (SELECT 1 FROM json_each(of.detail, '$.actions') a JOIN blocked_hashes bh ON bh.action_id = a.value WHERE bh.lifted_at IS NULL AND (bh.expires_at IS NULL OR bh.expires_at > ?3)))))",
        privacy_sql("?4", "?5"),
        blocking_hold_sql("?2")
    )
}

/// After an administrator lifts a takedown's re-upload blocks, the open `blocklist` flags of circle versions that only that takedown's (or other
/// now inactive) blocks held are marked `block_lifted: true`; with `release` their versions are released in the same call (readable again, the
/// flag resolved, a head's copyright hold lifted, the listing refreshed), never one also withheld for student data. Returns (lifted, released).
pub fn circle_unblock(env: &SendEnv, action_id: &str, release: bool, admin_id: &str) -> Res<(i64, i64)> {
    let x = env.x;
    let now = x.now;
    struct F {
        id: String,
        bank_id: String,
        version_id: String,
        flag: bool,
    }
    let pick = |sql: &str, flag: bool| -> rusqlite::Result<Vec<F>> {
        x.conn.prepare(sql)?.query_map(params![action_id, now], |r| Ok(F { id: r.get(0)?, bank_id: r.get(1)?, version_id: r.get(2)?, flag })).and_then(|it| it.collect())
    };
    let cand = pick(
        "SELECT f.id, f.item_id AS bank_id, f.publish_id AS version_id FROM admin_flags f
          WHERE f.kind = 'blocklist' AND f.resolved_at IS NULL AND json_valid(f.detail) AND f.detail ->> '$.circle_id' IS NOT NULL
            AND EXISTS (SELECT 1 FROM json_each(f.detail, '$.actions') a WHERE a.value = ?1)
            AND NOT EXISTS (SELECT 1 FROM json_each(f.detail, '$.actions') a JOIN blocked_hashes bh ON bh.action_id = a.value WHERE bh.lifted_at IS NULL AND (bh.expires_at IS NULL OR bh.expires_at > ?2))",
        true,
    )?;
    // Versions withheld past the sender's day of flags, named on their bank's cap flag with the takedowns that matched them.
    let caps = pick(
        "SELECT f.id, f.item_id AS bank_id, json_extract(v.value, '$.version_id') AS version_id FROM admin_flags f, json_each(f.detail, '$.versions') v
          WHERE f.kind = 'cap' AND f.resolved_at IS NULL AND json_valid(f.detail) AND f.detail ->> '$.metric' = 'circle_send_flags'
            AND json_extract(v.value, '$.student_data') = 0
            AND EXISTS (SELECT 1 FROM json_each(v.value, '$.actions') a WHERE a.value = ?1)
            AND NOT EXISTS (SELECT 1 FROM json_each(v.value, '$.actions') a JOIN blocked_hashes bh ON bh.action_id = a.value WHERE bh.lifted_at IS NULL AND (bh.expires_at IS NULL OR bh.expires_at > ?2))",
        false,
    )?;
    if cand.is_empty() && caps.is_empty() {
        return Ok((0, 0));
    }
    let mut ids: Vec<&String> = cand.iter().chain(caps.iter()).map(|f| &f.id).collect();
    ids.sort();
    ids.dedup();
    x.conn.execute("UPDATE admin_flags SET detail = json_set(detail, '$.block_lifted', json('true')) WHERE id IN (SELECT value FROM json_each(?1))", [serde_json::to_string(&ids).unwrap()])?;
    let mut released = 0;
    if release {
        for f in cand.iter().chain(caps.iter()).take(UNBLOCK_RELEASE_MAX) {
            // Only a version the lifted takedown alone withheld. Its body is checked again now, as a send's is: confirmed student data or another
            // active block keeps it withheld for the administrator's own release; so does a body that is gone or unreadable.
            let v: Option<(String, String)> = x
                .conn
                .query_row("SELECT full_key, committer_id FROM circle_versions WHERE id = ?1 AND withheld_at IS NOT NULL AND full_key <> ''", [&f.version_id], |r| Ok((r.get(0)?, r.get::<_, Option<String>>(1)?.unwrap_or_default())))
                .optional()?;
            let Some((full_key, by)) = v else { continue };
            let scanned = with_stored_full(env.core, env.store, &full_key, |qs, mh| {
                let js: Vec<Json> = qs.iter().map(|q| Json::parse(q.to_string().as_bytes()).unwrap_or(Json::Null)).collect();
                privacy_scan(x, env.fp, &[ScanForm { img_at: images_at(&js), qs: js, mh: mh.to_vec() }], &by)
            })?;
            let Some(sc) = scanned.transpose()? else { continue };
            if sc.exact.len() + sc.near.len() + sc.copyright.len() > 0 {
                continue;
            }
            let tx = x.conn.unchecked_transaction()?;
            let n = tx.execute(
                &format!("UPDATE circle_versions SET withheld_at = NULL WHERE id = ?1 AND withheld_at IS NOT NULL AND {}", copyright_only_sql()),
                params![f.version_id, f.bank_id, now, serde_json::to_string(&sc.keys).unwrap(), serde_json::to_string(&sc.shas).unwrap()],
            )?;
            let mut d = json!({ "bank_id": f.bank_id, "version_id": f.version_id, "via": "unblock", "action_id": action_id });
            if !f.flag {
                d["past_flags"] = json!(true);
            }
            audit(&tx, &AuditEntry { actor_id: Some(admin_id), actor_kind: "admin", action: "circle.admin_version_release", item_id: None, target_user_id: None, detail: Some(d), note: None }, now, Some("changes() = 1"))?;
            if f.flag {
                tx.execute(
                    "UPDATE admin_flags SET resolved_at = ?2, resolved_by = ?3 WHERE id = ?1 AND resolved_at IS NULL AND EXISTS (SELECT 1 FROM circle_versions v WHERE v.id = ?4 AND v.withheld_at IS NULL)",
                    params![f.id, now, admin_id, f.version_id],
                )?;
            }
            tx.execute(
                "DELETE FROM circle_holds WHERE bank_id = ?1 AND source = 'blocklist'
                   AND EXISTS (SELECT 1 FROM circle_banks b JOIN circle_versions v ON v.id = b.head_version_id WHERE b.id = ?1 AND v.id = ?2 AND v.withheld_at IS NULL)",
                params![f.bank_id, f.version_id],
            )?;
            tx.execute(&listing_refresh_sql("EXISTS (SELECT 1 FROM circle_versions rv WHERE rv.id = ?2 AND rv.withheld_at IS NULL)"), params![f.bank_id, f.version_id])?;
            tx.commit()?;
            if n == 1 {
                released += 1;
            }
        }
    }
    Ok((cand.len() as i64, released))
}

/// GET /admin/pool/circle-reports?after=: open circle reports, oldest first, paged (`after` = the last one's at:id).
pub fn list_reports(x: &PoolCtx, after: Option<&str>) -> Res<Value> {
    let cur = cursor(after);
    let mut rs = rows(
        x.conn,
        "SELECT r.id, r.circle_id, r.bank_id, r.version, r.reason, r.note, r.at, r.reporter_id, ru.display_name AS reporter_name,
                ci.name AS circle_name, b.title AS bank_title, b.author_id, (SELECT json_group_array(h.reason) FROM circle_holds h WHERE h.bank_id = r.bank_id) AS holds
           FROM circle_reports r JOIN circles ci ON ci.id = r.circle_id JOIN circle_banks b ON b.id = r.bank_id LEFT JOIN users ru ON ru.id = r.reporter_id
          WHERE r.resolved_at IS NULL AND (?1 IS NULL OR r.at > ?1 OR (r.at = ?1 AND r.id > ?2)) ORDER BY r.at, r.id LIMIT ?3",
        &[&cur.as_ref().map(|c| c.0), &cur.as_ref().map(|c| c.1.clone()).unwrap_or_default(), &(CIRCLE_ADMIN_PAGE as i64 + 1)],
    )?;
    let more = rs.len() > CIRCLE_ADMIN_PAGE;
    rs.truncate(CIRCLE_ADMIN_PAGE);
    for r in &mut rs {
        parse_json_col(r, "holds");
    }
    let next = if more { rs.last().map(|l| json!(format!("{}:{}", l["at"], l["id"].as_str().unwrap_or("")))) } else { None };
    Ok(json!({ "reports": rs, "next_cursor": next.unwrap_or(Value::Null) }))
}

/// GET /admin/pool/circles?user_id=&after=: circles, newest first, paged (`after` = created_at:id); `user_id` the circles an account is (or was) in.
pub fn list_circles(x: &PoolCtx, user_id: Option<&str>, after: Option<&str>) -> Res<Value> {
    if let Some(u) = user_id {
        if u.len() != 36 || !u.bytes().all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f' | b'-')) {
            return Err(bad("user_id"));
        }
    }
    let cur = cursor(after);
    let mut rs = rows(
        x.conn,
        "SELECT ci.id, ci.name, ci.created_by, ci.created_at, ci.closed_at, ci.close_reason, ci.purge_after, ci.purged_at,
                (SELECT COUNT(*) FROM circle_members m WHERE m.circle_id = ci.id AND m.left_at IS NULL) AS members,
                (SELECT COUNT(*) FROM circle_banks b WHERE b.circle_id = ci.id AND b.purged_at IS NULL) AS banks,
                (SELECT m.user_id FROM circle_members m WHERE m.circle_id = ci.id AND m.role = 'owner' AND m.left_at IS NULL) AS owner_id
           FROM circles ci
          WHERE (?1 IS NULL OR ci.id IN (SELECT circle_id FROM circle_members WHERE user_id = ?1))
            AND (?2 IS NULL OR ci.created_at < ?2 OR (ci.created_at = ?2 AND ci.id < ?3))
          ORDER BY ci.created_at DESC, ci.id DESC LIMIT ?4",
        &[&user_id, &cur.as_ref().map(|c| c.0), &cur.as_ref().map(|c| c.1.clone()).unwrap_or_default(), &(CIRCLE_ADMIN_PAGE as i64 + 1)],
    )?;
    let more = rs.len() > CIRCLE_ADMIN_PAGE;
    rs.truncate(CIRCLE_ADMIN_PAGE);
    let next = if more { rs.last().map(|l| json!(format!("{}:{}", l["created_at"], l["id"].as_str().unwrap_or("")))) } else { None };
    Ok(json!({ "circles": rs, "next_cursor": next.unwrap_or(Value::Null) }))
}

/// GET /admin/pool/circle?id=: metadata (members past and present, banks with holds and reports, codes' states). Audited.
pub fn circle_detail(x: &PoolCtx, admin: &str, id: Option<&str>) -> Res<(u16, Value)> {
    let cid = id_of(id.map(|s| Value::String(s.into())).as_ref(), "id")?;
    let circle = rows(x.conn, "SELECT * FROM circles WHERE id = ?1", &[&cid])?.into_iter().next();
    let Some(circle) = circle else { return Ok((404, json!({ "error": "not_found" }))) };
    let members = rows(x.conn, "SELECT m.user_id, u.display_name, m.role, m.joined_at, m.join_seq, m.left_at, m.left_reason, m.invite_id FROM circle_members m LEFT JOIN users u ON u.id = m.user_id WHERE m.circle_id = ?1 ORDER BY m.join_seq", &[&cid])?;
    let mut banks = rows(
        x.conn,
        "SELECT b.id, b.title, b.author_id, b.license, b.head_version, b.question_count, b.has_images, b.anonymized, b.created_at, b.updated_at, b.removed_at, b.removed_reason, b.purge_after, b.purged_at,
                (SELECT json_group_array(json_object('reason', h.reason, 'source', h.source, 'at', h.at)) FROM circle_holds h WHERE h.bank_id = b.id) AS holds,
                (SELECT COUNT(*) FROM circle_reports r WHERE r.bank_id = b.id AND r.resolved_at IS NULL) AS open_reports
           FROM circle_banks b WHERE b.circle_id = ?1 ORDER BY b.created_at",
        &[&cid],
    )?;
    for b in &mut banks {
        parse_json_col(b, "holds");
    }
    let invites = rows(x.conn, "SELECT id, role, created_by, created_at, max_uses, uses, expires_at, revoked_at FROM circle_invites WHERE circle_id = ?1 ORDER BY created_at DESC LIMIT 200", &[&cid])?;
    let tx = x.conn.unchecked_transaction()?;
    both(&tx, admin, x.now, &AuditEntry { actor_id: Some(admin), actor_kind: "admin", action: "circle.admin_open", item_id: Some(&cid), ..Default::default() }, "pool.circle_open", &cid, None, None)?;
    tx.commit()?;
    Ok((200, json!({ "circle": circle, "members": members, "banks": banks, "invites": invites })))
}

/// GET /admin/pool/circle-bank?id=&version=&basis=report|notice|privacy|copyright[&notice_id=]: a kept version's body. Audited.
pub fn bank_view(x: &PoolCtx, store: &crate::blobs::BlobStore, admin: &str, q: &Map<String, Value>) -> Res<(u16, Value)> {
    let bid = id_of(q.get("id"), "id")?;
    let basis = q.get("basis").and_then(Value::as_str);
    let b: Option<(String, i64, String, String, String)> = x
        .conn
        .query_row("SELECT circle_id, head_version, title, author_id, license FROM circle_banks WHERE id = ?1", [&bid], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?)))
        .optional()?;
    let Some((circle_id, head_version, title, author_id, license)) = b else { return Ok((404, json!({ "error": "not_found" }))) };
    let mut notice_id: Option<String> = None;
    let ok: bool = match basis {
        Some("report") => x.conn.query_row("SELECT EXISTS (SELECT 1 FROM circle_reports WHERE bank_id = ?1)", [&bid], |r| r.get(0))?,
        Some("notice") => {
            let n = id_of(q.get("notice_id"), "notice_id")?;
            let e = x.conn.query_row("SELECT EXISTS (SELECT 1 FROM dmca_notices WHERE id = ?1)", [&n], |r| r.get(0))?;
            notice_id = Some(n);
            e
        }
        Some("privacy") => x.conn.query_row(&format!("SELECT {PRIVACY_MATCH_SQL}"), [&bid], |r| r.get(0))?,
        Some("copyright") => x.conn.query_row(&format!("SELECT {COPYRIGHT_MATCH_SQL}"), params![bid, x.now], |r| r.get(0))?,
        _ => return Err(PoolError::new(400, "invalid_request").with("field", json!("basis")).with("allowed", json!(["report", "notice", "privacy", "copyright"])).into()),
    };
    if !ok {
        return Ok((403, json!({ "error": "basis_required", "message": "Circle content is opened only when a member reported it, a recorded copyright notice names it, a copyright re-upload block or a student-data block matches it." })));
    }
    let want = match q.get("version") {
        None => head_version,
        Some(v) => match v.as_str() {
            Some(s) if !s.is_empty() && s.len() <= 9 && !s.starts_with('0') && s.bytes().all(|c| c.is_ascii_digit()) => s.parse().unwrap(),
            _ => return Err(bad("version")),
        },
    };
    type V = (String, i64, String, String, Option<i64>, Option<String>, Option<String>, String);
    let v: Option<V> = x
        .conn
        .query_row(
            "SELECT id, version, status, full_key, committed_at, message, committer_id, license FROM circle_versions WHERE bank_id = ?1 AND version = ?2",
            params![bid, want],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?, r.get(5)?, r.get(6)?, r.get(7)?)),
        )
        .optional()?;
    let readable = v.as_ref().filter(|v| (v.2 == "current" || v.2 == "previous") && !v.3.is_empty());
    let Some((vid, version, status, full_key, committed_at, message, committer_id, vlicense)) = readable.cloned() else {
        // Evidence past the bound was pruned; what its flags recorded is what is left of it.
        let ev: Vec<(String, String, i64, Option<i64>, String)> = match &v {
            Some(v) => x
                .conn
                .prepare(&format!("SELECT id, kind, at, resolved_at, detail FROM admin_flags WHERE publish_id = ?1 AND kind IN {SEND_FLAG_KINDS} AND json_valid(detail) AND detail ->> '$.evidence_pruned' = 1 ORDER BY at, id LIMIT 10"))?
                .query_map([&v.0], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?)))?
                .collect::<Result<_, _>>()?,
            None => vec![],
        };
        let mut e = not_retained(x, &bid, head_version)?.body();
        if !ev.is_empty() {
            e["evidence_pruned"] = json!(true);
            e["message"] = json!("This version's text was deleted to keep the circle's storage bounded (only the newest flagged versions are kept); its flags keep the positions, the matching block hashes and the takedowns.");
            e["flags"] = json!(ev.iter().map(|f| json!({ "flag_id": f.0, "kind": f.1, "at": f.2, "resolved_at": f.3, "detail": serde_json::from_str::<Value>(&f.4).unwrap_or(Value::Null) })).collect::<Vec<_>>());
        }
        e["error"] = json!("version_not_retained");
        return Ok((404, e));
    };
    let mut detail = json!({ "bank_id": bid, "version": version, "basis": basis });
    if let Some(n) = &notice_id {
        detail["notice_id"] = json!(n);
    }
    let tx = x.conn.unchecked_transaction()?;
    both(&tx, admin, x.now, &AuditEntry { actor_id: Some(admin), actor_kind: "admin", action: "circle.admin_view", item_id: Some(&circle_id), detail: Some(detail.clone()), ..Default::default() }, "pool.circle_view", &bid, None, Some(detail))?;
    tx.commit()?;
    let payload = store.get(&full_key).ok().flatten().and_then(|b| serde_json::from_slice::<Value>(&b).ok()).unwrap_or(Value::Null);
    let _ = (status.as_str(), vid);
    Ok((
        200,
        json!({
            "bank": { "id": bid, "circle_id": circle_id, "head_version": head_version, "title": title, "author_id": author_id, "license": license },
            "version": { "version": version, "status": status, "committer_id": committer_id, "committed_at": committed_at, "message": message, "license": vlicense },
            "payload": payload,
        }),
    ))
}

/// POST /admin/pool/circle-evidence-captured {bank_id, version?, note}: the administrator has the case file. Each open student-data and copyright
/// flag stays open for the decision and is marked `evidence_captured`; it no longer keeps its version's body or the bank from a trim, a prune
/// or a purge.
pub fn evidence_captured(x: &PoolCtx, admin: &str, body: &Map<String, Value>) -> Res<(u16, Value)> {
    let bid = id_of(body.get("bank_id"), "bank_id")?;
    let n = reason_of(body.get("note"))?;
    let v = version_of(body.get("version"))?;
    let Some(circle_id): Option<String> = x.conn.query_row("SELECT circle_id FROM circle_banks WHERE id = ?1", [&bid], |r| r.get(0)).optional()? else { return Ok((404, json!({ "error": "not_found" }))) };
    let tx = x.conn.unchecked_transaction()?;
    let flags = tx.execute(
        &format!(
            "UPDATE admin_flags SET detail = json_set(detail, '$.evidence_captured', json('true'), '$.evidence_captured_at', ?2, '$.evidence_captured_by', ?3)
              WHERE item_id = ?1 AND kind IN {SEND_FLAG_KINDS} AND resolved_at IS NULL AND json_valid(detail) AND COALESCE(detail ->> '$.evidence_captured', 0) = 0
                AND (?4 IS NULL OR detail ->> '$.version' = ?4)"
        ),
        params![bid, x.now, admin, v],
    )?;
    let d = json!({ "bank_id": bid }).as_object().cloned().map(|mut o| {
        if let Some(v) = v {
            o.insert("version".into(), json!(v));
        }
        Value::Object(o)
    });
    both(&tx, admin, x.now, &AuditEntry { actor_id: Some(admin), actor_kind: "admin", action: "circle.admin_evidence_captured", item_id: Some(&circle_id), detail: d, note: Some(&n), ..Default::default() }, "pool.circle_evidence_captured", &bid, Some(&n), None)?;
    tx.commit()?;
    Ok((200, json!({ "ok": true, "flags": flags })))
}

/// POST /admin/pool/circle-hold {bank_id, note}: hidden from every member but its author until released.
pub fn hold(x: &PoolCtx, admin: &str, body: &Map<String, Value>) -> Res<(u16, Value)> {
    let bid = id_of(body.get("bank_id"), "bank_id")?;
    let n = reason_of(body.get("note"))?;
    let Some(circle_id): Option<String> = x.conn.query_row("SELECT circle_id FROM circle_banks WHERE id = ?1 AND purged_at IS NULL", [&bid], |r| r.get(0)).optional()? else { return Ok((404, json!({ "error": "not_found" }))) };
    let tx = x.conn.unchecked_transaction()?;
    let ins = tx.execute("INSERT OR IGNORE INTO circle_holds (bank_id, reason, source, at, by) VALUES (?1, 'admin', 'manual', ?2, ?3)", params![bid, x.now, admin])?;
    both(&tx, admin, x.now, &AuditEntry { actor_id: Some(admin), actor_kind: "admin", action: "circle.admin_hold", item_id: Some(&circle_id), detail: Some(json!({ "bank_id": bid })), note: Some(&n), ..Default::default() }, "pool.circle_hold", &bid, Some(&n), None)?;
    tx.commit()?;
    Ok((200, json!({ "ok": true, "changed": ins == 1 })))
}

/// POST /admin/pool/circle-release {bank_id, note}: every hold of the bank goes and its open reports are dismissed (a version withheld for
/// student data stays withheld). {bank_id, version, note} releases THAT withheld version instead.
pub fn release(x: &PoolCtx, admin: &str, body: &Map<String, Value>) -> Res<(u16, Value)> {
    let bid = id_of(body.get("bank_id"), "bank_id")?;
    let n = reason_of(body.get("note"))?;
    let Some(circle_id): Option<String> = x.conn.query_row("SELECT circle_id FROM circle_banks WHERE id = ?1", [&bid], |r| r.get(0)).optional()? else { return Ok((404, json!({ "error": "not_found" }))) };
    let now = x.now;
    let tx = x.conn.unchecked_transaction()?;
    if let Some(v) = version_of(body.get("version"))? {
        let up = tx.execute("UPDATE circle_versions SET withheld_at = NULL WHERE bank_id = ?1 AND version = ?2 AND withheld_at IS NOT NULL", params![bid, v])?;
        tx.execute(
            &format!("UPDATE admin_flags SET resolved_at = ?3, resolved_by = ?4 WHERE kind IN {SEND_FLAG_KINDS} AND resolved_at IS NULL AND publish_id = (SELECT id FROM circle_versions WHERE bank_id = ?1 AND version = ?2) AND changes() = 1"),
            params![bid, v, now, admin],
        )?;
        tx.execute(
            &format!(
                "DELETE FROM circle_holds WHERE bank_id = ?1 AND source IN {SEND_HOLD_SOURCES} AND EXISTS (SELECT 1 FROM circle_banks hb WHERE hb.id = ?1 AND hb.head_version = ?2)
                   AND EXISTS (SELECT 1 FROM circle_versions hv WHERE hv.bank_id = ?1 AND hv.version = ?2 AND hv.withheld_at IS NULL)"
            ),
            params![bid, v],
        )?;
        // The bank's listing again from its newest version that is not withheld, which may be this one now.
        tx.execute(&listing_refresh_sql("EXISTS (SELECT 1 FROM circle_versions rv WHERE rv.bank_id = ?1 AND rv.version = ?2 AND rv.withheld_at IS NULL)"), params![bid, v])?;
        both(&tx, admin, now, &AuditEntry { actor_id: Some(admin), actor_kind: "admin", action: "circle.admin_version_release", item_id: Some(&circle_id), detail: Some(json!({ "bank_id": bid, "version": v })), note: Some(&n), ..Default::default() }, "pool.circle_version_release", &bid, Some(&n), Some(json!({ "version": v })))?;
        tx.commit()?;
        return Ok(if up == 1 { (200, json!({ "ok": true, "released_version": v })) } else { (409, json!({ "error": "not_withheld", "message": "That version is not withheld." })) });
    }
    let del = tx.execute("DELETE FROM circle_holds WHERE bank_id = ?1", [&bid])?;
    let rep = tx.execute("UPDATE circle_reports SET resolved_at = ?2, resolved_by = ?3, action = 'dismissed' WHERE bank_id = ?1 AND resolved_at IS NULL", params![bid, now, admin])?;
    // And the student-data matches a send raised for it: the admin has decided; except those of versions still withheld.
    tx.execute(
        &format!(
            "UPDATE admin_flags SET resolved_at = ?2, resolved_by = ?3 WHERE kind IN {SEND_FLAG_KINDS} AND item_id = ?1 AND resolved_at IS NULL
               AND NOT EXISTS (SELECT 1 FROM circle_versions wv WHERE wv.id = admin_flags.publish_id AND wv.withheld_at IS NOT NULL)"
        ),
        params![bid, now, admin],
    )?;
    both(&tx, admin, now, &AuditEntry { actor_id: Some(admin), actor_kind: "admin", action: "circle.admin_release", item_id: Some(&circle_id), detail: Some(json!({ "bank_id": bid })), note: Some(&n), ..Default::default() }, "pool.circle_release", &bid, Some(&n), None)?;
    tx.commit()?;
    Ok((200, json!({ "ok": true, "released": del, "dismissed": rep })))
}

/// POST /admin/pool/circle-remove {bank_id, note}: removed now, purged after 14 days; its open reports are resolved as removed, its holds go.
pub fn remove_bank(x: &PoolCtx, admin: &str, body: &Map<String, Value>) -> Res<(u16, Value)> {
    let bid = id_of(body.get("bank_id"), "bank_id")?;
    let n = reason_of(body.get("note"))?;
    let Some(circle_id): Option<String> = x.conn.query_row("SELECT circle_id FROM circle_banks WHERE id = ?1", [&bid], |r| r.get(0)).optional()? else { return Ok((404, json!({ "error": "not_found" }))) };
    let now = x.now;
    let tx = x.conn.unchecked_transaction()?;
    let upd = tx.execute(
        "UPDATE circle_banks SET removed_at = COALESCE(removed_at, ?2), removed_reason = 'admin', purge_after = ?3, updated_at = ?2
          WHERE id = ?1 AND purged_at IS NULL AND (removed_reason IS NULL OR removed_reason IN ('author_removed', 'owner_removed', 'account_deleted', 'circle_closed'))",
        params![bid, now, now + CIRCLE_UNDO_MS],
    )?;
    tx.execute("UPDATE circle_reports SET resolved_at = ?2, resolved_by = ?3, action = 'removed' WHERE bank_id = ?1 AND resolved_at IS NULL AND changes() = 1", params![bid, now, admin])?;
    both(&tx, admin, now, &AuditEntry { actor_id: Some(admin), actor_kind: "admin", action: "circle.admin_bank_remove", item_id: Some(&circle_id), detail: Some(json!({ "bank_id": bid })), note: Some(&n), ..Default::default() }, "pool.circle_remove", &bid, Some(&n), None)?;
    // The admin decided: its holds go with the decision, so the purge follows in 14 days.
    tx.execute("DELETE FROM circle_holds WHERE bank_id = ?1 AND EXISTS (SELECT 1 FROM circle_banks WHERE id = ?1 AND removed_reason = 'admin')", [&bid])?;
    tx.execute(&format!("UPDATE admin_flags SET resolved_at = ?2, resolved_by = ?3 WHERE kind IN {SEND_FLAG_KINDS} AND item_id = ?1 AND resolved_at IS NULL AND EXISTS (SELECT 1 FROM circle_banks WHERE id = ?1 AND removed_reason = 'admin')"), params![bid, now, admin])?;
    tx.commit()?;
    Ok(if upd == 1 { (200, json!({ "ok": true })) } else { (409, json!({ "error": "not_removable" })) })
}

/// POST /admin/pool/circle-close {circle_id, note}: closed for everyone now (no owner undo), purged after 14 days; open reports resolved, holds lifted.
pub fn close_circle(x: &PoolCtx, admin: &str, body: &Map<String, Value>) -> Res<(u16, Value)> {
    let cid = id_of(body.get("circle_id"), "circle_id")?;
    let n = reason_of(body.get("note"))?;
    let now = x.now;
    let tx = x.conn.unchecked_transaction()?;
    let upd = tx.execute(
        "UPDATE circles SET closed_at = COALESCE(closed_at, ?2), close_reason = 'admin', undo_until = NULL, purge_after = ?3, updated_at = ?2 WHERE id = ?1 AND purged_at IS NULL AND close_reason IS NOT 'admin'",
        params![cid, now, now + CIRCLE_UNDO_MS],
    )?;
    tx.execute("UPDATE circle_reports SET resolved_at = ?2, resolved_by = ?3, action = 'removed' WHERE circle_id = ?1 AND resolved_at IS NULL AND changes() = 1", params![cid, now, admin])?;
    both(&tx, admin, now, &AuditEntry { actor_id: Some(admin), actor_kind: "admin", action: "circle.admin_close", item_id: Some(&cid), note: Some(&n), ..Default::default() }, "pool.circle_close", &cid, Some(&n), None)?;
    // And the holds of its banks, and their student-data flags: the admin's close is the decision on all of it.
    tx.execute("DELETE FROM circle_holds WHERE bank_id IN (SELECT id FROM circle_banks WHERE circle_id = ?1) AND EXISTS (SELECT 1 FROM circles WHERE id = ?1 AND close_reason = 'admin')", [&cid])?;
    tx.execute(
        &format!(
            "UPDATE admin_flags SET resolved_at = ?2, resolved_by = ?3 WHERE kind IN {SEND_FLAG_KINDS} AND resolved_at IS NULL AND item_id IN (SELECT id FROM circle_banks WHERE circle_id = ?1)
               AND EXISTS (SELECT 1 FROM circles WHERE id = ?1 AND close_reason = 'admin')"
        ),
        params![cid, now, admin],
    )?;
    tx.commit()?;
    Ok(if upd == 1 { (200, json!({ "ok": true, "purge_after": now + CIRCLE_UNDO_MS })) } else { (409, json!({ "error": "not_closable" })) })
}

#[cfg(test)]
#[path = "circleadmin_tests.rs"]
mod tests;
