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

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::Value;

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
