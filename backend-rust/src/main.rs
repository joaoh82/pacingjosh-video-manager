//! Standalone binary entry point for the video-manager backend.
//!
//! The actual server logic lives in `lib.rs` (so the Tauri shell can embed it
//! without spawning a subprocess). This binary is kept for the standalone
//! `cargo run` development workflow.

use std::net::SocketAddr;
use std::path::PathBuf;

use video_manager_backend::{run, BackendPaths};

fn load_dotenv() {
    // Load .env file if present (search CWD-relative locations)
    let candidates = ["backend-rust/.env", ".env"];
    for candidate in &candidates {
        if let Ok(contents) = std::fs::read_to_string(candidate) {
            for line in contents.lines() {
                let line = line.trim();
                if line.is_empty() || line.starts_with('#') {
                    continue;
                }
                if let Some((key, value)) = line.split_once('=') {
                    std::env::set_var(key.trim(), value.trim());
                }
            }
            break;
        }
    }
}

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    load_dotenv();
    env_logger::init_from_env(env_logger::Env::default().default_filter_or("info"));

    // Standalone mode uses CWD-relative ./data for app data.
    let app_data_dir = PathBuf::from("./data");

    // Resolve bind address from env vars (HOST/PORT) with sensible defaults.
    let host = std::env::var("HOST").unwrap_or_else(|_| "127.0.0.1".to_string());
    let port: u16 = std::env::var("PORT")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(8000);
    let bind_addr: SocketAddr = format!("{}:{}", host, port)
        .parse()
        .expect("Invalid HOST/PORT");

    run(BackendPaths {
        app_data_dir,
        bind_addr,
        ffmpeg: None, // use system PATH
    })
    .await
}
