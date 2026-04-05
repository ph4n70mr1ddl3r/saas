use axum::http::{header, HeaderValue, Method};
use std::env;
use tower_http::cors::CorsLayer;
use tower_http::trace::TraceLayer;

pub fn create_cors_layer() -> CorsLayer {
    let cors_origin =
        env::var("CORS_ORIGIN").unwrap_or_else(|_| "http://localhost:3000".to_string());

    let origins: Vec<HeaderValue> = cors_origin
        .split(',')
        .filter_map(|o| {
            let trimmed = o.trim();
            if trimmed.is_empty() {
                return None;
            }
            HeaderValue::from_str(trimmed).ok()
        })
        .collect();

    let cors = if origins.len() == 1 {
        CorsLayer::new().allow_origin(origins.into_iter().next().unwrap())
    } else {
        CorsLayer::new().allow_origin(origins)
    };

    cors.allow_methods([
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

pub fn create_trace_layer(
) -> TraceLayer<tower_http::classify::SharedClassifier<tower_http::classify::ServerErrorsAsFailures>>
{
    TraceLayer::new_for_http()
}
