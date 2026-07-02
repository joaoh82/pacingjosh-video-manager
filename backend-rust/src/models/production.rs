use chrono::NaiveDateTime;
use diesel::prelude::*;
use serde::{Deserialize, Serialize};

use crate::schema::{productions, video_productions};

/// Normalize a production type to one of the two supported values,
/// defaulting to long-form for anything unknown.
pub fn normalize_production_type(raw: &str) -> String {
    match raw.trim().to_lowercase().as_str() {
        "short" => "short".to_string(),
        _ => "long".to_string(),
    }
}

pub fn default_production_type() -> String {
    "long".to_string()
}

#[derive(Debug, Queryable, Selectable, Identifiable, Serialize, Clone)]
#[diesel(table_name = productions)]
pub struct Production {
    pub id: i32,
    pub title: String,
    pub platform: Option<String>,
    pub link: Option<String>,
    pub is_published: bool,
    pub production_type: String,
    pub published_at: Option<NaiveDateTime>,
}

#[derive(Debug, Insertable)]
#[diesel(table_name = productions)]
pub struct NewProduction {
    pub title: String,
    pub platform: Option<String>,
    pub link: Option<String>,
    pub is_published: bool,
    pub production_type: String,
    pub published_at: Option<NaiveDateTime>,
}

#[derive(Debug, AsChangeset)]
#[diesel(table_name = productions)]
#[diesel(treat_none_as_null = true)]
pub struct ProductionChangeset {
    pub title: String,
    pub platform: Option<String>,
    pub link: Option<String>,
    pub is_published: bool,
    pub production_type: String,
    pub published_at: Option<NaiveDateTime>,
}

#[derive(Debug, Queryable, Selectable, Identifiable, Associations, Clone)]
#[diesel(table_name = video_productions)]
#[diesel(primary_key(video_id, production_id))]
#[diesel(belongs_to(super::Video, foreign_key = video_id))]
#[diesel(belongs_to(Production, foreign_key = production_id))]
pub struct VideoProduction {
    pub video_id: i32,
    pub production_id: i32,
}

#[derive(Debug, Insertable)]
#[diesel(table_name = video_productions)]
pub struct NewVideoProduction {
    pub video_id: i32,
    pub production_id: i32,
}

#[derive(Debug, Deserialize)]
pub struct ProductionCreate {
    pub title: String,
    pub platform: Option<String>,
    pub link: Option<String>,
    #[serde(default)]
    pub is_published: bool,
    /// "long" | "short" — anything else is normalized to "long".
    #[serde(default = "default_production_type")]
    pub production_type: String,
    /// Publish date as "YYYY-MM-DD" or a full ISO datetime. Omitted/empty while
    /// `is_published` is true → the server stamps the current date.
    #[serde(default)]
    pub published_at: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct ProductionResponse {
    pub id: i32,
    pub title: String,
    pub platform: Option<String>,
    pub link: Option<String>,
    pub is_published: bool,
    pub production_type: String,
    pub published_at: Option<NaiveDateTime>,
    pub video_count: i64,
}
