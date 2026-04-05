CREATE TABLE ap_invoice_lines (
    id TEXT PRIMARY KEY,
    invoice_id TEXT NOT NULL REFERENCES ap_invoices(id),
    description TEXT,
    account_code TEXT NOT NULL,
    amount_cents INTEGER NOT NULL
);
