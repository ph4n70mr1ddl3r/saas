use serde::{Deserialize, Serialize};
use validator::Validate;

#[derive(Debug, Serialize, Deserialize, Validate)]
pub struct CreatePurchaseOrder {
    pub supplier_id: String,
    pub order_date: String,
    pub lines: Vec<CreatePurchaseOrderLine>,
}

#[derive(Debug, Serialize, Deserialize, Validate)]
pub struct CreatePurchaseOrderLine {
    pub item_id: String,
    pub quantity: i64,
    pub unit_price_cents: i64,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ReceivePurchaseOrder {
    pub lines: Vec<ReceiveLine>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ReceiveLine {
    pub po_line_id: String,
    pub quantity_received: i64,
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
