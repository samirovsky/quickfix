//! Billing reads. Writes are deferred to slice 4 (engine emits events).

use chrono::{Datelike, Utc};
use sqlx::SqlitePool;

use crate::error::ApiResult;

pub async fn fees_year_to_date(pool: &SqlitePool, user_id: &str) -> ApiResult<i64> {
    let year_start = format!("{}-01-01T00:00:00Z", Utc::now().year());
    let total: Option<i64> = sqlx::query_scalar(
        "SELECT COALESCE(SUM(amount_cents), 0)
         FROM billing_events
         WHERE user_id = ?1 AND kind = 'transaction_fee' AND recorded_at >= ?2",
    )
    .bind(user_id)
    .bind(year_start)
    .fetch_one(pool)
    .await?;
    Ok(total.unwrap_or(0))
}

pub async fn fees_this_month(pool: &SqlitePool, user_id: &str) -> ApiResult<i64> {
    let now = Utc::now();
    let month_start = format!("{:04}-{:02}-01T00:00:00Z", now.year(), now.month());
    let total: Option<i64> = sqlx::query_scalar(
        "SELECT COALESCE(SUM(amount_cents), 0)
         FROM billing_events
         WHERE user_id = ?1 AND kind = 'transaction_fee' AND recorded_at >= ?2",
    )
    .bind(user_id)
    .bind(month_start)
    .fetch_one(pool)
    .await?;
    Ok(total.unwrap_or(0))
}
