CREATE TABLE IF NOT EXISTS enrollments (
    id              TEXT PRIMARY KEY,
    employee_id     TEXT NOT NULL,
    plan_id         TEXT NOT NULL REFERENCES benefit_plans(id),
    status          TEXT NOT NULL DEFAULT 'active' CHECK(status IN ('active','cancelled')),
    enrolled_at     TEXT NOT NULL DEFAULT (datetime('now')),
    cancelled_at    TEXT
);
