use diesel::prelude::*;
use serde::Serialize;

use crate::schema::{tags, video_tags};

#[derive(Debug, Queryable, Selectable, Identifiable, Serialize, Clone)]
#[diesel(table_name = tags)]
pub struct Tag {
    pub id: i32,
    pub name: String,
}

#[derive(Debug, Insertable)]
#[diesel(table_name = tags)]
pub struct NewTag {
    pub name: String,
}

#[derive(Debug, Queryable, Selectable, Identifiable, Associations, Clone)]
#[diesel(table_name = video_tags)]
#[diesel(primary_key(video_id, tag_id))]
#[diesel(belongs_to(super::Video, foreign_key = video_id))]
#[diesel(belongs_to(Tag, foreign_key = tag_id))]
pub struct VideoTag {
    pub video_id: i32,
    pub tag_id: i32,
}

#[derive(Debug, Insertable)]
#[diesel(table_name = video_tags)]
pub struct NewVideoTag {
    pub video_id: i32,
    pub tag_id: i32,
}

#[derive(Debug, Serialize)]
pub struct TagWithCount {
    pub id: i32,
    pub name: String,
    pub count: i64,
}

#[derive(Debug, Serialize)]
pub struct CategoryResponse {
    pub name: String,
    pub count: i64,
}
