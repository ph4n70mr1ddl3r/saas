use serde::{Deserialize, Serialize};
use validator::Validate;

#[derive(Debug, Serialize, Deserialize, Validate)]
pub struct CreateStockMovement {
    pub item_id: String,
    pub from_warehouse_id: Option<String>,
    pub to_warehouse_id: String,
    pub quantity: i64,
    pub movement_type: String,
    pub reference_type: Option<String>,
    pub reference_id: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, sqlx::FromRow)]
pub struct StockMovementResponse {
    pub id: String,
    pub item_id: String,
    pub from_warehouse_id: Option<String>,
    pub to_warehouse_id: String,
    pub quantity: i64,
    pub movement_type: String,
    pub reference_type: Option<String>,
    pub reference_id: Option<String>,
    pub created_at: String,
}
