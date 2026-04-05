use serde::{Deserialize, Serialize};
use validator::Validate;

#[derive(Debug, Serialize, Deserialize, Validate)]
pub struct CreatePurchaseOrder {
    #[validate(length(min = 1))]
    pub supplier_id: String,
    #[validate(length(min = 1))]
    pub order_date: String,
    pub lines: Vec<CreatePurchaseOrderLine>,
}

#[derive(Debug, Serialize, Deserialize, Validate)]
pub struct CreatePurchaseOrderLine {
    #[validate(length(min = 1))]
    pub item_id: String,
    #[validate(range(min = 1))]
    pub quantity: i64,
    #[validate(range(min = 0))]
    pub unit_price_cents: i64,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ReceivePurchaseOrder {
    pub lines: Vec<ReceiveLine>,
}

#[derive(Debug, Serialize, Deserialize, Validate)]
pub struct ReceiveLine {
    #[validate(length(min = 1))]
    pub po_line_id: String,
    #[validate(range(min = 1))]
    pub quantity_received: i64,
    #[validate(length(min = 1))]
    pub warehouse_id: String,
}

#[derive(Debug, Serialize, Deserialize, sqlx::FromRow)]
pub struct PurchaseOrderResponse {
    pub id: String,
    pub po_number: String,
    pub supplier_id: String,
    pub order_date: String,
    pub status: String,
    pub total_cents: i64,
    pub created_at: String,
}

#[derive(Debug, Serialize, Deserialize, sqlx::FromRow)]
pub struct PurchaseOrderLineResponse {
    pub id: String,
    pub po_id: String,
    pub line_number: i32,
    pub item_id: String,
    pub quantity: i64,
    pub unit_price_cents: i64,
    pub line_total_cents: i64,
    pub quantity_received: i64,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct PurchaseOrderDetailResponse {
    #[serde(flatten)]
    pub order: PurchaseOrderResponse,
    pub lines: Vec<PurchaseOrderLineResponse>,
}
