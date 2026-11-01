//! Tests for `publishchecks.rs`: the soft check, content blocks, held-item carry-over, a new item that repeats one of the author's own, and a privacy-blocked
//! image whose grace window ended.
use super::*;
use crate::db;
use crate::pool::core::Core;
use crate::pool::fingerprint::Fingerprinter;
use crate::pool::publishmeta::read_meta;
use crate::pool::publishrewrite::rewrite;
use crate::pool::publishstart::Landing;
use crate::pool::sqlfrag::DAY_MS;
use rusqlite::Connection;
use rusqlite::types::Value as Sql;
use serde_json::{Value, json};
use std::path::Path;

const KEY: &str = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";
const ROOT: &str = "7f757fb737085dcaaad0727deb89a14c18141681dc56be0a215a592c715f8e3b";
const ITEM: &str = "0123456789abcdef0123456789abcdef";
const OTHER: &str = "22222222222222222222222222222222";

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
    for (id, name) in [("me", "me"), ("src", "Source")] {
        c.execute("INSERT INTO users (id, email, display_name, trust, created_at) VALUES (?1, ?2, ?3, 'teacher', 1)", params![id, format!("{id}@x.org"), name]).unwrap();
    }
    let s = BlobStore::new(dir.path().join("blobs")).unwrap();
    W { c, s, core: Core::new(Some(KEY.into())), fp: Fingerprinter::from_root_hex(ROOT).unwrap(), _d: dir }
}
fn row(c: &Connection, table: &str, set: &[(&str, Sql)]) {
    let cols: Vec<(String, String)> = c
        .prepare(&format!("SELECT name, type FROM pragma_table_info('{table}') WHERE \"notnull\" = 1 AND dflt_value IS NULL"))
        .unwrap()
        .query_map([], |r| Ok((r.get(0)?, r.get(1)?)))
        .unwrap()
        .map(|x| x.unwrap())
        .collect();
    let (mut names, mut vals): (Vec<String>, Vec<Sql>) = (vec![], vec![]);
    for (n, t) in cols {
        if set.iter().any(|(k, _)| *k == n) {
            continue;
        }
        names.push(n);
        vals.push(if t.to_uppercase().contains("INT") { 0.into() } else { String::new().into() });
    }
    for (k, v) in set {
        names.push(k.to_string());
        vals.push(v.clone());
    }
    let q = vec!["?"; names.len()].join(",");
    c.execute(&format!("INSERT INTO {table} ({}) VALUES ({q})", names.join(",")), rusqlite::params_from_iter(vals)).unwrap();
}
fn s(v: &str) -> Sql {
    v.to_string().into()
}
fn mc(stem: &str) -> Value {
    let words = ["apple", "river", "stone", "cloud", "ember", "maple", "orbit", "quartz"];
    let answers: Vec<Value> = words.iter().enumerate().map(|(i, t)| json!({ "text": t, "correct": i == 3, "locked": false })).collect();
    json!({ "id": "q", "stem": stem, "kind": { "Mc": { "answers": answers } }, "points": null, "difficulty": null, "topics": [], "image": null, "explanation": "because", "figure": null, "created": 1, "modified": 1 })
}

struct Ready {
    rw: Rewritten,
    h: Hashed,
    f: Facts,
}
fn ready(w: &W, qs: Vec<Value>, license: &str) -> Ready {
    let payload = json!({ "schema_version": 1, "bank": { "id": "local", "name": "B", "questions": qs, "created": 1, "modified": 1 } }).to_string().into_bytes();
    let m = json!({ "publish_id": "AAAAAAAAAAAAAAAAAAAAAA", "hide_key": false, "license": license, "visibility": "unlisted", "attestation": { "accepted": true, "text_rev": "2026-10-19" }, "meta": { "title": "Q", "subject": "S" } });
    let m = read_meta(m.as_object().unwrap(), true).unwrap();
    let land = Landing { item_id: ITEM.into(), version: 1, visibility: "unlisted".into(), item_kind: None, base_of: None, prev_public_version: 0, prev_public_at: None, prev_hidden: false, prev_license: None };
    let x = PoolCtx::new(&w.c, 5 * DAY_MS);
    let rw = rewrite(&x, &w.s, &w.core, "me", &land, true, &m, &payload, false).unwrap();
    let h = crate::pool::publishhash::prepare(&x, &w.s, &w.core, &w.fp, &land, true, &m, &rw).unwrap();
    let f = crate::pool::publishfacts::gather(&x, &w.s, &w.fp, "me", None, true, false, &m, &rw, &h, None).unwrap();
    Ready { rw, h, f }
}
fn go(w: &W, r: &Ready, is_new: bool, license: &str) -> Res<Checks> {
    run(&PoolCtx::new(&w.c, 5 * DAY_MS), &w.s, "me", ITEM, is_new, license, &r.rw, &r.h, &r.f)
}
fn other_item(w: &W, author: &str, id: &str, publish: &str, license: &str, mh: &str) {
    row(&w.c, "items", &[("id", s(id)), ("author_id", s(author)), ("kind", s("bank")), ("visibility", s("public")), ("current_publish_id", s(publish)), ("license", s(license))]);
    row(&w.c, "item_publishes", &[("publish_id", s(publish)), ("item_id", s(id)), ("author_id", s(author)), ("status", s("current"))]);
    row(&w.c, "item_questions", &[("publish_id", s(publish)), ("qid", s(&format!("{id}q"))), ("pos", 0.into()), ("match_hash", s(mh))]);
}

