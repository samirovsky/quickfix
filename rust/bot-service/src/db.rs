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

const MIGRATIONS: &[(&str, &str)] = &[
    ("0001_init", include_str!("../migrations/0001_init.sql")),
    (
        "0002_marketplace",
        include_str!("../migrations/0002_marketplace.sql"),
    ),
];

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
    // sqlite executes one statement at a time through sqlx; the schema files
    // are multiple statements, so we strip line comments first (so leading
    // banners don't get attached to the first CREATE TABLE), then split on
    // ';' and execute each non-empty piece. Naive but adequate for the
    // hand-written migration files we have today; switch to a real
    // migration crate if anything in here ever grows quoted ';' literals.
    //
    // Each migration is idempotent (every statement is IF NOT EXISTS) so we
    // run the whole list on every boot without tracking which have been
    // applied. Revisit when the first non-idempotent migration shows up.
    for (name, sql) in MIGRATIONS {
        let stripped: String = sql
            .lines()
            .map(|line| match line.find("--") {
                Some(i) => &line[..i],
                None => line,
            })
            .collect::<Vec<_>>()
            .join("\n");
        for stmt in stripped.split(';') {
            let trimmed = stmt.trim();
            if trimmed.is_empty() {
                continue;
            }
            sqlx::query(trimmed)
                .execute(pool)
                .await
                .with_context(|| format!("migration {name} statement failed: {trimmed}"))?;
        }
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
