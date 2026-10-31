//! Tests for `circlesendprep.rs`: the cheap checks before the heavy work and the payload becoming a commit spec, committed on the real schema.
use super::*;
use crate::db;
use crate::pool::circlecommit::commit_version;
use crate::pool::circles::create_circle;
use rusqlite::Connection;
use std::path::Path;

const SECRET: &str = "test-secret-0123456789abcdef0123456789abcdef";
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
    for u in ["a", "b"] {
        c.execute("INSERT INTO users (id, email, display_name, trust, created_at) VALUES (?1, ?2, ?3, 'teacher', 1)", params![u, format!("{u}@x.org"), format!("Teacher {u}")]).unwrap();
    }
    let body = json!({ "name": "Team" }).as_object().unwrap().clone();
    let cid = create_circle(&PoolCtx::new(&c, 1_000), "a", &body).unwrap().1["circle"]["id"].as_str().unwrap().to_string();
    c.execute("INSERT INTO circle_members (circle_id, user_id, role, joined_at, join_seq) VALUES (?1, 'b', 'editor', 10, 5)", [&cid]).unwrap();
    let s = BlobStore::new(d.path().join("blobs")).unwrap();
    W { c, _d: d, s, core: Core::new(Some(KEY.into())), fp: Fingerprinter::from_root_hex(ROOT).unwrap(), keys: PoolKeys::from_secret(SECRET), cid }
}
impl W {
    fn env<'a>(&'a self, x: &'a PoolCtx<'a>) -> SendEnv<'a> {
        SendEnv { x, core: &self.core, fp: &self.fp, keys: &self.keys, store: &self.s }
    }
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
    let mut m = json!({ "commit_id": commit, "license": "circle-only", "meta": { "title": "Quiz", "subject": "Science" }, "attest": { "own_work": true, "terms_version": crate::pool::license::POOL_TERMS_VERSION } });
    for (k, v) in extra.as_object().unwrap() {
        m[k] = v.clone();
    }
    m.as_object().unwrap().clone()
}
/// precheck then send_once then commit, as a route does.
fn send(w: &W, me: &str, bid: Option<&str>, payload: &[u8], meta: &Map<String, Value>) -> Res<(u16, Value)> {
    let x = PoolCtx::new(&w.c, 5_000);
    let env = w.env(&x);
    let raw = [serde_json::to_vec(meta).unwrap(), payload.to_vec()].concat();
    let read = ReadSend { raw: &raw, meta, payload };
    match precheck(&env, me, &w.cid, bid, &read)? {
        Pre::Replay(v) => Ok((200, v)),
        Pre::Go(pre) => {
            let spec = send_once(&env, me, &w.cid, bid, &pre, &read, true)?;
            commit_version(&x, &w.s, &spec)
        }
    }
}
fn code(r: Res<(u16, Value)>) -> (u16, String) {
    match r.unwrap_err() {
        Fail::Pool(p) => (p.status, p.code),
        o => panic!("{o:?}"),
    }
}

#[test]
fn a_new_bank_then_a_second_version_keep_qids_and_count_what_changed() {
    let w = world();
    let (status, one) = send(&w, "b", None, &payload(vec![mc("First stem about rivers", None), mc("Second stem about stones", None)]), &meta("commit-one-commit-one-1", json!({}))).unwrap();
    assert_eq!((status, one["version"].clone(), one["added"].clone(), one["changed"].clone(), one["removed"].clone()), (201, json!(1), json!(2), json!(0), json!(0)));
    let qids: Vec<String> = one["qids"].as_array().unwrap().iter().map(|q| q.as_str().unwrap().to_string()).collect();
    let bid = one["bank_id"].as_str().unwrap().to_string();
    assert_eq!(n(&w.c, "SELECT question_count FROM circle_banks"), 2);
    // The next version: question 1 unchanged (its qid sent), question 2 reworded (qid sent), a new third question.
    let next = payload(vec![mc("First stem about rivers", Some(&qids[0])), mc("Second stem about stones, reworded", Some(&qids[1])), mc("A brand new third stem", None)]);
    let (status, two) = send(&w, "a", Some(&bid), &next, &meta("commit-two-commit-two-2", json!({ "base_version": 1, "message": "more" }))).unwrap();
    assert_eq!((status, two["version"].clone(), two["added"].clone(), two["changed"].clone(), two["removed"].clone()), (200, json!(2), json!(1), json!(1), json!(0)));
    let q2: Vec<String> = two["qids"].as_array().unwrap().iter().map(|q| q.as_str().unwrap().to_string()).collect();
    assert_eq!(&q2[..2], &qids[..], "sent qids the head held are kept");
    assert!(!qids.contains(&q2[2]));
    // The stored body has its ids rewritten and carries no client provenance.
    let key = c_full_key(&w.c, 2);
    let stored: Value = serde_json::from_slice(&w.s.get(&key).unwrap().unwrap()).unwrap();
    assert_eq!(stored["bank"]["id"], json!(bid));
    assert!(stored["bank"]["questions"][0].get("published").is_none());
    assert_eq!(stored["bank"]["questions"][1]["id"], json!(qids[1]));
}
fn c_full_key(c: &Connection, v: i64) -> String {
    c.query_row("SELECT full_key FROM circle_versions WHERE version = ?1", [v], |r| r.get(0)).unwrap()
}

