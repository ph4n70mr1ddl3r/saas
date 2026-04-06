use crate::models::*;
use crate::repository::LedgerRepo;
use saas_common::error::{AppError, AppResult};
use saas_nats_bus::NatsBus;
use saas_proto::events::{JournalEntryPosted, JournalEntryReversed, PeriodClosed, BudgetActivated};
use sqlx::SqlitePool;

#[derive(Clone)]
pub struct LedgerService {
    repo: LedgerRepo,
    bus: NatsBus,
}

impl LedgerService {
    pub fn new(pool: SqlitePool, bus: NatsBus) -> Self {
        Self {
            repo: LedgerRepo::new(pool),
            bus,
        }
    }

    // --- Accounts ---

    pub async fn list_accounts(&self) -> AppResult<Vec<Account>> {
        self.repo.list_accounts().await
    }

    pub async fn get_account(&self, id: &str) -> AppResult<Account> {
        self.repo.get_account(id).await
    }

    pub async fn create_account(&self, input: &CreateAccountRequest) -> AppResult<Account> {
        let valid_types = ["asset", "liability", "equity", "revenue", "expense"];
        if !valid_types.contains(&input.account_type.as_str()) {
            return Err(AppError::Validation(format!(
                "Invalid account_type '{}'. Must be one of: {:?}",
                input.account_type, valid_types
            )));
        }
        self.repo.create_account(input).await
    }

    // --- Periods ---

    pub async fn list_periods(&self) -> AppResult<Vec<Period>> {
        self.repo.list_periods().await
    }

    pub async fn create_period(&self, input: &CreatePeriodRequest) -> AppResult<Period> {
        if input.start_date >= input.end_date {
            return Err(AppError::Validation(
                "start_date must be before end_date".into(),
            ));
        }
        self.repo.create_period(input).await
    }

    pub async fn close_period(&self, id: &str) -> AppResult<Period> {
        let period = self.repo.close_period(id).await?;
        if let Err(e) = self
            .bus
            .publish(
                "erp.gl.period.closed",
                PeriodClosed {
                    period_id: period.id.clone(),
                    name: period.name.clone(),
                    fiscal_year: period.fiscal_year as i32,
                },
            )
            .await
        {
            tracing::error!(
                "CRITICAL: Failed to publish event '{}': {}. Data may be inconsistent.",
                "erp.gl.period.closed",
                e
            );
        }
        Ok(period)
    }

    // --- Journal Entries ---

    pub async fn list_journal_entries(&self) -> AppResult<Vec<JournalEntry>> {
        self.repo.list_journal_entries().await
    }

    pub async fn get_journal_entry(&self, id: &str) -> AppResult<JournalEntryWithLines> {
        let entry = self.repo.get_journal_entry(id).await?;
        let lines = self.repo.get_journal_lines(id).await?;
        Ok(JournalEntryWithLines { entry, lines })
    }

    pub async fn create_journal_entry(
        &self,
        input: &CreateJournalEntryRequest,
        created_by: &str,
    ) -> AppResult<JournalEntryWithLines> {
        // Validate at least one line
        if input.lines.is_empty() {
            return Err(AppError::Validation(
                "At least one journal line is required".into(),
            ));
        }

        // Validate line amounts
        for line in &input.lines {
            if line.debit_cents < 0 || line.credit_cents < 0 {
                return Err(AppError::Validation(
                    "Debit and credit amounts must be non-negative".into(),
                ));
            }
            if line.debit_cents > 0 && line.credit_cents > 0 {
                return Err(AppError::Validation(
                    "A line cannot have both debit and credit amounts".into(),
                ));
            }
        }

        // Validate debits equal credits
        let total_debits: i64 = input.lines.iter().map(|l| l.debit_cents).sum();
        let total_credits: i64 = input.lines.iter().map(|l| l.credit_cents).sum();
        if total_debits != total_credits {
            return Err(AppError::Validation(format!(
                "Debits ({}) must equal credits ({})",
                total_debits, total_credits
            )));
        }

        // Validate accounts exist
        for line in &input.lines {
            self.repo.get_account(&line.account_id).await?;
        }

        // Validate period is open
        let period = self.repo.get_period(&input.period_id).await?;
        if period.status != "open" {
            return Err(AppError::Validation(
                "Can only create entries in open periods".into(),
            ));
        }

        let entry_number = self.repo.next_entry_number().await?;
        let entry = self
            .repo
            .create_journal_entry(&entry_number, input, created_by)
            .await?;
        let lines = self.repo.get_journal_lines(&entry.id).await?;
        Ok(JournalEntryWithLines { entry, lines })
    }

