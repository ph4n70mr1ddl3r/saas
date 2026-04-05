use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, sqlx::FromRow)]
pub struct BenefitPlan {
    pub id: String,
    pub name: String,
    pub plan_type: String,
    pub description: Option<String>,
    pub employer_contribution_cents: i64,
    pub employee_contribution_cents: i64,
    pub is_active: bool,
    pub created_at: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct CreatePlanRequest {
    pub name: String,
    pub plan_type: String,
    pub description: Option<String>,
    pub employer_contribution_cents: Option<i64>,
    pub employee_contribution_cents: Option<i64>,
    pub is_active: Option<bool>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct UpdatePlanRequest {
    pub name: Option<String>,
    pub plan_type: Option<String>,
    pub description: Option<String>,
    pub employer_contribution_cents: Option<i64>,
    pub employee_contribution_cents: Option<i64>,
    pub is_active: Option<bool>,
}

#[derive(Debug, Serialize, Deserialize, sqlx::FromRow)]
pub struct Enrollment {
    pub id: String,
    pub employee_id: String,
    pub plan_id: String,
    pub status: String,
    pub enrolled_at: String,
    pub cancelled_at: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct CreateEnrollmentRequest {
    pub employee_id: String,
    pub plan_id: String,
}
