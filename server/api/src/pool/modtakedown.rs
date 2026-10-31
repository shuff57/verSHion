//! The DMCA takedown and the rightful-item check (worker/src/pool/moderation.ts `takedown`, `rightfulCheck`). Cores return what the admin route needs; the
//! route layer flushes the author's notice from `pool_outbox` (`flush_outbox`) after the call, as the Worker does.
use super::blocking::{BlockPlan, selected_blobs, selected_key_pairs, selected_match};
use super::ctx::{AuditEntry, Fail, PoolCtx, PoolError, PoolNotice, Res, audit};
use super::modlineage::{Copy, LineageOpts, derivation_of, ghost_copies, latest_ghost, lineage_copies, move_bodies};
use super::poolio::outbox_insert;
use super::served::upstream_snapshot_sql;
use super::sqlfrag::{new_id, q_key_eq, visible_to_others};
use crate::blobs::BlobStore;
use rusqlite::{OptionalExtension, params};
use serde_json::{Value, json};

fn pool_err(status: u16, code: &str) -> Fail {
    PoolError::new(status, code).into()
}

/// The action's selected keys, as the upstream snapshot reads them.
fn block_keys_of(plan: &BlockPlan) -> Vec<Value> {
    selected_key_pairs(plan).into_iter().map(|(h, _)| json!({ "k": "match", "h": h })).chain(selected_blobs(plan).into_iter().map(|h| json!({ "k": "blob", "h": h }))).collect()
}
/// What a block row is written with: kind, hash and the qhash the key came from.
fn block_rows_of(plan: &BlockPlan) -> Vec<Value> {
    selected_key_pairs(plan).into_iter().map(|(h, q)| json!({ "k": "match", "h": h, "q": q })).chain(selected_blobs(plan).into_iter().map(|h| json!({ "k": "blob", "h": h, "q": h }))).collect()
}

pub struct TakedownOut {
    pub action_id: String,
    pub copies: Vec<Copy>,
}

