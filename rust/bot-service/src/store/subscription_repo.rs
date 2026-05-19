//! SQL for `subscriptions`.

use chrono::Utc;
use sqlx::{Row, SqlitePool};
use uuid::Uuid;

use crate::error::{ApiError, ApiResult};
use crate::models::marketplace::{Subscription, SubscriptionStatus};

fn new_id() -> String {
    format!("s_{}", Uuid::new_v4().simple())
}

fn row_to_sub(row: sqlx::sqlite::SqliteRow) -> ApiResult<Subscription> {
    let status_raw: String = row.try_get("status")?;
    let status = SubscriptionStatus::parse(&status_raw)
        .ok_or_else(|| ApiError::Internal(anyhow::anyhow!("bad sub status: {status_raw}")))?;
    Ok(Subscription {
        id: row.try_get("id")?,
        subscriber_id: row.try_get("subscriber_id")?,
        listing_id: row.try_get("listing_id")?,
        allocated_capital_cents: row.try_get("allocated_capital_cents")?,
        status,
        started_at: row.try_get("started_at")?,
    })
}

pub async fn subscribe(
    pool: &SqlitePool,
    subscriber_id: &str,
    listing_id: &str,
    allocated_capital_cents: i64,
) -> ApiResult<Subscription> {
    if allocated_capital_cents <= 0 {
        return Err(ApiError::BadRequest(
            "allocated_capital_cents must be > 0".into(),
        ));
    }
    // Listing must exist + be published.
    let status: Option<String> =
        sqlx::query_scalar("SELECT status FROM marketplace_listings WHERE id = ?1")
            .bind(listing_id)
            .fetch_optional(pool)
            .await?;
    match status.as_deref() {
        Some("published") => {}
        Some(_) => return Err(ApiError::BadRequest("listing is not published".into())),
        None => return Err(ApiError::NotFound),
    }

    let id = new_id();
    let now = Utc::now();
    let res = sqlx::query(
        "INSERT INTO subscriptions
         (id, subscriber_id, listing_id, allocated_capital_cents, status, started_at)
         VALUES (?1, ?2, ?3, ?4, 'active', ?5)",
    )
    .bind(&id)
    .bind(subscriber_id)
    .bind(listing_id)
    .bind(allocated_capital_cents)
    .bind(now)
    .execute(pool)
    .await;
    if let Err(sqlx::Error::Database(err)) = &res {
        // sqlite reports "UNIQUE constraint failed: subscriptions.subscriber_id, subscriptions.listing_id"
        let msg = err.message();
        if msg.contains("UNIQUE") && msg.contains("subscriptions") {
            return Err(ApiError::BadRequest(
                "already subscribed to this listing".into(),
            ));
        }
    }
    res?;
    get(pool, &id).await
}

pub async fn cancel(pool: &SqlitePool, subscriber_id: &str, sub_id: &str) -> ApiResult<()> {
    let now = Utc::now();
    let res = sqlx::query(
        "UPDATE subscriptions
         SET status = 'cancelled', cancelled_at = ?1
         WHERE id = ?2 AND subscriber_id = ?3 AND status = 'active'",
    )
    .bind(now)
    .bind(sub_id)
    .bind(subscriber_id)
    .execute(pool)
    .await?;
    if res.rows_affected() == 0 {
        return Err(ApiError::NotFound);
    }
    Ok(())
}

pub async fn get(pool: &SqlitePool, sub_id: &str) -> ApiResult<Subscription> {
    let row = sqlx::query(
        "SELECT id, subscriber_id, listing_id, allocated_capital_cents, status, started_at
         FROM subscriptions WHERE id = ?1",
    )
    .bind(sub_id)
    .fetch_optional(pool)
    .await?
    .ok_or(ApiError::NotFound)?;
    row_to_sub(row)
}

pub async fn list_for_subscriber(
    pool: &SqlitePool,
    subscriber_id: &str,
) -> ApiResult<Vec<Subscription>> {
    let rows = sqlx::query(
        "SELECT id, subscriber_id, listing_id, allocated_capital_cents, status, started_at
         FROM subscriptions
         WHERE subscriber_id = ?1
         ORDER BY started_at DESC",
    )
    .bind(subscriber_id)
    .fetch_all(pool)
    .await?;
    rows.into_iter().map(row_to_sub).collect()
}

pub async fn is_subscribed(
    pool: &SqlitePool,
    subscriber_id: &str,
    listing_id: &str,
) -> ApiResult<bool> {
    let count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM subscriptions
         WHERE subscriber_id = ?1 AND listing_id = ?2 AND status = 'active'",
    )
    .bind(subscriber_id)
    .bind(listing_id)
    .fetch_one(pool)
    .await?;
    Ok(count > 0)
}
