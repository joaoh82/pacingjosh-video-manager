from typing import List
from fastapi import APIRouter, Depends
from sqlalchemy.orm import Session

from app.api.deps import get_database
from app.schemas.tag import TagWithCountResponse, CategoryResponse
from app.services.video_service import video_service


router = APIRouter(prefix="/tags", tags=["tags"])


@router.get("", response_model=List[TagWithCountResponse])
async def get_all_tags(db: Session = Depends(get_database)):
    """
    Get all tags with usage count.

    Returns a list of all tags in the database along with the number
    of videos using each tag.

    Args:
        db: Database session

    Returns:
        List of tags with usage counts
    """
    tags = video_service.get_all_tags(db)
    return [TagWithCountResponse(**tag) for tag in tags]


@router.get("/categories", response_model=List[CategoryResponse])
async def get_all_categories(db: Session = Depends(get_database)):
    """
    Get all categories with usage count.

    Returns a list of all unique categories in the database along with
    the number of videos in each category.

    Args:
        db: Database session

    Returns:
        List of categories with usage counts
    """
    categories = video_service.get_all_categories(db)
    return [CategoryResponse(**cat) for cat in categories]
