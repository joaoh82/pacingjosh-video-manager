from typing import Optional, List
from sqlalchemy.orm import Session
from sqlalchemy import func

from app.models import Production, VideoProduction
from app.schemas.production import ProductionCreate


class ProductionService:
    """Service for production CRUD operations."""

    def get_all_productions(self, db: Session) -> List[dict]:
        """Get all productions with video counts."""
        results = (
            db.query(
                Production.id,
                Production.title,
                Production.platform,
                Production.link,
                Production.is_published,
                func.count(VideoProduction.video_id).label('video_count')
            )
            .outerjoin(VideoProduction, Production.id == VideoProduction.production_id)
            .group_by(Production.id, Production.title, Production.platform, Production.link, Production.is_published)
            .order_by(Production.title)
            .all()
        )

        return [
            {"id": id, "title": title, "platform": platform, "link": link, "is_published": is_published, "video_count": video_count}
            for id, title, platform, link, is_published, video_count in results
        ]

    def get_production(self, db: Session, production_id: int) -> Optional[Production]:
        """Get a production by ID."""
        return db.query(Production).filter(Production.id == production_id).first()

    def create_production(self, db: Session, data: ProductionCreate) -> Production:
        """Create a new production."""
        production = Production(
            title=data.title,
            platform=data.platform,
            link=data.link,
            is_published=data.is_published,
        )
        db.add(production)
        db.commit()
        db.refresh(production)
        return production

    def update_production(self, db: Session, production_id: int, data: ProductionCreate) -> Optional[Production]:
        """Update an existing production."""
        production = self.get_production(db, production_id)
        if not production:
            return None

        production.title = data.title
        production.platform = data.platform
        production.link = data.link
        production.is_published = data.is_published
        db.commit()
        db.refresh(production)
        return production

    def delete_production(self, db: Session, production_id: int) -> bool:
        """Delete a production (cascade removes junction rows)."""
        production = self.get_production(db, production_id)
        if not production:
            return False

        db.delete(production)
        db.commit()
        return True


# Global production service instance
production_service = ProductionService()
