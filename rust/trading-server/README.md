# trading-server

Low-latency order-matching trading server, written in Rust.

* Custom binary wire protocol (more compact and cheaper to parse than FIX).
* Pluggable market-data layer — any provider (file replay, multicast, ITCH, websocket) can implement `MarketDataProvider` and feed the engine.
* Single-threaded matching core; **`process_new_order` p99 < 1 ms**, enforced by `tests/latency_gate.rs`. Measured on a generic dev container at p50 ≈ 100 ns, p99 ≈ 310 ns, p99.9 ≈ 550 ns over 100k iterations against a populated book (10k resting orders, 100 levels per side).
* Asynchronous write-ahead log — the hot path never blocks on disk; a dedicated writer thread batches and `flush()`es records.

## Separation from the C++ QuickFIX engine

This crate lives under **`rust/`** at the repository root, fully isolated from the surrounding QuickFIX C++ tree (`src/`, `lib/`, `examples/`, `spec/`). There is no FFI between them and the C++ autotools build does not see the Rust crate — `make` is unaffected.

## Performance notes

Optimisations on the hot path:

* `WalRecord` is a `Copy` enum of POD bodies — the engine `try_send`s it across a `crossbeam_channel` with zero heap allocation. Serialisation to bytes happens on the WAL writer thread, off the hot path.
* Maker-side execution reports are not emitted. This server has no client-id ↔ connection map, so they would route to `conn_id = 0` and be dropped by the dispatcher anyway. Skipping the construction + `try_send` saves one CAS per fill.
* `process_new_order`, `OrderBook::submit`, `match_against`, `rest`, and the protocol enum conversions are all `#[inline]`-hinted so cross-crate calls (benchmark, integration test, server) inline cleanly.
* Wire structs are laid out largest-field-first so `#[repr(C)]` produces zero internal padding — `zerocopy::AsBytes` parses/encodes via a single memcpy with no per-field work.
* `mimalloc` is the global allocator, set in `main.rs`.
* The engine thread is pinned to a single core via `core_affinity` so it doesn't migrate.
* Release profile: `lto = "fat"`, `codegen-units = 1`, `panic = "abort"`, `overflow-checks = false`.

For an additional ~10–30 ns at p99, pass `target-cpu=native` to rustc when building for the production host: `RUSTFLAGS="-C target-cpu=native" cargo build --release`. This is left as an opt-in because a stale `.cargo/config.toml` set globally can ICE rustc when proc-macros get loaded.

## Layout

```
rust/trading-server/
├── Cargo.toml
├── src/
│   ├── lib.rs                  re-exports + core type aliases
│   ├── main.rs                 binary entry point, wiring
│   ├── protocol.rs             binary wire format (zero-copy parse via zerocopy)
│   ├── orderbook.rs            single-symbol limit order book + matching
│   ├── engine.rs               owns Vec<OrderBook>, hot path: process_new_order
│   ├── server.rs               std::net accept + reader/writer threads, exec dispatcher
│   ├── wal.rs                  bounded channel + dedicated writer thread (Copy enum, no alloc)
│   ├── marketdata.rs           MarketDataProvider trait + FileReplayProvider
│   ├── metrics.rs              hdrhistogram + counters
│   └── error.rs                thiserror enum
├── benches/order_latency.rs    criterion benchmark
└── tests/
    ├── integration.rs          end-to-end: client → engine → exec report
    └── latency_gate.rs         hard p99 < 1 ms assertion
```

## Wire protocol

All multi-byte fields are little-endian. Header is 32 bytes; bodies are fixed.

| Field            | Size | Notes                                   |
| ---------------- | ---- | --------------------------------------- |
| `magic`          | 4    | `0x51465458` = ASCII "QFTX"             |
| `version`        | 1    | currently `1`                           |
| `msg_type`       | 1    | 1=NewOrder, 2=Cancel, 3=ExecReport, 4=Heartbeat |
| `_pad`           | 2    |                                         |
| `length`         | 4    | body bytes following the header         |
| `seq`            | 8    | sender-monotonic                        |
| `timestamp_ns`   | 8    | sender wall clock                       |

`NewOrder` body (48 bytes): `price:i64 | qty:u64 | client_id:u64 | order_id:u64 | symbol_id:u32 | side:u8 | ord_type:u8 | tif:u8 | _pad:u8 | _pad2:u32 | _pad3:u32`. Prices are fixed-point `i64` at `1e8` scale (so `100.00` is encoded as `10_000_000_000`). Fields are ordered largest-first so `#[repr(C)]` yields zero internal padding (a requirement of the `zerocopy::AsBytes` derive).

`CancelOrder` body (16 bytes), `ExecutionReport` body (48 bytes) — see `src/protocol.rs`.

## Build, test, bench

```bash
cd rust/trading-server
cargo build --release
cargo test  --release
cargo test  --release --test latency_gate -- --nocapture   # hard p99 < 1 ms
cargo bench
```

## Running the server

```bash
TRADING_BIND=127.0.0.1:9000 TRADING_SYMBOLS=64 TRADING_WAL=./trading.wal \
  cargo run --release
```

## What is intentionally out of scope

Auth, TLS, multi-symbol sharding across cores, WAL **recovery** on startup, real exchange connectivity (NYSE/Nasdaq/CME), risk/position/P&L, log compaction/HA/replication, an admin/control plane, and integration with the surrounding C++ QuickFIX autotools build.
