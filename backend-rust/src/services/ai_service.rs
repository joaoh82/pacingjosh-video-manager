//! AI content generation: audio transcription and SEO copy generation for
//! portrait/shorts videos, plus the LLM planning step for the video-edit
//! pipeline. Transcription providers: ElevenLabs, OpenAI, Gemini. Text/LLM
//! providers: Gemini, OpenAI, Anthropic. API keys and model selections come
//! from `AiSettings` (persisted in config.json).

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
const ELEVENLABS_BASE: &str = "https://api.elevenlabs.io/v1";

/// A single transcribed word with its start/end offset (seconds) in the source.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct TranscriptWord {
    pub text: String,
    pub start: f32,
    pub end: f32,
}

/// A transcript with optional word-level timing. `words` is empty when the
/// provider does not return timestamps (e.g. Gemini).
#[derive(Debug, Clone, serde::Serialize, Default)]
pub struct TimedTranscript {
    pub text: String,
    pub words: Vec<TranscriptWord>,
}

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
    Ok(transcribe_timed(audio_path, ai).await?.text)
}

/// Transcribe an audio file, returning word-level timestamps when the provider
/// supports them. ElevenLabs (Scribe) and OpenAI return word timings; Gemini
/// returns plain text only (the `words` vec is left empty).
pub async fn transcribe_timed(audio_path: &Path, ai: &AiSettings) -> Result<TimedTranscript, String> {
    let bytes = std::fs::read(audio_path).map_err(|e| format!("Failed to read audio: {}", e))?;

    match ai.transcription_provider.as_str() {
        "elevenlabs" => transcribe_elevenlabs(bytes, ai).await,
        "openai" => transcribe_openai(bytes, ai).await,
        "gemini" => transcribe_gemini(bytes, ai).await.map(|text| TimedTranscript { text, words: vec![] }),
        other => Err(format!("Unsupported transcription provider: {}", other)),
    }
}

/// ElevenLabs Scribe speech-to-text. Returns the full transcript plus
/// word-level timestamps. See https://elevenlabs.io/docs/api-reference/speech-to-text/convert
async fn transcribe_elevenlabs(bytes: Vec<u8>, ai: &AiSettings) -> Result<TimedTranscript, String> {
    let key = ai
        .elevenlabs_api_key
        .as_deref()
        .filter(|k| !k.is_empty())
        .ok_or("ElevenLabs API key is not configured")?;

    // Default to the current Scribe model when none is configured.
    let model = if ai.transcription_model.trim().is_empty() {
        "scribe_v1"
    } else {
        ai.transcription_model.as_str()
    };

    let part = reqwest::multipart::Part::bytes(bytes)
        .file_name("audio.mp3")
        .mime_str("audio/mpeg")
        .map_err(|e| e.to_string())?;
    let form = reqwest::multipart::Form::new()
        .text("model_id", model.to_string())
        .text("timestamps_granularity", "word")
        .part("file", part);

    let resp = client()
        .post(format!("{}/speech-to-text", ELEVENLABS_BASE))
        .header("xi-api-key", key)
        .multipart(form)
        .send()
        .await
        .map_err(|e| format!("ElevenLabs request failed: {}", e))?;

    let status = resp.status();
    let text = resp.text().await.unwrap_or_default();
    if !status.is_success() {
        return Err(format!("ElevenLabs transcription error ({}): {}", status, text));
    }

    let parsed: serde_json::Value =
        serde_json::from_str(&text).map_err(|e| format!("Bad ElevenLabs response: {}", e))?;

    let full_text = parsed["text"].as_str().unwrap_or_default().trim().to_string();

    // Keep only spoken words (drop "spacing" and "audio_event" entries).
    let words: Vec<TranscriptWord> = parsed["words"]
        .as_array()
        .map(|arr| {
            arr.iter()
                .filter(|w| w["type"].as_str().unwrap_or("word") == "word")
                .filter_map(|w| {
                    Some(TranscriptWord {
                        text: w["text"].as_str()?.to_string(),
                        start: w["start"].as_f64()? as f32,
                        end: w["end"].as_f64()? as f32,
                    })
                })
                .collect()
        })
        .unwrap_or_default();

    if full_text.is_empty() && words.is_empty() {
        return Err("ElevenLabs returned an empty transcript".to_string());
    }

    Ok(TimedTranscript { text: full_text, words })
}

