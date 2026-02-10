from fastapi import APIRouter, Depends, HTTPException, status
from sqlalchemy.orm import Session

from app.api.deps import get_database
from app.schemas.production import ProductionCreate, ProductionResponse
from app.services.production_service import production_service


router = APIRouter(prefix="/productions", tags=["productions"])


@router.get("", response_model=list[ProductionResponse])
async def list_productions(db: Session = Depends(get_database)):
    """List all productions with video counts."""
    results = production_service.get_all_productions(db)
    return [ProductionResponse(**r) for r in results]


@router.post("", response_model=ProductionResponse, status_code=status.HTTP_201_CREATED)
async def create_production(
    data: ProductionCreate,
    db: Session = Depends(get_database)
):
    """Create a new production."""
    try:
        production = production_service.create_production(db, data)
    except Exception:
        raise HTTPException(
            status_code=status.HTTP_400_BAD_REQUEST,
            detail=f"Production with title '{data.title}' already exists"
        )
    return ProductionResponse(
        id=production.id,
        title=production.title,
        platform=production.platform,
        link=production.link,
        is_published=production.is_published,
        video_count=0
    )


@router.put("/{production_id}", response_model=ProductionResponse)
async def update_production(
    production_id: int,
    data: ProductionCreate,
    db: Session = Depends(get_database)
):
    """Update an existing production."""
    production = production_service.update_production(db, production_id, data)
    if not production:
        raise HTTPException(
            status_code=status.HTTP_404_NOT_FOUND,
            detail=f"Production not found: {production_id}"
        )
    # Re-fetch with count
    all_prods = production_service.get_all_productions(db)
    for p in all_prods:
        if p["id"] == production_id:
            return ProductionResponse(**p)
    return ProductionResponse(
        id=production.id,
        title=production.title,
        platform=production.platform,
        link=production.link,
        is_published=production.is_published,
        video_count=0
    )


@router.delete("/{production_id}")
async def delete_production(
    production_id: int,
    db: Session = Depends(get_database)
):
    """Delete a production."""
    success = production_service.delete_production(db, production_id)
    if not success:
        raise HTTPException(
            status_code=status.HTTP_404_NOT_FOUND,
            detail=f"Production not found: {production_id}"
        )
    return {"message": f"Production {production_id} deleted successfully"}
