from datetime import datetime
from sqlalchemy import Column, Integer, String, Float, DateTime, Index
from sqlalchemy.orm import relationship
from app.database import Base


class Video(Base):
    """Video model representing a video file in the system."""

    __tablename__ = "videos"

    id = Column(Integer, primary_key=True, index=True, autoincrement=True)
    file_path = Column(String, unique=True, nullable=False, index=True)
    filename = Column(String, nullable=False, index=True)
    duration = Column(Float, nullable=True)  # Duration in seconds
    file_size = Column(Integer, nullable=True)  # Size in bytes
    resolution = Column(String, nullable=True)  # e.g., "1920x1080"
    fps = Column(Float, nullable=True)  # Frames per second
    codec = Column(String, nullable=True)  # Video codec
    created_date = Column(DateTime, nullable=True)  # File creation date
    indexed_date = Column(DateTime, default=datetime.utcnow, nullable=False)
    thumbnail_count = Column(Integer, default=0)

    # Relationships
    video_metadata = relationship("Metadata", back_populates="video", uselist=False, cascade="all, delete-orphan")
    video_tags = relationship("VideoTag", back_populates="video", cascade="all, delete-orphan")
    video_productions = relationship("VideoProduction", back_populates="video", cascade="all, delete-orphan")

    def __repr__(self):
        return f"<Video(id={self.id}, filename='{self.filename}')>"


# Create indexes
Index("idx_videos_created_date", Video.created_date)
Index("idx_videos_filename", Video.filename)
