mod config;
mod db;
mod models;
mod routes;
mod schema;
mod services;
mod utils;

use actix_cors::Cors;
use actix_web::{get, web, App, HttpResponse, HttpServer};
use log::info;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use crate::config::ConfigManager;
use crate::services::scanner::ScanMap;

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

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    // Load .env file if present
    if let Ok(contents) = std::fs::read_to_string("backend-rust/.env") {
        for line in contents.lines() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') { continue; }
            if let Some((key, value)) = line.split_once('=') {
                std::env::set_var(key.trim(), value.trim());
            }
        }
    } else if let Ok(contents) = std::fs::read_to_string(".env") {
        for line in contents.lines() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') { continue; }
            if let Some((key, value)) = line.split_once('=') {
                std::env::set_var(key.trim(), value.trim());
            }
        }
    }

    env_logger::init_from_env(env_logger::Env::default().default_filter_or("info"));

    // Initialize config
    let config_manager = ConfigManager::new("./data/config.json");
    let settings = config_manager.get_settings();

    let host = settings.host.clone();
    let port = settings.port;

    // Initialize database
    let pool = db::create_pool(&settings.database_path);
    db::init_db(&pool);

    // Ensure data directories exist (matches Python init_db behavior)
    config_manager.get_thumbnail_directory();

    // Scan progress map
    let scan_map: ScanMap = Arc::new(Mutex::new(HashMap::new()));

    let cors_origins = settings.cors_origins.clone();

    println!("============================================================");
    println!("  Video Manager API (Rust)");
    println!("============================================================");
    println!("  Server: http://{}:{}", host, port);
    println!("  Database: {}", settings.database_path);
    println!("  Video Directory: {}", settings.video_directory.as_deref().unwrap_or("Not configured"));
    println!("============================================================");
    println!();
    info!("Starting server on {}:{}", host, port);

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

        // Add allowed origins
        let cors = cors_origins.iter().fold(cors, |c, origin| {
            c.allowed_origin(origin)
        });

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
                    .configure(routes::productions::configure)
            )
    })
    .bind(format!("{}:{}", host, port))?
    .run()
    .await
}
