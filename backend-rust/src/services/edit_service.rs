//! Video-edit pipeline ("Edit & Create Video"): given a production's raw takes
//! and a script, transcribe each take (with word-level timestamps), ask the
//! configured LLM to assemble an edit decision list (best take per scene, in
//! script order, warm-ups trimmed), write that EDL to disk as JSON, then stitch
//! the chosen ranges into one final clip with ffmpeg.
//!
//! Like the scanner, progress is tracked in an in-memory map keyed by a job id
//! and polled by the frontend. The final EDL and video path are also persisted
//! to the `production_edits` table so a completed edit can be reopened later.

use chrono::{NaiveDateTime, Utc};
use diesel::prelude::*;
use log::{error, info, warn};
use serde::Serialize;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use uuid::Uuid;

use crate::config::AiSettings;
use crate::db::DbPool;
use crate::models::NewProductionEdit;
use crate::schema::production_edits;
use crate::services::ai_service::{self, TranscriptWord};
use crate::services::{ffmpeg_service, production_service};

pub type EditJobMap = Arc<Mutex<HashMap<String, EditJobProgress>>>;

/// Live progress for one edit pipeline run.
#[derive(Debug, Clone, Serialize)]
pub struct EditJobProgress {
    pub job_id: String,
    pub production_id: i32,
    /// "in_progress" | "completed" | "failed".
    pub status: String,
    /// Current pipeline stage: "transcribing" | "planning" | "stitching" | …
    pub stage: String,
    /// Human-readable detail for the current stage.
    pub message: String,
    pub processed: i64,
    pub total: i64,
    pub logs: Vec<String>,
    pub error: Option<String>,
    /// Populated on success: the resolved edit decision list.
    pub edl: Option<serde_json::Value>,
    pub output_path: Option<String>,
    pub edl_path: Option<String>,
    pub start_time: NaiveDateTime,
    pub end_time: Option<NaiveDateTime>,
}

