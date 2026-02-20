use actix_web::{get, post, options, web, HttpResponse};
use serde::Deserialize;

use crate::config::ConfigManager;

#[options("/config")]
async fn config_options() -> HttpResponse {
    HttpResponse::Ok().json(serde_json::json!({"status": "ok"}))
}

#[get("/config")]
async fn get_config(
    config: web::Data<ConfigManager>,
) -> HttpResponse {
    let settings = config.get_settings();
    HttpResponse::Ok().json(serde_json::json!({
        "configured": config.is_configured(),
        "video_directory": settings.video_directory,
        "supported_formats": settings.supported_formats,
        "thumbnail_count": settings.thumbnail_count,
    }))
}

#[derive(Deserialize)]
pub struct ConfigRequest {
    pub video_directory: String,
    #[serde(default = "default_thumb_count")]
    pub thumbnail_count: i32,
    #[serde(default = "default_thumb_width")]
    pub thumbnail_width: i32,
}

fn default_thumb_count() -> i32 { 5 }
fn default_thumb_width() -> i32 { 320 }

#[post("/config")]
async fn save_config(
    config: web::Data<ConfigManager>,
    body: web::Json<ConfigRequest>,
) -> HttpResponse {
    match config.save_config(
        Some(body.video_directory.clone()),
        Some(body.thumbnail_count),
        Some(body.thumbnail_width),
    ) {
        Ok(()) => HttpResponse::Ok().json(serde_json::json!({
            "status": "success",
            "message": "Configuration saved",
            "configured": config.is_configured(),
        })),
        Err(e) => HttpResponse::Ok().json(serde_json::json!({
            "status": "error",
            "message": format!("Failed to save configuration: {}", e),
        })),
    }
}

#[get("/browse-folder")]
async fn browse_folder() -> HttpResponse {
    let result = tokio::task::spawn_blocking(|| {
        open_folder_dialog()
    }).await;

    match result {
        Ok(Ok(path)) => HttpResponse::Ok().json(serde_json::json!({
            "success": true,
            "path": path,
        })),
        Ok(Err(msg)) => HttpResponse::Ok().json(serde_json::json!({
            "success": false,
            "message": msg,
        })),
        Err(e) => HttpResponse::Ok().json(serde_json::json!({
            "success": false,
            "message": format!("Error opening folder picker: {}", e),
        })),
    }
}

fn open_folder_dialog() -> Result<String, String> {
    #[cfg(target_os = "windows")]
    {
        use std::process::Command;
        // Use PowerShell with Windows Forms FolderBrowserDialog
        let script = r#"
Add-Type -AssemblyName System.Windows.Forms
$dialog = New-Object System.Windows.Forms.FolderBrowserDialog
$dialog.Description = "Select Video Directory"
$dialog.ShowNewFolderButton = $false
$result = $dialog.ShowDialog()
if ($result -eq [System.Windows.Forms.DialogResult]::OK) {
    Write-Output $dialog.SelectedPath
}
"#;
        let output = Command::new("powershell")
            .args(["-NoProfile", "-Command", script])
            .output()
            .map_err(|e| format!("Failed to run PowerShell: {}", e))?;

        let path = String::from_utf8_lossy(&output.stdout).trim().to_string();
        if path.is_empty() {
            Err("No folder selected".to_string())
        } else {
            Ok(path)
        }
    }

    #[cfg(target_os = "macos")]
    {
        use std::process::Command;
        let script = r#"
try
    set folderPath to choose folder with prompt "Select Video Directory" default location (path to home folder)
    return POSIX path of folderPath
on error errMsg
    return ""
end try
"#;
        let output = Command::new("osascript")
            .args(["-e", script])
            .output()
            .map_err(|e| format!("Failed to run osascript: {}", e))?;

        let path = String::from_utf8_lossy(&output.stdout).trim().to_string();
        if path.is_empty() {
            Err("No folder selected".to_string())
        } else {
            Ok(path)
        }
    }

    #[cfg(target_os = "linux")]
    {
        use std::process::Command;
        let output = Command::new("zenity")
            .args(["--file-selection", "--directory", "--title=Select Video Directory"])
            .output()
            .map_err(|e| format!("Failed to run zenity: {}", e))?;

        let path = String::from_utf8_lossy(&output.stdout).trim().to_string();
        if path.is_empty() {
            Err("No folder selected".to_string())
        } else {
            Ok(path)
        }
    }

    #[cfg(not(any(target_os = "windows", target_os = "macos", target_os = "linux")))]
    {
        Err("Folder picker not supported on this platform".to_string())
    }
}

pub fn configure(cfg: &mut web::ServiceConfig) {
    cfg.service(config_options)
        .service(get_config)
        .service(save_config)
        .service(browse_folder);
}
