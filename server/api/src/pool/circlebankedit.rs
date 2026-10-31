//! Circle bank blobs and the licence setting (worker/src/pool/circles.ts `getBankBlob`, `patchBank`'s licence path). HTTP-free cores; the
//! `hide_key` branch of the PATCH belongs with circlehide.
use super::circlebanks::*;
use super::circlebase::*;
use super::circles::bad_field;
use super::circlesql::*;
use super::ctx::{AuditEntry, PoolCtx, PoolError, Res, audit, not_found};
use super::license::{LICENSES, allowed_by_all, is_license};
use super::sqlfrag::{alive_sql, is_id, is_sha};
use rusqlite::{OptionalExtension, params};
use serde_json::{Map, Value, json};

/// The role hint of the routes the circle's owner or the bank's author (while an editor) may use.
pub const OWNER_OR_AUTHOR: &str = "owner_or_author";

/// The file extension a served image gets.
pub fn image_ext(content_type: &str) -> &'static str {
    match content_type {
        "image/png" => "png",
        "image/jpeg" => "jpg",
        "image/gif" => "gif",
        "image/webp" => "webp",
        _ => "bin",
    }
}

/// GET /circles/:cid/banks/:bid/blobs/:sha256: the content type of an image a KEPT version of the bank uses, to a member who may read the
/// bank, or a counted 404 (`blob_miss`). The route layer then serves `blobs/<sha>` with `blob_headers`.
pub fn bank_blob_type(x: &PoolCtx, me: &str, cid: &str, bid: &str, sha: &str) -> Res<String> {
    x.miss_guard(me, "blob_miss")?;
    let miss = || -> Res<String> {
        x.count_miss(me, "blob_miss")?;
        Err(not_found().into())
    };
    if !is_sha(sha) {
        return miss();
    }
    let m = if is_id(cid) { member_of(x, cid, me)? } else { None };
    let Some(m) = m.filter(|m| m.closed_at.is_none()) else { return miss() };
    let row: Option<String> = x
        .conn
        .query_row(
            &format!(
                "SELECT bl.content_type FROM circle_banks b
         JOIN circle_versions v ON v.bank_id = b.id AND v.status IN ('current', 'previous') AND v.withheld_at IS NULL
         JOIN circle_version_blobs vb ON vb.version_id = v.id AND vb.sha256 = ?3
         JOIN blobs bl ON bl.sha256 = vb.sha256
        WHERE b.id = ?1 AND b.circle_id = ?2 AND b.purged_at IS NULL AND b.removed_at IS NULL
          AND (b.author_id = ?4 OR NOT EXISTS (SELECT 1 FROM circle_holds h WHERE h.bank_id = b.id) OR (NOT {} AND {})) AND {}
          AND NOT EXISTS (SELECT 1 FROM privacy_blocks pb WHERE pb.kind = 'blob' AND pb.hash = ?3) AND bl.privacy_purged_at IS NULL
          AND (?5 = 0 OR b.hide_key = 0 OR ?3 IN (SELECT value FROM json_each(COALESCE(json_extract(v.strip_opts, '$.image_shas'), '[]'))))
        LIMIT 1",
                blocking_hold_sql("b.id"),
                active_sql("b.circle_id", "?4", &["owner"]),
                author_ok_sql("b")
            ),
            params![bid, cid, sha, me, if m.role == "viewer" { 1 } else { 0 }],
            |r| r.get(0),
        )
        .optional()?;
    match row {
        Some(t) => Ok(t),
        None => miss(),
    }
}

/// The response headers of a served bank image.
pub fn blob_headers(content_type: &str, sha: &str) -> Vec<(&'static str, String)> {
    vec![
        ("Content-Type", content_type.to_string()),
        ("X-Content-Type-Options", "nosniff".into()),
        ("Content-Security-Policy", "sandbox; default-src 'none'".into()),
        ("Content-Disposition", format!("attachment; filename=\"{sha}.{}\"", image_ext(content_type))),
        ("Cache-Control", "no-store".into()),
        ("Cross-Origin-Resource-Policy", "same-site".into()),
        ("Vary", "Authorization, Cookie, Origin".into()),
    ]
}

