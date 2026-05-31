use actix_web::{get, post, put, web, HttpResponse};
use serde::Deserialize;
use std::path::{Path, PathBuf};

use crate::config::{default_system_prompt, ConfigManager};
use crate::db::DbPool;
use crate::models::AiGenerationResponse;
use crate::services::{ai_service, ffmpeg_service, video_service};

// --- AI settings -------------------------------------------------------------

#[get("/ai/settings")]
async fn get_ai_settings(config: web::Data<ConfigManager>) -> HttpResponse {
    let ai = config.get_ai_settings();
    HttpResponse::Ok().json(serde_json::json!({
        "text_provider": ai.text_provider,
        "text_model": ai.text_model,
        "transcription_provider": ai.transcription_provider,
        "transcription_model": ai.transcription_model,
        "gemini_api_key_set": key_set(&ai.gemini_api_key),
        "openai_api_key_set": key_set(&ai.openai_api_key),
        "anthropic_api_key_set": key_set(&ai.anthropic_api_key),
        "system_prompt": ai.system_prompt,
        "default_system_prompt": default_system_prompt(),
    }))
}

fn key_set(key: &Option<String>) -> bool {
    key.as_deref().map(|k| !k.is_empty()).unwrap_or(false)
}

#[derive(Deserialize)]
pub struct AiSettingsRequest {
    pub text_provider: Option<String>,
    pub text_model: Option<String>,
    pub transcription_provider: Option<String>,
    pub transcription_model: Option<String>,
    /// Blank or omitted leaves the stored key unchanged.
    pub gemini_api_key: Option<String>,
    pub openai_api_key: Option<String>,
    pub anthropic_api_key: Option<String>,
    /// Copy-generation prompt. Omitted leaves it unchanged; an empty string
    /// resets it to the built-in default.
    pub system_prompt: Option<String>,
}

#[put("/ai/settings")]
async fn save_ai_settings(
    config: web::Data<ConfigManager>,
    body: web::Json<AiSettingsRequest>,
) -> HttpResponse {
    let mut ai = config.get_ai_settings();

    if let Some(v) = &body.text_provider { ai.text_provider = v.clone(); }
    if let Some(v) = &body.text_model { ai.text_model = v.clone(); }
    if let Some(v) = &body.transcription_provider { ai.transcription_provider = v.clone(); }
    if let Some(v) = &body.transcription_model { ai.transcription_model = v.clone(); }

    // Keys are write-only: only overwrite when a non-empty value is supplied.
    update_key(&mut ai.gemini_api_key, &body.gemini_api_key);
    update_key(&mut ai.openai_api_key, &body.openai_api_key);
    update_key(&mut ai.anthropic_api_key, &body.anthropic_api_key);

    // An empty/whitespace prompt resets to the default; otherwise store as given.
    if let Some(p) = &body.system_prompt {
        ai.system_prompt = if p.trim().is_empty() {
            default_system_prompt()
        } else {
            p.clone()
        };
    }

    match config.save_ai_settings(ai) {
        Ok(()) => HttpResponse::Ok().json(serde_json::json!({
            "status": "success",
            "message": "AI settings saved",
        })),
        Err(e) => HttpResponse::InternalServerError().json(serde_json::json!({
            "detail": format!("Failed to save AI settings: {}", e),
        })),
    }
}

fn update_key(current: &mut Option<String>, incoming: &Option<String>) {
    if let Some(v) = incoming {
        if !v.is_empty() {
            *current = Some(v.clone());
        }
    }
}

// --- Generation --------------------------------------------------------------

#[get("/ai/generation/{video_id}")]
async fn get_generation(pool: web::Data<DbPool>, path: web::Path<i32>) -> HttpResponse {
    let mut conn = pool.get().expect("Failed to get DB connection");
    let video_id = path.into_inner();

    match ai_service::get_generation(&mut conn, video_id) {
        Some(g) => HttpResponse::Ok().json(AiGenerationResponse::from(g)),
        None => HttpResponse::Ok().json(serde_json::Value::Null),
    }
}

#[derive(Deserialize, Default)]
#[serde(default)]
pub struct GenerateRequest {
    pub regenerate: bool,
}

#[post("/ai/generate/{video_id}")]
async fn generate(
    pool: web::Data<DbPool>,
    config: web::Data<ConfigManager>,
    path: web::Path<i32>,
    body: web::Json<GenerateRequest>,
) -> HttpResponse {
    let video_id = path.into_inner();

    // Load the video and validate orientation up front.
    let (file_path, orientation) = {
        let mut conn = pool.get().expect("Failed to get DB connection");
        match video_service::get_video(&mut conn, video_id) {
            Some(v) => (v.file_path, v.orientation),
            None => {
                return HttpResponse::NotFound().json(serde_json::json!({
                    "detail": format!("Video not found: {}", video_id),
                }));
            }
        }
    };

    if orientation.as_deref() != Some("portrait") {
        return HttpResponse::BadRequest().json(serde_json::json!({
            "detail": "AI generation is only available for portrait (vertical) videos.",
        }));
    }

    // Return cached result unless a regenerate was requested.
    if !body.regenerate {
        let mut conn = pool.get().expect("Failed to get DB connection");
        if let Some(g) = ai_service::get_generation(&mut conn, video_id) {
            return HttpResponse::Ok().json(AiGenerationResponse::from(g));
        }
    }

    if !Path::new(&file_path).exists() {
        return HttpResponse::NotFound().json(serde_json::json!({
            "detail": "Video file no longer exists on disk.",
        }));
    }

    let ai = config.get_ai_settings();

    // Extract a compact audio track (blocking ffmpeg) off the async executor.
    let temp_dir: PathBuf = std::env::temp_dir().join("video-manager-audio");
    let fp = file_path.clone();
    let td = temp_dir.clone();
    let audio_path = match web::block(move || ffmpeg_service::extract_audio(Path::new(&fp), &td)).await {
        Ok(Ok(p)) => p,
        Ok(Err(e)) => {
            return HttpResponse::InternalServerError().json(serde_json::json!({ "detail": e }));
        }
        Err(e) => {
            return HttpResponse::InternalServerError()
                .json(serde_json::json!({ "detail": format!("Audio extraction task failed: {}", e) }));
        }
    };

    // Transcribe, then generate copy.
    let result = async {
        let transcript = ai_service::transcribe(&audio_path, &ai).await?;
        let content = ai_service::generate_content(&transcript, &ai).await?;
        Ok::<_, String>((transcript, content))
    }
    .await;

    let _ = std::fs::remove_file(&audio_path);

    let (transcript, content) = match result {
        Ok(v) => v,
        Err(e) => {
            return HttpResponse::InternalServerError().json(serde_json::json!({ "detail": e }));
        }
    };

    let mut conn = pool.get().expect("Failed to get DB connection");
    match ai_service::upsert_generation(
        &mut conn,
        video_id,
        &transcript,
        &content,
        &ai.text_provider,
        &ai.text_model,
    ) {
        Ok(saved) => HttpResponse::Ok().json(AiGenerationResponse::from(saved)),
        Err(e) => HttpResponse::InternalServerError().json(serde_json::json!({ "detail": e })),
    }
}

pub fn configure(cfg: &mut web::ServiceConfig) {
    cfg.service(get_ai_settings)
        .service(save_ai_settings)
        .service(get_generation)
        .service(generate);
}