impl EditJobProgress {
    fn new(job_id: String, production_id: i32) -> Self {
        Self {
            job_id,
            production_id,
            status: "in_progress".to_string(),
            stage: "starting".to_string(),
            message: "Preparing…".to_string(),
            processed: 0,
            total: 0,
            logs: Vec::new(),
            error: None,
            edl: None,
            output_path: None,
            edl_path: None,
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
        serde_json::json!({
            "job_id": self.job_id,
            "production_id": self.production_id,
            "status": self.status,
            "stage": self.stage,
            "message": self.message,
            "processed": self.processed,
            "total": self.total,
            "logs": self.logs,
            "error": self.error,
            "edl": self.edl,
            "output_path": self.output_path,
            "edl_path": self.edl_path,
            "elapsed_seconds": elapsed,
            "start_time": self.start_time.format("%Y-%m-%dT%H:%M:%S").to_string(),
            "end_time": self.end_time.map(|t| t.format("%Y-%m-%dT%H:%M:%S").to_string()),
        })
    }
}

/// Mutate the progress entry for `job_id` under the map lock, if it still exists.
fn update<F: FnOnce(&mut EditJobProgress)>(map: &EditJobMap, job_id: &str, f: F) {
    if let Ok(mut m) = map.lock() {
        if let Some(p) = m.get_mut(job_id) {
            f(p);
        }
    }
}

fn set_stage(map: &EditJobMap, job_id: &str, stage: &str, message: &str) {
    info!("[edit {}] {}: {}", job_id, stage, message);
    update(map, job_id, |p| {
        p.stage = stage.to_string();
        p.message = message.to_string();
        push_log(p, message);
    });
}

fn log_msg(map: &EditJobMap, job_id: &str, message: &str) {
    update(map, job_id, |p| push_log(p, message));
}

fn push_log(p: &mut EditJobProgress, message: &str) {
    p.logs.push(format!(
        "{} — {}",
        Utc::now().naive_utc().format("%H:%M:%S"),
        message
    ));
    // Keep the log bounded.
    let len = p.logs.len();
    if len > 200 {
        p.logs.drain(0..len - 200);
    }
}

/// Lightweight per-take metadata used while planning and stitching.
struct TakeMeta {
    video_id: i32,
    filename: String,
    file_path: String,
    duration: f32,
    resolution: Option<String>,
    fps: Option<f32>,
}

/// A clip selected for the final cut, resolved to a real file + clamped range.
#[derive(Clone)]
struct ResolvedClipInternal {
    video_id: i32,
    filename: String,
    file_path: String,
    start: f32,
    end: f32,
    reason: Option<String>,
}

/// Start a background edit pipeline for a production. Returns the job id used to
/// poll progress. Fails fast if the production has no videos.
#[allow(clippy::too_many_arguments)]
pub fn start_edit(
    production_id: i32,
    script: String,
    instructions: Option<String>,
    pool: DbPool,
    ai: AiSettings,
    edits_dir: PathBuf,
    edit_map: EditJobMap,
) -> Result<String, String> {
    let mut conn = pool.get().map_err(|e| format!("Database connection failed: {}", e))?;

    let production = production_service::get_production(&mut conn, production_id)
        .ok_or_else(|| format!("Production not found: {}", production_id))?;

    let videos = production_service::get_production_videos(&mut conn, production_id);
    if videos.is_empty() {
        return Err("This production has no videos. Add the raw takes to it first.".to_string());
    }
    if script.trim().is_empty() {
        return Err("A script is required to plan the edit.".to_string());
    }

    let takes: Vec<TakeMeta> = videos
        .into_iter()
        .map(|v| TakeMeta {
            video_id: v.id,
            filename: v.filename,
            file_path: v.file_path,
            duration: v.duration.unwrap_or(0.0),
            resolution: v.resolution,
            fps: v.fps,
        })
        .collect();

    let job_id = Uuid::new_v4().to_string();
    let progress = EditJobProgress::new(job_id.clone(), production_id);
    {
        let mut map = edit_map.lock().unwrap();
        map.insert(job_id.clone(), progress);
    }

    let production_title = production.title;
    let job_id_thread = job_id.clone();
    std::thread::Builder::new()
        .name(format!("vm-edit-{}", job_id))
        .spawn(move || {
            run_edit(
                &job_id_thread,
                production_id,
                production_title,
                script,
                instructions,
                takes,
                pool,
                ai,
                edits_dir,
                edit_map,
            );
        })
        .map_err(|e| format!("Failed to spawn edit thread: {}", e))?;

    Ok(job_id)
}

#[allow(clippy::too_many_arguments)]
fn run_edit(
    job_id: &str,
    production_id: i32,
    production_title: String,
    script: String,
    instructions: Option<String>,
    takes: Vec<TakeMeta>,
    pool: DbPool,
    ai: AiSettings,
    edits_dir: PathBuf,
    edit_map: EditJobMap,
) {
    match run_edit_inner(
        job_id,
        production_id,
        &production_title,
        &script,
        instructions.as_deref(),
        &takes,
        &ai,
        &edits_dir,
        &edit_map,
    ) {
        Ok((edl_value, edl_path, output_path)) => {
            persist_edit(
                &pool,
                production_id,
                "completed",
                &script,
                instructions.as_deref(),
                Some(&edl_value),
                Some(&output_path),
                Some(&edl_path),
                None,
                &ai,
            );
            update(&edit_map, job_id, |p| {
                p.status = "completed".to_string();
                p.stage = "completed".to_string();
                p.message = "Final video created.".to_string();
                p.edl = Some(edl_value);
                p.edl_path = Some(edl_path.to_string_lossy().to_string());
                p.output_path = Some(output_path.to_string_lossy().to_string());
                p.end_time = Some(Utc::now().naive_utc());
                push_log(p, "Done.");
            });
        }
        Err(e) => {
            error!("[edit {}] failed: {}", job_id, e);
            persist_edit(
                &pool,
                production_id,
                "failed",
                &script,
                instructions.as_deref(),
                None,
                None,
                None,
                Some(&e),
                &ai,
            );
            update(&edit_map, job_id, |p| {
                p.status = "failed".to_string();
                p.stage = "failed".to_string();
                p.message = e.clone();
                p.error = Some(e);
                p.end_time = Some(Utc::now().naive_utc());
            });
        }
    }
}

/// The pipeline proper. Returns (edl json value, edl path, output video path).
fn run_edit_inner(
    job_id: &str,
    production_id: i32,
    production_title: &str,
    script: &str,
    instructions: Option<&str>,
    takes: &[TakeMeta],
    ai: &AiSettings,
    edits_dir: &std::path::Path,
    edit_map: &EditJobMap,
) -> Result<(serde_json::Value, PathBuf, PathBuf), String> {
    let prod_dir = edits_dir.join(format!("production-{}", production_id));
    std::fs::create_dir_all(&prod_dir).map_err(|e| format!("Failed to create output dir: {}", e))?;

    // A dedicated current-thread Tokio runtime for the async HTTP work
    // (transcription + LLM). ffmpeg calls below are synchronous.
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|e| format!("Failed to start async runtime: {}", e))?;

