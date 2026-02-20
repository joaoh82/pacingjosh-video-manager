use chrono::NaiveDateTime;
use log::{error, warn};
use std::path::Path;
use std::process::Command;

#[derive(Debug, Clone)]
pub struct VideoMetadata {
    pub duration: Option<f32>,
    pub file_size: Option<i64>,
    pub codec: Option<String>,
    pub resolution: Option<String>,
    pub fps: Option<f32>,
    pub created_date: Option<NaiveDateTime>,
}

#[allow(dead_code)]
/// Check that ffmpeg and ffprobe are available on PATH
pub fn check_ffmpeg() -> Result<(), String> {
    Command::new("ffprobe")
        .arg("-version")
        .output()
        .map_err(|_| "ffprobe not found in PATH".to_string())?;

    Command::new("ffmpeg")
        .arg("-version")
        .output()
        .map_err(|_| "ffmpeg not found in PATH".to_string())?;

    Ok(())
}

/// Extract metadata from a video file using ffprobe
pub fn extract_metadata(video_path: &Path) -> Option<VideoMetadata> {
    let output = Command::new("ffprobe")
        .args([
            "-v", "quiet",
            "-print_format", "json",
            "-show_format",
            "-show_streams",
        ])
        .arg(video_path)
        .output();

    let output = match output {
        Ok(o) => o,
        Err(e) => {
            error!("Failed to run ffprobe: {}", e);
            return None;
        }
    };

    if !output.status.success() {
        error!("ffprobe failed for {:?}", video_path);
        return None;
    }

    let json_str = match String::from_utf8(output.stdout) {
        Ok(s) => s,
        Err(e) => {
            error!("ffprobe output not valid UTF-8: {}", e);
            return None;
        }
    };

    let parsed: serde_json::Value = match serde_json::from_str(&json_str) {
        Ok(v) => v,
        Err(e) => {
            error!("Failed to parse ffprobe JSON: {}", e);
            return None;
        }
    };

    // Find first video stream
    let video_stream = parsed["streams"]
        .as_array()
        .and_then(|streams| {
            streams.iter().find(|s| s["codec_type"].as_str() == Some("video"))
        });

    let duration = parsed["format"]["duration"]
        .as_str()
        .and_then(|d| d.parse::<f32>().ok());

    let file_size = parsed["format"]["size"]
        .as_str()
        .and_then(|s| s.parse::<i64>().ok());

    let codec = video_stream
        .and_then(|s| s["codec_name"].as_str())
        .map(|s| s.to_string());

    let resolution = video_stream.and_then(|s| {
        let w = s["width"].as_i64()?;
        let h = s["height"].as_i64()?;
        Some(format!("{}x{}", w, h))
    });

    let fps = video_stream
        .and_then(|s| s["r_frame_rate"].as_str())
        .and_then(parse_fps);

    let created_date = parsed["format"]["tags"]["creation_time"]
        .as_str()
        .and_then(|ct| {
            let cleaned = ct.replace('Z', "+00:00");
            chrono::DateTime::parse_from_rfc3339(&cleaned)
                .ok()
                .map(|dt| dt.naive_utc())
                .or_else(|| {
                    NaiveDateTime::parse_from_str(ct, "%Y-%m-%dT%H:%M:%S%.f").ok()
                })
        });

    Some(VideoMetadata {
        duration,
        file_size,
        codec,
        resolution,
        fps,
        created_date,
    })
}

/// Parse FPS string from ffprobe (handles "30000/1001" and "30.0")
fn parse_fps(fps_string: &str) -> Option<f32> {
    if let Some((num, den)) = fps_string.split_once('/') {
        let n: f32 = num.parse().ok()?;
        let d: f32 = den.parse().ok()?;
        if d == 0.0 {
            return None;
        }
        Some(n / d)
    } else {
        fps_string.parse::<f32>().ok()
    }
}

/// Generate thumbnails for a video at evenly-spaced intervals, stored by checksum
pub fn generate_thumbnails(
    video_path: &Path,
    checksum: &str,
    thumbnail_dir: &Path,
    count: i32,
    width: i32,
) -> i32 {
    let meta = match extract_metadata(video_path) {
        Some(m) => m,
        None => {
            warn!("Could not extract metadata for thumbnails: {:?}", video_path);
            return 0;
        }
    };

    let duration = match meta.duration {
        Some(d) if d > 0.0 => d,
        _ => {
            warn!("Invalid duration for thumbnail generation: {:?}", video_path);
            return 0;
        }
    };

    let out_dir = thumbnail_dir.join(checksum);
    std::fs::create_dir_all(&out_dir).ok();

    let interval = duration / (count as f32 + 1.0);
    let mut generated = 0;

    for i in 0..count {
        let timestamp = interval * (i as f32 + 1.0);
        let output_path = out_dir.join(format!("thumb_{}.jpg", i));

        let result = Command::new("ffmpeg")
            .args([
                "-ss", &format!("{}", timestamp),
                "-i",
            ])
            .arg(video_path)
            .args([
                "-vframes", "1",
                "-vf", &format!("scale={}:-1", width),
                "-y",
            ])
            .arg(&output_path)
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .output();

        match result {
            Ok(output) if output.status.success() && output_path.exists() => {
                generated += 1;
            }
            Ok(_) => {
                warn!("Thumbnail generation failed for index {} of {:?}", i, video_path);
            }
            Err(e) => {
                error!("Failed to run ffmpeg for thumbnail: {}", e);
            }
        }
    }

    generated
}
