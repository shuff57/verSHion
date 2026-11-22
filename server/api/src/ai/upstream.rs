//! The model call (port of `callOpenRouter`, `shapeReply` and `callModel` in worker/src/ai/index.ts).
//!
//! OpenRouter is the first upstream (CheaperInference the default, see `config.rs`). Off Cloudflare there is no Workers AI binding, so the fallback is a
//! second model on the same provider (decision 2026-10-31): one try on the primary, then up to two on the fallback
//! model, or a second try on the primary when none is configured. The HTTP itself sits behind `Transport`
//! so the logic is tested without a network. Only a status or an error CLASS is logged: a message can carry
//! the key or the prompt.
use super::request::Message;
use serde_json::{Value, json};
use std::time::Duration;

pub const OPENROUTER_URL: &str = "https://openrouter.ai/api/v1/chat/completions";
pub const DEFAULT_MODEL: &str = "google/gemma-4-26b-a4b-it";
pub const CHEAPERINFERENCE_URL: &str = "https://api.cheaperinference.com/v1/chat/completions";
/// CheaperInference names the same model without the vendor prefix.
pub const CHEAPERINFERENCE_DEFAULT_MODEL: &str = "deepseek-v4.1-flash-1017";

/// One configured chat-completions provider: the key, endpoint and model the relay calls. Built by
/// `config.rs` from `AI_UPSTREAM` (CheaperInference by default, or OpenRouter).
pub struct Upstream {
    /// Which provider this is, as `/ai/health` reports it ("openrouter" or "cheaperinference").
    pub name: &'static str,
    pub key: String,
    /// The chat-completions endpoint of the upstream.
    pub url: String,
    /// A plain OpenAI-compatible upstream (CheaperInference): none of OpenRouter's own request fields are sent.
    pub plain: bool,
    pub structured_outputs: bool,
    pub tools: bool,
    /// Extra top-level request fields for a plain upstream (for example how it turns reasoning off), from the deployment's env.
    pub extra: Option<serde_json::Map<String, Value>>,
    pub model: String,
    /// Tried when the primary fails, returns nothing or times out.
    pub fallback_model: Option<String>,
    /// Ask for providers that do not keep prompts (default true; only the deployment can relax it).
    pub privacy: bool,
    pub timeout: Duration,
}

impl Upstream {
    /// OpenRouter with its defaults; the other fields are overridden with struct update syntax.
    pub fn openrouter(key: impl Into<String>) -> Self {
        Self { name: "openrouter", key: key.into(), url: OPENROUTER_URL.into(), plain: false, structured_outputs: false, tools: false, extra: None, model: DEFAULT_MODEL.into(), fallback_model: None, privacy: true, timeout: Duration::from_secs(60) }
    }
}

/// One POST to the upstream. `Err` carries an error CLASS (for example "Timeout"), never a message.
pub struct Post<'a> {
    pub url: &'a str,
    pub key: &'a str,
    pub body: &'a Value,
    pub timeout: Duration,
}

pub struct Answer {
    pub status: u16,
    pub body: Value,
}

