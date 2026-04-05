use axum::body::Body;
use axum::http::{Method, Request, Response};
use axum::response::IntoResponse;
use reqwest::Client;

const ALLOWED_HEADERS: &[&str] = &[
    "content-type",
    "authorization",
    "accept",
    "x-request-id",
    "x-correlation-id",
];

const SAFE_RESPONSE_HEADERS: &[&str] = &[
    "content-type",
    "cache-control",
    "etag",
    "x-request-id",
    "location",
    "retry-after",
];

const MAX_RESPONSE_BODY: usize = 10 * 1024 * 1024; // 10 MiB

/// Sanitize a request URI to prevent path traversal attacks.
/// Handles percent-encoding of dots and double-encoding attempts.
fn sanitize_request_uri(uri: &axum::http::Uri) -> String {
    let path = uri.path();

    // Decode percent-encoded characters to normalize before checking
    let decoded_path = percent_decode_str(path);
    let normalized: String = decoded_path
        .split('/')
        .filter(|segment| {
            if segment.is_empty() {
                return false;
            }
            let seg = segment.replace("%2e", ".").replace("%2E", ".");
            seg != ".." && seg != "."
        })
        .collect::<Vec<_>>()
        .join("/");

    if let Some(query) = uri.query() {
        format!("/{}?{}", normalized, query)
    } else {
        format!("/{}", normalized)
    }
}

/// Percent-decode a string, replacing %XX sequences with their byte values.
fn percent_decode_str(s: &str) -> String {
    let mut result = Vec::new();
    let bytes = s.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            if let (Some(hi), Some(lo)) = (
                char::from(bytes[i + 1]).to_digit(16),
                char::from(bytes[i + 2]).to_digit(16),
            ) {
                result.push((hi * 16 + lo) as u8);
                i += 3;
                continue;
            }
        }
        result.push(bytes[i]);
        i += 1;
    }
    String::from_utf8_lossy(&result).into_owned()
}

pub async fn forward_request(
    req: Request<Body>,
    backend_url: &str,
    client: &Client,
    client_ip: &str,
) -> Response<Body> {
    let (parts, body) = req.into_parts();

    // Pass through HTTP methods faithfully, reject unknown ones
    let method = match parts.method {
        Method::GET => reqwest::Method::GET,
        Method::POST => reqwest::Method::POST,
        Method::PUT => reqwest::Method::PUT,
        Method::DELETE => reqwest::Method::DELETE,
        Method::PATCH => reqwest::Method::PATCH,
        Method::HEAD => reqwest::Method::HEAD,
        Method::OPTIONS => reqwest::Method::OPTIONS,
        _ => {
            return (
                axum::http::StatusCode::METHOD_NOT_ALLOWED,
                axum::Json(serde_json::json!({"error": {"code": "METHOD_NOT_ALLOWED", "message": "HTTP method not supported by gateway"}}))
            ).into_response();
        }
    };

    let sanitized_uri = sanitize_request_uri(&parts.uri);
    let url = format!("{}{}", backend_url, sanitized_uri);

    let bytes = match axum::body::to_bytes(body, 1024 * 1024).await {
        Ok(b) => b,
        Err(e) => {
            tracing::error!("Failed to read request body: {}", e);
            return (
                axum::http::StatusCode::BAD_REQUEST,
                axum::Json(serde_json::json!({"error": {"code": "BAD_REQUEST", "message": "Failed to read body"}}))
            ).into_response();
        }
    };

    let request_id = parts
        .headers
        .get("x-request-id")
        .and_then(|v| v.to_str().ok())
        .map(|s| s.to_string())
        .unwrap_or_else(|| uuid::Uuid::new_v4().to_string());

    let mut builder = client.request(method, &url);

    // Forward only allowed headers (prevent header injection)
    for (name, value) in parts.headers.iter() {
        if ALLOWED_HEADERS.contains(&name.as_str()) {
            builder = builder.header(name.as_str(), value);
        }
    }

    // Add gateway-controlled headers
    builder = builder.header("x-forwarded-for", client_ip);
    builder = builder.header("x-request-id", &request_id);

    if !bytes.is_empty() {
        builder = builder.body(bytes);
    }

    match builder.send().await {
        Ok(resp) => {
            let status = axum::http::StatusCode::from_u16(resp.status().as_u16()).unwrap_or_else(
                |fallback| {
                    tracing::warn!("Backend returned non-standard status code: {}", fallback);
                    axum::http::StatusCode::INTERNAL_SERVER_ERROR
                },
            );

            let mut response_builder = Response::builder().status(status);

            for (name, value) in resp.headers().iter() {
                if SAFE_RESPONSE_HEADERS.contains(&name.as_str()) {
                    if let Ok(v) = value.to_str() {
                        response_builder = response_builder.header(name.as_str(), v);
                    }
                }
            }

            match resp.bytes().await {
                Ok(body_bytes) => {
                    if body_bytes.len() > MAX_RESPONSE_BODY {
                        tracing::warn!(
                            "Backend response body exceeded {} bytes, truncating",
                            MAX_RESPONSE_BODY
                        );
                        return (
                            axum::http::StatusCode::BAD_GATEWAY,
                            axum::Json(serde_json::json!({"error": {"code": "BAD_GATEWAY", "message": "Backend response too large"}}))
                        ).into_response();
                    }
                    response_builder
                        .body(Body::from(body_bytes))
                        .unwrap_or_else(|_| {
                            Response::builder()
                                .status(axum::http::StatusCode::INTERNAL_SERVER_ERROR)
                                .body(Body::from("Proxy error"))
                                .unwrap()
                        })
                }
                Err(e) => {
                    tracing::error!("Failed to read backend response: {}", e);
                    (
                        axum::http::StatusCode::BAD_GATEWAY,
                        axum::Json(serde_json::json!({"error": {"code": "BAD_GATEWAY", "message": "Backend response read error"}}))
                    ).into_response()
                }
            }
        }
        Err(e) => {
            tracing::error!("Failed to connect to backend: {}", e);
            (
                axum::http::StatusCode::BAD_GATEWAY,
                axum::Json(serde_json::json!({"error": {"code": "BAD_GATEWAY", "message": "Backend unavailable"}}))
            ).into_response()
        }
    }
}
