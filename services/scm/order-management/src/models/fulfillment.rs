use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, sqlx::FromRow)]
pub struct FulfillmentResponse {
    pub id: String,
    pub order_id: String,
    pub order_line_id: String,
    pub quantity: i64,
    pub warehouse_id: String,
    pub shipped_date: Option<String>,
    pub tracking_number: Option<String>,
    pub status: String,
}
