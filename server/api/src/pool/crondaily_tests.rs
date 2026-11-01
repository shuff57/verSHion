//! Tests for `crondaily.rs`: erasure catch-ups, the ghost GC, purges (due, appealed, failing), cleanup, and the orchestrator on a small world.
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
fn erase(c: &Connection, u: &str, at: i64) {
    c.execute("UPDATE users SET deleted_at = ?2, email = NULL, pw_hash = NULL, pw_params = NULL, display_name = NULL, profile = NULL WHERE id = ?1", params![u, at]).unwrap();
}
fn item(c: &Connection, id: &str, author: &str) {
    c.execute("INSERT INTO items (id, author_id, kind, visibility, title, license, created_at, updated_at) VALUES (?1, ?2, 'bank', 'unlisted', 't', 'CC-BY-4.0', 1, 1)", params![id, author]).unwrap();
}
fn n(c: &Connection, sql: &str) -> i64 {
    c.query_row(sql, [], |r| r.get(0)).unwrap()
}

#[test]
fn an_erasure_that_never_committed_is_run_again_until_it_does() {
    let (c, _d, _s) = open();
    item(&c, "i1", "a");
    erase(&c, "a", 5);
    c.execute("DELETE FROM erasure_done", []).unwrap();
    assert_eq!(erasure_finish(&PoolCtx::new(&c, 1_000)).unwrap(), 1);
    assert_eq!(n(&c, "SELECT COUNT(*) FROM erasure_done WHERE user_id = 'a'"), 1);
    assert_eq!(c.query_row::<String, _, _>("SELECT removed_reason FROM items", [], |r| r.get(0)).unwrap(), "account_deleted");
    assert_eq!(erasure_finish(&PoolCtx::new(&c, 2_000)).unwrap(), 0);
}

#[test]
fn the_resweep_cleans_what_a_late_request_wrote_for_an_erased_account() {
    let (c, _d, _s) = open();
    erase(&c, "a", 5);
    c.execute("INSERT OR IGNORE INTO erasure_done (user_id, at) VALUES ('a', 5)", []).unwrap();
    c.execute("INSERT INTO pool_prefs (user_id, keep_on_delete, updated_at) VALUES ('a', 1, 9)", []).unwrap();
    c.execute("INSERT INTO pool_limits (user_id, metric, value, updated_at, updated_by) VALUES ('a', 'm', 1, 9, 't')", []).unwrap();
    c.execute("INSERT INTO reports (id, item_id, reporter_id, reason, note, at) VALUES ('r', 'x', 'a', 'other', 'late', 9)", []).unwrap();
    erasure_resweep(&PoolCtx::new(&c, 1_000)).unwrap();
    assert_eq!(n(&c, "SELECT COUNT(*) FROM pool_prefs") + n(&c, "SELECT COUNT(*) FROM pool_limits"), 0);
    assert_eq!((n(&c, "SELECT note IS NULL FROM reports"), n(&c, "SELECT reporter_id IS NULL FROM reports")), (1, 1));
    // Running it again changes nothing and does not fail.
    erasure_resweep(&PoolCtx::new(&c, 2_000)).unwrap();
}

#[test]
fn an_item_left_live_by_an_erased_author_is_removed_once_the_erasure_committed() {
    let (c, _d, _s) = open();
    item(&c, "i1", "a");
    item(&c, "i2", "b");
    erase(&c, "a", 5);
    c.execute("DELETE FROM erasure_done", []).unwrap();
    assert_eq!(account_deleted_catchup(&PoolCtx::new(&c, 1_000)).unwrap(), 0, "not before the erasure batch committed");
    c.execute("INSERT INTO erasure_done (user_id, at) VALUES ('a', 5)", []).unwrap();
    assert_eq!(account_deleted_catchup(&PoolCtx::new(&c, 1_000)).unwrap(), 1);
    assert_eq!(n(&c, "SELECT purge_after FROM items WHERE id = 'i1'"), 1_000 + 14 * DAY_MS);
    assert_eq!(n(&c, "SELECT removed_at IS NULL FROM items WHERE id = 'i2'"), 1);
}

