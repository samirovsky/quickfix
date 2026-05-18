//! Pluggable market-data providers.
//!
//! A `MarketDataProvider` is anything that can push `MarketDataEvent`s into
//! a `crossbeam_channel::Sender`. The engine consumes those events alongside
//! its order channel, keeping it the sole writer to the order books.
//!
//! Ships with one concrete provider: `FileReplayProvider`, which reads a
//! binary-encoded recorded session and replays it. Real providers (a
//! WebSocket feed, a multicast UDP feed, an exchange ITCH parser, etc.)
//! implement the same trait and slot in unchanged.

use std::fs::File;
use std::io::Read;
use std::path::PathBuf;
use std::thread;
use std::time::Duration;

use crossbeam_channel::Sender;

use crate::error::Error;
use crate::{Price, Qty, SymbolId};

#[derive(Debug, Clone, Copy)]
pub enum MarketDataEvent {
    Trade {
        symbol_id: SymbolId,
        price: Price,
        qty: Qty,
        ts_ns: u64,
    },
    Quote {
        symbol_id: SymbolId,
        bid_price: Price,
        bid_qty: Qty,
        ask_price: Price,
        ask_qty: Qty,
        ts_ns: u64,
    },
    BookSnapshot {
        symbol_id: SymbolId,
        ts_ns: u64,
    },
}

impl MarketDataEvent {
    pub fn symbol_id(&self) -> SymbolId {
        match self {
            MarketDataEvent::Trade { symbol_id, .. }
            | MarketDataEvent::Quote { symbol_id, .. }
            | MarketDataEvent::BookSnapshot { symbol_id, .. } => *symbol_id,
        }
    }
}

/// Broadcast fan-out for market-data subscribers (gRPC `SubscribeMarketData`,
/// future WS subscriptions). The engine — or an external feed adapter —
/// publishes events; subscribers each hold a `tokio::sync::broadcast::Receiver`
/// and filter by `symbol_id` on their side.
///
/// `broadcast` drops the oldest events on slow subscribers, matching the
/// "availability over completeness" policy used by the WAL.
#[derive(Debug, Clone)]
pub struct MarketDataBus {
    tx: tokio::sync::broadcast::Sender<MarketDataEvent>,
}

impl MarketDataBus {
    pub fn new(capacity: usize) -> Self {
        let (tx, _) = tokio::sync::broadcast::channel(capacity);
        Self { tx }
    }

    /// Publish an event. Returns the current subscriber count (0 if no one
    /// is listening; the message is dropped silently in that case).
    #[inline]
    pub fn publish(&self, ev: MarketDataEvent) -> usize {
        self.tx.send(ev).unwrap_or(0)
    }

    pub fn subscribe(&self) -> tokio::sync::broadcast::Receiver<MarketDataEvent> {
        self.tx.subscribe()
    }
}

impl Default for MarketDataBus {
    fn default() -> Self {
        Self::new(1024)
    }
}

pub trait MarketDataProvider: Send {
    /// Name for logging.
    fn name(&self) -> &str;

    /// Start the provider. The implementation is expected to spawn its own
    /// thread (or set of threads) and push events into `sink` until
    /// `stop` is called or the source is exhausted.
    fn start(&mut self, sink: Sender<MarketDataEvent>) -> Result<(), Error>;

    /// Best-effort shutdown.
    fn stop(&mut self);
}

/// Replays a recorded binary session file.
///
/// File format (little-endian):
///
/// ```text
/// repeating: event_type:u8 | symbol_id:u32 | price:i64 | qty:u64 | ts_ns:u64
///   event_type: 0 = Trade, 1 = Quote (price/qty become bid; ask = price+1, ask_qty = qty)
/// ```
///
/// Trivial format intentionally; real providers will use a proper one.
pub struct FileReplayProvider {
    path: PathBuf,
    speed: f64,
    handle: Option<thread::JoinHandle<()>>,
}

impl FileReplayProvider {
    pub fn new(path: PathBuf, speed: f64) -> Self {
        Self {
            path,
            speed,
            handle: None,
        }
    }
}

impl std::fmt::Debug for FileReplayProvider {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("FileReplayProvider")
            .field("path", &self.path)
            .field("speed", &self.speed)
            .field("running", &self.handle.is_some())
            .finish()
    }
}

impl MarketDataProvider for FileReplayProvider {
    fn name(&self) -> &str {
        "file-replay"
    }

    fn start(&mut self, sink: Sender<MarketDataEvent>) -> Result<(), Error> {
        let mut file = File::open(&self.path)?;
        let mut buf = Vec::new();
        file.read_to_end(&mut buf)?;
        let speed = self.speed;

        let handle = thread::Builder::new()
            .name("md-file-replay".into())
            .spawn(move || {
                let mut i = 0;
                let record = 1 + 4 + 8 + 8 + 8;
                let mut last_ts: Option<u64> = None;
                while i + record <= buf.len() {
                    let ev_type = buf[i];
                    let symbol_id = u32::from_le_bytes(buf[i + 1..i + 5].try_into().unwrap());
                    let price = i64::from_le_bytes(buf[i + 5..i + 13].try_into().unwrap());
                    let qty = u64::from_le_bytes(buf[i + 13..i + 21].try_into().unwrap());
                    let ts_ns = u64::from_le_bytes(buf[i + 21..i + 29].try_into().unwrap());
                    if let Some(prev) = last_ts {
                        if speed > 0.0 && ts_ns > prev {
                            let wait_ns = ((ts_ns - prev) as f64 / speed) as u64;
                            if wait_ns > 0 {
                                thread::sleep(Duration::from_nanos(wait_ns));
                            }
                        }
                    }
                    last_ts = Some(ts_ns);
                    let event = match ev_type {
                        1 => MarketDataEvent::Quote {
                            symbol_id,
                            bid_price: price,
                            bid_qty: qty,
                            ask_price: price + 1,
                            ask_qty: qty,
                            ts_ns,
                        },
                        _ => MarketDataEvent::Trade {
                            symbol_id,
                            price,
                            qty,
                            ts_ns,
                        },
                    };
                    if sink.send(event).is_err() {
                        return;
                    }
                    i += record;
                }
            })?;
        self.handle = Some(handle);
        Ok(())
    }

    fn stop(&mut self) {
        // The thread exits when the file is exhausted or the sink is dropped.
        // For a richer impl we'd flip an AtomicBool; not needed for the demo.
        if let Some(h) = self.handle.take() {
            let _ = h.join();
        }
    }
}
