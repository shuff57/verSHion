//! Tests for `fuzzy.rs`: the comparisons are the Worker's, case by case; the SQL is prepared and run on the real schema.
use super::*;
use crate::db;
use std::path::Path;

fn open() -> (Connection, tempfile::TempDir) {
    let dir = tempfile::tempdir().unwrap();
    let mut conn = db::open(&dir.path().join("t.db")).unwrap();
    db::migrate(&mut conn, &Path::new(env!("CARGO_MANIFEST_DIR")).join("../worker/migrations")).unwrap();
    (conn, dir)
}

#[test]
fn min_first_takes_the_earlier_of_two_optional_seqs() {
    assert_eq!(min_first(None, None), None);
    assert_eq!(min_first(Some(5), None), Some(5));
    assert_eq!(min_first(None, Some(3)), Some(3));
    assert_eq!(min_first(Some(5), Some(3)), Some(3));
}

fn f<'a>(expl: Option<&'a str>, ans: Option<&'a str>) -> Fields<'a> {
    Fields { expl, ans }
}

#[test]
fn lacks_added_requires_the_source_to_differ_only_where_the_publisher_added() {
    // She added an explanation her loose earlier version lacked; the source has none: it lacks what she added.
    assert!(lacks_added(f(Some("E"), Some("A")), None, Some("A"), Some(f(None, Some("A")))));
    // The source carries the very explanation she added: it does not lack it.
    assert!(!lacks_added(f(Some("E"), Some("A")), Some("E"), Some("A"), Some(f(None, Some("A")))));
    // She added nothing (her earlier version had both): the source cannot lack anything, even without an explanation.
    assert!(lacks_added(f(Some("E"), Some("A")), None, None, Some(f(Some("E"), Some("A")))), "no loose field added, so the clause holds trivially");
    // Without a loose version every non-empty field of hers counts as added.
    assert!(!lacks_added(f(Some("E"), None), Some("E"), None, None));
    assert!(lacks_added(f(Some("E"), None), Some("X"), None, None));
    assert!(lacks_added(f(None, None), None, None, None));
    // An empty later field is never "added".
    assert!(lacks_added(f(Some(""), Some("")), None, None, None));
}

#[test]
fn adds_nothing_means_every_field_she_has_is_the_sources() {
    assert!(adds_nothing(f(None, None), Some("x"), None));
    assert!(adds_nothing(f(Some("E"), None), Some("E"), None));
    assert!(!adds_nothing(f(Some("E"), None), Some("X"), None));
    assert!(!adds_nothing(f(Some("E"), Some("A")), Some("E"), None));
}

#[test]
fn the_own_stems_query_prepares_and_an_empty_author_has_no_rows() {
    let (c, _d) = open();
    let mut st = c.prepare(OWN_STEMS_SQL).unwrap();
    let json = serde_json::json!([{"pos": 0, "bands": ["0:a:b"]}]).to_string();
    let n = st.query_map(params!["a", json, FUZZY_PAIRS_MAX], |_| Ok(())).unwrap().count();
    assert_eq!(n, 0);
}

#[derive(Clone, Debug, PartialEq)]
struct P {
    pos: usize,
    bands: Vec<String>,
}

fn stems(c: &Connection, author: &str, band: &str, n: usize) {
    for i in 0..n {
        c.execute("INSERT INTO author_stems (author_id, band, qhash, tok_sk, first_seq) VALUES (?1, ?2, ?3, 't', 1)", params![author, band, format!("h{i}")]).unwrap();
    }
}

#[test]
fn the_window_stops_at_the_position_where_the_busiest_bands_fill_the_row_cap() {
    let (c, _d) = open();
    stems(&c, "a", "0:busy", 1500);
    let kb: Vec<P> = (0..5).map(|pos| P { pos, bands: vec!["0:busy".into(), "1:none".into()] }).collect();
    let w = own_stems_window(&c, "a", &kb, |p| p.pos, |p| &p.bands).unwrap();
    // 1500 rows after position 0, 3000 >= 2000 after position 1: positions 0 and 1 can reach the result, no later one.
    assert_eq!(w.iter().map(|p| p.pos).collect::<Vec<_>>(), vec![0, 1]);
    // Few records: nothing is cut.
    assert_eq!(own_stems_window(&c, "someone", &kb, |p| p.pos, |p| &p.bands).unwrap().len(), 5);
    // No bands at all: unchanged.
    let none = vec![P { pos: 0, bands: vec![] }];
    assert_eq!(own_stems_window(&c, "a", &none, |p| p.pos, |p| &p.bands).unwrap(), none);
}