#[test]
fn a_ghost_goes_only_when_no_serve_references_it() {
    let (c, _d, _s) = open();
    for p in ["g1", "g2"] {
        c.execute("INSERT INTO ghost_publishes (publish_id, item_id, author_id, hide_key, public_version, license, at) VALUES (?1, 'it', 'a', 0, 1, 'CC-BY-4.0', 1)", [p]).unwrap();
    }
    let served = c.execute("INSERT INTO served (item_id, publish_id, reader_id, kind, license, first_at, at, licenses_json) VALUES ('it', 'g2', 'b', 'view', 'CC-BY-4.0', 1, 1, '[]')", []);
    assert_eq!(served.unwrap(), 1, "fixture: a serve of g2");
    ghost_gc(&PoolCtx::new(&c, 5), GHOST_GC_ROWS).unwrap();
    let left: Vec<String> = c.prepare("SELECT publish_id FROM ghost_publishes").unwrap().query_map([], |r| r.get(0)).unwrap().map(|x| x.unwrap()).collect();
    assert_eq!(left, vec!["g2".to_string()]);
}

#[test]
fn due_removals_are_purged_but_an_open_appeal_waits() {
    let (c, _d, s) = open();
    for id in ["due", "appealed", "notyet"] {
        item(&c, id, "a");
    }
    c.execute("UPDATE items SET removed_at = 5, removed_reason = 'admin', purge_after = 100 WHERE id IN ('due', 'appealed')", []).unwrap();
    c.execute("UPDATE items SET removed_at = 5, removed_reason = 'admin', purge_after = 9999 WHERE id = 'notyet'", []).unwrap();
    c.execute("INSERT INTO admin_flags (id, kind, item_id, dedup, at) VALUES ('f', 'appeal', 'appealed', 'd', 1)", []).unwrap();
    assert_eq!(purges(&PoolCtx::new(&c, 500), &s, PURGE_BATCHES_MAX, PURGE_TIME_BUDGET).unwrap(), 1);
    assert_eq!(n(&c, "SELECT purged_at FROM items WHERE id = 'due'"), 500);
    assert_eq!(n(&c, "SELECT purged_at IS NULL FROM items WHERE id = 'appealed'"), 1);
    assert_eq!(n(&c, "SELECT purged_at IS NULL FROM items WHERE id = 'notyet'"), 1);
}

#[test]
fn an_exhausted_budget_purges_nothing() {
    let (c, _d, s) = open();
    item(&c, "due", "a");
    c.execute("UPDATE items SET removed_at = 5, removed_reason = 'admin', purge_after = 100", []).unwrap();
    assert_eq!(purges(&PoolCtx::new(&c, 500), &s, 0, PURGE_TIME_BUDGET).unwrap(), 0);
    assert_eq!(purges(&PoolCtx::new(&c, 500), &s, 5, Duration::ZERO).unwrap(), 0);
    assert_eq!(n(&c, "SELECT purged_at IS NULL FROM items"), 1);
}

#[test]
fn cleanup_drops_old_counters_and_used_preview_tokens() {
    let (c, _d, _s) = open();
    let now = 100 * DAY_MS;
    c.execute("INSERT INTO usage_daily (user_id, day, metric, count) VALUES ('a', 1, 'x', 1), ('a', 100, 'x', 1)", []).unwrap();
    c.execute("INSERT INTO preview_tokens_used (token_hash, at) VALUES ('old', ?1), ('new', ?2)", params![now - PREVIEW_MS - 1, now]).unwrap();
    cleanup(&PoolCtx::new(&c, now)).unwrap();
    assert_eq!(n(&c, "SELECT COUNT(*) FROM usage_daily"), 1);
    assert_eq!(c.query_row::<String, _, _>("SELECT token_hash FROM preview_tokens_used", [], |r| r.get(0)).unwrap(), "new");
}

#[test]
fn the_daily_run_orders_the_stages_and_reports_counts() {
    let (c, _d, s) = open();
    item(&c, "due", "a");
    c.execute("UPDATE items SET removed_at = 5, removed_reason = 'admin', purge_after = 100", []).unwrap();
    let rep = daily(&PoolCtx::new(&c, 500), &s);
    assert_eq!(rep, DailyReport { purged: 1, restored: 0, quarantine_expired: 0 });
    assert_eq!(n(&c, "SELECT COUNT(*) FROM admin_flags WHERE kind = 'cron_failure'"), 0, "no stage failed");
}
