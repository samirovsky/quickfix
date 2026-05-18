//! Binary entry point.
//!
//! Wires together: the engine (one OS thread), the WAL writer (one OS
//! thread), the exec dispatcher (one OS thread), and the TCP accept loop
//! (the main thread).

use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::Arc;

use anyhow::Context;
use crossbeam_channel::bounded;
use tracing_subscriber::EnvFilter;

use trading_server::engine::{Engine, EngineMsg, OutboundExec};
use trading_server::server::{ConnRegistry, Server};
use trading_server::wal::{self, WalRecord};

#[global_allocator]
static GLOBAL: mimalloc::MiMalloc = mimalloc::MiMalloc;

const ENGINE_QUEUE: usize = 65_536;
const EXEC_QUEUE: usize = 65_536;
const WAL_QUEUE: usize = 1_048_576;

fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")),
        )
        .init();

    let args = Args::from_env();
    tracing::info!(?args, "starting trading server");

    let (engine_tx, engine_rx) = bounded::<EngineMsg>(ENGINE_QUEUE);
    let (exec_tx, exec_rx) = bounded::<OutboundExec>(EXEC_QUEUE);
    let (wal_tx, wal_rx) = bounded::<WalRecord>(WAL_QUEUE);

    let registry = Arc::new(ConnRegistry::default());

    let wal_handle = wal::spawn(args.wal_path.clone(), wal_rx);

    let engine_handle = std::thread::Builder::new()
        .name("engine".into())
        .spawn({
            let exec_tx = exec_tx.clone();
            let wal_tx = wal_tx.clone();
            let symbols = args.symbols;
            move || {
                if let Some(core_id) =
                    core_affinity::get_core_ids().and_then(|v| v.into_iter().next())
                {
                    core_affinity::set_for_current(core_id);
                }
                let mut engine = Engine::new(symbols);
                engine.run(engine_rx, exec_tx, wal_tx);
            }
        })
        .context("spawn engine thread")?;

    let dispatcher_handle =
        trading_server::server::spawn_exec_dispatcher(exec_rx, Arc::clone(&registry));

    let server = Server::bind(args.bind, engine_tx.clone(), Arc::clone(&registry))?;
    server.run()?;

    // Tear down in reverse dependency order.
    drop(engine_tx);
    let _ = engine_handle.join();
    drop(exec_tx);
    let _ = dispatcher_handle.join();
    drop(wal_tx);
    if let Ok(Err(e)) = wal_handle.join() {
        tracing::warn!(error = %e, "wal writer ended with error");
    }
    Ok(())
}

#[derive(Debug)]
struct Args {
    bind: SocketAddr,
    symbols: u32,
    wal_path: PathBuf,
}

impl Args {
    fn from_env() -> Self {
        let bind = std::env::var("TRADING_BIND")
            .unwrap_or_else(|_| "127.0.0.1:9000".to_string())
            .parse()
            .expect("TRADING_BIND must be a valid socket address");
        let symbols = std::env::var("TRADING_SYMBOLS")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(64);
        let wal_path = std::env::var("TRADING_WAL")
            .map(PathBuf::from)
            .unwrap_or_else(|_| PathBuf::from("trading-server.wal"));
        Self {
            bind,
            symbols,
            wal_path,
        }
    }
}
