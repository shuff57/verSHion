//! Tests for `visible.rs`: the rules its comments state, case by case, and the SQL it hands the evidence query run
//! against a real SQLite so the comparison itself is exercised.
use super::*;
use rusqlite::{Connection, params};

fn set(v: &[&str]) -> HashSet<String> {
    v.iter().map(|s| s.to_string()).collect()
}
fn shown() -> Subject {
    subject_for(false, "kf", "FULL", "NEAR", "nkf", Some("BARE"), true)
}
fn hidden() -> Subject {
    subject_for(true, "kf", "FULL", "NEAR", "nkf", Some("BARE"), true)
}
fn cand(item: &str, chain: &[&str], seq: i64, same: bool, same_full: bool) -> Candidate {
    Candidate { served: Served { same, same_full }, item_id: item.into(), chain_items: chain.iter().map(|s| s.to_string()).collect(), first_seq: seq }
}

#[test]
fn a_hidden_subject_carries_nothing_key_bearing() {
    let h = hidden();
    assert_eq!((h.full, h.near, h.near_bare, h.no_expl), (None, None, None, false));
    assert_eq!((h.keyfree.as_str(), h.near_kf.as_str()), ("kf", "nkf"));
    let s = shown();
    assert_eq!((s.full.as_deref(), s.near.as_deref(), s.near_bare.as_deref(), s.no_expl), (Some("FULL"), Some("NEAR"), Some("BARE"), true));
    assert_eq!(subject_for(false, "k", "f", "n", "nk", None, false).near_bare.as_deref(), Some("n"), "bare defaults to near");
    let p = subject_param(&hidden(), "mh", vec![SourceRef { item: "i".into(), qid: "q".into() }]);
    assert_eq!((p.full, p.nfull, p.nbare, p.noexpl, p.refs.len()), (None, None, None, 0, 1));
    assert_eq!(subject_param(&shown(), "mh", vec![]).noexpl, 1);
}

#[test]
fn a_source_counts_unless_it_is_the_publishers_own_lineage_or_later_than_their_first_publish() {
    let none = HashSet::new();
    let c = cand("src", &[], 5, false, false);
    assert!(admissible_with(&c, &shown(), &none, None, None, &none, None));
    assert!(admissible_with(&c, &shown(), &set(&["other"]), Some(9), None, &none, None), "earlier than my first publish");
    assert!(!admissible_with(&c, &shown(), &set(&["other"]), Some(5), None, &none, None), "same time or later does not count");
    assert!(!admissible_with(&c, &shown(), &set(&["src"]), None, None, &none, None), "my own item");
    assert!(!admissible_with(&cand("src", &["mine"], 1, false, false), &shown(), &set(&["mine"]), None, None, &none, None), "a chain through my item");
}

#[test]
fn a_later_or_derived_source_still_counts_when_the_publisher_took_its_additions() {
    let none = HashSet::new();
    let mine = set(&["mine"]);
    let derived = cand("adapter", &["mine"], 9, true, true);
    assert!(admissible_with(&derived, &shown(), &mine, Some(1), None, &none, None), "a shown subject equal to the full serve took the additions");
    assert!(!admissible_with(&derived, &hidden(), &mine, Some(1), None, &none, None), "a hidden key proves nothing");
    assert!(!admissible_with(&cand("adapter", &["mine"], 9, true, false), &shown(), &mine, Some(1), None, &none, None), "only the full serve");
    let own = own_shown([("FULL", false), ("SECRET", true)]);
    assert!(!admissible_with(&derived, &shown(), &mine, Some(1), Some(&own), &none, None), "it is the publisher's own earlier shown content");
    let own_hidden_only = own_shown([("FULL", true)]);
    assert!(admissible_with(&derived, &shown(), &mine, Some(1), Some(&own_hidden_only), &none, None), "a hidden publish never decides");
}

#[test]
fn void_material_taints_a_lineage_except_for_a_verbatim_copy() {
    let none = HashSet::new();
    let void = set(&["bad"]);
    assert!(!admissible_with(&cand("bad", &[], 1, true, true), &shown(), &none, None, None, &void, None), "the void item itself");
    assert!(!admissible_with(&cand("child", &["bad"], 1, false, false), &shown(), &none, None, None, &void, None), "shares what came from the void item");
    assert!(admissible_with(&cand("child", &["bad"], 1, true, true), &shown(), &none, None, None, &void, None), "a verbatim copy of its own additions");
    let lineage = set(&["root"]);
    assert!(!admissible_with(&cand("child", &["root"], 1, false, false), &shown(), &none, None, None, &none, Some(&lineage)), "a separate lineage set");
}

fn db() -> Connection {
    let c = Connection::open_in_memory().unwrap();
    c.execute_batch("CREATE TABLE iq (id INTEGER, content_hash TEXT, keyfree_hash TEXT, near_hash TEXT, near_bare TEXT, near_kf TEXT, form TEXT)").unwrap();
    c
}
fn matches(c: &Connection, sql: String, p: &SubjectParam) -> Vec<i64> {
    let q = format!("WITH s AS (SELECT ?1 AS full, ?2 AS kf, ?3 AS nfull, ?4 AS nkf, ?5 AS nbare, ?6 AS noexpl) SELECT iq.id FROM s, iq WHERE {} ORDER BY iq.id", sql);
    let mut st = c.prepare(&q).unwrap();
    st.query_map(params![p.full, p.kf, p.nfull, p.nkf, p.nbare, p.noexpl], |r| r.get(0)).unwrap().map(|x| x.unwrap()).collect()
}

#[test]
fn same_sql_compares_full_content_only_for_a_shown_subject() {
    let c = db();
    c.execute_batch("INSERT INTO iq VALUES (1, 'FULL', 'other', '', '', '', 'full'), (2, 'zzz', 'kf', '', '', '', 'full'), (3, 'zzz', 'nope', '', '', '', 'full')").unwrap();
    assert_eq!(matches(&c, same_sql("s", "iq"), &subject_param(&shown(), "mh", vec![])), vec![1]);
    assert_eq!(matches(&c, same_sql("s", "iq"), &subject_param(&hidden(), "mh", vec![])), vec![2], "a hidden subject matches on key-free facts only");
}

#[test]
fn near_sql_ignores_points_and_matches_a_stripped_serve_and_a_dropped_explanation() {
    let c = db();
    c.execute_batch(
        "INSERT INTO iq VALUES
         (1, '', '', 'NEAR', 'x', 'x', 'full'),
         (2, '', '', 'x', 'BARE', 'x', 'full'),
         (3, '', '', 'x', 'x', 'nkf', 'stripped'),
         (4, '', '', 'x', 'x', 'nkf', 'full'),
         (5, '', '', 'x', 'x', 'zzz', 'stripped')",
    )
    .unwrap();
    assert_eq!(matches(&c, near_sql("s", "iq"), &subject_param(&shown(), "mh", vec![])), vec![1, 2, 3]);
    let with_expl = subject_for(false, "kf", "FULL", "NEAR", "nkf", Some("BARE"), false);
    assert_eq!(matches(&c, near_sql("s", "iq"), &subject_param(&with_expl, "mh", vec![])), vec![1], "a subject with an explanation matches only its near hash");
    assert_eq!(matches(&c, near_sql("s", "iq"), &subject_param(&hidden(), "mh", vec![])), vec![3, 4], "hidden: key-free facts only");
}
