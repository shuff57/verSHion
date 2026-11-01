//! Tests for `publishinsert.rs`: a real pipeline staged end to end (rows, bodies, status), the id-reservation and quota refusals, the discard on a failed write,
//! and the small pure helpers.
use super::*;
use crate::db;
use crate::pool::core::Core;
use crate::pool::fingerprint::Fingerprinter;
use crate::pool::publishderive::derive;
use crate::pool::publishevid::{Evid, collect};
use crate::pool::publishfacts::gather;
use crate::pool::publishmeta::read_meta;
use crate::pool::publishrewrite::rewrite;
use crate::pool::publishstage::{limits, sequences};
use crate::pool::publishstart::Landing;
use crate::pool::publishunits::Units;
use crate::pool::sqlfrag::DAY_MS;
use rusqlite::Connection;
use std::path::Path;

const KEY: &str = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";
const ROOT: &str = "7f757fb737085dcaaad0727deb89a14c18141681dc56be0a215a592c715f8e3b";
const ITEM: &str = "0123456789abcdef0123456789abcdef";

struct W {
    c: Connection,
    d: tempfile::TempDir,
    s: BlobStore,
    core: Core,
    fp: super::super::fingerprint::Fingerprinter,
}
fn world() -> W {
    let dir = tempfile::tempdir().unwrap();
    let mut c = db::open(&dir.path().join("t.db")).unwrap();
    db::migrate(&mut c, &Path::new(env!("CARGO_MANIFEST_DIR")).join("../worker/migrations")).unwrap();
    c.execute("INSERT INTO users (id, email, display_name, trust, created_at) VALUES ('me', 'me@x.org', 'Me', 'teacher', 1)", []).unwrap();
    let s = BlobStore::new(dir.path().join("blobs")).unwrap();
    W { c, s, core: Core::new(Some(KEY.into())), fp: Fingerprinter::from_root_hex(ROOT).unwrap(), d: dir }
}
fn mc(stem: &str) -> Value {
    let words = ["apple", "river", "stone", "cloud", "ember", "maple", "orbit", "quartz"];
    let answers: Vec<Value> = words.iter().enumerate().map(|(i, t)| json!({ "text": t, "correct": i == 3, "locked": false })).collect();
    json!({ "id": "q", "stem": stem, "kind": { "Mc": { "answers": answers } }, "points": null, "difficulty": null, "topics": ["Geography"], "image": null, "explanation": "because", "figure": null, "created": 1, "modified": 1 })
}
struct Run {
    m: PublishMeta,
    rw: Rewritten,
    h: Hashed,
    f: Facts,
    d: Derived,
    s: Seqs,
    plan: Plan,
    charge: i64,
}
fn pipeline(w: &W, hide_key: bool) -> Run {
    let payload = json!({ "schema_version": 1, "bank": { "id": "local", "name": "Capitals", "questions": [mc("What is the capital of France?"), mc("What is the capital of Spain?")], "created": 1, "modified": 1 } }).to_string().into_bytes();
    let m = json!({ "publish_id": "AAAAAAAAAAAAAAAAAAAAAA", "hide_key": hide_key, "license": "CC-BY-4.0", "visibility": "unlisted", "attestation": { "accepted": true, "text_rev": "2026-10-19" }, "meta": { "title": "Capitals", "subject": "Geo", "tags": ["Maps"] } });
    let m = read_meta(m.as_object().unwrap(), true).unwrap();
    let land = Landing { item_id: ITEM.into(), version: 1, visibility: "unlisted".into(), item_kind: None, base_of: None, prev_public_version: 0, prev_public_at: None, prev_hidden: false, prev_license: None };
    let x = PoolCtx::new(&w.c, 5 * DAY_MS);
    let rw = rewrite(&x, &w.s, &w.core, "me", &land, true, &m, &payload, false).unwrap();
    let h = crate::pool::publishhash::prepare(&x, &w.s, &w.core, &w.fp, &land, true, &m, &rw).unwrap();
    let f = gather(&x, &w.s, &w.fp, "me", None, true, false, &m, &rw, &h, None).unwrap();
    let e: Evid = collect(&x, &w.fp, "me", &m, &rw, &h, &f, &Units::default()).unwrap();
    let u = Units::default();
    let d = derive(&m, &rw, &h, &f, &e, &u, None, false);
    let s = sequences(&x, &rw, &h, &f, &u, None).unwrap();
    let (plan, charge) = plan(&m, &rw, &h, true);
    Run { m, rw, h, f, d, s, plan, charge }
}
fn stage_it(w: &W, r: &Run) -> Res<Staged> {
    let x = PoolCtx::new(&w.c, 5 * DAY_MS);
    let l = limits(&x, "me").unwrap();
    let served = r.plan.shas.clone();
    stage(
        &x,
        &w.s,
        &StageIn { me: "me", item_id: ITEM, version: 1, request_sha: "rq", m: &r.m, rw: &r.rw, h: &r.h, f: &r.f, d: &r.d, s: &r.s, plan: &r.plan, charge: r.charge, limits: l, search_words: &r.h.search_words, served_shas: &served },
    )
}
fn count(w: &W, sql: &str) -> i64 {
    w.c.query_row(sql, [], |r| r.get(0)).unwrap()
}

