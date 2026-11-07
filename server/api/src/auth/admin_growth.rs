//! The growth report: which ad / link source (`?src=<tag>`, kept on the account at sign-up) brings sign-ups,
//! approvals and purchases, plus Cloudflare Web Analytics visit counts when they are connected. An admin read
//! (`GET {admin}/growth?days=N`), behind the same guard as the queue. It returns counts only: never an address,
//! a user id or anything else that names an account.
//!
//! Visits come from Cloudflare's GraphQL Analytics API (dataset `rumPageloadEventsAdaptiveGroups`) when
//! `CF_ANALYTICS_TOKEN`, `CF_ACCOUNT_ID` and `CF_SITE_TAG` are all set. That dataset has no query-string or UTM
//! dimension (the beacon sends the page URL without its query), so visits cannot be split by `src` tag: they are
//! reported by referring host instead, beside the per-tag sign-ups. The call is fail-soft (5 s timeout; a failure is
//! `available: false` and the rest of the report still answers 200) and cached per `days` so a refreshing admin
//! cannot hammer the API.
use super::admin::{dberr, guard_admin, lock};
use super::config::{APPROVED_LEVEL, TRUST, trust_rank};
use super::http::{ClientInfo, HttpError, json};
use super::session::Ctx;
use axum::http::HeaderMap;
use axum::response::Response;
use rusqlite::params;
use serde_json::{Value, json};
use std::collections::HashMap;
use std::sync::Mutex;
use std::time::Duration;

pub const DEFAULT_DAYS: i64 = 30;
pub const MAX_DAYS: i64 = 365;
/// The most days asked of Cloudflare in one query: the dataset refuses longer ranges (93 days observed on a free
/// account), so a longer report shows visits for its most recent 90 days and says so (`visits.days`).
pub const MAX_VISIT_DAYS: i64 = 90;
/// How many referring hosts the report lists.
pub const TOP_REFERRERS: usize = 15;
/// A Cloudflare answer is reused this long (per `days`).
pub const CACHE_MS: i64 = 10 * 60 * 1000;
/// A failed call is not retried for this long (per `days`), so an outage is not hammered either.
pub const FAIL_CACHE_MS: i64 = 60 * 1000;
pub const CF_TIMEOUT: Duration = Duration::from_secs(5);
pub const CF_GRAPHQL_URL: &str = "https://api.cloudflare.com/client/v4/graphql";

/// Visit counts for one window.
#[derive(Clone, Debug, PartialEq)]
pub struct VisitCounts {
    pub total: i64,
    /// (referring host, visits), most first; an empty host (a direct visit, or no referrer sent) is `None`.
    pub by_referrer: Vec<(Option<String>, i64)>,
}

/// Fetches visit counts for `[since_ms, until_ms)`. The real one calls Cloudflare; tests inject a fake.
pub trait VisitFetch: Send + Sync {
    fn fetch(&self, since_ms: i64, until_ms: i64) -> Result<VisitCounts, String>;
}

/// The visits part of the report: an optional fetcher and its cache.
pub struct Visits {
    fetch: Option<Box<dyn VisitFetch>>,
    /// days -> (when it was fetched, what came back)
    cache: Mutex<HashMap<i64, (i64, Result<VisitCounts, ()>)>>,
}

impl Visits {
    /// Not connected: every report says `not_configured`.
    pub fn none() -> Visits {
        Visits { fetch: None, cache: Mutex::new(HashMap::new()) }
    }

    pub fn new(fetch: Box<dyn VisitFetch>) -> Visits {
        Visits { fetch: Some(fetch), cache: Mutex::new(HashMap::new()) }
    }

    /// Cloudflare, when all three settings are present; otherwise not connected.
    pub fn from_settings(token: Option<String>, account: Option<String>, site_tag: Option<String>) -> Visits {
        match (token, account, site_tag) {
            (Some(token), Some(account), Some(site_tag)) => Visits::new(Box::new(Cloudflare::new(token, account, site_tag))),
            _ => Visits::none(),
        }
    }

    pub fn configured(&self) -> bool {
        self.fetch.is_some()
    }

    /// The `visits` object of the report for a `days` window ending `now`.
    pub fn report(&self, days: i64, now: i64) -> Value {
        let Some(fetch) = &self.fetch else { return json!({ "available": false, "reason": "not_configured" }) };
        let days = days.min(MAX_VISIT_DAYS);
        let cached = {
            let cache = self.cache.lock().unwrap_or_else(|p| p.into_inner());
            cache.get(&days).filter(|(at, r)| now - at < if r.is_ok() { CACHE_MS } else { FAIL_CACHE_MS }).map(|(_, r)| r.clone())
        };
        let result = match cached {
            Some(r) => r,
            None => {
                let r = fetch.fetch(now - days * 86_400_000, now).map_err(|e| {
                    // The reason only: never the token, and no request details.
                    tracing::warn!("growth: visit counts unavailable: {e}");
                });
                self.cache.lock().unwrap_or_else(|p| p.into_inner()).insert(days, (now, r.clone()));
                r
            }
        };
        match result {
            Ok(v) => json!({
                "available": true,
                "days": days,
                "total": v.total,
                "by_referrer": v.by_referrer.iter().take(TOP_REFERRERS).map(|(r, n)| json!({ "referrer": r, "visits": n })).collect::<Vec<_>>(),
            }),
            Err(()) => json!({ "available": false, "reason": "unavailable" }),
        }
    }
}

