use serde::{Deserialize, Serialize};
use validator::Validate;

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

#[derive(Debug, Serialize, Deserialize, Validate)]
pub struct CreateCustomerRequest {
    #[validate(length(min = 1))]
    pub name: String,
    #[validate(email)]
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

#[derive(Debug, Serialize, Deserialize, Validate)]
pub struct CreateArInvoiceRequest {
    #[validate(length(min = 1))]
    pub customer_id: String,
    #[validate(length(min = 1))]
    pub invoice_number: String,
    pub invoice_date: String,
    pub due_date: String,
    #[validate(nested)]
    pub lines: Vec<CreateArInvoiceLineRequest>,
}

#[derive(Debug, Serialize, Deserialize, Validate)]
pub struct CreateArInvoiceLineRequest {
    pub description: Option<String>,
    #[validate(range(min = 1))]
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

#[derive(Debug, Serialize, Deserialize, Validate)]
pub struct CreateReceiptRequest {
    #[validate(length(min = 1))]
    pub invoice_id: String,
    #[validate(length(min = 1))]
    pub customer_id: String,
    #[validate(range(min = 1))]
    pub amount_cents: i64,
    pub receipt_date: String,
    pub method: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ArInvoiceWithLines {
    pub invoice: ArInvoice,
    pub lines: Vec<ArInvoiceLine>,
}

#[derive(Debug, Serialize, Deserialize, sqlx::FromRow)]
pub struct CreditMemo {
    pub id: String,
    pub customer_id: String,
    pub amount_cents: i64,
    pub reason: Option<String>,
    pub status: String,
    pub applied_to_invoice_id: Option<String>,
    pub applied_amount_cents: i64,
    pub created_at: String,
}

#[derive(Debug, Serialize, Deserialize, Validate)]
pub struct CreateCreditMemoRequest {
    #[validate(length(min = 1))]
    pub customer_id: String,
    #[validate(range(min = 1))]
    pub amount_cents: i64,
    pub reason: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ApplyCreditMemoRequest {
    pub invoice_id: String,
    pub amount_cents: i64,
}

#[derive(Debug, Serialize, Deserialize, sqlx::FromRow)]
pub struct ArAgingRow {
    pub customer_id: String,
    pub customer_name: String,
    pub invoice_id: String,
    pub invoice_number: String,
    pub total_cents: i64,
    pub due_date: String,
    pub aging_bucket: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ArAgingReport {
    pub current_total: i64,
    pub bucket_1_30_total: i64,
    pub bucket_31_60_total: i64,
    pub bucket_61_90_total: i64,
    pub bucket_90_plus_total: i64,
    pub invoices: Vec<ArAgingRow>,
}
