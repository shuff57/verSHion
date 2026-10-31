//! Rolling a circle bank back (worker/src/pool/circlesend.ts `rollbackBank`, `rollbackOnce`, `currentListing`, and the daily-cap wrapper
//! `locked`): a NEW version whose body is a kept version's (copied from the store, never re-validated; its qids, images and recorded
//! origins with it), accepted only if the head is still `base_version`. The owner, or the bank's author while an editor. The per-account
//! lock around it belongs to the route layer.
use super::circlebanks::{bank_or_miss, not_retained, require_readable, withheld_error};
use super::circlebankedit::OWNER_OR_AUTHOR;
use super::circlecommit::{CommitSpec, Kind, commit_version, conflict, fresh_bank};
use super::circlehide::decisions_of;
use super::circlesendmeta::*;
use super::circlesendprep::{Origin, SendEnv};
use super::core::{CoreErr, questions_of};
use super::ctx::{Fail, PoolError, Res};
use super::license::allowed_by_all;
use super::qjson::Json;
use super::sqlfrag::{DAY_MS, alive_sql, is_publish_id, new_id};
use crate::auth::validate::clean_text;
use rusqlite::{OptionalExtension, params};
use serde_json::{Map, Value, json};

fn bad(field: &str) -> PoolError {
    PoolError::new(400, "invalid_request").with("field", json!(field))
}

/// A send or rollback answer is not charged to the day's allowance when nothing new was stored (a replay, a conflict, a large-change
/// confirm, busy).
pub const REFUNDED: [u16; 3] = [409, 428, 503];

/// The day's counter around a send or a rollback: given back when the answer is a replay, a conflict, a large-change confirm or busy.
/// `run` returns (status, body, replayed).
pub fn locked_daily(env: &SendEnv, me: &str, metric: &str, run: impl FnOnce() -> Res<(u16, Value, bool)>) -> Res<(u16, Value)> {
    env.x.require_daily(me, metric, 1)?;
    let refund = || {
        let _ = env.x.give_daily(me, metric, 1);
    };
    match run() {
        Ok((status, body, replayed)) => {
            if replayed || REFUNDED.contains(&status) {
                refund();
            }
            Ok((status, body))
        }
        Err(e) => {
            if let Fail::Pool(p) = &e {
                if REFUNDED.contains(&p.status) {
                    refund();
                }
            }
            Err(e)
        }
    }
}

pub struct RollbackReq {
    pub commit_id: String,
    pub to: i64,
    pub base: i64,
    pub message: Option<String>,
    pub confirm: bool,
}

/// POST /circles/:cid/banks/:bid/rollback {to_version, base_version, commit_id, message?, confirm_large_change?}. Never to a pruned version
/// (`404 version_not_retained`) or to the head itself (400). Dropping more than half of the head's questions needs `confirm_large_change`.
/// CIRCLE_ROLLBACKS a day.
pub fn rollback_bank(env: &SendEnv, me: &str, cid: &str, bid: &str, body: &Map<String, Value>) -> Res<(u16, Value)> {
    let (m, b) = bank_or_miss(env.x, cid, bid, me)?;
    require_readable(env.x, &m, &b, me)?;
    if !(m.role == "owner" || (b.author_id == me && m.role == "editor")) {
        return Err(PoolError::new(403, "circle_role_required").with("needed", json!(OWNER_OR_AUTHOR)).with("role", json!(m.role)).into());
    }
    let commit_id = match body.get("commit_id").and_then(Value::as_str) {
        Some(c) if is_publish_id(c) => c.to_string(),
        _ => return Err(bad("commit_id").into()),
    };
    let base = body.get("base_version").and_then(int_at_least_1).ok_or_else(|| bad("base_version"))?;
    let to = match body.get("to_version").and_then(int_at_least_1) {
        Some(t) if t < base => t,
        _ => return Err(PoolError::new(400, "invalid_request").with("field", json!("to_version")).with("message", json!("Roll back to an earlier version than the one you are looking at.")).into()),
    };
    if body.get("confirm_large_change").is_some_and(|v| !v.is_boolean()) {
        return Err(bad("confirm_large_change").into());
    }
    let r = RollbackReq { commit_id, to, base, message: message_of(body.get("message"))?, confirm: body.get("confirm_large_change") == Some(&Value::Bool(true)) };
    locked_daily(env, me, "circle_rollbacks", || rollback_once(env, me, cid, bid, &r))
}

