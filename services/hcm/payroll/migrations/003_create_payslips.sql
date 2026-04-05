CREATE TABLE IF NOT EXISTS payslips (
    id              TEXT PRIMARY KEY,
    pay_run_id      TEXT NOT NULL REFERENCES pay_runs(id),
    employee_id     TEXT NOT NULL,
    gross_pay       INTEGER NOT NULL,
    net_pay         INTEGER NOT NULL,
    tax             INTEGER NOT NULL,
    deductions      INTEGER NOT NULL,
    status          TEXT NOT NULL DEFAULT 'pending',
    created_at      TEXT NOT NULL DEFAULT (datetime('now'))
);
