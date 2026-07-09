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

/// Copy for a finished cut. For long-form videos only the YouTube fields are
/// filled; short-form cuts also carry Instagram/TikTok captions and hashtags
/// (the extra fields stay `None`/empty on long-form, and are skipped when
/// serializing, so persisted long-form `copy_json` is unchanged).
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
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub instagram_description: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tiktok_description: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub hashtags: Vec<String>,
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

/// Generate short-form copy (Shorts title options/description/tags plus
/// Instagram & TikTok captions and hashtags) from the final cut's transcript.
/// Same return shape as [`generate_youtube_copy`] so the edit row's `copy_json`
/// and the copy panel handle both.
pub async fn generate_short_copy(transcript: &str, ai: &AiSettings) -> Result<YoutubeCopy, String> {
    let prompt = format!(
        "You are a social media copywriter for SHORT-FORM vertical videos (YouTube Shorts, \
Instagram Reels, TikTok). Based ONLY on the following video transcript, produce engaging, \
SEO-optimized copy.\n\n\
Return STRICT JSON (no markdown, no commentary) with exactly these keys:\n\
- \"titles\": array of 3 punchy, SEO-optimized YouTube Shorts title options (each at most ~70 \
characters, no hashtags)\n\
- \"description\": a YouTube Shorts description with a strong hook and SEO keywords\n\
- \"tags\": array of UP TO 15 SEO keyword tags for YouTube (plain keywords, no leading #)\n\
- \"thumbnail_texts\": array of 3 short on-screen hook text ideas (max ~6 words each)\n\
- \"instagram_description\": an Instagram Reels caption with a strong hook and SEO keywords\n\
- \"tiktok_description\": a TikTok caption optimized for discovery\n\
- \"hashtags\": array of UP TO 5 relevant, high-traffic hashtags (each starting with #)\n\n\
Keep captions concise and native to each platform. Do not invent facts not in the transcript.\n\n\
TRANSCRIPT:\n\"\"\"\n{}\n\"\"\"",
        transcript
    );

    let raw = complete(&prompt, ai, 2048).await?;
    let json = extract_json(&raw);
    let mut copy: YoutubeCopy = serde_json::from_str(json)
        .map_err(|e| format!("Failed to parse copy JSON: {} — raw: {}", e, raw))?;

    copy.titles.truncate(3);
    copy.thumbnail_texts.truncate(3);
    copy.tags.truncate(15);
    copy.hashtags.truncate(5);
    if copy.titles.is_empty() && copy.description.trim().is_empty() {
        return Err("The model returned empty copy.".to_string());
    }
    Ok(copy)
}

/// Restyle a thumbnail frame with the configured image provider/model: send the
/// still + a style prompt, get back an edited image (keeping the subject).
/// Returns image bytes. (Text is added as a real overlay later, not by the model,
/// so it stays accurate.)
pub async fn restyle_image(image_jpeg: &[u8], prompt: &str, ai: &AiSettings) -> Result<Vec<u8>, String> {
    match ai.image_provider.as_str() {
        "gemini" => restyle_gemini(image_jpeg, prompt, ai).await,
        "openai" => restyle_openai(image_jpeg, prompt, ai).await,
        other => Err(format!("Unsupported image provider: {}", other)),
    }
}

