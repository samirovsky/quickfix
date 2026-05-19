//! gRPC client to the QFTX `TradingService`.
//!
//! Subscribes to `MarketData` and feeds observed prices into a shared
//! `PriceCache`. The paper engine reads that cache to use real prices
//! when available, falling back to its synthetic walk otherwise.
//!
//! Disabled unless `BOT_SERVICE_TRADING_GRPC_URL` is set. When it is,
//! `run` is spawned on startup; failures retry with a fixed 5 second
//! backoff so a temporarily-down trading-server doesn't make
//! bot-service crash.

use std::time::Duration;

use tokio::time::sleep;

use crate::pb::trading_service_client::TradingServiceClient;
use crate::pb::{market_data_update::Event, SubscribeMarketDataRequest};
use crate::price_cache::PriceCache;

const RECONNECT_DELAY: Duration = Duration::from_secs(5);

/// Run the gRPC client loop until the process exits. Designed to be
/// `tokio::spawn`ed from `main.rs`. Loops on connection failures so the
/// rest of bot-service stays up even when the trading-server is down.
pub async fn run(url: String, cache: PriceCache) {
    loop {
        match connect_and_subscribe(&url, &cache).await {
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
async fn connect_and_subscribe(url: &str, cache: &PriceCache) -> anyhow::Result<()> {
    tracing::info!(url, "connecting to trading-server gRPC");
    let mut client = TradingServiceClient::connect(url.to_string()).await?;
    tracing::info!("trading-server connected; subscribing to market data");

    // Empty `symbol_ids` = subscribe to every symbol. We filter per-bot
    // at evaluation time.
    let mut stream = client
        .subscribe_market_data(SubscribeMarketDataRequest { symbol_ids: vec![] })
        .await?
        .into_inner();

    while let Some(update) = stream.message().await? {
        if let Some(event) = update.event {
            ingest(&event, cache);
        }
    }
    Ok(())
}

fn ingest(event: &Event, cache: &PriceCache) {
    match event {
        Event::Trade(t) => cache.update(t.symbol_id, t.price),
        Event::Quote(q) => {
            // Use the mid-price for the cache; bots that care about
            // bid/ask separately can read from a richer surface later.
            let mid = (q.bid_price + q.ask_price) / 2;
            cache.update(q.symbol_id, mid);
        }
        Event::Snapshot(_) => {
            // No price field on the snapshot event; ignore for the cache.
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pb::{QuoteEvent, TradeEvent};

    /// We can't reach a real trading-server from the test environment.
    /// What we can verify is that `connect_and_subscribe` returns an
    /// error (rather than panicking) when no server is listening — that's
    /// the contract `run()` relies on for its retry loop.
    #[tokio::test]
    async fn connect_failure_returns_error_not_panic() {
        // 127.0.0.1:1 is reserved and reliably refuses connections.
        let cache = PriceCache::new();
        let res = connect_and_subscribe("http://127.0.0.1:1", &cache).await;
        assert!(res.is_err(), "expected connection error, got {res:?}");
    }

    #[test]
    fn ingest_trade_updates_cache() {
        let cache = PriceCache::new();
        ingest(
            &Event::Trade(TradeEvent {
                symbol_id: 3,
                price: 99_000,
                qty: 1,
                ts_ns: 0,
            }),
            &cache,
        );
        let obs = cache.get(3, Duration::from_secs(1)).expect("fresh");
        assert_eq!(obs.price_ticks, 99_000);
    }

    #[test]
    fn ingest_quote_uses_mid() {
        let cache = PriceCache::new();
        ingest(
            &Event::Quote(QuoteEvent {
                symbol_id: 5,
                bid_price: 100,
                bid_qty: 1,
                ask_price: 200,
                ask_qty: 1,
                ts_ns: 0,
            }),
            &cache,
        );
        let obs = cache.get(5, Duration::from_secs(1)).expect("fresh");
        assert_eq!(obs.price_ticks, 150);
    }
}
