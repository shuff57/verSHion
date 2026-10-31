//! Deleting publishes (worker/src/pool/publish.ts `deletePublishStatements`): what bound a publish's readers must outlive it, so a publish anyone was
//! served leaves a key-only ghost (hashes, credits, licence; no text, no tokens) before its rows go. Runs inside the caller's transaction.
use super::offer::offered_sql;
use super::served::served_form;
use rusqlite::Connection;
use serde_json::json;

/// SQL: item `item`'s licence now when its public version has unit `key` AND the item is offered to the pool (an unlisted, held or removed item's
/// licence grants nothing).
fn now_unit(item: &str, key: &str) -> String {
    format!(
        "(SELECT json_array(ni.license) FROM items ni WHERE ni.id = {item} AND (EXISTS (SELECT 1 FROM item_questions nq WHERE nq.publish_id = ni.public_publish_id AND (nq.bk_expl = {key} OR nq.bk_ans = {key}))
        OR EXISTS (SELECT 1 FROM item_part_keys npk WHERE npk.publish_id = ni.public_publish_id AND npk.key = {key})
        OR EXISTS (SELECT 1 FROM item_blobs nb LEFT JOIN blobs nbx ON nbx.sha256 = nb.sha256 WHERE nb.publish_id = ni.public_publish_id AND (nb.sha256 = {key} OR nbx.pixel_hash = {key})))
        AND {})",
        offered_sql("ni")
    )
}

