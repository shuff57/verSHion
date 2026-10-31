//! Tests for `circlesendmeta.rs`: the send's meta rules, idempotency by commit_id, the index and its diff, the refusals, and the scan.
use super::*;
use crate::db;
use crate::pool::core::CoreError;
use std::path::Path;

const ROOT: &str = "7f757fb737085dcaaad0727deb89a14c18141681dc56be0a215a592c715f8e3b";
const CID: &str = "abcdefabcdefabcdefabcdefabcdefab";
const COMMIT: &str = "commit-id-commit-id-commit-id";
const SHA: &str = "cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc";

fn open() -> (rusqlite::Connection, tempfile::TempDir, BlobStore) {
    let dir = tempfile::tempdir().unwrap();
    let mut conn = db::open(&dir.path().join("t.db")).unwrap();
    db::migrate(&mut conn, &Path::new(env!("CARGO_MANIFEST_DIR")).join("../worker/migrations")).unwrap();
    let store = BlobStore::new(dir.path().join("blobs")).unwrap();
    (conn, dir, store)
}
fn m(v: Value) -> Map<String, Value> {
    v.as_object().unwrap().clone()
}
fn meta(extra: Value) -> Map<String, Value> {
    let mut base = json!({ "commit_id": COMMIT, "license": "circle-only", "attest": { "own_work": true, "terms_version": POOL_TERMS_VERSION } });
    for (k, v) in extra.as_object().unwrap() {
        base[k] = v.clone();
    }
    m(base)
}
fn field(e: PoolError) -> (u16, String, Value) {
    (e.status, e.code.clone(), e.body())
}

#[test]
fn a_first_version_and_a_later_one_read_their_meta_differently() {
    let first = read_send_meta(&meta(json!({ "message": "  hello  ", "hide_key": true })), true).unwrap();
    assert_eq!((first.base, first.message.as_deref(), first.hide_key, first.confirm), (None, Some("hello"), true, false));
    let later = read_send_meta(&meta(json!({ "base_version": 4, "confirm_large_change": true, "merge_token": "tok", "merged_from": 2 })), false).unwrap();
    assert_eq!((later.base, later.confirm, later.merge_token.as_deref(), later.merged_from), (Some(4), true, Some("tok".into()), Some(2)));
    assert_eq!(read_send_meta(&meta(json!({})), true).unwrap().meta, json!({}));
}

#[test]
fn bad_metas_name_their_field() {
    let f = |extra: Value, new: bool| field(read_send_meta(&meta(extra), new).unwrap_err()).2["field"].clone();
    assert_eq!(f(json!({ "commit_id": "short" }), true), json!("commit_id"));
    assert_eq!(f(json!({ "license": "MIT" }), true), json!("license"));
    assert_eq!(f(json!({ "base_version": 1 }), true), json!("base_version"));
    assert_eq!(f(json!({ "merge_token": "t" }), true), json!("merge_token"));
    assert_eq!(f(json!({}), false), json!("base_version"));
    assert_eq!(f(json!({ "base_version": 0 }), false), json!("base_version"));
    assert_eq!(f(json!({ "base_version": 1, "merge_token": "x".repeat(501) }), false), json!("merge_token"));
    assert_eq!(f(json!({ "base_version": 1, "merged_from": 2 }), false), json!("merged_from"));
    assert_eq!(f(json!({ "base_version": 1, "confirm_large_change": "yes" }), false), json!("confirm_large_change"));
    assert_eq!(f(json!({ "base_version": 1, "hide_key": true }), false), json!("hide_key"));
    assert_eq!(f(json!({ "message": 5 }), true), json!("message"));
    let (s, code, body) = field(read_send_meta(&m(json!({ "commit_id": COMMIT, "license": "circle-only" })), true).unwrap_err());
    assert_eq!((s, code.as_str(), body["terms_version"].clone()), (422, "attestation_required", json!(POOL_TERMS_VERSION)));
}

#[test]
fn keyless_findings_are_found_by_position_and_worded_by_kind() {
    let err = |code: &str, path: &str| CoreError { code: code.into(), path: path.into(), ..Default::default() };
    let f = CoreFail { errors: vec![err("keyless_needs_hide_key", "bank.questions[3].kind"), err("placeholder_key", "bank.questions[1]"), err("other", "bank.questions[9]"), err("keyless_needs_hide_key", "question.kind"), err("placeholder_key", "bank.questions[3]")], ..Default::default() };
    assert_eq!(keyless_positions(&f), vec![0, 1, 3]);
    let qs = vec![json!({ "kind": { "Short": {} } }), json!({ "kind": { "Single": {} } }), json!({}), json!({ "kind": { "Short": {} } })];
    let e = field(keyless_question(&[0, 3], &qs));
    assert_eq!((e.0, e.1.as_str(), e.2["kinds"].clone()), (422, "keyless_question", json!(["Short", "Short"])));
    assert!(e.2["message"].as_str().unwrap().contains("type a sample answer for each of them"));
    assert!(field(keyless_question(&[1], &qs)).2["message"].as_str().unwrap().contains("mark the correct answer"));
    assert!(field(keyless_question(&[0, 1], &qs)).2["message"].as_str().unwrap().contains("mark the correct one, or type"));
}

