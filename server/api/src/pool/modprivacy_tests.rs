//! Tests for `modprivacy.rs`: a privacy purge quarantines, blocks, holds the previewed copies and is restorable; admin removals and appeals are decided once.
use super::*;
use crate::db;
use crate::pool::blocking::{Affected, BlobPlan};
use rusqlite::Connection;
use std::path::Path;

const ADMIN: Actor = Actor { id: Some("adm"), kind: "admin" };

fn world() -> (Connection, tempfile::TempDir, BlobStore) {
    let d = tempfile::tempdir().unwrap();
    let mut c = db::open(&d.path().join("t.db")).unwrap();
    db::migrate(&mut c, &Path::new(env!("CARGO_MANIFEST_DIR")).join("../worker/migrations")).unwrap();
    for u in ["a", "b"] {
        c.execute("INSERT INTO users (id, email, display_name, trust, created_at) VALUES (?1, ?2, ?3, 'teacher', 1)", params![u, format!("{u}@x.org"), u]).unwrap();
    }
    let s = BlobStore::new(d.path().join("blobs")).unwrap();
    (c, d, s)
}
fn item(c: &Connection, id: &str, author: &str, publish: &str) {
    c.execute("INSERT INTO items (id, author_id, kind, visibility, current_publish_id, license, title, created_at, updated_at) VALUES (?1, ?2, 'bank', 'public', ?3, 'cc-by', ?1, 1, 1)", params![id, author, publish]).unwrap();
    c.execute(
        "INSERT INTO item_publishes (publish_id, item_id, author_id, version, status, schema_version, hide_key, license, attested_at, attest_text_rev, acks_json, request_sha256, stage_token,
           full_key, full_bytes, title, subject, course, grade, kinds_mask, question_count, has_images, created_at, committed_at)
         VALUES (?1, ?2, ?3, 1, 'current', 1, 0, 'cc-by', 1, 'r', '{}', 'x', 't', ?4, 1, 'T', '', '', '', 0, 0, 0, 1, 1)",
        params![publish, id, author, format!("items/{id}/{publish}.json")],
    )
    .unwrap();
}
fn n(c: &Connection, sql: &str) -> i64 {
    c.query_row(sql, [], |r| r.get(0)).unwrap()
}
fn plan(affected: &[&str]) -> BlockPlan {
    BlockPlan {
        item_id: "i1".into(),
        blob: vec![BlobPlan { sha256: "abc".into(), other_authors: 0, introduced: true, selected: true, suggested: None }],
        affected: affected.iter().map(|i| Affected { item_id: i.to_string(), author_id: "b".into(), title: "t".into(), via: "image".into() }).collect(),
        affected_total: affected.len() as i64,
        ..Default::default()
    }
}

#[test]
fn a_privacy_purge_quarantines_blocks_and_holds_the_previewed_copies() {
    let (c, _d, s) = world();
    item(&c, "i1", "a", "p1");
    item(&c, "cp", "b", "pc");
    s.put("items/i1/p1.json", b"body").unwrap();
    assert!(privacy_purge(&PoolCtx::new(&c, 1_000), &s, "adm", "i1", &plan(&["cp"])).unwrap());
    let (reason, purge): (String, i64) = c.query_row("SELECT removed_reason, purge_after FROM items WHERE id = 'i1'", [], |r| Ok((r.get(0)?, r.get(1)?))).unwrap();
    assert_eq!((reason.as_str(), purge), ("privacy", 1_000 + PRIVACY_BYTES_GRACE_MS));
    assert_eq!(n(&c, "SELECT COUNT(*) FROM privacy_blocks WHERE kind = 'blob' AND hash = 'abc'"), 1);
    assert_eq!(n(&c, "SELECT COUNT(*) FROM item_holds WHERE item_id = 'cp' AND source = 'privacy'"), 1);
    assert_eq!(n(&c, "SELECT COUNT(*) FROM item_holds WHERE item_id = 'i1'"), 0);
    assert!(c.query_row::<String, _, _>("SELECT full_key FROM item_publishes WHERE publish_id = 'p1'", [], |r| r.get(0)).unwrap().starts_with("quarantine/"));
    assert_eq!(n(&c, "SELECT resolved_at FROM admin_flags WHERE kind = 'privacy_backlog'"), 1_000);
    assert_eq!(n(&c, "SELECT COUNT(*) FROM pool_audit WHERE action = 'privacy.purge'"), 1);
    // A second confirm is the admin retry of a purge: idempotent, still true (as the Worker answers 200).
    assert!(privacy_purge(&PoolCtx::new(&c, 2_000), &s, "adm", "i1", &plan(&["cp"])).unwrap());
    assert!(!privacy_purge(&PoolCtx::new(&c, 2_000), &s, "adm", "nope", &plan(&[])).unwrap());
}

