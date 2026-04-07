-- Add 'issue' to stock_movements movement_type check constraint
CREATE TABLE stock_movements_new (
    id TEXT PRIMARY KEY,
    item_id TEXT NOT NULL REFERENCES items(id),
    from_warehouse_id TEXT REFERENCES warehouses(id),
    to_warehouse_id TEXT NOT NULL REFERENCES warehouses(id),
    quantity INTEGER NOT NULL,
    movement_type TEXT NOT NULL CHECK(movement_type IN ('receipt','transfer','pick','adjustment','return','issue')),
    reference_type TEXT,
    reference_id TEXT,
    created_at TEXT NOT NULL DEFAULT (datetime('now'))
);

INSERT INTO stock_movements_new SELECT * FROM stock_movements;

DROP TABLE stock_movements;

ALTER TABLE stock_movements_new RENAME TO stock_movements;
