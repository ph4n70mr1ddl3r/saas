use crate::models::*;
use crate::routes::AppState;
use axum::extract::{Path, State};
use axum::Json;
use saas_auth_core::extractor::AuthUser;
use saas_auth_core::rbac;
use saas_common::error::AppError;
use saas_common::response::ApiResponse;

// --- Review Cycles ---

pub async fn list_review_cycles(
    _user: AuthUser,
    State(state): State<AppState>,
) -> Result<Json<ApiResponse<Vec<ReviewCycle>>>, AppError> {
    let list = state.service.list_review_cycles().await?;
    Ok(Json(ApiResponse::new(list)))
}

pub async fn get_review_cycle(
    _user: AuthUser,
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<ApiResponse<ReviewCycle>>, AppError> {
    let cycle = state.service.get_review_cycle(&id).await?;
    Ok(Json(ApiResponse::new(cycle)))
}

pub async fn create_review_cycle(
    user: AuthUser,
    State(state): State<AppState>,
    Json(input): Json<CreateReviewCycleRequest>,
) -> Result<Json<ApiResponse<ReviewCycle>>, AppError> {
    rbac::require_admin(&user.roles, "hcm").map_err(|e| AppError::Forbidden(e))?;
    let cycle = state.service.create_review_cycle(input).await?;
    Ok(Json(ApiResponse::new(cycle)))
}

pub async fn activate_review_cycle(
    user: AuthUser,
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<ApiResponse<ReviewCycle>>, AppError> {
    rbac::require_admin(&user.roles, "hcm").map_err(|e| AppError::Forbidden(e))?;
    let cycle = state.service.activate_review_cycle(&id).await?;
    Ok(Json(ApiResponse::new(cycle)))
}

pub async fn close_review_cycle(
    user: AuthUser,
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<ApiResponse<ReviewCycle>>, AppError> {
    rbac::require_admin(&user.roles, "hcm").map_err(|e| AppError::Forbidden(e))?;
    let cycle = state.service.close_review_cycle(&id).await?;
    Ok(Json(ApiResponse::new(cycle)))
}

// --- Goals ---

pub async fn list_goals(
    _user: AuthUser,
    State(state): State<AppState>,
) -> Result<Json<ApiResponse<Vec<Goal>>>, AppError> {
    let list = state.service.list_goals().await?;
    Ok(Json(ApiResponse::new(list)))
}

pub async fn create_goal(
    user: AuthUser,
    State(state): State<AppState>,
    Json(input): Json<CreateGoalRequest>,
) -> Result<Json<ApiResponse<Goal>>, AppError> {
    rbac::require_admin(&user.roles, "hcm").map_err(|e| AppError::Forbidden(e))?;
    let goal = state.service.create_goal(input).await?;
    Ok(Json(ApiResponse::new(goal)))
}

pub async fn update_goal(
    user: AuthUser,
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(input): Json<UpdateGoalRequest>,
) -> Result<Json<ApiResponse<Goal>>, AppError> {
    rbac::require_admin(&user.roles, "hcm").map_err(|e| AppError::Forbidden(e))?;
    let goal = state.service.update_goal(&id, input).await?;
    Ok(Json(ApiResponse::new(goal)))
}

// --- Review Assignments ---

pub async fn list_review_assignments(
    _user: AuthUser,
    State(state): State<AppState>,
) -> Result<Json<ApiResponse<Vec<ReviewAssignment>>>, AppError> {
    let list = state.service.list_review_assignments().await?;
    Ok(Json(ApiResponse::new(list)))
}

pub async fn create_review_assignment(
    user: AuthUser,
    State(state): State<AppState>,
    Json(input): Json<CreateReviewAssignmentRequest>,
) -> Result<Json<ApiResponse<ReviewAssignment>>, AppError> {
    rbac::require_admin(&user.roles, "hcm").map_err(|e| AppError::Forbidden(e))?;
    let assignment = state.service.create_review_assignment(input).await?;
    Ok(Json(ApiResponse::new(assignment)))
}

pub async fn submit_review(
    user: AuthUser,
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(input): Json<SubmitReviewRequest>,
) -> Result<Json<ApiResponse<ReviewAssignment>>, AppError> {
    rbac::require_admin(&user.roles, "hcm").map_err(|e| AppError::Forbidden(e))?;
    let assignment = state.service.submit_review(&id, input).await?;
    Ok(Json(ApiResponse::new(assignment)))
}

// --- Feedback ---

pub async fn list_feedback(
    _user: AuthUser,
    State(state): State<AppState>,
) -> Result<Json<ApiResponse<Vec<Feedback>>>, AppError> {
    let list = state.service.list_feedback().await?;
    Ok(Json(ApiResponse::new(list)))
}

pub async fn create_feedback(
    user: AuthUser,
    State(state): State<AppState>,
    Json(input): Json<CreateFeedbackRequest>,
) -> Result<Json<ApiResponse<Feedback>>, AppError> {
    rbac::require_admin(&user.roles, "hcm").map_err(|e| AppError::Forbidden(e))?;
    let feedback = state.service.create_feedback(input).await?;
    Ok(Json(ApiResponse::new(feedback)))
}
