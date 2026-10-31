//! The evidence hit and what reads it (worker/src/pool/provenance.ts: `ImportHit`, `bearingOf`, `unitOf`, `chainOf`,
//! `linkGrants`, `stemHits`, `voidedRefs`). The evidence queries that build hits come after this.
use super::chain::ChainLink;
use super::license::{ChainLinkLicense, Grant, LicenceUnit, chain_grants};
use rusqlite::{Connection, params};
use std::collections::{HashMap, HashSet};

/// One source ITEM this account was served that shares a question with ONE position of the publish, as the evidence query
/// groups it (by source item, not by (item, qid), so a bank repeating a question counts once).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ImportHit {
    pub pos: i64,
    pub match_hash: String,
    pub qid: String,
    pub item_id: String,
    /// Links in the chain the source carried at its latest serve.
    pub chain_len: i64,
    /// The licences the source's chains grant.
    pub chain_lics: Vec<String>,
    pub author_name: String,
    /// The licence at the latest serve.
    pub license: String,
    pub version: i64,
    pub versions: Vec<i64>,
    /// Earliest per-content seq of the source (for the "it came first" rule).
    pub source_first_seq: i64,
    pub stem_hash: Option<String>,
    /// "import" or "view".
    pub kind: String,
    pub same: bool,
    pub same_full: bool,
    pub same_imported: bool,
    pub near: bool,
    pub kf_same: bool,
    pub lic_all: Vec<String>,
    pub lic_same: Vec<String>,
    pub lic_near: Vec<String>,
    pub lic_groups: Vec<Vec<String>>,
    pub contents: Vec<String>,
    pub by_ref: bool,
    pub by_match: bool,
    pub fuzzy: bool,
    pub claimed: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Bearing {
    Verbatim,
    Adapted,
    Soft,
}

/// * `verbatim`: some serve is the same question in the licence sense, whether read or imported. Hard: it can refuse.
/// * `adapted`: an import, or an origin the author declared now, that the subject changed. Hard.
/// * `soft`: a plain read the subject changed. Credited and flagged, never a refusal.
pub fn bearing_of(h: &ImportHit, declared: bool) -> Bearing {
    // An attested-independent read of a short or generic question binds nothing.
    if h.claimed {
        return Bearing::Soft;
    }
    if h.near {
        return Bearing::Verbatim;
    }
    if declared || h.kind == "import" {
        return Bearing::Adapted;
    }
    Bearing::Soft
}
pub fn is_hard(h: &ImportHit, declared: bool) -> bool {
    bearing_of(h, declared) != Bearing::Soft
}

/// The licence unit of one source, as its bearing reads it (own licences grouped by served content).
pub fn unit_of(h: &ImportHit) -> LicenceUnit {
    let verbatim = h.near;
    let own = if verbatim {
        vec![if h.lic_near.is_empty() { h.lic_all.clone() } else { h.lic_near.clone() }]
    } else if h.lic_groups.is_empty() {
        vec![h.lic_all.clone()]
    } else {
        h.lic_groups.clone()
    };
    LicenceUnit { item: h.item_id.clone(), verbatim, chain: h.chain_lics.clone(), own, contents: h.contents.clone(), seq: h.source_first_seq }
}

/// A stored chain with the void items dropped and each one the admin tied to the rightful author's item replaced by a link
/// to that item. A stand-in keeps the replaced link's `modified` (a verbatim copy of the infringing item is a verbatim copy
/// of the rightful one). Repeated item ids are kept once.
pub fn chain_of_links(links: &[ChainLink], drop: &HashSet<String>, replaced: &HashMap<String, ChainLink>) -> Vec<ChainLink> {
    let mut out = Vec::new();
    let mut seen: HashSet<String> = HashSet::new();
    for l in links {
        let x = match replaced.get(&l.item_id) {
            Some(rep) => Some(ChainLink { modified: l.modified, ..rep.clone() }),
            None if drop.contains(&l.item_id) => None,
            None => Some(l.clone()),
        };
        let Some(x) = x else { continue };
        if drop.contains(&x.item_id) || (!x.item_id.is_empty() && seen.contains(&x.item_id)) {
            continue;
        }
        seen.insert(x.item_id.clone());
        out.push(x);
    }
    out
}
/// `chainOf` on a stored `chain_json`; an unreadable one is an empty chain.
pub fn chain_of(chain_json: &str, drop: &HashSet<String>, replaced: &HashMap<String, ChainLink>) -> Vec<ChainLink> {
    chain_of_links(&serde_json::from_str::<Vec<ChainLink>>(chain_json).unwrap_or_default(), drop, replaced)
}

