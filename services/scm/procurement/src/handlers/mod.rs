use saas_auth_core::rbac;
use saas_common::error::AppError;

pub async fn list_suppliers(
    _user: saas_auth_core::extractor::AuthUser,
    axum::extract::State(state): axum::extract::State<crate::routes::AppState>,
) -> Result<axum::Json<saas_common::response::ApiResponse<Vec<crate::models::supplier::SupplierResponse>>>, saas_common::error::AppError> {
    let suppliers = state.service.list_suppliers().await?;
    Ok(axum::Json(saas_common::response::ApiResponse::new(suppliers)))
}

pub async fn create_supplier(
    user: saas_auth_core::extractor::AuthUser,
    axum::extract::State(state): axum::extract::State<crate::routes::AppState>,
    axum::Json(input): axum::Json<crate::models::supplier::CreateSupplier>,
) -> Result<(axum::http::StatusCode, axum::Json<saas_common::response::ApiResponse<crate::models::supplier::SupplierResponse>>), saas_common::error::AppError> {
    rbac::require_admin(&user.roles, "scm").map_err(|e| AppError::Forbidden(e))?;
    let supplier = state.service.create_supplier(input).await?;
    Ok((axum::http::StatusCode::CREATED, axum::Json(saas_common::response::ApiResponse::new(supplier))))
}

pub async fn get_supplier(
    _user: saas_auth_core::extractor::AuthUser,
    axum::extract::State(state): axum::extract::State<crate::routes::AppState>,
    axum::extract::Path(id): axum::extract::Path<String>,
) -> Result<axum::Json<saas_common::response::ApiResponse<crate::models::supplier::SupplierResponse>>, saas_common::error::AppError> {
    let supplier = state.service.get_supplier(&id).await?;
    Ok(axum::Json(saas_common::response::ApiResponse::new(supplier)))
}

pub async fn update_supplier(
    user: saas_auth_core::extractor::AuthUser,
    axum::extract::State(state): axum::extract::State<crate::routes::AppState>,
    axum::extract::Path(id): axum::extract::Path<String>,
    axum::Json(input): axum::Json<crate::models::supplier::UpdateSupplier>,
) -> Result<axum::Json<saas_common::response::ApiResponse<crate::models::supplier::SupplierResponse>>, saas_common::error::AppError> {
    rbac::require_admin(&user.roles, "scm").map_err(|e| AppError::Forbidden(e))?;
    let supplier = state.service.update_supplier(&id, input).await?;
    Ok(axum::Json(saas_common::response::ApiResponse::new(supplier)))
}

pub async fn list_purchase_orders(
    _user: saas_auth_core::extractor::AuthUser,
    axum::extract::State(state): axum::extract::State<crate::routes::AppState>,
) -> Result<axum::Json<saas_common::response::ApiResponse<Vec<crate::models::purchase_order::PurchaseOrderResponse>>>, saas_common::error::AppError> {
    let orders = state.service.list_purchase_orders().await?;
    Ok(axum::Json(saas_common::response::ApiResponse::new(orders)))
}

pub async fn create_purchase_order(
    user: saas_auth_core::extractor::AuthUser,
    axum::extract::State(state): axum::extract::State<crate::routes::AppState>,
    axum::Json(input): axum::Json<crate::models::purchase_order::CreatePurchaseOrder>,
) -> Result<(axum::http::StatusCode, axum::Json<saas_common::response::ApiResponse<crate::models::purchase_order::PurchaseOrderResponse>>), saas_common::error::AppError> {
    rbac::require_admin(&user.roles, "scm").map_err(|e| AppError::Forbidden(e))?;
    let order = state.service.create_purchase_order(input).await?;
    Ok((axum::http::StatusCode::CREATED, axum::Json(saas_common::response::ApiResponse::new(order))))
}

pub async fn get_purchase_order(
    _user: saas_auth_core::extractor::AuthUser,
    axum::extract::State(state): axum::extract::State<crate::routes::AppState>,
    axum::extract::Path(id): axum::extract::Path<String>,
) -> Result<axum::Json<saas_common::response::ApiResponse<crate::models::purchase_order::PurchaseOrderDetailResponse>>, saas_common::error::AppError> {
    let order = state.service.get_purchase_order(&id).await?;
    Ok(axum::Json(saas_common::response::ApiResponse::new(order)))
}

pub async fn submit_purchase_order(
    user: saas_auth_core::extractor::AuthUser,
    axum::extract::State(state): axum::extract::State<crate::routes::AppState>,
    axum::extract::Path(id): axum::extract::Path<String>,
) -> Result<axum::Json<saas_common::response::ApiResponse<crate::models::purchase_order::PurchaseOrderResponse>>, saas_common::error::AppError> {
    rbac::require_admin(&user.roles, "scm").map_err(|e| AppError::Forbidden(e))?;
    let order = state.service.submit_purchase_order(&id).await?;
    Ok(axum::Json(saas_common::response::ApiResponse::new(order)))
}

pub async fn approve_purchase_order(
    user: saas_auth_core::extractor::AuthUser,
    axum::extract::State(state): axum::extract::State<crate::routes::AppState>,
    axum::extract::Path(id): axum::extract::Path<String>,
) -> Result<axum::Json<saas_common::response::ApiResponse<crate::models::purchase_order::PurchaseOrderResponse>>, saas_common::error::AppError> {
    rbac::require_admin(&user.roles, "scm").map_err(|e| AppError::Forbidden(e))?;
    let order = state.service.approve_purchase_order(&id).await?;
    Ok(axum::Json(saas_common::response::ApiResponse::new(order)))
}

pub async fn receive_purchase_order(
    user: saas_auth_core::extractor::AuthUser,
    axum::extract::State(state): axum::extract::State<crate::routes::AppState>,
    axum::extract::Path(id): axum::extract::Path<String>,
    axum::Json(input): axum::Json<crate::models::purchase_order::ReceivePurchaseOrder>,
) -> Result<axum::Json<saas_common::response::ApiResponse<crate::models::purchase_order::PurchaseOrderDetailResponse>>, saas_common::error::AppError> {
    rbac::require_admin(&user.roles, "scm").map_err(|e| AppError::Forbidden(e))?;
    let order = state.service.receive_purchase_order(&id, input).await?;
    Ok(axum::Json(saas_common::response::ApiResponse::new(order)))
}
