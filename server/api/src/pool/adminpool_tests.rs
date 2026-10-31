//! Tests for `adminpool.rs`: holds are placed and lifted source-aware, reports paged and grouped, removals reversible.
use super::*;
use crate::db;
use rusqlite::Connection;
use std::path::Path;

const I1: &str = "11111111111111111111111111111111";
const R1: &str = "22222222222222222222222222222222";
const R2: &str = "33333333333333333333333333333333";

fn world() -> (Connection, tempfile::TempDir) {
    let d = tempfile::tempdir().unwrap();
    let mut c = db::open(&d.path().join("t.db")).unwrap();
    db::migrate(&mut c, &Path::new(env!("CARGO_MANIFEST_DIR")).join("../worker/migrations")).unwrap();
    for u in ["a", "b"] {
        c.execute("INSERT INTO users (id, email, display_name, trust, created_at) VALUES (?1, ?2, ?3, 'teacher', 1)", params![u, format!("{u}@x.org"), u]).unwrap();
    }
    c.execute("INSERT INTO items (id, author_id, kind, visibility, current_publish_id, license, title, created_at, updated_at) VALUES (?1, 'a', 'bank', 'public', 'p1', 'cc-by', 'T', 1, 1)", [I1]).unwrap();
    c.execute(
        "INSERT INTO item_publishes (publish_id, item_id, author_id, version, status, schema_version, hide_key, license, attested_at, attest_text_rev, acks_json, request_sha256, stage_token,
           full_key, full_bytes, title, subject, course, grade, kinds_mask, question_count, has_images, created_at, committed_at)
         VALUES ('p1', ?1, 'a', 1, 'current', 1, 0, 'cc-by', 1, 'r', '{}', 'x', 't', 'items/k.json', 1, 'T', '', '', '', 0, 0, 0, 1, 1)",
        [I1],
    )
    .unwrap();
    (c, d)
}
fn body(v: Value) -> Map<String, Value> {
    v.as_object().unwrap().clone()
}
fn n(c: &Connection, sql: &str) -> i64 {
    c.query_row(sql, [], |r| r.get(0)).unwrap()
}
fn code(r: Res<(u16, Value)>) -> String {
    match r {
        Err(Fail::Pool(p)) => p.code.clone(),
        other => panic!("{other:?}"),
    }
}

#[test]
fn a_manual_hold_is_placed_once_and_lifted_by_a_human_release() {
    let (c, _d) = world();
    let x = PoolCtx::new(&c, 100);
    assert_eq!(hold(&x, "adm", &body(json!({ "item_id": I1, "note": "look" }))).unwrap(), (200, json!({ "ok": true, "changed": true })));
    assert_eq!(hold(&x, "adm", &body(json!({ "item_id": I1 }))).unwrap().1["changed"], false);
    assert_eq!(code(hold(&x, "adm", &body(json!({ "item_id": "bad" })))), "invalid_request");
    assert_eq!(n(&c, "SELECT COUNT(*) FROM auth_audit WHERE action = 'pool.hold'"), 2);
    let (st, v) = release(&x, "adm", &body(json!({ "item_id": I1, "reason": "admin" }))).unwrap();
    assert_eq!((st, v["released"].clone(), v["remaining"].clone()), (200, json!(1), json!([])), "a manual admin hold is lifted by reason admin");
}

#[test]
fn a_forced_release_needs_force_and_a_privacy_hold_has_its_own_route() {
    let (c, _d) = world();
    c.execute("INSERT INTO item_holds (item_id, reason, at, by, source) VALUES (?1, 'student_data', 1, 'x', 'privacy')", [I1]).unwrap();
    let x = PoolCtx::new(&c, 100);
    assert_eq!(code(release(&x, "adm", &body(json!({ "item_id": I1, "reason": "student_data", "source": "privacy" })))), "hold_has_own_route");
    assert_eq!(code(release(&x, "adm", &body(json!({ "item_id": I1, "reason": "nope" })))), "invalid_request");
    let (_, v) = release(&x, "adm", &body(json!({ "item_id": I1, "reason": "student_data", "source": "privacy", "force": true }))).unwrap();
    assert_eq!(v["released"], 1);
    assert_eq!(n(&c, "SELECT COUNT(*) FROM pool_audit WHERE action = 'item.release_forced'"), 1);
}

#[test]
fn releasing_a_hold_by_source_records_what_the_item_carries_and_names_the_publish() {
    let (c, _d) = world();
    c.execute("INSERT INTO item_holds (item_id, reason, at, by, source) VALUES (?1, 'blocklist', 1, 'x', 'needs_reupload')", [I1]).unwrap();
    let x = PoolCtx::new(&c, 100);
    let b = |extra: Value| {
        let mut m = body(json!({ "item_id": I1, "source": "needs_reupload", "note": "reviewed, innocent" }));
        m.extend(extra.as_object().unwrap().clone());
        m
    };
    assert_eq!(code(release_hold(&x, "adm", &body(json!({ "item_id": I1, "source": "needs_reupload", "note": "short" })))), "invalid_request");
    assert_eq!(code(release_hold(&x, "adm", &b(json!({})))), "invalid_request");
    assert_eq!(release_hold(&x, "adm", &b(json!({ "publish_id": "other" }))).unwrap(), (409, json!({ "error": "publish_changed", "current_publish_id": "p1" })));
    let (st, v) = release_hold(&x, "adm", &b(json!({ "publish_id": "p1" }))).unwrap();
    assert_eq!((st, v["released"].clone()), (200, json!(1)));
    assert_eq!(n(&c, "SELECT COUNT(*) FROM hold_releases WHERE item_id = ?1 AND source = 'needs_reupload'".replace("?1", &format!("'{I1}'")).as_str()), 1);
    assert_eq!(release_hold(&x, "adm", &body(json!({ "item_id": R1, "source": "", "note": "long enough note", "publish_id": "p1" }))).unwrap().0, 404);
}

