CREATE TABLE bank_transactions (
    id TEXT PRIMARY KEY,
    bank_account_id TEXT NOT NULL REFERENCES bank_accounts(id),
    amount_cents INTEGER NOT NULL,
    transaction_date TEXT NOT NULL,
    description TEXT,
    type TEXT NOT NULL CHECK(type IN ('deposit','withdrawal','transfer')),
    reference TEXT,
    created_at TEXT NOT NULL DEFAULT (datetime('now'))
);
