use diesel::prelude::*;
use diesel::sqlite::SqliteConnection;

use crate::models::*;
use crate::schema::{videos, metadata, productions, tags, video_tags, video_productions};

/// Get a single video with all relationships
pub fn get_video(conn: &mut SqliteConnection, video_id: i32) -> Option<VideoFull> {
    let video: Video = videos::table
        .find(video_id)
        .first(conn)
        .ok()?;

    Some(load_video_full(conn, video))
}

#[allow(dead_code)]
/// Get a video by its file path
pub fn get_video_by_path(conn: &mut SqliteConnection, file_path: &str) -> Option<Video> {
    videos::table
        .filter(videos::file_path.eq(file_path))
        .first(conn)
        .ok()
}

/// Load full video data with metadata, tags, and productions
pub fn load_video_full(conn: &mut SqliteConnection, video: Video) -> VideoFull {
    let meta: Option<Metadata> = metadata::table
        .filter(metadata::video_id.eq(video.id))
        .first(conn)
        .ok();

    let tag_list: Vec<Tag> = video_tags::table
        .inner_join(tags::table)
        .filter(video_tags::video_id.eq(video.id))
        .select(tags::all_columns)
        .load(conn)
        .unwrap_or_default();

    let prod_list: Vec<Production> = video_productions::table
        .inner_join(crate::schema::productions::table)
        .filter(video_productions::video_id.eq(video.id))
        .select(crate::schema::productions::all_columns)
        .load(conn)
        .unwrap_or_default();

    let orientation = orientation_from_resolution(&video.resolution);

    VideoFull {
        id: video.id,
        file_path: video.file_path,
        filename: video.filename,
        duration: video.duration,
        file_size: video.file_size,
        resolution: video.resolution,
        fps: video.fps,
        codec: video.codec,
        created_date: video.created_date,
        indexed_date: video.indexed_date,
        thumbnail_count: video.thumbnail_count,
        checksum: video.checksum,
        orientation,
        metadata: meta.map(|m| MetadataResponse {
            category: m.category,
            location: m.location,
            notes: m.notes,
        }),
        tags: tag_list.into_iter().map(|t| TagResponse {
            id: t.id,
            name: t.name,
        }).collect(),
        productions: prod_list.into_iter().map(|p| ProductionBriefResponse {
            id: p.id,
            title: p.title,
            platform: p.platform,
            link: p.link,
            is_published: p.is_published,
        }).collect(),
    }
}

/// Load full data for multiple videos
pub fn load_videos_full(conn: &mut SqliteConnection, video_list: Vec<Video>) -> Vec<VideoFull> {
    video_list.into_iter()
        .map(|v| load_video_full(conn, v))
        .collect()
}

/// Update a video's metadata, tags, and productions
pub fn update_video(
    conn: &mut SqliteConnection,
    video_id: i32,
    update: &VideoUpdate,
) -> Option<VideoFull> {
    // Verify video exists
    let _video: Video = videos::table.find(video_id).first(conn).ok()?;

    // Ensure metadata record exists
    ensure_metadata_exists(conn, video_id);

    // Update metadata fields
    if update.category.is_some() || update.location.is_some() || update.notes.is_some() {
        let changeset = MetadataChangeset {
            category: update.category.clone(),
            location: update.location.clone(),
            notes: update.notes.clone(),
        };
        diesel::update(metadata::table.filter(metadata::video_id.eq(video_id)))
            .set(&changeset)
            .execute(conn)
            .ok();
    }

    // Replace tags if provided
    if let Some(ref tag_names) = update.tags {
        update_video_tags(conn, video_id, tag_names);
    }

    // Replace productions if provided
    if let Some(ref prod_ids) = update.production_ids {
        update_video_productions(conn, video_id, prod_ids);
    }

    get_video(conn, video_id)
}

/// Why a video could not be deleted.
pub enum DeleteVideoError {
    NotFound,
    /// The video is linked to one or more productions (their titles).
    UsedInProductions(Vec<String>),
    Db(String),
}

