//! THE per-question licence derivation (worker/src/pool/publish.ts `publishOnce`, "licence and provenance": the loop over positions). Pure: it reads the facts, the
//! evidence and the units already gathered and decides, per question, its credit chain, the licences it allows, which items bind it, the marks readers are
//! told, and which positions are refused, grandfathered, in conflict or only soft. Nothing is read from the database and nothing is written.
use super::chain::ChainLink;
use super::evidence::{CREDITS_HEAD, CREDITS_MAX};
use super::hit::{ImportHit, chain_of, chain_of_links, is_hard, unit_of};
use super::license::{Grant, LicenceUnit, decide_licence, grant_options, may_republish, relied_licence, unit_allows};
use super::publishevid::Evid;
use super::publishfacts::Facts;
use super::publishhash::Hashed;
use super::publishmeta::PublishMeta;
use super::publishrewrite::Rewritten;
use super::publishunits::{Lift, UnitKind, Units};
use super::sqlfrag::is_id;
use super::textutil::collapse_chain;
use serde_json::json;
use std::collections::{HashMap, HashSet};

/// Links stored per question.
pub const STORED_CHAIN_MAX: usize = CREDITS_MAX as usize + 2;

/// What readers are told about a question beyond its recorded sources.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Marks {
    /// An unverified soft source.
    pub soft: bool,
    /// An attested-independent possible source.
    pub claimed: bool,
    /// An earlier identical publication by someone else with no evidence of a serve.
    pub earlier: bool,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct Entry {
    pub chain: Vec<ChainLink>,
    pub origin_item: Option<String>,
    pub origin_qid: Option<String>,
    pub modified: Option<bool>,
}

#[derive(Debug, Default)]
pub struct Derived {
    pub entries: Vec<Entry>,
    pub binders: Vec<Vec<String>>,
    pub per_question: Vec<Vec<&'static str>>,
    /// What the rule allowed per question and which items decided it, stored with it.
    pub allowed_json: Vec<String>,
    pub true_bound: Vec<Vec<(String, ChainLink)>>,
    pub marks: Vec<Marks>,
    pub incompatible: Vec<usize>,
    pub conflicts: Vec<usize>,
    pub grandfathered: Vec<usize>,
    pub overflowed: Vec<usize>,
    pub soft_only: Vec<usize>,
    /// Positions whose hidden-only text (served to this account elsewhere) forbids the licence: refused to the author alone.
    pub hidden_refused: Vec<usize>,
    /// ... and the items it was shown that material in, with the licences it was shown, in the order found.
    pub hidden_from: Vec<(String, Vec<String>)>,
    pub unverified: Vec<(usize, String)>,
    pub declared_modified: Vec<(usize, String)>,
    /// Questions refused because a source left out past an allowance forbids the licence; the item that does.
    pub cut_refused: HashMap<usize, String>,
    pub fuzzy_pos: HashSet<usize>,
}

fn is_word(b: u8) -> bool {
    b.is_ascii_alphanumeric() || b == b'_'
}
/// An independence statement that attributes the material to a public source (a URL alone attributes nothing): `\b(public domain|cc[- ]?by|cc0|creative
/// commons|openstax|wikimedia|wikipedia)\b`, case-insensitive.
pub fn attributes_public_source(statement: &str) -> bool {
    let s = statement.to_lowercase();
    let b = s.as_bytes();
    ["public domain", "cc by", "cc-by", "ccby", "cc0", "creative commons", "openstax", "wikimedia", "wikipedia"].iter().any(|w| {
        let mut from = 0;
        while let Some(at) = s[from..].find(w) {
            let (a, e) = (from + at, from + at + w.len());
            if (a == 0 || !is_word(b[a - 1])) && (e >= b.len() || !is_word(b[e])) {
                return true;
            }
            from = a + 1;
            while !s.is_char_boundary(from) {
                from += 1;
            }
        }
        false
    })
}

