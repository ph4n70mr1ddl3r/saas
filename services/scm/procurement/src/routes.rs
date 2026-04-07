use crate::handlers;
use crate::service::ProcurementService;
use axum::{
    routing::{get, post, put},
    Router,
};

#[derive(Clone)]
pub struct AppState {
    pub service: ProcurementService,
}

pub fn build_router(state: AppState) -> Router {
    Router::<AppState>::new()
        .route(
            "/api/v1/suppliers",
            get(handlers::list_suppliers).post(handlers::create_supplier),
        )
        .route(
            "/api/v1/suppliers/{id}",
            get(handlers::get_supplier).put(handlers::update_supplier),
        )
        .route(
            "/api/v1/purchase-orders",
            get(handlers::list_purchase_orders).post(handlers::create_purchase_order),
        )
        .route(
            "/api/v1/purchase-orders/{id}",
            get(handlers::get_purchase_order),
        )
        .route(
            "/api/v1/purchase-orders/{id}/submit",
            post(handlers::submit_purchase_order),
        )
        .route(
            "/api/v1/purchase-orders/{id}/approve",
            post(handlers::approve_purchase_order),
        )
        .route(
            "/api/v1/purchase-orders/{id}/receive",
            post(handlers::receive_purchase_order),
        )
        .route(
            "/api/v1/goods-receipts",
            get(handlers::list_goods_receipts),
        )
        .route(
            "/api/v1/goods-receipts/po/{po_id}",
            get(handlers::list_goods_receipts_by_po),
        )
        .route(
            "/health",
            get(|| async { axum::Json(serde_json::json!({"status": "ok"})) }),
        )
        .with_state(state)
}
