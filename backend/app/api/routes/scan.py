from fastapi import APIRouter, HTTPException, status
from pydantic import BaseModel, Field

from app.services.scanner import video_scanner
from app.config import config_manager
from app.database import SessionLocal


router = APIRouter(prefix="/scan", tags=["scan"])


class ScanRequest(BaseModel):
    """Request to start a directory scan."""

    directory: str = Field(..., description="Directory path to scan")
    save_config: bool = Field(
        default=True,
        description="Save directory to configuration"
    )


class ScanResponse(BaseModel):
    """Response for scan start."""

    status: str
    scan_id: str
    message: str


class ScanStatusResponse(BaseModel):
    """Response for scan status."""

    scan_id: str
    status: str
    total: int
    processed: int
    successful: int
    failed: int
    skipped: int
    current_file: str
    errors: list[str]
    start_time: str
    end_time: str | None
    elapsed_seconds: float
    eta_seconds: float | None


@router.options("")
async def scan_options():
    """Handle OPTIONS preflight for scan endpoint."""
    return {"status": "ok"}


@router.post("", response_model=ScanResponse)
async def start_scan(
    scan_request: ScanRequest,
):
    """
    Start scanning a directory for videos.

    This operation runs in a background thread and indexes all video files found
    in the specified directory and its subdirectories.

    Args:
        scan_request: Scan request with directory path

    Returns:
        Scan ID and status for tracking progress
    """
    # Save configuration if requested
    if scan_request.save_config:
        config_manager.save_config(video_directory=scan_request.directory)

    # Start scan in background thread
    try:
        scan_id = video_scanner.start_scan(scan_request.directory, SessionLocal)

        return ScanResponse(
            status="started",
            scan_id=scan_id,
            message=f"Scan started for directory: {scan_request.directory}"
        )
    except ValueError as e:
        raise HTTPException(
            status_code=status.HTTP_400_BAD_REQUEST,
            detail=str(e)
        )
    except Exception as e:
        raise HTTPException(
            status_code=status.HTTP_500_INTERNAL_SERVER_ERROR,
            detail=f"Failed to start scan: {str(e)}"
        )


@router.get("/status/{scan_id}", response_model=ScanStatusResponse)
async def get_scan_status(scan_id: str):
    """
    Get the status of a running or completed scan.

    Args:
        scan_id: Scan ID returned from start_scan

    Returns:
        Scan progress and status information
    """
    progress = video_scanner.get_scan_progress(scan_id)

    if not progress:
        raise HTTPException(
            status_code=status.HTTP_404_NOT_FOUND,
            detail=f"Scan not found: {scan_id}"
        )

    return ScanStatusResponse(**progress)


@router.post("/rescan", response_model=ScanResponse)
async def rescan_directory():
    """
    Rescan the configured video directory.

    Scans the directory that was previously configured, adding any new videos
    that have been added since the last scan.

    Returns:
        Scan ID and status for tracking progress
    """
    if not config_manager.is_configured():
        raise HTTPException(
            status_code=status.HTTP_428_PRECONDITION_REQUIRED,
            detail="No video directory configured. Use /scan endpoint first."
        )

    video_dir = config_manager.settings.video_directory

    try:
        scan_id = video_scanner.start_scan(video_dir, SessionLocal)

        return ScanResponse(
            status="started",
            scan_id=scan_id,
            message=f"Rescan started for directory: {video_dir}"
        )
    except Exception as e:
        raise HTTPException(
            status_code=status.HTTP_500_INTERNAL_SERVER_ERROR,
            detail=f"Failed to start rescan: {str(e)}"
        )
