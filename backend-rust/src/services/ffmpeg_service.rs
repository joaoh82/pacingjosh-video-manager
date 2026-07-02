use chrono::NaiveDateTime;
use log::{error, warn};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::OnceLock;

#[derive(Debug, Clone)]
pub struct VideoMetadata {
    pub duration: Option<f32>,
    pub file_size: Option<i64>,
    pub codec: Option<String>,
    pub resolution: Option<String>,
    pub fps: Option<f32>,
    pub created_date: Option<NaiveDateTime>,
}

/// Explicit binary paths for ffmpeg and ffprobe. When the backend is embedded
/// in Tauri, these point at sidecar binaries; when running standalone they
/// default to the plain binary names (resolved via PATH).
#[derive(Debug, Clone)]
pub struct FfmpegPaths {
    pub ffmpeg: PathBuf,
    pub ffprobe: PathBuf,
}

impl Default for FfmpegPaths {
    fn default() -> Self {
        Self {
            ffmpeg: PathBuf::from("ffmpeg"),
            ffprobe: PathBuf::from("ffprobe"),
        }
    }
}

static FFMPEG_PATHS: OnceLock<FfmpegPaths> = OnceLock::new();

/// Set the ffmpeg/ffprobe binary paths for this process. Must be called before
/// any scan or thumbnail generation begins. Idempotent — calling twice is a
/// no-op on the second call (OnceLock semantics).
pub fn set_ffmpeg_paths(paths: FfmpegPaths) {
    let _ = FFMPEG_PATHS.set(paths);
}

fn ffprobe_cmd() -> Command {
    let paths = FFMPEG_PATHS.get().cloned().unwrap_or_default();
    Command::new(paths.ffprobe)
}

fn ffmpeg_cmd() -> Command {
    let paths = FFMPEG_PATHS.get().cloned().unwrap_or_default();
    Command::new(paths.ffmpeg)
}

/// The resolved ffmpeg binary path plus its version line — surfaced in the
/// edit pipeline's activity log so it's clear which ffmpeg actually ran
/// (the bundled sidecar vs. a system PATH ffmpeg that may behave differently).
pub fn ffmpeg_diagnostics() -> String {
    let paths = FFMPEG_PATHS.get().cloned().unwrap_or_default();
    let version = ffmpeg_cmd()
        .arg("-version")
        .output()
        .ok()
        .and_then(|o| String::from_utf8(o.stdout).ok())
        .and_then(|s| s.lines().next().map(|l| l.trim().to_string()))
        .unwrap_or_else(|| "version unknown".to_string());
    format!("{} — {}", paths.ffmpeg.display(), version)
}

#[allow(dead_code)]
/// Check that ffmpeg and ffprobe are available (at the configured paths, or PATH).
pub fn check_ffmpeg() -> Result<(), String> {
    ffprobe_cmd()
        .arg("-version")
        .output()
        .map_err(|_| "ffprobe not found".to_string())?;

    ffmpeg_cmd()
        .arg("-version")
        .output()
        .map_err(|_| "ffmpeg not found".to_string())?;

    Ok(())
}

