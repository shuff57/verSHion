//! Sources this account was served whose stem is a near-rewording of a position's stem (worker/src/pool/provenance.ts
//! `fuzzyHits` / `fuzzyQuery`): another match_hash, token Jaccard >= FUZZY_JACCARD, found through the MinHash bands.
//! Admissibility as in the evidence query: never the publisher's own items or void ones, never later than the publisher's
//! own first publish, never one the publisher attests independence of. Imports become ADAPTATIONS (hard), reads soft;
//! every one raises origin_suspect.
//!
//! The SQL is the Worker's, verbatim, in `fuzzy_a.sql` / `fuzzy_branch.sql` / `fuzzy_b.sql` (the two import views are
//! joined between them), so the bounds and the admission rules cannot drift from it.
use super::chain::ChainLink;
use super::evidence::FuzzyCut;
use super::fingerprint::Fingerprinter;
use super::fuzzy::*;
use super::hit::ImportHit;
use super::qhash::{jaccard, same_body};
use rusqlite::{Connection, params};
use std::collections::{HashMap, HashSet};

#[derive(Clone, Debug)]
pub struct FuzzyPos {
    pub pos: usize,
    pub mh: String,
    pub tokens: Vec<String>,
    pub bands: Vec<String>,
    pub first: Option<i64>,
    pub nfull: Option<String>,
    pub nbare: Option<String>,
    pub noexpl: bool,
    pub body: Option<Vec<String>>,
}

#[derive(Debug, Default)]
pub struct FuzzyResult {
    pub hits: Vec<ImportHit>,
    /// A cap or a hot band applied (conservative: the publish is flagged).
    pub capped: bool,
    pub cut: HashMap<usize, FuzzyCut>,
    /// Positions whose near stem has another body: an admin flag at most, never a credit, a binding or a label.
    pub stem_only: HashSet<usize>,
}

fn fuzzy_sql() -> String {
    let branch = include_str!("fuzzy_branch.sql");
    let rows: Vec<String> = ["import_questions_live", "import_questions_ghost"].iter().map(|v| format!("\n{}", branch.replace("${v}", v))).collect();
    format!("{}{}{}", include_str!("fuzzy_a.sql"), rows.join("\n              UNION ALL"), include_str!("fuzzy_b.sql"))
}

fn union_keep_order(a: &[String], b: &[String]) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for x in a.iter().chain(b) {
        if !out.contains(x) {
            out.push(x.clone());
        }
    }
    out
}

fn add_cut(cut: &mut HashMap<usize, FuzzyCut>, pos: usize, lics: &[String], item: &str, n: i64, by: &HashMap<String, String>) {
    match cut.get(&pos) {
        Some(prev) => {
            let mut merged = by.clone();
            merged.extend(prev.by.clone()); // the earlier cut's named items win
            let c = FuzzyCut { lics: union_keep_order(&prev.lics, lics), item: prev.item.clone(), n: prev.n + n, by: merged };
            cut.insert(pos, c);
        }
        None => {
            cut.insert(pos, FuzzyCut { lics: union_keep_order(&[], lics), item: item.to_string(), n, by: by.clone() });
        }
    }
}

fn strs(v: Option<String>) -> Vec<String> {
    let parsed: Vec<serde_json::Value> = v.and_then(|s| serde_json::from_str(&s).ok()).unwrap_or_default();
    parsed.into_iter().filter_map(|x| x.as_str().map(String::from)).collect()
}
fn str_vec(v: Option<String>) -> Option<Vec<String>> {
    v.and_then(|s| serde_json::from_str(&s).ok())
}

