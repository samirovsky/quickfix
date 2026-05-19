-- Marketplace + subscriptions, plus a clarification on bot_trades units.
--
-- bot_trades.realised_pnl is in CENTS (USD). The slice-4 execution engine
-- will be responsible for converting QFTX price ticks to cents at the
-- point of recording a trade. The seeder uses cents directly.

CREATE TABLE IF NOT EXISTS marketplace_listings (
    id                  TEXT PRIMARY KEY,
    bot_config_id       TEXT NOT NULL UNIQUE REFERENCES bot_configs(id) ON DELETE CASCADE,
    creator_id          TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    title               TEXT NOT NULL,
    summary             TEXT NOT NULL,
    monthly_price_cents INTEGER NOT NULL,
    status              TEXT NOT NULL DEFAULT 'published',
    published_at        TEXT NOT NULL,
    unpublished_at      TEXT
);
CREATE INDEX IF NOT EXISTS idx_listings_status ON marketplace_listings(status, published_at DESC);

CREATE TABLE IF NOT EXISTS subscriptions (
    id                      TEXT PRIMARY KEY,
    subscriber_id           TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    listing_id              TEXT NOT NULL REFERENCES marketplace_listings(id) ON DELETE CASCADE,
    allocated_capital_cents INTEGER NOT NULL,
    status                  TEXT NOT NULL DEFAULT 'active',
    started_at              TEXT NOT NULL,
    cancelled_at            TEXT
);
CREATE INDEX IF NOT EXISTS idx_subs_subscriber ON subscriptions(subscriber_id, status);
CREATE INDEX IF NOT EXISTS idx_subs_listing ON subscriptions(listing_id, status);
-- Enforce one active subscription per (subscriber, listing). sqlite supports
-- partial unique indexes which fit exactly:
CREATE UNIQUE INDEX IF NOT EXISTS idx_subs_one_active
    ON subscriptions(subscriber_id, listing_id) WHERE status = 'active';