/// Extract metadata from a video file using ffprobe
pub fn extract_metadata(video_path: &Path) -> Option<VideoMetadata> {
    let output = ffprobe_cmd()
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
        let mut w = s["width"].as_i64()?;
        let mut h = s["height"].as_i64()?;
        // Account for display rotation so portrait phone footage that is stored
        // as landscape + a rotation tag reports its true display orientation.
        let rotation = stream_rotation(s);
        if rotation == 90 || rotation == 270 {
            std::mem::swap(&mut w, &mut h);
        }
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

/// Extract the display rotation (in degrees, normalized to 0/90/180/270) from a
/// video stream's ffprobe JSON. Reads both the legacy `tags.rotate` field and
/// the modern Display Matrix `side_data_list[].rotation` (which is negative).
fn stream_rotation(stream: &serde_json::Value) -> i64 {
    // Legacy container tag, e.g. "tags": { "rotate": "90" }
    if let Some(rotate) = stream["tags"]["rotate"]
        .as_str()
        .and_then(|s| s.parse::<i64>().ok())
    {
        return rotate.rem_euclid(360);
    }

    // Modern Display Matrix side data, e.g. "rotation": -90
    if let Some(list) = stream["side_data_list"].as_array() {
        for sd in list {
            if let Some(rot) = sd["rotation"].as_f64() {
                return (rot.round() as i64).rem_euclid(360);
            }
        }
    }

    0
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

        let result = ffmpeg_cmd()
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

/// Extract a compact mono audio track from a video for transcription. Returns
/// the path to a temporary MP3 (mono, 16 kHz, 64 kbps) written under `out_dir`.
/// The caller is responsible for deleting the file when done.
pub fn extract_audio(video_path: &Path, out_dir: &Path) -> Result<PathBuf, String> {
    std::fs::create_dir_all(out_dir).map_err(|e| e.to_string())?;

    let stem = video_path
        .file_stem()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_else(|| "audio".to_string());
    let output_path = out_dir.join(format!("{}-{}.mp3", stem, std::process::id()));

    let result = ffmpeg_cmd()
        .arg("-i")
        .arg(video_path)
        .args([
            "-vn",            // drop video
            "-ac", "1",       // mono
            "-ar", "16000",   // 16 kHz
            "-b:a", "64k",
            "-y",
        ])
        .arg(&output_path)
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .output();

    match result {
        Ok(output) if output.status.success() && output_path.exists() => Ok(output_path),
        Ok(_) => Err(format!("ffmpeg failed to extract audio from {:?}", video_path)),
        Err(e) => Err(format!("Failed to run ffmpeg: {}", e)),
    }
}

/// Build the audio-filter chain for "Enhance voice" (CapCut-style noise removal)
/// at `intensity` (0.0–1.0). Higher intensity removes more noise:
///   * `highpass` rolls off low-frequency wind/handling rumble (cutoff rises with
///     intensity),
///   * `afftdn` is an FFT denoiser that knocks down steady background hiss (its
///     noise-reduction amount in dB rises with intensity),
///   * `adeclick` removes impulsive mouth clicks/pops,
///   * a gentle high-shelf (`treble`) restores the presence/clarity the denoiser
///     dulls, so the cleaned voice doesn't sound muffled.
/// All filters are built into FFmpeg, so no external model file is needed.
pub fn voice_enhance_filter(intensity: f32) -> String {
    let i = intensity.clamp(0.0, 1.0);
    // 70 Hz (gentle) → 110 Hz (firmer wind/rumble cut).
    let highpass = 70.0 + 40.0 * i;
    // 6 dB (subtle) → 20 dB (strong) of broadband noise reduction. Kept below
    // afftdn's heavy range, where speech harmonics start to smear/muffle.
    let nr = 6.0 + 14.0 * i;
    // High-shelf gain to counteract the dulling the denoiser introduces; scales
    // with how hard we denoise.
    let presence = 2.0 + 2.0 * i;
    format!(
        "highpass=f={highpass:.0},afftdn=nr={nr:.1}:nf=-30,adeclick,treble=g={presence:.1}:f=3000",
        highpass = highpass,
        nr = nr,
        presence = presence,
    )
}

/// Extract a single clip `[start, end)` (seconds) from `input` and normalize it
/// to a fixed `width`x`height`/`fps`, H.264 + AAC, yuv420p. Normalizing every
/// segment to the same spec lets the final concat use stream copy.
///
/// When `subtitle_name` is set, that subtitle file (an SRT in the same directory
/// as `out_path`) is burned into the frame. It is passed as a bare filename and
/// ffmpeg is run with its working directory set to the output folder, which
/// sidesteps the `subtitles` filter's painful path escaping on Windows.
///
/// When `audio_filter` is set, it is applied to the audio with `-af` (used for
/// the optional "Enhance voice" noise removal — see [`voice_enhance_filter`]).
///
/// Returns an error (with the tail of ffmpeg's stderr) on failure.
#[allow(clippy::too_many_arguments)]
pub fn extract_clip_segment(
    input: &Path,
    start: f32,
    end: f32,
    width: i32,
    height: i32,
    fps: f32,
    out_path: &Path,
    subtitle_name: Option<&str>,
    audio_filter: Option<&str>,
) -> Result<(), String> {
    let duration = end - start;
    if duration <= 0.0 {
        return Err(format!("Invalid clip range: start {} >= end {}", start, end));
    }
    let parent = out_path.parent().map(|p| p.to_path_buf());
    if let Some(ref p) = parent {
        std::fs::create_dir_all(p).map_err(|e| e.to_string())?;
    }

    // Even dimensions only — libx264/yuv420p requires width and height divisible by 2.
    let w = (width.max(2) / 2) * 2;
    let h = (height.max(2) / 2) * 2;
    let fps = if fps > 0.0 { fps } else { 30.0 };

    let mut vf = format!(
        "scale={w}:{h}:force_original_aspect_ratio=decrease,\
pad={w}:{h}:(ow-iw)/2:(oh-ih)/2:black,setsar=1,fps={fps}",
        w = w,
        h = h,
        fps = fps
    );
    // Burn in captions last so they're sized relative to the final frame.
    // Portrait (shorts) frames get the platform-native look: bold text and a
    // higher bottom margin so the captions clear the Reels/Shorts/TikTok UI.
    if let Some(name) = subtitle_name {
        let style = if h > w {
            "Fontsize=16,Bold=1,Outline=1,Shadow=0,MarginV=110"
        } else {
            "Fontsize=18,Outline=1,Shadow=0,MarginV=40"
        };
        vf.push_str(&format!(",subtitles={}:force_style='{}'", name, style));
    }

    let mut cmd = ffmpeg_cmd();
    // Run from the output directory so the bare subtitle filename resolves.
    if subtitle_name.is_some() {
        if let Some(ref p) = parent {
            cmd.current_dir(p);
        }
    }
    cmd.arg("-i")
        .arg(input)
        // Accurate (output-side) seek so cut points match the chosen timestamps.
        .args(["-ss", &format!("{}", start), "-t", &format!("{}", duration)])
        .args(["-vf", &vf]);
    // Optional voice-enhancement (noise removal) on the audio track.
    if let Some(af) = audio_filter.filter(|s| !s.is_empty()) {
        cmd.args(["-af", af]);
    }
    let output = cmd
        .args([
            "-c:v", "libx264",
            "-preset", "veryfast",
            "-crf", "20",
            "-pix_fmt", "yuv420p",
            "-c:a", "aac",
            "-ar", "48000",
            "-ac", "2",
            "-movflags", "+faststart",
            "-y",
        ])
        .arg(out_path)
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::piped())
        .output()
        .map_err(|e| format!("Failed to run ffmpeg: {}", e))?;

    if output.status.success() && out_path.exists() {
        Ok(())
    } else {
        Err(format!(
            "ffmpeg failed to extract clip from {:?}: {}",
            input,
            stderr_tail(&output.stderr)
        ))
    }
}

/// Build the ffmpeg `volume` expression that automates the background music
/// gain across the final timeline. The music sits at `full` when no one is
/// speaking and drops to `duck` inside any `duck_intervals` (speech + any music
/// the user "removed" on the timeline). For each `fade_intervals` region the
/// gain ramps **up** from `duck` to `full` over the first ~second and back
/// **down** to `duck` over the last ~second, so a music burst swells in and out
/// instead of popping. Ducking takes precedence over fades where they overlap.
///
/// Kept as a pure function so the (fiddly) expression can be unit-tested without
/// invoking ffmpeg.
pub fn music_volume_expr(
    full: f32,
    duck: f32,
    duck_intervals: &[(f32, f32)],
    fade_intervals: &[(f32, f32)],
) -> String {
    let full = full.clamp(0.0, 1.0);
    let duck = duck.clamp(0.0, full);

    // duck = full inside any duck interval. `between()` returns 1 within [s,e];
    // summing them is nonzero (→ true) anywhere a region applies.
    let duck_cond = duck_intervals
        .iter()
        .filter(|(s, e)| e > s)
        .map(|(s, e)| format!("between(t,{:.3},{:.3})", s, e))
        .collect::<Vec<_>>()
        .join("+");

    // Each fade region contributes an `if(between(...), <ramp>, ...)` clause that
    // ramps duck→full→duck across the region. Duck intervals are checked first
    // (outermost), so speech still wins.
    let mut expr = format!("{:.4}", full);
    for (s, e) in fade_intervals.iter().filter(|(s, e)| e > s) {
        let span = e - s;
        // Fade length: up to ~1s, but never more than half the region so the
        // ramps don't cross.
        let f = (span / 2.0).clamp(0.01, 1.0);
        let fade_in_end = s + f;
        let fade_out_start = e - f;
        let ramp_in = format!("{:.4}+({:.4})*(t-{:.3})/{:.3}", duck, full - duck, s, f);
        let ramp_out = format!("{:.4}-({:.4})*(t-{:.3})/{:.3}", full, full - duck, fade_out_start, f);
        let region_expr = format!(
            "if(lt(t,{:.3}),{},if(gt(t,{:.3}),{},{:.4}))",
            fade_in_end, ramp_in, fade_out_start, ramp_out, full
        );
        expr = format!("if(between(t,{:.3},{:.3}),{},{})", s, e, region_expr, expr);
    }

    if !duck_cond.is_empty() {
        expr = format!("if({},{:.4},{})", duck_cond, duck, expr);
    }
    expr
}

/// Mix a background-music track under an existing video's audio and write the
/// result to `out_path`, with **two explicit levels**: the looped music sits at
/// `full_volume` when no one is speaking and at `duck_volume` while the voice is
/// talking.
///
/// Ducking is driven by the KNOWN speech intervals (from the transcript word
/// timestamps), not by audio-level sidechain detection. This is deterministic
/// and independent of how loud/quiet the speech was recorded — and a
/// `duck_volume` of 0 truly silences the music during speech. The music's gain
/// is automated with a `volume` expression (see [`music_volume_expr`]) that
/// returns `duck_volume` inside any `duck` interval, `full_volume` everywhere
/// else, and ramps in/out across any `fade` region. The voice is mixed back at
/// full level and a limiter guards against clipping; the video stream is copied
/// untouched. `duck` and `fade` are lists of (start, end) intervals in seconds
/// on the final timeline. Returns an error (with ffmpeg stderr) on failure.
pub fn add_background_music(
    video: &Path,
    music: &Path,
    full_volume: f32,
    duck_volume: f32,
    duck: &[(f32, f32)],
    fade: &[(f32, f32)],
    out_path: &Path,
) -> Result<(), String> {
    if let Some(parent) = out_path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    let full = full_volume.clamp(0.0, 1.0);
    let duck_v = duck_volume.clamp(0.0, full);

    let vol_expr = music_volume_expr(full, duck_v, duck, fade);

    let filter = format!(
        "[1:a]aformat=channel_layouts=stereo:sample_rates=48000,volume='{}':eval=frame[music];\
[0:a]aformat=channel_layouts=stereo:sample_rates=48000[voice];\
[voice][music]amix=inputs=2:duration=first:normalize=0[mixed];\
[mixed]alimiter=limit=0.95[aout]",
        vol_expr
    );

    let output = ffmpeg_cmd()
        .arg("-i")
        .arg(video)
        // Loop the music input indefinitely; amix(duration=first) stops at the
        // video's end.
        .args(["-stream_loop", "-1"])
        .arg("-i")
        .arg(music)
        .args(["-filter_complex", &filter])
        .args([
            "-map", "0:v",
            "-map", "[aout]",
            "-c:v", "copy",
            "-c:a", "aac",
            "-ar", "48000",
            "-ac", "2",
            "-movflags", "+faststart",
            "-y",
        ])
        .arg(out_path)
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::piped())
        .output()
        .map_err(|e| format!("Failed to run ffmpeg: {}", e))?;

    if output.status.success() && out_path.exists() {
        Ok(())
    } else {
        Err(format!(
            "ffmpeg failed to add background music: {}",
            stderr_tail(&output.stderr)
        ))
    }
}

