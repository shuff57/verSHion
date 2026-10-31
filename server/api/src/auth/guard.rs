//! Rate-limit guards and known-device tokens for the auth routes (port of the `limit`, `reserveFailure`,
//! `refundFailed` and device-token helpers in auth/src/auth.ts, and `ipBucket` in ratelimit.ts).
//! Every counter key is an HMAC, so no raw IP or email is ever stored.
use super::config::{Action, AuthConfig, FailureLimits, limits};
use super::crypto::{Keys, b64url, hmac, hmac_raw, random_bytes, timing_safe_equal, from_b64url};
use super::http::HttpError;
use crate::ratelimit::{self, Rule};
use axum::http::HeaderMap;
use rusqlite::Connection;

impl Action {
    pub fn name(self) -> &'static str {
        match self {
            Action::Signup => "signup",
            Action::Login => "login",
            Action::Refresh => "refresh",
            Action::Verify => "verify",
            Action::Resend => "resend",
            Action::Forgot => "forgot",
            Action::Reset => "reset",
            Action::Delete => "delete",
            Action::Enrol => "enrol",
            Action::Reauth => "reauth",
            Action::Admin => "admin",
            Action::Reapply => "reapply",
        }
    }
}

/// Groups IPv6 addresses by prefix (/64 by default: one subscriber usually holds a whole /64; /48 for login-failure
/// counting). IPv4 is left alone.
pub fn ip_bucket(ip: &str, prefix: usize) -> String {
    if !ip.contains(':') {
        return ip.to_string();
    }
    let addr = ip.split('%').next().unwrap_or(ip).to_lowercase();
    if addr.contains('.') {
        return addr; // IPv4-mapped; rare enough to keep whole
    }
    let (head, tail) = match addr.split_once("::") {
        Some((h, t)) => (h.to_string(), Some(t.split("::").next().unwrap_or("").to_string())),
        None => (addr.clone(), None),
    };
    let h: Vec<&str> = if head.is_empty() { vec![] } else { head.split(':').collect() };
    let t: Vec<&str> = match &tail {
        Some(t) if !t.is_empty() => t.split(':').collect(),
        _ => vec![],
    };
    let fill = if addr.contains("::") { 8usize.saturating_sub(h.len() + t.len()) } else { 0 };
    let groups: Vec<String> = h.iter().map(|s| s.to_string()).chain(std::iter::repeat_n("0".to_string(), fill)).chain(t.iter().map(|s| s.to_string())).map(strip_zeros).collect();
    format!("{}::/{prefix}", groups.iter().take(prefix / 16).cloned().collect::<Vec<_>>().join(":"))
}

fn strip_zeros(g: String) -> String {
    let t = g.trim_start_matches('0');
    if t.is_empty() && !g.is_empty() { "0".into() } else { t.to_string() }
}

/// The client address, from the reverse proxy in front of the server. Caddy sets `X-Forwarded-For`; the LAST entry is
/// the one the nearest proxy added, so a client cannot choose it. Missing means fail closed (403), as in the Worker.
pub fn client_ip(h: &HeaderMap) -> Result<String, HttpError> {
    let v = h.get("x-forwarded-for").and_then(|v| v.to_str().ok()).and_then(|s| s.rsplit(',').next()).map(str::trim).filter(|s| !s.is_empty());
    v.map(str::to_string).ok_or_else(|| HttpError::new(403, "client_ip_required"))
}

fn db_down() -> HttpError {
    tracing::error!("auth: rate limit store failed");
    HttpError::new(500, "internal_error")
}

fn limited(retry: i64) -> HttpError {
    HttpError::new(429, "rate_limited").header("Retry-After", &retry.to_string())
}

/// Every counter that can identify an account starts with this prefix, so deleting the account purges them all.
pub fn acct_prefix(keys: &Keys, account: &str) -> String {
    format!("a:{}:", hmac(&keys.limit, account))
}

#[derive(Debug)]
pub struct Hit {
    key: String,
    rule: Rule,
}

/// Hits taken so far; `refund` gives them back.
#[derive(Debug)]
pub struct Refund {
    hits: Vec<Hit>,
    now: i64,
}

impl Refund {
    pub fn none() -> Self {
        Refund { hits: vec![], now: 0 }
    }
    pub fn refund(&self, conn: &Connection) {
        for h in &self.hits {
            let _ = ratelimit::undo(conn, &h.key, h.rule, self.now);
        }
    }
}

