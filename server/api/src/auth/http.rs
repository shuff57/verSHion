//! HTTP plumbing for the auth routes (port of auth/src/http.ts): uniform JSON errors, JSON bodies that refuse
//! duplicate keys, email normalisation, client mode and origin rules, CORS and cookies.
use axum::http::{HeaderMap, HeaderName, HeaderValue, StatusCode, header};
use axum::response::{IntoResponse, Response};
use serde_json::{Map, Value};
use std::collections::HashSet;

pub const MAX_BODY_BYTES: usize = 8 * 1024;

#[derive(Debug, Clone, PartialEq)]
pub struct HttpError {
    pub status: u16,
    pub code: String,
    pub extra: Map<String, Value>,
    pub headers: Vec<(String, String)>,
}

impl HttpError {
    pub fn new(status: u16, code: &str) -> Self {
        Self { status, code: code.to_string(), extra: Map::new(), headers: vec![] }
    }
    pub fn field(status: u16, code: &str, field: &str) -> Self {
        Self::new(status, code).with("field", Value::String(field.to_string()))
    }
    pub fn with(mut self, key: &str, v: Value) -> Self {
        self.extra.insert(key.to_string(), v);
        self
    }
    pub fn header(mut self, name: &str, value: &str) -> Self {
        self.headers.push((name.to_string(), value.to_string()));
        self
    }
}

/// `{"error": code, ...extra}` with the response headers of every auth answer.
pub fn error_response(e: &HttpError) -> Response {
    let mut body = Map::new();
    body.insert("error".into(), Value::String(e.code.clone()));
    for (k, v) in &e.extra {
        body.insert(k.clone(), v.clone());
    }
    json(e.status, &Value::Object(body), &e.headers.iter().map(|(k, v)| (k.as_str(), v.as_str())).collect::<Vec<_>>())
}

fn set(res: &mut Response, name: &str, value: &str) {
    if let (Ok(n), Ok(v)) = (HeaderName::from_bytes(name.to_ascii_lowercase().as_bytes()), HeaderValue::from_str(value)) {
        // Set-Cookie may repeat; everything else replaces.
        if n == header::SET_COOKIE {
            res.headers_mut().append(n, v);
        } else {
            res.headers_mut().insert(n, v);
        }
    }
}

fn base_headers(res: &mut Response) {
    for (k, v) in [
        ("content-type", "application/json; charset=utf-8"),
        ("cache-control", "no-store"),
        ("pragma", "no-cache"),
        ("x-content-type-options", "nosniff"),
        ("referrer-policy", "no-referrer"),
    ] {
        set(res, k, v);
    }
}

pub fn json(status: u16, body: &Value, headers: &[(&str, &str)]) -> Response {
    let mut res = (StatusCode::from_u16(status).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR), body.to_string()).into_response();
    base_headers(&mut res);
    for (k, v) in headers {
        set(&mut res, k, v);
    }
    res
}

pub fn empty(status: u16, headers: &[(&str, &str)]) -> Response {
    let mut res = StatusCode::from_u16(status).unwrap_or(StatusCode::NO_CONTENT).into_response();
    base_headers(&mut res);
    res.headers_mut().remove(header::CONTENT_TYPE);
    for (k, v) in headers {
        set(&mut res, k, v);
    }
    res
}

/// Whether valid JSON `s` has an object with the same key twice (compared after unescaping). Linear.
pub fn has_duplicate_key(s: &str) -> bool {
    let b = s.as_bytes();
    let mut stack: Vec<Option<HashSet<String>>> = vec![];
    let mut want_key = false;
    let mut i = 0;
    while i < b.len() {
        match b[i] {
            b'{' => {
                stack.push(Some(HashSet::new()));
                want_key = true;
            }
            b'[' => {
                stack.push(None);
                want_key = false;
            }
            b'}' | b']' => {
                stack.pop();
                want_key = false;
            }
            b',' => want_key = matches!(stack.last(), Some(Some(_))),
            b'"' => {
                let mut j = i + 1;
                while j < b.len() && b[j] != b'"' {
                    j += if b[j] == b'\\' { 2 } else { 1 };
                }
                if want_key {
                    let key: String = serde_json::from_str(&s[i..=j.min(b.len() - 1)]).unwrap_or_default();
                    if let Some(Some(keys)) = stack.last_mut() {
                        if !keys.insert(key) {
                            return true;
                        }
                    }
                    want_key = false;
                }
                i = j;
            }
            _ => {}
        }
        i += 1;
    }
    false
}

