//! The evidence a publish's licence rule reads (worker/src/pool/publish.ts `publishOnce`, from the evidence call to the void lookups): every source this
//! account was served that shares a question with a position, the near-rewordings of what it was served, the author's own earlier versions that rule a later
//! source out, and the void items to drop. Nothing here decides a licence; the derivation step does.
use super::chain::{ChainLink, void_info};
use super::ctx::{PoolCtx, Res};
use super::evidence::{CRoot, Evidence, EvidenceOpts, FuzzyCut, PositionFacts, evidence};
use super::fingerprint::Fingerprinter;
use super::fuzzy::FUZZY_ROWS_MAX;
use super::fuzzyhits::{FuzzyPos, FuzzyResult, fuzzy_hits};
use super::fuzzynear::{LooseFirst, OwnPos, own_fuzzy_first};
use super::hit::voided_refs;
use super::publishfacts::Facts;
use super::publishhash::Hashed;
use super::publishmeta::PublishMeta;
use super::publishrewrite::Rewritten;
use super::sqlfrag::is_id;
use super::visible::{SourceRef, subject_param};
use super::publishunits::Units;
use rusqlite::params;
use serde_json::json;
use std::collections::{HashMap, HashSet};

#[derive(Debug, Default)]
pub struct Evid {
    pub ev: Evidence,
    /// The author's own near-rewordings published before, by position (earliest seq).
    pub own_fuzzy: HashMap<usize, i64>,
    pub own_cut: HashSet<usize>,
    pub own_loose: HashMap<usize, LooseFirst>,
    /// The near-stem hits' own cuts per position.
    pub fuzzy_cut: HashMap<usize, FuzzyCut>,
    /// Positions credited or flagged for a near stem (origin_suspect).
    pub fuzzy_pos: HashSet<usize>,
    /// Positions whose near source is on the author's own lineage: never credited or binding, only flagged.
    pub lineage_soft: HashSet<usize>,
    /// Recorded origins (`item/qid`) whose evidence was voided.
    pub void_refs: HashSet<String>,
    /// Items dropped from every chain (the account's own, the void ones) and the stand-ins for some of them.
    pub drop: HashSet<String>,
    pub replaced: HashMap<String, ChainLink>,
    /// Declared origins that are the account's own items.
    pub own_items: HashSet<String>,
}

impl Evid {
    /// The earliest of her own first publish, the qid's lineage and her own near-rewordings (for fuzzy sources and origin_suspect).
    pub fn fuzzy_first(&self, f: &Facts, i: usize) -> Option<i64> {
        [f.own_first.get(&i).copied(), f.lineage[i], self.own_fuzzy.get(&i).copied()].into_iter().flatten().filter(|x| *x > 0).min()
    }
    /// ... without the lineage, which never hides a source the account was served.
    pub fn first_no_lineage(&self, f: &Facts, i: usize) -> Option<i64> {
        [f.own_first.get(&i).copied(), self.own_fuzzy.get(&i).copied()].into_iter().flatten().filter(|x| *x > 0).min()
    }
}

