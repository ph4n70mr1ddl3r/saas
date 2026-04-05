use crate::models::bom::{BomComponentResponse, BomResponse, CreateBom};
use saas_common::error::{AppError, AppResult};
use sqlx::SqlitePool;

#[derive(Clone)]
pub struct BomRepo {
    pool: SqlitePool,
}

impl BomRepo {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    pub async fn list(&self) -> AppResult<Vec<BomResponse>> {
        let rows = sqlx::query_as::<_, BomResponse>(
            "SELECT id, name, description, finished_item_id, quantity, created_at FROM boms ORDER BY name"
        ).fetch_all(&self.pool).await?;
        Ok(rows)
    }

    pub async fn get_by_id(&self, id: &str) -> AppResult<BomResponse> {
        sqlx::query_as::<_, BomResponse>(
            "SELECT id, name, description, finished_item_id, quantity, created_at FROM boms WHERE id = ?"
        )
            .bind(id)
            .fetch_optional(&self.pool).await?
            .ok_or_else(|| AppError::NotFound(format!("BOM {} not found", id)))
    }

    pub async fn get_components(&self, bom_id: &str) -> AppResult<Vec<BomComponentResponse>> {
        let rows = sqlx::query_as::<_, BomComponentResponse>(
            "SELECT id, bom_id, component_item_id, quantity_required FROM bom_components WHERE bom_id = ?"
        )
        .bind(bom_id)
        .fetch_all(&self.pool).await?;
        Ok(rows)
    }

    pub async fn create(&self, input: &CreateBom) -> AppResult<BomResponse> {
        let id = uuid::Uuid::new_v4().to_string();
        let quantity = input.quantity.unwrap_or(1);

        let mut tx = self.pool.begin().await?;

        sqlx::query(
            "INSERT INTO boms (id, name, description, finished_item_id, quantity) VALUES (?, ?, ?, ?, ?)"
        )
        .bind(&id).bind(&input.name).bind(&input.description).bind(&input.finished_item_id).bind(quantity)
        .execute(&mut *tx).await?;

        for component in &input.components {
            let comp_id = uuid::Uuid::new_v4().to_string();
            sqlx::query(
                "INSERT INTO bom_components (id, bom_id, component_item_id, quantity_required) VALUES (?, ?, ?, ?)"
            )
            .bind(&comp_id).bind(&id).bind(&component.component_item_id).bind(component.quantity_required)
            .execute(&mut *tx).await?;
        }

        tx.commit().await?;
        self.get_by_id(&id).await
    }
}
