-- Persist the thumbnail builder state for a run (text, font, colors, position,
-- alignment, frame time, and the AI text-style treatment) as JSON so the
-- thumbnail can be rebuilt/edited after the modal is closed and reopened.
ALTER TABLE production_edits ADD COLUMN thumbnail_json TEXT;
