CREATE TABLE IF NOT EXISTS benefit_plans (
    id                          TEXT PRIMARY KEY,
    name                        TEXT NOT NULL,
    plan_type                   TEXT NOT NULL CHECK(plan_type IN ('medical','dental','vision','life','retirement')),
    description                 TEXT,
    employer_contribution_cents INTEGER NOT NULL DEFAULT 0,
    employee_contribution_cents INTEGER NOT NULL DEFAULT 0,
    is_active                   INTEGER NOT NULL DEFAULT 1,
    created_at                  TEXT NOT NULL DEFAULT (datetime('now'))
);
