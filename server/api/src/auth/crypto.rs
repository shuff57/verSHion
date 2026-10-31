//! Small crypto helpers (port of auth/src/crypto.ts): base64 codecs, random tokens, SHA-256, constant-time
//! comparison, and the per-purpose HMAC keys derived from the one server secret with HKDF.
use hkdf::Hkdf;
use hmac::{Hmac, Mac};
use sha2::{Digest, Sha256};

const URL_ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_";
const STD_ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

fn encode(bytes: &[u8], alphabet: &[u8; 64]) -> String {
    let mut out = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for chunk in bytes.chunks(3) {
        let n = (chunk[0] as u32) << 16 | (*chunk.get(1).unwrap_or(&0) as u32) << 8 | *chunk.get(2).unwrap_or(&0) as u32;
        for i in 0..=chunk.len() {
            out.push(alphabet[(n >> (18 - 6 * i) & 63) as usize] as char);
        }
    }
    out
}

/// Unpadded decode. Like `atob`, a length of 1 mod 4 is refused and leftover bits are ignored.
fn decode(s: &str, alphabet: &[u8; 64]) -> Option<Vec<u8>> {
    if s.len() % 4 == 1 {
        return None;
    }
    let mut out = Vec::with_capacity(s.len() * 3 / 4);
    let (mut acc, mut bits) = (0u32, 0);
    for b in s.bytes() {
        let v = alphabet.iter().position(|a| *a == b)? as u32;
        acc = acc << 6 | v;
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            out.push((acc >> bits & 0xff) as u8);
        }
    }
    Some(out)
}

/// URL-safe alphabet, unpadded.
pub fn b64url(bytes: &[u8]) -> String {
    encode(bytes, URL_ALPHABET)
}
pub fn from_b64url(s: &str) -> Option<Vec<u8>> {
    decode(s, URL_ALPHABET)
}
/// Standard alphabet, unpadded: the encoding PHC strings use for salt and hash.
pub fn b64(bytes: &[u8]) -> String {
    encode(bytes, STD_ALPHABET)
}
pub fn from_b64(s: &str) -> Option<Vec<u8>> {
    decode(s, STD_ALPHABET)
}

pub fn random_bytes(n: usize) -> Vec<u8> {
    let mut v = vec![0u8; n];
    getrandom::getrandom(&mut v).expect("the operating system has randomness");
    v
}

/// A random (version 4) UUID, lower case, as the Worker's `crypto.randomUUID()` writes it.
pub fn uuid_v4() -> String {
    let mut b = random_bytes(16);
    b[6] = b[6] & 0x0f | 0x40;
    b[8] = b[8] & 0x3f | 0x80;
    let h: String = b.iter().map(|x| format!("{x:02x}")).collect();
    format!("{}-{}-{}-{}-{}", &h[0..8], &h[8..12], &h[12..16], &h[16..20], &h[20..32])
}

/// 256-bit random token, base64url (43 chars).
pub fn random_token() -> String {
    b64url(&random_bytes(32))
}

pub fn is_token(s: &str) -> bool {
    s.len() == 43 && s.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
}

pub fn sha256(s: &str) -> String {
    b64url(&Sha256::digest(s.as_bytes()))
}

/// Constant-time comparison of two byte slices. Length is not secret.
pub fn timing_safe_equal(a: &[u8], b: &[u8]) -> bool {
    a.len() == b.len() && a.iter().zip(b).fold(0u8, |d, (x, y)| d | (x ^ y)) == 0
}

pub type Key = [u8; 32];

pub struct Keys {
    /// Signs access tokens (HS256).
    pub access: Key,
    /// Derives a refresh token's successor: HMAC(refresh, predecessor).
    pub refresh: Key,
    /// Salted hash of an email address kept on a deleted account's tombstone.
    pub email: Key,
    /// Keys rate-limit counters so they never hold a raw IP or email.
    pub limit: Key,
    /// Signs known-device tokens.
    pub device: Key,
    /// Signs short-lived sudo tokens (re-entered password).
    pub sudo: Key,
}

