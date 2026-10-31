//! Who may call the relay and what it costs them (port of `principal`, `checkQuota`, `balance`,
//! `startService` and the session gate in worker/src/ai/index.ts). No HTTP here: every function returns an
//! `Outcome` (status, body, headers) that the route layer turns into a response, so the rules are tested alone.
use super::credits::{DAY_MS, HOUR_MS, Service, WEEK_MS, describe_services, utc_day_start, utc_week_start};
use super::request::error_body;
use super::store::{AiStore, NewSession};
use serde_json::{Value, json};

pub struct Limits {
    pub max_image_bytes: usize,
    pub max_out_tokens: i64,
    /// Chat turns per principal per hour (callers without a credit session).
    pub chat_per_hour: i64,
    /// Credits a signed-in user gets each UTC week (Monday 00:00).
    pub weekly_credits: i64,
    pub import_per_day: i64,
    /// US dollars the upstream may bill in a UTC day: the budget guard for the whole key.
    pub daily_usd: f64,
    pub max_pages: i64,
}

impl Default for Limits {
    fn default() -> Self {
        Self { max_image_bytes: 4 * 1024 * 1024, max_out_tokens: 8192, chat_per_hour: 120, weekly_credits: 40, import_per_day: 20, daily_usd: 5.0, max_pages: 20 }
    }
}

#[derive(Debug, PartialEq)]
pub struct Outcome {
    pub status: u16,
    pub body: Value,
    pub headers: Vec<(&'static str, String)>,
}

impl Outcome {
    pub fn json(status: u16, body: Value) -> Self {
        Self { status, body, headers: vec![] }
    }
    pub fn err(status: u16, code: &str, message: &str) -> Self {
        Self::json(status, error_body(code, message))
    }
    fn with(mut self, name: &'static str, value: String) -> Self {
        self.headers.push((name, value));
        self
    }
    fn not_ready() -> Self {
        Self::err(503, "quota_unavailable", "The AI service is not ready. Try again later.")
    }
}

#[derive(Debug, PartialEq)]
pub struct Caller {
    pub user_id: String,
    /// Only a signed-in account is on credits: the relay key and a keyless native client have no identity to count against.
    pub credited: bool,
}

/// Constant-time comparison, so a wrong relay key cannot be found one byte at a time.
pub fn same_secret(a: &str, b: &str) -> bool {
    let (x, y) = (a.as_bytes(), b.as_bytes());
    if x.len() != y.len() {
        return false;
    }
    x.iter().zip(y).fold(0u8, |d, (p, q)| d | (p ^ q)) == 0
}

/// The token of an `Authorization: Bearer <token>` header (`^Bearer\s+(\S+)$`, case-insensitive scheme).
pub fn bearer_token(header: Option<&str>) -> Option<&str> {
    let h = header?;
    let (scheme, rest) = h.split_at_checked(6)?;
    if !scheme.eq_ignore_ascii_case("bearer") {
        return None;
    }
    let token = rest.trim_start_matches(|c: char| c.is_whitespace());
    (token.len() < rest.len() && !token.is_empty() && !token.contains(char::is_whitespace)).then_some(token)
}

pub struct CallerInput<'a> {
    /// The user id when a valid pool session token was presented.
    pub signed_in: Option<&'a str>,
    pub bearer: Option<&'a str>,
    pub origin: Option<&'a str>,
    pub allowed_origins: &'a [String],
    pub relay_key: Option<&'a str>,
}

/// The caller: a pool user, the relay key, or (keyless deployment, no Origin) the native app.
pub fn resolve_caller(i: &CallerInput) -> Result<Caller, Outcome> {
    // Same origin rule as the pool: a browser may only call from the app's own origins.
    if let Some(o) = i.origin {
        if !i.allowed_origins.iter().any(|a| a == o) {
            return Err(Outcome::err(403, "origin_not_allowed", "This origin may not call the relay."));
        }
    }
    if let Some(u) = i.signed_in {
        return Ok(Caller { user_id: u.into(), credited: true });
    }
    if let (Some(key), Some(token)) = (i.relay_key, i.bearer) {
        if same_secret(token, key) {
            return Ok(Caller { user_id: "relay".into(), credited: false });
        }
    }
    // With no relay key the native app (no Origin) may use the relay: such a deployment must stay unreachable.
    if i.relay_key.is_some() || i.origin.is_some() {
        return Err(Outcome::err(401, "unauthenticated", "Sign in first, or use the relay key."));
    }
    Ok(Caller { user_id: "native".into(), credited: false })
}

