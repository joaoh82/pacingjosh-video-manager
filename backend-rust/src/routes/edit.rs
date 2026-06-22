use actix_web::{delete, get, post, web, HttpResponse};
use base64::{engine::general_purpose::STANDARD, Engine};
use serde::Deserialize;
use std::path::Path;

use crate::config::ConfigManager;
use crate::db::DbPool;
use crate::models::ProductionEditResponse;
use crate::services::ai_service;
use crate::services::edit_service::{self, EditJobMap, EditOptions};
use crate::services::ffmpeg_service;

fn default_captions() -> bool { true }
fn default_music_volume() -> f32 { 0.3 }
fn default_music_duck_volume() -> f32 { 0.08 }
fn default_music_min_gap() -> f32 { 1.5 }
fn default_tighten_gap() -> f32 { 1.5 }
fn default_enhance_intensity() -> f32 { 0.6 }

#[derive(Deserialize)]
pub struct StartEditRequest {
    /// The script for the video (markdown / plain text).
    pub script: String,
    /// Optional extra guidance for the editor (warm-up phrase to cut, ordering
    /// notes, tone, etc.).
    #[serde(default)]
    pub instructions: Option<String>,
    /// Optional directory for the final video. Empty → app-data edits folder.
    #[serde(default)]
    pub output_dir: Option<String>,
    /// Optional filename for the final video. Empty → derived from the title.
    #[serde(default)]
    pub output_name: Option<String>,
    /// Burn the spoken words into the video as captions.
    #[serde(default = "default_captions")]
    pub captions: bool,
    /// Optional background-music file path, mixed under the speech.
    #[serde(default)]
    pub music_path: Option<String>,
    /// Music volume when no one is talking, 0.0–1.0.
    #[serde(default = "default_music_volume")]
    pub music_volume: f32,
    /// Music volume while the voice is talking (ducked level), 0.0–1.0.
    #[serde(default = "default_music_duck_volume")]
    pub music_duck_volume: f32,
    /// Only swell the music back up in pauses longer than this many seconds.
    #[serde(default = "default_music_min_gap")]
    pub music_min_gap: f32,
    /// Tighten the cut by removing long silences/filler within each clip.
    #[serde(default)]
    pub tighten: bool,
    /// When tightening, remove silence/filler gaps longer than this many seconds.
    #[serde(default = "default_tighten_gap")]
    pub tighten_gap: f32,
    /// "Enhance voice": take (video) ids whose audio should be cleaned up
    /// (wind/rumble, background hiss, mouth clicks). Empty → no enhancement.
    #[serde(default)]
    pub enhance_voice: Vec<i32>,
    /// Voice-enhancement intensity, 0.0–1.0 (how aggressively to remove noise).
    #[serde(default = "default_enhance_intensity")]
    pub enhance_voice_intensity: f32,
}

/// Kick off the edit pipeline for a production. Returns a job id to poll.
#[post("/productions/{production_id}/edit")]
async fn start_edit(
    pool: web::Data<DbPool>,
    config: web::Data<ConfigManager>,
    edit_map: web::Data<EditJobMap>,
    path: web::Path<i32>,
    body: web::Json<StartEditRequest>,
) -> HttpResponse {
    let production_id = path.into_inner();
    let ai = config.get_ai_settings();

    let opts = EditOptions {
        script: body.script.clone(),
        instructions: body.instructions.clone(),
        output_dir: body.output_dir.clone(),
        output_name: body.output_name.clone(),
        captions: body.captions,
        music_path: body.music_path.clone(),
        music_volume: body.music_volume,
        music_duck_volume: body.music_duck_volume,
        music_min_gap: body.music_min_gap,
        tighten: body.tighten,
        tighten_gap: body.tighten_gap,
        enhance_voice_video_ids: body.enhance_voice.clone(),
        enhance_voice_intensity: body.enhance_voice_intensity,
    };

    match edit_service::start_edit(
        production_id,
        opts,
        pool.get_ref().clone(),
        ai,
        edit_map.get_ref().clone(),
    ) {
        Ok(job_id) => HttpResponse::Ok().json(serde_json::json!({
            "status": "started",
            "job_id": job_id,
            "message": format!("Edit pipeline started for production {}", production_id),
        })),
        Err(e) => HttpResponse::BadRequest().json(serde_json::json!({ "detail": e })),
    }
}

