use crate::models::goods_receipt::GoodsReceiptResponse;
use saas_common::error::AppResult;
use sqlx::SqlitePool;

#[derive(Clone)]
pub struct GoodsReceiptRepo {
    pool: SqlitePool,
}

impl GoodsReceiptRepo {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    pub async fn create(
        &self,
        po_id: &str,
        po_line_id: &str,
        quantity_received: i64,
        received_date: &str,
    ) -> AppResult<GoodsReceiptResponse> {
        let id = uuid::Uuid::new_v4().to_string();
        sqlx::query(
            "INSERT INTO goods_receipts (id, po_id, po_line_id, quantity_received, received_date) VALUES (?, ?, ?, ?, ?)"
        )
        .bind(&id).bind(po_id).bind(po_line_id).bind(quantity_received).bind(received_date)
        .execute(&self.pool).await?;
        let row = sqlx::query_as::<_, GoodsReceiptResponse>(
            "SELECT id, po_id, po_line_id, quantity_received, received_date, created_at FROM goods_receipts WHERE id = ?"
        )
        .bind(&id)
        .fetch_optional(&self.pool).await?
        .ok_or_else(|| saas_common::error::AppError::Internal("Failed to fetch goods receipt".into()))?;
        Ok(row)
    }
}
