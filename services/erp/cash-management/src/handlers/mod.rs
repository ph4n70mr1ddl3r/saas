use crate::models::*;
use crate::routes::AppState;
use axum::extract::{Path, Query, State};
use axum::Json;
use saas_auth_core::extractor::AuthUser;
use saas_auth_core::rbac;
use saas_common::error::AppError;
use saas_common::response::ApiResponse;
use serde::Deserialize;

pub async fn list_bank_accounts(
    user: AuthUser,
    State(state): State<AppState>,
) -> Result<Json<ApiResponse<Vec<BankAccount>>>, AppError> {
    let _ = &user;
    let accounts = state.service.list_bank_accounts().await?;
    Ok(Json(ApiResponse::new(accounts)))
}

pub async fn get_bank_account(
    user: AuthUser,
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<ApiResponse<BankAccount>>, AppError> {
    let _ = &user;
    let account = state.service.get_bank_account(&id).await?;
    Ok(Json(ApiResponse::new(account)))
}

pub async fn create_bank_account(
    user: AuthUser,
    State(state): State<AppState>,
    Json(input): Json<CreateBankAccountRequest>,
) -> Result<Json<ApiResponse<BankAccount>>, AppError> {
    rbac::require_admin(&user.roles, "erp").map_err(|e| AppError::Forbidden(e))?;
    let account = state.service.create_bank_account(&input).await?;
    Ok(Json(ApiResponse::new(account)))
}

pub async fn update_bank_account(
    user: AuthUser,
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(input): Json<UpdateBankAccountRequest>,
) -> Result<Json<ApiResponse<BankAccount>>, AppError> {
    rbac::require_admin(&user.roles, "erp").map_err(|e| AppError::Forbidden(e))?;
    let account = state.service.update_bank_account(&id, &input).await?;
    Ok(Json(ApiResponse::new(account)))
}

pub async fn list_bank_transactions(
    user: AuthUser,
    State(state): State<AppState>,
) -> Result<Json<ApiResponse<Vec<BankTransaction>>>, AppError> {
    let _ = &user;
    let transactions = state.service.list_bank_transactions().await?;
    Ok(Json(ApiResponse::new(transactions)))
}

pub async fn create_bank_transaction(
    user: AuthUser,
    State(state): State<AppState>,
    Json(input): Json<CreateBankTransactionRequest>,
) -> Result<Json<ApiResponse<BankTransaction>>, AppError> {
    rbac::require_admin(&user.roles, "erp").map_err(|e| AppError::Forbidden(e))?;
    let transaction = state.service.create_bank_transaction(&input).await?;
    Ok(Json(ApiResponse::new(transaction)))
}

pub async fn list_reconciliations(
    user: AuthUser,
    State(state): State<AppState>,
) -> Result<Json<ApiResponse<Vec<Reconciliation>>>, AppError> {
    let _ = &user;
    let reconciliations = state.service.list_reconciliations().await?;
    Ok(Json(ApiResponse::new(reconciliations)))
}

pub async fn create_reconciliation(
    user: AuthUser,
    State(state): State<AppState>,
    Json(input): Json<CreateReconciliationRequest>,
) -> Result<Json<ApiResponse<Reconciliation>>, AppError> {
    rbac::require_admin(&user.roles, "erp").map_err(|e| AppError::Forbidden(e))?;
    let reconciliation = state.service.create_reconciliation(&input).await?;
    Ok(Json(ApiResponse::new(reconciliation)))
}

// --- Cash Flow Statement ---

#[derive(Debug, Deserialize)]
pub struct CashFlowQuery {
    pub period_start: String,
    pub period_end: String,
}

pub async fn cash_flow_statement(
    user: AuthUser,
    State(state): State<AppState>,
    Query(params): Query<CashFlowQuery>,
) -> Result<Json<ApiResponse<CashFlowStatement>>, AppError> {
    let _ = &user;
    let statement = state
        .service
        .cash_flow_statement(&params.period_start, &params.period_end)
        .await?;
    Ok(Json(ApiResponse::new(statement)))
}

// --- Transfers ---

pub async fn transfer(
    user: AuthUser,
    State(state): State<AppState>,
    Json(input): Json<TransferRequest>,
) -> Result<Json<ApiResponse<Vec<BankTransaction>>>, AppError> {
    rbac::require_admin(&user.roles, "erp").map_err(|e| AppError::Forbidden(e))?;
    let (withdrawal, deposit) = state.service.transfer(&input).await?;
    Ok(Json(ApiResponse::new(vec![withdrawal, deposit])))
}
