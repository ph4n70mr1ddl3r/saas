CREATE TABLE IF NOT EXISTS deductions (
    id              TEXT PRIMARY KEY,
    employee_id     TEXT NOT NULL,
    code            TEXT NOT NULL,
    amount_cents    INTEGER NOT NULL,
    recurring       INTEGER NOT NULL DEFAULT 1,
    start_date      TEXT NOT NULL,
    end_date        TEXT
);