pub trait Transport {
    fn post(&self, p: Post<'_>) -> impl std::future::Future<Output = Result<Answer, String>> + Send;
}

/// The real network: a shared `reqwest` client (rustls). Errors are reduced to a class name here, so nothing
/// that could hold the key or the prompt travels any further. A non-2xx answer is not read at all.
pub struct HttpTransport {
    client: reqwest::Client,
}

/// A reply bigger than this is refused: the relay only ever expects a completion.
const MAX_REPLY_BYTES: u64 = 16 * 1024 * 1024;

impl HttpTransport {
    pub fn new() -> Self {
        Self { client: reqwest::Client::builder().user_agent("vershion-api").build().expect("build the http client") }
    }
}

impl Default for HttpTransport {
    fn default() -> Self {
        Self::new()
    }
}

fn class(e: &reqwest::Error) -> String {
    if e.is_timeout() {
        "Timeout"
    } else if e.is_connect() {
        "Connect"
    } else if e.is_decode() || e.is_body() {
        "Decode"
    } else {
        "Request"
    }
    .to_string()
}

impl Transport for HttpTransport {
    async fn post(&self, p: Post<'_>) -> Result<Answer, String> {
        let res = self
            .client
            .post(p.url)
            .bearer_auth(p.key)
            .header("X-Title", "verSHion")
            .json(p.body)
            .timeout(p.timeout)
            .send()
            .await
            .map_err(|e| class(&e))?;
        let status = res.status().as_u16();
        if !res.status().is_success() {
            return Ok(Answer { status, body: Value::Null });
        }
        if res.content_length().is_some_and(|n| n > MAX_REPLY_BYTES) {
            return Err("TooLarge".into());
        }
        let body = res.json::<Value>().await.map_err(|e| class(&e))?;
        Ok(Answer { status, body })
    }
}

/// What one model turn gave back: the text and what it cost. Never the provider's own body.
#[derive(Debug, PartialEq)]
pub enum ModelOut {
    Ok(Reply),
    /// The call threw, timed out or answered with an error status.
    Failed,
    /// The call worked but returned no text.
    Empty,
}

#[derive(Debug, PartialEq)]
pub struct Reply {
    /// The app-facing OpenAI body.
    pub reply: Value,
    pub text: String,
    pub in_tokens: f64,
    pub out_tokens: f64,
    pub cost_usd: f64,
}

/// The app-facing reply: the upstream body with reasoning and `usage.cost` dropped. Hidden reasoning can be
/// larger than the answer and is not part of the contract; the cost is kept aside for the quota. An empty
/// completion is not an answer (None), so the caller retries.
pub fn shape_reply(raw: &Value) -> Option<Reply> {
    let choice = raw.pointer("/choices/0");
    let source=choice?.get("message")?;
    let content=source["content"].as_str().unwrap_or("");
    let calls=source["tool_calls"].as_array().filter(|a| !a.is_empty() && a.len()<=vershion_ai_core::MAX_TOOL_CALLS);
    if content.trim().is_empty() && calls.is_none() && source.get("refusal").is_none() { return None; }
    let mut message=json!({"role":"assistant","content":if content.is_empty(){Value::Null}else{json!(content)}});
    if let Some(calls)=calls {
        message["tool_calls"]=json!(calls);
        if let Some(details)=source.get("reasoning_details").filter(|d|d.is_array() && d.to_string().len()<=64*1024) { message["reasoning_details"]=details.clone(); }
    }
    if let Some(refusal)=source.get("refusal") { message["refusal"]=refusal.clone(); }
    let n = |p: &str| raw.pointer(p).and_then(Value::as_f64).filter(|x| x.is_finite()).unwrap_or(0.0);
    // CheaperInference reports the price as a decimal string in `billing.billed_cost_usd`; OpenRouter as `usage.cost`.
    let billed = raw.pointer("/billing/billed_cost_usd").and_then(Value::as_str).and_then(|c| c.parse::<f64>().ok()).filter(|x| x.is_finite());
    let (in_tokens, out_tokens) = (n("/usage/prompt_tokens"), n("/usage/completion_tokens"));
    let finish = choice?.get("finish_reason").and_then(Value::as_str).unwrap_or("stop");
    Some(Reply {
        text: content.to_string(),
        in_tokens,
        out_tokens,
        cost_usd: billed.unwrap_or_else(|| n("/usage/cost")),
        reply: json!({
            "choices": [{ "index": 0, "message": message, "finish_reason": finish }],
            "usage": { "prompt_tokens": in_tokens, "completion_tokens": out_tokens, "total_tokens": n("/usage/total_tokens") },
        }),
    })
}

/// The request body. The client chose none of the model, reasoning or provider options.
pub fn request_body(o: &Upstream, model: &str, messages: &[Message], max_tokens: i64, temperature: f64) -> Value {
    if o.plain {
        let mut body = json!({
            "model": model,
            "messages": messages.iter().map(Message::to_json).collect::<Vec<_>>(),
            "response_format": { "type": "json_object" },
            "max_tokens": max_tokens,
            "temperature": temperature,
        });
        if let (Some(extra), Some(map)) = (&o.extra, body.as_object_mut()) {
            for (k, v) in extra {
                map.entry(k.clone()).or_insert_with(|| v.clone());
            }
        }
        return body;
    }
    json!({
        "model": model,
        "messages": messages.iter().map(Message::to_json).collect::<Vec<_>>(),
        "response_format": { "type": "json_object" },
        "max_tokens": max_tokens,
        "temperature": temperature,
        // No hidden reasoning (slow, and billed as output), and report the cost of the call.
        "reasoning": { "enabled": false },
        "usage": { "include": true },
        "provider": { "data_collection": if o.privacy { "deny" } else { "allow" } },
    })
}

async fn call_once<T: Transport>(t: &T, o: &Upstream, model: &str, messages: &[Message], max_tokens: i64, temperature: f64, options: &super::request::Options) -> ModelOut {
    let mut body = request_body(o, model, messages, max_tokens, temperature);
    if let Some(format)=&options.response_format { body["response_format"]=format.clone(); }
    if let Some(tools)=&options.tools {
        body.as_object_mut().unwrap().remove("response_format");
        body["tools"]=tools.clone(); body["tool_choice"]=options.tool_choice.clone().unwrap_or(json!("auto")); body["parallel_tool_calls"]=json!(false);
    }
    if options.advanced() && !o.plain { body["provider"]["require_parameters"]=json!(true); }
    match t.post(Post { url: &o.url, key: &o.key, body: &body, timeout: o.timeout }).await {
        Err(class) => {
            tracing::error!(provider=o.name, "ai call failed ({class})");
            ModelOut::Failed
        }
        Ok(a) if !(200..300).contains(&a.status) => {
            tracing::error!(provider=o.name, "ai answered {}", a.status);
            ModelOut::Failed
        }
        Ok(a) => match shape_reply(&a.body) {
            Some(r) => ModelOut::Ok(r),
            None => {
                tracing::error!(provider=o.name, "ai returned no answer");
                ModelOut::Empty
            }
        },
    }
}

/// One model turn. Primary once; then the fallback model (up to two tries) or, with none, the primary again.
/// The result is the last failure's class when every try fails.
pub async fn call_model<T: Transport>(t: &T, o: &Upstream, messages: &[Message], max_tokens: i64, temperature: f64) -> ModelOut {
    call_model_with_options(t,o,messages,max_tokens,temperature,&super::request::Options::default()).await
}

pub async fn call_model_with_options<T: Transport>(t: &T, o: &Upstream, messages: &[Message], max_tokens: i64, temperature: f64, options: &super::request::Options) -> ModelOut {
    let first = call_once(t, o, &o.model, messages, max_tokens, temperature, options).await;
    if matches!(first, ModelOut::Ok(_)) {
        return first;
    }
    let fallback = if options.advanced() || messages.iter().any(|m| matches!(m.content, super::request::Content::Protocol(_))) { None } else { o.fallback_model.as_ref() };
    let (model, tries) = match fallback {
        Some(m) => (m.as_str(), 2),
        None => (o.model.as_str(), 1),
    };
    let mut last = first;
    for _ in 0..tries {
        last = call_once(t, o, model, messages, max_tokens, temperature, options).await;
        if matches!(last, ModelOut::Ok(_)) {
            break;
        }
    }
    last
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tool_only_answers_keep_reasoning_and_cost_without_leaking_the_raw_body() {
        let details=json!([{"type":"future","opaque":42}]);
        let raw=json!({"secret":"never forward","choices":[{"message":{"content":null,"tool_calls":[{"id":"x","type":"function","function":{"name":"get_figure","arguments":"{}"}}],"reasoning_details":details},"finish_reason":"tool_calls"}],"usage":{"cost":0.02}});
        let reply=shape_reply(&raw).unwrap();
        assert_eq!(reply.cost_usd,0.02); assert!(reply.reply.get("secret").is_none());
        assert_eq!(reply.reply["choices"][0]["message"]["reasoning_details"],details);
        assert_eq!(reply.reply["choices"][0]["finish_reason"],"tool_calls");
    }

    #[test]
    fn a_plain_upstream_gets_no_openrouter_fields_and_its_cost_is_read_from_billing() {
        let o = Upstream { plain: true, extra: json!({ "reasoning_effort": "none", "max_tokens": 1 }).as_object().cloned(), ..Upstream::openrouter("k") };
        let body = request_body(&o, "gemma-4-26b-a4b-it", &[], 100, 0.0);
        assert_eq!((body["reasoning_effort"].as_str(), body["max_tokens"].as_i64()), (Some("none"), Some(100)), "extra fields never override the relay's own");
        assert!(body.get("reasoning").is_none() && body.get("usage").is_none() && body.get("provider").is_none());
        assert_eq!(body["response_format"], json!({ "type": "json_object" }));
        let raw = json!({ "choices": [{ "message": { "content": "{}" } }], "usage": { "prompt_tokens": 10, "completion_tokens": 5 }, "billing": { "billed_cost_usd": "0.000123" } });
        let r = shape_reply(&raw).unwrap();
        assert!((r.cost_usd - 0.000123).abs() < 1e-12 && r.in_tokens == 10.0);
    }

    use crate::ai::request::Content;
    use std::collections::VecDeque;
    use std::sync::Mutex;

    /// Answers from a script, and remembers every request it was given.
    struct Fake {
        script: Mutex<VecDeque<Result<Answer, String>>>,
        seen: Mutex<Vec<(String, String, Value)>>, // (key, model, body)
    }

    impl Fake {
        fn new(script: Vec<Result<Answer, String>>) -> Self {
            Self { script: Mutex::new(script.into()), seen: Mutex::new(vec![]) }
        }
        fn models(&self) -> Vec<String> {
            self.seen.lock().unwrap().iter().map(|s| s.1.clone()).collect()
        }
    }

    impl Transport for Fake {
        async fn post(&self, p: Post<'_>) -> Result<Answer, String> {
            self.seen.lock().unwrap().push((p.key.to_string(), p.body["model"].as_str().unwrap().to_string(), p.body.clone()));
            self.script.lock().unwrap().pop_front().unwrap_or(Err("Exhausted".into()))
        }
    }

    fn ok(content: &str, cost: f64) -> Result<Answer, String> {
        Ok(Answer {
            status: 200,
            body: json!({ "choices": [{ "message": { "role": "assistant", "content": content, "reasoning": "hidden" }, "finish_reason": "stop" }], "usage": { "prompt_tokens": 7, "completion_tokens": 9, "total_tokens": 16, "cost": cost } }),
        })
    }
    fn status(code: u16) -> Result<Answer, String> {
        Ok(Answer { status: code, body: json!("upstream trouble with or-secret-key") })
    }
    fn msgs() -> Vec<Message> {
        vec![Message { role: "user".into(), content: Content::Text("hi".into()) }]
    }
    fn or(fallback: Option<&str>) -> Upstream {
        Upstream { model: "google/test-model".into(), fallback_model: fallback.map(String::from), ..Upstream::openrouter("or-secret-key") }
    }

    #[tokio::test]
    async fn a_schema_request_never_falls_back_to_a_model_with_unknown_capabilities() {
        let fake=Fake::new(vec![status(500),ok("{}",0.01)]);
        let options=super::super::request::Options {response_format:Some(vershion_ai_core::figure_pick().response_format()),..Default::default()};
        let _=call_model_with_options(&fake,&or(Some("other-model")),&msgs(),200,0.0,&options).await;
        assert_eq!(fake.models(),["google/test-model","google/test-model"]);
    }

    #[tokio::test]
    async fn a_good_answer_is_one_call_with_reasoning_off_cost_reported_and_no_retention() {
        let t = Fake::new(vec![ok("{\"ok\":true}", 0.00011)]);
        let out = call_model(&t, &or(None), &msgs(), 100, 0.2).await;
        let ModelOut::Ok(r) = out else { panic!("{out:?}") };
        assert_eq!((r.text.as_str(), r.in_tokens, r.out_tokens, r.cost_usd), ("{\"ok\":true}", 7.0, 9.0, 0.00011));
        let seen = t.seen.lock().unwrap();
        assert_eq!(seen.len(), 1);
        let (key, model, body) = &seen[0];
        assert_eq!((key.as_str(), model.as_str()), ("or-secret-key", "google/test-model"));
        assert_eq!(body["reasoning"], json!({ "enabled": false }));
        assert_eq!(body["usage"], json!({ "include": true }));
        assert_eq!(body["response_format"], json!({ "type": "json_object" }));
        assert_eq!(body["provider"], json!({ "data_collection": "deny" }));
        assert_eq!((body["max_tokens"].as_i64(), body["temperature"].as_f64()), (Some(100), Some(0.2)));
    }

    #[tokio::test]
    async fn the_reply_has_no_reasoning_and_no_cost() {
        let t = Fake::new(vec![ok("hello", 0.5)]);
        let ModelOut::Ok(r) = call_model(&t, &or(None), &msgs(), 10, 0.0).await else { panic!() };
        assert_eq!(r.reply["choices"][0]["message"], json!({ "role": "assistant", "content": "hello" }));
        assert_eq!(r.reply["usage"], json!({ "prompt_tokens": 7.0, "completion_tokens": 9.0, "total_tokens": 16.0 }));
        assert!(!r.reply.to_string().contains("hidden") && r.reply["usage"].get("cost").is_none());
    }

    #[tokio::test]
    async fn privacy_can_be_relaxed_only_by_the_deployment() {
        let t = Fake::new(vec![ok("x", 0.0)]);
        let o = Upstream { privacy: false, ..or(None) };
        call_model(&t, &o, &msgs(), 10, 0.0).await;
        assert_eq!(t.seen.lock().unwrap()[0].2["provider"], json!({ "data_collection": "allow" }));
    }

    #[tokio::test]
    async fn with_no_fallback_model_a_failure_is_tried_once_more_then_fails() {
        let t = Fake::new(vec![status(500), status(500)]);
        assert_eq!(call_model(&t, &or(None), &msgs(), 10, 0.0).await, ModelOut::Failed);
        assert_eq!(t.models(), ["google/test-model", "google/test-model"]);
        let t = Fake::new(vec![status(503), ok("fine", 0.001)]);
        assert!(matches!(call_model(&t, &or(None), &msgs(), 10, 0.0).await, ModelOut::Ok(_)));
    }

    #[tokio::test]
    async fn a_failure_empty_reply_or_timeout_goes_to_the_fallback_model() {
        for first in [status(503), ok("  ", 0.0), Err("Timeout".to_string())] {
            let t = Fake::new(vec![first, ok("from fallback", 0.002)]);
            let ModelOut::Ok(r) = call_model(&t, &or(Some("second/model")), &msgs(), 10, 0.0).await else { panic!() };
            assert_eq!((r.text.as_str(), r.cost_usd), ("from fallback", 0.002));
            assert_eq!(t.models(), ["google/test-model", "second/model"], "the primary is tried once");
        }
    }

    #[tokio::test]
    async fn the_fallback_model_gets_two_tries_and_the_last_failure_class_wins() {
        let t = Fake::new(vec![status(500), status(500), ok("", 0.0)]);
        assert_eq!(call_model(&t, &or(Some("second/model")), &msgs(), 10, 0.0).await, ModelOut::Empty);
        assert_eq!(t.models(), ["google/test-model", "second/model", "second/model"]);
        let t = Fake::new(vec![Err("Timeout".into()), Err("Timeout".into()), status(502)]);
        assert_eq!(call_model(&t, &or(Some("second/model")), &msgs(), 10, 0.0).await, ModelOut::Failed);
    }

    #[test]
    fn shaping_demands_text_and_defaults_the_rest() {
        assert_eq!(shape_reply(&json!({ "choices": [] })), None);
        assert_eq!(shape_reply(&json!({ "choices": [{ "message": { "content": 5 } }] })), None);
        assert_eq!(shape_reply(&json!({ "choices": [{ "message": { "content": "  \n" } }] })), None);
        let r = shape_reply(&json!({ "choices": [{ "message": { "content": "x" }, "finish_reason": "length" }] })).unwrap();
        assert_eq!((r.cost_usd, r.in_tokens, r.reply["choices"][0]["finish_reason"].as_str()), (0.0, 0.0, Some("length")));
        let r = shape_reply(&json!({ "choices": [{ "message": { "content": "x" } }] })).unwrap();
        assert_eq!(r.reply["choices"][0]["finish_reason"], "stop");
    }

    mod http {
        use super::*;
        use axum::{Router, extract::State, http::HeaderMap, response::IntoResponse, routing::post};
        use std::sync::Arc;

        type Seen = Arc<Mutex<Vec<(String, String, String, Value)>>>; // (authorization, content-type, x-title, body)

        async fn serve() -> (String, Seen) {
            let seen: Seen = Arc::default();
            async fn ok(State(s): State<Seen>, h: HeaderMap, axum::Json(b): axum::Json<Value>) -> axum::Json<Value> {
                let g = |n: &str| h.get(n).and_then(|v| v.to_str().ok()).unwrap_or("").to_string();
                s.lock().unwrap().push((g("authorization"), g("content-type"), g("x-title"), b));
                axum::Json(json!({ "choices": [{ "message": { "content": "hi" } }] }))
            }
            async fn busy() -> impl IntoResponse {
                (axum::http::StatusCode::SERVICE_UNAVAILABLE, "trouble with or-secret-key")
            }
            async fn garbled() -> &'static str {
                "this is not json"
            }
            async fn slow() -> &'static str {
                tokio::time::sleep(Duration::from_secs(5)).await;
                "{}"
            }
            let app = Router::new().route("/ok", post(ok)).route("/busy", post(busy)).route("/garbled", post(garbled)).route("/slow", post(slow)).with_state(seen.clone());
            let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
            let base = format!("http://{}", listener.local_addr().unwrap());
            tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
            (base, seen)
        }

