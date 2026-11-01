//! Tests for `publishmeta.rs`: each field refused by name, nothing preselected, lists bounded and deduplicated.
use super::*;
use serde_json::json;

const PID: &str = "AAAAAAAAAAAAAAAAAAAAAA";
const PID2: &str = "BBBBBBBBBBBBBBBBBBBBBB";

fn good() -> Map<String, Value> {
    json!({ "publish_id": PID, "hide_key": false, "license": "CC-BY-4.0", "visibility": "public", "attestation": { "accepted": true, "text_rev": "2026-10-19" } })
        .as_object()
        .unwrap()
        .clone()
}
fn with(k: &str, v: Value) -> Map<String, Value> {
    let mut m = good();
    m.insert(k.into(), v);
    m
}
fn field(r: Result<PublishMeta, PoolError>) -> String {
    let e = r.err().expect("refused");
    assert_eq!((e.status, e.code.as_str()), (400, "invalid_request"));
    e.extra["field"].as_str().unwrap().to_string()
}

#[test]
fn a_minimal_new_publish_reads_with_nothing_preselected() {
    let m = read_meta(&good(), true).unwrap();
    assert_eq!((m.publish_id.as_str(), m.base, m.hide_key, m.license.as_str(), m.visibility.as_deref()), (PID, None, false, "CC-BY-4.0", Some("public")));
    assert_eq!(m.attest_rev, "2026-10-19");
    assert_eq!((m.labels.len(), m.order_acks.len(), m.image_ok.len(), m.independent.len(), m.attest.is_none(), m.statement.is_none()), (0, 0, 0, 0, true, true));
    assert_eq!(m.meta, json!({}));
    for k in ["publish_id", "hide_key", "license", "attestation", "visibility"] {
        let mut g = good();
        g.remove(k);
        assert_eq!(field(read_meta(&g, true)), k, "{k} is required");
    }
}

#[test]
fn ids_licences_and_the_attestation_are_checked() {
    assert_eq!(field(read_meta(&with("publish_id", json!("short")), true)), "publish_id");
    assert_eq!(field(read_meta(&with("license", json!("MIT")), true)), "license");
    assert_eq!(field(read_meta(&with("hide_key", json!("yes")), true)), "hide_key");
    assert_eq!(field(read_meta(&with("attestation", json!({ "accepted": false, "text_rev": "2026-10-19" })), true)), "attestation");
    assert_eq!(field(read_meta(&with("attestation", json!({ "accepted": true, "text_rev": "1999-01-01" })), true)), "attestation");
}

#[test]
fn a_new_publish_has_no_base_and_a_republish_needs_one_and_no_visibility() {
    assert_eq!(field(read_meta(&with("base_publish_id", json!(PID2)), true)), "base_publish_id");
    assert!(read_meta(&with("base_publish_id", Value::Null), true).is_ok(), "null counts as absent");
    assert_eq!(field(read_meta(&good(), false)), "base_publish_id");
    let m = read_meta(&with("base_publish_id", json!(PID2)), false).unwrap();
    assert_eq!((m.base.as_deref(), m.visibility), (Some(PID2), None));
}

#[test]
fn visibility_is_public_or_unlisted_and_a_circle_only_licence_cannot_be_public() {
    assert_eq!(field(read_meta(&with("visibility", json!("secret")), true)), "visibility");
    let mut m = with("license", json!("circle-only"));
    let e = read_meta(&m, true).err().unwrap();
    assert_eq!((e.status, e.code.as_str()), (422, "license_not_public"));
    m.insert("visibility".into(), json!("unlisted"));
    assert!(read_meta(&m, true).is_ok());
}

