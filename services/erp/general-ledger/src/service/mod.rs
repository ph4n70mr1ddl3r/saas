use crate::models::*;
use crate::repository::LedgerRepo;
use saas_common::error::{AppError, AppResult};
use saas_nats_bus::NatsBus;
use sqlx::SqlitePool;

#[derive(Clone)]
pub struct LedgerService {
    repo: LedgerRepo,
    #[allow(dead_code)]
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
        self.repo.close_period(id).await
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
        self.repo.update_budget_status(id, "active").await
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
}
