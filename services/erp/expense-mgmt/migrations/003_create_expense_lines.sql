CREATE TABLE IF NOT EXISTS expense_lines (
    id TEXT PRIMARY KEY,
    report_id TEXT NOT NULL REFERENCES expense_reports(id),
    expense_date TEXT NOT NULL,
    category_id TEXT NOT NULL REFERENCES expense_categories(id),
    amount_cents INTEGER NOT NULL CHECK(amount_cents > 0),
    description TEXT,
    receipt_url TEXT,
    created_at TEXT NOT NULL DEFAULT (datetime('now'))
);
