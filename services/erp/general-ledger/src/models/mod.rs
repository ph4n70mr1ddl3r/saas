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

#[derive(Debug, Serialize, Deserialize, sqlx::FromRow, Clone)]
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
    pub reversal_of: Option<String>,
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

// --- Income Statement ---

#[derive(Debug, Serialize, Deserialize, sqlx::FromRow)]
pub struct IncomeStatementRow {
    pub account_code: String,
    pub account_name: String,
    pub account_type: String,
    pub balance_cents: i64,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct IncomeStatement {
    pub revenue: Vec<IncomeStatementRow>,
    pub total_revenue_cents: i64,
    pub expenses: Vec<IncomeStatementRow>,
    pub total_expense_cents: i64,
    pub net_income_cents: i64,
}

// --- Budget Management ---

#[derive(Debug, Serialize, Deserialize, sqlx::FromRow)]
pub struct Budget {
    pub id: String,
    pub name: String,
    pub period_id: String,
    pub status: String,
    pub total_budget_cents: i64,
    pub created_by: String,
    pub created_at: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct CreateBudgetRequest {
    pub name: String,
    pub period_id: String,
    pub lines: Vec<CreateBudgetLineRequest>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct CreateBudgetLineRequest {
    pub account_id: String,
    pub budgeted_cents: i64,
}

#[derive(Debug, Serialize, Deserialize, sqlx::FromRow)]
pub struct BudgetLine {
    pub id: String,
    pub budget_id: String,
    pub account_id: String,
    pub budgeted_cents: i64,
    pub created_at: String,
}

#[derive(Debug, Deserialize)]
pub struct IncomeStatementQuery {
    pub period_start: String,
    pub period_end: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct BudgetWithLines {
    pub budget: Budget,
    pub lines: Vec<BudgetLine>,
}

#[derive(Debug, Serialize, Deserialize, sqlx::FromRow)]
pub struct BudgetVarianceRow {
    pub account_code: String,
    pub account_name: String,
    pub budgeted_cents: i64,
    pub actual_cents: i64,
    pub variance_cents: i64,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct BudgetVarianceReport {
    pub budget_id: String,
    pub budget_name: String,
    pub period_id: String,
    pub lines: Vec<BudgetVarianceRow>,
    pub total_budgeted_cents: i64,
    pub total_actual_cents: i64,
    pub total_variance_cents: i64,
}
