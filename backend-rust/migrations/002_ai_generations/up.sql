CREATE TABLE ai_generations (
    id INTEGER NOT NULL PRIMARY KEY AUTOINCREMENT,
    video_id INTEGER NOT NULL UNIQUE REFERENCES videos(id) ON DELETE CASCADE,
    transcript TEXT,
    thumbnail_text TEXT,
    instagram_description TEXT,
    tiktok_description TEXT,
    youtube_short_description TEXT,
    hashtags TEXT,
    provider TEXT,
    model TEXT,
    generated_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP
);

CREATE INDEX idx_ai_generations_video ON ai_generations (video_id);
