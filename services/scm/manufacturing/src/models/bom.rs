use serde::{Deserialize, Serialize};
use validator::Validate;

#[derive(Debug, Serialize, Deserialize, Validate)]
pub struct CreateBom {
    #[validate(length(min = 1, max = 200))]
    pub name: String,
    pub description: Option<String>,
    pub finished_item_id: String,
    pub quantity: Option<i64>,
    pub components: Vec<CreateBomComponent>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct CreateBomComponent {
    pub component_item_id: String,
    pub quantity_required: i64,
}

#[derive(Debug, Serialize, Deserialize, sqlx::FromRow)]
pub struct BomResponse {
    pub id: String,
    pub name: String,
    pub description: Option<String>,
    pub finished_item_id: String,
    pub quantity: i64,
    pub created_at: String,
}

#[derive(Debug, Serialize, Deserialize, sqlx::FromRow)]
pub struct BomComponentResponse {
    pub id: String,
    pub bom_id: String,
    pub component_item_id: String,
    pub quantity_required: i64,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct BomDetailResponse {
    #[serde(flatten)]
    pub bom: BomResponse,
    pub components: Vec<BomComponentResponse>,
}
