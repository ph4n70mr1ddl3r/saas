CREATE TABLE journal_entries (
    id TEXT PRIMARY KEY,
    entry_number TEXT NOT NULL UNIQUE,
    description TEXT,
    period_id TEXT NOT NULL REFERENCES periods(id),
    status TEXT NOT NULL DEFAULT 'draft' CHECK(status IN ('draft','posted','reversed')),
    posted_at TEXT,
    created_by TEXT NOT NULL,
    created_at TEXT NOT NULL DEFAULT (datetime('now'))
);
