use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, sqlx::FromRow)]
pub struct StockLevelResponse {
    pub id: String,
    pub item_id: String,
    pub warehouse_id: String,
    pub quantity_on_hand: i64,
    pub quantity_reserved: i64,
    pub quantity_available: i64,
    pub updated_at: String,
}
