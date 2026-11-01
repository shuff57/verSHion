//! Tests for `publishearlier.rs`: the served-body comparison (what strip shuffles is compared as a multiset, nothing else), the served-order remap onto a
//! reused body, and the earlier-publication lookup's candidate choice (first overall, first sharing her added explanation or answer key).
use super::*;
use crate::db;
use rusqlite::types::Value as Sql;
use std::path::Path;

fn mc(stem: &str, answers: &[(&str, bool, bool)]) -> Value {
    let a: Vec<Value> = answers.iter().enumerate().map(|(i, (t, c, l))| json!({ "id": format!("a{i}"), "text": t, "correct": c, "locked": l })).collect();
    json!({ "id": "q", "stem": stem, "kind": { "Mc": { "answers": a } }, "created": 1, "modified": 1 })
}
fn bank(qs: Vec<Value>) -> Value {
    json!({ "schema_version": 2, "bank": { "id": "x", "name": "b", "questions": qs, "created": 1, "modified": 1 } })
}
fn so(question: i64, order: &[i64]) -> Value {
    json!({ "question": question, "order": order })
}

#[test]
fn ids_and_times_do_not_matter_but_content_does() {
    let a = bank(vec![mc("Which?", &[("one", true, false), ("two", false, false)])]);
    let mut b = a.clone();
    b["bank"]["created"] = json!(99);
    b["bank"]["questions"][0]["id"] = json!("other");
    b["bank"]["questions"][0]["kind"]["Mc"]["answers"][0]["id"] = json!("zzz");
    assert!(same_served(&a, &b, &[], &[]));
    let mut c = a.clone();
    c["bank"]["questions"][0]["stem"] = json!("Which one?");
    assert!(!same_served(&a, &c, &[], &[]));
    let mut d = a.clone();
    d["bank"]["name"] = json!("renamed");
    assert!(!same_served(&a, &d, &[], &[]), "the bank's own fields count");
}

#[test]
fn unlocked_choices_that_strip_shuffled_compare_as_a_multiset_but_kept_and_locked_ones_do_not() {
    let a = bank(vec![mc("Q", &[("one", true, false), ("two", false, false), ("three", false, true)])]);
    let swapped = bank(vec![mc("Q", &[("two", false, false), ("one", true, false), ("three", false, true)])]);
    // With the served orders naming question 0, the swap of unlocked choices is the shuffle.
    assert!(same_served(&a, &swapped, &[so(0, &[1, 0, 2])], &[]));
    // With no served orders every unlocked list is a shuffle candidate unless the author asked to keep its order.
    assert!(same_served(&a, &swapped, &[], &[]));
    assert!(!same_served(&a, &swapped, &[], &[so(0, &[0, 1, 2])]), "a kept order is compared in order");
    // A locked choice moving is a real change.
    let moved = bank(vec![mc("Q", &[("one", true, false), ("three", false, true), ("two", false, false)])]);
    assert!(!same_served(&a, &moved, &[so(0, &[0, 1, 2])], &[]));
}

#[test]
fn a_matching_word_bank_is_a_multiset_and_a_kept_left_column_is_ordered() {
    let m = |pairs: &[(&str, &str)], d: &[&str]| {
        json!({ "id": "q", "stem": "Match", "kind": { "Matching": { "pairs": pairs.iter().map(|(l, r)| json!({ "left": l, "right": r })).collect::<Vec<_>>(), "distractors": d } } })
    };
    let a = bank(vec![m(&[("L1", "R1"), ("L2", "R2")], &["X"])]);
    let b = bank(vec![m(&[("L1", "R2"), ("L2", "R1")], &["X"])]);
    assert!(same_served(&a, &b, &[], &[]), "the rights are shuffled among the pairs");
    let swapped_left = bank(vec![m(&[("L2", "R1"), ("L1", "R2")], &["X"])]);
    assert!(!same_served(&a, &swapped_left, &[], &[]), "the left column is ordered unless it was served shuffled");
    let lo = json!({ "question": 0, "matching": true, "order": [0, 1], "left_order": [1, 0] });
    assert!(same_served(&a, &swapped_left, &[lo], &[]));
}

#[test]
fn a_different_number_of_questions_is_never_the_same() {
    let q = mc("Q", &[("one", true, false)]);
    assert!(!same_served(&bank(vec![q.clone()]), &bank(vec![q.clone(), q]), &[], &[]));
}

