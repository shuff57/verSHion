//! Tests for `poolio.rs`: burst limits, the outbox, held-copy notices and the pure body readers (worker/test/pool/round19.test.ts D5
//! for the duplicate-key refusals).
use super::*;
use crate::auth::crypto::derive_keys;
use crate::db;
use crate::pool::ctx::Fail;
use std::path::Path;

fn open() -> (Connection, tempfile::TempDir) {
    let dir = tempfile::tempdir().unwrap();
    let mut conn = db::open(&dir.path().join("t.db")).unwrap();
    db::migrate(&mut conn, &Path::new(env!("CARGO_MANIFEST_DIR")).join("../worker/migrations")).unwrap();
    (conn, dir)
}
fn pool_err(r: Res<impl std::fmt::Debug>) -> PoolError {
    match r {
        Err(Fail::Pool(e)) => e,
        other => panic!("{other:?}"),
    }
}
fn key() -> Key {
    derive_keys("test-secret-0123456789abcdef0123456789abcdef").limit
}
fn ident(s: &str) -> String {
    s.to_string()
}

#[test]
fn a_missing_client_address_is_a_403() {
    let (c, _d) = open();
    for ip in [None, Some(""), Some("   ")] {
        let e = pool_err(rate_limit(&c, &key(), 1_000, ip, &ident, "u", false));
        assert_eq!((e.status, e.code.as_str()), (403, "client_ip_required"));
    }
}

#[test]
fn the_account_limit_trips_at_300_in_five_minutes_and_a_denied_request_costs_nothing_more() {
    let (c, _d) = open();
    let k = key();
    for _ in 0..300 {
        rate_limit(&c, &k, 1_000, Some("198.51.100.4"), &ident, "u", false).unwrap();
    }
    let e = pool_err(rate_limit(&c, &k, 1_000, Some("198.51.100.4"), &ident, "u", false));
    assert_eq!((e.status, e.code.as_str()), (429, "rate_limited"));
    assert!(e.headers.iter().any(|(h, v)| h.eq_ignore_ascii_case("retry-after") && v.parse::<i64>().unwrap() >= 1));
    // Another account at the same address is unaffected, and the refused hits were taken back (the address saw 300, not 301).
    rate_limit(&c, &k, 1_000, Some("198.51.100.4"), &ident, "v", false).unwrap();
    let ip_rows: i64 = c.query_row("SELECT COALESCE(SUM(count), 0) FROM rate_limits WHERE key LIKE 'i:%' AND key NOT LIKE '%read%'", [], |r| r.get(0)).unwrap();
    assert_eq!(ip_rows, 301, "300 passed + 1 for the other account; the refused one was undone");
}

#[test]
fn an_address_over_its_limit_refuses_every_account_behind_it_and_reads_have_their_own_daily_budget() {
    let (c, _d) = open();
    let k = key();
    // A read charges the account's and the address's daily read counters too.
    rate_limit(&c, &k, 1_000, Some("203.0.113.9"), &ident, "u", true).unwrap();
    let reads: i64 = c.query_row("SELECT COUNT(*) FROM rate_limits WHERE key LIKE '%read:86400'", [], |r| r.get(0)).unwrap();
    assert_eq!(reads, 2);
    let writes: i64 = c.query_row("SELECT COUNT(*) FROM rate_limits", [], |r| r.get(0)).unwrap();
    rate_limit(&c, &k, 1_000, Some("203.0.113.9"), &ident, "u", false).unwrap();
    assert_eq!(writes, 4, "a read: account burst + account read + address burst + address read");
    // Fill the address's burst limit with many accounts.
    for i in 0..6000 {
        let _ = rate_limit(&c, &k, 2_000, Some("192.0.2.77"), &ident, &format!("acct{}", i % 25), false);
    }
    let e = pool_err(rate_limit(&c, &k, 2_000, Some("192.0.2.77"), &ident, "fresh", false));
    assert_eq!(e.status, 429);
}

