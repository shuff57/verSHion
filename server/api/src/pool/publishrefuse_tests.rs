//! Tests for `publishrefuse.rs`: the earlier-identical credit through a real pipeline and a public source, and the two `license_incompatible` refusals over
//! hand-made derivations.
use super::*;
use crate::blobs::BlobStore;
use crate::db;
use crate::pool::core::Core;
use crate::pool::fingerprint::Fingerprinter;
use crate::pool::publishderive::{Entry, Marks};
use crate::pool::publishevid::collect;
use crate::pool::publishfacts::gather;
use crate::pool::publishmeta::read_meta;
use crate::pool::publishrewrite::rewrite;
use crate::pool::publishstart::Landing;
use crate::pool::publishunits::Units;
use crate::pool::sqlfrag::DAY_MS;
use rusqlite::Connection;
use std::path::Path;

const KEY: &str = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";
const ROOT: &str = "7f757fb737085dcaaad0727deb89a14c18141681dc56be0a215a592c715f8e3b";
const ITEM: &str = "0123456789abcdef0123456789abcdef";
const SRC: &str = "11111111111111111111111111111111";

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
    for (id, name) in [("me", "Me"), ("b", "Bea")] {
        c.execute("INSERT INTO users (id, email, display_name, trust, created_at) VALUES (?1, ?2, ?3, 'teacher', 1)", params![id, format!("{id}@x.org"), name]).unwrap();
    }
    let s = BlobStore::new(dir.path().join("blobs")).unwrap();
    W { c, s, core: Core::new(Some(KEY.into())), fp: Fingerprinter::from_root_hex(ROOT).unwrap(), _d: dir }
}
fn mc(stem: &str) -> Value {
    let words = ["apple", "river", "stone", "cloud", "ember", "maple", "orbit", "quartz"];
    let answers: Vec<Value> = words.iter().enumerate().map(|(i, t)| json!({ "text": t, "correct": i == 3, "locked": false })).collect();
    json!({ "id": "q", "stem": stem, "kind": { "Mc": { "answers": answers } }, "points": null, "difficulty": null, "topics": [], "image": null, "explanation": "because", "figure": null, "created": 1, "modified": 1 })
}
fn pipeline(w: &W, independent: &[&str]) -> (PublishMeta, Rewritten, Hashed, Facts, Evid) {
    let payload = json!({ "schema_version": 1, "bank": { "id": "local", "name": "B", "questions": [mc("Which of these words names a kind of fruit grown in orchards?")], "created": 1, "modified": 1 } }).to_string().into_bytes();
    let m = json!({ "publish_id": "AAAAAAAAAAAAAAAAAAAAAA", "hide_key": false, "license": "CC-BY-4.0", "visibility": "unlisted", "attestation": { "accepted": true, "text_rev": "2026-10-19" }, "meta": { "title": "Q", "subject": "S" } });
    let mut m = m;
    if !independent.is_empty() {
        m["independent_of"] = json!({ "accepted": true, "items": independent, "statement": "I wrote these myself without copying." });
    }
    let m = read_meta(m.as_object().unwrap(), true).unwrap();
    let land = Landing { item_id: ITEM.into(), version: 1, visibility: "unlisted".into(), item_kind: None, base_of: None, prev_public_version: 0, prev_public_at: None, prev_hidden: false, prev_license: None };
    let x = PoolCtx::new(&w.c, 5 * DAY_MS);
    let rw = rewrite(&x, &w.s, &w.core, "me", &land, true, &m, &payload, false).unwrap();
    let h = crate::pool::publishhash::prepare(&x, &w.s, &w.core, &w.fp, &land, true, &m, &rw).unwrap();
    let f = gather(&x, &w.s, &w.fp, "me", None, true, false, &m, &rw, &h, None).unwrap();
    let e = collect(&x, &w.fp, "me", &m, &rw, &h, &f, &Units::default()).unwrap();
    (m, rw, h, f, e)
}
/// A public item by Bea, visible to others, with one question whose near hash is `near`.
fn public_source(w: &W, near: &str) {
    w.c.execute("INSERT INTO items (id, author_id, kind, visibility, current_publish_id, public_publish_id, public_version, license, title, created_at, updated_at) VALUES (?1, 'b', 'bank', 'public', 'pb', 'pb', 1, 'CC-BY-SA-4.0', 'Orchard bank', 1, 1)", [SRC]).unwrap();
    w.c.execute(
        "INSERT INTO item_publishes (publish_id, item_id, author_id, version, public_version, status, schema_version, hide_key, license, attested_at, attest_text_rev,
            acks_json, request_sha256, stage_token, full_key, full_bytes, title, subject, course, grade, kinds_mask, question_count, has_images, created_at)
         VALUES ('pb', ?1, 'b', 1, 1, 'current', 2, 0, 'CC-BY-SA-4.0', 1, 'r', '{}', 'x', 'pb', 'k', 10, '', '', '', '', 0, 1, 1, 1)",
        [SRC],
    )
    .unwrap();
    let required: Vec<String> = {
        let mut st = w.c.prepare("SELECT name FROM pragma_table_info('item_questions') WHERE \"notnull\" = 1 AND dflt_value IS NULL").unwrap();
        st.query_map([], |r| r.get(0)).unwrap().map(|x| x.unwrap()).collect()
    };
    let vals: Vec<String> = required.iter().map(|n| if n == "pos" { "0".to_string() } else { "''".to_string() }).collect();
    w.c.execute(&format!("INSERT INTO item_questions ({}) VALUES ({})", required.join(", "), vals.join(", ")), []).unwrap();
    w.c.execute(
        "UPDATE item_questions SET publish_id = 'pb', qid = 'sq', match_hash = 'other', seq = 7, near_full = ?1, near_stripped = ?1, near_kf = ?1, chain_json = '[]'
          WHERE rowid = last_insert_rowid()",
        [near],
    )
    .unwrap();
}
fn blank(n: usize) -> Derived {
    Derived {
        entries: vec![Entry::default(); n],
        marks: vec![Marks::default(); n],
        binders: vec![vec![]; n],
        per_question: vec![vec!["CC-BY-4.0"]; n],
        true_bound: vec![vec![]; n],
        allowed_json: vec![String::new(); n],
        ..Default::default()
    }
}
fn code(r: Res<()>) -> (u16, String, Value) {
    match r {
        Err(Fail::Pool(e)) => (e.status, e.code.to_string(), json!(e.extra)),
        other => panic!("expected a pool error, got {other:?}"),
    }
}

