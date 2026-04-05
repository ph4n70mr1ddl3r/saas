CREATE TABLE assets (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    description TEXT,
    asset_number TEXT NOT NULL UNIQUE,
    category TEXT NOT NULL,
    purchase_date TEXT NOT NULL,
    purchase_cost_cents INTEGER NOT NULL,
    salvage_value_cents INTEGER NOT NULL DEFAULT 0,
    useful_life_months INTEGER NOT NULL,
    depreciation_method TEXT NOT NULL DEFAULT 'straight_line',
    status TEXT NOT NULL DEFAULT 'active' CHECK(status IN ('active','disposed')),
    created_at TEXT NOT NULL DEFAULT (datetime('now'))
);
