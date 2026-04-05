use axum::Router;
use saas_common::tracing_setup;
use saas_db::{pool::create_pool, migrate::run_migrations};
use saas_nats_bus::NatsBus;
use std::env;
use tower_http::cors::CorsLayer;

mod handlers;
mod models;
mod repository;
mod service;
mod routes;
mod events;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_setup::init("saas-scm-procurement");
    let database_url = env::var("DATABASE_URL").unwrap_or_else(|_| "sqlite:./data/procurement.db".into());
    let nats_url = env::var("NATS_URL").unwrap_or_else(|_| "nats://localhost:4222".into());
    let port: u16 = env::var("PORT").unwrap_or_else(|_| "8031".into()).parse()?;
    std::fs::create_dir_all("./data")?;
    let pool = create_pool(&database_url).await?;
    run_migrations(&pool, "./migrations").await?;
    let bus = NatsBus::connect(&nats_url).await?;
    events::register(&bus, pool.clone()).await?;
    let service = service::ProcurementService::new(pool, bus);
    let app = routes::build_router(routes::AppState { service }).layer(CorsLayer::permissive());
    let listener = tokio::net::TcpListener::bind(format!("0.0.0.0:{}", port)).await?;
    tracing::info!("Procurement service listening on port {}", port);
    axum::serve(listener, app).await?;
    Ok(())
}
