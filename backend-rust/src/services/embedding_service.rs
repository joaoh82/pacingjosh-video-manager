//! Semantic search over the video/production library.
//!
//! Each video and production is reduced to a plain-text "document" built from
//! the text we already have (filename, metadata, tags, transcript for videos;
//! title, script, take transcripts and copy for productions). Documents are
//! embedded with the configured provider (`ai_service::embed_texts`) and the
//! resulting vectors are cached in `video_embeddings` / `production_embeddings`
//! (a packed little-endian f32 BLOB per row). A search embeds the query and
//! ranks stored vectors by cosine similarity — all locally, no per-query LLM
//! call. Re-indexing is incremental: a document whose `content_hash` and
//! `model` are unchanged is skipped.

use chrono::{NaiveDateTime, Utc};
use diesel::prelude::*;
use diesel::sqlite::SqliteConnection;
use log::{error, info, warn};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use uuid::Uuid;

use crate::config::AiSettings;
use crate::db::DbPool;
use crate::schema::{
    ai_generations, metadata, production_edits, production_embeddings, productions, tags,
    video_embeddings, video_productions, video_tags, videos,
};
use crate::services::{ai_service, ffmpeg_service};

/// Cap each document at this many characters before embedding, keeping requests
/// well under the models' token limits (~8k tokens for OpenAI's small model).
pub const MAX_DOC_CHARS: usize = 8000;

/// How many documents to embed per provider request during re-indexing.
const EMBED_BATCH: usize = 64;

// --- Progress tracking (mirrors scanner::ScanMap) ----------------------------

pub type SearchIndexMap = Arc<Mutex<HashMap<String, ReindexProgress>>>;

/// Live progress for a background re-index run.
#[derive(Debug, Clone, Serialize)]
pub struct ReindexProgress {
    pub job_id: String,
    pub status: String,
    pub stage: String,
    pub total: i64,
    pub processed: i64,
    pub videos_indexed: i64,
    pub videos_skipped: i64,
    pub productions_indexed: i64,
    pub productions_skipped: i64,
    /// Transcription progress for the optional "transcribe missing" pre-pass.
    pub transcribed: i64,
    pub transcribe_total: i64,
    pub transcribe_failed: i64,
    /// Visual-description progress for the optional "describe visuals" pre-pass.
    pub described: i64,
    pub describe_total: i64,
    pub describe_failed: i64,
    pub error: Option<String>,
    pub start_time: NaiveDateTime,
    pub end_time: Option<NaiveDateTime>,
}

impl ReindexProgress {
    fn new(job_id: String) -> Self {
        Self {
            job_id,
            status: "in_progress".to_string(),
            stage: "starting".to_string(),
            total: 0,
            processed: 0,
            videos_indexed: 0,
            videos_skipped: 0,
            productions_indexed: 0,
            productions_skipped: 0,
            transcribed: 0,
            transcribe_total: 0,
            transcribe_failed: 0,
            described: 0,
            describe_total: 0,
            describe_failed: 0,
            error: None,
            start_time: Utc::now().naive_utc(),
            end_time: None,
        }
    }
}

fn update<F: FnOnce(&mut ReindexProgress)>(map: &SearchIndexMap, job_id: &str, f: F) {
    if let Ok(mut m) = map.lock() {
        if let Some(p) = m.get_mut(job_id) {
            f(p);
        }
    }
}

pub fn get_progress(map: &SearchIndexMap, job_id: &str) -> Option<ReindexProgress> {
    map.lock().ok().and_then(|m| m.get(job_id).cloned())
}

// --- Pure helpers (unit-tested) ----------------------------------------------

/// The identity of the embedding space: only vectors produced by the same
/// `provider:model` are comparable, so a provider/model change forces a
/// re-index and query-time comparisons filter on this.
pub fn model_id(ai: &AiSettings) -> String {
    format!("{}:{}", ai.embedding_provider, ai.embedding_model)
}

/// Stable SHA-256 hex of a document, used to skip re-embedding unchanged text.
pub fn content_hash(text: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(text.as_bytes());
    format!("{:x}", hasher.finalize())
}

/// Pack an embedding vector into a little-endian f32 BLOB for storage.
pub fn embedding_to_blob(vec: &[f32]) -> Vec<u8> {
    let mut out = Vec::with_capacity(vec.len() * 4);
    for f in vec {
        out.extend_from_slice(&f.to_le_bytes());
    }
    out
}

/// Decode a little-endian f32 BLOB back into an embedding vector. Trailing bytes
/// that don't form a full f32 are ignored.
pub fn blob_to_embedding(blob: &[u8]) -> Vec<f32> {
    blob.chunks_exact(4)
        .map(|c| f32::from_le_bytes([c[0], c[1], c[2], c[3]]))
        .collect()
}

/// Cosine similarity of two equal-length vectors. Returns 0.0 for a length
/// mismatch or a zero-magnitude vector (so incomparable rows rank last).
pub fn cosine_similarity(a: &[f32], b: &[f32]) -> f32 {
    if a.len() != b.len() || a.is_empty() {
        return 0.0;
    }
    let mut dot = 0.0f32;
    let mut na = 0.0f32;
    let mut nb = 0.0f32;
    for i in 0..a.len() {
        dot += a[i] * b[i];
        na += a[i] * a[i];
        nb += b[i] * b[i];
    }
    if na == 0.0 || nb == 0.0 {
        return 0.0;
    }
    dot / (na.sqrt() * nb.sqrt())
}

/// Truncate to at most `MAX_DOC_CHARS` characters on a char boundary.
fn truncate_doc(text: String) -> String {
    if text.chars().count() <= MAX_DOC_CHARS {
        text
    } else {
        text.chars().take(MAX_DOC_CHARS).collect()
    }
}