    pub async fn post_journal_entry(&self, id: &str) -> AppResult<JournalEntryWithLines> {
        let entry = self.repo.get_journal_entry(id).await?;
        if entry.status != "draft" {
            return Err(AppError::Validation(
                "Only draft entries can be posted".into(),
            ));
        }

        // Validate period is still open
        let period = self.repo.get_period(&entry.period_id).await?;
        if period.status != "open" {
            return Err(AppError::Validation("Can only post to open periods".into()));
        }

        // Validate debits equal credits
        let lines = self.repo.get_journal_lines(id).await?;
        let total_debits: i64 = lines.iter().map(|l| l.debit_cents).sum();
        let total_credits: i64 = lines.iter().map(|l| l.credit_cents).sum();
        if total_debits != total_credits {
            return Err(AppError::Validation(format!(
                "Debits ({}) must equal credits ({})",
                total_debits, total_credits
            )));
        }

        let entry = self.repo.post_journal_entry(id).await?;
        let lines = self.repo.get_journal_lines(id).await?;

        if let Err(e) = self
            .bus
            .publish(
                "erp.gl.journal.posted",
                JournalEntryPosted {
                    entry_id: entry.id.clone(),
                    entry_number: entry.entry_number.clone(),
                    lines: lines
                        .iter()
                        .map(|l| saas_proto::events::JournalLinePosted {
                            account_code: l.account_id.clone(),
                            debit_cents: l.debit_cents,
                            credit_cents: l.credit_cents,
                        })
                        .collect(),
                    posted_by: entry.created_by.clone(),
                },
            )
            .await
        {
            tracing::error!(
                "CRITICAL: Failed to publish event '{}': {}. Data may be inconsistent.",
                "erp.gl.journal.posted",
                e
            );
        }

        Ok(JournalEntryWithLines { entry, lines })
    }

    pub async fn reverse_journal_entry(&self, id: &str) -> AppResult<JournalEntryWithLines> {
        let entry = self.repo.get_journal_entry(id).await?;
        if entry.status != "posted" {
            return Err(AppError::Validation(
                "Only posted entries can be reversed".into(),
            ));
        }
        let entry = self.repo.reverse_journal_entry(id).await?;
        let lines = self.repo.get_journal_lines(id).await?;

        // Find the reversal entry to publish its ID
        let all_entries = self.repo.list_journal_entries().await?;
        let reversal = all_entries
            .iter()
            .find(|e| e.entry_number.starts_with("REV-") && e.status == "posted");

        if let Some(rev) = reversal {
            if let Err(e) = self
                .bus
                .publish(
                    "erp.gl.journal.reversed",
                    JournalEntryReversed {
                        entry_id: rev.id.clone(),
                        original_entry_id: id.to_string(),
                        reversed_by: "system".to_string(),
                    },
                )
                .await
            {
                tracing::error!(
                    "CRITICAL: Failed to publish event '{}': {}. Data may be inconsistent.",
                    "erp.gl.journal.reversed",
                    e
                );
            }
        }

        Ok(JournalEntryWithLines { entry, lines })
    }

    // --- Reports ---

    pub async fn trial_balance(&self) -> AppResult<Vec<TrialBalanceRow>> {
        self.repo.trial_balance().await
    }

    pub async fn balance_sheet(&self) -> AppResult<Vec<BalanceSheetRow>> {
        self.repo.balance_sheet().await
    }

