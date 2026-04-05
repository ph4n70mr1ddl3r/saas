use crate::models::supplier::{CreateSupplier, SupplierResponse, UpdateSupplier};
use saas_common::error::{AppError, AppResult};
use sqlx::SqlitePool;

#[derive(Clone)]
pub struct SupplierRepo {
    pool: SqlitePool,
}

impl SupplierRepo {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    pub async fn list(&self) -> AppResult<Vec<SupplierResponse>> {
        let rows = sqlx::query_as::<_, SupplierResponse>(
            "SELECT id, name, email, phone, address, is_active, created_at FROM suppliers ORDER BY name"
        ).fetch_all(&self.pool).await?;
        Ok(rows)
    }

    pub async fn get_by_id(&self, id: &str) -> AppResult<SupplierResponse> {
        sqlx::query_as::<_, SupplierResponse>(
            "SELECT id, name, email, phone, address, is_active, created_at FROM suppliers WHERE id = ?"
        )
            .bind(id)
            .fetch_optional(&self.pool).await?
            .ok_or_else(|| AppError::NotFound(format!("Supplier {} not found", id)))
    }

    pub async fn create(&self, input: &CreateSupplier) -> AppResult<SupplierResponse> {
        let id = uuid::Uuid::new_v4().to_string();
        sqlx::query(
            "INSERT INTO suppliers (id, name, email, phone, address) VALUES (?, ?, ?, ?, ?)",
        )
        .bind(&id)
        .bind(&input.name)
        .bind(&input.email)
        .bind(&input.phone)
        .bind(&input.address)
        .execute(&self.pool)
        .await?;
        self.get_by_id(&id).await
    }

    pub async fn update(&self, id: &str, input: &UpdateSupplier) -> AppResult<SupplierResponse> {
        let current = self.get_by_id(id).await?;
        sqlx::query(
            "UPDATE suppliers SET name=?, email=?, phone=?, address=?, is_active=? WHERE id=?",
        )
        .bind(input.name.as_deref().unwrap_or(&current.name))
        .bind(input.email.as_deref().or(current.email.as_deref()))
        .bind(input.phone.as_deref().or(current.phone.as_deref()))
        .bind(input.address.as_deref().or(current.address.as_deref()))
        .bind(input.is_active.unwrap_or(current.is_active))
        .bind(id)
        .execute(&self.pool)
        .await?;
        self.get_by_id(id).await
    }
}
