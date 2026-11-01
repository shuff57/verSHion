//! The run-time guards around a publish (worker/src/pool/publish.ts): the per-account lock, the heavy-work slot, and the cost bound a publish holds while it
//! parses and validates.
use super::ctx::{PoolCtx, PoolError};
use super::sqlfrag::{alive_sql, new_id};
use rusqlite::{OptionalExtension, params};
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;
use tokio::sync::{OwnedSemaphorePermit, Semaphore};

/// A publish's account lock lives this long unless it is refreshed.
pub const ACCOUNT_LOCK_MS: i64 = 60_000;
/// How long a publish body may take to arrive.
pub const BODY_DEADLINE_MS: i64 = 15_000;
/// Concurrent publishes that may parse, validate and strip at once.
pub const HEAVY_CONCURRENCY: usize = 1;
/// Publishes allowed to wait for it; beyond that, 503 with Retry-After.
pub const HEAVY_QUEUE_MAX: usize = 4;
/// How long a publish may wait for the heavy slot (then 503 busy).
pub const HEAVY_QUEUE_WAIT_MS: u64 = 20_000;
/// Memory a publish is charged per byte of its bodies.
pub const PUBLISH_COST_PER_BYTE: i64 = 16;
pub const POST_HEAVY_BYTES_PER_QUESTION: i64 = 12 * 1024;
pub const POST_HEAVY_BYTES_PER_WORD: i64 = 224;
pub const POST_HEAVY_BYTES_PER_WORD_CHAR: i64 = 8;
pub const POST_HEAVY_BYTES_PER_PART: i64 = 2 * 1024;

/// The bytes a publish holds after the heavy phase, from the measured shape of its payload (bodies, meta, questions, parts, words, word characters).
pub struct Shape {
    pub bodies: i64,
    pub meta_bytes: i64,
    pub questions: i64,
    pub parts: i64,
    pub words: i64,
    pub word_chars: i64,
}
pub fn post_heavy_bytes(o: &Shape) -> i64 {
    o.bodies
        + o.meta_bytes * PUBLISH_COST_PER_BYTE
        + o.questions * POST_HEAVY_BYTES_PER_QUESTION
        + o.words * POST_HEAVY_BYTES_PER_WORD
        + o.word_chars * POST_HEAVY_BYTES_PER_WORD_CHAR
        + o.parts * POST_HEAVY_BYTES_PER_PART
}

fn busy() -> PoolError {
    PoolError::new(503, "busy").with("message", "The server is busy with large publishes; try again shortly.".into()).header("Retry-After", "10")
}

/// The process's heavy-work slots: at most HEAVY_CONCURRENCY at once, HEAVY_QUEUE_MAX waiting, first come first served.
#[derive(Clone)]
pub struct HeavySlots {
    sem: Arc<Semaphore>,
    waiting: Arc<AtomicUsize>,
}
impl Default for HeavySlots {
    fn default() -> Self {
        HeavySlots { sem: Arc::new(Semaphore::new(HEAVY_CONCURRENCY)), waiting: Arc::new(AtomicUsize::new(0)) }
    }
}
/// Held while a publish is in its heavy phase; dropping it frees the slot for the next in line.
pub struct Heavy(#[allow(dead_code)] OwnedSemaphorePermit);

impl HeavySlots {
    /// Waits for the slot, at most `wait` (503 busy after it, or at once when the queue is full), calling `beat` every few seconds meanwhile so the account
    /// lock lives exactly as long as the request.
    pub async fn acquire(&self, wait: Duration, mut beat: impl FnMut()) -> Result<Heavy, PoolError> {
        if let Ok(p) = self.sem.clone().try_acquire_owned() {
            return Ok(Heavy(p));
        }
        if self.waiting.fetch_add(1, Ordering::SeqCst) >= HEAVY_QUEUE_MAX {
            self.waiting.fetch_sub(1, Ordering::SeqCst);
            return Err(busy());
        }
        let started = tokio::time::Instant::now();
        let every = Duration::from_millis(((ACCOUNT_LOCK_MS / 3) as u64).max(1000));
        let turn = self.sem.clone().acquire_owned();
        tokio::pin!(turn);
        let res = loop {
            let left = wait.saturating_sub(started.elapsed());
            if left.is_zero() {
                break Err(busy());
            }
            match tokio::time::timeout(left.min(every), &mut turn).await {
                Ok(Ok(p)) => break Ok(Heavy(p)),
                Ok(Err(_)) => break Err(busy()),
                Err(_) => beat(),
            }
        };
        self.waiting.fetch_sub(1, Ordering::SeqCst);
        res
    }

    /// The slot if it is free NOW, else None. It never waits and never takes a place in the queue (so it can never turn a publish away): for work a read may
    /// skip when the server is busy.
    pub fn try_acquire(&self) -> Option<Heavy> {
        if self.waiting.load(Ordering::SeqCst) > 0 {
            return None;
        }
        self.sem.clone().try_acquire_owned().ok().map(Heavy)
    }
}

/// The account's publish lock: the token if taken, None when another request holds it (still alive) or the account is erased.
pub fn acquire_account_lock(x: &PoolCtx, me: &str) -> rusqlite::Result<Option<String>> {
    let token = new_id();
    let got: Option<String> = x
        .conn
        .query_row(
            &format!(
                "INSERT INTO pool_locks (user_id, token, until) SELECT ?1, ?2, ?3 WHERE {}
       ON CONFLICT (user_id) DO UPDATE SET token = excluded.token, until = excluded.until WHERE pool_locks.until < ?4
       RETURNING token",
                alive_sql("?1")
            ),
            params![me, token, x.now + ACCOUNT_LOCK_MS, x.now],
            |r| r.get(0),
        )
        .optional()?;
    Ok(got.filter(|t| *t == token))
}

pub fn release_account_lock(x: &PoolCtx, me: &str, token: &str) -> rusqlite::Result<()> {
    x.conn.execute("DELETE FROM pool_locks WHERE user_id = ?1 AND token = ?2", params![me, token])?;
    Ok(())
}

/// Keeps the account lock alive while the request is: refreshed at every phase.
pub fn refresh_account_lock(x: &PoolCtx, me: &str, token: &str) -> rusqlite::Result<()> {
    x.conn.execute("UPDATE pool_locks SET until = ?3 WHERE user_id = ?1 AND token = ?2", params![me, token, x.now + ACCOUNT_LOCK_MS])?;
    Ok(())
}

#[cfg(test)]
#[path = "publishrun_tests.rs"]
mod tests;
