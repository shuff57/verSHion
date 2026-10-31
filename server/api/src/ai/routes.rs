//! The relay's HTTP routes (port of `handle` in worker/src/ai/index.ts): `/ai/health`, `/ai/credits`,
//! `/ai/service` and `/ai/chat/completions`. The rules live in `policy`, `request` and `upstream`; this
//! layer reads the request, calls them in the Worker's order and writes the answer. Import jobs follow.
use super::policy::{self, Caller, CallerInput, Limits, Outcome};
use super::request::{self, MAX_BODY_BYTES};
use super::store::{AiStore, Usage};
use super::upstream::{self, ModelOut, OpenRouter, Transport};
use axum::{
    Router,
    body::Bytes,
    extract::State,
    http::{HeaderMap, HeaderName, HeaderValue, Method, StatusCode, header},
    response::{IntoResponse, Response},
    routing::any,
};
use rusqlite::Connection;
use serde_json::{Value, json};
use std::sync::{Arc, Mutex};

pub const AI_PATH: &str = "/ai/chat/completions";
pub const AI_HEALTH_PATH: &str = "/ai/health";
pub const AI_SERVICE_PATH: &str = "/ai/service";
pub const AI_CREDITS_PATH: &str = "/ai/credits";

/// Who a pool session token belongs to. The real one arrives with the auth port (phase 3).
pub trait Authenticator: Send + Sync {
    fn user(&self, headers: &HeaderMap) -> Option<String>;
}

/// Nobody is signed in: only the relay key or the keyless native app can call.
pub struct NoAuth;
impl Authenticator for NoAuth {
    fn user(&self, _: &HeaderMap) -> Option<String> {
        None
    }
}

pub struct AiState<T: Transport> {
    pub conn: Mutex<Connection>,
    pub limits: Limits,
    /// Absent: every model call answers 503 ai_not_configured.
    pub openrouter: Option<OpenRouter>,
    pub transport: T,
    pub auth: Box<dyn Authenticator>,
    pub relay_key: Option<String>,
    pub allowed_origins: Vec<String>,
    /// Signed-in accounts must spend credits through a service session (default on).
    pub credits: bool,
    pub clock: Box<dyn Fn() -> i64 + Send + Sync>,
}

impl<T: Transport> AiState<T> {
    fn now(&self) -> i64 {
        (self.clock)()
    }
    fn store<R>(&self, f: impl FnOnce(&mut AiStore) -> R) -> R {
        let mut g = self.conn.lock().unwrap_or_else(|p| p.into_inner());
        f(&mut AiStore::new(&mut g))
    }
}

pub fn system_clock() -> Box<dyn Fn() -> i64 + Send + Sync> {
    Box::new(|| std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_millis() as i64).unwrap_or(0))
}

fn respond(o: Outcome) -> Response {
    let mut res = (StatusCode::from_u16(o.status).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR), axum::Json(o.body)).into_response();
    for (name, value) in o.headers {
        if let (Ok(n), Ok(v)) = (HeaderName::from_bytes(name.to_ascii_lowercase().as_bytes()), HeaderValue::from_str(&value)) {
            res.headers_mut().insert(n, v);
        }
    }
    res
}

fn err(status: u16, code: &str, message: &str) -> Response {
    respond(Outcome::err(status, code, message))
}

/// `Allow` is the one header a 405 carries.
fn method_not_allowed(path: &str, want: &str) -> Response {
    let mut r = respond(Outcome::err(405, "method_not_allowed", &format!("Use {want} {path}.")));
    r.headers_mut().insert(header::ALLOW, HeaderValue::from_str(&format!("{want}, OPTIONS")).unwrap());
    r
}

