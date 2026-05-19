//! Last-known-price cache for paper-engine evaluation.
//!
//! The gRPC market-data consumer in `trading_client` writes into this
//! cache; the paper engine reads from it. Sharing through an Arc<RwLock>
//! is fine for the v1 cadence (a few writes per second, a few reads per
//! tick); a lock-free DashMap would be the next step if this becomes
//! contended.

use std::collections::HashMap;
use std::sync::{Arc, RwLock};
use std::time::{Duration, Instant};

/// Most-recently-observed price for a symbol.
#[derive(Debug, Clone, Copy)]
pub struct PriceObservation {
    /// Fixed-point i64, same scale as the matching engine (1 USD = 1e8 ticks).
    pub price_ticks: i64,
    pub observed_at: Instant,
}

#[derive(Debug, Default)]
struct Inner {
    by_symbol: HashMap<u32, PriceObservation>,
}

#[derive(Debug, Clone, Default)]
pub struct PriceCache {
    inner: Arc<RwLock<Inner>>,
}

impl PriceCache {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn update(&self, symbol_id: u32, price_ticks: i64) {
        let mut g = self.inner.write().expect("price cache RwLock poisoned");
        g.by_symbol.insert(
            symbol_id,
            PriceObservation {
                price_ticks,
                observed_at: Instant::now(),
            },
        );
    }

    /// Returns the most recent price for `symbol_id` if we've seen one
    /// in the last `max_age`. `None` means "no fresh data, fall back".
    pub fn get(&self, symbol_id: u32, max_age: Duration) -> Option<PriceObservation> {
        let g = self.inner.read().expect("price cache RwLock poisoned");
        let obs = *g.by_symbol.get(&symbol_id)?;
        if obs.observed_at.elapsed() <= max_age {
            Some(obs)
        } else {
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trip_in_window() {
        let c = PriceCache::new();
        c.update(7, 12_345);
        let obs = c.get(7, Duration::from_secs(10)).expect("fresh");
        assert_eq!(obs.price_ticks, 12_345);
    }

    #[test]
    fn returns_none_when_stale() {
        let c = PriceCache::new();
        c.update(7, 12_345);
        // Tiny window guarantees the observation is "old".
        std::thread::sleep(Duration::from_millis(5));
        assert!(c.get(7, Duration::from_nanos(1)).is_none());
    }

    #[test]
    fn returns_none_for_unknown_symbol() {
        let c = PriceCache::new();
        assert!(c.get(7, Duration::from_secs(10)).is_none());
    }
}
