use serde::{Deserialize, Serialize};
use validator::Validate;

#[derive(Debug, Serialize, Deserialize, Validate)]
pub struct CreateSalesOrder {
    pub customer_id: String,
    pub order_date: String,
    pub shipping_address: Option<String>,
    pub notes: Option<String>,
    pub lines: Vec<CreateSalesOrderLine>,
}

#[derive(Debug, Serialize, Deserialize, Validate)]
pub struct CreateSalesOrderLine {
    pub item_id: String,
    pub quantity: i64,
    pub unit_price_cents: i64,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct FulfillRequest {
    pub lines: Vec<FulfillLine>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct FulfillLine {
    pub order_line_id: String,
    pub warehouse_id: String,
    pub quantity: i64,
}

#[derive(Debug, Serialize, Deserialize, sqlx::FromRow)]
pub struct SalesOrderResponse {
    pub id: String,
    pub order_number: String,
    pub customer_id: String,
    pub order_date: String,
    pub status: String,
    pub total_cents: i64,
    pub shipping_address: Option<String>,
    pub notes: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Serialize, Deserialize, sqlx::FromRow)]
pub struct SalesOrderLineResponse {
    pub id: String,
    pub order_id: String,
    pub line_number: i32,
    pub item_id: String,
    pub quantity: i64,
    pub unit_price_cents: i64,
    pub line_total_cents: i64,
    pub status: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SalesOrderDetailResponse {
    #[serde(flatten)]
    pub order: SalesOrderResponse,
    pub lines: Vec<SalesOrderLineResponse>,
}
