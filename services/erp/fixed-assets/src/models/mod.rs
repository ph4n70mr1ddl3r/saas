use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, sqlx::FromRow)]
pub struct Asset {
    pub id: String,
    pub name: String,
    pub description: Option<String>,
    pub asset_number: String,
    pub category: String,
    pub purchase_date: String,
    pub purchase_cost_cents: i64,
    pub salvage_value_cents: i64,
    pub useful_life_months: i64,
    pub depreciation_method: String,
    pub status: String,
    pub created_at: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct CreateAssetRequest {
    pub name: String,
    pub description: Option<String>,
    pub asset_number: String,
    pub category: String,
    pub purchase_date: String,
    pub purchase_cost_cents: i64,
    pub salvage_value_cents: Option<i64>,
    pub useful_life_months: i64,
    pub depreciation_method: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct UpdateAssetRequest {
    pub name: Option<String>,
    pub description: Option<String>,
    pub category: Option<String>,
    pub status: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, sqlx::FromRow)]
pub struct DepreciationSchedule {
    pub id: String,
    pub asset_id: String,
    pub period: String,
    pub depreciation_cents: i64,
    pub accumulated_cents: i64,
    pub net_book_value_cents: i64,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct RunDepreciationRequest {
    pub period: String,
}
