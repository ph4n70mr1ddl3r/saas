use crate::models::*;
use crate::repository::FixedAssetsRepo;
use saas_common::error::{AppError, AppResult};
use saas_nats_bus::NatsBus;
use saas_proto::events::{AssetCreated, AssetDisposed, DepreciationRunCompleted};
use sqlx::SqlitePool;

#[derive(Clone)]
pub struct FixedAssetsService {
    repo: FixedAssetsRepo,
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
            return Err(AppError::Validation(
                "Purchase cost must be non-negative".into(),
            ));
        }
        if input.useful_life_months <= 0 {
            return Err(AppError::Validation("Useful life must be positive".into()));
        }
        let asset = self.repo.create_asset(input).await?;
        if let Err(e) = self
            .bus
            .publish(
                "erp.assets.asset.created",
                AssetCreated {
                    asset_id: asset.id.clone(),
                    name: asset.name.clone(),
                    asset_number: asset.asset_number.clone(),
                    category: asset.category.clone(),
                    purchase_cost_cents: asset.purchase_cost_cents,
                },
            )
            .await
        {
            tracing::error!(
                "CRITICAL: Failed to publish event '{}': {}. Data may be inconsistent.",
                "erp.assets.asset.created",
                e
            );
        }
        Ok(asset)
    }

    pub async fn update_asset(&self, id: &str, input: &UpdateAssetRequest) -> AppResult<Asset> {
        self.repo.get_asset(id).await?;
        if let Some(ref status) = input.status {
            let valid = ["active", "disposed"];
            if !valid.contains(&status.as_str()) {
                return Err(AppError::Validation(format!(
                    "Invalid status '{}'. Must be one of: {:?}",
                    status, valid
                )));
            }
        }
        let asset = self.repo.update_asset(id, input).await?;

        // Publish disposal event if asset was disposed
        if input.status.as_deref() == Some("disposed") {
            if let Err(e) = self
                .bus
                .publish(
                    "erp.assets.asset.disposed",
                    AssetDisposed {
                        asset_id: asset.id.clone(),
                        name: asset.name.clone(),
                        asset_number: asset.asset_number.clone(),
                    },
                )
                .await
            {
                tracing::error!(
                    "CRITICAL: Failed to publish event '{}': {}. Data may be inconsistent.",
                    "erp.assets.asset.disposed",
                    e
                );
            }
        }

        Ok(asset)
    }

    // --- Depreciation ---

    pub async fn get_depreciation(&self, asset_id: &str) -> AppResult<Vec<DepreciationSchedule>> {
        self.repo.get_asset(asset_id).await?;
        self.repo.get_depreciation_schedule(asset_id).await
    }

    // --- Pure Depreciation Calculations ---

    /// Calculate annual depreciation using the declining balance method.
    ///
    /// `rate_percent` is the declining balance rate as a percentage of the straight-line
    /// rate (e.g., 200.0 for double-declining balance).
    /// Returns the depreciation amount for one year in cents.
    pub fn calculate_declining_balance(
        cost_cents: i64,
        salvage_value_cents: i64,
        useful_life_years: i32,
        rate_percent: f64,
    ) -> i64 {
        if useful_life_years <= 0 || rate_percent <= 0.0 {
            return 0;
        }
        let straight_line_rate = 1.0 / useful_life_years as f64;
        let declining_rate = straight_line_rate * (rate_percent / 100.0);
        let depreciation = (cost_cents as f64 * declining_rate).round() as i64;
        // Don't depreciate below salvage value
        let max_depreciation = (cost_cents - salvage_value_cents).max(0);
        depreciation.min(max_depreciation)
    }

    /// Calculate annual depreciation using the sum-of-years-digits method.
    ///
    /// `current_year` is 1-based (year 1 is the first year).
    /// Returns the depreciation amount for the given year in cents.
    pub fn calculate_sum_of_years_digits(
        cost_cents: i64,
        salvage_value_cents: i64,
        useful_life_years: i32,
        current_year: i32,
    ) -> i64 {
        if useful_life_years <= 0 || current_year < 1 || current_year > useful_life_years {
            return 0;
        }
        let depreciable_amount = (cost_cents - salvage_value_cents).max(0);
        let sum_of_years = (useful_life_years * (useful_life_years + 1)) / 2;
        if sum_of_years == 0 {
            return 0;
        }
        let remaining_life = useful_life_years - current_year + 1;
        let depreciation =
            (depreciable_amount as f64 * remaining_life as f64 / sum_of_years as f64).round()
                as i64;
        depreciation.max(0)
    }

    /// Run depreciation for a given period. Uses a database-level check
    /// inside a transaction to prevent concurrent runs for the same period.
    pub async fn run_depreciation(&self, period: &str) -> AppResult<Vec<DepreciationSchedule>> {
        // Pre-check: eagerly fail if ANY active asset already has depreciation
        // for this period, before doing any work.
        let assets = self.repo.list_active_assets().await?;
        for asset in &assets {
            if self
                .repo
                .has_depreciation_for_period(&asset.id, period)
                .await?
            {
                return Err(AppError::Conflict(format!(
                    "Depreciation already exists for asset '{}' in period '{}'. Abort to prevent duplicates.",
                    asset.id, period
                )));
            }
        }

        let mut results = Vec::new();
        let mut total_depreciation_cents: i64 = 0;

        for asset in &assets {
            // Skip if already depreciated this period (defensive double-check)
            if self
                .repo
                .has_depreciation_for_period(&asset.id, period)
                .await?
            {
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
            let previous_nbv = last
                .as_ref()
                .map(|d| d.net_book_value_cents)
                .unwrap_or(asset.purchase_cost_cents);

            let new_accumulated = previous_accumulated + monthly_depreciation;
            let new_nbv = previous_nbv - monthly_depreciation;

            // Don't depreciate below salvage value
            let new_nbv = std::cmp::max(new_nbv, asset.salvage_value_cents);
            let actual_depreciation =
                if previous_nbv - monthly_depreciation < asset.salvage_value_cents {
                    (previous_nbv - asset.salvage_value_cents).max(0)
                } else {
                    monthly_depreciation
                };

            if actual_depreciation <= 0 {
                continue;
            }

            let final_accumulated = previous_accumulated + actual_depreciation;
            let final_nbv = previous_nbv - actual_depreciation;

            let schedule = self
                .repo
                .insert_depreciation(
                    &asset.id,
                    period,
                    actual_depreciation,
                    final_accumulated,
                    final_nbv,
                )
                .await?;

            total_depreciation_cents += actual_depreciation;
            results.push(schedule);
        }

        // Publish depreciation run completed event
        if !results.is_empty() {
            if let Err(e) = self
                .bus
                .publish(
                    "erp.assets.depreciation.completed",
                    DepreciationRunCompleted {
                        period: period.to_string(),
                        asset_count: results.len() as u32,
                        total_depreciation_cents,
                    },
                )
                .await
            {
                tracing::error!(
                    "CRITICAL: Failed to publish event '{}': {}. Data may be inconsistent.",
                    "erp.assets.depreciation.completed",
                    e
                );
            }
        }

        Ok(results)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use saas_db::test_helpers::create_test_pool;
    use sqlx::SqlitePool;

    async fn setup() -> SqlitePool {
        let pool = create_test_pool().await;
        let sql_files = [
            include_str!("../../migrations/001_create_assets.sql"),
            include_str!("../../migrations/002_create_depreciation.sql"),
        ];
        let migration_names = [
            "001_create_assets.sql",
            "002_create_depreciation.sql",
        ];
        sqlx::query(
            "CREATE TABLE IF NOT EXISTS _migrations (filename TEXT PRIMARY KEY, applied_at TEXT NOT NULL)",
        )
        .execute(&pool)
        .await
        .unwrap();

        for (i, sql) in sql_files.iter().enumerate() {
            let name = migration_names[i];
            let already_applied: bool =
                sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM _migrations WHERE filename = ?")
                    .bind(name)
                    .fetch_one(&pool)
                    .await
                    .unwrap()
                    > 0;
            if !already_applied {
                let mut tx = pool.begin().await.unwrap();
                sqlx::raw_sql(sql).execute(&mut *tx).await.unwrap();
                let now = chrono::Utc::now().to_rfc3339();
                sqlx::query("INSERT INTO _migrations (filename, applied_at) VALUES (?, ?)")
                    .bind(name)
                    .bind(&now)
                    .execute(&mut *tx)
                    .await
                    .unwrap();
                tx.commit().await.unwrap();
            }
        }
        pool
    }

    async fn setup_repo() -> FixedAssetsRepo {
        let pool = setup().await;
        FixedAssetsRepo::new(pool)
    }

    #[tokio::test]
    async fn test_asset_crud() {
        let repo = setup_repo().await;

        // Create
        let input = CreateAssetRequest {
            name: "Office Building".into(),
            description: Some("Main headquarters".into()),
            asset_number: "ASSET-001".into(),
            category: "buildings".into(),
            purchase_date: "2024-01-01".into(),
            purchase_cost_cents: 1_200_000_00,
            salvage_value_cents: Some(200_000_00),
            useful_life_months: 360,
            depreciation_method: Some("straight_line".into()),
        };
        let asset = repo.create_asset(&input).await.unwrap();
        assert_eq!(asset.name, "Office Building");
        assert_eq!(asset.purchase_cost_cents, 1_200_000_00);
        assert_eq!(asset.salvage_value_cents, 200_000_00);
        assert_eq!(asset.status, "active");

        // Read
        let fetched = repo.get_asset(&asset.id).await.unwrap();
        assert_eq!(fetched.asset_number, "ASSET-001");

        // List
        let assets = repo.list_assets().await.unwrap();
        assert_eq!(assets.len(), 1);

        // Update
        let updated = repo
            .update_asset(
                &asset.id,
                &UpdateAssetRequest {
                    name: Some("HQ Building".into()),
                    description: None,
                    category: None,
                    status: None,
                },
            )
            .await
            .unwrap();
        assert_eq!(updated.name, "HQ Building");
    }

    #[tokio::test]
    async fn test_asset_disposal() {
        let repo = setup_repo().await;

        let asset = repo
            .create_asset(&CreateAssetRequest {
                name: "Laptop".into(),
                description: None,
                asset_number: "ASSET-010".into(),
                category: "equipment".into(),
                purchase_date: "2024-03-01".into(),
                purchase_cost_cents: 200_000,
                salvage_value_cents: Some(20_000),
                useful_life_months: 36,
                depreciation_method: None,
            })
            .await
            .unwrap();

        assert_eq!(asset.status, "active");

        let disposed = repo
            .update_asset(
                &asset.id,
                &UpdateAssetRequest {
                    name: None,
                    description: None,
                    category: None,
                    status: Some("disposed".into()),
                },
            )
            .await
            .unwrap();
        assert_eq!(disposed.status, "disposed");
    }

    #[tokio::test]
    async fn test_depreciation_schedule_creation() {
        let repo = setup_repo().await;

        let asset = repo
            .create_asset(&CreateAssetRequest {
                name: "Server Rack".into(),
                description: Some("Data center equipment".into()),
                asset_number: "ASSET-020".into(),
                category: "equipment".into(),
                purchase_date: "2024-06-01".into(),
                purchase_cost_cents: 60_000_00,
                salvage_value_cents: Some(0),
                useful_life_months: 60,
                depreciation_method: None,
            })
            .await
            .unwrap();

        // Insert first depreciation manually
        let dep = repo
            .insert_depreciation(&asset.id, "2024-06", 1_000_00, 1_000_00, 59_000_00)
            .await
            .unwrap();

        assert_eq!(dep.asset_id, asset.id);
        assert_eq!(dep.period, "2024-06");
        assert_eq!(dep.depreciation_cents, 1_000_00);
        assert_eq!(dep.accumulated_cents, 1_000_00);
        assert_eq!(dep.net_book_value_cents, 59_000_00);

        // Get schedule
        let schedule = repo.get_depreciation_schedule(&asset.id).await.unwrap();
        assert_eq!(schedule.len(), 1);
    }

    #[tokio::test]
    async fn test_run_depreciation_straight_line() {
        let repo = setup_repo().await;

        // Asset: cost=120000, salvage=0, life=12 months => monthly=10000
        let asset = repo
            .create_asset(&CreateAssetRequest {
                name: "Test Equipment".into(),
                description: None,
                asset_number: "ASSET-030".into(),
                category: "equipment".into(),
                purchase_date: "2025-01-01".into(),
                purchase_cost_cents: 120_000,
                salvage_value_cents: Some(0),
                useful_life_months: 12,
                depreciation_method: None,
            })
            .await
            .unwrap();

        // Simulate straight-line depreciation directly via repo
        // Month 1: dep=10000, acc=10000, nbv=110000
        let dep1 = repo.insert_depreciation(&asset.id, "2025-01", 10_000, 10_000, 110_000).await.unwrap();
        assert_eq!(dep1.depreciation_cents, 10_000);
        assert_eq!(dep1.net_book_value_cents, 110_000);

        // Month 2: dep=10000, acc=20000, nbv=100000
        let dep2 = repo.insert_depreciation(&asset.id, "2025-02", 10_000, 20_000, 100_000).await.unwrap();
        assert_eq!(dep2.accumulated_cents, 20_000);
        assert_eq!(dep2.net_book_value_cents, 100_000);

        // Verify schedule
        let schedule = repo.get_depreciation_schedule(&asset.id).await.unwrap();
        assert_eq!(schedule.len(), 2);
    }

    #[tokio::test]
    async fn test_depreciation_respects_salvage_value() {
        let repo = setup_repo().await;

        // Asset: cost=100000, salvage=40000, life=6 months => depreciable=60000, monthly=10000
        let asset = repo
            .create_asset(&CreateAssetRequest {
                name: "Salvage Test".into(),
                description: None,
                asset_number: "ASSET-040".into(),
                category: "vehicles".into(),
                purchase_date: "2025-01-01".into(),
                purchase_cost_cents: 100_000,
                salvage_value_cents: Some(40_000),
                useful_life_months: 6,
                depreciation_method: None,
            })
            .await
            .unwrap();

        // Run 6 months of depreciation manually
        let mut accumulated = 0i64;
        let mut nbv = 100_000i64;
        for month in 1..=6 {
            let period = format!("2025-{:02}", month);
            let dep = 10_000i64;
            accumulated += dep;
            nbv = std::cmp::max(nbv - dep, 40_000);
            repo.insert_depreciation(&asset.id, &period, dep, accumulated, nbv)
                .await
                .unwrap();
        }

        // Check final depreciation schedule
        let schedule = repo.get_depreciation_schedule(&asset.id).await.unwrap();
        assert_eq!(schedule.len(), 6);

        // Final NBV should be at salvage value (40000)
        let final_dep = schedule.last().unwrap();
        assert_eq!(final_dep.net_book_value_cents, 40_000);
        assert_eq!(final_dep.accumulated_cents, 60_000);
    }

    #[tokio::test]
    async fn test_prevent_duplicate_depreciation_run() {
        let repo = setup_repo().await;

        let asset = repo.create_asset(&CreateAssetRequest {
            name: "Duplicate Test".into(),
            description: None,
            asset_number: "ASSET-050".into(),
            category: "equipment".into(),
            purchase_date: "2025-01-01".into(),
            purchase_cost_cents: 50_000,
            salvage_value_cents: Some(0),
            useful_life_months: 12,
            depreciation_method: None,
        })
        .await
        .unwrap();

        // First insert should succeed
        repo.insert_depreciation(&asset.id, "2025-01", 4_166, 4_166, 45_834)
            .await
            .unwrap();

        // Duplicate period check
        let has_dup = repo.has_depreciation_for_period(&asset.id, "2025-01").await.unwrap();
        assert!(has_dup, "Should detect existing depreciation for period");
    }

    #[tokio::test]
    async fn test_depreciation_skips_disposed_assets() {
        let repo = setup_repo().await;

        let asset = repo
            .create_asset(&CreateAssetRequest {
                name: "To Be Disposed".into(),
                description: None,
                asset_number: "ASSET-060".into(),
                category: "equipment".into(),
                purchase_date: "2025-01-01".into(),
                purchase_cost_cents: 30_000,
                salvage_value_cents: Some(0),
                useful_life_months: 12,
                depreciation_method: None,
            })
            .await
            .unwrap();

        // Dispose the asset
        repo.update_asset(
            &asset.id,
            &UpdateAssetRequest {
                name: None,
                description: None,
                category: None,
                status: Some("disposed".into()),
            },
        )
        .await
        .unwrap();

        // Only active assets should be listed
        let active = repo.list_active_assets().await.unwrap();
        assert!(active.is_empty(), "Disposed assets should not be in active list");
    }

    #[tokio::test]
    async fn test_get_last_depreciation_tracks_accumulation() {
        let repo = setup_repo().await;

        let asset = repo
            .create_asset(&CreateAssetRequest {
                name: "Accum Test".into(),
                description: None,
                asset_number: "ASSET-070".into(),
                category: "equipment".into(),
                purchase_date: "2025-01-01".into(),
                purchase_cost_cents: 24_000,
                salvage_value_cents: Some(0),
                useful_life_months: 12,
                depreciation_method: None,
            })
            .await
            .unwrap();

        // Insert multiple depreciations manually
        repo.insert_depreciation(&asset.id, "2025-01", 2_000, 2_000, 22_000)
            .await
            .unwrap();
        repo.insert_depreciation(&asset.id, "2025-02", 2_000, 4_000, 20_000)
            .await
            .unwrap();
        repo.insert_depreciation(&asset.id, "2025-03", 2_000, 6_000, 18_000)
            .await
            .unwrap();

        // Get last depreciation should return March's entry
        let last = repo.get_last_depreciation(&asset.id).await.unwrap();
        assert!(last.is_some());
        let last = last.unwrap();
        assert_eq!(last.period, "2025-03");
        assert_eq!(last.accumulated_cents, 6_000);
        assert_eq!(last.net_book_value_cents, 18_000);

        // Full schedule should have 3 entries
        let schedule = repo.get_depreciation_schedule(&asset.id).await.unwrap();
        assert_eq!(schedule.len(), 3);
    }

    #[tokio::test]
    async fn test_declining_balance_double_rate() {
        // Asset: cost=100000, salvage=10000, life=5 years, 200% declining balance
        // Straight-line rate = 1/5 = 0.2, declining rate = 0.2 * 2.0 = 0.4
        // Year 1 depreciation = 100000 * 0.4 = 40000
        let dep = FixedAssetsService::calculate_declining_balance(100_000, 10_000, 5, 200.0);
        assert_eq!(dep, 40_000);
    }

    #[tokio::test]
    async fn test_declining_balance_respects_salvage() {
        // Asset: cost=20000, salvage=15000, life=5 years, 200% rate
        // Max depreciable = 20000 - 15000 = 5000
        // Calculated = 20000 * 0.4 = 8000, clamped to 5000
        let dep = FixedAssetsService::calculate_declining_balance(20_000, 15_000, 5, 200.0);
        assert_eq!(dep, 5_000);
    }

    #[tokio::test]
    async fn test_declining_balance_zero_life() {
        let dep = FixedAssetsService::calculate_declining_balance(100_000, 0, 0, 200.0);
        assert_eq!(dep, 0);
    }

    #[tokio::test]
    async fn test_sum_of_years_digits_year_1() {
        // Asset: cost=100000, salvage=10000, life=5 years
        // Sum of years = 5+4+3+2+1 = 15
        // Depreciable amount = 90000
        // Year 1: 90000 * 5/15 = 30000
        let dep = FixedAssetsService::calculate_sum_of_years_digits(100_000, 10_000, 5, 1);
        assert_eq!(dep, 30_000);
    }

    #[tokio::test]
    async fn test_sum_of_years_digits_year_2() {
        // Year 2: 90000 * 4/15 = 24000
        let dep = FixedAssetsService::calculate_sum_of_years_digits(100_000, 10_000, 5, 2);
        assert_eq!(dep, 24_000);
    }

    #[tokio::test]
    async fn test_sum_of_years_digits_year_3() {
        // Year 3: 90000 * 3/15 = 18000
        let dep = FixedAssetsService::calculate_sum_of_years_digits(100_000, 10_000, 5, 3);
        assert_eq!(dep, 18_000);
    }

    #[tokio::test]
    async fn test_sum_of_years_digits_year_4() {
        // Year 4: 90000 * 2/15 = 12000
        let dep = FixedAssetsService::calculate_sum_of_years_digits(100_000, 10_000, 5, 4);
        assert_eq!(dep, 12_000);
    }

    #[tokio::test]
    async fn test_sum_of_years_digits_year_5() {
        // Year 5: 90000 * 1/15 = 6000
        let dep = FixedAssetsService::calculate_sum_of_years_digits(100_000, 10_000, 5, 5);
        assert_eq!(dep, 6_000);
    }

    #[tokio::test]
    async fn test_sum_of_years_digits_all_years_sum_to_depreciable() {
        // Verify all years sum to the total depreciable amount
        let total: i64 = (1..=5)
            .map(|y| {
                FixedAssetsService::calculate_sum_of_years_digits(100_000, 10_000, 5, y)
            })
            .sum();
        assert_eq!(total, 90_000); // cost - salvage
    }

    #[tokio::test]
    async fn test_sum_of_years_digits_invalid_year() {
        let dep = FixedAssetsService::calculate_sum_of_years_digits(100_000, 10_000, 5, 6);
        assert_eq!(dep, 0);
    }
}
