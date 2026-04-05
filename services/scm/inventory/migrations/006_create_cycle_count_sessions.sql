CREATE TABLE IF NOT EXISTS cycle_count_sessions (
    id TEXT PRIMARY KEY,
    warehouse_id TEXT NOT NULL,
    status TEXT NOT NULL DEFAULT 'draft' CHECK(status IN ('draft','submitted','approved','posted')),
    count_date TEXT NOT NULL,
    counted_by TEXT NOT NULL,
    approved_by TEXT,
    approved_at TEXT,
    created_at TEXT NOT NULL DEFAULT (datetime('now'))
);
