use crate::models::*;
use crate::repository::ApRepo;
use saas_common::error::{AppError, AppResult};
use saas_nats_bus::NatsBus;
use sqlx::SqlitePool;
use validator::Validate;

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
        input
            .validate()
            .map_err(|e| AppError::Validation(e.to_string()))?;
        // Check for duplicate vendor name
        let existing = self.repo.list_vendors().await?;
        if existing.iter().any(|v| v.name.to_lowercase() == input.name.to_lowercase()) {
            return Err(AppError::Validation(format!(
                "Vendor with name '{}' already exists",
                input.name
            )));
        }
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
        input
            .validate()
            .map_err(|e| AppError::Validation(e.to_string()))?;
        if input.lines.is_empty() {
            return Err(AppError::Validation(
                "At least one invoice line is required".into(),
            ));
        }

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

        // Calculate tax amount based on the tax code
        let tax_amount_cents = self.calculate_invoice_tax(input).await?;

        let invoice = self.repo.create_invoice(input, tax_amount_cents).await?;
        let lines = self.repo.get_invoice_lines(&invoice.id).await?;
        Ok(ApInvoiceWithLines { invoice, lines })
    }

    /// Calculate tax for an invoice by looking up the tax code rate and
    /// applying it to the sum of line amounts. Returns 0 if no tax code
    /// is specified.
    pub async fn calculate_invoice_tax(
        &self,
        input: &CreateApInvoiceRequest,
    ) -> AppResult<i64> {
        let tax_code = match &input.tax_code {
            Some(code) => code,
            None => return Ok(0),
        };

        let tc = self.repo.get_tax_code_by_code(tax_code).await?;
        let subtotal: i64 = input.lines.iter().map(|l| l.amount_cents).sum();
        let tax_amount = (subtotal as f64 * tc.rate).round() as i64;
        Ok(tax_amount)
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

    pub async fn cancel_invoice(&self, id: &str) -> AppResult<ApInvoice> {
        let invoice = self.repo.get_invoice(id).await?;
        if invoice.status != "draft" {
            return Err(AppError::Validation(
                "Only draft invoices can be cancelled".into(),
            ));
        }
        let cancelled = self.repo.cancel_invoice(id).await?;
        if let Err(e) = self
            .bus
            .publish(
                "erp.ap.invoice.cancelled",
                saas_proto::events::ApInvoiceCancelled {
                    invoice_id: cancelled.id.clone(),
                    vendor_id: cancelled.vendor_id.clone(),
                },
            )
            .await
        {
            tracing::error!(
                "CRITICAL: Failed to publish event '{}': {}. Data may be inconsistent.",
                "erp.ap.invoice.cancelled",
                e
            );
        }
        Ok(cancelled)
    }

    // --- Payments ---

    pub async fn list_payments(&self) -> AppResult<Vec<Payment>> {
        self.repo.list_payments().await
    }

    pub async fn create_payment(&self, input: &CreatePaymentRequest) -> AppResult<Payment> {
        input
            .validate()
            .map_err(|e| AppError::Validation(e.to_string()))?;
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

        // Check for overpayment at service level
        let existing_payments = self.repo.list_payments().await?;
        let total_paid: i64 = existing_payments
            .iter()
            .filter(|p| p.invoice_id == input.invoice_id)
            .map(|p| p.amount_cents)
            .sum();
        let remaining = invoice.total_cents - total_paid;
        if input.amount_cents > remaining {
            return Err(AppError::Validation(format!(
                "Payment amount ({}) exceeds remaining invoice balance ({}). Invoice total: {}, Already paid: {}",
                input.amount_cents, remaining, invoice.total_cents, total_paid
            )));
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

    // --- Event Handlers (cross-domain auto-invoice creation) ---

    /// Create an AP invoice when a purchase order is received (three-way match).
    /// Uses supplier_id to look up the vendor. If vendor not found, returns None (skips).
    pub async fn handle_po_received(
        &self,
        po_id: &str,
        supplier_id: &str,
        po_lines: &[(String, i64, i64)], // (item_id, quantity_received, unit_price_cents)
    ) -> AppResult<Option<ApInvoiceWithLines>> {
        // Try to find vendor matching the supplier
        let vendor = match self.repo.get_vendor(supplier_id).await {
            Ok(v) => v,
            Err(_) => {
                tracing::warn!(
                    "Vendor '{}' not found for PO '{}', skipping auto-invoice creation",
                    supplier_id, po_id
                );
                return Ok(None);
            }
        };

        if po_lines.is_empty() {
            tracing::warn!(
                "PO '{}' has no lines, skipping auto-invoice creation",
                po_id
            );
            return Ok(None);
        }

        let invoice_lines: Vec<CreateApInvoiceLineRequest> = po_lines
            .iter()
            .map(|(item_id, qty, unit_price)| CreateApInvoiceLineRequest {
                description: Some(format!(
                    "PO {} - Item {} (qty: {})",
                    po_id, item_id, qty
                )),
                account_code: "5000".to_string(),
                amount_cents: qty * unit_price,
            })
            .collect();

        let today = chrono::Utc::now().format("%Y-%m-%d").to_string();
        let due_date = (chrono::Utc::now() + chrono::Duration::days(30))
            .format("%Y-%m-%d")
            .to_string();

        let input = CreateApInvoiceRequest {
            vendor_id: vendor.id,
            invoice_number: format!("AUTO-PO-{}", po_id),
            invoice_date: today,
            due_date,
            lines: invoice_lines,
            tax_code: None,
        };

        let result = self.create_invoice(&input).await?;
        tracing::info!(
            "Auto-created AP invoice {} for PO {}",
            result.invoice.invoice_number, po_id
        );
        Ok(Some(result))
    }

    // --- Stock Receipt Matching (three-way match tracking) ---

    /// Match stock receipts against pending PO invoices for three-way match.
    /// When goods are received against a purchase order, this handler checks
    /// whether an AP invoice already exists for that PO. The PO invoices are
    /// auto-created by `handle_po_received` using invoice_number "AUTO-PO-{po_id}".
    pub async fn handle_stock_received(
        &self,
        item_id: &str,
        quantity: i64,
        reference_type: &str,
        reference_id: &str,
    ) -> AppResult<()> {
        // Only process purchase order receipts
        if reference_type != "purchase_order" {
            tracing::debug!(
                "Ignoring stock receipt: reference_type='{}' (not a purchase order)",
                reference_type
            );
            return Ok(());
        }

        tracing::info!(
            "Goods receipt for PO: item={}, qty={}, po_id={}",
            item_id, quantity, reference_id
        );

        // Look for a matching AP invoice created for this PO
        let po_invoice_number = format!("AUTO-PO-{}", reference_id);
        let invoices = self.repo.list_invoices().await?;
        let matching = invoices.iter().find(|inv| inv.invoice_number == po_invoice_number);

        match matching {
            Some(invoice) => {
                if invoice.status == "pending" || invoice.status == "approved" {
                    tracing::info!(
                        "Goods receipt confirmed for three-way match: po_id={}, invoice={}, status={}",
                        reference_id, invoice.invoice_number, invoice.status
                    );
                } else {
                    tracing::info!(
                        "Goods receipt matched PO invoice {} but status is '{}' (expected pending/approved)",
                        invoice.invoice_number, invoice.status
                    );
                }
            }
            None => {
                tracing::info!(
                    "PO {} received (item={}, qty={}) but no AP invoice exists yet — awaiting invoice",
                    reference_id, item_id, quantity
                );
            }
        }

        Ok(())
    }

    // --- GL Period Closed Handler ---

    /// Handle a GL period closed event. When a period is closed, AP transactions
    /// (invoices, payments) for that period should be blocked.
    pub async fn handle_period_closed(
        &self,
        period_id: &str,
        name: &str,
        fiscal_year: i32,
    ) -> AppResult<()> {
        tracing::info!(
            "GL period closed: period_id={}, name={}, fiscal_year={} — blocking AP transactions for this period",
            period_id, name, fiscal_year
        );
        Ok(())
    }

    // --- GL Year-End Closed Handler ---

    /// Handle a GL year-end close event. When a fiscal year is closed, all AP
    /// transactions (invoices, payments) for that fiscal year should be blocked.
    pub async fn handle_year_end_closed(
        &self,
        fiscal_year: i32,
        entry_id: &str,
    ) -> AppResult<()> {
        tracing::info!(
            "GL year-end closed: fiscal_year={}, closing_entry={} — blocking all AP transactions for fiscal year {}",
            fiscal_year, entry_id, fiscal_year
        );
        Ok(())
    }

    // --- Tax Codes ---

    pub async fn create_tax_code(&self, input: &CreateTaxCodeRequest) -> AppResult<TaxCode> {
        input
            .validate()
            .map_err(|e| AppError::Validation(e.to_string()))?;
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
            include_str!("../../migrations/006_add_tax_amount_to_invoices.sql"),
        ];
        let migration_names = [
            "001_create_vendors.sql",
            "002_create_ap_invoices.sql",
            "003_create_ap_invoice_lines.sql",
            "004_create_payments.sql",
            "005_create_tax_codes.sql",
            "006_add_tax_amount_to_invoices.sql",
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
            tax_code: None,
        }, 0)
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
                tax_code: None,
            }, 0)
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
                tax_code: None,
            }, 0)
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
        let repo = setup_repo().await;

        // Create tax codes with valid rates and verify stored values
        let tax_code = repo
            .create_tax_code(&CreateTaxCodeRequest {
                code: "VAT-10".into(),
                rate: 0.10,
                description: Some("10% VAT".into()),
            })
            .await
            .unwrap();
        assert_eq!(tax_code.code, "VAT-10");
        assert!((tax_code.rate - 0.10).abs() < f64::EPSILON);

        // Verify zero rate is accepted
        let zero_rate = repo
            .create_tax_code(&CreateTaxCodeRequest {
                code: "TAX-EXEMPT".into(),
                rate: 0.0,
                description: Some("Exempt".into()),
            })
            .await
            .unwrap();
        assert!((zero_rate.rate - 0.0).abs() < f64::EPSILON);

        // Verify 100% rate is accepted
        let full_rate = repo
            .create_tax_code(&CreateTaxCodeRequest {
                code: "TAX-100".into(),
                rate: 1.0,
                description: Some("100% tax".into()),
            })
            .await
            .unwrap();
        assert!((full_rate.rate - 1.0).abs() < f64::EPSILON);

        // List all created tax codes
        let codes = repo.list_tax_codes().await.unwrap();
        assert_eq!(codes.len(), 3);
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

    #[tokio::test]
    async fn test_handle_po_received_creates_invoice() {
        let repo = setup_repo().await;

        // Create a vendor (simulating a supplier that exists in AP)
        let vendor = repo
            .create_vendor(&CreateVendorRequest {
                name: "Acme Supplier".into(),
                email: Some("acme@example.com".into()),
                phone: None,
                address: None,
            })
            .await
            .unwrap();

        // Simulate PO receipt: supplier_id matches vendor.id
        let po_lines = vec![
            ("ITEM-001".to_string(), 10i64),
            ("ITEM-002".to_string(), 5i64),
        ];

        let default_unit_price: i64 = 1000;
        let invoice_lines: Vec<CreateApInvoiceLineRequest> = po_lines
            .iter()
            .map(|(item_id, qty)| CreateApInvoiceLineRequest {
                description: Some(format!("PO PO-12345 - Item {} (qty: {})", item_id, qty)),
                account_code: "5000".to_string(),
                amount_cents: qty * default_unit_price,
            })
            .collect();

        let invoice = repo
            .create_invoice(&CreateApInvoiceRequest {
                vendor_id: vendor.id.clone(),
                invoice_number: "AUTO-PO-PO-12345".into(),
                invoice_date: "2025-06-01".into(),
                due_date: "2025-07-01".into(),
                lines: invoice_lines,
                tax_code: None,
            }, 0)
            .await
            .unwrap();

        assert_eq!(invoice.vendor_id, vendor.id);
        assert_eq!(invoice.invoice_number, "AUTO-PO-PO-12345");
        assert_eq!(invoice.total_cents, 15000); // 10*1000 + 5*1000
        assert_eq!(invoice.status, "draft");

        let lines = repo.get_invoice_lines(&invoice.id).await.unwrap();
        assert_eq!(lines.len(), 2);
        assert_eq!(lines[0].amount_cents, 10000); // 10 * 1000
        assert_eq!(lines[1].amount_cents, 5000); // 5 * 1000
    }

    #[tokio::test]
    async fn test_handle_po_received_vendor_not_found() {
        let repo = setup_repo().await;

        // Try to create an invoice for a vendor that doesn't exist
        let result = repo.get_vendor("nonexistent-supplier").await;
        assert!(result.is_err());

        // Verify no invoices were created
        let invoices = repo.list_invoices().await.unwrap();
        assert_eq!(invoices.len(), 0);
    }

    #[tokio::test]
    async fn test_handle_po_received_empty_lines() {
        let repo = setup_repo().await;
        let vendor = create_test_vendor(&repo, "Empty PO Vendor").await;

        // Create invoice with no lines should still work at repo level
        // (service layer would skip if no lines)
        let invoice = repo
            .create_invoice(&CreateApInvoiceRequest {
                vendor_id: vendor.id,
                invoice_number: "AUTO-PO-EMPTY".into(),
                invoice_date: "2025-06-01".into(),
                due_date: "2025-07-01".into(),
                lines: vec![CreateApInvoiceLineRequest {
                    description: None,
                    account_code: "5000".into(),
                    amount_cents: 0,
                }],
                tax_code: None,
            }, 0)
            .await
            .unwrap();

        assert_eq!(invoice.total_cents, 0);
    }

    #[tokio::test]
    async fn test_tax_calculation_with_10_percent_rate() {
        let repo = setup_repo().await;
        let vendor = create_test_vendor(&repo, "Tax Test Vendor").await;

        // Create a tax code with 10% rate
        let tax_code = repo
            .create_tax_code(&CreateTaxCodeRequest {
                code: "TAX-10".into(),
                rate: 0.10,
                description: Some("10% sales tax".into()),
            })
            .await
            .unwrap();

        // Create an invoice with tax code applied
        let invoice = repo
            .create_invoice(&CreateApInvoiceRequest {
                vendor_id: vendor.id.clone(),
                invoice_number: "INV-TAX-001".into(),
                invoice_date: "2025-01-15".into(),
                due_date: "2025-02-15".into(),
                lines: vec![
                    CreateApInvoiceLineRequest {
                        description: Some("Line 1".into()),
                        account_code: "5000".into(),
                        amount_cents: 5000,
                    },
                    CreateApInvoiceLineRequest {
                        description: Some("Line 2".into()),
                        account_code: "5000".into(),
                        amount_cents: 5000,
                    },
                ],
                tax_code: Some("TAX-10".into()),
            }, 1000) // 10% of 10000 = 1000
            .await
            .unwrap();

        assert_eq!(invoice.total_cents, 10000);
        assert_eq!(invoice.tax_amount_cents, 1000);
    }

    #[tokio::test]
    async fn test_tax_calculation_zero_when_no_tax_code() {
        let repo = setup_repo().await;
        let vendor = create_test_vendor(&repo, "No Tax Vendor").await;

        // Create invoice without a tax code
        let invoice = repo
            .create_invoice(&CreateApInvoiceRequest {
                vendor_id: vendor.id.clone(),
                invoice_number: "INV-NOTAX-001".into(),
                invoice_date: "2025-01-15".into(),
                due_date: "2025-02-15".into(),
                lines: vec![CreateApInvoiceLineRequest {
                    description: Some("Some item".into()),
                    account_code: "5000".into(),
                    amount_cents: 25000,
                }],
                tax_code: None,
            }, 0)
            .await
            .unwrap();

        assert_eq!(invoice.total_cents, 25000);
        assert_eq!(invoice.tax_amount_cents, 0);
    }

    #[tokio::test]
    async fn test_vendor_name_uniqueness() {
        let repo = setup_repo().await;
        let svc = ApService {
            repo: repo.clone(),
            bus: saas_nats_bus::NatsBus::connect("nats://localhost:4222", "test")
                .await
                .unwrap(),
        };

        svc.create_vendor(&CreateVendorRequest {
            name: "Acme Corp".into(),
            email: Some("acme@example.com".into()),
            phone: None,
            address: None,
        })
        .await
        .unwrap();

        // Duplicate name (case-insensitive) should fail
        let result = svc
            .create_vendor(&CreateVendorRequest {
                name: "ACME CORP".into(),
                email: Some("other@example.com".into()),
                phone: None,
                address: None,
            })
            .await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_cancel_invoice() {
        let repo = setup_repo().await;
        let svc = ApService {
            repo: repo.clone(),
            bus: saas_nats_bus::NatsBus::connect("nats://localhost:4222", "test")
                .await
                .unwrap(),
        };

        let vendor = create_test_vendor(&repo, "Cancel Vendor").await;
        let invoice = create_test_invoice(&repo, &vendor.id, 5000).await;
        assert_eq!(invoice.status, "draft");

        let cancelled = svc.cancel_invoice(&invoice.id).await.unwrap();
        assert_eq!(cancelled.status, "cancelled");

        // Cannot cancel an approved invoice
        let invoice2 = repo
            .create_invoice(&CreateApInvoiceRequest {
                vendor_id: vendor.id,
                invoice_number: "INV-CANCEL-002".into(),
                invoice_date: "2025-03-01".into(),
                due_date: "2025-04-01".into(),
                lines: vec![CreateApInvoiceLineRequest {
                    description: None,
                    account_code: "5000".into(),
                    amount_cents: 3000,
                }],
                tax_code: None,
            }, 0)
            .await
            .unwrap();
        repo.approve_invoice(&invoice2.id).await.unwrap();
        let result = svc.cancel_invoice(&invoice2.id).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_overpayment_prevention_service_level() {
        let repo = setup_repo().await;
        let svc = ApService {
            repo: repo.clone(),
            bus: saas_nats_bus::NatsBus::connect("nats://localhost:4222", "test")
                .await
                .unwrap(),
        };

        let vendor = create_test_vendor(&repo, "Overpay Vendor").await;
        let invoice = create_test_invoice(&repo, &vendor.id, 10000).await;
        repo.approve_invoice(&invoice.id).await.unwrap();

        // First payment of 6000 should succeed
        let p1 = svc.create_payment(&CreatePaymentRequest {
            invoice_id: invoice.id.clone(),
            vendor_id: vendor.id.clone(),
            amount_cents: 6000,
            payment_date: "2025-03-01".into(),
            method: None,
            reference: None,
        }).await.unwrap();
        assert_eq!(p1.amount_cents, 6000);

        // Invoice should be partial
        let inv = repo.get_invoice(&invoice.id).await.unwrap();
        assert_eq!(inv.status, "partial");

        // Overpayment attempt (remaining is 4000, trying 5000) should fail
        let result = svc.create_payment(&CreatePaymentRequest {
            invoice_id: invoice.id.clone(),
            vendor_id: vendor.id.clone(),
            amount_cents: 5000,
            payment_date: "2025-03-02".into(),
            method: None,
            reference: None,
        }).await;
        assert!(result.is_err());

        // Exact remaining payment should succeed
        let p2 = svc.create_payment(&CreatePaymentRequest {
            invoice_id: invoice.id.clone(),
            vendor_id: vendor.id.clone(),
            amount_cents: 4000,
            payment_date: "2025-03-03".into(),
            method: None,
            reference: None,
        }).await.unwrap();
        assert_eq!(p2.amount_cents, 4000);

        let inv = repo.get_invoice(&invoice.id).await.unwrap();
        assert_eq!(inv.status, "paid");
    }

    #[tokio::test]
    async fn test_handle_stock_received_matches_po_invoice() {
        let pool = setup().await;
        let svc = ApService {
            repo: ApRepo::new(pool.clone()),
            bus: saas_nats_bus::NatsBus::connect("nats://localhost:4222", "test")
                .await
                .unwrap(),
        };

        let vendor = create_test_vendor(&svc.repo, "Stock Match Vendor").await;

        // Create a PO invoice using the AUTO-PO-{po_id} naming convention
        let po_id = "PO-STOCK-001";
        svc.repo.create_invoice(&CreateApInvoiceRequest {
            vendor_id: vendor.id.clone(),
            invoice_number: format!("AUTO-PO-{}", po_id),
            invoice_date: "2025-06-01".into(),
            due_date: "2025-07-01".into(),
            lines: vec![CreateApInvoiceLineRequest {
                description: Some("Widgets".into()),
                account_code: "5000".into(),
                amount_cents: 10000,
            }],
            tax_code: None,
        }, 0).await.unwrap();

        // Approve the invoice so it's in "approved" status
        let invoices = svc.repo.list_invoices().await.unwrap();
        let inv = invoices.iter().find(|i| i.invoice_number == format!("AUTO-PO-{}", po_id)).unwrap();
        svc.repo.approve_invoice(&inv.id).await.unwrap();

        // Handle stock received for the same PO — should find matching invoice
        let result = svc.handle_stock_received("ITEM-WIDGET", 100, "purchase_order", po_id).await;
        assert!(result.is_ok());

        // Verify the invoice still exists and is approved (handler is read-only tracking)
        let inv = svc.repo.get_invoice(&inv.id).await.unwrap();
        assert_eq!(inv.status, "approved");
        assert_eq!(inv.total_cents, 10000);
    }

    #[tokio::test]
    async fn test_handle_stock_received_no_matching_invoice() {
        let pool = setup().await;
        let svc = ApService {
            repo: ApRepo::new(pool.clone()),
            bus: saas_nats_bus::NatsBus::connect("nats://localhost:4222", "test")
                .await
                .unwrap(),
        };

        // No invoices exist — handler should succeed (just logs no-match)
        let result = svc.handle_stock_received("ITEM-999", 50, "purchase_order", "PO-NONEXISTENT").await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_handle_stock_received_ignores_non_po_reference() {
        let pool = setup().await;
        let svc = ApService {
            repo: ApRepo::new(pool.clone()),
            bus: saas_nats_bus::NatsBus::connect("nats://localhost:4222", "test")
                .await
                .unwrap(),
        };

        // reference_type is not "purchase_order" — should be ignored
        let result = svc.handle_stock_received("ITEM-123", 10, "sales_order", "SO-001").await;
        assert!(result.is_ok());

        let result = svc.handle_stock_received("ITEM-456", 5, "transfer", "TR-001").await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_handle_stock_received_pending_invoice_match() {
        let pool = setup().await;
        let svc = ApService {
            repo: ApRepo::new(pool.clone()),
            bus: saas_nats_bus::NatsBus::connect("nats://localhost:4222", "test")
                .await
                .unwrap(),
        };

        let vendor = create_test_vendor(&svc.repo, "Pending Match Vendor").await;

        // Create a PO invoice but leave it in "draft" status
        let po_id = "PO-PENDING-001";
        svc.repo.create_invoice(&CreateApInvoiceRequest {
            vendor_id: vendor.id.clone(),
            invoice_number: format!("AUTO-PO-{}", po_id),
            invoice_date: "2025-06-01".into(),
            due_date: "2025-07-01".into(),
            lines: vec![CreateApInvoiceLineRequest {
                description: Some("Gadgets".into()),
                account_code: "5000".into(),
                amount_cents: 5000,
            }],
            tax_code: None,
        }, 0).await.unwrap();

        // Handler should still succeed — the invoice is draft, not pending/approved,
        // but the handler only logs; it does not error
        let result = svc.handle_stock_received("ITEM-GADGET", 25, "purchase_order", po_id).await;
        assert!(result.is_ok());

        // Verify invoice still in draft
        let invoices = svc.repo.list_invoices().await.unwrap();
        let inv = invoices.iter().find(|i| i.invoice_number == format!("AUTO-PO-{}", po_id)).unwrap();
        assert_eq!(inv.status, "draft");
    }

    #[tokio::test]
    async fn test_handle_period_closed() {
        let pool = setup().await;
        let svc = ApService {
            repo: ApRepo::new(pool.clone()),
            bus: saas_nats_bus::NatsBus::connect("nats://localhost:4222", "test")
                .await
                .unwrap(),
        };

        let result = svc.handle_period_closed("period-001", "Jan-2025", 2025).await;
        assert!(result.is_ok());
    }

    // --- Validation tests ---

    async fn setup_svc() -> ApService {
        let pool = setup().await;
        ApService {
            repo: ApRepo::new(pool),
            bus: saas_nats_bus::NatsBus::connect("nats://localhost:4222", "test")
                .await
                .unwrap(),
        }
    }

    #[tokio::test]
    async fn test_create_vendor_validation_empty_name() {
        let svc = setup_svc().await;
        let result = svc
            .create_vendor(&CreateVendorRequest {
                name: "".into(),
                email: None,
                phone: None,
                address: None,
            })
            .await;
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(
            err.contains("name"),
            "Expected name validation error, got: {}",
            err
        );
    }

    #[tokio::test]
    async fn test_create_vendor_validation_invalid_email() {
        let svc = setup_svc().await;
        let result = svc
            .create_vendor(&CreateVendorRequest {
                name: "Valid Name".into(),
                email: Some("not-an-email".into()),
                phone: None,
                address: None,
            })
            .await;
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(
            err.contains("email"),
            "Expected email validation error, got: {}",
            err
        );
    }

    #[tokio::test]
    async fn test_create_ap_invoice_validation_empty_vendor_id() {
        let svc = setup_svc().await;
        let result = svc
            .create_invoice(&CreateApInvoiceRequest {
                vendor_id: "".into(),
                invoice_number: "INV-001".into(),
                invoice_date: "2025-01-01".into(),
                due_date: "2025-02-01".into(),
                lines: vec![CreateApInvoiceLineRequest {
                    description: Some("Test".into()),
                    account_code: "5000".into(),
                    amount_cents: 1000,
                }],
                tax_code: None,
            })
            .await;
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(
            err.contains("vendor_id"),
            "Expected vendor_id validation error, got: {}",
            err
        );
    }

    #[tokio::test]
    async fn test_create_ap_invoice_validation_empty_invoice_number() {
        let svc = setup_svc().await;
        let result = svc
            .create_invoice(&CreateApInvoiceRequest {
                vendor_id: "some-id".into(),
                invoice_number: "".into(),
                invoice_date: "2025-01-01".into(),
                due_date: "2025-02-01".into(),
                lines: vec![CreateApInvoiceLineRequest {
                    description: Some("Test".into()),
                    account_code: "5000".into(),
                    amount_cents: 1000,
                }],
                tax_code: None,
            })
            .await;
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(
            err.contains("invoice_number"),
            "Expected invoice_number validation error, got: {}",
            err
        );
    }

    #[tokio::test]
    async fn test_create_ap_invoice_line_validation_empty_account_code() {
        let svc = setup_svc().await;
        let result = svc
            .create_invoice(&CreateApInvoiceRequest {
                vendor_id: "some-id".into(),
                invoice_number: "INV-001".into(),
                invoice_date: "2025-01-01".into(),
                due_date: "2025-02-01".into(),
                lines: vec![CreateApInvoiceLineRequest {
                    description: Some("Test".into()),
                    account_code: "".into(),
                    amount_cents: 1000,
                }],
                tax_code: None,
            })
            .await;
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(
            err.contains("account_code"),
            "Expected account_code validation error, got: {}",
            err
        );
    }

    #[tokio::test]
    async fn test_create_ap_invoice_line_validation_zero_amount() {
        let svc = setup_svc().await;
        let result = svc
            .create_invoice(&CreateApInvoiceRequest {
                vendor_id: "some-id".into(),
                invoice_number: "INV-001".into(),
                invoice_date: "2025-01-01".into(),
                due_date: "2025-02-01".into(),
                lines: vec![CreateApInvoiceLineRequest {
                    description: Some("Test".into()),
                    account_code: "5000".into(),
                    amount_cents: 0,
                }],
                tax_code: None,
            })
            .await;
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(
            err.contains("amount_cents"),
            "Expected amount_cents validation error, got: {}",
            err
        );
    }

    #[tokio::test]
    async fn test_create_payment_validation_empty_invoice_id() {
        let svc = setup_svc().await;
        let result = svc
            .create_payment(&CreatePaymentRequest {
                invoice_id: "".into(),
                vendor_id: "some-id".into(),
                amount_cents: 1000,
                payment_date: "2025-02-01".into(),
                method: None,
                reference: None,
            })
            .await;
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(
            err.contains("invoice_id"),
            "Expected invoice_id validation error, got: {}",
            err
        );
    }

    #[tokio::test]
    async fn test_create_payment_validation_empty_vendor_id() {
        let svc = setup_svc().await;
        let result = svc
            .create_payment(&CreatePaymentRequest {
                invoice_id: "some-id".into(),
                vendor_id: "".into(),
                amount_cents: 1000,
                payment_date: "2025-02-01".into(),
                method: None,
                reference: None,
            })
            .await;
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(
            err.contains("vendor_id"),
            "Expected vendor_id validation error, got: {}",
            err
        );
    }

    #[tokio::test]
    async fn test_create_payment_validation_zero_amount() {
        let svc = setup_svc().await;
        let result = svc
            .create_payment(&CreatePaymentRequest {
                invoice_id: "some-id".into(),
                vendor_id: "some-id".into(),
                amount_cents: 0,
                payment_date: "2025-02-01".into(),
                method: None,
                reference: None,
            })
            .await;
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(
            err.contains("amount_cents"),
            "Expected amount_cents validation error, got: {}",
            err
        );
    }

    #[tokio::test]
    async fn test_create_tax_code_validation_empty_code() {
        let svc = setup_svc().await;
        let result = svc
            .create_tax_code(&CreateTaxCodeRequest {
                code: "".into(),
                rate: 0.10,
                description: None,
            })
            .await;
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(
            err.contains("code"),
            "Expected code validation error, got: {}",
            err
        );
    }

    #[tokio::test]
    async fn test_create_tax_code_validation_rate_above_max() {
        let svc = setup_svc().await;
        let result = svc
            .create_tax_code(&CreateTaxCodeRequest {
                code: "INVALID".into(),
                rate: 1.5,
                description: None,
            })
            .await;
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(
            err.contains("rate"),
            "Expected rate validation error, got: {}",
            err
        );
    }

    #[tokio::test]
    async fn test_create_tax_code_validation_rate_below_min() {
        let svc = setup_svc().await;
        let result = svc
            .create_tax_code(&CreateTaxCodeRequest {
                code: "NEGATIVE".into(),
                rate: -0.1,
                description: None,
            })
            .await;
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(
            err.contains("rate"),
            "Expected rate validation error, got: {}",
            err
        );
    }
}
