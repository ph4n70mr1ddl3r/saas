CREATE TABLE IF NOT EXISTS leave_requests (
    id              TEXT PRIMARY KEY,
    employee_id     TEXT NOT NULL,
    leave_type      TEXT NOT NULL CHECK(leave_type IN ('vacation','sick','personal')),
    start_date      TEXT NOT NULL,
    end_date        TEXT NOT NULL,
    status          TEXT NOT NULL DEFAULT 'pending' CHECK(status IN ('pending','approved','rejected')),
    reason          TEXT,
    created_at      TEXT NOT NULL DEFAULT (datetime('now'))
);
