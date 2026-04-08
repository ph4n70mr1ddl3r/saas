use serde::{Deserialize, Serialize};
use validator::Validate;

#[derive(Debug, Serialize, Deserialize, sqlx::FromRow)]
pub struct Compensation {
    pub id: String,
    pub employee_id: String,
    pub salary_type: String,
    pub amount_cents: i64,
    pub currency: String,
    pub effective_date: String,
    pub end_date: Option<String>,
    pub created_at: String,
}

#[derive(Debug, Serialize, Deserialize, Validate)]
pub struct CreateCompensationRequest {
    #[validate(length(min = 1, message = "Employee ID is required"))]
    pub employee_id: String,
    #[validate(length(min = 1, message = "Salary type is required"))]
    pub salary_type: String,
    #[validate(range(min = 0, message = "Amount must be non-negative"))]
    pub amount_cents: i64,
    pub currency: Option<String>,
    pub effective_date: String,
    pub end_date: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Default, Validate)]
pub struct UpdateCompensationRequest {
    #[validate(length(min = 1, message = "Salary type is required"))]
    pub salary_type: Option<String>,
    #[validate(range(min = 0, message = "Amount must be non-negative"))]
    pub amount_cents: Option<i64>,
    pub currency: Option<String>,
    pub effective_date: Option<String>,
    pub end_date: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, sqlx::FromRow)]
pub struct PayRun {
    pub id: String,
    pub period_start: String,
    pub period_end: String,
    pub pay_date: String,
    pub status: String,
    pub created_at: String,
}

#[derive(Debug, Serialize, Deserialize, Validate)]
pub struct CreatePayRunRequest {
    #[validate(length(min = 1, message = "Period start is required"))]
    pub period_start: String,
    #[validate(length(min = 1, message = "Period end is required"))]
    pub period_end: String,
    #[validate(length(min = 1, message = "Pay date is required"))]
    pub pay_date: String,
}

#[derive(Debug, Serialize, Deserialize, sqlx::FromRow)]
pub struct Payslip {
    pub id: String,
    pub pay_run_id: String,
    pub employee_id: String,
    pub gross_pay: i64,
    pub net_pay: i64,
    pub tax: i64,
    pub deductions: i64,
    pub status: String,
    pub created_at: String,
}

#[derive(Debug, Serialize, Deserialize, sqlx::FromRow)]
pub struct Deduction {
    pub id: String,
    pub employee_id: String,
    pub code: String,
    pub amount_cents: i64,
    pub recurring: bool,
    pub start_date: String,
    pub end_date: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Validate)]
pub struct CreateDeductionRequest {
    #[validate(length(min = 1, message = "Employee ID is required"))]
    pub employee_id: String,
    #[validate(length(min = 1, message = "Code is required"))]
    pub code: String,
    #[validate(range(min = 1, message = "Amount must be at least 1"))]
    pub amount_cents: i64,
    pub recurring: Option<bool>,
    pub start_date: String,
    pub end_date: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, sqlx::FromRow)]
pub struct TaxBracket {
    pub id: String,
    pub name: String,
    pub min_income_cents: i64,
    pub max_income_cents: Option<i64>,
    pub rate_percent: f64,
    pub created_at: String,
}

#[derive(Debug, Serialize, Deserialize, Validate)]
pub struct CreateTaxBracketRequest {
    #[validate(length(min = 1, message = "Name is required"))]
    pub name: String,
    pub min_income_cents: i64,
    pub max_income_cents: Option<i64>,
    #[validate(range(min = 0.0, max = 100.0, message = "Rate must be between 0 and 100"))]
    pub rate_percent: f64,
}