fn until_midnight(now: i64) -> String {
    // Whole seconds, rounded up.
    ((utc_day_start(now) + DAY_MS - now + 999) / 1000).to_string()
}

/// The account's daily dollar cap and this caller's hourly turn cap; None when both have room. Counted from
/// usage rows, so two requests racing at the line can both pass: a budget guard, not an exact meter. Fails
/// closed: a quota that cannot be read must not become an open relay.
pub fn check_quota(store: &mut AiStore, lim: &Limits, user: &str, on_session: bool, now: i64) -> Option<Outcome> {
    let read = (|| -> rusqlite::Result<Option<Outcome>> {
        if store.cost_since(utc_day_start(now))? >= lim.daily_usd {
            return Ok(Some(Outcome::err(429, "quota_exhausted", "The AI allowance for today is used up.").with("Retry-After", until_midnight(now))));
        }
        // A credit session has its own call budget; the per-hour turn cap is for callers without one.
        if !on_session && store.chat_turns_since(user, now - HOUR_MS)? >= lim.chat_per_hour {
            return Ok(Some(Outcome::err(429, "rate_limited", "Too many requests this hour. Try again later.").with("Retry-After", "600".into())));
        }
        Ok(None)
    })();
    read.unwrap_or_else(|_| {
        tracing::error!("ai: quota unavailable");
        Some(Outcome::not_ready())
    })
}

/// `GET /ai/credits`: the weekly balance and the price list.
pub fn balance(store: &mut AiStore, lim: &Limits, who: &Caller, now: i64) -> Outcome {
    if !who.credited {
        return Outcome::json(200, json!({ "unlimited": true, "services": describe_services() }));
    }
    let week_start = utc_week_start(now);
    let read = (|| -> rusqlite::Result<Outcome> {
        store.refund_expired(&who.user_id, now)?;
        let used = store.credits_spent(&who.user_id, week_start, now)?;
        let purchased = store.purchased_balance(&who.user_id)?;
        Ok(Outcome::json(
            200,
            json!({ "allowance": lim.weekly_credits, "used": used, "left": (lim.weekly_credits - used).max(0), "purchased": purchased, "resets_at": week_start + WEEK_MS, "services": describe_services() }),
        ))
    })();
    read.unwrap_or_else(|_| {
        tracing::error!("ai: credits unavailable");
        Outcome::not_ready()
    })
}