/// What happened on disk during a successful delete.
pub struct DeleteVideoOutcome {
    /// True when the source file was removed (or was already gone).
    pub file_deleted: bool,
    /// Set when file removal was requested but failed (e.g. file in use).
    pub file_error: Option<String>,
}

/// Delete a video from the library. Refuses when the video is used in any
/// production — those are part of an edit history and must be unlinked first.
/// DB relationships cascade; the thumbnail folder is removed unless a
/// duplicate video shares the same checksum. With `delete_file`, the source
/// file is also removed from disk (best-effort — the DB row is already gone,
/// so a locked file is reported rather than rolling back).
pub fn delete_video_checked(
    conn: &mut SqliteConnection,
    video_id: i32,
    delete_file: bool,
    thumbnail_dir: &std::path::Path,
) -> Result<DeleteVideoOutcome, DeleteVideoError> {
    let video: Video = videos::table
        .find(video_id)
        .first(conn)
        .map_err(|_| DeleteVideoError::NotFound)?;

    let used_in: Vec<String> = video_productions::table
        .inner_join(productions::table)
        .filter(video_productions::video_id.eq(video_id))
        .select(productions::title)
        .load(conn)
        .map_err(|e| DeleteVideoError::Db(e.to_string()))?;
    if !used_in.is_empty() {
        return Err(DeleteVideoError::UsedInProductions(used_in));
    }

    diesel::delete(videos::table.find(video_id))
        .execute(conn)
        .map_err(|e| DeleteVideoError::Db(e.to_string()))?;

    // Thumbnails are stored per-checksum; only remove the folder when no
    // other (duplicate) video still points at it.
    if let Some(checksum) = video.checksum.as_deref().filter(|c| !c.is_empty()) {
        let still_shared: i64 = videos::table
            .filter(videos::checksum.eq(checksum))
            .count()
            .get_result(conn)
            .unwrap_or(0);
        if still_shared == 0 {
            let _ = std::fs::remove_dir_all(thumbnail_dir.join(checksum));
        }
    }

    let mut outcome = DeleteVideoOutcome { file_deleted: false, file_error: None };
    if delete_file {
        let path = std::path::Path::new(&video.file_path);
        if !path.exists() {
            outcome.file_deleted = true; // already gone from disk
        } else {
            match std::fs::remove_file(path) {
                Ok(()) => outcome.file_deleted = true,
                Err(e) => outcome.file_error = Some(e.to_string()),
            }
        }
    }
    Ok(outcome)
}

/// Bulk update multiple videos
pub fn bulk_update_videos(
    conn: &mut SqliteConnection,
    bulk: &BulkUpdateRequest,
) -> Result<i64, String> {
    let video_list: Vec<Video> = videos::table
        .filter(videos::id.eq_any(&bulk.video_ids))
        .load(conn)
        .map_err(|e| e.to_string())?;

    let count = video_list.len() as i64;

    for video in &video_list {
        ensure_metadata_exists(conn, video.id);

        // Update fields
        if bulk.category.is_some() || bulk.location.is_some() || bulk.notes.is_some() {
            let changeset = MetadataChangeset {
                category: bulk.category.clone(),
                location: bulk.location.clone(),
                notes: bulk.notes.clone(),
            };
            diesel::update(metadata::table.filter(metadata::video_id.eq(video.id)))
                .set(&changeset)
                .execute(conn)
                .ok();
        }

        // Add tags
        if let Some(ref tag_names) = bulk.add_tags {
            add_tags_to_video(conn, video.id, tag_names);
        }

        // Remove tags
        if let Some(ref tag_names) = bulk.remove_tags {
            remove_tags_from_video(conn, video.id, tag_names);
        }

        // Add productions
        if let Some(ref prod_ids) = bulk.add_production_ids {
            add_productions_to_video(conn, video.id, prod_ids);
        }

        // Remove productions
        if let Some(ref prod_ids) = bulk.remove_production_ids {
            remove_productions_from_video(conn, video.id, prod_ids);
        }
    }

    Ok(count)
}

// --- Tag helpers ---

