from typing import Optional
from pydantic import BaseModel


class MetadataBase(BaseModel):
    """Base metadata schema."""

    category: Optional[str] = None
    location: Optional[str] = None
    notes: Optional[str] = None


class MetadataCreate(MetadataBase):
    """Schema for creating metadata."""

    video_id: int


class MetadataUpdate(MetadataBase):
    """Schema for updating metadata."""

    pass


class MetadataResponse(MetadataBase):
    """Schema for metadata response."""

    id: int
    video_id: int

    class Config:
        from_attributes = True
