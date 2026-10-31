//! Tests for `recordserved.rs` on the real migrated schema: what a read or an import leaves behind, and when nothing is left.
use super::*;
use crate::db;
use crate::pool::itemview::load_visible;
use rusqlite::Connection;
use std::path::Path;

fn open() -> (Connection, tempfile::TempDir) {
    let dir = tempfile::tempdir().unwrap();
    let mut conn = db::open(&dir.path().join("t.db")).unwrap();
    db::migrate(&mut conn, &Path::new(env!("CARGO_MANIFEST_DIR")).join("../worker/migrations")).unwrap();
    (conn, dir)
}
fn user(c: &Connection, id: &str) {
    c.execute("INSERT INTO users (id, email, display_name, trust, created_at) VALUES (?1, ?2, ?1, 'teacher', 1)", params![id, format!("{id}@x.org")]).unwrap();
}
fn world() -> (Connection, tempfile::TempDir, ItemRow) {
    let (c, d) = open();
    user(&c, "a");
    user(&c, "r");
    let id = "1".repeat(32);
    c.execute(
        "INSERT INTO items (id, author_id, kind, visibility, current_publish_id, public_publish_id, public_version, version, title, subject, course, grade, kinds_mask, question_count,
            license, has_images, committed_at, created_at, updated_at) VALUES (?1, 'a', 'bank', 'public', 'pc', 'pp', 2, 3, 'T', '', '', '', 1, 1, 'CC-BY-4.0', 0, 100, 1, 1)",
        [&id],
    )
    .unwrap();
    for (p, v) in [("pc", 3), ("pp", 2)] {
        c.execute(
            "INSERT INTO item_publishes (publish_id, item_id, author_id, version, public_version, status, schema_version, hide_key, license, attested_at, attest_text_rev,
                acks_json, request_sha256, stage_token, full_key, stripped_key, full_bytes, title, subject, course, grade, kinds_mask, question_count, has_images, created_at)
             VALUES (?1, ?2, 'a', ?3, 2, 'current', 2, 0, 'CC-BY-4.0', 1, 'r', '{}', 'x', ?1, 'FK', 'SK', 10, '', '', '', '', 0, 1, 1, 1)",
            params![p, id, v],
        )
        .unwrap();
    }
    let row = load_visible(&c, &id, "r").unwrap().unwrap();
    (c, d, row)
}
fn count(c: &Connection, sql: &str) -> i64 {
    c.query_row(sql, [], |r| r.get(0)).unwrap()
}

