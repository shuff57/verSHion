//! vershion-api: the pool, AI relay and billing server (docs/hosting-port.md, phase 0).
//!
//! Configuration is environment variables only; secrets are never read from argv or logged.
//!   VERSHION_DB       path of the SQLite file            (default ./data/pool.db)
//!   VERSHION_BLOBS    directory for pool bodies and jobs (default ./data/blobs)
//!   VERSHION_MIGRATIONS  directory of NNNN_*.sql files   (default ./migrations)
//!   VERSHION_LISTEN   address to bind                    (default 127.0.0.1:8787; Caddy fronts it)
mod ai;
mod blobs;
mod db;

use axum::{Json, Router, routing::get};
use serde_json::{Value, json};
use std::path::PathBuf;

fn env_or(name: &str, default: &str) -> String {
    std::env::var(name).ok().filter(|v| !v.trim().is_empty()).unwrap_or_else(|| default.to_string())
}

/// `share-core`'s own health value, so a running server proves the shared crate is linked and sane.
async fn health() -> Json<Value> {
    Json(json!({ "ok": true, "share_core": share_core::health() }))
}

pub fn router() -> Router {
    Router::new().route("/health", get(health))
}

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt::init();
    let db_path = PathBuf::from(env_or("VERSHION_DB", "./data/pool.db"));
    if let Some(dir) = db_path.parent() {
        std::fs::create_dir_all(dir).expect("create the database directory");
    }
    let mut conn = db::open(&db_path).expect("open the database");
    let applied = db::migrate(&mut conn, &PathBuf::from(env_or("VERSHION_MIGRATIONS", "./migrations"))).expect("migrate");
    tracing::info!(applied = applied.len(), "migrations");
    let _blobs = blobs::BlobStore::new(env_or("VERSHION_BLOBS", "./data/blobs")).expect("open the blob directory");

    let addr = env_or("VERSHION_LISTEN", "127.0.0.1:8787");
    let listener = tokio::net::TcpListener::bind(&addr).await.expect("bind");
    tracing::info!(%addr, "listening");
    axum::serve(listener, router())
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
}
