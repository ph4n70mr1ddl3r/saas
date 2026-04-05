use crate::models::*;
use crate::repository::ApRepo;
use saas_common::error::{AppError, AppResult};
use saas_nats_bus::NatsBus;
use sqlx::SqlitePool;

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

    pub async fn create_invoice(
        &self,
        input: &CreateApInvoiceRequest,
    ) -> AppResult<ApInvoiceWithLines> {
        // Validate vendor exists
        self.repo.get_vendor(&input.vendor_id).await?;

        // Validate line amounts are non-negative
        for line in &input.lines {
            if line.amount_cents < 0 {
                return Err(AppError::Validation(
                    "Invoice line amounts must be non-negative".into(),
                ));
            }
        }

        let invoice = self.repo.create_invoice(input).await?;
        let lines = self.repo.get_invoice_lines(&invoice.id).await?;
        Ok(ApInvoiceWithLines { invoice, lines })
    }

    pub async fn approve_invoice(&self, id: &str) -> AppResult<ApInvoiceWithLines> {
        let invoice = self.repo.get_invoice(id).await?;
        if invoice.status != "draft" {
            return Err(AppError::Validation(
                "Only draft invoices can be approved".into(),
            ));
        }

        let invoice = self.repo.approve_invoice(id).await?;
        let lines = self.repo.get_invoice_lines(id).await?;

        // Publish erp.ap.invoice.approved event
        let first_line = lines.first();
        let gl_account_code = first_line
            .map(|l| l.account_code.clone())
            .unwrap_or_default();
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
        // Validate amount is positive
        if input.amount_cents <= 0 {
            return Err(AppError::Validation(
                "Payment amount must be positive".into(),
            ));
        }

        // Validate invoice exists and is approved
        let invoice = self.repo.get_invoice(&input.invoice_id).await?;
        if invoice.status != "approved" && invoice.status != "partial" {
            return Err(AppError::Validation(
                "Can only pay approved or partially paid invoices".into(),
            ));
        }

        // Verify vendor_id matches the invoice
        if invoice.vendor_id != input.vendor_id {
            return Err(AppError::Validation(
                "Vendor ID does not match invoice".into(),
            ));
        }

        self.repo.create_payment(input).await
    }

    // --- Tax Codes ---

    pub async fn create_tax_code(&self, input: &CreateTaxCodeRequest) -> AppResult<TaxCode> {
        if input.rate < 0.0 || input.rate > 1.0 {
            return Err(AppError::Validation(
                "Tax rate must be between 0 and 1".into(),
            ));
        }
        self.repo.create_tax_code(input).await
    }

    pub async fn list_tax_codes(&self) -> AppResult<Vec<TaxCode>> {
        self.repo.list_tax_codes().await
    }

    // --- Aging Report ---

    pub async fn aging_report(&self, as_of_date: &str) -> AppResult<ApAgingReport> {
        let rows = self.repo.aging_report(as_of_date).await?;
        let mut current_total: i64 = 0;
        let mut bucket_1_30_total: i64 = 0;
        let mut bucket_31_60_total: i64 = 0;
        let mut bucket_61_90_total: i64 = 0;
        let mut bucket_90_plus_total: i64 = 0;

        for row in &rows {
            match row.aging_bucket.as_str() {
                "current" => current_total += row.total_cents,
                "1-30" => bucket_1_30_total += row.total_cents,
                "31-60" => bucket_31_60_total += row.total_cents,
                "61-90" => bucket_61_90_total += row.total_cents,
                "90+" => bucket_90_plus_total += row.total_cents,
                _ => {}
            }
        }

        Ok(ApAgingReport {
            current_total,
            bucket_1_30_total,
            bucket_31_60_total,
            bucket_61_90_total,
            bucket_90_plus_total,
            invoices: rows,
        })
    }
}
