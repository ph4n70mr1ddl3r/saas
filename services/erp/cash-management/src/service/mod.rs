use crate::models::*;
use crate::repository::CashManagementRepo;
use saas_common::error::{AppError, AppResult};
use saas_nats_bus::NatsBus;
use sqlx::SqlitePool;

#[derive(Clone)]
pub struct CashManagementService {
    repo: CashManagementRepo,
    #[allow(dead_code)]
    bus: NatsBus,
}

impl CashManagementService {
    pub fn new(pool: SqlitePool, bus: NatsBus) -> Self {
        Self {
            repo: CashManagementRepo::new(pool),
            bus,
        }
    }

    // --- Bank Accounts ---

    pub async fn list_bank_accounts(&self) -> AppResult<Vec<BankAccount>> {
        self.repo.list_bank_accounts().await
    }

    pub async fn get_bank_account(&self, id: &str) -> AppResult<BankAccount> {
        self.repo.get_bank_account(id).await
    }

    pub async fn create_bank_account(
        &self,
        input: &CreateBankAccountRequest,
    ) -> AppResult<BankAccount> {
        self.repo.create_bank_account(input).await
    }

    // --- Bank Transactions ---

    pub async fn list_bank_transactions(&self) -> AppResult<Vec<BankTransaction>> {
        self.repo.list_bank_transactions().await
    }

    pub async fn create_bank_transaction(
        &self,
        input: &CreateBankTransactionRequest,
    ) -> AppResult<BankTransaction> {
        // Validate bank account exists
        self.repo.get_bank_account(&input.bank_account_id).await?;

        // Validate transaction type
        let valid_types = ["deposit", "withdrawal", "transfer"];
        if !valid_types.contains(&input.r#type.as_str()) {
            return Err(AppError::Validation(format!(
                "Invalid transaction type '{}'. Must be one of: {:?}",
                input.r#type, valid_types
            )));
        }

        if input.amount_cents < 0 {
            return Err(AppError::Validation("Amount must be non-negative".into()));
        }

        self.repo.create_bank_transaction(input).await
    }

    // --- Reconciliations ---

    pub async fn list_reconciliations(&self) -> AppResult<Vec<Reconciliation>> {
        self.repo.list_reconciliations().await
    }

    pub async fn create_reconciliation(
        &self,
        input: &CreateReconciliationRequest,
    ) -> AppResult<Reconciliation> {
        // Validate bank account exists
        self.repo.get_bank_account(&input.bank_account_id).await?;
        self.repo.create_reconciliation(input).await
    }

    // --- Cash Flow Statement ---

    pub async fn cash_flow_statement(
        &self,
        period_start: &str,
        period_end: &str,
    ) -> AppResult<CashFlowStatement> {
        let rows = self
            .repo
            .cash_flow_statement(period_start, period_end)
            .await?;

        let mut operating = Vec::new();
        let mut investing = Vec::new();
        let mut financing = Vec::new();
        let mut total_operating_cents: i64 = 0;
        let mut total_investing_cents: i64 = 0;
        let mut total_financing_cents: i64 = 0;

        for row in rows {
            match row.category.as_str() {
                "operating" => {
                    total_operating_cents += row.amount_cents;
                    operating.push(row);
                }
                "investing" => {
                    total_investing_cents += row.amount_cents;
                    investing.push(row);
                }
                "financing" => {
                    total_financing_cents += row.amount_cents;
                    financing.push(row);
                }
                _ => {
                    // Uncategorized rows go to operating
                    total_operating_cents += row.amount_cents;
                    operating.push(row);
                }
            }
        }

        let net_change_cents =
            total_operating_cents + total_investing_cents + total_financing_cents;

        Ok(CashFlowStatement {
            operating,
            total_operating_cents,
            investing,
            total_investing_cents,
            financing,
            total_financing_cents,
            net_change_cents,
        })
    }

    // --- Transfers ---

    pub async fn transfer(
        &self,
        input: &TransferRequest,
    ) -> AppResult<(BankTransaction, BankTransaction)> {
        // Validate accounts exist
        let from_account = self.repo.get_bank_account(&input.from_account_id).await?;
        let to_account = self.repo.get_bank_account(&input.to_account_id).await?;

        // Validate different accounts
        if input.from_account_id == input.to_account_id {
            return Err(AppError::Validation(
                "Source and destination accounts must be different".into(),
            ));
        }

        // Validate positive amount
        if input.amount_cents <= 0 {
            return Err(AppError::Validation(
                "Transfer amount must be positive".into(),
            ));
        }

        // Validate sufficient balance on source
        if from_account.balance_cents < input.amount_cents {
            return Err(AppError::Validation(format!(
                "Insufficient balance on source account. Available: {}, Requested: {}",
                from_account.balance_cents, input.amount_cents
            )));
        }

        // Verify currencies match
        if from_account.currency != to_account.currency {
            return Err(AppError::Validation(
                "Source and destination accounts must have the same currency".into(),
            ));
        }

        self.repo
            .transfer_between_accounts(
                &input.from_account_id,
                &input.to_account_id,
                input.amount_cents,
                &input.transfer_date,
                input.description.as_deref(),
                input.reference.as_deref(),
            )
            .await
    }
}
