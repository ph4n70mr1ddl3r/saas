pub async fn list_employees(
    _user: saas_auth_core::extractor::AuthUser,
    axum::extract::State(state): axum::extract::State<crate::routes::AppState>,
    axum::extract::Query(pag): axum::extract::Query<saas_common::pagination::PaginationParams>,
    axum::extract::Query(filters): axum::extract::Query<crate::models::employee::EmployeeFilters>,
) -> Result<axum::Json<saas_common::response::ApiListResponse<crate::models::employee::EmployeeResponse>>, saas_common::error::AppError> {
    let result = state.service.list_employees(&pag, &filters).await?;
    Ok(axum::Json(result))
}

pub async fn create_employee(
    _user: saas_auth_core::extractor::AuthUser,
    axum::extract::State(state): axum::extract::State<crate::routes::AppState>,
    axum::Json(input): axum::Json<crate::models::employee::CreateEmployee>,
) -> Result<(axum::http::StatusCode, axum::Json<saas_common::response::ApiResponse<crate::models::employee::EmployeeResponse>>), saas_common::error::AppError> {
    let emp = state.service.create_employee(input).await?;
    Ok((axum::http::StatusCode::CREATED, axum::Json(saas_common::response::ApiResponse::new(emp))))
}

pub async fn get_employee(
    _user: saas_auth_core::extractor::AuthUser,
    axum::extract::State(state): axum::extract::State<crate::routes::AppState>,
    axum::extract::Path(id): axum::extract::Path<String>,
) -> Result<axum::Json<saas_common::response::ApiResponse<crate::models::employee::EmployeeResponse>>, saas_common::error::AppError> {
    let emp = state.service.get_employee(&id).await?;
    Ok(axum::Json(saas_common::response::ApiResponse::new(emp)))
}

pub async fn update_employee(
    _user: saas_auth_core::extractor::AuthUser,
    axum::extract::State(state): axum::extract::State<crate::routes::AppState>,
    axum::extract::Path(id): axum::extract::Path<String>,
    axum::Json(input): axum::Json<crate::models::employee::UpdateEmployee>,
) -> Result<axum::Json<saas_common::response::ApiResponse<crate::models::employee::EmployeeResponse>>, saas_common::error::AppError> {
    let emp = state.service.update_employee(&id, input).await?;
    Ok(axum::Json(saas_common::response::ApiResponse::new(emp)))
}

pub async fn delete_employee(
    _user: saas_auth_core::extractor::AuthUser,
    axum::extract::State(state): axum::extract::State<crate::routes::AppState>,
    axum::extract::Path(id): axum::extract::Path<String>,
) -> Result<axum::Json<saas_common::response::ApiResponse<crate::models::employee::EmployeeResponse>>, saas_common::error::AppError> {
    let emp = state.service.delete_employee(&id).await?;
    Ok(axum::Json(saas_common::response::ApiResponse::new(emp)))
}

pub async fn get_direct_reports(
    _user: saas_auth_core::extractor::AuthUser,
    axum::extract::State(state): axum::extract::State<crate::routes::AppState>,
    axum::extract::Path(id): axum::extract::Path<String>,
) -> Result<axum::Json<saas_common::response::ApiResponse<Vec<crate::models::employee::EmployeeResponse>>>, saas_common::error::AppError> {
    let reports = state.service.get_direct_reports(&id).await?;
    Ok(axum::Json(saas_common::response::ApiResponse::new(reports)))
}

pub async fn list_departments(
    _user: saas_auth_core::extractor::AuthUser,
    axum::extract::State(state): axum::extract::State<crate::routes::AppState>,
) -> Result<axum::Json<saas_common::response::ApiResponse<Vec<crate::models::department::DepartmentResponse>>>, saas_common::error::AppError> {
    let depts = state.service.list_departments().await?;
    Ok(axum::Json(saas_common::response::ApiResponse::new(depts)))
}

pub async fn create_department(
    _user: saas_auth_core::extractor::AuthUser,
    axum::extract::State(state): axum::extract::State<crate::routes::AppState>,
    axum::Json(input): axum::Json<crate::models::department::CreateDepartment>,
) -> Result<(axum::http::StatusCode, axum::Json<saas_common::response::ApiResponse<crate::models::department::DepartmentResponse>>), saas_common::error::AppError> {
    let dept = state.service.create_department(input).await?;
    Ok((axum::http::StatusCode::CREATED, axum::Json(saas_common::response::ApiResponse::new(dept))))
}

pub async fn get_department(
    _user: saas_auth_core::extractor::AuthUser,
    axum::extract::State(state): axum::extract::State<crate::routes::AppState>,
    axum::extract::Path(id): axum::extract::Path<String>,
) -> Result<axum::Json<saas_common::response::ApiResponse<crate::models::department::DepartmentResponse>>, saas_common::error::AppError> {
    let dept = state.service.get_department(&id).await?;
    Ok(axum::Json(saas_common::response::ApiResponse::new(dept)))
}

pub async fn update_department(
    _user: saas_auth_core::extractor::AuthUser,
    axum::extract::State(state): axum::extract::State<crate::routes::AppState>,
    axum::extract::Path(id): axum::extract::Path<String>,
    axum::Json(input): axum::Json<crate::models::department::UpdateDepartment>,
) -> Result<axum::Json<saas_common::response::ApiResponse<crate::models::department::DepartmentResponse>>, saas_common::error::AppError> {
    let dept = state.service.update_department(&id, input).await?;
    Ok(axum::Json(saas_common::response::ApiResponse::new(dept)))
}

pub async fn get_org_chart(
    _user: saas_auth_core::extractor::AuthUser,
    axum::extract::State(state): axum::extract::State<crate::routes::AppState>,
) -> Result<axum::Json<saas_common::response::ApiResponse<Vec<crate::models::department::OrgChartNode>>>, saas_common::error::AppError> {
    let chart = state.service.get_org_chart().await?;
    Ok(axum::Json(saas_common::response::ApiResponse::new(chart)))
}
