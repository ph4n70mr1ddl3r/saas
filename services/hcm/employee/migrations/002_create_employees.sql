CREATE TABLE IF NOT EXISTS employees (
    id               TEXT PRIMARY KEY,
    first_name       TEXT NOT NULL,
    last_name        TEXT NOT NULL,
    email            TEXT NOT NULL UNIQUE,
    phone            TEXT,
    hire_date        TEXT NOT NULL,
    termination_date TEXT,
    status           TEXT NOT NULL DEFAULT 'active' CHECK (status IN ('active','on_leave','terminated')),
    department_id    TEXT NOT NULL REFERENCES departments(id),
    reports_to       TEXT REFERENCES employees(id),
    job_title        TEXT NOT NULL,
    employee_number  TEXT NOT NULL UNIQUE,
    created_at       TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at       TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE INDEX IF NOT EXISTS idx_employees_department ON employees(department_id);
CREATE INDEX IF NOT EXISTS idx_employees_reports_to ON employees(reports_to);
CREATE INDEX IF NOT EXISTS idx_employees_status ON employees(status);
