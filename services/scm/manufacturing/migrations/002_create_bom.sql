CREATE TABLE boms (id TEXT PRIMARY KEY, name TEXT NOT NULL, description TEXT, finished_item_id TEXT NOT NULL, quantity INTEGER NOT NULL DEFAULT 1, created_at TEXT NOT NULL DEFAULT (datetime('now')));
