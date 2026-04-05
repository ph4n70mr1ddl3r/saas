use sqlx::SqlitePool;
use saas_nats_bus::NatsBus;
use saas_common::error::{AppError, AppResult};
use crate::models::*;
use crate::repository::CashManagementRepo;

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

    pub async fn create_bank_account(&self, input: &CreateBankAccountRequest) -> AppResult<BankAccount> {
        self.repo.create_bank_account(input).await
    }

    // --- Bank Transactions ---

    pub async fn list_bank_transactions(&self) -> AppResult<Vec<BankTransaction>> {
        self.repo.list_bank_transactions().await
    }

    pub async fn create_bank_transaction(&self, input: &CreateBankTransactionRequest) -> AppResult<BankTransaction> {
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

    pub async fn create_reconciliation(&self, input: &CreateReconciliationRequest) -> AppResult<Reconciliation> {
        // Validate bank account exists
        self.repo.get_bank_account(&input.bank_account_id).await?;
        self.repo.create_reconciliation(input).await
    }
}
