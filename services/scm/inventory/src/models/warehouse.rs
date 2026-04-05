use serde::{Deserialize, Serialize};
use validator::Validate;

#[derive(Debug, Serialize, Deserialize, Validate)]
pub struct CreateWarehouse {
    #[validate(length(min = 1, max = 200))]
    pub name: String,
    pub address: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct UpdateWarehouse {
    pub name: Option<String>,
    pub address: Option<String>,
    pub is_active: Option<bool>,
}

#[derive(Debug, Serialize, Deserialize, sqlx::FromRow)]
pub struct WarehouseResponse {
    pub id: String,
    pub name: String,
    pub address: Option<String>,
    pub is_active: bool,
    pub created_at: String,
}
