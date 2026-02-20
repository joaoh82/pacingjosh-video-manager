use chrono::{NaiveDateTime, Utc};
use diesel::prelude::*;
use diesel::sqlite::SqliteConnection;
use log::{error, info, warn};
use serde::Serialize;
use std::collections::HashMap;
use std::path::Path;
use std::sync::{Arc, Mutex};
use uuid::Uuid;
use walkdir::WalkDir;

use crate::db::DbPool;
use crate::models::*;
use crate::schema::{videos, metadata};
use crate::services::ffmpeg_service;
use crate::utils;

pub type ScanMap = Arc<Mutex<HashMap<String, ScanProgress>>>;

#[derive(Debug, Clone, Serialize)]
pub struct ScanProgress {
    pub scan_id: String,
    pub status: String,
    pub total_files: i64,
    pub processed_files: i64,
    pub successful: i64,
    pub failed: i64,
    pub skipped: i64,
    pub current_file: String,
    pub errors: Vec<String>,
    pub start_time: NaiveDateTime,
    pub end_time: Option<NaiveDateTime>,
}

impl ScanProgress {
    pub fn new(scan_id: String) -> Self {
        Self {
            scan_id,
            status: "in_progress".to_string(),
            total_files: 0,
            processed_files: 0,
            successful: 0,
            failed: 0,
            skipped: 0,
            current_file: String::new(),
            errors: Vec::new(),
            start_time: Utc::now().naive_utc(),
            end_time: None,
        }
    }

    pub fn to_response(&self) -> serde_json::Value {
        let elapsed = if let Some(end) = self.end_time {
            (end - self.start_time).num_milliseconds() as f64 / 1000.0
        } else {
            (Utc::now().naive_utc() - self.start_time).num_milliseconds() as f64 / 1000.0
        };

        let eta = if self.processed_files > 0 && self.status == "in_progress" && self.total_files > 0 {
            let avg_per_file = elapsed / self.processed_files as f64;
            let remaining = self.total_files - self.processed_files;
            Some(avg_per_file * remaining as f64)
        } else {
            None
        };

        // Return last 10 errors only
        let errors: Vec<&String> = self.errors.iter().rev().take(10).collect();

        serde_json::json!({
            "scan_id": self.scan_id,
            "status": self.status,
            "total": self.total_files,
            "processed": self.processed_files,
            "successful": self.successful,
            "failed": self.failed,
            "skipped": self.skipped,
            "current_file": self.current_file,
            "errors": errors,
            "start_time": self.start_time.format("%Y-%m-%dT%H:%M:%S").to_string(),
            "end_time": self.end_time.map(|t| t.format("%Y-%m-%dT%H:%M:%S").to_string()),
            "elapsed_seconds": elapsed,
            "eta_seconds": eta,
        })
    }
}

/// Start a background scan of a directory
pub fn start_scan(
    directory: String,
    pool: DbPool,
    scan_map: ScanMap,
    supported_formats: Vec<String>,
    thumbnail_dir: String,
    thumbnail_count: i32,
    thumbnail_width: i32,
) -> Result<String, String> {
    utils::validate_directory(&directory)?;

    let scan_id = Uuid::new_v4().to_string();
    let progress = ScanProgress::new(scan_id.clone());

    {
        let mut map = scan_map.lock().unwrap();
        map.insert(scan_id.clone(), progress);
    }

    let scan_id_clone = scan_id.clone();
    let scan_map_clone = scan_map.clone();

    std::thread::spawn(move || {
        run_scan(
            &scan_id_clone,
            &directory,
            pool,
            scan_map_clone,
            &supported_formats,
            &thumbnail_dir,
            thumbnail_count,
            thumbnail_width,
        );
    });

    Ok(scan_id)
}