/// Poll live progress for a running (or finished) edit job.
#[get("/edit/status/{job_id}")]
async fn edit_status(
    edit_map: web::Data<EditJobMap>,
    path: web::Path<String>,
) -> HttpResponse {
    let job_id = path.into_inner();
    match edit_service::get_edit_progress(edit_map.get_ref(), &job_id) {
        Some(progress) => HttpResponse::Ok().json(progress),
        None => HttpResponse::NotFound().json(serde_json::json!({
            "detail": format!("Edit job not found: {}", job_id),
        })),
    }
}

/// The latest persisted edit result for a production (EDL + output path), or
/// `null` if the production has never been edited.
#[get("/productions/{production_id}/edit")]
async fn get_latest_edit(
    pool: web::Data<DbPool>,
    path: web::Path<i32>,
) -> HttpResponse {
    let mut conn = pool.get().expect("Failed to get DB connection");
    let production_id = path.into_inner();

    match edit_service::get_latest_edit(&mut conn, production_id) {
        Some(edit) => HttpResponse::Ok().json(ProductionEditResponse::from(edit)),
        None => HttpResponse::Ok().json(serde_json::Value::Null),
    }
}

/// Full edit history for a production (newest first) — script, EDL, activity
/// log, output path, and error per run.
#[get("/productions/{production_id}/edits")]
async fn list_edits(
    pool: web::Data<DbPool>,
    path: web::Path<i32>,
) -> HttpResponse {
    let mut conn = pool.get().expect("Failed to get DB connection");
    let production_id = path.into_inner();

    let edits: Vec<ProductionEditResponse> = edit_service::get_all_edits(&mut conn, production_id)
        .into_iter()
        .map(ProductionEditResponse::from)
        .collect();
    HttpResponse::Ok().json(edits)
}

/// Reveal the latest final video for a production in the OS file browser.
#[post("/productions/{production_id}/edit/reveal")]
async fn reveal_edit_output(
    pool: web::Data<DbPool>,
    path: web::Path<i32>,
) -> HttpResponse {
    let production_id = path.into_inner();
    let output = {
        let mut conn = pool.get().expect("Failed to get DB connection");
        edit_service::get_latest_edit(&mut conn, production_id).and_then(|e| e.output_path)
    };
    reveal_output(output)
}

#[derive(Deserialize)]
pub struct RerenderRequest {
    /// Regions (seconds, final timeline) to mute the music in — the music
    /// "bursts" the user removed on the timeline.
    #[serde(default)]
    pub mute: Vec<MuteRegion>,
}

#[derive(Deserialize)]
pub struct MuteRegion {
    pub start: f32,
    pub end: f32,
}

/// Re-render a run with timeline edits (muted music regions) into a new version,
/// reusing the saved cut/transcripts (no transcription or LLM cost).
#[post("/edits/{edit_id}/rerender")]
async fn rerender_edit(
    pool: web::Data<DbPool>,
    config: web::Data<ConfigManager>,
    edit_map: web::Data<EditJobMap>,
    path: web::Path<i32>,
    body: web::Json<RerenderRequest>,
) -> HttpResponse {
    let edit_id = path.into_inner();
    let mute: Vec<(f32, f32)> = body.mute.iter().map(|m| (m.start, m.end)).collect();

    match edit_service::start_rerender(
        edit_id,
        mute,
        pool.get_ref().clone(),
        config.get_ai_settings(),
        edit_map.get_ref().clone(),
    ) {
        Ok(job_id) => HttpResponse::Ok().json(serde_json::json!({
            "status": "started",
            "job_id": job_id,
            "message": "Re-render started",
        })),
        Err(e) => HttpResponse::BadRequest().json(serde_json::json!({ "detail": e })),
    }
}

