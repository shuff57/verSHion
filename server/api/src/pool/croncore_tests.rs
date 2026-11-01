//! Tests for `croncore.rs`: stages are isolated and flagged once an hour; abandoned stagings go; the reconciler marks vanished blobs; the traps flag; the
//! daily put-back and quarantine stages.
use super::*;
use crate::db;
use rusqlite::Connection;
use std::path::Path;

fn open() -> (Connection, tempfile::TempDir, BlobStore) {
    let dir = tempfile::tempdir().unwrap();
    let mut c = db::open(&dir.path().join("t.db")).unwrap();
    db::migrate(&mut c, &Path::new(env!("CARGO_MANIFEST_DIR")).join("../worker/migrations")).unwrap();
    c.execute("INSERT INTO users (id, email, display_name, trust, created_at) VALUES ('a', 'a@x.org', 'a', 'teacher', 1)", []).unwrap();
    let s = BlobStore::new(dir.path().join("blobs")).unwrap();
    (c, dir, s)
}
/// Inserts a row giving the named columns and a neutral value to every other NOT NULL column without a default.
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
fn n(c: &Connection, sql: &str) -> i64 {
    c.query_row(sql, [], |r| r.get(0)).unwrap()
}
fn t(s: &str) -> rusqlite::types::Value {
    s.to_string().into()
}
fn i(v: i64) -> rusqlite::types::Value {
    v.into()
}

#[test]
fn a_failed_stage_returns_its_fallback_and_is_flagged_once_an_hour() {
    let (c, _d, _s) = open();
    let x = PoolCtx::new(&c, 10 * HOUR_MS + 5);
    let fail = || -> Res<i32> { Err(Fail::Pool(super::super::ctx::PoolError::new(500, "boom"))) };
    assert_eq!(stage(&x, "gc", 7, fail), 7);
    assert_eq!(stage(&x, "gc", 7, fail), 7);
    assert_eq!(n(&c, "SELECT COUNT(*) FROM admin_flags WHERE kind = 'cron_failure'"), 1);
    assert_eq!(stage(&PoolCtx::new(&c, 11 * HOUR_MS), "gc", 0, fail), 0);
    assert_eq!(n(&c, "SELECT COUNT(*) FROM admin_flags WHERE kind = 'cron_failure'"), 2);
    assert_eq!(stage(&x, "ok", 0, || Ok(3)), 3);
}

#[test]
fn only_stale_never_committed_publishes_are_discarded_with_their_bodies() {
    let (c, _d, s) = open();
    let now = 10 * HOUR_MS;
    let mk = |p: &str, status: &str, created: i64| {
        row(&c, "items", &[("kind", t("bank")), ("visibility", t("unlisted")), ("id", t(&format!("it-{p}"))), ("author_id", t("a")), ("created_at", i(1)), ("updated_at", i(1))]);
        row(&c, "item_publishes", &[("publish_id", t(p)), ("item_id", t(&format!("it-{p}"))), ("author_id", t("a")), ("status", t(status)), ("created_at", i(created)), ("full_key", t(&format!("k/{p}"))), ("stripped_key", t(&format!("s/{p}")))]);
        s.put(&format!("k/{p}"), b"x").unwrap();
        s.put(&format!("s/{p}"), b"y").unwrap();
    };
    mk("old", "staged", now - 2 * HOUR_MS);
    mk("new", "staged", now - 10);
    mk("live", "current", now - 2 * HOUR_MS);
    assert_eq!(discard_abandoned(&PoolCtx::new(&c, now), &s).unwrap(), 1);
    assert_eq!(n(&c, "SELECT COUNT(*) FROM item_publishes"), 2);
    assert!(s.get("k/old").unwrap().is_none() && s.get("s/old").unwrap().is_none());
    assert!(s.get("k/new").unwrap().is_some() && s.get("k/live").unwrap().is_some());
}

#[test]
fn the_reconciler_marks_a_blob_without_bytes_and_clears_one_found_again() {
    let (c, _d, s) = open();
    row(&c, "blobs", &[("content_type", t("image/png")), ("sha256", t("aa")), ("checked_at", i(5))]);
    row(&c, "blobs", &[("content_type", t("image/png")), ("sha256", t("bb")), ("missing_at", i(3))]);
    s.put("blobs/bb", b"here").unwrap();
    assert_eq!(reconcile(&PoolCtx::new(&c, 1_000), &s).unwrap(), 1);
    assert_eq!((n(&c, "SELECT missing_at FROM blobs WHERE sha256 = 'aa'"), n(&c, "SELECT missing_at IS NULL FROM blobs WHERE sha256 = 'bb'")), (1_000, 1));
    assert_eq!(n(&c, "SELECT checked_at FROM blobs WHERE sha256 = 'bb'"), 1_000);
}

