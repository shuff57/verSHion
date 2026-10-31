//! Tests for `circlerollback.rs`: a rollback is a new version carrying a kept version's body, with the guards the Worker keeps.
use super::*;
use crate::blobs::BlobStore;
use crate::db;
use crate::pool::circles::create_circle;
use crate::pool::circlesendprep::{Pre, ReadSend, precheck, send_once};
use crate::pool::core::Core;
use crate::pool::ctx::PoolCtx;
use crate::pool::fingerprint::Fingerprinter;
use crate::pool::poolkeys::PoolKeys;
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
    for u in ["a", "b", "c"] {
        c.execute("INSERT INTO users (id, email, display_name, trust, created_at) VALUES (?1, ?2, ?3, 'teacher', 1)", params![u, format!("{u}@x.org"), format!("Teacher {u}")]).unwrap();
    }
    let body = json!({ "name": "Team" }).as_object().unwrap().clone();
    let cid = create_circle(&PoolCtx::new(&c, 1_000), "a", &body).unwrap().1["circle"]["id"].as_str().unwrap().to_string();
    for (u, seq) in [("b", 5), ("c", 6)] {
        c.execute("INSERT INTO circle_members (circle_id, user_id, role, joined_at, join_seq) VALUES (?1, ?2, 'editor', 10, ?3)", params![cid, u, seq]).unwrap();
    }
    let s = BlobStore::new(d.path().join("blobs")).unwrap();
    W { c, _d: d, s, core: Core::new(Some(KEY.into())), fp: Fingerprinter::from_root_hex(ROOT).unwrap(), keys: PoolKeys::from_secret("test-secret-0123456789abcdef0123456789abcdef"), cid }
}
fn n(c: &Connection, sql: &str) -> i64 {
    c.query_row(sql, [], |r| r.get(0)).unwrap()
}
fn mc(stem: &str, qid: Option<&str>) -> Value {
    let words = ["apple", "river", "stone", "cloud", "ember", "maple", "orbit", "quartz"];
    let answers: Vec<Value> = words.iter().enumerate().map(|(i, t)| json!({ "text": t, "correct": i == 3, "locked": false })).collect();
    let mut q = json!({ "id": "q", "stem": stem, "kind": { "Mc": { "answers": answers } }, "points": null, "difficulty": null, "topics": [], "image": null, "explanation": "because", "figure": null, "created": 1, "modified": 1 });
    if let Some(id) = qid {
        q["published"] = json!({ "qid": id });
    }
    q
}
fn payload(qs: Vec<Value>) -> Vec<u8> {
    json!({ "schema_version": 1, "bank": { "id": "local", "name": "My bank", "questions": qs, "created": 1, "modified": 1 } }).to_string().into_bytes()
}
fn meta(commit: &str, extra: Value) -> Map<String, Value> {
    let mut m = json!({ "commit_id": commit, "license": "circle-only", "meta": { "title": "Quiz" }, "attest": { "own_work": true, "terms_version": crate::pool::license::POOL_TERMS_VERSION } });
    for (k, v) in extra.as_object().unwrap() {
        m[k] = v.clone();
    }
    m.as_object().unwrap().clone()
}
fn env<'a>(w: &'a W, x: &'a PoolCtx<'a>) -> SendEnv<'a> {
    SendEnv { x, core: &w.core, fp: &w.fp, keys: &w.keys, store: &w.s }
}
fn send(w: &W, me: &str, bid: Option<&str>, qs: Vec<Value>, commit: &str, extra: Value) -> Value {
    let x = PoolCtx::new(&w.c, 5_000);
    let e = env(w, &x);
    let (m, p) = (meta(commit, extra), payload(qs));
    let read = ReadSend { raw: &p, meta: &m, payload: &p };
    let Pre::Go(pre) = precheck(&e, me, &w.cid, bid, &read).unwrap() else { panic!("replayed") };
    let spec = send_once(&e, me, &w.cid, bid, &pre, &read, true).unwrap();
    commit_version(&x, &w.s, &spec).unwrap().1
}
fn qids(v: &Value) -> Vec<String> {
    v["qids"].as_array().unwrap().iter().map(|q| q.as_str().unwrap().to_string()).collect()
}
fn roll(w: &W, me: &str, bid: &str, body: Value) -> Res<(u16, Value)> {
    let x = PoolCtx::new(&w.c, 6_000);
    rollback_bank(&env(w, &x), me, &w.cid, bid, body.as_object().unwrap())
}
fn code(r: Res<(u16, Value)>) -> (u16, String) {
    match r.unwrap_err() {
        Fail::Pool(p) => (p.status, p.code),
        o => panic!("{o:?}"),
    }
}
fn used(c: &Connection) -> i64 {
    c.query_row("SELECT COALESCE(SUM(count), 0) FROM usage_daily WHERE metric = 'circle_rollbacks'", [], |r| r.get(0)).unwrap()
}

