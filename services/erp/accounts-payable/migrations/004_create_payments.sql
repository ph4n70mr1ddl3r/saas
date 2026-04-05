CREATE TABLE payments (
    id TEXT PRIMARY KEY,
    invoice_id TEXT NOT NULL REFERENCES ap_invoices(id),
    vendor_id TEXT NOT NULL REFERENCES vendors(id),
    amount_cents INTEGER NOT NULL,
    payment_date TEXT NOT NULL,
    method TEXT NOT NULL DEFAULT 'wire',
    reference TEXT,
    created_at TEXT NOT NULL DEFAULT (datetime('now'))
);
