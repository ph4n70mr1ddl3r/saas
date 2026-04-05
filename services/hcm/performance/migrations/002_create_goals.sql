CREATE TABLE IF NOT EXISTS goals (
    id TEXT PRIMARY KEY,
    employee_id TEXT NOT NULL,
    cycle_id TEXT NOT NULL REFERENCES review_cycles(id),
    title TEXT NOT NULL,
    description TEXT,
    weight REAL NOT NULL DEFAULT 1.0 CHECK(weight > 0 AND weight <= 10),
    progress REAL NOT NULL DEFAULT 0.0 CHECK(progress >= 0 AND progress <= 100),
    status TEXT NOT NULL DEFAULT 'not_started' CHECK(status IN ('not_started','in_progress','completed','cancelled')),
    due_date TEXT,
    created_at TEXT NOT NULL DEFAULT (datetime('now'))
);
