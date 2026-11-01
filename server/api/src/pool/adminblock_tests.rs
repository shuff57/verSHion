//! Tests for `adminblock.rs`: previews and their one-use tokens, the purge and takedown confirms, unblocks, evidence, the DMCA records, limits and flags.
use super::*;
use crate::db;
use rusqlite::Connection;
use std::path::Path;

const ADM: &str = "adm";

fn open() -> (Connection, tempfile::TempDir, BlobStore) {
    let dir = tempfile::tempdir().unwrap();
    let mut conn = db::open(&dir.path().join("t.db")).unwrap();
    db::migrate(&mut conn, &Path::new(env!("CARGO_MANIFEST_DIR")).join("../worker/migrations")).unwrap();
    for u in ["a", "b"] {
        conn.execute("INSERT INTO users (id, email, display_name, trust, created_at) VALUES (?1, ?2, ?1, 'teacher', 1)", params![u, format!("{u}@x.org")]).unwrap();
    }
    let s = BlobStore::new(dir.path().join("blobs")).unwrap();
    (conn, dir, s)
}
fn keys() -> PoolKeys {
    PoolKeys::from_secret("test-secret-0123456789abcdef0123456789abcdef")
}
fn id(n: u8) -> String {
    format!("{:032x}", n)
}
fn item(c: &Connection, n: u8, author: &str) {
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
         VALUES (?1, ?2, ?3, 1, 1, 'current', 2, 0, 'CC-BY-4.0', 1, 'r', '{}', 'x', ?1, ?5, 'SK', 10, '', '', '', '', 0, 1, 1, 1)",
        params![p, i, author, 0, format!("items/{i}/{p}.json")],
    )
    .unwrap();
}
fn question(c: &Connection, n: u8, pos: i64, h: &str, seq: i64) {
    let p = format!("p{n}");
    let required: Vec<String> = {
        let mut st = c.prepare("SELECT name FROM pragma_table_info('item_questions') WHERE \"notnull\" = 1 AND dflt_value IS NULL").unwrap();
        st.query_map([], |r| r.get(0)).unwrap().map(|x| x.unwrap()).collect()
    };
    let vals: Vec<String> = required.iter().map(|n| if n == "pos" { "0".to_string() } else { "''".to_string() }).collect();
    c.execute(&format!("INSERT INTO item_questions ({}) VALUES ({})", required.join(", "), vals.join(", ")), []).unwrap();
    c.execute(
        "UPDATE item_questions SET publish_id = ?1, qid = ?2, pos = ?3, qkind = 'Mc', match_hash = ?4, seq = ?5, bk_norm = ?6, bk_stem = ?7, bk_full = ?8, bk_expl = ?9, bk_ans = ?10
          WHERE rowid = last_insert_rowid()",
        params![p, format!("q{pos}-{p}"), pos, format!("m:{h}"), seq, format!("n:{h}"), format!("s:{h}"), format!("f:{h}"), format!("e:{h}"), format!("a:{h}")],
    )
    .unwrap();
}
fn body(v: Value) -> Map<String, Value> {
    v.as_object().unwrap().clone()
}
fn n(c: &Connection, sql: &str) -> i64 {
    c.query_row(sql, [], |r| r.get(0)).unwrap()
}
fn code<T: std::fmt::Debug>(r: Res<T>) -> (u16, String) {
    match r {
        Err(Fail::Pool(p)) => (p.status, p.code.clone()),
        other => panic!("{other:?}"),
    }
}
fn h64(c: char) -> String {
    c.to_string().repeat(64)
}

