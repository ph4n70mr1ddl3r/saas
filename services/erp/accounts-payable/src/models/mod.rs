use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, sqlx::FromRow)]
pub struct Vendor {
    pub id: String,
    pub name: String,
    pub email: Option<String>,
    pub phone: Option<String>,
    pub address: Option<String>,
    pub is_active: i64,
    pub created_at: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct CreateVendorRequest {
    pub name: String,
    pub email: Option<String>,
    pub phone: Option<String>,
    pub address: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct UpdateVendorRequest {
    pub name: Option<String>,
    pub email: Option<String>,
    pub phone: Option<String>,
    pub address: Option<String>,
    pub is_active: Option<bool>,
}

#[derive(Debug, Serialize, Deserialize, sqlx::FromRow)]
pub struct ApInvoice {
    pub id: String,
    pub vendor_id: String,
    pub invoice_number: String,
    pub invoice_date: String,
    pub due_date: String,
    pub total_cents: i64,
    pub tax_amount_cents: i64,
    pub status: String,
    pub created_at: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct CreateApInvoiceRequest {
    pub vendor_id: String,
    pub invoice_number: String,
    pub invoice_date: String,
    pub due_date: String,
    pub lines: Vec<CreateApInvoiceLineRequest>,
    pub tax_code: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct CreateApInvoiceLineRequest {
    pub description: Option<String>,
    pub account_code: String,
    pub amount_cents: i64,
}

#[derive(Debug, Serialize, Deserialize, sqlx::FromRow)]
pub struct ApInvoiceLine {
    pub id: String,
    pub invoice_id: String,
    pub description: Option<String>,
    pub account_code: String,
    pub amount_cents: i64,
}

#[derive(Debug, Serialize, Deserialize, sqlx::FromRow)]
pub struct Payment {
    pub id: String,
    pub invoice_id: String,
    pub vendor_id: String,
    pub amount_cents: i64,
    pub payment_date: String,
    pub method: String,
    pub reference: Option<String>,
    pub created_at: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct CreatePaymentRequest {
    pub invoice_id: String,
    pub vendor_id: String,
    pub amount_cents: i64,
    pub payment_date: String,
    pub method: Option<String>,
    pub reference: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ApInvoiceWithLines {
    pub invoice: ApInvoice,
    pub lines: Vec<ApInvoiceLine>,
}

#[derive(Debug, Serialize, Deserialize, sqlx::FromRow)]
pub struct TaxCode {
    pub id: String,
    pub code: String,
    pub rate: f64,
    pub description: Option<String>,
    pub is_active: i64,
    pub created_at: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct CreateTaxCodeRequest {
    pub code: String,
    pub rate: f64,
    pub description: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, sqlx::FromRow)]
pub struct ApAgingRow {
    pub vendor_id: String,
    pub vendor_name: String,
    pub invoice_id: String,
    pub invoice_number: String,
    pub total_cents: i64,
    pub due_date: String,
    pub aging_bucket: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ApAgingReport {
    pub current_total: i64,
    pub bucket_1_30_total: i64,
    pub bucket_31_60_total: i64,
    pub bucket_61_90_total: i64,
    pub bucket_90_plus_total: i64,
    pub invoices: Vec<ApAgingRow>,
}