/// A small JSON object body. Only `application/json` is accepted: HTML forms cannot send it cross-site without a
/// preflight. Duplicate keys are refused, never read two ways.
pub fn parse_json(headers: &HeaderMap, bytes: &[u8], max: usize) -> Result<Map<String, Value>, HttpError> {
    let ct = headers.get(header::CONTENT_TYPE).and_then(|v| v.to_str().ok()).unwrap_or("").split(';').next().unwrap_or("").trim().to_ascii_lowercase();
    if ct != "application/json" {
        return Err(HttpError::new(415, "unsupported_media_type"));
    }
    if bytes.len() > max {
        return Err(HttpError::new(413, "body_too_large"));
    }
    let text = std::str::from_utf8(bytes).map_err(|_| HttpError::new(400, "invalid_json"))?;
    let v: Value = serde_json::from_str(text).map_err(|_| HttpError::new(400, "invalid_json"))?;
    if has_duplicate_key(text) {
        return Err(HttpError::new(400, "invalid_json").with("reason", Value::String("duplicate_key".into())));
    }
    match v {
        Value::Object(m) => Ok(m),
        _ => Err(HttpError::new(400, "invalid_json")),
    }
}

/// A required (or optional) string field of 1..max UTF-16 units.
pub fn str_field(body: &Map<String, Value>, key: &str, max: usize, required: bool) -> Result<Option<String>, HttpError> {
    match body.get(key) {
        None | Some(Value::Null) => {
            if required {
                Err(HttpError::field(400, "invalid_request", key))
            } else {
                Ok(None)
            }
        }
        Some(Value::String(s)) if !s.is_empty() && s.encode_utf16().count() <= max => Ok(Some(s.clone())),
        Some(_) => Err(HttpError::field(400, "invalid_request", key)),
    }
}

fn label_ok(l: &str) -> bool {
    let b = l.as_bytes();
    !b.is_empty() && b.len() <= 63 && b[0] != b'-' && b[b.len() - 1] != b'-' && b.iter().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || *c == b'-')
}

/// Trim, lower-case and check shape. The same address always maps to the same string.
pub fn normalize_email(raw: &str) -> Result<String, HttpError> {
    let bad = || HttpError::field(400, "invalid_request", "email");
    let t = raw.trim();
    // ASCII only (IDN domains in punycode): no lookalike '@', no control, format or zero-width characters.
    if t.is_empty() || !t.bytes().all(|c| (0x21..=0x7e).contains(&c)) {
        return Err(bad());
    }
    let e = t.to_ascii_lowercase();
    if e.len() > 254 {
        return Err(bad());
    }
    let Some((local, domain)) = e.split_once('@') else { return Err(bad()) };
    let local_ok = (1..=64).contains(&local.len()) && !local.bytes().any(|c| matches!(c, b'@' | b'"' | b'(' | b')' | b',' | b':' | b';' | b'<' | b'>' | b'[' | b']' | b'\\'));
    let labels: Vec<&str> = domain.split('.').collect();
    if !local_ok || domain.len() > 253 || labels.len() < 2 || !labels.iter().all(|l| label_ok(l)) {
        return Err(bad());
    }
    Ok(e)
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum ClientMode {
    Web,
    Native,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ClientInfo {
    pub mode: ClientMode,
    pub origin: Option<String>,
}

fn hv<'a>(h: &'a HeaderMap, name: &str) -> Option<&'a str> {
    h.get(name).and_then(|v| v.to_str().ok())
}

