//! Video Manager backend as an embeddable library.
//!
//! Exposes `run()` which starts the Actix-web HTTP server on the provided
//! socket address, using the provided `app_data_dir` as the base for the
//! SQLite database, thumbnails, and config file. Intended to be called from
//! a Tauri shell that owns the Tokio runtime.

pub mod config;
pub mod db;
pub mod models;
pub mod routes;
pub mod schema;
pub mod services;
pub mod utils;

use actix_cors::Cors;
use actix_web::{get, web, App, HttpResponse, HttpServer};
use log::info;
use std::collections::HashMap;
use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use crate::config::ConfigManager;
use crate::services::ffmpeg_service::FfmpegPaths;
use crate::services::scanner::ScanMap;

/// Paths and bind configuration passed in by the embedding process.
pub struct BackendPaths {
    /// Base directory for per-user app data (database, config.json, thumbnails).
    pub app_data_dir: PathBuf,
    /// Socket address to bind the HTTP server on.
    pub bind_addr: SocketAddr,
    /// Explicit ffmpeg/ffprobe binary paths. If `None`, the backend falls back
    /// to resolving `ffmpeg` and `ffprobe` via system PATH.
    pub ffmpeg: Option<FfmpegPaths>,
}

#[get("/")]
async fn root(config: web::Data<ConfigManager>) -> HttpResponse {
    let settings = config.get_settings();
    HttpResponse::Ok().json(serde_json::json!({
        "name": settings.app_name,
        "version": settings.app_version,
        "status": "running",
        "docs": "/docs",
        "message": "Welcome to Video Manager API",
    }))
}

#[get("/health")]
async fn health() -> HttpResponse {
    HttpResponse::Ok().json(serde_json::json!({
        "status": "healthy",
        "database": "connected",
    }))
}

/// Blocking variant: builds a fresh actix_web::rt::System on the current
/// thread and runs the server until it exits. Intended for embedding inside
/// a Tauri shell — call this from a dedicated OS thread spawned at startup
/// so it doesn't collide with Tauri's own Tokio runtime.
pub fn run_blocking(paths: BackendPaths) -> std::io::Result<()> {
    actix_web::rt::System::new().block_on(run(paths))
}

/// Start the backend HTTP server. This future runs for the lifetime of the
/// server — await it on an actix_web runtime. Standalone `main.rs` uses this
/// directly via `#[actix_web::main]`.
pub async fn run(paths: BackendPaths) -> std::io::Result<()> {
    // Install ffmpeg binary paths if provided
    if let Some(ffmpeg_paths) = paths.ffmpeg {
        services::ffmpeg_service::set_ffmpeg_paths(ffmpeg_paths);
    }

    // Initialize config rooted at app_data_dir
    let config_manager = ConfigManager::new(&paths.app_data_dir);
    let settings = config_manager.get_settings();

    // Initialize database
    let pool = db::create_pool(&settings.database_path);
    db::init_db(&pool);

    // Ensure thumbnail directory exists (matches Python init_db behavior)
    config_manager.get_thumbnail_directory();

    // Scan progress map
    let scan_map: ScanMap = Arc::new(Mutex::new(HashMap::new()));

    let cors_origins = settings.cors_origins.clone();

    println!("============================================================");
    println!("  Video Manager API (Rust)");
    println!("============================================================");
    println!("  Server: http://{}", paths.bind_addr);
    println!("  App data: {}", paths.app_data_dir.display());
    println!("  Database: {}", settings.database_path);
    println!(
        "  Video Directory: {}",
        settings.video_directory.as_deref().unwrap_or("Not configured")
    );
    println!("============================================================");
    info!("Starting server on {}", paths.bind_addr);

    let config_data = web::Data::new(config_manager);
    let pool_data = web::Data::new(pool);
    let scan_data = web::Data::new(scan_map);

    HttpServer::new(move || {
        let cors = Cors::default()
            .allowed_methods(vec!["GET", "POST", "PUT", "DELETE", "OPTIONS"])
            .allowed_headers(vec![
                actix_web::http::header::AUTHORIZATION,
                actix_web::http::header::ACCEPT,
                actix_web::http::header::CONTENT_TYPE,
            ])
            .supports_credentials()
            .max_age(3600);

        // Add allowed origins (plus tauri://localhost for the embedded WebView)
        let cors = cors_origins
            .iter()
            .fold(cors, |c, origin| c.allowed_origin(origin))
            .allowed_origin("tauri://localhost")
            .allowed_origin("http://tauri.localhost");

        App::new()
            .wrap(cors)
            .app_data(config_data.clone())
            .app_data(pool_data.clone())
            .app_data(scan_data.clone())
            .service(root)
            .service(health)
            .service(
                web::scope("/api")
                    .configure(routes::config_routes::configure)
                    .configure(routes::scan::configure)
                    .configure(routes::videos::configure)
                    .configure(routes::tags::configure)
                    .configure(routes::stream::configure)
                    .configure(routes::productions::configure),
            )
    })
    .bind(paths.bind_addr)?
    .run()
    .await
}
