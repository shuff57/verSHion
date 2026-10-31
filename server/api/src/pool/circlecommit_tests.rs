//! Tests for `circlecommit.rs` on the real migrated schema: the compare-and-set commit of a new bank and a later version, the holds a scan
//! sets, the cleanup of a lost commit and the reason it gives.
use super::*;
use crate::db;
use crate::pool::circles::create_circle;
use rusqlite::Connection;
use serde_json::Map;
use std::path::Path;

fn open() -> (Connection, tempfile::TempDir, BlobStore) {
    let dir = tempfile::tempdir().unwrap();
    let mut conn = db::open(&dir.path().join("t.db")).unwrap();
    db::migrate(&mut conn, &Path::new(env!("CARGO_MANIFEST_DIR")).join("../worker/migrations")).unwrap();
    let store = BlobStore::new(dir.path().join("blobs")).unwrap();
    (conn, dir, store)
}
fn user(c: &Connection, u: &str) {
    c.execute("INSERT INTO users (id, email, display_name, trust, created_at) VALUES (?1, ?2, ?3, 'teacher', 1)", params![u, format!("{u}@x.org"), format!("Teacher {u}")]).unwrap();
}
fn n(c: &Connection, sql: &str) -> i64 {
    c.query_row(sql, [], |r| r.get(0)).unwrap()
}
fn world() -> (Connection, tempfile::TempDir, BlobStore, String) {
    let (c, d, s) = open();
    for u in ["a", "b", "z"] {
        user(&c, u);
    }
    let body: Map<String, Value> = json!({ "name": "Team" }).as_object().unwrap().clone();
    let cid = create_circle(&PoolCtx::new(&c, 1_000), "a", &body).unwrap().1["circle"]["id"].as_str().unwrap().to_string();
    c.execute("INSERT INTO circle_members (circle_id, user_id, role, joined_at, join_seq) VALUES (?1, 'b', 'editor', 10, 5)", [&cid]).unwrap();
    (c, d, s, cid)
}
fn spec(cid: &str, bid: &str, me: &str, version: i64, commit: &str) -> CommitSpec {
    let vid = new_id();
    CommitSpec {
        kind: Kind::Send,
        me: me.into(),
        cid: cid.into(),
        bid: bid.into(),
        vid: vid.clone(),
        is_new: version == 1,
        base: if version == 1 { None } else { Some(version - 1) },
        version,
        commit_id: commit.into(),
        request_sha: format!("sha-{commit}"),
        message: None,
        license: "circle-only".into(),
        meta: Some(Listing { title: "Quiz".into(), subject: "Math".into(), course: "".into(), grade: "".into() }),
        listing: Some(json!({ "title": "Quiz", "subject": "Math", "course": "", "grade": "" }).to_string()),
        n: 3,
        has_images: 0,
        bytes: 100,
        new_keys: 2,
        hashes: vec!["h1".into(), "h2".into()],
        shas: vec![],
        origins: vec![],
        merged_from: None,
        rollback_of: None,
        privacy: Scan::default(),
        strip_opts: None,
        pairs: vec![],
        hide_key: Some(0),
        result: json!({ "circle_id": cid, "bank_id": bid, "version": version, "version_id": vid, "qids": [], "added": 3, "changed": 0, "removed": 0 }),
        audit_detail: json!({ "added": 3, "license": "circle-only" }),
        full_bytes: b"{\"full\":1}".to_vec(),
        index_bytes: b"{\"qids\":[]}".to_vec(),
    }
}
fn code(r: Res<(u16, Value)>) -> (u16, String) {
    match r.unwrap_err() {
        Fail::Pool(p) => (p.status, p.code),
        o => panic!("{o:?}"),
    }
}

