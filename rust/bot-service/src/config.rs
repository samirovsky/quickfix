//! Environment-driven configuration. No layered config files, no clap. Per
//! the v1 roadmap.

use serde::Deserialize;
use std::env;
use std::net::SocketAddr;

#[derive(Debug, Clone)]
pub struct ServiceConfig {
    pub bind: SocketAddr,
    pub database_url: String,
    pub seed_keys_path: Option<String>,
    pub seed_demo: bool,
    /// Comma-separated list of allowed CORS origins. Empty list (or
    /// missing env var) → CORS is **not** applied; the service responds
    /// only to same-origin clients, suitable for local dev. Set to a
    /// concrete list (e.g. `https://qftx.vercel.app`) for production.
    /// `*` means "any origin" — convenient for demos, never use with
    /// real auth.
    pub cors_origins: Vec<String>,
    /// When true, an in-process paper-execution engine runs in the
    /// background and writes synthetic `bot_trades` rows for every bot
    /// in `status = 'paper'`. Off by default — leave it off in tests.
    pub enable_paper_engine: bool,
    /// How often (seconds) the paper engine wakes up to generate trades.
    pub paper_tick_secs: u64,
    /// gRPC URL of the trading-server (`http://host:9001`). When set, the
    /// bot-service opens a client connection on startup and subscribes
    /// to market data. Disabled when empty — synthetic mode only.
    pub trading_grpc_url: Option<String>,
}

impl ServiceConfig {
    pub fn from_env() -> Self {
        let bind: SocketAddr = env::var("BOT_SERVICE_BIND")
            .unwrap_or_else(|_| "127.0.0.1:9100".into())
            .parse()
            .expect("BOT_SERVICE_BIND must be host:port");
        let database_url =
            env::var("BOT_SERVICE_DB").unwrap_or_else(|_| "sqlite://bot-service.sqlite".into());
        let seed_keys_path = env::var("BOT_SERVICE_KEYS").ok().filter(|s| !s.is_empty());
        let seed_demo = env::var("BOT_SERVICE_SEED_DEMO")
            .map(|s| s == "1" || s.eq_ignore_ascii_case("true"))
            .unwrap_or(false);
        let cors_origins = env::var("BOT_SERVICE_CORS_ORIGINS")
            .ok()
            .filter(|s| !s.trim().is_empty())
            .map(|s| {
                s.split(',')
                    .map(|p| p.trim().to_string())
                    .filter(|p| !p.is_empty())
                    .collect()
            })
            .unwrap_or_default();
        let enable_paper_engine = env::var("BOT_SERVICE_ENABLE_PAPER_ENGINE")
            .map(|s| s == "1" || s.eq_ignore_ascii_case("true"))
            .unwrap_or(false);
        let paper_tick_secs = env::var("BOT_SERVICE_PAPER_TICK_SECS")
            .ok()
            .and_then(|s| s.parse().ok())
            .filter(|n: &u64| *n > 0)
            .unwrap_or(30);
        let trading_grpc_url = env::var("BOT_SERVICE_TRADING_GRPC_URL")
            .ok()
            .filter(|s| !s.trim().is_empty());
        Self {
            bind,
            database_url,
            seed_keys_path,
            seed_demo,
            cors_origins,
            enable_paper_engine,
            paper_tick_secs,
            trading_grpc_url,
        }
    }
}

/// JSON file format for `BOT_SERVICE_KEYS`:
///
/// ```json
/// [ { "name": "alice", "api_key": "secret-1" }, ... ]
/// ```
#[derive(Debug, Deserialize)]
pub struct SeedKey {
    pub name: String,
    pub api_key: String,
}
