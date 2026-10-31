//! Tests for `circleerase.rs` on the real migrated schema: an account's erasure and the daily resweep for what a request in flight left.
use super::*;
use crate::db;
use crate::pool::circles::create_circle;
use crate::pool::ctx::PoolCtx;
use serde_json::{Map, Value, json};
use std::path::Path;

const NOW: i64 = 50_000_000;

fn open() -> (Connection, tempfile::TempDir) {
    let dir = tempfile::tempdir().unwrap();
    let mut conn = db::open(&dir.path().join("t.db")).unwrap();
    db::migrate(&mut conn, &Path::new(env!("CARGO_MANIFEST_DIR")).join("../worker/migrations")).unwrap();
    (conn, dir)
}
fn user(c: &Connection, u: &str) {
    c.execute("INSERT INTO users (id, email, display_name, trust, created_at) VALUES (?1, ?2, ?3, 'teacher', 1)", params![u, format!("{u}@x.org"), format!("Teacher {u}")]).unwrap();
}
fn count(c: &Connection, sql: &str) -> i64 {
    c.query_row(sql, [], |r| r.get(0)).unwrap()
}
fn role_of(c: &Connection, cid: &str, u: &str) -> Option<String> {
    c.query_row("SELECT role FROM circle_members WHERE circle_id = ?1 AND user_id = ?2 AND left_at IS NULL", params![cid, u], |r| r.get(0)).ok()
}
/// Circle owned by `a` with editor `b` and viewer `c`; `a` authored a bank, sent a version, filed a report, made a code and offered a transfer.
fn world() -> (Connection, tempfile::TempDir, String, String) {
    let (c, d) = open();
    for u in ["a", "b", "c"] {
        user(&c, u);
    }
    let body: Map<String, Value> = json!({ "name": "Team" }).as_object().unwrap().clone();
    let cid = create_circle(&PoolCtx::new(&c, 1_000), "a", &body).unwrap().1["circle"]["id"].as_str().unwrap().to_string();
    c.execute("INSERT INTO circle_members (circle_id, user_id, role, joined_at, join_seq) VALUES (?1, 'b', 'editor', 10, 5)", [&cid]).unwrap();
    c.execute("INSERT INTO circle_members (circle_id, user_id, role, joined_at, join_seq) VALUES (?1, 'c', 'viewer', 10, 6)", [&cid]).unwrap();
    let bid = format!("{:032x}", 1);
    c.execute("INSERT INTO circle_banks (id, circle_id, author_id, title, license, head_version, head_version_id, created_at, updated_at) VALUES (?1, ?2, 'a', 'Quiz', 'circle-only', 1, 'v1', 1, 77)", params![bid, cid]).unwrap();
    c.execute("INSERT INTO circle_versions (id, bank_id, circle_id, version, commit_id, request_sha256, committer_id, license, status, full_key, index_key, bytes, question_count, created_at, committed_at, message) VALUES ('v1', ?1, ?2, 1, 'c1', 'x', 'a', 'circle-only', 'current', 'k', 'i', 1, 1, 600, 600, 'my note')", params![bid, cid]).unwrap();
    c.execute("INSERT INTO circle_reports (id, circle_id, bank_id, reporter_id, reason, note, at) VALUES ('r1', ?1, ?2, 'a', 'other', 'secret words', 5)", params![cid, bid]).unwrap();
    c.execute("INSERT INTO circle_invites (id, circle_id, code_hash, role, created_by, created_at, expires_at, note) VALUES ('i1', ?1, 'h', 'viewer', 'a', 2, 99999999999, 'for Sam')", [&cid]).unwrap();
    c.execute("INSERT INTO circle_transfers (circle_id, from_user, to_user, created_at, expires_at) VALUES (?1, 'a', 'b', 1, 99999999999)", [&cid]).unwrap();
    c.execute("INSERT INTO pool_alert_budget (hour, channel, sent) VALUES (1, 'circle_email_inviter:a', 3)", []).unwrap();
    c.execute("INSERT INTO pool_alert_budget (hour, channel, sent) VALUES (1, 'circle_email_domain:abcdef0123456789:a', 3)", []).unwrap();
    c.execute("INSERT INTO pool_alert_budget (hour, channel, sent) VALUES (1, 'circle_email_inviter:b', 3)", []).unwrap();
    (c, d, cid, bid)
}

#[test]
fn erasing_the_owner_passes_the_circle_on_and_scrubs_what_names_them() {
    let (c, _d, cid, bid) = world();
    c.execute("INSERT INTO pool_prefs (user_id, keep_on_delete, updated_at) VALUES ('a', 1, 1)", []).unwrap();
    circle_erasure(&c, "a", NOW).unwrap();
    assert_eq!(role_of(&c, &cid, "b").as_deref(), Some("owner"), "the editor takes over");
    assert_eq!(count(&c, "SELECT COUNT(*) FROM circle_members WHERE user_id = 'a'"), 0);
    assert_eq!(count(&c, "SELECT COUNT(*) FROM pool_audit WHERE action = 'circle.member_erased' AND target_user_id = 'a'"), 1);
    assert_eq!(count(&c, "SELECT COUNT(*) FROM pool_audit WHERE action = 'circle.owner_succession'"), 1);
    // Kept shared items stay, shown by "deleted teacher".
    let b: (i64, Option<i64>) = c.query_row("SELECT anonymized, removed_at FROM circle_banks WHERE id = ?1", [&bid], |r| Ok((r.get(0)?, r.get(1)?))).unwrap();
    assert_eq!(b, (1, None));
    let msg: Option<String> = c.query_row("SELECT message FROM circle_versions", [], |r| r.get(0)).unwrap();
    assert_eq!(msg, None);
    let rep: (Option<String>, Option<String>) = c.query_row("SELECT note, reporter_id FROM circle_reports", [], |r| Ok((r.get(0)?, r.get(1)?))).unwrap();
    assert_eq!(rep, (None, None));
    let inv: (Option<i64>, Option<String>) = c.query_row("SELECT revoked_at, note FROM circle_invites", [], |r| Ok((r.get(0)?, r.get(1)?))).unwrap();
    assert_eq!(inv, (Some(NOW), None));
    assert_eq!(count(&c, "SELECT COUNT(*) FROM circle_transfers"), 0);
    let left: Vec<String> = c.prepare("SELECT channel FROM pool_alert_budget").unwrap().query_map([], |r| r.get(0)).unwrap().collect::<Result<_, _>>().unwrap();
    assert_eq!(left, vec!["circle_email_inviter:b".to_string()]);
}