#[test]
fn labels_order_acks_and_image_ok_are_validated_and_bounded() {
    let m = read_meta(&with("labels", json!([{ "question": 0, "choice": "strip" }, { "question": 2, "part": 1, "choice": "keep" }])), true).unwrap();
    assert_eq!(m.labels, vec![LabelDecision { question: 0, part: None, choice: "strip".into() }, LabelDecision { question: 2, part: Some(1), choice: "keep".into() }]);
    for bad in [json!([{ "question": 0, "choice": "maybe" }]), json!([{ "question": -1, "choice": "keep" }]), json!([{ "question": 0.5, "choice": "keep" }]), json!([{ "question": 0, "part": "x", "choice": "keep" }]), json!([5]), json!("no")] {
        assert_eq!(field(read_meta(&with("labels", bad), true)), "labels");
    }
    assert_eq!(field(read_meta(&with("labels", json!((0..60_001).map(|_| json!({ "question": 0, "choice": "keep" })).collect::<Vec<_>>())), true)), "labels");
    let m = read_meta(&with("order_acks", json!([{ "question": 1 }, { "question": 3, "part": 0 }])), true).unwrap();
    assert_eq!(m.order_acks, vec![Pos { question: 1, part: None }, Pos { question: 3, part: Some(0) }]);
    assert_eq!(field(read_meta(&with("order_acks", json!([{ "question": 1_000_000 }])), true)), "order_acks");
    assert_eq!(read_meta(&with("image_ok", json!([0, 4])), true).unwrap().image_ok, vec![0, 4]);
    assert_eq!(field(read_meta(&with("image_ok", json!([0, "4"])), true)), "image_ok");
    assert_eq!(field(read_meta(&with("image_ok", json!((0..1001).collect::<Vec<_>>())), true)), "image_ok");
}

#[test]
fn acknowledged_formatting_is_deduplicated_in_order() {
    assert_eq!(read_meta(&with("ack_formatting", json!([3, 1, 3, 2, 1])), true).unwrap().ack_formatting, vec![3, 1, 2]);
    assert_eq!(field(read_meta(&with("ack_formatting", json!([1.5])), true)), "ack_formatting");
}

#[test]
fn an_independence_claim_needs_acceptance_real_ids_and_a_statement() {
    let id1 = "0123456789abcdef0123456789abcdef";
    let id2 = "fedcba9876543210fedcba9876543210";
    let claim = |items: Value, statement: Value, accepted: bool| with("independent_of", json!({ "accepted": accepted, "items": items, "statement": statement }));
    let m = read_meta(&claim(json!([id1, id2, id1]), json!("  I wrote these myself.  "), true), true).unwrap();
    assert_eq!((m.independent, m.statement.as_deref()), (vec![id1.to_string(), id2.to_string()], Some("I wrote these myself.")));
    assert_eq!(field(read_meta(&claim(json!([id1]), json!("I wrote these myself."), false), true)), "independent_of");
    assert_eq!(field(read_meta(&claim(json!([]), json!("I wrote these myself."), true), true)), "independent_of");
    assert_eq!(field(read_meta(&claim(json!(["nope"]), json!("I wrote these myself."), true), true)), "independent_of");
    assert_eq!(field(read_meta(&claim(json!((0..51).map(|_| id1).collect::<Vec<_>>()), json!("I wrote these myself."), true), true)), "independent_of");
    assert_eq!(field(read_meta(&claim(json!([id1]), json!("too short"), true), true)), "independent_of.statement");
    assert_eq!(field(read_meta(&claim(json!([id1]), Value::Null, true), true)), "independent_of.statement");
    assert!(read_meta(&with("independent_of", Value::Null), true).unwrap().independent.is_empty());
}

#[test]
fn the_publishing_terms_affirmation_is_recorded_only_when_well_formed() {
    let m = read_meta(&with("attest", json!({ "own_work": true, "terms_version": "v1" })), true).unwrap();
    assert_eq!(m.attest, Some(Attest { own_work: true, terms_version: "v1".into() }));
    assert_eq!(read_meta(&with("attest", json!({ "own_work": "yes", "terms_version": "v1" })), true).unwrap().attest.unwrap().own_work, false);
    assert!(read_meta(&with("attest", json!({ "own_work": true })), true).unwrap().attest.is_none());
    assert!(read_meta(&with("attest", json!([1])), true).unwrap().attest.is_none());
    let long = "x".repeat(60);
    assert_eq!(read_meta(&with("attest", json!({ "own_work": true, "terms_version": long })), true).unwrap().attest.unwrap().terms_version.len(), 40);
}

#[test]
fn the_free_form_meta_is_passed_on_or_defaults_to_an_empty_object() {
    assert_eq!(read_meta(&with("meta", json!({ "title": "T" })), true).unwrap().meta, json!({ "title": "T" }));
    assert_eq!(read_meta(&with("meta", Value::Null), true).unwrap().meta, json!({}));
}
