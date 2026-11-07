//! Tests for `admin_growth.rs`: the per-tag counts, the window, the guard, `days`, and the visits part with a fake
//! Cloudflare (configured, not configured, failing, cached).
use super::super::admin::tests::{World, body_of, code, hdrs, native, obj, world};
use super::*;
use crate::auth::session::login;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

const T0: i64 = 1_800_000_000_000;
const DAY_MS: i64 = 86_400_000;

fn q(pairs: &[(&str, &str)]) -> HashMap<String, String> {
    pairs.iter().map(|(k, v)| (k.to_string(), v.to_string())).collect()
}

/// An account created `days_ago` before T0 with this tag and trust level; returns its id.
fn signed_up(w: &World, email: &str, tag: Option<&str>, trust: &str, days_ago: i64) -> String {
    let id = w.account(email, trust);
    w.db.lock().unwrap().execute("UPDATE users SET signup_source = ?1, created_at = ?2 WHERE id = ?3", params![tag, T0 - days_ago * DAY_MS, id]).unwrap();
    id
}

fn ledger(w: &World, user: &str, kind: &str, r: &str, cents: Option<i64>) {
    w.db.lock()
        .unwrap()
        .execute("INSERT INTO credit_ledger (id, principal, delta, kind, ref, at, cents) VALUES (?1, ?2, 10, ?3, ?4, ?5, ?6)", params![format!("{kind}-{r}"), user, kind, r, T0, cents])
        .unwrap();
}

fn report(w: &World, h: &HeaderMap, pairs: &[(&str, &str)], v: &Visits) -> Result<Value, HttpError> {
    growth(&w.ctx(), h, &native(), &q(pairs), v).map(body_of)
}

#[test]
fn the_report_counts_signups_approvals_purchases_revenue_and_refunds_per_tag() {
    let w = world();
    let a = w.admin("root@x.org");
    // The admin itself was made long ago (created_at = 1): outside every window.
    let f1 = signed_up(&w, "f1@x.org", Some("fb-math"), "teacher", 1);
    let _f2 = signed_up(&w, "f2@x.org", Some("fb-math"), "pending", 2);
    let _f3 = signed_up(&w, "f3@x.org", Some("fb-math"), "unverified", 3);
    let r1 = signed_up(&w, "r1@x.org", Some("reddit"), "teacher", 4);
    let n1 = signed_up(&w, "n1@x.org", None, "admin", 5);
    let _n2 = signed_up(&w, "n2@x.org", None, "pending", 5);
    let _z = signed_up(&w, "z@x.org", Some("zz"), "pending", 6);
    ledger(&w, &f1, "purchase", "pi_1", Some(500));
    // A purchase long after the window started still counts: it is the account's, not the window's.
    ledger(&w, &f1, "purchase", "pi_2", Some(1500));
    ledger(&w, &f1, "reversal", "pi_2#re_1", None);
    ledger(&w, &f1, "spend", "s1", None);
    ledger(&w, &r1, "purchase", "pi_3", Some(1000));
    ledger(&w, &n1, "grant", "g1", None);
    // A deleted account is not counted at all.
    let gone = signed_up(&w, "gone@x.org", Some("reddit"), "teacher", 1);
    w.db.lock().unwrap().execute("UPDATE users SET email = NULL, pw_hash = NULL, pw_params = NULL, display_name = NULL, profile = NULL, deleted_at = 1 WHERE id = ?1", [&gone]).unwrap();
    ledger(&w, &gone, "purchase", "pi_9", Some(9999));

    let b = report(&w, &a.read_only, &[], &Visits::none()).unwrap();
    assert_eq!(b["days"], 30);
    let rows = b["sources"].as_array().unwrap();
    let names: Vec<Value> = rows.iter().map(|r| r["source"].clone()).collect();
    // Most sign-ups first; equal counts by tag name, the "no tag" row after the named ones.
    assert_eq!(names, vec![json!("fb-math"), Value::Null, json!("reddit"), json!("zz")]);
    assert_eq!(rows[0], json!({ "source": "fb-math", "signups": 3, "approved": 1, "purchases": 2, "revenue_cents": 2000, "refunds": 1 }));
    assert_eq!(rows[1], json!({ "source": null, "signups": 2, "approved": 1, "purchases": 0, "revenue_cents": 0, "refunds": 0 }));
    assert_eq!(rows[2], json!({ "source": "reddit", "signups": 1, "approved": 1, "purchases": 1, "revenue_cents": 1000, "refunds": 0 }));
    assert_eq!(rows[3], json!({ "source": "zz", "signups": 1, "approved": 0, "purchases": 0, "revenue_cents": 0, "refunds": 0 }));
    assert_eq!(b["totals"], json!({ "signups": 7, "approved": 3, "purchases": 3, "revenue_cents": 3000, "refunds": 1 }));
    // Counts only: nothing that names an account.
    let text = b.to_string();
    for leak in ["@x.org", f1.as_str(), r1.as_str(), n1.as_str(), a.id.as_str()] {
        assert!(!text.contains(leak), "{leak} in {text}");
    }
}

