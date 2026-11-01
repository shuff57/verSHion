//! Tests for `adminhold.rs`: a takedown's copies are previewed then held, released and exempted; a court action is resolved; a withdrawn notice puts back.
use super::*;
use crate::db;
use crate::pool::blockplan::block_plan;
use crate::pool::blocking::{AFFECTED_LIST, BlockSelection};
use crate::pool::modtakedown::takedown;
use rusqlite::Connection;
use std::path::Path;

const ADM: &str = "adm";

fn id(n: u8) -> String {
    format!("{:032x}", n)
}
fn h64(c: char) -> String {
    c.to_string().repeat(64)
}
fn keys() -> PoolKeys {
    PoolKeys::from_secret("test-secret-0123456789abcdef0123456789abcdef")
}
fn item(c: &Connection, n: u8, author: &str, hash: &str, seq: i64) {
    let (i, p) = (id(n), format!("p{n}"));
    c.execute(
        "INSERT INTO items (id, author_id, kind, visibility, current_publish_id, public_publish_id, public_version, version, title, license, committed_at, created_at, updated_at, fold_rev)
         VALUES (?1, ?2, 'bank', 'public', ?3, ?3, 1, 1, ?4, 'CC-BY-4.0', 1, 1, 1, 1)",
        params![i, author, p, format!("Item {n}")],
    )
    .unwrap();
    c.execute(
        "INSERT INTO item_publishes (publish_id, item_id, author_id, version, public_version, status, schema_version, hide_key, license, attested_at, attest_text_rev,
            acks_json, request_sha256, stage_token, full_key, stripped_key, full_bytes, title, subject, course, grade, kinds_mask, question_count, has_images, created_at)
         VALUES (?1, ?2, ?3, 1, 1, 'current', 2, 0, 'CC-BY-4.0', 1, 'r', '{}', 'x', ?1, ?4, 'SK', 10, '', '', '', '', 0, 1, 1, 1)",
        params![p, i, author, format!("items/{i}/{p}.json")],
    )
    .unwrap();
    let required: Vec<String> = {
        let mut st = c.prepare("SELECT name FROM pragma_table_info('item_questions') WHERE \"notnull\" = 1 AND dflt_value IS NULL").unwrap();
        st.query_map([], |r| r.get(0)).unwrap().map(|x| x.unwrap()).collect()
    };
    let vals: Vec<String> = required.iter().map(|n| if n == "pos" { "0".to_string() } else { "''".to_string() }).collect();
    c.execute(&format!("INSERT INTO item_questions ({}) VALUES ({})", required.join(", "), vals.join(", ")), []).unwrap();
    c.execute(
        "UPDATE item_questions SET publish_id = ?1, qid = ?2, pos = 0, qkind = 'Mc', match_hash = ?3, seq = ?4, bk_norm = ?5, bk_stem = ?6, bk_full = ?7, bk_expl = ?8, bk_ans = ?9
          WHERE rowid = last_insert_rowid()",
        params![p, format!("q-{p}"), format!("m:{hash}"), seq, format!("n:{hash}"), format!("s:{hash}"), format!("f:{hash}"), format!("e:{hash}"), format!("a:{hash}")],
    )
    .unwrap();
}
fn body(v: Value) -> Map<String, Value> {
    v.as_object().unwrap().clone()
}
fn n(c: &Connection, sql: &str) -> i64 {
    c.query_row(sql, [], |r| r.get(0)).unwrap()
}

/// Item 1 (a) taken down under notice 9; item 2 (b) copies its question later. Returns the action id.
fn world() -> (Connection, tempfile::TempDir, BlobStore, String) {
    let dir = tempfile::tempdir().unwrap();
    let mut c = db::open(&dir.path().join("t.db")).unwrap();
    db::migrate(&mut c, &Path::new(env!("CARGO_MANIFEST_DIR")).join("../worker/migrations")).unwrap();
    for u in ["a", "b"] {
        c.execute("INSERT INTO users (id, email, display_name, trust, created_at) VALUES (?1, ?2, ?1, 'teacher', 1)", params![u, format!("{u}@x.org")]).unwrap();
    }
    let s = BlobStore::new(dir.path().join("blobs")).unwrap();
    item(&c, 1, "a", &h64('1'), 1);
    item(&c, 2, "b", &h64('1'), 2);
    c.execute("INSERT INTO dmca_notices (id, received_at, complainant_name, complainant_contact, work_desc, material_desc, has_signature, has_good_faith, has_perjury_stmt, status, recorded_by) VALUES (?1, 1, 'C', 'c@x', 'w', 'm', 1, 1, 1, 'open', 'admin')", [id(9)]).unwrap();
    let x = PoolCtx::new(&c, 1_000);
    let sel = BlockSelection { all: true, ..Default::default() };
    let plan = block_plan(&x, AFFECTED_LIST, &id(1), &sel, "dmca", None).unwrap();
    let out = takedown(&x, &s, ADM, &id(9), &id(1), &plan, Some(1_000 + 60 * 86_400_000), &[], None).unwrap();
    (c, dir, s, out.action_id)
}

