"""Health, readiness, config and metrics endpoints (no auth)."""

from __future__ import annotations

from fastapi import APIRouter, Request, Response
from fastapi.responses import JSONResponse, PlainTextResponse

from ..metrics import get_metrics, get_registry
from ..state import AppState

router = APIRouter(tags=["health"])


def _health_payload(state: AppState, telegram_connected: bool, ready: bool) -> dict:
    settings = state.settings
    # Storage health
    db_ok = False
    try:
        state.storage._query("SELECT 1")
        db_ok = True
    except Exception:
        pass
    # Disk free space check (data dir)
    import shutil
    disk_free_mb = -1
    try:
        usage = shutil.disk_usage(settings.data_dir)
        disk_free_mb = usage.free // (1024 * 1024)
    except Exception:
        pass
    return {
        "status": "ok" if ready else "not_ready",
        "version": state.version,
        "telegram_connected": telegram_connected,
        "uptime_secs": state.uptime_secs,
        "build": f"{state.version}-local",
        "ready": ready,
        "transport_mode": state.effective_transport_mode(),
        "bot_configured": state.bot_configured,
        "user_configured": state.user_configured,
        "db_connected": db_ok,
        "disk_free_mb": disk_free_mb,
        "upload_queue": state.transfers.queue_status(),
        "metadata_cache_enabled": settings.metadata_cache_enabled,
        "metadata_cache_ttl_secs": settings.metadata_cache_ttl_secs,
        "public_file_id_download": settings.public_file_id_download,
        "upload_share_ttl_hours": settings.upload_share_ttl_hours,
        "presigned_download_enabled": len(settings.download_signing_secret) >= 32,
        "multi_tenant_enabled": settings.multi_tenant_enabled,
    }


@router.get("/api/v1/health")
async def api_health(request: Request) -> JSONResponse:
    state: AppState = request.app.state.app
    telegram_connected = await state.telegram.is_authorized()
    ready = await state.is_ready()
    # /api/v1/health always returns 200 (contract with web UI + e2e).
    return JSONResponse(_health_payload(state, telegram_connected, ready))


@router.get("/health/live")
async def health_live(request: Request) -> JSONResponse:
    state: AppState = request.app.state.app
    return JSONResponse(
        {"status": "alive", "uptime_secs": state.uptime_secs, "version": state.version}
    )


@router.get("/health/ready")
async def health_ready(request: Request) -> JSONResponse:
    state: AppState = request.app.state.app
    telegram_connected = await state.telegram.is_authorized()
    ready = await state.is_ready()
    payload = _health_payload(state, telegram_connected, ready)
    status_code = 200 if ready else 503
    return JSONResponse(payload, status_code=status_code)


@router.get("/config")
async def legacy_config(request: Request) -> dict:
    """tg-disk compatible config endpoint (no auth)."""
    state: AppState = request.app.state.app
    settings = state.settings
    return {
        "chunk_size_mb": settings.chunk_size_mb,
        "chunk_concurrent": settings.chunk_concurrent,
        "files_concurrent": settings.files_concurrent,
        "download_threads": settings.download_threads,
        "stream_port": settings.port,
        "api_version": state.version,
        "transport_mode": state.effective_transport_mode(),
        "bot_configured": state.bot_configured,
        "user_configured": state.user_configured,
        "upload_queue": state.transfers.queue_status(),
        "metadata_cache_enabled": settings.metadata_cache_enabled,
        "metadata_cache_ttl_secs": settings.metadata_cache_ttl_secs,
        "public_file_id_download": settings.public_file_id_download,
        "upload_share_ttl_hours": settings.upload_share_ttl_hours,
    }


@router.get("/metrics")
async def metrics(request: Request) -> Response:
    state: AppState = request.app.state.app
    if not state.settings.metrics_enabled:
        return PlainTextResponse("metrics disabled", status_code=404)
    registry = get_registry()
    queue = state.transfers.queue_status()
    registry.upload_slots_available.set(queue["file_slots_available"])
    registry.upload_chunk_slots_available.set(queue["chunk_slots_available"])
    return Response(content=get_metrics(), media_type="text/plain; version=0.0.4")
