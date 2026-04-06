use crate::models::*;
use crate::repository::CashManagementRepo;
use saas_common::error::{AppError, AppResult};
use saas_nats_bus::NatsBus;
use saas_proto::events::{
    BankAccountCreated, ReconciliationCompleted, TransferCompleted,
};
use sqlx::SqlitePool;

#[derive(Clone)]
pub struct CashManagementService {
    repo: CashManagementRepo,
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
        let account = self.repo.create_bank_account(input).await?;
        if let Err(e) = self
            .bus
            .publish(
                "erp.cash.account.created",
                BankAccountCreated {
                    account_id: account.id.clone(),
                    name: account.name.clone(),
                    bank_name: account.bank_name.clone(),
                    currency: account.currency.clone(),
                },
            )
            .await
        {
            tracing::error!(
                "CRITICAL: Failed to publish event '{}': {}. Data may be inconsistent.",
                "erp.cash.account.created",
                e
            );
        }
        Ok(account)
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
        let recon = self.repo.create_reconciliation(input).await?;

        // Publish reconciliation completed event if balanced
        if recon.status == "completed" {
            if let Err(e) = self
                .bus
                .publish(
                    "erp.cash.reconciliation.completed",
                    ReconciliationCompleted {
                        reconciliation_id: recon.id.clone(),
                        bank_account_id: recon.bank_account_id.clone(),
                        book_balance_cents: recon.book_balance_cents,
                        statement_balance_cents: recon.statement_balance_cents,
                        difference_cents: recon.difference_cents,
                    },
                )
                .await
            {
                tracing::error!(
                    "CRITICAL: Failed to publish event '{}': {}. Data may be inconsistent.",
                    "erp.cash.reconciliation.completed",
                    e
                );
            }
        }

        Ok(recon)
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

        let result = self
            .repo
            .transfer_between_accounts(
                &input.from_account_id,
                &input.to_account_id,
                input.amount_cents,
                &input.transfer_date,
                input.description.as_deref(),
                input.reference.as_deref(),
            )
            .await?;

        if let Err(e) = self
            .bus
            .publish(
                "erp.cash.transfer.completed",
                TransferCompleted {
                    from_account_id: input.from_account_id.clone(),
                    to_account_id: input.to_account_id.clone(),
                    amount_cents: input.amount_cents,
                    currency: from_account.currency.clone(),
                },
            )
            .await
        {
            tracing::error!(
                "CRITICAL: Failed to publish event '{}': {}. Data may be inconsistent.",
                "erp.cash.transfer.completed",
                e
            );
        }

