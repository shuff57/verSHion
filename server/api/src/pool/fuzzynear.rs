//! The two near-stem lookups of worker/src/pool/provenance.ts: the author's OWN earlier versions of a question
//! (`ownFuzzyFirst`) and the earliest visible question by ANOTHER author whose stem is a near-rewording
//! (`publicNearEarlier`). Both only ever decide that a later source is not the origin or name an earlier public one; a cut
//! is counted and flagged, never silent.
use super::fingerprint::Fingerprinter;
use super::fuzzy::*;
use super::qhash::{jaccard, same_body};
use super::sqlfrag::visible_to_others;
use rusqlite::{Connection, params};
use std::collections::{HashMap, HashSet};

pub struct OwnPos {
    pub pos: usize,
    pub tokens: Vec<String>,
    pub bands: Vec<String>,
    pub body: String,
    pub body_toks: Option<Vec<String>>,
    pub expl: Option<String>,
    pub ans: Option<String>,
}
impl OwnPos {
    fn fields(&self) -> Fields<'_> {
        Fields { expl: self.expl.as_deref(), ans: self.ans.as_deref() }
    }
}

/// The publisher's earliest own version with the same choices EVEN IF the later one adds an explanation or answer to it.
/// Used only against a candidate source that lacks what she added; never for binding.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LooseFirst {
    pub seq: i64,
    pub expl: Option<String>,
    pub ans: Option<String>,
}

fn json_strings(s: Option<String>) -> Option<Vec<String>> {
    s.and_then(|s| serde_json::from_str(&s).ok())
}

