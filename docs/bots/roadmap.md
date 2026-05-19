# Bot Platform — Implementation Roadmap

Source spec: `Automated & Manual Trading Interface Specification v1.0`.

This roadmap maps that spec onto the existing codebase (Rust matching engine + Expo/React Native mobile client), records the scope decisions we've made for v1, and details the first slice we agreed to build.

---

## 1. v1 posture (decisions already made)

| Topic                     | Decision                                                                                                     |
| ------------------------- | ------------------------------------------------------------------------------------------------------------ |
| Identity                  | **Minimal API-key auth.** Static `api_key → user_id` allowlist loaded from config. No signup, no passwords, no JWT issuer. |
| Multi-tenancy             | **Single-user by default**, but `user_id` is threaded through every data model so multi-tenancy works the moment we add more keys. |
| Marketplace               | **Deferred.** No publishing flow, no subscriptions, no revenue share, no admin approval queue.               |
| Billing                   | **Stubbed.** Billing endpoints return success with computed amounts; no Stripe/Adyen integration. Per-transaction fees are recorded in the DB for audit but never charged. |
| AI strategy generator     | Not in v1 (deferred to slice 2 — see §5). When we add it, Anthropic Claude via the Anthropic SDK.            |
| Execution engine          | Not in v1 CRUD slice. Will be a separate Rust crate that talks to the matching engine over its existing TCP/gRPC API; **no changes to the matching engine itself**. |
| Throughput target         | The spec says 500k concurrent bots. v1 targets 100–1000 bots on a single node. Horizontal scaling is a later concern. |

Anything in the spec not on the above list is **out of scope for v1**. We'll revisit when the foundation is real.

---

## 2. Current codebase ↔ spec mapping

| Spec area (section)                    | Today                                              | Action                                                                                                                          |
| -------------------------------------- | -------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------- |
| Manual trading (§3.1)                  | ✅ `rust/trading-server/` + `clients/mobile/`     | Unchanged. Manual UI keeps working without any bot dependency.                                                                  |
| Order execution pipeline (§3.1, §5.1)  | ✅ matching engine + TCP/gRPC/WS                  | Reused as-is. Bots submit orders through the existing gRPC `OrderSession` like any other client.                                |
| Bot templates library (§3.2.1)         | ❌                                                 | New: bundled with `bot-service` as static JSON. No CMS yet.                                                                     |
| AI prompt → strategy (§3.2.2)          | ❌                                                 | New microservice (slice 2). Anthropic Claude.                                                                                   |
| Visual flowchart editor (§3.2.3)       | ❌                                                 | Mobile-UI work; deferred until form-based editor is solid. Big surface area on mobile, low priority.                            |
| Parameters & config (§3.2.4)           | ❌                                                 | Lives in `BotConfig.strategy_json` (slice 1).                                                                                   |
| Backtesting (§3.2.5)                   | ❌                                                 | New `bot-backtest` crate later; reuses historical data store (to be built).                                                     |
| Deploy live (§3.2.6)                   | ❌                                                 | Lives in the execution engine (slice 3).                                                                                        |
| Monitoring dashboard (§3.3)            | ❌                                                 | Mobile screens (slice 4).                                                                                                       |
| Marketplace (§3.4)                     | ❌                                                 | Deferred.                                                                                                                       |
| Monetisation / billing (§3.5)          | ❌                                                 | Stubbed in slice 1. Real integration later.                                                                                     |
| Bot Engine cluster (§5.1)              | ❌                                                 | Slice 3: a separate Rust crate `rust/bot-engine/`.                                                                               |
| AI Service (§5.2)                      | ❌                                                 | Slice 2.                                                                                                                        |
| Backtesting Engine (§5.3)              | ❌                                                 | Later.                                                                                                                          |
| Marketplace / Billing (§5.4)           | ❌                                                 | Deferred.                                                                                                                       |

---

## 3. Target architecture (after first few slices)

