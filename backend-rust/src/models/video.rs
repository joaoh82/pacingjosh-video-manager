use chrono::NaiveDateTime;
use diesel::prelude::*;
use serde::{Deserialize, Serialize};

use crate::schema::{videos, metadata};

#[derive(Debug, Queryable, Selectable, Identifiable, Serialize, Clone)]
#[diesel(table_name = videos)]
pub struct Video {
    pub id: i32,
    pub file_path: String,
    pub filename: String,
    pub duration: Option<f32>,
    pub file_size: Option<i64>,
    pub resolution: Option<String>,
    pub fps: Option<f32>,
    pub codec: Option<String>,
    pub created_date: Option<NaiveDateTime>,
    pub indexed_date: NaiveDateTime,
    pub thumbnail_count: i32,
    pub checksum: Option<String>,
}

#[derive(Debug, Insertable)]
#[diesel(table_name = videos)]
pub struct NewVideo {
    pub file_path: String,
    pub filename: String,
    pub duration: Option<f32>,
    pub file_size: Option<i64>,
    pub resolution: Option<String>,
    pub fps: Option<f32>,
    pub codec: Option<String>,
    pub created_date: Option<NaiveDateTime>,
    pub indexed_date: NaiveDateTime,
    pub thumbnail_count: i32,
    pub checksum: Option<String>,
}

#[derive(Debug, Queryable, Selectable, Identifiable, Associations, Serialize, Clone)]
#[diesel(table_name = metadata)]
#[diesel(belongs_to(Video, foreign_key = video_id))]
pub struct Metadata {
    pub id: i32,
    pub video_id: i32,
    pub category: Option<String>,
    pub location: Option<String>,
    pub notes: Option<String>,
}

#[derive(Debug, Insertable)]
#[diesel(table_name = metadata)]
pub struct NewMetadata {
    pub video_id: i32,
    pub category: Option<String>,
    pub location: Option<String>,
    pub notes: Option<String>,
}

#[derive(Debug, AsChangeset)]
#[diesel(table_name = metadata)]
pub struct MetadataChangeset {
    pub category: Option<String>,
    pub location: Option<String>,
    pub notes: Option<String>,
}

/// Full video data with all relationships loaded
#[derive(Debug, Serialize, Clone)]
pub struct VideoFull {
    pub id: i32,
    pub file_path: String,
    pub filename: String,
    pub duration: Option<f32>,
    pub file_size: Option<i64>,
    pub resolution: Option<String>,
    pub fps: Option<f32>,
    pub codec: Option<String>,
    pub created_date: Option<NaiveDateTime>,
    pub indexed_date: NaiveDateTime,
    pub thumbnail_count: i32,
    pub checksum: Option<String>,
    pub metadata: Option<MetadataResponse>,
    pub tags: Vec<TagResponse>,
    pub productions: Vec<ProductionBriefResponse>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct MetadataResponse {
    pub category: Option<String>,
    pub location: Option<String>,
    pub notes: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct TagResponse {
    pub id: i32,
    pub name: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ProductionBriefResponse {
    pub id: i32,
    pub title: String,
    pub platform: Option<String>,
    pub link: Option<String>,
    pub is_published: bool,
}

#[derive(Debug, Deserialize)]
pub struct VideoUpdate {
    pub category: Option<String>,
    pub location: Option<String>,
    pub notes: Option<String>,
    pub tags: Option<Vec<String>>,
    pub production_ids: Option<Vec<i32>>,
}

#[derive(Debug, Serialize)]
pub struct VideoListResponse {
    pub videos: Vec<VideoFull>,
    pub total: i64,
    pub page: i64,
    pub limit: i64,
    pub pages: i64,
}

#[derive(Debug, Deserialize)]
pub struct BulkUpdateRequest {
    pub video_ids: Vec<i32>,
    pub category: Option<String>,
    pub location: Option<String>,
    pub notes: Option<String>,
    pub add_tags: Option<Vec<String>>,
    pub remove_tags: Option<Vec<String>>,
    pub add_production_ids: Option<Vec<i32>>,
    pub remove_production_ids: Option<Vec<i32>>,
}

#[derive(Debug, Serialize)]
pub struct BulkUpdateResponse {
    pub updated: i64,
    pub message: String,
}
