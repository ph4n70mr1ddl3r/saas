use axum::{routing::get, Router};
use crate::handlers;
use crate::service::PayrollService;

#[derive(Clone)]
pub struct AppState {
    pub service: PayrollService,
}

pub fn build_router(state: AppState) -> Router {
    Router::<AppState>::new()
        .route("/api/v1/compensation", get(handlers::list_compensation).post(handlers::create_compensation))
        .route("/api/v1/compensation/{id}", get(handlers::get_compensation).put(handlers::update_compensation))
        .route("/api/v1/compensation/employee/{employee_id}", get(handlers::list_compensation_by_employee))
        .route("/api/v1/pay-runs", get(handlers::list_pay_runs).post(handlers::create_pay_run))
        .route("/api/v1/pay-runs/{id}/process", axum::routing::post(handlers::process_pay_run))
        .route("/api/v1/pay-runs/{id}/payslips", get(handlers::list_payslips_for_run))
        .route("/api/v1/deductions/employee/{employee_id}", get(handlers::list_deductions_by_employee))
        .route("/api/v1/deductions", axum::routing::post(handlers::create_deduction))
        .route("/health", get(|| async { axum::Json(serde_json::json!({"status": "ok"})) }))
        .with_state(state)
}
