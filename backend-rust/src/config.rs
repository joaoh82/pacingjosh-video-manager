use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::sync::RwLock;
use log::{info, warn};

/// LLM provider configuration for AI content generation. Persisted in
/// config.json (no environment variables). API keys are stored here but are
/// never returned by the read API — only their presence is exposed.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AiSettings {
    /// Provider used for text generation: "gemini" | "openai" | "anthropic".
    pub text_provider: String,
    /// Model id for text generation (e.g. "gemini-2.0-flash", "gpt-4o").
    pub text_model: String,
    /// Provider used for transcription: "elevenlabs" | "openai" | "gemini".
    pub transcription_provider: String,
    /// Model id for transcription (e.g. "scribe_v1", "whisper-1", "gemini-2.0-flash").
    pub transcription_model: String,
    pub gemini_api_key: Option<String>,
    pub openai_api_key: Option<String>,
    pub anthropic_api_key: Option<String>,
    /// ElevenLabs API key, used by the Scribe speech-to-text service. Like the
    /// other keys it is write-only over the API. Defaults via serde so configs
    /// written before this field existed still load.
    #[serde(default)]
    pub elevenlabs_api_key: Option<String>,
    /// System/instruction prompt used to generate social copy from a transcript.
    /// Editable by the user. The literal token `{transcript}` is replaced with
    /// the video transcript at generation time (appended if absent). Defaults via
    /// serde so configs written before this field existed still load.
    #[serde(default = "default_system_prompt")]
    pub system_prompt: String,
    /// Instruction prompt used by the video-edit pipeline to turn a script plus
    /// the per-take timestamped transcripts into an edit decision list. Editable
    /// by the user. The literal tokens `{script}` and `{transcripts}` are replaced
    /// at run time. Defaults via serde so older configs still load.
    #[serde(default = "default_edit_prompt")]
    pub edit_prompt: String,
}

/// The default copy-generation prompt. Exposed so the API can offer a
/// "reset to default" affordance and so older configs can backfill it.
pub fn default_system_prompt() -> String {
    "You are a social media copywriter for short-form vertical videos (Instagram Reels, \
TikTok, YouTube Shorts). Based ONLY on the following video transcript, produce engaging, \
SEO-optimized copy.\n\n\
Return STRICT JSON (no markdown, no commentary) with exactly these keys:\n\
- \"thumbnail_texts\": array of 3 short, punchy on-screen thumbnail/hook text ideas (max ~6 words each)\n\
- \"instagram_description\": an Instagram caption with a strong hook and SEO keywords\n\
- \"tiktok_description\": a TikTok caption optimized for discovery\n\
- \"youtube_short_title\": a punchy, SEO-optimized YouTube Shorts title (max ~70 characters, no hashtags)\n\
- \"youtube_short_description\": a YouTube Shorts description with SEO keywords\n\
- \"youtube_short_tags\": array of UP TO 15 SEO keyword tags for YouTube (plain keywords, no leading #)\n\
- \"hashtags\": array of UP TO 5 relevant, high-traffic hashtags (each starting with #)\n\n\
Keep captions concise and native to each platform. Do not invent facts not in the transcript.\n\n\
TRANSCRIPT:\n\"\"\"\n{transcript}\n\"\"\"".to_string()
}

/// The default video-edit-planning prompt. Drives the pipeline that stitches
/// the best raw takes into a final clip. Exposed so the API can offer a
/// "reset to default" affordance and so older configs can backfill it.
pub fn default_edit_prompt() -> String {
    "You are a meticulous video editor. You are given a SCRIPT for a video and a set of \
RAW TAKES. Each take is one recording attempt; there are usually multiple takes per scene. \
For every take you get its video_id, filename, duration, and a transcript with word-level \
timestamps (in seconds).\n\n\
Your job is to assemble an edit decision list (EDL) that stitches the best takes into one \
final video, following the script from beginning to end.\n\n\
Guidelines:\n\
- Follow the SCRIPT order. Produce one entry per scene in the script, in timeline order.\n\
- For each scene, pick the take(s) whose transcript best matches that part of the script.\n\
- When several takes cover the same scene, the best take is USUALLY the latest one with the \
fewest filler words (\"um\", \"uh\", false starts, stumbles) — but use judgement; an earlier, \
cleaner take can win.\n\
- The creator may have RE-SHOT an early scene near the end of the session. Place every clip \
where it belongs in the SCRIPT's timeline, regardless of recording order.\n\
- Creators often warm up by saying something like \"Hey <name>\" or repeating the first words \
before the real take. Trim that lead-in: set the clip `start` to the moment the real scripted \
line begins.\n\
- Trim trailing dead air, restarts, and out-of-script chatter by choosing a tight `end`.\n\
- `start` and `end` are in seconds and MUST fall within that take's duration, with end > start.\n\n\
Return STRICT JSON (no markdown, no commentary) with exactly this shape:\n\
{\n\
  \"scenes\": [\n\
    {\n\
      \"scene_number\": 1,\n\
      \"scene_description\": \"short label for this scene\",\n\
      \"clips\": [\n\
        { \"video_id\": 12, \"start\": 2.4, \"end\": 11.0, \"reason\": \"why this take/range\" }\n\
      ]\n\
    }\n\
  ]\n\
}\n\n\
SCRIPT:\n\"\"\"\n{script}\n\"\"\"\n\n\
RAW TAKES (with word-level timestamps):\n\"\"\"\n{transcripts}\n\"\"\"".to_string()
}