#[test]
fn a_repeated_commit_id_replays_and_a_stale_base_or_bad_merge_token_is_refused_early() {
    let w = world();
    let p = payload(vec![mc("Only stem about clouds", None)]);
    let m1 = meta("commit-one-commit-one-1", json!({}));
    let (_, one) = send(&w, "b", None, &p, &m1).unwrap();
    let bid = one["bank_id"].as_str().unwrap().to_string();
    let (status, again) = send(&w, "b", None, &p, &m1).unwrap();
    assert_eq!((status, again["replayed"].clone(), again["version_id"].clone()), (200, json!(true), one["version_id"].clone()));
    assert_eq!(n(&w.c, "SELECT COUNT(*) FROM circle_versions"), 1);
    // Based on a head that is not the head.
    assert_eq!(code(send(&w, "a", Some(&bid), &p, &meta("commit-stale-commit-stale", json!({ "base_version": 7 })))), (409, "conflict".to_string()));
    // A token for another bank, and a merged_from without its token.
    assert_eq!(code(send(&w, "a", Some(&bid), &p, &meta("commit-tok-commit-tok-1", json!({ "base_version": 1, "merge_token": "nonsense" })))), (422, "merge_token_invalid".to_string()));
    // An editor sending a different licence than the author's.
    assert_eq!(code(send(&w, "a", Some(&bid), &p, &meta("commit-lic-commit-lic-1", json!({ "base_version": 1, "license": "CC-BY-4.0" })))), (422, "license_author_only".to_string()));
    assert_eq!(n(&w.c, "SELECT COUNT(*) FROM circle_versions"), 1);
}

#[test]
fn a_valid_merge_token_records_where_the_merge_started() {
    let w = world();
    let (_, one) = send(&w, "b", None, &payload(vec![mc("Only stem about clouds", None)]), &meta("commit-one-commit-one-1", json!({}))).unwrap();
    let bid = one["bank_id"].as_str().unwrap().to_string();
    let q0 = one["qids"][0].as_str().unwrap().to_string();
    let mk = |commit: &str, base: i64, head: i64| w.keys.merge_token(5_000, "a", &bid, base, head).0.to_string() + &format!("#{commit}");
    let _ = mk;
    // head 2 first
    let (_, two) = send(&w, "a", Some(&bid), &payload(vec![mc("Only stem about clouds", Some(&q0)), mc("Added stem about fields", None)]), &meta("commit-two-commit-two-2", json!({ "base_version": 1 }))).unwrap();
    assert_eq!(two["version"], json!(2));
    let (tok, _) = w.keys.merge_token(5_000, "a", &bid, 1, 2);
    let q = two["qids"].as_array().unwrap().iter().map(|v| v.as_str().unwrap().to_string()).collect::<Vec<_>>();
    let next = payload(vec![mc("Only stem about clouds", Some(&q[0])), mc("Added stem about fields", Some(&q[1]))]);
    let (_, three) = send(&w, "a", Some(&bid), &next, &meta("commit-three-commit-three", json!({ "base_version": 2, "merge_token": tok, "merged_from": 1 }))).unwrap();
    assert_eq!((three["version"].clone(), three["merged_from"].clone()), (json!(3), json!(1)));
    assert_eq!(n(&w.c, "SELECT merged_from FROM circle_versions WHERE version = 3"), 1);
    // merged_from that disagrees with the token.
    let (tok2, _) = w.keys.merge_token(5_000, "a", &bid, 1, 3);
    assert_eq!(code(send(&w, "a", Some(&bid), &next, &meta("commit-four-commit-four", json!({ "base_version": 3, "merge_token": tok2, "merged_from": 2 })))), (400, "invalid_request".to_string()));
}

