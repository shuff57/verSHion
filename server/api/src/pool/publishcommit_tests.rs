//! Tests for `publishcommit.rs`: a staged publish committed through the real pipeline (item row, statuses, durable records, offer, audit), a lost race, a
//! privacy block placing its hold and flag, a carried hold, and the flags it is handed.
use super::*;
use crate::db;
use crate::pool::core::Core;
use crate::pool::fingerprint::Fingerprinter;
use crate::pool::publishderive::{Derived, derive};
use crate::pool::publishevid::collect;
use crate::pool::publishfacts::{Facts, gather};
use crate::pool::publishhash::Hashed;
use crate::pool::publishinsert::{StageIn, plan, stage};
use crate::pool::publishmeta::read_meta;
use crate::pool::publishrewrite::{Rewritten, rewrite};
use crate::pool::publishstage::{Seqs, limits, sequences};
use crate::pool::publishstart::Landing;
use crate::pool::publishunits::Units;
use crate::pool::sqlfrag::DAY_MS;
use rusqlite::Connection;
use std::path::Path;

const KEY: &str = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";
const ROOT: &str = "7f757fb737085dcaaad0727deb89a14c18141681dc56be0a215a592c715f8e3b";
const ITEM: &str = "0123456789abcdef0123456789abcdef";
const PID: &str = "AAAAAAAAAAAAAAAAAAAAAA";

struct W {
    c: Connection,
    _d: tempfile::TempDir,
    s: BlobStore,
}
struct Staged {
    m: PublishMeta,
    rw: Rewritten,
    h: Hashed,
    f: Facts,
    seqs: Seqs,
    plan: Plan,
    full_key: String,
    stripped_key: Option<String>,
    token: String,
}
fn world() -> W {
    let dir = tempfile::tempdir().unwrap();
    let mut c = db::open(&dir.path().join("t.db")).unwrap();
    db::migrate(&mut c, &Path::new(env!("CARGO_MANIFEST_DIR")).join("../worker/migrations")).unwrap();
    c.execute("INSERT INTO users (id, email, display_name, trust, created_at) VALUES ('me', 'me@x.org', 'Me', 'teacher', 1)", []).unwrap();
    let s = BlobStore::new(dir.path().join("blobs")).unwrap();
    W { c, s, _d: dir }
}
fn mc(stem: &str) -> Value {
    let words = ["apple", "river", "stone", "cloud", "ember", "maple", "orbit", "quartz"];
    let answers: Vec<Value> = words.iter().enumerate().map(|(i, t)| json!({ "text": t, "correct": i == 3, "locked": false })).collect();
    json!({ "id": "q", "stem": stem, "kind": { "Mc": { "answers": answers } }, "points": null, "difficulty": null, "topics": [], "image": null, "explanation": "because", "figure": null, "created": 1, "modified": 1 })
}
fn stage_one(w: &W, hide_key: bool) -> Staged {
    let core = Core::new(Some(KEY.into()));
    let fp = Fingerprinter::from_root_hex(ROOT).unwrap();
    let payload = json!({ "schema_version": 1, "bank": { "id": "local", "name": "Capitals", "questions": [mc("What is the capital of France?")], "created": 1, "modified": 1 } }).to_string().into_bytes();
    let m = json!({ "publish_id": PID, "hide_key": hide_key, "license": "CC-BY-4.0", "visibility": "unlisted", "attestation": { "accepted": true, "text_rev": "2026-10-19" }, "meta": { "title": "Capitals", "subject": "Geo" }, "attest": { "own_work": true, "terms_version": "t1" } });
    let m = read_meta(m.as_object().unwrap(), true).unwrap();
    let land = Landing { item_id: ITEM.into(), version: 1, visibility: "unlisted".into(), item_kind: None, base_of: None, prev_public_version: 0, prev_public_at: None, prev_hidden: false, prev_license: None };
    let x = PoolCtx::new(&w.c, 5 * DAY_MS);
    let rw = rewrite(&x, &w.s, &core, "me", &land, true, &m, &payload, false).unwrap();
    let h = crate::pool::publishhash::prepare(&x, &w.s, &core, &fp, &land, true, &m, &rw).unwrap();
    let f = gather(&x, &w.s, &fp, "me", None, true, false, &m, &rw, &h, None).unwrap();
    let e = collect(&x, &fp, "me", &m, &rw, &h, &f, &Units::default()).unwrap();
    let u = Units::default();
    let d: Derived = derive(&m, &rw, &h, &f, &e, &u, None, false);
    let seqs = sequences(&x, &rw, &h, &f, &u, None).unwrap();
    let (pl, charge) = plan(&m, &rw, &h, true);
    let l = limits(&x, "me").unwrap();
    let st = stage(
        &x,
        &w.s,
        &StageIn { me: "me", item_id: ITEM, version: 1, request_sha: "rq", m: &m, rw: &rw, h: &h, f: &f, d: &d, s: &seqs, plan: &pl, charge, limits: l, search_words: &h.search_words, served_shas: &pl.shas },
    )
    .unwrap();
    Staged { m, rw, h, f, seqs, plan: pl, full_key: st.full_key, stripped_key: st.stripped_key, token: st.token }
}
fn go(w: &W, s: &Staged, carried: Option<&Carried>, flags: &[Flag], folded: &[Flag], privacy_near: usize) -> Res<()> {
    let x = PoolCtx::new(&w.c, 5 * DAY_MS);
    let ckeys: Vec<String> = s.h.near_full.iter().map(|n| format!("q:{n}")).collect();
    commit(
        &x,
        &w.s,
        &CommitIn {
            me: "me",
            item_id: ITEM,
            is_new: true,
            kind: s.rw.kind,
            visibility: "unlisted",
            version: 1,
            m: &s.m,
            plan: &s.plan,
            subject: &s.rw.meta.subject,
            course: &s.rw.meta.course,
            grade: &s.rw.meta.grade,
            n: s.rw.n,
            public_version: 1,
            public_at: None,
            reused: false,
            token: &s.token,
            base_of: None,
            result_json: "{\"ok\":true}",
            ckeys: &ckeys,
            unit_record: &s.seqs.unit_record,
            stem_record: &s.f.stem_record,
            carried,
            flags,
            folded,
            circle_claimed: &[],
            circle_statement: None,
            privacy_near,
            full_key: &s.full_key,
            stripped_key: s.stripped_key.as_deref(),
        },
    )
}
fn count(w: &W, sql: &str) -> i64 {
    w.c.query_row(sql, [], |r| r.get(0)).unwrap()
}

