//! Tests for `read.rs` on the real migrated schema and a real blob directory: what GET, HEAD, a conditional GET and an
//! import answer, what they record, and what they give back when they fail.
use super::*;
use crate::db;
use crate::pool::ctx::Fail;
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
const BODY: &[u8] = br#"{"questions":[1,2]}"#;
fn world() -> (Connection, tempfile::TempDir, BlobStore, String) {
    let (c, d) = open();
    user(&c, "a");
    user(&c, "r");
    let id = "1".repeat(32);
    c.execute(
        "INSERT INTO items (id, author_id, kind, visibility, current_publish_id, public_publish_id, public_version, version, title, subject, course, grade, kinds_mask, question_count,
            license, has_images, committed_at, created_at, updated_at, fold_rev) VALUES (?1, 'a', 'bank', 'public', 'pc', 'pc', 1, 1, 'T', '', '', '', 1, 1, 'CC-BY-4.0', 0, 100, 1, 1, 1)",
        [&id],
    )
    .unwrap();
    c.execute(
        "INSERT INTO item_publishes (publish_id, item_id, author_id, version, public_version, status, schema_version, hide_key, license, attested_at, attest_text_rev,
            acks_json, request_sha256, stage_token, full_key, stripped_key, full_bytes, title, subject, course, grade, kinds_mask, question_count, has_images, created_at)
         VALUES ('pc', ?1, 'a', 1, 1, 'current', 2, 0, 'CC-BY-4.0', 1, 'r', '{}', 'x', 'pc', 'items/a/full', 'items/a/stripped', 10, '', '', '', '', 0, 1, 1, 1)",
        [&id],
    )
    .unwrap();
    let store = BlobStore::new(d.path().join("blobs")).unwrap();
    store.put("items/a/full", BODY).unwrap();
    (c, d, store, id)
}
fn rd<'a>(x: &'a PoolCtx<'a>, store: &'a BlobStore, opens: &'a OpenReads, me: &'a str) -> Reader<'a> {
    Reader { x, store, opens, me, addr: "net" }
}
fn count(c: &Connection, sql: &str) -> i64 {
    c.query_row(sql, [], |r| r.get(0)).unwrap()
}
fn body_json(out: ReadOut) -> (Value, Streamed) {
    match out {
        ReadOut::Body(s) => (serde_json::from_slice(&s.bytes()).unwrap(), s),
        other => panic!("{other:?}"),
    }
}
fn pool_err(r: Res<ReadOut>) -> (u16, String) {
    match r {
        Err(Fail::Pool(e)) => (e.status, e.code),
        other => panic!("{other:?}"),
    }
}

#[test]
fn a_read_splices_the_stored_payload_unchanged_into_the_head_and_records_a_view() {
    let (c, _d, store, id) = world();
    let (x, opens) = (PoolCtx::new(&c, 1_000), OpenReads::new());
    let (j, s) = body_json(rd(&x, &store, &opens, "r").get_item(&id, false, None).unwrap());
    assert_eq!(j["payload"], json!({"questions": [1, 2]}));
    assert_eq!((j["item"]["id"].clone(), j["item"]["form"].clone()), (json!(id), json!("full")));
    assert!(j["item"]["tags"].is_array() && j["item"]["standards"].is_array() && j["license_check"]["status"] == "ok" && j["questions"].is_array());
    assert!(s.bytes().ends_with(b"{\"questions\":[1,2]}}"), "the stored bytes are not re-serialised");
    assert_eq!(s.etag.as_deref(), Some(etag_of(&s.head).as_str()));
    assert_eq!(count(&c, "SELECT COUNT(*) FROM served WHERE reader_id = 'r' AND kind = 'view'"), 1);
    assert_eq!(opens.open("u:r"), 1, "the slot is held while the reply is");
    drop(s);
    assert_eq!(opens.open("u:r"), 0);
}

#[test]
fn the_author_reads_without_leaving_a_record() {
    let (c, _d, store, id) = world();
    let (x, opens) = (PoolCtx::new(&c, 1_000), OpenReads::new());
    let (j, _s) = body_json(rd(&x, &store, &opens, "a").get_item(&id, false, None).unwrap());
    assert_eq!((j["item"]["is_author"].clone(), j["item"]["visibility"].clone()), (json!(true), json!("public")));
    assert_eq!(count(&c, "SELECT COUNT(*) FROM served"), 0);
}

