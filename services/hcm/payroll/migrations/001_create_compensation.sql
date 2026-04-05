CREATE TABLE IF NOT EXISTS compensation (
    id              TEXT PRIMARY KEY,
    employee_id     TEXT NOT NULL,
    salary_type     TEXT NOT NULL CHECK(salary_type IN ('salaried','hourly')),
    amount_cents    INTEGER NOT NULL,
    currency        TEXT NOT NULL DEFAULT 'USD',
    effective_date  TEXT NOT NULL,
    end_date        TEXT,
    created_at      TEXT NOT NULL DEFAULT (datetime('now'))
);
