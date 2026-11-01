//! Tests for `publishstart.rs`: the daily allowances and their refund, the account lock, the held-publish cap, the idempotency rules (replay, conflict, in
//! flight, abandoned) and the item a publish lands on.
use super::*;
use crate::db;
use rusqlite::Connection;
use rusqlite::types::Value as Sql;
use serde_json::Map;
use std::path::Path;

const PID: &str = "AAAAAAAAAAAAAAAAAAAAAA";
const PID2: &str = "BBBBBBBBBBBBBBBBBBBBBB";
const SHA: &str = "sha-of-the-request";

fn open() -> (Connection, tempfile::TempDir, BlobStore) {
    let dir = tempfile::tempdir().unwrap();
    let mut c = db::open(&dir.path().join("t.db")).unwrap();
    db::migrate(&mut c, &Path::new(env!("CARGO_MANIFEST_DIR")).join("../worker/migrations")).unwrap();
    for u in ["me", "other"] {
        c.execute("INSERT INTO users (id, email, display_name, trust, created_at) VALUES (?1, ?2, ?1, 'teacher', 1)", params![u, format!("{u}@x.org")]).unwrap();
    }
    let s = BlobStore::new(dir.path().join("blobs")).unwrap();
    (c, dir, s)
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
    let ph: Vec<String> = (1..=names.len()).map(|i| format!("?{i}")).collect();
    c.execute(&format!("INSERT INTO {table} ({}) VALUES ({})", names.join(", "), ph.join(", ")), rusqlite::params_from_iter(vals)).unwrap();
}
fn t(s: &str) -> Sql {
    s.to_string().into()
}
fn i(v: i64) -> Sql {
    v.into()
}
fn n(c: &Connection, sql: &str) -> i64 {
    c.query_row(sql, [], |r| r.get(0)).unwrap()
}
fn err(r: Res<impl std::fmt::Debug>) -> PoolError {
    match r.expect_err("refused") {
        Fail::Pool(p) => p,
        other => panic!("not a pool error: {other:?}"),
    }
}
fn meta(is_new: bool, base: Option<&str>, own_work: bool) -> PublishMeta {
    let mut m: Map<String, Value> = json!({
        "publish_id": PID, "hide_key": false, "license": "CC-BY-4.0", "visibility": "unlisted",
        "attestation": { "accepted": true, "text_rev": "2026-10-19" },
        "attest": { "own_work": own_work, "terms_version": POOL_TERMS_VERSION }
    })
    .as_object()
    .unwrap()
    .clone();
    if let Some(b) = base {
        m.insert("base_publish_id".into(), json!(b));
    }
    super::super::publishmeta::read_meta(&m, is_new).unwrap()
}
fn item(c: &Connection, id: &str, author: &str, current: Option<&str>) {
    row(c, "items", &[("id", t(id)), ("author_id", t(author)), ("kind", t("bank")), ("visibility", t("unlisted")), ("version", i(3)), ("license", t("CC-BY-4.0")), ("current_publish_id", current.map(t).unwrap_or(Sql::Null)), ("created_at", i(1)), ("updated_at", i(1))]);
}
fn publish_row(c: &Connection, id: &str, item_id: &str, author: &str, status: &str, sha: &str, result: Option<&str>, created: i64) {
    row(
        c,
        "item_publishes",
        &[("publish_id", t(id)), ("item_id", t(item_id)), ("author_id", t(author)), ("status", t(status)), ("request_sha256", t(sha)), ("result_json", result.map(t).unwrap_or(Sql::Null)), ("full_key", t(&format!("bodies/{id}"))), ("created_at", i(created))],
    );
}

