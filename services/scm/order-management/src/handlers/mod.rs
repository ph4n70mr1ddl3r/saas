use saas_auth_core::rbac;
use saas_common::error::AppError;

pub async fn list_sales_orders(
    _user: saas_auth_core::extractor::AuthUser,
    axum::extract::State(state): axum::extract::State<crate::routes::AppState>,
) -> Result<
    axum::Json<
        saas_common::response::ApiResponse<Vec<crate::models::sales_order::SalesOrderResponse>>,
    >,
    saas_common::error::AppError,
> {
    let orders = state.service.list_sales_orders().await?;
    Ok(axum::Json(saas_common::response::ApiResponse::new(orders)))
}

pub async fn create_sales_order(
    user: saas_auth_core::extractor::AuthUser,
    axum::extract::State(state): axum::extract::State<crate::routes::AppState>,
    axum::Json(input): axum::Json<crate::models::sales_order::CreateSalesOrder>,
) -> Result<
    (
        axum::http::StatusCode,
        axum::Json<
            saas_common::response::ApiResponse<crate::models::sales_order::SalesOrderResponse>,
        >,
    ),
    saas_common::error::AppError,
> {
    rbac::require_admin(&user.roles, "scm").map_err(|e| AppError::Forbidden(e))?;
    let order = state.service.create_sales_order(input).await?;
    Ok((
        axum::http::StatusCode::CREATED,
        axum::Json(saas_common::response::ApiResponse::new(order)),
    ))
}

pub async fn get_sales_order(
    _user: saas_auth_core::extractor::AuthUser,
    axum::extract::State(state): axum::extract::State<crate::routes::AppState>,
    axum::extract::Path(id): axum::extract::Path<String>,
) -> Result<
    axum::Json<
        saas_common::response::ApiResponse<crate::models::sales_order::SalesOrderDetailResponse>,
    >,
    saas_common::error::AppError,
> {
    let order = state.service.get_sales_order(&id).await?;
    Ok(axum::Json(saas_common::response::ApiResponse::new(order)))
}

pub async fn confirm_sales_order(
    user: saas_auth_core::extractor::AuthUser,
    axum::extract::State(state): axum::extract::State<crate::routes::AppState>,
    axum::extract::Path(id): axum::extract::Path<String>,
) -> Result<
    axum::Json<
        saas_common::response::ApiResponse<crate::models::sales_order::SalesOrderDetailResponse>,
    >,
    saas_common::error::AppError,
> {
    rbac::require_admin(&user.roles, "scm").map_err(|e| AppError::Forbidden(e))?;
    let order = state.service.confirm_sales_order(&id).await?;
    Ok(axum::Json(saas_common::response::ApiResponse::new(order)))
}

pub async fn fulfill_sales_order(
    user: saas_auth_core::extractor::AuthUser,
    axum::extract::State(state): axum::extract::State<crate::routes::AppState>,
    axum::extract::Path(id): axum::extract::Path<String>,
    axum::Json(input): axum::Json<crate::models::sales_order::FulfillRequest>,
) -> Result<
    axum::Json<
        saas_common::response::ApiResponse<crate::models::sales_order::SalesOrderDetailResponse>,
    >,
    saas_common::error::AppError,
> {
    rbac::require_admin(&user.roles, "scm").map_err(|e| AppError::Forbidden(e))?;
    let order = state.service.fulfill_sales_order(&id, input).await?;
    Ok(axum::Json(saas_common::response::ApiResponse::new(order)))
}

pub async fn list_returns(
    _user: saas_auth_core::extractor::AuthUser,
    axum::extract::State(state): axum::extract::State<crate::routes::AppState>,
) -> Result<
    axum::Json<
        saas_common::response::ApiResponse<Vec<crate::models::return_model::ReturnResponse>>,
    >,
    saas_common::error::AppError,
> {
    let returns = state.service.list_returns().await?;
    Ok(axum::Json(saas_common::response::ApiResponse::new(returns)))
}

pub async fn get_return(
    _user: saas_auth_core::extractor::AuthUser,
    axum::extract::State(state): axum::extract::State<crate::routes::AppState>,
    axum::extract::Path(id): axum::extract::Path<String>,
) -> Result<
    axum::Json<saas_common::response::ApiResponse<crate::models::return_model::ReturnResponse>>,
    saas_common::error::AppError,
> {
    let ret = state.service.get_return(&id).await?;
    Ok(axum::Json(saas_common::response::ApiResponse::new(ret)))
}

pub async fn create_return(
    user: saas_auth_core::extractor::AuthUser,
    axum::extract::State(state): axum::extract::State<crate::routes::AppState>,
    axum::Json(input): axum::Json<crate::models::return_model::CreateReturn>,
) -> Result<
    (
        axum::http::StatusCode,
        axum::Json<saas_common::response::ApiResponse<crate::models::return_model::ReturnResponse>>,
    ),
    saas_common::error::AppError,
> {
    rbac::require_admin(&user.roles, "scm").map_err(|e| AppError::Forbidden(e))?;
    let ret = state.service.create_return(input).await?;
    Ok((
        axum::http::StatusCode::CREATED,
        axum::Json(saas_common::response::ApiResponse::new(ret)),
    ))
}

pub async fn list_fulfillments(
    _user: saas_auth_core::extractor::AuthUser,
    axum::extract::State(state): axum::extract::State<crate::routes::AppState>,
) -> Result<
    axum::Json<
        saas_common::response::ApiResponse<Vec<crate::models::fulfillment::FulfillmentResponse>>,
    >,
    saas_common::error::AppError,
> {
    let fulfillments = state.service.list_fulfillments().await?;
    Ok(axum::Json(saas_common::response::ApiResponse::new(fulfillments)))
}

pub async fn list_fulfillments_by_order(
    _user: saas_auth_core::extractor::AuthUser,
    axum::extract::State(state): axum::extract::State<crate::routes::AppState>,
    axum::extract::Path(order_id): axum::extract::Path<String>,
) -> Result<
    axum::Json<
        saas_common::response::ApiResponse<Vec<crate::models::fulfillment::FulfillmentResponse>>,
    >,
    saas_common::error::AppError,
> {
    let fulfillments = state.service.list_fulfillments_by_order(&order_id).await?;
    Ok(axum::Json(saas_common::response::ApiResponse::new(fulfillments)))
}
