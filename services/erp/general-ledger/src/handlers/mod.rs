use axum::extract::{Path, State};
use axum::Json;
use saas_auth_core::extractor::AuthUser;
use saas_common::error::AppError;
use saas_common::response::ApiResponse;
use crate::models::*;
use crate::routes::AppState;

pub async fn list_accounts(
    _user: AuthUser,
    State(state): State<AppState>,
) -> Result<Json<ApiResponse<Vec<Account>>>, AppError> {
    let accounts = state.service.list_accounts().await?;
    Ok(Json(ApiResponse::new(accounts)))
}

pub async fn get_account(
    _user: AuthUser,
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<ApiResponse<Account>>, AppError> {
    let account = state.service.get_account(&id).await?;
    Ok(Json(ApiResponse::new(account)))
}

pub async fn create_account(
    _user: AuthUser,
    State(state): State<AppState>,
    Json(input): Json<CreateAccountRequest>,
) -> Result<Json<ApiResponse<Account>>, AppError> {
    let account = state.service.create_account(&input).await?;
    Ok(Json(ApiResponse::new(account)))
}

pub async fn list_periods(
    _user: AuthUser,
    State(state): State<AppState>,
) -> Result<Json<ApiResponse<Vec<Period>>>, AppError> {
    let periods = state.service.list_periods().await?;
    Ok(Json(ApiResponse::new(periods)))
}

pub async fn create_period(
    _user: AuthUser,
    State(state): State<AppState>,
    Json(input): Json<CreatePeriodRequest>,
) -> Result<Json<ApiResponse<Period>>, AppError> {
    let period = state.service.create_period(&input).await?;
    Ok(Json(ApiResponse::new(period)))
}

pub async fn close_period(
    _user: AuthUser,
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<ApiResponse<Period>>, AppError> {
    let period = state.service.close_period(&id).await?;
    Ok(Json(ApiResponse::new(period)))
}

pub async fn list_journal_entries(
    _user: AuthUser,
    State(state): State<AppState>,
) -> Result<Json<ApiResponse<Vec<JournalEntry>>>, AppError> {
    let entries = state.service.list_journal_entries().await?;
    Ok(Json(ApiResponse::new(entries)))
}

pub async fn get_journal_entry(
    _user: AuthUser,
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<ApiResponse<JournalEntryWithLines>>, AppError> {
    let entry = state.service.get_journal_entry(&id).await?;
    Ok(Json(ApiResponse::new(entry)))
}

pub async fn create_journal_entry(
    user: AuthUser,
    State(state): State<AppState>,
    Json(input): Json<CreateJournalEntryRequest>,
) -> Result<Json<ApiResponse<JournalEntryWithLines>>, AppError> {
    let entry = state.service.create_journal_entry(&input, &user.user_id).await?;
    Ok(Json(ApiResponse::new(entry)))
}

pub async fn post_journal_entry(
    _user: AuthUser,
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<ApiResponse<JournalEntryWithLines>>, AppError> {
    let entry = state.service.post_journal_entry(&id).await?;
    Ok(Json(ApiResponse::new(entry)))
}

pub async fn reverse_journal_entry(
    _user: AuthUser,
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<ApiResponse<JournalEntryWithLines>>, AppError> {
    let entry = state.service.reverse_journal_entry(&id).await?;
    Ok(Json(ApiResponse::new(entry)))
}

pub async fn trial_balance(
    _user: AuthUser,
    State(state): State<AppState>,
) -> Result<Json<ApiResponse<Vec<TrialBalanceRow>>>, AppError> {
    let rows = state.service.trial_balance().await?;
    Ok(Json(ApiResponse::new(rows)))
}

pub async fn balance_sheet(
    _user: AuthUser,
    State(state): State<AppState>,
) -> Result<Json<ApiResponse<Vec<BalanceSheetRow>>>, AppError> {
    let rows = state.service.balance_sheet().await?;
    Ok(Json(ApiResponse::new(rows)))
}
