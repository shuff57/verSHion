//! What follows a won commit, and the dry run's answer (worker/src/pool/publish.ts 2279-2307 and 2913-2967): the held-publish charge, retention of old
//! publishes, the privacy alert, and the dry-run response a publish with `dry_run` returns instead of storing anything.
use super::chain::ChainLink;
use super::ctx::{PoolCtx, PoolNotice, Res};
use super::licensecheck::{license_check, stored_allowed};
use super::publishcommit::HOLD_RELEASE_MS;
use super::publishdel::delete_publishes;
use super::publishderive::Derived;
use super::publishhash::Hashed;
use super::publishinsert::with_marks;
use super::publishmeta::PublishMeta;
use super::served::{rightful_exempts, upstream_carried};
use super::sqlfrag::{alive_sql, day_of, q_key_eq};
use crate::blobs::BlobStore;
use rusqlite::params;
use serde_json::{Value, json};
use std::collections::HashMap;

/// Deletes every publish of `item_id` that is neither current, nor the previous one, nor the one non-authors are served (`public_publish_id`:
/// hidden-only republishes keep serving an older publish, whose rows, search words and body must stay).
pub fn retain(x: &PoolCtx, store: &BlobStore, item_id: &str) -> Res<()> {
    let mut st = x.conn.prepare(
        "SELECT p.publish_id, p.full_key, p.stripped_key FROM item_publishes p JOIN items i ON i.id = p.item_id
          WHERE p.item_id = ?1 AND p.status = 'previous' AND p.publish_id IS NOT i.prev_publish_id AND p.publish_id IS NOT i.public_publish_id",
    )?;
    let old: Vec<(String, String, Option<String>)> = st.query_map([item_id], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))?.collect::<rusqlite::Result<_>>()?;
    if old.is_empty() {
        return Ok(());
    }
    let tx = x.conn.unchecked_transaction()?;
    delete_publishes(&tx, &old.iter().map(|o| o.0.clone()).collect::<Vec<_>>())?;
    tx.commit()?;
    for (_, full, stripped) in &old {
        for k in std::iter::once(full).chain(stripped) {
            let _ = store.delete(k);
        }
    }
    Ok(())
}

/// Charged PER QUESTION that a block would hold (privacy, or an active copyright block, by any key; and near-rewordings of blocked student-data text), whatever
/// else happens. Never charged: what an exemption covers (the rightful author and those served the rightful item, exempt authors, an admin's release of this
/// item): honest work after a takedown is never throttled.
pub fn held_charge(x: &PoolCtx, me: &str, item_id: &str, publish_id: &str, privacy_near: usize) -> Res<i64> {
    let sql = format!(
        "SELECT COUNT(*) AS n FROM item_questions iq WHERE iq.publish_id = ?3 AND (
           EXISTS (SELECT 1 FROM privacy_blocks pb WHERE pb.kind = 'match' AND {pk}
                     AND NOT EXISTS (SELECT 1 FROM hold_releases hr, json_each(hr.keys_json) r WHERE hr.item_id = ?6 AND hr.source = 'privacy' AND r.value = pb.hash AND hr.at > ?4 - {HOLD_RELEASE_MS} AND pb.at <= hr.at))
           OR EXISTS (SELECT 1 FROM blocked_hashes bh WHERE bh.kind = 'match' AND bh.lifted_at IS NULL AND (bh.expires_at IS NULL OR bh.expires_at > ?4) AND {bk}
                     AND NOT EXISTS (SELECT 1 FROM dmca_exempt e WHERE e.action_id = bh.action_id AND e.user_id = ?1)
                     AND NOT {carried}
                     AND NOT EXISTS (SELECT 1 FROM dmca_actions a WHERE a.id = bh.action_id AND a.rightful_item_id IS NOT NULL AND {exempt})
                     AND NOT EXISTS (SELECT 1 FROM hold_releases hr, json_each(hr.keys_json) r WHERE hr.item_id = ?6 AND hr.source = 'dmca:' || bh.action_id AND r.value = bh.hash AND hr.at > ?4 - {HOLD_RELEASE_MS} AND bh.at <= hr.at)))",
        pk = q_key_eq("iq", "pb.hash"),
        bk = q_key_eq("iq", "bh.hash"),
        carried = upstream_carried("bh.action_id", "?6", "bh.hash"),
        exempt = rightful_exempts("a.rightful_item_id", "bh", "?1", "?3"),
    );
    let n: i64 = x.conn.query_row(&sql, params![me, day_of(x.now), publish_id, x.now, 0, item_id], |r| r.get(0))?;
    Ok(privacy_near as i64 + n)
}

