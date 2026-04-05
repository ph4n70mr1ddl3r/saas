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
    _user: AuthUser,
    State(_state): State<AuthState>,
) -> Result<Json<ApiResponse<serde_json::Value>>, AppError> {
    Ok(Json(ApiResponse::new(
        serde_json::json!({"message": "Logged out"}),
    )))
}