#[test]
fn a_staged_publish_becomes_the_items_current_one_with_its_durable_records() {
    let w = world();
    let s = stage_one(&w, false);
    go(&w, &s, None, &[], &[], 0).unwrap();
    let (cur, ver, title, qc, hide): (String, i64, String, i64, i64) =
        w.c.query_row("SELECT current_publish_id, version, title, question_count, hide_key FROM items WHERE id = ?1", [ITEM], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?))).unwrap();
    assert_eq!((cur.as_str(), ver, title.as_str(), qc, hide), (PID, 1, "Capitals", 1, 0));
    let (status, result): (String, String) = w.c.query_row("SELECT status, result_json FROM item_publishes WHERE publish_id = ?1", [PID], |r| Ok((r.get(0)?, r.get(1)?))).unwrap();
    assert_eq!((status.as_str(), result.as_str()), ("current", "{\"ok\":true}"));
    assert_eq!(count(&w, "SELECT COUNT(*) FROM author_contents WHERE author_id = 'me' AND kind = 'nshown'"), 1);
    assert_eq!(count(&w, "SELECT COUNT(*) FROM author_contents WHERE author_id = 'me' AND kind = 'kf'"), 1);
    assert_eq!(count(&w, "SELECT COUNT(*) FROM author_contents WHERE author_id = 'me' AND kind = 'shown'"), 1, "a shown key records its full content");
    assert_eq!(count(&w, "SELECT COUNT(*) FROM author_firsts WHERE author_id = 'me'"), 1);
    assert_eq!(count(&w, "SELECT COUNT(*) FROM item_offer WHERE item_id = ?1".replace("?1", &format!("'{ITEM}'")).as_str()), 1);
    assert_eq!(count(&w, "SELECT COUNT(*) FROM attestations WHERE publish_id = 'AAAAAAAAAAAAAAAAAAAAAA' AND terms_version = 't1'"), 1);
    let (action, detail): (String, String) = w.c.query_row("SELECT action, detail FROM pool_audit WHERE item_id = ?1", [ITEM], |r| Ok((r.get(0)?, r.get(1)?))).unwrap();
    assert_eq!(action, "item.publish");
    assert_eq!(serde_json::from_str::<Value>(&detail).unwrap()["version"], 1);
}

#[test]
fn a_hidden_key_records_no_shown_content() {
    let w = world();
    let s = stage_one(&w, true);
    go(&w, &s, None, &[], &[], 0).unwrap();
    assert_eq!(count(&w, "SELECT COUNT(*) FROM author_contents WHERE kind = 'shown'"), 0);
    assert_eq!(count(&w, "SELECT COUNT(*) FROM author_contents WHERE kind = 'nshown'"), 1);
}

#[test]
fn a_lost_race_keeps_nothing_and_discards_the_staged_publish() {
    let w = world();
    let s = stage_one(&w, false);
    // Someone created the item first.
    w.c.execute("INSERT INTO items (id, author_id, kind, visibility, license, version, created_at, updated_at) VALUES (?1, 'me', 'bank', 'unlisted', 'CC-BY-4.0', 3, 1, 1)", [ITEM]).unwrap();
    match go(&w, &s, None, &[], &[], 0) {
        Err(Fail::Pool(e)) => {
            assert_eq!((e.status, e.code.to_string()), (409, "conflict".to_string()));
            assert_eq!(e.extra.get("current_version"), Some(&json!(3)));
        }
        other => panic!("{other:?}"),
    }
    assert_eq!(count(&w, "SELECT COUNT(*) FROM item_publishes"), 0, "the staged publish is gone");
    assert_eq!(count(&w, "SELECT COUNT(*) FROM author_contents"), 0);
    assert!(w.s.get(&s.full_key).unwrap().is_none(), "and its body");
}

