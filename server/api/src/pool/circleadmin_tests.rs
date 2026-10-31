//! Tests for `circleadmin.rs` on banks made by real sends: the queue, the basis gate, holds, releases, removal, closing and the unblock release.
use super::*;
use crate::blobs::BlobStore;
use crate::db;
use crate::pool::circlecommit::commit_version;
use crate::pool::circles::create_circle;
use crate::pool::circlesendprep::{Pre, ReadSend, precheck, send_once};
use crate::pool::core::Core;
use crate::pool::fingerprint::Fingerprinter;
use crate::pool::poolkeys::PoolKeys;
use crate::pool::qhash::stem_key;
use rusqlite::Connection;
use std::path::Path;

const KEY: &str = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";
const ROOT: &str = "7f757fb737085dcaaad0727deb89a14c18141681dc56be0a215a592c715f8e3b";
const ADMIN: &str = "admin-0001";
const RID: &str = "11111111111111111111111111111111";

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
    for u in ["a", "b", "v"] {
        c.execute("INSERT INTO users (id, email, display_name, trust, created_at) VALUES (?1, ?2, ?3, 'teacher', 1)", params![u, format!("{u}@x.org"), format!("Teacher {u}")]).unwrap();
    }
    let body = json!({ "name": "Team" }).as_object().unwrap().clone();
    let cid = create_circle(&PoolCtx::new(&c, 1_000), "a", &body).unwrap().1["circle"]["id"].as_str().unwrap().to_string();
    c.execute("INSERT INTO circle_members (circle_id, user_id, role, joined_at, join_seq) VALUES (?1, 'b', 'editor', 10, 5)", [&cid]).unwrap();
    let s = BlobStore::new(d.path().join("blobs")).unwrap();
    W { c, _d: d, s, core: Core::new(Some(KEY.into())), fp: Fingerprinter::from_root_hex(ROOT).unwrap(), keys: PoolKeys::from_secret("test-secret-0123456789abcdef0123456789abcdef"), cid }
}
fn mc(stem: &str) -> Value {
    let words = ["apple", "river", "stone", "cloud", "ember", "maple", "orbit", "quartz"];
    let answers: Vec<Value> = words.iter().enumerate().map(|(i, t)| json!({ "text": t, "correct": i == 3, "locked": false })).collect();
    json!({ "id": "q", "stem": stem, "kind": { "Mc": { "answers": answers } }, "points": null, "difficulty": null, "topics": [], "image": null, "explanation": "because", "figure": null, "created": 1, "modified": 1 })
}
fn make_bank(w: &W) -> String {
    let x = PoolCtx::new(&w.c, 5_000);
    let e = SendEnv { x: &x, core: &w.core, fp: &w.fp, keys: &w.keys, store: &w.s };
    let m = json!({ "commit_id": "commit-ca-0001-commit-ca-0001", "license": "circle-only", "message": "first", "meta": { "title": "Quiz" }, "attest": { "own_work": true, "terms_version": crate::pool::license::POOL_TERMS_VERSION } });
    let m = m.as_object().unwrap().clone();
    let p = json!({ "schema_version": 1, "bank": { "id": "l", "name": "My bank", "questions": [mc("Stem A about rivers"), mc("Stem B about stones")], "created": 1, "modified": 1 } }).to_string().into_bytes();
    let read = ReadSend { raw: &p, meta: &m, payload: &p };
    let Pre::Go(pre) = precheck(&e, "b", &w.cid, None, &read).unwrap() else { panic!("replayed") };
    let spec = send_once(&e, "b", &w.cid, None, &pre, &read, true).unwrap();
    commit_version(&x, &w.s, &spec).unwrap().1["bank_id"].as_str().unwrap().to_string()
}
fn report(w: &W, bid: &str) {
    w.c.execute("INSERT INTO circle_reports (id, circle_id, bank_id, version, reporter_id, reason, at) VALUES (?1, ?2, ?3, 1, 'v', 'other', 7)", params![RID, w.cid, bid]).unwrap();
}
fn n(c: &Connection, sql: &str) -> i64 {
    c.query_row(sql, [], |r| r.get(0)).unwrap()
}
fn obj(v: Value) -> Map<String, Value> {
    v.as_object().unwrap().clone()
}
fn code<T: std::fmt::Debug>(r: Res<T>) -> (u16, String) {
    match r.unwrap_err() {
        Fail::Pool(p) => (p.status, p.code),
        o => panic!("{o:?}"),
    }
}
fn stem(text: &str) -> String {
    stem_key(&Fingerprinter::from_root_hex(ROOT).unwrap(), &Json::parse(json!({ "stem": text }).to_string().as_bytes()).unwrap())
}
const NOTE: &str = "reviewing the report";

