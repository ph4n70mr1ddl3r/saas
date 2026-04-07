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

    pub async fn get_by_id(&self, id: &str) -> AppResult<GoodsReceiptResponse> {
        sqlx::query_as::<_, GoodsReceiptResponse>(
            "SELECT id, po_id, po_line_id, quantity_received, received_date, created_at FROM goods_receipts WHERE id = ?",
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await?
        .ok_or_else(|| saas_common::error::AppError::NotFound(format!("Goods receipt {} not found", id)))
    }

    pub async fn list_by_po(&self, po_id: &str) -> AppResult<Vec<GoodsReceiptResponse>> {
        let rows = sqlx::query_as::<_, GoodsReceiptResponse>(
            "SELECT id, po_id, po_line_id, quantity_received, received_date, created_at FROM goods_receipts WHERE po_id = ? ORDER BY created_at DESC",
        )
        .bind(po_id)
        .fetch_all(&self.pool)
        .await?;
        Ok(rows)
    }

    pub async fn list_all(&self) -> AppResult<Vec<GoodsReceiptResponse>> {
        let rows = sqlx::query_as::<_, GoodsReceiptResponse>(
            "SELECT id, po_id, po_line_id, quantity_received, received_date, created_at FROM goods_receipts ORDER BY created_at DESC",
        )
        .fetch_all(&self.pool)
        .await?;
        Ok(rows)
    }
}
