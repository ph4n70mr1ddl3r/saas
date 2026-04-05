use crate::models::department::{CreateDepartment, DepartmentResponse};
use saas_common::error::{AppError, AppResult};
use sqlx::SqlitePool;

#[derive(Clone)]
pub struct DepartmentRepo {
    pool: SqlitePool,
}

impl DepartmentRepo {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    pub async fn list(&self) -> AppResult<Vec<DepartmentResponse>> {
        let rows =
            sqlx::query_as::<_, DepartmentResponse>("SELECT * FROM departments ORDER BY name")
                .fetch_all(&self.pool)
                .await?;
        Ok(rows)
    }

    pub async fn get_by_id(&self, id: &str) -> AppResult<DepartmentResponse> {
        sqlx::query_as::<_, DepartmentResponse>("SELECT * FROM departments WHERE id = ?")
            .bind(id)
            .fetch_optional(&self.pool)
            .await?
            .ok_or_else(|| AppError::NotFound(format!("Department {} not found", id)))
    }

    pub async fn create(&self, input: &CreateDepartment) -> AppResult<DepartmentResponse> {
        let id = uuid::Uuid::new_v4().to_string();
        let now = chrono::Utc::now().to_rfc3339();
        sqlx::query(
            "INSERT INTO departments (id, name, parent_id, manager_id, cost_center, created_at, updated_at) VALUES (?, ?, ?, ?, ?, ?, ?)"
        ).bind(&id).bind(&input.name).bind(&input.parent_id).bind(&input.manager_id)
         .bind(&input.cost_center).bind(&now).bind(&now)
         .execute(&self.pool).await?;
        self.get_by_id(&id).await
    }

    pub async fn update(
        &self,
        id: &str,
        name: Option<&str>,
        parent_id: Option<&str>,
        manager_id: Option<&str>,
        cost_center: Option<&str>,
    ) -> AppResult<DepartmentResponse> {
        let current = self.get_by_id(id).await?;
        let now = chrono::Utc::now().to_rfc3339();
        sqlx::query("UPDATE departments SET name=?, parent_id=?, manager_id=?, cost_center=?, updated_at=? WHERE id=?")
            .bind(name.unwrap_or(&current.name))
            .bind(parent_id.or(current.parent_id.as_deref()))
            .bind(manager_id.or(current.manager_id.as_deref()))
            .bind(cost_center.or(current.cost_center.as_deref()))
            .bind(&now).bind(id)
            .execute(&self.pool).await?;
        self.get_by_id(id).await
    }
}
