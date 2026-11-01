//! Tests for `publishevid.rs`: the evidence of a fresh bank, a source this account was served, the account's own items and void items, all through the real
//! pipeline (rewrite, prepare, gather) so the subjects are the ones a publish builds.
use super::*;
use crate::blobs::BlobStore;
use crate::db;
use crate::pool::core::Core;
use crate::pool::publishfacts::gather;
use crate::pool::publishmeta::read_meta;
use crate::pool::publishrewrite::rewrite;
use crate::pool::publishstart::Landing;
use crate::pool::sqlfrag::DAY_MS;
use rusqlite::Connection;
use serde_json::{Value, json};
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
fn go(w: &W, qs: Vec<Value>, declared: Option<&str>) -> (Hashed, Facts, Evid) {
    let mut qs = qs;
    if let Some(item) = declared {
        qs[0]["origin"] = json!({ "item_id": item, "qid": "22222222222222222222222222222222", "version": 1 });
    }
    let payload = json!({ "schema_version": 1, "bank": { "id": "local", "name": "B", "questions": qs, "created": 1, "modified": 1 } }).to_string().into_bytes();
    let m = json!({ "publish_id": "AAAAAAAAAAAAAAAAAAAAAA", "hide_key": false, "license": "CC-BY-4.0", "visibility": "unlisted", "attestation": { "accepted": true, "text_rev": "2026-10-19" }, "meta": { "title": "Q", "subject": "S" } });
    let m = read_meta(m.as_object().unwrap(), true).unwrap();
    let land = Landing { item_id: ITEM.into(), version: 1, visibility: "unlisted".into(), item_kind: None, base_of: None, prev_public_version: 0, prev_public_at: None, prev_hidden: false, prev_license: None };
    let x = PoolCtx::new(&w.c, 5 * DAY_MS);
    let rw = rewrite(&x, &w.s, &w.core, "me", &land, true, &m, &payload, false).unwrap();
    let h = crate::pool::publishhash::prepare(&x, &w.s, &w.core, &w.fp, &land, true, &m, &rw).unwrap();
    let f = gather(&x, &w.s, &w.fp, "me", None, true, false, &m, &rw, &h, None).unwrap();
    let u = crate::pool::publishunits::Units::default();
    let e = collect(&x, &w.fp, "me", &m, &rw, &h, &f, &u).unwrap();
    (h, f, e)
}
/// A public item by Bea with a question of `mh`, served to `me`.
fn source(w: &W, mh: &str) {
    w.c.execute("INSERT INTO items (id, author_id, kind, visibility, current_publish_id, license, created_at, updated_at) VALUES (?1, 'b', 'bank', 'public', 'pb', 'CC-BY-4.0', 1, 1)", [SRC]).unwrap();
    w.c.execute(
        "INSERT INTO item_publishes (publish_id, item_id, author_id, version, public_version, status, schema_version, hide_key, license, attested_at, attest_text_rev,
            acks_json, request_sha256, stage_token, full_key, full_bytes, title, subject, course, grade, kinds_mask, question_count, has_images, created_at)
         VALUES ('pb', ?1, 'b', 1, 1, 'current', 2, 0, 'CC-BY-4.0', 1, 'r', '{}', 'x', 'pb', 'k', 10, '', '', '', '', 0, 1, 1, 1)",
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
        "UPDATE item_questions SET publish_id = 'pb', qid = 'q', match_hash = ?1, seq = 7, full_content = 'FULL', stripped_content = 'STRIP', keyfree_content = 'KF',
            near_full = 'NEAR', near_stripped = 'NEARS', near_kf = 'NKF', chain_json = '[]' WHERE rowid = last_insert_rowid()",
        [mh],
    )
    .unwrap();
    w.c.execute("INSERT INTO served (reader_id, item_id, publish_id, kind, license, licenses_json, first_at, at) VALUES ('me', ?1, 'pb', 'import', 'CC-BY-4.0', '[\"CC-BY-4.0\"]', 1, 5)", [SRC]).unwrap();
}

#[test]
fn a_fresh_bank_has_no_evidence() {
    let w = world();
    let (_, _, e) = go(&w, vec![mc("What is the capital of France?")], None);
    assert!(e.ev.by_pos.is_empty() && e.ev.overflow.is_empty() && e.fuzzy_pos.is_empty() && e.lineage_soft.is_empty() && e.void_refs.is_empty() && e.own_items.is_empty());
}

#[test]
fn a_source_the_account_was_served_with_the_same_question_is_found() {
    let w = world();
    let (h, ..) = go(&w, vec![mc("What is the capital of France?")], None);
    source(&w, &h.match_hashes[0]);
    let (_, _, e) = go(&w, vec![mc("What is the capital of France?")], None);
    let hit = &e.ev.by_pos[&0][0];
    assert_eq!((hit.item_id.as_str(), hit.author_name.as_str(), hit.kind.as_str()), (SRC, "Bea", "import"));
    assert!(hit.by_match && !hit.same, "the source's stored contents differ from this one's real hashes");
    assert_eq!(e.ev.credits[&0].links[0].item_id, SRC);
}

#[test]
fn a_declared_origin_naming_the_accounts_own_item_is_recorded_as_own() {
    let w = world();
    w.c.execute("INSERT INTO items (id, author_id, kind, visibility, license, created_at, updated_at) VALUES (?1, 'me', 'bank', 'unlisted', 'CC-BY-4.0', 1, 1)", [SRC]).unwrap();
    let (_, _, e) = go(&w, vec![mc("What is the capital of France?")], Some(SRC));
    assert!(e.own_items.contains(SRC));
    assert!(e.drop.contains(SRC), "the account's own items are dropped from every chain");
}

#[test]
fn a_void_item_is_no_evidence_at_all() {
    let w = world();
    w.c.execute("INSERT INTO void_items (item_id, reason, at) VALUES (?1, 'takedown', 1)", [SRC]).unwrap();
    let (h, ..) = go(&w, vec![mc("What is the capital of France?")], None);
    source(&w, &h.match_hashes[0]);
    let (_, _, e) = go(&w, vec![mc("What is the capital of France?")], None);
    assert!(e.ev.by_pos.is_empty(), "a voided source is never evidence");
}

#[test]
fn first_publish_helpers_ignore_non_positive_seqs_and_take_the_earliest() {
    let mut f = Facts { lineage: vec![Some(9), None], ..Default::default() };
    f.own_first.insert(0, 12);
    let mut e = Evid::default();
    e.own_fuzzy.insert(0, 5);
    e.own_fuzzy.insert(1, 0);
    assert_eq!((e.fuzzy_first(&f, 0), e.first_no_lineage(&f, 0)), (Some(5), Some(5)));
    assert_eq!((e.fuzzy_first(&f, 1), e.first_no_lineage(&f, 1)), (None, None));
    e.own_fuzzy.remove(&0);
    assert_eq!((e.fuzzy_first(&f, 0), e.first_no_lineage(&f, 0)), (Some(9), Some(12)));
}
