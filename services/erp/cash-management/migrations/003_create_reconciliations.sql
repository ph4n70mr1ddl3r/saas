CREATE TABLE reconciliations (
    id TEXT PRIMARY KEY,
    bank_account_id TEXT NOT NULL REFERENCES bank_accounts(id),
    period_start TEXT NOT NULL,
    period_end TEXT NOT NULL,
    statement_balance_cents INTEGER NOT NULL,
    book_balance_cents INTEGER NOT NULL,
    difference_cents INTEGER NOT NULL DEFAULT 0,
    status TEXT NOT NULL DEFAULT 'open' CHECK(status IN ('open','completed')),
    created_at TEXT NOT NULL DEFAULT (datetime('now'))
);
