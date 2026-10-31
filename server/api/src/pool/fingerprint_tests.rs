//! Tests for `fingerprint.rs`. The expected values were made by WebCrypto (HKDF and HMAC-SHA256, as the Worker computes
//! them) from the secret below, so a divergence here is a divergence from every stored hash.
use super::*;
use crate::db;
use std::path::Path;

const SECRET: &str = "test-secret-0123456789abcdef0123456789abcdef";
const ROOT: &str = "7f757fb737085dcaaad0727deb89a14c18141681dc56be0a215a592c715f8e3b";

#[test]
fn the_derived_root_key_and_every_purpose_match_webcrypto() {
    assert_eq!(derive_hash_key(SECRET, "full-hash").as_deref(), Some(ROOT));
    assert_eq!(derive_hash_key("too short", "full-hash"), None);
    assert_eq!(derive_hash_key(&"x".repeat(31), "full-hash"), None);
    assert!(derive_hash_key(&"x".repeat(32), "full-hash").is_some());
    let fp = Fingerprinter::from_secret(SECRET).unwrap();
    for (purpose, want) in [
        ("near", "dded7157960382b3ba3c672e07e9bb1997e3bca6142c180d417c268fe1af77f0"),
        ("keyfree", "4fd40711a07dc3a8cb9de3ec6ff89bf192f8701e61c984cd341f0f7b03da8b75"),
        ("block", "07626812e02d55390802908c435ce2031751ef11600b73bbdb38843c2c4d716c"),
        ("stem", "4e2cce1e10b58ca307890d3bf7a26d364ae432a35ab7090cb0caf7664a7ab3f4"),
        ("sketch-v1", "f8eb23b88bb4f913a80ef9ffe329fcbc7ce59787ab7e5ffa11218d73ba6bda4b"),
        ("key-id", "ccd7bf660c1874553bdfea9e83f94c5038e1fbe65cdd7d1281e94f8d6537827b"),
    ] {
        assert_eq!(fp.fingerprint(purpose, "hello wörld"), want, "{purpose}");
    }
    assert_eq!(fp.pool_key_id(), "94bed39911cfb493");
    let mut memo = HashMap::new();
    assert_eq!(fp.keyed_one("t:", "cell", &mut memo), "13e9c07bc6d9acb6");
    assert_eq!(memo.len(), 1, "memoised under prefix + text");
    assert!(Fingerprinter::from_root_hex("xyz").is_none() && Fingerprinter::from_root_hex("abc").is_none());
    assert_eq!(Fingerprinter::from_root_hex(ROOT).unwrap().pool_key_id(), "94bed39911cfb493");
}

#[test]
fn keyed_sets_are_unordered_deduplicated_and_sorted_and_bodies_keep_the_kind_first() {
    let fp = Fingerprinter::from_secret(SECRET).unwrap();
    let mut memo = HashMap::new();
    let s = |v: &[&str]| v.iter().map(|x| x.to_string()).collect::<Vec<_>>();
    let a = fp.keyed_set(&s(&["b", "a", "b", "c"]), &mut memo, "t:");
    let b = fp.keyed_set(&s(&["c", "b", "a"]), &mut memo, "t:");
    assert_eq!(a, b);
    assert_eq!(a.len(), 3);
    assert!(a.windows(2).all(|w| w[0] < w[1]) && a.iter().all(|h| h.len() == 16));
    let body = fp.keyed_body(&s(&["mc", "x", "y", "x"]), &mut memo);
    assert_eq!(body[0], fp.keyed_one("k:", "mc", &mut memo), "the kind first");
    assert_eq!(body[1..].to_vec(), fp.keyed_set(&s(&["x", "y"]), &mut memo, "t:"));
    assert!(fp.keyed_body(&[], &mut memo).is_empty());
    assert_ne!(fp.keyed_bands(&s(&["a"]), &mut memo), fp.keyed_set(&s(&["a"]), &mut memo, "t:"), "bands use their own prefix");
    let other = Fingerprinter::from_secret(&"y".repeat(40)).unwrap();
    assert_ne!(other.keyed_one("t:", "cell", &mut HashMap::new()), "13e9c07bc6d9acb6", "another key, another sketch");
}