/// The author's OWN earlier versions of each question, fuzzily: the earliest publish sequence of a question the author
/// published themselves (any of their items, any retained publish) whose stem is a near-rewording (Jaccard >=
/// FUZZY_JACCARD) of this one, plus her DURABLE keyed record of every stem she published (deleted, purged and re-added
/// material stays hers). Bands of the author's own stems live under user "own:<id>". Used ONLY to decide that a later
/// fuzzy source (or a loose stem match) is not the origin: never for exact evidence.
pub fn own_fuzzy_first(conn: &Connection, fp: &Fingerprinter, me: &str, pos: &[OwnPos], cut: &mut HashSet<usize>, loose: &mut HashMap<usize, LooseFirst>) -> rusqlite::Result<HashMap<usize, i64>> {
    let mut out: HashMap<usize, i64> = HashMap::new();
    let want: Vec<&OwnPos> = pos.iter().filter(|p| !p.bands.is_empty()).collect();
    if want.is_empty() {
        return Ok(out);
    }
    let by_pos: HashMap<usize, &OwnPos> = want.iter().map(|p| (p.pos, *p)).collect();
    let q = serde_json::to_string(&want.iter().map(|p| serde_json::json!({"pos": p.pos, "bands": p.bands})).collect::<Vec<_>>()).unwrap();
    let mut st = conn.prepare(
        "WITH b AS MATERIALIZED (SELECT CAST(j.value ->> '$.pos' AS INTEGER) AS pos, bv.value AS band FROM json_each(?2) j, json_each(j.value -> '$.bands') bv),
              bc AS MATERIALIZED (SELECT DISTINCT band, (SELECT COUNT(*) FROM (SELECT 1 FROM stem_bands x WHERE x.user_id = ?1 AND x.band = b.band LIMIT ?5 + 1)) AS n FROM b),
              cand0 AS (SELECT b.pos, sb.item_id, sb.qid, COUNT(*) AS shared FROM b CROSS JOIN bc ON bc.band = b.band AND bc.n <= ?5 CROSS JOIN stem_bands sb ON sb.user_id = ?1 AND sb.band = b.band GROUP BY b.pos, sb.item_id, sb.qid),
              ranked AS MATERIALIZED (SELECT cand0.*, ROW_NUMBER() OVER (PARTITION BY pos ORDER BY shared DESC, item_id, qid) AS rn FROM cand0),
              cand AS MATERIALIZED (SELECT * FROM ranked WHERE rn <= ?3 LIMIT ?4)
         SELECT c.pos, iq.seq, iq.stem_tokens, iq.body_hash, iq.body_tokens, iq.bk_expl, iq.bk_ans FROM cand c CROSS JOIN item_publishes p ON p.item_id = c.item_id AND p.status IN ('current', 'previous')
           CROSS JOIN item_questions iq ON iq.publish_id = p.publish_id AND iq.qid = c.qid
          WHERE p.author_id = ?6
         -- (a cut is counted: the caller flags it; only the author's OWN items are looked at, so a cut can only keep a source binding, never free one)
         UNION ALL SELECT pos, NULL, NULL, NULL, NULL, NULL, NULL FROM ranked WHERE rn > ?3 GROUP BY pos",
    )?;
    type Row = (usize, Option<i64>, Option<String>, Option<String>, Option<String>, Option<String>, Option<String>);
    let rows: Vec<Row> = st
        .query_map(params![format!("own:{me}"), q, FUZZY_PER_POSITION as i64, FUZZY_PAIRS_MAX, FUZZY_BAND_MAX, me], |r| {
            Ok((r.get::<_, i64>(0)? as usize, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?, r.get(5)?, r.get(6)?))
        })?
        .collect::<Result<_, _>>()?;
    for (pos_i, seq, stem_tokens, body_hash, body_tokens, bk_expl, bk_ans) in rows {
        let Some(seq) = seq else {
            cut.insert(pos_i);
            continue;
        };
        // The author's own earlier question must have the same body (everything but the stem): a reworded stem of their own
        // question, never a bare stem they planted. "The same body" as every other near path judges it (the kind and its
        // choices, `same_body`), so dropping or adding an explanation never makes her own earlier version "another".
        let p0 = by_pos[&pos_i];
        let same_choices = p0.body_toks.as_ref().is_some_and(|b| same_body(Some(b), json_strings(body_tokens.clone()).as_deref()));
        let same = (body_hash.is_some() && body_hash.as_deref() == Some(p0.body.as_str())) || (same_choices && adds_nothing(p0.fields(), bk_expl.as_deref(), bk_ans.as_deref()));
        if !same && !same_choices {
            continue;
        }
        if seq <= 0 || jaccard(&p0.tokens, &json_strings(stem_tokens).unwrap_or_default()) < FUZZY_JACCARD {
            continue;
        }
        // Keep the loose version's own fields: what she ADDED is judged per field.
        if loose.get(&pos_i).is_none_or(|cur| seq < cur.seq) {
            loose.insert(pos_i, LooseFirst { seq, expl: bk_expl.clone(), ans: bk_ans.clone() });
        }
        if !same {
            continue;
        }
        let e = out.entry(pos_i).or_insert(seq);
        *e = (*e).min(seq);
    }
    // ... and her DURABLE keyed record of every stem she published (any item, retained or not), by keyed bands, keyed sets.
    // ALWAYS consult the durable record and keep the EARLIEST of the two: a newer retained version matching the subject must
    // not hide her older record: her first publication of the material is what makes a later copier "not earlier".
    let mut memo: HashMap<String, String> = HashMap::new();
    let rest: Vec<&&OwnPos> = want.iter().filter(|p| p.body_toks.is_some()).collect();
    if rest.is_empty() {
        return Ok(out);
    }
    struct Kb {
        pos: usize,
        bands: Vec<String>,
    }
    let kb: Vec<Kb> = rest.iter().map(|p| Kb { pos: p.pos, bands: p.bands.iter().map(|b| fp.keyed_one("b:", b, &mut memo)).collect() }).collect();
    let windowed: Vec<(usize, Vec<String>)> = {
        let pairs: Vec<(usize, Vec<String>)> = kb.iter().map(|k| (k.pos, k.bands.clone())).collect();
        own_stems_window(conn, me, &pairs, |p| p.0, |p| &p.1)?
    };
    let wj = serde_json::to_string(&windowed.iter().map(|(p, b)| serde_json::json!({"pos": p, "bands": b})).collect::<Vec<_>>()).unwrap();
    let mut st = conn.prepare(OWN_STEMS_SQL)?;
    type DRow = (usize, i64, String, Option<String>, Option<String>, Option<String>, Option<String>);
    let d: Vec<DRow> = st
        .query_map(params![me, wj, FUZZY_PAIRS_MAX], |r| Ok((r.get::<_, i64>(0)? as usize, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?, r.get(5)?, r.get(6)?)))?
        .collect::<Result<_, _>>()?;
    for (pos_i, first_seq, tok_sk, body_sk, bk_expl, bk_ans, bk_expl_full) in &d {
        let p0 = by_pos[pos_i];
        // A hidden publish's served form has no explanation; compare the later subject with the FULL form's explanation key
        // when the record has it. (The answer is always the served one: a hidden key must never change an observable outcome.)
        let expl = bk_expl_full.clone().filter(|s| !s.is_empty()).or_else(|| bk_expl.clone());
        let Some(btoks) = p0.body_toks.as_ref() else { continue };
        if *first_seq <= 0 {
            continue;
        }
        if jaccard(&fp.keyed_set(&p0.tokens, &mut memo, "t:"), &json_strings(Some(tok_sk.clone())).unwrap_or_default()) < FUZZY_JACCARD {
            continue;
        }
        if !same_body(Some(&fp.keyed_body(btoks, &mut memo)), json_strings(body_sk.clone()).as_deref()) {
            continue;
        }
        if loose.get(pos_i).is_none_or(|cur| *first_seq < cur.seq) {
            loose.insert(*pos_i, LooseFirst { seq: *first_seq, expl: expl.clone(), ans: bk_ans.clone() });
        }
        let non_empty = |s: &Option<String>| s.clone().filter(|x| !x.is_empty());
        if !adds_nothing(p0.fields(), non_empty(&expl).as_deref(), non_empty(bk_ans).as_deref()) {
            continue;
        }
        let e = out.entry(*pos_i).or_insert(*first_seq);
        *e = (*e).min(*first_seq);
    }
    // The durable lookup's own allowance is counted, not silent: past it the caller flags each position (a cut here can
    // only keep a source binding, never free one).
    if d.len() as i64 >= FUZZY_PAIRS_MAX {
        for p in &rest {
            cut.insert(p.pos);
        }
    }
    Ok(out)
}

pub struct PubPos {
    pub pos: usize,
    pub tokens: Vec<String>,
    pub bands: Vec<String>,
    pub qkind: String,
    pub first: Option<i64>,
    pub body: Option<Vec<String>>,
    pub loose_first: Option<i64>,
    pub loose_expl: Option<String>,
    pub loose_ans: Option<String>,
    pub expl: Option<String>,
    pub ans: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NearSource {
    pub item_id: String,
    pub qid: String,
    pub version: i64,
    pub license: String,
    pub author_name: String,
    pub seq: i64,
}

/// The earliest visible question by ANOTHER author whose served stem is a near-rewording (token Jaccard >= FUZZY_JACCARD)
/// of each position's, of the same kind, first published before `first`. Bands of every publish's served stems live under
/// user "pub"; a band shared by more than PUB_BAND_MAX questions is skipped (common stems). Public data only.
pub fn public_near_earlier(conn: &Connection, me: &str, pos: &[PubPos]) -> rusqlite::Result<(HashMap<usize, NearSource>, HashSet<usize>)> {
    let mut out: HashMap<usize, NearSource> = HashMap::new();
    let mut cut = HashSet::new();
    let want: Vec<&PubPos> = pos.iter().filter(|p| !p.bands.is_empty()).collect();
    if want.is_empty() {
        return Ok((out, cut));
    }
    // The admissible candidates (visible, someone else's, the same kind, published before the author's first) EARLIEST
    // first, before any cut: a later copy never takes an earlier item's place; a cut is counted and flagged, never silent.
    let sql = format!(
        "WITH b AS MATERIALIZED (SELECT CAST(j.value ->> '$.pos' AS INTEGER) AS pos, j.value ->> '$.qkind' AS qkind, j.value ->> '$.first' AS first, bv.value AS band
                                  FROM json_each(?2) j, json_each(j.value -> '$.bands') bv),
              bc AS MATERIALIZED (SELECT DISTINCT band, (SELECT COUNT(*) FROM (SELECT 1 FROM stem_bands x WHERE x.user_id = 'pub' AND x.band = b.band LIMIT ?5 + 1)) AS n FROM b),
              cand0 AS MATERIALIZED (SELECT b.pos, MIN(b.qkind) AS qkind, MIN(b.first) AS first, sb.item_id, sb.qid FROM b CROSS JOIN bc ON bc.band = b.band AND bc.n <= ?5
                                       CROSS JOIN stem_bands sb ON sb.user_id = 'pub' AND sb.band = b.band GROUP BY b.pos, sb.item_id, sb.qid),
              adm AS MATERIALIZED (SELECT c0.pos, i.id AS item_id, iq.qid, i.public_version AS version, i.license, COALESCE(u.display_name, 'deleted teacher') AS author_name,
                                          iq.stem_tokens, iq.qkind, iq.seq, iq.body_tokens, iq.bk_expl, iq.bk_ans
                                     FROM cand0 c0 CROSS JOIN items i ON i.id = c0.item_id JOIN users u ON u.id = i.author_id
                                     CROSS JOIN item_questions iq ON iq.publish_id = i.public_publish_id AND iq.qid = c0.qid
                                    WHERE i.author_id <> ?1 AND {} AND iq.qkind = c0.qkind AND (c0.first IS NULL OR iq.seq < CAST(c0.first AS INTEGER))
                                      -- (never a copy of the publisher's OWN earlier question: its content was hers first, or its chain runs to one of her items)
                                      AND NOT EXISTS (SELECT 1 FROM author_firsts af WHERE af.author_id = ?1 AND af.match_hash = iq.match_hash AND af.first_seq <= iq.seq)
                                      AND NOT EXISTS (SELECT 1 FROM json_each(COALESCE(iq.chain_json, '[]')) cl JOIN items mi ON mi.id = cl.value ->> '$.item_id' WHERE mi.author_id = ?1)),
              ranked AS MATERIALIZED (SELECT adm.*, ROW_NUMBER() OVER (PARTITION BY pos ORDER BY seq, item_id, qid) AS rn FROM adm)
         SELECT * FROM ranked WHERE rn <= ?3
         UNION ALL SELECT pos, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, COUNT(*) FROM ranked WHERE rn > ?3 GROUP BY pos",
        visible_to_others()
    );
    // The SQL window is the looser of the two "firsts"; each candidate is judged below.
    let q = serde_json::to_string(&want.iter().map(|p| serde_json::json!({"pos": p.pos, "bands": p.bands, "qkind": p.qkind, "first": min_first(p.first, p.loose_first)})).collect::<Vec<_>>()).unwrap();
    let by_pos: HashMap<usize, &&PubPos> = want.iter().map(|p| (p.pos, p)).collect();
    let mut st = conn.prepare(&sql)?;
    type Row = (usize, Option<String>, Option<String>, Option<i64>, Option<String>, Option<String>, Option<String>, Option<String>, Option<i64>, Option<String>, Option<String>, Option<String>);
    let rows: Vec<Row> = st
        .query_map(params![me, q, FUZZY_WINDOW, FUZZY_PAIRS_MAX, PUB_BAND_MAX], |r| {
            Ok((r.get::<_, i64>(0)? as usize, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?, r.get(5)?, r.get(6)?, r.get(7)?, r.get(8)?, r.get(9)?, r.get(10)?, r.get(11)?))
        })?
        .collect::<Result<_, _>>()?;
    for (pos_i, item_id, qid, version, license, author_name, stem_tokens, qkind, seq, body_tokens, bk_expl, bk_ans) in rows {
        let Some(item_id) = item_id else {
            cut.insert(pos_i);
            continue;
        };
        let p = by_pos[&pos_i];
        let (qid, license, author_name, seq) = (qid.unwrap_or_default(), license.unwrap_or_default(), author_name.unwrap_or_default(), seq.unwrap_or(0));
        // A source that lacks what her current version ADDED (her explanation or answer) is judged against her earliest own
        // version without it.
        let loose = Fields { expl: p.loose_expl.as_deref(), ans: p.loose_ans.as_deref() };
        let pf = Fields { expl: p.expl.as_deref(), ans: p.ans.as_deref() };
        let first = if lacks_added(pf, bk_expl.as_deref(), bk_ans.as_deref(), Some(loose)) { min_first(p.first, p.loose_first) } else { p.first };
        if qkind.as_deref() != Some(p.qkind.as_str()) || first.is_some_and(|f| seq >= f) {
            continue;
        }
        if jaccard(&p.tokens, &json_strings(stem_tokens).unwrap_or_default()) < FUZZY_JACCARD {
            continue;
        }
        // The same body too (a near stem alone is another question).
        if !same_body(p.body.as_deref(), json_strings(body_tokens).as_deref()) {
            continue;
        }
        let better = match out.get(&pos_i) {
            None => true,
            Some(prev) => seq < prev.seq || (seq == prev.seq && item_id < prev.item_id),
        };
        if better {
            out.insert(pos_i, NearSource { item_id, qid, version: version.unwrap_or(0), license, author_name, seq });
        }
    }
    Ok((out, cut))
}

#[cfg(test)]
#[path = "fuzzynear_tests.rs"]
mod tests;