#[test]
fn copies_are_previewed_then_held_once_per_token_and_released_together() {
    let (c, _d, _s, aid) = world();
    let k = keys();
    let b = body(json!({ "action_id": aid, "include_common": true }));
    let (st, pv) = hold_copies_preview(&PoolCtx::new(&c, 2_000), &k, ADM, &b).unwrap();
    assert_eq!(st, 200);
    assert_eq!(pv["copies"].as_array().unwrap().iter().map(|c| c["item_id"].clone()).collect::<Vec<_>>(), vec![json!(id(2))]);
    // Confirm without the token fails; with it the copy is held and the author told.
    assert!(matches!(hold_copies(&PoolCtx::new(&c, 2_000), &k, ADM, &b), Err(Fail::Pool(p)) if p.code == "preview_required"));
    let mut ok = b.clone();
    ok.insert("confirm_preview".into(), pv["confirm_preview"].clone());
    let (_, v) = hold_copies(&PoolCtx::new(&c, 2_000), &k, ADM, &ok).unwrap();
    assert_eq!((v["held"].clone(), v["more"].clone()), (json!([id(2)]), json!(false)));
    assert_eq!(n(&c, &format!("SELECT COUNT(*) FROM item_holds WHERE source = 'dmca:{aid}'")), 1);
    assert!(matches!(hold_copies(&PoolCtx::new(&c, 2_000), &k, ADM, &ok), Err(Fail::Pool(p)) if p.code == "preview_required"), "token is one use");
    let (_, l) = copies_of_action(&PoolCtx::new(&c, 2_000), Some(&aid), None, None, None).unwrap();
    assert_eq!((l["held"][0]["item_id"].clone(), l["restored"].clone(), l["candidates"].as_array().unwrap().len()), (json!(id(2)), json!(false), 0));
    let (_, r) = release_copies(&PoolCtx::new(&c, 2_000), ADM, &body(json!({ "action_id": aid }))).unwrap();
    assert_eq!(r["released"], 1);
    assert_eq!(hold_copies_preview(&PoolCtx::new(&c, 2_000), &k, ADM, &body(json!({ "action_id": id(77) }))).unwrap().0, 404);
}

#[test]
fn naming_the_rightful_author_releases_their_held_items() {
    let (c, _d, _s, aid) = world();
    c.execute("INSERT INTO item_holds (item_id, reason, at, by, source) VALUES (?1, 'admin', 1, 'x', ?2)", params![id(2), format!("dmca:{aid}")]).unwrap();
    let x = PoolCtx::new(&c, 2_000);
    let uid = "0123456789abcdef0123456789abcdef0123";
    c.execute("UPDATE items SET author_id = 'b' WHERE id = ?1", [id(2)]).unwrap();
    assert!(matches!(exempt(&x, ADM, &body(json!({ "action_id": aid, "user_id": "nope", "action": "add" }))), Err(Fail::Pool(p)) if p.code == "invalid_request"));
    assert_eq!(exempt(&x, ADM, &body(json!({ "action_id": id(77), "user_id": uid, "action": "add" }))).unwrap().0, 404);
    // The fixture's "b" is not a 36-char id, so give the author a real-shaped one.
    c.execute("UPDATE users SET id = ?1 WHERE id = 'b'", [uid]).unwrap();
    c.execute("UPDATE items SET author_id = ?1 WHERE id = ?2", params![uid, id(2)]).unwrap();
    let (_, v) = exempt(&x, ADM, &body(json!({ "action_id": aid, "user_id": uid, "action": "add" }))).unwrap();
    assert_eq!(v["released"], 1);
    assert_eq!(n(&c, "SELECT COUNT(*) FROM dmca_exempt"), 1);
    let (_, v) = exempt(&x, ADM, &body(json!({ "action_id": aid, "user_id": uid, "action": "remove" }))).unwrap();
    assert_eq!((v["released"].clone(), n(&c, "SELECT COUNT(*) FROM dmca_exempt")), (json!(0), 0));
}

