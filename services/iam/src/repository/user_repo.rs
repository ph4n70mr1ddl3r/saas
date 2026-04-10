use crate::models::user::{CreateUser, UserListRow, UserRow};
use saas_common::error::{AppError, AppResult};
use saas_common::pagination::PaginationParams;
use sqlx::SqlitePool;

#[derive(Clone)]
pub struct UserRepo {
    pool: SqlitePool,
}

impl UserRepo {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    /// List users with password hashes. Private — prefer `list_safe()` for any
    /// handler-facing usage to avoid leaking password hashes.
    async fn list(&self, pag: &PaginationParams) -> AppResult<(Vec<UserRow>, u64)> {
        let offset = pag.offset();
        let limit = pag.per_page();
        let rows = sqlx::query_as::<_, UserRow>(
            "SELECT id, username, email, password_hash, display_name, is_active, created_at, updated_at FROM users ORDER BY username LIMIT ? OFFSET ?"
        )
        .bind(limit)
        .bind(offset)
        .fetch_all(&self.pool)
        .await?;

        let count: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM users")
            .fetch_one(&self.pool)
            .await?;

        Ok((rows, count.0 as u64))
    }

    pub async fn list_safe(&self, pag: &PaginationParams) -> AppResult<(Vec<UserListRow>, u64)> {
        let offset = pag.offset();
        let limit = pag.per_page();
        let rows = sqlx::query_as::<_, UserListRow>(
            "SELECT id, username, email, display_name, is_active, created_at, updated_at FROM users ORDER BY username LIMIT ? OFFSET ?"
        )
        .bind(limit)
        .bind(offset)
        .fetch_all(&self.pool)
        .await?;

        let count: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM users")
            .fetch_one(&self.pool)
            .await?;

        Ok((rows, count.0 as u64))
    }

    pub async fn get_by_id(&self, id: &str) -> AppResult<UserRow> {
        sqlx::query_as::<_, UserRow>(
            "SELECT id, username, email, password_hash, display_name, is_active, created_at, updated_at FROM users WHERE id = ?"
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await?
        .ok_or_else(|| AppError::NotFound("User not found".into()))
    }

    pub async fn get_by_username(&self, username: &str) -> AppResult<UserRow> {
        sqlx::query_as::<_, UserRow>(
            "SELECT id, username, email, password_hash, display_name, is_active, created_at, updated_at FROM users WHERE username = ?"
        )
        .bind(username)
        .fetch_optional(&self.pool)
        .await?
        .ok_or_else(|| AppError::NotFound("User not found".into()))
    }

    pub async fn create(&self, input: &CreateUser, password_hash: &str) -> AppResult<UserRow> {
        let id = uuid::Uuid::new_v4().to_string();
        let now = chrono::Utc::now().to_rfc3339();
        sqlx::query(
            "INSERT INTO users (id, username, email, password_hash, display_name, is_active, created_at, updated_at) VALUES (?, ?, ?, ?, ?, 1, ?, ?)"
        )
        .bind(&id)
        .bind(&input.username)
        .bind(&input.email)
        .bind(password_hash)
        .bind(&input.display_name)
        .bind(&now)
        .bind(&now)
        .execute(&self.pool)
        .await?;

        self.get_by_id(&id).await
    }

    pub async fn update(
        &self,
        id: &str,
        email: Option<&str>,
        display_name: Option<&str>,
        is_active: Option<bool>,
    ) -> AppResult<UserRow> {
        let now = chrono::Utc::now().to_rfc3339();
        // Use COALESCE for atomic single-statement update (no read-then-write race)
        sqlx::query("UPDATE users SET email = COALESCE(?, email), display_name = COALESCE(?, display_name), is_active = COALESCE(?, is_active), updated_at = ? WHERE id = ?")
            .bind(email)
            .bind(display_name)
            .bind(is_active.map(|b| b as i32))
            .bind(&now)
            .bind(id)
            .execute(&self.pool)
            .await?;

        self.get_by_id(id).await
    }

    pub async fn update_password(&self, id: &str, password_hash: &str) -> AppResult<()> {
        let now = chrono::Utc::now().to_rfc3339();
        sqlx::query("UPDATE users SET password_hash = ?, updated_at = ? WHERE id = ?")
            .bind(password_hash)
            .bind(&now)
            .bind(id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    pub async fn soft_delete(&self, id: &str) -> AppResult<()> {
        let now = chrono::Utc::now().to_rfc3339();
        sqlx::query("UPDATE users SET is_active = 0, updated_at = ? WHERE id = ?")
            .bind(&now)
            .bind(id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    pub async fn get_user_roles(&self, user_id: &str) -> AppResult<Vec<String>> {
        let rows: Vec<(String,)> = sqlx::query_as(
            "SELECT r.name FROM roles r JOIN user_roles ur ON r.id = ur.role_id WHERE ur.user_id = ?"
        )
        .bind(user_id)
        .fetch_all(&self.pool)
        .await?;

        Ok(rows.into_iter().map(|r| r.0).collect())
    }

    /// Atomically replace all role assignments for a user inside a transaction.
    pub async fn set_user_roles(&self, user_id: &str, role_ids: &[String]) -> AppResult<()> {
        let mut tx = self.pool.begin().await?;

        sqlx::query("DELETE FROM user_roles WHERE user_id = ?")
            .bind(user_id)
            .execute(&mut *tx)
            .await?;

        for role_id in role_ids {
            sqlx::query("INSERT INTO user_roles (user_id, role_id) VALUES (?, ?)")
                .bind(user_id)
                .bind(role_id)
                .execute(&mut *tx)
                .await?;
        }

        tx.commit().await?;
        Ok(())
    }
}