#[test]
fn queues_and_metadata_are_paged_and_opening_a_circle_is_audited() {
    let w = world();
    let bid = make_bank(&w);
    report(&w, &bid);
    let x = PoolCtx::new(&w.c, 9_000);
    let r = list_reports(&x, None).unwrap();
    assert_eq!((r["reports"].as_array().unwrap().len(), r["reports"][0]["circle_name"].clone(), r["reports"][0]["holds"].clone(), r["next_cursor"].clone()), (1, json!("Team"), json!([]), Value::Null));
    assert_eq!(list_reports(&x, Some(&format!("7:{RID}"))).unwrap()["reports"], json!([]));
    let l = list_circles(&x, None, None).unwrap();
    assert_eq!((l["circles"][0]["members"].clone(), l["circles"][0]["banks"].clone(), l["circles"][0]["owner_id"].clone()), (json!(2), json!(1), json!("a")));
    assert_eq!(code(list_circles(&x, Some("b"), None)).0, 400);
    let (st, d) = circle_detail(&x, ADMIN, Some(&w.cid)).unwrap();
    assert_eq!((st, d["members"].as_array().unwrap().len(), d["banks"][0]["open_reports"].clone()), (200, 2, json!(1)));
    assert_eq!(n(&w.c, "SELECT COUNT(*) FROM pool_audit WHERE action = 'circle.admin_open'") + n(&w.c, "SELECT COUNT(*) FROM auth_audit WHERE action = 'pool.circle_open'"), 2);
    assert_eq!(circle_detail(&x, ADMIN, Some(&"0".repeat(32))).unwrap().0, 404);
    assert_eq!(code(circle_detail(&x, ADMIN, Some("nope"))).0, 400);
}

#[test]
fn circle_content_is_opened_only_on_a_basis_and_every_open_is_audited() {
    let w = world();
    let bid = make_bank(&w);
    let x = PoolCtx::new(&w.c, 9_000);
    let q = |b: Option<&str>, v: Option<&str>| {
        let mut m = obj(json!({ "id": bid }));
        if let Some(b) = b {
            m.insert("basis".into(), json!(b));
        }
        if let Some(v) = v {
            m.insert("version".into(), json!(v));
        }
        m
    };
    assert_eq!(code(bank_view(&x, &w.s, ADMIN, &q(None, None))), (400, "invalid_request".into()));
    assert_eq!(bank_view(&x, &w.s, ADMIN, &q(Some("report"), None)).unwrap().0, 403);
    report(&w, &bid);
    let (st, d) = bank_view(&x, &w.s, ADMIN, &q(Some("report"), None)).unwrap();
    assert_eq!((st, d["version"]["version"].clone(), d["payload"]["bank"]["questions"].as_array().map(Vec::len)), (200, json!(1), Some(2)));
    assert_eq!(n(&w.c, "SELECT COUNT(*) FROM pool_audit WHERE action = 'circle.admin_view'"), 1);
    assert_eq!(code(bank_view(&x, &w.s, ADMIN, &q(Some("report"), Some("0")))).0, 400);
    let (st, e) = bank_view(&x, &w.s, ADMIN, &q(Some("report"), Some("9"))).unwrap();
    assert_eq!((st, e["error"].clone()), (404, json!("version_not_retained")));
    // A privacy basis needs a student-data match; a copyright basis a block or a hold.
    assert_eq!(bank_view(&x, &w.s, ADMIN, &q(Some("privacy"), None)).unwrap().0, 403);
    w.c.execute("INSERT INTO circle_holds (bank_id, reason, source, at, by) VALUES (?1, 'copyright', 'blocklist', 1, NULL)", [&bid]).unwrap();
    assert_eq!(bank_view(&x, &w.s, ADMIN, &q(Some("copyright"), None)).unwrap().0, 200);
}

