use serde::{Deserialize, Serialize};
use validator::Validate;

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

#[derive(Debug, Serialize, Deserialize, Validate)]
pub struct CreateBankAccountRequest {
    #[validate(length(min = 1))]
    pub name: String,
    #[validate(length(min = 1))]
    pub bank_name: String,
    pub account_number: String,
    pub routing_number: Option<String>,
    pub balance_cents: Option<i64>,
    #[validate(length(min = 3, max = 3, message = "Currency must be 3-letter ISO code"))]
    pub currency: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct UpdateBankAccountRequest {
    pub name: Option<String>,
    pub bank_name: Option<String>,
    pub account_number: Option<String>,
    pub routing_number: Option<String>,
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

#[derive(Debug, Serialize, Deserialize, Validate)]
pub struct CreateBankTransactionRequest {
    #[validate(length(min = 1))]
    pub bank_account_id: String,
    #[validate(range(min = 1, message = "Amount must be positive"))]
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

#[derive(Debug, Serialize, Deserialize, sqlx::FromRow)]
pub struct CashFlowRow {
    pub category: String,
    pub description: Option<String>,
    pub amount_cents: i64,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct CashFlowStatement {
    pub operating: Vec<CashFlowRow>,
    pub total_operating_cents: i64,
    pub investing: Vec<CashFlowRow>,
    pub total_investing_cents: i64,
    pub financing: Vec<CashFlowRow>,
    pub total_financing_cents: i64,
    pub net_change_cents: i64,
}

#[derive(Debug, Serialize, Deserialize, Validate)]
pub struct TransferRequest {
    #[validate(length(min = 1))]
    pub from_account_id: String,
    #[validate(length(min = 1))]
    pub to_account_id: String,
    #[validate(range(min = 1, message = "Amount must be positive"))]
    pub amount_cents: i64,
    pub transfer_date: String,
    pub description: Option<String>,
    pub reference: Option<String>,
}
