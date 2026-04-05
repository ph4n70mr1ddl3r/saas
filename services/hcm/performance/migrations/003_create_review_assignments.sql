CREATE TABLE IF NOT EXISTS review_assignments (
    id TEXT PRIMARY KEY,
    cycle_id TEXT NOT NULL REFERENCES review_cycles(id),
    reviewer_id TEXT NOT NULL,
    employee_id TEXT NOT NULL,
    status TEXT NOT NULL DEFAULT 'pending' CHECK(status IN ('pending','completed')),
    rating INTEGER CHECK(rating IS NULL OR (rating >= 1 AND rating <= 5)),
    comments TEXT,
    submitted_at TEXT,
    created_at TEXT NOT NULL DEFAULT (datetime('now'))
);
