use saas_common::error::AppResult;
use sqlx::SqlitePool;

#[derive(Clone)]
pub struct TokenRepo {
    pool: SqlitePool,
}

impl TokenRepo {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    pub async fn revoke_token(&self, jti: &str, user_id: &str, expires_at: &str) -> AppResult<()> {
        let now = chrono::Utc::now().to_rfc3339();
        sqlx::query(
            "INSERT OR IGNORE INTO revoked_tokens (jti, user_id, expires_at, revoked_at) VALUES (?, ?, ?, ?)",
        )
        .bind(jti)
        .bind(user_id)
        .bind(expires_at)
        .bind(&now)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn is_revoked(&self, jti: &str) -> AppResult<bool> {
        let count: (i64,) = sqlx::query_as(
            "SELECT COUNT(*) FROM revoked_tokens WHERE jti = ?",
        )
        .bind(jti)
        .fetch_one(&self.pool)
        .await?;
        Ok(count.0 > 0)
    }

    pub async fn cleanup_expired(&self) -> AppResult<u64> {
        let now = chrono::Utc::now().to_rfc3339();
        let result = sqlx::query("DELETE FROM revoked_tokens WHERE expires_at < ?")
            .bind(&now)
            .execute(&self.pool)
            .await?;
        Ok(result.rows_affected())
    }
}
