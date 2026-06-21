-- Persist the activity log of each edit pipeline run (stored as a JSON array of
-- strings) so the history view can show what happened on past runs.
ALTER TABLE production_edits ADD COLUMN logs TEXT;
