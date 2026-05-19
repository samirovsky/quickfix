-- Initial schema for bot-service.
--
-- All ids are TEXT prefixed with a kind tag ("u_", "b_", ...) for grep-ability
-- in logs. Timestamps are ISO-8601 strings written via chrono — sqlite has no
-- real TIMESTAMP type but TEXT round-trips through chrono cleanly.

PRAGMA foreign_keys = ON;

CREATE TABLE IF NOT EXISTS users (
    id           TEXT PRIMARY KEY,
    name         TEXT NOT NULL,
    api_key_hash TEXT NOT NULL UNIQUE,
    created_at   TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS bot_configs (
    id            TEXT PRIMARY KEY,
    user_id       TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    name          TEXT NOT NULL,
    description   TEXT NOT NULL DEFAULT '',
    status        TEXT NOT NULL DEFAULT 'draft',
    strategy_json TEXT NOT NULL,
    asset_filter  TEXT NOT NULL,
    source        TEXT NOT NULL DEFAULT 'manual',
    created_at    TEXT NOT NULL,
    updated_at    TEXT NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_bot_configs_user ON bot_configs(user_id);

CREATE TABLE IF NOT EXISTS bot_trades (
    id            TEXT PRIMARY KEY,
    bot_config_id TEXT NOT NULL REFERENCES bot_configs(id) ON DELETE CASCADE,
    order_id      INTEGER NOT NULL,
    side          INTEGER NOT NULL,
    price_ticks   INTEGER NOT NULL,
    qty           INTEGER NOT NULL,
    realised_pnl  INTEGER NOT NULL DEFAULT 0,
    fees_ticks    INTEGER NOT NULL DEFAULT 0,
    executed_at   TEXT NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_bot_trades_bot ON bot_trades(bot_config_id, executed_at DESC);

CREATE TABLE IF NOT EXISTS billing_events (
    id           TEXT PRIMARY KEY,
    user_id      TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    kind         TEXT NOT NULL,
    amount_cents INTEGER NOT NULL,
    metadata     TEXT NOT NULL DEFAULT '{}',
    recorded_at  TEXT NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_billing_events_user ON billing_events(user_id, recorded_at DESC);
