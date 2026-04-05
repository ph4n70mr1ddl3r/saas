CREATE TABLE warehouses (id TEXT PRIMARY KEY, name TEXT NOT NULL, address TEXT, is_active INTEGER NOT NULL DEFAULT 1, created_at TEXT NOT NULL DEFAULT (datetime('now')));
