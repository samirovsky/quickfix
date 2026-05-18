//! Lightweight runtime metrics.
//!
//! Counters use `AtomicU64` with `Relaxed` ordering. Latency uses an
//! `hdrhistogram::Histogram` protected by `parking_lot::Mutex` — the mutex
//! is contended only by the engine thread + the snapshot path, so it is
//! effectively uncontended in practice. Off the hot path callers can take a
//! `snapshot()` at any time to read percentiles.

use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

use hdrhistogram::Histogram;
use parking_lot::Mutex;

pub struct EngineMetrics {
    orders_total: AtomicU64,
    fills_total: AtomicU64,
    market_data_total: AtomicU64,
    wal_dropped_total: AtomicU64,
    latency_ns: Mutex<Histogram<u64>>,
}

impl EngineMetrics {
    pub fn new() -> Self {
        // 3 significant digits, max value 60s in ns — plenty of headroom.
        let hist = Histogram::<u64>::new_with_bounds(1, 60_000_000_000, 3).expect("hist");
        Self {
            orders_total: AtomicU64::new(0),
            fills_total: AtomicU64::new(0),
            market_data_total: AtomicU64::new(0),
            wal_dropped_total: AtomicU64::new(0),
            latency_ns: Mutex::new(hist),
        }
    }

    pub fn record_order(&self, filled: u64, fills: usize) {
        self.orders_total.fetch_add(1, Ordering::Relaxed);
        if filled > 0 {
            self.fills_total.fetch_add(fills as u64, Ordering::Relaxed);
        }
    }

    pub fn record_market_data(&self) {
        self.market_data_total.fetch_add(1, Ordering::Relaxed);
    }

    pub fn record_wal_dropped(&self) {
        self.wal_dropped_total.fetch_add(1, Ordering::Relaxed);
    }

    pub fn record_order_latency(&self, dur: Duration) {
        let ns = dur.as_nanos().min(u64::MAX as u128) as u64;
        if ns == 0 {
            return;
        }
        let mut hist = self.latency_ns.lock();
        let _ = hist.record(ns);
    }

    pub fn snapshot(&self) -> MetricsSnapshot {
        let hist = self.latency_ns.lock();
        MetricsSnapshot {
            orders_total: self.orders_total.load(Ordering::Relaxed),
            fills_total: self.fills_total.load(Ordering::Relaxed),
            market_data_total: self.market_data_total.load(Ordering::Relaxed),
            wal_dropped_total: self.wal_dropped_total.load(Ordering::Relaxed),
            p50_ns: hist.value_at_quantile(0.50),
            p99_ns: hist.value_at_quantile(0.99),
            p999_ns: hist.value_at_quantile(0.999),
            max_ns: hist.max(),
            samples: hist.len(),
        }
    }
}

impl Default for EngineMetrics {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Debug for EngineMetrics {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = self.snapshot();
        f.debug_struct("EngineMetrics")
            .field("orders_total", &s.orders_total)
            .field("fills_total", &s.fills_total)
            .field("wal_dropped_total", &s.wal_dropped_total)
            .field("p50_ns", &s.p50_ns)
            .field("p99_ns", &s.p99_ns)
            .finish()
    }
}

#[derive(Debug, Clone, Copy)]
pub struct MetricsSnapshot {
    pub orders_total: u64,
    pub fills_total: u64,
    pub market_data_total: u64,
    pub wal_dropped_total: u64,
    pub p50_ns: u64,
    pub p99_ns: u64,
    pub p999_ns: u64,
    pub max_ns: u64,
    pub samples: u64,
}
