CREATE TABLE IF NOT EXISTS leave_balances (
    id              TEXT PRIMARY KEY,
    employee_id     TEXT NOT NULL,
    leave_type      TEXT NOT NULL,
    entitled        REAL NOT NULL DEFAULT 0,
    used            REAL NOT NULL DEFAULT 0,
    remaining       REAL NOT NULL DEFAULT 0,
    UNIQUE(employee_id, leave_type)
);
