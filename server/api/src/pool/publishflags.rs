//! The admin flags one publish raises (worker/src/pool/publish.ts 2546-2650): one flag per kind per publish (a count and the first few positions, never one per
//! question), and the repeated-independence-claim counts they depend on. Past FLAGS_PER_ACCOUNT_DAY rows of a kind a day, the kind folds into one aggregated
//! row with a count; integrity kinds never fold, `origin_unverified` always does. Nothing is dropped.
//! (Detail objects are written with sorted keys; readers parse them, the Worker's insertion order was never relied on.)
use super::ctx::{PoolCtx, Res};
use super::publishchecks::Blocked;
use super::publishderive::Derived;
use super::publishmeta::PublishMeta;
use super::sqlfrag::{COLLAPSED_FLAGS, DAY_MS, FLAGS_PER_ACCOUNT_DAY, FLAG_DETAIL_MAX, INTEGRITY_FLAGS, new_id};
use rusqlite::params;
use serde_json::{Value, json};
use std::collections::{HashMap, HashSet};

#[derive(Debug, Clone, PartialEq)]
pub struct Flag {
    pub id: String,
    pub kind: String,
    pub detail: String,
    /// Set on a folded flag: the aggregate row it joins.
    pub dedup: Option<String>,
}

/// `{count, first}`: the number of entries and the first few.
pub fn few<T: serde::Serialize>(xs: &[T]) -> Value {
    json!({ "count": xs.len(), "first": xs.iter().take(FLAG_DETAIL_MAX).collect::<Vec<_>>() })
}

/// How many distinct items the account claimed independence of in the last 30 days, when this publish names one it never claimed before (else 0): a claim
/// the account already made, about the same items (its own bank maintenance), is not a new one. `detail_key` is `independent_of` or `circle_independent_of`.
pub fn prior_claims(x: &PoolCtx, me: &str, named: &[String], detail_key: &str) -> Res<i64> {
    if named.is_empty() {
        return Ok(0);
    }
    let path = format!("$.{detail_key}");
    let mut before: HashSet<String> = HashSet::new();
    let mut st = x.conn.prepare(
        "SELECT DISTINCT v.value AS id FROM pool_audit a, json_each(json_extract(a.detail, ?2)) v
          WHERE a.actor_id = ?1 AND a.action IN ('item.publish', 'item.republish') AND json_extract(a.detail, ?2) IS NOT NULL",
    )?;
    for r in st.query_map(params![me, path], |r| r.get::<_, String>(0))? {
        before.insert(r?);
    }
    if !named.iter().any(|n| !before.contains(n)) {
        return Ok(0);
    }
    Ok(x.conn.query_row(
        "SELECT COUNT(DISTINCT v.value) AS n FROM pool_audit a, json_each(json_extract(a.detail, ?3)) v
          WHERE a.actor_id = ?1 AND a.action IN ('item.publish', 'item.republish') AND a.at > ?2 AND json_extract(a.detail, ?3) IS NOT NULL",
        params![me, x.now - 30 * DAY_MS, path],
        |r| r.get(0),
    )?)
}

pub struct FlagIn<'a> {
    pub me: &'a str,
    pub m: &'a PublishMeta,
    pub d: &'a Derived,
    /// Visible items with the same question under a licence that does not allow this one.
    pub soft: &'a [(String, String)],
    pub blocked: Option<&'a Blocked>,
    pub suspect: &'a [usize],
    pub privacy_near: &'a [usize],
    /// Evidence overflow: every position past a cap, including the author's own near-rewordings' cuts.
    pub overflowed: &'a [usize],
    pub circle_claimed: &'a [String],
    pub circle_statement: Option<&'a str>,
}

/// The flags to write, as (kept, folded).
pub fn build(x: &PoolCtx, p: &FlagIn) -> Res<(Vec<Flag>, Vec<Flag>)> {
    let (m, d) = (p.m, p.d);
    let prior = prior_claims(x, p.me, &m.independent, "independent_of")? + prior_claims(x, p.me, p.circle_claimed, "circle_independent_of")?;
    let license = &m.license;
    let mut all: Vec<(&str, Value)> = vec![];
    if prior >= 2 {
        all.push(("independent_repeat", json!({ "claims_in_30_days": prior + 1 })));
    }
    if !p.circle_claimed.is_empty() {
        all.push(("independent_claim", json!({ "circle_banks": few(p.circle_claimed), "statement": p.circle_statement })));
    }
    if !d.soft_only.is_empty() || !p.soft.is_empty() {
        let ids: Vec<&String> = p.soft.iter().map(|(id, _)| id).collect();
        all.push(("license_soft", json!({ "positions": few(&d.soft_only), "matches_items": few(&ids), "license": license })));
    }
    if !d.conflicts.is_empty() {
        all.push(("license_conflict", json!({ "positions": few(&d.conflicts), "license": license })));
    }
    if !d.grandfathered.is_empty() {
        all.push(("license_grandfathered", json!({ "positions": few(&d.grandfathered), "license": license })));
    }
    if !d.unverified.is_empty() {
        all.push(("origin_unverified", few(&d.unverified)));
    }
    if !d.declared_modified.is_empty() {
        all.push(("origin_modified", few(&d.declared_modified)));
    }
    if !p.suspect.is_empty() {
        all.push(("origin_suspect", few(p.suspect)));
    }
    if let Some(b) = p.blocked {
        all.push(("blocklist", json!({ "hash_kind": b.kind })));
    }
    if !m.independent.is_empty() {
        all.push(("independent_claim", json!({ "items": few(&m.independent), "statement": m.statement })));
    }
    if !p.overflowed.is_empty() {
        all.push(("evidence_overflow", few(p.overflowed)));
    }
    if !p.privacy_near.is_empty() {
        all.push(("privacy_text_match", json!({ "near": few(p.privacy_near) })));
    }
    // Rows per (account, kind, day).
    let day_start = x.now.div_euclid(DAY_MS) * DAY_MS;
    let mut per_kind: HashMap<String, i64> = HashMap::new();
    let mut st = x.conn.prepare("SELECT kind, COUNT(*) AS n FROM admin_flags WHERE user_id = ?1 AND at >= ?2 AND dedup IS NULL GROUP BY kind")?;
    for r in st.query_map(params![p.me, day_start], |r| Ok((r.get::<_, String>(0)?, r.get::<_, i64>(1)?)))? {
        let (k, n) = r?;
        per_kind.insert(k, n);
    }
    let folds = |k: &str| COLLAPSED_FLAGS.contains(&k) || (!INTEGRITY_FLAGS.contains(&k) && per_kind.get(k).copied().unwrap_or(0) >= FLAGS_PER_ACCOUNT_DAY);
    let (mut kept, mut folded) = (vec![], vec![]);
    for (kind, detail) in all {
        let detail = detail.to_string();
        if folds(kind) {
            folded.push(Flag { id: new_id(), kind: kind.into(), detail, dedup: Some(format!("agg:{kind}:{}:{}", p.me, day_start / DAY_MS)) });
        } else {
            kept.push(Flag { id: new_id(), kind: kind.into(), detail, dedup: None });
        }
    }
    Ok((kept, folded))
}

#[cfg(test)]
#[path = "publishflags_tests.rs"]
mod tests;
