//! AI content generation: audio transcription and SEO copy generation for
//! portrait/shorts videos. Providers: Gemini, OpenAI, Anthropic. API keys and
//! model selections come from `AiSettings` (persisted in config.json).

use base64::{engine::general_purpose::STANDARD, Engine};
use chrono::Utc;
use diesel::prelude::*;
use diesel::sqlite::SqliteConnection;
use log::error;
use serde::Deserialize;
use std::path::Path;

use crate::config::AiSettings;
use crate::models::{AiGeneration, NewAiGeneration};
use crate::schema::ai_generations;

const GEMINI_BASE: &str = "https://generativelanguage.googleapis.com/v1beta/models";
const OPENAI_BASE: &str = "https://api.openai.com/v1";
const ANTHROPIC_BASE: &str = "https://api.anthropic.com/v1";

/// Structured copy returned by the text model.
#[derive(Debug, Deserialize, Default, Clone)]
pub struct GeneratedContent {
    #[serde(default)]
    pub thumbnail_texts: Vec<String>,
    #[serde(default)]
    pub instagram_description: String,
    #[serde(default)]
    pub tiktok_description: String,
    #[serde(default)]
    pub youtube_short_title: String,
    #[serde(default)]
    pub youtube_short_description: String,
    #[serde(default)]
    pub youtube_short_tags: Vec<String>,
    #[serde(default)]
    pub hashtags: Vec<String>,
}

fn client() -> reqwest::Client {
    reqwest::Client::new()
}

// --- Transcription -----------------------------------------------------------

/// Transcribe an audio file using the configured transcription provider.
pub async fn transcribe(audio_path: &Path, ai: &AiSettings) -> Result<String, String> {
    let bytes = std::fs::read(audio_path).map_err(|e| format!("Failed to read audio: {}", e))?;

    match ai.transcription_provider.as_str() {
        "openai" => transcribe_openai(bytes, ai).await,
        "gemini" => transcribe_gemini(bytes, ai).await,
        other => Err(format!("Unsupported transcription provider: {}", other)),
    }
}

async fn transcribe_openai(bytes: Vec<u8>, ai: &AiSettings) -> Result<String, String> {
    let key = ai
        .openai_api_key
        .as_deref()
        .filter(|k| !k.is_empty())
        .ok_or("OpenAI API key is not configured")?;

    let part = reqwest::multipart::Part::bytes(bytes)
        .file_name("audio.mp3")
        .mime_str("audio/mpeg")
        .map_err(|e| e.to_string())?;
    let form = reqwest::multipart::Form::new()
        .text("model", ai.transcription_model.clone())
        .part("file", part);

    let resp = client()
        .post(format!("{}/audio/transcriptions", OPENAI_BASE))
        .bearer_auth(key)
        .multipart(form)
        .send()
        .await
        .map_err(|e| format!("OpenAI request failed: {}", e))?;

    let status = resp.status();
    let text = resp.text().await.unwrap_or_default();
    if !status.is_success() {
        return Err(format!("OpenAI transcription error ({}): {}", status, text));
    }

    let parsed: serde_json::Value =
        serde_json::from_str(&text).map_err(|e| format!("Bad OpenAI response: {}", e))?;
    parsed["text"]
        .as_str()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .ok_or_else(|| "OpenAI returned an empty transcript".to_string())
}

async fn transcribe_gemini(bytes: Vec<u8>, ai: &AiSettings) -> Result<String, String> {
    let key = ai
        .gemini_api_key
        .as_deref()
        .filter(|k| !k.is_empty())
        .ok_or("Gemini API key is not configured")?;

    let body = serde_json::json!({
        "contents": [{
            "parts": [
                { "text": "Transcribe this audio verbatim. Output only the spoken words as plain text, no commentary." },
                { "inline_data": { "mime_type": "audio/mpeg", "data": STANDARD.encode(&bytes) } }
            ]
        }]
    });

    let url = format!("{}/{}:generateContent?key={}", GEMINI_BASE, ai.transcription_model, key);
    let resp = client()
        .post(url)
        .json(&body)
        .send()
        .await
        .map_err(|e| format!("Gemini request failed: {}", e))?;

    let status = resp.status();
    let text = resp.text().await.unwrap_or_default();
    if !status.is_success() {
        return Err(format!("Gemini transcription error ({}): {}", status, text));
    }

    let transcript = gemini_extract_text(&text)?;
    if transcript.trim().is_empty() {
        return Err("Gemini returned an empty transcript".to_string());
    }
    Ok(transcript.trim().to_string())
}

