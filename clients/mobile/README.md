# QFTX Trader — mobile client

Cross-platform mobile trading UI built with **Expo + React Native + TypeScript**. Talks to the QFTX trading server over the WebSocket transport (binary QFTX frames).

Runs on iOS, Android, and the web from a single codebase.

## What's in it

| Screen     | Purpose                                                                                  |
| ---------- | ---------------------------------------------------------------------------------------- |
| Connect    | Enter the server WebSocket URL; settings are persisted to AsyncStorage; live status badge|
| Trade      | Symbol picker · order book (own resting) · order entry · depth chart · live exec feed   |
| Orders     | All open orders across symbols with per-order Cancel + Cancel-All                       |
| Fills      | Realised P&amp;L per symbol, total realised, full fills history                          |
| Settings   | Theme (system / light / dark) · session reset · settings reset                          |

## Architecture

```
App.tsx                              navigation root, bottom tabs, theme + hydration boot
src/
├── protocol/
│   ├── types.ts                     mirrors rust/trading-server/src/protocol.rs enums + sizes
│   └── qftx.ts                      binary frame encode (NewOrder/Cancel) + decode (ExecReport)
├── transport/
│   └── client.ts                    WebSocket wrapper: auto-reconnect, outbox queue, subprotocol
├── state/
│   ├── settings.ts                  Zustand store, persisted via @react-native-async-storage
│   └── trading.ts                   connection · open orders · fills · positions · live feed
├── theme/
│   ├── colors.ts                    light + dark palettes
│   └── ThemeProvider.tsx            system / explicit theme toggle
├── components/                      Card · SegmentedControl · OrderBook · OrderEntry ·
│                                    DepthChart · OpenOrdersList · ExecutionsList · ...
└── screens/                         Connect · Trading · Orders · Fills · Settings
```

The app never relies on the global allocator's order book — it shows the **client's own** resting orders. Market-wide depth requires the gRPC `SubscribeMarketData` stream, which is planned as a follow-up.

## Setup

```bash
cd clients/mobile
npm install
npm run typecheck            # tsc --noEmit
npm start                    # launches Metro + QR code for Expo Go
```

Open the app on:

* **iOS Simulator** — `npm run ios`
* **Android Emulator** — `npm run android` (use `ws://10.0.2.2:9002/ws` to reach the dev host)
* **Web** — `npm run web` (use `ws://localhost:9002/ws`)
* **Physical phone** — install Expo Go, scan the QR code; use `ws://<dev-machine-LAN-IP>:9002/ws`

## Connecting to the trading server

Start the server with the WebSocket listener enabled (it is by default):

```bash
cd rust/trading-server
cargo run --release
# WebSocket listening on 127.0.0.1:9002
```

In the **Connect** tab, set the WebSocket URL to the host the phone can reach the dev machine on, then tap CONNECT. The badge in the top-right of every screen turns LIVE once the WS handshake completes.

| Where the app runs                 | Server URL to enter                  |
| ---------------------------------- | ------------------------------------ |
| iOS Simulator (same Mac)           | `ws://127.0.0.1:9002/ws`             |
| Android Emulator                   | `ws://10.0.2.2:9002/ws`              |
| Web browser on the dev machine     | `ws://127.0.0.1:9002/ws`             |
| Physical phone (same Wi-Fi)        | `ws://<dev-machine-LAN-IP>:9002/ws`  |

The dev machine needs the server reachable on the chosen network interface. If you bind to `127.0.0.1` you can only reach it from the same host; set `TRADING_WS_BIND=0.0.0.0:9002` to listen on every interface (open the firewall accordingly).

## Wire-format reference

See [`../../rust/trading-server/docs/api/binary-protocol.md`](../../rust/trading-server/docs/api/binary-protocol.md) and [`websocket.md`](../../rust/trading-server/docs/api/websocket.md). The frame layouts in `src/protocol/qftx.ts` mirror those documents exactly.

## Deploying the web build

```bash
npm run vercel-build       # produces dist/ — a static SPA
npx vercel deploy --prod   # uploads dist/ and prints a *.vercel.app URL
```

See [`DEPLOY.md`](./DEPLOY.md) for the Git-integration path and the **`ws://` mixed-content gotcha** that affects any deployed HTTPS site connecting to a local trading server.
