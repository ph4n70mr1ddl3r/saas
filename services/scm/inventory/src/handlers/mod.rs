use saas_auth_core::rbac;
use saas_common::error::AppError;

pub async fn list_warehouses(
    _user: saas_auth_core::extractor::AuthUser,
    axum::extract::State(state): axum::extract::State<crate::routes::AppState>,
) -> Result<
    axum::Json<
        saas_common::response::ApiResponse<Vec<crate::models::warehouse::WarehouseResponse>>,
    >,
    saas_common::error::AppError,
> {
    let warehouses = state.service.list_warehouses().await?;
    Ok(axum::Json(saas_common::response::ApiResponse::new(
        warehouses,
    )))
}

pub async fn create_warehouse(
    user: saas_auth_core::extractor::AuthUser,
    axum::extract::State(state): axum::extract::State<crate::routes::AppState>,
    axum::Json(input): axum::Json<crate::models::warehouse::CreateWarehouse>,
) -> Result<
    (
        axum::http::StatusCode,
        axum::Json<saas_common::response::ApiResponse<crate::models::warehouse::WarehouseResponse>>,
    ),
    saas_common::error::AppError,
> {
    rbac::require_admin(&user.roles, "scm").map_err(|e| AppError::Forbidden(e))?;
    let warehouse = state.service.create_warehouse(input).await?;
    Ok((
        axum::http::StatusCode::CREATED,
        axum::Json(saas_common::response::ApiResponse::new(warehouse)),
    ))
}

pub async fn update_warehouse(
    user: saas_auth_core::extractor::AuthUser,
    axum::extract::State(state): axum::extract::State<crate::routes::AppState>,
    axum::extract::Path(id): axum::extract::Path<String>,
    axum::Json(input): axum::Json<crate::models::warehouse::UpdateWarehouse>,
) -> Result<
    axum::Json<saas_common::response::ApiResponse<crate::models::warehouse::WarehouseResponse>>,
    saas_common::error::AppError,
> {
    rbac::require_admin(&user.roles, "scm").map_err(|e| AppError::Forbidden(e))?;
    let warehouse = state.service.update_warehouse(&id, input).await?;
    Ok(axum::Json(saas_common::response::ApiResponse::new(warehouse)))
}

pub async fn list_items(
    _user: saas_auth_core::extractor::AuthUser,
    axum::extract::State(state): axum::extract::State<crate::routes::AppState>,
    axum::extract::Query(filters): axum::extract::Query<crate::models::item::ItemFilters>,
) -> Result<
    axum::Json<saas_common::response::ApiResponse<Vec<crate::models::item::ItemResponse>>>,
    saas_common::error::AppError,
> {
    let items = state.service.list_items(&filters).await?;
    Ok(axum::Json(saas_common::response::ApiResponse::new(items)))
}

pub async fn create_item(
    user: saas_auth_core::extractor::AuthUser,
    axum::extract::State(state): axum::extract::State<crate::routes::AppState>,
    axum::Json(input): axum::Json<crate::models::item::CreateItem>,
) -> Result<
    (
        axum::http::StatusCode,
        axum::Json<saas_common::response::ApiResponse<crate::models::item::ItemResponse>>,
    ),
    saas_common::error::AppError,
> {
    rbac::require_admin(&user.roles, "scm").map_err(|e| AppError::Forbidden(e))?;
    let item = state.service.create_item(input).await?;
    Ok((
        axum::http::StatusCode::CREATED,
        axum::Json(saas_common::response::ApiResponse::new(item)),
    ))
}

pub async fn get_item(
    _user: saas_auth_core::extractor::AuthUser,
    axum::extract::State(state): axum::extract::State<crate::routes::AppState>,
    axum::extract::Path(id): axum::extract::Path<String>,
) -> Result<
    axum::Json<saas_common::response::ApiResponse<crate::models::item::ItemResponse>>,
    saas_common::error::AppError,
> {
    let item = state.service.get_item(&id).await?;
    Ok(axum::Json(saas_common::response::ApiResponse::new(item)))
}

pub async fn update_item(
    user: saas_auth_core::extractor::AuthUser,
    axum::extract::State(state): axum::extract::State<crate::routes::AppState>,
    axum::extract::Path(id): axum::extract::Path<String>,
    axum::Json(input): axum::Json<crate::models::item::UpdateItem>,
) -> Result<
    axum::Json<saas_common::response::ApiResponse<crate::models::item::ItemResponse>>,
    saas_common::error::AppError,
> {
    rbac::require_admin(&user.roles, "scm").map_err(|e| AppError::Forbidden(e))?;
    let item = state.service.update_item(&id, input).await?;
    Ok(axum::Json(saas_common::response::ApiResponse::new(item)))
}

pub async fn list_items_below_reorder_point(
    _user: saas_auth_core::extractor::AuthUser,
    axum::extract::State(state): axum::extract::State<crate::routes::AppState>,
) -> Result<
    axum::Json<saas_common::response::ApiResponse<Vec<crate::models::item::ItemResponse>>>,
    saas_common::error::AppError,
