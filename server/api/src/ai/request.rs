//! What a client may send the relay, and nothing more (port of worker/src/ai/util.ts and `readMessages`,
//! `max_tokens` and `temperature` handling in index.ts). The client never chooses the model, the provider
//! options or reasoning: only messages, a clamped `max_tokens` and a bounded `temperature` get through.
use serde_json::{Value, json};

pub const MAX_BODY_BYTES: usize = 13 * 1024 * 1024;
pub const MAX_IMAGES: usize = 4;
pub const MAX_MESSAGES: usize = 40;

#[derive(Debug, Clone, PartialEq)]
pub enum Part {
    Text(String),
    /// A validated `data:image/…;base64,…` URI.
    Image(String),
}

#[derive(Debug, Clone, PartialEq)]
pub enum Content {
    Text(String),
    Parts(Vec<Part>),
}

#[derive(Debug, Clone, PartialEq)]
pub struct Message {
    pub role: String,
    pub content: Content,
}

impl Message {
    /// The OpenAI-shaped JSON the upstream takes.
    pub fn to_json(&self) -> Value {
        let content = match &self.content {
            Content::Text(t) => json!(t),
            Content::Parts(ps) => Value::Array(
                ps.iter()
                    .map(|p| match p {
                        Part::Text(t) => json!({ "type": "text", "text": t }),
                        Part::Image(u) => json!({ "type": "image_url", "image_url": { "url": u } }),
                    })
                    .collect(),
            ),
        };
        json!({ "role": self.role, "content": content })
    }
}

/// The OpenAI-style error body: never a raw upstream string.
pub fn error_body(code: &str, message: &str) -> Value {
    json!({ "error": { "code": code, "message": message } })
}

/// Decoded byte count of a base64 image data URI, or None when the URI is not one we accept
/// (`^data:image/(png|jpeg|webp);base64,[A-Za-z0-9+/=]+$`).
pub fn data_uri_bytes(url: &str) -> Option<usize> {
    let b64 = ["png", "jpeg", "webp"].iter().find_map(|t| url.strip_prefix(&format!("data:image/{t};base64,")))?;
    if b64.is_empty() || !b64.bytes().all(|c| c.is_ascii_alphanumeric() || matches!(c, b'+' | b'/' | b'=')) {
        return None;
    }
    Some(b64.len() * 3 / 4)
}

/// Validates `messages` and counts images across the whole conversation. Err carries the client-facing sentence.
pub fn read_messages(v: &Value, max_image_bytes: usize) -> Result<Vec<Message>, String> {
    let list = match v.as_array() {
        Some(a) if !a.is_empty() => a,
        _ => return Err("messages must be a non-empty array".into()),
    };
    if list.len() > MAX_MESSAGES {
        return Err(format!("messages has more than {MAX_MESSAGES} entries"));
    }
    let mut out = Vec::with_capacity(list.len());
    let mut images = 0;
    for (i, m) in list.iter().enumerate() {
        let Some(obj) = m.as_object() else {
            return Err(format!("messages[{i}] is not an object"));
        };
        let role = match obj.get("role").and_then(Value::as_str) {
            Some(r @ ("system" | "user" | "assistant")) => r.to_string(),
            _ => return Err(format!("messages[{i}].role must be system, user or assistant")),
        };
        let content = obj.get("content");
        if let Some(t) = content.and_then(Value::as_str) {
            out.push(Message { role, content: Content::Text(t.to_string()) });
            continue;
        }
        let parts = match content.and_then(Value::as_array) {
            Some(a) if !a.is_empty() => a,
            _ => return Err(format!("messages[{i}].content must be a string or a non-empty array")),
        };
        let mut read = Vec::with_capacity(parts.len());
        for (j, part) in parts.iter().enumerate() {
            let kind = part.get("type").and_then(Value::as_str);
            let text = part.get("text").and_then(Value::as_str);
            let url = part.get("image_url").and_then(|u| u.get("url")).and_then(Value::as_str);
            match (kind, text, url) {
                (Some("text"), Some(t), _) => read.push(Part::Text(t.to_string())),
                (Some("image_url"), _, Some(u)) => {
                    let Some(bytes) = data_uri_bytes(u) else {
                        return Err(format!("messages[{i}].content[{j}] is not a png/jpeg/webp base64 data URI"));
                    };
                    if bytes > max_image_bytes {
                        return Err(format!("messages[{i}].content[{j}] image is too large"));
                    }
                    images += 1;
                    if images > MAX_IMAGES {
                        return Err(format!("more than {MAX_IMAGES} images in one request"));
                    }
                    read.push(Part::Image(u.to_string()));
                }
                _ => return Err(format!("messages[{i}].content[{j}] is not a text or image_url part")),
            }
        }
        out.push(Message { role, content: Content::Parts(read) });
    }
    Ok(out)
}

/// `max_tokens`: the client's number floored, at least 1, never above the deployment's cap; absent means the cap.
pub fn clamp_max_tokens(requested: Option<&Value>, cap: i64) -> i64 {
    let asked = requested.and_then(Value::as_f64).filter(|n| n.is_finite()).map(|n| n.floor() as i64).unwrap_or(cap);
    asked.min(cap).max(1)
}

