-- YouTube Shorts also need a title and keyword tags, not just a description.
-- `youtube_short_title` is plain text; `youtube_short_tags` is a JSON array of
-- keyword strings (mirrors how `hashtags` is stored).
ALTER TABLE ai_generations ADD COLUMN youtube_short_title TEXT;
ALTER TABLE ai_generations ADD COLUMN youtube_short_tags TEXT;
