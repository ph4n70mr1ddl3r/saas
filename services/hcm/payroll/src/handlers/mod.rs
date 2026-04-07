use crate::models::*;
use crate::routes::AppState;
use axum::extract::{Path, State};
use axum::Json;
use saas_auth_core::extractor::AuthUser;
use saas_auth_core::rbac;
use saas_common::error::AppError;
use saas_common::response::ApiResponse;

pub async fn list_compensation(
    _user: AuthUser,
    State(state): State<AppState>,
) -> Result<Json<ApiResponse<Vec<Compensation>>>, AppError> {
    let list = state.service.list_compensation().await?;
    Ok(Json(ApiResponse::new(list)))
}

pub async fn get_compensation(
    _user: AuthUser,
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<ApiResponse<Compensation>>, AppError> {
    let comp = state.service.get_compensation(&id).await?;
    Ok(Json(ApiResponse::new(comp)))
}

pub async fn list_compensation_by_employee(
    _user: AuthUser,
    State(state): State<AppState>,
    Path(employee_id): Path<String>,
) -> Result<Json<ApiResponse<Vec<Compensation>>>, AppError> {
    let list = state
        .service
        .list_compensation_by_employee(&employee_id)
        .await?;
    Ok(Json(ApiResponse::new(list)))
}

pub async fn create_compensation(
    user: AuthUser,
    State(state): State<AppState>,
    Json(input): Json<CreateCompensationRequest>,
) -> Result<Json<ApiResponse<Compensation>>, AppError> {
    rbac::require_admin(&user.roles, "hcm").map_err(|e| AppError::Forbidden(e))?;
    let comp = state.service.create_compensation(input).await?;
    Ok(Json(ApiResponse::new(comp)))
}

pub async fn update_compensation(
    user: AuthUser,
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(input): Json<UpdateCompensationRequest>,
) -> Result<Json<ApiResponse<Compensation>>, AppError> {
    rbac::require_admin(&user.roles, "hcm").map_err(|e| AppError::Forbidden(e))?;
    let comp = state.service.update_compensation(&id, input).await?;
    Ok(Json(ApiResponse::new(comp)))
}

pub async fn list_pay_runs(
    _user: AuthUser,
    State(state): State<AppState>,
) -> Result<Json<ApiResponse<Vec<PayRun>>>, AppError> {
    let list = state.service.list_pay_runs().await?;
    Ok(Json(ApiResponse::new(list)))
}

pub async fn create_pay_run(
    user: AuthUser,
    State(state): State<AppState>,
    Json(input): Json<CreatePayRunRequest>,
) -> Result<Json<ApiResponse<PayRun>>, AppError> {
    rbac::require_admin(&user.roles, "hcm").map_err(|e| AppError::Forbidden(e))?;
    let run = state.service.create_pay_run(input).await?;
    Ok(Json(ApiResponse::new(run)))
}

pub async fn process_pay_run(
    user: AuthUser,
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<ApiResponse<PayRun>>, AppError> {
    rbac::require_admin(&user.roles, "hcm").map_err(|e| AppError::Forbidden(e))?;
    let run = state.service.process_pay_run(&id).await?;
    Ok(Json(ApiResponse::new(run)))
}

pub async fn list_payslips_for_run(
    _user: AuthUser,
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<ApiResponse<Vec<Payslip>>>, AppError> {
    let list = state.service.list_payslips_for_run(&id).await?;
    Ok(Json(ApiResponse::new(list)))
}

pub async fn list_deductions_by_employee(
    _user: AuthUser,
    State(state): State<AppState>,
    Path(employee_id): Path<String>,
) -> Result<Json<ApiResponse<Vec<Deduction>>>, AppError> {
    let list = state
        .service
        .list_deductions_by_employee(&employee_id)
        .await?;
    Ok(Json(ApiResponse::new(list)))
}

pub async fn create_deduction(
    user: AuthUser,
    State(state): State<AppState>,
    Json(input): Json<CreateDeductionRequest>,
) -> Result<Json<ApiResponse<Deduction>>, AppError> {
    rbac::require_admin(&user.roles, "hcm").map_err(|e| AppError::Forbidden(e))?;
    let deduction = state.service.create_deduction(input).await?;
    Ok(Json(ApiResponse::new(deduction)))
}

pub async fn list_tax_brackets(
    _user: AuthUser,
    State(state): State<AppState>,
) -> Result<Json<ApiResponse<Vec<TaxBracket>>>, AppError> {
    let list = state.service.list_tax_brackets().await?;
    Ok(Json(ApiResponse::new(list)))
}

pub async fn create_tax_bracket(
    user: AuthUser,
    State(state): State<AppState>,
    Json(input): Json<CreateTaxBracketRequest>,
) -> Result<Json<ApiResponse<TaxBracket>>, AppError> {
    rbac::require_admin(&user.roles, "hcm").map_err(|e| AppError::Forbidden(e))?;
    let bracket = state.service.create_tax_bracket(input).await?;
    Ok(Json(ApiResponse::new(bracket)))
}
