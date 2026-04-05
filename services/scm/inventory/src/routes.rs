use axum::{routing::{get, post, delete}, Router};
use crate::handlers;
use crate::service::InventoryService;

#[derive(Clone)]
pub struct AppState {
    pub service: InventoryService,
}

pub fn build_router(state: AppState) -> Router {
    Router::<AppState>::new()
        .route("/api/v1/warehouses", get(handlers::list_warehouses).post(handlers::create_warehouse))
        .route("/api/v1/items", get(handlers::list_items).post(handlers::create_item))
        .route("/api/v1/items/{id}", get(handlers::get_item))
        .route("/api/v1/items/{id}/stock", get(handlers::get_item_stock))
        .route("/api/v1/items/{id}/availability", get(handlers::get_item_availability))
        .route("/api/v1/stock-movements", get(handlers::list_stock_movements).post(handlers::create_stock_movement))
        .route("/api/v1/reservations", get(handlers::list_reservations).post(handlers::create_reservation))
        .route("/api/v1/reservations/{id}", delete(handlers::cancel_reservation))
        .route("/health", get(|| async { axum::Json(serde_json::json!({"status": "ok"})) }))
        .with_state(state)
}
