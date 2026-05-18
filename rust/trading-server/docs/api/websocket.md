# WebSocket API

The WebSocket transport at `TRADING_WS_BIND` (default `ws://127.0.0.1:9002/ws`) carries **the same QFTX binary frames** as the TCP transport, packaged one frame per WebSocket binary message. Browser clients can parse and produce them with `DataView` and `Uint8Array`.

## Handshake

* Endpoint: `ws://<host>:<port>/ws`
* Subprotocol: `qftx-binary.v1` — recommended. The server advertises it back when a client offers it. Clients that omit a subprotocol still connect (for browser-demo convenience).
* No `Origin` check — see _security_ at the bottom.

Example handshake from a browser:

```js
const ws = new WebSocket("ws://127.0.0.1:9002/ws", "qftx-binary.v1");
ws.binaryType = "arraybuffer";
```

## Framing

* **Inbound** (client → server) — each WS binary message **must** contain exactly **one** QFTX frame: 32-byte header + body. The server closes the connection with code `1003` (`Unsupported`) on text messages, and `1007` / `1002` on malformed binary frames.
* **Outbound** (server → client) — same rule: each WS binary message contains one frame (always a 32-byte header + 48-byte `ExecutionReport` body in the current build).
* WS ping/pong: handled automatically by the WS layer. For application-level keepalive use the QFTX `Heartbeat` frame (msg_type `4`).

## Close codes

| Code  | Reason                                            |
| ----- | ------------------------------------------------- |
| 1000  | Normal closure                                    |
| 1002  | Malformed QFTX frame                              |
| 1003  | Text frame received (unsupported)                 |
| 1008  | Engine unavailable                                |
| 1011  | Server internal error                             |

## Browser example

```html
<script>
const enc = new TextEncoder();
const ws  = new WebSocket("ws://127.0.0.1:9002/ws", "qftx-binary.v1");
ws.binaryType = "arraybuffer";

const HEADER = 32, NEW_ORDER = 48;
const MAGIC  = 0x51465458;   // "QFTX" LE
const PRICE_SCALE = 100_000_000n;

ws.onopen = () => {
  const buf = new ArrayBuffer(HEADER + NEW_ORDER);
  const dv  = new DataView(buf);

  // Header
  dv.setBigUint64(0,  1n,                true);  // seq
  dv.setBigUint64(8,  BigInt(Date.now()) * 1_000_000n, true); // ts_ns
  dv.setUint32(16, MAGIC,                true);
  dv.setUint32(20, NEW_ORDER,            true);  // body length
  dv.setUint8(24,  1);                            // version
  dv.setUint8(25,  1);                            // msg_type = NewOrder

  // NewOrder body
  dv.setBigInt64(32 + 0,  100n * PRICE_SCALE, true); // price
  dv.setBigUint64(32 + 8,  5n,                true); // qty
  dv.setBigUint64(32 + 16, 1n,                true); // client_id
  dv.setBigUint64(32 + 24, 1n,                true); // order_id
  dv.setUint32(32 + 32,    0,                 true); // symbol_id
  dv.setUint8 (32 + 36,    0); // side = Buy
  dv.setUint8 (32 + 37,    0); // ord_type = Limit
  dv.setUint8 (32 + 38,    0); // tif = Day

  ws.send(buf);
};

ws.onmessage = ev => {
  const dv = new DataView(ev.data);
  console.log("exec report status=", dv.getUint8(32 + 44));
};
</script>
```

A complete copy of the above (and a tiny UI) lives in [`examples/ws-client.html`](./examples/ws-client.html).

## Security note

The server has **no authentication** today (an explicit choice for the v1 release). Run it behind a trusted network boundary; do not expose it to the open internet. TLS, auth, and `Origin` validation are planned follow-ups.
