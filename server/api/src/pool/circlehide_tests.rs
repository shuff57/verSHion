//! Tests for `circlehide.rs`: decision reading and recording, the qkeys SQL, and stripping a stored body with the real share-core.
use super::*;
use crate::db;
use std::path::Path;

const KEY: &str = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";

fn m(v: Value) -> Map<String, Value> {
    v.as_object().unwrap().clone()
}
fn field(r: Result<StripDecisions, PoolError>) -> String {
    r.unwrap_err().extra["field"].as_str().unwrap().to_string()
}
fn mc() -> String {
    let words = ["apple", "river", "stone", "cloud", "ember", "maple", "orbit", "quartz"];
    let answers: Vec<Value> = words.iter().enumerate().map(|(i, t)| json!({"text": t, "correct": i == 3, "locked": false})).collect();
    json!({"schema_version": 1, "question": {"id": "q", "stem": "Pick", "kind": {"Mc": {"answers": answers}}, "points": null, "difficulty": null, "topics": [], "image": null, "explanation": "because", "figure": null, "created": 1, "modified": 1}}).to_string()
}

#[test]
fn decisions_are_read_as_a_publish_reads_them() {
    let d = read_strip_decisions(&m(json!({
        "labels": [{ "question": 0, "choice": "strip" }, { "question": 1, "part": 2, "choice": "keep" }],
        "order_acks": [{ "question": 3 }, { "question": 4, "part": 0 }],
        "image_ok": [1, 1, 2], "ack_formatting": [5, 5]
    })))
    .unwrap();
    assert_eq!((d.labels.len(), d.labels[1].part, d.order_acks[1].part, d.image_ok.clone(), d.ack_formatting.clone()), (2, Some(2), Some(0), vec![1, 2], vec![5]));
    assert_eq!(read_strip_decisions(&m(json!({}))).unwrap(), StripDecisions::default());
    assert_eq!(field(read_strip_decisions(&m(json!({ "labels": "x" })))), "labels");
    assert_eq!(field(read_strip_decisions(&m(json!({ "labels": [{ "question": 0, "choice": "maybe" }] })))), "labels");
    assert_eq!(field(read_strip_decisions(&m(json!({ "labels": [{ "question": -1, "choice": "keep" }] })))), "labels");
    assert_eq!(field(read_strip_decisions(&m(json!({ "labels": [{ "question": 0, "part": "a", "choice": "keep" }] })))), "labels");
    assert_eq!(field(read_strip_decisions(&m(json!({ "order_acks": [{ "question": 1_000_000 }] })))), "order_acks");
    assert_eq!(field(read_strip_decisions(&m(json!({ "image_ok": [1.5] })))), "image_ok");
    assert_eq!(field(read_strip_decisions(&m(json!({ "image_ok": vec![1; 1001] })))), "image_ok");
    assert_eq!(field(read_strip_decisions(&m(json!({ "ack_formatting": ["a"] })))), "ack_formatting");
}

