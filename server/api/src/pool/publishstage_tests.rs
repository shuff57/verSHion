//! Tests for `publishstage.rs`: the tag cap, the record cap (new records only, held-item exemption), the three quota refusals, and the sequence numbers.
use super::*;
use crate::blobs::BlobStore;
use crate::db;
use crate::pool::core::Core;
use crate::pool::fingerprint::Fingerprinter;
use crate::pool::publishfacts::{PrevRow, gather};
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
    c.execute("INSERT INTO users (id, email, display_name, trust, created_at) VALUES ('me', 'me@x.org', 'Me', 'teacher', 1)", []).unwrap();
    let s = BlobStore::new(dir.path().join("blobs")).unwrap();
    W { c, s, core: Core::new(Some(KEY.into())), fp: Fingerprinter::from_root_hex(ROOT).unwrap(), _d: dir }
}
fn mc(stem: &str) -> Value {
    let words = ["apple", "river", "stone", "cloud", "ember", "maple", "orbit", "quartz"];
    let answers: Vec<Value> = words.iter().enumerate().map(|(i, t)| json!({ "text": t, "correct": i == 3, "locked": false })).collect();
    json!({ "id": "q", "stem": stem, "kind": { "Mc": { "answers": answers } }, "points": null, "difficulty": null, "topics": [], "image": null, "explanation": "because", "figure": null, "created": 1, "modified": 1 })
}
fn pipeline(w: &W) -> (PublishMeta, Rewritten, Hashed, Facts) {
    let payload = json!({ "schema_version": 1, "bank": { "id": "local", "name": "B", "questions": [mc("What is the capital of France?")], "created": 1, "modified": 1 } }).to_string().into_bytes();
    let m = json!({ "publish_id": "AAAAAAAAAAAAAAAAAAAAAA", "hide_key": false, "license": "CC-BY-4.0", "visibility": "unlisted", "attestation": { "accepted": true, "text_rev": "2026-10-19" }, "meta": { "title": "Q", "subject": "S" } });
    let m = read_meta(m.as_object().unwrap(), true).unwrap();
    let land = Landing { item_id: ITEM.into(), version: 1, visibility: "unlisted".into(), item_kind: None, base_of: None, prev_public_version: 0, prev_public_at: None, prev_hidden: false, prev_license: None };
    let x = PoolCtx::new(&w.c, 5 * DAY_MS);
    let rw = rewrite(&x, &w.s, &w.core, "me", &land, true, &m, &payload, false).unwrap();
    let h = crate::pool::publishhash::prepare(&x, &w.s, &w.core, &w.fp, &land, true, &m, &rw).unwrap();
    let f = gather(&x, &w.s, &w.fp, "me", None, true, false, &m, &rw, &h, None).unwrap();
    (m, rw, h, f)
}
fn err(r: Res<()>) -> (u16, String, Value) {
    match r {
        Err(super::super::ctx::Fail::Pool(e)) => (e.status, e.code.to_string(), json!(e.extra)),
        other => panic!("expected a pool error, got {other:?}"),
    }
}
fn set_limit(w: &W, metric: &str, v: i64) {
    w.c.execute("INSERT OR REPLACE INTO pool_limits (metric, user_id, value, updated_at, updated_by) VALUES (?1, 'me', ?2, 1, 'me')", params![metric, v]).unwrap();
}

#[test]
fn the_tag_cap_is_an_error_not_a_silent_drop() {
    assert!(check_tags(MAX_TAGS).is_ok());
    let (s, c, e) = err(check_tags(MAX_TAGS + 1));
    assert_eq!((s, c.as_str(), e["limit"].as_i64()), (422, "too_many_tags", Some(200)));
}