    // --- Income Statement ---

    pub async fn income_statement(
        &self,
        period_start: &str,
        period_end: &str,
    ) -> AppResult<IncomeStatement> {
        let rows = self.repo.income_statement(period_start, period_end).await?;
        let mut revenue = Vec::new();
        let mut expenses = Vec::new();
        let mut total_revenue_cents: i64 = 0;
        let mut total_expense_cents: i64 = 0;

        for row in rows {
            if row.account_type == "revenue" {
                total_revenue_cents += row.balance_cents;
                revenue.push(row);
            } else {
                total_expense_cents += row.balance_cents;
                expenses.push(row);
            }
        }

        Ok(IncomeStatement {
            revenue,
            total_revenue_cents,
            expenses,
            total_expense_cents,
            net_income_cents: total_revenue_cents - total_expense_cents,
        })
    }

    // --- Budgets ---

    pub async fn create_budget(
        &self,
        input: &CreateBudgetRequest,
        created_by: &str,
    ) -> AppResult<BudgetWithLines> {
        if input.lines.is_empty() {
            return Err(AppError::Validation(
                "At least one budget line is required".into(),
            ));
        }
        for line in &input.lines {
            if line.budgeted_cents < 0 {
                return Err(AppError::Validation(
                    "Budget line amounts must be non-negative".into(),
                ));
            }
            self.repo.get_account(&line.account_id).await?;
        }
        self.repo.get_period(&input.period_id).await?;
        let budget = self.repo.create_budget(input, created_by).await?;
        let lines = self.repo.get_budget_lines(&budget.id).await?;
        Ok(BudgetWithLines { budget, lines })
    }

    pub async fn get_budget(&self, id: &str) -> AppResult<BudgetWithLines> {
        let budget = self.repo.get_budget(id).await?;
        let lines = self.repo.get_budget_lines(id).await?;
        Ok(BudgetWithLines { budget, lines })
    }

    pub async fn list_budgets(&self) -> AppResult<Vec<Budget>> {
        self.repo.list_budgets().await
    }

    pub async fn approve_budget(&self, id: &str) -> AppResult<Budget> {
        let budget = self.repo.get_budget(id).await?;
        if budget.status != "draft" {
            return Err(AppError::Validation(
                "Only draft budgets can be approved".into(),
            ));
        }
        self.repo.update_budget_status(id, "approved").await
    }

    pub async fn activate_budget(&self, id: &str) -> AppResult<Budget> {
        let budget = self.repo.get_budget(id).await?;
        if budget.status != "approved" {
            return Err(AppError::Validation(
                "Only approved budgets can be activated".into(),
            ));
        }
        let budget = self.repo.update_budget_status(id, "active").await?;
        if let Err(e) = self
            .bus
            .publish(
                "erp.gl.budget.activated",
                BudgetActivated {
                    budget_id: budget.id.clone(),
                    name: budget.name.clone(),
                    total_budget_cents: budget.total_budget_cents,
                },
            )
            .await
        {
            tracing::error!(
                "CRITICAL: Failed to publish event '{}': {}. Data may be inconsistent.",
                "erp.gl.budget.activated",
                e
            );
        }
        Ok(budget)
    }

    pub async fn close_budget(&self, id: &str) -> AppResult<Budget> {
        let budget = self.repo.get_budget(id).await?;
        if budget.status != "active" {
            return Err(AppError::Validation(
                "Only active budgets can be closed".into(),
            ));
        }
        self.repo.update_budget_status(id, "closed").await
    }

    pub async fn budget_variance(&self, id: &str) -> AppResult<BudgetVarianceReport> {
        let budget = self.repo.get_budget(id).await?;
        let lines = self.repo.budget_variance(id).await?;
        let total_budgeted_cents: i64 = lines.iter().map(|l| l.budgeted_cents).sum();
        let total_actual_cents: i64 = lines.iter().map(|l| l.actual_cents).sum();
        Ok(BudgetVarianceReport {
            budget_id: budget.id,
            budget_name: budget.name,
            period_id: budget.period_id,
            lines,
            total_budgeted_cents,
            total_actual_cents,
            total_variance_cents: total_budgeted_cents - total_actual_cents,
        })
    }

