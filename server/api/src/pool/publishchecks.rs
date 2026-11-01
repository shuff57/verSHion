//! The pool-wide checks that decide no refusal of their own licence (worker/src/pool/publish.ts `publishOnce`, "soft check, blocklist, duplicates"): visible
//! items with the same question this account was never served, an image whose privacy block's grace window ended, content blocks, holds that follow the
//! content of a deleted item, and a new item that repeats one of the author's own. Holds are never a refusal; the commit step reads what is found here.
use super::ctx::{PoolCtx, PoolError, Res};
use super::license::{LICENSES, may_republish};
use super::publishfacts::Facts;
use super::publishhash::Hashed;
use super::publishrewrite::Rewritten;
use super::sqlfrag::visible_to_others;
use crate::blobs::BlobStore;
use rusqlite::{OptionalExtension, params};
use serde_json::json;
use std::collections::HashSet;

/// An active content block that matches this publish.
#[derive(Debug, Clone, PartialEq)]
pub struct Blocked {
    pub kind: String,
    pub hash: String,
    pub action_id: Option<String>,
}

/// A hold on another of the author's items that follows this content.
#[derive(Debug, Clone, PartialEq)]
pub struct Carried {
    pub reason: String,
    pub item_id: String,
}

#[derive(Debug, Default, PartialEq)]
pub struct Checks {
    /// (item id, licence) of visible items with the same question, under a licence that does not allow this one (at most 50).
    pub soft: Vec<(String, String)>,
    pub blocked: Option<Blocked>,
    pub carried: Option<Carried>,
    pub possible_duplicate: Option<String>,
}

fn dedup(xs: impl IntoIterator<Item = String>) -> Vec<String> {
    let mut seen = HashSet::new();
    xs.into_iter().filter(|x| seen.insert(x.clone())).collect()
}

