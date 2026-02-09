from typing import Generator
from fastapi import Depends, HTTPException, status
from sqlalchemy.orm import Session

from app.database import get_db
from app.config import config_manager


def get_database() -> Generator:
    """
    Dependency for database session.

    Yields:
        Database session
    """
    yield from get_db()


def verify_configuration() -> None:
    """
    Dependency to verify that the application is configured.

    Raises:
        HTTPException: If application is not configured
    """
    if not config_manager.is_configured():
        raise HTTPException(
            status_code=status.HTTP_428_PRECONDITION_REQUIRED,
            detail="Application not configured. Please complete initial setup."
        )
