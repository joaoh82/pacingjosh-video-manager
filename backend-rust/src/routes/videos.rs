use actix_web::{get, post, put, delete, web, HttpResponse};
use chrono::NaiveDateTime;
use serde::Deserialize;

use crate::db::DbPool;
use crate::models::*;
use crate::services::{video_service, search_service};

#[derive(Deserialize)]
pub struct VideoSearchParams {
    pub search: Option<String>,
    pub category: Option<String>,
    pub tags: Option<String>,
    pub production: Option<i32>,
    pub date_from: Option<String>,
    pub date_to: Option<String>,
    pub sort: Option<String>,
    pub page: Option<i64>,
    pub limit: Option<i64>,
}

#[get("/videos")]
async fn list_videos(
    pool: web::Data<DbPool>,
    query: web::Query<VideoSearchParams>,
) -> HttpResponse {
    let mut conn = pool.get().expect("Failed to get DB connection");

    let page = query.page.unwrap_or(1).max(1);
    let limit = query.limit.unwrap_or(50).max(1).min(200);
    let sort = query.sort.as_deref().unwrap_or("date_desc");

    let tag_names = query.tags.as_ref().map(|t| {
        t.split(',').map(|s| s.trim().to_string()).filter(|s| !s.is_empty()).collect::<Vec<_>>()
    });

    let date_from = query.date_from.as_ref().and_then(|d| {
        NaiveDateTime::parse_from_str(d, "%Y-%m-%dT%H:%M:%S")
            .or_else(|_| NaiveDateTime::parse_from_str(&format!("{}T00:00:00", d), "%Y-%m-%dT%H:%M:%S"))
            .ok()
    });

    let date_to = query.date_to.as_ref().and_then(|d| {
        NaiveDateTime::parse_from_str(d, "%Y-%m-%dT%H:%M:%S")
            .or_else(|_| NaiveDateTime::parse_from_str(&format!("{}T23:59:59", d), "%Y-%m-%dT%H:%M:%S"))
            .ok()
    });

    let (videos, total) = search_service::search_videos(
        &mut conn,
        query.search.as_deref(),
        query.category.as_deref(),
        tag_names,
        query.production,
        date_from,
        date_to,
        sort,
        page,
        limit,
    );

    let pages = if total == 0 { 1 } else { (total + limit - 1) / limit };

    HttpResponse::Ok().json(VideoListResponse {
        videos,
        total,
        page,
        limit,
        pages,
    })
}

#[get("/videos/{video_id}")]
async fn get_video(
    pool: web::Data<DbPool>,
    path: web::Path<i32>,
) -> HttpResponse {
    let mut conn = pool.get().expect("Failed to get DB connection");
    let video_id = path.into_inner();

    match video_service::get_video(&mut conn, video_id) {
        Some(video) => HttpResponse::Ok().json(video),
        None => HttpResponse::NotFound().json(serde_json::json!({
            "detail": format!("Video not found: {}", video_id),
        })),
    }
}

#[put("/videos/{video_id}")]
async fn update_video(
    pool: web::Data<DbPool>,
    path: web::Path<i32>,
    body: web::Json<VideoUpdate>,
) -> HttpResponse {
    let mut conn = pool.get().expect("Failed to get DB connection");
    let video_id = path.into_inner();

    match video_service::update_video(&mut conn, video_id, &body) {
        Some(video) => HttpResponse::Ok().json(video),
        None => HttpResponse::NotFound().json(serde_json::json!({
            "detail": format!("Video not found: {}", video_id),
        })),
    }
}

#[delete("/videos/{video_id}")]
async fn delete_video(
    pool: web::Data<DbPool>,
    path: web::Path<i32>,
) -> HttpResponse {
    let mut conn = pool.get().expect("Failed to get DB connection");
    let video_id = path.into_inner();

    if video_service::delete_video(&mut conn, video_id) {
        HttpResponse::Ok().json(serde_json::json!({
            "message": format!("Video {} deleted", video_id),
        }))
    } else {
        HttpResponse::NotFound().json(serde_json::json!({
            "detail": format!("Video not found: {}", video_id),
        }))
    }
}