fn run_scan(
    scan_id: &str,
    directory: &str,
    pool: DbPool,
    scan_map: ScanMap,
    supported_formats: &[String],
    thumbnail_dir: &str,
    thumbnail_count: i32,
    thumbnail_width: i32,
) {
    let mut conn = match pool.get() {
        Ok(c) => c,
        Err(e) => {
            error!("Failed to get DB connection for scan: {}", e);
            let mut map = scan_map.lock().unwrap();
            if let Some(p) = map.get_mut(scan_id) {
                p.status = "failed".to_string();
                p.errors.push(format!("Database connection failed: {}", e));
                p.end_time = Some(Utc::now().naive_utc());
            }
            return;
        }
    };

    // Enable foreign keys on this connection
    diesel::sql_query("PRAGMA foreign_keys=ON;")
        .execute(&mut conn)
        .ok();

    match scan_directory(
        scan_id, directory, &mut conn, &scan_map,
        supported_formats, thumbnail_dir, thumbnail_count, thumbnail_width,
    ) {
        Ok(()) => {
            let mut map = scan_map.lock().unwrap();
            if let Some(p) = map.get_mut(scan_id) {
                p.status = "completed".to_string();
                p.end_time = Some(Utc::now().naive_utc());
                info!("Scan {} completed: {} successful, {} failed, {} skipped",
                    scan_id, p.successful, p.failed, p.skipped);
            }
        }
        Err(e) => {
            let mut map = scan_map.lock().unwrap();
            if let Some(p) = map.get_mut(scan_id) {
                p.status = "failed".to_string();
                p.errors.push(e.clone());
                p.end_time = Some(Utc::now().naive_utc());
                error!("Scan {} failed: {}", scan_id, e);
            }
        }
    }
}

fn scan_directory(
    scan_id: &str,
    directory: &str,
    conn: &mut SqliteConnection,
    scan_map: &ScanMap,
    supported_formats: &[String],
    thumbnail_dir: &str,
    thumbnail_count: i32,
    thumbnail_width: i32,
) -> Result<(), String> {
    // Collect all video files
    let video_files: Vec<_> = WalkDir::new(directory)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().is_file())
        .filter(|e| utils::is_video_file(e.path(), supported_formats))
        .map(|e| e.into_path())
        .collect();

    let total = video_files.len() as i64;

    {
        let mut map = scan_map.lock().unwrap();
        if let Some(p) = map.get_mut(scan_id) {
            p.total_files = total;
        }
    }

    info!("Found {} video files in {}", total, directory);

    for file_path in &video_files {
        let file_path_str = file_path.to_string_lossy().to_string();

        // Update current file
        {
            let mut map = scan_map.lock().unwrap();
            if let Some(p) = map.get_mut(scan_id) {
                p.current_file = file_path.file_name()
                    .map(|f| f.to_string_lossy().to_string())
                    .unwrap_or_default();
            }
        }

        // Compute checksum before duplicate check
        let checksum = match utils::compute_file_checksum(file_path) {
            Ok(cs) => cs,
            Err(e) => {
                warn!("Failed to compute checksum for {:?}: {}", file_path, e);
                let mut map = scan_map.lock().unwrap();
                if let Some(p) = map.get_mut(scan_id) {
                    p.failed += 1;
                    p.processed_files += 1;
                    p.errors.push(format!("{}: checksum failed: {}", file_path_str, e));
                }
                continue;
            }
        };

        // Check if already indexed by checksum first (catches moved files)
        let exists_by_checksum = videos::table
            .filter(videos::checksum.eq(&checksum))
            .count()
            .get_result::<i64>(conn)
            .unwrap_or(0) > 0;

        if exists_by_checksum {
            let mut map = scan_map.lock().unwrap();
            if let Some(p) = map.get_mut(scan_id) {
                p.skipped += 1;
                p.processed_files += 1;
            }
            continue;
        }

        // Also check by file path (catches unchanged files without checksum yet)
        let exists_by_path = videos::table
            .filter(videos::file_path.eq(&file_path_str))
            .count()
            .get_result::<i64>(conn)
            .unwrap_or(0) > 0;

        if exists_by_path {
            let mut map = scan_map.lock().unwrap();
            if let Some(p) = map.get_mut(scan_id) {
                p.skipped += 1;
                p.processed_files += 1;
            }
            continue;
        }

        // Process the video
        match process_video(file_path, conn, &checksum, thumbnail_dir, thumbnail_count, thumbnail_width) {
            Ok(()) => {
                let mut map = scan_map.lock().unwrap();
                if let Some(p) = map.get_mut(scan_id) {
                    p.successful += 1;
                }
            }
            Err(e) => {
                warn!("Failed to process {:?}: {}", file_path, e);
                let mut map = scan_map.lock().unwrap();
                if let Some(p) = map.get_mut(scan_id) {
                    p.failed += 1;
                    p.errors.push(format!("{}: {}", file_path_str, e));
                }
            }
        }

        // Update processed count
        {
            let mut map = scan_map.lock().unwrap();
            if let Some(p) = map.get_mut(scan_id) {
                p.processed_files += 1;
            }
        }
    }

    Ok(())
}