#[test]
fn payloads_that_are_not_a_bank_or_name_a_missing_image_are_refused() {
    let w = world();
    let one_q = json!({ "schema_version": 1, "question": mc("A single question", None) }).to_string().into_bytes();
    assert_eq!(code(send(&w, "b", None, &one_q, &meta("commit-one-commit-one-1", json!({})))), (422, "invalid_payload".to_string()));
    let empty = payload(vec![]);
    assert_eq!(code(send(&w, "b", None, &empty, &meta("commit-two-commit-two-2", json!({})))), (422, "invalid_payload".to_string()));
    let mut with_image = mc("A stem with a picture", None);
    with_image["image"] = json!({ "file": format!("media/{}.png", "e".repeat(64)), "width": 1, "height": 1 });
    let x = PoolCtx::new(&w.c, 5_000);
    let env = w.env(&x);
    let p = payload(vec![with_image]);
    let m = meta("commit-img-commit-img-1", json!({}));
    let read = ReadSend { raw: &p, meta: &m, payload: &p };
    let Pre::Go(pre) = precheck(&env, "b", &w.cid, None, &read).unwrap() else { panic!("replayed") };
    let e = match send_once(&env, "b", &w.cid, None, &pre, &read, true) {
        Err(Fail::Pool(p)) => p,
        other => panic!("{:?}", other.map(|_| ())),
    };
    assert_eq!((e.status, e.code.as_str(), e.extra["positions"].clone()), (422, "blob_missing", json!([0])));
    // A payload the JSON scan refused is shown the share-core reason.
    let junk = b"\xef\xbb\xbf{}".to_vec();
    let read = ReadSend { raw: &junk, meta: &m, payload: &junk };
    assert!(matches!(send_once(&env, "b", &w.cid, None, &pre, &read, false), Err(Fail::Pool(p)) if p.status == 422));
}

#[test]
fn a_bank_that_hides_its_keys_records_the_viewers_decisions_with_the_version() {
    let w = world();
    let (status, one) = send(&w, "b", None, &payload(vec![mc("Hidden key stem about moons", None)]), &meta("commit-one-commit-one-1", json!({ "hide_key": true }))).unwrap();
    assert_eq!((status, one["keys_hidden"].clone()), (201, json!(true)));
    assert_eq!(n(&w.c, "SELECT hide_key FROM circle_banks"), 1);
    let rec: Value = serde_json::from_str(&w.c.query_row::<String, _, _>("SELECT strip_opts FROM circle_versions", [], |r| r.get(0)).unwrap()).unwrap();
    assert!(rec["labels"].is_array() && rec["image_shas"].is_array());
    // The stored body is still the FULL text (the keys are only hidden at read).
    let stored = String::from_utf8(w.s.get(&c_full_key(&w.c, 1)).unwrap().unwrap()).unwrap();
    assert!(stored.contains("because"));
    // A later send to it is checked as hidden-key too: a refusal has the publish's shape and nothing is stored.
    let bid = one["bank_id"].as_str().unwrap().to_string();
    let mut bad = mc("Hidden key stem about moons", Some(one["qids"][0].as_str().unwrap()));
    bad["stem"] = json!("What is the capital of France? ANSWER: D");
    let r = send(&w, "b", Some(&bid), &payload(vec![bad]), &meta("commit-two-commit-two-2", json!({ "base_version": 1 })));
    assert_eq!(code(r).0, 422);
    assert_eq!(n(&w.c, "SELECT COUNT(*) FROM circle_versions"), 1);
}