async fn transcribe_openai(bytes: Vec<u8>, ai: &AiSettings) -> Result<TimedTranscript, String> {
    let key = ai
        .openai_api_key
        .as_deref()
        .filter(|k| !k.is_empty())
        .ok_or("OpenAI API key is not configured")?;

    let part = reqwest::multipart::Part::bytes(bytes)
        .file_name("audio.mp3")
        .mime_str("audio/mpeg")
        .map_err(|e| e.to_string())?;
    // Ask for verbose JSON with word-level timestamps so the edit pipeline can
    // choose precise cut points. (Whisper-class models support this; gpt-4o
    // transcribe models ignore the extra fields and still return `text`.)
    let form = reqwest::multipart::Form::new()
        .text("model", ai.transcription_model.clone())
        .text("response_format", "verbose_json")
        .text("timestamp_granularities[]", "word")
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

    let full_text = parsed["text"].as_str().unwrap_or_default().trim().to_string();
    let words: Vec<TranscriptWord> = parsed["words"]
        .as_array()
        .map(|arr| {
            arr.iter()
                .filter_map(|w| {
                    Some(TranscriptWord {
                        text: w["word"].as_str()?.to_string(),
                        start: w["start"].as_f64()? as f32,
                        end: w["end"].as_f64()? as f32,
                    })
                })
                .collect()
        })
        .unwrap_or_default();

    if full_text.is_empty() && words.is_empty() {
        return Err("OpenAI returned an empty transcript".to_string());
    }

    Ok(TimedTranscript { text: full_text, words })
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
    let raw = complete(&prompt, ai, 1024).await?;

    let json = extract_json(&raw);
    let mut content: GeneratedContent =
        serde_json::from_str(json).map_err(|e| format!("Failed to parse model JSON: {} — raw: {}", e, raw))?;

    // Enforce the array-size contracts regardless of model behavior.
    content.hashtags.truncate(5);
    content.youtube_short_tags.truncate(15);
    Ok(content)
}

/// YouTube copy for a long-form video: title options, description, tags, and
/// thumbnail text ideas.
#[derive(Debug, Deserialize, serde::Serialize, Default, Clone)]
pub struct YoutubeCopy {
    #[serde(default)]
    pub titles: Vec<String>,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default)]
    pub thumbnail_texts: Vec<String>,
}

/// Generate long-form YouTube copy (3 SEO titles, a description, tags, and
/// thumbnail text ideas) from the final video's transcript.
pub async fn generate_youtube_copy(transcript: &str, ai: &AiSettings) -> Result<YoutubeCopy, String> {
    let prompt = format!(
        "You are a YouTube growth & SEO copywriter for LONG-FORM videos. Based ONLY on the \
following video transcript, write copy that maximizes click-through and watch time while staying \
accurate to the content.\n\n\
Return STRICT JSON (no markdown, no commentary) with exactly these keys:\n\
- \"titles\": array of 3 distinct, SEO-optimized YouTube title options (each at most ~70 \
characters; compelling but not misleading)\n\
- \"description\": a YouTube description — a strong first line/hook, then a short keyword-rich \
summary paragraph, then a few relevant hashtags on the last line\n\
- \"tags\": array of UP TO 20 SEO keyword tags (plain keywords/phrases, no leading #)\n\
- \"thumbnail_texts\": array of 3 short, punchy on-screen thumbnail text ideas (max ~5 words each)\n\n\
Do not invent facts that aren't supported by the transcript.\n\n\
TRANSCRIPT:\n\"\"\"\n{}\n\"\"\"",
        transcript
    );

    let raw = complete(&prompt, ai, 2048).await?;
    let json = extract_json(&raw);
    let mut copy: YoutubeCopy = serde_json::from_str(json)
        .map_err(|e| format!("Failed to parse copy JSON: {} — raw: {}", e, raw))?;

    copy.titles.truncate(3);
    copy.thumbnail_texts.truncate(5);
    copy.tags.truncate(25);
    if copy.titles.is_empty() && copy.description.trim().is_empty() {
        return Err("The model returned empty copy.".to_string());
    }
    Ok(copy)
}

/// Run a single JSON-mode completion against the configured text provider and
/// return the raw response text. Shared by social-copy generation and the
/// video-edit planning step.
pub async fn complete(prompt: &str, ai: &AiSettings, max_tokens: u32) -> Result<String, String> {
    match ai.text_provider.as_str() {
        "gemini" => generate_gemini(prompt, ai).await,
        "openai" => generate_openai(prompt, ai).await,
        "anthropic" => generate_anthropic(prompt, ai, max_tokens).await,
        other => Err(format!("Unsupported text provider: {}", other)),
    }
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

async fn generate_anthropic(prompt: &str, ai: &AiSettings, max_tokens: u32) -> Result<String, String> {
    let key = ai
        .anthropic_api_key
        .as_deref()
        .filter(|k| !k.is_empty())
        .ok_or("Anthropic API key is not configured")?;

    let body = serde_json::json!({
        "model": ai.text_model,
        "max_tokens": max_tokens,
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
