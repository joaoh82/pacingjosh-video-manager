# Video Manager - System Architecture

This document provides a comprehensive overview of the Video Manager application architecture, design decisions, and component interactions.

## 🏛️ High-Level Architecture

```
┌─────────────────────────────────────────────────────────────┐
│                         Frontend                             │
│                    (Next.js + React)                         │
│  ┌──────────────────────────────────────────────────────┐  │
│  │  Components                                           │  │
│  │  ├─ VideoCard    ├─ VideoGrid    ├─ VideoModal     │  │
│  │  ├─ SearchBar    ├─ FilterPanel  ├─ BulkActions    │  │
│  │  └─ Scanner                                          │  │
│  └──────────────────────────────────────────────────────┘  │
│  ┌──────────────────────────────────────────────────────┐  │
│  │  State Management                                     │  │
│  │  ├─ React State  ├─ TanStack Query                  │  │
│  └──────────────────────────────────────────────────────┘  │
│  ┌──────────────────────────────────────────────────────┐  │
│  │  API Client (lib/api.ts)                             │  │
│  └──────────────────────────────────────────────────────┘  │
└─────────────────────────────────────────────────────────────┘
                            │
                      HTTP/REST API
                            │
┌─────────────────────────────────────────────────────────────┐
│                         Backend                              │
│                      (FastAPI + Python)                      │
│  ┌──────────────────────────────────────────────────────┐  │
│  │  API Routes                                           │  │
│  │  ├─ /api/scan      ├─ /api/videos                   │  │
│  │  ├─ /api/tags      ├─ /api/stream                   │  │
│  └──────────────────────────────────────────────────────┘  │
│  ┌──────────────────────────────────────────────────────┐  │
│  │  Services (Business Logic)                           │  │
│  │  ├─ VideoService   ├─ SearchService                 │  │
│  │  ├─ ScanService    ├─ FFmpegService                 │  │
│  └──────────────────────────────────────────────────────┘  │
│  ┌──────────────────────────────────────────────────────┐  │
│  │  Database Layer (SQLAlchemy)                         │  │
│  │  ├─ Models         ├─ Schemas                       │  │
│  └──────────────────────────────────────────────────────┘  │
└─────────────────────────────────────────────────────────────┘
                            │
                    ┌───────┴────────┐
                    │                │
                    ▼                ▼
              ┌──────────┐    ┌──────────┐
              │  SQLite  │    │  FFmpeg  │
              │ Database │    │  Binary  │
              └──────────┘    └──────────┘
                    │
                    ▼
              ┌──────────┐
              │  Video   │
              │  Files   │
              └──────────┘
```

## 🗂️ Backend Architecture

### Layered Architecture Pattern

The backend follows a clean layered architecture:

```
┌─────────────────────────────────────────────┐
│          API Layer (FastAPI Routes)         │
│  - Request validation (Pydantic)            │
│  - Response serialization                   │
│  - Error handling                           │
│  - CORS configuration                       │
└─────────────────────────────────────────────┘
                    │
                    ▼
┌─────────────────────────────────────────────┐
│         Service Layer (Business Logic)      │
│  - VideoService: CRUD operations            │
│  - SearchService: Search and filtering      │
│  - ScannerService: Directory scanning       │
│  - FFmpegService: Video processing          │
└─────────────────────────────────────────────┘
                    │
                    ▼
┌─────────────────────────────────────────────┐
│         Data Layer (SQLAlchemy ORM)         │
│  - Models: Database tables                  │
│  - Schemas: Request/Response validation     │
│  - Migrations: Alembic                      │
└─────────────────────────────────────────────┘
                    │
                    ▼
┌─────────────────────────────────────────────┐
│            SQLite Database                  │
└─────────────────────────────────────────────┘
```

### Key Design Patterns

**1. Repository Pattern**
- Services abstract database operations
- Enables easy testing and mocking
- Separates business logic from data access

**2. Dependency Injection**
- FastAPI's dependency system for database sessions
- Clean separation of concerns
- Easier testing

