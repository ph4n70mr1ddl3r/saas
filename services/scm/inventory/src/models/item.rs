use serde::{Deserialize, Serialize};
use validator::Validate;

#[derive(Debug, Serialize, Deserialize, Validate)]
pub struct CreateItem {
    #[validate(length(min = 1, max = 50))]
    pub sku: String,
    #[validate(length(min = 1, max = 200))]
    pub name: String,
    pub description: Option<String>,
    #[validate(length(min = 1, max = 10))]
    pub unit_of_measure: Option<String>,
    pub item_type: String,
    pub reorder_point: i64,
    pub safety_stock: i64,
    pub economic_order_qty: i64,
    pub unit_price_cents: Option<i64>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct UpdateItem {
    pub name: Option<String>,
    pub description: Option<String>,
    pub unit_of_measure: Option<String>,
    pub item_type: Option<String>,
    pub is_active: Option<bool>,
    pub unit_price_cents: Option<i64>,
}

#[derive(Debug, Serialize, Deserialize, sqlx::FromRow)]
pub struct ItemResponse {
    pub id: String,
    pub sku: String,
    pub name: String,
    pub description: Option<String>,
    pub unit_of_measure: String,
    pub item_type: String,
    pub is_active: bool,
    pub reorder_point: i64,
    pub safety_stock: i64,
    pub economic_order_qty: i64,
    pub unit_price_cents: i64,
    pub created_at: String,
}

#[derive(Debug, Deserialize)]
pub struct ItemFilters {
    pub item_type: Option<String>,
    pub is_active: Option<bool>,
}
