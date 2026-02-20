use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::RwLock;
use log::{info, warn};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Settings {
    pub app_name: String,
    pub app_version: String,
    pub host: String,
    pub port: u16,
    pub database_path: String,
    pub video_directory: Option<String>,
    pub supported_formats: Vec<String>,
    pub thumbnail_directory: String,
    pub thumbnail_count: i32,
    pub thumbnail_width: i32,
    pub cors_origins: Vec<String>,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            app_name: "Video Manager".to_string(),
            app_version: "1.0.0".to_string(),
            host: "127.0.0.1".to_string(),
            port: 8000,
            database_path: "./data/database.db".to_string(),
            video_directory: None,
            supported_formats: vec![
                ".mp4".into(), ".mov".into(), ".avi".into(), ".mkv".into(),
                ".webm".into(), ".flv".into(), ".wmv".into(),
            ],
            thumbnail_directory: "./data/thumbnails".to_string(),
            thumbnail_count: 5,
            thumbnail_width: 320,
            cors_origins: vec![
                "http://localhost:3000".into(),
                "http://127.0.0.1:3000".into(),
                "http://localhost:3002".into(),
                "http://127.0.0.1:3002".into(),
            ],
        }
    }
}

impl Settings {
    /// Load settings from environment variables, falling back to defaults
    pub fn from_env() -> Self {
        let mut settings = Self::default();

        if let Ok(val) = std::env::var("HOST") { settings.host = val; }
        if let Ok(val) = std::env::var("PORT") {
            if let Ok(p) = val.parse() { settings.port = p; }
        }
        if let Ok(val) = std::env::var("DATABASE_PATH") { settings.database_path = val; }
        if let Ok(val) = std::env::var("VIDEO_DIRECTORY") { settings.video_directory = Some(val); }
        if let Ok(val) = std::env::var("THUMBNAIL_DIRECTORY") { settings.thumbnail_directory = val; }
        if let Ok(val) = std::env::var("THUMBNAIL_COUNT") {
            if let Ok(n) = val.parse() { settings.thumbnail_count = n; }
        }
        if let Ok(val) = std::env::var("THUMBNAIL_WIDTH") {
            if let Ok(n) = val.parse() { settings.thumbnail_width = n; }
        }
        settings
    }
}

/// Persistent config manager with JSON file backing
pub struct ConfigManager {
    pub config_path: PathBuf,
    pub settings: RwLock<Settings>,
}

/// JSON structure for config file persistence
#[derive(Debug, Serialize, Deserialize, Default)]
struct ConfigFile {
    #[serde(skip_serializing_if = "Option::is_none")]
    video_directory: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    database_path: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    thumbnail_directory: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    thumbnail_count: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    thumbnail_width: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    supported_formats: Option<Vec<String>>,
}

impl ConfigManager {
    pub fn new(config_path: &str) -> Self {
        let mut settings = Settings::from_env();
        let path = PathBuf::from(config_path);

        // Load from config file if exists
        if path.exists() {
            match std::fs::read_to_string(&path) {
                Ok(contents) => {
                    match serde_json::from_str::<ConfigFile>(&contents) {
                        Ok(cfg) => {
                            if let Some(v) = cfg.video_directory { settings.video_directory = Some(v); }
                            if let Some(v) = cfg.database_path { settings.database_path = v; }
                            if let Some(v) = cfg.thumbnail_directory { settings.thumbnail_directory = v; }
                            if let Some(v) = cfg.thumbnail_count { settings.thumbnail_count = v; }
                            if let Some(v) = cfg.thumbnail_width { settings.thumbnail_width = v; }
                            if let Some(v) = cfg.supported_formats { settings.supported_formats = v; }
                            info!("Loaded config from {}", config_path);
                        }
                        Err(e) => warn!("Failed to parse config file: {}", e),
                    }
                }
                Err(e) => warn!("Failed to read config file: {}", e),
            }
        }

        Self {
            config_path: path,
            settings: RwLock::new(settings),
        }
    }

    pub fn save_config(
        &self,
        video_directory: Option<String>,
        thumbnail_count: Option<i32>,
        thumbnail_width: Option<i32>,
    ) -> Result<(), String> {
        let mut settings = self.settings.write().map_err(|e| e.to_string())?;

        if let Some(ref v) = video_directory { settings.video_directory = Some(v.clone()); }
        if let Some(v) = thumbnail_count { settings.thumbnail_count = v; }
        if let Some(v) = thumbnail_width { settings.thumbnail_width = v; }

        // Ensure parent directory exists
        if let Some(parent) = self.config_path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }

        let cfg = ConfigFile {
            video_directory: settings.video_directory.clone(),
            database_path: Some(settings.database_path.clone()),
            thumbnail_directory: Some(settings.thumbnail_directory.clone()),
            thumbnail_count: Some(settings.thumbnail_count),
            thumbnail_width: Some(settings.thumbnail_width),
            supported_formats: Some(settings.supported_formats.clone()),
        };

        let json = serde_json::to_string_pretty(&cfg).map_err(|e| e.to_string())?;
        std::fs::write(&self.config_path, json).map_err(|e| e.to_string())?;

        info!("Configuration saved to {:?}", self.config_path);
        Ok(())
    }

    pub fn is_configured(&self) -> bool {
        let settings = self.settings.read().unwrap();
        settings.video_directory.is_some() && self.config_path.exists()
    }

    pub fn get_settings(&self) -> Settings {
        self.settings.read().unwrap().clone()
    }

    pub fn get_thumbnail_directory(&self) -> PathBuf {
        let settings = self.settings.read().unwrap();
        let path = PathBuf::from(&settings.thumbnail_directory);
        std::fs::create_dir_all(&path).ok();
        path
    }

    #[allow(dead_code)]
    pub fn get_database_path(&self) -> PathBuf {
        let settings = self.settings.read().unwrap();
        let path = PathBuf::from(&settings.database_path);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).ok();
        }
        path
    }
}
