use diesel::prelude::*;
use diesel::sqlite::SqliteConnection;
use chrono::NaiveDateTime;

use crate::models::*;
use crate::schema::{videos, metadata, tags, video_tags, video_productions};
use crate::services::video_service;

/// Search videos with filters, sorting, and pagination
///
/// Strategy: collect filtered video IDs first, then count + paginate + load.
/// This avoids Diesel borrow/ownership issues with boxed queries.
pub fn search_videos(
    conn: &mut SqliteConnection,
    search: Option<&str>,
    category: Option<&str>,
    tag_names: Option<Vec<String>>,
    production_id: Option<i32>,
    date_from: Option<NaiveDateTime>,
    date_to: Option<NaiveDateTime>,
    sort: &str,
    page: i64,
    limit: i64,
) -> (Vec<VideoFull>, i64) {
    // Start with all video IDs, then progressively filter
    let mut filtered_ids: Option<Vec<i32>> = None;

    // Search filter (filename, location, notes)
    if let Some(search_term) = search {
        if !search_term.is_empty() {
            let pattern = format!("%{}%", search_term);

            let filename_ids: Vec<i32> = videos::table
                .filter(videos::filename.like(&pattern))
                .select(videos::id)
                .load(conn)
                .unwrap_or_default();

            let meta_ids: Vec<i32> = metadata::table
                .filter(
                    metadata::location.like(&pattern)
                        .or(metadata::notes.like(&pattern))
                )
                .select(metadata::video_id)
                .load(conn)
                .unwrap_or_default();

            let mut combined: Vec<i32> = filename_ids;
            for id in meta_ids {
                if !combined.contains(&id) {
                    combined.push(id);
                }
            }

            filtered_ids = Some(intersect_ids(filtered_ids, combined));
        }
    }

    // Category filter
    if let Some(cat) = category {
        if !cat.is_empty() {
            let cat_ids: Vec<i32> = metadata::table
                .filter(metadata::category.eq(cat))
                .select(metadata::video_id)
                .load(conn)
                .unwrap_or_default();

            filtered_ids = Some(intersect_ids(filtered_ids, cat_ids));
        }
    }

    // Tags filter (AND logic)
    if let Some(ref tag_list) = tag_names {
        for tag_name in tag_list {
            let tag_ids: Vec<i32> = video_tags::table
                .inner_join(tags::table)
                .filter(tags::name.eq(tag_name))
                .select(video_tags::video_id)
                .load(conn)
                .unwrap_or_default();

            filtered_ids = Some(intersect_ids(filtered_ids, tag_ids));
        }
    }

    // Production filter
    if let Some(prod_id) = production_id {
        let prod_ids: Vec<i32> = video_productions::table
            .filter(video_productions::production_id.eq(prod_id))
            .select(video_productions::video_id)
            .load(conn)
            .unwrap_or_default();

        filtered_ids = Some(intersect_ids(filtered_ids, prod_ids));
    }

    // Date range filters
    if let Some(from) = date_from {
        let date_ids: Vec<i32> = videos::table
            .filter(videos::created_date.ge(from))
            .select(videos::id)
            .load(conn)
            .unwrap_or_default();

        filtered_ids = Some(intersect_ids(filtered_ids, date_ids));
    }
    if let Some(to) = date_to {
        let date_ids: Vec<i32> = videos::table
            .filter(videos::created_date.le(to))
            .select(videos::id)
            .load(conn)
            .unwrap_or_default();

        filtered_ids = Some(intersect_ids(filtered_ids, date_ids));
    }

    // Build final query from filtered IDs
    let total: i64;
    let video_list: Vec<Video>;
    let offset = (page - 1) * limit;

    match filtered_ids {
        Some(ids) => {
            total = ids.len() as i64;
            let query = videos::table
                .filter(videos::id.eq_any(&ids))
                .into_boxed();
            let query = apply_sorting(query, sort);
            video_list = query
                .offset(offset)
                .limit(limit)
                .load(conn)
                .unwrap_or_default();
        }
        None => {
            // No filters applied — get all videos
            total = videos::table
                .count()
                .get_result(conn)
                .unwrap_or(0);

            let query = videos::table.into_boxed();
            let query = apply_sorting(query, sort);
            video_list = query
                .offset(offset)
                .limit(limit)
                .load(conn)
                .unwrap_or_default();
        }
    }

    let full_videos = video_service::load_videos_full(conn, video_list);
    (full_videos, total)
}

