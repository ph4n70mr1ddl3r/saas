use axum::extract::{Path, State};
use axum::Json;
use saas_auth_core::extractor::AuthUser;
use saas_common::error::AppError;
use saas_common::response::ApiResponse;
use crate::models::*;
use crate::routes::AppState;

pub async fn list_timesheets(
    _user: AuthUser,
    State(state): State<AppState>,
) -> Result<Json<ApiResponse<Vec<Timesheet>>>, AppError> {
    let list = state.service.list_timesheets().await?;
    Ok(Json(ApiResponse::new(list)))
}

pub async fn create_timesheet(
    _user: AuthUser,
    State(state): State<AppState>,
    Json(input): Json<CreateTimesheetRequest>,
) -> Result<Json<ApiResponse<Timesheet>>, AppError> {
    let ts = state.service.create_timesheet(input).await?;
    Ok(Json(ApiResponse::new(ts)))
}

pub async fn list_timesheets_by_employee(
    _user: AuthUser,
    State(state): State<AppState>,
    Path(employee_id): Path<String>,
) -> Result<Json<ApiResponse<Vec<Timesheet>>>, AppError> {
    let list = state.service.list_timesheets_by_employee(&employee_id).await?;
    Ok(Json(ApiResponse::new(list)))
}

pub async fn submit_timesheet(
    _user: AuthUser,
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<ApiResponse<Timesheet>>, AppError> {
    let ts = state.service.submit_timesheet(&id).await?;
    Ok(Json(ApiResponse::new(ts)))
}

pub async fn approve_timesheet(
    _user: AuthUser,
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<ApiResponse<Timesheet>>, AppError> {
    let ts = state.service.approve_timesheet(&id).await?;
    Ok(Json(ApiResponse::new(ts)))
}

pub async fn list_leave_requests(
    _user: AuthUser,
    State(state): State<AppState>,
) -> Result<Json<ApiResponse<Vec<LeaveRequest>>>, AppError> {
    let list = state.service.list_leave_requests().await?;
    Ok(Json(ApiResponse::new(list)))
}

pub async fn create_leave_request(
    _user: AuthUser,
    State(state): State<AppState>,
    Json(input): Json<CreateLeaveRequestRequest>,
) -> Result<Json<ApiResponse<LeaveRequest>>, AppError> {
    let req = state.service.create_leave_request(input).await?;
    Ok(Json(ApiResponse::new(req)))
}

pub async fn approve_leave_request(
    _user: AuthUser,
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<ApiResponse<LeaveRequest>>, AppError> {
    let req = state.service.approve_leave_request(&id).await?;
    Ok(Json(ApiResponse::new(req)))
}

pub async fn reject_leave_request(
    _user: AuthUser,
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<ApiResponse<LeaveRequest>>, AppError> {
    let req = state.service.reject_leave_request(&id).await?;
    Ok(Json(ApiResponse::new(req)))
}

pub async fn list_leave_balances_by_employee(
    _user: AuthUser,
    State(state): State<AppState>,
    Path(employee_id): Path<String>,
) -> Result<Json<ApiResponse<Vec<LeaveBalance>>>, AppError> {
    let list = state.service.list_leave_balances_by_employee(&employee_id).await?;
    Ok(Json(ApiResponse::new(list)))
}
