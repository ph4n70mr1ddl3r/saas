CREATE TABLE IF NOT EXISTS pay_runs (
    id              TEXT PRIMARY KEY,
    period_start    TEXT NOT NULL,
    period_end      TEXT NOT NULL,
    pay_date        TEXT NOT NULL,
    status          TEXT NOT NULL DEFAULT 'draft' CHECK(status IN ('draft','processing','completed','failed')),
    created_at      TEXT NOT NULL DEFAULT (datetime('now'))
);