#[test]
fn closed_licence_rows_no_serve_reaches_are_collected() {
    let (c, _d, _s) = open();
    row(&c, "items", &[("kind", t("bank")), ("visibility", t("unlisted")), ("id", t("it")), ("author_id", t("a")), ("created_at", i(1)), ("updated_at", i(1))]);
    row(&c, "content_licences", &[("item_id", t("it")), ("ckey", t("k1")), ("until_at", i(50))]);
    row(&c, "content_licences", &[("item_id", t("it")), ("ckey", t("k2")), ("until_at", i(50))]);
    row(&c, "content_licences", &[("item_id", t("it")), ("ckey", t("k3"))]);
    row(&c, "served", &[("kind", t("view")), ("item_id", t("it")), ("reader_id", t("a")), ("first_at", i(40)), ("at", i(40))]);
    // A serve at 40 reaches the rows closed at 50: nothing is collected; once the serve is later, both go (the open row stays).
    assert_eq!(content_licences_gc(&PoolCtx::new(&c, 100)).unwrap(), 0);
    c.execute("UPDATE served SET first_at = 60", []).unwrap();
    assert_eq!(content_licences_gc(&PoolCtx::new(&c, 100)).unwrap(), 2);
    assert_eq!(n(&c, "SELECT COUNT(*) FROM content_licences"), 1);
}

#[test]
fn more_than_the_trap_limit_in_an_hour_raises_one_flag_and_old_counters_go() {
    let (c, _d, _s) = open();
    let now = 100 * HOUR_MS + 1;
    c.execute("INSERT INTO pool_kv (key, value) VALUES ('wasm_traps:99', '9'), ('wasm_traps:100', '2'), ('wasm_traps:50', '1')", []).unwrap();
    wasm_trap_check(&PoolCtx::new(&c, now)).unwrap();
    wasm_trap_check(&PoolCtx::new(&c, now)).unwrap();
    assert_eq!(n(&c, "SELECT COUNT(*) FROM admin_flags WHERE kind = 'wasm_trap'"), 1);
    assert_eq!(n(&c, "SELECT COUNT(*) FROM pool_kv WHERE key = 'wasm_traps:50'"), 0);
    assert_eq!(n(&c, "SELECT COUNT(*) FROM pool_kv"), 2);
}

#[test]
fn an_unanswered_takedown_gets_its_purge_date_after_thirty_days() {
    let (c, _d, s) = open();
    let now = 100 * DAY_MS;
    for (it, notified) in [("due", now - QUARANTINE_MS - 1), ("early", now - 1000)] {
        row(&c, "items", &[("kind", t("bank")), ("visibility", t("unlisted")), ("id", t(it)), ("author_id", t("a")), ("created_at", i(1)), ("updated_at", i(1)), ("removed_at", i(5)), ("removed_reason", t("takedown"))]);
        row(&c, "dmca_actions", &[("id", t(&format!("act-{it}"))), ("item_id", t(it)), ("author_id", t("a")), ("disabled_at", i(5)), ("author_notified_at", i(notified))]);
    }
    let r = daily_early(&PoolCtx::new(&c, now), &s);
    assert_eq!(r, DailyEarly { restored: 0, quarantine_expired: 1 });
    assert_eq!(n(&c, "SELECT purge_after FROM items WHERE id = 'due'"), now);
    assert_eq!(n(&c, "SELECT purge_after IS NULL FROM items WHERE id = 'early'"), 1);
}

#[test]
fn a_counter_noticed_takedown_is_put_back_once_its_wait_has_passed() {
    let (c, _d, s) = open();
    row(&c, "items", &[("kind", t("bank")), ("visibility", t("unlisted")), ("id", t("it")), ("author_id", t("a")), ("created_at", i(1)), ("updated_at", i(1)), ("removed_at", i(5)), ("removed_reason", t("takedown"))]);
    row(&c, "dmca_actions", &[("id", t("act")), ("item_id", t("it")), ("author_id", t("a")), ("disabled_at", i(5)), ("counter_notice_id", t("cn")), ("restore_not_before", i(500))]);
    assert_eq!(daily_early(&PoolCtx::new(&c, 400), &s).restored, 0);
    assert_eq!(daily_early(&PoolCtx::new(&c, 600), &s).restored, 1);
    assert_eq!(n(&c, "SELECT removed_at IS NULL FROM items"), 1);
    assert_eq!(c.query_row::<String, _, _>("SELECT outcome FROM dmca_actions", [], |r| r.get(0)).unwrap(), "put_back");
}

#[test]
fn the_hourly_run_expires_pending_uploads_and_sends_the_outbox() {
    let (c, _d, s) = open();
    let now = 100 * HOUR_MS;
    row(&c, "pending_blobs", &[("sha256", t("p1")), ("until", i(now - 2 * HOUR_MS))]);
    row(&c, "pending_blobs", &[("sha256", t("p2")), ("until", i(now))]);
    let mut sent = 0;
    let rep = hourly(&PoolCtx::new(&c, now), &s, &mut |_| {
        sent += 1;
        Ok(())
    });
    assert_eq!((rep.pending, rep.staged, rep.missing), (1, 0, 0));
    assert_eq!(sent, 0);
    assert_eq!(n(&c, "SELECT COUNT(*) FROM pending_blobs"), 1);
}
