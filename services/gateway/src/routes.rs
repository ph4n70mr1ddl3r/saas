use crate::proxy;
use crate::rate_limit;
use axum::{
    body::Body, extract::ConnectInfo, extract::State, http::Request, response::Response,
    routing::get, Router,
};
use reqwest::Client;
use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::sync::RwLock;

#[derive(Clone)]
pub struct AppState {
    pub service_map: Arc<HashMap<String, String>>,
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

    // Rate limiting using direct client IP only (not trusting X-Forwarded-For
    // unless deployed behind a known load balancer that strips/replaces it)
    let ip = addr.ip().to_string();

    {
        let mut limiters = state.rate_limiter.write().await;
        let bucket = limiters
            .entry(ip.clone())
            .or_insert_with(rate_limit::TokenBucket::new);
        if !bucket.try_consume() {
            return axum::response::IntoResponse::into_response((
                axum::http::StatusCode::TOO_MANY_REQUESTS,
                axum::Json(
                    serde_json::json!({"error": {"code": "RATE_LIMITED", "message": "Too many requests"}}),
                ),
            ));
        }
    }

    // Route to backend service based on path prefix
    let service_key = resolve_service(&path);

    if let Some(backend_url) = state.service_map.get(&service_key) {
        proxy::forward_request(req, backend_url, &state.http_client, &ip).await
    } else {
        axum::response::IntoResponse::into_response((
            axum::http::StatusCode::NOT_FOUND,
            axum::Json(
                serde_json::json!({"error": {"code": "NOT_FOUND", "message": "No service found for path"}}),
            ),
        ))
    }
}

