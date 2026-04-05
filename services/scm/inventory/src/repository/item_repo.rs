use sqlx::SqlitePool;
use saas_common::error::{AppError, AppResult};
use crate::models::item::{ItemResponse, CreateItem, ItemFilters};

#[derive(Clone)]
pub struct ItemRepo { pool: SqlitePool }

impl ItemRepo {
    pub fn new(pool: SqlitePool) -> Self { Self { pool } }

    pub async fn list(&self, filters: &ItemFilters) -> AppResult<Vec<ItemResponse>> {
        let rows = sqlx::query_as::<_, ItemResponse>(
            "SELECT id, sku, name, description, unit_of_measure, item_type, is_active, created_at FROM items WHERE (? IS NULL OR item_type = ?) AND (? IS NULL OR is_active = ?) ORDER BY name"
        )
        .bind(&filters.item_type).bind(&filters.item_type)
        .bind(filters.is_active).bind(filters.is_active)
        .fetch_all(&self.pool).await?;
        Ok(rows)
    }

    pub async fn get_by_id(&self, id: &str) -> AppResult<ItemResponse> {
        sqlx::query_as::<_, ItemResponse>(
            "SELECT id, sku, name, description, unit_of_measure, item_type, is_active, created_at FROM items WHERE id = ?"
        )
            .bind(id)
            .fetch_optional(&self.pool).await?
            .ok_or_else(|| AppError::NotFound(format!("Item {} not found", id)))
    }

    pub async fn create(&self, input: &CreateItem) -> AppResult<ItemResponse> {
        let id = uuid::Uuid::new_v4().to_string();
        let uom = input.unit_of_measure.as_deref().unwrap_or("EA");
        sqlx::query(
            "INSERT INTO items (id, sku, name, description, unit_of_measure, item_type) VALUES (?, ?, ?, ?, ?, ?)"
        )
        .bind(&id).bind(&input.sku).bind(&input.name)
        .bind(&input.description).bind(uom).bind(&input.item_type)
        .execute(&self.pool).await?;
        self.get_by_id(&id).await
    }

    pub async fn update(&self, id: &str, name: Option<&str>, description: Option<&str>, unit_of_measure: Option<&str>, item_type: Option<&str>, is_active: Option<bool>) -> AppResult<ItemResponse> {
        let current = self.get_by_id(id).await?;
        sqlx::query(
            "UPDATE items SET name=?, description=?, unit_of_measure=?, item_type=?, is_active=? WHERE id=?"
        )
        .bind(name.unwrap_or(&current.name))
        .bind(description.or(current.description.as_deref()))
        .bind(unit_of_measure.unwrap_or(&current.unit_of_measure))
        .bind(item_type.unwrap_or(&current.item_type))
        .bind(is_active.unwrap_or(current.is_active))
        .bind(id)
        .execute(&self.pool).await?;
        self.get_by_id(id).await
    }
}