/// Build a video's searchable document from all available text. Empty sections
/// are dropped; the result is truncated to the embedding size cap.
#[allow(clippy::too_many_arguments)]
pub fn assemble_video_doc(
    filename: &str,
    category: Option<&str>,
    location: Option<&str>,
    notes: Option<&str>,
    tags: &[String],
    transcript: Option<&str>,
    production_titles: &[String],
    visual: Option<&str>,
) -> String {
    let mut parts: Vec<String> = Vec::new();
    push_labeled(&mut parts, "File", Some(filename));
    push_labeled(&mut parts, "Category", category);
    push_labeled(&mut parts, "Location", location);
    push_labeled(&mut parts, "Notes", notes);
    if !tags.is_empty() {
        push_labeled(&mut parts, "Tags", Some(&tags.join(", ")));
    }
    push_labeled(&mut parts, "Visuals", visual);
    if !production_titles.is_empty() {
        push_labeled(&mut parts, "Productions", Some(&production_titles.join(", ")));
    }
    push_labeled(&mut parts, "Transcript", transcript);
    truncate_doc(parts.join("\n"))
}

/// Build a production's searchable document from its title/platform, its latest
/// edit's script and generated copy, and the transcripts of its takes.
pub fn assemble_production_doc(
    title: &str,
    platform: Option<&str>,
    script: Option<&str>,
    copy: Option<&str>,
    take_transcripts: &[String],
) -> String {
    let mut parts: Vec<String> = Vec::new();
    push_labeled(&mut parts, "Title", Some(title));
    push_labeled(&mut parts, "Platform", platform);
    push_labeled(&mut parts, "Script", script);
    push_labeled(&mut parts, "Copy", copy);
    let joined = take_transcripts
        .iter()
        .filter(|t| !t.trim().is_empty())
        .cloned()
        .collect::<Vec<_>>()
        .join("\n");
    if !joined.trim().is_empty() {
        push_labeled(&mut parts, "Transcript", Some(&joined));
    }
    truncate_doc(parts.join("\n"))
}

fn push_labeled(parts: &mut Vec<String>, label: &str, value: Option<&str>) {
    if let Some(v) = value {
        let v = v.trim();
        if !v.is_empty() {
            parts.push(format!("{}: {}", label, v));
        }
    }
}

fn nonempty(v: Option<&str>) -> bool {
    v.map(|s| !s.trim().is_empty()).unwrap_or(false)
}

/// Whether a filename carries real words (e.g. `morning_run_whistler.mp4`)
/// rather than an opaque camera/device code (`GX011916.MP4`, `IMG_1234.MOV`).
///
/// Bare camera codes are semantically meaningless and embed into one dense,
/// high-similarity cluster that dominates every query, so we only treat a
/// filename as searchable text when it looks descriptive.
pub fn filename_is_descriptive(name: &str) -> bool {
    let stem = name.rsplit_once('.').map(|(s, _)| s).unwrap_or(name);
    let words: Vec<&str> = stem
        .split(|c: char| !c.is_ascii_alphabetic())
        .filter(|w| w.len() >= 3)
        .collect();
    words.len() >= 2 || words.iter().any(|w| w.len() >= 5)
}

// --- Transcript sourcing -----------------------------------------------------

/// One transcribed word — just the text; timings are irrelevant for search.
#[derive(Debug, Deserialize)]
struct WordText {
    text: String,
}

/// Build a `video_id -> plain transcript` map from every source we already have:
/// the per-video AI generation transcript, plus any take transcripts saved by
/// the edit pipeline (`production_edits.transcripts_json`, keyed by video_id).
/// This maximizes recall for "what did I talk about" queries even for takes that
/// were never run through the AI content panel.
fn build_transcript_map(conn: &mut SqliteConnection) -> HashMap<i32, String> {
    let mut map: HashMap<i32, String> = HashMap::new();

    // Primary: clean per-video transcripts from the AI content panel.
    let gen_rows: Vec<(i32, Option<String>)> = ai_generations::table
        .select((ai_generations::video_id, ai_generations::transcript))
        .load(conn)
        .unwrap_or_default();
    for (vid, transcript) in gen_rows {
        if let Some(t) = transcript {
            let t = t.trim();
            if !t.is_empty() {
                map.insert(vid, t.to_string());
            }
        }
    }

    // Enrichment: take transcripts stored by the edit pipeline. Only fills gaps
    // (never overrides a clean AI-panel transcript).
    let edit_rows: Vec<Option<String>> = production_edits::table
        .select(production_edits::transcripts_json)
        .load(conn)
        .unwrap_or_default();
    for json in edit_rows.into_iter().flatten() {
        if let Ok(parsed) = serde_json::from_str::<HashMap<String, Vec<WordText>>>(&json) {
            for (vid_str, words) in parsed {
                if let Ok(vid) = vid_str.parse::<i32>() {
                    if map.contains_key(&vid) {
                        continue;
                    }
                    let text = words
                        .iter()
                        .map(|w| w.text.trim())
                        .filter(|t| !t.is_empty())
                        .collect::<Vec<_>>()
                        .join(" ");
                    if !text.trim().is_empty() {
                        map.insert(vid, text);
                    }
                }
            }
        }
    }

    map
}

/// Build a `video_id -> visual description` map from `ai_generations`.
fn build_visual_map(conn: &mut SqliteConnection) -> HashMap<i32, String> {
    let rows: Vec<(i32, Option<String>)> = ai_generations::table
        .select((ai_generations::video_id, ai_generations::visual_description))
        .load(conn)
        .unwrap_or_default();
    rows.into_iter()
        .filter_map(|(vid, d)| {
            let d = d?;
            let d = d.trim();
            (!d.is_empty()).then(|| (vid, d.to_string()))
        })
        .collect()
}

