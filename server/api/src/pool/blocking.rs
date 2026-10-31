//! Block plans for privacy purges and DMCA takedowns (worker/src/pool/blocking.ts): which question texts (match_hash) and images a
//! block covers, chosen per question by the admin, with a preview of every item the block would hold, confirmed by a short-lived
//! token so the confirm acts on exactly what was previewed.
//!
//! This file holds the plan's types, the SQL fragments, the preview token, the selection reader and the two hold sweeps; the plan
//! itself (`block_plan`) is in `blockplan.rs`.
use super::ctx::{PoolCtx, PoolError, Res, bad};
use super::poolio::notify_held;
use super::poolkeys::PoolKeys;
use super::served::item_serves_blob;
use super::sqlfrag::{new_id, q_key_eq, q_key_in};
use crate::auth::crypto::{b64url, from_b64url, hmac, timing_safe_equal};
use rusqlite::params;
use serde::Serialize;
use serde_json::{Map, Value, json};
use std::collections::BTreeMap;

/// What a question's block covers: "default" | "skeleton" | "stem" | "full" | "explanation" | "answer" | "part:<k>".
pub type BlockScope = String;
pub const BLOCK_SCOPES: [&str; 6] = ["default", "skeleton", "stem", "full", "explanation", "answer"];
/// `part:<k>` blocks only part k of a Multipart (its stem, explanation and answers, wherever they appear).
pub fn is_scope(v: &Value) -> bool {
    let Some(s) = v.as_str() else { return false };
    BLOCK_SCOPES.contains(&s) || s.strip_prefix("part:").is_some_and(|k| (1..=3).contains(&k.len()) && k.bytes().all(|b| b.is_ascii_digit()) && (k == "0" || !k.starts_with('0')))
}
/// Other authors' visible items sharing a stem from which it counts as generic (the preview warns).
pub const GENERIC_STEM_AUTHORS: i64 = 3;
/// Kinds whose skeleton is the bare stem: a "stem" block of one is stem-wide.
pub const STEM_ONLY: [&str; 3] = ["Tf", "Short", "Blank"];

#[derive(Debug, Default, Clone)]
pub struct BlockSelection {
    /// Question positions in the item's current publish; None = every distinctive question.
    pub questions: Option<Vec<i64>>,
    /// Per position, what its block covers (default "stem"; a privacy block's default is wider).
    pub scopes: Option<BTreeMap<String, Value>>,
    /// Image hashes; None = every image of the item.
    pub images: Option<Vec<String>>,
    /// A takedown of a purged item (a ghost) selects NOTHING by default; `all` (sent as `questions: "all"`) applies the live
    /// default to every position, explicitly.
    pub all: bool,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct Affects {
    pub item_id: String,
    pub title: String,
}
#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct First {
    pub item_id: String,
    pub author_id: String,
}
#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct MatchPlan {
    pub pos: i64,
    pub hash: String,
    /// The block keys this position's block holds, by its scope.
    pub keys: Vec<String>,
    pub scope: BlockScope,
    /// A "stem" block of a stem-only kind: it holds every question with this stem, whatever its answer or explanation.
    pub stem_wide: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub generic_stem: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub generic_authors: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub suggest_scope: Option<&'static str>,
    /// Up to 20 other items a selected position's block alone would hold.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub affects: Option<Vec<Affects>>,
    pub other_authors: i64,
    pub introduced: bool,
    pub selected: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub suggested: Option<bool>,
    /// The item (and author) that had this content first, when not this one.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub first: Option<First>,
}
#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct BlobPlan {
    pub sha256: String,
    pub other_authors: i64,
    pub introduced: bool,
    pub selected: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub suggested: Option<bool>,
}
#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct Affected {
    pub item_id: String,
    pub author_id: String,
    pub title: String,
    /// "question" | "image"
    pub via: String,
}
#[derive(Debug, Clone, Serialize, PartialEq, Default)]
pub struct BlockPlan {
    pub item_id: String,
    #[serde(rename = "match")]
    pub matches: Vec<MatchPlan>,
    pub blob: Vec<BlobPlan>,
    /// Other items (any state) the block would hold: they use a selected question or SERVE a selected image (first AFFECTED_LIST).
    pub affected: Vec<Affected>,
    /// How many there are in all: more than AFFECTED_LIST needs an explicit confirm_large (fails closed).
    pub affected_total: i64,
    /// Planned from a purged item's ghost; nothing selected yet (the admin must choose).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ghost: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub selection_required: Option<bool>,
}

pub const PREVIEW_MS: i64 = 10 * 60_000;
/// Affected items listed (and held in the confirm's own batch); the rest are held in batches after it.
pub const AFFECTED_LIST: usize = 500;

