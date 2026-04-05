use saas_common::tracing_setup;
use std::env;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::RwLock;
use tower_http::cors::CorsLayer;
use tower_http::trace::TraceLayer;

mod proxy;
mod routes;
mod rate_limit;

use routes::AppState;

/// Periodically evict stale rate-limiter entries to prevent unbounded memory growth.
async fn cleanup_rate_limiter(rate_limiter: Arc<RwLock<HashMap<String, rate_limit::TokenBucket>>>) {
    loop {
        tokio::time::sleep(Duration::from_secs(300)).await;
        let mut map = rate_limiter.write().await;
        map.retain(|_, bucket| bucket.last_refill().elapsed().as_secs() < 600);
        map.shrink_to_fit();
    }
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_setup::init("saas-gateway");

    let port: u16 = env::var("PORT")
        .unwrap_or_else(|_| "8000".into())
        .parse()?;

    let mut service_map: HashMap<String, String> = HashMap::new();
    service_map.insert("iam".to_string(), env::var("IAM_URL").unwrap_or_else(|_| "http://localhost:8001".into()));
    service_map.insert("config".to_string(), env::var("CONFIG_URL").unwrap_or_else(|_| "http://localhost:8002".into()));
    service_map.insert("employee".to_string(), env::var("EMPLOYEE_URL").unwrap_or_else(|_| "http://localhost:8010".into()));
    service_map.insert("payroll".to_string(), env::var("PAYROLL_URL").unwrap_or_else(|_| "http://localhost:8011".into()));
    service_map.insert("benefits".to_string(), env::var("BENEFITS_URL").unwrap_or_else(|_| "http://localhost:8012".into()));
    service_map.insert("time-labor".to_string(), env::var("TIME_LABOR_URL").unwrap_or_else(|_| "http://localhost:8013".into()));
    service_map.insert("recruiting".to_string(), env::var("RECRUITING_URL").unwrap_or_else(|_| "http://localhost:8014".into()));
    service_map.insert("gl".to_string(), env::var("GL_URL").unwrap_or_else(|_| "http://localhost:8020".into()));
    service_map.insert("ap".to_string(), env::var("AP_URL").unwrap_or_else(|_| "http://localhost:8021".into()));
    service_map.insert("ar".to_string(), env::var("AR_URL").unwrap_or_else(|_| "http://localhost:8022".into()));
    service_map.insert("assets".to_string(), env::var("ASSETS_URL").unwrap_or_else(|_| "http://localhost:8023".into()));
    service_map.insert("cash".to_string(), env::var("CASH_URL").unwrap_or_else(|_| "http://localhost:8024".into()));
    service_map.insert("inventory".to_string(), env::var("INVENTORY_URL").unwrap_or_else(|_| "http://localhost:8030".into()));
    service_map.insert("procurement".to_string(), env::var("PROCUREMENT_URL").unwrap_or_else(|_| "http://localhost:8031".into()));
    service_map.insert("orders".to_string(), env::var("ORDERS_URL").unwrap_or_else(|_| "http://localhost:8032".into()));
    service_map.insert("manufacturing".to_string(), env::var("MANUFACTURING_URL").unwrap_or_else(|_| "http://localhost:8033".into()));

    let rate_limiter: Arc<RwLock<HashMap<String, rate_limit::TokenBucket>>> =
        Arc::new(RwLock::new(HashMap::new()));

    // Spawn background cleanup task
    tokio::spawn(cleanup_rate_limiter(rate_limiter.clone()));

    let state = AppState {
        service_map: Arc::new(RwLock::new(service_map)),
        http_client: reqwest::Client::new(),
        rate_limiter,
    };

    let app = routes::build_router(state)
        .layer(CorsLayer::permissive())
        .layer(TraceLayer::new_for_http())
        .into_make_service_with_connect_info::<std::net::SocketAddr>();

    let listener = tokio::net::TcpListener::bind(format!("0.0.0.0:{}", port)).await?;
    tracing::info!("API Gateway listening on port {}", port);
    axum::serve(listener, app).await?;

    Ok(())
}
