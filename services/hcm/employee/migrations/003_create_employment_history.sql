CREATE TABLE IF NOT EXISTS employment_history (
    id TEXT PRIMARY KEY,
    employee_id TEXT NOT NULL,
    field_name TEXT NOT NULL,
    old_value TEXT,
    new_value TEXT,
    effective_date TEXT NOT NULL,
    created_at TEXT NOT NULL DEFAULT (datetime('now'))
);