/// Cloudflare's GraphQL Analytics API (docs: developers.cloudflare.com/analytics/graphql-api/). The token needs
/// Account > Account Analytics > Read only. `sum.visits` is Cloudflare's "visit": a page view whose referrer is
/// another site or none (in-site navigation is not counted). Counts are sampled estimates.
pub struct Cloudflare {
    token: String,
    account: String,
    site_tag: String,
    url: String,
    timeout: Duration,
}

impl Cloudflare {
    pub fn new(token: String, account: String, site_tag: String) -> Cloudflare {
        Cloudflare { token, account, site_tag, url: CF_GRAPHQL_URL.to_string(), timeout: CF_TIMEOUT }
    }

    /// Another endpoint and timeout (tests: a local stand-in for Cloudflare).
    pub fn at(mut self, url: &str, timeout: Duration) -> Cloudflare {
        self.url = url.to_string();
        self.timeout = timeout;
        self
    }
}

/// One GraphQL document with two aliased reads of the same dataset: the window's total (no dimensions: one group) and
/// the top referring hosts.
pub const VISITS_QUERY: &str = "query Visits($account: String!, $site: String!, $since: Time!, $until: Time!) {
  viewer {
    accounts(filter: { accountTag: $account }) {
      total: rumPageloadEventsAdaptiveGroups(limit: 1, filter: { siteTag: $site, datetime_geq: $since, datetime_lt: $until }) {
        sum { visits }
      }
      referrers: rumPageloadEventsAdaptiveGroups(limit: 15, filter: { siteTag: $site, datetime_geq: $since, datetime_lt: $until }, orderBy: [sum_visits_DESC]) {
        sum { visits }
        dimensions { refererHost }
      }
    }
  }
}";

/// The request body for a window.
pub fn visits_request(account: &str, site_tag: &str, since_ms: i64, until_ms: i64) -> Value {
    let t = |ms: i64| {
        // whole seconds are enough, and Cloudflare's Time scalar is RFC 3339
        let s = super::signup::iso8601(ms - ms.rem_euclid(1000));
        s.replace(".000Z", "Z")
    };
    json!({ "query": VISITS_QUERY, "variables": { "account": account, "site": site_tag, "since": t(since_ms), "until": t(until_ms) } })
}

/// Reads Cloudflare's answer. A GraphQL error comes back with HTTP 200 and an `errors` array, so that is checked
/// first; the messages are not passed on (they may echo the account).
pub fn parse_visits(v: &Value) -> Result<VisitCounts, String> {
    if v.get("errors").and_then(Value::as_array).is_some_and(|e| !e.is_empty()) {
        return Err("the query was refused".into());
    }
    let acct = v.pointer("/data/viewer/accounts/0").ok_or("no account in the answer")?;
    let visits = |g: &Value| g.pointer("/sum/visits").and_then(Value::as_i64).unwrap_or(0);
    let total = acct.get("total").and_then(Value::as_array).ok_or("no total")?.iter().map(visits).sum();
    let by_referrer = acct
        .get("referrers")
        .and_then(Value::as_array)
        .ok_or("no referrers")?
        .iter()
        .map(|g| (g.pointer("/dimensions/refererHost").and_then(Value::as_str).filter(|h| !h.is_empty()).map(str::to_string), visits(g)))
        .collect();
    Ok(VisitCounts { total, by_referrer })
}

impl VisitFetch for Cloudflare {
    fn fetch(&self, since_ms: i64, until_ms: i64) -> Result<VisitCounts, String> {
        // The handler runs on a blocking thread of the server's runtime, so the async client is driven from here.
        let rt = tokio::runtime::Handle::try_current().map_err(|_| "no runtime".to_string())?;
        let body = visits_request(&self.account, &self.site_tag, since_ms, until_ms);
        rt.block_on(async {
            let client = reqwest::Client::builder().timeout(self.timeout).build().map_err(|_| "client".to_string())?;
            let resp = client.post(&self.url).bearer_auth(&self.token).json(&body).send().await.map_err(|e| if e.is_timeout() { "timed out".to_string() } else { "request failed".to_string() })?;
            if !resp.status().is_success() {
                return Err(format!("HTTP {}", resp.status().as_u16()));
            }
            let v: Value = resp.json().await.map_err(|_| "not JSON".to_string())?;
            parse_visits(&v)
        })
    }
}

