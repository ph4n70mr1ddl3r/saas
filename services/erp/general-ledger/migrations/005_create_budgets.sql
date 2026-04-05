CREATE TABLE IF NOT EXISTS budgets (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    period_id TEXT NOT NULL REFERENCES periods(id),
    status TEXT NOT NULL DEFAULT 'draft' CHECK(status IN ('draft','approved','active','closed')),
    total_budget_cents INTEGER NOT NULL DEFAULT 0,
    created_by TEXT NOT NULL,
    created_at TEXT NOT NULL DEFAULT (datetime('now'))
);
