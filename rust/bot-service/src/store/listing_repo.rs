//! SQL for `marketplace_listings`.

use chrono::Utc;
use sqlx::{Row, SqlitePool};
use uuid::Uuid;

use crate::error::{ApiError, ApiResult};
use crate::models::marketplace::{Listing, ListingStatus};

fn new_id() -> String {
    format!("l_{}", Uuid::new_v4().simple())
}

fn row_to_listing(row: sqlx::sqlite::SqliteRow) -> ApiResult<Listing> {
    let status_raw: String = row.try_get("status")?;
    let status = ListingStatus::parse(&status_raw)
        .ok_or_else(|| ApiError::Internal(anyhow::anyhow!("bad listing status: {status_raw}")))?;
    Ok(Listing {
        id: row.try_get("id")?,
        bot_config_id: row.try_get("bot_config_id")?,
        creator_id: row.try_get("creator_id")?,
        creator_name: row.try_get("creator_name")?,
        title: row.try_get("title")?,
        summary: row.try_get("summary")?,
        monthly_price_cents: row.try_get("monthly_price_cents")?,
        status,
        published_at: row.try_get("published_at")?,
        total_subscribers: row.try_get("total_subscribers")?,
    })
}

const SELECT_LISTING_JOIN: &str = "
    SELECT l.id, l.bot_config_id, l.creator_id, u.name AS creator_name,
           l.title, l.summary, l.monthly_price_cents, l.status, l.published_at,
           COALESCE((SELECT COUNT(*) FROM subscriptions s
                     WHERE s.listing_id = l.id AND s.status = 'active'), 0) AS total_subscribers
    FROM marketplace_listings l
    JOIN users u ON u.id = l.creator_id
";

pub async fn publish(
    pool: &SqlitePool,
    bot_config_id: &str,
    creator_id: &str,
    title: &str,
    summary: &str,
    monthly_price_cents: i64,
) -> ApiResult<Listing> {
    if title.trim().is_empty() {
        return Err(ApiError::BadRequest("title must not be empty".into()));
    }
    if monthly_price_cents < 0 {
        return Err(ApiError::BadRequest(
            "monthly_price_cents must be >= 0".into(),
        ));
    }

    // If a listing already exists for this bot, just republish it. The
    // UNIQUE constraint on bot_config_id keeps us to one row per bot.
    let existing_id: Option<String> = sqlx::query_scalar(
        "SELECT id FROM marketplace_listings WHERE bot_config_id = ?1 AND creator_id = ?2",
    )
    .bind(bot_config_id)
    .bind(creator_id)
    .fetch_optional(pool)
    .await?;

    let now = Utc::now();
    if let Some(id) = existing_id {
        sqlx::query(
            "UPDATE marketplace_listings
             SET title = ?1, summary = ?2, monthly_price_cents = ?3,
                 status = 'published', published_at = ?4, unpublished_at = NULL
             WHERE id = ?5",
        )
        .bind(title)
        .bind(summary)
        .bind(monthly_price_cents)
        .bind(now)
        .bind(&id)
        .execute(pool)
        .await?;
        get(pool, &id).await
    } else {
        let id = new_id();
        sqlx::query(
            "INSERT INTO marketplace_listings
             (id, bot_config_id, creator_id, title, summary, monthly_price_cents, status, published_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, 'published', ?7)",
        )
        .bind(&id)
        .bind(bot_config_id)
        .bind(creator_id)
        .bind(title)
        .bind(summary)
        .bind(monthly_price_cents)
        .bind(now)
        .execute(pool)
        .await?;
        get(pool, &id).await
    }
}

pub async fn unpublish(pool: &SqlitePool, listing_id: &str, creator_id: &str) -> ApiResult<()> {
    let now = Utc::now();
    let res = sqlx::query(
        "UPDATE marketplace_listings
         SET status = 'unpublished', unpublished_at = ?1
         WHERE id = ?2 AND creator_id = ?3 AND status = 'published'",
    )
    .bind(now)
    .bind(listing_id)
    .bind(creator_id)
    .execute(pool)
    .await?;
    if res.rows_affected() == 0 {
        return Err(ApiError::NotFound);
    }
    Ok(())
}

pub async fn get(pool: &SqlitePool, listing_id: &str) -> ApiResult<Listing> {
    let row = sqlx::query(&format!("{SELECT_LISTING_JOIN} WHERE l.id = ?1"))
        .bind(listing_id)
        .fetch_optional(pool)
        .await?
        .ok_or(ApiError::NotFound)?;
    row_to_listing(row)
}

pub async fn list_published(pool: &SqlitePool) -> ApiResult<Vec<Listing>> {
    let rows = sqlx::query(&format!(
        "{SELECT_LISTING_JOIN} WHERE l.status = 'published' ORDER BY l.published_at DESC"
    ))
    .fetch_all(pool)
    .await?;
    rows.into_iter().map(row_to_listing).collect()
}

/// Returns the `bot_config_id` referenced by a listing, used to authorise
/// performance lookups for subscribers.
pub async fn bot_id_for(pool: &SqlitePool, listing_id: &str) -> ApiResult<String> {
    let id: Option<String> =
        sqlx::query_scalar("SELECT bot_config_id FROM marketplace_listings WHERE id = ?1")
            .bind(listing_id)
            .fetch_optional(pool)
            .await?;
    id.ok_or(ApiError::NotFound)
}
