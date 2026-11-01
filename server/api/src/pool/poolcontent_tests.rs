//! Tests for `poolcontent.rs`: account deletion keeps or removes items by the author's choice, scrubs what the account wrote, and is idempotent; prefs, standing
//! and the pending-path gates.
use super::*;
use crate::db;
use std::path::Path;

fn open() -> (Connection, tempfile::TempDir) {
    let dir = tempfile::tempdir().unwrap();
    let mut c = db::open(&dir.path().join("t.db")).unwrap();
    db::migrate(&mut c, &Path::new(env!("CARGO_MANIFEST_DIR")).join("../worker/migrations")).unwrap();
    for u in ["u1", "u2"] {
        c.execute("INSERT INTO users (id, email, display_name, trust, created_at) VALUES (?1, ?2, ?1, 'teacher', 1)", params![u, format!("{u}@x.org")]).unwrap();
    }
    (c, dir)
}
fn item(c: &Connection, id: &str, author: &str) {
    c.execute("INSERT INTO items (id, author_id, kind, visibility, title, license, created_at, updated_at) VALUES (?1, ?2, 'bank', 'public', 't', 'CC-BY-4.0', 1, 1)", params![id, author]).unwrap();
}
/// The auth package tombstones the account, then runs the content hook in the same transaction.
fn del(c: &Connection, user: &str, now: i64) -> rusqlite::Result<()> {
    c.execute("UPDATE users SET deleted_at = ?2, email = NULL, pw_hash = NULL, pw_params = NULL, display_name = NULL, profile = NULL WHERE id = ?1", params![user, now])?;
    delete_content(c, user, now)
}
fn n(c: &Connection, sql: &str) -> i64 {
    c.query_row(sql, [], |r| r.get(0)).unwrap()
}

#[test]
fn with_no_choice_only_items_others_imported_are_kept() {
    let (c, _d) = open();
    item(&c, "kept", "u1");
    item(&c, "gone", "u1");
    c.execute("INSERT INTO imports (user_id, item_id, publish_id, version, form, license, author_id, at, kind) VALUES ('u2', 'kept', 'p', 1, 'full', 'CC-BY-4.0', 'u1', 5, 'import')", []).unwrap();
    del(&c, "u1", 1_000).unwrap();
    let (anon, removed): (i64, Option<String>) = c.query_row("SELECT anonymized, removed_reason FROM items WHERE id = 'kept'", [], |r| Ok((r.get(0)?, r.get(1)?))).unwrap();
    assert_eq!((anon, removed), (1, None));
    let (reason, purge): (String, i64) = c.query_row("SELECT removed_reason, purge_after FROM items WHERE id = 'gone'", [], |r| Ok((r.get(0)?, r.get(1)?))).unwrap();
    assert_eq!((reason.as_str(), purge), ("account_deleted", 1_000 + 14 * DAY_MS));
    assert_eq!(n(&c, "SELECT COUNT(*) FROM erasure_done WHERE user_id = 'u1'"), 1);
    assert_eq!(n(&c, "SELECT COUNT(*) FROM pool_audit WHERE action = 'account_deleted.content_removed'"), 1);
}
#[test]
fn an_explicit_choice_overrides_the_default_both_ways() {
    let (c, _d) = open();
    item(&c, "a", "u1");
    c.execute("INSERT INTO pool_prefs (user_id, keep_on_delete, updated_at) VALUES ('u1', 1, 1)", []).unwrap();
    del(&c, "u1", 1_000).unwrap();
    assert_eq!(n(&c, "SELECT anonymized FROM items WHERE id = 'a'"), 1);
    assert_eq!(n(&c, "SELECT removed_at IS NULL FROM items WHERE id = 'a'"), 1);
    assert_eq!(n(&c, "SELECT COUNT(*) FROM pool_prefs"), 0, "the preference goes with the account");
    let (c, _d) = open();
    item(&c, "b", "u1");
    c.execute("INSERT INTO pool_prefs (user_id, keep_on_delete, updated_at) VALUES ('u1', 0, 1)", []).unwrap();
    del(&c, "u1", 1_000).unwrap();
    assert_eq!(c.query_row::<String, _, _>("SELECT removed_reason FROM items", [], |r| r.get(0)).unwrap(), "account_deleted");
}

