use sqlx::SqlitePool;
use saas_common::error::{AppError, AppResult};
use crate::models::*;

#[derive(Clone)]
pub struct ArRepo {
    pool: SqlitePool,
}

impl ArRepo {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    // --- Customers ---

    pub async fn list_customers(&self) -> AppResult<Vec<Customer>> {
        let rows = sqlx::query_as::<_, Customer>(
            "SELECT id, name, email, phone, address, is_active, created_at FROM customers ORDER BY name",
        )
        .fetch_all(&self.pool)
        .await?;
        Ok(rows)
    }

    pub async fn get_customer(&self, id: &str) -> AppResult<Customer> {
        sqlx::query_as::<_, Customer>(
            "SELECT id, name, email, phone, address, is_active, created_at FROM customers WHERE id = ?",
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("Customer '{}' not found", id)))
    }

    pub async fn create_customer(&self, input: &CreateCustomerRequest) -> AppResult<Customer> {
        let id = uuid::Uuid::new_v4().to_string();
        sqlx::query(
            "INSERT INTO customers (id, name, email, phone, address) VALUES (?, ?, ?, ?, ?)",
        )
        .bind(&id)
        .bind(&input.name)
        .bind(&input.email)
        .bind(&input.phone)
        .bind(&input.address)
        .execute(&self.pool)
        .await?;
        self.get_customer(&id).await
    }

    pub async fn update_customer(&self, id: &str, input: &UpdateCustomerRequest) -> AppResult<Customer> {
        let current = self.get_customer(id).await?;
        let name = input.name.as_deref().unwrap_or(&current.name);
        let email = input.email.as_deref().or(current.email.as_deref());
        let phone = input.phone.as_deref().or(current.phone.as_deref());
        let address = input.address.as_deref().or(current.address.as_deref());
        let is_active = input.is_active.map(|b| if b { 1 } else { 0 }).unwrap_or(current.is_active);

        sqlx::query(
            "UPDATE customers SET name = ?, email = ?, phone = ?, address = ?, is_active = ? WHERE id = ?",
        )
        .bind(name)
        .bind(email)
        .bind(phone)
        .bind(address)
        .bind(is_active)
        .bind(id)
        .execute(&self.pool)
        .await?;
        self.get_customer(id).await
    }

    // --- AR Invoices ---

    pub async fn list_invoices(&self) -> AppResult<Vec<ArInvoice>> {
        let rows = sqlx::query_as::<_, ArInvoice>(
            "SELECT id, customer_id, invoice_number, invoice_date, due_date, total_cents, status, created_at FROM ar_invoices ORDER BY created_at DESC",
        )
        .fetch_all(&self.pool)
        .await?;
        Ok(rows)
    }

    pub async fn get_invoice(&self, id: &str) -> AppResult<ArInvoice> {
        sqlx::query_as::<_, ArInvoice>(
            "SELECT id, customer_id, invoice_number, invoice_date, due_date, total_cents, status, created_at FROM ar_invoices WHERE id = ?",
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("AR Invoice '{}' not found", id)))
    }

    pub async fn get_invoice_lines(&self, invoice_id: &str) -> AppResult<Vec<ArInvoiceLine>> {
        let rows = sqlx::query_as::<_, ArInvoiceLine>(
            "SELECT id, invoice_id, description, amount_cents FROM ar_invoice_lines WHERE invoice_id = ?",
        )
        .bind(invoice_id)
        .fetch_all(&self.pool)
        .await?;
        Ok(rows)
    }

    pub async fn create_invoice(&self, input: &CreateArInvoiceRequest) -> AppResult<ArInvoice> {
        let id = uuid::Uuid::new_v4().to_string();
        let total_cents: i64 = input.lines.iter().map(|l| l.amount_cents).sum();

        sqlx::query(
            "INSERT INTO ar_invoices (id, customer_id, invoice_number, invoice_date, due_date, total_cents) VALUES (?, ?, ?, ?, ?, ?)",
        )
        .bind(&id)
        .bind(&input.customer_id)
        .bind(&input.invoice_number)
        .bind(&input.invoice_date)
        .bind(&input.due_date)
        .bind(total_cents)
        .execute(&self.pool)
        .await?;

        for line in &input.lines {
            let line_id = uuid::Uuid::new_v4().to_string();
            sqlx::query(
                "INSERT INTO ar_invoice_lines (id, invoice_id, description, amount_cents) VALUES (?, ?, ?, ?)",
            )
            .bind(&line_id)
            .bind(&id)
            .bind(&line.description)
            .bind(line.amount_cents)
            .execute(&self.pool)
            .await?;
        }

        self.get_invoice(&id).await
    }

    pub async fn mark_invoice_sent(&self, id: &str) -> AppResult<ArInvoice> {
        sqlx::query("UPDATE ar_invoices SET status = 'sent' WHERE id = ?")
            .bind(id)
            .execute(&self.pool)
            .await?;
        self.get_invoice(id).await
    }

    pub async fn mark_invoice_paid(&self, id: &str) -> AppResult<ArInvoice> {
        sqlx::query("UPDATE ar_invoices SET status = 'paid' WHERE id = ?")
            .bind(id)
            .execute(&self.pool)
            .await?;
        self.get_invoice(id).await
    }

    // --- Receipts ---

    pub async fn list_receipts(&self) -> AppResult<Vec<Receipt>> {
        let rows = sqlx::query_as::<_, Receipt>(
            "SELECT id, invoice_id, customer_id, amount_cents, receipt_date, method, created_at FROM receipts ORDER BY created_at DESC",
        )
        .fetch_all(&self.pool)
        .await?;
        Ok(rows)
    }

    pub async fn create_receipt(&self, input: &CreateReceiptRequest) -> AppResult<Receipt> {
        let id = uuid::Uuid::new_v4().to_string();
        let method = input.method.as_deref().unwrap_or("wire");

        sqlx::query(
            "INSERT INTO receipts (id, invoice_id, customer_id, amount_cents, receipt_date, method) VALUES (?, ?, ?, ?, ?, ?)",
        )
        .bind(&id)
        .bind(&input.invoice_id)
        .bind(&input.customer_id)
        .bind(input.amount_cents)
        .bind(&input.receipt_date)
        .bind(method)
        .execute(&self.pool)
        .await?;

        // Update invoice status to paid
        self.mark_invoice_paid(&input.invoice_id).await?;

        sqlx::query_as::<_, Receipt>(
            "SELECT id, invoice_id, customer_id, amount_cents, receipt_date, method, created_at FROM receipts WHERE id = ?",
        )
        .bind(&id)
        .fetch_one(&self.pool)
        .await
        .map_err(|e| e.into())
    }
}
