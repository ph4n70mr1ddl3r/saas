use axum::{routing::{get, post}, Router};
use crate::handlers;
use crate::service::ManufacturingService;

#[derive(Clone)]
pub struct AppState {
    pub service: ManufacturingService,
}

pub fn build_router(state: AppState) -> Router {
    Router::<AppState>::new()
        .route("/api/v1/work-orders", get(handlers::list_work_orders).post(handlers::create_work_order))
        .route("/api/v1/work-orders/{id}", get(handlers::get_work_order))
        .route("/api/v1/work-orders/{id}/start", post(handlers::start_work_order))
        .route("/api/v1/work-orders/{id}/complete", post(handlers::complete_work_order))
        .route("/api/v1/bom", get(handlers::list_boms).post(handlers::create_bom))
        .route("/api/v1/bom/{id}", get(handlers::get_bom))
        .route("/health", get(|| async { axum::Json(serde_json::json!({"status": "ok"})) }))
        .with_state(state)
}
