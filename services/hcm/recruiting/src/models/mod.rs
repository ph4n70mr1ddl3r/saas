use serde::{Deserialize, Serialize};
use validator::Validate;

#[derive(Debug, Serialize, Deserialize, sqlx::FromRow)]
pub struct JobPosting {
    pub id: String,
    pub title: String,
    pub department_id: String,
    pub description: Option<String>,
    pub requirements: Option<String>,
    pub status: String,
    pub posted_at: String,
    pub closed_at: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Validate)]
pub struct CreateJobRequest {
    #[validate(length(min = 1))]
    pub title: String,
    #[validate(length(min = 1))]
    pub department_id: String,
    pub description: Option<String>,
    pub requirements: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct UpdateJobRequest {
    pub title: Option<String>,
    pub department_id: Option<String>,
    pub description: Option<String>,
    pub requirements: Option<String>,
    pub status: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, sqlx::FromRow)]
pub struct Application {
    pub id: String,
    pub job_id: String,
    pub candidate_first_name: String,
    pub candidate_last_name: String,
    pub candidate_email: String,
    pub status: String,
    pub applied_at: String,
    pub notes: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Validate)]
pub struct CreateApplicationRequest {
    #[validate(length(min = 1))]
    pub job_id: String,
    #[validate(length(min = 1))]
    pub candidate_first_name: String,
    #[validate(length(min = 1))]
    pub candidate_last_name: String,
    #[validate(email)]
    pub candidate_email: String,
    pub notes: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct UpdateApplicationStatusRequest {
    pub status: String,
    pub notes: Option<String>,
}