#[test]
fn a_question_equal_to_an_earlier_public_one_is_credited_as_an_earlier_publication() {
    let w = world();
    let (m, rw, h, f, e) = pipeline(&w, &[]);
    public_source(&w, &h.near_full[0]);
    let x = PoolCtx::new(&w.c, 5 * DAY_MS);
    let mut d = blank(1);
    earlier_identical(&x, "me", &m, &rw, &h, &f, &e, &mut d).unwrap();
    let l = &d.entries[0].chain[0];
    assert_eq!((l.item_id.as_str(), l.author_name.as_str(), l.earlier_match, l.modified), (SRC, "Bea", Some(true), false));
    assert!(d.marks[0].earlier && !d.marks[0].claimed);
    assert_eq!(d.soft_only, vec![0], "credited, flagged, never a refusal");
}

#[test]
fn an_independence_attestation_turns_the_credit_into_a_possible_source() {
    let w = world();
    let (m, rw, h, f, e) = pipeline(&w, &[SRC]);
    public_source(&w, &h.near_full[0]);
    let x = PoolCtx::new(&w.c, 5 * DAY_MS);
    let mut d = blank(1);
    earlier_identical(&x, "me", &m, &rw, &h, &f, &e, &mut d).unwrap();
    assert_eq!(d.entries[0].chain[0].possible_source, Some(true));
    assert!(d.marks[0].claimed && !d.marks[0].earlier);
}

#[test]
fn a_question_that_already_credits_someone_or_has_no_earlier_twin_is_left_alone() {
    let w = world();
    let (m, rw, h, f, e) = pipeline(&w, &[]);
    let x = PoolCtx::new(&w.c, 5 * DAY_MS);
    let mut d = blank(1);
    earlier_identical(&x, "me", &m, &rw, &h, &f, &e, &mut d).unwrap();
    assert!(d.entries[0].chain.is_empty() && d.soft_only.is_empty(), "no twin");
    public_source(&w, &h.near_full[0]);
    d.entries[0].chain = vec![ChainLink { item_id: "x".into(), ..Default::default() }];
    earlier_identical(&x, "me", &m, &rw, &h, &f, &e, &mut d).unwrap();
    assert_eq!(d.entries[0].chain.len(), 1);
    assert_eq!(d.entries[0].chain[0].item_id, "x");
}

#[test]
fn nothing_incompatible_is_no_refusal() {
    let w = world();
    let (m, ..) = pipeline(&w, &[]);
    let x = PoolCtx::new(&w.c, 5 * DAY_MS);
    assert!(refuse(&x, "me", &m, &blank(1)).is_ok());
}

#[test]
fn the_general_refusal_lists_what_each_question_allows_and_names_a_visible_binder() {
    let w = world();
    let (m, ..) = pipeline(&w, &[]);
    public_source(&w, "n");
    let x = PoolCtx::new(&w.c, 5 * DAY_MS);
    let mut d = blank(2);
    d.incompatible = vec![1];
    d.per_question[1] = vec!["CC-BY-NC-4.0"];
    d.binders[1] = vec![SRC.into()];
    let (status, code_, extra) = code(refuse(&x, "me", &m, &d));
    assert_eq!((status, code_.as_str()), (422, "license_incompatible"));
    assert_eq!(extra["positions"], json!([1]));
    assert_eq!(extra["allowed"], json!([]), "the bank allows only what every question allows");
    let pq = &extra["per_question"][0];
    assert_eq!(pq["pos"], 1);
    assert_eq!(pq["allowed"], json!(["CC-BY-NC-4.0"]));
    assert_eq!((pq["bound_by"][0]["item_id"].as_str(), pq["bound_by"][0]["title"].as_str(), pq["bound_by"][0]["author_name"].as_str()), (Some(SRC), Some("Orchard bank"), Some("Bea")));
}

