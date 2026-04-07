ALTER TABLE journal_entries ADD COLUMN reversal_of TEXT REFERENCES journal_entries(id);
