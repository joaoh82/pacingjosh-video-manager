from contextlib import asynccontextmanager
from fastapi import FastAPI
from fastapi.middleware.cors import CORSMiddleware
from pydantic import BaseModel

from app.config import settings
from app.database import init_db
from app.api.routes import scan, videos, tags, stream, productions


@asynccontextmanager
async def lifespan(app: FastAPI):
    """
    Lifespan context manager for startup and shutdown events.

    Handles:
    - Database initialization on startup
    - Cleanup on shutdown
    """
    # Startup
    print("🚀 Starting Video Manager API...")
    print(f"📁 Database: {settings.database_path}")
    print(f"🎬 Video directory: {settings.video_directory or 'Not configured'}")

    # Initialize database
    try:
        init_db()
        print("✅ Database initialized")
    except Exception as e:
        print(f"⚠️  Database initialization warning: {e}")

    yield

    # Shutdown
    print("👋 Shutting down Video Manager API...")


# Create FastAPI application
app = FastAPI(
    title=settings.app_name,
    version=settings.app_version,
    description="Video indexing and management API",
    lifespan=lifespan,
    docs_url="/docs",
    redoc_url="/redoc"
)

# Configure CORS
app.add_middleware(
    CORSMiddleware,
    allow_origins=settings.cors_origins,
    allow_credentials=True,
    allow_methods=["*"],
    allow_headers=["*"],
    max_age=3600,  # Cache preflight requests for 1 hour
)

# Include routers
app.include_router(scan.router, prefix="/api")
app.include_router(videos.router, prefix="/api")
app.include_router(tags.router, prefix="/api")
app.include_router(stream.router, prefix="/api")
app.include_router(productions.router, prefix="/api")


@app.get("/")
async def root():
    """Root endpoint with API information."""
    return {
        "name": settings.app_name,
        "version": settings.app_version,
        "status": "running",
        "docs": "/docs",
        "message": "Welcome to Video Manager API"
    }


@app.get("/health")
async def health_check():
    """Health check endpoint."""
    return {
        "status": "healthy",
        "database": "connected"
    }


@app.options("/api/config")
async def config_options():
    """Handle OPTIONS preflight for config endpoint."""
    return {"status": "ok"}


@app.get("/api/config")
async def get_config():
    """
    Get current configuration status.

    Returns configuration information without sensitive data.
    """
    from app.config import config_manager

    return {
        "configured": config_manager.is_configured(),
        "video_directory": config_manager.settings.video_directory,
        "supported_formats": config_manager.settings.supported_formats,
        "thumbnail_count": config_manager.settings.thumbnail_count,
    }


class ConfigRequest(BaseModel):
    video_directory: str
    thumbnail_count: int = 5
    thumbnail_width: int = 320


@app.get("/api/browse-folder")
async def browse_folder():
    """
    Open native folder picker dialog.

    Returns selected folder path or error if user cancels.
    """
    import platform
    import subprocess

    system = platform.system()

    try:
        if system == "Darwin":  # macOS
            # Use AppleScript for native macOS folder picker
            script = '''
            try
                set folderPath to choose folder with prompt "Select Video Directory" default location (path to home folder)
                return POSIX path of folderPath
            on error errMsg
                return ""
            end try
            '''
            result = subprocess.run(
                ['osascript', '-e', script],
                capture_output=True,
                text=True,
                timeout=300  # 5 minute timeout
            )

            if result.returncode == 0 and result.stdout.strip():
                folder_path = result.stdout.strip()
                return {
                    "success": True,
                    "path": folder_path
                }
            else:
                return {
                    "success": False,
                    "message": "No folder selected"
                }

        elif system == "Windows":
            # Use tkinter on Windows
            import tkinter as tk
            from tkinter import filedialog

            root = tk.Tk()
            root.withdraw()
            root.attributes('-topmost', True)

            folder_path = filedialog.askdirectory(
                title="Select Video Directory",
                mustexist=True
            )

            root.destroy()

            if folder_path:
                return {
                    "success": True,
                    "path": folder_path
                }
            else:
                return {
                    "success": False,
                    "message": "No folder selected"
                }

        elif system == "Linux":
            # Try zenity on Linux (most common)
            result = subprocess.run(
                ['zenity', '--file-selection', '--directory', '--title=Select Video Directory'],
                capture_output=True,
                text=True,
                timeout=300
            )

            if result.returncode == 0 and result.stdout.strip():
                folder_path = result.stdout.strip()
                return {
                    "success": True,
                    "path": folder_path
                }
            else:
                return {
                    "success": False,
                    "message": "No folder selected or zenity not installed"
                }

        else:
            return {
                "success": False,
                "message": f"Folder picker not supported on {system}"
            }

    except subprocess.TimeoutExpired:
        return {
            "success": False,
            "message": "Folder selection timed out"
        }
    except FileNotFoundError as e:
        return {
            "success": False,
            "message": f"Required system tool not found: {str(e)}"
        }
    except Exception as e:
        return {
            "success": False,
            "message": f"Error opening folder picker: {str(e)}"
        }


@app.post("/api/config")
async def save_config(config: ConfigRequest):
    """
    Save application configuration.

    Args:
        config: Configuration request with video directory and thumbnail settings

    Returns:
        Configuration status
    """
    from app.config import config_manager

    try:
        config_manager.save_config(
            video_directory=config.video_directory,
            thumbnail_count=config.thumbnail_count,
            thumbnail_width=config.thumbnail_width
        )

        return {
            "status": "success",
            "message": "Configuration saved",
            "configured": config_manager.is_configured()
        }
    except Exception as e:
        return {
            "status": "error",
            "message": f"Failed to save configuration: {str(e)}"
        }


if __name__ == "__main__":
    import uvicorn

    uvicorn.run(
        "app.main:app",
        host=settings.host,
        port=settings.port,
        reload=True
    )
