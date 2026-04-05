use crate::models::*;
use crate::routes::AppState;
use axum::extract::{Path, State};
use axum::Json;
use saas_auth_core::extractor::AuthUser;
use saas_auth_core::rbac;
use saas_common::error::AppError;
use saas_common::response::ApiResponse;

// --- Expense Categories ---

pub async fn list_expense_categories(
    user: AuthUser,
    State(state): State<AppState>,
) -> Result<Json<ApiResponse<Vec<ExpenseCategory>>>, AppError> {
    let _ = &user;
    let categories = state.service.list_categories().await?;
    Ok(Json(ApiResponse::new(categories)))
}

pub async fn get_expense_category(
    user: AuthUser,
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<ApiResponse<ExpenseCategory>>, AppError> {
    let _ = &user;
    let category = state.service.get_category(&id).await?;
    Ok(Json(ApiResponse::new(category)))
}

pub async fn create_expense_category(
    user: AuthUser,
    State(state): State<AppState>,
    Json(input): Json<CreateExpenseCategoryRequest>,
) -> Result<Json<ApiResponse<ExpenseCategory>>, AppError> {
    rbac::require_admin(&user.roles, "erp").map_err(|e| AppError::Forbidden(e))?;
    let category = state.service.create_category(&input).await?;
    Ok(Json(ApiResponse::new(category)))
}

// --- Expense Reports ---

pub async fn list_expense_reports(
    user: AuthUser,
    State(state): State<AppState>,
) -> Result<Json<ApiResponse<Vec<ExpenseReport>>>, AppError> {
    let _ = &user;
    let reports = state.service.list_reports().await?;
    Ok(Json(ApiResponse::new(reports)))
}

pub async fn get_expense_report(
    user: AuthUser,
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<ApiResponse<ExpenseReportWithLines>>, AppError> {
    let _ = &user;
    let report = state.service.get_report(&id).await?;
    Ok(Json(ApiResponse::new(report)))
}

pub async fn create_expense_report(
    user: AuthUser,
    State(state): State<AppState>,
    Json(input): Json<CreateExpenseReportRequest>,
) -> Result<Json<ApiResponse<ExpenseReport>>, AppError> {
    rbac::require_admin(&user.roles, "erp").map_err(|e| AppError::Forbidden(e))?;
    let report = state.service.create_report(&input).await?;
    Ok(Json(ApiResponse::new(report)))
}

pub async fn submit_expense_report(
    user: AuthUser,
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<ApiResponse<ExpenseReport>>, AppError> {
    rbac::require_admin(&user.roles, "erp").map_err(|e| AppError::Forbidden(e))?;
    let report = state.service.submit_report(&id).await?;
    Ok(Json(ApiResponse::new(report)))
}

pub async fn approve_expense_report(
    user: AuthUser,
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<ApiResponse<ExpenseReport>>, AppError> {
    rbac::require_admin(&user.roles, "erp").map_err(|e| AppError::Forbidden(e))?;
    let report = state.service.approve_report(&id, &user.user_id).await?;
    Ok(Json(ApiResponse::new(report)))
}

pub async fn reject_expense_report(
    user: AuthUser,
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(input): Json<RejectReportRequest>,
) -> Result<Json<ApiResponse<ExpenseReport>>, AppError> {
    rbac::require_admin(&user.roles, "erp").map_err(|e| AppError::Forbidden(e))?;
    let report = state
        .service
        .reject_report(&id, &input.rejected_reason)
        .await?;
    Ok(Json(ApiResponse::new(report)))
}

pub async fn mark_expense_report_paid(
    user: AuthUser,
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<ApiResponse<ExpenseReport>>, AppError> {
    rbac::require_admin(&user.roles, "erp").map_err(|e| AppError::Forbidden(e))?;
    let report = state.service.mark_paid(&id).await?;
    Ok(Json(ApiResponse::new(report)))
}

// --- Expense Lines ---

pub async fn create_expense_line(
    user: AuthUser,
    State(state): State<AppState>,
    Json(input): Json<CreateExpenseLineRequest>,
) -> Result<Json<ApiResponse<ExpenseLine>>, AppError> {
    rbac::require_admin(&user.roles, "erp").map_err(|e| AppError::Forbidden(e))?;
    let line = state.service.create_line(&input).await?;
    Ok(Json(ApiResponse::new(line)))
}

// --- Per Diems ---

pub async fn list_per_diems(
    user: AuthUser,
    State(state): State<AppState>,
) -> Result<Json<ApiResponse<Vec<PerDiem>>>, AppError> {
    let _ = &user;
    let per_diems = state.service.list_all_per_diems().await?;
    Ok(Json(ApiResponse::new(per_diems)))
}

pub async fn create_per_diem(
    user: AuthUser,
    State(state): State<AppState>,
    Json(input): Json<CreatePerDiemRequest>,
) -> Result<Json<ApiResponse<PerDiem>>, AppError> {
    rbac::require_admin(&user.roles, "erp").map_err(|e| AppError::Forbidden(e))?;
    let per_diem = state.service.create_per_diem(&input).await?;
    Ok(Json(ApiResponse::new(per_diem)))
}

// --- Mileage ---

pub async fn list_mileage(
    user: AuthUser,
    State(state): State<AppState>,
) -> Result<Json<ApiResponse<Vec<Mileage>>>, AppError> {
    let _ = &user;
    let mileage = state.service.list_all_mileage().await?;
    Ok(Json(ApiResponse::new(mileage)))
}

pub async fn create_mileage(
    user: AuthUser,
    State(state): State<AppState>,
    Json(input): Json<CreateMileageRequest>,
) -> Result<Json<ApiResponse<Mileage>>, AppError> {
    rbac::require_admin(&user.roles, "erp").map_err(|e| AppError::Forbidden(e))?;
    let mileage = state.service.create_mileage(&input).await?;
    Ok(Json(ApiResponse::new(mileage)))
}
