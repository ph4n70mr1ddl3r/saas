use saas_auth_core::jwt;
use saas_common::tracing_setup;
use saas_db::{pool::create_pool, migrate::run_migrations};
use saas_nats_bus::NatsBus;
use std::env;

mod handlers;
mod models;
mod repository;
mod service;
mod routes;
mod events;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_setup::init("saas-hcm-employee");
    jwt::init_jwt_secret();
    let database_url = env::var("DATABASE_URL").unwrap_or_else(|_| "sqlite:./data/employee.db".into());
    let nats_url = env::var("NATS_URL").unwrap_or_else(|_| "nats://localhost:4222".into());
    let port: u16 = env::var("PORT").unwrap_or_else(|_| "8010".into()).parse()?;
    std::fs::create_dir_all("./data")?;
    let pool = create_pool(&database_url).await?;
    run_migrations(&pool, "./migrations").await?;
    let bus = NatsBus::connect(&nats_url, "saas-hcm-employee").await?;
    let service = service::EmployeeService::new(pool, bus.clone());
    let state = routes::AppState { service };
    let app = routes::build_router(state)
        .layer(saas_common::middleware::create_cors_layer())
        .layer(saas_common::middleware::create_trace_layer());
    let listener = tokio::net::TcpListener::bind(format!("0.0.0.0:{}", port)).await?;
    tracing::info!("Employee service listening on port {}", port);
    axum::serve(listener, app).await?;
    Ok(())
}
