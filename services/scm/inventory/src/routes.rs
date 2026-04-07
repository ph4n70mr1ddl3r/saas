use crate::handlers;
use crate::service::InventoryService;
use axum::{
    routing::{delete, get, post, put},
    Router,
};

#[derive(Clone)]
pub struct AppState {
    pub service: InventoryService,
}

pub fn build_router(state: AppState) -> Router {
    Router::<AppState>::new()
        .route(
            "/api/v1/warehouses",
            get(handlers::list_warehouses).post(handlers::create_warehouse),
        )
        .route(
            "/api/v1/warehouses/{id}",
            put(handlers::update_warehouse),
        )
        .route(
            "/api/v1/items",
            get(handlers::list_items).post(handlers::create_item),
        )
        .route("/api/v1/items/{id}", get(handlers::get_item).put(handlers::update_item))
        .route(
            "/api/v1/items/below-reorder-point",
            get(handlers::list_items_below_reorder_point),
        )
        .route("/api/v1/items/{id}/stock", get(handlers::get_item_stock))
        .route(
            "/api/v1/items/{id}/availability",
            get(handlers::get_item_availability),
        )
        .route(
            "/api/v1/stock-movements",
            get(handlers::list_stock_movements).post(handlers::create_stock_movement),
        )
        .route(
            "/api/v1/reservations",
            get(handlers::list_reservations).post(handlers::create_reservation),
        )
        .route(
            "/api/v1/reservations/{id}",
            delete(handlers::cancel_reservation),
        )
        .route(
            "/api/v1/cycle-counts",
            get(handlers::list_cycle_count_sessions).post(handlers::create_cycle_count_session),
        )
        .route(
            "/api/v1/cycle-counts/{id}",
            get(handlers::get_cycle_count_session),
        )
        .route(
            "/api/v1/cycle-counts/{id}/lines",
            post(handlers::add_cycle_count_line),
        )
        .route(
            "/api/v1/cycle-counts/{id}/lines/{line_id}",
            put(handlers::update_counted_quantity),
        )
        .route(
            "/api/v1/cycle-counts/{id}/submit",
            post(handlers::submit_cycle_count),
        )
        .route(
            "/api/v1/cycle-counts/{id}/approve",
            post(handlers::approve_cycle_count),
        )
        .route(
            "/api/v1/cycle-counts/{id}/post",
            post(handlers::post_cycle_count),
        )
        .route(
            "/health",
            get(|| async { axum::Json(serde_json::json!({"status": "ok"})) }),
        )
        .with_state(state)
}
