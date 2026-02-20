CREATE TABLE videos (
    id INTEGER NOT NULL PRIMARY KEY AUTOINCREMENT,
    file_path TEXT NOT NULL UNIQUE,
    filename TEXT NOT NULL,
    duration REAL,
    file_size BIGINT,
    resolution TEXT,
    fps REAL,
    codec TEXT,
    created_date TIMESTAMP,
    indexed_date TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    thumbnail_count INTEGER NOT NULL DEFAULT 0,
    checksum TEXT
);

CREATE INDEX idx_videos_created_date ON videos (created_date);
CREATE INDEX idx_videos_filename ON videos (filename);
CREATE INDEX idx_videos_checksum ON videos (checksum);

-- For existing databases: add checksum column if table already exists
ALTER TABLE videos ADD COLUMN checksum TEXT;

CREATE TABLE metadata (
    id INTEGER NOT NULL PRIMARY KEY AUTOINCREMENT,
    video_id INTEGER NOT NULL UNIQUE REFERENCES videos(id) ON DELETE CASCADE,
    category TEXT,
    location TEXT,
    notes TEXT
);

CREATE INDEX idx_metadata_category ON metadata (category);

CREATE TABLE tags (
    id INTEGER NOT NULL PRIMARY KEY AUTOINCREMENT,
    name TEXT NOT NULL UNIQUE
);

CREATE INDEX idx_tags_name ON tags (name);

CREATE TABLE video_tags (
    video_id INTEGER NOT NULL REFERENCES videos(id) ON DELETE CASCADE,
    tag_id INTEGER NOT NULL REFERENCES tags(id) ON DELETE CASCADE,
    PRIMARY KEY (video_id, tag_id)
);

CREATE INDEX idx_video_tags_video ON video_tags (video_id);
CREATE INDEX idx_video_tags_tag ON video_tags (tag_id);

CREATE TABLE productions (
    id INTEGER NOT NULL PRIMARY KEY AUTOINCREMENT,
    title TEXT NOT NULL UNIQUE,
    platform TEXT,
    link TEXT,
    is_published BOOLEAN NOT NULL DEFAULT 0
);

CREATE INDEX idx_productions_title ON productions (title);

CREATE TABLE video_productions (
    video_id INTEGER NOT NULL REFERENCES videos(id) ON DELETE CASCADE,
    production_id INTEGER NOT NULL REFERENCES productions(id) ON DELETE CASCADE,
    PRIMARY KEY (video_id, production_id)
);

CREATE INDEX idx_video_productions_video ON video_productions (video_id);
CREATE INDEX idx_video_productions_production ON video_productions (production_id);