#[test]
fn a_rollback_is_a_new_version_with_an_old_body_and_the_old_qids() {
    let w = world();
    let one = send(&w, "b", None, vec![mc("First stem about rivers", None), mc("Second stem about stones", None)], "commit-one-commit-one-1", json!({}));
    let bid = one["bank_id"].as_str().unwrap().to_string();
    let q1 = qids(&one);
    let two = send(&w, "b", Some(&bid), vec![mc("First stem about rivers", Some(&q1[0])), mc("Second stem reworded entirely", Some(&q1[1])), mc("Third stem about clouds", None)], "commit-two-commit-two-2", json!({ "base_version": 1 }));
    assert_eq!(two["version"], json!(2));
    let (status, r) = roll(&w, "a", &bid, json!({ "commit_id": "commit-rb-commit-rb-001", "to_version": 1, "base_version": 2, "message": "back" })).unwrap();
    assert_eq!((status, r["version"].clone(), r["rollback_of"].clone(), qids(&r)), (200, json!(3), json!(1), q1.clone()));
    assert_eq!((r["added"].clone(), r["changed"].clone(), r["removed"].clone()), (json!(0), json!(1), json!(1)));
    let k = |v: i64| w.c.query_row::<String, _, _>("SELECT full_key FROM circle_versions WHERE version = ?1", [v], |row| row.get(0)).unwrap();
    assert_eq!(w.s.get(&k(3)).unwrap(), w.s.get(&k(1)).unwrap(), "the body is the target's, byte for byte");
    assert_eq!(n(&w.c, "SELECT head_version FROM circle_banks"), 3);
    assert_eq!(n(&w.c, "SELECT question_count FROM circle_banks"), 2);
    assert_eq!((n(&w.c, "SELECT COUNT(*) FROM pool_audit WHERE action = 'circle.rollback'"), n(&w.c, "SELECT COUNT(*) FROM attestations")), (1, 2), "a rollback sends no new work");
    assert_eq!(n(&w.c, "SELECT rollback_of FROM circle_versions WHERE version = 3"), 1);
    assert_eq!(used(&w.c), 1);
    // The same commit_id again replays the answer and gives the day's charge back.
    let (status, again) = roll(&w, "a", &bid, json!({ "commit_id": "commit-rb-commit-rb-001", "to_version": 1, "base_version": 2, "message": "back" })).unwrap();
    assert_eq!((status, again["replayed"].clone(), again["version"].clone()), (200, json!(true), json!(3)));
    assert_eq!((used(&w.c), n(&w.c, "SELECT COUNT(*) FROM circle_versions")), (1, 3));
}

