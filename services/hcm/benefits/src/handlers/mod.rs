use axum::extract::{Path, State};
use axum::Json;
use saas_auth_core::extractor::AuthUser;
use saas_auth_core::rbac;
use saas_common::error::AppError;
use saas_common::response::ApiResponse;
use crate::models::*;
use crate::routes::AppState;

pub async fn list_plans(
    _user: AuthUser,
    State(state): State<AppState>,
) -> Result<Json<ApiResponse<Vec<BenefitPlan>>>, AppError> {
    let list = state.service.list_plans().await?;
    Ok(Json(ApiResponse::new(list)))
}

pub async fn get_plan(
    _user: AuthUser,
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<ApiResponse<BenefitPlan>>, AppError> {
    let plan = state.service.get_plan(&id).await?;
    Ok(Json(ApiResponse::new(plan)))
}

pub async fn create_plan(
    user: AuthUser,
    State(state): State<AppState>,
    Json(input): Json<CreatePlanRequest>,
) -> Result<Json<ApiResponse<BenefitPlan>>, AppError> {
    rbac::require_admin(&user.roles, "hcm").map_err(|e| AppError::Forbidden(e))?;
    let plan = state.service.create_plan(input).await?;
    Ok(Json(ApiResponse::new(plan)))
}

pub async fn update_plan(
    user: AuthUser,
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(input): Json<UpdatePlanRequest>,
) -> Result<Json<ApiResponse<BenefitPlan>>, AppError> {
    rbac::require_admin(&user.roles, "hcm").map_err(|e| AppError::Forbidden(e))?;
    let plan = state.service.update_plan(&id, input).await?;
    Ok(Json(ApiResponse::new(plan)))
}

pub async fn list_enrollments(
    _user: AuthUser,
    State(state): State<AppState>,
) -> Result<Json<ApiResponse<Vec<Enrollment>>>, AppError> {
    let list = state.service.list_enrollments().await?;
    Ok(Json(ApiResponse::new(list)))
}

pub async fn create_enrollment(
    user: AuthUser,
    State(state): State<AppState>,
    Json(input): Json<CreateEnrollmentRequest>,
) -> Result<Json<ApiResponse<Enrollment>>, AppError> {
    rbac::require_admin(&user.roles, "hcm").map_err(|e| AppError::Forbidden(e))?;
    let enrollment = state.service.create_enrollment(input).await?;
    Ok(Json(ApiResponse::new(enrollment)))
}

pub async fn list_enrollments_by_employee(
    _user: AuthUser,
    State(state): State<AppState>,
    Path(employee_id): Path<String>,
) -> Result<Json<ApiResponse<Vec<Enrollment>>>, AppError> {
    let list = state.service.list_enrollments_by_employee(&employee_id).await?;
    Ok(Json(ApiResponse::new(list)))
}

pub async fn cancel_enrollment(
    user: AuthUser,
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<ApiResponse<Enrollment>>, AppError> {
    rbac::require_admin(&user.roles, "hcm").map_err(|e| AppError::Forbidden(e))?;
    let enrollment = state.service.cancel_enrollment(&id).await?;
    Ok(Json(ApiResponse::new(enrollment)))
}