fn take(conn: &Connection, hits: Vec<Hit>, now: i64) -> Result<Result<Refund, i64>, HttpError> {
    let mut denied = 0i64;
    let mut any = false;
    for h in &hits {
        let v = ratelimit::hit(conn, &h.key, h.rule, now).map_err(|_| db_down())?;
        if !v.allowed {
            any = true;
            denied = denied.max(v.retry_after);
        }
    }
    let r = Refund { hits, now };
    if any {
        r.refund(conn);
        return Ok(Err(denied));
    }
    Ok(Ok(r))
}

/// Rate limit BEFORE any expensive work; Err is the 429. The returned `Refund` gives the hits back when a caller ends
/// up doing no expensive work, so a shared address's budget only counts work actually done.
/// - `ip`: per client IP (IPv6 /64); `account`: per account, every attempt; `ip_account`: per (IP, account) pair.
pub fn limit(conn: &Connection, keys: &Keys, action: Action, ip: &str, account: Option<&str>, now: i64) -> Result<Refund, HttpError> {
    let rules = limits(action);
    let ip_h = hmac(&keys.limit, &ip_bucket(ip, 64));
    let name = action.name();
    let mut hits: Vec<Hit> = rules.ip.iter().map(|r| Hit { key: format!("i:{ip_h}:{name}:{}", r.window_seconds), rule: *r }).collect();
    if let Some(a) = account {
        let pre = acct_prefix(keys, a);
        hits.extend(rules.account.iter().map(|r| Hit { key: format!("{pre}{name}:{}", r.window_seconds), rule: *r }));
        hits.extend(rules.ip_account.iter().map(|r| Hit { key: format!("{pre}{name}:ip:{}:{ip_h}", r.window_seconds), rule: *r }));
    }
    if hits.is_empty() {
        return Ok(Refund::none());
    }
    take(conn, hits, now)?.map_err(limited)
}

/// A failed login's per-IP charge goes back if it fits the separate per-IP "failed login" allowance.
pub fn refund_failed(conn: &Connection, keys: &Keys, ip: &str, now: i64, refund_ip: &Refund) {
    let rules = limits(Action::Login).ip_failed;
    if rules.is_empty() {
        return;
    }
    let ip_h = hmac(&keys.limit, &ip_bucket(ip, 64));
    let hits: Vec<Hit> = rules.iter().map(|r| Hit { key: format!("i:{ip_h}:login-failed:{}", r.window_seconds), rule: *r }).collect();
    match take(conn, hits, now) {
        Ok(Ok(_)) => refund_ip.refund(conn),
        _ => {}
    }
}

// ---- known devices --------------------------------------------------------------------------
// A stateless token, `v1.<userId>.<epoch>.<expSeconds>.<nonce>.<mac>`, issued at each successful login, reset,
// activation and enrolment. The MAC key is separate from every other key. Bumping users.device_epoch revokes them
// all. users.last_device holds the nonce of the most recently issued one: the owner's current device.

#[derive(Clone, Debug, PartialEq)]
pub struct Device {
    pub token: String,
    pub nonce: String,
}

pub fn issue_device_token(keys: &Keys, user_id: &str, epoch: i64, now_ms: i64, ttl_s: i64) -> Device {
    // The random nonce makes every issued token distinct, so two devices that log in in the same second never share
    // a token (or its per-device failure counter).
    let nonce = b64url(&random_bytes(9));
    let payload = format!("v1.{user_id}.{epoch}.{}.{nonce}", now_ms / 1000 + ttl_s);
    Device { token: format!("{payload}.{}", hmac(&keys.device, &payload)), nonce }
}

pub struct DeviceUser<'a> {
    pub id: &'a str,
    pub device_epoch: i64,
}

pub fn parse_device(keys: &Keys, token: Option<&str>, user: Option<&DeviceUser>, now_ms: i64) -> Option<Device> {
    let (token, user) = (token?, user?);
    let parts: Vec<&str> = token.split('.').collect();
    let [v, uid, epoch, exp, nonce, mac] = parts[..] else { return None };
    let digits = |s: &str, max: usize| !s.is_empty() && s.len() <= max && s.bytes().all(|b| b.is_ascii_digit());
    let urlsafe = |s: &str, n: usize| s.len() == n && s.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_');
    if v != "v1" || uid.len() != 36 || !uid.bytes().all(|b| b.is_ascii_hexdigit() && !b.is_ascii_uppercase() || b == b'-') || !digits(epoch, 9) || !digits(exp, 12) || !urlsafe(nonce, 12) || !urlsafe(mac, 43) {
        return None;
    }
    // Per account: a token for one account never unlocks another.
    if uid != user.id || epoch.parse::<i64>().ok()? != user.device_epoch || exp.parse::<i64>().ok()? <= now_ms / 1000 {
        return None;
    }
    let payload = format!("v1.{uid}.{epoch}.{exp}.{nonce}");
    let want = hmac_raw(&keys.device, payload.as_bytes());
    timing_safe_equal(&from_b64url(mac)?, &want).then(|| Device { token: token.to_string(), nonce: nonce.to_string() })
}

