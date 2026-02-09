from typing import Optional, List
from sqlalchemy.orm import Session, joinedload
from sqlalchemy import func

from app.models import Video, Metadata, Tag, VideoTag, VideoUsage
from app.schemas.video import VideoUpdate, BulkUpdateRequest


class VideoService:
    """Service for video CRUD operations and metadata management."""

    def get_video(self, db: Session, video_id: int) -> Optional[Video]:
        """
        Get a video by ID with all related data.

        Args:
            db: Database session
            video_id: Video ID

        Returns:
            Video object with metadata and tags, or None if not found
        """
        return (
            db.query(Video)
            .options(
                joinedload(Video.video_metadata),
                joinedload(Video.video_tags).joinedload(VideoTag.tag),
                joinedload(Video.video_usages)
            )
            .filter(Video.id == video_id)
            .first()
        )

    def get_video_by_path(self, db: Session, file_path: str) -> Optional[Video]:
        """
        Get a video by file path.

        Args:
            db: Database session
            file_path: Video file path

        Returns:
            Video object or None if not found
        """
        return db.query(Video).filter(Video.file_path == file_path).first()

    def update_video(
        self,
        db: Session,
        video_id: int,
        update_data: VideoUpdate
    ) -> Optional[Video]:
        """
        Update video metadata and tags.

        Args:
            db: Database session
            video_id: Video ID
            update_data: Update data

        Returns:
            Updated video object, or None if not found
        """
        video = self.get_video(db, video_id)
        if not video:
            return None

        # Update metadata
        if video.video_metadata is None:
            video.video_metadata = Metadata(video_id=video_id)
            db.add(video.video_metadata)

        if update_data.category is not None:
            video.video_metadata.category = update_data.category
        if update_data.location is not None:
            video.video_metadata.location = update_data.location
        if update_data.notes is not None:
            video.video_metadata.notes = update_data.notes

        # Update tags
        if update_data.tags is not None:
            self._update_video_tags(db, video, update_data.tags)

        # Update usages
        if update_data.usages is not None:
            self._update_video_usages(db, video, update_data.usages)

        db.commit()
        db.refresh(video)
        return video

    def bulk_update_videos(
        self,
        db: Session,
        bulk_update: BulkUpdateRequest
    ) -> int:
        """
        Update multiple videos at once.

        Args:
            db: Database session
            bulk_update: Bulk update data

        Returns:
            Number of videos updated
        """
        videos = (
            db.query(Video)
            .options(joinedload(Video.video_metadata))
            .filter(Video.id.in_(bulk_update.video_ids))
            .all()
        )

        for video in videos:
            # Ensure metadata exists
            if video.video_metadata is None:
                video.video_metadata = Metadata(video_id=video.id)
                db.add(video.video_metadata)

            # Update metadata fields
            if bulk_update.category is not None:
                video.video_metadata.category = bulk_update.category
            if bulk_update.location is not None:
                video.video_metadata.location = bulk_update.location
            if bulk_update.notes is not None:
                video.video_metadata.notes = bulk_update.notes

            # Add tags
            if bulk_update.add_tags:
                self._add_tags_to_video(db, video, bulk_update.add_tags)

            # Remove tags
            if bulk_update.remove_tags:
                self._remove_tags_from_video(db, video, bulk_update.remove_tags)

        db.commit()
        return len(videos)

    def delete_video(self, db: Session, video_id: int) -> bool:
        """
        Delete a video from the database.

        Args:
            db: Database session
            video_id: Video ID

        Returns:
            True if deleted, False if not found
        """
        video = db.query(Video).filter(Video.id == video_id).first()
        if not video:
            return False

        db.delete(video)
        db.commit()
        return True

    def _update_video_tags(self, db: Session, video: Video, tag_names: List[str]) -> None:
        """
        Replace all tags for a video.

        Args:
            db: Database session
            video: Video object
            tag_names: List of tag names
        """
        # Remove existing tags
        db.query(VideoTag).filter(VideoTag.video_id == video.id).delete()

        # Add new tags
        for tag_name in tag_names:
            tag = self._get_or_create_tag(db, tag_name.strip())
            video_tag = VideoTag(video_id=video.id, tag_id=tag.id)
            db.add(video_tag)

    def _add_tags_to_video(self, db: Session, video: Video, tag_names: List[str]) -> None:
        """
        Add tags to a video (doesn't remove existing tags).

        Args:
            db: Database session
            video: Video object
            tag_names: List of tag names to add
        """
        # Get existing tag IDs
        existing_tag_ids = {vt.tag_id for vt in video.video_tags}

        for tag_name in tag_names:
            tag = self._get_or_create_tag(db, tag_name.strip())
            if tag.id not in existing_tag_ids:
                video_tag = VideoTag(video_id=video.id, tag_id=tag.id)
                db.add(video_tag)

    def _remove_tags_from_video(self, db: Session, video: Video, tag_names: List[str]) -> None:
        """
        Remove tags from a video.

        Args:
            db: Database session
            video: Video object
            tag_names: List of tag names to remove
        """
        # Get tag IDs to remove
        tags_to_remove = (
            db.query(Tag.id)
            .filter(Tag.name.in_([t.strip() for t in tag_names]))
            .all()
        )
        tag_ids_to_remove = {t.id for t in tags_to_remove}

        # Remove video_tags
        db.query(VideoTag).filter(
            VideoTag.video_id == video.id,
            VideoTag.tag_id.in_(tag_ids_to_remove)
        ).delete(synchronize_session=False)

    def _get_or_create_tag(self, db: Session, tag_name: str) -> Tag:
        """
        Get an existing tag or create a new one.

        Args:
            db: Database session
            tag_name: Tag name

        Returns:
            Tag object
        """
        tag = db.query(Tag).filter(Tag.name == tag_name).first()
        if not tag:
            tag = Tag(name=tag_name)
            db.add(tag)
            db.flush()
        return tag

    def _update_video_usages(self, db: Session, video: Video, usages: list) -> None:
        """
        Replace all usages for a video.

        Args:
            db: Database session
            video: Video object
            usages: List of UsageEntry objects with title and link
        """
        # Remove existing usages
        db.query(VideoUsage).filter(VideoUsage.video_id == video.id).delete()

        # Add new usages
        for usage in usages:
            video_usage = VideoUsage(
                video_id=video.id,
                title=usage.title,
                link=usage.link
            )
            db.add(video_usage)

    def get_all_categories(self, db: Session) -> List[dict]:
        """
        Get all unique categories with usage count.

        Args:
            db: Database session

        Returns:
            List of category dictionaries with name and count
        """
        results = (
            db.query(
                Metadata.category,
                func.count(Metadata.category).label('count')
            )
            .filter(Metadata.category.isnot(None))
            .group_by(Metadata.category)
            .order_by(Metadata.category)
            .all()
        )

        return [{"name": cat, "count": count} for cat, count in results]

    def get_all_tags(self, db: Session) -> List[dict]:
        """
        Get all tags with usage count.

        Args:
            db: Database session

        Returns:
            List of tag dictionaries with id, name, and count
        """
        results = (
            db.query(
                Tag.id,
                Tag.name,
                func.count(VideoTag.video_id).label('count')
            )
            .outerjoin(VideoTag, Tag.id == VideoTag.tag_id)
            .group_by(Tag.id, Tag.name)
            .order_by(Tag.name)
            .all()
        )

        return [{"id": id, "name": name, "count": count} for id, name, count in results]


# Global video service instance
video_service = VideoService()
