-- Semantic search embeddings for videos and productions.
--
-- Each row stores a single embedding vector for the entity's searchable
-- "document" (filename + metadata + tags + transcript for videos; title +
-- script + take transcripts + copy for productions). The vector is a packed
-- little-endian f32 array in the `embedding` BLOB. `content_hash` lets a
-- re-index skip documents whose text is unchanged, and `model` records which
-- provider:model produced the vector so a provider/model change forces a
-- re-embed (and query-time comparisons only mix vectors from the same model).
CREATE TABLE IF NOT EXISTS video_embeddings (
    video_id INTEGER PRIMARY KEY REFERENCES videos(id) ON DELETE CASCADE,
    content_hash TEXT NOT NULL,
    model TEXT NOT NULL,
    dim INTEGER NOT NULL,
    embedding BLOB NOT NULL,
    updated_at TIMESTAMP NOT NULL
);

CREATE TABLE IF NOT EXISTS production_embeddings (
    production_id INTEGER PRIMARY KEY REFERENCES productions(id) ON DELETE CASCADE,
    content_hash TEXT NOT NULL,
    model TEXT NOT NULL,
    dim INTEGER NOT NULL,
    embedding BLOB NOT NULL,
    updated_at TIMESTAMP NOT NULL
);
