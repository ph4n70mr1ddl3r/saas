CREATE TABLE IF NOT EXISTS budget_lines (
    id TEXT PRIMARY KEY,
    budget_id TEXT NOT NULL REFERENCES budgets(id),
    account_id TEXT NOT NULL REFERENCES accounts(id),
    budgeted_cents INTEGER NOT NULL CHECK(budgeted_cents >= 0),
    created_at TEXT NOT NULL DEFAULT (datetime('now'))
);
