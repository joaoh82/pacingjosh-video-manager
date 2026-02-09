# PacingJosh Video Manager 🎬

A modern, full-stack video indexing and management application for organizing and browsing large video collections. Built for runners, content creators, and anyone managing thousands of videos locally.

![Python](https://img.shields.io/badge/Python-3.11+-blue.svg)
![FastAPI](https://img.shields.io/badge/FastAPI-0.109.0-green.svg)
![Next.js](https://img.shields.io/badge/Next.js-14.2.0-black.svg)
![TypeScript](https://img.shields.io/badge/TypeScript-5.3.3-blue.svg)
![License](https://img.shields.io/badge/License-MIT-green.svg)

![Video Manager](images/main-screen.png)

## 🎯 Features

### Video Management
- 📁 **Recursive Directory Scanning** - Automatically index videos from any directory structure
- 🖼️ **Thumbnail Generation** - Auto-generate multiple thumbnails per video for quick preview
- 🎬 **Video Streaming** - Built-in video player with seeking support
- 📊 **Metadata Extraction** - Extract duration, resolution, FPS, codec, and more using FFmpeg

### Organization & Search
- 🔍 **Full-Text Search** - Search across filenames, locations, and notes
- 🏷️ **Tagging System** - Multi-tag support with tag management
- 📂 **Categories** - Organize videos into categories
- 📅 **Date Filtering** - Filter by date range
- 📝 **Notes** - Add detailed notes to each video

### Advanced Features
- ✏️ **Metadata Editing** - Edit all metadata inline
- 📦 **Bulk Operations** - Update multiple videos at once
- 🔄 **Real-Time Progress** - Live scanning progress with detailed status
- 🎨 **Modern UI** - Clean, responsive interface with dark mode
- 💾 **SQLite Database** - Fast, reliable local storage

## 🏗️ Architecture

**Backend:**
- FastAPI (Python) - REST API
- SQLite - Database
- SQLAlchemy - ORM
- FFmpeg - Video processing
- Alembic - Migrations

**Frontend:**
- Next.js 14 (App Router)
- React 18
- TypeScript
- Tailwind CSS
- TanStack Query

## 📋 Prerequisites

### Required Software

| Software | Minimum Version | Installation |
|----------|----------------|--------------|
| Python | 3.11+ | [python.org](https://www.python.org/downloads/) |
| Node.js | 18.0+ | [nodejs.org](https://nodejs.org/) |
| FFmpeg | Latest | See OS-specific instructions below |

### Installing FFmpeg

<details>
<summary><b>macOS</b></summary>

```bash
# Using Homebrew
brew install ffmpeg

# Verify installation
ffmpeg -version
```
</details>

<details>
<summary><b>Ubuntu/Debian Linux</b></summary>

```bash
# Install FFmpeg
sudo apt update
sudo apt install ffmpeg

# Verify installation
ffmpeg -version
```
</details>

<details>
<summary><b>Windows</b></summary>

1. Download FFmpeg from [ffmpeg.org](https://ffmpeg.org/download.html#build-windows)
2. Extract to `C:\ffmpeg`
3. Add `C:\ffmpeg\bin` to your system PATH
4. Open a new Command Prompt and verify:
```cmd
ffmpeg -version
```
</details>

### System Requirements

- **OS**: macOS 10.15+, Ubuntu 20.04+, Windows 10+
- **RAM**: 2GB minimum (4GB+ recommended)
- **Disk Space**:
  - Application: ~50MB
  - Database: ~1-2MB per 1000 videos
  - Thumbnails: ~50-100KB per video

## 🚀 Quick Start

### macOS / Linux

```bash
# 1. Clone the repository
git clone https://github.com/joaoh82/pacingjosh-video-manager.git
cd pacingjosh-video-manager

# 2. Backend setup
cd backend
python3 -m venv venv
source venv/bin/activate
pip install -r requirements.txt

# 3. Frontend setup (in new terminal)
cd frontend
npm install
cp .env.example .env.local

# 4. Run backend (Terminal 1)
cd backend && source venv/bin/activate && python run.py

# 5. Run frontend (Terminal 2)
cd frontend && npm run dev
```

### Windows

```powershell
# 1. Clone the repository
git clone https://github.com/joaoh82/pacingjosh-video-manager.git
cd pacingjosh-video-manager

# 2. Backend setup
cd backend
python -m venv venv
.\venv\Scripts\activate
pip install -r requirements.txt

# 3. Frontend setup (in new PowerShell window)
cd frontend
npm install
copy .env.example .env.local

# 4. Run backend (PowerShell 1)
cd backend
.\venv\Scripts\activate
python run.py

# 5. Run frontend (PowerShell 2)
cd frontend
npm run dev
```

### First-Time Setup

1. Open **http://localhost:3000** in your browser
2. You'll be redirected to the setup page
3. Click **"📁 Browse..."** to select your video directory (or type the path)
4. Click **"Start Scanning"**
5. Wait for the scan to complete
6. Start browsing your videos! 🎉

## 📖 Usage

### Scanning Videos

**Initial Scan:**
- Navigate to http://localhost:3000
- Enter your video directory path
- The application will recursively scan all subdirectories
- Supported formats: .mp4, .mov, .avi, .mkv, .webm, .flv, .wmv

**Rescanning:**
- Use the backend API: `POST /api/scan/rescan`
- Or manually trigger from the frontend

### Searching and Filtering

**Search:**
- Use the search bar to find videos by filename, location, or notes
- Search is debounced for performance

**Filters:**
- **Category** - Filter by video category
- **Tags** - Select multiple tags (AND logic)
- **Date Range** - Filter by creation date
- **Sort** - 8 different sorting options

### Editing Metadata

**Single Video:**
1. Click on any video card
2. Click **"Edit"** in the modal
3. Update category, location, tags, or notes
4. Click **"Save"**

**Bulk Edit:**
1. Select multiple videos (checkboxes)
2. Click **"Bulk Edit"** in the bottom toolbar
3. Set category, add/remove tags
4. Click **"Apply Changes"**

### Watching Videos

- Click any video card to open the modal
- Use the built-in HTML5 video player
- Seeking and playback controls included
- Videos stream directly from your local files

## 🎨 Screenshots

### Main Video Grid
![Video Grid](images/video-grid.png)
*Browse your video collection with thumbnails and metadata*

### Video Player Modal
![Video Player](images/video-modal.png)
*Watch videos and edit metadata in a sleek modal interface*

### Settings Page
![Settings](images/settings.png)
*Configure video directory and thumbnail preferences*

> **Note**: Screenshots show the application with sample video data. Your interface will look similar with your own videos.

## 🔧 Configuration

### Backend Configuration

Edit `backend/app/config.py` or use environment variables:

```python
# Database
DATABASE_URL=sqlite:///./data/database.db

# Video Settings
VIDEO_DIRECTORY=/path/to/videos
SUPPORTED_FORMATS=[".mp4", ".mov", ".avi", ".mkv", ".webm", ".flv", ".wmv"]

# Thumbnail Settings
THUMBNAIL_DIRECTORY=./data/thumbnails
THUMBNAIL_COUNT=5
THUMBNAIL_WIDTH=320

# Server
HOST=127.0.0.1
PORT=8000
```

### Frontend Configuration

Edit `frontend/.env.local`:

```bash
NEXT_PUBLIC_API_URL=http://localhost:8000/api
```

## 📚 API Documentation

Once the backend is running, visit:
- **Interactive Docs**: http://localhost:8000/docs
- **Alternative Docs**: http://localhost:8000/redoc

### Key Endpoints

```
POST   /api/scan              - Start directory scan
GET    /api/scan/status/{id}  - Get scan progress
GET    /api/videos            - List/search videos
GET    /api/videos/{id}       - Get video details
PUT    /api/videos/{id}       - Update video
POST   /api/videos/bulk-update - Bulk update
GET    /api/tags              - List all tags
GET    /api/tags/categories   - List all categories
GET    /api/stream/{id}       - Stream video
GET    /api/thumbnails/{id}/{index} - Get thumbnail
```

## 🗄️ Database Schema

### Tables

**videos**
- id, file_path, filename
- duration, file_size, resolution, fps, codec
- created_date, indexed_date, thumbnail_count

**metadata**
- id, video_id, category, location, notes

**tags**
- id, name

**video_tags**
- video_id, tag_id (junction table)

## 🛠️ Development

### Project Structure

```
video_manager/
├── backend/
│   ├── app/
│   │   ├── api/routes/     # API endpoints
│   │   ├── models/         # SQLAlchemy models
│   │   ├── schemas/        # Pydantic schemas
│   │   ├── services/       # Business logic
│   │   ├── utils/          # Utilities
│   │   ├── config.py       # Configuration
│   │   ├── database.py     # Database setup
│   │   └── main.py         # FastAPI app
│   ├── alembic/            # Database migrations
│   └── tests/              # Tests
├── frontend/
│   └── src/
│       ├── app/            # Next.js pages
│       ├── components/     # React components
│       ├── lib/            # API client & types
│       └── styles/         # Global styles
└── data/
    ├── database.db         # SQLite database
    └── thumbnails/         # Generated thumbnails
```

### Running Tests

**Backend:**
```bash
cd backend
pytest tests/
```

**Frontend:**
```bash
cd frontend
npm test
```

### Database Migrations

**Create a new migration:**
```bash
cd backend
alembic revision --autogenerate -m "Description"
```

**Apply migrations:**
```bash
alembic upgrade head
```

**Rollback:**
```bash
alembic downgrade -1
```

## 🐛 Troubleshooting

### FFmpeg Not Found
**Error:** `FFmpeg is not installed or not in PATH`

**Solution:**
- Install FFmpeg (see Prerequisites)
- Verify installation: `ffmpeg -version`
- Add FFmpeg to your PATH

### Port Already in Use
**Error:** `Address already in use`

**Solution:**
```bash
# Find and kill process
lsof -ti:8000 | xargs kill  # Backend
lsof -ti:3000 | xargs kill  # Frontend

# Or use different ports
# Backend: python run.py (edit port in config.py)
# Frontend: npm run dev -- -p 3001
```

### Database Locked
**Error:** `database is locked`

**Solution:**
- Close any SQLite browser tools
- Restart the backend server
- Check file permissions on `data/database.db`

### Videos Not Showing
**Issue:** Scan completed but no videos appear

**Solution:**
1. Check if videos are in supported formats
2. Verify file permissions
3. Check backend logs for errors
4. Try rescanning: `POST /api/scan/rescan`

### Thumbnails Not Loading
**Issue:** Video cards show placeholder icon

**Solution:**
1. Check `data/thumbnails/` directory exists
2. Verify FFmpeg can read your video files
3. Check browser console for 404 errors
4. Rescan to regenerate thumbnails

## 🤝 Contributing

Contributions are welcome! Please feel free to submit a Pull Request.

### Development Workflow

1. Fork the repository
2. Create a feature branch (`git checkout -b feature/amazing-feature`)
3. Commit your changes (`git commit -m 'Add amazing feature'`)
4. Push to the branch (`git push origin feature/amazing-feature`)
5. Open a Pull Request

## 📝 License

This project is licensed under the MIT License - see the LICENSE file for details.

## 🙏 Acknowledgments

- **FastAPI** - Modern Python web framework
- **Next.js** - React framework
- **FFmpeg** - Video processing
- **SQLAlchemy** - SQL toolkit
- **Tailwind CSS** - Utility-first CSS

## 📧 Support

For issues, questions, or suggestions:
- Open an issue on GitHub
- Check existing documentation
- Review troubleshooting guide

## 🔮 Roadmap

Future enhancements planned:

- [ ] Playlist management
- [ ] Video analytics and statistics dashboard
- [ ] Export/import functionality
- [ ] Mobile app companion
- [ ] Cloud storage integration
- [ ] Advanced AI features (face detection, transcription)
- [ ] Collaborative features and sharing

## 🤝 Contributing

Contributions, issues, and feature requests are welcome!

1. Fork the repository
2. Create your feature branch (`git checkout -b feature/AmazingFeature`)
3. Commit your changes (`git commit -m 'Add some AmazingFeature'`)
4. Push to the branch (`git push origin feature/AmazingFeature`)
5. Open a Pull Request

## 📄 License

This project is licensed under the MIT License - see the [LICENSE](LICENSE) file for details.

## 🙏 Acknowledgments

- Built with [FastAPI](https://fastapi.tiangolo.com/)
- Powered by [FFmpeg](https://ffmpeg.org/)
- UI framework by [Next.js](https://nextjs.org/)
- Styled with [Tailwind CSS](https://tailwindcss.com/)

## 📧 Contact

**João Henrique Machado Silva** - [@joaoh82](https://github.com/joaoh82)

Project Link: [https://github.com/joaoh82/pacingjosh-video-manager](https://github.com/joaoh82/pacingjosh-video-manager)

---

<div align="center">

**Made for runners who love tracking their journey** 🏃‍♂️

*If you find this project helpful, please consider giving it a ⭐️*

</div>