fn open() -> (rusqlite::Connection, tempfile::TempDir) {
    let dir = tempfile::tempdir().unwrap();
    let mut conn = db::open(&dir.path().join("t.db")).unwrap();
    db::migrate(&mut conn, &Path::new(env!("CARGO_MANIFEST_DIR")).join("../worker/migrations")).unwrap();
    (conn, dir)
}
fn pool_err(f: super::super::ctx::Fail) -> PoolError {
    match f {
        super::super::ctx::Fail::Pool(p) => p,
        super::super::ctx::Fail::Db(e) => panic!("db: {e}"),
    }
}

#[test]
fn the_first_key_is_pinned_and_any_other_key_fails_closed_with_one_alert() {
    let (c, _d) = open();
    let x = PoolCtx::new(&c, 1_000);
    let fp = Fingerprinter::from_secret(SECRET).unwrap();
    assert!(assert_pool_key(&x, &fp, || KeyVerdict::Empty).unwrap());
    let pinned: String = c.query_row("SELECT value FROM pool_kv WHERE key = 'pool_key_id'", [], |r| r.get(0)).unwrap();
    assert_eq!(pinned, "94bed39911cfb493");
    assert!(assert_pool_key(&x, &fp, || panic!("no verdict once pinned")).unwrap());
    let other = Fingerprinter::from_secret(&"z".repeat(40)).unwrap();
    let e = pool_err(assert_pool_key(&x, &other, || KeyVerdict::Match).unwrap_err());
    assert_eq!((e.status, e.code.as_str()), (503, "pool_key_mismatch"));
    assert_eq!(x.take_notices(), vec![PoolNotice::KeyMismatch { key_id: other.pool_key_id(), stored_key_id: pinned.clone() }]);
    assert_eq!(pool_err(assert_pool_key(&x, &other, || KeyVerdict::Match).unwrap_err()).status, 503);
    assert!(x.take_notices().is_empty(), "alerted once per key");
    let still: String = c.query_row("SELECT value FROM pool_kv WHERE key = 'pool_key_id'", [], |r| r.get(0)).unwrap();
    assert_eq!(still, pinned, "a wrong key never replaces the pinned one");
}

#[test]
fn a_populated_pool_with_no_recorded_key_is_verified_never_pinned_on_a_guess() {
    let (c, _d) = open();
    let x = PoolCtx::new(&c, 1_000);
    let fp = Fingerprinter::from_secret(SECRET).unwrap();
    // The data says this is another key: refused and NOT pinned, so the right key still works later.
    assert_eq!(pool_err(assert_pool_key(&x, &fp, || KeyVerdict::Mismatch).unwrap_err()).code, "pool_key_mismatch");
    assert_eq!(c.query_row("SELECT COUNT(*) FROM pool_kv WHERE key = 'pool_key_id'", [], |r| r.get::<_, i64>(0)).unwrap(), 0);
    // Unverifiable data: allowed through but not pinned.
    assert!(!assert_pool_key(&x, &fp, || KeyVerdict::Unknown).unwrap());
    assert_eq!(c.query_row("SELECT COUNT(*) FROM pool_kv WHERE key = 'pool_key_id'", [], |r| r.get::<_, i64>(0)).unwrap(), 0);
    // Verified: pinned.
    assert!(assert_pool_key(&x, &fp, || KeyVerdict::Match).unwrap());
    assert_eq!(c.query_row("SELECT COUNT(*) FROM pool_kv WHERE key = 'pool_key_id'", [], |r| r.get::<_, i64>(0)).unwrap(), 1);
}

#[test]
fn publishes_wait_while_old_items_are_unbackfilled() {
    let (c, _d) = open();
    let x = PoolCtx::new(&c, 1_000);
    assert_not_migrating(&x).unwrap();
    c.execute("INSERT INTO users (id, email, trust, created_at) VALUES ('a', 'a@x.org', 'teacher', 1)", []).unwrap();
    c.execute("INSERT INTO items (id, author_id, kind, visibility, current_publish_id, license, created_at, updated_at, fold_rev) VALUES ('i', 'a', 'bank', 'public', 'p', 'CC0-1.0', 1, 1, 0)", []).unwrap();
    let e = pool_err(assert_not_migrating(&x).unwrap_err());
    assert_eq!((e.status, e.code.as_str()), (503, "pool_migrating"));
    assert_eq!(e.headers, vec![("Retry-After".to_string(), "600".to_string())]);
    c.execute("UPDATE items SET fold_rev = 1", []).unwrap();
    assert_not_migrating(&x).unwrap();
}
