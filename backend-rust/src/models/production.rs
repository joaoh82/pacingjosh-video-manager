use diesel::prelude::*;
use serde::{Deserialize, Serialize};

use crate::schema::{productions, video_productions};

#[derive(Debug, Queryable, Selectable, Identifiable, Serialize, Clone)]
#[diesel(table_name = productions)]
pub struct Production {
    pub id: i32,
    pub title: String,
    pub platform: Option<String>,
    pub link: Option<String>,
    pub is_published: bool,
}

#[derive(Debug, Insertable)]
#[diesel(table_name = productions)]
pub struct NewProduction {
    pub title: String,
    pub platform: Option<String>,
    pub link: Option<String>,
    pub is_published: bool,
}

#[derive(Debug, AsChangeset)]
#[diesel(table_name = productions)]
pub struct ProductionChangeset {
    pub title: String,
    pub platform: Option<String>,
    pub link: Option<String>,
    pub is_published: bool,
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
}

#[derive(Debug, Serialize)]
pub struct ProductionResponse {
    pub id: i32,
    pub title: String,
    pub platform: Option<String>,
    pub link: Option<String>,
    pub is_published: bool,
    pub video_count: i64,
}
