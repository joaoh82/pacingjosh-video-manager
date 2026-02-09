#!/usr/bin/env python3
"""
Simple script to run the Video Manager API server.

Usage:
    python run.py
"""

import uvicorn
from app.config import settings

if __name__ == "__main__":
    print("=" * 60)
    print("🎬 Video Manager API")
    print("=" * 60)
    print(f"📡 Server: http://{settings.host}:{settings.port}")
    print(f"📚 API Docs: http://{settings.host}:{settings.port}/docs")
    print(f"📁 Database: {settings.database_path}")
    print(f"🎬 Video Directory: {settings.video_directory or 'Not configured'}")
    print("=" * 60)
    print("\n🚀 Starting server...\n")

    uvicorn.run(
        "app.main:app",
        host=settings.host,
        port=settings.port,
        reload=True,
        log_level="info"
    )