/// Login failure limits, checked before the hash. A failure slot is reserved atomically before the hash and given back
/// on success, so concurrent guesses cannot overshoot and only failures count.
/// - Without a device token: the (account, network /48) pair and every per-account rule must hold.
/// - With a valid known-device token: none of those apply, so the owner is never refused for other people's failures;
///   the device's own failures count per device, and devices other than the owner's current one also share
///   `per_account_device`. None of these feed the lockdown that refuses everyone else.
/// Every key includes the account's `device_epoch`, so a reset, activation or logout-everywhere re-keys all of the
/// account's counters at once. Unknown addresses use epoch 0, so refusals look the same for them.
pub fn reserve_failure(conn: &Connection, keys: &Keys, ip: &str, account: &str, epoch: i64, device: Option<&Device>, last_device: Option<&str>, now: i64) -> Result<Refund, HttpError> {
    let Some(f) = limits(Action::Login).failures else { return Ok(Refund::none()) };
    let FailureLimits { per_account_ip, per_account, per_device, per_account_device } = f;
    let pre = acct_prefix(keys, account);
    let e = format!("e{epoch}");
    let mut hits: Vec<Hit> = vec![];
    if let Some(d) = device {
        let dh = hmac(&keys.limit, &d.token);
        hits.extend(per_device.iter().map(|r| Hit { key: format!("{pre}lf:dev:{dh}:{}", r.window_seconds), rule: *r }));
        if Some(d.nonce.as_str()) != last_device {
            hits.extend(per_account_device.iter().map(|r| Hit { key: format!("{pre}lf:devacct:{e}:{}", r.window_seconds), rule: *r }));
        }
    } else {
        let ip_h = hmac(&keys.limit, &ip_bucket(ip, 48));
        hits.push(Hit { key: format!("{pre}lf:pair:{e}:{}:{ip_h}", per_account_ip.window_seconds), rule: per_account_ip });
        hits.extend(per_account.iter().map(|r| Hit { key: format!("{pre}lf:acct:{e}:{}", r.window_seconds), rule: *r }));
    }
    take(conn, hits, now)?.map_err(limited)
}

