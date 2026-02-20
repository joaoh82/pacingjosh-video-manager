use actix_web::{get, post, options, web, HttpResponse};
use serde::Deserialize;

use crate::config::ConfigManager;
use crate::db::DbPool;
use crate::services::scanner::{self, ScanMap};

#[options("/scan")]
async fn scan_options() -> HttpResponse {
    HttpResponse::Ok().json(serde_json::json!({"status": "ok"}))
}

#[derive(Deserialize)]
pub struct ScanRequest {
    pub directory: String,
    #[serde(default = "default_save_config")]
    pub save_config: bool,
}

fn default_save_config() -> bool { true }

#[post("/scan")]
async fn start_scan(
    pool: web::Data<DbPool>,
    scan_map: web::Data<ScanMap>,
    config: web::Data<ConfigManager>,
    body: web::Json<ScanRequest>,
) -> HttpResponse {
    // Save config if requested
    if body.save_config {
        config.save_config(Some(body.directory.clone()), None, None).ok();
    }

    let settings = config.get_settings();

    match scanner::start_scan(
        body.directory.clone(),
        pool.get_ref().clone(),
        scan_map.get_ref().clone(),
        settings.supported_formats,
        settings.thumbnail_directory,
        settings.thumbnail_count,
        settings.thumbnail_width,
    ) {
        Ok(scan_id) => HttpResponse::Ok().json(serde_json::json!({
            "status": "started",
            "scan_id": scan_id,
            "message": format!("Scan started for directory: {}", body.directory),
        })),
        Err(e) => HttpResponse::BadRequest().json(serde_json::json!({
            "detail": e,
        })),
    }
}

#[get("/scan/status/{scan_id}")]
async fn scan_status(
    scan_map: web::Data<ScanMap>,
    path: web::Path<String>,
) -> HttpResponse {
    let scan_id = path.into_inner();

    match scanner::get_scan_progress(scan_map.get_ref(), &scan_id) {
        Some(progress) => HttpResponse::Ok().json(progress),
        None => HttpResponse::NotFound().json(serde_json::json!({
            "detail": format!("Scan not found: {}", scan_id),
        })),
    }
}

#[post("/scan/rescan")]
async fn rescan(
    pool: web::Data<DbPool>,
    scan_map: web::Data<ScanMap>,
    config: web::Data<ConfigManager>,
) -> HttpResponse {
    let settings = config.get_settings();

    let directory = match settings.video_directory {
        Some(dir) if !dir.is_empty() => dir,
        _ => {
            return HttpResponse::build(actix_web::http::StatusCode::PRECONDITION_REQUIRED)
                .json(serde_json::json!({
                    "detail": "Video directory not configured. Please configure first.",
                }));
        }
    };

    match scanner::start_scan(
        directory.clone(),
        pool.get_ref().clone(),
        scan_map.get_ref().clone(),
        settings.supported_formats,
        settings.thumbnail_directory,
        settings.thumbnail_count,
        settings.thumbnail_width,
    ) {
        Ok(scan_id) => HttpResponse::Ok().json(serde_json::json!({
            "status": "started",
            "scan_id": scan_id,
            "message": format!("Rescan started for directory: {}", directory),
        })),
        Err(e) => HttpResponse::InternalServerError().json(serde_json::json!({
            "detail": e,
        })),
    }
}

pub fn configure(cfg: &mut web::ServiceConfig) {
    cfg.service(scan_options)
        .service(start_scan)
        .service(scan_status)
        .service(rescan);
}