fn get_or_create_tag(conn: &mut SqliteConnection, tag_name: &str) -> i32 {
    if let Ok(tag) = tags::table
        .filter(tags::name.eq(tag_name))
        .first::<Tag>(conn)
    {
        return tag.id;
    }

    diesel::insert_into(tags::table)
        .values(&NewTag { name: tag_name.to_string() })
        .execute(conn)
        .ok();

    tags::table
        .filter(tags::name.eq(tag_name))
        .first::<Tag>(conn)
        .map(|t| t.id)
        .unwrap_or(0)
}

fn update_video_tags(conn: &mut SqliteConnection, video_id: i32, tag_names: &[String]) {
    // Delete all existing tags for this video
    diesel::delete(video_tags::table.filter(video_tags::video_id.eq(video_id)))
        .execute(conn)
        .ok();

    // Add new tags
    for name in tag_names {
        let tag_id = get_or_create_tag(conn, name);
        diesel::insert_into(video_tags::table)
            .values(&NewVideoTag { video_id, tag_id })
            .execute(conn)
            .ok();
    }
}

fn add_tags_to_video(conn: &mut SqliteConnection, video_id: i32, tag_names: &[String]) {
    let existing_tag_ids: Vec<i32> = video_tags::table
        .filter(video_tags::video_id.eq(video_id))
        .select(video_tags::tag_id)
        .load(conn)
        .unwrap_or_default();

    for name in tag_names {
        let tag_id = get_or_create_tag(conn, name);
        if !existing_tag_ids.contains(&tag_id) {
            diesel::insert_into(video_tags::table)
                .values(&NewVideoTag { video_id, tag_id })
                .execute(conn)
                .ok();
        }
    }
}

fn remove_tags_from_video(conn: &mut SqliteConnection, video_id: i32, tag_names: &[String]) {
    let tag_ids: Vec<i32> = tags::table
        .filter(tags::name.eq_any(tag_names))
        .select(tags::id)
        .load(conn)
        .unwrap_or_default();

    diesel::delete(
        video_tags::table
            .filter(video_tags::video_id.eq(video_id))
            .filter(video_tags::tag_id.eq_any(&tag_ids)),
    )
    .execute(conn)
    .ok();
}

// --- Production helpers ---

fn update_video_productions(conn: &mut SqliteConnection, video_id: i32, prod_ids: &[i32]) {
    diesel::delete(video_productions::table.filter(video_productions::video_id.eq(video_id)))
        .execute(conn)
        .ok();

    for &prod_id in prod_ids {
        diesel::insert_into(video_productions::table)
            .values(&NewVideoProduction { video_id, production_id: prod_id })
            .execute(conn)
            .ok();
    }
}

fn add_productions_to_video(conn: &mut SqliteConnection, video_id: i32, prod_ids: &[i32]) {
    let existing: Vec<i32> = video_productions::table
        .filter(video_productions::video_id.eq(video_id))
        .select(video_productions::production_id)
        .load(conn)
        .unwrap_or_default();

    for &prod_id in prod_ids {
        if !existing.contains(&prod_id) {
            diesel::insert_into(video_productions::table)
                .values(&NewVideoProduction { video_id, production_id: prod_id })
                .execute(conn)
                .ok();
        }
    }
}

fn remove_productions_from_video(conn: &mut SqliteConnection, video_id: i32, prod_ids: &[i32]) {
    diesel::delete(
        video_productions::table
            .filter(video_productions::video_id.eq(video_id))
            .filter(video_productions::production_id.eq_any(prod_ids)),
    )
    .execute(conn)
    .ok();
}

// --- Metadata helpers ---

fn ensure_metadata_exists(conn: &mut SqliteConnection, video_id: i32) {
    let exists = metadata::table
        .filter(metadata::video_id.eq(video_id))
        .count()
        .get_result::<i64>(conn)
        .unwrap_or(0) > 0;

    if !exists {
        diesel::insert_into(metadata::table)
            .values(&NewMetadata {
                video_id,
                category: None,
                location: None,
                notes: None,
            })
            .execute(conn)
            .ok();
    }
}
