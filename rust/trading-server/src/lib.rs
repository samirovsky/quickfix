//! Low-latency trading server.
//!
//! The crate is organised so that the order-matching hot path (`engine` +
//! `orderbook`) is free of allocations, locks, and syscalls. I/O (`server`),
//! durability (`wal`), and market-data ingestion (`marketdata`) all sit
//! around the engine and communicate with it through bounded channels.

#![deny(rust_2018_idioms)]
#![warn(missing_debug_implementations)]

pub mod engine;
pub mod error;
pub mod marketdata;
pub mod metrics;
pub mod orderbook;
pub mod protocol;
pub mod server;
pub mod wal;

pub use error::Error;

/// Fixed-point scale applied to all price values on the wire and in the book.
/// `1.00` is encoded as `100_000_000`.
pub const PRICE_SCALE: i64 = 100_000_000;

/// Monotonic price tick. Negative values are legal (spreads, P&L deltas) but
/// resting-order prices must be `> 0`.
pub type Price = i64;

/// Order quantity in whole units.
pub type Qty = u64;

/// Globally unique order identifier assigned by the client.
pub type OrderId = u64;

/// Identifier for a tradable instrument. Engine maps `SymbolId -> OrderBook`
/// via a plain `Vec`; ids are dense and assigned at startup.
pub type SymbolId = u32;

/// Opaque per-connection identifier used to route execution reports back to
/// the client that submitted the order.
pub type ConnId = u64;
