use axum::Router;
use saas_common::tracing_setup;
use saas_db::{pool::create_pool, migrate::run_migrations};
use saas_nats_bus::NatsBus;
use std::env;
use tower_http::cors::CorsLayer;

mod events;
mod handlers;
mod models;
mod repository;
mod routes;
mod service;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_setup::init("saas-hcm-time-labor");

    let database_url = env::var("DATABASE_URL")
        .unwrap_or_else(|_| "sqlite:./data/time_labor.db".into());
    let nats_url = env::var("NATS_URL")
        .unwrap_or_else(|_| "nats://localhost:4222".into());
    let port: u16 = env::var("PORT")
        .unwrap_or_else(|_| "8013".into())
        .parse()?;

    std::fs::create_dir_all("./data")?;

    let pool = create_pool(&database_url).await?;
    run_migrations(&pool, "./migrations").await?;

    let bus = NatsBus::connect(&nats_url).await?;

    let service = service::TimeLaborService::new(pool, bus.clone());
    let app = routes::build_router(routes::AppState { service })
        .layer(CorsLayer::permissive());

    events::subscribe(&bus).await?;

    let listener = tokio::net::TcpListener::bind(format!("0.0.0.0:{}", port)).await?;
    tracing::info!("Time & Labor service listening on port {}", port);
    axum::serve(listener, app).await?;

    Ok(())
}