/// Runs the checks. An image whose block's grace window ended (bytes possibly gone) is usable only where it is not served and only by an author who already
/// uses it there, so no served use and no other caller can depend on whether its bytes were kept; one still blocked is held at commit, never refused here.
#[allow(clippy::too_many_arguments)]
pub fn run(x: &PoolCtx, store: &BlobStore, me: &str, item_id: &str, is_new: bool, license: &str, rw: &Rewritten, h: &Hashed, f: &Facts) -> Res<Checks> {
    let n = rw.n;
    let mh_json = json!(dedup(h.match_hashes.iter().cloned())).to_string();
    let lics: Vec<&str> = LICENSES.iter().copied().filter(|l| *l == license || may_republish(l, license)).collect();
    let soft: Vec<(String, String)> = {
        let mut st = x.conn.prepare(&format!(
            "SELECT DISTINCT i.id, i.license FROM item_questions iq
             JOIN items i ON i.current_publish_id = iq.publish_id JOIN users u ON u.id = i.author_id
            WHERE iq.match_hash IN (SELECT value FROM json_each(?2)) AND i.author_id <> ?1 AND {}
              AND NOT EXISTS (SELECT 1 FROM imports im WHERE im.user_id = ?1 AND im.item_id = i.id)
              AND NOT EXISTS (SELECT 1 FROM json_each(COALESCE(iq.chain_json, '[]')) cl JOIN items mi ON mi.id = cl.value ->> '$.item_id' WHERE mi.author_id = ?1)
              AND NOT EXISTS (SELECT 1 FROM author_firsts af WHERE af.author_id = ?1 AND af.match_hash = iq.match_hash AND af.first_seq <= iq.seq)
              AND i.license NOT IN (SELECT value FROM json_each(?3)) LIMIT 50",
            visible_to_others()
        ))?;
        let rows = st.query_map(params![me, mh_json, json!(lics).to_string()], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)))?;
        rows.collect::<rusqlite::Result<_>>()?
    };
    let served_shas = dedup(f.img_at.iter().flatten().cloned());
    let mut shas = dedup(rw.images.iter().flatten().cloned());
    shas.retain(|s| rw.blob_info.contains_key(s));
    let purged: Vec<&String> = shas.iter().filter(|s| rw.blob_info[*s].privacy_purged_at.is_some()).collect();
    if !purged.is_empty() {
        let own_unserved: HashSet<String> = {
            let mut st = x.conn.prepare(
                "SELECT DISTINCT ub.sha256 FROM item_blobs ub JOIN item_publishes up ON up.publish_id = ub.publish_id
              WHERE up.author_id = ?1 AND up.status IN ('current', 'previous') AND ub.form = 'full' AND up.hide_key = 1 AND ub.sha256 IN (SELECT value FROM json_each(?2))
                AND NOT EXISTS (SELECT 1 FROM item_blobs us WHERE us.publish_id = ub.publish_id AND us.form = 'stripped' AND us.sha256 = ub.sha256)",
            )?;
            let rows = st.query_map(params![me, json!(purged).to_string()], |r| r.get::<_, String>(0))?;
            rows.collect::<rusqlite::Result<_>>()?
        };
        let served: HashSet<&String> = served_shas.iter().collect();
        let still_blocked: HashSet<String> = {
            let mut st = x.conn.prepare("SELECT hash FROM privacy_blocks WHERE kind = 'blob' AND hash IN (SELECT value FROM json_each(?1))")?;
            let rows = st.query_map([json!(purged).to_string()], |r| r.get::<_, String>(0))?;
            rows.collect::<rusqlite::Result<_>>()?
        };
        let mut gone: Vec<String> = purged.iter().filter(|s| !still_blocked.contains(**s) && (served.contains(**s) || !own_unserved.contains(**s))).map(|s| (*s).clone()).collect();
        for s in &purged {
            if !gone.contains(s) && !still_blocked.contains(*s) && store.len(&format!("blobs/{s}")).map_err(|e| rusqlite::Error::ToSqlConversionFailure(Box::new(e)))?.is_none() {
                gone.push((*s).clone());
            }
        }
        if !gone.is_empty() {
            let positions: Vec<usize> = rw.images.iter().enumerate().filter(|(_, s)| s.as_ref().is_some_and(|s| gone.contains(s))).map(|(i, _)| i).take(100).collect();
            return Err(PoolError::new(422, "blob_missing").with("reason", json!("reupload")).with("positions", json!(positions)).into());
        }
    }
    let blocked: Option<Blocked> = x
        .conn
        .query_row(
            "SELECT kind, hash, action_id FROM blocked_hashes WHERE lifted_at IS NULL AND (expires_at IS NULL OR expires_at > ?3)
         AND ((kind = 'match' AND hash IN (SELECT value FROM json_each(?1))) OR (kind = 'blob' AND hash IN (SELECT value FROM json_each(?2)))) LIMIT 1",
            params![json!(f.block_keys).to_string(), json!(served_shas).to_string(), x.now],
            |r| Ok(Blocked { kind: r.get(0)?, hash: r.get(1)?, action_id: r.get(2)? }),
        )
        .optional()?;
    // Matched by the normalised skeleton and full content too, and through the keys a held item left when it was deleted (carried_keys, until the reports
    // are resolved or CARRY_KEYS_MS passes). Never a common or short question, and only a hold an admin placed or that two or more distinct reporters stand behind.
    let carry_keys = dedup(h.match_hashes.iter().enumerate().filter(|(i, hh)| !(f.short_q[*i] || f.actx.common.contains(*hh))).flat_map(|(i, hh)| {
        let mut v = vec![hh.clone()];
        v.extend(f.bk_norm.get(i).cloned());
        v.extend(f.bk_full.get(i).cloned());
        v
    }));
    let carried: Option<Carried> = x
        .conn
        .query_row(
            "SELECT reason, item_id FROM (
         SELECT h.reason, h.item_id FROM item_holds h JOIN items i ON i.id = h.item_id AND i.author_id = ?1 AND i.purged_at IS NULL
          WHERE h.item_id <> ?2 AND h.source IN ('', 'manual', 'key_leak')
            AND (h.by IS NOT NULL OR (SELECT COUNT(DISTINCT r.reporter_id) FROM reports r WHERE r.item_id = h.item_id AND r.resolved_at IS NULL) >= 2)
            AND EXISTS (SELECT 1 FROM item_publishes p JOIN item_questions iq ON iq.publish_id = p.publish_id
                         WHERE p.item_id = h.item_id AND (iq.match_hash IN (SELECT value FROM json_each(?3)) OR iq.bk_norm IN (SELECT value FROM json_each(?3)) OR iq.bk_full IN (SELECT value FROM json_each(?3))))
         UNION ALL
         SELECT ck.reason, ck.item_id FROM carried_keys ck
          WHERE ck.author_id = ?1 AND ck.item_id <> ?2 AND ck.until > ?4 AND ck.key IN (SELECT value FROM json_each(?3)))
       ORDER BY reason LIMIT 1",
            params![me, item_id, json!(carry_keys).to_string(), x.now],
            |r| Ok(Carried { reason: r.get(0)?, item_id: r.get(1)? }),
        )
        .optional()?;
    let possible_duplicate: Option<String> = if is_new {
        x.conn
            .query_row(
                "SELECT i.id FROM items i JOIN item_questions iq ON iq.publish_id = i.current_publish_id
          WHERE i.author_id = ?1 AND i.removed_at IS NULL
          GROUP BY i.id HAVING COUNT(*) = ?2 AND SUM(iq.match_hash IN (SELECT value FROM json_each(?3))) = ?2 LIMIT 1",
                params![me, n as i64, mh_json],
                |r| r.get(0),
            )
            .optional()?
    } else {
        None
    };
    Ok(Checks { soft, blocked, carried, possible_duplicate })
}

#[cfg(test)]
#[path = "publishchecks_tests.rs"]
mod tests;
