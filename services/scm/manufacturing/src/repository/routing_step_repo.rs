use sqlx::SqlitePool;
use saas_common::error::AppResult;
use crate::models::routing_step::RoutingStepResponse;

#[derive(Clone)]
pub struct RoutingStepRepo { pool: SqlitePool }

impl RoutingStepRepo {
    pub fn new(pool: SqlitePool) -> Self { Self { pool } }

    pub async fn list_by_work_order(&self, work_order_id: &str) -> AppResult<Vec<RoutingStepResponse>> {
        let rows = sqlx::query_as::<_, RoutingStepResponse>(
            "SELECT id, work_order_id, step_number, description, status FROM routing_steps WHERE work_order_id = ? ORDER BY step_number"
        )
        .bind(work_order_id)
        .fetch_all(&self.pool).await?;
        Ok(rows)
    }

    pub async fn create(&self, work_order_id: &str, step_number: i32, description: &str) -> AppResult<RoutingStepResponse> {
        let id = uuid::Uuid::new_v4().to_string();
        sqlx::query(
            "INSERT INTO routing_steps (id, work_order_id, step_number, description, status) VALUES (?, ?, ?, ?, 'pending')"
        )
        .bind(&id).bind(work_order_id).bind(step_number).bind(description)
        .execute(&self.pool).await?;
        let row = sqlx::query_as::<_, RoutingStepResponse>(
            "SELECT id, work_order_id, step_number, description, status FROM routing_steps WHERE id = ?"
        )
        .bind(&id)
        .fetch_optional(&self.pool).await?
        .ok_or_else(|| saas_common::error::AppError::Internal("Failed to fetch routing step".into()))?;
        Ok(row)
    }

    pub async fn update_status(&self, id: &str, status: &str) -> AppResult<()> {
        sqlx::query("UPDATE routing_steps SET status = ? WHERE id = ?")
            .bind(status).bind(id)
            .execute(&self.pool).await?;
        Ok(())
    }
}
