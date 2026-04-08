use crate::models::*;
use saas_common::error::{AppError, AppResult};
use sqlx::SqlitePool;

#[derive(Clone)]
pub struct PayrollRepo {
    pool: SqlitePool,
}

impl PayrollRepo {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    // --- Compensation ---

    pub async fn list_compensation(&self) -> AppResult<Vec<Compensation>> {
        let rows = sqlx::query_as::<_, Compensation>(
            "SELECT id, employee_id, salary_type, amount_cents, currency, effective_date, end_date, created_at FROM compensation ORDER BY created_at DESC"
        )
        .fetch_all(&self.pool)
        .await?;
        Ok(rows)
    }

    pub async fn get_compensation(&self, id: &str) -> AppResult<Compensation> {
        sqlx::query_as::<_, Compensation>(
            "SELECT id, employee_id, salary_type, amount_cents, currency, effective_date, end_date, created_at FROM compensation WHERE id = ?"
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("Compensation '{}' not found", id)))
    }

    pub async fn list_compensation_by_employee(
        &self,
        employee_id: &str,
    ) -> AppResult<Vec<Compensation>> {
        let rows = sqlx::query_as::<_, Compensation>(
            "SELECT id, employee_id, salary_type, amount_cents, currency, effective_date, end_date, created_at FROM compensation WHERE employee_id = ? ORDER BY effective_date DESC"
        )
        .bind(employee_id)
        .fetch_all(&self.pool)
        .await?;
        Ok(rows)
    }

    pub async fn create_compensation(
        &self,
        input: &CreateCompensationRequest,
    ) -> AppResult<Compensation> {
        let id = uuid::Uuid::new_v4().to_string();
        let currency = input.currency.as_deref().unwrap_or("USD");
        sqlx::query(
            "INSERT INTO compensation (id, employee_id, salary_type, amount_cents, currency, effective_date, end_date) VALUES (?, ?, ?, ?, ?, ?, ?)"
        )
        .bind(&id)
        .bind(&input.employee_id)
        .bind(&input.salary_type)
        .bind(input.amount_cents)
        .bind(currency)
        .bind(&input.effective_date)
        .bind(&input.end_date)
        .execute(&self.pool)
        .await?;
        self.get_compensation(&id).await
    }

    pub async fn update_compensation(
        &self,
        id: &str,
        input: &UpdateCompensationRequest,
    ) -> AppResult<Compensation> {
        let existing = self.get_compensation(id).await?;
        let salary_type = input
            .salary_type
            .as_deref()
            .unwrap_or(&existing.salary_type);
        let amount_cents = input.amount_cents.unwrap_or(existing.amount_cents);
        let currency = input.currency.as_deref().unwrap_or(&existing.currency);
        let effective_date = input
            .effective_date
            .as_deref()
            .unwrap_or(&existing.effective_date);
        let end_date = input.end_date.as_ref().or(existing.end_date.as_ref());

        sqlx::query(
            "UPDATE compensation SET salary_type = ?, amount_cents = ?, currency = ?, effective_date = ?, end_date = ? WHERE id = ?"
        )
        .bind(salary_type)
        .bind(amount_cents)
        .bind(currency)
        .bind(effective_date)
        .bind(end_date)
        .bind(id)
        .execute(&self.pool)
        .await?;
        self.get_compensation(id).await
    }

    // --- PayRuns ---

    pub async fn list_pay_runs(&self) -> AppResult<Vec<PayRun>> {
        let rows = sqlx::query_as::<_, PayRun>(
            "SELECT id, period_start, period_end, pay_date, status, created_at FROM pay_runs ORDER BY created_at DESC"
        )
        .fetch_all(&self.pool)
        .await?;
        Ok(rows)
    }

    pub async fn get_pay_run(&self, id: &str) -> AppResult<PayRun> {
        sqlx::query_as::<_, PayRun>(
            "SELECT id, period_start, period_end, pay_date, status, created_at FROM pay_runs WHERE id = ?"
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("Pay run '{}' not found", id)))
    }

    pub async fn create_pay_run(&self, input: &CreatePayRunRequest) -> AppResult<PayRun> {
        let id = uuid::Uuid::new_v4().to_string();
        sqlx::query(
            "INSERT INTO pay_runs (id, period_start, period_end, pay_date) VALUES (?, ?, ?, ?)",
        )
        .bind(&id)
        .bind(&input.period_start)
        .bind(&input.period_end)
        .bind(&input.pay_date)
        .execute(&self.pool)
        .await?;
        self.get_pay_run(&id).await
    }

    pub async fn update_pay_run_status(&self, id: &str, status: &str) -> AppResult<PayRun> {
        sqlx::query("UPDATE pay_runs SET status = ? WHERE id = ?")
            .bind(status)
            .bind(id)
            .execute(&self.pool)
            .await?;
        self.get_pay_run(id).await
    }

    // --- Payslips ---

    pub async fn list_payslips_for_run(&self, pay_run_id: &str) -> AppResult<Vec<Payslip>> {
        let rows = sqlx::query_as::<_, Payslip>(
            "SELECT id, pay_run_id, employee_id, gross_pay, net_pay, tax, deductions, status, created_at FROM payslips WHERE pay_run_id = ? ORDER BY employee_id"
        )
        .bind(pay_run_id)
        .fetch_all(&self.pool)
        .await?;
        Ok(rows)
    }

    pub async fn create_payslip(
        &self,
        pay_run_id: &str,
        employee_id: &str,
        gross_pay: i64,
        net_pay: i64,
        tax: i64,
        deductions: i64,
    ) -> AppResult<Payslip> {
        let id = uuid::Uuid::new_v4().to_string();
        sqlx::query(
            "INSERT INTO payslips (id, pay_run_id, employee_id, gross_pay, net_pay, tax, deductions) VALUES (?, ?, ?, ?, ?, ?, ?)"
        )
        .bind(&id)
        .bind(pay_run_id)
        .bind(employee_id)
        .bind(gross_pay)
        .bind(net_pay)
        .bind(tax)
        .bind(deductions)
        .execute(&self.pool)
        .await?;
        sqlx::query_as::<_, Payslip>(
            "SELECT id, pay_run_id, employee_id, gross_pay, net_pay, tax, deductions, status, created_at FROM payslips WHERE id = ?"
        )
        .bind(&id)
        .fetch_optional(&self.pool)
        .await?
        .ok_or_else(|| AppError::Internal("Failed to read created payslip".into()))
    }

    // --- Deductions ---

    pub async fn list_deductions_by_employee(
        &self,
        employee_id: &str,
    ) -> AppResult<Vec<Deduction>> {
        let rows = sqlx::query_as::<_, Deduction>(
            "SELECT id, employee_id, code, amount_cents, recurring, start_date, end_date FROM deductions WHERE employee_id = ? ORDER BY start_date DESC"
        )
        .bind(employee_id)
        .fetch_all(&self.pool)
        .await?;
        Ok(rows)
    }

    pub async fn create_deduction(&self, input: &CreateDeductionRequest) -> AppResult<Deduction> {
        let id = uuid::Uuid::new_v4().to_string();
        let recurring = input.recurring.unwrap_or(true);
        sqlx::query(
            "INSERT INTO deductions (id, employee_id, code, amount_cents, recurring, start_date, end_date) VALUES (?, ?, ?, ?, ?, ?, ?)"
        )
        .bind(&id)
        .bind(&input.employee_id)
        .bind(&input.code)
        .bind(input.amount_cents)
        .bind(recurring)
        .bind(&input.start_date)
        .bind(&input.end_date)
        .execute(&self.pool)
        .await?;
        sqlx::query_as::<_, Deduction>(
            "SELECT id, employee_id, code, amount_cents, recurring, start_date, end_date FROM deductions WHERE id = ?"
        )
        .bind(&id)
        .fetch_optional(&self.pool)
        .await?
        .ok_or_else(|| AppError::Internal("Failed to read created deduction".into()))
    }

    pub async fn get_deduction(&self, id: &str) -> AppResult<Deduction> {
        sqlx::query_as::<_, Deduction>(
            "SELECT id, employee_id, code, amount_cents, recurring, start_date, end_date FROM deductions WHERE id = ?"
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("Deduction '{}' not found", id)))
    }

    /// Find an active (no end_date) deduction for a given employee and code.
    pub async fn find_active_deduction_by_employee_and_code(
        &self,
        employee_id: &str,
        code: &str,
    ) -> AppResult<Option<Deduction>> {
        let row = sqlx::query_as::<_, Deduction>(
            "SELECT id, employee_id, code, amount_cents, recurring, start_date, end_date FROM deductions WHERE employee_id = ? AND code = ? AND end_date IS NULL ORDER BY start_date DESC LIMIT 1"
        )
        .bind(employee_id)
        .bind(code)
        .fetch_optional(&self.pool)
        .await?;
        Ok(row)
    }

    /// Deactivate a deduction by setting its end_date.
    pub async fn deactivate_deduction(&self, id: &str, end_date: &str) -> AppResult<Deduction> {
        sqlx::query("UPDATE deductions SET end_date = ? WHERE id = ?")
            .bind(end_date)
            .bind(id)
            .execute(&self.pool)
            .await?;
        self.get_deduction(id).await
    }

    // --- Tax Brackets ---

    pub async fn list_tax_brackets(&self) -> AppResult<Vec<TaxBracket>> {
        let rows = sqlx::query_as::<_, TaxBracket>(
            "SELECT id, name, min_income_cents, max_income_cents, rate_percent, created_at FROM tax_brackets ORDER BY min_income_cents ASC"
        )
        .fetch_all(&self.pool)
        .await?;
        Ok(rows)
    }

    pub async fn create_tax_bracket(
        &self,
        input: &CreateTaxBracketRequest,
    ) -> AppResult<TaxBracket> {
        let id = uuid::Uuid::new_v4().to_string();
        sqlx::query(
            "INSERT INTO tax_brackets (id, name, min_income_cents, max_income_cents, rate_percent) VALUES (?, ?, ?, ?, ?)"
        )
        .bind(&id)
        .bind(&input.name)
        .bind(input.min_income_cents)
        .bind(input.max_income_cents)
        .bind(input.rate_percent)
        .execute(&self.pool)
        .await?;
        sqlx::query_as::<_, TaxBracket>(
            "SELECT id, name, min_income_cents, max_income_cents, rate_percent, created_at FROM tax_brackets WHERE id = ?"
        )
        .bind(&id)
        .fetch_optional(&self.pool)
        .await?
        .ok_or_else(|| AppError::Internal("Failed to read created tax bracket".into()))
    }

    pub async fn get_tax_bracket(&self, id: &str) -> AppResult<TaxBracket> {
        sqlx::query_as::<_, TaxBracket>(
            "SELECT id, name, min_income_cents, max_income_cents, rate_percent, created_at FROM tax_brackets WHERE id = ?"
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("Tax bracket '{}' not found", id)))
    }
}
