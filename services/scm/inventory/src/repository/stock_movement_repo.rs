use sqlx::SqlitePool;
use saas_common::error::AppResult;
use crate::models::stock_movement::{StockMovementResponse, CreateStockMovement};

#[derive(Clone)]
pub struct StockMovementRepo { pool: SqlitePool }

impl StockMovementRepo {
    pub fn new(pool: SqlitePool) -> Self { Self { pool } }

    pub async fn list(&self) -> AppResult<Vec<StockMovementResponse>> {
        let rows = sqlx::query_as::<_, StockMovementResponse>(
            "SELECT id, item_id, from_warehouse_id, to_warehouse_id, quantity, movement_type, reference_type, reference_id, created_at FROM stock_movements ORDER BY created_at DESC"
        ).fetch_all(&self.pool).await?;
        Ok(rows)
    }

    pub async fn create(&self, input: &CreateStockMovement) -> AppResult<StockMovementResponse> {
        let id = uuid::Uuid::new_v4().to_string();
        sqlx::query(
            "INSERT INTO stock_movements (id, item_id, from_warehouse_id, to_warehouse_id, quantity, movement_type, reference_type, reference_id) VALUES (?, ?, ?, ?, ?, ?, ?, ?)"
        )
        .bind(&id).bind(&input.item_id).bind(&input.from_warehouse_id)
        .bind(&input.to_warehouse_id).bind(input.quantity).bind(&input.movement_type)
        .bind(&input.reference_type).bind(&input.reference_id)
        .execute(&self.pool).await?;
        let row = sqlx::query_as::<_, StockMovementResponse>(
            "SELECT id, item_id, from_warehouse_id, to_warehouse_id, quantity, movement_type, reference_type, reference_id, created_at FROM stock_movements WHERE id = ?"
        )
        .bind(&id)
        .fetch_optional(&self.pool).await?
        .ok_or_else(|| saas_common::error::AppError::Internal("Failed to fetch stock movement".into()))?;
        Ok(row)
    }
}
