use crate::handlers;
use crate::service::LedgerService;
use axum::{routing::get, Router};
use saas_nats_bus::NatsBus;
use sqlx::SqlitePool;

#[derive(Clone)]
pub struct AppState {
    pub service: LedgerService,
}

impl AppState {
    pub fn new(pool: SqlitePool, bus: NatsBus) -> Self {
        Self {
            service: LedgerService::new(pool, bus),
        }
    }
}

pub fn build_router(state: AppState) -> Router {
    Router::<AppState>::new()
        .route(
            "/api/v1/accounts",
            get(handlers::list_accounts).post(handlers::create_account),
        )
        .route("/api/v1/accounts/{id}", get(handlers::get_account))
        .route(
            "/api/v1/periods",
            get(handlers::list_periods).post(handlers::create_period),
        )
        .route(
            "/api/v1/periods/{id}/close",
            axum::routing::put(handlers::close_period),
        )
        .route(
            "/api/v1/journal-entries",
            get(handlers::list_journal_entries).post(handlers::create_journal_entry),
        )
        .route(
            "/api/v1/journal-entries/{id}",
            get(handlers::get_journal_entry),
        )
        .route(
            "/api/v1/journal-entries/{id}/post",
            axum::routing::post(handlers::post_journal_entry),
        )
        .route(
            "/api/v1/journal-entries/{id}/reverse",
            axum::routing::post(handlers::reverse_journal_entry),
        )
        .route("/api/v1/trial-balance", get(handlers::trial_balance))
        .route("/api/v1/balance-sheet", get(handlers::balance_sheet))
        .route("/api/v1/income-statement", get(handlers::income_statement))
        .route(
            "/api/v1/budgets",
            get(handlers::list_budgets).post(handlers::create_budget),
        )
        .route("/api/v1/budgets/{id}", get(handlers::get_budget))
        .route(
            "/api/v1/budgets/{id}/approve",
            axum::routing::post(handlers::approve_budget),
        )
        .route(
            "/api/v1/budgets/{id}/activate",
            axum::routing::post(handlers::activate_budget),
        )
        .route(
            "/api/v1/budgets/{id}/close",
            axum::routing::post(handlers::close_budget),
        )
        .route(
            "/api/v1/budgets/{id}/variance",
            get(handlers::budget_variance),
        )
        .route(
            "/api/v1/year-end-close/{fiscal_year}",
            axum::routing::post(handlers::year_end_close),
        )
        .route(
            "/health",
            get(|| async { axum::Json(serde_json::json!({"status": "ok"})) }),
        )
        .with_state(state)
}