#[test]
fn a_privacy_purge_needs_its_preview_token_once_and_a_selection() {
    let (c, _d, s) = open();
    item(&c, 1, "a");
    question(&c, 1, 0, &h64('1'), 1);
    s.put(&format!("items/{}/p1.json", id(1)), b"body").unwrap();
    let k = keys();
    let b = body(json!({ "item_id": id(1), "questions": [0] }));
    let pv = purge_preview(&PoolCtx::new(&c, 1_000), &k, ADM, &b).unwrap();
    assert_eq!((pv["large"].clone(), pv["expires_in"].clone()), (json!(false), json!(600)));
    // No token, then a wrong admin's token: refused, nothing written.
    assert_eq!(code(purge_route(&PoolCtx::new(&c, 1_000), &s, &k, ADM, &b)).1, "preview_required");
    let other = purge_preview(&PoolCtx::new(&c, 1_000), &k, "someone-else", &b).unwrap();
    let mut bad_tok = b.clone();
    bad_tok.insert("confirm_preview".into(), other["confirm_preview"].clone());
    assert_eq!(code(purge_route(&PoolCtx::new(&c, 1_000), &s, &k, ADM, &bad_tok)).1, "preview_required");
    assert_eq!(n(&c, "SELECT COUNT(*) FROM auth_audit WHERE action = 'pool.purge_privacy'"), 0);
    let mut ok = b.clone();
    ok.insert("confirm_preview".into(), pv["confirm_preview"].clone());
    let (st, v) = purge_route(&PoolCtx::new(&c, 1_000), &s, &k, ADM, &ok).unwrap();
    assert_eq!((st, v["ok"].clone(), v["blocked"]["questions"].clone()), (200, json!(true), json!(1)));
    assert_eq!(c.query_row::<String, _, _>("SELECT removed_reason FROM items", [], |r| r.get(0)).unwrap(), "privacy");
    assert_eq!(n(&c, "SELECT COUNT(*) FROM auth_audit WHERE action = 'pool.purge_privacy'"), 1);
    // The token was used up.
    assert_eq!(code(purge_route(&PoolCtx::new(&c, 1_000), &s, &k, ADM, &ok)).1, "preview_required");
}

#[test]
fn a_privacy_purge_with_nothing_selected_is_refused() {
    let (c, _d, s) = open();
    item(&c, 1, "a");
    question(&c, 1, 0, &h64('1'), 1);
    let k = keys();
    let b = body(json!({ "item_id": id(1), "questions": [] }));
    let pv = purge_preview(&PoolCtx::new(&c, 1_000), &k, ADM, &b).unwrap();
    let mut ok = b.clone();
    ok.insert("confirm_preview".into(), pv["confirm_preview"].clone());
    assert_eq!(code(purge_route(&PoolCtx::new(&c, 1_000), &s, &k, ADM, &ok)), (400, "selection_required".into()));
}

#[test]
fn a_takedown_is_previewed_then_confirmed_with_the_same_notice_and_days() {
    let (c, _d, s) = open();
    item(&c, 1, "a");
    question(&c, 1, 0, &h64('1'), 2);
    item(&c, 2, "b");
    question(&c, 2, 0, &h64('1'), 1);
    c.execute("INSERT INTO dmca_notices (id, received_at, complainant_name, complainant_contact, work_desc, material_desc, has_signature, has_good_faith, has_perjury_stmt, status, recorded_by) VALUES (?1, 1, 'C', 'c@x', 'w', 'm', 1, 1, 1, 'open', 'admin')", [id(9)]).unwrap();
    let k = keys();
    let b = body(json!({ "item_id": id(1), "notice_id": id(9), "questions": "all", "block_days": 30, "rightful_item_id": id(2) }));
    let pv = takedown_preview(&PoolCtx::new(&c, 1_000), &k, ADM, &b).unwrap();
    assert_eq!(pv["rightful_check"]["ok"], true, "{pv}");
    // A different block length than previewed is refused.
    let mut other = b.clone();
    other.insert("block_days".into(), json!(31));
    other.insert("confirm_preview".into(), pv["confirm_preview"].clone());
    assert_eq!(code(takedown_route(&PoolCtx::new(&c, 1_000), &s, &k, ADM, &other)).1, "preview_required");
    let mut ok = b.clone();
    ok.insert("confirm_preview".into(), pv["confirm_preview"].clone());
    let (st, v) = takedown_route(&PoolCtx::new(&c, 1_000), &s, &k, ADM, &ok).unwrap();
    assert_eq!((st, v["ok"].clone(), v["copies_more"].clone()), (200, json!(true), json!(false)));
    assert_eq!(n(&c, "SELECT COUNT(*) FROM dmca_actions"), 1);
    assert_eq!(n(&c, "SELECT expires_at FROM blocked_hashes LIMIT 1"), 1_000 + 30 * DAY_MS);
    assert_eq!(n(&c, "SELECT COUNT(*) FROM auth_audit WHERE action = 'pool.takedown'"), 1);
    assert_eq!(code(takedown_route(&PoolCtx::new(&c, 1_000), &s, &k, ADM, &b)).1, "preview_required");
}

