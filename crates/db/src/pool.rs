use sqlx::SqlitePool;
use sqlx::sqlite::SqlitePoolOptions;
use anyhow::Result;
use std::time::Duration;

pub async fn create_pool(database_url: &str) -> Result<SqlitePool> {
    let max_conns: u32 = std::env::var("DB_MAX_CONNECTIONS")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(5);
    let pool = SqlitePoolOptions::new()
        .max_connections(max_conns)
        .acquire_timeout(Duration::from_secs(5))
        .idle_timeout(Some(Duration::from_secs(600)))
        .max_lifetime(Some(Duration::from_secs(1800)))
        .connect(database_url)
        .await?;
    Ok(pool)
}
