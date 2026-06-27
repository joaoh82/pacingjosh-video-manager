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
cargo tauri icon images/icon.png  # generate icons from the square (512x512) source

# Dev mode — launches Next.js dev server + Tauri window (run from repo root)
cargo tauri dev

# Production bundle (msi/dmg/deb/appimage)
cargo tauri build
```

The Tauri shell embeds the backend as a library (`video_manager_backend::run_blocking`) on a dedicated OS thread, picks a free port at startup, and injects `window.__VMAN_API__` so the static-exported frontend knows where to find the backend.

> **Worktrees / fresh checkouts (IMPORTANT):** two sets of build inputs under `src-tauri/` are **gitignored** and therefore are **not** copied into a new `git worktree` (or fresh clone), so a Tauri build fails until they're regenerated. **Always run both of these in any new worktree before `cargo tauri dev` / `cargo tauri build`:**
> 1. **FFmpeg sidecars** (`src-tauri/binaries/`) — else `resource path binaries\ffmpeg-...exe doesn't exist`. Fix: `scripts\fetch-ffmpeg.ps1` (Windows) or `bash scripts/fetch-ffmpeg.sh` (macOS/Linux).
> 2. **App icons** (`src-tauri/icons/`) — else `icons/icon.ico not found; required for generating a Windows Resource file`. Fix: `cargo tauri icon images/icon.png`.
>
> The pure backend (`cd backend-rust && cargo run`) and the frontend (`npm run dev`) need **neither** — they use `ffmpeg`/`ffprobe` from `PATH` and don't bundle icons — so web dev mode works in a fresh worktree without either step.

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
- **Routes** (`routes/`): Actix-web handlers — `videos.rs`, `scan.rs`, `tags.rs`, `productions.rs`, `stream.rs`, `config_routes.rs`, `ai.rs` (AI settings + per-video copy generation), `edit.rs` (the Edit & Create Video pipeline, incl. `GET /api/overlays/builtin` listing the bundled overlay snippets). All mounted under `/api`.
- **Services** (`services/`): Business logic — `scanner.rs` (background directory scanning), `ffmpeg_service.rs` (metadata/thumbnails/clip extraction + concat + overlay compositing, with injectable `FfmpegPaths` via `set_ffmpeg_paths()`), `video_service.rs`, `search_service.rs`, `production_service.rs`, `ai_service.rs` (transcription incl. ElevenLabs/OpenAI/Gemini + LLM completion), `edit_service.rs` (background edit pipeline orchestration, in-memory `EditJobMap`), `overlay_service.rs` (the bundled "Subscribe" overlay — a transparent GIF embedded via `include_bytes!` from `assets/overlays/` and written out to `<app-data>/overlays/` on startup; exposes the built-in overlay list).
- **Models** (`models/`): Diesel ORM — `video.rs`, `tag.rs`, `production.rs`, `ai.rs`, `edit.rs`.
- **Schema** (`schema.rs`): Diesel auto-generated schema from migrations.
- **Config** (`config.rs`): `ConfigManager::new(app_data_dir)` — all default paths (database, thumbnails, config.json) are rooted at the provided app-data dir.
- **DB** (`db.rs`): Connection pool setup.

### Frontend (frontend/src/)

