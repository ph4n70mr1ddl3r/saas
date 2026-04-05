use axum::{routing::get, Router};
use sqlx::SqlitePool;
use saas_nats_bus::NatsBus;
use crate::handlers;
use crate::service::ArService;

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
        .route("/api/v1/customers", get(handlers::list_customers).post(handlers::create_customer))
        .route("/api/v1/customers/{id}", get(handlers::get_customer).put(handlers::update_customer))
        .route("/api/v1/ar-invoices", get(handlers::list_invoices).post(handlers::create_invoice))
        .route("/api/v1/ar-invoices/{id}", get(handlers::get_invoice))
        .route("/api/v1/receipts", get(handlers::list_receipts).post(handlers::create_receipt))
        .route("/health", get(|| async { axum::Json(serde_json::json!({"status": "ok"})) }))
        .with_state(state)
}