fn current_listing(env: &SendEnv, bid: &str, title: &str) -> Res<String> {
    let l: Option<String> = env
        .x
        .conn
        .query_row("SELECT json_object('title', CASE WHEN ?2 <> '' THEN ?2 ELSE title END, 'subject', subject, 'course', course, 'grade', grade) FROM circle_banks WHERE id = ?1", params![bid, title], |r| r.get(0))
        .optional()?;
    Ok(l.unwrap_or_else(|| json!({ "title": title, "subject": "", "course": "", "grade": "" }).to_string()))
}

fn to_json(v: &Value) -> Json {
    Json::parse(v.to_string().as_bytes()).unwrap_or(Json::Null)
}

/// Returns (status, body, replayed).
pub fn rollback_once(env: &SendEnv, me: &str, cid: &str, bid: &str, r: &RollbackReq) -> Res<(u16, Value, bool)> {
    let x = env.x;
    let now = x.now;
    let request_sha = super::blobs::sha256_hex(json!(["circle-rollback/v1", bid, r.to, r.base, r.message.clone().unwrap_or_default()]).to_string().as_bytes());
    if let Some(v) = replay_of(x, env.store, me, cid, Some(bid), &r.commit_id, &request_sha)? {
        return Ok((200, v, true));
    }
    let bank = fresh_bank(x, cid, bid)?;
    if bank.head_version != r.base {
        return Err(conflict(x, bid)?.into());
    }
    struct T {
        id: String,
        status: String,
        full_key: String,
        index_key: String,
        question_count: i64,
        origins: Option<String>,
        withheld_at: Option<i64>,
        listing: Option<String>,
        strip_opts: Option<String>,
    }
    let target = x
        .conn
        .query_row(
            "SELECT id, status, full_key, index_key, bytes, question_count, origins, withheld_at, listing, strip_opts FROM circle_versions WHERE bank_id = ?1 AND version = ?2",
            params![bid, r.to],
            |row| Ok(T { id: row.get(0)?, status: row.get(1)?, full_key: row.get(2)?, index_key: row.get(3)?, question_count: row.get(5)?, origins: row.get(6)?, withheld_at: row.get(7)?, listing: row.get(8)?, strip_opts: row.get(9)? }),
        )
        .optional()?
        .ok_or_else(|| Fail::from(super::ctx::not_found()))?;
    let not_kept = || -> Fail {
        match not_retained(x, bid, bank.head_version) {
            Ok(e) => e.into(),
            Err(e) => e.into(),
        }
    };
    if (target.status != "current" && target.status != "previous") || target.full_key.is_empty() {
        return Err(not_kept());
    }
    // Never back to a version that held confirmed student data (until an administrator releases it).
    if target.withheld_at.is_some() {
        return Err(withheld_error(None).into());
    }
    let head_key: Option<String> = match &bank.head_version_id {
        Some(h) => x.conn.query_row("SELECT index_key FROM circle_versions WHERE id = ?1", [h], |row| row.get(0)).optional()?,
        None => None,
    };
    let t_idx = read_index(env.store, &target.index_key).ok_or_else(not_kept)?;
    let h_idx = head_key.and_then(|k| read_index(env.store, &k));
    let (added, changed, removed) = diff(h_idx.as_ref(), &t_idx.qids, &t_idx.h);
    if let (false, Some(h)) = (r.confirm, &h_idx) {
        if is_large(removed, h.qids.len() as i64) {
            return Err(large_error(removed, h.qids.len() as i64).into());
        }
    }
    // The bank's licence stays (the author's choice); the copied questions' recorded origins must allow it.
    let mut origins: Vec<Origin> = target.origins.as_deref().and_then(|o| serde_json::from_str(o).ok()).unwrap_or_default();
    let allowed = allowed_by_all(origins.iter().map(|o| o.license.as_str()));
    if !origins.is_empty() && !allowed.contains(&bank.license.as_str()) {
        return Err(PoolError::new(422, "license_incompatible")
            .with("allowed", json!(allowed))
            .with("message", json!("That version holds public questions whose licence does not allow this bank's current licence."))
            .into());
    }
    // Origins keep their circle qid (a later send carries them on).
    for o in origins.iter_mut() {
        if o.cq.is_none() {
            o.cq = t_idx.qids.get(o.pos).cloned();
        }
    }

    // The body is copied. Its student-data check needs its questions.
    let body_bytes = env.store.get(&target.full_key).ok().flatten().ok_or_else(not_kept)?;
    let shas: Vec<String> = {
        let mut st = x.conn.prepare("SELECT sha256 FROM circle_version_blobs WHERE version_id = ?1")?;
        let v = st.query_map([&target.id], |row| row.get(0))?.collect::<Result<Vec<String>, _>>()?;
        v
    };
    // The images it copies are held for the account until the commit, as a send's are.
    if !shas.is_empty() {
        x.conn.execute(
            &format!(
                "INSERT INTO pending_blobs (sha256, uploader_id, size, until) SELECT sha256, ?2, size, ?3 FROM blobs WHERE sha256 IN (SELECT value FROM json_each(?1)) AND {}
         ON CONFLICT (sha256, uploader_id) DO UPDATE SET until = MAX(until, excluded.until)",
                alive_sql("?2")
            ),
            params![serde_json::to_string(&shas).unwrap(), me, now + DAY_MS],
        )?;
    }
    let pj: Value = serde_json::from_slice(&body_bytes).map_err(|_| not_kept())?;
    let qs: Vec<Json> = questions_of(&pj).into_iter().map(to_json).collect();
    let body_name = pj.get("bank").and_then(|b| b.get("name")).and_then(|n| clean_text(n, 200)).unwrap_or_default();
    // The key-free match hashes were kept in the index from the start of circles on; an older version's are computed again.
    let mh = match t_idx.m.clone() {
        Some(m) => m,
        None => env.core.match_hashes(&body_bytes).map_err(|_| not_kept())?,
    };
    let full_img = images_at(&qs);
    // A bank that hides its keys serves its viewers the target's stripped form with the decisions it recorded (copied with it): that form is
    // checked too. (No decisions recorded: viewers are refused it.)
    let mut forms = vec![ScanForm { qs, mh: mh.clone(), img_at: full_img }];
    if bank.hide_key == 1 && target.strip_opts.is_some() {
        match env.core.strip(&body_bytes, &decisions_of(target.strip_opts.as_deref()).opts()) {
            Ok(st) => {
                let vq: Vec<Json> = questions_of(&st.v.payload).into_iter().map(to_json).collect();
                forms.push(ScanForm { img_at: images_at(&vq), mh: st.match_hashes.clone(), qs: vq });
            }
            Err(CoreErr::Fail(_)) => {}
            Err(_) => return Err(PoolError::new(500, "server_error").into()),
        }
    }
    let privacy = privacy_scan(x, env.fp, &forms, me)?;
    drop(forms);
    let index = serde_json::to_vec(&json!({ "qids": t_idx.qids, "h": t_idx.h, "m": mh })).unwrap();
    // Charged per version as a send is: the body, the index and each image it references.
    let image_bytes: i64 = if shas.is_empty() {
        0
    } else {
        x.conn.query_row("SELECT COALESCE(SUM(size), 0) FROM blobs WHERE sha256 IN (SELECT value FROM json_each(?1))", [serde_json::to_string(&shas).unwrap()], |row| row.get(0))?
    };
    let vid = new_id();
    let version = r.base + 1;
    let listing = match target.listing.clone() {
        Some(l) => l,
        None => current_listing(env, bid, &body_name)?,
    };
    let spec = CommitSpec {
        kind: Kind::Rollback,
        me: me.into(),
        cid: cid.into(),
        bid: bid.into(),
        vid: vid.clone(),
        is_new: false,
        base: Some(r.base),
        version,
        commit_id: r.commit_id.clone(),
        request_sha,
        message: r.message.clone(),
        license: bank.license.clone(),
        meta: None,
        listing: Some(listing),
        n: target.question_count,
        has_images: (!shas.is_empty()) as i64,
        bytes: body_bytes.len() as i64 + index.len() as i64 + image_bytes,
        new_keys: 0,
        hashes: vec![],
        shas,
        origins: origins.iter().map(|o| serde_json::to_value(o).unwrap()).collect(),
        merged_from: None,
        rollback_of: Some(r.to),
        privacy,
        strip_opts: target.strip_opts.clone(),
        pairs: vec![],
        hide_key: Some(bank.hide_key),
        result: json!({ "circle_id": cid, "bank_id": bid, "version": version, "version_id": vid, "qids": t_idx.qids, "added": added, "changed": changed, "removed": removed, "rollback_of": r.to }),
        audit_detail: json!({ "added": added, "changed": changed, "removed": removed, "rollback_of": r.to }),
        full_bytes: body_bytes,
        index_bytes: index,
    };
    let (status, body) = commit_version(x, env.store, &spec)?;
    Ok((status, body, false))
}

#[cfg(test)]
#[path = "circlerollback_tests.rs"]
mod tests;