/// Restyle via Google Gemini's image model (e.g. gemini-2.5-flash-image).
async fn restyle_gemini(image_jpeg: &[u8], prompt: &str, ai: &AiSettings) -> Result<Vec<u8>, String> {
    let key = ai
        .gemini_api_key
        .as_deref()
        .filter(|k| !k.is_empty())
        .ok_or("A Gemini API key is required for AI restyle (add it under Settings → AI / LLM).")?;

    let body = serde_json::json!({
        "contents": [{
            "parts": [
                { "inline_data": { "mime_type": "image/jpeg", "data": STANDARD.encode(image_jpeg) } },
                { "text": prompt }
            ]
        }],
        "generationConfig": { "responseModalities": ["TEXT", "IMAGE"] }
    });

    let url = format!("{}/{}:generateContent?key={}", GEMINI_BASE, ai.image_model, key);
    let resp = client()
        .post(url)
        .json(&body)
        .send()
        .await
        .map_err(|e| format!("Gemini request failed: {}", e))?;

    let status = resp.status();
    let text = resp.text().await.unwrap_or_default();
    if !status.is_success() {
        return Err(format!("Gemini image error ({}): {}", status, truncate_err(&text)));
    }

    let parsed: serde_json::Value =
        serde_json::from_str(&text).map_err(|e| format!("Bad Gemini response: {}", e))?;

    if let Some(parts) = parsed["candidates"][0]["content"]["parts"].as_array() {
        for p in parts {
            let data = p["inline_data"]["data"]
                .as_str()
                .or_else(|| p["inlineData"]["data"].as_str());
            if let Some(b64) = data {
                return STANDARD
                    .decode(b64)
                    .map_err(|e| format!("Failed to decode generated image: {}", e));
            }
        }
    }

    // No image came back — surface why, as clearly as the API allows. A STOP
    // finish with a text part is the model replying in words (often a soft
    // refusal); include that text so the reason is visible.
    let mut returned_text = String::new();
    if let Some(parts) = parsed["candidates"][0]["content"]["parts"].as_array() {
        for p in parts {
            if let Some(t) = p["text"].as_str() {
                if !returned_text.is_empty() {
                    returned_text.push(' ');
                }
                returned_text.push_str(t.trim());
            }
        }
    }
    let finish = parsed["candidates"][0]["finishReason"].as_str().unwrap_or("");
    let detail = parsed["candidates"][0]["finishMessage"]
        .as_str()
        .or_else(|| parsed["promptFeedback"]["blockReason"].as_str())
        .filter(|s| !s.is_empty())
        .unwrap_or(returned_text.as_str());

    let mut err = String::from("Gemini didn't return an image");
    if !finish.is_empty() {
        err.push_str(&format!(" ({})", finish));
    }
    if !detail.trim().is_empty() {
        err.push_str(&format!(": {}", truncate_err(detail.trim())));
    }
    err.push_str(
        ". This usually means the model declined to edit a real, identifiable face, or the \
selected model isn't an image model. Try: (1) OpenAI gpt-image-2 in Settings (often edits \
photos more permissively); (2) a wider frame where the face isn't the focus; or (3) keep AI \
off and use the real frame + text overlay.",
    );
    Err(err)
}

/// Restyle via OpenAI's image edit endpoint (e.g. gpt-image-1 / gpt-image-2).
async fn restyle_openai(image_jpeg: &[u8], prompt: &str, ai: &AiSettings) -> Result<Vec<u8>, String> {
    let key = ai
        .openai_api_key
        .as_deref()
        .filter(|k| !k.is_empty())
        .ok_or("An OpenAI API key is required for AI restyle (add it under Settings → AI / LLM).")?;

    let part = reqwest::multipart::Part::bytes(image_jpeg.to_vec())
        .file_name("frame.jpg")
        .mime_str("image/jpeg")
        .map_err(|e| e.to_string())?;
    let form = reqwest::multipart::Form::new()
        .text("model", ai.image_model.clone())
        .text("prompt", prompt.to_string())
        .text("size", "1536x1024")
        .part("image[]", part);

    let resp = client()
        .post(format!("{}/images/edits", OPENAI_BASE))
        .bearer_auth(key)
        .multipart(form)
        .send()
        .await
        .map_err(|e| format!("OpenAI request failed: {}", e))?;

    let status = resp.status();
    let text = resp.text().await.unwrap_or_default();
    if !status.is_success() {
        return Err(format!("OpenAI image error ({}): {}", status, truncate_err(&text)));
    }

    let parsed: serde_json::Value =
        serde_json::from_str(&text).map_err(|e| format!("Bad OpenAI response: {}", e))?;
    if let Some(b64) = parsed["data"][0]["b64_json"].as_str() {
        return STANDARD
            .decode(b64)
            .map_err(|e| format!("Failed to decode generated image: {}", e));
    }
    Err(format!("OpenAI did not return an image: {}", truncate_err(&text)))
}

fn truncate_err(s: &str) -> String {
    if s.len() <= 500 {
        s.to_string()
    } else {
        format!("{}…", &s[..500])
    }
}

// --- AI thumbnail text styling ----------------------------------------------

