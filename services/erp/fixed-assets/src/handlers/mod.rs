use axum::extract::{Path, State};
use axum::Json;
use saas_auth_core::extractor::AuthUser;
use saas_common::error::AppError;
use saas_common::response::ApiResponse;
use crate::models::*;
use crate::routes::AppState;

pub async fn list_assets(
    _user: AuthUser,
    State(state): State<AppState>,
) -> Result<Json<ApiResponse<Vec<Asset>>>, AppError> {
    let assets = state.service.list_assets().await?;
    Ok(Json(ApiResponse::new(assets)))
}

pub async fn get_asset(
    _user: AuthUser,
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<ApiResponse<Asset>>, AppError> {
    let asset = state.service.get_asset(&id).await?;
    Ok(Json(ApiResponse::new(asset)))
}

pub async fn create_asset(
    _user: AuthUser,
    State(state): State<AppState>,
    Json(input): Json<CreateAssetRequest>,
) -> Result<Json<ApiResponse<Asset>>, AppError> {
    let asset = state.service.create_asset(&input).await?;
    Ok(Json(ApiResponse::new(asset)))
}

pub async fn update_asset(
    _user: AuthUser,
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(input): Json<UpdateAssetRequest>,
) -> Result<Json<ApiResponse<Asset>>, AppError> {
    let asset = state.service.update_asset(&id, &input).await?;
    Ok(Json(ApiResponse::new(asset)))
}

pub async fn get_depreciation(
    _user: AuthUser,
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<ApiResponse<Vec<DepreciationSchedule>>>, AppError> {
    let schedule = state.service.get_depreciation(&id).await?;
    Ok(Json(ApiResponse::new(schedule)))
}

pub async fn run_depreciation(
    _user: AuthUser,
    State(state): State<AppState>,
    Json(input): Json<RunDepreciationRequest>,
) -> Result<Json<ApiResponse<Vec<DepreciationSchedule>>>, AppError> {
    let results = state.service.run_depreciation(&input.period).await?;
    Ok(Json(ApiResponse::new(results)))
}