```
                                                +----------------------------+
                                                |  clients/mobile (Expo RN)  |
                                                |  - manual trading (WS)     |
                                                |  - bot list / editor (REST)|
                                                |  - bot monitoring (SSE)    |
                                                +----------------------------+
                                                       |             |
                                            REST/JSON  |             | WS (binary QFTX)
                                                       v             v
+----------------+                          +---------------------+   +----------------------+
| rust/ai-       |  ──── HTTP, slice 2 ───> | rust/bot-service    |   | rust/trading-server  |
| strategy-gen   |        prompt → JSON     | - BotConfig CRUD    |   | - matching engine    |
| (axum + Claude)|                          | - templates         |   | - TCP / gRPC / WS    |
+----------------+                          | - api-key auth      |   +----------------------+
                                            | - billing stubs     |               ^
                                            | - sqlite storage    |               |
                                            +---------------------+        gRPC OrderSession
                                                       ^                          |
                                                       | reads configs            |
                                                       | (slice 3)                |
                                                +---------------------+           |
                                                | rust/bot-engine     | ──────────+
                                                | - subscribes to MD  |
                                                | - eval rules        |
                                                | - submits orders    |
                                                +---------------------+
```

**Key invariant: the matching engine never imports bot code.** Bots are external clients of the engine, exactly like the mobile app or a Python script. This keeps the hot path latency we've been protecting (~150 ns p99) untouched.

---

## 4. Slice plan

Five slices in dependency order. Each is independently mergeable and demoable. **Only slice 1 is detailed below.** Later slices get a 1-paragraph sketch — we'll detail each when its turn comes, with whatever has been learned by then.

### Slice 1 — `bot-service` Rust crate with BotConfig CRUD  ← **next**

Deliverable: a new Rust service exposing REST/JSON endpoints to create, read, update, delete bot configurations, persisted in sqlite, behind an API-key middleware that resolves to `user_id`. Templates are bundled as a static list. No execution, no AI, no backtest. Tests + a smoke script.

Full design in §5 below.

### Slice 2 — AI prompt → strategy JSON microservice

Small Rust crate `rust/ai-strategy-gen/` (or sidecar inside `bot-service` if it stays tiny). `POST /generate-strategy` takes natural-language text and returns a validated strategy JSON conforming to the same schema `bot-service` accepts. Uses the Anthropic Claude SDK with prompt caching for the system prompt + schema definition. Mobile UI hook: a "Generate from prompt" button on the Bot Builder screen that calls this endpoint, displays the JSON in the parameter editor for user review before saving via `bot-service`.

### Slice 3 — Bot Builder mobile screens

Add to the existing Expo app: Templates list, template detail with editable parameters, "Save as my bot" → `bot-service`, "My Bots" list, bot detail, edit, delete. No live execution UI yet (bots stay in `draft` status). Hits slice 1 endpoints + slice 2 if available; works fine without slice 2.

### Slice 4 — Paper execution engine

New Rust crate `rust/bot-engine/`. For each bot config in `paper` mode, runs a lightweight evaluator: subscribes to the matching engine's market data (gRPC `SubscribeMarketData`), computes the configured indicators, evaluates entry/exit conditions, emits orders via gRPC `OrderSession`. State (rolling indicator windows, current position) in-memory per bot. Persists trades back to `bot-service` for history. **Paper only** — uses a separate "paper" symbol space, no real fills.

### Slice 5 — Monitoring + live promotion

Mobile bot-monitoring tab (per-bot tile: P&L, status, trades, log). Bot log streamed via SSE from `bot-service`. "Promote to live" toggle on a bot detail screen flips `mode: paper → live` and the engine starts routing orders against the real symbol space. Risk validator runs in front of every emitted order, identical rules to manual orders.

### Beyond v1 (not slices, just listing)

Backtesting engine, walk-forward optimisation, visual flowchart editor, marketplace + revenue share, Stripe billing, admin tools, fair-usage limits, news-event scheduling, multi-tenant scaling.

---

## 5. Slice 1 in detail — `bot-service` Rust crate

### 5.1 Layout

