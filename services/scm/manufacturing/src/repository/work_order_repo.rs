use crate::models::work_order::{CreateWorkOrder, WorkOrderResponse};
use saas_common::error::{AppError, AppResult};
use sqlx::SqlitePool;

#[derive(Clone)]
pub struct WorkOrderRepo {
    pool: SqlitePool,
}

impl WorkOrderRepo {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    pub async fn list(&self) -> AppResult<Vec<WorkOrderResponse>> {
        let rows = sqlx::query_as::<_, WorkOrderResponse>(
            "SELECT id, wo_number, item_id, quantity, status, planned_start, planned_end, actual_start, actual_end, created_at FROM work_orders ORDER BY created_at DESC"
        ).fetch_all(&self.pool).await?;
        Ok(rows)
    }

    pub async fn get_by_id(&self, id: &str) -> AppResult<WorkOrderResponse> {
        sqlx::query_as::<_, WorkOrderResponse>(
            "SELECT id, wo_number, item_id, quantity, status, planned_start, planned_end, actual_start, actual_end, created_at FROM work_orders WHERE id = ?"
        )
            .bind(id)
            .fetch_optional(&self.pool).await?
            .ok_or_else(|| AppError::NotFound(format!("Work order {} not found", id)))
    }

    pub async fn create(&self, input: &CreateWorkOrder) -> AppResult<WorkOrderResponse> {
        let id = uuid::Uuid::new_v4().to_string();
        let wo_number = format!("WO-{}-{}", chrono::Utc::now().format("%Y%m%d%H%M%S"), &id[..8]);
        sqlx::query(
            "INSERT INTO work_orders (id, wo_number, item_id, quantity, status, planned_start, planned_end) VALUES (?, ?, ?, ?, 'planned', ?, ?)"
        )
        .bind(&id).bind(&wo_number).bind(&input.item_id).bind(input.quantity)
        .bind(&input.planned_start).bind(&input.planned_end)
        .execute(&self.pool).await?;
        self.get_by_id(&id).await
    }

    pub async fn update_status(
        &self,
        id: &str,
        status: &str,
        actual_start: Option<&str>,
        actual_end: Option<&str>,
    ) -> AppResult<()> {
        let result = sqlx::query(
            "UPDATE work_orders SET status = ?, actual_start = COALESCE(?, actual_start), actual_end = COALESCE(?, actual_end) WHERE id = ?"
        )
        .bind(status).bind(actual_start).bind(actual_end).bind(id)
        .execute(&self.pool).await?;
        if result.rows_affected() == 0 {
            return Err(AppError::NotFound(format!("Work order {} not found", id)));
        }
        Ok(())
    }
}
