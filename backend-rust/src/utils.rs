use std::io::{Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};
use std::time::SystemTime;
use chrono::NaiveDateTime;
use sha2::{Sha256, Digest};

#[allow(dead_code)]
/// Check if a file has a supported video extension
pub fn is_video_file(path: &Path, supported_formats: &[String]) -> bool {
    path.extension()
        .and_then(|ext| ext.to_str())
        .map(|ext| {
            let dotted = format!(".{}", ext.to_lowercase());
            supported_formats.contains(&dotted)
        })
        .unwrap_or(false)
}

/// Get file size in bytes
pub fn get_file_size(path: &Path) -> Option<u64> {
    std::fs::metadata(path).ok().map(|m| m.len())
}

/// Get file creation date (falls back to modified time)
pub fn get_file_creation_date(path: &Path) -> Option<NaiveDateTime> {
    let meta = std::fs::metadata(path).ok()?;

    let time = meta.created()
        .or_else(|_| meta.modified())
        .ok()?;

    let duration = time.duration_since(SystemTime::UNIX_EPOCH).ok()?;
    let timestamp = duration.as_secs() as i64;
    chrono::DateTime::from_timestamp(timestamp, 0)
        .map(|dt| dt.naive_utc())
}

/// Validate that a directory exists and is readable
pub fn validate_directory(directory: &str) -> Result<(), String> {
    if directory.is_empty() {
        return Err("Directory path is empty".to_string());
    }

    let path = Path::new(directory);

    if !path.exists() {
        return Err(format!("Directory does not exist: {}", directory));
    }

    if !path.is_dir() {
        return Err(format!("Path is not a directory: {}", directory));
    }

    // Check readability by trying to read dir
    match std::fs::read_dir(path) {
        Ok(_) => Ok(()),
        Err(e) => Err(format!("Directory is not readable: {}", e)),
    }
}

#[allow(dead_code)]
/// Check if a file path is safe (no path traversal)
pub fn is_path_safe(file_path: &str, base_directory: &str) -> bool {
    let target = match Path::new(file_path).canonicalize() {
        Ok(p) => p,
        Err(_) => return false,
    };

    let base = match Path::new(base_directory).canonicalize() {
        Ok(p) => p,
        Err(_) => return false,
    };

    target.starts_with(&base)
}

/// Compute a partial file checksum: SHA-256 of first 64KB + last 64KB + file size.
/// Fast even for multi-GB files, practically unique for distinct video content.
pub fn compute_file_checksum(path: &Path) -> Result<String, String> {
    const CHUNK_SIZE: u64 = 64 * 1024; // 64KB

    let mut file = std::fs::File::open(path)
        .map_err(|e| format!("Failed to open file for checksum: {}", e))?;

    let file_size = file.metadata()
        .map_err(|e| format!("Failed to get file metadata: {}", e))?
        .len();

    let mut hasher = Sha256::new();

    // Hash file size
    hasher.update(file_size.to_le_bytes());

    // Read first 64KB
    let first_chunk_size = std::cmp::min(file_size, CHUNK_SIZE) as usize;
    let mut buf = vec![0u8; first_chunk_size];
    file.read_exact(&mut buf)
        .map_err(|e| format!("Failed to read first chunk: {}", e))?;
    hasher.update(&buf);

    // Read last 64KB (if file is large enough that it doesn't overlap)
    if file_size > CHUNK_SIZE {
        let last_offset = file_size - CHUNK_SIZE;
        file.seek(SeekFrom::Start(last_offset))
            .map_err(|e| format!("Failed to seek for last chunk: {}", e))?;
        let mut last_buf = vec![0u8; CHUNK_SIZE as usize];
        file.read_exact(&mut last_buf)
            .map_err(|e| format!("Failed to read last chunk: {}", e))?;
        hasher.update(&last_buf);
    }

    let hash = hasher.finalize();
    Ok(format!("{:x}", hash))
}

/// Get the thumbnail path for a video, keyed by checksum
pub fn get_thumbnail_path(checksum: &str, index: i32, thumbnail_dir: &Path) -> PathBuf {
    let dir = thumbnail_dir.join(checksum);
    std::fs::create_dir_all(&dir).ok();
    dir.join(format!("thumb_{}.jpg", index))
}

#[allow(dead_code)]
/// Ensure a directory exists, creating it if necessary
pub fn ensure_directory_exists(directory: &Path) {
    std::fs::create_dir_all(directory).ok();
}

/// Get content type for a video file extension
pub fn get_video_content_type(path: &Path) -> &'static str {
    match path.extension().and_then(|e| e.to_str()).map(|e| e.to_lowercase()).as_deref() {
        Some("mp4") => "video/mp4",
        Some("mov") => "video/quicktime",
        Some("avi") => "video/x-msvideo",
        Some("mkv") => "video/x-matroska",
        Some("webm") => "video/webm",
        Some("flv") => "video/x-flv",
        Some("wmv") => "video/x-ms-wmv",
        _ => "video/mp4",
    }
}
