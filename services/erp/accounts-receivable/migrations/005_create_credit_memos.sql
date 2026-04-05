CREATE TABLE IF NOT EXISTS credit_memos (
    id TEXT PRIMARY KEY,
    customer_id TEXT NOT NULL,
    amount_cents INTEGER NOT NULL CHECK(amount_cents > 0),
    reason TEXT,
    status TEXT NOT NULL DEFAULT 'open' CHECK(status IN ('open','applied','cancelled')),
    applied_to_invoice_id TEXT,
    applied_amount_cents INTEGER NOT NULL DEFAULT 0,
    created_at TEXT NOT NULL DEFAULT (datetime('now'))
);