#[test]
fn dismissing_a_report_resolves_it_once() {
    let (c, _d) = world();
    c.execute("INSERT INTO reports (id, item_id, reporter_id, reason, at) VALUES (?1, ?2, 'b', 'other', 5)", params![R1, I1]).unwrap();
    let x = PoolCtx::new(&c, 100);
    assert_eq!(dismiss(&x, "adm", &body(json!({ "report_id": R1 }))).unwrap(), (200, json!({ "ok": true })));
    assert_eq!(n(&c, "SELECT COUNT(*) FROM reports WHERE action = 'dismissed' AND resolved_at = 100"), 1);
    assert_eq!(dismiss(&x, "adm", &body(json!({ "report_id": R1 }))).unwrap().0, 404);
}

#[test]
fn open_reports_are_grouped_by_item_and_paged_by_cursor() {
    let (c, _d) = world();
    c.execute("INSERT INTO reports (id, item_id, reporter_id, reason, at) VALUES (?1, ?3, 'b', 'other', 5), (?2, ?3, 'b', 'wrong_answer', 6)", params![R1, R2, I1]).unwrap();
    let v = list_reports(&PoolCtx::new(&c, 100), None).unwrap();
    assert_eq!(v["items"].as_array().unwrap().len(), 1);
    assert_eq!(v["items"][0]["reports"].as_array().unwrap().len(), 2);
    assert_eq!(v["items"][0]["reports"][0]["reporter_reports"], 2);
    assert_eq!(v["next_cursor"], Value::Null);
    let after = list_reports(&PoolCtx::new(&c, 100), Some(&format!("5:{R1}"))).unwrap();
    assert_eq!(after["items"][0]["reports"].as_array().unwrap().len(), 1);
}

#[test]
fn flags_list_most_severe_first_and_reject_a_bad_cursor() {
    let (c, _d) = world();
    c.execute("INSERT INTO admin_flags (id, kind, item_id, dedup, at) VALUES (?1, 'appeal', ?3, 'd1', 5), (?2, 'privacy_text_match', ?3, 'd2', 4)", params![R1, R2, I1]).unwrap();
    let x = PoolCtx::new(&c, 100);
    let v = list_flags(&x, Some(1), None).unwrap();
    assert_eq!((v["flags"].as_array().unwrap().len(), v["total_open"].clone()), (1, json!(2)));
    let cur = v["next_cursor"].as_str().unwrap().to_string();
    let next = list_flags(&x, Some(1), Some(&cur)).unwrap();
    assert_eq!(next["flags"].as_array().unwrap().len(), 1);
    assert_ne!(next["flags"][0]["id"], v["flags"][0]["id"]);
    assert!(matches!(list_flags(&x, None, Some("junk")), Err(Fail::Pool(p)) if p.code == "invalid_request"));
}

#[test]
fn item_head_hides_the_body_key_and_returns_it_for_streaming() {
    let (c, _d) = world();
    let x = PoolCtx::new(&c, 100);
    let (head, key) = item_head(&x, I1).unwrap().unwrap();
    assert_eq!(key.as_deref(), Some("items/k.json"));
    assert!(head["publishes"][0].get("full_key").is_none());
    assert_eq!(head["publishes"][0]["publish_id"], "p1");
    assert!(item_head(&x, R1).unwrap().is_none());
}

#[test]
fn an_admin_removal_is_reversible_and_not_repeatable() {
    let (c, _d) = world();
    let x = PoolCtx::new(&c, 1_000);
    c.execute("INSERT INTO reports (id, item_id, reporter_id, reason, at) VALUES (?1, ?2, 'b', 'other', 5)", params![R1, I1]).unwrap();
    assert_eq!(remove(&x, "adm", &body(json!({ "item_id": I1 }))).unwrap(), (200, json!({ "ok": true })));
    assert_eq!(n(&c, "SELECT COUNT(*) FROM reports WHERE action = 'removed'"), 1);
    assert_eq!(remove(&x, "adm", &body(json!({ "item_id": I1 }))).unwrap(), (409, json!({ "error": "not_removable" })));
    assert_eq!(unremove(&x, "adm", &body(json!({ "item_id": I1 }))).unwrap(), (200, json!({ "ok": true })));
    assert_eq!(n(&c, "SELECT removed_at IS NULL FROM items"), 1);
    assert_eq!(n(&c, "SELECT COUNT(*) FROM auth_audit WHERE action = 'pool.unremove'"), 1);
    assert_eq!(unremove(&x, "adm", &body(json!({ "item_id": I1 }))).unwrap().0, 404);
}