#[test]
fn the_window_leaves_out_older_signups_and_ties_sort_by_name() {
    let w = world();
    let a = w.admin("root@x.org");
    signed_up(&w, "a@x.org", Some("b-tag"), "teacher", 3);
    signed_up(&w, "b@x.org", Some("a-tag"), "teacher", 6);
    signed_up(&w, "n@x.org", None, "teacher", 2);
    signed_up(&w, "c@x.org", Some("old"), "teacher", 40);
    let b = report(&w, &a.read_only, &[("days", "7")], &Visits::none()).unwrap();
    assert_eq!(b["days"], 7);
    let names: Vec<Value> = b["sources"].as_array().unwrap().iter().map(|r| r["source"].clone()).collect();
    assert_eq!(names, [json!("a-tag"), json!("b-tag"), Value::Null], "equal counts by name, no tag last");
    let b = report(&w, &a.read_only, &[("days", "5")], &Visits::none()).unwrap();
    assert_eq!(b["totals"]["signups"], 2);
    let b = report(&w, &a.read_only, &[("days", "90")], &Visits::none()).unwrap();
    assert_eq!(b["totals"]["signups"], 4);
    let b = report(&w, &a.read_only, &[("days", "1")], &Visits::none()).unwrap();
    assert_eq!((b["sources"].clone(), b["totals"]["signups"].clone()), (json!([]), json!(0)), "an empty window is empty rows, not an error");
}

#[test]
fn days_must_be_a_whole_number_from_1_to_365() {
    let w = world();
    let a = w.admin("root@x.org");
    for ok in ["1", "7", "365", "030"] {
        assert!(report(&w, &a.read_only, &[("days", ok)], &Visits::none()).is_ok(), "{ok}");
    }
    for bad in ["0", "366", "-1", "", "7.5", "abc", "1000", " 7"] {
        let e = report(&w, &a.read_only, &[("days", bad)], &Visits::none()).unwrap_err();
        assert_eq!((e.status, e.code.as_str(), e.extra.get("field")), (400, "invalid_request", Some(&json!("days"))), "{bad:?}");
    }
}

#[test]
fn only_a_signed_in_admin_on_a_known_device_may_read_it_like_the_queue() {
    let w = world();
    let a = w.admin("root@x.org");
    let v = Visits::none();
    assert_eq!(code(growth(&w.ctx(), &hdrs(&[]), &native(), &q(&[]), &v)).0, 401, "signed out");
    let mut no_device = a.read_only.clone();
    no_device.remove("x-device-token");
    assert_eq!(code(growth(&w.ctx(), &no_device, &native(), &q(&[]), &v)), (403, "known_device_required".into()));
    w.account("teacher@x.org", "teacher");
    let b = body_of(login(&w.ctx(), &hdrs(&[]), &native(), &obj(json!({ "email": "teacher@x.org", "password": "violet-harbor-tractor" }))).unwrap());
    let th = hdrs(&[("authorization", &format!("Bearer {}", b["access_token"].as_str().unwrap()))]);
    assert_eq!(code(growth(&w.ctx(), &th, &native(), &q(&[]), &v)), (403, "forbidden".into()));
    // No sudo token needed: it only reads.
    assert!(growth(&w.ctx(), &a.read_only, &native(), &q(&[]), &v).is_ok());
}

// ---- visits ------------------------------------------------------------------------------------

struct Fake {
    calls: Arc<AtomicUsize>,
    windows: Arc<Mutex<Vec<(i64, i64)>>>,
    answer: Result<VisitCounts, String>,
}

impl VisitFetch for Fake {
    fn fetch(&self, since_ms: i64, until_ms: i64) -> Result<VisitCounts, String> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        self.windows.lock().unwrap().push((since_ms, until_ms));
        self.answer.clone()
    }
}