#[derive(Deserialize, Default)]
#[serde(default)]
pub struct GenerateCopyRequest {
    pub regenerate: bool,
}

/// Generate (or return cached) YouTube copy — title options, description, tags,
/// and thumbnail text — from a run's final-cut transcript.
#[post("/edits/{edit_id}/copy")]
async fn generate_copy(
    pool: web::Data<DbPool>,
    config: web::Data<ConfigManager>,
    path: web::Path<i32>,
    body: web::Json<GenerateCopyRequest>,
) -> HttpResponse {
    let edit_id = path.into_inner();

    let edit = {
        let mut conn = pool.get().expect("Failed to get DB connection");
        edit_service::get_edit_by_id(&mut conn, edit_id)
    };
    let edit = match edit {
        Some(e) => e,
        None => {
            return HttpResponse::NotFound()
                .json(serde_json::json!({ "detail": format!("Edit not found: {}", edit_id) }))
        }
    };

    // Return previously-generated copy unless a regenerate was requested.
    if !body.regenerate {
        if let Some(existing) = edit
            .copy_json
            .as_deref()
            .and_then(|s| serde_json::from_str::<serde_json::Value>(s).ok())
        {
            return HttpResponse::Ok().json(existing);
        }
    }

    let transcript = edit_service::final_transcript_for_edit(&edit);
    if transcript.trim().is_empty() {
        return HttpResponse::BadRequest().json(serde_json::json!({
            "detail": "No transcript is available for this run, so copy can't be generated. Re-run the edit with a transcription provider that returns word timestamps (ElevenLabs/Whisper).",
        }));
    }

    let ai = config.get_ai_settings();
    let copy = match ai_service::generate_youtube_copy(&transcript, &ai).await {
        Ok(c) => c,
        Err(e) => {
            return HttpResponse::InternalServerError().json(serde_json::json!({ "detail": e }))
        }
    };

    let copy_value = serde_json::to_value(&copy).unwrap_or(serde_json::Value::Null);
    {
        let mut conn = pool.get().expect("Failed to get DB connection");
        let _ = edit_service::save_copy(&mut conn, edit_id, &copy_value);
    }
    HttpResponse::Ok().json(copy_value)
}

// --- Thumbnail builder -------------------------------------------------------

#[derive(Deserialize)]
pub struct FrameQuery {
    pub t: Option<f32>,
}

/// Resolve a run's final video path if it still exists on disk.
fn edit_output_path(pool: &DbPool, edit_id: i32) -> Option<String> {
    let mut conn = pool.get().ok()?;
    edit_service::get_edit_by_id(&mut conn, edit_id)
        .and_then(|e| e.output_path)
        .filter(|p| Path::new(p).exists())
}

/// Grab a still frame (1280x720 JPEG) from a run's final video at `t` seconds.
#[get("/edits/{edit_id}/frame")]
async fn edit_frame(
    pool: web::Data<DbPool>,
    path: web::Path<i32>,
    query: web::Query<FrameQuery>,
) -> HttpResponse {
    let edit_id = path.into_inner();
    let out = match edit_output_path(pool.get_ref(), edit_id) {
        Some(p) => p,
        None => return HttpResponse::NotFound().json(serde_json::json!({ "detail": "Final video not found" })),
    };
    let t = query.t.unwrap_or(0.0);
    match web::block(move || ffmpeg_service::extract_frame(Path::new(&out), t, 1280, 720)).await {
        Ok(Ok(bytes)) => HttpResponse::Ok().content_type("image/jpeg").body(bytes),
        Ok(Err(e)) => HttpResponse::InternalServerError().json(serde_json::json!({ "detail": e })),
        Err(e) => HttpResponse::InternalServerError().json(serde_json::json!({ "detail": e.to_string() })),
    }
}

#[derive(Deserialize)]
pub struct RestyleRequest {
    pub t: Option<f32>,
    pub prompt: Option<String>,
}

