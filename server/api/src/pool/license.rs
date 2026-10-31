//! Licences and the inheritance table (docs/shared-pool.md "Sharing model", "Licenses inherit, per question"). A CC
//! grant is per version; a copy keeps the licence of the publish it came from. Port of worker/src/pool/license.ts.
use std::collections::{BTreeSet, HashMap};

pub const LICENSES: [&str; 6] = ["CC-BY-4.0", "CC0-1.0", "CC-BY-SA-4.0", "CC-BY-NC-4.0", "CC-BY-NC-SA-4.0", "circle-only"];

pub fn is_license(x: &str) -> bool {
    LICENSES.contains(&x)
}

/// Origin licence -> licences a derived publish may carry. "Stricter" never means a different condition set: BY-SA
/// cannot become BY-NC-SA. circle-only material cannot be re-shared by an importer at all (its author can still
/// republish their own).
fn may(origin: &str) -> &'static [&'static str] {
    match origin {
        "CC0-1.0" => &LICENSES,
        "CC-BY-4.0" => &["CC-BY-4.0", "CC-BY-SA-4.0", "CC-BY-NC-4.0", "CC-BY-NC-SA-4.0"],
        "CC-BY-SA-4.0" => &["CC-BY-SA-4.0"],
        "CC-BY-NC-4.0" => &["CC-BY-NC-4.0", "CC-BY-NC-SA-4.0"],
        "CC-BY-NC-SA-4.0" => &["CC-BY-NC-SA-4.0"],
        _ => &[],
    }
}

/// May material under `origin` be published by someone else under `target`?
pub fn may_republish(origin: &str, target: &str) -> bool {
    is_license(origin) && is_license(target) && may(origin).contains(&target)
}

/// The licences an importer may publish a copy under (for the import response).
pub fn republish_options(origin: &str) -> &'static [&'static str] {
    if is_license(origin) { may(origin) } else { &[] }
}

/// Licences allowed by every one of `origins` (empty when none is), in table order.
pub fn allowed_by_all<'a>(origins: impl IntoIterator<Item = &'a str>) -> Vec<&'static str> {
    let mut out: Vec<&'static str> = LICENSES.to_vec();
    for o in origins {
        out.retain(|t| may_republish(o, t));
    }
    out
}

/// Revisions of the rights-attestation wording the server accepts. The app shows the newest; an older one stays accepted
/// until the app versions that show it are gone.
pub const ATTEST_TEXT_REVS: [&str; 1] = ["2026-10-19"];
/// The publishing terms the teacher affirms on every publish and republish (`attest: { own_work: true, terms_version }`).
/// An affirmation by the teacher, recorded, never verified by the server; a preview does not need it.
pub const POOL_TERMS_VERSION: &str = "2026-10-22";
pub const ATTEST_STATEMENT: &str = "I confirm this content is my own original work, or I have the right to share it under the licence I chose. It is not copied from a published source (textbooks, publisher test banks, exam boards, paid worksheets) without permission.";

/// One grant a question carries: the licence of an upstream publish of item `item`, and whether the copy changed it. A
/// verbatim copy keeps its licence (CC 4.0 lets you add terms only to your own adaptation), except that CC0 material may
/// be relabelled freely; an adaptation follows the table.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Grant {
    pub item: String,
    pub license: String,
    pub modified: bool,
}

pub fn grant_options(g: &Grant) -> &'static [&'static str] {
    if !is_license(&g.license) || g.license == "circle-only" {
        return &[];
    }
    if g.modified || g.license == "CC0-1.0" {
        return may(&g.license);
    }
    // A verbatim copy keeps exactly its own licence.
    LICENSES.iter().find(|l| **l == g.license).map(std::slice::from_ref).unwrap_or(&[])
}

type Set = BTreeSet<&'static str>;

fn all() -> Set {
    LICENSES.iter().copied().collect()
}
fn cut_by(set: &Set, o: &[&str]) -> Set {
    set.iter().copied().filter(|l| o.contains(l)).collect()
}
fn in_table_order(s: &Set) -> Vec<&'static str> {
    LICENSES.iter().copied().filter(|l| s.contains(l)).collect()
}

/// THE licence rule, used by both the import response (`may_republish_as`) and the publish check, so the two can never
/// disagree. Within one item: union of the verbatim grants if there are any, otherwise intersection; across different
/// items: intersection. No grants: any licence.
pub fn allowed_for(grants: &[Grant]) -> Vec<&'static str> {
    let mut by_item: Vec<(&str, Vec<&Grant>)> = Vec::new();
    for g in grants {
        match by_item.iter_mut().find(|(i, _)| *i == g.item) {
            Some((_, v)) => v.push(g),
            None => by_item.push((&g.item, vec![g])),
        }
    }
    let mut out = all();
    for (_, gs) in &by_item {
        // Within one item: a verbatim copy of some version may use any of the grants of the versions it is identical to
        // (union of the verbatim grants only). An adaptation must satisfy every version it drew on (intersection):
        // adding an NC-only explanation to a v2 never makes it CC-BY just because v1 was.
        let verbatim: Vec<&&Grant> = gs.iter().filter(|g| !g.modified).collect();
        let ok: Set = if !verbatim.is_empty() {
            verbatim.iter().flat_map(|g| grant_options(g).iter().copied()).collect()
        } else {
            let mut ok = all();
            for g in gs {
                ok = cut_by(&ok, grant_options(g));
            }
            ok
        };
        out = out.intersection(&ok).copied().collect();
    }
    in_table_order(&out)
}

