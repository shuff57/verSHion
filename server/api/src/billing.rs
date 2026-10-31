//! Buying credits: Stripe PaymentIntents for the Payment Element on the web page, and the webhook that credits
//! (and takes back) the ledger (port of worker/src/billing/index.ts). Stripe's REST API over reqwest and its
//! HMAC signature over `hmac`/`sha2`.
//!
//! The flag (`enabled`) gates BUYING only (`/billing/packs` and `/billing/intent` answer 404 when off). The webhook
//! stays live whenever its secret is set, so a refund or dispute on an earlier sale is never missed.
use crate::ai::policy::{Outcome, same_secret};
use crate::ai::routes::Authenticator;
use crate::ai::store::AiStore;
use crate::body::read_bounded;
use crate::ratelimit::{self, Rule};
use axum::{
    Router,
    body::Body,
    extract::State,
    http::{HeaderMap, HeaderName, HeaderValue, Method, StatusCode, Uri},
    response::{IntoResponse, Response},
    routing::any,
};
use hmac::{Hmac, Mac};
use rusqlite::Connection;
use serde_json::{Value, json};
use sha2::Sha256;
use std::sync::{Arc, Mutex};

pub const PACKS_PATH: &str = "/billing/packs";
pub const INTENT_PATH: &str = "/billing/intent";
pub const WEBHOOK_PATH: &str = "/billing/webhook";

const STRIPE_API: &str = "https://api.stripe.com/v1";
const STRIPE_VERSION: &str = "2025-12-15.clover";
const WEBHOOK_TOLERANCE_S: i64 = 300;
const WEBHOOK_BODY_MAX: usize = 64 * 1024;
const INTENT_BODY_MAX: usize = 1024;

#[derive(Clone, Debug, PartialEq)]
pub struct Pack {
    pub id: &'static str,
    /// Price in whole US dollars.
    pub usd: i64,
    pub credits: i64,
}

/// 25 credits per dollar (a credit is 4 cents; handwriting is a credit a page), with a small bonus on the larger packs.
pub const CREDITS_PER_USD: i64 = 25;
pub const DEFAULT_PACKS: [Pack; 3] = [Pack { id: "small", usd: 5, credits: 125 }, Pack { id: "medium", usd: 15, credits: 400 }, Pack { id: "large", usd: 40, credits: 1100 }];

pub struct StripePost {
    pub url: String,
    pub key: String,
    pub form: Vec<(String, String)>,
}

/// The one outbound call, behind a trait so tests script Stripe's answers.
pub trait StripeTransport: Send + Sync {
    fn post(&self, p: StripePost) -> impl std::future::Future<Output = Result<(u16, Value), String>> + Send;
}

pub struct HttpStripe {
    client: reqwest::Client,
}

impl HttpStripe {
    pub fn new() -> Self {
        Self { client: reqwest::Client::builder().user_agent("vershion-api").timeout(std::time::Duration::from_secs(30)).build().expect("build the http client") }
    }
}

impl Default for HttpStripe {
    fn default() -> Self {
        Self::new()
    }
}

