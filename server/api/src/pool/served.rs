//! THE served-blobs rule (worker/src/pool/served.ts). Every derivation that creates, keeps or lifts state a non-author can
//! observe (holds, blocks, unblocks, needs_reupload, the cron byte purge, the unremove re-hold, orphan releases, block
//! plans and their affected lists) looks at an item's images ONLY through these helpers: the blobs of the form its publish
//! SERVES to non-authors. For a hidden-key publish that is the stripped form (which lists only the images its author
//! ticked); otherwise the full form. An image that only a hidden publish's full form carries is invisible to all of them,
//! so no guess at it can move anything a non-author sees. The author's own views (the full form, quota) do not use them.
use super::sqlfrag::{in_q_keys_of, q_key_eq, visible_to_others};

/// SQL condition: the item_blobs row `ib` is in the form that publish `p` serves to non-authors.
pub fn served_form(ib: &str, p: &str) -> String {
    format!("{ib}.form = CASE WHEN {p}.hide_key = 1 THEN 'stripped' ELSE 'full' END")
}

/// SQL subquery: the served image hashes of the publish whose id is the SQL expression `pub_`.
pub fn served_blobs_of(pub_: &str) -> String {
    format!("(SELECT sb.sha256 FROM item_blobs sb JOIN item_publishes sp ON sp.publish_id = sb.publish_id WHERE sb.publish_id = {pub_} AND {})", served_form("sb", "sp"))
}

/// SQL condition: some retained publish of the item whose id is the SQL expression `item` serves an image satisfying
/// `cond` (a condition on the column reference it is given).
pub fn item_serves_blob(item: &str, cond: impl Fn(&str) -> String) -> String {
    format!(
        "EXISTS (SELECT 1 FROM item_publishes sp JOIN item_blobs sb ON sb.publish_id = sp.publish_id
            WHERE sp.item_id = {item} AND {} AND {})",
        served_form("sb", "sp"),
        cond("sb.sha256")
    )
}

/// SQL condition: the CURRENT publish of item row `i` serves an image satisfying `cond`.
pub fn current_serves_blob(i: &str, cond: impl Fn(&str) -> String) -> String {
    format!(
        "EXISTS (SELECT 1 FROM item_publishes sp JOIN item_blobs sb ON sb.publish_id = sp.publish_id
            WHERE sp.publish_id = {i}.current_publish_id AND {} AND {})",
        served_form("sb", "sp"),
        cond("sb.sha256")
    )
}

/// SQL condition: the image `sha` (a SQL expression) is used somewhere NOT served: only in the full form of a retained
/// hidden publish. Used by the cron byte purge to keep bytes that are an author's private material, and never anywhere a
/// non-author could observe it.
pub fn used_unserved(sha: &str) -> String {
    format!(
        "EXISTS (SELECT 1 FROM item_blobs ub JOIN item_publishes up ON up.publish_id = ub.publish_id JOIN items ui ON ui.id = up.item_id
            WHERE ub.sha256 = {sha} AND ub.form = 'full' AND up.hide_key = 1 AND up.status IN ('current', 'previous') AND ui.purged_at IS NULL
              AND NOT EXISTS (SELECT 1 FROM item_blobs us WHERE us.publish_id = ub.publish_id AND us.form = 'stripped' AND us.sha256 = ub.sha256))"
    )
}

/// SQL condition: the rightful item whose id is the SQL expression `item` itself contains the blocked hash row `bh` (a
/// question of a retained publish, or an image it serves). A takedown that names the rightful item exempts exactly that
/// material, never a hash merely because the publisher has some link to the rightful item.
pub fn rightful_has(item: &str, bh: &str) -> String {
    format!(
        "(({bh}.kind = 'match' AND EXISTS (SELECT 1 FROM item_publishes rp JOIN item_questions rq ON rq.publish_id = rp.publish_id
                                      WHERE rp.item_id = {item} AND rp.status IN ('current', 'previous') AND {}))
    OR ({bh}.kind = 'blob' AND {}))",
        q_key_eq("rq", &format!("{bh}.hash")),
        item_serves_blob(item, |sha| format!("{sha} = {bh}.hash"))
    )
}

/// SQL condition: a takedown's rightful item `r` exempts the blocked hash row `bh` from holding a publish by `publisher`
/// (publish id `pub_`) only when the rightful item itself contains it (rightful_has), and the publisher is the rightful
/// item's author (always: their own material), or was served it (an import or read record, or a question whose origin or
/// chain names it) while it is visible to others (never through an unlisted, circle-only, removed or held one). Never an
/// arbitrary publisher of the content.
pub fn rightful_exempts(r: &str, bh: &str, publisher: &str, pub_: &str) -> String {
    format!(
        "({}
    AND (EXISTS (SELECT 1 FROM items ra WHERE ra.id = {r} AND ra.author_id = {publisher})
         OR (EXISTS (SELECT 1 FROM items i JOIN users u ON u.id = i.author_id WHERE i.id = {r} AND {})
             -- Never the author of the taken-down copy.
             AND NOT EXISTS (SELECT 1 FROM dmca_actions xa JOIN items xi ON xi.id = xa.item_id WHERE xa.id = {bh}.action_id AND xi.author_id = {publisher})
             -- Served it: an import record, or a chain link that is derivation evidence (never an
             -- \"earlier identical publication\" credit or a claimed-independent possible source).
             AND (EXISTS (SELECT 1 FROM imports im WHERE im.user_id = {publisher} AND im.item_id = {r})
                  OR EXISTS (SELECT 1 FROM item_questions q2 WHERE q2.publish_id = {pub_}
                               AND (q2.origin_item_id = {r}
                                    OR EXISTS (SELECT 1 FROM json_each(q2.chain_json) cl WHERE cl.value ->> '$.item_id' = {r}
                                                 AND COALESCE(cl.value ->> '$.earlier_match', 0) = 0 AND COALESCE(cl.value ->> '$.possible_source', 0) = 0)))))))",
        rightful_has(r, bh),
        visible_to_others()
    )
}

