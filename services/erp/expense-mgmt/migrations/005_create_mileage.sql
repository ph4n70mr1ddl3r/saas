CREATE TABLE IF NOT EXISTS mileage (
    id TEXT PRIMARY KEY,
    report_id TEXT NOT NULL REFERENCES expense_reports(id),
    origin TEXT NOT NULL,
    destination TEXT NOT NULL,
    distance_miles REAL NOT NULL CHECK(distance_miles > 0),
    rate_per_mile_cents INTEGER NOT NULL CHECK(rate_per_mile_cents > 0),
    total_cents INTEGER NOT NULL,
    expense_date TEXT NOT NULL,
    created_at TEXT NOT NULL DEFAULT (datetime('now'))
);
