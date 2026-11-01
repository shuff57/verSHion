//! The front of a publish (worker/src/pool/publish.ts `publish` and the opening of `publishOnce`): the daily allowances, the account lock, the held-publish cap, the
//! idempotency check against an earlier attempt with the same publish id, and the item a publish lands on. HTTP-free; the route reads the body and takes the heavy
//! slot around these.
use super::croncore::discard_staged;
use super::ctx::{Fail, PoolCtx, PoolError, Res};
use super::license::{ATTEST_STATEMENT, POOL_TERMS_VERSION};
use super::publishmeta::PublishMeta;
use super::publishrun::acquire_account_lock;
use super::sqlfrag::{day_of, is_id, new_id};
use crate::blobs::BlobStore;
use rusqlite::{OptionalExtension, params};
use serde_json::{Value, json};

/// A staging/staged publish younger than this is assumed to be in flight (a concurrent retry gets 409).
pub const IN_FLIGHT_MS: i64 = 60_000;
pub use super::circlecommit::HELD_PUBLISHES_PER_DAY;

fn in_progress(message: Option<&str>) -> PoolError {
    let e = PoolError::new(409, "publish_in_progress").header("Retry-After", "5");
    match message {
        Some(m) => e.with("message", json!(m)),
        None => e,
    }
}

/// Refused publishes are counted per day (each costs a full validation): past the allowance the account gets 429 before any work is done. A dry run (a preview)
/// counts against its own daily allowance only (never the publish allowance, the refusal counter or the held cap) and stores nothing. The count is reserved
/// BEFORE any work and refunded only by `settle` on a definite non-refusal, so a killed process or any unexpected failure still counts.
pub fn admit(x: &PoolCtx, me: &str, dry: bool) -> Res<()> {
    if dry {
        x.require_daily(me, "previews", 1)?;
    } else {
        x.miss_guard(me, "publish_refusals")?;
    }
    // The day's publish allowance is checked BEFORE any work (cheap); a request refused for it still counts as a refused publish (it is not refunded), so
    // retries cannot buy free validations.
    let used: Option<i64> = x.conn.query_row("SELECT count FROM usage_daily WHERE user_id = ?1 AND day = ?2 AND metric = 'publishes'", params![me, day_of(x.now)], |r| r.get(0)).optional()?;
    if used.unwrap_or(0) >= x.limit_of(me, "publishes")? {
        if !dry {
            x.count_miss(me, "publish_refusals")?;
            x.cap_alert(me, "publishes")?;
        }
        return Err(PoolError::new(429, "daily_limit").with("metric", json!("publishes")).header("Retry-After", "3600").into());
    }
    if !dry {
        x.count_miss(me, "publish_refusals")?;
    }
    Ok(())
}

/// After the publish ran: the reserved refusal count is given back on success and on a 409 or 503 (a conflict, a retry or a busy server is not a refused
/// publish); any other failure keeps it.
pub fn settle(x: &PoolCtx, me: &str, dry: bool, failure: Option<&PoolError>) -> Res<()> {
    if dry {
        return Ok(());
    }
    if failure.is_none_or(|e| e.status == 409 || e.status == 503) {
        x.refund_miss(me, "publish_refusals")?;
    }
    Ok(())
}

/// One publish at a time per account. None of the refusals below is a refused publish, so the reserved count is given back first.
pub fn take_account_lock(x: &PoolCtx, me: &str, dry: bool) -> Res<String> {
    match acquire_account_lock(x, me)? {
        Some(t) => Ok(t),
        None => {
            if !dry {
                x.refund_miss(me, "publish_refusals")?;
            }
            x.throw_if_erased(me)?; // (the guarded lock insert wrote nothing because the account is erased)
            Err(in_progress(Some("Another publish from this account is still running.")).into())
        }
    }
}

/// Before the heavy slot: the held-publish cap (never a refusal: past it, publishes that match a block are held neutrally and the admin is alerted), the
/// item-id miss guard, and the migration gate. Returns whether the account is over the cap.
pub fn before_heavy(x: &PoolCtx, me: &str, path_item_id: Option<&str>) -> Res<bool> {
    let held: Option<i64> = x.conn.query_row("SELECT count FROM usage_daily WHERE user_id = ?1 AND day = ?2 AND metric = 'held_publishes'", params![me, day_of(x.now)], |r| r.get(0)).optional()?;
    let over_cap = held.unwrap_or(0) >= HELD_PUBLISHES_PER_DAY;
    if over_cap {
        x.cap_alert(me, "held_publishes")?;
    }
    if let Some(id) = path_item_id {
        x.miss_guard(me, "item_miss")?;
        if !is_id(id) {
            x.count_miss(me, "item_miss")?;
            return Err(PoolError::new(404, "not_found").into());
        }
    }
    super::fingerprint::assert_not_migrating(x)?;
    Ok(over_cap)
}

