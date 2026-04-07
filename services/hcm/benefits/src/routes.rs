use crate::handlers;
use crate::service::BenefitsService;
use axum::{routing::get, Router};

#[derive(Clone)]
pub struct AppState {
    pub service: BenefitsService,
}

pub fn build_router(state: AppState) -> Router {
    Router::<AppState>::new()
        .route(
            "/api/v1/benefits/plans",
            get(handlers::list_plans).post(handlers::create_plan),
        )
        .route(
            "/api/v1/benefits/plans/{id}",
            get(handlers::get_plan).put(handlers::update_plan),
        )
        .route(
            "/api/v1/benefits/plans/{id}/deactivate",
            axum::routing::put(handlers::deactivate_plan),
        )
        .route(
            "/api/v1/benefits/enrollments",
            get(handlers::list_enrollments).post(handlers::create_enrollment),
        )
        .route(
            "/api/v1/benefits/enrollments/{id}",
            get(handlers::get_enrollment),
        )
        .route(
            "/api/v1/benefits/enrollments/employee/{employee_id}",
            get(handlers::list_enrollments_by_employee),
        )
        .route(
            "/api/v1/benefits/enrollments/{id}/cancel",
            axum::routing::put(handlers::cancel_enrollment),
        )
        .route(
            "/health",
            get(|| async { axum::Json(serde_json::json!({"status": "ok"})) }),
        )
        .with_state(state)
}
