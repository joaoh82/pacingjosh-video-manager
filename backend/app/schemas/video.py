from datetime import datetime
from typing import Optional
from pydantic import BaseModel, Field

from app.schemas.production import ProductionBriefResponse


class VideoBase(BaseModel):
    """Base video schema with common fields."""

    filename: str
    duration: Optional[float] = None
    file_size: Optional[int] = None
    resolution: Optional[str] = None
    fps: Optional[float] = None
    codec: Optional[str] = None
    created_date: Optional[datetime] = None


class VideoCreate(VideoBase):
    """Schema for creating a new video."""

    file_path: str


class VideoUpdate(BaseModel):
    """Schema for updating video metadata."""

    category: Optional[str] = None
    location: Optional[str] = None
    notes: Optional[str] = None
    tags: Optional[list[str]] = None
    production_ids: Optional[list[int]] = None


class MetadataResponse(BaseModel):
    """Schema for metadata in video responses."""

    category: Optional[str] = None
    location: Optional[str] = None
    notes: Optional[str] = None

    class Config:
        from_attributes = True


class TagResponse(BaseModel):
    """Schema for tag in video responses."""

    id: int
    name: str

    class Config:
        from_attributes = True


class VideoResponse(VideoBase):
    """Schema for video response with all related data."""

    id: int
    file_path: str
    indexed_date: datetime
    thumbnail_count: int
    metadata: Optional[MetadataResponse] = Field(None, alias='video_metadata')
    tags: list[TagResponse] = []
    productions: list[ProductionBriefResponse] = []

    class Config:
        from_attributes = True
        populate_by_name = True


class VideoListResponse(BaseModel):
    """Schema for paginated video list response."""

    videos: list[VideoResponse]
    total: int
    page: int
    limit: int
    pages: int


class BulkUpdateRequest(BaseModel):
    """Schema for bulk update request."""

    video_ids: list[int] = Field(..., min_length=1)
    category: Optional[str] = None
    location: Optional[str] = None
    notes: Optional[str] = None
    add_tags: Optional[list[str]] = None
    remove_tags: Optional[list[str]] = None
    add_production_ids: Optional[list[int]] = None
    remove_production_ids: Optional[list[int]] = None


class BulkUpdateResponse(BaseModel):
    """Schema for bulk update response."""

    updated: int
    message: str
