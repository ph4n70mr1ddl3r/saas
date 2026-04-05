use sqlx::SqlitePool;
use saas_common::error::{AppError, AppResult};
use crate::models::role::{RoleResponse, PermissionResponse};

#[derive(Clone)]
pub struct RoleRepo {
    pool: SqlitePool,
}

impl RoleRepo {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    pub async fn list_roles(&self) -> AppResult<Vec<RoleResponse>> {
        let roles = sqlx::query_as::<_, RoleResponse>(
            "SELECT id, name, description, created_at FROM roles ORDER BY name"
        )
        .fetch_all(&self.pool)
        .await?;
        Ok(roles)
    }

    pub async fn get_role(&self, id: &str) -> AppResult<RoleResponse> {
        sqlx::query_as::<_, RoleResponse>(
            "SELECT id, name, description, created_at FROM roles WHERE id = ?"
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await?
        .ok_or_else(|| AppError::NotFound("Role not found".into()))
    }

    pub async fn create_role(&self, name: &str, description: Option<&str>) -> AppResult<RoleResponse> {
        let id = uuid::Uuid::new_v4().to_string();
        let now = chrono::Utc::now().to_rfc3339();
        sqlx::query("INSERT INTO roles (id, name, description, created_at) VALUES (?, ?, ?, ?)")
            .bind(&id)
            .bind(name)
            .bind(description)
            .bind(&now)
            .execute(&self.pool)
            .await?;
        self.get_role(&id).await
    }

    pub async fn update_role(&self, id: &str, name: Option<&str>, description: Option<&str>) -> AppResult<RoleResponse> {
        // Use COALESCE for atomic single-statement update
        sqlx::query("UPDATE roles SET name = COALESCE(?, name), description = COALESCE(?, description) WHERE id = ?")
            .bind(name)
            .bind(description)
            .bind(id)
            .execute(&self.pool)
            .await?;
        self.get_role(id).await
    }

    pub async fn list_permissions(&self) -> AppResult<Vec<PermissionResponse>> {
        let perms = sqlx::query_as::<_, PermissionResponse>(
            "SELECT id, code, resource, action, description FROM permissions ORDER BY resource, action"
        )
        .fetch_all(&self.pool)
        .await?;
        Ok(perms)
    }

    /// Atomically replace all permission assignments for a role inside a transaction.
    pub async fn set_role_permissions(&self, role_id: &str, permission_ids: &[String]) -> AppResult<()> {
        let mut tx = self.pool.begin().await?;

        sqlx::query("DELETE FROM role_permissions WHERE role_id = ?")
            .bind(role_id)
            .execute(&mut *tx)
            .await?;

        for perm_id in permission_ids {
            sqlx::query("INSERT INTO role_permissions (role_id, permission_id) VALUES (?, ?)")
                .bind(role_id)
                .bind(perm_id)
                .execute(&mut *tx)
                .await?;
        }

        tx.commit().await?;
        Ok(())
    }

    pub async fn get_role_permissions(&self, role_id: &str) -> AppResult<Vec<String>> {
        let rows: Vec<(String,)> = sqlx::query_as(
            "SELECT p.code FROM permissions p JOIN role_permissions rp ON p.id = rp.permission_id WHERE rp.role_id = ?"
        )
        .bind(role_id)
        .fetch_all(&self.pool)
        .await?;
        Ok(rows.into_iter().map(|r| r.0).collect())
    }
}