// --- Content generation ------------------------------------------------------

/// Build the final prompt from the user-configurable template. The literal
/// token `{transcript}` is substituted with the transcript; if the template
/// omits it, the transcript is appended so generation still has the source text.
fn build_prompt(template: &str, transcript: &str) -> String {
    if template.contains("{transcript}") {
        template.replace("{transcript}", transcript)
    } else {
        format!("{}\n\nTRANSCRIPT:\n\"\"\"\n{}\n\"\"\"", template, transcript)
    }
}

/// Generate social copy from a transcript using the configured text provider.
pub async fn generate_content(
    transcript: &str,
    ai: &AiSettings,
) -> Result<GeneratedContent, String> {
    let prompt = build_prompt(&ai.system_prompt, transcript);
    let raw = match ai.text_provider.as_str() {
        "gemini" => generate_gemini(&prompt, ai).await?,
        "openai" => generate_openai(&prompt, ai).await?,
        "anthropic" => generate_anthropic(&prompt, ai).await?,
        other => return Err(format!("Unsupported text provider: {}", other)),
    };

    let json = extract_json(&raw);
    let mut content: GeneratedContent =
        serde_json::from_str(json).map_err(|e| format!("Failed to parse model JSON: {} — raw: {}", e, raw))?;

    // Enforce the array-size contracts regardless of model behavior.
    content.hashtags.truncate(5);
    content.youtube_short_tags.truncate(15);
    Ok(content)
}

async fn generate_gemini(prompt: &str, ai: &AiSettings) -> Result<String, String> {
    let key = ai
        .gemini_api_key
        .as_deref()
        .filter(|k| !k.is_empty())
        .ok_or("Gemini API key is not configured")?;

    let body = serde_json::json!({
        "contents": [{ "parts": [{ "text": prompt }] }],
        "generationConfig": { "responseMimeType": "application/json" }
    });

    let url = format!("{}/{}:generateContent?key={}", GEMINI_BASE, ai.text_model, key);
    let resp = client()
        .post(url)
        .json(&body)
        .send()
        .await
        .map_err(|e| format!("Gemini request failed: {}", e))?;

    let status = resp.status();
    let text = resp.text().await.unwrap_or_default();
    if !status.is_success() {
        return Err(format!("Gemini generation error ({}): {}", status, text));
    }
    gemini_extract_text(&text)
}

async fn generate_openai(prompt: &str, ai: &AiSettings) -> Result<String, String> {
    let key = ai
        .openai_api_key
        .as_deref()
        .filter(|k| !k.is_empty())
        .ok_or("OpenAI API key is not configured")?;

    let body = serde_json::json!({
        "model": ai.text_model,
        "messages": [{ "role": "user", "content": prompt }],
        "response_format": { "type": "json_object" }
    });

    let resp = client()
        .post(format!("{}/chat/completions", OPENAI_BASE))
        .bearer_auth(key)
        .json(&body)
        .send()
        .await
        .map_err(|e| format!("OpenAI request failed: {}", e))?;

    let status = resp.status();
    let text = resp.text().await.unwrap_or_default();
    if !status.is_success() {
        return Err(format!("OpenAI generation error ({}): {}", status, text));
    }

    let parsed: serde_json::Value =
        serde_json::from_str(&text).map_err(|e| format!("Bad OpenAI response: {}", e))?;
    parsed["choices"][0]["message"]["content"]
        .as_str()
        .map(|s| s.to_string())
        .ok_or_else(|| "OpenAI returned no content".to_string())
}