#[test]
fn served_orders_follow_their_choices_onto_the_reused_body() {
    // The reused body shows the choices as [A, B, C]; the fresh strip shuffled them to [C, A, B]. The author's order on the fresh body [2, 0, 1] maps
    // each fresh position to the author's original index; remapped onto the reused body it lists the same choices in the reused body's order.
    let reused = bank(vec![mc("Q", &[("A", true, false), ("B", false, false), ("C", false, false)])]);
    let fresh = bank(vec![mc("Q", &[("C", false, false), ("A", true, false), ("B", false, false)])]);
    let out = remap_served_orders(&reused, &fresh, &[so(0, &[2, 0, 1])]).unwrap();
    assert_eq!(out, vec![so(0, &[0, 1, 2])]);
    // Identical bodies map to themselves.
    assert_eq!(remap_served_orders(&reused, &reused, &[so(0, &[1, 2, 0])]).unwrap(), vec![so(0, &[1, 2, 0])]);
}

#[test]
fn a_choice_without_a_counterpart_means_the_body_is_not_reused() {
    let reused = bank(vec![mc("Q", &[("A", true, false), ("B", false, false)])]);
    let fresh = bank(vec![mc("Q", &[("A", true, false), ("Z", false, false)])]);
    assert!(remap_served_orders(&reused, &fresh, &[so(0, &[0, 1])]).is_none());
    // An order for a question that does not exist, or of the wrong kind, cannot be remapped.
    assert!(remap_served_orders(&reused, &reused, &[so(5, &[0, 1])]).is_none());
    let matching = json!({ "question": 0, "matching": true, "order": [0] });
    assert!(remap_served_orders(&reused, &reused, &[matching]).is_none());
}

#[test]
fn a_matching_orders_distractors_and_left_column_are_remapped() {
    let m = |pairs: &[(&str, &str)], d: &[&str]| {
        bank(vec![json!({ "id": "q", "stem": "M", "kind": { "Matching": { "pairs": pairs.iter().map(|(l, r)| json!({ "left": l, "right": r })).collect::<Vec<_>>(), "distractors": d.iter().map(|t| json!({ "id": "d", "text": t })).collect::<Vec<_>>() } } })])
    };
    let reused = m(&[("L1", "R1"), ("L2", "R2")], &["X", "Y"]);
    let fresh = m(&[("L2", "R2"), ("L1", "R1")], &["Y", "X"]);
    let raw = json!({ "question": 0, "matching": true, "order": [1, 0], "left_order": [1, 0] });
    let out = remap_served_orders(&reused, &fresh, &[raw]).unwrap();
    assert_eq!(out[0]["order"], json!([0, 1]), "X is at fresh index 1 (entry 0), Y at index 0 (entry 1)");
    assert_eq!(out[0]["left_order"], json!([0, 1]));
}

// ---- the earlier-publication lookup ----

fn open() -> (rusqlite::Connection, tempfile::TempDir) {
    let dir = tempfile::tempdir().unwrap();
    let mut c = db::open(&dir.path().join("t.db")).unwrap();
    db::migrate(&mut c, &Path::new(env!("CARGO_MANIFEST_DIR")).join("../worker/migrations")).unwrap();
    for u in ["me", "o1", "o2"] {
        c.execute("INSERT INTO users (id, email, display_name, trust, created_at) VALUES (?1, ?2, ?1, 'teacher', 1)", rusqlite::params![u, format!("{u}@x.org")]).unwrap();
    }
    (c, dir)
}
fn row(c: &rusqlite::Connection, table: &str, set: &[(&str, Sql)]) {
    let cols: Vec<(String, String)> = c
        .prepare(&format!("SELECT name, type FROM pragma_table_info('{table}') WHERE \"notnull\" = 1 AND dflt_value IS NULL"))
        .unwrap()
        .query_map([], |r| Ok((r.get(0)?, r.get(1)?)))
        .unwrap()
        .map(|x| x.unwrap())
        .collect();
    let (mut names, mut vals): (Vec<String>, Vec<Sql>) = (vec![], vec![]);
    for (n, t) in cols {
        if set.iter().any(|(k, _)| *k == n) {
            continue;
        }
        names.push(n);
        vals.push(if t.to_uppercase().contains("INT") { 0.into() } else { String::new().into() });
    }
    for (k, v) in set {
        names.push(k.to_string());
        vals.push(v.clone());
    }
    let ph: Vec<String> = (1..=names.len()).map(|i| format!("?{i}")).collect();
    c.execute(&format!("INSERT INTO {table} ({}) VALUES ({})", names.join(", "), ph.join(", ")), rusqlite::params_from_iter(vals)).unwrap();
}
fn t(s: &str) -> Sql {
    s.to_string().into()
}
fn i(v: i64) -> Sql {
    v.into()
}
/// A public, visible item of `author` whose one question has the near hash `near` (shown form) and the stored keys.
fn source(c: &rusqlite::Connection, item: &str, author: &str, near: &str, seq: i64, bk_expl: Option<&str>) -> i64 {
    let publish = format!("p-{item}");
    row(c, "items", &[("id", t(item)), ("author_id", t(author)), ("kind", t("bank")), ("visibility", t("public")), ("current_publish_id", t(&publish)), ("public_publish_id", t(&publish)), ("created_at", i(1)), ("updated_at", i(1))]);
    row(c, "item_publishes", &[("publish_id", t(&publish)), ("item_id", t(item)), ("author_id", t(author)), ("status", t("current")), ("hide_key", i(0))]);
    row(
        c,
        "item_questions",
        &[("publish_id", t(&publish)), ("qid", t(&format!("q-{item}"))), ("pos", i(0)), ("qkind", t("Mc")), ("near_full", t(near)), ("seq", i(seq)), ("bk_expl", bk_expl.map(t).unwrap_or(Sql::Null))],
    );
    c.query_row("SELECT rowid FROM item_questions WHERE publish_id = ?1", [publish], |r| r.get(0)).unwrap()
}

