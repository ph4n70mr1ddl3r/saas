CREATE TABLE IF NOT EXISTS tax_brackets (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    min_income_cents INTEGER NOT NULL,
    max_income_cents INTEGER,
    rate_percent REAL NOT NULL,
    created_at TEXT NOT NULL DEFAULT (datetime('now'))
);
