CREATE TABLE IF NOT EXISTS cycle_count_lines (
    id TEXT PRIMARY KEY,
    session_id TEXT NOT NULL REFERENCES cycle_count_sessions(id),
    item_id TEXT NOT NULL REFERENCES items(id),
    system_quantity INTEGER NOT NULL,
    counted_quantity INTEGER,
    variance INTEGER,
    notes TEXT,
    created_at TEXT NOT NULL DEFAULT (datetime('now'))
);
