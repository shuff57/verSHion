//! Tests for `publishderive.rs`: the per-question licence rule over hand-made evidence on top of a real pipeline's hashes: refusals, soft flags, grandfathering,
//! earlier publications, hidden-text refusals and the attribution words.
use super::*;
use crate::blobs::BlobStore;
use crate::db;
use crate::pool::core::Core;
use crate::pool::ctx::PoolCtx;
use crate::pool::fingerprint::Fingerprinter;
use crate::pool::publishfacts::{PrevRow, gather};
use crate::pool::publishmeta::read_meta;
use crate::pool::publishrewrite::{Declared, rewrite};
use crate::pool::publishstart::Landing;
use crate::pool::publishunits::Lift;
use crate::pool::sqlfrag::DAY_MS;
use serde_json::{Value, json};
use std::path::Path;

const KEY: &str = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";
const ROOT: &str = "7f757fb737085dcaaad0727deb89a14c18141681dc56be0a215a592c715f8e3b";
const ITEM: &str = "0123456789abcdef0123456789abcdef";
const SRC: &str = "11111111111111111111111111111111";

struct B {
    m: PublishMeta,
    rw: Rewritten,
    h: Hashed,
    f: Facts,
    e: Evid,
    u: Units,
}
fn mc(stem: &str) -> Value {
    let words = ["apple", "river", "stone", "cloud", "ember", "maple", "orbit", "quartz"];
    let answers: Vec<Value> = words.iter().enumerate().map(|(i, t)| json!({ "text": t, "correct": i == 3, "locked": false })).collect();
    json!({ "id": "q", "stem": stem, "kind": { "Mc": { "answers": answers } }, "points": null, "difficulty": null, "topics": [], "image": null, "explanation": "because", "figure": null, "created": 1, "modified": 1 })
}
fn base(license: &str) -> B {
    let dir = tempfile::tempdir().unwrap();
    let mut c = db::open(&dir.path().join("t.db")).unwrap();
    db::migrate(&mut c, &Path::new(env!("CARGO_MANIFEST_DIR")).join("../worker/migrations")).unwrap();
    c.execute("INSERT INTO users (id, email, display_name, trust, created_at) VALUES ('me', 'me@x.org', 'me', 'teacher', 1)", []).unwrap();
    let store = BlobStore::new(dir.path().join("blobs")).unwrap();
    let (core, fp) = (Core::new(Some(KEY.into())), Fingerprinter::from_root_hex(ROOT).unwrap());
    let payload = json!({ "schema_version": 1, "bank": { "id": "local", "name": "B", "questions": [mc("What is the capital of France?")], "created": 1, "modified": 1 } }).to_string().into_bytes();
    let m = json!({ "publish_id": "AAAAAAAAAAAAAAAAAAAAAA", "hide_key": false, "license": license, "visibility": "unlisted", "attestation": { "accepted": true, "text_rev": "2026-10-19" }, "meta": { "title": "Q", "subject": "S" } });
    let m = read_meta(m.as_object().unwrap(), true).unwrap();
    let land = Landing { item_id: ITEM.into(), version: 1, visibility: "unlisted".into(), item_kind: None, base_of: None, prev_public_version: 0, prev_public_at: None, prev_hidden: false, prev_license: None };
    let x = PoolCtx::new(&c, 5 * DAY_MS);
    let rw = rewrite(&x, &store, &core, "me", &land, true, &m, &payload, false).unwrap();
    let h = crate::pool::publishhash::prepare(&x, &store, &core, &fp, &land, true, &m, &rw).unwrap();
    let f = gather(&x, &store, &fp, "me", None, true, false, &m, &rw, &h, None).unwrap();
    B { m, rw, h, f, e: Evid::default(), u: Units::default() }
}
fn hit(license: &str, kind: &str, near: bool) -> ImportHit {
    ImportHit {
        pos: 0,
        qid: "sq".into(),
        item_id: SRC.into(),
        chain_len: 1,
        author_name: "Bea".into(),
        license: license.into(),
        version: 1,
        versions: vec![1],
        source_first_seq: 7,
        kind: kind.into(),
        same: near,
        same_full: near,
        near,
        kf_same: near,
        lic_all: vec![license.into()],
        lic_same: vec![license.into()],
        lic_near: if near { vec![license.into()] } else { vec![] },
        lic_groups: vec![vec![license.into()]],
        contents: vec!["c".into()],
        by_match: true,
        ..Default::default()
    }
}
fn run(b: &B) -> Derived {
    derive(&b.m, &b.rw, &b.h, &b.f, &b.e, &b.u, None, false)
}

