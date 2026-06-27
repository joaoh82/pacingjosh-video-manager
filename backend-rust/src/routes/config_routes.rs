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

#[derive(serde::Deserialize)]
struct BrowseFileQuery {
    /// Which file types to show: "image" (images + GIFs) or anything else
    /// (audio/video, for background music). Defaults to media.
    #[serde(default)]
    kind: Option<String>,
}

#[get("/browse-file")]
async fn browse_file(query: web::Query<BrowseFileQuery>) -> HttpResponse {
    let kind = query.kind.clone().unwrap_or_default();
    let result = tokio::task::spawn_blocking(move || open_file_dialog(&kind)).await;

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
            "message": format!("Error opening file picker: {}", e),
        })),
    }
}

/// Open an OS file picker. `kind == "image"` filters to images + GIFs (for
/// overlay snippets); anything else filters to audio/video (background music).
fn open_file_dialog(kind: &str) -> Result<String, String> {
    let image = kind == "image";
    let title = if image { "Select an image or GIF" } else { "Select Background Music" };

    #[cfg(target_os = "windows")]
    {
        use std::process::Command;
        let filter = if image {
            "Images & GIFs (*.gif;*.png;*.jpg;*.jpeg;*.webp;*.bmp)|*.gif;*.png;*.jpg;*.jpeg;*.webp;*.bmp|All files (*.*)|*.*"
        } else {
            "Audio/Video (*.mp3;*.wav;*.m4a;*.aac;*.flac;*.ogg;*.mp4;*.mov)|*.mp3;*.wav;*.m4a;*.aac;*.flac;*.ogg;*.mp4;*.mov|All files (*.*)|*.*"
        };
        let script = format!(
            "Add-Type -AssemblyName System.Windows.Forms\n\
$dialog = New-Object System.Windows.Forms.OpenFileDialog\n\
$dialog.Title = \"{title}\"\n\
$dialog.Filter = \"{filter}\"\n\
$result = $dialog.ShowDialog()\n\
if ($result -eq [System.Windows.Forms.DialogResult]::OK) {{ Write-Output $dialog.FileName }}\n",
            title = title,
            filter = filter,
        );
        let output = Command::new("powershell")
            .args(["-NoProfile", "-Command", &script])
            .output()
            .map_err(|e| format!("Failed to run PowerShell: {}", e))?;

        let path = String::from_utf8_lossy(&output.stdout).trim().to_string();
        if path.is_empty() {
            Err("No file selected".to_string())
        } else {
            Ok(path)
        }
    }

    #[cfg(target_os = "macos")]
    {
        use std::process::Command;
        // Restrict to images (incl. GIF) when picking an overlay.
        let type_clause = if image { " of type {\"public.image\"}" } else { "" };
        let script = format!(
            "try\n\
    set theFile to choose file with prompt \"{title}\"{type_clause}\n\
    return POSIX path of theFile\n\
on error errMsg\n\
    return \"\"\n\
end try\n",
            title = title,
            type_clause = type_clause,
        );
        let output = Command::new("osascript")
            .args(["-e", &script])
            .output()
            .map_err(|e| format!("Failed to run osascript: {}", e))?;

        let path = String::from_utf8_lossy(&output.stdout).trim().to_string();
        if path.is_empty() {
            Err("No file selected".to_string())
        } else {
            Ok(path)
        }
    }

    #[cfg(target_os = "linux")]
    {
        use std::process::Command;
        let mut args: Vec<String> = vec![
            "--file-selection".to_string(),
            format!("--title={}", title),
        ];
        if image {
            args.push("--file-filter=Images & GIFs | *.gif *.png *.jpg *.jpeg *.webp *.bmp".to_string());
            args.push("--file-filter=All files | *".to_string());
        }
        let output = Command::new("zenity")
            .args(&args)
            .output()
            .map_err(|e| format!("Failed to run zenity: {}", e))?;

        let path = String::from_utf8_lossy(&output.stdout).trim().to_string();
        if path.is_empty() {
            Err("No file selected".to_string())
        } else {
            Ok(path)
        }
    }

    #[cfg(not(any(target_os = "windows", target_os = "macos", target_os = "linux")))]
    {
        let _ = title;
        Err("File picker not supported on this platform".to_string())
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
        .service(browse_folder)
        .service(browse_file);
}
