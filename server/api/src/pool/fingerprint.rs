//! Keyed fingerprints (worker/src/pool/shared.ts, "keyed fingerprints", and core.ts `deriveHashKey`). Every fingerprint of
//! question content (licence-sense hashes, key-free facts, block keys, loose stem hashes) is HMAC-SHA256 under a subkey of
//! the pool hash key, one per purpose, never a bare SHA-256: a full-form fingerprint covers a hidden answer, and unkeyed it
//! would let anyone holding a row test a guessed answer.
//!
//! These values are STORED, and the cutover copies the Worker's rows: every function here must produce the Worker's bytes.
//! The tests carry values made by WebCrypto under the same inputs.
use super::ctx::{PoolCtx, PoolError, PoolNotice, Res};
use super::sqlfrag::hex;
use hkdf::Hkdf;
use hmac::{Hmac, Mac};
use rusqlite::{OptionalExtension, params};
use sha2::Sha256;
use std::collections::HashMap;

type HmacSha256 = Hmac<Sha256>;

pub const PURPOSES: [&str; 6] = ["near", "keyfree", "block", "stem", "sketch-v1", "key-id"];

fn hmac(key: &[u8], data: &[u8]) -> [u8; 32] {
    let mut m = HmacSha256::new_from_slice(key).expect("HMAC takes any key length");
    m.update(data);
    m.finalize().into_bytes().into()
}

/// The derived full-hash key from the POOL_HASH_KEY secret: HKDF-SHA256, salt "vershion-pool/v1", info
/// "pool-hash-key:<purpose>", 32 bytes, hex. None when the secret is missing or under 32 characters (the pool then refuses
/// every request, never hashing unkeyed).
pub fn derive_hash_key(secret: &str, purpose: &str) -> Option<String> {
    if secret.chars().count() < 32 {
        return None;
    }
    let mut out = [0u8; 32];
    Hkdf::<Sha256>::new(Some(b"vershion-pool/v1"), secret.as_bytes()).expand(format!("pool-hash-key:{purpose}").as_bytes(), &mut out).ok()?;
    Some(hex(&out))
}

#[derive(Clone)]
pub struct Fingerprinter {
    subkeys: HashMap<&'static str, [u8; 32]>,
}

impl Fingerprinter {
    /// From the root key in hex (the output of `derive_hash_key(secret, "full-hash")`).
    pub fn from_root_hex(root: &str) -> Option<Self> {
        if root.len() % 2 != 0 || !root.bytes().all(|b| b.is_ascii_hexdigit()) {
            return None;
        }
        let bytes: Vec<u8> = (0..root.len() / 2).map(|i| u8::from_str_radix(&root[2 * i..2 * i + 2], 16).unwrap()).collect();
        let subkeys = PURPOSES.iter().map(|p| (*p, hmac(&bytes, format!("vershion-pool/fingerprint/{p}").as_bytes()))).collect();
        Some(Fingerprinter { subkeys })
    }
    pub fn from_secret(secret: &str) -> Option<Self> {
        Self::from_root_hex(&derive_hash_key(secret, "full-hash")?)
    }

    /// HMAC-SHA256 of `text` under the purpose's subkey, hex.
    pub fn fingerprint(&self, purpose: &str, text: &str) -> String {
        let key = self.subkeys.get(purpose).unwrap_or_else(|| panic!("unknown fingerprint purpose {purpose}"));
        hex(&hmac(key, text.as_bytes()))
    }

    /// The keyed token sketch of purged, erased or private content: 64 bits, as an UNORDERED, deduplicated, sorted set (no
    /// positions, no first-appearance order). Without the key nothing in it can be inverted or tested against a dictionary;
    /// rotating the key makes old sketches unmatchable.
    pub fn keyed_one(&self, prefix: &str, text: &str, memo: &mut HashMap<String, String>) -> String {
        let k = format!("{prefix}{text}");
        memo.entry(k.clone()).or_insert_with(|| self.fingerprint("sketch-v1", &k)[..16].to_string()).clone()
    }
    pub fn keyed_set(&self, tokens: &[String], memo: &mut HashMap<String, String>, prefix: &str) -> Vec<String> {
        let mut out: Vec<String> = tokens.iter().map(|t| self.keyed_one(prefix, t, memo)).collect();
        out.sort();
        out.dedup();
        out
    }
    /// The keyed sketch of a body (its kind tag first, then the keyed set of its words: same_body needs the kind).
    pub fn keyed_body(&self, body: &[String], memo: &mut HashMap<String, String>) -> Vec<String> {
        if body.is_empty() {
            return vec![];
        }
        let mut out = self.keyed_set(&body[..1], memo, "k:");
        out.extend(self.keyed_set(&body[1..], memo, "t:"));
        out
    }
    /// The keyed MinHash bands (each band tuple under the same key): what ghosts and privacy rows keep.
    pub fn keyed_bands(&self, bands: &[String], memo: &mut HashMap<String, String>) -> Vec<String> {
        self.keyed_set(bands, memo, "b:")
    }

