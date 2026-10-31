//! Tests for `circleversions.rs`: history paging, per-question change lists, merge-initiate and trim on versions made by real sends.
use super::*;
use crate::db;
use crate::pool::circlecommit::commit_version;
use crate::pool::circles::create_circle;
use crate::pool::circlesendprep::{Pre, ReadSend, SendEnv, precheck, send_once};
use crate::pool::core::Core;
use crate::pool::fingerprint::Fingerprinter;
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
fn mc(stem: &str, qid: Option<&str>) -> Value {
    let words = ["apple", "river", "stone", "cloud", "ember", "maple", "orbit", "quartz"];
    let answers: Vec<Value> = words.iter().enumerate().map(|(i, t)| json!({ "text": t, "correct": i == 3, "locked": false })).collect();
    let mut q = json!({ "id": "q", "stem": stem, "kind": { "Mc": { "answers": answers } }, "points": null, "difficulty": null, "topics": [], "image": null, "explanation": "because", "figure": null, "created": 1, "modified": 1 });
    if let Some(id) = qid {
        q["published"] = json!({ "qid": id });
    }
    q
}
fn send(w: &W, me: &str, bid: Option<&str>, qs: Vec<Value>, commit: &str, base: Option<i64>, message: &str) -> Value {
    let x = PoolCtx::new(&w.c, 5_000);
    let e = SendEnv { x: &x, core: &w.core, fp: &w.fp, keys: &w.keys, store: &w.s };
    let mut m = json!({ "commit_id": commit, "license": "circle-only", "message": message, "meta": { "title": "Quiz" }, "attest": { "own_work": true, "terms_version": crate::pool::license::POOL_TERMS_VERSION } });
    if let Some(b) = base {
        m["base_version"] = json!(b);
    }
    let m = m.as_object().unwrap().clone();
    let p = json!({ "schema_version": 1, "bank": { "id": "l", "name": "My bank", "questions": qs, "created": 1, "modified": 1 } }).to_string().into_bytes();
    let read = ReadSend { raw: &p, meta: &m, payload: &p };
    let Pre::Go(pre) = precheck(&e, me, &w.cid, bid, &read).unwrap() else { panic!("replayed") };
    let spec = send_once(&e, me, &w.cid, bid, &pre, &read, true).unwrap();
    commit_version(&x, &w.s, &spec).unwrap().1
}
fn qids(v: &Value) -> Vec<String> {
    v["qids"].as_array().unwrap().iter().map(|q| q.as_str().unwrap().to_string()).collect()
}
/// n versions: v1 has A and B; v2 rewords B; v3 adds C; later ones reword A.
fn bank_with(w: &W, versions: usize) -> (String, Vec<String>) {
    let one = send(w, "b", None, vec![mc("Stem A about rivers", None), mc("Stem B about stones", None)], "commit-0001-commit-0001-1", None, "first");
    let bid = one["bank_id"].as_str().unwrap().to_string();
    let q = qids(&one);
    let mut all = q.clone();
    for v in 2..=versions {
        let mut qs = vec![mc(&format!("Stem A about rivers, edit {v}"), Some(&q[0])), mc("Stem B about stones", Some(&q[1]))];
        if v == 2 {
            qs[0] = mc("Stem A about rivers", Some(&q[0]));
            qs[1] = mc("Stem B about stones reworded", Some(&q[1]));
        }
        if v >= 3 {
            qs.push(mc("Stem C about clouds", all.get(2).map(String::as_str)));
        }
        let r = send(w, "b", Some(&bid), qs, &format!("commit-{v:04}-commit-{v:04}-1"), Some(v as i64 - 1), &format!("m{v}"));
        if v == 3 {
            all = qids(&r);
        }
    }
    (bid, all)
}
fn ctx(w: &W) -> PoolCtx<'_> {
    PoolCtx::new(&w.c, 9_000)
}
fn code<T: std::fmt::Debug>(r: Res<T>) -> (u16, String) {
    match r.unwrap_err() {
        Fail::Pool(p) => (p.status, p.code),
        o => panic!("{o:?}"),
    }
}