/// SQL: the snapshot a hold keeps of its cause at the admin action: the item's current publish and the positions of its questions
/// in the JSON array `mh`. Notices show it, never a re-evaluation against later content (no block-membership probe).
pub fn hold_snapshot(item: &str, mh: &str) -> String {
    format!(
        "json_object('publish_id', (SELECT current_publish_id FROM items WHERE id = {item}),
               'positions', (SELECT json_group_array(iq.pos) FROM item_questions iq WHERE iq.publish_id = (SELECT current_publish_id FROM items WHERE id = {item}) AND {}))",
        q_key_in("iq", &format!("(SELECT value FROM json_each({mh}))"))
    )
}

/// SQL condition on item row `i`: it uses a question in the JSON array `mh` (any retained publish) or SERVES an image in the JSON
/// array `bh`: what a block holds.
pub fn uses_blocked(mh: &str, bh: &str) -> String {
    format!(
        "(EXISTS (SELECT 1 FROM item_publishes p JOIN item_questions iq ON iq.publish_id = p.publish_id WHERE p.item_id = i.id AND {})
    OR {})",
        q_key_in("iq", &format!("(SELECT value FROM json_each({mh}))")),
        item_serves_blob("i.id", |sha| format!("{sha} IN (SELECT value FROM json_each({bh}))"))
    )
}

/// The selected block keys with the match_hash of the question each came from (block rows keep it as `qhash`).
pub fn selected_key_pairs(p: &BlockPlan) -> Vec<(String, String)> {
    let mut seen = std::collections::HashSet::new();
    p.matches.iter().filter(|x| x.selected).flat_map(|x| x.keys.iter().filter(|k| seen.insert((*k).clone())).map(|k| (k.clone(), x.hash.clone())).collect::<Vec<_>>()).collect()
}
/// The block keys of the selected positions (by their scopes).
pub fn selected_match(p: &BlockPlan) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for k in p.matches.iter().filter(|x| x.selected).flat_map(|x| x.keys.iter()) {
        if !out.contains(k) {
            out.push(k.clone());
        }
    }
    out
}
pub fn selected_blobs(p: &BlockPlan) -> Vec<String> {
    p.blob.iter().filter(|x| x.selected).map(|x| x.sha256.clone()).collect()
}

/// What a block-plan confirm binds: the selection, the affected items, the admin and the action's parameters.
pub fn plan_digest(kind: &str, p: &BlockPlan, bind: &Map<String, Value>) -> String {
    let sorted = |mut v: Vec<String>| {
        v.sort();
        v
    };
    let mut entries: Vec<(&String, &Value)> = bind.iter().collect();
    entries.sort_by(|a, b| a.0.cmp(b.0));
    json!([
        kind,
        p.item_id,
        sorted(selected_match(p)),
        sorted(selected_blobs(p)),
        sorted(p.affected.iter().map(|a| a.item_id.clone()).collect()),
        p.affected_total,
        entries
    ])
    .to_string()
}

/// A token binding a confirm to exactly the previewed digest, valid PREVIEW_MS and usable once.
pub fn token_for(x: &PoolCtx, keys: &PoolKeys, digest: &str) -> String {
    let exp = x.now + PREVIEW_MS;
    let body = b64url(json!([exp, hmac(&keys.preview, digest), new_id()]).to_string().as_bytes());
    format!("{body}.{}", hmac(&keys.preview, &body))
}

fn preview_required() -> PoolError {
    PoolError::new(428, "preview_required").with("message", "Preview first and confirm with its confirm_preview token (it binds the confirm to what you saw, once).".into())
}

/// 428 unless `token` was issued for this very digest (same selection, affected items, admin and parameters), is unexpired, and was
/// never used before (it is consumed here).
pub fn check_token(x: &PoolCtx, keys: &PoolKeys, digest: &str, token: &Value) -> Res<()> {
    let Some(token) = token.as_str() else { return Err(preview_required().into()) };
    // `^([A-Za-z0-9_-]{1,600})\.([A-Za-z0-9_-]{43})$`
    let url_safe = |t: &str| t.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_');
    let Some((body, mac)) = token.split_once('.').filter(|(b, m)| (1..=600).contains(&b.len()) && m.len() == 43 && url_safe(b) && url_safe(m)) else {
        return Err(preview_required().into());
    };
    let same = |a: &str, b: &str| a.len() == b.len() && timing_safe_equal(a.as_bytes(), b.as_bytes());
    if !same(&hmac(&keys.preview, body), mac) {
        return Err(preview_required().into());
    }
    let parsed: Option<(i64, String)> = from_b64url(body).and_then(|b| serde_json::from_slice::<Value>(&b).ok()).and_then(|v| Some((v.get(0)?.as_i64()?, v.get(1)?.as_str()?.to_string())));
    let Some((exp, h)) = parsed else { return Err(preview_required().into()) };
    if exp < x.now || !same(&h, &hmac(&keys.preview, digest)) {
        return Err(preview_required().into());
    }
    let used = x.conn.execute("INSERT OR IGNORE INTO preview_tokens_used (token_hash, at) VALUES (?1, ?2)", params![hmac(&keys.preview, &format!("used:{mac}")), x.now])?;
    if used != 1 {
        return Err(preview_required().into());
    }
    Ok(())
}

