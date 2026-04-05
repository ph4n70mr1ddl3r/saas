use crate::handlers;
use crate::service::EmployeeService;
use axum::{
    routing::{delete, get, post, put},
    Router,
};

#[derive(Clone)]
pub struct AppState {
    pub service: EmployeeService,
}

pub fn build_router(state: AppState) -> Router {
    Router::<AppState>::new()
        .route(
            "/api/v1/employees",
            get(handlers::list_employees).post(handlers::create_employee),
        )
        .route(
            "/api/v1/employees/{id}",
            get(handlers::get_employee)
                .put(handlers::update_employee)
                .delete(handlers::delete_employee),
        )
        .route(
            "/api/v1/employees/{id}/reports",
            get(handlers::get_direct_reports),
        )
        .route(
            "/api/v1/departments",
            get(handlers::list_departments).post(handlers::create_department),
        )
        .route(
            "/api/v1/departments/{id}",
            get(handlers::get_department).put(handlers::update_department),
        )
        .route("/api/v1/org-chart", get(handlers::get_org_chart))
        .route(
            "/health",
            get(|| async { axum::Json(serde_json::json!({"status": "ok"})) }),
        )
        .with_state(state)
}
