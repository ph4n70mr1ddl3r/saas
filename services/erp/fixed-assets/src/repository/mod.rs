use sqlx::SqlitePool;
use saas_common::error::{AppError, AppResult};
use crate::models::*;

#[derive(Clone)]
pub struct FixedAssetsRepo {
    pool: SqlitePool,
}

impl FixedAssetsRepo {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    // --- Assets ---

    pub async fn list_assets(&self) -> AppResult<Vec<Asset>> {
        let rows = sqlx::query_as::<_, Asset>(
            "SELECT id, name, description, asset_number, category, purchase_date, purchase_cost_cents, salvage_value_cents, useful_life_months, depreciation_method, status, created_at FROM assets ORDER BY asset_number",
        )
        .fetch_all(&self.pool)
        .await?;
        Ok(rows)
    }

    pub async fn get_asset(&self, id: &str) -> AppResult<Asset> {
        sqlx::query_as::<_, Asset>(
            "SELECT id, name, description, asset_number, category, purchase_date, purchase_cost_cents, salvage_value_cents, useful_life_months, depreciation_method, status, created_at FROM assets WHERE id = ?",
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("Asset '{}' not found", id)))
    }

    pub async fn create_asset(&self, input: &CreateAssetRequest) -> AppResult<Asset> {
        let id = uuid::Uuid::new_v4().to_string();
        let salvage_value_cents = input.salvage_value_cents.unwrap_or(0);
        let depreciation_method = input.depreciation_method.as_deref().unwrap_or("straight_line");

        sqlx::query(
            "INSERT INTO assets (id, name, description, asset_number, category, purchase_date, purchase_cost_cents, salvage_value_cents, useful_life_months, depreciation_method) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(&id)
        .bind(&input.name)
        .bind(&input.description)
        .bind(&input.asset_number)
        .bind(&input.category)
        .bind(&input.purchase_date)
        .bind(input.purchase_cost_cents)
        .bind(salvage_value_cents)
        .bind(input.useful_life_months)
        .bind(depreciation_method)
        .execute(&self.pool)
        .await?;
        self.get_asset(&id).await
    }

    pub async fn update_asset(&self, id: &str, input: &UpdateAssetRequest) -> AppResult<Asset> {
        let current = self.get_asset(id).await?;
        let name = input.name.as_deref().unwrap_or(&current.name);
        let description = input.description.as_deref().or(current.description.as_deref());
        let category = input.category.as_deref().unwrap_or(&current.category);
        let status = input.status.as_deref().unwrap_or(&current.status);

        sqlx::query(
            "UPDATE assets SET name = ?, description = ?, category = ?, status = ? WHERE id = ?",
        )
        .bind(name)
        .bind(description)
        .bind(category)
        .bind(status)
        .bind(id)
        .execute(&self.pool)
        .await?;
        self.get_asset(id).await
    }

    // --- Depreciation ---

    pub async fn get_depreciation_schedule(&self, asset_id: &str) -> AppResult<Vec<DepreciationSchedule>> {
        let rows = sqlx::query_as::<_, DepreciationSchedule>(
            "SELECT id, asset_id, period, depreciation_cents, accumulated_cents, net_book_value_cents FROM depreciation_schedule WHERE asset_id = ? ORDER BY period",
        )
        .bind(asset_id)
        .fetch_all(&self.pool)
        .await?;
        Ok(rows)
    }

    pub async fn get_last_depreciation(&self, asset_id: &str) -> AppResult<Option<DepreciationSchedule>> {
        let row = sqlx::query_as::<_, DepreciationSchedule>(
            "SELECT id, asset_id, period, depreciation_cents, accumulated_cents, net_book_value_cents FROM depreciation_schedule WHERE asset_id = ? ORDER BY period DESC LIMIT 1",
        )
        .bind(asset_id)
        .fetch_optional(&self.pool)
        .await?;
        Ok(row)
    }

    pub async fn has_depreciation_for_period(&self, asset_id: &str, period: &str) -> AppResult<bool> {
        let count: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM depreciation_schedule WHERE asset_id = ? AND period = ?",
        )
        .bind(asset_id)
        .bind(period)
        .fetch_one(&self.pool)
        .await?;
        Ok(count > 0)
    }

    pub async fn insert_depreciation(
        &self,
        asset_id: &str,
        period: &str,
        depreciation_cents: i64,
        accumulated_cents: i64,
        net_book_value_cents: i64,
    ) -> AppResult<DepreciationSchedule> {
        let id = uuid::Uuid::new_v4().to_string();
        sqlx::query(
            "INSERT INTO depreciation_schedule (id, asset_id, period, depreciation_cents, accumulated_cents, net_book_value_cents) VALUES (?, ?, ?, ?, ?, ?)",
        )
        .bind(&id)
        .bind(asset_id)
        .bind(period)
        .bind(depreciation_cents)
        .bind(accumulated_cents)
        .bind(net_book_value_cents)
        .execute(&self.pool)
        .await?;

        sqlx::query_as::<_, DepreciationSchedule>(
            "SELECT id, asset_id, period, depreciation_cents, accumulated_cents, net_book_value_cents FROM depreciation_schedule WHERE id = ?",
        )
        .bind(&id)
        .fetch_one(&self.pool)
        .await
        .map_err(|e| e.into())
    }

    pub async fn list_active_assets(&self) -> AppResult<Vec<Asset>> {
        let rows = sqlx::query_as::<_, Asset>(
            "SELECT id, name, description, asset_number, category, purchase_date, purchase_cost_cents, salvage_value_cents, useful_life_months, depreciation_method, status, created_at FROM assets WHERE status = 'active' ORDER BY asset_number",
        )
        .fetch_all(&self.pool)
        .await?;
        Ok(rows)
    }
}
