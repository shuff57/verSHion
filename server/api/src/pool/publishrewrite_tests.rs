//! Tests for `publishrewrite.rs`: the author's own display text, the payload's shape, question ids (kept only when owned and unique), images (verified, the
//! author's, present), and the server's rewrite of ids, dates and provenance.
use super::*;
use crate::db;
use crate::pool::core::Meta;
use rusqlite::Connection;
use rusqlite::types::Value as Sql;
use std::path::Path;

const KEY: &str = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";
const PID: &str = "AAAAAAAAAAAAAAAAAAAAAA";
const ITEM: &str = "0123456789abcdef0123456789abcdef";

struct W {
    c: Connection,
    _d: tempfile::TempDir,
    s: BlobStore,
    core: Core,
}
fn world() -> W {
    let dir = tempfile::tempdir().unwrap();
    let mut c = db::open(&dir.path().join("t.db")).unwrap();
    db::migrate(&mut c, &Path::new(env!("CARGO_MANIFEST_DIR")).join("../worker/migrations")).unwrap();
    c.execute("INSERT INTO users (id, email, display_name, trust, created_at) VALUES ('me', 'me@x.org', 'me', 'teacher', 1)", []).unwrap();
    let s = BlobStore::new(dir.path().join("blobs")).unwrap();
    W { c, s, core: Core::new(Some(KEY.into())), _d: dir }
}
fn mc(stem: &str, qid: Option<&str>, image: Option<&str>) -> Value {
    let words = ["apple", "river", "stone", "cloud", "ember", "maple", "orbit", "quartz"];
    let answers: Vec<Value> = words.iter().enumerate().map(|(i, t)| json!({ "text": t, "correct": i == 3, "locked": false })).collect();
    let mut q = json!({ "id": "q", "stem": stem, "kind": { "Mc": { "answers": answers } }, "points": null, "difficulty": null, "topics": [], "image": null, "explanation": "because", "figure": null, "created": 1, "modified": 1 });
    if let Some(id) = qid {
        q["published"] = json!({ "qid": id });
    }
    if let Some(sha) = image {
        q["image"] = json!({ "file": format!("media/{sha}.png"), "width": 1, "height": 1 });
    }
    q
}
fn bank(qs: Vec<Value>, v: i64) -> Vec<u8> {
    json!({ "schema_version": v, "bank": { "id": "local", "name": "My bank", "questions": qs, "created": 1, "modified": 1, "origin": { "item_id": "x" } } }).to_string().into_bytes()
}
fn meta_for(is_new: bool, extra: Value) -> PublishMeta {
    let mut m = json!({
        "publish_id": PID, "hide_key": false, "license": "CC-BY-4.0", "visibility": "unlisted",
        "attestation": { "accepted": true, "text_rev": "2026-10-19" }, "meta": { "title": "Quiz", "subject": "Science" }
    });
    if !is_new {
        m["base_publish_id"] = json!("CCCCCCCCCCCCCCCCCCCCCC");
    }
    for (k, v) in extra.as_object().unwrap() {
        m[k] = v.clone();
    }
    super::super::publishmeta::read_meta(m.as_object().unwrap(), is_new).unwrap()
}
fn landing(is_new: bool) -> Landing {
    Landing {
        item_id: ITEM.into(),
        version: if is_new { 1 } else { 2 },
        visibility: "unlisted".into(),
        item_kind: (!is_new).then(|| "bank".into()),
        base_of: (!is_new).then(|| "CCCCCCCCCCCCCCCCCCCCCC".into()),
        prev_public_version: 0,
        prev_public_at: None,
        prev_hidden: false,
        prev_license: None,
    }
}
fn run(w: &W, is_new: bool, m: &PublishMeta, payload: &[u8], dry: bool) -> Res<Rewritten> {
    rewrite(&PoolCtx::new(&w.c, 5 * DAY_MS + 777), &w.s, &w.core, "me", &landing(is_new), is_new, m, payload, dry)
}
fn err(r: Res<Rewritten>) -> PoolError {
    match r.expect_err("refused") {
        Fail::Pool(p) => p,
        other => panic!("not a pool error: {other:?}"),
    }
}
fn meta_of(title: &str, tags: &[&str]) -> Meta {
    Meta { title: title.into(), subject: String::new(), course: String::new(), grade: String::new(), tags: tags.iter().map(|t| t.to_string()).collect(), standards: vec![] }
}

