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
        Self {
            bind,
            database_url,
            seed_keys_path,
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
