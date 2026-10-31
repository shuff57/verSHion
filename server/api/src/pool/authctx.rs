//! What the publisher authored, for the evidence rules (worker/src/pool/provenance.ts `authorContext`,
//! `ownContentFirst`): their item ids, per match_hash their first publish (seq) and their shown contents, the key-free
//! contents they published at all, and the "common question" roots.
use super::chain::ChainLink;
use super::ctx::{PoolCtx, Res};
use super::sqlfrag::visible_to_others;
use super::visible::{OwnShown, own_shown};
use rusqlite::params;
use std::collections::{HashMap, HashSet};

/// Root contents kept per question for the "common" test (the earliest first).
pub const ROOTS_PER_QUESTION: usize = 64;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Root {
    pub r: String,
    pub s: i64,
    pub n: String,
}

#[derive(Debug, Default)]
pub struct AuthorCtx {
    pub mine: HashSet<String>,
    pub first_at: HashMap<String, i64>,
    pub own: HashMap<String, OwnShown>,
    /// void_items among the hits' items and chain links (filled by the evidence query).
    pub voided: HashSet<String>,
    /// ... and the rightful-author links standing in for some of them.
    pub replaced: HashMap<String, ChainLink>,
    /// Questions (match_hash) at least K other INDEPENDENT authors' visible items use: views of them are not evidence.
    pub common: HashSet<String>,
    /// Key-free contents (near_kf) the author published at all, hidden or purged included.
    pub own_kf: HashSet<String>,
    /// Per question, its independent roots' first publishes, the earliest first.
    pub roots: HashMap<String, Vec<Root>>,
    pub k: i64,
}

pub fn author_context(x: &PoolCtx, me: &str, hashes: &[String]) -> Res<AuthorCtx> {
    let mut out = AuthorCtx { k: 3, ..Default::default() };
    {
        let mut st = x.conn.prepare("SELECT id FROM items WHERE author_id = ?1")?;
        for r in st.query_map([me], |r| r.get::<_, String>(0))? {
            out.mine.insert(r?);
        }
    }
    if hashes.is_empty() {
        return Ok(out);
    }
    let mut uniq: Vec<&String> = Vec::new();
    for h in hashes {
        if !uniq.contains(&h) {
            uniq.push(h);
        }
    }
    let uniq = serde_json::to_string(&uniq).unwrap();

    // Own work: contents of retained publishes, with whether that publish hid its key (`own_shown` keeps only the shown
    // ones). First publish: the durable per-content record (author_firsts, never lowered by retention or purge) or,
    // failing it, the retained rows.
    {
        let mut st = x.conn.prepare(
            "SELECT iq.match_hash, MIN(iq.seq) AS at, json_group_array(json_array(iq.full_content, p.hide_key)) AS fulls FROM item_questions iq
               JOIN item_publishes p ON p.publish_id = iq.publish_id AND p.author_id = ?1 AND p.status IN ('current', 'previous')
              WHERE iq.match_hash IN (SELECT value FROM json_each(?2)) GROUP BY iq.match_hash",
        )?;
        let rows = st.query_map(params![me, uniq], |r| Ok((r.get::<_, String>(0)?, r.get::<_, i64>(1)?, r.get::<_, String>(2)?)))?;
        for row in rows {
            let (mh, at, fulls) = row?;
            out.first_at.insert(mh.clone(), at);
            let parsed: Vec<(String, i64)> = serde_json::from_str(&fulls).unwrap_or_default();
            out.own.insert(mh, own_shown(parsed.iter().map(|(c, hid)| (c.as_str(), *hid != 0))));
        }
    }
    {
        let mut st = x.conn.prepare("SELECT match_hash, first_seq FROM author_firsts WHERE author_id = ?1 AND match_hash IN (SELECT value FROM json_each(?2))")?;
        for row in st.query_map(params![me, uniq], |r| Ok((r.get::<_, String>(0)?, r.get::<_, i64>(1)?)))? {
            let (mh, seq) = row?;
            let e = out.first_at.entry(mh).or_insert(seq);
            *e = (*e).min(seq);
        }
    }
    // Durable own contents: shown ones join OwnShown (a purged or retained-away original is still the author's own);
    // key-free ones say the author published that question.
    {
        let mut more: HashMap<String, Vec<String>> = HashMap::new();
        let mut st = x.conn.prepare("SELECT match_hash, kind, hash FROM author_contents WHERE author_id = ?1 AND match_hash IN (SELECT value FROM json_each(?2))")?;
        for row in st.query_map(params![me, uniq], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?, r.get::<_, String>(2)?)))? {
            let (mh, kind, hash) = row?;
            if kind == "kf" {
                out.own_kf.insert(hash);
            } else {
                more.entry(mh).or_default().push(hash);
            }
        }
        for (mh, list) in more {
            let mut all: Vec<String> = out.own.get(&mh).map(|o| o.shown.iter().cloned().collect()).unwrap_or_default();
            all.extend(list);
            out.own.insert(mh, own_shown(all.iter().map(|c| (c.as_str(), false))));
        }
    }
    let k = x.limit_of("*", "block_common_k")?;
    // An "earlier identical publication" link is not lineage: it never makes a root. Independent authors only: each user
    // counts by the ROOT of its question's lineage (the first link of its chain, else itself), so copies descending from
    // one source never make it "common"; the publisher's own lineage never counts. Per root, each served CONTENT (licence
    // sense) with its first publish.
    let sql = format!(
        "SELECT match_hash, root, n, MIN(seq) AS s FROM (
           SELECT iq.match_hash, iq.seq, CASE WHEN p.hide_key = 1 THEN iq.near_stripped ELSE iq.near_full END AS n,
                  COALESCE((SELECT ri.author_id FROM items ri WHERE ri.id = NULLIF(CASE WHEN COALESCE(iq.chain_json ->> '$[0].earlier_match', 0) THEN '' ELSE iq.chain_json ->> '$[0].item_id' END, '')), i.author_id) AS root
             FROM item_questions iq JOIN items i ON i.current_publish_id = iq.publish_id JOIN item_publishes p ON p.publish_id = iq.publish_id JOIN users u ON u.id = i.author_id
            WHERE iq.match_hash IN (SELECT value FROM json_each(?2)) AND i.author_id <> ?1 AND {})
          WHERE root <> ?1 AND n IS NOT NULL GROUP BY match_hash, root, n ORDER BY match_hash, s",
        visible_to_others()
    );
    {
        let mut st = x.conn.prepare(&sql)?;
        for row in st.query_map(params![me, uniq], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?, r.get::<_, String>(2)?, r.get::<_, i64>(3)?)))? {
            let (mh, r, n, s) = row?;
            let l = out.roots.entry(mh).or_default();
            if l.len() < ROOTS_PER_QUESTION {
                l.push(Root { r, s, n });
            }
        }
    }
    for (mh, l) in &out.roots {
        let distinct: HashSet<&String> = l.iter().map(|x| &x.r).collect();
        if distinct.len() as i64 >= k {
            out.common.insert(mh.clone());
        }
    }
    out.k = k;
    Ok(out)
}

