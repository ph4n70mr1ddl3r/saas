use serde::{Deserialize, Serialize};
use validator::Validate;

#[derive(Debug, Serialize, Deserialize, Validate)]
pub struct CreateStockMovement {
    #[validate(length(min = 1))]
    pub item_id: String,
    pub from_warehouse_id: Option<String>,
    #[validate(length(min = 1))]
    pub to_warehouse_id: String,
    #[validate(range(min = 1))]
    pub quantity: i64,
    #[validate(length(min = 1))]
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