// --- Document loading --------------------------------------------------------

/// A document to (maybe) embed: the entity id, its text, and the text's hash.
struct Doc {
    id: i32,
    text: String,
    hash: String,
}

/// A video's `(category, location, notes)` metadata fields.
type MetaFields = (Option<String>, Option<String>, Option<String>);

/// Build documents for every video in the library.
fn load_video_docs(
    conn: &mut SqliteConnection,
    transcripts: &HashMap<i32, String>,
    visuals: &HashMap<i32, String>,
) -> Vec<Doc> {
    let base: Vec<(i32, String)> = videos::table
        .select((videos::id, videos::filename))
        .load(conn)
        .unwrap_or_default();

    let meta_rows: Vec<(i32, MetaFields)> = metadata::table
        .select((
            metadata::video_id,
            (metadata::category, metadata::location, metadata::notes),
        ))
        .load(conn)
        .unwrap_or_default();
    let meta: HashMap<i32, MetaFields> = meta_rows.into_iter().collect();

    let tag_rows: Vec<(i32, String)> = video_tags::table
        .inner_join(tags::table)
        .select((video_tags::video_id, tags::name))
        .load(conn)
        .unwrap_or_default();
    let mut tags_by_video: HashMap<i32, Vec<String>> = HashMap::new();
    for (vid, name) in tag_rows {
        tags_by_video.entry(vid).or_default().push(name);
    }

    let prod_rows: Vec<(i32, String)> = video_productions::table
        .inner_join(productions::table)
        .select((video_productions::video_id, productions::title))
        .load(conn)
        .unwrap_or_default();
    let mut prods_by_video: HashMap<i32, Vec<String>> = HashMap::new();
    for (vid, title) in prod_rows {
        prods_by_video.entry(vid).or_default().push(title);
    }

    base.into_iter()
        .filter_map(|(id, filename)| {
            let (cat, loc, notes) = meta
                .get(&id)
                .map(|(c, l, n)| (c.as_deref(), l.as_deref(), n.as_deref()))
                .unwrap_or((None, None, None));
            let empty: Vec<String> = Vec::new();
            let tags = tags_by_video.get(&id).unwrap_or(&empty);
            let transcript = transcripts.get(&id).map(|s| s.as_str());
            let visual = visuals.get(&id).map(|s| s.as_str());
            let prods = prods_by_video.get(&id).unwrap_or(&empty);
            let descriptive = filename_is_descriptive(&filename);

            // Only index videos that have some describable text. A video whose
            // sole "text" is an opaque camera filename carries no meaning and
            // would otherwise pollute every result (see filename_is_descriptive).
            // Production membership alone doesn't qualify (a shared title would
            // just re-create a cluster), but it's included as extra signal below.
            // A vision-LLM visual description also qualifies a clip.
            let has_text = descriptive
                || nonempty(cat)
                || nonempty(loc)
                || nonempty(notes)
                || !tags.is_empty()
                || nonempty(transcript)
                || nonempty(visual);
            if !has_text {
                return None;
            }

            // Embed the filename only when it's descriptive; an opaque code is noise.
            let file_for_doc = if descriptive { filename.as_str() } else { "" };
            let text =
                assemble_video_doc(file_for_doc, cat, loc, notes, tags, transcript, prods, visual);
            let hash = content_hash(&text);
            Some(Doc { id, text, hash })
        })
        .collect()
}

/// Build documents for every production in the library.
fn load_production_docs(conn: &mut SqliteConnection, transcripts: &HashMap<i32, String>) -> Vec<Doc> {
    let base: Vec<(i32, String, Option<String>)> = productions::table
        .select((productions::id, productions::title, productions::platform))
        .load(conn)
        .unwrap_or_default();

    // Latest edit (script + copy) per production, newest first so the first seen
    // wins.
    let edit_rows: Vec<(i32, Option<String>, Option<String>)> = production_edits::table
        .select((
            production_edits::production_id,
            production_edits::script,
            production_edits::copy_json,
        ))
        .order(production_edits::created_at.desc())
        .load(conn)
        .unwrap_or_default();
    let mut latest_edit: HashMap<i32, (Option<String>, Option<String>)> = HashMap::new();
    for (pid, script, copy) in edit_rows {
        latest_edit.entry(pid).or_insert((script, copy));
    }

    // Take video ids per production, to gather their transcripts.
    let take_rows: Vec<(i32, i32)> = video_productions::table
        .select((video_productions::production_id, video_productions::video_id))
        .load(conn)
        .unwrap_or_default();
    let mut takes_by_prod: HashMap<i32, Vec<i32>> = HashMap::new();
    for (pid, vid) in take_rows {
        takes_by_prod.entry(pid).or_default().push(vid);
    }

    base.into_iter()
        .map(|(id, title, platform)| {
            let (script, copy_json) = latest_edit
                .get(&id)
                .map(|(s, c)| (s.clone(), c.clone()))
                .unwrap_or((None, None));
            let copy_text = copy_json.as_deref().map(copy_json_to_text);
            let take_transcripts: Vec<String> = takes_by_prod
                .get(&id)
                .map(|vids| {
                    vids.iter()
                        .filter_map(|v| transcripts.get(v).cloned())
                        .collect()
                })
                .unwrap_or_default();
            let text = assemble_production_doc(
                &title,
                platform.as_deref(),
                script.as_deref(),
                copy_text.as_deref(),
                &take_transcripts,
            );
            let hash = content_hash(&text);
            Doc { id, text, hash }
        })
        .collect()
}

