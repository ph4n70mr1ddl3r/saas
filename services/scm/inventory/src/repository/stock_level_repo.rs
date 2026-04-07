use crate::models::stock_level::StockLevelResponse;
use saas_common::error::{AppError, AppResult};
use sqlx::SqlitePool;

#[derive(Clone)]
pub struct StockLevelRepo {
    pool: SqlitePool,
}

impl StockLevelRepo {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    pub async fn get_by_item(&self, item_id: &str) -> AppResult<Vec<StockLevelResponse>> {
        let rows = sqlx::query_as::<_, StockLevelResponse>(
            "SELECT id, item_id, warehouse_id, quantity_on_hand, quantity_reserved, quantity_available, updated_at FROM stock_levels WHERE item_id = ?"
        )
        .bind(item_id)
        .fetch_all(&self.pool).await?;
        Ok(rows)
    }

    pub async fn get_by_item_warehouse(
        &self,
        item_id: &str,
        warehouse_id: &str,
    ) -> AppResult<Option<StockLevelResponse>> {
        let row = sqlx::query_as::<_, StockLevelResponse>(
            "SELECT id, item_id, warehouse_id, quantity_on_hand, quantity_reserved, quantity_available, updated_at FROM stock_levels WHERE item_id = ? AND warehouse_id = ?"
        )
        .bind(item_id).bind(warehouse_id)
        .fetch_optional(&self.pool).await?;
        Ok(row)
    }

    pub async fn upsert_receipt(
        &self,
        item_id: &str,
        warehouse_id: &str,
        quantity: i64,
    ) -> AppResult<StockLevelResponse> {
        let id = uuid::Uuid::new_v4().to_string();
        sqlx::query(
            "INSERT INTO stock_levels (id, item_id, warehouse_id, quantity_on_hand, quantity_reserved, quantity_available) VALUES (?, ?, ?, ?, 0, ?) ON CONFLICT(item_id, warehouse_id) DO UPDATE SET quantity_on_hand = quantity_on_hand + ?, quantity_available = quantity_on_hand + ? - quantity_reserved, updated_at = datetime('now')"
        )
        .bind(&id).bind(item_id).bind(warehouse_id).bind(quantity).bind(quantity)
        .bind(quantity).bind(quantity)
        .execute(&self.pool).await?;
        self.get_by_item_warehouse(item_id, warehouse_id)
            .await?
            .ok_or_else(|| AppError::Internal("Failed to upsert stock level".into()))
    }

    pub async fn reserve(
        &self,
        item_id: &str,
        warehouse_id: &str,
        quantity: i64,
    ) -> AppResult<StockLevelResponse> {
        let id = uuid::Uuid::new_v4().to_string();
        sqlx::query(
            "INSERT INTO stock_levels (id, item_id, warehouse_id, quantity_on_hand, quantity_reserved, quantity_available) VALUES (?, ?, ?, 0, ?, -?) ON CONFLICT(item_id, warehouse_id) DO UPDATE SET quantity_reserved = quantity_reserved + ?, quantity_available = quantity_on_hand - (quantity_reserved + ?), updated_at = datetime('now')"
        )
        .bind(&id).bind(item_id).bind(warehouse_id).bind(quantity).bind(quantity)
        .bind(quantity).bind(quantity)
        .execute(&self.pool).await?;
        self.get_by_item_warehouse(item_id, warehouse_id)
            .await?
            .ok_or_else(|| AppError::Internal("Failed to update reservation".into()))
    }

    pub async fn release_reservation(
        &self,
        item_id: &str,
        warehouse_id: &str,
        quantity: i64,
    ) -> AppResult<()> {
        sqlx::query(
            "UPDATE stock_levels SET quantity_reserved = quantity_reserved - ?, quantity_available = quantity_on_hand - (quantity_reserved - ?), updated_at = datetime('now') WHERE item_id = ? AND warehouse_id = ?"
        )
        .bind(quantity).bind(quantity).bind(item_id).bind(warehouse_id)
        .execute(&self.pool).await?;
        Ok(())
    }

    pub async fn deduct(
        &self,
        item_id: &str,
        warehouse_id: &str,
        quantity: i64,
    ) -> AppResult<StockLevelResponse> {
        sqlx::query(
            "UPDATE stock_levels SET quantity_on_hand = quantity_on_hand - ?, quantity_available = quantity_on_hand - ? - quantity_reserved, updated_at = datetime('now') WHERE item_id = ? AND warehouse_id = ? AND quantity_on_hand >= ?"
        )
        .bind(quantity).bind(quantity).bind(item_id).bind(warehouse_id).bind(quantity)
        .execute(&self.pool).await?;
        self.get_by_item_warehouse(item_id, warehouse_id)
            .await?
            .ok_or_else(|| AppError::Internal("Failed to deduct stock level".into()))
    }

    pub async fn get_first_warehouse_for_item(
        &self,
        item_id: &str,
    ) -> AppResult<Option<StockLevelResponse>> {
        let row = sqlx::query_as::<_, StockLevelResponse>(
            "SELECT id, item_id, warehouse_id, quantity_on_hand, quantity_reserved, quantity_available, updated_at FROM stock_levels WHERE item_id = ? LIMIT 1"
        )
        .bind(item_id)
        .fetch_optional(&self.pool).await?;
        Ok(row)
    }
}
