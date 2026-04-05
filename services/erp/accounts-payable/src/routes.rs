use axum::{routing::get, Router};
use sqlx::SqlitePool;
use saas_nats_bus::NatsBus;
use crate::handlers;
use crate::service::ApService;

#[derive(Clone)]
pub struct AppState {
    pub service: ApService,
}

impl AppState {
    pub fn new(pool: SqlitePool, bus: NatsBus) -> Self {
        Self {
            service: ApService::new(pool, bus),
        }
    }
}

pub fn build_router(state: AppState) -> Router {
    Router::<AppState>::new()
        .route("/api/v1/vendors", get(handlers::list_vendors).post(handlers::create_vendor))
        .route("/api/v1/vendors/{id}", get(handlers::get_vendor).put(handlers::update_vendor))
        .route("/api/v1/invoices", get(handlers::list_invoices).post(handlers::create_invoice))
        .route("/api/v1/invoices/{id}", get(handlers::get_invoice))
        .route("/api/v1/invoices/{id}/approve", axum::routing::post(handlers::approve_invoice))
        .route("/api/v1/payments", get(handlers::list_payments).post(handlers::create_payment))
        .route("/health", get(|| async { axum::Json(serde_json::json!({"status": "ok"})) }))
        .with_state(state)
}