/// The grants a chain carries (earlier links as adaptations) plus its immediate source link.
pub fn link_grants(earlier: &[ChainLink], immediate: Option<&Grant>) -> Vec<Grant> {
    let empty: Vec<String> = Vec::new();
    let links: Vec<ChainLinkLicense> = earlier.iter().map(|l| ChainLinkLicense { item_id: &l.item_id, license: &l.license, collapsed_licenses: l.collapsed_licenses.as_ref().unwrap_or(&empty) }).collect();
    let mut out = chain_grants(&links);
    if let Some(g) = immediate.filter(|g| !g.item.is_empty()) {
        out.push(g.clone());
    }
    out
}

/// Served questions whose loose stem hash matches, with the earliest first publish of each (so the author's own earlier
/// work is never flagged).
pub fn stem_hits(conn: &Connection, me: &str, stems: &[String]) -> rusqlite::Result<HashMap<String, i64>> {
    let mut out = HashMap::new();
    if stems.is_empty() {
        return Ok(out);
    }
    let mut uniq: Vec<&String> = Vec::new();
    for s in stems {
        if !uniq.contains(&s) {
            uniq.push(s);
        }
    }
    let mut st = conn.prepare(
        "SELECT stem_hash, MIN(source_first_seq) AS s FROM import_questions WHERE user_id = ?1 AND author_id <> ?1 AND void_at IS NULL AND stem_hash IN (SELECT value FROM json_each(?2)) GROUP BY stem_hash",
    )?;
    let rows = st.query_map(params![me, serde_json::to_string(&uniq).unwrap()], |r| Ok((r.get::<_, String>(0)?, r.get::<_, i64>(1)?)))?;
    for r in rows {
        let (h, s) = r?;
        out.insert(h, s);
    }
    Ok(out)
}

/// SQL: served row `s` holds question `j.value ->> '$.qid'` (its live publish or its ghost).
const HAS_Q: &str = "(EXISTS (SELECT 1 FROM item_questions hq WHERE hq.publish_id = s.publish_id AND hq.qid = j.value ->> '$.qid')
               OR EXISTS (SELECT 1 FROM ghost_questions hg WHERE hg.publish_id = s.publish_id AND hg.qid = j.value ->> '$.qid'))";

/// Recorded origins (`item/qid`) whose evidence for `me` was voided: a kept question's provenance from such a record is
/// void too, never reinstated as "a record that is merely gone".
pub fn voided_refs(conn: &Connection, me: &str, refs: &[(String, String)]) -> rusqlite::Result<HashSet<String>> {
    let mut out = HashSet::new();
    if refs.is_empty() {
        return Ok(out);
    }
    let json: Vec<serde_json::Value> = refs.iter().map(|(i, q)| serde_json::json!({"item": i, "qid": q})).collect();
    // Per recorded origin, the reader's served publishes of that item holding the question.
    let sql = format!(
        "SELECT DISTINCT j.value ->> '$.item' AS item_id, j.value ->> '$.qid' AS qid FROM json_each(?2) j
          WHERE EXISTS (SELECT 1 FROM served s WHERE s.reader_id = ?1 AND s.item_id = j.value ->> '$.item' AND s.void_at IS NOT NULL AND {HAS_Q})
            AND NOT EXISTS (SELECT 1 FROM served s WHERE s.reader_id = ?1 AND s.item_id = j.value ->> '$.item' AND s.void_at IS NULL AND {HAS_Q})
            AND NOT EXISTS (SELECT 1 FROM void_items vr WHERE vr.item_id = j.value ->> '$.item' AND vr.replaced_by IS NOT NULL)
         UNION SELECT j.value ->> '$.item', j.value ->> '$.qid' FROM json_each(?2) j WHERE EXISTS (SELECT 1 FROM void_items vi WHERE vi.item_id = j.value ->> '$.item' AND vi.replaced_by IS NULL)"
    );
    let mut st = conn.prepare(&sql)?;
    let rows = st.query_map(params![me, serde_json::to_string(&json).unwrap()], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)))?;
    for r in rows {
        let (i, q) = r?;
        out.insert(format!("{i}/{q}"));
    }
    Ok(out)
}

#[cfg(test)]
#[path = "hit_tests.rs"]
mod tests;