/// Concatenate already-normalized segment files into `out_path` using the
/// concat demuxer with stream copy. `segments` must share identical codecs and
/// parameters (guaranteed when produced by `extract_clip_segment`).
pub fn concat_clips(segments: &[PathBuf], out_path: &Path) -> Result<(), String> {
    if segments.is_empty() {
        return Err("No segments to concatenate".to_string());
    }
    if let Some(parent) = out_path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }

    // The concat demuxer reads a list file. Use forward slashes so the paths
    // parse identically on Windows and Unix.
    let list_path = out_path.with_extension("concat.txt");
    let mut list = String::new();
    for seg in segments {
        let p = seg.to_string_lossy().replace('\\', "/");
        list.push_str(&format!("file '{}'\n", p));
    }
    std::fs::write(&list_path, list).map_err(|e| format!("Failed to write concat list: {}", e))?;

    let output = ffmpeg_cmd()
        .args(["-f", "concat", "-safe", "0", "-i"])
        .arg(&list_path)
        .args(["-c", "copy", "-movflags", "+faststart", "-y"])
        .arg(out_path)
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::piped())
        .output()
        .map_err(|e| format!("Failed to run ffmpeg: {}", e))?;

    let _ = std::fs::remove_file(&list_path);

    if output.status.success() && out_path.exists() {
        Ok(())
    } else {
        Err(format!(
            "ffmpeg failed to concatenate clips: {}",
            stderr_tail(&output.stderr)
        ))
    }
}

