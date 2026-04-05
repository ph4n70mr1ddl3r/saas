CREATE TABLE IF NOT EXISTS job_postings (
    id              TEXT PRIMARY KEY,
    title           TEXT NOT NULL,
    department_id   TEXT NOT NULL,
    description     TEXT,
    requirements    TEXT,
    status          TEXT NOT NULL DEFAULT 'open' CHECK(status IN ('open','closed','filled')),
    posted_at       TEXT NOT NULL DEFAULT (datetime('now')),
    closed_at       TEXT
);
