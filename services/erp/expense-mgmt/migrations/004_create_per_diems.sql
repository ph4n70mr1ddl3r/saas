CREATE TABLE IF NOT EXISTS per_diems (
    id TEXT PRIMARY KEY,
    report_id TEXT NOT NULL REFERENCES expense_reports(id),
    location TEXT NOT NULL,
    start_date TEXT NOT NULL,
    end_date TEXT NOT NULL,
    daily_rate_cents INTEGER NOT NULL CHECK(daily_rate_cents > 0),
    total_cents INTEGER NOT NULL,
    created_at TEXT NOT NULL DEFAULT (datetime('now'))
);