pub fn delete_publishes(conn: &Connection, publish_ids: &[String]) -> rusqlite::Result<()> {
    if publish_ids.is_empty() {
        return Ok(());
    }
    let ids = json!(publish_ids).to_string();
    let in_ids = "IN (SELECT value FROM json_each(?1))";
    let ghost_item = |t: &str| format!("(SELECT gp.item_id FROM ghost_publishes gp WHERE gp.publish_id = {t}.publish_id)");
    let stmts: Vec<String> = vec![
        // Committed ids stay reserved forever (a staged id that never committed may be retried).
        format!("INSERT OR IGNORE INTO retired_publish_ids (publish_id, author_id) SELECT publish_id, author_id FROM item_publishes WHERE publish_id {in_ids} AND status IN ('current', 'previous')"),
        format!(
            "INSERT OR IGNORE INTO ghost_publishes (publish_id, item_id, author_id, hide_key, public_version, license, at)
             SELECT p.publish_id, p.item_id, p.author_id, p.hide_key, p.public_version, p.license, COALESCE(p.committed_at, p.created_at) FROM item_publishes p
              WHERE p.publish_id {in_ids} AND EXISTS (SELECT 1 FROM served s WHERE s.publish_id = p.publish_id)"
        ),
        format!(
            "INSERT OR IGNORE INTO ghost_questions (publish_id, qid, pos, match_hash, full_content, stripped_content, keyfree_content, near_full, near_stripped, near_kf,
                                                near_bare_full, near_bare_stripped, chain_json, seq, stem_hash, bk_expl, bk_ans, tok_sk, body_sk, qkind, bk_norm, bk_stem, bk_full, now_licenses)
             SELECT iq.publish_id, iq.qid, iq.pos, iq.match_hash, iq.full_content, iq.stripped_content, iq.keyfree_content, iq.near_full, iq.near_stripped, iq.near_kf,
                    iq.near_bare_full, iq.near_bare_stripped, iq.chain_json, iq.seq, iq.stem_hash, iq.bk_expl, iq.bk_ans, iq.tok_sk, iq.body_sk,
                    iq.qkind, iq.bk_norm, iq.bk_stem, iq.bk_full,
                    (SELECT json_array(i.license) FROM items i JOIN item_publishes cp ON cp.publish_id = i.public_publish_id JOIN item_questions cq ON cq.publish_id = cp.publish_id AND cq.qid = iq.qid
                      WHERE i.id = p.item_id AND (CASE WHEN cp.hide_key = 1 THEN cq.near_stripped ELSE cq.near_full END) = (CASE WHEN p.hide_key = 1 THEN iq.near_stripped ELSE iq.near_full END) AND {})
               FROM item_questions iq JOIN item_publishes p ON p.publish_id = iq.publish_id WHERE iq.publish_id {in_ids} AND iq.publish_id IN (SELECT publish_id FROM ghost_publishes)",
            offered_sql("i")
        ),
        format!(
            "INSERT OR IGNORE INTO ghost_units (publish_id, pos, part, key, kind, seq, now_licenses)
             SELECT iq.publish_id, iq.pos, -1, k.value, 'text', COALESCE(json_extract(iq.unit_seqs, '$.\"' || k.value || '\"'), iq.seq), {} FROM item_questions iq, json_each(json_array(iq.bk_expl, iq.bk_ans)) k
              WHERE iq.publish_id {in_ids} AND iq.publish_id IN (SELECT publish_id FROM ghost_publishes) AND k.value IS NOT NULL",
            now_unit(&ghost_item("iq"), "k.value")
        ),
        format!(
            "INSERT OR IGNORE INTO ghost_units (publish_id, pos, part, key, kind, seq, now_licenses)
             SELECT pk.publish_id, pk.pos, pk.part, pk.key, CASE WHEN pk.key LIKE 'u:%' THEN 'part' ELSE 'text' END,
                    (SELECT uq.seq FROM item_questions uq WHERE uq.publish_id = pk.publish_id AND uq.pos = pk.pos),
                    {} FROM item_part_keys pk
              WHERE pk.publish_id {in_ids} AND pk.publish_id IN (SELECT publish_id FROM ghost_publishes) AND (pk.key LIKE 'u:%' OR pk.key LIKE 'e:%' OR pk.key LIKE 'a:%')",
            now_unit(&ghost_item("pk"), "pk.key")
        ),
        format!(
            "INSERT OR IGNORE INTO ghost_units (publish_id, pos, part, key, kind, seq, now_licenses)
             SELECT ib.publish_id, -1, -1, k.value, 'image', ib.seq, {} FROM item_blobs ib JOIN item_publishes p ON p.publish_id = ib.publish_id LEFT JOIN blobs b ON b.sha256 = ib.sha256,
                    json_each(json_array(ib.sha256, b.pixel_hash)) k
              WHERE ib.publish_id {in_ids} AND ib.publish_id IN (SELECT publish_id FROM ghost_publishes) AND {} AND k.value IS NOT NULL",
            now_unit("p.item_id", "k.value"),
            served_form("ib", "p")
        ),
        // Every ghost of these items also takes the item's licence NOW, where its public version still has the very question or unit.
        format!(
            "UPDATE ghost_questions SET now_licenses = (SELECT json_group_array(DISTINCT v) FROM (
                SELECT nl.value AS v FROM json_each(COALESCE(ghost_questions.now_licenses, '[]')) nl
                UNION SELECT i.license FROM ghost_publishes gp JOIN items i ON i.id = gp.item_id JOIN item_publishes cp ON cp.publish_id = i.public_publish_id
                        JOIN item_questions cq ON cq.publish_id = cp.publish_id AND cq.qid = ghost_questions.qid
                 WHERE gp.publish_id = ghost_questions.publish_id
                   AND (CASE WHEN cp.hide_key = 1 THEN cq.near_stripped ELSE cq.near_full END) = (CASE WHEN gp.hide_key = 1 THEN ghost_questions.near_stripped ELSE ghost_questions.near_full END)
                   AND {}))
              WHERE publish_id IN (SELECT gp.publish_id FROM ghost_publishes gp WHERE gp.item_id IN (SELECT item_id FROM item_publishes WHERE publish_id {in_ids}))",
            offered_sql("i")
        ),
        format!(
            "UPDATE ghost_units SET now_licenses = (SELECT json_group_array(DISTINCT v) FROM (
                SELECT nl.value AS v FROM json_each(COALESCE(ghost_units.now_licenses, '[]')) nl
                UNION SELECT nn.value FROM json_each(COALESCE({}, '[]')) nn))
              WHERE publish_id IN (SELECT gp.publish_id FROM ghost_publishes gp WHERE gp.item_id IN (SELECT item_id FROM item_publishes WHERE publish_id {in_ids}))",
            now_unit(&ghost_item("ghost_units"), "ghost_units.key")
        ),
        // Each part's block keys (keyed), so a ghost takedown's plan has its parts.
        format!(
            "INSERT OR IGNORE INTO ghost_block_keys (publish_id, pos, part, key) SELECT pk.publish_id, pk.pos, pk.part, pk.key FROM item_part_keys pk
              WHERE pk.publish_id {in_ids} AND pk.publish_id IN (SELECT publish_id FROM ghost_publishes)"
        ),
        // A ghost is found by its KEYED bands only.
        format!(
            "INSERT OR IGNORE INTO ghost_bands (band, item_id, qid)
             SELECT kb.value, p.item_id, iq.qid FROM item_questions iq JOIN item_publishes p ON p.publish_id = iq.publish_id, json_each(COALESCE(iq.kbands, '[]')) kb
              WHERE iq.publish_id {in_ids} AND iq.publish_id IN (SELECT publish_id FROM ghost_publishes)"
        ),
        format!("DELETE FROM item_questions WHERE publish_id {in_ids}"),
        format!("DELETE FROM item_part_keys WHERE publish_id {in_ids}"),
        // The public and own stem bands of those items that no retained publish still has go with them (an account's served bands are evidence and stay).
        format!(
            "DELETE FROM stem_bands WHERE (user_id = 'pub' OR user_id LIKE 'own:%') AND item_id IN (SELECT item_id FROM item_publishes WHERE publish_id {in_ids})
               AND NOT EXISTS (SELECT 1 FROM item_publishes rp JOIN item_questions rq ON rq.publish_id = rp.publish_id, json_each(COALESCE(rq.stem_bands, '[]')) b
                                WHERE rp.item_id = stem_bands.item_id AND rp.status IN ('current', 'previous') AND rq.qid = stem_bands.qid AND b.value = stem_bands.band)"
        ),
        format!("DELETE FROM item_tags WHERE publish_id {in_ids}"),
        format!("DELETE FROM served_words WHERE publish_id {in_ids}"),
        format!("DELETE FROM item_standards WHERE publish_id {in_ids}"),
        format!("DELETE FROM item_blobs WHERE publish_id {in_ids}"),
        // An attestation stays while its publish or its ghost does.
        format!("DELETE FROM attestations WHERE publish_id {in_ids} AND publish_id NOT IN (SELECT publish_id FROM ghost_publishes)"),
        format!("DELETE FROM item_publishes WHERE publish_id {in_ids}"),
    ];
    for s in &stmts {
        conn.execute(s, [&ids])?;
    }
    Ok(())
}

#[cfg(test)]
#[path = "publishdel_tests.rs"]
mod tests;
