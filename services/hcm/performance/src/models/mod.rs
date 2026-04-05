use serde::{Deserialize, Serialize};

// --- Review Cycle ---

#[derive(Debug, Serialize, Deserialize, sqlx::FromRow)]
pub struct ReviewCycle {
    pub id: String,
    pub name: String,
    pub description: Option<String>,
    pub start_date: String,
    pub end_date: String,
    pub status: String,
    pub created_at: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct CreateReviewCycleRequest {
    pub name: String,
    pub description: Option<String>,
    pub start_date: String,
    pub end_date: String,
}

// --- Goals ---

#[derive(Debug, Serialize, Deserialize, sqlx::FromRow)]
pub struct Goal {
    pub id: String,
    pub employee_id: String,
    pub cycle_id: String,
    pub title: String,
    pub description: Option<String>,
    pub weight: f64,
    pub progress: f64,
    pub status: String,
    pub due_date: Option<String>,
    pub created_at: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct CreateGoalRequest {
    pub employee_id: String,
    pub cycle_id: String,
    pub title: String,
    pub description: Option<String>,
    pub weight: Option<f64>,
    pub progress: Option<f64>,
    pub due_date: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct UpdateGoalRequest {
    pub title: Option<String>,
    pub description: Option<String>,
    pub weight: Option<f64>,
    pub progress: Option<f64>,
    pub status: Option<String>,
    pub due_date: Option<String>,
}

// --- Review Assignments ---

#[derive(Debug, Serialize, Deserialize, sqlx::FromRow)]
pub struct ReviewAssignment {
    pub id: String,
    pub cycle_id: String,
    pub reviewer_id: String,
    pub employee_id: String,
    pub status: String,
    pub rating: Option<i32>,
    pub comments: Option<String>,
    pub submitted_at: Option<String>,
    pub created_at: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct CreateReviewAssignmentRequest {
    pub cycle_id: String,
    pub reviewer_id: String,
    pub employee_id: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SubmitReviewRequest {
    pub rating: i32,
    pub comments: Option<String>,
}

// --- Feedback ---

#[derive(Debug, Serialize, Deserialize, sqlx::FromRow)]
pub struct Feedback {
    pub id: String,
    pub cycle_id: String,
    pub from_employee_id: String,
    pub to_employee_id: String,
    pub content: String,
    pub is_anonymous: bool,
    pub created_at: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct CreateFeedbackRequest {
    pub cycle_id: String,
    pub from_employee_id: String,
    pub to_employee_id: String,
    pub content: String,
    pub is_anonymous: Option<bool>,
}
