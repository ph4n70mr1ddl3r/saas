use axum::extract::{Path, State};
use axum::Json;
use saas_auth_core::extractor::AuthUser;
use saas_auth_core::rbac;
use saas_common::error::AppError;
use saas_common::response::ApiResponse;
use crate::models::*;
use crate::routes::AppState;

pub async fn list_customers(
    user: AuthUser,
    State(state): State<AppState>,
) -> Result<Json<ApiResponse<Vec<Customer>>>, AppError> {
    let _ = &user;
    let customers = state.service.list_customers().await?;
    Ok(Json(ApiResponse::new(customers)))
}

pub async fn get_customer(
    user: AuthUser,
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<ApiResponse<Customer>>, AppError> {
    let _ = &user;
    let customer = state.service.get_customer(&id).await?;
    Ok(Json(ApiResponse::new(customer)))
}

pub async fn create_customer(
    user: AuthUser,
    State(state): State<AppState>,
    Json(input): Json<CreateCustomerRequest>,
) -> Result<Json<ApiResponse<Customer>>, AppError> {
    rbac::require_admin(&user.roles, "erp").map_err(|e| AppError::Forbidden(e))?;
    let customer = state.service.create_customer(&input).await?;
    Ok(Json(ApiResponse::new(customer)))
}

pub async fn update_customer(
    user: AuthUser,
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(input): Json<UpdateCustomerRequest>,
) -> Result<Json<ApiResponse<Customer>>, AppError> {
    rbac::require_admin(&user.roles, "erp").map_err(|e| AppError::Forbidden(e))?;
    let customer = state.service.update_customer(&id, &input).await?;
    Ok(Json(ApiResponse::new(customer)))
}

pub async fn list_invoices(
    user: AuthUser,
    State(state): State<AppState>,
) -> Result<Json<ApiResponse<Vec<ArInvoice>>>, AppError> {
    let _ = &user;
    let invoices = state.service.list_invoices().await?;
    Ok(Json(ApiResponse::new(invoices)))
}

pub async fn get_invoice(
    user: AuthUser,
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<ApiResponse<ArInvoiceWithLines>>, AppError> {
    let _ = &user;
    let invoice = state.service.get_invoice(&id).await?;
    Ok(Json(ApiResponse::new(invoice)))
}

pub async fn create_invoice(
    user: AuthUser,
    State(state): State<AppState>,
    Json(input): Json<CreateArInvoiceRequest>,
) -> Result<Json<ApiResponse<ArInvoiceWithLines>>, AppError> {
    rbac::require_admin(&user.roles, "erp").map_err(|e| AppError::Forbidden(e))?;
    let invoice = state.service.create_invoice(&input).await?;
    Ok(Json(ApiResponse::new(invoice)))
}

pub async fn list_receipts(
    user: AuthUser,
    State(state): State<AppState>,
) -> Result<Json<ApiResponse<Vec<Receipt>>>, AppError> {
    let _ = &user;
    let receipts = state.service.list_receipts().await?;
    Ok(Json(ApiResponse::new(receipts)))
}

pub async fn create_receipt(
    user: AuthUser,
    State(state): State<AppState>,
    Json(input): Json<CreateReceiptRequest>,
) -> Result<Json<ApiResponse<Receipt>>, AppError> {
    rbac::require_admin(&user.roles, "erp").map_err(|e| AppError::Forbidden(e))?;
    let receipt = state.service.create_receipt(&input).await?;
    Ok(Json(ApiResponse::new(receipt)))
}
