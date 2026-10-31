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

fn is_letter_or_number(c: char) -> bool {
    matches!(
        get_general_category(c),
        G::UppercaseLetter | G::LowercaseLetter | G::TitlecaseLetter | G::ModifierLetter | G::OtherLetter | G::DecimalNumber | G::LetterNumber | G::OtherNumber
    )
}
fn is_letter(c: char) -> bool {
    matches!(get_general_category(c), G::UppercaseLetter | G::LowercaseLetter | G::TitlecaseLetter | G::ModifierLetter | G::OtherLetter)
}

/// The search key of one text: folded, punctuation and hyphens as spaces.
pub fn search_key(s: &str) -> String {
    let mut out = String::new();
    let mut gap = false;
    for c in fold(s).chars() {
        let rep: Option<&str> = match c {
            'œ' => Some("oe"),
            'æ' => Some("ae"),
            'ø' => Some("o"),
            'ł' => Some("l"),
            'đ' | 'ð' => Some("d"),
            'þ' => Some("th"),
            'ı' => Some("i"),
            'ĸ' => Some("k"),
            'ŋ' => Some("n"),
            _ => None,
        };
        if rep.is_some() || is_letter_or_number(c) {
            if gap {
                out.push(' ');
                gap = false;
            }
            match rep {
                Some(r) => out.push_str(r),
                None => out.push(c),
            }
        } else {
            gap = true;
        }
    }
    // (a leading gap is dropped: `trim` of the Worker's result)
    out.trim().to_string()
}

/// The stored title search column: the search key, the German spelling of umlauts (ü = ue), and hyphenated words joined
/// (co-operative = cooperative), each word-searchable (it starts with a space, and `q` matches the start of any word).
pub fn title_search(title: &str) -> String {
    let german: String = title
        .chars()
        .map(|c| match c {
            'ä' => "ae".to_string(),
            'ö' => "oe".to_string(),
            'ü' => "ue".to_string(),
            'Ä' => "Ae".to_string(),
            'Ö' => "Oe".to_string(),
            'Ü' => "Ue".to_string(),
            c => c.to_string(),
        })
        .collect();
    // `(\p{L})[-‐‑'’](?=\p{L})` -> `$1`: the joiner between two letters goes.
    let cs: Vec<char> = title.chars().collect();
    let joined: String = cs
        .iter()
        .enumerate()
        .filter(|(i, c)| !(matches!(c, '-' | '\u{2010}' | '\u{2011}' | '\'' | '\u{2019}') && *i > 0 && is_letter(cs[*i - 1]) && cs.get(i + 1).is_some_and(|n| is_letter(*n))))
        .map(|(_, c)| *c)
        .collect();
    let mut v: Vec<String> = Vec::new();
    for k in [search_key(title), search_key(&german), search_key(&joined)] {
        if !k.is_empty() && !v.contains(&k) {
            v.push(k);
        }
    }
    format!(" {}", v.join(" "))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::Value;

    #[test]
    fn search_key_and_title_search_match_the_worker() {
        let v: Value = serde_json::from_str(include_str!("searchkey_vectors.json")).unwrap();
        for (i, t) in v["texts"].as_array().unwrap().iter().enumerate() {
            let t = t.as_str().unwrap();
            assert_eq!(search_key(t), v["keys"][i].as_str().unwrap(), "key of {t:?}");
            assert_eq!(title_search(t), v["titles"][i].as_str().unwrap(), "title of {t:?}");
        }
    }

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
