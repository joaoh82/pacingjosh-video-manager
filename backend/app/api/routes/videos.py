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
from app.services.video_service import video_service
from app.services.search_service import search_service


router = APIRouter(prefix="/videos", tags=["videos"])


@router.get("", response_model=VideoListResponse)
async def list_videos(
    search: Optional[str] = Query(None, description="Search term"),
    category: Optional[str] = Query(None, description="Filter by category"),
    tags: Optional[str] = Query(None, description="Comma-separated list of tags"),
    date_from: Optional[datetime] = Query(None, description="Filter from date"),
    date_to: Optional[datetime] = Query(None, description="Filter to date"),
    sort: str = Query("date_desc", description="Sort order"),
    page: int = Query(1, ge=1, description="Page number"),
    limit: int = Query(50, ge=1, le=200, description="Results per page"),
    db: Session = Depends(get_database)
):
    """
    List videos with optional filtering and pagination.

    Supports:
    - Full-text search across filename, location, and notes
    - Filter by category
    - Filter by tags (comma-separated, videos must have all tags)
    - Filter by date range
    - Multiple sort options
    - Pagination

    Args:
        search: Search term
        category: Category filter
        tags: Comma-separated tag names
        date_from: Start date filter
        date_to: End date filter
        sort: Sort order (date_desc, date_asc, name_asc, name_desc, duration_desc, duration_asc)
        page: Page number (1-based)
        limit: Results per page
        db: Database session

    Returns:
        Paginated list of videos with metadata and tags
    """
    # Parse tags
    tag_list = [t.strip() for t in tags.split(",")] if tags else None

    # Search videos
    videos, total = search_service.search_videos(
        db=db,
        search=search,
        category=category,
        tags=tag_list,
        date_from=date_from,
        date_to=date_to,
        sort=sort,
        page=page,
        limit=limit
    )

    # Convert to response format
    video_responses = []
    for video in videos:
        tags_list = [TagResponse(id=vt.tag.id, name=vt.tag.name) for vt in video.video_tags]
        video_response = VideoResponse.model_validate(video)
        video_response.tags = tags_list
        video_responses.append(video_response)

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
    """
    Get a single video by ID with all metadata and tags.

    Args:
        video_id: Video ID
        db: Database session

    Returns:
        Video details with metadata and tags
    """
    video = video_service.get_video(db, video_id)

    if not video:
        raise HTTPException(
            status_code=status.HTTP_404_NOT_FOUND,
            detail=f"Video not found: {video_id}"
        )

    # Convert tags
    tags_list = [TagResponse(id=vt.tag.id, name=vt.tag.name) for vt in video.video_tags]
    video_response = VideoResponse.model_validate(video)
    video_response.tags = tags_list

    return video_response


@router.put("/{video_id}", response_model=VideoResponse)
async def update_video(
    video_id: int,
    update_data: VideoUpdate,
    db: Session = Depends(get_database)
):
    """
    Update video metadata and tags.

    Args:
        video_id: Video ID
        update_data: Update data (category, location, notes, tags)
        db: Database session

    Returns:
        Updated video details
    """
    video = video_service.update_video(db, video_id, update_data)

    if not video:
        raise HTTPException(
            status_code=status.HTTP_404_NOT_FOUND,
            detail=f"Video not found: {video_id}"
        )

    # Convert tags
    tags_list = [TagResponse(id=vt.tag.id, name=vt.tag.name) for vt in video.video_tags]
    video_response = VideoResponse.model_validate(video)
    video_response.tags = tags_list

    return video_response


@router.delete("/{video_id}")
async def delete_video(
    video_id: int,
    db: Session = Depends(get_database)
):
    """
    Delete a video from the database.

    Note: This only removes the database entry, not the actual file.

    Args:
        video_id: Video ID
        db: Database session

    Returns:
        Success message
    """
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
    """
    Update multiple videos at once.

    Allows bulk updates of:
    - Category
    - Location
    - Notes
    - Add tags
    - Remove tags

    Args:
        bulk_update: Bulk update request with video IDs and updates
        db: Database session

    Returns:
        Number of videos updated
    """
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
    """
    Get recently indexed videos.

    Args:
        limit: Number of videos to return
        db: Database session

    Returns:
        List of recently indexed videos
    """
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
    """
    Get database statistics.

    Returns summary statistics including:
    - Total number of videos
    - Total duration
    - Total file size
    - Number of categories
    - Number of tags

    Args:
        db: Database session

    Returns:
        Statistics dictionary
    """
    return search_service.get_statistics(db)
