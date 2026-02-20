# Video Manager Backend (Rust)

Actix-web backend for the Video Manager application, rewritten in Rust for improved performance.

## Setup

### 1. Install Rust

```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
```

### 2. Install Diesel CLI

```bash
cargo install diesel_cli --no-default-features --features sqlite
```

### 3. Configure Environment

Copy `.env.example` to `.env` and customize:

```bash
cp .env.example .env
```

Key variables:

| Variable             | Default                | Description                        |
|----------------------|------------------------|------------------------------------|
| `HOST`               | `127.0.0.1`            | Server bind address                |
| `PORT`               | `8000`                 | Server port                        |
| `DATABASE_PATH`      | `./data/database.db`   | SQLite database file path          |
| `VIDEO_DIRECTORY`    | _(empty)_              | Directory to scan for videos       |
| `THUMBNAIL_DIRECTORY`| `./data/thumbnails`    | Where generated thumbnails are stored |
| `THUMBNAIL_COUNT`    | `5`                    | Number of thumbnails per video     |
| `THUMBNAIL_WIDTH`    | `320`                  | Thumbnail width in pixels          |

### 4. Run Database Migrations

```bash
diesel migration run
```

### 5. Run Development Server

```bash
cargo run
```

The API will be available at `http://localhost:8000`.

For a release build:

```bash
cargo build --release
./target/release/video-manager-backend
```

## Project Structure

```
backend-rust/
├── src/
│   ├── models/              # Diesel ORM models
│   │   ├── video.rs
│   │   ├── tag.rs
│   │   └── production.rs
│   ├── routes/              # Actix-web route handlers
│   │   ├── videos.rs
│   │   ├── scan.rs
│   │   ├── tags.rs
│   │   ├── productions.rs
│   │   ├── stream.rs
│   │   └── config_routes.rs
│   ├── services/            # Business logic
│   │   ├── scanner.rs
│   │   ├── ffmpeg_service.rs
│   │   ├── video_service.rs
│   │   ├── search_service.rs
│   │   └── production_service.rs
│   ├── config.rs            # Configuration management
│   ├── db.rs                # Database connection pool
│   ├── schema.rs            # Diesel schema (auto-generated)
│   ├── utils.rs             # Utility functions
│   └── main.rs              # Application entry point
├── migrations/              # Diesel migrations
├── data/                    # Runtime data (gitignored)
│   ├── database.db
│   └── thumbnails/
├── Cargo.toml
└── diesel.toml
```

## Database Schema

### Tables

- **videos**: Video files with path, hash, duration, resolution, codec, etc.
- **metadata**: Per-video category, location, notes, and rating
- **tags**: Tag definitions
- **video_tags**: Many-to-many relationship between videos and tags
- **productions**: Named productions/projects
- **video_productions**: Many-to-many relationship between videos and productions

## API Endpoints

- `POST /api/scan` — Start directory scan
- `GET /api/scan/status/{scan_id}` — Get scan progress
- `POST /api/scan/rescan` — Rescan existing library
- `GET /api/videos` — List/search videos
- `GET /api/videos/{id}` — Get video details
- `PUT /api/videos/{id}` — Update video metadata
- `DELETE /api/videos/{id}` — Delete video record
- `POST /api/videos/bulk-update` — Bulk update videos
- `GET /api/thumbnails/{video_id}/{index}` — Get thumbnail
- `GET /api/stream/{video_id}` — Stream video (range request support)
- `GET /api/tags` — List tags
- `POST /api/tags` — Create tag
- `GET /api/productions` — List productions
- `POST /api/productions` — Create production
- `GET /api/config` — Get configuration
- `PUT /api/config` — Update configuration

## Configuration

Configuration is managed through environment variables (`.env` file), stored in `data/config.json` at runtime.

## External Dependencies

- **FFmpeg/ffprobe** must be installed and available on `PATH` for metadata extraction and thumbnail generation
- **Supported video formats**: `.mp4`, `.mov`, `.avi`, `.mkv`, `.webm`, `.flv`, `.wmv`

## Development

### Database Migrations

Create a new migration:

```bash
diesel migration generate description_of_changes
```

Apply migrations:

```bash
diesel migration run
```

Revert last migration:

```bash
diesel migration revert
```
