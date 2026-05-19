//! bot-service library entrypoint.
//!
//! The binary in `src/main.rs` is a thin wrapper; everything testable is
//! exposed here so integration tests can spawn the service on a random
//! port without going through the CLI.

use std::net::SocketAddr;
use std::path::Path;
use std::sync::Arc;

use anyhow::{Context, Result};
use axum::http::{HeaderName, HeaderValue, Method};
use axum::middleware;
use axum::routing::{get, post};
use axum::Router;
use sqlx::SqlitePool;
use tokio::net::TcpListener;
use tower_http::cors::{Any, CorsLayer};
use tower_http::trace::TraceLayer;

pub mod auth;
pub mod config;
pub mod db;
pub mod error;
pub mod models;
pub mod paper_engine;
pub mod routes;
pub mod seed;
pub mod state;
pub mod store;

pub use state::AppState;

const TEMPLATES_JSON: &str = include_str!("../templates/library.json");

/// QFTX price scale factor: 1 USD = 1e8 ticks. The matching engine uses
/// fixed-point i64 prices at this scale; the seeder generates demo prices
/// in the same units so a slice-4 engine can be swapped in without
/// rewriting the trade-recording logic.
pub const PRICE_TICKS_PER_DOLLAR: i64 = 100_000_000;

pub fn load_templates() -> Result<Vec<models::template::Template>> {
    serde_json::from_str(TEMPLATES_JSON).context("parsing bundled templates/library.json")
}

pub async fn build_state(database_url: &str) -> Result<AppState> {
    let db: SqlitePool = db::connect(database_url).await?;
    let templates = Arc::new(load_templates()?);
    Ok(AppState { db, templates })
}

/// Build a [`CorsLayer`] from a list of allowed origins.
///
/// - Empty list → CORS layer is **not** applied; the caller decides.
/// - `["*"]` → any-origin without credentials (demo / public API).
/// - Otherwise → exact allowlist with `X-API-Key` and `Content-Type`
///   allowed in requests, suitable for a deployed UI calling a
///   deployed bot-service across origins.
pub fn cors_layer(origins: &[String]) -> Option<CorsLayer> {
    if origins.is_empty() {
        return None;
    }
    let methods = [
        Method::GET,
        Method::POST,
        Method::PUT,
        Method::DELETE,
        Method::OPTIONS,
    ];
    let headers: [HeaderName; 2] = [
        HeaderName::from_static("x-api-key"),
        HeaderName::from_static("content-type"),
    ];
    let layer = if origins.iter().any(|o| o == "*") {
        CorsLayer::new()
            .allow_origin(Any)
            .allow_methods(methods)
            .allow_headers(headers)
    } else {
        let parsed: Vec<HeaderValue> = origins
            .iter()
            .filter_map(|o| HeaderValue::from_str(o).ok())
            .collect();
        CorsLayer::new()
            .allow_origin(parsed)
            .allow_methods(methods)
            .allow_headers(headers)
    };
    Some(layer)
}

pub fn router(state: AppState) -> Router {
    router_with_cors(state, None)
}

pub fn router_with_cors(state: AppState, cors: Option<CorsLayer>) -> Router {
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
        .route("/v1/bots/:id/publish", post(routes::marketplace::publish))
        .route(
            "/v1/bots/:id/performance",
            get(routes::marketplace::bot_performance),
        )
        .route(
            "/v1/marketplace/listings",
            get(routes::marketplace::list_listings),
        )
        .route(
            "/v1/marketplace/listings/:id",
            get(routes::marketplace::get_listing),
        )
        .route(
            "/v1/marketplace/listings/:id/unpublish",
            post(routes::marketplace::unpublish),
        )
        .route(
            "/v1/marketplace/listings/:id/subscribe",
            post(routes::marketplace::subscribe),
        )
        .route(
            "/v1/marketplace/listings/:id/performance",
            get(routes::marketplace::listing_performance),
        )
        .route(
            "/v1/subscriptions/:id/cancel",
            post(routes::marketplace::cancel_subscription),
        )
        .route(
            "/v1/me/subscriptions",
            get(routes::marketplace::my_subscriptions),
        )
        .route("/v1/billing/usage", get(routes::billing::usage))
        .route("/v1/billing/estimate", post(routes::billing::estimate))
        .route("/v1/ai/generate-strategy", post(routes::ai::generate))
        .route_layer(middleware::from_fn_with_state(
            state.clone(),
            auth::require_api_key,
        ));

    let app = Router::new()
        .route("/healthz", get(routes::health::healthz))
        .merge(authed)
        .layer(TraceLayer::new_for_http());
    let app = match cors {
        // CORS sits outside everything so preflight OPTIONS short-circuit
        // before they hit the auth middleware.
        Some(layer) => app.layer(layer),
        None => app,
    };
    app.with_state(state)
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
pub async fn serve(addr: SocketAddr, state: AppState, cors: Option<CorsLayer>) -> Result<()> {
    let app = router_with_cors(state, cors);
    let listener = TcpListener::bind(addr).await?;
    let real_addr = listener.local_addr()?;
    tracing::info!(addr = %real_addr, "bot-service listening");
    axum::serve(listener, app).await?;
    Ok(())
}