#[test]
fn the_authors_nfc_text_is_kept_when_share_core_changed_only_the_compatibility_fold() {
    // "x²" folds to "x2" under NFKC; the author's text stays when nothing else changed and it is no longer.
    assert_eq!(authors_text(Some(&json!("Area x²")), "Area x2"), "Area x²");
    assert_eq!(authors_text(Some(&json!("  Area   x² ")), "Area x2"), "Area x²", "whitespace runs collapse");
    // Share-core changed more than the fold (a control character dropped): its text.
    assert_eq!(authors_text(Some(&json!("Area\u{7}x²")), "Area x2"), "Area x2");
    // A ligature is one character where its fold is three: the author's text is no longer, so it stays.
    assert_eq!(authors_text(Some(&json!("ﬃ")), "ffi"), "ﬃ");
    // Longer than share-core's text: its text.
    assert_eq!(authors_text(Some(&json!("Area xx²")), "Area x2"), "Area x2");
    assert_eq!(authors_text(Some(&json!(5)), "kept"), "kept");
    assert_eq!(authors_text(None, "kept"), "kept");
}

#[test]
fn tags_keep_the_authors_form_once_each_and_come_back_sorted() {
    let raw = json!({ "tags": ["Water²", "water2", "Zeta", 7, "Alpha"] });
    let got = authors_meta(&raw, &meta_of("T", &["water2", "zeta", "alpha", "extra"]));
    assert_eq!(got.tags, vec!["alpha", "extra", "water²", "zeta"]);
}

#[test]
fn a_new_banks_ids_dates_and_provenance_are_the_servers() {
    let w = world();
    let payload = bank(vec![mc("First?", Some("not-mine"), None), mc("Second?", None, None)], 1);
    let r = run(&w, true, &meta_for(true, json!({})), &payload, false).unwrap();
    assert_eq!((r.kind, r.n), ("bank", 2));
    assert!(r.qids.iter().all(|q| is_id_hex(q)) && r.qids[0] != r.qids[1], "fresh ids");
    assert_eq!(r.new_qids, r.qids, "all new on a new item");
    let out: Value = serde_json::from_slice(&r.bytes).unwrap();
    let day = (5 * DAY_MS + 777).div_euclid(DAY_MS) * DAY_MS;
    assert_eq!((out["bank"]["id"].as_str(), out["bank"]["created"].as_i64(), out["bank"]["modified"].as_i64()), (Some(ITEM), Some(day), Some(day)));
    assert!(out["bank"].get("origin").is_none());
    for (i, q) in out["bank"]["questions"].as_array().unwrap().iter().enumerate() {
        assert_eq!((q["id"].as_str(), q["created"].as_i64()), (Some(r.qids[i].as_str()), Some(day)));
        assert!(q.get("published").is_none() && q.get("origin").is_none());
    }
    assert_eq!(r.meta.title, "Quiz");
}

fn is_id_hex(s: &str) -> bool {
    s.len() == 32 && s.bytes().all(|b| b.is_ascii_hexdigit())
}

#[test]
fn a_sent_qid_is_kept_only_when_it_belongs_to_the_item_and_appears_once() {
    let w = world();
    let own = "11111111111111111111111111111111";
    let other = "22222222222222222222222222222222";
    w.c.execute("INSERT INTO question_ids (qid, item_id, first_publish_id, created_at) VALUES (?1, ?2, 'p0', 1)", params![own, ITEM]).unwrap();
    w.c.execute("INSERT INTO question_ids (qid, item_id, first_publish_id, created_at) VALUES (?1, 'someotheritem', 'p0', 1)", params![other]).unwrap();
    let payload = bank(vec![mc("A", Some(own), None), mc("B", Some(other), None), mc("C", Some(own), None), mc("D", None, None)], 2);
    let r = run(&w, false, &meta_for(false, json!({})), &payload, false).unwrap();
    // `own` appears twice, so neither use is kept; another item's qid is never adopted.
    assert!(r.qids.iter().all(|q| q != own && q != other));
    let once = bank(vec![mc("A", Some(own), None), mc("B", None, None)], 2);
    let r = run(&w, false, &meta_for(false, json!({})), &once, false).unwrap();
    assert_eq!(r.qids[0], own);
    assert_eq!(r.new_qids, vec![r.qids[1].clone()], "only the new one is new");
}

#[test]
fn a_republish_needs_schema_two_and_the_same_kind() {
    let w = world();
    let v1 = bank(vec![mc("A", None, None)], 1);
    let e = err(run(&w, false, &meta_for(false, json!({})), &v1, false));
    assert_eq!((e.status, e.code.as_str()), (422, "schema_upgrade_required"));
    let single = json!({ "schema_version": 2, "question": mc("A", None, None) }).to_string().into_bytes();
    assert_eq!(err(run(&w, false, &meta_for(false, json!({})), &single, false)).code, "kind_mismatch");
    assert!(run(&w, true, &meta_for(true, json!({})), &single, false).is_ok(), "a lone question is a fine new item");
}

#[test]
fn a_malformed_payload_gets_share_cores_refusal() {
    let w = world();
    let m = meta_for(true, json!({}));
    for bad in [b"not json".to_vec(), b"[]".to_vec(), br#"{"schema_version":1}"#.to_vec(), br#"{"schema_version":1,"bank":{"questions":[]}}"#.to_vec(), br#"{"schema_version":1,"bank":{"questions":[1]}}"#.to_vec()] {
        let e = err(run(&w, true, &m, &bad, false));
        assert_eq!(e.status, 422, "{}", String::from_utf8_lossy(&bad));
        assert!(e.extra.contains_key("errors"));
    }
    // A duplicate key is share-core's to refuse, before this side's parse could keep the last one.
    let dup = br#"{"schema_version":1,"schema_version":2,"bank":{"questions":[]}}"#;
    assert_eq!(err(run(&w, true, &m, dup, false)).status, 422);
}

