pub async fn list_warehouses(
    _user: saas_auth_core::extractor::AuthUser,
    axum::extract::State(state): axum::extract::State<crate::routes::AppState>,
) -> Result<axum::Json<saas_common::response::ApiResponse<Vec<crate::models::warehouse::WarehouseResponse>>>, saas_common::error::AppError> {
    let warehouses = state.service.list_warehouses().await?;
    Ok(axum::Json(saas_common::response::ApiResponse::new(warehouses)))
}

pub async fn create_warehouse(
    _user: saas_auth_core::extractor::AuthUser,
    axum::extract::State(state): axum::extract::State<crate::routes::AppState>,
    axum::Json(input): axum::Json<crate::models::warehouse::CreateWarehouse>,
) -> Result<(axum::http::StatusCode, axum::Json<saas_common::response::ApiResponse<crate::models::warehouse::WarehouseResponse>>), saas_common::error::AppError> {
    let warehouse = state.service.create_warehouse(input).await?;
    Ok((axum::http::StatusCode::CREATED, axum::Json(saas_common::response::ApiResponse::new(warehouse))))
}

pub async fn list_items(
    _user: saas_auth_core::extractor::AuthUser,
    axum::extract::State(state): axum::extract::State<crate::routes::AppState>,
    axum::extract::Query(filters): axum::extract::Query<crate::models::item::ItemFilters>,
) -> Result<axum::Json<saas_common::response::ApiResponse<Vec<crate::models::item::ItemResponse>>>, saas_common::error::AppError> {
    let items = state.service.list_items(&filters).await?;
    Ok(axum::Json(saas_common::response::ApiResponse::new(items)))
}

pub async fn create_item(
    _user: saas_auth_core::extractor::AuthUser,
    axum::extract::State(state): axum::extract::State<crate::routes::AppState>,
    axum::Json(input): axum::Json<crate::models::item::CreateItem>,
) -> Result<(axum::http::StatusCode, axum::Json<saas_common::response::ApiResponse<crate::models::item::ItemResponse>>), saas_common::error::AppError> {
    let item = state.service.create_item(input).await?;
    Ok((axum::http::StatusCode::CREATED, axum::Json(saas_common::response::ApiResponse::new(item))))
}

pub async fn get_item(
    _user: saas_auth_core::extractor::AuthUser,
    axum::extract::State(state): axum::extract::State<crate::routes::AppState>,
    axum::extract::Path(id): axum::extract::Path<String>,
) -> Result<axum::Json<saas_common::response::ApiResponse<crate::models::item::ItemResponse>>, saas_common::error::AppError> {
    let item = state.service.get_item(&id).await?;
    Ok(axum::Json(saas_common::response::ApiResponse::new(item)))
}

pub async fn get_item_stock(
    _user: saas_auth_core::extractor::AuthUser,
    axum::extract::State(state): axum::extract::State<crate::routes::AppState>,
    axum::extract::Path(id): axum::extract::Path<String>,
) -> Result<axum::Json<saas_common::response::ApiResponse<Vec<crate::models::stock_level::StockLevelResponse>>>, saas_common::error::AppError> {
    let stock = state.service.get_item_stock(&id).await?;
    Ok(axum::Json(saas_common::response::ApiResponse::new(stock)))
}

pub async fn get_item_availability(
    _user: saas_auth_core::extractor::AuthUser,
    axum::extract::State(state): axum::extract::State<crate::routes::AppState>,
    axum::extract::Path(id): axum::extract::Path<String>,
) -> Result<axum::Json<saas_common::response::ApiResponse<Vec<crate::models::stock_level::StockLevelResponse>>>, saas_common::error::AppError> {
    let availability = state.service.get_item_availability(&id).await?;
    Ok(axum::Json(saas_common::response::ApiResponse::new(availability)))
}

pub async fn list_stock_movements(
    _user: saas_auth_core::extractor::AuthUser,
    axum::extract::State(state): axum::extract::State<crate::routes::AppState>,
) -> Result<axum::Json<saas_common::response::ApiResponse<Vec<crate::models::stock_movement::StockMovementResponse>>>, saas_common::error::AppError> {
    let movements = state.service.list_stock_movements().await?;
    Ok(axum::Json(saas_common::response::ApiResponse::new(movements)))
}

pub async fn create_stock_movement(
    _user: saas_auth_core::extractor::AuthUser,
    axum::extract::State(state): axum::extract::State<crate::routes::AppState>,
    axum::Json(input): axum::Json<crate::models::stock_movement::CreateStockMovement>,
) -> Result<(axum::http::StatusCode, axum::Json<saas_common::response::ApiResponse<crate::models::stock_movement::StockMovementResponse>>), saas_common::error::AppError> {
    let movement = state.service.create_stock_movement(input).await?;
    Ok((axum::http::StatusCode::CREATED, axum::Json(saas_common::response::ApiResponse::new(movement))))
}

pub async fn list_reservations(
    _user: saas_auth_core::extractor::AuthUser,
    axum::extract::State(state): axum::extract::State<crate::routes::AppState>,
) -> Result<axum::Json<saas_common::response::ApiResponse<Vec<crate::models::reservation::ReservationResponse>>>, saas_common::error::AppError> {
    let reservations = state.service.list_reservations().await?;
    Ok(axum::Json(saas_common::response::ApiResponse::new(reservations)))
}

pub async fn create_reservation(
    _user: saas_auth_core::extractor::AuthUser,
    axum::extract::State(state): axum::extract::State<crate::routes::AppState>,
    axum::Json(input): axum::Json<crate::models::reservation::CreateReservation>,
) -> Result<(axum::http::StatusCode, axum::Json<saas_common::response::ApiResponse<crate::models::reservation::ReservationResponse>>), saas_common::error::AppError> {
    let reservation = state.service.create_reservation(input).await?;
    Ok((axum::http::StatusCode::CREATED, axum::Json(saas_common::response::ApiResponse::new(reservation))))
}

pub async fn cancel_reservation(
    _user: saas_auth_core::extractor::AuthUser,
    axum::extract::State(state): axum::extract::State<crate::routes::AppState>,
    axum::extract::Path(id): axum::extract::Path<String>,
) -> Result<axum::Json<saas_common::response::ApiResponse<crate::models::reservation::ReservationResponse>>, saas_common::error::AppError> {
    let reservation = state.service.cancel_reservation(&id).await?;
    Ok(axum::Json(saas_common::response::ApiResponse::new(reservation)))
}
