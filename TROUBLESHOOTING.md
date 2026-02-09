# Troubleshooting Guide

This guide covers common issues and their solutions for the Video Manager application.

## 📋 Table of Contents

- [Installation Issues](#installation-issues)
- [FFmpeg Issues](#ffmpeg-issues)
- [Backend Issues](#backend-issues)
- [Frontend Issues](#frontend-issues)
- [Database Issues](#database-issues)
- [Video Issues](#video-issues)
- [Performance Issues](#performance-issues)
- [Network Issues](#network-issues)

---

## 🔧 Installation Issues

### Python Version Incompatibility

**Error:** `ERROR: Python 3.9 is not supported`

**Cause:** Application requires Python 3.10 or higher

**Solution:**
```bash
# Check Python version
python --version

# Install Python 3.10+
# macOS
brew install python@3.10

# Ubuntu
sudo apt install python3.10

# Or use pyenv
pyenv install 3.10.0
pyenv global 3.10.0
```

### Node.js Version Incompatibility

**Error:** `The engine "node" is incompatible`

**Cause:** Application requires Node.js 18 or higher

**Solution:**
```bash
# Check Node version
node --version

# Install Node 18+
# Using nvm
nvm install 18
nvm use 18

# Or download from https://nodejs.org/
```

### Package Installation Fails

**Error:** `pip install` or `npm install` fails

**Solutions:**

1. **Clear cache:**
```bash
# Python
pip cache purge
pip install -r requirements.txt

# Node
npm cache clean --force
rm -rf node_modules package-lock.json
npm install
```

2. **Use virtual environment:**
```bash
cd backend
python -m venv venv
source venv/bin/activate
pip install --upgrade pip
pip install -r requirements.txt
```

3. **Check network:**
```bash
# Test connectivity
curl https://pypi.org
curl https://registry.npmjs.org
```

---

## 🎬 FFmpeg Issues

### FFmpeg Not Found

**Error:** `FFmpeg is not installed or not in PATH`

**Cause:** FFmpeg not installed or not accessible

**Solutions:**

1. **Install FFmpeg:**
```bash
# macOS
brew install ffmpeg

# Ubuntu/Debian
sudo apt update
sudo apt install ffmpeg

# Windows
# Download from https://ffmpeg.org/download.html
# Add to PATH
```

2. **Verify installation:**
```bash
ffmpeg -version
ffprobe -version
```

3. **Add to PATH (if installed but not found):**
```bash
# macOS/Linux
export PATH="/usr/local/bin:$PATH"

# Windows
# Add FFmpeg bin directory to System PATH in Environment Variables
```

### FFmpeg Permission Denied

**Error:** `Permission denied` when running FFmpeg

**Solution:**
```bash
# Make FFmpeg executable
chmod +x /usr/local/bin/ffmpeg
chmod +x /usr/local/bin/ffprobe
```

### Thumbnail Generation Fails

**Error:** Thumbnails not generated for some videos

**Possible Causes & Solutions:**

1. **Corrupted video file:**
   - Try playing video in VLC
   - Re-encode with FFmpeg: `ffmpeg -i input.mp4 -c copy output.mp4`

2. **Unsupported codec:**
   - Check codec: `ffprobe video.mp4`
   - Re-encode: `ffmpeg -i input.mp4 -c:v libx264 output.mp4`

3. **Permissions:**
   - Check file permissions: `ls -l video.mp4`
   - Fix: `chmod 644 video.mp4`

---

## 🔙 Backend Issues

### Port Already in Use

**Error:** `[Errno 48] Address already in use`

**Cause:** Port 8000 is already occupied

**Solutions:**

1. **Kill existing process:**
```bash
# Find process using port 8000
lsof -ti:8000

# Kill it
lsof -ti:8000 | xargs kill

# Or force kill
lsof -ti:8000 | xargs kill -9
```

2. **Use different port:**
```python
# Edit backend/app/config.py
port: int = 8001  # Change to available port
```

### Database Migration Errors

**Error:** `alembic.util.exc.CommandError: Can't locate revision`

**Solutions:**

1. **Initialize Alembic:**
```bash
cd backend
alembic revision --autogenerate -m "Initial migration"
alembic upgrade head
```

2. **Reset migrations:**
```bash
# Backup data first!
rm -rf alembic/versions/*
rm data/database.db
alembic revision --autogenerate -m "Initial"
alembic upgrade head
```

### Import Errors

**Error:** `ModuleNotFoundError: No module named 'app'`

**Cause:** Running from wrong directory or virtual environment not activated

**Solution:**
```bash
# Make sure you're in backend directory
cd backend

# Activate virtual environment
source venv/bin/activate  # macOS/Linux
venv\Scripts\activate     # Windows

# Run from correct directory
python run.py
```

### CORS Errors

**Error:** Browser console shows CORS errors

**Solution:**

Check `backend/app/config.py`:
```python
cors_origins: list[str] = [
    "http://localhost:3000",
    "http://127.0.0.1:3000"
]
```

Add your frontend URL if different.

---

## 🎨 Frontend Issues

### Module Not Found

**Error:** `Error: Cannot find module 'next'`

**Solution:**
```bash
cd frontend
rm -rf node_modules package-lock.json
npm install
```

### Build Errors

**Error:** `Error: Failed to compile`

**Solutions:**

1. **Clear Next.js cache:**
```bash
rm -rf .next
npm run dev
```

2. **Check TypeScript errors:**
```bash
npm run lint
```

3. **Verify Node version:**
```bash
node --version  # Should be 18+
```

### API Connection Refused

**Error:** `fetch failed` or `ECONNREFUSED`

**Cause:** Backend not running or wrong API URL

**Solutions:**

1. **Verify backend is running:**
```bash
curl http://localhost:8000/health
```

2. **Check API URL:**
```bash
# frontend/.env.local
NEXT_PUBLIC_API_URL=http://localhost:8000/api
```

3. **Check CORS configuration in backend**

### Images Not Loading

**Error:** 404 for thumbnail images

**Solutions:**

1. **Verify thumbnails exist:**
```bash
ls -la data/thumbnails/
```

2. **Check Next.js image configuration:**
```javascript
// frontend/next.config.js
images: {
  remotePatterns: [
    {
      protocol: 'http',
      hostname: 'localhost',
      port: '8000',
      pathname: '/api/thumbnails/**',
    },
  ],
}
```

---

## 💾 Database Issues

### Database Locked

**Error:** `database is locked`

**Causes & Solutions:**

1. **Another process accessing database:**
```bash
# Find processes
lsof data/database.db

# Kill them
kill <PID>
```

2. **SQLite browser open:**
   - Close DB Browser for SQLite or similar tools
   - Restart backend

3. **Filesystem issues:**
```bash
# Check permissions
ls -l data/database.db

# Fix permissions
chmod 644 data/database.db
```

### Cannot Connect to Database

**Error:** `OperationalError: unable to open database file`

**Solutions:**

1. **Create data directory:**
```bash
mkdir -p data
```

2. **Check permissions:**
```bash
chmod 755 data
```

3. **Run migrations:**
```bash
cd backend
alembic upgrade head
```

### Database Corruption

**Error:** `database disk image is malformed`

**Solutions:**

1. **Try to recover:**
```bash
sqlite3 data/database.db "PRAGMA integrity_check"
```

2. **Restore from backup:**
```bash
cp data/database.db.backup data/database.db
```

3. **Start fresh:**
```bash
# CAUTION: This deletes all data!
rm data/database.db
cd backend
alembic upgrade head
# Rescan your videos
```

---

## 🎥 Video Issues

### Videos Not Appearing After Scan

**Possible Causes & Solutions:**

1. **Unsupported format:**
   - Check supported formats in config
   - Verify file extension

2. **Permission issues:**
```bash
# Check video file permissions
ls -l /path/to/videos/

# Fix permissions
chmod -R 644 /path/to/videos/*.mp4
```

3. **Scan failed silently:**
   - Check backend logs
   - Check scan status: GET `/api/scan/status/{scan_id}`

4. **Already indexed:**
   - Videos are skipped if already in database
   - Delete from database to rescan

### Video Won't Play

**Possible Causes & Solutions:**

1. **Codec not supported by browser:**
   - Chrome/Edge: H.264, VP8, VP9
   - Firefox: H.264, VP8, VP9, Theora
   - Safari: H.264

   **Solution:** Re-encode video:
```bash
ffmpeg -i input.mp4 -c:v libx264 -c:a aac output.mp4
```

2. **File moved or deleted:**
   - Verify file exists: `ls <file_path>`
   - Rescan if moved

3. **Permissions:**
```bash
chmod 644 /path/to/video.mp4
```

### Seeking Not Working

**Issue:** Cannot seek/skip in video

**Cause:** Video not encoded for streaming

**Solution:** Re-encode with web optimization:
```bash
ffmpeg -i input.mp4 -movflags +faststart -c:v libx264 -c:a aac output.mp4
```

The `-movflags +faststart` moves metadata to the beginning for streaming.

---

## ⚡ Performance Issues

### Slow Scanning

**Issue:** Directory scan takes very long

**Solutions:**

1. **Reduce thumbnail count:**
```python
# backend/app/config.py
thumbnail_count: int = 3  # Instead of 5
```

2. **Increase workers:**
```python
# backend/app/config.py
max_scan_workers: int = 8  # Instead of 4
```

3. **Skip large files:**
   - Add file size limit in scanner
   - Filter by size in directory

4. **Use SSD:**
   - Store database on SSD
   - Store thumbnails on SSD

### Slow Search

**Issue:** Search takes too long

**Solutions:**

1. **Check indexes:**
```sql
-- Verify indexes exist
.indexes videos
.indexes metadata
.indexes tags
```

2. **Optimize query:**
   - Reduce page size
   - Limit tag filters
   - Use category filter first

3. **Database maintenance:**
```bash
sqlite3 data/database.db "VACUUM"
sqlite3 data/database.db "ANALYZE"
```

### High Memory Usage

**Issue:** Application using too much RAM

**Solutions:**

1. **Reduce thumbnail cache:**
   - Clear browser cache
   - Reduce thumbnail size

2. **Limit concurrent scans:**
```python
max_scan_workers: int = 2
```

3. **Pagination:**
   - Reduce page size
   - Use smaller limits

---

## 🌐 Network Issues

### API Requests Timing Out

**Error:** `Request timeout`

**Solutions:**

1. **Increase timeout:**
```typescript
// frontend/src/lib/api.ts
const response = await fetch(url, {
  ...options,
  signal: AbortSignal.timeout(30000) // 30 seconds
});
```

2. **Check network:**
```bash
# Test backend
curl http://localhost:8000/health

# Check latency
time curl http://localhost:8000/api/videos
```

3. **Optimize query:**
   - Reduce page size
   - Remove heavy filters

### Proxy Issues

**Error:** Requests not reaching backend through Next.js proxy

**Solution:**

Check `frontend/next.config.js`:
```javascript
async rewrites() {
  return [
    {
      source: '/api/:path*',
      destination: 'http://localhost:8000/api/:path*',
    },
  ];
}
```

---

## 🐛 Debug Mode

### Enable Detailed Logging

**Backend:**
```python
# backend/app/main.py
import logging
logging.basicConfig(level=logging.DEBUG)
```

**Frontend:**
```typescript
// frontend/src/lib/api.ts
console.log('API Request:', endpoint, options);
console.log('API Response:', response);
```

### Check Logs

**Backend logs:**
```bash
# Run with verbose output
python run.py
```

**Frontend logs:**
- Open browser DevTools (F12)
- Check Console tab
- Check Network tab for failed requests

### Database Inspection

```bash
# Open SQLite database
sqlite3 data/database.db

# List tables
.tables

# Check videos
SELECT COUNT(*) FROM videos;
SELECT * FROM videos LIMIT 5;

# Check scan progress
SELECT * FROM videos ORDER BY indexed_date DESC LIMIT 10;
```

---

## 📞 Getting Help

If you're still experiencing issues:

1. **Check existing GitHub issues**
2. **Review documentation thoroughly**
3. **Provide detailed information:**
   - Operating system
   - Python/Node versions
   - FFmpeg version
   - Error messages (full stack trace)
   - Steps to reproduce

4. **Create detailed bug report:**
   - What you expected
   - What actually happened
   - Screenshots if applicable
   - Relevant logs

---

## 🔍 Useful Commands

### Health Checks

```bash
# Backend health
curl http://localhost:8000/health

# Database connection
sqlite3 data/database.db "SELECT COUNT(*) FROM videos"

# FFmpeg
ffmpeg -version
ffprobe -version

# Python environment
python --version
pip list

# Node environment
node --version
npm list
```

### Clean Start

```bash
# Complete reset (CAUTION: Deletes all data!)
rm -rf data/database.db data/thumbnails/*
rm -rf backend/venv
rm -rf frontend/.next frontend/node_modules

# Reinstall
cd backend && python -m venv venv && source venv/bin/activate && pip install -r requirements.txt
cd ../frontend && npm install

# Restart
cd backend && python run.py &
cd frontend && npm run dev
```

---

**Last Updated:** 2024
