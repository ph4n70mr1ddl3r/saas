use serde::{Deserialize, Serialize};

// --- Expense Categories ---

#[derive(Debug, Serialize, Deserialize, sqlx::FromRow)]
pub struct ExpenseCategory {
    pub id: String,
    pub name: String,
    pub description: Option<String>,
    pub limit_cents: i64,
    pub requires_receipt: i64,
    pub is_active: i64,
    pub gl_account_code: Option<String>,
    pub created_at: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct CreateExpenseCategoryRequest {
    pub name: String,
    pub description: Option<String>,
    pub limit_cents: Option<i64>,
    pub requires_receipt: Option<bool>,
}

// --- Expense Reports ---

#[derive(Debug, Serialize, Deserialize, sqlx::FromRow)]
pub struct ExpenseReport {
    pub id: String,
    pub employee_id: String,
    pub title: String,
    pub description: Option<String>,
    pub total_cents: i64,
    pub status: String,
    pub submitted_at: Option<String>,
    pub approved_by: Option<String>,
    pub approved_at: Option<String>,
    pub rejected_reason: Option<String>,
    pub created_at: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct CreateExpenseReportRequest {
    pub employee_id: String,
    pub title: String,
    pub description: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ExpenseReportWithLines {
    pub report: ExpenseReport,
    pub lines: Vec<ExpenseLine>,
    pub per_diems: Vec<PerDiem>,
    pub mileage: Vec<Mileage>,
}

// --- Expense Lines ---

#[derive(Debug, Serialize, Deserialize, sqlx::FromRow)]
pub struct ExpenseLine {
    pub id: String,
    pub report_id: String,
    pub expense_date: String,
    pub category_id: String,
    pub amount_cents: i64,
    pub description: Option<String>,
    pub receipt_url: Option<String>,
    pub created_at: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct CreateExpenseLineRequest {
    pub report_id: String,
    pub expense_date: String,
    pub category_id: String,
    pub amount_cents: i64,
    pub description: Option<String>,
    pub receipt_url: Option<String>,
}

// --- Per Diems ---

#[derive(Debug, Serialize, Deserialize, sqlx::FromRow)]
pub struct PerDiem {
    pub id: String,
    pub report_id: String,
    pub location: String,
    pub start_date: String,
    pub end_date: String,
    pub daily_rate_cents: i64,
    pub total_cents: i64,
    pub created_at: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct CreatePerDiemRequest {
    pub report_id: String,
    pub location: String,
    pub start_date: String,
    pub end_date: String,
    pub daily_rate_cents: i64,
}

// --- Mileage ---

#[derive(Debug, Serialize, Deserialize, sqlx::FromRow)]
pub struct Mileage {
    pub id: String,
    pub report_id: String,
    pub origin: String,
    pub destination: String,
    pub distance_miles: f64,
    pub rate_per_mile_cents: i64,
    pub total_cents: i64,
    pub expense_date: String,
    pub created_at: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct CreateMileageRequest {
    pub report_id: String,
    pub origin: String,
    pub destination: String,
    pub distance_miles: f64,
    pub rate_per_mile_cents: i64,
    pub expense_date: String,
}

// --- Status transition requests ---

#[derive(Debug, Serialize, Deserialize)]
pub struct UpdateExpenseCategoryRequest {
    pub name: Option<String>,
    pub description: Option<String>,
    pub limit_cents: Option<i64>,
    pub requires_receipt: Option<bool>,
    pub gl_account_code: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SubmitReportRequest {
    pub rejected_reason: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ApproveReportRequest {
    pub rejected_reason: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct RejectReportRequest {
    pub rejected_reason: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct MarkPaidRequest {
    pub rejected_reason: Option<String>,
}