    // --- Event Handlers (cross-domain auto-JE creation) ---

    /// Create a journal entry when an AP invoice is approved.
    /// Debit expense account (from gl_account_code), credit AP control account.
    pub async fn handle_ap_invoice_approved(
        &self,
        invoice_id: &str,
        total_cents: i64,
        gl_account_code: &str,
    ) -> AppResult<JournalEntryWithLines> {
        let period = self.find_open_period().await?;
        let expense_account = match self.repo.find_account_by_code(gl_account_code).await {
            Ok(a) => a,
            Err(_) => match self.repo.find_account_by_type_async("expense").await {
                Ok(a) => a,
                Err(_) => self.repo.find_account_by_type_async("liability").await?,
            },
        };
        let ap_account = match self.repo.find_account_by_type_async("liability").await {
            Ok(a) => a,
            Err(_) => self.repo.find_account_by_type_async("expense").await?,
        };

        let input = CreateJournalEntryRequest {
            description: Some(format!("AP Invoice {} approved", invoice_id)),
            period_id: period.id,
            lines: vec![
                CreateJournalLineRequest {
                    account_id: expense_account.id.clone(),
                    debit_cents: total_cents,
                    credit_cents: 0,
                    description: Some(format!("AP invoice {}", invoice_id)),
                },
                CreateJournalLineRequest {
                    account_id: ap_account.id.clone(),
                    debit_cents: 0,
                    credit_cents: total_cents,
                    description: Some(format!("AP payable for invoice {}", invoice_id)),
                },
            ],
        };

        let result = self.create_journal_entry(&input, "system").await?;
        self.post_journal_entry(&result.entry.id).await
    }

    /// Create a journal entry when an AR invoice is created.
    /// Debit AR control account, credit revenue account.
    pub async fn handle_ar_invoice_created(
        &self,
        invoice_id: &str,
        total_cents: i64,
    ) -> AppResult<JournalEntryWithLines> {
        let period = self.find_open_period().await?;
        let ar_account = match self.repo.find_account_by_type_async("asset").await {
            Ok(a) => a,
            Err(_) => self.repo.find_account_by_type_async("revenue").await?,
        };
        let revenue_account = match self.repo.find_account_by_type_async("revenue").await {
            Ok(a) => a,
            Err(_) => self.repo.find_account_by_type_async("asset").await?,
        };

        let input = CreateJournalEntryRequest {
            description: Some(format!("AR Invoice {} created", invoice_id)),
            period_id: period.id,
            lines: vec![
                CreateJournalLineRequest {
                    account_id: ar_account.id.clone(),
                    debit_cents: total_cents,
                    credit_cents: 0,
                    description: Some(format!("AR receivable for invoice {}", invoice_id)),
                },
                CreateJournalLineRequest {
                    account_id: revenue_account.id.clone(),
                    debit_cents: 0,
                    credit_cents: total_cents,
                    description: Some(format!("Revenue from invoice {}", invoice_id)),
                },
            ],
        };

        let result = self.create_journal_entry(&input, "system").await?;
        self.post_journal_entry(&result.entry.id).await
    }