```
rust/bot-service/
├── Cargo.toml                # axum, tokio, serde, sqlx (sqlite), uuid, anyhow, thiserror, tracing
├── migrations/
│   └── 0001_init.sql         # users, bot_configs, bot_trades, billing_events tables
├── templates/
│   └── library.json          # 5–8 starter templates per spec §3.2.1 categories
├── src/
│   ├── main.rs               # bind, route, run
│   ├── lib.rs                # re-exports
│   ├── config.rs             # env: BOT_SERVICE_BIND, BOT_SERVICE_DB, BOT_SERVICE_KEYS
│   ├── auth.rs               # api-key middleware → UserId
│   ├── error.rs              # ApiError + IntoResponse
│   ├── db.rs                 # sqlx pool, init, migrations
│   ├── models/
│   │   ├── bot_config.rs     # struct mirroring DB row + serde
│   │   ├── strategy.rs       # StrategyConfig JSON schema (strongly typed where it pays)
│   │   └── billing.rs        # FeeBreakdown for stubbed estimates
│   ├── routes/
│   │   ├── bots.rs           # CRUD handlers
│   │   ├── templates.rs      # GET /templates, GET /templates/{id}
│   │   ├── billing.rs        # GET /billing/usage (stub), GET /billing/fees (stub)
│   │   └── health.rs         # GET /healthz
│   └── store/
│       └── bot_repo.rs       # all SQL lives here; routes call this
└── tests/
    ├── api_smoke.rs          # spawn server on :0, exercise full CRUD via reqwest
    └── auth.rs               # missing key → 401, bad key → 401, good key → 200
```

### 5.2 Data model

```sql
-- migrations/0001_init.sql

CREATE TABLE users (
  id           TEXT PRIMARY KEY,           -- "u_" + uuid
  api_key_hash TEXT NOT NULL UNIQUE,       -- sha256 of the key; raw key never stored
  created_at   TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
);

CREATE TABLE bot_configs (
  id            TEXT PRIMARY KEY,          -- "b_" + uuid
  user_id       TEXT NOT NULL REFERENCES users(id),
  name          TEXT NOT NULL,
  description   TEXT NOT NULL DEFAULT '',
  status        TEXT NOT NULL DEFAULT 'draft',   -- draft | paper | live | paused | stopped
  strategy_json TEXT NOT NULL,              -- canonical JSON; validated on write
  asset_filter  TEXT NOT NULL,              -- JSON: { symbols: [...], market: ... }
  source        TEXT NOT NULL DEFAULT 'manual', -- manual | template:<id> | ai
  created_at    TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
  updated_at    TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
);
CREATE INDEX idx_bot_configs_user ON bot_configs(user_id);

CREATE TABLE bot_trades (              -- written by slice 4; created here for forward compat
  id              TEXT PRIMARY KEY,
  bot_config_id   TEXT NOT NULL REFERENCES bot_configs(id) ON DELETE CASCADE,
  order_id        INTEGER NOT NULL,
  side            INTEGER NOT NULL,    -- 0=Buy, 1=Sell (matches QFTX)
  price_ticks     INTEGER NOT NULL,
  qty             INTEGER NOT NULL,
  realised_pnl    INTEGER NOT NULL DEFAULT 0,
  fees_ticks      INTEGER NOT NULL DEFAULT 0,
  executed_at     TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
);
CREATE INDEX idx_bot_trades_bot ON bot_trades(bot_config_id, executed_at DESC);

CREATE TABLE billing_events (         -- stubbed: written but never charged
  id              TEXT PRIMARY KEY,
  user_id         TEXT NOT NULL REFERENCES users(id),
  kind            TEXT NOT NULL,      -- 'transaction_fee' | 'subscription_period'
  amount_cents    INTEGER NOT NULL,
  metadata        TEXT NOT NULL DEFAULT '{}',
  recorded_at     TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
);
```

The `strategy_json` field holds a structured strategy. The schema we'll validate against (subject to refinement once slice 2 starts pushing real prompts at us):

