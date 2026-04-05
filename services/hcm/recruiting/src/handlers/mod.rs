use axum::extract::{Path, State};
use axum::Json;
use saas_auth_core::extractor::AuthUser;
use saas_common::error::AppError;
use saas_common::response::ApiResponse;
use crate::models::*;
use crate::routes::AppState;

pub async fn list_jobs(
    _user: AuthUser,
    State(state): State<AppState>,
) -> Result<Json<ApiResponse<Vec<JobPosting>>>, AppError> {
    let list = state.service.list_jobs().await?;
    Ok(Json(ApiResponse::new(list)))
}

pub async fn get_job(
    _user: AuthUser,
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<ApiResponse<JobPosting>>, AppError> {
    let job = state.service.get_job(&id).await?;
    Ok(Json(ApiResponse::new(job)))
}

pub async fn create_job(
    _user: AuthUser,
    State(state): State<AppState>,
    Json(input): Json<CreateJobRequest>,
) -> Result<Json<ApiResponse<JobPosting>>, AppError> {
    let job = state.service.create_job(input).await?;
    Ok(Json(ApiResponse::new(job)))
}

pub async fn update_job(
    _user: AuthUser,
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(input): Json<UpdateJobRequest>,
) -> Result<Json<ApiResponse<JobPosting>>, AppError> {
    let job = state.service.update_job(&id, input).await?;
    Ok(Json(ApiResponse::new(job)))
}

pub async fn list_applications(
    _user: AuthUser,
    State(state): State<AppState>,
) -> Result<Json<ApiResponse<Vec<Application>>>, AppError> {
    let list = state.service.list_applications().await?;
    Ok(Json(ApiResponse::new(list)))
}

pub async fn create_application(
    _user: AuthUser,
    State(state): State<AppState>,
    Json(input): Json<CreateApplicationRequest>,
) -> Result<Json<ApiResponse<Application>>, AppError> {
    let app = state.service.create_application(input).await?;
    Ok(Json(ApiResponse::new(app)))
}

pub async fn update_application_status(
    _user: AuthUser,
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(input): Json<UpdateApplicationStatusRequest>,
) -> Result<Json<ApiResponse<Application>>, AppError> {
    let app = state.service.update_application_status(&id, input).await?;
    Ok(Json(ApiResponse::new(app)))
}

pub async fn list_applications_by_job(
    _user: AuthUser,
    State(state): State<AppState>,
    Path(job_id): Path<String>,
) -> Result<Json<ApiResponse<Vec<Application>>>, AppError> {
    let list = state.service.list_applications_by_job(&job_id).await?;
    Ok(Json(ApiResponse::new(list)))
}
