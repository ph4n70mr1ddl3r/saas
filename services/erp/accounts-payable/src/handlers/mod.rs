use crate::models::*;
use crate::routes::AppState;
use axum::extract::{Path, Query, State};
use axum::Json;
use saas_auth_core::extractor::AuthUser;
use saas_auth_core::rbac;
use saas_common::error::AppError;
use saas_common::response::ApiResponse;
use serde::Deserialize;

pub async fn list_vendors(
    user: AuthUser,
    State(state): State<AppState>,
) -> Result<Json<ApiResponse<Vec<Vendor>>>, AppError> {
    let _ = &user;
    let vendors = state.service.list_vendors().await?;
    Ok(Json(ApiResponse::new(vendors)))
}

pub async fn get_vendor(
    user: AuthUser,
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<ApiResponse<Vendor>>, AppError> {
    let _ = &user;
    let vendor = state.service.get_vendor(&id).await?;
    Ok(Json(ApiResponse::new(vendor)))
}

pub async fn create_vendor(
    user: AuthUser,
    State(state): State<AppState>,
    Json(input): Json<CreateVendorRequest>,
) -> Result<Json<ApiResponse<Vendor>>, AppError> {
    rbac::require_admin(&user.roles, "erp").map_err(|e| AppError::Forbidden(e))?;
    let vendor = state.service.create_vendor(&input).await?;
    Ok(Json(ApiResponse::new(vendor)))
}

pub async fn update_vendor(
    user: AuthUser,
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(input): Json<UpdateVendorRequest>,
) -> Result<Json<ApiResponse<Vendor>>, AppError> {
    rbac::require_admin(&user.roles, "erp").map_err(|e| AppError::Forbidden(e))?;
    let vendor = state.service.update_vendor(&id, &input).await?;
    Ok(Json(ApiResponse::new(vendor)))
}

pub async fn list_invoices(
    user: AuthUser,
    State(state): State<AppState>,
) -> Result<Json<ApiResponse<Vec<ApInvoice>>>, AppError> {
    let _ = &user;
    let invoices = state.service.list_invoices().await?;
    Ok(Json(ApiResponse::new(invoices)))
}

pub async fn get_invoice(
    user: AuthUser,
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<ApiResponse<ApInvoiceWithLines>>, AppError> {
    let _ = &user;
    let invoice = state.service.get_invoice(&id).await?;
    Ok(Json(ApiResponse::new(invoice)))
}

pub async fn create_invoice(
    user: AuthUser,
    State(state): State<AppState>,
    Json(input): Json<CreateApInvoiceRequest>,
) -> Result<Json<ApiResponse<ApInvoiceWithLines>>, AppError> {
    rbac::require_admin(&user.roles, "erp").map_err(|e| AppError::Forbidden(e))?;
    let invoice = state.service.create_invoice(&input).await?;
    Ok(Json(ApiResponse::new(invoice)))
}

pub async fn approve_invoice(
    user: AuthUser,
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<ApiResponse<ApInvoiceWithLines>>, AppError> {
    rbac::require_admin(&user.roles, "erp").map_err(|e| AppError::Forbidden(e))?;
    let invoice = state.service.approve_invoice(&id).await?;
    Ok(Json(ApiResponse::new(invoice)))
}

pub async fn cancel_invoice(
    user: AuthUser,
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<ApiResponse<ApInvoice>>, AppError> {
    rbac::require_admin(&user.roles, "erp").map_err(|e| AppError::Forbidden(e))?;
    let invoice = state.service.cancel_invoice(&id).await?;
    Ok(Json(ApiResponse::new(invoice)))
}

pub async fn list_payments(
    user: AuthUser,
    State(state): State<AppState>,
) -> Result<Json<ApiResponse<Vec<Payment>>>, AppError> {
    let _ = &user;
    let payments = state.service.list_payments().await?;
    Ok(Json(ApiResponse::new(payments)))
}

pub async fn create_payment(
    user: AuthUser,
    State(state): State<AppState>,
    Json(input): Json<CreatePaymentRequest>,
) -> Result<Json<ApiResponse<Payment>>, AppError> {
    rbac::require_admin(&user.roles, "erp").map_err(|e| AppError::Forbidden(e))?;
    let payment = state.service.create_payment(&input).await?;
    Ok(Json(ApiResponse::new(payment)))
}

// --- Tax Codes ---

pub async fn list_tax_codes(
    user: AuthUser,
    State(state): State<AppState>,
) -> Result<Json<ApiResponse<Vec<TaxCode>>>, AppError> {
    let _ = &user;
    let tax_codes = state.service.list_tax_codes().await?;
    Ok(Json(ApiResponse::new(tax_codes)))
}

pub async fn create_tax_code(
    user: AuthUser,
    State(state): State<AppState>,
    Json(input): Json<CreateTaxCodeRequest>,
) -> Result<Json<ApiResponse<TaxCode>>, AppError> {
    rbac::require_admin(&user.roles, "erp").map_err(|e| AppError::Forbidden(e))?;
    let tax_code = state.service.create_tax_code(&input).await?;
    Ok(Json(ApiResponse::new(tax_code)))
}

// --- Aging Report ---

#[derive(Debug, Deserialize)]
pub struct AgingQuery {
    pub as_of_date: String,
}

pub async fn aging_report(
    user: AuthUser,
    State(state): State<AppState>,
    Query(params): Query<AgingQuery>,
) -> Result<Json<ApiResponse<ApAgingReport>>, AppError> {
    let _ = &user;
    let report = state.service.aging_report(&params.as_of_date).await?;
    Ok(Json(ApiResponse::new(report)))
}
