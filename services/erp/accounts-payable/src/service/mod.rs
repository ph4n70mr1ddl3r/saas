use sqlx::SqlitePool;
use saas_nats_bus::NatsBus;
use saas_common::error::{AppError, AppResult};
use crate::models::*;
use crate::repository::ApRepo;

#[derive(Clone)]
pub struct ApService {
    repo: ApRepo,
    bus: NatsBus,
}

impl ApService {
    pub fn new(pool: SqlitePool, bus: NatsBus) -> Self {
        Self {
            repo: ApRepo::new(pool),
            bus,
        }
    }

    // --- Vendors ---

    pub async fn list_vendors(&self) -> AppResult<Vec<Vendor>> {
        self.repo.list_vendors().await
    }

    pub async fn get_vendor(&self, id: &str) -> AppResult<Vendor> {
        self.repo.get_vendor(id).await
    }

    pub async fn create_vendor(&self, input: &CreateVendorRequest) -> AppResult<Vendor> {
        self.repo.create_vendor(input).await
    }

    pub async fn update_vendor(&self, id: &str, input: &UpdateVendorRequest) -> AppResult<Vendor> {
        self.repo.get_vendor(id).await?;
        self.repo.update_vendor(id, input).await
    }

    // --- Invoices ---

    pub async fn list_invoices(&self) -> AppResult<Vec<ApInvoice>> {
        self.repo.list_invoices().await
    }

    pub async fn get_invoice(&self, id: &str) -> AppResult<ApInvoiceWithLines> {
        let invoice = self.repo.get_invoice(id).await?;
        let lines = self.repo.get_invoice_lines(id).await?;
        Ok(ApInvoiceWithLines { invoice, lines })
    }

    pub async fn create_invoice(&self, input: &CreateApInvoiceRequest) -> AppResult<ApInvoiceWithLines> {
        // Validate vendor exists
        self.repo.get_vendor(&input.vendor_id).await?;
        let invoice = self.repo.create_invoice(input).await?;
        let lines = self.repo.get_invoice_lines(&invoice.id).await?;
        Ok(ApInvoiceWithLines { invoice, lines })
    }

    pub async fn approve_invoice(&self, id: &str) -> AppResult<ApInvoiceWithLines> {
        let invoice = self.repo.get_invoice(id).await?;
        if invoice.status != "draft" {
            return Err(AppError::Validation("Only draft invoices can be approved".into()));
        }

        let invoice = self.repo.approve_invoice(id).await?;
        let lines = self.repo.get_invoice_lines(id).await?;

        // Publish erp.ap.invoice.approved event
        let first_line = lines.first();
        let gl_account_code = first_line.map(|l| l.account_code.clone()).unwrap_or_default();
        let event = saas_proto::events::VendorInvoiceApproved {
            invoice_id: invoice.id.clone(),
            vendor_id: invoice.vendor_id.clone(),
            total_cents: invoice.total_cents,
            gl_account_code,
        };
        if let Err(e) = self.bus.publish("erp.ap.invoice.approved", &event).await {
            tracing::error!("Failed to publish invoice approved event: {}", e);
        }

        Ok(ApInvoiceWithLines { invoice, lines })
    }

    // --- Payments ---

    pub async fn list_payments(&self) -> AppResult<Vec<Payment>> {
        self.repo.list_payments().await
    }

    pub async fn create_payment(&self, input: &CreatePaymentRequest) -> AppResult<Payment> {
        // Validate invoice exists and is approved
        let invoice = self.repo.get_invoice(&input.invoice_id).await?;
        if invoice.status != "approved" {
            return Err(AppError::Validation("Can only pay approved invoices".into()));
        }
        self.repo.create_payment(input).await
    }
}
