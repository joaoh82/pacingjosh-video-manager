from sqlalchemy import Column, Integer, String, ForeignKey, Index
from sqlalchemy.orm import relationship
from app.database import Base


class VideoUsage(Base):
    """Tracks where a video has been used (YouTube, Instagram, TikTok, etc.)."""

    __tablename__ = "video_usages"

    id = Column(Integer, primary_key=True, index=True, autoincrement=True)
    video_id = Column(Integer, ForeignKey("videos.id", ondelete="CASCADE"), nullable=False)
    title = Column(String, nullable=False)
    link = Column(String, nullable=False)

    # Relationships
    video = relationship("Video", back_populates="video_usages")

    def __repr__(self):
        return f"<VideoUsage(id={self.id}, video_id={self.video_id}, title='{self.title}')>"


# Create indexes
Index("idx_video_usages_video_id", VideoUsage.video_id)