#[test]
fn a_fresh_bank_is_staged_with_every_row_and_both_bodies_written() {
    let w = world();
    let r = pipeline(&w, true);
    let st = stage_it(&w, &r).unwrap();
    let (status, title, qcount, kinds, bytes): (String, String, i64, i64, i64) =
        w.c.query_row("SELECT status, title, question_count, kinds_mask, full_bytes FROM item_publishes WHERE publish_id = 'AAAAAAAAAAAAAAAAAAAAAA'", [], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?))).unwrap();
    assert_eq!((status.as_str(), title.as_str(), qcount, kinds), ("staged", "Capitals", 2, 1));
    assert_eq!(bytes as usize, r.h.prep.full.bytes.len());
    assert_eq!(count(&w, "SELECT COUNT(*) FROM item_questions WHERE publish_id = 'AAAAAAAAAAAAAAAAAAAAAA'"), 2);
    assert_eq!(count(&w, "SELECT COUNT(*) FROM question_ids"), 2);
    assert!(count(&w, "SELECT COUNT(*) FROM served_words") > 0);
    assert_eq!(count(&w, "SELECT COUNT(*) FROM item_tags WHERE tag IN ('maps', 'geography')"), 2);
    assert!(w.s.get(&st.full_key).unwrap().is_some() && w.s.get(st.stripped_key.as_deref().unwrap()).unwrap().is_some());
    // The stored allowed_json and chain are what the derivation decided.
    let (aj, chain, seq): (String, String, i64) = w.c.query_row("SELECT allowed_json, chain_json, seq FROM item_questions WHERE pos = 0", [], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?))).unwrap();
    assert!(aj.starts_with("{\"allowed\":") && chain == "[]");
    assert_eq!(seq, r.s.new_seq);
}

#[test]
fn a_shown_key_publish_has_no_stripped_body() {
    let w = world();
    let r = pipeline(&w, false);
    let st = stage_it(&w, &r).unwrap();
    assert!(st.stripped_key.is_none());
    assert_eq!(count(&w, "SELECT COUNT(*) FROM item_questions WHERE stripped_content IS NOT NULL"), 0);
}

#[test]
fn the_same_publish_id_twice_is_in_progress_with_a_retry_after() {
    let w = world();
    let r = pipeline(&w, false);
    stage_it(&w, &r).unwrap();
    match stage_it(&w, &r) {
        Err(super::super::ctx::Fail::Pool(e)) => {
            assert_eq!((e.status, e.code.to_string()), (409, "publish_in_progress".to_string()));
            assert!(e.headers.iter().any(|(k, v)| k == "Retry-After" && v == "5"));
        }
        other => panic!("{other:?}"),
    }
}

#[test]
fn a_quota_that_stops_the_insert_is_named() {
    let w = world();
    let r = pipeline(&w, false);
    w.c.execute("INSERT INTO pool_limits (metric, user_id, value, updated_at, updated_by) VALUES ('storage_bytes', 'me', 5, 1, 'me')", []).unwrap();
    match stage_it(&w, &r) {
        Err(super::super::ctx::Fail::Pool(e)) => assert_eq!((e.status, e.code.to_string()), (413, "quota_exceeded".to_string())),
        other => panic!("{other:?}"),
    }
    assert_eq!(count(&w, "SELECT COUNT(*) FROM item_publishes"), 0);
}

#[test]
fn a_failed_write_discards_the_staged_publish_and_its_bodies() {
    let w = world();
    let mut r = pipeline(&w, false);
    // A file where the items/ directory must go makes the first body write fail after the staging row exists.
    std::fs::write(w.d.path().join("blobs").join("items"), b"x").unwrap();
    let _ = &mut r;
    assert!(stage_it(&w, &r).is_err());
    assert_eq!(count(&w, "SELECT COUNT(*) FROM item_publishes"), 0);
    assert_eq!(count(&w, "SELECT COUNT(*) FROM item_questions"), 0);
}

#[test]
fn marks_are_appended_in_order_and_never_duplicated() {
    let aj = "{\"allowed\":[\"CC0-1.0\"],\"binders\":[]}";
    assert_eq!(with_marks(aj, false, false, false), aj);
    assert_eq!(with_marks(aj, true, false, true), "{\"allowed\":[\"CC0-1.0\"],\"binders\":[],\"soft\":true,\"earlier\":true}");
    let once = with_marks(aj, true, true, false);
    assert_eq!(with_marks(&once, true, true, false), once);
    assert_eq!(with_marks("not json", true, false, false), "not json");
}

#[test]
fn image_shas_come_only_from_a_well_formed_media_path() {
    let sha = "a".repeat(64);
    assert_eq!(image_sha(&json!({ "image": { "file": format!("media/{sha}.png") } })), Some(sha.clone()));
    for bad in [json!({}), json!({ "image": null }), json!({ "image": { "file": format!("media/{sha}.svg") } }), json!({ "image": { "file": "media/short.png" } }), json!({ "image": { "file": format!("x/{sha}.png") } })] {
        assert_eq!(image_sha(&bad), None);
    }
}

#[test]
fn tags_merge_meta_tags_and_topics_lowercased() {
    let qs = vec![json!({ "topics": ["Rivers", " rivers ", "", 3] }), json!({})];
    let t = tags_of(&["Maps".into(), "RIVERS".into()], &qs);
    assert_eq!(t.into_iter().collect::<Vec<_>>(), vec!["maps", "rivers"]);
}