/// Decides web vs native and enforces the origin rules. `X-Client: native` with an `Origin` is refused, so a browser
/// script can never receive a refresh token in a body; any `Origin` off the exact allow-list is refused.
pub fn client_info(h: &HeaderMap, allowed: &[String]) -> Result<ClientInfo, HttpError> {
    let origin = hv(h, "origin").map(str::to_string);
    let mut mode = ClientMode::Web;
    if let Some(xc) = hv(h, "x-client") {
        if xc.trim().to_lowercase() != "native" {
            return Err(HttpError::new(400, "invalid_client"));
        }
        mode = ClientMode::Native;
    }
    if mode == ClientMode::Native && origin.is_some() {
        return Err(HttpError::new(400, "native_client_with_origin"));
    }
    if let Some(o) = &origin {
        if !allowed.iter().any(|a| a == o) {
            return Err(HttpError::new(403, "origin_not_allowed"));
        }
    }
    Ok(ClientInfo { mode, origin })
}

pub fn cors_headers(origin: Option<&str>, allowed: &[String]) -> Vec<(&'static str, String)> {
    match origin {
        Some(o) if allowed.iter().any(|a| a == o) => vec![
            ("access-control-allow-origin", o.to_string()),
            ("access-control-allow-credentials", "true".into()),
            ("access-control-expose-headers", "Retry-After, ETag".into()),
            ("vary", "Origin".into()),
        ],
        _ => vec![("vary", "Origin".into())],
    }
}

/// Answers a CORS preflight. Disallowed origins get a 403 with no CORS headers.
pub fn preflight(h: &HeaderMap, allowed: &[String], methods: &str) -> Response {
    let origin = hv(h, "origin");
    if !origin.is_some_and(|o| allowed.iter().any(|a| a == o)) {
        return empty(403, &[("vary", "Origin")]);
    }
    let cors = cors_headers(origin, allowed);
    let mut res = empty(204, &[]);
    for (k, v) in &cors {
        set(&mut res, k, v);
    }
    set(&mut res, "access-control-allow-methods", methods);
    // X-Client is deliberately absent: a browser can never send `X-Client: native` cross-origin.
    set(&mut res, "access-control-allow-headers", "Content-Type, Authorization, X-Sudo-Token, If-None-Match");
    set(&mut res, "access-control-max-age", "600");
    res
}

/// Adds headers to a response; `Vary` is appended, the rest replace.
pub fn with_headers(mut res: Response, headers: &[(&str, String)]) -> Response {
    for (k, v) in headers {
        if k.eq_ignore_ascii_case("vary") {
            if let Ok(v) = HeaderValue::from_str(v) {
                res.headers_mut().append(header::VARY, v);
            }
        } else {
            set(&mut res, k, v);
        }
    }
    res
}

pub fn refresh_cookie(name: &str, token: &str, max_age_seconds: i64) -> String {
    format!("{name}={token}; Path=/; Max-Age={max_age_seconds}; HttpOnly; Secure; SameSite=Strict")
}

pub fn clear_cookie(name: &str) -> String {
    format!("{name}=; Path=/; Max-Age=0; HttpOnly; Secure; SameSite=Strict")
}

/// The one cookie called `name`. Two with our name means something planted one: refuse both.
pub fn read_cookie(h: &HeaderMap, name: &str) -> Option<String> {
    let header = hv(h, "cookie").filter(|s| !s.is_empty())?;
    let mut found: Option<&str> = None;
    for part in header.split(';') {
        let Some(i) = part.find('=') else { continue };
        if part[..i].trim() == name {
            if found.is_some() {
                return None;
            }
            found = Some(part[i + 1..].trim());
        }
    }
    found.map(str::to_string)
}

