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
    tracing_setup::init("saas-scm-procurement");
    saas_auth_core::jwt::init_jwt_secret();
    let database_url =
        env::var("DATABASE_URL").unwrap_or_else(|_| "sqlite:./data/procurement.db".into());
    let nats_url = env::var("NATS_URL").unwrap_or_else(|_| "nats://localhost:4222".into());
    let port: u16 = env::var("PORT").unwrap_or_else(|_| "8031".into()).parse()?;
    std::fs::create_dir_all("./data")?;
    let pool = create_pool(&database_url).await?;
    run_migrations(&pool, "./migrations").await?;
    let bus = NatsBus::connect(&nats_url, "saas-scm-procurement").await?;
    events::register(&bus, pool.clone()).await?;
    let service = service::ProcurementService::new(pool, bus);
    let cors_origin =
        env::var("CORS_ORIGIN").unwrap_or_else(|_| "http://localhost:3000".to_string());
    let cors_origin =
        std::env::var("CORS_ORIGIN").unwrap_or_else(|_| "http://localhost:3000".to_string());
    let cors = CorsLayer::new()
        .allow_origin(
            axum::http::HeaderValue::from_bytes(cors_origin.as_bytes())
                .expect("Invalid CORS origin"),
        )
        .allow_methods([
            axum::http::Method::GET,
            axum::http::Method::POST,
            axum::http::Method::PUT,
            axum::http::Method::DELETE,
            axum::http::Method::PATCH,
        ])
        .allow_headers([
            axum::http::header::CONTENT_TYPE,
            axum::http::header::AUTHORIZATION,
        ]);
    let app = routes::build_router(routes::AppState { service }).layer(cors);
    let listener = tokio::net::TcpListener::bind(format!("0.0.0.0:{}", port)).await?;
    tracing::info!("Procurement service listening on port {}", port);
    axum::serve(listener, app).await?;
    Ok(())
}
