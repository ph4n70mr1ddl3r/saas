use axum::extract::{Path, State};
use axum::Json;
use saas_auth_core::extractor::AuthUser;
use saas_common::error::AppError;
use saas_common::response::ApiResponse;
use crate::models::{ConfigEntry, SetConfigRequest};
use crate::routes::AppState;

pub async fn list_config(
    _user: AuthUser,
    State(state): State<AppState>,
) -> Result<Json<ApiResponse<Vec<ConfigEntry>>>, AppError> {
    let entries = state.service.list().await?;
    Ok(Json(ApiResponse::new(entries)))
}

pub async fn get_config(
    _user: AuthUser,
    State(state): State<AppState>,
    Path(key): Path<String>,
) -> Result<Json<ApiResponse<ConfigEntry>>, AppError> {
    let entry = state.service.get(&key).await?;
    Ok(Json(ApiResponse::new(entry)))
}

pub async fn set_config(
    _user: AuthUser,
    State(state): State<AppState>,
    Path(key): Path<String>,
    Json(input): Json<SetConfigRequest>,
) -> Result<Json<ApiResponse<ConfigEntry>>, AppError> {
    let entry = state.service.set(&key, &input.value).await?;
    Ok(Json(ApiResponse::new(entry)))
}