#[test]
fn without_keep_the_authors_banks_are_removed_for_purge() {
    let (c, _d, cid, bid) = world();
    c.execute("INSERT INTO pool_prefs (user_id, keep_on_delete, updated_at) VALUES ('a', 0, 1)", []).unwrap();
    circle_erasure(&c, "a", NOW).unwrap();
    let b: (Option<i64>, Option<String>, Option<i64>, i64) = c.query_row("SELECT removed_at, removed_reason, purge_after, anonymized FROM circle_banks WHERE id = ?1", [&bid], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?))).unwrap();
    assert_eq!(b, (Some(NOW), Some("account_deleted".into()), Some(NOW + CIRCLE_UNDO_MS), 0));
    assert!(role_of(&c, &cid, "b").is_some());
}

#[test]
fn erasing_the_last_member_closes_the_circle_and_a_rerun_changes_nothing() {
    let (c, _d, cid, _bid) = world();
    for u in ["b", "c", "a"] {
        circle_erasure(&c, u, NOW).unwrap();
    }
    let ci: (Option<i64>, Option<String>, Option<i64>) = c.query_row("SELECT closed_at, close_reason, purge_after FROM circles WHERE id = ?1", [&cid], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?))).unwrap();
    assert_eq!(ci, (Some(NOW), Some("last_member_gone".into()), Some(NOW + CIRCLE_UNDO_MS)));
    let audits = count(&c, "SELECT COUNT(*) FROM pool_audit");
    circle_erasure(&c, "a", NOW + 10).unwrap();
    assert_eq!(count(&c, "SELECT COUNT(*) FROM pool_audit"), audits);
}

#[test]
fn the_resweep_cleans_what_an_erased_account_wrote_after_its_erasure_and_repairs_ownerless_circles() {
    let (c, _d, cid, bid) = world();
    // `a` is erased, but a request in flight left every trace behind.
    c.execute("INSERT INTO erasure_done (user_id, at) VALUES ('a', 1)", []).unwrap();
    circle_resweep(&c, NOW, &[]).unwrap();
    assert_eq!(role_of(&c, &cid, "b").as_deref(), Some("owner"));
    assert_eq!(count(&c, "SELECT COUNT(*) FROM circle_members WHERE user_id = 'a'"), 0);
    let b: i64 = c.query_row("SELECT anonymized FROM circle_banks WHERE id = ?1", [&bid], |r| r.get(0)).unwrap();
    assert_eq!(b, 1);
    assert_eq!(count(&c, "SELECT COUNT(*) FROM circle_versions WHERE message IS NOT NULL"), 0);
    assert_eq!(count(&c, "SELECT COUNT(*) FROM circle_reports WHERE note IS NOT NULL OR reporter_id IS NOT NULL"), 0);
    assert_eq!(count(&c, "SELECT COUNT(*) FROM circle_transfers"), 0);
    let left: Vec<String> = c.prepare("SELECT channel FROM pool_alert_budget").unwrap().query_map([], |r| r.get(0)).unwrap().collect::<Result<_, _>>().unwrap();
    assert_eq!(left, vec!["circle_email_inviter:b".to_string()]);
    // A rerun does nothing more.
    let audits = count(&c, "SELECT COUNT(*) FROM pool_audit");
    circle_resweep(&c, NOW + 5, &[]).unwrap();
    assert_eq!(count(&c, "SELECT COUNT(*) FROM pool_audit"), audits);
}

#[test]
fn ownerless_circles_are_found_and_handed_on() {
    let (c, _d, cid, _bid) = world();
    assert!(ownerless_circles(&c, NOW).unwrap().is_empty());
    // The owner's row is gone without a succession (a crashed batch).
    c.execute("UPDATE circle_members SET left_at = 1, left_reason = 'account_deleted' WHERE user_id = 'a'", []).unwrap();
    let ids = ownerless_circles(&c, NOW).unwrap();
    assert_eq!(ids, vec![cid.clone()]);
    circle_resweep(&c, NOW, &ids).unwrap();
    assert_eq!(role_of(&c, &cid, "b").as_deref(), Some("owner"));
    assert!(ownerless_circles(&c, NOW).unwrap().is_empty());
    // An owner-closed circle inside its undo window without an owner is found too.
    c.execute("UPDATE circle_members SET role = 'editor' WHERE user_id = 'b'", []).unwrap();
    c.execute("UPDATE circles SET closed_at = 5, close_reason = 'owner_closed', undo_until = ?1, purge_after = ?1", [NOW + 1000]).unwrap();
    assert_eq!(ownerless_circles(&c, NOW).unwrap(), vec![cid]);
}
