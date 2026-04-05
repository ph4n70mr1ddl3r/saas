use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, sqlx::FromRow)]
pub struct Account {
    pub id: String,
    pub code: String,
    pub name: String,
    pub account_type: String,
    pub parent_id: Option<String>,
    pub is_active: i64,
    pub created_at: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct CreateAccountRequest {
    pub code: String,
    pub name: String,
    pub account_type: String,
    pub parent_id: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, sqlx::FromRow)]
pub struct Period {
    pub id: String,
    pub name: String,
    pub start_date: String,
    pub end_date: String,
    pub status: String,
    pub fiscal_year: i64,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct CreatePeriodRequest {
    pub name: String,
    pub start_date: String,
    pub end_date: String,
    pub fiscal_year: i64,
}

#[derive(Debug, Serialize, Deserialize, sqlx::FromRow)]
pub struct JournalEntry {
    pub id: String,
    pub entry_number: String,
    pub description: Option<String>,
    pub period_id: String,
    pub status: String,
    pub posted_at: Option<String>,
    pub created_by: String,
    pub created_at: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct CreateJournalEntryRequest {
    pub description: Option<String>,
    pub period_id: String,
    pub lines: Vec<CreateJournalLineRequest>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct CreateJournalLineRequest {
    pub account_id: String,
    pub debit_cents: i64,
    pub credit_cents: i64,
    pub description: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, sqlx::FromRow)]
pub struct JournalLine {
    pub id: String,
    pub entry_id: String,
    pub account_id: String,
    pub debit_cents: i64,
    pub credit_cents: i64,
    pub description: Option<String>,
    pub created_at: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct JournalEntryWithLines {
    pub entry: JournalEntry,
    pub lines: Vec<JournalLine>,
}

#[derive(Debug, Serialize, Deserialize, sqlx::FromRow)]
pub struct TrialBalanceRow {
    pub account_code: String,
    pub account_name: String,
    pub account_type: String,
    pub total_debit_cents: i64,
    pub total_credit_cents: i64,
}

#[derive(Debug, Serialize, Deserialize, sqlx::FromRow)]
pub struct BalanceSheetRow {
    pub account_code: String,
    pub account_name: String,
    pub account_type: String,
    pub balance_cents: i64,
}
