from sqlalchemy import Column, Integer, String, Boolean, ForeignKey, Index, PrimaryKeyConstraint
from sqlalchemy.orm import relationship
from app.database import Base


class Production(Base):
    """Production model representing a published video (YouTube, TikTok, etc.)."""

    __tablename__ = "productions"

    id = Column(Integer, primary_key=True, index=True, autoincrement=True)
    title = Column(String, unique=True, nullable=False, index=True)
    platform = Column(String, nullable=True)
    link = Column(String, nullable=True)
    is_published = Column(Boolean, default=False, nullable=False)

    # Relationships
    video_productions = relationship("VideoProduction", back_populates="production", cascade="all, delete-orphan")

    def __repr__(self):
        return f"<Production(id={self.id}, title='{self.title}')>"


class VideoProduction(Base):
    """Association table for many-to-many relationship between videos and productions."""

    __tablename__ = "video_productions"

    video_id = Column(Integer, ForeignKey("videos.id", ondelete="CASCADE"), nullable=False)
    production_id = Column(Integer, ForeignKey("productions.id", ondelete="CASCADE"), nullable=False)

    # Composite primary key
    __table_args__ = (
        PrimaryKeyConstraint("video_id", "production_id"),
        Index("idx_video_productions_video", "video_id"),
        Index("idx_video_productions_production", "production_id"),
    )

    # Relationships
    video = relationship("Video", back_populates="video_productions")
    production = relationship("Production", back_populates="video_productions")

    def __repr__(self):
        return f"<VideoProduction(video_id={self.video_id}, production_id={self.production_id})>"


# Create indexes
Index("idx_productions_title", Production.title)
