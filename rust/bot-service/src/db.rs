//! Sqlite pool + migrations.
//!
//! Migrations are embedded as static strings rather than the `sqlx::migrate!`
//! macro so the build doesn't need a running database. For a single-file
//! migration this trades a hair of cleverness for a much simpler build.

use anyhow::{Context, Result};
use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
use sqlx::SqlitePool;
use std::path::Path;
use std::str::FromStr;

const SCHEMA: &str = include_str!("../migrations/0001_init.sql");

pub async fn connect(database_url: &str) -> Result<SqlitePool> {
    // `sqlite:./bot.db` or `sqlite::memory:`
    let opts = SqliteConnectOptions::from_str(database_url)?
        .create_if_missing(true)
        .foreign_keys(true);
    let pool = SqlitePoolOptions::new()
        .max_connections(5)
        .connect_with(opts)
        .await
        .with_context(|| format!("opening sqlite {database_url}"))?;
    apply_migrations(&pool).await?;
    Ok(pool)
}

async fn apply_migrations(pool: &SqlitePool) -> Result<()> {
    // sqlite executes one statement at a time through sqlx; the schema file
    // is multiple statements, so split on ';' and execute each non-empty
    // piece. Naive but adequate for a hand-written migration file with no
    // string literals containing semicolons.
    for stmt in SCHEMA.split(';') {
        let trimmed = stmt.trim();
        if trimmed.is_empty() || trimmed.starts_with("--") {
            continue;
        }
        sqlx::query(trimmed)
            .execute(pool)
            .await
            .with_context(|| format!("migration statement failed: {trimmed}"))?;
    }
    Ok(())
}

pub fn default_url(path: impl AsRef<Path>) -> String {
    let p = path.as_ref();
    if p.to_string_lossy().contains(':') {
        // Already a URL like "sqlite::memory:".
        p.to_string_lossy().into_owned()
    } else {
        format!("sqlite://{}", p.display())
    }
}