#[test]
fn the_history_lists_newest_first_and_pages_by_version() {
    let w = world();
    let (bid, _) = bank_with(&w, 3);
    let x = ctx(&w);
    let h = history(&x, "a", &w.cid, &bid, None).unwrap();
    let vs = h["versions"].as_array().unwrap();
    assert_eq!((vs.len(), vs[0]["version"].clone(), vs[0]["by"].clone(), vs[0]["message"].clone(), vs[0]["kept"].clone()), (3, json!(3), json!("Teacher b"), json!("m3"), json!(true)));
    assert_eq!((h["head_version"].clone(), h["oldest_kept"].clone(), h["next_before"].clone()), (json!(3), json!(1), Value::Null));
    assert_eq!((vs[2]["added"].clone(), vs[1]["changed"].clone()), (json!(2), json!(1)));
    let older = history(&x, "a", &w.cid, &bid, Some("3")).unwrap();
    assert_eq!(older["versions"].as_array().unwrap().iter().map(|v| v["version"].as_i64().unwrap()).collect::<Vec<_>>(), vec![2, 1]);
    for bad in ["0", "abc", "1234567890", "-1", ""] {
        assert_eq!(code(history(&x, "a", &w.cid, &bid, Some(bad))), (400, "invalid_request".to_string()), "{bad}");
    }
    // A withheld version shows no message and is marked; a non-member sees nothing.
    w.c.execute("UPDATE circle_versions SET withheld_at = 5 WHERE version = 2", []).unwrap();
    let h = history(&x, "a", &w.cid, &bid, None).unwrap();
    let v2 = &h["versions"].as_array().unwrap()[1];
    assert_eq!((v2["withheld"].clone(), v2["kept"].clone(), v2["message"].clone(), v2["review_copy"].clone()), (json!(true), json!(false), json!(""), json!("full")));
    assert_eq!(code(history(&x, "nobody", &w.cid, &bid, None)).0, 404);
    // A viewer of a bank that hides its keys reads no messages.
    w.c.execute("UPDATE circle_banks SET hide_key = 1", []).unwrap();
    assert_eq!(history(&x, "v", &w.cid, &bid, None).unwrap()["versions"][0]["message"], json!(""));
    assert_eq!(history(&x, "b", &w.cid, &bid, None).unwrap()["versions"][0]["message"], json!("m3"));
}

#[test]
fn changes_compare_each_readable_version_with_the_one_below_and_honour_the_etag() {
    let w = world();
    let (bid, q) = bank_with(&w, 3);
    let x = ctx(&w);
    let Changes::Body(body, etag) = changes(&x, &w.s, "a", &w.cid, &bid, None).unwrap() else { panic!("304") };
    let vs = body["versions"].as_array().unwrap();
    assert_eq!(vs.len(), 2, "the oldest has no base inside the window");
    assert_eq!((vs[0]["version"].clone(), vs[0]["compared_to"].clone(), vs[0]["added"].clone(), vs[0]["changed"].clone(), vs[0]["removed"].clone()), (json!(3), json!(2), json!([q[2]]), json!([q[0], q[1]]), json!([])));
    assert_eq!((vs[1]["version"].clone(), vs[1]["changed"].clone()), (json!(2), json!([q[1]])));
    assert!(etag.starts_with('"') && etag.len() == 34);
    assert!(matches!(changes(&x, &w.s, "a", &w.cid, &bid, Some(&format!("W/{etag}"))).unwrap(), Changes::NotModified(e) if e == etag));
    // A gap when an index is gone; a withheld version is never a comparison base.
    let ik: String = w.c.query_row("SELECT index_key FROM circle_versions WHERE version = 2", [], |r| r.get(0)).unwrap();
    w.s.delete(&ik).unwrap();
    let Changes::Body(body, _) = changes(&x, &w.s, "a", &w.cid, &bid, None).unwrap() else { panic!() };
    assert_eq!(body["versions"][0]["gap"], json!(true));
    w.c.execute("UPDATE circle_versions SET withheld_at = 5 WHERE version = 2", []).unwrap();
    let Changes::Body(body, _) = changes(&x, &w.s, "a", &w.cid, &bid, None).unwrap() else { panic!() };
    assert_eq!((body["versions"].as_array().unwrap().len(), body["versions"][0]["compared_to"].clone()), (1, json!(1)));
    // The reads are charged; a viewer of a hidden-key bank gets no lists.
    assert!(n(&w.c, "SELECT COALESCE(SUM(count),0) FROM usage_daily WHERE metric = 'circle_change_reads'") >= 2);
    w.c.execute("UPDATE circle_banks SET hide_key = 1", []).unwrap();
    let Changes::Body(body, _) = changes(&x, &w.s, "v", &w.cid, &bid, None).unwrap() else { panic!() };
    assert_eq!((body["per_question"].clone(), body["reason"].clone()), (json!(false), json!("keys_hidden")));
}