        Ok(result)
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
            include_str!("../../migrations/001_create_bank_accounts.sql"),
            include_str!("../../migrations/002_create_bank_transactions.sql"),
            include_str!("../../migrations/003_create_reconciliations.sql"),
            include_str!("../../migrations/004_add_flow_category.sql"),
        ];
        let migration_names = [
            "001_create_bank_accounts.sql",
            "002_create_bank_transactions.sql",
            "003_create_reconciliations.sql",
            "004_add_flow_category.sql",
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

    async fn setup_repo() -> CashManagementRepo {
        let pool = setup().await;
        CashManagementRepo::new(pool)
    }

    #[tokio::test]
    async fn test_bank_account_crud() {
        let repo = setup_repo().await;

        let input = CreateBankAccountRequest {
            name: "Operating Account".into(),
            bank_name: "First Bank".into(),
            account_number: "1234567890".into(),
            routing_number: Some("021000021".into()),
            balance_cents: Some(100_000),
            currency: Some("USD".into()),
        };
        let account = repo.create_bank_account(&input).await.unwrap();
        assert_eq!(account.name, "Operating Account");
        assert_eq!(account.balance_cents, 100_000);
        assert_eq!(account.currency, "USD");

        let fetched = repo.get_bank_account(&account.id).await.unwrap();
        assert_eq!(fetched.id, account.id);
        assert_eq!(fetched.bank_name, "First Bank");

        let accounts = repo.list_bank_accounts().await.unwrap();
        assert_eq!(accounts.len(), 1);

        let updated = repo.update_balance(&account.id, 250_000).await.unwrap();
        assert_eq!(updated.balance_cents, 250_000);
    }

    #[tokio::test]
    async fn test_deposit_increases_balance() {
        let repo = setup_repo().await;

        let acct = repo
            .create_bank_account(&CreateBankAccountRequest {
                name: "Savings".into(),
                bank_name: "Bank A".into(),
                account_number: "1111111111".into(),
                routing_number: None,
                balance_cents: Some(50_000),
                currency: Some("USD".into()),
            })
            .await
            .unwrap();

        let tx = repo
            .create_bank_transaction(&CreateBankTransactionRequest {
                bank_account_id: acct.id.clone(),
                amount_cents: 25_000,
                transaction_date: "2025-06-01".into(),
                description: Some("Cash deposit".into()),
                r#type: "deposit".into(),
                reference: None,
            })
            .await
            .unwrap();

        assert_eq!(tx.r#type, "deposit");
        assert_eq!(tx.amount_cents, 25_000);

        let updated_acct = repo.get_bank_account(&acct.id).await.unwrap();
        assert_eq!(updated_acct.balance_cents, 75_000);
    }

    #[tokio::test]
    async fn test_withdrawal_decreases_balance() {
        let repo = setup_repo().await;

        let acct = repo
            .create_bank_account(&CreateBankAccountRequest {
                name: "Checking".into(),
                bank_name: "Bank B".into(),
                account_number: "2222222222".into(),
                routing_number: None,
                balance_cents: Some(100_000),
                currency: Some("USD".into()),
            })
            .await
            .unwrap();

        let tx = repo
            .create_bank_transaction(&CreateBankTransactionRequest {
                bank_account_id: acct.id.clone(),
                amount_cents: 30_000,
                transaction_date: "2025-06-02".into(),
                description: Some("ATM withdrawal".into()),
                r#type: "withdrawal".into(),
                reference: None,
            })
            .await
            .unwrap();

        assert_eq!(tx.r#type, "withdrawal");

        let updated_acct = repo.get_bank_account(&acct.id).await.unwrap();
        assert_eq!(updated_acct.balance_cents, 70_000);
    }

    #[tokio::test]
    async fn test_withdrawal_prevents_negative_balance() {
        let repo = setup_repo().await;

        let acct = repo
            .create_bank_account(&CreateBankAccountRequest {
                name: "Low Balance".into(),
                bank_name: "Bank C".into(),
                account_number: "3333333333".into(),
                routing_number: None,
                balance_cents: Some(10_000),
                currency: Some("USD".into()),
            })
            .await
            .unwrap();

        let result = repo
            .create_bank_transaction(&CreateBankTransactionRequest {
                bank_account_id: acct.id.clone(),
                amount_cents: 20_000,
                transaction_date: "2025-06-03".into(),
                description: None,
                r#type: "withdrawal".into(),
                reference: None,
            })
            .await;

        assert!(result.is_err());
        let acct_after = repo.get_bank_account(&acct.id).await.unwrap();
        assert_eq!(acct_after.balance_cents, 10_000);
    }

    #[tokio::test]
    async fn test_transfer_between_accounts() {
        let repo = setup_repo().await;

        let from_acct = repo
            .create_bank_account(&CreateBankAccountRequest {
                name: "Source".into(),
                bank_name: "Bank A".into(),
                account_number: "4444444444".into(),
                routing_number: None,
                balance_cents: Some(100_000),
                currency: Some("USD".into()),
            })
            .await
            .unwrap();

        let to_acct = repo
            .create_bank_account(&CreateBankAccountRequest {
                name: "Destination".into(),
                bank_name: "Bank A".into(),
                account_number: "5555555555".into(),
                routing_number: None,
                balance_cents: Some(0),
                currency: Some("USD".into()),
            })
            .await
            .unwrap();

        let (withdrawal, deposit) = repo
            .transfer_between_accounts(
                &from_acct.id,
                &to_acct.id,
                40_000,
                "2025-06-10",
                Some("Inter-account transfer"),
                None,
            )
            .await
            .unwrap();

        assert_eq!(withdrawal.r#type, "withdrawal");
        assert_eq!(deposit.r#type, "deposit");
        assert_eq!(withdrawal.amount_cents, 40_000);
        assert_eq!(deposit.amount_cents, 40_000);

        let from_after = repo.get_bank_account(&from_acct.id).await.unwrap();
        let to_after = repo.get_bank_account(&to_acct.id).await.unwrap();
        assert_eq!(from_after.balance_cents, 60_000);
        assert_eq!(to_after.balance_cents, 40_000);
    }

    #[tokio::test]
    async fn test_reconciliation_balanced() {
        let repo = setup_repo().await;

        let acct = repo
            .create_bank_account(&CreateBankAccountRequest {
                name: "Recon Account".into(),
                bank_name: "Bank D".into(),
                account_number: "6666666666".into(),
                routing_number: None,
                balance_cents: Some(50_000),
                currency: Some("USD".into()),
            })
            .await
            .unwrap();

        let recon = repo
            .create_reconciliation(&CreateReconciliationRequest {
                bank_account_id: acct.id.clone(),
                period_start: "2025-06-01".into(),
                period_end: "2025-06-30".into(),
                statement_balance_cents: 50_000,
            })
            .await
            .unwrap();

        assert_eq!(recon.status, "completed");
        assert_eq!(recon.difference_cents, 0);
        assert_eq!(recon.book_balance_cents, 50_000);
    }

    #[tokio::test]
    async fn test_reconciliation_unbalanced() {
        let repo = setup_repo().await;

        let acct = repo
            .create_bank_account(&CreateBankAccountRequest {
                name: "Unbalanced Acct".into(),
                bank_name: "Bank E".into(),
                account_number: "7777777777".into(),
                routing_number: None,
                balance_cents: Some(50_000),
                currency: Some("USD".into()),
            })
            .await
            .unwrap();

        let recon = repo
            .create_reconciliation(&CreateReconciliationRequest {
                bank_account_id: acct.id.clone(),
                period_start: "2025-06-01".into(),
                period_end: "2025-06-30".into(),
                statement_balance_cents: 45_000,
            })
            .await
            .unwrap();

        assert_eq!(recon.status, "open");
        assert_eq!(recon.difference_cents, -5_000);
    }

    #[tokio::test]
    async fn test_transfer_insufficient_balance_rejected() {
        let repo = setup_repo().await;

        let from_acct = repo
            .create_bank_account(&CreateBankAccountRequest {
                name: "Empty Source".into(),
                bank_name: "Bank F".into(),
                account_number: "8888888888".into(),
                routing_number: None,
                balance_cents: Some(5_000),
                currency: Some("USD".into()),
            })
            .await
            .unwrap();

        let to_acct = repo
            .create_bank_account(&CreateBankAccountRequest {
                name: "Target".into(),
                bank_name: "Bank F".into(),
                account_number: "9999999999".into(),
                routing_number: None,
                balance_cents: Some(0),
                currency: Some("USD".into()),
            })
            .await
            .unwrap();

        let result = repo
            .transfer_between_accounts(
                &from_acct.id,
                &to_acct.id,
                10_000,
                "2025-06-15",
                None,
                None,
            )
            .await;

        assert!(result.is_err());
        let from_after = repo.get_bank_account(&from_acct.id).await.unwrap();
        let to_after = repo.get_bank_account(&to_acct.id).await.unwrap();
        assert_eq!(from_after.balance_cents, 5_000);
        assert_eq!(to_after.balance_cents, 0);
    }

    #[tokio::test]
    async fn test_transaction_listing_by_period() {
        let repo = setup_repo().await;

        let acct = repo
            .create_bank_account(&CreateBankAccountRequest {
                name: "Period Acct".into(),
                bank_name: "Bank G".into(),
                account_number: "1010101010".into(),
                routing_number: None,
                balance_cents: Some(100_000),
                currency: Some("USD".into()),
            })
            .await
            .unwrap();

        repo.create_bank_transaction(&CreateBankTransactionRequest {
            bank_account_id: acct.id.clone(),
            amount_cents: 10_000,
            transaction_date: "2025-06-15".into(),
            description: None,
            r#type: "deposit".into(),
            reference: None,
        })
        .await
        .unwrap();

        repo.create_bank_transaction(&CreateBankTransactionRequest {
            bank_account_id: acct.id.clone(),
            amount_cents: 5_000,
            transaction_date: "2025-07-10".into(),
            description: None,
            r#type: "withdrawal".into(),
            reference: None,
        })
        .await
        .unwrap();

        let june_txns = repo
            .get_transactions_for_period(&acct.id, "2025-06-01", "2025-06-30")
            .await
            .unwrap();
        assert_eq!(june_txns.len(), 1);
        assert_eq!(june_txns[0].amount_cents, 10_000);

        let all_txns = repo
            .get_transactions_for_period(&acct.id, "2025-06-01", "2025-07-31")
            .await
            .unwrap();
        assert_eq!(all_txns.len(), 2);
    }

    #[tokio::test]
    async fn test_multiple_reconciliations_for_account() {
        let repo = setup_repo().await;

        let acct = repo
            .create_bank_account(&CreateBankAccountRequest {
                name: "Multi Recon".into(),
                bank_name: "Bank H".into(),
                account_number: "1212121212".into(),
                routing_number: None,
                balance_cents: Some(75_000),
                currency: Some("USD".into()),
            })
            .await
            .unwrap();

        repo.create_reconciliation(&CreateReconciliationRequest {
            bank_account_id: acct.id.clone(),
            period_start: "2025-05-01".into(),
            period_end: "2025-05-31".into(),
            statement_balance_cents: 75_000,
        })
        .await
        .unwrap();

        repo.create_reconciliation(&CreateReconciliationRequest {
            bank_account_id: acct.id.clone(),
            period_start: "2025-06-01".into(),
            period_end: "2025-06-30".into(),
            statement_balance_cents: 75_000,
        })
        .await
        .unwrap();

        let recons = repo.list_reconciliations().await.unwrap();
        assert_eq!(recons.len(), 2);
    }
}
