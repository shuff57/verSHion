//! Tests for `circlebanks.rs` on the real migrated schema: bank lookup, readability, kept versions, origins and the head of a read.
use super::*;
use crate::db;
use crate::pool::circles::create_circle;
use serde_json::Map;
use std::path::Path;

fn open() -> (rusqlite::Connection, tempfile::TempDir) {
    let dir = tempfile::tempdir().unwrap();
    let mut conn = db::open(&dir.path().join("t.db")).unwrap();
    db::migrate(&mut conn, &Path::new(env!("CARGO_MANIFEST_DIR")).join("../worker/migrations")).unwrap();
    (conn, dir)
}
fn user(c: &rusqlite::Connection, u: &str) {
    c.execute("INSERT INTO users (id, email, display_name, trust, created_at) VALUES (?1, ?2, ?3, 'teacher', 1)", params![u, format!("{u}@x.org"), format!("Teacher {u}")]).unwrap();
}
fn err(f: Fail) -> (u16, String, Value) {
    match f {
        Fail::Pool(e) => (e.status, e.code.clone(), e.body()),
        other => panic!("{other:?}"),
    }
}
fn make(c: &rusqlite::Connection, u: &str) -> String {
    let body: Map<String, Value> = json!({ "name": "Team" }).as_object().unwrap().clone();
    create_circle(&PoolCtx::new(c, 1_000), u, &body).unwrap().1["circle"]["id"].as_str().unwrap().to_string()
}
fn join(c: &rusqlite::Connection, cid: &str, u: &str, role: &str, seq: i64) {
    c.execute("INSERT INTO circle_members (circle_id, user_id, role, joined_at, join_seq) VALUES (?1, ?2, ?3, 10, ?4)", params![cid, u, role, seq]).unwrap();
}
fn bank(c: &rusqlite::Connection, cid: &str, author: &str) -> String {
    let id = format!("{:032x}", 1);
    c.execute("INSERT INTO circle_banks (id, circle_id, author_id, title, license, head_version, head_version_id, created_at, updated_at) VALUES (?1, ?2, ?3, 'Quiz', 'circle-only', 3, 'v3', 1, 77)", params![id, cid, author]).unwrap();
    id
}
fn version(c: &rusqlite::Connection, bid: &str, cid: &str, n: i64, status: &str, key: &str) {
    c.execute(
        "INSERT INTO circle_versions (id, bank_id, circle_id, version, commit_id, request_sha256, committer_id, license, status, full_key, index_key, bytes, question_count, created_at, committed_at, message)
         VALUES (?1, ?2, ?3, ?4, ?5, 'x', 'a', 'circle-only', ?6, ?7, 'ik', 1, 1, 600, 600, 'note')",
        params![format!("v{n}"), bid, cid, n, format!("c{n}"), status, key],
    )
    .unwrap();
}
fn no_hide(_: &str, _: &str, _: &str) -> bool {
    false
}

fn world() -> (rusqlite::Connection, tempfile::TempDir, String, String) {
    let (c, d) = open();
    for u in ["a", "b", "c", "z"] {
        user(&c, u);
    }
    let cid = make(&c, "a");
    join(&c, &cid, "b", "editor", 5);
    join(&c, &cid, "c", "viewer", 6);
    let bid = bank(&c, &cid, "b");
    version(&c, &bid, &cid, 1, "previous", "k1");
    version(&c, &bid, &cid, 2, "previous", "k2");
    version(&c, &bid, &cid, 3, "current", "k3");
    (c, d, cid, bid)
}

#[test]
fn lookup_needs_membership_and_a_real_bank_and_counts_misses() {
    let (c, _d, cid, bid) = world();
    let x = PoolCtx::new(&c, 5_000);
    let (m, b) = bank_or_miss(&x, &cid, &bid, "c").unwrap();
    assert_eq!((m.role.as_str(), b.title.as_str(), b.head_version, b.author_ok, b.held), ("viewer", "Quiz", 3, true, false));
    for (who, id) in [("z", bid.as_str()), ("c", "nope"), ("c", &format!("{:032x}", 9))] {
        assert_eq!(err(bank_or_miss(&x, &cid, id, who).unwrap_err()).1, "not_found");
    }
    let misses: i64 = c.query_row("SELECT COALESCE(SUM(count),0) FROM usage_daily WHERE metric = 'item_miss'", [], |r| r.get(0)).unwrap();
    assert!(misses >= 2, "the member's misses are counted");
}

