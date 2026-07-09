-- Visual description for semantic search: a short caption + tags describing what
-- a video *shows* (setting, subjects, activity), produced by a vision LLM from
-- the video's thumbnails. Lets semantic search match visual content ("running in
-- the snow") that the transcript never mentions. Stored alongside the other
-- per-video AI-derived text.
ALTER TABLE ai_generations ADD COLUMN visual_description TEXT;
