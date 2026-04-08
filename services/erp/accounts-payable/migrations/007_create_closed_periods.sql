CREATE TABLE IF NOT EXISTS closed_periods (
    id TEXT PRIMARY KEY,
    period_name TEXT NOT NULL,
    fiscal_year INTEGER NOT NULL,
    period_start TEXT NOT NULL,
    period_end TEXT NOT NULL,
    closed_at TEXT NOT NULL DEFAULT (datetime('now')),
    UNIQUE(period_name, fiscal_year)
);

CREATE TABLE IF NOT EXISTS closed_fiscal_years (
    id TEXT PRIMARY KEY,
    fiscal_year INTEGER NOT NULL UNIQUE,
    closed_at TEXT NOT NULL DEFAULT (datetime('now'))
);
