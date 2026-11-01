//! Tests for `publishhash.rs`: the one share-core call and the hashes derived from it, for a bank that shows its key and one that hides it.
use super::*;
use crate::db;
use crate::pool::publishmeta::read_meta;
use crate::pool::publishrewrite::rewrite;
use crate::pool::sqlfrag::DAY_MS;
use rusqlite::Connection;
use std::path::Path;

const KEY: &str = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";
const ROOT: &str = "7f757fb737085dcaaad0727deb89a14c18141681dc56be0a215a592c715f8e3b";
const PID: &str = "AAAAAAAAAAAAAAAAAAAAAA";

struct W {
    c: Connection,
    _d: tempfile::TempDir,
    s: BlobStore,
    core: Core,
    fp: Fingerprinter,
}
fn world() -> W {
    let dir = tempfile::tempdir().unwrap();
    let mut c = db::open(&dir.path().join("t.db")).unwrap();
    db::migrate(&mut c, &Path::new(env!("CARGO_MANIFEST_DIR")).join("../worker/migrations")).unwrap();
    c.execute("INSERT INTO users (id, email, display_name, trust, created_at) VALUES ('me', 'me@x.org', 'me', 'teacher', 1)", []).unwrap();
    let s = BlobStore::new(dir.path().join("blobs")).unwrap();
    W { c, s, core: Core::new(Some(KEY.into())), fp: Fingerprinter::from_root_hex(ROOT).unwrap(), _d: dir }
}
fn mc(stem: &str) -> Value {
    let words = ["apple", "river", "stone", "cloud", "ember", "maple", "orbit", "quartz"];
    let answers: Vec<Value> = words.iter().enumerate().map(|(i, t)| json!({ "text": t, "correct": i == 3, "locked": false })).collect();
    json!({ "id": "q", "stem": stem, "kind": { "Mc": { "answers": answers } }, "points": null, "difficulty": null, "topics": [], "image": null, "explanation": "because", "figure": null, "created": 1, "modified": 1 })
}
fn bank(qs: Vec<Value>) -> Vec<u8> {
    json!({ "schema_version": 1, "bank": { "id": "local", "name": "My bank", "questions": qs, "created": 1, "modified": 1 } }).to_string().into_bytes()
}
fn landing() -> Landing {
    Landing { item_id: "0123456789abcdef0123456789abcdef".into(), version: 1, visibility: "unlisted".into(), item_kind: None, base_of: None, prev_public_version: 0, prev_public_at: None, prev_hidden: false, prev_license: None }
}
fn run(w: &W, hide: bool, qs: Vec<Value>) -> Res<Hashed> {
    let m = json!({ "publish_id": PID, "hide_key": hide, "license": "CC-BY-4.0", "visibility": "unlisted", "attestation": { "accepted": true, "text_rev": "2026-10-19" }, "meta": { "title": "Quiz", "subject": "Science" } });
    let m = read_meta(m.as_object().unwrap(), true).unwrap();
    let x = PoolCtx::new(&w.c, 5 * DAY_MS);
    let rw = rewrite(&x, &w.s, &w.core, "me", &landing(), true, &m, &bank(qs), false)?;
    prepare(&x, &w.s, &w.core, &w.fp, &landing(), true, &m, &rw)
}

#[test]
fn a_shown_key_bank_has_full_subjects_and_one_hash_of_each_kind_per_question() {
    let w = world();
    let h = run(&w, false, vec![mc("What is the capital of France?"), mc("Name the largest planet in our solar system")]).unwrap();
    assert!(h.prep.stripped.is_none() && h.stripped_content.is_none() && h.near_stripped.is_none() && h.reused.is_none());
    for len in [h.match_hashes.len(), h.full_hashes.len(), h.full_content.len(), h.keyfree.len(), h.near_full.len(), h.bare_full.len(), h.near_kf.len(), h.subjects.len(), h.stems.len(), h.tokens.len(), h.tok_sk.len(), h.k_bands.len()] {
        assert_eq!(len, 2);
    }
    assert_eq!((h.public_version, h.public_at), (1, None));
    assert!(h.subjects.iter().all(|s| s.full.is_some() && s.near.is_some()));
    assert_ne!(h.full_content[0], h.full_content[1]);
    assert!(h.stems.iter().all(Option::is_some));
    assert!(h.tokens[0].contains(&"capital".to_string()));
    assert_eq!(h.tok_sk[0].len(), h.tokens[0].len(), "one keyed word per distinct token");
    assert!(h.search_words.contains(&"france".to_string()) && h.search_words.contains(&"planet".to_string()));
    assert_eq!(h.served, h.full_qs, "a shown key serves the full form");
}

#[test]
fn a_hidden_key_bank_serves_the_stripped_form_and_its_subjects_carry_no_key() {
    let w = world();
    let h = run(&w, true, vec![mc("What is the capital of France?")]).unwrap();
    assert!(h.prep.stripped.is_some());
    let sc = h.stripped_content.as_ref().expect("stripped contents");
    assert_eq!(sc.len(), 1);
    assert_eq!(h.near_stripped.as_ref().unwrap().len(), 1);
    assert_eq!(h.bare_stripped.as_ref().unwrap().len(), 1);
    assert!(h.subjects[0].full.is_none() && h.subjects[0].near.is_none() && h.subjects[0].near_bare.is_none() && !h.subjects[0].no_expl);
    assert_ne!(h.served, h.full_qs, "the key is not in what is served");
    assert_eq!(h.match_hashes[0], h.prep.stripped.as_ref().unwrap().match_hashes[0]);
}

#[test]
fn the_derivations_are_deterministic_and_follow_the_text() {
    let w = world();
    let a = run(&w, false, vec![mc("Same words here please")]).unwrap();
    let b = run(&w, false, vec![mc("Same words here please")]).unwrap();
    assert_eq!((&a.full_content, &a.keyfree, &a.near_full, &a.tok_sk, &a.k_bands), (&b.full_content, &b.keyfree, &b.near_full, &b.tok_sk, &b.k_bands));
    let c = run(&w, false, vec![mc("Different words entirely now")]).unwrap();
    assert_ne!(a.full_content, c.full_content);
    assert_ne!(a.stems, c.stems);
}

#[test]
fn a_question_with_no_explanation_is_flagged() {
    let w = world();
    let mut q = mc("No explanation on this one");
    q["explanation"] = json!("   ");
    let h = run(&w, false, vec![q, mc("Has one for sure here")]).unwrap();
    assert_eq!(h.no_expl, vec![true, false]);
}

#[test]
fn search_words_are_distinct_bounded_and_skip_oversize_words() {
    let long = "x".repeat(41);
    let qs = vec![json!({ "stem": format!("Alpha beta ALPHA {long} gamma-delta") }), json!({ "stem": 5 }), json!({ "stem": "beta epsilon" })];
    assert_eq!(served_words(&qs), vec!["alpha", "beta", "gamma", "delta", "epsilon"]);
    let many = json!({ "stem": (0..2500).map(|i| format!("w{i}")).collect::<Vec<_>>().join(" ") });
    assert_eq!(served_words(&[many]).len(), SEARCH_WORDS_MAX);
}