/// Intersect current filtered IDs with new set.
/// If current is None (no filter yet), just return the new set.
fn intersect_ids(current: Option<Vec<i32>>, new: Vec<i32>) -> Vec<i32> {
    match current {
        None => new,
        Some(existing) => existing.into_iter().filter(|id| new.contains(id)).collect(),
    }
}

fn apply_sorting<'a>(
    query: videos::BoxedQuery<'a, diesel::sqlite::Sqlite>,
    sort: &'a str,
) -> videos::BoxedQuery<'a, diesel::sqlite::Sqlite> {
    match sort {
        "date_asc" => query.order(videos::created_date.asc()),
        "name_asc" => query.order(videos::filename.asc()),
        "name_desc" => query.order(videos::filename.desc()),
        "duration_asc" => query.order(videos::duration.asc()),
        "duration_desc" => query.order(videos::duration.desc()),
        "size_asc" => query.order(videos::file_size.asc()),
        "size_desc" => query.order(videos::file_size.desc()),
        _ => query.order(videos::created_date.desc()), // date_desc default
    }
}

/// Get recently indexed videos
pub fn get_recent_videos(conn: &mut SqliteConnection, limit: i64) -> Vec<VideoFull> {
    let video_list: Vec<Video> = videos::table
        .order(videos::indexed_date.desc())
        .limit(limit)
        .load(conn)
        .unwrap_or_default();

    video_service::load_videos_full(conn, video_list)
}

/// Get aggregate statistics
pub fn get_statistics(conn: &mut SqliteConnection) -> serde_json::Value {
    let total_videos: i64 = videos::table
        .count()
        .get_result(conn)
        .unwrap_or(0);

    let total_duration: f64 = diesel::dsl::sql::<diesel::sql_types::Double>(
        "COALESCE(SUM(duration), 0)"
    )
    .get_result(conn)
    .unwrap_or(0.0);

    let total_size: i64 = diesel::dsl::sql::<diesel::sql_types::BigInt>(
        "COALESCE(SUM(file_size), 0)"
    )
    .get_result(conn)
    .unwrap_or(0);

    let total_categories: i64 = metadata::table
        .filter(metadata::category.is_not_null())
        .select(diesel::dsl::sql::<diesel::sql_types::BigInt>(
            "COUNT(DISTINCT category)"
        ))
        .first(conn)
        .unwrap_or(0);

    let total_tags: i64 = tags::table
        .count()
        .get_result(conn)
        .unwrap_or(0);

    serde_json::json!({
        "total_videos": total_videos,
        "total_duration": total_duration,
        "total_size": total_size,
        "total_categories": total_categories,
        "total_tags": total_tags,
    })
}

/// Get all tags with video counts
pub fn get_all_tags(conn: &mut SqliteConnection) -> Vec<TagWithCount> {
    let results: Vec<(Tag, Option<i64>)> = tags::table
        .left_join(video_tags::table)
        .group_by(tags::id)
        .select((
            tags::all_columns,
            diesel::dsl::sql::<diesel::sql_types::Nullable<diesel::sql_types::BigInt>>(
                "COUNT(video_tags.video_id)"
            ),
        ))
        .order(tags::name.asc())
        .load(conn)
        .unwrap_or_default();

    results.into_iter().map(|(tag, count)| {
        TagWithCount {
            id: tag.id,
            name: tag.name,
            count: count.unwrap_or(0),
        }
    }).collect()
}

/// Get all categories with video counts
pub fn get_all_categories(conn: &mut SqliteConnection) -> Vec<CategoryResponse> {
    let results: Vec<(String, i64)> = metadata::table
        .filter(metadata::category.is_not_null())
        .group_by(metadata::category)
        .select((
            metadata::category.assume_not_null(),
            diesel::dsl::count(metadata::id),
        ))
        .order(metadata::category.asc())
        .load(conn)
        .unwrap_or_default();

    results.into_iter().map(|(name, count)| {
        CategoryResponse { name, count }
    }).collect()
}
