//! Auth configuration (port of the defaults and `resolve` in auth/src/auth.ts, fixed to the one configuration the
//! pool Worker passes in worker/src/index.ts `auth()`): trust levels, issuer and audience, paths, lifetimes and the
//! per-action rate-limit rules.
use super::password::Argon2Params;
use crate::ratelimit::Rule;

pub const MIN: i64 = 60;
pub const HOUR: i64 = 3600;
pub const DAY: i64 = 86400;

/// docs/phase1-argon2-spike.md
pub const ARGON2: Argon2Params = Argon2Params { m: 19456, t: 2, p: 1 };

/// Trust levels in order; a new account starts at the first, and a confirmed address moves it to `after_verify`.
pub const TRUST: [&str; 4] = ["unverified", "pending", "teacher", "admin"];
pub const AFTER_VERIFY: &str = "pending";
pub const APPROVED_LEVEL: &str = "teacher";
pub const ADMIN_LEVELS: [&str; 1] = ["admin"];

pub struct Paths {
    pub signup: &'static str,
    pub login: &'static str,
    pub refresh: &'static str,
    pub logout: &'static str,
    pub verify: &'static str,
    pub resend: &'static str,
    pub forgot: &'static str,
    pub reset: &'static str,
    pub me: &'static str,
    pub account: &'static str,
    pub export: &'static str,
    pub enrol: &'static str,
    pub enrolment_code: &'static str,
    pub reauth: &'static str,
    pub reapply: &'static str,
    pub admin: &'static str,
    pub admin_revoke: &'static str,
}

