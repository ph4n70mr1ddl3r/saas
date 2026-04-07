use crate::models::item::{CreateItem, ItemFilters, ItemResponse, UpdateItem};
use saas_common::error::{AppError, AppResult};
use sqlx::SqlitePool;

#[derive(Clone)]
pub struct ItemRepo {
    pool: SqlitePool,
}

impl ItemRepo {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    pub async fn list(&self, filters: &ItemFilters) -> AppResult<Vec<ItemResponse>> {
        let rows = sqlx::query_as::<_, ItemResponse>(
            "SELECT id, sku, name, description, unit_of_measure, item_type, is_active, reorder_point, safety_stock, economic_order_qty, unit_price_cents, created_at FROM items WHERE (? IS NULL OR item_type = ?) AND (? IS NULL OR is_active = ?) ORDER BY name"
        )
        .bind(&filters.item_type).bind(&filters.item_type)
        .bind(filters.is_active).bind(filters.is_active)
        .fetch_all(&self.pool).await?;
        Ok(rows)
    }

    pub async fn get_by_id(&self, id: &str) -> AppResult<ItemResponse> {
        sqlx::query_as::<_, ItemResponse>(
            "SELECT id, sku, name, description, unit_of_measure, item_type, is_active, reorder_point, safety_stock, economic_order_qty, unit_price_cents, created_at FROM items WHERE id = ?"
        )
            .bind(id)
            .fetch_optional(&self.pool).await?
            .ok_or_else(|| AppError::NotFound(format!("Item {} not found", id)))
    }

    pub async fn create(&self, input: &CreateItem) -> AppResult<ItemResponse> {
        let id = uuid::Uuid::new_v4().to_string();
        let uom = input.unit_of_measure.as_deref().unwrap_or("EA");
        let unit_price = input.unit_price_cents.unwrap_or(0);
        sqlx::query(
            "INSERT INTO items (id, sku, name, description, unit_of_measure, item_type, reorder_point, safety_stock, economic_order_qty, unit_price_cents) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)"
        )
        .bind(&id).bind(&input.sku).bind(&input.name)
        .bind(&input.description).bind(uom).bind(&input.item_type)
        .bind(input.reorder_point).bind(input.safety_stock).bind(input.economic_order_qty)
        .bind(unit_price)
        .execute(&self.pool).await?;
        self.get_by_id(&id).await
    }

    pub async fn update_item(
        &self,
        id: &str,
        input: &UpdateItem,
    ) -> AppResult<ItemResponse> {
        let current = self.get_by_id(id).await?;
        let is_active = input.is_active.unwrap_or(current.is_active);
        sqlx::query(
            "UPDATE items SET name=?, description=?, unit_of_measure=?, item_type=?, is_active=?, unit_price_cents=? WHERE id=?"
        )
        .bind(input.name.as_deref().unwrap_or(&current.name))
        .bind(input.description.as_deref().or(current.description.as_deref()))
        .bind(input.unit_of_measure.as_deref().unwrap_or(&current.unit_of_measure))
        .bind(input.item_type.as_deref().unwrap_or(&current.item_type))
        .bind(is_active)
        .bind(input.unit_price_cents.unwrap_or(current.unit_price_cents))
        .bind(id)
        .execute(&self.pool).await?;
        self.get_by_id(id).await
    }

    pub async fn list_items_below_reorder_point(&self) -> AppResult<Vec<ItemResponse>> {
        let rows = sqlx::query_as::<_, ItemResponse>(
            "SELECT i.id, i.sku, i.name, i.description, i.unit_of_measure, i.item_type, i.is_active, i.reorder_point, i.safety_stock, i.economic_order_qty, i.unit_price_cents, i.created_at FROM items i INNER JOIN stock_levels sl ON i.id = sl.item_id WHERE i.reorder_point > 0 AND sl.quantity_on_hand <= i.reorder_point ORDER BY i.name"
        )
        .fetch_all(&self.pool).await?;
        Ok(rows)
    }
}