#[test]
fn a_publish_counts_a_reserved_refusal_that_a_success_or_a_conflict_gives_back() {
    let (c, _d, _s) = open();
    let x = PoolCtx::new(&c, 1_000);
    admit(&x, "me", false).unwrap();
    assert_eq!(n(&c, "SELECT count FROM usage_daily WHERE metric = 'publish_refusals'"), 1);
    settle(&x, "me", false, None).unwrap();
    assert_eq!(n(&c, "SELECT count FROM usage_daily WHERE metric = 'publish_refusals'"), 0);
    admit(&x, "me", false).unwrap();
    settle(&x, "me", false, Some(&PoolError::new(409, "conflict"))).unwrap();
    assert_eq!(n(&c, "SELECT count FROM usage_daily WHERE metric = 'publish_refusals'"), 0, "a conflict is not a refusal");
    admit(&x, "me", false).unwrap();
    settle(&x, "me", false, Some(&PoolError::new(422, "invalid_payload"))).unwrap();
    assert_eq!(n(&c, "SELECT count FROM usage_daily WHERE metric = 'publish_refusals'"), 1, "a refused payload keeps its count");
}

#[test]
fn past_the_days_allowance_the_publish_is_refused_before_any_work_and_still_counts() {
    let (c, _d, _s) = open();
    let x = PoolCtx::new(&c, 1_000);
    let limit = x.limit_of("me", "publishes").unwrap();
    c.execute("INSERT INTO usage_daily (user_id, day, metric, count) VALUES ('me', ?1, 'publishes', ?2)", params![day_of(1_000), limit]).unwrap();
    let e = err(admit(&x, "me", false));
    assert_eq!((e.status, e.code.as_str(), e.extra["metric"].as_str()), (429, "daily_limit", Some("publishes")));
    assert_eq!(e.headers, vec![("Retry-After".to_string(), "3600".to_string())]);
    assert_eq!(n(&c, "SELECT count FROM usage_daily WHERE metric = 'publish_refusals'"), 1, "not refunded: retries cannot buy free validations");
    assert_eq!(n(&c, "SELECT COUNT(*) FROM admin_flags WHERE kind = 'cap'"), 1);
}

#[test]
fn a_dry_run_has_its_own_allowance_and_counts_nothing_else() {
    let (c, _d, _s) = open();
    let x = PoolCtx::new(&c, 1_000);
    admit(&x, "me", true).unwrap();
    assert_eq!(n(&c, "SELECT count FROM usage_daily WHERE metric = 'previews'"), 1);
    assert_eq!(n(&c, "SELECT COUNT(*) FROM usage_daily WHERE metric = 'publish_refusals'"), 0);
    settle(&x, "me", true, None).unwrap();
    let limit = x.limit_of("me", "previews").unwrap();
    c.execute("UPDATE usage_daily SET count = ?1 WHERE metric = 'previews'", [limit]).unwrap();
    assert_eq!(err(admit(&x, "me", true)).code, "daily_limit");
}

#[test]
fn only_one_publish_runs_per_account_and_a_refused_lock_gives_the_count_back() {
    let (c, _d, _s) = open();
    let x = PoolCtx::new(&c, 1_000);
    admit(&x, "me", false).unwrap();
    take_account_lock(&x, "me", false).unwrap();
    admit(&x, "me", false).unwrap();
    let e = err(take_account_lock(&x, "me", false));
    assert_eq!((e.status, e.code.as_str()), (409, "publish_in_progress"));
    assert_eq!(e.headers, vec![("Retry-After".to_string(), "5".to_string())]);
    assert_eq!(n(&c, "SELECT count FROM usage_daily WHERE metric = 'publish_refusals'"), 1, "the second attempt's count was given back");
}

#[test]
fn an_erased_account_is_told_it_is_unauthenticated_not_busy() {
    let (c, _d, _s) = open();
    c.execute("UPDATE users SET deleted_at = 5, email = NULL, pw_hash = NULL, pw_params = NULL, display_name = NULL, profile = NULL WHERE id = 'me'", []).unwrap();
    assert_eq!(err(take_account_lock(&PoolCtx::new(&c, 1_000), "me", false)).status, 401);
}

