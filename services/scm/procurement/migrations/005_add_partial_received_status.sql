-- Add 'partially_received' status to purchase_orders check constraint
CREATE TABLE purchase_orders_new (
    id TEXT PRIMARY KEY,
    po_number TEXT NOT NULL UNIQUE,
    supplier_id TEXT NOT NULL REFERENCES suppliers(id),
    order_date TEXT NOT NULL,
    status TEXT NOT NULL DEFAULT 'draft' CHECK(status IN ('draft','submitted','approved','received','partially_received','cancelled')),
    total_cents INTEGER NOT NULL DEFAULT 0,
    created_at TEXT NOT NULL DEFAULT (datetime('now'))
);

INSERT INTO purchase_orders_new SELECT * FROM purchase_orders;

DROP TABLE purchase_orders;

ALTER TABLE purchase_orders_new RENAME TO purchase_orders;