#[test]
fn a_privacy_restore_within_the_window_puts_the_item_back() {
    let (c, _d, s) = world();
    item(&c, "i1", "a", "p1");
    s.put("items/i1/p1.json", b"body").unwrap();
    privacy_purge(&PoolCtx::new(&c, 1_000), &s, "adm", "i1", &plan(&[])).unwrap();
    assert!(!privacy_restore(&PoolCtx::new(&c, 1_000 + PRIVACY_BYTES_GRACE_MS + 1), &s, &ADMIN, "i1", None).unwrap(), "past the window with no appeal");
    assert!(privacy_restore(&PoolCtx::new(&c, 2_000), &s, &ADMIN, "i1", Some("unblocked")).unwrap());
    assert_eq!(n(&c, "SELECT removed_at IS NULL AND removed_reason IS NULL AND purge_after IS NULL FROM items"), 1);
    assert_eq!(n(&c, "SELECT COUNT(*) FROM privacy_blocks"), 0);
    assert!(c.query_row::<String, _, _>("SELECT full_key FROM item_publishes", [], |r| r.get(0)).unwrap().starts_with("items/i1/"));
}

#[test]
fn held_copies_are_previewed_then_removed_in_bulk() {
    let (c, _d, s) = world();
    item(&c, "i1", "a", "p1");
    item(&c, "cp", "b", "pc");
    privacy_purge(&PoolCtx::new(&c, 1_000), &s, "adm", "i1", &plan(&["cp"])).unwrap();
    c.execute("INSERT INTO item_blobs (publish_id, form, sha256) VALUES ('pc', 'full', 'abc')", []).unwrap();
    let pre = purge_held_copies(&PoolCtx::new(&c, 2_000), &s, "adm", "i1", false).unwrap();
    assert_eq!((pre.items.clone(), pre.total, pre.removed), (vec!["cp".to_string()], 1, 0));
    let done = purge_held_copies(&PoolCtx::new(&c, 2_000), &s, "adm", "i1", true).unwrap();
    assert_eq!((done.removed, done.remaining), (1, 0));
    assert_eq!(c.query_row::<String, _, _>("SELECT removed_reason FROM items WHERE id = 'cp'", [], |r| r.get(0)).unwrap(), "privacy");
    assert_eq!(n(&c, "SELECT COUNT(*) FROM pool_audit WHERE action = 'privacy.purge_copy'"), 1);
}

#[test]
fn an_admin_removal_is_reversed_to_live_or_refused_over_limits() {
    let (c, _d, _s) = world();
    item(&c, "i1", "a", "p1");
    c.execute("UPDATE items SET removed_at = 10, removed_reason = 'admin', purge_after = 5000, question_count = 3", []).unwrap();
    // Over the shared-question cap: refused.
    c.execute("INSERT INTO pool_limits (metric, user_id, value, updated_at, updated_by) VALUES ('shared_questions', 'a', 2, 1, 't')", []).unwrap();
    assert_eq!(admin_unremove(&PoolCtx::new(&c, 100), "i1", &ADMIN, None).unwrap(), Decided::OverLimits);
    c.execute("DELETE FROM pool_limits", []).unwrap();
    assert_eq!(admin_unremove(&PoolCtx::new(&c, 100), "i1", &ADMIN, Some("ok")).unwrap(), Decided::Ok);
    assert_eq!(n(&c, "SELECT removed_at IS NULL FROM items"), 1);
    assert_eq!(admin_unremove(&PoolCtx::new(&c, 100), "i1", &ADMIN, None).unwrap(), Decided::NotFound);
}

