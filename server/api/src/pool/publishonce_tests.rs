//! Tests for `publishonce.rs`: the whole publish over the real pipeline (a new item, a replay, a republish, a dry run, and the refusals the front of the
//! publish owns).
use super::*;
use crate::db;
use crate::pool::ctx::PoolCtx;
use crate::pool::sqlfrag::DAY_MS;
use rusqlite::Connection;
use std::path::Path;

const KEY: &str = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";
const ROOT: &str = "7f757fb737085dcaaad0727deb89a14c18141681dc56be0a215a592c715f8e3b";
const NOW: i64 = 5 * DAY_MS;
const CT: Option<&str> = Some("application/vnd.vershion.publish");

struct W {
    c: Connection,
    s: BlobStore,
    core: Core,
    fp: Fingerprinter,
    _d: tempfile::TempDir,
}
fn world() -> W {
    let dir = tempfile::tempdir().unwrap();
    let mut c = db::open(&dir.path().join("t.db")).unwrap();
    db::migrate(&mut c, &Path::new(env!("CARGO_MANIFEST_DIR")).join("../worker/migrations")).unwrap();
    c.execute("INSERT INTO users (id, email, display_name, trust, created_at) VALUES ('me', 'me@x.org', 'Me', 'teacher', 1)", []).unwrap();
    let s = BlobStore::new(dir.path().join("blobs")).unwrap();
    W { c, s, core: Core::new(Some(KEY.into())), fp: Fingerprinter::from_root_hex(ROOT).unwrap(), _d: dir }
}
fn mc(stem: &str) -> Value {
    let words = ["apple", "river", "stone", "cloud", "ember", "maple", "orbit", "quartz"];
    let answers: Vec<Value> = words.iter().enumerate().map(|(i, t)| json!({ "text": t, "correct": i == 3, "locked": false })).collect();
    json!({ "id": "q", "stem": stem, "kind": { "Mc": { "answers": answers } }, "points": null, "difficulty": null, "topics": [], "image": null, "explanation": "because", "figure": null, "created": 1, "modified": 1 })
}
fn body(publish_id: &str, base: Option<&str>, stem: &str, schema: i64) -> Vec<u8> {
    let mut meta = json!({
        "publish_id": publish_id, "hide_key": false, "license": "CC-BY-4.0", "visibility": "unlisted",
        "attestation": { "accepted": true, "text_rev": "2026-10-19" },
        "attest": { "own_work": true, "terms_version": crate::pool::license::POOL_TERMS_VERSION },
        "meta": { "title": "Q", "subject": "S" }
    });
    if let Some(b) = base {
        meta["base_publish_id"] = json!(b);
    }
    let payload = json!({ "schema_version": schema, "bank": { "id": "local", "name": "B", "questions": [mc(stem)], "created": 1, "modified": 1 } });
    let mut out = meta.to_string().into_bytes();
    out.push(b'\n');
    out.extend(payload.to_string().into_bytes());
    out
}
fn run(w: &W, now: i64, item: Option<&str>, ct: Option<&str>, b: &[u8], dry: bool) -> Res<Outcome> {
    let x = PoolCtx::new(&w.c, now);
    publish(&x, &Env { store: &w.s, core: &w.core, fp: &w.fp }, "me", item, ct, b, dry, &mut || Ok(()))
}
fn count(w: &W, sql: &str) -> i64 {
    w.c.query_row(sql, [], |r| r.get(0)).unwrap()
}
fn err(r: Res<Outcome>) -> PoolError {
    match r {
        Err(Fail::Pool(e)) => e,
        other => panic!("expected a pool error, got {other:?}"),
    }
}
const P1: &str = "AAAAAAAAAAAAAAAAAAAAAA";
const P2: &str = "BBBBBBBBBBBBBBBBBBBBBB";

#[test]
fn a_new_item_is_created_and_a_replay_answers_the_same_result() {
    let w = world();
    let b = body(P1, None, "What is the capital of France?", 2);
    let o = run(&w, NOW, None, CT, &b, false).unwrap();
    assert_eq!(o.status, 201);
    let item = o.body["item_id"].as_str().unwrap().to_string();
    assert_eq!((o.body["version"].clone(), o.body["kind"].clone(), o.body["public_version"].clone()), (json!(1), json!("bank"), json!(1)));
    assert_eq!(o.body["qids"].as_array().unwrap().len(), 1);
    assert_eq!(count(&w, "SELECT COUNT(*) FROM items"), 1);
    assert_eq!(count(&w, "SELECT COUNT(*) FROM item_publishes WHERE status = 'current'"), 1);
    assert_eq!(count(&w, "SELECT COUNT(*) FROM item_questions"), 1);
    // The same request again: the stored result, marked replayed, status 200, nothing new.
    let r = run(&w, NOW + 1000, None, CT, &b, false).unwrap();
    assert_eq!(r.status, 200);
    assert_eq!(r.body["replayed"], json!(true));
    assert_eq!(r.body["item_id"], json!(item));
    assert_eq!(count(&w, "SELECT COUNT(*) FROM items"), 1);
    // The lock and the refusal count are given back.
    assert_eq!(count(&w, "SELECT COUNT(*) FROM pool_locks"), 0);
}

