# bot-service

REST/JSON service that owns **bot configuration**: CRUD, templates, stubbed billing, API-key auth. First slice of the bot-platform roadmap at [`docs/bots/roadmap.md`](../../docs/bots/roadmap.md).

No execution, no AI, no marketplace, no real billing yet — those are later slices. The matching engine at `rust/trading-server/` is untouched.

## Architecture in one paragraph

`axum` HTTP server. `sqlite` via `sqlx` for persistence. Static `templates/library.json` bundled into the binary. One middleware: `X-API-Key` → sha-256 → `users.api_key_hash` → `user_id` injected into request extensions; every `/v1/*` route requires it. Strongly-typed `Strategy` schema in `models::strategy` — invalid JSON fails at write time before it hits the DB.

## Endpoints

| Method | Path                                          | Notes                                                            |
| ------ | --------------------------------------------- | ---------------------------------------------------------------- |
| GET    | `/healthz`                                    | Public                                                           |
| GET    | `/v1/templates`                               | List bundled templates                                           |
| GET    | `/v1/templates/{id}`                          | Full template, including strategy                                |
| POST   | `/v1/bots`                                    | Create a bot from a full strategy payload                        |
| POST   | `/v1/bots/from-template/{id}`                 | Clone a template into a new bot                                  |
| GET    | `/v1/bots`                                    | List the caller's bots                                           |
| GET    | `/v1/bots/{id}`                               | Full bot config                                                  |
| PUT    | `/v1/bots/{id}`                               | Partial update (name, description, strategy, asset filter)       |
| DELETE | `/v1/bots/{id}`                               | 204 on success                                                   |
| POST   | `/v1/bots/{id}/status`                        | Transition between `draft` / `paper` / `paused` / `stopped`. Refuses `live` in v1. |
| POST   | `/v1/bots/{id}/publish`                       | Publish your bot to the marketplace                              |
| GET    | `/v1/bots/{id}/performance?days=N`            | Owner-only performance metrics from `bot_trades`                 |
| GET    | `/v1/marketplace/listings`                    | Browse all published listings (any authed user)                  |
| GET    | `/v1/marketplace/listings/{id}`               | Single listing with creator + subscriber count                   |
| POST   | `/v1/marketplace/listings/{id}/unpublish`     | Creator-only — flips status to `unpublished`                     |
| POST   | `/v1/marketplace/listings/{id}/subscribe`     | Subscribe with allocated capital                                 |
| GET    | `/v1/marketplace/listings/{id}/performance?days=N` | Performance metrics for the listed bot                       |
| POST   | `/v1/subscriptions/{id}/cancel`               | Cancel one of your subscriptions                                 |
| GET    | `/v1/me/subscriptions`                        | All active subscriptions with their listings                     |
| GET    | `/v1/billing/usage`                           | Stubbed totals from `billing_events` (zero in v1)               |
| POST   | `/v1/billing/estimate`                        | Fee estimate by trades/month + asset class                       |
| POST   | `/v1/ai/generate-strategy`                    | **Stubbed.** Takes `{prompt}`, returns a `Strategy` derived from a keyword match against the bundled templates. Wire shape is what a real LLM impl will return — `stub: true` until that lands. |

Errors are JSON `{ "error": "...", "code": "..." }`.

## Configuration

| Env var                      | Default                       | Purpose                                                              |
| ---------------------------- | ----------------------------- | -------------------------------------------------------------------- |
| `BOT_SERVICE_BIND`           | `127.0.0.1:9100`              | Listen address.                                                       |
| `BOT_SERVICE_DB`             | `sqlite://bot-service.sqlite` | sqlx URL. Use `sqlite::memory:` for ephemeral local dev.              |
| `BOT_SERVICE_KEYS`           | _none_                        | Optional path to a JSON file of `[{ "name", "api_key" }, ...]`. Seeded on startup. Local-dev convenience only. |
| `BOT_SERVICE_SEED_DEMO`      | _none_                        | Set to `1` to seed two demo users (`alice` / `bob`), three published bots, and 30 days of synthetic trades. Idempotent — skips if the marketplace already has listings. |
| `BOT_SERVICE_ENABLE_PAPER_ENGINE` | _none_                   | Set to `1` to run the in-process paper-execution engine. Iterates `status='paper'` bots every `BOT_SERVICE_PAPER_TICK_SECS` (default 30) and writes synthetic `bot_trades` rows. Not a real engine; swap with a real one when slice 5 lands. |
| `BOT_SERVICE_PAPER_TICK_SECS`     | `30`                     | Cadence for the paper engine.                                       |
| `BOT_SERVICE_CORS_ORIGINS`   | _none_                        | Comma-separated allowlist for CORS (e.g. `https://app.vercel.app`). `*` is allowed for demos but never echoes back credentials. Leave empty for same-origin (local dev). |
| `RUST_LOG`                   | `info`                        | `tracing` filter.                                                    |

## Container

A multi-stage `Dockerfile` is included for deployment:

```bash
docker build -t bot-service -f rust/bot-service/Dockerfile .
docker run --rm -p 9100:9100 \
  -v $(pwd)/data:/data \
  -e BOT_SERVICE_DB=sqlite:///data/bot.sqlite \
  -e BOT_SERVICE_SEED_DEMO=1 \
  -e BOT_SERVICE_CORS_ORIGINS='*' \
  bot-service
```

The full deploy story (Vercel + Fly.io/Render, mixed-content gotchas) is in [`../../clients/mobile/DEPLOY.md`](../../clients/mobile/DEPLOY.md).

Demo keys printed at startup when `BOT_SERVICE_SEED_DEMO=1`:
- `demo-alice-please-rotate`
- `demo-bob-please-rotate`

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