impl Default for AiSettings {
    fn default() -> Self {
        Self {
            text_provider: "gemini".to_string(),
            text_model: "gemini-2.0-flash".to_string(),
            transcription_provider: "gemini".to_string(),
            transcription_model: "gemini-2.0-flash".to_string(),
            gemini_api_key: None,
            openai_api_key: None,
            anthropic_api_key: None,
            elevenlabs_api_key: None,
            system_prompt: default_system_prompt(),
            edit_prompt: default_edit_prompt(),
        }
    }
}

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
    pub ai: AiSettings,
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
            ai: AiSettings::default(),
        }
    }
}

impl Settings {
    /// Load settings with the given app_data_dir as the base for default paths,
    /// then apply env-var overrides on top.
    pub fn from_env_with_base(app_data_dir: &Path) -> Self {
        let mut settings = Self::default();

        // Rebase default paths onto app_data_dir
        settings.database_path = app_data_dir
            .join("database.db")
            .to_string_lossy()
            .into_owned();
        settings.thumbnail_directory = app_data_dir
            .join("thumbnails")
            .to_string_lossy()
            .into_owned();

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

    /// Legacy helper — defaults to CWD-based `./data` directory.
    #[allow(dead_code)]
    pub fn from_env() -> Self {
        Self::from_env_with_base(Path::new("./data"))
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
    #[serde(skip_serializing_if = "Option::is_none")]
    ai: Option<AiSettings>,
}

impl ConfigManager {
    /// Create a ConfigManager with the given app_data_dir as the base for all
    /// default paths (database, thumbnails, config file). Env vars and the
    /// on-disk config.json still override these defaults.
    pub fn new(app_data_dir: &Path) -> Self {
        // Ensure app data dir exists
        std::fs::create_dir_all(app_data_dir).ok();

        let path = app_data_dir.join("config.json");
        let mut settings = Settings::from_env_with_base(app_data_dir);

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
                            if let Some(v) = cfg.ai { settings.ai = v; }
                            info!("Loaded config from {}", path.display());
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

    /// Legacy constructor accepting a string path to the config file.
    #[allow(dead_code)]
    pub fn from_config_path(config_path: &str) -> Self {
        let path = PathBuf::from(config_path);
        let app_data_dir = path
            .parent()
            .map(|p| p.to_path_buf())
            .unwrap_or_else(|| PathBuf::from("./data"));
        Self::new(&app_data_dir)
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
            ai: Some(settings.ai.clone()),
        };

        let json = serde_json::to_string_pretty(&cfg).map_err(|e| e.to_string())?;
        std::fs::write(&self.config_path, json).map_err(|e| e.to_string())?;

        info!("Configuration saved to {:?}", self.config_path);
        Ok(())
    }

    /// Update AI/LLM settings and persist the full config to disk. Other config
    /// fields (video directory, thumbnails) are preserved.
    pub fn save_ai_settings(&self, ai: AiSettings) -> Result<(), String> {
        {
            let mut settings = self.settings.write().map_err(|e| e.to_string())?;
            settings.ai = ai;
        }

        let settings = self.settings.read().map_err(|e| e.to_string())?;

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
            ai: Some(settings.ai.clone()),
        };

        let json = serde_json::to_string_pretty(&cfg).map_err(|e| e.to_string())?;
        std::fs::write(&self.config_path, json).map_err(|e| e.to_string())?;

        info!("AI settings saved to {:?}", self.config_path);
        Ok(())
    }

    pub fn get_ai_settings(&self) -> AiSettings {
        self.settings.read().unwrap().ai.clone()
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

    /// Base directory for video-edit pipeline output (EDL JSON + final clips),
    /// rooted at the app-data dir alongside the database and thumbnails.
    pub fn get_edits_directory(&self) -> PathBuf {
        let base = self
            .config_path
            .parent()
            .map(|p| p.to_path_buf())
            .unwrap_or_else(|| PathBuf::from("."));
        let path = base.join("edits");
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
