//! Tests for `publishfacts.rs`: short statements, served word counts, block keys of the served form, key work, lineage of a kept qid and the author's marks.
use super::*;
use crate::db;
use crate::pool::core::Core;
use crate::pool::publishmeta::read_meta;
use crate::pool::publishrewrite::rewrite;
use crate::pool::publishstart::Landing;
use crate::pool::sqlfrag::DAY_MS;
use rusqlite::Connection;
use std::path::Path;

const KEY: &str = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";
const ROOT: &str = "7f757fb737085dcaaad0727deb89a14c18141681dc56be0a215a592c715f8e3b";
const ITEM: &str = "0123456789abcdef0123456789abcdef";

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
fn tf(stem: &str) -> Value {
    json!({ "id": "q", "stem": stem, "kind": { "Tf": { "correct": true } }, "points": null, "difficulty": null, "topics": [], "image": null, "explanation": "", "figure": null, "created": 1, "modified": 1 })
}
fn gather_for(w: &W, hide: bool, qs: Vec<Value>) -> Facts {
    let payload = json!({ "schema_version": 1, "bank": { "id": "local", "name": "B", "questions": qs, "created": 1, "modified": 1 } }).to_string().into_bytes();
    let m = json!({ "publish_id": "AAAAAAAAAAAAAAAAAAAAAA", "hide_key": hide, "license": "CC-BY-4.0", "visibility": "unlisted", "attestation": { "accepted": true, "text_rev": "2026-10-19" }, "meta": { "title": "Q", "subject": "S" } });
    let m = read_meta(m.as_object().unwrap(), true).unwrap();
    let land = Landing { item_id: ITEM.into(), version: 1, visibility: "unlisted".into(), item_kind: None, base_of: None, prev_public_version: 0, prev_public_at: None, prev_hidden: false, prev_license: None };
    let x = PoolCtx::new(&w.c, 5 * DAY_MS);
    let rw = rewrite(&x, &w.s, &w.core, "me", &land, true, &m, &payload, false).unwrap();
    let h = crate::pool::publishhash::prepare(&x, &w.s, &w.core, &w.fp, &land, true, &m, &rw).unwrap();
    gather(&x, &w.s, &w.fp, "me", None, true, false, &m, &rw, &h, None).unwrap()
}

#[test]
fn short_statements_are_decided_on_the_served_form() {
    assert!(is_short(&tf("Is water wet today")));
    assert!(!is_short(&tf("one two three four five six seven eight nine")), "nine words");
    let mut with_expl = tf("Short one");
    with_expl["explanation"] = json!("because");
    assert!(!is_short(&with_expl));
    let mut keyless = tf("Short one");
    keyless["keyless"] = json!(true);
    assert!(!is_short(&keyless));
    assert!(is_short(&json!({ "stem": "Capital of France", "kind": { "Short": { "sample": "Paris" } } })));
    assert!(!is_short(&json!({ "stem": "Capital of France", "kind": { "Short": { "sample": "" } } })), "no served sample (hidden key)");
    assert!(is_short(&json!({ "stem": "Fill", "kind": { "Blank": { "accept": [["a", "b"], "c"] } } })));
    assert!(!is_short(&json!({ "stem": "Fill", "kind": { "Blank": { "accept": [] } } })));
    assert!(is_short(&json!({ "stem": "Pick", "kind": { "Mc": { "answers": [{ "text": "a b" }, { "text": "c" }] } } })));
    assert!(!is_short(&mc("Pick one of the many")), "eight choices and a stem exceed twelve words");
    assert!(!is_short(&json!({ "stem": "x", "kind": { "Matching": {} } })));
}

#[test]
fn served_words_count_text_but_not_ids_images_or_topics() {
    let q = json!({ "id": "one two", "stem": "alpha beta", "topics": ["x y z"], "image": { "file": "a b" }, "kind": { "Mc": { "answers": [{ "text": "gamma" }, { "text": "delta  epsilon" }] } }, "explanation": " zeta " });
    assert_eq!(served_word_count(&q), 6);
}

#[test]
fn a_new_banks_facts_cover_every_position_and_dedupe_the_block_keys() {
    let w = world();
    let f = gather_for(&w, false, vec![mc("What is the capital of France?"), tf("Is water wet today")]);
    assert_eq!((f.bk_norm.len(), f.bk_stem.len(), f.body.len(), f.short_q.len(), f.lineage.len(), f.img_at.len()), (2, 2, 2, 2, 2, 2));
    assert_eq!(f.short_q, vec![false, true]);
    assert_eq!(f.short40, vec![true, true]);
    assert!(f.lineage.iter().all(Option::is_none) && f.prev.is_empty() && f.carried_refs.is_empty());
    assert_eq!(f.block_keys.len(), f.block_keys.iter().collect::<HashSet<_>>().len());
    assert!(f.block_keys.iter().any(|k| k.starts_with("n:")) && f.block_keys.iter().any(|k| k.starts_with("s:")) && f.block_keys.iter().any(|k| k.starts_with("f:")));
    assert_eq!(f.lineage_root.len(), 2);
    assert_ne!(f.body[0], f.body[1]);
    assert_eq!(f.keywork, vec![false, false]);
    assert!(f.circle_claimed.is_empty() && f.refs.iter().all(Vec::is_empty));
}

#[test]
fn a_hidden_banks_block_keys_are_of_the_served_form_only() {
    let w = world();
    let f = gather_for(&w, true, vec![mc("What is the capital of France?")]);
    assert_eq!(f.bk_full.len(), 1);
    assert!(f.bk_full[0].starts_with("f:"));
    assert_eq!(f.stem_record.len(), 1, "the full form's explanation key is kept for the durable record");
    assert!(!f.stem_record[0].1.is_empty());
    assert_eq!(f.hidden_text.len(), 1);
}

#[test]
fn long_explanations_are_text_units_and_a_short_samples_key_is_a_work() {
    let w = world();
    let mut q = mc("Why does the moon show phases?");
    q["explanation"] = json!((0..30).map(|i| format!("word{i}")).collect::<Vec<_>>().join(" "));
    let f = gather_for(&w, false, vec![q]);
    assert_eq!(f.served_text[0].len(), 2, "an explanation can be pasted as a model answer and the other way round");
    let sq = json!({ "id": "q", "stem": "Name a primary colour", "kind": { "Short": { "sample": "red" } }, "points": null, "difficulty": null, "topics": [], "image": null, "explanation": "", "figure": null, "created": 1, "modified": 1 });
    assert_eq!(gather_for(&world(), false, vec![sq]).keywork, vec![true]);
}
