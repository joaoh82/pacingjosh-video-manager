import uuid
import threading
from pathlib import Path
from typing import Optional, Callable, Dict, Any
from datetime import datetime
from sqlalchemy.orm import Session

from app.models import Video, Metadata
from app.services.ffmpeg_service import ffmpeg_service
from app.utils.file_utils import (
    is_video_file,
    get_file_size,
    get_file_creation_date,
    validate_directory
)
from app.config import settings


class ScanProgress:
    """Track scanning progress."""

    def __init__(self, scan_id: str):
        self.scan_id = scan_id
        self.status = "in_progress"  # in_progress, completed, failed
        self.total_files = 0
        self.processed_files = 0
        self.successful = 0
        self.failed = 0
        self.skipped = 0
        self.current_file = ""
        self.errors: list[str] = []
        self.start_time = datetime.utcnow()
        self.end_time: Optional[datetime] = None

    def to_dict(self) -> Dict[str, Any]:
        """Convert progress to dictionary."""
        now = datetime.utcnow()
        elapsed = (now - self.start_time).total_seconds()

        eta_seconds = None
        if self.processed_files > 0 and self.total_files > 0:
            avg_per_file = elapsed / self.processed_files
            remaining = self.total_files - self.processed_files
            eta_seconds = round(avg_per_file * remaining, 1)

        return {
            "scan_id": self.scan_id,
            "status": self.status,
            "total": self.total_files,
            "processed": self.processed_files,
            "successful": self.successful,
            "failed": self.failed,
            "skipped": self.skipped,
            "current_file": self.current_file,
            "errors": self.errors[-10:],  # Last 10 errors
            "start_time": self.start_time.isoformat(),
            "end_time": self.end_time.isoformat() if self.end_time else None,
            "elapsed_seconds": round(elapsed, 1),
            "eta_seconds": eta_seconds,
        }


