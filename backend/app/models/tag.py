from sqlalchemy import Column, Integer, String, ForeignKey, Index, PrimaryKeyConstraint
from sqlalchemy.orm import relationship
from app.database import Base


class Tag(Base):
    """Tag model for categorizing videos."""

    __tablename__ = "tags"

    id = Column(Integer, primary_key=True, index=True, autoincrement=True)
    name = Column(String, unique=True, nullable=False, index=True)

    # Relationships
    video_tags = relationship("VideoTag", back_populates="tag", cascade="all, delete-orphan")

    def __repr__(self):
        return f"<Tag(id={self.id}, name='{self.name}')>"


class VideoTag(Base):
    """Association table for many-to-many relationship between videos and tags."""

    __tablename__ = "video_tags"

    video_id = Column(Integer, ForeignKey("videos.id", ondelete="CASCADE"), nullable=False)
    tag_id = Column(Integer, ForeignKey("tags.id", ondelete="CASCADE"), nullable=False)

    # Composite primary key
    __table_args__ = (
        PrimaryKeyConstraint("video_id", "tag_id"),
        Index("idx_video_tags_video", "video_id"),
        Index("idx_video_tags_tag", "tag_id"),
    )

    # Relationships
    video = relationship("Video", back_populates="video_tags")
    tag = relationship("Tag", back_populates="video_tags")

    def __repr__(self):
        return f"<VideoTag(video_id={self.video_id}, tag_id={self.tag_id})>"


# Create indexes
Index("idx_tags_name", Tag.name)
