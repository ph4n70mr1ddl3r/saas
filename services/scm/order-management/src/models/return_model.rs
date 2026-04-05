use serde::{Deserialize, Serialize};
use validator::Validate;

#[derive(Debug, Serialize, Deserialize, Validate)]
pub struct CreateReturn {
    #[validate(length(min = 1))]
    pub order_id: String,
    #[validate(length(min = 1))]
    pub order_line_id: String,
    #[validate(range(min = 1))]
    pub quantity: i64,
    pub reason: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, sqlx::FromRow)]
pub struct ReturnResponse {
    pub id: String,
    pub order_id: String,
    pub order_line_id: String,
    pub quantity: i64,
    pub reason: Option<String>,
    pub status: String,
    pub refund_amount_cents: Option<i64>,
    pub created_at: String,
}
