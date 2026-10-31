//! Tests for `circlehidepatch.rs`: hiding keys on and off on banks made by real sends, with student-data and copyright blocks.
use super::*;
use crate::blobs::BlobStore;
use crate::db;
use crate::pool::circlecommit::commit_version;
use crate::pool::circles::create_circle;
use crate::pool::circlesendprep::{Pre, ReadSend, precheck, send_once};
use crate::pool::core::Core;
use crate::pool::ctx::PoolCtx;
use crate::pool::fingerprint::Fingerprinter;
use crate::pool::poolkeys::PoolKeys;
use crate::pool::qhash::stem_key;
use rusqlite::Connection;
use std::path::Path;

const KEY: &str = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";
const ROOT: &str = "7f757fb737085dcaaad0727deb89a14c18141681dc56be0a215a592c715f8e3b";

struct W {
    c: Connection,
    _d: tempfile::TempDir,
    s: BlobStore,
    core: Core,
    fp: Fingerprinter,
    keys: PoolKeys,
    cid: String,
}
fn world() -> W {
    let d = tempfile::tempdir().unwrap();
    let mut c = db::open(&d.path().join("t.db")).unwrap();
    db::migrate(&mut c, &Path::new(env!("CARGO_MANIFEST_DIR")).join("../worker/migrations")).unwrap();
    for u in ["a", "b", "c", "v"] {
        c.execute("INSERT INTO users (id, email, display_name, trust, created_at) VALUES (?1, ?2, ?3, 'teacher', 1)", params![u, format!("{u}@x.org"), format!("Teacher {u}")]).unwrap();
    }
    let body = json!({ "name": "Team" }).as_object().unwrap().clone();
    let cid = create_circle(&PoolCtx::new(&c, 1_000), "a", &body).unwrap().1["circle"]["id"].as_str().unwrap().to_string();
    for (u, role, seq) in [("b", "editor", 5), ("c", "editor", 6), ("v", "viewer", 7)] {
        c.execute("INSERT INTO circle_members (circle_id, user_id, role, joined_at, join_seq) VALUES (?1, ?2, ?3, 10, ?4)", params![cid, u, role, seq]).unwrap();
    }
    let s = BlobStore::new(d.path().join("blobs")).unwrap();
    W { c, _d: d, s, core: Core::new(Some(KEY.into())), fp: Fingerprinter::from_root_hex(ROOT).unwrap(), keys: PoolKeys::from_secret("test-secret-0123456789abcdef0123456789abcdef"), cid }
}
fn n(c: &Connection, sql: &str) -> i64 {
    c.query_row(sql, [], |r| r.get(0)).unwrap()
}
fn mc(stem: &str) -> Value {
    let words = ["apple", "river", "stone", "cloud", "ember", "maple", "orbit", "quartz"];
    let answers: Vec<Value> = words.iter().enumerate().map(|(i, t)| json!({ "text": t, "correct": i == 3, "locked": false })).collect();
    json!({ "id": "q", "stem": stem, "kind": { "Mc": { "answers": answers } }, "points": null, "difficulty": null, "topics": [], "image": null, "explanation": "because", "figure": null, "created": 1, "modified": 1 })
}
fn make_bank(w: &W) -> String {
    let x = PoolCtx::new(&w.c, 5_000);
    let e = SendEnv { x: &x, core: &w.core, fp: &w.fp, keys: &w.keys, store: &w.s };
    let m = json!({ "commit_id": "commit-hp-0001-commit-hp-0001", "license": "circle-only", "message": "first", "meta": { "title": "Quiz" }, "attest": { "own_work": true, "terms_version": crate::pool::license::POOL_TERMS_VERSION } });
    let m = m.as_object().unwrap().clone();
    let p = json!({ "schema_version": 1, "bank": { "id": "l", "name": "My bank", "questions": [mc("Stem A about rivers"), mc("Stem B about stones")], "created": 1, "modified": 1 } }).to_string().into_bytes();
    let read = ReadSend { raw: &p, meta: &m, payload: &p };
    let Pre::Go(pre) = precheck(&e, "b", &w.cid, None, &read).unwrap() else { panic!("replayed") };
    let spec = send_once(&e, "b", &w.cid, None, &pre, &read, true).unwrap();
    commit_version(&x, &w.s, &spec).unwrap().1["bank_id"].as_str().unwrap().to_string()
}
/// PATCH as `me`, 9_000 ms into the day; (status, code) of a refusal.
fn patch(w: &W, me: &str, bid: &str, body: Value) -> Result<Value, (u16, String)> {
    let x = PoolCtx::new(&w.c, 9_000);
    let e = SendEnv { x: &x, core: &w.core, fp: &w.fp, keys: &w.keys, store: &w.s };
    let (m, b) = crate::pool::circlebankedit::patch_gate(&x, me, &w.cid, bid).map_err(|f| fail(f))?;
    patch_bank(&e, me, &w.cid, &m, &b, body.as_object().unwrap()).map_err(fail)
}
fn fail(f: Fail) -> (u16, String) {
    match f {
        Fail::Pool(p) => (p.status, p.code),
        o => panic!("{o:?}"),
    }
}
fn stem(text: &str) -> String {
    stem_key(&Fingerprinter::from_root_hex(ROOT).unwrap(), &Json::parse(json!({ "stem": text }).to_string().as_bytes()).unwrap())
}

