//! The near-rewording check of confirmed student-data text (worker/src/pool/privacynear.ts), shared by a public publish and a
//! circle send so both run the same code.
use super::fingerprint::Fingerprinter;
use super::fuzzy::FUZZY_JACCARD;
use super::qhash::jaccard;
use rusqlite::Connection;
use std::collections::{HashMap, HashSet};

/// One stem as the check sees it: its words and its MinHash bands (unkeyed; keyed here).
#[derive(Debug, Clone, Default)]
pub struct StemSketch {
    pub tokens: Vec<String>,
    pub bands: Vec<String>,
}

/// The positions whose stem (or one of its Multipart parts' stems) comes near a confirmed student-data block: a keyed band in
/// common with a privacy row and a token Jaccard >= FUZZY_JACCARD (a band row with no token sketch matches on the band). A
/// position one of whose exact or normalised keys (`keys_at`) is itself blocked is not "near" (it is held by the exact check).
/// Past `max` band rows every question sharing a band counts (fail closed). `exact_of` runs only when some band row matched.
pub fn privacy_near_positions(
    conn: &Connection,
    fp: &Fingerprinter,
    stems: &[Vec<StemSketch>],
    keys_at: &[Vec<Option<String>>],
    exact_of: impl FnOnce() -> rusqlite::Result<HashSet<String>>,
    sk_memo: &mut HashMap<String, String>,
    max: usize,
) -> rusqlite::Result<Vec<usize>> {
    let mut out = Vec::new();
    // Privacy rows keep keyed bands and keyed token sets only.
    let mut raw_bands: Vec<String> = Vec::new();
    for b in stems.iter().flatten().flat_map(|s| s.bands.iter()) {
        if !raw_bands.contains(b) {
            raw_bands.push(b.clone());
        }
    }
    let k_of: HashMap<String, String> = raw_bands.iter().map(|b| (b.clone(), fp.keyed_one("b:", b, sk_memo))).collect();
    let all_bands: Vec<String> = raw_bands.iter().map(|b| k_of[b].clone()).collect();
    let k_toks: Vec<Vec<Vec<String>>> = stems.iter().map(|ss| ss.iter().map(|s| fp.keyed_set(&s.tokens, sk_memo, "t:")).collect()).collect();
    if all_bands.is_empty() {
        return Ok(out);
    }
    let rows: Vec<(String, Option<String>)> = conn
        .prepare(&format!("SELECT DISTINCT band, qhash, tokens FROM privacy_bands WHERE band IN (SELECT value FROM json_each(?1)) LIMIT {}", max + 1))?
        .query_map([serde_json::to_string(&all_bands).unwrap()], |r| Ok((r.get(0)?, r.get(2)?)))?
        .collect::<Result<_, _>>()?;
    if rows.is_empty() {
        return Ok(out);
    }
    let exact = exact_of()?;
    let cut = rows.len() > max;
    for (i, ss) in stems.iter().enumerate() {
        if keys_at.get(i).is_some_and(|ks| ks.iter().flatten().any(|k| exact.contains(k))) {
            continue;
        }
        let near = ss.iter().enumerate().any(|(j, s)| {
            let set: HashSet<&String> = s.bands.iter().map(|b| &k_of[b]).collect();
            // The bands keep a keyed sketch, never words; a band with none matches on the band.
            rows.iter().any(|(band, tokens)| {
                set.contains(band)
                    && (cut
                        || tokens.is_none()
                        || serde_json::from_str::<Vec<String>>(tokens.as_ref().unwrap()).is_ok_and(|t| jaccard(&k_toks[i][j], &t) >= FUZZY_JACCARD))
            })
        });
        if near {
            out.push(i);
        }
    }
    Ok(out)
}

#[cfg(test)]
#[path = "privacynear_tests.rs"]
mod tests;