fn resolve_service(path: &str) -> String {
    // Map path prefixes to service keys
    if path.starts_with("/api/v1/auth")
        || path.starts_with("/api/v1/users")
        || path.starts_with("/api/v1/roles")
        || path.starts_with("/api/v1/permissions")
    {
        return "iam".to_string();
    }
    if path.starts_with("/api/v1/config") {
        return "config".to_string();
    }
    if path.starts_with("/api/v1/employees")
        || path.starts_with("/api/v1/departments")
        || path.starts_with("/api/v1/org-chart")
    {
        return "employee".to_string();
    }
    if path.starts_with("/api/v1/compensation")
        || path.starts_with("/api/v1/pay-runs")
        || path.starts_with("/api/v1/deductions")
        || path.starts_with("/api/v1/tax-brackets")
    {
        return "payroll".to_string();
    }
    if path.starts_with("/api/v1/benefits") {
        return "benefits".to_string();
    }
    if path.starts_with("/api/v1/review-cycles")
        || path.starts_with("/api/v1/goals")
        || path.starts_with("/api/v1/review-assignments")
        || path.starts_with("/api/v1/feedback")
    {
        return "performance".to_string();
    }
    if path.starts_with("/api/v1/timesheets") || path.starts_with("/api/v1/leave") {
        return "time-labor".to_string();
    }
    if path.starts_with("/api/v1/jobs") || path.starts_with("/api/v1/applications") {
        return "recruiting".to_string();
    }
    if path.starts_with("/api/v1/accounts")
        || path.starts_with("/api/v1/periods")
        || path.starts_with("/api/v1/journal-entries")
        || path.starts_with("/api/v1/trial-balance")
        || path.starts_with("/api/v1/balance-sheet")
        || path.starts_with("/api/v1/income-statement")
        || path.starts_with("/api/v1/budgets")
        || path.starts_with("/api/v1/year-end-close")
    {
        return "gl".to_string();
    }
    if path.starts_with("/api/v1/vendors")
        || path.starts_with("/api/v1/invoices")
        || path.starts_with("/api/v1/payments")
        || path.starts_with("/api/v1/ap-invoices")
        || path.starts_with("/api/v1/ap-payments")
        || path.starts_with("/api/v1/tax-codes")
    {
        return "ap".to_string();
    }
    if path.starts_with("/api/v1/customers")
        || path.starts_with("/api/v1/ar-invoices")
        || path.starts_with("/api/v1/receipts")
        || path.starts_with("/api/v1/credit-memos")
    {
        return "ar".to_string();
    }
    if path.starts_with("/api/v1/assets") || path.starts_with("/api/v1/depreciation") {
        return "assets".to_string();
    }
    if path.starts_with("/api/v1/bank-accounts")
        || path.starts_with("/api/v1/reconciliations")
        || path.starts_with("/api/v1/bank-transactions")
        || path.starts_with("/api/v1/cash-flow-statement")
    {
        return "cash".to_string();
    }
    if path.starts_with("/api/v1/expense-categories")
        || path.starts_with("/api/v1/expense-reports")
        || path.starts_with("/api/v1/expense-lines")
        || path.starts_with("/api/v1/per-diems")
        || path.starts_with("/api/v1/mileage")
    {
        return "expense-mgmt".to_string();
    }
    if path.starts_with("/api/v1/warehouses")
        || path.starts_with("/api/v1/items")
        || path.starts_with("/api/v1/stock-movements")
        || path.starts_with("/api/v1/reservations")
        || path.starts_with("/api/v1/cycle-counts")
    {
        return "inventory".to_string();
    }
    if path.starts_with("/api/v1/suppliers")
        || path.starts_with("/api/v1/purchase-orders")
        || path.starts_with("/api/v1/goods-receipts")
    {
        return "procurement".to_string();
    }
    if path.starts_with("/api/v1/sales-orders")
        || path.starts_with("/api/v1/fulfillments")
        || path.starts_with("/api/v1/returns")
    {
        return "orders".to_string();
    }
    if path.starts_with("/api/v1/work-orders") || path.starts_with("/api/v1/bom") {
        return "manufacturing".to_string();
    }
    "unknown".to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_resolve_service_iam() {
        assert_eq!(resolve_service("/api/v1/auth/login"), "iam");
        assert_eq!(resolve_service("/api/v1/users"), "iam");
        assert_eq!(resolve_service("/api/v1/users/123"), "iam");
        assert_eq!(resolve_service("/api/v1/roles"), "iam");
        assert_eq!(resolve_service("/api/v1/permissions"), "iam");
    }

    #[test]
    fn test_resolve_service_employee() {
        assert_eq!(resolve_service("/api/v1/employees"), "employee");
        assert_eq!(resolve_service("/api/v1/employees/abc123"), "employee");
        assert_eq!(resolve_service("/api/v1/departments"), "employee");
        assert_eq!(resolve_service("/api/v1/org-chart"), "employee");
    }

    #[test]
    fn test_resolve_service_payroll() {
        assert_eq!(resolve_service("/api/v1/compensation"), "payroll");
        assert_eq!(resolve_service("/api/v1/pay-runs"), "payroll");
        assert_eq!(resolve_service("/api/v1/deductions"), "payroll");
        assert_eq!(resolve_service("/api/v1/tax-brackets"), "payroll");
    }

    #[test]
    fn test_resolve_service_gl() {
        assert_eq!(resolve_service("/api/v1/accounts"), "gl");
        assert_eq!(resolve_service("/api/v1/periods"), "gl");
        assert_eq!(resolve_service("/api/v1/journal-entries"), "gl");
        assert_eq!(resolve_service("/api/v1/trial-balance"), "gl");
        assert_eq!(resolve_service("/api/v1/balance-sheet"), "gl");
        assert_eq!(resolve_service("/api/v1/income-statement"), "gl");
        assert_eq!(resolve_service("/api/v1/budgets"), "gl");
        assert_eq!(resolve_service("/api/v1/year-end-close/2025"), "gl");
    }

    #[test]
    fn test_resolve_service_ap() {
        assert_eq!(resolve_service("/api/v1/vendors"), "ap");
        assert_eq!(resolve_service("/api/v1/invoices"), "ap");
        assert_eq!(resolve_service("/api/v1/payments"), "ap");
        assert_eq!(resolve_service("/api/v1/ap-invoices"), "ap");
        assert_eq!(resolve_service("/api/v1/ap-payments"), "ap");
        assert_eq!(resolve_service("/api/v1/tax-codes"), "ap");
    }

    #[test]
    fn test_resolve_service_ar() {
        assert_eq!(resolve_service("/api/v1/customers"), "ar");
        assert_eq!(resolve_service("/api/v1/ar-invoices"), "ar");
        assert_eq!(resolve_service("/api/v1/receipts"), "ar");
        assert_eq!(resolve_service("/api/v1/credit-memos"), "ar");
    }

    #[test]
    fn test_resolve_service_inventory() {
        assert_eq!(resolve_service("/api/v1/warehouses"), "inventory");
        assert_eq!(resolve_service("/api/v1/items"), "inventory");
        assert_eq!(resolve_service("/api/v1/stock-movements"), "inventory");
        assert_eq!(resolve_service("/api/v1/reservations"), "inventory");
        assert_eq!(resolve_service("/api/v1/cycle-counts"), "inventory");
    }

    #[test]
    fn test_resolve_service_procurement() {
        assert_eq!(resolve_service("/api/v1/suppliers"), "procurement");
        assert_eq!(resolve_service("/api/v1/purchase-orders"), "procurement");
        assert_eq!(resolve_service("/api/v1/goods-receipts"), "procurement");
    }

    #[test]
    fn test_resolve_service_orders() {
        assert_eq!(resolve_service("/api/v1/sales-orders"), "orders");
        assert_eq!(resolve_service("/api/v1/fulfillments"), "orders");
        assert_eq!(resolve_service("/api/v1/returns"), "orders");
    }

    #[test]
    fn test_resolve_service_manufacturing() {
        assert_eq!(resolve_service("/api/v1/work-orders"), "manufacturing");
        assert_eq!(resolve_service("/api/v1/bom"), "manufacturing");
    }

    #[test]
    fn test_resolve_service_others() {
        assert_eq!(resolve_service("/api/v1/config"), "config");
        assert_eq!(resolve_service("/api/v1/benefits"), "benefits");
        assert_eq!(resolve_service("/api/v1/review-cycles"), "performance");
        assert_eq!(resolve_service("/api/v1/timesheets"), "time-labor");
        assert_eq!(resolve_service("/api/v1/leave"), "time-labor");
        assert_eq!(resolve_service("/api/v1/jobs"), "recruiting");
        assert_eq!(resolve_service("/api/v1/assets"), "assets");
        assert_eq!(resolve_service("/api/v1/depreciation"), "assets");
        assert_eq!(resolve_service("/api/v1/bank-accounts"), "cash");
        assert_eq!(resolve_service("/api/v1/cash-flow-statement"), "cash");
        assert_eq!(resolve_service("/api/v1/expense-categories"), "expense-mgmt");
    }

    #[test]
    fn test_resolve_service_unknown() {
        assert_eq!(resolve_service("/api/v1/unknown"), "unknown");
        assert_eq!(resolve_service("/health"), "unknown");
    }
}
