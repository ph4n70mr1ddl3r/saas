use crate::handlers;
use crate::service::FixedAssetsService;
use axum::{routing::get, Router};
use saas_nats_bus::NatsBus;
use sqlx::SqlitePool;

#[derive(Clone)]
pub struct AppState {
    pub service: FixedAssetsService,
}

impl AppState {
    pub fn new(pool: SqlitePool, bus: NatsBus) -> Self {
        Self {
            service: FixedAssetsService::new(pool, bus),
        }
    }
}

pub fn build_router(state: AppState) -> Router {
    Router::<AppState>::new()
        .route(
            "/api/v1/assets",
            get(handlers::list_assets).post(handlers::create_asset),
        )
        .route(
            "/api/v1/assets/{id}",
            get(handlers::get_asset).put(handlers::update_asset),
        )
        .route(
            "/api/v1/assets/{id}/depreciation",
            get(handlers::get_depreciation),
        )
        .route(
            "/api/v1/depreciation/run",
            axum::routing::post(handlers::run_depreciation),
        )
        .route(
            "/health",
            get(|| async { axum::Json(serde_json::json!({"status": "ok"})) }),
        )
        .with_state(state)
}
