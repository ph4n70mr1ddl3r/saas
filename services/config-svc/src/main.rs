use saas_auth_core::jwt;
use saas_common::tracing_setup;
use saas_db::{migrate::run_migrations, pool::create_pool};
use saas_nats_bus::NatsBus;
use std::env;

mod events;
mod handlers;
mod models;
mod repository;
mod routes;
mod service;

use routes::AppState;
use service::ConfigService;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_setup::init("saas-config");
    jwt::init_jwt_secret();

    let database_url =
        env::var("DATABASE_URL").unwrap_or_else(|_| "sqlite:./data/config.db".into());
    let nats_url = env::var("NATS_URL").unwrap_or_else(|_| "nats://localhost:4222".into());
    let port: u16 = env::var("PORT").unwrap_or_else(|_| "8002".into()).parse()?;

    std::fs::create_dir_all("./data")?;

    let pool = create_pool(&database_url).await?;
    run_migrations(&pool, "./migrations").await?;

    let bus = NatsBus::connect(&nats_url, "saas-config").await?;

    let service = ConfigService::new(pool, bus.clone());
    events::subscribe(&bus, service.clone()).await?;
    let app = routes::build_router(AppState { service })
        .layer(saas_common::middleware::create_cors_layer())
        .layer(saas_common::middleware::create_trace_layer());

    let listener = tokio::net::TcpListener::bind(format!("0.0.0.0:{}", port)).await?;
    tracing::info!("Config service listening on port {}", port);
    axum::serve(listener, app).await?;

    Ok(())
}
