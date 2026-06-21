-- Persist the generated YouTube copy (titles, description, tags, thumbnail
-- text) for a run, stored as JSON.
ALTER TABLE production_edits ADD COLUMN copy_json TEXT;
