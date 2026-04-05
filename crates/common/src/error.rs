use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::Json;
use serde::Serialize;

#[derive(Debug, thiserror::Error)]
pub enum AppError {
    #[error("Not found: {0}")]
    NotFound(String),
    #[error("Validation error: {0}")]
    Validation(String),
    #[error("Unauthorized")]
    Unauthorized,
    #[error("Forbidden: {0}")]
    Forbidden(String),
    #[error("Conflict: {0}")]
    Conflict(String),
    #[error("Internal error: {0}")]
    Internal(String),
    #[error("Database error")]
    Database(#[from] sqlx::Error),
    #[error("NATS error")]
    Nats(#[from] async_nats::Error),
}

#[derive(Serialize)]
struct ErrorBody {
    error: ErrorDetail,
}

#[derive(Serialize)]
struct ErrorDetail {
    code: String,
    message: String,
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let (status, code, message) = match &self {
            AppError::NotFound(msg) => (StatusCode::NOT_FOUND, "NOT_FOUND", msg.clone()),
            AppError::Validation(msg) => (StatusCode::BAD_REQUEST, "VALIDATION_ERROR", msg.clone()),
            AppError::Unauthorized => (StatusCode::UNAUTHORIZED, "UNAUTHORIZED", "Authentication required".into()),
            AppError::Forbidden(msg) => (StatusCode::FORBIDDEN, "FORBIDDEN", msg.clone()),
            AppError::Conflict(msg) => (StatusCode::CONFLICT, "CONFLICT", msg.clone()),
            AppError::Internal(msg) => {
                tracing::error!("Internal error: {}", msg);
                (StatusCode::INTERNAL_SERVER_ERROR, "INTERNAL_ERROR", "Internal server error".into())
            }
            AppError::Database(e) => {
                // Log error kind without leaking SQL queries or PII
                tracing::error!(error_kind = %std::any::type_name_of_val(e), "Database error occurred");
                (StatusCode::INTERNAL_SERVER_ERROR, "INTERNAL_ERROR", "Database error".into())
            }
            AppError::Nats(e) => {
                tracing::error!(error_kind = %std::any::type_name_of_val(e), "NATS error occurred");
                (StatusCode::INTERNAL_SERVER_ERROR, "INTERNAL_ERROR", "Message bus error".into())
            }
        };

        (status, Json(ErrorBody {
            error: ErrorDetail { code: code.into(), message },
        })).into_response()
    }
}

pub type AppResult<T> = Result<T, AppError>;
