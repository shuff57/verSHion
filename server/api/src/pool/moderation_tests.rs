//! Tests for `moderation.rs` (the user-facing half), replaying the Worker's author-action, report, appeal and counter-notice cases.
use super::*;
use crate::db;
use rusqlite::Connection;
use std::path::Path;

const ID: &str = "11111111111111111111111111111111";

fn world() -> (Connection, tempfile::TempDir) {
    let d = tempfile::tempdir().unwrap();
    let mut c = db::open(&d.path().join("t.db")).unwrap();
    db::migrate(&mut c, &Path::new(env!("CARGO_MANIFEST_DIR")).join("../worker/migrations")).unwrap();
    for u in ["a", "b", "c", "d"] {
        c.execute("INSERT INTO users (id, email, display_name, trust, created_at) VALUES (?1, ?2, ?3, 'teacher', 1)", params![u, format!("{u}@x.org"), format!("T {u}")]).unwrap();
    }
    c.execute(
        "INSERT INTO items (id, author_id, kind, visibility, current_publish_id, license, question_count, created_at, updated_at) VALUES (?1, 'a', 'bank', 'public', 'p1', 'cc-by', 3, 1, 1)",
        [ID],
    )
    .unwrap();
    c.execute("INSERT INTO item_publishes (publish_id, item_id, status, hide_key, full_key, at) VALUES ('p1', ?1, 'current', 0, 'k1', 1)", [ID]).ok();
    (c, d)
}
fn n(c: &Connection, sql: &str) -> i64 {
    c.query_row(sql, [], |r| r.get(0)).unwrap()
}
fn err(r: Res<(u16, Value)>) -> (u16, String) {
    match r {
        Err(Fail::Pool(p)) => (p.status, p.code.clone()),
        o => panic!("{o:?}"),
    }
}
fn obj(v: Value) -> Map<String, Value> {
    v.as_object().unwrap().clone()
}

#[test]
fn visibility_flips_and_circle_only_cannot_go_public() {
    let (c, _d) = world();
    let x = PoolCtx::new(&c, 1_000);
    let r = patch_item(&x, "a", ID, &obj(json!({ "visibility": "unlisted" }))).unwrap();
    assert_eq!((r.0, r.1["visibility"].clone()), (200, json!("unlisted")));
    assert_eq!(err(patch_item(&x, "a", ID, &obj(json!({ "visibility": "private" })))), (400, "invalid_request".into()));
    c.execute("UPDATE items SET license = 'circle-only', visibility = 'unlisted'", []).unwrap();
    assert_eq!(err(patch_item(&x, "a", ID, &obj(json!({ "visibility": "public" })))), (422, "license_not_public".into()));
    // Not the author: a miss, counted.
    assert_eq!(err(patch_item(&x, "b", ID, &obj(json!({ "visibility": "public" })))).0, 404);
    assert_eq!(n(&c, "SELECT count FROM usage_daily WHERE user_id = 'b' AND metric = 'item_miss'"), 1);
    assert_eq!(n(&c, "SELECT COUNT(*) FROM pool_audit WHERE action = 'item.visibility'"), 1);
}

#[test]
fn delete_hides_for_14_days_and_undelete_restores() {
    let (c, _d) = world();
    let r = delete_item(&PoolCtx::new(&c, 1_000), "a", ID).unwrap();
    assert_eq!(r.1["undo_until"], json!(1_000 + UNDO_MS));
    // A repeat gives the same window, never extended.
    let again = delete_item(&PoolCtx::new(&c, 5_000), "a", ID).unwrap();
    assert_eq!(again.1["undo_until"], json!(1_000 + UNDO_MS));
    assert_eq!(undelete_item(&PoolCtx::new(&c, 6_000), "a", ID).unwrap().0, 200);
    assert_eq!(n(&c, "SELECT COUNT(*) FROM items WHERE removed_at IS NULL"), 1);
    // After the window, undelete is a miss.
    delete_item(&PoolCtx::new(&c, 7_000), "a", ID).unwrap();
    assert_eq!(err(undelete_item(&PoolCtx::new(&c, 7_000 + UNDO_MS + 1), "a", ID)).0, 404);
    // Moderation removals are not an author deletion.
    c.execute("UPDATE items SET removed_reason = 'admin'", []).unwrap();
    assert_eq!(err(delete_item(&PoolCtx::new(&c, 8_000), "a", ID)), (409, "under_moderation".into()));
}

