use crate::models::*;
use crate::repository::ArRepo;
use saas_common::error::{AppError, AppResult};
use saas_nats_bus::NatsBus;
use sqlx::SqlitePool;

#[derive(Clone)]
pub struct ArService {
    repo: ArRepo,
    bus: NatsBus,
}

impl ArService {
    pub fn new(pool: SqlitePool, bus: NatsBus) -> Self {
        Self {
            repo: ArRepo::new(pool),
            bus,
        }
    }

    // --- Customers ---

    pub async fn list_customers(&self) -> AppResult<Vec<Customer>> {
        self.repo.list_customers().await
    }

    pub async fn get_customer(&self, id: &str) -> AppResult<Customer> {
        self.repo.get_customer(id).await
    }

    pub async fn create_customer(&self, input: &CreateCustomerRequest) -> AppResult<Customer> {
        self.repo.create_customer(input).await
    }

    pub async fn update_customer(
        &self,
        id: &str,
        input: &UpdateCustomerRequest,
    ) -> AppResult<Customer> {
        self.repo.get_customer(id).await?;
        self.repo.update_customer(id, input).await
    }

    // --- Invoices ---

    pub async fn list_invoices(&self) -> AppResult<Vec<ArInvoice>> {
        self.repo.list_invoices().await
    }

    pub async fn get_invoice(&self, id: &str) -> AppResult<ArInvoiceWithLines> {
        let invoice = self.repo.get_invoice(id).await?;
        let lines = self.repo.get_invoice_lines(id).await?;
        Ok(ArInvoiceWithLines { invoice, lines })
    }

    pub async fn create_invoice(
        &self,
        input: &CreateArInvoiceRequest,
    ) -> AppResult<ArInvoiceWithLines> {
        // Validate customer exists
        self.repo.get_customer(&input.customer_id).await?;

        // Validate line amounts are non-negative
        for line in &input.lines {
            if line.amount_cents < 0 {
                return Err(AppError::Validation(
                    "Invoice line amounts must be non-negative".into(),
                ));
            }
        }

        let invoice = self.repo.create_invoice(input).await?;

        // Update status to sent (draft -> sent on creation)
        let invoice = self.repo.mark_invoice_sent(&invoice.id).await?;
        let lines = self.repo.get_invoice_lines(&invoice.id).await?;

        // Publish erp.ar.invoice.created event
        let event = saas_proto::events::CustomerInvoiceCreated {
            invoice_id: invoice.id.clone(),
            customer_id: invoice.customer_id.clone(),
            total_cents: invoice.total_cents,
        };
        if let Err(e) = self.bus.publish("erp.ar.invoice.created", &event).await {
            tracing::error!("Failed to publish AR invoice created event: {}", e);
        }

        Ok(ArInvoiceWithLines { invoice, lines })
    }

    // --- Receipts ---

    pub async fn list_receipts(&self) -> AppResult<Vec<Receipt>> {
        self.repo.list_receipts().await
    }

    pub async fn create_receipt(&self, input: &CreateReceiptRequest) -> AppResult<Receipt> {
        // Validate amount is positive
        if input.amount_cents <= 0 {
            return Err(AppError::Validation(
                "Receipt amount must be positive".into(),
            ));
        }

        // Validate invoice exists and is in sent or partial status
        let invoice = self.repo.get_invoice(&input.invoice_id).await?;
        if invoice.status != "sent" && invoice.status != "partial" {
            return Err(AppError::Validation(
                "Can only receipt against sent or partially paid invoices".into(),
            ));
        }

        // Verify customer_id matches the invoice
        if invoice.customer_id != input.customer_id {
            return Err(AppError::Validation(
                "Customer ID does not match invoice".into(),
            ));
        }

        self.repo.create_receipt(input).await
    }

    // --- Credit Memos ---

    pub async fn create_credit_memo(
        &self,
        input: &CreateCreditMemoRequest,
    ) -> AppResult<CreditMemo> {
        if input.amount_cents <= 0 {
            return Err(AppError::Validation(
                "Credit memo amount must be positive".into(),
            ));
        }

        // Validate customer exists
        self.repo.get_customer(&input.customer_id).await?;

        self.repo.create_credit_memo(input).await
    }

    pub async fn apply_credit_memo(
        &self,
        memo_id: &str,
        input: &ApplyCreditMemoRequest,
    ) -> AppResult<CreditMemo> {
        // Get the credit memo
        let memo = self.repo.get_credit_memo(memo_id).await?;

        // Validate memo is open
        if memo.status != "open" {
            return Err(AppError::Validation(
                "Credit memo is not in 'open' status".into(),
            ));
        }

        // Validate amount does not exceed memo remaining
        if input.amount_cents <= 0 {
            return Err(AppError::Validation("Apply amount must be positive".into()));
        }
        let remaining = memo.amount_cents - memo.applied_amount_cents;
        if input.amount_cents > remaining {
            return Err(AppError::Validation(format!(
                "Apply amount {} exceeds credit memo remaining balance of {}",
                input.amount_cents, remaining
            )));
        }

        // Validate invoice exists and customer matches
        let invoice = self.repo.get_invoice(&input.invoice_id).await?;
        if invoice.customer_id != memo.customer_id {
            return Err(AppError::Validation(
                "Credit memo customer does not match invoice customer".into(),
            ));
        }

        self.repo
            .apply_credit_memo(memo_id, &input.invoice_id, input.amount_cents)
            .await
    }

    pub async fn list_credit_memos(&self) -> AppResult<Vec<CreditMemo>> {
        self.repo.list_credit_memos().await
    }

    // --- Aging Report ---

    pub async fn aging_report(&self, as_of_date: &str) -> AppResult<ArAgingReport> {
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

        Ok(ArAgingReport {
            current_total,
            bucket_1_30_total,
            bucket_31_60_total,
            bucket_61_90_total,
            bucket_90_plus_total,
            invoices: rows,
        })
    }
}