#[test]
fn hiding_on_records_the_decisions_once_and_off_restores_the_text() {
    let w = world();
    let bid = make_bank(&w);
    let r = patch(&w, "a", &bid, json!({ "hide_key": true })).unwrap();
    assert_eq!((r["ok"].clone(), r["hide_key"].clone(), r["held"].clone(), r["withheld"].clone()), (json!(true), json!(true), json!(false), json!(false)));
    assert_eq!(n(&w.c, "SELECT hide_key FROM circle_banks"), 1);
    assert_eq!(n(&w.c, "SELECT COUNT(*) FROM circle_versions WHERE strip_opts IS NOT NULL"), 1);
    assert_eq!(n(&w.c, "SELECT COUNT(*) FROM pool_audit WHERE action = 'circle.bank_hide_key'"), 1);
    // The same again changes nothing: no audit line, no count.
    assert_eq!(patch(&w, "a", &bid, json!({ "hide_key": true })).unwrap()["unchanged"], json!(true));
    assert_eq!(n(&w.c, "SELECT COUNT(*) FROM pool_audit WHERE action LIKE 'circle.bank_%hide%'"), 1);
    assert_eq!(n(&w.c, "SELECT count FROM usage_daily WHERE metric = 'circle_hide_changes'"), 1);
    // Off, by the bank's author while an editor.
    let r = patch(&w, "b", &bid, json!({ "hide_key": false })).unwrap();
    assert_eq!((r["hide_key"].clone(), r.get("unchanged").cloned(), r.get("restored").cloned()), (json!(false), None, None));
    assert_eq!(n(&w.c, "SELECT hide_key FROM circle_banks"), 0);
    assert_eq!(patch(&w, "a", &bid, json!({ "hide_key": false })).unwrap()["unchanged"], json!(true));
    assert_eq!(n(&w.c, "SELECT COUNT(*) FROM pool_audit WHERE action = 'circle.bank_hide_key'"), 2);
}

#[test]
fn the_request_is_checked_before_any_work() {
    let w = world();
    let bid = make_bank(&w);
    let c400 = |r: Result<Value, (u16, String)>| r.unwrap_err();
    assert_eq!(c400(patch(&w, "a", &bid, json!({ "hide_key": "yes" }))), (400, "invalid_request".into()));
    assert_eq!(c400(patch(&w, "a", &bid, json!({ "hide_key": true, "license": "circle-only" }))), (400, "invalid_request".into()));
    assert_eq!(c400(patch(&w, "a", &bid, json!({ "hide_key": false, "image_ok": [0] }))), (400, "invalid_request".into()));
    assert_eq!(c400(patch(&w, "a", &bid, json!({ "hide_key": true, "base_version": 0 }))), (400, "invalid_request".into()));
    assert_eq!(c400(patch(&w, "a", &bid, json!({ "hide_key": true, "bogus": 1 }))), (400, "invalid_request".into()));
    assert_eq!(c400(patch(&w, "a", &bid, json!({ "hide_key": true, "image_ok": [5] }))), (400, "invalid_request".into()));
    // A version sent meanwhile: the decisions were made on version 2 of a bank at version 1.
    assert_eq!(c400(patch(&w, "a", &bid, json!({ "hide_key": true, "base_version": 2 }))), (409, "conflict".into()));
    // Neither the owner nor the author: refused at the gate; the author-demoted viewer cannot either.
    assert_eq!(c400(patch(&w, "c", &bid, json!({ "hide_key": true }))), (403, "circle_role_required".into()));
    w.c.execute("UPDATE circle_members SET role = 'viewer' WHERE user_id = 'b'", []).unwrap();
    assert_eq!(c400(patch(&w, "b", &bid, json!({ "hide_key": true }))), (403, "circle_role_required".into()));
    // Nothing was charged for a refusal.
    assert_eq!(n(&w.c, "SELECT COUNT(*) FROM usage_daily WHERE metric LIKE 'circle_hide%'"), 0);
    // A bad request has no licence either way: an empty body names the licence.
    assert_eq!(c400(patch(&w, "a", &bid, json!({}))), (400, "invalid_request".into()));
}

