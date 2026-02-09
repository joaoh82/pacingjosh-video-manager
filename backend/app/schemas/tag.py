from pydantic import BaseModel, Field


class TagBase(BaseModel):
    """Base tag schema."""

    name: str = Field(..., min_length=1, max_length=100)


class TagCreate(TagBase):
    """Schema for creating a tag."""

    pass


class TagResponse(TagBase):
    """Schema for tag response."""

    id: int

    class Config:
        from_attributes = True


class TagWithCountResponse(TagResponse):
    """Schema for tag response with usage count."""

    count: int


class CategoryResponse(BaseModel):
    """Schema for category response."""

    name: str
    count: int
