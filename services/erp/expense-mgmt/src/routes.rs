use crate::handlers;
use crate::service::ExpenseService;
use axum::routing::{get, post};
use axum::Router;
use saas_nats_bus::NatsBus;
use sqlx::SqlitePool;

#[derive(Clone)]
pub struct AppState {
    pub service: ExpenseService,
}

impl AppState {
    pub fn new(pool: SqlitePool, bus: NatsBus) -> Self {
        Self {
            service: ExpenseService::new(pool, bus),
        }
    }
}

pub fn build_router(state: AppState) -> Router {
    Router::new()
        .route(
            "/api/v1/expense-categories",
            get(handlers::list_expense_categories).post(handlers::create_expense_category),
        )
        .route(
            "/api/v1/expense-categories/{id}",
            get(handlers::get_expense_category),
        )
        .route(
            "/api/v1/expense-reports",
            get(handlers::list_expense_reports).post(handlers::create_expense_report),
        )
        .route(
            "/api/v1/expense-reports/{id}",
            get(handlers::get_expense_report),
        )
        .route(
            "/api/v1/expense-reports/{id}/submit",
            axum::routing::post(handlers::submit_expense_report),
        )
        .route(
            "/api/v1/expense-reports/{id}/approve",
            axum::routing::post(handlers::approve_expense_report),
        )
        .route(
            "/api/v1/expense-reports/{id}/reject",
            axum::routing::post(handlers::reject_expense_report),
        )
        .route(
            "/api/v1/expense-reports/{id}/mark-paid",
            axum::routing::post(handlers::mark_expense_report_paid),
        )
        .route(
            "/api/v1/expense-lines",
            post(handlers::create_expense_line),
        )
        .route(
            "/api/v1/per-diems",
            get(handlers::list_per_diems).post(handlers::create_per_diem),
        )
        .route(
            "/api/v1/mileage",
            get(handlers::list_mileage).post(handlers::create_mileage),
        )
        .route(
            "/health",
            get(|| async { axum::Json(serde_json::json!({"status": "ok"})) }),
        )
        .with_state(state)
}