/// AI-restyle a still frame with Gemini's image model (keeps the subject, no
/// text). Returns a PNG. Requires a Gemini API key.
#[post("/edits/{edit_id}/restyle")]
async fn restyle_frame(
    pool: web::Data<DbPool>,
    config: web::Data<ConfigManager>,
    path: web::Path<i32>,
    body: web::Json<RestyleRequest>,
) -> HttpResponse {
    let edit_id = path.into_inner();
    let out = match edit_output_path(pool.get_ref(), edit_id) {
        Some(p) => p,
        None => return HttpResponse::NotFound().json(serde_json::json!({ "detail": "Final video not found" })),
    };
    let t = body.t.unwrap_or(0.0);
    let frame = match web::block(move || ffmpeg_service::extract_frame(Path::new(&out), t, 1280, 720)).await {
        Ok(Ok(b)) => b,
        Ok(Err(e)) => return HttpResponse::InternalServerError().json(serde_json::json!({ "detail": e })),
        Err(e) => return HttpResponse::InternalServerError().json(serde_json::json!({ "detail": e.to_string() })),
    };
    let prompt = body
        .prompt
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .unwrap_or(
            "Turn this still into a dramatic, eye-catching YouTube thumbnail background. Keep the \
person and scene clearly recognizable; boost contrast and saturation, cinematic color grade, \
crisp and vibrant. Do NOT add any text or logos.",
        )
        .to_string();

    let ai = config.get_ai_settings();
    match ai_service::restyle_image(&frame, &prompt, &ai).await {
        Ok(png) => HttpResponse::Ok().content_type("image/png").body(png),
        Err(e) => HttpResponse::InternalServerError().json(serde_json::json!({ "detail": e })),
    }
}

#[derive(Deserialize)]
pub struct SaveThumbnailRequest {
    /// Base64 PNG (optionally a `data:image/png;base64,...` data URL).
    pub image: String,
}

/// Save a finished thumbnail PNG next to the run's final video.
#[post("/edits/{edit_id}/thumbnail")]
async fn save_thumbnail(
    pool: web::Data<DbPool>,
    path: web::Path<i32>,
    body: web::Json<SaveThumbnailRequest>,
) -> HttpResponse {
    let edit_id = path.into_inner();
    let out = match edit_output_path(pool.get_ref(), edit_id) {
        Some(p) => p,
        None => return HttpResponse::NotFound().json(serde_json::json!({ "detail": "Final video not found" })),
    };

    let b64 = body.image.split(',').last().unwrap_or("").trim();
    let bytes = match STANDARD.decode(b64) {
        Ok(b) => b,
        Err(e) => return HttpResponse::BadRequest().json(serde_json::json!({ "detail": format!("Bad image data: {}", e) })),
    };

    let out_path = Path::new(&out);
    let parent = out_path.parent().unwrap_or_else(|| Path::new("."));
    let stem = out_path.file_stem().map(|s| s.to_string_lossy().to_string()).unwrap_or_else(|| "thumbnail".to_string());
    let thumb_path = parent.join(format!("{}-thumbnail.png", stem));

    match std::fs::write(&thumb_path, &bytes) {
        Ok(()) => HttpResponse::Ok().json(serde_json::json!({ "path": thumb_path.to_string_lossy() })),
        Err(e) => HttpResponse::InternalServerError().json(serde_json::json!({ "detail": format!("Failed to save: {}", e) })),
    }
}

/// Delete a run: removes its files from disk (video, EDL JSON, version folder)
/// and its database row.
#[delete("/edits/{edit_id}")]
async fn delete_edit(
    pool: web::Data<DbPool>,
    path: web::Path<i32>,
) -> HttpResponse {
    let edit_id = path.into_inner();
    let mut conn = pool.get().expect("Failed to get DB connection");

    match edit_service::delete_edit(&mut conn, edit_id) {
        Ok(true) => HttpResponse::Ok().json(serde_json::json!({ "message": "Deleted" })),
        Ok(false) => HttpResponse::NotFound().json(serde_json::json!({
            "detail": format!("Edit not found: {}", edit_id),
        })),
        Err(e) => HttpResponse::InternalServerError().json(serde_json::json!({
            "detail": format!("Failed to delete edit: {}", e),
        })),
    }
}

