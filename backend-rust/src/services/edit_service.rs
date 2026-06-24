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
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use uuid::Uuid;

use crate::config::AiSettings;
use crate::db::DbPool;
use crate::models::NewProductionEdit;
use crate::schema::production_edits;
use crate::services::ai_service::{self, TranscriptWord};
use crate::services::{ffmpeg_service, production_service, video_service};

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

/// Per-run options for the edit pipeline (everything beyond the production +
/// providers): the script, optional creator instructions, where to write the
/// final video, whether to burn in captions, and optional background music.
#[derive(Clone, Default, Serialize, Deserialize)]
pub struct EditOptions {
    pub script: String,
    pub instructions: Option<String>,
    /// Directory for the final video. Empty/None → the app-data edits folder.
    pub output_dir: Option<String>,
    /// Filename for the final video. Empty/None → derived from the title.
    pub output_name: Option<String>,
    /// Burn the spoken words into the video as captions (default true).
    pub captions: bool,
    /// Optional background-music file path, mixed under the speech.
    pub music_path: Option<String>,
    /// Music volume when no one is talking, 0.0–1.0.
    pub music_volume: f32,
    /// Music volume while the voice is talking, 0.0–1.0 (ducked level).
    pub music_duck_volume: f32,
    /// Only let the music swell back up in pauses LONGER than this many seconds;
    /// shorter pauses (thinking/breaths) stay ducked so the music doesn't pop in
    /// and out mid-sentence.
    pub music_min_gap: f32,
    /// Tighten the cut: within each chosen clip, drop long silences and filler
    /// ("um"/"uh") by splitting it into sub-clips of the actual speech.
    pub tighten: bool,
    /// When tightening, remove silence/filler gaps longer than this many seconds.
    pub tighten_gap: f32,
    /// "Enhance voice": the take (video) ids whose audio should be cleaned up
    /// (wind/rumble, background hiss, mouth clicks) while cutting their clips.
    /// Empty → no enhancement.
    #[serde(default)]
    pub enhance_voice_video_ids: Vec<i32>,
    /// Voice-enhancement intensity, 0.0–1.0 (how aggressively to remove noise).
    #[serde(default)]
    pub enhance_voice_intensity: f32,
}

/// A user edit to one clip of a saved cut, applied on re-render. Identified by
/// the clip `order` in the saved EDL. `source_start`/`source_end` (when set)
/// re-trim the clip to a sub-range of its take — e.g. shortening a long-running
/// take; `remove` drops it from the cut entirely.
#[derive(Clone, Debug, Default)]
pub struct ClipEdit {
    pub order: i32,
    /// Drop this clip from the re-rendered cut.
    pub remove: bool,
    /// Override the source range start (seconds into the take). None keeps it.
    pub source_start: Option<f32>,
    /// Override the source range end (seconds into the take). None keeps it.
    pub source_end: Option<f32>,
    /// Apply voice enhancement to this clip.
    pub enhance: bool,
}

/// Apply a clip edit to a saved EDL clip's source range, clamped to the take's
/// real `duration` (0 = unknown, no upper clamp). Returns `None` when the clip
/// is removed or the resulting range is empty/invalid. Pure for testability.
fn apply_clip_edit(
    saved_start: f32,
    saved_end: f32,
    duration: f32,
    edit: Option<&ClipEdit>,
) -> Option<(f32, f32)> {
    if edit.map(|e| e.remove).unwrap_or(false) {
        return None;
    }
    let mut start = edit.and_then(|e| e.source_start).unwrap_or(saved_start).max(0.0);
    let mut end = edit.and_then(|e| e.source_end).unwrap_or(saved_end);
    if duration > 0.0 {
        end = end.min(duration);
        start = start.min(duration);
    }
    if end <= start {
        None
    } else {
        Some((start, end))
    }
}

