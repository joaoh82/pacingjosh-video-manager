# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

Video Manager is a local-first desktop application for indexing, searching, and managing a video library. It extracts metadata via FFmpeg/ffprobe, generates thumbnails, and provides a web UI for browsing, tagging, and tracking video usage in productions.

> **Important:** The Rust backend (`backend-rust/`) is the **active, maintained** backend. The Python/FastAPI backend (`backend/`) is **deprecated legacy** — do not modify it or suggest running it unless explicitly asked.

> **Shipping target:** The project is packaged as a cross-platform Tauri 2.0 desktop app (`src-tauri/`). The Rust backend is compiled as a library and embedded inside the Tauri process; the Next.js frontend is static-exported into `frontend/out/` and served from the Tauri WebView.

## Development Commands

### Backend (Rust / Actix-web)
```bash
cd backend-rust

# First-time setup
cargo install diesel_cli --no-default-features --features sqlite
cp .env.example .env          # edit VIDEO_DIRECTORY, PORT, etc.
diesel migration run

# Run dev server (localhost:8000)
cargo run

# Release build
cargo build --release
./target/release/video-manager-backend
```

Key `.env` variables: `HOST`, `PORT`, `DATABASE_PATH`, `VIDEO_DIRECTORY`, `THUMBNAIL_DIRECTORY`, `THUMBNAIL_COUNT`, `THUMBNAIL_WIDTH`.

### Frontend (Next.js 14)
```bash
cd frontend
npm install
npm run dev                   # Dev server (localhost:3000)
npm run build                 # Production build
npm run lint                  # ESLint
```

### Desktop App (Tauri)
```bash
# First-time setup
cargo install tauri-cli --version "^2.0"
bash scripts/fetch-ffmpeg.sh      # or scripts\fetch-ffmpeg.ps1 on Windows
cargo tauri icon images/Logo.png  # generate icons from the repo logo

# Dev mode — launches Next.js dev server + Tauri window (run from repo root)
cargo tauri dev

# Production bundle (msi/dmg/deb/appimage)
cargo tauri build
```

The Tauri shell embeds the backend as a library (`video_manager_backend::run_blocking`) on a dedicated OS thread, picks a free port at startup, and injects `window.__VMAN_API__` so the static-exported frontend knows where to find the backend.

### Database Migrations (Diesel)
```bash
cd backend-rust
diesel migration generate description_of_changes
diesel migration run
diesel migration revert
```

## Architecture

### Backend (backend-rust/src/)

Layered architecture: **Routes → Services → Models/Schema**

- **`lib.rs`**: Public `run(BackendPaths) -> async` and `run_blocking(BackendPaths)` entry points. `main.rs` is a thin wrapper that uses CWD-relative `./data`; the Tauri shell calls `run_blocking` with the OS-specific app-data dir.
- **Routes** (`routes/`): Actix-web handlers — `videos.rs`, `scan.rs`, `tags.rs`, `productions.rs`, `stream.rs`, `config_routes.rs`, `ai.rs` (AI settings + per-video copy generation), `edit.rs` (the Edit & Create Video pipeline). All mounted under `/api`.
- **Services** (`services/`): Business logic — `scanner.rs` (background directory scanning), `ffmpeg_service.rs` (metadata/thumbnails/clip extraction + concat, with injectable `FfmpegPaths` via `set_ffmpeg_paths()`), `video_service.rs`, `search_service.rs`, `production_service.rs`, `ai_service.rs` (transcription incl. ElevenLabs/OpenAI/Gemini + LLM completion), `edit_service.rs` (background edit pipeline orchestration, in-memory `EditJobMap`).
- **Models** (`models/`): Diesel ORM — `video.rs`, `tag.rs`, `production.rs`, `ai.rs`, `edit.rs`.
- **Schema** (`schema.rs`): Diesel auto-generated schema from migrations.
- **Config** (`config.rs`): `ConfigManager::new(app_data_dir)` — all default paths (database, thumbnails, config.json) are rooted at the provided app-data dir.
- **DB** (`db.rs`): Connection pool setup.

### Frontend (frontend/src/)

- **App Router** (`app/`): Pages — main grid (`page.tsx`), setup wizard (`setup/`), settings (`settings/`).
- **Components** (`components/`): VideoGrid, VideoCard, VideoModal (player + metadata editor + AI content panel), FilterPanel, Scanner (progress polling), ProductionManager, VideoEditPipeline (the Edit & Create Video modal), EditTimeline (CapCut-style video/voice/music track preview built from `edl.timeline`), BulkActions.
- **API Client** (`lib/api.ts`): Centralized typed fetch functions for all backend endpoints.
- **Types** (`lib/types.ts`): Shared TypeScript interfaces matching backend schemas.