#[test]
fn the_first_question_of_a_hash_is_the_candidate_and_never_her_own() {
    let (c, _d) = open();
    let early = source(&c, "i1", "o1", "h1", 5, None);
    let _late = source(&c, "i2", "o2", "h1", 9, None);
    let _hers = source(&c, "i3", "me", "h1", 1, None);
    let got = earlier_candidates(&c, "me", &[Want { n: vec!["h1".into()], ..Default::default() }, Want { n: vec!["nope".into()], ..Default::default() }]).unwrap();
    assert_eq!(got, vec![vec![early], vec![]]);
}

#[test]
fn a_later_source_sharing_her_added_explanation_is_a_candidate_too() {
    let (c, _d) = open();
    let first = source(&c, "i1", "o1", "h1", 5, Some("e:other"));
    let shares = source(&c, "i2", "o2", "h1", 9, Some("e:mine"));
    let got = earlier_candidates(&c, "me", &[Want { n: vec!["h1".into()], expl: Some("e:mine".into()), ans: None }]).unwrap();
    assert_eq!(got, vec![vec![first, shares]]);
    let without = earlier_candidates(&c, "me", &[Want { n: vec!["h1".into()], ..Default::default() }]).unwrap();
    assert_eq!(without, vec![vec![first]]);
}

#[test]
fn a_held_or_removed_item_is_not_a_source() {
    let (c, _d) = open();
    source(&c, "i1", "o1", "h1", 5, None);
    c.execute("INSERT INTO item_holds (item_id, reason, at, source) VALUES ('i1', 'admin', 1, 'manual')", []).unwrap();
    let ok = source(&c, "i2", "o2", "h1", 9, None);
    assert_eq!(earlier_candidates(&c, "me", &[Want { n: vec!["h1".into()], ..Default::default() }]).unwrap(), vec![vec![ok]]);
    c.execute("UPDATE items SET removed_at = 5, removed_reason = 'admin' WHERE id = 'i2'", []).unwrap();
    assert_eq!(earlier_candidates(&c, "me", &[Want { n: vec!["h1".into()], ..Default::default() }]).unwrap(), vec![Vec::<i64>::new()]);
}

#[test]
fn the_per_position_test_names_the_source_only_when_it_came_first() {
    let (c, _d) = open();
    let src = source(&c, "i1", "o1", "h1", 5, None);
    let run = |first: i64| -> Option<String> {
        let positions = json!([{ "i": 0, "c": [src], "n": ["h1"], "expl": null, "ans": null, "first": first }]).to_string();
        c.query_row(&earlier_sql(), rusqlite::params!["me", positions], |r| r.get(1)).unwrap()
    };
    assert!(run(6).is_some_and(|m| m.contains("\"item_id\":\"i1\"")), "her first (6) is after the source (5): the source is earlier");
    assert!(run(5).is_none(), "equal seq is not earlier");
    assert!(run(3).is_none(), "she had it first");
}
