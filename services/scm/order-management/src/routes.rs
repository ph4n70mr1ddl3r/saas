use crate::handlers;
use crate::service::OrderManagementService;
use axum::{
    routing::{get, post},
    Router,
};

#[derive(Clone)]
pub struct AppState {
    pub service: OrderManagementService,
}

pub fn build_router(state: AppState) -> Router {
    Router::<AppState>::new()
        .route(
            "/api/v1/sales-orders",
            get(handlers::list_sales_orders).post(handlers::create_sales_order),
        )
        .route("/api/v1/sales-orders/{id}", get(handlers::get_sales_order))
        .route(
            "/api/v1/sales-orders/{id}/confirm",
            post(handlers::confirm_sales_order),
        )
        .route(
            "/api/v1/sales-orders/{id}/fulfill",
            post(handlers::fulfill_sales_order),
        )
        .route(
            "/api/v1/returns",
            get(handlers::list_returns).post(handlers::create_return),
        )
        .route("/api/v1/returns/{id}", get(handlers::get_return))
        .route(
            "/api/v1/fulfillments",
            get(handlers::list_fulfillments),
        )
        .route(
            "/api/v1/fulfillments/order/{order_id}",
            get(handlers::list_fulfillments_by_order),
        )
        .route(
            "/health",
            get(|| async { axum::Json(serde_json::json!({"status": "ok"})) }),
        )
        .with_state(state)
}