/// Start a background edit pipeline for a production. Returns the job id used to
/// poll progress. Fails fast if the production has no videos.
pub fn start_edit(
    production_id: i32,
    opts: EditOptions,
    pool: DbPool,
    ai: AiSettings,
    edit_map: EditJobMap,
) -> Result<String, String> {
    let mut conn = pool.get().map_err(|e| format!("Database connection failed: {}", e))?;

    let production = production_service::get_production(&mut conn, production_id)
        .ok_or_else(|| format!("Production not found: {}", production_id))?;

    let videos = production_service::get_production_videos(&mut conn, production_id);
    if videos.is_empty() {
        return Err("This production has no videos. Add the raw takes to it first.".to_string());
    }
    if opts.script.trim().is_empty() {
        return Err("A script is required to plan the edit.".to_string());
    }
    if opts.output_dir.as_deref().map(str::trim).unwrap_or("").is_empty() {
        return Err("Choose an output folder for the final video.".to_string());
    }
    if let Some(m) = opts.music_path.as_deref().filter(|s| !s.is_empty()) {
        if !std::path::Path::new(m).exists() {
            return Err(format!("Background music file not found: {}", m));
        }
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
                opts,
                takes,
                pool,
                ai,
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
    opts: EditOptions,
    takes: Vec<TakeMeta>,
    pool: DbPool,
    ai: AiSettings,
    edit_map: EditJobMap,
) {
    let result = run_edit_inner(
        job_id,
        production_id,
        &production_title,
        &opts,
        &takes,
        &ai,
        &edit_map,
    );

    // Snapshot the activity log so the persisted row matches what the user saw.
    let logs = snapshot_logs(&edit_map, job_id);
    let options_json = serde_json::to_string(&opts).ok();

    match result {
        Ok(ae) => {
            let persisted = persist_edit(
                &pool,
                production_id,
                "completed",
                &opts,
                Some(&ae.edl),
                Some(&ae.output_path),
                Some(&ae.edl_path),
                None,
                &logs,
                &ai,
                Some(&ae.transcripts_json),
                options_json.as_deref(),
            );
            update(&edit_map, job_id, |p| {
                p.status = "completed".to_string();
                p.stage = "completed".to_string();
                p.message = "Final video created.".to_string();
                p.edl = Some(ae.edl);
                p.edl_path = Some(ae.edl_path.to_string_lossy().to_string());
                p.output_path = Some(ae.output_path.to_string_lossy().to_string());
                p.end_time = Some(Utc::now().naive_utc());
                if let Err(e) = &persisted {
                    push_log(p, &format!("⚠ Could not save this run to history: {}", e));
                }
                push_log(p, "Done.");
            });
        }
        Err(e) => {
            error!("[edit {}] failed: {}", job_id, e);
            let persisted = persist_edit(
                &pool,
                production_id,
                "failed",
                &opts,
                None,
                None,
                None,
                Some(&e),
                &logs,
                &ai,
                None,
                options_json.as_deref(),
            );
            if let Err(pe) = &persisted {
                warn!("[edit {}] also failed to persist the failure: {}", job_id, pe);
            }
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

/// Copy the current activity-log lines out of the progress map.
fn snapshot_logs(edit_map: &EditJobMap, job_id: &str) -> Vec<String> {
    edit_map
        .lock()
        .ok()
        .and_then(|m| m.get(job_id).map(|p| p.logs.clone()))
        .unwrap_or_default()
}

/// Re-render an existing run with timeline edits applied, WITHOUT re-transcribing
/// or re-planning: it reuses the run's saved cut (the EDL clips), word
/// transcripts, and options, and writes a fresh version (v<N+1>). `mute` is a
/// list of (start, end) seconds on the final timeline where the music should be
/// ducked (the bursts the user removed); `fade` is a list of regions where the
/// music should ramp in/out. `clip_edits` re-trim or drop individual clips (a
/// long take shortened, or a clip removed). `enhance_clips` is a list of clip
/// `order`s (1-based, as in the EDL) the user marked for voice enhancement;
/// enhancement already on the saved cut (per-clip or take-level) is preserved so
/// it stays sticky across re-renders. Returns a job id to poll.
#[allow(clippy::too_many_arguments)]
pub fn start_rerender(
    edit_id: i32,
    mute: Vec<(f32, f32)>,
    fade: Vec<(f32, f32)>,
    clip_edits: Vec<ClipEdit>,
    enhance_clips: Vec<i32>,
    pool: DbPool,
    ai: AiSettings,
    edit_map: EditJobMap,
) -> Result<String, String> {
    let mut conn = pool.get().map_err(|e| format!("Database connection failed: {}", e))?;

    let edit = get_edit_by_id(&mut conn, edit_id).ok_or_else(|| format!("Edit not found: {}", edit_id))?;
    let production_id = edit.production_id;
    let production = production_service::get_production(&mut conn, production_id)
        .ok_or_else(|| format!("Production not found: {}", production_id))?;

    let opts: EditOptions = edit
        .options_json
        .as_deref()
        .and_then(|s| serde_json::from_str(s).ok())
        .ok_or("This run can't be re-rendered (it predates re-render support — make a new edit).")?;

    if opts.output_dir.as_deref().map(str::trim).unwrap_or("").is_empty() {
        return Err("This run has no saved output folder to re-render into.".to_string());
    }

    let transcripts: HashMap<i32, Vec<TranscriptWord>> = edit
        .transcripts_json
        .as_deref()
        .and_then(|s| serde_json::from_str(s).ok())
        .unwrap_or_default();

    // Reconstruct the cut from the saved EDL clips (source ranges + video ids).
    let edl: serde_json::Value = edit
        .edl_json
        .as_deref()
        .and_then(|s| serde_json::from_str(s).ok())
        .ok_or("This run has no saved edit decision list to re-render.")?;

    let edits_by_order: HashMap<i32, &ClipEdit> =
        clip_edits.iter().map(|e| (e.order, e)).collect();

    let mut cut_list: Vec<ResolvedClipInternal> = Vec::new();
    // Per-clip enhancement flags aligned to `cut_list`: a clip is enhanced if it
    // was already enhanced on the saved cut (sticky), or its take was checked at
    // run time, the user marked its `order` on the timeline, or a clip edit
    // enabled it.
    let mut enhance_flags: Vec<bool> = Vec::new();
    for c in edl["clips"].as_array().cloned().unwrap_or_default() {
        let vid = match c["video_id"].as_i64() {
            Some(v) => v as i32,
            None => continue,
        };
        let video = match video_service::get_video(&mut conn, vid) {
            Some(v) => v,
            None => continue,
        };
        let order = c["order"].as_i64().unwrap_or(0) as i32;
        let edit = edits_by_order.get(&order).copied();
        let saved_start = c["start"].as_f64().unwrap_or(0.0) as f32;
        let saved_end = c["end"].as_f64().unwrap_or(0.0) as f32;
        // Apply any trim/remove edit, clamped to the take's real duration.
        let (start, end) =
            match apply_clip_edit(saved_start, saved_end, video.duration.unwrap_or(0.0), edit) {
                Some(range) => range,
                None => continue, // removed or empty after trimming
            };
        let enhance = c["enhanced"].as_bool().unwrap_or(false)
            || edit.map(|e| e.enhance).unwrap_or(false)
            || enhance_clips.contains(&order)
            || opts.enhance_voice_video_ids.contains(&vid);
        cut_list.push(ResolvedClipInternal {
            video_id: vid,
            filename: video.filename.clone(),
            file_path: video.file_path.clone(),
            start,
            end,
            reason: c["reason"].as_str().map(|s| s.to_string()),
        });
        enhance_flags.push(enhance);
    }
    if cut_list.is_empty() {
        return Err("No clips left to re-render — every clip was removed or unavailable.".to_string());
    }

    let job_id = Uuid::new_v4().to_string();
    {
        let mut map = edit_map.lock().unwrap();
        map.insert(job_id.clone(), EditJobProgress::new(job_id.clone(), production_id));
    }

    let production_title = production.title;
    let job_id_thread = job_id.clone();
    std::thread::Builder::new()
        .name(format!("vm-rerender-{}", job_id))
        .spawn(move || {
            run_rerender(
                &job_id_thread,
                production_id,
                production_title,
                opts,
                ai,
                cut_list,
                enhance_flags,
                transcripts,
                mute,
                fade,
                pool,
                edit_map,
            );
        })
        .map_err(|e| format!("Failed to spawn re-render thread: {}", e))?;

    Ok(job_id)
}

#[allow(clippy::too_many_arguments)]
fn run_rerender(
    job_id: &str,
    production_id: i32,
    production_title: String,
    opts: EditOptions,
    ai: AiSettings,
    cut_list: Vec<ResolvedClipInternal>,
    enhance_flags: Vec<bool>,
    transcripts: HashMap<i32, Vec<TranscriptWord>>,
    mute: Vec<(f32, f32)>,
    fade: Vec<(f32, f32)>,
    pool: DbPool,
    edit_map: EditJobMap,
) {
    let result = rerender_inner(job_id, production_id, &production_title, &opts, &ai, &cut_list, &enhance_flags, &transcripts, &mute, &fade, &edit_map);
    let logs = snapshot_logs(&edit_map, job_id);
    let options_json = serde_json::to_string(&opts).ok();
    let transcripts_json = serde_json::to_string(&transcripts).ok();

    match result {
        Ok(ae) => {
            let _ = persist_edit(
                &pool,
                production_id,
                "completed",
                &opts,
                Some(&ae.edl),
                Some(&ae.output_path),
                Some(&ae.edl_path),
                None,
                &logs,
                &ai,
                transcripts_json.as_deref(),
                options_json.as_deref(),
            );
            update(&edit_map, job_id, |p| {
                p.status = "completed".to_string();
                p.stage = "completed".to_string();
                p.message = "Re-rendered video created.".to_string();
                p.edl = Some(ae.edl);
                p.edl_path = Some(ae.edl_path.to_string_lossy().to_string());
                p.output_path = Some(ae.output_path.to_string_lossy().to_string());
                p.end_time = Some(Utc::now().naive_utc());
                push_log(p, "Done.");
            });
        }
        Err(e) => {
            error!("[rerender {}] failed: {}", job_id, e);
            let _ = persist_edit(
                &pool, production_id, "failed", &opts, None, None, None, Some(&e), &logs, &ai,
                transcripts_json.as_deref(), options_json.as_deref(),
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

#[allow(clippy::too_many_arguments)]
fn rerender_inner(
    job_id: &str,
    production_id: i32,
    production_title: &str,
    opts: &EditOptions,
    ai: &AiSettings,
    cut_list: &[ResolvedClipInternal],
    enhance_flags: &[bool],
    transcripts: &HashMap<i32, Vec<TranscriptWord>>,
    mute: &[(f32, f32)],
    fade: &[(f32, f32)],
    edit_map: &EditJobMap,
) -> Result<AssembledEdit, String> {
    set_stage(edit_map, job_id, "starting", "Re-rendering with your timeline edits…");
    log_msg(edit_map, job_id, &format!("ffmpeg: {}", ffmpeg_service::ffmpeg_diagnostics()));

    let root = opts.output_dir.as_deref().map(str::trim).filter(|s| !s.is_empty())
        .ok_or("This run has no saved output folder.")?;
    let prod_dir = std::path::Path::new(root).join("productions");
    let version = next_version_number(&prod_dir);
    let out_dir = prod_dir.join(format!("v{}", version));
    std::fs::create_dir_all(&out_dir)
        .map_err(|e| format!("Failed to create output folder {}: {}", out_dir.display(), e))?;

    let stem = output_stem(opts.output_name.as_deref(), production_title, job_id);
    let output_path = out_dir.join(format!("{}.mp4", stem));
    let edl_path = out_dir.join(format!("{}.json", stem));

    let music_path = opts.music_path.as_deref().map(str::trim).filter(|s| !s.is_empty()).map(PathBuf::from);
    let music_name = music_path.as_ref().and_then(|p| p.file_name()).map(|f| f.to_string_lossy().to_string());

    // Music now ducks at speech ∪ the muted regions the user removed, and ramps
    // in/out across any fade regions.
    let speech = timeline_speech_intervals(cut_list, transcripts, opts.music_min_gap);
    let duck = merge_intervals(&speech, mute);
    if !mute.is_empty() {
        log_msg(edit_map, job_id, &format!("Muting music in {} selected region(s).", mute.len()));
    }
    log_msg(edit_map, job_id, &format!("Re-rendering {} clip(s) from the saved cut.", cut_list.len()));

    let mut edl_value = build_edl_json(
        production_id,
        production_title,
        ai,
        cut_list,
        &output_path,
        opts.captions,
        music_name.as_deref(),
        enhance_flags,
        opts.enhance_voice_intensity,
    );
    edl_value["timeline"] = build_timeline(cut_list, &speech, &duck, mute, fade, opts, music_name.as_deref(), enhance_flags);
    let pretty = serde_json::to_string_pretty(&edl_value).unwrap_or_else(|_| "{}".to_string());
    std::fs::write(&edl_path, &pretty).map_err(|e| format!("Failed to write EDL JSON: {}", e))?;

    let work_dir = out_dir.join(format!(".work-{}", job_id));
    assemble_final(
        edit_map, job_id, cut_list, transcripts, opts.captions,
        enhance_flags, opts.enhance_voice_intensity,
        &[], &work_dir, &output_path,
        music_path.as_deref(), opts.music_volume, opts.music_duck_volume, &duck, fade,
    )?;

    Ok(AssembledEdit {
        edl: edl_value,
        edl_path,
        output_path,
        transcripts_json: serde_json::to_string(transcripts).unwrap_or_else(|_| "{}".to_string()),
    })
}

/// Merge two interval lists into sorted, non-overlapping intervals.
fn merge_intervals(a: &[(f32, f32)], b: &[(f32, f32)]) -> Vec<(f32, f32)> {
    let mut all: Vec<(f32, f32)> = a.iter().chain(b.iter()).copied().filter(|(s, e)| e > s).collect();
    all.sort_by(|x, y| x.0.partial_cmp(&y.0).unwrap_or(std::cmp::Ordering::Equal));
    let mut out: Vec<(f32, f32)> = Vec::new();
    for (s, e) in all {
        if let Some(last) = out.last_mut() {
            if s <= last.1 {
                if e > last.1 {
                    last.1 = e;
                }
                continue;
            }
        }
        out.push((s, e));
    }
    out
}

/// Outcome of a successful pipeline run, carried back to `run_edit` for
/// persistence (including the transcripts needed to re-render later).
struct AssembledEdit {
    edl: serde_json::Value,
    edl_path: PathBuf,
    output_path: PathBuf,
    transcripts_json: String,
}

/// The pipeline proper.
fn run_edit_inner(
    job_id: &str,
    production_id: i32,
    production_title: &str,
    opts: &EditOptions,
    takes: &[TakeMeta],
    ai: &AiSettings,
    edit_map: &EditJobMap,
) -> Result<AssembledEdit, String> {
    let script = opts.script.as_str();
    let instructions = opts.instructions.as_deref();

    // Everything for this run lands under
    // <chosen output root>/productions/v<N>/ — a fresh version folder per run so
    // re-edits never overwrite each other. The chosen root is used verbatim (no
    // per-production subfolder), so re-runs don't nest. Nothing goes to app-data.
    let root = opts
        .output_dir
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .ok_or("Choose an output folder for the final video.")?;
    let prod_dir = std::path::Path::new(root).join("productions");
    let version = next_version_number(&prod_dir);
    let out_dir = prod_dir.join(format!("v{}", version));
    std::fs::create_dir_all(&out_dir)
        .map_err(|e| format!("Failed to create output folder {}: {}", out_dir.display(), e))?;

    // A dedicated current-thread Tokio runtime for the async HTTP work
    // (transcription + LLM). ffmpeg calls below are synchronous.
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|e| format!("Failed to start async runtime: {}", e))?;

    // Record which ffmpeg actually runs (bundled vs system PATH) — they can
    // differ in audio-filter behavior, which matters for music ducking.
    log_msg(edit_map, job_id, &format!("ffmpeg: {}", ffmpeg_service::ffmpeg_diagnostics()));

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

    // Optionally tighten the cut: split each clip into sub-clips of actual speech,
    // dropping internal silences/filler longer than `tighten_gap`. The resulting
    // `cut_list` is the real cut and drives everything downstream (segments,
    // timeline, music ducking) so the final video and the preview stay in sync.
    let cut_list: Vec<ResolvedClipInternal> = if opts.tighten {
        let before = resolved.len();
        let expanded: Vec<ResolvedClipInternal> = resolved
            .iter()
            .flat_map(|c| {
                let words = transcripts.get(&c.video_id).map(|w| w.as_slice()).unwrap_or(&[]);
                clip_keep_segments(words, c.start, c.end, opts.tighten_gap)
                    .into_iter()
                    .map(|(s, e)| ResolvedClipInternal {
                        video_id: c.video_id,
                        filename: c.filename.clone(),
                        file_path: c.file_path.clone(),
                        start: s,
                        end: e,
                        reason: c.reason.clone(),
                    })
                    .collect::<Vec<_>>()
            })
            .collect();
        if expanded.is_empty() {
            resolved.clone()
        } else {
            log_msg(
                edit_map,
                job_id,
                &format!("Tightened {} clip(s) into {} (removed long pauses/filler).", before, expanded.len()),
            );
            expanded
        }
    } else {
        resolved.clone()
    };

    // The final video and its EDL JSON share a basename and live side by side in
    // the production's output folder.
    let stem = output_stem(opts.output_name.as_deref(), production_title, job_id);
    let output_path = out_dir.join(format!("{}.mp4", stem));
    let edl_path = out_dir.join(format!("{}.json", stem));

    let music_path = opts
        .music_path
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(PathBuf::from);
    let music_name = music_path
        .as_ref()
        .and_then(|p| p.file_name())
        .map(|f| f.to_string_lossy().to_string());

    // Speech intervals on the final timeline — drive both the timeline's voice
    // track and the (deterministic) music ducking. Pauses shorter than
    // `music_min_gap` stay part of speech so the music doesn't swell mid-sentence.
    let speech_timeline = timeline_speech_intervals(&cut_list, &transcripts, opts.music_min_gap);

    // Per-clip "enhance voice" flags: a clip is enhanced when its take was
    // checked in the form (tightened sub-clips inherit their take id, so this
    // covers them too).
    let enhance_flags: Vec<bool> = cut_list
        .iter()
        .map(|c| opts.enhance_voice_video_ids.contains(&c.video_id))
        .collect();

    // Build the EDL JSON deliverable, plus a timeline (clips laid end-to-end +
    // speech intervals + music levels) for the editor-style preview.
    let mut edl_value = build_edl_json(
        production_id,
        production_title,
        ai,
        &cut_list,
        &output_path,
        opts.captions,
        music_name.as_deref(),
        &enhance_flags,
        opts.enhance_voice_intensity,
    );
    edl_value["timeline"] = build_timeline(&cut_list, &speech_timeline, &speech_timeline, &[], &[], opts, music_name.as_deref(), &enhance_flags);
    let pretty = serde_json::to_string_pretty(&edl_value).unwrap_or_else(|_| "{}".to_string());
    std::fs::write(&edl_path, &pretty).map_err(|e| format!("Failed to write EDL JSON: {}", e))?;
    log_msg(edit_map, job_id, &format!("Wrote edit decision list to {}", edl_path.display()));

    // --- Stage 3: stitch the clips + music with ffmpeg ---------------------
    let work_dir = out_dir.join(format!(".work-{}", job_id));
    assemble_final(
        edit_map,
        job_id,
        &cut_list,
        &transcripts,
        opts.captions,
        &enhance_flags,
        opts.enhance_voice_intensity,
        takes,
        &work_dir,
        &output_path,
        music_path.as_deref(),
        opts.music_volume,
        opts.music_duck_volume,
        &speech_timeline,
        &[],
    )?;

    Ok(AssembledEdit {
        edl: edl_value,
        edl_path,
        output_path,
        transcripts_json: serde_json::to_string(&transcripts).unwrap_or_else(|_| "{}".to_string()),
    })
}

/// Cut every (sub-)clip to a normalized segment (burning in captions when
/// enabled), concatenate them, and mix in the music ducked at `duck_intervals`.
/// Shared by the full pipeline and the re-render path. `takes_for_spec` may be
/// empty — then the output spec is probed from the first clip's file.
#[allow(clippy::too_many_arguments)]
fn assemble_final(
    edit_map: &EditJobMap,
    job_id: &str,
    cut_list: &[ResolvedClipInternal],
    transcripts: &HashMap<i32, Vec<TranscriptWord>>,
    captions: bool,
    // Per-clip "enhance voice" flags, aligned to `cut_list`. A clip whose flag
    // is true gets the noise-removal audio filter.
    enhance_flags: &[bool],
    enhance_intensity: f32,
    takes_for_spec: &[TakeMeta],
    work_dir: &std::path::Path,
    output_path: &std::path::Path,
    music_path: Option<&std::path::Path>,
    music_volume: f32,
    music_duck_volume: f32,
    duck_intervals: &[(f32, f32)],
    fade_intervals: &[(f32, f32)],
) -> Result<(), String> {
    let (target_w, target_h, target_fps) = resolve_target_spec(cut_list, takes_for_spec);
    set_stage(
        edit_map,
        job_id,
        "stitching",
        &format!(
            "Cutting {} clip(s) at {}x{} {:.0}fps{}…",
            cut_list.len(),
            target_w,
            target_h,
            target_fps,
            if captions { " with captions" } else { "" }
        ),
    );
    update(edit_map, job_id, |p| {
        p.total = cut_list.len() as i64;
        p.processed = 0;
    });

    std::fs::create_dir_all(work_dir).map_err(|e| format!("Failed to create work dir: {}", e))?;

    let enhance_count = enhance_flags.iter().filter(|f| **f).count();
    if enhance_count > 0 {
        log_msg(
            edit_map,
            job_id,
            &format!(
                "Enhancing voice (removing background noise) on {} clip(s) at {}% intensity.",
                enhance_count,
                (enhance_intensity.clamp(0.0, 1.0) * 100.0).round()
            ),
        );
    }

    let mut segments: Vec<PathBuf> = Vec::with_capacity(cut_list.len());
    for (i, clip) in cut_list.iter().enumerate() {
        let enhance = enhance_flags.get(i).copied().unwrap_or(false);
        log_msg(
            edit_map,
            job_id,
            &format!(
                "Cutting clip {}/{}: {} [{:.2}s–{:.2}s]{}",
                i + 1,
                cut_list.len(),
                clip.filename,
                clip.start,
                clip.end,
                if enhance { " · voice enhanced" } else { "" }
            ),
        );

        let sub_name = if captions {
            let words = transcripts.get(&clip.video_id).map(|w| w.as_slice()).unwrap_or(&[]);
            match build_clip_srt(words, clip.start, clip.end) {
                Some(srt) => {
                    let name = format!("seg_{:04}.srt", i);
                    std::fs::write(work_dir.join(&name), srt)
                        .map_err(|e| format!("Failed to write captions file: {}", e))?;
                    Some(name)
                }
                None => None,
            }
        } else {
            None
        };

        let audio_filter = if enhance {
            Some(ffmpeg_service::voice_enhance_filter(enhance_intensity))
        } else {
            None
        };

        let seg = work_dir.join(format!("seg_{:04}.mp4", i));
        ffmpeg_service::extract_clip_segment(
            std::path::Path::new(&clip.file_path),
            clip.start,
            clip.end,
            target_w,
            target_h,
            target_fps,
            &seg,
            sub_name.as_deref(),
            audio_filter.as_deref(),
        )?;
        segments.push(seg);
        update(edit_map, job_id, |p| p.processed = (i + 1) as i64);
    }

    if let Some(music) = music_path.filter(|p| p.exists()) {
        let concat_tmp = work_dir.join("_concat.mp4");
        set_stage(edit_map, job_id, "stitching", "Joining clips…");
        ffmpeg_service::concat_clips(&segments, &concat_tmp)?;

        set_stage(edit_map, job_id, "mixing", "Adding background music…");
        log_msg(
            edit_map,
            job_id,
            &format!(
                "Music ducking: {}% in pauses → {}% while talking",
                (music_volume * 100.0).round(),
                (music_duck_volume * 100.0).round()
            ),
        );
        if !fade_intervals.is_empty() {
            log_msg(
                edit_map,
                job_id,
                &format!("Fading the music in/out across {} region(s).", fade_intervals.len()),
            );
        }
        match ffmpeg_service::add_background_music(&concat_tmp, music, music_volume, music_duck_volume, duck_intervals, fade_intervals, output_path) {
            Ok(()) => log_msg(edit_map, job_id, "Mixed in background music."),
            Err(e) => {
                warn!("[edit {}] background music failed: {}", job_id, e);
                log_msg(edit_map, job_id, &format!("Background music failed ({}). Keeping the cut without music.", e));
                std::fs::copy(&concat_tmp, output_path)
                    .map_err(|e| format!("Failed to write final video: {}", e))?;
            }
        }
    } else {
        set_stage(edit_map, job_id, "stitching", "Joining clips into the final video…");
        ffmpeg_service::concat_clips(&segments, output_path)?;
    }

    let _ = std::fs::remove_dir_all(work_dir);

    if !output_path.exists() {
        return Err("ffmpeg reported success but the final video is missing.".to_string());
    }
    log_msg(edit_map, job_id, &format!("Final video: {}", output_path.display()));
    Ok(())
}

/// Strip path separators and awkward characters from a single path segment.
fn sanitize_segment(s: &str) -> String {
    let cleaned: String = s
        .chars()
        .map(|c| if c.is_alphanumeric() || matches!(c, ' ' | '-' | '_' | '.') { c } else { '_' })
        .collect();
    cleaned.trim().trim_matches('.').trim().to_string()
}

/// Next `v<N>` version number to use inside a production folder. Scans existing
/// `v<number>` subdirectories and returns max+1 (so deletions leave gaps rather
/// than reusing numbers). Starts at 1.
fn next_version_number(prod_dir: &std::path::Path) -> u32 {
    let mut max = 0u32;
    if let Ok(entries) = std::fs::read_dir(prod_dir) {
        for entry in entries.flatten() {
            if !entry.path().is_dir() {
                continue;
            }
            if let Some(name) = entry.file_name().to_str() {
                if let Some(num) = name.strip_prefix('v').and_then(|n| n.parse::<u32>().ok()) {
                    max = max.max(num);
                }
            }
        }
    }
    max + 1
}

/// Basename (no extension) shared by the final video and its EDL JSON. Uses the
/// requested filename, else the production title, else a per-job fallback.
fn output_stem(requested: Option<&str>, production_title: &str, job_id: &str) -> String {
    let raw = requested
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .unwrap_or(production_title);
    let mut name = sanitize_segment(raw);
    // Drop a user-typed .mp4 extension so we control it.
    if name.to_lowercase().ends_with(".mp4") {
        name.truncate(name.len() - 4);
        name = name.trim().trim_matches('.').trim().to_string();
    }
    if name.is_empty() {
        name = format!("final-{}", job_id);
    }
    name
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
#[allow(clippy::too_many_arguments)]
fn build_edl_json(
    production_id: i32,
    production_title: &str,
    ai: &AiSettings,
    resolved: &[ResolvedClipInternal],
    output_path: &std::path::Path,
    captions: bool,
    music: Option<&str>,
    enhance_flags: &[bool],
    enhance_intensity: f32,
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
                "enhanced": enhance_flags.get(i).copied().unwrap_or(false),
            })
        })
        .collect();

    let any_enhanced = enhance_flags.iter().any(|f| *f);

    serde_json::json!({
        "production_id": production_id,
        "production_title": production_title,
        "generated_at": Utc::now().naive_utc().format("%Y-%m-%dT%H:%M:%S").to_string(),
        "transcription_provider": ai.transcription_provider,
        "text_provider": ai.text_provider,
        "text_model": ai.text_model,
        "captions": captions,
        "music": music,
        "voice_enhance": {
            "enabled": any_enhanced,
            "intensity": round2(enhance_intensity.clamp(0.0, 1.0)),
        },
        "clips": clips,
        "output": output_path.file_name().map(|f| f.to_string_lossy().to_string()),
    })
}

/// Build the editor-style timeline for the EDL: every clip laid end-to-end on a
/// single timeline, the speech intervals (re-timed onto that timeline, used to
/// visualize where the music ducks), the total duration, and the music levels.
#[allow(clippy::too_many_arguments)]
fn build_timeline(
    resolved: &[ResolvedClipInternal],
    speech: &[(f32, f32)],
    duck: &[(f32, f32)],
    muted: &[(f32, f32)],
    fade: &[(f32, f32)],
    opts: &EditOptions,
    music_name: Option<&str>,
    enhance_flags: &[bool],
) -> serde_json::Value {
    let mut clips = Vec::new();
    let mut cursor = 0.0f32;
    for (i, c) in resolved.iter().enumerate() {
        let dur = (c.end - c.start).max(0.0);
        let tstart = cursor;
        let tend = cursor + dur;
        clips.push(serde_json::json!({
            "order": i + 1,
            "video_id": c.video_id,
            "filename": c.filename,
            "start": round2(tstart),
            "end": round2(tend),
            "source_start": round2(c.start),
            "source_end": round2(c.end),
            "enhanced": enhance_flags.get(i).copied().unwrap_or(false),
        }));
        cursor = tend;
    }

    let to_json = |xs: &[(f32, f32)]| -> Vec<serde_json::Value> {
        xs.iter()
            .map(|(s, e)| serde_json::json!({ "start": round2(*s), "end": round2(*e) }))
            .collect()
    };

    let music_present = opts
        .music_path
        .as_deref()
        .map(str::trim)
        .map(|s| !s.is_empty())
        .unwrap_or(false);

    serde_json::json!({
        "duration": round2(cursor),
        "clips": clips,
        "speech": to_json(speech),
        "duck": to_json(duck),
        "muted": to_json(muted),
        "fades": to_json(fade),
        "music": {
            "present": music_present,
            "name": music_name,
            "full_volume": opts.music_volume,
            "duck_volume": opts.music_duck_volume.min(opts.music_volume),
        },
    })
}

/// Speech intervals (start, end seconds) on the FINAL timeline: each clip's
/// words mapped onto its position in the assembled video. Used both to draw the
/// timeline's voice track and to drive the music ducking. Words within ~0.8s
/// join into one interval so the music doesn't pop up during tiny gaps.
fn timeline_speech_intervals(
    resolved: &[ResolvedClipInternal],
    transcripts: &HashMap<i32, Vec<TranscriptWord>>,
    min_gap: f32,
) -> Vec<(f32, f32)> {
    // Clamp to a sane range; words within `gap` seconds of each other count as
    // one continuous speech run (so short pauses don't bring the music back).
    let gap = min_gap.clamp(0.2, 10.0);
    let mut out = Vec::new();
    let mut cursor = 0.0f32;
    for c in resolved {
        let dur = (c.end - c.start).max(0.0);
        let tstart = cursor;
        if let Some(words) = transcripts.get(&c.video_id) {
            for (s, e) in speech_intervals(words, c.start, c.end, gap) {
                out.push(((s - c.start) + tstart, (e - c.start) + tstart));
            }
        }
        cursor += dur;
    }
    out
}

/// Merge a take's words into contiguous speech intervals (words closer than
/// `gap` seconds join), clamped to `[clip_start, clip_end]`. Source timeline.
fn speech_intervals(words: &[TranscriptWord], clip_start: f32, clip_end: f32, gap: f32) -> Vec<(f32, f32)> {
    let mut out: Vec<(f32, f32)> = Vec::new();
    for w in words.iter().filter(|w| w.end > clip_start && w.start < clip_end) {
        let s = w.start.clamp(clip_start, clip_end);
        let e = w.end.clamp(clip_start, clip_end);
        if e <= s {
            continue;
        }
        if let Some(last) = out.last_mut() {
            if s - last.1 <= gap {
                last.1 = last.1.max(e);
                continue;
            }
        }
        out.push((s, e));
    }
    out
}

/// Non-lexical filler sounds to drop when tightening (compared after lowercasing
/// and stripping punctuation). Conservative — only clear disfluencies, never real
/// words like "like"/"so" whose removal would change meaning.
fn is_filler(text: &str) -> bool {
    let t: String = text.trim().to_lowercase().chars().filter(|c| c.is_alphanumeric()).collect();
    matches!(
        t.as_str(),
        "um" | "umm" | "uh" | "uhh" | "uhm" | "erm" | "er" | "hmm" | "hm" | "mmm" | "mm" | "mhm" | "uhhuh" | "huh"
    )
}

/// Split a clip into the speech-only sub-ranges to keep, dropping internal
/// silences and filler runs longer than `gap` seconds. Real speech (non-filler
/// words) within `gap` of each other stays as one run; longer gaps become cut
/// points. Each kept run is padded slightly so word onsets/tails aren't clipped.
/// Returns the whole clip unchanged when there are no word timestamps.
fn clip_keep_segments(words: &[TranscriptWord], clip_start: f32, clip_end: f32, gap: f32) -> Vec<(f32, f32)> {
    let gap = gap.clamp(0.3, 10.0);
    const LEAD: f32 = 0.10;
    const TAIL: f32 = 0.20;

    let content: Vec<(f32, f32)> = words
        .iter()
        .filter(|w| w.end > clip_start && w.start < clip_end && !is_filler(&w.text))
        .map(|w| (w.start.clamp(clip_start, clip_end), w.end.clamp(clip_start, clip_end)))
        .filter(|(s, e)| e > s)
        .collect();

    if content.is_empty() {
        return vec![(clip_start, clip_end)];
    }

    // Merge content words separated by less than `gap` into one run.
    let mut runs: Vec<(f32, f32)> = Vec::new();
    for (s, e) in content {
        if let Some(last) = runs.last_mut() {
            if s - last.1 < gap {
                if e > last.1 {
                    last.1 = e;
                }
                continue;
            }
        }
        runs.push((s, e));
    }

    // Pad each run and merge any overlaps the padding introduces.
    let mut out: Vec<(f32, f32)> = Vec::new();
    for (s, e) in runs {
        let s = (s - LEAD).max(clip_start);
        let e = (e + TAIL).min(clip_end);
        if let Some(last) = out.last_mut() {
            if s <= last.1 {
                if e > last.1 {
                    last.1 = e;
                }
                continue;
            }
        }
        out.push((s, e));
    }
    out
}

/// Build an SRT for one clip: keep the words inside `[clip_start, clip_end]` and
/// group them into short, readable caption cues. Cue times stay on the SOURCE
/// take's timeline (NOT re-based to 0): the `subtitles` filter runs before the
/// output-side `-ss` trim, so it expects original timestamps; the trim then
/// shifts the burned-in captions into place. Returns `None` for a clip with no
/// words.
fn build_clip_srt(words: &[TranscriptWord], clip_start: f32, clip_end: f32) -> Option<String> {
    let local: Vec<TranscriptWord> = words
        .iter()
        .filter(|w| w.end > clip_start && w.start < clip_end)
        .map(|w| TranscriptWord {
            text: w.text.clone(),
            start: w.start.clamp(clip_start, clip_end),
            end: w.end.clamp(clip_start, clip_end),
        })
        .filter(|w| !w.text.trim().is_empty())
        .collect();

    if local.is_empty() {
        return None;
    }

    // Group into cues: break on long pauses, after ~7 words, or ~3s.
    const MAX_WORDS: usize = 7;
    const MAX_DUR: f32 = 3.0;
    const GAP: f32 = 0.7;

    let mut cues: Vec<(f32, f32, String)> = Vec::new();
    let mut start = local[0].start;
    let mut end = local[0].end;
    let mut buf: Vec<&str> = Vec::new();

    for (i, w) in local.iter().enumerate() {
        let gap_break = !buf.is_empty() && (w.start - end) > GAP;
        let len_break = buf.len() >= MAX_WORDS;
        let dur_break = !buf.is_empty() && (w.end - start) > MAX_DUR;
        if gap_break || len_break || dur_break {
            cues.push((start, end, buf.join(" ")));
            buf.clear();
        }
        if buf.is_empty() {
            start = w.start;
        }
        buf.push(w.text.trim());
        end = w.end;
        if i == local.len() - 1 && !buf.is_empty() {
            cues.push((start, end, buf.join(" ")));
        }
    }

    let mut srt = String::new();
    for (i, (s, e, text)) in cues.iter().enumerate() {
        // Guarantee a strictly increasing, non-zero-length cue.
        let end_t = if *e > *s { *e } else { *s + 0.4 };
        srt.push_str(&format!(
            "{}\n{} --> {}\n{}\n\n",
            i + 1,
            srt_time(*s),
            srt_time(end_t),
            text
        ));
    }
    Some(srt)
}

/// Format seconds as an SRT timestamp `HH:MM:SS,mmm`.
fn srt_time(secs: f32) -> String {
    let secs = secs.max(0.0);
    let total_ms = (secs * 1000.0).round() as u64;
    let ms = total_ms % 1000;
    let total_s = total_ms / 1000;
    let s = total_s % 60;
    let m = (total_s / 60) % 60;
    let h = total_s / 3600;
    format!("{:02}:{:02}:{:02},{:03}", h, m, s, ms)
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

/// Persist a completed or failed edit attempt to the database. Returns an error
/// (surfaced into the run's activity log) if the row cannot be written.
#[allow(clippy::too_many_arguments)]
fn persist_edit(
    pool: &DbPool,
    production_id: i32,
    status: &str,
    opts: &EditOptions,
    edl_value: Option<&serde_json::Value>,
    output_path: Option<&PathBuf>,
    edl_path: Option<&PathBuf>,
    error: Option<&str>,
    logs: &[String],
    ai: &AiSettings,
    transcripts_json: Option<&str>,
    options_json: Option<&str>,
) -> Result<(), String> {
    let mut conn = pool
        .get()
        .map_err(|e| format!("database connection failed: {}", e))?;

    let record = NewProductionEdit {
        production_id,
        status: status.to_string(),
        script: Some(opts.script.clone()),
        instructions: opts.instructions.clone(),
        edl_json: edl_value.map(|v| v.to_string()),
        output_path: output_path.map(|p| p.to_string_lossy().to_string()),
        edl_path: edl_path.map(|p| p.to_string_lossy().to_string()),
        error: error.map(|s| s.to_string()),
        transcription_provider: Some(ai.transcription_provider.clone()),
        text_provider: Some(ai.text_provider.clone()),
        text_model: Some(ai.text_model.clone()),
        created_at: Utc::now().naive_utc(),
        logs: serde_json::to_string(logs).ok(),
        transcripts_json: transcripts_json.map(|s| s.to_string()),
        options_json: options_json.map(|s| s.to_string()),
    };

    diesel::insert_into(production_edits::table)
        .values(&record)
        .execute(&mut conn)
        .map_err(|e| {
            warn!("Failed to insert production_edit row: {}", e);
            e.to_string()
        })?;
    Ok(())
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

/// A single persisted edit by its row id.
pub fn get_edit_by_id(
    conn: &mut diesel::sqlite::SqliteConnection,
    edit_id: i32,
) -> Option<crate::models::ProductionEdit> {
    production_edits::table.find(edit_id).first(conn).ok()
}

/// Reconstruct the spoken transcript of a run's FINAL cut: the words inside each
/// clip's source range, in clip order. Used to generate YouTube copy. Empty when
/// the run has no saved transcripts (e.g. an older run, or Gemini transcription).
pub fn final_transcript_for_edit(edit: &crate::models::ProductionEdit) -> String {
    let transcripts: HashMap<i32, Vec<TranscriptWord>> = edit
        .transcripts_json
        .as_deref()
        .and_then(|s| serde_json::from_str(s).ok())
        .unwrap_or_default();
    let edl: serde_json::Value = edit
        .edl_json
        .as_deref()
        .and_then(|s| serde_json::from_str(s).ok())
        .unwrap_or(serde_json::Value::Null);

    let mut out = String::new();
    for c in edl["clips"].as_array().cloned().unwrap_or_default() {
        let vid = match c["video_id"].as_i64() {
            Some(v) => v as i32,
            None => continue,
        };
        let start = c["start"].as_f64().unwrap_or(0.0) as f32;
        let end = c["end"].as_f64().unwrap_or(f64::MAX) as f32;
        if let Some(words) = transcripts.get(&vid) {
            for w in words.iter().filter(|w| w.end > start && w.start < end) {
                let t = w.text.trim();
                if t.is_empty() {
                    continue;
                }
                if !out.is_empty() {
                    out.push(' ');
                }
                out.push_str(t);
            }
        }
    }

    // Keep the prompt a sane size for very long videos.
    if out.chars().count() > 30000 {
        out = out.chars().take(30000).collect::<String>();
        out.push('…');
    }
    out
}

/// Save generated copy onto an edit row.
pub fn save_copy(
    conn: &mut diesel::sqlite::SqliteConnection,
    edit_id: i32,
    copy: &serde_json::Value,
) -> Result<(), String> {
    diesel::update(production_edits::table.find(edit_id))
        .set(production_edits::copy_json.eq(Some(copy.to_string())))
        .execute(conn)
        .map(|_| ())
        .map_err(|e| e.to_string())
}

/// Save the thumbnail builder state (text/layout/style/frame time) onto an edit
/// row so the thumbnail can be rebuilt and re-edited later.
pub fn save_thumbnail_spec(
    conn: &mut diesel::sqlite::SqliteConnection,
    edit_id: i32,
    spec: &serde_json::Value,
) -> Result<(), String> {
    diesel::update(production_edits::table.find(edit_id))
        .set(production_edits::thumbnail_json.eq(Some(spec.to_string())))
        .execute(conn)
        .map(|_| ())
        .map_err(|e| e.to_string())
}

/// The sidecar thumbnail files derived from a run's final video path:
/// `(composited PNG, background PNG)`. The background is the (possibly
/// AI-restyled) still the text was laid over — kept so the thumbnail rebuilds
/// exactly on reopen. Naming is shared by the save route and delete cleanup.
pub fn thumbnail_file_paths(output_path: &str) -> (std::path::PathBuf, std::path::PathBuf) {
    let out = std::path::Path::new(output_path);
    let parent = out.parent().unwrap_or_else(|| std::path::Path::new("."));
    let stem = out
        .file_stem()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_else(|| "thumbnail".to_string());
    (
        parent.join(format!("{}-thumbnail.png", stem)),
        parent.join(format!("{}-thumbnail-bg.png", stem)),
    )
}

/// Delete an edit: remove its files from disk (final video, EDL JSON, and the
/// now-empty version folder) and delete the database row. Returns `false` if no
/// such edit exists.
pub fn delete_edit(
    conn: &mut diesel::sqlite::SqliteConnection,
    edit_id: i32,
) -> Result<bool, String> {
    let edit = match get_edit_by_id(conn, edit_id) {
        Some(e) => e,
        None => return Ok(false),
    };

    // Remove the output video and EDL JSON if they still exist.
    for p in [edit.output_path.as_deref(), edit.edl_path.as_deref()]
        .into_iter()
        .flatten()
    {
        let path = std::path::Path::new(p);
        if path.is_file() {
            if let Err(e) = std::fs::remove_file(path) {
                warn!("Failed to delete {}: {}", p, e);
            }
        }
    }

    // Remove the thumbnail sidecar files (composited PNG + saved background) so
    // the version folder can still be reclaimed as "empty" below.
    if let Some(out) = edit.output_path.as_deref() {
        let (thumb, thumb_bg) = thumbnail_file_paths(out);
        for path in [thumb, thumb_bg] {
            if path.is_file() {
                if let Err(e) = std::fs::remove_file(&path) {
                    warn!("Failed to delete {}: {}", path.display(), e);
                }
            }
        }
    }

    // Remove the run's version folder if it's now empty.
    if let Some(out) = edit.output_path.as_deref() {
        if let Some(parent) = std::path::Path::new(out).parent() {
            let empty = std::fs::read_dir(parent)
                .map(|mut rd| rd.next().is_none())
                .unwrap_or(false);
            if empty {
                let _ = std::fs::remove_dir(parent);
            }
        }
    }

    diesel::delete(production_edits::table.find(edit_id))
        .execute(conn)
        .map_err(|e| e.to_string())?;
    Ok(true)
}

/// All persisted edit attempts for a production, newest first.
pub fn get_all_edits(
    conn: &mut diesel::sqlite::SqliteConnection,
    production_id: i32,
) -> Vec<crate::models::ProductionEdit> {
    match production_edits::table
        .filter(production_edits::production_id.eq(production_id))
        .order(production_edits::id.desc())
        .load(conn)
    {
        Ok(rows) => rows,
        Err(e) => {
            error!("Failed to load edit history for production {}: {}", production_id, e);
            Vec::new()
        }
    }
}

// --- AI timeline assistant ---------------------------------------------------

/// The spoken words inside a clip's source range, joined into one string.
fn clip_text(words: &[TranscriptWord], start: f32, end: f32) -> String {
    let mut out = String::new();
    for w in words.iter().filter(|w| w.end > start && w.start < end) {
        let t = w.text.trim();
        if t.is_empty() {
            continue;
        }
        if !out.is_empty() {
            out.push(' ');
        }
        out.push_str(t);
    }
    out
}

/// Complement of `speech` within `[0, total]`: the regions with no speech (where
/// background music plays at full level). Used to show the model the music
/// "bursts" it can fade/remove.
fn non_speech_regions(speech: &[(f32, f32)], total: f32) -> Vec<(f32, f32)> {
    let merged = merge_intervals(speech, &[]);
    let mut out = Vec::new();
    let mut cursor = 0.0f32;
    for (s, e) in merged {
        if s > cursor {
            out.push((cursor, s));
        }
        cursor = cursor.max(e);
    }
    if cursor < total {
        out.push((cursor, total));
    }
    out.into_iter().filter(|(s, e)| e - s > 0.05).collect()
}

/// Parse + validate the model's timeline-edit JSON against the real cut: keep
/// only clip edits whose `order` exists, clamp any source range to the clip's
/// listed source range, and keep music regions with a known action clamped to
/// `[0, total]`. Pure (no I/O) for testability.
fn parse_timeline_edit_plan(
    raw: &str,
    clips: &[serde_json::Value],
    total: f32,
) -> serde_json::Value {
    let value: serde_json::Value =
        serde_json::from_str(extract_json(raw)).unwrap_or(serde_json::Value::Null);

    // order -> (source_start, source_end) of the saved clip, for clamping.
    let mut ranges: HashMap<i64, (f32, f32)> = HashMap::new();
    for c in clips {
        if let Some(order) = c["order"].as_i64() {
            let s = c["start"].as_f64().unwrap_or(0.0) as f32;
            let e = c["end"].as_f64().unwrap_or(0.0) as f32;
            ranges.insert(order, (s, e));
        }
    }

    let mut out_clips = Vec::new();
    for c in value["clips"].as_array().cloned().unwrap_or_default() {
        let order = match c["order"].as_i64() {
            Some(o) => o,
            None => continue,
        };
        let (cs, ce) = match ranges.get(&order) {
            Some(r) => *r,
            None => continue, // unknown clip — drop
        };
        let remove = c["remove"].as_bool().unwrap_or(false);
        let enhance = c["enhance"].as_bool().unwrap_or(false);

        let mut obj = serde_json::Map::new();
        obj.insert("order".into(), serde_json::json!(order));
        if remove {
            obj.insert("remove".into(), serde_json::json!(true));
        }
        if enhance {
            obj.insert("enhance".into(), serde_json::json!(true));
        }
        if !remove {
            // Clamp any provided source range to the clip's existing range.
            let prov_s = read_f32(&c["source_start"]);
            let prov_e = read_f32(&c["source_end"]);
            let eff_s = prov_s.map(|v| v.clamp(cs, ce)).unwrap_or(cs);
            let eff_e = prov_e.map(|v| v.clamp(cs, ce)).unwrap_or(ce);
            if eff_e > eff_s {
                if prov_s.is_some() {
                    obj.insert("source_start".into(), serde_json::json!(round2(eff_s)));
                }
                if prov_e.is_some() {
                    obj.insert("source_end".into(), serde_json::json!(round2(eff_e)));
                }
            }
        }

        // Skip no-op edits (just an order with nothing changed).
        if obj.len() > 1 {
            out_clips.push(serde_json::Value::Object(obj));
        }
    }

    let cap = if total > 0.0 { total } else { f32::MAX };
    let mut out_music = Vec::new();
    for m in value["music"].as_array().cloned().unwrap_or_default() {
        let action = m["action"].as_str().unwrap_or("");
        if action != "remove" && action != "fade" {
            continue;
        }
        let s = read_f32(&m["start"]).unwrap_or(0.0).max(0.0);
        let e = read_f32(&m["end"]).unwrap_or(0.0).min(cap);
        if e <= s {
            continue;
        }
        out_music.push(serde_json::json!({
            "start": round2(s),
            "end": round2(e),
            "action": action,
        }));
    }

    serde_json::json!({
        "clips": out_clips,
        "music": out_music,
        "explanation": value["explanation"].as_str().unwrap_or("").to_string(),
    })
}

/// Build a structured "timeline edit plan" from a natural-language instruction.
/// The LLM is shown the saved cut (each clip's take, source range, duration and
/// spoken text) plus the music regions, and returns the clips to trim / remove /
/// enhance and the music regions to remove / fade. The plan is validated against
/// the real cut before returning (unknown clips dropped, ranges clamped). This
/// only PROPOSES edits — the user reviews them and then re-renders; it does not
/// re-transcribe or re-plan the whole edit.
pub async fn plan_timeline_edits(
    edit: &crate::models::ProductionEdit,
    ai: &AiSettings,
    instruction: &str,
    selected_orders: &[i32],
) -> Result<serde_json::Value, String> {
    if instruction.trim().is_empty() {
        return Err("Type what you'd like the AI to change.".to_string());
    }

    let edl: serde_json::Value = edit
        .edl_json
        .as_deref()
        .and_then(|s| serde_json::from_str(s).ok())
        .ok_or("This run has no saved edit decision list to edit.")?;
    let transcripts: HashMap<i32, Vec<TranscriptWord>> = edit
        .transcripts_json
        .as_deref()
        .and_then(|s| serde_json::from_str(s).ok())
        .unwrap_or_default();

    let clips = edl["clips"].as_array().cloned().unwrap_or_default();
    if clips.is_empty() {
        return Err("This run has no clips to edit.".to_string());
    }

    // Describe each clip with its spoken text so the model can make sensible cuts.
    let mut clip_block = String::new();
    for c in &clips {
        let order = c["order"].as_i64().unwrap_or(0);
        let vid = c["video_id"].as_i64().unwrap_or(0) as i32;
        let filename = c["filename"].as_str().unwrap_or("");
        let start = c["start"].as_f64().unwrap_or(0.0) as f32;
        let end = c["end"].as_f64().unwrap_or(0.0) as f32;
        let enhanced = c["enhanced"].as_bool().unwrap_or(false);
        let words = transcripts.get(&vid).map(|w| w.as_slice()).unwrap_or(&[]);
        let text = clip_text(words, start, end);
        let sel = if selected_orders.contains(&(order as i32)) { " [SELECTED]" } else { "" };
        clip_block.push_str(&format!(
            "CLIP order={}{} | take=\"{}\" video_id={} | source {:.2}s–{:.2}s ({:.2}s) | enhanced={}\n  text: {}\n",
            order, sel, filename, vid, start, end, (end - start).max(0.0), enhanced,
            if text.is_empty() { "(no transcript)".to_string() } else { truncate(&text, 600) }
        ));
    }

    // Music regions + levels from the saved timeline.
    let timeline = &edl["timeline"];
    let total = timeline["duration"].as_f64().unwrap_or(0.0) as f32;
    let music_present = timeline["music"]["present"].as_bool().unwrap_or(false);
    let mut music_block = String::new();
    if music_present {
        let full = timeline["music"]["full_volume"].as_f64().unwrap_or(0.0);
        let duck = timeline["music"]["duck_volume"].as_f64().unwrap_or(0.0);
        music_block.push_str(&format!(
            "MUSIC: present. Full level {}%, ducked level {}% while talking. Total timeline {:.2}s.\n",
            (full * 100.0).round(),
            (duck * 100.0).round(),
            total
        ));
        let speech: Vec<(f32, f32)> = timeline["speech"]
            .as_array()
            .map(|a| {
                a.iter()
                    .filter_map(|v| Some((v["start"].as_f64()? as f32, v["end"].as_f64()? as f32)))
                    .collect()
            })
            .unwrap_or_default();
        music_block.push_str("Music plays at full level in these non-speech regions (good fade/remove targets):\n");
        for (s, e) in non_speech_regions(&speech, total) {
            music_block.push_str(&format!("- {:.2}s–{:.2}s\n", s, e));
        }
    } else {
        music_block.push_str("MUSIC: none on this run (music edits will have no effect).\n");
    }

    let selected_note = if selected_orders.is_empty() {
        String::new()
    } else {
        format!(
            "\nThe creator has SELECTED these clip orders to focus on: {:?}. Prefer editing those unless the instruction clearly refers to others.\n",
            selected_orders
        )
    };

    let prompt = format!(
        "You are a precise video editor adjusting an ALREADY-ASSEMBLED cut. Apply ONLY the creator's instruction below; do not redo the whole edit.\n\n\
Return STRICT JSON (no markdown, no commentary) with this shape:\n\
{{\n  \"clips\": [ {{ \"order\": <int>, \"remove\": <bool, optional>, \"source_start\": <sec, optional>, \"source_end\": <sec, optional>, \"enhance\": <bool, optional> }} ],\n  \"music\": [ {{ \"start\": <sec>, \"end\": <sec>, \"action\": \"remove\"|\"fade\" }} ],\n  \"explanation\": \"<one or two sentences>\"\n}}\n\n\
Rules:\n\
- Only include clips/music regions you are CHANGING. Leave everything else out.\n\
- source_start/source_end are ABSOLUTE seconds INTO THE TAKE and must stay within the clip's current source range. To shorten a long-running take, narrow the range (raise source_start and/or lower source_end). To DROP a clip set remove:true.\n\
- Keep source_start < source_end.\n\
- music \"remove\" silences/ducks the music in that region; \"fade\" ramps it in and out. Use the listed non-speech regions as a guide.\n\
- enhance:true cleans up that clip's voice (background-noise removal).\n\
{selected}\n\
CURRENT CUT ({nclips} clips):\n{clips}\n{music}\n\
CREATOR INSTRUCTION:\n\"\"\"\n{instruction}\n\"\"\"",
        selected = selected_note,
        nclips = clips.len(),
        clips = clip_block,
        music = music_block,
        instruction = instruction.trim(),
    );

    let raw = ai_service::complete(&prompt, ai, 2048)
        .await
        .map_err(|e| format!("AI timeline edit failed: {}", e))?;

    Ok(parse_timeline_edit_plan(&raw, &clips, total))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn apply_clip_edit_keeps_saved_range_without_edit() {
        assert_eq!(apply_clip_edit(1.0, 5.0, 10.0, None), Some((1.0, 5.0)));
    }

    #[test]
    fn apply_clip_edit_remove_returns_none() {
        let e = ClipEdit { order: 1, remove: true, ..Default::default() };
        assert_eq!(apply_clip_edit(1.0, 5.0, 10.0, Some(&e)), None);
    }

    #[test]
    fn apply_clip_edit_trims_and_clamps_to_duration() {
        let e = ClipEdit {
            order: 1,
            source_start: Some(2.0),
            source_end: Some(50.0),
            ..Default::default()
        };
        // source_end clamped down to the take's real 8s duration.
        assert_eq!(apply_clip_edit(1.0, 5.0, 8.0, Some(&e)), Some((2.0, 8.0)));
    }

    #[test]
    fn apply_clip_edit_rejects_inverted_range() {
        let e = ClipEdit {
            order: 1,
            source_start: Some(5.0),
            source_end: Some(3.0),
            ..Default::default()
        };
        assert_eq!(apply_clip_edit(1.0, 9.0, 10.0, Some(&e)), None);
    }

    #[test]
    fn non_speech_regions_are_the_gaps() {
        let r = non_speech_regions(&[(2.0, 4.0), (6.0, 7.0)], 10.0);
        assert_eq!(r, vec![(0.0, 2.0), (4.0, 6.0), (7.0, 10.0)]);
    }

    #[test]
    fn parse_plan_drops_unknown_orders_and_clamps_ranges() {
        let clips = vec![serde_json::json!({
            "order": 1, "video_id": 5, "start": 0.0, "end": 10.0
        })];
        let raw = r#"{
            "clips": [
                { "order": 1, "source_start": -2, "source_end": 8 },
                { "order": 9, "remove": true }
            ],
            "music": [
                { "start": 0, "end": 3, "action": "fade" },
                { "start": 1, "end": 2, "action": "bogus" }
            ],
            "explanation": "ok"
        }"#;
        let plan = parse_timeline_edit_plan(raw, &clips, 12.0);
        let pc = plan["clips"].as_array().unwrap();
        assert_eq!(pc.len(), 1);
        assert_eq!(pc[0]["order"], 1);
        assert_eq!(pc[0]["source_start"], 0.0); // clamped up from -2
        assert_eq!(pc[0]["source_end"], 8.0);
        let pm = plan["music"].as_array().unwrap();
        assert_eq!(pm.len(), 1);
        assert_eq!(pm[0]["action"], "fade");
    }

    #[test]
    fn parse_plan_handles_remove_and_enhance() {
        let clips = vec![serde_json::json!({
            "order": 2, "video_id": 7, "start": 1.0, "end": 9.0
        })];
        let raw = r#"{ "clips": [ { "order": 2, "remove": true, "enhance": true } ], "music": [] }"#;
        let plan = parse_timeline_edit_plan(raw, &clips, 9.0);
        let pc = plan["clips"].as_array().unwrap();
        assert_eq!(pc.len(), 1);
        assert_eq!(pc[0]["remove"], true);
        assert_eq!(pc[0]["enhance"], true);
        assert!(pc[0].get("source_start").is_none());
    }
}
