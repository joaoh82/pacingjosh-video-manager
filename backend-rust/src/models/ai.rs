use chrono::NaiveDateTime;
use diesel::prelude::*;
use serde::{Deserialize, Serialize};

use crate::schema::ai_generations;

/// Raw `ai_generations` row. `thumbnail_text` and `hashtags` are stored as JSON
/// arrays of strings; all other text fields are plain text.
#[derive(Debug, Queryable, Selectable, Identifiable, Clone)]
#[diesel(table_name = ai_generations)]
pub struct AiGeneration {
    pub id: i32,
    pub video_id: i32,
    pub transcript: Option<String>,
    pub thumbnail_text: Option<String>,
    pub instagram_description: Option<String>,
    pub tiktok_description: Option<String>,
    pub youtube_short_title: Option<String>,
    pub youtube_short_description: Option<String>,
    pub youtube_short_tags: Option<String>,
    pub hashtags: Option<String>,
    pub provider: Option<String>,
    pub model: Option<String>,
    pub generated_at: NaiveDateTime,
}

#[derive(Debug, Insertable, AsChangeset)]
#[diesel(table_name = ai_generations)]
pub struct NewAiGeneration {
    pub video_id: i32,
    pub transcript: Option<String>,
    pub thumbnail_text: Option<String>,
    pub instagram_description: Option<String>,
    pub tiktok_description: Option<String>,
    pub youtube_short_title: Option<String>,
    pub youtube_short_description: Option<String>,
    pub youtube_short_tags: Option<String>,
    pub hashtags: Option<String>,
    pub provider: Option<String>,
    pub model: Option<String>,
    pub generated_at: NaiveDateTime,
}

/// API-facing shape with JSON-array columns decoded into `Vec<String>`.
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct AiGenerationResponse {
    pub video_id: i32,
    pub transcript: Option<String>,
    pub thumbnail_text: Vec<String>,
    pub instagram_description: Option<String>,
    pub tiktok_description: Option<String>,
    pub youtube_short_title: Option<String>,
    pub youtube_short_description: Option<String>,
    pub youtube_short_tags: Vec<String>,
    pub hashtags: Vec<String>,
    pub provider: Option<String>,
    pub model: Option<String>,
    pub generated_at: NaiveDateTime,
}

impl From<AiGeneration> for AiGenerationResponse {
    fn from(g: AiGeneration) -> Self {
        let thumbnail_text = g
            .thumbnail_text
            .as_deref()
            .and_then(|s| serde_json::from_str::<Vec<String>>(s).ok())
            .unwrap_or_default();
        let youtube_short_tags = g
            .youtube_short_tags
            .as_deref()
            .and_then(|s| serde_json::from_str::<Vec<String>>(s).ok())
            .unwrap_or_default();
        let hashtags = g
            .hashtags
            .as_deref()
            .and_then(|s| serde_json::from_str::<Vec<String>>(s).ok())
            .unwrap_or_default();

        Self {
            video_id: g.video_id,
            transcript: g.transcript,
            thumbnail_text,
            instagram_description: g.instagram_description,
            tiktok_description: g.tiktok_description,
            youtube_short_title: g.youtube_short_title,
            youtube_short_description: g.youtube_short_description,
            youtube_short_tags,
            hashtags,
            provider: g.provider,
            model: g.model,
            generated_at: g.generated_at,
        }
    }
}
