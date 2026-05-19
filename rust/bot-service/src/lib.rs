//! bot-service library entrypoint.
//!
//! The binary in `src/main.rs` is a thin wrapper; everything testable is
//! exposed here so integration tests can spawn the service on a random
//! port without going through the CLI.

use std::net::SocketAddr;
use std::path::Path;
use std::sync::Arc;

use anyhow::{Context, Result};
use axum::middleware;
use axum::routing::{get, post};
use axum::Router;
use sqlx::SqlitePool;
use tokio::net::TcpListener;
use tower_http::trace::TraceLayer;

pub mod auth;
pub mod config;
pub mod db;
pub mod error;
pub mod models;
pub mod routes;
pub mod state;
pub mod store;

pub use state::AppState;

const TEMPLATES_JSON: &str = include_str!("../templates/library.json");

pub fn load_templates() -> Result<Vec<models::template::Template>> {
    serde_json::from_str(TEMPLATES_JSON).context("parsing bundled templates/library.json")
}

pub async fn build_state(database_url: &str) -> Result<AppState> {
    let db: SqlitePool = db::connect(database_url).await?;
    let templates = Arc::new(load_templates()?);
    Ok(AppState { db, templates })
}

pub fn router(state: AppState) -> Router {
    let authed = Router::new()
        .route("/v1/templates", get(routes::templates::list))
        .route("/v1/templates/:id", get(routes::templates::get))
        .route(
            "/v1/bots",
            post(routes::bots::create).get(routes::bots::list),
        )
        .route(
            "/v1/bots/from-template/:id",
            post(routes::bots::create_from_template),
        )
        .route(
            "/v1/bots/:id",
            get(routes::bots::get)
                .put(routes::bots::update)
                .delete(routes::bots::delete),
        )
        .route("/v1/bots/:id/status", post(routes::bots::set_status))
        .route("/v1/billing/usage", get(routes::billing::usage))
        .route("/v1/billing/estimate", post(routes::billing::estimate))
        .route_layer(middleware::from_fn_with_state(
            state.clone(),
            auth::require_api_key,
        ));

    Router::new()
        .route("/healthz", get(routes::health::healthz))
        .merge(authed)
        .layer(TraceLayer::new_for_http())
        .with_state(state)
}

pub async fn seed_keys_from_file(state: &AppState, path: &Path) -> Result<()> {
    let raw = tokio::fs::read_to_string(path)
        .await
        .with_context(|| format!("reading seed key file {}", path.display()))?;
    let keys: Vec<config::SeedKey> = serde_json::from_str(&raw).context("parsing seed key JSON")?;
    for key in keys {
        let user = store::user_repo::upsert_with_key(&state.db, &key.name, &key.api_key).await?;
        tracing::info!(user_id = %user.id, name = %user.name, "seeded user");
    }
    Ok(())
}

/// Build the router around `state` and serve it on `addr`. Intended both
/// for `main.rs` and for integration tests that need a live server.
pub async fn serve(addr: SocketAddr, state: AppState) -> Result<()> {
    let app = router(state);
    let listener = TcpListener::bind(addr).await?;
    let real_addr = listener.local_addr()?;
    tracing::info!(addr = %real_addr, "bot-service listening");
    axum::serve(listener, app).await?;
    Ok(())
}