/// Flatten a persisted `copy_json` (titles/description/tags/…) into plain text
/// for embedding. Best-effort: unknown shapes contribute nothing.
fn copy_json_to_text(json: &str) -> String {
    let Ok(v) = serde_json::from_str::<serde_json::Value>(json) else {
        return String::new();
    };
    let mut parts: Vec<String> = Vec::new();
    let mut push_str = |val: &serde_json::Value| {
        if let Some(s) = val.as_str() {
            let s = s.trim();
            if !s.is_empty() {
                parts.push(s.to_string());
            }
        }
    };
    for key in ["description", "instagram_description", "tiktok_description"] {
        push_str(&v[key]);
    }
    for key in ["titles", "tags", "thumbnail_texts", "hashtags"] {
        if let Some(arr) = v[key].as_array() {
            for item in arr {
                push_str(item);
            }
        }
    }
    parts.join(" ")
}

// --- Storage -----------------------------------------------------------------

#[derive(Insertable)]
#[diesel(table_name = video_embeddings)]
struct NewVideoEmbedding {
    video_id: i32,
    content_hash: String,
    model: String,
    dim: i32,
    embedding: Vec<u8>,
    updated_at: NaiveDateTime,
}

#[derive(Insertable)]
#[diesel(table_name = production_embeddings)]
struct NewProductionEmbedding {
    production_id: i32,
    content_hash: String,
    model: String,
    dim: i32,
    embedding: Vec<u8>,
    updated_at: NaiveDateTime,
}

/// Stored `(hash, model)` per entity, to decide which documents changed.
fn stored_video_index(conn: &mut SqliteConnection) -> HashMap<i32, (String, String)> {
    let rows: Vec<(i32, String, String)> = video_embeddings::table
        .select((
            video_embeddings::video_id,
            video_embeddings::content_hash,
            video_embeddings::model,
        ))
        .load(conn)
        .unwrap_or_default();
    rows.into_iter().map(|(id, h, m)| (id, (h, m))).collect()
}

fn stored_production_index(conn: &mut SqliteConnection) -> HashMap<i32, (String, String)> {
    let rows: Vec<(i32, String, String)> = production_embeddings::table
        .select((
            production_embeddings::production_id,
            production_embeddings::content_hash,
            production_embeddings::model,
        ))
        .load(conn)
        .unwrap_or_default();
    rows.into_iter().map(|(id, h, m)| (id, (h, m))).collect()
}

/// Delete cached video embeddings whose video is not in `keep_ids` (i.e. is no
/// longer eligible for indexing). Chunked to stay under SQLite's bound-variable
/// limit.
fn prune_video_embeddings(conn: &mut SqliteConnection, keep_ids: &[i32]) {
    let all_ids: Vec<i32> = video_embeddings::table
        .select(video_embeddings::video_id)
        .load(conn)
        .unwrap_or_default();
    let keep: std::collections::HashSet<i32> = keep_ids.iter().copied().collect();
    let to_delete: Vec<i32> = all_ids.into_iter().filter(|id| !keep.contains(id)).collect();
    for chunk in to_delete.chunks(400) {
        let _ = diesel::delete(video_embeddings::table.filter(video_embeddings::video_id.eq_any(chunk)))
            .execute(conn);
    }
}

// --- Re-indexing (background job) --------------------------------------------

/// Start a background re-index of the whole library. Returns a job id to poll
/// via [`get_progress`]. Embedding-vector calls run on a dedicated thread with
/// its own current-thread Tokio runtime (mirrors the edit pipeline).
#[allow(clippy::too_many_arguments)]
pub fn start_reindex(
    pool: DbPool,
    ai: AiSettings,
    map: SearchIndexMap,
    transcribe_missing: bool,
    describe_visuals: bool,
    thumbnail_dir: PathBuf,
) -> Result<String, String> {
    // Fail fast when the selected provider has no key configured.
    let has_key = match ai.embedding_provider.as_str() {
        "openai" => ai.openai_api_key.as_deref().map(|k| !k.is_empty()).unwrap_or(false),
        "gemini" => ai.gemini_api_key.as_deref().map(|k| !k.is_empty()).unwrap_or(false),
        other => return Err(format!("Unsupported embedding provider: {}", other)),
    };
    if !has_key {
        return Err(format!(
            "No API key configured for the '{}' embedding provider. Add it under Settings → AI / LLM.",
            ai.embedding_provider
        ));
    }

    // If asked to transcribe missing videos first, the transcription provider
    // needs a key too.
    if transcribe_missing {
        let has_tr_key = match ai.transcription_provider.as_str() {
            "elevenlabs" => ai.elevenlabs_api_key.as_deref().map(|k| !k.is_empty()).unwrap_or(false),
            "openai" => ai.openai_api_key.as_deref().map(|k| !k.is_empty()).unwrap_or(false),
            "gemini" => ai.gemini_api_key.as_deref().map(|k| !k.is_empty()).unwrap_or(false),
            _ => false,
        };
        if !has_tr_key {
            return Err(format!(
                "No API key configured for the '{}' transcription provider. Add it under Settings → AI / LLM, or uncheck \"transcribe missing\".",
                ai.transcription_provider
            ));
        }
    }

    // Visual descriptions use the text/LLM provider (all are multimodal).
    if describe_visuals {
        let has_text_key = match ai.text_provider.as_str() {
            "gemini" => ai.gemini_api_key.as_deref().map(|k| !k.is_empty()).unwrap_or(false),
            "openai" => ai.openai_api_key.as_deref().map(|k| !k.is_empty()).unwrap_or(false),
            "anthropic" => ai.anthropic_api_key.as_deref().map(|k| !k.is_empty()).unwrap_or(false),
            _ => false,
        };
        if !has_text_key {
            return Err(format!(
                "No API key configured for the '{}' text/LLM provider (used for visual descriptions). Add it under Settings → AI / LLM, or uncheck \"describe visuals\".",
                ai.text_provider
            ));
        }
    }

    let job_id = Uuid::new_v4().to_string();
    {
        let mut m = map.lock().unwrap();
        m.insert(job_id.clone(), ReindexProgress::new(job_id.clone()));
    }

    let job_id_thread = job_id.clone();
    std::thread::Builder::new()
        .name(format!("vm-reindex-{}", job_id))
        .spawn(move || {
            let mut conn = match pool.get() {
                Ok(c) => c,
                Err(e) => {
                    update(&map, &job_id_thread, |p| {
                        p.status = "failed".to_string();
                        p.error = Some(format!("Database connection failed: {}", e));
                        p.end_time = Some(Utc::now().naive_utc());
                    });
                    return;
                }
            };
            let rt = match tokio::runtime::Builder::new_current_thread().enable_all().build() {
                Ok(rt) => rt,
                Err(e) => {
                    update(&map, &job_id_thread, |p| {
                        p.status = "failed".to_string();
                        p.error = Some(format!("Failed to start async runtime: {}", e));
                        p.end_time = Some(Utc::now().naive_utc());
                    });
                    return;
                }
            };
            run_reindex(
                &mut conn,
                &ai,
                &rt,
                &map,
                &job_id_thread,
                transcribe_missing,
                describe_visuals,
                &thumbnail_dir,
            );
        })
        .map_err(|e| format!("Failed to spawn reindex thread: {}", e))?;

    Ok(job_id)
}