#[test]
fn record_keys_are_the_shown_near_and_keyfree_hashes_once_each() {
    let w = world();
    let (m, _, h, _) = pipeline(&w);
    let keys = record_keys(&m, &h, &Units::default());
    let kinds: Vec<&str> = keys.iter().map(|k| k[0].as_str()).collect();
    assert_eq!(kinds, vec!["nshown", "kf", "shown"]);
    assert!(keys.iter().all(|k| k[2] == h.match_hashes[0]));
    let mut hidden = m.clone();
    hidden.hide_key = true;
    assert!(!record_keys(&hidden, &h, &Units::default()).iter().any(|k| k[0] == "shown"), "a hidden key records no shown content");
}

#[test]
fn only_new_records_count_against_the_cap() {
    let w = world();
    let (m, _, h, _) = pipeline(&w);
    let x = PoolCtx::new(&w.c, 5 * DAY_MS);
    let keys = record_keys(&m, &h, &Units::default());
    set_limit(&w, "author_records", 2);
    let (s, c, e) = err(check_records(&x, "me", ITEM, true, &keys));
    assert_eq!((s, c.as_str(), e["used"].as_i64(), e["limit"].as_i64()), (413, "author_records_exceeded", Some(0), Some(2)));
    // Once recorded, the same publish adds nothing new and passes.
    for k in &keys {
        w.c.execute("INSERT INTO author_contents (author_id, match_hash, kind, hash, first_seq) VALUES ('me', ?1, ?2, ?3, 1)", params![k[2], k[0], k[1]]).unwrap();
    }
    assert!(check_records(&x, "me", ITEM, true, &keys).is_ok());
}

#[test]
fn a_fix_of_a_held_item_is_exempt_up_to_the_allowance() {
    let w = world();
    let (m, _, h, _) = pipeline(&w);
    let x = PoolCtx::new(&w.c, 5 * DAY_MS);
    let keys = record_keys(&m, &h, &Units::default());
    set_limit(&w, "author_records", 1);
    assert!(check_records(&x, "me", ITEM, false, &keys).is_err(), "not held");
    w.c.execute("INSERT INTO items (id, author_id, kind, visibility, license, created_at, updated_at) VALUES (?1, 'me', 'bank', 'unlisted', 'CC-BY-4.0', 1, 1)", [ITEM]).unwrap();
    w.c.execute("INSERT INTO item_holds (item_id, reason, at, source) VALUES (?1, 'admin', 1, 'manual')", [ITEM]).unwrap();
    assert!(check_records(&x, "me", ITEM, false, &keys).is_ok(), "held fix");
    assert!(check_records(&x, "me", ITEM, true, &keys).is_err(), "a new item is never a held fix");
}

#[test]
fn the_quotas_refuse_in_order_with_what_was_used_and_needed() {
    let w = world();
    let x = PoolCtx::new(&w.c, 5 * DAY_MS);
    let l = limits(&x, "me").unwrap();
    assert!(check_quotas(&x, "me", ITEM, 10, 1, &l).is_ok());
    let tight = Limits { quota: 5, ..l };
    let (s, c, e) = err(check_quotas(&x, "me", ITEM, 10, 1, &tight));
    assert_eq!((s, c.as_str(), e["needed"].as_i64(), e["limit"].as_i64()), (413, "quota_exceeded", Some(10), Some(5)));
    let (s, c, e) = err(check_quotas(&x, "me", ITEM, 10, 3, &Limits { shared_cap: 2, ..l }));
    assert_eq!((s, c.as_str(), e["needed"].as_i64(), e["limit"].as_i64()), (422, "shared_questions_limit", Some(3), Some(2)));
    let (s, c, _) = err(check_quotas(&x, "me", ITEM, 10, 1, &Limits { removed_cap: -1, ..l }));
    assert_eq!((s, c.as_str()), (413, "removed_storage_exceeded"));
}