> {
    let items = state.service.list_items_below_reorder_point().await?;
    Ok(axum::Json(saas_common::response::ApiResponse::new(items)))
}

pub async fn get_item_stock(
    _user: saas_auth_core::extractor::AuthUser,
    axum::extract::State(state): axum::extract::State<crate::routes::AppState>,
    axum::extract::Path(id): axum::extract::Path<String>,
) -> Result<
    axum::Json<
        saas_common::response::ApiResponse<Vec<crate::models::stock_level::StockLevelResponse>>,
    >,
    saas_common::error::AppError,
> {
    let stock = state.service.get_item_stock(&id).await?;
    Ok(axum::Json(saas_common::response::ApiResponse::new(stock)))
}

pub async fn get_item_availability(
    _user: saas_auth_core::extractor::AuthUser,
    axum::extract::State(state): axum::extract::State<crate::routes::AppState>,
    axum::extract::Path(id): axum::extract::Path<String>,
) -> Result<
    axum::Json<
        saas_common::response::ApiResponse<Vec<crate::models::stock_level::StockLevelResponse>>,
    >,
    saas_common::error::AppError,
> {
    let availability = state.service.get_item_availability(&id).await?;
    Ok(axum::Json(saas_common::response::ApiResponse::new(
        availability,
    )))
}

pub async fn list_stock_movements(
    _user: saas_auth_core::extractor::AuthUser,
    axum::extract::State(state): axum::extract::State<crate::routes::AppState>,
) -> Result<
    axum::Json<
        saas_common::response::ApiResponse<
            Vec<crate::models::stock_movement::StockMovementResponse>,
        >,
    >,
    saas_common::error::AppError,
> {
    let movements = state.service.list_stock_movements().await?;
    Ok(axum::Json(saas_common::response::ApiResponse::new(
        movements,
    )))
}

pub async fn create_stock_movement(
    user: saas_auth_core::extractor::AuthUser,
    axum::extract::State(state): axum::extract::State<crate::routes::AppState>,
    axum::Json(input): axum::Json<crate::models::stock_movement::CreateStockMovement>,
) -> Result<
    (
        axum::http::StatusCode,
        axum::Json<
            saas_common::response::ApiResponse<
                crate::models::stock_movement::StockMovementResponse,
            >,
        >,
    ),
    saas_common::error::AppError,
> {
    rbac::require_admin(&user.roles, "scm").map_err(|e| AppError::Forbidden(e))?;
    let movement = state.service.create_stock_movement(input).await?;
    Ok((
        axum::http::StatusCode::CREATED,
        axum::Json(saas_common::response::ApiResponse::new(movement)),
    ))
}

pub async fn list_reservations(
    _user: saas_auth_core::extractor::AuthUser,
    axum::extract::State(state): axum::extract::State<crate::routes::AppState>,
) -> Result<
    axum::Json<
        saas_common::response::ApiResponse<Vec<crate::models::reservation::ReservationResponse>>,
    >,
    saas_common::error::AppError,
> {
    let reservations = state.service.list_reservations().await?;
    Ok(axum::Json(saas_common::response::ApiResponse::new(
        reservations,
    )))
}

pub async fn create_reservation(
    user: saas_auth_core::extractor::AuthUser,
    axum::extract::State(state): axum::extract::State<crate::routes::AppState>,
    axum::Json(input): axum::Json<crate::models::reservation::CreateReservation>,
) -> Result<
    (
        axum::http::StatusCode,
        axum::Json<
            saas_common::response::ApiResponse<crate::models::reservation::ReservationResponse>,
        >,
    ),
    saas_common::error::AppError,
> {
    rbac::require_admin(&user.roles, "scm").map_err(|e| AppError::Forbidden(e))?;
    let reservation = state.service.create_reservation(input).await?;
    Ok((
        axum::http::StatusCode::CREATED,
        axum::Json(saas_common::response::ApiResponse::new(reservation)),
    ))
}

pub async fn cancel_reservation(
    user: saas_auth_core::extractor::AuthUser,
    axum::extract::State(state): axum::extract::State<crate::routes::AppState>,
    axum::extract::Path(id): axum::extract::Path<String>,
) -> Result<
    axum::Json<saas_common::response::ApiResponse<crate::models::reservation::ReservationResponse>>,
    saas_common::error::AppError,
> {
    rbac::require_admin(&user.roles, "scm").map_err(|e| AppError::Forbidden(e))?;
    let reservation = state.service.cancel_reservation(&id).await?;
    Ok(axum::Json(saas_common::response::ApiResponse::new(
        reservation,
    )))
}

// Cycle Count handlers

pub async fn create_cycle_count_session(
    user: saas_auth_core::extractor::AuthUser,
    axum::extract::State(state): axum::extract::State<crate::routes::AppState>,
    axum::Json(input): axum::Json<crate::models::cycle_count::CreateCycleCountSessionRequest>,
) -> Result<
    (
        axum::http::StatusCode,
        axum::Json<
            saas_common::response::ApiResponse<crate::models::cycle_count::CycleCountSession>,
        >,
    ),
    saas_common::error::AppError,