#[test]
fn a_merge_starts_with_a_token_for_this_account_bank_and_head() {
    let w = world();
    let (bid, _) = bank_with(&w, 3);
    let x = ctx(&w);
    let body = |b: Value| json!({ "base_version": b }).as_object().unwrap().clone();
    let r = merge_initiate(&x, &w.keys, "a", &w.cid, &bid, &body(json!(1))).unwrap();
    assert_eq!((r["base_kept"].clone(), r["head_version"].clone(), r["between_total"].clone(), r["head_by"].clone()), (json!(true), json!(3), json!(2), json!("Teacher b")));
    assert_eq!(r["between"].as_array().unwrap().iter().map(|v| v["version"].as_i64().unwrap()).collect::<Vec<_>>(), vec![2, 3]);
    assert_eq!(w.keys.verify_merge_token(9_000, r["merge_token"].as_str().unwrap(), "a", &bid, 3), Some(1));
    assert_eq!(r["merge_token_expires_at"], json!(9_000 + crate::pool::poolkeys::MERGE_TOKEN_MS));
    for bad in [json!(3), json!(0), json!("1"), json!(1.5), json!(null)] {
        assert_eq!(code(merge_initiate(&x, &w.keys, "a", &w.cid, &bid, &body(bad))), (400, "invalid_request".to_string()));
    }
    assert_eq!(code(merge_initiate(&x, &w.keys, "v", &w.cid, &bid, &body(json!(1)))), (403, "circle_role_required".to_string()));
    w.c.execute("UPDATE circle_versions SET status = 'pruned', full_key = '' WHERE version = 1", []).unwrap();
    assert_eq!(merge_initiate(&x, &w.keys, "a", &w.cid, &bid, &body(json!(1))).unwrap()["base_kept"], json!(false));
}

#[test]
fn trim_prunes_older_versions_for_the_owner_or_author_and_says_what_stays() {
    let w = world();
    let (bid, _) = bank_with(&w, 5);
    let x = ctx(&w);
    let keep = |k: Value| json!({ "keep": k }).as_object().unwrap().clone();
    assert_eq!(code(trim_versions(&x, "c", &w.cid, &bid, &keep(json!(2)))), (403, "circle_role_required".to_string()));
    for bad in [json!(0), json!(20), json!("2"), json!(1.5)] {
        assert_eq!(code(trim_versions(&x, "a", &w.cid, &bid, &keep(bad))), (400, "invalid_request".to_string()));
    }
    // An open report names version 2: it stays.
    w.c.execute("INSERT INTO circle_reports (id, circle_id, bank_id, version, reporter_id, reason, created_at) VALUES ('r1', ?1, ?2, 2, 'v', 'other', 1)", params![w.cid, bid]).ok();
    let reported = n(&w.c, "SELECT COUNT(*) FROM circle_reports");
    let r = trim_versions(&x, "b", &w.cid, &bid, &keep(json!(2))).unwrap();
    let st: Vec<(i64, String)> = w.c.prepare("SELECT version, status FROM circle_versions ORDER BY version").unwrap().query_map([], |r| Ok((r.get(0)?, r.get(1)?))).unwrap().collect::<Result<_, _>>().unwrap();
    let want = if reported == 1 { vec![(1, "pruned"), (2, "previous"), (3, "pruned"), (4, "previous"), (5, "current")] } else { vec![(1, "pruned"), (2, "pruned"), (3, "pruned"), (4, "previous"), (5, "current")] };
    assert_eq!(st, want.iter().map(|(v, s)| (*v, s.to_string())).collect::<Vec<_>>());
    assert_eq!((r["ok"].clone(), r["pruned"].clone()), (json!(true), json!(if reported == 1 { 2 } else { 3 })));
    assert_eq!(n(&w.c, "SELECT COUNT(*) FROM pool_audit WHERE action = 'circle.versions_trimmed'"), 1);
    // Trimming again prunes nothing and writes no audit row.
    assert_eq!(trim_versions(&x, "a", &w.cid, &bid, &keep(json!(2))).unwrap()["pruned"], json!(0));
    assert_eq!(n(&w.c, "SELECT COUNT(*) FROM pool_audit WHERE action = 'circle.versions_trimmed'"), 1);
    // A blocking hold stops a trim.
    w.c.execute("INSERT INTO circle_holds (bank_id, reason, source, at, by) VALUES (?1, 'copyright', 'report', 1, NULL)", [&bid]).unwrap();
    assert_eq!(code(trim_versions(&x, "b", &w.cid, &bid, &keep(json!(2)))), (409, "under_moderation".to_string()));
}