- **App Router** (`app/`): Pages — main grid (`page.tsx`), setup wizard (`setup/`), settings (`settings/`).
- **Components** (`components/`): VideoGrid, VideoCard, VideoModal (player + metadata editor + AI content panel), FilterPanel, Scanner (progress polling), ProductionManager, VideoEditPipeline (the Edit & Create Video modal — includes an **Overlays** section: "🔔 Add Subscribe" pulls the built-in snippet from `GET /api/overlays/builtin`, "➕ Add image/GIF…" picks a custom transparent GIF/image; per-overlay position and size, sent as `overlays[]` on start-edit), EditTimeline (interactive CapCut-style video/voice/music timeline built from `edl.timeline` — embeds a preview player fed by `GET /api/edits/{id}/video` whose playback drives a red playhead, with click/drag-the-ruler scrubbing; zoom/scroll. Post-render editing: select clip(s) → inspector to **trim** (drag clip edges or numeric in/out, clamped by `takeDurations`) / **remove** (restore from "Removed" chips) / toggle **voice enhancement**; the layout re-flows live off the pending edits and remaps the speech/music tracks; click a music burst for a Remove/Fade menu; a read-only **Overlays** track shows where snippets landed, and an **Overlays** editor below ("🔔 Add Subscribe" / "➕ Add image/GIF…", per-item position/size) lets you add/change/remove overlays on an already-rendered run — seeded from the saved overlays so they stay sticky; **✨ Ask AI** posts to `POST /api/edits/{id}/ai-edit` and applies the returned plan. Re-render via `TimelineEdits` {clips: ClipEdit[], mute, fade, overlays?}), ThumbnailEditor (canvas thumbnail builder — frame grab + freely-draggable text overlay with X/Y sliders, left/center/right alignment, and a rich text style {fill/gradient/outline/shadow/highlight band}; **✨ AI style text** posts to `POST /api/edits/{id}/text-style` for an LLM-designed treatment, **✨ AI restyle frame** restyles the background still; rehydrates from a saved `ThumbnailSpec` and on save sends the composited PNG + background still + spec to persist), BulkActions.
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
3. The thread runs the stages: extract audio + transcribe each take with word-level timestamps (`ai_service::transcribe_timed`) → build the planning prompt and call the LLM (`ai_service::complete`) → parse/validate the returned EDL against the real takes (clamp ranges to durations) → optionally "tighten" each clip into speech-only sub-clips by dropping long silences/filler (`clip_keep_segments`, driven by word timestamps) → extract a normalized segment per (sub-)clip (burning in per-clip captions re-timed from the transcript when enabled, and optionally cleaning up the audio with a per-take "Enhance voice" filter chain — `ffmpeg_service::voice_enhance_filter` builds `highpass`+`afftdn`+`adeclick`+a `treble` clarity shelf scaled by an intensity 0–1, applied via `extract_clip_segment`'s `audio_filter` arg only to the takes in `EditOptions.enhance_voice_video_ids`) → concat with FFmpeg → optionally mix in looped background music with two explicit levels — a `music_volume` for pauses and a lower `music_duck_volume` while talking. Ducking is driven by the KNOWN speech intervals (transcript word timestamps mapped onto the final timeline), applied as a `volume` expression on the music — deterministic and independent of recording level, so `0` truly silences music during speech (`ffmpeg_service::add_background_music` takes the duck intervals plus optional `fade` regions; the `volume` automation is built by the testable `ffmpeg_service::music_volume_expr`, which ducks inside duck intervals and ramps duck↔full across each fade region) → optionally composite overlay snippets (e.g. a "Subscribe" bug) onto the finished video. Overlays are **transparent GIFs/images** — their native alpha is used (no chroma key). They come from `EditOptions.overlays` (`OverlaySpec`: path, label, `scale`, `opacity`, `position`, optional `duration`/`start`; an optional `chroma_color` keying path remains only for legacy/opaque files). `edit_service::resolve_overlays` resolves each snippet's on-screen duration (probe for GIF/video via ffprobe; a default for still images) and auto-places those without an explicit `start` in the longest non-speech gaps (`non_speech_regions` + the pure, testable `auto_overlay_starts`, largest gaps first, centered when they fit); `ffmpeg_service::composite_overlays` loops the snippet for its window (GIFs via `-ignore_loop 0`, still images via `-loop 1`), time-shifts it to its start, and lays it on with `enable='between(...)'` (graph built by the testable `ffmpeg_service::build_overlay_filter`, which keeps the alpha via `format=rgba` and only colorkeys when `chroma_color` is set). Compositing is best-effort — a failure keeps the un-overlaid video. Placements are echoed into the timeline JSON (`overlays` array — carrying both the display fields AND the full spec: path + scale/position) so the timeline editor can rehydrate, add to, change, or remove them; they're auto re-placed on every re-render. All output goes to a per-run version folder `<output_dir>/productions/v<N>/` (N = max existing `v*` + 1, so re-edits never overwrite; the chosen `output_dir` is used verbatim so nothing nests): the final `.mp4`, its `.json` EDL (same basename), and a transient `.work-<job>/` removed on success. **Nothing is written to the app-data directory** — `output_dir` is required. Deleting a run (`DELETE /api/edits/{id}`) removes the DB row and the files, plus the now-empty version folder. The run also persists `transcripts_json` + `options_json` so `POST /api/edits/{id}/rerender` can re-cut a NEW version without re-transcribing or re-planning — it reuses the saved cut via the shared `assemble_final`. The rerender body carries `clips` (per-clip `ClipEdit`s — `remove`, `source_start`/`source_end` re-trim via the testable `apply_clip_edit` clamped to the take duration, `enhance`), `mute` (music regions ducked away → merged into the duck intervals), `fade` (music regions ramped in/out → passed through to `add_background_music`), and an optional `overlays` array (omitted → keep the run's saved overlays; present → replace them, so overlays can be added/changed/removed on the timeline without re-transcribing). Removed/trimmed clips drop or narrow the reconstructed `cut_list`, so the new EDL/timeline (and thus subsequent edits) are naturally sticky. The applied `mute`/`fade` regions are also persisted on the new `timeline` JSON (`muted`/`fades` arrays); the frontend re-seeds them on reopen and re-sends them (remapped through any clip edits) so music removals/fades stay sticky across re-renders too. Voice enhancement is per-clip: `assemble_final`/`build_edl_json`/`build_timeline` take a `Vec<bool>` aligned to the cut (true → apply `voice_enhance_filter`), and re-render computes those flags from the saved EDL's per-clip `enhanced` field (sticky) ∪ take-level `enhance_voice_video_ids` ∪ each `ClipEdit.enhance` ∪ the legacy `enhance_clips`. `POST /api/edits/{id}/ai-edit` (`edit_service::plan_timeline_edits`) shows the LLM the saved cut (per-clip take/range/spoken text) + music regions and a natural-language instruction, then returns a validated plan (`parse_timeline_edit_plan` drops unknown clip orders, clamps ranges) of clip + music edits for the timeline to apply before re-rendering — no transcription cost. `POST /api/edits/{id}/copy` generates long-form YouTube copy (3 SEO titles, description, tags, thumbnail text) from the final cut's transcript (`final_transcript_for_edit` + `ai_service::generate_youtube_copy`), persisted in the row's `copy_json`. Thumbnail builder: `GET /api/edits/{id}/frame?t=<sec>` returns a 1280x720 still (`ffmpeg_service::extract_frame`); `POST /api/edits/{id}/restyle` AI-restyles that still via the configured image provider/model (`ai_service::restyle_image` dispatches to Gemini `generateContent` or OpenAI `/images/edits`; provider/model set by `AiSettings.image_provider`/`image_model` in Settings, default `gemini`/`gemini-2.5-flash-image`) keeping text out of the image; `POST /api/edits/{id}/text-style` asks the **text** LLM (`ai_service::generate_text_style` → `complete`) for an eye-catching caption treatment, returning a normalized/clamped style object (fill, optional gradient, outline, shadow, optional highlight band) via the testable `parse_text_style`/`build_style` — the text stays a real overlay, only the *style* is generated. `POST /api/edits/{id}/thumbnail` saves the composited PNG **and** the background still next to the video (`edit_service::thumbnail_file_paths` → `<stem>-thumbnail.png` + `<stem>-thumbnail-bg.png`) and persists the builder state (`edit_service::save_thumbnail_spec` → `production_edits.thumbnail_json`, migration `008`); `GET /api/edits/{id}/thumbnail-bg` serves the saved background so the editor rebuilds the exact (possibly restyled) thumbnail on reopen. The `ThumbnailEditor` component composites the background + draggable, aligned, styled text on an HTML canvas (text stays a real overlay for accuracy), rehydrates from the persisted `ThumbnailSpec` (`ProductionEditResponse.thumbnail`), and exports/saves the PNG. `delete_edit` also removes the thumbnail sidecar files so the version folder still empties.
4. The result (EDL + output path + activity log) is persisted to the `production_edits` table; the frontend polls `/api/edit/status/{job_id}` (~1.5s) via the VideoEditPipeline modal. `EditOptions` carries the per-run inputs (script, instructions, output dir/name, captions flag, tighten flag/gap, per-take voice-enhance ids + intensity, music path/volume). On reopen, `/api/productions/{id}/edit` returns the latest result and `/api/productions/{id}/edits` returns the full history (script, EDL, logs per run)
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

