CREATE TABLE IF NOT EXISTS applications (
    id                    TEXT PRIMARY KEY,
    job_id                TEXT NOT NULL REFERENCES job_postings(id),
    candidate_first_name  TEXT NOT NULL,
    candidate_last_name   TEXT NOT NULL,
    candidate_email       TEXT NOT NULL,
    status                TEXT NOT NULL DEFAULT 'applied' CHECK(status IN ('applied','screening','interview','offer','hired','rejected')),
    applied_at            TEXT NOT NULL DEFAULT (datetime('now')),
    notes                 TEXT
);
