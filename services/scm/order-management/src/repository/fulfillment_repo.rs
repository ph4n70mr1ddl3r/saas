use crate::models::fulfillment::FulfillmentResponse;
use saas_common::error::AppResult;
use sqlx::SqlitePool;

#[derive(Clone)]
pub struct FulfillmentRepo {
    pool: SqlitePool,
}

impl FulfillmentRepo {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    pub async fn create(
        &self,
        order_id: &str,
        order_line_id: &str,
        quantity: i64,
        warehouse_id: &str,
    ) -> AppResult<FulfillmentResponse> {
        let id = uuid::Uuid::new_v4().to_string();
        sqlx::query(
            "INSERT INTO fulfillments (id, order_id, order_line_id, quantity, warehouse_id, status) VALUES (?, ?, ?, ?, ?, 'pending')"
        )
        .bind(&id).bind(order_id).bind(order_line_id).bind(quantity).bind(warehouse_id)
        .execute(&self.pool).await?;
        let row = sqlx::query_as::<_, FulfillmentResponse>(
            "SELECT id, order_id, order_line_id, quantity, warehouse_id, shipped_date, tracking_number, status FROM fulfillments WHERE id = ?"
        )
        .bind(&id)
        .fetch_optional(&self.pool).await?
        .ok_or_else(|| saas_common::error::AppError::Internal("Failed to fetch fulfillment".into()))?;
        Ok(row)
    }

    pub async fn update_status(&self, id: &str, status: &str) -> AppResult<()> {
        sqlx::query("UPDATE fulfillments SET status = ? WHERE id = ?")
            .bind(status)
            .bind(id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }
}