/// A bank: every question's grants must allow the licence.
pub fn allowed_for_all(per_question: &[Vec<Grant>]) -> Vec<&'static str> {
    let mut out = all();
    for g in per_question {
        let a: Set = allowed_for(g).into_iter().collect();
        out = out.intersection(&a).copied().collect();
    }
    in_table_order(&out)
}

/// A stored chain link, as far as the licence rule reads it.
pub struct ChainLinkLicense<'a> {
    pub item_id: &'a str,
    pub license: &'a str,
    pub collapsed_licenses: &'a [String],
}

/// Grants carried by a stored chain (oldest first): every earlier link counts as adapted.
pub fn chain_grants(chain: &[ChainLinkLicense]) -> Vec<Grant> {
    let mut out = Vec::new();
    for l in chain {
        if !l.item_id.is_empty() && !l.license.is_empty() {
            out.push(Grant { item: l.item_id.to_string(), license: l.license.to_string(), modified: true });
        }
        // A collapsed run of links (a chain longer than CHAIN_MAX) still grants what its links did.
        for license in l.collapsed_licenses {
            out.push(Grant { item: format!("collapsed:{license}"), license: license.clone(), modified: true });
        }
    }
    out
}

/// One source as the licence rule sees it: its chain's licences (each an adaptation grant), its own licences grouped by
/// served content (`own`: every licence shown for that content; identical content across versions of one item is ONE
/// material, so its licences are united), whether the subject is a copy of it (licence sense), the contents it served
/// (sources that served the same content are one material) and when its content was first published (`seq`: the
/// earliest of its serves).
#[derive(Clone, Debug)]
pub struct LicenceUnit {
    pub item: String,
    pub verbatim: bool,
    pub chain: Vec<String>,
    pub own: Vec<Vec<String>>,
    pub contents: Vec<String>,
    pub seq: i64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LicenceDecision {
    /// The licences the question may be published under.
    pub allowed: Vec<&'static str>,
    /// The sources contradict each other: decided by publication order (flag license_conflict).
    pub conflict: bool,
    /// The items that decide it (named in a refusal's bound_by), earliest first.
    pub binders: Vec<String>,
}

fn opts(license: &str, modified: bool) -> &'static [&'static str] {
    grant_options(&Grant { item: String::new(), license: license.to_string(), modified })
}

/// What one source allows by itself: its own terms (a copy: any of the licences shown for that content; an adaptation:
/// for each content it served, any of them, and every content's), narrowed by its chain (oldest material first). If its
/// own grants contradict each other (a relabel against its chain), the older terms win: chain first, then its own,
/// skipping what would leave nothing (never a licence no single grant allows). Empty only for circle-only material.
pub fn unit_allows(u: &LicenceUnit) -> (Set, bool) {
    let own: Set = if u.verbatim {
        u.own.iter().flatten().flat_map(|l| opts(l, false).iter().copied()).collect()
    } else {
        u.own.iter().fold(all(), |acc, group| {
            let g: Set = group.iter().flat_map(|l| opts(l, true).iter().copied()).collect();
            cut_by(&acc, &g.into_iter().collect::<Vec<_>>())
        })
    };
    let mut full = own.clone();
    for l in &u.chain {
        full = cut_by(&full, opts(l, true));
    }
    if !full.is_empty() {
        return (full, false);
    }
    let mut out = all();
    let mut any = false;
    let mut sorted_chain = u.chain.clone();
    sorted_chain.sort();
    let mut steps: Vec<Vec<&str>> = sorted_chain.iter().map(|l| opts(l, true).to_vec()).collect();
    steps.push(own.iter().copied().collect());
    for o in steps {
        let n = cut_by(&out, &o);
        if !n.is_empty() {
            out = n;
            any = true;
        }
    }
    (if any { out } else { Set::new() }, true)
}

fn find(parent: &mut Vec<usize>, i: usize) -> usize {
    if parent[i] == i {
        return i;
    }
    let r = find(parent, parent[i]);
    parent[i] = r;
    r
}