#[post("/videos/bulk-update")]
async fn bulk_update(
    pool: web::Data<DbPool>,
    body: web::Json<BulkUpdateRequest>,
) -> HttpResponse {
    let mut conn = pool.get().expect("Failed to get DB connection");

    if body.video_ids.is_empty() {
        return HttpResponse::BadRequest().json(serde_json::json!({
            "detail": "video_ids must not be empty",
        }));
    }

    match video_service::bulk_update_videos(&mut conn, &body) {
        Ok(count) => HttpResponse::Ok().json(BulkUpdateResponse {
            updated: count,
            message: format!("Updated {} videos", count),
        }),
        Err(e) => HttpResponse::InternalServerError().json(serde_json::json!({
            "detail": e,
        })),
    }
}

#[derive(Deserialize)]
pub struct RecentParams {
    pub limit: Option<i64>,
}

#[get("/videos/recent/list")]
async fn recent_videos(
    pool: web::Data<DbPool>,
    query: web::Query<RecentParams>,
) -> HttpResponse {
    let mut conn = pool.get().expect("Failed to get DB connection");
    let limit = query.limit.unwrap_or(20).max(1).min(100);

    let videos = search_service::get_recent_videos(&mut conn, limit);
    HttpResponse::Ok().json(videos)
}

#[get("/videos/stats/summary")]
async fn statistics(
    pool: web::Data<DbPool>,
) -> HttpResponse {
    let mut conn = pool.get().expect("Failed to get DB connection");
    let stats = search_service::get_statistics(&mut conn);
    HttpResponse::Ok().json(stats)
}

#[post("/videos/{video_id}/open-folder")]
async fn open_folder(
    pool: web::Data<DbPool>,
    path: web::Path<i32>,
) -> HttpResponse {
    let mut conn = pool.get().expect("Failed to get DB connection");
    let video_id = path.into_inner();

    let video = match video_service::get_video(&mut conn, video_id) {
        Some(v) => v,
        None => {
            return HttpResponse::NotFound().json(serde_json::json!({
                "detail": format!("Video not found: {}", video_id),
            }));
        }
    };

    let file_path = &video.file_path;
    if !std::path::Path::new(file_path).exists() {
        return HttpResponse::NotFound().json(serde_json::json!({
            "detail": "File no longer exists on disk",
        }));
    }

    let result = open_in_explorer(file_path);

    match result {
        Ok(()) => HttpResponse::Ok().json(serde_json::json!({
            "message": "Folder opened",
        })),
        Err(e) => HttpResponse::InternalServerError().json(serde_json::json!({
            "detail": format!("Failed to open folder: {}", e),
        })),
    }
}

fn open_in_explorer(file_path: &str) -> Result<(), String> {
    #[cfg(target_os = "windows")]
    {
        std::process::Command::new("explorer")
            .args(["/select,", file_path])
            .spawn()
            .map_err(|e| e.to_string())?;
    }

    #[cfg(target_os = "macos")]
    {
        std::process::Command::new("open")
            .args(["-R", file_path])
            .spawn()
            .map_err(|e| e.to_string())?;
    }

    #[cfg(target_os = "linux")]
    {
        let folder = std::path::Path::new(file_path)
            .parent()
            .map(|p| p.to_string_lossy().to_string())
            .unwrap_or_else(|| file_path.to_string());
        std::process::Command::new("xdg-open")
            .arg(&folder)
            .spawn()
            .map_err(|e| e.to_string())?;
    }

    Ok(())
}

pub fn configure(cfg: &mut web::ServiceConfig) {
    cfg.service(list_videos)
        .service(recent_videos)
        .service(statistics)
        .service(get_video)
        .service(update_video)
        .service(delete_video)
        .service(bulk_update)
        .service(open_folder);
}
