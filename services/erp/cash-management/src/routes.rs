use axum::{routing::get, Router};
use sqlx::SqlitePool;
use saas_nats_bus::NatsBus;
use crate::handlers;
use crate::service::CashManagementService;

#[derive(Clone)]
pub struct AppState {
    pub service: CashManagementService,
}

impl AppState {
    pub fn new(pool: SqlitePool, bus: NatsBus) -> Self {
        Self {
            service: CashManagementService::new(pool, bus),
        }
    }
}

pub fn build_router(state: AppState) -> Router {
    Router::<AppState>::new()
        .route("/api/v1/bank-accounts", get(handlers::list_bank_accounts).post(handlers::create_bank_account))
        .route("/api/v1/bank-accounts/{id}", get(handlers::get_bank_account))
        .route("/api/v1/bank-transactions", get(handlers::list_bank_transactions).post(handlers::create_bank_transaction))
        .route("/api/v1/reconciliations", get(handlers::list_reconciliations).post(handlers::create_reconciliation))
        .route("/health", get(|| async { axum::Json(serde_json::json!({"status": "ok"})) }))
        .with_state(state)
}