/// Grab a single still frame from `video` at `t` seconds, scaled/cropped to
/// cover `width`x`height` (a thumbnail base). Returns the JPEG bytes.
pub fn extract_frame(video: &Path, t: f32, width: i32, height: i32) -> Result<Vec<u8>, String> {
    let w = (width.max(2) / 2) * 2;
    let h = (height.max(2) / 2) * 2;
    let out = std::env::temp_dir().join(format!("vm-frame-{}.jpg", std::process::id()));
    let vf = format!(
        "scale={w}:{h}:force_original_aspect_ratio=increase,crop={w}:{h}",
        w = w,
        h = h
    );
    let output = ffmpeg_cmd()
        .args(["-ss", &format!("{}", t.max(0.0))])
        .arg("-i")
        .arg(video)
        .args(["-frames:v", "1", "-vf", &vf, "-q:v", "2", "-y"])
        .arg(&out)
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::piped())
        .output()
        .map_err(|e| format!("Failed to run ffmpeg: {}", e))?;

    if !output.status.success() || !out.exists() {
        return Err(format!("ffmpeg failed to grab frame: {}", stderr_tail(&output.stderr)));
    }
    let bytes = std::fs::read(&out).map_err(|e| format!("Failed to read frame: {}", e))?;
    let _ = std::fs::remove_file(&out);
    Ok(bytes)
}