    /// The id of the pool hash key in use (an HMAC under its own subkey, 64 bits). Every identity hash in the pool is made
    /// under that key, so the key is PERMANENT.
    pub fn pool_key_id(&self) -> String {
        self.fingerprint("key-id", "vershion-pool/key-id/v1")[..16].to_string()
    }
}

/// What the stored data says about the key in use.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeyVerdict {
    /// No pool data at all (nothing to protect).
    Empty,
    Match,
    Mismatch,
    /// There is data but no readable body to check against.
    Unknown,
}

fn key_mismatch(x: &PoolCtx, kid: &str, stored: &str) -> Res<()> {
    let first = x.conn.execute("INSERT OR IGNORE INTO pool_kv (key, value) VALUES (?1, ?2)", params![format!("key_mismatch_alerted:{kid}"), x.now.to_string()])?;
    if first == 1 {
        x.send(PoolNotice::KeyMismatch { key_id: kid.into(), stored_key_id: stored.into() });
    }
    Err(PoolError::new(503, "pool_key_mismatch").with("message", serde_json::json!("The pool's hash key is not the one its data was made with. The key cannot be rotated.")).into())
}

/// The key in use must be the key the data was made under: the first use records its id in pool_kv; a request under any
/// other key fails closed (503 pool_key_mismatch, one admin alert) and changes nothing. A pool populated before the id was
/// recorded is verified first (`verdict`: it recomputes one retained publish's keyed hashes), never pinned on a guess: a
/// wrong key is refused and NOT pinned, and an unverifiable populated pool is not pinned either (an unpinned key locks
/// nobody out; a wrongly pinned one locks the real key out for good). Returns whether the key is now confirmed.
pub fn assert_pool_key(x: &PoolCtx, fp: &Fingerprinter, verdict: impl FnOnce() -> KeyVerdict) -> Res<bool> {
    let kid = fp.pool_key_id();
    let read = || -> rusqlite::Result<Option<String>> { x.conn.query_row("SELECT value FROM pool_kv WHERE key = 'pool_key_id'", [], |r| r.get(0)).optional() };
    let mut stored = read()?;
    if stored.is_none() {
        match verdict() {
            KeyVerdict::Mismatch => {
                key_mismatch(x, &kid, "")?;
            }
            KeyVerdict::Unknown => return Ok(false),
            _ => {}
        }
        x.conn.execute("INSERT OR IGNORE INTO pool_kv (key, value) VALUES ('pool_key_id', ?1)", [&kid])?;
        stored = read()?;
    }
    if stored.as_deref() == Some(kid.as_str()) {
        return Ok(true);
    }
    key_mismatch(x, &kid, stored.as_deref().unwrap_or(""))?;
    Ok(false)
}

/// While items from before the licence-sense hashes still wait for the cron backfill, no publish or import may create
/// evidence against them: 503 pool_migrating (the cron finishes in bounded batches).
pub fn assert_not_migrating(x: &PoolCtx) -> Res<()> {
    let waiting = x.conn.query_row("SELECT 1 FROM items WHERE fold_rev = 0 AND current_publish_id IS NOT NULL AND purged_at IS NULL LIMIT 1", [], |_| Ok(())).optional()?.is_some();
    if waiting {
        return Err(PoolError::new(503, "pool_migrating").with("message", serde_json::json!("The shared pool is finishing an upgrade. Try again in a little while.")).header("Retry-After", "600").into());
    }
    Ok(())
}

#[cfg(test)]
#[path = "fingerprint_tests.rs"]
mod tests;
