use axum::{routing::get, Router};
use sqlx::SqlitePool;
use saas_nats_bus::NatsBus;
use crate::handlers;
use crate::service::LedgerService;

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
        .route("/api/v1/accounts", get(handlers::list_accounts).post(handlers::create_account))
        .route("/api/v1/accounts/{id}", get(handlers::get_account))
        .route("/api/v1/periods", get(handlers::list_periods).post(handlers::create_period))
        .route("/api/v1/periods/{id}/close", axum::routing::put(handlers::close_period))
        .route("/api/v1/journal-entries", get(handlers::list_journal_entries).post(handlers::create_journal_entry))
        .route("/api/v1/journal-entries/{id}", get(handlers::get_journal_entry))
        .route("/api/v1/journal-entries/{id}/post", axum::routing::post(handlers::post_journal_entry))
        .route("/api/v1/journal-entries/{id}/reverse", axum::routing::post(handlers::reverse_journal_entry))
        .route("/api/v1/trial-balance", get(handlers::trial_balance))
        .route("/api/v1/balance-sheet", get(handlers::balance_sheet))
        .route("/health", get(|| async { axum::Json(serde_json::json!({"status": "ok"})) }))
        .with_state(state)
}
