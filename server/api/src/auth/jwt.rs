//! Minimal HS256 JWT for short-lived access tokens (port of auth/src/jwt.ts). Only HS256 is ever accepted.
use super::crypto::{Key, b64url, from_b64url, hmac_raw, timing_safe_equal};
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct AccessClaims {
    pub iss: String,
    pub aud: String,
    pub sub: String,
    pub sid: String,
    pub trust: String,
    pub iat: i64,
    pub exp: i64,
    pub jti: String,
}

/// The header is fixed and byte-compared on verify: no alg confusion, no "none", no kid games.
fn header() -> String {
    b64url(br#"{"alg":"HS256","typ":"at+jwt"}"#)
}

pub fn sign_access_token(key: &Key, claims: &AccessClaims) -> String {
    let input = format!("{}.{}", header(), b64url(serde_json::to_string(claims).expect("claims serialise").as_bytes()));
    format!("{input}.{}", b64url(&hmac_raw(key, input.as_bytes())))
}

pub struct Expect<'a> {
    pub iss: &'a str,
    pub aud: &'a str,
    pub now_seconds: i64,
}

/// The claims if the token is well formed, correctly signed, unexpired and for us.
pub fn verify_access_token(key: &Key, token: &str, expect: &Expect) -> Option<AccessClaims> {
    let parts: Vec<&str> = token.split('.').collect();
    let [h, p, s] = parts[..] else { return None };
    if h != header() {
        return None;
    }
    let sig = from_b64url(s)?;
    if !timing_safe_equal(&sig, &hmac_raw(key, format!("{h}.{p}").as_bytes())) {
        return None;
    }
    let v: Value = serde_json::from_slice(&from_b64url(p)?).ok()?;
    let text = |k: &str| v.get(k).and_then(Value::as_str).map(str::to_string);
    let num = |k: &str| v.get(k).and_then(Value::as_f64);
    if text("iss")? != expect.iss || text("aud")? != expect.aud {
        return None;
    }
    let (sub, sid, trust) = (text("sub")?, text("sid")?, text("trust")?);
    let (exp, iat) = (num("exp")?, num("iat")?);
    let now = expect.now_seconds as f64;
    if exp <= now || iat > now + 60.0 {
        return None;
    }
    Some(AccessClaims { iss: expect.iss.into(), aud: expect.aud.into(), sub, sid, trust, iat: iat as i64, exp: exp as i64, jti: text("jti").unwrap_or_default() })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::auth::crypto::derive_keys;

    const NOW: i64 = 1_800_000_000;
    fn claims() -> AccessClaims {
        AccessClaims { iss: "iss".into(), aud: "aud".into(), sub: "u1".into(), sid: "s1".into(), trust: "teacher".into(), iat: NOW, exp: NOW + 600, jti: "j1".into() }
    }
    fn ex(now: i64) -> Expect<'static> {
        Expect { iss: "iss", aud: "aud", now_seconds: now }
    }

    #[test]
    fn a_signed_token_verifies_and_carries_its_claims() {
        let k = derive_keys("s").access;
        let t = sign_access_token(&k, &claims());
        assert_eq!(t.split('.').count(), 3);
        assert_eq!(verify_access_token(&k, &t, &ex(NOW)), Some(claims()));
        assert_eq!(verify_access_token(&k, &t, &ex(NOW + 599)).map(|c| c.sub), Some("u1".into()));
    }

    #[test]
    fn expired_future_wrong_issuer_audience_or_key_are_refused() {
        let k = derive_keys("s").access;
        let t = sign_access_token(&k, &claims());
        assert!(verify_access_token(&k, &t, &ex(NOW + 600)).is_none(), "exp is exclusive");
        assert!(verify_access_token(&k, &t, &ex(NOW - 61)).is_none(), "issued in the future");
        assert!(verify_access_token(&k, &t, &ex(NOW - 60)).is_some(), "a minute of clock skew is allowed");
        assert!(verify_access_token(&k, &t, &Expect { iss: "other", aud: "aud", now_seconds: NOW }).is_none());
        assert!(verify_access_token(&k, &t, &Expect { iss: "iss", aud: "other", now_seconds: NOW }).is_none());
        assert!(verify_access_token(&derive_keys("other").access, &t, &ex(NOW)).is_none());
        assert!(verify_access_token(&derive_keys("s").refresh, &t, &ex(NOW)).is_none(), "another purpose's key");
    }

    #[test]
    fn tampering_and_alg_games_are_refused() {
        let k = derive_keys("s").access;
        let t = sign_access_token(&k, &claims());
        let parts: Vec<&str> = t.split('.').collect();
        let forged = b64url(serde_json::to_string(&AccessClaims { sub: "admin".into(), ..claims() }).unwrap().as_bytes());
        assert!(verify_access_token(&k, &format!("{}.{forged}.{}", parts[0], parts[2]), &ex(NOW)).is_none(), "payload swapped, old signature");
        let none = format!("{}.{}.", b64url(br#"{"alg":"none","typ":"at+jwt"}"#), parts[1]);
        assert!(verify_access_token(&k, &none, &ex(NOW)).is_none());
        let other_header = format!("{}.{}", b64url(br#"{"typ":"at+jwt","alg":"HS256"}"#), parts[1]);
        let resigned = format!("{other_header}.{}", b64url(&hmac_raw(&k, other_header.as_bytes())));
        assert!(verify_access_token(&k, &resigned, &ex(NOW)).is_none(), "a validly signed but different header");
        for bad in ["", "a.b", "a.b.c.d", "...", &format!("{}.{}.!!", parts[0], parts[1])] {
            assert!(verify_access_token(&k, bad, &ex(NOW)).is_none(), "{bad:?}");
        }
    }

    #[test]
    fn claims_of_the_wrong_type_are_refused_even_when_signed() {
        let k = derive_keys("s").access;
        for body in [
            r#"{"iss":"iss","aud":"aud","sub":1,"sid":"s","trust":"t","iat":1800000000,"exp":1800000600}"#,
            r#"{"iss":"iss","aud":"aud","sub":"u","sid":"s","trust":"t","iat":"1800000000","exp":1800000600}"#,
            r#"{"iss":"iss","aud":"aud","sub":"u","sid":"s","iat":1800000000,"exp":1800000600}"#,
            "null",
            "[]",
        ] {
            let input = format!("{}.{}", header(), b64url(body.as_bytes()));
            let t = format!("{input}.{}", b64url(&hmac_raw(&k, input.as_bytes())));
            assert!(verify_access_token(&k, &t, &ex(NOW)).is_none(), "{body}");
        }
    }
}
