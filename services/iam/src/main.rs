use saas_auth_core::revocation::RevocationCache;
use saas_common::tracing_setup;
use saas_db::{migrate::run_migrations, pool::create_pool};
use saas_nats_bus::NatsBus;
use std::env;
use std::sync::Arc;

mod events;
mod handlers;
mod models;
mod repository;
mod routes;
mod service;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_setup::init("saas-iam");

    saas_auth_core::jwt::init_jwt_secret();

    // Initialize revocation cache so logout actually works
    let revocation_cache = Arc::new(RevocationCache::new());
    saas_auth_core::extractor::set_revocation_cache(revocation_cache);

    let database_url = env::var("DATABASE_URL").unwrap_or_else(|_| "sqlite:./data/iam.db".into());
    let nats_url = env::var("NATS_URL").unwrap_or_else(|_| "nats://localhost:4222".into());
    let port: u16 = env::var("PORT").unwrap_or_else(|_| "8001".into()).parse()?;

    // Ensure data directory exists
    std::fs::create_dir_all("./data")?;

    let pool = create_pool(&database_url).await?;
    run_migrations(&pool, "./migrations").await?;

    let bus = NatsBus::connect(&nats_url, "saas-iam").await?;

    // Create services for event registration
    let (auth_service, user_service, role_service) = routes::build_services(pool, bus.clone());

    // Register event subscribers for audit logging
    if let Err(e) = events::register(&bus, auth_service.clone(), role_service.clone()).await {
        tracing::warn!("Failed to register event subscribers (non-fatal): {}", e);
    }

    let app = routes::build_router_from_services(auth_service, user_service, role_service)
        .layer(saas_common::middleware::create_cors_layer())
        .layer(saas_common::middleware::create_trace_layer());

    let listener = tokio::net::TcpListener::bind(format!("0.0.0.0:{}", port)).await?;
    tracing::info!("IAM service listening on port {}", port);
    axum::serve(listener, app).await?;

    Ok(())
}