pub fn fuzzy_hits(conn: &Connection, fp: &Fingerprinter, me: &str, pos: &[FuzzyPos], mine: &[String], independent: &[String], exact: &[usize]) -> rusqlite::Result<FuzzyResult> {
    let want: Vec<&FuzzyPos> = pos.iter().filter(|p| !p.bands.is_empty()).collect();
    if want.is_empty() {
        return Ok(FuzzyResult::default());
    }
    // Everything is bounded in SQL. At most FUZZY_PER_POSITION candidate (item, qid) pairs per question and FUZZY_PAIRS_MAX
    // in all; at most FUZZY_ROWS_MAX evidence rows examined (one per served content), aggregated per pair, so a source
    // republished many times cannot multiply what reaches the Worker. Past a cap: flagged, conservative.
    // Each band comes with its keyed form, which is what a ghost is found by.
    let mut sk_memo: HashMap<String, String> = HashMap::new();
    let subjects: Vec<serde_json::Value> = want
        .iter()
        .map(|p| {
            let bands: Vec<[String; 2]> = p.bands.iter().map(|b| [b.clone(), fp.keyed_one("b:", b, &mut sk_memo)]).collect();
            serde_json::json!({"pos": p.pos, "mh": p.mh, "bands": bands, "nfull": p.nfull, "nbare": p.nbare, "noexpl": p.noexpl as i64, "first": p.first})
        })
        .collect();
    let mut st = conn.prepare(&fuzzy_sql())?;
    let mut q = st.query(params![
        me,
        serde_json::to_string(&subjects).unwrap(),
        serde_json::to_string(mine).unwrap(),
        FUZZY_WINDOW,
        FUZZY_PAIRS_MAX,
        FUZZY_ROWS_MAX,
        PUB_BAND_MAX,
        serde_json::to_string(exact).unwrap(),
        serde_json::to_string(independent).unwrap()
    ])?;
    struct Row {
        pos: Option<usize>,
        item_id: String,
        qid: String,
        match_hash: String,
        imported: bool,
        fs: Option<i64>,
        an: String,
        ll: String,
        version: Option<i64>,
        near: bool,
        chain: String,
        toks: Option<String>,
        btoks: Option<String>,
        tsk: Option<String>,
        bsk: Option<String>,
        sh: Option<String>,
        contents: Option<String>,
        versions: Option<String>,
        lics: Option<String>,
        nlics: Option<String>,
    }
    let mut rows: Vec<Row> = Vec::new();
    let (mut examined, mut pairs, mut hot) = (0i64, 0i64, 0i64);
    let mut cut_json: Option<String> = None;
    let mut win_json: Option<String> = None;
    let mut first_row = true;
    while let Some(r) = q.next()? {
        if first_row {
            examined = r.get::<_, Option<i64>>("examined")?.unwrap_or(0);
            pairs = r.get::<_, Option<i64>>("pairs")?.unwrap_or(0);
            hot = r.get::<_, Option<i64>>("hot")?.unwrap_or(0);
            cut_json = r.get("cut")?;
            win_json = r.get("win")?;
            first_row = false;
        }
        let pos: Option<i64> = r.get("pos")?;
        rows.push(Row {
            pos: pos.map(|p| p as usize),
            item_id: r.get::<_, Option<String>>("item_id")?.unwrap_or_default(),
            qid: r.get::<_, Option<String>>("qid")?.unwrap_or_default(),
            match_hash: r.get::<_, Option<String>>("match_hash")?.unwrap_or_default(),
            imported: r.get::<_, Option<i64>>("imported")?.unwrap_or(0) != 0,
            fs: r.get("fs")?,
            an: r.get::<_, Option<String>>("an")?.unwrap_or_default(),
            ll: r.get::<_, Option<String>>("ll")?.unwrap_or_default(),
            version: r.get("version")?,
            near: r.get::<_, Option<i64>>("near")? == Some(1),
            chain: r.get::<_, Option<String>>("chain")?.unwrap_or_default(),
            toks: r.get("toks")?,
            btoks: r.get("btoks")?,
            tsk: r.get("tsk")?,
            bsk: r.get("bsk")?,
            sh: r.get("sh")?,
            contents: r.get("contents")?,
            versions: r.get("versions")?,
            lics: r.get("lics")?,
            nlics: r.get("nlics")?,
        });
    }
    // No row at all still says whether a cap or a hot band applied (the stats row is joined to each).
    let capped = !rows.is_empty() && (examined >= FUZZY_ROWS_MAX || pairs >= FUZZY_PAIRS_MAX || hot > 0);
    // Imported near candidates left out (past the window or the pair allowance, and every candidate if the row allowance
    // was reached): their licences, per question.
    #[derive(serde::Deserialize)]
    struct CutRow {
        pos: usize,
        lics: Vec<String>,
        item: String,
        n: i64,
        #[serde(default)]
        by: HashMap<String, String>,
    }
    let mut cut: HashMap<usize, FuzzyCut> = HashMap::new();
    for x in serde_json::from_str::<Vec<CutRow>>(cut_json.as_deref().unwrap_or("[]")).unwrap_or_default() {
        add_cut(&mut cut, x.pos, &x.lics, &x.item, x.n, &x.by);
    }
    if examined >= FUZZY_ROWS_MAX {
        for x in serde_json::from_str::<Vec<CutRow>>(win_json.as_deref().unwrap_or("[]")).unwrap_or_default() {
            if !exact.contains(&x.pos) {
                add_cut(&mut cut, x.pos, &x.lics, &x.item, x.n, &x.by);
            }
        }
    }
    let by_pos: HashMap<usize, &&FuzzyPos> = want.iter().map(|p| (p.pos, p)).collect();
    let mine_set: HashSet<&String> = mine.iter().collect();
    let indep: HashSet<&String> = independent.iter().collect();
    let mut order: Vec<String> = Vec::new();
    let mut out: HashMap<String, ImportHit> = HashMap::new();
    let mut stem_only: HashSet<usize> = HashSet::new();
    for x in rows {
        let Some(pos) = x.pos else { continue }; // the stats row alone
        let p = by_pos[&pos];
        // A ghost has no words, only its keyed sketch: compared keyed set to keyed set.
        let via_sketch = x.toks.is_none() && x.tsk.is_some();
        let mine_toks = if via_sketch { fp.keyed_set(&p.tokens, &mut sk_memo, "t:") } else { p.tokens.clone() };
        let theirs = str_vec(if via_sketch { x.tsk.clone() } else { x.toks.clone() }).unwrap_or_default();
        if jaccard(&mine_toks, &theirs) < FUZZY_JACCARD {
            continue;
        }
        // A near STEM with another body (choices, answers, kind) is another question: an admin flag at most
        // (origin_suspect), never a credit, a binding or a label.
        let their_body = if via_sketch { str_vec(x.bsk.clone()) } else { str_vec(x.btoks.clone()) };
        let my_body = match (&p.body, via_sketch) {
            (Some(b), true) => Some(fp.keyed_body(b, &mut sk_memo)),
            (b, _) => b.clone(),
        };
        if !same_body(my_body.as_deref(), their_body.as_deref()) {
            stem_only.insert(pos);
            continue;
        }
        // Independence never covers a copy in the licence sense (order-free, e.g. parts reordered).
        if indep.contains(&x.item_id) && !x.near {
            continue;
        }
        let chain: Vec<ChainLink> = {
            let s: String = x.chain.chars().skip(17).collect();
            serde_json::from_str(if s.is_empty() { "[]" } else { &s }).unwrap_or_default()
        };
        if chain.iter().any(|l| mine_set.contains(&l.item_id)) {
            continue;
        }
        let key = format!("{pos}/{}", x.item_id);
        let lics = strs(x.lics.clone());
        if let Some(prev) = out.get_mut(&key) {
            prev.near |= x.near;
            prev.lic_near = union_keep_order(&prev.lic_near, &strs(x.nlics.clone()));
            prev.lic_all = union_keep_order(&prev.lic_all, &lics);
            prev.lic_groups.push(lics);
            prev.contents.extend(strs(x.contents.clone()));
            if x.imported {
                prev.kind = "import".into();
            }
            prev.source_first_seq = prev.source_first_seq.min(x.fs.unwrap_or(0));
            continue;
        }
        let mut chain_lics: Vec<String> = Vec::new();
        for l in &chain {
            let mut add = Vec::new();
            if !l.license.is_empty() {
                add.push(l.license.clone());
            }
            add.extend(l.collapsed_licenses.clone().unwrap_or_default());
            chain_lics = union_keep_order(&chain_lics, &add);
        }
        let versions: Vec<i64> = serde_json::from_str::<Vec<serde_json::Value>>(x.versions.as_deref().unwrap_or("[]")).unwrap_or_default().iter().filter_map(|v| v.as_i64()).collect();
        order.push(key.clone());
        out.insert(
            key,
            ImportHit {
                pos: pos as i64,
                match_hash: x.match_hash,
                qid: x.qid,
                item_id: x.item_id,
                chain_len: chain.len() as i64,
                chain_lics,
                author_name: x.an.chars().skip(17).collect(),
                license: x.ll.chars().skip(17).collect(),
                version: x.version.unwrap_or(0),
                versions,
                source_first_seq: x.fs.unwrap_or(0),
                stem_hash: x.sh,
                kind: if x.imported { "import".into() } else { "view".into() },
                near: x.near,
                lic_all: lics.clone(),
                lic_near: if x.near { strs(x.nlics) } else { vec![] },
                lic_groups: vec![lics],
                contents: strs(x.contents),
                by_match: true,
                fuzzy: true,
                ..Default::default()
            },
        );
    }
    // ... and past FUZZY_KEEP ADMISSIBLE sources per question (earliest kept): the rest, likewise.
    let mut by_q: Vec<(usize, Vec<ImportHit>)> = Vec::new();
    for k in &order {
        let h = out.remove(k).unwrap();
        let pos = h.pos as usize;
        match by_q.iter_mut().find(|(p, _)| *p == pos) {
            Some((_, v)) => v.push(h),
            None => by_q.push((pos, vec![h])),
        }
    }
    let exact_set: HashSet<usize> = exact.iter().copied().collect();
    let mut kept = Vec::new();
    for (pos, mut hs) in by_q {
        hs.sort_by(|a, b| a.source_first_seq.cmp(&b.source_first_seq).then_with(|| if a.item_id < b.item_id { std::cmp::Ordering::Less } else { std::cmp::Ordering::Greater }));
        let rest: Vec<ImportHit> = hs.iter().skip(FUZZY_KEEP as usize).filter(|h| h.kind == "import").cloned().collect();
        hs.truncate(FUZZY_KEEP as usize);
        kept.extend(hs);
        if !rest.is_empty() && !exact_set.contains(&pos) {
            let mut by: HashMap<String, String> = HashMap::new();
            let mut lics: Vec<String> = Vec::new();
            for h in &rest {
                for l in h.lic_all.iter().chain(&h.chain_lics) {
                    if !by.contains_key(l) {
                        by.insert(l.clone(), h.item_id.clone());
                        lics.push(l.clone());
                    }
                }
            }
            add_cut(&mut cut, pos, &lics, &rest[0].item_id, rest.len() as i64, &by);
        }
    }
    Ok(FuzzyResult { hits: kept, capped, cut, stem_only })
}

#[cfg(test)]
#[path = "fuzzyhits_tests.rs"]
mod tests;