```jsonc
{
  "version": 1,
  "entry": {
    "all_of": [
      { "type": "indicator_threshold",
        "indicator": "rsi", "params": { "period": 14, "timeframe": "1h" },
        "op": "lt", "value": 30 }
    ]
  },
  "exit": {
    "any_of": [
      { "type": "take_profit_pct", "value": 2.0 },
      { "type": "stop_loss_pct",   "value": 1.0 },
      { "type": "trailing_stop_pct", "value": 0.5 }
    ]
  },
  "position_sizing": { "kind": "percent_portfolio", "value": 5.0, "max_notional_cents": 100000 },
  "risk":            { "max_concurrent": 3, "max_daily_loss_cents": 5000, "halt_after_n_losses": 4 },
  "schedule":        { "active_hours_utc": [[13, 21]], "active_days": ["Mon","Tue","Wed","Thu","Fri"] }
}
```

Validated server-side with a `serde` struct rather than a JSON-schema library — typed Rust is the schema. Anything that doesn't deserialise → 400.

### 5.3 Endpoints

All JSON, all `application/json`. All authenticated except `/healthz`.

| Method | Path                          | Body                                              | Returns                                              |
| ------ | ----------------------------- | ------------------------------------------------- | ---------------------------------------------------- |
| GET    | `/healthz`                    | —                                                 | `{ "status": "ok" }`                                 |
| GET    | `/v1/templates`               | —                                                 | `[ TemplateSummary, ... ]`                           |
| GET    | `/v1/templates/{id}`          | —                                                 | full `Template`                                      |
| POST   | `/v1/bots`                    | `{ name, description?, strategy_json, asset_filter, source? }` | `BotConfig` (201)                          |
| POST   | `/v1/bots/from-template/{id}` | `{ name, overrides? }`                            | `BotConfig` (201)                                    |
| GET    | `/v1/bots`                    | —                                                 | `[ BotConfigSummary, ... ]` for the caller           |
| GET    | `/v1/bots/{id}`               | —                                                 | full `BotConfig`                                     |
| PUT    | `/v1/bots/{id}`               | partial `BotConfig`                               | updated `BotConfig`                                  |
| DELETE | `/v1/bots/{id}`               | —                                                 | 204                                                  |
| POST   | `/v1/bots/{id}/status`        | `{ "status": "paper" \| "paused" \| "stopped" }` | updated `BotConfig` (no `live` yet — slice 5)        |
| GET    | `/v1/billing/usage`           | —                                                 | stubbed `{ subscription_status, transaction_fees_ytd }` |
| POST   | `/v1/billing/estimate`        | `{ trades_per_month, asset_class }`               | `{ monthly_cents, breakdown[] }`                     |

Errors are JSON `{ "error": "...", "code": "..." }` with appropriate HTTP status. No HTML error pages.

### 5.4 Auth

```
X-API-Key: <opaque-string>
```

Middleware:
1. Read header, return 401 if missing.
2. Hash with sha256, look up in `users.api_key_hash`. 401 if no match.
3. Inject `UserId` into the request extensions; handlers receive it as an extractor.

Keys are provisioned by a CLI tool: `cargo run --bin bot-service-admin -- add-user --name alice` prints the key once and never again. Out of scope: rotation, scoping, expiration.

### 5.5 Templates

Static JSON file shipped with the crate. 5-8 templates spanning the spec's categories:

| id                  | Category         | One-line description                       |
| ------------------- | ---------------- | ------------------------------------------ |
| `tpl_rsi_oversold`  | Mean Reversion   | Buy RSI<30, sell RSI>70                    |
| `tpl_ema_cross`     | Trend Following  | Buy EMA-12 crosses EMA-26 upward           |
| `tpl_breakout_box`  | Breakout         | Buy 20-bar high; ATR stop                  |
| `tpl_grid`          | Grid             | Fixed grid around mid-price                |
| `tpl_dca_weekly`    | DCA              | Buy fixed notional every Monday 14:00 UTC  |

Loaded once at startup, served from memory.

### 5.6 Tests

Two test files, both spawn the service on `127.0.0.1:0`:

