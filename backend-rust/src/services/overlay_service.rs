//! Built-in overlay snippets (e.g. the "Subscribe" animation) that can be
//! composited onto a final edit in the pauses where the creator isn't talking.
//!
//! The snippet ships embedded in the binary (so it's always available, even in
//! the packaged desktop app) and is written out to `<app-data>/overlays/` the
//! first time it's needed. Overlays are transparent GIFs/PNGs — their native
//! alpha is used, so there's no background to key out. Users can also point an
//! overlay at any image/GIF of their own; that path is used verbatim.

use serde::Serialize;
use std::path::{Path, PathBuf};

/// The bundled YouTube "Subscribe" snippet — a ~5s animated GIF with a
/// transparent background. Embedded from the repo asset so it ships with the
/// binary.
const SUBSCRIBE_BYTES: &[u8] = include_bytes!("../../../assets/overlays/youtube_subscribe.gif");

/// Filename the bundled subscribe snippet is written out as.
const SUBSCRIBE_FILENAME: &str = "youtube_subscribe.gif";

/// Metadata for a built-in overlay, surfaced to the frontend so it can offer a
/// one-click "add this overlay" affordance.
#[derive(Debug, Clone, Serialize)]
pub struct BuiltinOverlay {
    /// Stable id for the built-in.
    pub id: String,
    /// Human label shown in the UI.
    pub label: String,
    /// Absolute path to the snippet on disk (written out on demand).
    pub path: String,
}

/// The directory built-in overlays are written to under the app-data dir.
pub fn overlays_dir(app_data_dir: &Path) -> PathBuf {
    app_data_dir.join("overlays")
}

/// Ensure the bundled snippet(s) exist on disk under `<app-data>/overlays/`.
/// Idempotent: only writes a file if it's missing (or a different size, so an
/// updated bundled asset replaces a stale copy). Safe to call at startup.
pub fn ensure_builtin_overlays(app_data_dir: &Path) -> std::io::Result<PathBuf> {
    let dir = overlays_dir(app_data_dir);
    std::fs::create_dir_all(&dir)?;

    let subscribe = dir.join(SUBSCRIBE_FILENAME);
    let needs_write = match std::fs::metadata(&subscribe) {
        Ok(m) => m.len() != SUBSCRIBE_BYTES.len() as u64,
        Err(_) => true,
    };
    if needs_write {
        std::fs::write(&subscribe, SUBSCRIBE_BYTES)?;
    }
    Ok(dir)
}

/// List the built-in overlays, writing them out first so the returned paths are
/// valid on disk.
pub fn list_builtin_overlays(app_data_dir: &Path) -> Vec<BuiltinOverlay> {
    let dir = match ensure_builtin_overlays(app_data_dir) {
        Ok(d) => d,
        Err(_) => overlays_dir(app_data_dir),
    };
    vec![BuiltinOverlay {
        id: "subscribe".to_string(),
        label: "Subscribe button".to_string(),
        path: dir.join(SUBSCRIBE_FILENAME).to_string_lossy().to_string(),
    }]
}
