use crate::models::return_model::{CreateReturn, ReturnResponse};
use saas_common::error::{AppError, AppResult};
use sqlx::SqlitePool;

#[derive(Clone)]
pub struct ReturnRepo {
    pool: SqlitePool,
}

impl ReturnRepo {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    pub async fn list(&self) -> AppResult<Vec<ReturnResponse>> {
        let rows = sqlx::query_as::<_, ReturnResponse>(
            "SELECT id, order_id, order_line_id, quantity, reason, status, refund_amount_cents, created_at FROM returns ORDER BY created_at DESC"
        ).fetch_all(&self.pool).await?;
        Ok(rows)
    }

    pub async fn get_by_id(&self, id: &str) -> AppResult<ReturnResponse> {
        sqlx::query_as::<_, ReturnResponse>(
            "SELECT id, order_id, order_line_id, quantity, reason, status, refund_amount_cents, created_at FROM returns WHERE id = ?"
        )
            .bind(id)
            .fetch_optional(&self.pool).await?
            .ok_or_else(|| AppError::NotFound(format!("Return {} not found", id)))
    }

    pub async fn create(&self, input: &CreateReturn) -> AppResult<ReturnResponse> {
        let id = uuid::Uuid::new_v4().to_string();
        sqlx::query(
            "INSERT INTO returns (id, order_id, order_line_id, quantity, reason, status) VALUES (?, ?, ?, ?, ?, 'requested')"
        )
        .bind(&id).bind(&input.order_id).bind(&input.order_line_id)
        .bind(input.quantity).bind(&input.reason)
        .execute(&self.pool).await?;
        self.get_by_id(&id).await
    }

    pub async fn update_status(&self, id: &str, status: &str) -> AppResult<ReturnResponse> {
        sqlx::query("UPDATE returns SET status = ? WHERE id = ?")
            .bind(status)
            .bind(id)
            .execute(&self.pool)
            .await?;
        self.get_by_id(id).await
    }

    pub async fn update_refund_amount(&self, id: &str, refund_amount_cents: i64) -> AppResult<ReturnResponse> {
        sqlx::query("UPDATE returns SET refund_amount_cents = ? WHERE id = ?")
            .bind(refund_amount_cents)
            .bind(id)
            .execute(&self.pool)
            .await?;
        self.get_by_id(id).await
    }

    pub async fn list_by_order_line(&self, order_line_id: &str) -> AppResult<Vec<ReturnResponse>> {
        let rows = sqlx::query_as::<_, ReturnResponse>(
            "SELECT id, order_id, order_line_id, quantity, reason, status, refund_amount_cents, created_at FROM returns WHERE order_line_id = ? ORDER BY created_at DESC"
        )
            .bind(order_line_id)
            .fetch_all(&self.pool)
            .await?;
        Ok(rows)
    }
}