#[test]
fn readable_follows_removal_holds_and_the_circle_state() {
    let (c, _d, cid, bid) = world();
    let x = PoolCtx::new(&c, 5_000);
    let read = |who: &str| -> Result<(), (u16, String, Value)> {
        let (m, b) = bank_or_miss(&x, &cid, &bid, who).unwrap();
        require_readable(&x, &m, &b, who).map_err(err)
    };
    assert!(read("c").is_ok());
    c.execute("INSERT INTO circle_holds (bank_id, reason, source, at) VALUES (?1, 'reports', '', 1)", [&bid]).unwrap();
    assert_eq!(read("c").unwrap_err().1, "not_found", "a viewer cannot read a held bank");
    assert!(read("b").is_ok(), "its author can");
    assert_eq!(read("a").unwrap_err().1, "not_found", "the owner cannot while a report blocks it");
    c.execute("DELETE FROM circle_holds", []).unwrap();
    c.execute("INSERT INTO circle_holds (bank_id, reason, source, at) VALUES (?1, 'student_data', 'privacy', 1)", [&bid]).unwrap();
    assert!(read("a").is_ok(), "the owner may read a student-data hold");
    c.execute("DELETE FROM circle_holds", []).unwrap();
    c.execute("UPDATE circle_banks SET removed_at = 5, removed_reason = 'author_removed', purge_after = 900", []).unwrap();
    let e = read("c").unwrap_err();
    assert_eq!((e.0, e.1.as_str(), e.2["reason"].clone(), e.2["undo_until"].clone()), (409, "bank_removed", json!("author_removed"), json!(900)));
    c.execute("UPDATE circle_banks SET removed_reason = 'circle_closed'", []).unwrap();
    assert_eq!(read("c").unwrap_err().2["undo_until"], json!(null));
}

#[test]
fn kept_info_finds_the_run_ending_at_the_head_and_pins_older_ones() {
    let (c, _d, _cid, bid) = world();
    let x = PoolCtx::new(&c, 5_000);
    assert_eq!(kept_info(&x, &bid, 3).unwrap(), Kept { oldest_kept: Some(1), pinned_versions: vec![] });
    // v2 is withheld: the run starts at 3 and v1 is pinned.
    c.execute("UPDATE circle_versions SET withheld_at = 9 WHERE id = 'v2'", []).unwrap();
    assert_eq!(kept_info(&x, &bid, 3).unwrap(), Kept { oldest_kept: Some(3), pinned_versions: vec![1] });
    // Pruned ones (no key) are not kept; a bank with nothing kept says so.
    c.execute("UPDATE circle_versions SET status = 'pruned', full_key = ''", []).unwrap();
    assert_eq!(kept_info(&x, &bid, 3).unwrap(), Kept { oldest_kept: None, pinned_versions: vec![] });
}