/// `POST /ai/service`: prices a service, charges its credits and opens a session with a call budget. The client
/// names the service and the page count; the price, the call budget and the lifetime are the server's.
pub fn start_service(store: &mut AiStore, lim: &Limits, body: &Value, who: &Caller, credits_on: bool, now: i64) -> Outcome {
    let Some(service) = body.get("service").and_then(Value::as_str).and_then(Service::parse) else {
        return Outcome::err(400, "bad_request", "Unknown service.");
    };
    let mut pages = 0;
    if service.takes_pages() {
        pages = body.get("pages").and_then(Value::as_f64).map(|p| p.floor() as i64).unwrap_or(0);
        if pages < 1 {
            return Outcome::err(400, "bad_request", "pages must be a positive whole number.");
        }
        if pages > service.max_pages() {
            return Outcome::err(400, "too_many_pages", &format!("At most {} pages for this service.", service.max_pages()));
        }
    }
    // No identity to count credits against: no charge, no session.
    if !who.credited || !credits_on {
        return Outcome::json(200, json!({ "session": null, "credits": 0, "unlimited": true }));
    }
    let week_start = utc_week_start(now);
    let credits = service.credits(pages);
    let run = (|| -> rusqlite::Result<Outcome> {
        if store.cost_since(utc_day_start(now))? >= lim.daily_usd {
            return Ok(Outcome::err(429, "quota_exhausted", "The AI allowance for today is used up.").with("Retry-After", until_midnight(now)));
        }
        store.refund_expired(&who.user_id, now)?;
        // The weekly allowance pays first; the purchased balance covers the rest. A negative balance (a refund took
        // back credits already spent) covers nothing. One retry: a racing request can move either count.
        for _ in 0..2 {
            let used = store.credits_spent(&who.user_id, week_start, now)?;
            let purchased = store.purchased_balance(&who.user_id)?.max(0);
            let free_left = (lim.weekly_credits - used).max(0);
            let paid = (credits - free_left).max(0);
            if paid > purchased {
                return Ok(Outcome::json(
                    402,
                    json!({
                        "error": { "code": "insufficient_credits", "message": format!("This needs {credits} credits and you have {free_left} left this week and {purchased} purchased.") },
                        "needed": credits, "left": free_left, "purchased": purchased, "resets_at": week_start + WEEK_MS,
                    }),
                ));
            }
            let id = store.new_id()?;
            let expires_at = now + service.ttl_ms();
            let s = NewSession {
                id: &id, principal: &who.user_id, service: service.id(), credits, paid, calls_allowed: service.calls(pages), pages, now, expires_at, week_start, allowance: lim.weekly_credits,
            };
            if store.start_session(&s)? {
                return Ok(Outcome::json(
                    200,
                    json!({ "session": id, "credits": credits, "paid": paid, "left": free_left - (credits - paid), "purchased": purchased - paid, "calls": service.calls(pages), "expires_at": expires_at, "resets_at": week_start + WEEK_MS }),
                ));
            }
        }
        Ok(Outcome::err(429, "busy", "Another request is using your credits. Try again."))
    })();
    run.unwrap_or_else(|_| {
        tracing::error!("ai: could not start a service session");
        Outcome::not_ready()
    })
}