/// Independent HMAC-SHA256 keys from one server secret (HKDF), so a single secret never serves two purposes.
pub fn derive_keys(secret: &str) -> Keys {
    let hk = Hkdf::<Sha256>::new(Some(b"d1-auth/v1"), secret.as_bytes());
    let derive = |info: &str| -> Key {
        let mut k = [0u8; 32];
        hk.expand(info.as_bytes(), &mut k).expect("32 bytes is a valid HKDF length");
        k
    };
    Keys { access: derive("access-token"), refresh: derive("refresh-successor"), email: derive("tombstone-email"), limit: derive("rate-limit-key"), device: derive("known-device"), sudo: derive("sudo") }
}

pub fn hmac_raw(key: &Key, data: &[u8]) -> Vec<u8> {
    let mut mac = Hmac::<Sha256>::new_from_slice(key).expect("hmac takes any key length");
    mac.update(data);
    mac.finalize().into_bytes().to_vec()
}

/// HMAC-SHA256 of `data`, base64url.
pub fn hmac(key: &Key, data: &str) -> String {
    b64url(&hmac_raw(key, data.as_bytes()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn base64_matches_the_known_vectors_and_round_trips() {
        for (raw, std, url) in [(&b""[..], "", ""), (b"f", "Zg", "Zg"), (b"fo", "Zm8", "Zm8"), (b"foo", "Zm9v", "Zm9v"), (&[0xfb, 0xff, 0xfe][..], "+//+", "-__-")] {
            assert_eq!(b64(raw), std);
            assert_eq!(b64url(raw), url);
            assert_eq!(from_b64(std).unwrap(), raw);
            assert_eq!(from_b64url(url).unwrap(), raw);
        }
        let all: Vec<u8> = (0..=255).collect();
        assert_eq!(from_b64url(&b64url(&all)).unwrap(), all);
        assert_eq!(from_b64(&b64(&all)).unwrap(), all);
    }

    #[test]
    fn bad_alphabets_and_lengths_are_refused() {
        assert!(from_b64url("ab+d").is_none() && from_b64url("ab/d").is_none() && from_b64url("ab=d").is_none());
        assert!(from_b64("ab-d").is_none() && from_b64("ab_d").is_none());
        assert!(from_b64url("a").is_none() && from_b64url("abcde").is_none(), "length 1 mod 4");
        assert!(from_b64url("ab").is_some());
    }

    #[test]
    fn tokens_are_43_url_safe_characters_and_all_different() {
        let (a, b) = (random_token(), random_token());
        assert!(is_token(&a) && is_token(&b) && a != b);
        assert!(!is_token(&a[..42]) && !is_token(&format!("{a}=")) && !is_token(&"+".repeat(43)));
    }

    #[test]
    fn sha256_is_the_base64url_digest() {
        // SHA-256("abc") = ba7816bf 8f01cfea 414140de 5dae2223 b00361a3 96177a9c b410ff61 f20015ad
        assert_eq!(sha256("abc"), "ungWv48Bz-pBQUDeXa4iI7ADYaOWF3qctBD_YfIAFa0");
    }

    #[test]
    fn comparison_is_exact_and_length_sensitive() {
        assert!(timing_safe_equal(b"abc", b"abc"));
        assert!(!timing_safe_equal(b"abc", b"abd") && !timing_safe_equal(b"abc", b"ab") && !timing_safe_equal(b"", b"a"));
        assert!(timing_safe_equal(b"", b""));
    }

    #[test]
    fn derived_keys_are_stable_distinct_and_depend_on_the_secret() {
        let (a, b) = (derive_keys("secret-one"), derive_keys("secret-one"));
        let all = [a.access, a.refresh, a.email, a.limit, a.device, a.sudo];
        assert_eq!(all, [b.access, b.refresh, b.email, b.limit, b.device, b.sudo]);
        for i in 0..all.len() {
            for j in i + 1..all.len() {
                assert_ne!(all[i], all[j], "{i} vs {j}");
            }
        }
        assert_ne!(derive_keys("secret-two").access, a.access);
        assert_eq!(hmac(&a.access, "x"), hmac(&b.access, "x"));
        assert_ne!(hmac(&a.access, "x"), hmac(&a.refresh, "x"));
        assert_eq!(hmac(&a.access, "x").len(), 43);
    }
}
