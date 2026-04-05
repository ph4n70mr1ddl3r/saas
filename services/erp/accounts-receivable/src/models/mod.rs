use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, sqlx::FromRow)]
pub struct Customer {
    pub id: String,
    pub name: String,
    pub email: Option<String>,
    pub phone: Option<String>,
    pub address: Option<String>,
    pub is_active: i64,
    pub created_at: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct CreateCustomerRequest {
    pub name: String,
    pub email: Option<String>,
    pub phone: Option<String>,
    pub address: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct UpdateCustomerRequest {
    pub name: Option<String>,
    pub email: Option<String>,
    pub phone: Option<String>,
    pub address: Option<String>,
    pub is_active: Option<bool>,
}

#[derive(Debug, Serialize, Deserialize, sqlx::FromRow)]
pub struct ArInvoice {
    pub id: String,
    pub customer_id: String,
    pub invoice_number: String,
    pub invoice_date: String,
    pub due_date: String,
    pub total_cents: i64,
    pub status: String,
    pub created_at: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct CreateArInvoiceRequest {
    pub customer_id: String,
    pub invoice_number: String,
    pub invoice_date: String,
    pub due_date: String,
    pub lines: Vec<CreateArInvoiceLineRequest>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct CreateArInvoiceLineRequest {
    pub description: Option<String>,
    pub amount_cents: i64,
}

#[derive(Debug, Serialize, Deserialize, sqlx::FromRow)]
pub struct ArInvoiceLine {
    pub id: String,
    pub invoice_id: String,
    pub description: Option<String>,
    pub amount_cents: i64,
}

#[derive(Debug, Serialize, Deserialize, sqlx::FromRow)]
pub struct Receipt {
    pub id: String,
    pub invoice_id: String,
    pub customer_id: String,
    pub amount_cents: i64,
    pub receipt_date: String,
    pub method: String,
    pub created_at: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct CreateReceiptRequest {
    pub invoice_id: String,
    pub customer_id: String,
    pub amount_cents: i64,
    pub receipt_date: String,
    pub method: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ArInvoiceWithLines {
    pub invoice: ArInvoice,
    pub lines: Vec<ArInvoiceLine>,
}