class VideoScanner:
    """Service for scanning directories and indexing videos."""

    def __init__(self):
        self.active_scans: Dict[str, ScanProgress] = {}

    def start_scan(
        self,
        directory: str,
        db_factory: Callable[[], Session],
        progress_callback: Optional[Callable[[ScanProgress], None]] = None
    ) -> str:
        """
        Start scanning a directory for videos in a background thread.

        Args:
            directory: Directory path to scan
            db_factory: Callable that creates a new database session
            progress_callback: Optional callback function for progress updates

        Returns:
            Scan ID for tracking progress
        """
        # Validate directory
        is_valid, error = validate_directory(directory)
        if not is_valid:
            raise ValueError(error)

        # Create scan progress tracker
        scan_id = str(uuid.uuid4())
        progress = ScanProgress(scan_id)
        self.active_scans[scan_id] = progress

        def _run_scan():
            db = db_factory()
            try:
                self._scan_directory(directory, db, progress, progress_callback)
                progress.status = "completed"
                progress.end_time = datetime.utcnow()
            except Exception as e:
                progress.status = "failed"
                progress.errors.append(f"Scan failed: {str(e)}")
                progress.end_time = datetime.utcnow()
            finally:
                db.close()

        thread = threading.Thread(target=_run_scan, daemon=True)
        thread.start()

        return scan_id

    def _scan_directory(
        self,
        directory: str,
        db: Session,
        progress: ScanProgress,
        progress_callback: Optional[Callable[[ScanProgress], None]] = None
    ) -> None:
        """
        Internal method to scan directory recursively.

        Args:
            directory: Directory path to scan
            db: Database session
            progress: Progress tracker
            progress_callback: Optional callback for progress updates
        """
        dir_path = Path(directory)
        supported_formats = settings.supported_formats

        # First pass: count total video files
        video_files = []
        for file_path in dir_path.rglob('*'):
            if file_path.is_file() and is_video_file(file_path, supported_formats):
                video_files.append(file_path)

        progress.total_files = len(video_files)

        # Second pass: process each video
        for file_path in video_files:
            progress.current_file = str(file_path)

            try:
                # Check if video already exists in database
                existing = db.query(Video).filter(Video.file_path == str(file_path)).first()
                if existing:
                    progress.skipped += 1
                    progress.processed_files += 1
                    if progress_callback:
                        progress_callback(progress)
                    continue

                # Process the video
                self._process_video(file_path, db, progress)
                progress.successful += 1

            except Exception as e:
                error_msg = f"Error processing {file_path}: {str(e)}"
                progress.errors.append(error_msg)
                progress.failed += 1
                print(error_msg)

            finally:
                progress.processed_files += 1
                if progress_callback:
                    progress_callback(progress)

        # Commit all changes
        db.commit()

    def _process_video(self, file_path: Path, db: Session, progress: ScanProgress) -> None:
        """
        Process a single video file: extract metadata, generate thumbnails, add to database.

        Args:
            file_path: Path to the video file
            db: Database session
            progress: Progress tracker
        """
        # Check file size first - skip empty or very small files
        file_size = get_file_size(file_path)
        if file_size < 1024:  # Skip files smaller than 1KB
            error_msg = f"Skipping {file_path.name}: File is empty or too small ({file_size} bytes)"
            progress.errors.append(error_msg)
            print(f"⚠️  {error_msg}")
            raise ValueError(error_msg)

        # Extract metadata using FFmpeg
        ffmpeg_metadata = ffmpeg_service.extract_metadata(file_path)

        # Get file creation date
        file_created = get_file_creation_date(file_path)

        # Use FFmpeg creation date if available, otherwise use file creation date
        created_date = None
        if ffmpeg_metadata and 'created_date' in ffmpeg_metadata:
            created_date = ffmpeg_metadata['created_date']
        elif file_created:
            created_date = file_created

        # Create video record
        video = Video(
            file_path=str(file_path),
            filename=file_path.name,
            duration=ffmpeg_metadata.get('duration') if ffmpeg_metadata else None,
            file_size=file_size,
            resolution=ffmpeg_metadata.get('resolution') if ffmpeg_metadata else None,
            fps=ffmpeg_metadata.get('fps') if ffmpeg_metadata else None,
            codec=ffmpeg_metadata.get('codec') if ffmpeg_metadata else None,
            created_date=created_date,
            indexed_date=datetime.utcnow(),
            thumbnail_count=0
        )

        db.add(video)
        db.flush()  # Get the video ID

        # Generate thumbnails
        try:
            thumbnail_dir = Path(settings.thumbnail_directory)
            thumbnail_count = ffmpeg_service.generate_thumbnails(
                file_path,
                video.id,
                thumbnail_dir,
                count=settings.thumbnail_count,
                width=settings.thumbnail_width
            )
            video.thumbnail_count = thumbnail_count
        except Exception as e:
            print(f"Warning: Could not generate thumbnails for {file_path}: {e}")
            video.thumbnail_count = 0

        # Create empty metadata record
        metadata = Metadata(video_id=video.id)
        db.add(metadata)

    def get_scan_progress(self, scan_id: str) -> Optional[Dict[str, Any]]:
        """
        Get progress of a scan.

        Args:
            scan_id: Scan ID

        Returns:
            Progress dictionary, or None if scan not found
        """
        progress = self.active_scans.get(scan_id)
        return progress.to_dict() if progress else None

    def clear_completed_scans(self, max_age_minutes: int = 60) -> None:
        """
        Clear completed scans older than max_age_minutes.

        Args:
            max_age_minutes: Maximum age in minutes for completed scans to keep
        """
        current_time = datetime.utcnow()
        to_remove = []

        for scan_id, progress in self.active_scans.items():
            if progress.status in ["completed", "failed"] and progress.end_time:
                age = (current_time - progress.end_time).total_seconds() / 60
                if age > max_age_minutes:
                    to_remove.append(scan_id)

        for scan_id in to_remove:
            del self.active_scans[scan_id]


# Global scanner instance
video_scanner = VideoScanner()
