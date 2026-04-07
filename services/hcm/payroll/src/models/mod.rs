use serde::{Deserialize, Serialize};

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

#[derive(Debug, Serialize, Deserialize)]
pub struct CreateCompensationRequest {
    pub employee_id: String,
    pub salary_type: String,
    pub amount_cents: i64,
    pub currency: Option<String>,
    pub effective_date: String,
    pub end_date: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Default)]
pub struct UpdateCompensationRequest {
    pub salary_type: Option<String>,
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

#[derive(Debug, Serialize, Deserialize)]
pub struct CreatePayRunRequest {
    pub period_start: String,
    pub period_end: String,
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

#[derive(Debug, Serialize, Deserialize)]
pub struct CreateDeductionRequest {
    pub employee_id: String,
    pub code: String,
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

#[derive(Debug, Serialize, Deserialize)]
pub struct CreateTaxBracketRequest {
    pub name: String,
    pub min_income_cents: i64,
    pub max_income_cents: Option<i64>,
    pub rate_percent: f64,
}
