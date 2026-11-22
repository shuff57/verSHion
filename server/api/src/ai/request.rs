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
    Protocol(Value),
}

#[derive(Debug, Clone, PartialEq)]
pub struct Message {
    pub role: String,
    pub content: Content,
}

impl Message {
    /// The OpenAI-shaped JSON the upstream takes.
    pub fn to_json(&self) -> Value {
        if let Content::Protocol(v) = &self.content { return v.clone(); }
        let content = match &self.content {
            Content::Protocol(_) => unreachable!(),
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
    read_messages_with_tools(v, max_image_bytes, false)
}

pub fn read_messages_with_tools(v: &Value, max_image_bytes: usize, tools: bool) -> Result<Vec<Message>, String> {
    let list = match v.as_array() {
        Some(a) if !a.is_empty() => a,
        _ => return Err("messages must be a non-empty array".into()),
    };
    if list.len() > MAX_MESSAGES {
        return Err(format!("messages has more than {MAX_MESSAGES} entries"));
    }
    let mut out = Vec::with_capacity(list.len());
    let mut images = 0;
    let mut pending = std::collections::BTreeSet::new();
    let mut seen = std::collections::BTreeSet::new();
    for (i, m) in list.iter().enumerate() {
        let Some(obj) = m.as_object() else {
            return Err(format!("messages[{i}] is not an object"));
        };
        if tools && obj.get("role").and_then(Value::as_str) == Some("tool") {
            let id=obj.get("tool_call_id").and_then(Value::as_str).ok_or("A tool result needs a call id.")?;
            if !pending.remove(id) { return Err("A tool result has no pending call.".into()); }
            let text=obj.get("content").and_then(Value::as_str).ok_or("Tool results must be strings.")?;
            if text.len()>256*1024 { return Err("A tool result is too large.".into()); }
            out.push(Message {role:"tool".into(),content:Content::Protocol(json!({"role":"tool","tool_call_id":id,"content":text}))});
            continue;
        }
        if !pending.is_empty() { return Err("Return all tool results before continuing the conversation.".into()); }
        if tools && obj.get("role").and_then(Value::as_str) == Some("assistant") && obj.contains_key("tool_calls") {
            let calls=obj["tool_calls"].as_array().filter(|a| !a.is_empty() && a.len()<=vershion_ai_core::MAX_TOOL_CALLS).ok_or("Invalid tool calls.")?;
            for call in calls {
                let id=call["id"].as_str().filter(|id| !id.is_empty() && id.len()<=128).ok_or("Invalid tool call id.")?;
                if !seen.insert(id.to_string()) { return Err("Duplicate tool call id.".into()); }
                if call["type"]!="function" { return Err("Only function tools are accepted.".into()); }
                let name=call["function"]["name"].as_str().ok_or("Invalid tool name.")?;
                if !vershion_ai_core::FIGURE_TOOLS.contains(&name) && name!="submit_result" { return Err("Unknown tool name.".into()); }
                let args=call["function"]["arguments"].as_str().filter(|s| s.len()<=vershion_ai_core::MAX_SCHEMA_BYTES).ok_or("Invalid tool arguments.")?;
                if !serde_json::from_str::<Value>(args).is_ok_and(|v| v.is_object()) { return Err("Tool arguments must encode a JSON object.".into()); }
                pending.insert(id.to_string());
            }
            let content=obj.get("content").cloned().unwrap_or(Value::Null);
            if !content.is_null() && !content.is_string() { return Err("Invalid assistant content.".into()); }
            let mut msg=json!({"role":"assistant","content":content,"tool_calls":calls});
            if let Some(reasoning)=obj.get("reasoning_details") {
                if !reasoning.is_array() || reasoning.to_string().len()>64*1024 { return Err("Invalid reasoning metadata.".into()); }
                msg["reasoning_details"]=reasoning.clone();
            }
            out.push(Message{role:"assistant".into(),content:Content::Protocol(msg)});
            continue;
        }
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
    if !pending.is_empty() { return Err("Tool calls need results before requesting another turn.".into()); }
    Ok(out)
}

/// Only these new request fields can cross the relay. Provider/model/privacy options
/// remain deployment-owned, and complex or external-reference schemas are refused.
#[derive(Clone, Debug, Default)]
pub struct Options {
    pub response_format: Option<Value>,
    pub tools: Option<Value>,
    pub tool_choice: Option<Value>,
}
impl Options {
    pub fn advanced(&self) -> bool { self.response_format.is_some() || self.tools.is_some() }
}
pub fn read_options(body: &serde_json::Map<String,Value>, schemas: bool, tools: bool) -> Result<Options,String> {
    let mut out=Options::default();
    if let Some(format)=body.get("response_format") {
        match format["type"].as_str() {
            Some("json_object") => {},
            Some("json_schema") if schemas => {
                let spec=&format["json_schema"];
                let name=spec["name"].as_str().filter(|s| !s.is_empty() && s.len()<=64 && s.bytes().all(|b| b.is_ascii_alphanumeric() || b==b'_')).ok_or("Invalid schema name.")?;
                vershion_ai_core::check_schema(&spec["schema"])?;
                out.response_format=Some(json!({"type":"json_schema","json_schema":{"name":name,"strict":true,"schema":spec["schema"]}}));
            },
            _=>return Err("Structured outputs are not enabled for this deployment or the requested format is invalid.".into()),
        }
    }
    if let Some(list)=body.get("tools") {
        if !tools { return Err("Native tools are not enabled for this deployment.".into()); }
        let list=list.as_array().filter(|a| !a.is_empty() && a.len()<=6).ok_or("Invalid tool definitions.")?;
        let mut defs=Vec::new(); let mut names=std::collections::BTreeSet::new();
        for t in list {
            let f=&t["function"];
            let name=f["name"].as_str().ok_or("A tool needs a name.")?;
            if t["type"]!="function" || (!vershion_ai_core::FIGURE_TOOLS.contains(&name) && name!="submit_result") || !names.insert(name) { return Err("Unknown or duplicate function tool.".into()); }
            vershion_ai_core::check_schema(&f["parameters"])?;
            let desc=f["description"].as_str().unwrap_or("");
            if desc.len()>2048 { return Err("Tool description is too long.".into()); }
            defs.push(json!({"type":"function","function":{"name":name,"description":desc,"parameters":f["parameters"],"strict":f["strict"].as_bool().unwrap_or(false)}}));
        }
        let choice=body.get("tool_choice").cloned().unwrap_or(json!("auto"));
        let valid=choice.as_str().is_some_and(|s| ["auto","none","required"].contains(&s)) || (choice["type"]=="function" && choice["function"]["name"].as_str().is_some_and(|s|names.contains(s)));
        if !valid { return Err("Invalid tool choice.".into()); }
        out.tools=Some(json!(defs)); out.tool_choice=Some(choice);
    }
    if out.tools.is_some() && out.response_format.is_some() { return Err("Choose a result schema or tools for a turn, not both.".into()); }
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
    fn native_history_requires_matching_results_and_preserves_opaque_metadata() {
        let detail=json!([{"type":"future","opaque":true}]);
        let assistant=json!({"role":"assistant","content":null,"tool_calls":[{"id":"a","type":"function","function":{"name":"get_figure","arguments":"{\"id\":\"triangle\"}"}}],"reasoning_details":detail});
        let result=json!({"role":"tool","tool_call_id":"a","content":"parameters"});
        let messages=read_messages_with_tools(&json!([assistant.clone(),result.clone()]),1024,true).unwrap();
        assert_eq!(messages[0].to_json()["reasoning_details"],detail);
        assert_eq!(messages[1].to_json(),result);
        assert!(read_messages_with_tools(&json!([assistant.clone()]),1024,true).is_err());
        assert!(read_messages_with_tools(&json!([result.clone()]),1024,true).is_err());
        assert!(read_messages_with_tools(&json!([assistant,result.clone(),result]),1024,true).is_err());
    }
    #[test]
    fn only_bounded_explicit_schema_and_function_options_are_forwarded() {
        let format=vershion_ai_core::figure_pick().response_format();
        let body=json!({"response_format":format,"provider":{"data_collection":"allow"},"model":"expensive"});
        assert!(read_options(body.as_object().unwrap(),false,false).is_err());
        let opts=read_options(body.as_object().unwrap(),true,false).unwrap();
        assert_eq!(opts.response_format.unwrap()["type"],"json_schema");
        let body=json!({"tools":[{"type":"function","function":{"name":"shell","parameters":{"type":"object"}}}]});
        assert!(read_options(body.as_object().unwrap(),true,true).is_err());
        let body=json!({"response_format":{"type":"json_schema","json_schema":{"name":"bad","schema":{"type":"object","$ref":"http://external"}}}});
        assert!(read_options(body.as_object().unwrap(),true,true).is_err());
    }

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