/// Looks up the evidence for every position and merges the near-stem hits into it.
#[allow(clippy::too_many_arguments)]
pub fn collect(x: &PoolCtx, fp: &Fingerprinter, me: &str, m: &PublishMeta, rw: &Rewritten, h: &Hashed, f: &Facts, _units: &Units) -> Res<Evid> {
    let n = rw.n;
    let mut out = Evid::default();
    let own_pos: Vec<OwnPos> = (0..n)
        .map(|i| OwnPos { pos: i, tokens: h.tokens[i].clone(), bands: h.bands[i].clone(), body: f.body[i].clone(), body_toks: Some(h.body_tokens[i].clone()), expl: f.bk_fields[i].0.clone(), ans: f.bk_fields[i].1.clone() })
        .collect();
    out.own_fuzzy = own_fuzzy_first(x.conn, fp, me, &own_pos, &mut out.own_cut, &mut out.own_loose)?;
    let positions: Vec<PositionFacts> = (0..n)
        .map(|i| {
            let sj = &h.subjects[i];
            let refs = f.refs[i].iter().map(|(item, qid)| SourceRef { item: item.clone(), qid: qid.clone() }).collect();
            PositionFacts {
                subject: subject_param(sj, &h.match_hashes[i], refs),
                first: f.own_first.get(&i).copied(),
                ownshown: sj.full.as_deref().is_some_and(|fu| f.actx.own.get(&h.match_hashes[i]).is_some_and(|o| o.contains(fu))),
                common: f.short_q[i],
                keywork: f.keywork[i] as i64,
                short40: f.short40[i] as i64,
                croots: f.actx.roots.get(&h.match_hashes[i]).map(|rs| rs.iter().map(|r| CRoot { r: r.r.clone(), s: r.s, n: r.n.clone() }).collect()).unwrap_or_default(),
                ck: f.actx.k,
                ownkf: f.actx.own_kf.contains(&h.near_kf[i]) && !f.keywork[i],
            }
        })
        .collect();
    let mine: Vec<String> = f.actx.mine.iter().cloned().collect();
    out.ev = evidence(x.conn, me, &positions, &EvidenceOpts { mine: &mine, independent: &m.independent })?;
    let fpos: Vec<FuzzyPos> = (0..n)
        .map(|i| FuzzyPos {
            pos: i,
            mh: h.match_hashes[i].clone(),
            tokens: h.tokens[i].clone(),
            bands: h.bands[i].clone(),
            first: out.first_no_lineage(f, i),
            nfull: h.subjects[i].near.clone(),
            nbare: h.subjects[i].near_bare.clone(),
            noexpl: h.subjects[i].no_expl,
            body: Some(h.body_tokens[i].clone()),
        })
        .collect();
    let exact: Vec<usize> = out.ev.by_pos.iter().filter(|(_, l)| !l.is_empty()).map(|(k, _)| *k).collect();
    let fuzzy: FuzzyResult = fuzzy_hits(x.conn, fp, me, &fpos, &mine, &m.independent, &exact)?;
    // Past the fuzzy caps: every question that had candidates is flagged for the admin (conservative).
    if fuzzy.capped {
        for (i, bs) in h.bands.iter().enumerate() {
            if !bs.is_empty() && !out.ev.overflow.contains_key(&i) {
                out.ev.overflow.insert(i, FUZZY_ROWS_MAX);
            }
        }
    }
    // A near stem with another body: the admin is told (origin_suspect), nothing else.
    out.fuzzy_pos.extend(fuzzy.stem_only.iter().copied());
    for hit in &fuzzy.hits {
        let pos = hit.pos as usize;
        if let Some(ls) = f.lineage[pos] {
            if hit.source_first_seq >= ls {
                out.lineage_soft.insert(pos);
                continue;
            }
        }
        let list = out.ev.by_pos.entry(pos).or_default();
        if list.iter().any(|x| x.item_id == hit.item_id) {
            continue;
        }
        list.push(hit.clone());
        out.fuzzy_pos.insert(pos);
    }
    out.fuzzy_cut = fuzzy.cut;
    // Kept provenance whose record was voided is void, never "a record that is merely gone"; links to the publisher's own items or to void items are dropped.
    out.void_refs = voided_refs(x.conn, me, &f.carried_refs)?;
    let mut ids: Vec<String> = out.ev.by_pos.values().flatten().map(|hh| hh.item_id.clone()).collect();
    ids.extend(out.ev.credits.values().flat_map(|c| c.links.iter().map(|l| l.item_id.clone())));
    for p in f.prev.values() {
        ids.push(p.origin_item_id.clone().unwrap_or_default());
        ids.extend(serde_json::from_str::<Vec<ChainLink>>(&p.chain_json).unwrap_or_default().into_iter().map(|l| l.item_id));
    }
    let vi = void_info(x.conn, &ids)?;
    out.drop = f.actx.mine.iter().cloned().chain(vi.voided).collect();
    out.replaced = vi.replaced;
    let declared: Vec<&String> = rw.declared.iter().flatten().map(|d| &d.item).filter(|i| is_id(i)).collect();
    if !declared.is_empty() {
        let mut st = x.conn.prepare("SELECT id FROM items WHERE author_id = ?1 AND id IN (SELECT value FROM json_each(?2))")?;
        for r in st.query_map(params![me, json!(declared).to_string()], |r| r.get::<_, String>(0))? {
            out.own_items.insert(r?);
        }
    }
    Ok(out)
}

#[cfg(test)]
#[path = "publishevid_tests.rs"]
mod tests;
