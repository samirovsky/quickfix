//! Idempotent demo seeder.
//!
//! Activated by `BOT_SERVICE_SEED_DEMO=1`. Creates two users (`alice` and
//! `bob`), publishes a couple of bots from the bundled templates, and
//! inserts 30 days of synthetic trades so the performance endpoints
//! return real-looking numbers. Skips silently if the marketplace already
//! has listings — running it twice does nothing the second time.
//!
//! For local development only. Real users will be provisioned by an admin
//! tool that doesn't exist yet.

use anyhow::Result;
use chrono::{Duration, Utc};
use sqlx::SqlitePool;

use crate::store::{bot_repo, listing_repo, performance_repo, user_repo};
use crate::AppState;

const DEMO_KEY_ALICE: &str = "demo-alice-please-rotate";
const DEMO_KEY_BOB: &str = "demo-bob-please-rotate";

pub async fn seed_if_empty(state: &AppState) -> Result<()> {
    let existing: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM marketplace_listings WHERE status = 'published'")
            .fetch_one(&state.db)
            .await
            .unwrap_or(0);
    if existing > 0 {
        tracing::info!(existing, "demo seed skipped (listings already present)");
        return Ok(());
    }

    tracing::warn!(
        alice = DEMO_KEY_ALICE,
        bob = DEMO_KEY_BOB,
        "seeding demo data — these API keys are public, rotate before production"
    );

    let alice = user_repo::upsert_with_key(&state.db, "alice", DEMO_KEY_ALICE).await?;
    let bob = user_repo::upsert_with_key(&state.db, "bob", DEMO_KEY_BOB).await?;

    // Find templates.
    let tpl_rsi = state
        .templates
        .iter()
        .find(|t| t.id == "tpl_rsi_oversold")
        .ok_or_else(|| anyhow::anyhow!("template tpl_rsi_oversold missing from library"))?;
    let tpl_ema = state
        .templates
        .iter()
        .find(|t| t.id == "tpl_ema_cross")
        .ok_or_else(|| anyhow::anyhow!("template tpl_ema_cross missing from library"))?;
    let tpl_break = state
        .templates
        .iter()
        .find(|t| t.id == "tpl_breakout_box")
        .ok_or_else(|| anyhow::anyhow!("template tpl_breakout_box missing from library"))?;

    // Alice publishes a steady mean-reversion bot.
    let bot1 = bot_repo::create(
        &state.db,
        &alice.id,
        "RSI Bounce v2",
        "Tuned for low-vol majors. Steady singles.",
        &tpl_rsi.strategy,
        &tpl_rsi.asset_filter,
        &format!("template:{}", tpl_rsi.id),
    )
    .await?;
    let l1 = listing_repo::publish(
        &state.db,
        &bot1.id,
        &alice.id,
        "RSI Bounce v2",
        "Mean-reversion on majors. Track record below.",
        999,
    )
    .await?;
    seed_trades(&state.db, &bot1.id, TradeProfile::SteadyWinner).await?;

    // Alice publishes an aggressive breakout bot.
    let bot2 = bot_repo::create(
        &state.db,
        &alice.id,
        "Breakout Beast",
        "20-bar Donchian breakouts. High variance, high return.",
        &tpl_break.strategy,
        &tpl_break.asset_filter,
        &format!("template:{}", tpl_break.id),
    )
    .await?;
    let _l2 = listing_repo::publish(
        &state.db,
        &bot2.id,
        &alice.id,
        "Breakout Beast",
        "Aggressive trend-following. Expect drawdowns.",
        2999,
    )
    .await?;
    seed_trades(&state.db, &bot2.id, TradeProfile::Volatile).await?;

    // Bob publishes an EMA-cross bot.
    let bot3 = bot_repo::create(
        &state.db,
        &bob.id,
        "EMA 12/26",
        "Classic trend follower on the 1h chart.",
        &tpl_ema.strategy,
        &tpl_ema.asset_filter,
        &format!("template:{}", tpl_ema.id),
    )
    .await?;
    let _l3 = listing_repo::publish(
        &state.db,
        &bot3.id,
        &bob.id,
        "EMA 12/26 Crossover",
        "Long-only trend follower. Survives chop, eats trends.",
        1499,
    )
    .await?;
    seed_trades(&state.db, &bot3.id, TradeProfile::SlowGrinder).await?;

    tracing::info!(
        listing_id = %l1.id,
        "demo seed complete — try GET /v1/marketplace/listings"
    );
    Ok(())
}

#[derive(Clone, Copy)]
enum TradeProfile {
    SteadyWinner,
    Volatile,
    SlowGrinder,
}

/// Insert ~30 days of synthetic trades. Deterministic LCG so the demo
/// numbers don't drift run to run (modulo the calendar shift).
async fn seed_trades(pool: &SqlitePool, bot_config_id: &str, profile: TradeProfile) -> Result<()> {
    let mut rng_state: u64 = bot_config_id.bytes().fold(0xcafebabe_u64, |acc, b| {
        acc.wrapping_mul(6364136223846793005).wrapping_add(b as u64)
    });
    let mut rand = || {
        rng_state = rng_state
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        rng_state
    };
    let now = Utc::now();
    for day_back in 0..30i64 {
        let trades_today = match profile {
            TradeProfile::SteadyWinner => (rand() % 4) + 2, // 2..=5
            TradeProfile::Volatile => (rand() % 6) + 1,     // 1..=6
            TradeProfile::SlowGrinder => (rand() % 3) + 1,  // 1..=3
        };
        for i in 0..trades_today {
            let raw = (rand() % 800) as i64; // 0..800 cents
            let pnl_cents = match profile {
                TradeProfile::SteadyWinner => raw - 250,     // mean ~ +150
                TradeProfile::Volatile => (raw - 350) * 4,   // mean ~ +200, fat tails
                TradeProfile::SlowGrinder => (raw / 4) - 60, // mean ~ +40
            };
            let side = (rand() % 2) as i32;
            let executed_at = now
                - Duration::days(day_back)
                - Duration::hours((rand() % 12) as i64)
                - Duration::minutes(i as i64 * 7);
            performance_repo::record_trade(
                pool,
                bot_config_id,
                (day_back * 100 + i as i64) + 1,
                side,
                100 * crate::PRICE_TICKS_PER_DOLLAR + (rand() % 1000) as i64,
                1 + (rand() % 5) as i64,
                pnl_cents,
                ((rand() % 20) as i64).max(1),
                executed_at,
            )
            .await?;
        }
    }
    Ok(())
}