#[test]
fn head_and_a_matching_etag_serve_no_body_and_record_nothing() {
    let (c, _d, store, id) = world();
    let (x, opens) = (PoolCtx::new(&c, 1_000), OpenReads::new());
    let (_, full) = body_json(rd(&x, &store, &opens, "r").get_item(&id, false, None).unwrap());
    let etag = full.etag.clone().unwrap();
    let want_len = full.bytes().len() as u64;
    drop(full);
    c.execute("DELETE FROM served", []).unwrap();
    match rd(&x, &store, &opens, "r").get_item(&id, true, None).unwrap() {
        ReadOut::Head { etag: e, content_length } => assert_eq!((e, content_length), (etag.clone(), want_len)),
        other => panic!("{other:?}"),
    }
    for header in [etag.clone(), format!("W/{etag}"), format!("\"nope\", {etag}"), "*".to_string()] {
        assert!(matches!(rd(&x, &store, &opens, "r").get_item(&id, false, Some(&header)).unwrap(), ReadOut::NotModified { .. }), "{header}");
    }
    assert!(matches!(rd(&x, &store, &opens, "r").get_item(&id, false, Some("\"other\"")).unwrap(), ReadOut::Body(_)));
    assert_eq!(count(&c, "SELECT COUNT(*) FROM served"), 1, "only the unconditional read recorded");
    assert_eq!(opens.open("u:r"), 0);
}

#[test]
fn a_bad_id_a_hidden_item_and_a_missing_body_are_all_the_same_404_and_count_as_misses() {
    let (c, _d, store, id) = world();
    let (x, opens) = (PoolCtx::new(&c, 1_000), OpenReads::new());
    assert_eq!(pool_err(rd(&x, &store, &opens, "r").get_item("nope", false, None)), (404, "not_found".into()));
    assert_eq!(pool_err(rd(&x, &store, &opens, "r").get_item(&"2".repeat(32), false, None)), (404, "not_found".into()));
    store.delete("items/a/full").unwrap();
    assert_eq!(pool_err(rd(&x, &store, &opens, "r").get_item(&id, false, None)), (404, "not_found".into()));
    assert_eq!(pool_err(rd(&x, &store, &opens, "r").get_item(&id, true, None)), (404, "not_found".into()));
    assert_eq!(count(&c, "SELECT count FROM usage_daily WHERE user_id = 'r' AND metric = 'item_miss'"), 4);
    assert_eq!(opens.open("u:r"), 0, "a failed read gives its slot back");
    store.put("items/a/full", BODY).unwrap();
    c.execute("INSERT INTO item_holds (item_id, reason, at) VALUES (?1, 'admin', 1)", [&id]).unwrap();
    assert_eq!(pool_err(rd(&x, &store, &opens, "r").get_item(&id, false, None)).0, 404);
    assert_eq!(count(&c, "SELECT COUNT(*) FROM served"), 0);
}

