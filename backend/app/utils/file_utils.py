import os
from pathlib import Path
from typing import Optional
from datetime import datetime


def is_video_file(file_path: Path, supported_formats: list[str]) -> bool:
    """
    Check if a file is a supported video format.

    Args:
        file_path: Path to the file
        supported_formats: List of supported extensions (e.g., ['.mp4', '.mov'])

    Returns:
        True if the file has a supported video extension
    """
    return file_path.suffix.lower() in supported_formats


def get_file_size(file_path: Path) -> Optional[int]:
    """
    Get file size in bytes.

    Args:
        file_path: Path to the file

    Returns:
        File size in bytes, or None if file doesn't exist
    """
    try:
        return file_path.stat().st_size
    except (OSError, FileNotFoundError):
        return None


def get_file_creation_date(file_path: Path) -> Optional[datetime]:
    """
    Get file creation date.

    Args:
        file_path: Path to the file

    Returns:
        Creation datetime, or None if unavailable
    """
    try:
        stat = file_path.stat()
        # On Unix, st_birthtime is not always available, use st_mtime as fallback
        timestamp = getattr(stat, 'st_birthtime', stat.st_mtime)
        return datetime.fromtimestamp(timestamp)
    except (OSError, FileNotFoundError):
        return None


def validate_directory(directory: str) -> tuple[bool, Optional[str]]:
    """
    Validate that a directory exists and is accessible.

    Args:
        directory: Directory path string

    Returns:
        Tuple of (is_valid, error_message)
    """
    if not directory:
        return False, "Directory path is empty"

    dir_path = Path(directory)

    if not dir_path.exists():
        return False, f"Directory does not exist: {directory}"

    if not dir_path.is_dir():
        return False, f"Path is not a directory: {directory}"

    if not os.access(dir_path, os.R_OK):
        return False, f"Directory is not readable: {directory}"

    return True, None


def is_path_safe(file_path: str, base_directory: str) -> bool:
    """
    Check if a file path is safe (prevents path traversal attacks).

    Args:
        file_path: File path to validate
        base_directory: Base directory that should contain the file

    Returns:
        True if the path is safe (within base_directory)
    """
    try:
        base = Path(base_directory).resolve()
        target = Path(file_path).resolve()
        return target.is_relative_to(base)
    except (ValueError, RuntimeError):
        return False


def format_file_size(size_bytes: int) -> str:
    """
    Format file size in human-readable format.

    Args:
        size_bytes: Size in bytes

    Returns:
        Formatted string (e.g., "1.5 MB")
    """
    for unit in ['B', 'KB', 'MB', 'GB', 'TB']:
        if size_bytes < 1024.0:
            return f"{size_bytes:.1f} {unit}"
        size_bytes /= 1024.0
    return f"{size_bytes:.1f} PB"


def format_duration(seconds: float) -> str:
    """
    Format duration in human-readable format.

    Args:
        seconds: Duration in seconds

    Returns:
        Formatted string (e.g., "1h 23m 45s" or "5m 30s")
    """
    hours = int(seconds // 3600)
    minutes = int((seconds % 3600) // 60)
    secs = int(seconds % 60)

    if hours > 0:
        return f"{hours}h {minutes}m {secs}s"
    elif minutes > 0:
        return f"{minutes}m {secs}s"
    else:
        return f"{secs}s"


def get_thumbnail_path(video_id: int, index: int, thumbnail_dir: Path) -> Path:
    """
    Get the path for a video thumbnail.

    Args:
        video_id: Video ID
        index: Thumbnail index (0-based)
        thumbnail_dir: Base thumbnail directory

    Returns:
        Path to the thumbnail file
    """
    video_thumb_dir = thumbnail_dir / str(video_id)
    video_thumb_dir.mkdir(parents=True, exist_ok=True)
    return video_thumb_dir / f"thumb_{index}.jpg"


def ensure_directory_exists(directory: Path) -> None:
    """
    Ensure a directory exists, create if it doesn't.

    Args:
        directory: Directory path
    """
    directory.mkdir(parents=True, exist_ok=True)
