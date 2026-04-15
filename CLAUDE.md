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
- **Routes** (`routes/`): Actix-web handlers — `videos.rs`, `scan.rs`, `tags.rs`, `productions.rs`, `stream.rs`, `config_routes.rs`. All mounted under `/api`.
- **Services** (`services/`): Business logic — `scanner.rs` (background directory scanning), `ffmpeg_service.rs` (metadata/thumbnails, with injectable `FfmpegPaths` via `set_ffmpeg_paths()`), `video_service.rs`, `search_service.rs`, `production_service.rs`.
- **Models** (`models/`): Diesel ORM — `video.rs`, `tag.rs`, `production.rs`.
- **Schema** (`schema.rs`): Diesel auto-generated schema from migrations.
- **Config** (`config.rs`): `ConfigManager::new(app_data_dir)` — all default paths (database, thumbnails, config.json) are rooted at the provided app-data dir.
- **DB** (`db.rs`): Connection pool setup.

### Frontend (frontend/src/)

- **App Router** (`app/`): Pages — main grid (`page.tsx`), setup wizard (`setup/`), settings (`settings/`).
- **Components** (`components/`): VideoGrid, VideoCard, VideoModal (player + metadata editor), FilterPanel, Scanner (progress polling), ProductionManager, BulkActions.
- **API Client** (`lib/api.ts`): Centralized typed fetch functions for all backend endpoints.
- **Types** (`lib/types.ts`): Shared TypeScript interfaces matching backend schemas.

### API Proxying
Next.js rewrites `/api/*` requests to `http://localhost:8000/api/*` (configured in `next.config.js`). The frontend calls `/api/...` paths directly — no separate API URL needed in client code.

### Key Data Flow: Video Scanning
1. Frontend POSTs to `/api/scan` → backend creates a scan ID and spawns an async task
2. Scanner runs two passes: count files, then process each (ffprobe metadata → save to DB → generate thumbnails)
3. Progress tracked in-memory — not persistent across restarts
4. Frontend polls `/api/scan/status/{scanId}` every 1 second via the Scanner component

### Database Schema
SQLite at `backend-rust/data/database.db`. Key relationships:
- **Video ↔ Metadata**: 1:1 (cascade delete)
- **Video ↔ Tag**: Many-to-many via `video_tags` junction table
- **Video ↔ Production**: Many-to-many via `video_productions` junction table

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
