//! The first half of publish.ts's "stage" section (2232-2345): the limits a publish must pass before anything is stored (the tag cap, the account's record
//! cap, the storage / removed-storage / shared-question quotas as the staging INSERT checks them) and the publish sequence numbers it writes (one atomic
//! counter step; a kept qid keeps its seq only while its content is unchanged; a text unit and an image have their own).
use super::ctx::{PoolCtx, PoolError, Res};
use super::publishfacts::Facts;
use super::publishhash::Hashed;
use super::publishmeta::PublishMeta;
use super::publishrewrite::Rewritten;
use super::publishunits::{UnitKind, Units};
use super::served::served_form;
use super::sqlfrag::{REMOVED_SQL, shared_sql};
use rusqlite::params;
use serde_json::json;
use std::collections::{BTreeMap, HashMap, HashSet};

/// Distinct tags (with the merged topics) an item may have; more is an error, never silently dropped.
pub const MAX_TAGS: usize = 200;
/// A fix of a held item is exempt from the record cap up to this many new records.
pub const HELD_FIX_RECORDS: i64 = 1000;
/// Rows of per-part keys one publish writes.
pub const PART_KEYS_MAX: usize = 4000;

#[derive(Debug, Clone, Copy)]
pub struct Limits {
    pub quota: i64,
    pub shared_cap: i64,
    pub removed_cap: i64,
}

pub fn limits(x: &PoolCtx, me: &str) -> Res<Limits> {
    Ok(Limits { quota: x.limit_of(me, "storage_bytes")?, shared_cap: x.limit_of(me, "shared_questions")?, removed_cap: x.limit_of(me, "removed_bytes")? })
}

pub fn check_tags(count: usize) -> Res<()> {
    if count > MAX_TAGS {
        return Err(PoolError::new(422, "too_many_tags").with("limit", json!(MAX_TAGS)).with("message", json!("Too many distinct tags and topics in this item.")).into());
    }
    Ok(())
}

/// The durable records this publish would write, as (kind, hash, match_hash): what the commit writes, less those kept.
pub fn record_keys(m: &PublishMeta, h: &Hashed, units: &Units) -> Vec<[String; 3]> {
    let mut out: Vec<[String; 3]> = vec![];
    let mut seen: HashSet<String> = HashSet::new();
    let mut add = |k: &str, hash: Option<&String>, mh: &str| {
        if let Some(hv) = hash.filter(|s| !s.is_empty()) {
            if seen.insert(format!("{k}|{hv}|{mh}")) {
                out.push([k.to_string(), hv.clone(), mh.to_string()]);
            }
        }
    };
    for (i, mh) in h.match_hashes.iter().enumerate() {
        let near = if m.hide_key { h.near_stripped.as_ref().and_then(|v| v.get(i)).and_then(|s| s.as_ref()) } else { h.near_full.get(i) };
        add("nshown", near, mh);
        add("kf", h.near_kf.get(i), mh);
        if !m.hide_key {
            add("shown", h.full_content.get(i), mh);
        }
    }
    for u in units.units_at.iter().flatten() {
        add("unit", Some(&u.key), "");
    }
    out
}

/// Only NEW records count against the account's `author_records` (a republish that adds nothing new never does), and never for a fix of a held item, up to
/// HELD_FIX_RECORDS.
pub fn check_records(x: &PoolCtx, me: &str, item_id: &str, is_new: bool, keys: &[[String; 3]]) -> Res<()> {
    let held = !is_new && x.conn.query_row("SELECT 1 FROM item_holds WHERE item_id = ?1 LIMIT 1", [item_id], |_| Ok(())).is_ok();
    let (recs, fresh): (i64, i64) = x.conn.query_row(
        "SELECT (SELECT COUNT(*) FROM author_contents WHERE author_id = ?1) AS recs,
                (SELECT COUNT(*) FROM json_each(?2) k
                  WHERE NOT EXISTS (SELECT 1 FROM author_contents ac WHERE ac.author_id = ?1 AND ac.kind = k.value ->> '$[0]' AND ac.hash = k.value ->> '$[1]' AND ac.match_hash = k.value ->> '$[2]')) AS fresh",
        params![me, json!(keys).to_string()],
        |r| Ok((r.get(0)?, r.get(1)?)),
    )?;
    let cap = x.limit_of(me, "author_records")?;
    if fresh > 0 && recs + fresh > cap && !(held && fresh <= HELD_FIX_RECORDS) {
        return Err(PoolError::new(413, "author_records_exceeded")
            .with("used", json!(recs))
            .with("limit", json!(cap))
            .with(
                "message",
                json!("Your account's record of the questions it has published is full. Republishing your existing items still works; ask an admin to raise the limit (author_records) to publish new questions."),
            )
            .into());
    }
    Ok(())
}

