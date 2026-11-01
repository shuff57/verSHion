//! Configuration: environment variables only, the same names the Worker used for its vars and secrets.
//! Secrets are read here and nowhere else, never printed, and never put in argv.
use crate::ai::policy::Limits;
use crate::ai::upstream::{DEFAULT_MODEL, OpenRouter};
use std::path::PathBuf;
use std::time::Duration;

pub struct Config {
    pub db: PathBuf,
    pub blobs: PathBuf,
    pub migrations: PathBuf,
    pub listen: String,
    pub relay_key: Option<String>,
    pub allowed_origins: Vec<String>,
    pub limits: Limits,
    pub job_ttl_ms: i64,
    pub openrouter: Option<OpenRouter>,
    /// "1" turns buying credits on; the webhook works whenever its secret is set.
    pub billing_enabled: bool,
    pub stripe_key: Option<String>,
    pub stripe_webhook_secret: Option<String>,
    pub stripe_publishable_key: Option<String>,
    /// 32+ characters. Without it nobody can sign in: the AI relay and billing see only the relay key and the native app.
    pub auth_secret: Option<String>,
    /// 32+ characters (POOL_HASH_KEY). Without it the pool is not mounted: it never hashes unkeyed.
    pub pool_hash_key: Option<String>,
}

/// A positive whole number, or None (so the default stands) when unset or not one.
fn pos_int(v: Option<String>) -> Option<i64> {
    v?.trim().parse::<i64>().ok().filter(|n| *n > 0)
}

/// A positive number (dollars may be fractional), or None when unset or not one.
fn pos_num(v: Option<String>) -> Option<f64> {
    v?.trim().parse::<f64>().ok().filter(|n| n.is_finite() && *n > 0.0)
}

fn nonempty(v: Option<String>) -> Option<String> {
    v.map(|s| s.trim().to_string()).filter(|s| !s.is_empty())
}

impl Config {
    pub fn from_env() -> Config {
        Config::from_lookup(|k| std::env::var(k).ok())
    }