    // --- Stage 1: transcribe each take -------------------------------------
    update(edit_map, job_id, |p| {
        p.total = takes.len() as i64;
        p.processed = 0;
    });
    set_stage(edit_map, job_id, "transcribing", &format!("Transcribing {} take(s) with {}…", takes.len(), ai.transcription_provider));

    let audio_root = std::env::temp_dir().join(format!("video-manager-edit/{}", job_id));
    let mut transcripts: HashMap<i32, Vec<TranscriptWord>> = HashMap::new();
    let mut plain_texts: HashMap<i32, String> = HashMap::new();

    for (i, take) in takes.iter().enumerate() {
        log_msg(edit_map, job_id, &format!("Transcribing {} ({}/{})", take.filename, i + 1, takes.len()));

        let input = PathBuf::from(&take.file_path);
        if !input.exists() {
            return Err(format!("Take file no longer exists on disk: {}", take.file_path));
        }

        let audio = ffmpeg_service::extract_audio(&input, &audio_root)
            .map_err(|e| format!("Audio extraction failed for {}: {}", take.filename, e))?;

        let result = rt.block_on(ai_service::transcribe_timed(&audio, ai));
        let _ = std::fs::remove_file(&audio);

        let timed = result.map_err(|e| format!("Transcription failed for {}: {}", take.filename, e))?;
        plain_texts.insert(take.video_id, timed.text.clone());
        transcripts.insert(take.video_id, timed.words);

        update(edit_map, job_id, |p| p.processed = (i + 1) as i64);
    }
    let _ = std::fs::remove_dir_all(&audio_root);

    // --- Stage 2: plan the edit with the LLM -------------------------------
    set_stage(edit_map, job_id, "planning", &format!("Asking {} to assemble the edit…", ai.text_provider));

    let transcripts_block = format_transcripts(takes, &transcripts, &plain_texts);
    let prompt = build_edit_prompt(&ai.edit_prompt, script, instructions, &transcripts_block);

    let raw = rt
        .block_on(ai_service::complete(&prompt, ai, 8192))
        .map_err(|e| format!("Edit planning failed: {}", e))?;

    let resolved = resolve_plan(&raw, takes)?;
    if resolved.is_empty() {
        return Err("The model did not return any usable clips for this script.".to_string());
    }
    log_msg(edit_map, job_id, &format!("Planned {} clip(s) across the timeline.", resolved.len()));

    // Build the EDL JSON deliverable (grouped by scene).
    let output_path = prod_dir.join(format!("final-{}.mp4", job_id));
    let edl_value = build_edl_json(
        production_id,
        production_title,
        ai,
        &resolved,
        &output_path,
    );
    let edl_path = prod_dir.join(format!("edit-{}.json", job_id));
    let pretty = serde_json::to_string_pretty(&edl_value).unwrap_or_else(|_| "{}".to_string());
    std::fs::write(&edl_path, &pretty).map_err(|e| format!("Failed to write EDL JSON: {}", e))?;
    // Also keep a stable "latest.json" for convenience.
    let _ = std::fs::write(prod_dir.join("latest.json"), &pretty);
    log_msg(edit_map, job_id, &format!("Wrote edit decision list to {}", edl_path.display()));

    // --- Stage 3: stitch the clips with ffmpeg -----------------------------
    let (target_w, target_h, target_fps) = resolve_target_spec(&resolved, takes);
    set_stage(
        edit_map,
        job_id,
        "stitching",
        &format!("Cutting {} clip(s) at {}x{} {:.0}fps…", resolved.len(), target_w, target_h, target_fps),
    );
    update(edit_map, job_id, |p| {
        p.total = resolved.len() as i64;
        p.processed = 0;
    });

