use serde::{Deserialize, Serialize};
use validator::Validate;

#[derive(Debug, Serialize, Deserialize, Validate)]
pub struct CreateReservation {
    pub item_id: String,
    pub warehouse_id: String,
    pub quantity: i64,
    pub reference_type: String,
    pub reference_id: String,
}

#[derive(Debug, Serialize, Deserialize, sqlx::FromRow)]
pub struct ReservationResponse {
    pub id: String,
    pub item_id: String,
    pub warehouse_id: String,
    pub quantity: i64,
    pub reference_type: String,
    pub reference_id: String,
    pub status: String,
    pub created_at: String,
    pub fulfilled_at: Option<String>,
}
