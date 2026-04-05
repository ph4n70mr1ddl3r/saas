use axum::{routing::get, Router, extract::State, http::Request, body::Body, response::Response, extract::ConnectInfo};
use reqwest::Client;
use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::RwLock;
use crate::proxy;
use crate::rate_limit;

#[derive(Clone)]
pub struct AppState {
    pub service_map: Arc<RwLock<HashMap<String, String>>>,
    pub http_client: Client,
    pub rate_limiter: Arc<RwLock<HashMap<String, rate_limit::TokenBucket>>>,
}

pub fn build_router(state: AppState) -> Router<()> {
    Router::new()
        .route("/health", get(health))
        .fallback(proxy_handler)
        .with_state(state)
}

async fn health() -> axum::Json<serde_json::Value> {
    axum::Json(serde_json::json!({"status": "ok", "service": "gateway"}))
}

async fn proxy_handler(
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    state: State<AppState>,
    req: Request<Body>,
) -> Response<Body> {
    let path = req.uri().path().to_string();

    // Rate limiting using real client IP
    let ip = req.headers().get("x-forwarded-for")
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.split(',').last())
        .map(|s| s.trim().to_string())
        .unwrap_or_else(|| addr.ip().to_string());

    {
        let mut limiters = state.rate_limiter.write().await;
        let bucket = limiters.entry(ip.clone())
            .or_insert_with(rate_limit::TokenBucket::new);
        if !bucket.try_consume() {
            return axum::response::IntoResponse::into_response(
                (axum::http::StatusCode::TOO_MANY_REQUESTS,
                 axum::Json(serde_json::json!({"error": {"code": "RATE_LIMITED", "message": "Too many requests"}})))
            );
        }
    }

    // Route to backend service based on path prefix
    let service_key = resolve_service(&path);
    let service_map = state.service_map.read().await;

    if let Some(backend_url) = service_map.get(&service_key) {
        proxy::forward_request(req, backend_url, &state.http_client, &ip).await
    } else {
        axum::response::IntoResponse::into_response(
            (axum::http::StatusCode::NOT_FOUND,
             axum::Json(serde_json::json!({"error": {"code": "NOT_FOUND", "message": "No service found for path"}})))
        )
    }
}

fn resolve_service(path: &str) -> String {
    // Map path prefixes to service keys
    if path.starts_with("/api/v1/auth") || path.starts_with("/api/v1/users") || path.starts_with("/api/v1/roles") || path.starts_with("/api/v1/permissions") {
        return "iam".to_string();
    }
    if path.starts_with("/api/v1/config") {
        return "config".to_string();
    }
    if path.starts_with("/api/v1/employees") || path.starts_with("/api/v1/departments") || path.starts_with("/api/v1/org-chart") {
        return "employee".to_string();
    }
    if path.starts_with("/api/v1/compensation") || path.starts_with("/api/v1/pay-runs") || path.starts_with("/api/v1/deductions") {
        return "payroll".to_string();
    }
    if path.starts_with("/api/v1/benefits") {
        return "benefits".to_string();
    }
    if path.starts_with("/api/v1/timesheets") || path.starts_with("/api/v1/leave") {
        return "time-labor".to_string();
    }
    if path.starts_with("/api/v1/jobs") || path.starts_with("/api/v1/applications") {
        return "recruiting".to_string();
    }
    if path.starts_with("/api/v1/accounts") || path.starts_with("/api/v1/periods") || path.starts_with("/api/v1/journal-entries") || path.starts_with("/api/v1/trial-balance") || path.starts_with("/api/v1/balance-sheet") {
        return "gl".to_string();
    }
    if path.starts_with("/api/v1/vendors") || path.starts_with("/api/v1/ap-invoices") {
        return "ap".to_string();
    }
    if path.starts_with("/api/v1/payments") {
        return "ap".to_string();
    }
    if path.starts_with("/api/v1/customers") || path.starts_with("/api/v1/ar-invoices") || path.starts_with("/api/v1/receipts") {
        return "ar".to_string();
    }
    if path.starts_with("/api/v1/assets") || path.starts_with("/api/v1/depreciation") {
        return "assets".to_string();
    }
    if path.starts_with("/api/v1/bank-accounts") || path.starts_with("/api/v1/reconciliations") {
        return "cash".to_string();
    }
    if path.starts_with("/api/v1/warehouses") || path.starts_with("/api/v1/items") || path.starts_with("/api/v1/stock-movements") || path.starts_with("/api/v1/reservations") {
        return "inventory".to_string();
    }
    if path.starts_with("/api/v1/suppliers") || path.starts_with("/api/v1/purchase-orders") {
        return "procurement".to_string();
    }
    if path.starts_with("/api/v1/sales-orders") || path.starts_with("/api/v1/fulfillments") || path.starts_with("/api/v1/returns") {
        return "orders".to_string();
    }
    if path.starts_with("/api/v1/work-orders") || path.starts_with("/api/v1/bom") {
        return "manufacturing".to_string();
    }
    "unknown".to_string()
}
