use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, sqlx::FromRow)]
pub struct GoodsReceiptResponse {
    pub id: String,
    pub po_id: String,
    pub po_line_id: String,
    pub quantity_received: i64,
    pub received_date: String,
    pub created_at: String,
}
