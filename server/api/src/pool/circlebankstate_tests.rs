//! Tests for `circlebankstate.rs` on the real migrated schema: removing, restoring and reporting a bank.
use super::*;
use crate::db;
use crate::pool::circles::create_circle;
use crate::pool::ctx::Fail;
use std::path::Path;

const NOW: i64 = 10 * DAY_MS;

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
fn m(v: Value) -> Map<String, Value> {
    v.as_object().unwrap().clone()
}
fn join(c: &rusqlite::Connection, cid: &str, u: &str, role: &str, seq: i64) {
    c.execute("INSERT INTO circle_members (circle_id, user_id, role, joined_at, join_seq) VALUES (?1, ?2, ?3, 10, ?4)", params![cid, u, role, seq]).unwrap();
}
fn world() -> (rusqlite::Connection, tempfile::TempDir, String, String) {
    let (c, d) = open();
    for u in ["a", "b", "c", "d", "z"] {
        user(&c, u);
    }
    let cid = create_circle(&PoolCtx::new(&c, 1_000), "a", &m(json!({ "name": "Team" }))).unwrap().1["circle"]["id"].as_str().unwrap().to_string();
    join(&c, &cid, "b", "editor", 5);
    join(&c, &cid, "c", "viewer", 6);
    join(&c, &cid, "d", "viewer", 7);
    let bid = format!("{:032x}", 1);
    c.execute("INSERT INTO circle_banks (id, circle_id, author_id, title, license, head_version, head_version_id, created_at, updated_at) VALUES (?1, ?2, 'b', 'Quiz', 'circle-only', 1, 'v1', 1, 77)", params![bid, cid]).unwrap();
    (c, d, cid, bid)
}
fn removal(c: &rusqlite::Connection) -> (Option<i64>, Option<String>, Option<i64>) {
    c.query_row("SELECT removed_at, removed_reason, purge_after FROM circle_banks", [], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?))).unwrap()
}
fn audits(c: &rusqlite::Connection, action: &str) -> i64 {
    c.query_row("SELECT COUNT(*) FROM pool_audit WHERE action = ?1", [action], |r| r.get(0)).unwrap()
}

#[test]
fn the_author_or_owner_removes_a_bank_with_fourteen_days_to_undo() {
    let (c, _d, cid, bid) = world();
    let x = PoolCtx::new(&c, NOW);
    assert_eq!(err(delete_bank(&x, "c", &cid, &bid).unwrap_err()).1, "circle_role_required");
    assert_eq!(delete_bank(&x, "b", &cid, &bid).unwrap(), json!({ "ok": true, "undo_until": NOW + CIRCLE_UNDO_MS }));
    assert_eq!(removal(&c), (Some(NOW), Some("author_removed".into()), Some(NOW + CIRCLE_UNDO_MS)));
    assert_eq!(audits(&c, "circle.bank_remove"), 1);
    // Again: the same answer, nothing new written.
    let later = PoolCtx::new(&c, NOW + 5);
    assert_eq!(delete_bank(&later, "b", &cid, &bid).unwrap()["undo_until"], json!(NOW + CIRCLE_UNDO_MS));
    assert_eq!(audits(&c, "circle.bank_remove"), 1);
    // The owner's delete takes the removal over (the author can no longer undo it); the window is not extended.
    delete_bank(&later, "a", &cid, &bid).unwrap();
    assert_eq!(removal(&c), (Some(NOW), Some("owner_removed".into()), Some(NOW + CIRCLE_UNDO_MS)));
    assert_eq!(audits(&c, "circle.bank_remove"), 2);
    assert_eq!(err(undelete_bank(&later, "b", &cid, &bid).unwrap_err()).1, "circle_role_required");
}

#[test]
fn an_owner_removal_and_an_admin_removal_differ() {
    let (c, _d, cid, bid) = world();
    let x = PoolCtx::new(&c, NOW);
    delete_bank(&x, "a", &cid, &bid).unwrap();
    assert_eq!(removal(&c).1.as_deref(), Some("owner_removed"));
    c.execute("UPDATE circle_banks SET removed_reason = 'admin'", []).unwrap();
    assert_eq!(err(delete_bank(&x, "a", &cid, &bid).unwrap_err()).1, "under_moderation");
    assert_eq!(err(undelete_bank(&x, "a", &cid, &bid).unwrap_err()).1, "bank_removed");
}

