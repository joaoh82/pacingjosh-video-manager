-- Persisted results of the video-edit pipeline. Each row is one completed or
-- failed attempt to assemble a production's raw takes into a final clip. The
-- edit decision list is stored as JSON in edl_json and also written to disk at
-- edl_path, alongside the stitched video at output_path.
CREATE TABLE IF NOT EXISTS production_edits (
    id INTEGER NOT NULL PRIMARY KEY AUTOINCREMENT,
    production_id INTEGER NOT NULL REFERENCES productions(id) ON DELETE CASCADE,
    status TEXT NOT NULL,
    script TEXT,
    instructions TEXT,
    edl_json TEXT,
    output_path TEXT,
    edl_path TEXT,
    error TEXT,
    transcription_provider TEXT,
    text_provider TEXT,
    text_model TEXT,
    created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP
);

CREATE INDEX IF NOT EXISTS idx_production_edits_production ON production_edits (production_id);
