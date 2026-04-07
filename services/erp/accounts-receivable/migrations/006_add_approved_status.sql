-- Add 'approved' status to ar_invoices check constraint
-- SQLite doesn't support ALTER TABLE ... ALTER CONSTRAINT, so we recreate the table.

CREATE TABLE ar_invoices_new (
    id TEXT PRIMARY KEY,
    customer_id TEXT NOT NULL REFERENCES customers(id),
    invoice_number TEXT NOT NULL,
    invoice_date TEXT NOT NULL,
    due_date TEXT NOT NULL,
    total_cents INTEGER NOT NULL DEFAULT 0,
    status TEXT NOT NULL DEFAULT 'draft' CHECK(status IN ('draft','sent','approved','paid','cancelled','partial')),
    created_at TEXT NOT NULL DEFAULT (datetime('now'))
);

INSERT INTO ar_invoices_new SELECT * FROM ar_invoices;

DROP TABLE ar_invoices;

ALTER TABLE ar_invoices_new RENAME TO ar_invoices;