        async fn go(base: &str, path: &str, ms: u64) -> Result<Answer, String> {
            let url = format!("{base}{path}");
            HttpTransport::new().post(Post { url: &url, key: "or-secret-key", body: &json!({ "model": "m" }), timeout: Duration::from_millis(ms) }).await
        }

        #[tokio::test]
        async fn a_post_carries_the_key_in_the_header_and_the_body_as_json() {
            let (base, seen) = serve().await;
            let a = go(&base, "/ok", 2000).await.unwrap();
            assert_eq!((a.status, a.body["choices"][0]["message"]["content"].as_str()), (200, Some("hi")));
            let s = seen.lock().unwrap();
            assert_eq!(s[0].0, "Bearer or-secret-key");
            assert!(s[0].1.starts_with("application/json"));
            assert_eq!((s[0].2.as_str(), &s[0].3), ("verSHion", &json!({ "model": "m" })));
        }

        #[tokio::test]
        async fn an_error_status_is_returned_without_reading_its_body() {
            let (base, _) = serve().await;
            let a = go(&base, "/busy", 2000).await.unwrap();
            assert_eq!((a.status, a.body), (503, Value::Null), "the upstream text never travels on");
        }

        #[tokio::test]
        async fn failures_are_reduced_to_a_class_that_never_holds_the_key() {
            let (base, _) = serve().await;
            assert_eq!(go(&base, "/slow", 100).await.err().as_deref(), Some("Timeout"));
            assert_eq!(go(&base, "/garbled", 2000).await.err().as_deref(), Some("Decode"));
            assert_eq!(go("http://127.0.0.1:1", "/ok", 2000).await.err().as_deref(), Some("Connect"));
        }
    }
}