/// After a won commit: the held-publish charge (past the cap the item is held for review whatever happens to the blocks later), retention, and the privacy
/// alert when the commit raised a privacy flag.
pub fn after_commit(x: &PoolCtx, store: &BlobStore, me: &str, item_id: &str, publish_id: &str, privacy_near: usize, over_cap: bool) -> Res<()> {
    let charged = held_charge(x, me, item_id, publish_id, privacy_near)?;
    if charged > 0 {
        x.conn.execute(
            &format!("INSERT INTO usage_daily (user_id, day, metric, count) SELECT ?1, ?2, 'held_publishes', ?3 WHERE {} ON CONFLICT (user_id, day, metric) DO UPDATE SET count = count + excluded.count", alive_sql("?1")),
            params![me, day_of(x.now), charged],
        )?;
        x.conn.execute("INSERT OR IGNORE INTO item_holds (item_id, reason, at, by, source) SELECT ?1, 'admin', ?2, NULL, 'probe_cap' WHERE ?3 = 1", params![item_id, x.now, over_cap as i64])?;
    }
    retain(x, store, item_id)?;
    if x.conn.query_row("SELECT 1 FROM admin_flags WHERE publish_id = ?1 AND kind = 'privacy_text_match'", [publish_id], |_| Ok(())).is_ok() {
        let held = x.conn.query_row("SELECT 1 FROM item_holds WHERE item_id = ?1 AND source = 'privacy'", [item_id], |_| Ok(())).is_ok();
        x.alert_budgeted(PoolNotice::PrivacyTextMatch { item_id: item_id.into(), held, circle_id: None, version: None })?;
    }
    Ok(())
}

/// The dry run's answer: everything a publish would have refused has been refused; this says what students would be served and how the licence check reads.
pub fn dry_response(m: &PublishMeta, h: &Hashed, d: &Derived, n: usize) -> Value {
    let st = h.prep.stripped.as_ref();
    let served_text: Option<String> = match (&h.reused, st) {
        (Some(r), _) => Some(String::from_utf8_lossy(&r.bytes).into_owned()),
        (None, Some(s)) => Some(s.v.json()),
        _ => None,
    };
    let chains: Vec<Vec<ChainLink>> = d.entries.iter().map(|e| e.chain.clone()).collect();
    let stored: Vec<_> = d.allowed_json.iter().enumerate().map(|(i, a)| stored_allowed(Some(&with_marks(a, d.marks[i].soft, d.marks[i].claimed, d.marks[i].earlier)))).collect();
    let lc = license_check(&m.license, &chains, &stored, &HashMap::new());
    json!({
        "ok": true,
        "dry_run": true,
        "hide_key": m.hide_key,
        // What students are served: the exact stripped payload of a hidden-key publish (the shuffle of a real publish is drawn again: the orders are never
        // returned here); otherwise the full form.
        "student_view": served_text.and_then(|t| serde_json::from_str::<Value>(&t).ok()).unwrap_or_else(|| json!({ "questions": n, "key_shown": true })),
        "notes": h.prep.full.dropped,
        "label_findings": st.map(|s| s.label_findings.clone()).unwrap_or_default(),
        "labels_removed": st.map(|s| json!(s.labels_removed)).unwrap_or(json!([])),
        "images_removed": st.map(|s| json!(s.images_removed)).unwrap_or(json!([])),
        "figures_removed": st.map(|s| json!(s.figures_removed)).unwrap_or(json!([])),
        "order_reasons": st.map(|s| s.order_reasons.clone()).unwrap_or_default(),
        "license_check": lc,
        "chains": chains,
    })
}

#[cfg(test)]
#[path = "publishafter_tests.rs"]
mod tests;