fn form_encode(pairs: &[(String, String)]) -> String {
    fn enc(s: &str) -> String {
        s.bytes().map(|b| if b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_' | b'.' | b'~') { (b as char).to_string() } else { format!("%{b:02X}") }).collect()
    }
    pairs.iter().map(|(k, v)| format!("{}={}", enc(k), enc(v))).collect::<Vec<_>>().join("&")
}

impl StripeTransport for HttpStripe {
    async fn post(&self, p: StripePost) -> Result<(u16, Value), String> {
        let res = self
            .client
            .post(&p.url)
            .header("authorization", format!("Bearer {}", p.key))
            .header("content-type", "application/x-www-form-urlencoded")
            .header("stripe-version", STRIPE_VERSION)
            .body(form_encode(&p.form))
            .send()
            .await
            .map_err(|e| if e.is_timeout() { "Timeout".to_string() } else { "Request".to_string() })?;
        let status = res.status().as_u16();
        let body = res.json::<Value>().await.map_err(|_| "Decode".to_string())?;
        Ok((status, body))
    }
}

pub struct BillingState<S: StripeTransport> {
    pub conn: Mutex<Connection>,
    /// Off by default: buying is unavailable (404).
    pub enabled: bool,
    pub stripe_key: Option<String>,
    pub webhook_secret: Option<String>,
    pub publishable_key: Option<String>,
    pub auth: Box<dyn Authenticator>,
    pub transport: S,
    pub clock: Box<dyn Fn() -> i64 + Send + Sync>,
    /// Payments one account may start per hour (default 10).
    pub intents_per_hour: i64,
    pub packs: Vec<Pack>,
    /// Credits per dollar for a custom amount, and its whole-dollar range.
    pub credits_per_usd: i64,
    pub custom_min_usd: i64,
    pub custom_max_usd: i64,
}

impl<S: StripeTransport> BillingState<S> {
    pub fn new(conn: Connection, transport: S, auth: Box<dyn Authenticator>, clock: Box<dyn Fn() -> i64 + Send + Sync>) -> Self {
        Self {
            conn: Mutex::new(conn),
            enabled: false,
            stripe_key: None,
            webhook_secret: None,
            publishable_key: None,
            auth,
            transport,
            clock,
            intents_per_hour: 10,
            packs: DEFAULT_PACKS.to_vec(),
            credits_per_usd: CREDITS_PER_USD,
            custom_min_usd: 5,
            custom_max_usd: 200,
        }
    }

    fn now(&self) -> i64 {
        (self.clock)()
    }

    /// The guard is never held across an `.await`.
    pub fn with_conn<R>(&self, f: impl FnOnce(&mut Connection) -> R) -> R {
        f(&mut self.conn.lock().unwrap_or_else(|p| p.into_inner()))
    }

    /// What a payment of `cents` is worth: a pack's credits when the amount is exactly a pack's price, else the custom rate.
    fn credits_for(&self, cents: i64, pack: Option<&str>) -> Option<i64> {
        if let Some(id) = pack {
            let p = self.packs.iter().find(|p| p.id == id)?;
            return (p.usd * 100 == cents).then_some(p.credits);
        }
        if cents % 100 != 0 {
            return None;
        }
        let usd = cents / 100;
        (usd >= self.custom_min_usd && usd <= self.custom_max_usd).then_some(usd * self.credits_per_usd)
    }
}

fn hex(b: &[u8]) -> String {
    b.iter().map(|x| format!("{x:02x}")).collect()
}

/// The signature Stripe sends for `payload` at time `t`: HMAC-SHA256 of `t.payload`, hex.
pub fn stripe_signature(secret: &str, t: i64, payload: &[u8]) -> String {
    let mut mac = Hmac::<Sha256>::new_from_slice(secret.as_bytes()).expect("hmac takes any key length");
    mac.update(format!("{t}.").as_bytes());
    mac.update(payload);
    hex(&mac.finalize().into_bytes())
}

/// Verifies a `Stripe-Signature` header (`t=...,v1=...`) against the raw body.
pub fn verify_stripe(secret: &str, header: &str, payload: &[u8], now_s: i64) -> bool {
    let mut t: Option<i64> = None;
    let mut sigs: Vec<&str> = vec![];
    for part in header.split(',') {
        let mut kv = part.trim().split('=');
        let (k, v) = (kv.next().unwrap_or(""), kv.next().unwrap_or(""));
        if k == "t" {
            t = v.parse().ok();
        } else if k == "v1" && !v.is_empty() {
            sigs.push(v);
        }
    }
    let Some(t) = t else { return false };
    if (now_s - t).abs() > WEBHOOK_TOLERANCE_S || sigs.is_empty() {
        return false;
    }
    let want = stripe_signature(secret, t, payload);
    sigs.iter().any(|s| same_secret(s, &want))
}

fn respond(o: Outcome) -> Response {
    let mut res = (StatusCode::from_u16(o.status).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR), axum::Json(o.body)).into_response();
    res.headers_mut().insert("cache-control", HeaderValue::from_static("no-store"));
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

impl<S: StripeTransport> BillingState<S> {
    /// `POST /billing/intent {pack}` or `{amount_usd}`: creates the PaymentIntent for the Payment Element.
    async fn intent(&self, headers: &HeaderMap, body: Body, user: &str) -> Response {
        let bytes = match read_bounded(headers, body, INTENT_BODY_MAX, true).await {
            Ok(b) => b,
            Err(o) => return respond(o),
        };
        let parsed: Option<Value> = serde_json::from_slice(&bytes).ok().filter(|v: &Value| v.is_object() || v.is_array());
        let Some(parsed) = parsed else { return err(400, "bad_request", "Send a JSON body.") };
        let (cents, pack): (i64, Option<String>) = if let Some(id) = parsed.get("pack").and_then(Value::as_str) {
            let Some(p) = self.packs.iter().find(|p| p.id == id) else { return err(400, "bad_request", "Unknown pack.") };
            (p.usd * 100, Some(p.id.to_string()))
        } else if let Some(usd) = parsed.get("amount_usd").and_then(Value::as_f64).filter(|n| n.fract() == 0.0 && n.abs() < 1e9) {
            (usd as i64 * 100, None)
        } else {
            return err(400, "bad_request", "Choose a pack or a whole-dollar amount.");
        };
        let Some(credits) = self.credits_for(cents, pack.as_deref()) else {
            return err(400, "bad_amount", &format!("Choose between ${} and ${}, in whole dollars.", self.custom_min_usd, self.custom_max_usd));
        };
        // Counted only for a valid request: each one creates an object at Stripe, and a card-testing script would spam them.
        let rule = Rule { limit: self.intents_per_hour, window_seconds: 3600 };
        let now = self.now();
        match self.with_conn(|c| ratelimit::hit(c, &format!("billing:intent:{user}"), rule, now)) {
            Ok(v) if !v.allowed => {
                let mut o = Outcome::err(429, "rate_limited", "Too many payment attempts. Try again later.");
                o.headers.push(("retry-after", v.retry_after.to_string()));
                return respond(o);
            }
            Ok(_) => {}
            Err(_) => {
                tracing::error!("billing: rate limit store failed");
                return err(502, "payment_unavailable", "Could not start the payment. Try again.");
            }
        }
        let mut form: Vec<(String, String)> = vec![
            ("amount".into(), cents.to_string()),
            ("currency".into(), "usd".into()),
            ("description".into(), format!("{credits} AI credits")),
            ("automatic_payment_methods[enabled]".into(), "true".into()),
            ("metadata[purchase_type]".into(), "ai_credits".into()),
            ("metadata[user_id]".into(), user.to_string()),
            ("metadata[credits]".into(), credits.to_string()),
        ];
        if let Some(p) = &pack {
            form.push(("metadata[pack]".into(), p.clone()));
        }
        let key = self.stripe_key.clone().unwrap_or_default();
        let secret = match self.transport.post(StripePost { url: format!("{STRIPE_API}/payment_intents"), key, form }).await {
            Ok((status, v)) if (200..300).contains(&status) => v.get("client_secret").and_then(Value::as_str).map(str::to_string),
            _ => None,
        };
        match secret {
            Some(s) => respond(Outcome::json(200, json!({ "client_secret": s, "publishable_key": self.publishable_key, "credits": credits, "amount_usd": cents / 100 }))),
            None => {
                tracing::error!("billing: could not create a payment");
                err(502, "payment_unavailable", "Could not start the payment. Try again.")
            }
        }
    }

    /// Credits a succeeded payment once; the amount is re-checked against what was priced, never trusted from metadata alone.
    fn fulfil(&self, pi: &Value) -> rusqlite::Result<()> {
        let meta = pi.get("metadata");
        let m = |k: &str| meta.and_then(|m| m.get(k)).and_then(Value::as_str);
        let (Some("ai_credits"), Some(user)) = (m("purchase_type"), m("user_id").filter(|u| !u.is_empty())) else { return Ok(()) };
        let received = pi.get("amount_received").and_then(Value::as_i64);
        let credits = received.and_then(|c| self.credits_for(c, m("pack")));
        let id = pi.get("id").and_then(Value::as_str).unwrap_or("undefined");
        let Some(credits) = credits.filter(|c| Some(c.to_string().as_str()) == m("credits")) else {
            tracing::error!("billing: payment does not match a price, not credited");
            return Ok(());
        };
        let now = self.now();
        self.with_conn(|c| {
            let s = AiStore::new(c);
            // A payment that lands after the account was deleted credits nothing (the row would outlive the erasure);
            // the charge is logged for a manual refund in Stripe.
            if s.account_deleted(user)? {
                tracing::error!("billing: payment {id} is for a deleted account, refund it in Stripe");
                return Ok(());
            }
            s.ledger(user, credits, "purchase", id, now, received)?;
            Ok(())
        })
    }

    /// Takes back credits for a refund or dispute: the share of the purchase that was returned, minus what earlier
    /// events already took back. May leave the balance negative, which blocks purchased spending.
    fn reverse(&self, pi_id: &str, returned_cents: Option<i64>, tag: &str) -> rusqlite::Result<()> {
        let now = self.now();
        self.with_conn(|c| {
            let s = AiStore::new(c);
            let Some(bought) = s.purchase(pi_id)? else { return Ok(()) };
            let Some(returned) = returned_cents.filter(|r| *r > 0 && bought.cents > 0) else { return Ok(()) };
            let share = returned.min(bought.cents);
            let target = bought.credits.min((bought.credits * share + bought.cents - 1) / bought.cents);
            let owed = target - s.reversed_for(&format!("{pi_id}#"))?;
            if owed > 0 {
                s.ledger(&bought.principal, -owed, "reversal", &format!("{pi_id}#{tag}"), now, None)?;
            }
            Ok(())
        })
    }

    async fn webhook(&self, method: &Method, headers: &HeaderMap, body: Body) -> Response {
        if method != Method::POST {
            return err(405, "method_not_allowed", "Use POST.");
        }
        let Some(secret) = &self.webhook_secret else { return err(503, "billing_not_configured", "Billing is not configured.") };
        let payload = match read_bounded(headers, body, WEBHOOK_BODY_MAX, true).await {
            Ok(b) => b,
            Err(o) => return respond(o),
        };
        let sig = headers.get("stripe-signature").and_then(|v| v.to_str().ok());
        if !sig.is_some_and(|s| verify_stripe(secret, s, &payload, self.now() / 1000)) {
            return err(400, "bad_signature", "Invalid signature.");
        }
        let Ok(event) = serde_json::from_slice::<Value>(&payload) else { return err(400, "bad_request", "Invalid body.") };
        let o = event.pointer("/data/object").cloned().unwrap_or(Value::Null);
        let pi_id = o.get("payment_intent").and_then(Value::as_str).unwrap_or("");
        let done = match event.get("type").and_then(Value::as_str) {
            Some("payment_intent.succeeded") => self.fulfil(&o),
            Some("charge.refunded") => {
                let n = o.get("amount_refunded").and_then(Value::as_i64);
                self.reverse(pi_id, n, &format!("refund:{}", n.map_or("NaN".to_string(), |n| n.to_string())))
            }
            Some("charge.dispute.created") => self.reverse(pi_id, o.get("amount").and_then(Value::as_i64), &format!("dispute:{}", o.get("id").and_then(Value::as_str).unwrap_or("undefined"))),
            _ => Ok(()),
        };
        match done {
            Ok(()) => respond(Outcome::json(200, json!({ "received": true }))),
            Err(_) => {
                // Not acknowledged: Stripe redelivers, and every handler above is idempotent.
                tracing::error!("billing: webhook handler failed");
                err(500, "handler_failed", "Try again.")
            }
        }
    }

    async fn handle(&self, method: Method, uri: &Uri, headers: HeaderMap, body: Body) -> Response {
        let path = uri.path();
        if path == WEBHOOK_PATH {
            return self.webhook(&method, &headers, body).await;
        }
        if method == Method::OPTIONS {
            return StatusCode::NO_CONTENT.into_response();
        }
        if !self.enabled || self.stripe_key.is_none() {
            return err(404, "not_found", "Not found.");
        }
        let want = if path == PACKS_PATH { Method::GET } else { Method::POST };
        if method != want {
            return err(405, "method_not_allowed", &format!("Use {want} {path}."));
        }
        let Some(user) = self.auth.user(&headers) else { return err(401, "unauthenticated", "Sign in first.") };
        if path == PACKS_PATH {
            let packs: Vec<Value> = self.packs.iter().map(|p| json!({ "id": p.id, "usd": p.usd, "credits": p.credits })).collect();
            return respond(Outcome::json(200, json!({ "packs": packs, "custom": { "min_usd": self.custom_min_usd, "max_usd": self.custom_max_usd, "credits_per_usd": self.credits_per_usd }, "publishable_key": self.publishable_key })));
        }
        self.intent(&headers, body, &user).await
    }
}

async fn serve<S: StripeTransport + 'static>(State(st): State<Arc<BillingState<S>>>, method: Method, uri: Uri, headers: HeaderMap, body: Body) -> Response {
    st.handle(method, &uri, headers, body).await
}