#[test]
fn over_the_held_cap_the_admin_is_alerted_and_a_bad_item_id_is_a_counted_miss() {
    let (c, _d, _s) = open();
    let x = PoolCtx::new(&c, 1_000);
    assert!(!before_heavy(&x, "me", None).unwrap());
    c.execute("INSERT INTO usage_daily (user_id, day, metric, count) VALUES ('me', ?1, 'held_publishes', ?2)", params![day_of(1_000), HELD_PUBLISHES_PER_DAY]).unwrap();
    assert!(before_heavy(&x, "me", None).unwrap(), "over the cap: never a refusal");
    assert_eq!(n(&c, "SELECT COUNT(*) FROM admin_flags WHERE kind = 'cap'"), 1);
    let e = err(before_heavy(&x, "me", Some("not-an-id")));
    assert_eq!((e.status, e.code.as_str()), (404, "not_found"));
    assert_eq!(n(&c, "SELECT count FROM usage_daily WHERE metric = 'item_miss'"), 1);
}

#[test]
fn a_new_item_gets_a_fresh_id_at_version_one_with_the_chosen_visibility() {
    let (c, _d, s) = open();
    let x = PoolCtx::new(&c, 1_000);
    let Started::Go(l) = identify(&x, &s, "me", None, &meta(true, None, true), SHA, false).unwrap() else { panic!("go") };
    assert_eq!((l.version, l.visibility.as_str(), l.base_of, l.prev_public_version, l.prev_hidden), (1, "unlisted", None, 0, false));
    assert!(is_id(&l.item_id));
}

#[test]
fn the_terms_affirmation_is_required_except_for_a_preview() {
    let (c, _d, s) = open();
    let x = PoolCtx::new(&c, 1_000);
    let e = err(identify(&x, &s, "me", None, &meta(true, None, false), SHA, false));
    assert_eq!((e.status, e.code.as_str(), e.extra["terms_version"].as_str()), (422, "attestation_required", Some(POOL_TERMS_VERSION)));
    assert!(identify(&x, &s, "me", None, &meta(true, None, false), SHA, true).is_ok());
}

#[test]
fn a_republish_lands_on_the_next_version_of_the_current_head() {
    let (c, _d, s) = open();
    let head = "CCCCCCCCCCCCCCCCCCCCCC";
    let id = "0123456789abcdef0123456789abcdef";
    item(&c, id, "me", Some(head));
    let x = PoolCtx::new(&c, 1_000);
    let Started::Go(l) = identify(&x, &s, "me", Some(id), &meta(false, Some(head), true), SHA, false).unwrap() else { panic!("go") };
    assert_eq!((l.item_id.as_str(), l.version, l.base_of.as_deref(), l.item_kind.as_deref(), l.prev_license.as_deref()), (id, 4, Some(head), Some("bank"), Some("CC-BY-4.0")));
    // A stale base is a conflict that names the current head.
    let e = err(identify(&x, &s, "me", Some(id), &meta(false, Some(PID2), true), SHA, false));
    assert_eq!((e.status, e.code.as_str(), e.extra["current_publish_id"].as_str(), e.extra["current_version"].as_i64()), (409, "conflict", Some(head), Some(3)));
    // Someone else's item, or none, is a counted 404.
    let e = err(identify(&x, &s, "other", Some(id), &meta(false, Some(head), true), SHA, false));
    assert_eq!(e.status, 404);
    assert_eq!(n(&c, "SELECT count FROM usage_daily WHERE user_id = 'other' AND metric = 'item_miss'"), 1);
}