    let work_dir = prod_dir.join(format!("work-{}", job_id));
    std::fs::create_dir_all(&work_dir).map_err(|e| format!("Failed to create work dir: {}", e))?;

    let mut segments: Vec<PathBuf> = Vec::with_capacity(resolved.len());
    for (i, clip) in resolved.iter().enumerate() {
        log_msg(
            edit_map,
            job_id,
            &format!("Cutting clip {}/{}: {} [{:.2}s–{:.2}s]", i + 1, resolved.len(), clip.filename, clip.start, clip.end),
        );
        let seg = work_dir.join(format!("seg_{:04}.mp4", i));
        ffmpeg_service::extract_clip_segment(
            std::path::Path::new(&clip.file_path),
            clip.start,
            clip.end,
            target_w,
            target_h,
            target_fps,
            &seg,
        )?;
        segments.push(seg);
        update(edit_map, job_id, |p| p.processed = (i + 1) as i64);
    }

    set_stage(edit_map, job_id, "stitching", "Joining clips into the final video…");
    ffmpeg_service::concat_clips(&segments, &output_path)?;

    // Best-effort cleanup of intermediate segments.
    let _ = std::fs::remove_dir_all(&work_dir);

    if !output_path.exists() {
        return Err("ffmpeg reported success but the final video is missing.".to_string());
    }
    log_msg(edit_map, job_id, &format!("Final video: {}", output_path.display()));

    Ok((edl_value, edl_path, output_path))
}

/// Substitute the `{script}`, `{instructions}`, and `{transcripts}` tokens into
/// the user-configurable edit prompt template. Missing tokens are appended so
/// the model always has the source material.
fn build_edit_prompt(
    template: &str,
    script: &str,
    instructions: Option<&str>,
    transcripts_block: &str,
) -> String {
    let mut prompt = if template.contains("{script}") || template.contains("{transcripts}") {
        template
            .replace("{script}", script)
            .replace("{transcripts}", transcripts_block)
    } else {
        format!(
            "{}\n\nSCRIPT:\n\"\"\"\n{}\n\"\"\"\n\nRAW TAKES:\n\"\"\"\n{}\n\"\"\"",
            template, script, transcripts_block
        )
    };

    if let Some(extra) = instructions.map(str::trim).filter(|s| !s.is_empty()) {
        if prompt.contains("{instructions}") {
            prompt = prompt.replace("{instructions}", extra);
        } else {
            prompt.push_str(&format!("\n\nADDITIONAL INSTRUCTIONS FROM THE CREATOR:\n\"\"\"\n{}\n\"\"\"", extra));
        }
    } else {
        prompt = prompt.replace("{instructions}", "");
    }

    prompt
}

/// Render every take as a labeled block of timestamped segments for the prompt.
fn format_transcripts(
    takes: &[TakeMeta],
    words_by_id: &HashMap<i32, Vec<TranscriptWord>>,
    text_by_id: &HashMap<i32, String>,
) -> String {
    let mut out = String::new();
    for take in takes {
        out.push_str(&format!(
            "### Take | video_id={} | file=\"{}\" | duration={:.2}s\n",
            take.video_id, take.filename, take.duration
        ));

        let words = words_by_id.get(&take.video_id).map(|w| w.as_slice()).unwrap_or(&[]);
        if words.is_empty() {
            let text = text_by_id.get(&take.video_id).cloned().unwrap_or_default();
            out.push_str("(no word-level timestamps; use 0.00 to duration for ranges)\n");
            out.push_str(&format!("TRANSCRIPT: {}\n\n", text.trim()));
            continue;
        }

        let segments = segment_words(words);
        for (start, end, text) in segments.iter().take(600) {
            out.push_str(&format!("[{:.2}-{:.2}] {}\n", start, end, text));
        }
        if segments.len() > 600 {
            out.push_str("… (transcript truncated)\n");
        }
        out.push('\n');
    }
    out
}

