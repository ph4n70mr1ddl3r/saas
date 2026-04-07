use crate::models::*;
use saas_common::error::{AppError, AppResult};
use sqlx::SqlitePool;

#[derive(Clone)]
pub struct LedgerRepo {
    pool: SqlitePool,
}

impl LedgerRepo {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    // --- Accounts ---

    pub async fn list_accounts(&self) -> AppResult<Vec<Account>> {
        let rows = sqlx::query_as::<_, Account>(
            "SELECT id, code, name, account_type, parent_id, is_active, created_at FROM accounts ORDER BY code",
        )
        .fetch_all(&self.pool)
        .await?;
        Ok(rows)
    }

    pub async fn get_account(&self, id: &str) -> AppResult<Account> {
        sqlx::query_as::<_, Account>(
            "SELECT id, code, name, account_type, parent_id, is_active, created_at FROM accounts WHERE id = ?",
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("Account '{}' not found", id)))
    }

    pub async fn create_account(&self, input: &CreateAccountRequest) -> AppResult<Account> {
        let id = uuid::Uuid::new_v4().to_string();
        sqlx::query(
            "INSERT INTO accounts (id, code, name, account_type, parent_id) VALUES (?, ?, ?, ?, ?)",
        )
        .bind(&id)
        .bind(&input.code)
        .bind(&input.name)
        .bind(&input.account_type)
        .bind(&input.parent_id)
        .execute(&self.pool)
        .await?;
        self.get_account(&id).await
    }

    // --- Periods ---

    pub async fn list_periods(&self) -> AppResult<Vec<Period>> {
        let rows = sqlx::query_as::<_, Period>(
            "SELECT id, name, start_date, end_date, status, fiscal_year FROM periods ORDER BY start_date DESC",
        )
        .fetch_all(&self.pool)
        .await?;
        Ok(rows)
    }

    pub async fn get_period(&self, id: &str) -> AppResult<Period> {
        sqlx::query_as::<_, Period>(
            "SELECT id, name, start_date, end_date, status, fiscal_year FROM periods WHERE id = ?",
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("Period '{}' not found", id)))
    }

    pub async fn create_period(&self, input: &CreatePeriodRequest) -> AppResult<Period> {
        let id = uuid::Uuid::new_v4().to_string();
        sqlx::query(
            "INSERT INTO periods (id, name, start_date, end_date, fiscal_year) VALUES (?, ?, ?, ?, ?)",
        )
        .bind(&id)
        .bind(&input.name)
        .bind(&input.start_date)
        .bind(&input.end_date)
        .bind(input.fiscal_year)
        .execute(&self.pool)
        .await?;
        self.get_period(&id).await
    }

    pub async fn close_period(&self, id: &str) -> AppResult<Period> {
        let mut tx = self.pool.begin().await?;

        let period = sqlx::query_as::<_, Period>(
            "SELECT id, name, start_date, end_date, status, fiscal_year FROM periods WHERE id = ?",
        )
        .bind(id)
        .fetch_optional(&mut *tx)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("Period '{}' not found", id)))?;

        if period.status == "closed" {
            return Err(AppError::Validation("Period is already closed".into()));
        }

        // Check for draft journal entries in this period
        let draft_count: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM journal_entries WHERE period_id = ? AND status = 'draft'",
        )
        .bind(id)
        .fetch_one(&mut *tx)
        .await?;

        if draft_count > 0 {
            return Err(AppError::Validation(format!(
                "Cannot close period: {} draft journal entries exist",
                draft_count
            )));
        }

        sqlx::query("UPDATE periods SET status = 'closed' WHERE id = ?")
            .bind(id)
            .execute(&mut *tx)
            .await?;

        tx.commit().await?;
        self.get_period(id).await
    }

    // --- Journal Entries ---

    pub async fn list_journal_entries(&self) -> AppResult<Vec<JournalEntry>> {
        let rows = sqlx::query_as::<_, JournalEntry>(
            "SELECT id, entry_number, description, period_id, status, posted_at, created_by, created_at FROM journal_entries ORDER BY created_at DESC",
        )
        .fetch_all(&self.pool)
        .await?;
        Ok(rows)
    }

    pub async fn get_journal_entry(&self, id: &str) -> AppResult<JournalEntry> {
        sqlx::query_as::<_, JournalEntry>(
            "SELECT id, entry_number, description, period_id, status, posted_at, created_by, created_at FROM journal_entries WHERE id = ?",
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("Journal entry '{}' not found", id)))
    }

    pub async fn get_journal_lines(&self, entry_id: &str) -> AppResult<Vec<JournalLine>> {
        let rows = sqlx::query_as::<_, JournalLine>(
            "SELECT id, entry_id, account_id, debit_cents, credit_cents, description, created_at FROM journal_lines WHERE entry_id = ?",
        )
        .bind(entry_id)
        .fetch_all(&self.pool)
        .await?;
        Ok(rows)
    }

    /// Create a journal entry with its lines inside a database transaction.
    pub async fn create_journal_entry(
        &self,
        entry_number: &str,
        input: &CreateJournalEntryRequest,
        created_by: &str,
    ) -> AppResult<JournalEntry> {
        let mut tx = self.pool.begin().await?;

        let id = uuid::Uuid::new_v4().to_string();
        sqlx::query(
            "INSERT INTO journal_entries (id, entry_number, description, period_id, created_by) VALUES (?, ?, ?, ?, ?)",
        )
        .bind(&id)
        .bind(entry_number)
        .bind(&input.description)
        .bind(&input.period_id)
        .bind(created_by)
        .execute(&mut *tx)
        .await?;

        for line in &input.lines {
            let line_id = uuid::Uuid::new_v4().to_string();
            sqlx::query(
                "INSERT INTO journal_lines (id, entry_id, account_id, debit_cents, credit_cents, description) VALUES (?, ?, ?, ?, ?, ?)",
            )
            .bind(&line_id)
            .bind(&id)
            .bind(&line.account_id)
            .bind(line.debit_cents)
            .bind(line.credit_cents)
            .bind(&line.description)
            .execute(&mut *tx)
            .await?;
        }

        tx.commit().await?;
        self.get_journal_entry(&id).await
    }

    pub async fn post_journal_entry(&self, id: &str) -> AppResult<JournalEntry> {
        let mut tx = self.pool.begin().await?;

        let now = chrono::Utc::now().to_rfc3339();
        sqlx::query("UPDATE journal_entries SET status = 'posted', posted_at = ? WHERE id = ?")
            .bind(&now)
            .bind(id)
            .execute(&mut *tx)
            .await?;

        tx.commit().await?;
        self.get_journal_entry(id).await
    }

    /// Reverse a journal entry by creating a counter-entry with swapped debit/credit lines,
    /// then marking the original as reversed. All within a single transaction.
    pub async fn reverse_journal_entry(&self, id: &str) -> AppResult<JournalEntry> {
        let mut tx = self.pool.begin().await?;

        // Mark original as reversed
        sqlx::query("UPDATE journal_entries SET status = 'reversed' WHERE id = ?")
            .bind(id)
            .execute(&mut *tx)
            .await?;

        // Create counter-entry with swapped lines
        let reversal_id = uuid::Uuid::new_v4().to_string();
        let original = sqlx::query_as::<_, JournalEntry>(
            "SELECT id, entry_number, description, period_id, status, posted_at, created_by, created_at FROM journal_entries WHERE id = ?",
        )
        .bind(id)
        .fetch_one(&mut *tx)
        .await?;

        let reversal_number = format!("REV-{}", original.entry_number);

        sqlx::query(
            "INSERT INTO journal_entries (id, entry_number, description, period_id, created_by) VALUES (?, ?, ?, ?, ?)",
        )
        .bind(&reversal_id)
        .bind(&reversal_number)
        .bind(format!("Reversal of {}", original.entry_number))
        .bind(&original.period_id)
        .bind(&original.created_by)
        .execute(&mut *tx)
        .await?;

        // Copy lines with swapped debit/credit
        let original_lines = sqlx::query_as::<_, JournalLine>(
            "SELECT id, entry_id, account_id, debit_cents, credit_cents, description, created_at FROM journal_lines WHERE entry_id = ?",
        )
        .bind(id)
        .fetch_all(&mut *tx)
        .await?;

        for line in &original_lines {
            let line_id = uuid::Uuid::new_v4().to_string();
            sqlx::query(
                "INSERT INTO journal_lines (id, entry_id, account_id, debit_cents, credit_cents, description) VALUES (?, ?, ?, ?, ?, ?)",
            )
            .bind(&line_id)
            .bind(&reversal_id)
            .bind(&line.account_id)
            .bind(line.credit_cents) // swap: credit becomes debit
            .bind(line.debit_cents)  // swap: debit becomes credit
            .bind(format!("Reversal of JE {}", original.entry_number))
            .execute(&mut *tx)
            .await?;
        }

        // Post the reversal entry immediately
        let now = chrono::Utc::now().to_rfc3339();
        sqlx::query("UPDATE journal_entries SET status = 'posted', posted_at = ? WHERE id = ?")
            .bind(&now)
            .bind(&reversal_id)
            .execute(&mut *tx)
            .await?;

        tx.commit().await?;
        self.get_journal_entry(id).await
    }

    /// Generate the next entry number atomically using a counter table.
    /// Falls back to COUNT-based if the counter table doesn't exist.
    pub async fn next_entry_number(&self) -> AppResult<String> {
        // Try atomic counter first
        let result = sqlx::query_scalar::<_, i64>(
            "UPDATE je_counter SET last_value = last_value + 1 RETURNING last_value",
        )
        .fetch_one(&self.pool)
        .await;

        match result {
            Ok(next) => Ok(format!("JE-{:06}", next)),
            Err(_) => {
                // Fallback: use UUID suffix to guarantee uniqueness under concurrency
                let suffix = &uuid::Uuid::new_v4().to_string()[..8];
                Ok(format!("JE-{}", suffix))
            }
        }
    }

    // --- Trial Balance ---

    pub async fn trial_balance(&self) -> AppResult<Vec<TrialBalanceRow>> {
        let rows = sqlx::query_as::<_, TrialBalanceRow>(
            r#"SELECT a.code AS account_code, a.name AS account_name, a.account_type,
                      COALESCE(SUM(jl.debit_cents), 0) AS total_debit_cents,
                      COALESCE(SUM(jl.credit_cents), 0) AS total_credit_cents
               FROM accounts a
               LEFT JOIN journal_lines jl ON jl.account_id = a.id
               LEFT JOIN journal_entries je ON je.id = jl.entry_id AND je.status = 'posted'
               GROUP BY a.id
               ORDER BY a.code"#,
        )
        .fetch_all(&self.pool)
        .await?;
        Ok(rows)
    }

    // --- Balance Sheet ---

    pub async fn balance_sheet(&self) -> AppResult<Vec<BalanceSheetRow>> {
        let rows = sqlx::query_as::<_, BalanceSheetRow>(
            r#"SELECT a.code AS account_code, a.name AS account_name, a.account_type,
                      CASE
                        WHEN a.account_type IN ('asset', 'expense') THEN
                          COALESCE(SUM(jl.debit_cents), 0) - COALESCE(SUM(jl.credit_cents), 0)
                        ELSE
                          COALESCE(SUM(jl.credit_cents), 0) - COALESCE(SUM(jl.debit_cents), 0)
                      END AS balance_cents
               FROM accounts a
               LEFT JOIN journal_lines jl ON jl.account_id = a.id
               LEFT JOIN journal_entries je ON je.id = jl.entry_id AND je.status = 'posted'
               WHERE a.account_type IN ('asset', 'liability', 'equity')
               GROUP BY a.id
               ORDER BY a.account_type, a.code"#,
        )
        .fetch_all(&self.pool)
        .await?;
        Ok(rows)
    }

    // --- Income Statement ---

    pub async fn income_statement(
        &self,
        period_start: &str,
        period_end: &str,
    ) -> AppResult<Vec<IncomeStatementRow>> {
        let rows = sqlx::query_as::<_, IncomeStatementRow>(
            r#"SELECT a.code AS account_code, a.name AS account_name, a.account_type,
                      CASE
                        WHEN a.account_type = 'revenue' THEN
                          COALESCE(SUM(jl.credit_cents), 0) - COALESCE(SUM(jl.debit_cents), 0)
                        ELSE
                          COALESCE(SUM(jl.debit_cents), 0) - COALESCE(SUM(jl.credit_cents), 0)
                      END AS balance_cents
               FROM accounts a
               JOIN journal_lines jl ON jl.account_id = a.id
               JOIN journal_entries je ON je.id = jl.entry_id AND je.status = 'posted'
               JOIN periods p ON p.id = je.period_id
               WHERE a.account_type IN ('revenue', 'expense')
                 AND p.start_date >= ? AND p.end_date <= ?
               GROUP BY a.id
               ORDER BY a.account_type, a.code"#,
        )
        .bind(period_start)
        .bind(period_end)
        .fetch_all(&self.pool)
        .await?;
        Ok(rows)
    }

    // --- Budgets ---

    pub async fn create_budget(
        &self,
        input: &CreateBudgetRequest,
        created_by: &str,
    ) -> AppResult<Budget> {
        let mut tx = self.pool.begin().await?;
        let id = uuid::Uuid::new_v4().to_string();
        let total: i64 = input.lines.iter().map(|l| l.budgeted_cents).sum();

        sqlx::query(
            "INSERT INTO budgets (id, name, period_id, status, total_budget_cents, created_by) VALUES (?, ?, ?, 'draft', ?, ?)",
        )
        .bind(&id)
        .bind(&input.name)
        .bind(&input.period_id)
        .bind(total)
        .bind(created_by)
        .execute(&mut *tx)
        .await?;

        for line in &input.lines {
            let line_id = uuid::Uuid::new_v4().to_string();
            sqlx::query(
                "INSERT INTO budget_lines (id, budget_id, account_id, budgeted_cents) VALUES (?, ?, ?, ?)",
            )
            .bind(&line_id)
            .bind(&id)
            .bind(&line.account_id)
            .bind(line.budgeted_cents)
            .execute(&mut *tx)
            .await?;
        }

        tx.commit().await?;
        self.get_budget(&id).await
    }

    pub async fn get_budget(&self, id: &str) -> AppResult<Budget> {
        sqlx::query_as::<_, Budget>(
            "SELECT id, name, period_id, status, total_budget_cents, created_by, created_at FROM budgets WHERE id = ?",
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("Budget '{}' not found", id)))
    }

    pub async fn list_budgets(&self) -> AppResult<Vec<Budget>> {
        let rows = sqlx::query_as::<_, Budget>(
            "SELECT id, name, period_id, status, total_budget_cents, created_by, created_at FROM budgets ORDER BY created_at DESC",
        )
        .fetch_all(&self.pool)
        .await?;
        Ok(rows)
    }

    pub async fn get_budget_lines(&self, budget_id: &str) -> AppResult<Vec<BudgetLine>> {
        let rows = sqlx::query_as::<_, BudgetLine>(
            "SELECT id, budget_id, account_id, budgeted_cents, created_at FROM budget_lines WHERE budget_id = ?",
        )
        .bind(budget_id)
        .fetch_all(&self.pool)
        .await?;
        Ok(rows)
    }

    pub async fn update_budget_status(&self, id: &str, status: &str) -> AppResult<Budget> {
        sqlx::query("UPDATE budgets SET status = ? WHERE id = ?")
            .bind(status)
            .bind(id)
            .execute(&self.pool)
            .await?;
        self.get_budget(id).await
    }

    pub async fn budget_variance(&self, budget_id: &str) -> AppResult<Vec<BudgetVarianceRow>> {
        let rows = sqlx::query_as::<_, BudgetVarianceRow>(
            r#"SELECT a.code AS account_code, a.name AS account_name,
                      bl.budgeted_cents,
                      COALESCE(SUM(jl.debit_cents - jl.credit_cents), 0) AS actual_cents,
                      bl.budgeted_cents - COALESCE(SUM(jl.debit_cents - jl.credit_cents), 0) AS variance_cents
               FROM budget_lines bl
               JOIN accounts a ON a.id = bl.account_id
               JOIN budgets b ON b.id = bl.budget_id
               LEFT JOIN journal_lines jl ON jl.account_id = bl.account_id
               LEFT JOIN journal_entries je ON je.id = jl.entry_id AND je.status = 'posted' AND je.period_id = b.period_id
               WHERE bl.budget_id = ?
               GROUP BY bl.id
               ORDER BY a.code"#,
        )
        .bind(budget_id)
        .fetch_all(&self.pool)
        .await?;
        Ok(rows)
    }

    /// Find an account by its code. Used for auto-JE creation from cross-domain events.
    pub async fn find_account_by_code(&self, code: &str) -> AppResult<Account> {
        sqlx::query_as::<_, Account>(
            "SELECT id, code, name, account_type, parent_id, is_active, created_at FROM accounts WHERE code = ? LIMIT 1",
        )
        .bind(code)
        .fetch_optional(&self.pool)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("Account with code '{}' not found", code)))
    }

    /// Find the first active account of a given type (async).
    pub async fn find_account_by_type_async(&self, account_type: &str) -> AppResult<Account> {
        sqlx::query_as::<_, Account>(
            "SELECT id, code, name, account_type, parent_id, is_active, created_at FROM accounts WHERE account_type = ? AND is_active = 1 ORDER BY code LIMIT 1",
        )
        .bind(account_type)
        .fetch_optional(&self.pool)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("No active '{}' account found", account_type)))
    }

    /// Compute the net balance for an account within a fiscal year's periods.
    /// Revenue accounts: credits - debits. Expense accounts: debits - credits.
    pub async fn account_balance_for_fiscal_year(
        &self,
        account_id: &str,
        fiscal_year: i64,
    ) -> AppResult<i64> {
        let row: (i64,) = sqlx::query_as(
            r#"SELECT COALESCE(SUM(
                CASE
                    WHEN a.account_type IN ('asset', 'expense') THEN jl.debit_cents - jl.credit_cents
                    ELSE jl.credit_cents - jl.debit_cents
                END
            ), 0)
            FROM journal_lines jl
            JOIN journal_entries je ON je.id = jl.entry_id AND je.status = 'posted'
            JOIN periods p ON p.id = je.period_id AND p.fiscal_year = ?
            JOIN accounts a ON a.id = jl.account_id
            WHERE jl.account_id = ?"#,
        )
        .bind(fiscal_year)
        .bind(account_id)
        .fetch_one(&self.pool)
        .await?;
        Ok(row.0)
    }
}