#[test]
fn a_removed_item_cannot_be_republished_and_says_whether_it_is_gone_for_good() {
    let (c, _d, s) = open();
    let id = "0123456789abcdef0123456789abcdef";
    let head = "CCCCCCCCCCCCCCCCCCCCCC";
    item(&c, id, "me", Some(head));
    c.execute("UPDATE items SET removed_at = 5, removed_reason = 'admin'", []).unwrap();
    let x = PoolCtx::new(&c, 1_000);
    let e = err(identify(&x, &s, "me", Some(id), &meta(false, Some(head), true), SHA, false));
    assert_eq!((e.status, e.code.as_str(), e.extra.contains_key("purged")), (409, "item_removed", false));
    c.execute("UPDATE items SET purged_at = 9", []).unwrap();
    let e = err(identify(&x, &s, "me", Some(id), &meta(false, Some(head), true), SHA, false));
    assert_eq!(e.extra["purged"], json!(true));
}

#[test]
fn a_circle_only_licence_cannot_be_published_to_a_public_item() {
    let (c, _d, s) = open();
    let id = "0123456789abcdef0123456789abcdef";
    let head = "CCCCCCCCCCCCCCCCCCCCCC";
    item(&c, id, "me", Some(head));
    c.execute("UPDATE items SET visibility = 'public'", []).unwrap();
    let mut m = meta(false, Some(head), true);
    m.license = "circle-only".into();
    assert_eq!(err(identify(&PoolCtx::new(&c, 1_000), &s, "me", Some(id), &m, SHA, false)).code, "license_not_public");
}

#[test]
fn the_same_request_again_replays_its_result_and_anything_else_with_that_id_conflicts() {
    let (c, _d, s) = open();
    item(&c, "0123456789abcdef0123456789abcdef", "me", Some(PID));
    publish_row(&c, PID, "0123456789abcdef0123456789abcdef", "me", "current", SHA, Some(r#"{"item_id":"x","version":1}"#), 10);
    let x = PoolCtx::new(&c, 1_000_000);
    let Started::Replay(v) = identify(&x, &s, "me", None, &meta(true, None, true), SHA, false).unwrap() else { panic!("replay") };
    assert_eq!(v, json!({ "item_id": "x", "version": 1, "replayed": true }));
    // A different body under the same id, or another account's id: never resumed, never revealed.
    assert_eq!(err(identify(&x, &s, "me", None, &meta(true, None, true), "another-sha", false)).code, "publish_id_conflict");
    assert_eq!(err(identify(&x, &s, "other", None, &meta(true, None, true), SHA, false)).code, "publish_id_conflict");
    // A retired id is never reused.
    c.execute("INSERT INTO retired_publish_ids (publish_id, author_id) VALUES (?1, 'me')", [PID2]).unwrap();
    let mut m = meta(true, None, true);
    m.publish_id = PID2.into();
    assert_eq!(err(identify(&x, &s, "me", None, &m, SHA, false)).code, "publish_id_conflict");
}

#[test]
fn a_young_attempt_is_in_flight_and_an_old_one_is_discarded_to_start_again() {
    let (c, _d, s) = open();
    item(&c, "0123456789abcdef0123456789abcdef", "me", None);
    publish_row(&c, PID, "0123456789abcdef0123456789abcdef", "me", "staged", SHA, None, 990_000);
    s.put(&format!("bodies/{PID}"), b"staged body").unwrap();
    let e = err(identify(&PoolCtx::new(&c, 1_000_000), &s, "me", None, &meta(true, None, true), SHA, false));
    assert_eq!((e.status, e.code.as_str()), (409, "publish_in_progress"));
    assert_eq!(n(&c, "SELECT COUNT(*) FROM item_publishes"), 1);
    let x = PoolCtx::new(&c, 990_000 + IN_FLIGHT_MS + 1);
    assert!(matches!(identify(&x, &s, "me", None, &meta(true, None, true), SHA, false).unwrap(), Started::Go(_)));
    assert_eq!(n(&c, "SELECT COUNT(*) FROM item_publishes"), 0, "the abandoned attempt is gone");
    assert!(s.get(&format!("bodies/{PID}")).unwrap().is_none(), "and its body");
}