#[test]
fn origins_show_names_read_now() {
    let (c, _d, _cid, _bid) = world();
    let x = PoolCtx::new(&c, 5_000);
    assert!(origins_for_read(&x, None).unwrap().is_empty());
    assert!(origins_for_read(&x, Some("junk")).unwrap().is_empty());
    assert!(origins_for_read(&x, Some("[]")).unwrap().is_empty());
    let o = origins_for_read(&x, Some(r#"[{"pos":0,"item_id":"i-gone","license":"CC-BY"}]"#)).unwrap();
    assert_eq!(o[0]["author_name"], json!(DELETED_NAME));
    assert_eq!((o[0]["qid"].clone(), o[0]["version"].clone(), o[0]["pos"].clone()), (json!(null), json!(null), json!(0)));
}

#[test]
fn a_read_returns_the_head_for_the_head_or_a_kept_version_and_refuses_the_rest() {
    let (c, _d, cid, bid) = world();
    let x = PoolCtx::new(&c, 5_000);
    let r = bank_read(&x, "b", &cid, &bid, None, &no_hide).unwrap();
    let h: Value = serde_json::from_str(&r.head).unwrap();
    assert_eq!((h["bank"]["version"].clone(), h["bank"]["author_name"].clone(), h["bank"]["is_author"].clone(), h["bank"]["message"].clone()), (json!(3), json!("Teacher b"), json!(true), json!("note")));
    assert_eq!((h["circle"]["role"].clone(), r.full_key.as_str(), r.hidden), (json!("editor"), "k3", false));
    assert!(r.etag.starts_with('"') && r.etag.len() == 34);
    assert_eq!(bank_read(&x, "b", &cid, &bid, None, &no_hide).unwrap().etag, r.etag, "stable");
    let old = bank_read(&x, "c", &cid, &bid, Some(2), &no_hide).unwrap();
    assert_eq!((old.full_key.as_str(), serde_json::from_str::<Value>(&old.head).unwrap()["bank"]["head_version"].clone()), ("k2", json!(3)));
    // Out of range is a plain 404; a pruned version says what is kept.
    assert_eq!(err(bank_read(&x, "c", &cid, &bid, Some(9), &no_hide).unwrap_err()).1, "not_found");
    c.execute("UPDATE circle_versions SET status = 'pruned', full_key = '' WHERE id = 'v1'", []).unwrap();
    let (s, code, body) = err(bank_read(&x, "c", &cid, &bid, Some(1), &no_hide).unwrap_err());
    assert_eq!((s, code.as_str(), body["oldest_kept"].clone()), (404, "version_not_retained", json!(2)));
    // Each read is charged (the refused out-of-range one is not).
    let n: i64 = c.query_row("SELECT count FROM usage_daily WHERE user_id = 'c' AND metric = 'item_reads'", [], |r| r.get(0)).unwrap();
    assert_eq!(n, 2);
}

#[test]
fn withheld_versions_are_served_to_nobody_and_only_managers_learn_about_hiding() {
    let (c, _d, cid, bid) = world();
    let x = PoolCtx::new(&c, 5_000);
    c.execute("UPDATE circle_versions SET withheld_at = 9 WHERE id = 'v3'", []).unwrap();
    let viewer = err(bank_read(&x, "c", &cid, &bid, None, &no_hide).unwrap_err());
    assert_eq!((viewer.0, viewer.1.as_str()), (404, "version_withheld"));
    assert!(viewer.2.get("hide_off_releases").is_none());
    c.execute("UPDATE circle_banks SET hide_key = 1", []).unwrap();
    let owner = err(bank_read(&x, "a", &cid, &bid, None, &|_, _, _| true).unwrap_err());
    assert_eq!(owner.2["hide_off_releases"], json!(true));
    assert_eq!(err(bank_read(&x, "b", &cid, &bid, None, &no_hide).unwrap_err()).2["hide_off_releases"], json!(false), "the author while an editor");
    // A withheld version that was then pruned has its own words.
    c.execute("UPDATE circle_versions SET status = 'pruned', full_key = '', withheld_at = 9 WHERE id = 'v1'", []).unwrap();
    let e = err(bank_read(&x, "c", &cid, &bid, Some(1), &no_hide).unwrap_err());
    assert_eq!((e.1.as_str(), e.2["withheld"].clone(), e.2["review_copy"].clone()), ("version_not_retained", json!(true), json!("hashes_only")));
}

#[test]
fn a_viewer_of_a_key_hiding_bank_needs_recorded_strip_decisions() {
    let (c, _d, cid, bid) = world();
    let x = PoolCtx::new(&c, 5_000);
    c.execute("UPDATE circle_banks SET hide_key = 1", []).unwrap();
    assert_eq!(err(bank_read(&x, "c", &cid, &bid, None, &no_hide).unwrap_err()).1, "keys_hidden_unavailable");
    c.execute("UPDATE circle_versions SET strip_opts = '{\"a\":1}' WHERE id = 'v3'", []).unwrap();
    let v = bank_read(&x, "c", &cid, &bid, None, &no_hide).unwrap();
    let h: Value = serde_json::from_str(&v.head).unwrap();
    assert_eq!((v.hidden, h["bank"]["keys_hidden"].clone(), h["bank"]["message"].clone()), (true, json!(true), json!("")));
    // The editor reads the full text, and the viewer's ETag differs from it.
    let e = bank_read(&x, "b", &cid, &bid, None, &no_hide).unwrap();
    assert!(!e.hidden);
    assert_ne!(e.etag, v.etag);
}