#[test]
fn a_confirmed_student_data_block_holds_the_publish_and_raises_a_flag() {
    let w = world();
    let s = stage_one(&w, false);
    w.c.execute("INSERT INTO privacy_blocks (kind, hash, item_id, at) VALUES ('match', ?1, 'other', 1)", [&s.h.match_hashes[0]]).unwrap();
    go(&w, &s, None, &[], &[], 0).unwrap();
    assert_eq!(count(&w, "SELECT COUNT(*) FROM item_holds WHERE reason = 'student_data' AND source = 'privacy'"), 1);
    assert_eq!(count(&w, "SELECT COUNT(*) FROM admin_flags WHERE kind = 'privacy_text_match'"), 1);
    // A held item offers nothing to the licence history.
    assert_eq!(count(&w, "SELECT COUNT(*) FROM content_licences"), 0);
}

#[test]
fn a_near_rewording_count_places_its_own_hold() {
    let w = world();
    let s = stage_one(&w, false);
    go(&w, &s, None, &[], &[], 2).unwrap();
    assert_eq!(count(&w, "SELECT COUNT(*) FROM item_holds WHERE source = 'privacy_near'"), 1);
}

#[test]
fn a_hold_carried_from_another_item_is_placed_under_its_source() {
    let w = world();
    let s = stage_one(&w, false);
    go(&w, &s, Some(&Carried { reason: "admin".into(), item_id: "elsewhere".into() }), &[], &[], 0).unwrap();
    let (reason, source): (String, String) = w.c.query_row("SELECT reason, source FROM item_holds WHERE item_id = ?1", [ITEM], |r| Ok((r.get(0)?, r.get(1)?))).unwrap();
    assert_eq!((reason.as_str(), source.as_str()), ("admin", "carried:elsewhere"));
}

#[test]
fn flags_and_folded_flags_are_written_with_the_publish() {
    let w = world();
    let s = stage_one(&w, false);
    let kept = vec![Flag { id: "f1".into(), kind: "license_soft".into(), detail: "{\"x\":1}".into(), dedup: None }];
    let folded = vec![Flag { id: "f2".into(), kind: "origin_unverified".into(), detail: "{\"count\":2}".into(), dedup: Some("agg:origin_unverified:me:5".into()) }];
    go(&w, &s, None, &kept, &folded, 0).unwrap();
    assert_eq!(count(&w, "SELECT COUNT(*) FROM admin_flags WHERE id = 'f1' AND publish_id = 'AAAAAAAAAAAAAAAAAAAAAA'"), 1);
    let detail: String = w.c.query_row("SELECT detail FROM admin_flags WHERE dedup = 'agg:origin_unverified:me:5'", [], |r| r.get(0)).unwrap();
    let v: Value = serde_json::from_str(&detail).unwrap();
    assert_eq!((v["aggregated"].clone(), v["count"].clone()), (json!(1), json!(2)));
}

#[test]
fn a_public_item_offers_its_contents_to_the_licence_history() {
    let w = world();
    let mut s = stage_one(&w, false);
    // Public needs a public licence: CC-BY-4.0 is one.
    s.m.visibility = Some("public".into());
    let x = PoolCtx::new(&w.c, 5 * DAY_MS);
    let ckeys = vec!["q:abc".to_string()];
    commit(
        &x,
        &w.s,
        &CommitIn {
            me: "me",
            item_id: ITEM,
            is_new: true,
            kind: s.rw.kind,
            visibility: "public",
            version: 1,
            m: &s.m,
            plan: &s.plan,
            subject: "Geo",
            course: "",
            grade: "",
            n: 1,
            public_version: 1,
            public_at: None,
            reused: false,
            token: &s.token,
            base_of: None,
            result_json: "{}",
            ckeys: &ckeys,
            unit_record: &[],
            stem_record: &[],
            carried: None,
            flags: &[],
            folded: &[],
            circle_claimed: &[],
            circle_statement: None,
            privacy_near: 0,
            full_key: &s.full_key,
            stripped_key: None,
        },
    )
    .unwrap();
    let (ck, lic, until): (String, String, Option<i64>) = w.c.query_row("SELECT ckey, license, until_at FROM content_licences", [], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?))).unwrap();
    assert_eq!((ck.as_str(), lic.as_str(), until), ("q:abc", "CC-BY-4.0", None));
}

#[test]
fn dmca_still_used_names_the_blocked_actions_material() {
    let sql = dmca_still_used("h", "?2");
    assert!(sql.contains("bh.action_id = substr(h.source, 6)") && sql.contains("item_part_keys") && sql.contains("item_blobs"));
}
