//! Tests for `publishrun.rs`: the account lock (one holder, expiry, erased accounts), the heavy slot (one at a time, a bounded queue, 503 busy with Retry-After),
//! and the post-heavy cost bound.
use super::*;
use crate::db;
use rusqlite::Connection;
use std::path::Path;

fn open() -> (Connection, tempfile::TempDir) {
    let dir = tempfile::tempdir().unwrap();
    let mut c = db::open(&dir.path().join("t.db")).unwrap();
    db::migrate(&mut c, &Path::new(env!("CARGO_MANIFEST_DIR")).join("../worker/migrations")).unwrap();
    c.execute("INSERT INTO users (id, email, display_name, trust, created_at) VALUES ('a', 'a@x.org', 'a', 'teacher', 1)", []).unwrap();
    (c, dir)
}

#[test]
fn one_request_holds_the_account_lock_until_it_releases_or_it_expires() {
    let (c, _d) = open();
    let t1 = acquire_account_lock(&PoolCtx::new(&c, 1_000), "a").unwrap().expect("first takes it");
    assert!(acquire_account_lock(&PoolCtx::new(&c, 2_000), "a").unwrap().is_none(), "held and alive");
    release_account_lock(&PoolCtx::new(&c, 2_000), "a", "not-the-token").unwrap();
    assert!(acquire_account_lock(&PoolCtx::new(&c, 2_000), "a").unwrap().is_none(), "only the holder's token releases");
    release_account_lock(&PoolCtx::new(&c, 2_000), "a", &t1).unwrap();
    let t2 = acquire_account_lock(&PoolCtx::new(&c, 3_000), "a").unwrap().expect("free again");
    assert_ne!(t1, t2);
    // An expired lock is taken over.
    let t3 = acquire_account_lock(&PoolCtx::new(&c, 3_000 + ACCOUNT_LOCK_MS + 1), "a").unwrap();
    assert!(t3.is_some());
}

#[test]
fn a_refresh_keeps_the_lock_alive_and_an_erased_account_cannot_take_one() {
    let (c, _d) = open();
    let t = acquire_account_lock(&PoolCtx::new(&c, 1_000), "a").unwrap().unwrap();
    refresh_account_lock(&PoolCtx::new(&c, 50_000), "a", &t).unwrap();
    assert!(acquire_account_lock(&PoolCtx::new(&c, 1_000 + ACCOUNT_LOCK_MS + 1), "a").unwrap().is_none(), "refreshed past the original expiry");
    c.execute("UPDATE users SET deleted_at = 5, email = NULL, pw_hash = NULL, pw_params = NULL, display_name = NULL, profile = NULL WHERE id = 'a'", []).unwrap();
    c.execute("DELETE FROM pool_locks", []).unwrap();
    assert!(acquire_account_lock(&PoolCtx::new(&c, 9_000_000), "a").unwrap().is_none());
}

#[tokio::test]
async fn the_heavy_slot_serves_one_at_a_time_and_the_queue_is_bounded() {
    let slots = HeavySlots::default();
    let first = slots.acquire(Duration::from_millis(50), || {}).await.unwrap();
    assert!(slots.try_acquire().is_none(), "taken");
    // A waiter that is never served times out with 503 busy and Retry-After.
    let e = slots.acquire(Duration::from_millis(30), || {}).await.err().unwrap();
    assert_eq!((e.status, e.code.as_str()), (503, "busy"));
    assert_eq!(e.headers, vec![("Retry-After".to_string(), "10".to_string())]);
    // Waiters in turn: when the holder drops, the first waiter gets it.
    let s2 = slots.clone();
    let w = tokio::spawn(async move { s2.acquire(Duration::from_secs(5), || {}).await.is_ok() });
    tokio::time::sleep(Duration::from_millis(30)).await;
    assert!(slots.try_acquire().is_none(), "a reader never jumps the queue");
    drop(first);
    assert!(w.await.unwrap());
    assert!(slots.try_acquire().is_some(), "free again once the waiter finished");
}

#[tokio::test]
async fn a_full_queue_turns_the_next_publish_away_at_once() {
    let slots = HeavySlots::default();
    let _holder = slots.acquire(Duration::from_millis(10), || {}).await.unwrap();
    let mut waiters = vec![];
    for _ in 0..HEAVY_QUEUE_MAX {
        let s = slots.clone();
        waiters.push(tokio::spawn(async move { s.acquire(Duration::from_millis(400), || {}).await.is_ok() }));
    }
    tokio::time::sleep(Duration::from_millis(60)).await;
    let t = std::time::Instant::now();
    let e = slots.acquire(Duration::from_secs(5), || {}).await.err().unwrap();
    assert_eq!(e.status, 503);
    assert!(t.elapsed() < Duration::from_millis(200), "refused without waiting");
    for w in waiters {
        assert!(!w.await.unwrap(), "the holder never let go: each waiter timed out");
    }
}

#[test]
fn the_post_heavy_bound_sums_each_measured_part() {
    let s = Shape { bodies: 1000, meta_bytes: 10, questions: 2, parts: 3, words: 100, word_chars: 500 };
    assert_eq!(post_heavy_bytes(&s), 1000 + 10 * 16 + 2 * 12 * 1024 + 100 * 224 + 500 * 8 + 3 * 2 * 1024);
}
