use serde::{Deserialize, Serialize};
use validator::Validate;

#[derive(Debug, Serialize, Deserialize, Validate)]
pub struct CreateWorkOrder {
    pub item_id: String,
    pub quantity: i64,
    pub planned_start: Option<String>,
    pub planned_end: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, sqlx::FromRow)]
pub struct WorkOrderResponse {
    pub id: String,
    pub wo_number: String,
    pub item_id: String,
    pub quantity: i64,
    pub status: String,
    pub planned_start: Option<String>,
    pub planned_end: Option<String>,
    pub actual_start: Option<String>,
    pub actual_end: Option<String>,
    pub created_at: String,
}
