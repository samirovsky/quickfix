//! In-process paper execution engine.
//!
//! Activated by `BOT_SERVICE_ENABLE_PAPER_ENGINE=1`. Every
//! `BOT_SERVICE_PAPER_TICK_SECS` (default 30s) it iterates every bot in
//! `status = 'paper'` and emits zero-to-a-handful of synthetic trades
//! per bot into `bot_trades`. The mobile app sees these through the
//! existing performance endpoints — no API change needed.
//!
//! This is **not** a real execution engine. There is no market-data
//! subscription, no condition evaluation, no order routing. It's a
//! simulator shaped exactly like a future engine would be: it takes
//! the bot config as input, produces `BotTrade` rows as output, and
//! runs on a cadence. Swapping the synthetic outcome with a real one
//! (slice 5: connect to the trading-server's gRPC OrderSession) is a
//! local change inside `tick_bot`.

use std::time::Duration;

use anyhow::Result;
use chrono::Utc;
use sqlx::SqlitePool;
use tokio::time::sleep;

use crate::models::bot_config::BotStatus;
use crate::models::strategy::PositionSizing;
use crate::store::{bot_repo, performance_repo};

/// Run the paper-engine loop until the process exits. Designed to be
/// `tokio::spawn`ed from `main.rs`.
pub async fn run(pool: SqlitePool, tick_secs: u64) {
    let interval = Duration::from_secs(tick_secs.max(1));
    tracing::info!(
        tick_secs = interval.as_secs(),
        "paper engine started — generates synthetic trades for paper-mode bots"
    );
    loop {
        if let Err(e) = tick(&pool).await {
            tracing::warn!(error = %e, "paper engine tick failed");
        }
        sleep(interval).await;
    }
}

/// Run one round of the paper engine. Returns the number of trades it
/// inserted. Exposed for tests and for any future admin "kick" endpoint.
pub async fn tick(pool: &SqlitePool) -> Result<usize> {
    // Find paper-mode bots across all users. The store layer is per-user
    // because the public API requires it; here we step around that with
    // a small raw query — paper-engine is a server-side worker.
    let bot_ids: Vec<(String, String)> =
        sqlx::query_as("SELECT id, user_id FROM bot_configs WHERE status = 'paper' ORDER BY id")
            .fetch_all(pool)
            .await?;

    let mut total_inserted = 0usize;
    let now = Utc::now();
    for (bot_id, user_id) in bot_ids {
        let bot = match bot_repo::get(pool, &user_id, &bot_id).await {
            Ok(b) => b,
            Err(_) => continue,
        };
        if bot.status != BotStatus::Paper {
            continue;
        }
        let trades = tick_bot(pool, &bot_id, &bot.strategy.position_sizing, now).await?;
        total_inserted += trades;
    }
    if total_inserted > 0 {
        tracing::info!(trades = total_inserted, "paper engine tick");
    }
    Ok(total_inserted)
}

/// Simulate this minute's trades for one bot. Deterministic-ish — the
/// (bot_id, minute) tuple is the seed so calls within the same minute
/// don't blow up the trade count, but successive minutes yield
/// different outcomes.
async fn tick_bot(
    pool: &SqlitePool,
    bot_id: &str,
    sizing: &PositionSizing,
    now: chrono::DateTime<Utc>,
) -> Result<usize> {
    let minute_bucket = now.timestamp() / 60;
    let mut rng = lcg_seed(bot_id, minute_bucket as u64);

    // 60% chance of any trades this tick.
    if next(&mut rng) % 10 < 4 {
        return Ok(0);
    }

    // 1–3 trades.
    let n = (next(&mut rng) % 3 + 1) as usize;

    let nominal_cents = sizing_nominal_cents(sizing);
    // Centre P&L magnitude near ~2% of nominal; widen the tail.
    let centre = ((nominal_cents as i64).max(1_000) * 2) / 100;

    let mut inserted = 0usize;
    for i in 0..n {
        // Win bias 55%.
        let win = (next(&mut rng) % 100) < 55;
        // Magnitude jitter: half to 2× the centre.
        let mag = centre * ((next(&mut rng) % 150 + 50) as i64) / 100;
        let pnl_cents = if win { mag } else { -(mag * 90) / 100 };

        let side = (next(&mut rng) % 2) as i32;
        let price_ticks = 100 * crate::PRICE_TICKS_PER_DOLLAR + (next(&mut rng) % 10_000) as i64;
        let qty = 1 + (next(&mut rng) % 5) as i64;
        let order_id = minute_bucket * 1000 + i as i64;
        let ts = now - chrono::Duration::seconds(i as i64 * 7);

        performance_repo::record_trade(
            pool,
            bot_id,
            order_id,
            side,
            price_ticks,
            qty,
            pnl_cents,
            ((next(&mut rng) % 10) as i64).max(1),
            ts,
        )
        .await?;
        inserted += 1;
    }
    Ok(inserted)
}

fn sizing_nominal_cents(sizing: &PositionSizing) -> u64 {
    match sizing {
        PositionSizing::FixedAmount { value_cents } => *value_cents,
        PositionSizing::PercentPortfolio {
            max_notional_cents, ..
        } => *max_notional_cents,
        PositionSizing::Kelly {
            max_notional_cents, ..
        } => *max_notional_cents,
    }
}

// A tiny LCG. Same recurrence as the demo seeder so the two paths
// agree about what "deterministic given a seed" means.
fn lcg_seed(bot_id: &str, minute_bucket: u64) -> u64 {
    let mut s = minute_bucket
        .wrapping_mul(6364136223846793005)
        .wrapping_add(0xcafebabe);
    for b in bot_id.bytes() {
        s = s.wrapping_mul(6364136223846793005).wrapping_add(b as u64);
    }
    s
}
fn next(state: &mut u64) -> u64 {
    *state = state
        .wrapping_mul(6364136223846793005)
        .wrapping_add(1442695040888963407);
    *state
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::build_state;
    use crate::store::{bot_repo, user_repo};

    #[tokio::test]
    async fn tick_inserts_trades_for_paper_bots_only() {
        let state = build_state("sqlite::memory:").await.unwrap();

        // Provision a user, two bots — one paper, one draft.
        let alice = user_repo::upsert_with_key(&state.db, "alice", "k1")
            .await
            .unwrap();
        let tpl = state.templates.first().unwrap().clone();
        let paper = bot_repo::create(
            &state.db,
            &alice.id,
            "paper bot",
            "",
            &tpl.strategy,
            &tpl.asset_filter,
            &format!("template:{}", tpl.id),
        )
        .await
        .unwrap();
        bot_repo::set_status(&state.db, &alice.id, &paper.id, BotStatus::Paper)
            .await
            .unwrap();
        let _draft = bot_repo::create(
            &state.db,
            &alice.id,
            "draft bot",
            "",
            &tpl.strategy,
            &tpl.asset_filter,
            &format!("template:{}", tpl.id),
        )
        .await
        .unwrap();

        // Run a few ticks to defeat the 40% no-trade gate.
        let mut total = 0usize;
        for _ in 0..10 {
            total += tick(&state.db).await.unwrap();
        }
        assert!(total > 0, "paper engine produced no trades across 10 ticks");

        // Trades must belong to the paper bot.
        let count: i64 =
            sqlx::query_scalar("SELECT COUNT(*) FROM bot_trades WHERE bot_config_id = ?1")
                .bind(&paper.id)
                .fetch_one(&state.db)
                .await
                .unwrap();
        assert!(count > 0, "no trades attributed to paper bot");
    }
}