#[test]
fn ipv6_addresses_share_a_bucket_through_the_route_supplied_grouping() {
    let (c, _d) = open();
    let k = key();
    let group = |_: &str| "same".to_string();
    rate_limit(&c, &k, 1_000, Some("2001:db8::1"), &group, "u", false).unwrap();
    rate_limit(&c, &k, 1_000, Some("2001:db8::2"), &group, "u", false).unwrap();
    let ip_keys: i64 = c.query_row("SELECT COUNT(DISTINCT key) FROM rate_limits WHERE key LIKE 'i:%'", [], |r| r.get(0)).unwrap();
    assert_eq!(ip_keys, 1);
}

fn notice(item: &str) -> PoolNotice {
    PoolNotice::CopyHeld { item_id: item.into(), author_id: "a".into(), cause: "copyright".into() }
}

#[test]
fn the_outbox_writes_once_flushes_once_and_keeps_a_failed_notice_for_the_next_flush() {
    let (c, _d) = open();
    assert!(outbox_insert(&c, "n1:x", &notice("i1"), 10, "1").unwrap());
    assert!(!outbox_insert(&c, "n1:x", &notice("i1"), 11, "1").unwrap(), "a retry writes it once");
    assert!(!outbox_insert(&c, "n2:x", &notice("i2"), 12, "0").unwrap(), "the condition decides");
    let mut seen = Vec::new();
    let mut failing = |_: &PoolNotice| Err::<(), _>("smtp down".to_string());
    assert_eq!(flush_outbox(&c, 20, None, &mut failing).unwrap(), 0);
    assert_eq!(flush_outbox(&c, 20, Some(&["other".into()]), &mut failing).unwrap(), 0);
    assert_eq!(flush_outbox(&c, 30, None, &mut |n: &PoolNotice| { seen.push(n.clone()); Ok(()) }).unwrap(), 1);
    assert_eq!(seen, vec![notice("i1")]);
    assert_eq!(flush_outbox(&c, 40, None, &mut |_: &PoolNotice| Ok(())).unwrap(), 0, "sent notices are not sent again");
    let sent_at: i64 = c.query_row("SELECT sent_at FROM pool_outbox WHERE id = 'n1:x'", [], |r| r.get(0)).unwrap();
    assert_eq!(sent_at, 30);
    // By id.
    outbox_insert(&c, "n3:y", &notice("i3"), 50, "1").unwrap();
    assert_eq!(flush_outbox(&c, 60, Some(&["n3:y".into()]), &mut |_: &PoolNotice| Ok(())).unwrap(), 1);
}

#[test]
fn held_copy_notices_go_to_live_authors_only_and_obey_the_hourly_budget() {
    let (c, _d) = open();
    for u in ["a", "g"] {
        c.execute("INSERT INTO users (id, email, display_name, trust, created_at) VALUES (?1, ?2, ?1, 'teacher', 1)", params![u, format!("{u}@x.org")]).unwrap();
    }
    c.execute("UPDATE users SET deleted_at = 5, email = NULL, pw_hash = NULL, pw_params = NULL, display_name = NULL, profile = NULL WHERE id = 'g'", []).unwrap();
    for (n, author) in [(1, "a"), (2, "g")] {
        c.execute(
            "INSERT INTO items (id, author_id, kind, visibility, current_publish_id, public_publish_id, title, license, committed_at, created_at, updated_at, fold_rev) VALUES (?1, ?2, 'bank', 'public', 'p', 'p', 'T', 'CC-BY-4.0', 1, 1, 1, 1)",
            params![format!("{n:032x}"), author],
        )
        .unwrap();
    }
    let x = PoolCtx::new(&c, 1_000);
    notify_held(&x, &[format!("{:032x}", 1), format!("{:032x}", 2)], "student_data").unwrap();
    let sent = x.take_notices();
    assert_eq!(sent.len(), 1);
    assert!(matches!(&sent[0], PoolNotice::CopyHeld { author_id, cause, .. } if author_id == "a" && cause == "student_data"));
    notify_held(&x, &[], "copyright").unwrap();
    assert!(x.take_notices().is_empty());
}