/// Ask the text LLM to design an eye-catching text treatment (colors, gradient,
/// outline, shadow, optional highlight band) for a thumbnail caption. The text
/// itself stays a real canvas overlay — only the *style* is generated, so the
/// words remain crisp and accurate. Returns a normalized, clamped style object
/// (camelCase) that drops straight into the editor's renderer.
pub async fn generate_text_style(
    text: &str,
    context: &str,
    user_prompt: &str,
    ai: &AiSettings,
) -> Result<serde_json::Value, String> {
    let prompt = build_text_style_prompt(text, context, user_prompt);
    let raw = complete(&prompt, ai, 700).await?;
    parse_text_style(&raw)
}

fn build_text_style_prompt(text: &str, context: &str, user_prompt: &str) -> String {
    let extra = user_prompt.trim();
    let extra_line = if extra.is_empty() {
        "none".to_string()
    } else {
        extra.to_string()
    };
    let topic = if context.trim().is_empty() {
        "unknown"
    } else {
        context.trim()
    };
    format!(
        r##"You are a senior YouTube thumbnail typographer. Design a bold, high-contrast TEXT
treatment that maximizes click-through for the caption below. Avoid plain
white-with-a-black-outline unless it is genuinely the strongest choice — prefer
punchy color, an optional top-to-bottom gradient, a soft drop shadow for
legibility, and (when it fits) a colored highlight band behind the words.

Return ONLY a JSON object with EXACTLY these keys (no markdown, no commentary):
{{
  "fill": "#RRGGBB",                       // solid text color (used when gradient is null)
  "gradient": null | {{ "from": "#RRGGBB", "to": "#RRGGBB" }},  // top->bottom gradient, or null
  "outlineColor": "#RRGGBB",
  "outlineWidth": 0-32,                     // stroke thickness in px; 0 = no outline
  "shadowColor": "#RRGGBB",
  "shadowBlur": 0-48,
  "shadowOffsetY": 0-24,
  "highlight": null | {{ "color": "#RRGGBB", "textColor": "#RRGGBB" }}  // band behind text, or null
}}

Rules:
- Every color is a #RRGGBB hex string.
- Ensure strong contrast between the text and whatever sits behind it (band or video frame).
- When you set a highlight band, pick a "textColor" that pops against the band "color".
- Keep it legible at small sizes. Do not add any keys beyond those listed.

CAPTION: "{text}"
VIDEO TOPIC: "{topic}"
EXTRA DIRECTION: "{extra}""##,
        text = text.trim(),
        topic = topic,
        extra = extra_line,
    )
}

/// Parse + normalize the model's style JSON into the exact camelCase shape the
/// canvas renderer expects, clamping numbers and validating hex colors so a
/// loose model response can never produce an unrenderable style.
fn parse_text_style(raw: &str) -> Result<serde_json::Value, String> {
    let json = extract_json_object(raw)
        .ok_or("The model did not return a JSON text style.".to_string())?;
    Ok(build_style(&json))
}

/// Extract the first balanced top-level `{...}` object from a model response
/// (tolerates ```json fences and surrounding prose).
fn extract_json_object(raw: &str) -> Option<serde_json::Value> {
    let start = raw.find('{')?;
    let end = raw.rfind('}')?;
    if end <= start {
        return None;
    }
    serde_json::from_str(&raw[start..=end]).ok()
}

fn pick<'a>(v: &'a serde_json::Value, keys: &[&str]) -> Option<&'a serde_json::Value> {
    keys.iter()
        .filter_map(|k| v.get(*k))
        .find(|x| !x.is_null())
}

/// Validate a `#RRGGBB`/`#RGB` hex (case-insensitive), expanding shorthand and
/// falling back to `default` for anything malformed.
fn norm_hex(v: Option<&serde_json::Value>, default: &str) -> String {
    let raw = v.and_then(|x| x.as_str()).unwrap_or("").trim();
    let body = raw.strip_prefix('#').unwrap_or(raw);
    if !body.chars().all(|c| c.is_ascii_hexdigit()) {
        return default.to_string();
    }
    match body.len() {
        6 => format!("#{}", body.to_lowercase()),
        3 => {
            let mut out = String::from("#");
            for c in body.chars() {
                out.push(c.to_ascii_lowercase());
                out.push(c.to_ascii_lowercase());
            }
            out
        }
        _ => default.to_string(),
    }
}

fn norm_num(v: Option<&serde_json::Value>, default: f32, min: f32, max: f32) -> f32 {
    v.and_then(|x| x.as_f64())
        .map(|x| x as f32)
        .unwrap_or(default)
        .clamp(min, max)
}

