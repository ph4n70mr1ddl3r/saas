use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, sqlx::FromRow)]
pub struct RoutingStepResponse {
    pub id: String,
    pub work_order_id: String,
    pub step_number: i32,
    pub description: Option<String>,
    pub status: String,
}
