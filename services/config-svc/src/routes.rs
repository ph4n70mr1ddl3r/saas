use axum::{routing::{get, put}, Router};
use crate::handlers;
use crate::service::ConfigService;

#[derive(Clone)]
pub struct AppState {
    pub service: ConfigService,
}

pub fn build_router(state: AppState) -> Router {
    Router::<AppState>::new()
        .route("/api/v1/config", get(handlers::list_config))
        .route("/api/v1/config/{key}", get(handlers::get_config).put(handlers::set_config))
        .route("/health", get(|| async { axum::Json(serde_json::json!({"status": "ok"})) }))
        .with_state(state)
}
