from sqlalchemy import Column, Integer, String, ForeignKey, Index
from sqlalchemy.orm import relationship
from app.database import Base


class Metadata(Base):
    """Metadata model for storing additional video information."""

    __tablename__ = "metadata"

    id = Column(Integer, primary_key=True, index=True, autoincrement=True)
    video_id = Column(Integer, ForeignKey("videos.id", ondelete="CASCADE"), unique=True, nullable=False)
    category = Column(String, nullable=True, index=True)
    location = Column(String, nullable=True)
    notes = Column(String, nullable=True)

    # Relationships
    video = relationship("Video", back_populates="video_metadata")

    def __repr__(self):
        return f"<Metadata(id={self.id}, video_id={self.video_id}, category='{self.category}')>"


# Create indexes
Index("idx_metadata_category", Metadata.category)