#[test]
fn a_rightful_item_that_was_not_first_fails_the_takedown() {
    let (c, _d, s) = open();
    item(&c, 1, "a");
    question(&c, 1, 0, &h64('1'), 1);
    item(&c, 2, "b");
    question(&c, 2, 0, &h64('1'), 5);
    c.execute("INSERT INTO dmca_notices (id, received_at, complainant_name, complainant_contact, work_desc, material_desc, has_signature, has_good_faith, has_perjury_stmt, status, recorded_by) VALUES (?1, 1, 'C', 'c@x', 'w', 'm', 1, 1, 1, 'open', 'admin')", [id(9)]).unwrap();
    let k = keys();
    let b = body(json!({ "item_id": id(1), "notice_id": id(9), "questions": "all", "rightful_item_id": id(2) }));
    let pv = takedown_preview(&PoolCtx::new(&c, 1_000), &k, ADM, &b).unwrap();
    let mut ok = b.clone();
    ok.insert("confirm_preview".into(), pv["confirm_preview"].clone());
    assert_eq!(code(takedown_route(&PoolCtx::new(&c, 1_000), &s, &k, ADM, &ok)), (409, "rightful_item_invalid".into()));
    assert_eq!(n(&c, "SELECT COUNT(*) FROM dmca_actions"), 0);
}

#[test]
fn notices_are_stored_open_or_deficient_and_listed_newest_first_with_a_cursor() {
    let (c, _d, _s) = open();
    let full = |extra: Value| {
        let mut m = body(json!({ "complainant_name": "C", "complainant_contact": "c@x.org", "work_desc": "work", "material_desc": "material" }));
        m.extend(extra.as_object().unwrap().clone());
        m
    };
    let (st, v) = create_notice(&PoolCtx::new(&c, 10), ADM, &full(json!({ "has_signature": true, "has_good_faith": true, "has_perjury_stmt": true }))).unwrap();
    assert_eq!((st, v["status"].clone()), (201, json!("open")));
    let (_, v2) = create_notice(&PoolCtx::new(&c, 20), ADM, &full(json!({ "has_signature": true }))).unwrap();
    assert_eq!(v2["status"], "deficient");
    assert_eq!(code(create_notice(&PoolCtx::new(&c, 20), ADM, &body(json!({ "complainant_name": "C" })))).1, "invalid_request");
    let l = list_dmca(&PoolCtx::new(&c, 30), "notices", Some(1), None).unwrap();
    assert_eq!((l["items"][0]["at"].clone(), l["items"].as_array().unwrap().len()), (json!(20), 1));
    let next = list_dmca(&PoolCtx::new(&c, 30), "notices", Some(1), l["next_cursor"].as_str()).unwrap();
    assert_eq!((next["items"][0]["at"].clone(), next["next_cursor"].clone()), (json!(10), Value::Null));
    assert_eq!(code(list_dmca(&PoolCtx::new(&c, 30), "nope", None, None)).0, 404);
    assert_eq!(n(&c, "SELECT COUNT(*) FROM pool_audit WHERE action = 'dmca.notice'"), 2);
}

#[test]
fn limits_are_set_for_one_live_account_or_for_everyone() {
    let (c, _d, _s) = open();
    let x = PoolCtx::new(&c, 50);
    let metric = super::super::sqlfrag::DEFAULT_LIMITS[0].0;
    let uid = "0123456789abcdef0123456789abcdef0123";
    assert_eq!(set_limit(&x, ADM, &body(json!({ "user_id": "*", "metric": metric, "value": 7 }))).unwrap().0, 200);
    assert_eq!(n(&c, "SELECT value FROM pool_limits WHERE user_id = '*'"), 7);
    // An id with no user row is written (only an erased account is refused, as in the Worker).
    set_limit(&x, ADM, &body(json!({ "user_id": uid, "metric": metric, "value": 9 }))).unwrap();
    assert_eq!(n(&c, "SELECT COUNT(*) FROM pool_limits WHERE user_id <> '*'"), 1);
    set_limit(&x, ADM, &body(json!({ "user_id": "*", "metric": metric, "value": 8 }))).unwrap();
    assert_eq!(n(&c, "SELECT value FROM pool_limits WHERE user_id = '*'"), 8);
    assert_eq!(code(set_limit(&x, ADM, &body(json!({ "user_id": "*", "metric": "nope", "value": 1 })))).1, "invalid_request");
    assert_eq!(code(set_limit(&x, ADM, &body(json!({ "user_id": "*", "metric": metric, "value": -1 })))).1, "invalid_request");
}

