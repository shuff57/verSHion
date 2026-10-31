//! vershion-api: the pool, AI relay and billing server (docs/hosting-port.md).
//!
//! Configuration is environment variables only (see `config.rs`); secrets are never read from argv or logged.
mod ai;
mod auth;
mod billing;
mod blobs;
mod body;
mod config;
mod db;
mod ratelimit;

use ai::routes::{AiState, NoAuth, system_clock, tokio_spawn};
use ai::upstream::HttpTransport;
use billing::{BillingState, HttpStripe};
use axum::{Json, Router, routing::get};
use serde_json::{Value, json};
use std::sync::{Arc, Mutex};

/// `share-core`'s own health value, so a running server proves the shared crate is linked and sane.
async fn health() -> Json<Value> {
    Json(json!({ "ok": true, "share_core": share_core::health() }))
}

pub fn router() -> Router {
    Router::new().route("/health", get(health))
}

/// Everything the server serves so far: its own health check and the AI relay.
fn app(ai: Arc<AiState<HttpTransport>>, billing: Arc<BillingState<HttpStripe>>) -> Router {
    router().merge(ai::routes::router(ai)).merge(billing::router(billing))
}

/// Who is signed in: the pool's sessions when `AUTH_SECRET` is valid, otherwise nobody (only the relay key and the
/// keyless native app can call, and the buying routes answer 401). Each gets its own connection to the file.
fn authenticator(cfg: &config::Config) -> Box<dyn ai::routes::Authenticator> {
    let Some(secret) = &cfg.auth_secret else {
        tracing::warn!("AUTH_SECRET is not set: nobody can sign in");
        return Box::new(NoAuth);
    };
    match (auth::config::AuthConfig::new(secret, cfg.allowed_origins.clone()), db::open(&cfg.db)) {
        (Ok(a), Ok(conn)) => Box::new(auth::session::SessionAuth::new(conn, Arc::new(a))),
        (Err(e), _) => {
            tracing::error!("{e}");
            Box::new(NoAuth)
        }
        (_, Err(e)) => {
            tracing::error!("auth: could not open the database: {e}");
            Box::new(NoAuth)
        }
    }
}

