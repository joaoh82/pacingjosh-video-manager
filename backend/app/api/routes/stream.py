import os
from pathlib import Path
from typing import Optional
from fastapi import APIRouter, Depends, HTTPException, status, Request
from fastapi.responses import FileResponse, StreamingResponse
from sqlalchemy.orm import Session

from app.api.deps import get_database
from app.services.video_service import video_service
from app.utils.file_utils import get_thumbnail_path, is_path_safe
from app.config import config_manager


router = APIRouter(tags=["stream"])


@router.get("/thumbnails/{video_id}/{index}")
async def get_thumbnail(
    video_id: int,
    index: int,
    db: Session = Depends(get_database)
):
    """
    Get a thumbnail for a video.

    Args:
        video_id: Video ID
        index: Thumbnail index (0-based)
        db: Database session

    Returns:
        Thumbnail image file
    """
    # Verify video exists
    video = video_service.get_video(db, video_id)
    if not video:
        raise HTTPException(
            status_code=status.HTTP_404_NOT_FOUND,
            detail=f"Video not found: {video_id}"
        )

    # Verify thumbnail index is valid
    if index < 0 or index >= video.thumbnail_count:
        raise HTTPException(
            status_code=status.HTTP_404_NOT_FOUND,
            detail=f"Thumbnail index {index} out of range (0-{video.thumbnail_count - 1})"
        )

    # Get thumbnail path
    thumbnail_dir = config_manager.get_thumbnail_directory()
    thumbnail_path = get_thumbnail_path(video_id, index, thumbnail_dir)

    # Verify thumbnail exists
    if not thumbnail_path.exists():
        raise HTTPException(
            status_code=status.HTTP_404_NOT_FOUND,
            detail=f"Thumbnail not found: {thumbnail_path}"
        )

    return FileResponse(
        thumbnail_path,
        media_type="image/jpeg",
        headers={
            "Cache-Control": "public, max-age=86400"  # Cache for 24 hours
        }
    )


@router.get("/stream/{video_id}")
async def stream_video(
    video_id: int,
    request: Request,
    db: Session = Depends(get_database)
):
    """
    Stream a video file with range request support.

    Supports partial content (HTTP 206) for video seeking.

    Args:
        video_id: Video ID
        request: FastAPI request object
        db: Database session

    Returns:
        Video file stream
    """
    # Get video from database
    video = video_service.get_video(db, video_id)
    if not video:
        raise HTTPException(
            status_code=status.HTTP_404_NOT_FOUND,
            detail=f"Video not found: {video_id}"
        )

    # Get video file path
    video_path = Path(video.file_path)

    # Verify file exists
    if not video_path.exists():
        raise HTTPException(
            status_code=status.HTTP_404_NOT_FOUND,
            detail=f"Video file not found: {video.file_path}"
        )

    # Verify path safety
    video_dir = config_manager.get_video_directory()
    if video_dir and not is_path_safe(str(video_path), str(video_dir)):
        raise HTTPException(
            status_code=status.HTTP_403_FORBIDDEN,
            detail="Access denied"
        )

    # Get file size
    file_size = video_path.stat().st_size

    # Parse range header
    range_header = request.headers.get("range")

    # Determine content type
    content_type = get_content_type(video_path.suffix)

    if not range_header:
        # No range request, return full file
        return FileResponse(
            video_path,
            media_type=content_type,
            headers={
                "Accept-Ranges": "bytes",
                "Content-Length": str(file_size)
            }
        )

    # Parse range header (format: "bytes=start-end")
    try:
        range_str = range_header.replace("bytes=", "")
        range_parts = range_str.split("-")
        start = int(range_parts[0]) if range_parts[0] else 0
        end = int(range_parts[1]) if range_parts[1] else file_size - 1
    except (ValueError, IndexError):
        raise HTTPException(
            status_code=status.HTTP_416_REQUESTED_RANGE_NOT_SATISFIABLE,
            detail="Invalid range header"
        )

    # Validate range
    if start >= file_size or end >= file_size or start > end:
        raise HTTPException(
            status_code=status.HTTP_416_REQUESTED_RANGE_NOT_SATISFIABLE,
            detail=f"Range out of bounds (file size: {file_size})"
        )

    # Calculate content length
    content_length = end - start + 1

    # Create streaming response
    def iter_file():
        with open(video_path, "rb") as f:
            f.seek(start)
            remaining = content_length
            chunk_size = 8192  # 8KB chunks

            while remaining > 0:
                chunk = f.read(min(chunk_size, remaining))
                if not chunk:
                    break
                remaining -= len(chunk)
                yield chunk

    return StreamingResponse(
        iter_file(),
        media_type=content_type,
        status_code=206,  # Partial Content
        headers={
            "Content-Range": f"bytes {start}-{end}/{file_size}",
            "Accept-Ranges": "bytes",
            "Content-Length": str(content_length),
            "Cache-Control": "public, max-age=3600"
        }
    )


def get_content_type(extension: str) -> str:
    """
    Get content type for video file extension.

    Args:
        extension: File extension (e.g., '.mp4')

    Returns:
        MIME type string
    """
    content_types = {
        ".mp4": "video/mp4",
        ".mov": "video/quicktime",
        ".avi": "video/x-msvideo",
        ".mkv": "video/x-matroska",
        ".webm": "video/webm",
        ".flv": "video/x-flv",
        ".wmv": "video/x-ms-wmv",
    }
    return content_types.get(extension.lower(), "video/mp4")
