//! Tests for `cronpublish.rs`: the licence re-key maps only what it can (merging windows), and the stem backfill fills the full explanation key from a shown
//! publish's column or a hidden publish's retained body, counting unreadable bodies until it skips them.
use super::*;
use crate::db;
use rusqlite::Connection;
use rusqlite::types::Value;
use std::path::Path;

const ROOT: &str = "7f757fb737085dcaaad0727deb89a14c18141681dc56be0a215a592c715f8e3b";

fn open() -> (Connection, tempfile::TempDir, BlobStore) {
    let dir = tempfile::tempdir().unwrap();
    let mut c = db::open(&dir.path().join("t.db")).unwrap();
    db::migrate(&mut c, &Path::new(env!("CARGO_MANIFEST_DIR")).join("../worker/migrations")).unwrap();
    c.execute("INSERT INTO users (id, email, display_name, trust, created_at) VALUES ('a', 'a@x.org', 'a', 'teacher', 1)", []).unwrap();
    let s = BlobStore::new(dir.path().join("blobs")).unwrap();
    (c, dir, s)
}
/// Insert a row, filling every other NOT NULL column without a default with 0 / ''.
fn row(c: &Connection, table: &str, set: &[(&str, Value)]) {
    let cols: Vec<(String, String)> = c
        .prepare(&format!("SELECT name, type FROM pragma_table_info('{table}') WHERE \"notnull\" = 1 AND dflt_value IS NULL"))
        .unwrap()
        .query_map([], |r| Ok((r.get(0)?, r.get(1)?)))
        .unwrap()
        .map(|x| x.unwrap())
        .collect();
    let mut names = vec![];
    let mut vals: Vec<Value> = vec![];
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
fn t(s: &str) -> Value {
    s.to_string().into()
}
fn i(v: i64) -> Value {
    v.into()
}
fn n(c: &Connection, sql: &str) -> i64 {
    c.query_row(sql, [], |r| r.get(0)).unwrap()
}
fn item(c: &Connection) {
    row(c, "items", &[("id", t("it")), ("author_id", t("a")), ("kind", t("bank")), ("visibility", t("unlisted")), ("created_at", i(1)), ("updated_at", i(1))]);
}

#[test]
fn old_style_licence_rows_move_to_the_near_key_and_windows_merge() {
    let (c, _d, _s) = open();
    item(&c);
    row(&c, "item_publishes", &[("publish_id", t("p1")), ("item_id", t("it")), ("author_id", t("a")), ("status", t("current")), ("hide_key", i(0))]);
    row(&c, "item_questions", &[("publish_id", t("p1")), ("qid", t("q1")), ("pos", i(0)), ("qkind", t("Mc")), ("full_content", t("body one")), ("near_full", t("nearhash"))]);
    // The old-style row, an already-new-style row with an overlapping window, and an unmappable row (its content is gone).
    row(&c, "content_licences", &[("item_id", t("it")), ("ckey", t("q:body one")), ("license", t("CC-BY-4.0")), ("from_at", i(100)), ("until_at", Value::Null)]);
    row(&c, "content_licences", &[("item_id", t("it")), ("ckey", t("q:nearhash")), ("license", t("CC-BY-4.0")), ("from_at", i(300)), ("until_at", i(400))]);
    row(&c, "content_licences", &[("item_id", t("it")), ("ckey", t("q:gone")), ("license", t("CC-BY-4.0")), ("from_at", i(1)), ("until_at", Value::Null)]);
    row(&c, "item_offer", &[("item_id", t("it")), ("ckey", t("q:body one"))]);
    let changed = licence_rekey(&PoolCtx::new(&c, 1_000)).unwrap();
    assert_eq!(changed, 2, "one licence row and one offer row re-keyed");
    let (from, until): (i64, Option<i64>) = c.query_row("SELECT from_at, until_at FROM content_licences WHERE ckey = 'q:nearhash'", [], |r| Ok((r.get(0)?, r.get(1)?))).unwrap();
    assert_eq!((from, until), (100, None), "earliest start, open if either is open");
    assert_eq!(n(&c, "SELECT COUNT(*) FROM content_licences WHERE ckey = 'q:body one'"), 0);
    assert_eq!(n(&c, "SELECT COUNT(*) FROM content_licences WHERE ckey = 'q:gone'"), 1, "an unmappable row stays at rest");
    assert_eq!(n(&c, "SELECT COUNT(*) FROM item_offer WHERE ckey = 'q:nearhash'"), 1);
    assert_eq!(n(&c, "SELECT COUNT(*) FROM item_offer WHERE ckey = 'q:body one'"), 0);
    assert_eq!(licence_rekey(&PoolCtx::new(&c, 2_000)).unwrap(), 0, "a rerun changes nothing");
}

#[test]
fn a_ghosts_content_is_mapped_too() {
    let (c, _d, _s) = open();
    item(&c);
    row(&c, "ghost_publishes", &[("publish_id", t("g1")), ("item_id", t("it")), ("author_id", t("a")), ("hide_key", i(0)), ("public_version", i(1)), ("license", t("CC-BY-4.0")), ("at", i(1))]);
    row(&c, "ghost_questions", &[("publish_id", t("g1")), ("qid", t("q1")), ("pos", i(0)), ("full_content", t("old body")), ("near_full", t("ghostnear"))]);
    row(&c, "content_licences", &[("item_id", t("it")), ("ckey", t("Q:old body")), ("license", t("CC-BY-4.0")), ("from_at", i(5)), ("until_at", i(9))]);
    assert_eq!(licence_rekey(&PoolCtx::new(&c, 1_000)).unwrap(), 1);
    assert_eq!(n(&c, "SELECT COUNT(*) FROM content_licences WHERE ckey = 'q:ghostnear'"), 1);
}

fn stem_world(c: &Connection, hide_key: i64, full_key: &str, bk_expl: &str) {
    item(c);
    row(c, "item_publishes", &[("publish_id", t("p1")), ("item_id", t("it")), ("author_id", t("a")), ("status", t("current")), ("hide_key", i(hide_key)), ("full_key", t(full_key))]);
    row(
        c,
        "item_questions",
        &[("publish_id", t("p1")), ("qid", t("q1")), ("pos", i(0)), ("qkind", t("Mc")), ("match_hash", t("mh")), ("tok_sk", t("tok")), ("kbands", t("[\"b1\"]")), ("bk_expl", t(bk_expl)), ("seq", i(7))],
    );
}

#[test]
fn a_shown_publish_takes_its_explanation_key_from_the_column() {
    let (c, _d, s) = open();
    let fp = Fingerprinter::from_root_hex(ROOT).unwrap();
    stem_world(&c, 0, "bodies/p1", "e:abc");
    assert_eq!(stem_backfill(&PoolCtx::new(&c, 1_000), &s, &fp).unwrap(), 1);
    let (full, seq): (String, i64) = c.query_row("SELECT bk_expl_full, first_seq FROM author_stems WHERE author_id = 'a' AND band = 'b1'", [], |r| Ok((r.get(0)?, r.get(1)?))).unwrap();
    assert_eq!((full.as_str(), seq), ("e:abc", 7));
    assert_eq!(stem_backfill(&PoolCtx::new(&c, 2_000), &s, &fp).unwrap(), 0, "nothing left to fill");
}

#[test]
fn a_hidden_publish_computes_the_key_from_its_retained_body() {
    let (c, _d, s) = open();
    let fp = Fingerprinter::from_root_hex(ROOT).unwrap();
    stem_world(&c, 1, "bodies/p1", "");
    let body = r#"{"bank":{"questions":[{"id":"q1","stem":"What?","explanation":"Because.","kind":{"Mc":{"choices":[]}}}]}}"#;
    s.put("bodies/p1", body.as_bytes()).unwrap();
    assert_eq!(stem_backfill_author(&PoolCtx::new(&c, 1_000), &s, &fp, "a").unwrap(), 1);
    let full: String = c.query_row("SELECT bk_expl_full FROM author_stems", [], |r| r.get(0)).unwrap();
    assert!(full.starts_with("e:"), "the explanation key of the full form: {full}");
}

#[test]
fn an_unreadable_body_is_counted_then_skipped_and_the_admin_told_once() {
    let (c, _d, s) = open();
    let fp = Fingerprinter::from_root_hex(ROOT).unwrap();
    stem_world(&c, 1, "bodies/missing", "");
    for k in 1..=STEM_BACKFILL_FAILS {
        assert_eq!(stem_backfill_author(&PoolCtx::new(&c, 1_000 + k), &s, &fp, "a").unwrap(), 0);
        assert_eq!(skipped(&c, "p1"), k == STEM_BACKFILL_FAILS);
    }
    assert_eq!(n(&c, "SELECT COUNT(*) FROM admin_flags WHERE kind = 'cron_failure'"), 1);
    assert_eq!(n(&c, "SELECT COUNT(*) FROM author_stems"), 0, "unknown is never recorded as a key");
    // Once skipped it is no longer a candidate.
    assert_eq!(stem_backfill(&PoolCtx::new(&c, 5_000), &s, &fp).unwrap(), 0);
    assert_eq!(n(&c, "SELECT CAST(value AS INTEGER) FROM pool_kv WHERE key = 'stem_bf_fail:p1'"), 3);
}

#[test]
fn an_erased_author_is_never_backfilled() {
    let (c, _d, s) = open();
    let fp = Fingerprinter::from_root_hex(ROOT).unwrap();
    stem_world(&c, 0, "bodies/p1", "e:abc");
    c.execute("UPDATE users SET deleted_at = 5, email = NULL, pw_hash = NULL, pw_params = NULL, display_name = NULL, profile = NULL WHERE id = 'a'", []).unwrap();
    assert_eq!(stem_backfill(&PoolCtx::new(&c, 1_000), &s, &fp).unwrap(), 0);
    assert_eq!(n(&c, "SELECT COUNT(*) FROM author_stems"), 0);
}
