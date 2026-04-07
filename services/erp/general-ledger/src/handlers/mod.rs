use crate::models::*;
use crate::routes::AppState;
use axum::extract::{Path, Query, State};
use axum::Json;
use saas_auth_core::extractor::AuthUser;
use saas_auth_core::rbac;
use saas_common::error::AppError;
use saas_common::response::ApiResponse;

pub async fn list_accounts(
    user: AuthUser,
    State(state): State<AppState>,
) -> Result<Json<ApiResponse<Vec<Account>>>, AppError> {
    let _ = &user;
    let accounts = state.service.list_accounts().await?;
    Ok(Json(ApiResponse::new(accounts)))
}

pub async fn get_account(
    user: AuthUser,
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<ApiResponse<Account>>, AppError> {
    let _ = &user;
    let account = state.service.get_account(&id).await?;
    Ok(Json(ApiResponse::new(account)))
}

pub async fn create_account(
    user: AuthUser,
    State(state): State<AppState>,
    Json(input): Json<CreateAccountRequest>,
) -> Result<Json<ApiResponse<Account>>, AppError> {
    rbac::require_admin(&user.roles, "erp").map_err(|e| AppError::Forbidden(e))?;
    let account = state.service.create_account(&input).await?;
    Ok(Json(ApiResponse::new(account)))
}

pub async fn deactivate_account(
    user: AuthUser,
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<ApiResponse<Account>>, AppError> {
    rbac::require_admin(&user.roles, "erp").map_err(|e| AppError::Forbidden(e))?;
    let account = state.service.deactivate_account(&id).await?;
    Ok(Json(ApiResponse::new(account)))
}

pub async fn list_periods(
    user: AuthUser,
    State(state): State<AppState>,
) -> Result<Json<ApiResponse<Vec<Period>>>, AppError> {
    let _ = &user;
    let periods = state.service.list_periods().await?;
    Ok(Json(ApiResponse::new(periods)))
}

pub async fn create_period(
    user: AuthUser,
    State(state): State<AppState>,
    Json(input): Json<CreatePeriodRequest>,
) -> Result<Json<ApiResponse<Period>>, AppError> {
    rbac::require_admin(&user.roles, "erp").map_err(|e| AppError::Forbidden(e))?;
    let period = state.service.create_period(&input).await?;
    Ok(Json(ApiResponse::new(period)))
}

pub async fn close_period(
    user: AuthUser,
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<ApiResponse<Period>>, AppError> {
    rbac::require_admin(&user.roles, "erp").map_err(|e| AppError::Forbidden(e))?;
    let period = state.service.close_period(&id).await?;
    Ok(Json(ApiResponse::new(period)))
}

pub async fn list_journal_entries(
    user: AuthUser,
    State(state): State<AppState>,
) -> Result<Json<ApiResponse<Vec<JournalEntry>>>, AppError> {
    let _ = &user;
    let entries = state.service.list_journal_entries().await?;
    Ok(Json(ApiResponse::new(entries)))
}

pub async fn get_journal_entry(
    user: AuthUser,
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<ApiResponse<JournalEntryWithLines>>, AppError> {
    let _ = &user;
    let entry = state.service.get_journal_entry(&id).await?;
    Ok(Json(ApiResponse::new(entry)))
}

pub async fn create_journal_entry(
    user: AuthUser,
    State(state): State<AppState>,
    Json(input): Json<CreateJournalEntryRequest>,
) -> Result<Json<ApiResponse<JournalEntryWithLines>>, AppError> {
    rbac::require_admin(&user.roles, "erp").map_err(|e| AppError::Forbidden(e))?;
    let entry = state
        .service
        .create_journal_entry(&input, &user.user_id)
        .await?;
    Ok(Json(ApiResponse::new(entry)))
}

pub async fn post_journal_entry(
    user: AuthUser,
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<ApiResponse<JournalEntryWithLines>>, AppError> {
    rbac::require_admin(&user.roles, "erp").map_err(|e| AppError::Forbidden(e))?;
    let entry = state.service.post_journal_entry(&id).await?;
    Ok(Json(ApiResponse::new(entry)))
}

pub async fn reverse_journal_entry(
    user: AuthUser,
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<ApiResponse<JournalEntryWithLines>>, AppError> {
    rbac::require_admin(&user.roles, "erp").map_err(|e| AppError::Forbidden(e))?;
    let entry = state.service.reverse_journal_entry(&id).await?;
    Ok(Json(ApiResponse::new(entry)))
}

pub async fn trial_balance(
    user: AuthUser,
    State(state): State<AppState>,
) -> Result<Json<ApiResponse<Vec<TrialBalanceRow>>>, AppError> {
    let _ = &user;
    let rows = state.service.trial_balance().await?;
    Ok(Json(ApiResponse::new(rows)))
}

pub async fn balance_sheet(
    user: AuthUser,
    State(state): State<AppState>,
) -> Result<Json<ApiResponse<Vec<BalanceSheetRow>>>, AppError> {
    let _ = &user;
    let rows = state.service.balance_sheet().await?;
    Ok(Json(ApiResponse::new(rows)))
}

pub async fn income_statement(
    user: AuthUser,
    State(state): State<AppState>,
    Query(query): Query<IncomeStatementQuery>,
) -> Result<Json<ApiResponse<IncomeStatement>>, AppError> {
    let _ = &user;
    let report = state
        .service
        .income_statement(&query.period_start, &query.period_end)
        .await?;
    Ok(Json(ApiResponse::new(report)))
}

// --- Budgets ---

pub async fn create_budget(
    user: AuthUser,
    State(state): State<AppState>,
    Json(input): Json<CreateBudgetRequest>,
) -> Result<Json<ApiResponse<BudgetWithLines>>, AppError> {
    rbac::require_admin(&user.roles, "erp").map_err(|e| AppError::Forbidden(e))?;
    let budget = state.service.create_budget(&input, &user.user_id).await?;
    Ok(Json(ApiResponse::new(budget)))
}

pub async fn list_budgets(
    user: AuthUser,
    State(state): State<AppState>,
) -> Result<Json<ApiResponse<Vec<Budget>>>, AppError> {
    let _ = &user;
    let budgets = state.service.list_budgets().await?;
    Ok(Json(ApiResponse::new(budgets)))
}

pub async fn get_budget(
    user: AuthUser,
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<ApiResponse<BudgetWithLines>>, AppError> {
    let _ = &user;
    let budget = state.service.get_budget(&id).await?;
    Ok(Json(ApiResponse::new(budget)))
}

pub async fn approve_budget(
    user: AuthUser,
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<ApiResponse<Budget>>, AppError> {
    rbac::require_admin(&user.roles, "erp").map_err(|e| AppError::Forbidden(e))?;
    let budget = state.service.approve_budget(&id).await?;
    Ok(Json(ApiResponse::new(budget)))
}

pub async fn activate_budget(
    user: AuthUser,
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<ApiResponse<Budget>>, AppError> {
    rbac::require_admin(&user.roles, "erp").map_err(|e| AppError::Forbidden(e))?;
    let budget = state.service.activate_budget(&id).await?;
    Ok(Json(ApiResponse::new(budget)))
}

pub async fn close_budget(
    user: AuthUser,
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<ApiResponse<Budget>>, AppError> {
    rbac::require_admin(&user.roles, "erp").map_err(|e| AppError::Forbidden(e))?;
    let budget = state.service.close_budget(&id).await?;
    Ok(Json(ApiResponse::new(budget)))
}

pub async fn budget_variance(
    user: AuthUser,
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<ApiResponse<BudgetVarianceReport>>, AppError> {
    let _ = &user;
    let report = state.service.budget_variance(&id).await?;
    Ok(Json(ApiResponse::new(report)))
}

pub async fn year_end_close(
    user: AuthUser,
    State(state): State<AppState>,
    Path(fiscal_year): Path<i64>,
) -> Result<Json<ApiResponse<JournalEntryWithLines>>, AppError> {
    rbac::require_admin(&user.roles, "erp").map_err(|e| AppError::Forbidden(e))?;
    let entry = state.service.year_end_close(fiscal_year).await?;
    Ok(Json(ApiResponse::new(entry)))
}
