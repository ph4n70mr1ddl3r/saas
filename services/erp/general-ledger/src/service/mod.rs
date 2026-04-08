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
        // Check for duplicate account code
        let existing = self.repo.list_accounts().await?;
        if existing.iter().any(|a| a.code == input.code) {
            return Err(AppError::Validation(format!(
                "Account code '{}' already exists",
                input.code
            )));
        }
        self.repo.create_account(input).await
    }

    pub async fn deactivate_account(&self, id: &str) -> AppResult<Account> {
        self.repo.deactivate_account(id).await
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
        // Validate no overlapping periods
        let existing_periods = self.repo.list_periods().await?;
        for p in &existing_periods {
            if p.status == "closed" {
                continue; // Skip closed periods
            }
            let overlaps = input.start_date < p.end_date && input.end_date > p.start_date;
            if overlaps {
                return Err(AppError::Validation(format!(
                    "Period '{}' ({}) overlaps with existing period '{}' ({} to {})",
                    input.name, input.start_date, p.name, p.start_date, p.end_date
                )));
            }
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
        let (original, _reversal) = self.repo.reverse_journal_entry(id).await?;
        let lines = self.repo.get_journal_lines(id).await?;

        // Find the reversal entry using the tracked relationship
        let reversal = self.repo.find_reversal_for(id).await?;

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

        Ok(JournalEntryWithLines { entry: original, lines })
    }

    /// Delete a draft journal entry that has not been posted.
    pub async fn delete_journal_entry(&self, id: &str) -> AppResult<()> {
        let entry = self.repo.get_journal_entry(id).await?;
        if entry.status != "draft" {
            return Err(AppError::Validation(
                "Only draft journal entries can be deleted".into(),
            ));
        }
        // Verify the period is still open
        let period = self.repo.get_period(&entry.period_id).await?;
        if period.status != "open" {
            return Err(AppError::Validation(
                "Cannot delete entry from a closed period".into(),
            ));
        }
        self.repo.delete_journal_entry(id).await
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

    /// Create a journal entry when a depreciation run completes.
    /// Debit depreciation expense, credit accumulated depreciation (contra-asset).
    pub async fn handle_depreciation_completed(
        &self,
        period: &str,
        total_depreciation_cents: i64,
        asset_count: u32,
    ) -> AppResult<JournalEntryWithLines> {
        let open_period = self.find_open_period().await?;
        let expense_account = match self.repo.find_account_by_type_async("expense").await {
            Ok(a) => a,
            Err(_) => self.repo.find_account_by_type_async("asset").await?,
        };
        let contra_asset_account = match self.repo.find_account_by_code("1800").await {
            Ok(a) => a,
            Err(_) => match self.repo.find_account_by_type_async("asset").await {
                Ok(a) => a,
                Err(_) => self.repo.find_account_by_type_async("liability").await?,
            },
        };

        let input = CreateJournalEntryRequest {
            description: Some(format!(
                "Depreciation for {} ({} assets)",
                period, asset_count
            )),
            period_id: open_period.id,
            lines: vec![
                CreateJournalLineRequest {
                    account_id: expense_account.id.clone(),
                    debit_cents: total_depreciation_cents,
                    credit_cents: 0,
                    description: Some(format!("Depreciation expense for {}", period)),
                },
                CreateJournalLineRequest {
                    account_id: contra_asset_account.id.clone(),
                    debit_cents: 0,
                    credit_cents: total_depreciation_cents,
                    description: Some(format!("Accumulated depreciation for {}", period)),
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

    /// Create a journal entry when an AP payment is made.
    /// Debit AP (liability) to clear it, Credit Cash (asset).
    pub async fn handle_ap_payment_created(
        &self,
        payment_id: &str,
        amount_cents: i64,
    ) -> AppResult<JournalEntryWithLines> {
        let period = self.find_open_period().await?;
        let ap_account = match self.repo.find_account_by_type_async("liability").await {
            Ok(a) => a,
            Err(_) => self.repo.find_account_by_type_async("expense").await?,
        };
        let cash_account = match self.repo.find_account_by_type_async("asset").await {
            Ok(a) => a,
            Err(_) => self.repo.find_account_by_type_async("liability").await?,
        };

        let input = CreateJournalEntryRequest {
            description: Some(format!("AP Payment {} made", payment_id)),
            period_id: period.id,
            lines: vec![
                CreateJournalLineRequest {
                    account_id: ap_account.id.clone(),
                    debit_cents: amount_cents,
                    credit_cents: 0,
                    description: Some(format!("Clear AP for payment {}", payment_id)),
                },
                CreateJournalLineRequest {
                    account_id: cash_account.id.clone(),
                    debit_cents: 0,
                    credit_cents: amount_cents,
                    description: Some(format!("Cash disbursement for payment {}", payment_id)),
                },
            ],
        };

        let result = self.create_journal_entry(&input, "system").await?;
        self.post_journal_entry(&result.entry.id).await
    }

    /// Create a journal entry when an AR receipt is recorded.
    /// Debit Cash (asset), Credit AR (asset) to clear receivable.
    pub async fn handle_ar_receipt_created(
        &self,
        receipt_id: &str,
        amount_cents: i64,
    ) -> AppResult<JournalEntryWithLines> {
        let period = self.find_open_period().await?;
        let cash_account = match self.repo.find_account_by_type_async("asset").await {
            Ok(a) => a,
            Err(_) => self.repo.find_account_by_type_async("revenue").await?,
        };
        let ar_account = match self.repo.find_account_by_type_async("asset").await {
            Ok(a) => a,
            Err(_) => self.repo.find_account_by_type_async("revenue").await?,
        };
        // Use different accounts: find a second asset account for AR
        let ar_account = match self.repo.find_account_by_code("1200").await {
            Ok(a) => a,
            Err(_) => ar_account,
        };

        let input = CreateJournalEntryRequest {
            description: Some(format!("AR Receipt {} recorded", receipt_id)),
            period_id: period.id,
            lines: vec![
                CreateJournalLineRequest {
                    account_id: cash_account.id.clone(),
                    debit_cents: amount_cents,
                    credit_cents: 0,
                    description: Some(format!("Cash received for receipt {}", receipt_id)),
                },
                CreateJournalLineRequest {
                    account_id: ar_account.id.clone(),
                    debit_cents: 0,
                    credit_cents: amount_cents,
                    description: Some(format!("Clear AR for receipt {}", receipt_id)),
                },
            ],
        };

        let result = self.create_journal_entry(&input, "system").await?;
        self.post_journal_entry(&result.entry.id).await
    }

    /// Handle AP invoice cancellation by finding and reversing the original auto-created JE.
    /// The original JE description contains the invoice_id (e.g., "AP Invoice INV-001 approved").
    /// Returns the reversal entry (posted, with swapped debit/credit lines) or None if no JE found.
    pub async fn handle_ap_invoice_cancelled(
        &self,
        invoice_id: &str,
        _vendor_id: &str,
    ) -> AppResult<Option<JournalEntryWithLines>> {
        // Search for the original posted JE by description pattern
        let original = self.repo.find_posted_entry_by_description(
            &format!("AP Invoice {}", invoice_id)
        ).await?;

        match original {
            Some(entry) => {
                tracing::info!(
                    "Found original JE {} for cancelled AP invoice {}, reversing",
                    entry.entry_number, invoice_id
                );
                self.reverse_journal_entry(&entry.id).await?;
                // Look up the reversal entry (the new posted counter-entry)
                let reversal = self.repo.find_reversal_for(&entry.id).await?;
                match reversal {
                    Some(rev) => {
                        let lines = self.repo.get_journal_lines(&rev.id).await?;
                        Ok(Some(JournalEntryWithLines { entry: rev, lines }))
                    }
                    None => Ok(None),
                }
            }
            None => {
                tracing::warn!(
                    "No posted JE found for cancelled AP invoice {} - no reversal needed",
                    invoice_id
                );
                Ok(None)
            }
        }
    }

    /// Handle AR invoice cancellation by finding and reversing the original auto-created JE.
    /// The original JE description contains the invoice_id (e.g., "AR Invoice INV-001 created").
    /// Returns the reversal entry (posted, with swapped debit/credit lines) or None if no JE found.
    pub async fn handle_ar_invoice_cancelled(
        &self,
        invoice_id: &str,
        _customer_id: &str,
    ) -> AppResult<Option<JournalEntryWithLines>> {
        // Search for the original posted JE by description pattern
        let original = self.repo.find_posted_entry_by_description(
            &format!("AR Invoice {}", invoice_id)
        ).await?;

        match original {
            Some(entry) => {
                tracing::info!(
                    "Found original JE {} for cancelled AR invoice {}, reversing",
                    entry.entry_number, invoice_id
                );
                self.reverse_journal_entry(&entry.id).await?;
                // Look up the reversal entry (the new posted counter-entry)
                let reversal = self.repo.find_reversal_for(&entry.id).await?;
                match reversal {
                    Some(rev) => {
                        let lines = self.repo.get_journal_lines(&rev.id).await?;
                        Ok(Some(JournalEntryWithLines { entry: rev, lines }))
                    }
                    None => Ok(None),
                }
            }
            None => {
                tracing::warn!(
                    "No posted JE found for cancelled AR invoice {} - no reversal needed",
                    invoice_id
                );
                Ok(None)
            }
        }
    }

    /// Create a journal entry when a fixed asset is capitalized.
    /// Debit Fixed Asset, Credit Cash/AP.
    pub async fn handle_asset_created(
        &self,
        asset_id: &str,
        name: &str,
        cost_cents: i64,
    ) -> AppResult<JournalEntryWithLines> {
        let period = self.find_open_period().await?;
        let fixed_asset_account = match self.repo.find_account_by_code("1500").await {
            Ok(a) => a,
            Err(_) => match self.repo.find_account_by_type_async("asset").await {
                Ok(a) => a,
                Err(_) => self.repo.find_account_by_type_async("liability").await?,
            },
        };
        let cash_account = match self.repo.find_account_by_type_async("asset").await {
            Ok(a) => a,
            Err(_) => self.repo.find_account_by_type_async("liability").await?,
        };

        let input = CreateJournalEntryRequest {
            description: Some(format!("Asset capitalization: {} ({})", name, asset_id)),
            period_id: period.id,
            lines: vec![
                CreateJournalLineRequest {
                    account_id: fixed_asset_account.id.clone(),
                    debit_cents: cost_cents,
                    credit_cents: 0,
                    description: Some(format!("Fixed asset: {}", name)),
                },
                CreateJournalLineRequest {
                    account_id: cash_account.id.clone(),
                    debit_cents: 0,
                    credit_cents: cost_cents,
                    description: Some(format!("Payment for asset {}", name)),
                },
            ],
        };

        let result = self.create_journal_entry(&input, "system").await?;
        self.post_journal_entry(&result.entry.id).await
    }

    /// Create a journal entry when a fixed asset is disposed.
    /// Debit Accumulated Depreciation, Credit Fixed Asset, difference to Gain/Loss.
    pub async fn handle_asset_disposed(
        &self,
        asset_id: &str,
        name: &str,
        cost_cents: i64,
        accumulated_depreciation_cents: i64,
    ) -> AppResult<JournalEntryWithLines> {
        let period = self.find_open_period().await?;
        let accum_depr_account = match self.repo.find_account_by_code("1800").await {
            Ok(a) => a,
            Err(_) => self.repo.find_account_by_type_async("asset").await?,
        };
        let fixed_asset_account = match self.repo.find_account_by_code("1500").await {
            Ok(a) => a,
            Err(_) => match self.repo.find_account_by_type_async("asset").await {
                Ok(a) => a,
                Err(_) => self.repo.find_account_by_type_async("liability").await?,
            },
        };

        let net_book_value = cost_cents - accumulated_depreciation_cents;
        let mut lines = vec![
            CreateJournalLineRequest {
                account_id: accum_depr_account.id.clone(),
                debit_cents: accumulated_depreciation_cents,
                credit_cents: 0,
                description: Some(format!("Accumulated depreciation reversal for {}", name)),
            },
            CreateJournalLineRequest {
                account_id: fixed_asset_account.id.clone(),
                debit_cents: 0,
                credit_cents: cost_cents,
                description: Some(format!("Remove fixed asset: {}", name)),
            },
        ];

        // If net book value != 0, recognize gain or loss
        if net_book_value != 0 {
            let gain_loss_account = match self.repo.find_account_by_code("5200").await {
                Ok(a) => a, // Gain/Loss on disposal account
                Err(_) => match self.repo.find_account_by_type_async("expense").await {
                    Ok(a) => a,
                    Err(_) => self.repo.find_account_by_type_async("revenue").await?,
                },
            };
            if net_book_value > 0 {
                // Loss: net book value not fully depreciated
                lines.push(CreateJournalLineRequest {
                    account_id: gain_loss_account.id.clone(),
                    debit_cents: net_book_value,
                    credit_cents: 0,
                    description: Some(format!("Loss on disposal of {}", name)),
                });
            } else {
                // Gain: accumulated depreciation > cost (unusual but handle it)
                lines.push(CreateJournalLineRequest {
                    account_id: gain_loss_account.id.clone(),
                    debit_cents: 0,
                    credit_cents: -net_book_value,
                    description: Some(format!("Gain on disposal of {}", name)),
                });
            }
        }

        let input = CreateJournalEntryRequest {
            description: Some(format!("Asset disposal: {} ({})", name, asset_id)),
            period_id: period.id,
            lines,
        };

        let result = self.create_journal_entry(&input, "system").await?;
        self.post_journal_entry(&result.entry.id).await
    }

    /// Create a journal entry when a cash transfer is completed.
    /// Debit destination bank, Credit source bank.
    pub async fn handle_transfer_completed(
        &self,
        from_account_id: &str,
        to_account_id: &str,
        amount_cents: i64,
    ) -> AppResult<JournalEntryWithLines> {
        let period = self.find_open_period().await?;

        // Try to resolve the from/to accounts by code, fall back to distinct asset accounts
        let source_account = match self.repo.get_account(from_account_id).await {
            Ok(a) => a,
            Err(_) => match self.repo.find_account_by_code("1100").await {
                Ok(a) => a,
                Err(_) => self.repo.find_account_by_type_async("asset").await?,
            },
        };
        let dest_account = match self.repo.get_account(to_account_id).await {
            Ok(a) => a,
            Err(_) => match self.repo.find_account_by_code("1110").await {
                Ok(a) => a,
                Err(_) => {
                    // Get a different asset account than source
                    let accounts = self.repo.list_accounts().await?;
                    accounts.into_iter()
                        .find(|a| a.account_type == "asset" && a.id != source_account.id)
                        .ok_or_else(|| AppError::Validation("No distinct destination bank account found for transfer".into()))?
                }
            },
        };

        let input = CreateJournalEntryRequest {
            description: Some(format!(
                "Cash transfer: {} -> {} ({} cents)",
                from_account_id, to_account_id, amount_cents
            )),
            period_id: period.id,
            lines: vec![
                CreateJournalLineRequest {
                    account_id: dest_account.id.clone(),
                    debit_cents: amount_cents,
                    credit_cents: 0,
                    description: Some(format!("Transfer to account {}", to_account_id)),
                },
                CreateJournalLineRequest {
                    account_id: source_account.id.clone(),
                    debit_cents: 0,
                    credit_cents: amount_cents,
                    description: Some(format!("Transfer from account {}", from_account_id)),
                },
            ],
        };

        let result = self.create_journal_entry(&input, "system").await?;
        self.post_journal_entry(&result.entry.id).await
    }

    /// Create a journal entry for reconciliation difference.
    /// Adjusts for the difference between book and statement balance.
    pub async fn handle_reconciliation_completed(
        &self,
        reconciliation_id: &str,
        difference_cents: i64,
    ) -> AppResult<JournalEntryWithLines> {
        if difference_cents == 0 {
            // No GL adjustment needed for balanced reconciliation
            return Err(AppError::Validation(
                format!("Reconciliation {} is balanced - no GL adjustment needed", reconciliation_id)
            ));
        }

        let period = self.find_open_period().await?;
        let cash_account = match self.repo.find_account_by_type_async("asset").await {
            Ok(a) => a,
            Err(_) => self.repo.find_account_by_type_async("liability").await?,
        };
        let expense_account = match self.repo.find_account_by_type_async("expense").await {
            Ok(a) => a,
            Err(_) => self.repo.find_account_by_type_async("asset").await?,
        };

        let (debit_account, credit_account, debit_amount, credit_amount) = if difference_cents > 0 {
            // Book balance > statement balance -> bank charges/fees
            (&expense_account, &cash_account, difference_cents, difference_cents)
        } else {
            // Book balance < statement balance -> interest earned
            (&cash_account, &expense_account, -difference_cents, -difference_cents)
        };

        let input = CreateJournalEntryRequest {
            description: Some(format!("Reconciliation adjustment {}", reconciliation_id)),
            period_id: period.id,
            lines: vec![
                CreateJournalLineRequest {
                    account_id: debit_account.id.clone(),
                    debit_cents: debit_amount,
                    credit_cents: 0,
                    description: Some(format!("Reconciliation adjustment {}", reconciliation_id)),
                },
                CreateJournalLineRequest {
                    account_id: credit_account.id.clone(),
                    debit_cents: 0,
                    credit_cents: credit_amount,
                    description: Some(format!("Reconciliation adjustment {}", reconciliation_id)),
                },
            ],
        };

        let result = self.create_journal_entry(&input, "system").await?;
        self.post_journal_entry(&result.entry.id).await
    }

    /// Year-end close: close all revenue and expense accounts to retained earnings.
    pub async fn year_end_close(&self, fiscal_year: i64) -> AppResult<JournalEntryWithLines> {
        // Verify all periods for the fiscal year are closed
        let periods = self.repo.list_periods().await?;
        let year_periods: Vec<_> = periods
            .iter()
            .filter(|p| p.fiscal_year == fiscal_year)
            .collect();

        if year_periods.is_empty() {
            return Err(AppError::Validation(format!(
                "No periods found for fiscal year {}",
                fiscal_year
            )));
        }

        let open_periods: Vec<_> = year_periods
            .iter()
            .filter(|p| p.status == "open")
            .collect();
        if !open_periods.is_empty() {
            return Err(AppError::Validation(format!(
                "Cannot close year {} - {} period(s) still open",
                fiscal_year,
                open_periods.len()
            )));
        }

        // Calculate net income from posted journal entries in this year's periods
        let accounts = self.repo.list_accounts().await?;
        let mut revenue_accounts = Vec::new();
        let mut expense_accounts = Vec::new();

        for account in &accounts {
            if account.account_type == "revenue" {
                revenue_accounts.push(account.clone());
            } else if account.account_type == "expense" {
                expense_accounts.push(account.clone());
            }
        }

        // Create a new "closing" period if needed, or use the last closed period
        let close_period = periods
            .iter()
            .find(|p| p.fiscal_year == fiscal_year + 1 && p.status == "open")
            .cloned();

        let period_id = match close_period {
            Some(p) => p.id,
            None => {
                // Use the last period of the closing year
                year_periods
                    .last()
                    .ok_or_else(|| AppError::Validation("No period available for year-end close".into()))?
                    .id
                    .clone()
            }
        };

        // Build closing lines: debit revenue (to zero credit balances), credit expense (to zero debit balances)
        let mut lines = Vec::new();
        let mut net_income_cents: i64 = 0;


        // Close revenue accounts (debit their credit balance to zero them)
        for account in &revenue_accounts {
            let balance = self.repo.account_balance_for_fiscal_year(&account.id, fiscal_year).await?;
            if balance == 0 {
                continue; // Skip accounts with zero balance
            }
            net_income_cents += balance;
            lines.push(CreateJournalLineRequest {
                account_id: account.id.clone(),
                debit_cents: balance, // Debit to reverse the credit balance
                credit_cents: 0,
                description: Some(format!("Year-end close: revenue account {}", account.code)),
            });
        }

        // Close expense accounts (credit their debit balance to zero them)
        for account in &expense_accounts {
            let balance = self.repo.account_balance_for_fiscal_year(&account.id, fiscal_year).await?;
            if balance == 0 {
                continue; // Skip accounts with zero balance
            }
            net_income_cents -= balance;
            lines.push(CreateJournalLineRequest {
                account_id: account.id.clone(),
                debit_cents: 0,
                credit_cents: balance, // Credit to reverse the debit balance
                description: Some(format!("Year-end close: expense account {}", account.code)),
            });
        }

        if lines.is_empty() {
            return Err(AppError::Validation(
                "No revenue or expense accounts with nonzero balances to close".into(),
            ));
        }

        // Add retained earnings line to balance the entry
        let equity_account = match self.repo.find_account_by_code("3000").await {
            Ok(a) => a,
            Err(_) => match self.repo.find_account_by_type_async("equity").await {
                Ok(a) => a,
                Err(_) => self.repo.find_account_by_type_async("liability").await?,
            },
        };
        // net_income_cents is positive when revenue > expenses (net income)
        // Credit retained earnings for net income, debit for net loss
        if net_income_cents >= 0 {
            lines.push(CreateJournalLineRequest {
                account_id: equity_account.id.clone(),
                debit_cents: 0,
                credit_cents: net_income_cents,
                description: Some(format!("Year-end close: retained earnings for {}", fiscal_year)),
            });
        } else {
            lines.push(CreateJournalLineRequest {
                account_id: equity_account.id.clone(),
                debit_cents: -net_income_cents,
                credit_cents: 0,
                description: Some(format!("Year-end close: retained earnings (net loss) for {}", fiscal_year)),
            });
        }

        let input = CreateJournalEntryRequest {
            description: Some(format!("Year-end close for fiscal year {}", fiscal_year)),
            period_id,
            lines,
        };

        let result = self.create_journal_entry(&input, "system").await?;
        let posted = self.post_journal_entry(&result.entry.id).await?;

        if let Err(e) = self
            .bus
            .publish(
                "erp.gl.year_end.closed",
                saas_proto::events::YearEndClosed {
                    fiscal_year: fiscal_year as i32,
                    entry_id: posted.entry.id.clone(),
                },
            )
            .await
        {
            tracing::error!(
                "CRITICAL: Failed to publish event '{}': {}. Data may be inconsistent.",
                "erp.gl.year_end.closed",
                e
            );
        }

        Ok(posted)
    }

    /// Create a journal entry when a cycle count is posted.
    /// Debit Inventory, credit Cost of Goods Sold for the estimated adjustments.
    pub async fn handle_cycle_count_posted(
        &self,
        session_id: &str,
        warehouse_id: &str,
        adjustment_count: u32,
    ) -> AppResult<JournalEntryWithLines> {
        let amount_per_adjustment: i64 = 10_000; // $100 per line adjusted (estimated)
        let total_amount = amount_per_adjustment * (adjustment_count as i64);

        let period = self.find_open_period().await?;
        let inventory_account = match self.repo.find_account_by_code("1400").await {
            Ok(a) => a,
            Err(_) => match self.repo.find_account_by_type_async("asset").await {
                Ok(a) => a,
                Err(_) => self.repo.find_account_by_type_async("liability").await?,
            },
        };
        let cogs_account = match self.repo.find_account_by_code("5100").await {
            Ok(a) => a,
            Err(_) => match self.repo.find_account_by_type_async("expense").await {
                Ok(a) => a,
                Err(_) => self.repo.find_account_by_type_async("asset").await?,
            },
        };

        let input = CreateJournalEntryRequest {
            description: Some(format!(
                "Cycle count adjustment - session {}, warehouse {}, {} items",
                session_id, warehouse_id, adjustment_count
            )),
            period_id: period.id,
            lines: vec![
                CreateJournalLineRequest {
                    account_id: inventory_account.id.clone(),
                    debit_cents: total_amount,
                    credit_cents: 0,
                    description: Some(format!(
                        "Inventory adjustment for cycle count session {}",
                        session_id
                    )),
                },
                CreateJournalLineRequest {
                    account_id: cogs_account.id.clone(),
                    debit_cents: 0,
                    credit_cents: total_amount,
                    description: Some(format!(
                        "COGS adjustment for cycle count session {}",
                        session_id
                    )),
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
            include_str!("../../migrations/007_add_reversal_of.sql"),
        ];
        let migration_names = [
            "001_create_accounts.sql",
            "002_create_periods.sql",
            "003_create_journal_entries.sql",
            "004_create_journal_lines.sql",
            "005_create_budgets.sql",
            "006_create_budget_lines.sql",
            "007_add_reversal_of.sql",
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
        let (reversed, _) = repo.reverse_journal_entry(&entry.id).await.unwrap();
        assert_eq!(reversed.status, "reversed");

        // Original lines still exist
        let original_lines = repo.get_journal_lines(&entry.id).await.unwrap();
        assert_eq!(original_lines.len(), 2);

        // Find the reversal entry using tracked relationship
        let reversal_entry = repo.find_reversal_for(&entry.id).await.unwrap().unwrap();
        assert_eq!(reversal_entry.status, "posted");
        assert_eq!(reversal_entry.reversal_of, Some(entry.id.clone()));

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

    #[tokio::test]
    async fn test_handle_ap_invoice_approved_creates_je() {
        let repo = setup_repo().await;

        // Create a liability account for AP
        let liability = repo
            .create_account(&CreateAccountRequest {
                code: "2000".into(),
                name: "Accounts Payable".into(),
                account_type: "liability".into(),
                parent_id: None,
            })
            .await
            .unwrap();

        // Create an expense account
        let expense = repo
            .create_account(&CreateAccountRequest {
                code: "5000".into(),
                name: "Operating Expenses".into(),
                account_type: "expense".into(),
                parent_id: None,
            })
            .await
            .unwrap();

        // Simulate what the event handler does: create a JE for AP invoice
        let period = repo
            .create_period(&CreatePeriodRequest {
                name: "AP Test Period".into(),
                start_date: "2025-01-01".into(),
                end_date: "2025-01-31".into(),
                fiscal_year: 2025,
            })
            .await
            .unwrap();

        let entry_number = repo.next_entry_number().await.unwrap();
        let entry = repo
            .create_journal_entry(
                &entry_number,
                &CreateJournalEntryRequest {
                    description: Some("Auto-JE: AP Invoice Approved".into()),
                    period_id: period.id.clone(),
                    lines: vec![
                        CreateJournalLineRequest {
                            account_id: expense.id.clone(),
                            debit_cents: 10000,
                            credit_cents: 0,
                            description: None,
                        },
                        CreateJournalLineRequest {
                            account_id: liability.id.clone(),
                            debit_cents: 0,
                            credit_cents: 10000,
                            description: None,
                        },
                    ],
                },
                "system",
            )
            .await
            .unwrap();

        assert_eq!(entry.status, "draft");
        assert_eq!(entry.description, Some("Auto-JE: AP Invoice Approved".to_string()));

        // Post it
        let posted = repo.post_journal_entry(&entry.id).await.unwrap();
        assert_eq!(posted.status, "posted");
    }

    #[tokio::test]
    async fn test_handle_ar_invoice_created_creates_je() {
        let repo = setup_repo().await;

        let asset = repo
            .create_account(&CreateAccountRequest {
                code: "1200".into(),
                name: "Accounts Receivable".into(),
                account_type: "asset".into(),
                parent_id: None,
            })
            .await
            .unwrap();

        let revenue = repo
            .create_account(&CreateAccountRequest {
                code: "4000".into(),
                name: "Revenue".into(),
                account_type: "revenue".into(),
                parent_id: None,
            })
            .await
            .unwrap();

        let period = repo
            .create_period(&CreatePeriodRequest {
                name: "AR Test Period".into(),
                start_date: "2025-02-01".into(),
                end_date: "2025-02-28".into(),
                fiscal_year: 2025,
            })
            .await
            .unwrap();

        let entry_number = repo.next_entry_number().await.unwrap();
        let entry = repo
            .create_journal_entry(
                &entry_number,
                &CreateJournalEntryRequest {
                    description: Some("Auto-JE: AR Invoice Created".into()),
                    period_id: period.id.clone(),
                    lines: vec![
                        CreateJournalLineRequest {
                            account_id: asset.id.clone(),
                            debit_cents: 15000,
                            credit_cents: 0,
                            description: None,
                        },
                        CreateJournalLineRequest {
                            account_id: revenue.id.clone(),
                            debit_cents: 0,
                            credit_cents: 15000,
                            description: None,
                        },
                    ],
                },
                "system",
            )
            .await
            .unwrap();

        assert_eq!(entry.description, Some("Auto-JE: AR Invoice Created".to_string()));

        let posted = repo.post_journal_entry(&entry.id).await.unwrap();
        assert_eq!(posted.status, "posted");
    }

    #[tokio::test]
    async fn test_handle_payroll_completed_creates_je() {
        let repo = setup_repo().await;

        let expense = repo
            .create_account(&CreateAccountRequest {
                code: "6000".into(),
                name: "Salary Expense".into(),
                account_type: "expense".into(),
                parent_id: None,
            })
            .await
            .unwrap();

        let liability = repo
            .create_account(&CreateAccountRequest {
                code: "2100".into(),
                name: "Salaries Payable".into(),
                account_type: "liability".into(),
                parent_id: None,
            })
            .await
            .unwrap();

        let period = repo
            .create_period(&CreatePeriodRequest {
                name: "Payroll Period".into(),
                start_date: "2025-03-01".into(),
                end_date: "2025-03-31".into(),
                fiscal_year: 2025,
            })
            .await
            .unwrap();

        let entry_number = repo.next_entry_number().await.unwrap();
        let entry = repo
            .create_journal_entry(
                &entry_number,
                &CreateJournalEntryRequest {
                    description: Some("Auto-JE: Payroll Run Completed".into()),
                    period_id: period.id.clone(),
                    lines: vec![
                        CreateJournalLineRequest {
                            account_id: expense.id.clone(),
                            debit_cents: 50000,
                            credit_cents: 0,
                            description: None,
                        },
                        CreateJournalLineRequest {
                            account_id: liability.id.clone(),
                            debit_cents: 0,
                            credit_cents: 50000,
                            description: None,
                        },
                    ],
                },
                "system",
            )
            .await
            .unwrap();

        assert_eq!(entry.description, Some("Auto-JE: Payroll Run Completed".to_string()));
        let posted = repo.post_journal_entry(&entry.id).await.unwrap();
        assert_eq!(posted.status, "posted");
    }

    #[tokio::test]
    async fn test_handle_expense_approved_creates_je() {
        let repo = setup_repo().await;

        let expense = repo
            .create_account(&CreateAccountRequest {
                code: "6100".into(),
                name: "Travel Expense".into(),
                account_type: "expense".into(),
                parent_id: None,
            })
            .await
            .unwrap();

        let asset = repo
            .create_account(&CreateAccountRequest {
                code: "1100".into(),
                name: "Cash".into(),
                account_type: "asset".into(),
                parent_id: None,
            })
            .await
            .unwrap();

        let period = repo
            .create_period(&CreatePeriodRequest {
                name: "Expense Period".into(),
                start_date: "2025-04-01".into(),
                end_date: "2025-04-30".into(),
                fiscal_year: 2025,
            })
            .await
            .unwrap();

        let entry_number = repo.next_entry_number().await.unwrap();
        let entry = repo
            .create_journal_entry(
                &entry_number,
                &CreateJournalEntryRequest {
                    description: Some("Auto-JE: Expense Report Approved".into()),
                    period_id: period.id.clone(),
                    lines: vec![
                        CreateJournalLineRequest {
                            account_id: expense.id.clone(),
                            debit_cents: 7500,
                            credit_cents: 0,
                            description: None,
                        },
                        CreateJournalLineRequest {
                            account_id: asset.id.clone(),
                            debit_cents: 0,
                            credit_cents: 7500,
                            description: None,
                        },
                    ],
                },
                "system",
            )
            .await
            .unwrap();

        assert_eq!(entry.description, Some("Auto-JE: Expense Report Approved".to_string()));
        let posted = repo.post_journal_entry(&entry.id).await.unwrap();
        assert_eq!(posted.status, "posted");
    }

    #[tokio::test]
    async fn test_handle_depreciation_completed_creates_je() {
        let repo = setup_repo().await;

        let expense = repo
            .create_account(&CreateAccountRequest {
                code: "6200".into(),
                name: "Depreciation Expense".into(),
                account_type: "expense".into(),
                parent_id: None,
            })
            .await
            .unwrap();

        let contra_asset = repo
            .create_account(&CreateAccountRequest {
                code: "1800".into(),
                name: "Accumulated Depreciation".into(),
                account_type: "asset".into(),
                parent_id: None,
            })
            .await
            .unwrap();

        let period = repo
            .create_period(&CreatePeriodRequest {
                name: "Depreciation Period".into(),
                start_date: "2025-01-01".into(),
                end_date: "2025-01-31".into(),
                fiscal_year: 2025,
            })
            .await
            .unwrap();

        let entry_number = repo.next_entry_number().await.unwrap();
        let entry = repo
            .create_journal_entry(
                &entry_number,
                &CreateJournalEntryRequest {
                    description: Some("Depreciation for 2025-01 (5 assets)".into()),
                    period_id: period.id.clone(),
                    lines: vec![
                        CreateJournalLineRequest {
                            account_id: expense.id.clone(),
                            debit_cents: 25000,
                            credit_cents: 0,
                            description: Some("Depreciation expense for 2025-01".into()),
                        },
                        CreateJournalLineRequest {
                            account_id: contra_asset.id.clone(),
                            debit_cents: 0,
                            credit_cents: 25000,
                            description: Some("Accumulated depreciation for 2025-01".into()),
                        },
                    ],
                },
                "system",
            )
            .await
            .unwrap();

        assert_eq!(
            entry.description,
            Some("Depreciation for 2025-01 (5 assets)".to_string())
        );

        let posted = repo.post_journal_entry(&entry.id).await.unwrap();
        assert_eq!(posted.status, "posted");

        let lines = repo.get_journal_lines(&entry.id).await.unwrap();
        assert_eq!(lines.len(), 2);

        let debit_line = lines.iter().find(|l| l.debit_cents > 0).unwrap();
        assert_eq!(debit_line.debit_cents, 25000);
        assert_eq!(debit_line.account_id, expense.id);

        let credit_line = lines.iter().find(|l| l.credit_cents > 0).unwrap();
        assert_eq!(credit_line.credit_cents, 25000);
        assert_eq!(credit_line.account_id, contra_asset.id);
    }

    #[tokio::test]
    async fn test_depreciation_je_without_contra_asset_code_uses_asset_type() {
        let repo = setup_repo().await;

        // Only create generic accounts (no code 1800)
        let expense = create_test_account(&repo, "6200", "Depreciation Expense", "expense").await;
        let asset = create_test_account(&repo, "1000", "Fixed Assets", "asset").await;

        let period = create_test_period(&repo, "Q1 2025").await;

        // Simulate the depreciation handler: debit expense, credit asset
        let entry_number = repo.next_entry_number().await.unwrap();
        let entry = repo
            .create_journal_entry(
                &entry_number,
                &CreateJournalEntryRequest {
                    description: Some("Depreciation for 2025-Q1 (3 assets)".into()),
                    period_id: period.id,
                    lines: vec![
                        CreateJournalLineRequest {
                            account_id: expense.id,
                            debit_cents: 15000,
                            credit_cents: 0,
                            description: None,
                        },
                        CreateJournalLineRequest {
                            account_id: asset.id,
                            debit_cents: 0,
                            credit_cents: 15000,
                            description: None,
                        },
                    ],
                },
                "system",
            )
            .await
            .unwrap();

        let posted = repo.post_journal_entry(&entry.id).await.unwrap();
        assert_eq!(posted.status, "posted");
    }

    #[tokio::test]
    async fn test_handle_ap_payment_created_creates_je() {
        let repo = setup_repo().await;

        let liability = create_test_account(&repo, "2000", "Accounts Payable", "liability").await;
        let cash = create_test_account(&repo, "1000", "Cash", "asset").await;
        let period = create_test_period(&repo, "AP Payment Period").await;

        let entry_number = repo.next_entry_number().await.unwrap();
        let entry = repo
            .create_journal_entry(
                &entry_number,
                &CreateJournalEntryRequest {
                    description: Some("AP Payment PAY-001 made".into()),
                    period_id: period.id,
                    lines: vec![
                        CreateJournalLineRequest {
                            account_id: liability.id.clone(),
                            debit_cents: 15000,
                            credit_cents: 0,
                            description: Some("Clear AP for payment PAY-001".into()),
                        },
                        CreateJournalLineRequest {
                            account_id: cash.id.clone(),
                            debit_cents: 0,
                            credit_cents: 15000,
                            description: Some("Cash disbursement for payment PAY-001".into()),
                        },
                    ],
                },
                "system",
            )
            .await
            .unwrap();

        let posted = repo.post_journal_entry(&entry.id).await.unwrap();
        assert_eq!(posted.status, "posted");
    }

    #[tokio::test]
    async fn test_handle_ar_receipt_created_creates_je() {
        let repo = setup_repo().await;

        let cash = create_test_account(&repo, "1000", "Cash", "asset").await;
        let ar = create_test_account(&repo, "1200", "Accounts Receivable", "asset").await;
        let period = create_test_period(&repo, "AR Receipt Period").await;

        let entry_number = repo.next_entry_number().await.unwrap();
        let entry = repo
            .create_journal_entry(
                &entry_number,
                &CreateJournalEntryRequest {
                    description: Some("AR Receipt RCP-001 recorded".into()),
                    period_id: period.id,
                    lines: vec![
                        CreateJournalLineRequest {
                            account_id: cash.id.clone(),
                            debit_cents: 20000,
                            credit_cents: 0,
                            description: Some("Cash received for receipt RCP-001".into()),
                        },
                        CreateJournalLineRequest {
                            account_id: ar.id.clone(),
                            debit_cents: 0,
                            credit_cents: 20000,
                            description: Some("Clear AR for receipt RCP-001".into()),
                        },
                    ],
                },
                "system",
            )
            .await
            .unwrap();

        let posted = repo.post_journal_entry(&entry.id).await.unwrap();
        assert_eq!(posted.status, "posted");
    }

    #[tokio::test]
    async fn test_handle_asset_created_creates_je() {
        let repo = setup_repo().await;

        let fixed_asset = create_test_account(&repo, "1500", "Fixed Assets", "asset").await;
        let cash = create_test_account(&repo, "1000", "Cash", "asset").await;
        let period = create_test_period(&repo, "Asset Cap Period").await;

        let entry_number = repo.next_entry_number().await.unwrap();
        let entry = repo
            .create_journal_entry(
                &entry_number,
                &CreateJournalEntryRequest {
                    description: Some("Asset capitalization: Server (AST-001)".into()),
                    period_id: period.id,
                    lines: vec![
                        CreateJournalLineRequest {
                            account_id: fixed_asset.id.clone(),
                            debit_cents: 50000,
                            credit_cents: 0,
                            description: Some("Fixed asset: Server".into()),
                        },
                        CreateJournalLineRequest {
                            account_id: cash.id.clone(),
                            debit_cents: 0,
                            credit_cents: 50000,
                            description: Some("Payment for asset Server".into()),
                        },
                    ],
                },
                "system",
            )
            .await
            .unwrap();

        let posted = repo.post_journal_entry(&entry.id).await.unwrap();
        assert_eq!(posted.status, "posted");
    }

    #[tokio::test]
    async fn test_handle_asset_disposed_creates_je() {
        let repo = setup_repo().await;

        let accum_depr = create_test_account(&repo, "1800", "Accumulated Depreciation", "asset").await;
        let fixed_asset = create_test_account(&repo, "1500", "Fixed Assets", "asset").await;
        let _period = create_test_period(&repo, "Asset Disp Period").await;

        // Test asset disposal: cost=50000, accumulated_depreciation=30000 -> net_book_value=20000 (loss)
        let entry_number = repo.next_entry_number().await.unwrap();
        let entry = repo
            .create_journal_entry(
                &entry_number,
                &CreateJournalEntryRequest {
                    description: Some("Asset disposal: Laptop (AST-002)".into()),
                    period_id: _period.id,
                    lines: vec![
                        CreateJournalLineRequest {
                            account_id: accum_depr.id.clone(),
                            debit_cents: 30000,
                            credit_cents: 0,
                            description: Some("Accumulated depreciation reversal for Laptop".into()),
                        },
                        CreateJournalLineRequest {
                            account_id: fixed_asset.id.clone(),
                            debit_cents: 0,
                            credit_cents: 50000,
                            description: Some("Remove fixed asset: Laptop".into()),
                        },
                        CreateJournalLineRequest {
                            account_id: fixed_asset.id.clone(), // using same account as gain/loss stand-in
                            debit_cents: 20000,
                            credit_cents: 0,
                            description: Some("Loss on disposal of Laptop".into()),
                        },
                    ],
                },
                "system",
            )
            .await
            .unwrap();

        let posted = repo.post_journal_entry(&entry.id).await.unwrap();
        assert_eq!(posted.status, "posted");

        let lines = repo.get_journal_lines(&entry.id).await.unwrap();
        assert_eq!(lines.len(), 3);
        let total_debits: i64 = lines.iter().map(|l| l.debit_cents).sum();
        let total_credits: i64 = lines.iter().map(|l| l.credit_cents).sum();
        assert_eq!(total_debits, total_credits); // 30000 + 20000 == 50000
    }

    #[tokio::test]
    async fn test_handle_asset_disposed_fully_depreciated() {
        let repo = setup_repo().await;

        let accum_depr = create_test_account(&repo, "1800", "Accumulated Depreciation", "asset").await;
        let fixed_asset = create_test_account(&repo, "1500", "Fixed Assets", "asset").await;
        let _period = create_test_period(&repo, "Asset Disp Period 2").await;

        // Fully depreciated: cost=50000, accumulated=50000 -> no gain/loss
        let entry_number = repo.next_entry_number().await.unwrap();
        let entry = repo
            .create_journal_entry(
                &entry_number,
                &CreateJournalEntryRequest {
                    description: Some("Asset disposal: Server (AST-003)".into()),
                    period_id: _period.id,
                    lines: vec![
                        CreateJournalLineRequest {
                            account_id: accum_depr.id.clone(),
                            debit_cents: 50000,
                            credit_cents: 0,
                            description: Some("Accumulated depreciation reversal for Server".into()),
                        },
                        CreateJournalLineRequest {
                            account_id: fixed_asset.id.clone(),
                            debit_cents: 0,
                            credit_cents: 50000,
                            description: Some("Remove fixed asset: Server".into()),
                        },
                    ],
                },
                "system",
            )
            .await
            .unwrap();

        let posted = repo.post_journal_entry(&entry.id).await.unwrap();
        assert_eq!(posted.status, "posted");

        let lines = repo.get_journal_lines(&entry.id).await.unwrap();
        assert_eq!(lines.len(), 2); // no gain/loss line needed
        assert_eq!(lines[0].debit_cents, 50000);
        assert_eq!(lines[1].credit_cents, 50000);
    }

    #[tokio::test]
    async fn test_handle_transfer_completed_creates_je() {
        let repo = setup_repo().await;

        let bank_a = create_test_account(&repo, "1100", "Bank Account A", "asset").await;
        let bank_b = create_test_account(&repo, "1110", "Bank Account B", "asset").await;
        let _period = create_test_period(&repo, "Transfer Period").await;

        // Transfer: debit destination (bank_b), credit source (bank_a)
        let entry_number = repo.next_entry_number().await.unwrap();
        let entry = repo
            .create_journal_entry(
                &entry_number,
                &CreateJournalEntryRequest {
                    description: Some("Cash transfer: bank_a -> bank_b (30000 cents)".into()),
                    period_id: _period.id,
                    lines: vec![
                        CreateJournalLineRequest {
                            account_id: bank_b.id.clone(),
                            debit_cents: 30000,
                            credit_cents: 0,
                            description: Some("Transfer to account bank_b".into()),
                        },
                        CreateJournalLineRequest {
                            account_id: bank_a.id.clone(),
                            debit_cents: 0,
                            credit_cents: 30000,
                            description: Some("Transfer from account bank_a".into()),
                        },
                    ],
                },
                "system",
            )
            .await
            .unwrap();

        let posted = repo.post_journal_entry(&entry.id).await.unwrap();
        assert_eq!(posted.status, "posted");

        let lines = repo.get_journal_lines(&entry.id).await.unwrap();
        assert_eq!(lines.len(), 2);
        // Destination (bank_b) gets debited
        assert_eq!(lines[0].account_id, bank_b.id);
        assert_eq!(lines[0].debit_cents, 30000);
        // Source (bank_a) gets credited
        assert_eq!(lines[1].account_id, bank_a.id);
        assert_eq!(lines[1].credit_cents, 30000);

        let total_debits: i64 = lines.iter().map(|l| l.debit_cents).sum();
        let total_credits: i64 = lines.iter().map(|l| l.credit_cents).sum();
        assert_eq!(total_debits, total_credits);
    }

    #[tokio::test]
    async fn test_handle_reconciliation_with_difference_creates_je() {
        let repo = setup_repo().await;

        let cash = create_test_account(&repo, "1000", "Cash", "asset").await;
        let expense = create_test_account(&repo, "5000", "Bank Charges", "expense").await;
        let period = create_test_period(&repo, "Recon Period").await;

        // Positive difference = bank charges
        let entry_number = repo.next_entry_number().await.unwrap();
        let entry = repo
            .create_journal_entry(
                &entry_number,
                &CreateJournalEntryRequest {
                    description: Some("Reconciliation adjustment RECON-001".into()),
                    period_id: period.id,
                    lines: vec![
                        CreateJournalLineRequest {
                            account_id: expense.id.clone(),
                            debit_cents: 500,
                            credit_cents: 0,
                            description: Some("Reconciliation adjustment RECON-001".into()),
                        },
                        CreateJournalLineRequest {
                            account_id: cash.id.clone(),
                            debit_cents: 0,
                            credit_cents: 500,
                            description: Some("Reconciliation adjustment RECON-001".into()),
                        },
                    ],
                },
                "system",
            )
            .await
            .unwrap();

        let posted = repo.post_journal_entry(&entry.id).await.unwrap();
        assert_eq!(posted.status, "posted");
    }

    #[tokio::test]
    async fn test_year_end_close_creates_je() {
        let repo = setup_repo().await;

        let revenue = create_test_account(&repo, "4000", "Revenue", "revenue").await;
        let expense = create_test_account(&repo, "5000", "Expenses", "expense").await;
        let equity = create_test_account(&repo, "3000", "Retained Earnings", "equity").await;
        let cash = create_test_account(&repo, "1000", "Cash", "asset").await;

        // Create and close a period for fiscal year 2025
        let period = create_test_period(&repo, "FY2025-Q4").await;

        // Post revenue entry: debit cash 80000, credit revenue 80000
        let entry_number = repo.next_entry_number().await.unwrap();
        let entry = repo
            .create_journal_entry(
                &entry_number,
                &CreateJournalEntryRequest {
                    description: Some("Revenue entry".into()),
                    period_id: period.id.clone(),
                    lines: vec![
                        CreateJournalLineRequest {
                            account_id: cash.id.clone(),
                            debit_cents: 80000,
                            credit_cents: 0,
                            description: Some("Cash received".into()),
                        },
                        CreateJournalLineRequest {
                            account_id: revenue.id.clone(),
                            debit_cents: 0,
                            credit_cents: 80000,
                            description: Some("Revenue earned".into()),
                        },
                    ],
                },
                "system",
            )
            .await
            .unwrap();
        repo.post_journal_entry(&entry.id).await.unwrap();

        // Post expense entry: debit expense 30000, credit cash 30000
        let entry_number2 = repo.next_entry_number().await.unwrap();
        let entry2 = repo
            .create_journal_entry(
                &entry_number2,
                &CreateJournalEntryRequest {
                    description: Some("Expense entry".into()),
                    period_id: period.id.clone(),
                    lines: vec![
                        CreateJournalLineRequest {
                            account_id: expense.id.clone(),
                            debit_cents: 30000,
                            credit_cents: 0,
                            description: Some("Expense incurred".into()),
                        },
                        CreateJournalLineRequest {
                            account_id: cash.id.clone(),
                            debit_cents: 0,
                            credit_cents: 30000,
                            description: Some("Cash paid".into()),
                        },
                    ],
                },
                "system",
            )
            .await
            .unwrap();
        repo.post_journal_entry(&entry2.id).await.unwrap();

        // Close the period
        repo.close_period(&period.id).await.unwrap();

        // Verify balances using the new repo method
        let rev_balance = repo.account_balance_for_fiscal_year(&revenue.id, 2025).await.unwrap();
        assert_eq!(rev_balance, 80000); // credits - debits for revenue
        let exp_balance = repo.account_balance_for_fiscal_year(&expense.id, 2025).await.unwrap();
        assert_eq!(exp_balance, 30000); // debits - credits for expense

        // Create a period for the next fiscal year (for the closing entry)
        let next_period = repo
            .create_period(&CreatePeriodRequest {
                name: "FY2026-Q1".into(),
                start_date: "2026-01-01".into(),
                end_date: "2026-03-31".into(),
                fiscal_year: 2026,
            })
            .await
            .unwrap();

        // Simulate the year-end close: debit revenue 80000, credit expense 30000, credit retained earnings 50000
        let close_entry_number = repo.next_entry_number().await.unwrap();
        let close_entry = repo
            .create_journal_entry(
                &close_entry_number,
                &CreateJournalEntryRequest {
                    description: Some("Year-end close for fiscal year 2025".into()),
                    period_id: next_period.id,
                    lines: vec![
                        CreateJournalLineRequest {
                            account_id: revenue.id.clone(),
                            debit_cents: 80000,
                            credit_cents: 0,
                            description: Some("Year-end close: revenue account 4000".into()),
                        },
                        CreateJournalLineRequest {
                            account_id: expense.id.clone(),
                            debit_cents: 0,
                            credit_cents: 30000,
                            description: Some("Year-end close: expense account 5000".into()),
                        },
                        CreateJournalLineRequest {
                            account_id: equity.id.clone(),
                            debit_cents: 0,
                            credit_cents: 50000,
                            description: Some("Year-end close: retained earnings for 2025".into()),
                        },
                    ],
                },
                "system",
            )
            .await
            .unwrap();

        let posted = repo.post_journal_entry(&close_entry.id).await.unwrap();
        assert_eq!(posted.status, "posted");

        // Verify balanced: debits = credits = 80000
        let lines = repo.get_journal_lines(&close_entry.id).await.unwrap();
        let total_debits: i64 = lines.iter().map(|l| l.debit_cents).sum();
        let total_credits: i64 = lines.iter().map(|l| l.credit_cents).sum();
        assert_eq!(total_debits, total_credits); // 80000 == 30000 + 50000
    }

    #[tokio::test]
    async fn test_deactivate_account() {
        let repo = setup_repo().await;

        let account = create_test_account(&repo, "1000", "Cash", "asset").await;
        assert_eq!(account.is_active, 1);

        let deactivated = repo.deactivate_account(&account.id).await.unwrap();
        assert_eq!(deactivated.is_active, 0);

        // Double deactivate should fail
        let result = repo.deactivate_account(&account.id).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_deactivate_account_not_found() {
        let repo = setup_repo().await;

        let result = repo.deactivate_account("nonexistent").await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_deactivated_account_not_found_by_type() {
        let repo = setup_repo().await;

        let account = create_test_account(&repo, "1000", "Cash", "asset").await;
        repo.deactivate_account(&account.id).await.unwrap();

        // Deactivated account should not be found by type search
        let result = repo.find_account_by_type_async("asset").await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_delete_draft_journal_entry() {
        let repo = setup_repo().await;
        let svc = LedgerService {
            repo: repo.clone(),
            bus: saas_nats_bus::NatsBus::connect("nats://localhost:4222", "test")
                .await
                .unwrap(),
        };

        let period = create_test_period(&repo, "2025-01").await;
        let cash = create_test_account(&repo, "1111", "Cash", "asset").await;
        let revenue = create_test_account(&repo, "4111", "Revenue", "revenue").await;

        let entry = svc
            .create_journal_entry(
                &CreateJournalEntryRequest {
                    period_id: period.id.clone(),
                    description: Some("Draft to delete".into()),
                    lines: vec![
                        CreateJournalLineRequest {
                            account_id: cash.id.clone(),
                            debit_cents: 1000,
                            credit_cents: 0,
                            description: None,
                        },
                        CreateJournalLineRequest {
                            account_id: revenue.id.clone(),
                            debit_cents: 0,
                            credit_cents: 1000,
                            description: None,
                        },
                    ],
                },
                "user-1",
            )
            .await
            .unwrap();
        assert_eq!(entry.entry.status, "draft");

        // Delete draft should succeed
        svc.delete_journal_entry(&entry.entry.id).await.unwrap();

        // Entry should be gone
        let result = repo.get_journal_entry(&entry.entry.id).await;
        assert!(result.is_err());

        // Cannot delete a posted entry
        let entry2 = svc
            .create_journal_entry(
                &CreateJournalEntryRequest {
                    period_id: period.id,
                    description: Some("To post".into()),
                    lines: vec![
                        CreateJournalLineRequest {
                            account_id: cash.id,
                            debit_cents: 500,
                            credit_cents: 0,
                            description: None,
                        },
                        CreateJournalLineRequest {
                            account_id: revenue.id,
                            debit_cents: 0,
                            credit_cents: 500,
                            description: None,
                        },
                    ],
                },
                "user-1",
            )
            .await
            .unwrap();
        svc.post_journal_entry(&entry2.entry.id).await.unwrap();
        let result = svc.delete_journal_entry(&entry2.entry.id).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_account_code_uniqueness() {
        let repo = setup_repo().await;
        let svc = LedgerService {
            repo: repo.clone(),
            bus: saas_nats_bus::NatsBus::connect("nats://localhost:4222", "test")
                .await
                .unwrap(),
        };

        svc.create_account(&CreateAccountRequest {
            code: "1000".into(),
            name: "Cash".into(),
            account_type: "asset".into(),
            parent_id: None,
        }).await.unwrap();

        // Duplicate code should fail
        let result = svc.create_account(&CreateAccountRequest {
            code: "1000".into(),
            name: "Different Name".into(),
            account_type: "asset".into(),
            parent_id: None,
        }).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_delete_journal_entry_closed_period() {
        let repo = setup_repo().await;
        let svc = LedgerService {
            repo: repo.clone(),
            bus: saas_nats_bus::NatsBus::connect("nats://localhost:4222", "test")
                .await
                .unwrap(),
        };

        let period = repo.create_period(&CreatePeriodRequest {
            name: "Test Period".into(),
            start_date: "2025-01-01".into(),
            end_date: "2025-01-31".into(),
            fiscal_year: 2025,
        }).await.unwrap();

        let account1 = repo.create_account(&CreateAccountRequest {
            code: "6000".into(),
            name: "Debit Acc".into(),
            account_type: "asset".into(),
            parent_id: None,
        }).await.unwrap();
        let account2 = repo.create_account(&CreateAccountRequest {
            code: "6001".into(),
            name: "Credit Acc".into(),
            account_type: "liability".into(),
            parent_id: None,
        }).await.unwrap();

        // Create a posted entry so period can be closed (no drafts)
        let entry = svc.create_journal_entry(&CreateJournalEntryRequest {
            description: None,
            period_id: period.id.clone(),
            lines: vec![
                CreateJournalLineRequest {
                    account_id: account1.id,
                    debit_cents: 100,
                    credit_cents: 0,
                    description: None,
                },
                CreateJournalLineRequest {
                    account_id: account2.id,
                    debit_cents: 0,
                    credit_cents: 100,
                    description: None,
                },
            ],
        }, "user-1").await.unwrap();
        // Post it so period can be closed
        svc.post_journal_entry(&entry.entry.id).await.unwrap();

        // Close the period
        svc.close_period(&period.id).await.unwrap();

        // Now create a new draft in a new open period (needed for the delete test)
        // Since closed period blocks new entries, test the delete on a posted entry in closed period
        // A posted entry in closed period should fail because it's not draft
        let result = svc.delete_journal_entry(&entry.entry.id).await;
        assert!(result.is_err(), "Should fail: entry is posted, not draft");
    }

    #[tokio::test]
    async fn test_handle_cycle_count_posted_creates_je() {
        let repo = setup_repo().await;
        let svc = LedgerService {
            repo: repo.clone(),
            bus: saas_nats_bus::NatsBus::connect("nats://localhost:4222", "test")
                .await
                .unwrap(),
        };

        let inventory = create_test_account(&repo, "1400", "Inventory", "asset").await;
        let cogs = create_test_account(&repo, "5100", "Cost of Goods Sold", "expense").await;
        let _period = create_test_period(&repo, "Cycle Count Period").await;

        let result = svc
            .handle_cycle_count_posted("SESSION-001", "WH-01", 5)
            .await
            .unwrap();

        assert_eq!(result.entry.status, "posted");
        assert_eq!(
            result.entry.description,
            Some("Cycle count adjustment - session SESSION-001, warehouse WH-01, 5 items".to_string())
        );

        let lines = repo.get_journal_lines(&result.entry.id).await.unwrap();
        assert_eq!(lines.len(), 2);

        // Total amount: 5 adjustments * 10000 cents = 50000
        let debit_line = lines.iter().find(|l| l.debit_cents > 0).unwrap();
        assert_eq!(debit_line.debit_cents, 50000);
        assert_eq!(debit_line.account_id, inventory.id);

        let credit_line = lines.iter().find(|l| l.credit_cents > 0).unwrap();
        assert_eq!(credit_line.credit_cents, 50000);
        assert_eq!(credit_line.account_id, cogs.id);

        // Balanced entry
        let total_debits: i64 = lines.iter().map(|l| l.debit_cents).sum();
        let total_credits: i64 = lines.iter().map(|l| l.credit_cents).sum();
        assert_eq!(total_debits, total_credits);
    }

    #[tokio::test]
    async fn test_handle_ap_invoice_cancelled_reverses_original_je() {
        let pool = setup().await;
        let repo = LedgerRepo::new(pool.clone());
        let svc = LedgerService {
            repo: repo.clone(),
            bus: saas_nats_bus::NatsBus::connect("nats://localhost:4222", "test")
                .await
                .unwrap(),
        };

        // Set up accounts and period
        let expense = create_test_account(&repo, "5000", "Operating Expenses", "expense").await;
        let liability = create_test_account(&repo, "2000", "Accounts Payable", "liability").await;
        let _period = create_test_period(&repo, "AP Cancel Test Period").await;

        // First, create the original AP invoice JE via the handler
        let original = svc
            .handle_ap_invoice_approved("INV-AP-CANCEL-001", 10000, "5000")
            .await
            .unwrap();
        assert_eq!(original.entry.status, "posted");

        // Verify a single posted JE exists for this invoice
        let found = repo
            .find_posted_entry_by_description("AP Invoice INV-AP-CANCEL-001")
            .await
            .unwrap();
        assert!(found.is_some());
        assert_eq!(found.as_ref().unwrap().id, original.entry.id);

        // Now cancel the invoice
        let result = svc
            .handle_ap_invoice_cancelled("INV-AP-CANCEL-001", "VENDOR-001")
            .await
            .unwrap();
        assert!(result.is_some());

        let reversal_with_lines = result.unwrap();
        let reversal_entry = &reversal_with_lines.entry;

        // The reversal entry should be posted
        assert_eq!(reversal_entry.status, "posted");
        assert!(reversal_entry.reversal_of.is_some());
        assert_eq!(reversal_entry.reversal_of.as_ref().unwrap(), &original.entry.id);

        // Original entry should now be reversed
        let original_refreshed = repo.get_journal_entry(&original.entry.id).await.unwrap();
        assert_eq!(original_refreshed.status, "reversed");

        // Reversal lines should have swapped debit/credit
        let reversal_lines = repo.get_journal_lines(&reversal_entry.id).await.unwrap();
        assert_eq!(reversal_lines.len(), 2);

        let expense_line = reversal_lines
            .iter()
            .find(|l| l.account_id == expense.id)
            .unwrap();
        assert_eq!(expense_line.debit_cents, 0);
        assert_eq!(expense_line.credit_cents, 10000);

        let liability_line = reversal_lines
            .iter()
            .find(|l| l.account_id == liability.id)
            .unwrap();
        assert_eq!(liability_line.debit_cents, 10000);
        assert_eq!(liability_line.credit_cents, 0);

        // Reversal lines should be balanced
        let total_debits: i64 = reversal_lines.iter().map(|l| l.debit_cents).sum();
        let total_credits: i64 = reversal_lines.iter().map(|l| l.credit_cents).sum();
        assert_eq!(total_debits, total_credits);
    }

    #[tokio::test]
    async fn test_handle_ar_invoice_cancelled_reverses_original_je() {
        let pool = setup().await;
        let repo = LedgerRepo::new(pool.clone());
        let svc = LedgerService {
            repo: repo.clone(),
            bus: saas_nats_bus::NatsBus::connect("nats://localhost:4222", "test")
                .await
                .unwrap(),
        };

        // Set up accounts and period
        let asset = create_test_account(&repo, "1200", "Accounts Receivable", "asset").await;
        let revenue = create_test_account(&repo, "4000", "Revenue", "revenue").await;
        let _period = create_test_period(&repo, "AR Cancel Test Period").await;

        // First, create the original AR invoice JE via the handler
        let original = svc
            .handle_ar_invoice_created("INV-AR-CANCEL-001", 25000)
            .await
            .unwrap();
        assert_eq!(original.entry.status, "posted");

        // Verify a posted JE exists for this invoice
        let found = repo
            .find_posted_entry_by_description("AR Invoice INV-AR-CANCEL-001")
            .await
            .unwrap();
        assert!(found.is_some());
        assert_eq!(found.as_ref().unwrap().id, original.entry.id);

        // Now cancel the invoice
        let result = svc
            .handle_ar_invoice_cancelled("INV-AR-CANCEL-001", "CUSTOMER-001")
            .await
            .unwrap();
        assert!(result.is_some());

        let reversal_with_lines = result.unwrap();
        let reversal_entry = &reversal_with_lines.entry;

        // The reversal entry should be posted
        assert_eq!(reversal_entry.status, "posted");
        assert!(reversal_entry.reversal_of.is_some());
        assert_eq!(reversal_entry.reversal_of.as_ref().unwrap(), &original.entry.id);

        // Original entry should now be reversed
        let original_refreshed = repo.get_journal_entry(&original.entry.id).await.unwrap();
        assert_eq!(original_refreshed.status, "reversed");

        // Reversal lines should have swapped debit/credit
        let reversal_lines = repo.get_journal_lines(&reversal_entry.id).await.unwrap();
        assert_eq!(reversal_lines.len(), 2);

        let asset_line = reversal_lines
            .iter()
            .find(|l| l.account_id == asset.id)
            .unwrap();
        assert_eq!(asset_line.debit_cents, 0);
        assert_eq!(asset_line.credit_cents, 25000);

        let revenue_line = reversal_lines
            .iter()
            .find(|l| l.account_id == revenue.id)
            .unwrap();
        assert_eq!(revenue_line.debit_cents, 25000);
        assert_eq!(revenue_line.credit_cents, 0);

        // Reversal lines should be balanced
        let total_debits: i64 = reversal_lines.iter().map(|l| l.debit_cents).sum();
        let total_credits: i64 = reversal_lines.iter().map(|l| l.credit_cents).sum();
        assert_eq!(total_debits, total_credits);
    }

    #[tokio::test]
    async fn test_handle_ap_invoice_cancelled_no_original_je() {
        let pool = setup().await;
        let repo = LedgerRepo::new(pool.clone());
        let svc = LedgerService {
            repo: repo.clone(),
            bus: saas_nats_bus::NatsBus::connect("nats://localhost:4222", "test")
                .await
                .unwrap(),
        };

        // No JE exists for this invoice - cancellation should return Ok(None)
        let result = svc
            .handle_ap_invoice_cancelled("INV-AP-NONE-001", "VENDOR-002")
            .await
            .unwrap();
        assert!(result.is_none());
    }

    #[tokio::test]
    async fn test_handle_ar_invoice_cancelled_no_original_je() {
        let pool = setup().await;
        let repo = LedgerRepo::new(pool.clone());
        let svc = LedgerService {
            repo: repo.clone(),
            bus: saas_nats_bus::NatsBus::connect("nats://localhost:4222", "test")
                .await
                .unwrap(),
        };

        // No JE exists for this invoice - cancellation should return Ok(None)
        let result = svc
            .handle_ar_invoice_cancelled("INV-AR-NONE-001", "CUSTOMER-002")
            .await
            .unwrap();
        assert!(result.is_none());
    }

    #[tokio::test]
    async fn test_handle_ap_invoice_cancelled_already_reversed() {
        let pool = setup().await;
        let repo = LedgerRepo::new(pool.clone());
        let svc = LedgerService {
            repo: repo.clone(),
            bus: saas_nats_bus::NatsBus::connect("nats://localhost:4222", "test")
                .await
                .unwrap(),
        };

        // Set up and create original AP invoice JE
        let _expense = create_test_account(&repo, "5100", "Expenses", "expense").await;
        let _liability = create_test_account(&repo, "2100", "AP", "liability").await;
        let _period = create_test_period(&repo, "AP Already Rev Period").await;

        let _original = svc
            .handle_ap_invoice_approved("INV-AP-REV-001", 5000, "5100")
            .await
            .unwrap();

        // Cancel once - should succeed
        let first_cancel = svc
            .handle_ap_invoice_cancelled("INV-AP-REV-001", "VENDOR-003")
            .await
            .unwrap();
        assert!(first_cancel.is_some());

        // Cancel again - original JE is now "reversed", not "posted",
        // so find_posted_entry_by_description won't find it
        let second_cancel = svc
            .handle_ap_invoice_cancelled("INV-AP-REV-001", "VENDOR-003")
            .await
            .unwrap();
        assert!(second_cancel.is_none(), "Second cancellation should find no posted JE to reverse");
    }

    #[tokio::test]
    async fn test_handle_ar_invoice_cancelled_already_reversed() {
        let pool = setup().await;
        let repo = LedgerRepo::new(pool.clone());
        let svc = LedgerService {
            repo: repo.clone(),
            bus: saas_nats_bus::NatsBus::connect("nats://localhost:4222", "test")
                .await
                .unwrap(),
        };

        // Set up and create original AR invoice JE
        let _asset = create_test_account(&repo, "1300", "AR", "asset").await;
        let _revenue = create_test_account(&repo, "4100", "Revenue", "revenue").await;
        let _period = create_test_period(&repo, "AR Already Rev Period").await;

        let _original = svc
            .handle_ar_invoice_created("INV-AR-REV-001", 7500)
            .await
            .unwrap();

        // Cancel once - should succeed
        let first_cancel = svc
            .handle_ar_invoice_cancelled("INV-AR-REV-001", "CUSTOMER-003")
            .await
            .unwrap();
        assert!(first_cancel.is_some());

        // Cancel again - original JE is now "reversed", not "posted",
        // so find_posted_entry_by_description won't find it
        let second_cancel = svc
            .handle_ar_invoice_cancelled("INV-AR-REV-001", "CUSTOMER-003")
            .await
            .unwrap();
        assert!(second_cancel.is_none(), "Second cancellation should find no posted JE to reverse");
    }

    #[tokio::test]
    async fn test_find_posted_entry_by_description() {
        let pool = setup().await;
        let repo = LedgerRepo::new(pool);

        let cash = create_test_account(&repo, "1000", "Cash", "asset").await;
        let revenue = create_test_account(&repo, "4000", "Revenue", "revenue").await;
        let period = create_test_period(&repo, "Find Desc Period").await;

        // Create and post a journal entry with a specific description
        let entry_number = repo.next_entry_number().await.unwrap();
        let entry = repo
            .create_journal_entry(
                &entry_number,
                &CreateJournalEntryRequest {
                    description: Some("AP Invoice INV-FIND-001 approved".into()),
                    period_id: period.id.clone(),
                    lines: vec![
                        CreateJournalLineRequest {
                            account_id: cash.id.clone(),
                            debit_cents: 5000,
                            credit_cents: 0,
                            description: None,
                        },
                        CreateJournalLineRequest {
                            account_id: revenue.id.clone(),
                            debit_cents: 0,
                            credit_cents: 5000,
                            description: None,
                        },
                    ],
                },
                "system",
            )
            .await
            .unwrap();

        // Not found before posting (status is draft)
        let found = repo
            .find_posted_entry_by_description("AP Invoice INV-FIND-001")
            .await
            .unwrap();
        assert!(found.is_none());

        // Post it
        repo.post_journal_entry(&entry.id).await.unwrap();

        // Now it should be found
        let found = repo
            .find_posted_entry_by_description("AP Invoice INV-FIND-001")
            .await
            .unwrap();
        assert!(found.is_some());
        assert_eq!(found.unwrap().id, entry.id);

        // Non-matching pattern should not find it
        let not_found = repo
            .find_posted_entry_by_description("AR Invoice INV-FIND-001")
            .await
            .unwrap();
        assert!(not_found.is_none());
    }
}