#[test]
fn a_question_with_no_sources_allows_every_licence_and_credits_nobody() {
    let b = base("CC-BY-4.0");
    let d = run(&b);
    assert_eq!(d.entries, vec![Entry::default()]);
    assert!(d.incompatible.is_empty() && d.soft_only.is_empty() && d.grandfathered.is_empty() && d.conflicts.is_empty());
    assert_eq!(d.per_question[0].len(), 6);
    let aj: Value = serde_json::from_str(&d.allowed_json[0]).unwrap();
    assert_eq!(aj["allowed"].as_array().unwrap().len(), 6);
    assert_eq!(d.marks, vec![Marks::default()]);
}

#[test]
fn an_imported_copy_under_a_licence_that_does_not_allow_ours_refuses_and_names_the_binder() {
    let mut b = base("CC-BY-4.0");
    b.e.ev.by_pos.insert(0, vec![hit("CC-BY-NC-4.0", "import", true)]);
    let d = run(&b);
    assert_eq!(d.incompatible, vec![0]);
    assert_eq!(d.binders[0], vec![SRC.to_string()]);
    assert!(!d.per_question[0].contains(&"CC-BY-4.0"));
    assert_eq!((d.entries[0].origin_item.as_deref(), d.entries[0].modified), (Some(SRC), Some(false)));
    assert_eq!(d.entries[0].chain[0].item_id, SRC);
}

#[test]
fn a_verbatim_copy_under_ours_is_credited_with_the_licence_relied_on() {
    let mut b = base("CC-BY-4.0");
    b.e.ev.by_pos.insert(0, vec![hit("CC-BY-4.0", "import", true)]);
    let d = run(&b);
    assert!(d.incompatible.is_empty());
    assert_eq!(d.entries[0].chain.iter().map(|l| (l.item_id.as_str(), l.license.as_str(), l.modified)).collect::<Vec<_>>(), vec![(SRC, "CC-BY-4.0", false)]);
}

#[test]
fn a_changed_copy_of_something_only_read_is_soft_never_a_refusal() {
    let mut b = base("CC-BY-4.0");
    b.e.ev.by_pos.insert(0, vec![hit("CC-BY-NC-4.0", "view", false)]);
    let d = run(&b);
    assert!(d.incompatible.is_empty());
    assert_eq!(d.soft_only, vec![0]);
    assert!(d.marks[0].soft);
    assert_eq!(d.entries[0].modified, Some(true), "credited as modified");
}

#[test]
fn an_untouched_question_keeps_its_earlier_outcome_and_is_grandfathered() {
    let mut b = base("CC-BY-4.0");
    b.e.ev.by_pos.insert(0, vec![hit("CC-BY-NC-4.0", "import", true)]);
    b.f.prev.insert(b.rw.qids[0].clone(), PrevRow { full_content: b.h.full_content[0].clone(), allowed_json: Some("{\"allowed\":[\"CC-BY-4.0\"],\"binders\":[]}".into()), chain_json: "[]".into(), ..Default::default() });
    let d = derive(&b.m, &b.rw, &b.h, &b.f, &b.e, &b.u, Some("CC-BY-4.0"), false);
    assert_eq!((d.grandfathered.clone(), d.incompatible.clone()), (vec![0], vec![]));
    assert_eq!(d.allowed_json[0], "{\"allowed\":[\"CC-BY-4.0\"],\"binders\":[]}");
    // A different licence than before is touched again.
    let d = derive(&b.m, &b.rw, &b.h, &b.f, &b.e, &b.u, Some("CC0-1.0"), false);
    assert_eq!(d.incompatible, vec![0]);
}

