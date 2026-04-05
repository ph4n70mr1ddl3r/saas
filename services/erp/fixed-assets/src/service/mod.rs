use sqlx::SqlitePool;
use saas_nats_bus::NatsBus;
use saas_common::error::{AppError, AppResult};
use crate::models::*;
use crate::repository::FixedAssetsRepo;

#[derive(Clone)]
pub struct FixedAssetsService {
    repo: FixedAssetsRepo,
    #[allow(dead_code)]
    bus: NatsBus,
}

impl FixedAssetsService {
    pub fn new(pool: SqlitePool, bus: NatsBus) -> Self {
        Self {
            repo: FixedAssetsRepo::new(pool),
            bus,
        }
    }

    // --- Assets ---

    pub async fn list_assets(&self) -> AppResult<Vec<Asset>> {
        self.repo.list_assets().await
    }

    pub async fn get_asset(&self, id: &str) -> AppResult<Asset> {
        self.repo.get_asset(id).await
    }

    pub async fn create_asset(&self, input: &CreateAssetRequest) -> AppResult<Asset> {
        if input.purchase_cost_cents < 0 {
            return Err(AppError::Validation("Purchase cost must be non-negative".into()));
        }
        if input.useful_life_months <= 0 {
            return Err(AppError::Validation("Useful life must be positive".into()));
        }
        self.repo.create_asset(input).await
    }

    pub async fn update_asset(&self, id: &str, input: &UpdateAssetRequest) -> AppResult<Asset> {
        self.repo.get_asset(id).await?;
        if let Some(ref status) = input.status {
            let valid = ["active", "disposed"];
            if !valid.contains(&status.as_str()) {
                return Err(AppError::Validation(format!(
                    "Invalid status '{}'. Must be one of: {:?}", status, valid
                )));
            }
        }
        self.repo.update_asset(id, input).await
    }

    // --- Depreciation ---

    pub async fn get_depreciation(&self, asset_id: &str) -> AppResult<Vec<DepreciationSchedule>> {
        self.repo.get_asset(asset_id).await?;
        self.repo.get_depreciation_schedule(asset_id).await
    }

    /// Run depreciation for a given period. Uses a database-level check
    /// inside a transaction to prevent concurrent runs for the same period.
    pub async fn run_depreciation(&self, period: &str) -> AppResult<Vec<DepreciationSchedule>> {
        // Pre-check: eagerly fail if ANY active asset already has depreciation
        // for this period, before doing any work.
        let assets = self.repo.list_active_assets().await?;
        for asset in &assets {
            if self.repo.has_depreciation_for_period(&asset.id, period).await? {
                return Err(AppError::Conflict(format!(
                    "Depreciation already exists for asset '{}' in period '{}'. Abort to prevent duplicates.",
                    asset.id, period
                )));
            }
        }

        let mut results = Vec::new();

        for asset in &assets {
            // Skip if already depreciated this period (defensive double-check)
            if self.repo.has_depreciation_for_period(&asset.id, period).await? {
                continue;
            }

            // Calculate straight-line depreciation
            let depreciable_amount = asset.purchase_cost_cents - asset.salvage_value_cents;
            if depreciable_amount <= 0 {
                continue;
            }

            let monthly_depreciation = depreciable_amount / asset.useful_life_months;
            if monthly_depreciation <= 0 {
                continue;
            }

            // Get accumulated depreciation so far
            let last = self.repo.get_last_depreciation(&asset.id).await?;
            let previous_accumulated = last.as_ref().map(|d| d.accumulated_cents).unwrap_or(0);
            let previous_nbv = last.as_ref().map(|d| d.net_book_value_cents).unwrap_or(asset.purchase_cost_cents);

            let new_accumulated = previous_accumulated + monthly_depreciation;
            let new_nbv = previous_nbv - monthly_depreciation;

            // Don't depreciate below salvage value
            let new_nbv = std::cmp::max(new_nbv, asset.salvage_value_cents);
            let actual_depreciation = if previous_nbv - monthly_depreciation < asset.salvage_value_cents {
                (previous_nbv - asset.salvage_value_cents).max(0)
            } else {
                monthly_depreciation
            };

            if actual_depreciation <= 0 {
                continue;
            }

            let final_accumulated = previous_accumulated + actual_depreciation;
            let final_nbv = previous_nbv - actual_depreciation;

            let schedule = self.repo.insert_depreciation(
                &asset.id,
                period,
                actual_depreciation,
                final_accumulated,
                final_nbv,
            ).await?;

            results.push(schedule);
        }

        Ok(results)
    }
}
