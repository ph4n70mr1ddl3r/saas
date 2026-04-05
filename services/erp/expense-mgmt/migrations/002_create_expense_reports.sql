CREATE TABLE IF NOT EXISTS expense_reports (
    id TEXT PRIMARY KEY,
    employee_id TEXT NOT NULL,
    title TEXT NOT NULL,
    description TEXT,
    total_cents INTEGER NOT NULL DEFAULT 0,
    status TEXT NOT NULL DEFAULT 'draft' CHECK(status IN ('draft','submitted','approved','rejected','paid')),
    submitted_at TEXT,
    approved_by TEXT,
    approved_at TEXT,
    rejected_reason TEXT,
    created_at TEXT NOT NULL DEFAULT (datetime('now'))
);
