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

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_setup::init("saas-scm-order-management");
    saas_auth_core::jwt::init_jwt_secret();
    let database_url =
        env::var("DATABASE_URL").unwrap_or_else(|_| "sqlite:./data/order-management.db".into());
    let nats_url = env::var("NATS_URL").unwrap_or_else(|_| "nats://localhost:4222".into());
    let port: u16 = env::var("PORT").unwrap_or_else(|_| "8032".into()).parse()?;
    std::fs::create_dir_all("./data")?;
    let pool = create_pool(&database_url).await?;
    run_migrations(&pool, "./migrations").await?;
    let bus = NatsBus::connect(&nats_url, "saas-scm-order-management").await?;
    let service = service::OrderManagementService::new(pool, bus.clone());
    events::register(&bus, service.clone()).await?;
    let app = routes::build_router(routes::AppState { service })
        .layer(saas_common::middleware::create_cors_layer())
        .layer(saas_common::middleware::create_trace_layer());
    let listener = tokio::net::TcpListener::bind(format!("0.0.0.0:{}", port)).await?;
    tracing::info!("Order management service listening on port {}", port);
    axum::serve(listener, app).await?;
    Ok(())
}
