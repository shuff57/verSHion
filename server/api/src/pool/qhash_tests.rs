//! Tests for `qhash.rs`: every value in qhash_vectors.json was produced by the Worker's own provenance.ts functions
//! (run under Node) over the same questions, so a mismatch here is a mismatch with every stored hash.
use super::*;
use serde_json::Value;

fn vectors() -> Value {
    serde_json::from_str(include_str!("qhash_vectors.json")).unwrap()
}
fn fp(v: &Value) -> Fingerprinter {
    Fingerprinter::from_root_hex(v["root"].as_str().unwrap()).unwrap()
}
fn strs(v: &Value) -> Vec<String> {
    v.as_array().unwrap().iter().map(|x| x.as_str().unwrap().to_string()).collect()
}
fn img(q: &Json) -> Option<String> {
    (q.get("id").and_then(Json::as_str) == Some("q2")).then(|| "sha-of-image".to_string())
}

#[test]
fn text_normalisation_matches_the_worker() {
    let v = vectors();
    let texts = strs(&v["texts"]);
    let want = strs(&v["norm"]);
    for (t, w) in texts.iter().zip(&want) {
        assert_eq!(&norm_text(t), w, "{t:?}");
    }
}

#[test]
fn every_question_hash_matches_the_worker() {
    let v = vectors();
    let f = fp(&v);
    // Parsed as Json, not serde_json::Value: key order is part of the hashed bytes.
    let raw = Json::parse(include_bytes!("qhash_vectors.json")).unwrap();
    for j in raw.get("questions").unwrap().as_arr() {
        let id = j.get("id").and_then(Json::as_str).unwrap();
        let r = &v["results"][id];
        let j = j.clone();
        let mh = format!("mh-{id}");
        assert_eq!(near_hash(&f, &j, false), r["near"].as_str().unwrap(), "{id} near");
        assert_eq!(near_hash(&f, &j, true), r["nearBare"].as_str().unwrap(), "{id} nearBare");
        assert_eq!(keyfree_hash(&f, &mh, &j, &img), r["keyfree"].as_str().unwrap(), "{id} keyfree");
        assert_eq!(near_kf_hash(&f, &mh, &j, &img), r["nearKf"].as_str().unwrap(), "{id} nearKf");
        assert_eq!(norm_skeleton_key(&f, &j), r["skeleton"].as_str().unwrap(), "{id} skeleton");
        assert_eq!(stem_key(&f, &j), r["stemKey"].as_str().unwrap(), "{id} stemKey");
        let (e, a) = field_keys(&f, &j);
        assert_eq!(e.as_deref(), r["fieldKeys"]["expl"].as_str(), "{id} expl");
        assert_eq!(a.as_deref(), r["fieldKeys"]["ans"].as_str(), "{id} ans");
        let parts: Vec<(usize, String)> = r["partKeys"].as_array().unwrap().iter().map(|p| (p["part"].as_u64().unwrap() as usize, p["key"].as_str().unwrap().to_string())).collect();
        assert_eq!(part_keys_of(&f, &j), parts, "{id} partKeys");
        let units: Vec<UnitKey> = r["unitKeys"].as_array().unwrap().iter().map(|u| UnitKey { part: u["part"].as_i64().unwrap(), key: u["key"].as_str().unwrap().to_string(), stem_only: u["stemOnly"].as_bool().unwrap() }).collect();
        assert_eq!(unit_keys_of(&f, &j), units, "{id} unitKeys");
        assert_eq!(body_tokens(&j), strs(&r["body"]), "{id} body");
        let toks = stem_tokens(j.get("stem").unwrap());
        assert_eq!(toks, strs(&r["stemTokens"]), "{id} stemTokens");
        assert_eq!(stem_bands(&toks), strs(&r["stemBands"]), "{id} stemBands");
        assert_eq!(stem_hash(&f, j.get("stem").unwrap()).as_deref(), r["stemHash"].as_str(), "{id} stemHash");
        assert_eq!(order_free(&j).stringify(), r["orderFree"].as_str().unwrap(), "{id} orderFree");
    }
}

#[test]
fn field_keys_for_a_pasted_text_and_the_small_comparisons_match() {
    let v = vectors();
    let f = fp(&v);
    assert_eq!(text_field_keys(&f, "The  Same, text!"), strs(&v["textFieldKeys"]));
    assert!(text_field_keys(&f, " \u{a0}").is_empty());
    let s = |x: &[&str]| x.iter().map(|y| y.to_string()).collect::<Vec<_>>();
    let j = &v["jaccard"];
    assert_eq!(jaccard(&s(&["a", "b", "c"]), &s(&["b", "c", "d"])), j[0].as_f64().unwrap());
    assert_eq!(jaccard(&[], &[]), j[1].as_f64().unwrap());
    assert_eq!(jaccard(&s(&["a"]), &s(&["a"])), j[2].as_f64().unwrap());
    let sb = &v["sameBody"];
    let (tf, mc1, mc2, sh) = (s(&["kind:Tf"]), s(&["kind:Mc", "a", "b"]), s(&["kind:Mc", "a", "c"]), s(&["kind:Short", "a", "b"]));
    assert_eq!(same_body(Some(&tf), Some(&tf)), sb[0].as_bool().unwrap());
    assert_eq!(same_body(Some(&mc1), Some(&mc2)), sb[1].as_bool().unwrap());
    assert_eq!(same_body(Some(&mc1), Some(&sh)), sb[2].as_bool().unwrap());
    assert_eq!(same_body(None, Some(&tf)), sb[3].as_bool().unwrap());
}

#[test]
fn a_short_or_non_string_stem_has_no_loose_hash_and_few_tokens_no_bands() {
    let f = fp(&vectors());
    assert_eq!(stem_hash(&f, &Json::s("too short")), None);
    assert_eq!(stem_hash(&f, &Json::Num(5.0)), None);
    assert!(stem_hash(&f, &Json::s("long enough to mean something")).is_some());
    assert!(stem_bands(&["a".into(), "b".into(), "c".into()]).is_empty());
    assert!(stem_tokens(&Json::Null).is_empty());
}