/// Group consecutive words into short phrase segments, breaking on long pauses
/// or after ~10 words, so the prompt stays compact while preserving usable cut
/// points.
fn segment_words(words: &[TranscriptWord]) -> Vec<(f32, f32, String)> {
    const MAX_WORDS: usize = 10;
    const GAP: f32 = 0.6;

    let mut segments: Vec<(f32, f32, String)> = Vec::new();
    let mut cur_start = 0.0f32;
    let mut cur_end = 0.0f32;
    let mut cur_words: Vec<&str> = Vec::new();

    for (i, w) in words.iter().enumerate() {
        let gap_break = !cur_words.is_empty() && (w.start - cur_end) > GAP;
        let len_break = cur_words.len() >= MAX_WORDS;

        if gap_break || len_break {
            segments.push((cur_start, cur_end, cur_words.join(" ")));
            cur_words.clear();
        }

        if cur_words.is_empty() {
            cur_start = w.start;
        }
        cur_words.push(w.text.trim());
        cur_end = w.end;

        if i == words.len() - 1 && !cur_words.is_empty() {
            segments.push((cur_start, cur_end, cur_words.join(" ")));
        }
    }
    segments
}

/// Parse the model's JSON and resolve each clip against the real takes: drop
/// unknown video ids, clamp ranges to the take duration, drop empty/invalid
/// ranges. Returns the ordered clip list for stitching.
fn resolve_plan(raw: &str, takes: &[TakeMeta]) -> Result<Vec<ResolvedClipInternal>, String> {
    let json_str = extract_json(raw);
    let value: serde_json::Value = serde_json::from_str(json_str)
        .map_err(|e| format!("Could not parse the model's edit plan as JSON: {} — raw: {}", e, truncate(raw, 500)))?;

    let by_id: HashMap<i32, &TakeMeta> = takes.iter().map(|t| (t.video_id, t)).collect();

    let scenes = value["scenes"].as_array().cloned().unwrap_or_default();
    let mut resolved: Vec<ResolvedClipInternal> = Vec::new();

    for scene in &scenes {
        let clips = scene["clips"].as_array().cloned().unwrap_or_default();
        for clip in &clips {
            let vid = match clip["video_id"].as_i64() {
                Some(v) => v as i32,
                None => continue,
            };
            let take = match by_id.get(&vid) {
                Some(t) => *t,
                None => continue,
            };
            let mut start = read_f32(&clip["start"]).unwrap_or(0.0).max(0.0);
            let mut end = read_f32(&clip["end"]).unwrap_or(take.duration);

            // Clamp to the take's duration when known.
            if take.duration > 0.0 {
                end = end.min(take.duration);
                start = start.min(take.duration);
            }
            if end <= start {
                continue;
            }
            let reason = clip["reason"].as_str().map(|s| s.to_string());

            resolved.push(ResolvedClipInternal {
                video_id: vid,
                filename: take.filename.clone(),
                file_path: take.file_path.clone(),
                start,
                end,
                reason,
            });
        }
    }

    Ok(resolved)
}

/// Build the EDL JSON deliverable, grouped by scene (as returned by the model)
/// but using only the validated clips.
fn build_edl_json(
    production_id: i32,
    production_title: &str,
    ai: &AiSettings,
    resolved: &[ResolvedClipInternal],
    output_path: &std::path::Path,
) -> serde_json::Value {
    let clips: Vec<serde_json::Value> = resolved
        .iter()
        .enumerate()
        .map(|(i, c)| {
            serde_json::json!({
                "order": i + 1,
                "video_id": c.video_id,
                "filename": c.filename,
                "start": round2(c.start),
                "end": round2(c.end),
                "duration": round2(c.end - c.start),
                "reason": c.reason,
            })
        })
        .collect();

    serde_json::json!({
        "production_id": production_id,
        "production_title": production_title,
        "generated_at": Utc::now().naive_utc().format("%Y-%m-%dT%H:%M:%S").to_string(),
        "transcription_provider": ai.transcription_provider,
        "text_provider": ai.text_provider,
        "text_model": ai.text_model,
        "clips": clips,
        "output": output_path.file_name().map(|f| f.to_string_lossy().to_string()),
    })
}

