//! Two small pure helpers of worker/src/pool/publish.ts that the read side needs: `collapseChain` and `fold`. Both are
//! compared with values the Worker's own functions produced (textutil_vectors.json).
use super::chain::ChainLink;
use super::evidence::CREDITS_HEAD;
use unicode_general_category::{GeneralCategory as G, get_general_category};
use unicode_normalization::UnicodeNormalization;

/// Links shown per question before "and N contributors between".
pub const CHAIN_MAX: usize = 32;

/// A chain longer than `max` keeps its FIRST links (the originators are always credited), a placeholder carrying the
/// licences of the links it stands for (so their grants still apply), and the most recent links.
pub fn collapse_chain(chain: &[ChainLink], max: usize) -> Vec<ChainLink> {
    // The first CREDITS_HEAD (the originators), one placeholder, the most recent max - CREDITS_HEAD.
    let keep_head = (CREDITS_HEAD as usize).min((max >> 2).max(1));
    let keep_tail = max - keep_head;
    if chain.len() <= keep_head + keep_tail + 1 {
        return chain.to_vec();
    }
    let head = &chain[..keep_head];
    let tail = &chain[chain.len() - keep_tail..];
    let middle = &chain[keep_head..chain.len() - keep_tail];
    let mut lic: Vec<String> = Vec::new();
    for l in middle {
        for x in std::iter::once(&l.license).chain(l.collapsed_licenses.iter().flatten()) {
            if !x.is_empty() && !lic.contains(x) {
                lic.push(x.clone());
            }
        }
    }
    lic.sort();
    // The links left out are BETWEEN the first and the most recent (a placeholder's own count included).
    let count: u64 = middle.iter().map(|l| if l.item_id.is_empty() { placeholder_count(&l.author_name).unwrap_or(1) } else { 1 }).sum();
    let mut out = head.to_vec();
    out.push(ChainLink { author_name: format!("and {count} contributors between"), item_id: String::new(), qid: String::new(), version: 0, license: String::new(), modified: true, collapsed_licenses: Some(lic), ..Default::default() });
    out.extend_from_slice(tail);
    out
}

/// `^and (\d+) contributors between$`
fn placeholder_count(name: &str) -> Option<u64> {
    let n = name.strip_prefix("and ")?.strip_suffix(" contributors between")?;
    if n.is_empty() || !n.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    n.parse().ok()
}

/// The case fold used for browse filters (stored per item; any script, not only ASCII): compatibility forms, accents
/// optional (marks removed), lower case, and the folds lower-casing misses (ß -> ss, final sigma -> sigma).
pub fn fold(s: &str) -> String {
    let stripped: String = s.nfkd().filter(|c| !matches!(get_general_category(*c), G::NonspacingMark | G::SpacingMark | G::EnclosingMark)).collect();
    stripped.to_lowercase().replace('ß', "ss").replace('ς', "σ").nfc().collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::Value;

    fn vectors() -> Value {
        serde_json::from_str(include_str!("textutil_vectors.json")).unwrap()
    }

    #[test]
    fn fold_matches_the_worker() {
        let v = vectors();
        for (i, t) in v["folds"].as_array().unwrap().iter().enumerate() {
            assert_eq!(fold(t.as_str().unwrap()), v["foldOut"][i].as_str().unwrap(), "{t}");
        }
    }

    #[test]
    fn collapse_chain_matches_the_worker_for_short_long_and_placeholder_chains() {
        let v = vectors();
        for (name, chain) in v["chains"].as_object().unwrap() {
            let links: Vec<ChainLink> = serde_json::from_value(chain.clone()).unwrap();
            let want: Vec<ChainLink> = serde_json::from_value(v["collapsed"][name].clone()).unwrap();
            assert_eq!(collapse_chain(&links, CHAIN_MAX), want, "{name}");
        }
        let links: Vec<ChainLink> = serde_json::from_value(v["chains"]["n100"].clone()).unwrap();
        let want: Vec<ChainLink> = serde_json::from_value(v["small"].clone()).unwrap();
        assert_eq!(collapse_chain(&links, 8), want, "max 8");
    }

    #[test]
    fn a_chain_at_the_limit_is_untouched_and_the_placeholder_counts_what_it_stands_for() {
        let l = |i: usize| ChainLink { item_id: format!("i{i}"), license: "CC0-1.0".into(), ..Default::default() };
        let exactly: Vec<ChainLink> = (0..CHAIN_MAX + 1).map(l).collect();
        assert_eq!(collapse_chain(&exactly, CHAIN_MAX), exactly);
        let over: Vec<ChainLink> = (0..CHAIN_MAX + 2).map(l).collect();
        let c = collapse_chain(&over, CHAIN_MAX);
        assert_eq!(c.len(), CHAIN_MAX + 1);
        assert_eq!(c[8].author_name, "and 2 contributors between");
        assert_eq!(placeholder_count("and 12 contributors between"), Some(12));
        assert_eq!(placeholder_count("and x contributors between"), None);
    }
}
