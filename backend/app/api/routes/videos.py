import os
import sys
import subprocess
from typing import Optional, List
from datetime import datetime
from fastapi import APIRouter, Depends, HTTPException, Query, status
from sqlalchemy.orm import Session

from app.api.deps import get_database
from app.schemas.video import (
    VideoResponse,
    VideoListResponse,
    VideoUpdate,
    BulkUpdateRequest,
    BulkUpdateResponse,
    TagResponse
)
from app.schemas.production import ProductionBriefResponse
from app.services.video_service import video_service
from app.services.search_service import search_service


router = APIRouter(prefix="/videos", tags=["videos"])


def _build_video_response(video) -> VideoResponse:
    """Build a VideoResponse from a Video ORM object."""
    tags_list = [TagResponse(id=vt.tag.id, name=vt.tag.name) for vt in video.video_tags]
    productions_list = [
        ProductionBriefResponse(id=vp.production.id, title=vp.production.title, link=vp.production.link)
        for vp in video.video_productions
    ]
    video_response = VideoResponse.model_validate(video)
    video_response.tags = tags_list
    video_response.productions = productions_list
    return video_response


@router.get("", response_model=VideoListResponse)
async def list_videos(
    search: Optional[str] = Query(None, description="Search term"),
    category: Optional[str] = Query(None, description="Filter by category"),
    tags: Optional[str] = Query(None, description="Comma-separated list of tags"),
    production: Optional[int] = Query(None, description="Filter by production ID"),
    date_from: Optional[datetime] = Query(None, description="Filter from date"),
    date_to: Optional[datetime] = Query(None, description="Filter to date"),
    sort: str = Query("date_desc", description="Sort order"),
    page: int = Query(1, ge=1, description="Page number"),
    limit: int = Query(50, ge=1, le=200, description="Results per page"),
    db: Session = Depends(get_database)
):
    """List videos with optional filtering and pagination."""
    # Parse tags
    tag_list = [t.strip() for t in tags.split(",")] if tags else None

    # Search videos
    videos, total = search_service.search_videos(
        db=db,
        search=search,
        category=category,
        tags=tag_list,
        production_id=production,
        date_from=date_from,
        date_to=date_to,
        sort=sort,
        page=page,
        limit=limit
    )

    # Convert to response format
    video_responses = [_build_video_response(v) for v in videos]

    pages = (total + limit - 1) // limit  # Ceiling division

    return VideoListResponse(
        videos=video_responses,
        total=total,
        page=page,
        limit=limit,
        pages=pages
    )


@router.get("/{video_id}", response_model=VideoResponse)
async def get_video(
    video_id: int,
    db: Session = Depends(get_database)
):
    """Get a single video by ID with all metadata and tags."""
    video = video_service.get_video(db, video_id)

    if not video:
        raise HTTPException(
            status_code=status.HTTP_404_NOT_FOUND,
            detail=f"Video not found: {video_id}"
        )

    return _build_video_response(video)


@router.put("/{video_id}", response_model=VideoResponse)
async def update_video(
    video_id: int,
    update_data: VideoUpdate,
    db: Session = Depends(get_database)
):
    """Update video metadata and tags."""
    video = video_service.update_video(db, video_id, update_data)

    if not video:
        raise HTTPException(
            status_code=status.HTTP_404_NOT_FOUND,
            detail=f"Video not found: {video_id}"
        )

    return _build_video_response(video)


@router.delete("/{video_id}")
async def delete_video(
    video_id: int,
    db: Session = Depends(get_database)
):
    """Delete a video from the database."""
    success = video_service.delete_video(db, video_id)

    if not success:
        raise HTTPException(
            status_code=status.HTTP_404_NOT_FOUND,
            detail=f"Video not found: {video_id}"
        )

    return {"message": f"Video {video_id} deleted successfully"}


@router.post("/bulk-update", response_model=BulkUpdateResponse)
async def bulk_update_videos(
    bulk_update: BulkUpdateRequest,
    db: Session = Depends(get_database)
):
    """Update multiple videos at once."""
    try:
        updated = video_service.bulk_update_videos(db, bulk_update)

        return BulkUpdateResponse(
            updated=updated,
            message=f"Successfully updated {updated} video(s)"
        )
    except Exception as e:
        raise HTTPException(
            status_code=status.HTTP_500_INTERNAL_SERVER_ERROR,
            detail=f"Failed to update videos: {str(e)}"
        )


@router.get("/recent/list")
async def get_recent_videos(
    limit: int = Query(20, ge=1, le=100, description="Number of videos to return"),
    db: Session = Depends(get_database)
):
    """Get recently indexed videos."""
    videos = search_service.get_recent_videos(db, limit)

    # Convert to response format
    video_responses = []
    for video in videos:
        tags_list = [TagResponse(id=vt.tag.id, name=vt.tag.name) for vt in video.video_tags]
        video_response = VideoResponse.model_validate(video)
        video_response.tags = tags_list
        video_responses.append(video_response)

    return video_responses


@router.get("/stats/summary")
async def get_statistics(db: Session = Depends(get_database)):
    """Get database statistics."""
    return search_service.get_statistics(db)


@router.post("/{video_id}/open-folder")
async def open_video_folder(
    video_id: int,
    db: Session = Depends(get_database)
):
    """Open the file explorer at the video's location."""
    video = video_service.get_video(db, video_id)
    if not video:
        raise HTTPException(
            status_code=status.HTTP_404_NOT_FOUND,
            detail=f"Video not found: {video_id}"
        )

    file_path = video.file_path
    if not os.path.exists(file_path):
        raise HTTPException(
            status_code=status.HTTP_404_NOT_FOUND,
            detail="File no longer exists on disk"
        )

    try:
        if sys.platform == "win32":
            subprocess.Popen(["explorer", "/select,", file_path])
        elif sys.platform == "darwin":
            subprocess.Popen(["open", "-R", file_path])
        else:
            folder = os.path.dirname(file_path)
            subprocess.Popen(["xdg-open", folder])
    except Exception as e:
        raise HTTPException(
            status_code=status.HTTP_500_INTERNAL_SERVER_ERROR,
            detail=f"Failed to open folder: {str(e)}"
        )

    return {"message": "Folder opened"}
