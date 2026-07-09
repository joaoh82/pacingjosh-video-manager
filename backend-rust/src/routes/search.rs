//! Semantic search endpoints.
//!
//! `GET /api/search/semantic` embeds the query and ranks the indexed
//! videos/productions by cosine similarity. `POST /api/search/reindex` builds
//! (or refreshes) the embedding index in the background — polled via
//! `GET /api/search/reindex/status/{job_id}` — and `GET /api/search/index-status`
//! reports coverage for the current embedding model.

use actix_web::{get, post, web, HttpResponse};
use serde::Deserialize;

use crate::config::ConfigManager;
use crate::db::DbPool;
use crate::services::embedding_service::{self, SearchIndexMap};
use crate::services::{ai_service, production_service, video_service};

#[derive(Deserialize)]
pub struct SemanticSearchParams {
    /// The natural-language query.
    pub q: Option<String>,
    /// "videos" (default) or "productions".
    pub r#type: Option<String>,
    pub limit: Option<usize>,
}

#[get("/search/semantic")]
async fn semantic_search(
    pool: web::Data<DbPool>,
    config: web::Data<ConfigManager>,
    query: web::Query<SemanticSearchParams>,
) -> HttpResponse {
    let q = query.q.as_deref().unwrap_or("").trim().to_string();
    if q.is_empty() {
        return HttpResponse::BadRequest().json(serde_json::json!({
            "detail": "A search query is required.",
        }));
    }
    let kind = query.r#type.as_deref().unwrap_or("videos");
    let limit = query.limit.unwrap_or(30).clamp(1, 100);

    let ai = config.get_ai_settings();
    let model = embedding_service::model_id(&ai);

    // Embed the query first (no DB connection held across the await).
    let query_vec = match ai_service::embed_texts(std::slice::from_ref(&q), &ai).await {
        Ok(mut v) if !v.is_empty() => v.remove(0),
        Ok(_) => {
            return HttpResponse::InternalServerError().json(serde_json::json!({
                "detail": "The embedding provider returned no vector for the query.",
            }));
        }
        Err(e) => {
            return HttpResponse::BadRequest().json(serde_json::json!({ "detail": e }));
        }
    };

    let mut conn = pool.get().expect("Failed to get DB connection");
    let status = embedding_service::index_status(&mut conn, &ai);

    if kind == "productions" {
        let ranked = embedding_service::rank_productions(&mut conn, &model, &query_vec, limit);
        let weak = embedding_service::is_weak_ranking(&ranked);
        let productions: Vec<_> = ranked
            .into_iter()
            .filter_map(|(id, _score)| production_service::get_production_response(&mut conn, id))
            .collect();
        HttpResponse::Ok().json(serde_json::json!({
            "productions": productions,
            "total": productions_len(&productions),
            "index_empty": status.productions_indexed == 0,
            "weak_match": weak,
        }))
    } else {
        let ranked = embedding_service::rank_videos(&mut conn, &model, &query_vec, limit);
        let weak = embedding_service::is_weak_ranking(&ranked);
        let videos: Vec<_> = ranked
            .into_iter()
            .filter_map(|(id, _score)| video_service::get_video(&mut conn, id))
            .collect();
        let total = videos.len() as i64;
        HttpResponse::Ok().json(serde_json::json!({
            "videos": videos,
            "total": total,
            "index_empty": status.videos_indexed == 0,
            "weak_match": weak,
        }))
    }
}

fn productions_len<T>(v: &[T]) -> i64 {
    v.len() as i64
}

#[derive(Deserialize, Default)]
#[serde(default)]
pub struct ReindexRequest {
    /// Transcribe videos that have no transcript before indexing (slower, uses
    /// the transcription API).
    pub transcribe_missing: bool,
    /// Describe what un-described videos show (from their thumbnails) via the
    /// vision LLM before indexing (slower, uses the text/LLM API).
    pub describe_visuals: bool,
}

#[post("/search/reindex")]
async fn reindex(
    pool: web::Data<DbPool>,
    config: web::Data<ConfigManager>,
    index_map: web::Data<SearchIndexMap>,
    body: Option<web::Json<ReindexRequest>>,
) -> HttpResponse {
    let ai = config.get_ai_settings();
    let (transcribe_missing, describe_visuals) = body
        .map(|b| (b.transcribe_missing, b.describe_visuals))
        .unwrap_or((false, false));
    let thumbnail_dir = config.get_thumbnail_directory();
    match embedding_service::start_reindex(
        pool.get_ref().clone(),
        ai,
        index_map.get_ref().clone(),
        transcribe_missing,
        describe_visuals,
        thumbnail_dir,
    ) {
        Ok(job_id) => HttpResponse::Ok().json(serde_json::json!({
            "status": "started",
            "job_id": job_id,
            "message": "Semantic index build started.",
        })),
        Err(e) => HttpResponse::BadRequest().json(serde_json::json!({ "detail": e })),
    }
}

#[get("/search/reindex/status/{job_id}")]
async fn reindex_status(
    index_map: web::Data<SearchIndexMap>,
    path: web::Path<String>,
) -> HttpResponse {
    let job_id = path.into_inner();
    match embedding_service::get_progress(index_map.get_ref(), &job_id) {
        Some(progress) => HttpResponse::Ok().json(progress),
        None => HttpResponse::NotFound().json(serde_json::json!({
            "detail": format!("Reindex job not found: {}", job_id),
        })),
    }
}

#[get("/search/index-status")]
async fn index_status(
    pool: web::Data<DbPool>,
    config: web::Data<ConfigManager>,
) -> HttpResponse {
    let ai = config.get_ai_settings();
    let mut conn = pool.get().expect("Failed to get DB connection");
    let status = embedding_service::index_status(&mut conn, &ai);
    HttpResponse::Ok().json(status)
}

pub fn configure(cfg: &mut web::ServiceConfig) {
    cfg.service(semantic_search)
        .service(reindex)
        .service(reindex_status)
        .service(index_status);
}