#[test]
fn a_court_action_is_recorded_once_and_resolved_by_restoring() {
    let (c, _d, s, aid) = world();
    let x = PoolCtx::new(&c, 5_000);
    assert_eq!(court_action(&x, ADM, &body(json!({ "action_id": aid }))).unwrap().0, 200);
    assert_eq!(court_action(&x, ADM, &body(json!({ "action_id": aid }))).unwrap().0, 404, "already recorded");
    assert!(matches!(resolve(&x, &s, ADM, &body(json!({ "action_id": aid, "outcome": "maybe" }))), Err(Fail::Pool(p)) if p.code == "invalid_request"));
    assert_eq!(resolve(&x, &s, ADM, &body(json!({ "action_id": id(77), "outcome": "restore" }))).unwrap().0, 404);
    let (st, _) = resolve(&x, &s, ADM, &body(json!({ "action_id": aid, "outcome": "restore", "note": "court sided with author" }))).unwrap();
    assert_eq!(st, 200);
    assert_eq!(n(&c, "SELECT removed_at IS NULL FROM items WHERE id = ?1".replace("?1", &format!("'{}'", id(1))).as_str()), 1);
    assert_eq!(c.query_row::<String, _, _>("SELECT outcome FROM dmca_actions", [], |r| r.get(0)).unwrap(), "court_restore");
    assert_eq!(n(&c, "SELECT COUNT(*) FROM auth_audit WHERE action = 'pool.dmca_resolve'"), 1);
    assert_eq!(resolve(&x, &s, ADM, &body(json!({ "action_id": aid, "outcome": "restore" }))).unwrap().0, 404, "closed");
    assert_eq!(resolve(&x, &s, ADM, &body(json!({ "action_id": aid, "outcome": "put_back" }))).unwrap().0, 409);
}

#[test]
fn withdrawing_a_notice_puts_back_what_was_taken_down_under_it() {
    let (c, _d, s, _aid) = world();
    let x = PoolCtx::new(&c, 5_000);
    assert_eq!(withdraw(&x, &s, ADM, &body(json!({ "notice_id": id(77) }))).unwrap(), (404, json!({ "error": "notice_not_found" })));
    let (st, v) = withdraw(&x, &s, ADM, &body(json!({ "notice_id": id(9) }))).unwrap();
    assert_eq!((st, v["restored"].clone()), (200, json!(1)));
    assert_eq!(c.query_row::<String, _, _>("SELECT status FROM dmca_notices", [], |r| r.get(0)).unwrap(), "withdrawn");
    assert_eq!(n(&c, "SELECT removed_at IS NULL FROM items WHERE id = ?1".replace("?1", &format!("'{}'", id(1))).as_str()), 1);
    // A retry restores nothing more and is still 200.
    assert_eq!(withdraw(&x, &s, ADM, &body(json!({ "notice_id": id(9) }))).unwrap().1["restored"], 0);
    // Its copies are no longer offered for holding.
    assert!(matches!(hold_copies_preview(&x, &keys(), ADM, &body(json!({ "action_id": c.query_row::<String, _, _>("SELECT id FROM dmca_actions", [], |r| r.get(0)).unwrap() }))), Err(Fail::Pool(p)) if p.code == "action_closed"));
}

#[test]
fn the_pool_key_matches_until_a_different_one_is_stored() {
    let (c, _d, _s, _a) = world();
    let x = PoolCtx::new(&c, 1);
    assert_eq!(pool_key(&x, "k1").unwrap(), json!({ "key_id": "k1", "stored_key_id": null, "match": true }));
    c.execute("INSERT INTO pool_kv (key, value) VALUES ('pool_key_id', 'k1')", []).unwrap();
    assert_eq!(pool_key(&x, "k1").unwrap()["match"], true);
    assert_eq!(pool_key(&x, "k2").unwrap()["match"], false);
}