/// The gate every PATCH of a bank starts with: only the circle's owner or the bank's author may change anything here, refused before
/// the body is read.
pub fn patch_gate(x: &PoolCtx, me: &str, cid: &str, bid: &str) -> Res<(Membership, BankRow)> {
    let (m, b) = bank_or_miss(x, cid, bid, me)?;
    if m.role != "owner" && b.author_id != me {
        return Err(PoolError::new(403, "circle_role_required").with("needed", OWNER_OR_AUTHOR.into()).with("role", m.role.clone().into()).into());
    }
    Ok((m, b))
}

/// PATCH /circles/:cid/banks/:bid {license} (the bank's author): the re-share setting. Earlier versions keep their own licence.
pub fn patch_license(x: &PoolCtx, me: &str, cid: &str, m: &Membership, b: &BankRow, body: &Map<String, Value>) -> Res<Value> {
    for k in body.keys() {
        if k != "license" {
            if k == "base_version" {
                return Err(bad_field("base_version", &[("message", "base_version goes with hide_key.".into())]));
            }
            return Err(bad_field(k, &[]));
        }
    }
    let lic_err = || bad_field("license", &[("allowed", json!(LICENSES))]);
    let lic = match body.get("license") {
        None => return Err(lic_err()),
        Some(v) => v,
    };
    require_open(m)?;
    if b.author_id != me {
        return Err(PoolError::new(403, "circle_role_required").with("needed", "author".into()).with("role", m.role.clone().into()).into());
    }
    // The author while an editor (or the owner): a demoted author no longer changes it.
    if m.role != "owner" && m.role != "editor" {
        return Err(PoolError::new(403, "circle_role_required").with("needed", "editor".into()).with("role", m.role.clone().into()).into());
    }
    if b.removed_at.is_some() {
        return Err(PoolError::new(409, "bank_removed").with("reason", b.removed_reason.clone().map(Value::from).unwrap_or(Value::Null)).into());
    }
    let lic = match lic.as_str().filter(|l| is_license(l)) {
        Some(l) => l,
        None => return Err(lic_err()),
    };
    // Public material inside the bank (declared origins, checked at send): the licence must be one every origin allows.
    let origins_json: Option<String> = x.conn.query_row("SELECT origins FROM circle_versions WHERE id = ?1", [&b.head_version_id], |r| r.get(0)).optional()?.flatten();
    let origins: Vec<Value> = origins_json.and_then(|j| serde_json::from_str(&j).ok()).unwrap_or_default();
    let allowed = allowed_by_all(origins.iter().filter_map(|o| o.get("license").and_then(Value::as_str)));
    if !allowed.contains(&lic) {
        return Err(PoolError::new(422, "license_incompatible")
            .with("allowed", json!(allowed))
            .with("message", "Some questions in this bank came from the public pool under a licence that does not allow this one.".into())
            .into());
    }
    let tx = x.conn.unchecked_transaction()?;
    // A compare-and-set on the head whose origins were checked above.
    let upd = tx.execute(
        &format!(
            "UPDATE circle_banks SET license = ?4, updated_at = ?5 WHERE id = ?1 AND circle_id = ?2 AND author_id = ?3 AND head_version_id IS ?6 AND removed_at IS NULL AND {} AND {} AND {}",
            open_sql("?2"),
            active_sql("?2", "?3", &["owner", "editor"]),
            alive_sql("?3")
        ),
        params![b.id, cid, me, lic, x.now, b.head_version_id],
    )?;
    if upd == 1 {
        audit(&tx, &AuditEntry { actor_id: Some(me), actor_kind: "user", action: "circle.bank_license", item_id: Some(cid), target_user_id: Some(me), detail: Some(json!({ "bank_id": b.id, "license": lic })), ..Default::default() }, x.now, None)?;
    }
    tx.commit()?;
    if upd != 1 {
        return Err(explain(x, cid, me, None));
    }
    Ok(json!({ "ok": true, "license": lic }))
}

#[cfg(test)]
#[path = "circlebankedit_tests.rs"]
mod tests;
