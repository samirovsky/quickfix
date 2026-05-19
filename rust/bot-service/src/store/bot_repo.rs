//! All SQL touching `bot_configs` lives here. Routes call these.

use chrono::{DateTime, Utc};
use sqlx::{Row, SqlitePool};
use uuid::Uuid;

use crate::error::{ApiError, ApiResult};
use crate::models::bot_config::{BotConfig, BotStatus};
use crate::models::strategy::{AssetFilter, Strategy};

fn new_id() -> String {
    format!("b_{}", Uuid::new_v4().simple())
}

fn row_to_config(row: sqlx::sqlite::SqliteRow) -> ApiResult<BotConfig> {
    let id: String = row.try_get("id")?;
    let user_id: String = row.try_get("user_id")?;
    let name: String = row.try_get("name")?;
    let description: String = row.try_get("description")?;
    let status_raw: String = row.try_get("status")?;
    let strategy_json: String = row.try_get("strategy_json")?;
    let asset_filter_json: String = row.try_get("asset_filter")?;
    let source: String = row.try_get("source")?;
    let created_at: DateTime<Utc> = row.try_get("created_at")?;
    let updated_at: DateTime<Utc> = row.try_get("updated_at")?;

    let status = BotStatus::parse(&status_raw)
        .ok_or_else(|| ApiError::Internal(anyhow::anyhow!("bad status in DB: {status_raw}")))?;
    let strategy: Strategy = serde_json::from_str(&strategy_json).map_err(|e| {
        ApiError::Internal(anyhow::anyhow!("strategy_json deserialise failed: {e}"))
    })?;
    let asset_filter: AssetFilter = serde_json::from_str(&asset_filter_json)
        .map_err(|e| ApiError::Internal(anyhow::anyhow!("asset_filter deserialise failed: {e}")))?;

    Ok(BotConfig {
        id,
        user_id,
        name,
        description,
        status,
        strategy,
        asset_filter,
        source,
        created_at,
        updated_at,
    })
}

pub async fn create(
    pool: &SqlitePool,
    user_id: &str,
    name: &str,
    description: &str,
    strategy: &Strategy,
    asset_filter: &AssetFilter,
    source: &str,
) -> ApiResult<BotConfig> {
    let id = new_id();
    let now = Utc::now();
    let strategy_json =
        serde_json::to_string(strategy).map_err(|e| ApiError::Internal(e.into()))?;
    let asset_filter_json =
        serde_json::to_string(asset_filter).map_err(|e| ApiError::Internal(e.into()))?;

    sqlx::query(
        r#"INSERT INTO bot_configs
           (id, user_id, name, description, status, strategy_json, asset_filter, source, created_at, updated_at)
           VALUES (?1, ?2, ?3, ?4, 'draft', ?5, ?6, ?7, ?8, ?8)"#,
    )
    .bind(&id)
    .bind(user_id)
    .bind(name)
    .bind(description)
    .bind(&strategy_json)
    .bind(&asset_filter_json)
    .bind(source)
    .bind(now)
    .execute(pool)
    .await?;

    get(pool, user_id, &id).await
}

pub async fn get(pool: &SqlitePool, user_id: &str, id: &str) -> ApiResult<BotConfig> {
    let row = sqlx::query(
        "SELECT id, user_id, name, description, status, strategy_json, asset_filter, source, created_at, updated_at
         FROM bot_configs WHERE id = ?1 AND user_id = ?2",
    )
    .bind(id)
    .bind(user_id)
    .fetch_optional(pool)
    .await?
    .ok_or(ApiError::NotFound)?;
    row_to_config(row)
}

pub async fn list(pool: &SqlitePool, user_id: &str) -> ApiResult<Vec<BotConfig>> {
    let rows = sqlx::query(
        "SELECT id, user_id, name, description, status, strategy_json, asset_filter, source, created_at, updated_at
         FROM bot_configs WHERE user_id = ?1 ORDER BY updated_at DESC",
    )
    .bind(user_id)
    .fetch_all(pool)
    .await?;
    rows.into_iter().map(row_to_config).collect()
}

pub async fn update(
    pool: &SqlitePool,
    user_id: &str,
    id: &str,
    name: Option<&str>,
    description: Option<&str>,
    strategy: Option<&Strategy>,
    asset_filter: Option<&AssetFilter>,
) -> ApiResult<BotConfig> {
    // Load → apply → save. With sqlite that's two round-trips; the
    // alternative is a long COALESCE update. The clarity wins here.
    let current = get(pool, user_id, id).await?;
    let new_name = name.unwrap_or(&current.name);
    let new_desc = description.unwrap_or(&current.description);
    let new_strategy = strategy.unwrap_or(&current.strategy);
    let new_asset = asset_filter.unwrap_or(&current.asset_filter);

    let strategy_json =
        serde_json::to_string(new_strategy).map_err(|e| ApiError::Internal(e.into()))?;
    let asset_filter_json =
        serde_json::to_string(new_asset).map_err(|e| ApiError::Internal(e.into()))?;
    let now = Utc::now();

    sqlx::query(
        r#"UPDATE bot_configs
           SET name = ?1, description = ?2, strategy_json = ?3, asset_filter = ?4, updated_at = ?5
           WHERE id = ?6 AND user_id = ?7"#,
    )
    .bind(new_name)
    .bind(new_desc)
    .bind(&strategy_json)
    .bind(&asset_filter_json)
    .bind(now)
    .bind(id)
    .bind(user_id)
    .execute(pool)
    .await?;
    get(pool, user_id, id).await
}

pub async fn set_status(
    pool: &SqlitePool,
    user_id: &str,
    id: &str,
    status: BotStatus,
) -> ApiResult<BotConfig> {
    if status == BotStatus::Live {
        return Err(ApiError::BadRequest(
            "live promotion is not available in v1; use 'paper'".into(),
        ));
    }
    let now = Utc::now();
    let updated = sqlx::query(
        "UPDATE bot_configs SET status = ?1, updated_at = ?2 WHERE id = ?3 AND user_id = ?4",
    )
    .bind(status.as_str())
    .bind(now)
    .bind(id)
    .bind(user_id)
    .execute(pool)
    .await?;
    if updated.rows_affected() == 0 {
        return Err(ApiError::NotFound);
    }
    get(pool, user_id, id).await
}

pub async fn delete(pool: &SqlitePool, user_id: &str, id: &str) -> ApiResult<()> {
    let res = sqlx::query("DELETE FROM bot_configs WHERE id = ?1 AND user_id = ?2")
        .bind(id)
        .bind(user_id)
        .execute(pool)
        .await?;
    if res.rows_affected() == 0 {
        return Err(ApiError::NotFound);
    }
    Ok(())
}
