# bot-service

REST/JSON service that owns **bot configuration**: CRUD, templates, stubbed billing, API-key auth. First slice of the bot-platform roadmap at [`docs/bots/roadmap.md`](../../docs/bots/roadmap.md).

No execution, no AI, no marketplace, no real billing yet — those are later slices. The matching engine at `rust/trading-server/` is untouched.

## Architecture in one paragraph

`axum` HTTP server. `sqlite` via `sqlx` for persistence. Static `templates/library.json` bundled into the binary. One middleware: `X-API-Key` → sha-256 → `users.api_key_hash` → `user_id` injected into request extensions; every `/v1/*` route requires it. Strongly-typed `Strategy` schema in `models::strategy` — invalid JSON fails at write time before it hits the DB.

## Endpoints

| Method | Path                          | Notes                                                            |
| ------ | ----------------------------- | ---------------------------------------------------------------- |
| GET    | `/healthz`                    | Public                                                           |
| GET    | `/v1/templates`               | List bundled templates                                           |
| GET    | `/v1/templates/{id}`          | Full template, including strategy                                |
| POST   | `/v1/bots`                    | Create a bot from a full strategy payload                        |
| POST   | `/v1/bots/from-template/{id}` | Clone a template into a new bot                                  |
| GET    | `/v1/bots`                    | List the caller's bots                                           |
| GET    | `/v1/bots/{id}`               | Full bot config                                                  |
| PUT    | `/v1/bots/{id}`               | Partial update (name, description, strategy, asset filter)       |
| DELETE | `/v1/bots/{id}`               | 204 on success                                                   |
| POST   | `/v1/bots/{id}/status`        | Transition between `draft` / `paper` / `paused` / `stopped`. Refuses `live` in v1. |
| GET    | `/v1/billing/usage`           | Stubbed totals from `billing_events` (zero in v1)               |
| POST   | `/v1/billing/estimate`        | Fee estimate by trades/month + asset class                       |

Errors are JSON `{ "error": "...", "code": "..." }`.

## Configuration

| Env var               | Default                       | Purpose                                                              |
| --------------------- | ----------------------------- | -------------------------------------------------------------------- |
| `BOT_SERVICE_BIND`    | `127.0.0.1:9100`              | Listen address.                                                       |
| `BOT_SERVICE_DB`      | `sqlite://bot-service.sqlite` | sqlx URL. Use `sqlite::memory:` for ephemeral local dev.              |
| `BOT_SERVICE_KEYS`    | _none_                        | Optional path to a JSON file of `[{ "name", "api_key" }, ...]`. Seeded on startup. Local-dev convenience only — keys are stored as sha-256 hashes so a real admin tool will replace this. |
| `RUST_LOG`            | `info`                        | `tracing` filter.                                                    |

## Smoke test

```bash
# Build
cd rust/bot-service
cargo build --release

# Provision a key (don't commit dev-keys.json)
cat > dev-keys.json <<'EOF'
[{ "name": "alice", "api_key": "dev-key-alice-please-rotate" }]
EOF

# Run
BOT_SERVICE_KEYS=./dev-keys.json cargo run --release &
sleep 1

KEY=dev-key-alice-please-rotate
BASE=http://127.0.0.1:9100

# 1) Templates
curl -sS -H "X-API-Key: $KEY" $BASE/v1/templates | jq

# 2) Create a bot from the first template
curl -sS -H "X-API-Key: $KEY" -H "Content-Type: application/json" \
     -X POST $BASE/v1/bots/from-template/tpl_rsi_oversold \
     -d '{"name":"my first bot"}' | jq

# 3) List the caller's bots
curl -sS -H "X-API-Key: $KEY" $BASE/v1/bots | jq

# 4) Billing stub
curl -sS -H "X-API-Key: $KEY" -H "Content-Type: application/json" \
     -X POST $BASE/v1/billing/estimate \
     -d '{"trades_per_month": 50, "asset_class": "stocks"}' | jq
```

## Tests

```bash
cargo test                       # 2 unit + 3 api-smoke + 4 auth = 9
cargo clippy --all-targets -- -D warnings
cargo fmt -- --check
```

## What this slice does NOT do

* Execute strategies — slice 4 (`rust/bot-engine/`).
* Generate strategies from natural language — slice 2 (`rust/ai-strategy-gen/`).
* Backtest — later.
* Marketplace, subscriptions, revenue share — deferred.
* Charge real money — `billing_events` is recorded but never settled.
* User signup / password reset / SSO — only API keys, only at startup.
* Pagination — list endpoints are small in v1; add when bots grow into the thousands per user.
