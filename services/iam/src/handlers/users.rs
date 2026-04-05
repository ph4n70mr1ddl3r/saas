use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::Json;
use saas_auth_core::extractor::AuthUser;
use saas_auth_core::rbac::is_admin;
use saas_common::error::AppError;
use saas_common::response::{ApiResponse, ApiListResponse};
use saas_common::pagination::PaginationParams;
use crate::models::user::{CreateUser, UpdateUser, ChangePassword, UserResponse};
use crate::models::role::AssignRolesRequest;
use crate::routes::AuthState;

pub async fn list_users(
    _user: AuthUser,
    State(state): State<AuthState>,
    Query(pag): Query<PaginationParams>,
) -> Result<Json<ApiListResponse<UserResponse>>, AppError> {
    let result = state.user_service.list(&pag).await?;
    Ok(Json(result))
}

pub async fn create_user(
    user: AuthUser,
    State(state): State<AuthState>,
    Json(input): Json<CreateUser>,
) -> Result<(StatusCode, Json<ApiResponse<UserResponse>>), AppError> {
    if !is_admin(&user.roles) {
        return Err(AppError::Forbidden("Admin role required".into()));
    }
    let user = state.user_service.create(input).await?;
    Ok((StatusCode::CREATED, Json(ApiResponse::new(user))))
}

pub async fn get_user(
    _user: AuthUser,
    State(state): State<AuthState>,
    Path(id): Path<String>,
) -> Result<Json<ApiResponse<UserResponse>>, AppError> {
    let user = state.user_service.get(&id).await?;
    Ok(Json(ApiResponse::new(user)))
}

pub async fn update_user(
    user: AuthUser,
    State(state): State<AuthState>,
    Path(id): Path<String>,
    Json(input): Json<UpdateUser>,
) -> Result<Json<ApiResponse<UserResponse>>, AppError> {
    // Users can update themselves, admins can update anyone
    if user.user_id != id && !is_admin(&user.roles) {
        return Err(AppError::Forbidden("Cannot update other users".into()));
    }
    let user = state.user_service.update(&id, input).await?;
    Ok(Json(ApiResponse::new(user)))
}

pub async fn delete_user(
    user: AuthUser,
    State(state): State<AuthState>,
    Path(id): Path<String>,
) -> Result<StatusCode, AppError> {
    if !is_admin(&user.roles) {
        return Err(AppError::Forbidden("Admin role required".into()));
    }
    state.user_service.delete(&id).await?;
    Ok(StatusCode::NO_CONTENT)
}

pub async fn change_password(
    user: AuthUser,
    State(state): State<AuthState>,
    Path(id): Path<String>,
    Json(input): Json<ChangePassword>,
) -> Result<StatusCode, AppError> {
    state.user_service.change_password(&user.user_id, &user.roles, &id, input).await?;
    Ok(StatusCode::NO_CONTENT)
}

pub async fn assign_roles(
    user: AuthUser,
    State(state): State<AuthState>,
    Path(id): Path<String>,
    Json(input): Json<AssignRolesRequest>,
) -> Result<StatusCode, AppError> {
    if !is_admin(&user.roles) {
        return Err(AppError::Forbidden("Admin role required".into()));
    }
    state.user_service.assign_roles(&id, input.role_ids).await?;
    Ok(StatusCode::NO_CONTENT)
}
