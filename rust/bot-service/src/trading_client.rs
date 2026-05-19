//! gRPC client to the QFTX `TradingService`.
//!
//! Today this module just connects, subscribes to `MarketData`, and
//! logs ticks. The paper engine doesn't yet evaluate strategy
//! conditions against live data — that's the next slice. Wiring is in
//! place so the next slice is a local change to `paper_engine.rs`.
//!
//! Disabled unless `BOT_SERVICE_TRADING_GRPC_URL` is set. When it is,
//! `connect_and_run` is spawned on startup; failures retry with a fixed
//! 5 second backoff so a temporarily-down trading-server doesn't make
//! bot-service crash.

use std::time::Duration;

use tokio::time::sleep;

use crate::pb::trading_service_client::TradingServiceClient;
use crate::pb::SubscribeMarketDataRequest;

const RECONNECT_DELAY: Duration = Duration::from_secs(5);

/// Run the gRPC client loop until the process exits. Designed to be
/// `tokio::spawn`ed from `main.rs`. Loops on connection failures so the
/// rest of bot-service stays up even when the trading-server is down.
pub async fn run(url: String) {
    loop {
        match connect_and_subscribe(&url).await {
            Ok(()) => {
                tracing::warn!("trading-server stream ended; reconnecting");
            }
            Err(e) => {
                tracing::warn!(error = %e, "trading-server connection failed");
            }
        }
        sleep(RECONNECT_DELAY).await;
    }
}

/// One attempt at connecting + draining the market-data stream. Returns
/// when the stream ends or the connection errors out.
async fn connect_and_subscribe(url: &str) -> anyhow::Result<()> {
    tracing::info!(url, "connecting to trading-server gRPC");
    let mut client = TradingServiceClient::connect(url.to_string()).await?;
    tracing::info!("trading-server connected; subscribing to market data");

    // Empty `symbol_ids` = subscribe to every symbol. Bots will filter
    // on the consumer side.
    let mut stream = client
        .subscribe_market_data(SubscribeMarketDataRequest { symbol_ids: vec![] })
        .await?
        .into_inner();

    while let Some(item) = stream.message().await? {
        // Right now we only log — once the engine consumes this stream,
        // the next slice forwards events to a `tokio::sync::broadcast`
        // that the paper-engine subscribes to per bot.
        tracing::debug!(?item, "market data tick");
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// We can't reach a real trading-server from the test environment.
    /// What we can verify is that `connect_and_subscribe` returns an
    /// error (rather than panicking) when no server is listening — that's
    /// the contract `run()` relies on for its retry loop.
    #[tokio::test]
    async fn connect_failure_returns_error_not_panic() {
        // 127.0.0.1:1 is reserved and reliably refuses connections.
        let res = connect_and_subscribe("http://127.0.0.1:1").await;
        assert!(res.is_err(), "expected connection error, got {res:?}");
    }
}