#[test]
fn rollbacks_are_for_the_owner_or_the_author_and_refuse_bad_requests() {
    let w = world();
    let one = send(&w, "b", None, vec![mc("First stem about rivers", None)], "commit-one-commit-one-1", json!({}));
    let bid = one["bank_id"].as_str().unwrap().to_string();
    let q1 = qids(&one);
    send(&w, "b", Some(&bid), vec![mc("First stem about rivers", Some(&q1[0])), mc("Added stem about hills", None)], "commit-two-commit-two-2", json!({ "base_version": 1 }));
    let req = |commit: &str, to: Value, base: Value| json!({ "commit_id": commit, "to_version": to, "base_version": base });
    // c is an editor who is neither the owner nor the author.
    assert_eq!(code(roll(&w, "c", &bid, req("commit-rb-commit-rb-001", json!(1), json!(2)))), (403, "circle_role_required".to_string()));
    assert_eq!(code(roll(&w, "a", &bid, req("short", json!(1), json!(2)))), (400, "invalid_request".to_string()));
    assert_eq!(code(roll(&w, "a", &bid, req("commit-rb-commit-rb-002", json!(2), json!(2)))), (400, "invalid_request".to_string()));
    assert_eq!(code(roll(&w, "a", &bid, req("commit-rb-commit-rb-003", json!(0), json!(2)))), (400, "invalid_request".to_string()));
    assert_eq!(used(&w.c), 0);
    // A stale base is a conflict and is not charged.
    assert_eq!(code(roll(&w, "a", &bid, req("commit-rb-commit-rb-004", json!(1), json!(5)))), (409, "conflict".to_string()));
    assert_eq!(used(&w.c), 0);
    // A version that does not exist; one that is pruned.
    assert_eq!(code(roll(&w, "a", &bid, req("commit-rb-commit-rb-005", json!(1), json!(3)))).0, 409, "base 3 is not the head");
    w.c.execute("UPDATE circle_versions SET status = 'pruned', pruned_at = 1 WHERE version = 1", []).unwrap();
    assert_eq!(code(roll(&w, "a", &bid, req("commit-rb-commit-rb-006", json!(1), json!(2)))), (404, "version_not_retained".to_string()));
    // The author while an editor may (b wrote it), and a withheld target is refused.
    w.c.execute("UPDATE circle_versions SET status = 'previous', pruned_at = NULL, withheld_at = 7 WHERE version = 1", []).unwrap();
    assert_eq!(code(roll(&w, "b", &bid, req("commit-rb-commit-rb-007", json!(1), json!(2)))), (404, "version_withheld".to_string()));
    w.c.execute("UPDATE circle_versions SET withheld_at = NULL WHERE version = 1", []).unwrap();
    assert_eq!(roll(&w, "b", &bid, req("commit-rb-commit-rb-008", json!(1), json!(2))).unwrap().0, 200);
}

#[test]
fn dropping_more_than_half_the_head_needs_confirmation_and_the_daily_cap_applies() {
    let w = world();
    let one = send(&w, "b", None, vec![mc("Stem one about rivers", None)], "commit-one-commit-one-1", json!({}));
    let bid = one["bank_id"].as_str().unwrap().to_string();
    let q1 = qids(&one);
    send(&w, "b", Some(&bid), vec![mc("Stem one about rivers", Some(&q1[0])), mc("Stem two about hills", None), mc("Stem three about lakes", None)], "commit-two-commit-two-2", json!({ "base_version": 1 }));
    // Going back to one question drops 2 of 3 questions.
    let req = |commit: &str, extra: Value| {
        let mut b = json!({ "commit_id": commit, "to_version": 1, "base_version": 2 });
        for (k, v) in extra.as_object().unwrap() {
            b[k] = v.clone();
        }
        b
    };
    let e = match roll(&w, "a", &bid, req("commit-rb-commit-rb-001", json!({}))).unwrap_err() {
        Fail::Pool(p) => p,
        o => panic!("{o:?}"),
    };
    assert_eq!((e.status, e.code.as_str(), e.extra["removed"].clone(), e.extra["total"].clone()), (428, "confirm_large_required", json!(2), json!(3)));
    assert_eq!(used(&w.c), 0, "a confirm request is not charged");
    assert_eq!(roll(&w, "a", &bid, req("commit-rb-commit-rb-001", json!({ "confirm_large_change": true }))).unwrap().1["version"], json!(3));
    assert_eq!(used(&w.c), 1);
    // The day's allowance runs out.
    w.c.execute("UPDATE usage_daily SET count = 1000 WHERE metric = 'circle_rollbacks'", []).unwrap();
    assert_eq!(code(roll(&w, "a", &bid, req("commit-rb-commit-rb-002", json!({})))), (429, "daily_limit".to_string()));
}
