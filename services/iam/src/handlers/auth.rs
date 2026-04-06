use crate::models::user::{LoginRequest, LoginResponse};
use crate::routes::AuthState;
use axum::extract::State;
use axum::Json;
use saas_auth_core::extractor::AuthUser;
use saas_common::error::AppError;
use saas_common::response::ApiResponse;
use validator::Validate;

pub async fn login(
    State(state): State<AuthState>,
    Json(req): Json<LoginRequest>,
) -> Result<Json<ApiResponse<LoginResponse>>, AppError> {
    req.validate()
        .map_err(|e| AppError::Validation(e.to_string()))?;
    let response = state.auth_service.login(req).await?;
    Ok(Json(ApiResponse::new(response)))
}

pub async fn refresh(
    user: AuthUser,
    State(state): State<AuthState>,
) -> Result<Json<ApiResponse<LoginResponse>>, AppError> {
    let response = state.auth_service.refresh(&user.user_id).await?;
    Ok(Json(ApiResponse::new(response)))
}

pub async fn logout(
    user: AuthUser,
    State(state): State<AuthState>,
) -> Result<Json<ApiResponse<serde_json::Value>>, AppError> {
    let jti = user.jti.as_deref().unwrap_or("");
    if !jti.is_empty() {
        let exp = {
            // Use a default 24h window for the revocation expiry
            (chrono::Utc::now() + chrono::Duration::hours(24)).timestamp() as u64
        };
        state.auth_service.logout(&user.user_id, jti, exp).await?;
    }
    Ok(Json(ApiResponse::new(
        serde_json::json!({"message": "Logged out"}),
    )))
}