#[test]
fn a_new_bank_then_a_second_version_commit_and_move_the_head() {
    let (c, _d, s, cid) = world();
    let bid = new_id();
    let x = PoolCtx::new(&c, 5_000);
    let one = spec(&cid, &bid, "b", 1, "commit-one-commit-one-1");
    let (status, body) = commit_version(&x, &s, &one).unwrap();
    assert_eq!((status, body["version"].clone(), body["held"].clone(), body["withheld"].clone()), (201, json!(1), json!(false), json!(false)));
    assert_eq!((n(&c, "SELECT head_version FROM circle_banks"), n(&c, "SELECT COUNT(*) FROM circle_versions WHERE status = 'current' AND version = 1 AND seq IS NOT NULL")), (1, 1));
    assert_eq!(c.query_row::<String, _, _>("SELECT title || subject FROM circle_banks", [], |r| r.get(0)).unwrap(), "QuizMath");
    assert_eq!((n(&c, "SELECT qkeys FROM circles"), n(&c, "SELECT COUNT(*) FROM circle_qkeys")), (2, 2));
    assert_eq!((n(&c, "SELECT COUNT(*) FROM attestations"), n(&c, "SELECT COUNT(*) FROM pool_audit WHERE action = 'circle.bank_add'")), (1, 1));
    let (fk, ik) = full_key_of(&cid, &bid, &one.vid);
    assert_eq!((s.get(&fk).unwrap().unwrap(), s.get(&ik).unwrap().unwrap()), (one.full_bytes.clone(), one.index_bytes.clone()));
    // The next version by the owner, based on 1.
    let two = spec(&cid, &bid, "a", 2, "commit-two-commit-two-2");
    let (status, body) = commit_version(&x, &s, &two).unwrap();
    assert_eq!((status, body["version"].clone()), (200, json!(2)));
    let st: Vec<(i64, String)> = c.prepare("SELECT version, status FROM circle_versions ORDER BY version").unwrap().query_map([], |r| Ok((r.get(0)?, r.get(1)?))).unwrap().collect::<Result<_, _>>().unwrap();
    assert_eq!(st, vec![(1, "previous".into()), (2, "current".into())]);
    assert_eq!(n(&c, "SELECT head_version FROM circle_banks"), 2);
    assert_eq!(n(&c, "SELECT COUNT(*) FROM pool_audit WHERE action = 'circle.version'"), 1);
    assert_eq!(n(&c, "SELECT COUNT(*) FROM attestations"), 2);
}

#[test]
fn a_commit_on_a_moved_head_loses_cleans_up_and_names_the_conflict() {
    let (c, _d, s, cid) = world();
    let bid = new_id();
    let x = PoolCtx::new(&c, 5_000);
    commit_version(&x, &s, &spec(&cid, &bid, "b", 1, "commit-one-commit-one-1")).unwrap();
    commit_version(&x, &s, &spec(&cid, &bid, "a", 2, "commit-two-commit-two-2")).unwrap();
    // Another sender still based on version 1.
    let stale = spec(&cid, &bid, "b", 2, "commit-late-commit-late");
    let (fk, _) = full_key_of(&cid, &bid, &stale.vid);
    let e = match commit_version(&x, &s, &stale).unwrap_err() {
        Fail::Pool(p) => p,
        o => panic!("{o:?}"),
    };
    assert_eq!((e.status, e.code.as_str(), e.extra["head_version"].clone(), e.extra["head_by"].clone()), (409, "conflict", json!(2), json!("Teacher a")));
    assert_eq!(n(&c, "SELECT COUNT(*) FROM circle_versions WHERE commit_id = 'commit-late-commit-late'"), 0);
    assert!(s.get(&fk).unwrap().is_none(), "the lost version's body is deleted");
    assert_eq!(n(&c, "SELECT COUNT(*) FROM r2_orphans"), 0);
    assert_eq!(n(&c, "SELECT head_version FROM circle_banks"), 2);
}

#[test]
fn an_editor_changing_the_licence_or_an_outsider_is_refused_with_its_reason() {
    let (c, _d, s, cid) = world();
    let bid = new_id();
    let x = PoolCtx::new(&c, 5_000);
    commit_version(&x, &s, &spec(&cid, &bid, "b", 1, "commit-one-commit-one-1")).unwrap();
    let mut other_license = spec(&cid, &bid, "a", 2, "commit-lic-commit-lic-1");
    other_license.license = "cc-by".into();
    assert_eq!(code(commit_version(&x, &s, &other_license)), (422, "license_author_only".to_string()));
    // z is no member of the circle.
    assert_eq!(code(commit_version(&x, &s, &spec(&cid, &bid, "z", 2, "commit-out-commit-out-1"))), (404, "not_found".to_string()));
    // The head moved under a hidden-keys flip: the setting changed, not the head.
    c.execute("UPDATE circle_banks SET hide_key = 1", []).unwrap();
    assert_eq!(code(commit_version(&x, &s, &spec(&cid, &bid, "b", 2, "commit-hid-commit-hid-1"))), (409, "hide_key_changed".to_string()));
    assert_eq!(n(&c, "SELECT COUNT(*) FROM circle_versions"), 1);
}

#[test]
fn the_circle_caps_refuse_with_their_numbers() {
    let (c, _d, s, cid) = world();
    let bid = new_id();
    let x = PoolCtx::new(&c, 5_000);
    let caps = caps_of(&x).unwrap();
    let mut big = spec(&cid, &bid, "b", 1, "commit-big-commit-big-1");
    big.bytes = caps.bytes + 1;
    let e = match commit_version(&x, &s, &big).unwrap_err() {
        Fail::Pool(p) => p,
        o => panic!("{o:?}"),
    };
    assert_eq!((e.status, e.code.as_str(), e.extra["needed"].clone(), e.extra["limit"].clone()), (413, "quota_exceeded", json!(caps.bytes + 1), json!(caps.bytes)));
    let mut many = spec(&cid, &bid, "b", 1, "commit-many-commit-many");
    many.n = caps.questions + 1;
    assert_eq!(code(commit_version(&x, &s, &many)), (422, "circle_questions_limit".to_string()));
    let mut keys = spec(&cid, &bid, "b", 1, "commit-keys-commit-keys");
    keys.new_keys = caps.records + 1;
    assert_eq!(code(commit_version(&x, &s, &keys)), (413, "circle_records_full".to_string()));
    assert_eq!(n(&c, "SELECT COUNT(*) FROM circle_versions") + n(&c, "SELECT COUNT(*) FROM circle_banks"), 0);
}