**3. Service Layer Pattern**
- Business logic isolated in services
- Reusable across multiple routes
- Single responsibility principle

## 📊 Database Schema

### Entity-Relationship Diagram

```
┌──────────────────┐
│     videos       │
├──────────────────┤
│ id (PK)          │
│ file_path (UQ)   │
│ filename         │
│ duration         │
│ file_size        │
│ resolution       │
│ fps              │
│ codec            │
│ created_date     │
│ indexed_date     │
│ thumbnail_count  │
└──────────────────┘
        │
        │ 1:1
        ▼
┌──────────────────┐
│    metadata      │
├──────────────────┤
│ id (PK)          │
│ video_id (FK)    │
│ category         │
│ location         │
│ notes            │
└──────────────────┘

┌──────────────────┐
│     videos       │
└──────────────────┘
        │
        │ N:M
        ▼
┌──────────────────┐
│   video_tags     │
├──────────────────┤
│ video_id (FK)    │
│ tag_id (FK)      │
└──────────────────┘
        │
        ▼
┌──────────────────┐
│      tags        │
├──────────────────┤
│ id (PK)          │
│ name (UQ)        │
└──────────────────┘
```

### Indexes

```sql
-- Performance indexes
CREATE INDEX idx_videos_created_date ON videos(created_date);
CREATE INDEX idx_videos_filename ON videos(filename);
CREATE INDEX idx_metadata_category ON metadata(category);
CREATE INDEX idx_tags_name ON tags(name);
CREATE INDEX idx_video_tags_video ON video_tags(video_id);
CREATE INDEX idx_video_tags_tag ON video_tags(tag_id);
```

## 🎨 Frontend Architecture

### Component Hierarchy

```
App (layout.tsx)
└── Home Page (page.tsx)
    ├── Header
    │   └── SearchBar
    ├── Sidebar
    │   └── FilterPanel
    ├── Main Content
    │   ├── VideoGrid
    │   │   └── VideoCard (multiple)
    │   └── Pagination
    ├── VideoModal (conditional)
    └── BulkActions (conditional)

Setup Page (setup/page.tsx)
└── Scanner
```

### State Management Strategy

**Local State (useState)**
- Component-specific UI state
- Form inputs
- Modal visibility
- Selection state

**Server State (TanStack Query)**
- Video data
- Categories and tags
- Statistics
- Automatic caching and refetching

**URL State (Next.js Router)**
- Navigation
- Deep linking support

### Data Flow

```
User Action
    │
    ▼
Component Event Handler
    │
    ▼
API Client Function (lib/api.ts)
    │
    ▼
HTTP Request
    │
    ▼
Backend API
    │
    ▼
HTTP Response
    │
    ▼
State Update
    │
    ▼
React Re-render
```

## 🔄 Key Workflows

### 1. Video Scanning Workflow

```
1. User inputs directory path
   │
   ▼
2. POST /api/scan
   │
   ▼
3. Validate directory
   │
   ▼
4. Create scan progress tracker
   │
   ▼
5. For each video file:
   ├─ Check if already indexed
   ├─ Extract metadata (FFmpeg)
   ├─ Generate thumbnails
   ├─ Create database record
   └─ Update progress
   │
   ▼
6. Complete scan
   │
   ▼
7. Frontend polls GET /api/scan/status/{id}
   │
   ▼
8. Display real-time progress
```

### 2. Search & Filter Workflow

```
1. User enters search term or selects filter
   │
   ▼
2. Debounced state update (300ms)
   │
   ▼
3. GET /api/videos with query params
   │
   ▼
4. Backend builds SQL query:
   ├─ Search: LIKE on filename/location/notes
   ├─ Category: Exact match
   ├─ Tags: Subquery for each tag (AND logic)
   ├─ Date: Range filter
   └─ Sort: ORDER BY
   │
   ▼
5. Execute query with pagination
   │
   ▼
6. Return results
   │
   ▼
7. Frontend updates video grid
```

### 3. Video Streaming Workflow