    pub fn from_lookup(get: impl Fn(&str) -> Option<String>) -> Config {
        let or_default = |k: &str, d: &str| nonempty(get(k)).unwrap_or_else(|| d.to_string());
        let mut limits = Limits::default();
        if let Some(n) = pos_int(get("AI_CHAT_PER_HOUR")) {
            limits.chat_per_hour = n;
        }
        if let Some(n) = pos_int(get("AI_IMPORT_PER_DAY")) {
            limits.import_per_day = n;
        }
        if let Some(n) = pos_int(get("AI_WEEKLY_CREDITS")) {
            limits.weekly_credits = n;
        }
        if let Some(n) = pos_int(get("AI_MAX_PAGES")) {
            limits.max_pages = n;
        }
        if let Some(n) = pos_num(get("AI_DAILY_USD")) {
            limits.daily_usd = n;
        }
        let openrouter = nonempty(get("OPENROUTER_API_KEY")).map(|key| OpenRouter {
            key,
            model: nonempty(get("OPENROUTER_MODEL")).unwrap_or_else(|| DEFAULT_MODEL.to_string()),
            fallback_model: nonempty(get("OPENROUTER_FALLBACK_MODEL")),
            // Providers that may keep prompts are only allowed when the deployment says so.
            privacy: get("OPENROUTER_PRIVACY").as_deref() != Some("0"),
            timeout: Duration::from_secs(60),
        });
        Config {
            db: or_default("VERSHION_DB", "./data/pool.db").into(),
            blobs: or_default("VERSHION_BLOBS", "./data/blobs").into(),
            migrations: or_default("VERSHION_MIGRATIONS", "./migrations").into(),
            listen: or_default("VERSHION_LISTEN", "127.0.0.1:8787"),
            relay_key: nonempty(get("AI_RELAY_KEY")),
            allowed_origins: get("ALLOWED_ORIGINS").unwrap_or_default().split(',').map(|s| s.trim().to_string()).filter(|s| !s.is_empty()).collect(),
            limits,
            job_ttl_ms: pos_int(get("AI_JOB_TTL_HOURS")).unwrap_or(72) * 3_600_000,
            openrouter,
            billing_enabled: get("BILLING_ENABLED").as_deref().map(str::trim) == Some("1"),
            stripe_key: nonempty(get("STRIPE_SECRET_KEY")),
            stripe_webhook_secret: nonempty(get("STRIPE_WEBHOOK_SECRET")),
            stripe_publishable_key: nonempty(get("STRIPE_PUBLISHABLE_KEY")),
            auth_secret: nonempty(get("AUTH_SECRET")),
            pool_hash_key: nonempty(get("POOL_HASH_KEY")),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    fn cfg(pairs: &[(&str, &str)]) -> Config {
        let m: HashMap<String, String> = pairs.iter().map(|(k, v)| (k.to_string(), v.to_string())).collect();
        Config::from_lookup(|k| m.get(k).cloned())
    }

    #[test]
    fn defaults_when_nothing_is_set() {
        let c = cfg(&[]);
        assert_eq!((c.listen.as_str(), c.db.to_str().unwrap(), c.job_ttl_ms), ("127.0.0.1:8787", "./data/pool.db", 72 * 3_600_000));
        assert!(c.openrouter.is_none() && c.relay_key.is_none() && c.allowed_origins.is_empty());
        let d = Limits::default();
        assert_eq!((c.limits.chat_per_hour, c.limits.weekly_credits, c.limits.daily_usd), (d.chat_per_hour, d.weekly_credits, d.daily_usd));
    }

    #[test]
    fn openrouter_needs_a_key_and_keeps_privacy_on_unless_told_otherwise() {
        assert!(cfg(&[("OPENROUTER_API_KEY", "   ")]).openrouter.is_none());
        let o = cfg(&[("OPENROUTER_API_KEY", " or-key ")]).openrouter.unwrap();
        assert_eq!((o.key.as_str(), o.model.as_str(), o.privacy, o.fallback_model), ("or-key", DEFAULT_MODEL, true, None));
        let o = cfg(&[("OPENROUTER_API_KEY", "k"), ("OPENROUTER_MODEL", "m/one"), ("OPENROUTER_FALLBACK_MODEL", "m/two"), ("OPENROUTER_PRIVACY", "0")]).openrouter.unwrap();
        assert_eq!((o.model.as_str(), o.fallback_model.as_deref(), o.privacy), ("m/one", Some("m/two"), false));
        assert!(cfg(&[("OPENROUTER_API_KEY", "k"), ("OPENROUTER_PRIVACY", "false")]).openrouter.unwrap().privacy, "only the exact value 0 relaxes it");
    }

    #[test]
    fn limits_take_positive_values_and_ignore_the_rest() {
        let c = cfg(&[("AI_CHAT_PER_HOUR", "7"), ("AI_IMPORT_PER_DAY", "0"), ("AI_WEEKLY_CREDITS", "-3"), ("AI_MAX_PAGES", "ten"), ("AI_DAILY_USD", "0.25"), ("AI_JOB_TTL_HOURS", "24")]);
        let d = Limits::default();
        assert_eq!(c.limits.chat_per_hour, 7);
        assert_eq!((c.limits.import_per_day, c.limits.weekly_credits, c.limits.max_pages), (d.import_per_day, d.weekly_credits, d.max_pages));
        assert_eq!((c.limits.daily_usd, c.job_ttl_ms), (0.25, 24 * 3_600_000));
        assert_eq!(cfg(&[("AI_DAILY_USD", "NaN")]).limits.daily_usd, d.daily_usd);
        assert_eq!(cfg(&[("AI_DAILY_USD", "inf")]).limits.daily_usd, d.daily_usd);
    }

    #[test]
    fn billing_is_off_unless_exactly_one_and_the_stripe_values_are_trimmed() {
        let c = cfg(&[]);
        assert!(!c.billing_enabled && c.stripe_key.is_none() && c.stripe_webhook_secret.is_none() && c.stripe_publishable_key.is_none());
        for v in ["0", "true", "yes", ""] {
            assert!(!cfg(&[("BILLING_ENABLED", v)]).billing_enabled, "{v:?}");
        }
        let c = cfg(&[("BILLING_ENABLED", " 1 "), ("STRIPE_SECRET_KEY", " sk_test_x "), ("STRIPE_WEBHOOK_SECRET", "whsec_x"), ("STRIPE_PUBLISHABLE_KEY", "  ")]);
        assert_eq!((c.billing_enabled, c.stripe_key.as_deref(), c.stripe_webhook_secret.as_deref(), c.stripe_publishable_key), (true, Some("sk_test_x"), Some("whsec_x"), None));
    }

    #[test]
    fn origins_are_a_trimmed_comma_list_and_the_relay_key_is_trimmed() {
        let c = cfg(&[("ALLOWED_ORIGINS", " https://a.example , https://b.example,, "), ("AI_RELAY_KEY", " secret ")]);
        assert_eq!(c.allowed_origins, ["https://a.example", "https://b.example"]);
        assert_eq!(c.relay_key.as_deref(), Some("secret"));
    }
}