#[test]
fn a_hidden_key_publish_is_read_in_its_stripped_form_by_others() {
    let (c, _d, store, id) = world();
    c.execute("UPDATE item_publishes SET hide_key = 1", []).unwrap();
    store.put("items/a/stripped", br#"{"stripped":true}"#).unwrap();
    let (x, opens) = (PoolCtx::new(&c, 1_000), OpenReads::new());
    let (j, _s) = body_json(rd(&x, &store, &opens, "r").get_item(&id, false, None).unwrap());
    assert_eq!((j["item"]["form"].clone(), j["payload"].clone()), (json!("stripped"), json!({"stripped": true})));
    let (j, _s) = body_json(rd(&x, &store, &opens, "a").get_item(&id, false, None).unwrap());
    assert_eq!(j["payload"], json!({"questions": [1, 2]}));
}

#[test]
fn an_import_records_binds_and_reports_what_was_held_and_what_came_before() {
    let (c, _d, store, id) = world();
    let (x, opens) = (PoolCtx::new(&c, 1_000), OpenReads::new());
    let (j, _s) = body_json(rd(&x, &store, &opens, "r").import_item(&id).unwrap());
    assert_eq!((j["recorded"].clone(), j["license"].clone()), (json!(true), json!("CC-BY-4.0")));
    assert_eq!((j["previously_imported_versions"].clone(), j["images"].clone()), (json!([]), json!({"held": 0})));
    assert!(j["may_republish_as"].as_array().unwrap().len() == LICENSES.len(), "nothing else held: every licence is open");
    assert_eq!(count(&c, "SELECT COUNT(*) FROM imports WHERE user_id = 'r' AND kind = 'import'"), 1);
    assert_eq!(count(&c, "SELECT COUNT(*) FROM served WHERE reader_id = 'r' AND kind = 'import'"), 1);
    assert_eq!(count(&c, "SELECT count FROM usage_daily WHERE user_id = 'r' AND metric = 'imports'"), 1);
    let (j, _s) = body_json(rd(&x, &store, &opens, "r").import_item(&id).unwrap());
    assert_eq!(j["previously_imported_versions"], json!([1]));
}

#[test]
fn an_authors_own_import_records_nothing_and_every_licence_is_open() {
    let (c, _d, store, id) = world();
    let (x, opens) = (PoolCtx::new(&c, 1_000), OpenReads::new());
    let (j, _s) = body_json(rd(&x, &store, &opens, "a").import_item(&id).unwrap());
    assert_eq!((j["recorded"].clone(), j["may_republish_as"].as_array().unwrap().len()), (json!(false), LICENSES.len()));
    assert_eq!(count(&c, "SELECT COUNT(*) FROM imports") + count(&c, "SELECT COUNT(*) FROM served"), 0);
}

#[test]
fn a_failed_import_gives_its_allowance_back_and_circle_only_is_a_miss_for_others() {
    let (c, _d, store, id) = world();
    let (x, opens) = (PoolCtx::new(&c, 1_000), OpenReads::new());
    assert_eq!(pool_err(rd(&x, &store, &opens, "r").import_item(&"2".repeat(32))).0, 404);
    assert_eq!(count(&c, "SELECT count FROM usage_daily WHERE user_id = 'r' AND metric = 'imports'"), 0, "refunded");
    c.execute("UPDATE item_publishes SET license = 'circle-only'", []).unwrap();
    assert_eq!(pool_err(rd(&x, &store, &opens, "r").import_item(&id)).0, 404);
    assert_eq!(count(&c, "SELECT count FROM usage_daily WHERE user_id = 'r' AND metric = 'imports'"), 0);
    assert_eq!(count(&c, "SELECT COUNT(*) FROM imports"), 0);
    let live = rd(&x, &store, &opens, "a").import_item(&id).unwrap();
    assert!(matches!(live, ReadOut::Body(_)), "its author may");
    assert_eq!(opens.open("u:r") + opens.open("u:a"), 1, "only the live reply holds a slot");
}

#[test]
fn the_pool_is_closed_to_reads_that_record_while_a_backfill_runs() {
    let (c, _d, store, id) = world();
    c.execute("UPDATE items SET fold_rev = 0", []).unwrap();
    let (x, opens) = (PoolCtx::new(&c, 1_000), OpenReads::new());
    assert_eq!(pool_err(rd(&x, &store, &opens, "r").get_item(&id, false, None)), (503, "pool_migrating".into()));
    assert_eq!(pool_err(rd(&x, &store, &opens, "r").import_item(&id)), (503, "pool_migrating".into()));
    assert!(matches!(rd(&x, &store, &opens, "a").get_item(&id, false, None).unwrap(), ReadOut::Body(_)), "the author's read records nothing");
}

#[test]
fn the_prefix_and_the_weak_comparison_match_the_worker() {
    assert_eq!(payload_prefix(r#"{"a":1}"#, "payload"), br#"{"a":1,"payload":"#.to_vec());
    assert_eq!(payload_prefix("{}", "payload"), br#"{"payload":"#.to_vec());
    assert!(if_none_match_hits(r#"W/"x", "y""#, "\"y\""));
    assert!(!if_none_match_hits(r#""x""#, "\"y\""));
}