#[test]
fn undelete_is_refused_past_the_shared_question_cap() {
    let (c, _d) = world();
    delete_item(&PoolCtx::new(&c, 1_000), "a", ID).unwrap();
    c.execute("INSERT INTO pool_limits (metric, user_id, value, updated_at, updated_by) VALUES ('shared_questions', 'a', 2, 1, 't')", []).unwrap();
    let e = undelete_item(&PoolCtx::new(&c, 2_000), "a", ID);
    assert_eq!(err(e), (422, "shared_questions_limit".into()));
}

#[test]
fn reports_are_thanked_and_three_teachers_hide_the_item() {
    let (c, _d) = world();
    let store = BlobStore::new(std::env::temp_dir().join("mod-t")).unwrap();
    let rep = |u: &str, reason: &str| report(&PoolCtx::new(&c, 10 * DAY_MS), &store, u, ID, &obj(json!({ "reason": reason, "note": "bad" })));
    assert_eq!(rep("b", "wrong_answer").unwrap().0, 202);
    assert_eq!(n(&c, "SELECT COUNT(*) FROM item_holds"), 0);
    rep("c", "other").unwrap();
    assert_eq!(n(&c, "SELECT COUNT(*) FROM item_holds"), 0);
    rep("d", "inappropriate").unwrap();
    assert_eq!(n(&c, "SELECT COUNT(*) FROM item_holds WHERE reason = 'reports'"), 1);
    assert_eq!(err(report(&PoolCtx::new(&c, 10 * DAY_MS), &store, "a", ID, &obj(json!({ "reason": "other" })))), (400, "own_item".into()));
    assert_eq!(err(rep("b", "nonsense")), (400, "invalid_request".into()));
}

#[test]
fn a_student_data_report_from_a_seasoned_reporter_holds_at_once() {
    let (c, _d) = world();
    let store = BlobStore::new(std::env::temp_dir().join("mod-t")).unwrap();
    c.execute("INSERT INTO items (id, author_id, kind, visibility, license, created_at, updated_at) VALUES ('22222222222222222222222222222222', 'b', 'bank', 'unlisted', 'cc-by', 1, 1)", []).unwrap();
    let r = report(&PoolCtx::new(&c, 10 * DAY_MS), &store, "b", ID, &obj(json!({ "reason": "student_data" }))).unwrap();
    assert_eq!(r.0, 202);
    assert_eq!(n(&c, "SELECT COUNT(*) FROM item_holds WHERE reason = 'student_data'"), 1);
    // A brand-new account only leaves a report for the admin.
    c.execute("UPDATE users SET created_at = ?1 WHERE id = 'c'", [10 * DAY_MS - 1000]).unwrap();
    c.execute("INSERT INTO items (id, author_id, kind, visibility, license, created_at, updated_at) VALUES ('33333333333333333333333333333333', 'c', 'bank', 'unlisted', 'cc-by', 1, 1)", []).unwrap();
    c.execute("DELETE FROM item_holds", []).unwrap();
    report(&PoolCtx::new(&c, 10 * DAY_MS), &store, "c", ID, &obj(json!({ "reason": "student_data" }))).unwrap();
    assert_eq!(n(&c, "SELECT COUNT(*) FROM item_holds"), 0);
    assert_eq!(n(&c, "SELECT COUNT(*) FROM reports WHERE reporter_id = 'c'"), 1);
}

#[test]
fn an_appeal_needs_a_removal_or_hold_and_is_once_per_instance() {
    let (c, _d) = world();
    let e = appeal(&PoolCtx::new(&c, 5_000), "a", ID, &obj(json!({ "note": "please" })));
    assert_eq!(err(e), (409, "nothing_to_appeal".into()));
    c.execute("UPDATE items SET removed_at = 1000, removed_reason = 'admin', purge_after = 99999999999", []).unwrap();
    assert_eq!(err(appeal(&PoolCtx::new(&c, 5_000), "a", ID, &obj(json!({})))), (400, "invalid_request".into()));
    let r = appeal(&PoolCtx::new(&c, 5_000), "a", ID, &obj(json!({ "note": "please" }))).unwrap();
    assert_eq!(r.0, 202);
    let again = appeal(&PoolCtx::new(&c, 5_100), "a", ID, &obj(json!({ "note": "again" }))).unwrap();
    assert_eq!(again.1["message"], json!("Your appeal is already with the admin."));
    assert_eq!(n(&c, "SELECT COUNT(*) FROM admin_flags WHERE kind = 'appeal'"), 1);
    c.execute("UPDATE admin_flags SET resolved_at = 6000", []).unwrap();
    c.execute("UPDATE items SET appeal_decided_at = 6000", []).unwrap();
    assert_eq!(err(appeal(&PoolCtx::new(&c, 7_000), "a", ID, &obj(json!({ "note": "third" })))), (409, "decided".into()));
}