/// SQL condition: the item whose id is the SQL expression `item` is the item the action `action` took down came FROM: the
/// taken-down publish names it as an origin or carries it as a derivation chain link (never an `earlier_match` credit or a
/// claimed-independent possible source; a ghost source publish is read through its ghost chain). Its material predates the
/// block: it is the original, not a copy of its own copy, and holding it would punish the author of the work.
///
/// There is deliberately NO "the taken-down item's author imported it" branch. A plain GET is recorded as an import row
/// too, so that branch let an infringer shield any copy (or an accomplice's copy) from hold-copies with one read of it. The
/// server-recorded origin and chain are the only derivation evidence.
pub fn upstream_of(action: &str, item: &str) -> String {
    format!(
        "(EXISTS (SELECT 1 FROM dmca_actions ua JOIN json_each(COALESCE(ua.derivation_json, '[]')) d ON d.value = {item}
            WHERE ua.id = {action})
   OR EXISTS (SELECT 1 FROM dmca_actions ua JOIN items ui ON ui.id = ua.item_id
             JOIN item_questions uq ON uq.publish_id = ui.current_publish_id
            WHERE ua.id = {action} AND (uq.origin_item_id = {item}
              OR EXISTS (SELECT 1 FROM json_each(uq.chain_json) cl WHERE cl.value ->> '$.item_id' = {item}
                           AND COALESCE(cl.value ->> '$.earlier_match', 0) = 0 AND COALESCE(cl.value ->> '$.possible_source', 0) = 0)))
   OR EXISTS (SELECT 1 FROM dmca_actions ua3 JOIN ghost_publishes gp ON gp.item_id = ua3.item_id
                JOIN ghost_questions gq ON gq.publish_id = gp.publish_id
               WHERE ua3.id = {action} AND gp.at = (SELECT MAX(g2.at) FROM ghost_publishes g2 WHERE g2.item_id = ua3.item_id)
                 AND EXISTS (SELECT 1 FROM json_each(gq.chain_json) cl WHERE cl.value ->> '$.item_id' = {item}
                               AND COALESCE(cl.value ->> '$.earlier_match', 0) = 0 AND COALESCE(cl.value ->> '$.possible_source', 0) = 0)))"
    )
}

/// SQL condition: the item `item` is the upstream of the action `action` (upstream_of) AND it CARRIED the blocked hash
/// `hash` when the action was made (the snapshot in dmca_upstream_keys; for an action made before the snapshot existed, a
/// publish of it committed by the action's time). Material the upstream adds after the block is not exempt: held like any
/// re-upload.
pub fn upstream_carried(action: &str, item: &str, hash: &str) -> String {
    format!(
        "(EXISTS (SELECT 1 FROM dmca_upstream_keys uk WHERE uk.action_id = {action} AND uk.item_id = {item} AND uk.key = {hash})
    OR (EXISTS (SELECT 1 FROM dmca_actions la WHERE la.id = {action} AND la.upstream_snap = 0) AND {}
        AND EXISTS (SELECT 1 FROM item_publishes lp JOIN dmca_actions la2 ON la2.id = {action}
                     WHERE lp.item_id = {item} AND lp.committed_at <= la2.disabled_at
                       AND ({} OR {hash} IN {}))))",
        upstream_of(action, item),
        in_q_keys_of(hash, "lp.publish_id"),
        served_blobs_of("lp.publish_id")
    )
}

/// The snapshot itself, written in the action's own batch: of the action's selected keys (`?3`: [{k: "match"|"blob", h}]),
/// those each upstream item (`?2`: item ids) carried then, in a retained publish or a ghost. Binds `?1` = action id.
pub fn upstream_snapshot_sql() -> String {
    format!(
        "INSERT OR IGNORE INTO dmca_upstream_keys (action_id, item_id, key)
  SELECT ?1, d.value, j.value ->> '$.h' FROM json_each(?2) d, json_each(?3) j
   WHERE EXISTS (SELECT 1 FROM dmca_actions WHERE id = ?1) AND (
     (j.value ->> '$.k' = 'match' AND (
        EXISTS (SELECT 1 FROM item_publishes up WHERE up.item_id = d.value AND up.status IN ('current', 'previous') AND {})
        OR EXISTS (SELECT 1 FROM ghost_publishes gp JOIN ghost_questions gq ON gq.publish_id = gp.publish_id
                    WHERE gp.item_id = d.value AND (j.value ->> '$.h') IN (gq.match_hash, gq.bk_norm, gq.bk_stem, gq.bk_full, gq.bk_expl, gq.bk_ans))
        OR EXISTS (SELECT 1 FROM ghost_publishes gp JOIN ghost_block_keys gk ON gk.publish_id = gp.publish_id WHERE gp.item_id = d.value AND gk.key = (j.value ->> '$.h'))))
     OR (j.value ->> '$.k' = 'blob' AND (
        EXISTS (SELECT 1 FROM item_publishes up WHERE up.item_id = d.value AND up.status IN ('current', 'previous') AND (j.value ->> '$.h') IN {})
        OR EXISTS (SELECT 1 FROM ghost_publishes gp JOIN ghost_units gu ON gu.publish_id = gp.publish_id WHERE gp.item_id = d.value AND gu.kind = 'image' AND gu.key = (j.value ->> '$.h')))))",
        in_q_keys_of("(j.value ->> '$.h')", "up.publish_id"),
        served_blobs_of("up.publish_id")
    )
}

#[cfg(test)]
#[path = "served_tests.rs"]
mod tests;
