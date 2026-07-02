-- Publishing metadata for productions: the kind of production (long-form vs
-- short-form, which tells us the destination: YouTube vs Reels/Shorts/TikTok)
-- and the date it was actually published.
ALTER TABLE productions ADD COLUMN production_type TEXT NOT NULL DEFAULT 'long';
ALTER TABLE productions ADD COLUMN published_at TIMESTAMP;