    /// Create a journal entry when a payroll run completes.
    /// Debit salary expense, credit accrued payroll (liability).
    pub async fn handle_payroll_run_completed(
        &self,
        pay_run_id: &str,
        total_net_pay_cents: i64,
    ) -> AppResult<JournalEntryWithLines> {
        let period = self.find_open_period().await?;
        let expense_account = match self.repo.find_account_by_type_async("expense").await {
            Ok(a) => a,
            Err(_) => self.repo.find_account_by_type_async("liability").await?,
        };
        let liability_account = match self.repo.find_account_by_type_async("liability").await {
            Ok(a) => a,
            Err(_) => self.repo.find_account_by_type_async("expense").await?,
        };

        let input = CreateJournalEntryRequest {
            description: Some(format!("Payroll run {} completed", pay_run_id)),
            period_id: period.id,
            lines: vec![
                CreateJournalLineRequest {
                    account_id: expense_account.id.clone(),
                    debit_cents: total_net_pay_cents,
                    credit_cents: 0,
                    description: Some(format!("Salary expense for pay run {}", pay_run_id)),
                },
                CreateJournalLineRequest {
                    account_id: liability_account.id.clone(),
                    debit_cents: 0,
                    credit_cents: total_net_pay_cents,
                    description: Some(format!("Accrued payroll for pay run {}", pay_run_id)),
                },
            ],
        };

        let result = self.create_journal_entry(&input, "system").await?;
        self.post_journal_entry(&result.entry.id).await
    }

    /// Create a journal entry when an expense report is approved.
    /// Debit expense account, credit cash/AP.
    pub async fn handle_expense_report_approved(
        &self,
        report_id: &str,
        total_cents: i64,
        gl_account_code: &str,
    ) -> AppResult<JournalEntryWithLines> {
        let period = self.find_open_period().await?;
        let expense_account = match self.repo.find_account_by_code(gl_account_code).await {
            Ok(a) => a,
            Err(_) => self.repo.find_account_by_type_async("expense").await?,
        };
        let cash_account = match self.repo.find_account_by_type_async("asset").await {
            Ok(a) => a,
            Err(_) => self.repo.find_account_by_type_async("liability").await?,
        };

        let input = CreateJournalEntryRequest {
            description: Some(format!("Expense report {} approved", report_id)),
            period_id: period.id,
            lines: vec![
                CreateJournalLineRequest {
                    account_id: expense_account.id.clone(),
                    debit_cents: total_cents,
                    credit_cents: 0,
                    description: Some(format!("Expense report {}", report_id)),
                },
                CreateJournalLineRequest {
                    account_id: cash_account.id.clone(),
                    debit_cents: 0,
                    credit_cents: total_cents,
                    description: Some(format!("Payment for expense report {}", report_id)),
                },
            ],
        };

        let result = self.create_journal_entry(&input, "system").await?;
        self.post_journal_entry(&result.entry.id).await
    }