fn process_video(
    file_path: &Path,
    conn: &mut SqliteConnection,
    checksum: &str,
    thumbnail_dir: &str,
    thumbnail_count: i32,
    thumbnail_width: i32,
) -> Result<(), String> {
    // Check file size
    let file_size = utils::get_file_size(file_path).unwrap_or(0);
    if file_size < 1024 {
        return Err("File too small (< 1KB)".to_string());
    }

    // Extract metadata
    let meta = ffmpeg_service::extract_metadata(file_path);
    let file_creation = utils::get_file_creation_date(file_path);

    let file_path_str = file_path.to_string_lossy().to_string();
    let filename = file_path.file_name()
        .map(|f| f.to_string_lossy().to_string())
        .unwrap_or_default();

    // Prefer ffmpeg creation_date over file creation date
    let created_date = meta.as_ref()
        .and_then(|m| m.created_date)
        .or(file_creation);

    let new_video = NewVideo {
        file_path: file_path_str,
        filename,
        duration: meta.as_ref().and_then(|m| m.duration),
        file_size: meta.as_ref().and_then(|m| m.file_size).or(Some(file_size as i64)),
        resolution: meta.as_ref().and_then(|m| m.resolution.clone()),
        fps: meta.as_ref().and_then(|m| m.fps),
        codec: meta.as_ref().and_then(|m| m.codec.clone()),
        created_date,
        indexed_date: Utc::now().naive_utc(),
        thumbnail_count: 0,
        checksum: Some(checksum.to_string()),
    };

    diesel::insert_into(videos::table)
        .values(&new_video)
        .execute(conn)
        .map_err(|e| format!("Failed to insert video: {}", e))?;

    // Get the video ID
    let video: Video = videos::table
        .filter(videos::file_path.eq(&new_video.file_path))
        .first(conn)
        .map_err(|e| format!("Failed to fetch inserted video: {}", e))?;

    // Generate thumbnails (skip if checksum folder already has them)
    let thumb_dir = Path::new(thumbnail_dir);
    let existing_thumb_dir = thumb_dir.join(checksum);
    let already_has_thumbs = existing_thumb_dir.is_dir()
        && std::fs::read_dir(&existing_thumb_dir)
            .map(|entries| entries.filter_map(|e| e.ok()).any(|e| {
                e.path().extension().and_then(|ext| ext.to_str()) == Some("jpg")
            }))
            .unwrap_or(false);

    let thumb_count = if already_has_thumbs {
        // Count existing thumbnails
        std::fs::read_dir(&existing_thumb_dir)
            .map(|entries| entries.filter_map(|e| e.ok())
                .filter(|e| e.path().extension().and_then(|ext| ext.to_str()) == Some("jpg"))
                .count() as i32)
            .unwrap_or(0)
    } else {
        ffmpeg_service::generate_thumbnails(
            file_path, checksum, thumb_dir, thumbnail_count, thumbnail_width,
        )
    };

    // Update thumbnail count
    diesel::update(videos::table.find(video.id))
        .set(videos::thumbnail_count.eq(thumb_count))
        .execute(conn)
        .ok();

    if thumb_count == 0 {
        warn!("No thumbnails generated for video {}", video.id);
    }

    // Create empty metadata record
    diesel::insert_into(metadata::table)
        .values(&NewMetadata {
            video_id: video.id,
            category: None,
            location: None,
            notes: None,
        })
        .execute(conn)
        .ok();

    Ok(())
}

/// Get scan progress by ID
pub fn get_scan_progress(scan_map: &ScanMap, scan_id: &str) -> Option<serde_json::Value> {
    let map = scan_map.lock().unwrap();
    map.get(scan_id).map(|p| p.to_response())
}
