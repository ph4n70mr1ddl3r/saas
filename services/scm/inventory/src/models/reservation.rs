use serde::{Deserialize, Serialize};
use validator::Validate;

#[derive(Debug, Serialize, Deserialize, Validate)]
pub struct CreateReservation {
    #[validate(length(min = 1))]
    pub item_id: String,
    #[validate(length(min = 1))]
    pub warehouse_id: String,
    #[validate(range(min = 1))]
    pub quantity: i64,
    #[validate(length(min = 1))]
    pub reference_type: String,
    #[validate(length(min = 1))]
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