/// Pick the output spec from the first clip's take: prefer the values already
/// stored on the take, fall back to an ffprobe, then to 1080x1920 / 30fps.
fn resolve_target_spec(resolved: &[ResolvedClipInternal], takes: &[TakeMeta]) -> (i32, i32, f32) {
    let first = match resolved.first() {
        Some(c) => c,
        None => return (1080, 1920, 30.0),
    };

    // Use the resolution/fps captured at scan time when available.
    if let Some(take) = takes.iter().find(|t| t.video_id == first.video_id) {
        if let (Some((w, h)), Some(fps)) = (
            take.resolution.as_deref().and_then(parse_resolution),
            take.fps.filter(|f| *f > 0.0),
        ) {
            return (w, h, fps);
        }
    }

    // Otherwise probe the file directly.
    if let Some(meta) = ffmpeg_service::extract_metadata(std::path::Path::new(&first.file_path)) {
        let (w, h) = meta
            .resolution
            .as_deref()
            .and_then(parse_resolution)
            .unwrap_or((1080, 1920));
        let fps = meta.fps.filter(|f| *f > 0.0).unwrap_or(30.0);
        return (w, h, fps);
    }

    (1080, 1920, 30.0)
}

fn parse_resolution(res: &str) -> Option<(i32, i32)> {
    let (w, h) = res.split_once('x')?;
    Some((w.trim().parse().ok()?, h.trim().parse().ok()?))
}

/// Best-effort extraction of a JSON object from a model response that may be
/// wrapped in markdown fences or prose.
fn extract_json(text: &str) -> &str {
    let trimmed = text.trim();
    if let (Some(start), Some(end)) = (trimmed.find('{'), trimmed.rfind('}')) {
        if end >= start {
            return &trimmed[start..=end];
        }
    }
    trimmed
}

fn read_f32(v: &serde_json::Value) -> Option<f32> {
    v.as_f64()
        .map(|n| n as f32)
        .or_else(|| v.as_str().and_then(|s| s.trim().parse::<f32>().ok()))
}

fn round2(v: f32) -> f32 {
    (v * 100.0).round() / 100.0
}

fn truncate(s: &str, max: usize) -> String {
    if s.len() <= max {
        s.to_string()
    } else {
        format!("{}…", &s[..max])
    }
}

/// Persist a completed or failed edit attempt to the database.
#[allow(clippy::too_many_arguments)]
fn persist_edit(
    pool: &DbPool,
    production_id: i32,
    status: &str,
    script: &str,
    instructions: Option<&str>,
    edl_value: Option<&serde_json::Value>,
    output_path: Option<&PathBuf>,
    edl_path: Option<&PathBuf>,
    error: Option<&str>,
    ai: &AiSettings,
) {
    let mut conn = match pool.get() {
        Ok(c) => c,
        Err(e) => {
            warn!("Could not persist edit result: {}", e);
            return;
        }
    };

    let record = NewProductionEdit {
        production_id,
        status: status.to_string(),
        script: Some(script.to_string()),
        instructions: instructions.map(|s| s.to_string()),
        edl_json: edl_value.map(|v| v.to_string()),
        output_path: output_path.map(|p| p.to_string_lossy().to_string()),
        edl_path: edl_path.map(|p| p.to_string_lossy().to_string()),
        error: error.map(|s| s.to_string()),
        transcription_provider: Some(ai.transcription_provider.clone()),
        text_provider: Some(ai.text_provider.clone()),
        text_model: Some(ai.text_model.clone()),
        created_at: Utc::now().naive_utc(),
    };

    if let Err(e) = diesel::insert_into(production_edits::table)
        .values(&record)
        .execute(&mut conn)
    {
        warn!("Failed to insert production_edit row: {}", e);
    }
}

/// Fetch progress for a job id.
pub fn get_edit_progress(edit_map: &EditJobMap, job_id: &str) -> Option<serde_json::Value> {
    let map = edit_map.lock().unwrap();
    map.get(job_id).map(|p| p.to_response())
}

/// The most recent persisted edit attempt for a production, if any.
pub fn get_latest_edit(
    conn: &mut diesel::sqlite::SqliteConnection,
    production_id: i32,
) -> Option<crate::models::ProductionEdit> {
    production_edits::table
        .filter(production_edits::production_id.eq(production_id))
        .order(production_edits::id.desc())
        .first(conn)
        .ok()
}