/// One overlay snippet to composite onto the final video: which file, when it
/// appears on the timeline, how to key out its (solid-colour) background, and
/// where/how big to place it. Used for "Subscribe" bugs and similar bumpers.
#[derive(Debug, Clone)]
pub struct OverlayPlacement {
    pub path: PathBuf,
    /// Start time on the final timeline (seconds) where the snippet appears.
    pub start: f32,
    /// How long the snippet stays on screen (seconds) — usually its own length.
    pub duration: f32,
    /// Chroma-key colour to remove (the snippet's background) as an ffmpeg
    /// colour, e.g. `0xFFFFFF` for white. Empty disables keying (composited
    /// opaque).
    pub chroma_color: String,
    /// colorkey similarity (0..1): higher removes a wider range around the key.
    pub similarity: f32,
    /// colorkey blend (0..1): softens the keyed edge.
    pub blend: f32,
    /// Scale factor applied to the snippet (1.0 = original size).
    pub scale: f32,
    /// Overall opacity of the snippet (0..1).
    pub opacity: f32,
    /// Position preset: "center" (default), "top", "bottom", "left", "right",
    /// "top_left", "top_right", "bottom_left", "bottom_right".
    pub position: String,
}

/// Image extensions treated as a single still frame, held on screen (looped via
/// `-loop 1`) for the overlay's display window. GIFs are handled separately
/// (animated, looped via `-ignore_loop 0`); anything else is treated as video.
pub fn is_static_image_ext(ext: &str) -> bool {
    matches!(ext, "png" | "jpg" | "jpeg" | "webp" | "bmp")
}

/// Overlay x/y position expressions (in `overlay` filter syntax, using W/H for
/// the base frame and w/h for the snippet) for a named preset. `margin` is the
/// edge inset in pixels for the non-centered presets. Pure for testability.
fn overlay_xy(position: &str, margin: i32) -> (String, String) {
    let m = margin.max(0);
    let cx = "(W-w)/2".to_string();
    let cy = "(H-h)/2".to_string();
    let left = format!("{}", m);
    let right = format!("W-w-{}", m);
    let top = format!("{}", m);
    let bottom = format!("H-h-{}", m);
    match position {
        "top" => (cx, top),
        "bottom" => (cx, bottom),
        "left" => (left, cy),
        "right" => (right, cy),
        "top_left" => (left, top),
        "top_right" => (right, top),
        "bottom_left" => (left, bottom),
        "bottom_right" => (right, bottom),
        _ => (cx, cy),
    }
}