- `tests/api_smoke.rs`: provision a user, create a bot from a template, read it back, edit it, list bots, change status to `paper`, delete it. Use `reqwest`.
- `tests/auth.rs`: no header → 401; garbage header → 401; valid header → 200.

Success criteria for slice 1: both files pass, `cargo clippy -- -D warnings` clean, `cargo fmt -- --check` clean.

### 5.7 Configuration

| Var                     | Default                | Purpose                                                  |
| ----------------------- | ---------------------- | -------------------------------------------------------- |
| `BOT_SERVICE_BIND`      | `127.0.0.1:9100`       | listen address                                           |
| `BOT_SERVICE_DB`        | `bot-service.sqlite`   | sqlite file path                                         |
| `BOT_SERVICE_KEYS`      | _none_                 | optional path to a JSON file of `{ user_id, api_key }`; if set, seeded on startup. Local-dev convenience. |
| `RUST_LOG`              | `info`                 | tracing filter                                           |

### 5.8 Verification

```bash
cd rust/bot-service
cargo build --release
cargo test --release
cargo clippy -- -D warnings
cargo fmt -- --check

# Smoke
BOT_SERVICE_KEYS=./dev-keys.json cargo run --release &
KEY=$(jq -r '.[0].api_key' dev-keys.json)
curl -H "X-API-Key: $KEY" http://127.0.0.1:9100/v1/templates | jq
curl -H "X-API-Key: $KEY" -X POST http://127.0.0.1:9100/v1/bots/from-template/tpl_rsi_oversold \
     -d '{"name":"my first bot"}' -H 'content-type: application/json' | jq
curl -H "X-API-Key: $KEY" http://127.0.0.1:9100/v1/bots | jq
```

Definition of done:

1. The four bash commands above produce the expected JSON.
2. Both integration tests pass.
3. The new crate does not touch any file under `rust/trading-server/` or `clients/mobile/`.
4. README at `rust/bot-service/README.md` documents how to provision a user and call the endpoints.

---

## 6. Out of scope for this roadmap (re-stated, so future-us doesn't drift)

- Marketplace publishing, subscriptions, revenue share, admin approval flow.
- Real billing integration (Stripe / Adyen).
- Walk-forward optimisation and full backtesting engine.
- Visual flowchart editor on mobile.
- Multi-region deployment, 500k concurrent bot scale.
- News-event / economic-calendar integration.
- Copy trading market-manipulation monitoring.
- gRPC for `bot-service` endpoints (REST/JSON is enough for v1; gRPC adds friction for nothing here).
- Web client of the bot UI (web build of the Expo app already exists; that's our web for now).

---

## 7. Open questions before I start slice 1

These will block — or constrain — the implementation. None are deal-breakers; defaults below if you don't pick.

1. **Crate placement.** Standalone crate at `rust/bot-service/` (default), or convert `rust/` into a Cargo workspace and add it as a workspace member? Workspace gives shared `target/` and clippy config across crates; standalone keeps each crate movable. **Default: standalone.**
2. **DB choice.** sqlite (file-based, zero ops, fine for the bot-counts on a single node) vs. postgres (the spec's choice, more ops). **Default: sqlite.** Easy to migrate later — `sqlx` lets us swap drivers without rewriting queries.
3. **Strategy schema strictness.** Strongly-typed `serde` enum-of-condition-kinds (default) vs. accept-any-valid-JSON and validate at execution time. The typed approach catches errors at write time but constrains slice 2's AI to a fixed vocabulary. **Default: strongly typed for v1**; we widen the vocabulary as we add slices.
4. **Where does the matching engine learn about `user_id`?** Not at all in v1 — `bot-service` owns ownership. The engine still operates on `conn_id` and a single trust domain. This means a single bad bot can in principle interfere with another's orders. Acceptable for v1 (single trust domain anyway). Flag for later.
5. **Per-transaction fee accounting.** When slice 4 lands, where do `billing_events` get written from — the bot engine or the bot service? I'd say the engine writes a structured event back to the service over REST, and the service records the billing row. Confirm or change later.

If none of those flip, I'll proceed with the defaults when we move to implementation.
