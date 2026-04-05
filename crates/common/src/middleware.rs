use tower_http::cors::CorsLayer;
use tower_http::trace::TraceLayer;
use axum::http::{HeaderValue, Method, header};
use std::env;

pub fn create_cors_layer() -> CorsLayer {
    let cors_origin = env::var("CORS_ORIGIN").unwrap_or_else(|_| "http://localhost:3000".to_string());
    
    CorsLayer::new()
        .allow_origin(HeaderValue::from_bytes(cors_origin.as_bytes()).expect("Invalid CORS origin"))
        .allow_methods([
            Method::GET,
            Method::POST,
            Method::PUT,
            Method::DELETE,
            Method::PATCH,
            Method::OPTIONS,
        ])
        .allow_headers([
            header::CONTENT_TYPE,
            header::AUTHORIZATION,
            header::ACCEPT,
            header::HeaderName::from_static("x-request-id"),
            header::HeaderName::from_static("x-correlation-id"),
        ])
}

pub fn create_trace_layer() -> TraceLayer<tower_http::classify::SharedClassifier<tower_http::classify::ServerErrorsAsFailures>> {
    TraceLayer::new_for_http()
}
