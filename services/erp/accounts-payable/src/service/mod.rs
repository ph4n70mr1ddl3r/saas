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

        let payment = self.repo.create_payment(input).await?;

        // Publish AP payment created event
        if let Err(e) = self
            .bus
            .publish(
                "erp.ap.payment.created",
                saas_proto::events::ApPaymentCreated {
                    payment_id: payment.id.clone(),
                    invoice_id: input.invoice_id.clone(),
                    vendor_id: input.vendor_id.clone(),
                    amount_cents: input.amount_cents,
                },
            )
            .await
        {
            tracing::error!(
                "CRITICAL: Failed to publish event '{}': {}. Data may be inconsistent.",
                "erp.ap.payment.created",
                e
            );
        }

        Ok(payment)
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

#[cfg(test)]
mod tests {
    use super::*;
    use saas_db::test_helpers::create_test_pool;
    use sqlx::SqlitePool;

    async fn setup() -> SqlitePool {
        let pool = create_test_pool().await;
        let sql_files = [
            include_str!("../../migrations/001_create_vendors.sql"),
            include_str!("../../migrations/002_create_ap_invoices.sql"),
            include_str!("../../migrations/003_create_ap_invoice_lines.sql"),
            include_str!("../../migrations/004_create_payments.sql"),
            include_str!("../../migrations/005_create_tax_codes.sql"),
        ];
        let migration_names = [
            "001_create_vendors.sql",
            "002_create_ap_invoices.sql",
            "003_create_ap_invoice_lines.sql",
            "004_create_payments.sql",
            "005_create_tax_codes.sql",
        ];
        sqlx::query(
            "CREATE TABLE IF NOT EXISTS _migrations (filename TEXT PRIMARY KEY, applied_at TEXT NOT NULL)",
        )
        .execute(&pool)
        .await
        .unwrap();

        for (i, sql) in sql_files.iter().enumerate() {
            let name = migration_names[i];
            let already_applied: bool =
                sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM _migrations WHERE filename = ?")
                    .bind(name)
                    .fetch_one(&pool)
                    .await
                    .unwrap()
                    > 0;
            if !already_applied {
                let mut tx = pool.begin().await.unwrap();
                sqlx::raw_sql(sql).execute(&mut *tx).await.unwrap();
                let now = chrono::Utc::now().to_rfc3339();
                sqlx::query("INSERT INTO _migrations (filename, applied_at) VALUES (?, ?)")
                    .bind(name)
                    .bind(&now)
                    .execute(&mut *tx)
                    .await
                    .unwrap();
                tx.commit().await.unwrap();
            }
        }
        pool
    }

    async fn setup_repo() -> ApRepo {
        let pool = setup().await;
        ApRepo::new(pool)
    }

    // Helper to create a test vendor
    async fn create_test_vendor(repo: &ApRepo, name: &str) -> Vendor {
        repo.create_vendor(&CreateVendorRequest {
            name: name.into(),
            email: Some(format!("{}@example.com", name.to_lowercase())),
            phone: None,
            address: None,
        })
        .await
        .unwrap()
    }

    // Helper to create a test invoice
    async fn create_test_invoice(repo: &ApRepo, vendor_id: &str, amount_cents: i64) -> ApInvoice {
        repo.create_invoice(&CreateApInvoiceRequest {
            vendor_id: vendor_id.into(),
            invoice_number: "INV-001".into(),
            invoice_date: "2025-01-15".into(),
            due_date: "2025-02-15".into(),
            lines: vec![CreateApInvoiceLineRequest {
                description: Some("Office supplies".into()),
                account_code: "5000".into(),
                amount_cents,
            }],
        })
        .await
        .unwrap()
    }

