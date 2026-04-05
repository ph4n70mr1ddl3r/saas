CREATE TABLE IF NOT EXISTS timesheets (
    id              TEXT PRIMARY KEY,
    employee_id     TEXT NOT NULL,
    week_start      TEXT NOT NULL,
    status          TEXT NOT NULL DEFAULT 'draft' CHECK(status IN ('draft','submitted','approved','rejected')),
    total_hours     REAL NOT NULL DEFAULT 0,
    submitted_at    TEXT,
    approved_at     TEXT,
    created_at      TEXT NOT NULL DEFAULT (datetime('now'))
);