/// The billing state for one database file: its own connection (WAL lets both write), Stripe settings from the config.
fn billing_state(cfg: &config::Config, conn: rusqlite::Connection) -> BillingState<HttpStripe> {
    let mut b = BillingState::new(conn, HttpStripe::new(), authenticator(cfg), system_clock());
    b.enabled = cfg.billing_enabled;
    b.stripe_key = cfg.stripe_key.clone();
    b.webhook_secret = cfg.stripe_webhook_secret.clone();
    b.publishable_key = cfg.stripe_publishable_key.clone();
    b
}

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt::init();
    let cfg = config::Config::from_env();
    if let Some(dir) = cfg.db.parent() {
        std::fs::create_dir_all(dir).expect("create the database directory");
    }
    let mut conn = db::open(&cfg.db).expect("open the database");
    let applied = db::migrate(&mut conn, &cfg.migrations).expect("migrate");
    tracing::info!(applied = applied.len(), "migrations");
    if cfg.openrouter.is_none() {
        tracing::warn!("OPENROUTER_API_KEY is not set: the AI relay answers 503 ai_not_configured");
    }
    if cfg.stripe_webhook_secret.is_none() {
        tracing::warn!("STRIPE_WEBHOOK_SECRET is not set: /billing/webhook answers 503");
    }
    let billing = Arc::new(billing_state(&cfg, db::open(&cfg.db).expect("open the billing connection")));
    let job_ttl_ms = cfg.job_ttl_ms;
    let ai_auth = authenticator(&cfg);
    let state = Arc::new(AiState {
        conn: Mutex::new(conn),
        limits: cfg.limits,
        openrouter: cfg.openrouter,
        transport: HttpTransport::new(),
        auth: ai_auth,
        relay_key: cfg.relay_key,
        allowed_origins: cfg.allowed_origins,
        credits: true,
        clock: system_clock(),
        blobs: Some(blobs::BlobStore::new(&cfg.blobs).expect("open the blob directory")),
        spawn: tokio_spawn(),
    });

    // The Worker's hourly cron, AI stage: expired jobs and old usage rows.
    let hourly = state.clone();
    let hourly_billing = billing.clone();
    tokio::spawn(async move {
        let mut tick = tokio::time::interval(std::time::Duration::from_secs(3600));
        loop {
            tick.tick().await;
            let dropped = ai::cron::prune(&hourly, job_ttl_ms);
            let limits = hourly_billing.with_conn(|c| ratelimit::prune(c, (hourly_billing.clock)()).unwrap_or(0));
            tracing::info!(dropped, limits, "hourly prune");
        }
    });

    let listener = tokio::net::TcpListener::bind(&cfg.listen).await.expect("bind");
    tracing::info!(addr = %cfg.listen, "listening");
    axum::serve(listener, app(state, billing))
        .with_graceful_shutdown(async {
            let _ = tokio::signal::ctrl_c().await;
        })
        .await
        .expect("serve");
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::{body::Body, http::Request};
    use http_body_util::BodyExt;
    use tower::ServiceExt;

    #[tokio::test]
    async fn health_reports_ok_and_the_linked_share_core() {
        let res = router().oneshot(Request::get("/health").body(Body::empty()).unwrap()).await.unwrap();
        assert_eq!(res.status(), 200);
        let body: Value = serde_json::from_slice(&res.into_body().collect().await.unwrap().to_bytes()).unwrap();
        assert_eq!(body["ok"], true);
        assert!(body["share_core"].is_number());
    }

    #[tokio::test]
    async fn unknown_paths_are_404() {
        let res = router().oneshot(Request::get("/nope").body(Body::empty()).unwrap()).await.unwrap();
        assert_eq!(res.status(), 404);
    }

    /// The whole server as main builds it, over a real migrated file: health and the AI health answer, and
    /// a keyless relay without an OpenRouter key says so.
    #[tokio::test]
    async fn the_assembled_app_serves_health_and_the_ai_routes() {
        let dir = tempfile::tempdir().unwrap();
        let cfg = config::Config::from_lookup(|k| match k {
            "VERSHION_DB" => Some(dir.path().join("pool.db").to_string_lossy().into()),
            "VERSHION_BLOBS" => Some(dir.path().join("blobs").to_string_lossy().into()),
            "VERSHION_MIGRATIONS" => Some(concat!(env!("CARGO_MANIFEST_DIR"), "/../worker/migrations").into()),
            _ => None,
        });
        let mut conn = db::open(&cfg.db).unwrap();
        assert!(db::migrate(&mut conn, &cfg.migrations).unwrap().len() >= 26);
        let billing = Arc::new(billing_state(&cfg, db::open(&cfg.db).unwrap()));
        let state = Arc::new(AiState {
            conn: Mutex::new(conn),
            limits: cfg.limits,
            openrouter: cfg.openrouter,
            transport: HttpTransport::new(),
            auth: Box::new(NoAuth),
            relay_key: cfg.relay_key,
            allowed_origins: cfg.allowed_origins,
            credits: true,
            clock: system_clock(),
            blobs: Some(blobs::BlobStore::new(&cfg.blobs).unwrap()),
            spawn: tokio_spawn(),
        });
        let get = |p: &str| Request::get(p).body(Body::empty()).unwrap();
        let ai_health = app(state.clone(), billing.clone()).oneshot(get("/ai/health")).await.unwrap();
        let body: Value = serde_json::from_slice(&ai_health.into_body().collect().await.unwrap().to_bytes()).unwrap();
        assert_eq!(body, json!({ "ok": true, "model": null, "ai": false, "upstream": "openrouter" }));
        assert_eq!(app(state.clone(), billing.clone()).oneshot(get("/health")).await.unwrap().status(), 200);
        let credits = app(state.clone(), billing.clone()).oneshot(get("/ai/credits")).await.unwrap();
        let body: Value = serde_json::from_slice(&credits.into_body().collect().await.unwrap().to_bytes()).unwrap();
        assert_eq!(body["unlimited"], true, "the keyless native app is not on credits");
        // Billing is off by default: buying is a 404, and the webhook says it is not configured.
        assert_eq!(app(state.clone(), billing.clone()).oneshot(get("/billing/packs")).await.unwrap().status(), 404);
        let hook = Request::post("/billing/webhook").body(Body::empty()).unwrap();
        assert_eq!(app(state, billing).oneshot(hook).await.unwrap().status(), 503);
    }
}
