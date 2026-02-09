# Video Manager Backend

FastAPI backend for the Video Manager application.

## Setup

### 1. Create Virtual Environment

```bash
python -m venv venv
source venv/bin/activate  # On Windows: venv\Scripts\activate
```

### 2. Install Dependencies

```bash
pip install -r requirements.txt
```

### 3. Configure Environment

Copy `.env.example` to `.env` and customize:

```bash
cp .env.example .env
```

### 4. Initialize Database

Create the initial migration:

```bash
cd backend
alembic revision --autogenerate -m "Initial migration"
alembic upgrade head
```

Or use the database initialization in code (creates tables automatically):

```python
from app.database import init_db
init_db()
```

### 5. Run Development Server

```bash
uvicorn app.main:app --reload --port 8000
```

The API will be available at:
- API: http://localhost:8000
- Interactive docs: http://localhost:8000/docs
- Alternative docs: http://localhost:8000/redoc

## Project Structure

```
backend/
├── app/
│   ├── models/          # SQLAlchemy models
│   ├── schemas/         # Pydantic schemas
│   ├── api/
│   │   └── routes/      # API endpoints
│   ├── services/        # Business logic
│   ├── utils/           # Utility functions
│   ├── config.py        # Configuration management
│   ├── database.py      # Database connection
│   └── main.py          # FastAPI application
├── alembic/             # Database migrations
├── tests/               # Test files
└── requirements.txt     # Python dependencies
```

## Database Schema

### Tables

- **videos**: Video files with metadata (duration, resolution, etc.)
- **metadata**: Additional metadata (category, location, notes)
- **tags**: Tag definitions
- **video_tags**: Many-to-many relationship between videos and tags

## API Endpoints (Coming in Phase 3)

- `POST /api/scan` - Start directory scan
- `GET /api/scan/status/{scan_id}` - Get scan progress
- `GET /api/videos` - List/search videos
- `GET /api/videos/{id}` - Get video details
- `PUT /api/videos/{id}` - Update video metadata
- `POST /api/videos/bulk-update` - Bulk update videos
- `GET /api/thumbnails/{video_id}/{index}` - Get thumbnail
- `GET /api/stream/{video_id}` - Stream video
- `GET /api/categories` - List categories
- `GET /api/tags` - List tags

## Configuration

Configuration is managed through:
1. Environment variables (`.env` file)
2. Configuration file (`data/config.json`)
3. Default values in `app/config.py`

The `ConfigManager` class handles loading and saving configuration.

## Development

### Running Tests

```bash
pytest tests/
```

### Database Migrations

Create a new migration:

```bash
alembic revision --autogenerate -m "Description of changes"
```

Apply migrations:

```bash
alembic upgrade head
```

Rollback migration:

```bash
alembic downgrade -1
```

## Next Steps

Phase 1 ✅ Complete:
- [x] Backend structure
- [x] Configuration management
- [x] Database setup
- [x] SQLAlchemy models
- [x] Pydantic schemas
- [x] Alembic configuration

Phase 2: FFmpeg Integration & Scanning Service
Phase 3: API Endpoints
