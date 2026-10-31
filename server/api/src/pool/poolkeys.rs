//! The pool's derived HMAC keys and the browse cursor (worker/src/pool/shared.ts `makeCtx` keys, `signCursor`, `readCursor`,
//! `hmacOf`). Each key has its own HKDF purpose under the salt `vershion-pool/v1`, so a cursor, a preview token, a merge
//! token and an invited address' MAC are never interchangeable. Compared with values WebCrypto derived from the same secret
//! (searchkey_vectors.json).
use crate::auth::crypto::{b64url, from_b64url, hmac, timing_safe_equal, Key};
use hkdf::Hkdf;
use sha2::Sha256;

pub struct PoolKeys {
    pub cursor: Key,
    pub preview: Key,
    pub merge: Key,
    pub email: Key,
}

fn derive(secret: &str, info: &str) -> Key {
    let mut k = [0u8; 32];
    Hkdf::<Sha256>::new(Some(b"vershion-pool/v1"), secret.as_bytes()).expand(info.as_bytes(), &mut k).expect("32 bytes is a valid HKDF length");
    k
}

impl PoolKeys {
    pub fn from_secret(secret: &str) -> Self {
        PoolKeys { cursor: derive(secret, "browse-cursor"), preview: derive(secret, "block-preview"), merge: derive(secret, "circle-merge"), email: derive(secret, "circle-invite-email") }
    }

    /// The HMAC (base64url) of an invited address under its own purpose (the per-address cap).
    pub fn email_mac(&self, data: &str) -> String {
        hmac(&self.email, data)
    }

    /// `<base64url [at,id]>.<hmac>`: the cursor a browse page hands out, signed so it can only be one this server issued.
    pub fn sign_cursor(&self, at: i64, id: &str) -> String {
        let body = b64url(format!("[{at},{}]", serde_json::to_string(id).unwrap()).as_bytes());
        format!("{body}.{}", hmac(&self.cursor, &body))
    }

    /// The `(at, id)` of a cursor this server signed, or None.
    pub fn read_cursor(&self, s: &str) -> Option<(i64, String)> {
        let (body, mac) = s.split_once('.')?;
        let url_safe = |t: &str| t.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_');
        if body.is_empty() || body.len() > 200 || mac.len() != 43 || !url_safe(body) || !url_safe(mac) {
            return None;
        }
        let want = hmac(&self.cursor, body);
        if !timing_safe_equal(from_b64url(mac)?.as_slice(), from_b64url(&want)?.as_slice()) {
            return None;
        }
        let v: serde_json::Value = serde_json::from_slice(&from_b64url(body)?).ok()?;
        let arr = v.as_array()?;
        let at = arr.first()?.as_i64()?;
        let id = arr.get(1)?.as_str().filter(|i| super::sqlfrag::is_id(i))?;
        Some((at, id.to_string()))
    }
}

/// A merge token lives this long.
pub const MERGE_TOKEN_MS: i64 = 30 * 60_000;

impl PoolKeys {
    /// A merge token: `[tag, expiry, account, bank, base, head]`, HMAC'd with the merge key. Returns (token, expiry).
    pub fn merge_token(&self, now: i64, me: &str, bid: &str, base: i64, head: i64) -> (String, i64) {
        let exp = now + MERGE_TOKEN_MS;
        let body = b64url(serde_json::json!(["m1", exp, me, bid, base, head]).to_string().as_bytes());
        (format!("{body}.{}", hmac(&self.merge, &body)), exp)
    }