### API Proxying
Next.js rewrites `/api/*` requests to `http://localhost:8000/api/*` (configured in `next.config.js`). The frontend calls `/api/...` paths directly — no separate API URL needed in client code.

### Key Data Flow: Video Scanning
1. Frontend POSTs to `/api/scan` → backend creates a scan ID and spawns an async task
2. Scanner runs two passes: count files, then process each (ffprobe metadata → save to DB → generate thumbnails)
3. Progress tracked in-memory — not persistent across restarts
4. Frontend polls `/api/scan/status/{scanId}` every 1 second via the Scanner component

### Key Data Flow: Edit & Create Video pipeline
1. User adds raw takes to a production, then POSTs a script (+ optional instructions) to `/api/productions/{id}/edit`
2. `edit_service::start_edit` loads the production's videos, creates a job id in the in-memory `EditJobMap`, and spawns a dedicated OS thread (with its own current-thread Tokio runtime for the async HTTP calls)
3. The thread runs the stages: extract audio + transcribe each take with word-level timestamps (`ai_service::transcribe_timed`) → build the planning prompt and call the LLM (`ai_service::complete`) → parse/validate the returned EDL against the real takes (clamp ranges to durations) → optionally "tighten" each clip into speech-only sub-clips by dropping long silences/filler (`clip_keep_segments`, driven by word timestamps) → extract a normalized segment per (sub-)clip (burning in per-clip captions re-timed from the transcript when enabled) → concat with FFmpeg → optionally mix in looped background music with two explicit levels — a `music_volume` for pauses and a lower `music_duck_volume` while talking. Ducking is driven by the KNOWN speech intervals (transcript word timestamps mapped onto the final timeline), applied as a `volume` expression on the music — deterministic and independent of recording level, so `0` truly silences music during speech (`ffmpeg_service::add_background_music` takes the speech intervals). All output goes to a per-run version folder `<output_dir>/productions/v<N>/` (N = max existing `v*` + 1, so re-edits never overwrite; the chosen `output_dir` is used verbatim so nothing nests): the final `.mp4`, its `.json` EDL (same basename), and a transient `.work-<job>/` removed on success. **Nothing is written to the app-data directory** — `output_dir` is required. Deleting a run (`DELETE /api/edits/{id}`) removes the DB row and the files, plus the now-empty version folder.
4. The result (EDL + output path + activity log) is persisted to the `production_edits` table; the frontend polls `/api/edit/status/{job_id}` (~1.5s) via the VideoEditPipeline modal. `EditOptions` carries the per-run inputs (script, instructions, output dir/name, captions flag, music path/volume). On reopen, `/api/productions/{id}/edit` returns the latest result and `/api/productions/{id}/edits` returns the full history (script, EDL, logs per run)
5. Transcription word timestamps come from ElevenLabs (Scribe) or OpenAI (Whisper); Gemini returns plain text only (no fine cut points). Keys/models/prompts live in `AiSettings` (config.json), set via Settings → AI / LLM

### Database Schema
SQLite at `backend-rust/data/database.db`. Key relationships:
- **Video ↔ Metadata**: 1:1 (cascade delete)
- **Video ↔ Tag**: Many-to-many via `video_tags` junction table
- **Video ↔ Production**: Many-to-many via `video_productions` junction table
- **Video ↔ AiGeneration**: 1:1 (`ai_generations`, cascade delete) — saved transcript + social copy
- **Production ↔ ProductionEdit**: 1:many (`production_edits`, cascade delete) — persisted edit pipeline runs (EDL JSON + final video path)

### Video Streaming
`routes/stream.rs` supports HTTP range requests (206 Partial Content) for seeking — streams file chunks without loading entire video into memory.

## Important Patterns

- **Connection pooling**: Diesel r2d2 pool shared via Actix-web app data
- **Styling**: Tailwind CSS throughout, custom blue primary palette
- **Path alias**: `@/*` maps to `frontend/src/*`

## Known Issues

- Pre-existing ESLint errors in `settings/page.tsx` and `setup/page.tsx` (unescaped `'` entities)
- No test suite exists yet

## External Dependencies

- **FFmpeg/ffprobe** must be installed and available on PATH for metadata extraction and thumbnail generation
- **Rust** 1.75+ and **Diesel CLI** (sqlite feature) required for backend development
- **Supported video formats**: `.mp4`, `.mov`, `.avi`, `.mkv`, `.webm`, `.flv`, `.wmv`
