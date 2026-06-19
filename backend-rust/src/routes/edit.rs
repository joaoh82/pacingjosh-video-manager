use actix_web::{get, post, web, HttpResponse};
use serde::Deserialize;

use crate::config::ConfigManager;
use crate::db::DbPool;
use crate::models::ProductionEditResponse;
use crate::services::edit_service::{self, EditJobMap};

#[derive(Deserialize)]
pub struct StartEditRequest {
    /// The script for the video (markdown / plain text).
    pub script: String,
    /// Optional extra guidance for the editor (warm-up phrase to cut, ordering
    /// notes, tone, etc.).
    #[serde(default)]
    pub instructions: Option<String>,
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
    let edits_dir = config.get_edits_directory();

    match edit_service::start_edit(
        production_id,
        body.script.clone(),
        body.instructions.clone(),
        pool.get_ref().clone(),
        ai,
        edits_dir,
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

/// Reveal the latest final video (or the production's edit folder) in the OS
/// file browser.
#[post("/productions/{production_id}/edit/reveal")]
async fn reveal_edit_output(
    pool: web::Data<DbPool>,
    config: web::Data<ConfigManager>,
    path: web::Path<i32>,
) -> HttpResponse {
    let production_id = path.into_inner();

    // Prefer revealing the final video; fall back to the production edit folder.
    let target: Option<String> = {
        let mut conn = pool.get().expect("Failed to get DB connection");
        edit_service::get_latest_edit(&mut conn, production_id)
            .and_then(|e| e.output_path)
            .filter(|p| std::path::Path::new(p).exists())
    };

    let (reveal_path, select) = match target {
        Some(p) => (p, true),
        None => {
            let dir = config
                .get_edits_directory()
                .join(format!("production-{}", production_id));
            (dir.to_string_lossy().to_string(), false)
        }
    };

    match reveal_in_explorer(&reveal_path, select) {
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
        .service(get_latest_edit)
        .service(reveal_edit_output);
}
