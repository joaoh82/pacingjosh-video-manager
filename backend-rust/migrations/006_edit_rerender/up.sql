-- Persist what a re-render needs so it can reuse a run without re-transcribing
-- or re-planning: the per-take word transcripts and the render options.
ALTER TABLE production_edits ADD COLUMN transcripts_json TEXT;
ALTER TABLE production_edits ADD COLUMN options_json TEXT;
