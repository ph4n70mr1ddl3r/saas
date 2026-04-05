use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, sqlx::FromRow)]
pub struct CycleCountSession {
    pub id: String,
    pub warehouse_id: String,
    pub status: String,
    pub count_date: String,
    pub counted_by: String,
    pub approved_by: Option<String>,
    pub approved_at: Option<String>,
    pub created_at: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct CreateCycleCountSessionRequest {
    pub warehouse_id: String,
    pub count_date: String,
}

#[derive(Debug, Serialize, Deserialize, sqlx::FromRow)]
pub struct CycleCountLine {
    pub id: String,
    pub session_id: String,
    pub item_id: String,
    pub system_quantity: i64,
    pub counted_quantity: Option<i64>,
    pub variance: Option<i64>,
    pub notes: Option<String>,
    pub created_at: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct AddCycleCountLineRequest {
    pub item_id: String,
    pub notes: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct UpdateCountedQuantityRequest {
    pub counted_quantity: i64,
    pub notes: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct CycleCountSessionWithLines {
    pub session: CycleCountSession,
    pub lines: Vec<CycleCountLine>,
}
