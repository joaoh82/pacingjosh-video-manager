use chrono::NaiveDateTime;
use diesel::prelude::*;
use serde::Serialize;

use crate::schema::production_edits;

/// A persisted video-edit pipeline result for a production.
#[derive(Debug, Queryable, Selectable, Identifiable, Serialize, Clone)]
#[diesel(table_name = production_edits)]
pub struct ProductionEdit {
    pub id: i32,
    pub production_id: i32,
    pub status: String,
    pub script: Option<String>,
    pub instructions: Option<String>,
    pub edl_json: Option<String>,
    pub output_path: Option<String>,
    pub edl_path: Option<String>,
    pub error: Option<String>,
    pub transcription_provider: Option<String>,
    pub text_provider: Option<String>,
    pub text_model: Option<String>,
    pub created_at: NaiveDateTime,
    pub logs: Option<String>,
    pub transcripts_json: Option<String>,
    pub options_json: Option<String>,
    pub copy_json: Option<String>,
}

#[derive(Debug, Insertable)]
#[diesel(table_name = production_edits)]
pub struct NewProductionEdit {
    pub production_id: i32,
    pub status: String,
    pub script: Option<String>,
    pub instructions: Option<String>,
    pub edl_json: Option<String>,
    pub output_path: Option<String>,
    pub edl_path: Option<String>,
    pub error: Option<String>,
    pub transcription_provider: Option<String>,
    pub text_provider: Option<String>,
    pub text_model: Option<String>,
    pub created_at: NaiveDateTime,
    pub logs: Option<String>,
    pub transcripts_json: Option<String>,
    pub options_json: Option<String>,
}

/// API-facing shape with `edl_json` decoded into a structured value.
#[derive(Debug, Serialize, Clone)]
pub struct ProductionEditResponse {
    pub id: i32,
    pub production_id: i32,
    pub status: String,
    pub script: Option<String>,
    pub instructions: Option<String>,
    pub edl: Option<serde_json::Value>,
    pub output_path: Option<String>,
    pub edl_path: Option<String>,
    pub error: Option<String>,
    pub logs: Vec<String>,
    pub transcription_provider: Option<String>,
    pub text_provider: Option<String>,
    pub text_model: Option<String>,
    pub created_at: NaiveDateTime,
    /// Generated YouTube copy (titles / description / tags / thumbnail text).
    pub copy: Option<serde_json::Value>,
}

impl From<ProductionEdit> for ProductionEditResponse {
    fn from(e: ProductionEdit) -> Self {
        let edl = e
            .edl_json
            .as_deref()
            .and_then(|s| serde_json::from_str::<serde_json::Value>(s).ok());
        let logs = e
            .logs
            .as_deref()
            .and_then(|s| serde_json::from_str::<Vec<String>>(s).ok())
            .unwrap_or_default();
        let copy = e
            .copy_json
            .as_deref()
            .and_then(|s| serde_json::from_str::<serde_json::Value>(s).ok());
        Self {
            id: e.id,
            production_id: e.production_id,
            status: e.status,
            script: e.script,
            instructions: e.instructions,
            edl,
            output_path: e.output_path,
            edl_path: e.edl_path,
            error: e.error,
            logs,
            transcription_provider: e.transcription_provider,
            text_provider: e.text_provider,
            text_model: e.text_model,
            created_at: e.created_at,
            copy,
        }
    }
}