#[test]
fn the_publish_counter_is_strictly_increasing_and_a_changed_question_gets_the_new_one() {
    let w = world();
    let (_, rw, h, mut f) = pipeline(&w);
    let x = PoolCtx::new(&w.c, 5 * DAY_MS);
    let a = sequences(&x, &rw, &h, &f, &Units::default(), None).unwrap();
    let b = sequences(&x, &rw, &h, &f, &Units::default(), None).unwrap();
    assert_eq!(b.new_seq, a.new_seq + 1);
    assert_eq!(b.q_seq, vec![b.new_seq]);
    // Unchanged content keeps the previous seq; a changed one takes the new.
    let mut prev = PrevRow { match_hash: h.match_hashes[0].clone(), seq: 3, ..Default::default() };
    f.prev.insert(rw.qids[0].clone(), prev.clone());
    assert_eq!(sequences(&x, &rw, &h, &f, &Units::default(), None).unwrap().q_seq, vec![3]);
    prev.match_hash = "other".into();
    f.prev.insert(rw.qids[0].clone(), prev);
    let c = sequences(&x, &rw, &h, &f, &Units::default(), None).unwrap();
    assert_eq!(c.q_seq, vec![c.new_seq]);
}

#[test]
fn a_text_unit_carries_its_own_seq_only_while_its_key_was_in_the_previous_publish() {
    let w = world();
    let (_, rw, h, mut f) = pipeline(&w);
    let x = PoolCtx::new(&w.c, 5 * DAY_MS);
    f.served_text = vec![vec!["t1".into(), "t2".into()]];
    f.hidden_text = vec![vec![]];
    f.prev.insert(rw.qids[0].clone(), PrevRow { match_hash: "other".into(), unit_seqs: Some("{\"t1\":2,\"zz\":9}".into()), ..Default::default() });
    let s = sequences(&x, &rw, &h, &f, &Units::default(), None).unwrap();
    assert_eq!(s.unit_seqs[0].get("t1"), Some(&2));
    assert_eq!(s.unit_seqs[0].get("t2"), Some(&s.new_seq));
    assert!(!s.unit_seqs[0].contains_key("zz"));
}

#[test]
fn an_image_keeps_the_seq_of_the_first_publish_that_served_it() {
    let s = Seqs { new_seq: 10, blob_carry: [("a".to_string(), 4)].into(), ..Default::default() };
    let served: HashSet<&str> = ["a", "b"].into();
    assert_eq!((s.blob_seq(&served, "a"), s.blob_seq(&served, "b"), s.blob_seq(&served, "c")), (4, 10, 10));
}

#[test]
fn blob_carry_reads_the_served_form_of_the_base_publish() {
    let w = world();
    let (_, rw, h, f) = pipeline(&w);
    let x = PoolCtx::new(&w.c, 5 * DAY_MS);
    w.c.execute("INSERT INTO items (id, author_id, kind, visibility, license, created_at, updated_at) VALUES (?1, 'me', 'bank', 'unlisted', 'CC-BY-4.0', 1, 1)", [ITEM]).unwrap();
    w.c.execute(
        "INSERT INTO item_publishes (publish_id, item_id, author_id, version, public_version, status, schema_version, hide_key, license, attested_at, attest_text_rev,
            acks_json, request_sha256, stage_token, full_key, full_bytes, title, subject, course, grade, kinds_mask, question_count, has_images, created_at)
         VALUES ('base', ?1, 'me', 1, 1, 'current', 2, 0, 'CC-BY-4.0', 1, 'r', '{}', 'x', 'base', 'k', 10, '', '', '', '', 0, 1, 1, 1)",
        [ITEM],
    )
    .unwrap();
    let sha = "a".repeat(64);
    w.c.execute("INSERT INTO item_blobs (publish_id, form, sha256, seq) VALUES ('base', 'full', ?1, 6)", [&sha]).unwrap();
    w.c.execute("INSERT INTO item_blobs (publish_id, form, sha256, seq) VALUES ('base', 'stripped', ?1, 2)", ["b".repeat(64)]).unwrap();
    let s = sequences(&x, &rw, &h, &f, &Units::default(), Some("base")).unwrap();
    assert_eq!(s.blob_carry.get(&sha), Some(&6));
    assert_eq!(s.blob_carry.len(), 1, "only the form students are served");
}