pub const PATHS: Paths = Paths {
    signup: "/auth/signup",
    login: "/auth/login",
    refresh: "/auth/refresh",
    logout: "/auth/logout",
    verify: "/auth/verify",
    resend: "/auth/resend",
    forgot: "/auth/forgot",
    reset: "/auth/reset",
    me: "/me",
    account: "/account",
    export: "/account/export",
    enrol: "/auth/device/enrol",
    enrolment_code: "/auth/device/enrolment-code",
    reauth: "/auth/reauth",
    reapply: "/account/reapply",
    admin: "/admin",
    // The pool Worker overrides this one (`paths: { adminRevoke: "/revoke-teacher" }`).
    admin_revoke: "/revoke-teacher",
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Action {
    Signup,
    Login,
    Refresh,
    Verify,
    Resend,
    Forgot,
    Reset,
    Delete,
    Enrol,
    Reauth,
    Admin,
    Reapply,
}

pub struct FailureLimits {
    pub per_account_ip: Rule,
    pub per_account: &'static [Rule],
    pub per_device: &'static [Rule],
    pub per_account_device: &'static [Rule],
}

#[derive(Default)]
pub struct ActionLimits {
    pub ip: &'static [Rule],
    pub account: &'static [Rule],
    pub ip_account: &'static [Rule],
    pub ip_failed: &'static [Rule],
    pub failures: Option<FailureLimits>,
}

macro_rules! r {
    ($limit:expr, $window:expr) => {
        Rule { limit: $limit, window_seconds: $window }
    };
}

/// How long a claim on a one-time code lasts before another request may claim it.
pub const LEASE_MS: i64 = 30_000;
/// Wrong passwords allowed with one enrolment code before it is burned.
pub const ENROL_MAX_FAILURES: i64 = 5;
/// Enrolment codes per account per 24 h (all sources).
pub const ENROL_DAILY: i64 = 5;
pub const UA_MAX: usize = 256;

pub fn limits(a: Action) -> ActionLimits {
    match a {
        // Generous per IP because a school's staff may share one NAT address.
        Action::Signup => ActionLimits { ip: &[r!(20, HOUR)], account: &[r!(5, HOUR)], ..Default::default() },
        Action::Login => ActionLimits {
            // Every attempt counts per IP (this bounds hashing CPU). Per account only failures count.
            ip: &[r!(60, 5 * MIN), r!(300, HOUR)],
            // Every failed login gets its per-IP charge back within this separate allowance, so floods of wrong
            // passwords cannot starve real logins on a shared address.
            ip_failed: &[r!(120, 5 * MIN)],
            failures: Some(FailureLimits {
                per_account_ip: r!(5, 15 * MIN),
                // 30 an hour stops bursts; 100 a day caps a patient attacker who stays under the hourly rule.
                per_account: &[r!(30, HOUR), r!(100, DAY)],
                per_device: &[r!(10, 15 * MIN), r!(20, DAY)],
                // All of an account's devices together; never part of the lockdown that refuses other logins.
                per_account_device: &[r!(40, DAY)],
            }),
            ..Default::default()
        },
        Action::Refresh => ActionLimits { ip: &[r!(120, MIN)], ..Default::default() },
        // Verify and reset charge the IP only for an invalid token, so junk never blocks a valid link.
        Action::Verify => ActionLimits { ip: &[r!(30, HOUR)], ..Default::default() },
        // Resend and forgot: only per (IP, address), so junk aimed at other addresses never uses up an owner's allowance.
        Action::Resend | Action::Forgot => ActionLimits { ip_account: &[r!(5, HOUR)], ..Default::default() },
        // Charged only for an unknown enrolment code.
        Action::Enrol => ActionLimits { ip: &[r!(20, HOUR)], ..Default::default() },
        // Re-entering the password for a sudo token: its own per-account limiter, like deletion.
        Action::Reauth | Action::Delete => ActionLimits { ip: &[r!(20, HOUR)], account: &[r!(5, HOUR), r!(10, DAY)], ..Default::default() },
        Action::Admin => ActionLimits { ip: &[r!(600, 5 * MIN)], account: &[r!(600, 5 * MIN)], ..Default::default() },
        Action::Reapply => ActionLimits { ip: &[r!(20, HOUR)], account: &[r!(5, DAY)], ..Default::default() },
        Action::Reset => ActionLimits { ip: &[r!(20, HOUR)], ..Default::default() },
    }
}

pub struct AuthConfig {
    /// 32+ characters; the one secret the six purpose keys are derived from.
    pub secret: String,
    /// Exact origins of the web build.
    pub allowed_origins: Vec<String>,
    pub issuer: String,
    pub audience: String,
    pub argon2: Argon2Params,
    pub cookie_name: String,
    pub device_cookie_name: String,
    pub access_ttl: i64,
    pub refresh_ttl: i64,
    pub session_max: i64,
    pub grace_ms: i64,
    pub device_ttl: i64,
    pub mail_gap_ms: i64,
    pub mail_ceiling: i64,
    pub outstanding_links: i64,
    pub enrol_ttl: i64,
    pub verify_ttl: i64,
    pub reset_ttl: i64,
    pub tombstone_ttl: i64,
    pub min_password: usize,
    /// Never-confirmed accounts are removed after a week, freeing a squatted address.
    pub purge_unverified_after: i64,
    pub reapply_cooldown: i64,
    pub escalate_after: i64,
    pub digest: bool,
    pub bulk_reject_max: i64,
    pub admin_proposal_ttl: i64,
    pub admin_self_confirm_after: i64,
}

/// An exact origin: `scheme://host[:port]`, no path, no wildcard, and unchanged by URL normalisation.
fn exact_origin(o: &str) -> bool {
    if o.contains('*') {
        return false;
    }
    let Some((scheme, rest)) = o.split_once("://") else { return false };
    matches!(scheme, "http" | "https") && !rest.is_empty() && !rest.contains(['/', '?', '#', '@', ' ']) && rest == rest.to_ascii_lowercase()
}

impl AuthConfig {
    /// Fails (with the Worker's messages) on a short secret or an origin that is not exact.
    pub fn new(secret: &str, allowed_origins: Vec<String>) -> Result<AuthConfig, String> {
        if secret.chars().count() < 32 {
            return Err("auth: secret must be at least 32 characters".into());
        }
        for o in &allowed_origins {
            if !exact_origin(o) {
                return Err(format!("auth: allowed origin must be an exact origin: {o}"));
            }
        }
        Ok(AuthConfig {
            secret: secret.to_string(),
            allowed_origins,
            issuer: "vershion-pool".into(),
            audience: "vershion-pool".into(),
            argon2: ARGON2,
            cookie_name: "__Host-refresh".into(),
            device_cookie_name: "__Host-device".into(),
            access_ttl: 15 * MIN,
            refresh_ttl: 30 * DAY,
            session_max: 90 * DAY,
            grace_ms: 10_000,
            device_ttl: 90 * DAY,
            mail_gap_ms: 120_000,
            mail_ceiling: 10,
            outstanding_links: 3,
            enrol_ttl: 15 * MIN,
            verify_ttl: DAY,
            reset_ttl: HOUR,
            tombstone_ttl: 14 * DAY,
            min_password: 10,
            purge_unverified_after: 7 * DAY,
            reapply_cooldown: 7 * DAY,
            escalate_after: 48 * HOUR,
            digest: false,
            bulk_reject_max: 50,
            admin_proposal_ttl: 3 * DAY,
            admin_self_confirm_after: DAY,
        })
    }

    pub fn is_allowed_origin(&self, origin: &str) -> bool {
        self.allowed_origins.iter().any(|o| o == origin)
    }
}

pub fn trust_rank(level: &str) -> Option<usize> {
    TRUST.iter().position(|t| *t == level)
}

#[cfg(test)]
mod tests {
    use super::*;

    const SECRET: &str = "0123456789abcdef0123456789abcdef";

    #[test]
    fn the_secret_and_origins_are_validated_with_the_workers_messages() {
        assert!(AuthConfig::new("short", vec![]).err().unwrap().contains("at least 32 characters"));
        assert!(AuthConfig::new(&"x".repeat(31), vec![]).is_err());
        assert!(AuthConfig::new(SECRET, vec![]).is_ok());
        let ok = vec!["https://app.example.org".to_string(), "http://localhost:5173".to_string()];
        let c = AuthConfig::new(SECRET, ok).unwrap();
        assert!(c.is_allowed_origin("https://app.example.org") && !c.is_allowed_origin("https://app.example.org/") && !c.is_allowed_origin("https://evil.example"));
        for bad in ["*", "https://*.example.org", "https://app.example.org/", "https://app.example.org/path", "app.example.org", "ftp://x.example", "https://App.Example.org", "https://u@app.example.org", ""] {
            let e = AuthConfig::new(SECRET, vec![bad.to_string()]).err();
            assert_eq!(e, Some(format!("auth: allowed origin must be an exact origin: {bad}")), "{bad:?}");
        }
    }

    #[test]
    fn lifetimes_match_the_workers_defaults_for_the_pool() {
        let c = AuthConfig::new(SECRET, vec![]).unwrap();
        assert_eq!((c.issuer.as_str(), c.audience.as_str(), c.argon2), ("vershion-pool", "vershion-pool", Argon2Params { m: 19456, t: 2, p: 1 }));
        assert_eq!((c.access_ttl, c.refresh_ttl, c.session_max, c.grace_ms), (900, 30 * 86400, 90 * 86400, 10_000));
        assert_eq!((c.verify_ttl, c.reset_ttl, c.tombstone_ttl, c.purge_unverified_after), (86400, 3600, 14 * 86400, 7 * 86400));
        assert_eq!((c.mail_gap_ms, c.mail_ceiling, c.outstanding_links, c.min_password, c.enrol_ttl), (120_000, 10, 3, 10, 900));
        assert_eq!((c.cookie_name.as_str(), c.device_cookie_name.as_str()), ("__Host-refresh", "__Host-device"));
    }

    #[test]
    fn trust_levels_are_ordered_and_a_confirmed_address_lands_on_pending() {
        assert_eq!(trust_rank("unverified"), Some(0));
        assert!(trust_rank(AFTER_VERIFY) < trust_rank(APPROVED_LEVEL) && trust_rank(APPROVED_LEVEL) < trust_rank("admin"));
        assert_eq!(trust_rank("nobody"), None);
        assert!(ADMIN_LEVELS.iter().all(|l| trust_rank(l).is_some()));
    }

    #[test]
    fn paths_and_the_overridden_revoke_path() {
        assert_eq!((PATHS.login, PATHS.refresh, PATHS.me, PATHS.enrol, PATHS.export), ("/auth/login", "/auth/refresh", "/me", "/auth/device/enrol", "/account/export"));
        assert_eq!(PATHS.admin_revoke, "/revoke-teacher");
    }

    #[test]
    fn login_limits_are_the_documented_ones() {
        let l = limits(Action::Login);
        assert_eq!(l.ip.iter().map(|r| (r.limit, r.window_seconds)).collect::<Vec<_>>(), [(60, 300), (300, 3600)]);
        assert_eq!(l.ip_failed[0].limit, 120);
        let f = l.failures.unwrap();
        assert_eq!((f.per_account_ip.limit, f.per_account_ip.window_seconds), (5, 900));
        assert_eq!(f.per_account.len() + f.per_device.len() + f.per_account_device.len(), 5);
        assert_eq!(limits(Action::Refresh).ip[0].limit, 120);
        assert_eq!(limits(Action::Resend).ip_account[0].limit, 5);
        assert_eq!(limits(Action::Forgot).ip_account[0].limit, 5);
        assert!(limits(Action::Resend).ip.is_empty(), "no per-IP cap on mail actions");
        assert_eq!(limits(Action::Delete).account.len(), 2);
        assert_eq!(limits(Action::Reauth).account[1].window_seconds, 86400);
    }
}