/// Where a publish lands.
#[derive(Debug, Clone, PartialEq)]
pub struct Landing {
    pub item_id: String,
    pub version: i64,
    pub visibility: String,
    pub item_kind: Option<String>,
    pub base_of: Option<String>,
    pub prev_public_version: i64,
    pub prev_public_at: Option<i64>,
    pub prev_hidden: bool,
    pub prev_license: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Started {
    /// The same request was already answered: replay its result (`replayed: true`).
    Replay(Value),
    Go(Box<Landing>),
}

/// The publishing-terms affirmation, the idempotency check and the item. `request_sha` is the hash of the whole request body.
pub fn identify(x: &PoolCtx, store: &BlobStore, me: &str, path_item_id: Option<&str>, m: &PublishMeta, request_sha: &str, dry: bool) -> Res<Started> {
    // A publish or republish needs the teacher's affirmation of the current terms (recorded with the publish; not verified). A preview does not.
    if !dry && !m.attest.as_ref().is_some_and(|a| a.own_work && a.terms_version == POOL_TERMS_VERSION) {
        return Err(PoolError::new(422, "attestation_required").with("terms_version", json!(POOL_TERMS_VERSION)).with("statement", json!(ATTEST_STATEMENT)).into());
    }
    let is_new = path_item_id.is_none();
    type Prior = (String, String, String, String, Option<String>, String, Option<String>, i64);
    let prior: Option<Prior> = x
        .conn
        .query_row(
            "SELECT item_id, author_id, status, request_sha256, result_json, full_key, stripped_key, created_at FROM item_publishes WHERE publish_id = ?1",
            [&m.publish_id],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?, r.get(5)?, r.get(6)?, r.get(7)?)),
        )
        .optional()?;
    let retired = x.conn.query_row("SELECT 1 FROM retired_publish_ids WHERE publish_id = ?1", [&m.publish_id], |_| Ok(())).optional()?.is_some();
    if retired {
        return Err(PoolError::new(409, "publish_id_conflict").into());
    }
    if let Some((p_item, p_author, p_status, p_sha, p_result, p_full, p_stripped, p_created)) = prior {
        // Someone else's id, or the same id with a different body: never resumed, never revealed.
        if p_author != me || p_sha != request_sha || (!is_new && Some(p_item.as_str()) != path_item_id) {
            return Err(PoolError::new(409, "publish_id_conflict").into());
        }
        if matches!(p_status.as_str(), "current" | "previous") {
            if let Some(r) = p_result.as_deref().filter(|r| !r.is_empty()) {
                let mut v: Value = serde_json::from_str(r).unwrap_or_else(|_| json!({}));
                if let Value::Object(o) = &mut v {
                    o.insert("replayed".into(), Value::Bool(true));
                }
                return Ok(Started::Replay(v));
            }
        }
        if p_created > x.now - IN_FLIGHT_MS {
            return Err(in_progress(None).into());
        }
        // An abandoned attempt: start again from scratch with the same publish_id.
        if !discard_staged(x, store, &m.publish_id, &p_item, &p_full, p_stripped.as_deref(), p_created)? {
            return Err(in_progress(None).into());
        }
    }
    // ---- the item ----
    let Some(item_id) = path_item_id else {
        return Ok(Started::Go(Box::new(Landing {
            item_id: new_id(),
            version: 1,
            visibility: m.visibility.clone().unwrap_or_else(|| "unlisted".into()),
            item_kind: None,
            base_of: None,
            prev_public_version: 0,
            prev_public_at: None,
            prev_hidden: false,
            prev_license: None,
        })));
    };
    type ItemRow = (String, String, String, Option<String>, i64, Option<i64>, Option<i64>, i64, Option<i64>, i64, String);
    let it: Option<ItemRow> = x
        .conn
        .query_row(
            "SELECT id, kind, visibility, current_publish_id, version, removed_at, purged_at, public_version, public_at, hide_key, license FROM items WHERE id = ?1 AND author_id = ?2",
            params![item_id, me],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?, r.get(5)?, r.get(6)?, r.get(7)?, r.get(8)?, r.get(9)?, r.get(10)?)),
        )
        .optional()?;
    let Some((id, kind, visibility, current, version, removed_at, purged_at, public_version, public_at, hide_key, license)) = it else {
        x.count_miss(me, "item_miss")?;
        return Err(PoolError::new(404, "not_found").into());
    };
    if removed_at.is_some() {
        let e = PoolError::new(409, "item_removed");
        return Err(if purged_at.is_some() {
            e.with("purged", Value::Bool(true)).with("message", json!("This item was deleted for good. Publish it again as a new item."))
        } else {
            e.with("message", json!("This item is removed. Undo the deletion first, or publish it as a new item."))
        }
        .into());
    }
    if current != m.base {
        return Err(Fail::from(PoolError::new(409, "conflict").with("current_version", json!(version)).with("current_publish_id", json!(current))));
    }
    if visibility == "public" && m.license == "circle-only" {
        return Err(PoolError::new(422, "license_not_public").into());
    }
    Ok(Started::Go(Box::new(Landing {
        item_id: id,
        version: version + 1,
        visibility,
        item_kind: Some(kind),
        base_of: current,
        prev_public_version: public_version,
        prev_public_at: public_at,
        prev_hidden: hide_key == 1,
        prev_license: Some(license),
    })))
}

#[cfg(test)]
#[path = "publishstart_tests.rs"]
mod tests;
