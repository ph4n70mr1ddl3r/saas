use axum::Router;
use saas_common::tracing_setup;
use saas_db::{migrate::run_migrations, pool::create_pool};
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
    tracing_setup::init("saas-erp-cash-management");

    let database_url = env::var("DATABASE_URL")
        .unwrap_or_else(|_| "sqlite:./data/cash-management.db".into());
    let nats_url = env::var("NATS_URL")
        .unwrap_or_else(|_| "nats://localhost:4222".into());
    let port: u16 = env::var("PORT")
        .unwrap_or_else(|_| "8024".into())
        .parse()?;

    std::fs::create_dir_all("./data")?;

    let pool = create_pool(&database_url).await?;
    run_migrations(&pool, "./migrations").await?;

    let bus = NatsBus::connect(&nats_url).await?;

    let app_state = routes::AppState::new(pool.clone(), bus.clone());

    events::register(&bus, &app_state).await?;

    let app = routes::build_router(app_state).layer(CorsLayer::permissive());

    let listener = tokio::net::TcpListener::bind(format!("0.0.0.0:{}", port)).await?;
    tracing::info!("Cash Management service listening on port {}", port);
    axum::serve(listener, app).await?;

    Ok(())
}
