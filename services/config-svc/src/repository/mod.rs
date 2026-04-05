use sqlx::SqlitePool;
use saas_common::error::{AppError, AppResult};
use crate::models::ConfigEntry;

#[derive(Clone)]
pub struct ConfigRepo {
    pool: SqlitePool,
}

impl ConfigRepo {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    pub async fn list(&self) -> AppResult<Vec<ConfigEntry>> {
        let entries = sqlx::query_as::<_, ConfigEntry>(
            "SELECT key, value, updated_at FROM config ORDER BY key"
        )
        .fetch_all(&self.pool)
        .await?;
        Ok(entries)
    }

    pub async fn get(&self, key: &str) -> AppResult<ConfigEntry> {
        sqlx::query_as::<_, ConfigEntry>(
            "SELECT key, value, updated_at FROM config WHERE key = ?"
        )
        .bind(key)
        .fetch_optional(&self.pool)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("Config key '{}' not found", key)))
    }

    pub async fn set(&self, key: &str, value: &str) -> AppResult<ConfigEntry> {
        let now = chrono::Utc::now().to_rfc3339();
        sqlx::query(
            "INSERT INTO config (key, value, updated_at) VALUES (?, ?, ?) ON CONFLICT(key) DO UPDATE SET value = ?, updated_at = ?"
        )
        .bind(key)
        .bind(value)
        .bind(&now)
        .bind(value)
        .bind(&now)
        .execute(&self.pool)
        .await?;

        self.get(key).await
    }
}