fn licences_of(chain: &[ChainLink]) -> Vec<String> {
    chain.iter().flat_map(|x| x.license.is_empty().then(Vec::new).unwrap_or_else(|| vec![x.license.clone()]).into_iter().chain(x.collapsed_licenses.clone().unwrap_or_default())).collect()
}
fn push_unique<T: PartialEq>(v: &mut Vec<T>, x: T) {
    if !v.contains(&x) {
        v.push(x);
    }
}
fn kind_tag(k: UnitKind) -> &'static str {
    match k {
        UnitKind::Text => "text",
        UnitKind::Image => "image",
        UnitKind::Part => "part",
    }
}
fn link_of(h: &ImportHit, modified: bool) -> ChainLink {
    ChainLink { author_name: h.author_name.clone(), item_id: h.item_id.clone(), qid: h.qid.clone(), version: h.version, license: h.license.clone(), modified, ..Default::default() }
}
fn placeholder(text: String, lics: Vec<String>) -> ChainLink {
    ChainLink { author_name: text, modified: true, collapsed_licenses: Some(lics), ..Default::default() }
}

/// Derives every question's provenance and licence outcome. `prev_license` and `prev_hidden` are the item's before this publish (None / false for a new item).
#[allow(clippy::too_many_arguments)]
pub fn derive(m: &PublishMeta, rw: &Rewritten, h: &Hashed, f: &Facts, e: &Evid, u: &Units, prev_license: Option<&str>, prev_hidden: bool) -> Derived {
    let n = rw.n;
    let attributed = m.statement.as_deref().is_some_and(attributes_public_source);
    let mut out = Derived { fuzzy_pos: e.fuzzy_pos.clone(), marks: vec![Marks::default(); n], ..Default::default() };
    let by_key = |a: &ImportHit, b: &ImportHit| (format!("{}{}", a.item_id, a.qid)).cmp(&format!("{}{}", b.item_id, b.qid));
    for i in 0..n {
        let d = rw.declared[i].as_ref();
        let found: Vec<ImportHit> = e.ev.by_pos.get(&i).cloned().unwrap_or_default();
        let hits: Vec<&ImportHit> = found.iter().filter(|x| x.by_match).collect();
        let d_hit: Option<&ImportHit> = d.and_then(|d| found.iter().find(|x| x.by_ref && x.item_id == d.item && x.versions.contains(&d.version)));
        if let Some(d) = d {
            if d_hit.is_none() && !hits.iter().any(|x| x.item_id == d.item) && !e.own_items.contains(&d.item) {
                out.unverified.push((i, if is_id(&d.item) { d.item.clone() } else { String::new() }));
            }
        }
        if let Some(dh) = d_hit {
            if !dh.kf_same {
                out.declared_modified.push((i, dh.item_id.clone()));
            }
        }
        let pv = f.prev.get(&rw.qids[i]);
        let carried: Option<&ImportHit> = pv.and_then(|p| {
            let (oi, oq) = (p.origin_item_id.as_ref().filter(|s| !s.is_empty())?, p.origin_qid.as_ref().filter(|s| !s.is_empty())?);
            let _ = oq;
            found.iter().find(|x| x.by_ref && &x.item_id == oi)
        });
        // The sources in order (matched, declared, carried), one per item; `true` marks the one that IS the declared hit.
        let mut sources0: Vec<(&ImportHit, bool)> = vec![];
        let mut seen: HashSet<&str> = HashSet::new();
        for (x, is_d) in hits.iter().map(|x| (*x, false)).chain(d_hit.map(|x| (x, true))).chain(carried.map(|x| (x, false))) {
            if seen.insert(x.item_id.as_str()) {
                sources0.push((x, is_d));
            }
        }
        // A near-rewording found only by its stem is no credit when the question has a source matched on the question itself: it is flagged, not credited.
        let sources: Vec<(&ImportHit, bool)> = if sources0.iter().any(|(x, _)| !x.fuzzy) { sources0.iter().filter(|(x, _)| !x.fuzzy).copied().collect() } else { sources0 };
        if e.ev.overflow.contains_key(&i) {
            out.overflowed.push(i);
        }
        let cut_here: Vec<&super::evidence::FuzzyCut> =
            [if found.iter().any(|x| x.near && !x.fuzzy) { None } else { e.ev.cut.get(&i) }, e.fuzzy_cut.get(&i)].into_iter().flatten().collect();
        // THE licence rule: a copy of a source in the licence sense and an adaptation of an import or of a declared / carried origin are hard; identical sources are
        // united, distinct materials intersected, a contradiction never locks out. A changed copy of something only read is soft: credited, flagged, never a refusal.
        let hard_of = |x: &(&ImportHit, bool)| is_hard(x.0, x.1);
        let mut units: Vec<LicenceUnit> = sources.iter().filter(|s| hard_of(s)).map(|s| unit_of(s.0)).collect();
        let soft_units: Vec<LicenceUnit> = sources.iter().filter(|s| !hard_of(s)).map(|s| unit_of(s.0)).collect();
        let mut entry: Entry;
        if !sources.is_empty() {
            // The identical sources are all credited, in the order they were first published; with none identical, every candidate is merged. Never by version
            // numbers across items, never by import order, never by the hidden key.
            let mut sorted: Vec<&ImportHit> = sources.iter().map(|s| s.0).collect();
            sorted.sort_by(|a, b| a.source_first_seq.cmp(&b.source_first_seq).then_with(|| by_key(a, b)));
            let identical: Vec<&ImportHit> = sorted.iter().copied().filter(|x| x.same).collect();
            let pool = if identical.is_empty() { &sorted } else { &identical };
            let src = pool.iter().copied().fold(pool[0], |a, b| if b.chain_len > a.chain_len { b } else { a });
            let cred = e.ev.credits.get(&i);
            let mut links: Vec<ChainLink> = cred.map(|c| c.links.clone()).unwrap_or_default();
            if let Some(c) = cred {
                if c.total > links.len() as i64 {
                    let missing = if c.missing != 0 { c.missing } else { c.total - links.len() as i64 };
                    let head = (CREDITS_HEAD as usize).min(links.len());
                    let tail = links.split_off(head);
                    links.push(placeholder(format!("and {missing} contributors between"), c.more.clone()));
                    links.extend(tail);
                }
            }
            let mut chain = chain_of_links(&links, &e.drop, &e.replaced);
            if !chain.iter().any(|l| l.item_id == src.item_id) {
                chain.push(link_of(src, !src.same));
            }
            for x in &sorted {
                if x.fuzzy && !chain.iter().any(|l| l.item_id == x.item_id) {
                    chain.push(link_of(x, true));
                }
            }
            // A source the author attested independence of stays credited, as a possible source; it binds nothing.
            for x in &sorted {
                if x.claimed {
                    for l in chain.iter_mut().filter(|l| l.item_id == x.item_id) {
                        l.possible_source = Some(true);
                    }
                }
            }
            chain = collapse_chain(&chain, STORED_CHAIN_MAX);
            entry = Entry { chain, origin_item: Some(src.item_id.clone()), origin_qid: Some(src.qid.clone()), modified: Some(!src.same) };
        } else if let Some(p) = pv.filter(|p| {
            p.chain_json != "[]"
                && !(p.origin_item_id.as_ref().is_some_and(|s| !s.is_empty()) && p.origin_qid.as_ref().is_some_and(|s| !s.is_empty()) && e.void_refs.contains(&format!("{}/{}", p.origin_item_id.as_deref().unwrap_or(""), p.origin_qid.as_deref().unwrap_or(""))))
                && !chain_of(&p.chain_json, &e.drop, &e.replaced).is_empty()
        }) {
            // Provenance whose import record is gone: kept as it was, and an edit counts as a modification. Key-free comparison whenever either version hides its
            // key, so a public v2 never reveals whether a hidden v1's key differed. A VOIDED record is not "gone": its provenance is dropped. Carried provenance is hard.
            let mut chain = chain_of(&p.chain_json, &e.drop, &e.replaced);
            let changed = if m.hide_key || prev_hidden { p.keyfree_content != h.keyfree[i] } else { p.full_content != h.full_content[i] };
            if changed {
                if let Some(last) = chain.last_mut() {
                    last.modified = true;
                }
            }
            let last = chain.last().cloned();
            units.push(LicenceUnit {
                item: last.as_ref().map(|l| l.item_id.clone()).filter(|s| !s.is_empty()).unwrap_or_else(|| "carried".into()),
                verbatim: !last.as_ref().map_or(true, |l| l.modified),
                contents: vec![],
                chain: licences_of(&chain[..chain.len() - 1]),
                own: last.as_ref().filter(|l| !l.license.is_empty()).map(|l| vec![vec![l.license.clone()]]).unwrap_or_default(),
                seq: 0,
            });
            entry = Entry { modified: last.as_ref().map(|l| l.modified), chain, origin_item: p.origin_item_id.clone(), origin_qid: p.origin_qid.clone() };
        } else {
            entry = Entry::default();
        }
        // Units used besides the question itself: see `Units`.
        let mut hidden_units: Vec<LicenceUnit> = vec![];
        let lifts: &[Lift] = u.lifts.get(&i).map(Vec::as_slice).unwrap_or(&[]);
        for l in lifts {
            if sources.iter().any(|s| s.0.item_id == l.item_id && hard_of(s)) {
                continue;
            }
            let upstream = licences_of(&serde_json::from_str::<Vec<ChainLink>>(&l.chain).unwrap_or_default());
            // Bound by the licences the account RECEIVED, never the source's current one.
            let unit = LicenceUnit { item: l.item_id.clone(), verbatim: false, contents: vec![format!("unit:{}", l.key)], chain: upstream, own: vec![if l.received.is_empty() { vec![l.license.clone()] } else { l.received.clone() }], seq: l.seq };
            if l.hidden {
                // A hidden-only text may bind its author privately (a refusal only she sees, from material she was served), never anything others observe.
                if l.served {
                    hidden_units.push(unit);
                }
                continue;
            }
            let known = entry.chain.iter().any(|x| x.item_id == l.item_id);
            let link = ChainLink { author_name: l.author_name.clone(), item_id: l.item_id.clone(), qid: l.qid.clone(), version: l.version, license: l.license.clone(), modified: true, kind: Some(kind_tag(l.kind).into()), ..Default::default() };
            if !l.served {
                // Never served: an earlier public text, image or question unit is credited as an earlier publication (unverified, flagged, no refusal).
                if !l.visible {
                    continue;
                }
                if !known {
                    entry.chain.push(ChainLink { earlier_match: Some(true), ..link });
                }
                out.marks[i].earlier = true;
                push_unique(&mut out.soft_only, i);
                continue;
            }
            let claimed = m.independent.contains(&l.item_id);
            // A stem-only question unit READ is soft, like a changed copy of a read question: credited, flagged, never binding.
            if l.kind == UnitKind::Part && l.stem_only && l.ev_kind == Some("view") {
                if !known {
                    entry.chain.push(if claimed { ChainLink { possible_source: Some(true), ..link } } else { link });
                }
                if claimed {
                    out.marks[i].claimed = true;
                } else {
                    out.marks[i].soft = true;
                    push_unique(&mut out.soft_only, i);
                }
                continue;
            }
            // Claims: never over an IMPORTED text or image; a read one only when attributed to a public source, or an image the claimant uploaded before that item was
            // first published. Question units follow the question rule.
            let may_claim = claimed
                && if l.kind == UnitKind::Part { l.stem_only || (l.ev_kind == Some("view") && f.short40[i]) } else { l.ev_kind == Some("view") && (attributed || (l.kind == UnitKind::Image && l.uploaded_first)) };
            if may_claim {
                if !known {
                    entry.chain.push(ChainLink { possible_source: Some(true), ..link });
                }
                out.marks[i].claimed = true;
                continue;
            }
            // An adaptation of the source; the same material from several sources is one material, so their grants are united.
            units.push(unit);
            if !known {
                entry.chain.push(link);
            }
        }
        if !hidden_units.is_empty() && !decide_licence(&hidden_units).allowed.contains(&m.license.as_str()) {
            out.hidden_refused.push(i);
            for hu in &hidden_units {
                let licences: Vec<String> = hu.own.iter().flatten().cloned().collect();
                match out.hidden_from.iter_mut().find(|(k, _)| k == &hu.item) {
                    Some((_, v)) => licences.into_iter().for_each(|l| push_unique(v, l)),
                    None => {
                        let mut v = vec![];
                        licences.into_iter().for_each(|l| push_unique(&mut v, l));
                        out.hidden_from.push((hu.item.clone(), v));
                    }
                }
            }
        }
        let dec = decide_licence(&units);
        let allows = dec.allowed.contains(&m.license.as_str());
        // Each link to material this publish drew on records the licence that ALLOWED the use: the most permissive licence available to this account for that
        // material that permits this publish. Downstream copies are bound by exactly that (a CC grant is irrevocable), never by an older or stricter label.
        if allows {
            for link in entry.chain.iter_mut() {
                if link.item_id.is_empty() || link.earlier_match == Some(true) || link.possible_source == Some(true) {
                    continue;
                }
                let hs = sources.iter().find(|s| s.0.item_id == link.item_id && hard_of(s));
                let lf = if hs.is_some() { None } else { lifts.iter().find(|x| x.item_id == link.item_id && x.served) };
                let (groups, modified) = match (hs, lf) {
                    (Some(s), _) => {
                        let un = unit_of(s.0);
                        (Some(un.own.clone()), !un.verbatim)
                    }
                    (None, Some(lf)) => (Some(vec![if lf.received.is_empty() { vec![lf.license.clone()] } else { lf.received.clone() }]), true),
                    _ => (None, true),
                };
                if let Some(r) = groups.and_then(|g| relied_licence(&g, &m.license, modified)) {
                    link.license = r.to_string();
                }
            }
        }
        out.per_question.push(dec.allowed.clone());
        out.binders.push(dec.binders.clone());
        // When a binder's own licence allows this one, what binds is a link of its chain (an upstream licence it relabelled): name that link, not the relabel.
        out.true_bound.push(if allows {
            vec![]
        } else {
            dec.binders
                .iter()
                .flat_map(|b| {
                    let Some(hs) = sources.iter().find(|s| &s.0.item_id == b) else { return vec![] };
                    if !may_republish(&hs.0.license, &m.license) {
                        return vec![];
                    }
                    entry.chain.iter().filter(|l| !l.item_id.is_empty() && &l.item_id != b && !l.license.is_empty() && !may_republish(&l.license, &m.license)).map(|l| (b.clone(), l.clone())).collect()
                })
                .collect()
        });
        // A republish never refuses a question its author did not touch: same qid, same content (key-free whenever either version hides its key) and the same
        // licence keep the outcome their earlier publish had; the admin is told.
        let untouched = pv.is_some_and(|p| prev_license == Some(m.license.as_str()) && if m.hide_key || prev_hidden { p.keyfree_content == h.keyfree[i] } else { p.full_content == h.full_content[i] });
        out.allowed_json.push(match pv.and_then(|p| p.allowed_json.clone()).filter(|_| untouched) {
            Some(a) => a,
            None => json!({ "allowed": dec.allowed, "binders": dec.binders }).to_string(),
        });
        if !cut_here.is_empty() && !untouched {
            let forbids_lic = |l: &str| !grant_options(&Grant { item: String::new(), license: l.to_string(), modified: true }).contains(&m.license.as_str());
            if let Some(forbids) = cut_here.iter().find(|cx| cx.lics.iter().any(|l| forbids_lic(l))) {
                let lic0 = forbids.lics.iter().find(|l| forbids_lic(l)).cloned().unwrap_or_default();
                let item0 = forbids.by.get(&lic0).cloned().unwrap_or_else(|| forbids.item.clone());
                out.cut_refused.insert(i, item0.clone());
                push_unique(&mut out.binders[i], item0);
            } else {
                let n0: i64 = cut_here.iter().map(|cx| cx.n).sum();
                let mut lics: Vec<String> = vec![];
                cut_here.iter().flat_map(|cx| cx.lics.iter().cloned()).for_each(|l| push_unique(&mut lics, l));
                entry.chain.push(placeholder(format!("and {n0} earlier similar sources"), lics));
                out.fuzzy_pos.insert(i);
            }
        }
        if sources.iter().any(|s| s.0.claimed) {
            out.marks[i].claimed = true;
        }
        if !allows || out.cut_refused.contains_key(&i) {
            if untouched { out.grandfathered.push(i) } else { out.incompatible.push(i) }
        } else {
            if dec.conflict {
                out.conflicts.push(i);
            }
            // The soft flag: only an adaptation (no source is the same question) with no declared origin, of something read whose terms do not allow the licence.
            if d.is_none() && !sources.iter().any(|s| s.0.near) && soft_units.iter().any(|su| !unit_allows(su).0.contains(m.license.as_str())) {
                out.soft_only.push(i);
                out.marks[i].soft = true;
            }
        }
        out.entries.push(entry);
    }
    out
}

#[cfg(test)]
#[path = "publishderive_tests.rs"]
mod tests;