/// Reveal a specific run's final video in the OS file browser.
#[post("/edits/{edit_id}/reveal")]
async fn reveal_edit_by_id(
    pool: web::Data<DbPool>,
    path: web::Path<i32>,
) -> HttpResponse {
    let edit_id = path.into_inner();
    let output = {
        let mut conn = pool.get().expect("Failed to get DB connection");
        edit_service::get_edit_by_id(&mut conn, edit_id).and_then(|e| e.output_path)
    };
    reveal_output(output)
}

/// Reveal a recorded output path: highlight the file if it still exists,
/// otherwise open its containing folder.
fn reveal_output(output: Option<String>) -> HttpResponse {
    let path = match output {
        Some(p) if !p.is_empty() => p,
        _ => {
            return HttpResponse::NotFound()
                .json(serde_json::json!({ "detail": "No output file recorded for this run." }))
        }
    };
    let pb = std::path::Path::new(&path);
    let (target, select) = if pb.exists() {
        (path.clone(), true)
    } else if let Some(parent) = pb.parent().filter(|d| d.exists()) {
        (parent.to_string_lossy().to_string(), false)
    } else {
        return HttpResponse::NotFound()
            .json(serde_json::json!({ "detail": "The output file no longer exists on disk." }));
    };

    match reveal_in_explorer(&target, select) {
        Ok(()) => HttpResponse::Ok().json(serde_json::json!({ "message": "Opened" })),
        Err(e) => HttpResponse::InternalServerError().json(serde_json::json!({
            "detail": format!("Failed to open: {}", e),
        })),
    }
}

/// Open a path in the OS file browser. When `select` is true and supported,
/// the file itself is highlighted; otherwise the folder is opened.
fn reveal_in_explorer(path: &str, select: bool) -> Result<(), String> {
    #[cfg(target_os = "windows")]
    {
        use std::os::windows::process::CommandExt;
        // Explorer needs backslashes; our stored paths can be mixed (the output
        // folder may have been typed/prefilled with forward slashes). Normalize,
        // then pass the canonical `/select,"<path>"` as a single raw argument —
        // Rust's normal arg quoting mangles it and Explorer falls back to opening
        // the user's home folder instead of the file.
        let win_path = path.replace('/', "\\");
        let raw = if select {
            format!("/select,\"{}\"", win_path)
        } else {
            format!("\"{}\"", win_path)
        };
        std::process::Command::new("explorer")
            .raw_arg(&raw)
            .spawn()
            .map_err(|e| e.to_string())?;
    }

    #[cfg(target_os = "macos")]
    {
        let mut cmd = std::process::Command::new("open");
        if select {
            cmd.args(["-R", path]);
        } else {
            cmd.arg(path);
        }
        cmd.spawn().map_err(|e| e.to_string())?;
    }

    #[cfg(target_os = "linux")]
    {
        // xdg-open does not support selecting a file; open the containing folder.
        let folder = if select {
            std::path::Path::new(path)
                .parent()
                .map(|p| p.to_string_lossy().to_string())
                .unwrap_or_else(|| path.to_string())
        } else {
            path.to_string()
        };
        std::process::Command::new("xdg-open")
            .arg(&folder)
            .spawn()
            .map_err(|e| e.to_string())?;
    }

    Ok(())
}

pub fn configure(cfg: &mut web::ServiceConfig) {
    cfg.service(start_edit)
        .service(edit_status)
        .service(list_edits)
        .service(get_latest_edit)
        .service(reveal_edit_output)
        .service(reveal_edit_by_id)
        .service(rerender_edit)
        .service(generate_copy)
        .service(edit_frame)
        .service(restyle_frame)
        .service(save_thumbnail)
        .service(delete_edit);
}