/// `temperature`: bounded to [0, 1]; absent or not a finite number means 0.2.
pub fn clamp_temperature(requested: Option<&Value>) -> f64 {
    requested.and_then(Value::as_f64).filter(|n| n.is_finite()).unwrap_or(0.2).clamp(0.0, 1.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    const PNG: &str = "data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mP8z8BQDwAEhQGAhKmMIQAAAABJRU5ErkJggg==";

    #[test]
    fn only_png_jpeg_webp_base64_data_uris_count_and_their_size_is_the_decoded_size() {
        assert_eq!(data_uri_bytes(PNG), Some((PNG.len() - "data:image/png;base64,".len()) * 3 / 4));
        assert!(data_uri_bytes("data:image/jpeg;base64,AAAA").is_some() && data_uri_bytes("data:image/webp;base64,AAAA").is_some());
        for bad in ["", "https://evil.example/x.png", "data:image/gif;base64,AAAA", "data:image/png;base64,", "data:image/png;base64,AA AA", "data:image/png,AAAA", "data:text/html;base64,AAAA", "x data:image/png;base64,AAAA"] {
            assert_eq!(data_uri_bytes(bad), None, "{bad:?}");
        }
    }

    #[test]
    fn a_string_or_part_list_is_accepted_and_round_trips_to_the_upstream_shape() {
        let v = json!([{ "role": "system", "content": "be brief" }, { "role": "user", "content": [{ "type": "text", "text": "read it" }, { "type": "image_url", "image_url": { "url": PNG } }] }]);
        let m = read_messages(&v, 4 << 20).unwrap();
        assert_eq!(m.len(), 2);
        assert_eq!(m[0].to_json(), json!({ "role": "system", "content": "be brief" }));
        assert_eq!(m[1].to_json()["content"][1]["image_url"]["url"], PNG);
    }

    #[test]
    fn bad_conversations_are_refused_with_the_workers_sentences() {
        let err = |v: Value| read_messages(&v, 8).unwrap_err();
        assert_eq!(err(json!([])), "messages must be a non-empty array");
        assert_eq!(err(json!("x")), "messages must be a non-empty array");
        assert_eq!(err(json!([{ "role": "tool", "content": "x" }])), "messages[0].role must be system, user or assistant");
        assert_eq!(err(json!([5])), "messages[0] is not an object");
        assert_eq!(err(json!([{ "role": "user", "content": [] }])), "messages[0].content must be a string or a non-empty array");
        assert_eq!(err(json!([{ "role": "user" }])), "messages[0].content must be a string or a non-empty array");
        assert_eq!(err(json!([{ "role": "user", "content": [{ "type": "audio" }] }])), "messages[0].content[0] is not a text or image_url part");
        let remote = json!([{ "role": "user", "content": [{ "type": "image_url", "image_url": { "url": "https://evil.example/x.png" } }] }]);
        assert!(err(remote).contains("data URI"));
        let big = json!([{ "role": "user", "content": [{ "type": "image_url", "image_url": { "url": "data:image/png;base64,iVBORw0KGgo=" } }] }]);
        assert_eq!(err(big), "messages[0].content[0] image is too large");
    }

    #[test]
    fn the_message_and_image_counts_are_capped() {
        let many = Value::Array((0..=MAX_MESSAGES).map(|_| json!({ "role": "user", "content": "x" })).collect());
        assert_eq!(read_messages(&many, 1 << 20).unwrap_err(), format!("messages has more than {MAX_MESSAGES} entries"));
        let part = json!({ "type": "image_url", "image_url": { "url": PNG } });
        let five = json!([{ "role": "user", "content": vec![part.clone(); MAX_IMAGES + 1] }]);
        assert_eq!(read_messages(&five, 1 << 20).unwrap_err(), format!("more than {MAX_IMAGES} images in one request"));
        let split = json!([{ "role": "user", "content": vec![part.clone(); 3] }, { "role": "user", "content": vec![part; 2] }]);
        assert!(read_messages(&split, 1 << 20).is_err(), "images are counted across the whole conversation");
    }

    #[test]
    fn max_tokens_and_temperature_are_clamped() {
        assert_eq!(clamp_max_tokens(Some(&json!(999_999)), 64), 64);
        assert_eq!(clamp_max_tokens(Some(&json!(0)), 64), 1);
        assert_eq!(clamp_max_tokens(Some(&json!(-5)), 64), 1);
        assert_eq!(clamp_max_tokens(Some(&json!(10.9)), 64), 10);
        assert_eq!(clamp_max_tokens(Some(&json!("big")), 64), 64);
        assert_eq!(clamp_max_tokens(None, 64), 64);
        assert_eq!(clamp_temperature(Some(&json!(7))), 1.0);
        assert_eq!(clamp_temperature(Some(&json!(-1))), 0.0);
        assert_eq!(clamp_temperature(Some(&json!("hot"))), 0.2);
        assert_eq!(clamp_temperature(None), 0.2);
    }

    #[test]
    fn the_error_envelope_has_a_code_and_a_message_only() {
        assert_eq!(error_body("model_failed", "The model call failed. Try again."), json!({ "error": { "code": "model_failed", "message": "The model call failed. Try again." } }));
    }
}
