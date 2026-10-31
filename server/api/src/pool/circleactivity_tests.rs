//! Tests for `circleactivity.rs` on the real migrated schema: the activity list (names, filtering, titles by visibility, cursor paging)
//! and the heads route (every status).
use super::*;
use crate::db;
use crate::pool::circles::create_circle;
use crate::pool::ctx::Fail;
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
fn m(v: Value) -> Map<String, Value> {
    v.as_object().unwrap().clone()
}
fn err(f: Fail) -> (u16, String, Value) {
    match f {
        Fail::Pool(e) => (e.status, e.code.clone(), e.body()),
        other => panic!("{other:?}"),
    }
}
fn make(c: &rusqlite::Connection, u: &str) -> String {
    create_circle(&PoolCtx::new(c, 1_000), u, &m(json!({ "name": "Team" }))).unwrap().1["circle"]["id"].as_str().unwrap().to_string()
}
fn join(c: &rusqlite::Connection, cid: &str, u: &str, role: &str, seq: i64) {
    c.execute("INSERT INTO circle_members (circle_id, user_id, role, joined_at, join_seq) VALUES (?1, ?2, ?3, 10, ?4)", params![cid, u, role, seq]).unwrap();
}
fn bank(c: &rusqlite::Connection, cid: &str, n: u8, author: &str, title: &str) -> String {
    let id = format!("{:032x}", n);
    c.execute("INSERT INTO circle_banks (id, circle_id, author_id, title, license, head_version, head_version_id, created_at, updated_at) VALUES (?1, ?2, ?3, ?4, 'circle-only', 2, ?5, 1, 77)", params![id, cid, author, title, format!("v{n}")]).unwrap();
    id
}
fn log(c: &rusqlite::Connection, id: &str, at: i64, action: &str, actor: Option<&str>, kind: &str, target: Option<&str>, cid: &str, detail: Value) {
    c.execute(
        "INSERT INTO pool_audit (id, at, actor_id, actor_kind, action, item_id, target_user_id, detail) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
        params![id, at, actor, kind, action, cid, target, detail.to_string()],
    )
    .unwrap();
}

#[test]
fn the_activity_list_shows_members_events_by_name_and_hides_everything_else() {
    let (c, _d) = open();
    for u in ["a", "b"] {
        user(&c, u);
    }
    let cid = make(&c, "a");
    join(&c, &cid, "b", "viewer", 5);
    log(&c, &"1".repeat(32), 2_000, "circle.join", Some("b"), "user", Some("b"), &cid, json!({ "role": "viewer", "invite_id": "secret" }));
    log(&c, &"2".repeat(32), 2_100, "circle.invite_create", Some("a"), "user", Some("a"), &cid, json!({ "invite_id": "x" }));
    log(&c, &"3".repeat(32), 2_200, "circle.admin_close", None, "admin", None, &cid, json!({}));
    log(&c, &"4".repeat(32), 2_300, "circle.version", Some("a"), "user", None, &cid, json!({ "version": 3, "merged_from": 1, "bank_id": "nope" }));
    log(&c, &"5".repeat(32), 2_400, "circle.mystery", Some("a"), "user", None, &cid, json!({}));
    let x = PoolCtx::new(&c, 5_000);
    let r = activity(&x, "b", &cid, None).unwrap();
    let a = r["activity"].as_array().unwrap();
    // create (audited by create_circle) + join + admin close + version; the invite and the unknown action are not members' business.
    assert_eq!(a.iter().map(|l| l["what"].clone()).collect::<Vec<_>>(), vec![json!("version_sent"), json!("closed"), json!("joined"), json!("created")]);
    assert_eq!((a[0]["by"].clone(), a[0]["version"].clone(), a[0]["merged_from"].clone(), a[0]["member"].clone()), (json!("Teacher a"), json!(3), json!(1), json!(null)));
    assert_eq!((a[1]["by"].clone(), a[2]["member"].clone(), a[2]["role"].clone()), (json!("an administrator"), json!("Teacher b"), json!("viewer")));
    assert!(a[2].get("invite_id").is_none());
    assert!(r["next_before"].is_null());
    assert_eq!(err(activity(&x, "nobody", &cid, None).unwrap_err()).1, "not_found");
}

#[test]
fn bank_titles_follow_who_may_see_the_bank_and_the_walk_pages_exactly_once() {
    let (c, _d) = open();
    for u in ["a", "b", "c"] {
        user(&c, u);
    }
    let cid = make(&c, "a");
    join(&c, &cid, "b", "editor", 5);
    join(&c, &cid, "c", "viewer", 6);
    let live = bank(&c, &cid, 1, "b", "Live");
    let held = bank(&c, &cid, 2, "b", "Held");
    let removed = bank(&c, &cid, 3, "b", "Removed");
    c.execute("INSERT INTO circle_holds (bank_id, reason, source, at) VALUES (?1, 'reports', '', 1)", [&held]).unwrap();
    c.execute("UPDATE circle_banks SET removed_at = 5, removed_reason = 'author_removed' WHERE id = ?1", [&removed]).unwrap();
    for (i, b) in [&live, &held, &removed].iter().enumerate() {
        log(&c, &format!("{:032x}", 100 + i), 3_000 + i as i64, "circle.bank_add", Some("b"), "user", None, &cid, json!({ "bank_id": b }));
    }
    let x = PoolCtx::new(&c, 5_000);
    let titles = |who: &str| -> Vec<Value> { activity(&x, who, &cid, None).unwrap()["activity"].as_array().unwrap().iter().take(3).map(|l| l["bank_title"].clone()).collect() };
    // newest first: removed, held, live
    assert_eq!(titles("c"), vec![json!(null), json!(null), json!("Live")], "a viewer sees neither");
    assert_eq!(titles("b"), vec![json!("Removed"), json!("Held"), json!("Live")], "the author sees all of theirs");
    assert_eq!(titles("a"), vec![json!("Removed"), json!(null), json!("Live")], "the owner sees a removed bank but not a report-held one");
    // Paging: a page is ACTIVITY_PAGE long; the walk shows every line once, and a bad cursor is a 400.
    for i in 0..60 {
        log(&c, &format!("{:032x}", 1000 + i), 4_000 + i, "circle.edit", Some("a"), "user", None, &cid, json!({}));
    }
    let p1 = activity(&x, "a", &cid, None).unwrap();
    assert_eq!(p1["activity"].as_array().unwrap().len(), ACTIVITY_PAGE);
    let cur = p1["next_before"].as_str().unwrap().to_string();
    let p2 = activity(&x, "a", &cid, Some(&cur)).unwrap();
    let total = 60 + 3 + 1; // edits + bank_add x3 + the create
    assert_eq!(p2["activity"].as_array().unwrap().len(), total - ACTIVITY_PAGE);
    assert!(p2["next_before"].is_null());
    for bad in ["junk", "1:2", "12:zz", &format!("5:{}", "a".repeat(32)), &format!("4000:{}", "f".repeat(32))] {
        assert_eq!(err(activity(&x, "a", &cid, Some(bad)).unwrap_err()).2["field"], json!("before"), "{bad}");
    }
}

