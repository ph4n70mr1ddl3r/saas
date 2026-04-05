use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, sqlx::FromRow)]
pub struct BankAccount {
    pub id: String,
    pub name: String,
    pub bank_name: String,
    pub account_number: String,
    pub routing_number: Option<String>,
    pub balance_cents: i64,
    pub currency: String,
    pub created_at: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct CreateBankAccountRequest {
    pub name: String,
    pub bank_name: String,
    pub account_number: String,
    pub routing_number: Option<String>,
    pub balance_cents: Option<i64>,
    pub currency: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, sqlx::FromRow)]
pub struct BankTransaction {
    pub id: String,
    pub bank_account_id: String,
    pub amount_cents: i64,
    pub transaction_date: String,
    pub description: Option<String>,
    pub r#type: String,
    pub reference: Option<String>,
    pub created_at: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct CreateBankTransactionRequest {
    pub bank_account_id: String,
    pub amount_cents: i64,
    pub transaction_date: String,
    pub description: Option<String>,
    pub r#type: String,
    pub reference: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, sqlx::FromRow)]
pub struct Reconciliation {
    pub id: String,
    pub bank_account_id: String,
    pub period_start: String,
    pub period_end: String,
    pub statement_balance_cents: i64,
    pub book_balance_cents: i64,
    pub difference_cents: i64,
    pub status: String,
    pub created_at: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct CreateReconciliationRequest {
    pub bank_account_id: String,
    pub period_start: String,
    pub period_end: String,
    pub statement_balance_cents: i64,
}