/// Everything before the body is read: OPTIONS, the method, then who is calling.
fn prelude<T: Transport>(st: &AiState<T>, method: &Method, path: &str, want: &Method, headers: &HeaderMap) -> Result<Caller, Response> {
    if method == Method::OPTIONS {
        return Err(StatusCode::NO_CONTENT.into_response());
    }
    if method != want {
        return Err(method_not_allowed(path, want.as_str()));
    }
    let hv = |n: header::HeaderName| headers.get(n).and_then(|v| v.to_str().ok());
    let signed_in = st.auth.user(headers);
    policy::resolve_caller(&CallerInput {
        signed_in: signed_in.as_deref(),
        bearer: policy::bearer_token(hv(header::AUTHORIZATION)),
        origin: hv(header::ORIGIN),
        allowed_origins: &st.allowed_origins,
        relay_key: st.relay_key.as_deref(),
    })
    .map_err(respond)
}

/// The JSON object a POST carries, within the size cap.
fn read_object(body: &Bytes) -> Result<serde_json::Map<String, Value>, Response> {
    if body.len() > MAX_BODY_BYTES {
        return Err(err(413, "payload_too_large", "The request body was refused."));
    }
    match serde_json::from_slice::<Value>(body) {
        Ok(Value::Object(m)) => Ok(m),
        _ => Err(err(400, "bad_request", "The body is not JSON.")),
    }
}

async fn health<T: Transport + Send + Sync + 'static>(State(st): State<Arc<AiState<T>>>, method: Method) -> Response {
    if method != Method::GET {
        let mut r = err(405, "method_not_allowed", &format!("Use GET {AI_HEALTH_PATH}."));
        r.headers_mut().insert(header::ALLOW, HeaderValue::from_static("GET"));
        return r;
    }
    respond(Outcome::json(200, json!({ "ok": true, "model": st.openrouter.as_ref().map(|o| &o.model), "ai": st.openrouter.is_some(), "upstream": "openrouter" })))
}

async fn credits<T: Transport + Send + Sync + 'static>(State(st): State<Arc<AiState<T>>>, method: Method, headers: HeaderMap) -> Response {
    let who = match prelude(&st, &method, AI_CREDITS_PATH, &Method::GET, &headers) {
        Ok(w) => w,
        Err(r) => return r,
    };
    let now = st.now();
    respond(st.store(|s| policy::balance(s, &st.limits, &who, now)))
}

async fn service<T: Transport + Send + Sync + 'static>(State(st): State<Arc<AiState<T>>>, method: Method, headers: HeaderMap, body: Bytes) -> Response {
    let who = match prelude(&st, &method, AI_SERVICE_PATH, &Method::POST, &headers) {
        Ok(w) => w,
        Err(r) => return r,
    };
    if st.openrouter.is_none() {
        return err(503, "ai_not_configured", "This deployment has no OpenRouter key.");
    }
    let body = match read_object(&body) {
        Ok(b) => Value::Object(b),
        Err(r) => return r,
    };
    let now = st.now();
    respond(st.store(|s| policy::start_service(s, &st.limits, &body, &who, st.credits, now)))
}

async fn chat<T: Transport + Send + Sync + 'static>(State(st): State<Arc<AiState<T>>>, method: Method, headers: HeaderMap, body: Bytes) -> Response {
    let who = match prelude(&st, &method, AI_PATH, &Method::POST, &headers) {
        Ok(w) => w,
        Err(r) => return r,
    };
    let Some(or) = st.openrouter.as_ref() else {
        return err(503, "ai_not_configured", "This deployment has no OpenRouter key.");
    };
    let body = match read_object(&body) {
        Ok(b) => b,
        Err(r) => return r,
    };
    let messages = match request::read_messages(body.get("messages").unwrap_or(&Value::Null), st.limits.max_image_bytes) {
        Ok(m) => m,
        Err(e) => return err(400, "bad_request", &e),
    };
    // A signed-in account must be inside a service session it paid for; the session's call budget replaces the hourly cap.
    let mut session: Option<String> = None;
    if who.credited && st.credits {
        let now = st.now();
        match st.store(|s| policy::take_session(s, &who.user_id, body.get("session"), now)) {
            Ok(id) => session = Some(id),
            Err(o) => return respond(o),
        }
    }
    let now = st.now();
    if let Some(over) = st.store(|s| policy::check_quota(s, &st.limits, &who.user_id, session.is_some(), now)) {
        return respond(over);
    }
    let max_tokens = request::clamp_max_tokens(body.get("max_tokens"), st.limits.max_out_tokens);
    let temperature = request::clamp_temperature(body.get("temperature"));

    let out = upstream::call_model(&st.transport, or, &messages, max_tokens, temperature).await;
    let reply = match out {
        ModelOut::Ok(r) => r,
        ModelOut::Failed => return err(502, "model_failed", "The model call failed. Try again."),
        ModelOut::Empty => return err(502, "model_failed", "The model returned no text."),
    };
    // A usage row that fails to write must not cost the caller an answer they already paid for.
    let booked = st.store(|s| -> rusqlite::Result<()> {
        if let Some(id) = &session {
            s.call_answered(id)?;
        }
        s.record(&who.user_id, &Usage { kind: "chat", in_tokens: reply.in_tokens, out_tokens: reply.out_tokens, cost_usd: reply.cost_usd, session: session.as_deref() }, st.now())
    });
    if booked.is_err() {
        tracing::error!("ai: could not record usage");
    }
    respond(Outcome::json(200, reply.reply))
}

