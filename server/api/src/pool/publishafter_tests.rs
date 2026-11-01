//! Tests for `publishafter.rs`: retention keeps current / previous / served publishes and deletes the rest with their bodies, the held-publish charge counts
//! only the questions a block would hold, and the dry run says what students would see.
use super::*;
use crate::db;
use crate::pool::core::Core;
use crate::pool::fingerprint::Fingerprinter;
use crate::pool::publishderive::derive;
use crate::pool::publishevid::collect;
use crate::pool::publishfacts::gather;
use crate::pool::publishinsert::plan;
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
const NOW: i64 = 5 * DAY_MS;

struct W {
    c: Connection,
    d: tempfile::TempDir,
    s: BlobStore,
}
fn world() -> W {
    let dir = tempfile::tempdir().unwrap();
    let mut c = db::open(&dir.path().join("t.db")).unwrap();
    db::migrate(&mut c, &Path::new(env!("CARGO_MANIFEST_DIR")).join("../worker/migrations")).unwrap();
    c.execute("INSERT INTO users (id, email, display_name, trust, created_at) VALUES ('me', 'me@x.org', 'Me', 'teacher', 1)", []).unwrap();
    let s = BlobStore::new(dir.path().join("blobs")).unwrap();
    W { c, s, d: dir }
}
fn publish(w: &W, id: &str, status: &str) {
    w.c.execute(
        "INSERT INTO item_publishes (publish_id, item_id, author_id, version, public_version, status, schema_version, hide_key, license, attested_at, attest_text_rev,
            acks_json, request_sha256, stage_token, full_key, stripped_key, full_bytes, title, subject, course, grade, kinds_mask, question_count, has_images, created_at)
         VALUES (?1, ?2, 'me', 1, 1, ?3, 2, 0, 'CC-BY-4.0', 1, 'r', '{}', 'x', ?1, ?4, ?5, 10, '', '', '', '', 0, 1, 0, 1)",
        params![id, ITEM, status, format!("items/{ITEM}/{id}.json"), format!("items/{ITEM}/{id}-s.json")],
    )
    .unwrap();
    w.s.put(&format!("items/{ITEM}/{id}.json"), b"{}").unwrap();
    w.s.put(&format!("items/{ITEM}/{id}-s.json"), b"{}").unwrap();
}
fn item(w: &W, current: &str, prev: &str, public: &str) {
    w.c.execute(
        "INSERT INTO items (id, author_id, kind, visibility, current_publish_id, prev_publish_id, public_publish_id, license, created_at, updated_at) VALUES (?1, 'me', 'bank', 'unlisted', ?2, ?3, ?4, 'CC-BY-4.0', 1, 1)",
        params![ITEM, current, prev, public],
    )
    .unwrap();
}
fn count(w: &W, sql: &str) -> i64 {
    w.c.query_row(sql, [], |r| r.get(0)).unwrap()
}

#[test]
fn retention_keeps_current_previous_and_the_served_publish_and_deletes_the_rest_with_their_bodies() {
    let w = world();
    for (id, st) in [("p1", "previous"), ("p2", "previous"), ("p3", "previous"), ("p4", "current")] {
        publish(&w, id, st);
    }
    item(&w, "p4", "p3", "p2");
    retain(&PoolCtx::new(&w.c, NOW), &w.s, ITEM).unwrap();
    let left: Vec<String> = {
        let mut st = w.c.prepare("SELECT publish_id FROM item_publishes ORDER BY publish_id").unwrap();
        st.query_map([], |r| r.get(0)).unwrap().map(|x| x.unwrap()).collect()
    };
    assert_eq!(left, vec!["p2", "p3", "p4"]);
    assert!(w.s.get(&format!("items/{ITEM}/p1.json")).unwrap().is_none() && w.s.get(&format!("items/{ITEM}/p1-s.json")).unwrap().is_none());
    assert!(w.s.get(&format!("items/{ITEM}/p2.json")).unwrap().is_some());
    // Idempotent.
    retain(&PoolCtx::new(&w.c, NOW), &w.s, ITEM).unwrap();
    assert_eq!(count(&w, "SELECT COUNT(*) FROM item_publishes"), 3);
    let _ = &w.d;
}

fn question(w: &W, publish_id: &str, mh: &str) {
    let required: Vec<String> = {
        let mut st = w.c.prepare("SELECT name FROM pragma_table_info('item_questions') WHERE \"notnull\" = 1 AND dflt_value IS NULL").unwrap();
        st.query_map([], |r| r.get(0)).unwrap().map(|x| x.unwrap()).collect()
    };
    let vals: Vec<String> = required.iter().map(|n| if n == "pos" { "0".to_string() } else { "''".to_string() }).collect();
    w.c.execute(&format!("INSERT INTO item_questions ({}) VALUES ({})", required.join(", "), vals.join(", ")), []).unwrap();
    w.c.execute("UPDATE item_questions SET publish_id = ?1, qid = 'q', match_hash = ?2, seq = 1 WHERE rowid = last_insert_rowid()", params![publish_id, mh]).unwrap();
}