/// Build the `filter_complex` graph that composites `overlays` onto the base
/// video (input `[0:v]`; each overlay is input `[i+1:v]`). Each snippet is
/// chroma-keyed, optionally scaled/faded, time-shifted to start at its
/// `start`, and laid on with `enable='between(t,start,end)'` so it only shows
/// during its window. The final node is labelled `[vout]`. Pure (no I/O) so the
/// (fiddly) graph can be unit-tested without invoking ffmpeg.
pub fn build_overlay_filter(overlays: &[OverlayPlacement]) -> String {
    const MARGIN: i32 = 40;
    let mut parts: Vec<String> = Vec::new();

    // Prepare each snippet stream: key → scale → opacity → time-shift.
    for (i, o) in overlays.iter().enumerate() {
        let mut chain: Vec<String> = Vec::new();
        let color = o.chroma_color.trim();
        if !color.is_empty() {
            let sim = o.similarity.clamp(0.0, 1.0);
            let blend = o.blend.clamp(0.0, 1.0);
            chain.push(format!("colorkey={}:{:.3}:{:.3}", color, sim, blend));
        }
        chain.push("format=rgba".to_string());
        let scale = o.scale.clamp(0.05, 4.0);
        if (scale - 1.0).abs() > 1e-3 {
            chain.push(format!("scale=iw*{s:.4}:ih*{s:.4}", s = scale));
        }
        let opacity = o.opacity.clamp(0.0, 1.0);
        if opacity < 0.999 {
            chain.push(format!("colorchannelmixer=aa={:.3}", opacity));
        }
        let start = o.start.max(0.0);
        chain.push(format!("setpts=PTS-STARTPTS+{:.3}/TB", start));
        parts.push(format!("[{}:v]{}[ov{}]", i + 1, chain.join(","), i));
    }

    // Lay each prepared snippet onto the running base, in order.
    let mut last = "[0:v]".to_string();
    for (i, o) in overlays.iter().enumerate() {
        let (x, y) = overlay_xy(&o.position, MARGIN);
        let start = o.start.max(0.0);
        let end = start + o.duration.max(0.05);
        let out = if i + 1 == overlays.len() {
            "[vout]".to_string()
        } else {
            format!("[base{}]", i)
        };
        parts.push(format!(
            "{}[ov{}]overlay=x={}:y={}:enable='between(t,{:.3},{:.3})':eof_action=pass{}",
            last, i, x, y, start, end, out
        ));
        last = out;
    }

    parts.join(";")
}

/// Composite one or more overlay snippets onto `base`, writing `out_path`. The
/// video is re-encoded (overlays require a pixel pass); the base audio is copied
/// untouched. Snippet audio is dropped — these are visual bumpers. Returns an
/// error (with ffmpeg stderr) on failure.
pub fn composite_overlays(
    base: &Path,
    overlays: &[OverlayPlacement],
    out_path: &Path,
) -> Result<(), String> {
    if overlays.is_empty() {
        return Err("No overlays to composite".to_string());
    }
    if let Some(parent) = out_path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }

    let filter = build_overlay_filter(overlays);

    let mut cmd = ffmpeg_cmd();
    cmd.arg("-i").arg(base);
    for o in overlays {
        // Loop animated GIFs and hold static images for the whole on-screen
        // window; real video overlays just play through once.
        let ext = o
            .path
            .extension()
            .and_then(|e| e.to_str())
            .map(|s| s.to_lowercase())
            .unwrap_or_default();
        if ext == "gif" {
            cmd.args(["-ignore_loop", "0"]);
        } else if is_static_image_ext(&ext) {
            cmd.args(["-loop", "1"]);
        }
        cmd.arg("-i").arg(&o.path);
    }
    let output = cmd
        .args(["-filter_complex", &filter])
        .args(["-map", "[vout]", "-map", "0:a?"])
        .args([
            "-c:v", "libx264",
            "-preset", "veryfast",
            "-crf", "20",
            "-pix_fmt", "yuv420p",
            "-c:a", "copy",
            "-movflags", "+faststart",
            "-y",
        ])
        .arg(out_path)
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::piped())
        .output()
        .map_err(|e| format!("Failed to run ffmpeg: {}", e))?;

    if output.status.success() && out_path.exists() {
        Ok(())
    } else {
        Err(format!(
            "ffmpeg failed to composite overlays: {}",
            stderr_tail(&output.stderr)
        ))
    }
}