    async fn find_open_period(&self) -> AppResult<Period> {
        self.repo
            .list_periods()
            .await?
            .into_iter()
            .find(|p| p.status == "open")
            .ok_or_else(|| AppError::Validation("No open period found for auto-JE".into()))
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
            include_str!("../../migrations/001_create_accounts.sql"),
            include_str!("../../migrations/002_create_periods.sql"),
            include_str!("../../migrations/003_create_journal_entries.sql"),
            include_str!("../../migrations/004_create_journal_lines.sql"),
            include_str!("../../migrations/005_create_budgets.sql"),
            include_str!("../../migrations/006_create_budget_lines.sql"),
        ];
        let migration_names = [
            "001_create_accounts.sql",
            "002_create_periods.sql",
            "003_create_journal_entries.sql",
            "004_create_journal_lines.sql",
            "005_create_budgets.sql",
            "006_create_budget_lines.sql",
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

    async fn setup_repo() -> LedgerRepo {
        let pool = setup().await;
        LedgerRepo::new(pool)
    }

    // Helper to create a basic account
    async fn create_test_account(repo: &LedgerRepo, code: &str, name: &str, account_type: &str) -> Account {
        repo.create_account(&CreateAccountRequest {
            code: code.into(),
            name: name.into(),
            account_type: account_type.into(),
            parent_id: None,
        })
        .await
        .unwrap()
    }

    // Helper to create a basic open period
    async fn create_test_period(repo: &LedgerRepo, name: &str) -> Period {
        repo.create_period(&CreatePeriodRequest {
            name: name.into(),
            start_date: "2025-01-01".into(),
            end_date: "2025-03-31".into(),
            fiscal_year: 2025,
        })
        .await
        .unwrap()
    }

    #[tokio::test]
    async fn test_account_crud() {
        let repo = setup_repo().await;

        // Create
        let account = create_test_account(&repo, "1000", "Cash", "asset").await;
        assert_eq!(account.code, "1000");
        assert_eq!(account.name, "Cash");
        assert_eq!(account.account_type, "asset");
        assert_eq!(account.is_active, 1);

        // List
        let accounts = repo.list_accounts().await.unwrap();
        assert_eq!(accounts.len(), 1);
        assert_eq!(accounts[0].id, account.id);

        // Get by id
        let fetched = repo.get_account(&account.id).await.unwrap();
        assert_eq!(fetched.code, "1000");
        assert_eq!(fetched.name, "Cash");

        // Not found
        let result = repo.get_account("nonexistent").await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_period_management() {
        let repo = setup_repo().await;

        // Create
        let period = create_test_period(&repo, "Q1 2025").await;
        assert_eq!(period.name, "Q1 2025");
        assert_eq!(period.status, "open");
        assert_eq!(period.fiscal_year, 2025);

        // List
        let periods = repo.list_periods().await.unwrap();
        assert_eq!(periods.len(), 1);

        // Get by id
        let fetched = repo.get_period(&period.id).await.unwrap();
        assert_eq!(fetched.status, "open");

        // Close period (no draft entries, so should succeed)
        let closed = repo.close_period(&period.id).await.unwrap();
        assert_eq!(closed.status, "closed");

        // Double close should fail
        let result = repo.close_period(&period.id).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_period_close_blocks_with_draft_entries() {
        let repo = setup_repo().await;

        let account = create_test_account(&repo, "1000", "Cash", "asset").await;
        let _expense = create_test_account(&repo, "5000", "Expenses", "expense").await;
        let period = create_test_period(&repo, "Q1 2025").await;

        // Create a draft journal entry
        let entry_number = repo.next_entry_number().await.unwrap();
        let input = CreateJournalEntryRequest {
            description: Some("Test entry".into()),
            period_id: period.id.clone(),
            lines: vec![
                CreateJournalLineRequest {
                    account_id: account.id.clone(),
                    debit_cents: 1000,
                    credit_cents: 0,
                    description: None,
                },
            ],
        };
        repo.create_journal_entry(&entry_number, &input, "tester")
            .await
            .unwrap();

        // Closing should fail because of draft entries
        let result = repo.close_period(&period.id).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_journal_entry_create_with_lines() {
        let repo = setup_repo().await;

        let cash = create_test_account(&repo, "1000", "Cash", "asset").await;
        let revenue = create_test_account(&repo, "4000", "Revenue", "revenue").await;
        let period = create_test_period(&repo, "Q1 2025").await;

        let entry_number = repo.next_entry_number().await.unwrap();
        let input = CreateJournalEntryRequest {
            description: Some("Sale".into()),
            period_id: period.id.clone(),
            lines: vec![
                CreateJournalLineRequest {
                    account_id: cash.id.clone(),
                    debit_cents: 5000,
                    credit_cents: 0,
                    description: Some("Cash received".into()),
                },
                CreateJournalLineRequest {
                    account_id: revenue.id.clone(),
                    debit_cents: 0,
                    credit_cents: 5000,
                    description: Some("Revenue earned".into()),
                },
            ],
        };

        let entry = repo
            .create_journal_entry(&entry_number, &input, "user-1")
            .await
            .unwrap();
        assert_eq!(entry.status, "draft");
        assert_eq!(entry.created_by, "user-1");
        assert_eq!(entry.period_id, period.id);

        let lines = repo.get_journal_lines(&entry.id).await.unwrap();
        assert_eq!(lines.len(), 2);
    }

    #[tokio::test]
    async fn test_post_journal_entry() {
        let repo = setup_repo().await;

        let cash = create_test_account(&repo, "1000", "Cash", "asset").await;
        let revenue = create_test_account(&repo, "4000", "Revenue", "revenue").await;
        let period = create_test_period(&repo, "Q1 2025").await;

        let entry_number = repo.next_entry_number().await.unwrap();
        let input = CreateJournalEntryRequest {
            description: Some("Sale".into()),
            period_id: period.id.clone(),
            lines: vec![
                CreateJournalLineRequest {
                    account_id: cash.id.clone(),
                    debit_cents: 3000,
                    credit_cents: 0,
                    description: None,
                },
                CreateJournalLineRequest {
                    account_id: revenue.id.clone(),
                    debit_cents: 0,
                    credit_cents: 3000,
                    description: None,
                },
            ],
        };

        let entry = repo
            .create_journal_entry(&entry_number, &input, "user-1")
            .await
            .unwrap();
        assert_eq!(entry.status, "draft");

        let posted = repo.post_journal_entry(&entry.id).await.unwrap();
        assert_eq!(posted.status, "posted");
        assert!(posted.posted_at.is_some());
    }

    #[tokio::test]
    async fn test_reverse_journal_entry_creates_counter_lines() {
        let repo = setup_repo().await;

        let cash = create_test_account(&repo, "1000", "Cash", "asset").await;
        let revenue = create_test_account(&repo, "4000", "Revenue", "revenue").await;
        let period = create_test_period(&repo, "Q1 2025").await;

        let entry_number = repo.next_entry_number().await.unwrap();
        let input = CreateJournalEntryRequest {
            description: Some("Sale".into()),
            period_id: period.id.clone(),
            lines: vec![
                CreateJournalLineRequest {
                    account_id: cash.id.clone(),
                    debit_cents: 2000,
                    credit_cents: 0,
                    description: None,
                },
                CreateJournalLineRequest {
                    account_id: revenue.id.clone(),
                    debit_cents: 0,
                    credit_cents: 2000,
                    description: None,
                },
            ],
        };

        let entry = repo
            .create_journal_entry(&entry_number, &input, "user-1")
            .await
            .unwrap();
        repo.post_journal_entry(&entry.id).await.unwrap();

        // Reverse
        let reversed = repo.reverse_journal_entry(&entry.id).await.unwrap();
        assert_eq!(reversed.status, "reversed");

        // Original lines still exist
        let original_lines = repo.get_journal_lines(&entry.id).await.unwrap();
        assert_eq!(original_lines.len(), 2);

        // Find the reversal entry
        let all_entries = repo.list_journal_entries().await.unwrap();
        let reversal_entry = all_entries
            .iter()
            .find(|e| e.entry_number.starts_with("REV-"))
            .unwrap();
        assert_eq!(reversal_entry.status, "posted");

        // Reversal lines have swapped debit/credit
        let reversal_lines = repo.get_journal_lines(&reversal_entry.id).await.unwrap();
        assert_eq!(reversal_lines.len(), 2);
        let cash_line = reversal_lines
            .iter()
            .find(|l| l.account_id == cash.id)
            .unwrap();
        assert_eq!(cash_line.credit_cents, 2000);
        assert_eq!(cash_line.debit_cents, 0);
        let revenue_line = reversal_lines
            .iter()
            .find(|l| l.account_id == revenue.id)
            .unwrap();
        assert_eq!(revenue_line.debit_cents, 2000);
        assert_eq!(revenue_line.credit_cents, 0);
    }

    #[tokio::test]
    async fn test_budget_lifecycle() {
        let repo = setup_repo().await;

        let expense = create_test_account(&repo, "5000", "Office Supplies", "expense").await;
        let period = create_test_period(&repo, "Q1 2025").await;

        // Create budget with lines
        let input = CreateBudgetRequest {
            name: "Q1 Budget".into(),
            period_id: period.id.clone(),
            lines: vec![CreateBudgetLineRequest {
                account_id: expense.id.clone(),
                budgeted_cents: 50000,
            }],
        };
        let budget = repo.create_budget(&input, "admin").await.unwrap();
        assert_eq!(budget.status, "draft");
        assert_eq!(budget.name, "Q1 Budget");
        assert_eq!(budget.total_budget_cents, 50000);

        let lines = repo.get_budget_lines(&budget.id).await.unwrap();
        assert_eq!(lines.len(), 1);
        assert_eq!(lines[0].budgeted_cents, 50000);

        // Approve
        let approved = repo.update_budget_status(&budget.id, "approved").await.unwrap();
        assert_eq!(approved.status, "approved");

        // Activate
        let active = repo.update_budget_status(&budget.id, "active").await.unwrap();
        assert_eq!(active.status, "active");

        // Close
        let closed = repo.update_budget_status(&budget.id, "closed").await.unwrap();
        assert_eq!(closed.status, "closed");

        // List budgets
        let budgets = repo.list_budgets().await.unwrap();
        assert_eq!(budgets.len(), 1);
    }

    #[tokio::test]
    async fn test_budget_variance_report() {
        let repo = setup_repo().await;

        let cash = create_test_account(&repo, "1000", "Cash", "asset").await;
        let expense = create_test_account(&repo, "5000", "Office Supplies", "expense").await;
        let period = create_test_period(&repo, "Q1 2025").await;

        // Create and post an actual expense
        let entry_number = repo.next_entry_number().await.unwrap();
        let input = CreateJournalEntryRequest {
            description: Some("Office purchase".into()),
            period_id: period.id.clone(),
            lines: vec![
                CreateJournalLineRequest {
                    account_id: expense.id.clone(),
                    debit_cents: 3000,
                    credit_cents: 0,
                    description: None,
                },
                CreateJournalLineRequest {
                    account_id: cash.id.clone(),
                    debit_cents: 0,
                    credit_cents: 3000,
                    description: None,
                },
            ],
        };
        let entry = repo
            .create_journal_entry(&entry_number, &input, "user-1")
            .await
            .unwrap();
        repo.post_journal_entry(&entry.id).await.unwrap();

        // Create budget
        let budget_input = CreateBudgetRequest {
            name: "Q1 Budget".into(),
            period_id: period.id.clone(),
            lines: vec![CreateBudgetLineRequest {
                account_id: expense.id.clone(),
                budgeted_cents: 5000,
            }],
        };
        let budget = repo.create_budget(&budget_input, "admin").await.unwrap();

        // Check variance
        let variance = repo.budget_variance(&budget.id).await.unwrap();
        assert_eq!(variance.len(), 1);
        assert_eq!(variance[0].budgeted_cents, 5000);
        assert_eq!(variance[0].actual_cents, 3000);
        assert_eq!(variance[0].variance_cents, 2000); // under budget
    }

    #[tokio::test]
    async fn test_find_account_by_code() {
        let repo = setup_repo().await;

        let cash = create_test_account(&repo, "1000", "Cash", "asset").await;

        // Find by code
        let found = repo.find_account_by_code("1000").await.unwrap();
        assert_eq!(found.id, cash.id);
        assert_eq!(found.name, "Cash");

        // Not found
        let result = repo.find_account_by_code("9999").await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_find_account_by_type_async() {
        let repo = setup_repo().await;

        let _cash = create_test_account(&repo, "1000", "Cash", "asset").await;
        let _revenue = create_test_account(&repo, "4000", "Revenue", "revenue").await;

        // Find by type
        let found_asset = repo.find_account_by_type_async("asset").await.unwrap();
        assert_eq!(found_asset.account_type, "asset");
        assert_eq!(found_asset.code, "1000");

        let found_revenue = repo.find_account_by_type_async("revenue").await.unwrap();
        assert_eq!(found_revenue.account_type, "revenue");

        // Not found
        let result = repo.find_account_by_type_async("nonexistent").await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_next_entry_number() {
        let repo = setup_repo().await;

        let num1 = repo.next_entry_number().await.unwrap();
        assert!(num1.starts_with("JE-"));

        let num2 = repo.next_entry_number().await.unwrap();
        assert!(num2.starts_with("JE-"));
        assert_ne!(num1, num2, "Entry numbers should be unique");
    }
}