    /// The base a send's merge token records (`merged_from`), iff the token was issued to `me` for bank `bid` at head `head` and has not
    /// expired; else None (`422 merge_token_invalid`).
    pub fn verify_merge_token(&self, now: i64, token: &str, me: &str, bid: &str, head: i64) -> Option<i64> {
        let (body, mac) = token.split_once('.')?;
        let url_safe = |t: &str, n: std::ops::RangeInclusive<usize>| n.contains(&t.len()) && t.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_');
        if !url_safe(body, 1..=400) || !url_safe(mac, 43..=43) {
            return None;
        }
        let want = hmac(&self.merge, body);
        if want.len() != mac.len() || !timing_safe_equal(want.as_bytes(), mac.as_bytes()) {
            return None;
        }
        let v: serde_json::Value = serde_json::from_slice(&from_b64url(body)?).ok()?;
        let [tag, exp, u, b, base, h] = v.as_array()?.as_slice() else { return None };
        if tag.as_str() != Some("m1") || exp.as_i64()? < now || u.as_str() != Some(me) || b.as_str() != Some(bid) || h.as_i64() != Some(head) {
            return None;
        }
        base.as_i64().filter(|b| *b >= 1 && *b < head)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::Value;

    #[test]
    fn a_merge_token_is_bound_to_account_bank_head_and_time() {
        let k = keys();
        let (t, exp) = k.merge_token(1_000, "u1", "b1", 2, 5);
        assert_eq!(exp, 1_000 + MERGE_TOKEN_MS);
        assert_eq!(k.verify_merge_token(1_000, &t, "u1", "b1", 5), Some(2));
        assert_eq!(k.verify_merge_token(exp, &t, "u1", "b1", 5), Some(2), "valid through its expiry");
        for (now, me, bid, head) in [(exp + 1, "u1", "b1", 5), (1_000, "u2", "b1", 5), (1_000, "u1", "b2", 5), (1_000, "u1", "b1", 6)] {
            assert_eq!(k.verify_merge_token(now, &t, me, bid, head), None);
        }
        let (body, mac) = t.split_once('.').unwrap();
        let flipped = format!("{}{}", &mac[..42], if mac.ends_with('A') { 'B' } else { 'A' });
        for bad in [format!("{body}.{flipped}"), "nonsense".into(), String::new(), format!("{body}x.{mac}"), format!("{body}.")] {
            assert_eq!(k.verify_merge_token(1_000, &bad, "u1", "b1", 5), None, "{bad}");
        }
        // The base must precede the head, and the key is the merge key's alone.
        let (same, _) = k.merge_token(1_000, "u1", "b1", 5, 5);
        assert_eq!(k.verify_merge_token(1_000, &same, "u1", "b1", 5), None);
        let (zero, _) = k.merge_token(1_000, "u1", "b1", 0, 5);
        assert_eq!(k.verify_merge_token(1_000, &zero, "u1", "b1", 5), None);
        assert_eq!(PoolKeys::from_secret("another-secret-0123456789abcdef012345").verify_merge_token(1_000, &t, "u1", "b1", 5), None);
    }

    fn vectors() -> Value {
        serde_json::from_str(include_str!("searchkey_vectors.json")).unwrap()
    }
    fn keys() -> PoolKeys {
        PoolKeys::from_secret(vectors()["secret"].as_str().unwrap())
    }

    #[test]
    fn each_purpose_derives_the_key_webcrypto_does() {
        let v = vectors();
        let k = keys();
        for (name, key) in [("cursor", &k.cursor), ("preview", &k.preview), ("merge", &k.merge), ("email", &k.email)] {
            assert_eq!(hmac(key, "hello"), v["macs"][name].as_str().unwrap(), "{name}");
        }
        assert_eq!(k.email_mac("hello"), v["macs"]["email"].as_str().unwrap());
        assert_ne!(v["macs"]["cursor"], v["macs"]["preview"], "purposes are not interchangeable");
    }

    #[test]
    fn a_cursor_the_worker_signed_is_read_and_ours_is_byte_identical() {
        let v = vectors();
        let (at, id) = (v["cursorAt"].as_i64().unwrap(), v["cursorId"].as_str().unwrap());
        assert_eq!(keys().sign_cursor(at, id), v["cursor"].as_str().unwrap());
        assert_eq!(keys().read_cursor(v["cursor"].as_str().unwrap()), Some((at, id.to_string())));
    }

    #[test]
    fn a_tampered_foreign_or_malformed_cursor_is_refused() {
        let v = vectors();
        let good = v["cursor"].as_str().unwrap().to_string();
        let k = keys();
        let (body, mac) = good.split_once('.').unwrap();
        let other_body = b64url(br#"[1,"0123456789abcdef0123456789abcdef"]"#);
        for bad in [format!("{other_body}.{mac}"), format!("{body}.{}", "A".repeat(43)), format!("{body}."), body.to_string(), String::new(), format!(".{mac}"), format!("{body}.{mac}x")] {
            assert_eq!(k.read_cursor(&bad), None, "{bad}");
        }
        // A cursor under another secret, and a validly signed body of the wrong shape.
        assert_eq!(PoolKeys::from_secret("another-secret-0123456789abcdef012345").read_cursor(&good), None);
        for shape in [r#"["x","0123456789abcdef0123456789abcdef"]"#, r#"[1,"not-an-id"]"#, r#"[1]"#, r#"{"a":1}"#] {
            let body = b64url(shape.as_bytes());
            assert_eq!(k.read_cursor(&format!("{body}.{}", hmac(&k.cursor, &body))), None, "{shape}");
        }
    }
}