/// The cookie or header a client presents its device token in.
pub fn presented_device(h: &HeaderMap, native: bool, cfg: &AuthConfig) -> Option<String> {
    if native { h.get("x-device-token").and_then(|v| v.to_str().ok()).map(str::to_string) } else { super::http::read_cookie(h, &cfg.device_cookie_name) }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::auth::crypto::derive_keys;
    use axum::http::HeaderValue;

    const T: i64 = 1_800_000_000_000;
    const UID: &str = "123e4567-e89b-12d3-a456-426614174000";

    fn db() -> Connection {
        let c = Connection::open_in_memory().unwrap();
        c.execute_batch("CREATE TABLE rate_limits (key TEXT NOT NULL, window_start INTEGER NOT NULL, count INTEGER NOT NULL, expires_at INTEGER NOT NULL, PRIMARY KEY (key, window_start)) WITHOUT ROWID;").unwrap();
        c
    }
    fn total(c: &Connection) -> i64 {
        c.query_row("SELECT COALESCE(SUM(count), 0) FROM rate_limits", [], |r| r.get(0)).unwrap()
    }

    #[test]
    fn ipv6_is_grouped_by_prefix_and_ipv4_is_left_alone() {
        assert_eq!(ip_bucket("198.51.100.4", 64), "198.51.100.4");
        assert_eq!(ip_bucket("2001:db8:abcd:12:1:2:3:4", 64), "2001:db8:abcd:12::/64");
        assert_eq!(ip_bucket("2001:0db8:abcd:0012::99", 64), "2001:db8:abcd:12::/64");
        assert_eq!(ip_bucket("2001:db8::1", 64), "2001:db8:0:0::/64");
        assert_eq!(ip_bucket("::1", 64), "0:0:0:0::/64");
        assert_eq!(ip_bucket("2001:DB8:abcd:12:1:2:3:4", 48), "2001:db8:abcd::/48");
        assert_eq!(ip_bucket("fe80::1%eth0", 64), "fe80:0:0:0::/64");
        assert_eq!(ip_bucket("::ffff:192.0.2.1", 64), "::ffff:192.0.2.1", "mapped IPv4 stays whole");
    }

    #[test]
    fn the_client_ip_is_the_last_forwarded_entry_and_missing_fails_closed() {
        let mut h = HeaderMap::new();
        assert_eq!(client_ip(&h).unwrap_err().code, "client_ip_required");
        h.insert("x-forwarded-for", HeaderValue::from_static("6.6.6.6, 203.0.113.9"));
        assert_eq!(client_ip(&h).unwrap(), "203.0.113.9", "a client-chosen first entry is ignored");
        h.insert("x-forwarded-for", HeaderValue::from_static("  "));
        assert_eq!(client_ip(&h).unwrap_err().status, 403);
    }

    #[test]
    fn the_per_ip_limit_trips_per_network_and_a_refund_gives_the_hits_back() {
        let (c, k) = (db(), derive_keys("s"));
        // login: 60 per 5 minutes and 300 per hour per IP.
        let mut last = None;
        for _ in 0..60 {
            last = Some(limit(&c, &k, Action::Login, "10.0.0.1", Some("email:a@x.org"), T).unwrap());
        }
        let e = limit(&c, &k, Action::Login, "10.0.0.1", Some("email:a@x.org"), T).unwrap_err();
        assert_eq!((e.status, e.code.as_str()), (429, "rate_limited"));
        assert!(e.headers.iter().any(|(k, v)| k == "Retry-After" && v.parse::<i64>().unwrap() > 0));
        assert!(limit(&c, &k, Action::Login, "10.9.9.9", None, T).is_ok(), "another network is unaffected");
        assert!(limit(&c, &k, Action::Login, "2001:db8::1", None, T).is_ok());
        last.unwrap().refund(&c);
        assert!(limit(&c, &k, Action::Login, "10.0.0.1", None, T).is_ok(), "one hit came back");
    }

    #[test]
    fn a_refused_request_leaves_no_trace_and_keys_hold_no_raw_address() {
        let (c, k) = (db(), derive_keys("s"));
        for _ in 0..5 {
            limit(&c, &k, Action::Resend, "10.0.0.1", Some("email:a@x.org"), T).unwrap();
        }
        let before = total(&c);
        assert!(limit(&c, &k, Action::Resend, "10.0.0.1", Some("email:a@x.org"), T).is_err());
        assert_eq!(total(&c), before, "the denied hit was rolled back");
        assert!(limit(&c, &k, Action::Resend, "10.0.0.1", Some("email:other@x.org"), T).is_ok(), "per (IP, address)");
        let keys: Vec<String> = c.prepare("SELECT key FROM rate_limits").unwrap().query_map([], |r| r.get(0)).unwrap().map(|r| r.unwrap()).collect();
        assert!(keys.iter().all(|k| !k.contains("10.0.0.1") && !k.contains("a@x.org") && (k.starts_with("a:") || k.starts_with("i:"))), "{keys:?}");
    }

    #[test]
    fn failed_logins_lock_an_account_for_every_network_but_not_a_known_device() {
        let (c, k) = (db(), derive_keys("s"));
        let acct = "email:a@x.org";
        // Thirty failures from thirty different networks hit the hourly per-account rule.
        for i in 0..30 {
            reserve_failure(&c, &k, &format!("10.0.{i}.1"), acct, 3, None, None, T).unwrap();
        }
        let e = reserve_failure(&c, &k, "10.0.99.1", acct, 3, None, None, T).unwrap_err();
        assert_eq!(e.status, 429);
        // The owner's known device is not part of that lockdown.
        let dev = issue_device_token(&k, UID, 3, T, 86400);
        assert!(reserve_failure(&c, &k, "10.0.99.1", acct, 3, Some(&dev), Some(&dev.nonce), T).is_ok());
        // A reset bumps the epoch, which re-keys every counter: a clean slate with no clearing step.
        assert!(reserve_failure(&c, &k, "10.0.99.1", acct, 4, None, None, T).is_ok());
        // Unknown addresses (epoch 0) are limited just the same.
        for i in 0..30 {
            reserve_failure(&c, &k, &format!("10.1.{i}.1"), "email:nobody@x.org", 0, None, None, T).unwrap();
        }
        assert!(reserve_failure(&c, &k, "10.1.99.1", "email:nobody@x.org", 0, None, None, T).is_err());
    }

    #[test]
    fn one_network_gets_five_failures_per_account_then_a_release_gives_the_slot_back() {
        let (c, k) = (db(), derive_keys("s"));
        let mut slots = vec![];
        for _ in 0..5 {
            slots.push(reserve_failure(&c, &k, "10.0.0.1", "email:a@x.org", 1, None, None, T).unwrap());
        }
        assert!(reserve_failure(&c, &k, "10.0.0.1", "email:a@x.org", 1, None, None, T).is_err());
        slots.pop().unwrap().refund(&c); // a login that succeeded
        assert!(reserve_failure(&c, &k, "10.0.0.1", "email:a@x.org", 1, None, None, T).is_ok());
        assert!(reserve_failure(&c, &k, "10.0.0.2", "email:a@x.org", 1, None, None, T).is_ok(), "same /48-less IPv4 network differs");
    }

    #[test]
    fn a_stolen_second_device_shares_a_cap_but_the_current_device_is_exempt() {
        let (c, k) = (db(), derive_keys("s"));
        let current = issue_device_token(&k, UID, 1, T, 86400);
        // Forty other-device failures per day across different tokens exhaust the shared cap...
        for _ in 0..40 {
            let other = issue_device_token(&k, UID, 1, T, 86400);
            reserve_failure(&c, &k, "10.0.0.1", "email:a@x.org", 1, Some(&other), Some(&current.nonce), T).unwrap();
        }
        let other = issue_device_token(&k, UID, 1, T, 86400);
        assert!(reserve_failure(&c, &k, "10.0.0.1", "email:a@x.org", 1, Some(&other), Some(&current.nonce), T).is_err());
        // ...but the owner's current device is not locked out by it.
        assert!(reserve_failure(&c, &k, "10.0.0.1", "email:a@x.org", 1, Some(&current), Some(&current.nonce), T).is_ok());
    }

    #[test]
    fn a_failed_login_is_refunded_to_the_ip_budget_only_within_the_failed_allowance() {
        let (c, k) = (db(), derive_keys("s"));
        let spent = limit(&c, &k, Action::Login, "10.0.0.1", None, T).unwrap();
        let before = total(&c);
        refund_failed(&c, &k, "10.0.0.1", T, &spent);
        // Net: the login hit is given back, one "failed" hit taken.
        assert_eq!(total(&c), before - 2 + 1 * 1, "two login rules refunded, one failed-allowance hit counted");
        // Exhaust the failed allowance: refunds stop.
        for _ in 0..120 {
            let s = limit(&c, &k, Action::Login, "10.0.0.2", None, T).unwrap();
            refund_failed(&c, &k, "10.0.0.2", T, &s);
        }
        let s = limit(&c, &k, Action::Login, "10.0.0.2", None, T).unwrap();
        let before = total(&c);
        refund_failed(&c, &k, "10.0.0.2", T, &s);
        assert_eq!(total(&c), before, "past the allowance the charge stays");
    }

    #[test]
    fn device_tokens_verify_per_account_epoch_and_expiry() {
        let k = derive_keys("s");
        let d = issue_device_token(&k, UID, 2, T, 3600);
        assert_eq!(d.token.split('.').count(), 6);
        assert_eq!(d.nonce.len(), 12);
        assert_ne!(d.token, issue_device_token(&k, UID, 2, T, 3600).token, "every issue is distinct");
        let user = DeviceUser { id: UID, device_epoch: 2 };
        assert_eq!(parse_device(&k, Some(&d.token), Some(&user), T), Some(d.clone()));
        assert!(parse_device(&k, Some(&d.token), Some(&DeviceUser { id: UID, device_epoch: 3 }), T).is_none(), "a bumped epoch revokes");
        assert!(parse_device(&k, Some(&d.token), Some(&DeviceUser { id: "223e4567-e89b-12d3-a456-426614174000", device_epoch: 2 }), T).is_none(), "another account");
        assert!(parse_device(&k, Some(&d.token), Some(&user), T + 3_600_000).is_none(), "expired");
        assert!(parse_device(&derive_keys("other"), Some(&d.token), Some(&user), T).is_none(), "another secret");
        assert!(parse_device(&k, Some(&d.token), None, T).is_none() && parse_device(&k, None, Some(&user), T).is_none());
        let mut forged = d.token.clone();
        forged.replace_range(forged.len() - 1.., if forged.ends_with('A') { "B" } else { "A" });
        assert!(parse_device(&k, Some(&forged), Some(&user), T).is_none(), "a changed MAC");
        for bad in ["", "garbage", "v2.a.b.c.d.e", &d.token.replacen("v1", "v2", 1), &format!("{}.x", d.token)] {
            assert!(parse_device(&k, Some(bad), Some(&user), T).is_none(), "{bad:?}");
        }
    }
}