#[allow(clippy::too_many_arguments)]
fn run_reindex(
    conn: &mut SqliteConnection,
    ai: &AiSettings,
    rt: &tokio::runtime::Runtime,
    map: &SearchIndexMap,
    job_id: &str,
    transcribe_missing: bool,
    describe_visuals: bool,
    thumbnail_dir: &Path,
) {
    let model = model_id(ai);
    info!("[reindex {}] building documents (model {})", job_id, model);

    // Optional pre-pass: transcribe videos that have no transcript yet, so their
    // spoken content becomes searchable. Best-effort — failures are counted and
    // skipped, and silent/no-speech clips simply yield nothing.
    if transcribe_missing {
        transcribe_missing_videos(conn, ai, rt, map, job_id);
    }

    // Optional pre-pass: describe what each un-described video shows, from its
    // thumbnails, so visual content ("running in the snow") becomes searchable.
    if describe_visuals {
        describe_missing_visuals(conn, ai, rt, map, job_id, thumbnail_dir);
    }

    let transcripts = build_transcript_map(conn);
    let visuals = build_visual_map(conn);
    let video_docs = load_video_docs(conn, &transcripts, &visuals);
    let production_docs = load_production_docs(conn, &transcripts);

    // Drop cached embeddings for videos that are no longer eligible (e.g. their
    // only text was an opaque filename, or a transcript/tag was removed). This
    // clears out the content-less noise cluster on every rebuild.
    let keep_ids: Vec<i32> = video_docs.iter().map(|d| d.id).collect();
    prune_video_embeddings(conn, &keep_ids);

    let total = (video_docs.len() + production_docs.len()) as i64;
    update(map, job_id, |p| {
        p.total = total;
        p.stage = "videos".to_string();
    });

    // Videos.
    let stored = stored_video_index(conn);
    if let Err(e) = index_docs(
        conn,
        ai,
        rt,
        map,
        job_id,
        &model,
        &video_docs,
        &stored,
        EntityKind::Video,
    ) {
        finish_failed(map, job_id, &e);
        return;
    }

    update(map, job_id, |p| p.stage = "productions".to_string());

    // Productions.
    let stored = stored_production_index(conn);
    if let Err(e) = index_docs(
        conn,
        ai,
        rt,
        map,
        job_id,
        &model,
        &production_docs,
        &stored,
        EntityKind::Production,
    ) {
        finish_failed(map, job_id, &e);
        return;
    }

    // Deleted videos/productions drop their embeddings via FK cascade, so there
    // is nothing to prune here.
    update(map, job_id, |p| {
        p.stage = "done".to_string();
        p.status = "completed".to_string();
        p.end_time = Some(Utc::now().naive_utc());
    });
    info!("[reindex {}] completed", job_id);
}

/// Best-effort pre-pass: transcribe every video with no transcript yet, so its
/// spoken content becomes searchable. Extracts a compact audio track, calls the
/// configured transcription provider, and stores non-trivial results. Silent /
/// no-speech clips (empty result) are simply skipped; per-video failures are
/// counted and don't abort the run.
fn transcribe_missing_videos(
    conn: &mut SqliteConnection,
    ai: &AiSettings,
    rt: &tokio::runtime::Runtime,
    map: &SearchIndexMap,
    job_id: &str,
) {
    let existing = build_transcript_map(conn);
    let rows: Vec<(i32, String)> = videos::table
        .select((videos::id, videos::file_path))
        .load(conn)
        .unwrap_or_default();
    let candidates: Vec<(i32, String)> = rows
        .into_iter()
        .filter(|(id, path)| !existing.contains_key(id) && Path::new(path).exists())
        .collect();

    let total = candidates.len() as i64;
    update(map, job_id, |p| {
        p.stage = "transcribing".to_string();
        p.transcribe_total = total;
    });
    if candidates.is_empty() {
        return;
    }
    info!("[reindex {}] transcribing {} videos with no transcript", job_id, total);

    let temp_dir = std::env::temp_dir().join("video-manager-reindex-audio");
    for (id, path) in candidates {
        let audio = match ffmpeg_service::extract_audio(Path::new(&path), &temp_dir) {
            Ok(a) => a,
            Err(e) => {
                warn!("[reindex {}] audio extraction failed for video {}: {}", job_id, id, e);
                update(map, job_id, |p| p.transcribe_failed += 1);
                continue;
            }
        };
        let result = rt.block_on(ai_service::transcribe(&audio, ai));
        let _ = std::fs::remove_file(&audio);

        match result {
            // Save only non-trivial text; a couple of stray characters aren't
            // worth embedding (and guard against silence hallucinations).
            Ok(text) if text.trim().chars().count() >= 8 => {
                match ai_service::upsert_transcript(
                    conn,
                    id,
                    text.trim(),
                    &ai.transcription_provider,
                    &ai.transcription_model,
                ) {
                    Ok(()) => update(map, job_id, |p| p.transcribed += 1),
                    Err(e) => {
                        warn!("[reindex {}] failed to save transcript for video {}: {}", job_id, id, e);
                        update(map, job_id, |p| p.transcribe_failed += 1);
                    }
                }
            }
            // No usable speech — processed, nothing to save.
            Ok(_) => update(map, job_id, |p| p.transcribed += 1),
            Err(e) => {
                warn!("[reindex {}] transcription failed for video {}: {}", job_id, id, e);
                update(map, job_id, |p| p.transcribe_failed += 1);
            }
        }
    }
    let _ = std::fs::remove_dir_all(&temp_dir);
}