#[test]
fn the_charge_counts_only_questions_a_block_would_hold_plus_near_rewordings() {
    let w = world();
    publish(&w, "p1", "current");
    item(&w, "p1", "", "p1");
    question(&w, "p1", "mh-blocked");
    let x = PoolCtx::new(&w.c, NOW);
    assert_eq!(held_charge(&x, "me", ITEM, "p1", 0).unwrap(), 0);
    assert_eq!(held_charge(&x, "me", ITEM, "p1", 2).unwrap(), 2);
    w.c.execute("INSERT INTO privacy_blocks (kind, hash, item_id, at) VALUES ('match', 'mh-blocked', 'other', 1)", []).unwrap();
    assert_eq!(held_charge(&x, "me", ITEM, "p1", 2).unwrap(), 3);
    // An admin's release of this item covers the key it used (within 90 days, and not for a block placed after it).
    w.c.execute("INSERT INTO hold_releases (item_id, source, keys_json, at, by) VALUES (?1, 'privacy', '[\"mh-blocked\"]', ?2, 'me')", params![ITEM, NOW - DAY_MS]).unwrap();
    assert_eq!(held_charge(&x, "me", ITEM, "p1", 0).unwrap(), 0);
}

#[test]
fn a_charge_is_counted_for_the_day_and_past_the_cap_the_item_is_held() {
    let w = world();
    publish(&w, "p1", "current");
    item(&w, "p1", "", "p1");
    question(&w, "p1", "mh-blocked");
    w.c.execute("INSERT INTO privacy_blocks (kind, hash, item_id, at) VALUES ('match', 'mh-blocked', 'other', 1)", []).unwrap();
    let x = PoolCtx::new(&w.c, NOW);
    after_commit(&x, &w.s, "me", ITEM, "p1", 0, false).unwrap();
    assert_eq!(count(&w, "SELECT count FROM usage_daily WHERE user_id = 'me' AND metric = 'held_publishes'"), 1);
    assert_eq!(count(&w, "SELECT COUNT(*) FROM item_holds WHERE source = 'probe_cap'"), 0);
    after_commit(&x, &w.s, "me", ITEM, "p1", 0, true).unwrap();
    assert_eq!(count(&w, "SELECT count FROM usage_daily WHERE user_id = 'me' AND metric = 'held_publishes'"), 2);
    assert_eq!(count(&w, "SELECT COUNT(*) FROM item_holds WHERE source = 'probe_cap' AND reason = 'admin'"), 1);
}

#[test]
fn nothing_charged_means_nothing_written() {
    let w = world();
    publish(&w, "p1", "current");
    item(&w, "p1", "", "p1");
    after_commit(&PoolCtx::new(&w.c, NOW), &w.s, "me", ITEM, "p1", 0, true).unwrap();
    assert_eq!(count(&w, "SELECT COUNT(*) FROM usage_daily"), 0);
    assert_eq!(count(&w, "SELECT COUNT(*) FROM item_holds"), 0);
}

fn mc(stem: &str) -> Value {
    let words = ["apple", "river", "stone", "cloud", "ember", "maple", "orbit", "quartz"];
    let answers: Vec<Value> = words.iter().enumerate().map(|(i, t)| json!({ "text": t, "correct": i == 3, "locked": false })).collect();
    json!({ "id": "q", "stem": stem, "kind": { "Mc": { "answers": answers } }, "points": null, "difficulty": null, "topics": [], "image": null, "explanation": "because", "figure": null, "created": 1, "modified": 1 })
}
fn dry(w: &W, hide_key: bool) -> Value {
    let core = Core::new(Some(KEY.into()));
    let fp = Fingerprinter::from_root_hex(ROOT).unwrap();
    let payload = json!({ "schema_version": 1, "bank": { "id": "local", "name": "B", "questions": [mc("What is the capital of France?")], "created": 1, "modified": 1 } }).to_string().into_bytes();
    let m = json!({ "publish_id": "AAAAAAAAAAAAAAAAAAAAAA", "hide_key": hide_key, "license": "CC-BY-4.0", "visibility": "unlisted", "attestation": { "accepted": true, "text_rev": "2026-10-19" }, "meta": { "title": "Q", "subject": "S" } });
    let m = read_meta(m.as_object().unwrap(), true).unwrap();
    let land = Landing { item_id: ITEM.into(), version: 1, visibility: "unlisted".into(), item_kind: None, base_of: None, prev_public_version: 0, prev_public_at: None, prev_hidden: false, prev_license: None };
    let x = PoolCtx::new(&w.c, NOW);
    let rw = rewrite(&x, &w.s, &core, "me", &land, true, &m, &payload, true).unwrap();
    let h = crate::pool::publishhash::prepare(&x, &w.s, &core, &fp, &land, true, &m, &rw).unwrap();
    let f = gather(&x, &w.s, &fp, "me", None, true, true, &m, &rw, &h, None).unwrap();
    let e = collect(&x, &fp, "me", &m, &rw, &h, &f, &Units::default()).unwrap();
    let d = derive(&m, &rw, &h, &f, &e, &Units::default(), None, false);
    let _ = plan(&m, &rw, &h, true);
    dry_response(&m, &h, &d, rw.n)
}

#[test]
fn a_dry_run_with_a_shown_key_says_the_key_is_shown() {
    let w = world();
    let v = dry(&w, false);
    assert_eq!((v["ok"].clone(), v["dry_run"].clone(), v["hide_key"].clone()), (json!(true), json!(true), json!(false)));
    assert_eq!(v["student_view"], json!({ "questions": 1, "key_shown": true }));
    assert_eq!(v["license_check"]["status"], "ok");
    assert_eq!(v["chains"], json!([[]]));
    assert_eq!(v["label_findings"], json!([]));
}

#[test]
fn a_dry_run_with_a_hidden_key_returns_the_stripped_payload() {
    let w = world();
    let v = dry(&w, true);
    assert_eq!(v["hide_key"], true);
    let view = &v["student_view"];
    assert!(view.get("bank").is_some() || view.get("questions").is_some());
    assert!(view.get("key_shown").is_none());
    assert!(!view.to_string().contains("\"correct\":true"), "the key is gone");
}