fn put_blob(w: &W, sha: &str, purged: Option<i64>, bytes: bool) {
    w.c.execute("INSERT INTO blobs (sha256, size, content_type, width, height, first_uploaded_at, privacy_purged_at) VALUES (?1, 3, 'image/png', 7, 9, 1, ?2)", params![sha, purged]).unwrap();
    if bytes {
        w.s.put(&format!("blobs/{sha}"), b"png").unwrap();
    }
}
fn pend(w: &W, sha: &str, until: i64) {
    w.c.execute("INSERT INTO pending_blobs (sha256, uploader_id, size, until) VALUES (?1, 'me', 3, ?2)", params![sha, until]).unwrap();
}

#[test]
fn an_image_must_be_the_authors_and_present_and_is_rewritten_to_its_stored_form() {
    let w = world();
    let sha = "a".repeat(64);
    let payload = bank(vec![mc("Pic?", None, Some(&sha)), mc("Plain?", None, None)], 1);
    let m = meta_for(true, json!({}));
    // Unknown to the author: the position is named.
    let e = err(run(&w, true, &m, &payload, false));
    assert_eq!((e.status, e.code.as_str(), e.extra["positions"].clone()), (422, "blob_missing", json!([0])));
    put_blob(&w, &sha, None, true);
    assert_eq!(err(run(&w, true, &m, &payload, false)).code, "blob_missing", "stored but not the author's");
    pend(&w, &sha, 5 * DAY_MS + 777);
    let r = run(&w, true, &m, &payload, false).unwrap();
    let out: Value = serde_json::from_slice(&r.bytes).unwrap();
    assert_eq!(out["bank"]["questions"][0]["image"], json!({ "file": format!("media/{sha}.png"), "width": 7, "height": 9 }));
    assert_eq!(r.images, vec![Some(sha.clone()), None]);
    // The hold on the blob is extended by a day.
    assert_eq!(w.c.query_row::<i64, _, _>("SELECT until FROM pending_blobs WHERE sha256 = ?1", [&sha], |r| r.get(0)).unwrap(), 5 * DAY_MS + 777 + DAY_MS);
}

#[test]
fn a_dry_run_stores_nothing_and_a_missing_file_is_a_reupload() {
    let w = world();
    let sha = "b".repeat(64);
    put_blob(&w, &sha, None, false);
    pend(&w, &sha, 5 * DAY_MS + 777);
    let payload = bank(vec![mc("Pic?", None, Some(&sha)), mc("Again?", None, Some(&sha))], 1);
    let m = meta_for(true, json!({}));
    let e = err(run(&w, true, &m, &payload, true));
    assert_eq!((e.status, e.code.as_str(), e.extra["reason"].as_str(), e.extra["positions"].clone()), (422, "blob_missing", Some("reupload"), json!([0, 1])));
    assert_eq!(w.c.query_row::<Option<i64>, _, _>("SELECT missing_at FROM blobs", [], |r| r.get(0)).unwrap(), None, "a preview marks nothing");
    err(run(&w, true, &m, &payload, false));
    assert_eq!(w.c.query_row::<Option<i64>, _, _>("SELECT missing_at FROM blobs", [], |r| r.get(0)).unwrap(), Some(5 * DAY_MS + 777));
    // A privacy-blocked image's bytes are never required.
    let sha2 = "c".repeat(64);
    put_blob(&w, &sha2, Some(1), false);
    pend(&w, &sha2, 5 * DAY_MS + 777);
    let blocked = bank(vec![mc("Pic?", None, Some(&sha2))], 1);
    assert!(run(&w, true, &m, &blocked, false).is_ok());
    let _: Sql = Sql::Null;
}

#[test]
fn declared_origins_are_recorded_then_dropped() {
    let w = world();
    let mut q = mc("Copied?", None, None);
    q["origin"] = json!({ "item_id": "abc", "qid": "q9", "version": 4 });
    let mut q2 = mc("Odd?", None, None);
    q2["origin"] = json!({ "item_id": 7 });
    let r = run(&w, true, &meta_for(true, json!({})), &bank(vec![q, q2, mc("Plain?", None, None)], 1), false).unwrap();
    assert_eq!(r.declared, vec![Some(Declared { item: "abc".into(), qid: "q9".into(), version: 4 }), None, None]);
    let out: Value = serde_json::from_slice(&r.bytes).unwrap();
    assert!(out["bank"]["questions"].as_array().unwrap().iter().all(|q| q.get("origin").is_none()));
}