pub fn router<S: StripeTransport + 'static>(state: Arc<BillingState<S>>) -> Router {
    Router::new().route(PACKS_PATH, any(serve::<S>)).route(INTENT_PATH, any(serve::<S>)).route(WEBHOOK_PATH, any(serve::<S>)).with_state(state)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db;
    use axum::http::Request;
    use http_body_util::BodyExt;
    use std::collections::VecDeque;
    use std::path::Path;
    use tower::ServiceExt;

    const NOW: i64 = 1_793_361_600_000; // 2026-10-30T12:00:00Z
    const SECRET: &str = "whsec_test_secret";

    struct Fake {
        sent: Mutex<Vec<StripePost>>,
        script: Mutex<VecDeque<Result<(u16, Value), String>>>,
    }
    impl StripeTransport for Fake {
        async fn post(&self, p: StripePost) -> Result<(u16, Value), String> {
            self.sent.lock().unwrap().push(p);
            self.script.lock().unwrap().pop_front().unwrap_or_else(|| Ok((200, json!({ "id": "pi_new", "client_secret": "pi_new_secret_x" }))))
        }
    }
    struct HeaderAuth;
    impl Authenticator for HeaderAuth {
        fn user(&self, h: &HeaderMap) -> Option<String> {
            h.get("x-user").and_then(|v| v.to_str().ok()).map(str::to_string)
        }
    }

    struct World {
        st: Arc<BillingState<Fake>>,
        _dir: tempfile::TempDir,
    }

    fn world(tweak: impl FnOnce(&mut BillingState<Fake>)) -> World {
        let dir = tempfile::tempdir().unwrap();
        let mut conn = db::open(&dir.path().join("t.db")).unwrap();
        db::migrate(&mut conn, &Path::new(env!("CARGO_MANIFEST_DIR")).join("../worker/migrations")).unwrap();
        let mut st = BillingState::new(conn, Fake { sent: Mutex::new(vec![]), script: Mutex::new(VecDeque::new()) }, Box::new(HeaderAuth), Box::new(|| NOW));
        st.enabled = true;
        st.stripe_key = Some("sk_test_x".into());
        st.webhook_secret = Some(SECRET.into());
        st.publishable_key = Some("pk_test_x".into());
        tweak(&mut st);
        World { st: Arc::new(st), _dir: dir }
    }

    impl World {
        async fn call(&self, method: &str, path: &str, body: Option<String>, headers: &[(&str, String)]) -> (u16, Value, HeaderMap) {
            let mut req = Request::builder().method(method).uri(path);
            for (k, v) in headers {
                req = req.header(*k, v);
            }
            let body = match body {
                Some(b) => Body::from(b),
                None => Body::empty(),
            };
            let res = router(self.st.clone()).oneshot(req.body(body).unwrap()).await.unwrap();
            let (status, h) = (res.status().as_u16(), res.headers().clone());
            let bytes = res.into_body().collect().await.unwrap().to_bytes();
            (status, serde_json::from_slice(&bytes).unwrap_or(Value::Null), h)
        }
        async fn send(&self, path: &str, body: Option<Value>, user: Option<&str>, method: &str) -> (u16, Value, HeaderMap) {
            let text = body.map(|b| b.to_string());
            let mut h: Vec<(&str, String)> = vec![("content-type", "application/json".into())];
            if let Some(t) = &text {
                h.push(("content-length", t.len().to_string()));
            }
            if let Some(u) = user {
                h.push(("x-user", u.into()));
            }
            self.call(method, path, text, &h).await
        }
        async fn post(&self, path: &str, body: Value, user: &str) -> (u16, Value, HeaderMap) {
            self.send(path, Some(body), Some(user), "POST").await
        }
        /// A webhook as Stripe sends it: the raw body, signed with the endpoint secret.
        async fn hook(&self, kind: &str, object: Value) -> (u16, Value) {
            self.hook_with(kind, object, SECRET, NOW / 1000).await
        }
        async fn hook_with(&self, kind: &str, object: Value, secret: &str, t: i64) -> (u16, Value) {
            let payload = json!({ "id": "evt_1", "type": kind, "data": { "object": object } }).to_string();
            let sig = format!("t={t},v1={}", stripe_signature(secret, t, payload.as_bytes()));
            let (s, b, _) = self.call("POST", WEBHOOK_PATH, Some(payload.clone()), &[("stripe-signature", sig), ("content-length", payload.len().to_string())]).await;
            (s, b)
        }
        fn balance(&self, user: &str) -> i64 {
            self.st.with_conn(|c| AiStore::new(c).purchased_balance(user).unwrap())
        }
        fn sent(&self) -> usize {
            self.st.transport.sent.lock().unwrap().len()
        }
    }

    /// The $15 pack: 400 credits.
    fn paid(id: &str, user: &str, pack: Option<&str>, cents: i64, credits: i64) -> Value {
        let mut meta = json!({ "purchase_type": "ai_credits", "user_id": user, "credits": credits.to_string() });
        if let Some(p) = pack {
            meta["pack"] = json!(p);
        }
        json!({ "id": id, "amount_received": cents, "metadata": meta })
    }
    fn paid15() -> Value {
        paid("pi_1", "u1", Some("medium"), 1500, 400)
    }

    #[test]
    fn the_default_packs_are_25_credits_a_dollar_or_better() {
        for p in &DEFAULT_PACKS {
            assert!(p.credits >= p.usd * 25, "{}", p.id);
        }
        assert_eq!(DEFAULT_PACKS.iter().map(|p| (p.usd, p.credits)).collect::<Vec<_>>(), [(5, 125), (15, 400), (40, 1100)]);
    }

    #[test]
    fn the_signature_is_the_documented_hmac_and_garbage_is_refused() {
        // HMAC-SHA256("whsec_test_secret", "1000.{}") computed independently.
        let sig = stripe_signature(SECRET, 1000, b"{}");
        assert_eq!(sig.len(), 64);
        assert!(sig.bytes().all(|b| b.is_ascii_hexdigit() && !b.is_ascii_uppercase()));
        assert!(verify_stripe(SECRET, &format!("t=1000,v1={sig}"), b"{}", 1000));
        assert!(verify_stripe(SECRET, &format!("t=1000,v1=bad,v1={sig}"), b"{}", 1100), "any listed v1 may match");
        assert!(!verify_stripe(SECRET, &format!("t=1000,v1={sig}"), b"{}", 1000 + 301), "stale");
        assert!(!verify_stripe(SECRET, &format!("t=1000,v1={sig}"), b"{ }", 1000), "a changed body");
        assert!(!verify_stripe("other", &format!("t=1000,v1={sig}"), b"{}", 1000));
        for bad in ["garbage", "", "t=1000", "v1=abc", "t=x,v1=abc", "t=1000,v1="] {
            assert!(!verify_stripe(SECRET, bad, b"{}", 1000), "{bad:?}");
        }
        // A known vector (RFC 4231 test case 2 shape): key "Jefe", data "what do ya want for nothing?".
        let mut mac = Hmac::<Sha256>::new_from_slice(b"Jefe").unwrap();
        mac.update(b"what do ya want for nothing?");
        assert_eq!(hex(&mac.finalize().into_bytes()), "5bdcc146bf60754e6a042426089575c75a003f089d2739839dec58b964ec3843");
    }

    #[test]
    fn form_bodies_are_percent_encoded() {
        let f = form_encode(&[("metadata[user_id]".into(), "a b&c".into()), ("amount".into(), "1500".into())]);
        assert_eq!(f, "metadata%5Buser_id%5D=a%20b%26c&amount=1500");
    }

    #[tokio::test]
    async fn buying_is_off_by_default_and_the_webhook_still_answers() {
        let w = world(|s| s.enabled = false);
        assert_eq!(w.send(PACKS_PATH, None, Some("u1"), "GET").await.0, 404);
        assert_eq!(w.post(INTENT_PATH, json!({ "pack": "small" }), "u1").await.0, 404);
        assert_eq!(w.hook("payment_intent.succeeded", paid15()).await.0, 200);
        assert_eq!(w.balance("u1"), 400, "a sale made before the flag went off is still credited");
    }

    #[tokio::test]
    async fn packs_and_intents_need_a_signed_in_account_and_the_right_method() {
        let w = world(|_| {});
        assert_eq!(w.send(PACKS_PATH, None, None, "GET").await.0, 401);
        assert_eq!(w.post(INTENT_PATH, json!({ "pack": "small" }), "u1").await.0, 200);
        assert_eq!(w.send(INTENT_PATH, Some(json!({ "pack": "small" })), None, "POST").await.0, 401);
        let (s, b, _) = w.send(PACKS_PATH, None, Some("u1"), "GET").await;
        assert_eq!((s, b["publishable_key"].as_str(), b["packs"].as_array().unwrap().len(), b["custom"]["credits_per_usd"].as_i64()), (200, Some("pk_test_x"), 3, Some(25)));
        assert_eq!(b["packs"][1], json!({ "id": "medium", "usd": 15, "credits": 400 }));
        let (s, b, _) = w.send(PACKS_PATH, None, Some("u1"), "POST").await;
        assert_eq!((s, b["error"]["message"].as_str()), (405, Some("Use GET /billing/packs.")));
        assert_eq!(w.send(INTENT_PATH, None, Some("u1"), "OPTIONS").await.0, 204);
        let no_key = world(|s| s.stripe_key = None);
        assert_eq!(no_key.send(PACKS_PATH, None, Some("u1"), "GET").await.0, 404);
    }

    #[tokio::test]
    async fn a_pack_intent_is_priced_by_the_server_and_tagged_with_the_account() {
        let w = world(|_| {});
        let (s, b, h) = w.post(INTENT_PATH, json!({ "pack": "medium", "amount_usd": 1, "credits": 99999 }), "u1").await;
        assert_eq!(s, 200);
        assert_eq!((b["client_secret"].as_str(), b["credits"].as_i64(), b["amount_usd"].as_i64()), (Some("pi_new_secret_x"), Some(400), Some(15)));
        assert_eq!(h["cache-control"], "no-store");
        let sent = w.st.transport.sent.lock().unwrap();
        let get = |k: &str| sent[0].form.iter().find(|(n, _)| n == k).map(|(_, v)| v.as_str());
        assert_eq!(sent[0].url, "https://api.stripe.com/v1/payment_intents");
        assert_eq!(sent[0].key, "sk_test_x");
        assert_eq!([get("amount"), get("currency"), get("metadata[user_id]"), get("metadata[credits]"), get("metadata[pack]")], [Some("1500"), Some("usd"), Some("u1"), Some("400"), Some("medium")]);
    }

    #[tokio::test]
    async fn a_custom_amount_is_whole_dollars_inside_the_range_at_the_custom_rate() {
        let w = world(|_| {});
        let (s, b, _) = w.post(INTENT_PATH, json!({ "amount_usd": 20 }), "u1").await;
        assert_eq!((s, b["credits"].as_i64()), (200, Some(500)));
        for bad in [json!({ "amount_usd": 4 }), json!({ "amount_usd": 201 }), json!({ "amount_usd": 7.5 }), json!({ "amount_usd": "9" }), json!({ "amount_usd": -5 }), json!({ "pack": "nope" }), json!({})] {
            assert_eq!(w.post(INTENT_PATH, bad.clone(), "u1").await.0, 400, "{bad}");
        }
        let (_, b, _) = w.post(INTENT_PATH, json!({ "amount_usd": 4 }), "u1").await;
        assert_eq!(b["error"]["message"], "Choose between $5 and $200, in whole dollars.");
        assert_eq!(w.sent(), 1, "nothing invalid reached Stripe");
    }

    #[tokio::test]
    async fn a_body_without_a_content_length_or_past_the_cap_is_refused_never_read() {
        let w = world(|_| {});
        let (s, _, _) = w.call("POST", INTENT_PATH, Some("{}".into()), &[("x-user", "u1".into())]).await;
        assert_eq!(s, 411);
        let (s, b, _) = w.post(INTENT_PATH, json!({ "pack": "small", "pad": "x".repeat(2000) }), "u1").await;
        assert_eq!((s, b["error"]["code"].as_str()), (413, Some("body_too_large")));
        assert_eq!(w.sent(), 0);
    }

    #[tokio::test]
    async fn creating_payments_is_rate_limited_per_account_and_invalid_requests_do_not_count() {
        let w = world(|s| s.intents_per_hour = 2);
        for _ in 0..5 {
            assert_eq!(w.post(INTENT_PATH, json!({ "amount_usd": 1 }), "u1").await.0, 400);
        }
        assert_eq!(w.post(INTENT_PATH, json!({ "pack": "small" }), "u1").await.0, 200);
        assert_eq!(w.post(INTENT_PATH, json!({ "pack": "small" }), "u1").await.0, 200);
        let (s, b, h) = w.post(INTENT_PATH, json!({ "pack": "small" }), "u1").await;
        assert_eq!((s, b["error"]["code"].as_str()), (429, Some("rate_limited")));
        assert!(h["retry-after"].to_str().unwrap().parse::<i64>().unwrap() > 0);
        assert_eq!(w.post(INTENT_PATH, json!({ "pack": "small" }), "u2").await.0, 200, "another account is unaffected");
        assert_eq!(w.sent(), 3);
    }

    #[tokio::test]
    async fn a_stripe_failure_is_a_clean_502() {
        for answer in [Ok((500, json!({}))), Err("Timeout".to_string()), Ok((200, json!({ "id": "pi_x" })))] {
            let w = world(|_| {});
            w.st.transport.script.lock().unwrap().push_back(answer);
            let (s, b, _) = w.post(INTENT_PATH, json!({ "pack": "small" }), "u1").await;
            assert_eq!((s, b["error"]["code"].as_str()), (502, Some("payment_unavailable")));
        }
    }

    #[tokio::test]
    async fn signatures_wrong_secret_stale_time_and_a_missing_header_are_refused() {
        let w = world(|_| {});
        assert_eq!(w.hook_with("payment_intent.succeeded", paid15(), "whsec_other", NOW / 1000).await.0, 400);
        assert_eq!(w.hook_with("payment_intent.succeeded", paid15(), SECRET, NOW / 1000 - 3600).await.0, 400);
        let (s, b, _) = w.call("POST", WEBHOOK_PATH, Some("{}".into()), &[("content-length", "2".into())]).await;
        assert_eq!((s, b["error"]["code"].as_str()), (400, Some("bad_signature")));
        assert_eq!(w.balance("u1"), 0);
    }

    #[tokio::test]
    async fn a_webhook_with_no_secret_is_503_and_only_post_is_allowed() {
        let w = world(|s| s.webhook_secret = None);
        let (s, b, _) = w.call("POST", WEBHOOK_PATH, Some("{}".into()), &[]).await;
        assert_eq!((s, b["error"]["code"].as_str()), (503, Some("billing_not_configured")));
        assert_eq!(world(|_| {}).call("GET", WEBHOOK_PATH, None, &[]).await.0, 405);
    }

    #[tokio::test]
    async fn a_payment_credits_once_however_often_stripe_redelivers() {
        let w = world(|_| {});
        assert_eq!(w.hook("payment_intent.succeeded", paid15()).await.0, 200);
        assert_eq!(w.hook("payment_intent.succeeded", paid15()).await.0, 200);
        assert_eq!(w.balance("u1"), 400);
        let cents: i64 = w.st.with_conn(|c| c.query_row("SELECT cents FROM credit_ledger WHERE ref = 'pi_1'", [], |r| r.get(0)).unwrap());
        assert_eq!(cents, 1500);
    }

    #[tokio::test]
    async fn an_amount_that_matches_no_price_is_not_credited() {
        let w = world(|_| {});
        w.hook("payment_intent.succeeded", paid("pi_a", "u1", Some("medium"), 500, 400)).await; // a $5 payment claiming the $15 pack
        w.hook("payment_intent.succeeded", paid("pi_b", "u1", None, 1500, 9999)).await; // metadata claims too many credits
        w.hook("payment_intent.succeeded", paid("pi_c", "u1", None, 150, 37)).await; // $1.50 is below the minimum
        w.hook("payment_intent.succeeded", json!({ "id": "pi_f", "metadata": { "purchase_type": "ai_credits", "user_id": "u1", "credits": "400", "pack": "medium" } })).await; // no amount
        assert_eq!(w.balance("u1"), 0);
    }

    #[tokio::test]
    async fn payments_that_are_not_credit_sales_are_ignored() {
        let w = world(|_| {});
        w.hook("payment_intent.succeeded", json!({ "id": "pi_d", "amount_received": 500, "metadata": { "purchase_type": "donation" } })).await;
        w.hook("payment_intent.succeeded", json!({ "id": "pi_e", "amount_received": 500, "metadata": {} })).await;
        assert_eq!(w.balance("u1"), 0);
    }

    #[tokio::test]
    async fn a_refund_takes_credits_back_pro_rata_once_per_amount_and_a_full_refund_finishes_the_job() {
        let w = world(|_| {});
        w.hook("payment_intent.succeeded", paid15()).await;
        let refund = |n: i64| json!({ "id": "ch_1", "payment_intent": "pi_1", "amount_refunded": n });
        w.hook("charge.refunded", refund(500)).await; // a third of $15
        assert_eq!(w.balance("u1"), 400 - 134, "a third of 400, rounded up for the house");
        w.hook("charge.refunded", refund(500)).await;
        assert_eq!(w.balance("u1"), 400 - 134, "a redelivery changes nothing");
        w.hook("charge.refunded", refund(1500)).await;
        assert_eq!(w.balance("u1"), 0);
        w.hook("charge.refunded", refund(1500)).await;
        assert_eq!(w.balance("u1"), 0, "never below zero from the same payment");
    }

    #[tokio::test]
    async fn a_dispute_after_a_refund_takes_back_only_what_is_left() {
        let w = world(|_| {});
        w.hook("payment_intent.succeeded", paid15()).await;
        w.hook("charge.refunded", json!({ "id": "ch_1", "payment_intent": "pi_1", "amount_refunded": 750 })).await;
        w.hook("charge.dispute.created", json!({ "id": "dp_1", "payment_intent": "pi_1", "amount": 1500 })).await;
        assert_eq!(w.balance("u1"), 0, "never below zero from the same payment");
    }

    #[tokio::test]
    async fn credits_already_spent_make_the_balance_negative_which_blocks_purchased_spending() {
        let w = world(|_| {});
        w.hook("payment_intent.succeeded", paid15()).await;
        w.st.with_conn(|c| AiStore::new(c).ledger("u1", -150, "grant", "spent-by-hand", NOW, None).unwrap());
        w.hook("charge.dispute.created", json!({ "id": "dp_2", "payment_intent": "pi_1", "amount": 1500 })).await;
        assert_eq!(w.balance("u1"), -150);
    }

    #[tokio::test]
    async fn a_refund_of_a_payment_that_is_not_ours_is_acknowledged_and_ignored() {
        let w = world(|_| {});
        assert_eq!(w.hook("charge.refunded", json!({ "id": "ch_x", "payment_intent": "pi_unknown", "amount_refunded": 500 })).await.0, 200);
        assert_eq!(w.hook("charge.refunded", json!({ "id": "ch_y", "amount_refunded": 500 })).await.0, 200);
        assert_eq!(w.hook("some.other.event", json!({})).await.0, 200);
    }

    #[tokio::test]
    async fn a_database_failure_is_not_acknowledged_so_stripe_redelivers() {
        let w = world(|_| {});
        w.st.with_conn(|c| c.execute_batch("DROP TABLE credit_ledger;").unwrap());
        assert_eq!(w.hook("payment_intent.succeeded", paid15()).await.0, 500);
    }

    #[tokio::test]
    async fn a_payment_for_a_deleted_account_credits_nothing() {
        let w = world(|_| {});
        w.st.with_conn(|c| c.execute("INSERT INTO users (id, trust, created_at, deleted_at) VALUES ('u1', 'teacher', 1, ?1)", [NOW]).unwrap());
        assert_eq!(w.hook("payment_intent.succeeded", paid("pi_late", "u1", Some("medium"), 1500, 400)).await.0, 200);
        assert_eq!(w.balance("u1"), 0);
    }
}