#[test]
fn deciding_an_appeal_resolves_it_and_blocks_a_second_appeal() {
    let (c, _d, s) = world();
    item(&c, "i1", "a", "p1");
    c.execute("UPDATE items SET removed_at = 10, removed_reason = 'admin', purge_after = 5000", []).unwrap();
    c.execute("INSERT INTO admin_flags (id, kind, item_id, user_id, dedup, at) VALUES ('f1', 'appeal', 'i1', 'a', 'appeal:i1:10', 20)", []).unwrap();
    assert_eq!(decide_appeal(&PoolCtx::new(&c, 100), &s, "i1", true, &ADMIN, None).unwrap(), Decided::Ok);
    assert_eq!(n(&c, "SELECT resolved_at FROM admin_flags WHERE id = 'f1'"), 100);
    assert_eq!(n(&c, "SELECT appeal_decided_at FROM items"), 100);
    assert_eq!(c.query_row::<String, _, _>("SELECT appeal_decision FROM items", [], |r| r.get(0)).unwrap(), "restore");
    assert_eq!(decide_appeal(&PoolCtx::new(&c, 100), &s, "nope", true, &ADMIN, None).unwrap(), Decided::NotFound);
}

#[test]
fn an_unanswered_appeal_is_reminded_then_restored_but_a_privacy_one_is_escalated() {
    let (c, _d, s) = world();
    item(&c, "pol", "a", "pp");
    item(&c, "priv", "b", "pv");
    c.execute("UPDATE items SET removed_at = 10, removed_reason = 'admin', purge_after = 99999999999 WHERE id = 'pol'", []).unwrap();
    c.execute("UPDATE items SET removed_at = 10, removed_reason = 'privacy', purge_after = 99999999999 WHERE id = 'priv'", []).unwrap();
    for (f, i) in [("f1", "pol"), ("f2", "priv")] {
        c.execute("INSERT INTO admin_flags (id, kind, item_id, user_id, dedup, at) VALUES (?1, 'appeal', ?2, 'a', ?3, 1000)", params![f, i, format!("appeal:{i}:10")]).unwrap();
    }
    // Eight days in: both reminded, nothing restored.
    let day8 = 1000 + 8 * DAY_MS;
    assert_eq!(appeals_due(&PoolCtx::new(&c, day8), &s).unwrap(), (2, 0));
    assert_eq!(n(&c, "SELECT COUNT(*) FROM admin_flags WHERE kind = 'appeal_overdue'"), 2);
    // Past 60 days: the policy removal is restored; the privacy one is escalated.
    let (_, restored) = appeals_due(&PoolCtx::new(&c, 1000 + APPEAL_MAX_MS + DAY_MS), &s).unwrap();
    assert_eq!(restored, 1);
    assert_eq!(n(&c, "SELECT removed_at IS NULL FROM items WHERE id = 'pol'"), 1);
    assert_eq!(n(&c, "SELECT COUNT(*) FROM admin_flags WHERE kind = 'appeal_escalated' AND item_id = 'priv'"), 1);
    assert_eq!(n(&c, "SELECT COUNT(*) FROM admin_flags WHERE kind = 'appeal_auto_restored'"), 1);
    assert_eq!(c.query_row::<String, _, _>("SELECT removed_reason FROM items WHERE id = 'priv'", [], |r| r.get(0)).unwrap(), "privacy");
}