- `npm run lint` passes but emits warnings (`react-hooks/exhaustive-deps`, `@next/next/no-img-element`); these are non-blocking in CI
- The Rust code is not fully `rustfmt`-clean, so the CI `cargo fmt --check` step is advisory (non-blocking); Clippy and tests are the hard gates
- No test suite exists yet

## CI / Releases

Releases are **tag-driven and manually triggered** — merging to `main` never releases on its own.

- **`.github/workflows/ci.yml`** — runs on PRs to `main`: frontend lint + build, backend Clippy + tests.
- **`.github/workflows/tag-release.yml`** — manual `workflow_dispatch`. Derives the next version from Conventional Commits since the last tag (`mathieudutour/github-tag-action`, dry-run), bumps all manifests via `scripts/bump-version.mjs`, commits to `main`, pushes the `vX.Y.Z` tag, then calls `release.yml` via `workflow_call`. A `default_bump` input forces a bump when no conventional commits exist.
- **`.github/workflows/release.yml`** — builds + publishes Win/macOS(arm64)/Linux installers via `tauri-apps/tauri-action` for a tag. Triggers: `push` of a `v*.*.*` tag, `workflow_call` (the normal path, from tag-release), or `workflow_dispatch` to rebuild an existing tag. Fetches FFmpeg sidecars with `scripts/fetch-ffmpeg-ci.mjs`; regenerates icons from the square `images/icon.png`.
- The automated path uses `workflow_call` (not a PAT) because a tag pushed by `GITHUB_TOKEN` can't trigger `push: tags`. A `v1.0.0` baseline tag exists for versioning to bump from.

## External Dependencies

- **FFmpeg/ffprobe** must be installed and available on PATH for metadata extraction and thumbnail generation
- **Rust** 1.75+ and **Diesel CLI** (sqlite feature) required for backend development
- **Supported video formats**: `.mp4`, `.mov`, `.avi`, `.mkv`, `.webm`, `.flv`, `.wmv`
