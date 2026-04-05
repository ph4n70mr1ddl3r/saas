use axum::extract::{Path, State};
use axum::Json;
use saas_auth_core::extractor::AuthUser;
use saas_auth_core::rbac;
use saas_common::error::AppError;
use saas_common::response::ApiResponse;
use crate::models::SetConfigRequest;
use crate::routes::AppState;

const MAX_VALUE_SIZE: usize = 10 * 1024; // 10KB

pub async fn list_config(
    user: AuthUser,
    State(state): State<AppState>,
) -> Result<Json<ApiResponse<serde_json::Value>>, AppError> {
    // Authentication is enforced by the AuthUser extractor; no admin required for listing
    let _ = user;
    let entries = state.service.list().await?;
    Ok(Json(ApiResponse::new(serde_json::to_value(entries).unwrap())))
}

pub async fn get_config(
    user: AuthUser,
    State(state): State<AppState>,
    Path(key): Path<String>,
) -> Result<Json<ApiResponse<serde_json::Value>>, AppError> {
    // Authentication is enforced by the AuthUser extractor; no admin required for reading
    let _ = user;
    let entry = state.service.get(&key).await?;
    Ok(Json(ApiResponse::new(serde_json::to_value(entry).unwrap())))
}

pub async fn set_config(
    user: AuthUser,
    State(state): State<AppState>,
    Path(key): Path<String>,
    Json(input): Json<SetConfigRequest>,
) -> Result<Json<ApiResponse<serde_json::Value>>, AppError> {
    // RBAC: admin-only for writes
    rbac::require_admin(&user.roles, "admin")
        .map_err(|e| AppError::Forbidden(e))?;

    // Input validation on key
    validate_config_key(&key)?;

    // Input validation on value size
    if input.value.len() > MAX_VALUE_SIZE {
        return Err(AppError::Validation("Config value exceeds maximum size of 10KB".to_string()));
    }

    let entry = state.service.set(&key, &input.value).await?;
    Ok(Json(ApiResponse::new(serde_json::to_value(entry).unwrap())))
}

fn validate_config_key(key: &str) -> Result<(), AppError> {
    if key.is_empty() || key.len() > 255 {
        return Err(AppError::Validation("Config key must be 1-255 characters".to_string()));
    }
    let valid = key.chars().all(|c| {
        c.is_ascii_alphanumeric() || c == '_' || c == '-' || c == '/' || c == '.'
    });
    if !valid {
        return Err(AppError::Validation(
            "Config key contains invalid characters; only alphanumeric, underscore, hyphen, slash, and dot are allowed".to_string()
        ));
    }
    Ok(())
}
