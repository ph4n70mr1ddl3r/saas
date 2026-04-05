use crate::handlers;
use crate::service::PerformanceService;
use axum::{routing::get, Router};

#[derive(Clone)]
pub struct AppState {
    pub service: PerformanceService,
}

pub fn build_router(state: AppState) -> Router {
    Router::<AppState>::new()
        .route(
            "/api/v1/review-cycles",
            get(handlers::list_review_cycles).post(handlers::create_review_cycle),
        )
        .route(
            "/api/v1/review-cycles/{id}",
            get(handlers::get_review_cycle),
        )
        .route(
            "/api/v1/review-cycles/{id}/activate",
            axum::routing::put(handlers::activate_review_cycle),
        )
        .route(
            "/api/v1/review-cycles/{id}/close",
            axum::routing::put(handlers::close_review_cycle),
        )
        .route(
            "/api/v1/goals",
            get(handlers::list_goals).post(handlers::create_goal),
        )
        .route(
            "/api/v1/goals/{id}",
            axum::routing::put(handlers::update_goal),
        )
        .route(
            "/api/v1/review-assignments",
            get(handlers::list_review_assignments).post(handlers::create_review_assignment),
        )
        .route(
            "/api/v1/review-assignments/{id}/submit",
            axum::routing::post(handlers::submit_review),
        )
        .route(
            "/api/v1/feedback",
            get(handlers::list_feedback).post(handlers::create_feedback),
        )
        .route(
            "/health",
            get(|| async { axum::Json(serde_json::json!({"status": "ok"})) }),
        )
        .with_state(state)
}
