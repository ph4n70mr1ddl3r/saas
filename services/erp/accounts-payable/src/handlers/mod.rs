use axum::extract::{Path, State};
use axum::Json;
use saas_auth_core::extractor::AuthUser;
use saas_common::error::AppError;
use saas_common::response::ApiResponse;
use crate::models::*;
use crate::routes::AppState;

pub async fn list_vendors(
    _user: AuthUser,
    State(state): State<AppState>,
) -> Result<Json<ApiResponse<Vec<Vendor>>>, AppError> {
    let vendors = state.service.list_vendors().await?;
    Ok(Json(ApiResponse::new(vendors)))
}

pub async fn get_vendor(
    _user: AuthUser,
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<ApiResponse<Vendor>>, AppError> {
    let vendor = state.service.get_vendor(&id).await?;
    Ok(Json(ApiResponse::new(vendor)))
}

pub async fn create_vendor(
    _user: AuthUser,
    State(state): State<AppState>,
    Json(input): Json<CreateVendorRequest>,
) -> Result<Json<ApiResponse<Vendor>>, AppError> {
    let vendor = state.service.create_vendor(&input).await?;
    Ok(Json(ApiResponse::new(vendor)))
}

pub async fn update_vendor(
    _user: AuthUser,
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(input): Json<UpdateVendorRequest>,
) -> Result<Json<ApiResponse<Vendor>>, AppError> {
    let vendor = state.service.update_vendor(&id, &input).await?;
    Ok(Json(ApiResponse::new(vendor)))
}

pub async fn list_invoices(
    _user: AuthUser,
    State(state): State<AppState>,
) -> Result<Json<ApiResponse<Vec<ApInvoice>>>, AppError> {
    let invoices = state.service.list_invoices().await?;
    Ok(Json(ApiResponse::new(invoices)))
}

pub async fn get_invoice(
    _user: AuthUser,
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<ApiResponse<ApInvoiceWithLines>>, AppError> {
    let invoice = state.service.get_invoice(&id).await?;
    Ok(Json(ApiResponse::new(invoice)))
}

pub async fn create_invoice(
    _user: AuthUser,
    State(state): State<AppState>,
    Json(input): Json<CreateApInvoiceRequest>,
) -> Result<Json<ApiResponse<ApInvoiceWithLines>>, AppError> {
    let invoice = state.service.create_invoice(&input).await?;
    Ok(Json(ApiResponse::new(invoice)))
}

pub async fn approve_invoice(
    _user: AuthUser,
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<ApiResponse<ApInvoiceWithLines>>, AppError> {
    let invoice = state.service.approve_invoice(&id).await?;
    Ok(Json(ApiResponse::new(invoice)))
}

pub async fn list_payments(
    _user: AuthUser,
    State(state): State<AppState>,
) -> Result<Json<ApiResponse<Vec<Payment>>>, AppError> {
    let payments = state.service.list_payments().await?;
    Ok(Json(ApiResponse::new(payments)))
}

pub async fn create_payment(
    _user: AuthUser,
    State(state): State<AppState>,
    Json(input): Json<CreatePaymentRequest>,
) -> Result<Json<ApiResponse<Payment>>, AppError> {
    let payment = state.service.create_payment(&input).await?;
    Ok(Json(ApiResponse::new(payment)))
}