#[test]
fn a_declared_origin_nobody_can_verify_is_listed() {
    let mut b = base("CC-BY-4.0");
    b.rw.declared[0] = Some(Declared { item: SRC.into(), qid: "q9".into(), version: 1 });
    let d = run(&b);
    assert_eq!(d.unverified, vec![(0, SRC.to_string())]);
    b.e.own_items.insert(SRC.into());
    assert!(run(&b).unverified.is_empty(), "the account's own item needs no verification");
}

fn lift(kind: UnitKind, served: bool) -> Lift {
    Lift {
        item_id: SRC.into(),
        qid: "sq".into(),
        version: 1,
        license: "CC-BY-NC-4.0".into(),
        author_name: "Bea".into(),
        seq: 3,
        chain: "[]".into(),
        kind,
        key: "k1".into(),
        served,
        ev_kind: served.then_some("import"),
        received: vec!["CC-BY-NC-4.0".into()],
        stock: false,
        own_first: false,
        visible: true,
        uploaded_first: false,
        hidden: false,
        stem_only: false,
    }
}

#[test]
fn a_unit_from_an_item_never_served_is_an_earlier_publication_not_a_refusal() {
    let mut b = base("CC-BY-4.0");
    b.u.lifts.insert(0, vec![lift(UnitKind::Image, false)]);
    let d = run(&b);
    assert!(d.incompatible.is_empty());
    assert_eq!((d.entries[0].chain[0].earlier_match, d.entries[0].chain[0].kind.as_deref()), (Some(true), Some("image")));
    assert!(d.marks[0].earlier && d.soft_only == vec![0]);
}

#[test]
fn a_served_unit_binds_like_an_adaptation() {
    let mut b = base("CC-BY-4.0");
    b.u.lifts.insert(0, vec![lift(UnitKind::Text, true)]);
    let d = run(&b);
    assert_eq!(d.incompatible, vec![0]);
    assert_eq!(d.binders[0], vec![SRC.to_string()]);
}

#[test]
fn an_independence_claim_covers_a_read_image_only_with_attribution() {
    let mut b = base("CC-BY-4.0");
    let mut l = lift(UnitKind::Image, true);
    l.ev_kind = Some("view");
    b.u.lifts.insert(0, vec![l]);
    b.m.independent = vec![SRC.into()];
    assert_eq!(run(&b).incompatible, vec![0], "no attribution: the claim does not cover it");
    b.m.statement = Some("This is a diagram from Wikipedia, in the public domain.".into());
    let d = run(&b);
    assert!(d.incompatible.is_empty() && d.marks[0].claimed);
    assert_eq!(d.entries[0].chain[0].possible_source, Some(true));
}

#[test]
fn a_hidden_text_served_elsewhere_refuses_to_the_author_alone() {
    let mut b = base("CC-BY-4.0");
    let mut l = lift(UnitKind::Text, true);
    l.hidden = true;
    b.u.lifts.insert(0, vec![l]);
    let d = run(&b);
    assert_eq!(d.hidden_refused, vec![0]);
    assert_eq!(d.hidden_from, vec![(SRC.to_string(), vec!["CC-BY-NC-4.0".to_string()])]);
    assert!(d.incompatible.is_empty(), "nothing others can observe depends on it");
}

#[test]
fn attribution_words_are_whole_words_in_any_case() {
    for yes in ["Public Domain image", "licensed CC-BY 4.0", "cc by", "CCBY", "CC0", "from OpenStax", "see wikimedia.", "(Wikipedia)", "Creative Commons"] {
        assert!(attributes_public_source(yes), "{yes}");
    }
    for no in ["found at https://example.org", "my own work", "abccby", "cc0x", "wikipediax", "republic domain"] {
        assert!(!attributes_public_source(no), "{no}");
    }
}
