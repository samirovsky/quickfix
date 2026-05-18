# QFTX Binary Protocol

Canonical wire-format reference. Used **byte-identically** by:

* the TCP transport at `TRADING_BIND` (one continuous stream of frames), and
* the WebSocket transport at `TRADING_WS_BIND` (one frame per WS binary message).

The gRPC transport encodes the same logical data with protobuf instead — see [`grpc.md`](./grpc.md).

## Conventions

* All multi-byte integers are **little-endian**.
* Prices are signed fixed-point `i64` with **10⁸ scale**: `100.00` is encoded as `10_000_000_000`; `0.00012345` as `12_345`.
* Field offsets given below assume `#[repr(C)]` natural alignment — struct layouts are designed to have **zero internal padding** so they can be parsed with a single `memcpy`.

## Frame structure

```
+--------------------- 32-byte HEADER ---------------------+--------- BODY ---------+
| seq:u64 | ts_ns:u64 | magic:u32 | length:u32 | version:u8 | msg_type:u8 | _:6 |  ... body ...
+----------------------------------------------------------+------------------------+
```

| Header field   | Offset | Size | Notes                                         |
| -------------- | ------ | ---- | --------------------------------------------- |
| `seq`          | 0      | 8    | Sender-monotonic sequence number              |
| `timestamp_ns` | 8      | 8    | Sender wall-clock timestamp (UNIX nanoseconds)|
| `magic`        | 16     | 4    | Constant `0x51465458` (ASCII `"QFTX"`)        |
| `length`       | 20     | 4    | Body length in bytes                          |
| `version`      | 24     | 1    | Protocol version (currently `1`)              |
| `msg_type`     | 25     | 1    | See message-type table                        |
| `_pad`         | 26     | 6    | Reserved, must be zero on send                |

Frames with a bad magic, unsupported version, or unknown `msg_type` cause the server to close the connection.

## Message types

| Code | Direction       | Name              | Body bytes |
| ---- | --------------- | ----------------- | ---------- |
| `1`  | client → server | `NewOrder`        | 48         |
| `2`  | client → server | `CancelOrder`     | 16         |
| `3`  | server → client | `ExecutionReport` | 48         |
| `4`  | bidirectional   | `Heartbeat`       | 0          |

## NewOrder body (48 bytes)

| Field      | Offset | Size | Notes                                                     |
| ---------- | ------ | ---- | --------------------------------------------------------- |
| `price`    | 0      | 8    | `i64`, fixed-point ×10⁸                                   |
| `qty`      | 8      | 8    | `u64`, whole units                                        |
| `client_id`| 16     | 8    | Opaque, echoed back in fills for client-side correlation  |
| `order_id` | 24     | 8    | Globally unique per session; the engine indexes by this   |
| `symbol_id`| 32     | 4    | Dense `u32` index into the server's instrument table      |
| `side`     | 36     | 1    | `0` = Buy, `1` = Sell                                     |
| `ord_type` | 37     | 1    | `0` = Limit, `1` = Market                                 |
| `tif`      | 38     | 1    | `0` = Day, `1` = IOC, `2` = FOK                            |
| `_pad`     | 39     | 1    | Reserved                                                  |
| `_pad2`    | 40     | 4    | Reserved                                                  |
| `_pad3`    | 44     | 4    | Reserved                                                  |

## CancelOrder body (16 bytes)

| Field      | Offset | Size | Notes                                |
| ---------- | ------ | ---- | ------------------------------------ |
| `order_id` | 0      | 8    | Must match a resting order's id      |
| `symbol_id`| 8      | 4    | Must match the order's symbol        |
| `_pad`     | 12     | 4    | Reserved                             |

## ExecutionReport body (48 bytes)

| Field        | Offset | Size | Notes                                              |
| ------------ | ------ | ---- | -------------------------------------------------- |
| `order_id`   | 0      | 8    | The order this report concerns                     |
| `exec_id`    | 8      | 8    | Server-monotonic execution id                      |
| `last_price` | 16     | 8    | Price of this fill (`0` if no fill)                |
| `last_qty`   | 24     | 8    | Quantity of this fill (`0` if no fill)             |
| `leaves_qty` | 32     | 8    | Quantity still resting (`0` if terminal)           |
| `symbol_id`  | 40     | 4    |                                                    |
| `status`     | 44     | 1    | See status table below                             |
| `side`       | 45     | 1    | `0` = Buy, `1` = Sell                              |
| `_pad`       | 46     | 2    | Reserved                                           |

### ExecStatus values

| Code | Name                | Meaning                                                  |
| ---- | ------------------- | -------------------------------------------------------- |
| `0`  | `New`               | Order accepted, resting on the book                      |
| `1`  | `PartiallyFilled`   | Intermediate fill report; `leaves_qty > 0` if not done   |
| `2`  | `Filled`            | Terminal — order fully filled                            |
| `3`  | `Cancelled`         | Terminal — order successfully cancelled                  |
| `4`  | `Rejected`          | Terminal — validation failure or no liquidity for FOK    |

### Report cadence

* **Resting / no-fill / rejected** order → **one** report (status `New` / `Rejected`).
* **Filled with N fills** → **N** reports. The Nth report carries the terminal status (`Filled`, `PartiallyFilled+leaves_qty=0`, etc.); intermediate reports use `PartiallyFilled`.
* **Cancel** → **one** report (`Cancelled` if a resting order was removed, `Rejected` if the id wasn't found).

## Framing rules

| Transport | Rule                                                                                                       |
| --------- | ---------------------------------------------------------------------------------------------------------- |
| TCP       | Frames are concatenated on the wire. Parsers should buffer and call `parse()` until `ShortBuffer`.        |
| WebSocket | Exactly one frame per WS **binary** message. WS **text** messages cause the connection to close.          |

## Reference Rust definitions

The authoritative definitions live in [`../../src/protocol.rs`](../../src/protocol.rs). All fixed-size types derive `zerocopy::FromBytes`/`AsBytes`, so encoding and decoding are single memcpys.