#[test]
fn a_student_data_match_on_the_viewers_form_withholds_the_head_and_holds_the_bank() {
    let w = world();
    let bid = make_bank(&w);
    w.c.execute("INSERT INTO privacy_blocks (kind, hash, item_id, at) VALUES ('match', ?1, 'i', 1)", [stem("Stem A about rivers")]).unwrap();
    let r = patch(&w, "a", &bid, json!({ "hide_key": true })).unwrap();
    assert_eq!((r["held"].clone(), r["withheld"].clone()), (json!(true), json!(true)));
    assert_eq!(n(&w.c, "SELECT COUNT(*) FROM circle_holds WHERE source = 'privacy'"), 1);
    assert_eq!(n(&w.c, "SELECT COUNT(*) FROM circle_versions WHERE withheld_at IS NOT NULL"), 1);
    assert_eq!(n(&w.c, "SELECT COUNT(*) FROM admin_flags WHERE kind = 'privacy_text_match' AND detail ->> '$.via' = 'hide_key'"), 1);
    // A student-data match is never released by turning hiding off.
    let r = patch(&w, "a", &bid, json!({ "hide_key": false })).unwrap();
    assert_eq!(r.get("restored"), None);
    assert_eq!(n(&w.c, "SELECT COUNT(*) FROM circle_versions WHERE withheld_at IS NOT NULL"), 1);
    assert_eq!(n(&w.c, "SELECT COUNT(*) FROM circle_holds WHERE source = 'privacy'"), 1);
}

#[test]
fn a_copyright_match_is_released_when_hiding_is_turned_off_and_the_full_form_is_clean() {
    let w = world();
    let bid = make_bank(&w);
    w.c.execute("INSERT INTO blocked_hashes (kind, hash, action_id, at) VALUES ('match', ?1, 'act1', 1)", [stem("Stem A about rivers")]).unwrap();
    let r = patch(&w, "a", &bid, json!({ "hide_key": true })).unwrap();
    assert_eq!((r["held"].clone(), r["withheld"].clone()), (json!(true), json!(true)));
    assert_eq!(n(&w.c, "SELECT COUNT(*) FROM circle_holds WHERE source = 'blocklist'"), 1);
    assert_eq!(n(&w.c, "SELECT COUNT(*) FROM admin_flags WHERE kind = 'blocklist' AND detail ->> '$.via' = 'hide_key'"), 1);
    // The block is lifted meanwhile, so the full form is clean now; turning hiding off releases the version, its flag and the hold.
    w.c.execute("UPDATE blocked_hashes SET lifted_at = 2", []).unwrap();
    let x = PoolCtx::new(&w.c, 9_000);
    let e = SendEnv { x: &x, core: &w.core, fp: &w.fp, keys: &w.keys, store: &w.s };
    let vid: String = w.c.query_row("SELECT id FROM circle_versions", [], |r| r.get(0)).unwrap();
    assert!(hide_off_releases(&e, "a", &bid, &vid));
    let r = patch(&w, "a", &bid, json!({ "hide_key": false })).unwrap();
    assert_eq!(r["restored"], json!([1]));
    assert_eq!(n(&w.c, "SELECT COUNT(*) FROM circle_versions WHERE withheld_at IS NOT NULL"), 0);
    assert_eq!(n(&w.c, "SELECT COUNT(*) FROM circle_holds"), 0);
    assert_eq!(n(&w.c, "SELECT COUNT(*) FROM admin_flags WHERE kind = 'blocklist' AND resolved_at IS NOT NULL AND detail ->> '$.resolved_via' = 'hide_key_off'"), 1);
}
