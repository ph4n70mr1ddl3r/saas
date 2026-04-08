use serde::{Deserialize, Serialize};
use validator::Validate;

#[derive(Debug, Serialize, Deserialize, sqlx::FromRow)]
pub struct Timesheet {
    pub id: String,
    pub employee_id: String,
    pub week_start: String,
    pub status: String,
    pub total_hours: f64,
    pub submitted_at: Option<String>,
    pub approved_at: Option<String>,
    pub created_at: String,
}

#[derive(Debug, Serialize, Deserialize, Validate)]
pub struct CreateTimesheetRequest {
    #[validate(length(min = 1))]
    pub employee_id: String,
    #[validate(length(min = 1))]
    pub week_start: String,
}

#[derive(Debug, Serialize, Deserialize, sqlx::FromRow)]
pub struct TimeEntry {
    pub id: String,
    pub timesheet_id: String,
    pub date: String,
    pub hours: f64,
    pub project_code: Option<String>,
    pub description: Option<String>,
    pub created_at: String,
}

#[derive(Debug, Serialize, Deserialize, Validate)]
pub struct CreateTimeEntryRequest {
    #[validate(length(min = 1))]
    pub timesheet_id: String,
    #[validate(length(min = 1))]
    pub date: String,
    #[validate(range(min = 0.0, message = "Hours must be non-negative"))]
    pub hours: f64,
    pub project_code: Option<String>,
    pub description: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, sqlx::FromRow)]
pub struct LeaveRequest {
    pub id: String,
    pub employee_id: String,
    pub leave_type: String,
    pub start_date: String,
    pub end_date: String,
    pub status: String,
    pub reason: Option<String>,
    pub created_at: String,
}

#[derive(Debug, Serialize, Deserialize, Validate)]
pub struct CreateLeaveRequestRequest {
    #[validate(length(min = 1))]
    pub employee_id: String,
    #[validate(length(min = 1))]
    pub leave_type: String,
    #[validate(length(min = 1))]
    pub start_date: String,
    #[validate(length(min = 1))]
    pub end_date: String,
    pub reason: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, sqlx::FromRow)]
pub struct LeaveBalance {
    pub id: String,
    pub employee_id: String,
    pub leave_type: String,
    pub entitled: f64,
    pub used: f64,
    pub remaining: f64,
}
