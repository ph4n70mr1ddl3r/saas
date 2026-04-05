CREATE TABLE receipts (
    id TEXT PRIMARY KEY,
    invoice_id TEXT NOT NULL REFERENCES ar_invoices(id),
    customer_id TEXT NOT NULL REFERENCES customers(id),
    amount_cents INTEGER NOT NULL,
    receipt_date TEXT NOT NULL,
    method TEXT NOT NULL DEFAULT 'wire',
    created_at TEXT NOT NULL DEFAULT (datetime('now'))
);