pub fn preview_token(x: &PoolCtx, keys: &PoolKeys, kind: &str, p: &BlockPlan, bind: &Map<String, Value>) -> String {
    token_for(x, keys, &plan_digest(kind, p, bind))
}
pub fn check_preview(x: &PoolCtx, keys: &PoolKeys, kind: &str, p: &BlockPlan, token: &Value, bind: &Map<String, Value>) -> Res<()> {
    check_token(x, keys, &plan_digest(kind, p, bind), token)
}

/// Reads a selection from an admin request body.
pub fn read_selection(b: Option<&Map<String, Value>>) -> Res<BlockSelection> {
    let mut sel = BlockSelection::default();
    let get = |k: &str| b.and_then(|b| b.get(k));
    match get("questions") {
        Some(Value::String(s)) if s == "all" => sel.all = true,
        None => {}
        Some(Value::Array(a)) if a.len() <= 1000 => {
            // (a JS array of numbers; anything else fails the range check at plan time)
            sel.questions = Some(a.iter().map(|v| v.as_f64().filter(|f| f.fract() == 0.0).map(|f| f as i64).unwrap_or(-1)).collect());
        }
        Some(_) => return Err(bad("questions").into()),
    }
    match get("scopes") {
        None => {}
        Some(Value::Object(o)) if o.len() <= 1000 => sel.scopes = Some(o.iter().map(|(k, v)| (k.clone(), v.clone())).collect()),
        Some(_) => return Err(bad("scopes").into()),
    }
    match get("images") {
        None => {}
        Some(Value::Array(a)) if a.len() <= 1000 && a.iter().all(Value::is_string) => sel.images = Some(a.iter().map(|v| v.as_str().unwrap().to_string()).collect()),
        Some(_) => return Err(bad("images").into()),
    }
    Ok(sel)
}

/// Holds of `source` (reason `reason`) on items that no longer use any still-active block of that source are released: what an
/// unblock leaves behind. Images count only where SERVED.
pub fn release_orphan_holds(x: &PoolCtx, source: &str, reason: &str) -> Res<usize> {
    let blocks = if source == "privacy" {
        "SELECT kind, hash FROM privacy_blocks".to_string()
    } else {
        let id: String = source.chars().skip(5).filter(|c| c.is_ascii_digit() || ('a'..='f').contains(c)).collect();
        format!("SELECT kind, hash FROM blocked_hashes WHERE action_id = '{id}' AND lifted_at IS NULL AND (expires_at IS NULL OR expires_at > {})", x.now)
    };
    let n = x.conn.execute(
        &format!(
            "DELETE FROM item_holds WHERE source = ?1 AND reason = ?2 AND NOT (
         EXISTS (SELECT 1 FROM item_publishes p JOIN item_questions iq ON iq.publish_id = p.publish_id JOIN ({blocks}) bl ON bl.kind = 'match' AND {}
                  WHERE p.item_id = item_holds.item_id)
         OR {})",
            q_key_eq("iq", "bl.hash"),
            item_serves_blob("item_holds.item_id", |sha| format!("EXISTS (SELECT 1 FROM ({blocks}) bl WHERE bl.kind = 'blob' AND bl.hash = {sha})"))
        ),
        params![source, reason],
    )?;
    Ok(n)
}

/// Holds (student_data, source privacy) on up to `batches` x `size` more items that use the given privacy-blocked hashes and are
/// not held yet; returns whether some may remain.
pub fn hold_privacy_batch(x: &PoolCtx, size: usize, except_item: &str, mh: &[String], bh: &[String], by: Option<&str>, batches: usize) -> Res<bool> {
    for _ in 0..batches {
        let ids: Vec<String> = x
            .conn
            .prepare(&format!(
                "INSERT OR IGNORE INTO item_holds (item_id, reason, at, by, source, detail)
         SELECT i.id, 'student_data', ?4, ?5, 'privacy', {} FROM items i
          WHERE i.id <> ?1 AND i.purged_at IS NULL AND NOT EXISTS (SELECT 1 FROM item_holds h WHERE h.item_id = i.id AND h.source = 'privacy')
            AND {} LIMIT {size} RETURNING item_id",
                hold_snapshot("i.id", "?2"),
                uses_blocked("?2", "?3")
            ))?
            .query_map(params![except_item, serde_json::to_string(mh).unwrap(), serde_json::to_string(bh).unwrap(), x.now, by], |r| r.get(0))?
            .collect::<Result<_, _>>()?;
        notify_held(x, &ids, "student_data")?;
        if ids.len() < size {
            return Ok(false);
        }
    }
    Ok(true)
}

#[cfg(test)]
#[path = "blocking_tests.rs"]
mod tests;
