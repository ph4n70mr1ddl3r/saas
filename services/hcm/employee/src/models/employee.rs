use serde::{Deserialize, Serialize};
use validator::Validate;

#[derive(Debug, Serialize, Deserialize, Validate)]
pub struct CreateEmployee {
    #[validate(length(min = 1, max = 100))]
    pub first_name: String,
    #[validate(length(min = 1, max = 100))]
    pub last_name: String,
    #[validate(email)]
    pub email: String,
    pub phone: Option<String>,
    pub hire_date: String,
    pub department_id: String,
    pub reports_to: Option<String>,
    #[validate(length(min = 1, max = 100))]
    pub job_title: String,
    pub employee_number: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct UpdateEmployee {
    pub first_name: Option<String>,
    pub last_name: Option<String>,
    pub email: Option<String>,
    pub phone: Option<String>,
    pub department_id: Option<String>,
    pub reports_to: Option<String>,
    pub job_title: Option<String>,
    pub status: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, sqlx::FromRow)]
pub struct EmployeeResponse {
    pub id: String,
    pub first_name: String,
    pub last_name: String,
    pub email: String,
    pub phone: Option<String>,
    pub hire_date: String,
    pub termination_date: Option<String>,
    pub status: String,
    pub department_id: String,
    pub reports_to: Option<String>,
    pub job_title: String,
    pub employee_number: String,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Deserialize)]
pub struct EmployeeFilters {
    pub department_id: Option<String>,
    pub status: Option<String>,
}
