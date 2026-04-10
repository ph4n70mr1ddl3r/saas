use crate::models::*;
use saas_common::error::{AppError, AppResult};
use sqlx::SqlitePool;

#[derive(Clone)]
pub struct ApRepo {
    pool: SqlitePool,
}

impl ApRepo {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    // --- Vendors ---

    pub async fn list_vendors(&self) -> AppResult<Vec<Vendor>> {
        let rows = sqlx::query_as::<_, Vendor>(
            "SELECT id, name, email, phone, address, is_active, created_at FROM vendors ORDER BY name",
        )
        .fetch_all(&self.pool)
        .await?;
        Ok(rows)
    }

    pub async fn get_vendor(&self, id: &str) -> AppResult<Vendor> {
        sqlx::query_as::<_, Vendor>(
            "SELECT id, name, email, phone, address, is_active, created_at FROM vendors WHERE id = ?",
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("Vendor '{}' not found", id)))
    }

    pub async fn create_vendor(&self, input: &CreateVendorRequest) -> AppResult<Vendor> {
        let id = uuid::Uuid::new_v4().to_string();
        sqlx::query("INSERT INTO vendors (id, name, email, phone, address) VALUES (?, ?, ?, ?, ?)")
            .bind(&id)
            .bind(&input.name)
            .bind(&input.email)
            .bind(&input.phone)
            .bind(&input.address)
            .execute(&self.pool)
            .await?;
        self.get_vendor(&id).await
    }

    pub async fn update_vendor(&self, id: &str, input: &UpdateVendorRequest) -> AppResult<Vendor> {
        let current = self.get_vendor(id).await?;
        let name = input.name.as_deref().unwrap_or(&current.name);
        let email = input.email.as_deref().or(current.email.as_deref());
        let phone = input.phone.as_deref().or(current.phone.as_deref());
        let address = input.address.as_deref().or(current.address.as_deref());
        let is_active = input
            .is_active
            .map(|b| if b { 1 } else { 0 })
            .unwrap_or(current.is_active);

        sqlx::query(
            "UPDATE vendors SET name = ?, email = ?, phone = ?, address = ?, is_active = ? WHERE id = ?",
        )
        .bind(name)
        .bind(email)
        .bind(phone)
        .bind(address)
        .bind(is_active)
        .bind(id)
        .execute(&self.pool)
        .await?;
        self.get_vendor(id).await
    }

    // --- AP Invoices ---

    pub async fn list_invoices(&self) -> AppResult<Vec<ApInvoice>> {
        let rows = sqlx::query_as::<_, ApInvoice>(
            "SELECT id, vendor_id, invoice_number, invoice_date, due_date, total_cents, tax_amount_cents, status, created_at FROM ap_invoices ORDER BY created_at DESC",
        )
        .fetch_all(&self.pool)
        .await?;
        Ok(rows)
    }

    pub async fn get_invoice(&self, id: &str) -> AppResult<ApInvoice> {
        sqlx::query_as::<_, ApInvoice>(
            "SELECT id, vendor_id, invoice_number, invoice_date, due_date, total_cents, tax_amount_cents, status, created_at FROM ap_invoices WHERE id = ?",
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("AP Invoice '{}' not found", id)))
    }

    pub async fn get_invoice_lines(&self, invoice_id: &str) -> AppResult<Vec<ApInvoiceLine>> {
        let rows = sqlx::query_as::<_, ApInvoiceLine>(
            "SELECT id, invoice_id, description, account_code, amount_cents FROM ap_invoice_lines WHERE invoice_id = ?",
        )
        .bind(invoice_id)
        .fetch_all(&self.pool)
        .await?;
        Ok(rows)
    }

    /// Create invoice header + lines inside a database transaction.
    pub async fn create_invoice(&self, input: &CreateApInvoiceRequest, tax_amount_cents: i64) -> AppResult<ApInvoice> {
        let id = uuid::Uuid::new_v4().to_string();
        let total_cents: i64 = input.lines.iter().map(|l| l.amount_cents).sum();

        let mut tx = self.pool.begin().await?;

        sqlx::query(
            "INSERT INTO ap_invoices (id, vendor_id, invoice_number, invoice_date, due_date, total_cents, tax_amount_cents) VALUES (?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(&id)
        .bind(&input.vendor_id)
        .bind(&input.invoice_number)
        .bind(&input.invoice_date)
        .bind(&input.due_date)
        .bind(total_cents)
        .bind(tax_amount_cents)
        .execute(&mut *tx)
        .await?;

        for line in &input.lines {
            let line_id = uuid::Uuid::new_v4().to_string();
            sqlx::query(
                "INSERT INTO ap_invoice_lines (id, invoice_id, description, account_code, amount_cents) VALUES (?, ?, ?, ?, ?)",
            )
            .bind(&line_id)
            .bind(&id)
            .bind(&line.description)
            .bind(&line.account_code)
            .bind(line.amount_cents)
            .execute(&mut *tx)
            .await?;
        }

        tx.commit().await?;
        self.get_invoice(&id).await
    }

    pub async fn approve_invoice(&self, id: &str) -> AppResult<ApInvoice> {
        sqlx::query("UPDATE ap_invoices SET status = 'approved' WHERE id = ?")
            .bind(id)
            .execute(&self.pool)
            .await?;
        self.get_invoice(id).await
    }

    pub async fn cancel_invoice(&self, id: &str) -> AppResult<ApInvoice> {
        sqlx::query("UPDATE ap_invoices SET status = 'cancelled' WHERE id = ?")
            .bind(id)
            .execute(&self.pool)
            .await?;
        self.get_invoice(id).await
    }

    // --- Payments ---

    pub async fn list_payments(&self) -> AppResult<Vec<Payment>> {
        let rows = sqlx::query_as::<_, Payment>(
            "SELECT id, invoice_id, vendor_id, amount_cents, payment_date, method, reference, created_at FROM payments ORDER BY created_at DESC",
        )
        .fetch_all(&self.pool)
        .await?;
        Ok(rows)
    }

    /// Create payment inside a transaction with overpayment prevention.
    /// Reads the invoice, checks total prior payments, and ensures the
    /// new payment does not exceed the remaining balance.
    pub async fn create_payment(&self, input: &CreatePaymentRequest) -> AppResult<Payment> {
        let id = uuid::Uuid::new_v4().to_string();
        let method = input.method.as_deref().unwrap_or("wire");

        let mut tx = self.pool.begin().await?;

        // Read the invoice within the transaction for consistent view
        let invoice: ApInvoice = sqlx::query_as::<_, ApInvoice>(
            "SELECT id, vendor_id, invoice_number, invoice_date, due_date, total_cents, tax_amount_cents, status, created_at FROM ap_invoices WHERE id = ?",
        )
        .bind(&input.invoice_id)
        .fetch_optional(&mut *tx)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("AP Invoice '{}' not found", input.invoice_id)))?;

        // Sum existing payments for this invoice
        let previously_paid: i64 = sqlx::query_scalar(
            "SELECT COALESCE(SUM(amount_cents), 0) FROM payments WHERE invoice_id = ?",
        )
        .bind(&input.invoice_id)
        .fetch_one(&mut *tx)
        .await?;

        let remaining = invoice.total_cents - previously_paid;

        // Overpayment prevention: payment must not exceed remaining balance
        if input.amount_cents > remaining {
            return Err(AppError::Validation(format!(
                "Payment of {} cents exceeds remaining balance of {} cents on invoice '{}'",
                input.amount_cents, remaining, input.invoice_id
            )));
        }

        sqlx::query(
            "INSERT INTO payments (id, invoice_id, vendor_id, amount_cents, payment_date, method, reference) VALUES (?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(&id)
        .bind(&input.invoice_id)
        .bind(&input.vendor_id)
        .bind(input.amount_cents)
        .bind(&input.payment_date)
        .bind(method)
        .bind(&input.reference)
        .execute(&mut *tx)
        .await?;

        // Update invoice status: mark as paid only if fully paid
        let total_paid = previously_paid + input.amount_cents;
        if total_paid >= invoice.total_cents {
            sqlx::query("UPDATE ap_invoices SET status = 'paid' WHERE id = ?")
                .bind(&input.invoice_id)
                .execute(&mut *tx)
                .await?;
        } else {
            sqlx::query("UPDATE ap_invoices SET status = 'partial' WHERE id = ?")
                .bind(&input.invoice_id)
                .execute(&mut *tx)
                .await?;
        }

        tx.commit().await?;

        sqlx::query_as::<_, Payment>(
            "SELECT id, invoice_id, vendor_id, amount_cents, payment_date, method, reference, created_at FROM payments WHERE id = ?",
        )
        .bind(&id)
        .fetch_one(&self.pool)
        .await
        .map_err(|e| e.into())
    }

    // --- Tax Codes ---

    pub async fn create_tax_code(&self, input: &CreateTaxCodeRequest) -> AppResult<TaxCode> {
        let id = uuid::Uuid::new_v4().to_string();
        sqlx::query("INSERT INTO tax_codes (id, code, rate, description) VALUES (?, ?, ?, ?)")
            .bind(&id)
            .bind(&input.code)
            .bind(input.rate)
            .bind(&input.description)
            .execute(&self.pool)
            .await?;
        self.get_tax_code(&id).await
    }

    pub async fn list_tax_codes(&self) -> AppResult<Vec<TaxCode>> {
        let rows = sqlx::query_as::<_, TaxCode>(
            "SELECT id, code, rate, description, is_active, created_at FROM tax_codes ORDER BY code",
        )
        .fetch_all(&self.pool)
        .await?;
        Ok(rows)
    }

    pub async fn get_tax_code(&self, id: &str) -> AppResult<TaxCode> {
        sqlx::query_as::<_, TaxCode>(
            "SELECT id, code, rate, description, is_active, created_at FROM tax_codes WHERE id = ?",
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("Tax code '{}' not found", id)))
    }

    pub async fn get_tax_code_by_code(&self, code: &str) -> AppResult<TaxCode> {
        sqlx::query_as::<_, TaxCode>(
            "SELECT id, code, rate, description, is_active, created_at FROM tax_codes WHERE code = ? AND is_active = 1",
        )
        .bind(code)
        .fetch_optional(&self.pool)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("Tax code '{}' not found or inactive", code)))
    }

    // --- Period Close Enforcement ---

    pub async fn close_period(&self, period_name: &str, fiscal_year: i32, period_start: &str, period_end: &str) -> AppResult<()> {
        let id = uuid::Uuid::new_v4().to_string();
        sqlx::query("INSERT OR IGNORE INTO closed_periods (id, period_name, fiscal_year, period_start, period_end) VALUES (?, ?, ?, ?, ?)")
            .bind(&id).bind(period_name).bind(fiscal_year).bind(period_start).bind(period_end)
            .execute(&self.pool).await?;
        Ok(())
    }

    pub async fn close_fiscal_year(&self, fiscal_year: i32) -> AppResult<()> {
        let id = uuid::Uuid::new_v4().to_string();
        sqlx::query("INSERT OR IGNORE INTO closed_fiscal_years (id, fiscal_year) VALUES (?, ?)")
            .bind(&id).bind(fiscal_year)
            .execute(&self.pool).await?;
        Ok(())
    }

    pub async fn is_date_in_closed_period(&self, date: &str) -> AppResult<bool> {
        let closed: bool = sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM closed_periods WHERE ? >= period_start AND ? <= period_end"
        )
        .bind(date).bind(date)
        .fetch_one(&self.pool).await? > 0;
        if closed { return Ok(true); }
        // Also check fiscal year
        let year: i32 = date.get(..4).and_then(|s| s.parse().ok()).unwrap_or(0);
        let year_closed: bool = sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM closed_fiscal_years WHERE fiscal_year = ?"
        )
        .bind(year)
        .fetch_one(&self.pool).await? > 0;
        Ok(year_closed)
    }

    // --- Aging Report ---

    pub async fn aging_report(&self, as_of_date: &str) -> AppResult<Vec<ApAgingRow>> {
        let rows = sqlx::query_as::<_, ApAgingRow>(
            r#"
            SELECT
                v.id AS vendor_id,
                v.name AS vendor_name,
                i.id AS invoice_id,
                i.invoice_number,
                i.total_cents,
                i.due_date,
                CASE
                    WHEN julianday(?) - julianday(i.due_date) <= 0 THEN 'current'
                    WHEN julianday(?) - julianday(i.due_date) <= 30 THEN '1-30'
                    WHEN julianday(?) - julianday(i.due_date) <= 60 THEN '31-60'
                    WHEN julianday(?) - julianday(i.due_date) <= 90 THEN '61-90'
                    ELSE '90+'
                END AS aging_bucket
            FROM ap_invoices i
            JOIN vendors v ON v.id = i.vendor_id
            WHERE i.status IN ('approved', 'partial')
            ORDER BY v.name, i.due_date
            "#,
        )
        .bind(as_of_date)
        .bind(as_of_date)
        .bind(as_of_date)
        .bind(as_of_date)
        .fetch_all(&self.pool)
        .await?;
        Ok(rows)
    }
}
