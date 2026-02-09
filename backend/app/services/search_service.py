from typing import Optional, List, Tuple
from datetime import datetime
from sqlalchemy.orm import Session, joinedload
from sqlalchemy import or_, and_, func

from app.models import Video, Metadata, Tag, VideoTag


class SearchService:
    """Service for searching and filtering videos."""

    def search_videos(
        self,
        db: Session,
        search: Optional[str] = None,
        category: Optional[str] = None,
        tags: Optional[List[str]] = None,
        date_from: Optional[datetime] = None,
        date_to: Optional[datetime] = None,
        sort: str = "date_desc",
        page: int = 1,
        limit: int = 50
    ) -> Tuple[List[Video], int]:
        """
        Search and filter videos with pagination.

        Args:
            db: Database session
            search: Search term for filename, location, and notes
            category: Filter by category
            tags: Filter by tags (videos must have all specified tags)
            date_from: Filter by creation date from
            date_to: Filter by creation date to
            sort: Sort order (date_desc, date_asc, name_asc, name_desc, duration_desc, duration_asc)
            page: Page number (1-based)
            limit: Results per page

        Returns:
            Tuple of (videos list, total count)
        """
        # Base query with eager loading
        query = db.query(Video).options(
            joinedload(Video.video_metadata),
            joinedload(Video.video_tags).joinedload(VideoTag.tag)
        )

        # Apply search filter
        if search:
            search_term = f"%{search}%"
            query = query.join(Metadata, Video.id == Metadata.video_id, isouter=True)
            query = query.filter(
                or_(
                    Video.filename.ilike(search_term),
                    Metadata.location.ilike(search_term),
                    Metadata.notes.ilike(search_term)
                )
            )

        # Apply category filter
        if category:
            if not search:  # Only join if not already joined
                query = query.join(Metadata, Video.id == Metadata.video_id, isouter=True)
            query = query.filter(Metadata.category == category)

        # Apply tag filters
        if tags:
            # For each tag, the video must have that tag
            for tag_name in tags:
                tag_subquery = (
                    db.query(VideoTag.video_id)
                    .join(Tag, VideoTag.tag_id == Tag.id)
                    .filter(Tag.name == tag_name)
                    .subquery()
                )
                query = query.filter(Video.id.in_(tag_subquery))

        # Apply date filters
        if date_from:
            query = query.filter(Video.created_date >= date_from)
        if date_to:
            query = query.filter(Video.created_date <= date_to)

        # Get total count before pagination
        total = query.count()

        # Apply sorting
        query = self._apply_sorting(query, sort)

        # Apply pagination
        offset = (page - 1) * limit
        videos = query.offset(offset).limit(limit).all()

        return videos, total

    def _apply_sorting(self, query, sort: str):
        """
        Apply sorting to the query.

        Args:
            query: SQLAlchemy query
            sort: Sort parameter

        Returns:
            Query with sorting applied
        """
        sort_options = {
            "date_desc": Video.created_date.desc().nullslast(),
            "date_asc": Video.created_date.asc().nullsfirst(),
            "name_asc": Video.filename.asc(),
            "name_desc": Video.filename.desc(),
            "duration_desc": Video.duration.desc().nullslast(),
            "duration_asc": Video.duration.asc().nullsfirst(),
            "size_desc": Video.file_size.desc().nullslast(),
            "size_asc": Video.file_size.asc().nullsfirst(),
        }

        sort_order = sort_options.get(sort, Video.created_date.desc().nullslast())
        return query.order_by(sort_order)

    def get_videos_by_ids(self, db: Session, video_ids: List[int]) -> List[Video]:
        """
        Get multiple videos by their IDs.

        Args:
            db: Database session
            video_ids: List of video IDs

        Returns:
            List of Video objects
        """
        return (
            db.query(Video)
            .options(
                joinedload(Video.video_metadata),
                joinedload(Video.video_tags).joinedload(VideoTag.tag)
            )
            .filter(Video.id.in_(video_ids))
            .all()
        )

    def get_videos_by_category(
        self,
        db: Session,
        category: str,
        page: int = 1,
        limit: int = 50
    ) -> Tuple[List[Video], int]:
        """
        Get all videos in a specific category.

        Args:
            db: Database session
            category: Category name
            page: Page number
            limit: Results per page

        Returns:
            Tuple of (videos list, total count)
        """
        return self.search_videos(
            db=db,
            category=category,
            page=page,
            limit=limit
        )

    def get_videos_by_tag(
        self,
        db: Session,
        tag: str,
        page: int = 1,
        limit: int = 50
    ) -> Tuple[List[Video], int]:
        """
        Get all videos with a specific tag.

        Args:
            db: Database session
            tag: Tag name
            page: Page number
            limit: Results per page

        Returns:
            Tuple of (videos list, total count)
        """
        return self.search_videos(
            db=db,
            tags=[tag],
            page=page,
            limit=limit
        )

    def get_recent_videos(
        self,
        db: Session,
        limit: int = 20
    ) -> List[Video]:
        """
        Get recently indexed videos.

        Args:
            db: Database session
            limit: Number of videos to return

        Returns:
            List of recently indexed videos
        """
        return (
            db.query(Video)
            .options(
                joinedload(Video.video_metadata),
                joinedload(Video.video_tags).joinedload(VideoTag.tag)
            )
            .order_by(Video.indexed_date.desc())
            .limit(limit)
            .all()
        )

    def get_statistics(self, db: Session) -> dict:
        """
        Get database statistics.

        Args:
            db: Database session

        Returns:
            Dictionary with statistics
        """
        total_videos = db.query(func.count(Video.id)).scalar()
        total_duration = db.query(func.sum(Video.duration)).scalar() or 0
        total_size = db.query(func.sum(Video.file_size)).scalar() or 0
        total_categories = (
            db.query(func.count(func.distinct(Metadata.category)))
            .filter(Metadata.category.isnot(None))
            .scalar()
        )
        total_tags = db.query(func.count(Tag.id)).scalar()

        return {
            "total_videos": total_videos,
            "total_duration": total_duration,
            "total_size": total_size,
            "total_categories": total_categories,
            "total_tags": total_tags,
        }


# Global search service instance
search_service = SearchService()
