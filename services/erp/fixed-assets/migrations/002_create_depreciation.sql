CREATE TABLE depreciation_schedule (
    id TEXT PRIMARY KEY,
    asset_id TEXT NOT NULL REFERENCES assets(id),
    period TEXT NOT NULL,
    depreciation_cents INTEGER NOT NULL,
    accumulated_cents INTEGER NOT NULL,
    net_book_value_cents INTEGER NOT NULL,
    UNIQUE(asset_id, period)
);