/// The three quota refusals, in the order the staging INSERT's conditions are reported (also the dry run's last checks).
pub fn check_quotas(x: &PoolCtx, me: &str, item_id: &str, charge: i64, n: usize, l: &Limits) -> Res<()> {
    let used = super::blobs::usage(x, me)?;
    if used + charge > l.quota {
        return Err(PoolError::new(413, "quota_exceeded").with("used", json!(used)).with("needed", json!(charge)).with("limit", json!(l.quota)).into());
    }
    let removed: i64 = x.conn.query_row(&format!("SELECT {REMOVED_SQL} AS n"), [me], |r| r.get(0))?;
    if removed > l.removed_cap {
        return Err(PoolError::new(413, "removed_storage_exceeded")
            .with("used", json!(removed))
            .with("limit", json!(l.removed_cap))
            .with("message", json!("Deleted items still hold storage until they are purged (14 days). Undo a deletion or wait."))
            .into());
    }
    let qn: i64 = x.conn.query_row(&format!("SELECT {} AS n", shared_sql("?1", "?2")), params![me, item_id], |r| r.get(0))?;
    if qn + n as i64 > l.shared_cap {
        return Err(PoolError::new(422, "shared_questions_limit").with("used", json!(qn)).with("needed", json!(n)).with("limit", json!(l.shared_cap)).into());
    }
    Ok(())
}

#[derive(Debug, Default)]
pub struct Seqs {
    pub new_seq: i64,
    pub q_seq: Vec<i64>,
    pub unit_seqs: Vec<BTreeMap<String, i64>>,
    /// Every unit key of each question with its seq (a text's own; the rest the question's).
    pub unit_record: Vec<(String, i64)>,
    /// The seq of each served image in the base publish.
    pub blob_carry: HashMap<String, i64>,
}

impl Seqs {
    /// An image keeps the seq of the first publish of THIS item that served it.
    pub fn blob_seq(&self, served: &HashSet<&str>, sha: &str) -> i64 {
        if served.contains(sha) { self.blob_carry.get(sha).copied().unwrap_or(self.new_seq) } else { self.new_seq }
    }
}

/// One atomic increment of the counter (never a MAX()+1 over rows that can be deleted), then the seq of every question, text unit and image.
pub fn sequences(x: &PoolCtx, rw: &Rewritten, h: &Hashed, f: &Facts, units: &Units, base_of: Option<&str>) -> Res<Seqs> {
    let new_seq: i64 = x.conn.query_row("UPDATE pool_seq SET n = n + 1 WHERE id = 1 RETURNING n", [], |r| r.get(0)).unwrap_or(1);
    let q_seq: Vec<i64> = rw
        .qids
        .iter()
        .enumerate()
        .map(|(i, q)| match f.prev.get(q) {
            Some(pv) if pv.match_hash == h.match_hashes[i] && pv.seq > 0 => pv.seq,
            _ => new_seq,
        })
        .collect();
    // A TEXT unit has its OWN seq: carried only when this very key was in the previous publish of the qid, else this publish's.
    let unit_seqs: Vec<BTreeMap<String, i64>> = rw
        .qids
        .iter()
        .enumerate()
        .map(|(i, q)| {
            let was: HashMap<String, i64> = f.prev.get(q).and_then(|pv| pv.unit_seqs.as_deref()).and_then(|s| serde_json::from_str(s).ok()).unwrap_or_default();
            f.served_text[i].iter().chain(&f.hidden_text[i]).map(|k| (k.clone(), was.get(k).copied().filter(|s| *s > 0).unwrap_or(new_seq))).collect()
        })
        .collect();
    let unit_record: Vec<(String, i64)> = units
        .units_at
        .iter()
        .enumerate()
        .flat_map(|(i, us)| us.iter().map(|u| (u.key.clone(), if u.kind == UnitKind::Text { unit_seqs[i].get(&u.key).copied().unwrap_or(new_seq) } else { q_seq[i] })).collect::<Vec<_>>())
        .collect();
    let mut blob_carry = HashMap::new();
    if let Some(base) = base_of {
        let mut st = x.conn.prepare(&format!(
            "SELECT ib.sha256, MIN(ib.seq) AS seq FROM item_blobs ib JOIN item_publishes p ON p.publish_id = ib.publish_id WHERE ib.publish_id = ?1 AND {} GROUP BY ib.sha256",
            served_form("ib", "p")
        ))?;
        for r in st.query_map([base], |r| Ok((r.get::<_, String>(0)?, r.get::<_, i64>(1)?)))? {
            let (sha, seq) = r?;
            if seq > 0 {
                blob_carry.insert(sha, seq);
            }
        }
    }
    Ok(Seqs { new_seq, q_seq, unit_seqs, unit_record, blob_carry })
}

#[cfg(test)]
#[path = "publishstage_tests.rs"]
mod tests;