#[test]
fn undelete_restores_within_the_window_for_the_right_person() {
    let (c, _d, cid, bid) = world();
    let x = PoolCtx::new(&c, NOW);
    assert_eq!(undelete_bank(&x, "b", &cid, &bid).unwrap(), json!({ "ok": true, "already": true }));
    delete_bank(&x, "b", &cid, &bid).unwrap();
    assert_eq!(err(undelete_bank(&x, "c", &cid, &bid).unwrap_err()).1, "circle_role_required");
    assert_eq!(undelete_bank(&x, "b", &cid, &bid).unwrap(), json!({ "ok": true }));
    assert_eq!(removal(&c), (None, None, None));
    assert_eq!(audits(&c, "circle.bank_undelete"), 1);
    // The owner may undo the author's removal; past the window nobody can.
    delete_bank(&x, "b", &cid, &bid).unwrap();
    assert!(undelete_bank(&x, "a", &cid, &bid).is_ok());
    delete_bank(&x, "b", &cid, &bid).unwrap();
    let late = PoolCtx::new(&c, NOW + CIRCLE_UNDO_MS + 1);
    let (s, code, _) = err(undelete_bank(&late, "a", &cid, &bid).unwrap_err());
    assert_eq!((s, code.as_str()), (409, "bank_removed"));
    // Not in a closed circle.
    c.execute("UPDATE circles SET closed_at = 1, close_reason = 'owner_closed'", []).unwrap();
    assert!(undelete_bank(&x, "a", &cid, &bid).is_err());
}

#[test]
fn a_report_is_validated_and_always_thanked_and_does_not_hold_alone() {
    let (c, _d, cid, bid) = world();
    let x = PoolCtx::new(&c, NOW);
    for body in [json!({}), json!({ "reason": "spam" }), json!({ "reason": 1 })] {
        assert_eq!(err(report_bank(&x, "c", &cid, &bid, &m(body)).unwrap_err()).2["field"], json!("reason"));
    }
    assert_eq!(err(report_bank(&x, "c", &cid, &bid, &m(json!({ "reason": "other", "note": 5 }))).unwrap_err()).2["field"], json!("note"));
    assert_eq!(err(report_bank(&x, "b", &cid, &bid, &m(json!({ "reason": "other" }))).unwrap_err()).1, "own_item");
    assert_eq!(err(report_bank(&x, "z", &cid, &bid, &m(json!({ "reason": "other" }))).unwrap_err()).1, "not_found");
    let (s, v) = report_bank(&x, "c", &cid, &bid, &m(json!({ "reason": "other", "note": "hm" }))).unwrap();
    assert_eq!((s, v["ok"].clone()), (202, json!(true)));
    let holds: i64 = c.query_row("SELECT COUNT(*) FROM circle_holds", [], |r| r.get(0)).unwrap();
    assert_eq!((holds, audits(&c, "circle.report")), (0, 1), "one report is below the threshold");
    // The same member's same report again changes nothing.
    report_bank(&x, "c", &cid, &bid, &m(json!({ "reason": "other" }))).unwrap();
    let n: i64 = c.query_row("SELECT COUNT(*) FROM circle_reports", [], |r| r.get(0)).unwrap();
    assert_eq!((n, audits(&c, "circle.report")), (1, 1));
    // The admin is told once, with no text.
    let notices = x.take_notices();
    assert!(notices.iter().all(|n| !format!("{n:?}").contains("hm")));
}

#[test]
fn enough_distinct_reports_hold_the_bank_and_the_second_reader_is_refused() {
    let (c, _d, cid, bid) = world();
    let x = PoolCtx::new(&c, NOW);
    let threshold = x.limit_of("*", "report_threshold").unwrap();
    let reporters = ["c", "d", "a"];
    for (i, who) in reporters.iter().enumerate().take(threshold as usize) {
        report_bank(&x, who, &cid, &bid, &m(json!({ "reason": "inappropriate" }))).unwrap();
        let holds: i64 = c.query_row("SELECT COUNT(*) FROM circle_holds WHERE reason = 'reports'", [], |r| r.get(0)).unwrap();
        assert_eq!(holds, if i + 1 == threshold as usize { 1 } else { 0 }, "report {}", i + 1);
    }
    // Held: a viewer can no longer read, so cannot report either.
    assert_eq!(err(report_bank(&x, "d", &cid, &bid, &m(json!({ "reason": "copyright" }))).unwrap_err()).1, "not_found");
}

#[test]
fn an_established_members_student_data_report_holds_at_once_a_new_ones_does_not() {
    let (c, _d, cid, bid) = world();
    let x = PoolCtx::new(&c, NOW);
    // d joined at 10: not established in this circle for a day... NOW is far past, so established; make d new instead.
    c.execute("UPDATE circle_members SET joined_at = ?1 WHERE user_id = 'd'", [NOW - 1000]).unwrap();
    let (s, _) = report_bank(&x, "d", &cid, &bid, &m(json!({ "reason": "student_data" }))).unwrap();
    assert_eq!(s, 202);
    let holds: i64 = c.query_row("SELECT COUNT(*) FROM circle_holds", [], |r| r.get(0)).unwrap();
    assert_eq!(holds, 0, "a member of under a day does not hide the bank alone");
    report_bank(&x, "c", &cid, &bid, &m(json!({ "reason": "student_data" }))).unwrap();
    let h: (String, String) = c.query_row("SELECT reason, source FROM circle_holds", [], |r| Ok((r.get(0)?, r.get(1)?))).unwrap();
    assert_eq!(h, ("student_data".into(), "".into()));
    let note = x.take_notices();
    assert!(note.iter().any(|n| matches!(n, PoolNotice::CircleReport { held: true, .. })));
    assert!(note.iter().any(|n| matches!(n, PoolNotice::CircleReport { held: false, .. })));
}
