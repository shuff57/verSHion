//! Unfinished streamed reads (worker/src/pool/read.ts `takeReadSlots`, `readHold`): per account 4; per account within one
//! address 2; per address 8, so one account behind a school's NAT can never take all of an address's slots from the other
//! teachers on it. The Worker counted per isolate; one process counts for everyone, so the caps now hold as stated.
//! The Worker's isolate byte reservation (`reserveBytes(READ_COST)`) has no counterpart: blobs stream from disk here.
use super::ctx::PoolError;
use super::sqlfrag::hex;
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

pub const OPEN_READS_MAX: usize = 4;
pub const OPEN_READS_PER_ACCOUNT_ADDRESS: usize = 2;
pub const OPEN_READS_PER_ADDRESS: usize = 8;
/// A streamed read nobody pulls from for this long is cancelled.
pub const READ_IDLE_MS: u64 = 10_000;
/// ... and a stream slower than READ_MIN_RATE ends here; a faster one may live READ_LIFETIME_FACTOR times longer.
pub const READ_LIFETIME_MS: u64 = 60_000;
pub const READ_MIN_RATE: u64 = 20 * 1024;
pub const READ_LIFETIME_FACTOR: u64 = 5;

#[derive(Clone, Default)]
pub struct OpenReads {
    counts: Arc<Mutex<HashMap<String, usize>>>,
}

/// Held for the life of a streamed read; dropping it (or `release`) frees the slots, once.
#[derive(Debug)]
pub struct ReadSlots {
    counts: Arc<Mutex<HashMap<String, usize>>>,
    keys: Vec<String>,
}

impl OpenReads {
    pub fn new() -> Self {
        Self::default()
    }

    /// Takes a streamed-read slot for each (key, cap), or 429 `too_many_open_reads` with `Retry-After: 2`.
    pub fn take(&self, keys: &[(String, usize)]) -> Result<ReadSlots, PoolError> {
        let mut m = self.counts.lock().unwrap();
        if keys.iter().any(|(k, cap)| m.get(k).copied().unwrap_or(0) >= *cap) {
            return Err(PoolError::new(429, "too_many_open_reads")
                .with("message", serde_json::json!("Too many downloads are open at once (from your account or your network). Retry in a moment."))
                .header("Retry-After", "2"));
        }
        for (k, _) in keys {
            *m.entry(k.clone()).or_insert(0) += 1;
        }
        Ok(ReadSlots { counts: self.counts.clone(), keys: keys.iter().map(|(k, _)| k.clone()).collect() })
    }

    /// The per-account, per-account-and-address and per-address slots of one read; `addr` is the address bucket of the client.
    pub fn hold(&self, me: &str, addr: &str) -> Result<ReadSlots, PoolError> {
        self.take(&[
            (format!("u:{me}"), OPEN_READS_MAX),
            (format!("ua:{me}@{addr}"), OPEN_READS_PER_ACCOUNT_ADDRESS),
            (format!("i:{addr}"), OPEN_READS_PER_ADDRESS),
        ])
    }

    pub fn open(&self, key: &str) -> usize {
        self.counts.lock().unwrap().get(key).copied().unwrap_or(0)
    }
}

impl Drop for ReadSlots {
    fn drop(&mut self) {
        let mut m = self.counts.lock().unwrap();
        for k in self.keys.drain(..) {
            match m.get(&k).copied().unwrap_or(1) {
                0 | 1 => {
                    m.remove(&k);
                }
                n => {
                    m.insert(k, n - 1);
                }
            }
        }
    }
}

impl ReadSlots {
    /// Frees the slots now (dropping does the same, once).
    pub fn release(self) {}
}

/// An ETag for what a read serves: a hash of the head (which names the served publish), nothing more.
pub fn etag_of(head: &str) -> String {
    format!("\"{}\"", &hex(&Sha256::digest(head.as_bytes()))[..32])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_account_may_hold_four_reads_and_the_fifth_is_refused_with_a_retry_hint() {
        let r = OpenReads::new();
        let held: Vec<ReadSlots> = (0..4).map(|i| r.hold("u1", &format!("addr{i}")).unwrap()).collect();
        let e = r.hold("u1", "addr9").unwrap_err();
        assert_eq!((e.status, e.code.as_str()), (429, "too_many_open_reads"));
        assert_eq!(e.headers, vec![("Retry-After".to_string(), "2".to_string())]);
        assert_eq!(r.open("u:u1"), 4);
        drop(held);
        assert_eq!(r.open("u:u1"), 0);
        r.hold("u1", "addr9").unwrap();
    }

    #[test]
    fn one_account_gets_two_per_address_and_one_address_eight_in_all() {
        let r = OpenReads::new();
        let _a = r.hold("u1", "net").unwrap();
        let _b = r.hold("u1", "net").unwrap();
        assert_eq!(r.hold("u1", "net").unwrap_err().code, "too_many_open_reads", "a third from the same account and address");
        // Others behind the same address still get theirs until the address is full (8).
        let mut held = vec![];
        for u in ["u2", "u2", "u3", "u3", "u4", "u4"] {
            held.push(r.hold(u, "net").unwrap());
        }
        assert_eq!(r.open("i:net"), 8);
        assert_eq!(r.hold("u5", "net").unwrap_err().code, "too_many_open_reads", "the address is full");
        assert!(r.hold("u5", "elsewhere").is_ok());
    }

    #[test]
    fn a_refused_take_counts_nothing_and_release_is_idempotent_by_construction() {
        let r = OpenReads::new();
        let keys = vec![("a".to_string(), 1usize), ("b".to_string(), 1usize)];
        let g = r.take(&keys).unwrap();
        assert!(r.take(&keys).is_err());
        assert_eq!((r.open("a"), r.open("b")), (1, 1));
        g.release();
        assert_eq!((r.open("a"), r.open("b")), (0, 0));
        let _again = r.take(&keys).unwrap();
    }

    #[test]
    fn the_etag_is_a_quoted_32_hex_prefix_of_the_sha256_of_the_head() {
        // sha256("abc") = ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad
        assert_eq!(etag_of("abc"), "\"ba7816bf8f01cfea414140de5dae2223\"");
    }
}