fn takedown(c: &Connection, notified: i64) {
    c.execute("UPDATE items SET removed_at = ?1, removed_reason = 'takedown', purge_after = 99999999999", [notified]).unwrap();
    c.execute("INSERT INTO dmca_notices (id, received_at, complainant_name, complainant_contact, work_desc, material_desc, has_signature, has_good_faith, has_perjury_stmt, status, recorded_by) VALUES ('n1', 1, 'C', 'c@x.org', 'w', 'm', 1, 1, 1, 'actioned', 'admin')", []).unwrap();
    c.execute("INSERT INTO dmca_actions (id, notice_id, item_id, author_id, disabled_at, author_notified_at) VALUES ('act1', 'n1', ?1, 'a', ?2, ?2)", params![ID, notified]).unwrap();
}
fn full_notice() -> Map<String, Value> {
    obj(json!({ "signature": "A Teacher", "material_desc": "my own bank", "name": "A Teacher", "address": "1 Main St", "phone": "555", "perjury_stmt": true, "consent_jurisdiction": true, "accept_service": true }))
}

#[test]
fn a_counter_notice_must_be_complete_and_inside_the_window() {
    let (c, _d) = world();
    takedown(&c, 1_000);
    let mut bad = full_notice();
    bad.insert("perjury_stmt".into(), json!(false));
    bad.remove("phone");
    let Err(Fail::Pool(p)) = counter_notice(&PoolCtx::new(&c, 2_000), "a", ID, &bad) else { panic!() };
    assert_eq!((p.status, p.code.as_str(), p.body()["missing"].clone()), (400, "counter_notice_incomplete", json!(["phone", "perjury_stmt"])));
    let x = PoolCtx::new(&c, 2_000);
    let r = counter_notice(&x, "a", ID, &full_notice()).unwrap();
    assert_eq!(r.0, 202);
    assert!(r.1["restore_not_before"].as_i64().unwrap() > 2_000 && r.1["restore_by"].as_i64().unwrap() > r.1["restore_not_before"].as_i64().unwrap());
    assert_eq!(n(&c, "SELECT COUNT(*) FROM dmca_counter_notices"), 1);
    assert_eq!(n(&c, "SELECT COUNT(*) FROM admin_flags WHERE kind = 'counter_notice'"), 1);
    assert_eq!(x.take_notices().len(), 1);
    // One counter-notice per action: a second finds no open action (a plain miss, as in the Worker).
    assert_eq!(err(counter_notice(&PoolCtx::new(&c, 3_000), "a", ID, &full_notice())).0, 404);
}

#[test]
fn a_counter_notice_after_the_quarantine_says_the_window_closed() {
    let (c, _d) = world();
    takedown(&c, 1_000);
    let late = counter_notice(&PoolCtx::new(&c, 1_000 + QUARANTINE_MS + 1), "a", ID, &full_notice());
    assert_eq!(err(late), (409, "window_closed".into()));
    assert_eq!(err(counter_notice(&PoolCtx::new(&c, 2_000), "b", ID, &full_notice())).0, 404);
}

#[test]
fn recheck_holds_a_public_item_that_serves_a_privacy_blocked_key() {
    let (c, _d) = world();
    c.execute("INSERT INTO item_questions (publish_id, pos, match_hash) VALUES ('p1', 0, 'hh')", []).ok();
    c.execute("INSERT INTO privacy_blocks (kind, hash, at) VALUES ('match', 'hh', 1)", []).ok();
    recheck_blocks(&PoolCtx::new(&c, 5_000), ID).unwrap();
    assert_eq!(n(&c, "SELECT COUNT(*) FROM item_holds WHERE source = 'privacy'"), c.query_row("SELECT COUNT(*) FROM item_questions WHERE match_hash = 'hh'", [], |r| r.get::<_, i64>(0)).unwrap().min(1));
}