#[test]
fn a_version_matching_confirmed_student_data_is_withheld_flagged_and_holds_the_bank() {
    let (c, _d, s, cid) = world();
    let bid = new_id();
    let x = PoolCtx::new(&c, 5_000);
    c.execute("INSERT INTO privacy_blocks (kind, hash, item_id, at) VALUES ('match', 'bad-hash', 'i', 1)", []).unwrap();
    let mut one = spec(&cid, &bid, "b", 1, "commit-one-commit-one-1");
    one.privacy = Scan { keys: vec!["bad-hash".into(), "other".into()], exact: vec![1], hits: vec!["bad-hash".into()], ..Default::default() };
    let (status, body) = commit_version(&x, &s, &one).unwrap();
    assert_eq!((status, body["held"].clone(), body["withheld"].clone()), (201, json!(true), json!(true)));
    assert_eq!(n(&c, "SELECT COUNT(*) FROM circle_holds WHERE source = 'privacy' AND reason = 'student_data'"), 1);
    assert_eq!(n(&c, "SELECT COUNT(*) FROM circle_versions WHERE withheld_at IS NOT NULL"), 1);
    assert_eq!(n(&c, "SELECT COUNT(*) FROM admin_flags WHERE kind = 'privacy_text_match' AND item_id = (SELECT id FROM circle_banks)"), 1);
    let d: Value = serde_json::from_str(&c.query_row::<String, _, _>("SELECT detail FROM admin_flags WHERE kind = 'privacy_text_match'", [], |r| r.get(0)).unwrap()).unwrap();
    assert_eq!((d["positions"]["count"].clone(), d["positions"]["first"][0].clone(), d["hashes"][0].clone()), (json!(1), json!(1), json!("bad-hash")));
    let told = x.take_notices();
    assert!(told.iter().any(|t| matches!(t, PoolNotice::PrivacyTextMatch { held: true, circle_id: Some(_), version: Some(1), .. })), "the admin is told: {told:?}");
    // A clean next version by the author lifts the hold (and is not itself withheld).
    let mut two = spec(&cid, &bid, "b", 2, "commit-two-commit-two-2");
    two.privacy = Scan { keys: vec!["fine".into()], ..Default::default() };
    let (_, body) = commit_version(&x, &s, &two).unwrap();
    assert_eq!((body["held"].clone(), body["withheld"].clone()), (json!(false), json!(false)));
    assert_eq!(n(&c, "SELECT COUNT(*) FROM circle_holds"), 0);
    assert_eq!(n(&c, "SELECT COUNT(*) FROM circle_versions WHERE withheld_at IS NOT NULL"), 1, "the earlier version stays withheld");
}

#[test]
fn held_publishes_are_counted_and_past_the_budget_the_admin_is_flagged() {
    let (c, _d, _s, _cid) = world();
    let x = PoolCtx::new(&c, 5_000);
    charge_held_publishes(&x, "b", HELD_PUBLISHES_PER_DAY - 1).unwrap();
    assert_eq!(n(&c, "SELECT COUNT(*) FROM admin_flags WHERE kind = 'cap'"), 0);
    charge_held_publishes(&x, "b", 1).unwrap();
    assert_eq!(n(&c, "SELECT count FROM usage_daily WHERE metric = 'held_publishes'"), HELD_PUBLISHES_PER_DAY);
    assert_eq!(n(&c, "SELECT COUNT(*) FROM admin_flags WHERE kind = 'cap' AND user_id = 'b'"), 1);
    assert!(matches!(x.take_notices().as_slice(), [PoolNotice::Cap { metric, .. }] if metric == "held_publishes"));
}

#[test]
fn a_fresh_bank_is_read_or_missed() {
    let (c, _d, s, cid) = world();
    let bid = new_id();
    let x = PoolCtx::new(&c, 5_000);
    assert!(matches!(fresh_bank(&x, &cid, &bid), Err(Fail::Pool(p)) if p.status == 404));
    commit_version(&x, &s, &spec(&cid, &bid, "b", 1, "commit-one-commit-one-1")).unwrap();
    let b = fresh_bank(&x, &cid, &bid).unwrap();
    assert_eq!((b.head_version, b.author_id.as_str(), b.hide_key), (1, "b", 0));
}