async fn generate_anthropic(prompt: &str, ai: &AiSettings) -> Result<String, String> {
    let key = ai
        .anthropic_api_key
        .as_deref()
        .filter(|k| !k.is_empty())
        .ok_or("Anthropic API key is not configured")?;

    let body = serde_json::json!({
        "model": ai.text_model,
        "max_tokens": 1024,
        "messages": [{ "role": "user", "content": prompt }]
    });

    let resp = client()
        .post(format!("{}/messages", ANTHROPIC_BASE))
        .header("x-api-key", key)
        .header("anthropic-version", "2023-06-01")
        .json(&body)
        .send()
        .await
        .map_err(|e| format!("Anthropic request failed: {}", e))?;

    let status = resp.status();
    let text = resp.text().await.unwrap_or_default();
    if !status.is_success() {
        return Err(format!("Anthropic generation error ({}): {}", status, text));
    }

    let parsed: serde_json::Value =
        serde_json::from_str(&text).map_err(|e| format!("Bad Anthropic response: {}", e))?;
    parsed["content"][0]["text"]
        .as_str()
        .map(|s| s.to_string())
        .ok_or_else(|| "Anthropic returned no content".to_string())
}

// --- Helpers -----------------------------------------------------------------

/// Pull `candidates[0].content.parts[*].text` out of a Gemini response.
fn gemini_extract_text(body: &str) -> Result<String, String> {
    let parsed: serde_json::Value =
        serde_json::from_str(body).map_err(|e| format!("Bad Gemini response: {}", e))?;

    let parts = parsed["candidates"][0]["content"]["parts"].as_array();
    match parts {
        Some(parts) => {
            let combined: String = parts
                .iter()
                .filter_map(|p| p["text"].as_str())
                .collect::<Vec<_>>()
                .join("");
            Ok(combined)
        }
        None => Err(format!("Gemini response missing content: {}", body)),
    }
}

/// Best-effort extraction of a JSON object from a model response that may be
/// wrapped in markdown fences or prose.
fn extract_json(text: &str) -> &str {
    let trimmed = text.trim();
    if let (Some(start), Some(end)) = (trimmed.find('{'), trimmed.rfind('}')) {
        if end >= start {
            return &trimmed[start..=end];
        }
    }
    trimmed
}

// --- Persistence -------------------------------------------------------------

pub fn get_generation(conn: &mut SqliteConnection, video_id: i32) -> Option<AiGeneration> {
    ai_generations::table
        .filter(ai_generations::video_id.eq(video_id))
        .first(conn)
        .ok()
}

/// Insert or replace the stored generation for a video.
pub fn upsert_generation(
    conn: &mut SqliteConnection,
    video_id: i32,
    transcript: &str,
    content: &GeneratedContent,
    provider: &str,
    model: &str,
) -> Result<AiGeneration, String> {
    let record = NewAiGeneration {
        video_id,
        transcript: Some(transcript.to_string()),
        thumbnail_text: Some(serde_json::to_string(&content.thumbnail_texts).unwrap_or_else(|_| "[]".into())),
        instagram_description: Some(content.instagram_description.clone()),
        tiktok_description: Some(content.tiktok_description.clone()),
        youtube_short_title: Some(content.youtube_short_title.clone()),
        youtube_short_description: Some(content.youtube_short_description.clone()),
        youtube_short_tags: Some(serde_json::to_string(&content.youtube_short_tags).unwrap_or_else(|_| "[]".into())),
        hashtags: Some(serde_json::to_string(&content.hashtags).unwrap_or_else(|_| "[]".into())),
        provider: Some(provider.to_string()),
        model: Some(model.to_string()),
        generated_at: Utc::now().naive_utc(),
    };

    let existing = get_generation(conn, video_id);
    let result = if existing.is_some() {
        diesel::update(ai_generations::table.filter(ai_generations::video_id.eq(video_id)))
            .set(&record)
            .execute(conn)
    } else {
        diesel::insert_into(ai_generations::table)
            .values(&record)
            .execute(conn)
    };

    if let Err(e) = result {
        error!("Failed to persist AI generation for video {}: {}", video_id, e);
        return Err(format!("Failed to save generation: {}", e));
    }

    get_generation(conn, video_id).ok_or_else(|| "Failed to reload saved generation".to_string())
}