#[test]
fn heads_reports_each_bank_by_what_the_caller_may_know() {
    let (c, _d) = open();
    for u in ["a", "b", "z"] {
        user(&c, u);
    }
    let cid = make(&c, "a");
    join(&c, &cid, "b", "editor", 5);
    let cur = bank(&c, &cid, 1, "a", "Current");
    let held = bank(&c, &cid, 2, "a", "Held");
    let removed = bank(&c, &cid, 3, "a", "Removed");
    let gone = bank(&c, &cid, 4, "a", "Gone");
    c.execute("INSERT INTO circle_holds (bank_id, reason, source, at) VALUES (?1, 'reports', '', 1)", [&held]).unwrap();
    c.execute("UPDATE circle_banks SET removed_at = 5, removed_reason = 'owner_removed', purge_after = 900 WHERE id = ?1", [&removed]).unwrap();
    c.execute("UPDATE circle_banks SET removed_at = 5, removed_reason = 'circle_closed' WHERE id = ?1", [&gone]).unwrap();
    c.execute("INSERT INTO circle_versions (id, bank_id, circle_id, version, commit_id, request_sha256, committer_id, license, status, full_key, index_key, bytes, question_count, created_at) VALUES ('v1', ?1, ?2, 2, 'c1', 'x', 'a', 'circle-only', 'current', 'fk', 'ik', 1, 1, 600)", params![cur, cid]).unwrap();
    c.execute("UPDATE circle_versions SET committed_at = 600 WHERE id = 'v1'", []).unwrap();
    c.execute("UPDATE circle_banks SET head_version_id = 'v1' WHERE id = ?1", [&cur]).unwrap();
    let x = PoolCtx::new(&c, 5_000);
    let unknown = format!("{:032x}", 99);
    let r = heads(&x, "b", &m(json!({ "bank_ids": [cur, held, removed, gone, unknown, "not-an-id", cur] }))).unwrap();
    let banks = r["banks"].as_array().unwrap();
    assert_eq!(banks.len(), 7, "one answer per id asked, in order");
    assert_eq!(banks[0]["status"], json!("current"));
    assert_eq!((banks[0]["version"].clone(), banks[0]["license"].clone(), banks[0]["hide_key"].clone()), (json!(2), json!("circle-only"), json!(false)));
    assert_eq!(banks[1], json!({ "bank_id": held, "status": "under_review" }));
    assert_eq!(banks[2], json!({ "bank_id": removed, "status": "removed", "until": 900 }));
    assert_eq!(banks[3], json!({ "bank_id": gone, "status": "removed" }));
    assert_eq!(banks[4]["status"], json!("unavailable"));
    assert_eq!(banks[5]["status"], json!("unavailable"));
    assert_eq!(banks[6], banks[0]);
    // A non-member sees only "unavailable"; the author sees their own held bank.
    let z = heads(&x, "z", &m(json!({ "bank_ids": [cur] }))).unwrap();
    assert_eq!(z["banks"][0]["status"], json!("unavailable"));
    assert_eq!(heads(&x, "a", &m(json!({ "bank_ids": [held] }))).unwrap()["banks"][0]["status"], json!("current"));
    // A closed circle makes every bank unavailable.
    c.execute("UPDATE circles SET closed_at = 1, close_reason = 'owner_closed' WHERE id = ?1", [&cid]).unwrap();
    assert_eq!(heads(&x, "b", &m(json!({ "bank_ids": [cur] }))).unwrap()["banks"][0]["status"], json!("unavailable"));
}

#[test]
fn heads_validates_its_ids_and_charges_a_read() {
    let (c, _d) = open();
    user(&c, "a");
    let x = PoolCtx::new(&c, 5_000);
    for body in [json!({}), json!({ "bank_ids": [] }), json!({ "bank_ids": "x" }), json!({ "bank_ids": [1] }), json!({ "bank_ids": ["x".repeat(65)] }), json!({ "bank_ids": vec!["a"; 201] })] {
        assert_eq!(err(heads(&x, "a", &m(body)).unwrap_err()).2["field"], json!("bank_ids"));
    }
    heads(&x, "a", &m(json!({ "bank_ids": ["a"] }))).unwrap();
    let n: i64 = c.query_row("SELECT count FROM usage_daily WHERE user_id = 'a' AND metric = 'item_reads'", [], |r| r.get(0)).unwrap();
    assert_eq!(n, 1);
}
