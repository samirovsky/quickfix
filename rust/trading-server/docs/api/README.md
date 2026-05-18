# Trading Server API Reference

The trading server speaks three wire transports against the same underlying matching engine — every order, regardless of how it arrives, produces byte-identical execution reports because they all funnel through `engine::Engine::process_new_order`.

## Which transport should I use?

| Transport | Best for | Endpoint (default) | Reference |
| --------- | -------- | ------------------ | --------- |
| **Custom binary over TCP** | Co-located low-latency clients in C/C++/Rust/Go. Lowest overhead. | `127.0.0.1:9000` | [`binary-protocol.md`](./binary-protocol.md) |
| **gRPC** | Native desktop / server-side clients (Python, Java, Go, Node, .NET, Tauri/Electron) that want generated stubs and unary or bidirectional streaming RPCs. | `127.0.0.1:9001` | [`grpc.md`](./grpc.md) |
| **WebSocket** | Browsers and anything else that can't speak HTTP/2 trailers. Carries the same binary frames as the TCP transport, framed by WS messages. | `ws://127.0.0.1:9002/ws` | [`websocket.md`](./websocket.md) |

All three transports share the same wire data structures (same field layouts, same fixed-point price scale). The TCP and WebSocket transports are byte-identical at the frame level; gRPC encodes the same fields via protobuf.

## Running the server

```bash
cd rust/trading-server
cargo run --release
# Listening on:
#   tcp://127.0.0.1:9000     (raw QFTX binary)
#   grpc://127.0.0.1:9001    (TradingService)
#   ws://127.0.0.1:9002/ws   (QFTX binary inside WS frames)
```

Override any endpoint with environment variables (set to empty string to disable a transport):

| Variable             | Default          | Purpose                |
| -------------------- | ---------------- | ---------------------- |
| `TRADING_BIND`       | `127.0.0.1:9000` | TCP listener           |
| `TRADING_GRPC_BIND`  | `127.0.0.1:9001` | gRPC listener          |
| `TRADING_WS_BIND`    | `127.0.0.1:9002` | WebSocket listener     |
| `TRADING_SYMBOLS`    | `64`             | number of order books  |
| `TRADING_WAL`        | `trading-server.wal` | WAL file path      |

## Examples

The [`examples/`](./examples/) directory holds minimal runnable clients for each transport:

* [`tcp-client.py`](./examples/tcp-client.py)  — raw socket + `struct.pack`.
* [`grpc-client.py`](./examples/grpc-client.py) — `grpcio` with generated stubs.
* [`ws-client.html`](./examples/ws-client.html) — browser page placing an order via WebSocket.

## Versioning

The QFTX binary frames carry a `version` byte (`1` today); future incompatible frame changes will bump it. The `.proto` file declares package `qftx.trading.v1`; future service revisions will go in `v2`. Both versions can coexist at runtime if needed.
