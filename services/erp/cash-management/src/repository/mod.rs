use crate::models::*;
use saas_common::error::{AppError, AppResult};
use sqlx::SqlitePool;

#[derive(Clone)]
pub struct CashManagementRepo {
    pool: SqlitePool,
}

impl CashManagementRepo {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    // --- Bank Accounts ---

    pub async fn list_bank_accounts(&self) -> AppResult<Vec<BankAccount>> {
        let rows = sqlx::query_as::<_, BankAccount>(
            "SELECT id, name, bank_name, account_number, routing_number, balance_cents, currency, created_at FROM bank_accounts ORDER BY name",
        )
        .fetch_all(&self.pool)
        .await?;
        Ok(rows)
    }

    pub async fn get_bank_account(&self, id: &str) -> AppResult<BankAccount> {
        sqlx::query_as::<_, BankAccount>(
            "SELECT id, name, bank_name, account_number, routing_number, balance_cents, currency, created_at FROM bank_accounts WHERE id = ?",
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("Bank account '{}' not found", id)))
    }

    pub async fn create_bank_account(
        &self,
        input: &CreateBankAccountRequest,
    ) -> AppResult<BankAccount> {
        let id = uuid::Uuid::new_v4().to_string();
        let balance_cents = input.balance_cents.unwrap_or(0);
        let currency = input.currency.as_deref().unwrap_or("USD");

        sqlx::query(
            "INSERT INTO bank_accounts (id, name, bank_name, account_number, routing_number, balance_cents, currency) VALUES (?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(&id)
        .bind(&input.name)
        .bind(&input.bank_name)
        .bind(&input.account_number)
        .bind(&input.routing_number)
        .bind(balance_cents)
        .bind(currency)
        .execute(&self.pool)
        .await?;
        self.get_bank_account(&id).await
    }

    pub async fn update_balance(&self, id: &str, new_balance_cents: i64) -> AppResult<BankAccount> {
        sqlx::query("UPDATE bank_accounts SET balance_cents = ? WHERE id = ?")
            .bind(new_balance_cents)
            .bind(id)
            .execute(&self.pool)
            .await?;
        self.get_bank_account(id).await
    }

    pub async fn update_bank_account(
        &self,
        id: &str,
        input: &crate::models::UpdateBankAccountRequest,
    ) -> AppResult<BankAccount> {
        self.get_bank_account(id).await?;
        sqlx::query(
            "UPDATE bank_accounts SET name = COALESCE(?, name), bank_name = COALESCE(?, bank_name), account_number = COALESCE(?, account_number), routing_number = COALESCE(?, routing_number), currency = COALESCE(?, currency) WHERE id = ?",
        )
        .bind(&input.name)
        .bind(&input.bank_name)
        .bind(&input.account_number)
        .bind(&input.routing_number)
        .bind(&input.currency)
        .bind(id)
        .execute(&self.pool)
        .await?;
        self.get_bank_account(id).await
    }

    // --- Bank Transactions ---

    pub async fn list_bank_transactions(&self) -> AppResult<Vec<BankTransaction>> {
        let rows = sqlx::query_as::<_, BankTransaction>(
            "SELECT id, bank_account_id, amount_cents, transaction_date, description, type, reference, created_at FROM bank_transactions ORDER BY transaction_date DESC",
        )
        .fetch_all(&self.pool)
        .await?;
        Ok(rows)
    }

    pub async fn create_bank_transaction(
        &self,
        input: &CreateBankTransactionRequest,
    ) -> AppResult<BankTransaction> {
        // Validate amount is positive
        if input.amount_cents <= 0 {
            return Err(AppError::Validation(
                "Transaction amount must be positive".into(),
            ));
        }

        let id = uuid::Uuid::new_v4().to_string();

        // Compute delta: positive for deposits, negative for withdrawals
        let delta: i64 = match input.r#type.as_str() {
            "deposit" => input.amount_cents,
            "withdrawal" => -input.amount_cents,
            "transfer" => input.amount_cents, // positive for incoming transfer
            _ => input.amount_cents,
        };

        // Wrap transaction insert and atomic balance update in a database transaction
        let mut tx = self.pool.begin().await?;

        sqlx::query(
            "INSERT INTO bank_transactions (id, bank_account_id, amount_cents, transaction_date, description, type, reference) VALUES (?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(&id)
        .bind(&input.bank_account_id)
        .bind(input.amount_cents)
        .bind(&input.transaction_date)
        .bind(&input.description)
        .bind(&input.r#type)
        .bind(&input.reference)
        .execute(&mut *tx)
        .await?;

        // Atomic balance update
        sqlx::query("UPDATE bank_accounts SET balance_cents = balance_cents + ? WHERE id = ?")
            .bind(delta)
            .bind(&input.bank_account_id)
            .execute(&mut *tx)
            .await?;

        // Check for negative balance BEFORE commit (prevents overdrafts)
        let balance: i64 =
            sqlx::query_scalar("SELECT balance_cents FROM bank_accounts WHERE id = ?")
                .bind(&input.bank_account_id)
                .fetch_one(&mut *tx)
                .await?;

        if balance < 0 {
            // Rollback the transaction by returning early - the tx is dropped without commit
            return Err(AppError::Validation(
                "Transaction would result in negative bank account balance".into(),
            ));
        }

        tx.commit().await?;

        sqlx::query_as::<_, BankTransaction>(
            "SELECT id, bank_account_id, amount_cents, transaction_date, description, type, reference, created_at FROM bank_transactions WHERE id = ?",
        )
        .bind(&id)
        .fetch_one(&self.pool)
        .await
        .map_err(|e| e.into())
    }

    pub async fn get_transactions_for_period(
        &self,
        bank_account_id: &str,
        period_start: &str,
        period_end: &str,
    ) -> AppResult<Vec<BankTransaction>> {
        let rows = sqlx::query_as::<_, BankTransaction>(
            "SELECT id, bank_account_id, amount_cents, transaction_date, description, type, reference, created_at FROM bank_transactions WHERE bank_account_id = ? AND transaction_date >= ? AND transaction_date <= ? ORDER BY transaction_date",
        )
        .bind(bank_account_id)
        .bind(period_start)
        .bind(period_end)
        .fetch_all(&self.pool)
        .await?;
        Ok(rows)
    }

    // --- Reconciliations ---

    pub async fn list_reconciliations(&self) -> AppResult<Vec<Reconciliation>> {
        let rows = sqlx::query_as::<_, Reconciliation>(
            "SELECT id, bank_account_id, period_start, period_end, statement_balance_cents, book_balance_cents, difference_cents, status, created_at FROM reconciliations ORDER BY created_at DESC",
        )
        .fetch_all(&self.pool)
        .await?;
        Ok(rows)
    }

    pub async fn create_reconciliation(
        &self,
        input: &CreateReconciliationRequest,
    ) -> AppResult<Reconciliation> {
        let id = uuid::Uuid::new_v4().to_string();

        // Wrap balance read and insert in a single transaction to prevent TOCTOU
        let mut tx = self.pool.begin().await?;

        // Get the current book balance from the bank account (within tx)
        let book_balance_cents: i64 =
            sqlx::query_scalar("SELECT balance_cents FROM bank_accounts WHERE id = ?")
                .bind(&input.bank_account_id)
                .fetch_one(&mut *tx)
                .await
                .map_err(|_| {
                    AppError::NotFound(format!(
                        "Bank account '{}' not found",
                        input.bank_account_id
                    ))
                })?;

        let difference_cents = input.statement_balance_cents - book_balance_cents;
        let status = if difference_cents == 0 {
            "completed"
        } else {
            "open"
        };

        sqlx::query(
            "INSERT INTO reconciliations (id, bank_account_id, period_start, period_end, statement_balance_cents, book_balance_cents, difference_cents, status) VALUES (?, ?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(&id)
        .bind(&input.bank_account_id)
        .bind(&input.period_start)
        .bind(&input.period_end)
        .bind(input.statement_balance_cents)
        .bind(book_balance_cents)
        .bind(difference_cents)
        .bind(status)
        .execute(&mut *tx)
        .await?;

        tx.commit().await?;

        sqlx::query_as::<_, Reconciliation>(
            "SELECT id, bank_account_id, period_start, period_end, statement_balance_cents, book_balance_cents, difference_cents, status, created_at FROM reconciliations WHERE id = ?",
        )
        .bind(&id)
        .fetch_one(&self.pool)
        .await
        .map_err(|e| e.into())
    }

    // --- Cash Flow Statement ---

    pub async fn cash_flow_statement(
        &self,
        period_start: &str,
        period_end: &str,
    ) -> AppResult<Vec<CashFlowRow>> {
        let rows = sqlx::query_as::<_, CashFlowRow>(
            r#"
            SELECT
                COALESCE(flow_category, 'uncategorized') AS category,
                description,
                amount_cents
            FROM bank_transactions
            WHERE transaction_date >= ? AND transaction_date <= ?
                AND flow_category IS NOT NULL
            ORDER BY flow_category, transaction_date
            "#,
        )
        .bind(period_start)
        .bind(period_end)
        .fetch_all(&self.pool)
        .await?;
        Ok(rows)
    }

    // --- Transfers ---

    pub async fn transfer_between_accounts(
        &self,
        from_id: &str,
        to_id: &str,
        amount_cents: i64,
        transfer_date: &str,
        description: Option<&str>,
        reference: Option<&str>,
    ) -> AppResult<(BankTransaction, BankTransaction)> {
        let withdrawal_id = uuid::Uuid::new_v4().to_string();
        let deposit_id = uuid::Uuid::new_v4().to_string();

        let mut tx = self.pool.begin().await?;

        // Insert withdrawal on source account
        sqlx::query(
            "INSERT INTO bank_transactions (id, bank_account_id, amount_cents, transaction_date, description, type, reference, flow_category) VALUES (?, ?, ?, ?, ?, 'withdrawal', ?, 'operating')",
        )
        .bind(&withdrawal_id)
        .bind(from_id)
        .bind(amount_cents)
        .bind(transfer_date)
        .bind(description)
        .bind(reference)
        .execute(&mut *tx)
        .await?;

        // Debit source account balance
        sqlx::query("UPDATE bank_accounts SET balance_cents = balance_cents - ? WHERE id = ?")
            .bind(amount_cents)
            .bind(from_id)
            .execute(&mut *tx)
            .await?;

        // Check source balance is non-negative
        let from_balance: i64 =
            sqlx::query_scalar("SELECT balance_cents FROM bank_accounts WHERE id = ?")
                .bind(from_id)
                .fetch_one(&mut *tx)
                .await?;

        if from_balance < 0 {
            return Err(AppError::Validation(
                "Transfer would result in negative balance on source account".into(),
            ));
        }

        // Insert deposit on target account
        sqlx::query(
            "INSERT INTO bank_transactions (id, bank_account_id, amount_cents, transaction_date, description, type, reference, flow_category) VALUES (?, ?, ?, ?, ?, 'deposit', ?, 'operating')",
        )
        .bind(&deposit_id)
        .bind(to_id)
        .bind(amount_cents)
        .bind(transfer_date)
        .bind(description)
        .bind(reference)
        .execute(&mut *tx)
        .await?;

        // Credit target account balance
        sqlx::query("UPDATE bank_accounts SET balance_cents = balance_cents + ? WHERE id = ?")
            .bind(amount_cents)
            .bind(to_id)
            .execute(&mut *tx)
            .await?;

        tx.commit().await?;

        // Fetch the created transactions
        let withdrawal = sqlx::query_as::<_, BankTransaction>(
            "SELECT id, bank_account_id, amount_cents, transaction_date, description, type, reference, created_at FROM bank_transactions WHERE id = ?",
        )
        .bind(&withdrawal_id)
        .fetch_one(&self.pool)
        .await?;

        let deposit = sqlx::query_as::<_, BankTransaction>(
            "SELECT id, bank_account_id, amount_cents, transaction_date, description, type, reference, created_at FROM bank_transactions WHERE id = ?",
        )
        .bind(&deposit_id)
        .fetch_one(&self.pool)
        .await?;

        Ok((withdrawal, deposit))
    }
}