#[test]
fn a_binder_nobody_can_see_is_named_neutrally() {
    let w = world();
    let (m, ..) = pipeline(&w, &[]);
    public_source(&w, "n");
    w.c.execute("UPDATE items SET removed_at = 5, removed_reason = 'admin' WHERE id = ?1", [SRC]).unwrap();
    let x = PoolCtx::new(&w.c, 5 * DAY_MS);
    let mut d = blank(1);
    d.incompatible = vec![0];
    d.binders[0] = vec![SRC.into()];
    let (_, _, extra) = code(refuse(&x, "me", &m, &d));
    let b = &extra["per_question"][0]["bound_by"][0];
    assert!(b["item_id"].is_null() && b["note"].is_string());
}

#[test]
fn an_upstream_restriction_is_named_with_the_relabelling_item_as_via() {
    let w = world();
    let (m, ..) = pipeline(&w, &[]);
    public_source(&w, "n");
    let x = PoolCtx::new(&w.c, 5 * DAY_MS);
    let mut d = blank(1);
    d.incompatible = vec![0];
    d.binders[0] = vec![SRC.into()];
    d.true_bound[0] = vec![(SRC.into(), ChainLink { item_id: SRC.into(), license: "CC-BY-NC-4.0".into(), author_name: "Bea".into(), ..Default::default() })];
    let (_, _, extra) = code(refuse(&x, "me", &m, &d));
    let b = &extra["per_question"][0]["bound_by"][0];
    assert_eq!((b["license"].as_str(), b["via"].as_str()), (Some("CC-BY-NC-4.0"), Some(SRC)));
}

#[test]
fn the_hidden_material_refusal_comes_only_when_nothing_else_is_incompatible() {
    let w = world();
    let (m, ..) = pipeline(&w, &[]);
    public_source(&w, "n");
    let x = PoolCtx::new(&w.c, 5 * DAY_MS);
    let mut d = blank(1);
    d.hidden_refused = vec![0];
    d.hidden_from = vec![(SRC.into(), vec!["CC-BY-NC-4.0".into()])];
    let (status, code_, extra) = code(refuse(&x, "me", &m, &d));
    assert_eq!((status, code_.as_str()), (422, "license_incompatible"));
    assert_eq!((extra["hidden_material"].clone(), extra["positions"].clone()), (json!(true), json!([0])));
    assert_eq!(extra["sources"][0]["title"], "Orchard bank");
    assert_eq!(extra["sources"][0]["licenses"], json!(["CC-BY-NC-4.0"]));
    assert!(extra["message"].as_str().unwrap().contains("Orchard bank"));
    // With a general incompatibility too, the general refusal wins.
    d.incompatible = vec![0];
    let (_, _, extra) = code(refuse(&x, "me", &m, &d));
    assert!(extra.get("hidden_material").is_none() && extra.get("per_question").is_some());
}

#[test]
fn a_source_relicensed_after_it_was_received_is_explained() {
    let w = world();
    let (m, ..) = pipeline(&w, &[]);
    public_source(&w, "n");
    w.c.execute("INSERT INTO served (reader_id, item_id, publish_id, kind, license, licenses_json, first_at, at) VALUES ('me', ?1, 'pb', 'import', 'CC-BY-4.0', '[\"CC-BY-4.0\"]', 1, 5)", [SRC]).unwrap();
    let x = PoolCtx::new(&w.c, 5 * DAY_MS);
    let mut d = blank(1);
    d.incompatible = vec![0];
    d.binders[0] = vec![SRC.into()];
    let (_, _, extra) = code(refuse(&x, "me", &m, &d));
    assert_eq!(extra["relicensed"][0]["now"], "CC-BY-SA-4.0");
    assert_eq!(extra["relicensed"][0]["earlier"], json!(["CC-BY-4.0"]));
    assert!(extra["message"].as_str().unwrap().contains("CC-BY-SA-4.0"));
}

#[test]
fn suspects_flag_near_stem_positions_and_lineage_walks_become_soft() {
    let w = world();
    let (_, _, h, f, mut e) = pipeline(&w, &[]);
    let x = PoolCtx::new(&w.c, 5 * DAY_MS);
    let mut d = blank(1);
    assert!(suspects(&x, "me", &h, &f, &e, &mut d).unwrap().is_empty());
    d.fuzzy_pos.insert(0);
    assert_eq!(suspects(&x, "me", &h, &f, &e, &mut d).unwrap(), vec![0]);
    d.fuzzy_pos.clear();
    e.lineage_soft.insert(0);
    assert_eq!(suspects(&x, "me", &h, &f, &e, &mut d).unwrap(), vec![0]);
    assert_eq!(d.soft_only, vec![0]);
}