/// THE licence rule, for the publish check and the import response alike, over the HARD sources of one question:
/// 1. The subject is a copy of some sources (verbatim, licence sense): the EARLIEST-published of them is the original
///    and its terms decide (a later identical relabel can neither widen nor narrow them: a CC0 original next to an NC-SA
///    relabel still allows CC0). Adaptations beside a copy do not narrow it.
/// 2. Otherwise (an adaptation): sources that served the same content are one material (its earliest source decides it);
///    the materials are taken in publication order, earliest first, and each narrows what the earlier ones allow; one
///    that would leave nothing is skipped (`conflict`, flagged). So more sources never widen the set, the outcome never
///    depends on import order, and a contradiction never locks anyone out: the earliest material's terms bound it.
/// 3. Nothing at all only for circle-only material (allowed []).
/// No sources: any licence.
pub fn decide_licence(units: &[LicenceUnit]) -> LicenceDecision {
    if units.is_empty() {
        return LicenceDecision { allowed: LICENSES.to_vec(), conflict: false, binders: vec![] };
    }
    let by_age = |a: &&LicenceUnit, b: &&LicenceUnit| a.seq.cmp(&b.seq).then_with(|| a.item.cmp(&b.item));
    let mut verbatim: Vec<&LicenceUnit> = units.iter().filter(|u| u.verbatim).collect();
    verbatim.sort_by(by_age);
    if !verbatim.is_empty() {
        for u in &verbatim {
            let (a, conflict) = unit_allows(u);
            if !a.is_empty() {
                return LicenceDecision { allowed: in_table_order(&a), conflict, binders: vec![u.item.clone()] };
            }
        }
        return LicenceDecision { allowed: vec![], conflict: false, binders: vec![verbatim[0].item.clone()] };
    }
    // Materials: units connected by a shared served content (union-find over content hashes).
    let mut sorted: Vec<&LicenceUnit> = units.iter().collect();
    sorted.sort_by(by_age);
    let mut parent: Vec<usize> = (0..sorted.len()).collect();
    let mut by_content: HashMap<&str, usize> = HashMap::new();
    for (i, u) in sorted.iter().enumerate() {
        for h in &u.contents {
            match by_content.get(h.as_str()) {
                None => {
                    by_content.insert(h, i);
                }
                Some(&j) => {
                    let (ri, rj) = (find(&mut parent, i), find(&mut parent, j));
                    parent[ri] = rj;
                }
            }
        }
    }
    // Classes in the order of their first member, as a JS Map preserves insertion order.
    let mut class_order: Vec<usize> = Vec::new();
    let mut classes: HashMap<usize, Vec<usize>> = HashMap::new();
    for i in 0..sorted.len() {
        let r = find(&mut parent, i);
        if !classes.contains_key(&r) {
            class_order.push(r);
        }
        classes.entry(r).or_default().push(i);
    }
    struct Mat {
        seq: i64,
        items: Vec<String>,
        allowed: Option<Set>,
        inner: bool,
    }
    // Each material: its earliest source that allows anything decides it; materials earliest first.
    let mut mats: Vec<Mat> = class_order
        .iter()
        .map(|r| {
            let idx = &classes[r];
            let decided = idx.iter().map(|&i| unit_allows(sorted[i])).find(|(a, _)| !a.is_empty());
            Mat { seq: sorted[idx[0]].seq, items: idx.iter().map(|&i| sorted[i].item.clone()).collect(), inner: decided.as_ref().is_some_and(|d| d.1), allowed: decided.map(|d| d.0) }
        })
        .collect();
    mats.sort_by_key(|m| m.seq);
    let mut out = all();
    let (mut conflict, mut decided) = (false, false);
    for m in &mats {
        let Some(allowed) = &m.allowed else { continue };
        conflict |= m.inner;
        let n = cut_by(&out, &allowed.iter().copied().collect::<Vec<_>>());
        if !n.is_empty() {
            out = n;
            decided = true;
        } else {
            conflict = true;
        }
    }
    let mut binders: Vec<String> = Vec::new();
    for m in &mats {
        for it in &m.items {
            if !binders.contains(it) {
                binders.push(it.clone());
            }
        }
    }
    LicenceDecision { allowed: if decided { in_table_order(&out) } else { vec![] }, conflict, binders }
}

/// A bank: the licences every question allows.
pub fn allowed_for_bank(per_question: &[Vec<&str>]) -> Vec<&'static str> {
    LICENSES.iter().copied().filter(|l| per_question.iter().all(|a| a.contains(l))).collect()
}

/// The licence a stored link records, given the licences available to the publisher for the material (`groups`: per
/// served content, every licence it received for it or the source's licence now for the same content). Per content: the
/// most permissive one that permits `target`; across contents (an adaptation of several): the most restrictive of those.
/// None when some content has none (the label is then left as it was).
pub fn relied_licence(groups: &[Vec<String>], target: &str, modified: bool) -> Option<&'static str> {
    let reach = |l: &str| grant_options(&Grant { item: String::new(), license: l.to_string(), modified: true }).len();
    let mut out: Option<&'static str> = None;
    for g in groups {
        let mut seen: Vec<&'static str> = Vec::new();
        for l in g {
            if let Some(lic) = LICENSES.iter().find(|x| **x == l.as_str()) {
                if !seen.contains(lic) && opts(lic, modified).contains(&target) {
                    seen.push(lic);
                }
            }
        }
        if seen.is_empty() {
            return None;
        }
        // reduce keeping the first of equals, as the Worker's strict `>` did
        let best = seen.iter().copied().fold(seen[0], |a, b| if reach(b) > reach(a) { b } else { a });
        if out.is_none_or(|o| reach(best) < reach(o)) {
            out = Some(best);
        }
    }
    out
}

#[cfg(test)]
#[path = "license_tests.rs"]
mod tests;
