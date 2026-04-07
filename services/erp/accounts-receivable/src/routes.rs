use crate::handlers;
use crate::service::ArService;
use axum::{routing::get, Router};
use saas_nats_bus::NatsBus;
use sqlx::SqlitePool;

#[derive(Clone)]
pub struct AppState {
    pub service: ArService,
}

impl AppState {
    pub fn new(pool: SqlitePool, bus: NatsBus) -> Self {
        Self {
            service: ArService::new(pool, bus),
        }
    }
}

pub fn build_router(state: AppState) -> Router {
    Router::<AppState>::new()
        .route(
            "/api/v1/customers",
            get(handlers::list_customers).post(handlers::create_customer),
        )
        .route(
            "/api/v1/customers/{id}",
            get(handlers::get_customer).put(handlers::update_customer),
        )
        .route(
            "/api/v1/ar-invoices",
            get(handlers::list_invoices).post(handlers::create_invoice),
        )
        .route("/api/v1/ar-invoices/{id}", get(handlers::get_invoice))
        .route(
            "/api/v1/ar-invoices/{id}/cancel",
            axum::routing::post(handlers::cancel_invoice),
        )
        .route(
            "/api/v1/ar-invoices/{id}/approve",
            axum::routing::post(handlers::approve_invoice),
        )
        .route(
            "/api/v1/receipts",
            get(handlers::list_receipts).post(handlers::create_receipt),
        )
        .route(
            "/api/v1/credit-memos",
            get(handlers::list_credit_memos).post(handlers::create_credit_memo),
        )
        .route(
            "/api/v1/credit-memos/{id}/apply",
            axum::routing::post(handlers::apply_credit_memo),
        )
        .route("/api/v1/ar-invoices/aging", get(handlers::aging_report))
        .route(
            "/health",
            get(|| async { axum::Json(serde_json::json!({"status": "ok"})) }),
        )
        .with_state(state)
}
