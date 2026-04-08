use serde::{Deserialize, Serialize};
use validator::Validate;

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

#[derive(Debug, Serialize, Deserialize, Validate)]
pub struct CreatePlanRequest {
    #[validate(length(min = 1, message = "Name is required"))]
    pub name: String,
    #[validate(length(min = 1, message = "Plan type is required"))]
    pub plan_type: String,
    pub description: Option<String>,
    #[validate(range(min = 0, message = "Must be non-negative"))]
    pub employer_contribution_cents: Option<i64>,
    #[validate(range(min = 0, message = "Must be non-negative"))]
    pub employee_contribution_cents: Option<i64>,
    pub is_active: Option<bool>,
}

#[derive(Debug, Serialize, Deserialize, Validate)]
pub struct UpdatePlanRequest {
    #[validate(length(min = 1, message = "Name is required"))]
    pub name: Option<String>,
    #[validate(length(min = 1, message = "Plan type is required"))]
    pub plan_type: Option<String>,
    pub description: Option<String>,
    #[validate(range(min = 0, message = "Must be non-negative"))]
    pub employer_contribution_cents: Option<i64>,
    #[validate(range(min = 0, message = "Must be non-negative"))]
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

#[derive(Debug, Serialize, Deserialize, Validate)]
pub struct CreateEnrollmentRequest {
    #[validate(length(min = 1, message = "Employee ID is required"))]
    pub employee_id: String,
    #[validate(length(min = 1, message = "Plan ID is required"))]
    pub plan_id: String,
}