/// A DMCA takedown: disabled at once, then the bodies go to quarantine/. An item purged before the notice is taken down by the SAME state machine as a live
/// one (the author is told, the 30-day window opens, the strike is pending, re-uploads of its questions are blocked meanwhile); nothing is left to quarantine.
pub fn takedown(x: &PoolCtx, store: &BlobStore, admin_id: &str, notice_id: &str, item_id: &str, plan: &BlockPlan, block_until: Option<i64>, exempt: &[String], rightful_item: Option<&str>) -> Res<TakedownOut> {
    let now = x.now;
    let notice: Option<String> = x.conn.query_row("SELECT status FROM dmca_notices WHERE id = ?1", [notice_id], |r| r.get(0)).optional()?;
    let Some(status) = notice else { return Err(pool_err(404, "notice_not_found")) };
    if status == "deficient" || status == "withdrawn" {
        return Err(pool_err(409, "notice_not_actionable"));
    }
    let item: Option<(String, Option<String>, Option<String>, Option<i64>)> = x
        .conn
        .query_row("SELECT author_id, current_publish_id, removed_reason, purge_after FROM items WHERE id = ?1 AND purged_at IS NULL", [item_id], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)))
        .optional()?;
    let rows = serde_json::to_string(&block_rows_of(plan)).unwrap();
    let keys = serde_json::to_string(&block_keys_of(plan)).unwrap();
    let exempt_json = json!(exempt).to_string();
    let Some((author_id, current_publish, removed_reason, purge_after)) = item else {
        let gone: Option<(String, Option<String>)> = x.conn.query_row("SELECT author_id, removed_reason FROM items WHERE id = ?1 AND purged_at IS NOT NULL", [item_id], |r| Ok((r.get(0)?, r.get(1)?))).optional()?;
        let Some((gone_author, gone_reason)) = gone else { return Err(pool_err(404, "not_found")) };
        let open: Option<(String, String)> = x
            .conn
            .query_row("SELECT id, notice_id FROM dmca_actions WHERE item_id = ?1 AND restored_at IS NULL AND purged_at IS NULL ORDER BY disabled_at DESC LIMIT 1", [item_id], |r| Ok((r.get(0)?, r.get(1)?)))
            .optional()?;
        if let Some((open_id, open_notice)) = &open {
            if open_notice == notice_id {
                let der = derivation_of(x, latest_ghost(x, item_id)?.as_deref(), true)?;
                x.conn.execute("UPDATE dmca_actions SET derivation_json = COALESCE(derivation_json, ?2) WHERE id = ?1", params![open_id, der])?;
                return Ok(TakedownOut { action_id: open_id.clone(), copies: ghost_copies(x, item_id, "", "")? });
            }
        }
        let any: bool = x.conn.query_row("SELECT EXISTS (SELECT 1 FROM dmca_actions WHERE item_id = ?1 AND restored_at IS NULL)", [item_id], |r| r.get(0))?;
        if open.is_some() || any {
            return Err(pool_err(409, "already_removed"));
        }
        // Exactly the plan's selection (made from the ghost by the live rules), with its exempt authors; nothing selected is refused.
        if plan.selection_required == Some(true) {
            return Err(PoolError::new(409, "selection_required")
                .with("message", json!("Select the questions (or images) to block, or send questions: \"all\" for the default; the preview lists what each would hold."))
                .into());
        }
        let action_id = new_id();
        let der = derivation_of(x, latest_ghost(x, item_id)?.as_deref(), true)?;
        let tx = x.conn.unchecked_transaction()?;
        tx.execute(
            "INSERT INTO dmca_actions (id, notice_id, item_id, author_id, disabled_at, author_notified_at, prev_removed_reason, rightful_item_id, derivation_json) VALUES (?1, ?2, ?3, ?4, ?5, ?5, ?6, ?7, ?8)",
            params![action_id, notice_id, item_id, gone_author, now, gone_reason, rightful_item, der],
        )?;
        // The tombstone says what it is now: the counter-notice route and the author's list read it.
        tx.execute("UPDATE items SET removed_reason = 'takedown' WHERE id = ?1 AND purged_at IS NOT NULL", [item_id])?;
        tx.execute("INSERT INTO strikes (id, user_id, action_id, at, upheld_at) VALUES (?1, ?2, ?3, ?4, NULL)", params![new_id(), gone_author, action_id, now])?;
        tx.execute("INSERT OR IGNORE INTO dmca_exempt (action_id, user_id, at) SELECT ?1, value, ?2 FROM json_each(?3) WHERE EXISTS (SELECT 1 FROM dmca_actions WHERE id = ?1)", params![action_id, now, exempt_json])?;
        // Re-uploads of what was selected are held for review meanwhile (as for a live item).
        tx.execute(BLOCK_ROWS_SQL, params![action_id, now, block_until, rows])?;
        // What the item it came from carried of them, now; later additions are not exempt.
        tx.execute(&upstream_snapshot_sql(), params![action_id, der, keys])?;
        tx.execute("UPDATE dmca_actions SET upstream_snap = 1 WHERE id = ?1", [&action_id])?;
        tx.execute("UPDATE dmca_notices SET status = 'actioned' WHERE id = ?1", [notice_id])?;
        // The author is told as for a live item, from the same transaction (an erased account has no address: the admin alone sees it).
        let safe: String = gone_author.chars().filter(|c| *c != '\'').collect();
        outbox_insert(&tx, &format!("takedown_author:{action_id}"), &PoolNotice::TakedownAuthor { item_id: item_id.into(), author_id: gone_author.clone(), action_id: action_id.clone() }, now, &format!("EXISTS (SELECT 1 FROM users WHERE id = '{safe}' AND deleted_at IS NULL)"))?;
        audit(&tx, &AuditEntry { actor_id: Some(admin_id), actor_kind: "admin", action: "dmca.takedown", item_id: Some(item_id), target_user_id: Some(&gone_author), detail: Some(json!({ "notice_id": notice_id, "action_id": action_id, "deleted_by_author": true })), note: None }, now, None)?;
        tx.commit()?;
        return Ok(TakedownOut { action_id: action_id.clone(), copies: ghost_copies(x, item_id, "", "")? });
    };
    // A retry of the same notice's takedown that stopped part way resumes it (the bodies' move is idempotent), never a 409 that leaves them unquarantined.
    if removed_reason.as_deref() == Some("takedown") {
        let open: Option<String> = x
            .conn
            .query_row("SELECT id FROM dmca_actions WHERE item_id = ?1 AND notice_id = ?2 AND restored_at IS NULL AND purged_at IS NULL ORDER BY disabled_at DESC LIMIT 1", params![item_id, notice_id], |r| r.get(0))
            .optional()?;
        if let Some(open_id) = open {
            let der = derivation_of(x, current_publish.as_deref(), false)?;
            x.conn.execute("UPDATE dmca_actions SET derivation_json = COALESCE(derivation_json, ?2) WHERE id = ?1", params![open_id, der])?;
            move_bodies(x, store, item_id, true)?;
            let sel = selected_match(plan);
            let blobs = selected_blobs(plan);
            return Ok(TakedownOut { action_id: open_id, copies: lineage_copies(x, item_id, Some(&sel), false, Some(&blobs), &LineageOpts::default())? });
        }
    }
    // A live item, or one its author deleted (the takedown takes over: undelete is refused while it stands, and a put-back returns it to "deleted", never
    // to live). Items removed by a privacy purge are already disabled for good.
    if !matches!(removed_reason.as_deref(), None | Some("author_deleted" | "admin" | "account_deleted")) {
        return Err(pool_err(409, "already_removed"));
    }
    let action_id = new_id();
    // The derivation is captured from the live publish now; after an uphold with no serve the item leaves no ghost, and the original's republish must not
    // be held.
    let der = derivation_of(x, current_publish.as_deref(), false)?;
    let tx = x.conn.unchecked_transaction()?;
    tx.execute(
        "UPDATE items SET removed_at = ?2, removed_reason = 'takedown', quarantined_at = ?2, purge_after = NULL, updated_at = ?2
          WHERE id = ?1 AND purged_at IS NULL AND (removed_reason IS NULL OR removed_reason IN ('author_deleted', 'admin', 'account_deleted'))",
        params![item_id, now],
    )?;
    tx.execute(
        "INSERT INTO dmca_actions (id, notice_id, item_id, author_id, disabled_at, author_notified_at, prev_removed_reason, prev_purge_after, rightful_item_id, derivation_json) SELECT ?3, ?4, ?1, ?5, ?2, ?2, ?6, ?7, ?8, ?9 WHERE changes() = 1",
        params![item_id, now, action_id, notice_id, author_id, removed_reason, removed_reason.as_ref().and(purge_after), rightful_item, der],
    )?;
    let guard = "EXISTS (SELECT 1 FROM dmca_actions WHERE id = ?3)";
    // A strike counts only once UPHELD: upheld_at is set then, never at takedown time.
    tx.execute(&format!("INSERT INTO strikes (id, user_id, action_id, at, upheld_at) SELECT ?5, ?4, ?3, ?2, NULL WHERE {guard}"), params![item_id, now, action_id, author_id, new_id()])?;
    // Authors the admin names as the rightful ones: the re-upload blocks never hold their publishes.
    tx.execute(&format!("INSERT OR IGNORE INTO dmca_exempt (action_id, user_id, at) SELECT ?3, value, ?2 FROM json_each(?4) WHERE {guard}"), params![item_id, now, action_id, exempt_json])?;
    tx.execute(BLOCK_ROWS_SQL, params![action_id, now, block_until, rows])?;
    tx.execute(&upstream_snapshot_sql(), params![action_id, der, keys])?;
    tx.execute("UPDATE dmca_actions SET upstream_snap = 1 WHERE id = ?1", [&action_id])?;
    tx.execute("UPDATE dmca_notices SET status = 'actioned' WHERE id = ?1 AND EXISTS (SELECT 1 FROM dmca_actions WHERE id = ?2 AND notice_id = ?1)", params![notice_id, action_id])?;
    outbox_insert(&tx, &format!("takedown_author:{action_id}"), &PoolNotice::TakedownAuthor { item_id: item_id.into(), author_id: author_id.clone(), action_id: action_id.clone() }, now, &format!("EXISTS (SELECT 1 FROM dmca_actions WHERE id = '{action_id}')"))?;
    audit(&tx, &AuditEntry { actor_id: Some(admin_id), actor_kind: "admin", action: "dmca.takedown", item_id: Some(item_id), target_user_id: Some(&author_id), detail: Some(json!({ "notice_id": notice_id, "action_id": action_id })), note: None }, now, None)?;
    tx.commit()?;
    let ok: bool = x.conn.query_row("SELECT EXISTS (SELECT 1 FROM dmca_actions WHERE id = ?1)", [&action_id], |r| r.get(0))?;
    if !ok {
        return Err(pool_err(409, "already_removed"));
    }
    move_bodies(x, store, item_id, true)?;
    // The same copies hold-copies would hold: by the designated questions, or (none designated) the whole item, never an empty designation.
    let sel = selected_match(plan);
    let blobs = selected_blobs(plan);
    let copies = lineage_copies(x, item_id, if sel.is_empty() { None } else { Some(&sel) }, false, Some(&blobs), &LineageOpts::default())?;
    Ok(TakedownOut { action_id, copies })
}

