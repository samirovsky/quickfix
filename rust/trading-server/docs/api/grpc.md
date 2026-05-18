# gRPC API

The trading server exposes a gRPC `TradingService` on `TRADING_GRPC_BIND` (default `127.0.0.1:9001`). The protocol buffer schema is the source of truth: [`../../proto/trading.proto`](../../proto/trading.proto).

## Service overview

```proto
service TradingService {
  rpc OrderSession        (stream ClientMessage) returns (stream ServerMessage);
  rpc PlaceOrder          (PlaceOrderRequest)    returns (PlaceOrderResponse);
  rpc CancelOrder         (CancelOrderRequest)   returns (CancelOrderResponse);
  rpc SubscribeMarketData (SubscribeMarketDataRequest) returns (stream MarketDataUpdate);
}
```

| RPC                    | Shape              | Use when                                                            |
| ---------------------- | ------------------ | ------------------------------------------------------------------- |
| `OrderSession`         | bidi stream        | Persistent native client; lowest overhead per order after handshake |
| `PlaceOrder`           | unary              | Scripts and one-off orders; waits for the terminal report           |
| `CancelOrder`          | unary              | Scripts; waits for the cancel ack                                   |
| `SubscribeMarketData`  | server stream      | Market-data subscriber; filter by symbol or take everything         |

## Generating client stubs

The `.proto` file is plain proto3 with no exotic features. Use the standard generators for your language:

### Python

```bash
pip install grpcio grpcio-tools
python -m grpc_tools.protoc \
    -I rust/trading-server/proto \
    --python_out=. --grpc_python_out=. \
    rust/trading-server/proto/trading.proto
```

Example usage: [`examples/grpc-client.py`](./examples/grpc-client.py).

### TypeScript / Node

```bash
npm install @grpc/grpc-js @grpc/proto-loader
# Or generate types with ts-proto:
npm install --save-dev ts-proto
protoc --plugin=protoc-gen-ts_proto=./node_modules/.bin/protoc-gen-ts_proto \
       --ts_proto_out=. --ts_proto_opt=outputServices=grpc-js \
       -I rust/trading-server/proto \
       rust/trading-server/proto/trading.proto
```

### Go

```bash
protoc --go_out=. --go-grpc_out=. \
       -I rust/trading-server/proto \
       rust/trading-server/proto/trading.proto
```

### Rust

Use `tonic-build` from a `build.rs`:

```rust
tonic_build::compile_protos("../trading-server/proto/trading.proto").unwrap();
```

## Choosing OrderSession vs PlaceOrder

* `OrderSession` is one bidi stream — open it once, send any number of `NewOrder` / `CancelOrder` messages, receive every `ExecutionReport` for orders submitted on that session. Lowest overhead per order; mirrors the TCP transport.
* `PlaceOrder` opens a new internal connection per call, waits for the order's **terminal** report, then closes. Single-fill orders return one report; multi-fill orders return all `N` of them. Has a hard 5-second deadline server-side.

## Prices and units

Prices on the wire are signed fixed-point at **10⁸ scale** — same as the binary transport. `100.00` becomes `10_000_000_000`. We use `sint64` in proto so varint encoding stays compact for both positive prices and negative spreads.

## Error semantics

* Bad enum value (e.g. `side=0=UNSPECIFIED`) ⇒ `INVALID_ARGUMENT`.
* Engine inbox saturated for more than ~10 ms ⇒ `RESOURCE_EXHAUSTED`.
* `PlaceOrder`/`CancelOrder` timeout (5 s) ⇒ `DEADLINE_EXCEEDED`.
* Server shutting down ⇒ `UNAVAILABLE`.

`OrderSession` does **not** surface per-order errors as gRPC `Status`es — it returns `ServerMessage`s. A `Rejected` `ExecutionReport` indicates engine-level rejection.

## Worked example (Python)

```python
import grpc, trading_pb2 as pb, trading_pb2_grpc as svc

with grpc.insecure_channel("127.0.0.1:9001") as channel:
    client = svc.TradingServiceStub(channel)
    resp = client.PlaceOrder(pb.PlaceOrderRequest(order=pb.NewOrder(
        order_id=1, symbol_id=0, side=pb.BUY,
        ord_type=pb.LIMIT, tif=pb.DAY,
        price=100_00 * 1_000_000, qty=5, client_id=1,
    )))
    for r in resp.reports:
        print(r)
```

See [`examples/grpc-client.py`](./examples/grpc-client.py) for a runnable version.