#[test]
fn hold_release_remove_and_close_decide_the_case() {
    let w = world();
    let bid = make_bank(&w);
    let x = PoolCtx::new(&w.c, 9_000);
    let body = |extra: Value| {
        let mut m = obj(json!({ "bank_id": bid, "note": NOTE }));
        m.extend(obj(extra));
        m
    };
    assert_eq!(code(hold(&x, ADMIN, &obj(json!({ "bank_id": bid, "note": "short" })))), (400, "invalid_request".into()));
    assert_eq!(hold(&x, ADMIN, &body(json!({}))).unwrap().1["changed"], json!(true));
    assert_eq!(hold(&x, ADMIN, &body(json!({}))).unwrap().1["changed"], json!(false));
    assert_eq!(n(&w.c, "SELECT COUNT(*) FROM auth_audit WHERE action = 'pool.circle_hold'"), 2);
    report(&w, &bid);
    let r = release(&x, ADMIN, &body(json!({}))).unwrap().1;
    assert_eq!((r["released"].clone(), r["dismissed"].clone()), (json!(1), json!(1)));
    assert_eq!(n(&w.c, "SELECT COUNT(*) FROM circle_holds"), 0);
    // One version: not withheld → 409; withheld → released.
    assert_eq!(release(&x, ADMIN, &body(json!({ "version": 1 }))).unwrap().0, 409);
    w.c.execute("UPDATE circle_versions SET withheld_at = 5", []).unwrap();
    assert_eq!(release(&x, ADMIN, &body(json!({ "version": 1 }))).unwrap(), (200, json!({ "ok": true, "released_version": 1 })));
    assert_eq!(n(&w.c, "SELECT COUNT(*) FROM circle_versions WHERE withheld_at IS NOT NULL"), 0);
    assert_eq!(code(release(&x, ADMIN, &body(json!({ "version": 0 })))).0, 400);
    // Evidence captured marks the open flags once.
    w.c.execute("INSERT INTO admin_flags (id, kind, item_id, publish_id, user_id, detail, at) VALUES ('f1', 'blocklist', ?1, 'v1', 'b', '{\"version\":1}', 1)", [&bid]).unwrap();
    assert_eq!(evidence_captured(&x, ADMIN, &body(json!({ "version": 1 }))).unwrap().1["flags"], json!(1));
    assert_eq!(evidence_captured(&x, ADMIN, &body(json!({}))).unwrap().1["flags"], json!(0));
    // Remove: removed by the admin, purge in 14 days, its flags resolved; again is not removable.
    assert_eq!(remove_bank(&x, ADMIN, &body(json!({}))).unwrap(), (200, json!({ "ok": true })));
    assert_eq!(n(&w.c, "SELECT purge_after FROM circle_banks"), 9_000 + CIRCLE_UNDO_MS);
    assert_eq!(n(&w.c, "SELECT COUNT(*) FROM admin_flags WHERE resolved_at IS NOT NULL"), 1);
    assert_eq!(remove_bank(&x, ADMIN, &body(json!({}))).unwrap().0, 409);
    assert_eq!(remove_bank(&x, ADMIN, &obj(json!({ "bank_id": "0".repeat(32), "note": NOTE }))).unwrap().0, 404);
    // Close the circle.
    let cb = obj(json!({ "circle_id": w.cid, "note": NOTE }));
    assert_eq!(close_circle(&x, ADMIN, &cb).unwrap(), (200, json!({ "ok": true, "purge_after": 9_000 + CIRCLE_UNDO_MS })));
    assert_eq!(n(&w.c, "SELECT COUNT(*) FROM circles WHERE close_reason = 'admin' AND undo_until IS NULL"), 1);
    assert_eq!(close_circle(&x, ADMIN, &cb).unwrap().0, 409);
}

#[test]
fn lifting_a_takedown_releases_the_versions_only_it_withheld() {
    let w = world();
    let bid = make_bank(&w);
    w.c.execute("INSERT INTO blocked_hashes (kind, hash, action_id, at) VALUES ('match', ?1, 'act1', 1)", [stem("Stem A about rivers")]).unwrap();
    // Withheld by a send's copyright match: its hold, flag (naming the takedown) and withheld version.
    let vid: String = w.c.query_row("SELECT id FROM circle_versions", [], |r| r.get(0)).unwrap();
    w.c.execute("UPDATE circle_versions SET withheld_at = 5", []).unwrap();
    w.c.execute("INSERT INTO circle_holds (bank_id, reason, source, at, by) VALUES (?1, 'copyright', 'blocklist', 1, NULL)", [&bid]).unwrap();
    w.c.execute("INSERT INTO admin_flags (id, kind, item_id, publish_id, user_id, detail, at) VALUES ('f1', 'blocklist', ?1, ?2, 'b', ?3, 1)", params![bid, vid, json!({ "circle_id": w.cid, "version": 1, "actions": ["act1"] }).to_string()]).unwrap();
    let x = PoolCtx::new(&w.c, 9_000);
    let e = SendEnv { x: &x, core: &w.core, fp: &w.fp, keys: &w.keys, store: &w.s };
    // The takedown is still active: nothing is lifted.
    assert_eq!(circle_unblock(&e, "act1", true, ADMIN).unwrap(), (0, 0));
    w.c.execute("UPDATE blocked_hashes SET lifted_at = 2", []).unwrap();
    assert_eq!(circle_unblock(&e, "act1", false, ADMIN).unwrap(), (1, 0));
    assert_eq!(n(&w.c, "SELECT COUNT(*) FROM admin_flags WHERE detail ->> '$.block_lifted' = 1"), 1);
    assert_eq!(n(&w.c, "SELECT COUNT(*) FROM circle_versions WHERE withheld_at IS NOT NULL"), 1);
    assert_eq!(circle_unblock(&e, "act1", true, ADMIN).unwrap(), (1, 1));
    assert_eq!(n(&w.c, "SELECT COUNT(*) FROM circle_versions WHERE withheld_at IS NOT NULL"), 0);
    assert_eq!(n(&w.c, "SELECT COUNT(*) FROM circle_holds"), 0);
    assert_eq!(n(&w.c, "SELECT COUNT(*) FROM admin_flags WHERE resolved_at IS NOT NULL"), 1);
    assert_eq!(n(&w.c, "SELECT COUNT(*) FROM pool_audit WHERE action = 'circle.admin_version_release'"), 1);
}
