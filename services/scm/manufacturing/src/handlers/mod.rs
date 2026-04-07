use saas_auth_core::rbac;
use saas_common::error::AppError;

pub async fn list_work_orders(
    _user: saas_auth_core::extractor::AuthUser,
    axum::extract::State(state): axum::extract::State<crate::routes::AppState>,
) -> Result<
    axum::Json<
        saas_common::response::ApiResponse<Vec<crate::models::work_order::WorkOrderResponse>>,
    >,
    saas_common::error::AppError,
> {
    let orders = state.service.list_work_orders().await?;
    Ok(axum::Json(saas_common::response::ApiResponse::new(orders)))
}

pub async fn create_work_order(
    user: saas_auth_core::extractor::AuthUser,
    axum::extract::State(state): axum::extract::State<crate::routes::AppState>,
    axum::Json(input): axum::Json<crate::models::work_order::CreateWorkOrder>,
) -> Result<
    (
        axum::http::StatusCode,
        axum::Json<
            saas_common::response::ApiResponse<crate::models::work_order::WorkOrderResponse>,
        >,
    ),
    saas_common::error::AppError,
> {
    rbac::require_admin(&user.roles, "scm").map_err(|e| AppError::Forbidden(e))?;
    let order = state.service.create_work_order(input).await?;
    Ok((
        axum::http::StatusCode::CREATED,
        axum::Json(saas_common::response::ApiResponse::new(order)),
    ))
}

pub async fn get_work_order(
    _user: saas_auth_core::extractor::AuthUser,
    axum::extract::State(state): axum::extract::State<crate::routes::AppState>,
    axum::extract::Path(id): axum::extract::Path<String>,
) -> Result<
    axum::Json<saas_common::response::ApiResponse<crate::models::work_order::WorkOrderResponse>>,
    saas_common::error::AppError,
> {
    let order = state.service.get_work_order(&id).await?;
    Ok(axum::Json(saas_common::response::ApiResponse::new(order)))
}

pub async fn start_work_order(
    user: saas_auth_core::extractor::AuthUser,
    axum::extract::State(state): axum::extract::State<crate::routes::AppState>,
    axum::extract::Path(id): axum::extract::Path<String>,
) -> Result<
    axum::Json<saas_common::response::ApiResponse<crate::models::work_order::WorkOrderResponse>>,
    saas_common::error::AppError,
> {
    rbac::require_admin(&user.roles, "scm").map_err(|e| AppError::Forbidden(e))?;
    let order = state.service.start_work_order(&id).await?;
    Ok(axum::Json(saas_common::response::ApiResponse::new(order)))
}

pub async fn complete_work_order(
    user: saas_auth_core::extractor::AuthUser,
    axum::extract::State(state): axum::extract::State<crate::routes::AppState>,
    axum::extract::Path(id): axum::extract::Path<String>,
) -> Result<
    axum::Json<saas_common::response::ApiResponse<crate::models::work_order::WorkOrderResponse>>,
    saas_common::error::AppError,
> {
    rbac::require_admin(&user.roles, "scm").map_err(|e| AppError::Forbidden(e))?;
    let order = state.service.complete_work_order(&id).await?;
    Ok(axum::Json(saas_common::response::ApiResponse::new(order)))
}

pub async fn cancel_work_order(
    user: saas_auth_core::extractor::AuthUser,
    axum::extract::State(state): axum::extract::State<crate::routes::AppState>,
    axum::extract::Path(id): axum::extract::Path<String>,
) -> Result<
    axum::Json<saas_common::response::ApiResponse<crate::models::work_order::WorkOrderResponse>>,
    saas_common::error::AppError,
> {
    rbac::require_admin(&user.roles, "scm").map_err(|e| AppError::Forbidden(e))?;
    let order = state.service.cancel_work_order(&id).await?;
    Ok(axum::Json(saas_common::response::ApiResponse::new(order)))
}

pub async fn list_boms(
    _user: saas_auth_core::extractor::AuthUser,
    axum::extract::State(state): axum::extract::State<crate::routes::AppState>,
) -> Result<
    axum::Json<saas_common::response::ApiResponse<Vec<crate::models::bom::BomResponse>>>,
    saas_common::error::AppError,
> {
    let boms = state.service.list_boms().await?;
    Ok(axum::Json(saas_common::response::ApiResponse::new(boms)))
}

pub async fn create_bom(
    user: saas_auth_core::extractor::AuthUser,
    axum::extract::State(state): axum::extract::State<crate::routes::AppState>,
    axum::Json(input): axum::Json<crate::models::bom::CreateBom>,
) -> Result<
    (
        axum::http::StatusCode,
        axum::Json<saas_common::response::ApiResponse<crate::models::bom::BomResponse>>,
    ),
    saas_common::error::AppError,
> {
    rbac::require_admin(&user.roles, "scm").map_err(|e| AppError::Forbidden(e))?;
    let bom = state.service.create_bom(input).await?;
    Ok((
        axum::http::StatusCode::CREATED,
        axum::Json(saas_common::response::ApiResponse::new(bom)),
    ))
}

pub async fn get_bom(
    _user: saas_auth_core::extractor::AuthUser,
    axum::extract::State(state): axum::extract::State<crate::routes::AppState>,
    axum::extract::Path(id): axum::extract::Path<String>,
) -> Result<
    axum::Json<saas_common::response::ApiResponse<crate::models::bom::BomDetailResponse>>,
    saas_common::error::AppError,
> {
    let bom = state.service.get_bom(&id).await?;
    Ok(axum::Json(saas_common::response::ApiResponse::new(bom)))
}
