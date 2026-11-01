//! Tests for `cronretain.rs`: privacy bytes go after the grace window (unless an appeal is open or an author uses them unserved); retention removes only closed
//! takedown records past three years; deletion rewrites are bounded and resumable.
use super::*;
use crate::db;
use rusqlite::Connection;
use std::path::Path;

fn open() -> (Connection, tempfile::TempDir, BlobStore) {
    let dir = tempfile::tempdir().unwrap();
    let mut c = db::open(&dir.path().join("t.db")).unwrap();
    db::migrate(&mut c, &Path::new(env!("CARGO_MANIFEST_DIR")).join("../worker/migrations")).unwrap();
    for u in ["a", "b"] {
        c.execute("INSERT INTO users (id, email, display_name, trust, created_at) VALUES (?1, ?2, ?1, 'teacher', 1)", params![u, format!("{u}@x.org")]).unwrap();
    }
    let s = BlobStore::new(dir.path().join("blobs")).unwrap();
    (c, dir, s)
}
fn row(c: &Connection, table: &str, set: &[(&str, rusqlite::types::Value)]) {
    let cols: Vec<(String, String)> = c
        .prepare(&format!("SELECT name, type FROM pragma_table_info('{table}') WHERE \"notnull\" = 1 AND dflt_value IS NULL"))
        .unwrap()
        .query_map([], |r| Ok((r.get(0)?, r.get(1)?)))
        .unwrap()
        .map(|x| x.unwrap())
        .collect();
    let mut names: Vec<String> = vec![];
    let mut vals: Vec<rusqlite::types::Value> = vec![];
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
fn t(s: &str) -> rusqlite::types::Value {
    s.to_string().into()
}
fn i(v: i64) -> rusqlite::types::Value {
    v.into()
}
fn n(c: &Connection, sql: &str) -> i64 {
    c.query_row(sql, [], |r| r.get(0)).unwrap()
}
fn item(c: &Connection, id: &str, author: &str) {
    row(c, "items", &[("id", t(id)), ("author_id", t(author)), ("kind", t("bank")), ("visibility", t("unlisted")), ("created_at", i(1)), ("updated_at", i(1))]);
}

#[test]
fn blocked_images_lose_their_bytes_after_the_grace_unless_appealed_or_used_unserved() {
    let (c, _d, s) = open();
    item(&c, "blocker", "a");
    item(&c, "appealed", "a");
    for (sha, owner) in [("h1", "blocker"), ("h2", "appealed"), ("h3", "blocker")] {
        row(&c, "blobs", &[("sha256", t(sha)), ("content_type", t("image/png"))]);
        s.put(&format!("blobs/{sha}"), b"img").unwrap();
        row(&c, "privacy_blocks", &[("kind", t("blob")), ("hash", t(sha)), ("item_id", t(owner)), ("bytes_purge_after", i(100))]);
    }
    c.execute("INSERT INTO admin_flags (id, kind, item_id, dedup, at) VALUES ('f', 'appeal', 'appealed', 'd', 1)", []).unwrap();
    // h3 is still used, unserved, by a hidden publish's full form.
    row(&c, "item_publishes", &[("publish_id", t("pp")), ("item_id", t("blocker")), ("author_id", t("b")), ("status", t("current")), ("hide_key", i(1))]);
    row(&c, "item_blobs", &[("publish_id", t("pp")), ("form", t("full")), ("sha256", t("h3"))]);
    privacy_bytes(&PoolCtx::new(&c, 200), &s).unwrap();
    assert_eq!(n(&c, "SELECT COUNT(*) FROM privacy_blocks WHERE bytes_purged_at = 200"), 2, "the appealed block waits");
    assert_eq!(n(&c, "SELECT privacy_purged_at FROM blobs WHERE sha256 = 'h1'"), 200);
    assert!(s.get("blobs/h1").unwrap().is_none(), "unused: bytes deleted");
    assert!(s.get("blobs/h3").unwrap().is_some(), "an author's private use keeps the bytes");
    assert!(s.get("blobs/h2").unwrap().is_some());
    assert_eq!(n(&c, "SELECT missing_at IS NULL FROM blobs WHERE sha256 = 'h3'"), 1);
}

fn closed_action(c: &Connection, id: &str, item_id: &str, disabled: i64) {
    row(c, "dmca_actions", &[("id", t(id)), ("item_id", t(item_id)), ("author_id", t("a")), ("disabled_at", i(disabled)), ("restored_at", i(disabled + 10)), ("notice_id", t("n1"))]);
}

#[test]
fn only_closed_takedowns_past_three_years_with_nothing_depending_on_them_are_removed() {
    let (c, _d, _s) = open();
    item(&c, "it1", "a");
    item(&c, "it2", "a");
    row(&c, "dmca_notices", &[("id", t("n1")), ("received_at", i(1000)), ("status", t("actioned"))]);
    closed_action(&c, "old", "it1", 1000);
    closed_action(&c, "held", "it2", 1000);
    row(&c, "strikes", &[("id", t("s1")), ("user_id", t("a")), ("action_id", t("old")), ("at", i(1000))]);
    row(&c, "item_holds", &[("item_id", t("x")), ("reason", t("admin")), ("source", t("dmca:held"))]);
    let now = RETENTION_MS + 1_000_000;
    let r = retention(&PoolCtx::new(&c, now)).unwrap();
    assert_eq!(r.actions, 1);
    assert_eq!(n(&c, "SELECT COUNT(*) FROM dmca_actions WHERE id = 'old'"), 0);
    assert_eq!(n(&c, "SELECT COUNT(*) FROM strikes"), 0);
    assert_eq!(n(&c, "SELECT COUNT(*) FROM dmca_actions WHERE id = 'held'"), 1, "a copy it holds keeps it");
    // The notice is still named by the kept action: it stays; once that goes, a non-open notice goes too.
    assert_eq!(r.notices, 0);
    c.execute("DELETE FROM item_holds", []).unwrap();
    let r2 = retention(&PoolCtx::new(&c, now)).unwrap();
    assert_eq!((r2.actions, r2.notices, n(&c, "SELECT COUNT(*) FROM dmca_actions")), (1, 1, 0), "the notice is no longer named, so it goes in the same run");
}

#[test]
fn a_young_or_open_takedown_is_never_removed() {
    let (c, _d, _s) = open();
    item(&c, "it1", "a");
    closed_action(&c, "young", "it1", 5_000_000);
    row(&c, "dmca_notices", &[("id", t("open1")), ("received_at", i(10)), ("status", t("open"))]);
    let now = RETENTION_MS + 1_000_000;
    let r = retention(&PoolCtx::new(&c, now)).unwrap();
    assert_eq!(r, Retained::default());
    assert_eq!(n(&c, "SELECT COUNT(*) FROM dmca_notices WHERE id = 'open1'"), 1, "an open notice can still be acted on");
}

#[test]
fn old_audit_rows_go_except_the_trail_of_an_item_with_a_hold_or_a_kept_takedown() {
    let (c, _d, _s) = open();
    item(&c, "plain", "a");
    item(&c, "heldit", "a");
    row(&c, "item_holds", &[("item_id", t("heldit")), ("reason", t("admin")), ("source", t("manual"))]);
    for (id, it, at) in [("a1", "plain", 1000), ("a2", "heldit", 1000), ("a3", "plain", RETENTION_MS + 900_000)] {
        row(&c, "pool_audit", &[("id", t(id)), ("at", i(at)), ("actor_kind", t("system")), ("action", t("item.hold")), ("item_id", t(it))]);
    }
    let r = retention(&PoolCtx::new(&c, RETENTION_MS + 1_000_000)).unwrap();
    assert_eq!(r.audit, 1);
    assert_eq!(n(&c, "SELECT COUNT(*) FROM pool_audit WHERE id = 'a1'"), 0);
    assert_eq!(n(&c, "SELECT COUNT(*) FROM pool_audit WHERE id IN ('a2', 'a3')"), 2);
}

#[test]
fn a_deleted_accounts_name_in_others_chains_is_rewritten_in_resumable_batches() {
    let (c, _d, _s) = open();
    item(&c, "mine", "a");
    item(&c, "other", "b");
    row(&c, "item_publishes", &[("publish_id", t("pb")), ("item_id", t("other")), ("author_id", t("b")), ("status", t("current"))]);
    for k in 0..3 {
        let chain = if k == 1 { r#"[{"item_id":"other","author_name":"B"}]"# } else { r#"[{"item_id":"mine","author_name":"A"},{"item_id":"other","author_name":"B"}]"# };
        row(&c, "item_questions", &[("publish_id", t("pb")), ("qid", t(&format!("q{k}"))), ("pos", i(k)), ("qkind", t("Mc")), ("chain_json", t(chain))]);
    }
    c.execute("INSERT INTO deletion_rewrites (user_id, tbl, at) VALUES ('a', 'item_questions', 1)", []).unwrap();
    // One statement of one row a run: rows are visited in rowid order across runs.
    deletion_rewrites(&PoolCtx::new(&c, 50), 1, 1).unwrap();
    assert_eq!(n(&c, "SELECT COUNT(*) FROM item_questions WHERE chain_json LIKE '%deleted teacher%'"), 1);
    assert_eq!(n(&c, "SELECT done_at IS NULL FROM deletion_rewrites"), 1);
    deletion_rewrites(&PoolCtx::new(&c, 60), 1, 10).unwrap();
    assert_eq!(n(&c, "SELECT COUNT(*) FROM item_questions WHERE chain_json LIKE '%deleted teacher%'"), 2);
    assert_eq!(n(&c, "SELECT COUNT(*) FROM item_questions WHERE chain_json LIKE '%\"A\"%'"), 0);
    assert_eq!(n(&c, "SELECT done_at FROM deletion_rewrites"), 60);
    // The other author's link is untouched.
    assert_eq!(n(&c, "SELECT COUNT(*) FROM item_questions WHERE chain_json LIKE '%\"B\"%'"), 3);
    deletion_rewrites(&PoolCtx::new(&c, 70), 1, 10).unwrap();
}
