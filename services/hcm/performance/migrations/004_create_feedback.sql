CREATE TABLE IF NOT EXISTS feedback (
    id TEXT PRIMARY KEY,
    cycle_id TEXT NOT NULL REFERENCES review_cycles(id),
    from_employee_id TEXT NOT NULL,
    to_employee_id TEXT NOT NULL,
    content TEXT NOT NULL,
    is_anonymous INTEGER NOT NULL DEFAULT 0,
    created_at TEXT NOT NULL DEFAULT (datetime('now'))
);
