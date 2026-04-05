use sqlx::SqlitePool;
use saas_common::error::{AppError, AppResult};
use crate::models::*;

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

    pub async fn create_bank_account(&self, input: &CreateBankAccountRequest) -> AppResult<BankAccount> {
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

    // --- Bank Transactions ---

    pub async fn list_bank_transactions(&self) -> AppResult<Vec<BankTransaction>> {
        let rows = sqlx::query_as::<_, BankTransaction>(
            "SELECT id, bank_account_id, amount_cents, transaction_date, description, type, reference, created_at FROM bank_transactions ORDER BY transaction_date DESC",
        )
        .fetch_all(&self.pool)
        .await?;
        Ok(rows)
    }

    pub async fn create_bank_transaction(&self, input: &CreateBankTransactionRequest) -> AppResult<BankTransaction> {
        let id = uuid::Uuid::new_v4().to_string();

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
        .execute(&self.pool)
        .await?;

        // Update bank account balance
        let account = self.get_bank_account(&input.bank_account_id).await?;
        let adjustment = match input.r#type.as_str() {
            "deposit" => input.amount_cents,
            "withdrawal" => -input.amount_cents,
            "transfer" => input.amount_cents, // positive for incoming transfer
            _ => input.amount_cents,
        };
        let new_balance = account.balance_cents + adjustment;
        self.update_balance(&input.bank_account_id, new_balance).await?;

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

    pub async fn create_reconciliation(&self, input: &CreateReconciliationRequest) -> AppResult<Reconciliation> {
        let id = uuid::Uuid::new_v4().to_string();

        // Get the current book balance from the bank account
        let account = self.get_bank_account(&input.bank_account_id).await?;
        let book_balance_cents = account.balance_cents;
        let difference_cents = input.statement_balance_cents - book_balance_cents;
        let status = if difference_cents == 0 { "completed" } else { "open" };

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
        .execute(&self.pool)
        .await?;

        sqlx::query_as::<_, Reconciliation>(
            "SELECT id, bank_account_id, period_start, period_end, statement_balance_cents, book_balance_cents, difference_cents, status, created_at FROM reconciliations WHERE id = ?",
        )
        .bind(&id)
        .fetch_one(&self.pool)
        .await
        .map_err(|e| e.into())
    }
}
