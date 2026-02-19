# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

Video Manager is a local-first desktop application for indexing, searching, and managing a video library. It extracts metadata via FFmpeg/ffprobe, generates thumbnails, and provides a web UI for browsing, tagging, and tracking video usage in productions.

## Development Commands

### Backend (FastAPI + Python)
```bash
# From backend/ directory, with venv activated
cd backend
python -m venv .venv
.venv\Scripts\activate        # Windows
pip install -r requirements.txt

python run.py                 # Start dev server (localhost:8000, auto-reload)
```

### Frontend (Next.js 14)
```bash
cd frontend
npm install
npm run dev                   # Dev server (localhost:3000)
npm run build                 # Production build
npm run lint                  # ESLint
```

### Database Migrations (Alembic)
```bash
cd backend
alembic revision --autogenerate -m "description"
alembic upgrade head
```
Note: Tables are also auto-created via `Base.metadata.create_all()` in `init_db()` on startup. No migration version files exist yet — the project uses direct table creation.

### API Documentation
FastAPI auto-generates docs at `http://localhost:8000/docs` (Swagger) and `/redoc`.

## Architecture

### Backend (backend/app/)

Layered architecture: **Routes → Services → Models/Database**

- **Routes** (`api/routes/`): FastAPI routers for scan, videos, tags, stream, productions. All mounted under `/api` prefix.
- **Services** (`services/`): Business logic — `scanner.py` (background directory scanning), `ffmpeg_service.py` (metadata/thumbnails), `video_service.py`, `search_service.py`, `production_service.py`.
- **Models** (`models/`): SQLAlchemy ORM — Video, Metadata, Tag, VideoTag, Production, VideoProduction.
- **Schemas** (`schemas/`): Pydantic request/response validation.
- **Config** (`config.py`): `ConfigManager` persists settings to `data/config.json`; also reads from environment variables.

### Frontend (frontend/src/)

- **App Router** (`app/`): Pages — main grid (`page.tsx`), setup wizard (`setup/`), settings (`settings/`).
- **Components** (`components/`): VideoGrid, VideoCard, VideoModal (player + metadata editor), FilterPanel, Scanner (progress polling), ProductionManager, BulkActions.
- **API Client** (`lib/api.ts`): Centralized typed fetch functions for all backend endpoints.
- **Types** (`lib/types.ts`): Shared TypeScript interfaces matching backend schemas.

### API Proxying
Next.js rewrites `/api/*` requests to `http://localhost:8000/api/*` (configured in `next.config.js`). The frontend calls `/api/...` paths directly — no separate API URL needed in client code.

### Key Data Flow: Video Scanning
1. Frontend POSTs to `/api/scan` → backend creates a scan ID and spawns a daemon thread
2. Scanner runs two passes: count files, then process each (ffprobe metadata → save to DB → generate thumbnails)
3. Scanner uses `db_factory` (SessionLocal callable) to create its own DB session in the background thread
4. Progress tracked in-memory (`active_scans` dict) — not persistent across restarts
5. Frontend polls `/api/scan/status/{scanId}` every 1 second via the Scanner component

### Database Schema
SQLite at `backend/data/database.db`. Key relationships:
- **Video ↔ Metadata**: 1:1 (cascade delete)
- **Video ↔ Tag**: Many-to-many via `video_tags` junction table
- **Video ↔ Production**: Many-to-many via `video_productions` junction table

### Video Streaming
`stream.py` supports HTTP range requests (206 Partial Content) for seeking — streams file chunks without loading entire video into memory.

## Important Patterns

- **Dependency injection**: Routes use `get_db()` for sessions; scanner uses `db_factory` callable since it runs in a separate thread
- **Eager loading**: Queries use `joinedload()` for Video relationships (metadata, tags, productions)
- **Frontend state**: Local `useState` for UI; `@tanstack/react-query` is installed but component state is primary
- **Styling**: Tailwind CSS throughout, custom blue primary palette
- **Path alias**: `@/*` maps to `frontend/src/*`

## Known Issues

- Pre-existing ESLint errors in `settings/page.tsx` and `setup/page.tsx` (unescaped `'` entities)
- No test suite exists yet (no test files or test runner configured)

## External Dependencies

- **FFmpeg/ffprobe** must be installed and available on PATH for metadata extraction and thumbnail generation
- **Supported video formats**: `.mp4`, `.mov`, `.avi`, `.mkv`, `.webm`, `.flv`, `.wmv`
