use sqlx::SqlitePool;
use saas_common::error::{AppError, AppResult};
use crate::models::warehouse::{WarehouseResponse, CreateWarehouse, UpdateWarehouse};

#[derive(Clone)]
pub struct WarehouseRepo { pool: SqlitePool }

impl WarehouseRepo {
    pub fn new(pool: SqlitePool) -> Self { Self { pool } }

    pub async fn list(&self) -> AppResult<Vec<WarehouseResponse>> {
        let rows = sqlx::query_as::<_, WarehouseResponse>(
            "SELECT id, name, address, is_active, created_at FROM warehouses ORDER BY name"
        ).fetch_all(&self.pool).await?;
        Ok(rows)
    }

    pub async fn get_by_id(&self, id: &str) -> AppResult<WarehouseResponse> {
        sqlx::query_as::<_, WarehouseResponse>(
            "SELECT id, name, address, is_active, created_at FROM warehouses WHERE id = ?"
        )
            .bind(id)
            .fetch_optional(&self.pool).await?
            .ok_or_else(|| AppError::NotFound(format!("Warehouse {} not found", id)))
    }

    pub async fn create(&self, input: &CreateWarehouse) -> AppResult<WarehouseResponse> {
        let id = uuid::Uuid::new_v4().to_string();
        sqlx::query(
            "INSERT INTO warehouses (id, name, address) VALUES (?, ?, ?)"
        )
        .bind(&id).bind(&input.name).bind(&input.address)
        .execute(&self.pool).await?;
        self.get_by_id(&id).await
    }

    pub async fn update(&self, id: &str, input: &UpdateWarehouse) -> AppResult<WarehouseResponse> {
        let current = self.get_by_id(id).await?;
        sqlx::query(
            "UPDATE warehouses SET name=?, address=?, is_active=? WHERE id=?"
        )
        .bind(input.name.as_deref().unwrap_or(&current.name))
        .bind(input.address.as_deref().or(current.address.as_deref()))
        .bind(input.is_active.unwrap_or(current.is_active))
        .bind(id)
        .execute(&self.pool).await?;
        self.get_by_id(id).await
    }
}
