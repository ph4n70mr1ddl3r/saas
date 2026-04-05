CREATE TABLE IF NOT EXISTS tax_codes (
    id TEXT PRIMARY KEY,
    code TEXT NOT NULL UNIQUE,
    rate REAL NOT NULL CHECK(rate >= 0 AND rate <= 1),
    description TEXT,
    is_active INTEGER NOT NULL DEFAULT 1,
    created_at TEXT NOT NULL DEFAULT (datetime('now'))
);
