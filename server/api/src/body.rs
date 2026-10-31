//! The one body reader (port of the Worker's `readBody` in pool/shared.ts, the parts every route needs):
//! Content-Length is required (411) unless the body is optional and empty, must be at most `max` (413) and
//! exact (400 `length_mismatch`), and the body must arrive within the deadline (408). The per-key concurrency
//! and byte budgets arrive with the pool port (phase 4).
use crate::ai::policy::Outcome;
use axum::body::{Body, Bytes, to_bytes};
use axum::http::{HeaderMap, header};
use std::time::Duration;

pub const DEADLINE: Duration = Duration::from_secs(10);

fn refused(status: u16, code: &str) -> Outcome {
    Outcome::err(status, code, "The request body was refused.")
}

/// The body bytes, or the refusal to send. Nothing past `max` is ever read.
pub async fn read_bounded(headers: &HeaderMap, body: Body, max: usize, optional: bool) -> Result<Bytes, Outcome> {
    let Some(declared) = headers.get(header::CONTENT_LENGTH) else {
        if optional && to_bytes(body, 0).await.is_ok() {
            return Ok(Bytes::new());
        }
        return Err(Outcome::err(411, "length_required", "Send the body with a Content-Length."));
    };
    let text = declared.to_str().unwrap_or("");
    if text.is_empty() || text.len() > 12 || !text.bytes().all(|b| b.is_ascii_digit()) {
        return Err(refused(400, "invalid_request"));
    }
    let n: usize = text.parse().map_err(|_| refused(400, "invalid_request"))?;
    if n > max {
        return Err(refused(413, "body_too_large"));
    }
    if n == 0 {
        return Ok(Bytes::new());
    }
    let bytes = match tokio::time::timeout(DEADLINE, to_bytes(body, max)).await {
        Err(_) => return Err(refused(408, "request_timeout")),
        Ok(Err(_)) => return Err(refused(400, "invalid_request")),
        Ok(Ok(b)) => b,
    };
    if bytes.len() != n {
        return Err(refused(400, "length_mismatch"));
    }
    Ok(bytes)
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::http::HeaderValue;

    fn h(cl: Option<&str>) -> HeaderMap {
        let mut m = HeaderMap::new();
        if let Some(v) = cl {
            m.insert(header::CONTENT_LENGTH, HeaderValue::from_str(v).unwrap());
        }
        m
    }
    async fn read(cl: Option<&str>, body: &str, max: usize, optional: bool) -> Result<Bytes, (u16, String)> {
        read_bounded(&h(cl), Body::from(body.to_string()), max, optional).await.map_err(|o| (o.status, o.body["error"]["code"].as_str().unwrap().to_string()))
    }

    #[tokio::test]
    async fn a_declared_length_is_required_exact_and_capped() {
        assert_eq!(read(Some("2"), "{}", 10, false).await.unwrap(), "{}");
        assert_eq!(read(None, "{}", 10, false).await.unwrap_err(), (411, "length_required".into()));
        assert_eq!(read(Some("11"), "x", 10, false).await.unwrap_err(), (413, "body_too_large".into()));
        assert_eq!(read(Some("1"), "xyz", 10, false).await.unwrap_err().0, 400, "more bytes than declared");
        assert_eq!(read(Some("5"), "xy", 10, false).await.unwrap_err(), (400, "length_mismatch".into()));
        for bad in ["-1", "1.5", "abc", "", "1234567890123"] {
            assert_eq!(read(Some(bad), "{}", 10, false).await.unwrap_err(), (400, "invalid_request".into()), "{bad:?}");
        }
    }

    #[tokio::test]
    async fn an_empty_body_is_fine_when_optional_and_a_zero_length_is_empty() {
        assert!(read(None, "", 10, true).await.unwrap().is_empty());
        assert_eq!(read(None, "x", 10, true).await.unwrap_err().0, 411, "a body with no length is never trusted");
        assert!(read(Some("0"), "", 10, false).await.unwrap().is_empty());
    }
}