/// One subject for `own_content_first`.
pub struct ContentSubject<'a> {
    pub n: Option<&'a str>,
    pub kf: &'a str,
    /// The key is itself a work (so the key-free form does not stand for it).
    pub keywork: bool,
}

/// The publisher's first publish of each subject's CONTENT: the licence-sense content they served (`n`), or its key-free
/// form unless the key is itself a work. Never a stem or a match_hash alone, so publishing a bare stem early makes no one
/// "first" for a richer question. Keyed by the subject's index.
pub fn own_content_first(x: &PoolCtx, me: &str, subj: &[ContentSubject]) -> Res<HashMap<usize, i64>> {
    let mut out = HashMap::new();
    if subj.is_empty() {
        return Ok(out);
    }
    let json: Vec<serde_json::Value> = subj.iter().map(|s| serde_json::json!({"n": s.n, "kf": s.kf, "w": s.keywork as i64})).collect();
    // Two indexed lookups, never an OR over the author's rows.
    let mut st = x.conn.prepare(
        "SELECT CAST(j.key AS INTEGER) AS pos,
                NULLIF(MIN(COALESCE((SELECT MIN(ac.first_seq) FROM author_contents ac WHERE ac.author_id = ?1 AND ac.kind = 'nshown' AND ac.hash = j.value ->> '$.n'), 9e18),
                           COALESCE((SELECT MIN(ac.first_seq) FROM author_contents ac WHERE ac.author_id = ?1 AND ac.kind = 'kf' AND ac.hash = j.value ->> '$.kf' AND CAST(j.value ->> '$.w' AS INTEGER) = 0), 9e18)), 9e18) AS s
           FROM json_each(?2) j",
    )?;
    for row in st.query_map(params![me, serde_json::to_string(&json).unwrap()], |r| Ok((r.get::<_, i64>(0)?, r.get::<_, Option<f64>>(1)?)))? {
        let (pos, s) = row?;
        if let Some(s) = s.filter(|s| *s > 0.0) {
            out.insert(pos as usize, s as i64);
        }
    }
    Ok(out)
}

#[cfg(test)]
#[path = "authctx_tests.rs"]
mod tests;