#[test]
fn a_fresh_bank_finds_nothing() {
    let w = world();
    let r = ready(&w, vec![mc("What is the capital of France?")], "CC-BY-4.0");
    assert_eq!(go(&w, &r, true, "CC-BY-4.0").unwrap(), Checks::default());
}

#[test]
fn the_soft_check_lists_visible_copies_under_a_licence_that_does_not_allow_this_one() {
    let w = world();
    let r = ready(&w, vec![mc("What is the capital of France?")], "CC-BY-4.0");
    other_item(&w, "src", OTHER, "PPPPPPPPPPPPPPPPPPPPPP", "CC-BY-NC-4.0", &r.h.match_hashes[0]);
    assert_eq!(go(&w, &r, true, "CC-BY-4.0").unwrap().soft, vec![(OTHER.to_string(), "CC-BY-NC-4.0".to_string())]);
    // A source whose licence allows ours is no noise worth a flag.
    w.c.execute("UPDATE items SET license = 'CC0-1.0'", []).unwrap();
    assert!(go(&w, &r, true, "CC-BY-4.0").unwrap().soft.is_empty());
    // ... and one the account imported is not a miss.
    w.c.execute("UPDATE items SET license = 'CC-BY-NC-4.0'", []).unwrap();
    row(&w.c, "imports", &[("user_id", s("me")), ("item_id", s(OTHER)), ("form", s("full"))]);
    assert!(go(&w, &r, true, "CC-BY-4.0").unwrap().soft.is_empty());
}

#[test]
fn an_active_block_on_the_match_hash_is_found_until_lifted_or_expired() {
    let w = world();
    let r = ready(&w, vec![mc("What is the capital of France?")], "CC-BY-4.0");
    row(&w.c, "blocked_hashes", &[("kind", s("match")), ("hash", s(&r.h.match_hashes[0])), ("action_id", s("act1"))]);
    let b = go(&w, &r, true, "CC-BY-4.0").unwrap().blocked.unwrap();
    assert_eq!((b.kind.as_str(), b.action_id.as_deref()), ("match", Some("act1")));
    w.c.execute("UPDATE blocked_hashes SET expires_at = 1000", []).unwrap();
    assert!(go(&w, &r, true, "CC-BY-4.0").unwrap().blocked.is_none(), "expired");
    w.c.execute("UPDATE blocked_hashes SET expires_at = NULL, lifted_at = 1", []).unwrap();
    assert!(go(&w, &r, true, "CC-BY-4.0").unwrap().blocked.is_none(), "lifted");
}

#[test]
fn a_new_item_that_repeats_one_of_the_authors_own_is_a_possible_duplicate() {
    let w = world();
    let r = ready(&w, vec![mc("What is the capital of France?")], "CC-BY-4.0");
    other_item(&w, "me", OTHER, "PPPPPPPPPPPPPPPPPPPPPP", "CC-BY-4.0", &r.h.match_hashes[0]);
    assert_eq!(go(&w, &r, true, "CC-BY-4.0").unwrap().possible_duplicate.as_deref(), Some(OTHER));
    assert_eq!(go(&w, &r, false, "CC-BY-4.0").unwrap().possible_duplicate, None, "only a new item");
    w.c.execute("UPDATE items SET removed_at = 1, removed_reason = 'author_deleted'", []).unwrap();
    assert_eq!(go(&w, &r, true, "CC-BY-4.0").unwrap().possible_duplicate, None, "a removed item is no duplicate");
}

#[test]
fn a_hold_an_admin_placed_on_another_item_with_the_same_questions_is_carried() {
    let w = world();
    let r = ready(&w, vec![mc("What is the capital of France?")], "CC-BY-4.0");
    other_item(&w, "me", OTHER, "PPPPPPPPPPPPPPPPPPPPPP", "CC-BY-4.0", &r.h.match_hashes[0]);
    row(&w.c, "item_holds", &[("item_id", s(OTHER)), ("reason", s("admin")), ("source", s("manual")), ("by", s("admin1"))]);
    let c = go(&w, &r, false, "CC-BY-4.0").unwrap().carried.unwrap();
    assert_eq!((c.reason.as_str(), c.item_id.as_str()), ("admin", OTHER));
    // One reporter alone is not enough when no admin placed it.
    w.c.execute("UPDATE item_holds SET by = NULL", []).unwrap();
    assert!(go(&w, &r, false, "CC-BY-4.0").unwrap().carried.is_none());
}