#[test]
fn large_changes_need_more_than_half_gone() {
    assert!(!is_large(5, 10) && is_large(6, 10) && !is_large(0, 0));
    let e = field(large_error(6, 10));
    assert_eq!((e.0, e.1.as_str(), e.2["removed"].clone(), e.2["total"].clone()), (428, "confirm_large_required", json!(6), json!(10)));
}

#[test]
fn index_reading_is_strict_and_the_diff_counts_by_qid() {
    let (_c, _d, s) = open();
    s.put("circles/i1", br#"{"qids":["a","b","c"],"h":["1","2","3"],"m":["x","y","z"]}"#).unwrap();
    s.put("circles/i2", br#"{"qids":["a"],"h":[]}"#).unwrap();
    s.put("circles/i3", br#"{"qids":["a"],"h":["1"],"m":["x","y"]}"#).unwrap();
    s.put("circles/i4", b"not json").unwrap();
    let i = read_index(&s, "circles/i1").unwrap();
    assert_eq!((i.qids.len(), i.m.as_ref().map(|m| m.len())), (3, Some(3)));
    assert!(read_index(&s, "circles/i2").is_none() && read_index(&s, "circles/i4").is_none() && read_index(&s, "").is_none() && read_index(&s, "circles/none").is_none());
    assert_eq!(read_index(&s, "circles/i3").unwrap().m, None, "a match list of the wrong length is ignored");
    let next_q: Vec<String> = ["a", "b", "d"].iter().map(|s| s.to_string()).collect();
    let next_h: Vec<String> = ["1", "9", "4"].iter().map(|s| s.to_string()).collect();
    assert_eq!(diff(Some(&i), &next_q, &next_h), (1, 1, 1));
    assert_eq!(diff(None, &next_q, &next_h), (3, 0, 0));
}

#[test]
fn a_commit_id_replays_conflicts_or_is_restarted() {
    let (c, _d, s) = open();
    c.execute("INSERT INTO users (id, email, display_name, trust, created_at) VALUES ('a', 'a@x.org', 'A', 'teacher', 1)", []).unwrap();
    let x = PoolCtx::new(&c, 1_000_000);
    assert!(replay_of(&x, &s, "a", CID, None, COMMIT, "sha1").unwrap().is_none());
    let put = |status: &str, result: Option<&str>, at: i64| {
        c.execute("DELETE FROM circle_versions", []).unwrap();
        c.execute(
            "INSERT INTO circle_versions (id, bank_id, circle_id, commit_id, request_sha256, result_json, committer_id, license, status, full_key, index_key, bytes, question_count, created_at) VALUES ('v1', 'bk', ?1, ?2, 'sha1', ?3, 'a', 'circle-only', ?4, 'circles/f', 'circles/i', 1, 1, ?5)",
            params![CID, COMMIT, result, status, at],
        )
        .unwrap();
    };
    put("current", Some(r#"{"ok":true,"version":2}"#), 5);
    assert_eq!(replay_of(&x, &s, "a", CID, Some("bk"), COMMIT, "sha1").unwrap().unwrap(), json!({ "ok": true, "version": 2, "replayed": true }));
    let conflict = |me: &str, cid: &str, bid: Option<&str>, sha: &str| match replay_of(&x, &s, me, cid, bid, COMMIT, sha).unwrap_err() {
        crate::pool::ctx::Fail::Pool(e) => e.code.clone(),
        o => panic!("{o:?}"),
    };
    assert_eq!(conflict("b", CID, None, "sha1"), "commit_id_conflict");
    assert_eq!(conflict("a", CID, None, "other"), "commit_id_conflict");
    assert_eq!(conflict("a", "elsewhere", None, "sha1"), "commit_id_conflict");
    assert_eq!(conflict("a", CID, Some("other-bank"), "sha1"), "commit_id_conflict");
    // Still being worked on: busy. Abandoned: discarded and run again, keys recorded for deletion then deleted.
    put("staging", None, 1_000_000 - 10);
    assert_eq!(conflict("a", CID, None, "sha1"), "publish_in_progress");
    put("staging", None, 1_000_000 - IN_FLIGHT_MS - 1);
    s.put("circles/f", b"x").unwrap();
    assert!(replay_of(&x, &s, "a", CID, None, COMMIT, "sha1").unwrap().is_none());
    let left: i64 = c.query_row("SELECT (SELECT COUNT(*) FROM circle_versions) + (SELECT COUNT(*) FROM r2_orphans)", [], |r| r.get(0)).unwrap();
    assert_eq!(left, 0);
    assert!(s.get("circles/f").unwrap().is_none());
}

fn question(id: &str, stem: &str, image: Option<&str>) -> Json {
    let mut v = json!({ "id": id, "stem": stem, "kind": { "Single": { "options": ["a", "b"], "correct": 0 } } });
    if let Some(i) = image {
        v["image"] = json!({ "file": format!("media/{i}.png") });
    }
    Json::parse(v.to_string().as_bytes()).unwrap()
}

#[test]
fn the_scan_finds_exact_student_data_blocks_and_images_and_copyright_blocks() {
    let (c, _d, _s) = open();
    let fp = Fingerprinter::from_root_hex(ROOT).unwrap();
    let qs = vec![question("q1", "What is two plus two here", None), question("q2", "Entirely different words about rivers", Some(SHA))];
    let form = |qs: Vec<Json>| ScanForm { img_at: images_at(&qs), mh: vec![], qs };
    assert_eq!(images_at(&qs), vec![None, Some(SHA.to_string())]);
    let x = PoolCtx::new(&c, 5_000);
    let clean = privacy_scan(&x, &fp, &[form(qs.clone())], "z").unwrap();
    assert!((clean.exact.is_empty() && clean.near.is_empty() && clean.copyright.is_empty()) && clean.keys.len() > 3 && clean.shas == vec![SHA.to_string()]);
    // A confirmed block on question 1's stem key and one on question 2's image.
    c.execute("INSERT INTO privacy_blocks (kind, hash, item_id, at) VALUES ('match', ?1, 'i', 1)", [stem_key(&fp, &qs[0])]).unwrap();
    c.execute("INSERT INTO privacy_blocks (kind, hash, item_id, at) VALUES ('blob', ?1, 'i', 1)", [SHA]).unwrap();
    let hit = privacy_scan(&x, &fp, &[form(qs.clone())], "z").unwrap();
    assert_eq!(hit.exact, vec![0, 1]);
    assert_eq!(hit.hits.len(), 2);
    // A copyright block on question 1's skeleton key counts for another sender, and an exemption lifts it.
    c.execute("INSERT INTO blocked_hashes (kind, hash, action_id, at) VALUES ('match', ?1, 'act1', 1)", [norm_skeleton_key(&fp, &qs[0])]).unwrap();
    let cr = privacy_scan(&x, &fp, &[form(qs.clone())], "z").unwrap();
    assert_eq!((cr.copyright.clone(), cr.actions.clone(), cr.chits.len()), (vec![0], vec!["act1".to_string()], 1));
    c.execute("INSERT INTO dmca_exempt (action_id, user_id, at) VALUES ('act1', 'z', 1)", []).ok();
    let exempt: i64 = c.query_row("SELECT COUNT(*) FROM dmca_exempt", [], |r| r.get(0)).unwrap();
    if exempt == 1 {
        assert!(privacy_scan(&x, &fp, &[form(qs.clone())], "z").unwrap().copyright.is_empty());
    }
    // An expired or lifted block holds nothing.
    c.execute("UPDATE blocked_hashes SET lifted_at = 2", []).unwrap();
    assert!(privacy_scan(&x, &fp, &[form(qs)], "q").unwrap().copyright.is_empty());
}

#[test]
fn a_second_form_adds_its_keys_to_each_position() {
    let (c, _d, _s) = open();
    let fp = Fingerprinter::from_root_hex(ROOT).unwrap();
    let full = vec![question("q1", "Full form of the stem about cells", None)];
    let stripped = vec![question("q1", "Stripped form of the stem about cells", None)];
    let x = PoolCtx::new(&c, 5_000);
    c.execute("INSERT INTO privacy_blocks (kind, hash, item_id, at) VALUES ('match', ?1, 'i', 1)", [stem_key(&fp, &stripped[0])]).unwrap();
    let forms = [ScanForm { img_at: images_at(&full), mh: vec![], qs: full.clone() }, ScanForm { img_at: images_at(&stripped), mh: vec![], qs: stripped }];
    assert_eq!(privacy_scan(&x, &fp, &forms[..1], "z").unwrap().exact, Vec::<usize>::new());
    assert_eq!(privacy_scan(&x, &fp, &forms, "z").unwrap().exact, vec![0], "a position is held if any form a member may be served matches");
}

#[test]
fn a_hidden_key_refusal_has_the_publishes_shape_and_code() {
    let err = |code: &str| CoreError { code: code.into(), path: "p".into(), ..Default::default() };
    let f = |codes: &[&str]| CoreFail {
        errors: codes.iter().map(|c| err(c)).collect(),
        label_findings: Some(vec![json!({ "question": 0 })]),
        needs_order_ack: Some(vec![crate::pool::core::Pos { question: 1, part: None }]),
        ..Default::default()
    };
    let e = strip_refusal(&f(&["key_in_stem"]));
    assert_eq!((e.status, e.code.as_str()), (422, "invalid_payload"));
    let b = e.body();
    assert_eq!((b["label_findings"][0]["question"].clone(), b["needs_order_ack"][0]["question"].clone()), (json!(0), json!(1)));
    assert!(b.get("order_preview").is_none() && b.get("formatting_findings").is_none());
    assert_eq!(strip_refusal(&f(&["labels_keep_refused"])).code, "labels_need_decision");
    assert_eq!(strip_refusal(&f(&["labels_keep_refused", "order_ack_required"])).code, "order_ack_required");
}