fn fake(answer: Result<VisitCounts, String>) -> (Visits, Arc<AtomicUsize>, Arc<Mutex<Vec<(i64, i64)>>>) {
    let (calls, windows) = (Arc::new(AtomicUsize::new(0)), Arc::new(Mutex::new(Vec::new())));
    (Visits::new(Box::new(Fake { calls: calls.clone(), windows: windows.clone(), answer })), calls, windows)
}

fn counts() -> VisitCounts {
    let mut by_referrer = vec![(Some("reddit.com".to_string()), 40), (None, 25), (Some("facebook.com".to_string()), 10)];
    for i in 0..20 {
        by_referrer.push((Some(format!("site{i}.example")), 1));
    }
    VisitCounts { total: 120, by_referrer }
}

#[test]
fn visits_are_not_configured_without_all_three_settings() {
    let w = world();
    let a = w.admin("root@x.org");
    let b = report(&w, &a.read_only, &[], &Visits::none()).unwrap();
    assert_eq!(b["visits"], json!({ "available": false, "reason": "not_configured" }));
    for (t, ac, s) in [(None, Some("a"), Some("s")), (Some("t"), None, Some("s")), (Some("t"), Some("a"), None)] {
        assert!(!Visits::from_settings(t.map(str::to_string), ac.map(str::to_string), s.map(str::to_string), Some("h.example".into())).configured());
    }
    assert!(Visits::from_settings(Some("t".into()), Some("a".into()), Some("s".into()), Some("h.example".into())).configured());
    assert!(!Visits::from_settings(Some("t".into()), Some("a".into()), Some("s".into()), None).configured(), "no hostname, no counts: the whole domain would be mixed in");
}

#[test]
fn configured_visits_give_the_total_and_the_top_15_referrers_for_the_window() {
    let w = world();
    let a = w.admin("root@x.org");
    let (v, calls, windows) = fake(Ok(counts()));
    let b = report(&w, &a.read_only, &[("days", "7")], &v).unwrap();
    assert_eq!((b["visits"]["available"].clone(), b["visits"]["total"].clone(), b["visits"]["days"].clone()), (json!(true), json!(120), json!(7)));
    let refs = b["visits"]["by_referrer"].as_array().unwrap();
    assert_eq!(refs.len(), 15);
    assert_eq!(refs[0], json!({ "referrer": "reddit.com", "visits": 40 }));
    assert_eq!(refs[1], json!({ "referrer": null, "visits": 25 }), "a direct visit has no referrer");
    assert_eq!(calls.load(Ordering::SeqCst), 1);
    assert_eq!(windows.lock().unwrap()[0], (T0 - 7 * DAY_MS, T0));
    // A year's report asks Cloudflare for its last 90 days only (longer ranges are refused there), and says so.
    let b = report(&w, &a.read_only, &[("days", "365")], &v).unwrap();
    assert_eq!((b["days"].clone(), b["visits"]["days"].clone()), (json!(365), json!(90)));
    assert_eq!(windows.lock().unwrap()[1], (T0 - 90 * DAY_MS, T0));
}

#[test]
fn a_failing_cloudflare_call_is_unavailable_and_the_report_still_answers() {
    let w = world();
    let a = w.admin("root@x.org");
    signed_up(&w, "a@x.org", Some("fb"), "teacher", 1);
    let (v, calls, _) = fake(Err("timed out".into()));
    let b = report(&w, &a.read_only, &[], &v).unwrap();
    assert_eq!(b["visits"], json!({ "available": false, "reason": "unavailable" }));
    assert_eq!(b["totals"]["signups"], 1);
    // A failure is not retried at once either.
    report(&w, &a.read_only, &[], &v).unwrap();
    assert_eq!(calls.load(Ordering::SeqCst), 1);
    assert_eq!(v.report(30, T0 + FAIL_CACHE_MS)["reason"], "unavailable");
    assert_eq!(calls.load(Ordering::SeqCst), 2, "retried once the short failure cache runs out");
}

#[test]
fn a_cloudflare_answer_is_cached_per_days_for_ten_minutes() {
    let (v, calls, _) = fake(Ok(counts()));
    assert_eq!(v.report(30, T0)["total"], 120);
    assert_eq!(v.report(30, T0 + CACHE_MS - 1)["total"], 120);
    assert_eq!(calls.load(Ordering::SeqCst), 1, "a refresh within ten minutes is served from memory");
    v.report(7, T0 + 1);
    assert_eq!(calls.load(Ordering::SeqCst), 2, "another window is its own entry");
    v.report(30, T0 + CACHE_MS);
    assert_eq!(calls.load(Ordering::SeqCst), 3, "stale after ten minutes");
}

