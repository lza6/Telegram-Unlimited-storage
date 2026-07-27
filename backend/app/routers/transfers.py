"""Transfer Center API Routes (TASK-U-02).

Provides:
- GET  /api/v1/transfers           → list all active transfers
- POST /api/v1/transfers/{id}/cancel
- POST /api/v1/transfers/{id}/retry
- POST /api/v1/transfers/{id}/pause
- GET  /api/v1/transfers/events    → SSE stream of progress updates
"""

from __future__ import annotations

import asyncio
import json
from typing import Any, Optional

from fastapi import APIRouter, Request
from fastapi.responses import JSONResponse, StreamingResponse

from ..state import AppState

router = APIRouter(prefix="/api/v1", tags=["transfers"])


def get_state(request: Request) -> AppState:
    return request.app.state.app


def api_error(code: str, message: str, status_code: int) -> JSONResponse:
    return JSONResponse(
        {"error": {"code": code, "message": message}}, status_code=status_code
    )


@router.get("/transfers")
async def list_transfers(request: Request) -> JSONResponse:
    """List all tracked transfers (active + recently completed)."""
    state = get_state(request)
    state.authenticator.require_auth(request)
    snapshots = state.transfers.list_all_progress()
    return JSONResponse({"transfers": snapshots})


@router.post("/transfers/{session_id}/cancel")
async def cancel_transfer(session_id: str, request: Request) -> JSONResponse:
    state = get_state(request)
    state.authenticator.require_auth(request)
    ok = state.transfers.cancel_transfer(session_id)
    if not ok:
        return api_error("NOT_FOUND", "Transfer session not found", 404)
    return JSONResponse({"ok": True, "session_id": session_id, "status": "cancelled"})


@router.post("/transfers/{session_id}/retry")
async def retry_transfer(session_id: str, request: Request) -> JSONResponse:
    state = get_state(request)
    state.authenticator.require_auth(request)
    ok = state.transfers.retry_transfer(session_id)
    if not ok:
        return api_error("NOT_FOUND", "Transfer not found or not retryable", 404)
    return JSONResponse({"ok": True, "session_id": session_id, "status": "queued"})


@router.post("/transfers/{session_id}/pause")
async def pause_transfer(session_id: str, request: Request) -> JSONResponse:
    state = get_state(request)
    state.authenticator.require_auth(request)
    ok = state.transfers.pause_transfer(session_id)
    if not ok:
        return api_error("NOT_FOUND", "Transfer session not found", 404)
    return JSONResponse({"ok": True, "session_id": session_id, "status": "paused"})


@router.get("/transfers/events")
async def transfers_events(request: Request) -> StreamingResponse:
    """SSE stream of all transfer progress updates.

    Client subscribes and receives snapshot arrays. Falls back gracefully
    if the client loses connection (browser auto-reconnects SSE).
    """
    state = get_state(request)
    state.authenticator.require_auth(request)

    async def event_generator() -> Any:
        # Send initial snapshot immediately
        initial = state.transfers.list_all_progress()
        yield f"data: {json.dumps(initial)}\n\n"

        # Poll every 2 seconds for changes (simple approach; could use bus per session)
        while True:
            await asyncio.sleep(2)
            if await request.is_disconnected():
                break
            snapshots = state.transfers.list_all_progress()
            yield f"data: {json.dumps(snapshots)}\n\n"

    return StreamingResponse(
        event_generator(),
        media_type="text/event-stream",
        headers={
            "Cache-Control": "no-cache",
            "Connection": "keep-alive",
            "X-Accel-Buffering": "no",
        },
    )
