import json
import subprocess
from pathlib import Path
from typing import Optional, Dict, Any
from datetime import datetime
from app.utils.file_utils import get_thumbnail_path


class FFmpegService:
    """Service for interacting with FFmpeg to extract metadata and generate thumbnails."""

    def __init__(self):
        self._check_ffmpeg_installed()

    def _check_ffmpeg_installed(self) -> None:
        """Check if ffmpeg and ffprobe are installed."""
        try:
            subprocess.run(['ffmpeg', '-version'], capture_output=True, check=True)
            subprocess.run(['ffprobe', '-version'], capture_output=True, check=True)
        except (subprocess.CalledProcessError, FileNotFoundError) as e:
            raise RuntimeError(
                "FFmpeg is not installed or not in PATH. "
                "Please install FFmpeg: https://ffmpeg.org/download.html"
            ) from e

    def extract_metadata(self, video_path: Path) -> Optional[Dict[str, Any]]:
        """
        Extract metadata from a video file using ffprobe.

        Args:
            video_path: Path to the video file

        Returns:
            Dictionary containing video metadata, or None if extraction fails
        """
        try:
            # Run ffprobe to get video information in JSON format
            cmd = [
                'ffprobe',
                '-v', 'quiet',
                '-print_format', 'json',
                '-show_format',
                '-show_streams',
                str(video_path)
            ]

            result = subprocess.run(cmd, capture_output=True, text=True, check=True)
            data = json.loads(result.stdout)

            # Find the video stream
            video_stream = None
            for stream in data.get('streams', []):
                if stream.get('codec_type') == 'video':
                    video_stream = stream
                    break

            if not video_stream:
                return None

            # Extract relevant metadata
            metadata = {
                'duration': float(data.get('format', {}).get('duration', 0)),
                'file_size': int(data.get('format', {}).get('size', 0)),
                'codec': video_stream.get('codec_name', ''),
                'resolution': f"{video_stream.get('width', 0)}x{video_stream.get('height', 0)}",
                'fps': self._parse_fps(video_stream.get('r_frame_rate', '0/0')),
            }

            # Try to extract creation date from format tags
            format_tags = data.get('format', {}).get('tags', {})
            creation_time = format_tags.get('creation_time')
            if creation_time:
                try:
                    metadata['created_date'] = datetime.fromisoformat(
                        creation_time.replace('Z', '+00:00')
                    )
                except (ValueError, AttributeError):
                    pass

            return metadata

        except (subprocess.CalledProcessError, json.JSONDecodeError, Exception) as e:
            print(f"Error extracting metadata from {video_path}: {e}")
            return None

    def _parse_fps(self, fps_string: str) -> Optional[float]:
        """
        Parse FPS from FFmpeg's fractional format (e.g., "30000/1001").

        Args:
            fps_string: FPS in fraction format

        Returns:
            FPS as float, or None if parsing fails
        """
        try:
            if '/' in fps_string:
                num, denom = fps_string.split('/')
                return float(num) / float(denom)
            return float(fps_string)
        except (ValueError, ZeroDivisionError):
            return None

    def generate_thumbnails(
        self,
        video_path: Path,
        video_id: int,
        thumbnail_dir: Path,
        count: int = 5,
        width: int = 320
    ) -> int:
        """
        Generate thumbnails from a video file.

        Args:
            video_path: Path to the video file
            video_id: Video ID for thumbnail naming
            thumbnail_dir: Directory to store thumbnails
            count: Number of thumbnails to generate
            width: Thumbnail width in pixels (height auto-calculated)

        Returns:
            Number of thumbnails successfully generated
        """
        try:
            # Create directory for this video's thumbnails
            video_thumb_dir = thumbnail_dir / str(video_id)
            video_thumb_dir.mkdir(parents=True, exist_ok=True)

            # Calculate interval for evenly distributed thumbnails
            # Use fps filter to extract frames at regular intervals
            # fps=1/X means extract 1 frame every X seconds
            metadata = self.extract_metadata(video_path)
            if not metadata or metadata['duration'] <= 0:
                return 0

            duration = metadata['duration']
            interval = duration / (count + 1)  # +1 to avoid first and last frame

            # Generate thumbnails using FFmpeg
            # We'll use the select filter to pick specific timestamps
            output_pattern = str(video_thumb_dir / "thumb_%03d.jpg")

            cmd = [
                'ffmpeg',
                '-i', str(video_path),
                '-vf', f"select='not(mod(n\\,{int(metadata['fps'] * interval)}))',scale={width}:-1",
                '-frames:v', str(count),
                '-vsync', 'vfr',
                '-y',  # Overwrite output files
                output_pattern
            ]

            # Alternative simpler approach: extract frames at specific timestamps
            # This is more reliable for getting evenly distributed thumbnails
            generated = 0
            for i in range(count):
                timestamp = interval * (i + 1)
                output_file = get_thumbnail_path(video_id, i, thumbnail_dir)

                cmd = [
                    'ffmpeg',
                    '-ss', str(timestamp),
                    '-i', str(video_path),
                    '-vframes', '1',
                    '-vf', f'scale={width}:-1',
                    '-y',
                    str(output_file)
                ]

                result = subprocess.run(
                    cmd,
                    capture_output=True,
                    text=True,
                    timeout=30
                )

                if result.returncode == 0 and output_file.exists():
                    generated += 1

            return generated

        except (subprocess.CalledProcessError, subprocess.TimeoutExpired, Exception) as e:
            print(f"Error generating thumbnails for {video_path}: {e}")
            return 0

    def get_video_info(self, video_path: Path) -> Optional[str]:
        """
        Get a quick video information string for debugging.

        Args:
            video_path: Path to the video file

        Returns:
            String with video information, or None if extraction fails
        """
        metadata = self.extract_metadata(video_path)
        if not metadata:
            return None

        return (
            f"Duration: {metadata['duration']:.2f}s, "
            f"Resolution: {metadata['resolution']}, "
            f"Codec: {metadata['codec']}, "
            f"FPS: {metadata['fps']:.2f}"
        )


# Global FFmpeg service instance
ffmpeg_service = FFmpegService()
