use crate::handlers;
use crate::service::RecruitingService;
use axum::{routing::get, Router};

#[derive(Clone)]
pub struct AppState {
    pub service: RecruitingService,
}

pub fn build_router(state: AppState) -> Router {
    Router::<AppState>::new()
        .route(
            "/api/v1/jobs",
            get(handlers::list_jobs).post(handlers::create_job),
        )
        .route(
            "/api/v1/jobs/{id}",
            get(handlers::get_job).put(handlers::update_job),
        )
        .route(
            "/api/v1/applications",
            get(handlers::list_applications).post(handlers::create_application),
        )
        .route(
            "/api/v1/applications/{id}",
            get(handlers::get_application),
        )
        .route(
            "/api/v1/applications/{id}/status",
            axum::routing::put(handlers::update_application_status),
        )
        .route(
            "/api/v1/applications/job/{job_id}",
            get(handlers::list_applications_by_job),
        )
        .route(
            "/health",
            get(|| async { axum::Json(serde_json::json!({"status": "ok"})) }),
        )
        .with_state(state)
}
