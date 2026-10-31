//! Tests for `privacynear.rs` on the real schema: a keyed band in common plus a token Jaccard, the band-only match, the exact-key
//! exemption, and the fail-closed cut.
use super::*;
use crate::db;
use rusqlite::params;
use std::path::Path;

const SECRET: &str = "test-secret-0123456789abcdef0123456789abcdef";

fn open() -> (Connection, tempfile::TempDir) {
    let dir = tempfile::tempdir().unwrap();
    let mut conn = db::open(&dir.path().join("t.db")).unwrap();
    db::migrate(&mut conn, &Path::new(env!("CARGO_MANIFEST_DIR")).join("../worker/migrations")).unwrap();
    (conn, dir)
}
fn fp() -> Fingerprinter {
    Fingerprinter::from_secret(SECRET).unwrap()
}
fn words(n: usize, tag: &str) -> Vec<String> {
    (0..n).map(|i| format!("{tag}{i}")).collect()
}
fn sketch(tokens: Vec<String>, bands: &[&str]) -> StemSketch {
    StemSketch { tokens, bands: bands.iter().map(|b| b.to_string()).collect() }
}
/// A privacy row: keyed band, and the keyed token set (or none).
fn row(c: &Connection, f: &Fingerprinter, band: &str, tokens: Option<&[String]>, qhash: &str) {
    let mut m = HashMap::new();
    let kb = f.keyed_one("b:", band, &mut m);
    let kt = tokens.map(|t| serde_json::to_string(&f.keyed_set(t, &mut m, "t:")).unwrap());
    c.execute("INSERT INTO privacy_bands (band, qhash, tokens, item_id, at) VALUES (?1, ?2, ?3, 'item', 1)", params![kb, qhash, kt]).unwrap();
}
fn run(c: &Connection, stems: &[Vec<StemSketch>], keys: &[Vec<Option<String>>], exact: &[&str], max: usize) -> (Vec<usize>, bool) {
    let mut asked = false;
    let ex: HashSet<String> = exact.iter().map(|s| s.to_string()).collect();
    let out = privacy_near_positions(c, &fp(), stems, keys, || { asked = true; Ok(ex) }, &mut HashMap::new(), max).unwrap();
    (out, asked)
}

#[test]
fn nothing_is_near_when_no_band_matches_and_the_exact_check_is_not_even_asked() {
    let (c, _d) = open();
    row(&c, &fp(), "0:a:b", None, "q1");
    let stems = vec![vec![sketch(words(10, "w"), &["9:z:z"])]];
    assert_eq!(run(&c, &stems, &[vec![]], &[], 100), (vec![], false));
    assert_eq!(run(&c, &[], &[], &[], 100), (vec![], false), "no stems, no bands");
}

#[test]
fn a_shared_band_with_a_close_token_set_is_near_and_a_distant_one_is_not() {
    let (c, _d) = open();
    let base = words(10, "w");
    row(&c, &fp(), "0:a:b", Some(&base), "q1");
    let mut close = base.clone();
    close[0] = "other".into(); // 9 of 11 shared words = 0.818
    let far = words(10, "x");
    let stems = vec![vec![sketch(close, &["0:a:b"])], vec![sketch(far, &["0:a:b"])], vec![sketch(base.clone(), &["0:a:b"])]];
    let (out, asked) = run(&c, &stems, &[vec![], vec![], vec![]], &[], 100);
    assert_eq!(out, vec![0, 2]);
    assert!(asked);
}

#[test]
fn a_row_with_no_token_sketch_matches_on_the_band_alone() {
    let (c, _d) = open();
    row(&c, &fp(), "0:a:b", None, "q1");
    let stems = vec![vec![sketch(words(5, "unrelated"), &["0:a:b"])]];
    assert_eq!(run(&c, &stems, &[vec![]], &[], 100).0, vec![0]);
}

#[test]
fn a_multipart_stem_is_near_when_any_of_its_parts_is() {
    let (c, _d) = open();
    let base = words(8, "p");
    row(&c, &fp(), "1:q:r", Some(&base), "q1");
    let stems = vec![vec![sketch(words(8, "n"), &["5:n:n"]), sketch(base, &["1:q:r"])]];
    assert_eq!(run(&c, &stems, &[vec![]], &[], 100).0, vec![0]);
}

#[test]
fn a_position_whose_exact_key_is_blocked_is_held_by_the_exact_check_not_near() {
    let (c, _d) = open();
    let base = words(8, "p");
    row(&c, &fp(), "1:q:r", Some(&base), "q1");
    let stems = vec![vec![sketch(base.clone(), &["1:q:r"])], vec![sketch(base, &["1:q:r"])]];
    let keys = vec![vec![Some("k1".to_string()), None], vec![Some("k2".to_string())]];
    assert_eq!(run(&c, &stems, &keys, &["k1"], 100).0, vec![1]);
}

#[test]
fn past_the_row_limit_every_question_sharing_a_band_counts() {
    let (c, _d) = open();
    for i in 0..4 {
        row(&c, &fp(), "0:a:b", Some(&words(10, &format!("r{i}_"))), &format!("q{i}"));
    }
    let stems = vec![vec![sketch(words(10, "zz"), &["0:a:b"])]];
    assert_eq!(run(&c, &stems, &[vec![]], &[], 100).0, Vec::<usize>::new(), "four rows, none close");
    assert_eq!(run(&c, &stems, &[vec![]], &[], 3).0, vec![0], "max 3 < 4 rows: fail closed");
}