#[test]
fn an_item_already_removed_follows_the_purge_date_and_what_the_account_wrote_is_scrubbed() {
    let (c, _d) = open();
    item(&c, "r", "u1");
    c.execute("UPDATE items SET removed_at = 5, removed_reason = 'admin', purge_after = ?1 WHERE id = 'r'", [1_000 + 30 * DAY_MS]).unwrap();
    c.execute("INSERT INTO reports (id, item_id, reporter_id, reason, note, at) VALUES ('rp', 'r', 'u1', 'other', 'my words', 5)", []).unwrap();
    c.execute("INSERT INTO usage_daily (user_id, day, metric, count) VALUES ('u1', 1, 'x', 1)", []).unwrap();
    c.execute("INSERT INTO pool_audit (id, at, actor_id, actor_kind, action, note) VALUES ('au', 1, 'u1', 'user', 'item.undelete', 'typed')", []).unwrap();
    c.execute("INSERT INTO admin_flags (id, kind, item_id, user_id, dedup, at, detail) VALUES ('f', 'appeal', 'r', 'u1', 'd', 1, '{\"note\":\"please\",\"x\":1}')", []).unwrap();
    del(&c, "u1", 1_000).unwrap();
    assert_eq!(n(&c, "SELECT purge_after FROM items WHERE id = 'r'"), 1_000 + 14 * DAY_MS);
    assert_eq!((n(&c, "SELECT note IS NULL FROM reports"), n(&c, "SELECT reporter_id IS NULL FROM reports")), (1, 1));
    assert_eq!(n(&c, "SELECT note IS NULL FROM pool_audit WHERE id = 'au'"), 1);
    assert_eq!(c.query_row::<String, _, _>("SELECT detail FROM admin_flags WHERE id = 'f'", [], |r| r.get(0)).unwrap(), "{\"x\":1,\"outcome\":\"author_erased\"}");
    assert_eq!(n(&c, "SELECT resolved_at FROM admin_flags WHERE id = 'f'"), 1_000, "the open appeal closes");
    assert_eq!(n(&c, "SELECT COUNT(*) FROM deletion_rewrites WHERE user_id = 'u1'"), 2);
}

#[test]
fn deleting_content_twice_is_harmless() {
    let (c, _d) = open();
    item(&c, "g", "u1");
    del(&c, "u1", 1_000).unwrap();
    del(&c, "u1", 2_000).unwrap();
    assert_eq!(n(&c, "SELECT purge_after FROM items"), 1_000 + 14 * DAY_MS, "the earlier date stands");
    assert_eq!(n(&c, "SELECT COUNT(*) FROM erasure_done"), 1);
}

#[test]
fn prefs_read_null_until_set_and_are_not_written_for_an_erased_account() {
    let (c, _d) = open();
    assert_eq!(get_prefs(&c, "u1").unwrap(), json!({ "keep_on_delete": null, "default": "keep_imported_delete_rest" }));
    let b = |v: Value| v.as_object().unwrap().clone();
    assert!(matches!(put_prefs(&c, "u1", 5, &b(json!({ "keep_on_delete": "yes" }))), Err(Fail::Pool(p)) if p.code == "invalid_request"));
    assert_eq!(put_prefs(&c, "u1", 5, &b(json!({ "keep_on_delete": false }))).unwrap(), json!({ "keep_on_delete": false }));
    assert_eq!(get_prefs(&c, "u1").unwrap()["keep_on_delete"], false);
    // An erased account's write is refused with the unauthenticated answer.
    c.execute("UPDATE users SET deleted_at = 9, email = NULL, pw_hash = NULL, pw_params = NULL, display_name = NULL, profile = NULL WHERE id = 'u2'", []).unwrap();
    assert!(matches!(put_prefs(&c, "u2", 5, &b(json!({ "keep_on_delete": true }))), Err(Fail::Pool(p)) if p.status == 401));
}

#[test]
fn standing_changes_write_only_an_audit_row() {
    let (c, _d) = open();
    standing_audit(&c, "u1", 7, "author_suspended").unwrap();
    assert_eq!(n(&c, "SELECT COUNT(*) FROM pool_audit WHERE action = 'author_suspended' AND target_user_id = 'u1' AND actor_kind = 'system'"), 1);
}

#[test]
fn a_pending_account_reaches_only_circle_paths_and_the_image_upload() {
    assert!(pending_circle_path("GET", "/circles"));
    assert!(!pending_circle_path("POST", "/circles"));
    assert!(pending_circle_path("POST", "/circles/join"));
    assert!(pending_circle_path("GET", "/circles/join/preview"));
    assert!(pending_circle_path("GET", "/circles/heads"));
    let cid = "0123456789abcdef0123456789abcdef";
    assert!(pending_circle_path("POST", &format!("/circles/{cid}")));
    assert!(pending_circle_path("GET", &format!("/circles/{cid}/banks/x")));
    assert!(!pending_circle_path("GET", &format!("/circles/{cid}x")));
    assert!(!pending_circle_path("GET", "/items"));
    let sha = "a".repeat(64);
    assert!(pending_blob_path("PUT", &format!("/blobs/{sha}")));
    assert!(!pending_blob_path("GET", &format!("/blobs/{sha}")));
    assert!(!pending_blob_path("PUT", "/blobs/short"));
    assert!(pool_trust("admin") && pool_trust("teacher") && !pool_trust("pending"));
}

#[test]
fn only_an_undecided_latest_request_is_awaiting_a_first_decision() {
    let (c, _d) = open();
    assert!(awaiting_first_decision(&c, "u1").unwrap(), "no row: not decided");
    c.execute("INSERT INTO approval_requests (id, user_id, requested_at, decided_at, decision) VALUES ('r1', 'u1', 1, 5, 'rejected')", []).unwrap();
    assert!(!awaiting_first_decision(&c, "u1").unwrap());
    c.execute("INSERT INTO approval_requests (id, user_id, requested_at) VALUES ('r2', 'u1', 10)", []).unwrap();
    assert!(awaiting_first_decision(&c, "u1").unwrap(), "a re-application waits again");
}