fn build_style(json: &serde_json::Value) -> serde_json::Value {
    let gradient = pick(json, &["gradient"]).and_then(|g| {
        if g.is_null() {
            return None;
        }
        let from = pick(g, &["from", "start", "top"]);
        let to = pick(g, &["to", "end", "bottom"]);
        if from.is_none() && to.is_none() {
            return None;
        }
        Some(serde_json::json!({
            "from": norm_hex(from, "#ffd200"),
            "to": norm_hex(to, "#ff6a00"),
        }))
    });

    let highlight = pick(json, &["highlight", "band", "background"]).and_then(|h| {
        if h.is_null() {
            return None;
        }
        let color = pick(h, &["color", "bg", "background", "fill"]);
        color?;
        Some(serde_json::json!({
            "color": norm_hex(color, "#e11d2a"),
            "textColor": norm_hex(pick(h, &["textColor", "text_color"]), "#ffffff"),
        }))
    });

    serde_json::json!({
        "fill": norm_hex(pick(json, &["fill", "color", "textColor", "text_color"]), "#ffffff"),
        "gradient": gradient,
        "outlineColor": norm_hex(pick(json, &["outlineColor", "outline_color", "stroke", "edgeColor"]), "#000000"),
        "outlineWidth": norm_num(pick(json, &["outlineWidth", "outline_width", "strokeWidth"]), 10.0, 0.0, 32.0),
        "shadowColor": norm_hex(pick(json, &["shadowColor", "shadow_color"]), "#000000"),
        "shadowBlur": norm_num(pick(json, &["shadowBlur", "shadow_blur"]), 0.0, 0.0, 48.0),
        "shadowOffsetY": norm_num(pick(json, &["shadowOffsetY", "shadow_offset_y", "shadowOffset"]), 0.0, 0.0, 24.0),
        "highlight": highlight,
    })
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

// --- Embeddings --------------------------------------------------------------

/// Embed a batch of texts using the configured embedding provider, returning one
/// vector per input in the same order. Used by semantic search to index the
/// video/production library and to embed a query. Reuses the provider's stored
/// API key (OpenAI or Gemini).
pub async fn embed_texts(texts: &[String], ai: &AiSettings) -> Result<Vec<Vec<f32>>, String> {
    if texts.is_empty() {
        return Ok(Vec::new());
    }
    match ai.embedding_provider.as_str() {
        "openai" => embed_openai(texts, ai).await,
        "gemini" => embed_gemini(texts, ai).await,
        other => Err(format!("Unsupported embedding provider: {}", other)),
    }
}

/// OpenAI embeddings (`/v1/embeddings`). A single request accepts an array of
/// inputs and returns one vector per input; results are re-sorted by the `index`
/// field so ordering matches the input regardless of response order.
async fn embed_openai(texts: &[String], ai: &AiSettings) -> Result<Vec<Vec<f32>>, String> {
    let key = ai
        .openai_api_key
        .as_deref()
        .filter(|k| !k.is_empty())
        .ok_or("An OpenAI API key is required for semantic search (add it under Settings → AI / LLM).")?;

    let body = serde_json::json!({
        "model": ai.embedding_model,
        "input": texts,
    });

    let resp = client()
        .post(format!("{}/embeddings", OPENAI_BASE))
        .bearer_auth(key)
        .json(&body)
        .send()
        .await
        .map_err(|e| format!("OpenAI request failed: {}", e))?;

    let status = resp.status();
    let text = resp.text().await.unwrap_or_default();
    if !status.is_success() {
        return Err(format!("OpenAI embeddings error ({}): {}", status, truncate_err(&text)));
    }

    let parsed: serde_json::Value =
        serde_json::from_str(&text).map_err(|e| format!("Bad OpenAI response: {}", e))?;
    let data = parsed["data"]
        .as_array()
        .ok_or("OpenAI embeddings response missing `data`")?;

    // Collect (index, vector) then sort by index so order matches the input.
    let mut indexed: Vec<(usize, Vec<f32>)> = Vec::with_capacity(data.len());
    for item in data {
        let idx = item["index"].as_u64().unwrap_or(indexed.len() as u64) as usize;
        let vec = json_to_f32_vec(&item["embedding"])
            .ok_or("OpenAI embedding item had no numeric vector")?;
        indexed.push((idx, vec));
    }
    indexed.sort_by_key(|(i, _)| *i);
    Ok(indexed.into_iter().map(|(_, v)| v).collect())
}

/// Gemini embeddings via `:batchEmbedContents`. Each request carries the model
/// (prefixed with `models/` when the id omits it) and the text part; the
/// response's `embeddings[].values` arrays come back in request order.
async fn embed_gemini(texts: &[String], ai: &AiSettings) -> Result<Vec<Vec<f32>>, String> {
    let key = ai
        .gemini_api_key
        .as_deref()
        .filter(|k| !k.is_empty())
        .ok_or("A Gemini API key is required for semantic search (add it under Settings → AI / LLM).")?;

    let model_path = if ai.embedding_model.starts_with("models/") {
        ai.embedding_model.clone()
    } else {
        format!("models/{}", ai.embedding_model)
    };

    let requests: Vec<serde_json::Value> = texts
        .iter()
        .map(|t| {
            serde_json::json!({
                "model": model_path,
                "content": { "parts": [{ "text": t }] },
            })
        })
        .collect();
    let body = serde_json::json!({ "requests": requests });

    let url = format!("{}/{}:batchEmbedContents?key={}", GEMINI_BASE, model_path.trim_start_matches("models/"), key);
    let resp = client()
        .post(url)
        .json(&body)
        .send()
        .await
        .map_err(|e| format!("Gemini request failed: {}", e))?;

    let status = resp.status();
    let text = resp.text().await.unwrap_or_default();
    if !status.is_success() {
        return Err(format!("Gemini embeddings error ({}): {}", status, truncate_err(&text)));
    }

    let parsed: serde_json::Value =
        serde_json::from_str(&text).map_err(|e| format!("Bad Gemini response: {}", e))?;
    let embeddings = parsed["embeddings"]
        .as_array()
        .ok_or("Gemini embeddings response missing `embeddings`")?;

    let mut out = Vec::with_capacity(embeddings.len());
    for e in embeddings {
        let vec = json_to_f32_vec(&e["values"])
            .ok_or("Gemini embedding item had no numeric vector")?;
        out.push(vec);
    }
    Ok(out)
}

/// Parse a JSON array of numbers into `Vec<f32>`. Returns `None` if the value is
/// not an array or contains a non-numeric entry.
fn json_to_f32_vec(v: &serde_json::Value) -> Option<Vec<f32>> {
    let arr = v.as_array()?;
    let mut out = Vec::with_capacity(arr.len());
    for n in arr {
        out.push(n.as_f64()? as f32);
    }
    Some(out)
}

// --- Visual description (vision captioning) ----------------------------------

const VISUAL_PROMPT: &str = "You are analyzing a few still frames sampled from a SINGLE short video. \
Based ONLY on what is visible, describe what the video shows. Return STRICT JSON (no markdown, no \
commentary) with EXACTLY these keys:\n\
{\n\
  \"summary\": \"1-2 sentences: the setting, the subject(s), and the main activity or action\",\n\
  \"tags\": [\"5-12 short lowercase visual keywords: places, objects, actions, weather, time of day\"]\n\
}\n\
Describe only what is visible. Do not guess audio, names, or anything not shown.";

/// Describe what a video shows from a few sampled thumbnail frames, using the
/// configured TEXT/LLM provider (Gemini/OpenAI/Anthropic — all multimodal).
/// Returns a compact "summary + tags" string ready to embed for semantic search.
pub async fn describe_video_frames(frames: &[Vec<u8>], ai: &AiSettings) -> Result<String, String> {
    if frames.is_empty() {
        return Err("No frames to describe".to_string());
    }
    let raw = match ai.text_provider.as_str() {
        "gemini" => describe_gemini(frames, ai).await,
        "openai" => describe_openai(frames, ai).await,
        "anthropic" => describe_anthropic(frames, ai).await,
        other => Err(format!("Unsupported provider for vision: {}", other)),
    }?;
    let out = format_visual(&raw);
    if out.trim().is_empty() {
        return Err("The model returned an empty visual description".to_string());
    }
    Ok(out)
}

/// Flatten the model's `{summary, tags}` JSON into a single embeddable line.
/// Falls back to the raw text when the response isn't the expected JSON.
fn format_visual(raw: &str) -> String {
    let json = extract_json(raw);
    if let Ok(v) = serde_json::from_str::<serde_json::Value>(json) {
        let summary = v["summary"].as_str().unwrap_or("").trim().to_string();
        let tags: Vec<String> = v["tags"]
            .as_array()
            .map(|a| {
                a.iter()
                    .filter_map(|t| t.as_str())
                    .map(|s| s.trim().to_string())
                    .filter(|s| !s.is_empty())
                    .collect()
            })
            .unwrap_or_default();
        let mut out = summary;
        if !tags.is_empty() {
            if !out.is_empty() {
                out.push(' ');
            }
            out.push_str(&format!("Tags: {}", tags.join(", ")));
        }
        if !out.trim().is_empty() {
            return out;
        }
    }
    raw.trim().to_string()
}

async fn describe_gemini(frames: &[Vec<u8>], ai: &AiSettings) -> Result<String, String> {
    let key = ai
        .gemini_api_key
        .as_deref()
        .filter(|k| !k.is_empty())
        .ok_or("A Gemini API key is required for visual descriptions.")?;

    let mut parts: Vec<serde_json::Value> = frames
        .iter()
        .map(|f| serde_json::json!({ "inline_data": { "mime_type": "image/jpeg", "data": STANDARD.encode(f) } }))
        .collect();
    parts.push(serde_json::json!({ "text": VISUAL_PROMPT }));

    let body = serde_json::json!({
        "contents": [{ "parts": parts }],
        "generationConfig": { "responseMimeType": "application/json" }
    });

    let url = format!("{}/{}:generateContent?key={}", GEMINI_BASE, ai.text_model, key);
    let resp = client().post(url).json(&body).send().await.map_err(|e| format!("Gemini request failed: {}", e))?;
    let status = resp.status();
    let text = resp.text().await.unwrap_or_default();
    if !status.is_success() {
        return Err(format!("Gemini vision error ({}): {}", status, truncate_err(&text)));
    }
    gemini_extract_text(&text)
}

async fn describe_openai(frames: &[Vec<u8>], ai: &AiSettings) -> Result<String, String> {
    let key = ai
        .openai_api_key
        .as_deref()
        .filter(|k| !k.is_empty())
        .ok_or("An OpenAI API key is required for visual descriptions.")?;

    let mut content: Vec<serde_json::Value> = frames
        .iter()
        .map(|f| {
            serde_json::json!({
                "type": "image_url",
                "image_url": { "url": format!("data:image/jpeg;base64,{}", STANDARD.encode(f)) }
            })
        })
        .collect();
    content.push(serde_json::json!({ "type": "text", "text": VISUAL_PROMPT }));

    let body = serde_json::json!({
        "model": ai.text_model,
        "messages": [{ "role": "user", "content": content }],
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
        return Err(format!("OpenAI vision error ({}): {}", status, truncate_err(&text)));
    }
    let parsed: serde_json::Value =
        serde_json::from_str(&text).map_err(|e| format!("Bad OpenAI response: {}", e))?;
    parsed["choices"][0]["message"]["content"]
        .as_str()
        .map(|s| s.to_string())
        .ok_or_else(|| "OpenAI returned no content".to_string())
}

async fn describe_anthropic(frames: &[Vec<u8>], ai: &AiSettings) -> Result<String, String> {
    let key = ai
        .anthropic_api_key
        .as_deref()
        .filter(|k| !k.is_empty())
        .ok_or("An Anthropic API key is required for visual descriptions.")?;

    let mut content: Vec<serde_json::Value> = frames
        .iter()
        .map(|f| {
            serde_json::json!({
                "type": "image",
                "source": { "type": "base64", "media_type": "image/jpeg", "data": STANDARD.encode(f) }
            })
        })
        .collect();
    content.push(serde_json::json!({ "type": "text", "text": VISUAL_PROMPT }));

    let body = serde_json::json!({
        "model": ai.text_model,
        "max_tokens": 512,
        "messages": [{ "role": "user", "content": content }]
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
        return Err(format!("Anthropic vision error ({}): {}", status, truncate_err(&text)));
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
    let existing = get_generation(conn, video_id);
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
        // Preserve any existing visual description — generating copy must not wipe it.
        visual_description: existing.as_ref().and_then(|e| e.visual_description.clone()),
    };

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

/// Insert or update ONLY the transcript for a video, leaving any existing social
/// copy untouched. Used by the semantic re-index's "transcribe missing" step to
/// give un-transcribed clips searchable text without generating copy.
pub fn upsert_transcript(
    conn: &mut SqliteConnection,
    video_id: i32,
    transcript: &str,
    provider: &str,
    model: &str,
) -> Result<(), String> {
    let now = Utc::now().naive_utc();
    let result = if get_generation(conn, video_id).is_some() {
        diesel::update(ai_generations::table.filter(ai_generations::video_id.eq(video_id)))
            .set((
                ai_generations::transcript.eq(Some(transcript.to_string())),
                ai_generations::provider.eq(Some(provider.to_string())),
                ai_generations::model.eq(Some(model.to_string())),
                ai_generations::generated_at.eq(now),
            ))
            .execute(conn)
    } else {
        let record = NewAiGeneration {
            video_id,
            transcript: Some(transcript.to_string()),
            thumbnail_text: None,
            instagram_description: None,
            tiktok_description: None,
            youtube_short_title: None,
            youtube_short_description: None,
            youtube_short_tags: None,
            hashtags: None,
            provider: Some(provider.to_string()),
            model: Some(model.to_string()),
            generated_at: now,
            visual_description: None,
        };
        diesel::insert_into(ai_generations::table).values(&record).execute(conn)
    };

    result.map(|_| ()).map_err(|e| format!("Failed to save transcript: {}", e))
}

/// Insert or update ONLY the visual description for a video (used by the semantic
/// re-index's "describe visuals" step). Leaves the transcript and any social copy
/// untouched.
pub fn upsert_visual_description(
    conn: &mut SqliteConnection,
    video_id: i32,
    description: &str,
) -> Result<(), String> {
    let now = Utc::now().naive_utc();
    let result = if get_generation(conn, video_id).is_some() {
        diesel::update(ai_generations::table.filter(ai_generations::video_id.eq(video_id)))
            .set((
                ai_generations::visual_description.eq(Some(description.to_string())),
                ai_generations::generated_at.eq(now),
            ))
            .execute(conn)
    } else {
        let record = NewAiGeneration {
            video_id,
            transcript: None,
            thumbnail_text: None,
            instagram_description: None,
            tiktok_description: None,
            youtube_short_title: None,
            youtube_short_description: None,
            youtube_short_tags: None,
            hashtags: None,
            provider: None,
            model: None,
            generated_at: now,
            visual_description: Some(description.to_string()),
        };
        diesel::insert_into(ai_generations::table).values(&record).execute(conn)
    };

    result.map(|_| ()).map_err(|e| format!("Failed to save visual description: {}", e))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_text_style_normalizes_and_clamps() {
        let raw = r##"Here you go:
        ```json
        {
          "fill": "FFF",
          "gradient": { "from": "#FFD200", "to": "ff6a00" },
          "outlineColor": "#000",
          "outlineWidth": 999,
          "shadowColor": "#101010",
          "shadowBlur": 30,
          "shadowOffsetY": -5,
          "highlight": { "color": "#E11D2A", "textColor": "#FFFFFF" }
        }
        ```"##;
        let style = parse_text_style(raw).expect("should parse");
        assert_eq!(style["fill"], "#ffffff"); // 3-digit shorthand expanded
        assert_eq!(style["gradient"]["from"], "#ffd200");
        assert_eq!(style["gradient"]["to"], "#ff6a00"); // missing '#' tolerated
        assert_eq!(style["outlineColor"], "#000000");
        assert_eq!(style["outlineWidth"], 32.0); // clamped to max
        assert_eq!(style["shadowOffsetY"], 0.0); // clamped to min
        assert_eq!(style["highlight"]["color"], "#e11d2a");
        assert_eq!(style["highlight"]["textColor"], "#ffffff");
    }

    #[test]
    fn parse_text_style_falls_back_for_bad_values() {
        let raw = r#"{ "fill": "not-a-color", "gradient": null, "highlight": null }"#;
        let style = parse_text_style(raw).expect("should parse");
        assert_eq!(style["fill"], "#ffffff"); // invalid hex -> default
        assert!(style["gradient"].is_null());
        assert!(style["highlight"].is_null());
        assert_eq!(style["outlineColor"], "#000000"); // missing -> default
        assert_eq!(style["outlineWidth"], 10.0); // missing -> default
    }

    #[test]
    fn parse_text_style_rejects_non_json() {
        assert!(parse_text_style("the model refused").is_err());
    }
}