/// The session a signed-in chat turn runs in: a 32-hex id from the body, with a call taken from its budget.
pub fn take_session(store: &mut AiStore, user: &str, given: Option<&Value>, now: i64) -> Result<String, Outcome> {
    let id = given.and_then(Value::as_str).filter(|s| s.len() == 32 && s.bytes().all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f')));
    let Some(id) = id else {
        return Err(Outcome::err(402, "session_required", "Start the service from the app first (credits)."));
    };
    match store.take_call(id, user, now) {
        Ok(true) => Ok(id.to_string()),
        Ok(false) => Err(Outcome::err(402, "session_exhausted", "This AI session has ended or used its calls. Start the service again.")),
        Err(_) => {
            tracing::error!("ai: session store unavailable");
            Err(Outcome::not_ready())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db;
    use rusqlite::Connection;
    use std::path::Path;

    const MON: i64 = 1_792_972_800_000; // 2026-10-26, as in worker/test/ai-credits.test.ts
    const T: i64 = MON + 9 * HOUR_MS;

    fn conn() -> Connection {
        let mut c = Connection::open_in_memory().unwrap();
        db::migrate(&mut c, &Path::new(env!("CARGO_MANIFEST_DIR")).join("../worker/migrations")).unwrap();
        c
    }
    /// The Worker's `world()`: ten free credits a week.
    fn lim() -> Limits {
        Limits { weekly_credits: 10, ..Limits::default() }
    }
    fn u(id: &str) -> Caller {
        Caller { user_id: id.into(), credited: true }
    }
    fn svc(s: &mut AiStore, who: &Caller, body: Value, now: i64) -> Outcome {
        start_service(s, &lim(), &body, who, true, now)
    }
    fn burn_week(s: &mut AiStore, who: &Caller) {
        for _ in 0..5 {
            assert_eq!(svc(s, who, json!({ "service": "figure" }), T).status, 200);
        }
    }

    #[test]
    fn starting_a_service_charges_the_servers_price_and_the_balance_shows_it() {
        let mut c = conn();
        let mut s = AiStore::new(&mut c);
        let o = svc(&mut s, &u("u1"), json!({ "service": "import", "pages": 4, "credits": 0 }), T);
        assert_eq!((o.status, &o.body["credits"], &o.body["left"]), (200, &json!(2), &json!(8)), "the client cannot set the price");
        assert_eq!(o.body["session"].as_str().unwrap().len(), 32);
        let b = balance(&mut s, &lim(), &u("u1"), T);
        assert_eq!((&b.body["allowance"], &b.body["used"], &b.body["left"], &b.body["resets_at"]), (&json!(10), &json!(2), &json!(8), &json!(MON + WEEK_MS)));
        assert_eq!(b.body["services"]["import"]["credits"], 2);
    }

    #[test]
    fn handwriting_is_a_credit_per_page_and_bad_requests_are_refused() {
        let mut c = conn();
        let mut s = AiStore::new(&mut c);
        let w = u("u1");
        assert_eq!(svc(&mut s, &w, json!({ "service": "handwriting", "pages": 3 }), T).body["credits"], 3);
        assert_eq!(svc(&mut s, &w, json!({ "service": "import", "pages": 21 }), T).body["error"]["code"], "too_many_pages");
        assert_eq!(svc(&mut s, &w, json!({ "service": "import" }), T).status, 400);
        assert_eq!(svc(&mut s, &w, json!({ "service": "nope", "pages": 1 }), T).status, 400);
        assert_eq!(svc(&mut s, &w, json!({ "service": "constructor", "pages": 1 }), T).status, 400);
    }

    #[test]
    fn credits_run_out_name_what_is_needed_and_come_back_next_monday_per_account() {
        let mut c = conn();
        let mut s = AiStore::new(&mut c);
        burn_week(&mut s, &u("u1"));
        let r = svc(&mut s, &u("u1"), json!({ "service": "figure" }), T);
        assert_eq!((r.status, &r.body["error"]["code"], &r.body["needed"], &r.body["left"]), (402, &json!("insufficient_credits"), &json!(2), &json!(0)));
        assert_eq!(svc(&mut s, &u("u2"), json!({ "service": "figure" }), T).status, 200, "credits are per account");
        assert_eq!(svc(&mut s, &u("u1"), json!({ "service": "figure" }), MON + WEEK_MS).status, 200);
    }

    #[test]
    fn purchased_credits_cover_the_shortfall_and_the_allowance_pays_first() {
        let mut c = conn();
        let mut s = AiStore::new(&mut c);
        let w = u("u1");
        burn_week(&mut s, &w);
        assert_eq!(svc(&mut s, &w, json!({ "service": "figure" }), T).status, 402);
        assert!(s.ledger("u1", 5, "purchase", "cs_1", MON, None).unwrap());
        let o = svc(&mut s, &w, json!({ "service": "figure" }), T);
        assert_eq!([&o.body["credits"], &o.body["paid"], &o.body["left"], &o.body["purchased"]], [&json!(2), &json!(2), &json!(0), &json!(3)]);
        let b = balance(&mut s, &lim(), &w, T);
        assert_eq!((&b.body["left"], &b.body["purchased"]), (&json!(0), &json!(3)));

        let mut c2 = conn();
        let mut s2 = AiStore::new(&mut c2);
        for _ in 0..4 {
            svc(&mut s2, &w, json!({ "service": "figure" }), T); // 8 of 10 used
        }
        s2.ledger("u1", 5, "purchase", "cs_1", MON, None).unwrap();
        let o = svc(&mut s2, &w, json!({ "service": "handwriting", "pages": 3 }), T); // 2 free + 1 paid
        assert_eq!([&o.body["credits"], &o.body["paid"], &o.body["left"], &o.body["purchased"]], [&json!(3), &json!(1), &json!(0), &json!(4)]);
    }

    #[test]
    fn an_insufficient_balance_names_both_buckets_and_charges_nothing() {
        let mut c = conn();
        let mut s = AiStore::new(&mut c);
        let w = u("u1");
        burn_week(&mut s, &w);
        s.ledger("u1", 1, "purchase", "cs_1", MON, None).unwrap();
        let r = svc(&mut s, &w, json!({ "service": "figure" }), T);
        assert_eq!((r.status, &r.body["needed"], &r.body["left"], &r.body["purchased"]), (402, &json!(2), &json!(0), &json!(1)));
        assert_eq!(s.purchased_balance("u1").unwrap(), 1);
    }

    #[test]
    fn an_overdrawn_balance_blocks_purchased_spending_but_not_the_free_week() {
        let mut c = conn();
        let mut s = AiStore::new(&mut c);
        let w = u("u1");
        burn_week(&mut s, &w);
        s.ledger("u1", 4, "purchase", "cs_1", MON, None).unwrap();
        assert_eq!(svc(&mut s, &w, json!({ "service": "figure" }), T).status, 200); // 2 spent, 2 left
        assert!(s.ledger("u1", -4, "reversal", "re_1", MON, None).unwrap());
        assert_eq!(s.purchased_balance("u1").unwrap(), -2);
        assert_eq!(svc(&mut s, &w, json!({ "service": "figure" }), T).status, 402);
        assert_eq!(balance(&mut s, &lim(), &w, T).body["purchased"], -2);
        assert_eq!(svc(&mut s, &w, json!({ "service": "figure" }), MON + WEEK_MS).status, 200, "the new week's free credits still work");
    }

    #[test]
    fn an_unused_session_gives_the_purchased_part_back_once() {
        let mut c = conn();
        let mut s = AiStore::new(&mut c);
        let w = u("u1");
        burn_week(&mut s, &w);
        s.ledger("u1", 2, "purchase", "cs_1", MON, None).unwrap();
        assert_eq!(svc(&mut s, &w, json!({ "service": "figure" }), T).status, 200);
        assert_eq!(s.purchased_balance("u1").unwrap(), 0, "spent while open");
        let later = T + Service::Figure.ttl_ms() + 1;
        assert_eq!(balance(&mut s, &lim(), &w, later).body["purchased"], 2);
        assert_eq!(balance(&mut s, &lim(), &w, later).body["purchased"], 2, "a second look does not refund twice");
    }

    #[test]
    fn the_relay_key_and_an_anonymous_client_are_not_on_credits() {
        let mut c = conn();
        let mut s = AiStore::new(&mut c);
        let relay = Caller { user_id: "relay".into(), credited: false };
        let o = start_service(&mut s, &lim(), &json!({ "service": "figure" }), &relay, true, T);
        assert_eq!(o.body, json!({ "session": null, "credits": 0, "unlimited": true }));
        assert_eq!(balance(&mut s, &lim(), &relay, T).body["unlimited"], true);
        let o = start_service(&mut s, &lim(), &json!({ "service": "figure" }), &u("u1"), false, T);
        assert_eq!(o.body["unlimited"], true, "credits switched off");
    }

    #[test]
    fn a_signed_in_chat_needs_a_session_of_its_own_with_calls_left() {
        let mut c = conn();
        let mut s = AiStore::new(&mut c);
        let code = |r: Result<String, Outcome>| r.unwrap_err().body["error"]["code"].as_str().unwrap().to_string();
        assert_eq!(code(take_session(&mut s, "u1", None, T)), "session_required");
        assert_eq!(code(take_session(&mut s, "u1", Some(&json!("not-hex")), T)), "session_required");
        assert_eq!(code(take_session(&mut s, "u1", Some(&json!("0".repeat(32))), T)), "session_exhausted");
        let id = svc(&mut s, &u("u1"), json!({ "service": "figure" }), T).body["session"].as_str().unwrap().to_string();
        assert_eq!(code(take_session(&mut s, "u2", Some(&json!(id)), T)), "session_exhausted", "a session belongs to its account");
        for _ in 0..40 {
            assert_eq!(take_session(&mut s, "u1", Some(&json!(id)), T).unwrap(), id);
        }
        assert_eq!(code(take_session(&mut s, "u1", Some(&json!(id)), T)), "session_exhausted", "the 40-call budget is used up");
        let id2 = svc(&mut s, &u("u1"), json!({ "service": "figure" }), T).body["session"].as_str().unwrap().to_string();
        assert_eq!(code(take_session(&mut s, "u1", Some(&json!(id2)), T + Service::Figure.ttl_ms())), "session_exhausted", "expired");
    }

    #[test]
    fn the_daily_dollar_cap_and_the_hourly_turn_cap_with_fail_closed() {
        let mut c = conn();
        let mut s = AiStore::new(&mut c);
        let l = Limits { daily_usd: 0.5, chat_per_hour: 2, ..lim() };
        assert_eq!(check_quota(&mut s, &l, "u1", false, T), None);
        for _ in 0..2 {
            s.record("u1", &super::super::store::Usage { kind: "chat", in_tokens: 1.0, out_tokens: 1.0, cost_usd: 0.1, session: None }, T - 1).unwrap();
        }
        let o = check_quota(&mut s, &l, "u1", false, T).unwrap();
        assert_eq!((o.status, &o.body["error"]["code"], o.headers.clone()), (429, &json!("rate_limited"), vec![("Retry-After", "600".to_string())]));
        assert_eq!(check_quota(&mut s, &l, "u1", true, T), None, "a session has its own budget");
        assert_eq!(check_quota(&mut s, &l, "u2", false, T), None, "another principal is not affected");
        assert_eq!(check_quota(&mut s, &l, "u1", false, T + HOUR_MS + 1), None, "an hour later the window has moved");
        s.record("u2", &super::super::store::Usage { kind: "chat", in_tokens: 1.0, out_tokens: 1.0, cost_usd: 0.4, session: None }, T - 1).unwrap();
        let o = check_quota(&mut s, &l, "u3", false, T).unwrap();
        assert_eq!((o.status, &o.body["error"]["code"]), (429, &json!("quota_exhausted")));
        assert_eq!(o.headers, vec![("Retry-After", (utc_day_start(T) + DAY_MS - T).div_euclid(1000).to_string())]);
        // Fail closed: with the usage table gone the quota cannot be read, and the answer is 503, not "go ahead".
        s_drop_usage(&mut s);
        let o = check_quota(&mut s, &l, "u1", false, T).unwrap();
        assert_eq!((o.status, &o.body["error"]["code"]), (503, &json!("quota_unavailable")));
    }

    fn s_drop_usage(s: &mut AiStore) {
        s.exec_for_test("DROP TABLE ai_usage");
    }

    #[test]
    fn callers_the_origin_rule_the_relay_key_and_the_keyless_native_app() {
        let allowed = vec!["https://app.example.org".to_string()];
        let key = "k".repeat(32);
        let who = |signed_in, bearer, origin, relay_key| {
            resolve_caller(&CallerInput { signed_in, bearer, origin, allowed_origins: &allowed, relay_key })
        };
        let code = |r: Result<Caller, Outcome>| r.unwrap_err().body["error"]["code"].as_str().unwrap().to_string();
        assert_eq!(who(Some("u1"), None, None, Some(&key)).unwrap(), Caller { user_id: "u1".into(), credited: true });
        assert_eq!(who(None, Some(&key), None, Some(&key)).unwrap(), Caller { user_id: "relay".into(), credited: false });
        assert_eq!(code(who(None, Some(&key[1..]), None, Some(&key))), "unauthenticated");
        assert_eq!(code(who(None, None, None, Some(&key))), "unauthenticated", "a configured key is required");
        assert_eq!(who(None, None, None, None).unwrap(), Caller { user_id: "native".into(), credited: false });
        assert_eq!(code(who(None, None, Some("https://app.example.org"), None)), "unauthenticated", "a browser needs a session or a key");
        assert_eq!(code(who(Some("u1"), None, Some("https://evil.example"), None)), "origin_not_allowed");
        assert_eq!(who(Some("u1"), None, Some("https://app.example.org"), None).unwrap().user_id, "u1");
    }

    #[test]
    fn bearer_tokens_and_secret_comparison() {
        assert_eq!(bearer_token(Some("Bearer abc")), Some("abc"));
        assert_eq!(bearer_token(Some("bearer   abc")), Some("abc"));
        for bad in [None, Some(""), Some("Bearer"), Some("Bearer "), Some("Bearer a b"), Some("Basic abc"), Some("Bearerabc")] {
            assert_eq!(bearer_token(bad), None, "{bad:?}");
        }
        assert!(same_secret("abc", "abc") && !same_secret("abc", "abd") && !same_secret("abc", "ab"));
    }
}
