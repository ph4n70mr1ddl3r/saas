use crate::models::role::{
    CreateRole, PermissionResponse, RoleResponse, SetPermissionsRequest, UpdateRole,
};
use crate::routes::AuthState;
use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::Json;
use saas_auth_core::extractor::AuthUser;
use saas_auth_core::rbac::is_admin;
use saas_common::error::AppError;
use saas_common::response::ApiResponse;

pub async fn list_roles(
    _user: AuthUser,
    State(state): State<AuthState>,
) -> Result<Json<ApiResponse<Vec<RoleResponse>>>, AppError> {
    let roles = state.role_service.list().await?;
    Ok(Json(ApiResponse::new(roles)))
}

pub async fn create_role(
    user: AuthUser,
    State(state): State<AuthState>,
    Json(input): Json<CreateRole>,
) -> Result<(StatusCode, Json<ApiResponse<RoleResponse>>), AppError> {
    if !is_admin(&user.roles) {
        return Err(AppError::Forbidden("Admin role required".into()));
    }
    let role = state.role_service.create(input).await?;
    Ok((StatusCode::CREATED, Json(ApiResponse::new(role))))
}

pub async fn get_role(
    _user: AuthUser,
    State(state): State<AuthState>,
    Path(id): Path<String>,
) -> Result<Json<ApiResponse<RoleResponse>>, AppError> {
    let role = state.role_service.get(&id).await?;
    Ok(Json(ApiResponse::new(role)))
}

pub async fn update_role(
    user: AuthUser,
    State(state): State<AuthState>,
    Path(id): Path<String>,
    Json(input): Json<UpdateRole>,
) -> Result<Json<ApiResponse<RoleResponse>>, AppError> {
    if !is_admin(&user.roles) {
        return Err(AppError::Forbidden("Admin role required".into()));
    }
    let role = state.role_service.update(&id, input).await?;
    Ok(Json(ApiResponse::new(role)))
}

pub async fn set_permissions(
    user: AuthUser,
    State(state): State<AuthState>,
    Path(id): Path<String>,
    Json(input): Json<SetPermissionsRequest>,
) -> Result<StatusCode, AppError> {
    if !is_admin(&user.roles) {
        return Err(AppError::Forbidden("Admin role required".into()));
    }
    state
        .role_service
        .set_permissions(&id, input.permission_ids)
        .await?;
    Ok(StatusCode::NO_CONTENT)
}

pub async fn list_permissions(
    _user: AuthUser,
    State(state): State<AuthState>,
) -> Result<Json<ApiResponse<Vec<PermissionResponse>>>, AppError> {
    let perms = state.role_service.list_permissions().await?;
    Ok(Json(ApiResponse::new(perms)))
}