#[test]
fn the_cloudflare_request_and_answer_have_the_documented_shape() {
    let body = visits_request("acct-1", "site-1", "app.example", T0 - 7 * DAY_MS, T0 + 123);
    assert_eq!(body["variables"], json!({ "account": "acct-1", "site": "site-1", "host": "app.example", "since": "2027-01-08T08:00:00Z", "until": "2027-01-15T08:00:00Z" }));
    let query = body["query"].as_str().unwrap();
    for part in ["rumPageloadEventsAdaptiveGroups", "accountTag: $account", "siteTag: $site", "requestHost: $host", "datetime_geq: $since", "datetime_lt: $until", "sum { visits }", "refererHost", "sum_visits_DESC"] {
        assert!(query.contains(part), "{part}");
    }
    let answer = json!({ "data": { "viewer": { "accounts": [{
        "total": [{ "sum": { "visits": 57 } }],
        "referrers": [
            { "sum": { "visits": 30 }, "dimensions": { "refererHost": "reddit.com" } },
            { "sum": { "visits": 27 }, "dimensions": { "refererHost": "" } }
        ]
    }] } }, "errors": null });
    assert_eq!(parse_visits(&answer).unwrap(), VisitCounts { total: 57, by_referrer: vec![(Some("reddit.com".into()), 30), (None, 27)] });
    // GraphQL refuses with HTTP 200 and an errors array.
    assert!(parse_visits(&json!({ "data": null, "errors": [{ "message": "unknown field" }] })).is_err());
    assert!(parse_visits(&json!({ "data": { "viewer": { "accounts": [] } } })).is_err());
}

#[test]
fn the_real_fetcher_without_a_runtime_fails_soft() {
    let c = Cloudflare::new("t".into(), "a".into(), "s".into(), "h.example".into());
    assert!(c.fetch(0, 1).is_err());
}

/// A local stand-in for Cloudflare's endpoint: records what it was sent, answers `answer` after `delay`.
async fn stand_in(answer: Value, delay: Duration) -> (String, Arc<Mutex<Vec<(Option<String>, Value)>>>) {
    use axum::routing::post;
    let seen: Arc<Mutex<Vec<(Option<String>, Value)>>> = Arc::new(Mutex::new(Vec::new()));
    let s2 = seen.clone();
    let app = axum::Router::new().route(
        "/graphql",
        post(move |h: HeaderMap, body: axum::Json<Value>| {
            let (seen, answer) = (s2.clone(), answer.clone());
            async move {
                seen.lock().unwrap().push((h.get("authorization").and_then(|v| v.to_str().ok()).map(str::to_string), body.0));
                tokio::time::sleep(delay).await;
                axum::Json(answer)
            }
        }),
    );
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
    (format!("http://{addr}/graphql"), seen)
}

#[tokio::test(flavor = "multi_thread")]
async fn the_real_fetcher_posts_the_query_with_the_token_from_a_blocking_handler_thread() {
    let answer = json!({ "data": { "viewer": { "accounts": [{ "total": [{ "sum": { "visits": 9 } }], "referrers": [{ "sum": { "visits": 9 }, "dimensions": { "refererHost": "news.example" } }] }] } } });
    let (url, seen) = stand_in(answer, Duration::ZERO).await;
    let c = Cloudflare::new("tok-123".into(), "acct".into(), "site".into(), "h.example".into()).at(&url, Duration::from_secs(5));
    // The handler runs on a blocking thread, exactly like this.
    let got = tokio::task::spawn_blocking(move || c.fetch(T0 - DAY_MS, T0)).await.unwrap().unwrap();
    assert_eq!(got, VisitCounts { total: 9, by_referrer: vec![(Some("news.example".into()), 9)] });
    let seen = seen.lock().unwrap();
    assert_eq!(seen[0].0.as_deref(), Some("Bearer tok-123"));
    assert_eq!(seen[0].1["variables"]["site"], "site");
}

#[tokio::test(flavor = "multi_thread")]
async fn a_slow_cloudflare_times_out_and_the_report_says_unavailable() {
    let (url, _) = stand_in(json!({}), Duration::from_secs(3)).await;
    let v = Visits::new(Box::new(Cloudflare::new("t".into(), "a".into(), "s".into(), "h.example".into()).at(&url, Duration::from_millis(200))));
    let started = std::time::Instant::now();
    let out = tokio::task::spawn_blocking(move || v.report(30, T0)).await.unwrap();
    assert_eq!(out, json!({ "available": false, "reason": "unavailable" }));
    assert!(started.elapsed() < Duration::from_secs(2), "gave up at the timeout");
}
