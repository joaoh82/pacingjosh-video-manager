// Prevents additional console window on Windows in release
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::net::{SocketAddr, TcpListener};
use std::path::PathBuf;

use tauri::{path::BaseDirectory, AppHandle, Manager, WindowEvent};
use tauri_plugin_dialog::DialogExt;

use video_manager_backend::services::ffmpeg_service::FfmpegPaths;
use video_manager_backend::{run_blocking, BackendPaths};

/// Pick a directory using the OS-native folder picker.
#[tauri::command]
fn pick_directory(app: AppHandle) -> Result<Option<String>, String> {
    // tauri-plugin-dialog 2.x returns results via callback; we use the
    // blocking helper to keep the IPC signature simple.
    let picked = app.dialog().file().blocking_pick_folder();
    Ok(picked.map(|p| p.to_string()))
}

/// Resolve the bundled ffmpeg/ffprobe sidecar binaries. On desktop Tauri, the
/// sidecars are placed next to the app binary at runtime (Tauri takes care of
/// platform-specific naming via the target triple suffix in `tauri.conf.json`).
/// If the resolve fails (dev builds without sidecars installed), fall back to
/// `None` so the backend uses system PATH.
fn resolve_ffmpeg_paths(app: &AppHandle) -> Option<FfmpegPaths> {
    let resolver = app.path();

    // Tauri's sidecar resolution renames the bundled binary to drop the target
    // triple at install time, so at runtime we look for `ffmpeg` and `ffprobe`
    // inside the Resource directory. Platform-specific `.exe` is added
    // automatically by `Command` on Windows.
    let ffmpeg_name = if cfg!(windows) { "ffmpeg.exe" } else { "ffmpeg" };
    let ffprobe_name = if cfg!(windows) { "ffprobe.exe" } else { "ffprobe" };

    let ffmpeg = resolver.resolve(ffmpeg_name, BaseDirectory::Resource).ok()?;
    let ffprobe = resolver.resolve(ffprobe_name, BaseDirectory::Resource).ok()?;

    if ffmpeg.exists() && ffprobe.exists() {
        log::info!("Using bundled ffmpeg: {}", ffmpeg.display());
        log::info!("Using bundled ffprobe: {}", ffprobe.display());
        Some(FfmpegPaths { ffmpeg, ffprobe })
    } else {
        log::warn!("Bundled ffmpeg sidecars not found — falling back to system PATH");
        None
    }
}

/// Pick a free local port. We bind to port 0, read the assigned port, then
/// drop the listener. There is a small race window between dropping and the
/// Actix server re-binding, but in practice on localhost it's not an issue
/// because nothing else is contending for the same port in that microsecond.
fn pick_free_port() -> std::io::Result<u16> {
    let listener = TcpListener::bind("127.0.0.1:0")?;
    let port = listener.local_addr()?.port();
    drop(listener);
    Ok(port)
}

fn main() {
    env_logger::init_from_env(env_logger::Env::default().default_filter_or("info"));

    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_dialog::init())
        .invoke_handler(tauri::generate_handler![pick_directory])
        .setup(|app| {
            // Resolve per-user app data directory (e.g. %APPDATA%\com.pacingjosh.video-manager)
            let app_data_dir: PathBuf = app
                .path()
                .app_data_dir()
                .expect("failed to resolve app_data_dir");
            std::fs::create_dir_all(&app_data_dir).ok();
            log::info!("App data dir: {}", app_data_dir.display());

            // Resolve bundled ffmpeg sidecars (or fall back to system PATH)
            let ffmpeg_paths = resolve_ffmpeg_paths(&app.handle());

            // Pick a free port for the embedded backend
            let port = pick_free_port().expect("failed to pick free port");
            let bind_addr: SocketAddr =
                format!("127.0.0.1:{port}").parse().expect("invalid bind addr");
            log::info!("Embedded backend will bind to {bind_addr}");

            // Inject the backend URL into the WebView before any script runs.
            // The frontend's api.ts reads `window.__VMAN_API__` at first fetch.
            let api_url = format!("http://127.0.0.1:{port}");
            let init_script = format!(
                "window.__VMAN_API__ = {:?};",
                api_url
            );
            if let Some(window) = app.get_webview_window("main") {
                // Use eval so it's set even for already-loaded pages in dev mode.
                let _ = window.eval(&init_script);
            }
            // Also register as an init script for future navigations.
            // (Tauri's builder-level `initialization_script` would need to be
            // set before `.setup()` runs, so this eval covers the dev case.)

            // Spawn the Actix server on a dedicated OS thread with its own
            // actix_web::rt::System. This avoids colliding with Tauri's Tokio.
            let paths = BackendPaths {
                app_data_dir,
                bind_addr,
                ffmpeg: ffmpeg_paths,
            };
            std::thread::Builder::new()
                .name("video-manager-backend".into())
                .spawn(move || {
                    if let Err(e) = run_blocking(paths) {
                        log::error!("Backend server exited with error: {e}");
                    }
                })
                .expect("failed to spawn backend thread");

            Ok(())
        })
        .on_window_event(|_window, event| {
            if let WindowEvent::Destroyed = event {
                // The OS will reclaim the backend thread + port when the
                // process exits. No explicit teardown needed.
            }
        })
        .run(tauri::generate_context!())
        .expect("error while running Video Manager");
}
