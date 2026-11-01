//! Tests for `publishflags.rs`: one flag per kind with a count and the first few, the repeated-claim counts, and the daily folding.
use super::*;
use crate::db;
use crate::pool::publishmeta::read_meta;
use crate::pool::sqlfrag::DAY_MS;
use rusqlite::Connection;
use std::path::Path;

const NOW: i64 = 40 * DAY_MS;

fn open() -> (Connection, tempfile::TempDir) {
    let dir = tempfile::tempdir().unwrap();
    let mut c = db::open(&dir.path().join("t.db")).unwrap();
    db::migrate(&mut c, &Path::new(env!("CARGO_MANIFEST_DIR")).join("../worker/migrations")).unwrap();
    c.execute("INSERT INTO users (id, email, display_name, trust, created_at) VALUES ('me', 'me@x.org', 'Me', 'teacher', 1)", []).unwrap();
    (c, dir)
}
fn meta(independent: &[&str]) -> PublishMeta {
    let mut m = json!({ "publish_id": "AAAAAAAAAAAAAAAAAAAAAA", "hide_key": false, "license": "CC-BY-4.0", "visibility": "unlisted", "attestation": { "accepted": true, "text_rev": "2026-10-19" }, "meta": { "title": "Q", "subject": "S" } });
    if !independent.is_empty() {
        m["independent_of"] = json!({ "accepted": true, "items": independent, "statement": "I wrote these myself." });
    }
    read_meta(m.as_object().unwrap(), true).unwrap()
}
fn input<'a>(m: &'a PublishMeta, d: &'a Derived) -> FlagIn<'a> {
    FlagIn { me: "me", m, d, soft: &[], blocked: None, suspect: &[], privacy_near: &[], overflowed: &[], circle_claimed: &[], circle_statement: None }
}
fn audit_claim(c: &Connection, id: &str, at: i64, items: &[&str]) {
    c.execute(
        "INSERT INTO pool_audit (id, at, actor_id, actor_kind, action, detail) VALUES (?1, ?2, 'me', 'user', 'item.publish', ?3)",
        params![id, at, json!({ "independent_of": items }).to_string()],
    )
    .unwrap();
}
const A: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
const B: &str = "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";
const C: &str = "cccccccccccccccccccccccccccccccc";

#[test]
fn a_clean_publish_raises_nothing() {
    let (c, _d) = open();
    let x = PoolCtx::new(&c, NOW);
    let m = meta(&[]);
    let d = Derived::default();
    assert_eq!(build(&x, &input(&m, &d)).unwrap(), (vec![], vec![]));
}

#[test]
fn each_kind_is_one_flag_with_a_count_and_the_first_few() {
    let (c, _d) = open();
    let x = PoolCtx::new(&c, NOW);
    let m = meta(&[]);
    let d = Derived { conflicts: (0..9).collect(), soft_only: vec![2], unverified: vec![(1, "i".into())], ..Default::default() };
    let soft = vec![("it1".to_string(), "CC-BY-NC-4.0".to_string())];
    let blocked = Blocked { kind: "match".into(), hash: "h".into(), action_id: None };
    let (kept, folded) = build(&x, &FlagIn { soft: &soft, blocked: Some(&blocked), suspect: &[3, 4], ..input(&m, &d) }).unwrap();
    let kinds: Vec<&str> = kept.iter().map(|f| f.kind.as_str()).collect();
    assert_eq!(kinds, vec!["license_soft", "license_conflict", "origin_suspect", "blocklist"]);
    let conflict: Value = serde_json::from_str(&kept[1].detail).unwrap();
    assert_eq!(conflict["positions"]["count"], 9);
    assert_eq!(conflict["positions"]["first"], json!([0, 1, 2, 3, 4]));
    assert_eq!(conflict["license"], "CC-BY-4.0");
    assert_eq!(serde_json::from_str::<Value>(&kept[0].detail).unwrap()["matches_items"]["first"], json!(["it1"]));
    // origin_unverified always folds into a daily aggregate.
    assert_eq!(folded.len(), 1);
    assert_eq!((folded[0].kind.as_str(), folded[0].dedup.as_deref()), ("origin_unverified", Some("agg:origin_unverified:me:40")));
}