/// One row of the report.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct SourceRow {
    pub source: Option<String>,
    pub signups: i64,
    pub approved: i64,
    pub purchases: i64,
    pub revenue_cents: i64,
    pub refunds: i64,
}

impl SourceRow {
    fn to_json(&self) -> Value {
        json!({ "source": self.source, "signups": self.signups, "approved": self.approved, "purchases": self.purchases, "revenue_cents": self.revenue_cents, "refunds": self.refunds })
    }
}

/// `days` from the query: absent is 30, otherwise a whole number 1..=365.
pub fn days_of(q: &HashMap<String, String>) -> Result<i64, HttpError> {
    match q.get("days") {
        None => Ok(DEFAULT_DAYS),
        Some(v) if !v.is_empty() && v.len() <= 3 && v.bytes().all(|b| b.is_ascii_digit()) => {
            v.parse::<i64>().ok().filter(|n| (1..=MAX_DAYS).contains(n)).ok_or_else(|| invalid("days"))
        }
        Some(_) => Err(invalid("days")),
    }
}

fn invalid(field: &str) -> HttpError {
    HttpError::new(400, "invalid_request").with("field", json!(field))
}

/// The per-tag counts for accounts created since `since_ms`, most sign-ups first, then by tag (no tag last).
/// - signups: accounts created in the window that are not deleted;
/// - approved: of those, the ones at the approved level or above now;
/// - purchases / revenue_cents: their `purchase` ledger rows at any time (a purchase may come after the window).
///   Revenue is GROSS: what Stripe received, before refunds, disputes and fees;
/// - refunds: their `reversal` ledger rows (a refund or dispute; one payment refunded in parts counts each part).
pub fn rows(conn: &rusqlite::Connection, since_ms: i64) -> Result<Vec<SourceRow>, HttpError> {
    let approved: Vec<String> = TRUST.iter().filter(|t| trust_rank(t) >= trust_rank(APPROVED_LEVEL)).map(|t| format!("'{t}'")).collect();
    let sql = format!(
        "SELECT u.signup_source, COUNT(*), COALESCE(SUM(u.trust IN ({})), 0),
                COALESCE(SUM(l.purchases), 0), COALESCE(SUM(l.cents), 0), COALESCE(SUM(l.reversals), 0)
         FROM users u
         LEFT JOIN (SELECT principal,
                           SUM(kind = 'purchase') AS purchases,
                           SUM(CASE WHEN kind = 'purchase' THEN COALESCE(cents, 0) ELSE 0 END) AS cents,
                           SUM(kind = 'reversal') AS reversals
                    FROM credit_ledger WHERE kind IN ('purchase', 'reversal') GROUP BY principal) l ON l.principal = u.id
         WHERE u.deleted_at IS NULL AND u.created_at >= ?1
         GROUP BY u.signup_source",
        approved.join(", ")
    );
    let mut st = conn.prepare(&sql).map_err(dberr)?;
    let mut out: Vec<SourceRow> = st
        .query_map(params![since_ms], |r| Ok(SourceRow { source: r.get(0)?, signups: r.get(1)?, approved: r.get(2)?, purchases: r.get(3)?, revenue_cents: r.get(4)?, refunds: r.get(5)? }))
        .map_err(dberr)?
        .collect::<Result<_, _>>()
        .map_err(dberr)?;
    out.sort_by(|a, b| b.signups.cmp(&a.signups).then_with(|| match (&a.source, &b.source) {
        (Some(x), Some(y)) => x.cmp(y),
        (Some(_), None) => std::cmp::Ordering::Less,
        (None, Some(_)) => std::cmp::Ordering::Greater,
        (None, None) => std::cmp::Ordering::Equal,
    }));
    Ok(out)
}

/// `GET {admin}/growth?days=N`.
pub fn growth(c: &Ctx, h: &HeaderMap, client: &ClientInfo, q: &HashMap<String, String>, visits: &Visits) -> Result<Response, HttpError> {
    guard_admin(c, h, client, false)?;
    let days = days_of(q)?;
    // The database lock is let go before the (possibly slow) visits call.
    let sources = rows(&lock(c), c.now - days * 86_400_000)?;
    let mut totals = SourceRow::default();
    for r in &sources {
        totals.signups += r.signups;
        totals.approved += r.approved;
        totals.purchases += r.purchases;
        totals.revenue_cents += r.revenue_cents;
        totals.refunds += r.refunds;
    }
    let mut t = totals.to_json();
    if let Some(o) = t.as_object_mut() {
        o.remove("source");
    }
    Ok(json(
        200,
        &json!({ "days": days, "sources": sources.iter().map(SourceRow::to_json).collect::<Vec<_>>(), "totals": t, "visits": visits.report(days, c.now) }),
        &[],
    ))
}

#[cfg(test)]
#[path = "admin_growth_tests.rs"]
mod tests;
