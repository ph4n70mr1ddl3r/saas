use crate::models::employment_history::*;
use saas_common::error::AppResult;
use sqlx::SqlitePool;

#[derive(Clone)]
pub struct EmploymentHistoryRepo {
    pool: SqlitePool,
}

impl EmploymentHistoryRepo {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    pub async fn list_by_employee(&self, employee_id: &str) -> AppResult<Vec<EmploymentHistory>> {
        let rows = sqlx::query_as::<_, EmploymentHistory>(
            "SELECT id, employee_id, field_name, old_value, new_value, effective_date, created_at FROM employment_history WHERE employee_id = ? ORDER BY effective_date DESC, created_at DESC"
        )
        .bind(employee_id)
        .fetch_all(&self.pool)
        .await?;
        Ok(rows)
    }

    pub async fn create(&self, input: &CreateEmploymentHistoryRequest) -> AppResult<EmploymentHistory> {
        let id = uuid::Uuid::new_v4().to_string();
        sqlx::query(
            "INSERT INTO employment_history (id, employee_id, field_name, old_value, new_value, effective_date) VALUES (?, ?, ?, ?, ?, ?)"
        )
        .bind(&id)
        .bind(&input.employee_id)
        .bind(&input.field_name)
        .bind(&input.old_value)
        .bind(&input.new_value)
        .bind(&input.effective_date)
        .execute(&self.pool)
        .await?;

        let row = sqlx::query_as::<_, EmploymentHistory>(
            "SELECT id, employee_id, field_name, old_value, new_value, effective_date, created_at FROM employment_history WHERE id = ?"
        )
        .bind(&id)
        .fetch_optional(&self.pool)
        .await?
        .ok_or_else(|| saas_common::error::AppError::Internal("Failed to read employment history".into()))?;
        Ok(row)
    }
}