/// Return the last ~600 chars of captured stderr, for surfacing ffmpeg errors.
fn stderr_tail(stderr: &[u8]) -> String {
    let s = String::from_utf8_lossy(stderr);
    let trimmed = s.trim();
    let start = trimmed.len().saturating_sub(600);
    trimmed[start..].to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn volume_expr_no_regions_is_constant_full() {
        let e = music_volume_expr(0.3, 0.08, &[], &[]);
        assert_eq!(e, "0.3000");
    }

    #[test]
    fn volume_expr_ducks_inside_speech() {
        let e = music_volume_expr(0.3, 0.08, &[(1.0, 2.0)], &[]);
        // duck check is the outermost clause, dropping to the ducked level.
        assert!(e.starts_with("if(between(t,1.000,2.000),0.0800,"));
        assert!(e.ends_with("0.3000)"));
    }

    #[test]
    fn volume_expr_fade_region_ramps_between_levels() {
        let e = music_volume_expr(0.3, 0.08, &[], &[(0.0, 4.0)]);
        // A fade region introduces a between() clause with an inner ramp.
        assert!(e.contains("between(t,0.000,4.000)"));
        assert!(e.contains("lt(t,"));
        assert!(e.contains("gt(t,"));
    }

    #[test]
    fn volume_expr_duck_takes_precedence_over_fade() {
        let e = music_volume_expr(0.3, 0.08, &[(1.0, 2.0)], &[(0.0, 4.0)]);
        // Speech ducking is evaluated first (outermost if).
        assert!(e.starts_with("if(between(t,1.000,2.000),0.0800,"));
    }

    #[test]
    fn volume_expr_clamps_duck_to_full() {
        // duck above full is clamped down to full (no louder-while-talking).
        let e = music_volume_expr(0.2, 0.9, &[(0.0, 1.0)], &[]);
        assert!(e.contains(",0.2000,"));
    }

    fn overlay(start: f32, dur: f32) -> OverlayPlacement {
        OverlayPlacement {
            path: PathBuf::from("sub.mp4"),
            start,
            duration: dur,
            chroma_color: "0xFFFFFF".to_string(),
            similarity: 0.1,
            blend: 0.05,
            scale: 1.0,
            opacity: 1.0,
            position: "center".to_string(),
        }
    }

    #[test]
    fn overlay_filter_keys_shifts_and_gates_a_single_overlay() {
        let f = build_overlay_filter(&[overlay(3.0, 5.0)]);
        // White is keyed out, the snippet is time-shifted to its start, and it's
        // only enabled during its window, ending at start+duration.
        assert!(f.contains("colorkey=0xFFFFFF:0.100:0.050"));
        assert!(f.contains("setpts=PTS-STARTPTS+3.000/TB"));
        assert!(f.contains("enable='between(t,3.000,8.000)'"));
        assert!(f.contains("eof_action=pass"));
        assert!(f.ends_with("[vout]"));
        // A single overlay composites straight onto the base video.
        assert!(f.contains("[0:v][ov0]overlay="));
    }

    #[test]
    fn overlay_filter_chains_multiple_overlays() {
        let f = build_overlay_filter(&[overlay(1.0, 2.0), overlay(5.0, 2.0)]);
        // First overlay writes an intermediate node that the second consumes.
        assert!(f.contains("[base0]"));
        assert!(f.contains("[base0][ov1]overlay="));
        assert!(f.ends_with("[vout]"));
    }

    #[test]
    fn overlay_filter_skips_key_when_color_empty() {
        let mut o = overlay(0.0, 2.0);
        o.chroma_color = "".to_string();
        let f = build_overlay_filter(&[o]);
        assert!(!f.contains("colorkey"));
    }

    #[test]
    fn overlay_xy_presets() {
        assert_eq!(overlay_xy("center", 40), ("(W-w)/2".to_string(), "(H-h)/2".to_string()));
        assert_eq!(overlay_xy("bottom_right", 40), ("W-w-40".to_string(), "H-h-40".to_string()));
        assert_eq!(overlay_xy("top", 40).1, "40".to_string());
    }
}
