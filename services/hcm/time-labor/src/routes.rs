use axum::{routing::get, Router};
use crate::handlers;
use crate::service::TimeLaborService;

#[derive(Clone)]
pub struct AppState {
    pub service: TimeLaborService,
}

pub fn build_router(state: AppState) -> Router {
    Router::<AppState>::new()
        .route("/api/v1/timesheets", get(handlers::list_timesheets).post(handlers::create_timesheet))
        .route("/api/v1/timesheets/employee/{employee_id}", get(handlers::list_timesheets_by_employee))
        .route("/api/v1/timesheets/{id}/submit", axum::routing::put(handlers::submit_timesheet))
        .route("/api/v1/timesheets/{id}/approve", axum::routing::put(handlers::approve_timesheet))
        .route("/api/v1/leave/requests", get(handlers::list_leave_requests).post(handlers::create_leave_request))
        .route("/api/v1/leave/requests/{id}/approve", axum::routing::put(handlers::approve_leave_request))
        .route("/api/v1/leave/requests/{id}/reject", axum::routing::put(handlers::reject_leave_request))
        .route("/api/v1/leave/balances/employee/{employee_id}", get(handlers::list_leave_balances_by_employee))
        .route("/health", get(|| async { axum::Json(serde_json::json!({"status": "ok"})) }))
        .with_state(state)
}