/// Read up to `max` thumbnail JPEGs for a video, spread across the available
/// frames (first / middle / last), from `<thumbnail_dir>/<checksum>/thumb_N.jpg`.
fn load_thumbnail_frames(thumbnail_dir: &Path, checksum: &str, count: i32, max: usize) -> Vec<Vec<u8>> {
    let count = count.max(0) as usize;
    if count == 0 || max == 0 {
        return Vec::new();
    }
    let out_dir = thumbnail_dir.join(checksum);
    let n = max.min(count);
    let mut frames = Vec::new();
    for k in 0..n {
        let idx = if n == 1 { count / 2 } else { k * (count - 1) / (n - 1) };
        let path = out_dir.join(format!("thumb_{}.jpg", idx));
        if let Ok(bytes) = std::fs::read(&path) {
            if !bytes.is_empty() {
                frames.push(bytes);
            }
        }
    }
    frames
}

/// Best-effort pre-pass: for every video with no visual description yet, send a
/// few of its thumbnails to the vision LLM and store the returned caption/tags,
/// so visual content becomes searchable. Per-video failures are counted and
/// don't abort the run.
fn describe_missing_visuals(
    conn: &mut SqliteConnection,
    ai: &AiSettings,
    rt: &tokio::runtime::Runtime,
    map: &SearchIndexMap,
    job_id: &str,
    thumbnail_dir: &Path,
) {
    let existing = build_visual_map(conn);
    let rows: Vec<(i32, Option<String>, i32)> = videos::table
        .select((videos::id, videos::checksum, videos::thumbnail_count))
        .load(conn)
        .unwrap_or_default();
    let candidates: Vec<(i32, String, i32)> = rows
        .into_iter()
        .filter_map(|(id, checksum, count)| {
            if existing.contains_key(&id) {
                return None;
            }
            let cs = checksum.filter(|c| !c.is_empty())?;
            (count > 0).then_some((id, cs, count))
        })
        .collect();

    let total = candidates.len() as i64;
    update(map, job_id, |p| {
        p.stage = "describing".to_string();
        p.describe_total = total;
    });
    if candidates.is_empty() {
        return;
    }
    info!("[reindex {}] describing visuals for {} videos", job_id, total);

    for (id, checksum, count) in candidates {
        let frames = load_thumbnail_frames(thumbnail_dir, &checksum, count, 3);
        if frames.is_empty() {
            warn!("[reindex {}] no thumbnails on disk for video {}", job_id, id);
            update(map, job_id, |p| p.describe_failed += 1);
            continue;
        }
        match rt.block_on(ai_service::describe_video_frames(&frames, ai)) {
            Ok(desc) if !desc.trim().is_empty() => {
                match ai_service::upsert_visual_description(conn, id, desc.trim()) {
                    Ok(()) => update(map, job_id, |p| p.described += 1),
                    Err(e) => {
                        warn!("[reindex {}] failed to save visual description for video {}: {}", job_id, id, e);
                        update(map, job_id, |p| p.describe_failed += 1);
                    }
                }
            }
            Ok(_) => update(map, job_id, |p| p.describe_failed += 1),
            Err(e) => {
                warn!("[reindex {}] visual description failed for video {}: {}", job_id, id, e);
                update(map, job_id, |p| p.describe_failed += 1);
            }
        }
    }
}

#[derive(Clone, Copy)]
enum EntityKind {
    Video,
    Production,
}