#[test]
fn recorded_decisions_round_trip_and_unreadable_records_mean_none() {
    let d = StripDecisions { labels: vec![LabelDecision { question: 0, part: None, choice: "strip".into() }], order_acks: vec![Pos { question: 2, part: Some(1) }], image_ok: vec![0], ack_formatting: vec![1] };
    let j = decisions_json(&d, &["a".repeat(64)]);
    assert_eq!(decisions_of(Some(&j)), d);
    assert_eq!(serde_json::from_str::<Value>(&j).unwrap()["image_shas"][0], json!("a".repeat(64)));
    for bad in [None, Some(""), Some("not json"), Some("[1]"), Some(r#"{"labels":"x"}"#)] {
        assert_eq!(decisions_of(bad), StripDecisions::default(), "{bad:?}");
    }
    assert!(same_decisions(&d, &StripDecisions { labels: d.labels.clone(), order_acks: d.order_acks.clone(), image_ok: vec![0, 0], ack_formatting: vec![1, 1] }));
    assert!(!same_decisions(&d, &StripDecisions::default()));
}

#[test]
fn kept_images_and_hash_pairs() {
    let sha = "d".repeat(64);
    let p = json!({ "bank": { "questions": [
        { "image": { "file": format!("media/{sha}.png") } }, { "image": { "file": format!("media/{sha}.webp") } },
        { "image": { "file": "media/short.png" } }, { "image": null }, {} ] } });
    assert_eq!(kept_images(&p), vec![sha]);
    let s = |v: &[&str]| v.iter().map(|x| x.to_string()).collect::<Vec<_>>();
    assert_eq!(hash_pairs(&s(&["a", "b", "a", "c"]), &s(&["a", "B", "B", ""])), vec![("b".to_string(), "B".to_string()), ("a".to_string(), "B".to_string())]);
}

#[test]
fn the_stripped_qkeys_sql_records_stripped_hashes_under_the_first_sender() {
    let dir = tempfile::tempdir().unwrap();
    let mut c = db::open(&dir.path().join("t.db")).unwrap();
    db::migrate(&mut c, &Path::new(env!("CARGO_MANIFEST_DIR")).join("../worker/migrations")).unwrap();
    c.execute("INSERT INTO circle_qkeys (circle_id, match_hash, bank_id, first_version, first_seq, first_by) VALUES ('c', 'full1', 'b', 3, 7, 'u1')", []).unwrap();
    let sql = stripped_qkeys_sql("?1", "?2", "?3", "1");
    c.execute(&sql, params_from(["c", "b", r#"[["full1","strip1"],["nope","x"]]"#])).unwrap();
    let r: (String, i64, String) = c.query_row("SELECT match_hash, first_version, first_by FROM circle_qkeys WHERE match_hash = 'strip1'", [], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?))).unwrap();
    assert_eq!(r, ("strip1".into(), 3, "u1".into()));
    assert_eq!(c.query_row::<i64, _, _>("SELECT COUNT(*) FROM circle_qkeys", [], |r| r.get(0)).unwrap(), 2);
    c.execute(&stripped_qkeys_sql("?1", "?2", "?3", "0"), params_from(["c", "b", r#"[["full1","other"]]"#])).unwrap();
    assert_eq!(c.query_row::<i64, _, _>("SELECT COUNT(*) FROM circle_qkeys", [], |r| r.get(0)).unwrap(), 2, "a failing condition inserts nothing");
}
fn params_from<const N: usize>(v: [&str; N]) -> impl rusqlite::Params + '_ {
    rusqlite::params_from_iter(v.into_iter())
}

fn world() -> (rusqlite::Connection, tempfile::TempDir, BlobStore, Core) {
    let dir = tempfile::tempdir().unwrap();
    let mut c = db::open(&dir.path().join("t.db")).unwrap();
    db::migrate(&mut c, &Path::new(env!("CARGO_MANIFEST_DIR")).join("../worker/migrations")).unwrap();
    c.execute("INSERT INTO users (id, email, display_name, trust, created_at) VALUES ('v', 'v@x.org', 'V', 'teacher', 1)", []).unwrap();
    let s = BlobStore::new(dir.path().join("blobs")).unwrap();
    (c, dir, s, Core::new(Some(KEY.into())))
}

#[test]
fn a_stored_body_is_stripped_of_its_key_and_the_decisions_are_recorded() {
    let (_c, _d, s, core) = world();
    s.put("circles/full", mc().as_bytes()).unwrap();
    let StoredStrip::Ok { bytes, match_hashes, full_hashes, recorded, report, .. } = strip_stored(&core, &s, "circles/full", &StripDecisions::default(), true).unwrap() else { panic!("refused") };
    let text = String::from_utf8(bytes).unwrap();
    assert!(!text.contains(r#""correct":true"#) && !text.contains("because"));
    let full = full_hashes.unwrap();
    assert_eq!((match_hashes.len(), full.len()), (1, 1));
    assert_eq!(recorded, StripDecisions::default());
    assert!(report["labels_removed"].is_array());
    // A missing body is `missing`; garbage is a refusal that is not.
    assert!(matches!(strip_stored(&core, &s, "circles/none", &StripDecisions::default(), false).unwrap(), StoredStrip::Refused { missing: true, .. }));
    s.put("circles/bad", b"{}").unwrap();
    assert!(matches!(strip_stored(&core, &s, "circles/bad", &StripDecisions::default(), false).unwrap(), StoredStrip::Refused { missing: false, .. }));
}

#[test]
fn a_viewer_read_charges_the_daily_cap_and_gives_it_back_when_nothing_is_served() {
    let (c, _d, s, core) = world();
    s.put("circles/full", mc().as_bytes()).unwrap();
    s.put("circles/bad", b"{}").unwrap();
    let x = PoolCtx::new(&c, 5_000);
    let used = || c.query_row::<i64, _, _>("SELECT COALESCE(SUM(count), 0) FROM usage_daily WHERE metric = 'circle_hidden_reads'", [], |r| r.get(0)).unwrap();
    assert!(!String::from_utf8(stripped_for_viewer(&x, &core, &s, "v", "circles/full", None).unwrap()).unwrap().contains("because"));
    assert_eq!(used(), 1);
    let code = |r: Res<Vec<u8>>| match r.unwrap_err() {
        Fail::Pool(p) => (p.status, p.code),
        o => panic!("{o:?}"),
    };
    assert_eq!(code(stripped_for_viewer(&x, &core, &s, "v", "circles/bad", None)), (409, "keys_hidden_unavailable".to_string()));
    assert_eq!(code(stripped_for_viewer(&x, &core, &s, "v", "circles/none", None)), (404, "not_found".to_string()));
    assert_eq!(used(), 1, "refused reads are given back");
}

#[test]
fn the_full_form_is_handed_to_the_caller_or_nothing() {
    let (_c, _d, s, core) = world();
    s.put("circles/full", mc().as_bytes()).unwrap();
    s.put("circles/bad", b"{}").unwrap();
    let got = with_stored_full(&core, &s, "circles/full", |qs, mh| (qs.len(), mh.len())).unwrap();
    assert_eq!(got, Some((1, 1)));
    assert!(with_stored_full(&core, &s, "circles/bad", |_, _| ()).unwrap().is_none());
    assert!(with_stored_full(&core, &s, "circles/none", |_, _| ()).unwrap().is_none());
}
