//! The fuzzy-stem pieces of worker/src/pool/provenance.ts that stand alone: the limits, the author's own-earlier-version
//! comparisons (`minFirst`, `lacksAdded`, `addsNothing`), and the window over the author's durable stem records
//! (`OWN_STEMS_SQL`, `ownStemsWindow`). The fuzzy queries that use them come next.
use rusqlite::{Connection, params};
use std::collections::{HashMap, HashSet};

/// Token Jaccard at which two stems are a near-rewording.
pub const FUZZY_JACCARD: f64 = 0.8;
/// Fuzzy candidates per question, pairs in all, and evidence rows examined per publish.
pub const FUZZY_PER_POSITION: usize = 8;
/// Near candidates examined per question, earliest first.
pub const FUZZY_WINDOW: i64 = 200;
/// Admissible near sources kept per question, earliest first; the rest only bound the licence.
pub const FUZZY_KEEP: i64 = 32;
pub const FUZZY_PAIRS_MAX: i64 = 2000;
pub const FUZZY_ROWS_MAX: i64 = 20_000;
/// ... and in the public band index (all publishes), where a band is shared far more often.
pub const PUB_BAND_MAX: i64 = 256;
/// A MinHash band shared by more served questions than this is skipped (and the publish flagged).
pub const FUZZY_BAND_MAX: i64 = 32;
pub const FUZZY_ROW_BYTES: usize = 8 * 1024;

/// The earlier of two optional "first" seqs.
pub fn min_first(a: Option<i64>, b: Option<i64>) -> Option<i64> {
    match (a, b) {
        (None, b) => b,
        (a, None) => a,
        (Some(a), Some(b)) => Some(a.min(b)),
    }
}

/// The publisher's later version has a value her earlier one did not.
fn added(loose: Option<&str>, later: Option<&str>) -> bool {
    match later {
        None | Some("") => false,
        Some(l) => matches!(loose, None | Some("")) || loose != Some(l),
    }
}

/// An explanation / model-answer pair as the comparisons read it.
#[derive(Clone, Copy, Debug, Default)]
pub struct Fields<'a> {
    pub expl: Option<&'a str>,
    pub ans: Option<&'a str>,
}

/// The candidate source lacks what the publisher's version ADDED to her own earlier one. Per field: a field she did not
/// add is hers either way (her earlier version already carried it), so requiring the source to differ there wrongly kept
/// the strict first for Short/Blank, whose answer or sample is usually unchanged when she adds an explanation.
pub fn lacks_added(p: Fields, expl: Option<&str>, ans: Option<&str>, loose: Option<Fields>) -> bool {
    let l = loose.unwrap_or_default();
    (!added(l.expl, p.expl) || expl.is_none() || expl != p.expl) && (!added(l.ans, p.ans) || ans.is_none() || ans != p.ans)
}

/// The author's later version adds nothing to a source's explanation / answer: no explanation or answer the earlier one
/// did not have (removing them is her own edit; adding another's is not hers).
pub fn adds_nothing(p: Fields, expl: Option<&str>, ans: Option<&str>) -> bool {
    (p.expl.is_none() || p.expl == expl) && (p.ans.is_none() || p.ans == ans)
}

/// Her durable stem records sharing a keyed band with each position (`own_fuzzy_first`): one row per (position, content),
/// the first FUZZY_PAIRS_MAX in (pos, qhash, bk_expl, bk_ans) order.
pub const OWN_STEMS_SQL: &str = "WITH b AS MATERIALIZED (SELECT CAST(j.value ->> '$.pos' AS INTEGER) AS pos, bv.value AS band FROM json_each(?2) j, json_each(j.value -> '$.bands') bv)
         SELECT b.pos, s.first_seq, s.tok_sk, s.body_sk, s.bk_expl, s.bk_ans, s.bk_expl_full FROM b CROSS JOIN author_stems s ON s.author_id = ?1 AND s.band = b.band
          GROUP BY b.pos, s.qhash, s.bk_expl, s.bk_ans LIMIT ?3";

/// The positions OWN_STEMS_SQL can answer for, so it never joins the rest. Its rows are cut at FUZZY_PAIRS_MAX in
/// position order, and a position has at least as many rows as its busiest band has records (one band holds a content at
/// most once: the primary key), so once those counts add up to FUZZY_PAIRS_MAX no later position can reach the result.
/// Without this, a bank of like stems republished joined every position with every record of her common bands and grew
/// with the square of the bank. The result is the same as the full join: the kept positions' rows are joined in the same
/// order. Each band's count stops at FUZZY_PAIRS_MAX, so this reads at most that many rows per band.
pub fn own_stems_window<T: Clone>(conn: &Connection, me: &str, kb: &[T], pos_of: impl Fn(&T) -> usize, bands_of: impl Fn(&T) -> &[String]) -> rusqlite::Result<Vec<T>> {
    let mut seen = HashSet::new();
    let bands: Vec<&String> = kb.iter().flat_map(|x| bands_of(x).iter()).filter(|b| seen.insert(*b)).collect();
    if bands.is_empty() {
        return Ok(kb.to_vec());
    }
    let mut st = conn.prepare("SELECT j.value AS band, (SELECT COUNT(*) FROM (SELECT 1 FROM author_stems s WHERE s.author_id = ?1 AND s.band = j.value LIMIT ?3)) AS n FROM json_each(?2) j")?;
    let mut n: HashMap<String, i64> = HashMap::new();
    for r in st.query_map(params![me, serde_json::to_string(&bands).unwrap(), FUZZY_PAIRS_MAX], |r| Ok((r.get::<_, String>(0)?, r.get::<_, i64>(1)?)))? {
        let (b, c) = r?;
        n.insert(b, c);
    }
    let mut least: HashMap<usize, i64> = HashMap::new();
    for x in kb {
        let m = bands_of(x).iter().map(|b| n.get(b).copied().unwrap_or(0)).max().unwrap_or(0);
        let e = least.entry(pos_of(x)).or_insert(0);
        *e = (*e).max(m);
    }
    let mut order: Vec<(usize, i64)> = least.into_iter().collect();
    order.sort();
    let mut total = 0;
    for (pos, m) in order {
        total += m;
        if total >= FUZZY_PAIRS_MAX {
            return Ok(kb.iter().filter(|y| pos_of(y) <= pos).cloned().collect());
        }
    }
    Ok(kb.to_vec())
}

#[cfg(test)]
#[path = "fuzzy_tests.rs"]
mod tests;
