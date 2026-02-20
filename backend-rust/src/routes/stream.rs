use actix_web::{get, web, HttpRequest, HttpResponse};
use std::io::{Read, Seek, SeekFrom};
use std::path::Path;

use crate::config::ConfigManager;
use crate::db::DbPool;
use crate::services::video_service;
use crate::utils;

#[get("/thumbnails/{video_id}/{index}")]
async fn get_thumbnail(
    pool: web::Data<DbPool>,
    config: web::Data<ConfigManager>,
    path: web::Path<(i32, i32)>,
) -> HttpResponse {
    let (video_id, index) = path.into_inner();
    let mut conn = pool.get().expect("Failed to get DB connection");

    let video = match video_service::get_video(&mut conn, video_id) {
        Some(v) => v,
        None => {
            return HttpResponse::NotFound().json(serde_json::json!({
                "detail": format!("Video not found: {}", video_id),
            }));
        }
    };

    if index < 0 || index >= video.thumbnail_count {
        return HttpResponse::NotFound().json(serde_json::json!({
            "detail": format!("Thumbnail index out of range: {} (0-{})", index, video.thumbnail_count - 1),
        }));
    }

    let thumb_dir = config.get_thumbnail_directory();

    // Use checksum-based path if available, fall back to video_id for legacy data
    let fallback = video_id.to_string();
    let checksum_key = video.checksum.as_deref().unwrap_or(&fallback);
    let thumb_path = utils::get_thumbnail_path(checksum_key, index, &thumb_dir);

    if !thumb_path.exists() {
        return HttpResponse::NotFound().json(serde_json::json!({
            "detail": "Thumbnail file not found",
        }));
    }

    match std::fs::read(&thumb_path) {
        Ok(data) => HttpResponse::Ok()
            .content_type("image/jpeg")
            .insert_header(("Cache-Control", "public, max-age=86400"))
            .body(data),
        Err(_) => HttpResponse::InternalServerError().json(serde_json::json!({
            "detail": "Failed to read thumbnail",
        })),
    }
}

#[get("/stream/{video_id}")]
async fn stream_video(
    pool: web::Data<DbPool>,
    path: web::Path<i32>,
    req: HttpRequest,
) -> HttpResponse {
    let video_id = path.into_inner();
    let mut conn = pool.get().expect("Failed to get DB connection");

    let video = match video_service::get_video(&mut conn, video_id) {
        Some(v) => v,
        None => {
            return HttpResponse::NotFound().json(serde_json::json!({
                "detail": format!("Video not found: {}", video_id),
            }));
        }
    };

    let file_path = Path::new(&video.file_path);
    if !file_path.exists() {
        return HttpResponse::NotFound().json(serde_json::json!({
            "detail": "File no longer exists on disk",
        }));
    }

    let content_type = utils::get_video_content_type(file_path);
    let file_size = match std::fs::metadata(file_path) {
        Ok(m) => m.len(),
        Err(_) => {
            return HttpResponse::InternalServerError().json(serde_json::json!({
                "detail": "Failed to read file metadata",
            }));
        }
    };

    // Check for Range header
    let range_header = req.headers().get("Range").and_then(|v| v.to_str().ok());

    match range_header {
        Some(range_str) => serve_range(file_path, file_size, range_str, content_type),
        None => serve_full(file_path, file_size, content_type),
    }
}

fn serve_full(file_path: &Path, file_size: u64, content_type: &str) -> HttpResponse {
    match std::fs::read(file_path) {
        Ok(data) => HttpResponse::Ok()
            .content_type(content_type)
            .insert_header(("Accept-Ranges", "bytes"))
            .insert_header(("Content-Length", file_size.to_string()))
            .body(data),
        Err(_) => HttpResponse::InternalServerError().json(serde_json::json!({
            "detail": "Failed to read file",
        })),
    }
}

fn serve_range(file_path: &Path, file_size: u64, range_str: &str, content_type: &str) -> HttpResponse {
    let range = match parse_range(range_str, file_size) {
        Some(r) => r,
        None => {
            return HttpResponse::build(actix_web::http::StatusCode::RANGE_NOT_SATISFIABLE)
                .insert_header(("Content-Range", format!("bytes */{}", file_size)))
                .json(serde_json::json!({
                    "detail": "Invalid range",
                }));
        }
    };

    let (start, end) = range;
    let chunk_size = end - start + 1;

    let mut file = match std::fs::File::open(file_path) {
        Ok(f) => f,
        Err(_) => {
            return HttpResponse::InternalServerError().json(serde_json::json!({
                "detail": "Failed to open file",
            }));
        }
    };

    if file.seek(SeekFrom::Start(start)).is_err() {
        return HttpResponse::InternalServerError().json(serde_json::json!({
            "detail": "Failed to seek in file",
        }));
    }

    let mut buffer = vec![0u8; chunk_size as usize];
    match file.read_exact(&mut buffer) {
        Ok(()) => {}
        Err(_) => {
            // Try reading what we can
            buffer.truncate(0);
            file.seek(SeekFrom::Start(start)).ok();
            let mut limited = file.take(chunk_size);
            limited.read_to_end(&mut buffer).ok();
        }
    }

    HttpResponse::build(actix_web::http::StatusCode::PARTIAL_CONTENT)
        .content_type(content_type)
        .insert_header(("Content-Range", format!("bytes {}-{}/{}", start, end, file_size)))
        .insert_header(("Accept-Ranges", "bytes"))
        .insert_header(("Content-Length", chunk_size.to_string()))
        .insert_header(("Cache-Control", "public, max-age=3600"))
        .body(buffer)
}

/// Parse Range header: "bytes=start-end"
fn parse_range(range_str: &str, file_size: u64) -> Option<(u64, u64)> {
    let range_str = range_str.strip_prefix("bytes=")?;
    let parts: Vec<&str> = range_str.splitn(2, '-').collect();

    if parts.len() != 2 {
        return None;
    }

    let start: u64;
    let end: u64;

    if parts[0].is_empty() {
        // bytes=-N (last N bytes)
        let suffix: u64 = parts[1].parse().ok()?;
        start = file_size.saturating_sub(suffix);
        end = file_size - 1;
    } else if parts[1].is_empty() {
        // bytes=N- (from N to end)
        start = parts[0].parse().ok()?;
        end = file_size - 1;
    } else {
        // bytes=N-M
        start = parts[0].parse().ok()?;
        end = parts[1].parse().ok()?;
    }

    if start >= file_size || end >= file_size || start > end {
        return None;
    }

    Some((start, end))
}

pub fn configure(cfg: &mut web::ServiceConfig) {
    cfg.service(get_thumbnail)
        .service(stream_video);
}