/// Re-uploads of the designated (previewed) questions and images are held for review, until the admin's chosen end or until lifted. A lifted older row is
/// taken over.
const BLOCK_ROWS_SQL: &str = "INSERT INTO blocked_hashes (kind, hash, action_id, at, expires_at, qhash) SELECT j.value ->> '$.k', j.value ->> '$.h', ?1, ?2, ?3, j.value ->> '$.q' FROM json_each(?4) j WHERE EXISTS (SELECT 1 FROM dmca_actions WHERE id = ?1)
   ON CONFLICT (kind, hash) DO UPDATE SET action_id = excluded.action_id, at = excluded.at, expires_at = excluded.expires_at, lifted_at = NULL, qhash = excluded.qhash WHERE blocked_hashes.lifted_at IS NOT NULL";

#[derive(Debug, PartialEq, Clone)]
pub struct HashSeq {
    pub kind: &'static str,
    pub hash: String,
    pub rightful_seq: Option<i64>,
    pub item_seq: Option<i64>,
    pub derived: bool,
    pub ok: bool,
}
#[derive(Debug, PartialEq)]
pub struct RightfulCheck {
    pub ok: bool,
    pub reasons: Vec<&'static str>,
    pub hashes: Vec<HashSeq>,
}

/// Is `rightful` a valid rightful item for material of `item_id`? It must be another author's live item visible to others, not derived from the infringing
/// item (no chain or origin naming it, its author never served it), and contain EVERY hash in `hashes` in a publish committed before the infringing item's
/// first publish containing it (per-content seq, never the item's age). With no hashes (no block), it must share at least one question with the infringing
/// item, each shared one earlier.
pub fn rightful_check(x: &PoolCtx, rightful: &str, item_id: &str, match_hashes: &[String], blob_hashes: &[String]) -> Res<RightfulCheck> {
    let mut reasons: Vec<&'static str> = Vec::new();
    let r: Option<(String, String, bool, Option<i64>)> = x
        .conn
        .query_row(
            &format!(
                "SELECT r.author_id, x.author_id, EXISTS (SELECT 1 FROM items i JOIN users u ON u.id = i.author_id WHERE i.id = r.id AND {}),
                        (SELECT MIN(COALESCE(im.first_at, im.at)) FROM imports im WHERE im.user_id = r.author_id AND im.item_id = x.id)
                   FROM items r JOIN items x ON x.id = ?2 WHERE r.id = ?1 AND r.purged_at IS NULL",
                visible_to_others()
            ),
            params![rightful, item_id],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
        )
        .optional()?;
    let Some((ra, xa, visible, served_at)) = r else { return Ok(RightfulCheck { ok: false, reasons: vec!["rightful_not_found"], hashes: vec![] }) };
    if rightful == item_id || ra == xa {
        reasons.push("same_author");
    }
    if !visible {
        reasons.push("rightful_not_visible");
    }
    let seqs = |kind: &'static str, list: &[String]| -> Res<Vec<HashSeq>> {
        if list.is_empty() {
            return Ok(vec![]);
        }
        let sql = if kind == "match" {
            // Per blocked hash, whether R's question with it came from X (an origin or chain naming X, or a serve of X before R's first publish containing
            // it): one copied question never disqualifies the rest of R.
            format!(
                "SELECT j.value,
                        (SELECT MIN(q.seq) FROM item_publishes p JOIN item_questions q ON q.publish_id = p.publish_id WHERE p.item_id = ?1 AND p.status IN ('current', 'previous') AND {k}),
                        (SELECT MIN(q.seq) FROM item_publishes p JOIN item_questions q ON q.publish_id = p.publish_id WHERE p.item_id = ?2 AND {k}),
                        (EXISTS (SELECT 1 FROM item_publishes p JOIN item_questions q ON q.publish_id = p.publish_id WHERE p.item_id = ?1 AND {k} AND (q.origin_item_id = ?2 OR instr(q.chain_json, ?2) > 0))
                         OR (?4 IS NOT NULL AND ?4 < (SELECT MIN(p.committed_at) FROM item_publishes p JOIN item_questions q ON q.publish_id = p.publish_id WHERE p.item_id = ?1 AND p.committed_at IS NOT NULL AND {k})))
                   FROM json_each(?3) j",
                k = q_key_eq("q", "j.value")
            )
        } else {
            "SELECT j.value,
                    (SELECT MIN(b.seq) FROM item_publishes p JOIN item_blobs b ON b.publish_id = p.publish_id WHERE p.item_id = ?1 AND p.status IN ('current', 'previous') AND b.sha256 = j.value),
                    (SELECT MIN(b.seq) FROM item_publishes p JOIN item_blobs b ON b.publish_id = p.publish_id WHERE p.item_id = ?2 AND b.sha256 = j.value),
                    0
               FROM json_each(?3) j"
                .to_string()
        };
        let mut st = x.conn.prepare(&sql)?;
        let out = st
            .query_map(params![rightful, item_id, json!(list).to_string(), served_at], |row| {
                let (rs, xs, derived): (Option<i64>, Option<i64>, bool) = (row.get(1)?, row.get(2)?, row.get(3)?);
                Ok(HashSeq { kind, hash: row.get(0)?, rightful_seq: rs, item_seq: xs, derived, ok: rs.is_some() && xs.is_none_or(|v| rs.unwrap() < v) && !derived })
            })?
            .collect::<Result<_, _>>()?;
        Ok(out)
    };
    let mut list = seqs("match", match_hashes)?;
    list.extend(seqs("blob", blob_hashes)?);
    if list.is_empty() {
        // No block: the questions the two items share, each earlier in the rightful item.
        let mut st = x.conn.prepare(
            "SELECT DISTINCT xq.match_hash FROM item_publishes xp JOIN item_questions xq ON xq.publish_id = xp.publish_id
              WHERE xp.item_id = ?2 AND EXISTS (SELECT 1 FROM item_publishes rp JOIN item_questions rq ON rq.publish_id = rp.publish_id WHERE rp.item_id = ?1 AND rq.match_hash = xq.match_hash) LIMIT 1001",
        )?;
        let shared: Vec<String> = st.query_map(params![rightful, item_id], |row| row.get(0))?.collect::<Result<_, _>>()?;
        drop(st);
        // More shared questions than one check covers is never a pass.
        if shared.len() > 1000 {
            reasons.push("too_many_shared_questions");
        }
        list = seqs("match", &shared[..shared.len().min(1000)])?;
        if list.is_empty() {
            reasons.push("no_shared_material");
        }
    }
    if list.iter().any(|h| h.derived) {
        reasons.push("rightful_derived_from_item");
    }
    if list.iter().any(|h| !h.ok && !h.derived) {
        reasons.push("not_earlier_in_rightful");
    }
    list.truncate(200);
    Ok(RightfulCheck { ok: reasons.is_empty(), reasons, hashes: list })
}

#[cfg(test)]
#[path = "modtakedown_tests.rs"]
mod tests;