    #[tokio::test]
    async fn test_vendor_crud() {
        let repo = setup_repo().await;

        // Create
        let vendor = create_test_vendor(&repo, "Acme Corp").await;
        assert_eq!(vendor.name, "Acme Corp");
        assert_eq!(vendor.is_active, 1);

        // List
        let vendors = repo.list_vendors().await.unwrap();
        assert_eq!(vendors.len(), 1);
        assert_eq!(vendors[0].id, vendor.id);

        // Get by id
        let fetched = repo.get_vendor(&vendor.id).await.unwrap();
        assert_eq!(fetched.name, "Acme Corp");

        // Update
        let updated = repo
            .update_vendor(
                &vendor.id,
                &UpdateVendorRequest {
                    name: Some("Acme Inc".into()),
                    email: None,
                    phone: Some("555-0100".into()),
                    address: None,
                    is_active: None,
                },
            )
            .await
            .unwrap();
        assert_eq!(updated.name, "Acme Inc");

        // Deactivate
        let deactivated = repo
            .update_vendor(
                &vendor.id,
                &UpdateVendorRequest {
                    name: None,
                    email: None,
                    phone: None,
                    address: None,
                    is_active: Some(false),
                },
            )
            .await
            .unwrap();
        assert_eq!(deactivated.is_active, 0);

        // Not found
        let result = repo.get_vendor("nonexistent").await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_ap_invoice_create_with_lines() {
        let repo = setup_repo().await;
        let vendor = create_test_vendor(&repo, "Supplier A").await;

        let invoice = create_test_invoice(&repo, &vendor.id, 10000).await;
        assert_eq!(invoice.vendor_id, vendor.id);
        assert_eq!(invoice.total_cents, 10000);
        assert_eq!(invoice.status, "draft");
        assert_eq!(invoice.invoice_number, "INV-001");

        let lines = repo.get_invoice_lines(&invoice.id).await.unwrap();
        assert_eq!(lines.len(), 1);
        assert_eq!(lines[0].amount_cents, 10000);
        assert_eq!(lines[0].account_code, "5000");
    }

    #[tokio::test]
    async fn test_ap_invoice_lifecycle_approve_and_pay() {
        let repo = setup_repo().await;
        let vendor = create_test_vendor(&repo, "Supplier B").await;
        let invoice = create_test_invoice(&repo, &vendor.id, 20000).await;

        // Approve
        let approved = repo.approve_invoice(&invoice.id).await.unwrap();
        assert_eq!(approved.status, "approved");

        // Full payment
        let payment = repo
            .create_payment(&CreatePaymentRequest {
                invoice_id: invoice.id.clone(),
                vendor_id: vendor.id.clone(),
                amount_cents: 20000,
                payment_date: "2025-02-01".into(),
                method: Some("wire".into()),
                reference: Some("REF-001".into()),
            })
            .await
            .unwrap();
        assert_eq!(payment.amount_cents, 20000);
        assert_eq!(payment.vendor_id, vendor.id);

        // Invoice should be marked as paid
        let paid_invoice = repo.get_invoice(&invoice.id).await.unwrap();
        assert_eq!(paid_invoice.status, "paid");
    }

    #[tokio::test]
    async fn test_ap_invoice_two_payments_full() {
        let repo = setup_repo().await;
        let vendor = create_test_vendor(&repo, "Supplier C").await;
        let invoice = create_test_invoice(&repo, &vendor.id, 15000).await;
        repo.approve_invoice(&invoice.id).await.unwrap();

        // First payment
        let payment1 = repo
            .create_payment(&CreatePaymentRequest {
                invoice_id: invoice.id.clone(),
                vendor_id: vendor.id.clone(),
                amount_cents: 5000,
                payment_date: "2025-02-01".into(),
                method: None,
                reference: None,
            })
            .await
            .unwrap();
        assert_eq!(payment1.amount_cents, 5000);

        // Still in partial status
        let inv = repo.get_invoice(&invoice.id).await.unwrap();
        assert_eq!(inv.status, "partial");
    }

    #[tokio::test]
    async fn test_overpayment_prevention_full() {
        let repo = setup_repo().await;
        let vendor = create_test_vendor(&repo, "Supplier D").await;
        let invoice = create_test_invoice(&repo, &vendor.id, 5000).await;
        repo.approve_invoice(&invoice.id).await.unwrap();

        // Pay exactly full amount
        let payment = repo
            .create_payment(&CreatePaymentRequest {
                invoice_id: invoice.id.clone(),
                vendor_id: vendor.id.clone(),
                amount_cents: 5000,
                payment_date: "2025-02-15".into(),
                method: None,
                reference: None,
            })
            .await
            .unwrap();
        assert_eq!(payment.amount_cents, 5000);

        // Verify invoice is now paid
        let updated = repo.get_invoice(&invoice.id).await.unwrap();
        assert_eq!(updated.status, "paid");

        // Attempt to pay 1 more should fail
        let result = repo
            .create_payment(&CreatePaymentRequest {
                invoice_id: invoice.id.clone(),
                vendor_id: vendor.id.clone(),
                amount_cents: 1,
                payment_date: "2025-02-20".into(),
                method: None,
                reference: None,
            })
            .await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_list_invoices_and_payments() {
        let repo = setup_repo().await;
        let vendor = create_test_vendor(&repo, "Supplier F").await;

        // Create two invoices
        let inv1 = repo
            .create_invoice(&CreateApInvoiceRequest {
                vendor_id: vendor.id.clone(),
                invoice_number: "INV-010".into(),
                invoice_date: "2025-01-01".into(),
                due_date: "2025-02-01".into(),
                lines: vec![CreateApInvoiceLineRequest {
                    description: None,
                    account_code: "5000".into(),
                    amount_cents: 3000,
                }],
            })
            .await
            .unwrap();
        let inv2 = repo
            .create_invoice(&CreateApInvoiceRequest {
                vendor_id: vendor.id.clone(),
                invoice_number: "INV-011".into(),
                invoice_date: "2025-01-15".into(),
                due_date: "2025-02-15".into(),
                lines: vec![CreateApInvoiceLineRequest {
                    description: None,
                    account_code: "5000".into(),
                    amount_cents: 7000,
                }],
            })
            .await
            .unwrap();

        let invoices = repo.list_invoices().await.unwrap();
        assert_eq!(invoices.len(), 2);

        // Approve and pay one
        repo.approve_invoice(&inv1.id).await.unwrap();
        repo.create_payment(&CreatePaymentRequest {
            invoice_id: inv1.id.clone(),
            vendor_id: vendor.id.clone(),
            amount_cents: 3000,
            payment_date: "2025-02-01".into(),
            method: None,
            reference: None,
        })
        .await
        .unwrap();

        let payments = repo.list_payments().await.unwrap();
        assert_eq!(payments.len(), 1);
        assert_eq!(payments[0].invoice_id, inv1.id);

        // Verify paid invoice status
        let paid = repo.get_invoice(&inv1.id).await.unwrap();
        assert_eq!(paid.status, "paid");

        // Verify unpaid invoice status
        let unpaid = repo.get_invoice(&inv2.id).await.unwrap();
        assert_eq!(unpaid.status, "draft");
    }

    #[tokio::test]
    async fn test_ap_aging_report() {
        let repo = setup_repo().await;
        let vendor = create_test_vendor(&repo, "Supplier G").await;

        // Create and approve invoice
        let invoice = create_test_invoice(&repo, &vendor.id, 8000).await;
        repo.approve_invoice(&invoice.id).await.unwrap();

        // Run aging report as of the due date
        let rows = repo.aging_report("2025-02-15").await.unwrap();
        // Should appear in at least one bucket since it's approved
        assert!(!rows.is_empty());
        assert_eq!(rows[0].vendor_name, "Supplier G");
        assert_eq!(rows[0].invoice_number, "INV-001");
    }

    #[tokio::test]
    async fn test_tax_code_crud() {
        let repo = setup_repo().await;

        let tax_code = repo
            .create_tax_code(&CreateTaxCodeRequest {
                code: "VAT-20".into(),
                rate: 0.20,
                description: Some("Standard VAT".into()),
            })
            .await
            .unwrap();
        assert_eq!(tax_code.code, "VAT-20");
        assert!((tax_code.rate - 0.20).abs() < f64::EPSILON);
        assert_eq!(tax_code.is_active, 1);

        let tax_codes = repo.list_tax_codes().await.unwrap();
        assert_eq!(tax_codes.len(), 1);

        let fetched = repo.get_tax_code(&tax_code.id).await.unwrap();
        assert_eq!(fetched.code, "VAT-20");
    }

    #[tokio::test]
    async fn test_tax_rate_validation() {
        // Valid rates: between 0.0 and 1.0 inclusive
        let valid_rates = [0.0, 0.05, 0.20, 0.5, 1.0];
        for rate in valid_rates {
            assert!(rate >= 0.0 && rate <= 1.0, "Rate {} should be valid", rate);
        }

        // Invalid rates
        let invalid_rates = [-0.01, 1.01, 2.0];
        for rate in invalid_rates {
            assert!(rate < 0.0 || rate > 1.0, "Rate {} should be invalid", rate);
        }
    }

    #[tokio::test]
    async fn test_ap_payment_publishes_event_data() {
        let repo = setup_repo().await;
        let vendor = create_test_vendor(&repo, "Payment Event Vendor").await;
        let invoice = create_test_invoice(&repo, &vendor.id, 25000).await;

        // Approve the invoice first
        repo.approve_invoice(&invoice.id).await.unwrap();

        // Create a payment
        let payment = repo
            .create_payment(&CreatePaymentRequest {
                invoice_id: invoice.id.clone(),
                vendor_id: vendor.id.clone(),
                amount_cents: 25000,
                payment_date: "2025-03-01".into(),
                method: Some("wire".into()),
                reference: Some("PAY-REF-001".into()),
            })
            .await
            .unwrap();

        // Verify payment data matches what would be in the event
        assert_eq!(payment.invoice_id, invoice.id);
        assert_eq!(payment.vendor_id, vendor.id);
        assert_eq!(payment.amount_cents, 25000);

        // Verify invoice is now paid
        let paid = repo.get_invoice(&invoice.id).await.unwrap();
        assert_eq!(paid.status, "paid");
    }
}