> {
    rbac::require_admin(&user.roles, "scm").map_err(|e| AppError::Forbidden(e))?;
    let session = state
        .service
        .create_cycle_count_session(input, &user.user_id)
        .await?;
    Ok((
        axum::http::StatusCode::CREATED,
        axum::Json(saas_common::response::ApiResponse::new(session)),
    ))
}

pub async fn list_cycle_count_sessions(
    _user: saas_auth_core::extractor::AuthUser,
    axum::extract::State(state): axum::extract::State<crate::routes::AppState>,
) -> Result<
    axum::Json<
        saas_common::response::ApiResponse<Vec<crate::models::cycle_count::CycleCountSession>>,
    >,
    saas_common::error::AppError,
> {
    let sessions = state.service.list_cycle_count_sessions().await?;
    Ok(axum::Json(saas_common::response::ApiResponse::new(
        sessions,
    )))
}

pub async fn get_cycle_count_session(
    _user: saas_auth_core::extractor::AuthUser,
    axum::extract::State(state): axum::extract::State<crate::routes::AppState>,
    axum::extract::Path(id): axum::extract::Path<String>,
) -> Result<
    axum::Json<
        saas_common::response::ApiResponse<crate::models::cycle_count::CycleCountSessionWithLines>,
    >,
    saas_common::error::AppError,
> {
    let session = state.service.get_cycle_count_session(&id).await?;
    Ok(axum::Json(saas_common::response::ApiResponse::new(session)))
}

pub async fn add_cycle_count_line(
    user: saas_auth_core::extractor::AuthUser,
    axum::extract::State(state): axum::extract::State<crate::routes::AppState>,
    axum::extract::Path(id): axum::extract::Path<String>,
    axum::Json(input): axum::Json<crate::models::cycle_count::AddCycleCountLineRequest>,
) -> Result<
    (
        axum::http::StatusCode,
        axum::Json<saas_common::response::ApiResponse<crate::models::cycle_count::CycleCountLine>>,
    ),
    saas_common::error::AppError,
> {
    rbac::require_admin(&user.roles, "scm").map_err(|e| AppError::Forbidden(e))?;
    let line = state.service.add_cycle_count_line(&id, input).await?;
    Ok((
        axum::http::StatusCode::CREATED,
        axum::Json(saas_common::response::ApiResponse::new(line)),
    ))
}

pub async fn update_counted_quantity(
    user: saas_auth_core::extractor::AuthUser,
    axum::extract::State(state): axum::extract::State<crate::routes::AppState>,
    axum::extract::Path((session_id, line_id)): axum::extract::Path<(String, String)>,
    axum::Json(input): axum::Json<crate::models::cycle_count::UpdateCountedQuantityRequest>,
) -> Result<
    axum::Json<saas_common::response::ApiResponse<crate::models::cycle_count::CycleCountLine>>,
    saas_common::error::AppError,
> {
    rbac::require_admin(&user.roles, "scm").map_err(|e| AppError::Forbidden(e))?;
    let line = state
        .service
        .update_counted_quantity(&session_id, &line_id, input)
        .await?;
    Ok(axum::Json(saas_common::response::ApiResponse::new(line)))
}

pub async fn submit_cycle_count(
    user: saas_auth_core::extractor::AuthUser,
    axum::extract::State(state): axum::extract::State<crate::routes::AppState>,
    axum::extract::Path(id): axum::extract::Path<String>,
) -> Result<
    axum::Json<saas_common::response::ApiResponse<crate::models::cycle_count::CycleCountSession>>,
    saas_common::error::AppError,
> {
    rbac::require_admin(&user.roles, "scm").map_err(|e| AppError::Forbidden(e))?;
    let session = state.service.submit_cycle_count(&id).await?;
    Ok(axum::Json(saas_common::response::ApiResponse::new(session)))
}

pub async fn approve_cycle_count(
    user: saas_auth_core::extractor::AuthUser,
    axum::extract::State(state): axum::extract::State<crate::routes::AppState>,
    axum::extract::Path(id): axum::extract::Path<String>,
) -> Result<
    axum::Json<saas_common::response::ApiResponse<crate::models::cycle_count::CycleCountSession>>,
    saas_common::error::AppError,
> {
    rbac::require_admin(&user.roles, "scm").map_err(|e| AppError::Forbidden(e))?;
    let session = state
        .service
        .approve_cycle_count(&id, &user.user_id)
        .await?;
    Ok(axum::Json(saas_common::response::ApiResponse::new(session)))
}

pub async fn post_cycle_count(
    user: saas_auth_core::extractor::AuthUser,
    axum::extract::State(state): axum::extract::State<crate::routes::AppState>,
    axum::extract::Path(id): axum::extract::Path<String>,
) -> Result<
    axum::Json<saas_common::response::ApiResponse<crate::models::cycle_count::CycleCountSession>>,
    saas_common::error::AppError,
> {
    rbac::require_admin(&user.roles, "scm").map_err(|e| AppError::Forbidden(e))?;
    let session = state.service.post_cycle_count(&id, &user.user_id).await?;
    Ok(axum::Json(saas_common::response::ApiResponse::new(session)))
}
