use actix_web::{delete, get, post, web, HttpResponse};
use serde::Deserialize;

use crate::config::ConfigManager;
use crate::db::DbPool;
use crate::models::ProductionEditResponse;
use crate::services::edit_service::{self, EditJobMap, EditOptions};

fn default_captions() -> bool { true }
fn default_music_volume() -> f32 { 0.3 }

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
    /// Background-music volume, 0.0–1.0.
    #[serde(default = "default_music_volume")]
    pub music_volume: f32,
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
        let mut cmd = std::process::Command::new("explorer");
        if select {
            cmd.args(["/select,", path]);
        } else {
            cmd.arg(path);
        }
        cmd.spawn().map_err(|e| e.to_string())?;
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
        .service(delete_edit);
}