/// The compact JWT in `Authorization: Bearer ...`, if shaped like one.
pub fn bearer(h: &HeaderMap) -> Option<String> {
    let t = hv(h, "authorization")?.trim();
    let tok = t.strip_prefix("Bearer ")?;
    let parts: Vec<&str> = tok.split('.').collect();
    let ok = parts.len() == 3 && parts.iter().all(|p| !p.is_empty() && p.bytes().all(|c| c.is_ascii_alphanumeric() || c == b'-' || c == b'_'));
    ok.then(|| tok.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::to_bytes;
    use serde_json::json;

    fn hm(pairs: &[(&str, &str)]) -> HeaderMap {
        let mut m = HeaderMap::new();
        for (k, v) in pairs {
            m.append(HeaderName::from_bytes(k.as_bytes()).unwrap(), HeaderValue::from_str(v).unwrap());
        }
        m
    }
    fn code(r: Result<impl std::fmt::Debug, HttpError>) -> (u16, String) {
        let e = r.unwrap_err();
        (e.status, e.code)
    }
    const JSON_CT: &[(&str, &str)] = &[("content-type", "application/json; charset=utf-8")];

    #[tokio::test]
    async fn responses_carry_the_uniform_headers_and_errors_the_error_envelope() {
        let r = error_response(&HttpError::field(400, "invalid_request", "email").header("Retry-After", "7"));
        assert_eq!(r.status(), 400);
        for (k, v) in [("cache-control", "no-store"), ("pragma", "no-cache"), ("x-content-type-options", "nosniff"), ("referrer-policy", "no-referrer"), ("retry-after", "7")] {
            assert_eq!(r.headers()[k], v, "{k}");
        }
        assert_eq!(r.headers()["content-type"], "application/json; charset=utf-8");
        let b: Value = serde_json::from_slice(&to_bytes(r.into_body(), 1000).await.unwrap()).unwrap();
        assert_eq!(b, json!({ "error": "invalid_request", "field": "email" }));
        let e = empty(204, &[]);
        assert!(e.headers().get("content-type").is_none() && e.headers()["cache-control"] == "no-store");
        let two = json(200, &json!({}), &[("set-cookie", "a=1"), ("set-cookie", "b=2")]);
        assert_eq!(two.headers().get_all("set-cookie").iter().count(), 2);
    }

    #[test]
    fn duplicate_keys_are_found_at_any_depth_after_unescaping() {
        assert!(has_duplicate_key(r#"{"a":1,"a":2}"#));
        assert!(has_duplicate_key(r#"{"a":1,"b":{"c":1,"c":2}}"#));
        assert!(has_duplicate_key(r#"{"a":1,"b":[{"x":1,"x":2}]}"#));
        assert!(has_duplicate_key(r#"{"a":1,"a":2}"#), "escaped spelling of the same key");
        assert!(!has_duplicate_key(r#"{"a":1,"b":{"a":2},"c":[{"a":3},{"a":4}]}"#), "the same name in different objects is fine");
        assert!(!has_duplicate_key(r#"{"a":"a","b":"a"}"#), "values are not keys");
        assert!(!has_duplicate_key(r#"{"a":"x\"y","b":1}"#));
        assert!(!has_duplicate_key("{}"));
    }

    #[test]
    fn json_bodies_are_objects_of_the_right_type_size_and_encoding() {
        assert_eq!(parse_json(&hm(JSON_CT), br#"{"a":1}"#, 100).unwrap()["a"], 1);
        assert_eq!(code(parse_json(&hm(&[("content-type", "text/plain")]), b"{}", 100)), (415, "unsupported_media_type".into()));
        assert_eq!(code(parse_json(&hm(&[]), b"{}", 100)), (415, "unsupported_media_type".into()));
        assert!(parse_json(&hm(&[("content-type", "Application/JSON ; charset=x")]), b"{}", 100).is_ok());
        assert_eq!(code(parse_json(&hm(JSON_CT), &[b' '; 101], 100)), (413, "body_too_large".into()));
        for bad in [&b"{"[..], b"[]", b"null", b"5", b"\"x\"", b"", &[0xff, 0xfe]] {
            assert_eq!(code(parse_json(&hm(JSON_CT), bad, 100)), (400, "invalid_json".into()), "{bad:?}");
        }
        let e = parse_json(&hm(JSON_CT), br#"{"a":1,"a":2}"#, 100).unwrap_err();
        assert_eq!((e.status, e.extra["reason"].as_str()), (400, Some("duplicate_key")));
    }

    #[test]
    fn string_fields_are_required_or_optional_and_bounded() {
        let b = json!({ "a": "xy", "e": "", "n": 5, "z": null }).as_object().unwrap().clone();
        assert_eq!(str_field(&b, "a", 5, true).unwrap().as_deref(), Some("xy"));
        assert_eq!(code(str_field(&b, "a", 1, true)), (400, "invalid_request".into()));
        assert_eq!(str_field(&b, "missing", 5, false).unwrap(), None);
        assert_eq!(str_field(&b, "z", 5, false).unwrap(), None);
        for k in ["missing", "z", "e", "n"] {
            assert_eq!(str_field(&b, k, 5, true).unwrap_err().extra["field"], k);
        }
        assert!(str_field(&b, "e", 5, false).is_err() && str_field(&b, "n", 5, false).is_err());
    }

    #[test]
    fn emails_are_trimmed_lowercased_and_strictly_shaped() {
        assert_eq!(normalize_email("  Jordan.Smith@School.Example.ORG ").unwrap(), "jordan.smith@school.example.org");
        assert_eq!(normalize_email("a+tag@x-y.example.com").unwrap(), "a+tag@x-y.example.com");
        assert_eq!(normalize_email("a@xn--bcher-kva.example").unwrap(), "a@xn--bcher-kva.example");
        let long_local = format!("{}@example.org", "a".repeat(65));
        let long_label = format!("a@{}.org", "a".repeat(64));
        let long_all = format!("{}@{}.org", "a".repeat(60), "b.".repeat(100));
        for bad in [
            "", "plain", "a@b", "a@@b.org", "@b.org", "a@.org", "a@b..org", "a@-b.org", "a@b-.org", "a b@x.org", "a@x .org", "a\"b@x.org", "a(b)@x.org", "a,b@x.org", "a<b>@x.org",
            "a[b]@x.org", "a\\b@x.org", "a:b@x.org", "a;b@x.org", "a＠x.org", "ä@x.org", "a@exämple.org", "a\u{200B}@x.org", "a@x.org\n", &long_local, &long_label, &long_all, "a@x_y.org",
        ] {
            let r = normalize_email(bad);
            assert!(r.as_ref().is_err_and(|e| e.status == 400 && e.extra["field"] == "email") || bad == "a@x.org\n", "{bad:?} -> {r:?}");
        }
        assert_eq!(normalize_email("a@x.org\n").unwrap(), "a@x.org", "trailing whitespace is trimmed first");
    }

    #[test]
    fn client_mode_and_origin_rules() {
        let allowed = vec!["https://app.example.org".to_string()];
        let web = client_info(&hm(&[("origin", "https://app.example.org")]), &allowed).unwrap();
        assert_eq!((web.mode, web.origin.as_deref()), (ClientMode::Web, Some("https://app.example.org")));
        assert_eq!(client_info(&hm(&[]), &allowed).unwrap(), ClientInfo { mode: ClientMode::Web, origin: None });
        assert_eq!(client_info(&hm(&[("x-client", " Native ")]), &allowed).unwrap().mode, ClientMode::Native);
        assert_eq!(code(client_info(&hm(&[("x-client", "web")]), &allowed)), (400, "invalid_client".into()));
        assert_eq!(code(client_info(&hm(&[("x-client", "native"), ("origin", "https://app.example.org")]), &allowed)), (400, "native_client_with_origin".into()));
        assert_eq!(code(client_info(&hm(&[("origin", "https://evil.example")]), &allowed)), (403, "origin_not_allowed".into()));
        assert_eq!(code(client_info(&hm(&[("origin", "https://app.example.org/")]), &allowed)), (403, "origin_not_allowed".into()));
    }

    #[test]
    fn cors_headers_are_exact_origin_only_and_preflight_hides_x_client() {
        let allowed = vec!["https://app.example.org".to_string()];
        assert_eq!(cors_headers(Some("https://evil.example"), &allowed), vec![("vary", "Origin".to_string())]);
        assert_eq!(cors_headers(None, &allowed).len(), 1);
        let ok = cors_headers(Some("https://app.example.org"), &allowed);
        assert!(ok.contains(&("access-control-allow-origin", "https://app.example.org".to_string())) && ok.contains(&("access-control-allow-credentials", "true".to_string())));
        let p = preflight(&hm(&[("origin", "https://app.example.org")]), &allowed, "GET, POST");
        assert_eq!(p.status(), 204);
        assert_eq!(p.headers()["access-control-allow-methods"], "GET, POST");
        assert!(!p.headers()["access-control-allow-headers"].to_str().unwrap().to_lowercase().contains("x-client"));
        assert_eq!(p.headers()["access-control-max-age"], "600");
        let denied = preflight(&hm(&[("origin", "https://evil.example")]), &allowed, "GET");
        assert!(denied.status() == 403 && denied.headers().get("access-control-allow-origin").is_none());
        assert_eq!(preflight(&hm(&[]), &allowed, "GET").status(), 403);
        let w = with_headers(json(200, &json!({}), &[]), &[("vary", "Origin".into()), ("access-control-allow-origin", "x".into())]);
        assert_eq!(w.headers()["access-control-allow-origin"], "x");
        assert_eq!(w.headers()["vary"], "Origin");
    }

    #[test]
    fn cookies_are_hardened_and_a_duplicated_name_is_refused() {
        assert_eq!(refresh_cookie("__Host-refresh", "tok", 60), "__Host-refresh=tok; Path=/; Max-Age=60; HttpOnly; Secure; SameSite=Strict");
        assert_eq!(clear_cookie("__Host-refresh"), "__Host-refresh=; Path=/; Max-Age=0; HttpOnly; Secure; SameSite=Strict");
        assert_eq!(read_cookie(&hm(&[("cookie", "a=1; __Host-refresh=tok ; b=2")]), "__Host-refresh").as_deref(), Some("tok"));
        assert_eq!(read_cookie(&hm(&[("cookie", "__Host-refresh=a; __Host-refresh=b")]), "__Host-refresh"), None);
        assert_eq!(read_cookie(&hm(&[("cookie", "x__Host-refresh=a; junk")]), "__Host-refresh"), None);
        assert_eq!(read_cookie(&hm(&[]), "a"), None);
        assert_eq!(read_cookie(&hm(&[("cookie", "a=b=c")]), "a").as_deref(), Some("b=c"));
    }

    #[test]
    fn bearer_must_look_like_a_compact_jwt() {
        assert_eq!(bearer(&hm(&[("authorization", "Bearer aa.bb.cc")])).as_deref(), Some("aa.bb.cc"));
        assert_eq!(bearer(&hm(&[("authorization", "  Bearer a-_.b.c  ")])).as_deref(), Some("a-_.b.c"));
        for bad in ["bearer aa.bb.cc", "Bearer aa.bb", "Bearer aa.bb.cc.dd", "Bearer a a.b.c", "Bearer ..", "Basic aa.bb.cc", "Bearer  aa.bb.cc", "Bearer aa.b+.cc"] {
            assert_eq!(bearer(&hm(&[("authorization", bad)])), None, "{bad:?}");
        }
        assert_eq!(bearer(&hm(&[])), None);
    }
}
