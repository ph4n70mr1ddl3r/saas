CREATE TABLE ar_invoice_lines (
    id TEXT PRIMARY KEY,
    invoice_id TEXT NOT NULL REFERENCES ar_invoices(id),
    description TEXT,
    amount_cents INTEGER NOT NULL
);