```
1. User clicks video card
   │
   ▼
2. VideoModal opens
   │
   ▼
3. Video player loads: GET /api/stream/{id}
   │
   ▼
4. Backend:
   ├─ Verify video exists
   ├─ Check file access
   └─ Parse Range header
   │
   ▼
5. Stream video with Range support:
   ├─ Full request: Return entire file
   └─ Range request: Return partial content (206)
   │
   ▼
6. Video plays in browser
```

### 4. Bulk Update Workflow

```
1. User selects multiple videos
   │
   ▼
2. BulkActions toolbar appears
   │
   ▼
3. User fills bulk edit form
   │
   ▼
4. POST /api/videos/bulk-update
   │
   ▼
5. Backend:
   ├─ Load all selected videos
   ├─ Update metadata fields
   ├─ Add/remove tags
   └─ Commit transaction
   │
   ▼
6. Return updated count
   │
   ▼
7. Frontend refreshes video list
```

## 🔐 Security Considerations

### Path Traversal Prevention

```python
def is_path_safe(file_path: str, base_directory: str) -> bool:
    """Prevent path traversal attacks"""
    base = Path(base_directory).resolve()
    target = Path(file_path).resolve()
    return target.is_relative_to(base)
```

### Input Validation

- Pydantic schemas validate all API inputs
- SQL injection prevented by SQLAlchemy ORM
- File paths sanitized before access

### CORS Configuration

- Restricted to localhost by default
- Configurable via settings

## ⚡ Performance Optimizations

### Backend

**Database**
- Proper indexes on frequently queried columns
- Eager loading with joinedload for relationships
- Pagination to limit result sets

**Video Streaming**
- Range request support for efficient seeking
- File streaming (no full load into memory)
- Thumbnail caching

**Scanning**
- Background tasks for non-blocking scans
- Progress tracking without blocking
- Skip already indexed files

### Frontend

**React Optimizations**
- Debounced search input (300ms)
- Lazy loading for video grid
- Memoization where appropriate
- Virtual scrolling (future enhancement)

**Network**
- API response caching
- Thumbnail lazy loading
- Optimized image formats

**Bundle**
- Code splitting by route
- Tree shaking unused code
- SWC minification

## 🧪 Testing Strategy

### Backend Tests

```python
# Unit Tests
- FFmpeg service (mocked subprocess)
- File utilities
- Search logic

# Integration Tests
- API endpoints
- Database operations
- Scanner with fixture videos
```

### Frontend Tests

```typescript
// Component Tests
- React Testing Library
- User interaction simulation
- Accessibility checks

// E2E Tests (Optional)
- Playwright
- Full user workflows
- Cross-browser testing
```

## 🚀 Deployment Considerations

### Development
- SQLite database
- Local file storage
- Hot reload enabled

### Production
- Consider PostgreSQL for better concurrency
- Shared storage for thumbnails
- Process manager (PM2, systemd)
- Nginx reverse proxy
- SSL/TLS certificates

### Scaling

**Horizontal Scaling**
- Multiple backend instances
- Load balancer
- Shared database
- Centralized file storage

**Vertical Scaling**
- Increase server resources
- Database optimization
- Caching layer (Redis)

## 📈 Future Enhancements

### Architecture Improvements

1. **Microservices**
   - Separate scanning service
   - Video processing service
   - Search service with Elasticsearch

2. **Message Queue**
   - RabbitMQ/Celery for async tasks
   - Better progress tracking
   - Retry mechanisms

3. **Caching Layer**
   - Redis for API responses
   - Thumbnail CDN
   - Session management

4. **Advanced Search**
   - Elasticsearch integration
   - Full-text search improvements
   - Faceted search

5. **Real-time Updates**
   - WebSocket support
   - Live scan updates
   - Collaborative features

## 📚 References

- [FastAPI Documentation](https://fastapi.tiangolo.com/)
- [Next.js Documentation](https://nextjs.org/docs)
- [SQLAlchemy Documentation](https://docs.sqlalchemy.org/)
- [FFmpeg Documentation](https://ffmpeg.org/documentation.html)
- [React Documentation](https://react.dev/)

---

**Last Updated:** 2024