pub fn router<T: Transport + Send + Sync + 'static>(state: Arc<AiState<T>>) -> Router {
    Router::new()
        .route(AI_HEALTH_PATH, any(health::<T>))
        .route(AI_CREDITS_PATH, any(credits::<T>))
        .route(AI_SERVICE_PATH, any(service::<T>))
        .route(AI_PATH, any(chat::<T>))
        .with_state(state)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ai::upstream::{Answer, Post};
    use crate::db;
    use axum::body::Body;
    use axum::http::Request;
    use http_body_util::BodyExt;
    use std::collections::VecDeque;
    use std::path::Path;
    use tower::ServiceExt;

    const MON: i64 = 1_792_972_800_000;

    struct Fake {
        script: Mutex<VecDeque<Result<Answer, String>>>,
        seen: Mutex<Vec<Value>>,
    }
    impl Transport for Fake {
        async fn post(&self, p: Post<'_>) -> Result<Answer, String> {
            self.seen.lock().unwrap().push(p.body.clone());
            self.script.lock().unwrap().pop_front().unwrap_or_else(|| Ok(good("{\"ok\":true}")))
        }
    }
    fn good(content: &str) -> Answer {
        Answer {
            status: 200,
            body: json!({ "id": "r1", "choices": [{ "message": { "role": "assistant", "content": content, "reasoning": "hidden chain of thought" }, "finish_reason": "stop" }], "usage": { "prompt_tokens": 10, "completion_tokens": 20, "total_tokens": 30, "cost": 0.00011 } }),
        }
    }

    /// Signed in as whoever the `x-user` header names, like the Worker's test world.
    struct HeaderAuth;
    impl Authenticator for HeaderAuth {
        fn user(&self, h: &HeaderMap) -> Option<String> {
            h.get("x-user").and_then(|v| v.to_str().ok()).map(String::from)
        }
    }

    struct World {
        st: Arc<AiState<Fake>>,
    }
    fn world(script: Vec<Result<Answer, String>>, tweak: impl FnOnce(&mut AiState<Fake>)) -> World {
        let mut c = Connection::open_in_memory().unwrap();
        db::migrate(&mut c, &Path::new(env!("CARGO_MANIFEST_DIR")).join("../worker/migrations")).unwrap();
        let mut st = AiState {
            conn: Mutex::new(c),
            limits: Limits { weekly_credits: 10, ..Limits::default() },
            openrouter: Some(OpenRouter { model: "or/test-model".into(), ..OpenRouter::new("or-secret-key") }),
            transport: Fake { script: Mutex::new(script.into()), seen: Mutex::new(vec![]) },
            auth: Box::new(HeaderAuth),
            relay_key: Some("k".repeat(32)),
            allowed_origins: vec!["https://app.example.org".into()],
            credits: true,
            clock: Box::new(|| MON + 9 * 3_600_000),
        };
        tweak(&mut st);
        World { st: Arc::new(st) }
    }
    impl World {
        async fn send(&self, method: &str, path: &str, body: Option<Value>, headers: &[(&str, &str)]) -> (u16, Value, HeaderMap) {
            let mut b = Request::builder().method(method).uri(path);
            for (k, v) in headers {
                b = b.header(*k, *v);
            }
            let body = match body {
                Some(v) => Body::from(v.to_string()),
                None => Body::empty(),
            };
            let res = router(self.st.clone()).oneshot(b.body(body).unwrap()).await.unwrap();
            let (status, h) = (res.status().as_u16(), res.headers().clone());
            let bytes = res.into_body().collect().await.unwrap().to_bytes();
            (status, serde_json::from_slice(&bytes).unwrap_or(Value::Null), h)
        }
        async fn chat(&self, body: Value, headers: &[(&str, &str)]) -> (u16, Value, HeaderMap) {
            self.send("POST", AI_PATH, Some(body), headers).await
        }
        fn upstream_calls(&self) -> usize {
            self.st.transport.seen.lock().unwrap().len()
        }
    }
    fn say() -> Value {
        json!({ "messages": [{ "role": "user", "content": "hi" }] })
    }
    const KEY: &[(&str, &str)] = &[("authorization", "Bearer kkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkk")];

    #[tokio::test]
    async fn a_valid_request_returns_the_content_with_reasoning_and_cost_stripped() {
        let w = world(vec![], |_| {});
        let (status, body, _) = w.chat(say(), KEY).await;
        assert_eq!(status, 200);
        assert_eq!(body["choices"][0]["message"]["content"], "{\"ok\":true}");
        assert!(!body.to_string().contains("hidden") && body["usage"].get("cost").is_none());
        assert_eq!(body["usage"]["total_tokens"], 30.0);
        let sent = w.st.transport.seen.lock().unwrap()[0].clone();
        assert_eq!(sent["response_format"], json!({ "type": "json_object" }));
        assert_eq!(w.st.store(|s| s.cost_since(0).unwrap()), 0.00011, "the relay key's call is booked");
    }

    #[tokio::test]
    async fn the_client_cannot_choose_the_model_reasoning_or_provider_and_max_tokens_is_clamped() {
        let w = world(vec![], |s| s.limits.max_out_tokens = 64);
        let mut b = say();
        b["model"] = json!("evil/expensive-model");
        b["reasoning"] = json!({ "enabled": true });
        b["provider"] = json!({ "order": ["x"] });
        b["max_tokens"] = json!(999_999);
        b["temperature"] = json!(7);
        w.chat(b, KEY).await;
        let sent = w.st.transport.seen.lock().unwrap()[0].clone();
        assert_eq!(sent["model"], "or/test-model");
        assert_eq!(sent["reasoning"], json!({ "enabled": false }));
        assert_eq!(sent["provider"], json!({ "data_collection": "deny" }));
        assert_eq!((sent["max_tokens"].as_i64(), sent["temperature"].as_f64()), (Some(64), Some(1.0)));
    }

    #[tokio::test]
    async fn who_may_call() {
        let w = world(vec![], |_| {});
        let code = |r: &(u16, Value, HeaderMap)| r.1["error"]["code"].as_str().unwrap_or("").to_string();
        let none = w.chat(say(), &[]).await;
        assert_eq!((none.0, code(&none).as_str()), (401, "unauthenticated"), "a keyed relay needs a key or a session");
        let short = w.chat(say(), &[("authorization", "Bearer kkkkkkkkkkkkkkkkkkkkkkkkkkkkkkk")]).await;
        assert_eq!(short.0, 401);
        let evil = w.chat(say(), &[("origin", "https://evil.example"), ("authorization", KEY[0].1)]).await;
        assert_eq!((evil.0, code(&evil).as_str()), (403, "origin_not_allowed"));
        let keyless = world(vec![], |s| s.relay_key = None);
        assert_eq!(keyless.chat(say(), &[]).await.0, 200, "a native client may call a keyless relay");
        assert_eq!(keyless.chat(say(), &[("origin", "https://app.example.org")]).await.0, 401, "a browser may not");
    }

    #[tokio::test]
    async fn methods_paths_and_options() {
        let w = world(vec![], |_| {});
        let (s, b, h) = w.send("GET", AI_PATH, None, KEY).await;
        assert_eq!((s, b["error"]["code"].as_str(), h["allow"].to_str().unwrap()), (405, Some("method_not_allowed"), "POST, OPTIONS"));
        assert_eq!(w.send("OPTIONS", AI_PATH, None, &[]).await.0, 204);
        assert_eq!(w.send("GET", "/items", None, KEY).await.0, 404);
        assert_eq!(w.send("POST", AI_CREDITS_PATH, Some(json!({})), KEY).await.0, 405);
    }

    #[tokio::test]
    async fn bad_bodies_are_400_and_spend_no_model_call() {
        let w = world(vec![], |_| {});
        for bad in [json!({ "messages": [] }), json!({ "messages": [{ "role": "tool", "content": "x" }] }), json!({}), json!([1])] {
            assert_eq!(w.chat(bad, KEY).await.0, 400);
        }
        let req = Request::builder().method("POST").uri(AI_PATH).header("authorization", KEY[0].1).body(Body::from("not json")).unwrap();
        assert_eq!(router(w.st.clone()).oneshot(req).await.unwrap().status(), 400);
        let remote = json!({ "messages": [{ "role": "user", "content": [{ "type": "image_url", "image_url": { "url": "https://evil.example/x.png" } }] }] });
        let (s, b, _) = w.chat(remote, KEY).await;
        assert_eq!(s, 400);
        assert!(b["error"]["message"].as_str().unwrap().contains("data URI"));
        assert_eq!(w.upstream_calls(), 0);
    }

    #[tokio::test]
    async fn no_openrouter_key_is_503_and_health_says_so() {
        let w = world(vec![], |s| s.openrouter = None);
        let (s, b, _) = w.chat(say(), KEY).await;
        assert_eq!((s, b["error"]["code"].as_str()), (503, Some("ai_not_configured")));
        let (s, b, _) = w.send("GET", AI_HEALTH_PATH, None, &[]).await;
        assert_eq!((s, b), (200, json!({ "ok": true, "model": null, "ai": false, "upstream": "openrouter" })));
        let ok = world(vec![], |_| {});
        assert_eq!(ok.send("GET", AI_HEALTH_PATH, None, &[]).await.1, json!({ "ok": true, "model": "or/test-model", "ai": true, "upstream": "openrouter" }));
        assert_eq!(ok.send("POST", AI_HEALTH_PATH, None, &[]).await.0, 405);
    }

    #[tokio::test]
    async fn an_upstream_failure_is_502_and_never_echoes_the_key_or_the_provider() {
        let fail = || Ok(Answer { status: 500, body: Value::Null });
        let w = world(vec![fail(), fail()], |_| {});
        let (s, b, _) = w.chat(say(), KEY).await;
        assert_eq!((s, b["error"]["code"].as_str()), (502, Some("model_failed")));
        assert!(!b.to_string().contains("or-secret-key"));
        assert_eq!(w.upstream_calls(), 2, "tried twice");
        assert_eq!(w.st.store(|s| s.cost_since(0).unwrap()), 0.0, "a failed call records nothing");
        let empty = world(vec![Ok(Answer { status: 200, body: json!({ "choices": [] }) }), Ok(Answer { status: 200, body: json!({ "choices": [] }) })], |_| {});
        let (s, b, _) = empty.chat(say(), KEY).await;
        assert_eq!((s, b["error"]["message"].as_str()), (502, Some("The model returned no text.")));
    }

    #[tokio::test]
    async fn the_hourly_cap_applies_without_a_session_and_the_daily_cap_to_everyone() {
        let w = world(vec![], |s| s.limits.chat_per_hour = 2);
        assert_eq!(w.chat(say(), KEY).await.0, 200);
        assert_eq!(w.chat(say(), KEY).await.0, 200);
        let (s, b, h) = w.chat(say(), KEY).await;
        assert_eq!((s, b["error"]["code"].as_str(), h["retry-after"].to_str().unwrap()), (429, Some("rate_limited"), "600"));
        let rich = world(vec![], |s| s.limits.daily_usd = 0.0001);
        assert_eq!(rich.chat(say(), KEY).await.0, 200);
        assert_eq!(rich.chat(say(), KEY).await.0, 429);
    }

    #[tokio::test]
    async fn a_signed_in_account_pays_through_a_service_session_and_the_cost_is_linked_to_it() {
        let w = world(vec![], |_| {});
        let u1 = [("x-user", "u1")];
        let (s, b, _) = w.chat(say(), &u1).await;
        assert_eq!((s, b["error"]["code"].as_str()), (402, Some("session_required")));
        assert_eq!(w.chat(json!({ "messages": say()["messages"], "session": "0".repeat(32) }), &u1).await.1["error"]["code"], "session_exhausted");
        assert_eq!(w.upstream_calls(), 0);

        let (s, started, _) = w.send("POST", AI_SERVICE_PATH, Some(json!({ "service": "import", "pages": 4, "credits": 0 })), &u1).await;
        assert_eq!((s, &started["credits"], &started["left"]), (200, &json!(2), &json!(8)));
        let session = started["session"].as_str().unwrap().to_string();
        let (s, ..) = w.chat(json!({ "messages": say()["messages"], "session": session }), &u1).await;
        assert_eq!(s, 200);
        let report = w.st.store(|s| s.cost_by_service(0).unwrap());
        assert_eq!((report[0].service.as_str(), report[0].sessions, report[0].calls), ("import", 1, 1));
        let (_, bal, _) = w.send("GET", AI_CREDITS_PATH, None, &u1).await;
        assert_eq!((&bal["allowance"], &bal["used"], &bal["left"]), (&json!(10), &json!(2), &json!(8)));
        assert_eq!(w.chat(json!({ "messages": say()["messages"], "session": session }), &[("x-user", "u2")]).await.0, 402, "a session belongs to its account");
    }

    #[tokio::test]
    async fn a_session_turn_that_fails_is_still_not_an_answered_call_and_an_unused_session_is_refunded() {
        let fail = || Ok(Answer { status: 500, body: Value::Null });
        let w = world(vec![fail(), fail()], |_| {});
        let u1 = [("x-user", "u1")];
        for _ in 0..5 {
            w.send("POST", AI_SERVICE_PATH, Some(json!({ "service": "figure" })), &u1).await; // burn the week
        }
        w.st.store(|s| s.ledger("u1", 2, "purchase", "cs_1", MON, Some(50)).unwrap());
        let (_, started, _) = w.send("POST", AI_SERVICE_PATH, Some(json!({ "service": "figure" })), &u1).await;
        let id = started["session"].as_str().unwrap().to_string();
        assert_eq!(w.chat(json!({ "messages": say()["messages"], "session": id }), &u1).await.0, 502);
        assert_eq!(w.st.store(|s| s.purchased_balance("u1").unwrap()), 0, "spent while open");
    }

    #[tokio::test]
    async fn the_relay_key_and_keyless_clients_get_an_unmetered_answer_to_service_and_credits() {
        let w = world(vec![], |_| {});
        let (s, b, _) = w.send("POST", AI_SERVICE_PATH, Some(json!({ "service": "figure" })), KEY).await;
        assert_eq!((s, b), (200, json!({ "session": null, "credits": 0, "unlimited": true })));
        let (s, b, _) = w.send("GET", AI_CREDITS_PATH, None, KEY).await;
        assert_eq!((s, b["unlimited"].clone()), (200, json!(true)));
        assert_eq!(w.send("POST", AI_SERVICE_PATH, Some(json!({ "service": "nope" })), KEY).await.0, 400);
    }
}