/// Embed the changed documents in batches and upsert their vectors, updating
/// progress as it goes. Unchanged documents (same hash + model) are skipped.
#[allow(clippy::too_many_arguments)]
fn index_docs(
    conn: &mut SqliteConnection,
    ai: &AiSettings,
    rt: &tokio::runtime::Runtime,
    map: &SearchIndexMap,
    job_id: &str,
    model: &str,
    docs: &[Doc],
    stored: &HashMap<i32, (String, String)>,
    kind: EntityKind,
) -> Result<(), String> {
    // Skip unchanged; collect the rest to embed.
    let mut to_embed: Vec<&Doc> = Vec::new();
    let mut skipped = 0i64;
    for doc in docs {
        match stored.get(&doc.id) {
            Some((h, m)) if h == &doc.hash && m == model => skipped += 1,
            _ => to_embed.push(doc),
        }
    }
    bump(map, job_id, kind, 0, skipped);

    for batch in to_embed.chunks(EMBED_BATCH) {
        let inputs: Vec<String> = batch.iter().map(|d| d.text.clone()).collect();
        let vectors = rt
            .block_on(ai_service::embed_texts(&inputs, ai))
            .map_err(|e| format!("Embedding failed: {}", e))?;
        if vectors.len() != batch.len() {
            return Err(format!(
                "Embedding provider returned {} vectors for {} inputs",
                vectors.len(),
                batch.len()
            ));
        }

        let now = Utc::now().naive_utc();
        for (doc, vec) in batch.iter().zip(vectors.iter()) {
            let blob = embedding_to_blob(vec);
            let dim = vec.len() as i32;
            let res = match kind {
                EntityKind::Video => diesel::replace_into(video_embeddings::table)
                    .values(&NewVideoEmbedding {
                        video_id: doc.id,
                        content_hash: doc.hash.clone(),
                        model: model.to_string(),
                        dim,
                        embedding: blob,
                        updated_at: now,
                    })
                    .execute(conn),
                EntityKind::Production => diesel::replace_into(production_embeddings::table)
                    .values(&NewProductionEmbedding {
                        production_id: doc.id,
                        content_hash: doc.hash.clone(),
                        model: model.to_string(),
                        dim,
                        embedding: blob,
                        updated_at: now,
                    })
                    .execute(conn),
            };
            if let Err(e) = res {
                error!("[reindex {}] failed to store embedding for {}: {}", job_id, doc.id, e);
            }
        }
        bump(map, job_id, kind, batch.len() as i64, 0);
    }

    Ok(())
}

fn bump(map: &SearchIndexMap, job_id: &str, kind: EntityKind, indexed: i64, skipped: i64) {
    update(map, job_id, |p| {
        p.processed += indexed + skipped;
        match kind {
            EntityKind::Video => {
                p.videos_indexed += indexed;
                p.videos_skipped += skipped;
            }
            EntityKind::Production => {
                p.productions_indexed += indexed;
                p.productions_skipped += skipped;
            }
        }
    });
}

fn finish_failed(map: &SearchIndexMap, job_id: &str, err: &str) {
    error!("[reindex {}] failed: {}", job_id, err);
    update(map, job_id, |p| {
        p.status = "failed".to_string();
        p.error = Some(err.to_string());
        p.end_time = Some(Utc::now().naive_utc());
    });
}

// --- Querying ----------------------------------------------------------------

/// Rank all indexed videos for the current model against a query vector,
/// returning `(video_id, score)` for the top `limit`, best first. Only vectors
/// from the same `model` (and same dimensionality) participate.
pub fn rank_videos(
    conn: &mut SqliteConnection,
    model: &str,
    query: &[f32],
    limit: usize,
) -> Vec<(i32, f32)> {
    let rows: Vec<(i32, Vec<u8>)> = video_embeddings::table
        .filter(video_embeddings::model.eq(model))
        .select((video_embeddings::video_id, video_embeddings::embedding))
        .load(conn)
        .unwrap_or_default();
    rank(rows, query, limit)
}

/// Rank all indexed productions for the current model against a query vector.
pub fn rank_productions(
    conn: &mut SqliteConnection,
    model: &str,
    query: &[f32],
    limit: usize,
) -> Vec<(i32, f32)> {
    let rows: Vec<(i32, Vec<u8>)> = production_embeddings::table
        .filter(production_embeddings::model.eq(model))
        .select((production_embeddings::production_id, production_embeddings::embedding))
        .load(conn)
        .unwrap_or_default();
    rank(rows, query, limit)
}

fn rank(rows: Vec<(i32, Vec<u8>)>, query: &[f32], limit: usize) -> Vec<(i32, f32)> {
    let mut scored: Vec<(i32, f32)> = rows
        .into_iter()
        .map(|(id, blob)| (id, cosine_similarity(query, &blob_to_embedding(&blob))))
        .collect();
    scored.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
    scored.truncate(limit);
    scored
}

/// Whether a ranking has no meaningfully-better match — the top results are all
/// essentially tied. This happens when a query matches nothing specific and
/// falls back to a block of near-identical low-information documents (e.g.
/// "running in the snow" over a library with no snow footage). Measures relative
/// flatness (top vs the 10th result), so it's independent of the model's
/// absolute cosine scale.
pub fn is_weak_ranking(ranked: &[(i32, f32)]) -> bool {
    if ranked.len() < 3 {
        return false;
    }
    let top = ranked[0].1;
    let low = ranked[ranked.len().min(10) - 1].1;
    // A real match spreads the top scores out (empirically a gap of 0.10+); a
    // query with nothing specific to match sits in a flat band (~0.03). 0.05 is
    // the midpoint that flags the latter without tripping on genuine matches.
    (top - low) < 0.05
}

/// Snapshot of index coverage for the current model, for the Settings UI.
#[derive(Debug, Serialize)]
pub struct IndexStatus {
    pub model: String,
    pub videos_total: i64,
    pub videos_indexed: i64,
    pub productions_total: i64,
    pub productions_indexed: i64,
}

