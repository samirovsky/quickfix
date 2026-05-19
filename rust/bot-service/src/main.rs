use anyhow::Context;
use std::path::Path;
use tracing_subscriber::EnvFilter;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")),
        )
        .init();

    let cfg = bot_service::config::ServiceConfig::from_env();
    tracing::info!(?cfg, "bot-service starting");

    let state = bot_service::build_state(&cfg.database_url)
        .await
        .context("building app state")?;

    if let Some(path) = &cfg.seed_keys_path {
        bot_service::seed_keys_from_file(&state, Path::new(path))
            .await
            .context("seeding api keys")?;
    }
    if cfg.seed_demo {
        bot_service::seed::seed_if_empty(&state)
            .await
            .context("seeding demo data")?;
    }

    let cors = bot_service::cors_layer(&cfg.cors_origins);
    if !cfg.cors_origins.is_empty() {
        tracing::info!(origins = ?cfg.cors_origins, "CORS enabled");
    }

    // One shared price cache: the gRPC client writes, the paper engine reads.
    let price_cache = bot_service::price_cache::PriceCache::new();

    if cfg.enable_paper_engine {
        let pool = state.db.clone();
        let secs = cfg.paper_tick_secs;
        let cache = price_cache.clone();
        tokio::spawn(async move {
            bot_service::paper_engine::run(pool, secs, cache).await;
        });
    }
    if let Some(url) = cfg.trading_grpc_url.clone() {
        let cache = price_cache.clone();
        tokio::spawn(async move {
            bot_service::trading_client::run(url, cache).await;
        });
    }

    bot_service::serve(cfg.bind, state, cors).await
}
