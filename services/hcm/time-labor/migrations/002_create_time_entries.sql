CREATE TABLE IF NOT EXISTS time_entries (
    id              TEXT PRIMARY KEY,
    timesheet_id    TEXT NOT NULL REFERENCES timesheets(id),
    date            TEXT NOT NULL,
    hours           REAL NOT NULL,
    project_code    TEXT,
    description     TEXT,
    created_at      TEXT NOT NULL DEFAULT (datetime('now'))
);
