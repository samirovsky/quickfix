//! Aggregations over `bot_trades` for the performance endpoints.

use chrono::{Duration, Utc};
use sqlx::{Row, SqlitePool};

use crate::error::ApiResult;
use crate::models::performance::{DailyMetric, PerformanceMetrics};

pub async fn metrics_for_bot(
    pool: &SqlitePool,
    bot_config_id: &str,
    window_days: u32,
) -> ApiResult<PerformanceMetrics> {
    let since = (Utc::now() - Duration::days(window_days as i64)).to_rfc3339();

    let (trades, total_pnl, largest_win, largest_loss): (i64, i64, i64, i64) = sqlx::query_as(
        "SELECT COUNT(*) AS trades,
                COALESCE(SUM(realised_pnl), 0)        AS total_pnl,
                COALESCE(MAX(realised_pnl), 0)        AS largest_win,
                COALESCE(MIN(realised_pnl), 0)        AS largest_loss
         FROM bot_trades
         WHERE bot_config_id = ?1 AND executed_at >= ?2",
    )
    .bind(bot_config_id)
    .bind(&since)
    .fetch_one(pool)
    .await?;

    let wins: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM bot_trades
         WHERE bot_config_id = ?1 AND executed_at >= ?2 AND realised_pnl > 0",
    )
    .bind(bot_config_id)
    .bind(&since)
    .fetch_one(pool)
    .await?;
    let win_rate = if trades > 0 {
        wins as f64 / trades as f64
    } else {
        0.0
    };

    let rows = sqlx::query(
        "SELECT substr(executed_at, 1, 10) AS day,
                COALESCE(SUM(realised_pnl), 0)   AS pnl,
                COUNT(*)                         AS n
         FROM bot_trades
         WHERE bot_config_id = ?1 AND executed_at >= ?2
         GROUP BY day
         ORDER BY day DESC",
    )
    .bind(bot_config_id)
    .bind(&since)
    .fetch_all(pool)
    .await?;
    let mut daily = Vec::with_capacity(rows.len());
    for row in rows {
        daily.push(DailyMetric {
            date: row.try_get::<String, _>("day")?,
            realised_pnl_cents: row.try_get::<i64, _>("pnl")?,
            trades: row.try_get::<i64, _>("n")?,
        });
    }

    Ok(PerformanceMetrics {
        window_days,
        total_trades: trades,
        total_realised_pnl_cents: total_pnl,
        win_rate,
        // largest_loss is the smallest (most negative) value; clamp to 0 if no losing trades.
        largest_win_cents: largest_win.max(0),
        largest_loss_cents: largest_loss.min(0),
        daily,
    })
}

// Trade inserts have many columns by nature; grouping them into a struct
// only obscures call sites that already look like a SQL insert.
#[allow(clippy::too_many_arguments)]
pub async fn record_trade(
    pool: &SqlitePool,
    bot_config_id: &str,
    order_id: i64,
    side: i32,
    price_ticks: i64,
    qty: i64,
    realised_pnl_cents: i64,
    fees_ticks: i64,
    executed_at: chrono::DateTime<Utc>,
) -> ApiResult<String> {
    let id = format!("t_{}", uuid::Uuid::new_v4().simple());
    sqlx::query(
        "INSERT INTO bot_trades
         (id, bot_config_id, order_id, side, price_ticks, qty, realised_pnl, fees_ticks, executed_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
    )
    .bind(&id)
    .bind(bot_config_id)
    .bind(order_id)
    .bind(side)
    .bind(price_ticks)
    .bind(qty)
    .bind(realised_pnl_cents)
    .bind(fees_ticks)
    .bind(executed_at)
    .execute(pool)
    .await?;
    Ok(id)
}