#[test]
fn a_flag_is_resolved_once_and_evidence_is_voided_and_restored() {
    let (c, _d, _s) = open();
    item(&c, 1, "a");
    c.execute("INSERT INTO admin_flags (id, kind, item_id, dedup, at) VALUES (?1, 'appeal', ?2, 'd', 5)", params![id(7), id(1)]).unwrap();
    let x = PoolCtx::new(&c, 100);
    resolve_flag(&x, ADM, &body(json!({ "flag_id": id(7) }))).unwrap();
    resolve_flag(&PoolCtx::new(&c, 200), ADM, &body(json!({ "flag_id": id(7) }))).unwrap();
    assert_eq!(n(&c, "SELECT resolved_at FROM admin_flags"), 100);
    // Evidence: neither or both of item_id/user_id is refused; a void marks the item, a restore clears it.
    assert_eq!(code(evidence(&x, ADM, &body(json!({ "action": "void" })))).1, "invalid_request");
    assert_eq!(code(evidence(&x, ADM, &body(json!({ "item_id": id(1), "action": "burn" })))).1, "invalid_request");
    let (_, v) = evidence(&x, ADM, &body(json!({ "item_id": id(1), "action": "void", "note": "sock puppet" }))).unwrap();
    assert_eq!(v["ok"], true);
    assert_eq!(n(&c, "SELECT COUNT(*) FROM void_items"), 1);
    evidence(&x, ADM, &body(json!({ "item_id": id(1), "action": "restore" }))).unwrap();
    assert_eq!(n(&c, "SELECT COUNT(*) FROM void_items"), 0);
    assert_eq!(n(&c, "SELECT COUNT(*) FROM pool_audit WHERE action IN ('evidence.void', 'evidence.restore')"), 2);
    assert_eq!(code(evidence(&x, ADM, &body(json!({ "item_id": id(1), "action": "restore", "replaced_by": id(2) })))).1, "invalid_request");
}

#[test]
fn lifting_a_privacy_block_releases_orphan_holds_and_an_unknown_one_is_404() {
    let (c, _d, s) = open();
    item(&c, 1, "a");
    item(&c, 2, "b");
    let hash = h64('c');
    c.execute("INSERT INTO privacy_blocks (kind, hash, item_id, at) VALUES ('blob', ?1, ?2, 5)", params![hash, id(1)]).unwrap();
    c.execute("INSERT INTO item_holds (item_id, reason, at, by, source) VALUES (?1, 'student_data', 5, 'x', 'privacy')", [id(2)]).unwrap();
    let x = PoolCtx::new(&c, 1_000);
    assert_eq!(privacy_unblock(&x, &s, ADM, &body(json!({ "kind": "blob", "hash": h64('d') }))).unwrap().0, 404);
    assert_eq!(code(privacy_unblock(&x, &s, ADM, &body(json!({ "kind": "blob", "hash": "short" })))).1, "invalid_request");
    let (st, v) = privacy_unblock(&x, &s, ADM, &body(json!({ "kind": "blob", "sha256": hash, "note": "false positive" }))).unwrap();
    assert_eq!((st, v["released"].clone(), v["bytes_lost"].clone(), v["needs_reupload"].clone()), (200, json!(1), json!(false), json!([])));
    assert_eq!(n(&c, "SELECT COUNT(*) FROM privacy_blocks"), 0);
    assert_eq!(n(&c, "SELECT COUNT(*) FROM item_holds"), 0);
    assert_eq!(n(&c, "SELECT COUNT(*) FROM pool_audit WHERE action = 'privacy.unblock'"), 1);
}

#[test]
fn an_appeal_decision_names_a_known_item_and_a_valid_decision() {
    let (c, _d, s) = open();
    let x = PoolCtx::new(&c, 100);
    assert_eq!(code(appeal_decision(&x, &s, ADM, &body(json!({ "item_id": id(1), "decision": "maybe" })))).1, "invalid_request");
    assert_eq!(appeal_decision(&x, &s, ADM, &body(json!({ "item_id": id(1), "decision": "restore" }))).unwrap(), (404, json!({ "error": "not_found" })));
    assert_eq!(privacy_restore_route(&x, &s, ADM, &body(json!({ "item_id": id(1) }))).unwrap().0, 409);
    let r = purge_held_copies_route(&x, &s, ADM, &body(json!({ "item_id": id(1) }))).unwrap();
    assert_eq!(r["total"], 0);
}