#[test]
fn the_same_publish_id_with_another_body_is_a_conflict() {
    let w = world();
    run(&w, NOW, None, CT, &body(P1, None, "What is the capital of France?", 2), false).unwrap();
    let e = err(run(&w, NOW + 1000, None, CT, &body(P1, None, "What is the capital of Spain?", 2), false));
    assert_eq!((e.status, e.code.as_str()), (409, "publish_id_conflict"));
}

#[test]
fn a_republish_moves_the_item_on_and_a_stale_base_is_a_conflict() {
    let w = world();
    let first = run(&w, NOW, None, CT, &body(P1, None, "What is the capital of France?", 2), false).unwrap();
    let item = first.body["item_id"].as_str().unwrap().to_string();
    let second = run(&w, NOW + 1000, Some(&item), CT, &body(P2, Some(P1), "What is the capital of France, in Europe?", 2), false).unwrap();
    assert_eq!(second.status, 200);
    assert_eq!(second.body["version"], json!(2));
    assert_eq!(count(&w, "SELECT COUNT(*) FROM items WHERE current_publish_id = 'BBBBBBBBBBBBBBBBBBBBBB'"), 1);
    let e = err(run(&w, NOW + 2000, Some(&item), CT, &body("CCCCCCCCCCCCCCCCCCCCCC", Some(P1), "Another stem entirely here?", 2), false));
    assert_eq!((e.status, e.code.as_str()), (409, "conflict"));
    assert_eq!(e.extra.get("current_version"), Some(&json!(2)));
}

#[test]
fn a_republish_needs_schema_two_and_the_right_kind() {
    let w = world();
    let first = run(&w, NOW, None, CT, &body(P1, None, "What is the capital of France?", 2), false).unwrap();
    let item = first.body["item_id"].as_str().unwrap().to_string();
    let e = err(run(&w, NOW + 1000, Some(&item), CT, &body(P2, Some(P1), "What is the capital of France, in Europe?", 1), false));
    assert_eq!((e.status, e.code.as_str()), (422, "schema_upgrade_required"));
}

#[test]
fn a_dry_run_stores_nothing_and_does_not_use_up_the_publish_allowance() {
    let w = world();
    let o = run(&w, NOW, None, CT, &body(P1, None, "What is the capital of France?", 2), true).unwrap();
    assert_eq!(o.status, 200);
    assert_eq!((o.body["ok"].clone(), o.body["dry_run"].clone()), (json!(true), json!(true)));
    assert_eq!(count(&w, "SELECT COUNT(*) FROM items"), 0);
    assert_eq!(count(&w, "SELECT COUNT(*) FROM item_publishes"), 0);
    assert_eq!(count(&w, "SELECT COUNT(*) FROM usage_daily WHERE metric = 'publishes'"), 0);
    assert_eq!(count(&w, "SELECT COUNT(*) FROM usage_daily WHERE metric = 'previews'"), 1);
}

#[test]
fn the_wrong_media_type_and_a_missing_attestation_are_refused() {
    let w = world();
    let b = body(P1, None, "What is the capital of France?", 2);
    let e = err(run(&w, NOW, None, Some("application/json"), &b, false));
    assert_eq!((e.status, e.code.as_str()), (415, "unsupported_media_type"));
    let mut meta = json!({ "publish_id": P1, "hide_key": false, "license": "CC-BY-4.0", "visibility": "unlisted", "attestation": { "accepted": true, "text_rev": "2026-10-19" }, "meta": { "title": "Q", "subject": "S" } });
    meta["x"] = json!(1);
    let mut raw = meta.to_string().into_bytes();
    raw.push(b'\n');
    raw.extend(b"{}");
    let e = err(run(&w, NOW, None, CT, &raw, false));
    assert_eq!((e.status, e.code.as_str()), (422, "attestation_required"));
    // A refused publish keeps its count; a conflict or a busy server does not.
    assert_eq!(count(&w, "SELECT COALESCE(SUM(count), 0) FROM usage_daily WHERE metric = 'publish_refusals'"), 2);
}

#[test]
fn an_item_id_that_is_not_an_id_is_a_404_and_a_second_publish_in_flight_is_a_409() {
    let w = world();
    let b = body(P1, None, "What is the capital of France?", 2);
    let e = err(run(&w, NOW, Some("nope"), CT, &b, false));
    assert_eq!((e.status, e.code.as_str()), (404, "not_found"));
    w.c.execute("INSERT INTO pool_locks (user_id, token, until) VALUES ('me', 'other', ?1)", [NOW + 60_000]).unwrap();
    let e = err(run(&w, NOW, None, CT, &b, false));
    assert_eq!((e.status, e.code.as_str()), (409, "publish_in_progress"));
}