pub fn index_status(conn: &mut SqliteConnection, ai: &AiSettings) -> IndexStatus {
    let model = model_id(ai);
    let videos_total: i64 = videos::table.count().get_result(conn).unwrap_or(0);
    let productions_total: i64 = productions::table.count().get_result(conn).unwrap_or(0);
    let videos_indexed: i64 = video_embeddings::table
        .filter(video_embeddings::model.eq(&model))
        .count()
        .get_result(conn)
        .unwrap_or(0);
    let productions_indexed: i64 = production_embeddings::table
        .filter(production_embeddings::model.eq(&model))
        .count()
        .get_result(conn)
        .unwrap_or(0);
    IndexStatus {
        model,
        videos_total,
        videos_indexed,
        productions_total,
        productions_indexed,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cosine_identical_is_one() {
        let v = vec![0.2, 0.5, 0.9, -0.3];
        assert!((cosine_similarity(&v, &v) - 1.0).abs() < 1e-5);
    }

    #[test]
    fn cosine_orthogonal_is_zero() {
        let a = vec![1.0, 0.0];
        let b = vec![0.0, 1.0];
        assert!(cosine_similarity(&a, &b).abs() < 1e-6);
    }

    #[test]
    fn cosine_handles_mismatch_and_zero() {
        assert_eq!(cosine_similarity(&[1.0, 2.0], &[1.0]), 0.0);
        assert_eq!(cosine_similarity(&[0.0, 0.0], &[1.0, 1.0]), 0.0);
        assert_eq!(cosine_similarity(&[], &[]), 0.0);
    }

    #[test]
    fn blob_roundtrips() {
        let v = vec![0.0f32, 1.5, -2.25, 3.125, 1e-7];
        let blob = embedding_to_blob(&v);
        assert_eq!(blob.len(), v.len() * 4);
        assert_eq!(blob_to_embedding(&blob), v);
    }

    #[test]
    fn blob_ignores_trailing_partial_float() {
        let mut blob = embedding_to_blob(&[1.0, 2.0]);
        blob.push(0xAB); // stray byte
        assert_eq!(blob_to_embedding(&blob), vec![1.0, 2.0]);
    }

    #[test]
    fn content_hash_is_stable_and_sensitive() {
        assert_eq!(content_hash("hello"), content_hash("hello"));
        assert_ne!(content_hash("hello"), content_hash("hello!"));
    }

    #[test]
    fn video_doc_drops_empty_sections_and_labels_the_rest() {
        let doc = assemble_video_doc(
            "run_snow.mp4",
            None,
            Some("Whistler"),
            None,
            &["running".to_string(), "winter".to_string()],
            Some("Today I went for a jog in the fresh snow."),
            &[],
            Some("A person running on a snow-covered trail. Tags: snow, running, winter"),
        );
        assert!(doc.contains("File: run_snow.mp4"));
        assert!(doc.contains("Location: Whistler"));
        assert!(doc.contains("Tags: running, winter"));
        assert!(doc.contains("Visuals: A person running on a snow-covered trail"));
        assert!(doc.contains("Transcript: Today I went for a jog"));
        assert!(!doc.contains("Category:"));
        assert!(!doc.contains("Notes:"));
    }

    #[test]
    fn production_doc_joins_take_transcripts() {
        let doc = assemble_production_doc(
            "Parenting tips",
            Some("YouTube"),
            Some("Talk about bedtime routines."),
            None,
            &["Kids need a consistent bedtime".to_string(), "".to_string()],
        );
        assert!(doc.contains("Title: Parenting tips"));
        assert!(doc.contains("Platform: YouTube"));
        assert!(doc.contains("Script: Talk about bedtime routines."));
        assert!(doc.contains("Transcript: Kids need a consistent bedtime"));
    }

    #[test]
    fn doc_is_truncated_to_cap() {
        let long = "a".repeat(MAX_DOC_CHARS + 500);
        let doc = assemble_video_doc(&long, None, None, None, &[], None, &[], None);
        assert_eq!(doc.chars().count(), MAX_DOC_CHARS);
    }

    #[test]
    fn copy_json_flattens_strings_and_arrays() {
        let json = r#"{"titles":["A","B"],"description":"Desc","tags":["t1","t2"],"nested":{"x":1}}"#;
        let text = copy_json_to_text(json);
        assert!(text.contains("Desc"));
        assert!(text.contains("A") && text.contains("B"));
        assert!(text.contains("t1") && text.contains("t2"));
    }

    #[test]
    fn descriptive_filename_detection() {
        // Opaque camera/device codes → not descriptive (skipped/omitted).
        assert!(!filename_is_descriptive("GX011916.MP4"));
        assert!(!filename_is_descriptive("IMG_1234.MOV"));
        assert!(!filename_is_descriptive("GH010123.mp4"));
        assert!(!filename_is_descriptive("C0001.MP4"));
        // Real words → descriptive.
        assert!(filename_is_descriptive("morning_run_whistler.mp4"));
        assert!(filename_is_descriptive("snow-run.mov"));
        assert!(filename_is_descriptive("whistler.mp4")); // single long word
    }

    #[test]
    fn weak_ranking_flags_flat_ties_only() {
        // A block of tied scores → weak (no real match).
        let flat: Vec<(i32, f32)> = (0..12).map(|i| (i, 0.45)).collect();
        assert!(is_weak_ranking(&flat));
        // A clear downward slope → not weak (there's a real winner).
        let sloped: Vec<(i32, f32)> = (0..12).map(|i| (i, 0.60 - i as f32 * 0.02)).collect();
        assert!(!is_weak_ranking(&sloped));
        // A couple of strong matches above a flat pack → not weak.
        let mut mixed = vec![(1, 0.50f32), (2, 0.47)];
        mixed.extend((0..10).map(|i| (100 + i, 0.34)));
        assert!(!is_weak_ranking(&mixed));
        // Tiny result sets are never flagged.
        assert!(!is_weak_ranking(&[(1, 0.45), (2, 0.45)]));
    }

    #[test]
    fn rank_orders_by_similarity_and_truncates() {
        let query = vec![1.0f32, 0.0];
        let rows = vec![
            (1, embedding_to_blob(&[1.0, 0.0])),   // score 1.0
            (2, embedding_to_blob(&[0.0, 1.0])),   // score 0.0
            (3, embedding_to_blob(&[0.7, 0.7])),   // score ~0.707
        ];
        let ranked = rank(rows, &query, 2);
        assert_eq!(ranked.len(), 2);
        assert_eq!(ranked[0].0, 1);
        assert_eq!(ranked[1].0, 3);
    }
}