#[test]
fn an_import_binds_the_reader_to_the_public_publish_and_the_real_one_it_was_served() {
    let (c, _d, r) = world();
    let x = PoolCtx::new(&c, 500);
    assert!(record_served(&x, "r", &r, Form::Full, ServeKind::Import).unwrap());
    let (pid, real, kind, lic): (String, String, String, String) =
        c.query_row("SELECT publish_id, real_publish_id, kind, licenses_json FROM imports WHERE user_id = 'r'", [], |q| Ok((q.get(0)?, q.get(1)?, q.get(2)?, q.get(3)?))).unwrap();
    assert_eq!((pid.as_str(), real.as_str(), kind.as_str(), lic.as_str()), ("pp", "pc", "import", r#"["CC-BY-4.0"]"#));
    let (spid, skind): (String, String) = c.query_row("SELECT publish_id, kind FROM served WHERE reader_id = 'r'", [], |q| Ok((q.get(0)?, q.get(1)?))).unwrap();
    assert_eq!((spid.as_str(), skind.as_str()), ("pc", "import"));
}

#[test]
fn a_view_after_an_import_never_downgrades_it_and_a_repeat_adds_no_row() {
    let (c, _d, r) = world();
    let x = PoolCtx::new(&c, 500);
    record_served(&x, "r", &r, Form::Full, ServeKind::Import).unwrap();
    let later = PoolCtx::new(&c, 900);
    record_served(&later, "r", &r, Form::Full, ServeKind::View).unwrap();
    assert_eq!(count(&c, "SELECT COUNT(*) FROM imports"), 1);
    assert_eq!(count(&c, "SELECT COUNT(*) FROM served"), 1);
    assert_eq!(c.query_row::<String, _, _>("SELECT kind FROM imports", [], |q| q.get(0)).unwrap(), "import");
    assert_eq!(c.query_row::<String, _, _>("SELECT kind FROM served", [], |q| q.get(0)).unwrap(), "import");
    assert_eq!(count(&c, "SELECT at FROM served"), 900);
    assert_eq!(count(&c, "SELECT first_at FROM served"), 500);
}

#[test]
fn a_takedown_or_hold_landing_after_the_body_was_read_records_nothing() {
    let (c, _d, r) = world();
    c.execute("INSERT INTO item_holds (item_id, reason, at) VALUES (?1, 'admin', 1)", [&r.id]).unwrap();
    let x = PoolCtx::new(&c, 500);
    assert!(!record_served(&x, "r", &r, Form::Full, ServeKind::Import).unwrap());
    assert_eq!(count(&c, "SELECT COUNT(*) FROM imports") + count(&c, "SELECT COUNT(*) FROM served"), 0);
}

#[test]
fn a_publish_pruned_during_the_read_leaves_no_serve_behind() {
    let (c, _d, r) = world();
    c.execute("UPDATE item_publishes SET status = 'staged' WHERE publish_id = 'pc'", []).unwrap();
    let x = PoolCtx::new(&c, 500);
    assert!(!record_served(&x, "r", &r, Form::Full, ServeKind::View).unwrap());
    assert_eq!(count(&c, "SELECT COUNT(*) FROM served"), 0);
}

#[test]
fn quotas_refuse_a_new_read_or_import_past_the_limit_but_not_a_repeat() {
    let (c, _d, r) = world();
    c.execute("INSERT INTO pool_limits (user_id, metric, value, updated_at, updated_by) VALUES ('r', 'reads_max', 0, 1, 'a')", []).unwrap();
    let x = PoolCtx::new(&c, 500);
    match record_served(&x, "r", &r, Form::Full, ServeKind::View) {
        Err(Fail::Pool(e)) => assert_eq!((e.status, e.code.as_str()), (413, "reads_quota")),
        other => panic!("{other:?}"),
    }
    // An import is never refused by the reads count.
    assert!(record_served(&x, "r", &r, Form::Full, ServeKind::Import).unwrap());
    // Already read in some version: a repeat read is no new read.
    assert!(record_served(&x, "r", &r, Form::Full, ServeKind::View).unwrap());
    c.execute("INSERT INTO pool_limits (user_id, metric, value, updated_at, updated_by) VALUES ('r2', 'imports_max', 0, 1, 'a')", []).unwrap();
    user(&c, "r2");
    match record_served(&x, "r2", &r, Form::Full, ServeKind::Import) {
        Err(Fail::Pool(e)) => assert_eq!((e.status, e.code.as_str()), (413, "imports_quota")),
        other => panic!("{other:?}"),
    }
}

#[test]
fn an_erased_reader_records_nothing_and_is_told_as_if_unauthenticated() {
    let (c, _d, r) = world();
    c.execute("UPDATE users SET deleted_at = 5, email = NULL, pw_hash = NULL, pw_params = NULL, display_name = NULL, profile = NULL WHERE id = 'r'", []).unwrap();
    let x = PoolCtx::new(&c, 500);
    match record_served(&x, "r", &r, Form::Full, ServeKind::Import) {
        Err(Fail::Pool(e)) => assert_eq!(e.status, 401),
        other => panic!("{other:?}"),
    }
    assert_eq!(count(&c, "SELECT COUNT(*) FROM imports"), 0);
}

#[test]
fn tags_and_standards_come_back_sorted() {
    let (c, _d, _r) = world();
    for t in ["zeta", "alpha"] {
        c.execute("INSERT INTO item_tags (publish_id, tag) VALUES ('pc', ?1)", [t]).unwrap();
    }
    c.execute("INSERT INTO item_standards (publish_id, code) VALUES ('pc', 'CCSS.8.EE')", []).unwrap();
    let x = PoolCtx::new(&c, 1);
    assert_eq!(tags_for(&x, "pc").unwrap(), (vec!["alpha".to_string(), "zeta".to_string()], vec!["CCSS.8.EE".to_string()]));
    assert_eq!(tags_for(&x, "none").unwrap(), (vec![], vec![]));
}

#[test]
fn imported_images_are_held_within_the_quota_and_not_beyond_it() {
    let (c, _d, _r) = world();
    let x = PoolCtx::new(&c, 1_000);
    assert_eq!(hold_imported_images(&x, "r", "pc", Form::Full).unwrap(), Held { held: 0, until: None, reason: None }, "no images, nothing to hold");
    c.execute("INSERT INTO blobs (sha256, size, content_type, width, height, first_uploaded_at) VALUES (?1, 400, 'image/png', 1, 1, 1)", ["a".repeat(64)]).unwrap_or_else(|e| panic!("{e}"));
    c.execute("INSERT INTO item_blobs (publish_id, form, sha256) VALUES ('pc', 'full', ?1)", ["a".repeat(64)]).unwrap();
    let h = hold_imported_images(&x, "r", "pc", Form::Full).unwrap();
    assert_eq!((h.held, h.until, h.reason), (1, Some(1_000 + IMPORT_IMAGE_HOLD_MS), None));
    assert_eq!(count(&c, "SELECT size FROM pending_blobs WHERE uploader_id = 'r'"), 400);
    // A tighter quota refuses to hold.
    user(&c, "r3");
    c.execute("INSERT INTO pool_limits (user_id, metric, value, updated_at, updated_by) VALUES ('r3', 'storage_bytes', 100, 1, 'a')", []).unwrap();
    let h = hold_imported_images(&x, "r3", "pc", Form::Full).unwrap();
    assert_eq!((h.held, h.reason), (0, Some("quota")));
}
