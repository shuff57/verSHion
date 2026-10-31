//! Replays worker/test/pool/core.test.ts (and the adapter rules around it) against the native share-core.
use super::*;

const TF: &str = r#"{"schema_version":1,"question":{"id":"q","stem":"Is it?","kind":{"Tf":{"correct":true}},"points":2.0,"difficulty":null,"topics":[],"image":null,"explanation":null,"figure":null,"created":1,"modified":1}}"#;
const KEY: &str = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";

fn core() -> Core {
    Core::new(Some(KEY.into()))
}
fn mc() -> String {
    let words = ["apple", "river", "stone", "cloud", "ember", "maple", "orbit", "quartz"];
    let answers: Vec<Value> = words.iter().enumerate().map(|(i, t)| json!({"text": t, "correct": i == 3, "locked": false})).collect();
    json!({"schema_version": 1, "question": {"id": "q", "stem": "Pick", "kind": {"Mc": {"answers": answers}}, "points": null, "difficulty": null, "topics": [], "image": null, "explanation": "because", "figure": null, "created": 1, "modified": 1}}).to_string()
}

#[test]
fn raw_field_slices_a_top_level_value_byte_for_byte() {
    let j = r#"{"a":1,"payload":{"1":"x","b":[1,{"c":"}\"]"}],"d":1.0e21},"z":"payload"}"#;
    assert_eq!(raw_field(j, "payload"), Some(r#"{"1":"x","b":[1,{"c":"}\"]"}],"d":1.0e21}"#));
    assert_eq!(raw_field(j, "z"), Some(r#""payload""#));
    assert_eq!(raw_field(j, "a"), Some("1"));
    assert_eq!(raw_field(j, "missing"), None);
    assert_eq!(raw_field("[1]", "a"), None);
    // Re-serialising would have reordered "1" and changed the number: the stored bytes are share-core's.
    let v: Value = serde_json::from_str(raw_field(j, "payload").unwrap()).unwrap();
    assert_ne!(v.to_string(), raw_field(j, "payload").unwrap());
}

#[test]
fn the_adapter_returns_share_cores_canonical_bytes_and_refusals_carry_codes_and_paths_only() {
    let v = core().validate_for_publish(TF.as_bytes(), false, &[], &[]).unwrap();
    // share-core writes floats as floats (JS would print 2.0 as 2; serde_json keeps it, but the bytes are share-core's anyway).
    assert!(v.json().contains(r#""points":2.0"#), "{}", v.json());
    let bad = core().validate_for_publish(TF.replace("Is it?", "x\u{202e}y").as_bytes(), false, &[], &[]);
    match bad {
        Err(CoreErr::Fail(f)) => {
            let e = serde_json::to_value(&f.errors[0]).unwrap();
            for k in e.as_object().unwrap().keys() {
                assert!(["code", "path", "limit", "offset16", "cp", "message", "reason", "ack_allowed", "author_only", "hint"].contains(&k.as_str()), "{k}");
            }
            assert!(!serde_json::to_string(&f.errors).unwrap().contains("x\u{202e}y"));
        }
        other => panic!("{other:?}"),
    }
    assert_eq!(core().schema().unwrap(), (1, 2, 2));
}

#[test]
fn a_shared_error_gets_its_author_message_and_hostile_fields_are_dropped() {
    let raw = json!([{"code": "key_in_stem", "path": "q[0]", "cp": "U+202E", "reason": "ok_1", "secret": "zz", "hint": "h"},
                     {"code": "x", "cp": "not-a-cp", "reason": "BAD REASON", "hint": "h".repeat(501)}]);
    let e = clean_errors(Some(&raw));
    assert_eq!((e[0].message.is_some(), e[0].cp.as_deref(), e[0].reason.as_deref(), e[0].hint.as_deref()), (true, Some("U+202E"), Some("ok_1"), Some("h")));
    assert_eq!((e[1].message, e[1].cp.clone(), e[1].reason.clone(), e[1].hint.clone()), (None, None, None, None));
    assert_eq!(clean_errors(None)[0].code, "share_core_error");
    assert_eq!(clean_errors(Some(&json!(vec![json!({}); 150]))).len(), 100);
}

#[test]
fn strip_through_the_adapter_hashes_each_question_and_the_seed_is_share_cores() {
    let p = mc();
    let mut orders = std::collections::HashSet::new();
    for _ in 0..12 {
        let s = core().strip(p.as_bytes(), &PublishOpts::default()).unwrap();
        assert_eq!(s.match_hashes.len(), 1);
        let j = s.v.json();
        assert!(!j.contains(r#""correct":true"#) && !j.contains("because"));
        orders.insert(serde_json::to_string(&s.served_orders).unwrap());
    }
    assert!(orders.len() > 1, "the shuffle is random (the server never picks the seed)");
}

#[test]
fn publish_prepare_needs_the_key_and_returns_full_hashes_and_the_stripped_form_only_when_the_key_is_hidden() {
    let p = mc();
    assert!(matches!(Core::new(None).publish_prepare(p.as_bytes(), &PublishOpts::default()), Err(CoreErr::Config(_))));
    let shown = core().publish_prepare(p.as_bytes(), &PublishOpts { hide_key: false, ..Default::default() }).unwrap();
    assert!(shown.stripped.is_none());
    assert_eq!((shown.match_hashes.len(), shown.full_hashes.len()), (1, 1));
    let hidden = core().publish_prepare(p.as_bytes(), &PublishOpts { hide_key: true, ..Default::default() }).unwrap();
    let s = hidden.stripped.expect("a stripped form");
    assert!(!s.v.json().contains("because"));
    assert_eq!(hidden.match_hashes, shown.match_hashes, "the match hash is key-free");
    assert_eq!(hidden.full_hashes, shown.full_hashes);
    assert_eq!(s.served_orders.len(), 1, "the author-only served order was taken with the strip");
    // A different key gives different full hashes.
    let other = Core::new(Some("f".repeat(64))).publish_prepare(p.as_bytes(), &PublishOpts::default()).unwrap();
    assert_ne!(other.full_hashes, shown.full_hashes);
    assert_eq!(other.match_hashes, shown.match_hashes);
}

#[test]
fn match_and_full_hashes_and_the_no_key_refusal() {
    let p = mc();
    assert_eq!(core().match_hashes(p.as_bytes()).unwrap().len(), 1);
    assert_eq!(core().full_hashes(p.as_bytes()).unwrap().len(), 1);
    assert!(matches!(Core::new(None).full_hashes(p.as_bytes()), Err(CoreErr::Config(_))), "an unkeyed full hash is a guessable oracle");
    assert!(is_fail(&core().match_hashes(b"{")));
}

#[test]
fn metadata_is_validated_by_share_core() {
    let m = core().validate_meta(&json!({"title": "T", "subject": "Math", "tags": ["a"], "standards": []})).unwrap();
    assert_eq!((m.title.as_str(), m.subject.as_str(), m.course.as_str(), m.tags.clone()), ("T", "Math", "", vec!["a".to_string()]));
    assert!(is_fail(&core().validate_meta(&json!({"title": 5}))));
}

#[test]
fn content_hashes_ignore_ids_dates_and_provenance_and_depend_on_the_key() {
    let a = json!({"id": "a", "created": 1, "modified": 2, "origin": {"x": 1}, "published": {"y": 2}, "stem": "S", "kind": {"Tf": {"correct": true}}});
    let b = json!({"kind": {"Tf": {"correct": true}}, "stem": "S", "id": "b", "created": 9, "modified": 9});
    let c = json!({"stem": "S2", "kind": {"Tf": {"correct": true}}});
    let h = core().content_hashes(&[a, b, c]).unwrap();
    assert_eq!(h[0], h[1]);
    assert_ne!(h[0], h[2]);
    assert_eq!(h[0].len(), 64);
    assert_ne!(Core::new(Some("a".repeat(64))).content_hashes(&[json!({"stem": "S"})]).unwrap()[0], core().content_hashes(&[json!({"stem": "S"})]).unwrap()[0]);
    assert!(matches!(Core::new(None).content_hashes(&[]), Err(CoreErr::Config(_))));
}

#[test]
fn canonical_sorts_keys_like_the_worker_and_the_question_helpers_read_the_payload() {
    assert_eq!(canonical(&json!({"b": 1, "a": [true, null, "x\"y"], "é": 2.5, "10": 1, "9": 2})), r#"{"10":1,"9":2,"a":[true,null,"x\"y"],"b":1,"é":2.5}"#);
    let bank = json!({"schema_version": 2, "bank": {"questions": [{"kind": {"Mc": {}}}, {"kind": {"Tf": {}}}]}});
    assert_eq!(questions_of(&bank).len(), 2);
    assert_eq!(kind_tag(questions_of(&bank)[1]), "Tf");
    assert_eq!(questions_of(&serde_json::from_str(TF).unwrap()).len(), 1);
    assert!(questions_of(&json!({})).is_empty());
}

#[test]
fn a_panic_inside_share_core_is_a_counted_trap_not_a_crash() {
    take_wasm_traps();
    let r: CoreRes<()> = guard(|| panic!("boom"));
    assert!(matches!(r, Err(CoreErr::Trap)));
    assert!(take_wasm_traps() >= 1);
}
