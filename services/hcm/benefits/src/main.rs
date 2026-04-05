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

use routes::AppState;
use service::BenefitsService;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_setup::init("saas-hcm-benefits");

    let database_url = env::var("DATABASE_URL")
        .unwrap_or_else(|_| "sqlite:./data/benefits.db".into());
    let nats_url = env::var("NATS_URL")
        .unwrap_or_else(|_| "nats://localhost:4222".into());
    let port: u16 = env::var("PORT")
        .unwrap_or_else(|_| "8012".into())
        .parse()?;

    std::fs::create_dir_all("./data")?;

    let pool = create_pool(&database_url).await?;
    run_migrations(&pool, "./migrations").await?;

    let bus = NatsBus::connect(&nats_url).await?;

    let service = BenefitsService::new(pool, bus.clone());
    let app = routes::build_router(AppState { service })
        .layer(CorsLayer::permissive());

    events::subscribe(&bus).await?;

    let listener = tokio::net::TcpListener::bind(format!("0.0.0.0:{}", port)).await?;
    tracing::info!("Benefits service listening on port {}", port);
    axum::serve(listener, app).await?;

    Ok(())
}