#[test]
fn a_new_independence_claim_is_flagged_and_the_third_in_30_days_flags_the_account() {
    let (c, _d) = open();
    let x = PoolCtx::new(&c, NOW);
    let d = Derived::default();
    let m = meta(&[C]);
    let (kept, _) = build(&x, &input(&m, &d)).unwrap();
    assert_eq!(kept.iter().map(|f| f.kind.as_str()).collect::<Vec<_>>(), vec!["independent_claim"]);
    audit_claim(&c, "a1", NOW - DAY_MS, &[A]);
    audit_claim(&c, "a2", NOW - 2 * DAY_MS, &[B]);
    let (kept, _) = build(&x, &input(&m, &d)).unwrap();
    let kinds: Vec<&str> = kept.iter().map(|f| f.kind.as_str()).collect();
    assert_eq!(kinds, vec!["independent_repeat", "independent_claim"]);
    assert_eq!(serde_json::from_str::<Value>(&kept[0].detail).unwrap()["claims_in_30_days"], 3);
}

#[test]
fn a_claim_about_items_already_claimed_is_not_a_new_one() {
    let (c, _d) = open();
    let x = PoolCtx::new(&c, NOW);
    audit_claim(&c, "a1", NOW - DAY_MS, &[A]);
    audit_claim(&c, "a2", NOW - 2 * DAY_MS, &[B]);
    assert_eq!(prior_claims(&x, "me", &[A.to_string()], "independent_of").unwrap(), 0);
    assert_eq!(prior_claims(&x, "me", &[A.to_string(), C.to_string()], "independent_of").unwrap(), 2);
    assert_eq!(prior_claims(&x, "me", &[], "independent_of").unwrap(), 0);
}

#[test]
fn claims_older_than_30_days_do_not_count() {
    let (c, _d) = open();
    let x = PoolCtx::new(&c, NOW);
    audit_claim(&c, "a1", NOW - 31 * DAY_MS, &[A]);
    assert_eq!(prior_claims(&x, "me", &[C.to_string()], "independent_of").unwrap(), 0);
}

#[test]
fn circle_claims_name_the_banks_and_the_statement() {
    let (c, _d) = open();
    let x = PoolCtx::new(&c, NOW);
    let m = meta(&[]);
    let d = Derived::default();
    let banks = vec!["bank1".to_string()];
    let (kept, _) = build(&x, &FlagIn { circle_claimed: &banks, circle_statement: Some("mine"), ..input(&m, &d) }).unwrap();
    let v: Value = serde_json::from_str(&kept[0].detail).unwrap();
    assert_eq!((kept[0].kind.as_str(), v["statement"].as_str(), v["circle_banks"]["first"].clone()), ("independent_claim", Some("mine"), json!(["bank1"])));
}

#[test]
fn a_busy_kind_folds_after_the_daily_cap_but_integrity_kinds_never_do() {
    let (c, _d) = open();
    let x = PoolCtx::new(&c, NOW);
    for i in 0..FLAGS_PER_ACCOUNT_DAY {
        for kind in ["license_conflict", "hold_cleared"] {
            c.execute("INSERT INTO admin_flags (id, kind, user_id, detail, at) VALUES (?1, ?2, 'me', '{}', ?3)", params![format!("{kind}{i}"), kind, NOW - 1000]).unwrap();
        }
    }
    let m = meta(&[]);
    let d = Derived { conflicts: vec![0], ..Default::default() };
    let (kept, folded) = build(&x, &input(&m, &d)).unwrap();
    assert_eq!(kept.len(), 1, "license_conflict is an integrity kind: it is never folded");
    assert!(folded.is_empty());
}
