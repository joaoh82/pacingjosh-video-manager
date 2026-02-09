import json
import os
from pathlib import Path
from typing import Optional
from pydantic_settings import BaseSettings


class Settings(BaseSettings):
    """Application settings with support for config file and environment variables."""

    # Application settings
    app_name: str = "Video Manager"
    app_version: str = "1.0.0"

    # Server settings
    host: str = "127.0.0.1"
    port: int = 8000

    # Database settings
    database_url: str = "sqlite:///./data/database.db"
    database_path: str = "./data/database.db"

    # Video settings
    video_directory: Optional[str] = None
    supported_formats: list[str] = [".mp4", ".mov", ".avi", ".mkv", ".webm", ".flv", ".wmv"]

    # Thumbnail settings
    thumbnail_directory: str = "./data/thumbnails"
    thumbnail_count: int = 5
    thumbnail_width: int = 320

    # Scanning settings
    max_scan_workers: int = 4

    # CORS settings
    cors_origins: list[str] = ["http://localhost:3000", "http://127.0.0.1:3000", "http://localhost:3002", "http://127.0.0.1:3002"]

    class Config:
        env_file = ".env"
        env_file_encoding = "utf-8"


class ConfigManager:
    """Manages application configuration with persistence to JSON file."""

    def __init__(self, config_path: str = "./data/config.json"):
        self.config_path = Path(config_path)
        self.settings = Settings()
        self._load_config()

    def _load_config(self):
        """Load configuration from JSON file if it exists."""
        if self.config_path.exists():
            try:
                with open(self.config_path, 'r') as f:
                    config_data = json.load(f)
                    # Update settings with values from config file
                    for key, value in config_data.items():
                        if hasattr(self.settings, key):
                            setattr(self.settings, key, value)
            except Exception as e:
                print(f"Warning: Could not load config file: {e}")

    def save_config(self, **kwargs):
        """Save configuration to JSON file."""
        # Update settings
        for key, value in kwargs.items():
            if hasattr(self.settings, key):
                setattr(self.settings, key, value)

        # Ensure data directory exists
        self.config_path.parent.mkdir(parents=True, exist_ok=True)

        # Save to file
        config_data = {
            "video_directory": self.settings.video_directory,
            "database_path": self.settings.database_path,
            "thumbnail_directory": self.settings.thumbnail_directory,
            "thumbnail_count": self.settings.thumbnail_count,
            "thumbnail_width": self.settings.thumbnail_width,
            "supported_formats": self.settings.supported_formats,
            "max_scan_workers": self.settings.max_scan_workers,
        }

        with open(self.config_path, 'w') as f:
            json.dump(config_data, f, indent=2)

    def is_configured(self) -> bool:
        """Check if the application is configured (has video directory set)."""
        return self.settings.video_directory is not None and self.config_path.exists()

    def get_video_directory(self) -> Optional[Path]:
        """Get the configured video directory as Path object."""
        if self.settings.video_directory:
            return Path(self.settings.video_directory)
        return None

    def get_thumbnail_directory(self) -> Path:
        """Get thumbnail directory as Path object, create if doesn't exist."""
        thumb_dir = Path(self.settings.thumbnail_directory)
        thumb_dir.mkdir(parents=True, exist_ok=True)
        return thumb_dir

    def get_database_path(self) -> Path:
        """Get database path as Path object, create parent dir if needed."""
        db_path = Path(self.settings.database_path)
        db_path.parent.mkdir(parents=True, exist_ok=True)
        return db_path


# Global config instance
config_manager = ConfigManager()
settings = config_manager.settings
