from typing import Optional
from pydantic import BaseModel


class ProductionCreate(BaseModel):
    """Schema for creating/updating a production."""

    title: str
    platform: Optional[str] = None
    link: Optional[str] = None
    is_published: bool = False


class ProductionResponse(BaseModel):
    """Schema for production response with video count."""

    id: int
    title: str
    platform: Optional[str] = None
    link: Optional[str] = None
    is_published: bool = False
    video_count: int = 0

    class Config:
        from_attributes = True


class ProductionBriefResponse(BaseModel):
    """Schema for production data embedded in video responses."""

    id: int
    title: str
    platform: Optional[str] = None
    link: Optional[str] = None
    is_published: bool = False

    class Config:
        from_attributes = True
