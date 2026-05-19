//! User provisioning. Slice 1 only uses this from the optional
//! `BOT_SERVICE_KEYS` startup seeder; runtime creation happens later
//! (admin tool, signup flow, whatever).

use chrono::Utc;
use sqlx::SqlitePool;
use uuid::Uuid;

use crate::auth::hash_key;
use crate::error::ApiResult;

#[derive(Debug, Clone)]
pub struct ProvisionedUser {
    pub id: String,
    pub name: String,
}

pub async fn upsert_with_key(
    pool: &SqlitePool,
    name: &str,
    api_key: &str,
) -> ApiResult<ProvisionedUser> {
    let hash = hash_key(api_key);
    if let Some(existing) = lookup_by_hash(pool, &hash).await? {
        return Ok(existing);
    }
    let id = format!("u_{}", Uuid::new_v4().simple());
    let now = Utc::now();
    sqlx::query("INSERT INTO users (id, name, api_key_hash, created_at) VALUES (?1, ?2, ?3, ?4)")
        .bind(&id)
        .bind(name)
        .bind(&hash)
        .bind(now)
        .execute(pool)
        .await?;
    Ok(ProvisionedUser {
        id,
        name: name.into(),
    })
}

async fn lookup_by_hash(pool: &SqlitePool, hash: &str) -> ApiResult<Option<ProvisionedUser>> {
    let row: Option<(String, String)> =
        sqlx::query_as("SELECT id, name FROM users WHERE api_key_hash = ?1")
            .bind(hash)
            .fetch_optional(pool)
            .await?;
    Ok(row.map(|(id, name)| ProvisionedUser { id, name }))
}