fn framed(meta: &str, payload: &str) -> Vec<u8> {
    format!("{meta}\n{payload}").into_bytes()
}

#[test]
fn a_publish_is_a_meta_line_then_the_payload_with_its_limits() {
    let raw = framed(r#"{"license":"CC-BY-4.0"}"#, "PAYLOAD");
    let (meta, at) = parse_publish(&raw, 100, 100).unwrap();
    assert_eq!(meta["license"], "CC-BY-4.0");
    assert_eq!(&raw[at..], b"PAYLOAD");
    // No newline, or a meta line past its limit: invalid_request field=meta.
    for r in [parse_publish(b"{}", 100, 100), parse_publish(&raw, 100, 5)] {
        let e = pool_err(r);
        assert_eq!((e.status, e.code.as_str(), e.body()["field"].clone()), (400, "invalid_request", "meta".into()));
    }
    let e = pool_err(parse_publish(&raw, 3, 100));
    assert_eq!((e.status, e.code.as_str(), e.body()["limit"].clone()), (413, "body_too_large", 3.into()));
    // Duplicate keys in the meta are refused, never read last-wins; so is a non-object or invalid UTF-8.
    let e = pool_err(parse_publish(&framed(r#"{"license":"CC-BY-4.0","license":"CC0-1.0"}"#, "x"), 100, 100));
    assert_eq!((e.status, e.code.as_str(), e.body()["reason"].clone()), (400, "invalid_json", "duplicate_key".into()));
    for meta in ["[1]", "5", "null"] {
        assert_eq!(pool_err(parse_publish(&framed(meta, "x"), 100, 100)).code, "invalid_json", "{meta}");
    }
    let mut bad = b"{\"a\":\"".to_vec();
    bad.extend([0xff, 0xfe]);
    bad.extend(b"\"}\nx");
    assert_eq!(pool_err(parse_publish(&bad, 100, 100)).code, "invalid_json");
}

#[test]
fn json_bodies_need_the_json_type_a_clean_object_and_an_optional_body_may_be_absent() {
    let json = Some("application/json; charset=utf-8");
    assert_eq!(read_json(json, Some("13"), br#"{"reason":"x"}"#, false).unwrap()["reason"], "x");
    // D5: a duplicate key is refused before the parse.
    let e = pool_err(read_json(json, None, br#"{"reason":"wrong_answer","reason":"student_data"}"#, false));
    assert_eq!((e.status, e.code.as_str(), e.body()["reason"].clone()), (400, "invalid_json", "duplicate_key".into()));
    for body in [&b"[1]"[..], b"5", b"", b"{", b"\xef\xbb\xbf{}"] {
        assert_eq!(pool_err(read_json(json, None, body, false)).code, "invalid_json", "{body:?}");
    }
    for ct in [None, Some("text/plain"), Some("application/x-www-form-urlencoded")] {
        assert_eq!(pool_err(read_json(ct, Some("2"), b"{}", false)).status, 415);
    }
    let big = format!("{{\"a\":\"{}\"}}", "x".repeat(JSON_BODY_MAX));
    assert_eq!(pool_err(read_json(json, None, big.as_bytes(), false)).status, 413);
    // Optional: Content-Length 0 whatever the type, or neither a length nor a type, is {}; a real body of the wrong type is a 415.
    assert!(read_json(Some("text/plain"), Some("0"), b"", true).unwrap().is_empty());
    assert!(read_json(None, None, b"", true).unwrap().is_empty());
    assert!(read_json(json, None, b"", true).unwrap().is_empty());
    assert_eq!(pool_err(read_json(Some("text/plain"), Some("2"), b"{}", true)).status, 415);
}

#[test]
fn the_content_type_is_the_media_type_alone() {
    assert_eq!(content_type_of(Some(" Application/JSON ; charset=x")), "application/json");
    assert_eq!(content_type_of(None), "");
    assert!(is_publish_type(Some("application/vnd.vershion.publish")));
    assert!(!is_publish_type(Some("application/json")));
}
